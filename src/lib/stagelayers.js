/**
 * WHICH OF A STAGE SCREEN'S TWO LAYERS RELAY IS PLACING ITSELF (RG-226).
 *
 * A stage screen is sent two things no congregation template has any reason to
 * carry: a Stage Message, and the preacher's clocks. Both have a fallback, and
 * both fallbacks are right — a template designed before Stage Messages existed
 * is exactly the screen an emergency has to reach, and a clock that painted
 * nowhere because the look had no rail is a preacher with no clock (RG-224).
 *
 * What was missing is the sentence. An operator put a stage screen on a
 * scripture template, got a message along the foot and a rail they had not
 * designed, and concluded the template was being ignored — because nothing
 * anywhere said Relay had stepped in, or that the binding exists in the editor
 * to take it back. The behaviour was right and silent, which on this surface is
 * the same failure as the behaviour being wrong.
 *
 * Pure, and it answers about a TEMPLATE rather than about a screen, so the desk
 * and the editor could both ask it.
 *
 * **A hidden layer counts as absent.** The designer put it there and switched it
 * off, so the fallback is what will paint and the sentence has to describe what
 * will paint. **An unreadable template claims nothing** — `any: false` — because
 * "we do not know yet" is not "Relay is placing two layers", and a desk that
 * said the latter would be wrong for a moment on every mount.
 */
const declares = (template, bind) =>
  (template?.layout?.layers ?? []).some((L) => L?.bind === bind && L?.visible !== false);

/**
 * @param template the template this screen wears
 * @returns `{ message, programme, any }` — true where RELAY places it, not the designer
 */
export function stagePlacement(template) {
  const layers = template?.layout?.layers;
  if (!Array.isArray(layers) || !layers.length) {
    return { message: false, programme: false, any: false };
  }
  const message = !declares(template, 'stage_message');
  const programme = !declares(template, 'programme');
  return { message, programme, any: message || programme };
}
