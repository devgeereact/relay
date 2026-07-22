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
});
