// THE LOWER-THIRD BAND, AND THE GROUND IT GIVES — docs/REBRAND.md §4.
//
// Phase 5 shipped three lower thirds and left "band-gives-ground" unbuilt, for a
// reason worth keeping in front of this file: a template here is a flat list of
// independently placed objects, with no parent, no child and no "these two
// belong together". The rejected shortcut was a rule that fires when a shape
// happens to be NAMED `Band` — a coupling that is invisible in the data, true of
// some templates and not others, and gone the moment somebody renames an object.
//
// What is built instead is an explicit one: a `band` layer carries `members`, a
// list of the ids that live inside it. Every test below is about that list being
// real — that the band reads it, that a duplicate does not share it, that a
// delete does not orphan it, and that a template which never opted in is
// untouched by any of it.
import { describe, it, expect } from 'vitest';
import {
  bandBox, bandFit, bandLayout, BAND_GROW_MAX, BAND_MAX_SHARE, BAND_TYPE_FLOOR,
} from './templatemodel.js';
import {
  STARTERS, makeLayer, bandOf, bandMembers, topLevelLayers, drawBoxes, isKeyedTemplate,
} from './layers.js';
import { duplicateLayer, removeLayer, resetLayer } from './layerops.js';

const starter = (key) => STARTERS.find((s) => s.key === key).make();
const bandLayer = (t) => t.layout.layers.find((L) => L.type === 'band');
const words = (t) => bandMembers(t.layout.layers, bandLayer(t));

const LONG =
  'For God so loved the world, that he gave his only begotten Son, that whosoever ' +
  'believeth in him should not perish, but have everlasting life. For God sent not ' +
  'his Son into the world to condemn the world; but that the world through him ' +
  'might be saved.';

describe('the band is a real element', () => {
  it('runs from its top to the BOTTOM edge, inset by the side safe area', () => {
    // §4: "from `top%` to the bottom, inset by the side safe area". The old band
    // was a shape floating at y:74 h:18, stopping at 92% with nothing under it.
    const box = bandBox({ top: 74, side: 6, pad: 3, lift: 3 });
    expect(box.height).toBe(26);
    expect(box.top + box.height).toBe(100);

    const { box: drawn } = bandLayout({ band: makeLayer('band'), members: [] });
    expect(drawn.y + drawn.h).toBe(100);
    expect(drawn.x).toBe(6);
    expect(drawn.w).toBe(88);
  });

  it('gives its words the width the band leaves them, not the frame', () => {
    // side on BOTH edges, then the inner gutter on both: 100 - 12 - 6.
    expect(bandBox({ top: 74, side: 6, pad: 3, lift: 3 }).textWidth).toBe(82);
  });

  it('centres the words in what is left above the lift — equal above, equal below', () => {
    const band = makeLayer('band', { top: 74, side: 6, pad: 3, lift: 3 });
    const lay = bandLayout({
      band,
      members: [
        { text: 'Pastor Ade', size: 3.2, face: 'sans', h: 10 },
        { text: 'GUEST SPEAKER', size: 1.4, face: 'sans', h: 5 },
      ],
    });
    const first = lay.members[0];
    const last = lay.members[lay.members.length - 1];
    const above = first.y - lay.top;
    const below = (100 - band.lift) - (last.y + last.h);
    expect(above).toBeCloseTo(below, 6);
    expect(above).toBeGreaterThan(0);
    // Stacked in the order the band names them, with no gap invented between.
    expect(lay.members[1].y).toBeCloseTo(first.y + first.h, 6);
  });

  it('keeps every word inside the band, above the baseline lift', () => {
    const band = makeLayer('band');
    const lay = bandLayout({
      band,
      members: [{ text: 'Pastor Ade', size: 3.2, face: 'sans', h: 10 }],
    });
    for (const m of lay.members) {
      expect(m.y).toBeGreaterThanOrEqual(lay.top);
      expect(m.y + m.h).toBeLessThanOrEqual(100 - band.lift + 1e-9);
    }
  });
});

