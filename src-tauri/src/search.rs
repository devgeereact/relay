//! WHAT A TYPED QUERY MEANS, AND WHY EACH ANSWER IS HERE.
//!
//! Two questions arrive in one box — *which verse is this reference* and *which
//! verse says these words* — and they are scored separately, a reference always
//! outranking a phrase (`docs/REBRAND.md` §9, DECISIONS §72).
//!
//! This module is **pure**: no database, no IO, no clock, no state. It reads a
//! string and says what kind of claim can be made about it. The orchestration
//! that turns those claims into verses lives in `main.rs`, because that is where
//! the connection and the semantic index are; everything it *decides* lives
//! here, so there is one answer to "what does this query mean".
//!
//! ── THE RULE THAT BOUNDS THIS FILE ────────────────────────────────────────
//!
//! **Nothing here may auto-fire. A search is an operator action.** CLAUDE.md
//! rule 10 is untouched by this file and must stay that way: only
//! `DetectionMethod::Direct` may reach a wall unattended, and "Direct" means
//! Relay HEARD it. This file does approximate matching over book names and over
//! verse text, which is the same *class* of code as the `fuzzy_book` repair that
//! put **Numbers 3:16** on a wall at 0.840 from "please turn to hymn number
//! three sixteen". The only difference — and it is the whole safety argument —
//! is that a person typed this, looked at the list, and chose. So:
//!
//! - nothing in this module produces a `DetectionMatch`, a `RouteDecision` or a
//!   `Fire`, and nothing in it is reachable from `emit_detections`;
//! - `prefix_books` (below) is the widest match in the product and is
//!   **deliberately absent from `detection.rs`** — a preacher saying "am" or
//!   "is" must never resolve to Amos or Isaiah. `the_live_detector_does_not_
//!   know_about_prefixes` holds that boundary from the other side.
//!
//! Scores here are for ORDERING A LIST. They are not confidences and they never
//! cross the router.

use crate::detection;

/// Which kind of claim a hit is making. This reaches the operator (rule 18: the
/// operator must see WHICH KIND of claim is being made, not just a number).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchKind {
    /// The query parsed as a reference. Not a guess about anything.
    Reference,
    /// The query parsed as a reference only after a book PREFIX was expanded
    /// ("philipp 4 13"). Relay chose the book; the operator chose the words.
    BookPrefix,
    /// The whole query appears in the verse, verbatim and in order.
    Phrase,
    /// The verse's words cover the query (`MIN_COVERAGE`), in any order.
    Words,
    /// Close in MEANING, not in words. A TF-IDF cosine is not a probability, so
    /// this one is a guess and is coloured like one.
    Paraphrase,
}

impl MatchKind {
    /// The wire name. Stable — the rail keys its colour off this.
    pub fn wire(self) -> &'static str {
        match self {
            MatchKind::Reference => "reference",
            MatchKind::BookPrefix => "prefix",
            MatchKind::Phrase => "phrase",
            MatchKind::Words => "words",
            MatchKind::Paraphrase => "paraphrase",
        }
    }

    /// Is this hit a guess about meaning? Cyan, per the colour law, and never
    /// amber — amber is ON AIR and is never allowed to lie.
    pub fn is_guess(self) -> bool {
        matches!(self, MatchKind::Paraphrase | MatchKind::BookPrefix)
    }

    /// Where this kind sits in the list. A reference always outranks a phrase.
    ///
    /// One band per kind, with room inside each for the source's own ordering,
    /// so "which kind won" can never depend on a bm25 score drifting.
    pub fn band(self) -> f32 {
        match self {
            MatchKind::Reference => 1.0,
            MatchKind::BookPrefix => 0.96,
            MatchKind::Phrase => 0.95,
            MatchKind::Paraphrase => 0.5,
            MatchKind::Words => 0.45,
        }
    }
}

/// Why one verse is in the results — the kind of claim, and the evidence for it.
///
/// The sentence is composed HERE and not on each surface. The Library, the Live
/// rail, the Planner and the preacher's remote all read this search; a sentence
/// written four times is four sentences that will disagree.
#[derive(Debug, Clone)]
pub struct Why {
    pub kind: MatchKind,
    /// The words of the query that actually landed. Empty for a reference —
    /// there, the evidence is the reference itself.
    pub matched: Vec<String>,
    /// One line an operator can read at a glance.
    pub sentence: String,
}

