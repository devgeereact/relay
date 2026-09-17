// W4·E · THE JOIN — the console that starts a programme clock, and the rail that
// has to show it. RG-146 and RG-147.
//
// WHY THIS FILE EXISTS RATHER THAN MORE CASES IN THE TWO BESIDE IT.
//
// Wave 4 Track A built three states on the preacher's programme rail — a warning,
// a finished message, a held row — and tested every one of them against a frame it
// wrote by hand (`timers.test.js`). Wave 4 Track B built the only thing in the
// product that starts a programme clock, and tested that it starts exactly one,
// for the right cue, never in a rehearsal (`cuetimer.test.js`). Both suites were
// green. Neither could see that the clock Track B starts carries no threshold and
// no message, so on a shipped copy of Relay the rail could never turn red and could
// never say anything at zero but `0:00`; and that nothing ever stops one, so a walk
// of a three-cue plan left three dead clocks on a preacher's phone with the live
// one hidden inside `+N more` (RG-146, RG-147, driven on 2026-09-17 against the
// real backend).
//
// A test inside either side would have been green as well. So these drive Live's
// real fire path, put the traffic it emits through a registry that keeps the same
// rules the Rust one keeps, and mount the REAL stage page on the frame that comes
// out — console, wire, rail, in one assertion each.
//
//   npx vitest run src/lib/progtimerjoin.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { setSession, clearSession } = await import('./session.js');
const Stage = (await import('../Stage.svelte')).default;

// The same self-detecting gate the rest of the suite uses: without
// `resolve: { conditions: ['browser'] }` in `vitest.config.js`, `onMount` is a
// literal empty function and a mounted Live fetches nothing at all. These SKIP
// loudly rather than pass over a view that never loaded a plan.
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
const notice = (id, label, over = {}) => ({
  id,
  plan_id: 1,
  position: id,
  cue_type: 'announce',
  label,
  payload_json: JSON.stringify({ body: `${label} — body` }),
  template_id: null,
  section_title: '',
  duration_sec: 0,
  timer_minutes: null,
  ...over,
});

// ── A REGISTRY THAT KEEPS THE RULES THE RUST ONE KEEPS ───────────────────────
//
// Three of them, and each is a rule `src-tauri/src/timers.rs` and `main::start_timer`
// state in their own words: ids only go up; starting a clock for a cue that already
// has one REPLACES it rather than stacking; `list_timers` answers oldest first, by
// id. A fake that let a second clock stack for one cue would hide the very failure
// these tests are about.
let registry;
let nextTimerId;
function startedTimer(args) {
  const cue = args.planItemId ?? null;
  if (cue != null) registry = registry.filter((t) => t.plan_item_id !== cue);
  nextTimerId += 1;
  registry.push({
    id: nextTimerId,
    label: args.label ?? '',
    minutes: Number(args.minutes) || 0,
    done_msg: args.doneMsg ?? '',
    warn_ms: args.warnMs ?? null,
    scope: args.scope,
    plan_item_id: cue,
  });
  return nextTimerId;
}

/** The frame `channels::timer_frame_json` builds out of the registry, at `at`. */
const timerFrame = (at) => ({
  kind: 'timer',
  timers: registry
    .filter((t) => t.scope === 'stage')
    .sort((a, b) => a.id - b.id)
    .map((t) => ({
      id: t.id,
      label: t.label,
      countdown_to: at + t.minutes * 60_000,
      countdown_from: at,
      countdown_paused_ms: null,
      countdown_done: t.done_msg,
      warn_ms: t.warn_ms,
    })),
});

let cues = [];
let host;
let app;
let stageHost;
let stageApp;
let socket;

class FakeSocket {
  constructor() {
    socket = this;
    this.sent = [];
  }
  send(m) {
    this.sent.push(m);
  }
  close() {
    this.closed = true;
  }
}