describe('the band gives ground before the words do', () => {
  it('a short name at the same setting does not move it', () => {
    // The half of the rule that is easy to lose: a band that grows for
    // everything has simply been redesigned taller.
    const band = makeLayer('band');
    const fit = bandFit({
      band,
      members: [
        { text: 'Pastor Ade', size: 3.2, face: 'sans', h: 10 },
        { text: 'GUEST SPEAKER', size: 1.4, face: 'sans', h: 5 },
      ],
    });
    expect(fit.top).toBe(band.top);
    expect(fit.scale).toBe(1);
  });

  it('a long line makes it climb', () => {
    const band = makeLayer('band');
    const fit = bandFit({ band, members: [{ text: LONG, size: 2.6, face: 'serif', h: 10 }] });
    expect(fit.top).toBeLessThan(band.top);
    expect(fit.scale).toBeGreaterThan(1);
  });

  it('never climbs past a third of the frame, however long the line', () => {
    const band = makeLayer('band');
    const fit = bandFit({ band, members: [{ text: 'x'.repeat(4000), size: 2.6, face: 'serif', h: 10 }] });
    expect(100 - fit.top).toBeLessThanOrEqual(100 * BAND_MAX_SHARE + 1e-9);
  });

  it('never climbs more points than the band allows', () => {
    // A band low enough that the 16-point allowance binds before the third does.
    const band = makeLayer('band', { top: 88 });
    const fit = bandFit({ band, members: [{ text: 'x'.repeat(4000), size: 2.6, face: 'serif', h: 6 }] });
    expect(band.top - fit.top).toBeLessThanOrEqual(BAND_GROW_MAX);
    expect(band.top - fit.top).toBe(BAND_GROW_MAX);
  });

  it('a band told to give no ground gives none, and the words shrink instead', () => {
    const band = makeLayer('band', { grow: 0 });
    const lay = bandLayout({ band, members: [{ text: LONG, size: 2.6, face: 'serif', h: 10 }] });
    expect(lay.top).toBe(band.top);
    expect(lay.scale).toBe(1);
  });

  it('stops at the floor, not at the limit, when the floor comes first', () => {
    // Not "grow for every long line" and not "grow until it runs out of room":
    // climb until the words no longer need to be smaller than BAND_TYPE_FLOOR of
    // the size their designer asked for, then stop.
    expect(BAND_TYPE_FLOOR).toBe(0.78);
    const band = makeLayer('band', { top: 82, lift: 3 });
    const line = 'Great is thy faithfulness, O God my Father; there is no shadow of turning with thee.';
    const fit = bandFit({ band, members: [{ text: line, size: 3.2, face: 'sans', h: 9 }] });
    // It moved, and it stopped short of BOTH limits — so a limit is not what
    // stopped it.
    expect(fit.top).toBeLessThan(band.top);
    expect(band.top - fit.top).toBeLessThan(BAND_GROW_MAX);
    expect(100 - fit.top).toBeLessThan(100 * BAND_MAX_SHARE);
  });

  it('reads how far it may climb off the band, not off its caller', () => {
    // `grow` used to be a parameter with a constant default, so a caller that did
    // not pass one silently overrode the band's own setting — a second home for a
    // property, which is the defect this model exists to prevent.
    const band = makeLayer('band', { grow: 3 });
    const fit = bandFit({ band, members: [{ text: 'x'.repeat(4000), size: 2.6, face: 'serif', h: 10 }] });
    expect(band.top - fit.top).toBe(3);
  });
});

