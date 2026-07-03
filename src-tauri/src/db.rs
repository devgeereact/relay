//! SQLite access layer.
//!
//! Single responsibility: local-first persistence against the schema in
//! docs/data/schema.sql. Nothing else in this codebase should touch SQLite
//! directly — go through this module. See PROMPT.md Phase 2.
//!
//! Offline-first: the schema is compiled in via `include_str!`, so there is no
//! runtime dependency on the docs/ file being shipped alongside the binary.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// The canonical schema, baked into the binary at compile time.
const SCHEMA: &str = include_str!("../../docs/data/schema.sql");

/// A single verse row, shaped for the frontend (serialized across the Tauri
/// bridge). `reference` is the human-facing citation, e.g. "John 3:16".
#[derive(Debug, Clone, Serialize)]
pub struct VerseRow {
    pub id: i64,
    pub book: String,
    pub chapter: i64,
    pub verse: i64,
    pub text: String,
    pub reference: String,
    pub translation: String,
}

/// An output template: layout (regions + alignment) and style (fonts, colors,
/// sizes). `layout` and `style` are opaque JSON blobs interpreted by the shared
/// renderer (Output.svelte) — the DB doesn't care about their internals, which
/// keeps the template shape editable without a migration. See docs/SPEC.md §5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub layout: Value,
    pub style: Value,
}

/// All templates, ordered by id.
pub fn list_templates(conn: &Connection) -> rusqlite::Result<Vec<Template>> {
    let mut stmt =
        conn.prepare("SELECT id, name, region_config_json, style_json FROM templates ORDER BY id")?;
    let rows = stmt.query_map([], row_to_template)?;
    rows.collect()
}

/// A single template by id.
pub fn get_template(conn: &Connection, id: i64) -> rusqlite::Result<Option<Template>> {
    conn.query_row(
        "SELECT id, name, region_config_json, style_json FROM templates WHERE id = ?1",
        [id],
        row_to_template,
    )
    .optional()
}

/// Insert (id <= 0) or update (id > 0) a template. Returns its id.
pub fn upsert_template(conn: &Connection, t: &Template) -> rusqlite::Result<i64> {
    let layout = t.layout.to_string();
    let style = t.style.to_string();
    if t.id > 0 {
        conn.execute(
            "UPDATE templates SET name = ?1, region_config_json = ?2, style_json = ?3 WHERE id = ?4",
            (&t.name, &layout, &style, t.id),
        )?;
        Ok(t.id)
    } else {
        conn.execute(
            "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
            (&t.name, &layout, &style),
        )?;
        Ok(conn.last_insert_rowid())
    }
}

/// A configured output channel (name + render target + assigned template).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputChannel {
    pub id: i64,
    pub name: String,
    pub render_target: String, // native_window | ndi_encode | network_client
    pub template_id: Option<i64>,
    pub display_target: Option<String>,
    pub status: String,
}

/// All configured output channels.
pub fn list_output_channels(conn: &Connection) -> rusqlite::Result<Vec<OutputChannel>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, render_target, template_id, display_target, status
           FROM output_channels ORDER BY id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(OutputChannel {
            id: r.get(0)?,
            name: r.get(1)?,
            render_target: r.get(2)?,
            template_id: r.get(3)?,
            display_target: r.get(4)?,
            status: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// Assign a template to a channel (the "make outputs assignable" control).
pub fn set_channel_template(conn: &Connection, id: i64, template_id: i64) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE output_channels SET template_id = ?1 WHERE id = ?2",
        (template_id, id),
    )?;
    Ok(())
}

/// Assign a physical display to a channel (HDMI screen assignment). `display` is
/// the monitor index as a string, or None to clear (use the primary display).
pub fn set_channel_display(
    conn: &Connection,
    id: i64,
    display: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE output_channels SET display_target = ?1 WHERE id = ?2",
        (display, id),
    )?;
    Ok(())
}

