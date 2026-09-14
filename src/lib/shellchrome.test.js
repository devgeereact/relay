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

  it('neither Themes nor Help takes a slot in it', () => {
    expect(stripKeys()).not.toContain('themes');
    expect(stripKeys()).not.toContain('help');
  });

  it('but Help is still a real route with a view behind it', () => {
    // A surface nothing can reach is an orphan. Settings renders two controls
    // that set this tab, and the resolver is handed `routes` (not the strip) so
    // they land rather than bouncing back to Live.
    expect(APP).toMatch(/const routes = \[\.\.\.tabs\.map\(\(x\) => x\.key\), 'help'\]/);
    expect(APP).toMatch(/resolveActiveTab\(\$session\.activeTab, routes\)/);
    expect(APP).toMatch(/help:\s*\(\) => import\('\.\/lib\/views\/Help\.svelte'\)/);
    expect(read('src/lib/views/Settings.svelte')).toMatch(/activeTab: 'help'/);
  });

  it('the Themes desk is mounted inside the Templates workspace', () => {
    // Folded, not deleted. Both galleries and both editors are still rendered —
    // by one router now instead of two.
    const TPL = read('src/lib/views/Templates.svelte');
    for (const child of ['TemplateGallery', 'TemplateEditor', 'ThemeGallery', 'ThemeEditor']) {
      expect(TPL, `${child} must still be rendered by something`).toContain(`<${child}`);
    }
    // And the switch between the two desks is a real control, not a dead branch.
    // The strip itself is `DeskStrip`, rendered by BOTH galleries and owned by
    // neither; this router hears its event and writes the choice to the session,
    // so a reload puts the operator back on the desk they were on. Two strips
    // would be two answers to "which desk am I on", which is why the router
    // renders none of its own.
    expect(TPL).toMatch(/on:desk=\{changeDesk\}/);
    expect(TPL).toMatch(/setSession\(\{ templatesDesk: e\.detail\.desk \}\)/);
    expect(read('src/lib/views/templates/TemplateGallery.svelte')).toMatch(/<DeskStrip desk="templates" on:desk/);
    expect(read('src/lib/views/themes/ThemeGallery.svelte')).toMatch(/<DeskStrip desk="themes" on:desk/);
  });
});

// ── §2 · THE SCREEN LAMPS ───────────────────────────────────────────────────
//
// Rule 35, and RG-01 is the instance it was written from: Live's Output Status
// pane derived every badge from GLOBAL state, so a kiosk source that had gone
// away still read On Air on the one surface an operator watches. A second set of
// lamps in the chrome deriving its own verdict would be that bug, again, one
// strip higher — and this time on every tab.
describe('§2 · the chrome lamps are never a second opinion about a screen', () => {
  const lampBlock = () => APP.slice(APP.indexOf('$: screenLamps'), APP.indexOf('$: screens ='));

  it('every lamp is a describeScreen verdict', () => {
    expect(lampBlock()).toContain('describeScreen(');
  });

  it('and no lamp is derived from global state', () => {
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
    // lamps it sits beside, which is the same failure in arithmetic.
    expect(APP).toMatch(/\$: screens = screenTally\(screenLamps\)/);
    expect(APP).toMatch(/\{screens\.live\} of \{screens\.total\}/);
  });

  it('a lamp wears only the colour law', () => {
    // amber = on air, amethyst = rehearsal, red = not responding, grey = idle.
    // No fifth colour, and in particular no green "ok": green is not in the law.
    const map = APP.slice(APP.indexOf('const LAMP_TONE'), APP.indexOf('const lampWord'));
    expect(map).toContain("onair: 'amber'");
    expect(map).toContain("rehearsal: 'amethyst'");
    expect(map).toContain("down: 'red'");
    expect(map).not.toMatch(/green|emerald/i);
  });

  it('a long screen name is ELLIPSED, not hard-clipped', () => {
    // Measured at 1440×960: `Streaming` lost its last letter with nothing to say
    // it had been cut, because `text-overflow` is a property of a block container
    // and the lamp row is an inline-flex — the text child sat in an anonymous
    // flex item and the declaration did nothing. A clipped name reads as a
    // different screen; an ellipsis reads as a long one.
    expect(APP).toMatch(/<span class="signm">\{lampWord\(sc\.name\)\}<\/span>/);
    const css = read('src/app.css');
    const rule = css.slice(css.indexOf('.siglamps .signm{'), css.indexOf('}', css.indexOf('.siglamps .signm{')));
    expect(rule).toContain('display:block');
    expect(rule).toContain('text-overflow:ellipsis');
  });

  it('a long screen name truncates rather than wrapping the chrome', () => {
    // The bar is 34px. A second row of lamps pushes the whole desk — and the
    // slide grid, which is the job — down by a row.
    const css = read('src/app.css');
    const rule = css.slice(css.indexOf('.siglamps .sig{'), css.indexOf('}', css.indexOf('.siglamps .sig{')));
    expect(rule).toContain('white-space:nowrap');
    expect(rule).toContain('text-overflow:ellipsis');
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

// ── THE CHROME'S RIGHT-HAND END ─────────────────────────────────────────────
describe('the chrome carries facts and a panic control, and no decoration', () => {
  it('Emergency Stop is still there and still reaches clear_screens', () => {
    // A panic control lives at a fixed screen corner an operator can hit without
    // reading (rule 15, DECISIONS §20). It is the one thing in this bar that may
    // never move to make room for something else.
    expect(APP).toMatch(/<button class="r-btn danger sm" on:click=\{clearScreens\}[^>]*>Emergency Stop<\/button>/);
  });

  it('and it is the LAST thing in the bar', () => {
    const chrome = APP.slice(APP.indexOf('<header class="topbar-v">'), APP.indexOf('</header>'));
    expect(chrome.indexOf('Emergency Stop')).toBeGreaterThan(chrome.indexOf('siglamps'));
  });

  it('the two decorative icons are gone', () => {
    // A drawn "Signal" glyph wired to nothing, and a clock face beside a clock.
    // In a room whose whole premise is that an indicator means something, a
    // picture of an indicator is the defect drawn rather than written.
    const chrome = APP.slice(APP.indexOf('<header class="topbar-v">'), APP.indexOf('</header>'));
    expect(chrome).not.toContain('title="Signal"');
    expect(chrome).not.toContain('class="topbar-icons"');
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

  it('Clear screens is full width and first', () => {
    const card = DOCK.slice(DOCK.indexOf('<div class="dbody ctlbody">'));
    const order = ['Clear screens', 'Blackout', 'Rehearse'];
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

  it('with no engine attached the dial is disabled and claims no value', async () => {
    // `getSensitivity` swallows and returns 50 with no backend, and 50 is also a
    // perfectly ordinary real setting — so the reading alone cannot tell the two
    // apart. On the one control that governs what the AI may put on a wall
    // unasked, a plausible number nobody set is rule 35 with a dial on it.
    await mount();
    const dial = host.querySelector('input[aria-label="Detection sensitivity"]');
    expect(dial.disabled).toBe(true);
    expect(host.querySelector('.sensv').textContent.trim()).toBe('—');
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
