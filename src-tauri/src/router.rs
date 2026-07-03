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

use serde::{Deserialize, Serialize};

/// Two-tier gate thresholds. Seed defaults per docs/DECISIONS.md (placeholders
/// until tuned against a real corpus); nudged per install by operator feedback.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Thresholds {
    pub auto_fire: f32,
    pub suggest: f32,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            auto_fire: 0.90,
            suggest: 0.60,
        }
    }
}

impl Thresholds {
    /// Map a single operator "sensitivity" dial (0..=100) to the two-tier
    /// thresholds. Higher sensitivity → lower bars → the AI fires/suggests more
    /// readily (more catches, more noise). This sets the *baseline*; per-install
    /// feedback (`record_feedback`) then nudges from here. The mid dial position
    /// (50) reproduces the seed 0.90 / 0.60 defaults exactly.
    pub fn from_sensitivity(sensitivity: u8) -> Self {
        let s = (sensitivity.min(100) as f32) / 100.0;
        // auto_fire: 0.97 (cautious) → 0.83 (eager); suggest: 0.70 → 0.50.
        let auto_fire = 0.97 - 0.14 * s;
        let suggest = (0.70 - 0.20 * s).min(auto_fire);
        Thresholds { auto_fire, suggest }
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

/// Mid-point of the spec's 4–6s repeat cooldown.
const DEFAULT_DEBOUNCE_MS: u64 = 5_000;
/// Per-feedback threshold nudge; small so calibration is gradual.
const FEEDBACK_STEP: f32 = 0.01;

pub struct Router {
    thresholds: Thresholds,
    debounce_ms: u64,
    /// (reference key, when it last auto-fired) — for the repeat cooldown.
    last_fire: Option<(String, u64)>,
}

impl Default for Router {
    fn default() -> Self {
        Router {
            thresholds: Thresholds::default(),
            debounce_ms: DEFAULT_DEBOUNCE_MS,
            last_fire: None,
        }
    }
}

impl Router {
    /// Gate a candidate detection.
    ///
    /// `explicit` marks a strong direct-quote match (e.g. a colon-form
    /// reference) — per docs/SPEC.md it overrides the debounce instantly, so a
    /// deliberate re-reference isn't swallowed by the cooldown. `now_ms` is a
    /// monotonic timestamp (audio-position ms from the transcript).
    pub fn decide(
        &mut self,
        key: &str,
        confidence: f32,
        explicit: bool,
        now_ms: u64,
    ) -> RouteDecision {
        if confidence >= self.thresholds.auto_fire {
            if let Some((k, t)) = &self.last_fire {
                let within_cooldown = now_ms.saturating_sub(*t) < self.debounce_ms;
                if k == key && within_cooldown && !explicit {
                    return RouteDecision::Drop; // debounced repeat of the same verse
                }
            }
            self.last_fire = Some((key.to_string(), now_ms));
            RouteDecision::AutoFire
        } else if confidence >= self.thresholds.suggest {
            RouteDecision::Suggest
        } else {
            RouteDecision::Drop
        }
    }

    /// Operator manual override — always fires, bypassing thresholds and
    /// debounce entirely. This is a first-class control (CLAUDE.md), never a
    /// fallback: it must always win.
    pub fn manual_fire(&mut self, key: &str, now_ms: u64) -> RouteDecision {
        self.last_fire = Some((key.to_string(), now_ms));
        RouteDecision::AutoFire
    }

    /// Feed operator confirm/reject signal back into the thresholds (the
    /// self-calibrating mechanism). Confirming a suggestion loosens `suggest`;
    /// rejecting an auto-fire tightens `auto_fire`. Bounded, and the invariant
    /// `auto_fire >= suggest` is preserved.
    pub fn record_feedback(&mut self, confirmed: bool) {
        if confirmed {
            self.thresholds.suggest = (self.thresholds.suggest - FEEDBACK_STEP).clamp(0.40, 0.85);
        } else {
            self.thresholds.auto_fire =
                (self.thresholds.auto_fire + FEEDBACK_STEP).clamp(0.85, 0.99);
        }
        if self.thresholds.suggest > self.thresholds.auto_fire {
            self.thresholds.suggest = self.thresholds.auto_fire;
        }
    }

    pub fn thresholds(&self) -> Thresholds {
        self.thresholds
    }

    /// Manual override slider in Settings — always available (DECISIONS.md).
    pub fn set_thresholds(&mut self, t: Thresholds) {
        self.thresholds = Thresholds {
            auto_fire: t.auto_fire.clamp(0.0, 1.0),
            suggest: t.suggest.clamp(0.0, 1.0).min(t.auto_fire),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_by_tier() {
        let mut r = Router::default(); // 0.90 / 0.60
        assert_eq!(
            r.decide("John 3:16", 0.95, false, 0),
            RouteDecision::AutoFire
        );
        assert_eq!(
            r.decide("Romans 8:28", 0.70, false, 100),
            RouteDecision::Suggest
        );
        assert_eq!(
            r.decide("Psalms 23:1", 0.40, false, 200),
            RouteDecision::Drop
        );
    }

    #[test]
    fn debounces_same_verse_repeat() {
        let mut r = Router::default();
        assert_eq!(
            r.decide("John 3:16", 0.95, false, 0),
            RouteDecision::AutoFire
        );
        // same verse, 2s later, not explicit → dropped
        assert_eq!(
            r.decide("John 3:16", 0.95, false, 2_000),
            RouteDecision::Drop
        );
        // after the 5s window → fires again
        assert_eq!(
            r.decide("John 3:16", 0.95, false, 6_000),
            RouteDecision::AutoFire
        );
    }

    #[test]
    fn explicit_overrides_debounce() {
        let mut r = Router::default();
        r.decide("John 3:16", 0.95, false, 0);
        assert_eq!(
            r.decide("John 3:16", 0.98, true, 1_000),
            RouteDecision::AutoFire
        );
    }

    #[test]
    fn different_verse_not_debounced() {
        let mut r = Router::default();
        r.decide("John 3:16", 0.95, false, 0);
        assert_eq!(
            r.decide("Romans 8:28", 0.95, false, 1_000),
            RouteDecision::AutoFire
        );
    }

    #[test]
    fn manual_fire_always_wins() {
        let mut r = Router::default();
        r.decide("John 3:16", 0.95, false, 0);
        // immediate repeat via manual override still fires
        assert_eq!(r.manual_fire("John 3:16", 500), RouteDecision::AutoFire);
    }

    #[test]
    fn feedback_nudges_and_bounds() {
        let mut r = Router::default();
        let before = r.thresholds();
        r.record_feedback(false); // reject auto → tighten auto_fire
        assert!(r.thresholds().auto_fire > before.auto_fire);
        r.record_feedback(true); // confirm suggestion → loosen suggest
        assert!(r.thresholds().suggest < before.suggest);

        // Bounds hold under repeated feedback.
        for _ in 0..100 {
            r.record_feedback(false);
            r.record_feedback(true);
        }
        assert!(r.thresholds().auto_fire <= 0.99);
        assert!(r.thresholds().suggest >= 0.40);
        assert!(r.thresholds().suggest <= r.thresholds().auto_fire);
    }

    #[test]
    fn sensitivity_maps_and_stays_ordered() {
        // Mid dial reproduces the seed defaults exactly.
        let mid = Thresholds::from_sensitivity(50);
        assert!((mid.auto_fire - 0.90).abs() < 1e-4);
        assert!((mid.suggest - 0.60).abs() < 1e-4);
        // Higher sensitivity lowers both bars; lower raises them.
        let hi = Thresholds::from_sensitivity(100);
        let lo = Thresholds::from_sensitivity(0);
        assert!(hi.auto_fire < mid.auto_fire && mid.auto_fire < lo.auto_fire);
        assert!(hi.suggest < mid.suggest && mid.suggest < lo.suggest);
        // Invariant holds across the whole range.
        for s in 0..=100u8 {
            let t = Thresholds::from_sensitivity(s);
            assert!(t.suggest <= t.auto_fire, "s={s}");
        }
        // Clamps above 100.
        assert_eq!(
            Thresholds::from_sensitivity(200).auto_fire,
            Thresholds::from_sensitivity(100).auto_fire
        );
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