/// Add a new output channel; returns its id. `render_target` must be one of
/// native_window / ndi_encode / network_client (enforced by the schema CHECK).
pub fn add_channel(
    conn: &Connection,
    name: &str,
    render_target: &str,
    template_id: i64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO output_channels (name, render_target, template_id, status)
         VALUES (?1, ?2, ?3, 'offline')",
        (name, render_target, template_id),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Delete an output channel.
pub fn delete_channel(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM output_channels WHERE id = ?1", [id])?;
    Ok(())
}

// ===== Voice profiles (Phase B — accent & speaker calibration) ==============

/// A per-preacher calibration profile. Bundles the STT language hint, decoder-
/// bias vocabulary, the sensitivity dial, and the live feedback-adapted
/// thresholds so accent + threshold learning persists per speaker across
/// services and restarts. See docs/SPEC.md §4.6 and router.rs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceProfile {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    /// None = auto-detect / code-switch; else an ISO code ("en"/"yo"/"sw"/"ha").
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: i64,
    #[serde(default = "default_auto_fire")]
    pub auto_fire: f64,
    #[serde(default = "default_suggest")]
    pub suggest: f64,
    #[serde(default)]
    pub bias_terms: String,
    #[serde(default)]
    pub is_active: bool,
}

fn default_sensitivity() -> i64 {
    50
}
fn default_auto_fire() -> f64 {
    0.90
}
fn default_suggest() -> f64 {
    0.60
}

const PROFILE_COLS: &str =
    "id, name, language, sensitivity, auto_fire, suggest, bias_terms, is_active";

fn row_to_profile(r: &rusqlite::Row) -> rusqlite::Result<VoiceProfile> {
    Ok(VoiceProfile {
        id: r.get(0)?,
        name: r.get(1)?,
        language: r.get(2)?,
        sensitivity: r.get(3)?,
        auto_fire: r.get(4)?,
        suggest: r.get(5)?,
        bias_terms: r.get(6)?,
        is_active: r.get::<_, i64>(7)? != 0,
    })
}

/// Create the `voice_profiles` table if missing and guarantee exactly one active
/// profile (seeding a "Default" if the table is empty). Idempotent — safe to run
/// on every open, including DBs created before Phase B existed.
pub fn ensure_voice_profiles(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS voice_profiles (
            id          INTEGER PRIMARY KEY,
            name        TEXT NOT NULL,
            language    TEXT,
            sensitivity INTEGER NOT NULL DEFAULT 50,
            auto_fire   REAL NOT NULL DEFAULT 0.90,
            suggest     REAL NOT NULL DEFAULT 0.60,
            bias_terms  TEXT NOT NULL DEFAULT '',
            is_active   INTEGER NOT NULL DEFAULT 0
        );",
    )?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM voice_profiles", [], |r| r.get(0))?;
    if n == 0 {
        conn.execute(
            "INSERT INTO voice_profiles (name, language, is_active) VALUES ('Default', NULL, 1)",
            [],
        )?;
    } else {
        ensure_one_active(conn)?;
    }
    Ok(())
}

/// Guarantee a single active profile: if none is active, promote the lowest id.
fn ensure_one_active(conn: &Connection) -> rusqlite::Result<()> {
    let active: i64 = conn.query_row(
        "SELECT COUNT(*) FROM voice_profiles WHERE is_active = 1",
        [],
        |r| r.get(0),
    )?;
    if active == 0 {
        conn.execute(
            "UPDATE voice_profiles SET is_active = 1
               WHERE id = (SELECT MIN(id) FROM voice_profiles)",
            [],
        )?;
    }
    Ok(())
}

/// All voice profiles, ordered by id.
pub fn list_voice_profiles(conn: &Connection) -> rusqlite::Result<Vec<VoiceProfile>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {PROFILE_COLS} FROM voice_profiles ORDER BY id"
    ))?;
    let rows = stmt.query_map([], row_to_profile)?;
    rows.collect()
}

/// The currently active profile, if any.
pub fn active_voice_profile(conn: &Connection) -> rusqlite::Result<Option<VoiceProfile>> {
    conn.query_row(
        &format!("SELECT {PROFILE_COLS} FROM voice_profiles WHERE is_active = 1 LIMIT 1"),
        [],
        row_to_profile,
    )
    .optional()
}

/// Create a new profile (with default calibration) and return its id.
pub fn create_voice_profile(
    conn: &Connection,
    name: &str,
    language: Option<&str>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO voice_profiles (name, language) VALUES (?1, ?2)",
        (name, language),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Persist editable fields of a profile (name, language, sensitivity, bias
/// terms). Thresholds are saved separately via `save_profile_thresholds` because
/// they are machine-adapted, not user-edited.
pub fn update_voice_profile(conn: &Connection, p: &VoiceProfile) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE voice_profiles
            SET name = ?1, language = ?2, sensitivity = ?3, bias_terms = ?4
          WHERE id = ?5",
        (&p.name, &p.language, p.sensitivity, &p.bias_terms, p.id),
    )?;
    Ok(())
}

