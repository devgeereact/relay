// RG-139 — A RUNNING COUNTDOWN UNDID THE FIT OF EVERY TEXT LAYER BESIDE IT.
//
// The measured symptom, at 1920×1080 in a real browser against the shipped
// `Timer · Titled` with a 118-character label: the fitter answered
// `data-fitted="1.99375"` and the element painted at its DECLARED `3.4cqw` from
// the first countdown tick onward — 176px of words inside a 97px
// `overflow:hidden` box, three lines of a notice sliced through the middle on a
// congregation's pre-service screen. `Stage · Rail` did the same at 109px in a
// 108px box, and its three static labels dropped from their fitted `2.2773cqw`
// to their declared `1.6cqw` at the same tick.
//
// `onFit` reported `{ scale: 0.586, legible: true, clipped: false }` throughout.
// That report was not a lie when it was taken — it was taken half a second
// earlier, before anything had undone the fit. Rule 37 says shrink and report,
// never silently clip; here the shrink was computed, reported, and then thrown
// away by something that never reports at all.
//
// ── THE CAUSE, READ OUT OF THE COMPILER RATHER THAN GUESSED ────────────────
//
// `.lfit` declares `font-size:{baseSize(L)}cqw` inline on purpose (rule 42,
// `cardfit.test.js`): an un-fitted element must paint the DESIGNED size, not the
// app's 12px UI body text. Svelte 4 compiles that attribute into one
// `set_style(div, 'font-size', …)` per interpolation, and — unlike the plain
// `data-base` / `data-fit` attributes emitted two lines above it — the update is
// guarded on the DIRTY BIT ALONE, with no comparison against the last value:
//
//     if (dirty[0] & /*stackLayers*/ 262144) {
//       set_style(div, "font-size", baseSize(ctx[142]) + "cqw");
//     }
//
// So any update that dirties `stackLayers` re-writes the declared base over the
// imperative fit, in place, on every visible text layer. A countdown tick is
// exactly that update, and it is the one case nothing recovers from: `fitSig`
// folds a ticking layer in by text LENGTH, so `4:59` → `4:58` does not move the
// signature and no re-fit runs.
//
// Both recovery paths declined, and for the same reason: they asked `data-sized`,
// which is a ONE-WAY LATCH. It records that a fit once happened, never that its
// answer is still on the element — so an element clobbered in place kept the
// latch, `reapplyFitted` returned early and `anythingUnfitted` said no. A flag
// that cannot report that the thing it stands for has been undone is rule 37's
// own shape, one level above the loop it was written for.
//
// ── WHAT THIS FILE CAN AND CANNOT SEE ──────────────────────────────────────
//
// jsdom computes no layout, so nothing here measures a painted pixel and no
// assertion below claims one. What jsdom reproduces exactly is the MECHANISM:
// the same compiled `set_style` runs, on the same tick, against the same element,
// and the question asked is the one the renderer itself has to answer — is the
// size on this element the size the fitter answered for its box. Because jsdom
// reports every box as 0×0, round one of the search always fits and every
// 'both'-mode layer lands on its ceiling (22cqw), which is a value that differs
// from the declared base — so the revert is visible here even though the clip is
// not. `VACUOUS` below refuses to let that stop being true silently.
//
// The pixel evidence is a browser pass at 1920×1080 against the real component:
// before, 176/97 and 4 of 482 animation frames painting the declared base; after,
// 98/97 and 964 of 964 frames painting the fitted size.
//
//   npx vitest run src/lib/countdownfit.test.js

import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';

const SHELF = JSON.parse(
  readFileSync(resolve(process.cwd(), 'src-tauri/data/shelf_templates.json'), 'utf8'),
).templates;

/** Every shipped look with a layer bound to the clock — the surface of this row.
 *  Enumerated from the shelf rather than named, because the reproducer this was
 *  filed against (`Stage · Large type`) no longer ships and the six that replaced
 *  it were found by a re-measure, not by the original report. */
const TICKING = SHELF.filter((t) =>
  (t.layout?.layers || []).some((L) => L.bind === 'countdown' && L.visible !== false),
);

/** A live countdown, shaped as `pipeline::Fire` broadcasts one. The label is long
 *  on purpose: `Service begins in` is 17 characters, fits at the declared size on
 *  every shipped Timer look, and therefore never shows this defect at all. The
 *  audit's own threshold note says so, and a fixture that used it would be a test
 *  that cannot fail. */
const LONG_LABEL =
  'Our service begins shortly, please take your seats and silence your phones, thank you kindly friends';
