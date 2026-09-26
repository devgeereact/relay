// THE STAGE TIMERS, ON THE SCREEN THE PREACHER IS HOLDING.
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
import { formatCountdown, countdownWarning } from './layers.js';
import { countdownRemainingMs, countdownTotalMs } from './countdown.js';

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

describe('the stage page shows the Stage Timers', () => {
  it('paints one row per Stage Timer, in the order the hub sent them', async () => {
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

  it('calls a timer nobody named TIMER, rather than showing digits alone', async () => {
    // ── REVERSED ON THE OPERATOR'S INSTRUCTION, 2026-09-23 (RG-287) ──────────
    //
    // This case used to assert the opposite — *"shows the digits alone when a
    // timer has no label"* — on the reasoning that Wave 5 Track G makes
    // label-less the dock's default and an empty label must give up its room
    // rather than take it. The operator, looking at a platform monitor:
    // *"Defult timer text should be TIMER not just empty."*
    //
    // The half of the old rule that was protecting something is KEPT and is
    // tested elsewhere: `programmeRoom` stands the whole head down below
    // `BARE_BELOW_PX`, so a rail too short for a name and a clock still spends
    // its height on the clock. What changes is a rail that HAS the room.
    //
    // The default is applied in `programmeRows` and nowhere else, so the phone
    // (here) and the stage TV (`stagefill.test.js`) have it or neither does.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, '', 5, at)] });
    expect(rows()).toEqual([['TIMER', '5:00']]);
    expect(host.querySelector('[data-timer-id] .tlabel').textContent).toBe('TIMER');
  });

  it('takes the rows away when the hub sends an empty set', async () => {
    // Stopping the last Stage Timer publishes `timers: []`, and that is how a
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

// ── THE ROW'S OWN GEOMETRY AND THE WARNING ──────────────────────────────────────
//
// These are the BEHAVIOUR half of RG-147 and RG-148: what the page hands the
// stylesheet, and which rows it marks. The stylesheet's own arithmetic, and what a
// browser did with it, are in `stageprogrow.test.js` — which says plainly what a
// test outside a layout engine can and cannot prove about a painted pixel.
describe('the programme row is sized from the text it is actually painting', () => {
  const progrow = () => host.querySelector('.progrow');

  it('hands the stylesheet the longest value in the row, not a constant', async () => {
    // RG-147. `.tval`'s width budget used to be a hard-coded six characters while
    // `formatCountdown` emits seven past an hour, so a 95-minute timer overflowed
    // its `overflow: hidden` box and `1:30:13` read as `1:30:1` — a wrong clock
    // that looks entirely normal to the one person reading it mid-sermon.
    //
    // This is the same instrument the figure row across the bottom already uses
    // (`figCh` into `--ch`), not a new one.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Offering', 5, at), entry(2, 'Livestream', 95, at)],
    });

    expect(rows()).toEqual([
      ['Offering', '5:00'],
      ['Livestream', '1:35:00'],
    ]);
    // Seven characters, because one of the rows has seven.
    expect(progrow().style.getPropertyValue('--tch')).toBe('7');
    expect(progrow().style.getPropertyValue('--tmrs')).toBe('2');
  });

  it('gives the budget back when the long timer goes away', async () => {
    // A budget that only ever grows is a row that stays small for the rest of the
    // service after one hour-long timer has been and gone.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Livestream', 95, at)] });
    expect(progrow().style.getPropertyValue('--tch')).toBe('7');

    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 5, at)] });
    expect(progrow().style.getPropertyValue('--tch')).toBe('4');
  });

  it('follows the clock across the hour boundary without being told', async () => {
    // The budget is derived from the rendered string every tick, so the row
    // re-sizes itself the moment a timer drops under an hour. Nothing has to
    // remember to do it.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Livestream', 60, at)] });
    expect(rows()[0][1]).toBe('1:00:00');
    expect(progrow().style.getPropertyValue('--tch')).toBe('7');

    vi.advanceTimersByTime(1000);
    await tick();
    await tick();
    expect(rows()[0][1]).toBe('59:59');
    expect(progrow().style.getPropertyValue('--tch')).toBe('5');
  });
});

