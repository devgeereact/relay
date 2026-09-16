// NOT ASKED YET · ASKED AND FAILED · ASKED AND GENUINELY EMPTY.
//
// `readErrors` exists because a GROUP 2 read swallows to `[]` and the view cannot
// tell those three apart from the list alone (`capture.js`, "WHY A LIST WAS EMPTY").
// RG-95 fixed the panes that were found at the time. This file covered four that
// were not, each of which printed the EMPTY sentence over a failed read. Three
// remain; the fourth's pane no longer exists (themes were folded into templates,
// DECISIONS §87) and is recorded here rather than silently dropped, because the
// defect it names is the one this whole file is about:
//
//   · Outputs → Content looks   "No templates yet — make one in the Templates tab
//                                first." A fresh install ships five built-ins, so
//                                that sentence cannot be true of a working Relay —
//                                and the operator's answer to it is to build five
//                                more. Same defect RG-95 was filed for, third door.
//   · Templates → Themes        GONE with the desk. It hid the defect better than
//                                anywhere else, because the BUILT-INS always
//                                rendered: a failed read showed `Custom 0` and "No
//                                theme matches this filter", and the operator
//                                rebuilds themes they still have. Worth keeping in
//                                mind for the next gallery that ships built-ins.
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
const { readErrors, templates } = cap;

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

