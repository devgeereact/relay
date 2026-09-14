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

describe('the theme door — a background style reaches the wall', () => {
  it('survives applyTheme, which drops any key not on the whitelist', async () => {
    // A theme is a SPARSE set of defaults and `applyTheme` copies only the keys
    // in THEME_STYLE_KEYS. A control offering a choice whose key is not on that
    // list is a control that saves and then silently loses — the theme editor
    // would show "Centre glow" while the wall painted flat colour.
    const { applyTheme } = await import('./themes.js');
    const out = applyTheme(
      { layout: {}, style: {} },
      { style: { background: '#123456', bgStyle: 'glow' } },
    );
    expect(out.style.bgStyle).toBe('glow');

    const { slideBG } = await import('./templatemodel.js');
    expect(slideBG(out.style)).toContain('radial-gradient');
  });
});
