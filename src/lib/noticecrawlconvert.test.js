// ── A SCROLLING REGION ROW IS A BAND CRAWL, AND THE CONVERTER SAID OTHERWISE ──
//
// `style.scroll` on a REGION-model template renders as a ProPresenter footer
// ticker: a band pinned to the very bottom of the screen, an optional fixed label
// on the left, the body crawling right-to-left at a constant reading speed. It
// never occupies the centre of the wall — `TemplateRender`'s own comment says so
// at the markup, and the branch is taken before the band check, so it is true of
// a lower third as well.
//
// `regionsToLayers` converted that same row into `mkVerse(20, 54)`: a full-frame
// block in the middle of the screen with `scroll: true` on it. So ONE template
// rendered as two different things depending on which path a surface took — a
// band ticker on the region path, a full-frame runner on the converted path — and
// `TemplateGallery.upgradeLegacyToLayers` runs the conversion on mount and SAVES
// it. A church's announcement template would have been permanently re-shaped, in
// silence, by opening the Templates tab.
//
// THE DEFECT IS IN THE CONVERTER, NOT IN THE RENDERING. The rendering half (a
// stopped crawl measured as though it were fitted text) is `crawls(L)` on
// `fix/countdown-rendering`. This is the other half.
//
//   npx vitest run src/lib/noticecrawlconvert.test.js
import { describe, it, expect } from 'vitest';
import { regionsToLayers } from './layers.js';

/** A notice template as the region model expresses one. */
const notice = (over = {}) => ({
  id: 3,
  name: 'Scroll · Banner',
  layout: { regions: ['verse_text', 'reference'], ...(over.layout ?? {}) },
  style: { scroll: true, background: '#101319', accent: '#ffb000', ...(over.style ?? {}) },
});

const layersOf = (t) => regionsToLayers(t).layers;
const verse = (ls) => ls.find((L) => L.bind === 'verse');
const reference = (ls) => ls.find((L) => L.bind === 'reference');

describe('a scrolling region template converts to a band crawl', () => {
  it('puts the crawl in a band at the bottom, never across the middle', () => {
    const v = verse(layersOf(notice()));
    expect(v, 'the converted template has no verse layer at all').toBeTruthy();
    expect(v.scroll, 'the crawl stopped being a crawl on the converted path').toBe(true);
    // THE BAND IS THE CLAIM. `mkVerse(20, 54)` is the defect, stated as numbers:
    // a block starting a fifth of the way down and 54% of the frame tall is the
    // centre of the wall, which is the one place the region path guarantees a
    // notice never goes.
    expect(
      v.y + v.h,
      'the crawl does not reach the bottom of the frame — a footer ticker that ' +
        'floats is not a footer ticker',
    ).toBe(100);
    expect(
      v.h,
      'the crawl is taller than a band: on the region path it is one line of ' +
        'type plus padding, and a 54%-tall box is a full-frame runner',
    ).toBeLessThanOrEqual(20);
  });

  it('keeps the reference as a FIXED label beside the crawl, not inside it', () => {
    const ls = layersOf(notice());
    const r = reference(ls);
    const v = verse(ls);
    expect(r, 'the reference vanished in conversion').toBeTruthy();
    expect(r.scroll, 'the label crawled — on the region path it is fixed on the left').toBe(false);
    expect(r.y, 'the label is not in the same band as the crawl it labels').toBe(v.y);
    expect(r.h).toBe(v.h);
    expect(r.x, 'the label is not to the LEFT of the crawl').toBeLessThan(v.x);
    expect(
      r.x + r.w,
      'the label and the crawl overlap — the region path gives each its own box',
    ).toBeLessThanOrEqual(v.x);
  });

  it('gives the crawl the whole band when the reference is turned off', () => {
    const v = verse(layersOf(notice({ layout: { regions: ['verse_text'] } })));
    expect(reference(layersOf(notice({ layout: { regions: ['verse_text'] } })))).toBeUndefined();
    expect(v.w, 'the crawl kept a gap for a label that is not there').toBeGreaterThan(90);
  });

  it('paints the bar the ticker is, so a keyed template still shows one', () => {
    // On the region path the ticker carries its own `background:{tickerBg}` —
    // the template's background, else the accent, else a dark scrim, and NEVER
    // transparent, because the bar IS the visible ticker on a lower-third
    // channel where it is composited over a camera. A conversion that dropped it
    // would leave a keyed notice crawling over nothing.
    const bar = layersOf(notice({ style: { background: 'transparent' } })).find(
      (L) => L.type === 'shape',
    );
    expect(bar, 'the converted crawl has no bar behind it').toBeTruthy();
    expect(bar.fill).toBe('#ffb000');
    expect(bar.y + bar.h).toBe(100);
  });

  it('a lower third that scrolls converts to the crawl and not to a band', () => {
    // The region path takes its `scroll` branch BEFORE it asks about
    // `lowerThird`, so a band template that scrolls renders as the ticker and
    // nothing else. A converter that emitted both would paint two bands.
    const ls = layersOf(notice({ layout: { lowerThird: true } }));
    const shapes = ls.filter((L) => L.type === 'shape');
    expect(shapes.length, 'a band shape AND a ticker bar — two bands').toBe(1);
    expect(
      ls.some((L) => L.type === 'background'),
      'a keyed template gained an opaque background in conversion — the camera ' +
        'it keys over is covered',
    ).toBe(false);
  });

  it('a template that does NOT scroll is converted exactly as before', () => {
    // The whole rest of the shelf goes through this function. A regression here
    // re-shapes every template a church owns, once, on the next visit to the
    // Templates tab.
    const ls = layersOf(notice({ style: { scroll: false } }));
    const v = verse(ls);
    expect(v.y).toBe(20);
    expect(v.h).toBe(54);
    expect(v.scroll).toBe(false);
    expect(reference(ls).y).toBe(76);
  });
});
