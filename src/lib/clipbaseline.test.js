// A SCRUB BELONGS TO THE CLIP IT SCRUBBED (RG-271).
//
// The operator, with a screenshot of a clip that had just started and a bar
// already a fifth of the way along: *"The bar already run half way when the
// video is just starting... and also no consistency... operator still see
// something different from whats on the output screens."*
//
// **This is RG-260's own regression and it is worth saying plainly.** That change
// made the console's transport store the frame the screens were sent, which was
// right, and then Live used the frame's `startedAt` unconditionally:
//
//     media_started_at: $mediaTransport.startedAt
//
// `startedAt` is only ever set by a SCRUB, and nothing ever clears it. So after
// one scrub, every clip fired for the rest of the service was previewed against
// the baseline of that scrub — a clip fired fresh was drawn as though it had
// started however long ago the handle had been dropped, while every projector
// in the building drew it from zero.
//
// The rule is a comparison rather than a flag: a scrub matters when it happened
// AFTER the content was fired. Before, it belongs to a clip that is gone.
import { describe, it, expect } from 'vitest';
import { baselineFor, readingIsAboutThisClip } from './clipbaseline.js';

const FIRED = 1_700_000_000_000;

describe('baselineFor — which instant a preview measures from', () => {
  it('is the content’s own, when nothing has been scrubbed', () => {
    expect(baselineFor(FIRED, null)).toBe(FIRED);
  });

  it('is the SCRUB’s, when the handle was dropped after the fire', () => {
    // The operator moved the clip; the preview has to follow it, which is the
    // whole of RG-260's second half.
    expect(baselineFor(FIRED, FIRED + 5_000)).toBe(FIRED + 5_000);
  });

  it('is the content’s, when the scrub belongs to a clip that is gone', () => {
    // THE DEFECT. A scrub from before this clip was fired is a fact about the
    // last one, and using it draws a brand new clip part-way through itself.
    expect(baselineFor(FIRED, FIRED - 42_000)).toBe(FIRED);
  });

  it('says nothing when the content has no clip', () => {
    // A verse has no baseline, and inventing one from a stale scrub would be
    // the same defect with nothing on screen to notice it.
    expect(baselineFor(null, FIRED)).toBeNull();
    expect(baselineFor(undefined, FIRED)).toBeNull();
  });

  it('ignores a scrub figure that is not a number', () => {
    for (const bad of [undefined, null, Number.NaN, 'soon']) {
      expect(baselineFor(FIRED, bad)).toBe(FIRED);
    }
  });
});

describe('and the run surface uses it', () => {
  it('Live measures its preview from the one rule, not the raw frame', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const src = readFileSync(resolve('src/lib/views/Live.svelte'), 'utf8');
    expect(src).toContain("from '../clipbaseline.js'");
    expect(src).toMatch(/baselineFor\(/);
    // AND NOT THE OLD UNCONDITIONAL FORM, which is the line that shipped the bug.
    expect(src, 'the stale scrub is still being applied').not.toMatch(
      /media_started_at:\s*\$mediaTransport\.startedAt/,
    );
  });
});

// ── AND A BEAT ABOUT THE LAST CLIP IS NOT A BEAT ABOUT THIS ONE (RG-271) ────
//
// The other half of *"the bar already run half way when the video is just
// starting"*, and it is separate from the baseline.
//
// A screen reports its clip position once per 2s beat and the console polls on
// its own 2s timer, so for up to about four seconds after a new clip is fired
// the newest reading in `channelHealth` is still ABOUT THE CLIP BEFORE IT. A
// clip that had run to 1:18 therefore drew the new clip's bar near its end, and
// then snapped back to zero when a real reading landed.
//
// `describeMediaClock` cannot tell: a `MediaBeat` carries a position, a
// duration and whether it is paused, and says nothing about WHICH clip. So the
// desk has to hold the question itself — it knows when the clip on the screens
// changed, and a reading older than that change is about something else.
describe('readingIsAboutThisClip — a beat older than the fire is not about it', () => {
  it('accepts a reading taken after the clip went up', () => {
    expect(readingIsAboutThisClip(FIRED + 1, FIRED)).toBe(true);
  });

  it('refuses one taken before it', () => {
    // THE DEFECT: this is the previous clip's last position, arriving up to
    // four seconds after the new one was fired.
    expect(readingIsAboutThisClip(FIRED - 1, FIRED)).toBe(false);
  });

  it('accepts everything when no clip change has been seen', () => {
    // A bar that refused until it had witnessed a change would never draw at
    // all on a console opened mid-clip.
    expect(readingIsAboutThisClip(FIRED, null)).toBe(true);
    expect(readingIsAboutThisClip(FIRED, undefined)).toBe(true);
  });
});
