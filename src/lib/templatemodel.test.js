// Phase 2 of the rebrand (docs/REBRAND.md §3.1, §3.4): one property, one home.
//
// The defect this closes is not a crash. `style.font` was a WHOLE-TEMPLATE font
// that `verseFont` and `refFont` fell back to, so a template could carry the
// same fact in two places — and the editor showed one of them while writing the
// other. A property with two homes is a property that drifts: the preview and
// the wall read different keys, both "work", and the disagreement only shows up
// on a Sunday.
//
// So the model gets one home each, a migration that DELETES the legacy key after
// writing the per-element ones, and a resolver that fills the rest. The test for
// a migration is not that it runs — it is that running it twice is the same as
// running it once, and that it never overwrites a value a person chose.
import { describe, it, expect } from 'vitest';
import {
  LEGACY_STYLE_KEYS,
  STYLE_DEFAULTS,
  migrateStyle,
  migrateTemplate,
  resolveStyle,
  BG_STYLES,
  slideBG,
  FACE_ADVANCE,
  faceOf,
  estimateLines,
  fitScale,
} from './templatemodel.js';

describe('§3.1 · migrateStyle — the legacy keys leave, the facts stay', () => {
  it('moves a whole-template font onto both elements and deletes it', () => {
    const out = migrateStyle({ font: 'Fraunces', verseSize: 6 });
    expect(out.verseFont).toBe('Fraunces');
    expect(out.refFont).toBe('Fraunces');
    expect(out.font).toBeUndefined();
    expect(out.verseSize).toBe(6);
  });

  it('moves a whole-template shadow onto both elements and deletes it', () => {
    const out = migrateStyle({ textShadow: 0.6 });
    expect(out.verseShadow).toBe(0.6);
    expect(out.refShadow).toBe(0.6);
    expect(out.textShadow).toBeUndefined();
  });

  it('never overwrites a per-element value somebody chose', () => {
    // This is the half that makes a migration safe to run on every load: a
    // template already edited under the new model must come out unchanged.
    const out = migrateStyle({ font: 'Inter', verseFont: 'Fraunces', textShadow: 0.5, refShadow: 0 });
    expect(out.verseFont).toBe('Fraunces');
    expect(out.refFont).toBe('Inter');
    expect(out.verseShadow).toBe(0.5);
    expect(out.refShadow).toBe(0);
  });

  it('is idempotent — twice is once', () => {
    const once = migrateStyle({ font: 'Fraunces', textShadow: 0.4 });
    const twice = migrateStyle(once);
    expect(twice).toEqual(once);
  });

  it('does not mutate what it was given', () => {
    const before = { font: 'Fraunces' };
    migrateStyle(before);
    expect(before.font).toBe('Fraunces');
  });

  it('a shadow of 0 is a choice, not an absence', () => {
    // `??` rather than `||`: a designer who turned the shadow off means it.
    const out = migrateStyle({ textShadow: 0 });
    expect(out.verseShadow).toBe(0);
    expect(out.refShadow).toBe(0);
  });

  it('every legacy key it knows about is gone afterwards', () => {
    const seeded = Object.fromEntries(LEGACY_STYLE_KEYS.map((k) => [k, 'x']));
    const out = migrateStyle(seeded);
    for (const k of LEGACY_STYLE_KEYS) expect(out[k]).toBeUndefined();
  });

  it('maps the three old transition names onto the register', () => {
    // A saved theme holds whichever of `fade` / `slide` / `zoom` its operator
    // chose in the old picker. Dropping them would turn every one of those
    // themes into a cut — losing a choice somebody made, silently, while
    // looking like the upgrade worked.
    expect(migrateStyle({ transition: 'fade' }).transition).toBe('crossfade');
    expect(migrateStyle({ transition: 'slide' }).transition).toBe('slideup');
    expect(migrateStyle({ transition: 'zoom' }).transition).toBe('materialise');
  });

  it('leaves a transition that is already current alone, and twice is once', () => {
    expect(migrateStyle({ transition: 'pushleft' }).transition).toBe('pushleft');
    const once = migrateStyle({ transition: 'fade' });
    expect(migrateStyle(once)).toEqual(once);
  });

  it('survives a template with no style at all', () => {
    expect(migrateStyle(undefined)).toEqual({});
    expect(migrateStyle(null)).toEqual({});
  });
});