describe('the three lower thirds', () => {
  it('each is its own template with its own band, and editing one touches no other', () => {
    // W4 acceptance clause 1, proved by identity rather than by promise: the three
    // starters share no object at all, so there is nothing an edit to one could
    // reach in another.
    const a = starter('lower.name');
    const b = starter('lower.lyric');
    const c = starter('lower.bible');
    const ids = [a, b, c].flatMap((t) => t.layout.layers.map((L) => L.id));
    expect(new Set(ids).size).toBe(ids.length);

    const ba = bandLayer(a);
    ba.fill = '#ff0000';
    ba.top = 55;
    ba.members.push('smuggled');
    expect(bandLayer(b).fill).not.toBe('#ff0000');
    expect(bandLayer(b).top).toBe(74);
    expect(bandLayer(c).members).not.toContain('smuggled');
    expect(bandLayer(c).top).toBe(74);
  });

  it('each band owns exactly the words beside it', () => {
    for (const key of ['lower.name', 'lower.lyric', 'lower.bible']) {
      const t = starter(key);
      const band = bandLayer(t);
      expect(band, `${key} has a band`).toBeTruthy();
      const texts = t.layout.layers.filter((L) => L.type === 'text');
      expect(band.members).toEqual(texts.map((L) => L.id));
      expect(words(t).length).toBe(texts.length);
      // Membership is never inferred from a name. Rename the band and it still
      // owns the same words — which is the whole reason the list exists.
      band.name = 'Ribbon';
      expect(bandMembers(t.layout.layers, band).length).toBe(texts.length);
    }
  });

  it('Lyric carries the words and no reference at all', () => {
    const t = starter('lower.lyric');
    const binds = t.layout.layers.filter((L) => L.type === 'text').map((L) => L.bind);
    expect(binds).toEqual(['verse']);
    expect(binds).not.toContain('reference');
  });

  it('Name carries a name over a role, and Scripture a verse over its reference', () => {
    const name = starter('lower.name').layout.layers.filter((L) => L.type === 'text');
    expect(name.map((L) => L.bind)).toEqual(['verse', 'reference']);
    const bible = starter('lower.bible').layout.layers.filter((L) => L.type === 'text');
    expect(bible.map((L) => L.bind)).toEqual(['verse', 'reference']);
    // §4: the reference is a citation — right-aligned, tracked and small.
    const ref = bible[1];
    expect(ref.align).toBe('right');
    expect(ref.letterSpacing).toBeGreaterThan(0);
    expect(ref.size).toBeLessThan(bible[0].size);
  });

  it('stays keyed, so the camera it captions is never covered', () => {
    for (const key of ['lower.name', 'lower.lyric', 'lower.bible']) {
      expect(isKeyedTemplate(starter(key)), key).toBe(true);
    }
  });

  it('lays the words out where the band puts them, not where they store themselves', () => {
    const t = starter('lower.name');
    const boxes = drawBoxes(t.layout.layers, (L) => (L.bind === 'verse' ? 'Pastor Ade' : 'GUEST'));
    for (const L of t.layout.layers) {
      expect(boxes.has(L.id), `${L.name} has a drawn box`).toBe(true);
    }
    const w = words(t)[0];
    expect(boxes.get(w.id).x).toBe(9); // side 6 + inner 3
    expect(boxes.get(w.id).w).toBe(82);
  });

  it('a band draws its words; the stack does not draw them twice', () => {
    const t = starter('lower.bible');
    const top = topLevelLayers(t.layout.layers);
    expect(top.map((L) => L.type)).toEqual(['band']);
    for (const w of words(t)) expect(bandOf(t.layout.layers, w.id)).toBeTruthy();
  });

  it('a template with no band is not touched by any of this', () => {
    const t = starter('fullscreen');
    expect(t.layout.layers.some((L) => L.type === 'band')).toBe(false);
    expect(topLevelLayers(t.layout.layers)).toEqual(t.layout.layers);
    expect(drawBoxes(t.layout.layers, () => 'x').size).toBe(0);
  });
});

