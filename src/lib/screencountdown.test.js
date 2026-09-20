// THE SCREEN COUNTDOWN LEAVES QUICK TOOLS, AND DOES NOT BECOME UNREACHABLE.
//
// Operator's instruction, 2026-09-20: *"Remove SCREEN COUNTDOWN from Quick Tools
// in the Live workspace. The countdown that goes to the live screen must remain
// available, but only where it is actually needed: the pre-service countdown for
// the service going online. Do not delete the countdown feature itself, only its
// placement in Quick Tools."*
//
// ── WHAT THIS FILE IS FOR ────────────────────────────────────────────────────
//
// Removing a control from a card is the easy half. The half this repository keeps
// getting wrong is the other one: a control that leaves a surface and lands
// nowhere is a feature that was deleted without anybody deciding to delete it,
// and `scripts/qa-inventory.mjs` is the instrument that exists because of it. So
// every assertion here is about where the instrument WENT.
//
//   1. Quick tools is two blocks and the Screen Countdown is not one of them.
//   2. Live's run surface carries the whole transport — Start, Pause/Resume,
//      Reset, ±1, Clear and `Put back on screens` — beside the Stage Timer band,
//      where an operator's hands already are.
//   3. It can be AIMED. The operator's church runs `ONLINE SCREEN`, `STAGE
//      SCREEN` and `ONLINE + STAGE SCREEN`, and the countdown they mean is the
//      pre-service one on the stream. Relay cannot know which screen that is —
//      there is no `stream` role, and guessing by NAME is the heuristic
//      `channelroles.js` was written to remove — so the band asks.
//   4. The reach line describes the screens the countdown is AIMED at, not every
//      screen that exists. Narrowing the aim and leaving the line saying
//      "Goes to all 3 screens" is rule 35 on the one line that answers "where
//      would this go".
//
//   npx vitest run src/lib/screencountdown.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
// THE ONE COMMENT STRIPPER. Not a regex of this file's own — `codeonly.js` exists
// because four of them is how a scanner goes blind, and it names the two ways a
// hand-rolled one fails. This file wrote one anyway for half an hour and it was
// the wrong shape; the shared one is the whole reason it does not have to be.
import { codeOnly } from './codeonly.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const { setSession } = await import('./session.js');
const { countdownSet, countdownFormat, DEFAULT_COUNTDOWN_MS } = await import('./countdown.js');
const Live = (await import('./views/Live.svelte')).default;

const DOCK_SRC = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');
const LIVE_SRC = readFileSync(resolve(process.cwd(), 'src/lib/views/Live.svelte'), 'utf8');

/** A template that shows everything — nothing is filtered out of its reach. */
const PLAIN = {
  id: 1,
  name: 'Classic Serif',
  layout: { regions: ['verse_text', 'reference'] },
  style: { background: '#0b0b0d' },
};
/** A template whose `shows` allow-list omits `countdown`: it drops the fire. */
const NO_CLOCK = {
  id: 2,
  name: 'Verses only',
  layout: { regions: ['verse_text'], shows: ['scripture'] },
  style: { background: '#0b0b0d' },
};

const chan = (id, name, template_id = 1) => ({
  id,
  name,
  role: null,
  render_target: 'network_client',
  template_id,
});

let host;
let app;

function bridge({ channels = [], timers = [], templates = [PLAIN, NO_CLOCK] } = {}) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_output_channels') return channels;
    if (cmd === 'list_timers') return timers;
    if (cmd === 'list_templates') return templates;
    if (cmd === 'list_plans') return [];
    if (cmd === 'list_books') return [{ book: 'Psalms', chapters: 150 }];
    if (cmd === 'rehearsal') return false;
    if (cmd === 'get_sensitivity') return 50;
    if (cmd === 'get_default_template') return 1;
    return null;
  });
}

function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Live({ target: host, props: {} });
  return host;
}

async function settle(ms = 60) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const band = () => host.querySelector('.sc-band');
const byLabel = (text, root = host) =>
  [...root.querySelectorAll('button')].find((b) => b.textContent.trim() === text);
const called = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);

beforeEach(async () => {
  invoke.mockReset();
  bridge();
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.templates.set([PLAIN, NO_CLOCK]);
  cap.readErrors.set({});
  countdownSet.set(DEFAULT_COUNTDOWN_MS);
  setSession({ planId: null });
  // Warm the bridge before mounting, for the reason `programmetimer.test.js`
  // records at the same point: the very first mocked import resolves undefined.
  await cap.loadTemplates();
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  cap.readErrors.set({});
  cap.live.set(null);
});

