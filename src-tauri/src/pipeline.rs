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
    /// RG-135 — see `DetectionEvent::named_translation_missing`. On `Fire` because
    /// `Fire` is the one place an event may be built (CLAUDE.md), so a path that
    /// forgets it is a path that does not compile.
    pub named_translation_missing: Option<String>,
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
            named_translation_missing: self.named_translation_missing.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SAFE SCREEN — the last check before a congregation sees anything
// ─────────────────────────────────────────────────────────────────────────────

/// Why a payload was refused.
///
/// A closed set, because each one is a different sentence to an operator and
/// "something went wrong" is not a sentence an operator can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsafe {
    /// The payload would render an empty screen while the operator believes
    /// something is up there.
    Nothing,
    /// A template was chosen for this cue and it is not valid JSON, so the output
    /// page would silently fall back to a different look.
    BrokenTemplate,
}

impl Unsafe {
    /// What the operator is told. Names the problem and what it means for the
    /// wall — never a Rust error, never "invalid input".
    pub fn message(self) -> &'static str {
        match self {
            Unsafe::Nothing => {
                "Nothing was sent to the screens — that cue has no text, no media and no \
                 reference, so it would have shown an empty screen."
            }
            Unsafe::BrokenTemplate => {
                "That cue's template could not be read, so the screens were left as they \
                 were. Re-pick its template in the Planner, or clear the cue's own template \
                 to use the screen's."
            }
        }
    }
}

