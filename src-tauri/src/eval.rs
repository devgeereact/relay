//! Detection benchmark: does the right verse reach the screen?
//!
//! Single responsibility: score Relay's detection against a labelled corpus and
//! fail the build if it regresses.
//!
//! ## Why this exists
//!
//! `docs/SPEC.md` sets a target of a **<5% wrong-verse rate**, and until now
//! nothing anywhere checked it. The product's headline claim — that it hears
//! scripture in Yorùbá, Kiswahili and Hausa — was entirely unmeasured. You cannot
//! improve what you have never baselined, and you cannot defend a moat you cannot
//! put a number on.
//!
//! ## What it measures, and what it deliberately does not
//!
//! Two things get conflated in speech evaluation, and conflating them here would
//! have hidden the exact bug we just fixed:
//!
//! 1. **Transcription quality (WER)** — how well whisper turns audio into text.
//!    Needs real sermon audio, which does not exist yet. See `docs/LANGUAGES.md`;
//!    recording 30 minutes of it is the most valuable contribution anyone can make.
//!
//! 2. **Detection quality** — given a transcript, does the right verse reach the
//!    congregation? **This needs no audio at all.** And it is where the failure
//!    actually was: the detector spoke only English, so a *perfect* Yorùbá model
//!    would still have detected nothing.
//!
//! This harness measures (2), today, in CI.
//!
//! ## It scores through the ROUTER, not just the parser
//!
//! What matters is not what `detect_direct` found — it is what would have been
//! **put on a wall in front of a congregation**. So every case runs through the
//! real `Router`, and the headline metric is the one the product lives or dies on:
//!
//! > **False auto-fire rate** — how often Relay shows a verse nobody asked for.

