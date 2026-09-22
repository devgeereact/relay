// THE PROGRAMME ROW'S OWN GEOMETRY, ON THE PREACHER'S SCREEN.
//
// Three defects were measured in a real layout engine by the wave 3 browser pass
// (`docs/qa/audits/DESIGN.md` §8 and §9) and filed as RG-147 and
// RG-154. This file is what holds them shut afterwards.
//
// ── WHAT THIS FILE CAN AND CANNOT PROVE, PLAINLY ────────────────────────────────
//
// jsdom computes NO layout. Nothing here measures a painted pixel, and any test in
// this repository that claims one from jsdom is claiming something it cannot see.
// `src/lib/` already settled on the honest instrument for a CSS claim — read the
// component's own `<style>` block, with its commentary stripped, and assert the
// shipped declaration (`cardfit.test.js`, `countdownwarnmotion.test.js`). That is
// what the first and third describes below do.
//
// The second describe goes one step further and it is worth being exact about what
// that step is. It reads the constants OUT of the shipped declaration — the share,
// the divisors, the floor and the cap — and evaluates them arithmetically against
// the audit's own measured mono advance of 0.600. That is an ARITHMETIC MODEL OF
// THE RULE, not a measurement of the paint. What makes it trustworthy rather than a
// second opinion is that it reproduces the audit's browser figures to a tenth of a
// pixel: at 1280 x 720 with six timers it computes a 51.28 px font against the
// audit's measured 51.276 px, and a 215.4 px string against the measured 215.3 px
// in a 199 px box. A model that lands on the numbers a browser produced is evidence
// about the rule; it is still not evidence about the screen. The screen was checked
// once, in the audit, and cannot be checked from here.
//
//   npx vitest run src/lib/stageprogrow.test.js
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { formatCountdown } from './layers.js';

const ROOT = path.resolve(__dirname, '../..');

/** The `<style>` block of a Svelte file, with its commentary removed and its
 *  whitespace flattened. A comment that mentions a declaration is not the
 *  declaration — this repository has had a scan report a removed control as
 *  present for exactly that reason. */
function styleOf(file) {
  const src = fs.readFileSync(path.join(ROOT, file), 'utf8');
  const at = src.indexOf('<style>');
  return src
    .slice(at, src.lastIndexOf('</style>'))
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/\s+/g, ' ');
}

/** The body of the first rule with this exact selector.
 *
 *  The boundary is load-bearing and its absence cost an hour: a plain
 *  `indexOf('.tval {')` also finds `.tmr.warn .tval {`, so asking for the base
 *  rule handed back a warning rule from inside a media query and the assertions
 *  above it were about a declaration nobody had written. A selector match has to
 *  start where a selector can start. */
function ruleOf(css, selector) {
  const m = new RegExp(
    `(?:^|[{};])\\s*${selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}\\s*\\{`,
  ).exec(css);
  if (!m) return null;
  const open = m.index + m[0].length - 1;
  let depth = 0;
  for (let i = open; i < css.length; i++) {
    if (css[i] === '{') depth++;
    else if (css[i] === '}' && --depth === 0) return css.slice(open + 1, i);
  }
  return null;
}

/** The [start, end) extents of every `@media (...query...)` block, by brace
 *  matching. A regex cannot answer "is this declaration inside that block", and
 *  that is the only question the motion assertions ask. Same instrument as
 *  `countdownwarnmotion.test.js`, which holds the other three surfaces. */
function mediaBlocks(css, query) {
  const out = [];
  const needle = `@media (${query})`;
  let at = css.indexOf(needle);
  while (at !== -1) {
    let i = css.indexOf('{', at);
    let depth = 0;
    for (; i < css.length; i++) {
      if (css[i] === '{') depth++;
      else if (css[i] === '}' && --depth === 0) break;
    }
    out.push([at, i]);
    at = css.indexOf(needle, i);
  }
  return out;
}

const inside = (blocks, idx) => blocks.some(([a, b]) => idx > a && idx < b);

const CSS = styleOf('src/Stage.svelte');
const TVAL = ruleOf(CSS, '.tval');
const PROGROW = ruleOf(CSS, '.progrow');

