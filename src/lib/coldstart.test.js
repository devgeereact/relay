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

  /// THE FINDING.
  ///
  /// `load()`'s corrupt branch is commented "A CORRUPT payload is NOT a fresh
  /// install, and the difference matters" — and then returns `{ ...EMPTY,
  /// activeTab: 'live' }`, whose `setupDone` is `false`, which IS the fresh-install
  /// signal and the only thing App.svelte reads.
  ///
  /// Worse, `session.subscribe` persists on every change and fires IMMEDIATELY on
  /// subscribe, so the fallback is written straight back over the corrupt payload.
  /// One unreadable read is therefore not a one-launch inconvenience: the install is
  /// permanently rewritten to "nobody has ever set this up", and the modal wizard
  /// returns on every launch from then on.
  ///
  /// If this test fails, the corrupt branch learned to preserve `setupDone` (or to
  /// stop writing itself back) — good; delete it.
  it('heals a corrupt payload into a PERMANENT "never set up" state', async () => {
    localStorage.setItem(KEY, 'not json{{{');

    const { session } = await import('./session.js?coldstart1');
    let v;
    session.subscribe((s) => (v = s))();

    // This launch shows the wizard...
    expect(v.setupDone).toBe(false);

    // ...and the corrupt payload has already been replaced by one that says the
    // same thing, so every future launch shows it too.
    const rewritten = JSON.parse(localStorage.getItem(KEY));
    expect(rewritten.setupDone).toBe(false);

    // Prove it is permanent: a second, clean load of the module reads what the
    // first one wrote and still reports a fresh install.
    const again = await import('./session.js?coldstart2');
    let w;
    again.session.subscribe((s) => (w = s))();
    expect(w.setupDone).toBe(false);
  });

  /// The same write-back destroys the resume point the corrupt branch's own comment
  /// is worried about ("it may have been mid-service thirty seconds ago"). Nothing
  /// can inspect or repair the original bytes afterwards, because they are gone
  /// before any other module gets a chance to look.
  it('overwrites the unreadable payload before anything can inspect it', async () => {
    const corrupt = '{"setupDone":true,"liveCueId":42,"serviceId":7'; // truncated write
    localStorage.setItem(KEY, corrupt);

    const { session } = await import('./session.js?coldstart3');
    session.subscribe(() => {})();

    expect(localStorage.getItem(KEY)).not.toBe(corrupt);
    const now = JSON.parse(localStorage.getItem(KEY));
    expect(now.liveCueId).toBe(null);
    expect(now.serviceId).toBe(null);
    // `setupDone: true` was legible in the raw text and is discarded with the rest.
    expect(corrupt).toContain('"setupDone":true');
    expect(now.setupDone).toBe(false);
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
