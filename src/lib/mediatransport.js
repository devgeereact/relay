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
 * **Replay is an event, not a state**, and the wire carries it as a counter for
 * exactly that reason. The caller keeps the last epoch acted on and hands it
 * back in, so a re-render, a reconnect, or a retained frame replayed on hello
 * cannot start the clip again — a screen that rejoined mid-clip would otherwise
 * jump to the beginning, because the frame it was handed still names a replay
 * from ten minutes ago.
 *
 * **`play()` may be rejected** by autoplay policy or by a source that is not
 * ready, and a rejection is swallowed here deliberately. What says whether a
 * clip is actually moving is the BEAT, which reads the element; a control that
 * reported its own instruction back as an outcome is rule 35 with extra steps.
 *
 * @param el          the `<video>`, or null before one is mounted
 * @param t           `{ paused, loop, replayEpoch }`, or null for no instruction
 * @param actedReplay the last replay epoch this element acted on
 * @returns the epoch to remember for the next call
 */
export function applyMediaTransport(el, t, actedReplay = null) {
  if (!el || !t) return actedReplay;
  // `loop` is a property, not markup: setting it through an attribute would need
  // the element to be re-created, and re-creating a video to change its loop
  // restarts it from zero.
  el.loop = !!t.loop;
  let acted = actedReplay;
  const epoch = t.replayEpoch ?? null;
  if (epoch != null && epoch !== acted) {
    acted = epoch;
    try {
      el.currentTime = 0;
    } catch {
      /* a video with no metadata yet cannot be seeked; the next frame will. */
    }
  }
  const want = !!t.paused;
  if (want && !el.paused) el.pause();
  else if (!want && el.paused) void el.play?.()?.catch?.(() => {});
  return acted;
}
