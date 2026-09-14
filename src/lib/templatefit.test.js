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
import { fitScale, estimateLines, keepShrinking, FIT_STEP, FIT_MIN_SCALE } from './templatemodel.js';

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

// ───────────────────────────────────────────────────────────────────────────
// THE SECOND WAY A VERSE GETS CUT IN HALF: THE LOOP RAN OUT OF ROUNDS.
//
// Rule 42 above is a fit that measured the wrong FACE. This is a fit that never
// finished. Both loops — the estimate in `templatemodel.js` and the measured one
// in `TemplateRender` — stopped after 40 rounds of ×0.95, and 0.95^40 is 0.1285.
// A box needing less than that got the loop's last guess and kept it, still
// overflowing, inside an `overflow: hidden` box: the same sliced verse by a
// different road.
//
// It does not take a long passage. ONE SHORT LINE at a large designed size in a
// shallow box — a band, a stage zone — needs a scale below the old floor, and
// that is an ordinary template. Swept: 2954 of 14175 settings overflowed.

/** Does the block actually fit the box the caller described? */
function fits({ text, size, face, aspect, widthPct, heightPct, lineHeight }) {
  const s = fitScale({ text, size, face, aspect, widthPct, heightPct, lineHeight });
  const sz = size * s;
  const lines = estimateLines({ text, size: sz, face, widthPct });
  return { scale: s, fits: lines * sz * lineHeight <= (100 / aspect) * (heightPct / 100) };
}

