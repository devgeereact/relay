//! Content router: confidence gating, debounce, decides what fires where.
//!
//! Single responsibility: take Detections from detection.rs and decide
//! auto-fire / suggest / drop, apply the debounce window, and hand the final
//! "show this content" event to channels.rs. Owns the self-calibrating
//! threshold state per docs/DECISIONS.md — thresholds are configuration,
//! never hardcoded constants. See PROMPT.md Phase 6.
//!
//! Pure and deterministic: the caller passes a monotonic `now_ms`, so debounce
//! and gating are fully unit-testable without a clock.

use crate::detection::DetectionMethod;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Two-tier gate thresholds. Seed defaults per docs/DECISIONS.md (placeholders
/// until tuned against a real corpus); nudged per install by operator feedback.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Thresholds {
    pub auto_fire: f32,
    pub suggest: f32,
}

/// The sensitivity dial position that the out-of-box defaults correspond to.
/// The dial is the ONLY baseline — `Thresholds::default()` is defined as this
/// dial position, so the two can never drift apart again.
///
/// (They previously did: `default()` shipped 0.50/0.35 while
/// `from_sensitivity(50)` returned 0.90/0.60, and the dial's range couldn't even
/// reach 0.50. Saving a profile for any reason snapped the live thresholds from
/// one scale to the other and silently wiped the operator's calibration.)
pub const DEFAULT_SENSITIVITY: u8 = 50;

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds::from_sensitivity(DEFAULT_SENSITIVITY)
    }
}

/// Linear interpolate.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

impl Thresholds {
    /// Map the single operator "sensitivity" dial (0..=100) to the two-tier
    /// thresholds. Higher sensitivity → lower bars → the AI fires/suggests more
    /// readily (more catches, more noise). This sets the *baseline*; per-install
    /// feedback (`record_feedback`) nudges from here.
    ///
    /// Piecewise-linear, anchored so the mid dial position is exactly the
    /// shipped default (operator preference: auto-push above ~50%):
    ///
    /// | dial | auto_fire | suggest | behaviour              |
    /// |------|-----------|---------|------------------------|
    /// | 0    | 0.90      | 0.70    | cautious — few, sure   |
    /// | 50   | 0.50      | 0.30    | **the default**        |
    /// | 100  | 0.30      | 0.10    | eager — many, noisy    |
    ///
    /// `suggest` is not a curve of its own any more: it is `auto_fire` less
    /// `SUGGEST_BAND`, at every dial position (DECISIONS §117, the operator's
    /// instruction of 2026-09-23). It used to run 0.70 → 0.35 → 0.20 beside an
    /// auto bar of 0.90 → 0.50 → 0.30, so the band between "offer it" and "put it
    /// up" narrowed from 20 points to 10 as the dial went right — narrowest at
    /// exactly the end where an operator most wants things offered rather than
    /// fired. The auto-fire curve itself is untouched.
    ///
    /// Note these gate `Direct` detections only — semantic/ambiguous candidates
    /// can never auto-fire at ANY dial position (see `Router::decide`).
    pub fn from_sensitivity(sensitivity: u8) -> Self {
        let s = (sensitivity.min(100) as f32) / 100.0;
        let auto_fire = if s <= 0.5 {
            let t = s * 2.0; // 0..1 across the cautious half
            lerp(0.90, 0.50, t)
        } else {
            let t = (s - 0.5) * 2.0; // 0..1 across the eager half
            lerp(0.50, 0.30, t)
        };
        Thresholds {
            auto_fire,
            suggest: (auto_fire - SUGGEST_BAND).max(0.0),
        }
    }

    /// The gate as an operator reads it: how READY Relay is, 0 = never, 100 =
    /// anything.
    ///
    /// ── Why the printed figure is not the threshold ────────────────────────
    ///
    /// The operator's instruction, 2026-09-23: *"when the sensor is on Auto fire
    /// above 100, then it auto fires not when on 0."* The two figures were
    /// printed as the raw confidence bars, directly under the sensitivity slider
    /// — and they run the OPPOSITE way to it. The dial's cautious end (0) printed
    /// `Auto-fire above 90%`; its eager end (100) printed `30%`. Sitting under a
    /// control, a figure reads as a setting, and that one said the machine was
    /// keenest where it fires least.
    ///
    /// A threshold cannot be made to rise with eagerness — a bar you must clear
    /// is lower when more gets through, and that is arithmetic, not a choice. So
    /// the printed quantity changes instead of its direction — and then the WORD
    /// changed too, which is what finally settled it (the passage guard, 2026-09-25).
    ///
    /// This is what each bar NEEDS: a match's confidence, 0-100. So `auto_fire`
    /// is always the LARGER of the two, by exactly `SUGGEST_BAND`, because an
    /// auto-fire is the harder bar — which is the operator's second instruction,
    /// *"suggestions should be lower by 20 if auto fire is on 100 so auto fire
    /// has the higher priority"*, and it is true at every dial position.
    ///
    /// **The figures fall as the dial rises, and that is not a bug.** A bar you
    /// must clear is lower when more gets through; no labelling changes that.
    /// §117 tried to fix the confusion by inverting the number, which put the
    /// pair in an order that reads as a ranking — a suggestion outranking an
    /// auto-fire. The word "needs" fixes it instead: a smaller number under
    /// "needs" is obviously the easier bar rather than the keener setting, and
    /// the dial keeps its own direction as the control.
    ///
    /// It lives here, beside the mapping, for the reason `follows_dial` does: a
    /// copy of this arithmetic in the frontend would be a second opinion about
    /// one gate.
    pub fn readiness(self) -> GateReadiness {
        GateReadiness {
            auto_fire: readiness_of(self.auto_fire),
            suggest: readiness_of(self.suggest),
        }
    }

    /// Inverse of `from_sensitivity`, recovered from `auto_fire` (which is
    /// monotonic in the dial). Display-only: it positions the operator's single
    /// "sensitivity" slider from the stored thresholds so the two directions of
    /// the mapping live in ONE place (here), never duplicated in the frontend.
    /// The gate itself always uses the thresholds, never this number.
    pub fn to_sensitivity(self) -> u8 {
        let a = self.auto_fire;
        let s = if a >= 0.50 {
            // cautious half: auto_fire 0.90→0.50 maps to dial 0.0→0.5
            ((0.90 - a) / 0.80).clamp(0.0, 0.5)
        } else {
            // eager half: auto_fire 0.50→0.30 maps to dial 0.5→1.0
            0.5 + ((0.50 - a) / 0.40).clamp(0.0, 0.5)
        };
        (s * 100.0).round() as u8
    }

    /// Would the dial position this gate reports actually PRODUCE this gate?
    ///
    /// ── Why a read-out needs this and a thumb does not ──────────────────────
    ///
    /// `to_sensitivity` is a nearest-position answer, taken from `auto_fire`
    /// alone, and that is all a slider thumb needs. A READ-OUT needs more. Three
    /// things move `Router.thresholds` without touching the dial — `apply_profile`
    /// restoring what a voice profile LEARNED, a room being applied, and
    /// `record_feedback` on every confirm and dismiss — and after any of them the
    /// gate need not sit anywhere on this curve. It still has a nearest dial
    /// position, and the dial will still be drawn at it, so the operator is shown
    /// a number that reads as their own setting and is not.
    ///
    /// That is rule 35 on the one control governing what the AI may put on a wall
    /// unasked: a reading that says the same thing when the thing behind it has
    /// moved is not a reading. The surface asks this and says so in words.
    ///
    /// BOTH numbers are compared, and the second one is the load-bearing half.
    /// `record_feedback` corrects `auto_fire` and leaves `suggest` alone, so the
    /// reported dial position follows `auto_fire` while `suggest` stays behind —
    /// a comparison of `auto_fire` against its own inverse would be tautological
    /// on exactly the drift that actually happens.
    ///
    /// It lives here, beside the mapping, because it is a question ABOUT the
    /// mapping: a copy of this reasoning in the frontend would be a second opinion
    /// about the curve, and there is exactly one curve.
    pub fn follows_dial(self) -> bool {
        let dial = Thresholds::from_sensitivity(self.to_sensitivity());
        (dial.auto_fire - self.auto_fire).abs() < DIAL_READOUT_EPSILON
            && (dial.suggest - self.suggest).abs() < DIAL_READOUT_EPSILON
    }
}

/// How far the gate may sit from the dial's own figures before the read-out has
/// something to tell the operator.
///
/// Half of one percentage point, because the read-out is printed in whole
/// percentage points. A difference smaller than this cannot change a figure on
/// screen, so announcing it would be a caveat about nothing — and a caveat that
/// is permanently on is a caveat an operator learns to read past, which would
/// cost the real one its meaning. It is deliberately NOT bit-exactness: the
/// thresholds are floats and the dial is an integer, so a gate that came back
/// from SQLite as an `f64` must still be allowed to say it is the dial's.
pub const DIAL_READOUT_EPSILON: f32 = 0.005;

/// How far below the auto-fire bar the suggest bar sits, at every dial position.
///
/// Twenty points, on the operator's instruction of 2026-09-23. It replaces a
/// second interpolation curve whose band narrowed from 0.20 at the cautious end
/// to 0.10 at the eager end — narrowest exactly where a wide band of offers is
/// most wanted. One number, one relationship, and `from_sensitivity` is the only
/// place it is applied.
pub const SUGGEST_BAND: f32 = 0.20;

/// The gate as a pair of 0-100 figures that rise with the dial. See
/// `Thresholds::readiness`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateReadiness {
    pub auto_fire: u8,
    pub suggest: u8,
}

/// One threshold as the confidence it needs. Whole percentage points, because that is
/// what is printed; clamped, because a stored gate can be anything.
fn readiness_of(threshold: f32) -> u8 {
    ((threshold.clamp(0.0, 1.0) * 100.0).round() as i32).clamp(0, 100) as u8
}

/// Decide what thresholds a voice-profile save should land on.
///
/// Pure, so the rule is actually testable — it lives here rather than inline in
/// the Tauri command precisely because the bug it encodes (a profile rename
/// silently wiping the operator's accumulated calibration) was invisible for
/// exactly as long as it was unreachable from a test.
///
/// The rule: moving the sensitivity dial is the operator deliberately
/// re-baselining the gate, so re-derive. Any other edit must preserve whatever
/// the self-calibration has learned.
pub fn thresholds_on_profile_save(
    sensitivity_changed: bool,
    new_sensitivity: u8,
    stored: Thresholds,
) -> Thresholds {
    if sensitivity_changed {
        Thresholds::from_sensitivity(new_sensitivity.min(100))
    } else {
        stored
    }
}

