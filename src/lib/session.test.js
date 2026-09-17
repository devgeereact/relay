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
  // `KNOWN` is what App.svelte calls `routes`: the six workspaces in the strip,
  // plus Help, which is reachable from inside Settings but is not a workspace.
  // Handing the resolver the STRIP alone would bounce Settings' two "Open Help"
  // buttons straight back to Live — a control that looks like it worked and did
  // nothing — so the two lists are deliberately different and this is the one the
  // resolver is given.
  const KNOWN = ['live', 'library', 'planner', 'templates', 'channels', 'settings', 'help'];

  it('sends each relocated surface to the tab that absorbed it', async () => {
    const { resolveActiveTab } = await import('./session.js?tabs1');
    // The gallery became real backend channels.
    expect(resolveActiveTab('stagedisplays', KNOWN)).toBe('channels');
    // Both became sections INSIDE Settings.
    expect(resolveActiveTab('dashboard', KNOWN)).toBe('settings');
    expect(resolveActiveTab('history', KNOWN)).toBe('settings');
    // Themes were folded INTO templates (DECISIONS §87) — first as a desk inside
    // the Templates workspace, then into the template model itself. Without the
    // map entry an operator who was last on Themes lands on Live and has no
    // reason to believe the thing they were editing still exists. It does: it is
    // the template's own style.
    expect(resolveActiveTab('themes', KNOWN)).toBe('templates');
  });

  // HELP LEFT THE STRIP AND IS NOT A REDIRECT. It did not move anywhere, so it is
  // deliberately absent from `MOVED_TABS`: an operator whose session remembers
  // Help must land on Help. This is the assertion that makes "off the strip" and
  // "unreachable" two different things.
  it('keeps Help reachable even though it is not a workspace', async () => {
    const { resolveActiveTab, MOVED_TABS } = await import('./session.js?tabs5');
    expect(resolveActiveTab('help', KNOWN)).toBe('help');
    expect(MOVED_TABS).not.toHaveProperty('help');
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

  // ── THE DESK THE REDIRECT LANDED ON IS ITSELF GONE, AND THE REDIRECT IS NOT ──
  //
  // `MOVED_TABS` gets somebody who was on the old Themes tab into the Templates
  // WORKSPACE. That workspace briefly had two desks and `migrateSession` chose
  // between them; themes are now folded into the template model (DECISIONS §87),
  // so there is one desk and `templatesDesk` is dropped like any other key with
  // no reader. The redirect still matters — more, not less: an operator whose
  // laptop was left on that tab must land on the workspace that now holds what
  // they were editing rather than bounce to Live.
  it('still sends somebody last on the Themes tab to the Templates workspace', async () => {
    const { resolveActiveTab, MOVED_TABS } = await import('./session.js?desk1');
    expect(MOVED_TABS.themes).toBe('templates');
    expect(resolveActiveTab('themes', KNOWN)).toBe('templates');
  });

  it('drops the desk key nothing reads any more, and nothing else', async () => {
    const { migrateSession } = await import('./session.js?desk2');
    // A key with no reader is re-persisted for the life of the install by the
    // store's subscriber, exactly as `liveDensity` was — same defect, same fix.
    const out = migrateSession({ activeTab: 'templates', templatesDesk: 'themes' });
    expect('templatesDesk' in out).toBe(false);
    // A deletion, not a reset: everything else about that session survives.
    expect(out).toEqual({ activeTab: 'templates' });
  });
});


// ── A SETTING THAT WAS DELETED, NOT MOVED (T2) ──────────────────────────────
//
// `liveDensity` backed Live's `Normal | Compact` segment, removed on the
// operator's instruction. A key with no reader is not inert here: the store's
// subscriber writes the whole object back to localStorage on every change, so
// an un-dropped key is re-persisted for the life of the install and reads, to
// the next person, as a setting somebody forgot to wire up. `migrateSession`
// drops it — the same door `MOVED_TABS` uses, because it is already applied to
// every load and already cannot be skipped on the corrupt-payload path.
describe('a density that no longer exists is dropped, not carried', () => {
  beforeEach(() => localStorage.clear());

  const read = (store) => {
    let v;
    store.subscribe((s) => (v = s))();
    return v;
  };

  it('drops a density a saved session still carries', async () => {
    const { migrateSession } = await import('./session.js?dens1');
    const out = migrateSession({
      activeTab: 'live',
      liveFullscreen: true,
      liveDensity: 'compact',
    });
    expect('liveDensity' in out).toBe(false);
    // Everything else about that session is untouched — this is a deletion, not
    // a reset, and an operator mid-service keeps their place.
    expect(out).toEqual({ activeTab: 'live', liveFullscreen: true });
  });

  it('drops it for a session that still names the old Themes tab', async () => {
    // This used to be the SECOND of two returns, and fixing one while leaving the
    // other is the exact shape of "a guarantee is only kept on the doors you
    // checked". There is one return now, which is why there is one to check.
    const { migrateSession } = await import('./session.js?dens2');
    const out = migrateSession({ activeTab: 'themes', liveDensity: 'compact' });
    expect('liveDensity' in out).toBe(false);
    // `activeTab` is left exactly as saved: `resolveActiveTab` redirects it at
    // render, against the tabs that exist then.
    expect(out).toEqual({ activeTab: 'themes' });
  });

  it('a stored density does not survive a real load', async () => {
    // The end-to-end claim: what the app is handed, through the same
    // localStorage a running install reads.
    localStorage.setItem(
      'relay.session.v1',
      JSON.stringify({ activeTab: 'live', liveCueId: 'c7', liveDensity: 'compact' }),
    );
    const { session } = await import('./session.js?dens3');
    const s = read(session);
    expect('liveDensity' in s).toBe(false);
    // The resume point itself is NOT collateral damage.
    expect(s.liveCueId).toBe('c7');
    // …and the subscriber has already written the cleaned object back, so the
    // key does not come round again on the next boot.
    expect(JSON.parse(localStorage.getItem('relay.session.v1')))
      .not.toHaveProperty('liveDensity');
  });

  it('EMPTY does not reintroduce it', async () => {
    // `load()` merges over EMPTY, so a key left in EMPTY would be re-added to
    // every session by the merge and the drop would be undone one line later.
    const { session } = await import('./session.js?dens4');
    expect('liveDensity' in read(session)).toBe(false);
  });
});