describe('the programme row warns, on the one rule', () => {
  const warned = () =>
    [...host.querySelectorAll('[data-timer-id]')]
      .filter((el) => el.classList.contains('warn'))
      .map((el) => el.getAttribute('data-timer-id'));

  it('marks a timer inside its own chosen window and leaves the others alone', async () => {
    // RG-148. `warn_ms` rides every timer frame and was read off the hub verbatim
    // (`{"id":9,"label":"Wrap up",…,"warn_ms":30000}`), and nothing on the one
    // surface whose entire purpose is telling a preacher how long is left read it.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({
      kind: 'timer',
      timers: [
        // Inside its own explicit 30 s window: 20 s left.
        entry(1, 'Wrap up', 1, at - 40_000, { warn_ms: 30_000 }),
        // Outside it: 50 s left against the same 30 s window.
        entry(2, 'Notices', 1, at - 10_000, { warn_ms: 30_000 }),
      ],
    });

    expect(rows()).toEqual([
      ['Wrap up', '0:20'],
      ['Notices', '0:50'],
    ]);
    expect(warned()).toEqual(['1']);
  });

  it('asks layers.js rather than deciding for itself', async () => {
    // Asserted against `countdownWarning` itself, not against a literal: a literal
    // would still pass if this page grew a fourth reading of the rule that happened
    // to agree today. A timer with no chosen threshold falls to the shared rule,
    // the last minute or the last tenth of a countdown shorter than ten minutes,
    // and the short-countdown half is the one a page with no `countdown_from` could
    // never reach.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    // A four-minute countdown with 20 s left: under the shared rule its window is
    // a tenth of four minutes, 24 s, so it warns. Under a flat minute it would
    // have warned for a quarter of its life.
    const short = entry(3, 'Video', 4, at - (4 * 60_000 - 20_000));
    // A twenty-minute one with 90 s left: outside the minute.
    const long = entry(4, 'Sermon', 20, at - (20 * 60_000 - 90_000));
    await deliver({ kind: 'timer', timers: [short, long] });

    const expected = [short, long]
      .filter((t) => countdownWarning(countdownRemainingMs(t, at), countdownTotalMs(t), t.warn_ms))
      .map((t) => String(t.id));
    expect(expected).toEqual(['3']);
    expect(warned()).toEqual(expected);
  });

  it('does not ask the threshold rule about a timer that has already run out', async () => {
    // Zero is not "nearly gone", it is gone, and `countdownWarning` says so — it
    // answers false at and below zero, and that did NOT move when RG-153 landed.
    // The row is still marked, because being OVER is not a threshold question and
    // does not need one: what marks it is the sign of the one reader's answer.
    // This test exists to catch the wrong fix. Widening `countdownWarning` to
    // accept a negative would turn the row red the same way, and would be a fourth
    // reading of when to worry on the rule three surfaces share.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    const spent = entry(1, 'Offering', 1, at - 120_000, { warn_ms: 30_000 });
    await deliver({ kind: 'timer', timers: [spent] });

    expect(
      countdownWarning(
        countdownRemainingMs(spent, at, { past: true }),
        countdownTotalMs(spent),
        spent.warn_ms,
      ),
      'the shared threshold rule now answers true past zero — that is a fourth reading of it',
    ).toBe(false);
    expect(rows()).toEqual([['Offering', '+1:00']]);
    expect(warned()).toEqual(['1']);
  });
});

