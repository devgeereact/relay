// THE ONE ANIMATION LOOP ON THE CONSOLE, AND WHETHER IT EVER STOPS.
//
// The Live audio card repaints its trace on a clock rather than on a delivery,
// because a time axis that only moves when audio arrives stands still through a
// stall and then jumps — which is the "running in bits" the operator reported.
// So there is a frame loop, and a frame loop is the shape of defect that makes an
// app "a bit slower now" while every screen still looks right: it does not fail,
// it just never stops.
//
// Three things have to hold, and none of them was pinned. Each is asserted by
// DRIVING the component, never by reading its source — a loop that is cancelled
// in a function nothing calls greps exactly like one that is cancelled.
//
//   1. It does not run when the microphone is not live. An idle console costs
//      nothing.
//   2. Stopping the microphone stops it. Not "schedules a stop" — the next frame
//      must not be requested.
//   3. Destroying the component stops it. A console that switches workspace, or
//      a webview that reloads, must not leave a repaint running against a canvas
//      nobody can see. This is the one that is invisible: nothing breaks, the
//      machine is just permanently a little busier than it was.
//
// WHAT IS DELIBERATELY NOT ASSERTED: a frame rate, and pixels. jsdom has no 2d
// context so `draw()` returns early, and a budget assertion on a timer would be
// flaky and then get weakened. The claim here is only ever "is a frame
// outstanding" — which is the fact that decides whether the loop is alive.
//
//   npx vitest run src/lib/dockloop.test.js
//
// CLAUDE.md "Measure before optimising" · rule 12 (the trace is the reading, so
// it is not suppressed under reduced motion) · docs/REBRAND.md §2
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

vi.mock('@tauri-apps/api/core', () => ({ invoke: async () => null }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const Dock = (await import('./Dock.svelte')).default;

// jsdom has no 2d canvas context and says so, loudly, once per repaint. `draw()`
// already returns early on a null context — this only stops the warning from
// burying the run. It does not change what is exercised: the loop is what is
// under test, not the raster.
HTMLCanvasElement.prototype.getContext = () => null;

// A REAL FRAME PUMP, not a spy. The loop re-arms itself from inside its own
// callback, so a counter that never runs the callback cannot tell a loop that
// stopped from one that is simply between frames. This runs the frames.
let pending = [];
let served = 0;
globalThis.requestAnimationFrame = (fn) => {
  pending.push(fn);
  return pending.length;
};
globalThis.cancelAnimationFrame = (id) => {
  pending = pending.filter((_, i) => i + 1 !== id);
};
/** Run whatever frames are outstanding, and report how many ran. */
function pump(rounds = 3) {
  served = 0;
  let ts = 0;
  for (let i = 0; i < rounds; i++) {
    const q = pending;
    pending = [];
    ts += 100; // well past the loop's own ~50 ms throttle
    for (const fn of q) {
      served += 1;
      fn(ts);
    }
  }
  return served;
}
/** Is a frame outstanding — i.e. is the loop still alive? */
const armed = () => pending.length > 0;

let host;
let app;
async function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Dock({ target: host, props: {} });
  await tick();
  return app;
}
async function setMic(on) {
  cap.capture.update((s) => ({ ...s, available: true, capturing: on }));
  await tick();
}

beforeEach(() => {
  pending = [];
  served = 0;
  cap.live.set(null);
  cap.meter.set({ level: 0, isVoice: false });
  cap.transcript.set({ partial: '', finals: [], finalsAt: [] });
  cap.capture.update((s) => ({
    ...s,
    available: true,
    capturing: false,
    devices: [],
    inputDevice: '',
  }));
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  pending = [];
});

describe('the Live audio trace repaints only while there is something to repaint', () => {
  it('an idle console asks for no frames at all', async () => {
    await mount();
    pump();
    expect(armed()).toBe(false);
  });

  it('going live starts it, and it keeps itself going', async () => {
    await mount();
    await setMic(true);
    expect(armed()).toBe(true);
    // It re-arms from inside its own callback: serving three frames must leave a
    // fourth outstanding, or the trace freezes after the first repaint.
    expect(pump(3)).toBe(3);
    expect(armed()).toBe(true);
  });

  it('stopping the microphone stops it — the next frame is never asked for', async () => {
    await mount();
    await setMic(true);
    pump(2);
    expect(armed()).toBe(true);
    await setMic(false);
    // Drain anything that was already in flight; nothing may re-arm behind it.
    pump(3);
    expect(armed()).toBe(false);
  });

  it('destroying the card stops it, even mid-service', async () => {
    // THE INVISIBLE ONE. Nothing breaks when this regresses — the console is
    // just permanently busier, on the machine that is also decoding speech.
    await mount();
    await setMic(true);
    pump(2);
    expect(armed()).toBe(true);
    app.$destroy();
    app = null;
    pump(3);
    expect(armed()).toBe(false);
  });

  it('however busy the card gets, exactly ONE frame is ever outstanding', async () => {
    // TWO LOOPS LOOK EXACTLY LIKE ONE. They just cost twice as much, on the
    // machine that is also decoding speech. The card is invalidated several
    // times a second in an ordinary service — every meter reading, every
    // transcript line — and none of that may add a frame.
    //
    // HONEST ABOUT WHAT THIS DOES AND DOES NOT HOLD. It was written to pin
    // `startLoop`'s `if (!raf)` guard and it does NOT: removing that guard, and
    // the `micLive !== wasLive` transition check with it, leaves this green,
    // because `micLive` is a boolean and Svelte does not invalidate a primitive
    // whose value has not changed — so the block simply never re-runs. The guard
    // is still right (it costs nothing and it is the thing that would hold if
    // `micLive` ever stopped being a primitive), it is just held by the
    // framework here rather than by this file. What this test DOES hold is the
    // invariant an operator feels: churn on the busiest card on the console adds
    // no frames. It goes red if the loop stops re-arming.
    await mount();
    await setMic(true);
    expect(pending.length).toBe(1);
    for (let i = 0; i < 8; i++) {
      cap.meter.set({ level: 0.2 + i / 100, isVoice: i % 2 === 0 });
      cap.transcript.set({ partial: `line ${i}`, finals: [], finalsAt: [] });
      // Re-assert the state the loop keys on, which is what a store write that
      // does not actually change `capturing` does on every audio event.
      cap.capture.update((s) => ({ ...s, available: true, capturing: true }));
      await tick();
      expect(pending.length).toBe(1);
    }
    pump(1);
    expect(pending.length).toBe(1);
  });
});
