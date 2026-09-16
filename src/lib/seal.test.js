// WAVE 5, TRACK E — THE SEAL: the operator console's own skin stops at the glass.
//
// Relay's console and its congregation screens are built from the same source
// tree, and three separate paths let the console's own chrome cross onto a wall:
//
//   1. TEMPLATE DATA naming an app-chrome token. A seeded `style_json` storing
//      `"font":"var(--f-serif)"` is a template that renders in whatever the
//      console's stylesheet happens to alias that name to THIS WEEK. `--f-display`
//      has already been re-aliased once, from Space Grotesk to Inter, which
//      silently changed the typeface of every template naming it. A template names
//      a real family; the console names tokens.
//   2. THE RENDERER falling back to a console token. `TemplateRender` defaulted an
//      absent `style.accent` to `var(--v-amber)` — the operator console's ON AIR
//      colour, on a congregation screen, which is rule 18 in the one place it is
//      least visible. A renderer default is the TEMPLATE's default, not the app's.
//   3. THE CONSOLE STYLESHEET ITSELF. `output.js` and `stage.js` imported
//      `app.css`, and Svelte does not scope a global stylesheet, so every
//      unscoped console rule in it was live on `output.html` and `stage.html`.
//      `src/tokens.css` now carries the shared palette and nothing else, and it is
//      what the two congregation-facing entry points import.
//
// What is NOT sealed, deliberately: the palette. Output and stage still import the
// token block and the self-hosted fonts, so a screen stays on the same colours as
// the console that drives it. The seal is against RULES and against TEMPLATE DATA.
//
// Every assertion here was verified by reintroducing the defect it describes.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(__dirname, '../..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

/**
 * Blank out `/* … *\/` and `<!-- … -->` regions, keeping every other character.
 *
 * Two of the assertions below have to read CODE and not PROSE, because the prose
 * in this repository explains at length what it is forbidding — this file's own
 * header names all six console rules, and `TemplateRender` names both tokens it
 * stopped reaching for. A scanner that read those would go red on the comment
 * that documents the fix. Offsets are preserved so a line number still means
 * something, and the stripper has a self-check of its own below, because a
 * scanner that quietly widens is the same defect as one that quietly narrows.
 */
function stripComments(src) {
  const out = src.split('');
  let i = 0;
  while (i < src.length) {
    const block = src.startsWith('/*', i) ? '*/' : src.startsWith('<!--', i) ? '-->' : null;
    if (!block) {
      i += 1;
      continue;
    }
    const end = src.indexOf(block, i + 2);
    const stop = end === -1 ? src.length : end + block.length;
    for (let j = i; j < stop; j += 1) if (out[j] !== '\n') out[j] = ' ';
    i = stop;
  }
  // …and a WHOLE-LINE `//` comment, which is how the Svelte components explain
  // themselves. Only a whole line: a `//` after code is left alone, because
  // stripping to the end of a line would also eat the `//` in a URL, and an
  // over-eager stripper hides the thing the scanner is for. A trailing comment
  // naming a token therefore goes RED, which is the safe direction to be wrong in.
  return out
    .join('')
    .split('\n')
    .map((line) => (line.trimStart().startsWith('//') ? '' : line))
    .join('\n');
}

/**
 * THE FILES THAT AUTHOR TEMPLATE DATA — the bytes that end up in a
 * `templates` row, or in the starter a new layer is built from.
 *
 * Not the files that READ it: `templatemodel.js::faceOf` classifies a legacy
 * `var(--f-mono)` as a mono face and must keep doing so, because rows written
 * years ago still hold one and the renderer still passes them through. The rule
 * is about what Relay WRITES, which is why the scanner below matches a token
 * sitting in the value position of a style key rather than the token anywhere.
 */
const TEMPLATE_DATA = [
  'src-tauri/src/db/templates.rs',
  'src/lib/templates.js',
  'src/lib/layers.js',
  'src/lib/templatemodel.js',
];

/** The style and layer keys whose value reaches a congregation screen. */
const STYLE_KEYS =
  'font|verseFont|refFont|accent|color|refColor|verseColor|fill|background|plate|outlineColor';

/**
 * Every app-chrome token sitting in the VALUE position of a style key.
 *
 * Matches both spellings the two sides of the bridge use: `"font":"var(--f-serif)"`
 * in the Rust seed's JSON string literals, and `font: 'var(--f-display)'` in the
 * frontend's object literals.
 */
