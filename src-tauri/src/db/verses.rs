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
                let tail = &after[close + 1..];
                if !is_gloss(inner, is_trailing_run(tail)) {
                    out.push_str(inner); // supplied word — keep, minus braces
                }
                rest = tail;
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

/// Is this brace group part of the run of marginal notes at the END of a verse?
/// True when nothing but whitespace and further brace groups follow it.
///
/// **Position is the primary signal, and it has to be**, because neither half of
/// the corpus is marked: `{feed or, rule}` (Micah 5:4) is a note and `{it was}`
/// is verse text, and no wording rule separates them. Every marginal note in
/// `kjv.json` sits in that trailing run — checked group by group over all 31,102
/// verses — so the run is what identifies them.
fn is_trailing_run(tail: &str) -> bool {
    let mut tail = tail;
    loop {
        tail = tail.trim_start();
        if tail.is_empty() {
            return true;
        }
        if !tail.starts_with('{') {
            return false;
        }
        match tail.find('}') {
            Some(close) => tail = &tail[close + 1..],
            None => return false,
        }
    }
}

/// A brace group is a marginal gloss (not verse text) rather than an italicised
/// supplied word.
///
/// **`inner.contains(": ")` used to be the whole rule, and it deleted real
/// scripture.** Seven verses carry a colon inside their supplied words — Genesis
/// 30:27 `{tarry: for}`, Genesis 42:34 `{men: so}`, Leviticus 23:21, Job 36:5,
/// Psalms 18:41, Isaiah 6:13, Jeremiah 22:16 — and each of them reached the wall
/// with words missing ("if I have found favour in thine eyes, I have learned by
/// experience"). The rule also worked the other way: eight notes with no colon
/// were kept AS scripture, so Luke 17:36 ended "the other left. this verse is not
/// found in most of the Greek copies" on a congregation's screen.
///
/// Both classes are closed by asking WHERE the group sits (see `is_trailing_run`)
/// and keeping the wording markers only for the one note that appears mid-verse
/// (Hebrews 10:34, caught by its `...` lead-in ellipsis). Verified verse by verse
/// against an independent copy of the KJV: 15 verses changed, and no other.
fn is_gloss(inner: &str, trailing: bool) -> bool {
    trailing
        || inner.contains("...")
        || inner.starts_with("Or,")
        || inner.starts_with("or,")
        || inner.contains("Heb.")
        || inner.contains("Gr.")
        || inner.contains("Chaldee")
        || inner.contains("Syriac")
}

