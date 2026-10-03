// THE TIMER IS THE LARGEST FIGURE ON THE PREACHER'S SCREEN (RG-243).
//
// The operator: *"Time is not the main important thing on preachers screen...
// Timer is"*.
//
// Every figure on the row was one size, by construction: `--ch` is the widest
// rendered string on the row and `.fig .figv` divides by it, so the wall clock
// was exactly as large as the countdown a preacher is working to. A screen that
// gives equal weight to the time of day and the time remaining has not decided
// what it is for.
//
// **The clock is not removed — it is made secondary**, and it was already
// switchable (`zones.clock`). What changes is its weight.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const SRC = readFileSync(resolve(__dirname, '../Stage.svelte'), 'utf8');

/** The body of one CSS rule, by selector, so a match cannot come from prose. */
function rule(selector) {
  const i = SRC.indexOf(`\n  ${selector} {`);
  expect(i, `the rule \`${selector}\` is gone — this test is reading nothing`).toBeGreaterThan(-1);
  return SRC.slice(i, SRC.indexOf('\n  }', i) + 4);
}

describe('the clock is secondary to the timer', () => {
  it('the clock carries a class that says so', () => {
    // A modifier on the CELL, not a second figure list: the row is built from
    // one list (`figCells`) and a second one is how the two get different
    // labels, which is the defect RG-213 closed here.
    expect(SRC, 'nothing marks the clock as secondary').toMatch(/class:secondary=\{/);
  });

  it('and it is DERIVED from the figure’s own size, not a second number', () => {
    // A literal here would be a second size to keep in step with the first, and
    // the two would drift — which is the shape of RG-223, where a literal
    // ceiling quietly beat the size somebody had chosen.
    // A SHARE OF THE FIGURE'S OWN COMPUTED SIZE. `em` would not do and the first
    // draft of this used it: `em` resolves against the PARENT's size, not the
    // size the rule above just computed, so `0.42em` would have been a share of
    // the row's inherited type and not of the figure at all.
    const secondary = rule('.fig.secondary .figv');
    expect(secondary).toMatch(/font-size:\s*calc\(var\(--figsz\)/);
    expect(rule('.fig .figv'), 'the shared size is not stated once').toMatch(/--figsz:/);
  });

  it('the timer itself is untouched — this makes the clock smaller, not the timer bigger', () => {
    // The distinction matters: growing the countdown would push it past the box
    // it is capped by, and `.fig .figv`'s cap is what stops a figure being
    // sliced (RG-147).
    expect(rule('.fig .figv')).toMatch(/52cqh/);
  });
});

describe('and the countdown comes first in the row', () => {
  it('the list puts it before the clock', () => {
    // `figureList` is the order the row renders in. A preacher reads left to
    // right; the thing they are working to is the thing they should reach first.
    const at = SRC.indexOf('$: figureList = [');
    expect(at).toBeGreaterThan(-1);
    const list = SRC.slice(at, SRC.indexOf('];', at));
    expect(list.indexOf("'countdown'")).toBeLessThan(list.indexOf("'clock'"));
    expect(list.indexOf("'clip'"), 'the clip clock moved ahead of the countdown').toBeLessThan(
      list.indexOf("'countdown'"),
    );
  });
});
