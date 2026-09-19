// THE TIMER WRAPPERS BELONG TO GROUP 1, AND THIS IS WHAT HOLDS THEM THERE.
//
// `capture.js`'s header splits every wrapper in two by ONE question: **can the
// congregation see the difference?** Group 1 THROWS, because a failure the caller
// cannot see is a control that lies about what it did. Group 2 swallows, because a
// failed preference is not worth interrupting a service for.
//
// A wrapper placed in a group by a comment and nothing else does not stay there.
// `stopCapture` sat in the THROWS group, swallowing, for as long as its comment
// existed — one bare `catch {}` around both the bridge import and the command — so
// a `stop_capture` that failed on a poisoned audio lock printed "Start listening"
// over a live microphone and no caller's `catch` could fire. `micstop.test.js` is
// what holds that one. This is the same thing for the five timer commands.
//
// `showTimer` is the sharpest of them: it is one of exactly two things that may put
// a timer on a congregation screen, so a failure nobody is told about is a wall an
// operator believes has a countdown on it and does not.
import { describe, it, expect, beforeEach, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { startTimer, adjustTimer, stopTimer, listTimers, showTimer } = await import(
  './stores/capture.js'
);

describe('the timer wrappers', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('startTimer names the command and hands back the identity the engine gave it', async () => {
    invoke.mockResolvedValue(7);
    await expect(
      startTimer({ minutes: 20, label: 'Sermon', doneMsg: 'Wrap up', scope: 'stage' }),
    ).resolves.toBe(7);
    expect(invoke).toHaveBeenCalledWith('start_timer', {
      minutes: 20,
      label: 'Sermon',
      doneMsg: 'Wrap up',
      scope: 'stage',
      warnMs: null,
      planItemId: null,
      // A LENGTH, NOT AN APPOINTMENT (DECISIONS §102). Absent is `null` and never
      // omitted, for the same reason `warnMs` is: the equality here is
      // exhaustive on purpose, and a field that may or may not be in the payload
      // is a field nobody is checking.
      untilMs: null,
    });
  });

  it('adjustTimer names the timer, so it moves the one under the operator’s finger', async () => {
    invoke.mockResolvedValue(null);
    await adjustTimer(7, { remainingMs: 90_000 });
    expect(invoke).toHaveBeenCalledWith('adjust_timer', {
      timerId: 7,
      remainingMs: 90_000,
      paused: null,
    });
  });

  it('stopTimer and listTimers reach their commands', async () => {
    invoke.mockResolvedValue(null);
    await stopTimer(7);
    expect(invoke).toHaveBeenCalledWith('stop_timer', { timerId: 7 });

    invoke.mockResolvedValue([{ id: 7, remaining_ms: 1000 }]);
    await expect(listTimers()).resolves.toEqual([{ id: 7, remaining_ms: 1000 }]);
    expect(invoke).toHaveBeenCalledWith('list_timers');
  });

  it('showTimer is the explicit way back onto a wall, and it says so to the engine', async () => {
    invoke.mockResolvedValue(null);
    await showTimer(7);
    expect(invoke).toHaveBeenCalledWith('show_timer', { timerId: 7, templateId: null });
  });

  // ── The group, not the wiring ───────────────────────────────────────────────
  //
  // Each of these changes what is on a screen, what a preacher is being told, or
  // what an operator believes about either. Every one of them must reach its
  // caller when it fails.
  for (const [name, fire] of [
    ['startTimer', () => startTimer({ minutes: 5, label: '', doneMsg: '', scope: 'both' })],
    ['adjustTimer', () => adjustTimer(7, { remainingMs: 60_000 })],
    ['stopTimer', () => stopTimer(7)],
    ['listTimers', () => listTimers()],
    ['showTimer', () => showTimer(7)],
  ]) {
    it(`${name} THROWS when the engine refuses — group 1`, async () => {
      invoke.mockRejectedValue({ kind: 'refused', message: 'That timer is not running.' });
      await expect(fire()).rejects.toBeTruthy();
    });
  }

  it('listTimers returns a real list rather than swallowing a failure into an empty one', async () => {
    // The trap this one is written against. An empty array is what a console with
    // no timers renders, and it is also what a swallowed failure would produce —
    // so a broken bridge would look exactly like a quiet Sunday, on the surface an
    // operator uses to find the clock that is counting down to the wrong thing.
    invoke.mockRejectedValue('the bridge is gone');
    await expect(listTimers()).rejects.toBeTruthy();
  });
});
