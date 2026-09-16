// THE PROGRAMME TIMERS, ON THE SCREEN THE PREACHER IS HOLDING.
//
// A `Stage`-scoped timer publishes no content frame at all — that is exactly why it
// survives a verse, a song and a notice. It reaches the stage tablet as its own hub
// frame, `{"kind":"timer","timers":[…]}`, and this file is the stage page's half of
// that contract.
//
// ── WHAT THESE TESTS ARE REALLY GUARDING ────────────────────────────────────────
//
// `countdown.js::countdownRemainingMs` is the ONE reader of how long is left, on
// every surface (docs/REBRAND.md §7). The wall, the stage page and the console each
// did their own `countdown_to - now` once; three copies of a subtraction is
// survivable, and three copies of a subtraction that now has an EXCEPTION is not — a
// held timer whose exception one of the three has never heard of goes on counting
// down on that surface while the other two hold, and one of the three is the screen
// a preacher reads from mid-sermon. So a programme row must be wrong in exactly the
// same way the wall would be, or not at all.
//
// The rows also ride the page's EXISTING 1000 ms tick. A second interval is a second
// clock, and two clocks on one page drift.
//
//   npx vitest run src/lib/timers.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { formatCountdown } from './layers.js';
import { countdownRemainingMs } from './countdown.js';

const Stage = (await import('../Stage.svelte')).default;

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

let host;
let app;

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
});
afterEach(() => {
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
  vi.useRealTimers();
});

async function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  await tick();
}

/** Deliver one hub frame to the page, exactly as the socket would. */
async function deliver(frame) {
  socket.onmessage({ data: JSON.stringify(frame) });
  await tick();
  await tick();
}

/** The frame the Rust `timer_frame_json` builds, for `n` minutes from `at`. */
const entry = (id, label, minutes, at, extra = {}) => ({
  id,
  label,
  countdown_to: at + minutes * 60_000,
  countdown_from: at,
  countdown_paused_ms: null,
  countdown_done: '',
  warn_ms: null,
  ...extra,
});

/** The rendered programme rows, as `[label, digits]` pairs. */
const rows = () =>
  [...host.querySelectorAll('[data-timer-id]')].map((el) => [
    el.querySelector('.tlabel')?.textContent?.trim() ?? '',
    el.querySelector('.tval')?.textContent?.trim() ?? '',
  ]);

describe('the stage page shows the programme timers', () => {
  it('paints one row per stage timer, in the order the hub sent them', async () => {
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Offering', 5, at), entry(2, 'Sermon', 20, at)],
    });

    expect(rows()).toEqual([
      ['Offering', '5:00'],
      ['Sermon', '20:00'],
    ]);
  });

  it('reads how long is left through the one reader, never its own subtraction', async () => {
    // The assertion is deliberately against `countdownRemainingMs` itself rather
    // than against a literal: a literal would still pass if the page grew a second
    // copy of the arithmetic that happened to agree today.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    const held = entry(2, 'Sermon', 20, at, { countdown_paused_ms: 90_000 });
    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 5, at), held] });

    // Four minutes of real time pass on the page's own tick.
    vi.advanceTimersByTime(4 * 60_000);
    await tick();
    await tick();
    const now = at + 4 * 60_000;

    expect(rows()).toEqual([
      ['Offering', formatCountdown(countdownRemainingMs(entry(1, 'Offering', 5, at), now))],
      ['Sermon', formatCountdown(countdownRemainingMs(held, now))],
    ]);
    // …and the held one is the whole point: it must still read 1:30, not 16:00.
    expect(rows()[1][1]).toBe('1:30');
  });

  it('ticks on the page’s existing interval and opens no second one', async () => {
    // Two clocks on one page drift, and the one that drifts is the one nobody is
    // watching. The stage page already ticks once a second for the wall clock, the
    // elapsed zone and the countdown mirror; the programme rows ride that.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    // COUNTED FROM BEFORE THE MOUNT, not from after it. Counting from after would
    // only ever have caught an interval opened by the `timer` branch itself, and
    // the natural place to write the second clock is `onMount` beside the first —
    // where a from-after count reads zero new intervals and passes. Watched to fail
    // with a second `setInterval` added to `onMount`.
    const spy = vi.spyOn(globalThis, 'setInterval');
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 5, at)] });
    expect(rows()[0][1]).toBe('5:00');

    vi.advanceTimersByTime(1000);
    await tick();
    await tick();
    expect(rows()[0][1]).toBe('4:59');
    expect(
      spy.mock.calls.length,
      'the stage page is running more than one clock — two clocks on one page drift, ' +
        'and the one that drifts is the one nobody is watching',
    ).toBe(1);
    spy.mockRestore();
  });

  it('shows the digits alone when a timer has no label', async () => {
    // Wave 5 Track G makes label-less the dock's default, so this page has to
    // survive it already. A collapsed or clipped box is the failure to avoid: an
    // empty label must give up its room, not take it.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, '', 5, at)] });
    expect(rows()).toEqual([['', '5:00']]);
    expect(host.querySelector('[data-timer-id] .tlabel')).toBeNull();
  });

  it('takes the rows away when the hub sends an empty set', async () => {
    // Stopping the last programme timer publishes `timers: []`, and that is how a
    // clock comes OFF a preacher's screen. A page that ignored the empty set would
    // leave it there, counting, for the rest of the service.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 5, at)] });
    expect(rows()).toHaveLength(1);

    await deliver({ kind: 'timer', timers: [] });
    expect(rows()).toHaveLength(0);
  });

  it('keeps the programme when the congregation screens are cleared or blacked', async () => {
    // The operator's decision this wave carries: a `Stage`-scoped timer survives a
    // panic control and a `Both`-scoped one does not. The backend half is the
    // registry's (`stop_scope(Both)` from `clear` and `black`); this is the page
    // half, and it holds because the rows are rendered OUTSIDE the `visible` gate
    // rather than because anything remembered to re-send them.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Sermon', 20, at)] });
    await deliver({ kind: 'content', reference: 'Romans 8:28', text: 'And we know' });
    expect(host.textContent).toContain('Romans 8:28');

    await deliver({ kind: 'clear' });
    expect(host.textContent).not.toContain('Romans 8:28');
    expect(rows(), 'a clear took the preacher’s programme with it').toEqual([
      ['Sermon', '20:00'],
    ]);

    await deliver({ kind: 'black' });
    expect(rows(), 'a blackout took the preacher’s programme with it').toEqual([
      ['Sermon', '20:00'],
    ]);
  });

  it('survives a frame whose timers field is missing or not a list', async () => {
    // The hub is the only thing that sends this, but the stage page has no backend
    // and cannot verify who is on the other end of its socket (SECURITY.md T4). A
    // frame it cannot read must leave the page showing what it was showing rather
    // than throwing inside the message handler, which would kill every frame after
    // it for the rest of the service.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Sermon', 20, at)] });
    await deliver({ kind: 'timer' });
    expect(rows()).toHaveLength(0);

    await deliver({ kind: 'timer', timers: [entry(1, 'Sermon', 20, at)] });
    await deliver({ kind: 'timer', timers: 'nonsense' });
    expect(rows()).toHaveLength(0);

    // …and the socket is still alive for the next real frame.
    await deliver({ kind: 'content', reference: 'John 3:16', text: 'For God so loved' });
    expect(host.textContent).toContain('John 3:16');
  });
});
