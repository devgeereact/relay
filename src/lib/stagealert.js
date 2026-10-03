/**
 * THE STAGE MESSAGE'S SIZE — one table, for the template path.
 *
 * A Stage Message is read by somebody mid-sentence, facing a congregation, from
 * a platform, in bright light. It is the whole screen or it is nothing, and the
 * operator's instruction is explicit: *a subtle tint or a small badge is not
 * acceptable*. What varies is how many words are in it, and that is what this
 * file answers.
 *
 * ## Four steps would be a guess with a decimal point on it
 *
 * There is no measurement available at the moment this is decided — the panel is
 * drawn over the renderer rather than inside a fitted box — so a continuous fit
 * would be arithmetic pretending to be evidence. Steps cannot produce a
 * pathological size, and the first one is docs/REBRAND.md §5's own figure for
 * §5's own case ("Wrap up — 5 minutes").
 *
 * ## THE LAST STEP IS THE BACKEND'S CAP, AND IT HAS TO BE
 *
 * `main::send_stage_alert` takes the first `MAX` characters. A step whose range
 * begins above that cap is a branch nothing can reach, which looks exactly like a
 * branch that works — RG-165, found once already on the phone. `ALERT_MAX` is
 * that cap, and `stagealerttemplate.test.js` reads it out of `main.rs` and out of
 * here in one assertion so neither can move alone.
 *
 * ## WHY THIS TABLE EXISTS TWICE, SAID OUT LOUD
 *
 * `Stage.svelte` states its own `ALERT_MAX` and `ALERT_STEPS` in its script, and
 * it keeps them: `stagezones.test.js` pins the phone's sizing by reading that
 * file's TEXT (`/const ALERT_MAX = (\d+);/`), so importing them from here would
 * either break that instrument or mean editing a test written to hold a rule that
 * has not changed. Two copies of a table is the thing this repository hates, so
 * the copies are PINNED TO EACH OTHER rather than left to drift — the same test
 * asserts, step for step, that the phone's table and this one are the same table.
 * If the phone's sizing is ever refactored onto this module, delete that case and
 * the duplication with it.
 */

/** The longest line `main::send_stage_alert` will deliver. */
export const ALERT_MAX = 140;

/**
 * Length → size class, most generous first.
 *
 * `xl` is §5's 8.5cqw: a phrase, which is what a Stage Message is meant to be.
 * `lg` and `md` are for the message an operator actually types when something has
 * gone wrong, which is a sentence or three — those ran past the bottom of a panel
 * that clips, silently, on the one surface whose whole purpose is that a person
 * reads every word of it while facing a congregation.
 */
export const ALERT_STEPS = [
  { max: 24, size: 'xl' },
  { max: 64, size: 'lg' },
  { max: ALERT_MAX, size: 'md' },
];

/**
 * The size class for a message.
 *
 * The fallback is the smallest step that EXISTS, not a fourth one: nothing longer
 * than `ALERT_MAX` can arrive through `send_stage_alert`, so this is a floor.
 * Were the cap ever raised without these steps following it, a long alert would
 * render at `md` and be readable, rather than at a size with no rule behind it.
 */
export function alertStep(text) {
  const n = typeof text === 'string' ? text.length : 0;
  return ALERT_STEPS.find((s) => n <= s.max)?.size ?? ALERT_STEPS[ALERT_STEPS.length - 1].size;
}