impl Why {
    /// A reference the parser read straight out of what was typed.
    pub fn reference(typed: &str) -> Why {
        Why {
            kind: MatchKind::Reference,
            matched: vec![],
            sentence: format!("the reference you typed — “{}”", typed.trim()),
        }
    }

    /// A reference reached by expanding a book PREFIX. Names the book it chose,
    /// because choosing the book is the part Relay did rather than the operator.
    pub fn book_prefix(prefix: &str, book: &str) -> Why {
        Why {
            kind: MatchKind::BookPrefix,
            matched: vec![prefix.to_string()],
            sentence: format!("“{prefix}” read as {book}"),
        }
    }

    /// The whole query, verbatim, in the verse.
    pub fn phrase() -> Why {
        Why {
            kind: MatchKind::Phrase,
            matched: vec![],
            sentence: "your words, in this order".into(),
        }
    }

    /// A word-coverage hit. Says WHICH words landed, so an operator can see at
    /// once that the half of the query they cared about is the half that missed.
    pub fn words(matched: Vec<String>) -> Why {
        let list = matched.join(", ");
        Why {
            kind: MatchKind::Words,
            matched,
            sentence: if list.is_empty() {
                "matched the words you typed".into()
            } else {
                format!("matched: {list}")
            },
        }
    }

    /// A semantic hit. Says it is a guess, in words, and carries NO percentage —
    /// a cosine is not a probability and a number that lies is worse than none
    /// (CLAUDE.md rule 18).
    pub fn paraphrase() -> Why {
        Why {
            kind: MatchKind::Paraphrase,
            matched: vec![],
            sentence: "close in meaning, not in words — a guess".into(),
        }
    }
}

