//! Content router: confidence gating, debounce, decides what fires where.
//!
//! Single responsibility: take Detections from detection.rs and decide
//! auto-fire / suggest / drop, apply the debounce window, and hand the final
//! "show this content" event to channels.rs. Owns the self-calibrating
//! threshold state per docs/DECISIONS.md — thresholds are configuration,
//! never hardcoded constants. See PROMPT.md Phase 6.
//!
//! TODO(phase 6): two-tier gating (seed: auto-fire >=0.90, suggest >=0.60),
//! ~4-6s debounce, per-install threshold nudging from operator confirm/reject
//! signal, manual override path (must always short-circuit everything above).

pub enum RouteDecision {
    AutoFire,
    Suggest,
    Drop,
}
