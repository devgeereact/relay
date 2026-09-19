// THE COUNTDOWN TRANSPORT (docs/REBRAND.md §7).
//
// Every test here is about a number that ends up on a wall in front of people, so
// the interesting ones are the refusals rather than the arithmetic.
//
//   npx vitest run src/lib/countdown.test.js

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
  countdownPress,
  countdownCan,
  countdownSet,
  msFromFields,
  fieldsFromMs,
  DEFAULT_COUNTDOWN_MS,
  COUNTDOWN_STEP_MS,
  MAX_COUNTDOWN_MS,
  MIN_BROADCAST_MS,
  countdownRemainingMs,
  countdownIsPaused,
  countdownTotalMs,
} from './countdown.js';

const MIN = 60_000;

describe('hh : mm : ss', () => {
  it('reads the three fields as one duration', () => {
    expect(msFromFields(1, 2, 3)).toBe((3600 + 120 + 3) * 1000);
  });

  it('a blank or nonsense field is zero, never NaN — a NaN reaches the backend as 5:00', () => {
    expect(msFromFields('', 5, '')).toBe(5 * MIN);
    expect(msFromFields('abc', null, undefined)).toBe(0);
    expect(msFromFields(-4, 5, 0)).toBe(5 * MIN);
  });

  it('caps at twelve hours, so a mistyped hour cannot put a nine-hour timer up', () => {
    expect(msFromFields(99, 0, 0)).toBe(MAX_COUNTDOWN_MS);
  });

  it('round-trips', () => {
    expect(fieldsFromMs(msFromFields(2, 34, 56))).toEqual({ h: 2, m: 34, s: 56 });
    expect(fieldsFromMs(5 * MIN)).toEqual({ h: 0, m: 5, s: 0 });
  });
});

describe('Start', () => {
  it('puts the set length on the screens', () => {
    expect(countdownPress('start', 7 * MIN, null)).toEqual({
      setMs: 7 * MIN,
      broadcastMs: 7 * MIN,
      pause: null,
      refused: null,
    });
  });

  it('refuses a second countdown over a running one, and says why', () => {
    const r = countdownPress('start', 7 * MIN, 90_000);
    expect(r.broadcastMs).toBe(null);
    expect(r.refused).toMatch(/already running/i);
  });

  it('refuses a countdown of zero rather than letting the backend substitute five minutes', () => {
    const r = countdownPress('start', 0, null);
    expect(r.broadcastMs).toBe(null);
    expect(r.refused).toMatch(/length/i);
  });
});

describe('Reset', () => {
  it('puts the RUNNING countdown back to its full length', () => {
    expect(countdownPress('reset', 10 * MIN, 42_000).broadcastMs).toBe(10 * MIN);
  });

  // Reset must not become a second Start. Start is the one control that puts a
  // countdown in front of a congregation, and it should stay the one.
  it('does nothing when nothing is counting down', () => {
    const r = countdownPress('reset', 10 * MIN, null);
    expect(r.broadcastMs).toBe(null);
    expect(r.refused).toMatch(/nothing/i);
  });
});

describe('± one minute', () => {
  it('moves the TOOL when nothing is running, and touches no screen', () => {
    expect(countdownPress('plus', 5 * MIN, null)).toEqual({
      setMs: 6 * MIN,
      broadcastMs: null,
      pause: null,
      refused: null,
    });
    expect(countdownPress('minus', 5 * MIN, null).setMs).toBe(4 * MIN);
  });

  it('moves the WALL when something is running, and leaves the tool alone', () => {
    const r = countdownPress('plus', 5 * MIN, 90_000);
    expect(r.setMs).toBe(5 * MIN);
    expect(r.broadcastMs).toBe(90_000 + COUNTDOWN_STEP_MS);
  });

  it('never takes the tool below zero', () => {
    expect(countdownPress('minus', 30_000, null).setMs).toBe(0);
  });

  it('caps the wall at twelve hours', () => {
    expect(countdownPress('plus', 5 * MIN, MAX_COUNTDOWN_MS).broadcastMs).toBe(MAX_COUNTDOWN_MS);
  });

  // THE ONE THAT MATTERS. `start_countdown` in Rust reads `minutes` and falls back
  // to FIVE MINUTES when it is not greater than zero, so "−1" with fifty seconds
  // left would not show 0:00 — it would put 5:00 on the wall, which is the
  // opposite of what was pressed and is worse than doing nothing.
  it('refuses to take a nearly-finished countdown below zero', () => {
    const r = countdownPress('minus', 5 * MIN, 50_000);
    expect(r.broadcastMs).toBe(null);
    expect(r.refused).toMatch(/less than a minute/i);
    expect(MIN_BROADCAST_MS).toBeGreaterThan(0);
  });
});

describe('Clear', () => {
  // §7's one emphasis: "Clear resets it without removing the tool". And the thing
  // §7 does not say, which matters more in a dark booth: it is not Clear screens.
  it('returns the tool to its default and touches NO screen', () => {
    expect(countdownPress('clear', 47 * MIN, 42_000)).toEqual({
      setMs: DEFAULT_COUNTDOWN_MS,
      broadcastMs: null,
      pause: null,
      refused: null,
    });
  });

  it('works while a countdown is running, and still does not clear it', () => {
    expect(countdownPress('clear', 9 * MIN, 120_000).broadcastMs).toBe(null);
  });
});

