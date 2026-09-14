// THE COLOUR LAW, HELD AT THE TWO TAXONOMY TABLES THAT BROKE IT.
//
// CLAUDE.md rule 18 / DECISIONS §21 / REBRAND §1: amber = ON AIR, amethyst =
// rehearsal, cyan = a guess, grey = CUED, red = destructive. A colour that
// carries a promise may be used ONLY where it means what it says.
//
// `plan.js` had two tables that each painted a KIND rather than a STATE, and
// between them they spent every one of those colours — on Live, the run surface,
// inches from the rules that signal the real states. A song cue's stripe was
// ON-AIR amber; a Chorus chip was ON-AIR amber and more saturated than a
// genuinely live row, whose amber is a 15% wash.
//
// This file fails if either table reaches for a promise colour again. It tests
// the VALUES the tables return, not the stylesheet, because that is where the
// defect lived: `Live.svelte` correctly wrote "Amber = it is in front of the
// congregation. Nothing else may use it" five lines above the rule that painted
// `--acc` with whatever `slideAccent` handed it.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { TYPE, TAXONOMY_INK, slideAccent, typeOf } from './plan.js';

/** Every token that carries a promise, with what it promises. */
const PROMISE = {
  '--v-amber': 'ON AIR',
  '--v-cyan': 'a guess',
  '--v-amethyst': 'rehearsal',
  '--v-grey': 'CUED',
  '--v-rose': 'destructive',
  '--v-red': 'destructive',
};

/**
 * Names a promise token, INCLUDING its variants — `--v-amber-soft`, `--v-amber2`,
 * `--v-cyan-line`. A plain substring match is what is wanted: a variant of a
 * promise colour is still that promise, and no non-promise token in `app.css`
 * begins with one of these names.
 */
function promiseIn(value) {
  const v = String(value || '');
  for (const [token, means] of Object.entries(PROMISE)) {
    if (v.includes(token)) return `${token} (${means})`;
  }
  return null;
}

describe('the colour law — a taxonomy may not paint a promise', () => {
  it('the scanner recognises a promise colour when it sees one', () => {
    // Guards the guard. A matcher that silently matched nothing would make every
    // assertion below pass vacuously, which is how two scanners in this repo were
    // wrong while looking exhaustive.
    expect(promiseIn('var(--v-amber)')).toContain('ON AIR');
    expect(promiseIn('var(--v-amethyst-soft)')).toContain('rehearsal');
    expect(promiseIn('var(--v-cyan-line)')).toContain('a guess');
    expect(promiseIn('var(--v-rose)')).toContain('destructive');
    expect(promiseIn('var(--v-faint)')).toBeNull();
    expect(promiseIn('var(--v-sel)')).toBeNull();
  });

  it('no cue type paints a promise colour', () => {
    const offenders = Object.entries(TYPE)
      .map(([kind, row]) => [kind, promiseIn(row.color)])
      .filter(([, hit]) => hit)
      .map(([kind, hit]) => `TYPE.${kind} → ${hit}`);
    expect(offenders).toEqual([]);
  });

  it('every cue type still NAMES its kind, because the label is now the taxonomy', () => {
    // The colour was removed on the argument that the words already carry it.
    // If a label were ever blank, that argument would be false and the kind
    // would be unreadable rather than merely uncoloured.
    for (const [kind, row] of Object.entries(TYPE)) {
      expect(row.label, `TYPE.${kind}.label`).toBeTruthy();
      expect(row.trig, `TYPE.${kind}.trig`).toBeTruthy();
    }
  });

  it('no slide tag paints a promise colour — including the ones that used to', () => {
    const tags = [
      'C', 'C1', 'C2', 'CHORUS', // was --v-amber, ON AIR
      'V', 'V1', 'V2', 'VERSE', // was --v-cyan, a guess
      'BR', 'B', 'B2', 'BRIDGE', // was --v-amethyst, rehearsal
      'NOTE', 'OUT', 'OUTRO', 'END', 'TAG', 'REF', // was --v-rose, destructive
      'BG', // was --v-amethyst, rehearsal
      'PC', 'PC2', 'INT', 'IL', // was --v-emerald
      '1', '2', '17', // was --v-faint, a hair from --v-grey / CUED
      '⏱', 'SCR', '', null, undefined, 'ZZZ-UNKNOWN',
    ];
    const offenders = tags
      .map((t) => [t, promiseIn(slideAccent(t))])
      .filter(([, hit]) => hit)
      .map(([t, hit]) => `slideAccent(${JSON.stringify(t)}) → ${hit}`);
    expect(offenders).toEqual([]);
  });

  it('both tables answer with the one taxonomy ink', () => {
    expect(slideAccent('C')).toBe(TAXONOMY_INK);
    expect(typeOf('song').color).toBe(TAXONOMY_INK);
    expect(promiseIn(TAXONOMY_INK)).toBeNull();
  });

  it('slideAccent does not mention a promise colour in its own source', () => {
    // The value test above can only see the tags it thought to try. This sees the
    // function: a new branch returning amber for some tag nobody listed fails here.
    const src = readFileSync(resolve(__dirname, './plan.js'), 'utf8');
    const body = src.slice(src.indexOf('export function slideAccent'));
    const fn = body.slice(0, body.indexOf('\n}') + 2);
    const offenders = Object.keys(PROMISE).filter((t) => fn.includes(`var(${t}`));
    expect(offenders).toEqual([]);
  });

  it('Live still reserves amber for ON AIR and grey for CUED, and selection is steel', () => {
    // The other half of the law: the promise colours must still be used where they
    // DO mean what they say. Removing them from the taxonomy is only half a fix if
    // the states stop being signalled.
    const live = readFileSync(resolve(__dirname, './views/Live.svelte'), 'utf8');
    expect(live).toMatch(/\.cue\.islive,\.slide\.islive\{border-color:var\(--v-amber\)/);
    expect(live).toMatch(/\.cue\.cued,\.slide\.cued\{[^}]*var\(--v-grey\)/);
    // Selection used to be cyan — "a guess" — one line above the amber rule.
    expect(live).toMatch(/\.cue\.sel\{border-color:var\(--v-sel-line\)\}/);
    expect(live).not.toMatch(/\.cue\.sel\{border-color:var\(--v-cyan/);
  });
});
