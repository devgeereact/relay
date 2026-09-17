// ── THE STANDING BACKGROUND, at the renderer ────────────────────────────────
//
// Relay had no persistent background. The entire layer stack renders inside
// `{#if content}` and `media_url` was a FIELD ON THE CONTENT, so a verse and a
// picture were mutually exclusive payloads: firing the church's backdrop replaced
// the reading and firing the reading replaced the backdrop.
//
// The Rust side holds the wire form, the retained slot and the panic drop
// (`channels::tests`, `e2e.rs`). This file holds the three things only a real
// layout engine can answer:
//
//   1. a clear removes it — the invariant, written before the rendering work and
//      watched to fail against the pre-fix condition;
//   2. a template with NO backdrop layer renders exactly what it rendered before
//      this existed, because that is the entire opt-in and there is no flag;
//   3. a verse painted over a backdrop leaves the backdrop alone, which is the
//      whole point of the payload.
//
// Rendered against the real `TemplateRender`, never a copy of its rules: the one
// renderer is the WYSIWYG guarantee and a second opinion about what it draws is
// worth nothing.

import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import TemplateRender from './TemplateRender.svelte';
import { makeLayer } from './layers.js';

let host;
let app;
function mount(props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
});

/** A layered template, with or without a backdrop layer. */
function template({ backdrop = false } = {}) {
  const layers = [
    { ...makeLayer('background'), id: 'bg', fill: '#101010' },
    { ...makeLayer('text'), id: 'v', bind: 'verse', x: 6, y: 20, w: 88, h: 50 },
  ];
  // BETWEEN the opaque fill and the words, which is the only place a backdrop is
  // any use: every built-in ships an opaque `background` layer, so one pinned
  // beneath it would never be seen.
  if (backdrop) layers.splice(1, 0, { ...makeLayer('backdrop'), id: 'bd' });
  return { id: 1, name: 'T', layout: { layers }, style: {} };
}

const PICTURE = { media_url: 'http://10.0.0.5:8032/media/3', media_kind: 'image' };
const VERSE = { kind: 'scripture', reference: 'Romans 8:28', text: 'And we know' };

const pictures = (el) => [...el.querySelectorAll('.lmediafill')].map((n) => n.getAttribute('src'));
const words = (el) => el.textContent.replace(/\s+/g, ' ').trim();

