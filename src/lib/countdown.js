// THE COUNTDOWN TRANSPORT — what Start, Reset, ±1 and Clear each ASK FOR.
//
// `docs/REBRAND.md` §7: "One timer, one formatter, read by the slide, the stage
// rail and the transport so they cannot drift … Transport: Start/Pause · Reset ·
// ±1 · Clear — and Clear resets it without removing the tool."
//
// The formatter already exists and is already shared (`layers.js::formatCountdown`,
// read by `TemplateRender` and by the stage page). What did not exist was the
// transport. This module is its whole decision layer, pure, so the arithmetic that
// decides what a congregation's screen says can be tested without a screen.
//
// ── WHY THE TOOL'S SETTING IS A STORE AND NOT A COMPONENT VARIABLE ───────────
//
// The dock is `{#if !liveFullscreen}<Dock />{/if}` in the app shell, so pressing
// "Full screen" DESTROYS it and pressing it again builds a new one. Anything the
// operator had typed into a component-local `let` is gone, silently, at the one
// moment they are least able to notice — mid-service, on the run surface. The set
// duration lives here instead, at module scope, and survives every unmount.
//
// ── WHAT THIS MODULE DELIBERATELY DOES NOT DO ────────────────────────────────
//
//   · It never calls a backend. `countdownPress` returns a DESCRIPTION of what
//     was asked for; the caller performs it and reports its own failure.
//   · It has no notion of success. Nothing here may say a screen changed.
//   · It does not format. There is exactly one countdown formatter and it is in
//     `layers.js` — a second one is how the wall and the stage came to disagree.

import { writable } from 'svelte/store';

/** What a fresh tool is set to, and what `Clear` returns it to. */
export const DEFAULT_COUNTDOWN_MS = 5 * 60_000;

/** One press of ±. A minute, because that is what a countdown is measured in. */
export const COUNTDOWN_STEP_MS = 60_000;

/**
 * The longest countdown the tool will set. A pre-service countdown is minutes;
 * this exists so a mistyped hour field cannot put a nine-hour timer on the wall.
 */
export const MAX_COUNTDOWN_MS = 12 * 3600_000;

/**
 * THE SMALLEST COUNTDOWN THAT MAY BE BROADCAST, and it is not zero.
 *
 * `start_countdown` in Rust reads `minutes` and falls back to **five minutes**
 * when it is not finite or not greater than zero. So asking for a countdown of 0
 * does not put 0:00 on the wall — it puts 5:00 there, silently, which is the
 * opposite of what the operator pressed. Nothing below this leaves this module.
 */
export const MIN_BROADCAST_MS = 1000;

/**
 * ══ THE ONE READER OF HOW LONG IS LEFT ══════════════════════════════════════
 *
 * `docs/REBRAND.md` §7 asks for one timer read by the slide, the stage rail and the
 * transport. The FORMATTER has been shared since phase 7 (`layers.js`), and the
 * arithmetic in front of it was not: the wall, the stage page and the console each
 * did their own `countdown_to - now`. Three copies of one subtraction is survivable.
 * Three copies of a subtraction that now has an EXCEPTION is not — a held countdown
 * whose exception one of the three has never heard of goes on counting down on that
 * surface while the other two hold, and one of the three is the congregation's.
 *
 * So this is the exception, once:
 *
 *   · `countdown_paused_ms` set  → that figure, frozen. The instant is ignored.
 *   · otherwise                  → the gap to `countdown_to`, floored at zero.
 *   · no countdown at all        → **null**, which is not the same as zero. Zero
 *                                  means "it finished" and shows the done message;
 *                                  null means there is nothing here to show.
 *
 * ══ AND THE OTHER SIDE OF ZERO, WHICH IS THE SAME SUBTRACTION ═════════════
 *
 * `past: true` lifts the floor and answers the NEGATIVE once the instant has gone
 * by. It exists for one caller: the stage page's programme rail, where a timer that
 * has run out counts upward so a preacher can see how far over they are (RG-153,
 * the operator's decision of 2026-09-17). `+4:37` on that rail is this figure
 * negated and formatted, and nothing else.
 *
 * It is an OPT-IN rather than the default because zero is a real answer everywhere
 * else: the wall reads zero as "it finished" and paints the done message over the
 * digits, and a wall handed a negative would never reach that branch. And it is an
 * option on this function rather than a second exported helper because a second
 * helper is a second subtraction, which is the defect `docs/REBRAND.md` phase 7
 * records as fixed once already.
 *
 * The HOLD exception still comes first, and must: a countdown paused at 4:00 whose
 * instant went by six minutes ago is stopped at four minutes, not ten minutes over.
 *
 * @param {object|null} content the live output content
 * @param {number} nowMs        the caller's tick, so a renderer's clock and this
 *                              cannot disagree about "now"
 * @param {{past?: boolean}} opts `past` answers the negative past the instant
 *                              instead of flooring at zero
 * @returns {number|null} ms left, 0 when finished (negative with `past`), null when
 *                        there is no countdown
 */
