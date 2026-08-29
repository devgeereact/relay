// The practice session's shell-level state, and the rehearsal discipline around it.
//
// Separated from `training.js` (which is pure) because starting and stopping a
// session touches the app: it forces rehearsal on and puts it back. That is the
// same rule the path check follows and for the same reason — **the controls are
// real, so the sandbox has to be too.** A drill that put a practice verse on a
// congregation's wall would be the single most embarrassing bug this product could
// ship.
import { writable, get } from 'svelte/store';
import * as training from './training.js';
import { setRehearsal, rehearsing, serviceLock } from './stores/capture.js';
import { humanError } from './errors.js';

export const practice = writable({
  session: training.newSession(),
  /** What rehearsal was BEFORE practice started, so it can be put back. */
  wasRehearsing: false,
  error: '',
});

/**
 * Begin. Refuses during a recorded service, and abandons if rehearsal will not
 * engage — running drills live is the accident this exists to prevent.
 */
export async function startPractice() {
  if (get(serviceLock).engaged) {
    practice.update((p) => ({
      ...p,
      error: 'A service is being recorded. End it before practising.',
    }));
    return false;
  }
  const wasRehearsing = get(rehearsing);
  try {
    if (!wasRehearsing) await setRehearsal(true);
    if (!get(rehearsing)) throw new Error('rehearsal did not turn on');
  } catch (e) {
    practice.update((p) => ({
      ...p,
      error: `Relay would not switch to rehearsal, so practice was not started — it will not put a practice verse on your screens. ${humanError(e)}`,
    }));
    return false;
  }
  practice.set({ session: training.start(), wasRehearsing, error: '' });
  return true;
}

/** End, and put rehearsal back the way it was found. */
export async function stopPractice() {
  const { wasRehearsing } = get(practice);
  practice.update((p) => ({ ...p, session: training.stop(p.session) }));
  try {
    if (!wasRehearsing) await setRehearsal(false);
  } catch (e) {
    // Said out loud. Leaving the app in rehearsal without telling anybody is how a
    // Sunday morning starts with screens that never light up.
    practice.update((p) => ({
      ...p,
      error: `Practice ended, but Relay could not leave rehearsal: ${humanError(e)}. Turn it off on the Live tab before the service.`,
    }));
  }
}
