// A CONTROL THAT POINTS AT A SETTING MAY SAY WHICH ONE.
//
// Settings is eleven sections behind one tab, and every control that meant one of
// them could only say "go to Settings". Two consequences, and the second is worse
// than a nuisance:
//
//   * the detection inspector's `Change sensitivity in Settings` NAMES a control
//     and landed on General, leaving the operator to find it;
//   * Dashboard's `All history` pointed at the LIBRARY, where History has not
//     lived since it moved into Settings — so somebody looking for past services
//     was sent to a workspace that no longer has any.
//
// One-shot, not a resume point. `activeTab` is persisted deliberately, so an
// operator running a service yesterday comes back to Live; a section chosen by a
// button last week is not something to come back to.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

vi.mock('@tauri-apps/api/core', () => ({ invoke: async () => null }));

const { session, setSession, clearSession } = await import('./session.js');
const read = (p) => readFileSync(resolve(__dirname, p), 'utf8');

beforeEach(() => clearSession());

describe('a control may name the Settings section it means', () => {
  it('the session carries a section, and it starts empty', () => {
    expect(get(session).settingsSection ?? null).toBeNull();
    setSession({ activeTab: 'settings', settingsSection: 'history' });
    expect(get(session).settingsSection).toBe('history');
  });

  it('Dashboard sends All history to Settings, not to the Library', () => {
    const src = read('./views/Dashboard.svelte');
    const btn = src.slice(src.indexOf('All history') - 400, src.indexOf('All history'));
    expect(btn, 'All history still points at the Library').not.toMatch(/go\('library'\)/);
    expect(btn).toMatch(/activeTab: 'settings'/);
    expect(btn).toMatch(/settingsSection: 'history'/);
  });

  it("the inspector's sensitivity link names the section that has the control", () => {
    const src = read('./views/Live.svelte');
    const at = src.indexOf('onTuning=');
    expect(at, 'the inspector no longer offers the link').toBeGreaterThan(-1);
    const body = src.slice(at, at + 400);
    expect(body).toMatch(/settingsSection: 'ai'/);
  });

  it('Settings clears it after acting, so it cannot outlive the press', () => {
    // The whole reason it is not persisted like `activeTab`. Asserted on the
    // source because the behaviour is in `onMount`, and mounting Settings under
    // jsdom needs the whole bridge — see `readstates.test.js`'s own note on that.
    const src = read('./views/Settings.svelte');
    expect(src).toMatch(/settingsSection/);
    expect(src, 'a section that is used must be cleared').toMatch(
      /setSession\(\{\s*settingsSection:\s*null\s*\}\)/,
    );
    // And it is checked against the real section list rather than trusted: a key
    // persisted from an older layout would otherwise render an empty pane.
    expect(src).toMatch(/SECTIONS\.some\(/);
  });
});
