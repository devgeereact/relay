// THE CARD THAT SLICED ITS OWN WORDS.
//
// Found by rendering the Templates gallery at 1600x1000 and asking every text box
// whether it is bigger than the hole it is drawn through:
//
//   [...document.querySelectorAll('.ltext')]
//     .filter(e => e.scrollHeight > Math.ceil(e.getBoundingClientRect().height) + 2)
//
// Four cards answered, all of them lower thirds: a 15px box with 39px of content
// inside `overflow: hidden`, so the words were cut through the middle on the one
// surface an operator uses to judge a look BEFORE putting it in front of a
// congregation.
//
// ── WHICH OF THE THREE IT WAS ──────────────────────────────────────────────
// It was not the band's derived geometry failing to shrink with the container:
// `bandLayout` works entirely in percent of the frame, so the drawn boxes scale
// exactly. It was not the card being too small for the type, either — the second
// describe below computes that, and the type needs to give up 10-15%, nowhere
// near rule 37's 45% floor.
//
// It was the third thing, and it is the one nobody looks for: `.lfit` DECLARED NO
// FONT SIZE AT ALL. Colour, family, weight, alignment, transform, line-height,
// letter-spacing, shadow and style were all emitted inline; the one property that
// decides whether the words fit the box was not. The only thing that ever set it
// was the imperative fit (`fitLayers`), which runs in a requestAnimationFrame and
// is deliberately deferred while a card is off screen — so until it lands, a
// layered template paints in the APP'S UI BODY SIZE, `--v-fs-b1: 12px`, inherited
// from `body`. Twelve pixels at line-height 1.32 is 15.8px for ONE line, and the
// band's verse box is 14.8px tall at a gallery card. It overflowed before it
// wrapped, and 125 characters wrap it three times.
//
// The legacy (region-mode) branch of the same component never had this: it emits
// `font-size:{verseSize}cqw` inline and the fit only refines it. Two text paths,
// one of them missing the base size — which is the failure shape this repository
// keeps writing down (`bandLayout`'s own doc comment: "Two text paths is how a
// shadow, a transform or a fit fix lands on one kind of layer and not the other").
//
// ── HOW THIS WAS REPRODUCED, PLAINLY ───────────────────────────────────────
// jsdom does no layout, so nothing here measures a pixel. Two things are asserted
// instead, and together they are the defect:
//   1. the markup the renderer EMITS — which is what decides the painted size
//      before any imperative fit lands, and is exactly the state the four cards
//      were caught in;
//   2. the geometry the renderer COMPUTES — `drawBoxes` + `fitScale`, the repo's
//      own estimator, which "mirrors the DOM fitter step for step".
// The pixel evidence is the lead's rendered pass, quoted above.
import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { regionsToLayers, drawBoxes, boundValue, STARTERS } from './layers.js';
import { fitScale, faceOf } from './templatemodel.js';

const SHELF = JSON.parse(
  readFileSync(resolve(__dirname, '../../src-tauri/data/shelf_templates.json'), 'utf8')
).templates;

/** The gallery's own preview content — `TemplateGallery.SAMPLE`, verbatim. */
const SAMPLE = {
  text: 'The LORD is my shepherd; I shall not want. He maketh me to lie down in green pastures: he leadeth me beside the still waters.',
  reference: 'Psalms 23:1-2 · KJV',
};

/** A gallery card's thumbnail: `grid-template-columns: repeat(auto-fill, minmax(210px, 1fr))`
 *  with `.tg-thumb { aspect-ratio: 16/9 }`. 240px is what a 1600px window gave. */
const THUMB_W = 240;
const THUMB_H = (THUMB_W * 9) / 16;
/** `--v-fs-b1` in `src/app.css`, inherited by anything that declares no size. */
const UI_BODY_PX = 12;
/** `TemplateRender`'s own floor — 45% of the size the designer asked for. */
const MIN_LEGIBLE_SCALE = 0.45;
/** One report per fit pass, plus one per bounded retry (`MAX_REFIT = 2`). */
const MIN_LEGIBLE_REPORTS = 3;

let host;
let app;
function mount(template, content = SAMPLE, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content, ...props } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

/** Every seeded legacy template is converted to layers by the gallery on mount
 *  (`upgradeLegacyToLayers`), so the shape a card renders is the CONVERTED one,
 *  not the one in the seed. Read the real seed rather than inventing a shape. */