const countdown = () => ({
  kind: 'countdown',
  content_kind: 'countdown',
  reference: LONG_LABEL,
  text: '',
  countdown_to: Date.now() + 5 * 60_000,
  countdown_from: Date.now(),
});

let host;
let app;
function mount(template, content, props = {}) {
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

/** Every text layer on screen, with the size it is WEARING and the size the
 *  fitter last ANSWERED for its box. Attribute reads only — the same two facts
 *  `TemplateRender.fitInForce` compares, asked from outside. */
function sizes(el) {
  return [...el.querySelectorAll('.ltext')]
    .map((box) => {
      const fit = box.querySelector('.lfit');
      if (!fit) return null;
      return {
        base: Number(fit.dataset.base),
        answered: box.dataset.fitted == null ? null : Number(box.dataset.fitted),
        worn: parseFloat(fit.style.fontSize),
        words: (fit.textContent || '').trim(),
      };
    })
    .filter(Boolean);
}

/** The fit is async by construction — `afterUpdate` schedules a frame and the
 *  verdict is taken a frame after that (`verifyFit`'s `nextFrame`). */
const settle = () => new Promise((r) => setTimeout(r, 300));
/** Long enough for `countdownText` to change, which is what dirties
 *  `stackLayers`. The clock reads every 250ms and the FORMATTED string moves once
 *  a second, so a shorter wait would pass without the defect ever being armed. */
const oneTick = () => new Promise((r) => setTimeout(r, 1400));

/** A layer the fit had to move off its declared size. Without at least one of
 *  these the assertion below is satisfied by a renderer that never fits anything,
 *  which is precisely the failure rule 37 exists to name. */
const moved = (rows) => rows.filter((r) => r.answered != null && Math.abs(r.answered - r.base) > 0.05);

describe('a running countdown does not undo the fit beside it', () => {
  /** How many layers, across the whole shelf, the fitter actually moved off their
   *  declared size. If this is zero the invariant below is satisfied by a renderer
   *  that never fits anything — rule 40's "a test that cannot fail is a theory
   *  that was never tested", which is why it is counted rather than assumed. */
  let nonVacuous = 0;

  it('the shelf ships looks this can happen to', () => {
    // Six, per the 2026-09-16 re-measure. Asserted as "some" rather than as six:
    // the claim is that the surface exists and is enumerated from the shipped
    // file, not that its size is frozen.
    expect(TICKING.length, 'no shipped look binds a layer to the countdown').toBeGreaterThan(0);
  });

  it.each(TICKING.map((t) => [t.name, t]))(
    '%s keeps every layer at the size it was fitted to, tick after tick',
    async (name, template) => {
      const el = mount(template, countdown());
      await settle();

      const first = sizes(el);
      expect(first.length, `${name} drew no text at all`).toBeGreaterThan(0);
      // Recorded per look rather than asserted per look, because two of the six
      // genuinely cannot show this: `Timer · Monolith` and `Timer · Contrast`
      // carry the countdown layer ALONE, and `Contrast` declares 22cqw, which is
      // already the ceiling a 'both'-mode search may reach. A look with nothing
      // moved has nothing to revert, and failing it here would be failing it for
      // being simple. The suite's own guard against vacuity is the test below.
      nonVacuous += moved(first).length;

      await oneTick();

      for (const row of sizes(el)) {
        if (row.answered == null) continue;
        expect(
          row.worn,
          `${name}: "${row.words.slice(0, 24)}" was fitted to ${row.answered}cqw and is painting ${row.worn}cqw (declared ${row.base}cqw)`,
        ).toBeCloseTo(row.answered, 2);
      }
    },
  );

  it('and the assertion above had something to catch', () => {
    // Runs after the `it.each`, which is what makes the count meaningful. Vitest
    // executes tests in declaration order within a describe.
    expect(
      nonVacuous,
      'the fitter moved no layer off its declared size anywhere on the shelf, so nothing above could fail',
    ).toBeGreaterThan(0);
  });

  it('and it is not the clock that matters — ANY update that redraws the stack did it', async () => {
    // The countdown is where this is unrecoverable, because its ticks do not move
    // `fitSig` and so nothing re-fits. But the clobber itself is general: every
    // update that dirties `stackLayers` re-runs the compiled `set_style`. A Stage
    // Message arriving mid-countdown is the same update on a surface a preacher
    // is reading, and it moves no signature either — `fitSig` folds in the layers'
    // geometry and words, and a template with no `stage_message` layer has none of
    // it to move.
    // TWO CONDITIONS, AND BOTH WERE LEARNED BY WATCHING THIS TEST PASS OVER THE
    // DEFECT. The layer has to be OFF THE CLOCK — `Timer · Monolith`'s only text
    // layer is the countdown itself, and a `{#key text}` rebuild hands that one
    // back its size by the other road, so it recovers with the fix reverted. And
    // it has to be a layer jsdom can move off its declared size, which means
    // 'both' mode: a label or a static is 'shrink', its ceiling IS its base, and
    // in a layout engine that reports every box as 0×0 the fit lands exactly where
    // the clobber would put it. `Timer · Titled` — the browser's own reproducer —
    // fails both halves here, which is the jsdom limit this file opens by naming.
    const TICKS = ['countdown', 'clock', 'elapsed', 'remaining'];
    // `defaultFit`'s own rule, read from the renderer: 'shrink' for a LABEL_BIND
    // or a `type:'static'` layer, 'both' for everything else. Note that a layer
    // BOUND to `static` is type `text` and therefore 'both' — the rule is about
    // the layer's type, not its binding, and the shelf's caption layers are the
    // former only in name.
    const GROWS = (L) =>
      L.type === 'text' &&
      L.visible !== false &&
      !TICKS.includes(L.bind) &&
      !['reference', 'translation'].includes(L.bind);
    const template = TICKING.find((t) => (t.layout?.layers || []).some(GROWS));
    expect(template, 'no shipped ticking look has a growable layer off the clock').toBeTruthy();
    const el = mount(template, countdown(), { stageMessage: '' });
    await settle();
    const before = sizes(el);
    expect(moved(before).length).toBeGreaterThan(0);

    app.$set({ stageMessage: 'two minutes' });
    await settle();

    for (const row of sizes(el)) {
      if (row.answered == null) continue;
      expect(row.worn, `"${row.words.slice(0, 24)}" lost its fit to an unrelated redraw`).toBeCloseTo(
        row.answered,
        2,
      );
    }
  });
});

describe('the repair is not allowed to cost what the gating saves', () => {
  // THE REGRESSION THIS FIX ALREADY CAUSED ONCE, pinned so it cannot come back.
  // The first version of `fitInForce` compared `el.style.fontSize` to
  // `` `${fitted}cqw` `` as STRINGS. The fitter's answers are full-precision JS
  // numbers and the CSSOM does not keep the string it was handed: write
  // `21.166796875cqw`, read `21.1668cqw`. So the comparison said "different"
  // forever, on an element wearing exactly the right size, and `anythingUnfitted`
  // sent the whole binary search through a forced reflow on every frame —
  // measured in the browser at 118 fit passes and 2682 style writes in 29 seconds
  // of one countdown. With the numeric comparison: 1 fit pass, 2 reports.
  it('compares sizes as numbers, against the fitter own epsilon', () => {
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    const fn = src.slice(src.indexOf('function sizeIs('));
    expect(fn.slice(0, 200)).toMatch(/parseFloat/);
    expect(fn.slice(0, 200)).toMatch(/FIT_EPS_CQW/);
    expect(fn.slice(0, 200), 'a string compare cannot survive CSSOM rounding').not.toMatch(
      /===\s*`\$\{px\}cqw`/,
    );
  });

  it('puts the size back in the same task that took it away, not a frame later', () => {
    // `afterUpdate` runs before the browser paints; the next animation frame is
    // one paint later. With the repair left in the frame, a sample of
    // `getComputedStyle().fontSize` on every animation frame caught the declared
    // base on 4 of 482 frames — one per tick, a 65px flash on a 38px label, four
    // times a second, for the whole pre-service countdown. The expensive half —
    // the search, which READS layout — stays in the frame, which is what that
    // gating is for.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    const fn = src.slice(src.indexOf('function scheduleFit()'), src.indexOf('afterUpdate(scheduleFit)'));
    expect(fn).toMatch(/reapplyFitted\(\);/);
    expect(fn.indexOf('reapplyFitted();')).toBeLessThan(fn.indexOf('fitRaf ='));
  });

  it('and a layer the fitter DECLINES to measure records that as its answer', () => {
    // A crawl (`lscroll`) and a `fit:'none'` layer never enter the search, so they
    // never carried `data-sized` — and `anythingUnfitted` therefore said "true"
    // about them on every frame, which is the same reflow storm by a second road
    // for any template that pairs a notice crawl with a countdown. The size the
    // fitter answers for a layer it declines to measure is that layer's declared
    // base; recording it is truthful and free.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    const skip = src.slice(src.indexOf("if (el.classList.contains('lscroll') || mode === 'none')"));
    const body = skip.slice(0, skip.indexOf('return;'));
    expect(body).toMatch(/el\.dataset\.sized = '1';/);
    expect(body).toMatch(/box\.dataset\.fitted = String\(base\);/);
  });
});
