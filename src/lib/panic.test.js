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
import { installShortcuts } from './shortcuts.js';
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { clearScreens, blackScreen, panicError, dismissPanicError, capture, stageAlert } =
  await import('./stores/capture.js');

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

// ── TWO PATHS, AND THE CHROME IS NO LONGER ONE OF THEM ──────────────────────
//
// On 2026-09-14 the operator cleared the chrome bar back to the wordmark and the
// six workspaces, and an `Emergency Stop` button at its right-hand corner went
// with the rest of it. That was a panic control (rule 15, DECISIONS §20), so
// removing it was only ever safe BECAUSE of the two paths it duplicated:
//
//   · `Esc`, installed once at the shell and bound straight to `clearScreens` —
//     so it works on every workspace, including one whose view has crashed, and
//     including while the cursor is in a text field;
//   · `Clear screens`, full width along the bottom edge of the Controls card in
//     the dock, which the SHELL renders on every workspace and which never
//     scrolls (the describe above).
//
// This describe is the tripwire on that reasoning. It is deliberately here and
// not only in `shellchrome.test.js`: the chrome's own tests are about what the
// bar contains, and what is being claimed is about what a panic control can
// still reach. If either path is ever taken away, this is what fails, and the
// fix is to put a control back — not to delete the assertion.
describe('removing Emergency Stop left both of its paths standing', () => {
  const app = readFileSync(resolve(process.cwd(), 'src/App.svelte'), 'utf8');
  const shortcuts = readFileSync(resolve(process.cwd(), 'src/lib/shortcuts.js'), 'utf8');
  const dock = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

  // Comments stripped. The shell's own comment records WHY the button went and
  // therefore names it; a `not.toContain` over the prose would fail on an honest
  // note about a deletion. Only the code is the claim — the same lesson
  // `transitionoverride.test.js` records from the other direction.
  const code = codeOnly(app);

  it('the chrome carries no panic control any more', () => {
    const chrome = code.slice(code.indexOf('<header class="topbar-v">'), code.indexOf('</header>'));
    expect(chrome.slice(chrome.indexOf('</nav>'))).not.toContain('<button');
    expect(code).not.toContain('Emergency Stop');
  });

  it('PATH 1 · Escape is installed once at the shell and clears from anywhere', () => {
    expect(app).toMatch(/teardownKeys = installShortcuts\(\{ clearScreens, blackScreen \}\)/);
    // Once, in the shell — never per-view (rule 11). A second install would give
    // two listeners and the wrong one could be torn down.
    expect([...app.matchAll(/installShortcuts\(/g)]).toHaveLength(1);
    // And the key reaches the store DIRECTLY, not through a mounted view's
    // context, which is what makes it survive a crashed workspace.
    const esc = shortcuts.slice(shortcuts.indexOf("if (e.key === 'Escape')"));
    expect(esc.slice(0, esc.indexOf('\n    }\n'))).toContain('clearScreens();');
    expect(esc).not.toMatch(/ctx\.\w+\(\)[\s\S]{0,40}clearScreens/);
  });

  it('…and it really does fire, on any surface, even mid-typing', () => {
    // The source claim above, driven. A view that has registered nothing at all
    // is the crashed-workspace case; an INPUT target is the reference box.
    const clearScreens = vi.fn();
    const blackScreen = vi.fn();
    const teardown = installShortcuts({ clearScreens, blackScreen });
    for (const target of [document.body, document.createElement('input')]) {
      const e = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true });
      Object.defineProperty(e, 'target', { value: target });
      window.dispatchEvent(e);
    }
    expect(clearScreens).toHaveBeenCalledTimes(2);
    teardown();
  });

  it('PATH 2 · Clear screens is in the dock, and the SHELL renders the dock', () => {
    // Not Live. An operator editing a template is on another workspace entirely,
    // and the whole reason the dock lives in the shell is that the control has to
    // be one reach away there too.
    expect(app).toContain('<Dock />');
    expect(dock).toContain('<button class="r-cbtn danger wide" on:click={doClear}');
    // `wide` is the full-width bottom edge; `doClear` is the unwrapped call, so
    // a failure reaches the shell's panic banner rather than a 9px dock line.
    expect(dock).toMatch(/const doClear = \(\) => clearScreens\(\);/);
  });

  it('the one surface that has neither hides the chrome too, and always did', () => {
    // Full-screen Live: `.chromeless` hides the header AND the shell withholds
    // the dock. So `Esc` was already the only path there before this change, and
    // nothing about that case moved. Stated rather than assumed, because "the
    // button is gone" and "there is no button here anyway" are different facts.
    const css = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8');
    expect(css).toMatch(/\.shell\.chromeless \.topbar-v,/);
    expect(app).toContain('{#if !liveFullscreen}<Dock />{/if}');
  });
});

// ── THE CONSOLE'S MIRROR OF THE STAGE MESSAGE ───────────────────────────────
//
// DECISIONS §91 decided that a panic control takes back every sentence anybody put
// on a screen and stops none of the clocks, so `Stage.svelte` now clears `alert` on
// `clear` and on `black`. `stageAlert` is the console's mirror of what Relay SENT,
// and until §91 it was right to survive a panic, because the alert did. It is not
// right any more: Quick tools paints an "on stage" badge and enables **Take down**
// from this store, so after Esc the operator was offered a control for a word that
// was already down, under a comment claiming the badge says "what the preacher's
// monitor is painting at that moment". Rule 35, on the one surface that could have
// told them. RG-145.
//
// The mirror is cleared only on SUCCESS, the same discipline `sendStageAlert`
// documents for itself: a panic that failed has taken nothing off any screen, and a
// console that said otherwise would be the failure rule 15 exists for, one surface
// along. Both cases below were watched to fail by removing the `stageAlert.set(null)`
// from `panicRun`'s success branch.
describe('a panic control takes the console mirror of the Stage Message with it (RG-145)', () => {
  beforeEach(() => {
    invoke.mockReset();
    panicError.set(null);
    stageAlert.set('Wrap up in 5');
    capture.update((s) => ({ ...s, available: true }));
  });

  it('clearScreens lets go of the word it can no longer see', async () => {
    invoke.mockResolvedValue(null);
    expect(await clearScreens()).toBe(true);
    expect(
      get(stageAlert),
      'Quick tools still offers Take down for a word that is already down',
    ).toBe(null);
  });

  it('blackout answers the same way', async () => {
    invoke.mockResolvedValue(null);
    expect(await blackScreen()).toBe(true);
    expect(get(stageAlert)).toBe(null);
  });

  it('a panic that FAILED leaves the mirror alone, because nothing came off a screen', async () => {
    invoke.mockRejectedValue('emit failed');
    expect(await clearScreens()).toBe(false);
    expect(
      get(stageAlert),
      'a failed clear claimed to have taken the word off the preacher\'s monitor',
    ).toBe('Wrap up in 5');
  });
});
