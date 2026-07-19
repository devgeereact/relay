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
        while let Some(t) = tokens.get(k) {
            if is_verse_word(t) || *t == ":" {
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
        if is_verse_word(t) || *t == ":" {
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

    // BARE DIGITS with no "chapter"/"verse" keyword ("psalms 2 3") are a different
    // animal from "Psalm 23 verse 1", and must not be trusted the same way.
    //
    // That form exists for TYPED shorthand ("ps 23 1") — and typed input goes
    // through `manual_fire`, which bypasses the gate entirely. So demoting it here
    // costs the operator nothing, and it fixes garbled speech. A real transcript,
    // from a live rehearsal:
    //
    //     "Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,"
    //
    // scored 0.92 and put Psalms 2:3 on the wall, unasked. Nobody SAYS "Psalms two
    // three" — they say "Psalms two verse three". A bare digit pair now reaches the
    // operator, not the congregation, and a human decides.
    let bare_digits = chapter_was_digit && verse_was_digit && !used_kw;
    let base = if bare_digits {
        0.45 // below auto-fire, above suggest
    } else if chapter_was_digit && verse_was_digit {
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
    /// stem → the readable word it came from, for the operator-facing "why".
    /// Stemming is right for matching and wrong for reading: Snowball turns
    /// "belly" into "belli". Rule #18 says the operator must be able to judge
    /// the claim, and nobody can judge "belli · husk".
    surface: HashMap<String, String>,
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
        let idf: HashMap<String, f32> = df
            .into_iter()
            .map(|(t, d)| (t, (n / d).ln() + 1.0))
            .collect();

        let docs = tokenized
            .into_iter()
            .map(|(r, toks)| (r, tfidf_vector(&toks, &idf)))
            .collect();

        SemanticIndex { idf, docs, surface }
    }

    /// Top-k verses by cosine similarity to `query`, highest first. Scores are
    /// in [0, 1]; the caller maps them to confidence and applies the gate.
    pub fn top_k(&self, query: &str, k: usize) -> Vec<(VerseRef, f32)> {
        self.top_k_explained(query, k)
            .into_iter()
            .map(|(r, s, _)| (r, s))
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
        let qvec = tfidf_vector(&expand_with_gloss(stem_all(tokenize(query))), &self.idf);
        if qvec.is_empty() {
            return Vec::new();
        }
        // Sorted once per query, so every document is scored by summing the same
        // terms in the same order — identical input, identical score, every run.
        let mut qsorted: Vec<(String, f32)> = qvec.iter().map(|(t, w)| (t.clone(), *w)).collect();
        qsorted.sort_by(|a, b| a.0.cmp(&b.0));
        let mut scored: Vec<(usize, f32)> = self
            .docs
            .iter()
            .enumerate()
            .map(|(i, (_, dvec))| (i, cosine(&qsorted, dvec)))
            .filter(|(_, s)| *s > 0.0)
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);
        scored
            .into_iter()
            .map(|(i, s)| {
                let (r, dvec) = &self.docs[i];
                let why = top_terms(&qvec, dvec, EXPLAIN_TERMS)
                    .into_iter()
                    .map(|t| self.surface.get(&t).cloned().unwrap_or(t))
                    .collect();
                (r.clone(), s, why)
            })
            .collect()
    }
}

/// How many overlapping words to show as the reason for a paraphrase match. Four
/// is enough to judge it and few enough to read at a glance, in the dark.
const EXPLAIN_TERMS: usize = 4;

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
