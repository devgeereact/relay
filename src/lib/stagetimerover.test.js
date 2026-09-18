import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { timerRemainingMs } from './timers.js';
import { countdownRemainingMs } from './countdown.js';
import { formatCountdown } from './layers.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const { setSession } = await import('./session.js');
const Live = (await import('./views/Live.svelte')).default;

// ══ THE OPERATOR'S MIRROR OF THE PREACHER'S CLOCK (DECISIONS §99) ═══════════
//
// `Stage.svelte`'s programme rail has answered the NEGATIVE since RG-153 — a
// clock past its deadline reads `+4:37`, because the only question a preacher
// asks it once the time has gone is how far over. Live's Stage Timer band is the
// same clock, read by the one person who can act on the answer, and it floored at
// zero: `0:00` a second after the timer ran out, and `0:00` for the rest of the
// service. One reading over two situations that need different actions, which is
// rule 35 stated in a figure instead of a sentence.
//
// This file holds that, the `+5` grant that made `adjust_timer` reachable from a
// rendered control for the first time, and the guarantee neither of them is
// allowed to break: this band claims nothing about any screen, in no words and in
// no colour (`programmetimer.test.js`, which stays green beside this).
//
//   npx vitest run src/lib/stagetimerover.test.js

let host;
let app;

/** `ms` before or after now. Negative means the deadline has already gone by. */
const T = (leftMs, over = {}) => ({
  id: 7,
  label: 'Sermon',
  done_msg: '',
  target_ms: Date.now() + leftMs,
  from_ms: Date.now() - 25 * 60_000,
  paused_ms: null,
  warn_ms: null,
  scope: 'stage',
  plan_item_id: null,
  remaining_ms: Math.max(0, leftMs),
  ...over,
});

function bridge({ timers = [] } = {}) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_timers') return timers;
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
const chipFigure = () => host.querySelector('.pt-band .pt-fig');

beforeEach(() => {
  invoke.mockReset();
  bridge();
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.templates.set([]);
  setSession({ planId: null });
  // Warm the bridge before mounting — see `programmetimer.test.js` for why the
  // very first mocked `@tauri-apps/api/core` import resolves to undefined.
  return cap.loadTemplates();
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
});

describe('how long is left, on the two sides of zero', () => {
  it('answers the negative when the caller asks for it, and floors at zero when it does not', () => {
    // THE DEFAULT IS UNCHANGED, and that half is not decoration: the dock's
    // way-back figure is a CONGREGATION countdown, where zero is a real answer —
    // the wall reads it as "it finished" and paints the done message, and a
    // negative would never reach that branch. `programmetimer.test.js`'s
    // "never reports a negative, so a row cannot count up" pins the same default
    // from the other side and must stay green beside this.
    const t = T(-760_000); // twelve minutes forty seconds over
    expect(timerRemainingMs(t, Date.now())).toBe(0);
    expect(timerRemainingMs(t, Date.now(), { past: true })).toBeLessThan(-759_000);
  });

  it('is still ONE subtraction — the band and the preacher’s rail cannot disagree', () => {
    // `Stage.svelte`'s rail is `countdownRemainingMs(t, now, { past: true })` on
    // the timer frame. This asserts the band's chain ENDS in that same call
    // rather than restating its rule, which is the property that makes the two
    // surfaces impossible to drift.
    const now = Date.now();
    const t = T(-760_000);
    const rail = countdownRemainingMs(
      { countdown_to: t.target_ms, countdown_paused_ms: t.paused_ms },
      now,
      { past: true },
    );
    expect(timerRemainingMs(t, now, { past: true })).toBe(rail);
  });

  it('a held figure still wins over the instant, on both sides of the flag', () => {
    // The hold exception comes FIRST and must: a clock held at 4:00 whose instant
    // went by six minutes ago is stopped at four minutes, not ten minutes over.
    const held = T(-600_000, { paused_ms: 240_000 });
    expect(timerRemainingMs(held, Date.now())).toBe(240_000);
    expect(timerRemainingMs(held, Date.now(), { past: true })).toBe(240_000);
  });

  it('a timer that names no deadline is still nothing, not zero and not a negative', () => {
    expect(timerRemainingMs(T(0, { target_ms: null }), Date.now(), { past: true })).toBe(null);
    expect(timerRemainingMs(null, Date.now(), { past: true })).toBe(null);
  });
});

