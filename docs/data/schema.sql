-- Relay — SQLite schema (v0.1 draft)
-- Local-first. Every table lives on-device by default.

PRAGMA foreign_keys = ON;

-- ===== Reference data =====

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
    embedding      BLOB                   -- precomputed vector for semantic match, nullable until indexed
);
CREATE INDEX idx_verses_lookup ON verses(translation_id, book, chapter, verse);

-- ===== Presentation =====

CREATE TABLE templates (
    id                 INTEGER PRIMARY KEY,
    name               TEXT NOT NULL,
    region_config_json TEXT NOT NULL,     -- layout regions, see docs/SPEC.md §5
    style_json         TEXT NOT NULL      -- fonts, colors, transitions
);

CREATE TABLE output_channels (
    id             INTEGER PRIMARY KEY,
    name           TEXT NOT NULL,
    render_target  TEXT NOT NULL CHECK (render_target IN ('native_window', 'ndi_encode', 'network_client')),
    template_id    INTEGER REFERENCES templates(id),
    display_target TEXT,                  -- display index, NDI source name, or kiosk client id
    status         TEXT NOT NULL DEFAULT 'offline' CHECK (status IN ('online', 'offline'))
);

-- ===== Live session data =====

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
    language   TEXT NOT NULL,             -- detected language for this chunk
    confidence REAL                       -- STT confidence, 0-1
);

CREATE TABLE detections (
    id            INTEGER PRIMARY KEY,
    transcript_id INTEGER NOT NULL REFERENCES transcripts(id),
    verse_id      INTEGER REFERENCES verses(id),
    method        TEXT NOT NULL CHECK (method IN ('direct', 'semantic')),
    confidence    REAL NOT NULL,
    status        TEXT NOT NULL CHECK (status IN ('auto', 'suggested', 'dismissed')),
    fired_at      REAL                    -- seconds since service start, null if never fired
);

CREATE TABLE cues (
    id           INTEGER PRIMARY KEY,
    service_id   INTEGER NOT NULL REFERENCES services(id),
    type         TEXT NOT NULL,           -- e.g. "manual_override", "clear_screens", "template_change"
    payload_json TEXT,
    triggered_at REAL NOT NULL
);

-- ===== Accent & speaker calibration (Phase B) =====
-- One row per preacher. Bundles the STT language hint, decoder-bias vocabulary,
-- the sensitivity dial, and the self-calibrated confidence thresholds so that
-- accent/threshold learning persists per speaker across services and restarts.
CREATE TABLE voice_profiles (
    id          INTEGER PRIMARY KEY,
    name        TEXT NOT NULL,
    language    TEXT,                                  -- null = auto-detect / code-switch; else "en"/"yo"/"sw"/"ha"
    sensitivity INTEGER NOT NULL DEFAULT 50,           -- 0..100 dial → threshold baseline
    auto_fire   REAL NOT NULL DEFAULT 0.90,            -- live, feedback-adapted
    suggest     REAL NOT NULL DEFAULT 0.60,
    bias_terms  TEXT NOT NULL DEFAULT '',              -- extra decoder-bias vocab (church name, phrases)
    is_active   INTEGER NOT NULL DEFAULT 0             -- exactly one row is active at a time
);
