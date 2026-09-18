-- Relay — canonical SQLite schema
--
-- ⚠️ THIS FILE IS COMPILED INTO THE BINARY. `db/mod.rs` does
--    `const SCHEMA: &str = include_str!("../../../docs/data/schema.sql");`
-- so it is not a transcript of the schema — it IS the baseline schema every
-- fresh install is created from. Editing it changes the product. Deleting or
-- moving it breaks the build.
--
-- What it is NOT is the whole story: the `ensure_*` rungs in `db/*.rs` and the
-- `PRAGMA user_version` ladder (`db::SCHEMA_VERSION`) evolve a database created
-- from this baseline. A column added by a rung will not appear here, and that is
-- correct — but keeping the two agreeing is manual, and that is tracked as debt
-- in docs/KNOWN_ISSUES.md §4.
--
-- Rule of thumb: a NEW table or column for a fresh install belongs here AND in a
-- rung (so existing installs get it too). To dump what a live database actually
-- has, which is the tiebreaker:
--   sqlite3 "$HOME/Library/Application Support/com.relay.app/relay.db" .schema
-- (defaults shown below match the code — a dev DB created before an amendment may
-- show stale column DEFAULTs, e.g. voice_profiles auto_fire/suggest.)
--
-- Schema evolution is by idempotent `ensure_*`/migration rungs; see
-- `db::SCHEMA_VERSION` and `run_migrations` in db/mod.rs, and the retryable-
-- migration rule in CLAUDE.md §25.

PRAGMA foreign_keys = ON;

-- ===== Reference data (db/verses.rs) =====

CREATE TABLE translations (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,          -- e.g. "King James Version"
    abbreviation  TEXT NOT NULL,          -- e.g. "KJV"
    language      TEXT NOT NULL,          -- ISO 639-1/3 code, e.g. "en", "yo", "sw", "ha"
    license_type  TEXT                    -- public domain / licensed, etc.
);

CREATE TABLE verses (
    id             INTEGER PRIMARY KEY,
    translation_id INTEGER NOT NULL REFERENCES translations(id),
    book           TEXT NOT NULL,         -- canonical book name
    chapter        INTEGER NOT NULL,
    verse          INTEGER NOT NULL,
    text           TEXT NOT NULL,
    embedding      BLOB                   -- precomputed vector for semantic match; NEVER YET WRITTEN (see docs/KNOWN_ISSUES.md)
);
CREATE INDEX idx_verses_lookup ON verses(translation_id, book, chapter, verse);

-- Full-text recall, sitting BEHIND the reference/phrase/semantic ranker (DECISIONS).
-- Contentless FTS5 mirror of verses(text); rebuilt idempotently by migration.
CREATE VIRTUAL TABLE verses_fts USING fts5(
    text, content='verses', content_rowid='id', tokenize='porter unicode61');

-- ===== Presentation (db/templates.rs, db/channels.rs) =====

CREATE TABLE templates (
    id                 INTEGER PRIMARY KEY,
    name               TEXT NOT NULL,
    region_config_json TEXT NOT NULL,     -- layout regions, see docs/SPEC.md §5
    style_json         TEXT NOT NULL,     -- fonts, colors, transitions
    console_active     INTEGER NOT NULL DEFAULT 0, -- one of the (max 4) styles on the console output grid
    seed_key           TEXT,               -- the seeder's stable slug; NULL = hand-made. Never written by an operator.
    edited_at          TEXT                -- stamped by any save that is not the seeder's
);

