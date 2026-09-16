// THE TIMER RUNS BARE — who supplies the words beside the clock.
//
// `Dock.svelte` hard-coded them. Every countdown an operator started from the
// console said "Service begins in", and "Welcome" at zero, and no interface
// anywhere in Relay could type anything else. The words are PAYLOAD, not
// template: they ride in `content.reference` (`main::start_countdown`) and a
// template's `reference`-bound layer is what draws them. A surface with no field
// for them therefore has nothing to say, and now says nothing.
//
// The constant lived behind four doors, which is why this file has four parts —
// this repository's recurring bug is a rule kept on one door and skipped on its
// twin, and three of these four are twins of the first:
//
//   1. the dock's Start, which named both constants outright;
//   2. `capture.js`'s own defaults, which handed them to any caller that omitted
//      them — so removing them from the dock alone would have changed nothing;
//   3. the Planner, which wrote the same two constants into every countdown cue
//      with no control anywhere to edit them;
//   4. Live, which fired a cue's words through `p.label || 'Service begins in'`
//      — so a cue whose operator deliberately left the words blank got the
//      constant back on the way to the wall, from a fourth copy.
//
// What this file does NOT touch, and what the sixteen countdown tests still own:
// how long is left (`countdown.js::countdownRemainingMs`), the transport's
// arbitration (`countdownwiring.test.js`), and the renderer's behaviour at zero.
//
// Each test below is annotated with the defect that reproduces it.
//
//   npx vitest run src/lib/timerwords.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');

// The same self-detecting gate `surface.test.js` and `plannerbuildonly.test.js`
// use: without `resolve: { conditions: ['browser'] }` in `vitest.config.js`,
// `onMount` is a literal empty function and a mounted component fetches nothing.
// These skip loudly rather than pass over a surface that never loaded.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const settle = async (ms = 20) => {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
};

const startArgs = () => invoke.mock.calls.find((c) => c[0] === 'start_countdown')?.[1];

// ── 1 · THE STORE ───────────────────────────────────────────────────────────
//
// Reintroduce by restoring the defaults at `capture.js`'s `startCountdown`:
//   label = 'Service begins in', doneMsg = 'Welcome'
describe('the store invents no words for a caller that supplies none', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(null);
    cap.live.set(null);
  });

  it('an omitted label and done message reach the engine as nothing at all', async () => {
    await cap.startCountdown(5);
    expect(startArgs()).toEqual({ minutes: 5, label: '', doneMsg: '', templateId: null });
  });

  // The other half, and the reason the defaults are emptied rather than the
  // parameters removed: a caller that HAS words still gets to say them, and the
  // engine still receives them verbatim.
  it('and words a caller does supply are passed through untouched', async () => {
    await cap.startCountdown(3, 'Doors open in', 'Please come in', 7);
    expect(startArgs()).toEqual({
      minutes: 3,
      label: 'Doors open in',
      doneMsg: 'Please come in',
      templateId: 7,
    });
  });
});

// ── 2 · THE DOCK ────────────────────────────────────────────────────────────
//
// Reintroduce by putting the two constants back at `Dock.svelte`'s `press`:
//   startCountdown(r.broadcastMs / 60_000, 'Service begins in', 'Welcome')
describe('the dock starts a bare timer', () => {
  let host;
  let app;

  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_templates') return Promise.resolve([]);
      return Promise.resolve(null);
    });
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

  itMounted('Start asks for digits alone — the console has no words to give', async () => {
    const Dock = (await import('./Dock.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Dock({ target: host, props: {} });
    await settle();

    const start = [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Start');
    expect(start, 'the dock draws a Start button').toBeTruthy();
    expect(start.disabled).toBe(false);
    start.click();
    await settle();

    const args = startArgs();
    expect(args, 'Start reached `start_countdown`').toBeTruthy();
    expect(args.label, 'the console supplied words it has no field for').toBe('');
    expect(args.doneMsg, 'the console supplied a done message it has no field for').toBe('');
  });
});

// ── 3 · THE PLANNER ─────────────────────────────────────────────────────────
//
// Reintroduce by deleting the two fields from the add row and restoring
// `ServicePlanner.svelte`'s `addCountdownCue` payload to
//   { minutes: m, label: 'Service begins in', done: 'Welcome' }
describe('the Planner is the one place words beside the clock are typed', () => {
  const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-21', cue_count: 0 };
  let host;
  let app;

  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd) => {
      switch (cmd) {
        case 'list_plans':
          return Promise.resolve([PLAN]);
        case 'plan_items':
          return Promise.resolve([]);
        case 'list_templates':
          return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
        default:
          return Promise.resolve([]);
      }
    });
    cap.capture.update((c) => ({ ...c, available: true }));
  });

  afterEach(() => {
    app?.$destroy();
    host?.remove();
    app = host = null;
    document.body.innerHTML = '';
  });

  /** The input a visible `<label for>` names — a placeholder is not a name. */
  const namedField = (text) => {
    const label = [...host.querySelectorAll('label')].find(
      (l) => l.textContent.trim().toLowerCase() === text.toLowerCase(),
    );
    return label ? host.querySelector(`#${label.getAttribute('for')}`) : null;
  };

  const type = async (el, value) => {
    el.value = value;
    el.dispatchEvent(new Event('input'));
    await settle();
  };

  const openAdd = async () => {
    const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new ServicePlanner({ target: host, props: {} });
    for (let i = 0; i < 50 && !host.querySelector('.sp-cdadd'); i += 1) {
      const add = [...host.querySelectorAll('button')].find((b) => b.textContent.includes('Add cue'));
      add?.click();
      await settle();
    }
    expect(host.querySelector('.sp-cdadd'), 'the add panel draws the countdown row').toBeTruthy();
  };

  const addCue = async () => {
    const go = [...host.querySelectorAll('button')].find(
      (b) => b.closest('.sp-cdadd') && b.textContent.includes('Add'),
    );
    expect(go, 'the countdown row draws its own Add').toBeTruthy();
    go.click();
    await settle(30);
    const call = invoke.mock.calls.find((c) => c[0] === 'add_plan_item');
    expect(call, 'the countdown row reached `add_plan_item`').toBeTruthy();
    return JSON.parse(call[1].payloadJson);
  };

  itMounted('draws two named fields for them, and writes what was typed', async () => {
    await openAdd();
    const label = namedField('Words above the clock');
    const done = namedField('Words at zero');
    expect(label, 'no named field for the words above the clock').toBeTruthy();
    expect(done, 'no named field for the words at zero').toBeTruthy();

    await type(label, '  Doors open in  ');
    await type(done, 'Please come in');
    const payload = await addCue();
    expect(payload.label).toBe('Doors open in');
    expect(payload.done).toBe('Please come in');
  });

  // A cue nobody typed words into carries none. This is the half that fails
  // against today's code loudest: the constants were written whether or not
  // anybody wanted them, and there was no way to unwrite them.
  itMounted('and writes no words into a cue nobody typed any into', async () => {
    await openAdd();
    const payload = await addCue();
    expect(payload.label).toBe('');
    expect(payload.done).toBe('');
    expect(payload.minutes).toBeGreaterThan(0);
  });
});

