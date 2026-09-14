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
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
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
    const fn = src.slice(src.indexOf('function fitLayers'), src.indexOf('// Fit is driven by'));
    expect(fn).toMatch(/if \(onFit\)/);
    expect(fn).toMatch(/try \{/);
    expect(fn).toMatch(/catch/);
  });
});