describe('what the buttons may say about themselves', () => {
  // A disabled control and the refusal it would have given come from one place, so
  // they cannot disagree — "a status badge that cannot detect its own failure"
  // (CLAUDE.md rule 35) in its smallest form.
  it('Start is available only when nothing is running and a length is set', () => {
    expect(countdownCan('start', 5 * MIN, null)).toBe(true);
    expect(countdownCan('start', 5 * MIN, 30_000)).toBe(false);
    expect(countdownCan('start', 0, null)).toBe(false);
  });

  it('Reset is available only while something is running', () => {
    expect(countdownCan('reset', 5 * MIN, 30_000)).toBe(true);
    expect(countdownCan('reset', 5 * MIN, null)).toBe(false);
  });

  it('− is unavailable in the last minute', () => {
    expect(countdownCan('minus', 5 * MIN, 30_000)).toBe(false);
    expect(countdownCan('minus', 5 * MIN, 5 * MIN)).toBe(true);
  });

  it('Clear is always available', () => {
    expect(countdownCan('clear', 0, null)).toBe(true);
    expect(countdownCan('clear', 5 * MIN, 30_000)).toBe(true);
  });
});

// ── PAUSE, AND THE ONE READER IT FORCED ─────────────────────────────────────
//
// `docs/REBRAND.md` §7 asks for Start/**Pause** · Reset · ±1 · Clear. Four of the
// five were built and Pause was not, for a reason the spec records honestly:
// `countdown_to` is an absolute instant, so every other press is a re-aim, and there
// is no instant that means "not moving". Pause needed a field of its own
// (`countdown_paused_ms`) and, with it, ONE reader that knows the exception —
// because three surfaces each doing their own subtraction is survivable only while
// the subtraction has no exception.

describe('how long is left — the one reader', () => {
  it('a running countdown is the gap to its instant', () => {
    const now = 1_700_000_000_000;
    expect(countdownRemainingMs({ countdown_to: now + 90_000 }, now)).toBe(90_000);
  });

  it('a HELD countdown is the held figure, and the instant is ignored entirely', () => {
    const now = 1_700_000_000_000;
    // The instant is in the PAST, which is the ordinary case: hold at 4:00 and the
    // preacher talks for ten minutes. Read as an instant this countdown finished six
    // minutes ago; read correctly it still says 4:00, which is what the wall, the
    // stage rail and the transport must all show.
    expect(
      countdownRemainingMs({ countdown_to: now - 360_000, countdown_paused_ms: 4 * MIN }, now),
    ).toBe(4 * MIN);
    expect(countdownIsPaused({ countdown_paused_ms: 4 * MIN })).toBe(true);
    expect(countdownIsPaused({ countdown_to: now + 1000 })).toBe(false);
  });

  // Zero and null are DIFFERENT answers and the renderer needs both: zero shows the
  // operator's done message, null shows nothing at all. Collapsing them is how a
  // "Welcome" caption appears over a verse.
  it('a finished countdown is zero; no countdown at all is null', () => {
    const now = 1_700_000_000_000;
    expect(countdownRemainingMs({ countdown_to: now - 1 }, now)).toBe(0);
    expect(countdownRemainingMs({ reference: 'John 3:16' }, now)).toBe(null);
    expect(countdownRemainingMs(null, now)).toBe(null);
  });

  // The span the warning rule works from. It was read by the renderer and written by
  // nothing for the whole life of the field, so §7's short-countdown rule had never
  // once fired in the product.
  it('the aimed length is to − from, and null rather than a guess when either is missing', () => {
    expect(countdownTotalMs({ countdown_to: 1_000_000, countdown_from: 700_000 })).toBe(300_000);
    expect(countdownTotalMs({ countdown_to: 1_000_000 })).toBe(null);
    expect(countdownTotalMs({ countdown_from: 700_000 })).toBe(null);
    // A span that has gone backwards is an absence, not a negative length.
    expect(countdownTotalMs({ countdown_to: 500, countdown_from: 900 })).toBe(null);
  });
});

