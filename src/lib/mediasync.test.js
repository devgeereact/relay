// ALL MEDIA IN SYNC — WHAT THAT CAN HONESTLY MEAN (RG-220).
//
// The operator's words were *"All Media need to be in sync"*, and the plan
// (`docs/superpowers/plans/2026-09-21-stage-templates-media.md` §12) put three
// readings to them because they cost different things:
//
//   1 · every screen showing the same FRAME at the same instant — independent
//       browsers over church wifi, with no shared clock beyond `beat_ack`. Relay
//       cannot honestly promise it, and claiming it would be the kind of promise
//       this repository deletes;
//   2 · every screen STARTING together and being corrected when it drifts;
//   3 · the preacher's countdown agreeing with what the congregation is watching.
//
// They chose 2 and 3. So this is the rule for 2, and 3 follows from it: the
// stage's own player is corrected against the same instant as every other
// screen, so the figure beside it (RG-213) is about the same moment.
//
// **The baseline is Relay's clock, not a leader screen.** Every page already
// knows Relay's clock through `beat_ack`'s `hostOffsetMs`, and a screen elected
// as leader would make the whole wall follow whichever browser buffered worst.
//
// **A correction is a SEEK, and seeking is visible.** So it happens only past a
// tolerance, and the tolerance is a named decision rather than a number inside a
// component: two surfaces run this and a threshold typed twice is one that will
// differ between the stage and the wall.
import { describe, it, expect } from 'vitest';
import { syncSeek, SYNC_TOLERANCE_MS } from './mediasync.js';

const base = (over = {}) => ({
  startedAt: 1_000_000,
  now: 1_010_000, // ten seconds later
  duration: 60,
  position: 10,
  paused: false,
  looping: false,
  ...over,
});

describe('syncSeek — where this clip should be, if it should move at all', () => {
  it('a screen that is where it should be is left alone', () => {
    expect(syncSeek(base())).toBeNull();
  });

  it('a screen inside the tolerance is left alone, because a seek is visible', () => {
    const near = (SYNC_TOLERANCE_MS - 200) / 1000;
    expect(syncSeek(base({ position: 10 + near }))).toBeNull();
    expect(syncSeek(base({ position: 10 - near }))).toBeNull();
  });

  it('a screen that has fallen behind is sent to where the clip should be', () => {
    // Four seconds behind: buffering, a late start, a browser that was throttled
    // in a background tab. The answer is a position in SECONDS, because that is
    // what a media element's `currentTime` is.
    expect(syncSeek(base({ position: 6 }))).toBeCloseTo(10, 3);
  });

  it('and one that has run ahead is pulled back the same way', () => {
    expect(syncSeek(base({ position: 15 }))).toBeCloseTo(10, 3);
  });

  it('a looping clip is measured within the pass it is on', () => {
    // 130 seconds into a 60-second loop is 10 seconds into the third pass. A rule
    // that did not wrap would ask every looping screen to seek past its own end,
    // for ever.
    expect(syncSeek(base({ now: 1_130_000, looping: true, position: 10 }))).toBeNull();
    expect(syncSeek(base({ now: 1_130_000, looping: true, position: 30 }))).toBeCloseTo(10, 3);
  });

  it('a clip that should have finished is not dragged back to its end', () => {
    // 90 seconds into a 60-second clip that is not looping: it is over, the
    // element is sitting at its last frame, and seeking it to 60 every beat would
    // be a correction that never stops.
    expect(syncSeek(base({ now: 1_090_000, position: 60 }))).toBeNull();
  });

  it('nothing is corrected while the clip is held', () => {
    // A held clip has left the clock it started on. Nothing here can know how
    // long it was held for, so it says so by refusing rather than by guessing —
    // the alternative is a Play that jumps the picture forward by however long
    // the operator was thinking about it.
    expect(syncSeek(base({ position: 2, paused: true }))).toBeNull();
  });

  it('and nothing is corrected before there is anything to correct against', () => {
    for (const missing of [
      { startedAt: null },
      { startedAt: undefined },
      { duration: 0 },
      { duration: NaN },
      { duration: Infinity },
      { position: NaN },
      { now: null },
    ])
      expect(syncSeek(base(missing)), JSON.stringify(missing)).toBeNull();
  });

  it('a clip that started in the future is not wound backwards', () => {
    // Clock skew between Relay and a browser that has not had a `beat_ack` yet.
    // A negative expected position is not a position.
    expect(syncSeek(base({ now: 999_000 }))).toBeNull();
  });

  it('the tolerance is a decision, and a visible one', () => {
    expect(SYNC_TOLERANCE_MS).toBeGreaterThanOrEqual(500);
    expect(SYNC_TOLERANCE_MS).toBeLessThanOrEqual(5000);
  });
});
