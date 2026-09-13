// THE WORKSPACE GRAMMAR — the instrument for docs/REBRAND.md §2 and §11.
//
// §2's note in the REBRAND status table says the quiet part out loud: twelve
// finished phases did not look like the prototype, because §2 — the workspace
// grammar — had no phase number and the implementation list never mentioned it.
// The shell got the chrome bar, the dock row and the status bar; the workspace
// BODIES stayed three unrelated designs.
//
// This file is what stops that happening again to the three desks. It does not
// test that a screen is pretty — nothing can. It tests the two things that made
// them diverge in the first place:
//
//   1. there is ONE definition of the layout and the type scale, and all three
//      workspaces go through it;
//   2. the decisions that were made once and then quietly re-litigated per file
//      — no pills, tokens not literals, steel blue for selection — hold.
//
// Written the way CLAUDE.md asks: each assertion fails if the defect it names is
// reintroduced. Checked by reverting each rule and watching it go red.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const read = (f) => readFileSync(resolve(__dirname, '../../..', f), 'utf8');

const FRAME = 'src/lib/views/WorkspaceFrame.svelte';
// The three workspaces this pass covers. Live is deliberately absent: it is the
// run surface and has its own studio split (§2), which the frame does not own.
// Library, Templates and Themes are other agents' files and other passes.
const DESKS = [
  'src/lib/views/ServicePlanner.svelte',
  'src/lib/views/Channels.svelte',
  'src/lib/views/Settings.svelte',
];

describe('§2 · one workspace grammar, not three', () => {
  it('every desk lays itself out in the shared frame', () => {
    for (const f of DESKS) {
      const src = read(f);
      expect(src, `${f} does not import the frame`).toMatch(
        /import WorkspaceFrame from '\.\/WorkspaceFrame\.svelte'/,
      );
      expect(src, `${f} imports the frame but does not render it`).toMatch(/<WorkspaceFrame\b/);
    }
  });

  it('and none of them declares its own three-column body', () => {
    // The defect this replaces: the Planner's `.sp-shell`, Outputs' `.ch-shell`
    // and Settings' `.s-layout` were three separate grid declarations with three
    // different gutters (16px, 16px, 20px) for the same shape. A fourth copy is
    // how they drift apart again.
    for (const f of DESKS) {
      const grids = [...read(f).matchAll(/grid-template-columns:\s*([^;}]+)/g)].map((m) =>
        m[1].trim(),
      );
      // A table's column track is a different thing and is allowed; a RAIL ·
      // MAIN · INSPECTOR track is not, and it is recognisable: it ends in a
      // fixed inspector width after a `minmax(0,1fr)` main column.
      const bodies = grids.filter((g) => /minmax\(0,\s*1fr\)\s+\d+px\s*$/.test(g));
      expect(bodies, `${f} declares its own workspace body`).toEqual([]);
    }
  });

  it('the frame is the one place the columns are described', () => {
    const frame = read(FRAME);
    expect(frame).toMatch(/grid-template-columns:var\(--rw-cols\)/);
    // Rail · main · inspector. Every caller passes three tracks, in that order.
    for (const f of DESKS) {
      const [, cols] = read(f).match(/columns="([^"]+)"/) ?? [];
      expect(cols, `${f} passes no column track to the frame`).toBeTruthy();
      expect(cols.trim().split(/\s+(?![^(]*\))/).length, `${f}: ${cols}`).toBe(3);
    }
  });
});

describe('§11 · one type scale, three roles', () => {
  const frame = read(FRAME);

  it('the frame defines page title, standfirst and row', () => {
    expect(frame, 'no page-title role').toMatch(/\.rw-h1\{[\s\S]*?font-size:var\(--v-fs-h1\)/);
    expect(frame, 'no standfirst role').toMatch(/\.rw-lead\{[\s\S]*?font-size:var\(--v-fs-b2\)/);
    // A row is a NAME and a VALUE — §11 is explicit, and the value half is what
    // `settingvalue.js` exists to fill honestly.
    expect(frame, 'no row name').toMatch(/\.rw-nvk\)?\{/);
    expect(frame, 'no row value').toMatch(/\.rw-nvv\)?\{/);
    // …and the footnote sits behind a hairline, so it reads as an aside rather
    // than as one more row of the list above it.
    expect(frame).toMatch(/\.rw-foot\)?\{[\s\S]*?border-top:1px solid var\(--v-line\)/);
  });

  it('the value half is mono, so a figure cannot reflow the name beside it', () => {
    // §1: "IBM Plex Mono (every figure, so a changing number never reflows the
    // row beside it)". A proportional value column shuffles the whole rail every
    // time an uptime or a client count ticks.
    const rule = frame.slice(frame.indexOf('.rw-nvv'));
    expect(rule.slice(0, 260)).toMatch(/font-family:var\(--f-mono\)/);
    expect(rule.slice(0, 260)).toMatch(/tabular-nums/);
  });

  it('no desk sets a font size in raw pixels', () => {
    // Settings alone carried eleven hand-picked sizes — 13.5px, 14px, 13px,
    // 12.5px, 12px, 11px, 10.5px, 10px, 9px, 18px, 30px — none from the scale.
    // Eleven steps chosen one control at a time is not a type scale.
    const offenders = [];
    // The WHOLE file, not just its <style> block: two of the sizes this found
    // were inline `style="font-size:12.5px"` on an empty-state, which is exactly
    // where a hand-picked size hides from a stylesheet-only scan.
    for (const f of [...DESKS, FRAME]) {
      for (const m of read(f).matchAll(/font-size:\s*(\d+(?:\.\d+)?)px/g)) {
        offenders.push(`${f}: ${m[0]}`);
      }
    }
    expect(offenders, 'use a --v-fs-* token — the scale is the point').toEqual([]);
  });
});

describe('§1 · the decisions that kept being re-litigated per file', () => {
  it('no pills — a pill in a control room reads as a toy', () => {
    const offenders = [];
    for (const f of [...DESKS, FRAME]) {
      for (const m of read(f).matchAll(/border-radius:\s*99px/g)) offenders.push(`${f}: ${m[0]}`);
    }
    // `--v-r-round` survives for the two shapes that are genuinely round — a
    // slider thumb and a switch — and a switch asks for the TOKEN, not the
    // literal, so this stays absolute.
    expect(offenders, 'radius is 3px; --v-r-round is for a thumb and a switch').toEqual([]);
  });

  it('selection is steel blue, and amber is still only ever ON AIR', () => {
    // A selected row is the thing you are working on. Amber means a congregation
    // is looking at something, and it is never allowed to mean anything else —
    // which is exactly the sort of rule a restyle erodes one file at a time.
    for (const f of DESKS) {
      const style = read(f).slice(read(f).lastIndexOf('<style>'));
      for (const m of style.matchAll(/\.[\w-]*\.(?:sel|on)\{([^}]*)\}/g)) {
        expect(m[1], `${f}: a selected row is painted amber`).not.toMatch(/--v-amber/);
      }
    }
  });

  it('a desk paints with tokens, never with a raw hex', () => {
    const offenders = [];
    for (const f of [...DESKS, FRAME]) {
      const style = read(f).slice(read(f).lastIndexOf('<style>'));
      for (const m of style.matchAll(/#[0-9a-fA-F]{3,8}\b/g)) offenders.push(`${f}: ${m[0]}`);
    }
    // `--v-rose,#e0526a` is a var() FALLBACK on the arrangement picker and #fff
    // is ink on a filled destructive button; both are pre-existing and named.
    expect(offenders.filter((o) => !/#e0526a|#fff\b/.test(o))).toEqual([]);
  });
});