/// What the router decided to do with a candidate detection.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RouteDecision {
    AutoFire,
    Suggest,
    Drop,
}

/// Repeat cooldown for the same verse.
///
/// **Derived from the STT rolling window, not chosen freely.** docs/SPEC.md
/// originally specified a 4–6s cooldown, and 5s was used — but the STT window is
/// 8s long and is re-transcribed about once a second, so one spoken reference is
/// re-detected on every pass for as long as it sits in that window. A 5s cooldown
/// is *shorter than the thing causing the repeats*: the verse would fire, go quiet
/// for 5s, then fire again while the operator was still looking at it.
///
/// So the cooldown must outlast the window that produces the duplicates. Anything
/// re-detected inside it is, definitionally, the same utterance being heard again
/// — not the preacher saying it twice. (And if the operator clears the screens,
/// `forget_last_fire` drops the memory anyway, so a real re-reference is never
/// stuck behind this.)
const DEFAULT_DEBOUNCE_MS: u64 = (crate::stt::WINDOW_SECS as u64 + 2) * 1_000;
/// How far a single operator decision moves the gate toward what that decision
/// implies. Deliberately gradual — one surprising verse shouldn't reshape the
/// gate, but a consistent pattern over a service should.
const FEEDBACK_ALPHA: f32 = 0.25;
/// How hard each feedback event pulls the gate back toward the operator's chosen
/// baseline (the sensitivity dial). This is what stops the calibration ratcheting.
const BASELINE_PULL: f32 = 0.04;
/// Keep the auto bar a little above a confidence the operator just rejected, so
/// the identical score doesn't immediately fire again.
const REJECT_MARGIN: f32 = 0.02;

pub struct Router {
    thresholds: Thresholds,
    /// The operator's chosen baseline (from the sensitivity dial). Feedback
    /// nudges away from this; decay always pulls back toward it. Without an
    /// anchor, calibration is a one-way ratchet.
    baseline: Thresholds,
    debounce_ms: u64,
    /// When each reference last reached the screen — for the repeat cooldown.
    ///
    /// **A MAP, not a single slot.** It was `Option<(String, u64)>`, which could
    /// only ever remember the most recent key: the moment a *different* verse
    /// fired, the previous one's cooldown was erased. That made the debounce
    /// defeatable by the exact thing it exists to absorb. A rolling window
    /// re-transcribed once a second does not yield one steady reference — it
    /// yields a mutating hypothesis, and two candidates alternating inside it
    /// cleared each other's memory on every pass. Live, 2026-07-26, one second
    /// apart: `2 Chronicles 7:1 · 1 Thessalonians 3:1 · 2 Chronicles 7:2 ·
    /// 2 Chronicles 7:1 · 2 Chronicles 7:2 · 2 Chronicles 7:1 · …` — eight
    /// broadcasts of two verses in eight seconds, in front of a congregation.
    ///
    /// Pruned to the cooldown window on every insert, so it stays a handful of
    /// entries across a whole service rather than growing with it.
    fired_at: HashMap<String, u64>,
    /// When each reference was last READ out of the rolling window — which is not
    /// the same as reaching a screen. Drives the corroboration rule in
    /// `decide_live`; pruned on insert like `fired_at`.
    sighted_at: HashMap<String, u64>,
    /// Confidence of the most recent AUTO-FIRE. `dismiss_detection` is an "undo"
    /// with no argument — it can't tell us what it rejected — so the router
    /// remembers what it just put on screen. Without this, a rejection is a blind
    /// nudge: rejecting a 0.99 fire and rejecting a 0.51 fire would move the gate
    /// by the same amount, which is not what either one means.
    last_fire_conf: Option<f32>,
    /// The last verse the router put on a screen — auto, manual, or a nav step
    /// (everything goes through `note_fired`).
    ///
    /// **Not a second copy of `ContextMemory.current`, and the difference is the
    /// one `liveCue` already records: position and on-air-ness are separate
    /// facts.** `ContextMemory` is where `→` resumes and deliberately survives a
    /// blackout, so a cleared screen still knows which passage it was walking.
    /// This is what the screens are actually showing, so a clear drops it
    /// (`forget_last_fire`) and a song replaces it (`forget_wall`). Asking
    /// `ContextMemory` instead would mean a verse cleared off the wall could never
    /// be read back onto it, which is the exact hole `forget_last_fire` exists to
    /// stop.
    ///
    /// **Not `fired_at` either**, which is a COOLDOWN and expires after ten
    /// seconds. The whole finding behind the passage guard is that a reading
    /// outlives that (11, 17 and 120 seconds measured), so the question "is this
    /// verse already up?" cannot be answered by anything with a clock in it.
    last_wall: Option<String>,
    /// May a verse Relay heard being READ go to the screens unattended?
    ///
    /// The operator's instruction of 2026-09-23, and the church's switch over it
    /// (`app_settings['detection.follow_the_reader']`, default ON). It lives on
    /// the router rather than in `detection` because this is the one door every
    /// candidate passes through — rule 36. `detection::for_quotation` decides what
    /// the EVIDENCE is; this decides whether the church wants it acted on, and a
    /// second construction site for quoted candidates could not get round it.
    follow_the_reader: bool,
}

impl Default for Router {
    fn default() -> Self {
        Router {
            thresholds: Thresholds::default(),
            baseline: Thresholds::default(),
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            fired_at: HashMap::new(),
            sighted_at: HashMap::new(),
            last_fire_conf: None,
            last_wall: None,
            // ON by default, as instructed. A church that wants the old behaviour
            // turns it off in Settings → AI & Detection.
            follow_the_reader: true,
        }
    }
}

impl Router {
    /// Gate a candidate detection. `now_ms` is a monotonic timestamp (audio
    /// position from the transcript), so this is deterministic and clock-free.
    ///
    /// `method` is a HARD gate, applied before the thresholds: only
    /// `DetectionMethod::Direct` may ever auto-fire. A semantic (TF-IDF) score
    /// is a raw cosine similarity, not a calibrated probability — comparing it
    /// against `auto_fire` is comparing against noise, and a sermon window that
    /// happens to share a few rare words with some verse could otherwise put the
    /// wrong scripture in front of the congregation with no human in the loop.
    /// Such candidates are capped at `Suggest` no matter how high they score.
    /// See docs/DECISIONS.md.
    ///
    /// ## The debounce is UNCONDITIONAL for the same verse
    ///
    /// It did not used to be: a match confident enough to be called "explicit"
    /// (≥0.95) was exempted from its own cooldown, on the theory that a preacher
    /// deliberately re-reading a verse shouldn't be swallowed by it.
    ///
    /// That theory does not survive contact with how STT actually works. Speech
    /// arrives as a *rolling window* that is re-transcribed roughly once a second,
    /// so one clearly-spoken "Romans chapter eight verse one" is detected again on
    /// every re-transcription for as long as it stays in the window. Being exempt
    /// from the cooldown, it re-fired **nine times, once a second** — the same
    /// verse re-broadcast and re-crossfaded on the projector, in front of the
    /// congregation. Caught in a live rehearsal, not by a test.
    ///
    /// And the theory was never sound anyway: re-firing a verse that is **already
    /// on the screen** accomplishes nothing. There is no swallowed re-reference to
    /// rescue — the verse is right there. So the same key is now always debounced.
    /// If the operator clears the screens, `forget_last_fire` drops the memory, so
    /// a genuine re-reference after a clear fires immediately.
    /// The live-path entry point: `decide`, plus the rule that a reference read out
    /// of a PARTIAL window has to be seen twice before it may reach a wall.
    ///
    /// ── Why this exists ─────────────────────────────────────────────────────────
    ///
    /// The worker re-decodes the same rolling window every step, and a shorter
    /// window decodes to different words than a longer one. Measured on real speech
    /// ("Romans chapter eight verse twenty eight", `stt::e2e_latency`), the
    /// intermediate decodes produced **Romans 8:16** and **Romans 8:21** before the
    /// window grew enough to settle on 8:28. Each was a complete, non-provisional,
    /// `Direct` reference — `is_provisional` cannot catch them, because nothing was
    /// cut off; whisper simply misheard a number with less context.
    ///
    /// At the old fixed cadence there were few enough passes that this was rare.
    /// Stepping faster makes it common, so the cadence change and this rule are one
    /// change: **latency comes from decoding more often, and safety comes from
    /// requiring the extra decodes to agree.** A transient misread appears in one
    /// pass and is gone; a reference the preacher actually said survives into the
    /// next. One corroboration costs one step — at the adaptive cadence roughly
    /// 250ms, still far inside the second the operator used to wait.
    ///
    /// A FINAL window is exempt and fires on first sight: the utterance is closed,
    /// there is no "next pass" coming to confirm it, and waiting for one would mean
    /// a verse spoken just before a pause never reaches the screen at all.
    ///
    /// Suggestions are NOT gated. A wrong suggestion costs the operator a glance; a
    /// wrong auto-fire costs a congregation the wrong scripture. Only the second one
    /// is worth latency.
    pub fn decide_live(
        &mut self,
        key: &str,
        confidence: f32,
        method: DetectionMethod,
        now_ms: u64,
        is_final: bool,
    ) -> RouteDecision {
        let corroborated = is_final || self.note_sighting(key, now_ms);
        // Checked BEFORE `decide`, never after: `decide` stamps `fired_at` when it
        // returns AutoFire, and a fire we then downgrade would leave the cooldown
        // holding a verse that never reached a screen — so the corroborating pass
        // one step later would be swallowed as a repeat. The gate has to decline
        // the fire, not undo it.
        // The SAME predicate `decide` uses, never a restatement of it. Asking
        // "is the score over the bar?" here was right while every auto-firing
        // method was gated on its score — and a `Reading` is not, so that question
        // answers NO for a reading at a cautious dial, the corroboration wait is
        // skipped, and `decide` fires it one line later anyway. A safety wait lost
        // by a predicate drifting out of step with the gate it guards is rule 34,
        // arriving silently.
        if !corroborated && self.may_reach_a_wall(method) && self.clears_the_bar(confidence, method)
        {
            return RouteDecision::Suggest;
        }
        self.decide(key, confidence, method, now_ms)
    }

