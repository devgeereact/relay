// The Library's collection rail (REBRAND §10).
//
// Three things are worth a test here, and none of them is the layout:
//
//   1. the register and the shell must agree about which panes exist — a
//      collection offering a view nothing renders is a dead control that
//      `qa-inventory` cannot see, because the button HAS a handler;
//   2. a count that has not loaded, a count whose query failed and an empty
//      collection must not read the same (rule 35). This is the whole reason
//      `countWords` exists rather than `${n} items`;
//   3. the collection colour must stay a tint. Amber means ON AIR on every
//      other surface in this console, so the colour is allowed to paint the
//      icon tile and nothing else, and selection stays steel.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { COLLECTIONS, VIEW_KEYS, collectionOf, collectionByKey, countMark, countWords } from './collections.js';

const read = (p) => readFileSync(new URL(p, import.meta.url), 'utf8');
const bar = read('./Collections.svelte');
const shell = read('../Library.svelte');
const css = read('../../../app.css');

describe('the register', () => {
  it('gives every view exactly one collection', () => {
    expect(new Set(VIEW_KEYS).size).toBe(VIEW_KEYS.length);
    for (const key of VIEW_KEYS) expect(collectionOf(key)?.key).toBeTruthy();
    expect(collectionOf('nothing-owns-this')).toBeUndefined();
  });

  it('names four collections, each with a colour token that exists', () => {
    expect(COLLECTIONS.map((c) => c.key)).toEqual(['scripture', 'songs', 'notices', 'media']);
    for (const c of COLLECTIONS) {
      expect(css, `--v-col-${c.colour} is not a token`).toMatch(
        new RegExp(`--v-col-${c.colour}\\s*:`),
      );
    }
  });

  it('and the shell renders every view the register offers', () => {
    // A collection button that lands on a pane `Library.svelte` has no branch
    // for is a control that does nothing and reports success by looking normal.
    for (const key of VIEW_KEYS) {
      expect(shell, `Library.svelte never renders the "${key}" view`).toMatch(
        new RegExp(`active === '${key}'`),
      );
    }
  });

  it('and collectionByKey answers for the four and nothing else', () => {
    expect(collectionByKey('songs')?.label).toBe('Songs');
    expect(collectionByKey('hymns')).toBeUndefined();
  });
});

describe('a count that is not a number is not zero', () => {
  const scripture = collectionByKey('scripture');

  it('an empty collection says it is empty', () => {
    expect(countWords(scripture, 0)).toBe('0 saved verses');
    expect(countMark(0)).toBe('0');
  });

  // THE BUG THIS PINS. `counts` starts as `{ scripture: null, … }` and is filled
  // in after four commands answer. Drawing that as "0 saved verses" tells an
  // operator their library is empty for as long as the read takes — and tells
  // them the same thing for ever if it never answers.
  it('a count that has not loaded says so, and never draws a 0', () => {
    expect(countWords(scripture, null)).toBe('count not loaded');
    expect(countWords(scripture, undefined)).toBe('count not loaded');
    expect(countMark(null)).not.toBe('0');
    expect(countMark(undefined)).not.toBe('0');
  });

  it('a count whose query failed says THAT, which is different again', () => {
    expect(countWords(scripture, -1)).toBe('count unavailable');
    expect(countMark(-1)).not.toBe('0');
    expect(countWords(scripture, -1)).not.toBe(countWords(scripture, null));
  });

  it('and one of a thing is not "1 saved verses"', () => {
    expect(countWords(scripture, 1)).toBe('1 saved verse');
    expect(countWords(collectionByKey('media'), 1)).toBe('1 item');
  });
});

describe('the colour law holds on the rail', () => {
  it('the collection colour reaches the icon tile and nothing else', () => {
    // `--cc` is the collection colour. It is declared on the chip so the tile can
    // read it, and `.cr-i` is the only rule allowed to paint with it.
    const users = [...bar.matchAll(/([.#][\w-]+)\s*\{[^}]*var\(--cc[^}]*\}/g)].map((m) => m[1]);
    expect(users.sort()).toEqual(['.cr-i']);
  });

  it('and selection is steel, on a rail where one collection is already steel', () => {
    // Songs' collection colour IS --v-sel. If selection were carried by the
    // collection colour, an open Songs chip and a closed one would look alike.
    expect(bar).toMatch(/\.cr-c\.on\s*\{[^}]*--v-sel-/);
  });

  it('and no collection wears a status word', () => {
    // Comments stripped first. This file EXPLAINS the colour law in prose, and a
    // grep a comment can trip is a grep that will trip on the next one — the same
    // lesson `r6-contracts.test.js` learned about `aria-modal`.
    const code = bar
      .replace(/\/\*[\s\S]*?\*\//g, '')
      .replace(/^\s*(\/\/|<!--).*$/gm, '');
    for (const word of ['ON AIR', 'CUED', 'Rehears']) {
      expect(code, `the collection rail says "${word}"`).not.toContain(word);
    }
  });
});
