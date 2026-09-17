// Phase 2 — the migration is on the DOORS, not on the renderer alone.
//
// `TemplateRender` resolves the model before it draws, so nothing is ever drawn
// from a two-home style. That is enough to make the WALL correct and not enough
// to make the STORED template correct: the editor loads a row from the database,
// shows it, and saves what it was given. Migrate only at the renderer and the
// legacy key lives in the database for ever, waiting for the next thing that
// reads a template without resolving it first.
//
// So the migration also runs where a template ENTERS the frontend: the list read
// from the database, and a file somebody imported. Both are doors; this is the
// test that they are.
//
// (The same reasoning as CLAUDE.md's "a guarantee is only kept on the doors you
// checked" — four bugs in this repository have the shape of a rule applied on
// one surface and skipped on its twin.)
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { parseImportedTemplate, serializeTemplate } from './templates.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { loadTemplates, templates } = await import('./stores/capture.js');

const read = (store) => {
  let v;
  store.subscribe((x) => (v = x))();
  return v;
};

describe('the database door — loadTemplates', () => {
  beforeEach(() => {
    invoke.mockReset();
    templates.set([]);
  });

  it('migrates every template on the way in', async () => {
    invoke.mockResolvedValue([
      { id: 1, name: 'Legacy', layout: {}, style: { font: 'Fraunces', textShadow: 0.4 } },
      { id: 2, name: 'Modern', layout: {}, style: { verseFont: 'Inter' } },
    ]);
    await loadTemplates();
    const list = read(templates);
    expect(list[0].style.font).toBeUndefined();
    expect(list[0].style.verseFont).toBe('Fraunces');
    expect(list[0].style.refShadow).toBe(0.4);
    expect(list[1].style.verseFont).toBe('Inter');
  });

  it('still survives a backend that hands back nothing useful', async () => {
    // GROUP 2: a failed read costs the operator nothing they can see, and the
    // migration must not turn an empty answer into a thrown one.
    invoke.mockRejectedValue(new Error('database is locked'));
    await expect(loadTemplates()).resolves.toEqual([]);
    expect(read(templates)).toEqual([]);
  });
});

describe('the file door — parseImportedTemplate', () => {
  const file = (style) =>
    JSON.stringify({ marker: 'relay.template/v1', name: 'Shared look', layout: { regions: ['verse'] }, style });

  it('migrates a template exported by an older Relay', () => {
    const t = parseImportedTemplate(file({ font: 'Fraunces', textShadow: 0.5 }));
    expect(t.style.font).toBeUndefined();
    expect(t.style.verseFont).toBe('Fraunces');
    expect(t.style.verseShadow).toBe(0.5);
  });

  it('round-trips: export, import, and the legacy key is gone for good', () => {
    const imported = parseImportedTemplate(file({ font: 'Fraunces' }));
    const again = parseImportedTemplate(serializeTemplate(imported));
    expect(again.style.font).toBeUndefined();
    expect(again.style.verseFont).toBe('Fraunces');
  });

  it('does not weaken the sanitiser it runs beside', () => {
    // Import is untrusted input, and the migration runs on the way out of it.
    // A migration that moved a hostile value into a new key would carry it past
    // the check that was meant to strip it.
    const hostile = parseImportedTemplate(
      file({ font: 'url("http://tracker.example/beacon.png")' }),
    );
    expect(JSON.stringify(hostile)).not.toContain('tracker.example');
  });
});

describe('the token door — a style a layer BINDS to reaches the wall', () => {
  it('resolves a bound layer against the template own style, not a default', async () => {
    // THE DOOR THAT REPLACED A WHITELIST. This used to assert that `applyTheme`
    // carried `bgStyle` through the THEME_STYLE_KEYS whitelist — a control
    // offering a choice whose key was off the list saved and then silently lost
    // it. Themes were folded into templates (DECISIONS §87), so there is no
    // whitelist and no second style to be filtered: a template's own `style` is
    // the whole answer, and `templatemodel.test.js` holds `slideBG` over the
    // background treatments directly.
    //
    // What still passes through a door is a LAYER bound to a token. A starter
    // dropped onto a template must wear THAT template's colours, so the wrong
    // answer here is the renderer's default silently standing in for a colour an
    // operator chose — which is the same failure in the same place.
    const { resolveTokens } = await import('./styletokens.js');
    const out = resolveTokens({
      style: { accent: '#123456', background: '#010203' },
      layout: { layers: [{ id: 'a', type: 'text', color: 'theme:accent', fill: 'theme:background' }] },
    });
    expect(out.layout.layers[0].color).toBe('#123456');
    expect(out.layout.layers[0].fill).toBe('#010203');
  });
});