/// The KJV's own versification: how many verses each of the 1,189 chapters has,
/// one line per book in canonical order.
///
/// **This exists because a verse number in `kjv.json` is a POSITION, not a
/// label.** `import_full_kjv` numbers verses `vi + 1`, so a chapter that is one
/// verse short does not lose its last verse — it renumbers every verse after the
/// gap, silently, and Relay then answers a correct reference with the next verse
/// along. The bundled file was short six verses (Matthew 2:16, 22:1, 26:38, Mark
/// 4:40, 7:11, 8:8) and carried four split ones, so "Matthew 22:37" put the words
/// of 22:38 on a wall and "Matthew 2:23" did not exist at all. Nothing detected
/// it: the seed test asserted `> 31_000`, which 31,100 satisfies.
///
/// The numbers below were taken from an independent public-domain KJV whose
/// verses carry EXPLICIT numbers rather than positions, and they total 31,102 —
/// the KJV's own count. Do not regenerate this table from `kjv.json`; that is the
/// artefact it exists to check.
#[cfg(test)]
const KJV_VERSES_PER_CHAPTER: &[&str] = &[
    "Genesis:31,25,24,26,32,22,24,22,29,32,32,20,18,24,21,16,27,33,38,18,34,24,20,67,34,35,46,22,35,43,55,32,20,31,29,43,36,30,23,23,57,38,34,34,28,34,31,22,33,26",
    "Exodus:22,25,22,31,23,30,25,32,35,29,10,51,22,31,27,36,16,27,25,26,36,31,33,18,40,37,21,43,46,38,18,35,23,35,35,38,29,31,43,38",
    "Leviticus:17,16,17,35,19,30,38,36,24,20,47,8,59,57,33,34,16,30,37,27,24,33,44,23,55,46,34",
    "Numbers:54,34,51,49,31,27,89,26,23,36,35,16,33,45,41,50,13,32,22,29,35,41,30,25,18,65,23,31,40,16,54,42,56,29,34,13",
    "Deuteronomy:46,37,29,49,33,25,26,20,29,22,32,32,18,29,23,22,20,22,21,20,23,30,25,22,19,19,26,68,29,20,30,52,29,12",
    "Joshua:18,24,17,24,15,27,26,35,27,43,23,24,33,15,63,10,18,28,51,9,45,34,16,33",
    "Judges:36,23,31,24,31,40,25,35,57,18,40,15,25,20,20,31,13,31,30,48,25",
    "Ruth:22,23,18,22",
    "1 Samuel:28,36,21,22,12,21,17,22,27,27,15,25,23,52,35,23,58,30,24,42,15,23,29,22,44,25,12,25,11,31,13",
    "2 Samuel:27,32,39,12,25,23,29,18,13,19,27,31,39,33,37,23,29,33,43,26,22,51,39,25",
    "1 Kings:53,46,28,34,18,38,51,66,28,29,43,33,34,31,34,34,24,46,21,43,29,53",
    "2 Kings:18,25,27,44,27,33,20,29,37,36,21,21,25,29,38,20,41,37,37,21,26,20,37,20,30",
    "1 Chronicles:54,55,24,43,26,81,40,40,44,14,47,40,14,17,29,43,27,17,19,8,30,19,32,31,31,32,34,21,30",
    "2 Chronicles:17,18,17,22,14,42,22,18,31,19,23,16,22,15,19,14,19,34,11,37,20,12,21,27,28,23,9,27,36,27,21,33,25,33,27,23",
    "Ezra:11,70,13,24,17,22,28,36,15,44",
    "Nehemiah:11,20,32,23,19,19,73,18,38,39,36,47,31",
    "Esther:22,23,15,17,14,14,10,17,32,3",
    "Job:22,13,26,21,27,30,21,22,35,22,20,25,28,22,35,22,16,21,29,29,34,30,17,25,6,14,23,28,25,31,40,22,33,37,16,33,24,41,30,24,34,17",
    "Psalms:6,12,8,8,12,10,17,9,20,18,7,8,6,7,5,11,15,50,14,9,13,31,6,10,22,12,14,9,11,12,24,11,22,22,28,12,40,22,13,17,13,11,5,26,17,11,9,14,20,23,19,9,6,7,23,13,11,11,17,12,8,12,11,10,13,20,7,35,36,5,24,20,28,23,10,12,20,72,13,19,16,8,18,12,13,17,7,18,52,17,16,15,5,23,11,13,12,9,9,5,8,28,22,35,45,48,43,13,31,7,10,10,9,8,18,19,2,29,176,7,8,9,4,8,5,6,5,6,8,8,3,18,3,3,21,26,9,8,24,13,10,7,12,15,21,10,20,14,9,6",
    "Proverbs:33,22,35,27,23,35,27,36,18,32,31,28,25,35,33,33,28,24,29,30,31,29,35,34,28,28,27,28,27,33,31",
    "Ecclesiastes:18,26,22,16,20,12,29,17,18,20,10,14",
    "Song of Solomon:17,17,11,16,16,13,13,14",
    "Isaiah:31,22,26,6,30,13,25,22,21,34,16,6,22,32,9,14,14,7,25,6,17,25,18,23,12,21,13,29,24,33,9,20,24,17,10,22,38,22,8,31,29,25,28,28,25,13,15,22,26,11,23,15,12,17,13,12,21,14,21,22,11,12,19,12,25,24",
    "Jeremiah:19,37,25,31,31,30,34,22,26,25,23,17,27,22,21,21,27,23,15,18,14,30,40,10,38,24,22,17,32,24,40,44,26,22,19,32,21,28,18,16,18,22,13,30,5,28,7,47,39,46,64,34",
    "Lamentations:22,22,66,22,22",
    "Ezekiel:28,10,27,17,17,14,27,18,11,22,25,28,23,23,8,63,24,32,14,49,32,31,49,27,17,21,36,26,21,26,18,32,33,31,15,38,28,23,29,49,26,20,27,31,25,24,23,35",
    "Daniel:21,49,30,37,31,28,28,27,27,21,45,13",
    "Hosea:11,23,5,19,15,11,16,14,17,15,12,14,16,9",
    "Joel:20,32,21",
    "Amos:15,16,15,13,27,14,17,14,15",
    "Obadiah:21",
    "Jonah:17,10,10,11",
    "Micah:16,13,12,13,15,16,20",
    "Nahum:15,13,19",
    "Habakkuk:17,20,19",
    "Zephaniah:18,15,20",
    "Haggai:15,23",
    "Zechariah:21,13,10,14,11,15,14,23,17,12,17,14,9,21",
    "Malachi:14,17,18,6",
    "Matthew:25,23,17,25,48,34,29,34,38,42,30,50,58,36,39,28,27,35,30,34,46,46,39,51,46,75,66,20",
    "Mark:45,28,35,41,43,56,37,38,50,52,33,44,37,72,47,20",
    "Luke:80,52,38,44,39,49,50,56,62,42,54,59,35,35,32,31,37,43,48,47,38,71,56,53",
    "John:51,25,36,54,47,71,53,59,41,42,57,50,38,31,27,33,26,40,42,31,25",
    "Acts:26,47,26,37,42,15,60,40,43,48,30,25,52,28,41,40,34,28,41,38,40,30,35,27,27,32,44,31",
    "Romans:32,29,31,25,21,23,25,39,33,21,36,21,14,23,33,27",
    "1 Corinthians:31,16,23,21,13,20,40,13,27,33,34,31,13,40,58,24",
    "2 Corinthians:24,17,18,18,21,18,16,24,15,18,33,21,14",
    "Galatians:24,21,29,31,26,18",
    "Ephesians:23,22,21,32,33,24",
    "Philippians:30,30,21,23",
    "Colossians:29,23,25,18",
    "1 Thessalonians:10,20,13,18,28",
    "2 Thessalonians:12,17,18",
    "1 Timothy:20,15,16,16,25,21",
    "2 Timothy:18,26,17,22",
    "Titus:16,15,15",
    "Philemon:25",
    "Hebrews:14,18,19,16,14,20,28,13,28,39,40,29,25",
    "James:27,26,18,17,20",
    "1 Peter:25,25,22,19,14",
    "2 Peter:21,22,18",
    "1 John:10,29,24,21,21",
    "2 John:13",
    "3 John:14",
    "Jude:25",
    "Revelation:20,29,22,11,14,17,17,13,21,11,19,17,18,20,8,21,18,24,21,15,27,21",
];

