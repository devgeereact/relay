// Diagnostic: the wall must SWAP to a new verse when `content` changes, leaving
// no stale slide behind. This reproduces the "Program screen frozen on the first
// fired verse" report against the real TemplateRender.

import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import TemplateRender from './TemplateRender.svelte';

let host;
let app;
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

const scripture = {
  id: 1,
  name: 'T',
  layout: { regions: ['verse_text', 'reference'], align: 'center' },
  style: { verseColor: '#ffffff', accent: '#ffb000', background: '#101010' },
};
const A = { reference: 'Psalms 23:1', text: 'The LORD is my shepherd' };
const B = { reference: 'Revelation 1:1', text: 'The revelation of Jesus Christ' };
const allVerses = (el) => [...el.querySelectorAll('.verse')].map((v) => v.textContent).join(' ');

describe('content swap (region template)', () => {
  it('swaps the rendered verse and leaves no stale slide', async () => {
    const el = mount(scripture, A);
    expect(allVerses(el)).toMatch(/shepherd/i);
    app.$set({ content: B });
    await tick();
    const verses = allVerses(el);
    expect(verses).toMatch(/revelation of Jesus/i);
    expect(verses).not.toMatch(/shepherd/i);
  });

  it('survives a rapid A→B→C without sticking', async () => {
    const C = { reference: 'John 3:16', text: 'For God so loved the world' };
    const el = mount(scripture, A);
    app.$set({ content: B });
    await tick();
    app.$set({ content: C });
    await tick();
    const verses = allVerses(el);
    expect(verses).toMatch(/so loved the world/i);
    expect(verses).not.toMatch(/shepherd/i);
    expect(verses).not.toMatch(/revelation/i);
  });
});

import { makeLayer } from './layers.js';
const layered = {
  id: 2,
  name: 'L',
  layout: {
    align: 'center',
    layers: [
      makeLayer('background', { fill: '#101010' }),
      makeLayer('text', { name: 'Verse', bind: 'verse', x: 8, y: 34, w: 84, h: 40, size: 5, color: '#fff' }),
      makeLayer('text', { name: 'Ref', bind: 'reference', x: 8, y: 74, w: 84, h: 10, size: 2.5, color: '#f0b74a' }),
    ],
  },
  style: {},
};
describe('content swap (LAYERED template)', () => {
  it('swaps the bound verse text when content changes', async () => {
    const el = mount(layered, A);
    await tick();
    expect(el.textContent).toMatch(/shepherd/i);
    app.$set({ content: B });
    await tick();
    expect(el.textContent).toMatch(/revelation of Jesus/i);
    expect(el.textContent).not.toMatch(/shepherd/i);
  });
});

describe('clear removes EVERYTHING (content → null)', () => {
  it('region: background and slide are gone when content clears', async () => {
    const el = mount(scripture, A);
    expect(el.querySelector('.bglayer')).toBeTruthy();
    expect(el.querySelector('.slide')).toBeTruthy();
    app.$set({ content: null });
    await tick();
    expect(el.querySelector('.bglayer')).toBeNull();
    expect(el.querySelector('.slide')).toBeNull();
    expect(el.textContent.trim()).toBe('');
  });

  it('layered: background, band shape and text are all gone when content clears', async () => {
    const band = {
      id: 3, name: 'LT',
      layout: { align: 'left', layers: [
        makeLayer('shape', { name: 'Band', x: 6, y: 74, w: 88, h: 18, fill: '#101319' }),
        makeLayer('text', { name: 'Verse', bind: 'verse', x: 9, y: 76, w: 82, h: 10, size: 2.6, color: '#fff' }),
      ] },
      style: {},
    };
    const el = mount(band, A);
    await tick();
    expect(el.querySelector('.lshape')).toBeTruthy();
    expect(el.textContent).toMatch(/shepherd/i);
    app.$set({ content: null });
    await tick();
    expect(el.querySelector('.lshape')).toBeNull();
    expect(el.querySelector('.ltext')).toBeNull();
    expect(el.textContent.trim()).toBe('');
  });
});

