// WHAT THE FIT COSTS, AND WHY THE NUMBER IS THE ONE IT IS.
//
// The operator's report was "the app is running a bit slower now". This file is
// the measurement that answered it, kept because the finding is the kind that
// comes back: the fit is correct, it is gated so it does not run on a clock, and
// it was still the most expensive thing on the console — because of how it was
// INTERLEAVED, not how often it ran.
//
// ── THE UNIT THAT COSTS FRAMES ─────────────────────────────────────────────
//
// `fitLayers` searches for the largest font size that fits each text box. Every
// probe writes a `font-size` and reads `scrollHeight` back, and a geometry read
// taken while a style write is outstanding forces the browser to lay the whole
// page out synchronously before it can answer. The reads AFTER it are free —
// layout is clean again until the next write. So the cost of the fit is not the
// arithmetic and not the number of reads; it is the number of write→read
// TRANSITIONS, and that is what this file counts.
//
// jsdom lays nothing out, so none of this is expensive HERE. The count is the
// claim, not the milliseconds — and the count is the same count a real webview
// would pay.
//
// ── WHAT WAS MEASURED ──────────────────────────────────────────────────────
//
// Searching each layer to completion before starting the next one made the cost
// `rounds × layers`. On the shipped `High Visibility` template (two text layers)
// that was 32 forced layouts for one render, and Live mounts one render per
// slide-grid cell: 1281 forced layouts to open a forty-slide plan, in a single
// frame, before the operator has touched anything.
//
// Writing every layer's next candidate and then reading them all makes one flush
// serve the whole render. Same bisection per layer, same bracket, same answer —
// only the interleaving changes.
//
//   npx vitest run src/lib/fitcost.test.js
//
// CLAUDE.md "Measure before optimising" · rule 37 · rule 42
import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import TemplateRender, {
  FIT_EPS_CQW,
  FIT_FLOOR_CQW,
  FIT_MAX_ROUNDS,
  fitRoundsFor,
} from './TemplateRender.svelte';

// ── The instrument ─────────────────────────────────────────────────────────
// A style write dirties layout; the first geometry read while dirty is one
// forced layout and cleans it again. That is the browser's own rule, modelled
// here so the count can be asserted without a browser.
let dirty = false;
let layouts = 0;
// A TEXT-FLOW MODEL, and it is labelled as one. jsdom lays nothing out, so every
// box reports 0, every candidate size "fits", and the search would take its
// ceiling shortcut every time — which would make a shortcut look free and prove
// nothing about the ladder underneath it. So when `modelling` is on, a `.ltext`
// answers with a crude flow: characters per line from the box width and the font
// size, lines from that, height from lines × line-height. It decides only WHETHER
// a size fits. The count of forced layouts is a property of the SEARCH, not of
// the model — which is why the model can be this crude and the number still mean
// something.
let modelling = false;
const STAGE_W = 1280;
const BOX_W = 240;
const BOX_H = 110;
function modelledHeight(box) {
  const kid = box.querySelector?.('.lfit');
  const cqw = parseFloat(kid?.style?.fontSize) || 0;
  const chars = (kid?.textContent || '').length;
  if (!cqw || !chars) return 0;
  const px = (cqw * STAGE_W) / 100;
  const perLine = Math.max(1, Math.floor(BOX_W / (px * 0.5)));
  return Math.ceil(chars / perLine) * px * 1.25;
}
const GEOMETRY = ['scrollHeight', 'clientHeight', 'scrollWidth', 'clientWidth'];
for (const prop of GEOMETRY) {
  const d = Object.getOwnPropertyDescriptor(Element.prototype, prop);
  Object.defineProperty(Element.prototype, prop, {
    configurable: true,
    get() {
      if (dirty) {
        layouts += 1;
        dirty = false;
      }
      if (modelling && this.classList?.contains('ltext')) {
        if (prop === 'scrollHeight') return modelledHeight(this);
        if (prop === 'clientHeight') return BOX_H;
        if (prop === 'clientWidth') return BOX_W;
        if (prop === 'scrollWidth') return 0;
      }
      return d.get.call(this);
    },
  });
}
// jsdom puts the camelCase style accessors on a prototype BELOW
// `CSSStyleDeclaration` and has moved them before, so find whichever one
// actually owns `fontSize` rather than naming a class that may not have it.
const styleProto = (() => {
  let o = document.createElement('div').style;
  while (o && !Object.getOwnPropertyDescriptor(o, 'fontSize')) o = Object.getPrototypeOf(o);
  return o;
})();
const fontSizeDesc = styleProto && Object.getOwnPropertyDescriptor(styleProto, 'fontSize');
Object.defineProperty(styleProto, 'fontSize', {
  configurable: true,
  enumerable: fontSizeDesc.enumerable,
  get() {
    return fontSizeDesc.get.call(this);
  },
  set(v) {
    dirty = true;
    return fontSizeDesc.set.call(this, v);
  },
});

