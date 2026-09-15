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

const read = (f) => readFileSync(resolve(process.cwd(), f), 'utf8');
const help = read('src/lib/views/Help.svelte');
const settings = read('src/lib/views/Settings.svelte');
const dock = read('src/lib/Dock.svelte');

/** Prose only — the markup an operator reads, comments stripped. */
const prose = (s) => s.replace(/<!--[\s\S]*?-->/g, '').replace(/^\s*\/\/.*$/gm, '');

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