import { isKeyedTemplate } from './layers.js';
describe('isKeyedTemplate (blackout must not black a keyed channel)', () => {
  it('a layer-model lower third (band shape, no full background) is keyed', () => {
    const lt = { layout: { layers: [
      makeLayer('shape', { x: 6, y: 74, w: 88, h: 18, fill: '#101319' }),
      makeLayer('text', { bind: 'verse' }),
    ] } };
    expect(isKeyedTemplate(lt)).toBe(true);
  });
  it('a full-screen layered template with a background layer is NOT keyed', () => {
    const full = { layout: { layers: [
      makeLayer('background', { fill: '#201010' }),
      makeLayer('text', { bind: 'verse' }),
    ] } };
    expect(isKeyedTemplate(full)).toBe(false);
  });
  it('region: lowerThird is keyed; an opaque background is not', () => {
    expect(isKeyedTemplate({ layout: { lowerThird: true }, style: {} })).toBe(true);
    expect(isKeyedTemplate({ layout: {}, style: { background: '#101010' } })).toBe(false);
    expect(isKeyedTemplate({ layout: {}, style: { background: 'transparent' } })).toBe(true);
  });
});

import { resolveOutputTemplate } from './layers.js';
describe('resolveOutputTemplate (per-screen template is authoritative; cue choice pins)', () => {
  const keyed = { layout: { layers: [makeLayer('shape', { fill: '#101319' }), makeLayer('text', { bind: 'verse' })] } };
  const opaque = { layout: { layers: [makeLayer('background', { fill: '#201010' }), makeLayer('text', { bind: 'verse' })] } };
  it('no override → channel template', () => {
    expect(resolveOutputTemplate(keyed, null)).toBe(keyed);
  });
  it('keyed channel + opaque override → keeps its keyed template (camera stays), even if pinned', () => {
    expect(resolveOutputTemplate(keyed, opaque)).toBe(keyed);
    expect(resolveOutputTemplate(keyed, opaque, true)).toBe(keyed); // transparency law wins over pin
  });
  it('opaque channel + content-look default (not pinned) → the SCREEN keeps its own template', () => {
    // The fix: an operator who set this screen's template sees it, not a content
    // default that silently replaces it.
    expect(resolveOutputTemplate(opaque, opaque)).toBe(opaque); // channelTpl, which is `opaque` here
    const otherLook = { layout: { layers: [makeLayer('background', { fill: '#0a0a0a' }), makeLayer('text', { bind: 'verse' })] } };
    expect(resolveOutputTemplate(opaque, otherLook, false)).toBe(opaque); // screen wins over the look
  });
  it('a PINNED cue choice overrides an opaque screen (the operator picked it for that cue)', () => {
    const cueTpl = { layout: { layers: [makeLayer('background', { fill: '#123456' }), makeLayer('text', { bind: 'verse' })] } };
    expect(resolveOutputTemplate(opaque, cueTpl, true)).toBe(cueTpl);
  });
});

describe('media fills the frame ALONE (no band / bg / text)', () => {
  const IMG = { media_url: 'blob:x', media_kind: 'image', reference: '', text: '' };
  it('layered WITHOUT a media layer shows media full-frame by default (main-screen case)', async () => {
    const band = { id: 9, name: 'LT', layout: { align: 'left', layers: [
      makeLayer('shape', { name: 'Band', x: 6, y: 74, w: 88, h: 18, fill: '#101319' }),
      makeLayer('text', { name: 'Verse', bind: 'verse' }),
    ] }, style: {} };
    const el = mount(band, IMG);
    await tick();
    // No media layer → the picture fills the frame on top (foreground).
    expect(el.querySelector('img.media')).toBeTruthy();
  });
  it('region template: only the media, no background or verse', async () => {
    const el = mount(scripture, IMG);
    await tick();
    expect(el.querySelector('img.media')).toBeTruthy();
    expect(el.querySelector('.bglayer')).toBeNull();
    expect(el.querySelector('.slide')).toBeNull();
  });
  it('video media renders a video element', async () => {
    const el = mount(scripture, { media_url: 'blob:v', media_kind: 'video', reference: '', text: '' });
    await tick();
    expect(el.querySelector('video.media')).toBeTruthy();
  });
});

