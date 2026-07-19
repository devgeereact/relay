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
/// Words too common in the KJV to constrain a search.
///
/// Deliberately SMALL and archaic-aware. This is not a general English stop list:
/// it exists so that a remembered phrase is matched on the words that carry it,
/// and every entry here is a word that appears in thousands of verses. `upon`
/// and `on` are both present precisely because they are the kind of word a
/// person misremembers — the whole problem this ladder solves.
const KJV_STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "did", "do", "for", "from", "had",
    "hath", "have", "he", "her", "him", "his", "i", "in", "is", "it", "me", "my", "not", "o", "of",
    "on", "or", "our", "shall", "she", "should", "so", "that", "the", "thee", "their", "them",
    "then", "there", "they", "thou", "thy", "to", "unto", "up", "upon", "us", "was", "we", "were",
    "what", "when", "which", "who", "will", "with", "ye", "you", "your",
];

fn is_stopword(t: &str) -> bool {
    KJV_STOPWORDS.contains(&t)
}

/// Build a ladder of FTS5 match expressions, STRICTEST FIRST.
///
/// ── The problem ────────────────────────────────────────────────────────────
///
/// This used to be a single `terms.join(" OR ")`. Searching a remembered phrase —
/// "they that wait on the lord" — asked for *any verse containing any of those
/// words*, so every verse with "the" or "that" became a candidate. Word order and
/// proximity carried no weight at all; bm25 was left to rescue a recall set of
/// thousands, and an operator hunting a half-remembered line got noise.
///
/// ── Why a ladder rather than one cleverer query ────────────────────────────
///
/// People misremember scripture, and they misremember it in a specific way: the
/// CONTENT words survive and the FUNCTION words drift. The KJV reads "they that
/// wait *upon* the LORD"; a person types "*on*". An exact phrase search finds
/// nothing, which is the worst possible answer because the verse is right there.
///
/// So each rung asks for less than the one above, and the first rung that
/// answers wins:
///
///   1. **Exact phrase** — the whole query, in order. Best possible hit.
///   2. **Content words, near each other** — order-free but proximity-bound, so
///      "wait … LORD" within a few words matches "wait upon the LORD" while a
///      verse mentioning "wait" in Genesis and "LORD" 30 words later does not.
///   3. **Content words, all present** — anywhere in the verse.
///   4. **Any word** — the old behaviour, kept as the floor so a typo like
///      "ont he lord" still returns something rather than nothing.
///
/// A query made ENTIRELY of stopwords ("I am", "it is I") keeps its words: for
/// those verses the function words *are* the content, and dropping them would
/// turn a valid search into an empty one.
pub(super) fn fts_ladder(query: &str) -> Vec<String> {
    let terms: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect();
    if terms.is_empty() {
        return Vec::new();
    }

    let quoted: Vec<String> = terms.iter().map(|t| format!("\"{t}\"")).collect();
    let content: Vec<String> = terms.iter().filter(|t| !is_stopword(t)).cloned().collect();
    // All stopwords ⇒ they are the content. See the doc comment.
    let content = if content.is_empty() {
        terms.clone()
    } else {
        content
    };
    let content_q: Vec<String> = content.iter().map(|t| format!("\"{t}\"")).collect();

    let mut ladder = Vec::new();

    // 1. The whole thing, in order. FTS5 treats a quoted multi-word string as a
    //    phrase, so this is a true sequence match.
    if terms.len() >= 2 {
        ladder.push(format!("\"{}\"", terms.join(" ")));
    }

    // 1b. CONTIGUOUS FRAGMENTS. Measured, not assumed: on the real 31k corpus,
    //     "they that wait on the lord" found Isaiah 40:31 nowhere in the top ten —
    //     with the exact phrase (rung 1) failing on "on" vs "upon", and with rung 2
    //     because dozens of verses pair "wait" with "LORD" and bm25 prefers the
    //     shorter ones.
    //
    //     What actually identifies the verse is the FRAGMENT the person got right:
    //     "they that wait". So try every contiguous run of words, longest first —
    //     a surviving 3- or 4-word fragment is far rarer than its words apart, and
    //     the drifted preposition simply falls outside it.
    //     Each fragment gets its OWN rung, longest first. OR-ing them together
    //     was measured and does not work: "they that wait" OR "on the lord"
    //     drowns the rare fragment in the common one, and Isaiah 40:31 stayed off
    //     the first page. Asked on its own, "they that wait" is decisive.
    for n in (3..=terms.len().min(5)).rev() {
        for w in terms.windows(n) {
            ladder.push(format!("\"{}\"", w.join(" ")));
        }
    }

    // 2. Content words within a window. NEAR's default is 10 tokens; 8 is about
    //    one clause of KJV English, which is the span a remembered phrase covers.
    if content_q.len() >= 2 {
        ladder.push(format!("NEAR({}, 8)", content_q.join(" ")));
        // 3. Same words, any distance.
        ladder.push(content_q.join(" AND "));
    } else if content_q.len() == 1 && terms.len() >= 2 {
        // One content word in a longer query: that word is the whole signal.
        ladder.push(content_q[0].clone());
    }

    // 4. The floor: anything. Never returns nothing just because a word was typed
    //    wrong.
    ladder.push(quoted.join(" OR "));
    ladder.dedup();
    ladder
}

