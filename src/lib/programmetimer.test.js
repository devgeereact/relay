import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { stageTimers, timerAsContent, timerRemainingMs } from './timers.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const { setSession } = await import('./session.js');
const Live = (await import('./views/Live.svelte')).default;

// ── THE PROGRAMME TIMER, ON THE OPERATOR'S SIDE ─────────────────────────────
//
// `list_timers` hands back the registry's own shape — `target_ms`, `paused_ms`,
// `scope` — and the one arithmetic this side of the bridge is allowed to do is
// `countdown.js::countdownRemainingMs`, which reads the CONTENT shape
// (`countdown_to`, `countdown_paused_ms`). So there is exactly one projection
// between the two, and it is the mirror of `timers::project_both` in Rust.
//
// A second reader of "how long is left" is the bug docs/REBRAND.md phase 7
// records as fixed once already, which is why these tests assert that the
// projection ends in that function rather than restating its rule.

const timer = (over = {}) => ({
  id: 1,
  label: 'Sermon',
  done_msg: '',
  target_ms: 1_700_000_300_000,
  from_ms: 1_700_000_000_000,
  paused_ms: null,
  warn_ms: null,
  scope: 'stage',
  plan_item_id: null,
  remaining_ms: 300_000,
  ...over,
});

describe('which timers the programme list may show', () => {
  it('shows the stage-scoped ones and no congregation timer', () => {
    // A congregation timer is the dock's Countdown block, in the shell, and it
    // has its own transport and its own figure. Listing it here as well would
    // put two Stops on one clock and two figures on one wall.
    const all = [
      timer({ id: 1, scope: 'stage', label: 'Sermon' }),
      timer({ id: 2, scope: 'both', label: 'Service begins in' }),
      timer({ id: 3, scope: 'stage', label: 'Notices' }),
    ];
    expect(stageTimers(all).map((t) => t.id)).toEqual([1, 3]);
  });

  it('answers with an empty list for anything that is not a list', () => {
    // A bridge that returned null must render as "no timers", not throw inside a
    // reactive block and take the run surface down with it. The CALLER is what
    // distinguishes a failed read from an empty one; this is only the shaping.
    for (const junk of [null, undefined, 'nope', 42, {}]) {
      expect(stageTimers(junk)).toEqual([]);
    }
  });
});

describe('how long is left on a programme timer', () => {
  it('projects a registry timer into the shape the one arithmetic reads', () => {
    expect(timerAsContent(timer())).toEqual({
      countdown_to: 1_700_000_300_000,
      countdown_paused_ms: null,
    });
  });

  it('counts down against the clock while it is running', () => {
    const t = timer({ target_ms: 1_700_000_300_000, paused_ms: null });
    expect(timerRemainingMs(t, 1_700_000_000_000)).toBe(300_000);
    expect(timerRemainingMs(t, 1_700_000_240_000)).toBe(60_000);
  });

  it('reports the figure it was held at, at any instant', () => {
    // A held timer is a figure, not an instant — the same rule
    // `timers::remaining_ms` keeps on the other side of the bridge. A row that
    // read the deadline would show a held clock ticking down on a console while
    // the preacher's monitor showed it stopped.
    const held = timer({ target_ms: 1_700_000_090_000, paused_ms: 90_000 });
    for (const at of [1_700_000_000_000, 1_700_000_500_000, 1_700_900_000_000]) {
      expect(timerRemainingMs(held, at)).toBe(90_000);
    }
  });

  it('never reports a negative, so a row cannot count up', () => {
    const t = timer({ target_ms: 1_700_000_300_000 });
    expect(timerRemainingMs(t, 1_700_000_900_000)).toBe(0);
  });

  it('says nothing rather than guessing when the timer names no deadline', () => {
    // Null, not zero. A row that printed 0:00 over a timer whose target never
    // arrived would read exactly like one that had just run out.
    expect(timerRemainingMs(timer({ target_ms: null, paused_ms: null }), 1)).toBe(null);
    expect(timerRemainingMs(null, 1)).toBe(null);
  });
});