use crate::detection::{self, DetectionMethod};
use crate::router::{RouteDecision, Router};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, Deserialize)]
struct Corpus {
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct Case {
    id: String,
    lang: String,
    text: String,
    /// Verses that must be found. Empty = a NEGATIVE case: nothing may fire.
    expect: Vec<String>,
    /// May be suggested, must never auto-fire (paraphrases).
    #[serde(default)]
    must_not_fire: bool,
}

/// What Relay actually did with one case.
struct Outcome {
    /// Verses that would have gone STRAIGHT TO THE SCREEN.
    auto_fired: Vec<String>,
    /// Verses offered to the operator for a decision.
    suggested: Vec<String>,
}

/// Run one case through the real detection + routing path.
fn run(case: &Case) -> Outcome {
    let mut router = Router::default();
    let mut auto_fired = Vec::new();
    let mut suggested = Vec::new();

    for m in detection::detect_direct(&case.text) {
        let key = format!(
            "{} {}:{}",
            m.reference.book, m.reference.chapter, m.reference.verse
        );
        match router.decide(&key, m.confidence, DetectionMethod::Direct, 0) {
            RouteDecision::AutoFire => auto_fired.push(key),
            RouteDecision::Suggest => suggested.push(key),
            RouteDecision::Drop => {}
        }
    }
    // Ambiguous candidates ("Revelation 22" → 22:1 or 2:2) can never auto-fire,
    // but they still reach the operator, so they belong in the score.
    if detection::detect_direct(&case.text).is_empty() {
        for r in detection::detect_ambiguous(&case.text) {
            let key = format!("{} {}:{}", r.book, r.chapter, r.verse);
            if let RouteDecision::Suggest = router.decide(&key, 0.70, DetectionMethod::Ambiguous, 0)
            {
                suggested.push(key);
            }
        }
    }
    Outcome {
        auto_fired,
        suggested,
    }
}

#[derive(Default, Clone)]
struct Score {
    cases: usize,
    /// Expected verses that Relay found (auto or suggested).
    found: usize,
    expected: usize,
    /// Verses Relay put on the SCREEN that nobody asked for. The number that matters.
    false_auto_fires: usize,
    /// A paraphrase that auto-fired. Must always be zero.
    paraphrase_auto_fires: usize,
}

/// The whole point: a printable, checkable scorecard.
pub fn scorecard() -> String {
    const RAW: &str = include_str!("../data/eval_corpus.json");
    let corpus: Corpus = serde_json::from_str(RAW).expect("eval_corpus.json is not valid JSON");

    let mut by_lang: BTreeMap<String, Score> = BTreeMap::new();
    let mut total = Score::default();
    let mut failures: Vec<String> = Vec::new();

    for case in &corpus.cases {
        let out = run(case);
        let s = by_lang.entry(case.lang.clone()).or_default();
        s.cases += 1;
        total.cases += 1;

        // Recall: did we find what we were supposed to?
        for want in &case.expect {
            s.expected += 1;
            total.expected += 1;
            if out.auto_fired.contains(want) || out.suggested.contains(want) {
                s.found += 1;
                total.found += 1;
            } else {
                failures.push(format!(
                    "  MISSED  [{}] {:?}\n          expected {want}, got auto={:?} suggested={:?}",
                    case.id, case.text, out.auto_fired, out.suggested
                ));
            }
        }

        // The metric that decides whether Relay is safe to run live: did it put a
        // verse on the screen that nobody asked for?
        for got in &out.auto_fired {
            if !case.expect.contains(got) {
                s.false_auto_fires += 1;
                total.false_auto_fires += 1;
                if case.must_not_fire {
                    s.paraphrase_auto_fires += 1;
                    total.paraphrase_auto_fires += 1;
                }
                failures.push(format!(
                    "  WRONG   [{}] {:?}\n          AUTO-FIRED {got} — nobody asked for it",
                    case.id, case.text
                ));
            }
        }
    }

    let pct = |n: usize, d: usize| {
        if d == 0 {
            100.0
        } else {
            n as f64 / d as f64 * 100.0
        }
    };

    let mut out = String::new();
    out.push_str("\n  Relay — detection benchmark\n");
    out.push_str("  ─────────────────────────────────────────────────────────────\n");
    out.push_str("  lang   cases   recall    verses found   wrong verses on screen\n");
    for (lang, s) in &by_lang {
        out.push_str(&format!(
            "  {:<6} {:>5}   {:>5.0}%    {:>3}/{:<3}        {}\n",
            lang,
            s.cases,
            pct(s.found, s.expected),
            s.found,
            s.expected,
            s.false_auto_fires,
        ));
    }
    out.push_str("  ─────────────────────────────────────────────────────────────\n");
    out.push_str(&format!(
        "  TOTAL  {:>5}   {:>5.0}%    {:>3}/{:<3}        {}\n",
        total.cases,
        pct(total.found, total.expected),
        total.found,
        total.expected,
        total.false_auto_fires,
    ));
    out.push_str(&format!(
        "\n  wrong-verse rate: {:.1}%  (SPEC target: <5%)\n",
        pct(total.false_auto_fires, total.cases)
    ));
    out.push_str(&format!(
        "  paraphrases auto-fired: {}  (must be 0)\n",
        total.paraphrase_auto_fires
    ));

    if !failures.is_empty() {
        out.push_str("\n  Failures:\n");
        for f in &failures {
            out.push_str(f);
            out.push('\n');
        }
    }
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Print the scorecard. `cargo test eval -- --nocapture`
    #[test]
    fn print_scorecard() {
        println!("{}", scorecard());
    }

    /// THE gate. A paraphrase reaching the congregation without a human agreeing
    /// is the failure this whole product is built to avoid — a TF-IDF cosine is
    /// not a probability, and no threshold on it is meaningful.
    #[test]
    fn no_paraphrase_ever_auto_fires() {
        const RAW: &str = include_str!("../data/eval_corpus.json");
        let corpus: Corpus = serde_json::from_str(RAW).unwrap();
        for case in corpus.cases.iter().filter(|c| c.must_not_fire) {
            let out = run(case);
            assert!(
                out.auto_fired.is_empty(),
                "[{}] auto-fired {:?} — a paraphrase reached the screen",
                case.id,
                out.auto_fired
            );
        }
    }

    /// A negative case must put NOTHING on the screen. Most of these are real
    /// garbled transcripts from a live rehearsal that used to fire wrong verses.
    #[test]
    fn negative_cases_put_nothing_on_the_screen() {
        const RAW: &str = include_str!("../data/eval_corpus.json");
        let corpus: Corpus = serde_json::from_str(RAW).unwrap();
        for case in corpus.cases.iter().filter(|c| c.expect.is_empty()) {
            let out = run(case);
            assert!(
                out.auto_fired.is_empty(),
                "[{}] {:?}\n  auto-fired {:?} — a wrong verse would have gone on the wall",
                case.id,
                case.text,
                out.auto_fired
            );
        }
    }

    /// Every verse the corpus says we should find, we must find.
    #[test]
    fn recall_is_total() {
        const RAW: &str = include_str!("../data/eval_corpus.json");
        let corpus: Corpus = serde_json::from_str(RAW).unwrap();
        let mut missed = Vec::new();
        for case in &corpus.cases {
            let out = run(case);
            for want in &case.expect {
                if !out.auto_fired.contains(want) && !out.suggested.contains(want) {
                    missed.push(format!("[{}] {:?} → missed {want}", case.id, case.text));
                }
            }
        }
        assert!(
            missed.is_empty(),
            "missed references:\n{}",
            missed.join("\n")
        );
    }

    /// The SPEC number, enforced. docs/SPEC.md §2: wrong-verse rate <5%.
    #[test]
    fn wrong_verse_rate_beats_the_spec_target() {
        const RAW: &str = include_str!("../data/eval_corpus.json");
        let corpus: Corpus = serde_json::from_str(RAW).unwrap();
        let mut wrong = 0usize;
        for case in &corpus.cases {
            let out = run(case);
            wrong += out
                .auto_fired
                .iter()
                .filter(|g| !case.expect.contains(g))
                .count();
        }
        let rate = wrong as f64 / corpus.cases.len() as f64 * 100.0;
        assert!(
            rate < 5.0,
            "wrong-verse rate {rate:.1}% exceeds the SPEC target of 5%"
        );
    }

    /// The tier-1 languages must actually be covered by the benchmark, or the
    /// moat goes back to being unmeasured the moment someone deletes a case.
    #[test]
    fn every_tier1_language_is_benchmarked() {
        const RAW: &str = include_str!("../data/eval_corpus.json");
        let corpus: Corpus = serde_json::from_str(RAW).unwrap();
        for lang in ["yo", "sw", "ha"] {
            let n = corpus.cases.iter().filter(|c| c.lang == lang).count();
            assert!(
                n >= 3,
                "only {n} benchmark cases for {lang:?} — the differentiator needs coverage"
            );
        }
    }
}

/// The PARAPHRASE benchmark — a separate question from the one above.
///
/// ## Why this had to exist
///
/// The corpus in `eval_corpus.json` is almost entirely DIRECT references: a
/// preacher naming book, chapter and verse. It scores the parser. It says
/// nothing whatsoever about the other half of the product — recognising a verse
/// the preacher never named, from the meaning of what they said.
///
/// That half was completely unmeasured, and it showed. The operator's complaint,
/// in their words: *"I don't want the AI to just detect a word and match the word
/// to the whole bible."* They were right, and nothing in the build could confirm
/// or deny it, let alone tell whether a change made it better.
///
/// First measurement, over the 16 cases below: **recall@1 = 69%**, and the wrong
/// answers were all one shape — a whole verse justified by one or two words:
///
/// | said                                        | offered        | on |
/// |---------------------------------------------|----------------|----|
/// | "the word became flesh and dwelt among us"   | Proverbs 23:20 | `flesh`, `among` |
/// | "the promise of God … mixing with faith"     | Galatians 3:18 | `promise`, `god` |
///
/// Raising the evidence floor to 3 (see `MIN_EVIDENCE_TERMS`) took it to **75%**
/// and removed both.
///
/// ## What it does NOT claim
///
/// This is TF-IDF: lexical overlap, not meaning. The ceiling is visible right
/// here in the corpus — "do not be anxious about anything" cannot reach
/// Philippians 4:6, because the KJV says *"be careful for nothing"* and the two
/// share no content word at all. **No amount of tuning fixes a vocabulary
/// mismatch; only a semantic embedder does** (CLAUDE.md lists it as parked, the
/// seam is `SemanticIndex::top_k`, and `verses.embedding` is still empty).
///
/// So this is deliberately a SCOREBOARD and not a build gate. Its job is that
/// the embedder, when it lands, has a number to beat and a way to prove it —
/// and that nobody can quietly make paraphrase worse in the meantime.
#[cfg(test)]
mod paraphrase {
    use crate::detection::{SemanticIndex, VerseRef, CANONICAL_BOOKS};