// ── RG-153 — A STAGE TIMER PAST ZERO ──────────────────────────────
//
// The browser pass watched a 45-second `Offering` timer read `0:00` for the next
// twelve minutes, and two of the six rows in the §8 screenshot were in that state
// (`audits/DESIGN.md` §11). A row that reads `0:00` cannot be told
// from one that has just been started at zero, and neither of them answers the
// question a stage monitor is actually asked.
//
// The operator's decision, taken 2026-09-17: a Stage Timer that reaches zero
// KEEPS COUNTING, UPWARD, wearing the warning colour. `+4:37` means the preacher is
// four and a half minutes over.
describe('RG-153 — a Stage Timer past zero counts up', () => {
  const warned = () =>
    [...host.querySelectorAll('[data-timer-id]')]
      .filter((el) => el.classList.contains('warn'))
      .map((el) => el.getAttribute('data-timer-id'));

  it('counts up past zero instead of sitting at 0:00', async () => {
    // The row the audit watched: a 45-second timer, four minutes and thirty-seven
    // seconds past its deadline. It used to read `0:00` and now reads how far over
    // the preacher is, which is the whole of the decision.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Offering', 0.75, at - (45_000 + 277_000))],
    });
    expect(rows()).toEqual([['Offering', '+4:37']]);
  });

  it('reads the over figure through the one reader, never its own subtraction', async () => {
    // Asserted against `countdownRemainingMs` itself rather than against a literal,
    // the same discipline as the running case above: the upward figure is the one
    // reader's answer negated and handed to the one formatter, and a second
    // subtraction that happened to agree today would still pass a literal.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    const spent = entry(1, 'Sermon', 20, at - 26 * 60_000);
    await deliver({ kind: 'timer', timers: [spent] });

    const left = countdownRemainingMs(spent, at, { past: true });
    expect(left).toBeLessThan(0);
    expect(rows()[0][1]).toBe(`+${formatCountdown(-left)}`);
  });

  it('keeps counting on the page’s existing tick', async () => {
    // A figure computed once that then stands still is the defect in a different
    // costume: `0:00` for twelve minutes and `+4:37` for twelve minutes are the
    // same failure. It rides the page's own 1000 ms tick, like every other figure
    // on it — a second interval would be a second clock.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 1, at - 90_000)] });
    expect(rows()[0][1]).toBe('+0:30');

    vi.advanceTimersByTime(1000);
    await tick();
    await tick();
    expect(rows()[0][1]).toBe('+0:31');
  });

  it('wears the warning colour the row already has, and no new one', async () => {
    // Not a fourth colour: the class is the one RG-148 gave this row, resolved in
    // the stylesheet to the countdown's own red. Amber means ON AIR, cyan means the
    // AI is guessing, amethyst means rehearsal, and a timer that is over time is
    // none of those — it is a claim about TIME. The colour itself is pinned by
    // `stageprogrow.test.js`, which reads the shipped declaration; jsdom computes
    // no layout and nothing here may claim a painted pixel.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Offering', 1, at - 120_000), entry(2, 'Sermon', 20, at)],
    });
    expect(rows()).toEqual([
      ['Offering', '+1:00'],
      ['Sermon', '20:00'],
    ]);
    expect(warned()).toEqual(['1']);
  });

  it('marks the row the moment it reaches zero, with no unmarked tick in between', async () => {
    // The boundary is the whole reason the mark is the SIGN and not the threshold
    // rule. `countdownWarning` answers false at and below zero, so a row marked
    // only by it would lose its colour at the instant the time ran out — one tick
    // of nothing, on the one surface being read mid-sermon.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 1, at - 59_000)] });
    expect(rows()[0][1]).toBe('0:01');
    expect(warned()).toEqual(['1']);

    vi.advanceTimersByTime(1000);
    await tick();
    await tick();
    expect(rows()[0][1]).toBe('+0:00');
    expect(warned(), 'the row lost its colour at the instant the time ran out').toEqual(['1']);
  });

  it('does not count up a HELD timer whose instant has passed', async () => {
    // The hold exception, at the new boundary. A timer paused at 4:00 whose
    // `countdown_to` is six minutes in the past is not ten minutes over — it is
    // stopped at four minutes, which is what the wall and the console both show.
    // The one reader already knows this, and the sign test must not get in front
    // of it.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Sermon', 20, at - 26 * 60_000, { countdown_paused_ms: 4 * 60_000 })],
    });
    expect(rows()).toEqual([['Sermon', '4:00']]);
    expect(warned()).toEqual([]);
  });

  it('paints the over figure, not the operator’s done message', async () => {
    // A DECISION, written as a test so it is not read later as an oversight. The
    // words an operator typed are what a CONGREGATION countdown says when it lands
    // — `countdown_done` replaces the digits on the wall, and the stage mirror was
    // seen showing `Welcome` in the same screenshot — and a Stage Timer is a
    // different instrument: it is the preacher's own bookkeeping, and what it is
    // asked past zero is HOW FAR OVER. `done_msg` therefore keeps having no reader
    // on this rail. If that is ever revisited it is a new row, not RG-153.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    await deliver({
      kind: 'timer',
      timers: [entry(1, 'Offering', 1, at - 120_000, { countdown_done: 'Welcome' })],
    });
    expect(rows()).toEqual([['Offering', '+1:00']]);
    expect(host.textContent).not.toContain('Welcome');
  });

  it('counts the `+` in the width budget it hands the stylesheet', async () => {
    // RG-147 IN A NEW COSTUME, and the reason this assertion is here. A `+` figure
    // is one character wider than the figure it replaces, and this row was slicing
    // a seven-character time three days ago. The budget survives it for one reason
    // only: `--tch` is the length of the longest RENDERED value, so the sign is
    // measured because it is part of the string. A budget derived from the
    // remaining milliseconds instead would be a character short.
    const at = 1_700_000_000_000;
    vi.useFakeTimers();
    vi.setSystemTime(at);
    await mount();

    const progrow = () => host.querySelector('.progrow');

    // `+4:37` — five characters where `0:00` was four.
    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 1, at - 337_000)] });
    expect(rows()[0][1]).toBe('+4:37');
    expect(progrow().style.getPropertyValue('--tch')).toBe('5');

    // An hour over: `+1:00:01`, eight characters, one more than the seven that
    // clipped at 1280 x 720 before RG-147 was closed.
    await deliver({ kind: 'timer', timers: [entry(1, 'Livestream', 1, at - 3_661_000)] });
    expect(rows()[0][1]).toBe('+1:00:01');
    expect(progrow().style.getPropertyValue('--tch')).toBe('8');

    // …and the budget still comes back when the over-time row goes.
    await deliver({ kind: 'timer', timers: [entry(1, 'Offering', 5, at)] });
    expect(rows()[0][1]).toBe('5:00');
    expect(progrow().style.getPropertyValue('--tch')).toBe('4');
  });
});

