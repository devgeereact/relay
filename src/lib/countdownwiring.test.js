// THE COUNTDOWN TRANSPORT, ON THE SURFACE THAT RENDERS IT.
//
// `countdown.test.js` pins the decisions. A pure decision layer nothing renders is
// not covered, however green its tests (CLAUDE.md, testing §), and this repository's
// recurring bug is a rule kept on one door and skipped on its twin. So this file
// asks the questions the pure tests cannot:
//
//   · does anything render the transport at all?
//   · do ALL five presses go through the one arbiter, or is there a second path?
//   · can the button labelled "Clear", one panel from the red panic control, reach
//     `clear_screens`?
//   · does re-aiming a running countdown change the number and NOTHING ELSE —
//     not its label, not its done message, and above all not which template the
//     screens are wearing (DECISIONS §29)?
//
//   npx vitest run src/lib/countdownwiring.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const src = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

/**
 * THE COMPONENT'S OWN SCRIPT — not whichever `<script>` happens to come first.
 *
 * These scanners sliced `src.slice(0, src.indexOf('</script>'))`, which was the
 * instance script for exactly as long as `Dock.svelte` had only one. It now opens
 * with a `<script context="module">` (the format picker's choice has to survive
 * the shell destroying this component on Full screen, the same reason
 * `countdown.js` keeps the set duration at module scope), and both scanners
 * quietly narrowed to that block instead — where `press` does not live. They went
 * on passing nothing rather than failing, which is the exact failure mode
 * `ipc.test.js` records twice: a scanner that narrows looks exhaustive while
 * checking less than it claims.
 *
 * `instanceScript()` finds the LAST `<script` that is not a module script, and
 * the test below holds it to that — so the next block added here cannot make
 * these two checks vacuous in silence.
 */
function instanceScript() {
  const open = src.lastIndexOf('<script>');
  const close = src.indexOf('</script>', open);
  return src.slice(open, close);
}

it('the scanner below reads the component script, not the module one', () => {
  // The thing it is FOR: `press` is in the instance script and nowhere else.
  expect(instanceScript()).toContain('function press(');
  // And it really is a narrower slice than the file, so a passing scan means
  // something was found rather than everything was searched.
  expect(instanceScript().length).toBeLessThan(src.length);
  // The module script exists and is NOT what the scanners get.
  expect(src).toContain('<script context="module">');
  expect(instanceScript()).not.toContain('context="module"');
});

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(null);
  cap.live.set(null);
});

