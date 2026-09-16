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
    expect(page).toMatch(
      /resolveOutputTemplate\(t, override, !!content\?\.template_pinned, defaultTpl\)/,
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
    expect(outputs).toMatch(/\$: scriptureLook = \$templates[\s\S]{0,80}\$contentTemplates/);
  });

  it('the preview caption says which of the two situations it is in', () => {
    // Rule 35 in small. "Sample" read the same for a screen with its own look and
    // for one following a look it never showed.
    expect(outputs).toMatch(/follows the content look/);
  });
});
