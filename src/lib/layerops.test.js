// Phase 3 of the rebrand (docs/REBRAND.md §3.2): what the inspector's
// new / duplicate / reset / delete buttons actually do.
//
// These live outside the component because they are list arithmetic, and list
// arithmetic is where the two mistakes live: a "duplicate" that shares a
// sub-object with its original (edit one, change both), and a "reset" that
// throws away the thing a person spent their time on — where the object sits on
// the slide.
import { describe, it, expect } from 'vitest';
import {
  duplicateLayer, resetLayer, moveLayer,
  alignLayer, spaceEvenly, isMovable, movableLayers,
} from './layerops.js';

const text = (over = {}) => ({
  id: 't1',
  type: 'text',
  name: 'Verse',
  bind: 'verse',
  x: 10, y: 20, w: 60, h: 30,
  color: '#ffffff',
  size: 6,
  shadow: { k: 0.4 },
  ...over,
});

describe('duplicateLayer', () => {
  it('puts the copy directly in front of the original', () => {
    const a = text({ id: 'a' });
    const b = text({ id: 'b' });
    const out = duplicateLayer([a, b], 'a');
    expect(out.map((l) => l.id)[0]).toBe('a');
    expect(out.length).toBe(3);
    expect(out[2].id).toBe('b');
  });

  it('gives the copy an id of its own', () => {
    const out = duplicateLayer([text({ id: 'a' })], 'a');
    expect(out[1].id).not.toBe('a');
    expect(out[1].id).toBeTruthy();
  });

  it('is a DEEP copy — editing the copy cannot change the original', () => {
    // The failure this prevents is silent and looks like a rendering bug: two
    // layers that share a nested object move together for ever, and the person
    // editing the second one swears the first one is haunted.
    const src = text({ id: 'a' });
    const out = duplicateLayer([src], 'a');
    out[1].shadow.k = 0.9;
    expect(src.shadow.k).toBe(0.4);
  });

  it('offsets the copy so it is visible rather than exactly behind', () => {
    const out = duplicateLayer([text({ id: 'a', x: 10, y: 20 })], 'a');
    expect(out[1].x).toBeGreaterThan(10);
    expect(out[1].y).toBeGreaterThan(20);
  });

  it('keeps the copy on the canvas rather than nudging it off the edge', () => {
    const out = duplicateLayer([text({ id: 'a', x: 99, y: 99, w: 10, h: 10 })], 'a');
    expect(out[1].x + out[1].w).toBeLessThanOrEqual(100);
    expect(out[1].y + out[1].h).toBeLessThanOrEqual(100);
  });

  it('names the copy so the two are told apart in a list', () => {
    const out = duplicateLayer([text({ id: 'a', name: 'Verse' })], 'a');
    expect(out[1].name).toBe('Verse copy');
  });

  it('leaves the list alone when the id is not in it', () => {
    const list = [text({ id: 'a' })];
    expect(duplicateLayer(list, 'nope')).toEqual(list);
    expect(duplicateLayer(list, null)).toEqual(list);
  });

  it('does not mutate the list it was given', () => {
    const list = [text({ id: 'a' })];
    duplicateLayer(list, 'a');
    expect(list.length).toBe(1);
  });
});

describe('resetLayer', () => {
  it('puts the styling back to the starter for that kind', () => {
    const styled = text({ id: 'a', color: '#ff0000', size: 22 });
    const out = resetLayer([styled], 'a');
    expect(out[0].color).not.toBe('#ff0000');
    expect(out[0].size).not.toBe(22);
  });

  it('keeps where the object sits, which is the part somebody placed by hand', () => {
    const out = resetLayer([text({ id: 'a', x: 12, y: 34, w: 56, h: 7 })], 'a');
    expect(out[0]).toMatchObject({ x: 12, y: 34, w: 56, h: 7 });
  });

  it('keeps the identity: same id, same kind, same binding, same name', () => {
    const out = resetLayer([text({ id: 'a', bind: 'reference', name: 'Ref' })], 'a');
    expect(out[0]).toMatchObject({ id: 'a', type: 'text', bind: 'reference', name: 'Ref' });
  });

  it('keeps whether the object is hidden or locked', () => {
    // Resetting the LOOK of an object must not put a hidden one back on the
    // wall — that is a template change nobody asked for, on a screen.
    const out = resetLayer([text({ id: 'a', visible: false, locked: true })], 'a');
    expect(out[0].visible).toBe(false);
    expect(out[0].locked).toBe(true);
  });

  it('leaves the list alone when the id is not in it', () => {
    const list = [text({ id: 'a' })];
    expect(resetLayer(list, 'nope')).toEqual(list);
  });

  it('does not mutate the layer it was given', () => {
    const src = text({ id: 'a', color: '#ff0000' });
    resetLayer([src], 'a');
    expect(src.color).toBe('#ff0000');
  });
});

