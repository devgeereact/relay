import { describe, it, expect } from 'vitest';
import { templateKind, kindsPresent } from './templateKind.js';

// Shapes taken verbatim from the seeded built-ins in db/templates.rs, so these
// pin the derivation against the templates every install actually ships with.
const classicSerif = { layout: { regions: ['verse_text', 'reference'], lowerThird: false } };
const stageMono = { layout: { regions: ['reference', 'verse_text'], refFirst: true } };
const lowerThird = { layout: { regions: ['verse_text', 'reference'], lowerThird: true } };
const worshipLyrics = { layout: { regions: ['verse_text'] } };
const lobbyWarm = { layout: { regions: ['reference', 'verse_text'] } };

describe('templateKind', () => {
  it('reads a verse-with-citation as scripture', () => {
    expect(templateKind(classicSerif)).toBe('scripture');
    expect(templateKind(stageMono)).toBe('scripture');
    expect(templateKind(lobbyWarm)).toBe('scripture');
  });

  it('reads verse-text alone as a song', () => {
    // Lyrics have no reference to show.
    expect(templateKind(worshipLyrics)).toBe('song');
  });

  it('reads a lower-third band as a lower third whatever regions it carries', () => {
    // The band wins even though this template also lists reference + verse.
    expect(templateKind(lowerThird)).toBe('lower-third');
  });

  it('falls back to custom for a shape that fits no rule', () => {
    expect(templateKind({ layout: { regions: [] } })).toBe('custom');
    expect(templateKind({ layout: { regions: ['reference'] } })).toBe('custom');
  });

  it('never throws on a malformed template', () => {
    expect(templateKind(null)).toBe('custom');
    expect(templateKind({})).toBe('custom');
    expect(templateKind({ layout: null })).toBe('custom');
    expect(templateKind({ layout: { regions: 'nonsense' } })).toBe('custom');
  });
});

describe('kindsPresent', () => {
  it('lists only kinds that actually occur, in display order, with counts', () => {
    const kinds = kindsPresent([classicSerif, stageMono, lobbyWarm, worshipLyrics, lowerThird]);
    expect(kinds.map((k) => k.key)).toEqual(['scripture', 'song', 'lower-third']);
    expect(kinds.find((k) => k.key === 'scripture').count).toBe(3);
    expect(kinds.find((k) => k.key === 'song').count).toBe(1);
    // No 'custom' tab when nothing is custom — an empty type tab would mislead.
    expect(kinds.some((k) => k.key === 'custom')).toBe(false);
  });

  it('is empty for an empty library', () => {
    expect(kindsPresent([])).toEqual([]);
    expect(kindsPresent(undefined)).toEqual([]);
  });
});
