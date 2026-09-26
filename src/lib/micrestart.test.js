// A microphone that died on its own must come back CLEAN, not twice over.
//
// `stopCapture` is not the only way capture ends. A device that is unplugged,
// re-plugged, or taken by another application arrives as `audio://error`, and
// `capture.js` handles that event by clearing `capturing` so the microphone
// control offers Start again. It tears nothing down, because it is an event and
// not a command — so the three listeners `startCapture` attached are still live.
//
// The next Start then overwrote all three handles with new ones. The old
// listeners were not detached and could no longer be reached, so for the rest of
// the service every `stt://transcript` was delivered to TWO subscribers, every
// `detection://match` queued its suggestion twice, and the level meter was
// written twice per chunk. The operator's cure for a dead microphone was a
// console that got worse each time they used it.
//
// The fix is one line — detach on the way in as well as on the way out — and this
// test is written against the count, not against the call, because the defect was
// never a missing call. It was a handle that had nothing pointing at it.
import { describe, it, expect, beforeEach, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

// One counter per event name: how many listeners are attached RIGHT NOW.
const attached = new Map();
const listen = vi.fn(async (name) => {
  attached.set(name, (attached.get(name) ?? 0) + 1);
  return () => attached.set(name, attached.get(name) - 1);
});
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const { startCapture, stopCapture, capture } = await import('./stores/capture.js');

/** The events `startCapture` owns. `audio://error` belongs to `initAudio`. */
const OWNED = ['audio://chunk', 'stt://transcript', 'detection://match'];
const live = () => OWNED.map((n) => attached.get(n) ?? 0);

describe('starting the microphone again after it died', () => {
  // `capture.js` is a module with three module-level handles, so one test's
  // listeners outlive it. Tear them down through the real function rather than by
  // wiping the counter — a counter reset would hide exactly the leak under test.
  beforeEach(async () => {
    invoke.mockReset();
    invoke.mockResolvedValue(null);
    await stopCapture().catch(() => {});
    listen.mockClear();
    attached.clear();
  });

  it('attaches exactly one listener per event on a first start', async () => {
    await startCapture(null);
    expect(live()).toEqual([1, 1, 1]);
  });

  it('does NOT double up when a dead device is started again', async () => {
    await startCapture(null);
    // What `audio://error` does, and all it does: the operator is told, and the
    // control goes back to offering Start. Nothing is detached, because nothing
    // issued a command.
    capture.update((s) => ({ ...s, audioError: 'device unplugged', capturing: false }));

    await startCapture(null);

    expect(live()).toEqual([1, 1, 1]);
  });

  it('survives being started three times without leaking a listener', async () => {
    await startCapture(null);
    await startCapture(null);
    await startCapture(null);
    expect(live()).toEqual([1, 1, 1]);
  });

  it('still detaches everything on a clean stop', async () => {
    await startCapture(null);
    await stopCapture();
    expect(live()).toEqual([0, 0, 0]);
  });

  it('a failed stop detaches nothing, because nothing stopped', async () => {
    await startCapture(null);
    invoke.mockRejectedValue('the audio lock is poisoned');
    await stopCapture().catch(() => {});
    // `stopCapture` rethrows BEFORE any teardown (see its own note). The listeners
    // must still be attached, or the console goes deaf over a live microphone.
    expect(live()).toEqual([1, 1, 1]);
  });
});
