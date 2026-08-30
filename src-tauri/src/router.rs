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
    /// | 50   | 0.50      | 0.35    | **the default**        |
    /// | 100  | 0.30      | 0.20    | eager — many, noisy    |
    ///
    /// Note these gate `Direct` detections only — semantic/ambiguous candidates
    /// can never auto-fire at ANY dial position (see `Router::decide`).
    pub fn from_sensitivity(sensitivity: u8) -> Self {
        let s = (sensitivity.min(100) as f32) / 100.0;
        let (auto_fire, suggest) = if s <= 0.5 {
            let t = s * 2.0; // 0..1 across the cautious half
            (lerp(0.90, 0.50, t), lerp(0.70, 0.35, t))
        } else {
            let t = (s - 0.5) * 2.0; // 0..1 across the eager half
            (lerp(0.50, 0.30, t), lerp(0.35, 0.20, t))
        };
        Thresholds {
            auto_fire,
            suggest: suggest.min(auto_fire),
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
        if !corroborated && method.may_auto_fire() && confidence >= self.thresholds.auto_fire {
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

    pub fn decide(
        &mut self,
        key: &str,
        confidence: f32,
        method: DetectionMethod,
        now_ms: u64,
    ) -> RouteDecision {
        // Uncalibrated methods can reach the operator, never the screen.
        if !method.may_auto_fire() {
            return if confidence >= self.thresholds.suggest {
                RouteDecision::Suggest
            } else {
                RouteDecision::Drop
            };
        }
        if confidence >= self.thresholds.auto_fire {
            if let Some(t) = self.fired_at.get(key) {
                if now_ms.saturating_sub(*t) < self.debounce_ms {
                    // Already on screen, and said again within the cooldown —
                    // almost always the same utterance being re-transcribed.
                    return RouteDecision::Drop;
                }
            }
            self.note_fired(key, now_ms);
            // Remember WHAT we put on screen, so a later "undo" is a proportional
            // correction rather than a blind nudge.
            self.last_fire_conf = Some(confidence);
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
    pub fn forget_last_fire(&mut self) {
        self.fired_at.clear();
        self.last_fire_conf = None;
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

    #[test]
    fn gates_by_tier() {
        let mut r = Router::default(); // 0.50 / 0.35 (push above ~50%)
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
            r.decide("Psalms 23:1", 0.30, DIRECT, 200),
            RouteDecision::Drop
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
                for m in [DetectionMethod::Semantic, DetectionMethod::Ambiguous] {
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
        assert!((def.suggest - 0.35).abs() < 1e-4, "{}", def.suggest);
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
