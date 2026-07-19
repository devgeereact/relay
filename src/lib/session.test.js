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
  it('a corrupt session falls back to a safe default', async () => {
    localStorage.setItem('relay.session.v1', 'not json{{{');
    const { session } = await import('./session.js?fresh3');
    let v;
    session.subscribe((s) => (v = s))();
    expect(v.setupDone).toBe(false);
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

  it('sends a genuinely fresh install to the Dashboard', async () => {
    // Nothing has ever been saved: nobody has run a service on this machine, and
    // "is this going to work?" is the only question they have.
    const { session } = await import('./session.js?land1');
    let v;
    session.subscribe((s) => (v = s))();
    expect(v.activeTab).toBe('dashboard');
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
