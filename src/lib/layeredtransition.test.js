// THE TRANSITION THAT ONLY HALF THE RENDERER HAD.
//
// Operator report: "transition too not rendering as in the artifacts."
//
// `TemplateRender` has two render paths. `{#if layered}` draws a layer stack;
// the `{:else}` below it draws the legacy region model. `transitionMode` and
// `transitionMs` are computed once, for both — and `{#key slideKey}` with
// `in:slideIn` existed in the REGION branch only. So every layered template
// changed slide with a hard cut whatever its style, its theme or the operator's
// live override said, and layered is what everything new is: all eight shelf
// starters, the three lower thirds, SuperSource, both stage looks, and every
// legacy template the gallery has converted (which is all of them — see
// `nameband.test.js` for the database read).
//
// `cardfit.test.js` recorded the gap as a fact while ruling out a different
// theory: "`{#key slideKey}` and `in:slideIn` wrap ONLY the region branch's
// `.slide`. The layered branch … has no transition on it at all." That comment
// is now out of date and says so.
//
// ── HOW THIS IS MEASURED ───────────────────────────────────────────────────
// jsdom does no layout and paints nothing, but Svelte's css transitions are
// still observable: `create_rule` writes a generated keyframe name into the
// node's own `style.animation`. So "is a transition running on this element" is
// a real question here, and these tests ask it of the DOM rather than of the
// source text.
//
// ── THE THREE THINGS THIS MUST NOT BREAK ───────────────────────────────────
//  1. A countdown ticks four times a second and `slideKey` deliberately excludes
//     `now`. A timer layer that re-keyed per tick would fade once a quarter
//     second, for ever.
//  2. Reduced motion is a CUT, and so is `cut` itself — not a fast animation.
//  3. Furniture is not the slide. The region path keeps `bglayer` and the full
//     media element OUTSIDE its key; the layered path keeps `background` and
//     `media` layers outside this one, so a fire over a looping background video
//     does not restart the video.
import { describe, it, expect, afterEach, beforeEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { STARTERS, makeLayer } from './layers.js';
import { liveTransition } from './transitions.js';

const settle = (ms = 0) => new Promise((r) => setTimeout(r, ms));

const starter = (key) => STARTERS.find((x) => x.key === key).make();

/** A layered template, at a named transition. */
const layered = (key, style = {}) => {
  const s = starter(key);
  return { id: 50, name: key, layout: s.layout, style: { ...s.style, ...style } };
};

const XFADE = { transition: 'crossfade', transitionMs: 320 };

/** The legacy region model, for the control — this path always animated. */
const region = (style = {}) => ({
  id: 1,
  name: 'region',
  layout: { regions: ['reference', 'verse_text'] },
  style: { ...XFADE, ...style },
});

/** Every element that is currently running a Svelte css transition. */
const animating = (root) =>
  [...root.querySelectorAll('*')].filter((el) => el.style && el.style.animation);

describe('a layered template changes slide with the transition it was given', () => {
  let app = null;
  let target = null;
  beforeEach(() => {
    liveTransition.set(null); // follow the template
    target = document.createElement('div');
    document.body.appendChild(target);
  });
  afterEach(() => {
    app?.$destroy?.();
    app = null;
    document.body.innerHTML = '';
    liveTransition.set(null);
  });

  const mount = (template, content) => {
    app = new TemplateRender({ target, props: { template, content } });
    return app;
  };

  it('the region path animates — the control that proves the harness sees one', async () => {
    mount(region(), { reference: 'A 1:1', text: 'one' });
    await settle(30);
    app.$set({ content: { reference: 'B 2:2', text: 'two' } });
    await settle();
    expect(target.querySelector('.slide').style.animation).toBeTruthy();
  });

  it('animates the words of a layered template on a new slide', async () => {
    // THE BUG. Before the fix nothing in this subtree animates at all.
    mount(layered('lower.name', XFADE), { reference: 'Guest Speaker', text: 'Pastor Ade' });
    await settle(30);
    app.$set({ content: { reference: 'Worship Leader', text: 'Sister Tolu' } });
    await settle();
    const text = [...target.querySelectorAll('.ltext')];
    expect(text.length, 'the band drew its words').toBeGreaterThan(0);
    expect(text.every((el) => el.style.animation), 'every text layer animates').toBe(true);
  });

  it('animates the band the words sit in, so the bar does not pop in under a fade', async () => {
    mount(layered('lower.name', XFADE), { reference: 'Guest Speaker', text: 'Pastor Ade' });
    await settle(30);
    app.$set({ content: { reference: 'Worship Leader', text: 'Sister Tolu' } });
    await settle();
    expect(target.querySelector('.lband').style.animation).toBeTruthy();
  });

  it('runs it for exactly as long as the template asked', async () => {
    mount(layered('lower.name', { transition: 'crossfade', transitionMs: 500 }), {
      reference: 'r',
      text: 'one',
    });
    await settle(30);
    app.$set({ content: { reference: 'r', text: 'two' } });
    await settle();
    expect(target.querySelector('.ltext').style.animation).toContain('500ms');
  });

  it('cuts when the template says cut — and cuts exactly as the region path does', async () => {
    // A cut is `transitionDuration` returning 0 and `transitionCss('cut', t)`
    // returning no declarations, so Svelte runs a 0ms rule over empty keyframes:
    // instant, and indistinguishable from no transition at all. The claim worth
    // pinning is not "no animation object exists" — the region path creates the
    // same 0ms rule, measured — but that the TWO PATHS AGREE. Asserting the
    // stricter thing would have pinned a difference between them as if it were
    // the rule.
    mount(layered('lower.name', { transition: 'cut', transitionMs: 320 }), {
      reference: 'r',
      text: 'one',
    });
    await settle(30);
    app.$set({ content: { reference: 'r', text: 'two' } });
    await settle();
    for (const el of animating(target)) expect(el.style.animation).toContain('0ms');

    app.$destroy();
    document.body.innerHTML = '';
    target = document.createElement('div');
    document.body.appendChild(target);
    mount(region({ transition: 'cut', transitionMs: 320 }), { reference: 'r', text: 'one' });
    await settle(30);
    app.$set({ content: { reference: 'r', text: 'two' } });
    await settle();
    expect(target.querySelector('.slide').style.animation).toContain('0ms');
  });

  it('takes the motion away on reduced motion, on this path too', async () => {
    // `transitionDuration` returns 0 for a viewer who asked for no motion, and
    // that value reaches this path through the same `transitionMs` the region
    // path reads — so there is no second place for reduced motion to be honoured
    // or forgotten. The renderer reads `matchMedia` once at module scope, which
    // jsdom does not implement, so the DURATION is asserted at its source here
    // and the wiring by the fact that `transitionMs` is what `slideIn` is given.
    const { transitionDuration } = await import('./transitions.js');
    expect(transitionDuration('crossfade', 320, true)).toBe(0);
    const RENDER = readFileSync(resolve(__dirname, 'TemplateRender.svelte'), 'utf8');
    const uses = RENDER.match(/in:slideIn=\{\{ mode: transitionMode, duration: transitionMs \}\}/g) ?? [];
    expect(uses.length, 'every slideIn is driven by the one resolved pair').toBe(4);
  });

  it("follows the operator's live override over the template (DECISIONS §84)", async () => {
    // One mechanism, one resolution. The layered path must read the SAME
    // `resolveTransition(style, activeOverride)` the region path reads, or the
    // console preview and the wall can disagree about what is in force.
    mount(layered('lower.name', { transition: 'cut', transitionMs: 320 }), {
      reference: 'r',
      text: 'one',
    });
    await settle(30);
    liveTransition.set({ mode: 'crossfade', ms: 200 });
    await settle(30);
    app.$set({ content: { reference: 'r', text: 'two' } });
    await settle();
    expect(target.querySelector('.ltext').style.animation).toContain('200ms');
  });
});

describe('what the transition may never do', () => {
  let app = null;
  let target = null;
  beforeEach(() => {
    liveTransition.set(null);
    target = document.createElement('div');
    document.body.appendChild(target);
  });
  afterEach(() => {
    app?.$destroy?.();
    app = null;
    document.body.innerHTML = '';
    liveTransition.set(null);
  });

  it('does not re-animate a countdown four times a second', async () => {
    // `slideKey` excludes `now` on purpose. A timer layer inside the key must
    // therefore sit still while the figure it draws changes underneath it.
    const t = layered('timer', XFADE);
    app = new TemplateRender({
      target,
      props: { template: t, content: { countdown_to: Date.now() + 600000, text: '', reference: '' } },
    });
    await settle(30);
    // Long enough for several 250ms ticks.
    await settle(700);
    expect(animating(target)).toHaveLength(0);
  });

  it('does not restart a background video when the words change over it', async () => {
    // A media layer is furniture, like `bglayer` on the region path. Rebuilding
    // it would tear the <video> element down and start the loop again, mid-fire,
    // on a congregation screen.
    const template = {
      id: 60,
      name: 'media + words',
      layout: {
        layers: [
          makeLayer('media', { name: 'Backdrop', x: 0, y: 0, w: 100, h: 100 }),
          makeLayer('text', { name: 'Words', bind: 'verse', x: 10, y: 40, w: 80, h: 20, size: 4 }),
        ],
        align: 'center',
      },
      style: { ...XFADE },
    };
    const base = { media_url: 'loop.webm', media_kind: 'video', reference: 'r', text: 'one' };
    app = new TemplateRender({ target, props: { template, content: base } });
    await settle(30);
    const before = target.querySelector('video');
    expect(before, 'the media layer painted').toBeTruthy();
    app.$set({ content: { ...base, text: 'two' } });
    await settle(30);
    expect(target.querySelector('video'), 'the same element, never torn down').toBe(before);
    expect(before.style.animation, 'and it does not animate').toBeFalsy();
  });

  it('leaves the background layer alone', async () => {
    const template = {
      id: 61,
      name: 'bg + words',
      layout: {
        layers: [
          makeLayer('background', { fill: '#0a0a0a' }),
          makeLayer('text', { name: 'Words', bind: 'verse', x: 10, y: 40, w: 80, h: 20, size: 4 }),
        ],
        align: 'center',
      },
      style: { ...XFADE },
    };
    app = new TemplateRender({
      target,
      props: { template, content: { reference: 'r', text: 'one' } },
    });
    await settle(30);
    const bg = target.querySelector('.lbg');
    app.$set({ content: { reference: 'r', text: 'two' } });
    await settle();
    expect(target.querySelector('.lbg'), 'the same element').toBe(bg);
    expect(bg.style.animation).toBeFalsy();
  });
});
