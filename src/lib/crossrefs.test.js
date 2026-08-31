// RG-67 — a citation that resolves to nothing is worse than no citation.
//
// `ARCHITECTURE.md` and `SPEC.md` cited "DECISIONS.md §16" for the 0.50/0.35
// threshold seed, in three places. **The numbered decision log starts at §18.**
// The decision itself is real and recorded — it is one of the unnumbered table rows
// that predate the numbering — but a reader following the citation lands on nothing
// and cannot tell whether the decision is missing or the document is wrong.
//
// That is worse than an uncited claim, which at least announces that it needs
// checking. It is the same failure as a status badge that cannot detect its own
// fault, in prose: the reference LOOKS like evidence.
//
// This repository leans on cross-references more than most — CLAUDE.md's rules cite
// DECISIONS, DECISIONS cites the register, the register cites both, and code
// comments cite all three. So the citations are load-bearing, and nothing was
// checking them. Three sweeps corrected counts and none of them looked at whether a
// section number still pointed anywhere.
//
// Static and fast: it reads files, mounts nothing, and touches no product code.
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, join } from 'node:path';

const root = resolve(__dirname, '../..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

/** Every text file that is allowed to carry a citation. */
function citingFiles() {
  const out = [];
  const walk = (dir, depth = 0) => {
    for (const e of readdirSync(resolve(root, dir), { withFileTypes: true })) {
      if (e.name === 'node_modules' || e.name === 'target' || e.name === 'dist') continue;
      if (e.name.startsWith('.')) continue;
      const rel = join(dir, e.name);
      if (e.isDirectory()) {
        if (depth < 3) walk(rel, depth + 1);
      } else if (/\.(md|rs|js|svelte)$/.test(e.name) && !e.name.includes('.test.')) {
        out.push([rel, read(rel)]);
      }
    }
  };
  walk('.');
  return out;
}

const FILES = citingFiles();

describe('RG-67 · every cross-reference resolves', () => {
  it('reads a real slice of the repository (the guard on the two below)', () => {
    // Both assertions would also pass if this found nothing at all.
    expect(FILES.length).toBeGreaterThan(50);
    expect(FILES.some(([f]) => f.endsWith('CLAUDE.md'))).toBe(true);
    expect(FILES.some(([f]) => f.endsWith('main.rs'))).toBe(true);
  });

  it('every "DECISIONS §N" points at a numbered decision that exists', () => {
    const headings = new Set(
      [...read('docs/DECISIONS.md').matchAll(/^## (\d+)\./gm)].map((m) => Number(m[1])),
    );
    // The log starts at §18 on purpose: everything before it is an unnumbered table
    // row from before the numbering existed. Citing one of those by number is the
    // exact mistake this test was written for, so the set is the authority.
    expect(headings.size).toBeGreaterThan(20);

    const dangling = [];
    for (const [file, text] of FILES) {
      for (const m of text.matchAll(/DECISIONS(?:\.md\))?\s*§(\d+)/g)) {
        const n = Number(m[1]);
        if (!headings.has(n)) dangling.push(`${file} → §${n}`);
      }
    }
    expect(
      dangling,
      `citations point at DECISIONS sections that do not exist: ${dangling.join(', ')}`,
    ).toEqual([]);
  });

  it('every RG- id mentioned anywhere exists in the register', () => {
    const doc = read('docs/RELAY_GAP.md');
    const start = doc.indexOf('## 23. Gap register');
    const end = doc.indexOf('## 24. GO / NO-GO');
    expect(start, 'the register is gone').toBeGreaterThan(-1);
    // A row is identified by its STATUS MARKER, never by the id alone — the
    // "Depends on" column contains cells that read exactly like a row start.
    const ids = new Set(
      [...doc.slice(start, end).matchAll(/\|\s*(?:✅|⏳|⚠️|~~)\s?RG-(\d+)/g)].map((m) =>
        Number(m[1]),
      ),
    );
    expect(ids.size).toBeGreaterThan(40);

    const dangling = [];
    for (const [file, text] of FILES) {
      for (const m of text.matchAll(/\bRG-(\d+)\b/g)) {
        const n = Number(m[1]);
        if (!ids.has(n)) dangling.push(`${file} → RG-${n}`);
      }
    }
    expect(
      dangling,
      `ids referenced with no register row: ${[...new Set(dangling)].join(', ')}`,
    ).toEqual([]);
  });
});
