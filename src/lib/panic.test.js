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
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

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

// ── REACHABLE AT EVERY WIDTH ────────────────────────────────────────────────
//
// Rule 15 / DECISIONS §20: a panic control may never be scrolled out of reach.
// The dock is a four-column rack on a desk, and stacking it for a narrow window
// broke that twice, both measured in a render rather than reasoned about:
//
//   1. `.dbody` is `flex: 1 1 0`, which divides a KNOWN 178px rack height. In a
//      grid row of auto height that basis collapses the body to nothing — at
//      768px the Controls body measured **19px tall over 122px of content**,
//      behind `overflow:hidden`. Clear screens was on the screen and invisible.
//   2. One column puts the Controls card LAST, so at 430px Clear screens sat
//      inside its card and below the viewport: reachable only by scrolling.
//
// jsdom does no layout, so this asserts the two rules that prevent both. The
// measurement itself lives in the audit note; this is the tripwire.
describe('the dock keeps its panic controls reachable when it stacks', () => {
  const dock = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

  it('a stacked card sizes to its content instead of collapsing its body', () => {
    const narrow = dock.slice(dock.indexOf('@media (max-width: 900px)'));
    expect(narrow).toMatch(/\.dbody\s*\{\s*flex:\s*1\s+1\s+auto/);
    expect(narrow).toMatch(/\.dpanel\s*\{\s*min-height:/);
  });

  it('the Controls card comes FIRST once the dock is one column', () => {
    const phone = dock.slice(dock.indexOf('@media (max-width: 640px)'));
    expect(phone).toMatch(/\.dpanel\.ctl\s*\{\s*order:\s*-1/);
    // …and the card it names is really the Controls card, not whichever card
    // happened to be fourth when this was written.
    const ctl = dock.slice(dock.indexOf('<div class="dpanel ctl">'));
    expect(ctl.slice(0, 1400)).toMatch(/Clear screens/);
  });
});
