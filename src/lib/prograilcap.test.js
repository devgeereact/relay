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
    expect(SRC).toMatch(/railHeightVar\(progH\[i\]\)/);
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
