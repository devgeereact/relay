import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { BUILTIN_THEMES } from './themes.js';

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
});
