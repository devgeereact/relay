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

// ── AND THE CONSOLE NO LONGER CARRIES IT EITHER (2026-09-20, evening) ──────
//
// It was moved out of Quick tools into a band of its own on Live that morning,
// and taken off the console the same evening on the operator's instruction. The
// tests that asserted the band are gone with it, which is correct: a test for a
// surface that was deliberately removed is a test that argues with a decision.
//
// This is what replaces them. It is written against the SOURCE rather than
// against a mounted component on purpose, because the claim is an absence and a
// mounted component can only ever show what it does render. Reinstating the band
// should be a decision somebody makes, not something a merge does quietly.
//
// WHAT IT DOES NOT CLAIM: that a church cannot show a countdown. The Planner
// builds one as a plan cue (see the next block), and `lib/countdown.js` and
// every backend command are untouched.
/**
 * `Live.svelte` with every comment removed: HTML comments, block comments and
 * line comments.
 *
 * The assertions below are about what the file RENDERS and STYLES, and the note
 * left where the band used to be necessarily names the things it removed. Without
 * this the tests would fail on their own explanation, which would teach the next
 * person to delete the explanation rather than to keep the guarantee.
 */
const LIVE_CODE = codeOnly(LIVE_SRC);

describe('the run surface carries NO Screen Countdown — the second removal', () => {
  // THE HISTORY, BECAUSE IT HAS NOW GONE BOTH WAYS TWICE AND A READER NEEDS THE
  // ORDER. It moved from Quick tools to a band on Live on the morning of
  // 2026-09-20; the operator asked that evening for it to go and it went; the
  // 2026-09-21 audit read the note it left back as F13 and the operator asked for
  // it back (DECISIONS §109); and on 2026-09-21 the operator asked for it removed
  // COMPLETELY. DECISIONS §115 is that decision and records what it costs.
  //
  // THE COST, stated here rather than only in the decision, because this is the
  // file somebody reads when they wonder where it went: a countdown already in
  // front of a room can no longer be held, re-aimed, nudged or put back. The only
  // thing that takes it off a wall is a panic control. That was true between
  // 2026-09-20 and 2026-09-21 as well, and it is what F13 filed.
  //
  // WHAT A CHURCH CAN STILL DO: build a countdown as a Planner cue aimed at named
  // screens, and fire it from the plan (the block further down asserts that path
  // still works). `start_countdown` is untouched and still has that one caller.
  it('names no Screen Countdown and draws no transport', () => {
    for (const gone of ['Screen Countdown', 'sc-band', 'cdtrans', 'Put back on screens']) {
      expect(LIVE_CODE, `Live still renders ${gone}`).not.toContain(gone);
    }
  });

  it('and keeps none of the transport state behind the removed band', () => {
    // The same rule this file already holds against the dock, one surface along:
    // a block deleted from the markup while its reactive half stays behind is a
    // tick and two reads running for nothing, and the next reader finds a
    // `cdPress('start')` with no button and puts the band back.
    for (const ghost of ['cdPress(', 'function cdRun', 'cdUntilBad', 'cdChosen', 'cdReach', 'cdBack']) {
      expect(LIVE_CODE, `Live still holds ${ghost}`).not.toContain(ghost);
    }
  });

  it('and styles nothing it no longer draws', () => {
    // An unused selector is a surface somebody rebuilds half of by accident.
    for (const sel of ['.sc-band{', '.cdfields{', '.cdstatev{', '.sc-ch{', '@keyframes cdwarn']) {
      expect(LIVE_CODE, `Live still styles ${sel}`).not.toContain(sel);
    }
  });

  it('and the two commands it was the only caller of are gone from the bridge', () => {
    // NOT LEFT BEHIND, which is the half the FIRST removal got wrong: F13 found
    // `adjustCountdown` and `showTimer` imported into Live and called nowhere.
    // Every registered command is invokable from the webview, so one nothing
    // calls is attack surface nobody is watching (CLAUDE.md, and the precedent of
    // the ten deleted before these two).
    const CAPTURE = readFileSync(resolve(process.cwd(), 'src/lib/stores/capture.js'), 'utf8');
    for (const gone of ['adjustCountdown', 'showTimer']) {
      expect(codeOnly(CAPTURE), `the wrapper ${gone} is still here`).not.toContain(
        `export async function ${gone}`,
      );
      expect(LIVE_CODE, `Live still imports ${gone}`).not.toContain(gone);
    }
    const MAIN = readFileSync(resolve(process.cwd(), 'src-tauri/src/main.rs'), 'utf8');
    expect(MAIN, 'adjust_countdown is still registered').not.toMatch(/fn adjust_countdown\b/);
    expect(MAIN, 'show_timer is still registered').not.toMatch(/fn show_timer\b/);
  });

  it('…and `start_countdown` is NOT, because the plan cue still fires it', () => {
    // The guard on the four above. A removal that took the whole feature out
    // would satisfy every one of them and would be a different decision from the
    // one the operator made.
    expect(LIVE_CODE, 'the plan cue lost its countdown').toContain('startCountdown(');
    const MAIN = readFileSync(resolve(process.cwd(), 'src-tauri/src/main.rs'), 'utf8');
    expect(MAIN).toMatch(/fn start_countdown\b/);
  });
});

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

// ── 3 · IT CAN BE AIMED ─────────────────────────────────────────────────────

// ── 4 · THE LINE DESCRIBES WHAT IT IS AIMED AT ──────────────────────────────

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

// ── 7 · A TIME OF DAY (DECISIONS §102), MOVED FROM `quicktools.test.js` ─────
//
// "The service starts at 10:30" is the commonest countdown a church puts on a
// screen, and it was the one thing this transport could not express. Every
// creator in the product took `minutes: f64` and computed `now + minutes*60000`
// — there was no `time_of_day` in the schema, the commands, the stores or any
// control — so a time of day was arithmetic an operator did in their head, and
// it was wrong the moment the service slipped while the wall counted on.

// ── 8 · THE SOURCE OF TRUTH DID NOT FORK ────────────────────────────────────

