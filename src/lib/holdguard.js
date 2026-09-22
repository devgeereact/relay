/**
 * A PRESS THAT HAS TO BE MEANT (RG-242).
 *
 * The Control panel on the preacher's screen fires to every screen in the
 * building, and it opened on a single tap of a small button in the header of a
 * page somebody is holding mid-sermon. A sleeve, a thumb on the edge, a phone
 * picked up off a lectern — one contact and it is open over the reading.
 *
 * A hold is the ordinary answer and the one the platform already teaches. It is
 * deliberately NOT a confirmation: rule 41 forbids a native dialog, and an
 * in-app one would be a second thing to dismiss while a congregation waits.
 *
 * Pure and clock-free — every instant is passed in — so the rule can be tested
 * without timers and cannot drift with the scheduler.
 */
export const HOLD_MS = 550;

export function holdGuard(threshold = HOLD_MS) {
  let startedAt = null;
  return {
    down(at) {
      startedAt = at;
    },
    /** Was this release a real hold? Consumes the press either way. */
    up(at) {
      const from = startedAt;
      startedAt = null;
      if (from === null) return false;
      return at - from >= threshold;
    },
    /**
     * The press is off. A cancelled press is FORGOTTEN rather than banked: a
     * press cancelled by a scroll would otherwise count toward the next tap and
     * let exactly the brush this guard exists to stop straight through.
     */
    cancel() {
      startedAt = null;
    },
  };
}