// ── THE OPERATOR'S SURFACE ───────────────────────────────────────────────────
//
// A registry nothing can reach from a rendered control is a command nobody
// calls, which this repository counts as attack surface rather than a feature.
// These tests hold the control that makes `start_timer` / `list_timers` /
// `stop_timer` reachable, and the three claims it is NOT allowed to make.
//
// WHERE IT LIVES: one wrapping row in Live's stage column, between the monitors
// and the slide grid. Four more obvious homes are each ruled out by a rule with
// more weight — Quick tools holds three things by an operator instruction,
// the Controls card never scrolls, nothing may push TAKE or the arrows in the
// rack, and the inspector column holds one pane. The reasoning is at
// `loadProgrammeTimers` in `views/Live.svelte`; `quicktools.test.js` and
// `livedesk.test.js` are what hold two of the four, and both stay green.
//
// The cost of the choice is real and asserted nowhere because it is not a
// defect: unlike the dock, this is only on the Live workspace.
//
//   npx vitest run src/lib/programmetimer.test.js

let host;
let app;

const T = (over = {}) => ({
  id: 7,
  label: 'Sermon',
  done_msg: '',
  target_ms: Date.now() + 300_000,
  from_ms: Date.now(),
  paused_ms: null,
  warn_ms: null,
  scope: 'stage',
  plan_item_id: null,
  remaining_ms: 300_000,
  ...over,
});

/** What the bridge answers, per command. `list_timers` defaults to nothing running. */
function bridge({ timers = [], fail = null } = {}) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_timers') {
      if (fail) throw fail;
      return timers;
    }
    if (cmd === 'start_timer') return 9;
    if (cmd === 'list_output_channels') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'list_plans') return [];
    if (cmd === 'list_books') return [{ book: 'Psalms', chapters: 150 }];
    if (cmd === 'rehearsal') return false;
    if (cmd === 'get_sensitivity') return 50;
    return null;
  });
}

function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Live({ target: host, props: {} });
  return host;
}

async function settle(ms = 80) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const called = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);
const block = () => host.querySelector('.pt-band');
const byLabel = (text) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === text);

beforeEach(() => {
  invoke.mockReset();
  bridge();
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  // A run surface with a model loaded, or `ModelSetup` mounts into the detection
  // panel and asks for a model list this harness has none of.
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.templates.set([]);
  setSession({ planId: null });
  // WARM THE BRIDGE BEFORE MOUNTING, for the reason `livedesk.test.js` records at
  // the same point: under vitest the very FIRST
  // `await import('@tauri-apps/api/core')` against the mock resolves to
  // undefined, so whichever guarded read goes first loses its answer. It is a
  // harness artefact — the real app imports a real module — but without this the
  // loser would be whichever of Live's mount reads happened to run first, and on
  // some orderings that is `list_timers`, which would make every assertion below
  // about a bridge failure rather than about the band.
  return cap.loadTemplates();
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
});

describe('starting a programme timer', () => {
  it('is reachable from a rendered control, and asks for the STAGE scope', async () => {
    mount();
    await settle();
    expect(block()).not.toBeNull();

    const mins = host.querySelector('[aria-label="Programme timer minutes"]');
    const name = host.querySelector('[aria-label="Programme timer name"]');
    expect(mins).not.toBeNull();
    expect(name).not.toBeNull();
    mins.value = '25';
    mins.dispatchEvent(new Event('input'));
    name.value = 'Sermon';
    name.dispatchEvent(new Event('input'));
    await settle();

    byLabel('Start timer').click();
    await settle();

    const [, args] = called('start_timer')[0];
    expect(args).toMatchObject({ minutes: 25, label: 'Sermon', scope: 'stage' });
  });

  it('puts nothing in front of a congregation', async () => {
    // `start_timer` creates and publishes nothing, and this control must never
    // reach for the two doors that DO paint a wall. A programme timer appearing
    // on the congregation screens is the one mistake here that cannot be taken
    // back quietly.
    mount();
    await settle();
    const mins = host.querySelector('[aria-label="Programme timer minutes"]');
    mins.value = '25';
    mins.dispatchEvent(new Event('input'));
    await settle();
    byLabel('Start timer').click();
    await settle();

    expect(called('start_timer')).toHaveLength(1);
    for (const forbidden of ['show_timer', 'start_countdown', 'fire_content', 'fire_media']) {
      expect(called(forbidden)).toHaveLength(0);
    }
  });
});

