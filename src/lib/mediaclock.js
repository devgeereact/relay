/**
 * HOW LONG IS LEFT OF THE CLIP, and which screen said so.
 *
 * The second half of requirement 11: *"the countdown for the media so as to help
 * know when media is almost done or time remaining for preparation of the next
 * plan"*.
 *
 * ## Why this is a module and not four lines inside Live
 *
 * The same reason `outputHealth.js::describeScreen` is one: two surfaces will
 * eventually want to say this, and the moment the rule exists twice they can
 * disagree about the same clip. Pure, so it can be asserted against numbers
 * rather than by mounting a run surface that needs a backend.
 *
 * ## The rule, and the part of it that is a refusal
 *
 * **The figure only ever comes from a screen.** The console has its own player of
 * the same file in the programme pane, and timing the clip off that one would be
 * a number that keeps counting while the wall is frozen — rule 35, on the one
 * readout an operator times the next cue against.
 *
 * A screen contributes only if it is PAINTING and it reported a clip. Everything
 * else is `known: false`, which the surface must render as words rather than as a
 * dash: "not reporting" is a fact about the screens, and an operator who is told
 * it will go and look at one.
 *
 * **The shortest remaining wins**, because that is the question being asked. The
 * operator is preparing the next cue, so what matters is when the first screen
 * runs out, not the average of several.
 *
 * **And a disagreement is reported rather than smoothed.** Two screens playing
 * the same file drift — they are separate players that started at separate
 * instants — but a large spread is not drift, it is one screen stalled or
 * buffering, which is exactly what an operator needs to know before they cue
 * something over it.
 */

import { formatCountdown } from './layers.js';

/**
 * How far apart two screens may be before it stops being drift.
 *
 * Separate players of the same file are never in lockstep; a second or two is
 * ordinary. Beyond this a screen is stalled, buffering, or was started late, and
 * that is a fault rather than a rounding difference. Deliberately larger than the
 * beat interval, because two screens reporting on different ticks of the same
 * clock are already up to one interval apart through no fault of their own.
 */
export const DRIFT_TOLERANCE_MS = 4000;

/** One screen's contribution, or `null` when it has nothing to contribute. */
function clipOf(row) {
  if (!row || row.painting !== true) return null;
  const m = row.media;
  if (!m) return null;
  const dur = Number(m.dur_ms);
  const pos = Number(m.pos_ms);
  if (!Number.isFinite(dur) || dur <= 0 || !Number.isFinite(pos) || pos < 0) return null;
  return {
    name: row.name || `Screen ${row.id}`,
    remaining: Math.max(0, Math.round(dur - Math.min(pos, dur))),
    paused: !!m.paused,
  };
}

/**
 * What to say about the clip on the screens.
 *
 * @param {Array} rows channel status rows, exactly as `channel_status` returns them
 * @returns {{
 *   known: boolean, text: string, remainingMs: number|null,
 *   paused: boolean, from: string|null, screens: number, disagree: boolean,
 * }}
 */
export function describeMediaClock(rows) {
  const clips = (Array.isArray(rows) ? rows : []).map(clipOf).filter(Boolean);
  if (!clips.length) {
    return {
      known: false,
      // NOT A DASH, and not "0:00". An operator reading a dash assumes the clip
      // has no clock; an operator reading words knows a screen is not answering
      // and goes and looks at it.
      text: 'No screen is reporting a clip',
      remainingMs: null,
      paused: false,
      from: null,
      screens: 0,
      disagree: false,
    };
  }
  // The shortest remaining: the operator is preparing for the moment the first
  // screen runs out, not for an average.
  const soonest = clips.reduce((a, b) => (b.remaining < a.remaining ? b : a));
  const spread =
    Math.max(...clips.map((c) => c.remaining)) - Math.min(...clips.map((c) => c.remaining));
  // PAUSED IS THE SCREENS' ANSWER, NOT A VOTE. If any screen says it is paused
  // then something an operator pressed has taken effect somewhere, and the
  // conservative reading is to say so rather than to let a majority hide it.
  const paused = clips.some((c) => c.paused);
  return {
    known: true,
    text: paused
      ? `${formatCountdown(soonest.remaining)} left · held`
      : `${formatCountdown(soonest.remaining)} left`,
    remainingMs: soonest.remaining,
    paused,
    from: soonest.name,
    screens: clips.length,
    disagree: clips.length > 1 && spread > DRIFT_TOLERANCE_MS,
  };
}