// ── REORDERING, and the band that made the old one a no-op ─────────────────
//
// The layer list is the ONE surface that drives paint order, and its Forward /
// Back buttons used to swap two adjacent entries of the raw array. A band's
// words are IN that array and are not drawn from it — the band draws them, in
// the order it names them — so every swap with a word moved nothing anybody
// could see. On `lowerBible` (a band and its two words, which is the whole
// template) the control was inert in every direction; add a shape to it and one
// step back took three presses.
const band = (over = {}) => ({ id: 'b1', type: 'band', name: 'Band', members: [], ...over });
const shape = (over = {}) => ({ id: 's1', type: 'shape', name: 'Shape', x: 0, y: 0, w: 10, h: 10, ...over });

describe('moveLayer — the paint order', () => {
  it('steps a top-level object PAST the band member sitting next to it, not into it', () => {
    // The shipped shape of `lowerBible` plus one shape: [band, Verse, Ref, shape].
    const list = [
      band({ members: ['w1', 'w2'] }),
      text({ id: 'w1' }),
      text({ id: 'w2' }),
      shape(),
    ];
    // One press back = the shape is now behind the band. Not "behind a word",
    // which is a position nothing renders.
    const out = moveLayer(list, 's1', -1);
    expect(out.map((l) => l.id)).toEqual(['s1', 'b1', 'w1', 'w2']);
  });

  it('reports the end of the stack by changing nothing', () => {
    const list = [band({ members: ['w1'] }), text({ id: 'w1' }), shape()];
    expect(moveLayer(list, 's1', 1)).toBe(list);
    expect(moveLayer(list, 'b1', -1)).toBe(list);
  });

  it('moves a WORD within its band, which is the order the band draws them in', () => {
    const list = [band({ members: ['w1', 'w2'] }), text({ id: 'w1' }), text({ id: 'w2' })];
    const out = moveLayer(list, 'w2', -1);
    expect(out.find((l) => l.id === 'b1').members).toEqual(['w2', 'w1']);
  });

  it('does not let a word leave its band by being moved', () => {
    const list = [band({ members: ['w1', 'w2'] }), text({ id: 'w1' }), text({ id: 'w2' })];
    const out = moveLayer(list, 'w1', -1); // already first
    expect(out).toBe(list);
    const both = moveLayer(list, 'w2', 1); // already last
    expect(both).toBe(list);
  });

  it('leaves the list alone when the id is not in it', () => {
    const list = [shape()];
    expect(moveLayer(list, 'nope', 1)).toBe(list);
  });

  it('does not mutate the list it was given', () => {
    const list = [shape({ id: 'a' }), shape({ id: 'b' })];
    const before = list.map((l) => l.id);
    moveLayer(list, 'a', 1);
    expect(list.map((l) => l.id)).toEqual(before);
  });
});

// ── ALIGNING AND SPACING — wave 3, agent S1 ────────────────────────────────
//
// The same class of mistake as the two above, and here for the same reason: it
// is invisible on screen. "Align right" that stores `x = 100` puts a 60%-wide
// object more than half off the frame, and in a 240px preview that looks like a
// centred object with a generous margin.
//
// Each assertion below was watched to fail with its own defect reintroduced —
// the clamp removed, `isMovable` softened to `type !== 'background'`, and the
// three-object floor dropped to two.
describe('alignLayer', () => {
  const box = (over) => text({ ...over });

  it('puts an object against an edge as a SHARE of the frame, never a pixel', () => {
    expect(alignLayer([box({ id: 'a', x: 10, w: 60 })], 'a', 'left')[0].x).toBe(0);
    expect(alignLayer([box({ id: 'a', y: 20, h: 30 })], 'a', 'top')[0].y).toBe(0);
  });

  it("CLAMPS the far edges by the object's own size", () => {
    // THE DEFECT: `x = 100` for "right", `y = 100` for "bottom". The number grid
    // in the Position panel would then read a perfectly plausible 100 over an
    // object that is mostly off the canvas.
    expect(alignLayer([box({ id: 'a', w: 60 })], 'a', 'right')[0].x).toBe(40);
    expect(alignLayer([box({ id: 'a', h: 30 })], 'a', 'bottom')[0].y).toBe(70);
  });

  it("centres on the object's middle, to a tenth", () => {
    expect(alignLayer([box({ id: 'a', w: 33 })], 'a', 'hcenter')[0].x).toBe(33.5);
    expect(alignLayer([box({ id: 'a', h: 33 })], 'a', 'vmiddle')[0].y).toBe(33.5);
  });

  it('refuses the three kinds these four numbers do not place', () => {
    // Not a nicety. Nothing draws a background, a band or a word-in-a-band from
    // x/y/w/h, so a button that writes them is a button that changes nothing —
    // the defect DECISIONS §69 closed, one control along.
    const band = { id: 'b1', type: 'band', members: ['w1'], top: 74, side: 6 };
    const list = [band, box({ id: 'w1' }), { id: 'bg', type: 'background', x: 0, y: 0, w: 100, h: 100 }, box({ id: 'a' })];
    expect(alignLayer(list, 'b1', 'left')).toBe(list);
    expect(alignLayer(list, 'w1', 'left')).toBe(list);
    expect(alignLayer(list, 'bg', 'left')).toBe(list);
    // ...and it does move the one object that IS placed by them, so the three
    // refusals above are a rule rather than a function that never works.
    expect(alignLayer(list, 'a', 'left')).not.toBe(list);
  });

  it('refuses a LOCKED object — the padlock means the same thing on every surface', () => {
    const list = [box({ id: 'a', locked: true })];
    expect(alignLayer(list, 'a', 'left')).toBe(list);
  });

  it('returns the SAME list when nothing moved, so a caller can tell', () => {
    const list = [box({ id: 'a', x: 0 })];
    expect(alignLayer(list, 'a', 'left')).toBe(list);
    expect(alignLayer(list, 'nope', 'left')).toBe(list);
    expect(alignLayer(list, 'a', 'diagonally')).toBe(list);
  });
});

