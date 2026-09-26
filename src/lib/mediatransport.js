/**
 * WHAT A CLIP IS DOING, APPLIED TO ONE PLAYER — the rule, in one place.
 *
 * Every screen in the building is sent the same `media_transport` frame, and
 * until RG-214 two surfaces answered it differently: `TemplateRender` applied it
 * to the element, and `Stage.svelte` ignored the frame and hard-coded `loop` on
 * its own slide. So Pause, Play and Loop moved every screen except the one in
 * front of the preacher, and a clip held on the wall went on running there for
 * the rest of the cue.
 *
 * That is the twin-door shape this repository keeps recording, so the rule moved
 * out of the component rather than being typed a second time. It is a function
 * over an element because that is all it ever was: three properties and the one
 * piece of bookkeeping below.
 *
 * ## The bookkeeping, which is the only subtle part
 *
 * **Replay and scrub are events, not states**, and the wire carries each as its
 * own counter for exactly that reason. The caller keeps the last pair acted on
 * and hands it back in, so a re-render, a reconnect, or a retained frame
 * replayed on hello cannot start the clip again or drag it back to where
 * somebody once dropped the handle — a screen that rejoined mid-clip would
 * otherwise jump, because the frame it was handed still names an instruction
 * from ten minutes ago.
 *
 * **They are counted APART.** One epoch for both would swallow a scrub made
 * immediately after a replay, which is precisely the pair of presses an operator
 * makes when a clip started in the wrong place (RG-221).
 *
 * **Volume is the room, not the clip.** It is applied when the frame names one
 * and left entirely alone when it does not: a Pause that also reset the sound to
 * full is the shape of every "one control moved another" bug this transport
 * exists to avoid. It is clamped, because a value outside 0–1 throws on a real
 * media element.
 *
 * **`play()` may be rejected** by autoplay policy or by a source that is not
 * ready, and a rejection is swallowed here deliberately. What says whether a
 * clip is actually moving is the BEAT, which reads the element; a control that
 * reported its own instruction back as an outcome is rule 35 with extra steps.
 *
 * @param el    the `<video>`, or null before one is mounted
 * @param t     `{ paused, loop, replayEpoch, seekEpoch, seekMs, volume }`, or null
 * @param acted `{ replay, seek }` — the epochs this element has already acted on
 * @returns the pair to remember for the next call
 */
export function applyMediaTransport(el, t, acted = null) {
  const seen = { replay: acted?.replay ?? null, seek: acted?.seek ?? null };
  if (!el || !t) return seen;
  // `loop` is a property, not markup: setting it through an attribute would need
  // the element to be re-created, and re-creating a video to change its loop
  // restarts it from zero.
  el.loop = !!t.loop;
  // THE ROOM'S LEVEL. Only when the frame names one — see the note above.
  if (t.volume != null && Number.isFinite(Number(t.volume))) {
    el.volume = Math.max(0, Math.min(1, Number(t.volume)));
  }
  const epoch = t.replayEpoch ?? null;
  if (epoch != null && epoch !== seen.replay) {
    seen.replay = epoch;
    try {
      el.currentTime = 0;
    } catch {
      /* a video with no metadata yet cannot be seeked; the next frame will. */
    }
  }
  // THE SCRUB, AFTER THE REPLAY. Order matters and is not arbitrary: an operator
  // who replays and then scrubs means "start again, at this point", and a scrub
  // applied first would be overwritten by the zero.
  const seekEpoch = t.seekEpoch ?? null;
  if (seekEpoch != null && seekEpoch !== seen.seek) {
    seen.seek = seekEpoch;
    const at = Number(t.seekMs);
    if (Number.isFinite(at) && at >= 0) {
      try {
        el.currentTime = at / 1000;
      } catch {
        /* no metadata yet; the next frame carries the same epoch. */
      }
    }
  }
  const want = !!t.paused;
  if (want && !el.paused) el.pause();
  else if (!want && el.paused) void el.play?.()?.catch?.(() => {});
  return seen;
}
