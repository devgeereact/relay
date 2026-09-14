// L4 · THE RUN SURFACE — what the operator asked for, and what it may not cost.
//
// Five changes to the Sunday-morning console, each from the operator driving the
// rendered console beside the prototype:
//
//   1. the live and cued cells must be findable at a glance in a grid of twenty;
//   2. the take column must read as ONE block, not five stacked fragments;
//   3. the transition picker belongs beside the transport, not in the chrome;
//   4. the detection column may not carry a second copy of the dock's ARM switch;
//   5. a keys legend across the chrome — REVERSED on 2026-09-14, see the last
//      describe in this file. The chrome is navigation now, and `?` is once
//      again the one place the keys are documented.
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
// 5 · THE KEYS LEGEND — BUILT, THEN REVERSED (2026-09-14)
//
// A `.keyleg` strip briefly sat in the chrome, rendered from `liveShortcuts`, on
// the argument that an operator mid-service will not stop and press `?`. The
// operator's instruction a day later was to clear the chrome back to the
// wordmark and the six workspaces, and the legend went with the rest of it.
//
// WHAT THESE TESTS NOW HOLD is the reversal, not the absence: a strip that was
// removed from the markup while its `short` glosses stayed on `SHORTCUTS`, or
// its media rules stayed in `app.css`, would be the dead-weight defect this
// repository has cleaned up twice — so both halves are asserted. The claims the
// legend itself carried (nothing unbound may be advertised; `Space` advances and
// never takes) belong to the TABLE and to the cheatsheet, so they survive the
// legend and are still checked below.
// ─────────────────────────────────────────────────────────────────────────────
describe('L4 · the keys legend was removed, and left nothing behind', () => {
  it('the chrome renders no legend and no keycap', () => {
    expect(CHROME).not.toContain('class="keyleg"');
    expect(CHROME).not.toContain('<kbd>');
    expect(CHROME).not.toContain('$liveShortcuts');
  });

  it('and the ONE table carries no `short` gloss that nothing reads', () => {
    // The field existed for the legend and for nothing else. Left behind it
    // would be a column on every entry that no surface renders — the same dead
    // weight as a stylesheet rule whose class is gone.
    for (const s of SHORTCUTS) {
      expect(s, `${s.keys.join('/')} still carries a short form`).not.toHaveProperty('short');
    }
    expect(read('src/lib/shortcuts.js')).not.toMatch(/short:\s*'/);
    expect(read('src/App.svelte')).not.toContain('s.short');
  });

  it('and the stylesheet keeps no rule for it, including its two rungs', () => {
    const css = read('src/app.css');
    for (const sel of ['.keyleg{', '.keyleg .kl{', '.keyleg kbd{', '.keyleg .klw{']) {
      expect(css, `${sel} renders nothing now`).not.toContain(sel);
    }
    expect(css).not.toContain('.keyleg .kl.ctx{ display:none; }');
  });

  it('`?` is once again the one place the keys are documented', () => {
    // The cheatsheet reads the same `liveShortcuts` the legend did, so nothing
    // an operator could learn from the strip has become unavailable — it is one
    // keystroke away instead of on the screen, which is the trade the operator
    // asked for and is recorded here rather than assumed.
    expect(APP).toContain('{#if $cheatsheet}');
    const cheat = APP.slice(APP.indexOf('{#if $cheatsheet}'));
    expect(cheat).toMatch(/\{#each \$liveShortcuts as s\}/);
    expect(cheat).toMatch(/\{#each s\.keys as k\}<kbd>\{k\}<\/kbd>\{\/each\}/);
    expect(cheat).toContain('{s.label}');
  });

  it('Space still ADVANCES, and the table still never says it takes', () => {
    // The gloss that could have said otherwise is gone, so this is now a claim
    // about the cheatsheet's sentence. Rule 11 gives Space exactly one meaning
    // app-wide and TAKE is a button on the run surface; help that taught
    // otherwise would be teaching a false fact about a key that puts scripture
    // in front of people.
    const next = SHORTCUTS.find((s) => s.needs === 'next');
    expect(next.keys).toContain('Space');
    expect(SHORTCUTS.some((s) => /\btake\b/i.test(s.label))).toBe(false);
  });

  it('nothing advertises the two keys the operator asked for and nothing binds', () => {
    // `R` rehearse and `1`–`6` workspace. They are in the prototype's page
    // preamble and in no keydown handler Relay has; listing them anywhere would
    // hand an operator a key that does nothing, under pressure. (`R` is already
    // held from the binding side by `shortcuts.test.js`.)
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

// ─────────────────────────────────────────────────────────────────────────────
// C2 · THE MICROPHONE-QUALITY BANNER IS OUT (operator instruction 2026-09-14)
//
//   "take out this notification section completely… and nothing should go
//    there."
//
// The amber box at the foot of Live carried the `dsp.rs` warnings — clipping,
// too quiet, noisy — and beside it the language-instability note. Both are gone,
// and "nothing should go there" is the half of the instruction a test can hold:
// the easy way to half-obey it is to shrink the banner into a chip, which is the
// same claim in less space.
//
// WHAT IT COST IS WRITTEN DOWN RATHER THAN ARGUED WITH. `too_quiet` was the one
// place in the console that named a muted microphone as a muted microphone, and
// a rule-12 failure that nothing announces is exactly the shape the field audits
// were written about. The operator asked for it out; the last assertion here
// holds the note that says so at the site, so the next person finds the
// consequence rather than rediscovering it on a Sunday.
//
// Each assertion was watched to fail with the banners restored.
// ─────────────────────────────────────────────────────────────────────────────
describe('C2 · the mic-quality and language banners are gone from the run surface', () => {
  // The TEMPLATE only: everything after the instance script and before the
  // stylesheet, with its comments stripped. Both halves matter — the script
  // still names `langWarning` in the note recording why it went, and a scanner
  // that read the whole file would report the defect present and the defect
  // fixed at once (the reason `workspacegrammar.test.js` strips prose too).
  const MARKUP = LIVE.slice(LIVE.lastIndexOf('</script>'), LIVE.lastIndexOf('<style>'))
    .replace(/<!--[\s\S]*?-->/g, '');

  it('neither banner is rendered, and neither string table is left behind', () => {
    expect(MARKUP, 'the banner element is still rendered').not.toMatch(/sttwarn/);
    expect(MARKUP).not.toMatch(/qualityWarning|langWarning/);
    // The copy goes with the box. A string table nothing renders is a banner
    // waiting to be put back by somebody who reads it as dead code.
    const script = LIVE.slice(0, LIVE.indexOf('</script>')).replace(/\/\/[^\n]*/g, '');
    expect(script).not.toMatch(/Almost no sound is reaching Relay/);
    expect(script).not.toMatch(/const QUALITY = \{/);
    // …and the rule that dressed it.
    expect(STYLE, '.sttwarn still has a rule of its own').not.toMatch(/\.sttwarn\s*\{/);
  });

  it('and nothing quieter has been put in its place', () => {
    // The slot: between the live region that announces a screen going down and
    // the inspector. Comments are stripped above, so what is left between the
    // two anchors is only markup — and there must be none of it.
    const ANCHOR = 'aria-live="polite">{downAnnounce}</p>';
    const from = MARKUP.indexOf(ANCHOR);
    const to = MARKUP.indexOf('<DetectionInspector');
    expect(from, 'the anchors this test measures between have moved').toBeGreaterThan(-1);
    expect(to).toBeGreaterThan(from);
    expect(MARKUP.slice(from + ANCHOR.length, to).trim(), 'something is in the empty slot').toBe('');
  });

  it('the events are untouched — only the surface went', () => {
    // `audio://quality` and `stt://language_unstable` still cross the bridge and
    // are still stored, so `ipc.test.js`'s contract does not move and a future
    // surface has the facts to hand. Removing a listener to tidy up would be a
    // second, much larger change hiding inside a layout one.
    const CAPTURE = read('src/lib/stores/capture.js');
    expect(CAPTURE).toMatch(/listen\('audio:\/\/quality'/);
    expect(CAPTURE).toMatch(/listen\('stt:\/\/language_unstable'/);
  });

  it('and the file says what a muted microphone now costs', () => {
    // Not prose for its own sake: this is the only record, at the only place
    // somebody restoring the banner would be reading.
    const note = LIVE.match(/<!--[\s\S]*?THE MICROPHONE-QUALITY AND LANGUAGE BANNERS ARE GONE[\s\S]*?-->/);
    expect(note, 'the removal is undocumented at the site').not.toBeNull();
    expect(note[0]).toMatch(/too_quiet/);
    expect(note[0]).toMatch(/ONLY place/);
  });
});
