// RG-18 — can the back row read this?
//
// Two of the three answers are arithmetic and the third is not, and the third is
// the one these tests care about most: **"Relay cannot check this" is not a pass.**
// A green tick over an unreadable verse is the same class of harm as a status badge
// that cannot detect its own failure (DECISIONS §39).
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { parseColor, contrastRatio, effectiveBackground, checkContrast, checkDistance, textHeightMetres, previewScale, review, CONTRAST_FLOOR, PREVIEW_DISTANCES_M, reviewTemplate, styleOfTemplate } from './legibility.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

/**
 * THE HIGH VISIBILITY LOOK AS THE MIGRATION INLINES IT — FROZEN BYTES, AND THE
 * BLOCK BELOW SAYS SO RATHER THAN IMPLYING MORE.
 *
 * `legacy_themes.json` is a SNAPSHOT: `db/templates.rs` reads the same bytes
 * through `include_str!` and `ensure_themes_are_inlined` writes this style into
 * every template that pinned theme -9. The file can never change — that is what
 * a snapshot is for — so these assertions cannot spontaneously go red, and they
 * are deliberately NOT a claim that anything a church can pick today is legible.
 *
 * What they DO guard is real and is the migration: the numbers a church that
 * already chose High Visibility now carries inside their own template. If
 * somebody edits this file, or the migration's whitelist stops carrying a key
 * these assertions read, this is what notices — and the failure would be a look
 * changing on an update, which is the one promise `ensure_themes_are_inlined`
 * makes.
 *
 * It does NOT cover the selection. High Visibility was picked by choosing a
 * theme; themes are gone (DECISIONS §87) and no shelf template replaces it yet.
 * That is RG-37, which is PARTIAL for exactly this reason, and it is not a hole
 * this file can fill — a test cannot assert a template nobody has seeded.
 */
const HIGH_VIS = JSON.parse(
  fs.readFileSync(path.join(ROOT, 'src-tauri/data/legacy_themes.json'), 'utf8'),
).themes.find((t) => t.name === 'High Visibility');

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

