// NOT ASKED YET · ASKED AND FAILED · ASKED AND GENUINELY EMPTY.
//
// `readErrors` exists because a GROUP 2 read swallows to `[]` and the view cannot
// tell those three apart from the list alone (`capture.js`, "WHY A LIST WAS EMPTY").
// RG-95 fixed the panes that were found at the time. This file covers four that
// were not, each of which printed the EMPTY sentence over a failed read:
//
//   · Outputs → Content looks   "No templates yet — make one in the Templates tab
//                                first." A fresh install ships five built-ins, so
//                                that sentence cannot be true of a working Relay —
//                                and the operator's answer to it is to build five
//                                more. Same defect RG-95 was filed for, third door.
//   · Templates → Themes        The gallery hid it better than anywhere else,
//                                because the BUILT-INS always render: a failed read
//                                showed `Custom 0` and "No theme matches this
//                                filter", and the operator rebuilds themes they
//                                still have.
//   · Settings → Dashboard      "No plans yet. Build one in Planner" — the sentence
//                                `Loading.svelte`'s own header names as the one that
//                                makes an operator think they have lost their work,
//                                standing on a second surface. Both lists here are
//                                fire-and-forget `.then()`, so it was ALSO what the
//                                pane said before the database had answered.
//   · Settings → Translations   "No translations loaded." The KJV is bundled
//                                (`include_str!`), so this is impossible on a Relay
//                                that works. It was what a failed read said.
//
// ── HOW EACH TEST WAS CHECKED ────────────────────────────────────────────────
//
// Every case below was watched to go RED by reverting its `{:else if}` branch (and,
// for the Dashboard, the `asked*` flags) and re-running this file. Each assertion
// is paired with the GENUINELY-EMPTY control — same pane, same empty list, no
// recorded error — because a test that only ever sees the failure would stay green
// if the fix swallowed the empty case instead.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';
import { get } from 'svelte/store';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
// Settings' mount awaits this one before it asks for anything; unmocked it never
// resolves under jsdom and the reads under test are simply never reached.
vi.mock('@tauri-apps/api/app', () => ({ getVersion: () => Promise.resolve('0.0.0-test') }));

const cap = await import('./stores/capture.js');
const { readErrors, templates, customThemes } = cap;

let host;
let app;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue([]);
  readErrors.set({});
});
afterEach(() => {
  try {
    app?.$destroy();
  } catch {
    /* a view that failed to mount has nothing to destroy */
  }
  host?.remove();
  app = null;
  host = null;
});

/**
 * Let the view's mount actually finish — these views ask on mount and paint on the
 * answer, which is the whole subject of this file.
 *
 * MACROTASKS, not microtasks. `capture.js` reaches the bridge through a lazy
 * `await import('@tauri-apps/api/core')`, and a module import does not resolve on a
 * microtask drain however many times you await `Promise.resolve()` — so a loop of
 * those leaves every wrapper reading `undefined.invoke`, and the pane renders as
 * though the engine were missing whatever the commands are stubbed to do. Same
 * shape as `screencards.test.js`.
 */
async function settle(n = 14) {
  for (let i = 0; i < n; i += 1) {
    await new Promise((r) => setTimeout(r, 0));
    await tick();
  }
}

async function mount(path, props = {}) {
  const C = (await import(/* @vite-ignore */ path)).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new C({ target: host, props });
  await settle();
  return host;
}

/** These two views own their own desk/section state, so a test reaches the pane
    the way an operator does — by pressing the thing that opens it. */
async function press(el, label) {
  // A prefix, because a desk tab carries its own count ("Content looks 0") and a
  // section row carries its description.
  const btn = [...el.querySelectorAll('button')].find((b) =>
    b.textContent.replace(/\s+/g, ' ').trim().startsWith(label),
  );
  if (!btn) throw new Error(`no control labelled "${label}"`);
  btn.click();
  await settle();
}

const text = (el) => el.textContent.replace(/\s+/g, ' ');
/** The one assertive line. `ErrorState` is `role="alert"` and nothing else is. */
const alertText = (el) =>
  [...el.querySelectorAll('[role="alert"]')].map((n) => n.textContent.replace(/\s+/g, ' ')).join(' | ');

describe('Templates → Themes says why the custom themes are missing', () => {
  it('a failed read is not "No theme matches this filter."', async () => {
    customThemes.set([]);
    // The real read has to fail: seeding `readErrors` by hand would be cleared by
    // the gallery's own `onMount(loadThemes)` a moment later, and the test would
    // then be asserting against a store nothing had written.
    invoke.mockImplementation((cmd) =>
      cmd === 'get_setting' ? Promise.reject('database is locked') : Promise.resolve([]),
    );
    const el = await mount('./views/themes/ThemeGallery.svelte');

    // Humanised through errors.js (the ONE humaniser) and announced.
    expect(alertText(el)).toMatch(/Relay could not save that just now/);
    expect(alertText(el)).not.toMatch(/database is locked/);
  });

  it('and a read that worked says nothing at all', async () => {
    customThemes.set([]);
    invoke.mockResolvedValue([]);
    const el = await mount('./views/themes/ThemeGallery.svelte');
    expect(el.querySelector('[role="alert"]')).toBe(null);
    // The built-ins are still there, which is exactly why this pane hid the defect.
    expect(text(el)).not.toMatch(/No theme matches this filter/);
  });
});