export function countdownRemainingMs(content, nowMs = Date.now(), { past = false } = {}) {
  const c = content || {};
  const held = Number(c.countdown_paused_ms);
  if (Number.isFinite(held) && held > 0) return held;
  const to = Number(c.countdown_to);
  if (!Number.isFinite(to) || to <= 0) return null;
  const left = to - (Number(nowMs) || 0);
  return left > 0 || past ? left : 0;
}

/** Is the countdown on this content being HELD? One reader, same reason as above. */
export function countdownIsPaused(content) {
  const held = Number(content?.countdown_paused_ms);
  return Number.isFinite(held) && held > 0;
}

/**
 * The length this countdown was AIMED for, in ms, or null when nothing said.
 *
 * `countdown_to - countdown_from`. It is the only input the warning rule has for
 * its short-countdown case (`layers.js::countdownWarning` — the last minute, or the
 * last tenth of a countdown shorter than ten minutes), and until `countdown_from`
 * was written by `start_countdown` the answer was always null, so that half of §7's
 * rule had never once fired in the product.
 *
 * Null rather than a guess when either end is missing: a made-up span would put the
 * warning colour on at the wrong moment, and a colour that is on at the wrong moment
 * is worse than one that is on a minute early.
 */
export function countdownTotalMs(content) {
  const to = Number(content?.countdown_to);
  const from = Number(content?.countdown_from);
  if (!Number.isFinite(to) || !Number.isFinite(from) || !from) return null;
  const span = to - from;
  return span > 0 ? span : null;
}

/**
 * The duration the countdown tool is SET to — the hh:mm:ss fields.
 *
 * Not "what is on the wall": that is the live content's `countdown_to`, and the
 * two are deliberately different facts. A countdown running on the screens is
 * counting down; this is what the next Start (or Reset) would ask for.
 */
export const countdownSet = writable(DEFAULT_COUNTDOWN_MS);

const clampSet = (ms) => Math.max(0, Math.min(MAX_COUNTDOWN_MS, Math.round(Number(ms) || 0)));

/** hh : mm : ss → ms. Anything unreadable counts as zero rather than as NaN. */
export function msFromFields(h, m, s) {
  const n = (v) => {
    const x = Math.floor(Number(v));
    return Number.isFinite(x) && x > 0 ? x : 0;
  };
  return clampSet((n(h) * 3600 + n(m) * 60 + n(s)) * 1000);
}

/** ms → the three fields, each already padded-able by the caller. */
export function fieldsFromMs(ms) {
  const total = Math.max(0, Math.floor((Number(ms) || 0) / 1000));
  return {
    h: Math.floor(total / 3600),
    m: Math.floor((total % 3600) / 60),
    s: total % 60,
  };
}

/**
 * WHAT ONE PRESS OF THE TRANSPORT ASKS FOR.
 *
 * @param {'start'|'reset'|'plus'|'minus'|'pause'|'resume'|'clear'} action
 * @param {number} setMs        what the tool is set to
 * @param {number|null} runningMs  what is left on the WALL, or null when no
 *                                 countdown is on it. While one is HELD this is the
 *                                 held figure — it is still on the wall, it is just
 *                                 not moving.
 * @param {boolean} paused      is the countdown on the wall being held right now
 * @returns {{ setMs: number, broadcastMs: number|null, pause: boolean|null, refused: string|null }}
 *   `setMs`       what the tool should now be set to
 *   `broadcastMs` what to put on the screens, or **null for "touch no screen"**
 *   `pause`       whether to hold it (`true`), release it (`false`), or **null for
 *                 "leave the hold exactly as it is"** — which is what every press
 *                 except Pause and Resume means. `+1` on a held countdown moves the
 *                 number and must not start it running.
 *   `refused`     why nothing happened, in words an operator can read, or null
 *
 * The distinction between `Reset` and `Clear` is the one §7 leaves implicit and
 * this module makes explicit, because two buttons that both "reset" something is
 * how an operator presses the wrong one:
 *
 *   Reset  puts the RUNNING countdown back to its full length. It is about the
 *          wall, and it does nothing when there is no countdown on the wall.
 *   Clear  returns the TOOL to its default length and touches no screen at all.
 *          The tool stays in Quick tools — §7's one emphasis — and, critically,
 *          it is NOT "Clear screens". A control called Clear, one row above the
 *          red panic button, must not be able to blank a congregation's screen.
 */