CREATE TABLE output_channels (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL,
    render_target  TEXT NOT NULL CHECK (render_target IN ('native_window', 'ndi_encode', 'network_client')),
    template_id    INTEGER REFERENCES templates(id),
    display_target TEXT,                  -- display index, NDI source name, or kiosk client id
    status         TEXT NOT NULL DEFAULT 'offline' CHECK (status IN ('online', 'offline')),
    role           TEXT CHECK (role IS NULL OR role IN ('main', 'stage')),  -- what this screen is FOR; NULL = a congregation screen with no special job
    -- WHICH KINDS OF CONTENT THIS SCREEN SHOWS AT ALL (DECISIONS §98). A JSON
    -- array of content kinds, ANDed with the template's own `layout.shows` so it
    -- can only ever NARROW. NULL = no opinion, follow the template -- which is
    -- the behaviour every install had before this column, and is why there is no
    -- back-fill. NULL and [] are deliberately different: the first shows
    -- everything the template does, the second shows nothing.
    shows_json     TEXT
);

-- WHAT ONE SCREEN WEARS FOR ONE KIND OF CONTENT (db/channels.rs).
--
-- A ROW PER (screen, kind), not a column per kind: the five content kinds are
-- already mirrored by hand in three places, and a column would make DDL the
-- fourth mirror and the least editable of them (SQLite cannot drop or rename one
-- without a table rebuild at boot -- CLAUDE.md rule 25).
--
-- template_id is NOT NULL deliberately: NO ROW is the only way to say "this kind
-- inherits". An absent row and a NULL row would have to mean the same thing at
-- every reader, and two spellings of one fact is what output_channels.role
-- already refuses.
--
-- kind carries no CHECK, for the same rule-25 reason: SQLite cannot ALTER a
-- CHECK, so a sixth content kind would mean a rebuild before the window is
-- shown. A kind nobody reads is an inert row.
CREATE TABLE channel_looks (
    channel_id  INTEGER NOT NULL REFERENCES output_channels(id) ON DELETE CASCADE,
    kind        TEXT    NOT NULL,         -- one of CONTENT_KINDS (src/lib/layers.js)
    template_id INTEGER NOT NULL REFERENCES templates(id),
    PRIMARY KEY (channel_id, kind)
);

-- ===== Service plans & the unified cue (db/plans.rs) =====

CREATE TABLE service_plans (
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL,
    plan_date  TEXT NOT NULL DEFAULT '',
    notes      TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ''
);

