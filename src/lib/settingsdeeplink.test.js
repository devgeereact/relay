// A CONTROL THAT POINTS AT A SETTING MAY SAY WHICH ONE.
//
// Settings is eleven sections behind one tab, and every control that meant one of
// them could only say "go to Settings". Two consequences, and the second is worse
// than a nuisance:
//
//   * the detection inspector's `Change sensitivity in Settings` NAMES a control
//     and landed on General, leaving the operator to find it;
//   * Dashboard's `All history` pointed at the LIBRARY, where History had not
//     lived since it moved into Settings — so somebody looking for past services
//     was sent to a workspace that no longer has any. History has since come out
//     of Settings and become a route of its own, so that link is a plain tab
//     change again; what it may never go back to being is a pointer at a surface
//     the thing is not on, which is what this block is really about.
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
    setSession({ activeTab: 'settings', settingsSection: 'preachers' });
    expect(get(session).settingsSection).toBe('preachers');
  });

  it('Dashboard sends All history to the History ROUTE, not to a workspace it left', () => {
    const src = read('./views/Dashboard.svelte');
    const btn = src.slice(src.indexOf('All history') - 400, src.indexOf('All history'));
    expect(btn, 'All history still points at the Library').not.toMatch(/go\('library'\)/);
    expect(btn, 'All history still points at Settings, which no longer has it').not.toMatch(
      /settingsSection: 'history'/,
    );
    expect(btn).toMatch(/activeTab: 'history'/);
  });

  it('…and it is the one door in, so it may not be hidden behind an empty list', () => {
    // History is off the tab strip. This button is the only rendered control that
    // reaches it, and it used to sit inside `{#if services.length}` — so a church
    // that had recorded nothing could not open the screen that would have told
    // them so. `scripts/qa-inventory.mjs` counts a route no control reaches as an
    // orphan, and an empty state is a better answer than a missing door.
    // Comments stripped first: the explanation beside the fix necessarily NAMES
    // the conditional it removed, and a scanner that counted that would fail on a
    // correct file — whose cheapest repair is deleting the explanation.
    const src = read('./views/Dashboard.svelte').replace(/<!--[\s\S]*?-->/g, '');
    const head = src.slice(
      src.lastIndexOf('<header>', src.indexOf('All history')),
      src.indexOf('All history'),
    );
    expect(head, 'All history is behind a conditional again').not.toMatch(/\{#if/);
  });

  it("the inspector's sensitivity link names the section that has the control", () => {
    // AND THE KEY IS CHECKED AGAINST THE REAL SECTION LIST, which is what makes a
    // stale one dangerous rather than loud: `Settings.svelte` drops a key it does
    // not recognise and lands on section one, so a link left pointing at a
    // renamed section looks like it worked. This asserts the key resolves rather
    // than that it is a particular string.
    const src = read('./views/Live.svelte');
    const at = src.indexOf('onTuning=');
    expect(at, 'the inspector no longer offers the link').toBeGreaterThan(-1);
    // Searched from the handler rather than inside a fixed window: the comment
    // beside this call explains why the key moved, at length, and a 400-character
    // window stopped reaching the call it was about — a scanner that silently
    // finds nothing is the failure this file's neighbours keep recording.
    const key = src.slice(at).match(/settingsSection: '([a-z]+)'/)?.[1];
    expect(key, 'the inspector stopped naming a section').toBeTruthy();
    const settings = read('./views/Settings.svelte');
    const rail = settings.slice(settings.indexOf('const SECTIONS = ['), settings.indexOf("let section ="));
    expect(rail, `the inspector points at '${key}', which is not a section`).toContain(`key: '${key}'`);
    // …and it is the section that actually holds the dial it names.
    const pane = settings.slice(settings.indexOf(`section === '${key}'`));
    expect(pane).toMatch(/Sensitivity|sensitivity/);
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