beforeEach(() => {
  registry = [];
  nextTimerId = 0;
  socket = null;
  globalThis.WebSocket = FakeSocket;
  window.innerWidth = 1024;
  invoke.mockReset();
  invoke.mockImplementation((cmd, args = {}) => {
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
        return Promise.resolve(startedTimer(args));
      case 'list_timers':
        return Promise.resolve(registry.map((t) => ({ ...t })));
      case 'stop_timer':
        registry = registry.filter((t) => t.id !== args.timerId);
        return Promise.resolve(null);
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
  cap.countdownWarnMs.set(60_000);
  clearSession();
  host = document.createElement('div');
  document.body.appendChild(host);
});

afterEach(() => {
  // ALWAYS, not only at the end of the tests that install them: a fake clock left
  // running makes every `settle()` after it hang, so one real failure would come
  // back as nine timeouts and hide which assertion actually broke.
  vi.useRealTimers();
  app?.$destroy();
  stageApp?.$destroy();
  host?.remove();
  stageHost?.remove();
  app = host = stageApp = stageHost = null;
  clearSession();
  document.body.innerHTML = '';
  try {
    localStorage.removeItem('relay.stage.zones');
  } catch {
    /* the shim in test-setup.js hands us a real Storage */
  }
});

const calls = (cmd) => invoke.mock.calls.filter(([c]) => c === cmd).map(([, args]) => args);

/** Mount Live with the plan already chosen. */
async function runPlan() {
  setSession({ planId: PLAN.id });
  const Live = (await import('./views/Live.svelte')).default;
  app = new Live({ target: host, props: {} });
  await until(() => host.querySelector('.sg-cell'), 'the slide grid to draw the plan');
}

/** Press one slide cell and let the press arbiter's 190ms hold expire. */
async function take(n) {
  host.querySelectorAll('.sg-cell')[n].click();
  await settle(300);
  await tick();
}

/** Mount the real stage page and deliver the registry as the hub would. */
async function railAt(at) {
  stageHost = document.createElement('div');
  document.body.appendChild(stageHost);
  stageApp = new Stage({ target: stageHost });
  await tick();
  socket.onopen?.();
  socket.onmessage({ data: JSON.stringify(timerFrame(at)) });
  await tick();
  await tick();
  return [...stageHost.querySelectorAll('[data-timer-id]')].map((el) => ({
    label: el.querySelector('.tlabel')?.textContent?.trim() ?? '',
    value: el.querySelector('.tval')?.textContent?.trim() ?? '',
    warn: el.classList.contains('warn'),
  }));
}

// ─────────────────────────────────────────────────────────────────────────────
// RG-146 · THE WARNING THRESHOLD REACHES THE RAIL
// ─────────────────────────────────────────────────────────────────────────────
describe('a cue’s clock carries the warning window the operator chose', () => {
  itMounted('the figure Live sends is the one Settings is holding', async () => {
    // Not the shipped minute: a church that moved this control must get ITS figure
    // on the preacher's rail, or the rail and the wall disagree about the same
    // clock. `countdownWarnMs` is the console's mirror of `countdown.warn_ms`,
    // loaded at launch by `App.svelte`.
    cap.countdownWarnMs.set(120_000);
    cues = [notice(21, 'Reading', { timer_minutes: 12 })];
    await runPlan();
    invoke.mockClear();
    await take(0);

    const started = calls('start_timer');
    expect(started).toHaveLength(1);
    expect(
      started[0].warnMs,
      'the rail can only warn at a figure the FRAME carries — this page has no bridge',
    ).toBe(120_000);
  });

  itMounted('and the row on the real stage page crosses into the warning state', async () => {
    // THE JOIN. Track A proved the rail warns when a frame carries a threshold;
    // Track B proved Live starts the clock. This is the only assertion that says
    // the clock Live actually starts is one the rail can warn on.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    cap.countdownWarnMs.set(120_000);
    cues = [notice(21, 'Reading', { timer_minutes: 12 })];
    // The mounted-view helpers need real timers; only the rail's own clock is faked.
    vi.useRealTimers();
    await runPlan();
    await take(0);

    vi.useFakeTimers();
    vi.setSystemTime(at + 11 * 60_000); // 1:00 left, inside a two-minute window
    const rows = await railAt(at);
    expect(rows).toHaveLength(1);
    expect(rows[0].label).toBe('Reading');
    expect(rows[0].value).toBe('1:00');
    expect(rows[0].warn, 'a shipped copy of Relay could never reach this state').toBe(true);
    vi.useRealTimers();
  });

  itMounted('a cue carries no finished message, and the rail says 0:00 rather than inventing one', async () => {
    // THE HALF THAT STAYS OPEN, asserted so it stays an ABSENCE rather than drifting
    // into a sentence Relay composed and presented as somebody's choice. A cue has
    // no field for a done message and no control writes one; the surface that owns
    // message text is wave 3 Track E. RG-146 records it.
    const at = 1_700_000_000_000;
    cues = [notice(21, 'Reading', { timer_minutes: 12 })];
    await runPlan();
    await take(0);

    expect(calls('start_timer')[0].doneMsg ?? '').toBe('');
    vi.useFakeTimers();
    vi.setSystemTime(at + 12 * 60_000);
    const rows = await railAt(at);
    expect(rows[0].value, 'never an empty box').toBe('0:00');
    vi.useRealTimers();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// RG-147 · A CUE'S CLOCK ENDS WHEN THE CUE DOES
// ─────────────────────────────────────────────────────────────────────────────
describe('walking a plan leaves only the clock that is still running', () => {
  itMounted('the cue that leaves air has its clock stopped', async () => {
    cues = [notice(21, 'Opening song', { timer_minutes: 5 }), notice(22, 'Reading', { timer_minutes: 12 })];
    await runPlan();
    await take(0);
    const first = registry.find((t) => t.plan_item_id === 21);
    expect(first, 'the first cue never started a clock').toBeTruthy();

    invoke.mockClear();
    await take(1);

    expect(
      calls('stop_timer').map((a) => a.timerId),
      'nothing in the product could take a programme clock off the rail',
    ).toEqual([first.id]);
    expect(registry.filter((t) => t.scope === 'stage').map((t) => t.plan_item_id)).toEqual([22]);
  });

  itMounted('three cues walked end to end leave ONE row on the preacher’s rail', async () => {
    // The measured failure, reproduced through the real fire path: one walk of a
    // three-cue plan left `Opening song = 0:00 | Reading = 0:00 | Notices = 0:00`,
    // and at a phone's capacity of two the one clock shown was the OLDEST — a dead
    // one from the start of the service.
    const at = 1_700_000_000_000;
    cues = [
      notice(21, 'Opening song', { timer_minutes: 5 }),
      notice(22, 'Reading', { timer_minutes: 12 }),
      notice(23, 'Sermon', { timer_minutes: 25 }),
    ];
    await runPlan();
    await take(0);
    await take(1);
    await take(2);

    vi.useFakeTimers();
    vi.setSystemTime(at + 60_000);
    const rows = await railAt(at);
    expect(rows.map((r) => r.label), 'the rail kept the clocks that no longer matter').toEqual([
      'Sermon',
    ]);
    expect(rows[0].value).toBe('24:00');
    vi.useRealTimers();
  });

  itMounted('stepping to the next slide of the SAME cue stops nothing', async () => {
    // The cue has not left the air, so its clock has not ended. A rule that fired
    // per slide would take the sermon clock away in the middle of the sermon.
    cues = [notice(21, 'Reading', { timer_minutes: 12 }), notice(22, 'Notices')];
    await runPlan();
    await take(0);
    invoke.mockClear();
    await take(0);
    expect(calls('stop_timer')).toEqual([]);
    expect(calls('start_timer'), 'and it did not restart either').toEqual([]);
  });

  itMounted('an UNBOUND cue going on air still retires the clock before it', async () => {
    // The stop cannot live inside `startCueTimer`, which returns early for a cue
    // with no binding: `Sermon` (bound) followed by `Notices` (unbound) is exactly
    // the shape that would leave the sermon clock running for the rest of the
    // service. This is the door that would have been missed.
    cues = [notice(21, 'Sermon', { timer_minutes: 25 }), notice(22, 'Notices')];
    await runPlan();
    await take(0);
    const sermon = registry.find((t) => t.plan_item_id === 21);

    invoke.mockClear();
    await take(1);
    expect(calls('start_timer'), 'an unbound cue starts nothing').toEqual([]);
    expect(calls('stop_timer').map((a) => a.timerId)).toEqual([sermon.id]);
    expect(registry.filter((t) => t.scope === 'stage')).toEqual([]);
  });

  itMounted('a clock the console did not start is left alone', async () => {
    // `retireCueTimer` asks about `plan_item_id` AND `scope`. A congregation
    // countdown is not the programme, and the plan walking past a cue is not a
    // reason to take one off a wall — the way a countdown comes down is a panic
    // control or the operator.
    cues = [notice(21, 'Opening song', { timer_minutes: 5 }), notice(22, 'Reading', { timer_minutes: 12 })];
    await runPlan();
    await take(0);
    startedTimer({ minutes: 10, label: 'Service begins', scope: 'both', planItemId: null });
    const wall = registry.find((t) => t.scope === 'both');

    await take(1);
    expect(registry.some((t) => t.id === wall.id), 'the plan took a congregation countdown down').toBe(
      true,
    );
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE PANIC CONTROLS ARE NOT ONE OF THESE DOORS
// ─────────────────────────────────────────────────────────────────────────────
describe('what does NOT end a cue’s clock', () => {
  itMounted('taking the screens back leaves the programme running', async () => {
    // Wave 3's decision, and rule 15's: `clear` and `black` take `Both` and leave
    // `Stage`. A panic control may not gain a question it can fail to answer, and
    // the preacher's own bookkeeping is not the congregation's screen. The store's
    // `onAir` goes false and the clock does not.
    cues = [notice(21, 'Sermon', { timer_minutes: 25 })];
    await runPlan();
    await take(0);
    const sermon = registry.find((t) => t.plan_item_id === 21);

    invoke.mockClear();
    await cap.clearScreens();
    await settle();
    expect(calls('stop_timer')).toEqual([]);
    expect(registry.some((t) => t.id === sermon.id)).toBe(true);
  });

  itMounted('a verse fired off-script leaves the sermon clock running', async () => {
    // The preacher going off-script is the entire product. `manualFire` calls
    // `leavePlan()`, so the cue is no longer on air — and it is still the slot the
    // clock is counting, which is the case the clock exists for.
    cues = [notice(21, 'Sermon', { timer_minutes: 25 })];
    await runPlan();
    await take(0);
    const sermon = registry.find((t) => t.plan_item_id === 21);

    invoke.mockClear();
    await cap.manualFire('John 3:16');
    await settle();
    expect(calls('stop_timer')).toEqual([]);
    expect(registry.some((t) => t.id === sermon.id)).toBe(true);
  });
});
