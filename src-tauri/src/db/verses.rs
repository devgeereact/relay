//! The Bible corpus: verses, translations, search, and the bundled KJV import.
//!
//! Search is layered deliberately: exact reference and phrase first, then FTS5
//! bm25 as the recall tail (docs/DECISIONS.md) — precise matches must always
//! outrank loose word-bag ones.

use super::channels::seed_channels;
use super::templates::seed_templates;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use serde::Serialize;

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

/// Look up a single verse by canonical reference. Returns None if absent.
pub fn lookup_verse(
    conn: &Connection,
    book: &str,
    chapter: i64,
    verse: i64,
) -> rusqlite::Result<Option<VerseRow>> {
    // Prefer the operator-selected translation (app_settings.active_translation);
    // fall back to whatever translation has the verse. No caller needs to know.
    conn.query_row(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          WHERE v.book = ?1 AND v.chapter = ?2 AND v.verse = ?3
          ORDER BY (CAST(t.id AS TEXT) =
                     COALESCE((SELECT value FROM app_settings WHERE key = 'active_translation'), '')) DESC
          LIMIT 1",
        (book, chapter, verse),
        row_to_verse,
    )
    .optional()
}

/// A Bible translation available in the corpus (Settings → Bible translations).
#[derive(Debug, Clone, Serialize)]
pub struct Translation {
    pub id: i64,
    pub name: String,
    pub abbreviation: String,
    pub language: String,
}

/// All translations present in the DB.
pub fn list_translations(conn: &Connection) -> rusqlite::Result<Vec<Translation>> {
    let mut stmt =
        conn.prepare("SELECT id, name, abbreviation, language FROM translations ORDER BY id")?;
    let rows = stmt.query_map([], |r| {
        Ok(Translation {
            id: r.get(0)?,
            name: r.get(1)?,
            abbreviation: r.get(2)?,
            language: r.get(3)?,
        })
    })?;
    rows.collect()
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

/// Full-text (LIKE) scripture search — the fallback when a query isn't a
/// parseable reference (e.g. "shepherd"). Offline, corpus-only. Prefers the
/// operator-selected translation, same as `lookup_verse`.
pub fn search_verses_text(
    conn: &Connection,
    needle: &str,
    limit: i64,
) -> rusqlite::Result<Vec<VerseRow>> {
    // Strip LIKE wildcards from user input so they're matched literally.
    let pat = format!("%{}%", needle.replace(['%', '_'], ""));
    let mut stmt = conn.prepare(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          WHERE v.text LIKE ?1
          ORDER BY (CAST(t.id AS TEXT) =
                     COALESCE((SELECT value FROM app_settings WHERE key = 'active_translation'), '')) DESC,
                   v.id
          LIMIT ?2",
    )?;
    let rows = stmt.query_map((pat, limit), row_to_verse)?;
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
const KJV_JSON: &str = include_str!("../../data/kjv.json");

#[derive(serde::Deserialize)]
struct KjvBook {
    chapters: Vec<Vec<String>>,
}

/// Seed a fresh database: one KJV translation + the full Bible + templates.
pub(super) fn seed(conn: &Connection) -> rusqlite::Result<()> {
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
                        clean_verse(text),
                    ))?;
                    count += 1;
                }
            }
        }
    }
    tx.commit()?;
    rebuild_verses_fts(conn)?;
    Ok(count)
}

/// (Re)build the FTS5 full-text index over `verses` for fast word/phrase search.
/// External-content table (no duplicated text); 'rebuild' repopulates from
/// `verses`. Porter stemmer so "shepherd" also matches "shepherds".
pub(super) fn rebuild_verses_fts(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS verses_fts USING fts5(
            text, content='verses', content_rowid='id', tokenize='porter unicode61');
         INSERT INTO verses_fts(verses_fts) VALUES('rebuild');",
    )
}

/// Full-text scripture search (FTS5, ranked by bm25). Terms are quoted so the
/// user's punctuation/operators are treated literally, then OR'd for recall —
/// bm25 floats the verse carrying the most (and rarest) of the words to the top.
/// So "the lord is my shepherd" and loose "lord shepherd" both surface Ps 23:1.
pub fn search_verses_fts(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> rusqlite::Result<Vec<VerseRow>> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("\"{}\"", t.to_lowercase()))
        .collect();
    if terms.is_empty() {
        return Ok(vec![]);
    }
    let match_q = terms.join(" OR ");
    let mut stmt = conn.prepare(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses_fts
           JOIN verses v ON v.id = verses_fts.rowid
           JOIN translations t ON t.id = v.translation_id
          WHERE verses_fts MATCH ?1
          ORDER BY (CAST(t.id AS TEXT) =
                     COALESCE((SELECT value FROM app_settings WHERE key = 'active_translation'), '')) DESC,
                   bm25(verses_fts)
          LIMIT ?2",
    )?;
    let rows = stmt.query_map((match_q, limit), row_to_verse)?;
    rows.collect()
}

/// Clean a raw KJV verse. The source data brackets two very different things in
/// `{ }`: supplied-word italics (real text: `{it was}`, `{and}`) and translator
/// marginal glosses (NOT verse text: `{green...: Heb. pastures of tender grass}`).
/// Keep the supplied words (drop only the braces); drop the glosses entirely;
/// then collapse the whitespace the removed glosses leave behind.
pub(super) fn clean_verse(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let inner = &after[..close];
                if !is_gloss(inner) {
                    out.push_str(inner); // supplied word — keep, minus braces
                }
                rest = &after[close + 1..];
            }
            None => {
                // Unbalanced brace — keep the remainder verbatim, sans '{'.
                out.push_str(after);
                rest = "";
                break;
            }
        }
    }
    out.push_str(rest);
    // Collapse the double spaces a dropped gloss leaves and trim the ends.
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A brace group is a marginal gloss (not verse text) if it carries a
/// translator note. Supplied-word italics are short and never contain a colon
/// or a language marker — verified against the full corpus.
fn is_gloss(inner: &str) -> bool {
    inner.contains(": ")
        || inner.starts_with("Or,")
        || inner.contains("Heb.")
        || inner.contains("Gr.")
        || inner.contains("Chaldee")
        || inner.contains("Syriac")
}

/// Forward-fill the full corpus for DBs created before the full-Bible import
/// (they hold only the old 15-verse dev seed). FK-safe: nulls any detection
/// verse links first, then replaces the verses.
pub(super) fn reimport_full_kjv(conn: &Connection) -> rusqlite::Result<()> {
    // ATOMIC. This DELETES EVERY VERSE and then re-imports 31,100 of them, and it
    // runs during a migration on app start. Without a transaction, a crash — or a
    // power cut, in a market where power cuts are ordinary — leaves the church with
    // an EMPTY BIBLE and an app that can no longer show a single verse.
    //
    // A transaction makes the whole thing all-or-nothing: either the new corpus
    // lands, or the old one is still there. There is no state in between.
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE detections SET verse_id = NULL", [])?;
    tx.execute("DELETE FROM verses", [])?;
    let tid = kjv_translation_id(&tx)?;
    import_full_kjv(&tx, tid)?;
    tx.commit()?;
    Ok(())
}