    /// Record that `key` was read out of the current window. Returns whether it had
    /// already been seen recently — i.e. whether this is a corroboration.
    ///
    /// Sightings expire after `debounce_ms`, so a reference quoted again much later
    /// in the sermon starts over rather than inheriting a stale agreement.
    fn note_sighting(&mut self, key: &str, now_ms: u64) -> bool {
        let debounce = self.debounce_ms;
        let seen_before = self
            .sighted_at
            .get(key)
            .is_some_and(|t| now_ms.saturating_sub(*t) <= debounce);
        self.sighted_at.insert(key.to_string(), now_ms);
        if self.sighted_at.len() > 64 {
            self.sighted_at
                .retain(|_, t| now_ms.saturating_sub(*t) <= debounce);
        }
        seen_before
    }

    /// May a candidate detected THIS WAY reach a congregation unattended, in
    /// this install, right now?
    ///
    /// Two facts: what the method is (`may_auto_fire`, the hard rule) and whether
    /// the church has asked Relay to follow a reader (the switch). One place, so
    /// there is one answer.
    fn may_reach_a_wall(&self, method: DetectionMethod) -> bool {
        method.may_auto_fire() && (method != DetectionMethod::Reading || self.follow_the_reader)
    }

    /// Has this candidate cleared the numeric gate — for the methods that HAVE
    /// one?
    ///
    /// A `Reading` does not. Its qualification is a run of words the speaker said
    /// verbatim, held by exactly one verse (`detection::for_quotation`), and its
    /// `confidence` is that run length on a scale of its own. Comparing it with
    /// `auto_fire`, which is a parse probability, is the incomparable-scales
    /// mistake this whole module exists to prevent — read in the other direction
    /// it would mean a church at the cautious end of the dial needs a
    /// FIFTEEN-word run before Relay follows a reader, for no reason anybody
    /// could state. The dial governs what Relay does with a number; a reading
    /// does not bring one.
    fn clears_the_bar(&self, confidence: f32, method: DetectionMethod) -> bool {
        match method {
            DetectionMethod::Reading => true,
            _ => confidence >= self.thresholds.auto_fire,
        }
    }

    /// Is Relay following a reader in this install? See `follow_the_reader`.
    pub fn follows_the_reader(&self) -> bool {
        self.follow_the_reader
    }

    /// The church's switch. Applied at launch from `app_settings` and by
    /// `set_follow_the_reader` in main.rs; nothing else may move it.
    pub fn set_follow_the_reader(&mut self, on: bool) {
        self.follow_the_reader = on;
    }

    pub fn decide(
        &mut self,
        key: &str,
        confidence: f32,
        method: DetectionMethod,
        now_ms: u64,
    ) -> RouteDecision {
        // Uncalibrated methods can reach the operator, never the screen.
        if !self.may_reach_a_wall(method) {
            return if confidence >= self.thresholds.suggest {
                RouteDecision::Suggest
            } else {
                RouteDecision::Drop
            };
        }
        if self.clears_the_bar(confidence, method) {
            if let Some(t) = self.fired_at.get(key) {
                if now_ms.saturating_sub(*t) < self.debounce_ms {
                    // Already on screen, and said again within the cooldown —
                    // almost always the same utterance being re-transcribed.
                    return RouteDecision::Drop;
                }
            }
            // ONE SENTENCE HEARD TWO WAYS — 2026-09-20, live, on a congregation's
            // screens.
            //
            // The window decoded one utterance 3.2 seconds apart and disagreed with
            // itself about the book: `Numbers 10:29` at 0.88, then `Genesis 10:29`
            // at 0.88. The preacher said Numbers; the operator walked Numbers 10:30
            // to 10:32 by hand straight after, which is how we know.
            //
            // **Nothing above could see it.** The cooldown a few lines up is keyed
            // per REFERENCE, and those are two different references — the blind spot
            // rule 29 records for two verses in one window, in a new shape: one
            // sentence across two windows. `decide_live`'s corroboration cannot see
            // it either, because that rule waits for a second pass to AGREE and this
            // second pass did not disagree, it asserted a different book just as
            // confidently.
            //
            // **And no threshold could have saved it**, which is why this is a rule
            // and not a number. Both readings scored 0.88 while six CORRECT fires
            // that same morning scored 0.55; raising the bar discards the six and
            // keeps this one. Rule 10, in the costume it keeps returning in.
            //
            // The chapter and the verse agreeing while the book does not, inside the
            // cooldown, is treated as what it almost always is. The second reading
            // is OFFERED, never fired — not dropped, because the operator may want
            // it and a silent discard is a different lie. A genuine second citation
            // outside the cooldown is untouched, and so is any other verse.
            if let Some((book, chapter_verse)) = key.rsplit_once(' ') {
                let heard_another_way = self.fired_at.iter().any(|(fired, at)| {
                    now_ms.saturating_sub(*at) < self.debounce_ms
                        && fired
                            .rsplit_once(' ')
                            .is_some_and(|(b, cv)| cv == chapter_verse && b != book)
                });
                if heard_another_way {
                    return RouteDecision::Suggest;
                }
            }
            self.note_fired(key, now_ms);
            // Remember WHAT we put on screen, so a later "undo" is a proportional
            // correction rather than a blind nudge.
            //
            // ONLY WHEN THE NUMBER IS ONE THE GATE CAN LEARN FROM. `record_feedback`
            // falls back to this when a dismiss arrives with no argument, and a
            // `Reading`'s score is a word count — pushing the bar that governs
            // spoken references past it would make Relay deafer to every reference
            // anybody says because the operator cleared a reading off a wall. The
            // dismissal still counts; it just carries no number, which
            // `record_feedback` already handles.
            self.last_fire_conf = method.confidence_is_calibrated().then_some(confidence);
            RouteDecision::AutoFire
        } else if confidence >= self.thresholds.suggest {
            RouteDecision::Suggest
        } else {
            RouteDecision::Drop
        }
    }

    /// Forget what was last fired, so the very next detection of it fires again.
    ///
    /// Called when the operator clears or blacks out the screens. Without this,
    /// clearing the screen and having the preacher immediately re-reference the
    /// same verse would leave the screen stubbornly blank for the rest of the
    /// cooldown — the debounce would suppress the one fire the operator wants.
    ///
    /// Clears the remembered CONFIDENCE too, not just the key. `record_feedback`
    /// falls back to `last_fire_conf` when a dismiss arrives with no argument, so
    /// leaving it set meant a dismiss *after* a clear would tune the gate using the
    /// score of an auto-fire that is no longer on screen — correcting the router
    /// for a decision the operator was not actually reacting to.
    /// Clears the record of what is ON the wall too, and that half is load-bearing
    /// for the passage guard: rule B refuses to re-fire a verse the screens are
    /// already showing, and after a clear they are showing nothing. Without this a
    /// preacher who re-read the verse the operator had just cleared would find
    /// Relay silently declining to put it back — the same defect this function was
    /// written for, in the guard's costume.
    pub fn forget_last_fire(&mut self) {
        self.fired_at.clear();
        self.last_fire_conf = None;
        self.last_wall = None;
    }

    /// Stamp a reference as on-screen, and drop every entry whose cooldown has
    /// already expired.
    ///
    /// Pruning here (rather than never) is what keeps the per-reference cooldown
    /// from being a slow leak: an expired entry can no longer suppress anything,
    /// so keeping it only grows the map for the length of the service.
    fn note_fired(&mut self, key: &str, now_ms: u64) {
        let debounce = self.debounce_ms;
        self.fired_at
            .retain(|_, t| now_ms.saturating_sub(*t) < debounce);
        self.fired_at.insert(key.to_string(), now_ms);
    }

    /// What the screens are showing, as far as this module has been told. `None`
    /// when nothing is, or when nobody can say (see `last_wall`).
    pub fn wall(&self) -> Option<&str> {
        self.last_wall.as_deref()
    }

    /// A verse has actually gone out to the screens.
    ///
    /// **Called from `broadcast_with_clock` — the ONE door content leaves the
    /// machine by — and NOT from `note_fired`, which was the first attempt and was
    /// measurably wrong.** `decide` returns `AutoFire` per candidate, and rule 29
    /// then lets only rank 0 reach a wall: *"one window may inform the operator
    /// about several verses; it may put at most ONE on a wall."* So `note_fired`
    /// records verses that were demoted to suggestions and never shown.
    ///
    /// It was not a theoretical objection. Service 40 of 2026-09-25, 769.2 s:
    /// *"Jeremiah chapter 6 verse 16 verse 17 verse 17"* yields `6:16` at 0.95 AND
    /// `6:17` at 0.88, both `Direct`, both `AutoFire`. `6:16` went to the screens
    /// and `6:17` was offered — and with the record kept in `note_fired`, the wall
    /// read `Jeremiah 6:17`. Eleven seconds later the preacher read verse 16 aloud,
    /// the guard compared it with `6:17`, found no match and fired the duplicate the
    /// whole rule exists to stop. Caught by the bench, on real transcripts, not by
    /// reading the code.
    pub fn note_wall(&mut self, key: &str) {
        self.last_wall = Some(key.to_string());
    }

    /// Something that is NOT scripture has taken the screens — a song, a notice, a
    /// picture, a countdown — so no verse is on them any more.
    ///
    /// Called from the same branch of the same door `ContextMemory::forget` is
    /// called at (rule 38), and for the same reason: a content kind added next year
    /// is handled by construction. Deliberately does NOT touch `fired_at`, because
    /// the repeat cooldown and the RG-178 rule are about what was recently HEARD and
    /// a song does not change that.
    pub fn forget_wall(&mut self) {
        self.last_wall = None;
    }

    /// Operator manual override — always fires, bypassing thresholds and
    /// debounce entirely. This is a first-class control (CLAUDE.md), never a
    /// fallback: it must always win.
    pub fn manual_fire(&mut self, key: &str, now_ms: u64) -> RouteDecision {
        self.note_fired(key, now_ms);
        // The AI did not choose this, so there is no AI decision to learn from.
        // Clearing it means that if the operator immediately clears the screen,
        // we don't "correct" the gate for a fire it never made — undoing your own
        // manual action must not tighten the machine's threshold.
        self.last_fire_conf = None;
        RouteDecision::AutoFire
    }

