// Phase 8 of the rebrand (docs/REBRAND.md §8): seven transitions that apply.
//
// What this holds is not "the animation looks nice" — it is the two ways a
// transition damages a service:
//
//   1. It never finishes cleanly. A mode that ends at opacity 0.98 or with a
//      leftover transform leaves the verse slightly wrong for as long as it is
//      on the wall, and nobody can say why the screen looks soft.
//   2. It animates something that costs layout. The auto-fit measures
//      `scrollHeight` against `clientHeight`; a transition on width, padding or
//      font-size makes it measure a shape the slide will not settle at. That is
//      why transitions were REMOVED from this renderer once already.
import { describe, it, expect } from 'vitest';
import {
  TRANSITIONS,
  DEFAULT_TRANSITION,
  isTransition,
  transitionCss,
  transitionDuration,
} from './transitions.js';

const IDS = ['cut', 'crossfade', 'dissolve', 'fadeblack', 'pushleft', 'slideup', 'materialise'];

describe('the register', () => {
  it('is the seven the spec names, cut first', () => {
    expect(TRANSITIONS.map((t) => t.id)).toEqual(IDS);
    expect(DEFAULT_TRANSITION).toBe('cut');
  });

  it('every one has a label and a sentence an operator can read', () => {
    for (const t of TRANSITIONS) {
      expect(t.label.length, t.id).toBeGreaterThan(2);
      expect(t.hint.length, t.id).toBeGreaterThan(8);
    }
  });

  it('knows what it does not know', () => {
    expect(isTransition('crossfade')).toBe(true);
    expect(isTransition('sparkle')).toBe(false);
    expect(isTransition(undefined)).toBe(false);
  });
});

describe('transitionCss', () => {
  it('a cut has no intermediate state at all', () => {
    // Not "opacity:1" — an opacity here would make a cut a one-frame fade.
    for (const t of [0, 0.5, 1]) expect(transitionCss('cut', t)).toBe('');
    expect(transitionCss('nonsense', 0.5)).toBe('');
  });

  it('every animated mode ENDS settled — no residue at t=1', () => {
    // The failure this prevents is permanent and quiet: a slide that finishes a
    // transition 2% transparent, or 0.6cqw blurred, stays that way.
    for (const id of IDS.filter((i) => i !== 'cut')) {
      const end = transitionCss(id, 1);
      expect(end, id).toContain('opacity:1');
      if (end.includes('translateX')) expect(end, id).toContain('translateX(0');
      if (end.includes('translateY')) expect(end, id).toContain('translateY(0');
      if (end.includes('scale')) expect(end, id).toContain('scale(1');
      if (end.includes('blur')) expect(end, id).toContain('blur(0');
      if (end.includes('brightness')) expect(end, id).toContain('brightness(1');
    }
  });

  it('every animated mode STARTS hidden, so nothing pops in at full strength', () => {
    for (const id of IDS.filter((i) => i !== 'cut')) {
      expect(transitionCss(id, 0), id).toMatch(/opacity:0[;.]?/);
    }
  });

  it('animates ONLY opacity, transform and filter', () => {
    // The three that are composited and never trigger layout. Anything else
    // would move `scrollHeight` under the fitter mid-transition.
    const allowed = /^(opacity|transform|filter)$/;
    for (const id of IDS) {
      for (const t of [0, 0.25, 0.5, 0.75, 1]) {
        for (const decl of transitionCss(id, t).split(';')) {
          const prop = decl.split(':')[0].trim();
          if (!prop) continue;
          expect(allowed.test(prop), `${id} animates ${prop}`).toBe(true);
        }
      }
    }
  });

  it('clamps a progress outside 0–1 rather than overshooting', () => {
    expect(transitionCss('crossfade', -1)).toBe('opacity:0;');
    expect(transitionCss('crossfade', 5)).toBe('opacity:1;');
  });

  it('fade through black is dark first and revealed second', () => {
    expect(transitionCss('fadeblack', 0.25)).toContain('brightness(0)');
    expect(transitionCss('fadeblack', 0.25)).toContain('opacity:0');
    expect(transitionCss('fadeblack', 0.75)).toMatch(/opacity:0\.5/);
  });
});

describe('transitionDuration', () => {
  it('a cut takes no time', () => {
    expect(transitionDuration('cut', 400)).toBe(0);
  });

  it('reduced motion turns every transition into a cut', () => {
    // Not "a shorter animation" — the viewer asked for none.
    for (const id of IDS) expect(transitionDuration(id, 400, true), id).toBe(0);
  });

  it('an unknown mode is a cut rather than a guess', () => {
    expect(transitionDuration('sparkle', 400)).toBe(0);
  });

  it('honours the duration a theme asked for', () => {
    expect(transitionDuration('crossfade', 250)).toBe(250);
  });

  it('a missing or nonsense duration is a cut, not an instant flash', () => {
    expect(transitionDuration('crossfade', 0)).toBe(0);
    expect(transitionDuration('crossfade', null)).toBe(0);
    expect(transitionDuration('crossfade', 'soon')).toBe(0);
  });

  it('caps a transition at a second — longer is an operator waiting mid-service', () => {
    expect(transitionDuration('crossfade', 5000)).toBe(1000);
  });
});
