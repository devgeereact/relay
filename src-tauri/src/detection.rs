//! Verse/content detection: direct match, semantic match, context memory.
//!
//! Single responsibility: given a rolling transcript window, return zero or
//! more candidate verse detections with a confidence score and method
//! ("direct" or "semantic"). Does NOT decide what to do with a detection —
//! that's router.rs. Does NOT touch SQLite — it returns references; the caller
//! resolves them to a verse_id via db.rs. Kept DB- and IO-free so the whole
//! parser is unit-testable. See docs/SPEC.md §4 and PROMPT.md Phase 5/9.
//!
//! Phase 5: direct pattern match — spoken and written references
//! (`John 3:16`, `John three sixteen`, `Romans chapter eight verse twenty-eight`)
//! against a multilingual-ready book-alias table, with tolerance for common
//! ASR homophones ("free" → three). Semantic match + context memory are Phase 9.

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// How a candidate was detected. This is NOT cosmetic metadata — the router
/// gates on it (see `router::Router::decide`), because confidences from these
/// three sources live on *incomparable scales* and a single scalar threshold
/// cannot safely gate all of them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMethod {
    /// A spoken reference the parser actually heard ("John three sixteen").
    /// Confidence is a real parse confidence. May auto-fire.
    Direct,
    /// A TF-IDF paraphrase match. Its "confidence" is a raw cosine similarity —
    /// a distance in an arbitrary vector space, NOT a probability. May never
    /// auto-fire; see `router.rs`.
    Semantic,
    /// A reference that parsed but is genuinely ambiguous ("Revelation 22"
    /// → 22:1 or 2:2). Confidence is a hardcoded placeholder, not measured.
    /// May never auto-fire.
    Ambiguous,
}

impl DetectionMethod {
    /// Whether a candidate detected this way is ever allowed onto the
    /// congregation's screen without a human confirming it first.
    ///
    /// Only `Direct` is. `Semantic` and `Ambiguous` carry confidences that are
    /// not calibrated probabilities, so no threshold on them is meaningful —
    /// gating them by number would be gating them by noise.
    pub fn may_auto_fire(&self) -> bool {
        matches!(self, DetectionMethod::Direct)
    }

    /// The value written to `detections.method`, which is constrained to
    /// `('direct','semantic')` (docs/data/schema.sql).
    ///
    /// `Ambiguous` persists as `direct` — and that is honest, not a fudge: the
    /// reference genuinely *was* parsed from spoken words ("Revelation 22"). What
    /// is ambiguous is *which verse* it resolves to, not how it was found. The
    /// routing distinction (never auto-fire) is enforced in the router, which is
    /// where it belongs; it isn't a property of the historical record.
    pub fn db_method(&self) -> &'static str {
        match self {
            DetectionMethod::Direct | DetectionMethod::Ambiguous => "direct",
            DetectionMethod::Semantic => "semantic",
        }
    }
}

/// A resolved scripture reference (canonical book name as stored in the DB).
#[derive(Debug, Clone, PartialEq)]
pub struct VerseRef {
    pub book: String,
    pub chapter: i64,
    pub verse: i64,
}

impl VerseRef {
    #[cfg(test)]
    fn reference_book_chapter_verse(&self) -> String {
        format!("{} {}:{}", self.book, self.chapter, self.verse)
    }
}

/// A candidate detection found in a transcript span.
///
/// `reference` is the anchor — the first verse to display. A multi-verse passage
/// is described by `verse_end` (explicit range, e.g. "John 3:16-18") or
/// `whole_chapter` ("Psalm 23", no verse). The caller fires the anchor verse and
/// stages the rest so "next" walks the passage (see ContextMemory). Keeping the
/// output single-verse means the template engine never special-cases passages.
#[derive(Debug, Clone)]
pub struct RefMatch {
    pub reference: VerseRef,
    /// Inclusive end verse of an explicit range; None for a single verse or a
    /// whole-chapter reference (whose end is resolved from the corpus at fire).
    pub verse_end: Option<i64>,
    /// True when only a book + chapter was spoken ("Psalm 23") — display verse 1
    /// and stage the whole chapter.
    pub whole_chapter: bool,
    pub confidence: f32,
    /// Always `Direct` — `detect_direct` is, by definition, the direct matcher.
    /// Kept because a `RefMatch` without its provenance is a footgun waiting for
    /// the day a second matcher produces one, and the routing gate keys on method
    /// (see `router::decide`). Callers state the method themselves rather than
    /// trusting this field, which is why nothing reads it today.
    #[allow(dead_code)]
    pub method: DetectionMethod,
    /// The exact span of transcript this reference was parsed from.
    ///
    /// This reaches the operator console (`DetectionEvent::matched_text`). Showing
    /// *what words* triggered a match is the clearest possible explanation of an AI
    /// decision: an operator can tell in a glance whether Relay heard "John three
    /// sixteen" or misheard "gone free sixty".
    pub matched_text: String,
    /// This match ran to the LAST WORD of the text it was parsed from — nothing
    /// followed it.
    ///
    /// Meaningless on its own; it matters only for a PARTIAL transcript, where the
    /// text is still growing. There, ending at the tail means the next word may
    /// still be part of this reference, so the reading is provisional. See
    /// `main::emit_detections`, which is where the transcript's finality is known.
    pub at_tail: bool,
}

impl RefMatch {
    /// True when this reading exists only because the transcript was CUT OFF, and
    /// so describes the window boundary rather than anything anyone said.
    ///
    /// "…turn to John chapter 3" is a complete, well-formed whole-chapter reference
    /// worth 0.88 — and it is also exactly what "John chapter 3 verse 16" looks like
    /// one second before the number arrives. The STT window is re-decoded about once
    /// a second and detection runs on every partial (DECISIONS.md), so whether a
    /// citation is seen whole or half depends only on where the boundary lands.
    /// Measured via `stt::bench::engine_shootout`, that coin toss put John 3:1 on the
    /// wall ahead of John 3:16.
    ///
    /// Narrow on purpose — a whole-chapter reading, with nothing after it, in text
    /// that can still grow. A complete "John 3:16" at the tail is NOT provisional and
    /// still fires instantly, so this costs no latency on the path that matters.
    /// Nothing is lost, either: the next partial carries the number, and a preacher
    /// who really did mean the chapter gets it when the utterance closes.
    ///
    /// Lives here, and is used by both `main::emit_detections` and
    /// `stt::bench::engine_shootout`, so the bench cannot score a policy the live
    /// path does not have.
    pub fn is_provisional(&self, is_final: bool) -> bool {
        self.whole_chapter && self.at_tail && !is_final
    }
}

/// The 66 canonical books in standard Protestant order. This is the source of
/// truth for both the direct-match alias table AND the full-Bible import
/// (db.rs maps the KJV JSON's books to these names by index), so a detected
/// reference and a stored verse always agree on spelling.
pub const CANONICAL_BOOKS: &[&str] = &[
    "Genesis",
    "Exodus",
    "Leviticus",
    "Numbers",
    "Deuteronomy",
    "Joshua",
    "Judges",
    "Ruth",
    "1 Samuel",
    "2 Samuel",
    "1 Kings",
    "2 Kings",
    "1 Chronicles",
    "2 Chronicles",
    "Ezra",
    "Nehemiah",
    "Esther",
    "Job",
    "Psalms",
    "Proverbs",
    "Ecclesiastes",
    "Song of Solomon",
    "Isaiah",
    "Jeremiah",
    "Lamentations",
    "Ezekiel",
    "Daniel",
    "Hosea",
    "Joel",
    "Amos",
    "Obadiah",
    "Jonah",
    "Micah",
    "Nahum",
    "Habakkuk",
    "Zephaniah",
    "Haggai",
    "Zechariah",
    "Malachi",
    "Matthew",
    "Mark",
    "Luke",
    "John",
    "Acts",
    "Romans",
    "1 Corinthians",
    "2 Corinthians",
    "Galatians",
    "Ephesians",
    "Philippians",
    "Colossians",
    "1 Thessalonians",
    "2 Thessalonians",
    "1 Timothy",
    "2 Timothy",
    "Titus",
    "Philemon",
    "Hebrews",
    "James",
    "1 Peter",
    "2 Peter",
    "1 John",
    "2 John",
    "3 John",
    "Jude",
    "Revelation",
];

/// Books with a single chapter — referenced by bare verse ("Jude 4" = Jude
/// 1:4, "Philemon verse 6" = Philemon 1:6). For these the first (or only)
/// number is the verse and the chapter defaults to 1.
const SINGLE_CHAPTER_BOOKS: &[&str] = &["Obadiah", "Philemon", "2 John", "3 John", "Jude"];

fn is_single_chapter(book: &str) -> bool {
    SINGLE_CHAPTER_BOOKS.contains(&book)
}

/// Verses per chapter, per book — indexed by `CANONICAL_BOOKS` position.
///
/// This is SHAPE, not scripture: 1189 chapter lengths, the same facts in every
/// translation. It is a `const` rather than a read of the bundled KJV so that
/// `detection` stays free of IO and of the 4 MB corpus, and a `#[cfg(test)]`
/// test parses `kjv.json` and asserts the two agree — so it cannot silently
/// drift from the Bible actually shipped.
///
/// It exists for `split_run_into_chapter_verse`, which cannot work without it:
/// deciding that "663" is 6:63 and not 66:3 requires knowing John has 21
/// chapters and that chapter 6 has at least 63 verses.
#[rustfmt::skip]
const VERSES_PER_CHAPTER: &[&[u8]] = &[
    &[31,25,24,26,32,22,24,22,29,32,32,20,18,24,21,16,27,33,38,18,34,24,20,67,34,35,46,22,35,43,55,32,20,31,29,43,36,30,23,23,57,38,34,34,28,34,31,22,33,26],
    &[22,25,22,31,23,30,25,32,35,29,10,51,22,31,27,36,16,27,25,26,36,31,33,18,40,37,21,43,46,38,18,35,23,35,35,38,29,31,43,38],
    &[17,16,17,35,19,30,38,36,24,20,47,8,59,57,33,34,16,30,37,27,24,33,44,23,55,46,34],
    &[54,34,51,49,31,27,89,26,23,36,35,16,33,45,41,50,13,32,22,29,35,41,30,25,18,65,23,31,40,16,54,42,56,29,34,13],
    &[46,37,29,49,33,25,26,20,29,22,32,32,18,29,23,22,20,22,21,20,23,30,25,22,19,19,26,68,29,20,30,52,29,12],
    &[18,24,17,24,15,27,26,35,27,43,23,24,33,15,63,10,18,28,51,9,45,34,16,33],
    &[36,23,31,24,31,40,25,35,57,18,40,15,25,20,20,31,13,31,30,48,25],
    &[22,23,18,22],
    &[28,36,21,22,12,21,17,22,27,27,15,25,23,52,35,23,58,30,24,43,15,23,29,22,44,25,12,25,11,31,13],
    &[27,32,39,12,25,23,29,18,13,19,27,31,39,33,37,23,29,33,43,26,22,51,39,25],
    &[53,46,28,34,18,38,51,66,28,29,43,33,34,31,34,34,24,46,21,43,29,54],
    &[18,25,27,44,27,33,20,29,37,36,21,21,25,29,38,20,41,37,37,21,26,20,37,20,30],
    &[54,55,24,43,26,81,40,40,44,14,47,40,14,17,29,43,27,17,19,8,30,19,32,31,31,32,34,21,30],
    &[17,18,17,22,14,42,22,18,31,19,23,16,22,15,19,14,19,34,11,37,20,12,21,27,28,23,9,27,36,27,21,33,25,33,27,23],
    &[11,70,13,24,17,22,28,36,15,44],
    &[11,20,32,23,19,19,73,18,38,39,36,47,31],
    &[22,23,15,17,14,14,10,17,32,3],
    &[22,13,26,21,27,30,21,22,35,22,20,25,28,22,35,22,16,21,29,29,34,30,17,25,6,14,23,28,25,31,40,22,33,37,16,33,24,41,30,24,34,17],
    &[6,12,8,8,12,10,17,9,20,18,7,8,6,7,5,11,15,50,14,9,13,31,6,10,22,12,14,9,11,12,24,11,22,22,28,12,40,22,13,17,13,11,5,26,17,11,9,14,20,23,19,9,6,7,23,13,11,11,17,12,8,12,11,10,13,20,7,35,36,5,24,20,28,23,10,12,20,72,13,19,16,8,18,12,13,17,7,18,52,17,16,15,5,23,11,13,12,9,9,5,8,28,22,35,45,48,43,13,31,7,10,10,9,8,18,19,2,29,176,7,8,9,4,8,5,6,5,6,8,8,3,18,3,3,21,26,9,8,24,13,10,7,12,15,21,10,20,14,9,6],
    &[33,22,35,27,23,35,27,36,18,32,31,28,25,35,33,33,28,24,29,30,31,29,35,34,28,28,27,28,27,33,31],
    &[18,26,22,16,20,12,29,17,18,20,10,14],
    &[17,17,11,16,16,13,13,14],
    &[31,22,26,6,30,13,25,22,21,34,16,6,22,32,9,14,14,7,25,6,17,25,18,23,12,21,13,29,24,33,9,20,24,17,10,22,38,22,8,31,29,25,28,28,25,13,15,22,26,11,23,15,12,17,13,12,21,14,21,22,11,12,19,12,25,24],
    &[19,37,25,31,31,30,34,22,26,25,23,17,27,22,21,21,27,23,15,18,14,30,40,10,38,24,22,17,32,24,40,44,26,22,19,32,21,28,18,16,18,22,13,30,5,28,7,47,39,46,64,34],
    &[22,22,66,22,22],
    &[28,10,27,17,17,14,27,18,11,22,25,28,23,23,8,63,24,32,14,49,32,31,49,27,17,21,36,26,21,26,18,32,33,31,15,38,28,23,29,49,26,20,27,31,25,24,23,35],
    &[21,49,30,37,31,28,28,27,27,21,45,13],
    &[11,23,5,19,15,11,16,14,17,15,12,14,16,9],
    &[20,32,21],
    &[15,16,15,13,27,14,17,14,15],
    &[21],
    &[17,10,10,11],
    &[16,13,12,13,15,16,20],
    &[15,13,19],
    &[17,20,19],
    &[18,15,20],
    &[15,23],
    &[21,13,10,14,11,15,14,23,17,12,17,14,9,21],
    &[14,17,18,6],
    &[25,22,17,25,48,34,29,34,38,42,30,50,58,36,39,28,27,35,30,34,46,45,39,51,46,74,66,20],
    &[45,28,35,40,43,56,36,37,50,52,33,44,37,72,47,20],
    &[80,52,38,44,39,49,50,56,62,42,54,59,35,35,32,31,37,43,48,47,38,71,56,53],
    &[51,25,36,54,47,71,53,59,41,42,57,50,38,31,27,33,26,40,42,31,25],
    &[26,47,26,37,42,15,60,40,43,48,30,25,52,28,41,40,34,28,41,38,40,30,35,27,27,32,44,31],
    &[32,29,31,25,21,23,25,39,33,21,36,21,14,23,33,27],
    &[31,16,23,21,13,20,40,13,27,33,34,31,13,40,58,24],
    &[24,17,18,18,21,18,16,24,15,18,33,21,14],
    &[24,21,29,31,26,18],
    &[23,22,21,32,33,24],
    &[30,30,21,23],
    &[29,23,25,18],
    &[10,20,13,18,28],
    &[12,17,18],
    &[20,15,16,16,25,21],
    &[18,26,17,22],
    &[16,15,15],
    &[25],
    &[14,18,19,16,14,20,28,13,28,39,40,29,25],
    &[27,26,18,17,20],
    &[25,25,22,19,14],
    &[21,22,18],
    &[10,29,24,21,21],
    &[13],
    &[15],
    &[25],
    &[20,29,22,11,14,17,17,13,21,11,19,18,18,20,8,21,18,24,21,15,27,21],
];

/// How many chapters a book has (0 if the book is unknown).
fn chapter_count(book: &str) -> usize {
    CANONICAL_BOOKS
        .iter()
        .position(|b| *b == book)
        .and_then(|i| VERSES_PER_CHAPTER.get(i))
        .map(|c| c.len())
        .unwrap_or(0)
}

/// How many verses are in `book` chapter `chapter` (0 if either is out of range).
fn verse_count(book: &str, chapter: i64) -> usize {
    if chapter < 1 {
        return 0;
    }
    CANONICAL_BOOKS
        .iter()
        .position(|b| *b == book)
        .and_then(|i| VERSES_PER_CHAPTER.get(i))
        .and_then(|c| c.get((chapter - 1) as usize))
        .map(|n| *n as usize)
        .unwrap_or(0)
}

/// Repair a digit run that whisper ran together: "663" → 6:63.
///
/// ── The mishearing this exists for ──────────────────────────────────────────
///
/// A preacher says "John six sixty-three". Whisper does not write `6:63`; on a
/// fast or accented delivery it writes the digits it heard, joined: `663`. The
/// parser then read the whole run as a CHAPTER, and a congregation was offered
/// `John 663:1`. Observed live on 2026-07-26, five times in one service, and
/// every one of them was a verse the operator then fired BY HAND:
///
///     "john 663"     → John 663:1        was John 6:63
///     "hebrews 416"  → Hebrews 416:1     was Hebrews 4:16
///     "mark 1124"    → Mark 1124:1       was Mark 11:24
///     "romans 828"   → Romans 828:1      was Romans 8:28
///     "john 1623"    → John 1623:1       was John 16:23
///
/// Relay had heard the reference perfectly and then mangled the number.
///
/// ── Why this is safe, and where it stops ────────────────────────────────────
///
/// **A run that IS a valid chapter of this book is never touched.** "Psalm 23"
/// is a whole-chapter reference and must stay one — splitting it into 2:3 would
/// be a new bug of exactly the kind being fixed. The repair only runs where
/// reading the run as a chapter is IMPOSSIBLE, which is a fact about the book,
/// not a guess about the speaker.
///
/// **An ambiguous split is refused**, the same rule `fuzzy_book` follows: if two
/// different chapter:verse pairs are both real, there is no evidence to choose,
/// and guessing is the failure mode. Measured over every book and every 3- and
/// 4-digit run, 95% of repairable runs have exactly one valid split.
fn split_run_into_chapter_verse(book: &str, run: i64) -> Option<(i64, i64)> {
    let chapters = chapter_count(book);
    if chapters == 0 {
        return None;
    }
    // A real chapter of this book. Not a mishearing — leave it entirely alone.
    if run >= 1 && (run as usize) <= chapters {
        return None;
    }
    let digits = run.to_string();
    let mut found: Option<(i64, i64)> = None;
    for i in 1..digits.len() {
        let (c, v) = digits.split_at(i);
        // A leading zero is not how anyone says or writes a number: "1005" is
        // not 100:5 by way of chapter 1 verse 005.
        if c.starts_with('0') || v.starts_with('0') {
            continue;
        }
        let (Ok(c), Ok(v)) = (c.parse::<i64>(), v.parse::<i64>()) else {
            continue;
        };
        if c < 1 || c as usize > chapters || v < 1 || v as usize > verse_count(book, c) {
            continue;
        }
        if found.is_some() {
            return None; // ambiguous — refuse, do not guess
        }
        found = Some((c, v));
    }
    found
}

/// Alias → canonical-book map, built once. Covers the lowercase full name, the
/// spoken/written forms of numbered books ("first"/"i"/"1", "1john"), plus a few
/// common variants and ASR mishears. Multilingual-ready: add rows per language.
static ALIAS_MAP: OnceLock<HashMap<String, &'static str>> = OnceLock::new();

