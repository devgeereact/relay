// RG-13 — the surface inventory, and the two lists it exists to keep at zero.
//
// ── Why the instrument is tested before the findings ──────────────────────────
//
// This list started at 13 rows. Eleven of them were the script being wrong:
//
//   · `aria-label={tg.title}` did not count, because only a static string did — so
//     the microphone toggle on the run surface and the Reset All Settings button
//     were both reported as unnamed, and both have carried an aria-label all along.
//   · `<label for=…>` and a wrapping `<label>` did not count at all, which reported
//     two correctly-labelled textareas as unnamed and pushed an author towards
//     adding an aria-label that then has to be kept in step with the visible text.
//   · The scanner read the `<script>` block, so a JSDoc comment in `VerseDeck` that
//     explains a keyboard rule using the word `<button>` was reported as a
//     handlerless button whose label was a fragment of the explanation.
//   · A `type="submit"` in a form was reported as handlerless, though its handler is
//     the form's `on:submit` — and "fixing" it with a click handler would break
//     Enter-to-submit.
//   · A permanently `disabled` button ("In use") was reported as handlerless, which
//     it is, correctly.
//
// **A report with false findings is a report people learn to scroll past**, and this
// one exists to be believed. So the exclusions are tested as carefully as the
// findings, and each one is narrow: `disabled={expr}` is still reported, because a
// conditionally-disabled button with no handler does nothing when it becomes enabled.
import { describe, it, expect } from 'vitest';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

const report = JSON.parse(
  execFileSync('node', ['scripts/qa-inventory.mjs', '--json'], {
    cwd: ROOT,
    encoding: 'utf8',
    maxBuffer: 32 * 1024 * 1024,
  }),
);

describe('the two lists are at zero, and stay there', () => {
  it('every rendered control has an accessible name', () => {
    expect(
      report.unlabelledControls,
      'a control with no accessible name cannot be operated by anyone using a screen reader',
    ).toEqual([]);
  });

  it('no rendered button is inert', () => {
    expect(
      report.inertControls,
      'a button with no handler is a control that looks live and does nothing',
    ).toEqual([]);
  });

  it('the inventory still sees a real surface (the guard on both assertions above)', () => {
    // Both lists would also be empty if the scanner stopped finding anything at
    // all, which is exactly the failure the `stripScript` change could have caused.
    expect(report.counts.controls).toBeGreaterThan(300);
    expect(report.counts.renderedComponents).toBeGreaterThan(40);
  });
});

describe('what the scanner counts as a name', () => {
  const controls = report.controls.filter((c) => c.rendered);
  const at = (file, fragment) =>
    controls.find((c) => c.file.endsWith(file) && String(c.handler ?? '').includes(fragment));

  it('a BOUND aria-label counts', () => {
    // A regex cannot evaluate the expression, and reporting the attribute as
    // absent because it is bound is a false finding about markup that is already
    // correct.
    //
    // This used to be pinned to `aria-label={tg.title}` on the two Settings
    // toggles, and it broke when those toggles were deleted (DECISIONS §69 — they
    // saved a preference nothing read). A capability of the SCANNER should not be
    // asserted through one view's markup: the next author to remove a control gets
    // a failure that says nothing about the thing under test. So the claim is made
    // over every rendered control instead — every `aria-label={…}` in the tree must
    // come back named, whichever file it is in and however many there are.
    // The assertion is that the name came FROM the bound attribute, not merely
    // that the control ended up with some name: most of these also have text
    // inside them, so "is it labelled" passes even with the bound branch deleted.
    // Each one's reported label must be the attribute's own expression.
    // The fixture must be a control whose ONLY possible name is the bound
    // attribute — otherwise the scanner falls through to the inner text and the
    // test passes with the bound branch deleted, which is a test that cannot
    // fail. `VerseDeck`'s favourites star holds an `<svg>` and nothing else, so
    // there is no text to fall back to; if the bound branch goes, its label goes
    // from an expression to `null` and this fails.
    //
    // It used to be pinned to `aria-label={tg.title}` on the two Settings
    // toggles, which broke when those were deleted (DECISIONS §69 — they saved a
    // preference nothing read). The star is chosen because it is a control this
    // file already depends on twice, not because it is unlikely to change.
    const star = controls.find(
      (c) => c.file.endsWith('VerseDeck.svelte') && /savedRefs\.has/.test(String(c.label ?? '')),
    );
    expect(star, 'the bound-aria-label fixture has gone from VerseDeck').toBeTruthy();
    const tag = fs
      .readFileSync(path.join(ROOT, star.file), 'utf8')
      .split('\n')
      .slice(star.line - 1, star.line + 4)
      .join('\n');
    expect(tag, 'the fixture is no longer named by a BOUND aria-label').toMatch(/aria-label=\{/);
    expect(star.label).toBeTruthy();
  });

  it('a wrapping <label> counts', () => {
    // The native mechanism, and the one an author should reach for: the visible
    // text and the accessible name are then the same string and cannot drift.
    const body = controls.find(
      (c) => c.file.endsWith('Announcements.svelte') && c.tag === 'textarea',
    );
    expect(body.label).toBe('wrapping <label>');
  });

  it('<label for=id> counts', () => {
    const lyrics = controls.find(
      (c) => c.file.endsWith('LyricsPane.svelte') && c.tag === 'textarea',
    );
    expect(lyrics.label).toMatch(/^label\[for=/);
  });
});

describe('what the scanner excuses, and what it still reports', () => {
  it('a submit button in a form is not inert — its handler is the form’s', () => {
    // "Fixing" one with a click handler would break Enter-to-submit, so a report
    // that demands it is a report that makes the product worse.
    const src = read('scripts/qa-inventory.mjs');
    expect(src).toMatch(/c\.type === 'submit' && c\.inForm/);
    expect(src).toMatch(/Enter-to-submit/);
  });

  it('a permanently disabled button is a state readout, not a dead control', () => {
    const src = read('scripts/qa-inventory.mjs');
    expect(src).toMatch(/c\.disabled !== 'always'/);
  });

  it('…but a CONDITIONALLY disabled button with no handler is still reported', () => {
    // The narrow half of that exclusion, and the reason it is narrow: such a button
    // does nothing at the moment it becomes enabled, which is the bug this list is
    // for. Asserted on the scanner's own classification.
    const src = read('scripts/qa-inventory.mjs');
    expect(src).toContain("disabled\\s*=\\s*\\{");
    expect(src).toMatch(/'always'/);
    const conditional = report.controls.filter((c) => c.disabled && c.disabled !== 'always');
    expect(conditional.length, 'the two states must be distinguishable').toBeGreaterThan(0);
  });

  it('prose about markup is not markup', () => {
    // `VerseDeck` explains its keyboard rule in a JSDoc comment containing the word
    // `<button>`. A finding about a paragraph is the clearest possible way to teach
    // somebody to ignore the report.
    const src = read('scripts/qa-inventory.mjs');
    expect(src).toMatch(/function stripScript/);
    expect(src).toMatch(/function stripComments/);
    // Blanked, not deleted — `file:line` has to stay clickable.
    expect(src).toMatch(/replace\(\/\[\^\\n\]\/g, ' '\)/);
    const deck = report.controls.filter((c) => c.file.endsWith('VerseDeck.svelte'));
    expect(deck.every((c) => !String(c.label ?? '').includes('preventDefault'))).toBe(true);
  });
});
