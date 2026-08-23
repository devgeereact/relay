//! The fire pipeline: turning a resolved verse into what the screens show.
//!
//! Single responsibility: given a verse that something has already decided to put
//! on screen, build the two payloads that go out — the `OutputContent` broadcast
//! to every output channel, and the `detection://match` event the console shows.
//!
//! ## Why this module exists
//!
//! Five call sites in `main.rs` each hand-rolled the same sequence — parse a
//! reference, look up the verse, pick the scripture template, broadcast, persist,
//! emit — and they had drifted apart. `handle_nav` and `handle_passage_nav` were
//! near-identical twins, and *both of them forgot the scripture template*, so a
//! verse reached by saying "next" rendered with the channel's default template
//! while the exact same verse reached by saying its reference rendered with the
//! scripture one. Same verse, same screen, different look, depending on how the
//! preacher happened to phrase it.
//!
//! A bug like that is not a coding mistake so much as a structural one: when the
//! payload is built in five places, the five will diverge. So it is built here,
//! once. `Fire::output()` and `Fire::event()` are the ONLY way a verse becomes
//! screen content — add a sixth caller and it is correct by construction.
//!
//! Deliberately DB-free and Tauri-free, so it is directly unit-testable. The
//! callers keep their own lock choreography (the lock-ordering and
//! never-hold-a-lock-across-emit rules in CLAUDE.md are load-bearing and stay
//! where they are).

use crate::channels::OutputContent;
use crate::detection::{DetectionMethod, VerseRef};
use serde::Serialize;

/// Who decided to put this verse on screen.
///
/// This is not cosmetic: it is written to `detections.status`, and the
/// self-calibrating router learns from that column. Recording a human's decision
/// as the machine's would train the gate on a falsified record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireStatus {
    /// The AI, unprompted, above the auto-fire threshold.
    Auto,
    /// The AI offered it; the operator has NOT accepted it yet. Not on screen.
    Suggested,
    /// A human: an override, a confirmed suggestion, or a next/back nav.
    Manual,
}

impl FireStatus {
    /// The value written to `detections.status` (see docs/data/schema.sql).
    pub fn as_str(&self) -> &'static str {
        match self {
            FireStatus::Auto => "auto",
            FireStatus::Suggested => "suggested",
            FireStatus::Manual => "manual",
        }
    }

    /// Does this actually go to the screens? A suggestion does not — it waits for
    /// a human.
    pub fn goes_to_screen(&self) -> bool {
        !matches!(self, FireStatus::Suggested)
    }
}

/// A verse that is about to be shown, with everything already resolved.
///
/// The caller has done the DB work (looked the verse up, picked the template);
/// this is the pure description of what to show. `text`/`translation` are
/// `Option` because a reference can parse cleanly and still not be in the corpus
/// — the console still shows the operator that it was heard.
#[derive(Debug, Clone)]
pub struct Fire {
    pub reference: VerseRef,
    /// Canonical "Book C:V" key — the debounce key and the display reference.
    pub key: String,
    pub verse_id: Option<i64>,
    pub text: Option<String>,
    pub translation: Option<String>,
    pub confidence: f32,
    pub method: DetectionMethod,
    pub status: FireStatus,
    /// Operator's private note — rides to the stage monitor, never to the
    /// congregation (no template region renders it).
    pub stage_note: Option<String>,
    /// The next verse coming up (reference + text), for a stage/confidence
    /// monitor. Rides to output alongside `stage_note`; only a monitor template
    /// with a `next` layer renders it. Filled by `attach_next_verse` from the
    /// staged passage, so it is BOUNDED by the read range. `None` at a range end
    /// or when the following verse is not in the corpus.
    pub next_reference: Option<String>,
    pub next_text: Option<String>,
    /// The per-content-type scripture template. EVERY fire path must carry this;
    /// forgetting it is the bug this module exists to make impossible.
    pub template_id: Option<i64>,
    pub template_json: Option<String>,
    /// True when the template is a cue's DELIBERATE choice (it overrides the
    /// screen), false for a content-type default (defers to the screen). See
    /// `OutputContent::template_pinned`.
    pub template_pinned: bool,
    /// WHY the machine thinks this verse. The transcript span a direct reference was
    /// parsed from ("john three sixteen"), or the overlapping words that produced a
    /// paraphrase's cosine ("grace · saved · faith"). `None` for a human's own fire —
    /// the operator does not need to be told why they did something.
    ///
    /// This rides to the console and is shown. It was captured for months and thrown
    /// away at the IPC boundary, which meant the operator was asked to accept or
    /// reject the AI's judgement while being shown nothing but a percentage.
    pub matched_text: Option<String>,
    /// The decode pass this verse came out of, when speech put it here. Set by
    /// `emit_detections`; `None` on every human-driven path, which is exactly the
    /// distinction the latency report needs (see `OutputContent::trace_id`).
    pub trace_id: Option<u64>,
}