// The declaration this whole file is about. The SHAPE is half the assertion: the
// character budget must be a var fed from the text the row is actually painting,
// not a constant. RG-147 is the constant.
const SIZE =
  /font-size:\s*min\(\s*clamp\(\s*([\d.]+)px\s*,\s*calc\(\s*([\d.]+)cqw\s*\/\s*var\(\s*--tmrs[^)]*\)\s*\/\s*var\(\s*--tch[^)]*\)\s*\/\s*([\d.]+)\s*\)\s*,\s*([\d.]+)px\s*\)\s*,\s*([^;]+)\)\s*;/;

describe('RG-147 — a Stage Timer past an hour is not silently clipped', () => {
  it('budgets the characters the row is actually painting, never a constant six', () => {
    // Watched to fail against the tree at 91abb59, where the declaration reads
    // `calc(92cqw / var(--tmrs) / 6 / 0.62)`: `formatCountdown` emits SEVEN
    // characters past an hour and the budget was six, so `1:30:13` painted
    // 215.3 px into a 199 px `overflow: hidden` box and read as `1:30:1`.
    expect(
      SIZE.test(TVAL ?? ''),
      'the Stage Timer digits are not sized from the string they are rendering — ' +
        'a fixed character budget is RG-147, and what is left of a clipped clock ' +
        'reads as a valid time',
    ).toBe(true);
  });

  it('ellipsises a figure it still cannot fit, rather than slicing the last digit', () => {
    // The second half, and the one that does not depend on an estimate being
    // right. `0.62` is an assumed advance and the measured one is 0.600; the
    // clamp floor can also bind on a narrow enough row with enough timers. In
    // either case the digits overflow a box that is `overflow: hidden`, and a
    // sliced `1:30:1` is a lie a preacher cannot detect. An ellipsis is the same
    // discipline `.tlabel` three lines above already keeps, and it is this page's
    // version of rule 37's report: shrink and show, and SAY when it did not fit.
    expect(TVAL).toMatch(/white-space:\s*nowrap/);
    expect(TVAL).toMatch(/text-overflow:\s*ellipsis/);
    expect(TVAL).toMatch(/overflow:\s*hidden/);
    expect(TVAL, 'a flex item that cannot shrink cannot ellipsise').toMatch(/min-width:\s*0/);
  });
});

