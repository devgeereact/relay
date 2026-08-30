import { describe, it, expect, beforeEach } from 'vitest';

describe('first-run gating', () => {
  beforeEach(() => localStorage.clear());

  /// A brand-new install must show setup. This is the whole "installable" epic:
  /// a volunteer who has never seen a terminal gets a verse on a projector.
  it('a fresh install has not done setup', async () => {
    const { session } = await import('./session.js?fresh1');
    let v;
    session.subscribe((s) => (v = s))();
    expect(v.setupDone).toBe(false);
  });

  /// ...and once it's done it NEVER comes back. A wizard that reappears is a
  /// wizard that gets clicked through blindly.
  it('setup never reappears once completed or skipped', async () => {
    localStorage.setItem(
      'relay.session.v1',
      JSON.stringify({ setupDone: true, activeTab: 'live' }),
    );
    const { session } = await import('./session.js?fresh2');
    let v;
    session.subscribe((s) => (v = s))();
    expect(v.setupDone).toBe(true);
  });

  /// A corrupt payload must not strand the operator in a permanent wizard, nor
  /// block boot.
  ///
  /// This test was NAMED for that guarantee and asserted its opposite: it required
  /// `setupDone === false`, which is the fresh-install signal — the permanent
  /// wizard it says must not happen. `coldstart.test.js` was written about exactly
  /// this pairing, and the assertion is corrected here rather than the finding being
  /// filed twice.
  it('a corrupt session falls back to a safe default', async () => {
    localStorage.setItem('relay.session.v1', 'not json{{{');
    const { session } = await import('./session.js?fresh3');
    let v;
    session.subscribe((s) => (v = s))();
    // Not a fresh install: a key existed, so this machine has been set up.
    expect(v.setupDone).toBe(true);
    // The run surface, because it may have been mid-service thirty seconds ago.
    expect(v.activeTab).toBe('live');
  });
});

// The wizard never appears uninvited — but it must be REACHABLE.
//
// An operator who skipped it, or who took over the laptop from whoever ran the desk last
// year, could not get it back at all. It is the only place that walks them through the
// projector, the microphone and a proof verse in one go, ending with them having SEEN it
// work. Never showing up uninvited and never being reachable are two different things,
// and only the first one is the good idea.
describe('re-running first-run setup', () => {
  beforeEach(() => localStorage.clear());

  it('restartSetup() brings the wizard back, because the operator asked for it', async () => {
    localStorage.setItem('relay.session.v1', JSON.stringify({ setupDone: true, activeTab: 'live' }));
    const { session, restartSetup } = await import('./session.js?fresh4');

    let v;
    const stop = session.subscribe((s) => (v = s));
    expect(v.setupDone).toBe(true);

    restartSetup();
    expect(v.setupDone).toBe(false);
    stop();
  });

  // Re-running setup is NOT a reset. An operator may open Settings while a service is
  // running, and losing the playhead would restart the plan at cue 1 — the opening
  // countdown, back on the wall, at the end of the service.
  it('keeps the operator’s place — it is not a reset', async () => {
    localStorage.setItem(
      'relay.session.v1',
      JSON.stringify({
        setupDone: true,
        activeTab: 'library',
        planId: 4,
        liveCueId: 9,
        liveSlide: 2,
        liveOnAir: true,
      }),
    );
    const { session, restartSetup } = await import('./session.js?fresh5');

    let v;
    const stop = session.subscribe((s) => (v = s));
    restartSetup();

    expect(v.setupDone).toBe(false);
    expect(v.planId).toBe(4);
    expect(v.liveCueId).toBe(9);
    expect(v.liveSlide).toBe(2);
    expect(v.liveOnAir).toBe(true);
    expect(v.activeTab).toBe('library');
    stop();
  });
});

describe('where a session lands', () => {
  beforeEach(() => localStorage.clear());

  it('sends a genuinely fresh install to the run surface (Live)', async () => {
    // Dashboard moved into Settings, so a fresh install lands on the surface an
    // operator actually runs a service from — Live — not a separate home screen.
    const { session } = await import('./session.js?land1');
    let v;
    session.subscribe((s) => (v = s))();
    expect(v.activeTab).toBe('live');
  });

  it('sends a CORRUPT session to the run surface, not the Dashboard', async () => {
    // The distinction this exists for: a corrupt payload is not a fresh install.
    // There WAS a session — possibly mid-service thirty seconds ago — and it is
    // simply unreadable. That operator needs the console, not a readiness report
    // about a service that is already happening.
    localStorage.setItem('relay.session.v1', '{not json');
    const { session } = await import('./session.js?land2');
    let v;
    session.subscribe((s) => (v = s))();
    expect(v.activeTab).toBe('live');
  });
});

// ── Tabs that moved ─────────────────────────────────────────────────────────
//
// `activeTab` is persisted and outlives the layout it was written under, so every
// relocated surface leaves a stale key in somebody's localStorage. The redirect
// lived inline in App.svelte as a one-key ternary and covered only
// `stagedisplays`; `dashboard` and `history` had ALSO stopped being tabs, and an
// operator last on either was silently dropped on Live instead of Settings, where
// both of them went.
//
// Nothing could catch that, because App.svelte is not unit-testable and the map
// was not a value. It is now both.
describe('a tab that moved sends the operator where it went', () => {
  const KNOWN = ['live', 'channels', 'templates', 'themes', 'library', 'planner', 'settings', 'help'];

  it('sends each relocated surface to the tab that absorbed it', async () => {
    const { resolveActiveTab } = await import('./session.js?tabs1');
    // The gallery became real backend channels.
    expect(resolveActiveTab('stagedisplays', KNOWN)).toBe('channels');
    // Both became sections INSIDE Settings.
    expect(resolveActiveTab('dashboard', KNOWN)).toBe('settings');
    expect(resolveActiveTab('history', KNOWN)).toBe('settings');
  });

  it('leaves a tab that still exists alone', async () => {
    const { resolveActiveTab } = await import('./session.js?tabs2');
    for (const k of KNOWN) expect(resolveActiveTab(k, KNOWN)).toBe(k);
  });

  // A key that is genuinely gone — not moved — must still land somewhere usable.
  // Live is the run surface and the only safe default mid-service.
  it('falls back to the run surface for a key that is simply unknown', async () => {
    const { resolveActiveTab } = await import('./session.js?tabs3');
    expect(resolveActiveTab('somethingelse', KNOWN)).toBe('live');
    expect(resolveActiveTab(undefined, KNOWN)).toBe('live');
    expect(resolveActiveTab(null, KNOWN)).toBe('live');
  });

  // The map is only correct if its targets are real. A rename of the Settings tab
  // would otherwise turn every redirect into a silent bounce back to Live — the
  // exact failure this whole block exists to prevent, one level up.
  it('every redirect target is itself a real tab', async () => {
    const { MOVED_TABS } = await import('./session.js?tabs4');
    for (const [from, to] of Object.entries(MOVED_TABS)) {
      expect(KNOWN, `${from} redirects to '${to}', which is not a tab`).toContain(to);
    }
  });
});
