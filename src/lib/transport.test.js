// The panic keys must reset the TRANSPORT, not just the screens.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

// No Tauri in the test env — every wrapper falls into its catch and is a no-op,
// which is exactly the path we want: the transport reset must happen REGARDLESS of
// whether the backend call succeeds. A panic key that half-works is worse than one
// that doesn't work at all.
import { liveCue, clearScreens, blackScreen, manualFire, confirmDetection } from './stores/capture.js';

describe('the panic keys reset where we are in the plan', () => {
  beforeEach(() => liveCue.set({ cueId: 7, slide: 2 }));

  /// THE bug. Before this, `Esc` cleared the screens but left liveCue pointing at
  /// cue 7 / slide 2 — so the very next → fired slide 3 STRAIGHT BACK onto the
  /// congregation's screen, moments after the operator had panicked and cleared it.
  it('Esc / clear leaves us nowhere in the plan', async () => {
    await clearScreens();
    expect(get(liveCue)).toEqual({ cueId: null, slide: 0 });
  });

  it('blackout also leaves us nowhere in the plan', async () => {
    await blackScreen();
    expect(get(liveCue)).toEqual({ cueId: null, slide: 0 });
  });

  /// A hand-typed verse is not a plan cue. If the arrows still thought we were in
  /// the plan, the next → would jump back to a slide the service has moved past.
  it('firing a verse by hand takes us out of the plan', async () => {
    await manualFire('John 3:16').catch(() => {});
    expect(get(liveCue).cueId).toBe(null);
  });

  it('accepting an AI suggestion takes us out of the plan', async () => {
    await confirmDetection('John 3:16');
    expect(get(liveCue).cueId).toBe(null);
  });

  /// The reset must survive a backend failure — it happens before the call, not
  /// after it.
  it('resets even when the backend call fails', async () => {
    await clearScreens(); // no Tauri here: the invoke throws and is caught
    expect(get(liveCue).cueId).toBe(null);
  });
});
