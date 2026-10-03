// AN OBJECT MAY HANG OFF THE SLIDE, AND BE CROPPED BY IT (RG-233).
//
// The operator: *"Items should be able to overflow the canva croping unwanted
// sections out"*.
//
// Everything was clamped to 0–100: the drag stopped at the edge, the number
// fields refused anything outside, and the nudge keys did the same. So the two
// ordinary ways of using a bleed were impossible — a band or a picture pushed
// past the edge so no seam shows, and a large word cropped deliberately.
//
// **The bound is now generous, not absent.** A layer may sit half off, and it
// may not be sent somewhere nobody can find it: at −100 an object is entirely
// outside the slide with no handle on the canvas to drag it back, and the only
// way home would be to type a number into a panel for a thing you cannot see.
import { describe, it, expect } from 'vitest';
import { OVERFLOW_MIN, OVERFLOW_MAX, clampGeom } from './geombounds.js';

describe('clampGeom — how far off the slide an object may go', () => {
  it('leaves anything inside the slide exactly alone', () => {
    for (const v of [0, 1.5, 50, 99.4, 100]) expect(clampGeom(v)).toBe(v);
  });

  it('allows a bleed on both edges', () => {
    expect(clampGeom(-20)).toBe(-20);
    expect(clampGeom(120)).toBe(120);
  });

  it('but never so far that the object cannot be caught again', () => {
    expect(clampGeom(-9999)).toBe(OVERFLOW_MIN);
    expect(clampGeom(9999)).toBe(OVERFLOW_MAX);
    // Half the slide either way: at the bound an object still has a corner on
    // the canvas to grab, which is the whole reason there is a bound.
    expect(OVERFLOW_MIN).toBeLessThan(0);
    expect(OVERFLOW_MAX).toBeGreaterThan(100);
    expect(OVERFLOW_MIN).toBeGreaterThanOrEqual(-50);
    expect(OVERFLOW_MAX).toBeLessThanOrEqual(150);
  });

  it('and nonsense is refused rather than turned into a number', () => {
    for (const v of [NaN, undefined, null, 'x']) expect(clampGeom(v)).toBeNull();
  });
});
