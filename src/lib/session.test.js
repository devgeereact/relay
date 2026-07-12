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
      JSON.stringify({ setupDone: true, activeTab: 'console' }),
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
    expect(v.activeTab).toBe('console');
  });
});
