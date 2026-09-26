/**
 * WHO TAKES THE ROOM NOBODY ELSE IS USING (RG-244).
 *
 * The preacher's screen gives its height to the reading and a fixed share to the
 * figures beneath it. That is right while there is a reading. With nothing
 * fired — most of a service, from the platform's point of view — it left a large
 * empty region above a small clock.
 *
 * One case was already handled: a running COUNTDOWN took the room, on the
 * argument that a pre-service countdown is the whole reason anybody looks at
 * this page. The same argument is true of the Stage Timer, the clock a preacher
 * is actually working to, and it was not.
 *
 * Four answers, in the order they are decided:
 *
 * - `reading`   — something is being read, or a slide is up. It wins, always:
 *                 it is what the screen is for. Also the answer when no clock is
 *                 running at all, because "— standby —" is the reading area's
 *                 own state and a stopped clock must not take a screen's height
 *                 to say nothing.
 * - `figures`   — a countdown is running. It beats the Stage Timer because
 *                 it is the one with a deadline the whole room is waiting for.
 * - `programme` — the Stage Timer is running and nothing else wants the room.
 *
 * Pure, so the page and its test share one definition.
 */
export function restingLayout({ reading = false, slide = false, countdown = false, programme = false } = {}) {
  if (reading || slide) return 'reading';
  if (countdown) return 'figures';
  if (programme) return 'programme';
  return 'reading';
}
