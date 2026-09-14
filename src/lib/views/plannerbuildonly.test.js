// W3 · THE PLANNER CANNOT REACH AN OUTPUT — the acceptance clause, as a test.
//
// `docs/REBRAND.md` §2 and this repository's own handbook both say the same thing
// about this workspace: *build* a plan here (a Tuesday job), *run* it in Live (a
// Sunday job), and "nothing here may reach an output — that is the whole point of
// the tab". The Planner is the one surface an operator uses with a congregation
// in the room but no service running, and it is full of small targets: a drag
// handle, three mini buttons per row, a list of search results each of which adds
// something. If one of them could put a slide on the wall, the wrong thing goes up
// on a Tuesday evening rehearsal or, worse, mid-service while somebody tidies next
// week's order.
//
// That guarantee was carried entirely by prose and by the discipline of whoever
// last edited the file. Every other claim of this shape in this repository has
// eventually been broken on a door nobody enumerated (`NavResult` thrown away by
// `remote_api`; rehearsal gated on three publishers of four), so it is pinned here
// from two directions:
//
//   1. STATICALLY — the component may not even import a command that takes a
//      screen. A control cannot call what the module never pulled in, and this
//      catches the import at the moment it is added rather than at the moment
//      somebody wires a button to it.
//   2. BEHAVIOURALLY — mount the real component against a recording bridge, click
//      every control it renders, and assert that no command that changes what a
//      congregation sees was dispatched.
//
// The static half alone would be a contract stated in a comment; the behavioural
// half alone would pass for a control that is added but not yet rendered. Together
// they cover both shapes of the mistake.
//
// Verified by reintroducing the defect: wiring a `manual_fire` call into the
// Planner's row click handler fails (2), and importing `fireContent` from the
// store fails (1).

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture } = await import('../stores/capture.js');

// Same self-detecting gate as `surface.test.js`: if `vitest.config.js` ever loses
// `resolve: { conditions: ['browser'] }`, `onMount` is a literal empty function
// and a mounted component silently fetches nothing. These tests SKIP loudly rather
// than pass over a Planner that never loaded a plan.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

/**
 * Every command that changes what a congregation (or the preacher's monitor) is
 * looking at, plus the two panic controls.
 *
 * The panic controls are in the list for the same reason as the rest: Esc and B
 * are mounted once, globally, in `App.svelte` (rule 11), and a view that reached
 * them by its own route would be a second definition of the one control that may
 * never report a success it did not achieve.
 */
const TAKES_A_SCREEN = [
  'manual_fire',
  'fire_content',
  'fire_media',
  'nav',
  'confirm_detection',
  'push_announcement',
  'start_countdown',
  'set_stage_next',
  'send_stage_alert',
  'clear_screens',
  'blackout',
];

/** The store wrappers that reach those commands, by exported name. */
const FIRE_WRAPPERS = [
  'manualFire',
  'fireContent',
  'fireMedia',
  'nav',
  'confirmDetection',
  'pushAnnouncement',
  'startCountdown',
  'setStageNext',
  'sendStageAlert',
  'clearScreens',
  'blackout',
];

const PLANNER = resolve(__dirname, 'ServicePlanner.svelte');
const src = readFileSync(PLANNER, 'utf8');

/** The identifiers the component actually imports from the store. */
function storeImports(text) {
  const m = text.match(/import\s*\{([^}]*)\}\s*from\s*'\.\.\/stores\/capture\.js'/);
  if (!m) throw new Error('ServicePlanner no longer imports from stores/capture.js');
  return m[1]
    .split(',')
    .map((s) => s.replace(/\/\/[^\n]*/g, '').trim())
    .filter(Boolean);
}

const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-21', cue_count: 2 };
const CUES = [
  {
    id: 11,
    plan_id: 1,
    cue_type: 'scripture',
    label: 'Romans 8:28',
    payload_json: JSON.stringify({ reference: 'Romans 8:28', text: 'And we know…', verse: 28 }),
    section_title: 'The Word',
    duration_sec: 0,
    template_id: null,
  },
  {
    id: 12,
    plan_id: 1,
    cue_type: 'song',
    label: 'Amazing Grace',
    payload_json: JSON.stringify({
      title: 'Amazing Grace',
      sections: [{ tag: 'V1', label: 'Verse 1', lyrics: 'Amazing grace' }],
      arrangement_name: 'Standard',
    }),
    section_title: null,
    duration_sec: 240,
    template_id: null,
  },
];

