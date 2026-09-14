// §3.1 · ONE PROPERTY, ONE HOME — at the DOORS, and through the renderer.
//
// `templatemodel.test.js` proves the migration is correct as a function: the
// legacy whole-template keys are written onto their elements and deleted,
// idempotently, without overwriting a choice. That is the harder half and it is
// done.
//
// This file holds the half a pure test cannot reach, and the half that has
// actually gone wrong in this repository before:
//
//   1. **Every door migrates.** A migration that runs at the renderer keeps the
//      WALL correct while the legacy key sits in the database for ever, waiting
//      for the next reader that does not resolve. `regionsToLayers` is the
//      recorded near-miss: it read `style.font` directly, so once migration ran
//      on the way out of the database it found nothing there and silently
//      substituted the serif default — and `upgradeLegacyToLayers` SAVES the
//      result, so one visit to the Templates tab would have re-typefaced a
//      church's shelf, once, for good.
//
//   2. **An old template renders identically after it.** The whole point of a
//      migration that DELETES is that nothing an operator can see changes. A
//      migration that quietly repaints is worse than the two homes it replaced,
//      because the two homes at least disagreed visibly.
import { describe, it, expect, vi } from 'vitest';
import { migrateStyle, migrateTemplate, resolveStyle, LEGACY_STYLE_KEYS } from './templatemodel.js';
import { parseImportedTemplate, TEMPLATE_FILE_MARKER } from './templates.js';
import { regionsToLayers } from './layers.js';

const read = (f) => require('node:fs').readFileSync(f, 'utf8');

/** A template as it existed BEFORE the model: whole-template keys, no per-element
 *  ones. This is what a shelf saved a year ago still holds. */
const legacy = () => ({
  id: 9,
  name: 'Old Shelf',
  layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false },
  style: {
    font: 'var(--f-display)',
    textShadow: 0.7,
    background: '#101018',
    accent: '#e8a33d',
    verseColor: '#f4e4c8',
    verseSize: '5.5',
    refSize: '2.6',
  },
});

describe('§3.1 · the legacy keys do not survive any door', () => {
  it('THE DATABASE DOOR — loadTemplates migrates every row on the way in', async () => {
    // The one that matters most: everything downstream (the gallery, the editor,
    // a save) reads the store, so a row that arrives unmigrated is a legacy key
    // that gets written back.
    const invoke = vi.fn(async (cmd) => (cmd === 'list_templates' ? [legacy()] : null));
    vi.doMock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
    vi.doMock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));
    vi.resetModules();
    const { loadTemplates } = await import('./stores/capture.js');

    const got = await loadTemplates();
    for (const k of LEGACY_STYLE_KEYS) expect(got[0].style, k).not.toHaveProperty(k);
    expect(got[0].style.verseFont, 'the fact survives, on its element').toBe('var(--f-display)');
    expect(got[0].style.refFont).toBe('var(--f-display)');
    vi.doUnmock('@tauri-apps/api/core');
    vi.doUnmock('@tauri-apps/api/event');
    vi.resetModules();
  });

  it('THE IMPORT DOOR — a template file is migrated, after it is sanitised', () => {
    // Order matters and is documented at the call site: the migration COPIES
    // values onto new keys, so running it before the sanitiser would carry a
    // hostile value past the check that exists to strip it, under a name the
    // check had already cleared.
    const file = parseImportedTemplate(
      JSON.stringify({ marker: TEMPLATE_FILE_MARKER, name: 'From disk', ...legacy() }),
    );
    for (const k of LEGACY_STYLE_KEYS) expect(file.style, k).not.toHaveProperty(k);
    expect(file.style.verseFont).toBe('var(--f-display)');
  });

  it('THE RENDER DOOR — resolveStyle migrates whatever it is handed', () => {
    // The renderer must not TRUST the door in front of it. A template reaching it
    // from a kiosk page, a retained frame or a broadcast payload has been through
    // a different path, and only this one is on every one of them.
    const s = resolveStyle(legacy().style);
    for (const k of LEGACY_STYLE_KEYS) expect(s, k).not.toHaveProperty(k);
    expect(s.verseFont).toBe('var(--f-display)');
  });

  it('THE CONVERSION DOOR — regionsToLayers reads the model, not the raw style', () => {
    // The recorded near-miss. This must give the same typeface whether it is
    // handed a raw template off a disk or a migrated one out of the store —
    // because `upgradeLegacyToLayers` SAVES what it returns.
    const raw = regionsToLayers(legacy());
    const already = regionsToLayers(migrateTemplate(legacy()));
    const faceOfVerse = (l) => l.layers.find((L) => L.name === 'Verse').font;
    expect(faceOfVerse(raw), 'a raw legacy template keeps its face').toBe('var(--f-display)');
    expect(faceOfVerse(already), 'and so does an already-migrated one').toBe('var(--f-display)');
    expect(faceOfVerse(raw)).toBe(faceOfVerse(already));
  });

  it('no door reads a legacy key directly any more', () => {
    // A grep, deliberately: the four doors above are the ones that exist today,
    // and a FIFTH reader added next year is exactly how this class of defect
    // came back the first time. Anything that wants `style.font` must go
    // through the model, which is why the model deletes rather than shadows.
    const offenders = [];
    for (const f of [
      'src/lib/layers.js',
      'src/lib/themes.js',
      'src/lib/templates.js',
      'src/lib/TemplateRender.svelte',
    ]) {
      const src = read(f);
      for (const line of src.split('\n')) {
        // A comment may NAME the key — that is how the reason is recorded beside
        // the code. Line comments, both block-comment forms, and HTML comments.
        if (/^\s*(\/\/|\/\*|\*|<!--)/.test(line)) continue;
        for (const k of LEGACY_STYLE_KEYS) {
          if (new RegExp(`(style|s)\\??\\.${k}\\b`).test(line)) offenders.push(`${f}: ${line.trim()}`);
        }
      }
    }
    expect(offenders).toEqual([]);
  });
});

describe('§3.1 · an old template renders identically after the migration', () => {
  // THE POINT OF A MIGRATION THAT DELETES. If migrating changes what an operator
  // sees, the deletion has lost a fact rather than moved it — and it would do so
  // on a shelf nobody looked at, between one release and the next.
  const interesting = ['verseFont', 'refFont', 'verseShadow', 'refShadow', 'verseSize', 'refSize', 'verseColor'];

  it('every property the renderer reads is the same before and after', () => {
    const before = resolveStyle(legacy().style);
    const after = resolveStyle(migrateTemplate(legacy()).style);
    for (const k of interesting) expect(after[k], k).toEqual(before[k]);
    expect(after).toEqual(before);
  });

  it('and migrating a second time changes nothing again', () => {
    const once = migrateStyle(legacy().style);
    expect(migrateStyle(once)).toEqual(once);
    expect(resolveStyle(migrateStyle(once))).toEqual(resolveStyle(legacy().style));
  });

  it('a template that never had a legacy key is untouched', () => {
    // The whole mechanism must be invisible to a shelf that opted in later.
    const modern = { verseFont: 'var(--f-serif)', refFont: 'var(--f-body)', verseShadow: 0.2 };
    expect(migrateStyle(modern)).toEqual(modern);
  });
});