fn alias_map() -> &'static HashMap<String, &'static str> {
    ALIAS_MAP.get_or_init(|| {
        let mut m: HashMap<String, &'static str> = HashMap::new();
        for &book in CANONICAL_BOOKS {
            let lower = book.to_lowercase();
            m.insert(lower.clone(), book);
            // Numbered books: "1 John" → "first john" / "i john" / "1john".
            for (digit, words) in [
                ("1", ["first", "i"]),
                ("2", ["second", "ii"]),
                ("3", ["third", "iii"]),
            ] {
                if let Some(rest) = lower.strip_prefix(&format!("{digit} ")) {
                    for w in words {
                        m.insert(format!("{w} {rest}"), book);
                    }
                    m.insert(format!("{digit}{rest}"), book); // "1john"
                }
            }
        }
        // Common written abbreviations for fast manual-override typing
        // ("ps 23 1", "rom 8 1", "1 jn 3 1"). Single-token ones here; numbered
        // ones ("1 jn") are added in the loop below.
        let abbr: &[(&str, &str)] = &[
            ("gen", "Genesis"),
            ("exo", "Exodus"),
            ("ex", "Exodus"),
            ("lev", "Leviticus"),
            ("lv", "Leviticus"),
            ("num", "Numbers"),
            ("nm", "Numbers"),
            ("deut", "Deuteronomy"),
            ("deu", "Deuteronomy"),
            ("dt", "Deuteronomy"),
            ("josh", "Joshua"),
            ("jos", "Joshua"),
            ("judg", "Judges"),
            ("jdg", "Judges"),
            ("rth", "Ruth"),
            ("ps", "Psalms"),
            ("psa", "Psalms"),
            ("pss", "Psalms"),
            ("prov", "Proverbs"),
            ("prv", "Proverbs"),
            ("pro", "Proverbs"),
            ("eccl", "Ecclesiastes"),
            ("ecc", "Ecclesiastes"),
            ("song", "Song of Solomon"),
            ("sos", "Song of Solomon"),
            ("isa", "Isaiah"),
            ("jer", "Jeremiah"),
            ("jr", "Jeremiah"),
            ("lam", "Lamentations"),
            ("ezek", "Ezekiel"),
            ("eze", "Ezekiel"),
            ("ezk", "Ezekiel"),
            ("dan", "Daniel"),
            ("dn", "Daniel"),
            ("hos", "Hosea"),
            ("jl", "Joel"),
            ("amo", "Amos"),
            ("obad", "Obadiah"),
            ("oba", "Obadiah"),
            ("jnh", "Jonah"),
            ("mic", "Micah"),
            ("nah", "Nahum"),
            ("hab", "Habakkuk"),
            ("zeph", "Zephaniah"),
            ("zep", "Zephaniah"),
            ("hag", "Haggai"),
            ("zech", "Zechariah"),
            ("zec", "Zechariah"),
            ("mal", "Malachi"),
            ("matt", "Matthew"),
            ("mat", "Matthew"),
            ("mt", "Matthew"),
            ("mrk", "Mark"),
            ("mk", "Mark"),
            ("luk", "Luke"),
            ("lk", "Luke"),
            ("jhn", "John"),
            ("jn", "John"),
            ("acts", "Acts"),
            ("ac", "Acts"),
            ("rom", "Romans"),
            ("rm", "Romans"),
            ("gal", "Galatians"),
            ("ga", "Galatians"),
            ("eph", "Ephesians"),
            ("phil", "Philippians"),
            ("php", "Philippians"),
            ("col", "Colossians"),
            ("tit", "Titus"),
            ("phm", "Philemon"),
            ("heb", "Hebrews"),
            ("jas", "James"),
            ("jde", "Jude"),
            ("rev", "Revelation"),
            ("rv", "Revelation"),
        ];
        for (a, canon) in abbr {
            m.insert((*a).into(), canon);
        }
        // Numbered-book abbreviations: "1 sa"/"1sa" → 1 Samuel, "1 jn"/"1jn" →
        // 1 John, etc. Two-letter stems keep them short to type.
        let numbered: &[(&str, &str)] = &[
            ("sa", "Samuel"),
            ("sm", "Samuel"),
            ("ki", "Kings"),
            ("kg", "Kings"),
            ("ch", "Chronicles"),
            ("chr", "Chronicles"),
            ("co", "Corinthians"),
            ("cor", "Corinthians"),
            ("th", "Thessalonians"),
            ("thess", "Thessalonians"),
            ("ti", "Timothy"),
            ("tim", "Timothy"),
            ("pe", "Peter"),
            ("pet", "Peter"),
            ("jn", "John"),
            ("jo", "John"),
        ];
        for d in ['1', '2', '3'] {
            for (stem, word) in numbered {
                let canon = CANONICAL_BOOKS
                    .iter()
                    .find(|b| b.starts_with(d) && b.ends_with(word))
                    .copied();
                if let Some(canon) = canon {
                    m.insert(format!("{d} {stem}"), canon); // "1 jn"
                    m.insert(format!("{d}{stem}"), canon); // "1jn"
                }
            }
        }

        // Extra spoken variants + ASR/accent mishears. The silent "P" in Psalms
        // is frequently dropped by ASR on African-accented speech ("sam",
        // "salm"), so those map to Psalms.
        m.insert("psalm".into(), "Psalms");
        m.insert("palms".into(), "Psalms");
        m.insert("sam".into(), "Psalms");
        m.insert("salm".into(), "Psalms");
        m.insert("salms".into(), "Psalms");
        m.insert("sams".into(), "Psalms");
        m.insert("jon".into(), "John");
        m.insert("mathew".into(), "Matthew");
        m.insert("mathews".into(), "Matthew");
        m.insert("proverb".into(), "Proverbs");
        m.insert("song of songs".into(), "Song of Solomon");
        m.insert("canticles".into(), "Song of Solomon");
        m.insert("revelations".into(), "Revelation");
        // Common spelling/accent variants whisper emits.
        m.insert("collosians".into(), "Colossians");
        m.insert("colosians".into(), "Colossians");
        m.insert("phillipians".into(), "Philippians");
        m.insert("philipians".into(), "Philippians");
        m.insert("efesians".into(), "Ephesians");
        m.insert("ephesus".into(), "Ephesians");
        m.insert("deutronomy".into(), "Deuteronomy");
        m.insert("ecclesiastis".into(), "Ecclesiastes");
        m.insert("thessalonians".into(), "1 Thessalonians"); // bare → most-common
        m.insert("galatia".into(), "Galatians");

        // ── Tier-1 languages: Yorùbá, Kiswahili, Hausa ──────────────────────
        //
        // THE thing that was missing. Relay's stated differentiator is
        // African-language speech, and until now the detector spoke only English:
        // a preacher could say "Jòhánù orí kẹta" with a perfect Yorùbá model
        // behind them and Relay would detect NOTHING, because the alias table had
        // no idea what "Jòhánù" was. Fine-tuning the acoustic model would not have
        // fixed that by a single verse — the moat was blocked on this table, not
        // on the model.
        //
        // Loaded from data, not hardcoded here: see the _readme in the JSON.
        for (alias, canonical) in language_aliases() {
            m.insert(alias, canonical);
        }
        m
    })
}

/// The book names Relay knows for a language, for biasing the STT decoder.
///
/// `lang` is a Whisper language code ("yo", "sw", "ha"); anything else (including
/// None/auto) yields the English canon.
///
/// ALWAYS includes English alongside the local names, because code-switching is
/// the normal case for this market, not an edge case (CLAUDE.md): a Yorùbá sermon
/// routinely names the book in Yorùbá and the chapter and verse in English.
pub fn bias_vocabulary(lang: Option<&str>) -> Vec<String> {
    let mut out: Vec<String> = CANONICAL_BOOKS.iter().map(|b| b.to_string()).collect();
    let Some(lang) = lang else { return out };
    const RAW: &str = include_str!("../data/book_aliases.json");
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(RAW) else {
        return out;
    };
    if let Some(books) = doc.get(lang).and_then(|v| v.as_object()) {
        for (english, names) in books {
            if english.starts_with('_') {
                continue;
            }
            // Only the FIRST spelling — the properly-accented one. The prompt is a
            // hint to the decoder, not a lookup table, and stuffing it with every
            // ASCII fallback dilutes the signal.
            if let Some(first) = names
                .as_array()
                .and_then(|a| a.first())
                .and_then(|v| v.as_str())
            {
                out.push(first.to_string());
            }
        }
    }
    out
}

/// Book names in Relay's tier-1 languages, from `data/book_aliases.json`.
///
/// Data rather than code on purpose. The maintainer does not speak all three of
/// these languages fluently, and a WRONG alias does not fail safely — it puts the
/// wrong scripture on a wall. Keeping the names in JSON lets a native speaker fix
/// them in a one-line pull request without touching Rust or knowing what a
/// HashMap is. That is the only path by which this table ever becomes trustworthy.
///
/// Baked into the binary (`include_str!`), so it stays fully offline.
fn language_aliases() -> Vec<(String, &'static str)> {
    const RAW: &str = include_str!("../data/book_aliases.json");
    let Ok(doc) = serde_json::from_str::<serde_json::Value>(RAW) else {
        eprintln!("detection: book_aliases.json is not valid JSON — tier-1 languages disabled");
        return Vec::new();
    };
    let Some(langs) = doc.as_object() else {
        return Vec::new();
    };

    let mut out = Vec::new();
    for (lang, books) in langs {
        if lang.starts_with('_') {
            continue; // _readme
        }
        let Some(books) = books.as_object() else {
            continue;
        };
        for (english, names) in books {
            if english.starts_with('_') {
                continue; // _language, _complete, _todo
            }
            // Key against the canonical spelling, so a typo in the data file is a
            // no-op rather than a phantom book that can never resolve to a verse.
            let Some(canonical) = CANONICAL_BOOKS.iter().find(|b| *b == english).copied() else {
                eprintln!("detection: book_aliases.json has unknown book {english:?} — ignored");
                continue;
            };
            for n in names.as_array().into_iter().flatten() {
                if let Some(n) = n.as_str() {
                    // normalize() folds the tone marks and dots-below, so the
                    // table is keyed on exactly what a transcript will produce.
                    let key = normalize(n);
                    if !key.is_empty() {
                        out.push((key, canonical));
                    }
                }
            }
        }
    }
    out
}

