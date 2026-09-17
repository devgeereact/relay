// THE WAY BACK ONTO A CONGREGATION SCREEN — RG-152.
//
// A timer now outlives the content that replaced it, which is the whole point of
// the registry: after a reading there is something to go back to. `show_timer` is
// how an operator goes back to it, and for the whole of wave 3 nothing rendered
// could ask. The wrappers existed and were pinned in the THROWS group, which is
// exactly the distinction CLAUDE.md draws and RG-21 was filed on: **the test is
// whether a RENDERED control can get there, not whether a wrapper exists.**
//
// The operator's decision (2026-09-17, `docs/superpowers/plans/2026-09-16-wave3-
// timers.md`) puts that control in the dock's Countdown block rather than in Live's
// programme band, because the dock is on every workspace and the band is on one.
//
// WHAT THESE TESTS ARE FOR, worst first:
//
//   1. THE CONTROL MAY NOT LIE ABOUT WHAT IS ON THE SCREENS (rule 35). Its state
//      comes from the same fact `main::adjust_countdown` reads before it decides
//      whether to repaint — `channels::live_content`, mirrored here as `$live`,
//      through the same three-armed question. The drift this would otherwise take
//      is specific and is tested by name: a countdown that has RUN OUT is still on
//      the screens, and the transport's own `cdRunning` calls it null.
//   2. IT MAY NOT CREATE A TIMER. Start is the one control that puts a countdown
//      in front of people for the first time; this one can only return to a timer
//      that already exists, and it is never offered for a `Stage` timer, which the
//      engine refuses in words anyway.
//   3. A BROKEN READ MAY NOT READ LIKE A QUIET SUNDAY. `list_timers` throws on
//      purpose (contract group 1); silence over a failed read says exactly what
//      silence over an empty registry says, which is rule 35's own failure.
//
//   npx vitest run src/lib/wayback.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const { isCountdownContent, wayBack } = await import('./countdown.js');
const Dock = (await import('./Dock.svelte')).default;
const src = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

/** A registry row in the shape `list_timers` hands back (`TimerView`). */
const timer = (over = {}) => ({
  id: 12,
  label: 'Service begins in',
  done_msg: 'Welcome',
  target_ms: Date.now() + 300_000,
  from_ms: Date.now(),
  paused_ms: null,
  warn_ms: null,
  scope: 'both',
  plan_item_id: null,
  remaining_ms: 300_000,
  ...over,
});

const A_VERSE = { kind: 'scripture', reference: 'John 3:16', verse: 'For God so loved…' };

let host;
let app;

function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Dock({ target: host, props: {} });
  return host;
}

async function settle(ms = 10) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

/**
 * MOUNT, AND WAIT UNTIL THE REGISTRY HAS ACTUALLY BEEN READ.
 *
 * The dock reads `list_timers` on mount and then every two seconds, and in THIS
 * ENVIRONMENT the mount-time read does not reach the mocked bridge: `capture.js`
 * resolves the Tauri core through a dynamic `import()`, and when several wrappers
 * issue one in the same mount, only the first is served the mocked module — the
 * rest get the real one and throw `window.__TAURI_INTERNALS__ is undefined`. That
 * is an artefact of the test runner and not of the product (in the app the module
 * is already loaded), but it is worth writing down: it is also why nothing has
 * ever asserted the dock's OWN mount-time `loadTemplates` call.
 *
 * So the poll is what these tests watch, advanced with fake timers rather than
 * waited out in real seconds. `shouldAdvanceTime` keeps the microtask queue real,
 * which is what lets the awaits inside the component resolve.
 */
async function mountAndRead() {
  vi.useFakeTimers({ shouldAdvanceTime: true });
  mount();
  await vi.advanceTimersByTimeAsync(2100);
  await tick();
  vi.useRealTimers();
  await settle();
  return host;
}

const called = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);
const byLabel = (text) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === text);
const putBack = () => byLabel('Put back on screens');