describe('the standing background', () => {
  // ── 1. THE INVARIANT ──────────────────────────────────────────────────────
  it('a clear takes the background off the screen', async () => {
    // A clear reaches this component as BOTH props going to null at once, which
    // is what `Output.svelte` does on `output://clear` and `output://black` — and
    // asserting on the null rather than on a command is deliberate: this is the
    // renderer, and its whole job is that nothing survives that.
    const el = mount({ template: template({ backdrop: true }), content: VERSE, backdrop: PICTURE });
    expect(pictures(el)).toEqual([PICTURE.media_url]);

    app.$set({ content: null, backdrop: null });
    await tick();
    expect(
      pictures(el),
      'a clear left the church picture on a congregation screen — the worst class ' +
        'of bug in this product',
    ).toEqual([]);
    expect(words(el), 'and it left the words too').toBe('');
  });

  it('a background with nothing fired over it still goes when the wall is cleared', async () => {
    // The state a clear is MOST likely to be pressed in is not the one above: a
    // church puts the backdrop up before the service and fires nothing for ten
    // minutes. `content` is already null throughout, so the old `{#if content}`
    // could not have been what removed the picture.
    const el = mount({ template: template({ backdrop: true }), content: null, backdrop: PICTURE });
    expect(pictures(el)).toEqual([PICTURE.media_url]);

    app.$set({ backdrop: null });
    await tick();
    expect(pictures(el)).toEqual([]);
  });

  // ── 2. THE OPT-IN IS THE TEMPLATE, AND THERE IS NO FLAG ───────────────────
  it('a template with no backdrop layer renders exactly what it rendered before', async () => {
    const plain = template();
    const el = mount({ template: plain, content: VERSE, backdrop: PICTURE });
    expect(
      pictures(el),
      'a screen whose template has no Backdrop layer painted one anyway — the ' +
        'change is opt-in by template design and this is what that means',
    ).toEqual([]);
    expect(words(el)).toMatch(/And we know/);

    // …and with nothing fired it is still blank, rather than showing the backdrop
    // on the strength of the payload alone.
    app.$set({ content: null });
    await tick();
    expect(words(el)).toBe('');
    expect(pictures(el)).toEqual([]);
  });

  it('a hidden backdrop layer shows nothing, like every other hidden layer', async () => {
    const t = template({ backdrop: true });
    t.layout.layers = t.layout.layers.map((L) => (L.id === 'bd' ? { ...L, visible: false } : L));
    const el = mount({ template: t, content: null, backdrop: PICTURE });
    expect(pictures(el)).toEqual([]);
  });

  // ── 3. IT OUTLIVES THE WORDS PAINTED ON IT ────────────────────────────────
  it('firing a verse over a backdrop leaves the backdrop exactly where it is', async () => {
    const el = mount({ template: template({ backdrop: true }), content: null, backdrop: PICTURE });
    const before = el.querySelector('.lmediafill');
    expect(before).toBeTruthy();

    app.$set({ content: VERSE });
    await tick();
    expect(words(el)).toMatch(/And we know/);
    expect(pictures(el)).toEqual([PICTURE.media_url]);
    // THE SAME ELEMENT, not a new one with the same src. A backdrop rebuilt on
    // every fire refetches the picture and — for a video backdrop — tears the
    // element down and restarts the loop, mid-reading, on a congregation screen.
    // That is why the backdrop branch sits OUTSIDE `{#key slideKey}`.
    expect(el.querySelector('.lmediafill')).toBe(before);
  });

  it('a second verse does not disturb it either', async () => {
    const el = mount({ template: template({ backdrop: true }), content: VERSE, backdrop: PICTURE });
    const before = el.querySelector('.lmediafill');
    app.$set({ content: { kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' } });
    await tick();
    expect(words(el)).toMatch(/For God so loved/);
    expect(el.querySelector('.lmediafill')).toBe(before);
  });

  // ── AND A CLEARED WALL STAYS EMPTY OF FURNITURE ───────────────────────────
  it('with no content, only the backdrop draws — no empty band, no empty shape', async () => {
    const t = template({ backdrop: true });
    t.layout.layers.splice(1, 0, { ...makeLayer('shape'), id: 'sh' });
    t.layout.layers.push({ ...makeLayer('band'), id: 'bn' });

    const withContent = mount({ template: t, content: VERSE, backdrop: PICTURE });
    expect(withContent.querySelectorAll('.lshape, .lband').length).toBeGreaterThan(0);
    app.$destroy();
    host.remove();

    const el = mount({ template: t, content: null, backdrop: PICTURE });
    expect(
      el.querySelectorAll('.lshape, .lband').length,
      'a band and a shape were painted over a wall with nothing on it — furniture ' +
        'on a congregation screen with nothing to say, which is the rule the ' +
        '`{#if content}` block has always kept',
    ).toBe(0);
    expect(pictures(el)).toEqual([PICTURE.media_url]);
  });

  // ── A VIDEO BACKDROP IS SILENT, ALWAYS ────────────────────────────────────
  it('a video backdrop is muted even on the screen that routes audio', async () => {
    const el = mount({
      template: template({ backdrop: true }),
      content: null,
      backdrop: { media_url: 'http://10.0.0.5:8032/media/9', media_kind: 'video' },
      // The native output window, the one wired to the house speakers.
      audio: true,
    });
    const v = el.querySelector('video.lmediafill');
    expect(v).toBeTruthy();
    expect(
      v.muted,
      'a backdrop runs under the whole service; sound from it would play under ' +
        'the sermon. The house speakers belong to the ONE fired video.',
    ).toBe(true);
  });
});
