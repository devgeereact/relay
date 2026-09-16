// AN OLD TEMPLATE MUST RENDER IDENTICALLY AFTER MIGRATION.
//
// This is the criterion that protects every church that already has templates
// saved. `migrateStyle` moves the whole-template keys onto the elements and then
// DELETES them (docs/REBRAND.md §3.1) — which is right, because a property with
// two homes is a property the editor can show while writing elsewhere. But a
// deletion is only safe if every reader was moved with it, and two were not:
//
//   1. `layers.js::regionsToLayers` read `style.font`. Every template Relay
//      seeds carries one, so after migration the conversion found nothing and
//      substituted the serif default — `var(--f-display)` and `var(--f-body)`
//      templates came back in the wrong typeface. And this is not a preview:
//      `TemplateGallery.upgradeLegacyToLayers` runs on mount and SAVES, so the
//      first visit to the Templates tab after an upgrade re-typefaced the shelf,
//      silently, once, for good.
//
//   2. `themes.js`'s `theme:font` resolver read `style.font`. Every text layer in
//      the stage, confidence and countdown starters binds to it, so on a migrated
//      template with no theme behind it they all fell back to serif.
//
// Neither was visible from reading either file: both read a key that is real in
// the shape they were written against and absent in the shape that now reaches
// them. What catches it is comparing the two renders.
import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { migrateTemplate, migrateStyle, LEGACY_STYLE_KEYS } from './templatemodel.js';
import { regionsToLayers } from './layers.js';
import { applyTheme } from './themes.js';
import { BUILTINS } from './templates.js';

let host;
let app;
function render(template, content) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = undefined;
  host = undefined;
});

const CONTENT = { reference: 'Romans 8:28', text: 'And we know that all things work together for good.' };

/**
 * THE REAL SHELF, not an invented one. `db/templates.rs` seeds every template a
 * church starts with, as JSON string literals; those are the exact styles that
 * reach `migrateStyle` on the first load after an upgrade. Reading them here
 * rather than retyping them is what keeps this test about Relay's templates and
 * not about a fixture that agrees with the code.
 */
