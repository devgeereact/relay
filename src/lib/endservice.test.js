// A CONTRACT STATED IN A COMMENT IS NOT A CONTRACT.
//
// `endService` swallowed every failure in a bare `catch {}`, and `History.svelte`
// then called `refresh()` and repainted an unchanged list, so a refused
// `end_service` left the operator believing the record was closed. The truth was
// on the page the whole time — the service lock is re-read afterwards, and the
// dock's End service button stays amber — in a different section, in smaller
// type, and only after the next poll.
//
// `end_service` really can fail: it takes `session.0.lock()?`, so a thread that
// panicked while holding the session mutex leaves it poisoned. The same failure
// shape that produced the `stopCapture` bug (`micstop.test.js`), one door along.
//
// WHICH GROUP. `capture.js`'s header files a wrapper by one question: can the
// congregation see the difference? `end_service` does not change what is on a
// screen — but it does `lock.release()`, and the service lock is the list of
// things Relay is currently REFUSING to do (deletions, speech-model changes,
// imports). An operator who presses End current service and is not told it
// failed is left with a console that goes on refusing, for a reason that has
// scrolled out of view. That is GROUP 1: the caller must handle it and say so.
//
// Both of its rendered callers already can. `Dock.svelte::run` wraps every dock
// action in `catch (e) { err = humanError(e) }`, and `History.svelte` now does
// the same beside its own button.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { endService, serviceLock } = await import('./stores/capture.js');

describe('ending a service cannot report a success it did not achieve', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('THROWS when the backend refused — the caller must be able to say so', async () => {
    invoke.mockRejectedValue(new Error('the session lock is poisoned'));
    await expect(endService()).rejects.toBeTruthy();
  });

  it('does not re-read the service lock over a failure', async () => {
    // Repainting an unchanged surface is how the original defect read as
    // success. A failed end leaves the lock exactly as it was, and the shell's
    // own five-second poll is what corrects it — not this wrapper pretending
    // the press did something.
    serviceLock.set({ engaged: true, held_back: ['deleting a song'] });
    invoke.mockRejectedValue(new Error('the session lock is poisoned'));
    await endService().catch(() => {});
    expect(invoke.mock.calls.map(([c]) => c)).not.toContain('service_lock');
    expect(get(serviceLock).engaged).toBe(true);
  });

  it('resolves and reads the lock back when the command succeeds', async () => {
    serviceLock.set({ engaged: true, held_back: ['deleting a song'] });
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'end_service') return null;
      if (cmd === 'service_lock') return { engaged: false, held_back: [] };
      throw new Error(`unexpected ${cmd}`);
    });
    await expect(endService()).resolves.toBeUndefined();
    expect(invoke.mock.calls.map(([c]) => c)).toContain('service_lock');
    expect(get(serviceLock).engaged).toBe(false);
  });
});