let host;
let app;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_plans':
        return Promise.resolve([PLAN]);
      case 'plan_items':
        return Promise.resolve(CUES);
      case 'list_templates':
        return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
      case 'list_media':
      case 'list_announcements':
      case 'search_scripture':
      case 'search_songs':
      case 'list_arrangements':
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
  capture.update((c) => ({ ...c, available: true }));
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  document.body.innerHTML = '';
});

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

async function until(predicate, what, tries = 50) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
  }
  throw new Error(`timed out waiting for: ${what}`);
}

describe('§2 · the Planner builds, and cannot take a screen', () => {
  it('does not import a single command that changes what is on a screen', () => {
    const imported = storeImports(src);
    expect(imported.length).toBeGreaterThan(5); // the scanner still sees the list
    expect(imported.filter((n) => FIRE_WRAPPERS.includes(n))).toEqual([]);
  });

  it('names no fire command as a bare string either', () => {
    // The import list is the front door; `invoke('manual_fire')` is the back one.
    // capture.js is the ONE place a Tauri command is named (CLAUDE.md), so a
    // command string appearing in a view is already wrong — but this asserts the
    // narrow, dangerous case rather than the style rule.
    for (const cmd of TAKES_A_SCREEN) {
      expect(src).not.toContain(`'${cmd}'`);
    }
  });

  itMounted('clicking every control it renders takes no screen', async () => {
    const ServicePlanner = (await import('./ServicePlanner.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new ServicePlanner({ target: host });

    await until(() => host.querySelector('.sp-railcard'), 'the plan rail to list a plan');
    host.querySelector('.sp-railcard').click();
    await until(() => host.querySelector('.sp-row'), 'the running order to draw its cues');

    // Press everything the workspace offers, in both left modes and all three
    // inspector tabs, including the cue rows themselves (they are role="button")
    // and the per-row move/remove minis. Deleting cues out from under the list is
    // fine and deliberate: what is under test is what reaches the wall, not
    // whether the plan survives being mashed.
    for (let pass = 0; pass < 3; pass += 1) {
      const controls = [
        ...host.querySelectorAll('button, [role="button"], .sp-row, .sp-result, .sp-slide'),
      ];
      for (const el of controls) {
        if (el.disabled) continue;
        el.click();
        await settle();
      }
      await settle();
    }

    const dispatched = invoke.mock.calls.map(([cmd]) => cmd);
    expect(dispatched.length).toBeGreaterThan(3); // it really did drive the component
    expect(dispatched.filter((c) => TAKES_A_SCREEN.includes(c))).toEqual([]);
  });

  itMounted('the one path to a screen is Run in Live, and it only hands over', async () => {
    // Start from a service that IS on air, because that is the case the assertion
    // is about: the Planner may hand Live a plan and a playhead, and it may not
    // make a claim in either direction about what the congregation can see. An
    // assertion that `liveOnAir` is false would pass on the default and prove
    // nothing; an assertion that the Planner SET it false would be asserting a
    // bug, since clearing a screen is Live's job and the panic controls'.
    const { setSession, session } = await import('../session.js');
    setSession({ liveOnAir: true, liveCueId: 99 });

    const ServicePlanner = (await import('./ServicePlanner.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new ServicePlanner({ target: host });

    await until(() => host.querySelector('.sp-railcard'), 'the plan rail to list a plan');
    host.querySelector('.sp-railcard').click();
    await until(() => host.querySelector('.sp-row'), 'the running order to draw its cues');

    const run = [...host.querySelectorAll('button')].find((b) =>
      /Run in Live/i.test(b.textContent),
    );
    expect(run).toBeTruthy();
    expect(run.disabled).toBe(false);

    invoke.mockClear();
    run.click();
    await settle();

    // It sets the session and changes tab. It does not invoke anything, and
    // certainly nothing that paints. `liveCueId: null` is the PLAYHEAD, not
    // on-air-ness — the two are separate facts (CLAUDE.md, `liveCue`) and only
    // Live and the panic controls may touch the second.
    expect(invoke.mock.calls.map(([c]) => c).filter((c) => TAKES_A_SCREEN.includes(c))).toEqual([]);
    let s;
    session.subscribe((v) => (s = v))();
    expect(s.activeTab).toBe('live');
    expect(s.planId).toBe(PLAN.id);
    expect(s.liveCueId).toBe(null); // the playhead starts this plan at the top
    expect(s.liveOnAir).toBe(true); // …and the Planner said nothing about the wall
  });
});