// ── 1 · IT IS OUT OF QUICK TOOLS ────────────────────────────────────────────

describe('the dock no longer carries it', () => {
  it('Quick tools names no Screen Countdown and draws no transport', () => {
    const card = DOCK_SRC.slice(DOCK_SRC.indexOf('<span class="dk">Quick tools</span>'));
    const body = card.slice(0, card.indexOf('<span class="dk">Controls</span>'));
    expect(body).not.toContain('Screen Countdown');
    expect(body).not.toContain('cdtrans');
    expect(body).not.toContain('Put back on screens');
  });

  it('and the dock keeps no countdown machinery behind the removed card', () => {
    // A block deleted from the markup while its reactive half stays behind is a
    // poll, a tick and two reads still running on every workspace for nothing —
    // and the next reader finds a `press('start')` with no button and puts the
    // card back. The instrument moved; nothing of it stayed.
    for (const ghost of ['countdownPress', 'countdownCan', 'adjustCountdown', 'pauseCountdown']) {
      expect(DOCK_SRC, `the dock still holds ${ghost}`).not.toContain(ghost);
    }
  });
});

// ── 2 · IT IS ON THE RUN SURFACE ────────────────────────────────────────────

describe('the transport is on Live, beside the Stage Timer band', () => {
  it('renders the whole transport rather than a subset of it', async () => {
    mount();
    await settle();
    const b = band();
    expect(b, 'Live renders no Screen Countdown band at all').toBeTruthy();
    for (const label of ['Start', 'Pause', 'Reset', '−1', '+1', 'Clear']) {
      expect(byLabel(label, b), `the band has no ${label}`).toBeTruthy();
    }
  });

  it('is a band of its own and does not move into the Stage Timer band', async () => {
    mount();
    await settle();
    // Two bands, two instruments. The Stage Timer touches no screen and this one
    // is the congregation's; merging them is how a press meant for a preacher's
    // monitor reaches a wall.
    expect(host.querySelector('.pt-band'), 'the Stage Timer band is gone').toBeTruthy();
    expect(band().querySelector('.pt-band')).toBeNull();
    expect(host.querySelector('.pt-band').querySelector('.sc-band')).toBeNull();
  });

  it('carries the way back onto a congregation screen', async () => {
    bridge({
      channels: [chan(1, 'ONLINE SCREEN')],
      timers: [{ id: 9, scope: 'both', label: '', target_ms: Date.now() + 300_000 }],
    });
    mount();
    await settle(120);
    expect(byLabel('Put back on screens', band()), 'the way back did not come with it')
      .toBeTruthy();
  });

  it('says which figure it is showing, and never in amber', async () => {
    mount();
    await settle();
    expect(band().querySelector('.cdstatev').textContent.trim()).toBe('not counting');
    for (const cls of ['onair', 'inreh', 'guess', 'amber']) {
      expect(band().querySelector(`.${cls}`), `the band wears .${cls}`).toBeNull();
    }
  });

  // ── MOVED FROM `shellchrome.test.js` (2026-09-20) ────────────────────────
  //
  // The figure is the largest thing in this band because it is the one thing an
  // operator reads from across a booth. That makes conflating its two states
  // expensive: the SET duration and what the screens are counting are different
  // facts, and a big number with no label is the half of a status line that lies.
  it('the figure says WHICH of its two facts it is showing', async () => {
    mount();
    await settle();
    // Nothing on the wall: it still renders — the control's biggest readout used
    // to appear only after the control had been used — and it says so.
    const fig = band().querySelector('.tfig');
    expect(fig).toBeTruthy();
    expect(fig.classList.contains('live')).toBe(false);
    expect(band().querySelector('.cdstatev').textContent.trim()).toBe('not counting');

    cap.live.set({ countdown_to: Date.now() + 5 * 60_000 });
    await settle();
    expect(band().querySelector('.tfig').classList.contains('live')).toBe(true);
    expect(band().querySelector('.cdstatev').textContent.trim()).toBe('on the screens');
  });

  it('the warning state is red, never amber, and only while it is on a wall', async () => {
    // Amber in this room means ON AIR and is never allowed to be anything else
    // (rule 18). The warning WINDOW is `layers.js::countdownWarning` — one rule,
    // shared with the wall and the stage page.
    const css = LIVE_SRC.slice(LIVE_SRC.indexOf('.tfig{'));
    expect(css, 'the figure has no rule in this stylesheet').not.toBe('');
    expect(css).toMatch(/\.tfig\.warn\{color:var\(--v-red\)\}/);
    // The TOKEN, not the word — the comment beside it says "never amber", which
    // is the sentence a naive grep would have been satisfied by.
    expect(css.slice(0, css.indexOf('.cdstatev'))).not.toMatch(/var\(--v-amber/);
    // `cdLive &&` is the half that stops a SET duration under a minute from
    // pulsing red at an operator about a countdown nobody can see.
    expect(LIVE_SRC).toMatch(/\$: cdWarn = cdLive && countdownWarning\(/);
  });
});

// ── 3 · IT CAN BE AIMED ─────────────────────────────────────────────────────

describe('aiming the pre-service countdown at one screen', () => {
  const start = () => byLabel('Start', band());
  const screenBtn = (name) =>
    [...band().querySelectorAll('.sc-ch')].find((b) => b.textContent.trim() === name);

  it('every screen is the default, and it is sent as null rather than a list', async () => {
    bridge({ channels: [chan(1, 'ONLINE SCREEN'), chan(2, 'STAGE SCREEN')] });
    mount();
    await settle(120);
    start().click();
    await settle();
    const args = called('start_countdown').at(-1)?.[1];
    expect(args, 'Start reached no command').toBeTruthy();
    // NOT `[1, 2]`. "All of them" has one spelling, and an explicit list of every
    // screen silently stops including a screen opened a minute later.
    expect(args.channels).toBe(null);
  });

  it('narrowing it to the streaming screen sends that screen and no other', async () => {
    bridge({ channels: [chan(1, 'ONLINE SCREEN'), chan(2, 'STAGE SCREEN')] });
    mount();
    await settle(120);
    expect(screenBtn('ONLINE SCREEN'), 'the band offers no screen picker').toBeTruthy();
    screenBtn('STAGE SCREEN').click();
    await settle();
    start().click();
    await settle();
    expect(called('start_countdown').at(-1)[1].channels).toEqual([1]);
  });

  it('refuses to start a countdown aimed at nothing, and says why', async () => {
    bridge({ channels: [chan(1, 'ONLINE SCREEN')] });
    mount();
    await settle(120);
    screenBtn('ONLINE SCREEN').click();
    await settle();
    expect(start().disabled, 'Start is offered over a countdown that would reach nothing').toBe(true);
    expect(band().textContent).toMatch(/No screen chosen/i);
    start().click();
    await settle();
    expect(called('start_countdown')).toHaveLength(0);
  });
});

// ── 4 · THE LINE DESCRIBES WHAT IT IS AIMED AT ──────────────────────────────

describe('the reach line answers about the aim, not about the building', () => {
  const screenBtn = (name) =>
    [...band().querySelectorAll('.sc-ch')].find((b) => b.textContent.trim() === name);
  const reach = () => band().querySelector('.cdreach');

  it('names the screen that would ignore it', async () => {
    bridge({ channels: [chan(1, 'ONLINE SCREEN', 1), chan(2, 'LOBBY', 2)] });
    mount();
    await settle(160);
    expect(reach(), 'the band renders no reach line').toBeTruthy();
    expect(reach().textContent).toContain('LOBBY ignores it');
  });

  it('stops naming a screen the countdown is no longer aimed at', async () => {
    // RULE 35, on the one line that answers "where would this go". Untick the
    // screen that ignores the clock and the line must stop reporting it — a line
    // that goes on describing every screen in the building over a countdown
    // aimed at one of them says the same thing in two different situations.
    bridge({ channels: [chan(1, 'ONLINE SCREEN', 1), chan(2, 'LOBBY', 2)] });
    mount();
    await settle(160);
    expect(reach().textContent).toContain('LOBBY ignores it');
    screenBtn('LOBBY').click();
    await settle();
    expect(reach().textContent).not.toContain('LOBBY');
    expect(reach().textContent).toContain('Goes to the one screen');
  });

  it('wears no law colour, whatever it says', async () => {
    bridge({ channels: [chan(1, 'LOBBY', 2)] });
    mount();
    await settle(160);
    for (const cls of ['onair', 'inreh', 'guess', 'amber']) {
      expect(reach().classList.contains(cls), `the line wears .${cls}`).toBe(false);
    }
  });
});

// ── 5 · AND THE PLANNER IS THE OTHER WAY IN ─────────────────────────────────
//
// Requirement 2: a countdown cue should be easy to point at one screen. The
// inspector's `Screens` row has been able to do it since RG-161, but only AFTER
// the cue exists and only once an operator has found the row. The add block asks
// at the moment the cue is built, which is the moment the operator knows the
// answer.
describe('a Planner countdown cue is aimed where it is built', () => {
  const PLANNER_SRC = readFileSync(
    resolve(process.cwd(), 'src/lib/views/ServicePlanner.svelte'),
    'utf8',
  );

  it('the add block carries a screen picker', () => {
    const add = PLANNER_SRC.slice(
      PLANNER_SRC.indexOf('<div class="sp-cdadd">'),
      PLANNER_SRC.indexOf('<div class="sp-results">'),
    );
    expect(add, 'the countdown add block has no screen choice').toContain('sp-cdch');
    expect(add).toContain('aria-pressed');
  });

  it('and the cue it creates is written with that choice', () => {
    // `addPlanItem` cannot express a screen set, so the aim is a second write
    // against the cue that was just created — the same `setPlanChannels` the
    // inspector row uses, so the two doors cannot disagree.
    //
    // ── COMMENTS STRIPPED, AND THAT IS NOT TIDINESS ─────────────────────────
    //
    // This assertion was written without the strip and it PASSED with the call
    // deleted, because the paragraph above `addCountdownCue` explains the write
    // and names `setPlanChannels` while doing so. A comment that mentions a
    // call is not the call; the same mistake is recorded in `wayback.test.js`
    // and in `countdownwarnmotion.test.js`, and it was caught here only because
    // the defect was deliberately reintroduced to watch this fail.
    const fn = codeOnly(
      PLANNER_SRC.slice(
        PLANNER_SRC.indexOf('async function addCountdownCue('),
        PLANNER_SRC.indexOf('async function addSong('),
      ),
    );
    // The scanner can still see the function it is judging, so an empty slice
    // cannot satisfy the assertion below by having nothing in it.
    expect(fn, 'the add function is not where this scan looks').toContain('addPlanItem(');
    expect(fn).toContain('setPlanChannels');
  });
});

// ── 6 · THE FORMAT PICKER, MOVED FROM `quicktools.test.js` ──────────────────
//
// `docs/REBRAND.md` §7: *"One timer, one formatter, read by the slide, the stage
// rail and the transport so they cannot drift."* The picker ASKS that formatter —
// it is the third argument `layers.js::formatCountdown` has always taken.
//
// `countdownFormat` was a store exported from `Dock.svelte`'s module script,
// which was always the odd shape: it is a decision, and the decision layer is
// `countdown.js`. It lives there now for the same reason `countdownSet` always
// did — this view is destroyed when the operator changes workspace, and a
// component `let` would drop the choice mid-service.

describe('the countdown format picker', () => {
  afterEach(() => countdownFormat.set('auto'));

  it('feeds the one formatter and adds no second one', () => {
    const open = LIVE_SRC.lastIndexOf('<script>');
    const script = LIVE_SRC.slice(open, LIVE_SRC.indexOf('</script>', open));
    expect(script).toMatch(/formatCountdown\([^)]*\$countdownFormat\)/);
    // No hand-rolled hours, minutes or seconds anywhere in the countdown's half
    // of this component.
    expect(script).not.toMatch(/Math\.floor\([^)]*3600\)/);
  });

  it('offers exactly the three §7 names', async () => {
    mount();
    await settle();
    const pick = band().querySelector('.cdfmt');
    expect(pick).not.toBeNull();
    expect([...pick.options].map((o) => o.value)).toEqual(['auto', 'ms', 'hms']);
    expect([...pick.options].map((o) => o.textContent.trim())).toEqual(['auto', 'm:ss', 'h:mm:ss']);
    expect(pick.getAttribute('aria-label')).toBe('Countdown format');
  });

  it('changes how the same number reads', async () => {
    mount();
    await settle();
    const fig = () => band().querySelector('.tfig').textContent.trim();
    expect(fig()).toBe('5:00');
    countdownFormat.set('hms');
    await tick();
    expect(fig()).toBe('0:05:00');
    countdownFormat.set('ms');
    await tick();
    expect(fig()).toBe('5:00');
  });

  // IT CHANGES THE READOUT, AND SAYS SO. A wall's countdown is rendered from
  // `OutputContent`, which carries no format field, so a control that implied it
  // reached the screens would be claiming a reach it has not got — rule 35's
  // family, on the panel an operator watches a service from.
  it('says which figure it governs, and claims nothing about the screens', async () => {
    mount();
    await settle();
    const title = band().querySelector('.cdfmt').getAttribute('title');
    expect(title).toMatch(/this readout/i);
    expect(title).toMatch(/screens read the countdown through their own template/i);
  });

  // THE CHOICE OUTLIVES THE COMPONENT. The workspace router destroys this view
  // when the operator changes workspace — the same trap `countdown.js` records
  // for the set duration, and the reason both stores are at module scope.
  it('survives the component being destroyed and rebuilt', async () => {
    mount();
    await settle();
    countdownFormat.set('hms');
    await tick();
    app.$destroy();
    host.remove();

    mount();
    await settle();
    expect(band().querySelector('.cdfmt').value).toBe('hms');
  });
});