/// THE LAST CHECK BEFORE A CONGREGATION SEES ANYTHING.
///
/// ## Why this is worth a gate of its own
///
/// Relay has a gate that decides *whether the AI is allowed to speak*
/// (`router.rs`) and one that decides *whether anything reaches a screen at all*
/// (`channels::broadcast_content`, rehearsal). Neither asks the third question:
/// **is the thing about to go up actually showable?**
///
/// Both failures it catches are silent, and both look identical to the operator:
/// the console says the verse went out, the screen is blank, and nothing anywhere
/// says why. A blank projector mid-service is indistinguishable from a crash from
/// twenty rows back.
///
/// ## What it deliberately does NOT do
///
/// **It does not check that anything is attached to show the content.** A service
/// runs with the console preview alone all the time — during setup, in rehearsal,
/// while a projector is being re-cabled — and refusing to fire because no screen
/// happens to be connected would take the operator's tool away at the exact moment
/// they are fixing the screen. That fact is REPORTED (RG-01, output health), never
/// enforced.
///
/// **It does not check that the text fits.** Fit is a layout question and only the
/// renderer can answer it; `TemplateRender` measures and reports. Guessing here
/// would mean refusing content that would have rendered perfectly well.
///
/// **It never refuses a clear or a blackout.** Those do not come through here at
/// all, and they must not: a panic control that a validator could block would be a
/// panic control that can fail (DECISIONS §20).
pub fn preflight(content: &OutputContent) -> Result<(), Unsafe> {
    // A cue that carries a countdown is showing the clock, and a countdown with no
    // text is the normal case rather than an empty screen.
    // A HELD countdown counts too. `countdown_paused_ms` is checked alongside the
    // instant rather than instead of it: a paused countdown carries both today, and
    // a validator that could refuse a paused timer would blank a wall at the one
    // moment the operator deliberately froze it.
    //
    // ── SITE 2 OF THE CONTENT-KIND SWEEP. NOTHING CHANGED HERE, AND WHY ────────
    //
    // The timer registry adds no wire form. A congregation-scoped (`Both`) timer is
    // still broadcast as `kind: "countdown"` with the same four `countdown_*`
    // fields, projected in exactly one place (`timers::project_both`), so all three
    // arms below match it and this guard needs no new clause. A programme-scoped
    // (`Stage`) timer never becomes an `OutputContent` at all — it has no
    // congregation wire form by construction, `show_timer` refuses to give it one,
    // and nothing it publishes passes through here.
    //
    // **This is the site that would have bitten.** A timer given field names of its
    // own and no words would fall through to `Unsafe::Nothing` below: a screen that
    // stays blank while every log says the fire succeeded, at the top of a service,
    // which is when timers are used. If the `Both` wire form ever moves, this guard
    // moves in the same commit.
    //
    // Two tests hold the coupling to the REAL projector rather than to a copy of it
    // (`a_label_less_timer_from_the_registry_is_not_an_empty_screen` and its held
    // twin), and here is the honest limit of what they catch: the three arms are
    // individually REDUNDANT for a projected timer, so removing any one of them
    // leaves all three tests green. Verified by removing each, and then by disabling
    // the whole guard, which is what turns them red. They catch the wire form moving
    // and the guard disappearing; they do not catch one arm being pruned as dead.
    let is_countdown = content.countdown_to.is_some()
        || content.countdown_paused_ms.is_some()
        || content.kind.as_deref() == Some("countdown");
    let has_text = content
        .text
        .as_deref()
        .is_some_and(|t| !t.trim().is_empty());
    let has_media = content
        .media_url
        .as_deref()
        .is_some_and(|u| !u.trim().is_empty());
    let has_reference = !content.reference.trim().is_empty();

    if !is_countdown && !has_text && !has_media && !has_reference {
        return Err(Unsafe::Nothing);
    }

    // A template the output page cannot parse does not fail loudly there — it
    // falls back, so the wall shows the right words in the wrong look and nobody
    // is told. Checked here, where there is still somebody to tell.
    if let Some(j) = content.template_json.as_deref() {
        if !j.trim().is_empty() && serde_json::from_str::<serde_json::Value>(j).is_err() {
            return Err(Unsafe::BrokenTemplate);
        }
    }

    Ok(())
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
    /// RG-135. A translation the SPEAKER named in this window that Relay does not
    /// have installed, so the words on the wall are not the words being read out.
    ///
    /// `None` is the ordinary case and means two different things that are the same
    /// fact here: nobody named a translation, or the one they named is the one in
    /// use. It is set ONLY when Relay can say something the operator does not
    /// already know, because a caveat on a correct fire is how somebody learns to
    /// stop reading this line.
    ///
    /// The reference is still right. This is not a wrong verse and must not be
    /// rendered as one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub named_translation_missing: Option<String>,
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
            named_translation_missing: None,
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

    // ── SAFE SCREEN ─────────────────────────────────────────────────────────

    fn content() -> OutputContent {
        OutputContent {
            kind: Some("scripture".into()),
            reference: "John 3:16".into(),
            text: Some("For God so loved the world…".into()),
            ..Default::default()
        }
    }

    #[test]
    fn ordinary_content_goes_out() {
        assert!(preflight(&content()).is_ok());
    }

    /// A PAYLOAD WITH NOTHING IN IT IS REFUSED.
    ///
    /// The failure this catches is silent and looks like a crash from twenty rows
    /// back: the console says the cue fired, the projector goes blank, and nothing
    /// anywhere says why.
    #[test]
    fn a_payload_that_would_show_an_empty_screen_is_refused() {
        let mut c = content();
        c.text = None;
        c.reference = String::new();
        assert_eq!(preflight(&c), Err(Unsafe::Nothing));

        // Whitespace is empty. A cue whose text is a stray newline renders exactly
        // as blank as one with no text at all.
        c.text = Some("   \n ".into());
        assert_eq!(preflight(&c), Err(Unsafe::Nothing));
    }

    /// …BUT ANY ONE REAL THING IS ENOUGH.
    #[test]
    fn a_reference_or_media_alone_is_showable() {
        let mut c = content();
        c.text = None;
        assert!(
            preflight(&c).is_ok(),
            "a reference alone still shows something"
        );

        c.reference = String::new();
        c.media_url = Some("http://192.168.1.9:8032/media/12".into());
        c.kind = Some("media".into());
        assert!(
            preflight(&c).is_ok(),
            "a media slide has no text and that is normal"
        );
    }

    /// A COUNTDOWN HAS NO TEXT, AND THAT IS THE NORMAL CASE.
    ///
    /// Refusing it would break the one cue whose whole content is a clock — and it
    /// would do so at the top of a service, which is when countdowns are used.
    #[test]
    fn a_countdown_is_not_an_empty_screen() {
        let mut c = content();
        c.kind = Some("countdown".into());
        c.reference = String::new();
        c.text = None;
        c.countdown_to = Some(1_700_000_000_000);
        assert!(preflight(&c).is_ok());
    }

    /// WHAT THE REGISTRY ACTUALLY PROJECTS SURVIVES THE VALIDATOR — SITE 2 OF THE
    /// CONTENT-KIND SWEEP, HELD TO THE REAL PROJECTION RATHER THAN TO A COPY OF IT.
    ///
    /// The test above states the rule using a hand-built payload, which is the right
    /// shape for the rule and the wrong shape for the hazard. The hazard is that
    /// `timers::project_both` and `is_countdown` are two halves of one agreement —
    /// the registry owns the facts, the four `countdown_*` fields are its
    /// projection, and `is_countdown` is what stops a projection with no words in it
    /// being refused as `Unsafe::Nothing`. A timer given field names of its own
    /// would be refused here: a screen that stays blank while every log says the
    /// fire succeeded, at the top of a service, which is when timers are used.
    ///
    /// So this calls the real projector. Change the wire form and this fails in the
    /// same commit, which is the whole of what the sweep asks for.
    ///
    /// **A timer with NO LABEL is the case that matters**, and it is deliberately
    /// what is tested. A labelled timer projects its label into `reference` and
    /// would pass on `has_reference` alone, with `is_countdown` never consulted — a
    /// test that used one would be green against a broken guard. A label-less timer
    /// has no words anywhere, so `is_countdown` is the only thing standing between
    /// it and a blank wall. It is also the shape a later track makes the default.
    #[test]
    fn a_label_less_timer_from_the_registry_is_not_an_empty_screen() {
        let shown = crate::timers::project_both(&crate::timers::Timer {
            id: 1,
            label: String::new(),
            done_msg: String::new(),
            target_ms: 1_700_000_300_000,
            from_ms: 1_700_000_000_000,
            paused_ms: None,
            warn_ms: Some(120_000),
            scope: crate::timers::Scope::Both,
            plan_item_id: None,
            started_in_rehearsal: false,
        });
        let c = OutputContent {
            kind: Some("countdown".into()),
            reference: shown.reference,
            countdown_to: Some(shown.countdown_to),
            countdown_from: Some(shown.countdown_from),
            countdown_paused_ms: shown.countdown_paused_ms,
            countdown_done: Some(shown.countdown_done).filter(|s| !s.is_empty()),
            // The wire form MOVED for RG-149 and this is the test that is supposed
            // to notice. It carries the whole projection, so a field added to it
            // and not carried here fails in the same commit — which is the whole of
            // what the content-kind sweep asks of this site.
            countdown_warn_ms: shown.countdown_warn_ms,
            ..Default::default()
        };
        assert!(
            c.reference.trim().is_empty() && c.text.is_none(),
            "precondition: this projection carries no words at all, so `is_countdown` \
             is the only thing that can save it"
        );
        assert_eq!(
            preflight(&c),
            Ok(()),
            "the pre-air validator refused what the timer registry actually projects \
             — a blank screen at the top of a service, with every log saying the fire \
             succeeded"
        );
    }

    /// A HELD TIMER IS NOT AN EMPTY SCREEN EITHER.
    ///
    /// The one an operator deliberately froze is the one a validator must never
    /// blank. `countdown_paused_ms` is checked alongside the instant rather than
    /// instead of it, and a held projection still carries both.
    #[test]
    fn a_held_label_less_timer_is_not_an_empty_screen() {
        let shown = crate::timers::project_both(&crate::timers::Timer {
            id: 1,
            label: String::new(),
            done_msg: String::new(),
            target_ms: 1_700_000_090_000,
            from_ms: 1_700_000_000_000,
            paused_ms: Some(90_000),
            warn_ms: None,
            scope: crate::timers::Scope::Both,
            plan_item_id: None,
            started_in_rehearsal: false,
        });
        let c = OutputContent {
            kind: Some("countdown".into()),
            reference: shown.reference,
            countdown_to: Some(shown.countdown_to),
            countdown_from: Some(shown.countdown_from),
            countdown_paused_ms: shown.countdown_paused_ms,
            countdown_warn_ms: shown.countdown_warn_ms,
            ..Default::default()
        };
        assert_eq!(shown.countdown_paused_ms, Some(90_000));
        assert_eq!(preflight(&c), Ok(()));
    }

    /// A WARNING THRESHOLD IS NOT A COUNTDOWN — THE OTHER HALF OF SITE 2.
    ///
    /// RG-149 put two new `countdown_*` fields on the wire form, and the danger in
    /// that is the mirror of the one the sweep was written for. `is_countdown`
    /// exists so a payload with no words in it is not refused as `Unsafe::Nothing`;
    /// widening it to recognise a warning field would do the opposite — it would
    /// wave through a payload that has a colour rule and no deadline, no text and no
    /// reference, and paint an empty screen while every log said the fire succeeded.
    ///
    /// So the recognition arms were deliberately NOT touched, and this is that
    /// decision written down where it can fail. `countdown_warn_ms` says WHEN to
    /// worry about a clock; it is not a clock.
    #[test]
    fn a_warning_threshold_with_no_deadline_is_still_an_empty_screen() {
        let c = OutputContent {
            countdown_warn_ms: Some(120_000),
            countdown_warn_default_ms: Some(150_000),
            ..Default::default()
        };
        assert_eq!(
            preflight(&c),
            Err(Unsafe::Nothing),
            "a warning window with nothing to warn about was taken for a countdown, \
             so the validator let a blank screen through"
        );
    }

    /// A TEMPLATE THE OUTPUT PAGE CANNOT READ IS REFUSED HERE, WHERE SOMEBODY IS
    /// STILL LISTENING.
    ///
    /// The output page does not fail loudly on a broken template — it falls back.
    /// So the wall shows the right words in the wrong look and nobody is told,
    /// which is the same class of silence the panic-control rule exists for.
    #[test]
    fn a_template_that_cannot_be_read_is_refused() {
        let mut c = content();
        c.template_json = Some("{not json".into());
        assert_eq!(preflight(&c), Err(Unsafe::BrokenTemplate));

        c.template_json = Some(r#"{"id":1,"name":"Sunday"}"#.into());
        assert!(preflight(&c).is_ok());

        // No template at all is the common case (the screen's own is used).
        c.template_json = None;
        assert!(preflight(&c).is_ok());
        c.template_json = Some("   ".into());
        assert!(
            preflight(&c).is_ok(),
            "an empty override is not a broken one"
        );
    }

    /// EVERY REFUSAL SAYS SOMETHING AN OPERATOR CAN ACT ON.
    ///
    /// A gate that refuses without a usable sentence has moved the problem, not
    /// solved it: the wall is still wrong and now the console is silent about it
    /// too.
    #[test]
    fn every_refusal_is_a_sentence_not_an_error_code() {
        for bad in [Unsafe::Nothing, Unsafe::BrokenTemplate] {
            let m = bad.message();
            assert!(m.len() > 40, "{bad:?}: too terse to act on");
            assert!(
                !m.contains("Err") && !m.contains("invalid") && !m.contains("None"),
                "{bad:?}: reads like a Rust error, not a sentence"
            );
            assert!(
                m.contains("screen"),
                "{bad:?}: must say what it means for the wall"
            );
        }
    }
}
