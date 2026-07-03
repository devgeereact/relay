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
#[derive(Debug, Clone)]
pub struct RefMatch {
    pub reference: VerseRef,
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
        // Extra spoken variants + conservative ASR mishears.
        m.insert("psalm".into(), "Psalms");
        m.insert("palms".into(), "Psalms");
        m.insert("jon".into(), "John");
        m.insert("song of songs".into(), "Song of Solomon");
        m.insert("canticles".into(), "Song of Solomon");
        m.insert("revelations".into(), "Revelation");
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

    // Combined "3:16" token.
    if let Some((ch, vs, next)) = try_colon_pair(tokens, i) {
        return Some((
            make_match(
                canonical, ch, vs, tokens, book_start, next, 0.96, used_kw, false,
            ),
            next,
        ));
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

    // Verse number.
    let (verse, after_vs, ph2) = parse_number(tokens, i)?;
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
    Some((
        make_match(
            canonical, chapter, verse, tokens, book_start, after_vs, base, used_kw, phonetic,
        ),
        after_vs,
    ))
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
        confidence: conf,
        method: DetectionMethod::Direct,
        matched_text: tokens[book_start..end].join(" "),
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

/// Tracks the "current passage" so a bare "verse 4" resolves against the
/// last-referenced book + chapter. Pure state — no IO. Fed by resolved direct
/// matches; queried when a bare verse is heard.
#[derive(Debug, Clone, Default)]
pub struct ContextMemory {
    current: Option<(String, i64)>, // (book, chapter)
}

impl ContextMemory {
    /// Update the current passage from a freshly matched reference.
    pub fn note(&mut self, r: &VerseRef) {
        self.current = Some((r.book.clone(), r.chapter));
    }

    /// Resolve a bare verse number against the current passage, if any.
    pub fn resolve_bare_verse(&self, verse: i64) -> Option<VerseRef> {
        self.current.as_ref().map(|(book, chapter)| VerseRef {
            book: book.clone(),
            chapter: *chapter,
            verse,
        })
    }

    pub fn current(&self) -> Option<&(String, i64)> {
        self.current.as_ref()
    }
}

/// Find bare verse references ("verse 4", "verse twenty-eight") in `text`.
/// Returns the verse numbers; the caller resolves them via ContextMemory.
pub fn detect_bare_verses(text: &str) -> Vec<i64> {
    let norm = normalize(text);
    let tokens: Vec<&str> = norm.split_whitespace().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if matches!(tokens[i], "verse" | "verses") {
            if let Some((n, _, _)) = parse_number(&tokens, i + 1) {
                out.push(n);
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
