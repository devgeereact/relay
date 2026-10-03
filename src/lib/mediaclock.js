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

/**
 * When a clip becomes something the preacher has to act on.
 *
 * Thirty seconds, which is the smallest useful amount of time to change what you
 * are about to do: stand up, find the place, look at the operator. A warning
 * that arrives at five seconds is an announcement that the clip has ended.
 *
 * It is a named decision rather than a number inside a component because two
 * surfaces already show this figure, and a threshold typed twice is a threshold
 * that will one day differ between the stage and the desk.
 */
export const CLIP_WARN_MS = 30_000;

/** One screen's contribution, or `null` when it has nothing to contribute. */
function clipOf(row) {
  if (!row || row.painting !== true) return null;
  const m = row.media;
  if (!m) return null;
  const dur = Number(m.dur_ms);
  const pos = Number(m.pos_ms);
  if (!Number.isFinite(dur) || dur <= 0 || !Number.isFinite(pos) || pos < 0) return null;
  return {
    id: row.id,
    name: row.name || `Screen ${row.id}`,
    remaining: Math.max(0, Math.round(dur - Math.min(pos, dur))),
    paused: !!m.paused,
    // The two a SCRUB needs (RG-221). Carried on the same clip as everything
    // else here so the bar and the readout cannot end up describing two
    // different screens.
    duration: Math.round(dur),
    position: Math.max(0, Math.round(Math.min(pos, dur))),
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
export function describeMediaClock(rows, { mainId = null } = {}) {
  const clips = (Array.isArray(rows) ? rows : []).map(clipOf).filter(Boolean);
  if (!clips.length) {
    return {
      known: false,
      // NOT A DASH, and not "0:00". An operator reading a dash assumes the clip
      // has no clock; an operator reading words knows a screen is not answering
      // and goes and looks at it.
      text: 'No screen is reporting a clip',
      remainingMs: null,
      // ABSENT, NEVER ZERO. A zero-length scrub bar looks usable and can move
      // nothing, which is the defect DECISIONS §69 closed seven controls of.
      durationMs: null,
      positionMs: null,
      paused: false,
      from: null,
      screens: 0,
      disagree: false,
    };
  }
  // The shortest remaining: the operator is preparing for the moment the first
  // screen runs out, not for an average.
  // ── WHICH SCREEN THE FIGURE IS ABOUT (RG-238) ──────────────────────────────
  //
  // The shortest remaining is the right answer to *when does the first screen
  // run out*, and the wrong one to *how long is left of what I am watching* —
  // which is what a readout beside a Pause button is asked. The desk said
  // `0:25 left · from STAGE MONITOR` while the operator was looking at the main
  // screen's clip, because a monitor that started a moment earlier wins a
  // comparison on remaining time.
  //
  // So the MAIN screen answers when it has a clip, and the soonest answers when
  // it does not — which is still every screen a church has not given a role to,
  // so nothing changes for an install that has not opened Outputs.
  const soonest = clips.reduce((a, b) => (b.remaining < a.remaining ? b : a));
  const main = mainId == null ? null : (clips.find((c) => c.id === mainId) ?? null);
  const quoted = main ?? soonest;
  const spread =
    Math.max(...clips.map((c) => c.remaining)) - Math.min(...clips.map((c) => c.remaining));
  // PAUSED IS THE SCREENS' ANSWER, NOT A VOTE. If any screen says it is paused
  // then something an operator pressed has taken effect somewhere, and the
  // conservative reading is to say so rather than to let a majority hide it.
  const paused = clips.some((c) => c.paused);
  return {
    known: true,
    text: paused
      ? `${formatCountdown(quoted.remaining)} left · held`
      : `${formatCountdown(quoted.remaining)} left`,
    remainingMs: quoted.remaining,
    durationMs: quoted.duration,
    positionMs: quoted.position,
    paused,
    from: quoted.name,
    screens: clips.length,
    disagree: clips.length > 1 && spread > DRIFT_TOLERANCE_MS,
  };
}

/**
 * THE MEDIA ID INSIDE A URL THE ENGINE BUILT, or `null`.
 *
 * `main.rs::media_url` builds `http://<ip>:8032/media/<id>` for an imported asset
 * and something else entirely for one Relay SHIPS (DECISIONS §90) — a bundled
 * picture has no row under `/media/<id>` at all. So this reads the id back where
 * there is one and answers `null` where there is not, rather than guessing.
 *
 * It exists because a control that acts on the clip currently up needs the id, and
 * the id is not on the content frame: the frame carries the URL, because that is
 * what a screen needs. Reading it back here is the smaller of two evils — the
 * other being a second field on every content frame that only one button reads.
 *
 * `null` disables the control with a reason rather than sending a guess.
 */
export function mediaIdFromUrl(url) {
  const m = /\/media\/(\d+)(?:[/?#]|$)/.exec(String(url ?? ''));
  if (!m) return null;
  const n = Number(m[1]);
  return Number.isSafeInteger(n) && n > 0 ? n : null;
}

/**
 * HOW LONG IS LEFT OF THE CLIP IN FRONT OF THE PREACHER, in milliseconds.
 *
 * The stage half of requirement 11, and a DIFFERENT question from the one
 * `describeMediaClock` answers. That one is the operator's: it asks the
 * congregation screens, on their beat, because the console's own player is not
 * the thing anybody is watching. The stage page has no channel health — it is a
 * client like every other screen — and the clip it is timing is the one playing
 * in front of the person who needs the answer. So the source is the player, and
 * the two rules stay apart: collapsing them would leave one of the two surfaces
 * quoting a number that is not about the picture it is showing.
 *
 * **What this deliberately does NOT claim.** It is this screen's copy of the
 * file, not the congregation's. Two players of the same clip started at
 * separate instants drift by a second or so, which is `DRIFT_TOLERANCE_MS`'s
 * whole subject, and nothing here corrects for it. For the question being asked
 * — *is the picture I am standing in front of about to end* — this screen's own
 * copy is the honest source, and it is the only one this page can reach.
 *
 * `null` for every state a player can be in before it knows: no element, no
 * metadata, a zero duration, a live stream with no end. **Never a zero**, which
 * would read as "it has finished" about a clip that has not started.
 *
 * @param el a media element, or anything carrying `duration` and `currentTime`
 */
export function clipRemainingMs(el) {
  if (!el) return null;
  const duration = Number(el.duration);
  const at = Number(el.currentTime);
  if (!Number.isFinite(duration) || duration <= 0) return null;
  if (!Number.isFinite(at)) return null;
  return Math.max(0, Math.round((duration - at) * 1000));
}