/// Find all direct scripture references in `text`. Returns them left-to-right.
pub fn detect_direct(text: &str) -> Vec<RefMatch> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if let Some((canonical, book_end, fuzzy)) = match_book(&tokens, i) {
            if let Some((mut m, next)) = parse_reference(&tokens, book_end, canonical, i, fuzzy) {
                // Nothing followed this reference in the text it came from. On a
                // partial transcript that means the next word might still belong to
                // it — see `RefMatch::at_tail`.
                m.at_tail = next >= tokens.len();
                out.push(m);
                i = next;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Fold a character down to its plain-ASCII skeleton.
///
/// Yorùbá and Hausa orthography carries marks that ASR reproduces unreliably:
/// tone marks (`ò á ń`), dots-below (`ẹ ọ ṣ`), and hooked consonants (`ɓ ɗ ƙ`).
/// Whisper will emit `Jòhánù`, `Johánù` or `Johanu` for the same audio depending
/// on the recording. If those are three different tokens, the alias table matches
/// none of them and Relay detects nothing — which is precisely the state it was
/// in before the multilingual table existed.
///
/// So all three fold to `johanu` and match once.
///
/// Deliberately lossy, and that is fine: this folds text for MATCHING, never for
/// display. What the congregation sees is always the canonical corpus text.
fn fold_char(c: char) -> Option<char> {
    match c {
        // Hausa hooked consonants are distinct letters, not accented ones, so NFD
        // will not decompose them. They have to be mapped by hand.
        'ɓ' | 'Ɓ' => Some('b'),
        'ɗ' | 'Ɗ' => Some('d'),
        'ƙ' | 'Ƙ' => Some('k'),
        'ƴ' | 'Ƴ' => Some('y'),
        // Combining marks left behind by NFD — tone marks, dots-below. Drop them.
        c if ('\u{0300}'..='\u{036F}').contains(&c) => None,
        c => Some(c),
    }
}

/// Lowercase, fold diacritics, strip punctuation except the digit-pairing colon,
/// split hyphens ("twenty-eight" → two tokens), collapse whitespace.
/// `pub(crate)` so the WER bench can fold a reference transcript EXACTLY the way the
/// detector folds a hypothesis. A scorer that normalises the two sides differently is
/// not measuring anything.
pub(crate) fn normalize(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let mut s = String::with_capacity(text.len());
    // NFD first, so `ọ` becomes `o` + combining-dot-below and the mark can be
    // dropped generically instead of via a 200-row lookup table.
    for ch in text.nfd().filter_map(fold_char) {
        match ch {
            c if c.is_alphanumeric() => s.extend(c.to_lowercase()),
            ':' => s.push(':'),
            // Apostrophes are DROPPED (not split) so ASR possessives stay one
            // token: "Sam's" → "sams" (→ Psalms). This also folds the Hausa
            // glottal in "Ru'ya" → "ruya".
            '\'' | '\u{2019}' | '\u{02BC}' => {}
            _ => s.push(' '), // hyphen, comma, period, etc. → separator
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Match the longest book alias starting at `start`. Returns (canonical, index
/// just past the alias, whether the match was FUZZY rather than exact).
fn match_book(tokens: &[&str], start: usize) -> Option<(&'static str, usize, bool)> {
    // Scan longest-first (up to 3 tokens) so multi-word books ("song of
    // solomon") and numbered forms ("first corinthians") match before a shorter
    // prefix would.
    for len in (1..=3).rev() {
        if start + len > tokens.len() {
            continue;
        }
        let candidate = tokens[start..start + len].join(" ");
        if let Some(&canonical) = alias_map().get(&candidate) {
            return Some((canonical, start + len, false));
        }
    }
    // Nothing matched exactly. Try to REPAIR a misheard book name.
    fuzzy_book(tokens, start).map(|c| (c, start + 1, true))
}

/// Words an ordinary sermon says constantly, which must never be repaired into a
/// book name however close they look.
///
/// This list is the difference between a helpful repair and putting the wrong
/// scripture on a wall. "among" is two edits from "amos"; "same" is two from
/// "james"; "act" and "acts" differ by one; "mark", "job" and "will" are all
/// ordinary English AND book names, so an approximate match on them is never a
/// repair — it is a coincidence.
const NEVER_FUZZY: &[&str] = &[
    "a", "am", "among", "amongst", "an", "and", "are", "as", "at", "be", "been", "but", "by",
    "call", "called", "came", "come", "did", "do", "does", "done", "for", "from", "gone", "good",
    "had", "has", "have", "he", "her", "here", "him", "his", "how", "i", "if", "in", "is", "it",
    "its", "just", "know", "let", "like", "look", "made", "make", "man", "many", "may", "me",
    "more", "most", "much", "must", "my", "name", "no", "not", "now", "of", "on", "one", "only",
    "or", "our", "out", "over", "own", "said", "same", "say", "says", "see", "shall", "she",
    "should", "so", "some", "son", "such", "take", "than", "that", "the", "their", "them", "then",
    "there", "these", "they", "thing", "this", "those", "thou", "time", "to", "up", "upon", "us",
    "very", "was", "way", "we", "well", "went", "were", "what", "when", "where", "which", "while",
    "who", "why", "will", "with", "word", "work", "would", "ye", "yes", "you", "your",
];

/// Repair a single misheard book token — but ONLY where a reference could
/// actually be.
///
/// ── Why this is gated on a following number ────────────────────────────────
///
/// Relay's whole promise is that a DIRECT match may go on a screen without a
/// human confirming it (CLAUDE.md §10). Approximate book matching is therefore
/// the single most dangerous thing in this file: get it wrong and the wrong
/// scripture is in front of a congregation, confidently.
///
/// So the repair may only run where the sentence is already reference-SHAPED —
/// the very next token is a chapter number or a chapter word. "sam" alone stays
/// an ordinary word; "sam twenty three" is a reference. That one condition
/// removes almost all of the risk, and as a side effect removes almost all of
/// the cost: the scan runs a handful of times per sermon, not once per token.
///
/// It is still marked FUZZY, which costs confidence downstream, so a repaired
/// reference needs to be otherwise strong to reach the auto-fire line.
fn fuzzy_book(tokens: &[&str], start: usize) -> Option<&'static str> {
    let token = *tokens.get(start)?;
    // Too short to repair safely: at two characters everything is one edit from
    // everything else.
    if token.len() < 3 || NEVER_FUZZY.contains(&token) {
        return None;
    }
    // Reference-shaped context only. See above.
    let next = tokens.get(start + 1)?;
    let numeric = next.chars().all(|c| c.is_ascii_digit())
        || classify_num_word(next).is_some()
        || is_chapter_word(next);
    if !numeric {
        return None;
    }

    // Budget scales with length: one edit for a short name, two for a long one.
    let budget = if token.len() <= 5 { 1 } else { 2 };

    let mut best: Option<(usize, &'static str)> = None;
    let mut second = usize::MAX;
    for (alias, &canonical) in alias_map().iter() {
        // Single-word aliases only; a multi-word mishear is a different problem.
        if alias.contains(' ') {
            continue;
        }
        // `continue`, NOT `?`. With `?` the whole search abandoned itself on the
        // first alias that happened to be far away — which is nearly always the
        // first one — so the repair never ran at all.
        let Some(d) = edit_distance_within(token, alias, budget) else {
            continue;
        };
        match best {
            Some((bd, _)) if d < bd => {
                second = bd;
                best = Some((d, canonical));
            }
            Some((bd, bc)) if d == bd && bc != canonical => second = second.min(d),
            None => best = Some((d, canonical)),
            _ => {}
        }
    }
    let (d, canonical) = best?;
    // AMBIGUOUS REPAIRS ARE REFUSED. If two different books are equally close,
    // there is no evidence to choose between them, and guessing is exactly the
    // failure mode this whole function is trying not to be.
    if second == d {
        return None;
    }
    let _ = d;
    Some(canonical)
}

/// Levenshtein distance, abandoning early once it exceeds `budget`.
///
/// Returns `Some(distance)` when within budget, `None` when it cannot be — the
/// `?` at the call site then skips the candidate. Bounded so a 31k-alias scan
/// stays cheap.
fn edit_distance_within(a: &str, b: &str, budget: usize) -> Option<usize> {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a.len().abs_diff(b.len()) > budget {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        cur[0] = i;
        let mut row_min = cur[0];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
            row_min = row_min.min(cur[j]);
        }
        if row_min > budget {
            return None;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    let d = prev[b.len()];
    (d <= budget).then_some(d)
}

/// Parse a chapter:verse reference beginning at `idx` (just past the book).
/// `book_start` is the book's first token index, used for the matched span.
fn parse_reference(
    tokens: &[&str],
    idx: usize,
    canonical: &str,
    book_start: usize,
    fuzzy_book: bool,
) -> Option<(RefMatch, usize)> {
    let mut i = idx;
    let mut used_kw = false;
    // A REPAIRED book name is a weaker claim than one that matched exactly, and
    // it is charged for exactly like a repaired number is.
    let mut phonetic = fuzzy_book;

    // optional "chapter" — in English or a tier-1 language ("sura ya tatu").
    if let Some(t) = tokens.get(i) {
        if is_chapter_word(t) {
            used_kw = true;
            i += 1;
            i = skip_linkers(tokens, i); // "sura YA tatu"
        }
    }

    // Combined "3:16" token, with optional "-18" range end.
    if let Some((ch, vs, next)) = try_colon_pair(tokens, i) {
        let mut m = make_match(
            canonical, ch, vs, tokens, book_start, next, 0.96, used_kw, false,
        );
        let mut end_idx = next;
        if let Some((e, after)) = parse_range_end(tokens, next, vs) {
            m.verse_end = Some(e);
            end_idx = after;
        }
        return Some((m, end_idx));
    }

    // Single-chapter books: "Jude 4" / "Jude verse four" → Jude 1:4. A leading
    // "verse" keyword, or a lone number, means the number is the verse (chapter
    // 1). An explicit second number ("Jude 1 4") is still read as chapter:verse.
    if is_single_chapter(canonical) {
        let mut j = i;
        let mut used_v = false;
        while let Some(t) = tokens.get(j) {
            if is_verse_word(t) {
                used_v = true;
                j += 1;
            } else {
                break;
            }
        }
        if used_v {
            let (verse, after, ph) = parse_number(tokens, j)?;
            return Some((
                make_match(
                    canonical, 1, verse, tokens, book_start, after, 0.95, true, ph,
                ),
                after,
            ));
        }
        let (n1, after1, ph1) = parse_number(tokens, i)?;
        let mut k = after1;
        let mut kw2 = used_kw;
        // Same commitment as the general chapter path below: consuming a verse
        // marker here means a number is expected.
        let mut verse_marker = false;
        while let Some(t) = tokens.get(k) {
            if is_verse_word(t) || *t == ":" {
                verse_marker = true;
                if *t != ":" {
                    kw2 = true;
                }
                k += 1;
                k = skip_linkers(tokens, k); // "mstari WA kwanza"
            } else {
                break;
            }
        }
        if let Some((n2, after2, ph2)) = parse_number(tokens, k) {
            // Two numbers → chapter:verse.
            //
            // But BARE digits, with no "chapter"/"verse" keyword and no colon, are a
            // different animal from "Psalm 23 verse 1" and must not be trusted the
            // same way. That form exists for TYPED shorthand ("ps 23 1") — and typed
            // input goes through `manual_fire`, which bypasses the gate entirely, so
            // demoting it here costs the operator nothing.
            //
            // What it fixes is garbled speech. This is a real transcript, from a live
            // rehearsal:
            //
            //     "Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,"
            //
            // It used to score 0.92 and put Psalms 2:3 on the wall, unasked. Nobody
            // SAYS "Psalms two three" — they say "Psalms two verse three". So a bare
            // pair now suggests, and a human decides.
            let base = if kw2 { 0.92 } else { 0.45 };
            return Some((
                make_match(
                    canonical,
                    n1,
                    n2,
                    tokens,
                    book_start,
                    after2,
                    base,
                    kw2,
                    ph1 || ph2,
                ),
                after2,
            ));
        }
        // TRUNCATED MID-REFERENCE, single-chapter twin. "Jude chapter 1 verse" —
        // the marker was consumed and the number never came, so falling through to
        // the lone-number reading would answer with Jude 1:1 at 0.95, which is
        // HIGHER than the 0.88 the general path was handing out. Same defect, worse
        // number. See the guard on the chapter path below for the full reasoning.
        if verse_marker {
            return None;
        }

        // Lone number → verse, chapter 1, with optional range ("Jude 4-6").
        //
        // ── Without a keyword this is the single-chapter twin of the bare whole
        //    chapter below, and it fails the same way ──────────────────────────
        //
        // Every single-chapter book is ALSO an ordinary word or a name in English
        // preaching — Jude, Philemon, Obadiah, and the "John" inside 2 John and
        // 3 John. Followed by a small spoken number, ordinary speech parses as a
        // complete reference. From the live service of 2026-07-26, all auto-fired
        // to a congregation: Jude 1:1, Jude 1:2, 2 John 1:2, 2 John 1:3.
        //
        // So the keyword rule is the same one, applied consistently: "Jude VERSE
        // four" (0.95 above) and "Jude chapter 1 verse 4" state referential
        // intent and still fire. A bare "Jude four" asks a human.
        //
        // This costs more here than it does for a whole chapter, and that is worth
        // being honest about: for a one-chapter book the bare form IS the natural
        // complete reference, so a genuine "Jude four" now needs a click. The
        // trade is accepted because these are 5 books of 66 and rarely preached,
        // while the words themselves are constant in sermon speech — the false
        // positives are frequent and the true positives are not.
        let base = if used_kw { 0.9 } else { 0.45 };
        let mut m = make_match(
            canonical, 1, n1, tokens, book_start, after1, base, used_kw, ph1,
        );
        let mut end_idx = after1;
        if let Some((e, after)) = parse_range_end(tokens, after1, n1) {
            m.verse_end = Some(e);
            end_idx = after;
        }
        return Some((m, end_idx));
    }

    // Chapter number.
    let (chapter, after_ch, ph1) = parse_number(tokens, i)?;
    phonetic |= ph1;
    i = after_ch;
    let chapter_was_digit = tokens
        .get(after_ch - 1)
        .map(|t| t.parse::<i64>().is_ok())
        .unwrap_or(false);

    // Colon-combined right after chapter? (e.g. tokens were "3" ":" "16" — rare)
    // optional "verse" / "verses" / "vs" / "v" / ":" separators
    //
    // `verse_marker` records that the grammar COMMITTED to a verse number here. If
    // one never arrives, this is not a whole-chapter reference — it is a reference
    // cut off mid-sentence. See the truncation guard below.
    let mut verse_marker = false;
    while let Some(t) = tokens.get(i) {
        if is_verse_word(t) || *t == ":" {
            verse_marker = true;
            if *t != ":" {
                used_kw = true;
            }
            i += 1;
            i = skip_linkers(tokens, i); // "aya TA farko"
        } else {
            break;
        }
    }

    // Verse number — if absent, this is a whole-chapter reference ("Psalm 23"):
    // display verse 1, stage the chapter.
    //
    // ── WITHOUT the "chapter" keyword, this is the weakest shape in the file ──
    //
    // The same reasoning as the bare-pair demotion below, one step further. A bare
    // pair at least has two numbers that line up. This has ONE number after a book
    // name, and Relay answers it by putting verse 1 on a wall — a verse the
    // preacher never asked for specifically.
    //
    // In ordinary preaching that shape is far more often speech than reference.
    // From a live service, every one of these auto-fired to the congregation:
    //
    //     "Matthew, one of the twelve…"      → Matthew 1:1
    //     "…the Lord to the children…"       → John 2:1, 1 Samuel 2:1
    //     garbled window                      → Job 1:1, Job 11:1, Revelation 2:1
    //
    // And it actively DESTROYS good detections. The rolling window is decoded
    // about once a second, so one utterance is parsed repeatedly at varying
    // completeness. A preacher on Hebrews 4:2 produced, five seconds apart:
    //
    //     Hebrews 4:2  conf 0.55   ← correct, the whole reference was heard
    //     Hebrews 4:1  conf 0.83   ← only "Hebrews four" survived that pass
    //
    // The LESS complete parse scored higher and replaced the right verse on the
    // wall. A partial hearing of a reference must never outrank a full one.
    //
    // So a keyword-less whole chapter now asks a human (0.45, below the 0.50
    // default auto bar, above the 0.35 suggest bar) — the operator sees it in the
    // suggestion list and it is one click away. "Psalm CHAPTER 23" keeps its 0.88:
    // the keyword is proof of referential intent, and no one says it by accident.
    // A manual push is unaffected — it bypasses the gate entirely — and the
    // sensitivity dial still governs all of it.
    let Some((verse, after_vs, ph2)) = parse_number(tokens, i) else {
        // ── TRUNCATED MID-REFERENCE ─────────────────────────────────────────
        //
        // "…John chapter 3 verse" — the speaker said a verse number and the
        // TRANSCRIPT STOPPED BEFORE IT. There is no whole-chapter reading to fall
        // back to: a dangling verse marker is proof a verse was coming, which is
        // the exact opposite of the referential intent the keyword bonus below
        // rewards. Answering it with verse 1 invents a verse nobody asked for.
        //
        // This is not hypothetical, and it is not rare. Detection runs on every
        // PARTIAL hypothesis (DECISIONS.md), and the STT window is re-decoded
        // about once a second — so every reference anyone speaks is parsed at
        // least once in a state where the number has not arrived yet. Measured
        // through `stt::bench::engine_shootout`, one clip citing two verses
        // auto-fired John 3:1 and Romans 8:1 to the wall on the way to the right
        // answer. The congregation sees the wrong verse flash, then the right one.
        //
        // Worse, the marker was ACTIVELY PROMOTING the mistake. A bare "Romans 8"
        // scores 0.45 and asks a human — but the dangling "verse" set `used_kw`,
        // which bought the truncated parse 0.88 and a straight path to the screen.
        // The most incomplete reading of the sentence outranked every other.
        //
        // So the parse fails. `detect_direct`'s scanner advances a token and
        // carries on, the next second's window carries the whole reference, and
        // that one fires. This is the same principle the keyword-less demotion
        // below rests on, stated one step harder: A PARTIAL HEARING OF A REFERENCE
        // MUST NEVER OUTRANK A FULL ONE.
        if verse_marker {
            return None;
        }

        // Before treating this as a whole chapter: is it even a chapter of this
        // book? "john 663" is not John chapter 663 — John has 21 — it is whisper
        // running "six sixty-three" together. Repair it to 6:63 when exactly one
        // reading is real. See `split_run_into_chapter_verse`.
        if let Some((c, v)) = split_run_into_chapter_verse(canonical, chapter) {
            // A REPAIRED reference, so it is charged like one: `phonetic` costs
            // confidence downstream, and the run being unreadable as a chapter is
            // hard evidence the number was misheard.
            let m = make_match(
                canonical, c, v, tokens, book_start, after_ch, 0.83, used_kw, true,
            );
            return Some((m, after_ch));
        }
        let base = if used_kw { 0.88 } else { 0.45 };
        let mut m = make_match(
            canonical, chapter, 1, tokens, book_start, after_ch, base, false, phonetic,
        );
        m.whole_chapter = true;
        return Some((m, after_ch));
    };
    phonetic |= ph2;
    let verse_was_digit = tokens
        .get(after_vs - 1)
        .map(|t| t.parse::<i64>().is_ok())
        .unwrap_or(false);

    // Optional range end ("John 3:16-18", "Psalm 23 verses 1 to 6"). Resolved
    // BEFORE scoring, because whether a following number was absorbed as a range
    // end is exactly what decides if a *leftover* one is a garble signal (below).
    let range = parse_range_end(tokens, after_vs, verse);
    let end_idx = range.map_or(after_vs, |(_, after)| after);

    // BARE DIGITS with no "chapter"/"verse" keyword ("psalm 23 1", "Acts 2, 1.").
    //
    // Preachers really do say these — "Romans eight one", "Psalm 23, 1" — and ASR
    // renders the pauses as commas and full stops, which `normalize` strips. So this
    // form has to reach the congregation, or the product misses ordinary preaching.
    //
    // But it is also the shape of garbled speech. A real transcript, from a live
    // rehearsal:
    //
    //     "Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,"
    //
    // scored 0.92 and put Psalms 2:3 on the wall, unasked.
    //
    // What separates the two is not confidence — the parser sees the same shape —
    // it is the LEFTOVER number. "Psalm 23 1" ends cleanly; "Psalms 2, 3, 1" parses
    // 2:3 and leaves a stray "1" that no range could absorb (a range end must be
    // >= the verse). A trailing loose number means the numbers did not line up, and
    // that is the case that stays a suggestion.
    //
    // Note a bare pair off a REPAIRED book name lands at 0.55 - 0.06 = 0.49, still
    // under the default auto-fire line: a misheard book plus loose digits always
    // asks a human. And the sensitivity dial still governs all of it — a cautious
    // install (low dial, auto-fire 0.90) demotes bare pairs exactly as before.
    let bare_digits = chapter_was_digit && verse_was_digit && !used_kw;
    let trailing_number = parse_number(tokens, end_idx).is_some();
    let base = if bare_digits && trailing_number {
        0.45 // the garble shape — reaches the operator, never the congregation
    } else if bare_digits {
        0.55 // above the default auto-fire (0.50), still dial-controllable
    } else if chapter_was_digit && verse_was_digit {
        0.92
    } else {
        0.90
    };
    let mut m = make_match(
        canonical, chapter, verse, tokens, book_start, after_vs, base, used_kw, phonetic,
    );
    if let Some((e, _)) = range {
        m.verse_end = Some(e);
    }
    Some((m, end_idx))
}

#[allow(clippy::too_many_arguments)]
fn make_match(
    canonical: &str,
    chapter: i64,
    verse: i64,
    tokens: &[&str],
    book_start: usize,
    end: usize,
    base: f32,
    used_kw: bool,
    phonetic: bool,
) -> RefMatch {
    let mut conf = base;
    if used_kw {
        conf = conf.max(0.95);
    }
    if phonetic {
        conf -= 0.06;
    }
    // Floor is 0.30, NOT 0.50. It used to be 0.50 — which is exactly the
    // auto-fire threshold — so the weakest possible direct match still went
    // straight to the congregation's screen. Nothing could be demoted to a
    // suggestion even when the parser was barely confident, which made the whole
    // confidence scale decorative below that line.
    let conf = conf.clamp(0.30, 0.99);
    RefMatch {
        reference: VerseRef {
            book: canonical.to_string(),
            chapter,
            verse,
        },
        verse_end: None,
        whole_chapter: false,
        confidence: conf,
        method: DetectionMethod::Direct,
        matched_text: tokens[book_start..end].join(" "),
        // Set by `detect_direct`, which is the only place that knows where the
        // scan actually stopped once ranges have been absorbed.
        at_tail: false,
    }
}

/// After a parsed verse at `idx`, look for an optional range end: an explicit
/// connector ("John 3:16 to 18") or an immediate bare number (hyphen ranges like
/// "3:16-18" tokenize to adjacent numbers, the hyphen becoming whitespace).
/// Returns (end, next_index) only when the span is sane (`end >= start`).
fn parse_range_end(tokens: &[&str], idx: usize, start: i64) -> Option<(i64, usize)> {
    let mut j = idx;
    let mut connector = false;
    while let Some(t) = tokens.get(j) {
        if matches!(*t, "to" | "through" | "thru" | "til" | "until") {
            connector = true;
            j += 1;
        } else {
            break;
        }
    }
    // Without a connector word, only an IMMEDIATELY adjacent number counts (the
    // hyphen-range case) — otherwise a following number is a separate reference.
    if !connector && j != idx {
        return None;
    }
    let (end, after, _) = parse_number(tokens, j)?;
    if end >= start && end - start <= 200 {
        Some((end, after))
    } else {
        None
    }
}

/// A single token of the form "3:16" (both sides numeric) → (chapter, verse).
fn try_colon_pair(tokens: &[&str], idx: usize) -> Option<(i64, i64, usize)> {
    let tok = tokens.get(idx)?;
    let (a, b) = tok.split_once(':')?;
    let ch = a.parse::<i64>().ok()?;
    let vs = b.parse::<i64>().ok()?;
    Some((ch, vs, idx + 1))
}

#[derive(Clone, Copy)]
enum NumWord {
    Ones(i64), // 1-9
    Teen(i64), // 10-19
    Ten(i64),  // 20,30,...,90
    Hundred,
    /// Swahili "mia", Hausa "ɗari" — the multiplier comes AFTER ("mia mbili" =
    /// 200, not 102). See parse_number.
    HundredPost,
}

/// Spoken numbers in the tier-1 languages, from `data/numerals.json`.
///
/// Data, not Rust, for the same reason as the book names: a wrong numeral does
/// not fail safely — it silently shows a DIFFERENT VERSE. If `tisa` were mapped
/// to 8 instead of 9, nobody would find out until a service. A native speaker can
/// fix a number in a one-line pull request without touching this file.
///
/// The GRAMMAR stays here; only the WORDS live in the data.
pub struct Numerals {
    pub ones: HashMap<String, i64>,
    pub tens: HashMap<String, i64>,
    pub hundred_post: HashSet<String>,
    pub connectors: HashSet<String>,
    pub chapter_words: HashSet<String>,
    pub verse_words: HashSet<String>,
    pub linkers: HashSet<String>,
}

static NUMERALS: OnceLock<Numerals> = OnceLock::new();

fn numerals() -> &'static Numerals {
    NUMERALS.get_or_init(|| {
        const RAW: &str = include_str!("../data/numerals.json");
        let mut n = Numerals {
            ones: HashMap::new(),
            tens: HashMap::new(),
            hundred_post: HashSet::new(),
            connectors: HashSet::new(),
            chapter_words: HashSet::new(),
            verse_words: HashSet::new(),
            linkers: HashSet::new(),
        };
        let Ok(doc) = serde_json::from_str::<serde_json::Value>(RAW) else {
            eprintln!("detection: numerals.json is not valid JSON — in-language numbers disabled");
            return n;
        };
        let Some(langs) = doc.as_object() else {
            return n;
        };
        for (lang, spec) in langs {
            if lang.starts_with('_') {
                continue;
            }
            let Some(spec) = spec.as_object() else {
                continue;
            };
            let nums = |key: &str, into: &mut HashMap<String, i64>| {
                if let Some(m) = spec.get(key).and_then(|v| v.as_object()) {
                    for (w, v) in m {
                        if let Some(v) = v.as_i64() {
                            // normalize() folds the hooked letters and diacritics,
                            // so `ɗaya` and `daya` become one key.
                            into.insert(normalize(w), v);
                        }
                    }
                }
            };
            nums("ones", &mut n.ones);
            nums("tens", &mut n.tens);
            let words = |key: &str, into: &mut HashSet<String>| {
                for w in spec
                    .get(key)
                    .and_then(|v| v.as_array())
                    .into_iter()
                    .flatten()
                {
                    if let Some(w) = w.as_str() {
                        into.insert(normalize(w));
                    }
                }
            };
            words("hundred_post", &mut n.hundred_post);
            words("connectors", &mut n.connectors);
            words("chapter_words", &mut n.chapter_words);
            words("verse_words", &mut n.verse_words);
            words("linkers", &mut n.linkers);
        }
        n
    })
}

/// "chapter" in any tier-1 language: Swahili "sura", Hausa "sura"/"babi".
fn is_chapter_word(t: &str) -> bool {
    matches!(t, "chapter" | "chap" | "ch") || numerals().chapter_words.contains(t)
}

/// "verse" in any tier-1 language: Swahili "mstari"/"aya", Hausa "aya".
fn is_verse_word(t: &str) -> bool {
    matches!(t, "verse" | "verses" | "vs" | "v") || numerals().verse_words.contains(t)
}

/// Grammatical glue between a keyword and its number — "sura YA tatu", "mstari WA
/// kwanza", "aya TA farko". Carries no meaning; skipped only when it sits directly
/// between a chapter/verse word and its number, never anywhere else, because "ya"
/// and "na" are among the most common words in Swahili and would otherwise swallow
/// half a sentence.
fn skip_linkers(tokens: &[&str], mut i: usize) -> usize {
    while let Some(t) = tokens.get(i) {
        if numerals().linkers.contains(*t) {
            i += 1;
        } else {
            break;
        }
    }
    i
}

/// Connectors that glue a spoken number together without carrying a value.
///
/// English "and" ("one hundred AND thirteen"), Swahili "na" ("kumi NA tatu"),
/// Hausa "sha" (teens: "goma SHA uku") and "da" ("ashirin DA uku").
///
/// Only ever skipped when a number word genuinely follows, so "one hundred and
/// God is good" does not silently swallow the "and".
fn word_is_and(t: &str) -> bool {
    t == "and" || numerals().connectors.contains(t)
}

/// Parse a spoken/written number starting at `start`. Returns
/// (value, next_index, phonetic_correction_applied) or None.
///
/// A finite state walk, so "three sixteen" parses as 3 (stopping before
/// "sixteen", which is a separate verse) while "twenty eight" → 28 and
/// "one hundred nineteen" → 119.
///
/// ## Swahili and Hausa put the hundred MULTIPLIER AFTER the hundred word
///
/// This is the one place their grammar diverges from English, and it is not
/// cosmetic:
///
/// ```text
///   mia moja  = 100   (literally "hundred one")   NOT 101
///   ɗari biyu = 200                               NOT 102
/// ```
///
/// English puts the multiplier first ("two hundred"). So the English parser, run
/// on Swahili, would read "mia mbili" as 100 + 2 = **102** — and put Psalm 102 on
/// the wall when the preacher said Psalm 200. `HundredPost` exists for exactly
/// that, and a connector disambiguates the two readings: "mia moja" (no connector)
/// is 1×100, while "mia na tatu" (connector) is 100+3.
fn parse_number(tokens: &[&str], start: usize) -> Option<(i64, usize, bool)> {
    // Bare digits: take one token.
    if let Some(t) = tokens.get(start) {
        if let Ok(n) = t.parse::<i64>() {
            return Some((n, start + 1, false));
        }
    }

    enum St {
        Start,
        AfterOnesOrTeen,
        AfterTen,
        AfterHundred,
        AfterHundredTen,
        /// Just saw a Swahili/Hausa hundred word ("mia", "ɗari"). The MULTIPLIER
        /// may still be coming — "mia mbili" is 200, not 102.
        AfterHundredPost,
        Complete,
    }
    let mut state = St::Start;
    let mut value = 0i64;
    let mut idx = start;
    let mut consumed = 0;
    let mut phonetic = false;
    // Did a connector immediately precede this word? It is what tells "mia moja"
    // (1×100) apart from "mia na tatu" (100+3) — see the fn doc.
    let mut saw_connector = false;

    while let Some(&raw) = tokens.get(idx) {
        if tokens[idx].parse::<i64>().is_ok() {
            break; // a digit doesn't extend a spoken number
        }
        // "one hundred AND thirteen" = 113. The FSM used to break on "and" and
        // return 100 — so "sam one hundred and thirteen verse one" auto-fired
        // PSALM 100:1. A wrong verse, on the wall, at full confidence.
        //
        // This is not an edge case for this market: Nigerian, Kenyan and British
        // English all say "a hundred AND thirteen" as the default form. American
        // English drops it, which is presumably why it was never noticed.
        //
        // Only skipped when a number word genuinely follows, so "one hundred and
        // God is good" doesn't silently swallow the "and".
        // Connectors carry no value, they just glue: English "one hundred AND
        // thirteen", Swahili "kumi NA tatu", Hausa "goma SHA uku" / "ashirin DA
        // uku". The FSM used to BREAK on "and" and return what it had, so "sam one
        // hundred and thirteen verse one" auto-fired PSALM 100:1 — a wrong verse,
        // on the wall, at full confidence. (Nigerian, Kenyan and British English
        // all say "a hundred AND thirteen" by default. American English drops it,
        // which is presumably why it was never noticed.)
        //
        // Only skipped when a number word genuinely follows, so "one hundred and
        // God is good" doesn't silently swallow the "and". And never from Start —
        // a bare "na"/"da" is an ordinary word, not the beginning of a number.
        if word_is_and(raw)
            && !matches!(state, St::Start)
            && tokens
                .get(idx + 1)
                .map(|t| classify_num_word(correct_homophone(t).0).is_some())
                .unwrap_or(false)
        {
            idx += 1;
            consumed += 1;
            saw_connector = true;
            continue;
        }
        let (word, ph) = correct_homophone(raw);
        let Some(nw) = classify_num_word(word) else {
            break;
        };
        let next = match (&state, nw) {
            (St::Start, NumWord::Ones(v)) => {
                value = v;
                St::AfterOnesOrTeen
            }
            (St::Start, NumWord::Teen(v)) => {
                value = v;
                St::AfterOnesOrTeen
            }
            (St::Start, NumWord::Ten(v)) => {
                value = v;
                St::AfterTen
            }
            (St::Start, NumWord::Hundred) => {
                value = 100;
                St::AfterHundred
            }
            // "mia" / "ɗari" alone is 100; a multiplier may follow.
            (St::Start, NumWord::HundredPost) => {
                value = 100;
                St::AfterHundredPost
            }
            // "mia MBILI" = 200. No connector → this is the multiplier, not an
            // addend. Getting this wrong shows Psalm 102 for Psalm 200.
            (St::AfterHundredPost, NumWord::Ones(v)) if !saw_connector => {
                value = v * 100;
                St::AfterHundred
            }
            // "mia NA tatu" = 103. A connector means it is an addend after all.
            (St::AfterHundredPost, NumWord::Ones(v)) => {
                value += v;
                St::Complete
            }
            (St::AfterHundredPost, NumWord::Teen(v)) => {
                value += v;
                St::Complete
            }
            // "ɗari da GOMA sha uku" = 113.
            (St::AfterHundredPost, NumWord::Ten(v)) => {
                value += v;
                St::AfterHundredTen
            }
            (St::AfterTen, NumWord::Ones(v)) => {
                value += v;
                St::Complete
            }
            (St::AfterTen, NumWord::Hundred) => {
                value *= 100;
                St::AfterHundred
            }
            (St::AfterOnesOrTeen, NumWord::Hundred) => {
                value *= 100;
                St::AfterHundred
            }
            (St::AfterHundred, NumWord::Ones(v)) | (St::AfterHundred, NumWord::Teen(v)) => {
                value += v;
                St::Complete
            }
            (St::AfterHundred, NumWord::Ten(v)) => {
                value += v;
                St::AfterHundredTen
            }
            (St::AfterHundredTen, NumWord::Ones(v)) => {
                value += v;
                St::Complete
            }
            _ => break, // can't grammatically extend → stop here
        };
        phonetic |= ph;
        consumed += 1;
        idx += 1;
        state = next;
        saw_connector = false; // only ever applies to the word directly after it
        if matches!(state, St::Complete) {
            break;
        }
    }

    if consumed == 0 {
        None
    } else {
        Some((value, idx, phonetic))
    }
}

/// Map common ASR homophones to their number word. Conservative set — only
/// mishears unlikely to collide with ordinary sermon speech.
fn correct_homophone(word: &str) -> (&str, bool) {
    match word {
        "free" | "tree" => ("three", true),
        "fore" => ("four", true),
        "ate" => ("eight", true),
        "won" => ("one", true),
        other => (other, false),
    }
}

fn classify_num_word(w: &str) -> Option<NumWord> {
    let v = match w {
        "one" => NumWord::Ones(1),
        "two" => NumWord::Ones(2),
        "three" => NumWord::Ones(3),
        "four" => NumWord::Ones(4),
        "five" => NumWord::Ones(5),
        "six" => NumWord::Ones(6),
        "seven" => NumWord::Ones(7),
        "eight" => NumWord::Ones(8),
        "nine" => NumWord::Ones(9),
        "ten" => NumWord::Teen(10),
        "eleven" => NumWord::Teen(11),
        "twelve" => NumWord::Teen(12),
        "thirteen" => NumWord::Teen(13),
        "fourteen" => NumWord::Teen(14),
        "fifteen" => NumWord::Teen(15),
        "sixteen" => NumWord::Teen(16),
        "seventeen" => NumWord::Teen(17),
        "eighteen" => NumWord::Teen(18),
        "nineteen" => NumWord::Teen(19),
        "twenty" => NumWord::Ten(20),
        "thirty" => NumWord::Ten(30),
        "forty" => NumWord::Ten(40),
        "fifty" => NumWord::Ten(50),
        "sixty" => NumWord::Ten(60),
        "seventy" => NumWord::Ten(70),
        "eighty" => NumWord::Ten(80),
        "ninety" => NumWord::Ten(90),
        "hundred" => NumWord::Hundred,
        // Tier-1 languages. Words come from data/numerals.json so a native
        // speaker can correct a number without touching Rust — a wrong numeral
        // does not fail safely, it silently shows a different verse.
        w => {
            let n = numerals();
            if let Some(&v) = n.ones.get(w) {
                NumWord::Ones(v)
            } else if let Some(&v) = n.tens.get(w) {
                NumWord::Ten(v)
            } else if n.hundred_post.contains(w) {
                NumWord::HundredPost
            } else {
                return None;
            }
        }
    };
    Some(v)
}

// ===== Context memory (PROMPT.md Phase 9) =====

/// Tracks the current on-screen verse so a bare "verse 4" resolves against the
/// last book+chapter, and "next"/"back" step from it. Pure state — no IO. Fed
/// by whatever verse actually fires.
#[derive(Debug, Clone, Default)]
pub struct ContextMemory {
    current: Option<VerseRef>,
    /// Inclusive last verse of the passage being walked, if the current verse is
    /// part of a multi-verse range or whole chapter. `next` stops here.
    span_end: Option<i64>,
}

impl ContextMemory {
    /// Record a single verse currently shown (clears any active passage span).
    pub fn note(&mut self, r: &VerseRef) {
        self.current = Some(r.clone());
        self.span_end = None;
    }

    /// Record the anchor of a multi-verse passage (range or whole chapter). `end`
    /// is the last verse to walk to; None means step until the chapter runs out.
    pub fn note_passage(&mut self, r: &VerseRef, end: Option<i64>) {
        self.current = Some(r.clone());
        self.span_end = end;
    }

    /// Move the current verse within an active passage WITHOUT clearing the span
    /// — used by "next"/"back" so the range end still bounds the walk.
    pub fn advance(&mut self, r: &VerseRef) {
        self.current = Some(r.clone());
    }

    /// Resolve a bare verse number against the current passage, if any.
    pub fn resolve_bare_verse(&self, verse: i64) -> Option<VerseRef> {
        self.current.as_ref().map(|c| VerseRef {
            book: c.book.clone(),
            chapter: c.chapter,
            verse,
        })
    }

    pub fn current(&self) -> Option<&VerseRef> {
        self.current.as_ref()
    }

    /// The next verse in the passage (verse + 1), if a current exists and we
    /// haven't reached an explicit range end.
    pub fn next_verse(&self) -> Option<VerseRef> {
        self.current.as_ref().and_then(|c| {
            if let Some(end) = self.span_end {
                if c.verse >= end {
                    return None; // reached the end of an explicit range
                }
            }
            Some(VerseRef {
                book: c.book.clone(),
                chapter: c.chapter,
                verse: c.verse + 1,
            })
        })
    }

    /// The previous verse (verse - 1), if a current exists and verse > 1.
    pub fn prev_verse(&self) -> Option<VerseRef> {
        self.current.as_ref().and_then(|c| {
            (c.verse > 1).then(|| VerseRef {
                book: c.book.clone(),
                chapter: c.chapter,
                verse: c.verse - 1,
            })
        })
    }
}

// ===== Topical concordance & cross-references (Phase A: A3/A4) =============

/// A topical theme: spoken trigger keywords → a ranked list of reference
/// strings. Offline, curated. Surfaces related scripture by theme even when the
/// preacher doesn't quote a verse directly (A4), and doubles as the cross-
/// reference source for a fired verse's theme (A3).
struct Theme {
    name: &'static str,
    keywords: &'static [&'static str],
    refs: &'static [&'static str],
}

#[rustfmt::skip]
const THEMES: &[Theme] = &[
    Theme { name: "Fear & Anxiety", keywords: &["afraid","fear","fearful","anxious","anxiety","worry","worried","scared","terrified","nervous","panic","dread"], refs: &["Isaiah 41:10","Philippians 4:6-7","John 14:27","2 Timothy 1:7","Psalm 56:3","Joshua 1:9"] },
    Theme { name: "Trouble & Storms", keywords: &["trouble","storm","storms","trial","trials","suffering","suffer","hardship","crisis","struggle","struggling","overwhelmed"], refs: &["Psalm 46:1","John 16:33","Romans 8:28","Isaiah 43:2","2 Corinthians 4:17","Psalm 34:18"] },
    Theme { name: "Refuge & Protection", keywords: &["refuge","shelter","protect","protection","safe","safety","fortress","shield","stronghold"], refs: &["Psalm 46:1","Psalm 91:1-2","Psalm 18:2","Nahum 1:7","Proverbs 18:10","Psalm 27:5"] },
    Theme { name: "Peace & Rest", keywords: &["peace","peaceful","rest","still","calm","quiet","weary","tired","burden","burdened"], refs: &["John 14:27","Matthew 11:28","Psalm 46:10","Isaiah 26:3","Philippians 4:7","Psalm 23:2"] },
    Theme { name: "Strength & Endurance", keywords: &["strength","strong","strengthen","weak","weakness","power","endure","persevere","overcome"], refs: &["Isaiah 40:31","Philippians 4:13","2 Corinthians 12:9","Psalm 46:1","Nehemiah 8:10","Ephesians 6:10"] },
    Theme { name: "Faith & Trust", keywords: &["faith","faithful","believe","believed","trust","trusting","doubt","doubting","confidence"], refs: &["Hebrews 11:1","Proverbs 3:5-6","Mark 11:24","2 Corinthians 5:7","Romans 10:17","Matthew 17:20"] },
    Theme { name: "Hope & Future", keywords: &["hope","hopeful","future","plans","tomorrow","expectation","promise","promises"], refs: &["Jeremiah 29:11","Romans 15:13","Romans 8:24-25","Lamentations 3:22-23","Psalm 39:7","Hebrews 6:19"] },
    Theme { name: "Love", keywords: &["love","loved","loves","loving","beloved","compassion","kindness"], refs: &["1 Corinthians 13:4-7","John 3:16","1 John 4:19","Romans 5:8","1 John 4:8","Romans 8:38-39"] },
    Theme { name: "Joy & Praise", keywords: &["joy","joyful","rejoice","glad","gladness","celebrate","praise","worship","thanksgiving","thankful","grateful"], refs: &["Psalm 16:11","Nehemiah 8:10","Philippians 4:4","Psalm 100:4","James 1:2","Psalm 30:5"] },
    Theme { name: "Forgiveness & Grace", keywords: &["forgive","forgiven","forgiveness","sin","sins","mercy","merciful","grace","guilt","repent","repentance"], refs: &["1 John 1:9","Ephesians 4:32","Psalm 103:12","Colossians 3:13","Romans 5:8","Micah 7:18"] },
    Theme { name: "Salvation", keywords: &["salvation","saved","save","saviour","savior","eternal","cross","gospel","redeemed"], refs: &["Ephesians 2:8-9","Romans 10:9","John 3:16","Acts 4:12","Titus 3:5","Romans 6:23"] },
    Theme { name: "Comfort & Grief", keywords: &["comfort","grief","grieve","grieving","mourn","mourning","sorrow","death","loss","brokenhearted","heartbroken","tears"], refs: &["Psalm 23:4","Matthew 5:4","Revelation 21:4","2 Corinthians 1:3-4","Psalm 34:18","Psalm 147:3"] },
    Theme { name: "Provision & Needs", keywords: &["provide","provision","need","needs","supply","money","finances","lack","hunger"], refs: &["Philippians 4:19","Matthew 6:33","Psalm 23:1","Malachi 3:10","Matthew 6:26","2 Corinthians 9:8"] },
    Theme { name: "Guidance & Direction", keywords: &["guide","guidance","direction","lead","leading","path","decision","wisdom","discern","purpose","calling"], refs: &["Proverbs 3:5-6","Psalm 119:105","Isaiah 30:21","Jeremiah 29:11","James 1:5","Psalm 32:8"] },
    Theme { name: "Prayer", keywords: &["pray","prayer","praying","intercede","petition","seek","knock"], refs: &["Philippians 4:6","1 Thessalonians 5:17","James 5:16","Matthew 7:7","Jeremiah 33:3","Matthew 6:9"] },
    Theme { name: "God's Faithfulness", keywords: &["faithfulness","forsake","unchanging","covenant","steadfast","forever","everlasting"], refs: &["Lamentations 3:22-23","Deuteronomy 31:6","Hebrews 13:5","Joshua 1:9","Psalm 100:5","2 Timothy 2:13"] },
    Theme { name: "New Life & Identity", keywords: &["identity","created","creation","transform","transformed","chosen","fearfully","wonderfully"], refs: &["2 Corinthians 5:17","Ephesians 2:10","1 Peter 2:9","Psalm 139:14","Galatians 2:20","Romans 12:2"] },
    Theme { name: "Light & Truth", keywords: &["light","darkness","dark","truth","lamp","shine","reveal"], refs: &["John 8:12","Psalm 119:105","John 1:5","Matthew 5:14","John 14:6","1 John 1:5"] },
];

/// A related-scripture suggestion: the matched theme and its references (already
/// parsed, so ranges carry through), with the anchor verse removed.
#[derive(Debug, Clone)]
pub struct RelatedSuggestion {
    pub theme: String,
    pub refs: Vec<RefMatch>,
}

/// Suggest related scripture for a transcript window by topical keyword match
/// (A4), also usable to cross-reference a fired verse's theme (A3). `exclude`
/// drops the currently-shown verse. Returns at most `max` refs, or None when no
/// theme is clearly indicated. Pure and offline.
pub fn suggest_related(
    text: &str,
    exclude: Option<&VerseRef>,
    max: usize,
) -> Option<RelatedSuggestion> {
    let norm = format!(" {} ", normalize(text));
    let mut best: Option<(&Theme, u32)> = None;
    for th in THEMES {
        let mut score = 0u32;
        for kw in th.keywords {
            if norm.contains(&format!(" {kw} ")) {
                // Multi-word keywords are stronger signal than single words.
                score += if kw.contains(' ') { 2 } else { 1 };
            }
        }
        if score > 0 && best.map(|(_, s)| score > s).unwrap_or(true) {
            best = Some((th, score));
        }
    }
    let (theme, _) = best?;
    let ex = exclude.map(|e| (e.book.as_str(), e.chapter, e.verse));
    let mut refs = Vec::new();
    for r in theme.refs {
        let Some(m) = detect_direct(r).into_iter().next() else {
            continue; // spelling that doesn't resolve → skip, never break
        };
        let vr = &m.reference;
        if Some((vr.book.as_str(), vr.chapter, vr.verse)) == ex {
            continue;
        }
        refs.push(m);
        if refs.len() >= max {
            break;
        }
    }
    if refs.is_empty() {
        None
    } else {
        Some(RelatedSuggestion {
            theme: theme.name.into(),
            refs,
        })
    }
}

/// Operator voice navigation commands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NavCommand {
    Next,
    Previous,
}

/// Detect a spoken navigation command in a short utterance ("next", "back",
/// "previous", "next verse"). Only fires on short utterances so it doesn't
/// trigger mid-sermon. Returns None for anything longer or unrelated.
pub fn detect_command(text: &str) -> Option<NavCommand> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 5 {
        return None;
    }
    let has = |w: &str| tokens.contains(&w);
    if has("next") {
        Some(NavCommand::Next)
    } else if has("back") || has("previous") || has("prev") {
        Some(NavCommand::Previous)
    } else {
        None
    }
}

/// Detect a spoken "clear / blackout the screen" command (Phase D3). Deliberately
/// conservative — "clear" and "blank" alone are common sermon words, so they only
/// fire when paired with a screen object; "blackout" is unambiguous on its own.
/// Short-utterance guarded so it never triggers on prose.
pub fn detect_clear(text: &str) -> bool {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 5 {
        return false;
    }
    let joined = format!(" {} ", tokens.join(" "));
    let has = |w: &str| tokens.contains(&w);
    let screen = has("screen") || has("screens");
    has("blackout")
        || joined.contains(" black out ")
        || (has("clear") && screen)
        || (has("blank") && screen)
        || joined.contains(" take it down ")
        || joined.contains(" take that down ")
        || joined.contains(" take it off ")
}

/// Find bare verse references ("verse 4", "verse twenty-eight") in `text`.
/// Returns the verse numbers; the caller resolves them via ContextMemory.
pub fn detect_bare_verses(text: &str) -> Vec<i64> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        // Both singular ("verse 1") and plural ("verses 1"), plus abbreviations.
        if matches!(tokens[i], "verse" | "verses" | "vs" | "v") {
            if let Some((n, _, _)) = parse_number(&tokens, i + 1) {
                out.push(n);
            }
        }
        i += 1;
    }
    out
}

