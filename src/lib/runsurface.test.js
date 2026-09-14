// L4 · THE RUN SURFACE — what the operator asked for, and what it may not cost.
//
// Five changes to the Sunday-morning console, each from the operator driving the
// rendered console beside the prototype:
//
//   1. the live and cued cells must be findable at a glance in a grid of twenty;
//   2. the take column must read as ONE block, not five stacked fragments;
//   3. the transition picker belongs beside the transport, not in the chrome;
//   4. the detection column may not carry a second copy of the dock's ARM switch;
//   5. the keys legend is always on the screen, and states only what is bound.
//
// `App.svelte` is not unit-testable (it mounts the console, opens the bridge and
// starts three timers), so the shell's half is asserted against its source, which
// is the convention `shellchrome.test.js` set and the reason it exists. Live IS
// mounted where mounting proves something the source cannot.
//
//   npx vitest run src/lib/runsurface.test.js
import { describe, it, expect, vi, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { installShortcuts, registerContext, SHORTCUTS } from './shortcuts.js';

const read = (f) => readFileSync(resolve(process.cwd(), f), 'utf8');
const APP = read('src/App.svelte');
const LIVE = read('src/lib/views/Live.svelte');
const STYLE = LIVE.slice(LIVE.indexOf('<style>'));
const CHROME = APP.slice(APP.indexOf('<header class="topbar-v">'), APP.indexOf('</header>'));

// ─────────────────────────────────────────────────────────────────────────────
// 1 · THE GRID SAYS WHICH CELL IS ON AIR FROM ACROSS THE BOOTH
//
// The colours were already right and already pinned (`slidegridwiring.test.js`:
// amber is ON AIR, steel blue is the preview). What was wrong was the AMOUNT:
// one hairline and a soft fill, on a 158px cell, in a grid of twenty rendered
// slides. This is the weight, not the hue, and nothing here introduces a colour.
// ─────────────────────────────────────────────────────────────────────────────
describe('L4 · the live cell and the cued cell are unmistakable at 158px', () => {
  /** The declaration block of one rule, from the stylesheet. */
  const rule = (sel) => {
    const at = STYLE.indexOf(`\n  ${sel}{`);
    expect(at, `${sel} is not declared in Live.svelte`).toBeGreaterThan(-1);
    return STYLE.slice(at, STYLE.indexOf('}', at) + 1);
  };

  it('a cell paints its state on TWO edges, not one hairline', () => {
    // A 2px border plus a 1px inset outline is three pixels of colour around the
    // picture. One `border-color` on a 1px border was the whole signal before,
    // and at cell size it is the smallest thing on the surface.
    const base = rule('.sg-thumb');
    expect(base).toMatch(/border:2px solid transparent/);
    expect(base).toMatch(/outline:1px solid var\(--v-line2\)/);

    for (const [state, token] of [
      ['.sg-cell.islive .sg-thumb', '--v-amber'],
      ['.sg-cell.cued .sg-thumb', '--v-sel'],
    ]) {
      const r = rule(state);
      expect(r, `${state} must colour the border`).toContain(`border-color:var(${token})`);
      expect(r, `${state} must colour the outline too`).toContain(`outline-color:var(${token})`);
    }
  });

  it('the coloured edge costs the cell no extra space, so the grid never reflows', () => {
    // `outline-offset:-1px` draws the outline INSIDE the box. Without it, a cell
    // going live would grow by two pixels and nudge every cell after it — on the
    // one surface an operator is clicking during a service.
    expect(rule('.sg-thumb')).toMatch(/outline-offset:-1px/);
  });

  it('the LABEL carries the state as well, for a cell whose slide is dark', () => {
    expect(STYLE).toMatch(/\.sg-cell\.islive \.sg-ttl\{color:var\(--v-amber\)\}/);
    expect(STYLE).toMatch(/\.sg-cell\.cued \.sg-ttl\{color:var\(--v-sel\)\}/);
  });

  it('keyboard focus is still its own ring, outside the box', () => {
    // Or a cell that is already live would be indistinguishable from a live cell
    // the keyboard has landed on.
    expect(rule('.sg-cell:focus-visible .sg-thumb')).toMatch(/outline-offset:2px/);
  });

  it('introduces no colour the law has not already spoken for', () => {
    // amber = ON AIR, amethyst = rehearsal, cyan = a guess, grey = cued. Two of
    // those four are spoken for elsewhere on this console and a cell's edge is
    // not entitled to either: amethyst on a live cell would read "nothing is
    // reaching the congregation" over something that is.
    const edge = STYLE.slice(STYLE.indexOf('\n  .sg-thumb{'), STYLE.indexOf('\n  .sg-tag{'));
    const rules = edge.replace(/\/\*[\s\S]*?\*\//g, '');
    expect(rules).not.toMatch(/--v-amethyst|--v-cyan|--v-rose|--v-emerald/);
    // And the two it IS entitled to are the two it already had.
    expect(rules).toMatch(/--v-amber/);
    expect(rules).toMatch(/--v-sel/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · THE TAKE COLUMN IS ONE BLOCK
//
// It was `Take` · TAKE · `‹ Prev` · `Next ›` · `walks the programme` · `VERSE`,
// six children at one even gap, so nothing in the markup said which caption
// belonged to which control.
// ─────────────────────────────────────────────────────────────────────────────
describe('L4 · the take column reads as one block, in three bands', () => {
  const rack = LIVE.slice(LIVE.indexOf('<aside class="rack">'), LIVE.indexOf('</aside>'));

  it('is three bands, in the order take · step · look', () => {
    const bands = [...rack.matchAll(/class="rk-band ([a-z-]+)"/g)].map((m) => m[1]);
    expect(bands).toEqual(['rk-take', 'rk-step', 'rk-x']);
  });

  it('the bands are divided by a seam, and the rack no longer pads its children apart', () => {
    // The whole fix is that the gap BETWEEN bands is a rule and the gap INSIDE a
    // band is 6px. An even gap everywhere is what made six controls read as six
    // unrelated things.
    expect(STYLE).toMatch(/\.rk-band\{[^}]*border-top:1px solid var\(--v-line\)/);
    expect(STYLE).toMatch(/\.rk-band:first-child\{border-top:0\}/);
    expect(STYLE).toMatch(/\.rack\{[^}]*padding:0/);
  });

  it('the two arrows are ONE named group, not two loose buttons', () => {
    const step = rack.slice(rack.indexOf('class="rk-band rk-step"'), rack.indexOf('class="rk-band rk-x"'));
    expect(step).toMatch(/role="group" aria-label="Transport"/);
    expect(step).toContain('‹ Prev');
    expect(step).toContain('Next ›');
    // The shared button instrument, not a hand-rolled shape (B1).
    expect([...step.matchAll(/class="r-btn rk wide"/g)]).toHaveLength(2);
  });

  it('the MODE badge is inside the band it is about', () => {
    // Relay's, not the prototype's: the transport is MODE-AWARE and says so,
    // because the same key silently meaning two things is how the wrong thing
    // reaches a congregation. It was a loose line under the caption; it is part
    // of the caption now, and it did not become smaller or quieter.
    const step = rack.slice(rack.indexOf('class="rk-band rk-step"'), rack.indexOf('class="rk-band rk-x"'));
    expect(step).toContain('rack-mode');
    expect(step).toMatch(/SLIDE' : 'VERSE'/);
  });

  it('the caption no longer carries a margin on top of the band gap', () => {
    // Two spacings stacked is exactly how it came to read as a fifth loose line.
    const cap = STYLE.slice(STYLE.indexOf('\n  .rack-cap{'), STYLE.indexOf('}', STYLE.indexOf('\n  .rack-cap{')));
    expect(cap).not.toMatch(/margin-top/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5 · THE KEYS LEGEND
//
// The cheatsheet already exists behind `?`. This is the short form, always on
// the screen — and the one thing it may never become is a second list of what
// the keys do. `transitionoverride.test.js` holds item 3 and `livedesk.test.js`
// holds item 4; both moved with the thing they are about.
// ─────────────────────────────────────────────────────────────────────────────
describe('L4 · the keys legend states what is bound, and nothing else', () => {
  it('every entry in the ONE table carries its own short form', () => {
    // The gloss lives beside the binding. A list of short labels written out in
    // the shell would be a second source of truth about the keys, and the one
    // that would eventually disagree is the one nobody tests.
    for (const s of SHORTCUTS) {
      expect(typeof s.short, `${s.keys.join('/')} has no short form`).toBe('string');
      expect(s.short.length).toBeGreaterThan(0);
    }
  });

  it('the chrome renders it from `liveShortcuts` and writes no key of its own', () => {
    const leg = CHROME.slice(CHROME.indexOf('<span class="keyleg"'), CHROME.indexOf('>Emergency Stop<'));
    expect(leg).toMatch(/\{#each \$liveShortcuts as s/);
    expect(leg).toMatch(/\{s\.short\}/);
    // No literal keycap anywhere in the strip's markup. If a key is not in the
    // table, the legend cannot mention it.
    expect(leg).not.toMatch(/<kbd>[A-Za-z0-9]/);
  });

  it('sits after the screen lamps and BEFORE the panic control, which is still last', () => {
    // A reference strip may never be the reason Emergency Stop moved (rule 15,
    // DECISIONS §20).
    // The BUTTON, not the word — it is named in the comment above the legend
    // precisely because it is the thing the legend must never displace, and an
    // index that matched the prose would pass while the markup was wrong.
    const stop = CHROME.indexOf('>Emergency Stop<');
    const leg = CHROME.indexOf('<span class="keyleg"');
    expect(CHROME.indexOf('siglamps')).toBeLessThan(leg);
    expect(leg).toBeLessThan(stop);
    // And nothing at all comes after it.
    expect(CHROME.slice(stop + '>Emergency Stop<'.length)).toMatch(/^\/button>\s*$/);
  });

  it('gives way in two rungs, and the panic control is on neither', () => {
    const css = read('src/app.css');
    const block = css.slice(css.indexOf('L4 · THE KEYS LEGEND'));
    expect(block).toMatch(/@media \(max-width:1400px\)\{ \.keyleg \.kl\.ctx\{ display:none; \} \}/);
    expect(block).toMatch(/@media \(max-width:1180px\)\{ \.keyleg\{ display:none; \} \}/);
  });

  it('says Space ADVANCES, and never that Space takes', () => {
    // The operator's sketch read `Space take`. Rule 11 gives Space exactly one
    // meaning app-wide and TAKE is a button; a legend that taught otherwise would
    // be teaching a false fact about a key that puts scripture in front of people.
    const next = SHORTCUTS.find((s) => s.needs === 'next');
    expect(next.keys).toContain('Space');
    expect(next.short).not.toMatch(/take/i);
    expect(SHORTCUTS.some((s) => /take/i.test(s.short))).toBe(false);
  });

  it('does not advertise the two keys the operator asked for and nothing binds', () => {
    // `R` rehearse and `1`–`6` workspace. They are in the prototype's page
    // preamble and in no keydown handler Relay has; a legend that listed them
    // would hand an operator a key that does nothing, under pressure. (`R` is
    // already held from the binding side by `shortcuts.test.js`.)
    const advertised = SHORTCUTS.flatMap((s) => s.keys).map((k) => k.toLowerCase());
    for (const k of ['r', '1', '2', '3', '4', '5', '6']) {
      expect(advertised, `${k} is advertised but nothing binds it`).not.toContain(k);
    }
  });
});

describe('L4 · and the workspace digits really are dead keys', () => {
  let teardown;
  afterEach(() => {
    teardown?.();
    teardown = undefined;
  });

  it('1 through 6 switch nothing and reach no panic control', () => {
    // The table is one door; this is the other. A legend that is honest about the
    // table would still be wrong if a digit quietly did something.
    const clearScreens = vi.fn();
    const blackScreen = vi.fn();
    teardown = installShortcuts({ clearScreens, blackScreen });
    const ctx = { accept: vi.fn(), dismiss: vi.fn(), next: vi.fn(), prev: vi.fn(), search: vi.fn() };
    const unregister = registerContext(ctx);
    for (const k of ['1', '2', '3', '4', '5', '6']) {
      const e = new KeyboardEvent('keydown', { key: k, bubbles: true, cancelable: true });
      Object.defineProperty(e, 'target', { value: document.body });
      window.dispatchEvent(e);
    }
    for (const fn of Object.values(ctx)) expect(fn).not.toHaveBeenCalled();
    expect(clearScreens).not.toHaveBeenCalled();
    expect(blackScreen).not.toHaveBeenCalled();
    unregister();
  });
});
