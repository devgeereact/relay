/**
 * WHAT `→` DOES NEXT, AND WHY IT IS A MODULE RATHER THAN A LINE IN A VIEW.
 *
 * The transport is mode-aware and says so in the bar: `→` steps a plan SLIDE
 * when plan content is on air, and walks the passage (VERSE) when a detected or
 * manual verse is. The same key silently meaning two things is how the wrong
 * thing reaches a congregation, so the mode is printed where the operator can
 * read it before they press.
 *
 * It lived as one reactive expression inside `Live.svelte`, which meant the one
 * rule governing the most-pressed key in the product could only be tested by
 * mounting the whole run surface or by grepping its source. Both defects below
 * were reported by an operator rather than caught by an instrument.
 *
 * ── THE TWO DEFECTS, 2026-09-20 ───────────────────────────────────────────
 *
 * *"The Next button should work with next verse, and next slide if not a verse
 * on the screen, and allow the keyboard control work accurately."*
 *
 * 1. The rule began `openPlan && items.length && …`, so with no plan loaded it
 *    read VERSE unconditionally, including when the screens were empty.
 * 2. Worse: `$live` means CONTENT IS ARMED, of any kind. A song, a notice or a
 *    countdown sets it exactly as a verse does — and rule 38 has
 *    `ContextMemory::forget` clear the remembered passage for precisely those
 *    kinds. So the bar said VERSE over a song, the operator pressed Next, the
 *    engine answered `NoPassage`, and nothing moved. The transport looked broken
 *    because it was describing itself wrongly, which is rule 35 on the one
 *    control an operator presses more than any other.
 *
 * Both are the same mistake: asking which TAB is open, or whether SOMETHING is
 * armed, instead of asking what is actually on the wall.
 */

/** The content kind `pipeline::Fire` stamps on a scripture fire. */
export const SCRIPTURE = 'scripture';

/**
 * Is a walkable passage genuinely in front of people?
 *
 * All four parts are load-bearing:
 *   · `live`        something is armed at all,
 *   · `!screenBlack` and a congregation can see it. A blackout leaves `live` set
 *                   and blanks the screens; reading VERSE after `B` and SLIDE
 *                   after `Esc` meant the same state drove opposite transports,
 *                   and the next `→` undid the emergency key,
 *   · `!planOnAir`  it did not come from the plan, or `→` belongs to the plan,
 *   · `kind`        and it is SCRIPTURE. This is the part that was missing.
 */
export function verseIsOnAir(live, screenBlack, planOnAir) {
  return !!live && !screenBlack && !planOnAir && live.kind === SCRIPTURE;
}

/**
 * Which mode the transport is in, and therefore what the bar must say.
 *
 * A verse on the wall always wins. Otherwise the plan gets the key if there is
 * a plan to step. With neither, it stays VERSE — not because a verse is likely,
 * but because there is nothing else for the key to do and `nav` will say so in
 * words rather than the bar claiming a plan that is not loaded.
 */
export function transportMode({ live, screenBlack, planOnAir, planLength }) {
  if (verseIsOnAir(live, screenBlack, planOnAir)) return 'verse';
  return planLength > 0 ? 'slide' : 'verse';
}

/**
 * The engine answered; should the press fall through and step the plan instead?
 *
 * `mode` decides from what is on the wall, and the wall can be right while the
 * ENGINE has nothing to walk: a passage forgotten under unrelated content
 * (rule 38), a verse fired before a restart, a reference the library does not
 * hold. In each of those the operator pressed the transport and the honest
 * answer is "there is no verse here" — which is exactly the case the operator
 * asked to carry on into the plan.
 *
 * `end_of_passage` DELIBERATELY DOES NOT FALL THROUGH. Operator decision,
 * 2026-09-20, asked and answered: reaching the last verse of a reading is a
 * correct boundary, not a failure, and carrying straight on into the plan would
 * put a slide in front of a congregation on a press that was meant to do
 * nothing. The ends of a plan are already hard stops that never wrap; this is
 * the same rule facing the other way. One press says the reading ended, `Esc`
 * hands the transport back to the plan, and the next press steps it.
 *
 * `fired` does not fall through either, for the obvious reason: it worked.
 */
export function fallsThroughToPlan(outcome, planLength) {
  if (!(planLength > 0)) return false;
  return outcome?.kind === 'no_passage' || outcome?.kind === 'not_in_library';
}

/**
 * P-1's question, asked of one `output://content` payload: is this the plan
 * scripture the console said it was about to put up?
 *
 * Only scripture is judged. It is the one kind that can reach the wall with no
 * wrapper running on this console — the AI's own fire, the preacher's phone, the
 * spoken "next" — and therefore the one kind the listener has to decide about.
 * A song, a picture or a countdown only ever arrives through a wrapper that
 * already knew whether it was a plan fire, so those answer `true` here and the
 * wrapper's own `keepPlan` is what decides.
 *
 * Loose on spelling because the plan holds what the operator typed and the wall
 * holds what the corpus calls it. Nothing expected means nothing matches: the
 * default has to be "this was not us".
 */
export function contentIsExpectedPlanFire(payload, expected) {
  const kind = payload?.kind ?? SCRIPTURE;
  if (kind !== SCRIPTURE) return true;
  if (!expected?.reference) return false;
  const norm = (s) => String(s ?? '').toLowerCase().replace(/\s+/g, ' ').trim();
  return norm(payload?.reference) === norm(expected.reference);
}