/// A spoken jump WITHIN the current book — chapter and/or verse, no book name.
#[derive(Debug, Clone, PartialEq)]
pub struct PassageNav {
    pub chapter: Option<i64>,
    pub verse: Option<i64>,
}

/// True if any book alias appears in the tokens.
fn book_named(tokens: &[&str]) -> bool {
    (0..tokens.len()).any(|i| match_book(tokens, i).is_some())
}

/// Detect a spoken jump within the CURRENT book — "chapter 5 verse 1",
/// "chapter fifty verse two", "go to chapter 5", "verse 4" — WITHOUT naming a
/// book (a named book goes through the normal reference path). The caller
/// resolves the book from context (the last verse shown) and keeps the operator
/// in the same passage. Requires a chapter/verse keyword and a short utterance
/// so ordinary sermon prose with numbers never triggers.
pub fn detect_passage_nav(text: &str) -> Option<PassageNav> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    if tokens.is_empty() || tokens.len() > 8 {
        return None;
    }
    if book_named(&tokens) {
        return None; // an explicit "Psalm 5:1" is a full reference, not a jump
    }
    let mut chapter = None;
    let mut verse = None;
    let mut saw_kw = false;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "chapter" | "chapters" | "chap" | "ch" => {
                saw_kw = true;
                if let Some((n, next, _)) = parse_number(&tokens, i + 1) {
                    chapter = Some(n);
                    i = next;
                    continue;
                }
            }
            "verse" | "verses" | "vs" | "v" => {
                saw_kw = true;
                if let Some((n, next, _)) = parse_number(&tokens, i + 1) {
                    verse = Some(n);
                    i = next;
                    continue;
                }
            }
            _ => {}
        }
        i += 1;
    }
    if !saw_kw || (chapter.is_none() && verse.is_none()) {
        return None;
    }
    Some(PassageNav { chapter, verse })
}

