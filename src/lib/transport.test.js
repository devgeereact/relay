// The playhead: where → resumes from, and whether the plan is ON AIR.
//
// These are two different facts and every path that takes plan content off the
// screen must clear the second WITHOUT destroying the first. Getting either half
// wrong puts the wrong thing in front of a congregation:
//
//   forget to clear onAir  → the operator accepts an AI-suggested verse, presses
//                            → to read on, and jumps back into the song instead.
//   destroy the position   → the operator presses Esc at cue 9, presses →, and the
//                            plan RESTARTS at cue 1 — the opening countdown, back
//                            on the wall, at the end of the service.
import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';

// No Tauri in the test env — every wrapper falls into its catch and is a no-op,
// which is exactly the path we want: the reset must happen REGARDLESS of whether
// the backend call succeeds. A panic key that half-works is worse than one that
// doesn't work at all.
import {
  liveCue,
  clearScreens,
  blackScreen,
  manualFire,
  confirmDetection,
} from './stores/capture.js';

describe('taking plan content off the screen', () => {
  // Cue 7, slide 2, on the congregation's wall.
  beforeEach(() => liveCue.set({ cueId: 7, slide: 2, onAir: true }));

  it('Esc / clear takes the plan off air but REMEMBERS the position', async () => {
    await clearScreens();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: false });
  });

  it('blackout does the same', async () => {
    await blackScreen();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: false });
  });

  /// A hand-typed verse is not a plan cue. If the transport still thought the plan
  /// was on air, → would jump back into the plan instead of walking the passage
  /// the operator just put up.
  it('firing a verse by hand takes the plan off air', async () => {
    await manualFire('John 3:16').catch(() => {});
    expect(get(liveCue).onAir).toBe(false);
    expect(get(liveCue).cueId).toBe(7); // still where we were
  });

  it('accepting an AI suggestion takes the plan off air', async () => {
    await confirmDetection('John 3:16');
    expect(get(liveCue).onAir).toBe(false);
    expect(get(liveCue).cueId).toBe(7);
  });

  /// It happens BEFORE the backend call, not after — so a failed call cannot leave
  /// the transport claiming the plan is on air when it isn't.
  it('holds even when the backend call fails', async () => {
    await clearScreens(); // no Tauri here: the invoke throws and is caught
    expect(get(liveCue).onAir).toBe(false);
  });

  it('is idempotent — clearing twice does not lose the position', async () => {
    await clearScreens();
    await clearScreens();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: false });
  });
});
