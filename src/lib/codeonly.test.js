// THE ONE COMMENT STRIPPER, AND THE RULE THAT THERE IS ONLY ONE (RG-169).
//
// Every static test in this repository that asks "does the CODE say this" takes
// the comments out first, and for a long time each one grew its own three-regex
// chain to do it. That pattern has cost this project twice, in both directions:
//
//   OVER-REMOVAL, which blinds a scanner. `names.test.js` stripped comments with
//   `.replace(/<!--…-->/g,'').replace(/\/\*…\*\//g,'').replace(/^\s*\/\/.*$/gm,'')`,
//   and a comment in `Stage.svelte` containing `:8032/api/*` opened a block
//   comment that ran to the next real `*/` SEVEN THOUSAND characters later. The
//   whole `ZONES` table went with it — the labels the preacher's own screen
//   renders — and the register read that page with its labels missing and
//   reported clean. A scanner that quietly narrows passes everything.
//
//   UNDER-REMOVAL, which cries wolf. `.replace(/<!--[\s\S]*?-->/g, '')` leaves a
//   dangling `<!--` behind a nested comment, so text that IS a comment survives
//   and is read as code. CodeQL flags that as
//   `js/incomplete-multi-character-sanitization` and raised three high-severity
//   alerts on PR #88, one per changed file.
//
// So this file holds two things: that `codeOnly` actually handles the cases the
// chains got wrong, and that no scanner has grown its own chain again.
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const ROOT = resolve(__dirname, '../..');

describe('codeOnly — the two directions the chains got wrong', () => {
  it('a `/*` inside a line comment does not open a block comment — the 7 KB bug, in miniature', () => {
    // THE EXACT SHAPE, from `Stage.svelte`. The old chain ran its block pass
    // first and over the whole file, so it could not know the `/*` it had found
    // was inside a `//`. Everything to the next real `*/` was blanked.
    const src = [
      "  // Hits the LAN HTTP API on :8031's sibling port (:8032/api/*)",
      "  const ZONES = ['reading', 'next', 'note'];",
      '  /* a real block comment */',
      "  const KEPT = 'after';",
    ].join('\n');
    const out = codeOnly(src);
    expect(out, 'the table after a `/*` in a line comment was eaten').toContain('ZONES');
    expect(out).toContain("'reading'");
    expect(out).toContain('KEPT');
    expect(out, 'the real block comment survived').not.toContain('a real block comment');
    expect(out, 'the line comment survived').not.toContain('sibling port');
  });

  it("a `/*` inside a STRING does not open one either", () => {
    // The same fact from the other side: a quoted run is skipped, because
    // `':8032/api/*'` in a string is a path and the string itself is code.
    const src = ["const url = '/api/*';", "const after = 'still here';"].join('\n');
    const out = codeOnly(src);
    expect(out).toContain('/api/*');
    expect(out).toContain('still here');
  });

  it('offsets and line numbers survive, because it blanks rather than deletes', () => {
    // This is what the chains could not do, and it is why a bounded
    // `[\s\S]{0,N}` window measured against a stripped file has to be measured
    // against THIS one (see `settingssections.test.js`'s note on `acceptCrash`).
    const src = 'const a = 1; // why\nconst b = 2;\n';
    const out = codeOnly(src);
    expect(out.length).toBe(src.length);
    expect(out.split('\n').length).toBe(src.split('\n').length);
    expect(out.indexOf('const b')).toBe(src.indexOf('const b'));
  });

  it('a nested `<!--` does not leave half a comment behind as code', () => {
    const out = codeOnly('<!-- outer <!-- inner --> <p>real</p>');
    expect(out, 'a dangling marker was read as markup').not.toContain('inner');
    expect(out).toContain('<p>real</p>');
  });

  it('a bare `http://host` outside a string keeps the rest of its line', () => {
    // The documented exemption, stated exactly: `//` is a comment only when it
    // does NOT follow a `:`. So the scheme survives and the line survives with
    // it. A LATER `//` on the same line still opens a comment, which is correct
    // and is asserted here rather than left to be discovered — I first wrote this
    // test asserting the opposite and it was the test that was wrong.
    const out = codeOnly('const u = http://host; const v = 2;');
    expect(out).toContain('http://host');
    expect(out).toContain('const v = 2');
    const trailing = codeOnly('const u = http://host; // why\nconst v = 2;');
    expect(trailing).toContain('http://host');
    expect(trailing, 'a trailing line comment survived').not.toContain('why');
    expect(trailing, 'the next line was eaten').toContain('const v = 2');
  });
});

