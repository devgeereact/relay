// R1 · COLD START — the onboarding trigger, and what happens when it misfires.
//
// `session.js` decides, alone, whether a volunteer is shown the six-step first-run
// wizard: `!$session.setupDone` in App.svelte, on top of everything, modal.
//
// `session.test.js` already covers "a fresh install shows it" and "a completed one
// never sees it again". These cover the third case — a session payload that cannot
// be parsed — because its existing test is named for a guarantee it does not make.

import { describe, it, expect, beforeEach } from 'vitest';

const KEY = 'relay.session.v1';

describe('a corrupt session payload and the first-run wizard', () => {
  beforeEach(() => localStorage.clear());

  /// CLOSED — the finding, inverted.
  ///
  /// `load()`'s corrupt branch is commented "A CORRUPT payload is NOT a fresh
  /// install, and the difference matters" — and it used to return `{ ...EMPTY,
  /// activeTab: 'live' }`, whose `setupDone` is `false`, which IS the fresh-install
  /// signal and the only thing App.svelte reads.
  ///
  /// And `session.subscribe` persists on every change and fires IMMEDIATELY, so
  /// that fallback was written straight back over the corrupt payload. One
  /// unreadable read was therefore not a one-launch inconvenience: the install was
  /// rewritten to "nobody has ever set this up", and the six-step modal wizard
  /// opened over a console that may have been mid-service.
  ///
  /// A key that EXISTS is proof the app has run here. A genuinely fresh install has
  /// no key and is handled a branch earlier.
  it('a corrupt payload does not turn the install into a fresh one', async () => {
    localStorage.setItem(KEY, 'not json{{{');

    const { session } = await import('./session.js?coldstart1');
    let v;
    session.subscribe((s) => (v = s))();

    // No wizard: this machine has plainly been set up before.
    expect(v.setupDone).toBe(true);
    // …and the run surface, not a summary screen — it may be mid-service.
    expect(v.activeTab).toBe('live');

    const rewritten = JSON.parse(localStorage.getItem(KEY));
    expect(rewritten.setupDone).toBe(true);

    // And it stays fixed: a second, clean load reads what the first one wrote.
    const again = await import('./session.js?coldstart2');
    let w;
    again.session.subscribe((s) => (w = s))();
    expect(w.setupDone).toBe(true);
  });

  /// CLOSED — the same write-back used to destroy the resume point the corrupt
  /// branch's own comment worries about ("it may have been mid-service thirty
  /// seconds ago"). The bytes are kept under a sidecar key now.
  ///
  /// Nothing reads the sidecar yet, and the test says so rather than implying a
  /// recovery feature that does not exist: it exists so the evidence survives, and
  /// so a future repair has something to repair FROM.
  it('keeps the unreadable payload instead of destroying it', async () => {
    const corrupt = '{"setupDone":true,"liveCueId":42,"serviceId":7'; // truncated write
    localStorage.setItem(KEY, corrupt);

    const { session } = await import('./session.js?coldstart3');
    session.subscribe(() => {})();

    // The live payload is replaced — it has to be, it could not be parsed…
    expect(localStorage.getItem(KEY)).not.toBe(corrupt);
    // …but the original bytes are still here, verbatim.
    expect(localStorage.getItem(`${KEY}.corrupt`)).toBe(corrupt);

    const now = JSON.parse(localStorage.getItem(KEY));
    // The resume point is genuinely gone: nothing guesses at half-written JSON.
    expect(now.liveCueId).toBe(null);
    expect(now.serviceId).toBe(null);
    // And the wizard does not reopen over a service.
    expect(now.setupDone).toBe(true);
  });

  /// A partial payload from an older build is merged over EMPTY, so a key that
  /// exists is honoured. This is the contrast case: the machinery to preserve
  /// `setupDone` across a bad read already exists one branch away.
  it('a PARTIAL payload keeps setupDone, which is what the corrupt branch does not', async () => {
    localStorage.setItem(KEY, JSON.stringify({ setupDone: true }));
    const { session } = await import('./session.js?coldstart4');
    let v;
    session.subscribe((s) => (v = s))();
    expect(v.setupDone).toBe(true);
    expect(v.activeTab).toBe('live');
  });
});

describe('the first-run wizard, if it does reappear', () => {
  /// Not a hypothetical: the wizard is `role="dialog" aria-modal="true"` over a
  /// full-viewport scrim, and its Audio step calls `setDetection(false)` and
  /// `startCapture()`. Whatever else is true, a wizard that can return uninvited
  /// can disarm the AI and re-open the microphone during a service.
  ///
  /// Layer C: this asserts the wizard's shape, not that it was ever shown. Proving
  /// the mid-service case needs a running app (layer D) or a human (layer E).
  it('is modal and its audio step disarms detection', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const src = readFileSync(resolve(__dirname, 'FirstRun.svelte'), 'utf8');
    expect(src).toContain('aria-modal="true"');
    expect(src).toContain('await setDetection(false)');
    expect(src).toContain('await startCapture(');
    // ...and it does put the previous value back on the way out, which is the
    // mitigation. It runs from `go()`, `done()` and `onDestroy`.
    expect(src).toContain('await setDetection(detectionWas)');
  });
});
