import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { timerRemainingMs, programmeRows, railSize } from './timers.js';
import { countdownRemainingMs } from './countdown.js';
import { formatCountdown } from './layers.js';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

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

// ── A SERMON THAT HAS RUN OVER ASKS FOR ATTENTION (operator, 2026-09-21) ────
//
// "Timer not flashing red while it outrun... Make it flash and get attention when
// outrun or when time is up."
//
// THE GAP THIS CLOSES IS A CONSISTENCY ONE. The congregation countdown has
// flashed on its warning since §7 — `.countdown.warn { animation: cdwarn … }` —
// and the preacher's own rail, the one clock a person is meant to ACT on, only
// ever changed colour. A red figure among red figures on a dark stage screen, at
// arm's length, under stage lighting, is the weakest signal in the product
// pointed at the person with the least attention to spare.
//
// WHY `over` AND NOT `warn`. `warn` is true for the whole warning window, which
// an operator sets and which is routinely five minutes. A clock flashing for five
// minutes is a clock somebody stops looking at, and then the flash means nothing
// when the time actually goes. So the steady red keeps the warning window and the
// flash marks the boundary the operator asked about: time up, and every second
// after it.
//
// WHAT THIS FILE CANNOT SEE: jsdom runs no animation. It asserts the CONTRACT —
// that a row past zero is marked, that the mark drives an animation, and that the
// mark is not the warning window — and the reduced-motion fallback, because an
// operator who has turned motion off must still get a signal rather than nothing.
describe('the rail flashes when the time has actually gone', () => {
  const RENDER = readFileSync(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');

  it('a row past zero is marked `over`, and one merely warning is not', () => {
    const now = 1_700_000_000_000;
    const gone = programmeRows(
      [{ id: 1, countdown_to: now - 62_000, countdown_from: now - 600_000 }],
      now,
    );
    expect(gone[0].over, 'a timer 62s past its deadline is not marked over').toBe(true);
    expect(gone[0].v).toBe('+1:02');

    // Inside the warning window but NOT past zero: red, never flashing.
    const soon = programmeRows(
      [{ id: 2, countdown_to: now + 30_000, countdown_from: now - 600_000, warn_ms: 120_000 }],
      now,
    );
    expect(soon[0].warn, 'the warning window stopped warning').toBe(true);
    expect(soon[0].over, 'a timer still running is being flashed').toBe(false);
  });

  it('a HELD timer past zero does not flash, because nothing is running out', () => {
    // The same discipline `warn` already keeps: a preacher held two minutes over
    // is a figure somebody chose to freeze, not an alarm (RG-175).
    const now = 1_700_000_000_000;
    const held = programmeRows(
      [
        {
          id: 3,
          countdown_to: now - 120_000,
          countdown_from: now - 600_000,
          countdown_paused_ms: -120_000,
        },
      ],
      now,
    );
    expect(held[0].held).toBe(true);
    expect(held[0].over, 'a held figure is flashing at the preacher').toBe(false);
  });

  it('the renderer drives an animation off that mark, not a colour alone', () => {
    expect(RENDER, 'no `over` class on the cell').toMatch(/class:over=\{t\.over\}/);
    expect(RENDER, 'the rail declares no flash').toMatch(/@keyframes\s+lpover/);
    expect(RENDER, 'the mark drives nothing').toMatch(
      /\.lp-cell\.over\s+\.lp-val\s*\{[^}]*animation:\s*lpover/,
    );
  });

  it('and an operator with motion turned off still gets a signal', () => {
    // The precedent is `.countdown.warn`'s own reduced-motion arm: the flash
    // becomes a glow rather than nothing at all. A person who has asked for no
    // animation has not asked to be left out of the one clock they act on.
    const rm = RENDER.slice(RENDER.indexOf('@media (prefers-reduced-motion: reduce)'));
    expect(rm).toMatch(/\.lp-cell\.over\s+\.lp-val\s*\{[^}]*(text-shadow|outline|box-shadow)/);
  });
});

// ── AND THE PREACHER'S PHONE KEEPS THE SAME RULE ───────────────────────────
//
// Two surfaces render a Stage Timer: `output.html` through `TemplateRender` (a
// monitor on HDMI) and `Stage.svelte` (the phone or tablet). They are the pair
// this repository has most often got half-right — the Stage Message reached one
// and not the other for five days (RG-156), and the rehearsal gate was true of
// one publisher and false of another before that.
//
// The phone already flashed, and it flashed on `warn`: the WHOLE warning window,
// routinely five minutes. So before this change the two surfaces disagreed in
// both directions — the monitor never flashed, and the phone flashed so early
// that the flash carried no information about the moment it was for.
describe('both stage surfaces flash at the same moment, and it is the same moment', () => {
  const PHONE = readFileSync(resolve(process.cwd(), 'src/Stage.svelte'), 'utf8');

  it('the phone marks a timer that is over, from the one shared derivation', () => {
    expect(PHONE, 'the phone cell carries no `over`').toMatch(/class:over=\{t\.over\}/);
  });

  it('…and flashes on THAT, not on the warning window', () => {
    expect(PHONE, 'the phone still flashes for the whole warning window').not.toMatch(
      /\.tmr\.warn\s+\.tval\s*\{\s*animation:/,
    );
    expect(PHONE, 'the phone stopped flashing altogether').toMatch(
      /\.tmr\.over\s+\.tval\s*\{\s*animation:\s*cdwarn/,
    );
  });

  it('but a timer merely inside its warning window is still RED on both', () => {
    // The steady red is the warning; the flash is the boundary. Losing the red
    // would trade one signal for another rather than adding one.
    expect(PHONE).toMatch(/\.tmr\.warn\s+\.tval\s*\{[^}]*color:/);
    const RENDER = readFileSync(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');
    expect(RENDER).toMatch(/\.lp-cell\.warn\s+\.lp-val\s*\{[^}]*color:/);
  });
});

// ── AND THE SIZE CONTROL REACHES IT (operator, 2026-09-21) ─────────────────
//
// "The stage timer is not responding to text edit or sizing as I want it bigger."
//
// It was not. Every other text layer in this product is sized by `L.size` in
// `cqw` and fitted by `fitLayers`. The programme rail was the ONE layer whose
// figure size came entirely from its container — `92cqw / --tmrs / --tch / 0.62`
// — so the Size field the editor draws for it moved nothing at all. A control
// wired to nothing is worse than an absent one: the operator drags it, sees no
// change, and concludes the application is broken rather than that one property
// was never connected.
//
// THE UNIT CONVERSION IS THE WHOLE TRICK, and it is why this could not be a
// one-line change. `L.size` is a share of the SLIDE's width, because that is what
// `cqw` means for every other layer. But `.lprog` declares `container-type:
// inline-size`, so a `cqw` written inside it is a share of the RAIL. A rail 40%
// of the screen wide would have rendered `6cqw` at 2.4% of the screen — smaller
// than before, on the control that was asked to make it bigger. So the ratio is
// computed where both numbers are known and handed over as a plain number.
describe('the Size control reaches the programme rail', () => {
  const RENDER = readFileSync(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');

  it('the rail is handed its layer’s declared size, converted into its own container', () => {
    expect(RENDER, 'the rail never receives the layer size').toMatch(/--lp-sz:/);
    // Converted, not passed through: the multiplier is `size * 100 / w`.
    expect(RENDER, 'the size is handed over in the wrong container’s units').toMatch(
      /--lp-sz:\$\{[^}]*railSize\(L\)[^}]*\}|railSize\(L\)/,
    );
  });

  it('and the declared size is what it asks for, with the container as the CAP', () => {
    // A figure may SHRINK to fit its box — RG-147's sliced `1:30:13` is a lie a
    // preacher cannot detect, so the caps stay — but it may never grow past what
    // the designer asked for, which is what "bigger" has to mean.
    const rule = /\.lp-val\s*\{[\s\S]*?font-size:([^;]*);/.exec(RENDER);
    expect(rule, 'the figure lost its font-size').toBeTruthy();
    expect(rule[1], 'the declared size is not in the calculation').toMatch(/--lp-sz/);
    expect(rule[1], 'the container caps were dropped — a long clock can be sliced again').toMatch(
      /--tch|--lp-room/,
    );
    expect(rule[1], 'it is not a min(), so the declared size can overflow the box').toContain('min(');
  });

  it('railSize converts a share of the SCREEN into a share of the RAIL', () => {
    // The arithmetic, asserted rather than trusted: a 6cqw figure in a rail that
    // is half the screen wide is 12cqw of that rail.
    expect(railSize({ size: 6, w: 50 })).toBeCloseTo(12, 5);
    expect(railSize({ size: 6, w: 100 })).toBeCloseTo(6, 5);
    // A missing or nonsense width must not divide by zero and blank the rail.
    expect(Number.isFinite(railSize({ size: 6, w: 0 }))).toBe(true);
    expect(Number.isFinite(railSize({ size: 6 }))).toBe(true);
    expect(Number.isFinite(railSize({}))).toBe(true);
  });
});

