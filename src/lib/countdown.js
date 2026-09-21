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
  const held = heldMs(c);
  // A HELD FIGURE IS SIGNED, AND THE FLOOR IS THE CALLER'S (RG-175).
  //
  // This used to test `held > 0`, which made a hold past zero impossible to
  // express: a stage timer two minutes over answered its deadline instead of
  // its held figure, so the `held` row `Stage.svelte` has rendered since wave 4
  // could never be produced. DECISIONS §99 recorded that as structural; it was
  // this comparison.
  //
  // The audience guarantee is kept by the same clamp a RUNNING countdown gets,
  // rather than by the contract being positive: a wall reads zero as "it
  // finished" and paints the done message, and `-2:00` in front of a room is
  // not a thing anybody asked for. One rule, one opt-in, both readings honest.
  if (held !== null) return past ? held : Math.max(0, held);
  const to = Number(c.countdown_to);
  if (!Number.isFinite(to) || to <= 0) return null;
  const left = to - (Number(nowMs) || 0);
  return left > 0 || past ? left : 0;
}

/**
 * The held figure, or `null` when this timer is not held.
 *
 * **The null guard is explicit because it used to be accidental.** `Number(null)`
 * is `0`, and `null` is exactly what the wire sends for a timer that is NOT
 * held — so the old `held > 0` was rejecting it by arithmetic rather than by
 * intent. Widen that test to "is it finite" without this, and every unheld
 * countdown in the product freezes at `0:00`.
 */
function heldMs(c) {
  const raw = c?.countdown_paused_ms;
  if (raw === null || raw === undefined || raw === '') return null;
  const held = Number(raw);
  return Number.isFinite(held) ? held : null;
}

/**
 * "10:30" → the epoch instant of today at that local clock time, or null.
 *
 * **THE ONLY PLACE IN RELAY WHERE A CLOCK TIME BECOMES AN INSTANT**, and it is on
 * this side of the bridge for a reason that is not convenience: turning a wall
 * time into a moment needs the machine's timezone and its DST rules, `std` has
 * neither, and `Date` has both. `setHours` on a local `Date` is DST-correct by
 * construction — on the morning a clock goes forward, "10:30" is still 10:30 on
 * the wall, and the number of milliseconds to it is whatever it is.
 *
 * **A time that has already gone is returned as it is**, not rolled to tomorrow.
 * The timer then simply starts over, which is what an operator who typed a time
 * that has passed needs to see; 23:55:00 on a lobby screen would hide the typo
 * until the service had started. That is a product decision, recorded in
 * DECISIONS §102, and this function is where it is implemented rather than
 * where it is decided — callers do not get to re-decide it.
 *
 * @param {string} hhmm  a 24-hour clock time, `H:MM` or `HH:MM`
 * @param {number} now   the instant "today" is measured from
 * @returns {number|null} epoch ms, or null when that is not a clock time
 */
export function atClockTime(hhmm, now = Date.now()) {
  const m = /^(\d{1,2}):(\d{2})$/.exec(String(hhmm ?? '').trim());
  if (!m) return null;
  const h = Number(m[1]);
  const min = Number(m[2]);
  if (!Number.isFinite(h) || !Number.isFinite(min) || h > 23 || min > 59) return null;
  const d = new Date(Number(now) || Date.now());
  d.setHours(h, min, 0, 0);
  return d.getTime();
}