export function countdownPress(action, setMs, runningMs = null, paused = false) {
  const set = clampSet(setMs);
  const running = Number.isFinite(Number(runningMs)) && Number(runningMs) > 0 ? Number(runningMs) : null;
  const held = running !== null && !!paused;
  const keep = { setMs: set, broadcastMs: null, pause: null, refused: null };

  switch (action) {
    case 'start':
      if (running !== null)
        return { ...keep, refused: 'A countdown is already running. Reset it, or clear the screens.' };
      if (set < MIN_BROADCAST_MS)
        return { ...keep, refused: 'Set a length first — a countdown of zero is not a countdown.' };
      return { ...keep, broadcastMs: set };

    case 'reset':
      // Back to full length, on the wall. With nothing running there is nothing
      // to put back, and saying so is better than quietly starting one: Start is
      // the control that puts a countdown in front of people, and Reset must not
      // become a second one.
      if (running === null) return { ...keep, refused: 'Nothing is counting down.' };
      if (set < MIN_BROADCAST_MS) return { ...keep, refused: 'Set a length first.' };
      return { ...keep, broadcastMs: set };

    case 'plus':
      return running === null
        ? { ...keep, setMs: clampSet(set + COUNTDOWN_STEP_MS) }
        : { ...keep, broadcastMs: Math.min(MAX_COUNTDOWN_MS, running + COUNTDOWN_STEP_MS) };

    case 'minus': {
      if (running === null) return { ...keep, setMs: clampSet(Math.max(0, set - COUNTDOWN_STEP_MS)) };
      const next = running - COUNTDOWN_STEP_MS;
      // Below a second the backend would substitute five minutes (see
      // MIN_BROADCAST_MS). Refusing is the honest answer; the operator who wants
      // the countdown gone has Clear screens, which is a panic control and is not
      // this button.
      if (next < MIN_BROADCAST_MS)
        return { ...keep, refused: 'Less than a minute left — take it off the screen instead.' };
      return { ...keep, broadcastMs: next };
    }

    // ── HOLD AND RELEASE ────────────────────────────────────────────────────
    //
    // The one press in this transport that is not a re-aim. `countdown_to` is an
    // absolute instant and ±1 simply moves it; "stopped" is not an instant at all,
    // which is the honest reason Pause was the last of §7's five controls to exist.
    // Both of these ask the engine to set `countdown_paused_ms` and neither changes
    // the number — a held countdown is held at exactly what it said.
    case 'pause':
      if (running === null) return { ...keep, refused: 'Nothing is counting down.' };
      if (held) return { ...keep, refused: 'It is already paused.' };
      return { ...keep, pause: true };

    case 'resume':
      if (running === null) return { ...keep, refused: 'Nothing is counting down.' };
      if (!held) return { ...keep, refused: 'It is already counting.' };
      return { ...keep, pause: false };

    case 'clear':
      return { setMs: DEFAULT_COUNTDOWN_MS, broadcastMs: null, pause: null, refused: null };

    default:
      return { ...keep, refused: null };
  }
}

/**
 * Is a press of `action` available right now?
 *
 * Derived from `countdownPress` rather than restated, so a disabled button and
 * the refusal it would have given can never disagree — the failure mode a
 * disabled control that does not say why always has.
 */
export function countdownCan(action, setMs, runningMs = null, paused = false) {
  const r = countdownPress(action, setMs, runningMs, paused);
  if (r.refused) return false;
  // Pause and Resume reach a screen without changing the number, so "is there
  // something to broadcast" is the wrong question for them — `pause` is their
  // instruction and it is never null once the press was not refused.
  if (action === 'pause' || action === 'resume') return r.pause !== null;
  return action === 'clear' || action === 'plus' || action === 'minus'
    ? true
    : r.broadcastMs !== null;
}