describe('what the band shows once the time has gone', () => {
  it('counts UP and says it is over, rather than sitting at 0:00 for the rest of the service', async () => {
    bridge({ timers: [T(-760_000)] });
    mount();
    await settle(40);

    const fig = chipFigure().textContent.replace(/\s+/g, ' ').trim();
    // THE DEFECT, in the figure an operator reads. `0:00` here is what the band
    // said a second after the clock ran out and what it went on saying twelve
    // minutes later, while the preacher's own rail said `+12:40` in red.
    expect(fig).not.toBe('0:00');
    expect(fig).toMatch(/^\+12:[34]\d over$/);
  });

  it('shows the SAME figure the preacher is looking at', async () => {
    // Not "a figure of the right shape" — the rail's own expression, verbatim,
    // on the same timer. Two surfaces about one clock.
    const t = T(-3_725_000); // an hour and change over, so the formatter emits seven characters
    bridge({ timers: [t] });
    mount();
    await settle(40);

    const railMs = countdownRemainingMs(
      { countdown_to: t.target_ms, countdown_paused_ms: t.paused_ms },
      Date.now(),
      { past: true },
    );
    const rail = `+${formatCountdown(-railMs)}`;
    expect(chipFigure().textContent.replace(/\s+/g, ' ').trim()).toBe(`${rail} over`);
  });

  it('a running clock is untouched — no sign, no word, just the time left', async () => {
    // The direction this change could most easily break. A band that put `over`
    // on a healthy clock would be worse than the bug it replaces.
    bridge({ timers: [T(300_000)] });
    mount();
    await settle(40);
    const fig = chipFigure().textContent.replace(/\s+/g, ' ').trim();
    expect(fig).toMatch(/^[45]:\d\d$/);
    expect(fig).not.toContain('over');
    expect(fig).not.toContain('+');
    expect(chipFigure().classList.contains('over')).toBe(false);
  });

  it('and an over-running clock still claims nothing about any screen', async () => {
    // `programmetimer.test.js` asserts this for a running timer. The over-time
    // branch is new markup on the same row and is not exempt from it — a
    // guarantee is only kept on the doors you checked.
    bridge({ timers: [T(-760_000)] });
    mount();
    await settle(40);
    const text = block().textContent.toLowerCase();
    for (const claim of ['on stage', 'on air', 'on the screens', 'live']) {
      expect(text).not.toContain(claim);
    }
    for (const cls of ['onair', 'inreh', 'guess', 'amber']) {
      expect(block().querySelector(`.${cls}`)).toBeNull();
      expect(block().classList.contains(cls)).toBe(false);
    }
  });
});

describe('five more minutes', () => {
  it('is reachable from a rendered control — `adjust_timer` had none', async () => {
    // A registered command nothing can reach is what this repository counts as
    // attack surface nobody is watching. `adjustTimer` had a wrapper, a test, and
    // no caller in any component; before this, extending a sermon meant Stop and
    // Start, which throws away the elapsed figure the preacher is reading.
    bridge({ timers: [T(180_000)] });
    mount();
    await settle(40);

    const grant = [...block().querySelectorAll('button')].find((b) =>
      (b.getAttribute('aria-label') || '').startsWith('Give '),
    );
    expect(grant, 'the band offers no way to extend a Stage Timer').not.toBeUndefined();
    expect(grant.getAttribute('aria-label')).toBe('Give Sermon five more minutes');

    grant.click();
    await settle();

    const [, args] = called('adjust_timer')[0];
    expect(args.timerId).toBe(7);
    // Three minutes left plus five granted. The tick is half a second, so the
    // window is the tolerance rather than the assertion.
    expect(args.remainingMs).toBeGreaterThan(478_000);
    expect(args.remainingMs).toBeLessThanOrEqual(480_000);
    // It must not release or apply a hold it was not asked about.
    expect(args.paused ?? null).toBe(null);
  });

  it('grants five minutes FROM NOW on a clock that has already run out', async () => {
    // THE ONE THAT MATTERS, and the reason `Math.max(0, left)` is there rather
    // than a plain sum. A clock 12:40 over would otherwise be re-aimed to 7:40 in
    // the past, which `timers::adjust` refuses as `TooShort` — so the press would
    // answer with a refusal at exactly the moment an operator most wants it.
    bridge({ timers: [T(-760_000)] });
    mount();
    await settle(40);

    [...block().querySelectorAll('button')]
      .find((b) => (b.getAttribute('aria-label') || '').startsWith('Give '))
      .click();
    await settle();

    const [, args] = called('adjust_timer')[0];
    expect(args.remainingMs).toBe(300_000);
  });

  it('says why when the engine refuses, and keeps the list it had', async () => {
    // Rule 35 on the control rather than on the readout: a grant that failed
    // silently leaves an operator believing the preacher has five more minutes.
    bridge({ timers: [T(180_000)] });
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'adjust_timer') throw { kind: 'refused', message: 'That timer is not running.' };
      if (cmd === 'list_timers') return [T(180_000)];
      if (cmd === 'list_output_channels') return [];
      if (cmd === 'list_templates') return [];
      if (cmd === 'list_plans') return [];
      if (cmd === 'list_books') return [{ book: 'Psalms', chapters: 150 }];
      if (cmd === 'rehearsal') return false;
      if (cmd === 'get_sensitivity') return 50;
      return null;
    });
    mount();
    await settle(40);

    [...block().querySelectorAll('button')]
      .find((b) => (b.getAttribute('aria-label') || '').startsWith('Give '))
      .click();
    await settle();

    expect(block().textContent).toContain('That timer is not running.');
    expect(block().textContent).toContain('Sermon');
  });
});
