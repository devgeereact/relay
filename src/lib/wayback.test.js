// THE WAY BACK ONTO A CONGREGATION SCREEN — RG-152.
//
// A timer now outlives the content that replaced it, which is the whole point of
// the registry: after a reading there is something to go back to. `show_timer` is
// how an operator goes back to it, and for the whole of wave 3 nothing rendered
// could ask. The wrappers existed and were pinned in the THROWS group, which is
// exactly the distinction CLAUDE.md draws and RG-21 was filed on: **the test is
// whether a RENDERED control can get there, not whether a wrapper exists.**
//
// The operator's decision (2026-09-17, `docs/archive/2026-09-16-wave3-
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
const { setSession } = await import('./session.js');
const { isCountdownContent, wayBack } = await import('./countdown.js');
// ── WHICH SURFACE CARRIES THE WAY BACK (2026-09-20) ─────────────────────────
//
// It was inside the dock's Countdown block, and the 2026-09-17 note beside it
// said why: the dock is in the SHELL and renders on every workspace, so a way
// back onto a congregation screen that an operator has to change workspace to
// reach is a way back they will not find during a service.
//
// The operator then asked for the Screen Countdown out of Quick tools, and the
// way back went with the instrument rather than being stranded in a card that no
// longer has a countdown in it. The 2026-09-17 argument is not wrong and is now
// the recorded price of the move — `views/Live.svelte`, above `cdPress`.
//
// WHAT DID NOT CHANGE is everything this file is about: `showTimer` is still the
// one door, it still creates nothing, the offer is still decided by `wayBack`
// from two facts, and a failed read still says so instead of falling silent.
const Live = (await import('./views/Live.svelte')).default;
const src = readFileSync(resolve(process.cwd(), 'src/lib/views/Live.svelte'), 'utf8');
const dockSrc = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

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
  app = new Live({ target: host, props: {} });
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
 * The run surface reads `list_timers` on mount and then every two seconds, and in
 * THIS ENVIRONMENT the mount-time read does not reach the mocked bridge: `capture.js`
 * resolves the Tauri core through a dynamic `import()`, and when several wrappers
 * issue one in the same mount, only the first is served the mocked module — the
 * rest get the real one and throw `window.__TAURI_INTERNALS__ is undefined`. That
 * is an artefact of the test runner and not of the product (in the app the module
 * is already loaded), but it is worth writing down: it is also why nothing has
 * ever asserted the surface's OWN mount-time `loadTemplates` call.
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

/** `list_timers` answers `rows`; the rest of what the surface asks on mount is
 *  answered with the empty shape each reader expects, so nothing this file is
 *  not about renders an error over the band it is watching. */
function registry(rows) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_timers') return rows;
    if (cmd === 'list_output_channels') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'list_plans') return [];
    if (cmd === 'list_books') return [{ book: 'Psalms', chapters: 150 }];
    if (cmd === 'rehearsal') return false;
    if (cmd === 'get_sensitivity') return 50;
    return null;
  });
}

beforeEach(() => {
  invoke.mockReset();
  registry([]);
  cap.live.set(null);
  cap.stageAlert.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  cap.readErrors.set({});
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.templates.set([]);
  setSession({ planId: null });
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.readErrors.set({});
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
    // `cap.countdownRemaining()` was asserted beside this until 2026-09-21: the
    // transport that read it is gone with the band (DECISIONS §115), and the
    // engine-side question below is the one that was always the claim.
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

