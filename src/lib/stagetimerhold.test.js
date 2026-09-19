// A SERMON THAT HAS RUN OVER CAN BE HELD (RG-175).
//
// The one state `Stage.svelte` could render and nothing in the product could
// produce. Its programme rail has drawn a `held` row — frozen, never warned,
// with its own `Held` chip and its own CSS — since wave 4, and `adjust_timer`
// has taken a `paused` argument for just as long. Between them:
//
//   * `timers::remaining_ms` clamped at zero, so a timer two minutes over
//     answered `0`;
//   * `TimerRegistry::adjust` refused anything under a second as `TooShort`;
//   * `countdown.js::countdownRemainingMs` returned a held figure only when it
//     was `> 0`, so even a negative that reached the page was ignored;
//   * and no rendered control anywhere called `adjust_timer` with `paused` at
//     all — `+5` and `Stop` were the whole transport.
//
// DECISIONS §99 looked at the first three and recorded the hold as
// "structurally inexpressible", filing RG-175 rather than forcing it. It was a
// clamp, a comparison and a missing button.
//
// **Why the operator's side matters more than it looks.** A clock past zero is
// the moment somebody has to decide something — let it run, or stop it. Holding
// is the third answer, and it is the one that keeps the elapsed figure the
// preacher has been reading. Stop throws that away; `+5` re-aims it. Neither is
// "note where we got to".
//
// The file also holds RESET, the third transport verb, because it is the same
// question one verb along: what an operator is allowed to do to a clock that
// has run out. `+5` adds to what is there, Stop takes the timer away, Hold
// freezes the figure — and Reset is "start that again" without losing the
// label, the chosen warning threshold or the cue binding, which Stop-then-Start
// throws away along with it.
//
//   npx vitest run src/lib/stagetimerhold.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { atClockTime } = await import('./countdown.js');
const cap = await import('./stores/capture.js');
const { setSession } = await import('./session.js');
const Live = (await import('./views/Live.svelte')).default;

let host;
let app;

/** A Stage Timer in the registry's own shape, as `list_timers` returns it. */
const T = (over = {}) => ({
  id: 7,
  label: 'Sermon',
  done_msg: '',
  target_ms: Date.now() + 300_000,
  from_ms: Date.now(),
  paused_ms: null,
  warn_ms: null,
  scope: 'stage',
  plan_item_id: null,
  remaining_ms: 300_000,
  ...over,
});

function bridge({ timers = [] } = {}) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_timers') return timers;
    if (cmd === 'adjust_timer') return timers[0] ?? null;
    if (cmd === 'list_output_channels') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'list_plans') return [];
    if (cmd === 'list_books') return [{ book: 'Psalms', chapters: 150 }];
    if (cmd === 'rehearsal') return false;
    if (cmd === 'get_sensitivity') return 50;
    return null;
  });
}

function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Live({ target: host, props: {} });
  return host;
}

async function settle(ms = 80) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const block = () => host.querySelector('.pt-band');
const called = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);
const byText = (text) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === text);

beforeEach(() => {
  invoke.mockReset();
  bridge();
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.templates.set([]);
  setSession({ planId: null });
  return cap.loadTemplates();
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
});

describe('holding a Stage Timer from the operator desk', () => {
  it('offers a Hold control on a running timer', async () => {
    bridge({ timers: [T()] });
    mount();
    await settle(40);
    expect(byText('Hold'), 'no Hold control on the band').toBeTruthy();
  });

  it('asks the engine to hold WITHOUT naming a figure', async () => {
    // The figure is the engine's to read. Sending one is how this broke before:
    // the clamped reading is `0` past zero, and `adjust` refuses a requested
    // length under a second as `TooShort` — so a hold computed on this side
    // would be refused at exactly the moment it is wanted.
    bridge({ timers: [T()] });
    mount();
    await settle(40);
    byText('Hold').click();
    await settle(40);
    const call = called('adjust_timer').at(-1);
    expect(call, 'Hold reached no backend command').toBeTruthy();
    expect(call[1].paused).toBe(true);
    expect(
      call[1].remainingMs ?? null,
      'Hold named a length; the engine must read the clock itself',
    ).toBeNull();
  });

  it('holds a timer that has already run over, which is the whole finding', async () => {
    bridge({ timers: [T({ target_ms: Date.now() - 120_000, remaining_ms: 0 })] });
    mount();
    await settle(40);
    expect(block().textContent).toMatch(/\+\s?[12]:\d\d/);
    const hold = byText('Hold');
    expect(hold, 'an overrun timer offered no Hold').toBeTruthy();
    expect(hold.disabled, 'Hold was disabled exactly when it is wanted').toBe(false);
    hold.click();
    await settle(40);
    expect(called('adjust_timer').at(-1)[1].paused).toBe(true);
  });

  it('offers Resume on a held timer, and says it is held', async () => {
    bridge({ timers: [T({ paused_ms: -120_000 })] });
    mount();
    await settle(40);
    expect(byText('Resume'), 'a held timer offered no way back').toBeTruthy();
    expect(byText('Hold'), 'a held timer still offered Hold').toBeFalsy();
    expect(block().textContent.toLowerCase()).toContain('held');
  });

  it('resumes by asking for paused false, again without a figure', async () => {
    bridge({ timers: [T({ paused_ms: -120_000 })] });
    mount();
    await settle(40);
    byText('Resume').click();
    await settle(40);
    const call = called('adjust_timer').at(-1);
    expect(call[1].paused).toBe(false);
    expect(call[1].remainingMs ?? null).toBeNull();
  });

  it('shows a held overrun timer frozen at the figure it was holding', async () => {
    // Not zero, and not counting. `-120_000` held means `+2:00`, and it must
    // still read `+2:00` a tick later.
    bridge({ timers: [T({ paused_ms: -120_000 })] });
    mount();
    await settle(40);
    const first = block().textContent;
    expect(first).toMatch(/\+\s?2:00/);
    await settle(1200);
    expect(block().textContent).toMatch(/\+\s?2:00/);
  });

  it('never touches a congregation control to do any of it', async () => {
    bridge({ timers: [T()] });
    mount();
    await settle(40);
    byText('Hold').click();
    await settle(40);
    for (const forbidden of ['show_timer', 'start_countdown', 'adjust_countdown', 'fire_content']) {
      expect(called(forbidden), `Hold reached ${forbidden}`).toHaveLength(0);
    }
  });
});

