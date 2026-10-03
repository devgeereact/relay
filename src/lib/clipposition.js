/**
 * WHERE THE CLIP IS, BETWEEN THE BEATS THAT SAY SO — RG-255.
 *
 * `mediaclock.js` answers "how long is left" from what the SCREENS report.
 * This answers the finer question the scrub bar asks — where is the handle
 * *right now* — and it exists because the honest answer to that is not on the
 * wire and cannot be.
 *
 * **The measurement, not a guess.** A screen samples its own player once per
 * `BEAT_INTERVAL_MS` (2000 ms, `outputHealth.js`) and the console polls the
 * backend on its own 2000 ms timer (`capture.js::startChannelHealth`). Those
 * two are independent, so a reported position is between 0 and about four
 * seconds old and changes at most once every two seconds. A bar driven straight
 * off it lurches twice a second-and-a-half and lags a wall the operator is
 * watching; the `step="250"` the control carried was four times finer than
 * anything behind it.
 *
 * So the desk does the arithmetic in between: the last position a screen
 * reported, plus the wall clock since that reading arrived. That is the same
 * thing `mediasync.js::syncSeek` already does on the output side to decide
 * whether a screen has drifted, which is the precedent that makes this
 * ordinary rather than novel.
 *
 * **It stops being clever the moment it stops knowing**, and that is most of
 * the module:
 *
 *   - a HELD clip does not advance. Its position is a fact, not a rate.
 *   - a beat older than `POSITION_STALE_MS` yields NOTHING — not a zero, which
 *     would draw a clip at its start, and not the last reading, which would
 *     draw a clip that is still running. A screen that has stopped beating may
 *     have stopped playing, been unplugged or gone to sleep, and the desk
 *     cannot tell which.
 *   - a clock that goes backwards adds nothing rather than subtracting.
 *   - and the answer is clamped to the clip's own length, so a clip that ended
 *     unobserved cannot push the handle off the end of the bar or count the
 *     figure beside it into negative time.
 *
 * Pure, and everything it needs is an argument — the same shape as
 * `mediaclock.js` and for the same reason: the rule can be asserted without a
 * timer, a socket or a player.
 */

/**
 * How old a reading may be before it stops meaning anything.
 *
 * This is `channels::BEAT_STALE_MS` — three beats and a quarter — and it is
 * deliberately the SAME figure rather than a second opinion about the same
 * silence. The backend already refuses to answer `media_of` past it; a
 * different number here would mean the desk drawing a handle for a clip the
 * backend had already given up on, and the two would drift apart the first time
 * either one moved.
 */
export const POSITION_STALE_MS = 6500;

/**
 * Where the clip is now, or `null` when that cannot honestly be said.
 *
 * @param {object} a
 * @param {number|null} a.positionMs  what a screen last reported
 * @param {number|null} a.durationMs  how long the clip is
 * @param {boolean} a.paused          whether that screen said it was held
 * @param {boolean} a.known           whether any screen is reporting at all
 * @param {number} a.seenAt           when that reading reached the desk (ms)
 * @param {number} a.now              now (ms)
 * @returns {number|null}
 */
export function clipPosition({ positionMs, durationMs, paused, known, seenAt, now } = {}) {
  if (!known) return null;
  if (!Number.isFinite(positionMs) || !Number.isFinite(durationMs) || durationMs <= 0) return null;
  if (!Number.isFinite(seenAt) || !Number.isFinite(now)) return null;

  const age = now - seenAt;
  // A CLOCK THAT WENT BACKWARDS IS NOT A STALE BEAT. It is a machine that
  // resynced mid-service, and the reading is still the newest thing there is —
  // so the position stands and nothing is added to it. Treating it as stale
  // would blank a bar over a clip that is playing perfectly well.
  if (age > POSITION_STALE_MS) return null;

  // A held clip is where it was left. Adding wall clock to it would walk the
  // handle across a picture that is standing still, which is the fault this
  // module exists to avoid in the other direction.
  const advanced = paused ? positionMs : positionMs + Math.max(0, age);
  return Math.min(durationMs, Math.max(0, advanced));
}
