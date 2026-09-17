// RG-146 · THE ROW THAT PAINTED OVER THE THING UNDER IT.
//
// Live's `.con-top` is a grid row with a DEFINITE height — `clamp(268px, 33vh,
// 364px)` — and one of its three children, `.rack`, is `align-self: start`, so
// it sizes to its own content and ignores the row entirely. The rack's content
// is about 320 px whatever the viewport is, because its column is a fixed 118 px
// and its bands are a 64 px TAKE, two 26 px arrows, a two-line caption and two
// 22 px pickers. So on any window shorter than roughly 970 px the row is shorter
// than the rack, the row does not clip, and the rack's last band — the
// transition pickers — paints OUTSIDE the row, on top of whatever comes next.
//
// For most of this project's life nothing came next but a slide grid, and the
// overlap was cosmetic. Wave 3 put the programme timer band there, and a
// cosmetic overflow became a button that does something other than what it says:
//
//   at 1320 x 860 — the size `tauri.conf.json` opens the window at —
//   `document.elementFromPoint` over the centre of **Start timer** returned
//   `select.r-select.xpick.xdur`, the transport's transition-duration picker.
//
// ── WHAT THIS FILE CAN AND CANNOT PROVE ────────────────────────────────────
// It CANNOT run that measurement. jsdom computes no layout, `elementFromPoint`
// there has no boxes to answer from, and this repository has no browser driver
// and no Playwright (`docs/qa/QA_HARNESS.md` Part 1 says so in its own words).
// Every hit test quoted in this file and in RG-146 was taken in a real browser,
// by hand, against a Vite server with a mocked bridge — a LAYOUT claim, never a
// behaviour claim — and those numbers are the evidence. This file is the
// TRIPWIRE for the mechanism behind them, and it is held to two things:
//
//   1. the stylesheet contract that makes the overlap impossible — the row may
//      not declare a height its own content can exceed, and it may not buy that
//      by clipping, because clipping the rack HIDES the transition pickers and a
//      control an operator cannot reach is worse than one that overlaps;
//   2. the structure, read off the MOUNTED component rather than off the source,
//      so the two elements this is about are genuinely the ones in the collision:
//      the pickers are inside `.con-top`, and Start timer is in the band
//      immediately after it.
//
// It would have failed on the night this landed, and it fails again the moment
// somebody restores `height:` on that row. It is not proof that the button is
// clickable. The browser pass is.
//
//   npx vitest run src/lib/liverackfit.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const Live = (await import('./views/Live.svelte')).default;

const SRC = readFileSync(resolve(__dirname, './views/Live.svelte'), 'utf8');

/**
 * Live's `<style>` with every `@media` block removed, so a rule read out of it
 * is the one that applies at a plain desktop width. The narrow steps already do
 * the right thing — `@media (max-width:1180px)` has said `height:auto` for as
 * long as it has existed, which is the precedent this fix follows — and reading
 * them as though they were the default is how a scanner passes over the bug.
 */
function defaultStyle() {
  const style = SRC.slice(SRC.lastIndexOf('<style>') + 7, SRC.lastIndexOf('</style>'));
  let out = '';
  for (let i = 0; i < style.length; i++) {
    if (style.startsWith('@media', i)) {
      const open = style.indexOf('{', i);
      let depth = 0;
      let j = open;
      for (; j < style.length; j++) {
        if (style[j] === '{') depth++;
        else if (style[j] === '}' && --depth === 0) break;
      }
      i = j;
      continue;
    }
    out += style[i];
  }
  return out;
}

/** The declarations of one selector's rule, lower-cased and whitespace-free. */
function rule(css, selector) {
  const at = css.indexOf(selector + '{');
  expect(at, `no default rule for ${selector} — the scanner has gone blind`).toBeGreaterThan(-1);
  const body = css.slice(at + selector.length + 1, css.indexOf('}', at));
  return body.replace(/\s+/g, '').toLowerCase();
}

describe('RG-146 · the run surface may not paint a control outside its own row', () => {
  const css = defaultStyle();

  it('sizes the monitors row with a FLOOR, never with a height its content can exceed', () => {
    // THE DEFECT ITSELF. `height:clamp(268px,33vh,364px)` is a definite height:
    // the row is that tall whatever is in it, and a child that is taller simply
    // paints past the bottom edge. `min-height` says the same thing about the
    // small case — the row never collapses below the clamp — and lets the rack
    // push the row down instead of painting over the band under it.
    const r = rule(css, '.con-top');
    expect(r, '.con-top must not declare a definite height').not.toMatch(/(^|;)height:/);
    expect(r, '.con-top must keep its floor as a min-height').toMatch(
      /(^|;)min-height:clamp\(268px,33vh,364px\)/,
    );
  });

  it('does not buy that by clipping the rack, which would hide the pickers', () => {
    // The other way to stop an overflow painting over its neighbour is to cut it
    // off. That is the wrong fix here and the register row says so: the thing in
    // the overflow is the transition pair, and clipping it makes a control
    // unreachable rather than merely misplaced. Requirement 2 outranks
    // requirement 1 — nothing may be pushed off-screen to fix an overlap.
    const r = rule(css, '.con-top');
    expect(r, '.con-top must not clip its own children').not.toMatch(
      /(^|;)overflow(-y)?:(hidden|clip|auto|scroll)/,
    );
  });

  it('keeps the rack sizing to its own content, which is what makes the floor work', () => {
    // `align-self:start` is deliberate and documented at the call site: the rack
    // is a short strip of buttons and stretching it to a tall row leaves a dead
    // gap under the pickers. Keeping it means the row's own content height IS
    // the rack's height, which is exactly what `min-height` now defers to. A
    // `height` here would put the bug back one level down.
    const r = rule(css, '.rack');
    expect(r).toMatch(/(^|;)align-self:start/);
    expect(r, '.rack must not be given a height of its own').not.toMatch(/(^|;)height:/);
  });

  describe('and the two elements this is about are the ones in the collision', () => {
    let host;

    beforeEach(() => {
      invoke.mockReset();
      invoke.mockImplementation(async (cmd) => {
        if (cmd === 'list_timers') return [];
        if (cmd === 'ping') return true;
        return [];
      });
      host = document.createElement('div');
      document.body.appendChild(host);
    });

    afterEach(() => {
      host.remove();
      vi.restoreAllMocks();
    });

    it('has the transition pickers INSIDE the row, and Start timer in the band after it', async () => {
      // Read off the mounted tree rather than off the source. The stylesheet
      // contract above is about `.con-top`; this is what says `.con-top` is the
      // row that holds the overflowing control, and that the programme timer
      // band really is the thing directly under it. Without this the rules
      // above could go on passing while the markup moved beneath them.
      const app = new Live({ target: host, props: {} });
      await new Promise((r) => setTimeout(r, 120));

      const row = host.querySelector('.con-top');
      expect(row).toBeTruthy();

      const pickers = [...host.querySelectorAll('.rack .rk-x .xpick')];
      expect(pickers).toHaveLength(2);
      for (const p of pickers) expect(row.contains(p)).toBe(true);

      const band = host.querySelector('.pt-band');
      expect(band, 'the programme timer band').toBeTruthy();
      expect(row.nextElementSibling, 'the band sits immediately after the row').toBe(band);

      const start = [...band.querySelectorAll('button')].find(
        (b) => b.textContent.trim() === 'Start timer',
      );
      expect(start, 'Start timer lives in that band').toBeTruthy();

      app.$destroy();
    });
  });
});