// A THE-INSTRUMENT-CAN-STILL-SEE-IT TEST. An instrument that has quietly stopped
// watching passes everything, which is how two scanners in this repo came to
// look exhaustive while checking less than they claimed.
it('the instrument still sees a write followed by a read', () => {
  const el = document.createElement('div');
  document.body.appendChild(el);
  layouts = 0;
  dirty = false;
  el.style.fontSize = '10px';
  void el.scrollHeight;
  void el.scrollHeight; // free — layout is clean again
  el.style.fontSize = '11px';
  void el.clientHeight;
  el.remove();
  expect(layouts).toBe(2);
});

const rafQueue = [];
globalThis.requestAnimationFrame = (fn) => rafQueue.push(fn);
globalThis.cancelAnimationFrame = () => {};
function drainFrames(n = 6) {
  for (let i = 0; i < n; i++) for (const fn of rafQueue.splice(0, rafQueue.length)) fn(0);
}

const VERSE =
  'And we know that all things work together for good to them that love God, to them who are the called according to his purpose.';

/** A template with `n` text layers, otherwise identical. */
function withTextLayers(n) {
  const layers = [{ id: 'bg', type: 'background', w: 100, h: 100, fill: '#000' }];
  for (let i = 0; i < n; i++) {
    layers.push({
      id: `t${i}`,
      type: 'text',
      src: i === 0 ? 'verse' : 'reference',
      x: 6,
      y: 6 + i * 4,
      w: 88,
      h: 20,
      size: 6,
    });
  }
  return { layout: { layers }, style: {} };
}

let app;
let host;
function measureMount(template) {
  host = document.createElement('div');
  document.body.appendChild(host);
  layouts = 0;
  dirty = false;
  app = new TemplateRender({
    target: host,
    props: { template, content: { reference: 'Romans 8:28', text: VERSE, translation: 'KJV' } },
  });
  drainFrames();
  return layouts;
}
afterEach(async () => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  rafQueue.length = 0;
  await tick();
});

describe('the fit costs one forced layout per ROUND, not one per LAYER', () => {
  it('prints what a render costs at one, two and four text layers', () => {
    const at = {};
    for (const n of [1, 2, 4]) {
      at[n] = measureMount(withTextLayers(n));
      app.$destroy();
      host.remove();
      app = null;
      host = null;
    }
    console.log(
      `[fit cost] forced layouts per render — 1 text layer: ${at[1]} · 2: ${at[2]} · 4: ${at[4]}`
    );
    // THE CLAIM. Four layers may not cost four times one. The old per-layer
    // search did exactly that (16, 32, 64 on these three), which is what made a
    // forty-cell slide grid over a thousand forced layouts in one frame.
    expect(at[4]).toBeLessThan(at[1] * 2);
  });

  it('a render with four text layers costs no more than one with one', () => {
    // Stronger than the bound above and true of the lockstep search: every layer
    // is probed inside the SAME round, so the round count — and therefore the
    // flush count — does not move at all with the layer count.
    const one = measureMount(withTextLayers(1));
    app.$destroy();
    host.remove();
    app = null;
    host = null;
    const four = measureMount(withTextLayers(4));
    expect(four).toBe(one);
  });
});

