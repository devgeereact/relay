// RG-141 — THE COUNTDOWN A LEGACY TEMPLATE RENDERS COLLAPSED TO A SIXTH OF ITS
// DESIGNED SIZE, AND THE FIT LOOP CALLED THAT A SUCCESS.
//
// Measured at 1920×1080 against the real component, on a region-model row
// declaring `verseSize 5` — so a countdown designed at 10cqw, 192px:
//
//     .content   59 × 35 px      digits 25.97px      label 6.23px
//     onFit      { scale: 0.135, legible: false }
//
// A 59×35 blob in the middle of an otherwise empty 1920×1080 screen. That is the
// pre-service wall, and it is unreadable from the second row.
//
// ── WHY IT COLLAPSED, MEASURED RATHER THAN INFERRED ────────────────────────
//
// `.content` in region mode declares no width and no height. It is a flex item
// with `max-width: 90%` and `max-height: 92%`, so it is shrink-to-fit: its box IS
// its text. For a VERSE that is correct and the audit confirmed it — prose wraps,
// a long passage grows until it meets the caps, and only then does it genuinely
// overflow, which is a real signal. A COUNTDOWN is one `nowrap` line under
// `.countdown { line-height: 1.05 }`, a leading deliberately TIGHTER than the
// face's own line box, so the glyph box is a fixed FRACTION taller than the box
// measured around it, at every size:
//
//     scale   1     0.8   0.6   0.4   0.3   0.2   0.15   0.135
//     clientH 256   205   153   103   77    51    38     35
//     scrollH 268   215   161   108   80    54    40     36
//     residue 12    10    8     5     3     3     2      1   ← stops here
//
// `fitOne`'s stop condition is `scrollHeight > clientHeight + 1` — an ABSOLUTE
// one pixel. A loop whose overflow scales with the thing it is adjusting, judged
// against a tolerance that does not, can only terminate by shrinking until the
// residue rounds under a pixel. It always succeeds, and the success is a blob.
// Rule 37, in the purest form the repository has found: the loop had no notion of
// failure because it had no notion of the BOX.
//
// The repair is the box, not the tolerance. Widening the tolerance to a fraction
// would blind the loop to real overflow on the one kind — a verse — where the
// check is load-bearing. Giving the countdown a definite rectangle makes
// `clientHeight` independent of the type, which is what the LAYER branch has
// always had for free: `.ltext` and `.cd-default`'s lines are percentage boxes,
// and that is why a layered countdown never showed this. Same guarantee, on the
// second door.
//
// AFTER, same instrument, same viewport: `.content` 1486 × 782, digits at
// **192px** — the full declared 10cqw — label at 46.08px, `onFit { scale: 1,
// legible: true }`. And with a template whose designer asked for more than the
// frame (`verseSize 40`, a countdown designed at 1536px) the loop still shrinks,
// to 642px, and still reports: `{ scale: 0.418, legible: false }`. Rule 37's
// genuinely-unfittable case is unchanged.
//
// ── WHY THE BRANCH WAS FIXED RATHER THAN THE TEMPLATES CONVERTED ───────────
//
// A fresh install cannot reach this any more: wave 5 Track A rebuilt the shelf as
// forty layer-model looks and `nothing_region_model_is_seeded_any_more` holds both
// halves of that. What is left is an EXISTING install, where retirement keeps a
// legacy row precisely when a channel, a plan cue, a content look or the
// configured default still names it — a row a church deliberately uses. Unreachable
// on a fresh install is not the same as fixed, and the register already records the
// product intending to keep this path working: `Lower Third · Timer` was
// deliberately NOT converted, because on the region path `countdownAllowed` refuses
// a countdown on a keyed screen and what it paints is nothing, which is the clean
// camera the transparency law would have given anyway.
//
// Converting the survivors instead would need either a second implementation of
// `regionsToLayers` in Rust — and two converters is exactly how the two
// renderings in RG-140 came to disagree — or a startup pass through the JS one,
// which `TemplateGallery`'s own comment calls "a separate change with its own
// evidence, not a line in this one". Neither belongs inside a rendering fix.
//
// ── WHAT THIS FILE CAN SEE ─────────────────────────────────────────────────
//
// jsdom computes no layout, so nothing here measures a pixel and no assertion
// claims one; the figures above are the browser's. What is asserted is the thing
// that DECIDES the painted size — that the box a countdown is fitted against is a
// definite rectangle, on the templates a church actually holds, and that no other
// content kind was given one.
//
//   npx vitest run src/lib/regioncountdown.test.js

import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { isLayered, isKeyedTemplate } from './layers.js';

const SRC = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');

/** The region-model rows a church that installed Relay before wave 5 still holds.
 *  Read out of the Rust seed literals rather than invented, so the shape under
 *  test is the shape in the field — the same method `cardfit.test.js` uses. */
