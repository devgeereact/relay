// The panic controls must never report a success they did not achieve.
//
// The bug this file exists to prevent, verbatim:
//
//   async function clearAll() {
//     try { await clearScreens(); } catch { /* backend absent */ }
//     flash('Screens cleared');            // ← unconditional
//   }
//
// `clearScreens()` swallowed its own errors internally, so that `catch` could never
// even fire — and the toast fired regardless. If the clear failed, the operator was
// told the wall was clean while the verse was still in front of the congregation.
// They then stop looking at the screen, because the app told them not to worry.
//
// That is the worst class of bug in live software: not a control that fails, but a
// control that LIES about failing. So the contract is pinned here.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { clearScreens, blackScreen, panicError, dismissPanicError, capture } = await import(
  './stores/capture.js'
);

describe('panic controls tell the truth', () => {
  beforeEach(() => {
    invoke.mockReset();
    panicError.set(null);
    // A real app with a real backend attached — see the last test for why this matters.
    capture.update((s) => ({ ...s, available: true }));
  });

  it('clearScreens returns true and raises no alarm when it works', async () => {
    invoke.mockResolvedValue(null);
    expect(await clearScreens()).toBe(true);
    expect(invoke).toHaveBeenCalledWith('clear_screens');
    expect(get(panicError)).toBe(null);
  });

  it('clearScreens returns FALSE when the backend fails — the caller must not flash success', async () => {
    invoke.mockRejectedValue('emit failed');
    expect(await clearScreens()).toBe(false);
  });

  it('a failed clear raises the panic banner, naming the danger', async () => {
    invoke.mockRejectedValue('emit failed');
    await clearScreens();
    const msg = get(panicError);
    // The operator must be told the CONSEQUENCE (the screen may still be live),
    // not merely that a command errored.
    expect(msg).toMatch(/still be seeing/i);
    expect(msg).toContain('emit failed');
  });

  it('blackout has the identical contract — it is a panic control too', async () => {
    invoke.mockRejectedValue('nope');
    expect(await blackScreen()).toBe(false);
    expect(get(panicError)).toMatch(/still be seeing/i);

    invoke.mockResolvedValue(null);
    expect(await blackScreen()).toBe(true);
    expect(invoke).toHaveBeenCalledWith('blackout');
  });

  it('a panic control that SUCCEEDS clears a stale warning', async () => {
    // Otherwise the banner outlives the problem, the operator learns to ignore it,
    // and the next real one is invisible.
    invoke.mockRejectedValue('boom');
    await clearScreens();
    expect(get(panicError)).toBeTruthy();

    invoke.mockResolvedValue(null);
    await clearScreens();
    expect(get(panicError)).toBe(null);
  });

  it('the operator can dismiss it, having looked at the actual screen', async () => {
    invoke.mockRejectedValue('boom');
    await clearScreens();
    dismissPanicError();
    expect(get(panicError)).toBe(null);
  });

  it('does NOT cry wolf when there is no backend at all', async () => {
    // In a plain browser there is no engine AND no output screen, so nothing can be
    // stranded in front of anyone. Warning there would train the operator to dismiss
    // the banner on sight — which is how they would dismiss a real one.
    capture.update((s) => ({ ...s, available: false }));
    invoke.mockRejectedValue('no backend');
    expect(await clearScreens()).toBe(false);
    expect(get(panicError)).toBe(null);
  });
});
