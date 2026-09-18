// W5 · A SCREEN CARD PREVIEWS WHAT THAT SCREEN REALLY SHOWS. docs/REBRAND.md §5.
//
// The Outputs inspector renders the wall's own component through the wall's own
// resolver, which is what makes it a preview rather than a drawing of one. It had
// one coercion in it — `templateOf(sel) ?? {}` — and that coercion was the whole
// bug: `{}` is truthy, so `resolveOutputTemplate` never reached the branch that
// answers for a screen with no look of its own, and the ONE screen whose look you
// cannot read off its own row was the one the panel could not answer for.
//
//   npx vitest run src/lib/screenpreview.test.js
//
//   CLAUDE.md rule 35 (two surfaces, one fact) · DECISIONS §29 · §70
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { resolveOutputTemplate } from './layers.js';
import { DEFAULT_TEMPLATE } from './templates.js';

const read = (p) => readFileSync(path.resolve(__dirname, '..', p), 'utf8');
/** The same file with its prose removed. A comment that NAMES the defect is not the
 *  defect, and a scanner that cannot tell the two apart reports the fix as the bug. */
const code = (src) =>
  src
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .replace(/^\s*\/\/.*$/gm, '');

// A content look: a full-screen, opaque scripture template.
const LOOK = {
  id: 9,
  name: 'Nocturne',
  layout: { layers: [{ type: 'background', fill: '#101018' }] },
  style: { verseSize: 6 },
};

describe('a screen that follows the content look', () => {
  it('an EMPTY OBJECT is not the same answer as NO TEMPLATE — and it was the wrong one', () => {
    // What the wall does. `null` means "this screen has no look of its own", so the
    // content look is what it wears.
    expect(resolveOutputTemplate(null, LOOK, false)).toBe(LOOK);

    // What the inspector used to do. `{}` is truthy, so the follower branch is
    // skipped; `isKeyedTemplate({})` is true (no layers, no background), the
    // transparency law keeps the channel template, and the preview paints an empty
    // frame — for every follower, live or idle.
    expect(resolveOutputTemplate({}, LOOK, false)).not.toBe(LOOK);
  });

  it('both surfaces fall back to the same floor when no look is set either', () => {
    expect(resolveOutputTemplate(null, null, false) || DEFAULT_TEMPLATE).toBe(DEFAULT_TEMPLATE);
  });

  it("a screen with its OWN look is untouched by a content look — DECISIONS §29", () => {
    const own = { id: 3, name: 'Mine', layout: { layers: [{ type: 'background', fill: '#000' }] } };
    expect(resolveOutputTemplate(own, LOOK, false)).toBe(own);
    // …unless the cue pinned one deliberately.
    expect(resolveOutputTemplate(own, LOOK, true)).toBe(LOOK);
  });
});