/** `list_timers` answers `rows`; everything else the dock asks on mount answers null. */
function registry(rows) {
  invoke.mockImplementation(async (cmd) => (cmd === 'list_timers' ? rows : null));
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(null);
  cap.live.set(null);
  cap.stageAlert.set(null);
  cap.capture.update((s) => ({ ...s, available: true }));
  cap.templates.set([]);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

// ── THE PURE HALF ───────────────────────────────────────────────────────────
//
// `countdown.js` is the decision layer for this transport and it stays the only
// arithmetic on this side of the bridge. These two functions are the frontend
// statement of what `main.rs` already asks itself — one rule, stated once on each
// side, never twice on one.
describe('is a countdown what is on the screens', () => {
  it('asks the engine’s own three-armed question, arm for arm', () => {
    expect(isCountdownContent({ countdown_to: Date.now() + 1000 })).toBe(true);
    expect(isCountdownContent({ countdown_paused_ms: 90_000 })).toBe(true);
    expect(isCountdownContent({ kind: 'countdown' })).toBe(true);
    expect(isCountdownContent(A_VERSE)).toBe(false);
    expect(isCountdownContent(null)).toBe(false);
  });

  // THE DRIFT THIS EXISTS TO PREVENT. The transport's `countdownRemaining` calls a
  // countdown that has run out null, deliberately — ±1 cannot re-aim it. A way back
  // that asked THAT question would offer to put a countdown back while it is still
  // on the wall at 0:00, which is a control lying about what is on the screens.
  it('a countdown that has RUN OUT is still what is on the screens', () => {
    const finished = { kind: 'countdown', countdown_to: Date.now() - 60_000 };
    cap.live.set(finished);
    expect(cap.countdownRemaining()).toBe(null); // the transport's reading
    expect(isCountdownContent(finished)).toBe(true); // the engine's
  });
});

describe('what the block may say about the way back', () => {
  it('says nothing at all until the registry has been read once', () => {
    expect(wayBack(null, A_VERSE).state).toBe('unknown');
    expect(wayBack(undefined, A_VERSE).state).toBe('unknown');
  });

  it('offers nothing when the registry holds no congregation timer', () => {
    expect(wayBack([], A_VERSE).state).toBe('none');
    expect(wayBack([timer({ scope: 'stage' })], A_VERSE).state).toBe('none');
  });

  it('offers the way back when a Both timer exists and a verse is on the screens', () => {
    const r = wayBack([timer()], A_VERSE);
    expect(r.state).toBe('offered');
    expect(r.timer.id).toBe(12);
  });

  it('offers nothing while the countdown is already on the screens', () => {
    expect(wayBack([timer()], { kind: 'countdown', countdown_to: Date.now() + 1000 }).state).toBe(
      'showing',
    );
    expect(wayBack([timer()], { countdown_paused_ms: 120_000 }).state).toBe('showing');
  });

  // The engine's transport means "the newest `Both` timer" (`newest_congregation_
  // timer`), and the list arrives oldest first. Two would be a mistake — `start_
  // countdown` takes the one before it — but if a later track ever allows two, the
  // console must mean the same one the engine means.
  it('means the newest congregation timer, exactly as the engine does', () => {
    const r = wayBack(
      [timer({ id: 4 }), timer({ scope: 'stage', id: 5 }), timer({ id: 9 })],
      A_VERSE,
    );
    expect(r.timer.id).toBe(9);
  });
});

// ── THE RENDERED HALF — the only half RG-152 was about ──────────────────────
describe('the control in the dock’s Countdown block', () => {
  it('reaches show_timer with the timer’s own id', async () => {
    registry([timer({ id: 12 })]);
    cap.live.set(A_VERSE);
    await mountAndRead();

    expect(putBack(), 'no rendered control reaches show_timer').toBeTruthy();
    putBack().click();
    await settle();

    expect(called('show_timer')).toHaveLength(1);
    expect(called('show_timer')[0][1]).toMatchObject({ timerId: 12 });
  });

  // START IS THE ONE CONTROL THAT PUTS A COUNTDOWN IN FRONT OF PEOPLE FOR THE
  // FIRST TIME. This one returns to a timer that exists and can do nothing else.
  it('creates nothing — no timer, no countdown', async () => {
    registry([timer()]);
    cap.live.set(A_VERSE);
    await mountAndRead();
    invoke.mockClear();
    putBack().click();
    await settle();
    expect(called('start_timer')).toHaveLength(0);
    expect(called('start_countdown')).toHaveLength(0);
    expect(called('adjust_countdown')).toHaveLength(0);
  });

  it('is not offered while the countdown is on the screens — including at 0:00', async () => {
    registry([timer()]);
    cap.live.set({ kind: 'countdown', countdown_to: Date.now() + 120_000 });
    await mountAndRead();
    expect(putBack()).toBeUndefined();

    // Run out, and still on the wall. The transport reads null here and the way
    // back must not take that for "the screens are showing something else".
    cap.live.set({ kind: 'countdown', countdown_to: Date.now() - 60_000 });
    await settle();
    expect(putBack(), 'offered over a countdown that is still on the wall').toBeUndefined();
  });

  // `show_timer` refuses a `Stage` timer in words, and a control that has to be
  // refused is a control that should not have been offered.
  it('is never offered for a programme timer', async () => {
    registry([timer({ id: 77, scope: 'stage' })]);
    cap.live.set(A_VERSE);
    await mountAndRead();
    expect(putBack()).toBeUndefined();
    expect(called('show_timer')).toHaveLength(0);
  });

  it('is not offered before the registry has answered', async () => {
    invoke.mockImplementation(() => new Promise(() => {})); // never answers
    cap.live.set(A_VERSE);
    mount();
    await settle();
    expect(putBack()).toBeUndefined();
  });

  // RULE 35, asked the way that rule asks it: what does this block say when the
  // thing behind it is broken? Not the same as when everything is fine.
  it('says the read failed rather than falling silent like an empty registry', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_timers') throw new Error('the timer registry is poisoned');
      return null;
    });
    cap.live.set(A_VERSE);
    await mountAndRead();
    expect(putBack()).toBeUndefined();
    expect(host.textContent).toContain('Cannot tell whether a countdown is waiting');
  });

  // The other half of the same claim, and the half that makes it a claim at all:
  // an EMPTY registry says nothing, so the sentence above cannot be what this
  // block says whatever happens.
  it('and says nothing of the kind over a registry that is simply empty', async () => {
    registry([]);
    cap.live.set(A_VERSE);
    await mountAndRead();
    expect(putBack()).toBeUndefined();
    expect(host.textContent).not.toContain('Cannot tell whether a countdown is waiting');
  });

  it('reports a refused put-back instead of claiming the countdown is back', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_timers') return [timer()];
      if (cmd === 'show_timer') throw new Error('That timer is not running.');
      return null;
    });
    cap.live.set(A_VERSE);
    await mountAndRead();
    putBack().click();
    await settle();
    expect(host.querySelector('.derr')?.textContent ?? '').toContain('not running');
    // And it is still offered, because nothing went back on any screen.
    expect(putBack()).toBeTruthy();
  });
});

