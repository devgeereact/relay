// 2026-09-21 · PL-1 (RG-198). The arrangement picker sheet mounted `role="dialog"`
// under `use:trapFocus` with a bare `on:keydown|stopPropagation`, so Escape
// originated inside it and was stopped there: the window handler that closes
// the sheet never ran, and `shortcuts.js` had already stood down for the dialog.
// The grip and the row ✕ did the same for every key, so Esc and B were dead on
// 120 of ~180 tab stops in a 60-cue plan. Rule 44: an overlay that disarms
// Escape must consume it; a control may swallow only the keys it uses.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const src = readFileSync(resolve(process.cwd(), 'src/lib/views/ServicePlanner.svelte'), 'utf8');

describe('the Planner swallows only the keys it uses', () => {
  it('has no bare on:keydown|stopPropagation anywhere', () => {
    expect(src).not.toMatch(/on:keydown\|stopPropagation(?=[\s>])/);
  });
  it('the arrangement sheet closes itself on Escape', () => {
    const sheet = src.slice(src.indexOf('class="sp-arrsheet"'), src.indexOf('</div>', src.indexOf('class="sp-arrsheet"')));
    expect(sheet).toMatch(/on:keydown=\{[^}]*Escape[^}]*\}/);
  });
});