/// Persist the live, feedback-adapted thresholds for a profile (the
/// self-calibrating loop — router.rs `record_feedback`).
pub fn save_profile_thresholds(
    conn: &Connection,
    id: i64,
    auto_fire: f64,
    suggest: f64,
) -> rusqlite::Result<()> {
    conn.execute(
        "UPDATE voice_profiles SET auto_fire = ?1, suggest = ?2 WHERE id = ?3",
        (auto_fire, suggest, id),
    )?;
    Ok(())
}

/// Make `id` the sole active profile.
pub fn set_active_profile(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("UPDATE voice_profiles SET is_active = 0", [])?;
    conn.execute(
        "UPDATE voice_profiles SET is_active = 1 WHERE id = ?1",
        [id],
    )?;
    ensure_one_active(conn)?;
    Ok(())
}

/// Delete a profile, then guarantee a profile still exists and one is active
/// (re-seeds a Default if the last one was removed).
pub fn delete_voice_profile(conn: &Connection, id: i64) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM voice_profiles WHERE id = ?1", [id])?;
    ensure_voice_profiles(conn)?;
    Ok(())
}

/// Seed the default output channels (idempotent — only when empty). Template ids
/// 1..4 match the seeded templates.
fn seed_channels(conn: &Connection) -> rusqlite::Result<()> {
    let channels: &[(&str, &str, i64, Option<&str>)] = &[
        ("Main screen", "native_window", 1, Some("Display 1")),
        ("Stage display", "network_client", 2, None),
        ("Streaming", "network_client", 3, None),
        ("Lobby screen", "network_client", 4, None),
    ];
    let mut stmt = conn.prepare(
        "INSERT INTO output_channels (name, render_target, template_id, display_target, status)
         VALUES (?1, ?2, ?3, ?4, 'offline')",
    )?;
    for (name, target, tmpl, disp) in channels {
        stmt.execute((name, target, tmpl, disp))?;
    }
    Ok(())
}

fn row_to_template(r: &rusqlite::Row) -> rusqlite::Result<Template> {
    let layout: String = r.get(2)?;
    let style: String = r.get(3)?;
    Ok(Template {
        id: r.get(0)?,
        name: r.get(1)?,
        layout: serde_json::from_str(&layout).unwrap_or(Value::Null),
        style: serde_json::from_str(&style).unwrap_or(Value::Null),
    })
}

/// Open (or create) the on-device database at the default per-OS data path,
/// applying the schema and dev seed on first creation.
///
/// Called once at startup (not on a live-service path), so surfacing a hard
/// error here is correct — a broken DB must fail loudly before a service, not
/// silently mid-sermon.
pub fn open() -> rusqlite::Result<Connection> {
    let path = default_db_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let fresh = !path.exists();
    let conn = Connection::open(&path)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    if fresh {
        init_fresh(&conn)?;
    } else {
        // Forward-fill for DBs created before templates were seeded (Phase 8).
        // Idempotent: only seeds when the table is empty.
        let n: i64 = conn.query_row("SELECT COUNT(*) FROM templates", [], |r| r.get(0))?;
        if n == 0 {
            seed_templates(&conn)?;
        } else {
            // One-time migration: the old seed stored sizes as "4.6vw"; the new
            // renderer uses cqw numbers. If any template still has the old "vw"
            // format (i.e. untouched defaults), reset the built-ins.
            let old: i64 = conn.query_row(
                "SELECT COUNT(*) FROM templates WHERE style_json LIKE '%vw%'",
                [],
                |r| r.get(0),
            )?;
            if old > 0 {
                reset_builtin_templates(&conn)?;
            }
        }
        // Forward-fill default output channels for pre-existing DBs.
        let cn: i64 = conn.query_row("SELECT COUNT(*) FROM output_channels", [], |r| r.get(0))?;
        if cn == 0 {
            seed_channels(&conn)?;
        }
        // Forward-fill the full Bible for DBs created with the old 15-verse seed.
        if verse_count(&conn)? < 30_000 {
            reimport_full_kjv(&conn)?;
        }
    }
    // Phase B: voice-profiles table + a guaranteed active profile. Idempotent,
    // and covers DBs created before this table existed.
    ensure_voice_profiles(&conn)?;
    Ok(conn)
}

/// Apply the full schema and seed a fresh connection. Public so tests (and any
/// future in-memory scratch DB) can build a ready-to-query database directly.
pub fn init_fresh(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    seed(conn)?;
    // Guarantee an active voice profile exists even on a bare in-memory DB.
    ensure_voice_profiles(conn)?;
    Ok(())
}