    /// How a preacher actually says it, and the verse they mean. Written from
    /// real preaching, not from the KJV text — the whole point is the gap between
    /// the two.
    const CASES: &[(&str, &str)] = &[
        (
            "god loved the world so much that he gave his only son",
            "John 3:16",
        ),
        (
            "everything works together for good for those who love god",
            "Romans 8:28",
        ),
        (
            "trust god completely and do not rely on your own understanding",
            "Proverbs 3:5",
        ),
        ("god will supply everything you need", "Philippians 4:19"),
        (
            "come to me if you are tired and heavy laden and i will give you rest",
            "Matthew 11:28",
        ),
        (
            "the name of the lord is a strong tower the righteous run into it and are safe",
            "Proverbs 18:10",
        ),
        (
            "ask and you will receive seek and you will find knock and it opens",
            "Matthew 7:7",
        ),
        ("we walk by faith and not by sight", "2 Corinthians 5:7"),
        (
            "god has not given us a spirit of fear but of power and love",
            "2 Timothy 1:7",
        ),
        (
            "do not be anxious about anything but pray about everything",
            "Philippians 4:6",
        ),
        (
            "whatever you ask in prayer believe that you receive it and you shall have it",
            "Mark 11:24",
        ),
        ("the word became flesh and dwelt among us", "John 1:14"),
        (
            "there is no condemnation for those who are in christ jesus",
            "Romans 8:1",
        ),
        (
            "faith comes by hearing and hearing by the word of god",
            "Romans 10:17",
        ),
        (
            "i can do all things through christ who strengthens me",
            "Philippians 4:13",
        ),
        // The real paraphrase a preacher used live on 2026-07-26. Relay got this
        // one right in the service and the operator kept it.
        (
            "the promise of god in the bible is mixing with faith in your heart",
            "Hebrews 4:2",
        ),
    ];

