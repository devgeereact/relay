import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { gridSource, planCells, passageCells, pressArbiter, PRESS_MS } from './slidegrid.js';

const slidesOf = (item) => item.slides ?? [];

const PLAN = [
  { id: 'a', label: 'Amazing Grace', slides: [{ label: 'Verse 1', text: 'Amazing grace', tag: 'Verse' }, { label: 'Chorus', text: 'How sweet', tag: 'Chorus' }] },
  { id: 'b', label: 'Romans 8:28', slides: [{ label: 'Romans 8:28', text: 'And we know', tag: '' }] },
];

const VERSES = [
  { verse: 1, text: 'The LORD is my shepherd', reference: 'Psalms 23:1' },
  { verse: 2, text: 'He maketh me to lie down', reference: 'Psalms 23:2' },
];

describe('what is staged, as cells', () => {
  it('flattens a plan into one cell per slide, in running order', () => {
    const cells = planCells(PLAN, slidesOf);
    expect(cells.map((c) => c.label)).toEqual(['Verse 1', 'Chorus', 'Romans 8:28']);
    expect(cells.map((c) => c.n)).toEqual([1, 2, 3]);
    expect(cells[1]).toMatchObject({ kind: 'plan', cueId: 'a', slideIdx: 1 });
  });

  it('gives every cell a distinct key, so a keyed each block cannot collide', () => {
    const keys = planCells(PLAN, slidesOf).map((c) => c.key);
    expect(new Set(keys).size).toBe(keys.length);
  });

  it('turns a chapter into verse cells that fire BY REFERENCE, not by the text it loaded', () => {
    const cells = passageCells(VERSES);
    expect(cells).toHaveLength(2);
    expect(cells[0]).toMatchObject({ kind: 'verse', reference: 'Psalms 23:1', tag: 'v1' });
    // The words are for the thumbnail. The fire carries the reference, so the
    // wall is painted from the active translation rather than from this pane.
    expect(cells[0].text).toBe('The LORD is my shepherd');
  });

  it('an open plan wins over the passage — it is what the operator loaded', () => {
    const g = gridSource({
      planOpen: true, planTitle: 'Sunday', items: PLAN, slidesOf,
      verses: VERSES, passageTitle: 'Psalms 23',
    });
    expect(g.source).toBe('plan');
    expect(g.title).toBe('Sunday');
    expect(g.cells).toHaveLength(3);
  });

  it('with no plan open the live chapter is staged instead', () => {
    const g = gridSource({ planOpen: false, items: [], slidesOf, verses: VERSES, passageTitle: 'Psalms 23' });
    expect(g.source).toBe('passage');
    expect(g.title).toBe('Psalms 23');
    expect(g.cells).toHaveLength(2);
  });

  it('says WHICH empty it is — nothing staged is not an empty plan', () => {
    expect(gridSource({ planOpen: false, items: [], slidesOf, verses: [] }).source).toBe('none');
    expect(gridSource({ planOpen: true, planTitle: 'Sunday', items: [], slidesOf, verses: [] }))
      .toMatchObject({ source: 'plan', cells: [] });
  });

  it('survives the absences a real load has — no items, no slides, no verses', () => {
    expect(() => gridSource({ planOpen: true, items: undefined, slidesOf: () => undefined })).not.toThrow();
    expect(passageCells(undefined)).toEqual([]);
  });
});

