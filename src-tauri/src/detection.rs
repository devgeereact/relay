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

/// Book alias table. Canonical name (DB spelling) → spoken/written variants,
/// including common ASR mishears. English-first for Phase 5; the shape is ready
/// for Yoruba/Swahili/Hausa alias rows (tier-1 languages) without code changes.
/// Only the seeded books are listed — extend as the corpus grows.
const BOOKS: &[(&str, &[&str])] = &[
    ("Genesis", &["genesis", "gen"]),
    ("Psalms", &["psalms", "psalm", "palms"]),
    ("John", &["john", "jon"]),
    ("Romans", &["romans", "roman"]),
];

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
    // Aliases are single-token for the seeded books, but scan up to 3 tokens so
    // multi-word books ("song of solomon") work when added later.
    for len in (1..=3).rev() {
        if start + len > tokens.len() {
            continue;
        }
        let candidate = tokens[start..start + len].join(" ");
        for (canonical, aliases) in BOOKS {
            if aliases.iter().any(|a| *a == candidate) {
                return Some((canonical, start + len));
            }
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
}
