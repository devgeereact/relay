// THE SHELL — the window chrome, the dock row and the status bar
// (docs/REBRAND.md §1, §2, §12).
//
// `App.svelte` is not unit-testable: it mounts the whole console, opens the
// Tauri bridge and starts three timers. That is exactly why the tab-redirect map
// went stale for two whole tabs — nothing could fail when it did. The answer this
// repository already reached (`session.test.js`, `degraded.test.js`,
// `r2livepath.test.js`) is to put the DECISIONS in pure modules and assert the
// shell's source against them, so this file does both:
//
//   · the pure half lives in `statusbar.js` and `session.js`, tested there;
//   · this file holds the claims that are only true of the shell's own markup —
//     what the strip contains, where a fact comes from, and what the chrome may
//     never grow back.
//
// The Dock is a real component and IS mounted.
//
//   npx vitest run src/lib/shellchrome.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const read = (f) => readFileSync(resolve(process.cwd(), f), 'utf8');
const APP = read('src/App.svelte');
const DOCK = read('src/lib/Dock.svelte');

/**
 * The shell with every comment removed.
 *
 * Any assertion of the form "X must not come back" has to read this and not
 * `APP`. `transitionoverride.test.js` learned it the expensive way in the other
 * direction — an assertion that a call was still PRESENT matched the paragraph
 * describing the call, and passed over a shell that had commented it out. The
 * mirror of that failure is a `not.toContain` that fails because the comment
 * explaining WHY something was removed names the thing it removed, which is
 * every honest comment about a deletion. Only the code is the claim.
 */
