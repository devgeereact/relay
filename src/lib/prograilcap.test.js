// THE RAIL'S HEIGHT CAP HAS TO MEASURE THE RAIL.
//
// `.lp-val` caps its figure so a single clock on a wide box cannot grow taller
// than the box it sits in. The cap was written as `62cqh`. `.lprog` is
// `container-type: inline-size`, which contains the INLINE axis and nothing
// else — so `cqh` inside it does not resolve against the rail at all, it
// resolves against the small viewport.
//
// Measured in a real engine on a 1920x1080 stage template, with the seeded
// Programme layer (7% of height, so a 76px rail):
//
//   62cqh resolved to          670px      (62% of the 1080 viewport)
//   62% of the rail would be    47px
//   digit font-size            64px       — the clamp ceiling, uncapped
//
// So the cap did nothing, the digits took the ceiling, and `overflow: hidden`
// cut them along the bottom. A preacher glancing down at a sliced figure is the
// RG-147 failure turned on its side: what is left of it still reads as a time.
//
// `Stage.svelte` does not have this bug and the difference is the whole lesson.
// Its rail is `container-type: inline-size` too (`:1719-1722`, deliberate), and
// it caps against `--progmax`, a `dvh` figure it sets itself. It never asks a
// container query for a height its container does not contain.
//
// This is a SOURCE-SHAPE test and that is deliberate: jsdom lays nothing out and
// computes no container query, so the only instrument available here is the rule
// itself. The measurement above is the evidence; this is what stops it drifting.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const SRC = readFileSync(resolve(__dirname, 'TemplateRender.svelte'), 'utf8');

/** The body of one CSS rule, by selector, so a match cannot come from a comment. */
function rule(selector) {
  const i = SRC.indexOf(`\n  ${selector} {`);
  expect(i, `the rule \`${selector}\` is gone — this test is reading nothing`).toBeGreaterThan(-1);
  return SRC.slice(i, SRC.indexOf('\n  }', i));
}