/// Generate candidate references for an AMBIGUOUS book+number with no verse,
/// e.g. "revelation 22" → [Revelation 22:1, Revelation 2:2]. Used only when no
/// full reference was detected, to surface operator-pickable suggestions. The
/// caller filters these against the corpus and gates them as suggestions.
pub fn detect_ambiguous(text: &str) -> Vec<VerseRef> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if let Some((canonical, book_end, _fuzzy)) = match_book(&tokens, i) {
            let mut j = book_end;
            if let Some(t) = tokens.get(j) {
                if matches!(*t, "chapter" | "chap" | "ch") {
                    j += 1;
                }
            }
            // A colon form is unambiguous — skip.
            if try_colon_pair(&tokens, j).is_none() {
                if let Some((n, after, _)) = parse_number(&tokens, j) {
                    // Is a verse already present? Then it's not ambiguous.
                    let mut k = after;
                    while let Some(t) = tokens.get(k) {
                        if matches!(*t, "verse" | "verses" | "vs" | "v" | ":") {
                            k += 1;
                        } else {
                            break;
                        }
                    }
                    let has_verse = parse_number(&tokens, k).is_some();
                    if !has_verse && !is_single_chapter(canonical) {
                        // chapter N, verse 1
                        out.push(VerseRef {
                            book: canonical.into(),
                            chapter: n,
                            verse: 1,
                        });
                        // two-digit split: 22 → 2:2, 21 → 2:1
                        if (11..=99).contains(&n) && n % 10 >= 1 {
                            out.push(VerseRef {
                                book: canonical.into(),
                                chapter: n / 10,
                                verse: n % 10,
                            });
                        }
                    }
                    i = after;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

// ===== Semantic match (PROMPT.md Phase 9) =====

/// A vector-similarity index over the verse corpus. Phase 9 uses a TF-IDF
/// bag-of-words embedding with cosine similarity — a genuine embedding+search
/// that runs fully offline with no model. A neural sentence-embedder is a
/// drop-in behind the same `top_k` seam (and the verses.embedding BLOB column)
/// later; it improves synonym/paraphrase recall that lexical overlap misses.
pub struct SemanticIndex {
    idf: HashMap<String, f32>,
    /// (reference, L2-normalized tf-idf vector) per verse.
    docs: Vec<(VerseRef, HashMap<String, f32>)>,
    /// stem → the readable word it came from, for the operator-facing "why".
    /// Stemming is right for matching and wrong for reading: Snowball turns
    /// "belly" into "belli". Rule #18 says the operator must be able to judge
    /// the claim, and nobody can judge "belli · husk".
    surface: HashMap<String, String>,
    /// STORIES. Overlapping windows of contiguous verses within one chapter,
    /// each with its own tf-idf vector, plus the range of `docs` it covers.
    /// Built from the SAME stemmed tokens as `docs`, so a story and its verses
    /// live in one vocabulary. See `PASSAGE_LEN` and `top_k_explained`.
    passages: Vec<(usize, usize, HashMap<String, f32>)>,
    /// Stems rare enough to stand as evidence ALONE — see `RARE_DF_FRACTION`
    /// and DECISIONS.md §25. Held as a set, computed once at build time from
    /// document frequency, because the query path must not re-derive `df` from
    /// a float `idf`.
    rare_terms: std::collections::HashSet<String>,
}

/// Modern English → KJV vocabulary, baked in (`include_str!`) so it stays
/// offline. See `data/kjv_gloss.json` for why this exists and what it is not.
/// Keys AND values are stemmed at load, and the gloss is applied after
/// stemming — so one entry ("pig") covers "pig" and "pigs", and the table stays
/// a list of concepts instead of a list of word forms.
fn kjv_gloss() -> &'static HashMap<String, Vec<String>> {
    static GLOSS: std::sync::OnceLock<HashMap<String, Vec<String>>> = std::sync::OnceLock::new();
    GLOSS.get_or_init(|| {
        #[derive(serde::Deserialize)]
        struct Raw {
            gloss: HashMap<String, Vec<String>>,
        }
        const RAW: &str = include_str!("../data/kjv_gloss.json");
        let raw = serde_json::from_str::<Raw>(RAW)
            .map(|r| r.gloss)
            .unwrap_or_default();
        let mut out: HashMap<String, Vec<String>> = HashMap::new();
        for (k, vs) in raw {
            let key = stem_all(vec![k]).pop().unwrap_or_default();
            out.entry(key).or_default().extend(stem_all(vs));
        }
        for vs in out.values_mut() {
            vs.sort();
            vs.dedup();
        }
        out
    })
}

/// Expand a QUERY's tokens with their KJV equivalents.
///
/// Applied to the query only, never when building the index: glossing the corpus
/// would change the document frequencies, and it is exactly those frequencies
/// (how rare "Meribah" or "husks" is) that make biblical nouns such strong
/// signals. The original token is kept — a retelling that already uses the KJV
/// word must not get worse — so this can only ever ADD evidence.
fn expand_with_gloss(tokens: Vec<String>) -> Vec<String> {
    let gloss = kjv_gloss();
    let mut out = Vec::with_capacity(tokens.len() * 2);
    for t in tokens {
        if let Some(alts) = gloss.get(&t) {
            out.extend(alts.iter().cloned());
        }
        out.push(t);
    }
    out
}

/// How many verses make a "story".
///
/// A pericope — the boy with the loaves, the storm on the lake, David and
/// Goliath — is a handful of verses, not a chapter (Psalm 119 is 176) and not a
/// verse. Eight is about the span of a narrative unit in the KJV, and the
/// windows OVERLAP by half so a story is never sliced down the middle.
const PASSAGE_LEN: usize = 8;
const PASSAGE_STEP: usize = 4;

/// How much of the final score comes from the STORY rather than the verse.
///
/// Asked for directly: *"while searching for paraphrase, prioritise the most
/// relevant stories in the bible and subsequently narrow down to the relevant
/// verse."*
///
/// Why blend rather than filter to the best story and stop: a paraphrase is
/// sometimes a single famous verse with no story around it ("for God so loved
/// the world"), and a hard story-first filter would rank that by the accident of
/// what surrounds it. Blending keeps the verse's own evidence in charge while
/// letting the surrounding narrative break ties — which is what "narrow down to
/// the relevant verse" actually means.
/// MEASURED, not chosen — `story_search::story_weight_measured_against_verse_only`
/// sweeps it over seven spoken story-paraphrases against the full corpus:
///
/// ```text
///   weight   mean rank (missing = 6)   found in top 5
///   0.00     2.57                      6/7      ← verse-only, the old behaviour
///   0.25     2.00                      7/7
///   0.35     1.86                      7/7      ← shipped
///   0.50     2.29                      7/7
///   0.65     2.71                      5/7      ← the story starts drowning the verse
/// ```
///
/// Past ~0.5 the narrative overwhelms the line: every verse in a matching story
/// scores alike and the gate is asked to choose between eight equally-blessed
/// candidates. Re-run the sweep before moving this.
const STORY_WEIGHT: f32 = 0.35;

impl SemanticIndex {
    /// Build the index from the corpus: (reference, verse text).
    pub fn build(corpus: &[(VerseRef, String)]) -> Self {
        let n = corpus.len().max(1) as f32;
        // Document frequency per term.
        let mut df: HashMap<String, f32> = HashMap::new();
        // stem → { original word → times seen }, collapsed below to the most
        // common surface form so the explanation reads like English.
        let mut surface_counts: HashMap<String, HashMap<String, usize>> = HashMap::new();
        let tokenized: Vec<(VerseRef, Vec<String>)> = corpus
            .iter()
            .map(|(r, text)| {
                let raw = tokenize(text);
                let stems = stem_all(raw.clone());
                for (stem, word) in stems.iter().zip(raw.iter()) {
                    *surface_counts
                        .entry(stem.clone())
                        .or_default()
                        .entry(word.clone())
                        .or_insert(0) += 1;
                }
                (r.clone(), stems)
            })
            .collect();
        // Most frequent original wins; ties go to the shorter word, which is the
        // one closer to a dictionary form.
        let surface: HashMap<String, String> = surface_counts
            .into_iter()
            .map(|(stem, counts)| {
                let best = counts
                    .into_iter()
                    .max_by(|a, b| a.1.cmp(&b.1).then(b.0.len().cmp(&a.0.len())))
                    .map(|(w, _)| w)
                    .unwrap_or_else(|| stem.clone());
                (stem, best)
            })
            .collect();
        for (_, toks) in &tokenized {
            let mut seen = std::collections::HashSet::new();
            for t in toks {
                if seen.insert(t.clone()) {
                    *df.entry(t.clone()).or_insert(0.0) += 1.0;
                }
            }
        }
        // Rarity is decided HERE, from the raw document frequencies, while they
        // still exist — `idf` is a float and recovering `df` back out of it is
        // not something the query path should be doing.
        let rare_cutoff = (n * RARE_DF_FRACTION).max(1.0);
        let rare_terms: std::collections::HashSet<String> = df
            .iter()
            .filter(|(_, d)| **d <= rare_cutoff)
            .map(|(t, _)| t.clone())
            .collect();
        let idf: HashMap<String, f32> = df
            .into_iter()
            .map(|(t, d)| (t, (n / d).ln() + 1.0))
            .collect();

        // Passage vectors are built from the same tokens, pooled. A story's
        // vocabulary is far richer than any one of its verses, which is exactly
        // why a paraphrase of the STORY matches it when it matches no single
        // verse strongly.
        let mut passages: Vec<(usize, usize, HashMap<String, f32>)> = Vec::new();
        let mut chapter_start = 0usize;
        for i in 0..=tokenized.len() {
            let boundary = i == tokenized.len()
                || (i > 0
                    && (tokenized[i].0.book != tokenized[i - 1].0.book
                        || tokenized[i].0.chapter != tokenized[i - 1].0.chapter));
            if !boundary {
                continue;
            }
            // One chapter spans [chapter_start, i). Window it.
            let mut w = chapter_start;
            while w < i {
                let end = (w + PASSAGE_LEN).min(i);
                let mut pooled: Vec<String> = Vec::new();
                for (_, toks) in &tokenized[w..end] {
                    pooled.extend(toks.iter().cloned());
                }
                if !pooled.is_empty() {
                    passages.push((w, end, tfidf_vector(&pooled, &idf)));
                }
                if end == i {
                    break;
                }
                w += PASSAGE_STEP;
            }
            chapter_start = i;
        }

        let docs: Vec<(VerseRef, HashMap<String, f32>)> = tokenized
            .into_iter()
            .map(|(r, toks)| (r, tfidf_vector(&toks, &idf)))
            .collect();

        SemanticIndex {
            idf,
            docs,
            surface,
            passages,
            rare_terms,
        }
    }

    /// Top-k verses by cosine similarity to `query`, highest first. Scores are
    /// in [0, 1]; the caller maps them to confidence and applies the gate.
    pub fn top_k(&self, query: &str, k: usize) -> Vec<(VerseRef, f32)> {
        self.top_k_explained(query, k)
            .into_iter()
            .map(|(r, s, _)| (r, s))
            .collect()
    }

    /// Repair query words the corpus has never seen.
    ///
    /// Asked for directly: *"the audio should be ultra sensitive to African tone
    /// — e.g. goden → golden"*. Whisper on accented speech drops and swaps
    /// consonants, and the result is a token that appears nowhere in the Bible.
    ///
    /// ── Why this is safe in a way that book-name repair is not ─────────────
    ///
    /// An out-of-vocabulary token has **no idf entry, so it contributes exactly
    /// nothing to the cosine today** — it is silently discarded. Anything this
    /// function does is therefore strictly additive: the worst case is that a
    /// word which was being ignored carries on being ignored.
    ///
    /// That is the opposite of `fuzzy_book`, where a wrong repair invents a
    /// reference that can auto-fire. Here a wrong repair merely adds a weak
    /// term to a paraphrase score which, by law, can never auto-fire at all
    /// (`DetectionMethod::Semantic`).
    ///
    /// Still conservative: known words are never touched, short words are left
    /// alone, and an ambiguous repair is refused rather than guessed.
    fn repair_query(&self, tokens: &[String]) -> Vec<String> {
        tokens
            .iter()
            .map(|t| {
                if t.len() < 4 || self.idf.contains_key(t) {
                    return t.clone();
                }
                let mut best: Option<(usize, &String)> = None;
                let mut tie = false;
                for cand in self.idf.keys() {
                    if cand.len().abs_diff(t.len()) > 1 {
                        continue;
                    }
                    let Some(d) = edit_distance_within(t, cand, 1) else {
                        continue;
                    };
                    match best {
                        Some((bd, _bc)) if d < bd => {
                            best = Some((d, cand));
                            tie = false;
                        }
                        Some((bd, bc)) if d == bd && bc != cand => tie = true,
                        None => best = Some((d, cand)),
                        _ => {}
                    }
                }
                match best {
                    // A tie is no evidence. Leave the word unknown, exactly as
                    // it is today.
                    Some((_, c)) if !tie => c.clone(),
                    _ => t.clone(),
                }
            })
            .collect()
    }

    /// `top_k`, plus the words that actually drove each match — strongest first.
    ///
    /// This is the paraphrase counterpart of `RefMatch::matched_text`, and the
    /// operator console needs it for the same reason. For a spoken reference, "what
    /// triggered this" is a span of transcript the parser read. A TF-IDF match has
    /// no span: its evidence is a handful of shared, rare words, and its score is a
    /// cosine — a distance in an arbitrary vector space, not a probability (see
    /// `DetectionMethod::Semantic`).
    ///
    /// So the operator is being asked to trust a machine's guess about *meaning*, on
    /// the strength of a number that does not mean what it looks like it means. The
    /// words it keyed on are the only thing that makes that judgeable in the second
    /// they have to judge it — "grace · saved · faith" is something a human can
    /// agree or disagree with. "0.61" is not.
    pub fn top_k_explained(&self, query: &str, k: usize) -> Vec<(VerseRef, f32, Vec<String>)> {
        // TOKENIZE → STEM → REPAIR → GLOSS, in that order, and the order is the
        // whole point. `repair_query` and the gloss both look words up in the
        // INDEX's vocabulary, and since the index is stemmed, they can only be
        // asked about stems. Repair before gloss so a misheard word is corrected
        // first and then expanded, rather than expanded as the wrong word.
        let qvec = tfidf_vector(
            &expand_with_gloss(self.repair_query(&stem_all(tokenize(query)))),
            &self.idf,
        );
        if qvec.is_empty() {
            return Vec::new();
        }
        // Sorted once per query, so every document is scored by summing the same
        // terms in the same order — identical input, identical score, every run.
        let mut qsorted: Vec<(String, f32)> = qvec.iter().map(|(t, w)| (t.clone(), *w)).collect();
        qsorted.sort_by(|a, b| a.0.cmp(&b.0));
        // ── STORY FIRST, THEN THE VERSE ──────────────────────────────────
        //
        // Score the STORIES, and let the best story lift the verses inside it.
        //
        // A single verse is a very short document: "and he took the five loaves"
        // shares few words with "jesus fed the crowd from a boy's lunch", so a
        // paraphrase of a NARRATIVE often matches no verse strongly while
        // matching its passage decisively. Pooling a pericope's vocabulary is
        // what makes the story findable; the verse-level score then decides
        // WHICH verse inside it goes on the screen.
        //
        // Each verse keeps the best score of any window containing it — windows
        // overlap, so a verse near a boundary is judged by the story it belongs
        // to rather than by where the window happened to be cut.
        let mut story: Vec<f32> = vec![0.0; self.docs.len()];
        for (from, to, pvec) in &self.passages {
            let s = cosine(&qsorted, pvec);
            if s <= 0.0 {
                continue;
            }
            for slot in story[*from..*to].iter_mut() {
                if s > *slot {
                    *slot = s;
                }
            }
        }

        let mut scored: Vec<(usize, f32)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, (_, dvec))| {
                let verse = cosine(&qsorted, dvec);
                // The verse's own evidence stays in charge; the story it sits in
                // breaks ties. A verse with no evidence of its own is NOT
                // promoted just for having good neighbours — otherwise every
                // verse in a matching chapter would become a candidate, and the
                // gate would be asked to choose between eight equally-blessed
                // lines.
                let blended = if verse > 0.0 {
                    verse * (1.0 - STORY_WEIGHT) + story[i] * STORY_WEIGHT
                } else {
                    0.0
                };
                (i, blended)
            })
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // NARROW TO WHAT CAN BE JUSTIFIED, **BEFORE** TAKING THE TOP k.
        //
        // The evidence filter used to run after `truncate(k)`, and the live path
        // asks for exactly one candidate (`top_k_explained(text, 1)`). So an
        // unjustifiable top-1 did not step aside — it CONSUMED THE ONLY SLOT and
        // was then dropped, and the correct verse sitting at rank 2 was never
        // considered. The operator saw no suggestion at all and no reason why.
        //
        // Rejecting a candidate has to mean the next one gets its turn.
        // A short query cannot corroborate itself three ways, so the requirement
        // bends to it — but never below TWO, whatever was said. A single shared
        // word is a coincidence with a good score at any query length, and
        // `evidence_floor` exists to forbid exactly that.
        let required = MIN_EVIDENCE_TERMS.min(qvec.len()).max(2);
        scored
            .into_iter()
            .filter_map(|(i, s)| {
                let (r, dvec) = &self.docs[i];
                // Judged on the STEMS, before they are made readable — rarity is
                // a fact about the index's vocabulary, and `surface` deliberately
                // maps several stems onto one word.
                let stems = top_terms(&qvec, dvec, EXPLAIN_TERMS);
                // NARROW TO WHAT CAN BE JUSTIFIED. `required` is the bar — see
                // above — so a candidate is corroborated rather than merely
                // confident. A SINGLE word clears it only when that word is rare
                // enough to be evidence by itself ("swine", "ossifrage") — never
                // a common one ("lord"). DECISIONS.md §25.
                if stems.len() < required && !stems.iter().any(|t| self.rare_terms.contains(t)) {
                    return None;
                }
                let why: Vec<String> = stems
                    .into_iter()
                    .map(|t| self.surface.get(&t).cloned().unwrap_or(t))
                    .collect();
                Some((r.clone(), s, why))
            })
            .take(k)
            .collect()
    }
}

impl SemanticIndex {
    /// Rank with an explicit story weight. TEST ONLY — this is how the value of
    /// `STORY_WEIGHT` is measured rather than asserted, by running the same
    /// queries at `0.0` (verse-only, the old behaviour) and at the shipped value.
    #[cfg(test)]
    pub fn top_k_story_weighted(&self, query: &str, k: usize, w: f32) -> Vec<(VerseRef, f32)> {
        // Must mirror `top_k_explained`'s query pipeline exactly, or the number
        // it measures is not the number that ships.
        let qvec = tfidf_vector(
            &expand_with_gloss(self.repair_query(&stem_all(tokenize(query)))),
            &self.idf,
        );
        if qvec.is_empty() {
            return Vec::new();
        }
        let mut qsorted: Vec<(String, f32)> = qvec.iter().map(|(t, w)| (t.clone(), *w)).collect();
        qsorted.sort_by(|a, b| a.0.cmp(&b.0));
        let mut story: Vec<f32> = vec![0.0; self.docs.len()];
        for (from, to, pvec) in &self.passages {
            let sc = cosine(&qsorted, pvec);
            if sc <= 0.0 {
                continue;
            }
            for slot in story[*from..*to].iter_mut() {
                if sc > *slot {
                    *slot = sc;
                }
            }
        }
        let mut scored: Vec<(usize, f32)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, (_, dvec))| {
                let verse = cosine(&qsorted, dvec);
                let blended = if verse > 0.0 {
                    verse * (1.0 - w) + story[i] * w
                } else {
                    0.0
                };
                (i, blended)
            })
            .filter(|(_, sc)| *sc > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
            .into_iter()
            .map(|(i, sc)| (self.docs[i].0.clone(), sc))
            .collect()
    }
}

/// How many overlapping words to show as the reason for a paraphrase match.
///
/// Raised from 4 to 6 on the operator's report that the console was showing a
/// single word: with a thin match there was one term to show, and one word is
/// not evidence a human can weigh. More terms cost nothing when they exist and
/// the UI truncates what will not fit.
const EXPLAIN_TERMS: usize = 6;

/// The least shared evidence a paraphrase may be suggested on.
///
/// ── Why a COUNT and not just a higher cosine ──────────────────────────────
///
/// A cosine can be respectable on ONE shared word if that word is rare enough —
/// the vector is short, so a single strong term dominates it. That is how the
/// console ended up offering a verse whose entire justification was one word,
/// which is not something an operator can agree or disagree with in the second
/// they have to judge it.
///
/// Requiring several independent overlapping words is a different kind of
/// evidence from requiring a bigger number: it asks the match to be corroborated
/// rather than merely confident. A verse sharing four content words with what
/// was said is defensible on its face; one sharing a single word is a
/// coincidence with a good score.
///
/// MEASURED, twice, and the second measurement moved it from 2 to 3.
///
/// It sat at 2 because "at 3 the shipped eval corpus loses recall". That was
/// true and it was an artifact: the filter ran AFTER `truncate(k)`, and the live
/// path asks for one candidate, so a rejected top-1 consumed the only slot and
/// left nothing rather than yielding to the verse behind it. Rejecting a
/// candidate now means the next one gets its turn, and at 3 the shipped corpus
/// holds 100% recall / 0% wrong-verse.
///
/// The value is set by the PARAPHRASE benchmark (`eval::paraphrase`), because
/// that is the behaviour it governs — the shipped corpus is almost entirely
/// direct references and cannot see this at all. Measured over 16 real preacher
/// paraphrases: **recall@1 rises 69% → 75%**, and the wrong top-1 answers fall
/// from 5 to 3. The two it kills are the exact shape the operator complained
/// about — a whole verse justified by two words:
///
///     ["flesh", "among"]  → Proverbs 23:20   for "the word became flesh and dwelt among us"
///     ["promise", "god"]  → Galatians 3:18   for "the promise of God … mixing with faith"
///
/// Corroboration, not confidence. A verse sharing four content words with what
/// was said is defensible on its face; one sharing two is a coincidence with a
/// good score, and no operator can weigh it in the second they have.
///
/// ONE EXCEPTION, and it is `RARE_DF_FRACTION`: a single word that is rare
/// enough IS corroboration, because there is nothing else in the corpus it
/// could have come from. See DECISIONS.md §25.
const MIN_EVIDENCE_TERMS: usize = 3;

/// How rare a stem must be to stand as evidence ON ITS OWN: it may appear in at
/// most this fraction of the corpus (floored at one document, so the rule still
/// means something on the tiny corpora the tests build).
///
/// 0.1% of the full KJV is ~31 of 31,102 verses. "swine" (~30 verses) and
/// "ossifrage" (2) clear it; "lord" (~7,800) is nowhere near. That is exactly
/// the line this is meant to draw — a word that names one story, versus a word
/// that names half the Bible.
const RARE_DF_FRACTION: f32 = 0.001;

/// The shared terms that contributed most to a cosine — the "why" of a paraphrase.
///
/// Contribution is `q_weight * d_weight` per term, which is exactly the summand in
/// `cosine`. So these are not "words that happen to appear in both"; they are, in
/// order, the terms that actually produced the score.
fn top_terms(q: &HashMap<String, f32>, d: &HashMap<String, f32>, n: usize) -> Vec<String> {
    let mut terms: Vec<(&String, f32)> = q
        .iter()
        .filter_map(|(t, qw)| d.get(t).map(|dw| (t, qw * dw)))
        .collect();
    terms.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(b.0)) // stable, so the UI does not flicker
    });
    terms.truncate(n);
    terms.into_iter().map(|(t, _)| t.clone()).collect()
}

/// Content-word tokenizer: lowercase, split on non-alphanumerics, drop short
/// tokens and function words. KJV archaisms (thou/hath/…) are dropped as
/// stopwords; scripture content words are kept.
fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter_map(|w| {
            let w = w.to_lowercase();
            if w.len() >= 3 && !is_stopword(&w) {
                Some(w)
            } else {
                None
            }
        })
        .collect()
}

/// Reduce tokens to their stems, so "pigs"/"pig", "flames"/"flame" and
/// "physicians"/"physician" stop being different words.
///
/// Applied to BOTH the index and the query — unlike the gloss, this is a
/// normalisation, and normalising only one side would simply stop them matching.
///
/// Deliberately a STANDARD Snowball stemmer rather than a hand-rolled suffix
/// stripper: a stemmer written here would inevitably get tuned against
/// data/paraphrase_corpus.json until the score looked good, which measures
/// nothing. An off-the-shelf algorithm cannot be fitted to our own benchmark.
fn stem_all(tokens: Vec<String>) -> Vec<String> {
    use rust_stemmers::{Algorithm, Stemmer};
    static STEMMER: std::sync::OnceLock<Stemmer> = std::sync::OnceLock::new();
    let s = STEMMER.get_or_init(|| Stemmer::create(Algorithm::English));
    tokens
        .into_iter()
        .map(|t| s.stem(&t).into_owned())
        .collect()
}

fn is_stopword(w: &str) -> bool {
    const STOP: &[&str] = &[
        "the", "and", "that", "for", "his", "him", "her", "she", "you", "your", "our", "with",
        "not", "but", "was", "were", "are", "this", "shall", "unto", "thou", "thy", "thee", "ye",
        "hath", "have", "had", "which", "will", "them", "they", "their", "there", "then", "than",
        "when", "who", "what", "all", "any", "from", "out", "into", "upon", "did", "does", "doth",
    ];
    STOP.contains(&w)
}

/// L2-normalized tf-idf vector for a token list, given a corpus idf map.
fn tfidf_vector(tokens: &[String], idf: &HashMap<String, f32>) -> HashMap<String, f32> {
    let mut tf: HashMap<String, f32> = HashMap::new();
    for t in tokens {
        if idf.contains_key(t) {
            *tf.entry(t.clone()).or_insert(0.0) += 1.0;
        }
    }
    let mut vec: HashMap<String, f32> = tf
        .into_iter()
        .map(|(t, f)| {
            let w = f * idf.get(&t).copied().unwrap_or(0.0);
            (t, w)
        })
        .collect();
    // Summed in a FIXED order. `vec.values()` iterates a HashMap, whose order
    // varies per instance, and float addition is not associative — so the norm
    // (and therefore every weight, and therefore every score) drifted in the last
    // decimal place between two runs of the same query. See `cosine`.
    let mut squares: Vec<f32> = vec.values().map(|v| v * v).collect();
    squares.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let norm: f32 = squares.into_iter().sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.values_mut() {
            *v /= norm;
        }
    }
    vec
}

/// Cosine similarity of two L2-normalized sparse vectors (= dot product).
/// Cosine of a query (as a SORTED slice) against a document vector.
///
/// The query side is a sorted slice, not a HashMap, so the summation order is
/// fixed. Float addition is not associative, and `HashMap` iteration order
/// varies per map instance — so summing over one made the same query score
/// microscopically differently between two calls. That is a real problem, not a
/// test nit: `SEMANTIC_FLOOR` gates on this number, so a borderline paraphrase
/// could be suggested on one run and dropped on the next, from identical input.
/// Live software has to be predictable.
fn cosine(query: &[(String, f32)], doc: &HashMap<String, f32>) -> f32 {
    query
        .iter()
        .filter_map(|(t, qv)| doc.get(t).map(|dv| qv * dv))
        .sum()
}

#[cfg(test)]
mod tests {

    use super::*;

    fn one(text: &str) -> RefMatch {
        let v = detect_direct(text);
        assert_eq!(v.len(), 1, "expected exactly one match in {text:?}: {v:?}");
        v.into_iter().next().unwrap()
    }

    fn refeq(m: &RefMatch, book: &str, ch: i64, vs: i64) {
        assert_eq!(
            m.reference,
            VerseRef {
                book: book.into(),
                chapter: ch,
                verse: vs
            }
        );
    }

    #[test]
    fn digit_colon_form() {
        let m = one("turn with me to John 3:16 this morning");
        refeq(&m, "John", 3, 16);
        assert!(m.confidence >= 0.95);
    }

    #[test]
    fn spoken_words_three_sixteen_is_not_nineteen() {
        // The crux: "three sixteen" = 3:16, not 19.
        refeq(&one("john three sixteen"), "John", 3, 16);
    }

    #[test]
    fn chapter_verse_keywords() {
        let m = one("romans chapter eight verse twenty-eight");
        refeq(&m, "Romans", 8, 28);
        assert!(m.confidence >= 0.95);
    }

    #[test]
    fn bare_digits_two_tokens() {
        refeq(&one("psalm 23 1"), "Psalms", 23, 1);
    }

    /// A preacher who never says the word "verse" must still reach the screen.
    /// ASR renders the pauses as commas and full stops; `normalize` strips them,
    /// so all of these are the same bare pair — and all must clear the default
    /// auto-fire line (0.50), not merely be offered as a suggestion.
    #[test]
    fn spoken_bare_pairs_auto_fire() {
        for (text, book, ch, vs) in [
            ("psalm 23 1", "Psalms", 23, 1),
            ("Acts 2, 1.", "Acts", 2, 1),
            ("John, 3, 16.", "John", 3, 16),
            ("Romans 8, 1", "Romans", 8, 1),
        ] {
            let m = one(text);
            refeq(&m, book, ch, vs);
            assert!(
                m.confidence > 0.50,
                "{text:?} scored {:.2} — at or below the default auto-fire line, so a \
                 preacher who never says \"verse\" would never reach the screen",
                m.confidence
            );
        }
    }

    /// The live-rehearsal regression this demotion exists for. "Psalms 2, 3, 1"
    /// parses 2:3 and leaves a stray "1" no range can absorb — the numbers did not
    /// line up, so it must reach the OPERATOR, never the congregation.
    ///
    /// Reintroduce the bug (drop the `trailing_number` guard) and this fails.
    #[test]
    fn a_garbled_number_run_never_auto_fires() {
        let m = one("Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,");
        refeq(&m, "Psalms", 2, 3);
        assert!(
            m.confidence < 0.50,
            "garbled '2, 3, 1' scored {:.2} — this is the transcript that put \
             Psalms 2:3 on the wall unasked",
            m.confidence
        );
    }