// ── 4 · LIVE ────────────────────────────────────────────────────────────────
//
// Reintroduce by restoring the fallbacks at `Live.svelte`'s countdown branch:
//   p.label || 'Service begins in',  p.done || 'Welcome'
// The first test below goes on passing against that defect — it is the cue WITH
// words, and a fallback never fires. The second is the one that reproduces it.
describe('Live fires the cue’s own words, and invents none', () => {
  const CHANNEL = { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 1 };
  const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-21', cue_count: 1 };
  let host;
  let app;
  let sess;

  const cue = (payload) => ({
    id: 11,
    plan_id: 1,
    cue_type: 'countdown',
    label: 'Countdown · 5 min',
    payload_json: JSON.stringify(payload),
    section_title: null,
    duration_sec: 300,
    template_id: null,
  });

  const mountLive = async (payload) => {
    const { BUILTINS } = await import('./templates.js');
    invoke.mockReset();
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') return Promise.resolve([CHANNEL]);
      if (cmd === 'list_templates') return Promise.resolve([BUILTINS[0]]);
      if (cmd === 'list_plans') return Promise.resolve([PLAN]);
      if (cmd === 'plan_items') return Promise.resolve([cue(payload)]);
      if (cmd === 'list_books') return Promise.resolve([{ book: 'Psalms', chapters: 150 }]);
      if (cmd === 'rehearsal') return Promise.resolve(false);
      if (cmd === 'get_sensitivity') return Promise.resolve(50);
      return Promise.resolve(null);
    });
    cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
    cap.live.set(null);
    cap.detections.set([]);
    cap.resolvedDetections.set([]);
    cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
    cap.channelHealth.set({});
    sess = await import('./session.js');
    sess.setSession({ planId: 1, liveCueId: null, liveSlide: 0, liveOnAir: false });
    const Live = (await import('./views/Live.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Live({ target: host, props: {} });
    await settle(160);
    const cell = host.querySelector('.sg-cell');
    expect(cell, 'the countdown cue draws a slide').toBeTruthy();
    cell.click();
    // The grid's 190ms arbitration beat — a single press only reaches the fire
    // path once the window for a double has closed (`slidegrid.js`).
    await settle(320);
    return startArgs();
  };

  afterEach(() => {
    app?.$destroy();
    host?.remove();
    app = host = null;
    sess?.setSession({ planId: null, liveCueId: null, liveSlide: 0, liveOnAir: false });
  });

  itMounted('a cue that asked for words fires those words', async () => {
    const args = await mountLive({ minutes: 5, label: 'Doors open in', done: 'Please come in' });
    expect(args, 'the cue reached `start_countdown`').toBeTruthy();
    expect(args.label).toBe('Doors open in');
    expect(args.doneMsg).toBe('Please come in');
  });

  itMounted('a cue that asked for none fires a bare timer', async () => {
    const args = await mountLive({ minutes: 5, label: '', done: '' });
    expect(args, 'the cue reached `start_countdown`').toBeTruthy();
    expect(args.label, 'Live put a word back over a label the operator cleared').toBe('');
    expect(args.doneMsg, 'Live put a word back over a done message the operator cleared').toBe('');
  });
});