impl Fire {
    /// Canonical key for a verse. One definition, so the debounce key the router
    /// sees and the reference the operator reads can never disagree.
    pub fn key_for(r: &VerseRef) -> String {
        format!("{} {}:{}", r.book, r.chapter, r.verse)
    }

    /// May this actually go to the congregation's screens?
    ///
    /// Two conditions, and the second one is the one that bites: the verse must
    /// EXIST. A reference can parse perfectly and still not resolve — garbled
    /// speech readily yields "Psalms 23:99" — and broadcasting that renders a
    /// verse with no text, which blanks the projector mid-service and tells the
    /// operator nothing about why.
    ///
    /// Such a detection is still *surfaced* (with `in_library: false`), so the
    /// operator can see Relay heard something it couldn't resolve. Heard-but-
    /// unresolvable must degrade to a suggestion, never to silence and never to a
    /// blank wall.
    pub fn may_broadcast(&self) -> bool {
        self.status.goes_to_screen() && self.verse_id.is_some()
    }

    /// What every output channel renders.
    pub fn output(&self) -> OutputContent {
        OutputContent {
            kind: Some("scripture".into()),
            trace_id: self.trace_id,
            reference: self.key.clone(),
            text: self.text.clone(),
            translation: self.translation.clone(),
            template_id: self.template_id,
            template_json: self.template_json.clone(),
            template_pinned: self.template_pinned,
            stage_note: self.stage_note.clone(),
            next_reference: self.next_reference.clone(),
            next_text: self.next_text.clone(),
            ..Default::default()
        }
    }

    /// What the operator console shows.
    pub fn event(&self) -> DetectionEvent {
        DetectionEvent {
            reference: self.key.clone(),
            book: self.reference.book.clone(),
            chapter: self.reference.chapter,
            verse: self.reference.verse,
            confidence: self.confidence,
            method: self.method,
            status: self.status.as_str(),
            in_library: self.verse_id.is_some(),
            text: self.text.clone(),
            translation: self.translation.clone(),
            matched_text: self.matched_text.clone(),
            trace_id: self.trace_id,
        }
    }
}

/// A detection surfaced to the operator console (`detection://match`).
///
/// `in_library` is false when the reference parsed cleanly but isn't in the
/// seeded corpus — the console still shows it, so the operator sees that Relay
/// *heard* it and can act, rather than silently swallowing a real reference.
///
/// `method` and `matched_text` are the two fields the operator is actually being
/// asked to judge, and the console used to render neither. Relay's entire safety
/// story is that a *heard reference* and a *paraphrase guess* are different kinds of
/// claim on incomparable scales — and both arrived on screen as an identical
/// "AI suggestion — 92% match". The distinction the whole gate is built on was
/// invisible to the one person who can overrule it.
#[derive(Clone, Serialize)]
pub struct DetectionEvent {
    pub reference: String,
    pub book: String,
    pub chapter: i64,
    pub verse: i64,
    pub confidence: f32,
    pub method: DetectionMethod,
    pub status: &'static str,
    pub in_library: bool,
    pub text: Option<String>,
    pub translation: Option<String>,
    /// The evidence. See `Fire::matched_text`.
    pub matched_text: Option<String>,
    /// The decode pass behind this detection. See `OutputContent::trace_id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<u64>,
}