describe('High Visibility, as the migration preserves it (frozen snapshot bytes)', () => {
  const hv = HIGH_VIS;

  it('is a style a template can carry, not a rendering branch', () => {
    // A parallel "accessibility mode" would be the `if channel_type ==` shape
    // CLAUDE.md forbids, and would need a decision at every render site. As a
    // style it reaches the wall, the stage monitor, the lower third and the
    // editor preview on the day it is selected, through the one renderer.
    expect(hv).toBeTruthy();
    // Every key is one `TemplateRender` already reads off a template's `style` —
    // which is exactly why the theme layer beneath templates had nothing left to
    // say, and why inlining it changed no look.
    expect(Object.keys(hv.style).length).toBeGreaterThan(0);
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

  it('is larger than the look Relay opened with', () => {
    const modern = JSON.parse(
      fs.readFileSync(path.join(ROOT, 'src-tauri/data/legacy_themes.json'), 'utf8'),
    ).themes.find((t) => t.name === 'Modern Dark');
    expect(Number(hv.style.verseSize)).toBeGreaterThan(Number(modern.style.verseSize));
  });
});

// ── THE LAYER MODEL IS THE MODEL. ────────────────────────────────────────────
//
// `checkContrast` and `checkDistance` read `style.verseColor`, `style.refColor`,
// `style.background` and `style.verseSize`. In the layer model those live on
// layers, and every shipped template leaves `style` EMPTY — all eight shelf
// entries carry `style: {}`.
//
// So the one tool in the product that answers "can the back row read this"
// returned `unknown` for ELEVEN OF THIRTEEN shipped templates, including all three
// lower thirds and both composites. Worse than inert: `unknown` renders in the
// panel's reassuring branch, so a template with a real contrast failure sat in the
// same state as one nobody had checked.
//
// The first three below were watched to fail by calling `review(t.style ?? {}, …)`
// instead of `reviewTemplate(t, …)` — which is exactly what the editor used to do.
// The last three pass either way, deliberately: they hold the REFUSALS, so the
// adapter cannot be "improved" later into guessing at a composite or a gradient.
describe('review answers for a layered template', () => {
  const ROOM = { screenWidthM: 4, backRowM: 18 };
  const shelf = JSON.parse(
    fs.readFileSync(path.resolve(process.cwd(), 'src-tauri/data/shelf_templates.json'), 'utf8'),
  );
  const list = Array.isArray(shelf) ? shelf : (shelf.templates ?? []);
  const byName = (n) => list.find((t) => t.name === n);

  it('a background + text template is fully answered', () => {
    const r = reviewTemplate(byName('Scripture · Meridian'), null, ROOM);
    expect(r.unknowns, 'every value is derivable from its layers').toBe(0);
    expect(r.verse.state).toBe('ok');
    expect(r.reference.state).toBe('ok');
  });

  it('a BAND template reads its ground off the band, which is what a caption bar is', () => {
    // A lower third has no background layer on purpose — the rest of the frame is
    // a camera Relay does not control. Before this, that meant `unknown`.
    //
    // Was `Lower Third · Scripture`, then `Lower Third · Lyric`; wave 5 rebuilt
    // the shelf and the bands are the five `Scroll · …` lower thirds.
    // `Scroll · Banner` exercises the same adapter path — the ground comes off
    // the band's own fill either way. The lookup is asserted first because
    // `reviewTemplate(undefined)` answers `unknown`, which is exactly what this
    // test forbids: a missing subject would have read as the very failure being
    // guarded against.
    const tpl = byName('Scroll · Banner');
    expect(tpl, 'the shelf has no band left to review').toBeTruthy();
    const r = reviewTemplate(tpl, null, ROOM);
    expect(r.verse.state).not.toBe('unknown');

    // THE REFERENCE HALF, RESTORED. It used to ride on the same shelf entry, and
    // a `Scroll · …` band cannot carry it: a crawling notice has no reference-bound
    // layer at all, on purpose (`rhide` in the prototype — a song's reference is
    // its title, and a title under every line reads like a slide rather than a
    // caption). Dropping the assertion with the template would have left NOTHING
    // anywhere holding "a band template's reference is derivable", which is half
    // the bug this block records — and `unknown` renders in the panel's
    // reassuring branch, so the loss would have looked exactly like a pass
    // (rule 35). The stack below is the smallest thing that has the shape: a band
    // with a reference inside it and no background layer, which is what a
    // scripture caption bar is.
    const bandWithRef = {
      name: 'a band that names its reference',
      layout: {
        align: 'left',
        layers: [
          { id: 'b', type: 'band', name: 'Band', x: 5, y: 62, w: 90, h: 38, top: 62, side: 5, pad: 3, lift: 4.4, grow: 16, members: ['v', 'r'], fill: '#0a0906', opacity: 0.9, radius: 0 },
          { id: 'v', type: 'text', name: 'Verse', bind: 'verse', x: 8, y: 70, w: 84, h: 14, font: 'var(--f-serif)', color: '#fff7e8', size: 3, align: 'left', valign: 'middle', lineHeight: 1.2, letterSpacing: 0, shadow: 0, italic: false },
          { id: 'r', type: 'text', name: 'Reference', bind: 'reference', x: 8, y: 85, w: 84, h: 6, font: 'var(--f-serif)', color: '#e6dccb', size: 1.7, align: 'right', valign: 'middle', lineHeight: 1.1, letterSpacing: 0.08, shadow: 0, italic: false },
        ],
      },
      style: {},
    };
    const withRef = reviewTemplate(bandWithRef, null, ROOM);
    expect(withRef.verse.state, 'the band is still the ground for its verse').not.toBe('unknown');
    expect(withRef.reference.state, 'a band reference must be derivable too').not.toBe('unknown');
  });

  it('…and the shelf is no longer mostly unanswerable', () => {
    // Thirty of forty since wave 5 rebuilt the shelf, up from three of seven.
    // The floor moves with the shelf rather than the claim being quietly dropped —
    // what it holds is that the adapter answers for the great majority of what a
    // fresh install can actually check. The ten it cannot are honest refusals:
    // the five composites (their words are a real inner template at a different
    // width), and the five looks with no reference-bound layer to answer for.
    const answered = list.filter((t) => reviewTemplate(t, null, ROOM).unknowns === 0);
    expect(answered.length, 'most of the shelf must now be checkable').toBeGreaterThanOrEqual(25);
  });

  it('a composite is REFUSED rather than guessed at', () => {
    // Its words are a real inner template at a different width, so answering from
    // the outer one would be a guess with a number on it.
    expect(reviewTemplate(byName('Source · Word right'), null, ROOM).verse.state).toBe('unknown');
  });

  it('and a gradient is still honestly unknown, not quietly passed', () => {
    // The file's own stance, and the half that must not be lost to the adapter.
    expect(reviewTemplate(byName('Notice Board'), null, ROOM).verse.state).toBe('unknown');
  });

  it('a flat-style template is untouched', () => {
    const flat = { style: { verseColor: '#ffffff', background: '#000000', verseSize: '5.5' } };
    expect(styleOfTemplate(flat)).toEqual(flat.style);
  });
});