describe('the inspector and the output page resolve the same way', () => {
  const outputs = code(read('lib/views/Channels.svelte'));
  const page = code(read('Output.svelte'));

  it('neither coerces a missing template into an object', () => {
    expect(
      outputs,
      'the inspector must pass the screen`s template as it is — `?? {}` is the defect',
    ).not.toMatch(/templateOf\(sel\)\s*\?\?\s*\{\}/);
    // `sel ? <the screen's own template, or null> : null`. It was written
    // `templateOf(sel)`, which reads `$templates` INSIDE a function body — so the
    // panel was right when a screen was selected and never repainted when that
    // template was edited, while the cards beside it did. Naming the store in the
    // reactive statement is what makes the two agree; the test below holds that.
    expect(outputs).toMatch(/resolveOutputTemplate\(\s*sel \? selOwn : null,/);
    expect(outputs, 'the inspector must NAME $templates, not reach it through a helper').toMatch(
      /\$: selOwn =[\s\S]{0,120}\$templates\.find/,
    );
    // …and the page passes its screen's template as it is, with the per-kind look
    // as the RUNG-3 ARGUMENT rather than as a second call (DECISIONS §97). The
    // two-call form is the defect: it applies the transparency law between the
    // per-kind look and the blanket template and silently discards an opaque
    // Announcement look on a lower-third screen.
    expect(page).toMatch(
      /resolveOutputTemplate\(t, override, !!content\?\.template_pinned, defaultTpl, kindLook\)/,
    );
    // And the component is actually handed that answer, not a second one built
    // inline — a preview resolved twice is a preview that can disagree with itself.
    expect(outputs).toMatch(/<TemplateRender template=\{previewTemplate\}/);
  });

  it('both name DEFAULT_TEMPLATE as the floor', () => {
    for (const [name, src] of [
      ['Channels.svelte', outputs],
      ['Output.svelte', page],
    ]) {
      expect(src, `${name} must fall back to the same floor the other one uses`).toMatch(
        /\|\|\s*DEFAULT_TEMPLATE/,
      );
    }
  });

  it('the preview names its stores rather than calling a helper that reads them', () => {
    // Svelte tracks the identifiers in a reactive expression. `lookName('scripture')`
    // reads `$templates` and `$contentTemplates` INSIDE a function, so the preview
    // would be right once, by luck of ordering, and never update again — the exact
    // trap `stageUrl()` fell into a few lines above it in this same file.
    //
    // BOTH IDENTIFIERS IN THE ONE STATEMENT is the whole claim, and it is not a
    // claim about the shape of the expression around them. The first version of
    // this pinned `= $templates` literally, so passing the two stores to the same
    // lookup the output page resolves a content look through — which names both,
    // in the statement, and is the point of doing it — failed a test whose own
    // comment says what it is checking. A regex tighter than its reason is a
    // regex that refuses the fix.
    expect(outputs).toMatch(/\$: scriptureLook =[^;]*\$templates[^;]*\$contentTemplates/);
  });

  it('the preview caption says which of the two situations it is in', () => {
    // Rule 35 in small. "Sample" read the same for a screen with its own look and
    // for one following a look it never showed.
    expect(outputs).toMatch(/follows the content look/);
  });
});

// ── THE DESK RESOLVES THE PER-KIND LOOK, OR IT IS THE ONE SURFACE THAT LIES ───
//
// DECISIONS §97. This file's own opening paragraph records why: the Outputs tile
// used to resolve a content look correctly while IDLE over a wall that was
// wearing something else, so the panel an operator opens to CHECK the setup was
// the one surface that made a broken setup look right. A per-kind look reaches
// the wall at rung 3; a preview that stops at rung 4 reproduces exactly that, one
// column along.
//
// Asserted on the SOURCE, like the assertions above it, because a card and an
// inspector rendering the same screen is not a thing jsdom can measure — it
// computes no layout. What it can hold is that both call sites pass the rung.
describe('the desk resolves the same rungs the wall does', () => {
  const outputs = code(read('lib/views/Channels.svelte'));

  it('the inspector preview passes the per-kind look', () => {
    expect(
      outputs,
      'the inspector resolved a screen without its per-kind look — the panel an ' +
        'operator opens to check the setup would show the blanket template over a ' +
        'wall wearing something else, which is this file’s own recorded defect',
    ).toMatch(/\$: previewTemplate =[\s\S]{0,400}previewKindLook/);
  });

  it('and every card resolves its OWN, for the same kind', () => {
    // NOT the inspector's value, and that distinction is the assertion. Every
    // card is a different screen; a card that borrowed `previewKindLook` would
    // paint the SELECTED screen's look on every tile, which is a worse lie than
    // not resolving the rung at all — it would look like a whole wall correctly
    // configured to one thing.
    expect(
      outputs,
      'the cards do not resolve their own per-kind look, so a desk of four ' +
        'screens shows the blanket template over four different walls',
    ).toMatch(/\$: cards =[\s\S]{0,1400}lookIdFor\(\$channelLooks, c\.id, previewKind\)/);
    expect(
      outputs,
      'a card borrowed the inspector\u2019s per-kind look — every tile would wear ' +
        'the selected screen\u2019s',
    ).not.toMatch(/\$: cards =[\s\S]{0,1400}previewKindLook/);
  });

  it('the kind it resolves for is the kind the preview is showing', () => {
    // Idle the stand-in is a verse, so the look that applies is the SCRIPTURE
    // one; live it is whatever is on air. A preview that always asked about
    // scripture would be wrong on exactly the screens this feature exists for.
    expect(outputs).toMatch(/\$: previewKind =[\s\S]{0,200}\$liveContent/);
    expect(outputs).toMatch(/\$: previewKind =[\s\S]{0,200}'scripture'/);
  });
});
