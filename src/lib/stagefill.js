/**
 * WHAT A STAGE SCREEN DOES WITH THE ROOM NOBODY ELSE IS USING — the big screen's
 * half of RG-244 (RG-285).
 *
 * The decision is DECISIONS §118; this is its one implementation.
 *
 * `stageresting.js` answers this for `stage.html`, the preacher's phone: with
 * nothing fired — most of a service, from the platform's point of view — the
 * clock a preacher is working to takes the height rather than sitting in a strip
 * under an empty region printing "— standby —".
 *
 * `output.html` is the OTHER way to put a screen in front of a preacher
 * (DECISIONS §89), and it had none of it: the operator's own platform monitor
 * showed a 130px rail across the foot of a 1080p screen with nothing above it.
 * That is RG-224, RG-265 and RG-280 for the fourth time — a thing built for the
 * phone, reported on the TV, and the TV never asked.
 *
 * ## THE RULE IS IMPORTED, NEVER RESTATED
 *
 * `restingLayout` is the decision and this is one function over it. Two surfaces
 * showing the same clocks in the same room, in front of the one person who
 * cannot look away from either, must not hold two opinions about when a clock
 * may take a screen — and `stagefill.test.js` asserts the two agree on every
 * combination of inputs rather than on the cases somebody thought of.
 *
 * What this adds to the phone's rule is the two facts the phone does not have:
 * a stage TV can be showing a CLIP whose own countdown has taken the rail's
 * place (requirement 6), and it can be carrying a quiet word from the desk that
 * the phone answers through `stagemessage.js` instead.
 *
 * Pure, so the renderer and its test share one definition.
 */
import { restingLayout } from './stageresting.js';

/**
 * How much of a filled screen the WORDS take when a clock is sharing it.
 *
 * The words get the larger share because they are prose: a message wraps, and
 * the one thing it may not do is clip (`.lmsg` is `overflow: hidden`, and a
 * message read half-way through by somebody facing a congregation is worse than
 * a small one). Digits do not wrap — the rail's own caps shrink them to whatever
 * box they are given and `programmeRoom` reports when that box is too small.
 *
 * 45% of a 1080p screen is 486px of rail against the 130px RG-265 left it at,
 * so even the smaller half of a shared screen is more than three times the clock
 * the operator complained about.
 */
export const FILL_MESSAGE_PCT = 55;

/**
 * THE SIZE CAP A FILLING RAIL WEARS, which is a cap that cannot bind.
 *
 * `--lp-sz` is the template editor's Size field converted into the rail's own
 * container (`timers.js::railSize`), and `.lp-val` treats it as a CAP: a figure
 * may shrink to fit its box, it may not grow past what the designer asked for.
 *
 * That is exactly right for a designed box and meaningless once the rail IS the
 * screen — there is no longer a box for the designer's share to be a share of,
 * and honouring it would leave a 3cqw figure, drawn for a corner, marooned in
 * the middle of a projector. So while the rail fills, the two REAL caps decide:
 * the per-column budget (which is what stops `1:30:13` being sliced into a
 * shorter time that still reads as valid — RG-147) and the rail's own measured
 * height. 100cqw is wider than the container, so it can never be the smallest of
 * the three.
 */
export const FILL_SIZE_CQW = 100;

/**
 * DOES THE CLIP'S OWN COUNTDOWN STAND IN FOR THE STAGE TIMER? (requirement 6)
 *
 * *"When a Media is Playing I want you to Cover the Stage timer with the clip
 * countdown to make it clean and professional... and return the timer back when
 * the media is cleared or done and this happened to only Videos Not to a static
 * picture."*
 *
 * ## The trap, which is a zero that is not a null
 *
 * `mediaclock.js::clipRemainingMs` answers `null` for every state a player can
 * be in before it knows, and **0 for a clip that has finished** — which is a
 * correct answer to the question it is asked and the wrong input to this one. A
 * cover keyed on "is there a figure" would hold the rail off a stage screen for
 * the rest of the service behind a dead `0:00`, which is the thing the operator
 * asked to have go away arriving through the door built to grant it.
 *
 * So the cover asks for time REMAINING, strictly. A clip that has run out has
 * given the room back before anybody unmounts it.
 *
 * `still` is the flag that already separates a picture from a clip everywhere
 * else in the renderer, and it is the operator's own distinction.
 */
export function clipCoversTimer({ stage = false, still = false, remainingMs = null } = {}) {
  if (!stage || still) return false;
  return typeof remainingMs === 'number' && Number.isFinite(remainingMs) && remainingMs > 0;
}

/**
 * WHO TAKES A STAGE SCREEN THAT NOTHING HAS BEEN FIRED TO.
 *
 * - `none`    — something is on the screen. It wins, always: a reading, a slide
 *               and a congregation countdown are what the screen is FOR, and
 *               §116's line that a note may not cover a reading is the same
 *               sentence said about a clock.
 * - `timer`   — the Stage Timers have the frame.
 * - `message` — a quiet word from the desk has it.
 * - `both`    — words above, digits below. Stacked and never overlapped: two
 *               things in one box with a stacking rule deciding which the
 *               preacher reads is the arrangement RG-247 measured at 140px off
 *               the middle of the phone.
 *
 * The first three inputs go STRAIGHT to `restingLayout`, so a `timer` verdict
 * here is exactly the phone's `programme` verdict and can never be wider.
 */
export function stageFill({
  reading = false,
  slide = false,
  countdown = false,
  timers = 0,
  message = false,
  clipCovers = false,
} = {}) {
  // The clip's countdown has taken the rail's place, so there is no rail to
  // enlarge — and a rail filling the screen BEHIND a clip clock would be the
  // thing requirement 6 asked to have covered, at its largest.
  const clocks = clipCovers ? 0 : Number(timers) || 0;
  const room = restingLayout({ reading, slide, countdown, programme: clocks > 0 });
  // A message never takes a screen that is in use. It still arrives — as the
  // foot strip §116 gave it, or in the place a template's own `stage_message`
  // layer declares — it simply does not take the room.
  if (room !== 'programme' && (reading || slide || countdown)) return 'none';
  const wantsTimer = room === 'programme';
  if (message && wantsTimer) return 'both';
  if (message) return 'message';
  if (wantsTimer) return 'timer';
  return 'none';
}

/**
 * The boxes a fill hands out, in percent of the frame, or `null` for a part that
 * is not filling.
 *
 * Percent because that is the unit every other box in `TemplateRender` is drawn
 * in (`boxStyle`), so a filling rail and a designed one are the same kind of
 * thing in the same coordinates and the geometry can be read off the element.
 */
export function fillGeometry(what) {
  const box = (top, height) => ({ top, height });
  switch (what) {
    case 'timer':
      return { message: null, timer: box(0, 100) };
    case 'message':
      return { message: box(0, 100), timer: null };
    case 'both':
      return {
        message: box(0, FILL_MESSAGE_PCT),
        timer: box(FILL_MESSAGE_PCT, 100 - FILL_MESSAGE_PCT),
      };
    default:
      return { message: null, timer: null };
  }
}
