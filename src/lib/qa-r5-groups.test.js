// R5 · THE THROW-vs-SWALLOW CONTRACT, HELD BY A TEST RATHER THAN BY A COMMENT.
//
// `capture.js`'s header names nine GROUP 1 wrappers — the ones that change what is
// on the screens, what the AI is allowed to do, or whether the microphone is live.
// `stopCapture` sat in that list while swallowing for as long as the list existed,
// and `micstop.test.js` was written to pin ONE of the nine. The other eight are
// still held by prose, and prose is what failed the first time.
//
//   "A contract stated in a comment is not a contract." — CLAUDE.md, testing §
//   "When you place a wrapper in a group, add the test that holds it there."
//
// So: the whole group, in one loop. Every one of them must reject when the backend
// rejects, because every caller wraps them in `catch (e) { flash(humanError(e)) }`
// and a swallowed rejection makes every one of those handlers dead code.
//
// The second half of the file is the finding: a control that governs what the AI
// may put on a wall unasked, which swallows AND returns a value that says it
// worked.

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const store = await import('./stores/capture.js');

beforeEach(() => {
  invoke.mockReset();
  store.capture.update((s) => ({ ...s, available: true }));
  store.live.set(null);
});

// The nine the header names, with the least-surprising argument each needs.
// `startCapture` is excluded from the loop and driven separately: it calls
// `start_service` first inside its own deliberate try/catch, so a blanket reject
// has to be distinguished from the one call that is allowed to fail.
const GROUP_1 = {
  manualFire: () => store.manualFire('John 3:16'),
  confirmDetection: () => store.confirmDetection('John 3:16'),
  setDetection: () => store.setDetection(false),
  setRehearsal: () => store.setRehearsal(true),
  navVerse: () => store.navVerse('next'),
  stopCapture: () => store.stopCapture(),
  fireContent: () => store.fireContent('Notice', 'Car park is blocked'),
  startCountdown: () => store.startCountdown(5),
  // Not in the header's list, but it is the most literal member of the group:
  // it paints over live scripture on every screen at once, for a fire alarm.
  pushAnnouncement: () => store.pushAnnouncement('Please leave by the side door'),
};

describe('GROUP 1 — every wrapper that can change what a congregation sees THROWS', () => {
  for (const [name, call] of Object.entries(GROUP_1)) {
    it(`${name} rejects when the backend rejects`, async () => {
      invoke.mockRejectedValue('the audio lock is poisoned');
      await expect(
        call(),
        `${name} is filed in GROUP 1 in capture.js's header. It swallowed a backend ` +
          `failure, so every caller's catch(e) { flash(humanError(e)) } is dead code ` +
          `and the operator is told something happened that did not.`,
      ).rejects.toBeTruthy();
    });
  }

  it('startCapture rejects when start_capture itself fails', async () => {
    // `start_service` is best-effort by design and has its own catch. The command
    // that opens the microphone is not.
    invoke.mockImplementation((cmd) =>
      cmd === 'start_capture'
        ? Promise.reject('device is in use by another application')
        : Promise.resolve(1),
    );
    await expect(store.startCapture(null)).rejects.toBeTruthy();
    expect(get(store.capture).capturing).toBe(false);
  });
});

describe('GROUP 3 — the panic controls report, never throw', () => {
  it('clearScreens returns false and raises panicError when the clear failed', async () => {
    invoke.mockRejectedValue('output window is gone');
    store.panicError.set(null);
    await expect(store.clearScreens()).resolves.toBe(false);
    expect(get(store.panicError)).toMatch(/congregation may still be seeing/i);
  });

  it('blackScreen does the same', async () => {
    invoke.mockRejectedValue('output window is gone');
    store.panicError.set(null);
    await expect(store.blackScreen()).resolves.toBe(false);
    expect(get(store.panicError)).toBeTruthy();
  });
});

// ── THE FINDING ─────────────────────────────────────────────────────────────
//
// `setSensitivity` is the operator's single dial for how willing the AI is to put
// a verse on the wall WITHOUT asking. DECISIONS §32.2 is entirely about this dial
// not sticking, and the Rust side was rebuilt so that a dial move sets the gate,
// re-anchors the learned baseline, and is persisted onto the active profile.
//
// The frontend wrapper swallows every failure — and does something worse than
// swallow. Its doc says "Returns the landed dial position", and on failure it
// returns the position the caller ASKED for. `Live.svelte::onSensitivity` writes
// its slider from the request either way, so the dial reads 80 while the router
// is still gated at 50, with nothing anywhere saying so.
//
// `set_sensitivity` really can fail: `routing.0.lock()?` on a poisoned router
// mutex, which is the same failure shape that produced the `stopCapture` bug.
//
// This is the exact rule that `clearScreens` and `stopCapture` were each fixed
// for, on a third control: a control may not report a success it did not achieve.
describe('the sensitivity dial may not report a position it did not reach', () => {
  it('returns what the BACKEND landed on when the call succeeds', async () => {
    invoke.mockImplementation((cmd) =>
      cmd === 'set_sensitivity'
        ? Promise.resolve(78)
        : Promise.resolve({ auto_fire: 0.7, suggest: 0.5 }),
    );
    await expect(store.setSensitivity(80)).resolves.toBe(78);
  });

  it('THROWS rather than claiming the requested position when the backend refused', async () => {
    // FIXED 2026-08-14 (R5-4). It was GROUP 2 with `catch { return sensitivity; }`
    // — the value the operator ASKED for, returned as if it had landed, under a
    // doc comment promising "the landed dial position". Third wrapper repaired for
    // the rule behind DECISIONS §20, and on the one control that governs what the
    // AI may put on a wall without asking.
    invoke.mockRejectedValue('the router lock is poisoned');
    await expect(
      store.setSensitivity(80),
      'setSensitivity must not resolve at all when the gate did not move',
    ).rejects.toBeTruthy();
  });
});

// ── A SECOND, SMALLER ONE ───────────────────────────────────────────────────
//
// `setStageNext` publishes to (and clears) the preacher's stage monitor — a real
// screen, on a stand, in front of a person. It is GROUP 2 (swallows). A failed
// CLEAR is the bad direction: `Live.svelte` calls `setStageNext(null, null)` to
// take the "up next" panel down, and if that call fails the preacher keeps reading
// a stale next-verse for the rest of the service with nothing reporting it.
describe('the stage monitor is a screen, and clearing it can fail silently', () => {
  it('setStageNext surfaces a failed clear', async () => {
    // FIXED 2026-08-14 (R5-8). Moved GROUP 2 → GROUP 1, which is a correction to
    // the GROUP 2 RULE as much as to this wrapper: its test is "can the
    // congregation see the difference?", and here the answer is "no, but the
    // preacher can, and he is the one acting on it". Throwing makes each call site
    // state its choice — the push after a fire shrugs deliberately, the clear does
    // not.
    invoke.mockRejectedValue('kiosk hub is gone');
    await expect(
      store.setStageNext(null, null),
      'clearing the preacher\'s "up next" panel failed and nothing anywhere says ' +
        'so — the stage monitor keeps showing the previous verse.',
    ).rejects.toBeTruthy();
  });
});