/// Resolve the default database file path per OS, honoring a RELAY_DB_PATH
/// override (handy for tests and dev). Kept dependency-free deliberately —
/// standard app-data locations, no `dirs` crate needed.
fn default_db_path() -> PathBuf {
    if let Ok(p) = std::env::var("RELAY_DB_PATH") {
        return PathBuf::from(p);
    }
    let base = if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|_| PathBuf::from("."))
    };
    base.join("com.relay.app").join("relay.db")
}

/// Look up a single verse by canonical reference. Returns None if absent.
pub fn lookup_verse(
    conn: &Connection,
    book: &str,
    chapter: i64,
    verse: i64,
) -> rusqlite::Result<Option<VerseRow>> {
    conn.query_row(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          WHERE v.book = ?1 AND v.chapter = ?2 AND v.verse = ?3
          LIMIT 1",
        (book, chapter, verse),
        row_to_verse,
    )
    .optional()
}

/// The highest verse number in a chapter — the end of a whole-chapter passage
/// walk (Phase A). None when the book/chapter isn't in the corpus.
pub fn chapter_last_verse(
    conn: &Connection,
    book: &str,
    chapter: i64,
) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT MAX(verse) FROM verses WHERE book = ?1 AND chapter = ?2",
        (book, chapter),
        |r| r.get::<_, Option<i64>>(0),
    )
}

/// Total verses currently seeded — a cheap health check for the data layer.
pub fn verse_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row("SELECT COUNT(*) FROM verses", [], |r| r.get(0))
}

// ===== Service-session persistence =====
//
// Local-first service history: transcripts, fired detections, and operator cues
// (manual overrides, clear-screens) are written to the current service and
// surfaced in the Library tab. Nothing leaves the device (CLAUDE.md).

/// A row for the Library service list. `duration_secs` is derived from the last
/// transcript timestamp; `verses` counts fired detections; `overrides` counts
/// manual-override cues.
#[derive(Debug, Clone, Serialize)]
pub struct ServiceSummary {
    pub id: i64,
    pub date: String,
    pub title: String,
    pub duration_secs: f64,
    pub verses: i64,
    pub overrides: i64,
}

/// A transcript line in a service detail view.
#[derive(Debug, Clone, Serialize)]
pub struct TranscriptRow {
    pub timestamp: f64,
    pub text: String,
    pub language: String,
}

/// A fired detection in a service detail view (verse ref resolved if known).
#[derive(Debug, Clone, Serialize)]
pub struct ServiceDetection {
    pub reference: Option<String>,
    pub method: String,
    pub confidence: f32,
    pub status: String,
    pub fired_at: f64,
}

/// Create a service and return its id.
pub fn create_service(conn: &Connection, date: &str, title: &str) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO services (date, title) VALUES (?1, ?2)",
        (date, title),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert a transcript line; returns its id.
pub fn insert_transcript(
    conn: &Connection,
    service_id: i64,
    timestamp: f64,
    text: &str,
    language: &str,
    confidence: Option<f32>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO transcripts (service_id, timestamp, text, language, confidence)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (service_id, timestamp, text, language, confidence),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert a fired detection linked to a transcript.
pub fn insert_detection(
    conn: &Connection,
    transcript_id: i64,
    verse_id: Option<i64>,
    method: &str,
    confidence: f32,
    status: &str,
    fired_at: Option<f64>,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO detections (transcript_id, verse_id, method, confidence, status, fired_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (
            transcript_id,
            verse_id,
            method,
            confidence,
            status,
            fired_at,
        ),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Insert an operator cue (e.g. "manual_override", "clear_screens").
pub fn insert_cue(
    conn: &Connection,
    service_id: i64,
    cue_type: &str,
    payload_json: Option<&str>,
    triggered_at: f64,
) -> rusqlite::Result<i64> {
    conn.execute(
        "INSERT INTO cues (service_id, type, payload_json, triggered_at) VALUES (?1, ?2, ?3, ?4)",
        (service_id, cue_type, payload_json, triggered_at),
    )?;
    Ok(conn.last_insert_rowid())
}

/// All services, newest first, with derived Library counts.
pub fn list_services(conn: &Connection) -> rusqlite::Result<Vec<ServiceSummary>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.date, s.title,
                COALESCE((SELECT MAX(timestamp) FROM transcripts WHERE service_id = s.id), 0.0),
                (SELECT COUNT(*) FROM detections d
                   JOIN transcripts t ON t.id = d.transcript_id
                  WHERE t.service_id = s.id),
                (SELECT COUNT(*) FROM cues c
                  WHERE c.service_id = s.id AND c.type = 'manual_override')
           FROM services s
          ORDER BY s.id DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(ServiceSummary {
            id: r.get(0)?,
            date: r.get(1)?,
            title: r.get(2)?,
            duration_secs: r.get(3)?,
            verses: r.get(4)?,
            overrides: r.get(5)?,
        })
    })?;
    rows.collect()
}