describe('spaceEvenly', () => {
  const at = (id, x, w = 10) => text({ id, x, w, y: 0, h: 10 });

  it('holds the outer two still and evens the centres between them', () => {
    // Distribution is a statement about the gaps, not about where the group
    // sits. A version that re-centred the row as well would look, on the one
    // press an operator gave it, like it had moved everything somewhere
    // arbitrary.
    expect(spaceEvenly([at('a', 0), at('b', 5), at('c', 40)], 'x').map((l) => l.x))
      .toEqual([0, 20, 40]);
  });

  it('works down the frame as well as across it', () => {
    const col = (id, y) => text({ id, x: 0, w: 10, y, h: 10 });
    expect(spaceEvenly([col('a', 0), col('b', 1), col('c', 60)], 'y').map((l) => l.y))
      .toEqual([0, 30, 60]);
  });

  it('sorts by where things ARE, not by where they sit in the array', () => {
    const out = spaceEvenly([at('c', 40), at('a', 0), at('b', 5)], 'x');
    expect(out.find((l) => l.id === 'b').x).toBe(20);
    expect(out.find((l) => l.id === 'a').x).toBe(0);
    expect(out.find((l) => l.id === 'c').x).toBe(40);
  });

  it('needs three, and says so by changing nothing', () => {
    const two = [at('a', 0), at('b', 40)];
    expect(spaceEvenly(two, 'x')).toBe(two);
  });

  it('counts only the objects it may move', () => {
    const list = [
      { id: 'b1', type: 'band', members: ['w1'], top: 74 },
      text({ id: 'w1' }),
      { id: 'bg', type: 'background', x: 0, y: 0, w: 100, h: 100 },
      at('a', 0), at('b', 40),
    ];
    expect(movableLayers(list).map((l) => l.id)).toEqual(['a', 'b']);
    // Two movable objects on a five-object slide is still two, so it declines.
    expect(spaceEvenly(list, 'x')).toBe(list);
  });

  it('brings an object that was ALREADY off the frame back onto it', () => {
    // AN INVARIANT, NOT A REGRESSION, and the difference is stated because the
    // rest of this file is the other kind. Removing the clamp inside
    // `spaceEvenly` was watched and every assertion here still passed: every
    // target centre lies between the outermost two centres, and both of those
    // belong to objects that are on the canvas, so an interior object can never
    // be pushed off by the spacing itself. What the clamp is actually for is an
    // object that arrived off the canvas — a hand-edited or imported template —
    // and that case IS reachable, so it is the case asserted.
    const overflowing = text({ id: 'b', x: 30, w: 80, y: 0, h: 10 });
    expect(overflowing.x + overflowing.w).toBeGreaterThan(100);
    const out = spaceEvenly([at('a', 0, 10), overflowing, at('c', 90, 10)], 'x');
    const moved = out.find((l) => l.id === 'b');
    expect(moved.x).toBeGreaterThanOrEqual(0);
    expect(moved.x + moved.w).toBeLessThanOrEqual(100);
  });
});

describe('isMovable', () => {
  it('takes an id or the object itself, and answers the same either way', () => {
    const list = [text({ id: 'a' })];
    expect(isMovable(list, 'a')).toBe(true);
    expect(isMovable(list, list[0])).toBe(true);
    expect(isMovable(list, 'ghost')).toBe(false);
  });
});
