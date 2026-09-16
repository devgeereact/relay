import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { BUILTIN_THEMES, THEME_STYLE_KEYS } from './themes.js';

// A MIGRATION INLINES THE VALUES AS THEY WERE.
//
// The themes are being deleted, and the one thing that must outlive them is
// what every template pinning one actually looked like. Rust does the inlining
// (it runs before any window is shown), and the builtin styles live in JS — so
// the values are frozen into a file both sides read, exactly as
// `shelf_templates.json` already is.
//
// This test exists for the window in which BOTH still exist. Once `themes.js`
// no longer exports `BUILTIN_THEMES` (Task 13), delete this file: the snapshot
// is then the only copy, which is the point of a snapshot.
describe('the frozen theme snapshot', () => {
  it('matches the table it was taken from, key for key', () => {
    const frozen = JSON.parse(readFileSync('src-tauri/data/legacy_themes.json', 'utf8')).themes;
    expect(frozen.length).toBe(BUILTIN_THEMES.length);
    for (const t of BUILTIN_THEMES) {
      const f = frozen.find((x) => x.id === t.id);
      expect(f, `theme ${t.id} (${t.name}) is missing from the snapshot`).toBeTruthy();
      expect(f.style).toEqual(t.style);
    }
  });

  it('carries the whitelist applyTheme filtered a theme through', () => {
    // The migration inlines a theme's style into the template that pinned it,
    // and it must inline exactly what reached the screen. `applyTheme` copied
    // only these keys, so a theme key outside the list was dropped on the way to
    // the wall. Without the list frozen beside the values, Rust would inline a
    // key no congregation ever saw, and the one promise of the migration (the
    // look does not change) would be broken by the migration itself.
    const frozen = JSON.parse(readFileSync('src-tauri/data/legacy_themes.json', 'utf8'));
    expect(frozen.style_keys).toEqual(THEME_STYLE_KEYS);
  });
});
