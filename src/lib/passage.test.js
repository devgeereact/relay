import { describe, it, expect } from 'vitest';
import { parsePassage, probeReference, inRange } from './passage.js';

const verses = Array.from({ length: 6 }, (_, i) => ({ verse: i + 1 }));

describe('parsePassage', () => {
  it('reads a range the way an operator types it', () => {
    for (const q of ['Ps 23 1-5', 'Ps 23:1-5', 'Psalm 23. 1 - 5', 'ps 23 1 to 5']) {
      expect(parsePassage(q)).toEqual({ book: expect.any(String), chapter: 23, from: 1, to: 5 });
    }
  });

  it('reads a single verse as a range of one', () => {
    expect(parsePassage('John 3:16')).toEqual({ book: 'John', chapter: 3, from: 16, to: 16 });
  });

  it('reads a whole chapter', () => {
    expect(parsePassage('Psalm 23')).toEqual({ book: 'Psalm', chapter: 23, from: null, to: null });
  });

  it('keeps numbered books whole', () => {
    expect(parsePassage('1 John 4:8')).toMatchObject({ book: '1 John', chapter: 4, from: 8 });
    expect(parsePassage('2 Kings 2')).toMatchObject({ book: '2 Kings', chapter: 2 });
  });

  it('does not resolve the book — that table lives in Rust', () => {
    // "Ps" is passed through verbatim; only the backend knows it means Psalms.
    expect(parsePassage('Ps 23 1-5').book).toBe('Ps');
  });

  it('reads a backwards range forwards', () => {
    // A typo must not produce an empty pane with no explanation.
    expect(parsePassage('Ps 23 5-1')).toMatchObject({ from: 1, to: 5 });
  });

  it('is null for a phrase, so the caller falls back to search', () => {
    for (const q of ['', '   ', 'there is therefore now no condemnation', 'love', '23']) {
      expect(parsePassage(q)).toBeNull();
    }
  });
});

describe('probeReference', () => {
  it('always asks the backend about ONE verse', () => {
    expect(probeReference(parsePassage('Ps 23 1-5'))).toBe('Ps 23:1');
    expect(probeReference(parsePassage('Psalm 23'))).toBe('Psalm 23:1');
  });
});

describe('inRange', () => {
  it('keeps only the verses the range asked for', () => {
    expect(inRange(verses, parsePassage('Ps 23 2-4')).map((v) => v.verse)).toEqual([2, 3, 4]);
  });

  it('CLEARING THE RANGE LEAVES THE WHOLE CHAPTER — the point of the feature', () => {
    expect(inRange(verses, null)).toHaveLength(6);
    expect(inRange(verses, parsePassage('Psalm 23'))).toHaveLength(6);
  });
});