// ── SETTINGS → THE SENTRY DSN ───────────────────────────────────────────────
//
// The one control in Relay that decides WHERE data leaves the machine, driven
// rather than scanned. `settingssections.test.js` holds the structure — that a
// second commit path exists, that the field has no `on:blur`, that `savedDsn` is
// written from what the backend returned. What it cannot hold is the sequence:
// type, don't save, read a banner promising the old address is still in force,
// and have that remain true whatever else is pressed.
//
// A Save button was chosen over commit-on-blur because a half-typed address must
// not become the live destination without a moment where the operator said so.
// `set_crash_reporting` persists the string AND calls `telemetry::enable` on it
// in the same breath, and reports already sent cannot be recalled — so every
// path that reaches that command with something other than the saved address is
// the same defect wearing a different button.
describe('Settings → the Sentry DSN', () => {
  const OLD = 'https://old@o1.ingest.sentry.io/1';
  const NEW = 'https://new@o2.ingest.sentry.io/2';

  /** Mount Settings with crash reporting ON and an address already in force. */
  const privacy = async (onSet) => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_crash_reporting') return Promise.resolve({ enabled: true, dsn: OLD });
      if (cmd === 'set_crash_reporting') return onSet(args);
      return Promise.resolve([]);
    });
    const el = await mount('./views/Settings.svelte');
    await press(el, 'Privacy & Advanced');
    return el;
  };

  const field = (el) => el.querySelector('#crash-dsn');
  const type = async (el, value) => {
    const input = field(el);
    input.value = value;
    input.dispatchEvent(new Event('input'));
    await settle(2);
  };
  const setCalls = () =>
    invoke.mock.calls.filter(([c]) => c === 'set_crash_reporting').map(([, a]) => a);

  it('an edit that has not been saved says so, and commits nothing', async () => {
    const el = await privacy(() => Promise.resolve({ enabled: true, dsn: OLD }));
    expect(field(el).value).toBe(OLD);

    await type(el, NEW);
    expect(text(el)).toMatch(/Not saved yet/);
    expect(text(el)).toMatch(/still go to the address Relay already has/);
    expect(setCalls(), 'typing reached the backend').toEqual([]);
  });

  it('flipping the switch does NOT smuggle the unsaved address through', async () => {
    // The switch is the operator saying yes to WHETHER, not to WHERE. Sending the
    // bound field here made the "Not saved yet" banner a lie: the half-typed
    // address became the live crash-report destination one click later.
    const el = await privacy((a) => Promise.resolve({ enabled: !a.enabled, dsn: a.dsn }));
    await type(el, NEW);

    el.querySelector('[role="switch"]').click();
    await settle();

    expect(setCalls().length).toBe(1);
    expect(
      setCalls()[0].dsn,
      'the switch sent the UNSAVED address. Turning reporting on is not consent ' +
        'to change where the reports go.',
    ).toBe(OLD);
  });

  it('Save address commits it, and the unsaved line goes away', async () => {
    const el = await privacy((a) => Promise.resolve({ enabled: true, dsn: a.dsn }));
    await type(el, NEW);

    const save = [...el.querySelectorAll('button')].find((b) => b.textContent.includes('Save address'));
    expect(save, 'no Save address button').toBeTruthy();
    save.click();
    await settle();

    expect(setCalls()).toEqual([{ enabled: true, dsn: NEW }]);
    expect(text(el)).not.toMatch(/Not saved yet/);
    expect(text(el)).toMatch(/Saved\./);
  });

  it('a backend answer with no shape at all does not take the section down', async () => {
    // `acceptCrash` guards `savedDsn` and used to assign `crash` unguarded, and
    // `$: crashOn = !!crash.enabled` runs on every assignment — so a null answer
    // threw inside the reactive statement rather than showing a wrong word. Half
    // a guard is the kind of thing that only ever fires on the day it matters.
    const el = await privacy(() => Promise.resolve(null));
    [...el.querySelectorAll('button')].find((b) => b.textContent.includes('Save address')) ??
      el.querySelector('[role="switch"]').click();
    await type(el, NEW);
    [...el.querySelectorAll('button')].find((b) => b.textContent.includes('Save address')).click();
    await settle();

    expect(text(el)).toMatch(/Crash reporting/);
    expect(
      field(el).value,
      'the section did not repaint from the backend answer — `crash = landed` on a ' +
        'null throws inside `$: crashOn = !!crash.enabled`.',
    ).toBe('');
  });

  it('a read that FAILED is not an empty address — and cannot be written over', async () => {
    // `getCrashReporting` swallowed into a bare `catch` and returned the safe
    // default, which this page takes as the truth: `savedDsn` became `''`. Flipping
    // the switch then sent `('', true)`, and `set_crash_reporting` writes the string
    // unconditionally — so a failed read DESTROYED the one address a church had
    // configured, one click later, with an empty field as the only clue. Nothing
    // leaked (`telemetry::enable` returns early on an empty DSN), and losing it is
    // bad enough. The reason now lands in `readErrors` and both writing controls
    // stand down. Watched to go red by restoring the bare `catch`.
    invoke.mockImplementation((cmd) =>
      cmd === 'get_crash_reporting'
        ? Promise.reject('database is locked')
        : Promise.resolve([]),
    );
    const el = await mount('./views/Settings.svelte');
    await press(el, 'Privacy & Advanced');

    expect(get(readErrors).getCrashReporting, 'the reason was thrown away').toBeTruthy();
    // Humanised through errors.js (the ONE humaniser), and announced.
    expect(alertText(el)).toMatch(/could not read the crash-reporting setting/i);
    expect(alertText(el)).not.toMatch(/database is locked/);
    expect(el.querySelector('[role="switch"][aria-label="Send crash reports"]').disabled).toBe(true);
    const save = [...el.querySelectorAll('button')].find((b) =>
      b.textContent.includes('Save address'),
    );
    expect(save.disabled).toBe(true);

    // And the switch cannot be pressed into writing the empty string over it.
    el.querySelector('[role="switch"][aria-label="Send crash reports"]').click();
    await settle();
    expect(setCalls()).toEqual([]);
  });

  it('flipping the switch does not throw away a half-typed address', async () => {
    // `acceptCrash` takes the whole landed object, `crash.dsn` included, so a flip
    // replaced the draft with the saved address and took the "Not saved yet" mark
    // with it — the one instrument that makes an edit which went nowhere visible
    // rather than discovered a week later. The draft is the operator's; a switch is
    // not a discard. Watched to go red by removing the restore line.
    const el = await privacy((a) => Promise.resolve({ enabled: !a.enabled, dsn: a.dsn }));
    await type(el, NEW);

    el.querySelector('[role="switch"][aria-label="Send crash reports"]').click();
    await settle();

    expect(field(el).value, 'the draft was discarded by a switch flip').toBe(NEW);
    expect(text(el)).toMatch(/Not saved yet/);
    // …and it still went nowhere: the switch committed the SAVED address.
    expect(setCalls()[0].dsn).toBe(OLD);
  });

  it('a save the backend does not honour leaves the field showing what is in force', async () => {
    // Rule 15, on the smallest control on the page. If the local copy were taken
    // from what was ASKED for, the field would show the new address, the banner
    // would clear, and the engine would go on reporting somewhere else — a
    // control claiming a success it did not achieve.
    const el = await privacy(() => Promise.resolve({ enabled: true, dsn: OLD }));
    await type(el, NEW);

    [...el.querySelectorAll('button')].find((b) => b.textContent.includes('Save address')).click();
    await settle();

    expect(field(el).value).toBe(OLD);
    expect(text(el)).not.toMatch(/Not saved yet/);
  });
});
