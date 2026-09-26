/**
 * WHICH INSTANT A CLIP PREVIEW MEASURES FROM — RG-271.
 *
 * `TemplateRender`'s corrector pulls a player back to where Relay's clock says
 * the clip should be, measured from `content.media_started_at`. A scrub MOVES
 * that instant: Rust publishes `started_at = now - seek_ms` precisely so the
 * corrector does not undo the operator's own drag, and every output page
 * restates it (`Output.svelte`, the `media_transport` branch).
 *
 * **RG-260 made Live do the same and got it wrong, and this is the fix for that
 * rather than for something new.** The transport store became the frame the
 * screens were sent, which was right, and Live then applied `startedAt`
 * unconditionally. `startedAt` is only ever set by a scrub and nothing clears
 * it, so after ONE scrub every clip fired for the rest of a service was
 * previewed against that scrub's baseline: a clip fired fresh drew as though it
 * had started however long ago the handle was dropped, while every projector in
 * the building drew it from zero. The operator saw the console disagree with
 * the room, which is the one thing that surface exists to not do.
 *
 * The rule is a COMPARISON, not a flag. A scrub matters when it happened after
 * the content was fired; a scrub from before belongs to a clip that is gone.
 * No new state, nothing to clear, and nothing to forget to clear.
 *
 * Pure: both instants are arguments, so the rule is asserted without a player,
 * a socket or a clock.
 */

/**
 * @param {number|null|undefined} firedAt   `content.media_started_at`
 * @param {number|null|undefined} scrubbedAt the transport frame's `startedAt`
 * @returns {number|null} the instant to measure from, or `null` for no clip
 */
export function baselineFor(firedAt, scrubbedAt) {
  // NO CLIP, NO BASELINE. A verse has no instant to measure from, and inventing
  // one out of a stale scrub is the same defect with nothing on screen to
  // notice it.
  if (!Number.isFinite(firedAt)) return null;
  if (!Number.isFinite(scrubbedAt)) return firedAt;
  return scrubbedAt > firedAt ? scrubbedAt : firedAt;
}

/**
 * IS THIS READING ABOUT THE CLIP THAT IS UP? — RG-271.
 *
 * A screen reports its clip position once per 2000 ms beat and the console
 * polls on its own 2000 ms timer, so for up to about four seconds after a new
 * clip is fired the newest reading on the desk is still about the clip BEFORE
 * it. A clip that had run to 1:18 drew the new one's bar near its end and then
 * snapped back to zero — *"the bar already run half way when the video is just
 * starting"*.
 *
 * `describeMediaClock` cannot tell the difference: a `MediaBeat` carries a
 * position, a duration and whether it is paused, and says nothing about WHICH
 * clip. So the desk holds the question itself — it knows when the clip on the
 * screens changed, and a reading taken before that change is about something
 * else.
 *
 * `null` for `changedAt` accepts everything, deliberately: a bar that refused
 * until it had witnessed a change would never draw at all on a console opened
 * part-way through a clip.
 *
 * @param {number} seenAt     when the reading reached the desk
 * @param {number|null} changedAt when the clip on the screens last changed
 */
export function readingIsAboutThisClip(seenAt, changedAt) {
  if (!Number.isFinite(changedAt)) return true;
  if (!Number.isFinite(seenAt)) return false;
  return seenAt >= changedAt;
}