const CODE = APP.replace(/<!--[\s\S]*?-->/g, '')
  .replace(/\/\*[\s\S]*?\*\//g, '')
  .replace(/^\s*\/\/.*$/gm, '');

// ── §2 · SIX WORKSPACES, IN THE PROTOTYPE'S ORDER ───────────────────────────
//
// The strip carried eight. Themes is the style layer beneath templates and never
// reaches a wall on its own; Help is not a surface anyone runs a service from.
// Both left the strip, and the whole risk in that change is the second half:
// LEAVING THE STRIP AND BECOMING UNREACHABLE ARE DIFFERENT THINGS, and only one
// of them is allowed (`scripts/qa-inventory.mjs` is the instrument that says so).
describe('§2 · the strip is the six workspaces, in order', () => {
  /** The `key:` of each entry in the shell's `tabs` array, in source order. */
  const stripKeys = () => {
    const arr = APP.slice(APP.indexOf('const tabs = ['), APP.indexOf('const routes'));
    return [...arr.matchAll(/\{\s*key:\s*'([a-z]+)'/g)].map((m) => m[1]);
  };

  it('renders exactly six workspaces in the prototype order', () => {
    // Live · Library · Planner · Templates · Outputs · Settings. Not
    // alphabetical: the Sunday path, then the week's, then the room, then the
    // machine. (`channels` is the Outputs tab's internal key — CLAUDE.md.)
    expect(stripKeys()).toEqual(['live', 'library', 'planner', 'templates', 'channels', 'settings']);
  });

  it('neither Themes, Help nor History takes a slot in it', () => {
    expect(stripKeys()).not.toContain('themes');
    expect(stripKeys()).not.toContain('help');
    // History was a Settings SECTION and is a route of its own again. Off the
    // strip both times: six workspaces is the grammar, and reading back what
    // happened last Sunday is not one of the six jobs a service is run from.
    expect(stripKeys()).not.toContain('history');
  });

  it('but Help and History are real routes with views behind them', () => {
    // A surface nothing can reach is an orphan. Settings renders two controls
    // that set the Help tab and the readiness screen renders one that sets
    // History, and the resolver is handed `routes` (not the strip) so they land
    // rather than bouncing back to Live.
    expect(APP).toMatch(/const routes = \[\.\.\.tabs\.map\(\(x\) => x\.key\), 'help', 'history'\]/);
    expect(APP).toMatch(/resolveActiveTab\(\$session\.activeTab, routes\)/);
    expect(APP).toMatch(/help:\s*\(\) => import\('\.\/lib\/views\/Help\.svelte'\)/);
    expect(APP).toMatch(
      /history:\s*\(\) => import\('\.\/lib\/views\/library\/History\.svelte'\)/,
    );
    expect(read('src/lib/views/Settings.svelte')).toMatch(/activeTab: 'help'/);
    expect(read('src/lib/views/Dashboard.svelte')).toMatch(/activeTab: 'history'/);
  });

  it('the Templates workspace is one desk, and nothing of Themes is left in it', () => {
    // Themes were folded INTO templates (DECISIONS §87) rather than moved again:
    // a theme's every field was already a template `style` key, so the desk
    // beneath this one could only ever say less than the template above it. Every
    // themed template had its theme inlined into its own style first, so no look
    // changed. What is left is browse, then make.
    const TPL = read('src/lib/views/Templates.svelte');
    for (const child of ['TemplateGallery', 'TemplateEditor']) {
      expect(TPL, `${child} must still be rendered by something`).toContain(`<${child}`);
    }
    // No desk means no desk strip, and nothing reads `templatesDesk`. A
    // segmented control offering one option is a control that does nothing; the
    // session key it backed is DROPPED rather than left to be re-persisted for
    // the life of the install, which is the reasoning `session.js` already
    // records about `liveDensity`. `session.test.js` holds that end.
    expect(TPL).not.toMatch(/DeskStrip|templatesDesk|on:desk/);
    // But a saved session naming the old Themes TAB still lands somewhere valid.
    // That is a different mechanism (`MOVED_TABS`) and it is untouched.
    expect(read('src/lib/session.js')).toMatch(/themes: 'templates'/);
  });
});

// ── THE CHROME'S LAMPS ARE GONE, AND THE ROWS THEY WERE DRAWN FROM ARE NOT ──
//
// On 2026-09-14 the operator cleared the chrome bar back to the wordmark and the
// six workspaces. The per-screen lamps went with the rest of it.
//
// Rule 35 is what makes that a change worth testing rather than a deletion. The
// lamps were one of the chrome's two true statements about what a congregation
// can see, and RG-01 — a kiosk source that had gone away still reading On Air —
// is the instance the rule was written from. So the claim is no longer "the
// chrome does not invent a second verdict"; it is that the ROWS still exist, are
// still one `describeScreen` verdict each, and still feed BOTH surfaces that now
// carry the answer: the status bar's tally, and the `Reduced` cell that names a
// screen which has stopped answering.
describe('the screen rows survive the lamps, and stay one verdict per screen', () => {
  const lampBlock = () => APP.slice(APP.indexOf('$: screenLamps'), APP.indexOf('$: screens ='));

  it('every row is a describeScreen verdict', () => {
    expect(lampBlock()).toContain('describeScreen(');
  });

  it('and no row is derived from global state', () => {
    // The RG-01 shape, literally: `$live && !$rehearsing && !$screenBlack`
    // deciding a per-screen badge. The wall IS passed to `describeScreen` — that
    // is correct, it is one of its two arguments — but the verdict must come back
    // out of the helper, never be computed here.
    const block = lampBlock();
    expect(block).not.toMatch(/kind\s*=\s*\$live/);
    expect(block).not.toMatch(/\$live\s*&&\s*!\$(rehearsing|screenBlack)/);
  });

  it('the SCREENS tally is counted from those same rows', () => {
    // If the count came from `$channelHealth` again it could disagree with the
    // Reduced cell it sits beside, which is the same failure in arithmetic.
    expect(APP).toMatch(/\$: screens = screenTally\(screenLamps\)/);
    expect(APP).toMatch(/\{screens\.live\} of \{screens\.total\}/);
  });

  it('a screen that has stopped answering is still NAMED, not merely counted', () => {
    // The lamps said WHICH screen was red, by name, on every workspace. The tally
    // that replaced them says "2 of 3" and cannot. What carries the name now is
    // the Reduced cell, fed by `screensDown` — which reads the SAME
    // `describeScreen` verdict, so the two cannot disagree about a screen.
    expect(APP).toMatch(/\$: screensDown = Object\.values\(\$channelHealth\)/);
    expect(APP).toContain("describeScreen(st, {}, Number.MAX_SAFE_INTEGER).kind === 'down'");
    expect(APP).toMatch(/screensDown,/);
    const bar = APP.slice(APP.indexOf('<footer class="footer-v"'), APP.indexOf('</footer>'));
    expect(bar).toContain('summarise(degraded)');
  });

  it('the chrome keeps no lamp, and the stylesheet keeps no rule for one', () => {
    // A class nothing renders is the dead CSS the design system warns about, and
    // this repository has cleaned that up twice. Both halves, or neither.
    const chrome = CODE.slice(CODE.indexOf('<header class="topbar-v">'), CODE.indexOf('</header>'));
    expect(chrome).not.toContain('class="siglamps"');
    expect(chrome).not.toContain('class="signm"');
    expect(CODE).not.toContain('LAMP_TONE');
    expect(CODE).not.toContain('lampWord');
    const css = read('src/app.css');
    for (const sel of ['.siglamps{', '.siglamps .sig{', '.siglamps .signm{']) {
      expect(css, `${sel} renders nothing now`).not.toContain(sel);
    }
  });

  it("the status bar's one lamp still wears only the colour law", () => {
    // amber = on air, amethyst = rehearsal (and safe mode, which outranks it),
    // grey = everything else. No fifth colour, and in particular no green "ok":
    // green is not in the law.
    const bar = APP.slice(APP.indexOf('<footer class="footer-v"'), APP.indexOf('</footer>'));
    const at = bar.indexOf('<span class="lamp');
    const lamp = bar.slice(at, bar.indexOf('</span>', at));
    expect(lamp).toContain("'amber'");
    expect(lamp).toContain("'amethyst'");
    expect(lamp).toContain("'grey'");
    expect(lamp).not.toMatch(/green|emerald/i);
  });
});

// ── §2 · THE STATUS BAR ─────────────────────────────────────────────────────
describe('§2 · every figure in the status bar comes from a real fact', () => {
  const bar = () => APP.slice(APP.indexOf('<footer class="footer-v"'), APP.indexOf('</footer>'));

  it('carries the cells the prototype carries', () => {
    const b = bar();
    for (const k of ['On air', 'Latency p50', 'Dropped', 'Model', 'Cadence', 'Screens', 'Clock']) {
      expect(b, `the ${k} cell`).toContain(`>${k}</span>`);
    }
  });

  it('reads every figure through the one pure module', () => {
    // Not "contains a number": contains the DERIVATION. A figure computed inline
    // here is a figure no test can reach, which is how the strip would acquire a
    // second opinion about latency or about the model.
    for (const fn of ['wallState(', 'latencyP50(', 'cadence(', 'dropped(', 'screenTally(', 'modelLabel(', 'elapsed(']) {
      expect(APP, `${fn} must be where this figure comes from`).toContain(fn);
    }
  });

  it('the state sentence is the same ladder as the chrome badge', () => {
    // Two ladders is how one strip says On Air while the other says Rehearsal, on
    // the same screen, about the same wall.
    expect(bar()).toContain('{wall.words}');
    expect(APP).toMatch(/\$: wall = wallState\(\{/);
  });

  it('no cell prints a constant dressed up as a measurement', () => {
    // The prototype's `LOCAL offline` is a literal, and a literal in a status bar
    // reads identically whether the thing behind it is fine or on fire (rule 35).
    // Every value in this strip is an expression.
    const values = [...bar().matchAll(/class="v[^"]*">([^<{][^<]*)</g)].map((m) => m[1].trim());
    expect(values).toEqual([]);
  });

  it('a fact that is absent says so instead of printing zero', () => {
    expect(bar()).toContain('orNoData(');
    // The two that would otherwise be a plausible-looking zero.
    expect(bar()).toMatch(/lat === null \? orNoData\(null\) : `\$\{lat\} ms`/);
    expect(bar()).toMatch(/cad === null \? orNoData\(null\) : `\$\{cad\} \/s`/);
  });

  it('the on-air clock is cleared, not zeroed, when the screens are', () => {
    // `00:00:00` is a measurement of nothing and reads exactly like a stopped
    // clock. Blackout deliberately does NOT stop it: the wall is black and the
    // session is still on air, which is the state being counted.
    expect(APP).toMatch(/\$: if \(!\$live\) onAirFrom = null;/);
    expect(APP).not.toMatch(/\$screenBlack.*onAirFrom = null/);
  });
});

// ── THE CHROME IS NAVIGATION, AND NOTHING ELSE ──────────────────────────────
//
// 2026-09-14, on the operator's instruction. Six things left this bar: the ON AIR
// ladder, the LISTENING chip, the PROTECTED chip, the screen lamps, the keys
// legend, and an Emergency Stop button.
//
// The last of those was a PANIC CONTROL (rule 15, DECISIONS §20), so this
// describe does not merely assert that it is gone. It asserts that the two paths
// it duplicated are both still there — because removing it is safe only for
// exactly as long as they are, and a later edit that quietly took one of them
// away would otherwise leave the operator with nothing.
describe('the chrome is the wordmark and the six workspaces', () => {
  // Comments stripped: this describe is all "must not come back", and the
  // comment in the shell that records WHY each of the six went names all six.
  const chrome = () => CODE.slice(CODE.indexOf('<header class="topbar-v">'), CODE.indexOf('</header>'));

  it('carries the lockup and the workspace strip', () => {
    const c = chrome();
    expect(c).toContain('<span class="chrome-brand">');
    expect(c).toContain('<nav class="ws-menu" aria-label="Workspaces">');
  });

  it('and renders no state, no lamp, no legend and no button at all', () => {
    const c = chrome();
    // Every one of the six, by the markup that drew it rather than by the words
    // in the comment that records why it went.
    for (const gone of [
      'class="r-badge',      // the On Air / Rehearsal / Blackout / Screens clear ladder
      'class="topbar-live"', // …and the name of what is on the wall
      'class="topbar-mic"',  // LISTENING
      'class="lockchip',     // PROTECTED
      'class="siglamps"',    // one lamp per screen
      'class="keyleg"',      // the keys legend
    ]) {
      expect(c, `${gone} must not be back in the chrome`).not.toContain(gone);
    }
    // No control of any kind after the nav. The workspace tabs are the only
    // buttons in this bar, and they are inside it.
    expect(c.slice(c.indexOf('</nav>'))).not.toContain('<button');
  });

  it('Emergency Stop is gone, and nothing in the shell took its place', () => {
    expect(CODE).not.toContain('Emergency Stop');
    // The shell's ONLY remaining uses of `clearScreens` are the import and the
    // keyboard install. A button added back here later would be a panic control
    // on a surface that is now navigation, which is what this line prevents.
    expect([...CODE.matchAll(/clearScreens/g)]).toHaveLength(2);
  });

  it('…because Esc still clears from every tab, mounted once in the shell', () => {
    // rule 15 / rule 11. `installShortcuts` binds Escape straight to
    // `clearScreens` — never through a view's context — so it survives a crashed
    // workspace. `panic.test.js` and `shortcuts.test.js` drive the key itself;
    // this is the half only the shell can answer: that it is installed here, once.
    expect(APP).toMatch(/teardownKeys = installShortcuts\(\{ clearScreens, blackScreen \}\)/);
    expect([...APP.matchAll(/installShortcuts\(/g)]).toHaveLength(1);
    const SC = read('src/lib/shortcuts.js');
    expect(SC.slice(SC.indexOf("if (e.key === 'Escape')"))).toContain('clearScreens();');
  });

  it('…and because Clear screens is in the dock, on every workspace', () => {
    // The dock is rendered by the SHELL, not by Live, so the second path is there
    // while an operator is editing a template. Full-screen Live hides both the
    // dock and this header — and it always did, so the key was already the only
    // path there and nothing about that case changed.
    expect(APP).toContain('<Dock />');
    expect(DOCK).toContain('<button class="r-cbtn danger wide" on:click={doClear}');
    // Full width along the bottom edge of a card that never scrolls.
    const ctlbody = DOCK.slice(DOCK.indexOf('.ctlbody {'));
    expect(ctlbody.slice(0, ctlbody.indexOf('}'))).toMatch(/overflow:\s*hidden/);
  });

  it('the two decorative icons are still gone', () => {
    // A drawn "Signal" glyph wired to nothing, and a clock face beside a clock.
    // In a room whose whole premise is that an indicator means something, a
    // picture of an indicator is the defect drawn rather than written.
    expect(chrome()).not.toContain('title="Signal"');
    expect(chrome()).not.toContain('class="topbar-icons"');
  });
});

// ── §2 · THE DOCK ROW ───────────────────────────────────────────────────────
describe('§2 · four equal cards on a trough', () => {
  const dockCss = () => DOCK.slice(DOCK.indexOf('<style>'));

  it('the row is one fixed height and the cards are seams apart', () => {
    const row = dockCss().slice(dockCss().indexOf('.dock {'), dockCss().indexOf('.dpanel {'));
    expect(row).toMatch(/height:\s*178px/);
    // The 1px gap over a darker ground IS the seam. Without it the four read as
    // adjacent panels rather than four instruments in a rack.
    expect(row).toMatch(/gap:\s*1px/);
    expect(row).toMatch(/background:\s*var\(--v-rule\)/);
  });

  it('every card has a grip, a caption and a meta slot', () => {
    const heads = [...DOCK.matchAll(/<div class="dhead">([\s\S]*?)<\/div>/g)].map((m) => m[1]);
    expect(heads.length, 'four cards, four heads').toBe(4);
    for (const h of heads) {
      expect(h).toContain('class="grip"');
      expect(h).toContain('class="dk"');
    }
    // Three of the four have something to say in the meta slot; the Controls card
    // has nothing true to put there and correctly puts nothing.
    expect(heads.filter((h) => h.includes('dspring')).length).toBe(3);
  });

  it('the grip is furniture, not a control that does nothing', () => {
    // It does not drag anything and must not look like it might to a screen
    // reader: a control that reports itself and then refuses is worse than none.
    expect(DOCK).toMatch(/<span class="grip" aria-hidden="true">/);
  });
});

describe('the controls card can never scroll a panic control out of reach', () => {
  it('its body hides overflow rather than scrolling it', () => {
    // rule 15's neighbourhood: an operator may never have to scroll to reach
    // Clear screens. `overflow:auto` here would be a panic control that is
    // reachable on a 1440px booth screen and not on a 1280px one.
    const css = DOCK.slice(DOCK.indexOf('.ctlbody {'));
    expect(css.slice(0, css.indexOf('}'))).toMatch(/overflow:\s*hidden/);
  });

  it('the card runs End service · Rehearse · Blackout · Clear screens', () => {
    // THE ORDER CHANGED ON 2026-09-14, on the operator's instruction, to the
    // prototype's: the session control on top, the two state controls as a pair,
    // and Clear screens full width along the bottom edge. What rule 15 requires is
    // unchanged and still tested: the card never scrolls, so no panic control is
    // ever behind an overflow edge, and at one column the whole card is ordered
    // first (`panic.test.js`).
    const card = DOCK.slice(DOCK.indexOf('<div class="dbody ctlbody">'));
    // By CLASS, not by label: `End service` also appears in the button's own
    // title text, so a label search finds the prose before the control.
    const order = ['r-cbtn endsvc', 'rehearse', 'r-cbtn black', 'r-cbtn danger wide'];
    let at = -1;
    for (const label of order) {
      const i = card.indexOf(label);
      expect(i, `${label} must be in the controls card`).toBeGreaterThan(at);
      at = i;
    }
    expect(card).toContain('class="r-cbtn danger wide"');
  });

  it('Blackout and Rehearse report their STATE, not just their name', () => {
    // "Blackout is a STATE, not a press": the button says whether the wall is
    // black, which is the one thing an operator glancing at this dock needs.
    const card = DOCK.slice(DOCK.indexOf('<div class="dbody ctlbody">'));
    expect(card).toContain('data-on={$screenBlack');
    expect(card).toContain('data-on={$rehearsing');
    expect(card).toContain('Black — restore');
    expect(card).toContain('Rehearsing');
  });

  it('the two panic controls do not go through the dock error wrapper', () => {
    // `clearScreens`/`blackScreen` return a boolean AND set `panicError`
    // themselves, because they also fire from a global key handler that cannot
    // catch (rule 15). Wrapping them in `run()` would put their failure in a
    // 9px line inside a dock panel instead of on the shell's panic banner.
    expect(DOCK).toMatch(/const doClear = \(\) => clearScreens\(\);/);
    expect(DOCK).toMatch(/const doBlack = \(\) => blackScreen\(\);/);
  });
});

// ── §2 · LIVE AUDIO ─────────────────────────────────────────────────────────
describe('the live-audio card shows the signal and the two decisions about it', () => {
  let host;
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_sensitivity') return 50;
      if (cmd === 'set_sensitivity') return 62;
      return null;
    });
  });
  afterEach(() => {
    host?.remove();
    host = null;
  });

  const settle = () => new Promise((r) => setTimeout(r, 0));
  async function mount() {
    const { default: Dock } = await import('./Dock.svelte');
    host = document.createElement('div');
    document.body.appendChild(host);
    const c = new Dock({ target: host });
    for (let i = 0; i < 4; i++) await settle();
    return c;
  }

  it('draws a waveform, not a bar meter', async () => {
    // A meter says how loud; a waveform says what the microphone has been
    // hearing, which is the question an operator is actually asking when the AI
    // goes quiet.
    await mount();
    expect(host.querySelector('canvas.wave')).toBeTruthy();
  });

  it('the sensitivity dial is here, beside the signal, and reads from the engine', async () => {
    const cap = await import('./stores/capture.js');
    cap.capture.update((s) => ({ ...s, available: true }));
    invoke.mockImplementation(async (cmd) => (cmd === 'get_sensitivity' ? 74 : null));
    await mount();
    const dial = host.querySelector('input[aria-label="Detection sensitivity"]');
    expect(dial).toBeTruthy();
    // It shows what the ENGINE holds, not a plausible default: there is one
    // forward mapping and one inverse and both live in Rust (DECISIONS §26).
    expect(invoke.mock.calls.map((c) => c[0])).toContain('get_sensitivity');
    expect(dial.value).toBe('74');
    cap.capture.update((s) => ({ ...s, available: false }));
  });

  it('with no engine attached the card says so in WORDS, and the figure stays a figure', async () => {
    // `getSensitivity` swallows and returns 50 with no backend, and 50 is also a
    // perfectly ordinary real setting — so the reading alone cannot tell the two
    // apart. On the one control that governs what the AI may put on a wall
    // unasked, a plausible number nobody set is rule 35 with a dial on it.
    //
    // The caveat goes in the card's META SLOT, the one place each dock card
    // already has for "I have no answer" — `no model` on the transcript card is
    // the same sentence about a different absence. NOT in the value column: a
    // glyph there cannot tell "nobody answered" from "the gate is at 50"
    // (R3-13), and an 18px column cannot hold the sentence that could.
    await mount();
    const meta = [...host.querySelectorAll('.dmeta')].map((n) => n.textContent.trim());
    expect(meta).toContain('no engine');
    // The dial is inert, because there is nothing to set.
    expect(host.querySelector('input[aria-label="Detection sensitivity"]').disabled).toBe(true);
    // And the figure is a figure: the value `getSensitivity` returned, not a
    // glyph and not an invented "unknown" number, which would be a second
    // reading nobody set laid over the first.
    expect(host.querySelector('.sensv').textContent.trim()).toMatch(/^\d+$/);
  });

  it('and it does not print `quiet` over a microphone that is not there', async () => {
    // `quiet` and `−∞ dB` read exactly like a live microphone in a silent room,
    // which is the one thing they must not be mistaken for when there is no
    // engine at all. Neither is shown; the card says what is true instead.
    await mount();
    expect(host.querySelector('.vad')).toBeNull();
    expect(host.querySelector('.db')).toBeNull();
  });

  it('detection is ONE switch, in the card about the signal', async () => {
    await mount();
    const sw = host.querySelector('[role="switch"][aria-label="Detection"]');
    expect(sw).toBeTruthy();
    // And it is not ALSO a button in the controls card — two controls for one
    // setting is two places for them to disagree.
    const card = DOCK.slice(DOCK.indexOf('<div class="dbody ctlbody">'));
    expect(card).not.toMatch(/setDetection/);
  });

  it('the waveform draws no absolute gate line', async () => {
    // Rule 12 / DECISIONS §19: audio levels are LEARNED and nothing may compare a
    // signal to an absolute level. A dashed line at a fixed height would draw the
    // voice gate as a threshold it is not — and the one time that picture matters
    // (a quiet preacher on a church laptop) it would be wrong in exactly the way
    // that made Relay silently deaf.
    const script = DOCK.slice(0, DOCK.indexOf('</script>'));
    expect(script).not.toContain('setLineDash');
    expect(script).not.toMatch(/gate\s*=/);
  });

  // ── §7 · THE COUNTDOWN FIGURE ─────────────────────────────────────────────
  //
  // It is the largest thing in the Quick tools panel because it is the one thing
  // an operator reads from across a booth. That makes conflating its two states
  // expensive: the SET duration and what the screens are counting are different
  // facts, and a big number with no label is the half of a status line that lies.
  it('the countdown figure says WHICH of its two facts it is showing', async () => {
    const cap = await import('./stores/capture.js');
    cap.live.set(null);
    await mount();
    // Nothing on the wall: it still renders — the panel's biggest control used to
    // have no readout at all until after it had been used — and it says so.
    const fig = host.querySelector('.tfig');
    expect(fig).toBeTruthy();
    expect(fig.classList.contains('live')).toBe(false);
    expect(host.querySelector('.cdstatev').textContent.trim()).toBe('not counting');

    cap.live.set({ countdown_to: Date.now() + 5 * 60_000 });
    await settle();
    expect(host.querySelector('.tfig').classList.contains('live')).toBe(true);
    expect(host.querySelector('.cdstatev').textContent.trim()).toBe('on the screens');
    cap.live.set(null);
  });

  it('the warning state is red, never amber, and only while it is on a wall', async () => {
    // Amber in this room means ON AIR and is never allowed to be anything else
    // (rule 18). The warning window itself is `layers.js::countdownWarning` — one
    // rule, shared with the wall and the stage page.
    const css = DOCK.slice(DOCK.indexOf('.tfig {'));
    expect(css).toMatch(/\.tfig\.warn \{ color: var\(--v-red\); \}/);
    // The TOKEN, not the word — the comment beside it says "never amber", which
    // is the sentence a naive grep would have been satisfied by.
    expect(css.slice(0, css.indexOf('.cdstate'))).not.toMatch(/var\(--v-amber/);
    // `cdLive &&` is the half that stops a SET duration under a minute from
    // pulsing red at an operator about a countdown nobody can see.
    expect(DOCK).toMatch(/\$: cdWarn = cdLive && countdownWarning\(/);
  });

  it('the transcript card says what is producing the transcript, or that nothing is', async () => {
    await mount();
    // With no model loaded it must not read like a working recogniser that has
    // simply not heard anything yet.
    const meta = [...host.querySelectorAll('.dmeta')].map((n) => n.textContent.trim());
    expect(meta).toContain('no model');
  });
});

// ═══════════════════════════════════════════════════════════════════════════
// S1 · WAVE 3 — the chrome's lockup, the fourth control, and the strip's cells.
// ═══════════════════════════════════════════════════════════════════════════

// ── §1 · THE LOCKUP IS TWO WORDS ────────────────────────────────────────────
describe('§1 · the wordmark carries the product AND the room', () => {
  const brand = () => APP.slice(APP.indexOf('<span class="chrome-brand">'), APP.indexOf('<nav class="ws-menu"'));

  it('renders RELAY and the mono tag beside it', () => {
    // The prototype's `.wordmark` is `<b>RELAY</b><span>studio</span>`; the app
    // carried only the first half.
    const b = brand();
    expect(b).toContain('<b>RELAY</b>');
    expect(b).toMatch(/class="chrome-tag"[^>]*>studio</);
  });

  it('the tag is decoration, not a second name', () => {
    // Every other surface — the bundle id, the window title, every document —
    // says Relay. A screen reader announcing "relay studio" here would be the
    // only place in the product that disagreed.
    expect(brand()).toMatch(/class="chrome-tag" aria-hidden="true"/);
  });

  it('and it is the first thing to give way when the bar is tight', () => {
    // The bar's fixed inhabitants are the panic control and the screen lamps. A
    // brand tag may never be the reason either of them moves, so it goes before
    // the row is under pressure rather than after.
    const css = read('src/app.css');
    expect(css).toMatch(/@media \(max-width:1180px\)\{ \.chrome-brand \.chrome-tag\{ display:none; \} \}/);
  });
});

// ── §1 · THE FOURTH CONTROL ─────────────────────────────────────────────────
//
// The card declined this for a whole wave, with a correct reason: there was no
// honest backend fact to drive it, and a button off a frontend flag would offer
// to end a service that had already ended (rule 35). The fact now exists.
describe('§1 · End service is the fourth control, and it reads the service', () => {
  const card = () =>
    DOCK.slice(DOCK.indexOf('<div class="dbody ctlbody">'), DOCK.indexOf('</section>', DOCK.indexOf('<div class="dbody ctlbody">')));

  it("four buttons, in the repository's order rather than the prototype's", () => {
    // The prototype's order, adopted on the operator's instruction 2026-09-14:
    // the session control on top, the two state controls as a pair, Clear screens
    // full width along the bottom. Matched by CLASS — `End service` also appears
    // in the button's own title text.
    const order = ['r-cbtn endsvc', 'rehearse', 'r-cbtn black', 'r-cbtn danger wide'];
    let at = -1;
    for (const label of order) {
      const i = card().indexOf(label);
      expect(i, `${label} must be in the controls card`).toBeGreaterThan(at);
      at = i;
    }
  });

  it('it is driven by `recording`, never by the LOCK', () => {
    // `engaged` is armed by start_service and released by end_service, so the
    // two agree almost always — and come apart the moment an operator lifts the
    // lock, which is a thing the product invites them to do. A button off
    // `engaged` would then say there is nothing to end over an open record.
    expect(DOCK).toMatch(/\$: recording = !!\$serviceLock\.recording;/);
    expect(card()).not.toMatch(/serviceLock\.engaged/);
    // And Rust reads it from the session itself — the same state `end_service`
    // clears — rather than from the lock.
    expect(read('src-tauri/src/main.rs')).toMatch(/recording: session\.0\.lock\(\)\.is_ok_and\(\|s\| s\.is_some\(\)\)/);
  });

  it('the shell re-asks, so a service ended elsewhere reaches this button', () => {
    // `loadServiceLock` used to run once at mount and after the two commands
    // that change it. Library → History ends services too.
    const poll = APP.slice(APP.indexOf('shedTimer = setInterval'), APP.indexOf('}, 5000);'));
    expect(poll).toContain('loadServiceLock()');
  });

  it('amber is the STATE, not the button', () => {
    // docs/REBRAND.md §1 calls it amber; CLAUDE.md rule 18 says amber IS on air
    // and nothing else. Both hold because the only time it burns amber is the
    // time a service really is recording.
    const css = read('src/app.css');
    expect(css).toMatch(/\.r-cbtn\.endsvc\[data-on="1"\]\{[^}]*--v-amber-soft/);
    // No unconditional amber on the class itself.
    expect(css).not.toMatch(/\.r-cbtn\.endsvc\{[^}]*amber/);
    // And green never appears: Relay has no Go Live, and green is not in the law.
    expect(card()).not.toContain('golive');
  });
});

describe('the End service button says which of its two states it is in', () => {
  let host;
  const settle = () => new Promise((r) => setTimeout(r, 0));
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async () => null);
  });
  afterEach(() => {
    host?.remove();
    host = null;
  });
  async function mount() {
    const { default: Dock } = await import('./Dock.svelte');
    host = document.createElement('div');
    document.body.appendChild(host);
    new Dock({ target: host });
    for (let i = 0; i < 4; i++) await settle();
  }
  const btn = () => [...host.querySelectorAll('.r-cbtn')].find((b) => /End service|No service/.test(b.textContent));

  it('with nothing recording it says so and refuses to be pressed', async () => {
    const cap = await import('./stores/capture.js');
    cap.capture.update((s) => ({ ...s, available: true }));
    cap.serviceLock.set({ engaged: false, held_back: [], recording: false });
    await mount();
    expect(btn().textContent.trim()).toBe('No service');
    expect(btn().disabled).toBe(true);
    expect(btn().dataset.on).toBe('0');
    cap.capture.update((s) => ({ ...s, available: false }));
  });

  it('with a service open it offers to end it, and pressing it reaches end_service', async () => {
    const cap = await import('./stores/capture.js');
    cap.capture.update((s) => ({ ...s, available: true }));
    cap.serviceLock.set({ engaged: true, held_back: [], recording: true });
    await mount();
    expect(btn().textContent.trim()).toBe('End service');
    expect(btn().disabled).toBe(false);
    expect(btn().dataset.on).toBe('1');
    btn().click();
    for (let i = 0; i < 4; i++) await settle();
    expect(invoke.mock.calls.map((c) => c[0])).toContain('end_service');
    cap.serviceLock.set({ engaged: false, held_back: [], recording: false });
    cap.capture.update((s) => ({ ...s, available: false }));
  });

  // ── THE REFUSAL, ON THE SURFACE THAT NOW OFFERS THE CONTROL ───────────────
  //
  // `end_service` really can fail: it takes `session.0.lock()?`, so a thread that
  // panicked holding the session mutex leaves it poisoned. `endservice.test.js`
  // holds that the wrapper THROWS; neither it nor the state tests above can see
  // what the surface does with the throw, and the original defect was entirely on
  // that side — `await endService(); refresh();` repainted an identical list under
  // an identical button, which is as close to a claim of success as a screen gets
  // without words (CLAUDE.md rule 15).
  //
  // This test used to press History's copy of the button. There is only one copy
  // now and it is this one, so the test came with it rather than being deleted:
  // `Dock.svelte::run` is the door every dock action goes through, and a guarantee
  // is only kept on the doors you checked.
  it('a refused End service says so on the surface that offers it', async () => {
    const cap = await import('./stores/capture.js');
    cap.capture.update((s) => ({ ...s, available: true }));
    cap.serviceLock.set({ engaged: true, held_back: [], recording: true });
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'end_service') throw { kind: 'refused', message: 'A service is being recorded.' };
      return null;
    });
    await mount();
    expect(host.querySelector('[role="alert"]')).toBe(null);
    btn().click();
    for (let i = 0; i < 6; i++) await settle();
    const alert = host.querySelector('[role="alert"]');
    expect(alert, 'a refused end reported nothing at all').toBeTruthy();
    expect(alert.textContent).toMatch(/A service is being recorded/);
    cap.serviceLock.set({ engaged: false, held_back: [], recording: false });
    cap.capture.update((s) => ({ ...s, available: false }));
  });

  it('a lock the operator lifted does not take the button with it', async () => {
    // The whole reason `recording` is not `engaged`.
    const cap = await import('./stores/capture.js');
    cap.capture.update((s) => ({ ...s, available: true }));
    cap.serviceLock.set({ engaged: false, held_back: [], recording: true });
    await mount();
    expect(btn().textContent.trim()).toBe('End service');
    expect(btn().disabled).toBe(false);
    cap.serviceLock.set({ engaged: false, held_back: [], recording: false });
    cap.capture.update((s) => ({ ...s, available: false }));
  });
});