/// A gate candidate: the anchor verse plus how it should route, and whether it is
/// part of a multi-verse passage (range / whole chapter) to stage for "next".
pub struct Cand {
    pub r: VerseRef,
    pub conf: f32,
    pub method: DetectionMethod,
    pub verse_end: Option<i64>,
    pub whole_chapter: bool,
    /// Why this candidate exists — carried through the gate to the console. See
    /// `Fire::matched_text`.
    pub matched: Option<String>,
}

impl Cand {
    /// A plain single-verse candidate (no passage span).
    pub fn single(
        r: VerseRef,
        conf: f32,
        method: DetectionMethod,
        matched: Option<String>,
    ) -> Self {
        Cand {
            r,
            conf,
            method,
            verse_end: None,
            whole_chapter: false,
            matched,
        }
    }
}

/// Pick the winner when the same verse is found more than one way.
///
/// Rank by `(may_auto_fire, confidence)` — NOT by confidence alone. The same
/// verse can be found both by a spoken reference and by a paraphrase match, and
/// the paraphrase's raw TF-IDF cosine is often the LARGER number despite being
/// the weaker evidence. Ranking on the number alone would let the paraphrase win
/// and, because paraphrases may never auto-fire, silently demote a real spoken
/// reference into a mere suggestion.
pub fn better(a: &Cand, b: &Cand) -> bool {
    (a.method.may_auto_fire(), a.conf) >= (b.method.may_auto_fire(), b.conf)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vref(book: &str, chapter: i64, verse: i64) -> VerseRef {
        VerseRef {
            book: book.into(),
            chapter,
            verse,
        }
    }

    fn fire(status: FireStatus) -> Fire {
        let r = vref("John", 3, 16);
        Fire {
            key: Fire::key_for(&r),
            reference: r,
            verse_id: Some(42),
            text: Some("For God so loved the world...".into()),
            translation: Some("KJV".into()),
            confidence: 0.93,
            method: DetectionMethod::Direct,
            status,
            stage_note: None,
            next_reference: None,
            next_text: None,
            template_id: Some(7),
            template_json: Some(r#"{"style":{}}"#.into()),
            template_pinned: false,
            matched_text: Some("john three sixteen".into()),
            trace_id: None,
        }
    }

    #[test]
    fn key_is_the_canonical_reference() {
        assert_eq!(Fire::key_for(&vref("1 John", 4, 8)), "1 John 4:8");
    }

    /// The regression test for the bug that motivated this module. A verse
    /// reached by saying "next" and the same verse reached by saying its
    /// reference must render IDENTICALLY — the nav paths used to drop the
    /// scripture template and render with the channel default instead.
    #[test]
    fn every_fire_carries_its_template() {
        let out = fire(FireStatus::Manual).output();
        assert_eq!(out.template_id, Some(7));
        assert!(out.template_json.is_some());
    }

    #[test]
    fn output_and_event_agree_on_the_reference() {
        let f = fire(FireStatus::Auto);
        assert_eq!(f.output().reference, f.event().reference);
        assert_eq!(f.output().reference, "John 3:16");
    }

    /// A human decision must never be recorded as the machine's — the
    /// self-calibrating router learns from this exact column.
    #[test]
    fn status_distinguishes_the_human_from_the_ai() {
        assert_eq!(fire(FireStatus::Auto).event().status, "auto");
        assert_eq!(fire(FireStatus::Manual).event().status, "manual");
        assert_eq!(fire(FireStatus::Suggested).event().status, "suggested");
    }

    #[test]
    fn a_suggestion_does_not_reach_the_screens() {
        assert!(!FireStatus::Suggested.goes_to_screen());
        assert!(FireStatus::Auto.goes_to_screen());
        assert!(FireStatus::Manual.goes_to_screen());
    }

    /// THE regression test from the live rehearsal. A garbled "Psalms 23:99"
    /// auto-fired and broadcast a verse with no text — blanking the projector,
    /// mid-service, with nothing to tell the operator why.
    #[test]
    fn a_verse_that_does_not_exist_is_never_broadcast() {
        for status in [FireStatus::Auto, FireStatus::Manual] {
            let mut f = fire(status);
            f.verse_id = None; // parsed cleanly, but no such verse
            f.text = None;
            assert!(
                !f.may_broadcast(),
                "{status:?} would have blanked the screen"
            );
        }
    }

    #[test]
    fn a_real_verse_is_broadcast() {
        assert!(fire(FireStatus::Auto).may_broadcast());
        assert!(fire(FireStatus::Manual).may_broadcast());
        // ...but a suggestion still waits for a human, however real the verse is.
        assert!(!fire(FireStatus::Suggested).may_broadcast());
    }

    #[test]
    fn a_verse_outside_the_corpus_is_still_shown_to_the_operator() {
        let mut f = fire(FireStatus::Auto);
        f.verse_id = None;
        f.text = None;
        let e = f.event();
        assert!(!e.in_library);
        // ...but the reference still reaches the console, so nothing is swallowed.
        assert_eq!(e.reference, "John 3:16");
    }

    #[test]
    fn the_stage_note_rides_to_the_output_but_is_not_a_detection_field() {
        let mut f = fire(FireStatus::Manual);
        f.stage_note = Some("hold for prayer".into());
        assert_eq!(f.output().stage_note.as_deref(), Some("hold for prayer"));
    }

    /// The next verse rides to output for a monitor's "up next" line — but never
    /// leaks onto the detection event (the console shows what IS on screen, not
    /// what is coming). Only a monitor template with a `next` layer renders it.
    #[test]
    fn the_next_verse_rides_to_the_output_only() {
        let mut f = fire(FireStatus::Manual);
        f.next_reference = Some("John 3:17".into());
        f.next_text = Some("For God sent not his Son...".into());
        let out = f.output();
        assert_eq!(out.next_reference.as_deref(), Some("John 3:17"));
        assert_eq!(
            out.next_text.as_deref(),
            Some("For God sent not his Son...")
        );
        // It is not a field on the detection event at all.
        let _ = f.event(); // compiles = DetectionEvent has no next_* to set
    }

    /// The pin flag rides to output: a cue's deliberate template choice (pinned)
    /// overrides the screen; a content-type default (not pinned) defers to it.
    #[test]
    fn the_template_pin_flag_rides_to_output() {
        let mut f = fire(FireStatus::Manual);
        f.template_pinned = true;
        assert!(
            f.output().template_pinned,
            "a pinned cue choice must ride out"
        );
        f.template_pinned = false;
        assert!(
            !f.output().template_pinned,
            "a content default is not pinned"
        );
    }

    /// A direct hit must beat a paraphrase for the same verse even when the
    /// paraphrase's raw cosine is the bigger number.
    #[test]
    fn a_direct_hit_outranks_a_higher_scoring_paraphrase() {
        let direct = Cand::single(vref("John", 3, 16), 0.70, DetectionMethod::Direct, None);
        let semantic = Cand::single(vref("John", 3, 16), 0.95, DetectionMethod::Semantic, None);
        assert!(better(&direct, &semantic));
        assert!(!better(&semantic, &direct));
    }

    #[test]
    fn between_two_direct_hits_the_more_confident_one_wins() {
        let lo = Cand::single(vref("John", 3, 16), 0.70, DetectionMethod::Direct, None);
        let hi = Cand::single(vref("John", 3, 16), 0.90, DetectionMethod::Direct, None);
        assert!(better(&hi, &lo));
        assert!(!better(&lo, &hi));
    }

    /// The operator is asked to accept or reject the machine's judgement. Both of
    /// the things they need in order to do that — WHAT KIND of claim this is, and
    /// WHAT WORDS produced it — must survive the trip to the console.
    ///
    /// `matched_text` was captured in the detector for months and dropped at this
    /// exact boundary: it was not a field on `DetectionEvent`, so it never crossed
    /// the IPC bridge. A paraphrase guess and a heard reference reached the console
    /// as the same sentence.
    #[test]
    fn the_event_carries_the_evidence_the_operator_must_judge() {
        let e = fire(FireStatus::Suggested).event();
        assert_eq!(e.matched_text.as_deref(), Some("john three sixteen"));
        assert_eq!(e.method, DetectionMethod::Direct);
    }

    /// A human's own fire needs no explanation — they are the reason it is on screen.
    #[test]
    fn a_manual_fire_carries_no_evidence_line() {
        let mut f = fire(FireStatus::Manual);
        f.matched_text = None;
        assert_eq!(f.event().matched_text, None);
    }
}