function legacyRows() {
  const src = readFileSync(resolve(process.cwd(), 'src-tauri/src/db/templates.rs'), 'utf8');
  const out = [];
  for (const m of src.matchAll(/r##"(\{.*?\})"##/gs)) {
    let o;
    try {
      o = JSON.parse(m[1]);
    } catch {
      continue; // a literal that is not JSON is not this test's business
    }
    if (o && Array.isArray(o.regions)) out.push({ layout: o, style: {} });
  }
  // Each layout literal is followed by its style literal; pair them by position.
  const styles = [];
  for (const m of src.matchAll(/r##"(\{.*?\})"##/gs)) {
    let o;
    try {
      o = JSON.parse(m[1]);
    } catch {
      continue;
    }
    if (o && !Array.isArray(o.regions) && (o.verseSize || o.background)) styles.push(o);
  }
  return out.map((t, i) => ({ ...t, style: styles[i] || { verseSize: '5', refSize: '2.4' } }));
}

const LEGACY = legacyRows();

/** A live countdown, shaped as `pipeline::Fire` broadcasts one. */
const countdown = () => ({
  kind: 'countdown',
  content_kind: 'countdown',
  reference: 'Service begins in',
  text: '',
  countdown_to: Date.now() + 5 * 60_000,
  countdown_from: Date.now(),
});
/** An ordinary fire, for the half of this that must NOT have changed. */
const VERSE = {
  kind: 'scripture',
  content_kind: 'scripture',
  reference: 'Romans 8:28 · KJV',
  text: 'And we know that all things work together for good to them that love God, to them who are the called according to his purpose.',
};

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
  app = null;
  host = null;
});

describe('a legacy template still holds region-model rows, and they still render', () => {
  it('the Rust seed literals are readable, or every assertion below is vacuous', () => {
    expect(LEGACY.length, 'no region-model literal was parsed out of templates.rs').toBeGreaterThan(0);
    for (const t of LEGACY) expect(isLayered(t), 'a region literal parsed as layered').toBe(false);
  });
});

describe('the box a countdown is fitted against is a definite rectangle', () => {
  it.each(LEGACY.filter((t) => !isKeyedTemplate(t)).map((t, i) => [`legacy row #${i + 1}`, t]))(
    '%s gives its countdown a sized box',
    (name, template) => {
      const el = mount(template, countdown());
      const box = el.querySelector('.content');
      expect(box, `${name} drew no content box for a countdown`).toBeTruthy();
      expect(box.classList.contains('cdbox'), `${name}: the countdown is fitted against its own text`).toBe(
        true,
      );
      // And the digits are actually there — a class on an empty box would satisfy
      // the assertion above while painting nothing.
      expect(el.querySelector('.verse.countdown'), `${name} painted no digits`).toBeTruthy();
    },
  );

  it('and the class is what makes the box definite, not merely a marker', () => {
    // The whole fix is these three declarations. A marker class with no size
    // would leave the box shrink-to-fit and the loop exactly where it was, which
    // is the failure mode this rule keeps producing: a change that looks like the
    // fix and measures like the defect.
    const rule = SRC.slice(SRC.indexOf('.content.cdbox {'));
    const body = rule.slice(0, rule.indexOf('}'));
    expect(body).toMatch(/width:\s*90%/);
    expect(body).toMatch(/height:\s*92%/);
    // Without this the panel mode's own padding would push the sized box past the
    // caps it is supposed to be restating.
    expect(body).toMatch(/box-sizing:\s*border-box/);
  });

  it('no other content kind was given one — a verse is measured as it always was', () => {
    // The shrink-to-fit box is CORRECT for prose and the audit measured it as
    // correct: on the same template `John 3:16` fitted at the full declared size
    // and `Esther 8:9` shrank and settled with `scrollHeight == clientHeight`.
    // Widening this fix to every kind would replace a working measurement with an
    // untested one.
    const template = LEGACY.find((t) => !isKeyedTemplate(t) && t.layout.regions.includes('verse_text'));
    const el = mount(template, VERSE);
    const box = el.querySelector('.content');
    expect(box).toBeTruthy();
    expect(box.classList.contains('cdbox'), 'a verse was handed the countdown box').toBe(false);
  });

  it('and a keyed region row still paints nothing at all for a countdown', () => {
    // `countdownAllowed` refuses a countdown on a screen that composites over a
    // camera, and `.cdbox` is gated on the same fact, so the refusal cannot be
    // turned into an empty sized box sitting over the shot of the preacher.
    const keyed = LEGACY.find((t) => isKeyedTemplate(t));
    if (!keyed) return; // no keyed literal survives; nothing to claim either way
    const el = mount(keyed, countdown());
    expect(el.querySelector('.verse.countdown'), 'a countdown reached a keyed screen').toBeNull();
    expect(el.querySelector('.content.cdbox'), 'a keyed screen got a sized countdown box').toBeNull();
  });
});