/// Transcript lines for a service, in order.
pub fn service_transcripts(
    conn: &Connection,
    service_id: i64,
) -> rusqlite::Result<Vec<TranscriptRow>> {
    let mut stmt = conn.prepare(
        "SELECT timestamp, text, language FROM transcripts
          WHERE service_id = ?1 ORDER BY timestamp",
    )?;
    let rows = stmt.query_map([service_id], |r| {
        Ok(TranscriptRow {
            timestamp: r.get(0)?,
            text: r.get(1)?,
            language: r.get(2)?,
        })
    })?;
    rows.collect()
}

/// Fired detections for a service, in order.
pub fn service_detections(
    conn: &Connection,
    service_id: i64,
) -> rusqlite::Result<Vec<ServiceDetection>> {
    let mut stmt = conn.prepare(
        "SELECT v.book, v.chapter, v.verse, d.method, d.confidence, d.status, d.fired_at
           FROM detections d
           JOIN transcripts t ON t.id = d.transcript_id
           LEFT JOIN verses v ON v.id = d.verse_id
          WHERE t.service_id = ?1
          ORDER BY d.fired_at",
    )?;
    let rows = stmt.query_map([service_id], |r| {
        let book: Option<String> = r.get(0)?;
        let chapter: Option<i64> = r.get(1)?;
        let verse: Option<i64> = r.get(2)?;
        let reference = match (book, chapter, verse) {
            (Some(b), Some(c), Some(v)) => Some(format!("{b} {c}:{v}")),
            _ => None,
        };
        Ok(ServiceDetection {
            reference,
            method: r.get(3)?,
            confidence: r.get(4)?,
            status: r.get(5)?,
            fired_at: r.get::<_, Option<f64>>(6)?.unwrap_or(0.0),
        })
    })?;
    rows.collect()
}

/// How many times a verse has already fired in a service (Phase A6 — the
/// series/repeat tracker). Counts only detections that actually fired.
pub fn count_verse_in_service(
    conn: &Connection,
    service_id: i64,
    verse_id: i64,
) -> rusqlite::Result<i64> {
    conn.query_row(
        "SELECT COUNT(*)
           FROM detections d
           JOIN transcripts t ON t.id = d.transcript_id
          WHERE t.service_id = ?1 AND d.verse_id = ?2 AND d.fired_at IS NOT NULL",
        (service_id, verse_id),
        |r| r.get(0),
    )
}

/// Every verse, for building the semantic index (Phase 9).
pub fn all_verses(conn: &Connection) -> rusqlite::Result<Vec<VerseRow>> {
    let mut stmt = conn.prepare(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          ORDER BY v.id",
    )?;
    let rows = stmt.query_map([], row_to_verse)?;
    rows.collect()
}

fn row_to_verse(r: &rusqlite::Row) -> rusqlite::Result<VerseRow> {
    let book: String = r.get(1)?;
    let chapter: i64 = r.get(2)?;
    let verse: i64 = r.get(3)?;
    Ok(VerseRow {
        id: r.get(0)?,
        reference: format!("{book} {chapter}:{verse}"),
        book,
        chapter,
        verse,
        text: r.get(4)?,
        translation: r.get(5)?,
    })
}

/// The full public-domain KJV, bundled at compile time (offline-first — no
/// runtime file dependency). Structure: array of books in canonical order, each
/// `{ "chapters": [[verse, …], …] }`. Book names come from CANONICAL_BOOKS by
/// index, so a stored verse and a detected reference always agree on spelling.
const KJV_JSON: &str = include_str!("../data/kjv.json");

#[derive(serde::Deserialize)]
struct KjvBook {
    chapters: Vec<Vec<String>>,
}

/// Seed a fresh database: one KJV translation + the full Bible + templates.
fn seed(conn: &Connection) -> rusqlite::Result<()> {
    let translation_id = kjv_translation_id(conn)?;
    import_full_kjv(conn, translation_id)?;
    seed_templates(conn)?;
    seed_channels(conn)?;
    Ok(())
}

