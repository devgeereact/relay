// WHERE THE CLIP IS, BETWEEN THE BEATS THAT SAY SO (RG-255).
//
// The operator asked for a slider that "needs to be accurate with where the media
// is at, to help slide media to where and place of choice to start from". The
// seek half is already exact — `seekMs` is an absolute position and the epoch
// guarantees a screen acts on it once. The READBACK half is not, and the figures
// were measured rather than guessed:
//
//   a screen samples its own player once per 2000 ms beat  (`outputHealth.js`)
//   the console polls the backend on its own 2000 ms timer (`capture.js`)
//   a beat older than 6500 ms says nothing at all           (`channels.rs`)
//
// So the reported position is between 0 and ~4 s old and changes at most once
// every two seconds. A bar driven straight off it lurches, and `step="250"` is
// four times finer than anything behind it.
//
// This module is the desk's own arithmetic in between: the last position a screen
// reported, plus the wall clock since it arrived. That is the same reasoning
// `mediasync.js::syncSeek` already does on the output side, which is the
// precedent that makes it safe rather than a guess.
//
// **It must stop being clever the moment it stops knowing.** A held clip does not
// advance; a beat nobody has refreshed is not a position; and an operator with a
// finger on the handle owns the handle. Each of those is a case below, and each
// was watched to fail.
import { describe, it, expect } from 'vitest';
import { clipPosition, POSITION_STALE_MS } from './clipposition.js';

const at = (over = {}) => ({
  positionMs: 20_000,
  durationMs: 60_000,
  paused: false,
  known: true,
  seenAt: 1_000_000,
  now: 1_000_000,
  ...over,
});

describe('clipPosition — where the clip is, between the beats', () => {
  it('is the reported position at the instant it was reported', () => {
    expect(clipPosition(at())).toBe(20_000);
  });

  it('carries on from there while nothing new arrives', () => {
    // THE WHOLE POINT. Without this the handle sits still for two seconds and
    // then jumps, which reads as a stuttering clip rather than a slow desk.
    expect(clipPosition(at({ now: 1_000_900 }))).toBe(20_900);
    expect(clipPosition(at({ now: 1_001_750 }))).toBe(21_750);
  });

  it('does not advance a clip the operator is holding', () => {
    // A held clip's position is a FACT, not a rate. Adding wall clock to it
    // would walk the handle across a picture that is standing still.
    expect(clipPosition(at({ paused: true, now: 1_003_000 }))).toBe(20_000);
  });

  it('never runs past the end of the clip', () => {
    // A clip that ended while the desk was not looking must not report a
    // position beyond its own length — the bar would overflow and the figure
    // beside it would count into negative time remaining.
    // The age here is deliberately INSIDE the stale window. Written with a nine
    // second gap first, this case went straight to the refusal below and never
    // reached the clamp it was about — a test passing through the wrong branch,
    // which is the shape this repository files against its own instruments.
    expect(clipPosition(at({ positionMs: 57_000, now: 1_005_000 }))).toBe(60_000);
  });

  it('stops extrapolating once the beat is too old to trust', () => {
    // NOT A ZERO AND NOT A GUESS — nothing. A screen that has stopped beating
    // may have stopped playing, been unplugged or gone to sleep, and the desk
    // cannot tell which. `describeMediaClock` already answers `known: false` in
    // words rather than a dash; this is the same refusal one layer down.
    expect(clipPosition(at({ now: 1_000_000 + POSITION_STALE_MS + 1 }))).toBeNull();
  });

  it('the staleness it uses is the one the backend already enforces', () => {
    // 6500 ms is `channels::BEAT_STALE_MS` — three beats and a quarter. A second
    // figure here would be a second opinion about the same silence, and the two
    // would drift the first time either moved.
    expect(POSITION_STALE_MS).toBe(6500);
  });

  it('answers nothing at all when no screen is reporting a clip', () => {
    expect(clipPosition(at({ known: false }))).toBeNull();
    expect(clipPosition(at({ positionMs: null }))).toBeNull();
    expect(clipPosition(at({ durationMs: null }))).toBeNull();
  });

  it('is not fooled by a clock that goes backwards', () => {
    // A machine that resyncs its clock mid-service hands this a negative
    // elapsed. Counting backwards from a real reading is worse than not
    // counting: the bar would retreat while the clip advanced.
    expect(clipPosition(at({ now: 999_000 }))).toBe(20_000);
  });
});