describe('Settings → Dashboard tells the truth about its lists', () => {
  // ── A NOTE ON WHICH OF THE TWO CARDS IS DRIVEN HERE ────────────────────────
  //
  // Recent services, not Service plans, and it is a harness limit rather than a
  // choice. Dashboard's mount starts four reads in one go, and `capture.js` reaches
  // the bridge through a lazy `await import('@tauri-apps/api/core')`. Under vitest's
  // mocked-module resolution the FIRST of several concurrent dynamic imports of a
  // mocked module resolves and the rest hand back `undefined` — so `listPlans` and
  // `listOutputChannels` fail on every run here however the commands are stubbed,
  // while `listServices` (the first to ask) works. In a browser a concurrent
  // `import()` of the same module returns the same namespace to every caller, so
  // this is the test bridge and not Relay.
  //
  // Both cards render the same three-way chain from the same two flags. The one
  // that can be driven is driven; the other is checked for shape at the end, which
  // is the honest split rather than a silent gap.

  it('says it is still asking before the first answer, not the empty sentence', async () => {
    let release;
    invoke.mockImplementation((cmd) =>
      cmd === 'list_services' ? new Promise((r) => (release = r)) : Promise.resolve([]),
    );
    const el = await mount('./views/Dashboard.svelte');

    expect(text(el)).toMatch(/Loading services…/);
    expect(text(el)).not.toMatch(/No services recorded yet/);
    release?.([]);
  });

  it('a failed read is not "No services recorded yet"', async () => {
    invoke.mockImplementation((cmd) =>
      cmd === 'list_services' ? Promise.reject('database is locked') : Promise.resolve([]),
    );
    const el = await mount('./views/Dashboard.svelte');

    expect(text(el)).not.toMatch(/No services recorded yet/);
    expect(alertText(el)).toMatch(/Relay could not save that just now/);
    // And the way back is a retry that re-ASKS, not a dead button (rule 35).
    expect(
      [...el.querySelectorAll('[role="alert"] button')].map((b) => b.textContent.trim()),
    ).toContain('Try again');
  });

  it('and a genuinely empty install still says what to do next', async () => {
    invoke.mockResolvedValue([]);
    const el = await mount('./views/Dashboard.svelte');

    expect(text(el)).toMatch(/No services recorded yet/);
    expect(text(el)).not.toMatch(/Loading services…/);
  });

  it('the Service plans card carries the same three-way chain', () => {
    // Shape only, and it says so. The sentence this protects is the one
    // `Loading.svelte`'s own header names as the worst in the product — "No plans
    // yet" over a read that had not answered, or had failed — and the behavioural
    // half cannot be reached from this harness (see the note above).
    const src = readFileSync(join(process.cwd(), 'src/lib/views/Dashboard.svelte'), 'utf8');
    const card = src.slice(src.indexOf('<h3>Service plans</h3>'));
    const chain = card.slice(0, card.indexOf('{/if}'));

    expect(chain, 'not asked yet').toMatch(/askedPlans/);
    expect(chain, 'asked and failed').toMatch(/readErrors\.listPlans/);
    expect(chain, 'the retry must re-ask').toMatch(/onRetry=\{loadPlans\}/);
    expect(chain, 'asked and genuinely empty').toMatch(/No plans yet/);
  });
});

describe('Outputs → Content looks does not send an operator to rebuild five built-ins', () => {
  const looksPane = async () => {
    templates.set([]);
    const el = await mount('./views/Channels.svelte');
    await press(el, 'Content looks');
    return el;
  };

  it('a failed template read is not "No templates yet"', async () => {
    // The REAL read fails. Seeding `readErrors` would be undone by the view's own
    // successful `loadTemplates` on mount.
    invoke.mockImplementation((cmd) =>
      cmd === 'list_templates' ? Promise.reject('database is locked') : Promise.resolve([]),
    );
    const el = await looksPane();
    expect(text(el)).not.toMatch(/No templates yet/);
    expect(alertText(el)).toMatch(/Relay could not save that just now/);
  });

  it('and a genuinely empty template list still says where to make one', async () => {
    invoke.mockResolvedValue([]);
    const el = await looksPane();
    expect(text(el)).toMatch(/No templates yet — make one in the Templates tab first/);
    expect(el.querySelector('[role="alert"]')).toBe(null);
  });
});

describe('Settings → Bible translations', () => {
  // The KJV is bundled, so the empty sentence is only ever reachable through a
  // failure or an unfinished read — which is exactly why it had to stop being the
  // only thing this row could say.
  const settings = async () => {
    const el = await mount('./views/Settings.svelte');
    await press(el, 'Scripture & Languages');
    return el;
  };

  it('a failed read is not "No translations loaded."', async () => {
    invoke.mockImplementation((cmd) =>
      cmd === 'list_translations' ? Promise.reject('database is locked') : Promise.resolve([]),
    );
    const el = await settings();
    expect(text(el)).not.toMatch(/No translations loaded\./);
    expect(alertText(el)).toMatch(/could not/i);
  });

  it('and the reason is recorded by the store, not invented by the view', async () => {
    invoke.mockImplementation((cmd) =>
      cmd === 'list_translations' ? Promise.reject('database is locked') : Promise.resolve([]),
    );
    await settings();
    expect(get(readErrors).listTranslations).toBeTruthy();
  });
});