describe('the transport is rendered, and its presses go through the one arbiter', () => {
  it('the dock draws Start · Reset · ±1 · Clear', () => {
    const bar = src.slice(src.indexOf('cdtrans'), src.indexOf('</div>', src.indexOf('cdtrans')));
    for (const label of ['start', 'reset', 'minus', 'plus', 'clear']) {
      expect(bar).toContain(`press('${label}')`);
    }
  });

  it('the hh : mm : ss fields are three named controls, not one minutes box', () => {
    for (const name of ['Countdown hours', 'Countdown minutes', 'Countdown seconds']) {
      expect(src).toContain(`aria-label="${name}"`);
    }
  });

  // The whole point of the pure module. A second path — an `on:click` that called
  // `startCountdown` or `adjustCountdown` straight — would be a press whose
  // refusals nothing tested.
  it('there is no second path to a screen: every broadcast comes from `press`', () => {
    const script = instanceScript();
    const body = script.slice(script.indexOf('function press('));
    expect(body).toMatch(/startCountdown\(/);
    expect(body).toMatch(/adjustCountdown\(/);
    // …and those are the ONLY places either is named outside the import list.
    const afterImports = script.slice(script.indexOf("from './countdown.js'"));
    expect([...afterImports.matchAll(/\bstartCountdown\(/g)].length).toBe(1);
    expect([...afterImports.matchAll(/\badjustCountdown\(/g)].length).toBe(1);
  });

  // A control called "Clear", one panel away from the red one that blanks a wall,
  // in a dark booth, mid-service. §7 says it "resets it without removing the tool";
  // it must not be able to do anything to a screen at all.
  it('the tool’s Clear can never reach a screen', async () => {
    const { countdownPress } = await import('./countdown.js');
    expect(countdownPress('clear', 9 * 60_000, 120_000).broadcastMs).toBe(null);
    // and it says so where an operator will read it
    expect(src).toMatch(/It does not clear the screens\./);
  });
});

describe('re-aiming a running countdown changes the number and nothing else', () => {
  const adjust = () => invoke.mock.calls.find((c) => c[0] === 'adjust_countdown')?.[1];

  // THE CONSOLE NO LONGER REBUILDS THE FIRE, AND THAT IS THE POINT.
  //
  // It used to: the label, the done message and the template were read back off
  // `$live` and handed to `start_countdown` again, and the three tests that used to
  // sit here pinned each of those hand-offs. That worked exactly as long as every
  // caller remembered every field — and `countdown_paused_ms` is one more to forget,
  // with the worst possible failure: a `+1` on a held countdown silently restarts it
  // in front of a congregation.
  //
  // So the carry-over moved into the engine (`main::adjust_countdown`), where it
  // holds for every caller rather than for this one, and the guarantees moved with
  // it — `e2e::r7_*` pins the label, the done message, the unpinned template
  // (DECISIONS §29) and the hold. What is left to check HERE is that the console
  // really does ask for one change rather than re-describing the countdown.
  it('asks the engine to change the time, and describes nothing else about the countdown', async () => {
    cap.live.set({
      reference: 'Doors open in',
      countdown_to: Date.now() + 300_000,
      countdown_done: 'Please come in',
      template_id: 7,
      template_pinned: false,
    });
    await cap.adjustCountdown(4 * 60_000);
    expect(invoke.mock.calls.map((c) => c[0])).toEqual(['adjust_countdown']);
    expect(adjust()).toEqual({ remainingMs: 4 * 60_000, paused: null });
    // Not one word about the label, the done message or the template: a re-aim that
    // restates them is a re-aim that can get one of them wrong.
    expect(Object.keys(adjust()).sort()).toEqual(['paused', 'remainingMs']);
  });

  it('refuses a target of zero rather than letting the backend substitute five minutes', async () => {
    await expect(cap.adjustCountdown(0)).rejects.toBeTruthy();
    expect(invoke).not.toHaveBeenCalled();
  });
});

// ── PAUSE ───────────────────────────────────────────────────────────────────
//
// §7 asks for Start/Pause · Reset · ±1 · Clear, and Pause was the one of the five
// that was never built. Every other press re-aims an absolute instant, which is
// something `countdown_to` can already say; "stopped" is not an instant, so it took
// a field the engine owns. These are the console's half of it.
describe('holding the countdown', () => {
  const adjust = () => invoke.mock.calls.find((c) => c[0] === 'adjust_countdown')?.[1];

  it('asks for the hold and nothing else — a pause must not move the number', async () => {
    cap.live.set({ reference: 'Service begins in', countdown_to: Date.now() + 300_000 });
    await cap.pauseCountdown(true);
    expect(adjust()).toEqual({ remainingMs: null, paused: true });
    invoke.mockClear();
    await cap.pauseCountdown(false);
    expect(adjust()).toEqual({ remainingMs: null, paused: false });
  });

  // The transport reads how long is left through the ONE reader, so a held
  // countdown reads as on the wall — not as "nothing is counting down", which would
  // re-enable Start and let a second countdown be laid over the first.
  it('a HELD countdown is still on the wall as far as the transport is concerned', () => {
    cap.live.set({
      reference: 'Service begins in',
      // Deliberately an instant in the PAST: a countdown held for longer than it had
      // left is the ordinary case (hold at 4:00, the preacher talks for ten minutes).
      // Read as an instant it is finished; read correctly it is still showing 4:00.
      countdown_to: Date.now() - 60_000,
      countdown_paused_ms: 4 * 60_000,
    });
    expect(cap.countdownRunning()).toBe(true);
    expect(cap.countdownRemaining()).toBe(4 * 60_000);
    expect(cap.countdownHeld()).toBe(true);
  });

  it('and the transport refuses to start a second one over it', async () => {
    const { countdownCan, countdownPress } = await import('./countdown.js');
    expect(countdownCan('start', 5 * 60_000, 4 * 60_000, true)).toBe(false);
    expect(countdownPress('start', 5 * 60_000, 4 * 60_000, true).refused).toMatch(/already running/);
    // …while ±1 still re-aims it, WITHOUT releasing the hold.
    const plus = countdownPress('plus', 5 * 60_000, 4 * 60_000, true);
    expect(plus.broadcastMs).toBe(5 * 60_000);
    expect(plus.pause).toBe(null);
  });
});

// ── THE WALL ITSELF ─────────────────────────────────────────────────────────
//
// Everything above is a decision about a number. This is the number on the screen
// the congregation is looking at, through the ONE renderer, with the clock moved by
// hand — because a held countdown that holds in the console and ticks on the wall is
// worse than no Pause at all.
describe('a held countdown holds on the wall', () => {
  let host;
  let app;
  const AT = 1_700_000_000_000;
  const template = {
    id: 40,
    name: 'Timer',
    layout: { regions: ['verse_text', 'reference'], align: 'center' },
    style: {},
  };
  const mount = async (content) => {
    const TemplateRender = (await import('./TemplateRender.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new TemplateRender({ target: host, props: { template, content } });
    return host;
  };
  afterEach(() => {
    app?.$destroy();
    host?.remove();
    vi.useRealTimers();
  });

  const figure = (el) => el.querySelector('.countdown')?.textContent.trim();

  it('paints the held figure, and does not move when time does', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(AT);
    const el = await mount({
      kind: 'countdown',
      reference: 'Service begins in',
      // Held at 4:00. The instant is a minute in the PAST — the ordinary state of a
      // countdown held for longer than it had left. Read as an instant this
      // countdown is over and the wall would show the done message.
      countdown_to: AT - 60_000,
      countdown_from: AT - 360_000,
      countdown_paused_ms: 4 * 60_000,
      countdown_done: 'Welcome',
    });
    await tick();
    expect(figure(el)).toBe('4:00');
    expect(el.textContent).not.toMatch(/Welcome/);
    // Two minutes of wall-clock later it still says 4:00. This is the whole claim.
    // `advanceTimersByTime` moves the mocked `Date.now()` as well as the interval,
    // so the renderer's clock and the wall clock move together — as they do in a room.
    vi.advanceTimersByTime(120_000);
    await tick();
    expect(figure(el)).toBe('4:00');
  });

  it('and starts moving again the moment the hold is released', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(AT);
    const el = await mount({
      kind: 'countdown',
      reference: 'Service begins in',
      countdown_to: AT - 60_000,
      countdown_from: AT - 360_000,
      countdown_paused_ms: 4 * 60_000,
    });
    await tick();
    expect(figure(el)).toBe('4:00');
    // What `adjust_countdown` broadcasts on resume: the hold gone, the instant
    // re-aimed to now + what was left.
    app.$set({
      content: {
        kind: 'countdown',
        reference: 'Service begins in',
        countdown_to: AT + 4 * 60_000,
        countdown_from: AT - 360_000,
        countdown_paused_ms: null,
      },
    });
    await tick();
    expect(figure(el)).toBe('4:00');
    vi.advanceTimersByTime(61_000);
    await tick();
    expect(figure(el)).toBe('2:59');
  });
});

describe('the Stage Message survives the dock being destroyed', () => {
  // The shell renders the dock as `{#if !liveFullscreen}<Dock />{/if}`. Pressing
  // Full screen destroys it. With the "did we send one" flag living in the
  // component, a word still on the preacher's monitor came back as "nothing sent"
  // — which DISABLES the only control that takes it down.
  it('the flag is a store, and the Clear button reads it', async () => {
    await cap.sendStageAlert('Five minutes left');
    const { get } = await import('svelte/store');
    expect(get(cap.stageAlert)).toBe('Five minutes left');
    expect(src).toMatch(/disabled=\{busy \|\| !\$stageAlert\}/);
    await cap.sendStageAlert(null);
    expect(get(cap.stageAlert)).toBe(null);
  });

  it('a send that FAILS does not leave the dock saying the preacher was told', async () => {
    const { get } = await import('svelte/store');
    invoke.mockRejectedValue('the hub is gone');
    await expect(cap.sendStageAlert('Wrap up')).rejects.toBeTruthy();
    expect(get(cap.stageAlert)).toBe(null);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE HOLD, ON THE SURFACE THAT RENDERS IT
//
// `countdown.js` decides and `capture.js` asks; neither is a control an operator
// can press. The engine field landed with its own tests and the transport still
// had four buttons, which is the exact shape of "a rule kept on one door and
// skipped on its twin" this file exists for.
//
// Two things are asserted and nothing else: the press reaches `pauseCountdown`
// through the ONE arbiter (never a second path), and PAUSE and RESUME are two
// buttons rather than one toggle — a toggle computed from state this panel might
// hold stale is how a press does the opposite of what its label says.
// ─────────────────────────────────────────────────────────────────────────────
describe('the transport can hold the countdown', () => {
  it('draws Pause and Resume as two separate presses, not one toggle', () => {
    const bar = src.slice(src.indexOf('cdtrans'), src.indexOf('</div>', src.indexOf('press(\'clear\')')));
    expect(bar).toContain("press('pause')");
    expect(bar).toContain("press('resume')");
    // Which one is offered is read from the CONTENT on the wall.
    expect(bar).toMatch(/\{#if cdPaused\}/);
    // No toggle: neither button computes its own instruction from a local flag.
    expect(bar).not.toMatch(/press\(cdPaused \?/);
  });

  it('the hold goes through the one arbiter — there is no second path to it', () => {
    const script = instanceScript();
    const body = script.slice(script.indexOf('function press('));
    expect(body).toMatch(/pauseCountdown\(r\.pause\)/);
    // …and that is the only place it is named outside the import list.
    const afterImports = script.slice(script.indexOf("from './countdown.js'"));
    expect([...afterImports.matchAll(/\bpauseCountdown\(/g)].length).toBe(1);
  });

  it('every button on the row is told whether it is held, so +1 cannot release it', () => {
    // `countdownCan`'s fourth argument. Without it `plus` reads as an ordinary
    // re-aim and the row would offer a press that quietly restarts a held timer.
    for (const action of ['start', 'reset', 'minus', 'plus', 'pause', 'resume']) {
      expect(src, `${action} is not told about the hold`).toContain(
        `countdownCan('${action}', $countdownSet, cdRunning, cdPaused)`,
      );
    }
    expect(src).toMatch(/countdownPress\(action, \$countdownSet, cdRunning, cdPaused\)/);
  });

  it('“on the screens” does not read the same over a countdown that has stopped', () => {
    // Rule 35, on the one figure an operator glances at from across a booth. A
    // held countdown IS on the screens; it is simply not moving.
    expect(src).toMatch(/cdPaused \? 'on the screens · held' : 'on the screens'/);
  });

  it('the warning threshold is scaled to the countdown’s REAL span, not the tool’s', () => {
    // `$countdownSet` is what Start would put up — a different number the moment
    // an operator types in the fields while one is running. The engine carries
    // the real span now, so the dock and the wall turn red together.
    expect(src).toMatch(/cdTotal = countdownTotalMs\(\$live\) \?\? \$countdownSet/);
    // The third argument is RG-149(a): a threshold chosen for this countdown beats
    // the span-scaled rule, and it is read off the same `$live` the span is, so the
    // dock and the wall cannot rank the two differently.
    expect(src).toMatch(
      /cdWarn = cdLive && countdownWarning\(cdRunning, cdTotal, \$live\?\.countdown_warn_ms\)/,
    );
  });

  it('the held state is read from the wall, never remembered by this panel', () => {
    expect(src).toMatch(/cdPaused = !!\$live && countdownHeld\(\)/);
    // No local hold flag that could outlive the content it describes.
    expect(src).not.toMatch(/let cdPaused/);
  });
});