describe('the programme rail caps its figure against the rail', () => {
  it('.lprog contains the inline axis only, which is why cqh is the wrong unit here', () => {
    expect(rule('.lprog')).toMatch(/container-type:\s*inline-size/);
  });

  it('the figure does not cap itself with a container-query HEIGHT', () => {
    // `cqw` is correct and stays: the width IS contained. `cqh`, `cqb`, `cqmin`
    // and `cqmax` all reach for a block axis this container does not have.
    expect(rule('.lp-val')).not.toMatch(/\d(cqh|cqb|cqmin|cqmax)\b/);
  });

  it('nor does the overflow cell, which had the same cap and the same bug', () => {
    expect(rule('.lp-msg')).not.toMatch(/\d(cqh|cqb|cqmin|cqmax)\b/);
  });

  it('both cap against the measured height instead', () => {
    expect(rule('.lp-val')).toMatch(/var\(--lp-h/);
    expect(rule('.lp-msg')).toMatch(/var\(--lp-h/);
  });

  it('and something actually measures it and hands it over', () => {
    // A cap reading a custom property nothing sets is a cap with a fallback and
    // no measurement — which is what this bug would look like the second time.
    expect(SRC).toMatch(/progH\[i\]\s*=\s*h/);
    expect(SRC).toMatch(/clientHeight/);
    expect(SRC).toMatch(/railRoomVars\(progH\[i\], progHeadH\[i\]\)/);
  });

  it('an unmeasured rail OMITS the declaration rather than setting it to zero', () => {
    // Setting `--lp-h: 0px` would be worse than setting nothing. The property
    // would BE set, so `var(--lp-h, 100px)` could never reach its fallback, and
    // `min(clamp(12px, …), calc(0px * 0.62))` is `0px`. Measured in a real
    // engine on a painted rail: forcing `--lp-h: 0px` took the digits from
    // 47.12px to 0px, so the rail would be blank for the frame before
    // `afterUpdate` runs and then appear.
    const fn = SRC.slice(SRC.indexOf('const railHeightVar'));
    expect(fn.slice(0, 120)).toMatch(/Number\(px\)\s*>\s*0\s*\?/);
    expect(fn.slice(0, 120)).toMatch(/:\s*''/);
    // And the fallback it falls back TO is a real size, not another zero.
    expect(rule('.lp-val')).toMatch(/var\(--lp-h,\s*\d+px\)/);
  });
});

// ── AND THE SECOND HALF OF THE SAME BUG, WHICH THE RULE ABOVE COULD NOT SEE ──
//
// Capping the figure against the rail stopped it exceeding the rail. It reserved
// nothing for the LABEL above it, and the two only fit at 16:9 by arithmetic
// accident: `.lp-lbl` is `clamp(9px, …)`, so below a certain rail the label stops
// shrinking while the figure keeps going at its share, their sum passes the rail
// height, and `overflow: hidden` cuts the digits through the middle again.
//
// Measured in a real engine, three timers on the seeded stage template:
//
//   1920x1080  rail 75.59  head 21.12 + gap 7.68 + figure 47.12 = 75.92   fits
//    854x480   rail 33.59  head  9.90 + gap 3.42 + figure 21.08 = 34.39   SLICED
//    900x300   rail 21.00  head  9.90 + gap 3.60 + figure 13.02 = 26.52   SLICED
//
// At 900x300 the figure box was squeezed to 7.51px against a 13.02px line — 42%
// of every digit gone, `1:29:37` painting as `1.29.37`.
//
// **The rule above is green through all of that**, and that is the lesson worth
// keeping: it pins the UNIT (`var(--lp-h)`, never `cqh`) and not the OUTCOME, so
// it cannot see a cap that uses the right unit and the wrong number. The
// arithmetic now lives in `timers.js` where it can be tested against numbers.
import { programmeRoom, MIN_FIGURE_PX, BARE_BELOW_PX } from './timers.js';

describe('the figure gets the room the label is not using', () => {
  it('an unmeasured rail yields NOTHING, so the stylesheet fallback applies', () => {
    // Not zero. A property set to `0px` IS set, so `var(--lp-room, …)` could
    // never reach its fallback and the figure would be invisible for the frame
    // before the first measurement.
    expect(programmeRoom(0, 0)).toBeNull();
    expect(programmeRoom(undefined, undefined)).toBeNull();
    expect(programmeRoom(-5, 10)).toBeNull();
  });

  it('reserves what the label measured, rather than a share of the rail', () => {
    // The 1920x1080 case: 75.59 rail, 21.12 head. The old cap gave the figure
    // 0.62 x 75.59 = 46.87 and got away with it; this gives 54 and is right for
    // a reason rather than by luck.
    expect(programmeRoom(75.59, 21.12)).toEqual({ rail: 76, bare: false, figure: 55 });
  });

  it('the label stands down when the rail cannot hold both', () => {
    // 900x300, the composite region that sliced every digit.
    expect(programmeRoom(21, 9.9)).toEqual({ rail: 21, bare: true, figure: 21 });
    // 640x360.
    expect(programmeRoom(25.2, 9.9)).toEqual({ rail: 25, bare: true, figure: 25 });
  });

  it('and stays when it can — 854x480 was the size that used to slice', () => {
    const r = programmeRoom(33.59, 9.9);
    expect(r.bare).toBe(false);
    expect(r.figure).toBe(24);
    expect(r.figure).toBeLessThanOrEqual(r.rail - 9);
  });

  it('never returns a figure under the reading floor', () => {
    // A label that somehow measures nearly the whole rail must not starve the
    // figure to nothing — it yields the floor, and the caller lets it overflow
    // rather than painting something unreadable and calling it a clock.
    expect(programmeRoom(40, 39).figure).toBe(MIN_FIGURE_PX);
    expect(programmeRoom(40, 1000).figure).toBe(MIN_FIGURE_PX);
  });

  it('THE OSCILLATION GUARD — the bare decision ignores the head entirely', () => {
    // This is the one that matters. The first version of this decided from the
    // measured head, which is a feedback loop: hiding the head makes it measure
    // 0, which makes the condition that hid it false, which brings it back. It
    // flickered on every update and I watched it do so.
    //
    // So for ANY head measurement, including the 0 a hidden head reports, the
    // answer for a given rail must be the same.
    for (const rail of [10, 21, 25.2, 29.9, 30, 33.59, 75.59, 200]) {
      const answers = [0, 1, 9.9, 21.12, 999].map((h) => programmeRoom(rail, h)?.bare);
      expect(new Set(answers).size, `rail ${rail} changed its mind about the label`).toBe(1);
    }
  });

  it('the threshold is where a label and a readable figure stop both fitting', () => {
    // The rail is rounded to whole pixels before the comparison, deliberately —
    // a sub-pixel difference is not a reason for a label to appear or vanish, and
    // a rail measured at 29.99 and 30.01 on consecutive frames must not flicker.
    expect(programmeRoom(BARE_BELOW_PX - 1, 0).bare).toBe(true);
    expect(programmeRoom(BARE_BELOW_PX, 0).bare).toBe(false);
    expect(programmeRoom(BARE_BELOW_PX - 0.01, 0)).toEqual(programmeRoom(BARE_BELOW_PX + 0.01, 0));
  });
});

// ── AND THE CEILING THAT MADE THE SIZE CONTROL LOOK DEAD (RG-223) ───────────
//
// The operator: *"Size still dosent move still... only the blur background
// moved, not the timer itself"*.
//
// `--lp-sz` reached this layer in RG-212 and the control genuinely moved the
// digits — for about one and a half turns of the dial, and then stopped. The
// expression is `min(declared, widthBudget, room)` and `widthBudget` was a
// `clamp(12px, …, 64px)`. That **64px is a literal, not a fit**, and on any
// ordinary stage rail it is the smallest of the three long before the box is.
//
// Measured on a 1920×1080 screen, the seeded Programme layer (full width, 8%
// tall, one timer, a six-character figure — `/tmp/sizemath.mjs`):
//
//   size   declared   width budget   room    painted
//   2.2      42px        294px        72px     42px   ← the control works
//   3        58px        294px        72px     58px   ← still working
//   4        77px        294px        72px     64px   ← the CEILING binds
//   6       115px        294px        72px     64px   ← nothing moves
//   12      230px        294px        72px     64px   ← nothing moves
//
// The middle term — the per-column budget that stops `1:30:13` being sliced
// (RG-147) — was never the thing binding: it is 294px there. The ceiling was.
//
// So the ceiling goes and the two REAL caps stay. Past that the rail's own
// height binds, which is honest and visible: the box is drawn in the editor and
// an operator who wants bigger digits can see they need a taller box.
describe('the Size control is not capped by a number nobody chose', () => {
  const val = rule('.lp-val');

  it('still takes the smallest of the declared size and the two real caps', () => {
    expect(val).toMatch(/font-size:\s*min\(/);
    expect(val, 'the declared size is no longer a term').toMatch(/var\(--lp-sz/);
    expect(val, 'the per-column budget went with the ceiling').toMatch(/var\(--tch/);
    expect(val, 'the rail height cap went with the ceiling').toMatch(/var\(--lp-room/);
  });

  it('but the per-column budget has no hard upper bound any more', () => {
    // A `clamp(min, …, MAX)` here is a ceiling on the DESIGNER, not on the fit.
    // The floor stays: below 12px nothing is readable from a platform.
    //
    // THE WHOLE DECLARATION, not a sub-match. My first version of this matched
    // `/(clamp|max)\([^;]*?--tch[^;]*?\)/` — non-greedy, so it stopped at the
    // `)` closing `var(--tch, 6)` and never saw the ceiling it was written to
    // find. It passed against the broken file, which is the scanner-narrowing
    // failure this repository records under four other names.
    const decl = val.slice(val.indexOf('font-size:'));
    expect(decl, 'this test is reading nothing').toMatch(/--tch/);
    expect(decl, 'a literal ceiling is back — see the measurement above').not.toMatch(
      /clamp\([^;]*?,\s*\d+px\s*\)/s,
    );
    expect(decl, 'the 12px floor was lost with it').toMatch(/12px/);
  });
});