describe('§3.1 · migrateTemplate — the whole record, not just the style', () => {
  it('migrates the style and leaves everything else alone', () => {
    const t = { id: 4, name: 'Nocturne', layout: { regions: ['verse'] }, style: { font: 'Fraunces' } };
    const out = migrateTemplate(t);
    expect(out.id).toBe(4);
    expect(out.name).toBe('Nocturne');
    expect(out.layout).toEqual({ regions: ['verse'] });
    expect(out.style.verseFont).toBe('Fraunces');
    expect(out.style.font).toBeUndefined();
  });

  it('hands back a falsy template unchanged rather than inventing one', () => {
    expect(migrateTemplate(null)).toBe(null);
    expect(migrateTemplate(undefined)).toBe(undefined);
  });
});

describe('§3.1 · resolveStyle — the rest of the properties, in one place', () => {
  it('fills every default the renderer used to inline', () => {
    const r = resolveStyle({});
    expect(r.verseSize).toBe(STYLE_DEFAULTS.verseSize);
    expect(r.refSize).toBe(STYLE_DEFAULTS.refSize);
    expect(r.verseLineHeight).toBe(1.32);
    expect(r.refGap).toBe(1.4);
    expect(r.verseTransform).toBe('none');
  });

  it('does not answer for background or alignment', () => {
    // Both have a better answer elsewhere: an unset background is transparent,
    // and alignment falls back to `layout.align` first.
    const r = resolveStyle({});
    expect(r.background).toBeUndefined();
    expect(r.verseAlign).toBeUndefined();
    expect(r.refAlign).toBeUndefined();
  });

  it('a stored value always beats the default', () => {
    const r = resolveStyle({ verseSize: 9, verseTransform: 'uppercase' });
    expect(r.verseSize).toBe(9);
    expect(r.verseTransform).toBe('uppercase');
  });

  it('migrates on the way through, so a legacy template resolves correctly', () => {
    const r = resolveStyle({ font: 'Fraunces' });
    expect(r.verseFont).toBe('Fraunces');
    expect(r.font).toBeUndefined();
  });

  it('reads a number written as a string, which is what an <input> gives you', () => {
    const r = resolveStyle({ verseSize: '7.5', verseLineHeight: '1.1' });
    expect(r.verseSize).toBe(7.5);
    expect(r.verseLineHeight).toBe(1.1);
  });

  it('an empty string is an absent value, not a zero', () => {
    // A cleared number input yields ''. Number('') is 0, which would silently
    // set a verse to 0cqw — an invisible verse, on a wall, with no error.
    const r = resolveStyle({ verseSize: '', refGap: '' });
    expect(r.verseSize).toBe(STYLE_DEFAULTS.verseSize);
    expect(r.refGap).toBe(1.4);
  });

  it('a size of zero is refused — an invisible verse is not a choice', () => {
    // The renderer read `parseFloat(style.verseSize) || 6`, and that `||` was
    // doing two jobs: filling an absent size AND rejecting a zero. An honest
    // "is this a number" check keeps the first and drops the second, which is
    // how a template from an import renders a verse at 0cqw — nothing on the
    // wall, and no error anywhere.
    expect(resolveStyle({ verseSize: 0 }).verseSize).toBe(STYLE_DEFAULTS.verseSize);
    expect(resolveStyle({ refSize: 0 }).refSize).toBe(STYLE_DEFAULTS.refSize);
    expect(resolveStyle({ verseSize: -4 }).verseSize).toBe(STYLE_DEFAULTS.verseSize);
    expect(resolveStyle({ verseLineHeight: 0 }).verseLineHeight).toBe(1.32);
  });

  it('a zero somebody typed is kept where zero is meaningful', () => {
    expect(resolveStyle({ refGap: 0 }).refGap).toBe(0);
    expect(resolveStyle({ verseShadow: 0 }).verseShadow).toBe(0);
  });
});

