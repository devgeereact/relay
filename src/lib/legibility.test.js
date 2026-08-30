// RG-18 — can the back row read this?
//
// Two of the three answers are arithmetic and the third is not, and the third is
// the one these tests care about most: **"Relay cannot check this" is not a pass.**
// A green tick over an unreadable verse is the same class of harm as a status badge
// that cannot detect its own failure (DECISIONS §39).
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import {
  parseColor,
  contrastRatio,
  effectiveBackground,
  checkContrast,
  checkDistance,
  textHeightMetres,
  previewScale,
  review,
  CONTRAST_FLOOR,
  PREVIEW_DISTANCES_M,
} from './legibility.js';
import { BUILTIN_THEMES } from './themes.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

describe('the arithmetic', () => {
  it('reads the colour formats templates actually use', () => {
    expect(parseColor('#fff')).toEqual({ r: 255, g: 255, b: 255, a: 1 });
    expect(parseColor('#22d3ee')).toMatchObject({ r: 34, g: 211, b: 238 });
    expect(parseColor('rgba(0,0,0,0.5)')).toMatchObject({ r: 0, g: 0, b: 0, a: 0.5 });
  });

  it('returns NULL for anything it cannot evaluate', () => {
    // A gradient, a CSS variable, a named colour. Each is a real answer — "I cannot
    // compute this" — and must not be silently treated as black or white.
    for (const c of ['radial-gradient(120% 140% at 50% 30%, #16181d, #06070a)', 'var(--v-txt)', 'transparent', '', null, 12])
      expect(parseColor(c)).toBeNull();
  });

  it('computes WCAG contrast, checked against the known extremes', () => {
    const w = { r: 255, g: 255, b: 255 };
    const b = { r: 0, g: 0, b: 0 };
    expect(contrastRatio(w, b)).toBeCloseTo(21, 1);
    expect(contrastRatio(w, w)).toBeCloseTo(1, 5);
  });

  it('composites the dim scrim and the plate onto the background', () => {
    // The scrim is the whole reason a bright template can still be readable, and
    // ignoring it would flag designs that work.
    const plain = effectiveBackground({ background: '#ffffff' });
    expect(plain).toMatchObject({ r: 255, g: 255, b: 255 });
    const dimmed = effectiveBackground({ background: '#ffffff', bgDim: 0.5 });
    expect(dimmed.r).toBeCloseTo(127.5, 1);
  });
});

describe('what it refuses to guess', () => {
  it('says it cannot check contrast over a picture or a video', () => {
    const c = checkContrast(
      { background: '#000000', verseColor: '#ffffff' },
      { media_url: 'http://x/media/3' },
    );
    expect(c.state).toBe('unknown');
    expect(c.ratio).toBeNull();
    expect(c.note).toMatch(/only your eyes can/);
  });

  it('says it cannot check a gradient background', () => {
    const c = checkContrast({ background: 'linear-gradient(#000,#fff)', verseColor: '#fff' });
    expect(c.state).toBe('unknown');
  });

  it('unknown is counted as its own thing — not as a problem and not as a pass', () => {
    const r = review({ background: 'var(--x)', verseColor: '#fff', refColor: '#fff' }, null, {});
    expect(r.unknowns).toBe(3); // both colours plus the distance, with no room given
    expect(r.problems).toBe(0);
  });

  it('gives no distance verdict without the two numbers only a person can know', () => {
    // Nothing in software can know how big a projected image is or how far back the
    // last row sits. No numbers, no verdict.
    expect(checkDistance({ verseSize: '6' }, {}).state).toBe('unknown');
    expect(checkDistance({ verseSize: '6' }, { screenWidthM: 4 }).state).toBe('unknown');
    expect(checkDistance({}, { screenWidthM: 4, backRowM: 18 }).state).toBe('unknown');
  });
});

