// RG-41 — TWO OF THE SIX "NO HEADING" VIEWS ARE ROUTERS, AND A ROUTER MUST NOT
// HAVE ONE.
//
// RG-39 found six views a screen reader lands on with no heading at all and put a
// heading in each. Two of the six were not views: `Themes.svelte` and
// `Templates.svelte` were three lines that pick a child. A heading there would
// produce TWO headings for one screen, so a reader jumping by heading lands twice
// on the same view — which is worse than none, because it reads as two places.
//
// `surface.test.js`'s R3-12 block already records that correction, and it records
// it the way the finding was written: a hand-kept list of four files that must
// have a heading and one file that must not. That is the shape this repository
// keeps getting caught by. Two things it cannot see:
//
//   1. **A ROUTER ADDED TOMORROW.** The list names `Templates.svelte` and nothing
//      derives the set, so a second router would be asserted by nothing at all —
//      and a NEW WORKSPACE with no heading would be asserted by nothing either,
//      because only four of the nine view files are named.
//   2. **"AND MUST REALLY BE JUST THE SWITCH."** RG-41's own test cell asks for
//      that in as many words, and what holds it today is
//      `expect(t).toMatch(/mode === 'editor'/)` — a string that would still be
//      there under a page's worth of chrome, a pane head and an `<h2>`. The
//      exemption from the heading rule is EARNED by being nothing but the switch;
//      an assertion that cannot see the chrome cannot see the exemption expiring.
//
// So this file declares the routers and DERIVES THE TRIPWIRE: every other view
// file must carry a heading, and any file that paints nothing of its own and is
// not declared is an undeclared router, named by the test. Deriving membership
// itself was tried first and is wrong for a reason worth reading — see the note
// above `DECLARED_ROUTERS`.
//
// `Themes.svelte` is not in the list below and must not be added: themes became a
// desk inside Templates (DECISIONS §79) and were then folded into the template
// model itself (§87), so the file is gone. The router it left behind picks between
// two children — browse, or make — and both carry their own heading, which is
// asserted here too: the requirement did not disappear with the router, it moved
// to the children, and nothing checked that the children still honoured it.
//
//   npx vitest run src/lib/viewrouters.test.js

import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, dirname, basename } from 'node:path';
import { codeOnly, withoutBlock } from './codeonly.js';

const ROOT = resolve(__dirname, '../..');
const VIEWS = resolve(ROOT, 'src/lib/views');
const src = (p) => readFileSync(resolve(ROOT, p), 'utf8');

/** The markup half of a Svelte file: everything after the last `</script>`, with
 *  the `<style>` block and HTML comments removed. A comment mentioning `<h2>` is
 *  not a heading, and `.lib-sheeth` in a stylesheet is not an element.
 *
 *  THROUGH THE ONE STRIPPER (RG-169, RG-283), which this file was the last
 *  holdout from — and it hid there in a way worth recording, because three of
 *  the assertions below are negative (`.not.toMatch` a heading, `.toEqual([])`
 *  for own-painted tags) and every negative assertion passes on text that was
 *  thrown away.
 *
 *  Its private chain was the exact one `codeonly.test.js` forbids, and the sweep
 *  that forbids it reported this file clean for as long as it existed: `codeOnly`
 *  walked INTO the literal `/<!--…-->/`, read the `<!--` in it as a comment, and
 *  blanked the pattern down to `.replace(/   /g, '')`. The instrument could not
 *  see the one thing it was built to see. CodeQL found it instead
 *  (`js/incomplete-multi-character-sanitization`), and the root fix is in
 *  `codeonly.js`, which now skips a regex literal the way it already skipped a
 *  quoted run. */
function markup(text) {
  const i = text.lastIndexOf('</script>');
  const m = i === -1 ? text : text.slice(i + '</script>'.length);
  return withoutBlock(codeOnly(m), 'style');
}

/** Every tag name opened in a piece of markup, in source order. `svelte:*` special
 *  elements are dropped: `<svelte:window on:keydown>` is a listener, not something
 *  a reader lands on or a box on a screen. */
function tags(m) {
  return [...m.matchAll(/<([A-Za-z][A-Za-z0-9._:-]*)/g)]
    .map(([, t]) => t)
    .filter((t) => !t.startsWith('svelte:'));
}

const HEADING = /<h[1-6][\s>]/;
/** A Svelte component tag is Capitalised; an HTML element is not. */
const isComponent = (t) => /^[A-Z]/.test(t);
const isHeading = (t) => /^h[1-6]$/.test(t);

const FILES = readdirSync(VIEWS)
  .filter((f) => f.endsWith('.svelte'))
  .map((f) => `src/lib/views/${f}`);