describe('where it lives', () => {
  it('is inside the Countdown block, not a fourth thing in Quick tools', () => {
    const card = src.slice(src.indexOf('<span class="dk">Quick tools</span>'));
    const block = card.slice(
      card.indexOf('<div class="qblock tmr">'),
      card.indexOf('<span class="r-lbl">Name band</span>'),
    );
    expect(block).toContain('Put back on screens');
    // `quicktools.test.js` pins the card at three blocks on an operator
    // instruction, and this control is inside one of the three rather than a
    // fourth beside them.
    const body = card.slice(0, card.indexOf('<span class="dk">Controls</span>'));
    expect(body.match(/<div class="qblock/g) ?? []).toHaveLength(3);
  });

  // The Controls card never scrolls, because an operator may never have to scroll
  // to reach `Clear screens`. Nothing here is in that card.
  it('adds nothing to the Controls card', () => {
    // MARKUP ONLY. The stylesheet below it explains the row's height budget and
    // names the control while doing so, and a scanner that reads the prose about
    // a rule instead of the rule passes and fails for the wrong reasons.
    const markup = src.slice(0, src.indexOf('<style>'));
    const controls = markup.slice(markup.indexOf('<span class="dk">Controls</span>'));
    expect(controls).not.toContain('Put back on screens');
  });
});