/// The KJV translation id, creating the row if absent.
fn kjv_translation_id(conn: &Connection) -> rusqlite::Result<i64> {
    if let Some(id) = conn
        .query_row(
            "SELECT id FROM translations WHERE abbreviation = 'KJV'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO translations (name, abbreviation, language, license_type)
         VALUES (?1, ?2, ?3, ?4)",
        ("King James Version", "KJV", "en", "public domain"),
    )?;
    Ok(conn.last_insert_rowid())
}

/// Parse the bundled KJV and bulk-insert every verse (one transaction — 31k
/// rows). Strips the `{…}` italic markers KJV uses for supplied words. Returns
/// the verse count inserted.
fn import_full_kjv(conn: &Connection, translation_id: i64) -> rusqlite::Result<usize> {
    let raw = KJV_JSON.trim_start_matches('\u{feff}'); // strip UTF-8 BOM
    let books: Vec<KjvBook> = serde_json::from_str(raw)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

    let tx = conn.unchecked_transaction()?;
    let mut count = 0usize;
    {
        let mut stmt = tx.prepare(
            "INSERT INTO verses (translation_id, book, chapter, verse, text)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (bi, book) in books.iter().enumerate() {
            let name = crate::detection::CANONICAL_BOOKS
                .get(bi)
                .copied()
                .unwrap_or("Unknown");
            for (ci, chapter) in book.chapters.iter().enumerate() {
                for (vi, text) in chapter.iter().enumerate() {
                    stmt.execute((
                        translation_id,
                        name,
                        ci as i64 + 1,
                        vi as i64 + 1,
                        strip_italics(text),
                    ))?;
                    count += 1;
                }
            }
        }
    }
    tx.commit()?;
    Ok(count)
}

/// Remove KJV supplied-word markers `{ }`, keeping the words themselves.
fn strip_italics(text: &str) -> String {
    text.chars().filter(|c| *c != '{' && *c != '}').collect()
}

/// Forward-fill the full corpus for DBs created before the full-Bible import
/// (they hold only the old 15-verse dev seed). FK-safe: nulls any detection
/// verse links first, then replaces the verses.
fn reimport_full_kjv(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute("UPDATE detections SET verse_id = NULL", [])?;
    conn.execute("DELETE FROM verses", [])?;
    let tid = kjv_translation_id(conn)?;
    import_full_kjv(conn, tid)?;
    Ok(())
}

/// The four built-in output templates (SPEC §5, cqw sizes). Match the frontend
/// defaults in src/lib/templates.js. Source of truth for both fresh seed and
/// the in-place migration.
fn builtin_templates() -> &'static [(&'static str, &'static str, &'static str)] {
    &[
        (
            "Classic Serif",
            r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"#e8a33d","verseColor":"#f4e4c8","verseSize":"5.5","refSize":"2.6","italicRef":true}"##,
        ),
        (
            "Stage Mono",
            r##"{"regions":["reference","verse_text"],"align":"left","lowerThird":false,"refFirst":true}"##,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"#4fa8c9","verseColor":"#ffffff","verseSize":"6","refSize":"2.6","italicRef":false}"##,
        ),
        (
            "Lower Third",
            r##"{"regions":["verse_text","reference"],"align":"left","lowerThird":true,"refFirst":false}"##,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"#b080e0","verseColor":"#1c1224","verseSize":"2.6","refSize":"1.7","italicRef":false}"##,
        ),
        (
            "Lobby Warm",
            r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(160deg, #241419, #120a0e)","accent":"#e27d93","verseColor":"#f0dfe3","verseSize":"4","refSize":"2","italicRef":false}"##,
        ),
    ]
}

/// Seed the built-in templates into a fresh DB (ids 1..4).
fn seed_templates(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
    )?;
    for (name, layout, style) in builtin_templates() {
        stmt.execute((name, layout, style))?;
    }
    Ok(())
}

