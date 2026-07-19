-- Relay — canonical SQLite schema
--
-- This reflects the CURRENT on-device shape a fresh install creates, transcribed
-- from the migration/ensure code in `src-tauri/src/db/` (mod.rs + one file per
-- table-family). It supersedes the old v0.1 draft, which had drifted (it was
-- missing service_plans, plan_items, songs, arrangements, the library tables and
-- the FTS5 index).
--
-- SOURCE OF TRUTH IS THE CODE, NOT THIS FILE. Regenerate whenever a migration in
-- `db/mod.rs` (or a `db/*.rs` `ensure_*`) changes. To dump the live schema:
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
    embedding      BLOB                   -- precomputed vector for semantic match; NEVER YET WRITTEN (see docs/ROADMAP.md)
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
    console_active     INTEGER NOT NULL DEFAULT 0  -- one of the (max 4) styles on the console output grid
);

CREATE TABLE output_channels (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL,
    render_target  TEXT NOT NULL CHECK (render_target IN ('native_window', 'ndi_encode', 'network_client')),
    template_id    INTEGER REFERENCES templates(id),
    display_target TEXT,                  -- display index, NDI source name, or kiosk client id
    status         TEXT NOT NULL DEFAULT 'offline' CHECK (status IN ('online', 'offline'))
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
-- is read; template_id is an optional per-content-type override. See docs/DOMAIN_MODEL.md §4.
CREATE TABLE plan_items (
    id           INTEGER PRIMARY KEY,
    plan_id      INTEGER NOT NULL REFERENCES service_plans(id) ON DELETE CASCADE,
    position     INTEGER NOT NULL,
    cue_type     TEXT NOT NULL,           -- 'scripture' | 'song' | 'media' | 'announcement' | 'countdown'
    label        TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{}',
    template_id  INTEGER
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
    sequence TEXT NOT NULL DEFAULT '[]'   -- JSON array of section indices
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
    fired_at      REAL                    -- seconds since service start, null if never fired
);

-- Operator-action log for a running service (distinct from a plan_items cue).
CREATE TABLE cues (
    id           INTEGER PRIMARY KEY,
    service_id   INTEGER NOT NULL REFERENCES services(id),
    type         TEXT NOT NULL,           -- e.g. "manual_override", "clear_screens", "template_change"
    payload_json TEXT,
    triggered_at REAL NOT NULL
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
