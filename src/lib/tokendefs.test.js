// A `--v-*` a component reaches for and `app.css` never defines.
//
// This is the quietest failure the token layer has. `var(--v-nope)` with no
// fallback is invalid-at-computed-value-time: the declaration is not dropped, it
// becomes `unset` — `inherit` for an inherited property, `initial` for one that
// is not. Nothing throws, nothing logs, the build is green, the class is spelled
// correctly, and the element renders. It just renders in a colour nobody chose.
//
// Two were live on `rebrand/base` when this file was written, both pre-existing
// on every branch, and both were measured in a browser rather than argued from
// the stylesheet:
//
//   · `History.svelte` armed the second half of a two-step DELETE with
//     `color:var(--v-ink)`. `color` is inherited, so the armed button drew
//     --v-txt (#e8eaee) on the red fill: **2.82:1**, a WCAG AA failure on the one
//     word an operator has to read before a service record goes. --v-inverse,
//     which is what it meant, is 5.41:1.
//   · `Live.svelte` hovered a screen-repair button with
//     `border-color:var(--v-txt-dim)`. `border-color` is NOT inherited, so it
//     fell back to `currentColor` — set to --v-txt by the same declaration block
//     — and the control its own comment calls "deliberately quiet" grew the
//     loudest border in the pane.
//
// Neither is visible to `tokencontrast.test.js`, which checks the palette a
// developer reaches for and says so explicitly. This checks that what they
// reached for exists.
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve, join, extname } from 'node:path';

const SRC = resolve(__dirname, '..');
const CSS = resolve(SRC, 'app.css');

/** Every file a `--v-*` can be written in: components, the stylesheets, the JS. */
const EXTS = new Set(['.svelte', '.css', '.js', '.html']);

function walk(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === 'node_modules') continue;
    const p = join(dir, name);
    if (statSync(p).isDirectory()) walk(p, out);
    // A test file renders nothing, and prose about a token is not a use of it —
    // this file's own header names both of the tokens it was written for.
    else if (EXTS.has(extname(name)) && !name.endsWith('.test.js')) out.push(p);
  }
  return out;
}

/**
 * Block comments out. A token NAMED in a comment is not a token used, and — the
 * reason this exists at all — `app.css` separates declarations with them:
 * `--v-void:#131418;   /* … *\/\n  --v-bg:#1a1c21;` puts a whole comment between
 * the `;` and the next name. Without this the definition scanner saw 0 of 99,
 * and the whole file reported every token in the palette as missing.
 */
const strip = (s) => s.replace(/\/\*[\s\S]*?\*\//g, ' ').replace(/<!--[\s\S]*?-->/g, ' ');

const FILES = walk(SRC);

/**
 * Tokens `app.css` DEFINES. A definition is `--name:` in a declaration position,
 * which is what follows a `{` or a `;` — never the `--name` inside a `var(…)`,
 * which is preceded by `(`.
 */
const DEFINE_RE = /[;{]\s*(--v-[a-z0-9-]+)\s*:/g;
const USE_RE = /var\(\s*(--v-[a-z0-9-]+)/g;

function defined(src = readFileSync(CSS, 'utf8')) {
  const out = new Set();
  for (const m of strip(src).matchAll(DEFINE_RE)) out.add(m[1]);
  return out;
}

/** Every `var(--v-…)` reference anywhere under `src/`, with the file that wrote it. */
function referenced() {
  const out = new Map();
  for (const f of FILES) {
    for (const m of strip(readFileSync(f, 'utf8')).matchAll(USE_RE)) {
      if (!out.has(m[1])) out.set(m[1], []);
      out.get(m[1]).push(f.slice(SRC.length + 1));
    }
  }
  return out;
}

describe('every --v-* a component reaches for is defined in app.css', () => {
  // ── The guards. A scanner that quietly stops seeing things passes everything,
  // and this repository has watched that happen twice to `ipc.test.js` — first a
  // regex that excluded `_`, then a source list of one file. Both looked
  // exhaustive. So the scanner's own reach is asserted before its verdict is
  // trusted.
  it('the scanner reads the whole tree, not one file', () => {
    expect(FILES.length).toBeGreaterThan(80);
    expect(FILES.some((f) => f.endsWith('app.css'))).toBe(true);
    expect(FILES.some((f) => f.endsWith('App.svelte'))).toBe(true);
    // Both of the real defects were several directories down. A walker that
    // stopped at the first level would still satisfy everything above this line.
    expect(FILES.some((f) => f.endsWith(join('views', 'Live.svelte')))).toBe(true);
    expect(FILES.some((f) => f.endsWith(join('library', 'History.svelte')))).toBe(true);
    expect(FILES.every((f) => !f.endsWith('.test.js'))).toBe(true);
  });

  it('the two scanners each find the palette they are looking at', () => {
    const d = defined();
    // A representative of each part of the palette, so a regex that stopped
    // matching one shape of declaration cannot pass unnoticed.
    for (const t of ['--v-txt', '--v-amber', '--v-sel-fill', '--v-r-sm', '--v-fs-b1'])
      expect(d.has(t), `${t} should be DEFINED in app.css`).toBe(true);
    expect(d.size).toBeGreaterThan(60);

    const r = referenced();
    expect(r.size).toBeGreaterThan(60);
    expect(r.has('--v-txt')).toBe(true);
  });

  it('a definition is not counted from inside a var() reference', () => {
    // `var(--v-x)` must never look like a definition of `--v-x`, or a token that
    // is only ever REFERENCED would appear to define itself and this whole file
    // would assert nothing. Run against a literal rather than against app.css,
    // so the guard keeps its meaning on the day the palette happens to define
    // everything it uses — which is the day this file is supposed to be green.
    const sample = ':root{ --v-a:#fff; --v-b:#000 }\n.x{ color:var(--v-c); border:var(--v-d) }';
    expect([...defined(sample)].sort()).toEqual(['--v-a', '--v-b']);
    expect([...strip(sample).matchAll(USE_RE)].map((m) => m[1]).sort())
      .toEqual(['--v-c', '--v-d']);
  });

  it('does not read a token out of a comment', () => {
    // Both halves. A stylesheet that DISCUSSES `var(--v-gone)` in prose must not
    // be reported as using it, and a commented-out declaration is not a
    // definition. (This file's own header names two tokens for exactly this
    // reason, and it lives under src/.)
    const sample = ':root{ --v-a:#fff; /* --v-b:#000; and var(--v-c) */ }';
    expect([...defined(sample)]).toEqual(['--v-a']);
    expect([...strip(sample).matchAll(USE_RE)]).toEqual([]);
  });

  // ── The verdict.
  it('resolves every reference', () => {
    const d = defined();
    const missing = [...referenced()]
      .filter(([t]) => !d.has(t))
      .map(([t, where]) => `${t} — used by ${[...new Set(where)].join(', ')}`);
    expect(missing).toEqual([]);
  });
});