describe('§3.1 · slideBG — the five background styles, in one register', () => {
  it('names exactly the five the spec asks for', () => {
    expect(BG_STYLES.map((b) => b.id)).toEqual(['solid', 'vfade', 'glow', 'diagonal', 'vignette']);
  });

  it('every style has a label an operator can read', () => {
    for (const b of BG_STYLES) expect(b.label.length).toBeGreaterThan(2);
  });

  it('solid is the flat colour', () => {
    expect(slideBG({ background: '#101010', bgStyle: 'solid' })).toBe('#101010');
  });

  it('an unknown style falls back to solid rather than painting nothing', () => {
    // A template from a newer version, or a typo in an import: a background that
    // resolves to empty is a black screen with text that may not contrast.
    expect(slideBG({ background: '#101010', bgStyle: 'nonsense' })).toBe('#101010');
  });

  it('each gradient style carries the template colour', () => {
    for (const b of BG_STYLES.filter((x) => x.id !== 'solid')) {
      const css = slideBG({ background: '#123456', bgStyle: b.id });
      expect(css).toContain('gradient');
      expect(css.toLowerCase()).toContain('18, 52, 86');
    }
  });

  it('is null when the template names no background, so the caller can be transparent', () => {
    // A lower third is keyed over a live camera. A resolver that filled in a
    // dark colour here would black out a stream for every template that never
    // picked one — an absence that MEANS something cannot be defaulted away.
    expect(slideBG({})).toBe(null);
    expect(slideBG({ bgStyle: 'glow' })).toBe(null);
  });

  it('leaves a template that already stores a gradient string alone', () => {
    // `background` has always been able to hold raw CSS. Wrapping that in
    // another gradient would produce an invalid value and paint nothing.
    const raw = 'linear-gradient(180deg,#000,#222)';
    expect(slideBG({ background: raw })).toBe(raw);
    expect(slideBG({ background: raw, bgStyle: 'glow' })).toBe(raw);
  });
});

describe('§3.4 · the fit estimate — measured in cqw, per face', () => {
  it('publishes the three advances the spec measured', () => {
    expect(FACE_ADVANCE).toEqual({ mono: 0.62, serif: 0.49, sans: 0.52 });
  });

  it('recognises a face from a family name or a token', () => {
    expect(faceOf('var(--f-mono)')).toBe('mono');
    expect(faceOf('IBM Plex Mono')).toBe('mono');
    expect(faceOf('var(--f-serif)')).toBe('serif');
    expect(faceOf('Fraunces')).toBe('serif');
    expect(faceOf('Inter')).toBe('sans');
    expect(faceOf(undefined)).toBe('serif'); // the renderer's own default face
  });

  it('a wider line holds more characters, so it needs fewer lines', () => {
    const narrow = estimateLines({ text: 'x'.repeat(200), size: 6, face: 'serif', widthPct: 50 });
    const wide = estimateLines({ text: 'x'.repeat(200), size: 6, face: 'serif', widthPct: 100 });
    expect(wide).toBeLessThan(narrow);
  });

  it('a bigger face needs more lines for the same words', () => {
    const small = estimateLines({ text: 'x'.repeat(200), size: 4, face: 'serif' });
    const big = estimateLines({ text: 'x'.repeat(200), size: 8, face: 'serif' });
    expect(big).toBeGreaterThan(small);
  });

  it('empty text is zero lines, not one', () => {
    expect(estimateLines({ text: '', size: 6, face: 'serif' })).toBe(0);
  });

  it('a short verse in a 16:9 frame does not shrink at all', () => {
    const s = fitScale({ text: 'For God so loved the world.', size: 6, aspect: 16 / 9, face: 'serif' });
    expect(s).toBe(1);
  });

  it('a long passage in the same frame does shrink', () => {
    const s = fitScale({ text: 'word '.repeat(400), size: 6, aspect: 16 / 9, face: 'serif' });
    expect(s).toBeLessThan(1);
    expect(s).toBeGreaterThan(0);
  });

  it('the SAME text shrinks further in a shallower frame', () => {
    // This is the whole reason the estimate takes an aspect: a band in a lower
    // third, a stage reading and a SuperSource word region are not 16:9, and a
    // fitter that assumes they are reports a fit that is not true of the box it
    // is actually in.
    const wide = fitScale({ text: 'word '.repeat(120), size: 6, aspect: 16 / 9, face: 'serif' });
    const band = fitScale({ text: 'word '.repeat(120), size: 6, aspect: 16 / 2, face: 'serif' });
    expect(band).toBeLessThan(wide);
  });

  it('never returns zero, however much text it is given', () => {
    // A scale of 0 is an invisible verse. The floor is the reported failure
    // (rule 37): shrink, show it, and let the caller say it went too small.
    const s = fitScale({ text: 'word '.repeat(5000), size: 6, aspect: 16 / 9, face: 'serif' });
    expect(s).toBeGreaterThan(0);
  });

  it('a missing aspect is treated as 16:9 rather than as a divide by zero', () => {
    expect(fitScale({ text: 'short', size: 6, face: 'serif' })).toBe(1);
    expect(Number.isFinite(fitScale({ text: 'short', size: 6, aspect: 0, face: 'serif' }))).toBe(true);
  });
});
