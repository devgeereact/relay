// THE WORDS THAT TEACH THE APP MUST NAME CONTROLS THE APP HAS.
//
// The interface was rebuilt on 2026-09-14 and four instructions were left behind,
// each read by one kind of person: somebody who does not know Relay, usually under
// pressure. This file holds the class, not the four instances, because a fifth
// will be written the next time a control moves.
//
//   · Help's PANIC topic said "Or click Emergency Stop — it is in the top-right of
//     every screen, always." That control was deleted. It is the sentence a
//     volunteer reads in the four seconds after the wrong verse appears.
//   · Settings offered `v c b` as song-section keys. `SHORTCUTS` carries 'B', so
//     `RESERVED` carries `b`, so `assignKeys` can never issue it — a Bridge is on
//     `r` for exactly this reason. A volunteer who followed the page would BLACK
//     OUT the congregation's screens, and the same sentence promised "never one
//     this page lists".
//   · Four places said "press Start listening", which no visible control said.
//   · A dead microphone reached the degraded register nowhere.

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { SHORTCUTS } from './shortcuts.js';
import { degradations } from './degraded.js';
import { codeOnly } from './codeonly.js';

const read = (f) => readFileSync(resolve(process.cwd(), f), 'utf8');
const help = read('src/lib/views/Help.svelte');
const settings = read('src/lib/views/Settings.svelte');
const dock = read('src/lib/Dock.svelte');

/** Prose only — the markup an operator reads, comments stripped. */
const prose = (s) => codeOnly(s);

/**
 * THE DOCUMENTS ARE INSTRUCTIONS TOO.
 *
 * The first version of this file swept three `.svelte` files and found four
 * defects. It missed two more of exactly the same kind, in `docs/` — including
 * `AI_DISCLOSURE.md` telling an operator that **Emergency Stop** "is in the top
 * bar of every screen", which is the congregation-safety document naming a
 * control deleted on 2026-09-14.
 *
 * A scan that covers three of five doors is the narrowing failure this repository
 * has now recorded four times. Every surface that tells an operator to press
 * something is in range.
 */
const INSTRUCTION_SURFACES = [
  'src/lib/views/Help.svelte',
  'src/lib/views/Settings.svelte',
  'src/lib/Dock.svelte',
  'src/lib/FirstRun.svelte',
  'docs/USER_GUIDE.md',
  'docs/AI_DISCLOSURE.md',
];

describe('no instruction anywhere names a control that was deleted', () => {
  // Each was a real control once. `panic.test.js` and `shellchrome.test.js` assert
  // the first is gone from the code; nothing asserted it was gone from the prose.
  const GONE = ['Emergency Stop'];

  for (const file of INSTRUCTION_SURFACES) {
    it(`${file} names none of them`, () => {
      const text = prose(read(file));
      for (const control of GONE) {
        expect(text, `${file} still tells somebody to use "${control}"`).not.toContain(control);
      }
    });
  }

  it('…and the scan can still see the surfaces it is about', () => {
    // A list that quietly stops resolving passes everything.
    for (const f of INSTRUCTION_SURFACES) expect(read(f).length).toBeGreaterThan(200);
  });
});

describe('an instruction names a control that exists', () => {
  it('Help does not send a panicking operator to a deleted button', () => {
    expect(prose(help)).not.toContain('Emergency Stop');
    // …and it names the one that IS there, in the place it actually is.
    expect(prose(help)).toContain('Clear screens');
  });

  it('Settings never offers a panic key as a song-section key', () => {
    const reserved = new Set(
      SHORTCUTS.flatMap((s) => s.keys).filter((k) => k.length === 1).map((k) => k.toLowerCase()),
    );
    // Every <kbd> on the shortcuts page's section-key sentence.
    const sentence = prose(settings).slice(
      prose(settings).indexOf("song's sections take single letters"),
    );
    const offered = [...sentence.slice(0, 400).matchAll(/<kbd class="s-kbd">([a-z0-9])<\/kbd>/g)].map(
      (m) => m[1],
    );
    expect(offered.length, 'the scan must still find the example keys').toBeGreaterThan(1);
    for (const k of offered) {
      expect(reserved.has(k), `Settings offers \`${k}\` as a section key, and it is a reserved key`).toBe(
        false,
      );
    }
  });

  it('the listening control carries the word the instructions use', () => {
    // It was an unlabelled glyph captioned `off`, which reads as a status.
    expect(prose(dock)).toMatch(/\$capture\.capturing \? 'Listening' : 'Listen'/);
    expect(prose(help)).toContain('<b>Listen</b>');
  });

  it('a dead microphone reaches the degraded register, and says what to do', () => {
    const rows = degradations({ audioError: 'input device not found: Scarlett 2i2' });
    const row = rows.find((r) => r.id === 'audio');
    expect(row, 'a failed capture must produce a row').toBeTruthy();
    expect(row.level).toBe('blocked');
    expect(row.fix, 'and the fix must be an action, not a restatement').toMatch(/cable|device|microphone/i);
  });

  it('…and it says nothing when the microphone is fine', () => {
    // An instrument that always reports is one an operator learns to ignore.
    expect(degradations({}).find((r) => r.id === 'audio')).toBeUndefined();
  });
});