describe('§3.4 · the fit stops on an answer, not on a round count', () => {
  it('one short line at a large size in a shallow box still fits', () => {
    // The case the old 40-round bound could not reach: needs ~0.099, floor 0.1285.
    const r = fits({ text: 'For God so loved the world.', size: 14, face: 'serif', aspect: 16 / 2, widthPct: 100, heightPct: 20, lineHeight: 1.8 });
    expect(r.fits).toBe(true);
    expect(r.scale).toBeLessThan(0.1285);
    expect(r.scale).toBeGreaterThan(0);
  });

  it('no template overflows its box across the settings space', () => {
    // Sizes, faces, aspects, safe areas (the box's share of the frame), text
    // widths and line spacings. Content up to a whole long passage — the longest
    // verse in the bundled KJV is 534 characters.
    const texts = [
      'For God so loved the world.',
      'The LORD is my shepherd; I shall not want. He maketh me to lie down in green pastures: he leadeth me beside the still waters.',
      'w'.repeat(534),
      'word '.repeat(300),
    ];
    const aspects = [16 / 9, 21 / 9, 4 / 3, 1, 16 / 2, 9 / 16, 0.25];
    const sizes = [2, 3.5, 6, 9, 14];
    const faces = ['serif', 'sans', 'mono'];
    const widths = [40, 70, 100];
    const heights = [20, 60, 100];
    const lineHeights = [1, 1.32, 1.8];

    const bad = [];
    let checked = 0;
    for (const text of texts)
      for (const aspect of aspects)
        for (const size of sizes)
          for (const face of faces)
            for (const widthPct of widths)
              for (const heightPct of heights)
                for (const lineHeight of lineHeights) {
                  const r = fits({ text, size, face, aspect, widthPct, heightPct, lineHeight });
                  checked++;
                  if (!r.fits || !(r.scale > 0) || r.scale > 1) {
                    bad.push(`${text.length}ch a=${aspect.toFixed(2)} s=${size} ${face} ${widthPct}x${heightPct} lh=${lineHeight} -> ${r.scale}`);
                  }
                }
    // The sweep must be a sweep — a loop that checked nothing would pass here.
    expect(checked).toBeGreaterThan(10000);
    expect(bad.slice(0, 5)).toEqual([]);
  });

  it('shrinks only as far as it must — anything that fitted before lands unchanged', () => {
    // The curve is untouched, so this is the guard against "fixed it by making
    // everything smaller". A verse that needed no shrinking still gets 1.
    expect(fitScale({ text: 'For God so loved the world.', size: 6, aspect: 16 / 9, face: 'serif' })).toBe(1);
    // …and a long one lands on an exact power of the curve, not on something else.
    const s = fitScale({ text: 'word '.repeat(400), size: 6, aspect: 16 / 9, face: 'serif' });
    const rounds = Math.round(Math.log(s) / Math.log(FIT_STEP));
    expect(s).toBeCloseTo(FIT_STEP ** rounds, 10);
  });

  it('never shrinks past the floor, because an invisible verse is not a fit', () => {
    // Rule 37: it shows the verse and REPORTS. The floor is on the arithmetic,
    // not on legibility, and it is never zero.
    const s = fitScale({ text: 'word '.repeat(5000), size: 14, aspect: 16 / 2, face: 'mono', widthPct: 40, heightPct: 20, lineHeight: 1.8 });
    expect(s).toBeGreaterThanOrEqual(FIT_MIN_SCALE);
    expect(s).toBeLessThan(0.1285);
  });

  it('the shared predicate is what stops both loops', () => {
    expect(keepShrinking({ overflowing: false, scale: 1 })).toBe(false);
    expect(keepShrinking({ overflowing: true, scale: 1 })).toBe(true);
    // At the floor it stops even though the box still overflows.
    expect(keepShrinking({ overflowing: true, scale: FIT_MIN_SCALE })).toBe(false);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// THE ASPECT ARGUMENT MUST DO WORK, AND IT MUST DO IT IN THE CALLER'S UNITS.
//
// §3.4: "every nested context passes its REAL aspect — a SuperSource word
// region, a stage reading, a band". An argument that is accepted and then
// contradicted by the other two is an argument that was ignored.
describe('§3.4 · a box that is not 16:9 gets a different answer', () => {
  const TEXT = 'The LORD is my shepherd; I shall not want. He maketh me to lie down in green pastures.';

  it('a band, a stage rail and an ultrawide each land somewhere different', () => {
    // Long enough that every one of them has to shrink something; a short verse
    // fits four ways out of four and would prove nothing.
    const long = TEXT.repeat(6);
    const at = (aspect) => fitScale({ text: long, size: 6, face: 'serif', aspect });
    const band = at(16 / 2);
    const wide = at(21 / 9);
    const frame = at(16 / 9);
    const rail = at(9 / 16);
    // cqw is a share of WIDTH, so the frame is 100 wide by definition and its
    // height is 100/aspect. Shallower frame, less room, more shrinking — and an
    // ultrawide is SHALLOWER than 16:9 in these units, not roomier.
    expect(band).toBeLessThan(wide);
    expect(wide).toBeLessThan(frame);
    expect(frame).toBeLessThan(rail);
    // A tall stage rail has height to spare and needs none at all.
    expect(rail).toBe(1);
    expect(new Set([band, wide, frame, rail]).size).toBe(4);
  });

  it('the shares are read as shares of THAT frame, not of a 16:9 one', () => {
    // The same box described two ways must give the same answer: half the height
    // of a 16:9 frame IS a 16:4.5 box.
    const half = fitScale({ text: TEXT, size: 6, face: 'serif', aspect: 16 / 9, heightPct: 50 });
    const own = fitScale({ text: TEXT, size: 6, face: 'serif', aspect: 16 / 4.5 });
    expect(half).toBe(own);
  });

  it('a narrower text width needs more lines, so it shrinks further', () => {
    const full = fitScale({ text: TEXT, size: 6, face: 'serif', aspect: 16 / 2 });
    const narrow = fitScale({ text: TEXT, size: 6, face: 'serif', aspect: 16 / 2, widthPct: 45 });
    expect(narrow).toBeLessThan(full);
  });

  it('the renderer hands it the CONTAINER aspect and the box share, in the units the sizes are in', () => {
    // `.stage` carries `container-type`, so cqw is a share of the CONTAINER; but
    // the box measured is `.content`, which `.slide`'s 6%/7% padding and the
    // 90%/92% cap make about three quarters of it. Passing the BOX's own aspect
    // with the shares left at 100 describes a container the size of the box, and
    // over-states the room by exactly that ratio.
    const container = { w: 1920, h: 1080 };
    const box = { w: 1487, h: 795 }; // what `.content` measures there

    const wrong = fitScale({ text: TEXT, size: 6, face: 'serif', aspect: box.w / box.h });
    const right = fitScale({
      text: TEXT,
      size: 6,
      face: 'serif',
      aspect: container.w / container.h,
      widthPct: (100 * box.w) / container.w,
      heightPct: (100 * box.h) / container.h,
    });
    // The honest description is never the more generous one.
    expect(right).toBeLessThanOrEqual(wrong);
    // And it describes the same rectangle as stating the box in cqw directly:
    // a frame `boxW` wide and `100*h/W` tall, with the box filling its width.
    const boxWcqw = (100 * box.w) / container.w;
    const boxHcqw = (100 * box.h) / container.w;
    const direct = fitScale({ text: TEXT, size: 6, face: 'serif', aspect: 100 / boxHcqw, widthPct: boxWcqw });
    expect(right).toBeCloseTo(direct, 10);
  });
});