    fn full_bible_index() -> SemanticIndex {
        #[derive(serde::Deserialize)]
        struct KjvBook {
            chapters: Vec<Vec<String>>,
        }
        const RAW: &str = include_str!("../data/kjv.json");
        let books: Vec<KjvBook> =
            serde_json::from_str(RAW.trim_start_matches('\u{feff}')).expect("kjv.json parses");
        let mut corpus: Vec<(VerseRef, String)> = Vec::new();
        for (bi, b) in books.iter().enumerate() {
            let name = CANONICAL_BOOKS[bi];
            for (ci, ch) in b.chapters.iter().enumerate() {
                for (vi, t) in ch.iter().enumerate() {
                    corpus.push((
                        VerseRef {
                            book: name.to_string(),
                            chapter: ci as i64 + 1,
                            verse: vi as i64 + 1,
                        },
                        t.clone(),
                    ));
                }
            }
        }
        SemanticIndex::build(&corpus)
    }

    fn key(r: &VerseRef) -> String {
        format!("{} {}:{}", r.book, r.chapter, r.verse)
    }

    /// Rank of the true verse for each case, `None` if outside the top 20.
    fn ranks(idx: &SemanticIndex) -> Vec<(&'static str, &'static str, Option<usize>)> {
        CASES
            .iter()
            .map(|(q, want)| {
                let rank = idx.top_k(q, 20).iter().position(|(r, _)| key(r) == *want);
                (*q, *want, rank)
            })
            .collect()
    }

    /// The scoreboard. `cargo test paraphrase::print_scorecard -- --nocapture`.
    #[test]
    fn print_scorecard() {
        let idx = full_bible_index();
        let rows = ranks(&idx);
        println!("\n  Relay — paraphrase benchmark (TF-IDF, no embedder)");
        println!("  ─────────────────────────────────────────────────────────────");
        for (q, want, rank) in &rows {
            let r = rank
                .map(|r| format!("rank {}", r + 1))
                .unwrap_or_else(|| "MISS (>20)".into());
            println!("  {:<12} {:<11}  {}", want, r, &q[..q.len().min(46)]);
        }
        let n = rows.len();
        let at1 = rows.iter().filter(|(_, _, r)| *r == Some(0)).count();
        let at5 = rows
            .iter()
            .filter(|(_, _, r)| r.map(|x| x < 5).unwrap_or(false))
            .count();
        println!("  ─────────────────────────────────────────────────────────────");
        println!(
            "  recall@1 {}/{} ({:.0}%)   recall@5 {}/{} ({:.0}%)",
            at1,
            n,
            100.0 * at1 as f32 / n as f32,
            at5,
            n,
            100.0 * at5 as f32 / n as f32
        );
        println!("  TF-IDF is lexical. The ceiling is vocabulary, not tuning.\n");
    }

    /// A RATCHET, not a target. Paraphrase quality may not silently regress
    /// below what was measured when the evidence floor was raised to 3.
    ///
    /// Deliberately a floor and not an equality: an embedder landing behind this
    /// seam should make the test pass by a mile, not have to be edited.
    #[test]
    fn paraphrase_recall_does_not_regress() {
        let idx = full_bible_index();
        let rows = ranks(&idx);
        let n = rows.len();
        let at1 = rows.iter().filter(|(_, _, r)| *r == Some(0)).count();
        let at5 = rows
            .iter()
            .filter(|(_, _, r)| r.map(|x| x < 5).unwrap_or(false))
            .count();
        assert!(
            at1 * 100 / n >= 75,
            "paraphrase recall@1 fell to {at1}/{n} — was 12/16 (75%)"
        );
        assert!(
            at5 * 100 / n >= 81,
            "paraphrase recall@5 fell to {at5}/{n} — was 13/16 (81%)"
        );
    }

    /// THE OPERATOR'S COMPLAINT, as a test: *"I don't want the AI to just detect
    /// a word and match the word to the whole bible."*
    ///
    /// Both of these had a wrong verse at rank 1, carried entirely by two words.
    #[test]
    fn a_verse_justified_by_two_words_is_not_offered() {
        let idx = full_bible_index();
        for (q, forbidden) in [
            ("the word became flesh and dwelt among us", "Proverbs 23:20"),
            (
                "the promise of god in the bible is mixing with faith in your heart",
                "Galatians 3:18",
            ),
        ] {
            let top = idx.top_k(q, 1);
            if let Some((r, _)) = top.first() {
                assert_ne!(
                    key(r),
                    forbidden,
                    "{q:?} is once again answered by a verse sharing two words with it"
                );
            }
        }
    }
}
