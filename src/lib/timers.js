/**
 * The Stage Timer, on the operator's side: which ones to show, and how long
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
import { countdownRemainingMs, countdownIsPaused, countdownTotalMs } from './countdown.js';
// The FORMATTER and the warning rule, not a second copy of either: `layers.js`
// owns both and `Stage.svelte` already read them from there. A rail that spelled
// its own `MM:SS` would be the surface that disagrees with the wall about the
// same figure (docs/REBRAND.md §7).
import { formatCountdown, countdownWarning } from './layers.js';

/**
 * The STAGE TIMERS — the preacher's monitor — and nothing else.
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

/**
 * A REGISTRY TIMER IN THE SHAPE A RENDERER READS (RG-222).
 *
 * The same facts come in two spellings. The registry — what `list_timers` hands
 * the console — calls them `target_ms`, `from_ms`, `paused_ms`, `done_msg`;
 * `channels::timer_frame_json` renames them on the way to a screen, so
 * `TemplateRender` and `programmeRows` only ever see `countdown_to`,
 * `countdown_from`, `countdown_paused_ms`, `countdown_done`.
 *
 * Live reads the registry and previews through the renderer, so it has to cross
 * that seam. It crosses here rather than in an object literal inside a view,
 * because two copies of a rename is exactly how a field stops arriving on one
 * surface and nobody notices for a fortnight — this repository has the scar
 * under four different names.
 *
 * `warn_ms` and `id` are NOT renamed by the wire and are not renamed here.
 */