describe('the verdicts it does give', () => {
  it('passes white on black and fails grey on grey', () => {
    expect(checkContrast({ background: '#000000', verseColor: '#ffffff' }).state).toBe('ok');
    const low = checkContrast({ background: '#767676', verseColor: '#8a8a8a' });
    expect(low.state).toBe('low');
    expect(low.ratio).toBeLessThan(CONTRAST_FLOOR);
    expect(low.note).toMatch(/hard to read/);
  });

  it('holds a wall to the LARGE-text ratio, not the body-text one', () => {
    // A verse on a projector is enormous text by definition. Holding it to 4.5
    // would flag designs that read perfectly well from the back of a hall.
    expect(CONTRAST_FLOOR).toBe(3);
  });

  it('turns cqw into centimetres a person can picture', () => {
    // 6cqw on a 4m screen ≈ 4 × 0.06 × 0.7 = 0.168m.
    expect(textHeightMetres('6', 4)).toBeCloseTo(0.168, 3);
    expect(textHeightMetres('0', 4)).toBeNull();
    expect(textHeightMetres('6', 0)).toBeNull();
  });

  it('says a verse is too small for the back row, and what to do', () => {
    const small = checkDistance({ verseSize: '3' }, { screenWidthM: 3, backRowM: 25 });
    expect(small.state).toBe('small');
    expect(small.note).toMatch(/larger verse size/);
    const ok = checkDistance({ verseSize: '8' }, { screenWidthM: 5, backRowM: 15 });
    expect(ok.state).toBe('ok');
  });
});

describe('stepping back', () => {
  it('shrinks by the ratio of the distances — what a person does when they walk away', () => {
    expect(previewScale(5)).toBe(1);
    expect(previewScale(10)).toBeCloseTo(0.5, 5);
    expect(previewScale(20)).toBeCloseTo(0.25, 5);
  });

  it('never magnifies, and survives nonsense', () => {
    expect(previewScale(1)).toBe(1);
    expect(previewScale(0)).toBe(1);
    expect(previewScale('x')).toBe(1);
  });

  it('offers the four distances the brief asked for', () => {
    expect(PREVIEW_DISTANCES_M).toEqual([5, 10, 15, 20]);
  });
});

describe('the thresholds are reference points, and it says so', () => {
  it('carries the caveat with the verdict, not in a document', () => {
    // The person reading it is deciding whether to trust it right now. Neither WCAG
    // (a spec for screens at arm's length) nor the broadcast character-height rule
    // has been checked against a projector in a church — that is Stage B.
    const r = review({ background: '#000', verseColor: '#fff' }, null, {});
    expect(r.caveat).toMatch(/Neither has been checked against a projector/i);
    expect(r.caveat).toMatch(/worth looking at rather than as a verdict/);
    expect(read('src/lib/views/templates/TemplateEditor.svelte')).toMatch(/legible\.caveat/);
  });
});

describe('High Visibility is a THEME, not a mode', () => {
  const hv = BUILTIN_THEMES.find((t) => t.name === 'High Visibility');

  it('exists as a built-in theme', () => {
    // A parallel "accessibility mode" would be the `if channel_type ==` shape
    // CLAUDE.md forbids, and would need a decision at every render site. As a theme
    // it reaches the wall, the stage monitor, the lower third and the editor
    // preview on the day it is selected (DECISIONS §27).
    expect(hv).toBeTruthy();
    expect(hv.builtin).toBe(true);
    expect(hv.id).toBeLessThan(0); // built-in ids are negative and cannot collide
  });

  it('is the highest contrast a projector can make', () => {
    const c = checkContrast(hv.style, null, 'verse');
    expect(c.state).toBe('ok');
    expect(c.ratio).toBeCloseTo(21, 0);
  });

  it('does not spend contrast on a coloured reference', () => {
    // A coloured reference on a black ground is the first thing to disappear for
    // somebody with low vision or colour blindness, and it is the least important
    // text on the screen.
    expect(hv.style.refColor).toBe(hv.style.verseColor);
    expect(checkContrast(hv.style, null, 'ref').ratio).toBeCloseTo(21, 0);
  });

  it('removes the shadow and the transition', () => {
    // A soft edge IS a contrast reduction, and a transition long enough to notice
    // is one somebody can be disoriented by.
    expect(hv.style.verseShadow).toBe('0');
    expect(hv.style.transitionMs).toBe('0');
  });

  it('is larger than the default theme', () => {
    const modern = BUILTIN_THEMES.find((t) => t.name === 'Modern Dark');
    expect(Number(hv.style.verseSize)).toBeGreaterThan(Number(modern.style.verseSize));
  });
});
