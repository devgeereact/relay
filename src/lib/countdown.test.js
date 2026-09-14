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
