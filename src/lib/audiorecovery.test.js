// ── A MICROPHONE THAT WENT AWAY HAS THREE STATES, NOT ONE (RG-291) ──────────
//
// The operator: *"make sure live transcript dosent stop. as long as there is
// audio coming in.... when audio input switch, continue transcript once audio is
// dected..."*
//
// `audio.rs` now answers that: a lost device gets a bounded, backed-off run of
// re-opens, and a resume is announced by a BUFFER OF REAL AUDIO arriving rather
// than by a device opening — a device can resolve, accept `play()` and deliver
// nothing. `main.rs` emits that as `audio://recovery`.
//
// This file is the console's half. Rule 35 is the whole of it: while Relay is
// retrying, while Relay is listening again, and while Relay has given up are
// three different facts, and a shell that reads the same through all three is a
// shell that says nothing. In particular **`lost` must not clear `capturing`** —
// `audio://error` does that, correctly, because an error is the end of an
// attempt; `lost` is the attempt after it, and Relay IS still listening for the
// device. Clearing it there would put "Start listening" in front of an operator
// over a microphone that is about to come back by itself.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { degradations } from './degraded.js';

const listeners = new Map();
const invoke = vi.fn(() => Promise.resolve(null));
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: (name, fn) => {
    listeners.set(name, fn);
    return Promise.resolve(() => listeners.delete(name));
  },
}));

const { capture, initAudio } = await import('./stores/capture.js');

await initAudio();

const fire = (name, payload) => {
  const fn = listeners.get(name);
  if (!fn) throw new Error(`nothing is listening for ${name}`);
  fn({ payload });
};

describe('the console hears a microphone being recovered', () => {
  // ATTACHED ONCE, deliberately. `initAudio` is guarded against running twice
  // (that guard is the whole of rule 7's fix), so clearing the map and calling it
  // again per test leaves nothing listening and every case fails on the fixture
  // rather than on the code.
  beforeEach(() => {
    invoke.mockClear();
    capture.set({ ...get(capture), capturing: true, audioError: null, audioRecovery: null });
  });

  it('listens for the event at all', () => {
    expect(listeners.has('audio://recovery'), 'nothing is listening for audio://recovery').toBe(true);
  });

  it('a LOST device does not put "not listening" in front of the operator', () => {
    fire('audio://recovery', { state: 'lost', reason: 'device disconnected', attempt: 1, of: 5, retry_in_ms: 500 });
    const s = get(capture);
    expect(s.audioRecovery?.state).toBe('lost');
    // THE HALF THAT MATTERS. Relay is still trying; the microphone button must
    // not flip to "Start listening" over a capture that is about to resume.
    expect(s.capturing, 'a retry cleared capturing — the operator is now told to start a capture that is still running').toBe(true);
  });

  it('LISTENING says which input it came back on, because that is a separate fact', () => {
    fire('audio://recovery', { state: 'lost', reason: 'device disconnected', attempt: 1, of: 5, retry_in_ms: 500 });
    fire('audio://recovery', { state: 'listening', input: 'MacBook Pro Microphone', was_default: true, missed_ms: 1480 });
    const s = get(capture);
    expect(s.audioRecovery?.state).toBe('listening');
    expect(s.audioRecovery?.was_default).toBe(true);
    expect(s.capturing).toBe(true);
    // RG-121: a silent fallback once put Relay on a laptop microphone at the back
    // of a booth with a desk feed plugged in. "It came back" and "it came back on
    // a microphone nobody chose" may never be the same sentence.
    expect(s.audioRecovery?.input).toBe('MacBook Pro Microphone');
  });

  it('GAVE UP is the end, and only then does the microphone read as off', () => {
    fire('audio://recovery', { state: 'lost', reason: 'device disconnected', attempt: 5, of: 5, retry_in_ms: 8000 });
    fire('audio://recovery', { state: 'gave_up', reason: 'device disconnected', attempts: 5, after_ms: 15500 });
    const s = get(capture);
    expect(s.audioRecovery?.state).toBe('gave_up');
    expect(s.capturing, 'Relay has stopped trying and the console still says it is listening').toBe(false);
  });

  it('and a resume clears the state, so a healthy run carries no stale notice', async () => {
    // `audio://chunk` is attached by `startCapture`, not by `initAudio` — the hot
    // path only exists while a capture does. So this drives the real command.
    const { startCapture } = await import('./stores/capture.js');
    await startCapture();
    fire('audio://recovery', { state: 'lost', reason: 'x', attempt: 1, of: 5, retry_in_ms: 500 });
    fire('audio://recovery', { state: 'listening', input: 'Scarlett 2i2', was_default: false, missed_ms: 900 });
    expect(get(capture).audioRecovery, 'the notice was never raised').not.toBeNull();
    // A notice that stays up for the rest of the service is a notice an operator
    // learns to read past, which costs the next real one its meaning.
    fire('audio://chunk', { rms: 0.1, is_voice: true, peaks: [] });
    expect(get(capture).audioRecovery, 'the notice outlived the recovery').toBeNull();
  });
});

describe('and what is reduced right now says so', () => {
  it('a microphone being retried is a degradation, named and with a fix', () => {
    const rows = degradations({ audioRecovery: { state: 'lost', reason: 'device disconnected', attempt: 2, of: 5, retry_in_ms: 1000 } });
    const row = rows.find((r) => r.id === 'audiorecovery');
    expect(row, '§45 does not know a microphone has gone').toBeTruthy();
    expect(row.level).toBe('reduced');
    expect(row.title.toLowerCase()).toMatch(/microphone/);
    expect(row.what.toLowerCase()).toMatch(/transcrib|transcript|listening|detect/);
    expect(row.fix).toBeTruthy();
  });

  it('having given up is worse than retrying, and reads differently', () => {
    const rows = degradations({ audioRecovery: { state: 'gave_up', reason: 'device disconnected', attempts: 5, after_ms: 15500 } });
    const row = rows.find((r) => r.id === 'audiorecovery');
    expect(row, 'giving up is not reported at all').toBeTruthy();
    expect(row.level, 'giving up reads the same as still trying').toBe('blocked');
  });

  it('and a microphone that came back on the wrong input still says so', () => {
    // The row must not go quiet on `listening` when the input is not the one that
    // was chosen. That is RG-121's whole finding, on the recovery path.
    const rows = degradations({
      audioRecovery: { state: 'listening', input: 'MacBook Pro Microphone', was_default: true, missed_ms: 1200 },
      inputDevice: 'Scarlett 2i2',
    });
    const row = rows.find((r) => r.id === 'audiorecovery');
    expect(row, 'Relay resumed on a microphone nobody chose and said nothing').toBeTruthy();
    expect(row.what).toMatch(/MacBook Pro Microphone/);
  });

  it('a healthy capture reports nothing here', () => {
    expect(degradations({}).find((r) => r.id === 'audiorecovery')).toBeUndefined();
    expect(
      degradations({ audioRecovery: { state: 'listening', input: 'Scarlett 2i2', was_default: false, missed_ms: 800 } })
        .find((r) => r.id === 'audiorecovery'),
      'a clean resume on the chosen input is not a degradation',
    ).toBeUndefined();
  });
});
