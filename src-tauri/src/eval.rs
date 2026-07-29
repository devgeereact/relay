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

// ── Paraphrase RETRIEVAL benchmark ──────────────────────────────────────────
//
// The corpus above answers "does the right verse reach the screen". It cannot
// answer "when a preacher TELLS a story instead of quoting it, do we find the
// story", because it holds exactly two paraphrase cases and both are near-
// verbatim famous verses. A 100% scorecard therefore says nothing at all about
// paraphrase quality — which is precisely the claim the product makes.
//
// This measures retrieval directly against the real index: given a retelling,
// does a verse FROM THE RIGHT PASSAGE come back in the top-k? Nothing here is a
// build gate yet — the baseline has to exist before a floor can be argued for.

/// One narrative retelling and the passage it belongs to.
#[derive(Debug, Deserialize)]
pub struct ParaCase {
    pub id: String,
    pub text: String,
    pub book: String,
    pub chapter: i64,
    pub verse_start: i64,
    pub verse_end: i64,
    /// "kjv" (words present but spread across verses) or "modern" (synonymy —
    /// unfixable by any lexical method, tracked so the limit stays honest).
    pub vocab: String,
}

#[derive(Debug, Deserialize)]
struct ParaCorpus {
    cases: Vec<ParaCase>,
}

pub fn para_cases() -> Vec<ParaCase> {
    const RAW: &str = include_str!("../data/paraphrase_corpus.json");
    let c: ParaCorpus = serde_json::from_str(RAW).expect("paraphrase_corpus.json");
    c.cases
}

/// The bundled KJV as (reference, text) — the same corpus `main` indexes at
/// startup, but read straight from the JSON so this needs no database.
pub fn kjv_corpus() -> Vec<(detection::VerseRef, String)> {
    #[derive(Deserialize)]
    struct KjvBook {
        chapters: Vec<Vec<String>>,
    }
    const RAW: &str = include_str!("../data/kjv.json");
    let books: Vec<KjvBook> =
        serde_json::from_str(RAW.trim_start_matches('\u{feff}')).expect("kjv.json");
    let mut out = Vec::with_capacity(31_200);
    for (bi, book) in books.iter().enumerate() {
        let name = detection::CANONICAL_BOOKS.get(bi).copied().unwrap_or("?");
        for (ci, chapter) in book.chapters.iter().enumerate() {
            for (vi, text) in chapter.iter().enumerate() {
                out.push((
                    detection::VerseRef {
                        book: name.to_string(),
                        chapter: ci as i64 + 1,
                        verse: vi as i64 + 1,
                    },
                    text.replace(['{', '}'], ""),
                ));
            }
        }
    }
    out
}

impl ParaCase {
    /// Is `r` inside the passage this retelling came from?
    pub fn contains(&self, r: &detection::VerseRef) -> bool {
        r.book == self.book
            && r.chapter == self.chapter
            && r.verse >= self.verse_start
            && r.verse <= self.verse_end
    }
}

/// Scorecard: top-1 and top-5 in-passage rate, split by vocabulary type.
pub fn paraphrase_scorecard() -> String {
    use detection::SemanticIndex;
    let index = SemanticIndex::build(&kjv_corpus());
    let cases = para_cases();

    // hits@[1,2,3,5] and total, per vocab bucket, plus the overall row. The
    // cutoffs exist to CHOOSE how many suggestions production should surface:
    // every extra one costs the operator attention, so the gain has to be shown.
    const KS: [usize; 4] = [1, 2, 3, 5];
    let mut buckets: BTreeMap<String, ([usize; 4], usize)> = BTreeMap::new();
    let mut misses: Vec<String> = Vec::new();

    for case in &cases {
        let hits = index.top_k(&case.text, 5);
        let at: [usize; 4] =
            KS.map(|k| hits.iter().take(k).any(|(r, _)| case.contains(r)) as usize);
        for key in [case.vocab.clone(), "TOTAL".to_string()] {
            let e = buckets.entry(key).or_insert(([0; 4], 0));
            for (slot, hit) in e.0.iter_mut().zip(at.iter()) {
                *slot += hit;
            }
            e.1 += 1;
        }
        let at5 = at[3] == 1;
        if !at5 {
            let got = hits
                .first()
                .map(|(r, s)| format!("{} {}:{} @{:.2}", r.book, r.chapter, r.verse, s))
                .unwrap_or_else(|| "nothing".into());
            misses.push(format!(
                "    {:<24} want {} {}:{}-{}  got {}",
                case.id, case.book, case.chapter, case.verse_start, case.verse_end, got
            ));
        }
    }

    let pct = |a: usize, b: usize| {
        if b == 0 {
            0.0
        } else {
            a as f32 * 100.0 / b as f32
        }
    };
    let row = |label: &str, hits: &[usize; 4], n: usize| {
        format!(
            "  {label:<10} {n:>5}   {:>5.0}%  {:>5.0}%  {:>5.0}%  {:>5.0}%\n",
            pct(hits[0], n),
            pct(hits[1], n),
            pct(hits[2], n),
            pct(hits[3], n)
        )
    };
    let mut out = String::from("\n  Relay — paraphrase RETRIEVAL benchmark\n");
    out.push_str("  ─────────────────────────────────────────────────────────────\n");
    out.push_str("  vocab      cases      @1     @2     @3     @5\n");
    for (k, (hits, n)) in &buckets {
        if k != "TOTAL" {
            out.push_str(&row(k, hits, *n));
        }
    }
    if let Some((hits, n)) = buckets.get("TOTAL") {
        out.push_str("  ─────────────────────────────────────────────────────────────\n");
        out.push_str(&row("TOTAL", hits, *n));
    }
    if !misses.is_empty() {
        out.push_str("\n  missed entirely (not in top 5):\n");
        for m in &misses {
            out.push_str(m);
            out.push('\n');
        }
    }
    out
}

