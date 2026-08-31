// RG-74 — "every text token at AA" was a ticked box with no instrument.
//
// `PRODUCT_AUDIT.md` §16 has claimed **"WCAG: focus traps + restore, operable
// controls, `<h1>`, every text token at AA"** as done. The first three are pinned by
// tests. The fourth was not checked by anything.
//
// It is not a hypothetical failure. `src/app.css` records that `--text-faint` was
// `#5f6470` — **2.27:1, an AA failure on every surface it was used on** — and
// `Stage.svelte` records the same for its standby text at 2.25:1, "the worst contrast
// in the product, in its least forgiving location". Both were found by a human
// reading hex codes, which is the method that let them ship in the first place.
//
// The arithmetic already existed: `legibility.js::contrastRatio` computes WCAG
// relative luminance, and is itself tested against the known extremes (21:1 and 1:1).
// It was pointed at TEMPLATES — what a congregation reads on a wall — and never at
// the console's own palette. This points it at the palette.
//
// ── What this deliberately does NOT do ───────────────────────────────────────
//
// It checks the DESIGN TOKENS, not the rendered UI. A component that puts
// `--v-faint` on an unusual background is out of scope here and always will be —
// that needs eyes on a running app this machine cannot see (DESIGN_SYSTEM §6). What
// it can guarantee is that the palette a developer reaches for is sound, so the
// remaining risk is misuse rather than a poisoned well.
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve } from 'node:path';
import { contrastRatio, parseColor } from './legibility.js';

const css = readFileSync(resolve(__dirname, '../app.css'), 'utf8');

/**
 * Read a `--token:#hex;` declaration out of app.css, as the {r,g,b} `contrastRatio`
 * actually takes.
 *
 * **It does NOT take a hex string** — it takes a parsed colour, and returns `NaN`
 * for a string. The first version of this file passed hex in, every ratio came back
 * `NaN`, `NaN < 4.5` is false, and all four token assertions passed **by checking
 * nothing**. That is the exact defect this session spent the day finding in other
 * people's instruments, reproduced here within the hour — and it was caught only by
 * the guard below, which is the argument for always writing one.
 */
function token(name) {
  const m = css.match(new RegExp(`--${name}\\s*:\\s*(#[0-9a-fA-F]{3,8})`));
  expect(m, `--${name} is not defined as a hex literal in app.css`).toBeTruthy();
  const rgb = parseColor(m[1]);
  expect(rgb, `--${name} = ${m[1]} did not parse`).toBeTruthy();
  return rgb;
}

// The dark shell's surfaces that muted text is ACTUALLY placed on, deepest to
// lightest. `--v-surf3` is excluded and the exclusion is asserted below rather than
// assumed — clearing AA there would need `#9B9B9B`, which collapses the muted step
// into `--v-dim` and costs the ramp its third level for a pairing nothing uses.
const SURFACES = ['v-void', 'v-bg', 'v-surf', 'v-surf2'];

/** The one surface muted text is kept off. The test is that it stays that way. */
const EXCLUDED_SURFACE = 'v-surf3';

// The three text weights. `--v-txt` is body, `--v-dim` is secondary, `--v-faint` is
// the quietest — and the quietest is the one that has failed before, twice.
const TEXT = ['v-txt', 'v-dim', 'v-faint'];

/** WCAG 2.1: 4.5:1 for body text, 3:1 for large text (≥18.66px bold / ≥24px). */
const AA_BODY = 4.5;
const AA_LARGE = 3;

describe('RG-74 · every text token clears AA on every surface it can sit on', () => {
  it('the tokens are all readable from app.css (the guard on everything below)', () => {
    // If the palette moves to another file or another notation, this must fail
    // loudly rather than silently check nothing.
    for (const t of [...TEXT, ...SURFACES]) {
      const c = token(t);
      expect(Number.isFinite(c.r) && Number.isFinite(c.g) && Number.isFinite(c.b)).toBe(true);
    }
  });

  it('the arithmetic agrees with the known extremes', () => {
    // Guards the imported function, not the palette. A contrastRatio that returned
    // a constant would make every assertion below pass.
    const w = { r: 255, g: 255, b: 255 };
    const b = { r: 0, g: 0, b: 0 };
    expect(contrastRatio(w, b)).toBeCloseTo(21, 1);
    expect(contrastRatio(w, w)).toBeCloseTo(1, 5);
    // And a ratio that is not a number is not a pass. `NaN < 4.5` is false, which is
    // how the first version of this file checked nothing while going green.
    expect(Number.isFinite(contrastRatio(token('v-txt'), token('v-bg')))).toBe(true);
  });

  for (const t of TEXT) {
    it(`--${t} clears AA on every surface`, () => {
      const failures = [];
      for (const s of SURFACES) {
        const ratio = contrastRatio(token(t), token(s));
        // `--v-faint` is used for captions and labels, which are body-sized. It gets
        // the body bar like the rest — the large-text exemption is for headings,
        // and nothing in this palette is a heading-only token.
        if (ratio < AA_BODY) failures.push(`--${t} on --${s} = ${ratio.toFixed(2)}:1`);
      }
      expect(
        failures,
        `WCAG AA is ${AA_BODY}:1 for body text. ${failures.join('; ')}. ` +
          `--text-faint once shipped at 2.27:1 and Stage's standby text at 2.25:1; ` +
          `both were found by a person reading hex codes, which is why this is a test.`,
      ).toEqual([]);
    });
  }

  it('nothing pairs the muted token with the surface it cannot clear', () => {
    // The exclusion above is only honest if it is true. `--v-faint` on `--v-surf3`
    // is 3.76:1, and the previous hand-written comment relied on "faint text is not
    // placed on surf3 in the app" being remembered by every future author. This
    // checks it instead — and if it ever fails, the fix is to change the rule, not
    // to widen the palette.
    const svelte = [];
    const walk = (dir) => {
      for (const e of readdirSync(resolve(__dirname, '..', dir), { withFileTypes: true })) {
        if (e.isDirectory()) walk(`${dir}/${e.name}`);
        else if (e.name.endsWith('.svelte')) svelte.push(`${dir}/${e.name}`);
      }
    };
    walk('.');
    const sources = [['app.css', css], ...svelte.map((f) => [f, readFileSync(resolve(__dirname, '..', f), 'utf8')])];

    const paired = [];
    for (const [name, text] of sources) {
      for (const m of text.matchAll(/\{[^{}]*\}/g)) {
        const rule = m[0];
        // A contrast pairing is a rule that sets BOTH a text colour and a
        // background. Matching on the token names alone reported `:root` against
        // itself — it names every token — and then the legacy alias block, where
        // `--text-faint: var(--v-faint)` is a definition wearing `var()`.
        const paints = /(^|[;{\s])background(-color)?\s*:\s*var\(--v-surf3\)/.test(rule);
        const writes = /(^|[;{\s])color\s*:\s*var\(--v-faint\)/.test(rule);
        if (paints && writes) {
          paired.push(`${name}: ${rule.slice(0, 70)}`);
        }
      }
    }
    expect(
      paired,
      `--v-faint on --${EXCLUDED_SURFACE} is 3.76:1 — below AA. Change the rule, ` +
        `not the token: clearing it needs #9B9B9B, which collapses muted into --v-dim.`,
    ).toEqual([]);
  });

  it('states the large-text bar it is deliberately not using', () => {
    // Recorded so the next person does not "fix" a failure by claiming the 3:1
    // exemption for a token that captions body copy.
    expect(AA_LARGE).toBe(3);
    expect(AA_BODY).toBeGreaterThan(AA_LARGE);
  });
});
