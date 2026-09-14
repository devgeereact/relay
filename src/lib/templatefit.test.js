// THE FIRST VERSE OF THE SERVICE, CUT IN HALF.
//
// Reproduced on a fresh `output.html` at 1920×1080 with the default Classic Serif
// template, against the real backend: fire Romans 8:28 and the fitted block came
// out 732px tall inside a 583px box with `overflow: hidden` — the first and last
// lines sliced through the middle, on the congregation's screen, and it stayed
// that way until something happened to resize the window.
//
// The cause is that a webfont is only fetched when something first USES it. An
// output page opens with nothing on it, so at mount `document.fonts.status` is
// already `loaded`, the deferred re-fit hook in `TemplateRender` is skipped, and
// the binary search then measures the FIRST verse in the fallback face — which
// wraps into fewer lines and lets a larger size through. The real face lands a
// moment later and the text grows past the box it was fitted to.
//
// Checking `overflowing()` immediately after the fit does NOT catch this: the
// reflow the new face causes has not happened yet when we look. Whether the face
// is available is a fact that IS knowable at that moment, so that is what the
// rule asks.
import { describe, it, expect } from 'vitest';
import { needsRefit } from './TemplateRender.svelte';

describe('a fit knows when it measured the wrong thing', () => {
  it('re-fits when the face it measured is not the face that will be painted', () => {
    // The bug, exactly: nothing overflows YET, and the fit is still wrong.
    expect(needsRefit({ fontReady: false, overflowing: false, tries: 0 })).toBe(true);
  });

  it('does nothing when the font was ready and the text fits', () => {
    // The normal case, and it must stay free: the fit loops force synchronous
    // reflow, and a Library grid mounts a dozen of them at once.
    expect(needsRefit({ fontReady: true, overflowing: false, tries: 0 })).toBe(false);
  });

  it('re-fits when the box overflows for some other reason', () => {
    expect(needsRefit({ fontReady: true, overflowing: true, tries: 0 })).toBe(true);
  });

  it('gives up rather than looping on text that fits at no size', () => {
    // Rule 37: a verse that cannot fit is SHRUNK and REPORTED through `onFit`,
    // not retried forever. Two attempts, then it is somebody else's problem.
    expect(needsRefit({ fontReady: false, overflowing: true, tries: 2, max: 2 })).toBe(false);
    expect(needsRefit({ fontReady: true, overflowing: true, tries: 9, max: 2 })).toBe(false);
  });
});
