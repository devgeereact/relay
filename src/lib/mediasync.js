/**
 * KEEPING THE SCREENS ON THE SAME CLIP AT THE SAME PLACE (RG-220).
 *
 * The operator asked for *"all media in sync"* and chose, of the three things
 * that could mean, the two Relay can honestly do: every screen STARTS together
 * and is corrected when it drifts, and the preacher's countdown agrees with what
 * the congregation is watching. Frame-exact playback across independent browsers
 * on church wifi is not one of them, and claiming it would be the kind of promise
 * this repository deletes.
 *
 * ## The baseline is Relay's clock, never a leader screen
 *
 * A clip carries the instant Relay sent it (`media_started_at`), stamped at the
 * one door content leaves by. Every page already knows Relay's clock: the output
 * page takes it from `beat_ack` (RG-194) and the stage page from the same reply.
 * So each screen works out where the clip should be and corrects ITSELF, with no
 * round trip and nothing to elect. A leader screen would make the whole wall
 * follow whichever browser buffered worst.
 *
 * ## A correction is a seek, and a seek is visible
 *
 * So it happens only past `SYNC_TOLERANCE_MS`, and that threshold is here rather
 * than inside a component because two surfaces run this rule: a threshold typed
 * twice is one that will differ between the stage and the wall, which is the
 * failure this repository keeps recording under other names.
 *
 * ## What it refuses to answer, and why each refusal is a refusal
 *
 * **A held clip.** Nothing here knows how long the operator held it for, so the
 * baseline no longer describes it. Guessing would mean a Play that jumps the
 * picture forward by however long they were thinking about it.
 *
 * **A clip past its end.** It is over and the element is resting on its last
 * frame; seeking it to the end on every beat is a correction that never stops.
 *
 * **A clip that started in the future.** Clock skew, on a page that has not had
 * its first `beat_ack`. A negative position is not a position.
 */

/**
 * How far out a screen may be before it is worth a visible correction.
 *
 * Two seconds. Below about a second the seek is the more noticeable of the two
 * problems — a picture that jumps is read as a fault, where a picture a beat
 * behind is read as nothing at all. Above two or three, a screen is far enough
 * out that a congregation looking from one to the other can see it.
 */
export const SYNC_TOLERANCE_MS = 2000;

/**
 * Where this player should seek to, or `null` to leave it exactly alone.
 *
 * @param startedAt Relay's clock when the clip was sent (ms), or null
 * @param now       Relay's clock now (ms) — the page's own clock plus its offset
 * @param duration  the clip's length in SECONDS, as a media element reports it
 * @param position  where this player is, in seconds
 * @param paused    is the clip being held
 * @param looping   does it repeat
 * @returns a position in seconds, or null
 */
export function syncSeek({ startedAt, now, duration, position, paused, looping } = {}) {
  if (paused) return null;
  const start = Number(startedAt);
  const at = Number(now);
  const len = Number(duration);
  const here = Number(position);
  if (!Number.isFinite(start) || !Number.isFinite(at)) return null;
  if (!Number.isFinite(len) || len <= 0) return null;
  if (!Number.isFinite(here)) return null;
  const elapsed = (at - start) / 1000;
  if (elapsed < 0) return null;
  // A LOOP IS MEASURED WITHIN THE PASS IT IS ON. Without the wrap, every looping
  // screen would be asked to seek past its own end for the rest of the service.
  const want = looping ? elapsed % len : elapsed;
  if (!looping && want >= len) return null;
  if (Math.abs(want - here) * 1000 <= SYNC_TOLERANCE_MS) return null;
  return want;
}
