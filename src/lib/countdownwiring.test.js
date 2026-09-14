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

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const src = readFileSync(resolve(process.cwd(), 'src/lib/Dock.svelte'), 'utf8');

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
    const script = src.slice(0, src.indexOf('</script>'));
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
  const args = () => invoke.mock.calls.find((c) => c[0] === 'start_countdown')?.[1];

  it('carries the label and the done message over, so the wall does not rename itself', async () => {
    cap.live.set({
      reference: 'Doors open in',
      countdown_to: Date.now() + 300_000,
      countdown_done: 'Please come in',
    });
    await cap.adjustCountdown(4 * 60_000);
    expect(args().label).toBe('Doors open in');
    expect(args().doneMsg).toBe('Please come in');
    expect(args().minutes).toBe(4);
  });

  // DECISIONS §29. A countdown fired from the dock resolves through the CONTENT
  // LOOK, which defers to whatever template each screen has of its own. The
  // resolved id comes back on the live content — and handing it back to
  // `start_countdown` as `templateId` makes it a PINNED cue template, which
  // overrides the screen's own. A press of "+1" would silently re-skin every
  // screen in the building.
  it('does NOT re-pin a template the countdown never pinned', async () => {
    cap.live.set({
      reference: 'Service begins in',
      countdown_to: Date.now() + 300_000,
      template_id: 7,
      template_pinned: false,
    });
    await cap.adjustCountdown(4 * 60_000);
    expect(args().templateId).toBe(null);
  });

  it('…but keeps a template the cue DID pin', async () => {
    cap.live.set({
      reference: 'Service begins in',
      countdown_to: Date.now() + 300_000,
      template_id: 7,
      template_pinned: true,
    });
    await cap.adjustCountdown(4 * 60_000);
    expect(args().templateId).toBe(7);
  });

  it('refuses a target of zero rather than letting the backend substitute five minutes', async () => {
    await expect(cap.adjustCountdown(0)).rejects.toBeTruthy();
    expect(invoke).not.toHaveBeenCalled();
  });
});

describe('the word to the preacher survives the dock being destroyed', () => {
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