function chromeTokensIn(src) {
  const re = new RegExp(
    String.raw`["']?\b(${STYLE_KEYS})["']?\s*[:=]\s*["']\s*(var\(--[a-z0-9-]+\))`,
    'g',
  );
  const hits = [];
  for (const m of src.matchAll(re)) hits.push(`${m[1]} = ${m[2]}`);
  return hits;
}

describe('the seal — app chrome does not cross onto a congregation screen', () => {
  // ── Leak 1 · template data ────────────────────────────────────────────────
  for (const path of TEMPLATE_DATA) {
    it(`${path} names real font families, not app-chrome tokens`, () => {
      expect(chromeTokensIn(read(path))).toEqual([]);
    });
  }

  it('the scanner can still see a token it is meant to catch', () => {
    // A scanner that quietly narrows passes everything, and this repository has
    // had that exact failure twice (`ipc.test.js`, both times). Plant one of each
    // spelling and require the scanner to find both.
    const planted = [
      String.raw`r##"{"font":"var(--f-serif)","accent":"#e8a33d"}"##`,
      `  font: 'var(--f-display)',`,
      `      color: 'var(--v-amber)',`,
    ].join('\n');
    expect(chromeTokensIn(planted)).toEqual([
      'font = var(--f-serif)',
      'font = var(--f-display)',
      'color = var(--v-amber)',
    ]);
  });

  it('the comment stripper takes prose and leaves code', () => {
    const src = [
      '/* .toggle.on and var(--v-amber) are named here, in prose. */',
      '  // and var(--f-serif) is named here, in a line comment.',
      '.toggle.on{ color:var(--v-amber); }',
      "const u = 'https://example.invalid/var(--f-body)';",
    ].join('\n');
    const stripped = stripComments(src).split('\n');
    expect(stripped[0].trim(), 'block comment').toBe('');
    expect(stripped[1].trim(), 'line comment').toBe('');
    expect(stripped[2], 'a real rule').toBe('.toggle.on{ color:var(--v-amber); }');
    expect(stripped[3], 'a URL is not a comment').toContain('https://');
  });

  // ── Leak 2 · the renderer ─────────────────────────────────────────────────
  it('TemplateRender names no console token of its own', () => {
    // `--accent` and `--tickdur` are the renderer's OWN custom properties, set on
    // the stage element from template data. `--f-*` and `--v-*` are the console's,
    // and a fallback to one is a template-side default that the app can move.
    const src = stripComments(read('src/lib/TemplateRender.svelte'));
    const chrome = [...src.matchAll(/var\(--(?:f|v)-[a-z0-9-]+\)/g)].map((m) => m[0]);
    expect(chrome).toEqual([]);
  });

  // ── Leak 3 · the stylesheet ───────────────────────────────────────────────
  it('the congregation-facing entry points do not import the console stylesheet', () => {
    for (const entry of ['src/output.js', 'src/stage.js']) {
      const src = read(entry);
      expect(src, entry).not.toMatch(/['"][.\/]*app\.css['"]/);
      expect(src, entry).toMatch(/['"]\.\/tokens\.css['"]/);
    }
    // …and the console still gets the whole sheet, so this is a seal and not a
    // deletion.
    expect(read('src/main.js')).toMatch(/['"]\.\/app\.css['"]/);
  });

  it('the shared sheet is tokens only — no rule in it can reach a wall', () => {
    // The durable half. Deleting six named rules seals today; a sheet that may
    // only declare custom properties seals every rule anybody adds next year.
    const css = stripComments(read('src/tokens.css'));
    const selectors = [...css.matchAll(/(^|\})\s*([^{}@]+)\{/g)].map((m) => m[2].trim());
    const offenders = selectors.filter((s) => /[.#]|:hover|::/.test(s));
    expect(offenders).toEqual([]);
  });

  it('the six legacy console rules no longer reach output.html or stage.html', () => {
    // The rules `src/app.css` recorded as un-deletable without "eyes on a running
    // app": they are console-only and they were live on both congregation pages.
    const shared = stripComments(read('src/tokens.css'));
    for (const rule of [
      '.prev-main .verse',
      '.prev-stage .verse',
      '.prev-stream .lower-third',
      '.prev-lobby .verse',
      '.tmpl-row.active',
      '.toggle.on',
    ]) {
      expect(shared, rule).not.toContain(rule);
    }
  });

  it('the palette is still shared — the seal is not a fork', () => {
    const shared = stripComments(read('src/tokens.css'));
    for (const token of ['--v-void', '--v-amber', '--f-serif', '--f-body', '--f-mono']) {
      expect(shared, token).toContain(`${token}:`);
    }
  });
});