describe('RG-147 — the arithmetic of the shipped declaration, against the audit', () => {
  // The audit's own measured constant. `.tval` is `var(--f-mono)`; the advance the
  // browser reported for it is 0.600 em per character, and the stylesheet's own
  // divisor is 0.62 (a deliberate margin, not a disagreement).
  const MEASURED_ADVANCE = 0.6;

  const m = SIZE.exec(TVAL ?? '');
  const floorPx = m ? Number(m[1]) : NaN;
  const sharePct = m ? Number(m[2]) : NaN;
  const assumedAdvance = m ? Number(m[3]) : NaN;
  const capPx = m ? Number(m[4]) : NaN;

  // `.progrow`'s own box, read from the same stylesheet rather than retyped.
  const padPx = Number(/padding:\s*[\d.]+px\s+([\d.]+)px/.exec(PROGROW ?? '')?.[1]);
  const gapPx = Number(/gap:\s*([\d.]+)px/.exec(PROGROW ?? '')?.[1]);

  /**
   * The width of one `.tmr`, and the font-size the declaration yields, for a
   * viewport `vw` CSS pixels wide carrying `timers` rows whose longest value is
   * `chars` characters.
   *
   * `.progrow` is `container-type: inline-size`, so `cqw` is a share of its
   * CONTENT box — the frame less its own horizontal padding. The row is a flex
   * container with `gap`, and every `.tmr` is `flex: 1 1 0`, so each item takes an
   * equal share of what the gaps leave.
   */
  function model(vw, timers, chars) {
    const content = vw - 2 * padPx;
    const box = (content - (timers - 1) * gapPx) / timers;
    const wanted = (content * (sharePct / 100)) / timers / chars / assumedAdvance;
    const fontPx = Math.min(Math.max(wanted, floorPx), capPx);
    return { box, fontPx, textPx: chars * MEASURED_ADVANCE * fontPx };
  }

  it('reproduces the browser figures the audit measured, which is why it is trusted', () => {
    // Not a claim about the fix. This asserts that the model in this file is the
    // same arithmetic the browser performed, by landing on §8's table. If this
    // ever fails, every number below it is worthless and should be re-measured in
    // a real engine rather than argued with here.
    expect(padPx).toBe(18);
    expect(gapPx).toBe(10);
    // Six timers at 1280 x 720, the audit's own row: 51.276 px into a 199 px box.
    const asItWasSized = (() => {
      const content = 1280 - 36;
      const box = (content - 5 * gapPx) / 6;
      const font = (content * 0.92) / 6 / 6 / 0.62; // the SIX-character budget
      return { box, font, text: 7 * MEASURED_ADVANCE * font };
    })();
    expect(asItWasSized.box).toBeCloseTo(199, 0);
    expect(asItWasSized.font).toBeCloseTo(51.276, 2);
    expect(asItWasSized.text).toBeCloseTo(215.3, 0);
    expect(asItWasSized.text - asItWasSized.box).toBeCloseTo(16.3, 0);
  });

  it('fits a 95-minute timer at every viewport the audit clipped one at', () => {
    // `formatCountdown` is the source of the character count, not a literal: the
    // day it grows a field this test grows with it.
    const chars = formatCountdown(95 * 60_000).length;
    expect(chars).toBe(7);

    for (const vw of [1920, 1280, 1024, 844]) {
      const { box, textPx, fontPx } = model(vw, 6, chars);
      expect(
        textPx,
        `six Stage Timers at ${vw} px: a ${chars}-character value is ${textPx.toFixed(1)} px ` +
          `of text at ${fontPx.toFixed(1)} px in a ${box.toFixed(1)} px box`,
      ).toBeLessThanOrEqual(box);
    }
  });

  it('keeps the ordinary m:ss row as large as it always was', () => {
    // The fix must not pay for the hour case by shrinking every other service.
    // A four-character `5:00` now gets a LARGER budget than the old constant six.
    const chars = formatCountdown(5 * 60_000).length;
    expect(chars).toBe(4);
    const { fontPx, box, textPx } = model(1280, 6, chars);
    expect(textPx).toBeLessThanOrEqual(box);
    expect(fontPx).toBeGreaterThan(51.276);
  });
});