describe('Pause and Resume', () => {
  it('hold asks for the hold and moves no number', () => {
    const r = countdownPress('pause', 5 * MIN, 90_000, false);
    expect(r.pause).toBe(true);
    expect(r.broadcastMs).toBe(null);
    expect(r.setMs).toBe(5 * MIN);
    expect(r.refused).toBe(null);
  });

  it('release asks for the release and moves no number', () => {
    const r = countdownPress('resume', 5 * MIN, 90_000, true);
    expect(r.pause).toBe(false);
    expect(r.broadcastMs).toBe(null);
  });

  // The same rule as Reset: this transport is about the countdown that is already
  // there, and Start is the only control that puts one in front of people.
  it('neither can put a countdown on a screen', () => {
    expect(countdownPress('pause', 5 * MIN, null).refused).toBe('Nothing is counting down.');
    expect(countdownPress('resume', 5 * MIN, null).refused).toBe('Nothing is counting down.');
    expect(countdownPress('pause', 5 * MIN, null).pause).toBe(null);
    expect(countdownCan('pause', 5 * MIN, null)).toBe(false);
    expect(countdownCan('resume', 5 * MIN, null)).toBe(false);
  });

  it('each refuses the state it is already in, and says which', () => {
    expect(countdownPress('pause', 5 * MIN, 90_000, true).refused).toMatch(/already paused/);
    expect(countdownPress('resume', 5 * MIN, 90_000, false).refused).toMatch(/already counting/);
    expect(countdownCan('pause', 5 * MIN, 90_000, true)).toBe(false);
    expect(countdownCan('resume', 5 * MIN, 90_000, true)).toBe(true);
  });

  // THE ONE THAT MATTERS. A press of ±1 on a held countdown changes the number and
  // must not release the hold — `pause: null` means "leave it exactly as it is". The
  // opposite would start a timer running that the operator deliberately stopped, in
  // front of a congregation, from a button that says "+1".
  it('every other press leaves the hold exactly as it is', () => {
    for (const action of ['start', 'reset', 'plus', 'minus', 'clear']) {
      expect(countdownPress(action, 5 * MIN, 4 * MIN, true).pause).toBe(null);
      expect(countdownPress(action, 5 * MIN, 4 * MIN, false).pause).toBe(null);
    }
  });

  // Reset and ±1 are about the countdown on the wall, and a held countdown IS on the
  // wall. They must keep working while it is held.
  it('Reset and ±1 still work on a held countdown', () => {
    expect(countdownCan('reset', 5 * MIN, 4 * MIN, true)).toBe(true);
    expect(countdownPress('plus', 5 * MIN, 4 * MIN, true).broadcastMs).toBe(5 * MIN);
    expect(countdownPress('minus', 5 * MIN, 4 * MIN, true).broadcastMs).toBe(3 * MIN);
  });
});

describe('the set duration outlives the dock', () => {
  // `{#if !liveFullscreen}<Dock />{/if}` — pressing Full screen destroys the dock
  // and pressing it again builds a NEW one. A component-local `let` would lose
  // whatever the operator had typed, silently, mid-service.
  it('is a module store, so a remounted dock reads back what was set', () => {
    countdownSet.set(11 * MIN);
    expect(get(countdownSet)).toBe(11 * MIN);
    countdownSet.set(DEFAULT_COUNTDOWN_MS);
  });
});

// ── A HOLD PAST ZERO (RG-175, DECISIONS §99's open half) ───────────────────
//
// `paused_ms` was positive by contract, and this reader enforced it with
// `held > 0`. That is what DECISIONS §99 meant by a hold past zero being
// "structurally inexpressible": a stage timer two minutes over answered its
// deadline instead of its held figure, so `Stage.svelte`'s `held` row — real,
// styled and tested since wave 4 — could never be produced by anything.
//
// The contract is now SIGNED, and the audience reading is what keeps the old
// guarantee: a held figure is clamped at zero unless the caller opted into
// `past`, exactly as a running one is. A congregation wall reads zero as "it
// finished" and paints the done message; `-2:00` in front of a room is not a
// thing anybody asked for.
//
// **The `> 0` was also doing something nobody wrote down.** `Number(null)` is
// `0`, so a `countdown_paused_ms: null` — which is what the wire sends for a
// timer that is NOT held — fell through on the `> 0` and read as running by
// accident. Widen the check to "is it finite" and every unheld timer freezes at
// `0:00`. Hence the explicit null guard, and hence these tests.
describe('holding a clock that has already run out', () => {
  const overrun = { countdown_to: 1_000, countdown_paused_ms: -120_000 };

  it('answers the negative figure the preacher is reading', () => {
    expect(countdownRemainingMs(overrun, 999_999, { past: true })).toBe(-120_000);
  });

  it('clamps it for a congregation, where zero means the done message', () => {
    expect(countdownRemainingMs(overrun, 999_999)).toBe(0);
  });

  it('counts a negative hold as held, or the rail cannot render it frozen', () => {
    expect(countdownIsPaused(overrun)).toBe(true);
  });

  it('still counts a positive hold as held, and answers it either way', () => {
    const held = { countdown_to: 1_000, countdown_paused_ms: 90_000 };
    expect(countdownIsPaused(held)).toBe(true);
    expect(countdownRemainingMs(held, 999_999)).toBe(90_000);
    expect(countdownRemainingMs(held, 999_999, { past: true })).toBe(90_000);
  });

  // THE TRAP. `Number(null) === 0`, and `0` is finite.
  it.each([
    ['null', null],
    ['undefined', undefined],
    ['absent', 'ABSENT'],
  ])('does not call a timer held when paused_ms is %s', (_name, value) => {
    const c = { countdown_to: 2_000_000 };
    if (value !== 'ABSENT') c.countdown_paused_ms = value;
    expect(countdownIsPaused(c)).toBe(false);
    // …and it must still count down rather than freezing at that phantom zero.
    expect(countdownRemainingMs(c, 1_000_000)).toBe(1_000_000);
  });
});