function seedBlobs() {
  const file = readFileSync(resolve(__dirname, '../../src-tauri/src/db/templates.rs'), 'utf8');
  // A SEED IS PRODUCTION CODE, so stop at the first test module. The literals
  // below that line are fixtures other tests argue from, and one of them is a
  // deliberately half-built style ({themeRef, verseColor} and nothing else) that
  // this file would otherwise read as a shipped design and then fail for not
  // carrying the legacy keys no shipped design is allowed to be missing. The cut
  // is asserted below so the scanner cannot silently narrow to nothing.
  const cut = file.search(/\n#\[cfg\(test\)\]\nmod /);
  const src = cut === -1 ? file : file.slice(0, cut);
  const out = [];
  for (const m of src.matchAll(/r##"(\{.*?\})"##/gs)) {
    try {
      const o = JSON.parse(m[1]);
      if (o && typeof o === 'object' && !Array.isArray(o)) out.push(o);
    } catch {
      /* a literal that is not JSON is not this test's business */
    }
  }
  return out;
}
// The seed writes a layout blob and a style blob per template. A style is the
// one that says what the words look like.
const seededStyles = () => seedBlobs().filter((o) => 'verseColor' in o);
const seededLayouts = () => seedBlobs().filter((o) => Array.isArray(o.regions));

// A scanner that quietly narrows passes everything (CLAUDE.md, `ipc.test.js`).
// These two hold this file's own instruments to what they claim to read.
describe('the fixtures this file argues from are real', () => {
  it('finds the seeded styles in the Rust seed, and they carry the legacy keys', () => {
    const styles = seededStyles();
    expect(styles.length).toBeGreaterThan(20);
    // The test-module cut found a real boundary. Without this, a rename of the
    // first `mod …_tests` would silently widen the scan back over the fixtures,
    // or a stray match would narrow it, and either way this file would be
    // arguing from something other than the seed it names.
    const raw = readFileSync(resolve(__dirname, '../../src-tauri/src/db/templates.rs'), 'utf8');
    expect(raw.search(/\n#\[cfg\(test\)\]\nmod /)).toBeGreaterThan(0);
    // Layouts are in there too, and finding none of them would mean the scanner
    // had narrowed to something that happens to agree with it.
    expect(seededLayouts().length).toBeGreaterThan(5);
    const withLegacy = styles.filter((s) => LEGACY_STYLE_KEYS.some((k) => k in s));
    expect(withLegacy.length).toBe(styles.length);
    // The defect only bites a template whose font is not the default, so the
    // shelf must actually contain some. It contains three faces.
    const faces = new Set(styles.map((s) => s.font));
    expect(faces.size).toBeGreaterThan(1);
    expect(faces.has('var(--f-serif)')).toBe(true);
  });

  it('the built-ins the kiosk falls back to carry them too', () => {
    expect(BUILTINS.every((t) => 'font' in t.style)).toBe(true);
  });
});

describe('§3.1 · an old template renders identically after migration', () => {
  const shapes = () => [
    ...BUILTINS.map((t) => ({ name: t.name, template: t })),
    ...seededStyles().map((style, i) => ({
      name: `seed ${i}`,
      template: {
        id: 900 + i,
        name: `seed ${i}`,
        layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false, refFirst: false },
        style,
      },
    })),
  ];

  // What a reader can actually see. Font, shadow and transform are the ones the
  // legacy keys carried; the rest are here so a future move of a different key
  // is caught by the same test rather than needing a new one.
  const look = (el) => {
    const read = (sel) => {
      const n = el.querySelector(sel);
      if (!n) return null;
      const cs = getComputedStyle(n);
      return [cs.fontFamily, cs.textShadow, cs.color, cs.textTransform, cs.lineHeight, cs.letterSpacing, cs.fontStyle].join(' | ');
    };
    // A ticker template (`scroll: true`) renders its words as a footer crawl
    // rather than a centred verse, so the words live under a different class.
    // Both carry `verseStyle`, which is what this test is reading.
    return {
      verse: read('.verse, .ticker-run, .ltext .lfit'),
      ref: read('.reference, .ticker-label'),
    };
  };

  it('through the region renderer, for every template on the shelf', () => {
    for (const { name, template } of shapes()) {
      const before = look(render(template, CONTENT));
      app.$destroy();
      host.remove();
      const after = look(render(migrateTemplate(template), CONTENT));
      expect(after, name).toEqual(before);
      expect(before.verse, name).not.toBeNull();
    }
  });

  it('through the layer conversion the gallery performs on mount, for every template on the shelf', () => {
    // The one that was broken. `upgradeLegacyToLayers` converts and SAVES, and by
    // the time it runs `capture.js` has already migrated what it is converting.
    for (const { name, template } of shapes()) {
      const fromLegacy = regionsToLayers(template);
      const fromMigrated = regionsToLayers(migrateTemplate(template));
      const fonts = (r) => r.layers.filter((L) => L.type === 'text').map((L) => `${L.name}:${L.font}:${L.shadow}`);
      expect(fonts(fromMigrated), name).toEqual(fonts(fromLegacy));
      // …and the typeface is the template's own, not the renderer's default.
      const face = migrateStyle(template.style).verseFont;
      expect(fonts(fromMigrated).every((f) => f.includes(face)), `${name} wears ${face}`).toBe(true);
    }
  });

  it('and the converted stack renders the same words in the same face', () => {
    const mono = BUILTINS.find((t) => t.style.font === 'var(--f-display)');
    expect(mono, 'a non-serif built-in to argue from').toBeTruthy();

    const regionLook = look(render(mono, CONTENT));
    app.$destroy();
    host.remove();

    const converted = { ...migrateTemplate(mono), layout: regionsToLayers(migrateTemplate(mono)) };
    const layerEl = render(converted, CONTENT);
    const fitted = [...layerEl.querySelectorAll('.ltext .lfit')];
    expect(fitted.length).toBeGreaterThan(0);
    for (const n of fitted) expect(getComputedStyle(n).fontFamily).toBe('var(--f-display)');
    expect(regionLook.verse).toContain('var(--f-display)');
  });
});

describe('§3.1 · a theme token answers for the style it is given, old shape or new', () => {
  const stage = (style) => ({
    id: 7,
    name: 'Stage',
    layout: {
      layers: [
        { id: 'v', type: 'text', bind: 'verse', name: 'Verse', font: 'theme:font', color: '#fff', x: 0, y: 0, w: 100, h: 50, size: 4 },
      ],
    },
    style,
  });

  it('resolves `theme:font` to the template typeface after migration, not to the default', () => {
    const legacy = stage({ font: 'var(--f-display)' });
    const before = applyTheme(legacy, null).layout.layers[0].font;
    const after = applyTheme(migrateTemplate(legacy), null).layout.layers[0].font;
    expect(before).toBe('var(--f-display)');
    expect(after).toBe('var(--f-display)');
  });

  it('still falls back to the renderer default when no typeface was ever chosen', () => {
    expect(applyTheme(stage({}), null).layout.layers[0].font).toBe('var(--f-serif)');
  });

  it('a theme behind the template still supplies the face the template leaves unset', () => {
    const themed = applyTheme(stage({}), { style: { font: 'var(--f-body)' } });
    expect(themed.layout.layers[0].font).toBe('var(--f-body)');
  });
});
