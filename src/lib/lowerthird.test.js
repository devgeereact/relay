// A LOWER THIRD IS KEYED OVER A LIVE CAMERA. IT MAY NEVER PAINT.
//
// This channel exists to be composited: OBS or an ATEM keys it over the shot of
// the preacher. Every pixel it fills that it did not need to fill removes the
// preacher from the stream — and nobody in the building can see that happen,
// because the failure is only visible on the broadcast.
//
// So the rules below are enforced in the RENDERER, not per screen:
//   · a band template's own background is IGNORED (an operator picking a
//     background in the Templates editor must not be able to black out a stream)
//   · the band draws only where there are words
//   · a countdown never goes out on a band
//   · media is the one thing allowed to fill the frame, because the operator
//     deliberately chose a full-frame picture
//
//   CLAUDE.md · docs/SPEC.md §5 (channels are render targets of one engine)

import { describe, it, expect, afterEach } from 'vitest';
import TemplateRender from './TemplateRender.svelte';

let host;
let app;

const band = (over = {}) => ({
  id: 9,
  name: 'Stream lower third',
  layout: { lowerThird: true, regions: ['verse_text', 'reference'], ...(over.layout || {}) },
  style: { accent: '#8b5cf6', verseColor: '#1c1224', background: '#ff0000', ...(over.style || {}) },
});
const solid = () => ({
  id: 1,
  name: 'Main',
  layout: { regions: ['reference', 'verse_text'] },
  style: { accent: '#e0a458', background: '#120d08' },
});

/**
 * THE SAME BAND IN THE LAYER MODEL — and the reason `isKeyedTemplate` exists.
 *
 * There is no `lowerThird` flag here and there never can be: the band IS a shape
 * layer, and nothing paints the whole frame, which is exactly what makes the
 * channel keyed. `layers.js::isKeyedTemplate` answers `true`; `layout.lowerThird`
 * answers `false`; and until RG-166 the countdown asked the second one.
 */
const layeredBand = () => ({
  id: 10,
  name: 'Stream lower third (layers)',
  layout: {
    layers: [
      { id: 'b', type: 'shape', visible: true, x: 4, y: 76, w: 92, h: 20, fill: '#0b0f16', opacity: 1 },
      { id: 't', type: 'text', visible: true, bind: 'verse', x: 6, y: 79, w: 88, h: 12, size: 3, color: '#fff' },
    ],
  },
  style: {},
});

/** A layer-model template that DOES paint its own whole frame — the control. */
const layeredSolid = () => ({
  id: 11,
  name: 'Main (layers)',
  layout: {
    layers: [
      { id: 'bg', type: 'background', visible: true, fill: '#120d08', opacity: 1 },
      { id: 't', type: 'text', visible: true, bind: 'verse', x: 8, y: 30, w: 84, h: 40, size: 5, color: '#fff' },
    ],
  },
  style: { accent: '#e0a458' },
});

function mount(template, content) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
});

