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
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DetectionMethod {
    Direct,
    Semantic,
}

/// A resolved scripture reference (canonical book name as stored in the DB).
#[derive(Debug, Clone, PartialEq)]
pub struct VerseRef {
    pub book: String,
    pub chapter: i64,
    pub verse: i64,
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
    pub method: DetectionMethod,
    pub matched_text: String,
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
        m
    })
}

/// Find all direct scripture references in `text`. Returns them left-to-right.
pub fn detect_direct(text: &str) -> Vec<RefMatch> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if let Some((canonical, book_end)) = match_book(&tokens, i) {
            if let Some((m, next)) = parse_reference(&tokens, book_end, canonical, i) {
                out.push(m);
                i = next;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Lowercase, strip punctuation except the digit-pairing colon, split hyphens
/// ("twenty-eight" → two tokens), collapse whitespace.
fn normalize(text: &str) -> String {
    let mut s = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            c if c.is_alphanumeric() => s.extend(c.to_lowercase()),
            ':' => s.push(':'),
            // Apostrophes are DROPPED (not split) so ASR possessives stay one
            // token: "Sam's" → "sams" (→ Psalms), "Isaiah's" → "isaiahs".
            '\'' | '\u{2019}' => {}
            _ => s.push(' '), // hyphen, comma, period, etc. → separator
        }
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Match the longest book alias starting at `start`. Returns (canonical, index
/// just past the matched alias).
fn match_book(tokens: &[&str], start: usize) -> Option<(&'static str, usize)> {
    // Scan longest-first (up to 3 tokens) so multi-word books ("song of
    // solomon") and numbered forms ("first corinthians") match before a shorter
    // prefix would.
    for len in (1..=3).rev() {
        if start + len > tokens.len() {
            continue;
        }
        let candidate = tokens[start..start + len].join(" ");
        if let Some(&canonical) = alias_map().get(&candidate) {
            return Some((canonical, start + len));
        }
    }
    None
}

/// Parse a chapter:verse reference beginning at `idx` (just past the book).
/// `book_start` is the book's first token index, used for the matched span.
fn parse_reference(
    tokens: &[&str],
    idx: usize,
    canonical: &str,
    book_start: usize,
) -> Option<(RefMatch, usize)> {
    let mut i = idx;
    let mut used_kw = false;
    let mut phonetic = false;

    // optional "chapter" / "chap" / "ch"
    if let Some(t) = tokens.get(i) {
        if matches!(*t, "chapter" | "chap" | "ch") {
            used_kw = true;
            i += 1;
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
            if matches!(*t, "verse" | "verses" | "vs" | "v") {
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
        while let Some(t) = tokens.get(k) {
            if matches!(*t, "verse" | "verses" | "vs" | "v" | ":") {
                if *t != ":" {
                    kw2 = true;
                }
                k += 1;
            } else {
                break;
            }
        }
        if let Some((n2, after2, ph2)) = parse_number(tokens, k) {
            // Two numbers → treat as chapter:verse as spoken.
            return Some((
                make_match(
                    canonical,
                    n1,
                    n2,
                    tokens,
                    book_start,
                    after2,
                    0.92,
                    kw2,
                    ph1 || ph2,
                ),
                after2,
            ));
        }
        // Lone number → verse, chapter 1, with optional range ("Jude 4-6").
        let mut m = make_match(
            canonical, 1, n1, tokens, book_start, after1, 0.9, used_kw, ph1,
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
    while let Some(t) = tokens.get(i) {
        if matches!(*t, "verse" | "verses" | "vs" | "v" | ":") {
            if *t != ":" {
                used_kw = true;
            }
            i += 1;
        } else {
            break;
        }
    }

    // Verse number — if absent, this is a whole-chapter reference ("Psalm 23"):
    // display verse 1, stage the chapter. Moderate confidence so live detection
    // surfaces it as a suggestion (operator confirms) rather than auto-firing a
    // whole chapter unbidden; a manual push fires it straight away.
    let Some((verse, after_vs, ph2)) = parse_number(tokens, i) else {
        let base = if used_kw { 0.88 } else { 0.83 };
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

    let base = if chapter_was_digit && verse_was_digit {
        0.92
    } else {
        0.90
    };
    let mut m = make_match(
        canonical, chapter, verse, tokens, book_start, after_vs, base, used_kw, phonetic,
    );
    // Optional range end ("John 3:16-18", "Psalm 23 verses 1 to 6").
    let mut end_idx = after_vs;
    if let Some((e, after)) = parse_range_end(tokens, after_vs, verse) {
        m.verse_end = Some(e);
        end_idx = after;
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
    let conf = conf.clamp(0.5, 0.99);
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
}

/// Parse a spoken/written number starting at `start`. Returns
/// (value, next_index, phonetic_correction_applied) or None.
///
/// A finite state walk so that "three sixteen" parses as 3 (stopping before
/// "sixteen", which is a separate verse) while "twenty eight" → 28 and
/// "one hundred nineteen" → 119.
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
        Complete,
    }
    let mut state = St::Start;
    let mut value = 0i64;
    let mut idx = start;
    let mut consumed = 0;
    let mut phonetic = false;

    while let Some(&raw) = tokens.get(idx) {
        if tokens[idx].parse::<i64>().is_ok() {
            break; // a digit doesn't extend a spoken number
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
        _ => return None,
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
        if let Some((canonical, book_end)) = match_book(&tokens, i) {
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
}

impl SemanticIndex {
    /// Build the index from the corpus: (reference, verse text).
    pub fn build(corpus: &[(VerseRef, String)]) -> Self {
        let n = corpus.len().max(1) as f32;
        // Document frequency per term.
        let mut df: HashMap<String, f32> = HashMap::new();
        let tokenized: Vec<(VerseRef, Vec<String>)> = corpus
            .iter()
            .map(|(r, text)| (r.clone(), tokenize(text)))
            .collect();
        for (_, toks) in &tokenized {
            let mut seen = std::collections::HashSet::new();
            for t in toks {
                if seen.insert(t.clone()) {
                    *df.entry(t.clone()).or_insert(0.0) += 1.0;
                }
            }
        }
        let idf: HashMap<String, f32> = df
            .into_iter()
            .map(|(t, d)| (t, (n / d).ln() + 1.0))
            .collect();

        let docs = tokenized
            .into_iter()
            .map(|(r, toks)| (r, tfidf_vector(&toks, &idf)))
            .collect();

        SemanticIndex { idf, docs }
    }

    /// Top-k verses by cosine similarity to `query`, highest first. Scores are
    /// in [0, 1]; the caller maps them to confidence and applies the gate.
    pub fn top_k(&self, query: &str, k: usize) -> Vec<(VerseRef, f32)> {
        let qvec = tfidf_vector(&tokenize(query), &self.idf);
        if qvec.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(VerseRef, f32)> = self
            .docs
            .iter()
            .map(|(r, dvec)| (r.clone(), cosine(&qvec, dvec)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
    }
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
    let norm: f32 = vec.values().map(|v| v * v).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.values_mut() {
            *v /= norm;
        }
    }
    vec
}

/// Cosine similarity of two L2-normalized sparse vectors (= dot product).
fn cosine(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    // Iterate the smaller map.
    let (small, big) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    small
        .iter()
        .filter_map(|(t, av)| big.get(t).map(|bv| av * bv))
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

    // ---- A1/A2: whole-chapter references and verse ranges ------------------

    #[test]
    fn whole_chapter_reference_anchors_verse_one() {
        // "Psalm 23" (no verse) → display verse 1, flagged as a whole chapter.
        let m = one("turn to psalm 23");
        refeq(&m, "Psalms", 23, 1);
        assert!(m.whole_chapter);
        assert_eq!(m.verse_end, None);
        // Moderate confidence → surfaces as a suggestion, not a forced auto-fire.
        assert!(m.confidence < 0.90);
    }

    #[test]
    fn whole_chapter_with_keyword_is_more_confident() {
        let m = one("psalm chapter 23");
        refeq(&m, "Psalms", 23, 1);
        assert!(m.whole_chapter);
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
}

impl VerseRef {
    #[cfg(test)]
    fn reference_book_chapter_verse(&self) -> String {
        format!("{} {}:{}", self.book, self.chapter, self.verse)
    }
}
