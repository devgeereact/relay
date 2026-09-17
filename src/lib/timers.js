/**
 * The programme timer, on the operator's side: which ones to show, and how long
 * is left on one. Pure — no store, no bridge, no clock of its own.
 *
 * ## Why this is three small functions and not one component method
 *
 * `list_timers` hands back the REGISTRY's shape (`target_ms`, `paused_ms`,
 * `scope`) and the one piece of countdown arithmetic this side of the bridge is
 * allowed to do reads the CONTENT shape (`countdown_to`, `countdown_paused_ms`).
 * Something has to bridge the two, and the moment that lives inside a component
 * it is a second answer to "how long is left" — which is the defect
 * `docs/REBRAND.md` phase 7 records as fixed once already, and the reason
 * `countdown.js::countdownRemainingMs` is the only reader.
 *
 * So the bridging is one projection, here, tested, and it ENDS in that function.
 * It is the mirror of `timers::project_both` in Rust: one rule, stated once on
 * each side of the bridge and never twice on one.
 */
import { countdownRemainingMs } from './countdown.js';

/**
 * The PROGRAMME timers — the preacher's monitor — and nothing else.
 *
 * A congregation (`both`) timer belongs to the dock's Countdown block, which has
 * its own transport and its own figure. Listing it here as well would put two
 * Stops on one clock.
 *
 * Anything that is not a list answers empty rather than throwing: a reactive
 * block that throws takes the dock down with it, and the dock carries
 * `Clear screens`. Distinguishing a FAILED read from an empty one is the
 * caller's job and must not be done here — an empty list returned from a broken
 * bridge is exactly rule 35's failure, and this function cannot tell the
 * difference.
 */
export function stageTimers(list) {
  if (!Array.isArray(list)) return [];
  return list.filter((t) => t && t.scope === 'stage');
}

/** A registry timer in the shape `countdownRemainingMs` reads. The one projection. */
export function timerAsContent(t) {
  return {
    countdown_to: t?.target_ms ?? null,
    countdown_paused_ms: t?.paused_ms ?? null,
  };
}

/**
 * How long is left, in ms, or null when the timer names no deadline.
 *
 * Null rather than zero, deliberately: a row printing 0:00 over a timer whose
 * target never arrived reads exactly like one that has just run out.
 */
export function timerRemainingMs(t, nowMs) {
  if (!t) return null;
  return countdownRemainingMs(timerAsContent(t), nowMs);
}