// jsdom normalises inline styles: `#120d08` → `rgb(18, 13, 8)` and
// `transparent` → `rgba(0, 0, 0, 0)`. Read the computed value and treat both
// spellings of "nothing painted" as transparent.
// The background now lives on its own `.bglayer` (so its opacity can be dimmed
// without touching the text). No layer is rendered when nothing is painted —
// which is exactly the transparent case.
const stageBg = (el) => {
  const layer = el.querySelector('.bglayer');
  return layer ? getComputedStyle(layer).background || '' : 'transparent';
};
const isTransparent = (bg) => /transparent|rgba\(0, 0, 0, 0\)/.test(bg) && !/url\(/.test(bg);
const bandBg = (el) => {
  const c = el.querySelector('.slide.lower-third .content');
  return c ? getComputedStyle(c).background : null;
};

describe('the stage background', () => {
  it('is TRANSPARENT on a band, even though the template asks for red', () => {
    const el = mount(band(), { reference: 'John 3:16', text: 'For God so loved the world' });
    expect(isTransparent(stageBg(el))).toBe(true);
    expect(stageBg(el)).not.toMatch(/rgb\(255, 0, 0\)|#ff0000/i);
  });

  it('is transparent on a band with a background IMAGE set, too', () => {
    const el = mount(band({ style: { bgImage: 'data:image/png;base64,AAA' } }), { text: 'words' });
    expect(isTransparent(stageBg(el))).toBe(true);
  });

  it('still paints on an ordinary channel — this rule is for bands only', () => {
    const el = mount(solid(), { reference: 'John 3:16', text: 'For God so loved' });
    expect(stageBg(el)).toMatch(/rgb\(18, 13, 8\)|#120d08/i);
  });
});

describe('the band itself', () => {
  it('is drawn when there are words', () => {
    const el = mount(band(), { text: 'Blessed assurance, Jesus is mine' });
    expect(el.querySelector('.slide.lower-third')).toBeTruthy();
    expect(el.querySelector('.slide.bandless')).toBeNull();
  });

  it('IS NOT DRAWN over a picture — the media fills the frame alone', () => {
    // The bug: a coloured strip across the bottom of someone's photo, for no
    // reason, on the stream. A fired picture/video now fills the frame ALONE —
    // no band, no slide, no text — so nothing can sit over it.
    const el = mount(band(), { media_url: 'http://x/media/1', media_kind: 'image' });
    expect(el.querySelector('img.media')).toBeTruthy();
    expect(el.querySelector('.slide')).toBeNull();
  });

  it('lets a fired picture fill the frame', () => {
    const el = mount(band(), { media_url: 'http://x/media/1', media_kind: 'image' });
    expect(el.querySelector('img.media')).toBeTruthy();
  });

  it('plays a fired video', () => {
    const el = mount(band(), { media_url: 'http://x/media/2', media_kind: 'video' });
    expect(el.querySelector('video.media')).toBeTruthy();
  });
});

describe('the countdown', () => {
  const soon = () => Date.now() + 5 * 60_000;

  it('NEVER goes out on a band', () => {
    const el = mount(band(), { reference: 'Service begins in', countdown_to: soon() });
    expect(el.querySelector('.countdown')).toBeNull();
  });

  it('still shows on every other channel', () => {
    const el = mount(solid(), { reference: 'Service begins in', countdown_to: soon() });
    expect(el.querySelector('.countdown')).toBeTruthy();
  });

  // ── RG-166 · THE SECOND DOOR, WHICH THE TWO CASES ABOVE COULD NOT REACH ────
  //
  // Both of those mount REGION-model templates. `band()` sets
  // `layout: { lowerThird: true }`, which is the flag a region-model band has and
  // a LAYER-model one does not — its band is a shape layer. So the guarantee at
  // the top of this file was kept on one of the two render paths for as long as
  // the layer model has existed, and `TemplateRender`'s layer branch asked
  // nothing at all: `.cd-default` is `position:absolute; inset:0`, so a fired
  // countdown painted a full-frame clock straight over the camera.
  //
  // The whole shelf is enumerated in `keyedcountdown.test.js`. These two are here
  // because this is the file that states the rule, and a rule stated in one file
  // and tested in another is how the first door came to be the only one checked.
  it('never goes out on a LAYER-model lower third either', () => {
    const el = mount(layeredBand(), { reference: 'Service begins in', countdown_to: soon() });
    expect(el.querySelector('.cd-default'), 'the default overlay painted over the camera').toBeNull();
    expect(el.textContent, 'a clock reached a keyed screen').not.toMatch(/\d+:\d\d/);
  });

  it('and an OPAQUE layered template still paints the digits', () => {
    // The half that stops the fix above from being "layered templates lost their
    // countdown". A blanket refusal would satisfy the case above and blank the
    // pre-service screen of every church on a layer-model template.
    const el = mount(layeredSolid(), { reference: 'Service begins in', countdown_to: soon() });
    expect(el.querySelector('.cd-default'), 'an opaque layered template lost its countdown').toBeTruthy();
    expect(el.textContent).toMatch(/\d+:\d\d/);
  });
});