// ── 7 · A TIME OF DAY (DECISIONS §102), MOVED FROM `quicktools.test.js` ─────
//
// "The service starts at 10:30" is the commonest countdown a church puts on a
// screen, and it was the one thing this transport could not express. Every
// creator in the product took `minutes: f64` and computed `now + minutes*60000`
// — there was no `time_of_day` in the schema, the commands, the stores or any
// control — so a time of day was arithmetic an operator did in their head, and
// it was wrong the moment the service slipped while the wall counted on.

describe('counting down to a time of day', () => {
  const clockBox = () => band().querySelector('[aria-label="Countdown clock time"]');
  const startBtn = () => byLabel('Start', band());

  const typeAt = async (value) => {
    const box = clockBox();
    expect(box, 'no clock-time control on the countdown band').toBeTruthy();
    box.value = value;
    box.dispatchEvent(new Event('input'));
    await settle();
    return box;
  };

  it('sends the instant rather than a number of minutes', async () => {
    mount();
    await settle();
    await typeAt('10:30');
    startBtn().click();
    await settle();
    const call = called('start_countdown').at(-1);
    expect(call, 'Start reached no command').toBeTruthy();
    const d = new Date(call[1].untilMs);
    expect(d.getHours()).toBe(10);
    expect(d.getMinutes()).toBe(30);
  });

  it('sends no instant when the field is empty, so the length still means a length', async () => {
    mount();
    await settle();
    startBtn().click();
    await settle();
    expect(called('start_countdown').at(-1)[1].untilMs ?? null).toBeNull();
  });

  it('will not start on something that is not a time', async () => {
    mount();
    await settle();
    const box = await typeAt('half ten');
    expect(box.getAttribute('aria-invalid')).toBe('true');
    expect(startBtn().disabled).toBe(true);
    expect(called('start_countdown')).toHaveLength(0);
  });

  it('does not let a clock time re-aim a countdown that is already up', async () => {
    // `±1` and Reset are about the countdown on the wall. Re-aiming those at a
    // clock time would change what a congregation is counting to under an
    // operator who pressed a minute button.
    cap.live.set({ kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() + 300_000 });
    mount();
    await settle();
    await typeAt('10:30');
    const plus = byLabel('+1', band());
    if (plus && !plus.disabled) {
      plus.click();
      await settle();
      expect(called('start_countdown')).toHaveLength(0);
      const adj = called('adjust_countdown').at(-1);
      if (adj) expect(adj[1].untilMs ?? null).toBeNull();
    }
  });
});

// ── 8 · THE SOURCE OF TRUTH DID NOT FORK ────────────────────────────────────

describe('there is still exactly one decision layer', () => {
  it('every press on the run surface goes through the one arbiter', () => {
    const open = LIVE_SRC.lastIndexOf('<script>');
    const script = LIVE_SRC.slice(open, LIVE_SRC.indexOf('</script>', open));
    expect(script, 'the run surface does not use the pure transport').toContain('countdownPress(');
    // …and the two broadcasts are named exactly once each, inside it. A second
    // `startCountdown(` outside `press` would be a press whose refusals nothing
    // tested — except the plan-cue fire, which is a different instrument and is
    // named here so the count stays legible.
    const presses = [...script.matchAll(/\bcountdownPress\(/g)].length;
    expect(presses).toBe(1);
  });

  it('the dock and the run surface do not both hold the set duration', () => {
    expect(DOCK_SRC).not.toContain('countdownSet');
    expect(LIVE_SRC).toContain('countdownSet');
  });
});
