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
        }
        // Forward-fill the full Bible for DBs created with the old 15-verse seed.
        if verse_count(&conn)? < 30_000 {
            reimport_full_kjv(&conn)?;
        }
    }
    Ok(conn)
}

/// Apply the full schema and seed a fresh connection. Public so tests (and any
/// future in-memory scratch DB) can build a ready-to-query database directly.
pub fn init_fresh(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(SCHEMA)?;
    seed(conn)?;
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

/// The four built-in output templates (SPEC §5). These match the frontend
/// defaults in src/lib/templates.js — kept as the seed source of truth so a
/// fresh install has usable channels immediately. `layout`/`style` are the JSON
/// the shared renderer interprets.
fn seed_templates(conn: &Connection) -> rusqlite::Result<()> {
    let templates: &[(&str, &str, &str)] = &[
        (
            "Classic Serif",
            r##"{"regions":["verse_text","reference"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)","accent":"var(--amber)","verseColor":"#f4e4c8","verseSize":"4.6vw","refSize":"1.9vw","italicRef":true}"##,
        ),
        (
            "Stage Mono",
            r##"{"regions":["reference","verse_text"],"align":"left","lowerThird":false,"refFirst":true}"##,
            r##"{"font":"var(--f-display)","background":"#000000","accent":"var(--teal)","verseColor":"#f2f5f6","verseSize":"5vw","refSize":"2vw","italicRef":false}"##,
        ),
        (
            "Lower Third",
            r##"{"regions":["verse_text","reference"],"align":"left","lowerThird":true,"refFirst":false}"##,
            r##"{"font":"var(--f-body)","background":"transparent","accent":"var(--violet)","verseColor":"#1c1224","verseSize":"2.4vw","refSize":"1.4vw","italicRef":false}"##,
        ),
        (
            "Lobby Warm",
            r##"{"regions":["reference","verse_text"],"align":"center","lowerThird":false,"refFirst":false}"##,
            r##"{"font":"var(--f-serif)","background":"linear-gradient(160deg, #241419, #120a0e)","accent":"var(--rose)","verseColor":"#f0dfe3","verseSize":"3.2vw","refSize":"1.6vw","italicRef":false}"##,
        ),
    ];
    let mut stmt = conn.prepare(
        "INSERT INTO templates (name, region_config_json, style_json) VALUES (?1, ?2, ?3)",
    )?;
    for (name, layout, style) in templates {
        stmt.execute((name, layout, style))?;
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