pub fn search_verses_fts(
    conn: &Connection,
    query: &str,
    limit: i64,
) -> rusqlite::Result<Vec<VerseRow>> {
    // ACCUMULATE down the ladder; do not stop at the first rung that answers.
    //
    // Measured on the real corpus: for "they that wait on the lord", the rung
    // asking for "wait on the lord" answers first — with Psalms 27:14, which is a
    // perfectly good match for those words — and returning there hid Isaiah 40:31
    // completely. Stopping early means the strictest rung that matches ANYTHING
    // decides the whole result, and a stricter rung is not the same as a better
    // answer.
    //
    // So every rung contributes, in order, de-duplicated: earlier (stricter)
    // rungs rank first, later rungs fill the page. The caller (`search_scripture`)
    // then scores this whole list below its reference and phrase tiers.
    let mut out: Vec<VerseRow> = Vec::new();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for match_q in fts_ladder(query) {
        if out.len() >= limit as usize {
            break;
        }
        for v in fts_match(conn, &match_q, limit)? {
            if seen.insert(v.id) {
                out.push(v);
            }
        }
    }
    out.truncate(limit as usize);
    Ok(out)
}

/// Run one FTS5 match expression. A malformed expression is treated as "no
/// results" rather than an error: the ladder tries the next rung, and a search
/// box must never hand a volunteer a SQL syntax message.
fn fts_match(conn: &Connection, match_q: &str, limit: i64) -> rusqlite::Result<Vec<VerseRow>> {
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
    let rows = match stmt.query_map((match_q, limit), row_to_verse) {
        Ok(r) => r,
        Err(_) => return Ok(vec![]),
    };
    Ok(rows.filter_map(Result::ok).collect())
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

#[cfg(test)]
mod fts_ladder_tests {
    use super::*;

    // ⚠ THESE TESTS DO NOT DISCRIMINATE, and saying so is the point.
    //
    // They were written first and they pass against the OLD OR-everything query
    // too — with seven verses, bm25 alone ranks the right answer first. The
    // OR problem is a problem of SCALE, and a fixture cannot show it.
    //
    // They are kept as regression guards on the ladder's SHAPE. The actual
    // measurement lives in `fts_real_corpus` below, against all 31,100 verses.
    //
    // THE REQUEST THIS EXISTS FOR:
    //
    //   "For scripture search can we use multiple words/phrases instead of a
    //    single word — for instance 'they that wait on the lord' as opposed to
    //    'wait' or 'lord'."
    //
    // The search DID accept multiple words; it just threw the phrase away. Every
    // term was OR-ed, so the query asked for any verse containing "the" — and
    // word order and proximity counted for nothing.

    fn ladder(q: &str) -> Vec<String> {
        fts_ladder(q)
    }

    #[test]
    fn a_remembered_phrase_is_tried_in_order_first() {
        let l = ladder("they that wait on the lord");
        assert_eq!(
            l[0], "\"they that wait on the lord\"",
            "the strictest rung must be the whole phrase, in sequence"
        );
    }

    #[test]
    fn the_content_words_survive_a_misremembered_preposition() {
        // The KJV reads "they that wait UPON the LORD". A person types "on".
        // Rung 1 cannot match; rung 2 must, because it asks only for the content
        // words near each other — and both "on" and "upon" are stopwords, so the
        // difference between them stops mattering.
        let l = ladder("they that wait on the lord");
        let near = l
            .iter()
            .find(|r| r.starts_with("NEAR("))
            .expect("no NEAR rung");
        assert!(near.contains("\"wait\""));
        assert!(near.contains("\"lord\""));
        assert!(
            !near.contains("\"on\""),
            "a function word must not constrain the match"
        );
        assert!(!near.contains("\"they\""));
        assert!(!near.contains("\"that\""));
    }

    #[test]
    fn the_ladder_goes_strict_to_loose_and_ends_with_a_floor() {
        let l = ladder("they that wait on the lord");
        // phrase → NEAR → AND → OR
        assert!(l[0].starts_with('"'));
        assert!(l.iter().any(|r| r.starts_with("NEAR(")));
        assert!(l.iter().any(|r| r.contains(" AND ")));
        assert!(
            l.last().unwrap().contains(" OR "),
            "the last rung must be the permissive one, so a typo still returns something"
        );
    }

    #[test]
    fn a_typo_still_reaches_the_permissive_rung() {
        // The user's own example contained "ont he lord". "ont" matches nothing;
        // the OR floor still finds verses containing "lord", which is a far better
        // answer than an empty screen.
        let l = ladder("they that wait ont he lord");
        let floor = l.last().unwrap();
        assert!(floor.contains("\"lord\""));
        assert!(floor.contains(" OR "));
    }

    #[test]
    fn a_single_word_query_is_not_dressed_up_as_a_phrase() {
        let l = ladder("shepherd");
        assert!(!l.iter().any(|r| r.starts_with("NEAR(")));
        assert_eq!(l.last().unwrap(), "\"shepherd\"");
    }

    #[test]
    fn a_query_of_only_stopwords_keeps_its_words() {
        // "I am" (Exodus 3:14), "it is I". For these verses the function words
        // ARE the content — dropping them would turn a valid search into an
        // empty one.
        let l = ladder("i am");
        assert!(
            l.iter().any(|r| r.contains("\"i\"")),
            "stripping every word left nothing to search for"
        );
    }

    #[test]
    fn one_content_word_among_function_words_becomes_the_signal() {
        let l = ladder("the lord is my");
        assert!(l.iter().any(|r| r == "\"lord\""));
    }

    #[test]
    fn punctuation_and_case_do_not_change_the_terms() {
        let a = ladder("Wait, upon the LORD!");
        let b = ladder("wait upon the lord");
        assert_eq!(
            a.iter().find(|r| r.starts_with("NEAR(")),
            b.iter().find(|r| r.starts_with("NEAR(")),
        );
    }

    #[test]
    fn an_empty_query_produces_no_rungs() {
        assert!(ladder("").is_empty());
        assert!(ladder("   ,, ").is_empty());
    }
}

#[cfg(test)]
mod fts_search_tests {
    use super::*;
    use crate::db::SCHEMA;

    /// A handful of real KJV verses, enough to prove the ladder finds the right
    /// one and rejects the wrong ones. Isaiah 40:31 is the case the request came
    /// from — note it reads "wait UPON the LORD".
    const FIXTURE: &[(&str, i64, i64, &str)] = &[
        ("Isaiah", 40, 31, "But they that wait upon the LORD shall renew their strength; they shall mount up with wings as eagles"),
        ("Psalms", 27, 14, "Wait on the LORD: be of good courage, and he shall strengthen thine heart"),
        ("Psalms", 23, 1, "The LORD is my shepherd; I shall not want."),
        ("Genesis", 1, 1, "In the beginning God created the heaven and the earth."),
        ("John", 3, 16, "For God so loved the world, that he gave his only begotten Son"),
        ("Exodus", 3, 14, "And God said unto Moses, I AM THAT I AM"),
        ("Habakkuk", 2, 3, "though it tarry, wait for it; because it will surely come"),
    ];

    fn db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO translations (id, name, abbreviation, language) VALUES (1, 'King James Version', 'KJV', 'en')",
            [],
        )
        .unwrap();
        for (b, c, v, t) in FIXTURE {
            conn.execute(
                "INSERT INTO verses (translation_id, book, chapter, verse, text)
                 VALUES (1, ?1, ?2, ?3, ?4)",
                (b, c, v, t),
            )
            .unwrap();
        }
        rebuild_verses_fts(&conn).unwrap();
        conn
    }

    fn refs(rows: &[VerseRow]) -> Vec<String> {
        rows.iter()
            .map(|r| format!("{} {}:{}", r.book, r.chapter, r.verse))
            .collect()
    }

    #[test]
    fn finds_the_verse_from_a_misremembered_phrase() {
        // THE ORIGINAL REQUEST, end to end. The query says "on"; the verse says
        // "upon". Before the ladder this OR-ed six words and let bm25 sort out a
        // recall set containing every verse with "the" in it.
        let conn = db();
        let hits = search_verses_fts(&conn, "they that wait on the lord", 5).unwrap();
        assert!(!hits.is_empty(), "a remembered phrase returned nothing");
        let got = refs(&hits);
        assert!(
            got.iter().any(|r| r == "Isaiah 40:31") || got[0] == "Psalms 27:14",
            "expected the waiting-on-the-LORD verses, got {got:?}"
        );
        // Genesis and John share only stopwords with the query — they must not
        // outrank the real answer.
        assert_ne!(got[0], "Genesis 1:1");
        assert_ne!(got[0], "John 3:16");
    }

    #[test]
    fn an_exact_phrase_wins_outright() {
        let conn = db();
        let hits = search_verses_fts(&conn, "the lord is my shepherd", 5).unwrap();
        assert_eq!(refs(&hits)[0], "Psalms 23:1");
    }

    #[test]
    fn proximity_matters_more_than_mere_presence() {
        // Habakkuk contains "wait"; Isaiah contains "wait" AND "lord" close
        // together. The near rung must prefer Isaiah.
        let conn = db();
        let hits = search_verses_fts(&conn, "wait lord strength", 5).unwrap();
        assert_eq!(refs(&hits)[0], "Isaiah 40:31");
    }

    #[test]
    fn a_query_of_only_stopwords_still_finds_its_verse() {
        // Exodus 3:14 is almost entirely function words.
        let conn = db();
        let hits = search_verses_fts(&conn, "I AM THAT I AM", 5).unwrap();
        assert!(
            refs(&hits).contains(&"Exodus 3:14".to_string()),
            "stopword-only query lost its verse: {:?}",
            refs(&hits)
        );
    }

    #[test]
    fn a_typo_degrades_to_something_useful_instead_of_nothing() {
        // "ont" is not a word. The floor rung still finds the LORD verses.
        let conn = db();
        let hits = search_verses_fts(&conn, "they that wait ont he lord", 5).unwrap();
        assert!(!hits.is_empty(), "a typo produced an empty screen");
    }

    #[test]
    fn a_single_word_still_works() {
        let conn = db();
        let hits = search_verses_fts(&conn, "shepherd", 5).unwrap();
        assert_eq!(refs(&hits)[0], "Psalms 23:1");
    }

    #[test]
    fn nonsense_returns_nothing_rather_than_everything() {
        let conn = db();
        let hits = search_verses_fts(&conn, "zzzzqqqx", 5).unwrap();
        assert!(hits.is_empty());
    }
}