describe('seeing and stopping one', () => {
  it('lists a running programme timer with the time left on it', async () => {
    bridge({ timers: [T({ id: 7, label: 'Sermon', target_ms: Date.now() + 300_000 })] });
    mount();
    await settle(40);
    expect(block().textContent).toContain('Sermon');
    // Through `countdownRemainingMs` and `formatCountdown`, which is what keeps
    // this figure and the preacher's monitor from drifting.
    expect(block().textContent).toMatch(/[45]:\d\d/);
  });

  it('does not list the congregation countdown, which has its own transport', async () => {
    bridge({
      timers: [
        T({ id: 7, label: 'Sermon', scope: 'stage' }),
        T({ id: 8, label: 'Service begins in', scope: 'both' }),
      ],
    });
    mount();
    await settle(40);
    expect(block().textContent).toContain('Sermon');
    expect(block().textContent).not.toContain('Service begins in');
  });

  it('stops the one under the operator’s finger and no other', async () => {
    bridge({
      timers: [T({ id: 7, label: 'Sermon' }), T({ id: 11, label: 'Notices' })],
    });
    mount();
    await settle(40);

    const stops = [...block().querySelectorAll('button')].filter((b) =>
      (b.getAttribute('aria-label') || '').startsWith('Stop'),
    );
    expect(stops).toHaveLength(2);
    expect(stops[1].getAttribute('aria-label')).toBe('Stop Notices');
    stops[1].click();
    await settle();

    expect(called('stop_timer').map((c) => c[1].timerId)).toEqual([11]);
  });

  it('renders a timer with no name as its figure alone, not as a blank row', async () => {
    // A later track makes label-less the default supply, so this block has to
    // survive it already.
    bridge({ timers: [T({ id: 7, label: '', target_ms: Date.now() + 120_000 })] });
    mount();
    await settle(40);
    expect(block().textContent).toMatch(/[12]:\d\d/);
    const stop = block().querySelector('[aria-label^="Stop"]');
    expect(stop.getAttribute('aria-label')).toBe('Stop this timer');
  });
});

describe('what the block says when the thing behind it is broken (rule 35)', () => {
  it('says the reason rather than reporting an empty programme', async () => {
    // THE WHOLE POINT. A swallowed read answers `[]`, which renders exactly like
    // a quiet Sunday — one reassuring sentence over two different situations, on
    // the surface an operator would use to find a clock counting to the wrong
    // thing. The wrapper throws for this reason; the block must not undo that.
    bridge({ fail: { kind: 'internal', message: 'the engine is not answering' } });
    mount();
    await settle(40);
    expect(block().textContent).not.toContain('No programme timer');
    expect(block().textContent).toContain('the engine is not answering');
  });

  it('keeps saying nothing is running only while that is what it was told', async () => {
    bridge({ timers: [] });
    mount();
    await settle(40);
    expect(block().textContent).toContain('No programme timer');
  });

  it('never claims a programme timer is on a screen', async () => {
    // Nothing on this side can verify that a stage tablet is painting one. Amber
    // means ON AIR and is never allowed to lie; the stage red on the alert badge
    // means the preacher's monitor is showing that alert. A row here carries
    // neither, because the fact behind them is not available to it.
    bridge({ timers: [T({ id: 7, label: 'Sermon' })] });
    mount();
    await settle(40);
    const text = block().textContent.toLowerCase();
    for (const claim of ['on stage', 'on air', 'on the screens', 'live']) {
      expect(text).not.toContain(claim);
    }
    // And in no colour either. These are the law classes this surface uses for
    // the three states a timer row must not claim — `onair` is amber, `inreh` is
    // amethyst, `guess` is cyan. None of them may appear anywhere in the band.
    for (const cls of ['onair', 'inreh', 'guess', 'amber']) {
      expect(
        block().querySelector(`.${cls}`),
        `the band wears .${cls}, which is a claim about a screen it cannot check`,
      ).toBeNull();
      expect(block().classList.contains(cls)).toBe(false);
    }
  });
});