describe('MODELLED · a search that actually has to bisect', () => {
  // With a flow model in place the ceiling does NOT fit — a whole verse in a
  // 240×110 thumbnail — so the ladder below the shortcut runs for real. This is
  // the slide-grid cell, which is the case that made the console slow.
  afterEach(() => {
    modelling = false;
  });

  it('prints what a bisecting render costs at one, two and four text layers', () => {
    modelling = true;
    const at = {};
    for (const n of [1, 2, 4]) {
      at[n] = measureMount(withTextLayers(n));
      app.$destroy();
      host.remove();
      app = null;
      host = null;
    }
    console.log(
      `[fit cost · modelled] forced layouts per thumbnail — 1 layer: ${at[1]} · 2: ${at[2]} · 4: ${at[4]}`
    );
    // The whole claim, on the case that pays for it: the flush count is the
    // ROUND count, so it does not move with the layer count.
    expect(at[2]).toBe(at[1]);
    expect(at[4]).toBe(at[1]);
    // …and the round count itself is what `fitRoundsFor` says it should be:
    // one ceiling probe, the bisection, and one `overflowing()` verdict.
    const span = 22 - FIT_FLOOR_CQW;
    expect(at[1]).toBeLessThanOrEqual(fitRoundsFor(span) + 2);
    expect(at[1]).toBeLessThan(FIT_MAX_ROUNDS + 2);
  });

  it('prints what opening a forty-slide plan costs', () => {
    // THE NUMBER THE OPERATOR FEELS. Live's slide grid renders every cell through
    // the real renderer, so a forty-slide plan is forty of these, and they all
    // fit in the same frame. An IntersectionObserver defers the cells that are
    // off screen, so this is the ceiling rather than the typical case — but the
    // ceiling is what a maximised window on a short plan actually pays.
    modelling = true;
    const CELLS = 40;
    const kept = [];
    layouts = 0;
    dirty = false;
    for (let i = 0; i < CELLS; i++) {
      const h = document.createElement('div');
      document.body.appendChild(h);
      kept.push([
        new TemplateRender({
          target: h,
          props: {
            template: withTextLayers(2),
            content: { reference: 'Romans 8:28', text: VERSE, translation: 'KJV' },
          },
        }),
        h,
      ]);
    }
    drainFrames();
    const total = layouts;
    for (const [a, h] of kept) {
      a.$destroy();
      h.remove();
    }
    console.log(
      `[fit cost · modelled] ${CELLS} slide-grid cells mounting: ${total} forced layouts in one frame`
    );
    // It was 1281 before the search was put in lockstep (33 per cell, plus the
    // stage read). Nothing here asserts a budget — a budget test goes stale and
    // then gets weakened — only that the cost is still LINEAR IN CELLS and not
    // in cells × layers.
    expect(total).toBeLessThan(CELLS * (FIT_MAX_ROUNDS + 2));
  });
});

describe('the search is bounded by a size, not by a round count', () => {
  it('stops when the bracket is narrower than anything anyone could see', () => {
    // The old loop ran a flat sixteen rounds whatever the bracket. A 'both'-mode
    // layer brackets 0.4 … 22cqw, and sixteen halvings resolve that to 0.0003cqw
    // — four decimal places of a quantity that is not visible at two.
    const span = 22 - FIT_FLOOR_CQW;
    expect(fitRoundsFor(span)).toBeLessThan(FIT_MAX_ROUNDS);
    console.log(
      `[fit cost] a ${span}cqw bracket needs ${fitRoundsFor(span)} rounds to reach ${FIT_EPS_CQW}cqw, not ${FIT_MAX_ROUNDS}`
    );
  });

  it('a narrow bracket needs fewer rounds than a wide one', () => {
    expect(fitRoundsFor(4.6)).toBeLessThan(fitRoundsFor(21.6));
  });

  it('never exceeds the hard bound, whatever it is handed', () => {
    // Rule 37: text that fits at no size must still TERMINATE, be shrunk and be
    // reported — never retried for ever.
    expect(fitRoundsFor(1e9)).toBe(FIT_MAX_ROUNDS);
    expect(fitRoundsFor(0)).toBe(0);
    expect(fitRoundsFor(-1)).toBe(0);
  });

  it('the epsilon is small enough to be invisible on the biggest wall Relay drives', () => {
    // cqw is a share of the output's WIDTH, so the guarantee is resolution-
    // independent and can be stated in pixels at the top end.
    const pxAt4K = (FIT_EPS_CQW / 100) * 3840;
    expect(pxAt4K).toBeLessThan(1);
    console.log(`[fit cost] ${FIT_EPS_CQW}cqw is ${pxAt4K.toFixed(2)}px of font-size at 3840 wide`);
  });
});

describe('an idle render costs nothing', () => {
  it('a settled render forces no layout on a frame where nothing changed', async () => {
    measureMount(withTextLayers(2));
    await tick();
    drainFrames();
    layouts = 0;
    dirty = false;
    drainFrames();
    await tick();
    // This is what the countdown/clock gating buys, and it is the half of the
    // fit scheduling that was already right: a tick mutates internal state, the
    // signature does not move, and no reflow runs. See `fitSig`.
    expect(layouts).toBe(0);
  });
});
