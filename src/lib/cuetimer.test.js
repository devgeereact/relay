// W4 · A CUE CAN CARRY A CLOCK — the binding, and the one place it is acted on.
//
// `plan_items.timer_minutes` is a request stored on a cue. Two surfaces are
// involved and they have opposite permissions, which is the whole design:
//
//   · the PLANNER writes the binding and may not start, move, stop or show a
//     timer — `plannerbuildonly.test.js` holds that, and this wave added the four
//     timer commands to the list it refuses;
//   · LIVE reads the binding and starts the clock when the cue goes on air, at
//     ONE call site, because `fireSlide` is the single door a plan cue goes on
//     air through (rule 36's reasoning, applied to a timer).
//
// Each test here was watched to fail with its change reverted; the reverts are
// named in the PR body. The dangerous ones are the two about REPEATS: a clock
// that restarts on every slide of a five-section song, and a clock that starts
// during a rehearsal and then appears on the preacher's rail the moment the
// service goes live.
//
//   npx vitest run src/lib/cuetimer.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { setSession, clearSession } = await import('./session.js');

// The same self-detecting gate `surface.test.js` and `plannerbuildonly.test.js`
// use: without `resolve: { conditions: ['browser'] }`, `onMount` is a literal
// empty function and a mounted component fetches nothing. These SKIP loudly
// rather than pass over a view that never loaded a plan.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const settle = (ms = 40) => new Promise((r) => setTimeout(r, ms));
async function until(predicate, what, tries = 60) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
    await tick();
  }
  throw new Error(`timed out waiting for: ${what}`);
}

const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-20', cue_count: 2 };
const notice = (over = {}) => ({
  id: 21,
  plan_id: 1,
  position: 0,
  cue_type: 'announce',
  label: 'Welcome',
  payload_json: JSON.stringify({ body: 'Welcome to church' }),
  template_id: null,
  section_title: '',
  duration_sec: 0,
  timer_minutes: null,
  ...over,
});
const song = (over = {}) => ({
  id: 22,
  plan_id: 1,
  position: 1,
  cue_type: 'song',
  label: 'Amazing Grace',
  payload_json: JSON.stringify({
    title: 'Amazing Grace',
    sections: [
      { tag: 'V1', label: 'Verse 1', lyrics: 'Amazing grace' },
      { tag: 'V2', label: 'Verse 2', lyrics: 'Twas grace that taught' },
    ],
  }),
  template_id: null,
  section_title: '',
  duration_sec: 0,
  timer_minutes: null,
  ...over,
});

let cues = [notice()];
let host;
let app;

beforeEach(() => {
  cues = [notice()];
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_plans':
        return Promise.resolve([PLAN]);
      case 'plan_items':
        return Promise.resolve(cues);
      case 'list_templates':
        return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
      case 'list_output_channels':
        return Promise.resolve([]);
      case 'rehearsal':
        return Promise.resolve(false);
      case 'get_sensitivity':
        return Promise.resolve(50);
      case 'start_timer':
        return Promise.resolve(7);
      case 'list_media':
      case 'list_announcements':
      case 'search_scripture':
      case 'search_songs':
      case 'list_arrangements':
      case 'list_books':
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
  cap.capture.update((c) => ({ ...c, available: true, stt: { ...c.stt, loaded: true } }));
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  cap.rehearsing.set(false);
  clearSession();
  host = document.createElement('div');
  document.body.appendChild(host);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  clearSession();
  document.body.innerHTML = '';
});

const calls = (cmd) => invoke.mock.calls.filter(([c]) => c === cmd).map(([, args]) => args);