    /// A repaired (misheard) book name plus loose digits is two guesses stacked:
    /// 0.55 - 0.06 = 0.49, just under the line. Always asks a human.
    #[test]
    fn a_repaired_book_with_bare_digits_still_asks_a_human() {
        let m = one("psam 23 1"); // "psam" → Psalms, a Levenshtein repair
        refeq(&m, "Psalms", 23, 1);
        assert!(
            m.confidence < 0.50,
            "repaired book + bare digits scored {:.2}",
            m.confidence
        );
    }

    // ---- A1/A2: whole-chapter references and verse ranges ------------------

    #[test]
    fn whole_chapter_reference_anchors_verse_one() {
        // "Psalm 23" (no verse) → display verse 1, flagged as a whole chapter.
        let m = one("turn to psalm 23");
        refeq(&m, "Psalms", 23, 1);
        assert!(m.whole_chapter);
        assert_eq!(m.verse_end, None);
        // Surfaces as a SUGGESTION, not a forced auto-fire.
        //
        // Asserted against the real gate, not a magic number. This test used to
        // say `< 0.90` and pass at 0.83 — which auto-fires at the 0.50 default
        // bar, the exact opposite of what the line above it claims. The comment
        // was right and the assertion could not fail.
        assert!(
            m.confidence < crate::router::Thresholds::default().auto_fire,
            "a keyword-less whole chapter must not reach the congregation unasked \
             (conf {} vs auto bar {})",
            m.confidence,
            crate::router::Thresholds::default().auto_fire
        );
    }

    #[test]
    fn whole_chapter_with_keyword_is_more_confident() {
        let m = one("psalm chapter 23");
        refeq(&m, "Psalms", 23, 1);
        assert!(m.whole_chapter);
        // "chapter" is proof of referential intent — nobody says it by accident —
        // so THIS one may still fire on its own.
        assert!(m.confidence >= crate::router::Thresholds::default().auto_fire);
    }

    /// A REFERENCE CUT OFF BEFORE ITS VERSE NUMBER IS NOT A WHOLE-CHAPTER
    /// REFERENCE.
    ///
    /// Measured, not imagined: `stt::bench::engine_shootout` drives real audio
    /// through the real pipeline, and because detection runs on every PARTIAL
    /// hypothesis of a window re-decoded once a second, every spoken reference is
    /// parsed at least once before its number has arrived. A clip citing Romans
    /// 8:28 and John 3:16 auto-fired **John 3:1** and **Romans 8:1** to the wall on
    /// the way to the right answers.
    #[test]
    fn a_transcript_that_stops_at_verse_does_not_invent_verse_one() {
        for text in [
            "and we read again in john chapter 3 verse",
            "turn with me in your bibles to romans 8 verse",
            "romans chapter 8 verses",
            "let us read psalm 23 v",
            "john 3:",
            // SINGLE-CHAPTER BOOKS take a different branch in `parse_reference`,
            // and the first version of this guard missed it entirely — where the
            // truncated reading scored 0.95, higher than the 0.88 on the path that
            // was fixed. Jude, Philemon, Obadiah, 2 John, 3 John.
            "turn to jude chapter 1 verse",
            "2 john chapter 1 verse",
            "philemon chapter 1 verse",
            "jude 1:",
        ] {
            assert!(
                detect_direct(text).is_empty(),
                "{text:?} is a reference cut off mid-sentence, not a whole chapter — \
                 got {:?}",
                detect_direct(text)
                    .iter()
                    .map(|m| format!(
                        "{} {}:{} @{:.2}",
                        m.reference.book, m.reference.chapter, m.reference.verse, m.confidence
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }

    /// The dangling marker was not merely tolerated, it was PROMOTING the mistake.
    ///
    /// A bare "Romans 8" scores 0.45 and asks a human. The trailing "verse" set the
    /// keyword bonus, which bought the *less* complete parse 0.88 — straight past
    /// the 0.50 auto bar. The most truncated reading of the sentence outranked the
    /// honest one.
    #[test]
    fn a_dangling_verse_marker_cannot_buy_a_promotion_to_auto_fire() {
        let bar = crate::router::Thresholds::default().auto_fire;
        // The honest, complete form still fires.
        let full = one("turn to john chapter 3 verse 16");
        refeq(&full, "John", 3, 16);
        assert!(full.confidence >= bar);
        // The truncated form reaches nothing at all.
        assert!(detect_direct("turn to john chapter 3 verse").is_empty());
    }

    /// The guard must fire on the MISSING NUMBER, not on the word "verse". These
    /// all carry a verse number and must be untouched — this is what stops the fix
    /// above from being a silent recall regression.
    #[test]
    fn a_verse_marker_with_its_number_is_unaffected() {
        refeq(&one("john chapter 3 verse 16"), "John", 3, 16);
        refeq(&one("psalm 23 verse 1"), "Psalms", 23, 1);
        refeq(&one("john 3:16"), "John", 3, 16);
        refeq(&one("jude verse 4"), "Jude", 1, 4);
        // Single-chapter books, both shapes — the branch the guard also covers.
        refeq(&one("jude chapter 1 verse 4"), "Jude", 1, 4);
        refeq(&one("jude 4"), "Jude", 1, 4);
        refeq(&one("2 john chapter 1 verse 3"), "2 John", 1, 3);
        // A range still parses: "verses" then a number.
        let r = one("psalm 23 verses 1 to 6");
        refeq(&r, "Psalms", 23, 1);
        assert_eq!(r.verse_end, Some(6));
        // And a genuine whole chapter — no verse marker anywhere — is untouched.
        assert!(one("turn to psalm 23").whole_chapter);
        assert!(one("psalm chapter 23").whole_chapter);
    }

    /// Truncation must not swallow the rest of the sentence. `detect_direct`'s
    /// scanner advances a token on a failed parse, so a later, complete reference
    /// in the same window still lands — which is the whole reason returning None
    /// here is safe rather than lossy.
    #[test]
    fn a_truncated_reference_does_not_hide_a_complete_one_after_it() {
        let ms = detect_direct("romans 8 verse and then we turn to john chapter 3 verse 16");
        assert_eq!(ms.len(), 1, "got {ms:?}");
        refeq(&ms[0], "John", 3, 16);
    }

    /// `at_tail` is the fact `emit_detections` gates the whole-chapter guard on: it
    /// must mean "nothing followed this reference", and nothing more.
    #[test]
    fn at_tail_marks_only_a_reference_with_nothing_after_it() {
        assert!(one("and we read again in john chapter 3").at_tail);
        assert!(one("john 3:16").at_tail);
        // Trailing punctuation is not a word — `normalize` drops it, so a reference
        // ending a sentence is still at the tail. This is the shape the guard is
        // actually for: whisper emits "…John chapter 3." mid-utterance.
        assert!(one("and we read again in john chapter 3.").at_tail);
        // Something genuinely follows.
        assert!(!one("john chapter 3 verse 16 for god so loved the world").at_tail);
        assert!(!one("john 3:16 is the verse").at_tail);
        // A range is absorbed before the tail test, so the tail is past the range.
        let r = one("psalm 23 verses 1 to 6");
        assert_eq!(r.verse_end, Some(6));
        assert!(r.at_tail);
    }

    /// The guard is deliberately narrow, and THIS is the test that holds it narrow.
    ///
    /// A complete reference at the tail must keep firing instantly — that is the
    /// common case, and suppressing every tail match on a partial would delay
    /// essentially every auto-fire by about a second, trading this bug for a latency
    /// regression against SPEC's 3-second budget. Widen `is_provisional` past
    /// whole-chapter-at-tail and this fails.
    #[test]
    fn a_complete_reference_at_the_tail_is_not_treated_as_provisional() {
        let m = one("turn with me to john chapter 3 verse 16");
        assert!(m.at_tail, "precondition: it does end the text");
        assert!(
            !m.is_provisional(false),
            "a finished reference is not provisional just because it ends the window"
        );
        assert!(m.confidence >= crate::router::Thresholds::default().auto_fire);
    }

    /// `is_provisional` is the one place the rule lives — `emit_detections` and
    /// `stt::bench::engine_shootout` both call it — so its truth table is pinned
    /// here rather than inferred from either caller.
    #[test]
    fn only_a_growing_whole_chapter_at_the_tail_is_provisional() {
        // The shape the guard exists for: whole chapter, nothing after, still growing.
        let partial = one("and we read again in john chapter 3");
        assert!(partial.whole_chapter && partial.at_tail);
        assert!(partial.is_provisional(false));

        // Same words, but the utterance CLOSED. The preacher meant the chapter.
        assert!(
            !partial.is_provisional(true),
            "a final transcript will not grow — this must be allowed through"
        );

        // Whole chapter, but more text followed, so it was never truncated.
        let midtext = one("we read john chapter 3 and then we prayed");
        assert!(midtext.whole_chapter && !midtext.at_tail);
        assert!(!midtext.is_provisional(false));

        // Not a whole chapter at all.
        assert!(!one("john 3:16").is_provisional(false));
    }

    /// THE regression from the live service of 2026-07-26.
    ///
    /// The rolling STT window is decoded about once a second, so one utterance is
    /// parsed repeatedly at varying completeness. A preacher genuinely preaching
    /// Hebrews 4:2 produced, five seconds apart:
    ///
    ///     Hebrews 4:2  0.55   ← the whole reference was heard
    ///     Hebrews 4:1  0.83   ← only "Hebrews four" survived that pass
    ///
    /// The less complete parse outranked the complete one and replaced the right
    /// verse on the wall with the wrong one. A partial hearing of a reference may
    /// never outrank a full one.
    #[test]
    fn a_bare_chapter_never_outranks_the_full_reference_it_is_a_fragment_of() {
        let full = one("hebrews four two");
        refeq(&full, "Hebrews", 4, 2);
        let fragment = one("hebrews four");
        refeq(&fragment, "Hebrews", 4, 1);
        assert!(
            fragment.confidence < full.confidence,
            "the fragment ({}) outscored the full reference ({}) — it would \
             overwrite the correct verse on the projector",
            fragment.confidence,
            full.confidence
        );
    }

    /// THE SCREENSHOT BUG, live 2026-07-26. Whisper runs "six sixty-three"
    /// together into `663`, the parser read it as a chapter, and the operator was
    /// offered `John 663:1`. Every one of these was a verse they then fired BY
    /// HAND — Relay heard the reference correctly and mangled the number.
    #[test]
    fn a_run_together_chapter_verse_is_repaired_not_read_as_a_chapter() {
        for (text, book, ch, vs) in [
            ("john 663", "John", 6, 63),
            ("hebrews 416", "Hebrews", 4, 16),
            ("mark 1124", "Mark", 11, 24),
            ("romans 828", "Romans", 8, 28),
            ("john 1623", "John", 16, 23),
            ("john 316", "John", 3, 16),
            ("psalms 1191", "Psalms", 119, 1),
            ("matthew 2820", "Matthew", 28, 20),
        ] {
            let m = one(text);
            refeq(&m, book, ch, vs);
            assert!(!m.whole_chapter, "{text:?} is a verse, not a whole chapter");
        }
    }

    /// AND IT MUST STOP THERE. A run that is a real chapter of the book stays a
    /// whole-chapter reference — splitting "Psalm 23" into 2:3 would be the same
    /// class of bug pointing the other way.
    #[test]
    fn a_real_chapter_is_never_split_into_chapter_and_verse() {
        for (text, book, ch) in [
            ("psalm 23", "Psalms", 23),
            ("genesis 11", "Genesis", 11),
            ("psalm 119", "Psalms", 119),
            ("revelation 21", "Revelation", 21),
        ] {
            let m = one(text);
            refeq(&m, book, ch, 1);
            assert!(m.whole_chapter, "{text:?} must stay a whole chapter");
        }
    }

    /// An ambiguous split is refused rather than guessed — the same rule
    /// `fuzzy_book` follows. `Psalms 1015` is both 101:5 and 10:15, and both are
    /// real verses; there is no evidence to choose between them.
    #[test]
    fn an_ambiguous_run_is_refused_rather_than_guessed() {
        assert_eq!(split_run_into_chapter_verse("Psalms", 1015), None);
        // A run with no valid reading at all is also refused.
        assert_eq!(split_run_into_chapter_verse("Romans", 8128), None);
        // And an unknown book cannot be repaired.
        assert_eq!(split_run_into_chapter_verse("Nowhere", 663), None);
    }

    /// The verse-count table is a `const`, so nothing forces it to match the
    /// Bible actually shipped. This does. If `kjv.json` is ever replaced and the
    /// table is not regenerated, `split_run_into_chapter_verse` starts inventing
    /// references — silently, and only for the books that changed.
    #[test]
    fn the_verse_count_table_matches_the_bundled_kjv() {
        #[derive(serde::Deserialize)]
        struct KjvBook {
            chapters: Vec<Vec<String>>,
        }
        const RAW: &str = include_str!("../data/kjv.json");
        let books: Vec<KjvBook> =
            serde_json::from_str(RAW.trim_start_matches('\u{feff}')).expect("kjv.json parses");

        assert_eq!(books.len(), VERSES_PER_CHAPTER.len(), "book count");
        assert_eq!(books.len(), CANONICAL_BOOKS.len(), "book count vs names");
        for (i, book) in books.iter().enumerate() {
            let name = CANONICAL_BOOKS[i];
            assert_eq!(
                book.chapters.len(),
                VERSES_PER_CHAPTER[i].len(),
                "{name}: chapter count"
            );
            for (c, chapter) in book.chapters.iter().enumerate() {
                assert_eq!(
                    chapter.len(),
                    VERSES_PER_CHAPTER[i][c] as usize,
                    "{name} chapter {}: verse count",
                    c + 1
                );
            }
        }
        // Spot-check the lookups the repair actually depends on.
        assert_eq!(chapter_count("John"), 21);
        assert_eq!(chapter_count("Hebrews"), 13);
        assert_eq!(verse_count("John", 6), 71);
        assert_eq!(verse_count("Psalms", 119), 176);
        assert_eq!(verse_count("John", 22), 0, "out of range");
    }

    /// Every single-chapter book is also an ordinary word or a name in English
    /// preaching — Jude, Philemon, Obadiah, and the "John" inside 2/3 John. From
    /// the live service of 2026-07-26 these auto-fired to a congregation off
    /// ordinary speech: Jude 1:1, Jude 1:2, 2 John 1:2, 2 John 1:3.
    #[test]
    fn a_single_chapter_book_and_a_bare_number_asks_a_human_but_a_keyword_fires() {
        let auto_bar = crate::router::Thresholds::default().auto_fire;

        let bare = one("jude four");
        refeq(&bare, "Jude", 1, 4);
        assert!(
            bare.confidence < auto_bar,
            "bare 'jude four' at {} would reach the congregation unasked",
            bare.confidence
        );

        // Stating the intent still fires — that is the whole distinction.
        let kw = one("jude verse four");
        refeq(&kw, "Jude", 1, 4);
        assert!(
            kw.confidence >= auto_bar,
            "'jude verse four' is an explicit reference and must still fire ({})",
            kw.confidence
        );
    }

    /// Ordinary preaching that is NOT a reference, from the same live service.
    /// Every one of these auto-fired to a congregation.
    #[test]
    fn a_book_name_followed_by_a_spoken_number_does_not_fire_on_its_own() {
        let auto_bar = crate::router::Thresholds::default().auto_fire;
        for text in [
            "matthew one of the twelve disciples",
            "and the lord said to john two of them",
            "job one of the oldest books",
        ] {
            for m in detect_direct(text) {
                assert!(
                    m.confidence < auto_bar,
                    "{text:?} → {} {}:{} at {} would reach the congregation unasked",
                    m.reference.book,
                    m.reference.chapter,
                    m.reference.verse,
                    m.confidence
                );
            }
        }
    }

    #[test]
    fn hyphen_range_colon_form() {
        // "Psalm 23:1-6" tokenizes to "23:1" + "6"; range end = 6.
        let m = one("psalm 23:1-6");
        refeq(&m, "Psalms", 23, 1);
        assert_eq!(m.verse_end, Some(6));
    }

    #[test]
    fn hyphen_range_bare_digits() {
        let m = one("ps 23 1-6");
        refeq(&m, "Psalms", 23, 1);
        assert_eq!(m.verse_end, Some(6));
    }

    #[test]
    fn spoken_range_with_connector() {
        let m = one("john three sixteen to eighteen");
        refeq(&m, "John", 3, 16);
        assert_eq!(m.verse_end, Some(18));
    }

    #[test]
    fn descending_range_is_rejected() {
        // A trailing smaller number is not a valid range end (kept single).
        let m = one("john 3:16 to 2");
        refeq(&m, "John", 3, 16);
        assert_eq!(m.verse_end, None);
    }

    #[test]
    fn context_walks_whole_chapter_then_stops_at_span_end() {
        let mut ctx = ContextMemory::default();
        // Fire Psalm 23 as a whole chapter of 6 verses.
        ctx.note_passage(
            &VerseRef {
                book: "Psalms".into(),
                chapter: 23,
                verse: 1,
            },
            Some(6),
        );
        // Walk 1→2→…→6, preserving the span at each step.
        for expected in 2..=6 {
            let n = ctx.next_verse().expect("should have a next verse");
            assert_eq!(n.verse, expected);
            ctx.advance(&n);
        }
        // At verse 6 (span end) → no further next.
        assert!(ctx.next_verse().is_none());
    }

    // ---- A3/A4: topical / cross-reference suggestions ---------------------

    #[test]
    fn topical_theme_surfaces_related_scripture() {
        let s = suggest_related("i have been so afraid and anxious lately", None, 4).unwrap();
        assert_eq!(s.theme, "Fear & Anxiety");
        assert!(!s.refs.is_empty());
        // Ranges in the concordance carry through as verse_end.
        assert!(s.refs.iter().any(|m| m.verse_end.is_some()));
    }

    #[test]
    fn related_excludes_the_anchor_verse() {
        let anchor = VerseRef {
            book: "John".into(),
            chapter: 3,
            verse: 16,
        };
        let s = suggest_related("God's great love for us", Some(&anchor), 4).unwrap();
        assert!(!s.refs.iter().any(|m| m.reference == anchor));
    }

    #[test]
    fn no_theme_no_suggestion() {
        assert!(suggest_related("the quarterly budget meeting is on tuesday", None, 4).is_none());
    }

    // ---- D3: spoken clear / blackout command ------------------------------

    #[test]
    fn detects_clear_and_blackout_commands() {
        assert!(detect_clear("clear the screen"));
        assert!(detect_clear("blank the screens"));
        assert!(detect_clear("blackout"));
        assert!(detect_clear("black out"));
        assert!(detect_clear("take it down"));
    }

    #[test]
    fn clear_command_ignores_prose() {
        // "clear"/"blank" without a screen object must not fire mid-sermon.
        assert!(!detect_clear("the gospel makes it very clear to us"));
        assert!(!detect_clear("make it clear"));
        assert!(!detect_clear("a blank page before creation"));
    }

    // ---- In-passage voice jump: "chapter 5 verse 1" resolves in current book ----

    #[test]
    fn passage_nav_chapter_and_verse() {
        assert_eq!(
            detect_passage_nav("chapter 5 verse 1"),
            Some(PassageNav {
                chapter: Some(5),
                verse: Some(1)
            })
        );
        assert_eq!(
            detect_passage_nav("let's go to chapter fifty verse two"),
            Some(PassageNav {
                chapter: Some(50),
                verse: Some(2)
            })
        );
        // Chapter only → the caller defaults to verse 1.
        assert_eq!(
            detect_passage_nav("go to chapter 5"),
            Some(PassageNav {
                chapter: Some(5),
                verse: None
            })
        );
    }

    #[test]
    fn passage_nav_bare_verse_singular_and_plural() {
        // Both "verse" and "verses" — stay in the same chapter, change the verse.
        assert_eq!(
            detect_passage_nav("verse 4"),
            Some(PassageNav {
                chapter: None,
                verse: Some(4)
            })
        );
        assert_eq!(
            detect_passage_nav("verses 4"),
            Some(PassageNav {
                chapter: None,
                verse: Some(4)
            })
        );
    }

    #[test]
    fn passage_nav_ignores_named_book_and_prose() {
        assert!(detect_passage_nav("turn to psalm 5 verse 1").is_none()); // book named
        assert!(detect_passage_nav("the fifth chapter of our lives").is_none()); // no number
        assert!(detect_passage_nav("we had over five hundred members today").is_none());
        // no keyword
    }

    #[test]
    fn single_verse_note_clears_any_prior_span() {
        let mut ctx = ContextMemory::default();
        ctx.note_passage(
            &VerseRef {
                book: "John".into(),
                chapter: 3,
                verse: 16,
            },
            Some(18),
        );
        // A fresh single-verse detection resets the passage — next is unbounded.
        ctx.note(&VerseRef {
            book: "Romans".into(),
            chapter: 8,
            verse: 28,
        });
        assert_eq!(ctx.next_verse().unwrap().verse, 29);
    }

    #[test]
    fn psalm_alias_and_words() {
        refeq(
            &one("open your bibles to psalm twenty three verse four"),
            "Psalms",
            23,
            4,
        );
    }

    #[test]
    fn phonetic_free_for_three_lowers_confidence() {
        let m = one("john free sixteen");
        refeq(&m, "John", 3, 16);
        assert!(
            m.confidence < 0.90,
            "phonetic correction should cost confidence"
        );
    }

    #[test]
    fn hundreds() {
        refeq(
            &one("psalm one hundred nineteen verse one"),
            "Psalms",
            119,
            1,
        );
    }

    #[test]
    fn genesis_one_one() {
        refeq(&one("in genesis one one god created"), "Genesis", 1, 1);
    }

    #[test]
    fn two_references_in_one_window() {
        let v = detect_direct("as john 3:16 says and also romans eight twenty eight");
        assert_eq!(v.len(), 2);
        refeq(&v[0], "John", 3, 16);
        refeq(&v[1], "Romans", 8, 28);
    }

    #[test]
    fn no_false_positive_on_plain_prose() {
        assert!(detect_direct("for God so loved the world that he gave").is_empty());
    }

    #[test]
    fn book_without_numbers_is_ignored() {
        assert!(detect_direct("the gospel of john tells us").is_empty());
    }

    #[test]
    fn matched_text_is_captured() {
        let m = one("see John 3:16 now");
        assert_eq!(m.matched_text, "john 3:16");
    }

    #[test]
    fn numbered_books_spoken_and_written() {
        refeq(
            &one("turn to first corinthians thirteen four"),
            "1 Corinthians",
            13,
            4,
        );
        refeq(&one("2 timothy 3:16"), "2 Timothy", 3, 16);
        refeq(&one("second peter one twenty one"), "2 Peter", 1, 21);
    }

    #[test]
    fn multiword_and_variant_book_names() {
        refeq(&one("song of solomon two one"), "Song of Solomon", 2, 1);
        refeq(&one("revelations 22:21"), "Revelation", 22, 21);
    }

    #[test]
    fn single_chapter_books_bare_verse() {
        // Bare verse → chapter 1.
        refeq(&one("look at jude four"), "Jude", 1, 4);
        refeq(&one("jude 4"), "Jude", 1, 4);
        refeq(&one("philemon verse six"), "Philemon", 1, 6);
        refeq(&one("second john four"), "2 John", 1, 4);
        // Explicit forms still respected.
        refeq(&one("jude 1:4"), "Jude", 1, 4);
        refeq(&one("obadiah verse twenty one"), "Obadiah", 1, 21);
    }

    #[test]
    fn single_chapter_book_without_number_is_ignored() {
        assert!(detect_direct("the epistle of jude warns us").is_empty());
    }

    // --- context memory ---

    #[test]
    fn bare_verse_resolves_against_current_passage() {
        let mut ctx = ContextMemory::default();
        assert!(ctx.resolve_bare_verse(4).is_none()); // nothing yet
        ctx.note(&VerseRef {
            book: "Psalms".into(),
            chapter: 23,
            verse: 1,
        });
        assert_eq!(detect_bare_verses("now look at verse four"), vec![4]);
        assert_eq!(
            ctx.resolve_bare_verse(4).unwrap(),
            VerseRef {
                book: "Psalms".into(),
                chapter: 23,
                verse: 4
            }
        );
    }

    #[test]
    fn detect_bare_verses_reads_digits_and_words() {
        assert_eq!(
            detect_bare_verses("verse 4 and verse twenty-eight"),
            vec![4, 28]
        );
        assert!(detect_bare_verses("no reference here").is_empty());
    }

    #[test]
    fn nav_commands() {
        assert_eq!(detect_command("next"), Some(NavCommand::Next));
        assert_eq!(detect_command("go to the next"), Some(NavCommand::Next));
        assert_eq!(detect_command("back please"), Some(NavCommand::Previous));
        assert_eq!(detect_command("previous verse"), Some(NavCommand::Previous));
        // Long sentences are not treated as commands.
        assert_eq!(
            detect_command("and the next thing he said in his sermon"),
            None
        );
    }

    #[test]
    fn context_next_prev() {
        let mut ctx = ContextMemory::default();
        assert!(ctx.next_verse().is_none());
        ctx.note(&VerseRef {
            book: "John".into(),
            chapter: 3,
            verse: 16,
        });
        assert_eq!(
            ctx.next_verse().unwrap(),
            VerseRef {
                book: "John".into(),
                chapter: 3,
                verse: 17
            }
        );
        assert_eq!(
            ctx.prev_verse().unwrap(),
            VerseRef {
                book: "John".into(),
                chapter: 3,
                verse: 15
            }
        );
        ctx.note(&VerseRef {
            book: "Jude".into(),
            chapter: 1,
            verse: 1,
        });
        assert!(ctx.prev_verse().is_none()); // verse 1 → no previous
    }

    #[test]
    fn ambiguous_two_digit_gives_candidates() {
        let c = detect_ambiguous("turn to revelation twenty two");
        assert!(c.contains(&VerseRef {
            book: "Revelation".into(),
            chapter: 22,
            verse: 1
        }));
        assert!(c.contains(&VerseRef {
            book: "Revelation".into(),
            chapter: 2,
            verse: 2
        }));
    }

    #[test]
    fn full_reference_is_not_ambiguous() {
        assert!(detect_ambiguous("john 3:16").is_empty());
        assert!(detect_ambiguous("romans eight twenty eight").is_empty());
    }

    // ── REPAIRING A MISHEARD BOOK NAME ──────────────────────────────────
    //
    // Asked for directly: "the audio should be ultra sensitive to African tone
    // — e.g. Sam 23 → Psalm 23". The hand-written alias list covers the
    // mishears someone thought of; this covers the ones they did not, because a
    // list can only ever be as good as the last service that surprised it.

    #[test]
    fn repairs_book_names_the_alias_list_never_listed() {
        // None of these are in the alias table. All are one or two edits from a
        // real book, and all are the kind of thing whisper emits on
        // African-accented English.
        refeq(&one("psam 23 verse 1"), "Psalms", 23, 1);
        refeq(&one("salmon 23 verse 1"), "Psalms", 23, 1);
        refeq(&one("matthews 5 verse 3"), "Matthew", 5, 3);
        refeq(&one("romands 8 verse 1"), "Romans", 8, 1);
        refeq(&one("ephesian 2 verse 8"), "Ephesians", 2, 8);
    }

    #[test]
    fn a_repaired_book_is_a_weaker_claim_than_an_exact_one() {
        // It still detects — but it must not carry the same confidence, because
        // a Direct match is the only kind allowed onto a screen unattended.
        let exact = one("psalms 23 verse 1");
        let fixed = one("psam 23 verse 1");
        assert!(
            fixed.confidence < exact.confidence,
            "a guessed book name cost nothing: exact={} repaired={}",
            exact.confidence,
            fixed.confidence
        );
    }

    #[test]
    fn ordinary_sermon_words_are_never_repaired_into_books() {
        // THE FAILURE THIS MUST NOT HAVE. "among" is two edits from "amos",
        // "same" two from "james", "gone" close to "john". A preacher saying
        // "among 3 or 4 of them" must not put Amos on the wall.
        for phrase in [
            "among 3 of them",
            "same 5 people",
            "gone 4 times",
            "good 3 things",
            "word 3 times",
            "come 2 by 2",
        ] {
            assert!(
                detect_direct(phrase).is_empty(),
                "an ordinary phrase was repaired into a reference: {phrase:?}"
            );
        }
    }

    #[test]
    fn a_repair_needs_a_reference_shaped_sentence() {
        // The gate that makes the whole thing safe: no chapter number after it,
        // no repair. "psam" on its own is just a word.
        assert!(detect_direct("psam is a lovely word").is_empty());
        assert!(detect_direct("he read from psam and sat down").is_empty());
        // ...but the same token followed by a number is a reference.
        refeq(&one("psam 23"), "Psalms", 23, 1);
    }

    #[test]
    fn an_ambiguous_repair_is_refused_rather_than_guessed() {
        // "job" and "joel" are both one edit from "joe". With no evidence to
        // choose, guessing would put one of two unrelated books on a wall.
        let hits = detect_direct("joe 2 verse 1");
        assert!(
            hits.is_empty() || hits[0].reference.book != "Job",
            "an ambiguous book repair was guessed instead of refused"
        );
    }

    #[test]
    fn very_short_tokens_are_never_repaired() {
        // At two characters everything is one edit from everything.
        assert!(detect_direct("am 3 verse 1").is_empty());
        assert!(detect_direct("is 5 verse 2").is_empty());
    }

    #[test]
    fn the_distance_bound_actually_bounds() {
        assert_eq!(edit_distance_within("psam", "psalm", 2), Some(1));
        assert_eq!(edit_distance_within("sam", "psalms", 2), None);
        assert_eq!(edit_distance_within("abc", "xyz", 2), None);
        assert_eq!(edit_distance_within("same", "same", 0), Some(0));
    }

    #[test]
    fn phonetic_book_sam_is_psalms() {
        refeq(&one("sam twenty three verse one"), "Psalms", 23, 1);
    }

    #[test]
    fn fast_search_abbreviations() {
        refeq(&one("ps 23 1"), "Psalms", 23, 1);
        refeq(&one("rom 8 1"), "Romans", 8, 1);
        refeq(&one("1 jn 3 1"), "1 John", 3, 1);
        refeq(&one("1jn 3 1"), "1 John", 3, 1);
        refeq(&one("mt 5 3"), "Matthew", 5, 3);
        refeq(&one("rev 22 1"), "Revelation", 22, 1);
        refeq(&one("2 co 5 17"), "2 Corinthians", 5, 17);
    }

    #[test]
    fn asr_possessive_book_name() {
        // whisper often mishears "Psalms 23" as "Sam's 23"; the apostrophe must
        // not split the token.
        refeq(&one("read from Sam's 23, verse 1"), "Psalms", 23, 1);
        refeq(&one("sam\u{2019}s 23 verse 1"), "Psalms", 23, 1);
    }

    // --- semantic match ---

    fn seed_index() -> SemanticIndex {
        let corpus = vec![
            (
                VerseRef { book: "John".into(), chapter: 3, verse: 16 },
                "For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish, but have everlasting life.".to_string(),
            ),
            (
                VerseRef { book: "Psalms".into(), chapter: 23, verse: 1 },
                "The LORD is my shepherd; I shall not want.".to_string(),
            ),
            (
                VerseRef { book: "Romans".into(), chapter: 8, verse: 28 },
                "And we know that all things work together for good to them that love God.".to_string(),
            ),
        ];
        SemanticIndex::build(&corpus)
    }

    #[test]
    fn semantic_matches_paraphrase_by_overlap() {
        let idx = seed_index();
        // Paraphrase of John 3:16 with shared content words.
        let hits = idx.top_k("god loved the world and gave his son so we have life", 1);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0.book, "John");
        assert!(hits[0].1 > 0.2, "similarity too low: {}", hits[0].1);
    }

    #[test]
    fn semantic_picks_shepherd_for_shepherd_query() {
        let idx = seed_index();
        let hits = idx.top_k("the lord is my shepherd", 1);
        assert_eq!(hits[0].0.reference_book_chapter_verse(), "Psalms 23:1");
    }

    #[test]
    fn semantic_empty_on_no_content_overlap() {
        let idx = seed_index();
        assert!(idx.top_k("xyzzy plugh frobnicate", 1).is_empty());
    }

    // ── KJV gloss: modern speech against a 1611 text ────────────────────────

    /// The whole point: a word that appears NOWHERE in the KJV still finds the
    /// verse. Reintroduce the un-glossed query path and this fails — "pigs" and
    /// "dad" share not one token with the text.
    #[test]
    fn gloss_finds_the_verse_through_modern_words() {
        let corpus =
            vec![
            (
                VerseRef { book: "Luke".into(), chapter: 15, verse: 16 },
                "And he would fain have filled his belly with the husks that the swine did eat."
                    .to_string(),
            ),
            (
                VerseRef { book: "Genesis".into(), chapter: 1, verse: 1 },
                "In the beginning God created the heaven and the earth.".to_string(),
            ),
        ];
        let idx = SemanticIndex::build(&corpus);
        let hits = idx.top_k("he ended up feeding pigs", 1);
        assert_eq!(hits.len(), 1, "modern wording found nothing");
        assert_eq!(hits[0].0.reference_book_chapter_verse(), "Luke 15:16");
    }

    /// The gloss ADDS evidence, never replaces it: a retelling that already uses
    /// the KJV word must not be made worse by glossing something else in it.
    #[test]
    fn gloss_keeps_the_original_token() {
        let expanded = expand_with_gloss(vec!["swine".into(), "pigs".into()]);
        assert!(expanded.contains(&"swine".to_string()));
        assert!(expanded.contains(&"pigs".to_string()));
    }

    /// THE architectural invariant. The gloss is a QUERY-time expansion only.
    /// Glossing the corpus would change document frequencies, and it is exactly
    /// how rare a word like "husks" is that makes it such a strong signal —
    /// inflating those counts would quietly degrade every other match.
    #[test]
    fn gloss_never_touches_the_index() {
        // "boat" is modern; the corpus keeps it verbatim, so a KJV-word query
        // ("ship") must NOT reach it — expansion runs one way, on the query.
        let corpus = vec![(
            VerseRef {
                book: "Mark".into(),
                chapter: 4,
                verse: 37,
            },
            "the waves beat into the boat".to_string(),
        )];
        let idx = SemanticIndex::build(&corpus);
        assert!(
            idx.top_k("ship", 1).is_empty(),
            "the index was glossed — document frequencies are now wrong"
        );
        // ...while the modern query still finds the modern text unaided.
        assert!(!idx.top_k("boat", 1).is_empty());
    }

    /// Identical input must produce a bit-identical score, every time.
    ///
    /// It did not: `cosine` summed over a HashMap, whose iteration order varies
    /// per instance, and float addition is not associative. `SEMANTIC_FLOOR`
    /// gates on this number, so a borderline paraphrase could be suggested on
    /// one run and silently dropped on the next from the same words.
    #[test]
    fn the_same_query_always_scores_the_same() {
        let idx = seed_index();
        let q = "god so loved the world that he gave his only son to have life";
        let first = idx.top_k(q, 3);
        for _ in 0..25 {
            let again = idx.top_k(q, 3);
            assert_eq!(first.len(), again.len());
            for (a, b) in first.iter().zip(again.iter()) {
                assert_eq!(a.1.to_bits(), b.1.to_bits(), "score drifted between runs");
            }
        }
    }

    /// A gloss that names an answer is a cheat, not a gloss. Story-specific
    /// proper nouns must never appear as keys, or the benchmark measures itself.
    #[test]
    fn gloss_contains_no_story_specific_giveaways() {
        for key in kjv_gloss().keys() {
            for banned in ["samaritan", "sycomore", "zacchaeus", "lazarus", "goliath"] {
                assert_ne!(key.as_str(), banned, "gloss key '{key}' names an answer");
            }
        }
    }

    /// A paraphrase match must be able to SAY WHY.
    ///
    /// Its score is a cosine, not a probability, so "61%" tells the operator nothing
    /// they can act on. The overlapping words do: an operator who sees
    /// `shepherd · lord` can agree or disagree with that in the second they have.
    #[test]
    fn a_paraphrase_can_explain_itself_in_words() {
        let idx = seed_index();
        let hits = idx.top_k_explained("the lord is my shepherd", 1);
        let (r, _score, terms) = &hits[0];
        assert_eq!(r.reference_book_chapter_verse(), "Psalms 23:1");
        assert!(
            terms.contains(&"shepherd".to_string()),
            "the rarest shared word must be shown: {terms:?}"
        );
        // Only words the query and the verse actually SHARE — an "explanation"
        // listing words that were not in the sermon would be a fabricated one.
        for t in terms {
            assert!(
                "the lord is my shepherd".contains(t.as_str()),
                "{t:?} was never spoken"
            );
        }
    }

    /// The strongest evidence comes first, and it is the word that most narrows the
    /// corpus down — not the one the operator happens to say most often. "shepherd"
    /// identifies Psalm 23; "lord" is in half the Bible and identifies nothing.
    ///
    /// This needs a corpus where "lord" is actually common, which the 3-verse
    /// `seed_index` is not: there, both words appear exactly once, their idf is
    /// identical, and the ranking is a tie broken alphabetically. The distinction
    /// being asserted here only exists at corpus scale — so build one.
    #[test]
    fn the_explanation_is_ranked_by_evidence_not_by_frequency() {
        let corpus = vec![
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 23,
                    verse: 1,
                },
                "The LORD is my shepherd; I shall not want.".to_string(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 24,
                    verse: 1,
                },
                "The earth is the LORD's, and the fulness thereof.".to_string(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 27,
                    verse: 1,
                },
                "The LORD is my light and my salvation; whom shall I fear?".to_string(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 100,
                    verse: 2,
                },
                "Serve the LORD with gladness: come before his presence with singing.".to_string(),
            ),
        ];
        let idx = SemanticIndex::build(&corpus);
        let hits = idx.top_k_explained("lord shepherd", 1);
        assert_eq!(hits[0].0.reference_book_chapter_verse(), "Psalms 23:1");
        assert_eq!(
            hits[0].2.first().map(String::as_str),
            Some("shepherd"),
            "the rare word must lead the explanation: {:?}",
            hits[0].2
        );
    }

    /// The explanation is capped, so it stays readable in a dark booth.
    #[test]
    fn the_explanation_is_short_enough_to_read_at_a_glance() {
        let idx = seed_index();
        let hits =
            idx.top_k_explained("god loved the world and gave his only begotten son life", 1);
        assert!(hits[0].2.len() <= EXPLAIN_TERMS, "{:?}", hits[0].2);
    }

    /// `top_k` must keep agreeing with `top_k_explained` — it now delegates to it,
    /// and a divergence would mean the console explains a different verse than the
    /// one the gate actually routed.
    #[test]
    fn the_explained_and_plain_rankings_cannot_diverge() {
        let idx = seed_index();
        let q = "god loved the world and gave his son";
        let plain = idx.top_k(q, 3);
        let explained = idx.top_k_explained(q, 3);
        assert_eq!(plain.len(), explained.len());
        for (p, e) in plain.iter().zip(explained.iter()) {
            assert_eq!(
                p.0.reference_book_chapter_verse(),
                e.0.reference_book_chapter_verse()
            );
            assert_eq!(p.1, e.1);
        }
    }
}

#[cfg(test)]
mod perf {
    use super::*;
    use std::time::Instant;