describe('MEDIA LAYER — placement + per-screen opt-out', () => {
  const IMG = { media_url: 'blob:x', media_kind: 'image', reference: 'John 3:16', text: 'For God so loved' };
  const withMedia = { id: 20, layout: { layers: [
    makeLayer('background', { fill: '#101010' }),
    makeLayer('text', { bind: 'verse' }),
    makeLayer('media', { name: 'Media' }),
  ] }, style: {} };
  const noMediaScreen = { id: 21, layout: { noMedia: true, layers: [
    makeLayer('shape', { fill: '#101319' }),
    makeLayer('text', { bind: 'verse' }),
  ] }, style: {} };

  it('a MEDIA layer places media in its own box (not full-frame)', async () => {
    const el = mount(withMedia, IMG);
    await tick();
    expect(el.querySelector('img.lmediafill')).toBeTruthy(); // placed
    expect(el.querySelector('img.media')).toBeNull(); // NOT the full-frame path
  });
  it('a screen with noMedia NEVER shows media (opt-out); its text still shows', async () => {
    const el = mount(noMediaScreen, IMG);
    await tick();
    expect(el.querySelector('img.lmediafill')).toBeNull();
    expect(el.querySelector('img.media')).toBeNull();
    expect(el.textContent).toMatch(/so loved/i);
  });
  it('media layer is empty on a text-only cue', async () => {
    const el = mount(withMedia, { reference: 'John 3:16', text: 'For God so loved' });
    await tick();
    expect(el.querySelector('img.lmediafill')).toBeNull();
    expect(el.textContent).toMatch(/so loved/i);
  });
  it('a media layer does not make a keyed template opaque', () => {
    const keyedWithMedia = { layout: { layers: [makeLayer('media', {}), makeLayer('text', { bind: 'verse' })] } };
    expect(isKeyedTemplate(keyedWithMedia)).toBe(true);
  });
  it('clear (content → null) removes full-frame media too', async () => {
    const plain = { id: 22, layout: { layers: [makeLayer('text', { bind: 'verse' })] }, style: {} };
    const el = mount(plain, IMG);
    await tick();
    expect(el.querySelector('img.media')).toBeTruthy();
    app.$set({ content: null });
    await tick();
    expect(el.querySelector('img.media')).toBeNull();
    expect(el.textContent.trim()).toBe('');
  });
});

describe('COUNTDOWN on a layered template', () => {
  const cd = () => ({ reference: 'Service begins in', countdown_to: Date.now() + 300000, countdown_done: 'Welcome' });
  it('shows the MM:SS by default when there is NO timer layer', async () => {
    const scriptureLayered = { id: 30, layout: { layers: [
      makeLayer('background', { fill: '#101010' }),
      makeLayer('text', { name: 'Verse', bind: 'verse' }),
      makeLayer('text', { name: 'Ref', bind: 'reference' }),
    ] }, style: {} };
    const el = mount(scriptureLayered, cd());
    await tick();
    const digits = el.querySelector('.cd-default .countdown');
    expect(digits).toBeTruthy();
    expect(digits.textContent.trim()).toMatch(/^\d+:\d{2}$/); // e.g. 5:00
    // the label shows once (from the default block), not duplicated by the ref layer
    expect(el.textContent).toMatch(/Service begins in/);
  });
  it('a timer layer renders it instead (no default block)', async () => {
    const withTimer = { id: 31, layout: { layers: [
      makeLayer('background', { fill: '#101010' }),
      makeLayer('timer', { name: 'Timer' }),
    ] }, style: {} };
    const el = mount(withTimer, cd());
    await tick();
    expect(el.querySelector('.cd-default')).toBeNull();
    // the timer layer shows the MM:SS
    expect(el.textContent.trim()).toMatch(/\d+:\d{2}/);
  });
});