    /// Feed operator confirm/reject signal back into the thresholds — the
    /// self-calibrating mechanism (docs/DECISIONS.md).
    ///
    /// `confidence` is the score of the detection the operator actually acted on.
    /// `None` means "the thing I last auto-fired" (the dismiss/undo path, which
    /// carries no argument); the router remembers it.
    ///
    /// The gate moves TOWARD what the decision implies, proportionally:
    /// - **Confirmed** a suggestion scoring `c` → the operator wanted that on
    ///   screen, so the auto bar was too high. Pull `auto_fire` down toward `c`.
    /// - **Rejected** an auto-fire scoring `c` → the bar was too low. Push
    ///   `auto_fire` up past `c`.
    ///
    /// Every event also decays both bars toward the operator's baseline. The old
    /// version had no decay and moved by a fixed ±0.01 regardless of evidence, so
    /// it was a one-way ratchet: `auto_fire` only ever rose and `suggest` only
    /// ever fell, and over a few services both pinned to their clamps and stayed
    /// there. It also meant rejecting a 0.99 fire nudged a 0.51 bar by 0.01 —
    /// a "correction" that corrects nothing.
    ///
    /// Bounded, and the invariant `auto_fire >= suggest` is preserved.
    pub fn record_feedback(&mut self, confirmed: bool, confidence: Option<f32>) {
        // Correction and decay are EXCLUSIVE, and that is the whole trick.
        //
        // Decay means "the operator told us nothing new — relax toward the gate
        // they actually chose." Correction means "the operator told us the gate is
        // wrong — move." Applying both on the same event makes them fight, and
        // decay wins: with a 0.04 pull toward a 0.50 baseline and a 0.25 push
        // toward 0.82, the bar converges to ~0.786 and NEVER climbs past the 0.80
        // score that keeps misfiring. The operator would reject the same wrong
        // verse all service and the gate would never learn.
        let mut corrected = false;
        if let Some(c) = confidence.or(self.last_fire_conf) {
            let c = c.clamp(0.0, 1.0);
            if confirmed {
                // Wanted on screen at `c`, but it only reached them as a
                // suggestion → the auto bar sits above `c` and should come down.
                if c < self.thresholds.auto_fire {
                    self.thresholds.auto_fire = lerp(self.thresholds.auto_fire, c, FEEDBACK_ALPHA);
                    corrected = true;
                }
            } else {
                // The gate let `c` through and the operator pulled it back off the
                // screen → the bar belongs above `c`.
                let target = (c + REJECT_MARGIN).min(0.99);
                if target > self.thresholds.auto_fire {
                    self.thresholds.auto_fire =
                        lerp(self.thresholds.auto_fire, target, FEEDBACK_ALPHA);
                    corrected = true;
                }
            }
        }

        // No correction was warranted → the gate is behaving, so let it drift home
        // to the operator's baseline. This is what stops the one-way ratchet.
        if !corrected {
            self.thresholds.auto_fire = lerp(
                self.thresholds.auto_fire,
                self.baseline.auto_fire,
                BASELINE_PULL,
            );
            self.thresholds.suggest = lerp(
                self.thresholds.suggest,
                self.baseline.suggest,
                BASELINE_PULL,
            );
        }

        // Bounds + the ordering invariant.
        self.thresholds.auto_fire = self.thresholds.auto_fire.clamp(0.20, 0.99);
        self.thresholds.suggest = self
            .thresholds
            .suggest
            .clamp(0.15, 0.90)
            .min(self.thresholds.auto_fire);
    }

    pub fn thresholds(&self) -> Thresholds {
        self.thresholds
    }

    /// Manual override slider in Settings — always available (DECISIONS.md).
    ///
    /// Also re-anchors the decay baseline: whatever the operator explicitly sets
    /// IS the new target that calibration relaxes back toward. If the baseline
    /// stayed put here, feedback would keep dragging the gate back to a number the
    /// operator had already overruled.
    pub fn set_thresholds(&mut self, t: Thresholds) {
        let t = Thresholds {
            auto_fire: t.auto_fire.clamp(0.0, 1.0),
            suggest: t.suggest.clamp(0.0, 1.0).min(t.auto_fire),
        };
        self.thresholds = t;
        self.baseline = t;
    }

    /// Re-anchor the decay baseline WITHOUT disturbing the currently-learned
    /// thresholds — used when restoring a profile at startup, where the stored
    /// thresholds are the learned ones and the dial is the baseline they drift back to.
    /// The anchor calibration decays toward. Readable so a test can assert that
    /// moving a control moved the ANCHOR and not merely the gate — the half of
    /// R4-10 that bites inside a single service rather than at the next launch.
    pub fn baseline(&self) -> Thresholds {
        self.baseline
    }

    pub fn set_baseline(&mut self, t: Thresholds) {
        self.baseline = Thresholds {
            auto_fire: t.auto_fire.clamp(0.0, 1.0),
            suggest: t.suggest.clamp(0.0, 1.0).min(t.auto_fire),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIRECT: DetectionMethod = DetectionMethod::Direct;

    #[test]
    fn sensitivity_round_trips_through_the_thresholds() {
        // The dial anchors: 0 = cautious, 50 = the shipped default, 100 = eager.
        assert_eq!(Thresholds::from_sensitivity(50).to_sensitivity(), 50);
        assert_eq!(Thresholds::from_sensitivity(0).to_sensitivity(), 0);
        assert_eq!(Thresholds::from_sensitivity(100).to_sensitivity(), 100);
        // Every dial position recovers to itself (±1 rounding) across the range —
        // so the Live slider drawn from the stored thresholds lands where the
        // operator left it, in both halves of the piecewise curve.
        for s in 0..=100u8 {
            let back = Thresholds::from_sensitivity(s).to_sensitivity();
            assert!(
                (back as i16 - s as i16).abs() <= 1,
                "sensitivity {s} recovered as {back}"
            );
        }
        // The default thresholds ARE dial 50 (the one baseline, DECISIONS §19).
        assert_eq!(Thresholds::default().to_sensitivity(), 50);
    }

    /// ── THE DIAL POSITION IS NOT ALWAYS AN EXPLANATION OF THE GATE ──────────
    ///
    /// `to_sensitivity` answers "which dial position is nearest to this gate",
    /// and it answers it from `auto_fire` alone. That is the right answer for
    /// placing a thumb and the wrong answer for a READ-OUT, because a gate the
    /// learning has walked away from still has a nearest dial position and will
    /// happily report one. A surface that shows the dial at 40 cannot, from that
    /// number, tell "the operator set 40" from "the operator set 40 three weeks
    /// ago and the gate has moved since" — which is rule 35 on the one control
    /// governing what the AI may put on a wall unasked.
    ///
    /// `follows_dial` is the missing half: it asks whether the dial position
    /// being shown would actually PRODUCE the gate being shown beside it.
    #[test]
    fn a_gate_on_the_curve_is_explained_by_its_dial_and_a_learned_one_is_not() {
        // Every dial position the operator can reach is, by construction, on the
        // curve. Nothing the dial itself can do may ever raise the caveat.
        for s in 0..=100u8 {
            assert!(
                Thresholds::from_sensitivity(s).follows_dial(),
                "dial {s} does not explain the gate its own mapping produced"
            );
        }

        // THE STATE A REAL INSTALL WAS FOUND IN (the Settings two-slider control,
        // screenshotted at auto-fire 99% / suggest 69%). The dial cannot express
        // an auto-fire of 0.99 at all — its cautious end is 0.90 — so
        // `to_sensitivity` clamps to 0 and reports a position that would produce
        // 0.90/0.70. Shown without a caveat, that is a read-out claiming the
        // operator chose the most cautious setting there is while the gate sits
        // above the top of the scale.
        let screenshot = Thresholds {
            auto_fire: 0.99,
            suggest: 0.69,
        };
        assert_eq!(screenshot.to_sensitivity(), 0);
        assert!(!screenshot.follows_dial());

        // THE STATE A LIVE SERVICE WAS FOUND IN (DECISIONS §32.2): auto_fire
        // 0.832 beside a profile reading sensitivity 50, whose mapping is 0.50.
        assert!(!Thresholds {
            auto_fire: 0.832,
            suggest: 0.35,
        }
        .follows_dial());

        // THE CASE `to_sensitivity` IS STRUCTURALLY BLIND TO, and the reason this
        // function reads BOTH numbers. `record_feedback` corrects `auto_fire` and
        // leaves `suggest` where it was, so the learning walks the pair off the
        // curve while the reported dial position tracks `auto_fire` alone. Ask
        // only about `auto_fire` and a gate of 0.596/0.350 at dial 38 — whose own
        // mapping is 0.596/0.434 — reads as untouched.
        let mut r = Router::default(); // dial 50: 0.500 / 0.350
        r.set_thresholds(Thresholds {
            auto_fire: 0.596,
            suggest: 0.350,
        });
        let learned = r.thresholds();
        let implied = Thresholds::from_sensitivity(learned.to_sensitivity());
        assert!(
            (implied.auto_fire - learned.auto_fire).abs() < DIAL_READOUT_EPSILON,
            "this case is only interesting while auto_fire alone still agrees"
        );
        assert!(!learned.follows_dial());

        // AND THE TOLERANCE IS NOT ZERO, deliberately. The read-out is printed in
        // whole percentage points, so a difference that cannot change a printed
        // figure has nothing to say to an operator, and a caveat that is always on
        // is a caveat nobody reads.
        assert!(Thresholds {
            auto_fire: 0.5 + 0.001,
            suggest: 0.30 - 0.001,
        }
        .follows_dial());
    }

    #[test]
    fn gates_by_tier() {
        let mut r = Router::default(); // 0.50 / 0.30 (push above ~50%)
                                       // Above auto-fire → straight to the screens.
        assert_eq!(
            r.decide("John 3:16", 0.70, DIRECT, 0),
            RouteDecision::AutoFire
        );
        // Between suggest and auto-fire → operator-confirmable suggestion.
        assert_eq!(
            r.decide("Romans 8:28", 0.42, DIRECT, 100),
            RouteDecision::Suggest
        );
        // Below suggest → dropped silently.
        assert_eq!(
            r.decide("Psalms 23:1", 0.25, DIRECT, 200),
            RouteDecision::Drop
        );
    }

    /// ONE SENTENCE, DECODED TWICE, INTO TWO DIFFERENT BOOKS — 2026-09-20, live.
    ///
    /// The rolling window decoded the same utterance 3.2 seconds apart:
    ///
    /// ```text
    /// 375.4  "Numbers chapter 10. I'll read verse 29."            -> Numbers 10:29  0.88  AUTO
    /// 378.6  "Genesis 10, I'll read verse 29. It's the New King"  -> Genesis 10:29  0.88  AUTO
    /// ```
    ///
    /// The preacher said Numbers. The operator then walked Numbers 10:30, 10:31
    /// and 10:32 by hand, which is the corroboration. **Genesis 10:29 reached the
    /// congregation's screens and nobody said it.**
    ///
    /// Nothing in the router could see it. The debounce is keyed per REFERENCE and
    /// those are two different references — the same blind spot rule 29 records for
    /// two verses sharing one window, in a new shape: not two verses in one window,
    /// but one sentence in two windows. Corroboration (`decide_live`) cannot catch
    /// it either: that rule holds a reference until a second pass AGREES, and this
    /// second pass did not disagree, it asserted a different book at the same
    /// confidence.
    ///
    /// **And confidence could never have separated them.** Both scored 0.88, while
    /// six CORRECT fires that morning scored 0.55. Raising the bar would have
    /// discarded the six and kept this one — rule 10, in the costume it keeps
    /// coming back in.
    ///
    /// So the chapter and verse agreeing while the book does not, inside the
    /// cooldown, is treated as what it almost always is: one utterance heard two
    /// ways. The second is offered, never fired. It is NOT dropped — the operator
    /// may want it, and a silent discard would be a different lie.
    #[test]
    fn a_second_book_at_the_same_chapter_and_verse_is_offered_never_fired() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("Numbers 10:29", 0.88, DIRECT, 375_400),
            RouteDecision::AutoFire,
            "the first reading of the sentence must still reach the screen"
        );
        assert_eq!(
            r.decide("Genesis 10:29", 0.88, DIRECT, 378_600),
            RouteDecision::Suggest,
            "the same chapter and verse in a different book, 3.2s later, auto-fired"
        );
    }