describe('a band survives the inspector', () => {
  it('a duplicated band gets its OWN words — it does not share the original\'s', () => {
    // W4 acceptance clause 2. `structuredClone` alone is not enough here and this
    // is the first layer property for which that is true: it copies the member
    // IDS faithfully, and the copy then points at the original's text layers.
    // Both bands would lay out the same two objects and editing either would move
    // the other's type.
    const t = starter('lower.name');
    const band = bandLayer(t);
    const after = duplicateLayer(t.layout.layers, band.id);
    const bands = after.filter((L) => L.type === 'band');
    expect(bands.length).toBe(2);

    const [orig, copy] = bands;
    expect(copy.members.length).toBe(orig.members.length);
    for (const id of copy.members) expect(orig.members).not.toContain(id);
    expect(after.filter((L) => L.type === 'text').length).toBe(4);

    // And the copy's words are real objects in the list, not dangling ids.
    const copied = bandMembers(after, copy);
    expect(copied.length).toBe(orig.members.length);

    // Edit the copy's words: the original's must not move.
    copied[0].size = 99;
    expect(bandMembers(after, orig)[0].size).not.toBe(99);
  });

  it('a duplicated word joins the band its original is in', () => {
    const t = starter('lower.bible');
    const band = bandLayer(t);
    const word = words(t)[0];
    const after = duplicateLayer(t.layout.layers, word.id);
    const grown = after.find((L) => L.id === band.id);
    expect(grown.members.length).toBe(band.members.length + 1);
    expect(bandMembers(after, grown).length).toBe(grown.members.length);
  });

  it('deleting a word tells its band, so no band names an object that is gone', () => {
    const t = starter('lower.name');
    const band = bandLayer(t);
    const word = words(t)[1];
    const after = removeLayer(t.layout.layers, word.id);
    const left = after.find((L) => L.id === band.id);
    expect(left.members).not.toContain(word.id);
    expect(left.members.length).toBe(band.members.length - 1);
    // Every id a band names resolves to something in the list.
    for (const id of left.members) expect(after.some((L) => L.id === id)).toBe(true);
  });

  it('deleting a band deletes the band, and only the band', () => {
    const t = starter('lower.name');
    const band = bandLayer(t);
    const after = removeLayer(t.layout.layers, band.id);
    expect(after.some((L) => L.type === 'band')).toBe(false);
    expect(after.length).toBe(t.layout.layers.length - 1);
  });

  it('resetting a band restores its look and keeps its place and its words', () => {
    const t = starter('lower.bible');
    const band = bandLayer(t);
    band.top = 62;
    band.side = 11;
    band.fill = '#ff0000';
    band.opacity = 0.2;
    const after = resetLayer(t.layout.layers, band.id);
    const back = after.find((L) => L.id === band.id);
    expect(back.fill).toBe(makeLayer('band').fill);
    expect(back.opacity).toBe(makeLayer('band').opacity);
    expect(back.top).toBe(62);
    expect(back.side).toBe(11);
    expect(back.members).toEqual(band.members);
  });
});

// ── THE SHARE CAP BOUNDS GROWTH, NOT THE BAND'S DESIGNED SIZE ────────────────
//
// `limit = Math.max(byGrow, byShare, 0)` made this rule unreachable on both bands
// Relay ships. Both are designed taller than a third of the frame (38% and 36%),
// so `byShare` (66.67) already sat ABOVE `box.top`, `top - 1 >= limit` was false
// on the first iteration, and the band never moved. The type shrank instead —
// measured at 0.630 of the designer's size against a 0.78 floor.
//
// DECISIONS §75 and REBRAND §4 both promise the band gives ground before the words
// do. The mechanism was built, tested and documented, and could not fire on either
// template that declares it, so the observable behaviour was the promise inverted.
//
// Watched to fail by restoring `Math.max(byGrow, byShare, 0)`.
describe('§75 · a band designed past the share can still give ground', () => {
  // The shape of the two SHIPPED bands: top 62 (38% tall), 16 points of grow.
  const shipped = (text) => ({
    band: { top: 62, grow: 16 },
    members: [{ text, size: 5.5, face: 'sans', w: 84, h: 20 }],
    aspect: 16 / 9,
  });

  it('a long passage moves the band rather than only shrinking the type', () => {
    const long = bandFit(shipped('a'.repeat(420)));
    expect(long.top, 'the band must climb from its designed 62').toBeLessThan(62);
    // …and not past its own grow allowance.
    expect(long.top).toBeGreaterThanOrEqual(62 - 16);
  });

  it('and a short name at the same setting does not move it', () => {
    // REBRAND §4 states this half explicitly, and it is what stops the rule
    // becoming "the band is always as tall as it may be".
    const short = bandFit(shipped('Ada Lovelace'));
    expect(short.top).toBe(62);
  });
});
