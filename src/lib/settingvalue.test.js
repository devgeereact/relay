// Phase 11 of the rebrand (docs/REBRAND.md §11): a list row is a name and a
// VALUE — never an em dash standing in for one.
//
// The em dash was the same glyph for three situations an operator needs to tell
// apart: not fetched yet, fetch failed, and genuinely nothing there. That is
// CLAUDE.md rule 35 — a status that reads the same when broken as when fine is
// not a status — and it is exactly the shape of RG-83, where "up to date" was
// printed over a channel that had been 404ing for months.
import { describe, it, expect } from 'vitest';
import { settingValue, CHECKING } from './settingvalue.js';

describe('settingValue', () => {
  it('shows the value when there is one', () => {
    expect(settingValue('0.2.0-2')).toBe('0.2.0-2');
    expect(settingValue('192.168.1.40')).toBe('192.168.1.40');
  });

  it('says it is still checking, and that beats any absent-case wording', () => {
    expect(settingValue('', { loading: true, missing: 'no network' })).toBe(CHECKING);
    expect(settingValue('192.168.1.40', { loading: true })).toBe(CHECKING);
  });

  it('says what absence MEANS here, not a dash', () => {
    // The wording belongs to the call site: only it knows whether an empty
    // answer means "no network", "never asked" or "no model installed".
    expect(settingValue('', { missing: 'not on a network' })).toBe('not on a network');
    expect(settingValue(null, { missing: 'never checked' })).toBe('never checked');
    expect(settingValue(undefined, { missing: 'no model' })).toBe('no model');
  });

  it('treats whitespace as absent — a row of spaces is not an answer', () => {
    expect(settingValue('   ', { missing: 'unknown' })).toBe('unknown');
  });

  it('keeps a zero, because zero is an answer', () => {
    // `||` would have turned "0 dropped" and "0 ms" into "unavailable", which is
    // the opposite of what those rows mean.
    expect(settingValue(0)).toBe('0');
    expect(settingValue(false)).toBe('false');
  });

  it('refuses a number that is not one', () => {
    expect(settingValue(NaN, { missing: 'unknown' })).toBe('unknown');
    expect(settingValue(Infinity, { missing: 'unknown' })).toBe('unknown');
  });

  it('has a default for a call site that has not thought about it yet', () => {
    // Still a word rather than a dash: "unavailable" is at least a claim that
    // can be wrong, which an em dash is not.
    expect(settingValue('')).toBe('unavailable');
  });
});