/// How many suggestions to surface, and what it costs the operator.
///
/// A fixed top-N is the wrong shape. When one verse clearly wins, showing five
/// is noise; when four score alike, showing one is a coin toss presented as an
/// answer. So production keeps every hit within `ratio` of the best score — the
/// list widens exactly when Relay is genuinely unsure.
///
/// This sweeps that ratio so the choice is made on evidence: recall is what the
/// operator can reach, "avg shown" is what it costs them to read.
pub fn suggestion_policy_scorecard() -> String {
    use detection::SemanticIndex;
    let index = SemanticIndex::build(&kjv_corpus());
    let cases = para_cases();
    const FLOOR: f32 = 0.30; // SEMANTIC_FLOOR in main.rs
    const CAP: usize = 5; // never more than this, however flat the scores

    let mut out = String::from("\n  Suggestion policy — how many paraphrase hits to show\n");
    out.push_str("  ─────────────────────────────────────────────────────────────\n");
    out.push_str("  keep within   ALL recall  avg shown | MODERN recall  avg shown\n");
    for ratio in [1.00_f32, 0.90, 0.80, 0.70, 0.60, 0.50] {
        let (mut reached, mut shown) = (0usize, 0usize);
        let (mut m_reached, mut m_shown, mut m_n) = (0usize, 0usize, 0usize);
        for case in &cases {
            let hits = index.top_k(&case.text, CAP);
            let best = hits.first().map(|(_, s)| *s).unwrap_or(0.0);
            let kept: Vec<_> = hits
                .iter()
                .filter(|(_, s)| *s >= FLOOR && *s >= best * ratio)
                .collect();
            let hit = kept.iter().any(|(r, _)| case.contains(r)) as usize;
            shown += kept.len();
            reached += hit;
            if case.vocab == "modern" {
                m_n += 1;
                m_shown += kept.len();
                m_reached += hit;
            }
        }
        let n = cases.len();
        out.push_str(&format!(
            "  {:>6.0}%      {:>7.0}%      {:>5.2} | {:>10.0}%      {:>5.2}\n",
            ratio * 100.0,
            reached as f32 * 100.0 / n as f32,
            shown as f32 / n as f32,
            m_reached as f32 * 100.0 / m_n as f32,
            m_shown as f32 / m_n as f32,
        ));
    }
    out.push_str("\n  recall = right passage was reachable · avg shown = rows the\n");
    out.push_str("  operator reads per detection · silent = nothing offered at all\n");
    out
}

#[cfg(test)]
mod para_tests {
    use super::*;

    /// `cargo test eval::para_tests::print_suggestion_policy -- --nocapture`
    #[test]
    fn print_suggestion_policy() {
        println!("{}", suggestion_policy_scorecard());
    }

    /// `cargo test eval::para_tests::print_paraphrase_scorecard -- --nocapture`
    #[test]
    fn print_paraphrase_scorecard() {
        println!("{}", paraphrase_scorecard());
    }

    /// Every labelled passage must actually exist in the bundled KJV — a typo in
    /// a reference would silently score as a miss forever.
    #[test]
    fn every_labelled_passage_exists() {
        let corpus = kjv_corpus();
        for case in para_cases() {
            let found = corpus.iter().any(|(r, _)| case.contains(r));
            assert!(
                found,
                "[{}] {} {}:{}-{} is not in the bundled KJV",
                case.id, case.book, case.chapter, case.verse_start, case.verse_end
            );
        }
    }
}
