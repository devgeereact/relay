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
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { tick } from 'svelte';
import { formatCountdown } from './layers.js';
import { countdownRemainingMs } from './countdown.js';

// jsdom has no layout engine, so the parts of this that are geometry are held as a
// contract on the stylesheet — the same split `stagezones.test.js` records, and for
// the same reason: a `getBoundingClientRect` here is zeroes, and a test that asserts
// on zeroes passes over a broken page.
const SRC = readFileSync(path.resolve(__dirname, '../Stage.svelte'), 'utf8');

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

const WIDE = 1024; // jsdom's own default, and the width every test below assumes

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  window.innerWidth = WIDE;
});
afterEach(() => {
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
  window.innerWidth = WIDE;
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

// ══ WAVE 4 TRACK A ═══════════════════════════════════════════════════════════
//
// The rail above renders a label and digits and nothing else. `warn_ms` rides in
// the frame (`channels::timer_frame_json`) and was deliberately left unread while
// the warning rule was being settled on the other three surfaces; `countdown_done`
// rides there too and was dropped on the floor, so a finished programme timer read
// `0:00` — a clock that is still running and has just arrived. A held row froze,
// correctly, and said nothing about being held.
//
// This is the one surface in the building whose whole job is to tell a preacher how
// long is left, and it was the only timer surface with no warning state at all.

/** Every programme row element, in the order the rail paints them. */
const rowEls = () => [...host.querySelectorAll('[data-timer-id]')];
/** The ids of the rows wearing a given state class. */
const wearing = (cls) =>
  rowEls().filter((el) => el.classList.contains(cls)).map((el) => el.dataset.timerId);
/** Advance the page's own 1s clock and let Svelte settle. */
async function advance(ms) {
  vi.advanceTimersByTime(ms);
  await tick();
  await tick();
}
/** Mount at a fixed instant with the fake clock running. */
async function mountAt(at) {
  vi.useFakeTimers();
  vi.setSystemTime(at);
  await mount();
}

describe('a programme row that is nearly out says so', () => {
  const at = 1_700_000_000_000;

  it('crosses into the warning state at the threshold the frame carries', async () => {
    // The figure that decides this is the operator's, and it arrives per row.
    await mountAt(at);
    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Offering', 5, at, { warn_ms: 120_000 })],
    });
    expect(wearing('warn'), 'five minutes left is not a warning').toEqual([]);

    await advance(2 * 60_000);
    expect(rows()[0][1]).toBe('3:00');
    expect(wearing('warn'), 'three minutes left is still not a warning').toEqual([]);

    // The boundary itself: `countdownWarning` is `left <= chosen`, so the row is
    // already warning at exactly the threshold rather than a second after it.
    await advance(60_000);
    expect(rows()[0][1]).toBe('2:00');
    expect(wearing('warn')).toEqual(['1']);
  });

  it('never warns on a row whose frame chose no threshold, at any figure', async () => {
    // THE DEFECT THIS ONE EXISTS FOR. `countdownWarning` has a default rule — the
    // last minute, or the last tenth of a short countdown — and that default is
    // `Settings → General → Countdown warning`, which lives in the console's
    // database. THIS PAGE HAS NO BRIDGE AND NEVER READS IT: `setCountdownWarnDefault`
    // is called by `stores/capture.js`, which the stage page does not import. So a
    // rail that fell back to the default would be warning on a figure the church
    // never chose and the wall does not agree with — a fourth reading of the rule,
    // which is what §7 forbids. The threshold comes off the frame or there is none.
    await mountAt(at);
    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 5, at)] });

    await advance(4 * 60_000 + 59_000);
    expect(rows()[0][1]).toBe('0:01');
    expect(
      wearing('warn'),
      'the rail invented a threshold nobody on this page can see the setting for',
    ).toEqual([]);
  });

  it('does not flash a row that is being held', async () => {
    // A held timer is not running out. It is exactly where the operator left it,
    // and a frozen figure pulsing red says the opposite of what is true.
    await mountAt(at);
    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Sermon', 20, at, { countdown_paused_ms: 30_000, warn_ms: 120_000 })],
    });

    expect(rows()[0][1]).toBe('0:30');
    expect(wearing('warn'), 'a held clock is not running out').toEqual([]);
  });

  // THE TREATMENT ITSELF IS PINNED NEXT DOOR, in `countdownwarnmotion.test.js`,
  // which is the register of which surfaces carry a countdown warning and what each
  // of them does when a viewer asks for no motion. A second copy of that assertion
  // here is how two files come to disagree about one rule; the programme rail is a
  // fourth entry in that list rather than a second home for it.
});