    /// The half that must not regress, in four directions.
    #[test]
    fn the_second_book_rule_is_narrow() {
        let mut r = Router::default();

        // 1. A DIFFERENT verse in a different book is untouched. This is the
        //    ordinary case of a preacher moving between books and it must not slow.
        assert_eq!(
            r.decide("Numbers 10:29", 0.9, DIRECT, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("Genesis 1:1", 0.9, DIRECT, 1_000),
            RouteDecision::AutoFire,
            "an unrelated verse was held back"
        );

        // 2. OUTSIDE the cooldown it is two real citations, not one utterance.
        let mut r = Router::default();
        assert_eq!(
            r.decide("Numbers 10:29", 0.9, DIRECT, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("Genesis 10:29", 0.9, DIRECT, 60_000),
            RouteDecision::AutoFire,
            "a genuine second citation a minute later was held back"
        );

        // 3. The SAME reference still Drops rather than Suggests. The existing
        //    debounce owns that case and this rule must not take it over: a repeat
        //    is already on the screen, and offering it again is noise.
        let mut r = Router::default();
        assert_eq!(
            r.decide("Numbers 10:29", 0.9, DIRECT, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("Numbers 10:29", 0.9, DIRECT, 1_000),
            RouteDecision::Drop,
            "the same-verse debounce changed behaviour"
        );

        // 4. A numbered book is split correctly. `1 Corinthians 13:4` must yield
        //    the book `1 Corinthians`, not `1`.
        let mut r = Router::default();
        assert_eq!(
            r.decide("1 Corinthians 13:4", 0.9, DIRECT, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("2 Corinthians 13:4", 0.9, DIRECT, 2_000),
            RouteDecision::Suggest,
            "a numbered book was split on the wrong space"
        );
    }

    /// THE load-bearing test of this module. A TF-IDF cosine is not a
    /// probability; a paraphrase match must never reach the congregation's
    /// screen without a human confirming it — at ANY score, at ANY sensitivity.
    #[test]
    fn semantic_can_never_auto_fire() {
        for s in 0..=100u8 {
            let mut r = Router::default();
            r.set_thresholds(Thresholds::from_sensitivity(s));
            // Every confidence, including a perfect 1.0, and every dial position.
            for conf in [0.51, 0.75, 0.95, 0.99, 1.0] {
                // `Quoted` joined this list on 2026-09-20 and it is the one most
                // likely to be argued with, because its evidence LOOKS strong: a
                // contiguous run of a preacher's words that is verbatim in one
                // verse. It is still not a spoken reference. A preacher quotes
                // far more scripture than a congregation is shown, so the run
                // length says which verse and says nothing whatever about
                // whether anybody wants it on a wall.
                //
                // `UncertainBook` and `UncertainNumber` joined it on 2026-09-25 with
                // RG-305, and the gap they closed is worth stating: both were capped
                // here by `may_auto_fire` from the day they were added and NEITHER was
                // in this list, so the property test that exists to prove the gate is
                // the method covered three of five. The citation-doubt rule now demotes
                // a 0.95 `Direct` into one of those two, which makes the cap the whole
                // of its safety claim — a claim that was resting on a test that did not
                // check it.
                for m in [
                    DetectionMethod::Semantic,
                    DetectionMethod::Ambiguous,
                    DetectionMethod::Quoted,
                    DetectionMethod::UncertainBook,
                    DetectionMethod::UncertainNumber,
                ] {
                    assert_ne!(
                        r.decide("John 3:16", conf, m, 0),
                        RouteDecision::AutoFire,
                        "{m:?} auto-fired at conf={conf} sensitivity={s}"
                    );
                }
            }
            // A direct hit at the same confidence DOES fire — proving the gate is
            // the method, not just a high bar that happens to reject everything.
            assert_eq!(
                r.decide("John 3:16", 1.0, DIRECT, 0),
                RouteDecision::AutoFire,
                "direct failed to fire at sensitivity={s}"
            );
        }
    }

    /// Above the suggest bar, an uncalibrated hit still reaches the operator —
    /// it's demoted, not silenced. Losing paraphrase matches entirely would be
    /// its own regression.
    #[test]
    fn semantic_above_suggest_is_offered_to_the_operator() {
        let mut r = Router::default(); // suggest = 0.35
        assert_eq!(
            r.decide("John 3:16", 0.99, DetectionMethod::Semantic, 0),
            RouteDecision::Suggest
        );
    }

    /// ...but a genuinely weak semantic hit is still dropped, not surfaced.
    #[test]
    fn semantic_below_suggest_is_dropped() {
        let mut r = Router::default(); // suggest = 0.35
        assert_eq!(
            r.decide("John 3:16", 0.20, DetectionMethod::Semantic, 0),
            RouteDecision::Drop
        );
    }

    /// A semantic match must not poison the debounce slot for a later direct
    /// match of the same verse — the operator confirming a paraphrase, then the
    /// preacher actually reading the reference aloud, is a normal sequence.
    #[test]
    fn semantic_does_not_consume_the_debounce_slot() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("John 3:16", 0.99, DetectionMethod::Semantic, 0),
            RouteDecision::Suggest
        );
        // Immediately after, a real spoken reference to the same verse fires.
        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, 500),
            RouteDecision::AutoFire
        );
    }

