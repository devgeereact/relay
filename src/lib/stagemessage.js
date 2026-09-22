/**
 * WHICH SHAPE A STAGE MESSAGE TAKES (RG-239).
 *
 * Three states, and the difference between them is what else is on the screen:
 *
 * - `alert`  — the whole screen, flashing. An instruction that must stop the
 *              service, and it covers a reading because that is the point.
 * - `large`  — the reading's own room, because there is no reading. A phone on a
 *              lectern showing one line of 14px text is a phone nobody reads
 *              from a platform, and that was the state the message spent most of
 *              its life in.
 * - `strip`  — along the foot, beside whatever is being read. A reading is why
 *              the preacher is looking at the screen, and a message may not take
 *              that room while one is up.
 * - `none`   — there is no message. Not a small one: a blank panel over a
 *              preacher's clock is worse than no panel.
 *
 * Pure, so the page and its test share one definition and the rule can be read
 * without mounting a phone.
 */
export function messagePlacement({ message, urgent = false, reading = false, slide = false } = {}) {
  if (!String(message ?? '').trim()) return 'none';
  if (urgent) return 'alert';
  return reading || slide ? 'strip' : 'large';
}