/// Forward-fill the full corpus for DBs created before the full-Bible import
/// (they hold only the old 15-verse dev seed). FK-safe: nulls any detection
/// verse links first, then replaces the verses.
pub(super) fn reimport_full_kjv(conn: &Connection) -> rusqlite::Result<()> {
    // ATOMIC. This DELETES EVERY VERSE and then re-imports 31,102 of them, and it
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
    // measurement lives in `fts_real_corpus` below, against all 31,102 verses.
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

/// One book, and how many chapters it has, for the Library's browse tree.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BookSummary {
    pub book: String,
    pub chapters: i64,
}

/// Books present in a translation, IN CANONICAL ORDER.
///
/// Ordered by `detection::CANONICAL_BOOKS`, not alphabetically. A Bible that
/// opens with Amos, Acts, Chronicles is not a Bible anyone can navigate — the
/// order is part of what the book IS, and every reader's muscle memory depends
/// on it. Books the corpus does not contain are simply absent.
pub fn list_books(conn: &Connection, translation_id: i64) -> rusqlite::Result<Vec<BookSummary>> {
    let mut stmt = conn
        .prepare("SELECT book, MAX(chapter) FROM verses WHERE translation_id = ?1 GROUP BY book")?;
    let rows = stmt.query_map([translation_id], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let mut found: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
    for row in rows {
        let (b, c) = row?;
        found.insert(b, c);
    }
    Ok(crate::detection::CANONICAL_BOOKS
        .iter()
        .filter_map(|b| {
            found.get(*b).map(|&chapters| BookSummary {
                book: (*b).to_string(),
                chapters,
            })
        })
        .collect())
}

/// Every verse of one chapter, in order — the Library's reading pane.
pub fn chapter_verses(
    conn: &Connection,
    translation_id: i64,
    book: &str,
    chapter: i64,
) -> rusqlite::Result<Vec<VerseRow>> {
    let mut stmt = conn.prepare(
        "SELECT v.id, v.book, v.chapter, v.verse, v.text, t.abbreviation
           FROM verses v JOIN translations t ON t.id = v.translation_id
          WHERE v.translation_id = ?1 AND v.book = ?2 AND v.chapter = ?3
          ORDER BY v.verse",
    )?;
    let rows = stmt.query_map((translation_id, book, chapter), row_to_verse)?;
    rows.collect()
}

#[cfg(test)]
mod browse_tests {
    use super::*;

    fn corpus_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        conn
    }

    #[test]
    fn books_come_back_in_canonical_order_not_alphabetical() {
        // THE POINT OF THE WHOLE FUNCTION. A Bible whose contents page opens
        // "Acts, Amos, Chronicles" is not a Bible anyone can navigate — the
        // order is part of what the book IS, and every reader's muscle memory
        // depends on it. `GROUP BY book` returns alphabetical; this must not.
        let conn = corpus_db();
        let tid = crate::db::active_translation_id(&conn).unwrap();
        let books = list_books(&conn, tid).unwrap();
        assert!(
            books.len() > 60,
            "expected a whole Bible, got {}",
            books.len()
        );
        assert_eq!(books[0].book, "Genesis");
        assert_eq!(books[1].book, "Exodus");
        assert_eq!(books.last().unwrap().book, "Revelation");

        let alphabetical = {
            let mut v: Vec<String> = books.iter().map(|b| b.book.clone()).collect();
            v.sort();
            v
        };
        let actual: Vec<String> = books.iter().map(|b| b.book.clone()).collect();
        assert_ne!(actual, alphabetical, "books came back alphabetical");
    }

    #[test]
    fn chapter_counts_are_real() {
        let conn = corpus_db();
        let tid = crate::db::active_translation_id(&conn).unwrap();
        let books = list_books(&conn, tid).unwrap();
        let find = |n: &str| books.iter().find(|b| b.book == n).unwrap().chapters;
        assert_eq!(find("Genesis"), 50);
        assert_eq!(find("Psalms"), 150);
        assert_eq!(find("Jude"), 1);
        assert_eq!(find("Revelation"), 22);
    }

    #[test]
    fn a_chapter_reads_in_verse_order() {
        let conn = corpus_db();
        let tid = crate::db::active_translation_id(&conn).unwrap();
        let v = chapter_verses(&conn, tid, "Genesis", 1).unwrap();
        assert_eq!(v.len(), 31, "Genesis 1 has 31 verses");
        assert_eq!(v[0].verse, 1);
        assert!(v[0].text.starts_with("In the beginning"));
        // Ordering is explicit in SQL; prove it rather than trust insertion order.
        assert!(v.windows(2).all(|w| w[0].verse < w[1].verse));
    }

    #[test]
    fn a_chapter_that_does_not_exist_is_empty_not_an_error() {
        // Psalm 151 is not in the KJV. The Library must show an empty chapter,
        // never fail to open.
        let conn = corpus_db();
        let tid = crate::db::active_translation_id(&conn).unwrap();
        assert!(chapter_verses(&conn, tid, "Psalms", 151)
            .unwrap()
            .is_empty());
        assert!(chapter_verses(&conn, tid, "Nonesuch", 1)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_fresh_install_that_never_chose_a_translation_still_opens() {
        // `active_translation_id` falls back rather than erroring — a Library
        // that refuses to open because nobody has picked a Bible yet is absurd.
        let conn = corpus_db();
        assert!(crate::db::get_setting(&conn, "active_translation")
            .unwrap()
            .is_none());
        assert!(crate::db::active_translation_id(&conn).is_ok());
        assert!(
            !list_books(&conn, crate::db::active_translation_id(&conn).unwrap())
                .unwrap()
                .is_empty()
        );
    }
}

#[cfg(test)]
mod corpus_tests {
    use super::*;

    /// Every chapter of the bundled corpus has the number of verses the KJV
    /// itself has — the pin under [`KJV_VERSES_PER_CHAPTER`].
    ///
    /// **Test the bug, not the fix**: delete any verse from `kjv.json` and this
    /// names the chapter. A count check over the whole Bible would not — six
    /// missing verses and four split ones very nearly cancelled out (31,100
    /// against 31,102), which is how they survived a total-count assertion.
    #[test]
    fn the_bundled_kjv_matches_the_kjvs_own_versification() {
        let raw = KJV_JSON.trim_start_matches('\u{feff}');
        let books: Vec<KjvBook> = serde_json::from_str(raw).expect("kjv.json parses");
        assert_eq!(books.len(), 66, "the canon is 66 books");
        assert_eq!(
            books.len(),
            KJV_VERSES_PER_CHAPTER.len(),
            "the pin covers every book"
        );

        let mut total = 0usize;
        for (bi, book) in books.iter().enumerate() {
            let (name, counts) = KJV_VERSES_PER_CHAPTER[bi]
                .split_once(':')
                .expect("each pin line is 'Book:n,n,…'");
            let want: Vec<usize> = counts.split(',').map(|n| n.parse().unwrap()).collect();
            assert_eq!(
                book.chapters.len(),
                want.len(),
                "{name} has the wrong number of chapters"
            );
            for (ci, chapter) in book.chapters.iter().enumerate() {
                assert_eq!(
                    chapter.len(),
                    want[ci],
                    "{name} {} has {} verses, the KJV has {}",
                    ci + 1,
                    chapter.len(),
                    want[ci]
                );
                total += chapter.len();
            }
        }
        assert_eq!(total, 31_102, "the KJV has 31,102 verses");
    }

    /// No verse may be empty. An empty string renders as a blank wall under a
    /// correct reference, which is the one failure `pipeline::preflight` refuses
    /// and the one an operator cannot diagnose from the room.
    #[test]
    fn no_bundled_verse_is_empty() {
        let raw = KJV_JSON.trim_start_matches('\u{feff}');
        let books: Vec<KjvBook> = serde_json::from_str(raw).expect("kjv.json parses");
        for (bi, book) in books.iter().enumerate() {
            for (ci, chapter) in book.chapters.iter().enumerate() {
                for (vi, text) in chapter.iter().enumerate() {
                    assert!(
                        !clean_verse(text).is_empty(),
                        "book {} chapter {} verse {} is empty after cleaning",
                        bi + 1,
                        ci + 1,
                        vi + 1
                    );
                }
            }
        }
    }

    /// The seven verses whose supplied words carry a colon, and which the old
    /// `contains(": ")` rule deleted outright.
    #[test]
    fn supplied_words_containing_a_colon_survive() {
        assert_eq!(
            clean_verse(
                "And Laban said unto him, I pray thee, if I have found favour in thine eyes, {tarry: for} I have learned by experience that the LORD hath blessed me for thy sake."
            ),
            "And Laban said unto him, I pray thee, if I have found favour in thine eyes, tarry: for I have learned by experience that the LORD hath blessed me for thy sake."
        );
        assert!(clean_verse(
            "Behold, God is mighty, and despiseth not {any: he is} mighty in strength and wisdom."
        )
        .contains("any: he is mighty"));
    }

    /// The eight marginal notes with no wording marker at all, which the old rule
    /// kept AS scripture. Luke 17:36 is the one that reached a wall.
    #[test]
    fn a_trailing_note_is_never_scripture_however_it_is_worded() {
        assert_eq!(
            clean_verse(
                "Two men shall be in the field; the one shall be taken, and the other left. {this verse is not found in most of the Greek copies}"
            ),
            "Two men shall be in the field; the one shall be taken, and the other left."
        );
        assert_eq!(
            clean_verse("and now shall he be great unto the ends of the earth. {feed or, rule}"),
            "and now shall he be great unto the ends of the earth."
        );
        // …and a note that sits mid-verse is still caught by its lead-in ellipsis.
        assert_eq!(
            clean_verse("ye had {in yourselves...: or, that ye have in} heaven a better substance"),
            "ye had heaven a better substance"
        );
    }

    /// The four verses our copy had split, and the six it had dropped — the
    /// references an operator would have got wrong, asserted by their words.
    #[test]
    fn the_repaired_references_hold_their_own_words() {
        let raw = KJV_JSON.trim_start_matches('\u{feff}');
        let books: Vec<KjvBook> = serde_json::from_str(raw).expect("kjv.json parses");
        let at = |b: usize, c: usize, v: usize| clean_verse(&books[b - 1].chapters[c - 1][v - 1]);

        // Matthew is book 40. 2:16 was missing, so 2:16–22 each showed the next
        // verse along and 2:23 did not exist.
        assert!(at(40, 2, 16).starts_with("Then Herod, when he saw that he was mocked"));
        assert!(at(40, 2, 23).contains("He shall be called a Nazarene"));
        // 22:1 was missing, so the great commandment fired one verse late.
        assert!(at(40, 22, 1).starts_with("And Jesus answered and spake unto them again"));
        assert!(at(40, 22, 37).contains("Thou shalt love the Lord thy God"));
        // 26:38 was missing, so Gethsemane was off by one from there on.
        assert!(at(40, 26, 38).contains("My soul is exceeding sorrowful"));
        assert!(at(40, 26, 39).contains("let this cup pass from me"));
        // Mark is book 41.
        assert!(at(41, 4, 40).contains("Why are ye so fearful"));
        assert!(at(41, 7, 11).contains("It is Corban"));
        assert!(at(41, 8, 8).contains("seven baskets"));
        // The four our copy had split: each is now one verse, whole.
        assert!(
            at(9, 20, 42).contains("for ever.")
                && at(9, 20, 42).contains("Jonathan went into the city")
        );
        assert!(at(11, 22, 43).contains("the high places were not taken away"));
        assert!(at(64, 1, 14).contains("Greet the friends by name"));
        // Revelation 12:18 was the first clause of 13:1, standing alone.
        assert_eq!(books[65].chapters[11].len(), 17);
        assert!(at(66, 13, 1).starts_with("And I stood upon the sand of the sea, and saw a beast"));
    }
}