/// Reset the built-in templates IN PLACE (ids 1..4) — keeps ids stable so
/// output_channels FKs stay valid. Used by the vw→cqw migration.
fn reset_builtin_templates(conn: &Connection) -> rusqlite::Result<()> {
    for (i, (name, layout, style)) in builtin_templates().iter().enumerate() {
        conn.execute(
            "UPDATE templates SET name = ?1, region_config_json = ?2, style_json = ?3 WHERE id = ?4",
            (name, layout, style, i as i64 + 1),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        init_fresh(&conn).unwrap();
        conn
    }

    #[test]
    fn seeds_full_kjv() {
        let conn = fresh_db();
        // Full KJV is 31,102 verses; the bundled file has 31,100.
        assert!(verse_count(&conn).unwrap() > 31_000);
    }

    #[test]
    fn seeds_four_templates() {
        let conn = fresh_db();
        let ts = list_templates(&conn).unwrap();
        assert_eq!(ts.len(), 4);
        assert_eq!(ts[0].name, "Classic Serif");
        assert_eq!(ts[0].style["font"], "var(--f-serif)");
        assert_eq!(ts[0].layout["align"], "center");
    }

    #[test]
    fn upsert_updates_existing_template() {
        let conn = fresh_db();
        let mut t = get_template(&conn, 1).unwrap().unwrap();
        t.name = "Classic Serif (edited)".into();
        t.style["accent"] = serde_json::json!("#ffffff");
        let id = upsert_template(&conn, &t).unwrap();
        assert_eq!(id, 1);
        let reloaded = get_template(&conn, 1).unwrap().unwrap();
        assert_eq!(reloaded.name, "Classic Serif (edited)");
        assert_eq!(reloaded.style["accent"], "#ffffff");
    }

    #[test]
    fn upsert_inserts_new_template() {
        let conn = fresh_db();
        let t = Template {
            id: 0,
            name: "Custom".into(),
            layout: serde_json::json!({ "regions": ["verse_text"] }),
            style: serde_json::json!({ "font": "var(--f-body)" }),
        };
        let id = upsert_template(&conn, &t).unwrap();
        assert_eq!(id, 5);
        assert_eq!(list_templates(&conn).unwrap().len(), 5);
    }

    #[test]
    fn looks_up_john_3_16_verbatim() {
        let conn = fresh_db();
        let v = lookup_verse(&conn, "John", 3, 16).unwrap().unwrap();
        assert_eq!(v.reference, "John 3:16");
        assert_eq!(v.translation, "KJV");
        assert!(v.text.starts_with("For God so loved the world"));
    }

    #[test]
    fn psalm_23_is_complete_for_context_memory() {
        // Six consecutive verses — the fixture context-memory logic (Phase 9)
        // resolves a bare "verse 4" against.
        let conn = fresh_db();
        for verse in 1..=6 {
            assert!(
                lookup_verse(&conn, "Psalms", 23, verse).unwrap().is_some(),
                "Psalms 23:{verse} should be seeded"
            );
        }
    }

    #[test]
    fn missing_verse_returns_none() {
        let conn = fresh_db();
        // Genesis has 50 chapters — 999 is safely out of range.
        assert!(lookup_verse(&conn, "Genesis", 999, 1).unwrap().is_none());
    }

    #[test]
    fn service_persistence_and_library_counts() {
        let conn = fresh_db();
        let sid = create_service(&conn, "2026-07-03", "Sunday Service").unwrap();
        let t1 = insert_transcript(
            &conn,
            sid,
            12.5,
            "for god so loved the world",
            "en",
            Some(0.9),
        )
        .unwrap();
        insert_transcript(&conn, sid, 40.0, "turn to romans eight", "en", None).unwrap();

        let john = lookup_verse(&conn, "John", 3, 16).unwrap().unwrap();
        insert_detection(&conn, t1, Some(john.id), "direct", 0.96, "auto", Some(13.0)).unwrap();
        insert_detection(&conn, t1, None, "semantic", 0.62, "auto", Some(41.0)).unwrap();
        // A manual override cue counts toward "overrides".
        insert_cue(&conn, sid, "manual_override", Some("John 3:16"), 13.0).unwrap();
        insert_cue(&conn, sid, "clear_screens", None, 60.0).unwrap();

        let services = list_services(&conn).unwrap();
        assert_eq!(services.len(), 1);
        let s = &services[0];
        assert_eq!(s.title, "Sunday Service");
        assert_eq!(s.verses, 2); // two fired detections
        assert_eq!(s.overrides, 1); // one manual_override cue (clear_screens not counted)
        assert!((s.duration_secs - 40.0).abs() < 1e-6); // max transcript timestamp

        assert_eq!(service_transcripts(&conn, sid).unwrap().len(), 2);
        let dets = service_detections(&conn, sid).unwrap();
        assert_eq!(dets.len(), 2);
        assert_eq!(dets[0].reference.as_deref(), Some("John 3:16"));
        assert_eq!(dets[1].reference, None); // out-of-library verse
    }

    #[test]
    fn fresh_db_has_one_active_default_profile() {
        let conn = fresh_db();
        let all = list_voice_profiles(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "Default");
        assert!(all[0].language.is_none()); // auto-detect / code-switch
        let active = active_voice_profile(&conn).unwrap().unwrap();
        assert_eq!(active.id, all[0].id);
    }

    #[test]
    fn create_select_and_single_active_invariant() {
        let conn = fresh_db();
        let default_id = active_voice_profile(&conn).unwrap().unwrap().id;
        let pastor = create_voice_profile(&conn, "Pastor John", Some("yo")).unwrap();

        // Creating doesn't steal active.
        assert_eq!(active_voice_profile(&conn).unwrap().unwrap().id, default_id);

        set_active_profile(&conn, pastor).unwrap();
        let active = active_voice_profile(&conn).unwrap().unwrap();
        assert_eq!(active.id, pastor);
        assert_eq!(active.language.as_deref(), Some("yo"));
        // Exactly one active row.
        let n_active = list_voice_profiles(&conn)
            .unwrap()
            .iter()
            .filter(|p| p.is_active)
            .count();
        assert_eq!(n_active, 1);
    }

    #[test]
    fn threshold_calibration_persists_per_profile() {
        let conn = fresh_db();
        let id = active_voice_profile(&conn).unwrap().unwrap().id;
        save_profile_thresholds(&conn, id, 0.93, 0.55).unwrap();
        let p = active_voice_profile(&conn).unwrap().unwrap();
        assert!((p.auto_fire - 0.93).abs() < 1e-9);
        assert!((p.suggest - 0.55).abs() < 1e-9);
    }

    #[test]
    fn deleting_active_promotes_another_and_last_reseeds() {
        let conn = fresh_db();
        let default_id = active_voice_profile(&conn).unwrap().unwrap().id;
        let second = create_voice_profile(&conn, "Guest", None).unwrap();
        set_active_profile(&conn, second).unwrap();

        // Delete the active one → the remaining profile becomes active.
        delete_voice_profile(&conn, second).unwrap();
        let active = active_voice_profile(&conn).unwrap().unwrap();
        assert_eq!(active.id, default_id);

        // Delete the last one → a Default is re-seeded and made active.
        delete_voice_profile(&conn, default_id).unwrap();
        let all = list_voice_profiles(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert!(active_voice_profile(&conn).unwrap().is_some());
    }

    #[test]
    fn ensure_voice_profiles_is_idempotent() {
        let conn = fresh_db();
        ensure_voice_profiles(&conn).unwrap();
        ensure_voice_profiles(&conn).unwrap();
        assert_eq!(list_voice_profiles(&conn).unwrap().len(), 1);
    }

    #[test]
    fn channel_add_assign_display_and_delete() {
        let conn = fresh_db();
        let before = list_output_channels(&conn).unwrap().len();

        // Add a native-window channel.
        let id = add_channel(&conn, "Balcony", "native_window", 1).unwrap();
        let after = list_output_channels(&conn).unwrap();
        assert_eq!(after.len(), before + 1);
        let ch = after.iter().find(|c| c.id == id).unwrap();
        assert_eq!(ch.render_target, "native_window");
        assert!(ch.display_target.is_none());

        // Assign it to display index 1, then clear.
        set_channel_display(&conn, id, Some("1")).unwrap();
        let ch = list_output_channels(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap();
        assert_eq!(ch.display_target.as_deref(), Some("1"));
        set_channel_display(&conn, id, None).unwrap();
        assert!(list_output_channels(&conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == id)
            .unwrap()
            .display_target
            .is_none());

        // Delete it.
        delete_channel(&conn, id).unwrap();
        assert_eq!(list_output_channels(&conn).unwrap().len(), before);
    }

    #[test]
    fn open_creates_and_seeds_a_real_file_db() {
        // Exercise the real file path (not in-memory): open() must create the
        // parent dir, apply the schema, seed once, and be idempotent on reopen.
        let dir = std::env::temp_dir().join(format!("relay-test-{}", std::process::id()));
        let file = dir.join("nested").join("relay.db");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("RELAY_DB_PATH", &file);

        let count = {
            let conn = open().unwrap();
            verse_count(&conn).unwrap()
        };
        assert!(count > 31_000, "full corpus should be seeded");
        assert!(file.exists(), "db file should be created on disk");
        {
            // Reopen: not fresh, so no re-seed / no duplicate-key error.
            let conn = open().unwrap();
            assert_eq!(verse_count(&conn).unwrap(), count);
        }

        std::env::remove_var("RELAY_DB_PATH");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
