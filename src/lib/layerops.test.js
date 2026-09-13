// Phase 3 of the rebrand (docs/REBRAND.md §3.2): what the inspector's
// new / duplicate / reset / delete buttons actually do.
//
// These live outside the component because they are list arithmetic, and list
// arithmetic is where the two mistakes live: a "duplicate" that shares a
// sub-object with its original (edit one, change both), and a "reset" that
// throws away the thing a person spent their time on — where the object sits on
// the slide.
import { describe, it, expect } from 'vitest';
import { duplicateLayer, resetLayer } from './layerops.js';

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