describe('a finished programme timer says its message, and a held one says it is held', () => {
  const at = 1_700_000_000_000;

  it('shows the operator’s own words when it reaches zero', async () => {
    await mountAt(at);
    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Offering', 5, at, { countdown_done: 'Offering over' })],
    });

    await advance(5 * 60_000);
    expect(rows()[0][1], '`0:00` reads as a clock that has just arrived').toBe('Offering over');
  });

  it('shows 0:00 at zero when no message was set, and never an empty box', async () => {
    await mountAt(at);
    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 5, at)] });

    await advance(5 * 60_000);
    expect(rows()[0][1]).toBe('0:00');
  });

  it('marks a held row and keeps its frozen figure', async () => {
    await mountAt(at);
    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Sermon', 20, at, { countdown_paused_ms: 90_000 })],
    });

    expect(rows()[0][1]).toBe('1:30');
    expect(wearing('held'), 'a held clock looked exactly like a running one').toEqual(['1']);
    expect(host.querySelector('[data-timer-id] .tstate').textContent.trim()).toBe('Held');

    // …and it is still frozen a minute later, which is what makes the mark worth
    // having: the digits alone cannot tell a held clock from a stopped page.
    await advance(60_000);
    expect(rows()[0][1]).toBe('1:30');
    expect(wearing('held')).toEqual(['1']);
  });

  it('takes the mark away when the hold is released', async () => {
    await mountAt(at);
    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Sermon', 20, at, { countdown_paused_ms: 90_000 })],
    });
    expect(wearing('held')).toEqual(['1']);

    // The registry re-aims the countdown on release and publishes the whole set.
    await deliver({
      kind: 'timer',
      timers: [{ ...entry(1, 'Sermon', 0, at), countdown_to: at + 90_000 }],
    });
    expect(wearing('held'), 'the mark outlived the hold').toEqual([]);
    expect(host.querySelector('[data-timer-id] .tstate')).toBeNull();
    expect(rows()[0][1]).toBe('1:30');
  });

  it('keeps the held mark off the colour law', () => {
    // Held is a state the operator caused deliberately, and `Dock.svelte` already
    // records which treatment that gets: the page's own ink. Amber means ON AIR,
    // cyan means a guess, amethyst means rehearsal, and none of those is true of a
    // clock somebody paused.
    const style = SRC.slice(SRC.indexOf('<style>')).replace(/\/\*[\s\S]*?\*\//g, '');
    const i = style.indexOf('.tstate {');
    expect(i, 'no rule for .tstate').toBeGreaterThan(-1);
    const rule = style.slice(i, style.indexOf('}', i));
    for (const [token, means] of Object.entries({
      '--v-amber': 'ON AIR',
      '--v-cyan': 'a guess',
      '--v-amethyst': 'rehearsal',
    })) {
      expect(rule, `the held mark may not be ${token} — that means ${means}`).not.toContain(token);
    }
  });
});

describe('the programme rail has a floor', () => {
  const at = 1_700_000_000_000;
  const many = (n) =>
    Array.from({ length: n }, (_, i) => entry(i + 1, `Item ${i + 1}`, i + 2, at));

  it('shows every timer while there is room for every timer', async () => {
    window.innerWidth = 1280;
    await mountAt(at);
    await deliver({ kind: 'timer', timers: many(6) });

    expect(rows()).toHaveLength(6);
    expect(host.querySelector('.tmore'), 'nothing was hidden, so nothing should say so').toBeNull();
  });

  it('says how many it could not show rather than shrinking them all below reading size', async () => {
    // `.tmr { flex: 1 1 0 }` divides the row by the number of timers with no floor
    // at all, so six timers on a phone in portrait is six columns of about sixty
    // pixels — every clock on the rail illegible, and nothing anywhere saying that
    // the rail had given up. A row that cannot show every timer must SAY SO.
    window.innerWidth = 400;
    await mountAt(at);
    await deliver({ kind: 'timer', timers: many(6) });

    const shown = rows();
    expect(shown.length, 'the rail shrank every timer instead of stopping').toBeLessThan(6);
    expect(shown.length, 'the rail must still show at least one clock').toBeGreaterThan(0);
    const more = host.querySelector('.tmore');
    expect(more, 'timers went missing and nothing said so').toBeTruthy();
    expect(more.textContent).toContain(String(6 - shown.length));

    // The cells the row divides itself by must be the cells it actually paints, or
    // the figures are sized for a rail nobody is looking at.
    const cells = host.querySelectorAll('.progrow .tmr').length;
    expect(host.querySelector('.progrow').style.getPropertyValue('--tmrs').trim()).toBe(
      String(cells),
    );
  });

  it('takes the count away again when the screen has room', async () => {
    window.innerWidth = 400;
    await mountAt(at);
    await deliver({ kind: 'timer', timers: many(6) });
    expect(host.querySelector('.tmore')).toBeTruthy();

    window.innerWidth = 1600;
    window.dispatchEvent(new Event('resize'));
    await tick();
    await tick();
    expect(rows()).toHaveLength(6);
    expect(host.querySelector('.tmore')).toBeNull();
  });
});
