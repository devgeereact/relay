// Phase 3 of the rebrand (docs/REBRAND.md §3.2): what the inspector's
// new / duplicate / reset / delete buttons actually do.
//
// These live outside the component because they are list arithmetic, and list
// arithmetic is where the two mistakes live: a "duplicate" that shares a
// sub-object with its original (edit one, change both), and a "reset" that
// throws away the thing a person spent their time on — where the object sits on
// the slide.
import { describe, it, expect } from 'vitest';
import { duplicateLayer, resetLayer, moveLayer } from './layerops.js';

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