function seededLowerThirds() {
  const src = readFileSync(resolve(__dirname, '../../src-tauri/src/db/templates.rs'), 'utf8');
  const out = [];
  for (const m of src.matchAll(/r##"(\{.*?\})"##/gs)) {
    try {
      const o = JSON.parse(m[1]);
      if (o && Array.isArray(o.regions) && o.lowerThird) {
        out.push({ layout: regionsToLayers({ layout: o, style: {} }), style: {} });
      }
    } catch {
      /* a literal that is not JSON is not this test's business */
    }
  }
  return out;
}

/** Every look this file argues about: the shelf's bands, the converted seeds and
 *  the three band starters. All of them reach a card through one renderer. */
function everyLowerThird() {
  const out = SHELF.filter((t) => /Lower Third/.test(t.name)).map((t) => [t.name, t]);
  seededLowerThirds().forEach((t, i) => out.push([`seeded lower third #${i + 1}`, t]));
  for (const s of STARTERS) {
    if (s.key.startsWith('lower.')) out.push([`starter ${s.key}`, s.make()]);
  }
  return out;
}

/** The text layers of a layered template, with the box each is actually DRAWN in
 *  (a band's words are positioned by `bandLayout`, not by their stored box). */
function drawnTextLayers(template) {
  const layers = template.layout.layers;
  const textOf = (L) => boundValue(L, SAMPLE) || '';
  const boxes = drawBoxes(layers, textOf);
  return layers
    .filter((L) => L.type === 'text' && L.visible !== false)
    .map((L) => ({ L, box: boxes.get(L.id) || L, text: textOf(L) }));
}

// ───────────────────────────────────────────────────────────────────────────
describe('a rendered text layer says how big it is', () => {
  // THE FIX, AND THE WHOLE OF IT. A size the renderer declares is a size the
  // browser paints on the first frame; a size only an imperative fit sets is a
  // size that is absent whenever that fit has not run yet — off screen, before
  // the first animation frame, or on an element `{#key text}` has just rebuilt.
  // Declaring it makes the un-fitted state the DESIGNED state, which the second
  // describe below shows fits, instead of the app's 12px UI body text, which
  // does not.
  it.each(everyLowerThird())('%s declares the size its designer asked for', (_name, template) => {
    const el = mount(template);
    const boxes = [...el.querySelectorAll('.ltext')];
    expect(boxes.length, 'a lower third that drew no words').toBeGreaterThan(0);
    for (const box of boxes) {
      const fit = box.querySelector('.lfit');
      const style = fit.getAttribute('style') || '';
      const base = Number(fit.dataset.base);
      expect(base, 'a text layer with no designed size').toBeGreaterThan(0);
      // cqw, never px — a rendered slide scales with its container (REBRAND §0).
      expect(style, `no declared font-size: ${style}`).toMatch(
        new RegExp(`font-size:\\s*${base}cqw`)
      );
    }
  });

  it('every layered look does, not just the ones that were caught', () => {
    // The four cards the probe found are where a 12px default SHOWS, because a
    // band's boxes are the shallowest on the shelf. The omission was general, so
    // the assertion is too — otherwise the next shallow box repeats this.
    for (const t of SHELF) {
      const el = mount(t);
      for (const box of el.querySelectorAll('.ltext')) {
        const fit = box.querySelector('.lfit');
        expect(fit.getAttribute('style') || '', t.name).toMatch(/font-size:\s*[\d.]+cqw/);
      }
      app.$destroy();
      host.remove();
      app = null;
      host = null;
    }
  });

  it('and the legacy path still does, which is where the idea came from', () => {
    // `font-size:{verseSize}cqw` on `.verse` is why the region renderer never had
    // this defect. If it ever loses it, it gains the same one.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    expect(src).toMatch(/class="verse"[^>]*font-size:\{verseSize\}cqw/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
describe('the card is not too small — the diagnosis, computed', () => {
  // This pins the ANSWER TO "which of the three is it", not the fix: it passes
  // before and after, and that is the point. If a later change makes a card
  // genuinely too small for a band, this is what turns red and sends the next
  // reader to rule 37's report instead of to the declared size.
  it.each(everyLowerThird())('%s fits a gallery card at its designed size', (name, template) => {
    for (const { L, box, text } of drawnTextLayers(template)) {
      if (!text) continue;
      const scale = fitScale({
        text,
        size: L.size,
        face: faceOf(L.font),
        aspect: 16 / 9,
        widthPct: box.w,
        heightPct: box.h,
        lineHeight: L.lineHeight || 1.3,
      });
      expect(scale, `${name} · ${L.name} is below rule 37's floor on a card`).toBeGreaterThanOrEqual(
        MIN_LEGIBLE_SCALE
      );
    }
  });

  it('and it fits identically at every size, which is what cqw MEANS', () => {
    // WHY "the card is too small" can never be the answer on its own, and why
    // the same defect had to be checked on the Outputs screen cards, the Planner
    // cue inspector, the Live slide grid and the console programme pane: a
    // template's sizes are cqw, a share of the container's width, and its boxes
    // are percentages of the frame. The RATIO between type and box is therefore
    // identical on a 120px grid cell and on a 1920px wall. Only a size that is
    // not in cqw — the inherited 12px above — can break that, which is why one
    // fix in the one renderer covers all five surfaces.
    const [, lyric] = everyLowerThird().find(([n]) => /Lyric/.test(n));
    const [{ L, box, text }] = drawnTextLayers(lyric).filter((r) => r.text);
    const at = () =>
      fitScale({
        text,
        size: L.size,
        face: faceOf(L.font),
        aspect: 16 / 9,
        widthPct: box.w,
        heightPct: box.h,
        lineHeight: L.lineHeight || 1.3,
      });
    expect(at()).toBe(at());
    // And the renderer has exactly one text path per mode, so there is no second
    // place for a surface to declare a size of its own.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    expect([...src.matchAll(/class="lfit"/g)]).toHaveLength(1);
  });

  it('while the app’s UI body size does not, which is what was being painted', () => {
    // The overflow the probe measured, in arithmetic: 12px inherited type in the
    // shallowest band box on the shelf. One line is already taller than the box.
    const shallowest = everyLowerThird()
      .flatMap(([name, t]) => drawnTextLayers(t).map((r) => ({ name, ...r })))
      .filter((r) => r.text)
      .sort((a, b) => a.box.h - b.box.h)[0];
    const boxPx = (shallowest.box.h * THUMB_H) / 100;
    const oneLinePx = UI_BODY_PX * (shallowest.L.lineHeight || 1.3);
    expect(oneLinePx).toBeGreaterThan(boxPx);
    // And the designed size is a different order of thing from the UI's.
    const designedPx = (shallowest.L.size / 100) * THUMB_W;
    expect(designedPx).toBeLessThan(UI_BODY_PX);
  });
});

// ───────────────────────────────────────────────────────────────────────────
describe('rule 37 has an instrument on the path that ships', () => {
  // `fitText` reported through `onFit` and `fitLayers` did not — so after
  // `TemplateGallery.upgradeLegacyToLayers` converts the shelf, which it does on
  // mount, rule 37's report covered NOTHING a church renders. Live's "may not be
  // readable from the back" line could not fire for any template on the shelf,
  // and it read exactly the same as a look that was working: rule 35.
  const layered = SHELF.find((t) => /Lower Third · Scripture/.test(t.name));

  it('a layered template reports its fit, like a region one always has', async () => {
    let seen = null;
    mount(layered, SAMPLE, { onFit: (f) => (seen = f) });
    // The report is deliberately taken a frame LATER than the fit — see the
    // round-2 block below for why that is the whole point.
    await new Promise((r) => setTimeout(r, 300));
    expect(seen, 'a layered fit reported nothing at all').not.toBeNull();
    expect(typeof seen.scale).toBe('number');
    expect(Number.isFinite(seen.scale)).toBe(true);
    expect(typeof seen.legible).toBe('boolean');
    // jsdom has no layout, so the NUMBER here is meaningless and is deliberately
    // not asserted. What is asserted is that the report exists — which is the
    // half that was missing, and the half a browser cannot tell you is missing.
  });

  it('and a reporter that throws may not take the wall down with it', () => {
    // Same guarantee `safescreen.test.js` holds over the region path. This runs
    // inside a requestAnimationFrame on the page that is ON THE WALL.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    expect(src).toMatch(/if \(onFit\)/);
    const fn = src.slice(src.indexOf('if (onFit)'));
    expect(fn.slice(0, 300)).toMatch(/try \{/);
    expect(fn.slice(0, 300)).toMatch(/catch/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
// ROUND 2 — THE FIT THAT SAID YES TO A BOX IT DOES NOT FIT.
//
// With the size declared, the lead re-rendered and three of the four clipped
// cards were simply OFF SCREEN (tops at 1097, 1280, 1463 in a 1000px viewport) —
// the IntersectionObserver deferral, working as intended; scrolling them in drops
// the count to one. The one that remained was `Nocturne · Lyrics`, fully in view,
// `data-base="8.5"`, computing to exactly 20.6975px = exactly 8.5cqw at a 243.5px
// card. Its box measured 104px and its content 174px, and it stayed that way.
//
// 8.5 IS REACHABLE AS A SEARCH RESULT, which is what makes this the interesting
// case rather than "the fit never ran": from `lo = 0.4, hi = 22` the mids are
// 11.2, then 5.8, then exactly 8.5. So the binary search asked "does 8.5 fit?",
// the DOM said yes, and the painted result is 70px over an `overflow:hidden` box.
// The measurement and the paint disagree.
//
// ── WHY `verifyFit` DID NOT CATCH IT, AND IT IS NOT THE FONT ────────────────
// Rule 42 exists for exactly this shape and has two signals. The first,
// `fittedWithTheRealFont()`, is INERT in the shipped product: Relay bundles no
// webfont at all — `src/app.css` line 11 says so, and there is no `@font-face`,
// no `fonts.googleapis` link and no `.woff` anywhere in `src/`, because every
// feature must work at zero network. An empty FontFaceSet makes
// `document.fonts.check()` answer true for any family, so that guard can never
// say "stale". That is correct rather than broken — with no webfont there is no
// swap — but it leaves `overflowing()` as the ONLY real signal `verifyFit` has.
//
// And `overflowing()` was sampled in the SAME synchronous frame as the fit, which
// is the one moment it cannot see a discrepancy that materialises a frame later.
// Rule 42's own comment says this about the font case; the blindness is general.
// Under `container-type: size` the fit writes `font-size` in `cqw` and reads
// `scrollHeight` back inside one loop, and a container that is still settling, a
// container-query length resolved in a later pass, or a system-font substitution
// all produce the same signature: a measurement that says yes and a paint that
// clips. This repository cannot tell those apart without a browser, and it does
// not have to — all three are invisible to a same-frame sample and visible to a
// next-frame one.
//
// So the sample moves to a later frame, under the SAME `MAX_REFIT` bound. No cap
// is raised, no tolerance widened, no threshold touched.
//
// ── AND WHEN THE BOUND IS SPENT, IT SAYS SO ────────────────────────────────
// `Nocturne · Lyrics` fitted at 8.5 of a designed 8.5 — `scale: 1.0`,
// `legible: true` — over a box showing roughly 60% of its words. `legible` only
// ever answered "did we shrink past 45%?", never "does it actually fit", so the
// most reassuring possible report sat over the worst possible outcome. Rule 35.
describe('a fit that still clips says so', () => {
  const layered = SHELF.find((t) => /Lower Third · Scripture/.test(t.name));

  /** Make the rendered boxes report an overflow jsdom cannot produce on its own.
   *  jsdom does no layout, so every box measures 0 — this drives the component's
   *  REAL logic (`overflowing()`, `needsRefit`, the retry bound, the report) with
   *  the one fact a browser would have supplied. */
  function clipBoxes(el, { scroll = 200, client = 100 } = {}) {
    for (const box of el.querySelectorAll('.ltext')) {
      Object.defineProperty(box, 'scrollHeight', { value: scroll, configurable: true });
      Object.defineProperty(box, 'clientHeight', { value: client, configurable: true });
      Object.defineProperty(box, 'scrollWidth', { value: 0, configurable: true });
      Object.defineProperty(box, 'clientWidth', { value: client, configurable: true });
    }
  }
  const settle = (ms = 600) => new Promise((r) => setTimeout(r, ms));

  it('reports `clipped` when the words do not fit, however small it went', async () => {
    let seen = null;
    const el = mount(layered, SAMPLE, { onFit: (f) => (seen = f) });
    clipBoxes(el);
    await settle();
    expect(seen, 'nothing was reported at all').not.toBeNull();
    expect(seen.clipped, 'a clipped box reported no clip').toBe(true);
  });

  it('and does not cry clip over a box that fits', async () => {
    let seen = null;
    mount(layered, SAMPLE, { onFit: (f) => (seen = f) });
    await settle(300);
    expect(seen).not.toBeNull();
    expect(seen.clipped).toBe(false);
  });

  it('sees an overflow that only appears AFTER the fit frame', async () => {
    // The whole point. A same-frame sample cannot see this; that is what let
    // `Nocturne · Lyrics` settle at its designed size over a box it overflows.
    let seen = null;
    const el = mount(layered, SAMPLE, { onFit: (f) => (seen = f) });
    await new Promise((r) => requestAnimationFrame(r)); // the fit frame passes clean
    clipBoxes(el); // ... and only THEN does the box turn out to be too small
    await settle();
    expect(seen?.clipped, 'the check was still taken in the fit frame').toBe(true);
  });

  it('gives up after the bounded retries rather than re-fitting for ever', async () => {
    // Rule 37: a passage that fits at NO size is shrunk and REPORTED, not
    // retried until the page stops responding. The bound is `MAX_REFIT`, and it
    // is the existing one — this fix does not raise it.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    expect(src).toMatch(/const MAX_REFIT = 2;/);
    let calls = 0;
    const el = mount(layered, SAMPLE, { onFit: () => (calls += 1) });
    clipBoxes(el);
    await settle(900);
    expect(calls, 'a clipped box re-fitted without end').toBeLessThanOrEqual(MIN_LEGIBLE_REPORTS);
    expect(calls).toBeGreaterThan(0);
  });

  it('looks again when the box goes bad AFTER the verdict has settled', async () => {
    // ROUND 3, AND IT IS ONE CAUSE FOR BOTH QUESTIONS.
    //
    // `verifyFit` — and therefore `report` — is reachable ONLY from `runFit`, and
    // `runFit` early-returns whenever `fitSig()` is unchanged. So the check that
    // asks "is what I painted actually fitting?" is gated on the same signature
    // that decides whether to re-fit. Once a verdict has settled, nothing looks
    // again until that signature moves — and it CANNOT move for this defect:
    // `fitSig` is built from the stage's integer clientWidth/clientHeight and
    // each layer's stored `w,h,size` plus its text length, and not one of those
    // changes when the painted content outgrows its box.
    //
    // So a render that fitted cleanly while its pane was still settling reports
    // `clipped: false`, the box then goes over, and the last thing Live was ever
    // told is that everything fits. That is the console the lead measured at
    // 2000x1175 with `Romans 8:28` on air: `.mon.prog` holding 207px of content
    // in a 174px box, the first line sliced through the middle, and
    // `document.body.innerText` carrying neither warning. The instrument was not
    // computing the wrong answer — it had stopped being asked the question.
    //
    // TWO EARLIER THEORIES DIED HERE, both killed by probing before building:
    // the retry budget being reset by a moving signature (it reports fine), and
    // the binary search keeping an answer it never re-measured (it already lands
    // on its floor when the measurement is consistently pessimistic).
    let seen = [];
    const el = mount(layered, SAMPLE, { onFit: (f) => seen.push(f) });
    await settle(120); // a clean verdict is reached and reported
    expect(seen.at(-1)?.clipped, 'it did not settle clean first').toBe(false);
    clipBoxes(el); // ... and only NOW does the box turn out to be too small
    await settle(900);
    expect(
      seen.at(-1)?.clipped,
      'a box that went over after the verdict was never looked at again'
    ).toBe(true);
  });

  it('and stops looking — a re-check is not a poll', async () => {
    // The cost of this check is a forced reflow, which is why the fit is gated in
    // the first place (a countdown ticking at 4 Hz and a Library grid of a dozen
    // renders are both in this component's history). One late re-look per settled
    // verdict, not a loop.
    let calls = 0;
    const el = mount(layered, SAMPLE, { onFit: () => (calls += 1) });
    clipBoxes(el);
    await settle(400);
    const early = calls;
    await settle(1200);
    expect(calls - early, 'the re-check turned into a poll').toBeLessThanOrEqual(1);
  });

  it('a resize re-takes the verdict, even when the fit itself is unchanged', async () => {
    // The one event that says "the geometry moved under you". It already calls
    // `scheduleFit`, and `runFit` already threw it away whenever the rounded
    // signature had not changed — which is exactly a pane settling by less than
    // a pixel, the case this whole round is about.
    const seen = [];
    let fire = null;
    const Real = globalThis.ResizeObserver;
    globalThis.ResizeObserver = class {
      constructor(cb) {
        fire = cb;
      }
      observe() {}
      disconnect() {}
    };
    try {
      const el = mount(layered, SAMPLE, { onFit: (f) => seen.push(f) });
      await settle(400);
      expect(seen.at(-1)?.clipped).toBe(false);
      clipBoxes(el);
      seen.length = 0;
      fire([]); // the pane settles; nothing about the FIT changes
      await settle(600);
      expect(seen.at(-1)?.clipped, 'a resize did not re-take the verdict').toBe(true);
    } finally {
      globalThis.ResizeObserver = Real;
    }
  });

  it('reports even while the signature keeps moving under it', async () => {
    // ROUND 3, AND THE REASON THE CONSOLE WAS SILENT. `report()` sat on the
    // FAILURE branch of the retry budget — it ran only when `needsRefit` returned
    // false, which happens only once `refitTries` reaches `MAX_REFIT`. And
    // `verifyFit` resets that counter to zero whenever the signature differs from
    // the one it last saw. On a still gallery card the signature never moves, the
    // counter reaches two, and it reports — which is why the round-2 tests passed.
    // On LIVE it moves constantly: `afterUpdate(scheduleFit)` runs on every store
    // tick and `fitSig` folds in the stage's pixel width and height and every
    // layer's text length, so a fired verse, a ticking clock layer or a pane
    // settling by one pixel each reset the counter. The budget is never spent,
    // `needsRefit` never returns false, and the box retries in silence for ever.
    //
    // Measured by the lead at 2000x1175 with `Romans 8:28` on air: `.mon.prog`
    // holding a 174px box with 207px of content, the first line sliced through
    // the middle, and `document.body.innerText` containing neither warning.
    //
    // So a settled verdict is now stated when it is known. The retry budget
    // governs RETRYING, which is its job; it never governed reporting, and the
    // one surface an operator watches all service is where that showed.
    let seen = [];
    const el = mount(layered, SAMPLE, { onFit: (f) => seen.push(f) });
    clipBoxes(el);
    // Keep the signature moving the way Live does — each of these changes a text
    // length, which is one of the terms `fitSig` is built from.
    const texts = ['Romans 8:28', 'And we know that all things work together', 'For God so loved'];
    for (let i = 0; i < 6; i++) {
      app.$set({ content: { ...SAMPLE, text: texts[i % texts.length], reference: `R ${i}` } });
      await new Promise((r) => setTimeout(r, 70));
    }
    await settle(300);
    expect(seen.length, 'a clipped box on a moving surface reported nothing at all').toBeGreaterThan(0);
    expect(
      seen.some((f) => f.clipped),
      'it retried in silence instead of saying the words were being cut off'
    ).toBe(true);
  });

  it('re-fits when the words change but their LENGTH does not', async () => {
    // ROUND 4, AND IT IS THE THING I NAMED IN ROUND 2 AND SHOULD HAVE FIXED THEN.
    //
    // `.lfit` lives inside `{#key text}`, so ANY content change destroys it and
    // builds a new one — which carries the DECLARED base size and no fitted size
    // at all. `fitSig` folds in each layer's text LENGTH, not its text, so two
    // different passages of equal length produce an identical signature,
    // `runFit` early-returns, and the freshly-built element is never fitted. It
    // paints at the base, it clips, and `verifyFit` is never called either — so
    // nothing is measured, nothing is retried and nothing is reported.
    //
    // That is the lead's console, measured at 2000x1175 after a fire: `.lfit` at
    // 36.465px, which is 5.2cqw on a ~700px pane — 5.2 being exactly the default
    // verse size, i.e. the declared base untouched. 207px of content in a 174px
    // box, no warning anywhere, still wrong six seconds later, and corrected by a
    // 10px window resize — because a resize moves `stageEl.clientWidth` and that
    // IS in the signature.
    //
    // ── THE TRANSITION HYPOTHESIS IS NOT IT, AND THE MARKUP SETTLES IT ─────────
    // `{#key slideKey}` and `in:slideIn` wrap ONLY the region branch's `.slide`.
    // The layered branch — every `.ltext` and `.lfit` — sits above that block and
    // has no transition on it at all, so nothing here is ever measured mid-flight.
    // (And `transitions.js` animates only opacity, transform and filter, none of
    // which move `scrollHeight`, which is what that comment in the renderer has
    // always said.) Fourth theory, fourth probe, and this one found the bug.
    // The claim is that the new words are MEASURED, so it is counted rather than
    // compared: `onFit` reports once per fit pass, and jsdom has no layout, so
    // every fit converges on the same number whatever the text says. A size that
    // merely differs from the base proves nothing once a remembered size can be
    // re-applied — which is the other half of this fix, below.
    const seen = [];
    mount(layered, { reference: 'Romans 8:28', text: 'AAAA BBBB CCCC' }, {
      onFit: (f) => seen.push(f),
    });
    await settle(400);
    const first = seen.length;
    expect(first, 'it never fitted in the first place').toBeGreaterThan(0);

    // Same length, different words — the one thing the signature could not see.
    app.$set({ content: { reference: 'Romans 8:28', text: 'DDDD EEEE FFFF' } });
    await settle(400);
    expect(seen.length, 'the new words were never measured').toBeGreaterThan(first);
  });

  it('and says so when that re-fit still clips', async () => {
    // The second half of the same defect: `verifyFit` is only ever called after a
    // fit, so a skipped fit is also a skipped verdict. This is why Live was silent.
    const seen = [];
    const el = mount(layered, { reference: 'Romans 8:28', text: 'AAAA BBBB CCCC' }, {
      onFit: (f) => seen.push(f),
    });
    await settle(400);
    clipBoxes(el);
    seen.length = 0;
    app.$set({ content: { reference: 'Romans 8:28', text: 'DDDD EEEE FFFF' } });
    await settle(700);
    expect(seen.length, 'an equal-length fire reported nothing at all').toBeGreaterThan(0);
    expect(seen.at(-1)?.clipped, 'it went back to saying everything fits').toBe(true);
  });

  it('a size that was fitted survives the element that was fitted', async () => {
    // THE OTHER HALF, and the reason the signature cannot simply carry the full
    // text for every layer. A countdown or clock layer's text changes on every
    // tick, so `{#key text}` rebuilds ITS `.lfit` four times a second — and
    // folding that text into the signature would re-run the binary search at 4 Hz,
    // which is the forced-reflow storm the fit gating exists to prevent. The
    // digits hold their width as they count, so the size found once stays right:
    // it is re-APPLIED to the new element, a style write with no layout read,
    // rather than re-measured.
    const el = mount(layered, SAMPLE);
    await settle(400);
    const box = el.querySelector('.ltext');
    const fit = box.querySelector('.lfit');
    const wanted = fit.style.fontSize;
    expect(wanted).toBeTruthy();
    // Exactly what a `{#key}` rebuild leaves behind: the declared base, unsized.
    fit.style.fontSize = `${fit.dataset.base}cqw`;
    delete fit.dataset.sized;
    app.$set({ content: { ...SAMPLE } }); // an update that moves no signature
    await settle(200);
    expect(box.querySelector('.lfit').style.fontSize, 'the fitted size was not carried over').toBe(
      wanted
    );
  });

  it('a ticking layer is still kept OUT of the signature, by name', () => {
    // The regression this guards is a performance one, and it is in the
    // renderer's history twice: re-fitting on every countdown tick is a forced
    // synchronous reflow at 4 Hz on every mounted output at once. Somebody
    // "simplifying" `fitSig` to carry the text for every layer would reintroduce
    // it, and nothing else in this suite would notice.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    const fn = src.slice(src.indexOf('function fitSig'), src.indexOf('function reapplyFitted'));
    expect(fn).toMatch(/isTicking\(L\)\s*\?\s*t\.length\s*:\s*t/);
    for (const bind of ['countdown', 'clock', 'elapsed', 'remaining']) {
      expect(src, `${bind} is a ticking bind in layerText and must be one here`).toMatch(
        new RegExp(`TICKING = \\[[^\\]]*'${bind}'`)
      );
    }
  });

  it('Live turns a clip into words, and not into the shrink sentence', () => {
    // A clip and a shrink are different failures: one is "you cannot read this
    // from the back", the other is "you cannot read all of it from anywhere".
    const live = readFileSync(resolve(__dirname, './views/Live.svelte'), 'utf8');
    const fn = live.slice(live.indexOf('function noteFit'), live.indexOf('function noteFit') + 700);
    expect(fn).toMatch(/f\.clipped/);
    // The existing sentence is still there for the shrink case.
    expect(live).toMatch(/may not be readable from the back/);
    expect(live).toMatch(/Math\.round\(f\.scale \* 100\)/);
  });
});