// ── THE ONE READER, ASKED FOR THE OTHER SIDE OF ZERO ──────────────────────
//
// These sit here rather than in `countdown.test.js` because the programme rail is
// the only surface that asks the question, and this file is that rail's contract.
// The opt-in exists so the upward figure is the one reader's own answer negated.
// The alternative was a subtraction on the stage page, which is precisely the
// defect `docs/REBRAND.md` phase 7 records as fixed once already.
describe('countdownRemainingMs answers past zero only when asked', () => {
  const now = 1_700_000_000_000;

  it('still floors at zero for every caller that does not ask', () => {
    // The wall reads zero as "it finished" and shows the done message. Nothing
    // about that moved, and this is the assertion that says so.
    expect(countdownRemainingMs({ countdown_to: now - 60_000 }, now)).toBe(0);
    expect(countdownRemainingMs({ countdown_to: now - 60_000 }, now, {})).toBe(0);
  });

  it('answers the negative when asked, and is still one subtraction', () => {
    expect(countdownRemainingMs({ countdown_to: now - 60_000 }, now, { past: true })).toBe(-60_000);
    expect(countdownRemainingMs({ countdown_to: now + 60_000 }, now, { past: true })).toBe(60_000);
  });

  it('keeps null meaning “there is no countdown here” on both sides', () => {
    // Null is not a large negative number. A timer that names no deadline must not
    // become a row counting up from the epoch.
    expect(countdownRemainingMs({ reference: 'John 3:16' }, now, { past: true })).toBe(null);
    expect(countdownRemainingMs(null, now, { past: true })).toBe(null);
  });

  it('keeps the hold exception in front of the sign', () => {
    expect(
      countdownRemainingMs(
        { countdown_to: now - 360_000, countdown_paused_ms: 4 * 60_000 },
        now,
        { past: true },
      ),
    ).toBe(4 * 60_000);
  });
});

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { codeOnly } from './codeonly.js';

/** The stage page's own source, for the one assertion that is about a CSS rule
 *  rather than about a rendered row. jsdom computes no layout, so a claim about
 *  the colour law is made against the declaration, not against a painted pixel. */
const SRC = readFileSync(path.resolve(__dirname, '../Stage.svelte'), 'utf8');

// ══ WAVE 4 TRACK A ═══════════════════════════════════════════════════════════
//
// The rail above renders a label and digits and nothing else. `warn_ms` rides in
// the frame (`channels::timer_frame_json`) and was deliberately left unread while
// the warning rule was being settled on the other three surfaces; `countdown_done`
// rides there too and was dropped on the floor, so a finished Stage Timer read
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

// ── THREE CASES FROM THIS SIDE ARE DELETED, NOT MERGED ──────────────────────
//
// Wave 4 asserted that a finished programme row shows the operator's words, or
// `0:00`, and that a row whose frame chose no threshold never warns. Wave 3
// asserted that the row counts up, and that the threshold falls back to the one
// shared rule. Neither branch could see the other. The consolidation settled both
// against wave 4, on evidence rather than on which ruling came second:
//
//   * `--tch` budgets every column on this rail from the widest RENDERED string, so
//     prose in that slot sizes every column to its length -- which is the
//     six-sixty-pixel-columns failure `MIN_TIMER_PX` further down exists to stop.
//     The two fixes fight the moment words are allowed in. Both fixes are correct.
//   * the no-fallback rule was argued from "this page has no Tauri bridge and never
//     reads the configured default". Wave 3's warn chain ships `warn_default_ms` on
//     the timer frame and `countdown_warn_default_ms` on content frames, and
//     `applyWarnDefault` reads them. The merge deletes the premise.
//
// Deleting rather than loosening is the point: a suite asserting both answers
// asserts neither. What a finished timer does now is held by
// `RG-153 -- a Stage Timer past zero counts up` above.
describe('a held programme row says it is held', () => {
  const at = 1_700_000_000_000;

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
    const style = codeOnly(SRC.slice(SRC.indexOf('<style>')));
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