export function timerAsRow(t) {
  return {
    id: t?.id,
    label: t?.label,
    countdown_to: t?.target_ms ?? null,
    countdown_from: t?.from_ms ?? null,
    countdown_paused_ms: t?.paused_ms ?? null,
    countdown_done: t?.done_msg ?? null,
    warn_ms: t?.warn_ms ?? null,
  };
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
 *
 * ── AND PAST ZERO, `past` ANSWERS THE NEGATIVE (DECISIONS §99) ──────────────
 *
 * The same opt-in `countdownRemainingMs` already carries, forwarded rather than
 * re-decided, so this stays one subtraction with a floor that one caller lifts.
 * `timerAsContent` is still the only projection between the registry shape and
 * the content shape, and this function is still where it ends.
 *
 * **It is an opt-in and not the default, and the two callers are the reason.**
 * Live's Stage Timer band mirrors the preacher's rail, and the rail counts up
 * (`Stage.svelte`, RG-153, DECISIONS §92) — a band that floored at zero told the
 * one person who can signal the preacher that a clock twelve minutes over had
 * just run out. The dock's way-back figure is a CONGREGATION countdown, where
 * zero is a real answer: the wall reads zero as "it finished" and paints the done
 * message, and a negative would never reach that branch. One rule, two honest
 * readings, and the caller says which it is asking for.
 */
/**
 * Is this registry timer being HELD?
 *
 * Through `timerAsContent`, so it is the same projection and the same reader
 * every other figure on this path goes through — `countdownIsPaused` is the one
 * answer to this question and a second `paused_ms != null` here would be a
 * surface that disagrees with the rail about whether a clock is frozen.
 *
 * A held figure is SIGNED as of RG-175: a sermon two minutes over holds at
 * `-120000`, which is `+2:00` on the rail. Anything testing `> 0` for held-ness
 * is the bug that made the hold impossible to express.
 */
export function timerIsHeld(t) {
  return countdownIsPaused(timerAsContent(t));
}

export function timerRemainingMs(t, nowMs, { past = false } = {}) {
  if (!t) return null;
  return countdownRemainingMs(timerAsContent(t), nowMs, { past });
}

// ── THE PROGRAMME SET, DERIVED ONCE (requirement 2b) ─────────────────────────
//
// Everything above answers about ONE timer. The four functions below answer
// about the SET — which rows there are, how wide their column has to be, how
// many of them will fit, and what the rail says about the ones it could not
// show. They were `Stage.svelte`'s reactive block and nothing else's, which was
// correct for exactly as long as one surface rendered a rail.
//
// Two now do. `output.html` with a `stage`-role channel and a template carrying
// a `programme` layer is the other supported route to a preacher's screen
// (DECISIONS §89), and the alternative to this module was a second copy of the
// arithmetic inside `TemplateRender` — which is how the two surfaces would come
// to disagree about the same clock, in the same room, in front of the one person
// who cannot look away from either. `outputHealth.js::describeScreen` serving
// Live and Outputs from one pure helper is the precedent.
//
// ── THE SHAPE THESE READ IS THE WIRE'S, NOT THE REGISTRY'S ───────────────────
//
// `stageTimers` and `timerRemainingMs` above take a REGISTRY row (`target_ms`,
// `paused_ms`, `scope`) and project it with `timerAsContent`. These take a row
// as `channels::timer_frame_json` puts it on the wire, which is already in the
// content shape (`countdown_to`, `countdown_paused_ms`) — deliberately, and the
// Rust side says why at the function: naming those fields anything else would
// mean a stage page doing its own subtraction. So there is no projection here
// and there must not be one; `countdownRemainingMs` is still the one reader.

/**
 * THE ROWS A PROGRAMME RAIL DRAWS, in the order they were given.
 *
 * A timer that names no deadline is not a row: null rather than zero, for the
 * reason `timerRemainingMs` states — a row printing 0:00 over a timer whose
 * target never arrived reads exactly like one that has just run out.
 *
 * `v` is the RENDERED string and carries its own sign, because past zero the
 * rail answers how far over (RG-153) and `+4:37` is one character wider than the
 * `0:00` it replaces. `programmeCh` budgets the column from this string for
 * exactly that reason.
 *
 * `warn` is never true for a HELD row. A held timer is not running out, it is
 * where the operator left it, and a frozen figure pulsing red says the opposite
 * of what is true.
 *
 * @param {Array} timers rows as the `timer` hub frame carries them
 * @param {number} nowMs
 */
/**
 * THE DECLARED FIGURE SIZE, CONVERTED INTO THE RAIL'S OWN CONTAINER.
 *
 * `L.size` is a share of the SLIDE's width, because that is what `cqw` means for
 * every other text layer in the product. `.lprog` declares
 * `container-type: inline-size`, so a `cqw` written inside it is a share of the
 * RAIL. Those are different numbers whenever the rail is not full width, and the
 * difference runs the wrong way: a rail 40% of the screen wide would render
 * `6cqw` at 2.4% of the screen — SMALLER, on the control an operator reached for
 * to make it bigger.
 *
 * rail width = w% of screen, so 1cqw(rail) = w/100 cqw(screen), so a figure of
 * `size` screen-units is `size * 100 / w` rail-units.
 *
 * A missing, zero or nonsense width falls back to full width rather than
 * dividing by zero: the wrong size is a bad look, and `NaN` is a blank rail on a
 * preacher's monitor.
 */
export function railSize(layer) {
  const size = Number(layer?.size);
  const w = Number(layer?.w);
  const base = Number.isFinite(size) && size > 0 ? size : 2.2;
  const width = Number.isFinite(w) && w > 0 ? w : 100;
  return (base * 100) / width;
}

/**
 * WHAT A TIMER NOBODY NAMED IS CALLED (operator, 2026-09-23; RG-287).
 *
 * *"Defult timer text should be TIMER not just empty."*
 *
 * It is applied HERE and nowhere else, because this function is the one place a
 * rail's rows are made — the preacher's phone and the stage TV both read it —
 * and a default written at a render site is a default the other surface does not
 * have. That has been this repository's recurring bug under four different
 * names.
 *
 * **It reverses a rule that was deliberate, so it says so.** A label-less timer
 * used to render its digits alone on purpose: an empty label must give up its
 * room rather than take it, and `timers.test.js` held that. The room is still
 * given up where it matters — `programmeRoom` stands the whole head down below
 * `BARE_BELOW_PX`, which is the case that rule was protecting — and what changes
 * is that a rail with room for a name now prints one instead of leaving a
 * preacher to work out which clock he is looking at.
 */
export const TIMER_LABEL = 'TIMER';

/**
 * THE WORD FOR A CLOCK THAT HAS RUN OUT (operator, 2026-09-23; RG-286).
 *
 * *"when the time is running over, change the text to Time Up to preacher can
 * see whiles the time run over"*.
 *
 * It goes in the STATE slot, beside the label, where `Held` already goes — not
 * over the label. Two reasons, and the second is the one that decided it:
 *
 *  - the rail already has a place for "what this clock is doing", it is drawn at
 *    full strength while the label sits at 0.62, and a preacher glancing up gets
 *    the state before the name either way;
 *  - replacing the name would leave a platform running three clocks unable to
 *    tell WHICH one had run out, and the figure beside it is already `+13:31` —
 *    the sign says over, the word says over, and the name still says which.
 *
 * It follows `over` exactly, so a HELD clock never says it: a figure somebody
 * chose to freeze past zero (RG-175) is not an alarm, and the rail must not say
 * two things about one clock.
 */
export const OVER_WORD = 'TIME UP';

export function programmeRows(timers, nowMs) {
  const list = Array.isArray(timers) ? timers : [];
  return list
    .map((t) => ({
      t,
      id: t?.id,
      label: (t?.label || '').trim() || TIMER_LABEL,
      held: countdownIsPaused(t),
      ms: countdownRemainingMs(t, nowMs, { past: true }),
    }))
    .filter((r) => r.ms != null)
    .map(({ t, ...r }) => ({
      ...r,
      v: r.ms <= 0 ? `+${formatCountdown(-r.ms)}` : formatCountdown(r.ms),
      warn:
        !r.held && (r.ms <= 0 || countdownWarning(r.ms, countdownTotalMs(t), t?.warn_ms)),
      // THE TIME HAS ACTUALLY GONE — a narrower fact than `warn`, and the rail
      // flashes on this one (operator, 2026-09-21).
      //
      // `warn` covers the whole warning window, which an operator sets and which
      // is routinely five minutes. A clock that flashes for five minutes is one
      // somebody stops seeing, and then the flash says nothing at the moment it
      // matters. So the steady red keeps the window and this marks the boundary:
      // time up, and every second after it.
      //
      // NOT WHILE HELD, the same exemption `warn` already keeps. A sermon held
      // two minutes over is a figure somebody chose to freeze (RG-175), and an
      // alarm about a deliberate act is an alarm nobody can act on.
      over: !r.held && r.ms <= 0,
      // THE WORD IN THE STATE SLOT — one field, so neither rail spells its own.
      // `Stage.svelte` hard-coded `Held` and `TemplateRender` copied it; the
      // moment there is a second state word that is two rails with an opinion
      // about the same clock.
      state: countdownIsPaused(t) ? 'Held' : r.ms <= 0 ? OVER_WORD : '',
    }));
}

/**
 * The column budget in characters — the widest RENDERED figure on the rail,
 * floored at four, the width of `0:00`.
 *
 * One size for every row, so the figures stay one size and the longest of them
 * still cannot be clipped. A flat six was what this was for as long as the rail
 * existed, and `formatCountdown` emits seven past an hour: `1:30:13` painted
 * 215.3px into a 199px `overflow: hidden` box and read as `1:30:1` (RG-147).
 * What is left of a clipped clock reads as a valid time, which is the part that
 * matters — a preacher glancing down cannot tell it from a correct one.
 */
export function programmeCh(rows) {
  const list = Array.isArray(rows) ? rows : [];
  return list.reduce((n, r) => Math.max(n, String(r?.v ?? '').length), 4);
}

/**
 * THE RAIL'S FLOOR — the width below which a cell cannot be read from a platform.
 *
 * 132px is arithmetic, not taste: a cell's figure is a share of the rail divided
 * by the cell count, so 132px puts `MM:SS` at about 32px, a little above the
 * reading's own 26px floor. Below that the digits are inside the box and nobody
 * across a platform can read them.
 */
export const MIN_TIMER_PX = 132;

/**
 * How many cells fit in `px`.
 *
 * AN UNMEASURED BOX IS NOT A BOX OF ZERO. A width of 0 is what an element
 * reports before it has been laid out, and answering "one cell" to that would
 * collapse the rail on its first frame and then quietly leave it collapsed on
 * any surface that never re-measures. 1024 is the same assumption
 * `Stage.svelte` has always started from, said once, here.
 */
export function programmeCapacity(px, minPx = MIN_TIMER_PX) {
  const w = Number(px);
  const min = Number(minPx) > 0 ? Number(minPx) : MIN_TIMER_PX;
  return Math.max(1, Math.floor((Number.isFinite(w) && w > 0 ? w : 1024) / min));
}

/**
 * The cells to draw, with the count of the ones that did not fit.
 *
 * The last slot is spent on the count when there is one, so the count cannot
 * itself be the thing that gets pushed off the end — and at least one clock
 * always survives, because a rail that says "6 more" and shows nothing has told
 * the preacher he cannot have the thing he is looking at.
 *
 * A rail that cannot show every timer must SAY SO. `.tmr { flex: 1 1 0 }` with
 * no floor divided the row by however many timers were in it: six timers on a
 * phone in portrait is six columns of about sixty pixels, every clock
 * illegible, and nothing anywhere saying the rail had given up — rule 35, one
 * rail along.
 */
export function programmeCells(rows, capacity) {
  const list = Array.isArray(rows) ? rows : [];
  const cap = Math.max(1, Math.floor(Number(capacity) || 1));
  if (list.length <= cap) return list;
  const keep = Math.max(1, cap - 1);
  return [...list.slice(0, keep), { more: list.length - keep }];
}

/**
 * The reading floor for a programme figure, in px.
 *
 * `.lp-val`'s own clamp bottoms out at 12px, and a clock squeezed under that has
 * stopped being an instrument. 16 leaves a little above the floor rather than
 * sitting on it.
 */
export const MIN_FIGURE_PX = 16;

/**
 * The rail below which the label cannot have a share at all.
 *
 * `.lp-lbl` bottoms out at 9px with `line-height: 1.1`, so a head costs about
 * 10px however short the rail gets, plus the gap under it. Below that plus
 * `MIN_FIGURE_PX` there is not room for both.
 */
export const BARE_BELOW_PX = 30;

/**
 * HOW MUCH HEIGHT THE FIGURE MAY TAKE, and whether the label may be there at all.
 *
 * The cap used to be a fraction of the rail, and a fraction reserves nothing for
 * the label above it. It happened to hold at 16:9 because the label was still
 * scaling there; it stopped the moment `.lp-lbl`'s `clamp(9px, …)` floor bit,
 * which is every rail shorter than about 36px — a small composite region, or any
 * output below 1024x576. The digits were then cut through the middle by the
 * `overflow: hidden` that is meant to be the last resort. Measured at 900x300:
 * head 9.90 + gap 3.60 + figure 13.02 into a 21.00 rail, and 42% of every digit
 * gone. What is left of a sliced clock still reads as a valid time (RG-147).
 *
 * **Below the floor the LABEL stands down, never the figure.** That follows the
 * rail's own law: the label takes what it can and ellipses, the figure is never
 * the thing that gets cut. A name over an unreadable clock tells a preacher
 * nothing; a clock with no name still says how long is left.
 *
 * **The bare decision is taken from the RAIL alone, never from the measured
 * head**, and that is the correctness of it rather than a simplification.
 * Deciding from the head is a feedback loop: hide the head, it measures 0, the
 * condition that hid it is no longer true, it returns, it measures 10 again. The
 * first version of this did exactly that and oscillated on every update. The
 * rail's height is an input this function cannot affect.
 *
 * The measured head is still used, but only to size the figure once the label is
 * staying — where it is a fact about what is on screen rather than a prediction
 * about what will be.
 *
 * An unmeasured rail returns `null`, so the caller omits the declaration
 * entirely and the stylesheet's own fallback applies. Returning zero would be
 * worse than returning nothing: a custom property set to `0px` IS set, so
 * `var(--lp-room, …)` could never reach its fallback and the figure would be
 * invisible for the frame before the first measurement.
 */
export function programmeRoom(railPx, headPx) {
  const rail = Math.round(Number(railPx) || 0);
  if (rail <= 0) return null;
  if (rail < BARE_BELOW_PX) return { rail, bare: true, figure: rail };
  const head = Math.max(0, Number(headPx) || 0);
  return { rail, bare: false, figure: Math.round(Math.max(rail - head, MIN_FIGURE_PX)) };
}