    /// Not an assertion — a measurement, printed with `--nocapture`.
    ///
    /// SPEC's success criterion is "runs smoothly on an 8GB Windows laptop", and
    /// `top_k` is a full linear scan over ~31k verses that runs on EVERY
    /// transcript partial (roughly once a second, while a sermon is in progress).
    /// That *looks* like something to optimise, so it was measured before anyone
    /// did.
    ///
    /// Result (release, Apple silicon):
    ///   build  ≈ 112 ms — once, at startup, off the live path
    ///   top_k  ≈ 2.6 ms per query, at ~1 query/sec
    ///
    /// That is roughly a quarter of one percent of a core. Even several times
    /// slower on a weak Windows laptop it stays around 1%. **So the linear scan
    /// stays.** An inverted index would be real complexity bought with no
    /// measurable win — the scan is not the bottleneck, and this test exists so
    /// that claim can be re-checked rather than believed.
    ///
    /// (If the corpus ever grows well beyond one translation, re-run this first.)
    #[test]
    #[ignore = "measurement, not a test — run with --ignored --nocapture"]
    fn measure_semantic_top_k() {
        // A corpus the size of the real one.
        let corpus: Vec<(VerseRef, String)> = (0..31_100)
            .map(|i| {
                (
                    VerseRef {
                        book: "John".into(),
                        chapter: (i / 100) as i64 + 1,
                        verse: (i % 100) as i64 + 1,
                    },
                    format!(
                        "for god so loved the world that he gave his only begotten son {i} \
                         whosoever believeth in him should not perish everlasting life"
                    ),
                )
            })
            .collect();

        let t0 = Instant::now();
        let idx = SemanticIndex::build(&corpus);
        let build_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let query = "god loved the world and gave his son so we would not perish";
        // Warm, then time a realistic number of queries (one per transcript partial).
        let _ = idx.top_k(query, 1);
        let t1 = Instant::now();
        const N: usize = 100;
        for _ in 0..N {
            let _ = idx.top_k(query, 1);
        }
        let per_query_ms = t1.elapsed().as_secs_f64() * 1000.0 / N as f64;

        println!("\n  SemanticIndex over {} verses:", corpus.len());
        println!("    build:     {build_ms:.0} ms (once, at startup)");
        println!("    top_k:     {per_query_ms:.2} ms per query (~1 query/sec live)");
        println!();
    }
}

#[cfg(test)]
mod tier1_languages {
    use super::*;

    fn refs(text: &str) -> Vec<String> {
        detect_direct(text)
            .iter()
            .map(|m| {
                format!(
                    "{} {}:{}",
                    m.reference.book, m.reference.chapter, m.reference.verse
                )
            })
            .collect()
    }

    /// THE test. Before the tier-1 alias table existed, every one of these
    /// returned NOTHING — a perfect Yorùbá acoustic model would have detected
    /// zero verses, because the detector had never heard of "Jòhánù".
    #[test]
    fn detects_a_verse_spoken_in_yoruba() {
        assert_eq!(refs("Jòhánù 3:16"), ["John 3:16"]);
        assert_eq!(refs("Sáàmù 23:1"), ["Psalms 23:1"]);
        assert_eq!(refs("Róòmù 8:28"), ["Romans 8:28"]);
        assert_eq!(refs("Ìfihàn 22:1"), ["Revelation 22:1"]);
    }

    #[test]
    fn detects_a_verse_spoken_in_swahili() {
        assert_eq!(refs("Yohana 3:16"), ["John 3:16"]);
        assert_eq!(refs("Zaburi 23:1"), ["Psalms 23:1"]);
        assert_eq!(refs("Warumi 8:28"), ["Romans 8:28"]);
        assert_eq!(refs("Mathayo 5:9"), ["Matthew 5:9"]);
        assert_eq!(refs("Ufunuo 22:1"), ["Revelation 22:1"]);
    }

    #[test]
    fn detects_a_verse_spoken_in_hausa() {
        assert_eq!(refs("Yahaya 3:16"), ["John 3:16"]);
        assert_eq!(refs("Zabura 23:1"), ["Psalms 23:1"]);
        assert_eq!(refs("Romawa 8:28"), ["Romans 8:28"]);
        assert_eq!(refs("Farawa 1:1"), ["Genesis 1:1"]);
    }

    /// Whisper emits tone marks unreliably — the same audio yields "Jòhánù",
    /// "Johánù" or "Johanu" depending on the recording. All must land on the same
    /// verse, or detection becomes a coin-flip on the quality of the microphone.
    #[test]
    fn tone_marks_and_dots_below_are_optional() {
        for spelling in ["Jòhánù", "Johánù", "Johanu", "JOHANU", "jòhanù"] {
            assert_eq!(
                refs(&format!("{spelling} 3:16")),
                ["John 3:16"],
                "failed on {spelling:?}"
            );
        }
        // Dots-below (Yorùbá) and the Hausa glottal both fold away.
        assert_eq!(refs("Jẹ́nẹ́sísì 1:1"), ["Genesis 1:1"]);
        assert_eq!(refs("Ru'ya ta Yohanna 22:1"), ["Revelation 22:1"]);
    }