// ── §2 · THE STATUS BAR'S CELLS, AGAINST THE PROTOTYPE'S ROW ────────────────
describe("§2 · the strip carries the prototype's row, and says where it differs", () => {
  const bar = () => APP.slice(APP.indexOf('<footer class="footer-v"'), APP.indexOf('</footer>'));

  it("every cell the prototype has, in the prototype's order", () => {
    // The prototype: state · On air · Latency p50 · Dropped · Model · Cadence ·
    // push · Screens · Local offline.
    const keys = [...bar().matchAll(/<span class="k">([^<]+)<\/span>/g)].map((m) => m[1]);
    expect(keys.slice(0, 5)).toEqual(['On air', 'Latency p50', 'Dropped', 'Model', 'Cadence']);
    expect(keys).toContain('Screens');
    // …and the state sentence leads, with no key of its own, as it does there.
    expect(bar().indexOf('{wall.words}')).toBeLessThan(bar().indexOf('On air'));
  });

  it('`LOCAL offline` is replaced by a fact, not copied as a constant', () => {
    // The prototype's last cell is a literal. Relay has no honest equivalent —
    // it is offline-first by design, so "offline" is not news — and the slot is
    // worth the one thing an operator cannot otherwise see: whether the console
    // can still reach the engine. It is re-asked on the same poll, so it detects
    // its own recovery as well as its own failure (rule 35, both directions).
    expect(bar()).toContain('Engine');
    expect(bar()).not.toContain('>Local<');
    expect(APP).toMatch(/engineOnline = await ping\(\)/);
  });
});