    #[test]
    fn debounces_same_verse_repeat() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, 0),
            RouteDecision::AutoFire
        );
        // Same verse, still inside the cooldown → dropped.
        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, 2_000),
            RouteDecision::Drop
        );
        // Past the cooldown → a genuinely new reference, fires again.
        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, DEFAULT_DEBOUNCE_MS + 1),
            RouteDecision::AutoFire
        );
    }

    /// The cooldown MUST outlast the STT rolling window, or the window will keep
    /// re-detecting the same utterance after the cooldown expires and the verse
    /// will re-fire on the projector. These two constants are coupled; this test
    /// is what stops someone "tuning" one of them in isolation.
    #[test]
    fn the_cooldown_outlasts_the_stt_window_that_causes_the_repeats() {
        let window_ms = crate::stt::WINDOW_SECS as u64 * 1_000;
        assert!(
            DEFAULT_DEBOUNCE_MS > window_ms,
            "debounce {DEFAULT_DEBOUNCE_MS}ms must exceed the {window_ms}ms STT window"
        );
    }

    /// THE regression test from the live rehearsal.
    ///
    /// STT re-transcribes a rolling window about once a second, so one clearly
    /// spoken "Romans chapter eight verse one" is re-detected on every pass while
    /// it stays in the window. A high-confidence match used to be EXEMPT from the
    /// debounce, so it re-fired nine times, once a second — the same verse
    /// re-broadcasting and re-crossfading on the projector, live, in front of a
    /// congregation. No unit test caught it; reading a verse aloud did.
    #[test]
    fn a_high_confidence_verse_fires_once_not_once_per_retranscription() {
        let mut r = Router::default();
        // t=0: the phrase enters the STT window and fires.
        assert_eq!(
            r.decide("Romans 8:1", 0.95, DIRECT, 0),
            RouteDecision::AutoFire
        );
        // Every re-transcription for as long as the phrase sits in the STT
        // window must be dropped — it is already on the screen.
        for t in 1..=crate::stt::WINDOW_SECS as u64 {
            assert_eq!(
                r.decide("Romans 8:1", 0.95, DIRECT, t * 1_000),
                RouteDecision::Drop,
                "re-fired at t={t}s — the projector would flicker"
            );
        }
    }

    /// THE regression test from the live service of 2026-07-26.
    ///
    /// The debounce above only ever saw the SAME key twice in a row, and that is
    /// the only shape it could catch: `last_fire` was a single slot, so any other
    /// verse firing in between ERASED the cooldown of the one before it.
    ///
    /// A rolling window re-transcribed once a second does not produce one steady
    /// reference — it produces a mutating hypothesis. Two candidates alternating
    /// inside it therefore defeated the debounce completely, each clearing the
    /// other's memory. From the live service, one second apart:
    ///
    ///     2 Chronicles 7:1 · 1 Thessalonians 3:1 · 2 Chronicles 7:2 ·
    ///     2 Chronicles 7:1 · 2 Chronicles 7:2 · 2 Chronicles 7:1 · …
    ///
    /// Eight broadcasts of two verses in eight seconds, in front of a
    /// congregation. The cooldown must be per-reference, not per-last-fire.
    #[test]
    fn two_alternating_verses_do_not_erase_each_others_cooldown() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("2 Chronicles 7:1", 0.88, DIRECT, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("2 Chronicles 7:2", 0.95, DIRECT, 1_000),
            RouteDecision::AutoFire
        );

        // Both are on the cooldown clock now. Neither may fire again while the
        // utterance that produced them is still inside the STT window, no matter
        // how they interleave.
        for t in 2..=crate::stt::WINDOW_SECS as u64 {
            assert_eq!(
                r.decide("2 Chronicles 7:1", 0.88, DIRECT, t * 1_000),
                RouteDecision::Drop,
                "7:1 re-fired at t={t}s — the other verse had erased its cooldown"
            );
            assert_eq!(
                r.decide("2 Chronicles 7:2", 0.95, DIRECT, t * 1_000 + 500),
                RouteDecision::Drop,
                "7:2 re-fired at t={t}s — the other verse had erased its cooldown"
            );
        }
    }

    /// The per-reference cooldown must not become a memory leak across a service.
    /// It must also still EXPIRE — a verse genuinely referenced again half an hour
    /// later is a new utterance and has to reach the screen.
    #[test]
    fn a_per_reference_cooldown_still_expires() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("Psalm 23:1", 0.95, DIRECT, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, 1_000),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("Psalm 23:1", 0.95, DIRECT, DEFAULT_DEBOUNCE_MS + 1),
            RouteDecision::AutoFire,
            "a genuine later re-reference must still fire"
        );
    }

    /// But clearing the screens forgets it: nothing is showing any more, so the
    /// very next reference to that verse must fire, cooldown or not. Otherwise the
    /// operator clears the screen, the preacher says the verse again, and the wall
    /// stays stubbornly blank.
    #[test]
    fn clearing_the_screens_lets_the_same_verse_fire_again_immediately() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, 1_000),
            RouteDecision::Drop
        );

        r.forget_last_fire(); // operator hit Esc / Blackout

        assert_eq!(
            r.decide("John 3:16", 0.95, DIRECT, 1_500),
            RouteDecision::AutoFire
        );
    }

    #[test]
    fn different_verse_not_debounced() {
        let mut r = Router::default();
        r.decide("John 3:16", 0.95, DIRECT, 0);
        assert_eq!(
            r.decide("Romans 8:28", 0.95, DIRECT, 1_000),
            RouteDecision::AutoFire
        );
    }

    #[test]
    fn manual_fire_always_wins() {
        let mut r = Router::default();
        r.decide("John 3:16", 0.95, DIRECT, 0);
        // immediate repeat via manual override still fires
        assert_eq!(r.manual_fire("John 3:16", 500), RouteDecision::AutoFire);
    }

    /// The operator override is a first-class control (CLAUDE.md): it bypasses
    /// the method gate too. A human choosing to put a paraphrase on screen is
    /// always allowed — it's the AI doing it unasked that is forbidden.
    #[test]
    fn manual_fire_bypasses_the_method_gate() {
        let mut r = Router::default();
        assert_eq!(r.manual_fire("Romans 8:28", 0), RouteDecision::AutoFire);
    }

    /// Rejecting an auto-fire must raise the bar ABOVE the thing that fired —
    /// proportionally to what actually went wrong, not by a blind fixed step.
    #[test]
    fn rejecting_an_auto_fire_raises_the_bar_past_it() {
        let mut r = Router::default(); // auto_fire 0.50
        r.decide("John 3:16", 0.80, DIRECT, 0); // fires at 0.80
        let before = r.thresholds().auto_fire;
        r.record_feedback(false, None); // operator pulls it back off screen
        let after = r.thresholds().auto_fire;
        assert!(after > before, "{before} -> {after}");
        // Repeated rejections of the same score converge above it, so the same
        // wrong verse stops firing. The old fixed ±0.01 step would have taken 30+
        // rejections to climb from 0.50 past 0.80 — an entire service of wrong verses.
        for _ in 0..12 {
            r.record_feedback(false, None);
        }
        assert!(
            r.thresholds().auto_fire > 0.80,
            "did not climb past the rejected score: {}",
            r.thresholds().auto_fire
        );
    }

    /// Confirming a suggestion means "this should have gone straight up" — so the
    /// auto bar should come DOWN toward that score.
    #[test]
    fn confirming_a_suggestion_lowers_the_auto_bar_toward_it() {
        let mut r = Router::default();
        r.set_thresholds(Thresholds {
            auto_fire: 0.80,
            suggest: 0.35,
        });
        let before = r.thresholds().auto_fire;
        r.record_feedback(true, Some(0.60));
        assert!(r.thresholds().auto_fire < before);
    }

    /// THE anti-ratchet test. The old loop only ever moved each bar one way, so a
    /// run of one-sided feedback pinned it to a clamp permanently — the gate could
    /// never recover, even once the operator's behaviour changed. Decay toward the
    /// baseline must bring it home.
    #[test]
    fn calibration_decays_back_to_baseline_and_never_pins() {
        let mut r = Router::default();
        let base = r.thresholds().auto_fire;

        // A long run of rejections drives the bar up...
        for _ in 0..50 {
            r.record_feedback(false, Some(0.95));
        }
        let stressed = r.thresholds().auto_fire;
        assert!(stressed > base);
        assert!(stressed <= 0.99);

        // ...and once the operator stops rejecting, it relaxes back toward their
        // chosen baseline rather than staying stuck at the ceiling forever.
        for _ in 0..200 {
            r.record_feedback(true, None); // no evidence → decay only
        }
        let relaxed = r.thresholds().auto_fire;
        assert!(
            (relaxed - base).abs() < 0.02,
            "did not decay back to baseline {base}: {relaxed}"
        );
    }

    /// Undoing your OWN manual fire must not tighten the machine's gate — you
    /// weren't correcting the AI, it never made a decision.
    #[test]
    fn dismissing_after_a_manual_fire_does_not_punish_the_ai() {
        let mut r = Router::default();
        r.decide("John 3:16", 0.95, DIRECT, 0); // AI fired something
        r.manual_fire("Romans 8:28", 1_000); // operator overrode with their own pick
        let before = r.thresholds().auto_fire;
        r.record_feedback(false, None); // operator clears their own choice
                                        // Only baseline decay applies — no upward correction from the stale AI fire.
        assert!(r.thresholds().auto_fire <= before + 1e-6);
    }

    #[test]
    fn feedback_stays_bounded_and_ordered() {
        let mut r = Router::default();
        for i in 0..200 {
            r.record_feedback(i % 2 == 0, Some(if i % 3 == 0 { 0.99 } else { 0.10 }));
            let t = r.thresholds();
            assert!(t.auto_fire <= 0.99 && t.auto_fire >= 0.20, "{t:?}");
            assert!(t.suggest <= t.auto_fire, "{t:?}");
        }
    }

    /// The dial and the default MUST be the same baseline. When these two
    /// drifted apart, saving a voice profile for any reason (even a rename)
    /// silently snapped the live thresholds onto the other scale and wiped the
    /// operator's accumulated calibration. This test is what stops that
    /// recurring — it fails the moment the two disagree again.
    #[test]
    fn default_is_exactly_the_mid_dial_position() {
        let mid = Thresholds::from_sensitivity(DEFAULT_SENSITIVITY);
        let def = Thresholds::default();
        assert!((mid.auto_fire - def.auto_fire).abs() < 1e-6);
        assert!((mid.suggest - def.suggest).abs() < 1e-6);
        // And it is the documented operator preference: push above ~50%.
        assert!((def.auto_fire - 0.50).abs() < 1e-4, "{}", def.auto_fire);
        // 0.30, not the 0.35 this line pinned until 2026-09-23: `suggest` is
        // `auto_fire - SUGGEST_BAND` everywhere now (DECISIONS §117).
        assert!((def.suggest - 0.30).abs() < 1e-4, "{}", def.suggest);
    }

    #[test]
    fn sensitivity_maps_and_stays_ordered() {
        let mid = Thresholds::from_sensitivity(50);
        let hi = Thresholds::from_sensitivity(100);
        let lo = Thresholds::from_sensitivity(0);
        // Higher sensitivity lowers both bars; lower raises them.
        assert!(hi.auto_fire < mid.auto_fire && mid.auto_fire < lo.auto_fire);
        assert!(hi.suggest < mid.suggest && mid.suggest < lo.suggest);
        // The dial spans the whole useful range — including the default, which
        // the old mapping (0.83..0.97) could not even express.
        assert!((lo.auto_fire - 0.90).abs() < 1e-4);
        assert!((hi.auto_fire - 0.30).abs() < 1e-4);
        // Invariant + monotonicity hold across the whole range.
        let mut prev = f32::MAX;
        for s in 0..=100u8 {
            let t = Thresholds::from_sensitivity(s);
            assert!(t.suggest <= t.auto_fire, "s={s}");
            assert!(t.auto_fire <= prev + 1e-6, "not monotonic at s={s}");
            prev = t.auto_fire;
        }
        // Clamps above 100.
        assert_eq!(
            Thresholds::from_sensitivity(200).auto_fire,
            Thresholds::from_sensitivity(100).auto_fire
        );
    }

    /// The regression test for the calibration-wipe bug: an operator renames a
    /// profile (or edits its language, or its bias terms) and the gate they have
    /// spent a whole service teaching must survive it untouched.
    #[test]
    fn saving_a_profile_without_moving_the_dial_preserves_calibration() {
        let learned = Thresholds {
            auto_fire: 0.62,
            suggest: 0.40,
        };
        let after = thresholds_on_profile_save(false, DEFAULT_SENSITIVITY, learned);
        assert_eq!(after.auto_fire, 0.62);
        assert_eq!(after.suggest, 0.40);
    }

    /// ...but deliberately moving the dial IS a re-baseline, and must reset.
    #[test]
    fn moving_the_dial_rebaselines_the_gate() {
        let learned = Thresholds {
            auto_fire: 0.62,
            suggest: 0.40,
        };
        let after = thresholds_on_profile_save(true, 100, learned);
        let expect = Thresholds::from_sensitivity(100);
        assert!((after.auto_fire - expect.auto_fire).abs() < 1e-6);
        assert!((after.suggest - expect.suggest).abs() < 1e-6);
    }

    #[test]
    fn set_thresholds_preserves_invariant() {
        let mut r = Router::default();
        r.set_thresholds(Thresholds {
            auto_fire: 0.5,
            suggest: 0.9, // invalid: above auto_fire
        });
        assert!(r.thresholds().suggest <= r.thresholds().auto_fire);
    }
}

#[cfg(test)]
mod corroboration {
    use super::*;
    const DIRECT: DetectionMethod = DetectionMethod::Direct;