// ── ADDED 2026-09-14 · the two doors this file did not have, and the claim the
//    deletion exists for ───────────────────────────────────────────────────────
//
// The two above are the doors a template ENTERS by. Two more read a stored style
// on the way to something else, and one of them is the recorded near-miss:
// `regionsToLayers` read `style.font` directly, so once migration ran on the way
// out of the database it found nothing there and silently substituted the serif
// default — and `TemplateGallery.upgradeLegacyToLayers` SAVES what it returns, so
// one visit to the Templates tab would have re-typefaced a church's shelf, once,
// for good.
//
// And the claim none of it was testing: **an old template renders identically
// after the migration**. That is the whole point of a migration that DELETES. One
// that quietly repaints is worse than the two homes it replaced, because the two
// homes at least disagreed visibly.
import { migrateStyle, migrateTemplate, resolveStyle, LEGACY_STYLE_KEYS } from './templatemodel.js';
import { regionsToLayers } from './layers.js';
import { readFileSync } from 'node:fs';

/** A template as it existed BEFORE the model: whole-template keys, no per-element
 *  ones. This is what a shelf saved a year ago still holds. */
const legacyTpl = () => ({
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

describe('the render door — resolveStyle', () => {
  it('migrates whatever it is handed, because it cannot trust the door in front of it', () => {
    // A template reaching the renderer from a kiosk page, a retained frame or a
    // broadcast payload has been through a different path. Only this one is on
    // every one of them.
    const s = resolveStyle(legacyTpl().style);
    for (const k of LEGACY_STYLE_KEYS) expect(s, k).not.toHaveProperty(k);
    expect(s.verseFont).toBe('var(--f-display)');
    expect(s.refFont).toBe('var(--f-display)');
  });
});

describe('the conversion door — regionsToLayers', () => {
  const verseFace = (l) => l.layers.find((L) => L.name === 'Verse').font;

  it('gives the same typeface for a raw legacy template and an already-migrated one', () => {
    // THE RECORDED NEAR-MISS, pinned. `upgradeLegacyToLayers` saves this result,
    // so a difference between these two is a permanent, silent re-typefacing.
    expect(verseFace(regionsToLayers(legacyTpl())), 'raw').toBe('var(--f-display)');
    expect(verseFace(regionsToLayers(migrateTemplate(legacyTpl()))), 'migrated').toBe('var(--f-display)');
  });
});

describe('no door reads a legacy key directly', () => {
  it('nothing outside the model reaches for style.font or style.textShadow', () => {
    // A grep, deliberately: the four doors above are the ones that exist today,
    // and a FIFTH reader added next year is exactly how this class of defect came
    // back the first time. Anything wanting `style.font` goes through the model,
    // which is why the model DELETES rather than shadows — there is nothing left
    // to read.
    const offenders = [];
    for (const f of [
      'src/lib/layers.js',
      'src/lib/styletokens.js',
      'src/lib/templates.js',
      'src/lib/TemplateRender.svelte',
    ]) {
      for (const line of readFileSync(f, 'utf8').split('\n')) {
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

describe('an old template renders identically after the migration', () => {
  it('every property the renderer reads is the same before and after', () => {
    const before = resolveStyle(legacyTpl().style);
    const after = resolveStyle(migrateTemplate(legacyTpl()).style);
    expect(after).toEqual(before);
  });

  it('and migrating a second time changes nothing again', () => {
    const once = migrateStyle(legacyTpl().style);
    expect(migrateStyle(once)).toEqual(once);
    expect(resolveStyle(migrateStyle(once))).toEqual(resolveStyle(legacyTpl().style));
  });

  it('a template that never had a legacy key is untouched', () => {
    // The whole mechanism must be invisible to a shelf that opted in later.
    const modern = { verseFont: 'var(--f-serif)', refFont: 'var(--f-body)', verseShadow: 0.2 };
    expect(migrateStyle(modern)).toEqual(modern);
  });
});