describe('single click sends to programme, double click previews', () => {
  let sent, previewed, errors, arb;

  beforeEach(() => {
    vi.useFakeTimers();
    sent = [];
    previewed = [];
    errors = [];
    arb = pressArbiter({
      send: (c) => { sent.push(c.label); },
      preview: (c) => { previewed.push(c.label); },
      onError: (e) => errors.push(e),
    });
  });
  afterEach(() => vi.useRealTimers());

  const cell = (label) => ({ label, kind: 'plan', cueId: 'a', slideIdx: 0 });

  it('a single press sends — but only after the beat, and only once', () => {
    arb.press(cell('Verse 1'));
    expect(sent).toEqual([]); // nothing has reached a wall yet
    vi.advanceTimersByTime(PRESS_MS - 1);
    expect(sent).toEqual([]);
    vi.advanceTimersByTime(1);
    expect(sent).toEqual(['Verse 1']);
    vi.advanceTimersByTime(1000);
    expect(sent).toEqual(['Verse 1']); // and it does not fire again
  });

  // THE defect this arbitration exists to prevent. Without the cancel, every
  // preview puts the slide on the congregation's screen on its way past.
  it('A DOUBLE PRESS NEVER FIRES BOTH — it previews, and nothing goes to air', () => {
    const c = cell('Verse 1');
    arb.press(c);
    arb.double(c);
    vi.advanceTimersByTime(10_000);
    expect(previewed).toEqual(['Verse 1']);
    expect(sent).toEqual([]);
  });

  it('the second press cancels even at the very end of the beat', () => {
    const c = cell('Verse 1');
    arb.press(c);
    vi.advanceTimersByTime(PRESS_MS - 1);
    arb.double(c);
    vi.advanceTimersByTime(10_000);
    expect(sent).toEqual([]);
    expect(previewed).toEqual(['Verse 1']);
  });

  it('two quick presses on DIFFERENT cells send the second, never both', () => {
    arb.press(cell('Verse 1'));
    arb.press(cell('Chorus'));
    vi.advanceTimersByTime(10_000);
    expect(sent).toEqual(['Chorus']);
  });

  it('cancel() drops a press that has not happened — a view that went away fires nothing', () => {
    arb.press(cell('Verse 1'));
    expect(arb.armed).toBe(true);
    arb.cancel();
    expect(arb.armed).toBe(false);
    vi.advanceTimersByTime(10_000);
    expect(sent).toEqual([]);
  });

  it('a send that REJECTS is reported, never swallowed (rule 15)', async () => {
    const boom = new Error('screens unreachable');
    const a = pressArbiter({
      send: () => Promise.reject(boom),
      preview: () => {},
      onError: (e) => errors.push(e),
    });
    a.press(cell('Verse 1'));
    vi.advanceTimersByTime(PRESS_MS);
    await Promise.resolve();
    await Promise.resolve();
    expect(errors).toEqual([boom]);
  });

  it('a send that THROWS synchronously is reported too', () => {
    const boom = new Error('nope');
    const a = pressArbiter({
      send: () => { throw boom; },
      preview: () => {},
      onError: (e) => errors.push(e),
    });
    a.press(cell('Verse 1'));
    vi.advanceTimersByTime(PRESS_MS);
    expect(errors).toEqual([boom]);
  });

  it('the arbiter never reports a success — it has no notion of one', () => {
    // Nothing it can call says "on air". That claim belongs to the fire path,
    // which sets it only after the fire resolves (CLAUDE.md rules 15 and 18).
    const a = pressArbiter({ send: () => {}, preview: () => {} });
    expect(Object.keys(a).sort()).toEqual(['armed', 'cancel', 'double', 'press']);
  });

  it('the beat is 190ms, as measured in the prototype (docs/REBRAND.md §2)', () => {
    expect(PRESS_MS).toBe(190);
  });
});

// ── WHAT THE OPERATOR PICKED IN THE RAIL ────────────────────────────────────
//
// The Live rail stages; it never fires. So the only question this half has to
// answer is which of three things the grid should be showing, and the rule that
// matters is the asymmetry: a HAND PICK outranks the plan, a DETECTION never
// does. Getting that backwards either way is a service-morning failure — the
// operator's own choice silently ignored, or the plan they built pushed off the
// grid by whatever the preacher happened to quote.
describe('a hand pick outranks the plan; a detection never does', () => {
  it('a chapter the operator picked wins over an open plan', () => {
    const g = gridSource({
      planOpen: true, planTitle: 'Sunday Morning', items: PLAN, slidesOf,
      verses: VERSES, passageTitle: 'Psalms 23', handPicked: true,
    });
    expect(g.source).toBe('passage');
    expect(g.title).toBe('Psalms 23');
  });

  it('but a chapter the PREACHER caused does not — the plan keeps the grid', () => {
    // This is the whole point of the flag. Same verses, same plan, and the only
    // difference is who asked for them.
    const g = gridSource({
      planOpen: true, planTitle: 'Sunday Morning', items: PLAN, slidesOf,
      verses: VERSES, passageTitle: 'Psalms 23', handPicked: false,
    });
    expect(g.source).toBe('plan');
  });

  it('a hand pick that has not loaded yet does not blank the plan', () => {
    // The verses arrive from a database read. Between the press and the answer,
    // "nothing is staged" is a claim about an absence — and the plan is still the
    // truthful answer to what the operator has in front of them.
    const g = gridSource({
      planOpen: true, planTitle: 'Sunday Morning', items: PLAN, slidesOf,
      verses: [], passageTitle: 'Psalms 23', handPicked: true,
    });
    expect(g.source).toBe('plan');
    expect(g.cells.length).toBe(3);
  });
});