/// `ps23:1` → `ps 23 1`: a space wherever letters meet digits.
///
/// The reference parser reads tokens, and `ps23:1` is one token, so the fastest
/// way to type a reference was the one way that returned nothing at all. Applied
/// ONLY to the reference pass — a phrase search must keep the query a person
/// actually typed.
pub fn split_digit_runs(q: &str) -> String {
    let mut out = String::with_capacity(q.len() + 4);
    let mut prev: Option<char> = None;
    for c in q.chars() {
        if let Some(p) = prev {
            if p.is_ascii_alphabetic() && c.is_ascii_digit()
                || p.is_ascii_digit() && c.is_ascii_alphabetic()
            {
                out.push(' ');
            }
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// Every reference in the query, using the SAME parser the live pipeline uses.
///
/// Deliberately not a second parser: a second parser is a second thing that can
/// disagree with the router about what a reference is, and the disagreement
/// would show up as a verse on a wall.
pub fn references_in(q: &str) -> Vec<detection::RefMatch> {
    let direct = detection::detect_direct(q);
    if direct.is_empty() {
        detection::detect_direct(&split_digit_runs(q))
    } else {
        direct
    }
}

/// The shortest book prefix this module will expand.
///
/// Two, from `docs/REBRAND.md` §9. One letter would make every "a", "i" and "j"
/// in a phrase query a book.
pub const MIN_PREFIX: usize = 2;

/// How many books one ambiguous prefix may offer.
///
/// "jo" is a prefix of six books. Offering all of them buries the phrase results
/// under a wall of chapters nobody asked for; offering none makes a two-letter
/// prefix useless. Five is the whole of "jo" minus the least likely, and it is a
/// list length, not a threshold — nothing is refused on the strength of it.
pub const MAX_PREFIX_BOOKS: usize = 5;

/// Canonical books this token is a prefix of, in Bible order.
///
/// **Search only.** This is the widest book match in the product and it exists
/// nowhere else on purpose: `detection.rs` resolves a book from an exact alias
/// or an edit-distance repair gated on a following number (CLAUDE.md rule 10),
/// and adding prefixes there would make "am", "is", "jo" and "so" into books in
/// the middle of a sermon. Here, a person typed it and will read the answer.
///
/// An exact alias is NOT a prefix match — `ps`, `mt`, `jn`, `php` already
/// resolve through the parser, and this pass never runs when that one succeeded.
pub fn prefix_books(token: &str) -> Vec<&'static str> {
    let t = token.trim().to_lowercase();
    if t.len() < MIN_PREFIX || !t.chars().all(|c| c.is_ascii_alphabetic()) {
        return vec![];
    }
    detection::CANONICAL_BOOKS
        .iter()
        .filter(|b| {
            let lower = b.to_lowercase();
            // A numbered book ("1 John") is reached by its NAME here; the number
            // is a separate token the parser already knows how to read.
            let bare = lower
                .strip_prefix("1 ")
                .or_else(|| lower.strip_prefix("2 "))
                .or_else(|| lower.strip_prefix("3 "))
                .unwrap_or(&lower);
            (lower.starts_with(&t) || bare.starts_with(&t)) && lower != t
        })
        .copied()
        .take(MAX_PREFIX_BOOKS)
        .collect()
}

/// A query rewritten with one book prefix expanded, ready for the real parser.
///
/// Returns `(prefix_as_typed, canonical_book, rewritten_query)` per candidate
/// book. The rewrite replaces ONLY the prefix token; everything after it is left
/// exactly as typed, so the chapter/verse shape is still the parser's decision
/// and not this module's.
pub fn prefix_expansions(q: &str) -> Vec<(String, &'static str, String)> {
    let split = split_digit_runs(q);
    let tokens: Vec<&str> = split.split_whitespace().collect();
    let Some((first, rest)) = tokens.split_first() else {
        return vec![];
    };
    // A bare prefix with nothing after it is not a reference, and expanding it
    // would offer sixty-six chapter ones for "jo". The parser needs a number.
    if rest.is_empty() {
        return vec![];
    }
    // A numbered book keeps its number: "1 corint 13 4" expands `corint`.
    let (lead, prefix_tok, tail) = match *first {
        "1" | "2" | "3" | "i" | "ii" | "iii" | "first" | "second" | "third" => {
            let Some((p, t)) = rest.split_first() else {
                return vec![];
            };
            if t.is_empty() {
                return vec![];
            }
            (Some(*first), *p, t)
        }
        _ => (None, *first, rest),
    };
    prefix_books(prefix_tok)
        .into_iter()
        .map(|book| {
            let head = match lead {
                Some(n) => format!("{n} {book}"),
                None => book.to_string(),
            };
            (
                prefix_tok.to_string(),
                book,
                format!("{head} {}", tail.join(" ")),
            )
        })
        .collect()
}

/// Words that carry almost no search signal on their own. A verse matching only
/// these has not matched the query.
pub const WEAK_WORDS: &[&str] = &[
    "the", "and", "of", "a", "an", "to", "in", "is", "that", "for", "it", "with", "as", "was",
    "be", "not", "but", "they", "he", "him", "his", "her", "she", "i", "you", "me", "my", "we",
    "us", "them", "their", "our", "this", "these", "those", "there", "then", "shall", "will",
    "unto", "upon", "o", "on", "at", "by", "from", "all", "are", "were", "have", "has", "had",
];

/// Below this share of the query, a loose text hit is a guess, and the search
/// says nothing instead. Four words out of five missing is not a near miss.
pub const MIN_COVERAGE: f32 = 0.55;

/// Split on non-alphanumerics and lower-case. One definition, used by both
/// halves of the coverage calculation so they cannot disagree about a word.
fn words_of(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

/// Does this query word land on a word of the verse?
///
/// Exactly, on a shared prefix (`shep` → `shepherd`), or one edit away — which
/// is what makes a typed query survive a typo without inventing a match.
/// `one_edit_apart` is `detection.rs`'s, so "nearly the same word" has one
/// definition in the product.
fn lands(verse_words: &[String], q: &str) -> bool {
    verse_words.iter().any(|w| {
        w == q
            || (q.len() >= 4 && w.starts_with(q))
            || (q.len() >= 5 && detection::one_edit_apart(w, q))
    })
}

/// How much of the query this verse actually contains, 0.0–1.0, and which of the
/// query's words landed.
///
/// A weak word is worth 0.3 of a real one, so "the lord is my shepherd" is not
/// counted as five-fifths matched because a verse happens to contain "the", "is"
/// and "my". The landed words are returned rather than recomputed, because the
/// "why" an operator reads must be the same evidence the floor was applied to.
pub fn coverage(query: &str, text: &str) -> (f32, Vec<String>) {
    let verse_words = words_of(text);
    let mut total = 0.0f32;
    let mut hit = 0.0f32;
    let mut matched: Vec<String> = Vec::new();
    for q in words_of(query) {
        let weight = if WEAK_WORDS.contains(&q.as_str()) {
            0.3
        } else {
            1.0
        };
        total += weight;
        if lands(&verse_words, &q) {
            hit += weight;
            // Weak words are counted but not quoted back: "matched: the, is, my"
            // is not evidence of anything.
            if weight > 0.3 && !matched.contains(&q) {
                matched.push(q);
            }
        }
    }
    let share = if total <= 0.0 { 0.0 } else { hit / total };
    (share, matched)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glued_digits_become_a_reference() {
        assert_eq!(split_digit_runs("ps23:1"), "ps 23:1");
        assert_eq!(split_digit_runs("1cor13"), "1 cor 13");
        assert_eq!(split_digit_runs("ps 23 1"), "ps 23 1");
    }

    /// The acceptance clause for the reference half of §9, read through the ONE
    /// parser. Every one of these is a shape an operator actually types.
    #[test]
    fn the_reference_shapes_a_person_types() {
        let top = |q: &str| {
            references_in(q).first().map(|m| {
                format!(
                    "{} {}:{}",
                    m.reference.book, m.reference.chapter, m.reference.verse
                )
            })
        };
        assert_eq!(top("ps 23 1").as_deref(), Some("Psalms 23:1"));
        assert_eq!(top("ps23:1").as_deref(), Some("Psalms 23:1"));
        assert_eq!(top("psalm 23").as_deref(), Some("Psalms 23:1"));
        assert_eq!(top("rom 8 28").as_deref(), Some("Romans 8:28"));
        assert_eq!(top("mt 6 33").as_deref(), Some("Matthew 6:33"));
        assert_eq!(top("1 cor 13 4").as_deref(), Some("1 Corinthians 13:4"));
        // The keywords §9 says are dropped.
        assert_eq!(top("rom ch 8 v 28").as_deref(), Some("Romans 8:28"));
        assert_eq!(
            top("romans chapter 8 verse 28").as_deref(),
            Some("Romans 8:28")
        );
    }

    #[test]
    fn a_prefix_of_two_letters_or_more_names_its_book() {
        assert_eq!(prefix_books("philipp"), vec!["Philippians"]);
        assert_eq!(prefix_books("gene"), vec!["Genesis"]);
        assert_eq!(prefix_books("revela"), vec!["Revelation"]);
        // Ambiguous is offered, not resolved — in Bible order, capped.
        let jo = prefix_books("jo");
        assert!(jo.contains(&"John"), "jo → {jo:?}");
        assert!(jo.contains(&"Joshua"), "jo → {jo:?}");
        assert!(jo.len() <= MAX_PREFIX_BOOKS);
        // One letter is not a prefix, and neither is a digit.
        assert!(prefix_books("j").is_empty());
        assert!(prefix_books("23").is_empty());
        // A numbered book is reachable by its bare name.
        assert!(prefix_books("corint").contains(&"1 Corinthians"));
    }

    #[test]
    fn an_expanded_prefix_is_still_the_real_parser_s_decision() {
        let refs = |q: &str| -> Vec<String> {
            prefix_expansions(q)
                .into_iter()
                .flat_map(|(_, _, rewritten)| references_in(&rewritten))
                .map(|m| {
                    format!(
                        "{} {}:{}",
                        m.reference.book, m.reference.chapter, m.reference.verse
                    )
                })
                .collect()
        };
        assert!(refs("philipp 4 13").contains(&"Philippians 4:13".to_string()));
        assert!(refs("1 corint 13 4").contains(&"1 Corinthians 13:4".to_string()));
        // A prefix with no number after it is not a reference, so nothing is
        // offered — this is the guard that keeps a phrase query out of the
        // reference branch.
        assert!(prefix_expansions("jo").is_empty());
        assert!(refs("see ye first the kingdom").is_empty());
    }

    /// THE BOUNDARY. Prefix expansion is the widest book match in the product,
    /// and it must not exist in the live pipeline. Moving `prefix_books` into
    /// `detection::match_book` makes this test fail, which is the point.
    ///
    /// The second list is the half that matters. `am`, `is`, `so` and `jo` are
    /// ordinary English words a preacher says all morning, and each is a legal
    /// prefix HERE — "am 5 24" offers Amos 5:24 to an operator who typed it.
    /// Spoken into a sermon they must resolve to nothing at all, because a book
    /// this module is willing to guess at is exactly the guess that put
    /// **Numbers 3:16** on a wall unattended (CLAUDE.md rule 10).
    ///
    /// **What this test does NOT claim**: that no near-miss reaches the live
    /// detector. `gene` and `gala` are one edit from the existing `gen`/`gal`
    /// aliases, so `fuzzy_book` already repairs them — at 0.49, as
    /// `UncertainBook`, which rule 10 caps at `Suggest` and can never fire. That
    /// is pre-existing, correct, and a different mechanism from this one; it is
    /// named here so a future reader does not read the empty list below as a
    /// wider guarantee than it is.
    #[test]
    fn the_live_detector_does_not_know_about_prefixes() {
        for spoken in ["philipp 4 13", "revela 22 1", "corint 13 4", "thessal 4 16"] {
            assert!(
                detection::detect_direct(spoken).is_empty(),
                "the live detector resolved a book PREFIX in {spoken:?} — rule 10"
            );
        }
        for ordinary in ["am 5 24", "is 40 31", "so 3 1", "jo 3 16"] {
            // Each of these IS a prefix this module will expand for a typist…
            assert!(
                !prefix_expansions(ordinary).is_empty(),
                "{ordinary:?} stopped being a prefix — the test below proves nothing"
            );
            // …and none of them is a book when a preacher says it.
            assert!(
                detection::detect_direct(ordinary).is_empty(),
                "an ordinary word became a book in the LIVE detector: {ordinary:?}"
            );
        }
    }

    #[test]
    fn coverage_weighs_the_words_that_carry_nothing() {
        let psalm = "The LORD is my shepherd; I shall not want.";
        let (share, matched) = coverage("the lord is my shepherd", psalm);
        assert!(share >= MIN_COVERAGE, "{share}");
        // The evidence quoted back is the words that carry signal, never "the".
        assert!(matched.contains(&"lord".to_string()));
        assert!(matched.contains(&"shepherd".to_string()));
        assert!(!matched.contains(&"the".to_string()));

        // One real word in five is below the floor.
        let (junk, _) = coverage("quantum shepherd tractor engine banana", psalm);
        assert!(junk < MIN_COVERAGE, "{junk}");
    }

    #[test]
    fn a_why_says_which_kind_of_claim_it_is() {
        assert_eq!(Why::reference("ps 23 1").kind.wire(), "reference");
        assert!(!Why::reference("ps 23 1").kind.is_guess());
        assert!(Why::paraphrase().kind.is_guess());
        assert!(Why::book_prefix("philipp", "Philippians").kind.is_guess());
        // A paraphrase carries NO percentage — a cosine is not a probability.
        let p = Why::paraphrase().sentence;
        assert!(!p.contains('%'), "{p}");
        assert!(p.contains("guess"), "{p}");
        // A words hit quotes its evidence.
        let w = Why::words(vec!["lamp".into(), "feet".into()]);
        assert_eq!(w.sentence, "matched: lamp, feet");
    }

    #[test]
    fn a_reference_always_outranks_a_phrase() {
        assert!(MatchKind::Reference.band() > MatchKind::Phrase.band());
        assert!(MatchKind::BookPrefix.band() > MatchKind::Phrase.band());
        assert!(MatchKind::Phrase.band() > MatchKind::Paraphrase.band());
        assert!(MatchKind::Paraphrase.band() > MatchKind::Words.band());
    }
}