    /// THE BUG THIS EXISTS FOR, measured on real speech.
    ///
    /// Decoding the rolling window more often means decoding it while it is still
    /// short, and a short window mishears numbers: "Romans chapter eight verse
    /// twenty eight" read as **Romans 8:16**, then **8:21**, before settling on
    /// 8:28. Complete, non-provisional, `Direct` — nothing else in the pipeline can
    /// tell them apart from the real thing. A misread appears once; the reference
    /// the preacher actually said survives into the next pass.
    #[test]
    fn a_reference_read_once_from_a_partial_window_may_not_reach_the_wall() {
        let mut r = Router::default();
        assert_eq!(
            r.decide_live("Romans 8:16", 0.95, DIRECT, 0, false),
            RouteDecision::Suggest,
            "a first sighting from a partial window auto-fired — this is the misread \
             that put the wrong verse on the wall"
        );
    }

    /// ...and the corroborating pass fires it, so the cost is one step, not a veto.
    #[test]
    fn the_second_sighting_fires_it() {
        let mut r = Router::default();
        assert_eq!(
            r.decide_live("Romans 8:28", 0.95, DIRECT, 0, false),
            RouteDecision::Suggest
        );
        assert_eq!(
            r.decide_live("Romans 8:28", 0.95, DIRECT, 250, false),
            RouteDecision::AutoFire,
            "a reference the decoder saw twice is the one the preacher said"
        );
    }

    /// A FINAL window fires on first sight. Without this exemption a verse spoken
    /// just before a pause would never reach the screen at all: the utterance closes,
    /// the window clears, and the corroborating pass that was supposed to confirm it
    /// never comes.
    #[test]
    fn a_final_window_needs_no_corroboration() {
        let mut r = Router::default();
        assert_eq!(
            r.decide_live("John 3:16", 0.95, DIRECT, 0, true),
            RouteDecision::AutoFire,
            "a closed utterance has no next pass to wait for"
        );
    }

    /// The declined fire must not poison the cooldown. `decide` stamps `fired_at`
    /// when it returns AutoFire, so checking corroboration AFTER it would leave the
    /// debounce holding a verse that never reached a screen — and swallow the real
    /// fire one step later. That is why the check happens first.
    #[test]
    fn declining_a_fire_does_not_start_its_cooldown() {
        let mut r = Router::default();
        r.decide_live("John 3:16", 0.95, DIRECT, 0, false); // declined -> Suggest
        assert_eq!(
            r.decide_live("John 3:16", 0.95, DIRECT, 10, false),
            RouteDecision::AutoFire,
            "the declined fire started a cooldown and ate the corroborated one"
        );
    }

    /// Suggestions are never gated: a wrong suggestion costs a glance, a wrong
    /// auto-fire costs a congregation. Only one of those is worth latency.
    #[test]
    fn a_suggestion_still_arrives_on_first_sight() {
        let mut r = Router::default();
        let mid = (Thresholds::default().suggest + Thresholds::default().auto_fire) / 2.0;
        assert_eq!(
            r.decide_live("John 3:16", mid, DIRECT, 0, false),
            RouteDecision::Suggest,
            "the operator must still see it immediately"
        );
    }

    /// A sighting expires with the cooldown, so a verse quoted again much later in
    /// the sermon starts over rather than inheriting a stale agreement.
    #[test]
    fn a_stale_sighting_does_not_corroborate() {
        let mut r = Router::default();
        r.decide_live("John 3:16", 0.95, DIRECT, 0, false);
        let long_after = DEFAULT_DEBOUNCE_MS + 1;
        assert_eq!(
            r.decide_live("John 3:16", 0.95, DIRECT, long_after, false),
            RouteDecision::Suggest,
            "an hour-old sighting corroborated a fresh misread"
        );
    }

    /// The paraphrase cap is untouched by any of this — it is enforced before
    /// corroboration is even consulted, and no number of sightings lifts it.
    #[test]
    fn corroboration_never_promotes_a_paraphrase() {
        let mut r = Router::default();
        for t in [0, 250, 500, 750] {
            assert_eq!(
                r.decide_live("John 3:16", 1.0, DetectionMethod::Semantic, t, false),
                RouteDecision::Suggest,
                "a guess seen four times is still a guess"
            );
        }
    }
}

#[cfg(test)]
mod sensitivity_sweep {
    use super::*;

    /// What would each dial setting have done to a REAL sermon?
    ///
    /// ```text
    /// RELAY_SWEEP_TRANSCRIPT=/path/lines.txt RELAY_SWEEP_TRUTH="Romans 10:17" \
    ///   cargo test sensitivity_sweep -- --ignored --nocapture
    /// ```
    ///
    /// One line per transcript window, in order, exactly as the STT worker emitted
    /// them. Each is run through the real parser and the real gate at every dial
    /// setting, and the question asked is the only one that matters: **which verses
    /// would have reached the wall?** Not what the parser saw — what fired.
    ///
    /// The synthetic corpus in `eval.rs` cannot answer this. It is 50 lines chosen
    /// to contain references; a sermon is an hour of ordinary speech that mostly
    /// contains none, and the false-positive rate on ordinary speech is precisely
    /// what the dial trades against.
    #[test]
    #[ignore = "needs a transcript export"]
    fn what_each_dial_setting_would_have_fired() {
        let Some(path) = std::env::var_os("RELAY_SWEEP_TRANSCRIPT") else {
            eprintln!("set RELAY_SWEEP_TRANSCRIPT");
            return;
        };
        let truth = std::env::var("RELAY_SWEEP_TRUTH").unwrap_or_default();
        let lines: Vec<String> = std::fs::read_to_string(&path)
            .expect("read transcript")
            .lines()
            .map(|l| l.to_string())
            .filter(|l| !l.trim().is_empty())
            .collect();

        println!(
            "\n{} windows of real sermon speech; truth = {:?}\n",
            lines.len(),
            truth
        );
        println!("  dial  auto_fire  suggest   FIRED   of which wrong   truth fired?");
        println!("  ────  ─────────  ───────   ─────   ──────────────   ────────────");

        for dial in [0u8, 6, 25, 40, 50, 60, 75, 89, 100] {
            let t = Thresholds::from_sensitivity(dial);
            let mut r = Router::default();
            r.set_thresholds(t);
            r.set_baseline(t);

            let mut fired: Vec<String> = Vec::new();
            for (i, line) in lines.iter().enumerate() {
                // Each window is a separate utterance as far as this replay is
                // concerned, so it is scored as FINAL — the corroboration rule needs a
                // second decode of the same rolling window, which an exported
                // transcript no longer has. This is therefore the PESSIMISTIC reading:
                // it counts fires that the live path would make an extra pass to
                // confirm, so the wrong-fire counts here are an upper bound.
                let now_ms = i as u64 * 1000;
                for m in crate::detection::detect_direct(line) {
                    if m.is_provisional(true) {
                        continue;
                    }
                    let key = format!(
                        "{} {}:{}",
                        m.reference.book, m.reference.chapter, m.reference.verse
                    );
                    if r.decide_live(&key, m.confidence, m.method, now_ms, true)
                        == RouteDecision::AutoFire
                    {
                        fired.push(key);
                    }
                }
            }
            let wrong = fired.iter().filter(|k| **k != truth).count();
            let hit = fired.contains(&truth);
            println!(
                "  {dial:>4}  {:>9.3}  {:>7.3}   {:>5}   {:>14}   {}",
                t.auto_fire,
                t.suggest,
                fired.len(),
                wrong,
                if hit { "YES" } else { "no" }
            );
            if wrong > 0 {
                let mut names: Vec<&String> = fired.iter().filter(|k| **k != truth).collect();
                names.sort();
                names.dedup();
                println!("          wrong: {names:?}");
            }
        }
    }
}

#[cfg(test)]
mod gate_readout_tests {
    use super::*;

    /// THE OPERATOR'S INSTRUCTION, 2026-09-23: *"Suggest above should be 20 below
    /// the auto-fire above."*
    ///
    /// It used to follow a curve of its own — 0.70 at the cautious end, 0.35 at
    /// the default, 0.20 at the eager end — so the band between "offer it" and
    /// "put it up" narrowed from 20 points to 10 as the dial was pushed right.
    /// That is the wrong way round: the eager end is exactly where an operator
    /// most wants a wide band of things offered rather than fired.
    #[test]
    fn the_suggest_bar_sits_exactly_twenty_below_the_auto_fire_bar() {
        for s in 0..=100u8 {
            let t = Thresholds::from_sensitivity(s);
            assert!(
                (t.auto_fire - t.suggest - SUGGEST_BAND).abs() < 1e-5,
                "dial {s}: auto {:.3} suggest {:.3} — band {:.3}, wanted {SUGGEST_BAND}",
                t.auto_fire,
                t.suggest,
                t.auto_fire - t.suggest
            );
        }
    }

    /// THE OPERATOR'S INSTRUCTION, 2026-09-23: *"when the sensor is on Auto fire
    /// above 100, then it auto fires not when on 0."*
    ///
    /// The printed figure was the raw confidence bar, which runs the other way
    /// from the dial it is printed under: the dial's cautious end (0) showed
    /// `Auto-fire above 90%` and its eager end (100) showed `30%`. Read as a
    /// setting — which is how it reads, sitting under a slider — that says the
    /// machine is keenest at the position where it fires least.
    ///
    /// `readiness` was that, and it put the figures in an order the operator then
    /// objected to in their turn: *"suggestions should be lower by 20 if auto
    /// fire is on 100 so auto fire has the higher priority."* On a readiness
    /// scale a suggestion is the LARGER number, because it is the easier bar.
    ///
    /// Both complaints are about the same pair and only one framing satisfies
    /// both: print what each one NEEDS. Auto-fire needs more confidence than a
    /// suggestion — always, at every dial position, by exactly `SUGGEST_BAND` —
    /// so auto-fire is the larger figure and the word "needs" makes a smaller
    /// number obviously the easier bar rather than the keener setting. The dial
    /// is the control and keeps its own direction; these are what it produced.
    #[test]
    fn auto_fire_always_needs_more_than_a_suggestion_and_by_exactly_the_band() {
        for s in 0..=100u8 {
            let g = Thresholds::from_sensitivity(s).readiness();
            assert!(
                g.auto_fire > g.suggest,
                "dial {s}: a suggestion needs as much as an auto-fire ({} vs {})",
                g.suggest,
                g.auto_fire
            );
            assert_eq!(
                g.auto_fire as i16 - g.suggest as i16,
                20,
                "dial {s}: the band is not 20 points wide on the printed scale"
            );
        }
        // The two ends, named, so a change to the curve has to say so out loud.
        // The dial's EAGER end needs the least, which is what eager means.
        assert_eq!(Thresholds::from_sensitivity(0).readiness().auto_fire, 90);
        assert_eq!(Thresholds::from_sensitivity(100).readiness().auto_fire, 30);
    }