describe('RG-154 — the height cap measures something that exists', () => {
  it('uses no container-height unit inside an inline-size-only container', () => {
    // Watched to fail against the tree at 91abb59, where `.tval` ended
    // `min(…, 9cqh)`. `.progrow` establishes an INLINE-axis container, so `cqh`
    // has no eligible container and silently falls back to the small viewport:
    // measured at 1920 x 500 with three timers, 9% of the row would be 6.70 px,
    // 9% of the viewport is 45.00 px, and the computed font-size was 45.00 px.
    // A cap that is not the cap anybody wrote is a latent version of the exact
    // bug the comment above it says it replaced.
    expect(PROGROW, 'the premise of this test').toMatch(/container-type:\s*inline-size/);
    expect(
      /\d(?:cqh|cqb|cqmin|cqmax)/.test(TVAL ?? ''),
      'a container-HEIGHT unit inside an inline-size container resolves against the ' +
        'viewport, not the row — RG-154',
    ).toBe(false);
  });

  it('caps the digits against the row’s own ceiling, from one figure stated once', () => {
    // The honest fix the register asked for: the row's ceiling and the digits'
    // cap are the same measurement, so they cannot drift, and the unit is one
    // that actually resolves. `dvh` is the page's own frame (`.sr` is `100dvh`).
    expect(PROGROW).toMatch(// `calc(20dvh * var(--tmul))` since RG-240: the operator's size multiplies the
    // row's ceiling, because `.tval` is capped against it and would otherwise
    // ignore the setting entirely — RG-223 in a second place.
    /--progmax:\s*calc\([\d.]+dvh \* var\(--tmul/);
    expect(PROGROW).toMatch(/max-height:\s*var\(--progmax\)/);
    expect(TVAL).toMatch(/calc\(\s*var\(--progmax\)\s*\*\s*[\d.]+\s*\)/);
  });
});

describe('RG-148 — the stage row wears the one warning rule', () => {
  it('reads the threshold rather than inventing a fourth one', () => {
    // THE RULE DID NOT MOVE; THE FILE DID. The derivation this asserts against was
    // `Stage.svelte`'s reactive block until `output.html` gained a programme rail
    // of its own (requirement 2b), at which point keeping it in a component would
    // have meant a second copy of exactly this expression in `TemplateRender` —
    // which is the defect this case exists to prevent, one surface along. Both
    // files are read, so the assertion still fails if the three-argument call
    // disappears from the product, and cannot pass because it was quietly moved
    // somewhere nobody is looking.
    const src =
      fs.readFileSync(path.join(ROOT, 'src/Stage.svelte'), 'utf8') +
      fs.readFileSync(path.join(ROOT, 'src/lib/timers.js'), 'utf8');
    // The per-timer override is the whole point: `warn_ms` rides every timer
    // frame and had no reader on this page. Three arguments, one rule.
    expect(src).toMatch(/countdownWarning\(\s*r\.ms\s*,\s*countdownTotalMs\(\s*t\s*\)\s*,\s*t\?\.warn_ms\s*\)/);
    // …and the comment that said it was deliberately unread is gone. Track D has
    // landed; a comment describing a decision the tree has since made is the
    // failure `docs/` has been corrected for repeatedly.
    expect(src).not.toMatch(/deliberately not read here yet/);
  });

  it('states the warning colour unconditionally, and it is not a law colour', () => {
    // Amber means ON AIR, cyan means the AI is guessing, amethyst means rehearsal.
    // The countdown's red is what this page already uses for the same rule.
    //
    // Asserted by INDEX, not by reading the first rule with this selector: the
    // colour has to sit outside every motion query, or a viewer who asked for no
    // motion gets no warning at all and the countdown simply runs out while
    // somebody watches it.
    const m = /\.tmr\.warn \.tval \{ color: var\(--v-red\); \}/.exec(CSS);
    expect(m, 'the programme row has no unconditional warning colour — RG-148').toBeTruthy();
    expect(inside(mediaBlocks(CSS, 'prefers-reduced-motion: no-preference'), m.index)).toBe(false);
    expect(inside(mediaBlocks(CSS, 'prefers-reduced-motion: reduce'), m.index)).toBe(false);
    expect(m[0]).not.toMatch(/--v-amber|--v-cyan|--v-amethyst/);
  });

  it('pulses only where motion was welcome, and glows where it was not', () => {
    // The dock is colour-only under reduced motion, deliberately. This page is a
    // platform monitor as well as a phone and already has a glow fallback, so the
    // programme row matches the page it is on rather than the dock.
    const noPref = mediaBlocks(CSS, 'prefers-reduced-motion: no-preference');
    const reduce = mediaBlocks(CSS, 'prefers-reduced-motion: reduce');

    // ON `over` SINCE 2026-09-21, not on `warn`. It pulsed for the whole warning
    // window — routinely five minutes — which is a pulse an operator stops seeing
    // before the moment it exists for. The steady red keeps the window;
    // `timers.js::programmeRows` carries the narrower fact.
    const pulse = CSS.indexOf('.tmr.over .tval { animation: cdwarn');
    expect(pulse, 'the programme row never pulses').toBeGreaterThan(-1);
    expect(inside(noPref, pulse), 'the pulse is given to a viewer who asked for no motion').toBe(true);

    const glow = /\.tmr\.over \.tval \{ text-shadow: [^}]*\}/.exec(CSS);
    expect(glow, 'no reduced-motion fallback on the programme row').toBeTruthy();
    expect(inside(reduce, glow.index)).toBe(true);
    expect(glow[0], 'a second opinion about red').toMatch(/244, ?81, ?91/);
  });
});