#[cfg(test)]
mod fts_real_corpus {
    use super::*;

    /// Does the ladder actually beat the old OR-everything query on the REAL
    /// 31k-verse corpus?
    ///
    /// ```text
    /// cargo test fts_real_corpus -- --ignored --nocapture
    /// ```
    ///
    /// Ignored because it imports the whole KJV. It exists because the small
    /// fixture above does NOT discriminate: with seven verses, bm25 alone ranks
    /// the right answer first, so those tests pass against the old code too. The
    /// OR problem is a problem of SCALE — thousands of candidates sharing a
    /// stopword — and it can only be measured at scale.
    #[test]
    #[ignore]
    fn ladder_versus_or_on_real_queries() {
        let conn = Connection::open_in_memory().unwrap();
        // The real init path: schema + full-corpus import + FTS build.
        crate::db::migrate(&conn, true).unwrap();
        rebuild_verses_fts(&conn).unwrap();

        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM verses", [], |r| r.get(0))
            .unwrap();
        println!("corpus: {total} verses\n");

        // What the OLD code did: every term, OR-ed.
        let old_query = |q: &str| -> String {
            q.split(|c: char| !c.is_alphanumeric())
                .filter(|t| !t.is_empty())
                .map(|t| format!("\"{}\"", t.to_lowercase()))
                .collect::<Vec<_>>()
                .join(" OR ")
        };

        // Which rung fires, and what does it return?
        for (i, rung) in fts_ladder("they that wait on the lord").iter().enumerate() {
            let rows = fts_match(&conn, rung, 5).unwrap();
            let got: Vec<String> = rows
                .iter()
                .map(|r| format!("{} {}:{}", r.book, r.chapter, r.verse))
                .collect();
            println!("  rung {i}: {rung}\n     -> {got:?}");
        }
        println!();

        let cases = [
            ("they that wait on the lord", "Isaiah", 40, 31),
            ("the lord is my shepherd", "Psalms", 23, 1),
            ("in the beginning god created", "Genesis", 1, 1),
            ("be still and know that i am god", "Psalms", 46, 10),
            ("i can do all things through christ", "Philippians", 4, 13),
        ];

        let rank_of = |rows: &[VerseRow], b: &str, c: i64, v: i64| -> Option<usize> {
            rows.iter()
                .position(|r| r.book == b && r.chapter == c && r.verse == v)
        };

        let mut old_better = 0;
        let mut new_better = 0;
        for (q, b, c, v) in cases {
            let new_rows = search_verses_fts(&conn, q, 10).unwrap();
            let old_rows = fts_match(&conn, &old_query(q), 10).unwrap();
            let rn = rank_of(&new_rows, b, c, v);
            let ro = rank_of(&old_rows, b, c, v);
            let fmt = |r: Option<usize>| match r {
                Some(i) => format!("#{}", i + 1),
                None => "MISSING".into(),
            };
            println!("{q:<38} target {b} {c}:{v}");
            println!("    ladder: {:<8} or-only: {}", fmt(rn), fmt(ro));
            match (rn, ro) {
                (Some(a), Some(x)) if a < x => new_better += 1,
                (Some(a), Some(x)) if a > x => old_better += 1,
                (Some(_), None) => new_better += 1,
                (None, Some(_)) => old_better += 1,
                _ => {}
            }
        }
        println!(
            "\nladder better on {new_better}, worse on {old_better}, of {} cases",
            cases.len()
        );
    }
}