    /// AND THE FIGURES FALL AS THE DIAL RISES, deliberately.
    ///
    /// That is the half §117 was written about and it has NOT been reverted: a
    /// bar you must clear is lower when more gets through, and no labelling can
    /// change that. What §117 got wrong was trying to fix it by turning the
    /// number over, which put the two in an order that read as a ranking. The
    /// fix is the WORD — "needs" — not the arithmetic, and this test exists so
    /// nobody re-inverts the figures to make them rise with the slider again.
    #[test]
    fn what_each_needs_falls_as_the_dial_rises() {
        let mut prev_auto = 101i16;
        for s in 0..=100u8 {
            let g = Thresholds::from_sensitivity(s).readiness();
            assert!(
                g.auto_fire as i16 <= prev_auto,
                "dial {s}: what an auto-fire needs ROSE to {}",
                g.auto_fire
            );
            prev_auto = g.auto_fire as i16;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// FOLLOWING THE READER · THE GATE (DECISIONS §118)
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod reading_gate {
    use super::*;

    const READING: DetectionMethod = DetectionMethod::Reading;
    const QUOTED: DetectionMethod = DetectionMethod::Quoted;

    /// THE OPERATOR'S INSTRUCTION, 2026-09-23: *"follow the verse whenever a
    /// preacher is reading a bible verse, you dont need to wait or suggest it."*
    ///
    /// And at EVERY dial position, because the thing that qualified it is a run
    /// of words, not a score — so there is no number for a dial to erase. This
    /// mirrors `semantic_can_never_auto_fire` deliberately: the same sweep, the
    /// opposite answer, for the one method where the evidence is what was said.
    #[test]
    fn a_reading_reaches_the_wall_at_every_dial_position() {
        for s in 0..=100u8 {
            let mut r = Router::default();
            r.set_thresholds(Thresholds::from_sensitivity(s));
            assert_eq!(
                r.decide("John 3:16", 0.69, READING, 0),
                RouteDecision::AutoFire,
                "a reading was held back at sensitivity {s}"
            );
        }
    }

    /// AND THE CHURCH MAY TURN IT OFF. Default on, per the operator; off, a
    /// reading behaves exactly as `Quoted` always has.
    #[test]
    fn the_switch_puts_it_back_where_it_was() {
        assert!(
            Router::default().follows_the_reader(),
            "the default must be ON — the operator asked for it"
        );
        let mut r = Router::default();
        r.set_follow_the_reader(false);
        assert_eq!(
            r.decide("John 3:16", 0.69, READING, 0),
            RouteDecision::Suggest
        );
        // Still offered, never silently dropped: the operator asked for less
        // firing, not for less information.
        r.set_follow_the_reader(true);
        assert_eq!(
            r.decide("John 3:16", 0.69, READING, 5_000),
            RouteDecision::AutoFire
        );
    }

    /// EVERYTHING SHORT OF THE BAR IS UNTOUCHED. `Quoted` is still capped at
    /// `Suggest` at any score and any dial — the cap moved for one new method,
    /// not for the class.
    #[test]
    fn a_quotation_that_is_not_a_reading_still_cannot_fire() {
        for s in 0..=100u8 {
            let mut r = Router::default();
            r.set_thresholds(Thresholds::from_sensitivity(s));
            for conf in [0.51, 0.75, 0.95, 1.0] {
                assert_ne!(
                    r.decide("John 3:16", conf, QUOTED, 0),
                    RouteDecision::AutoFire,
                    "Quoted auto-fired at conf={conf} sensitivity={s}"
                );
            }
        }
    }

    /// RULE 28 STILL APPLIES, and this is the trap it was nearly lost to.
    ///
    /// `decide_live` held a reference back for one more decode pass by asking
    /// "would this fire on its score?" — `confidence >= auto_fire`. A reading is
    /// NOT gated on its score, so at a cautious dial that question answers no, the
    /// corroboration wait is skipped, and the thing that skipped it fires anyway
    /// one line later. Removing a safety wait to make a path faster is rule 34,
    /// and it would have happened silently.
    #[test]
    fn a_reading_from_a_partial_window_still_waits_for_a_second_pass() {
        let mut r = Router::default();
        r.set_thresholds(Thresholds::from_sensitivity(0)); // the most cautious dial
        assert_eq!(
            r.decide_live("John 3:16", 0.69, READING, 0, false),
            RouteDecision::Suggest,
            "a reading read once out of a partial window reached the wall"
        );
        assert_eq!(
            r.decide_live("John 3:16", 0.69, READING, 400, false),
            RouteDecision::AutoFire,
            "the corroborating pass did not fire it"
        );
        // A closed utterance has no next pass coming, so it fires on first sight —
        // unchanged.
        let mut r = Router::default();
        assert_eq!(
            r.decide_live("Romans 8:28", 0.69, READING, 0, true),
            RouteDecision::AutoFire
        );
    }

    /// A READING MUST NEVER TEACH THE AUTO-FIRE BAR.
    ///
    /// `dismiss_detection` carries no number: the router falls back to the score
    /// of whatever it last put on screen. If that was a reading, the fallback is
    /// `quoted_confidence(run)` — a word count — and `record_feedback` would push
    /// the bar that governs SPOKEN REFERENCES past it. The operator pulled a
    /// followed reading off the wall and the machine would have got deafer to
    /// every reference anybody says.
    #[test]
    fn dismissing_a_followed_reading_does_not_move_the_reference_gate() {
        let mut r = Router::default();
        let before = r.thresholds().auto_fire;
        assert_eq!(
            r.decide("John 3:16", 0.95, READING, 0),
            RouteDecision::AutoFire
        );
        r.record_feedback(false, None); // the operator clears it off the wall
        let after = r.thresholds().auto_fire;
        assert!(
            after <= before + 1e-6,
            "a reading taught the reference gate: {before} → {after}"
        );
        // A DIRECT fire in the same position still does teach it — proving the
        // guard is about the method and not about the feedback path being inert.
        let mut r = Router::default();
        let before = r.thresholds().auto_fire;
        assert_eq!(
            r.decide("John 3:16", 0.95, DetectionMethod::Direct, 0),
            RouteDecision::AutoFire
        );
        r.record_feedback(false, None);
        assert!(r.thresholds().auto_fire > before);
    }

    /// The debounce, the cooldown and the RG-178 two-books rule all still see a
    /// reading — it goes through the same door as everything else.
    #[test]
    fn a_reading_is_debounced_like_anything_else() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("John 3:16", 0.69, READING, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("John 3:16", 0.69, READING, 500),
            RouteDecision::Drop,
            "the same verse re-transcribed fired twice"
        );
    }
}

/// **WHAT IS ON THE WALL** — `last_wall`, the fact the passage guard rests on
/// (the passage guard, 2026-09-25).
///
/// The wall is TOLD, not inferred, and that is the whole design: `broadcast_with_clock`
/// calls `note_wall` for scripture and `forget_wall` for everything else, so this
/// module records what actually left the machine rather than what it decided. The
/// end-to-end half — that every door a verse reaches a screen by really does arrive
/// here — is in `e2e`, because only the running app has the door.
#[cfg(test)]
mod the_wall {
    use super::*;

    #[test]
    fn a_fresh_router_says_nothing_is_on_the_wall() {
        assert_eq!(Router::default().wall(), None);
    }

    /// **DECIDING IS NOT SHOWING**, and this is the test the first design failed.
    ///
    /// `decide` returns `AutoFire` per candidate; rule 29 then lets only rank 0 reach
    /// a wall. So a gate decision may NEVER by itself claim the screens — service 40
    /// of 2026-09-25 yielded `Jeremiah 6:16` and `6:17` from one sentence, both
    /// `AutoFire`, and only 6:16 was shown. See `note_wall`.
    #[test]
    fn a_gate_decision_alone_never_claims_the_wall() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("Jeremiah 6:16", 0.95, DetectionMethod::Direct, 1_000),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("Jeremiah 6:17", 0.88, DetectionMethod::Direct, 1_000),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.wall(),
            None,
            "the gate said yes twice and rule 29 shows one; neither is a screen"
        );
        // The broadcast is what says so.
        r.note_wall("Jeremiah 6:16");
        assert_eq!(r.wall(), Some("Jeremiah 6:16"));
    }

    /// **THE CLEAR IS THE RELEASE THAT MATTERS.** `forget_last_fire` runs on a clear
    /// and a blackout, and without this half a preacher who re-read the verse the
    /// operator had just cleared would find Relay silently declining to put it back —
    /// the exact defect that function was written for, in the guard's costume.
    #[test]
    fn clearing_the_screens_forgets_what_was_on_them() {
        let mut r = Router::default();
        r.note_wall("John 3:16");
        r.forget_last_fire();
        assert_eq!(r.wall(), None);
    }

    /// A song, a notice, a picture or a countdown takes the screens, so no verse is
    /// on them.
    #[test]
    fn content_that_is_not_scripture_forgets_the_verse() {
        let mut r = Router::default();
        r.decide("John 3:16", 0.95, DetectionMethod::Direct, 1_000);
        r.note_wall("John 3:16");
        r.forget_wall();
        assert_eq!(r.wall(), None);
        // …and it leaves the repeat cooldown alone, because a song does not change
        // what was recently HEARD. The same verse inside the cooldown still Drops.
        assert_eq!(
            r.decide("John 3:16", 0.95, DetectionMethod::Direct, 2_000),
            RouteDecision::Drop
        );
    }

    /// The wall is NOT the cooldown, and this is the measurement that forced the two
    /// apart: the field gaps between a reading and the verse already on the wall were
    /// 11, 17 and 120 seconds, and `DEFAULT_DEBOUNCE_MS` is 10. Anything with a clock
    /// in it answers "no verse is up" while the verse is plainly up.
    #[test]
    fn the_wall_has_no_clock_in_it() {
        let mut r = Router::default();
        r.note_wall("Jeremiah 6:16");
        const { assert!(DEFAULT_DEBOUNCE_MS < 120_000) };
        // Two minutes on, with the cooldown long expired, the wall still says the
        // same thing — because it is a fact and not a timer.
        r.decide("Jeremiah 6:16", 0.95, DetectionMethod::Direct, 121_000);
        assert_eq!(r.wall(), Some("Jeremiah 6:16"));
    }
}
