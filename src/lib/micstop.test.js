// Stopping the microphone must never report a stop it did not achieve.
//
// This is the same class of bug as `panic.test.js`, one door along. `stopCapture`
// wrapped BOTH the bridge import and the `stop_capture` command in one bare catch:
//
//   try {
//     const call = await invoke();
//     await call('stop_capture');
//   } catch {
//     /* backend gone — nothing to stop */
//   }
//   … detach listeners …
//   capture.update((s) => ({ ...s, capturing: false }));
//
// The comment is true of exactly one case — a plain browser, where the import fails
// and there was never an engine. It was also catching a real failure of the real
// command, and `stop_capture` can fail: it takes a lock, so an audio thread that
// panicked while holding it leaves the mutex poisoned and the engine running.
//
// Every caller wraps `stopCapture()` in `catch (e) { … humanError(e) }`, and none of
// them could ever fire. The console detached its listeners, printed "Start
// listening", and hid the microphone indicator — over a live microphone, with
// detection still auto-firing to the congregation behind it.
//
// `capture.js`'s own header puts `stopCapture` in GROUP 1 — THROWS, on the grounds
// that it changes "whether the microphone is live". It was the one member of that
// group not keeping the contract.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { stopCapture, capture } = await import('./stores/capture.js');

describe('stopping the microphone', () => {
  beforeEach(() => {
    invoke.mockReset();
    capture.update((s) => ({ ...s, available: true, capturing: true }));
  });

  it('calls the backend and marks the microphone off when it really stopped', async () => {
    invoke.mockResolvedValue(null);
    await expect(stopCapture()).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('stop_capture');
    expect(get(capture).capturing).toBe(false);
  });

  it('THROWS when the backend could not stop — the caller must be able to say so', async () => {
    invoke.mockRejectedValue('the audio lock is poisoned');
    await expect(stopCapture()).rejects.toBeTruthy();
  });

  it('leaves the microphone reading LIVE when the stop failed', async () => {
    // The half that matters. A rejection nobody can see is only half the lie; the
    // other half is the console quietly redrawing itself as "not listening" over a
    // microphone that is still open. If the engine did not stop, the UI must not
    // claim it did — and the operator can press Stop again.
    invoke.mockRejectedValue('the audio lock is poisoned');
    await stopCapture().catch(() => {});
    expect(get(capture).capturing).toBe(true);
  });
});