/** Is the countdown on this content being HELD? One reader, same reason as above. */
export function countdownIsPaused(content) {
  return heldMs(content || {}) !== null;
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

/**
 * ══ IS A COUNTDOWN WHAT IS ON THE SCREENS RIGHT NOW ═════════════════════════
 *
 * The frontend statement of `main::is_countdown_content` — the same three arms,
 * asked of the same slot. The engine reads `channels::live_content`; this reads
 * `$live`, which is that slot mirrored across the bridge by the output events.
 * One fact, stated once on each side and never twice on one, for the same reason
 * `remaining_ms` and `countdownRemainingMs` are.
 *
 * **It is deliberately NOT `countdownRemaining() !== null`.** That reader calls a
 * countdown that has run out null, correctly — ±1 cannot re-aim something with
 * nothing left — and a countdown at `0:00` is still on the wall. A way back that
 * asked the transport's question would offer to put a countdown back while the
 * congregation is looking at it, which is a control lying about what is on the
 * screens (rule 35).
 *
 * @param {object|null} content the live output content
 * @returns {boolean}
 */
export function isCountdownContent(content) {
  const c = content || {};
  return c.countdown_to != null || c.countdown_paused_ms != null || c.kind === 'countdown';
}

/**
 * ══ THE WAY BACK ONTO A CONGREGATION SCREEN (RG-152) ════════════════════════
 *
 * A timer outlives the content that replaced it, so after a reading there is
 * something to go back to; `showTimer` is how an operator goes back to it. This
 * decides whether that is a thing to offer, and about which timer — pure, because
 * the question is "what do these two facts mean", and a component deciding it is a
 * component that can decide it differently from the next one.
 *
 * FOUR ANSWERS, and the first is not the second:
 *
 *   `unknown`  the registry has not answered yet (or answered something that is
 *              not a list). NOT "there is nothing" — a caller that renders silence
 *              for both says the same thing over a working console and a broken
 *              bridge, which is rule 35's own failure. The caller holds the read's
 *              failure separately and says so.
 *   `none`     read, and there is no congregation timer. Nothing to offer.
 *   `showing`  there is one and a countdown is already on the screens. The way
 *              back is not a thing to offer for something you are already looking
 *              at, and this is read through `isCountdownContent`, the same fact
 *              `adjust_countdown` reads before it decides whether to repaint.
 *   `offered`  there is one and the screens are showing something else.
 *
 * A `Stage` timer is never a candidate: it has no congregation wire form and
 * `show_timer` refuses one in words. A control that has to be refused is a control
 * that should not have been offered.
 *
 * THE NEWEST `Both` TIMER, because that is what the engine means by "the
 * congregation countdown" (`main::newest_congregation_timer, deleted 2026-09-21 with the transport that was its only caller (DECISIONS §115)`). `list_timers` hands
 * them back oldest first. There is at most one — `start_countdown` stops the one
 * before it — and agreeing with the engine costs one line either way.
 *
 * It cannot create anything and names no action: it returns a timer and a word.
 *
 * @param {Array|null|undefined} timers what `list_timers` answered, or null when
 *                                      it has not been asked yet
 * @param {object|null} content the live output content
 * @returns {{ state: 'unknown'|'none'|'showing'|'offered', timer: object|null }}
 */
export function wayBack(timers, content) {
  if (!Array.isArray(timers)) return { state: 'unknown', timer: null };
  const both = timers.filter((t) => t && t.scope === 'both');
  if (!both.length) return { state: 'none', timer: null };
  const timer = both[both.length - 1];
  return { state: isCountdownContent(content) ? 'showing' : 'offered', timer };
}

/**
 * ══ HOW THE CONSOLE'S READOUT READS (docs/REBRAND.md §7) ════════════════════
 *
 * `auto` | `ms` | `hms`, the three `layers.js::formatCountdown` already takes. The
 * picker feeds THAT function; there is no second copy of the arithmetic here, and
 * there must never be one.
 *
 * **WHAT IT DOES NOT REACH, said plainly: the screens.** A wall's countdown is
 * rendered by `TemplateRender` from `OutputContent`, which carries no format
 * field, so this changes the notation of the CONSOLE'S readout of the same number
 * and nothing else. The control says so where an operator can read it (rule 35's
 * family) and the backend half is still owed.
 *
 * AT MODULE SCOPE FOR THE SAME REASON `countdownSet` IS. It lived in
 * `Dock.svelte`'s module script while the transport lived in the dock, and moved
 * here with the transport (2026-09-20): the run surface is `{#if tab === 'live'}`
 * in the workspace router, so changing workspace destroys it, and a component
 * `let` would silently drop an operator's choice mid-service. A store exported
 * from a COMPONENT was always the odd shape — the decision layer is this module,
 * and the preference is a decision.
 */
export const countdownFormat = writable('auto');

/**
 * ══ WHICH SCREENS A COUNTDOWN IS AIMED AT ═══════════════════════════════════
 *
 * `chosen` is the operator's answer and `rows` is what `list_output_channels`
 * said. `null` means every screen and is NOT the same as a list naming every
 * screen: "all of them" has one spelling, and an explicit list silently stops
 * including a screen opened a minute later. That distinction is `start_countdown`'s
 * own (`Timer::channels`), and this is the one place the console spells it.
 *
 * It answers with the ROWS rather than with ids, because the only caller needs
 * rows: `describeCountdownReach` resolves each screen's template to decide
 * whether that screen would show a clock at all, and handing it every screen in
 * the building over a countdown aimed at one of them is the status line rule 35
 * is about.
 *
 * An id the operator chose that no longer names a screen simply drops out. A
 * screen can be deleted in Outputs while this band is mounted, and a band that
 * reported a screen the engine has never heard of would be claiming a reach it
 * has not got.
 *
 * @param {number[]|null} chosen ids the operator ticked, or null for every screen
 * @param {Array} rows           channel rows as `list_output_channels` returns them
 * @returns {Array} the rows this countdown would be sent to
 */
export function aimedScreens(chosen, rows) {
  const list = Array.isArray(rows) ? rows : [];
  if (!Array.isArray(chosen)) return list;
  const ids = new Set(chosen.map((n) => Number(n)));
  return list.filter((c) => ids.has(Number(c?.id)));
}

/**
 * Tick or untick one screen, starting from "every screen" when nothing is set.
 *
 * The same rule `ServicePlanner.svelte::toggleCueChannel` keeps on a plan cue,
 * stated once here so the console band and the Planner cannot come to different
 * conclusions about what "all of them" means. Ticking the last one back on
 * returns to `null` rather than to a list of every id, for the reason above.
 *
 * Untickng the LAST screen answers `[]` — a real answer meaning "no screen" —
 * rather than quietly wrapping round to every screen. A control that did the
 * opposite of what it says at one end of its range is worse than one that
 * reaches a state the caller must then refuse, and refusing it is the caller's
 * job: `Start` is disabled and says why.
 *
 * @param {number[]|null} chosen the current answer
 * @param {number} id            the screen that was pressed
 * @param {Array} rows           every screen there is
 * @returns {number[]|null} the new answer
 */
export function toggleScreen(chosen, id, rows) {
  const all = (Array.isArray(rows) ? rows : []).map((c) => Number(c?.id));
  const from = Array.isArray(chosen) ? chosen.map(Number) : all;
  const target = Number(id);
  const next = from.includes(target) ? from.filter((n) => n !== target) : [...from, target];
  return all.length > 0 && next.length === all.length ? null : next;
}
