// THE WIZARD'S MICROPHONE MUST NOT BE DECLARED OFF BEFORE IT IS OFF.
//
// This is `micstop.test.js`'s bug, one door along, on the surface whose entire
// purpose is proving to a volunteer that the microphone does what the screen says.
//
// `stopMicTest()` cleared its own `micOn` flag BEFORE awaiting `stopCapture()`:
//
//   async function stopMicTest() {
//     if (!micOn) return;
//     micOn = false;          // <-- before the backend has confirmed anything
//     try { await stopCapture(); } catch (e) { error = humanError(e); }
//
// `stopCapture` rejects when the microphone did not actually stop — it takes a
// lock, and an audio thread that panicked while holding it leaves the mutex
// poisoned and the engine running. So a failed stop left `micOn` reading "off"
// over a live microphone, and the `if (!micOn) return;` guard above then refused
// every subsequent attempt. The wizard could never close a microphone it had
// opened, and said nothing further about it.
//
// The fix is one line of ordering: clear the flag only after the await resolves.
// This test drives the real component, because the bug is in its state machine
// and a source-text assertion would pin the shape rather than the behaviour.
//
//   CLAUDE.md · "A contract stated in a comment is not a contract"

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { writable } from 'svelte/store';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const stopCapture = vi.fn();
const startCapture = vi.fn(async () => {});

vi.mock('./stores/capture.js', () => ({
  capture: writable({
    available: true,
    capturing: false,
    devices: [{ name: 'Desk feed', is_default: true }],
    inputDevice: 'Desk feed',
    stt: { loaded: true, model: 'ggml-base.bin', language: null },
    detectedLang: null,
    detectionOn: true,
  }),
  meter: writable({ level: 0, isVoice: false }),
  listMonitors: vi.fn(async () => []),
  listOutputChannels: vi.fn(async () => []),
  setChannelDisplay: vi.fn(async () => {}),
  openChannelOutput: vi.fn(async () => {}),
  setInputDevice: vi.fn(async () => {}),
  startCapture: (...a) => startCapture(...a),
  stopCapture: (...a) => stopCapture(...a),
  setDetection: vi.fn(async () => {}),
  setSttLanguage: vi.fn(async () => {}),
  manualFire: vi.fn(async () => {}),
}));

const FirstRun = (await import('./FirstRun.svelte')).default;

let host;
let app;
const btn = (label) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === label);
const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  stopCapture.mockReset();
  startCapture.mockClear();
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new FirstRun({ target: host });
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
});

describe("the first-run wizard's microphone", () => {
  it('does not open a second microphone over one that never closed', async () => {
    // A stop that does not stop — a poisoned audio lock, the case the whole
    // throw-vs-swallow contract exists for.
    stopCapture.mockRejectedValue(new Error('audio engine lock poisoned'));

    btn('Continue').click(); // welcome -> screen
    await settle();
    btn('Continue').click(); // screen -> audio, which opens the microphone
    await settle();
    expect(startCapture).toHaveBeenCalledTimes(1);

    // Leaving the audio step must close the microphone. It fails, loudly.
    btn('Back').click();
    await settle();
    expect(stopCapture).toHaveBeenCalledTimes(1);
    expect(host.textContent).toMatch(/lock poisoned|microphone/i);

    // Back onto the audio step. The microphone from the first visit is STILL OPEN,
    // so the wizard must not open another one on top of it.
    //
    // This is the assertion the bug fails. Clearing `micOn` before the await meant
    // the failed stop left the flag reading "off", so re-entering the step sailed
    // past its `if (micOn) return;` guard and called `startCapture` a second time —
    // two live captures, one meter, and the wizard's proof-of-microphone now
    // showing a device that is not necessarily the one it claims.
    btn('Continue').click();
    await settle();
    expect(startCapture).toHaveBeenCalledTimes(1);
  });

  it('stops asking once the microphone is genuinely closed', async () => {
    stopCapture.mockResolvedValue(undefined);

    btn('Continue').click();
    await settle();
    btn('Continue').click(); // audio: mic on
    await settle();
    btn('Back').click(); // stops cleanly
    await settle();
    expect(stopCapture).toHaveBeenCalledTimes(1);

    // Nothing is open now, so leaving a step must not call the backend again —
    // the guard still has to work in the ordinary case.
    btn('Back').click();
    await settle();
    expect(stopCapture).toHaveBeenCalledTimes(1);

    // ...and re-entering the step DOES reopen it, because it really was closed.
    btn('Continue').click();
    await settle();
    btn('Continue').click();
    await settle();
    expect(startCapture).toHaveBeenCalledTimes(2);
  });
});

// ── RG-61 · the wizard has to hand off to the three things it does not do ────
//
// The onboarding gap (brief §59/60) was never a missing feature: the drills, the
// six-stage path check and rehearsal all shipped. It was that a volunteer who has
// just finished the wizard has no way to learn any of them exist, and the wizard
// is the last moment anybody is guaranteed to be looking.
//
// It is deliberately a HAND-OFF and not three more steps. This wizard's own rule
// is that it asks as little as it can — Welcome and Finish ask nothing — and each
// of these is a thing to do on another day, not an answer to give now.
//
// Asserted on the source rather than by mounting, because the block lives on the
// last step and reaching it means driving five steps of device and model state
// this suite deliberately does not fake.
describe('RG-61 · the last step names what to do before the first Sunday', () => {
  const SRC = readFileSync(resolve(process.cwd(), 'src/lib/FirstRun.svelte'), 'utf8').replace(
    /\s+/g,
    ' ',
  );

  it('names all three, and where each one lives', () => {
    // The where is the whole point: naming the instrument without naming the tab
    // is the same as not naming it.
    expect(SRC).toMatch(/Practise/);
    expect(SRC).toMatch(/drills on the <b>Help<\/b> tab/);
    expect(SRC).toMatch(/Settings → Dashboard/);
    expect(SRC).toMatch(/Rehearse<\/b> on the Live tab/);
  });

  it('says the path check is about the whole chain, not the parts the wizard just set up', () => {
    // Without this sentence the step reads as a duplicate of the wizard, and an
    // operator who has just seen a verse hit the screen will skip it.
    expect(SRC).toMatch(/six stages between the microphone and the screen/);
    expect(SRC).toMatch(/can pass while the chain still does not work end to end/);
  });

  it('tells the operator this is not their last chance to find them', () => {
    // An operator who dismisses a wizard and then cannot find the setting again
    // has been actively harmed by it — the wizard's own header says so.
    expect(SRC).toMatch(/not your last chance to find them/);
  });

  it('adds no step — the hand-off is on the finish step, not a sixth question', () => {
    const steps = SRC.match(/const STEPS = \[(.*?)\];/)?.[1] ?? '';
    expect((steps.match(/key: '/g) ?? []).length).toBe(6);
  });
});