describe('and no scanner has grown its own chain again — RG-169', () => {
  /** Every `.js` under `src/`, derived rather than typed. */
  const files = (() => {
    const out = [];
    const walk = (dir) => {
      for (const name of readdirSync(join(ROOT, dir))) {
        const rel = `${dir}/${name}`;
        if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
        else if (name.endsWith('.js')) out.push(rel);
      }
    };
    walk('src');
    return out;
  })();

  it('reads a real slice of the tree (the guard on the one below)', () => {
    // A sweep that matches nothing passes everything, which is the failure this
    // whole file is about. Named here rather than assumed.
    expect(files.length).toBeGreaterThan(100);
    expect(files).toContain('src/lib/codeonly.js');
  });

  it('nothing strips HTML comments by hand', () => {
    // THE SWEEP STRIPS ITS OWN INPUT, and that is the whole design of it. Three
    // files NAME the retired chain in prose — `codeonly.js` explaining what it
    // replaced, `hardrules.test.js` explaining that it uses a WALK instead, and
    // the header of this file. A sweep over raw text would have reported all
    // three, and the cheapest way to go green would be deleting the
    // explanations, which is the trade `colourlaw.test.js` and
    // `workspacegrammar.test.js` both record losing.
    //
    // So there is NO exemption list. Stripping first makes every one of those
    // files honestly clean, which is also the plainest demonstration that the
    // stripper works: a scanner that needs a carve-out for the thing it is about
    // is a scanner that has not solved the problem.
    const offenders = files.filter((f) =>
      /replace\(\/<!--/.test(codeOnly(readFileSync(join(ROOT, f), 'utf8'))),
    );
    expect(
      offenders,
      'use `codeOnly` from src/lib/codeonly.js — a private chain is how a scanner goes blind (RG-169)',
    ).toEqual([]);
  });

  it('nothing strips BLOCK comments by hand either — RG-283', () => {
    // THE SWEEP ABOVE CLOSED ONE DIRECTION AND LEFT THE OTHER OPEN, which is this
    // repository's most-repeated instrument fault wearing the costume of the
    // instrument built to stop it. The HTML half of the chain was forbidden the
    // day `codeonly.js` was written; the CSS half was not — and the CSS half is
    // the MORE dangerous of the two, because it is the one that ate 7 KB of
    // `Stage.svelte`. Twenty-two live call sites across fifteen scanners were
    // still hand-rolling it, and CodeQL found them before this file did.
    //
    // NEITHER HALF IS SPELLED OUT HERE, deliberately, and that is worth its own
    // sentence: writing the retired chain into this comment makes the sweep
    // report its own file. The stripper does blank a `//` line — but a stray
    // quote in an earlier comment can leave a run open across the newline, so
    // the `//` is read as being inside a string and the line survives. That is a
    // real limit of `codeOnly` and it is recorded rather than worked around: it
    // costs a false ALARM, never a blind spot, which is the cheap direction.
    //
    // Same design as the sweep above and for the same reason: the input is
    // stripped first, so the files that NAME the retired chain in prose are
    // honestly clean and nobody is ever paid to delete an explanation.
    const offenders = files.filter((f) =>
      /replace\(\/\\\/\\\*/.test(codeOnly(readFileSync(join(ROOT, f), 'utf8'))),
    );
    expect(
      offenders,
      'use `codeOnly` from src/lib/codeonly.js — a private chain is how a scanner goes blind (RG-283)',
    ).toEqual([]);
  });

  it('…and every file that had one is reaching for the shared stripper', () => {
    // The other half of the claim. Forbidding the chain is satisfied by a file
    // that stops stripping altogether, which would be a scanner reading prose as
    // code — so the count of files that IMPORT the stripper is asserted too, and
    // it may grow but not shrink.
    const importers = files.filter((f) =>
      /from '\.{1,2}(\/\.\.)*\/?codeonly\.js'/.test(readFileSync(join(ROOT, f), 'utf8')),
    );
    expect(importers.length, 'a scanner stopped stripping instead of converting').toBeGreaterThanOrEqual(50);
  });
});