    /// Multi-word book names must match as a unit — Swahili and Yorùbá are full
    /// of them, and a greedy single-token match would find the wrong book.
    #[test]
    fn multi_word_book_names_match_as_a_unit() {
        assert_eq!(refs("Matendo ya Mitume 2:38"), ["Acts 2:38"]);
        assert_eq!(refs("Mambo ya Walawi 19:18"), ["Leviticus 19:18"]);
        assert_eq!(refs("Ayyukan Manzanni 2:38"), ["Acts 2:38"]);
    }

    /// Numbered books, in-language.
    #[test]
    fn numbered_books_work_in_language() {
        assert_eq!(refs("1 Yohana 4:8"), ["1 John 4:8"]);
        assert_eq!(refs("2 Wakorintho 5:17"), ["2 Corinthians 5:17"]);
        assert_eq!(refs("1 Jòhánù 4:8"), ["1 John 4:8"]);
    }

    /// Code-switching is the NORMAL case, not an edge case (CLAUDE.md): a Yorùbá
    /// sermon routinely says the book in Yorùbá and the numbers in English.
    #[test]
    fn code_switching_mid_sentence_still_detects() {
        assert_eq!(refs("E jọ̀wọ́, ẹ ṣí Jòhánù 3:16"), ["John 3:16"]);
        // In-language numerals now work: "chapter three" in Swahili.
        assert_eq!(refs("Tugeukie Yohana sura ya tatu"), ["John 3:1"]);
        assert_eq!(
            refs("Let us turn to Yohana chapter 3 verse 16"),
            ["John 3:16"]
        );
    }

    /// English must not regress. The whole table is shared.
    #[test]
    fn english_still_works() {
        assert_eq!(refs("John 3:16"), ["John 3:16"]);
        assert_eq!(
            refs("turn to psalm twenty three verse one"),
            ["Psalms 23:1"]
        );
    }
}

#[cfg(test)]
mod alias_table_integrity {
    use super::*;
    use std::collections::HashMap;

    fn table() -> serde_json::Value {
        serde_json::from_str(include_str!("../data/book_aliases.json")).unwrap()
    }

    /// All three tier-1 languages must cover all 66 books. If a book is missing,
    /// Relay simply cannot hear it in that language.
    #[test]
    fn every_tier1_language_covers_all_66_books() {
        let t = table();
        for lang in ["yo", "sw", "ha"] {
            let books = t[lang].as_object().unwrap();
            let named: Vec<&str> = books
                .keys()
                .filter(|k| !k.starts_with('_'))
                .map(|s| s.as_str())
                .collect();
            let missing: Vec<&&str> = CANONICAL_BOOKS
                .iter()
                .filter(|b| !named.contains(*b))
                .collect();
            assert!(
                missing.is_empty(),
                "{lang}: {} books missing: {missing:?}",
                missing.len()
            );
            assert_eq!(named.len(), 66, "{lang} has {} books", named.len());
        }
    }

    /// THE safety test. If two different books share an alias, one of them wins
    /// arbitrarily and the other silently puts the WRONG SCRIPTURE on a wall.
    ///
    /// Cross-language collisions are the real hazard: Hausa "Mika" is Micah, and
    /// so is Swahili "Mika" — harmless, same book. But if Hausa "Luka" (Luke)
    /// collided with some other language's Luke-that-isn't, nobody would notice
    /// until a service.
    #[test]
    fn no_alias_maps_to_two_different_books() {
        let t = table();
        let mut seen: HashMap<String, (String, String)> = HashMap::new(); // alias -> (book, lang)
        for lang in ["yo", "sw", "ha"] {
            for (book, names) in t[lang].as_object().unwrap() {
                if book.starts_with('_') {
                    continue;
                }
                for n in names.as_array().unwrap() {
                    let key = normalize(n.as_str().unwrap());
                    if let Some((other_book, other_lang)) = seen.get(&key) {
                        assert_eq!(
                            other_book, book,
                            "alias {key:?} maps to BOTH {other_book} ({other_lang}) and \
                             {book} ({lang}) — one of them would put the wrong verse on a wall"
                        );
                    }
                    seen.insert(key, (book.clone(), lang.to_string()));
                }
            }
        }
    }

    /// An alias must not collide with an ENGLISH book that isn't the same book —
    /// the English table is merged into the same map.
    #[test]
    fn no_alias_hijacks_an_english_book() {
        let t = table();
        for lang in ["yo", "sw", "ha"] {
            for (book, names) in t[lang].as_object().unwrap() {
                if book.starts_with('_') {
                    continue;
                }
                for n in names.as_array().unwrap() {
                    let key = normalize(n.as_str().unwrap());
                    // If this alias is ALSO an English book name, it must be the
                    // same book. ("Amos" = "Amos" is fine. "Mark" = Luke is not.)
                    if let Some(english) = CANONICAL_BOOKS.iter().find(|b| normalize(b) == key) {
                        assert_eq!(
                            *english, book,
                            "{lang}: {key:?} is the English book {english} but is listed under {book}"
                        );
                    }
                }
            }
        }
    }

    /// Words that are also ORDINARY words must not be aliases. Yorùbá "iṣẹ́" means
    /// "work" and "orin" means "song" — in a church. An alias like that fires
    /// scripture off normal speech.
    #[test]
    fn no_alias_is_a_bare_everyday_word() {
        let t = table();
        // Known traps, deliberately excluded from the table.
        let banned = ["ise", "orin", "aiye", "oro"];
        for lang in ["yo", "sw", "ha"] {
            for (book, names) in t[lang].as_object().unwrap() {
                if book.starts_with('_') {
                    continue;
                }
                for n in names.as_array().unwrap() {
                    let key = normalize(n.as_str().unwrap());
                    assert!(
                        !banned.contains(&key.as_str()),
                        "{lang}: {key:?} (under {book}) is an everyday word — it would fire \
                         scripture off ordinary speech. Use the full book name instead."
                    );
                }
            }
        }
    }

    /// Spot-check the books a church actually reads, in every language.
    #[test]
    fn the_books_churches_actually_read_resolve() {
        let cases: &[(&str, &str)] = &[
            // Yorùbá — both translations in common use.
            ("Sáàmù 23:1", "Psalms"),
            ("Psalmu 23:1", "Psalms"),
            ("Orin Dafidi 23:1", "Psalms"),
            ("Jẹ́nẹ́sísì 1:1", "Genesis"),
            ("Genesisi 1:1", "Genesis"),
            ("Òwe 3:5", "Proverbs"),
            ("Aísáyà 40:31", "Isaiah"),
            ("Ìṣe àwọn Àpọ́sítélì 2:38", "Acts"),
            // Hausa
            ("Farawa 1:1", "Genesis"),
            ("Zabura 23:1", "Psalms"),
            ("Karin Magana 3:5", "Proverbs"),
            ("Ishaya 40:31", "Isaiah"),
            ("Ibraniyawa 11:1", "Hebrews"),
            ("Wahayin Yahaya 22:1", "Revelation"),
            ("Ru'ya ta Yohanna 22:1", "Revelation"),
            // Swahili
            ("Zaburi 23:1", "Psalms"),
            ("Mithali 3:5", "Proverbs"),
            ("Isaya 40:31", "Isaiah"),
            ("Waebrania 11:1", "Hebrews"),
        ];
        for (text, want) in cases {
            let got = detect_direct(text);
            assert_eq!(
                got.first().map(|m| m.reference.book.as_str()),
                Some(*want),
                "{text:?} should resolve to {want}, got {:?}",
                got.first().map(|m| &m.reference.book)
            );
        }
    }
}

#[cfg(test)]
mod numeral_table_integrity {
    use super::*;

    /// A wrong numeral does not fail safely — it silently shows a DIFFERENT VERSE.
    /// If "tisa" were mapped to 8 instead of 9, nobody would find out until a
    /// service. These are the cheap structural checks that catch a fat-finger.
    #[test]
    fn numeral_values_are_sane() {
        let n = numerals();
        assert!(!n.ones.is_empty() && !n.tens.is_empty());
        for (w, v) in &n.ones {
            assert!((1..=9).contains(v), "ones {w:?} = {v}, must be 1-9");
        }
        for (w, v) in &n.tens {
            assert!(
                (10..=90).contains(v) && v % 10 == 0,
                "tens {w:?} = {v}, must be a multiple of 10 in 10..=90"
            );
        }
    }

    /// A word cannot be both a number and the glue between numbers, or it would
    /// be consumed twice and change the value.
    #[test]
    fn no_word_is_both_a_number_and_a_connector() {
        let n = numerals();
        for c in &n.connectors {
            assert!(
                !n.ones.contains_key(c),
                "{c:?} is both a connector and a one"
            );
            assert!(
                !n.tens.contains_key(c),
                "{c:?} is both a connector and a ten"
            );
            assert!(!n.hundred_post.contains(c));
        }
        for l in &n.linkers {
            assert!(!n.ones.contains_key(l), "{l:?} is both a linker and a one");
            assert!(!n.tens.contains_key(l), "{l:?} is both a linker and a ten");
        }
    }

    /// The whole point of `hundred_post`. English puts the multiplier BEFORE the
    /// hundred word; Swahili and Hausa put it AFTER. Read the wrong way, "mia
    /// mbili" (200) becomes 102 — and Psalm 102 goes on the wall instead of 200.
    #[test]
    fn the_hundred_multiplier_comes_after_not_before() {
        let n = |t: &str| {
            let norm = normalize(t);
            let toks: Vec<&str> = norm.split_whitespace().collect();
            parse_number(&toks, 0).map(|(v, _, _)| v)
        };
        // Swahili
        assert_eq!(n("mia moja"), Some(100), "mia moja is 100, not 101");
        assert_eq!(n("mia mbili"), Some(200), "mia mbili is 200, NOT 102");
        assert_eq!(n("mia tano"), Some(500));
        assert_eq!(n("mia moja na kumi na tatu"), Some(113));
        // Hausa
        assert_eq!(n("dari"), Some(100));
        assert_eq!(n("dari biyu"), Some(200), "dari biyu is 200, NOT 102");
        assert_eq!(n("ɗari biyu"), Some(200), "hooked ɗ must fold");
        assert_eq!(n("dari da goma sha uku"), Some(113));
        // English is unchanged — multiplier BEFORE.
        assert_eq!(n("two hundred"), Some(200));
        assert_eq!(n("one hundred and thirteen"), Some(113));
    }

    #[test]
    fn tens_and_units_join_correctly() {
        let n = |t: &str| {
            let norm = normalize(t);
            let toks: Vec<&str> = norm.split_whitespace().collect();
            parse_number(&toks, 0).map(|(v, _, _)| v)
        };
        assert_eq!(n("kumi na tatu"), Some(13)); // sw teens
        assert_eq!(n("ishirini na tatu"), Some(23)); // sw tens
        assert_eq!(n("themanini na mbili"), Some(82));
        assert_eq!(n("goma sha uku"), Some(13)); // ha teens
        assert_eq!(n("ashirin da uku"), Some(23)); // ha tens
        assert_eq!(n("tis'in da tara"), Some(99));
    }

    /// The connectors and linkers ("na", "ya", "da", "ta") are among the most
    /// common words in these languages. A bare one must never start a number, or
    /// ordinary speech would manufacture verse references.
    #[test]
    fn a_bare_connector_is_not_a_number() {
        let n = |t: &str| {
            let norm = normalize(t);
            let toks: Vec<&str> = norm.split_whitespace().collect();
            parse_number(&toks, 0).map(|(v, _, _)| v)
        };
        for w in ["na", "da", "ya", "wa", "ta", "sha", "and"] {
            assert_eq!(n(w), None, "{w:?} alone must not parse as a number");
        }
    }
}

#[cfg(test)]
mod story_search {
    use super::*;

    /// Does story-first ranking actually find the STORY a preacher is
    /// describing?
    ///
    /// ```text
    /// cargo test story_search -- --ignored --nocapture
    /// ```
    ///
    /// Ignored because it builds the full-corpus index. It exists because the
    /// shipped eval corpus is already at 100% recall and 0 wrong verses — it can
    /// catch a regression but it cannot show an improvement, and `STORY_WEIGHT`
    /// should be a measured value rather than a taste.
    ///
    /// Each case is a paraphrase of a NARRATIVE, phrased the way a preacher
    /// recalls it out loud — not a quotation. That is the case a verse-only
    /// index is worst at: no single verse carries the words, the story does.
    #[test]
    #[ignore]
    fn story_weight_measured_against_verse_only() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::migrate(&conn, true).unwrap();
        let corpus: Vec<(VerseRef, String)> = crate::db::all_verses(&conn)
            .unwrap()
            .into_iter()
            .map(|v| {
                (
                    VerseRef {
                        book: v.book,
                        chapter: v.chapter,
                        verse: v.verse,
                    },
                    v.text,
                )
            })
            .collect();
        assert!(
            corpus.len() > 30_000,
            "expected the full corpus, got {}",
            corpus.len()
        );
        let idx = SemanticIndex::build(&corpus);

        // (spoken paraphrase, book, chapter the story lives in)
        let cases: &[(&str, &str, i64)] = &[
            (
                "jesus fed the crowd with a boy's five loaves and two fishes",
                "John",
                6,
            ),
            (
                "david killed the giant with a stone and a sling",
                "1 Samuel",
                17,
            ),
            (
                "the son came home and his father ran out to meet him",
                "Luke",
                15,
            ),
            ("he rebuked the wind and the sea became calm", "Mark", 4),
            (
                "a woman touched the hem of his garment and was made whole",
                "Mark",
                5,
            ),
            (
                "the walls of the city fell down flat when they shouted",
                "Joshua",
                6,
            ),
            (
                "he was thrown into the den of lions and the mouths were shut",
                "Daniel",
                6,
            ),
        ];

        let hit = |rows: &[(VerseRef, f32)], book: &str, ch: i64| -> Option<usize> {
            rows.iter()
                .position(|(r, _)| r.book == book && r.chapter == ch)
        };

        // SWEEP the weight rather than trusting the shipped guess. The value
        // that ranks the most stories first, without pushing any off the page,
        // is the one worth shipping.
        println!("\n  weight   mean rank (missing = 6)   found-in-top-5");
        for w in [0.0f32, 0.15, 0.25, 0.35, 0.5, 0.65, 0.8] {
            let mut total = 0usize;
            let mut found = 0usize;
            for (q, book, ch) in cases {
                let rows = idx.top_k_story_weighted(q, 5, w);
                match hit(&rows, book, *ch) {
                    Some(i) => {
                        total += i + 1;
                        found += 1;
                    }
                    None => total += 6,
                }
            }
            println!(
                "  {w:<8.2} {:<24.2} {found}/{}",
                total as f32 / cases.len() as f32,
                cases.len()
            );
        }

        let mut better = 0;
        let mut worse = 0;
        println!("\n  story weight {STORY_WEIGHT} vs verse-only\n");
        for (q, book, ch) in cases {
            let story = idx.top_k_story_weighted(q, 5, STORY_WEIGHT);
            let verse = idx.top_k_story_weighted(q, 5, 0.0);
            let (a, b) = (hit(&story, book, *ch), hit(&verse, book, *ch));
            let f = |r: Option<usize>| match r {
                Some(i) => format!("#{}", i + 1),
                None => "MISSING".into(),
            };
            println!("  {q}");
            println!(
                "      target {book} {ch}    story: {:<8} verse-only: {}",
                f(a),
                f(b)
            );
            match (a, b) {
                (Some(x), Some(y)) if x < y => better += 1,
                (Some(x), Some(y)) if x > y => worse += 1,
                (Some(_), None) => better += 1,
                (None, Some(_)) => worse += 1,
                _ => {}
            }
        }
        println!(
            "\n  story-first better on {better}, worse on {worse}, of {}\n",
            cases.len()
        );
    }
}

#[cfg(test)]
mod query_repair {
    use super::*;

    fn idx() -> SemanticIndex {
        SemanticIndex::build(&[
            (
                VerseRef {
                    book: "Proverbs".into(),
                    chapter: 25,
                    verse: 11,
                },
                "A word fitly spoken is like apples of gold in pictures of silver".into(),
            ),
            (
                VerseRef {
                    book: "Exodus".into(),
                    chapter: 32,
                    verse: 4,
                },
                "and made it a molten calf of golden fashion".into(),
            ),
            (
                VerseRef {
                    book: "Psalms".into(),
                    chapter: 23,
                    verse: 1,
                },
                "The LORD is my shepherd I shall not want".into(),
            ),
        ])
    }

    #[test]
    fn repairs_a_misheard_content_word() {
        // "goden" is in no verse; today it contributes nothing at all and the
        // query is scored on whatever survives.
        let i = idx();
        let got = i.repair_query(&["goden".to_string()]);
        assert_eq!(got, vec!["golden".to_string()]);
    }

    #[test]
    fn a_misheard_word_now_finds_its_verse() {
        let i = idx();
        let hits = i.top_k("the goden calf", 2);
        assert_eq!(hits[0].0.book, "Exodus");
    }

    #[test]
    fn known_words_are_never_touched() {
        // The corpus knows "gold"; it must not become "golden" or anything else.
        let i = idx();
        assert_eq!(
            i.repair_query(&["gold".to_string()]),
            vec!["gold".to_string()]
        );
        assert_eq!(
            i.repair_query(&["shepherd".to_string()]),
            vec!["shepherd".to_string()]
        );
    }

    #[test]
    fn short_words_are_left_alone() {
        // At three characters the nearest neighbour is meaningless.
        let i = idx();
        assert_eq!(
            i.repair_query(&["xyz".to_string()]),
            vec!["xyz".to_string()]
        );
    }

    #[test]
    fn an_ambiguous_repair_is_refused() {
        // "word" and "gold" are both one edit from "wold"... construct a real
        // tie: "cald" is one edit from "calf" and from "cold"? Use the corpus we
        // have — "silver"/"sliver" style ties must not be guessed.
        let i = SemanticIndex::build(&[(
            VerseRef {
                book: "X".into(),
                chapter: 1,
                verse: 1,
            },
            "bald bold".into(),
        )]);
        // "bild" is one edit from neither; "bxld" is one from both bald and bold.
        assert_eq!(
            i.repair_query(&["bxld".to_string()]),
            vec!["bxld".to_string()]
        );
    }

    #[test]
    fn a_word_with_no_near_neighbour_is_left_unknown() {
        let i = idx();
        assert_eq!(
            i.repair_query(&["helicopter".to_string()]),
            vec!["helicopter".to_string()]
        );
    }
}

#[cfg(test)]
mod evidence_floor {
    use super::*;

    // Reported from the console: the paraphrase panel was offering a verse whose
    // whole justification was a single word under "MATCHED ON (FROM TRANSCRIPT)".
    //
    // One word is not evidence a human can weigh in the second they have to weigh
    // it. Neither the shipped eval corpus nor the story benchmark discriminates
    // this — the first is almost all DIRECT references, the second compares
    // story-vs-verse ranking at a fixed floor — so the floor is justified here,
    // against the behaviour it actually changes.

    fn idx() -> SemanticIndex {
        SemanticIndex::build(&[
            (
                VerseRef { book: "Isaiah".into(), chapter: 40, verse: 31 },
                "they that wait upon the LORD shall renew their strength they shall mount up with wings as eagles".into(),
            ),
            (
                VerseRef { book: "Leviticus".into(), chapter: 11, verse: 13 },
                "the eagle and the ossifrage and the ospray".into(),
            ),
            (
                VerseRef { book: "Psalms".into(), chapter: 23, verse: 1 },
                "The LORD is my shepherd I shall not want".into(),
            ),
        ])
    }

    /// A single COMMON word is still a coincidence with a good score, and is
    /// still refused. This is the half of the old one-word rule that survives
    /// DECISIONS.md §25.
    ///
    /// A LITERAL 2, not `MIN_EVIDENCE_TERMS`. Asserting against the constant
    /// under test is tautological — it passes at any value, including the old
    /// behaviour this exists to forbid.
    #[test]
    fn a_common_single_shared_word_is_not_offered_as_a_paraphrase() {
        // "lord" is in two of these three verses, so it names nothing in
        // particular — exactly the thin match to refuse.
        let hits = idx().top_k_explained("lord", 5);
        assert!(
            hits.iter().all(|(_, _, terms)| terms.len() >= 2),
            "a common one-word paraphrase survived: {:?}",
            hits.iter()
                .map(|(r, _, t)| (r.book.clone(), t.clone()))
                .collect::<Vec<_>>()
        );
    }

    /// The other half of §25: a word rare enough IS corroboration, because
    /// there is nowhere else in the corpus it could have come from. Without
    /// this the KJV gloss cannot work at all — a modern retelling reaches its
    /// verse through exactly one rare KJV noun ("pigs" → "swine").
    #[test]
    fn a_rare_single_shared_word_is_evidence_enough() {
        // "ossifrage" appears in exactly ONE verse in the corpus.
        let hits = idx().top_k_explained("ossifrage", 5);
        assert_eq!(hits.len(), 1, "a rare one-word match was dropped");
        assert_eq!(hits[0].0.book, "Leviticus");
        // And the operator is shown the word that did it, not a bare score.
        assert_eq!(hits[0].2, vec!["ossifrage".to_string()]);
    }

    #[test]
    fn a_corroborated_paraphrase_still_matches_and_shows_its_words() {
        // Several independent shared words: defensible on its face, and the
        // operator gets more than one word to judge.
        let hits =
            idx().top_k_explained("they shall renew their strength and mount up with wings", 3);
        assert!(!hits.is_empty(), "a well-evidenced paraphrase was dropped");
        assert_eq!(hits[0].0.book, "Isaiah");
        assert!(
            hits[0].2.len() >= 3,
            "expected several matched words, got {:?}",
            hits[0].2
        );
    }

    #[test]
    fn up_to_six_matched_words_are_surfaced() {
        // The console showed at most four. More evidence costs nothing when it
        // exists, and the UI truncates what will not fit.
        let hits = idx().top_k_explained("wait upon the lord renew strength mount wings eagles", 3);
        assert!(hits[0].2.len() > 4, "still capped low: {:?}", hits[0].2);
        assert!(hits[0].2.len() <= EXPLAIN_TERMS);
    }
}
