/**
 * WHICH SHAPE A STAGE MESSAGE TAKES (RG-239).
 *
 * Three states, and the difference between them is what else is on the screen:
 *
 * - `alert`  — the whole screen, flashing. An instruction that must stop the
 *              service, and it covers everything because that is the point.
 * - `large`  — the reading's own room, whether or not there is a reading.
 * - `none`   — there is no message. Not a small one: a blank panel over a
 *              preacher's clock is worse than no panel.
 *
 * ## The strip is gone, and that is a decision reversed on purpose (RG-245)
 *
 * This rule shipped with a third state: beside a reading a message shrank to a
 * line along the foot, on the argument that *a reading is why the preacher is
 * looking at the screen*. The operator watched it on a phone and overruled it,
 * and the reason is better than the original: a Stage Message is the desk
 * speaking to ONE person in the middle of a sermon. It is never ambient. If it
 * was worth sending it is worth reading now, and a reading the preacher is
 * already holding in their hand is the thing it is most often about.
 *
 * So a message covers the reading. **The clock is the one thing it may not
 * take** — that is the page's job, not this rule's, and `Stage.svelte` keeps the
 * clock above the message for it.
 *
 * Pure, so the page and its test share one definition and the rule can be read
 * without mounting a phone.
 */
export function messagePlacement({ message, urgent = false } = {}) {
  if (!String(message ?? '').trim()) return 'none';
  return urgent ? 'alert' : 'large';
}
