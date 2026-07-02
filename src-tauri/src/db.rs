//! SQLite access layer.
//!
//! Single responsibility: local-first persistence against the schema in
//! docs/data/schema.sql. Nothing else in this codebase should touch SQLite
//! directly — go through this module. See PROMPT.md Phase 2.
//!
//! Offline-first: the schema is compiled in via `include_str!`, so there is no
//! runtime dependency on the docs/ file being shipped alongside the binary.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
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

/// Curated development seed — NOT the full Bible.
///
/// PROMPT.md Phase 2 asks only for enough data to develop the detection loop
/// against. These are famous, public-domain KJV verses reproduced verbatim;
/// Psalm 23 is seeded complete so context-memory ("...verse 4") can be tested
/// against consecutive verses. Loading a full translation is a later, *sourced*
/// import from a public-domain KJV data file — never hand-typed scripture,
/// which risks silent transcription errors in a product whose whole job is
/// showing the right verse.
fn seed(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO translations (name, abbreviation, language, license_type)
         VALUES (?1, ?2, ?3, ?4)",
        ("King James Version", "KJV", "en", "public domain"),
    )?;
    let translation_id = conn.last_insert_rowid();

    // (book, chapter, verse, text) — verbatim KJV.
    let verses: &[(&str, i64, i64, &str)] = &[
        ("Genesis", 1, 1, "In the beginning God created the heaven and the earth."),
        ("Genesis", 1, 2, "And the earth was without form, and void; and darkness was upon the face of the deep. And the Spirit of God moved upon the face of the waters."),
        ("Genesis", 1, 3, "And God said, Let there be light: and there was light."),

        ("Psalms", 23, 1, "The LORD is my shepherd; I shall not want."),
        ("Psalms", 23, 2, "He maketh me to lie down in green pastures: he leadeth me beside the still waters."),
        ("Psalms", 23, 3, "He restoreth my soul: he leadeth me in the paths of righteousness for his name's sake."),
        ("Psalms", 23, 4, "Yea, though I walk through the valley of the shadow of death, I will fear no evil: for thou art with me; thy rod and thy staff they comfort me."),
        ("Psalms", 23, 5, "Thou preparest a table before me in the presence of mine enemies: thou anointest my head with oil; my cup runneth over."),
        ("Psalms", 23, 6, "Surely goodness and mercy shall follow me all the days of my life: and I will dwell in the house of the LORD for ever."),

        ("John", 3, 16, "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life."),
        ("John", 3, 17, "For God sent not his Son into the world to condemn the world; but that the world through him might be saved."),

        ("Romans", 8, 28, "And we know that all things work together for good to them that love God, to them who are the called according to his purpose."),
        ("Romans", 8, 31, "What shall we then say to these things? If God be for us, who can be against us?"),
        ("Romans", 8, 38, "For I am persuaded, that neither death, nor life, nor angels, nor principalities, nor powers, nor things present, nor things to come,"),
        ("Romans", 8, 39, "Nor height, nor depth, nor any other creature, shall be able to separate us from the love of God, which is in Christ Jesus our Lord."),
    ];

    let mut stmt = conn.prepare(
        "INSERT INTO verses (translation_id, book, chapter, verse, text)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;
    for (book, chapter, verse, text) in verses {
        stmt.execute((translation_id, book, chapter, verse, text))?;
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
    fn seeds_expected_verse_count() {
        let conn = fresh_db();
        assert_eq!(verse_count(&conn).unwrap(), 15);
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
        assert!(lookup_verse(&conn, "Obadiah", 1, 1).unwrap().is_none());
    }

    #[test]
    fn open_creates_and_seeds_a_real_file_db() {
        // Exercise the real file path (not in-memory): open() must create the
        // parent dir, apply the schema, seed once, and be idempotent on reopen.
        let dir = std::env::temp_dir().join(format!("relay-test-{}", std::process::id()));
        let file = dir.join("nested").join("relay.db");
        let _ = std::fs::remove_dir_all(&dir);
        std::env::set_var("RELAY_DB_PATH", &file);

        {
            let conn = open().unwrap();
            assert_eq!(verse_count(&conn).unwrap(), 15);
        }
        assert!(file.exists(), "db file should be created on disk");
        {
            // Reopen: not fresh, so no re-seed / no duplicate-key error.
            let conn = open().unwrap();
            assert_eq!(verse_count(&conn).unwrap(), 15);
        }

        std::env::remove_var("RELAY_DB_PATH");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
