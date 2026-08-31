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
import { readFileSync, readdirSync, existsSync } from 'node:fs';
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
  // `.claude/` is skipped by the dot-directory rule above, and it is exactly where
  // the last stale citation was found: an agent brief that pointed at a deleted
  // component and told the agent not to file a defect that was already fixed
  // (RG-68). Instruction files are read by something that ACTS on them, so a dead
  // reference there is worse than one in prose.
  for (const dir of ['.claude/agents', '.claude/commands']) {
    try {
      for (const e of readdirSync(resolve(root, dir), { withFileTypes: true })) {
        if (e.isFile() && e.name.endsWith('.md')) out.push([join(dir, e.name), read(join(dir, e.name))]);
      }
    } catch {
      // Not every checkout has them; their absence is not a failure of this test.
    }
  }
  return out;
}

const FILES = citingFiles();

/** Every Rust source file, for resolving `module::item` citations. */
const RUST_SOURCES = (() => {
  const out = [];
  const walk = (dir) => {
    for (const e of readdirSync(resolve(root, dir), { withFileTypes: true })) {
      const rel = join(dir, e.name);
      if (e.isDirectory()) walk(rel);
      else if (e.name.endsWith('.rs')) out.push(read(rel));
    }
  };
  walk('src-tauri/src');
  return out;
})();

describe('RG-67 · every cross-reference resolves', () => {
  it('reads a real slice of the repository (the guard on the two below)', () => {
    // Both assertions would also pass if this found nothing at all.
    expect(FILES.length).toBeGreaterThan(50);
    expect(FILES.some(([f]) => f.endsWith('CLAUDE.md'))).toBe(true);
    expect(FILES.some(([f]) => f.endsWith('main.rs'))).toBe(true);
    // And the agent briefs, which the dot-directory rule would otherwise skip.
    expect(FILES.some(([f]) => f.includes('.claude/agents'))).toBe(true);
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

  it('every `docs/…` path cited anywhere is a file that exists', () => {
    // The third kind of dead citation, and the one that had the most instances:
    // **sixteen source comments cited `docs/relaydesign/`, a directory that is not
    // in this repository.** Six were the right file under the wrong directory name
    // (`docs/design/`), one had also been renamed, and four named documents that
    // were never here at all — a design log, a screens reference, an HTML mock.
    //
    // A comment pointing at a file nobody can open is worse than no comment: it
    // reads as "the reasoning is written down over there" and sends the next
    // person looking for it.
    const HISTORICAL = /\bdelete[ds]?\b|superseded|supersedes|no longer|not in this repo|is gone|are gone/i;
    const dangling = [];
    for (const [file, text] of FILES) {
      const lines = text.split('\n');
      lines.forEach((line, i) => {
        for (const m of line.matchAll(/docs\/[A-Za-z0-9_./-]+\.(?:png|md|html|sql|json)/g)) {
          if (existsSync(resolve(root, m[0]))) continue;
          // A citation may name a file that is gone, as long as it SAYS so — the
          // register records deletions on purpose, and losing that would be worse.
          const window = lines.slice(Math.max(0, i - 2), i + 3).join(' ');
          if (HISTORICAL.test(window)) continue;
          dangling.push(`${file}:${i + 1} → ${m[0]}`);
        }
      });
    }
    expect(
      dangling,
      `citations to files that do not exist: ${dangling.join(', ')}`,
    ).toEqual([]);
  });

  it('every `module::item` citation into the Rust tree resolves', () => {
    // The fourth dimension, and the subtlest. This repository **inverts a test's
    // name when the defect it describes closes** — `two_of_the_three_new_item_menu_
    // entries_are_dead` became `all_three_new_item_menu_entries_do_something`, which
    // is exactly the right thing to do (RG-46: closed findings are inverted, never
    // deleted). But the register rows that cited the old names were not updated, so
    // three entries pointed their *validation* at functions that no longer exist.
    //
    // A register row whose evidence cannot be found reads as an unproven claim, and
    // the reader has no way to tell "the test was renamed" from "there was no test".
    const rust = RUST_SOURCES.join('\n');
    // Only citations into OUR OWN modules are checkable. An allow-list of external
    // crates would need extending every time somebody cites `fs::write` or
    // `usize::MAX`, and would go quiet the moment it fell behind — the exact failure
    // this file exists to catch. The module set is derived from the tree instead, so
    // it maintains itself.
    const OURS = new Set(
      readdirSync(resolve(root, 'src-tauri/src'), { withFileTypes: true })
        .filter((e) => e.isFile() && e.name.endsWith('.rs'))
        .map((e) => e.name.replace(/\.rs$/, '')),
    );
    const dangling = [];
    for (const [file, text] of FILES) {
      for (const m of text.matchAll(/`([a-z_]+)(?:\.rs)?::([a-zA-Z0-9_]+)`/g)) {
        const [, mod, item] = m;
        if (!OURS.has(mod)) continue;
        // `db::mod` names the file `db/mod.rs`, not an item in it.
        if (item === 'mod') continue;
        // `docs/audits/` is frozen evidence — those documents may not be edited, so a
        // dangling citation there cannot be fixed in place and must not fail this
        // test. It is reported in the audit's own fix log instead, which is the
        // mechanism they already use for closures. (There is one: F8 cites
        // `detection::r4_07`, which has never existed.)
        if (file.includes('docs/audits/')) continue;
        const declared = new RegExp(`\\b(fn|struct|enum|const|static|type|mod)\\s+${item}\\b`);
        if (!declared.test(rust)) dangling.push(`${file} → ${mod}::${item}`);
      }
    }
    expect(
      dangling,
      `citations into the Rust tree that resolve to nothing: ${[...new Set(dangling)].join(', ')}`,
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
