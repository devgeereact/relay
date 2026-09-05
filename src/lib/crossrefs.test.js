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

  // Where a document went when the tree was reorganised.
  //
  // `docs/qa/audits/*` are frozen — an audit that edits its own history stops being
  // evidence — so when a file they cite moves, the citation cannot be repaired in
  // place. Six citations across two of those documents were in exactly that
  // position. A redirect is the honest repair: the reader is told where the file
  // went, and the test below refuses to let the entry outlive its reason by
  // checking BOTH ends — the old path must be gone AND the new path must exist.
  //
  // Prefix entries end in `/`; everything else is an exact path.
  const MOVED = new Map([
    ['docs/audits/', 'docs/qa/audits/'],
    ['docs/RELAY_GAP.md', 'docs/qa/RELAY_GAP.md'],
    ['docs/PRODUCT_AUDIT.md', 'docs/qa/audits/PRODUCT-2026-07-13.md'],
  ]);

  /** The path a moved citation now names, or `null` if it did not move. */
  const redirect = (cited) => {
    for (const [from, to] of MOVED) {
      if (from.endsWith('/')) {
        if (cited.startsWith(from)) return to + cited.slice(from.length);
      } else if (cited === from) {
        return to;
      }
    }
    return null;
  };

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
      const frozen = file.includes('docs/qa/audits/');
      lines.forEach((line, i) => {
        for (const m of line.matchAll(/docs\/[A-Za-z0-9_./-]+\.(?:png|md|html|sql|json)/g)) {
          if (existsSync(resolve(root, m[0]))) continue;
          // A frozen audit CANNOT be edited to follow a file that moved, so a
          // redirect is the only honest repair available to it. Honoured for those
          // documents and nowhere else: everywhere a stale path fails today, it
          // still fails. See `MOVED` and the test that keeps it true.
          if (frozen && redirect(m[0])) continue;
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

  it('every relative Markdown link resolves to a file that exists', () => {
    // RG-87. The fifth kind of dead citation, and the only one with no instrument:
    // `[text](path.md)`.
    //
    // The other four checks read PROSE — a `§N`, an `RG-` id, a `docs/…` path, a
    // `module::item`. A Markdown link target is none of those shapes, so twenty-eight
    // of them broke in the 2026-09-02 reorganisation and every test in this file
    // stayed green. They were found by a throwaway script that was then thrown away,
    // which is the same as not having found them: the next move breaks more, silently,
    // and these documents are load-bearing enough that four tests already exist to
    // protect their other references.
    //
    // A broken link is worse than a bare filename in prose for the same reason a dead
    // `§16` is worse than an uncited claim: it presents as navigation that works.
    // A link inside a code span or a fenced block is not a link — it is a document
    // showing what link syntax looks like, which is exactly what a report about
    // broken links ends up doing. Strip both before scanning, or the checker
    // reports its own example.
    const prose = (text) =>
      text.replace(/```[\s\S]*?```/g, '').replace(/`[^`\n]*`/g, '');

    const dangling = [];
    for (const [file, raw] of FILES) {
      if (!file.endsWith('.md')) continue;
      const text = prose(raw);
      const dir = file.split('/').slice(0, -1).join('/') || '.';
      // A FROZEN audit's links were written where that document used to live, and it
      // may not be edited to follow a file that moved (see `MOVED` above). So its
      // links are resolved from its ORIGINAL directory as well — the same redirect,
      // applied to navigation rather than to prose, and honoured nowhere else.
      const frozen = file.includes('docs/qa/audits/');
      const wasAt = frozen
        ? [...MOVED].find(([, to]) => to === file.replace(/^\.\//, ''))?.[0]
        : null;
      const oldDir = wasAt ? wasAt.split('/').slice(0, -1).join('/') : null;

      for (const m of text.matchAll(/\[[^\]]*\]\(([^)\s]+)\)/g)) {
        const target = m[1];
        // External and same-page links are somebody else's problem.
        if (/^(https?:|mailto:|#)/.test(target)) continue;
        const [pathPart] = target.split('#');
        if (!pathPart) continue;
        const cited = decodeURIComponent(pathPart);
        if (existsSync(resolve(root, dir, cited))) continue;
        if (oldDir) {
          const asWritten = join(oldDir, cited).replace(/\\/g, '/');
          const moved = redirect(asWritten) ?? asWritten;
          if (existsSync(resolve(root, moved))) continue;
        }
        dangling.push(`${file} → ${target}`);
      }
    }
    expect(
      dangling,
      `Markdown links that go nowhere: ${dangling.join(', ')}`,
    ).toEqual([]);
  });

  it('every in-document anchor link points at a heading that exists', () => {
    // The sixth dimension, and the one a table of contents rots by. `[…](#a-heading)`
    // is skipped by the check above — it has no path — so a document can grow a
    // contents table, have its headings renamed underneath it, and go on presenting
    // twenty links that quietly land at the top of the page.
    //
    // Same principle as the rest of this file: a reference that LOOKS like navigation
    // and is not is worse than no reference. GitHub's slug rule is lowercase, strip
    // everything that is not a word character, space or hyphen, then spaces to
    // hyphens — which is why an em dash between two words leaves TWO hyphens.
    const slug = (h) =>
      h
        .trim()
        .toLowerCase()
        .replace(/[^\p{L}\p{N}\s_-]/gu, '')
        .replace(/\s/g, '-');

    const dangling = [];
    let checked = 0;
    for (const [file, text] of FILES) {
      if (!file.endsWith('.md')) continue;
      const headings = new Set(
        [...text.matchAll(/^#{1,6}\s+(.*)$/gm)].map((m) => slug(m[1])),
      );
      for (const m of text.matchAll(/\]\(#([^)\s]+)\)/g)) {
        checked += 1;
        if (!headings.has(decodeURIComponent(m[1]).toLowerCase())) {
          dangling.push(`${file} → #${m[1]}`);
        }
      }
    }
    expect(
      dangling,
      `anchor links that land nowhere: ${dangling.join(', ')}`,
    ).toEqual([]);
    // The guard, for the same reason as every other scanner here: this would also
    // pass over a regex that had stopped matching.
    expect(checked, 'the anchor scanner found no anchors at all').toBeGreaterThan(10);
  });

  it('the Markdown link check is actually reading links (the guard on the one above)', () => {
    // The failure mode of every scanner in this repository has been the same: it
    // narrows, keeps passing, and stops checking what it claims to. `ipc.test.js`
    // has done it twice. Count the links so a regex that matches nothing cannot
    // masquerade as a clean bill of health.
    const prose = (text) =>
      text.replace(/```[\s\S]*?```/g, '').replace(/`[^`\n]*`/g, '');
    let links = 0;
    for (const [file, raw] of FILES) {
      if (!file.endsWith('.md')) continue;
      for (const m of prose(raw).matchAll(/\[[^\]]*\]\(([^)\s]+)\)/g)) {
        if (!/^(https?:|mailto:|#)/.test(m[1])) links += 1;
      }
    }
    expect(links).toBeGreaterThan(100);

    // And the code-span stripping must not have swallowed the document. If a
    // regex ever eats more than it should, the link count collapses and the check
    // above passes over nothing — the same narrowing failure the scanner guards
    // in `hardrules.test.js` and `ipc.test.js`.
    const audit = FILES.find(([f]) => f.endsWith('RELAY_V1_AUDIT.md'));
    expect(audit, 'the audit is no longer being read').toBeTruthy();
    expect(prose(audit[1]).length).toBeGreaterThan(audit[1].length * 0.5);
  });

  it('every redirect is still needed, and still points somewhere real', () => {
    // The same shape as `KNOWN_ABSENT`'s guard below, and for the same reason: an
    // exception that outlives its reason is a hole. If a moved file is ever put
    // back, or moved again, this fails rather than quietly forgiving a citation
    // that has become genuinely wrong.
    const broken = [];
    for (const [from, to] of MOVED) {
      if (existsSync(resolve(root, from))) {
        broken.push(`${from} exists again — drop the redirect`);
      }
      if (!existsSync(resolve(root, to))) {
        broken.push(`${from} → ${to}, which does not exist`);
      }
    }
    expect(broken, `redirects that no longer hold: ${broken.join(', ')}`).toEqual([]);
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

    // Named absences, each with its reason. NOT a keyword allowance: the first
    // version of this excused any citation whose LINE contained a word like
    // "deleted" or "renamed", and register rows are single enormous lines that
    // almost always contain one — it blunted the check completely, and a genuinely
    // stale citation stopped being caught. A short explicit list cannot do that.
    const KNOWN_ABSENT = new Set([
      // F8 of the 2026-08-14 audit names this as a second pin. It has never existed
      // — the R4 tests run r4_01 … r4_06 — and that audit may not be edited, so the
      // correction lives in its fix log and this register records it. Recording it
      // is the point, so it must not fail here.
      'detection::r4_07',
    ]);
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
        // `docs/qa/audits/` is frozen evidence — those documents may not be edited, so a
        // dangling citation there cannot be fixed in place and must not fail this
        // test. It is reported in the audit's own fix log instead, which is the
        // mechanism they already use for closures. (There is one: F8 cites
        // `detection::r4_07`, which has never existed.)
        if (file.includes('docs/qa/audits/')) continue;
        const declared = new RegExp(`\\b(fn|struct|enum|const|static|type|mod)\\s+${item}\\b`);
        if (declared.test(rust)) continue;
        if (KNOWN_ABSENT.has(`${mod}::${item}`)) continue;
        dangling.push(`${file} → ${mod}::${item}`);
      }
    }
    expect(
      dangling,
      `citations into the Rust tree that resolve to nothing: ${[...new Set(dangling)].join(', ')}`,
    ).toEqual([]);
  });

  it('every known-absent citation is still genuinely absent', () => {
    // An exception that outlives its reason is a hole. If `detection::r4_07` is ever
    // written, this list must lose it rather than silently permit a real citation.
    const rust = RUST_SOURCES.join('\n');
    for (const ref of ['detection::r4_07']) {
      const item = ref.split('::')[1];
      const declared = new RegExp(`\\b(fn|struct|enum|const|static|type|mod)\\s+${item}\\b`);
      expect(declared.test(rust), `${ref} now exists — remove it from KNOWN_ABSENT`).toBe(false);
    }
  });

  it('every RG- id mentioned anywhere exists in the register', () => {
    const doc = read('docs/qa/RELAY_GAP.md');
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