describe('resetting a Stage Timer', () => {
  it('offers a Reset control and reaches the command with the timer id', async () => {
    bridge({ timers: [T()] });
    mount();
    await settle(40);
    const reset = byText('Reset');
    expect(reset, 'no Reset control on the band').toBeTruthy();
    reset.click();
    await settle(40);
    const call = called('reset_timer').at(-1);
    expect(call, 'Reset reached no backend command').toBeTruthy();
    expect(call[1].timerId).toBe(7);
  });

  it('is not a Stop, and must never reach one', async () => {
    // The whole reason Reset exists: doing it by hand means Stop then Start,
    // which loses the name, the warning threshold and the cue binding.
    bridge({ timers: [T()] });
    mount();
    await settle(40);
    byText('Reset').click();
    await settle(40);
    expect(called('stop_timer')).toHaveLength(0);
    expect(called('start_timer')).toHaveLength(0);
  });

  it('names no length, because only the registry knows the configured one', async () => {
    // A re-aim moves `target_ms` and leaves `from_ms`, so the span on this side
    // grows by five minutes every time `+5` is pressed. Nothing here can
    // reconstruct what was originally chosen, so nothing here should try.
    bridge({ timers: [T({ target_ms: Date.now() + 900_000 })] });
    mount();
    await settle(40);
    byText('Reset').click();
    await settle(40);
    const call = called('reset_timer').at(-1);
    expect(Object.keys(call[1])).toEqual(['timerId']);
  });

  it('resets a timer that has run over, and touches no congregation control', async () => {
    bridge({ timers: [T({ target_ms: Date.now() - 120_000, remaining_ms: 0 })] });
    mount();
    await settle(40);
    const reset = byText('Reset');
    expect(reset.disabled).toBe(false);
    reset.click();
    await settle(40);
    expect(called('reset_timer')).toHaveLength(1);
    for (const forbidden of ['adjust_countdown', 'start_countdown', 'show_timer']) {
      expect(called(forbidden), `Reset reached ${forbidden}`).toHaveLength(0);
    }
  });
});

// ── A LENGTH OR AN APPOINTMENT (DECISIONS §102) ────────────────────────────
//
// Every creator in the product took `minutes: f64` and computed
// `now + minutes*60000`. There was no `time_of_day` in any layer — not in the
// schema, the commands, the stores or any control — so "be off the platform at
// 11:15" was arithmetic an operator did in their head, and it was wrong the
// moment the service slipped.
describe('starting a Stage Timer at a time of day', () => {
  const typeAt = async (value) => {
    const box = host.querySelector('[aria-label="Stage Timer clock time"]');
    expect(box, 'no clock-time control on the band').toBeTruthy();
    box.value = value;
    box.dispatchEvent(new Event('input'));
    await settle(20);
    return box;
  };

  it('sends the instant that clock time names, not a number of minutes', async () => {
    bridge();
    mount();
    await settle(40);
    await typeAt('10:30');
    byText('Start timer').click();
    await settle(40);
    const call = called('start_timer').at(-1);
    expect(call, 'Start reached no command').toBeTruthy();
    const expected = atClockTime('10:30');
    // Within a second: the component and the assertion each call `Date.now()`.
    expect(Math.abs(call[1].untilMs - expected)).toBeLessThan(1000);
    expect(call[1].scope).toBe('stage');
  });

  it('sends no instant when the field is empty, so minutes still mean minutes', async () => {
    bridge();
    mount();
    await settle(40);
    byText('Start timer').click();
    await settle(40);
    expect(called('start_timer').at(-1)[1].untilMs).toBeNull();
  });

  it('refuses to start on something that is not a time, and says which field', async () => {
    bridge();
    mount();
    await settle(40);
    const box = await typeAt('half ten');
    expect(box.getAttribute('aria-invalid')).toBe('true');
    expect(byText('Start timer').disabled, 'Start stayed live over a time nobody can parse').toBe(true);
    expect(called('start_timer')).toHaveLength(0);
  });

  it('accepts a time that has already gone, so the clock starts over', async () => {
    // The decision: an operator who typed a time that has passed sees it
    // immediately. Rolling to tomorrow would read 23:55:00 and hide the typo.
    bridge();
    mount();
    await settle(40);
    const gone = new Date(Date.now() - 5 * 60_000);
    const hhmm = `${gone.getHours()}:${String(gone.getMinutes()).padStart(2, '0')}`;
    await typeAt(hhmm);
    expect(byText('Start timer').disabled).toBe(false);
    byText('Start timer').click();
    await settle(40);
    expect(called('start_timer').at(-1)[1].untilMs).toBeLessThan(Date.now());
  });
});