// ─────────────────────────────────────────────────────────────────────────────
// 1 · THE PLANNER STORES THE BINDING AND STARTS NOTHING
// ─────────────────────────────────────────────────────────────────────────────
describe('the Planner binds a cue to a clock', () => {
  itMounted('the Timer control writes the binding — and takes no screen doing it', async () => {
    const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
    app = new ServicePlanner({ target: host });
    await until(() => host.querySelector('.sp-railcard'), 'the plan rail');
    host.querySelector('.sp-railcard').click();
    await until(() => host.querySelector('.sp-row'), 'the running order');
    host.querySelector('.sp-row').click();
    await until(() => host.querySelector('.sp-tmrsel'), 'the Timer control in the inspector');

    const sel = host.querySelector('.sp-tmrsel');
    // A REAL, NAMED CONTROL. A placeholder is not an accessible name, and the two
    // editable fields in this inspector were once the only unnamed controls in
    // the product.
    expect(sel.getAttribute('aria-label')).toBe('Stage Timer for this cue');
    // Unbound reads as "no timer", not as a length somebody chose.
    expect(sel.value).toBe('');

    invoke.mockClear();
    sel.value = '20';
    sel.dispatchEvent(new Event('change', { bubbles: true }));
    await until(() => calls('set_plan_timer').length > 0, 'the binding to be stored');

    expect(calls('set_plan_timer')[0]).toEqual({ id: 21, minutes: 20 });
    // AND NOTHING WAS STARTED. `start_timer` puts a clock on the preacher's rail
    // and `show_timer` is a second door onto a congregation wall; the Planner may
    // reach neither. `plannerbuildonly.test.js` holds the general form of this —
    // here it is asserted on the one control this wave added.
    const dispatched = invoke.mock.calls.map(([c]) => c);
    expect(dispatched.filter((c) => /^(start|show|adjust|stop)_timer$/.test(c))).toEqual([]);
  });

  itMounted('choosing "No timer" clears the binding rather than storing a zero', async () => {
    cues = [notice({ timer_minutes: 20 })];
    const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
    app = new ServicePlanner({ target: host });
    await until(() => host.querySelector('.sp-railcard'), 'the plan rail');
    host.querySelector('.sp-railcard').click();
    await until(() => host.querySelector('.sp-row'), 'the running order');
    host.querySelector('.sp-row').click();
    await until(() => host.querySelector('.sp-tmrsel'), 'the Timer control');

    expect(host.querySelector('.sp-tmrsel').value).toBe('20');
    invoke.mockClear();
    const sel = host.querySelector('.sp-tmrsel');
    sel.value = '';
    sel.dispatchEvent(new Event('change', { bubbles: true }));
    await until(() => calls('set_plan_timer').length > 0, 'the binding to be cleared');
    // `null`, not 0. Unbound and "bound to nothing" are the same fact and it is
    // an absence; `db/plans.rs` refuses to store a non-positive length at all.
    expect(calls('set_plan_timer')[0]).toEqual({ id: 21, minutes: null });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · LIVE IS WHAT STARTS IT, ONCE
// ─────────────────────────────────────────────────────────────────────────────
describe('Live starts a bound cue’s clock when the cue goes on air', () => {
  /** Mount Live with the plan already chosen, and press the first slide cell. */
  async function runPlanAndTakeFirstCell() {
    setSession({ planId: PLAN.id });
    const Live = (await import('./views/Live.svelte')).default;
    app = new Live({ target: host, props: {} });
    await until(() => host.querySelector('.sg-cell'), 'the slide grid to draw the plan');
    invoke.mockClear();
    host.querySelector('.sg-cell').click();
    // The press arbiter holds a single click for a beat so a double click can
    // claim it instead. 190ms, plus room.
    await settle(300);
    await tick();
  }

  itMounted('a bound cue starts a Stage Timer, named for the cue', async () => {
    cues = [notice({ timer_minutes: 25 })];
    await runPlanAndTakeFirstCell();

    const started = calls('start_timer');
    expect(started).toHaveLength(1);
    expect(started[0].minutes).toBe(25);
    expect(started[0].planItemId).toBe(21);
    expect(started[0].label).toBe('Welcome');
    // STAGE, never `both`. A cue that silently put a countdown on the
    // congregation's wall is the one mistake that cannot be taken back quietly —
    // and `show_timer`/`start_countdown` remain the only two doors onto a wall.
    expect(started[0].scope).toBe('stage');
  });

  itMounted('an UNBOUND cue starts nothing at all', async () => {
    cues = [notice()];
    await runPlanAndTakeFirstCell();
    expect(calls('start_timer')).toEqual([]);
  });

  itMounted('a zero binding is not a zero-minute clock', async () => {
    // The backend refuses to store this, so it should never arrive — but a value
    // read off a row is a value from a database, and a 0:00 clock appearing on a
    // preacher's rail is exactly what the nullable column exists to prevent.
    cues = [notice({ timer_minutes: 0 })];
    await runPlanAndTakeFirstCell();
    expect(calls('start_timer')).toEqual([]);
  });

  itMounted('walking a song does not restart its clock on every section', async () => {
    cues = [song({ timer_minutes: 5 })];
    setSession({ planId: PLAN.id });
    const Live = (await import('./views/Live.svelte')).default;
    app = new Live({ target: host, props: {} });
    await until(() => host.querySelectorAll('.sg-cell').length >= 2, 'both song sections');
    invoke.mockClear();

    host.querySelectorAll('.sg-cell')[0].click();
    await settle(300);
    await tick();
    expect(calls('start_timer')).toHaveLength(1);

    // Verse 2 of the SAME cue. The cue did not go on air again; it is still on.
    host.querySelectorAll('.sg-cell')[1].click();
    await settle(300);
    await tick();
    expect(
      calls('start_timer'),
      'a five-section song would restart the sermon clock five times',
    ).toHaveLength(1);
  });

  itMounted('a rehearsal starts no clock — there would be nobody to show it to', async () => {
    // `channels::publish_timers` suppresses a rehearsal, so a clock started here
    // is one the registry holds and nobody can see — waiting to appear on the
    // preacher's rail the moment the service goes live.
    cues = [notice({ timer_minutes: 25 })];
    cap.rehearsing.set(true);
    await runPlanAndTakeFirstCell();
    expect(calls('start_timer')).toEqual([]);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · ONE CALL SITE, AND IT STAYS ONE
// ─────────────────────────────────────────────────────────────────────────────
describe('the wiring, read off the source', () => {
  const live = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

  it('Live starts a timer from two named places, and from nowhere else', () => {
    // Four bugs in this repository have the shape "a rule kept on one door and
    // skipped on its twin". A `startTimer` beside `stepLive`, beside the grid press
    // or beside the transport would be the fifth.
    //
    // THIS ASSERTED ONE CALLER UNTIL THE CONSOLIDATION MERGE, and it was right on
    // the branch it was written on. Wave 3 Track E then landed the operator's own
    // **Start timer** control, which is a rendered, deliberate second caller and
    // the answer to RG-162. The two waves could not see each other, so the count
    // moved without either being wrong.
    //
    // The guarantee is unchanged and is what is actually checked below: every
    // caller is NAMED here, so a third one has to come through this test. A count
    // on its own would have been satisfied by any two.
    const callers = [...live.matchAll(/async function (\w+)\([^)]*\)\s*\{(?:[^]*?)\}/g)];
    const starts = [...live.matchAll(/await startTimer\(/g)];
    expect(starts).toHaveLength(2);
    // One is the cue's clock, inside `startCueTimer` and nowhere else.
    const cueBody = live.slice(
      live.indexOf('async function startCueTimer('),
      live.indexOf('async function startCueTimer(') + 900,
    );
    expect(cueBody).toMatch(/await startTimer\(/);
    // The other is the operator pressing Start timer, which publishes nothing to a
    // congregation — `startCountdown` and `showTimer` are the controls that do, and
    // neither is reachable from that handler on purpose.
    expect(live).toMatch(/await startTimer\(\{ minutes: Number\(ptMins\)/);
    expect([...live.matchAll(/startCueTimer\(/g)]).toHaveLength(2); // the definition, and its one caller
    const body = live.slice(live.indexOf('async function fireSlide('), live.indexOf('async function startCueTimer('));
    expect(body).toMatch(/await startCueTimer\(item, cueWasOnAir\);/);
  });

  it('the Planner names no timer command and imports no timer wrapper', () => {
    const planner = readFileSync(resolve(__dirname, 'views/ServicePlanner.svelte'), 'utf8');
    for (const cmd of ['start_timer', 'show_timer', 'adjust_timer', 'stop_timer']) {
      expect(planner).not.toContain(`'${cmd}'`);
    }
    for (const w of ['startTimer', 'showTimer', 'adjustTimer', 'stopTimer']) {
      // `setPlanTimer` must not match `startTimer` by accident, so this is a
      // whole-identifier check.
      expect(planner).not.toMatch(new RegExp(`\\b${w}\\b`));
    }
  });
});
