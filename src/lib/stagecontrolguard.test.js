// THE ONE CONTROL LEFT ON THE PHONE OPENS ON A HOLD (RG-242).
//
// The operator: *"Also the control is good but should be perfected and solid"*.
//
// The Control panel — Prev, Next and verse search, which FIRE to every screen in
// the building — opened on a single tap of a 26px button in the header of a page
// a preacher holds mid-sermon. A sleeve, a thumb resting on the edge, a phone
// picked up off a lectern: one contact and the panel is open over the reading.
//
// A hold is the ordinary answer and the one the platform already teaches. It is
// NOT a confirmation dialog: rule 41 forbids a native one and an in-app one
// would be a second thing to dismiss while the congregation waits.
//
// **What this does not touch**: rule 44. The panel still consumes Escape and
// still may not cover a panic control — that is `panicoverlay.test.js`'s
// subject and it keeps it.
import { describe, it, expect } from 'vitest';
import { HOLD_MS, holdGuard } from './holdguard.js';

describe('holdGuard — a press that has to be meant', () => {
  it('a quick tap does not open it', () => {
    const g = holdGuard();
    g.down(1_000);
    expect(g.up(1_000 + 120), 'a brush of a sleeve opened the panel').toBe(false);
  });

  it('a held press does', () => {
    const g = holdGuard();
    g.down(1_000);
    expect(g.up(1_000 + HOLD_MS + 10)).toBe(true);
  });

  it('exactly the threshold counts as held, so the boundary is not a coin toss', () => {
    const g = holdGuard();
    g.down(0);
    expect(g.up(HOLD_MS)).toBe(true);
  });

  it('a release with no press is not a hold', () => {
    // A pointer that entered the button already down — dragged from elsewhere on
    // the screen — never pressed it.
    expect(holdGuard().up(9_999)).toBe(false);
  });

  it('and a cancelled press is forgotten, not banked', () => {
    // Otherwise a press cancelled by a scroll would count toward the NEXT tap,
    // and the guard would let a real brush through.
    const g = holdGuard();
    g.down(1_000);
    g.cancel();
    expect(g.up(1_000 + HOLD_MS + 500)).toBe(false);
  });

  it('the threshold is long enough to be deliberate and short enough to use', () => {
    expect(HOLD_MS).toBeGreaterThanOrEqual(400);
    expect(HOLD_MS).toBeLessThanOrEqual(1000);
  });
});
