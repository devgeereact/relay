import { describe, it, expect } from 'vitest';
import { enqueue, dequeue, move, reorder, take, clear } from './queue.js';

const q = (...refs) => refs.map((reference) => ({ reference, text: reference }));

describe('enqueue', () => {
  it('adds to the end', () => {
    expect(enqueue(q('a'), { reference: 'b' }).map((x) => x.reference)).toEqual(['a', 'b']);
  });
  it('refuses a duplicate rather than queueing the same verse twice', () => {
    expect(enqueue(q('a'), { reference: 'a' })).toHaveLength(1);
  });
  it('ignores an item with no reference — there would be nothing to fire', () => {
    expect(enqueue(q('a'), {})).toHaveLength(1);
    expect(enqueue(q('a'), null)).toHaveLength(1);
  });
});

describe('move', () => {
  it('swaps with the neighbour', () => {
    expect(move(q('a', 'b', 'c'), 'b', -1).map((x) => x.reference)).toEqual(['b', 'a', 'c']);
    expect(move(q('a', 'b', 'c'), 'b', 1).map((x) => x.reference)).toEqual(['a', 'c', 'b']);
  });
  it('DOES NOT WRAP at either end', () => {
    // Wrapping would send the top of the queue to the bottom on a mis-click,
    // mid-service, with no undo.
    expect(move(q('a', 'b'), 'a', -1).map((x) => x.reference)).toEqual(['a', 'b']);
    expect(move(q('a', 'b'), 'b', 1).map((x) => x.reference)).toEqual(['a', 'b']);
  });
  it('is a no-op for something not queued', () => {
    expect(move(q('a'), 'zz', 1).map((x) => x.reference)).toEqual(['a']);
  });
});

describe('reorder', () => {
  it('drags an item to a position', () => {
    expect(reorder(q('a', 'b', 'c'), 0, 2).map((x) => x.reference)).toEqual(['b', 'c', 'a']);
    expect(reorder(q('a', 'b', 'c'), 2, 0).map((x) => x.reference)).toEqual(['c', 'a', 'b']);
  });
  it('clamps a drop past either edge instead of dropping the row', () => {
    expect(reorder(q('a', 'b'), 0, 99).map((x) => x.reference)).toEqual(['b', 'a']);
    expect(reorder(q('a', 'b'), 1, -5).map((x) => x.reference)).toEqual(['b', 'a']);
  });
  it('ignores a bad source index', () => {
    expect(reorder(q('a'), 7, 0).map((x) => x.reference)).toEqual(['a']);
  });
});

describe('take', () => {
  it('returns the head and the rest', () => {
    const { item, rest } = take(q('a', 'b'));
    expect(item.reference).toBe('a');
    expect(rest.map((x) => x.reference)).toEqual(['b']);
  });
  it('is safe when empty — nothing to fire is not an error', () => {
    expect(take([])).toEqual({ item: null, rest: [] });
  });
  it('does not mutate the list it was given', () => {
    const list = q('a', 'b');
    take(list);
    expect(list).toHaveLength(2);
  });
});

describe('dequeue / clear', () => {
  it('removes one, keeps order', () => {
    expect(dequeue(q('a', 'b', 'c'), 'b').map((x) => x.reference)).toEqual(['a', 'c']);
  });
  it('empties', () => expect(clear()).toEqual([]));
});