-- One polymorphic cue for every content type. cue_type selects how payload_json
-- is read; template_id is an optional per-content-type override. See docs/DATA_MODEL.md §4.
--
-- The last three columns arrived after this table did, each through the idempotent
-- `add_plan_item_column` in db/plans.rs, and for a while this file did not show
-- them at all: a fresh install got them from the migration rather than from the
-- baseline, so the two agreed by accident. They are stated here now, because this
-- file IS the shipped fresh-install shape.
CREATE TABLE plan_items (
    id            INTEGER PRIMARY KEY,
    plan_id       INTEGER NOT NULL REFERENCES service_plans(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    cue_type      TEXT NOT NULL,          -- 'scripture' | 'song' | 'media' | 'announce' | 'countdown'
                                          -- (the canonical list is CONTENT_KINDS in src/lib/layers.js)
    label         TEXT NOT NULL,
    payload_json  TEXT NOT NULL DEFAULT '{}',
    template_id   INTEGER,
    section_title TEXT NOT NULL DEFAULT '',  -- non-empty = this cue BEGINS a section
    duration_sec  INTEGER NOT NULL DEFAULT 0, -- planned length, for the running-time
                                              -- estimate only (plan.js). 0 = untimed.
    timer_minutes INTEGER                     -- a programme clock this cue asks Live to
                                              -- start when it goes on air. NULL = unbound,
                                              -- which is NOT the same as 0. See PlanItem.
);
CREATE INDEX idx_plan_items ON plan_items(plan_id, position);

-- ===== Content library (db/songs.rs, db/library.rs) =====

CREATE TABLE songs (
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL,
    author     TEXT NOT NULL DEFAULT '',
    ccli       TEXT NOT NULL DEFAULT '',
    song_key   TEXT NOT NULL DEFAULT '',
    bpm        INTEGER,
    tags       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE song_sections (
    id       INTEGER PRIMARY KEY,
    song_id  INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    tag      TEXT NOT NULL,
    label    TEXT NOT NULL,
    lyrics   TEXT NOT NULL
);
CREATE INDEX idx_song_sections ON song_sections(song_id, position);

-- Named play-orders as section-index sequences (NOT copied lyrics), so a lyric
-- edit re-expands into the right slots. "Standard" is implicit, never stored.
CREATE TABLE song_arrangements (
    id       INTEGER PRIMARY KEY,
    song_id  INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    name     TEXT NOT NULL,
    sequence TEXT NOT NULL DEFAULT '[]',  -- JSON array of section indices
    -- The song's STRUCTURE when this order was built: [[tag, label], …], lyrics
    -- excluded on purpose. A lyric edit must not disturb an arrangement (that is
    -- what storing indices buys); a reorder, insert, delete or rename must, because
    -- index 3 then names a section nobody chose. Empty = built before this column,
    -- and is never reported stale — a claim from an absence. DECISIONS §55.
    built_shape TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_song_arrangements ON song_arrangements(song_id);

CREATE TABLE saved_scripture (
    id          INTEGER PRIMARY KEY,
    reference   TEXT NOT NULL UNIQUE,
    book        TEXT NOT NULL,
    chapter     INTEGER NOT NULL,
    verse       INTEGER NOT NULL,
    text        TEXT NOT NULL,
    translation TEXT NOT NULL DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT ''
);

CREATE TABLE announcements (
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL DEFAULT '',
    body       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ''
);

CREATE TABLE media_assets (
    id         INTEGER PRIMARY KEY,
    kind       TEXT NOT NULL,
    filename   TEXT NOT NULL,
    path       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT ''
);

-- ===== Live session data (db/services.rs) =====

CREATE TABLE services (
    id    INTEGER PRIMARY KEY,
    date  TEXT NOT NULL,                  -- ISO 8601 date
    title TEXT NOT NULL
);

CREATE TABLE transcripts (
    id         INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id),
    timestamp  REAL NOT NULL,             -- seconds since service start
    text       TEXT NOT NULL,
    language   TEXT NOT NULL,             -- detected language for this chunk (code-switching is normal)
    confidence REAL                       -- STT confidence, 0-1
);

-- Every history query starts from a service and walks down. Without these,
-- `service_transcripts`, `service_detections`, the timeline merge, the replay and
-- `delete_service` all scan the whole table — and these are the two tables that
-- grow without limit, one row per utterance, for as long as a church keeps using
-- Relay. A year of Sundays is six figures of rows.
--
-- SQLite creates an index for a PRIMARY KEY and for a UNIQUE constraint. It does
-- NOT create one for a REFERENCES clause, which is what these three columns are.
CREATE TABLE detections (
    id            INTEGER PRIMARY KEY,
    transcript_id INTEGER NOT NULL REFERENCES transcripts(id),
    verse_id      INTEGER REFERENCES verses(id),
    method        TEXT NOT NULL CHECK (method IN ('direct', 'semantic')),
    confidence    REAL NOT NULL,
    -- What actually happened. 'manual' means a HUMAN put this on screen (override,
    -- confirmed suggestion, or next/back nav) — NOT an AI decision, and must never
    -- be counted as one: the self-calibrating router learns from this column.
    status        TEXT NOT NULL CHECK (status IN ('auto', 'suggested', 'dismissed', 'manual')),
    fired_at      REAL,                   -- seconds since service start, null if never fired
    -- THE EVIDENCE: the exact text the detector was reading when this fired.
    --
    -- transcript_id alone cannot answer "why did this verse appear?". Detection
    -- runs on every partial STT hypothesis, and only FINAL transcripts are
    -- stored — so a fire is stamped onto whichever final happened to be most
    -- recent, which may be minutes old and may not contain the reference at all.
    -- A live service produced nine auto-fires attributed to a sentence that,
    -- replayed through the detector, yields nothing. Without this column a wrong
    -- verse on a wall is not diagnosable after the fact.
    heard_text    TEXT
);

CREATE INDEX idx_transcripts_service ON transcripts(service_id);
CREATE INDEX idx_detections_transcript ON detections(transcript_id);
-- Not for a query: the corpus repair deletes every verse, and without this
-- SQLite scans `detections` once per deleted parent to enforce the reference.
CREATE INDEX idx_detections_verse ON detections(verse_id);

-- Operator-action log for a running service (distinct from a plan_items cue).
CREATE TABLE cues (
    id           INTEGER PRIMARY KEY,
    service_id   INTEGER NOT NULL REFERENCES services(id),
    type         TEXT NOT NULL,           -- e.g. "manual_override", "clear_screens", "blackout",
                                          -- "suggestion_accepted", "suggestion_dismissed"
    payload_json TEXT,
    triggered_at REAL NOT NULL
);

CREATE INDEX idx_cues_service ON cues(service_id);

-- ===== The service timeline (db/services.rs) =====
--
-- ONE ordered, append-only record of what happened during a service, for the
-- facts that had no home anywhere else: the service starting and ending, the
-- panic controls, rehearsal going on and off, a screen going silent and coming
-- back, the operator lifting the service lock.
--
-- It does NOT duplicate `detections` or `cues`. Those already hold what the AI
-- decided and what the operator pressed, and copying them here would create two
-- answers to one question. The timeline READ merges all three; only the events
-- with nowhere else to live are written here.
--
-- Append-only by convention and by the absence of any UPDATE or DELETE in the
-- code. `seq` is monotonic per service so two events in the same millisecond
-- still have an order — a service produced two fires sharing a timestamp to the
-- tenth of a second once, and the ordering was unrecoverable afterwards.
CREATE TABLE service_events (
    id         INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id),
    seq        INTEGER NOT NULL,          -- monotonic within a service
    at_ms      REAL NOT NULL,             -- ms since service start
    kind       TEXT NOT NULL,             -- see db::EventKind
    detail     TEXT                       -- short human phrase; never verse text
);
CREATE INDEX idx_service_events ON service_events(service_id, seq);

-- Latency, kept past the end of the app.
--
-- `latency.rs` measures nine stamps per decode pass and holds it all in memory,
-- so the evidence a church would send back died the moment they closed Relay —
-- and the run that matters most is the one that ended badly. A snapshot is taken
-- once a minute while a service records, and once more when it ends.
--
-- Percentiles, not raw traces: a trace carries what was heard, and this table is
-- the one thing here that might travel. Numbers only.
CREATE TABLE perf_samples (
    id         INTEGER PRIMARY KEY,
    service_id INTEGER NOT NULL REFERENCES services(id),
    at_ms      REAL NOT NULL,             -- ms since service start
    metric     TEXT NOT NULL,             -- latency::Metric wire name
    samples    INTEGER NOT NULL,
    p50_ms     REAL,                      -- null = the stage was never reached
    p95_ms     REAL,
    -- One window in a hundred: roughly one visibly late verse per service, which is
    -- what a congregation notices and a median cannot show.
    p99_ms     REAL,
    worst_ms   REAL,
    -- The mean of the most recent complete MINUTE. Everything else in this row is
    -- cumulative since app start, and a cumulative percentile cannot answer "did it
    -- get worse" — which is the only question Stage F11 asks. See FIELD F-3.
    last_minute_ms REAL
);
CREATE INDEX idx_perf_samples ON perf_samples(service_id, at_ms);

-- ===== A room, remembered (db/environments.rs) =====
--
-- A church that runs in the main hall on Sunday and the youth room on Wednesday
-- rebuilds the same configuration twice a week. This holds it: the microphone, the
-- recognition language, the planned length, the active voice profile, and which
-- display each screen goes to.
--
-- NOT the audio thresholds. DECISIONS §19 / CLAUDE.md rule 12: nothing may compare
-- a signal to a stored level. A noise floor captured three weeks ago, applied to
-- the same hall today with the heating on and forty more people in it, is exactly
-- the assumption that rule forbids. Observed levels live in `notes`, for a person
-- to read, and nothing reads them back.
CREATE TABLE environment_profiles (
    id            INTEGER PRIMARY KEY,
    name          TEXT NOT NULL,
    is_active     INTEGER NOT NULL DEFAULT 0,
    settings_json TEXT NOT NULL DEFAULT '{}',   -- the remembered choices
    notes         TEXT NOT NULL DEFAULT '',     -- what Relay observed, in words
    updated_at    TEXT NOT NULL DEFAULT ''
);

-- ===== App settings (db/settings.rs) =====
-- Small operator preferences (active translation, per-content-type default
-- templates, …). Local-first key/value.
CREATE TABLE app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- ===== Accent & speaker calibration (db/profiles.rs) =====
-- One row per preacher: STT language hint, decoder-bias vocabulary, the
-- sensitivity dial, and the self-calibrated confidence thresholds, so accent /
-- threshold learning persists per speaker across services and restarts.
--
-- DEFAULTS BELOW MATCH THE CODE: sensitivity 50 → auto_fire 0.50 / suggest 0.35,
-- because Thresholds::default() is DEFINED as from_sensitivity(50) (router.rs).
-- There is exactly ONE baseline, by construction (DECISIONS; CLAUDE).
CREATE TABLE voice_profiles (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    language    TEXT,                                  -- null = auto-detect / code-switch; else "en"/"yo"/"sw"/"ha"
    sensitivity INTEGER NOT NULL DEFAULT 50,           -- 0..100 dial → threshold baseline
    auto_fire   REAL NOT NULL DEFAULT 0.50,            -- live, feedback-adapted
    suggest     REAL NOT NULL DEFAULT 0.35,
    bias_terms  TEXT NOT NULL DEFAULT '',              -- extra decoder-bias vocab (church name, phrases)
    is_active   INTEGER NOT NULL DEFAULT 0             -- exactly one row is active at a time
);

-- ===== Demo content, and the ledger that lets it be taken back (db/demo.rs) =====
--
-- A church evaluating Relay has an empty database and nothing to press. This is
-- the sample service they can load, and — the part that matters — remove again.
--
-- **NOTHING LOADS THIS.** Not `init_fresh`, not a migration, not an empty
-- database. It is written only by the `load_demo_content` command, which an
-- operator presses in Settings → History & Backup. A fresh install has this table
-- and no rows in it, which is what `the_bare_fixture_is_a_first_launch_and_nothing_more`
-- and `a_fresh_install_carries_no_demo_content` both hold.
--
-- The ledger is the MARKER. A name prefix is not: the demo plan is called
-- "Demo · Sunday Morning Service" so an operator can tell it from their own at a
-- glance, but they may rename it, and a removal keyed on a title would then
-- either miss it or take one of theirs. `(table_name, row_id)` cannot be renamed.
--
-- `fingerprint` is the row's content as it was SEEDED (a sha-256 over the fields
-- an operator can edit, cues and sections included). On removal a row whose
-- fingerprint still matches is deleted; one that has changed is KEPT and released
-- from the ledger, because it now contains work somebody did and this product has
-- no undo. The removal report says how many of each.
CREATE TABLE demo_content (
    id          INTEGER PRIMARY KEY,
    table_name  TEXT NOT NULL,           -- 'service_plans' | 'songs' | 'announcements'
                                         -- | 'saved_scripture' | 'media_assets'
    row_id      INTEGER NOT NULL,
    fingerprint TEXT NOT NULL,           -- sha-256 of the row as seeded
    loaded_at   TEXT NOT NULL DEFAULT '',
    UNIQUE (table_name, row_id)
);