/** THE ROUTERS, DECLARED — and the derivation below is the tripwire, not the list.
 *
 *  A PURELY DERIVED SET WAS TRIED FIRST AND IS WRONG, and it is worth writing down
 *  because it looks like the better instrument. Classifying a router as "a view
 *  file that paints no HTML of its own" means the two defects this file exists to
 *  catch both walk OUT of the set on their way in: put an `<h2>` in the router and
 *  it is no longer a router, so the heading case never runs; give it a wrapper
 *  `<div>` and the same. Both were reported by the vacuity guard, as *"no view file
 *  derives as a router any more"*, several assertions away from the thing that was
 *  actually wrong. A classifier a defect can leave describes the tree it is handed
 *  rather than the rule it is holding.
 *
 *  So membership is DECLARED, which is what makes the two cases below bite, and the
 *  derivation is inverted into a completeness check: a file that paints nothing of
 *  its own and is not on this list is an undeclared router, and the test names it.
 *  That is the direction the fragility should point — a hand-kept list cannot go
 *  quietly stale, and a declared router cannot quietly grow a page. */
const DECLARED_ROUTERS = ['src/lib/views/Templates.svelte'];
const LANDINGS = FILES.filter((f) => !DECLARED_ROUTERS.includes(f));
/** Files that render only child components — at most with a heading, which is the
 *  one thing a router might wrongly have and must still be caught for. */
const PAINTS_NOTHING_OF_ITS_OWN = FILES.filter((f) =>
  tags(markup(src(f))).every((t) => isComponent(t) || isHeading(t)),
);

/** The children a router picks, resolved from its own relative imports. */
function children(routerPath) {
  const dir = dirname(resolve(ROOT, routerPath));
  return [...src(routerPath).matchAll(/import\s+\w+\s+from\s+'(\.[^']+\.svelte)'/g)].map(([, rel]) =>
    resolve(dir, rel).slice(ROOT.length + 1),
  );
}

describe('RG-41 · a workspace router has no heading, and really is just the switch', () => {
  it('the tree still has both kinds, so neither rule below is vacuous', () => {
    // A scanner that quietly narrows passes everything. If either list ever went
    // empty, its `it.each` block would run over nothing and the suite would go
    // green having asserted nothing at all.
    expect(DECLARED_ROUTERS.length, 'no router is declared any more').toBeGreaterThan(0);
    expect(LANDINGS.length, 'every view file is declared a router').toBeGreaterThan(0);
    for (const f of DECLARED_ROUTERS) expect(FILES, `${f} is declared but is not in the tree`).toContain(f);
  });

  it('and a NEW router cannot arrive unasserted', () => {
    // The completeness half. A view file that paints nothing of its own is a
    // router whether or not anybody wrote it down, and the RG-39 sweep's own
    // hand-kept list is why this check exists: it named four files out of nine.
    const undeclared = PAINTS_NOTHING_OF_ITS_OWN.filter((f) => !DECLARED_ROUTERS.includes(f));
    expect(
      undeclared,
      `undeclared router(s): ${undeclared.join(', ')} — add to DECLARED_ROUTERS so the ` +
        'heading and switch rules below apply to them',
    ).toEqual([]);
  });

  it.each(DECLARED_ROUTERS)('%s is a router, so it carries NO heading', (f) => {
    const m = markup(src(f));
    expect(m, `${basename(f)} has a heading — a reader jumping by heading would land twice`).not.toMatch(
      HEADING,
    );
    // `role="heading"` is the same claim by another spelling, and the RG-39 sweep
    // only ever grepped for the element.
    expect(m, `${basename(f)} declares a heading role`).not.toMatch(/role=(["'])heading\1/);
  });

  it.each(DECLARED_ROUTERS)('%s really is just the switch — no chrome of its own', (f) => {
    const m = markup(src(f));
    // THE EXEMPTION IS EARNED, NOT DECLARED. A router is excused the heading rule
    // because it paints nothing; the moment it paints a wrapper, a pane head or a
    // toolbar it is a view, and a reader landing on it has nothing to jump to.
    const own = tags(m).filter((t) => !isComponent(t) && !isHeading(t));
    expect(own, `${basename(f)} paints HTML of its own: <${own.join('>, <')}>`).toEqual([]);
    // And it does switch: a file that renders one child unconditionally is a
    // re-export, not a router, and would not need this exemption at all.
    expect(m, `${basename(f)} branches on nothing`).toMatch(/\{#if\s/);
  });

  it.each(DECLARED_ROUTERS)('%s hands the heading to every child it picks', (f) => {
    const kids = children(f);
    expect(kids.length, `${basename(f)} imports no child component`).toBeGreaterThan(0);
    for (const kid of kids) {
      expect(src(kid), `${basename(kid)} renders no heading, and its router has none either`).toMatch(
        HEADING,
      );
    }
  });

  it.each(LANDINGS)('%s is a view a reader lands on, so it carries a heading', (f) => {
    expect(markup(src(f)), `${basename(f)} has no heading at all — RG-39, reopened`).toMatch(HEADING);
  });
});
