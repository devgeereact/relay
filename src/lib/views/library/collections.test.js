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
//      other surface in this console, so the colour is allowed to paint ONE
//      3px edge and nothing else, and selection stays steel.

import { describe, it, expect } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { COLLECTIONS, VIEW_KEYS, collectionOf, collectionByKey, countMark, countWords } from './collections.js';
import Collections from './Collections.svelte';

const read = (p) => readFileSync(new URL(p, import.meta.url), 'utf8');
const bar = read('./Collections.svelte');
const shell = read('../Library.svelte');
// Both halves of the stylesheet — the palette moved into `tokens.css` in wave 5,
// Track E so the congregation-facing pages could take the tokens and none of the
// console's rules.
const css = read('../../../tokens.css') + read('../../../app.css');

/** Markup and rules only. This file EXPLAINS the colour law in prose, and a grep
    a comment can trip is a grep that will trip on the next comment. */
const barCode = bar
  .replace(/<!--[\s\S]*?-->/g, ' ')
  .replace(/\/\*[\s\S]*?\*\//g, ' ')
  .replace(/(^|\s)\/\/[^\n]*/g, ' ');

function mount(props = {}) {
  const host = document.createElement('div');
  document.body.appendChild(host);
  const app = new Collections({ target: host, props });
  return { host, done: () => (app.$destroy(), host.remove()) };
}

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
  // REBRAND §1 and §10 put the collection colour on a 3px LEFT BORDER of a
  // square chip with a mono count. An edge is the whole of the spend: a chip
  // whose GROUND went amber-soft would be indistinguishable, at a glance and
  // across a dark booth, from the on-air wash that means a verse is in front of
  // a congregation — and `--v-col-scripture` really is `--v-amber`, so this is
  // not a hypothetical. The prototype fills the pressed chip with the colour
  // and colours its text; Relay does neither, because Relay has an ON AIR
  // ladder and a rehearsal state in the same window and the prototype's mock
  // does not. CLAUDE.md wins over the prototype (rule 18, DECISIONS §21).
  it('the collection colour is ONE 3px edge — never a fill, never ink', () => {
    // Every declaration that SPENDS `--cc`, by the property it lands on.
    const spends = [...barCode.matchAll(/([a-z][a-z-]*)\s*:\s*[^;{}]*var\(--cc\b[^;{}]*/g)].map(
      (m) => m[1],
    );
    expect(spends.length, '--cc is declared but nothing paints with it').toBeGreaterThan(0);
    expect([...new Set(spends)].sort()).toEqual(['border-left', 'border-left-color']);
    expect(barCode).toMatch(/border-left:\s*3px solid var\(--cc\)/);
  });

  it('and no soft ground or line variant of it survives to be reached for', () => {
    // `--cc-soft` / `--cc-line` were the tile's ground and border. A token that
    // still exists is a token the next edit will use, and the amber-soft wash is
    // exactly the one that would read as ON AIR.
    expect(barCode).not.toContain('--cc-soft');
    expect(barCode).not.toContain('--cc-line');
  });

  it('and selection is steel, on a rail where one collection is already steel', () => {
    // Songs' collection colour IS --v-sel. If selection were carried by the
    // collection colour, an open Songs chip and a closed one would look alike.
    expect(bar).toMatch(/\.cr-c\.on\s*\{[^}]*--v-sel-/);
  });

  it('and an OPEN collection keeps its own edge', () => {
    // `.cr-c.on` sets `border-color` for the steel selection, which is a
    // four-sided property: without the restatement it repaints the left edge too
    // and the open collection is the ONE chip that has stopped saying which
    // collection it is. Assert the restatement comes after, because in CSS the
    // order is the rule.
    const on = barCode.slice(barCode.indexOf('.cr-c.on'));
    const rule = on.slice(0, on.indexOf('}') + 1);
    expect(rule).toMatch(/border-color:\s*var\(--v-sel-line\)/);
    expect(rule.indexOf('border-left-color')).toBeGreaterThan(rule.indexOf('border-color:'));
  });

  it('and no collection wears a status word', () => {
    // Comments stripped first. This file EXPLAINS the colour law in prose, and a
    // grep a comment can trip is a grep that will trip on the next one — the same
    // lesson `r6-contracts.test.js` learned about `aria-modal`.
    for (const word of ['ON AIR', 'CUED', 'Rehears']) {
      expect(barCode, `the collection rail says "${word}"`).not.toContain(word);
    }
  });
});

// A grep over a stylesheet says what the rules ARE; it cannot say that anything
// renders them. This mounts the real component — the level at which two earlier
// Library findings lived, and the level `qa-inventory` cannot reach.
describe('the rail as it actually renders', () => {
  it('draws four chips, each wearing its own collection class', async () => {
    const { host, done } = mount({ collection: 'scripture', view: 'browse', counts: {} });
    await tick();
    const chips = [...host.querySelectorAll('.cr-c')];
    expect(chips.map((c) => c.textContent.trim().split(/\s+/)[0])).toEqual([
      'Scripture',
      'Songs',
      'Announcements',
      'Media',
    ]);
    // The class is what carries `--cc`, so a chip without it is a chip with no
    // colour at all — and a chip with the WRONG one names a different collection.
    expect(chips.map((c) => [...c.classList].find((k) => k.startsWith('cc-')))).toEqual([
      'cc-scripture',
      'cc-song',
      'cc-notice',
      'cc-media',
    ]);
    done();
  });

  it('prints the count in mono, and says the three non-numbers out loud', async () => {
    const { host, done } = mount({
      collection: 'scripture',
      view: 'browse',
      counts: { scripture: 12, songs: 0, notices: null, media: -1 },
    });
    await tick();
    const marks = [...host.querySelectorAll('.cr-k')];
    expect(marks.every((m) => m.classList.contains('r-mono'))).toBe(true);
    expect(marks.map((m) => m.textContent.trim())).toEqual(['12', '0', '·', '!']);
    // …and none of the three reads as a zero to a screen reader either.
    const said = [...host.querySelectorAll('.cr-c .sr-only')].map((s) => s.textContent.trim());
    expect(said).toEqual([
      '12 saved verses',
      '0 songs',
      'count not loaded',
      'count unavailable',
    ]);
    done();
  });

  it('marks exactly one chip selected, and the open collection is the one asked for', async () => {
    const { host, done } = mount({ collection: 'media', view: 'graphics', counts: {} });
    await tick();
    const on = [...host.querySelectorAll('.cr-c[aria-selected="true"]')];
    expect(on).toHaveLength(1);
    expect(on[0].classList.contains('cc-media')).toBe(true);
    // Media holds two panes, so its views are offered — on the same line.
    const views = [...host.querySelectorAll('.cr-v')].map((v) => v.textContent.trim());
    expect(views).toEqual(['Video & documents', 'Graphics']);
    done();
  });
});
