// R5 · A RESTART MUST NOT RESTORE "ON AIR".
//
// Amber means the congregation is looking at something, and it is never allowed
// to lie (DECISIONS §22, CLAUDE.md). `session.js` persists `liveOnAir`, and the
// boot sequence knows this: `BootSequence.svelte::resume()` clears it explicitly,
// with a comment saying exactly why —
//
//   // The POSITION comes back. `liveOnAir` does not — see RecoverSession.svelte.
//   // Amber means the congregation is looking at something, and it is never
//   // allowed to be true because an app restarted.
//   setSession({ liveOnAir: false });
//
// But `resume()` is one of three ways out of the boot sequence, and it is the only
// one that clears the flag:
//
//   resume()      → clears liveOnAir            ✔
//   startFresh()  → clearSession() clears all   ✔
//   Esc → finish() → clears NOTHING             ✘
//
// Esc is not an obscure path. It is a documented, deliberate affordance, written
// into the module's own header ("Esc skips straight to the console at any point in
// a stage"), because the boot sequence must never be the reason someone cannot
// reach the console. A volunteer relaunching after a mid-service power cut, forty
// seconds before the sermon, is exactly the person who presses it.
//
// The consequence, on the surface the operator actually reads: `Live.svelte`'s
// onMount restores `liveCue = { cueId, slide, onAir: saved.liveOnAir === true }`,
// and its cue rail renders `class:amber={planOnAir}` with the label "On Air". The
// process died, so the output windows died with it and the wall is dark — and the
// console says a cue is on air over it.
//
// THE TEST IS THE BUG: it asserts the flag is cleared however the sequence ends.
// It fails today on the Esc path only.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { get } from 'svelte/store';
import BootSequence from './boot/BootSequence.svelte';
import { session, setSession } from './session.js';
import { bootRecord, booting, resetBoot } from './boot/boot.js';
import { updateAvailable } from './updater.js';

// Every probe answers instantly and cleanly, so the sequence never holds and the
// only thing under test is how it ENDS.
const okProbes = new Proxy({}, { get: () => async () => ({ state: 'ok', note: '' }) });

let host;
let app;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new BootSequence({ target: host, props: { version: '0.1.0', probes: okProbes } });
  return host;
}

beforeEach(() => {
  vi.useRealTimers();
  localStorage.clear();
  resetBoot();
  bootRecord.set({ cleanExit: false, lastCrash: null, crashStreak: 0, safeMode: false });
  updateAvailable.set(null);
  // Mid-service, on air, when the process died.
  setSession({ planId: 7, liveCueId: 42, liveSlide: 3, liveOnAir: true, serviceId: 11 });
  booting.set(true);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

const esc = () =>
  window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

describe('a restart may not put a cue back on air', () => {
  it('persists liveOnAir in the first place (the precondition, not the bug)', () => {
    expect(get(session).liveOnAir).toBe(true);
    expect(JSON.parse(localStorage.getItem('relay.session.v1')).liveOnAir).toBe(true);
  });

  // The control case, asserted STATICALLY rather than by driving the gate.
  //
  // `onMount` does not fire for a component mounted under this vitest setup (the
  // sequencer's whole run is kicked off from `onMount`, so the stage machine never
  // advances and the Recover gate never appears). `svelte:window on:keydown` DOES
  // fire, which is why the Esc case below is a real, driven reproduction and this
  // one is a source-level contract. Saying which is which is the point — see
  // "Every claim must name the instrument that saw it" — from the three
  // Working-Agent documents, which `docs/QA_HARNESS.md` superseded and replaced.
  it('has exactly three exits, and only two of them clear liveOnAir', async () => {
    const src = await readFile(resolve(process.cwd(), 'src/lib/boot/BootSequence.svelte'), 'utf8');

    // The two that do.
    // FIXED 2026-08-14 (R5-3). The repair is not "clear it in the third exit too"
    // — it is that a flag which must die on EVERY exit belongs at the exit. All
    // three routes run through `finish()`, so that is where it is cleared, and a
    // fourth exit added later inherits the guarantee instead of needing to
    // remember it.
    expect(src, 'startFresh() still clears the whole session').toMatch(
      /function startFresh\(\)[\s\S]{0,200}?clearSession\(\)/,
    );

    const finishBody = src.match(/function finish\(\)\s*\{([\s\S]*?)\n  \}/)[1];
    expect(
      /liveOnAir/.test(finishBody),
      'finish() is the single exit every route runs through — Esc, the Recover ' +
        'gate, Start fresh, and the no-gate fall-through. If it does not clear ' +
        'liveOnAir, some route does not. finish() body:\n' + finishBody,
    ).toBe(true);
  });

  it('ALSO clears liveOnAir when the operator skips the sequence with Esc', async () => {
    // The whole finding. Esc is a documented skip, it is the reflex of a volunteer
    // in a hurry, and it is the one exit that never asks the resume question — so
    // the flag survives and Live.svelte lights amber over a dark wall.
    mount();
    await new Promise((r) => setTimeout(r, 30));
    esc();
    await new Promise((r) => setTimeout(r, 20));

    expect(get(booting)).toBe(false); // it really did hand over to the console
    expect(
      get(session).liveOnAir,
      'Esc skipped the Recover gate and left a cue marked ON AIR across a restart. ' +
        'Live.svelte renders class:amber={planOnAir} with the label "On Air" from ' +
        'exactly this value, over output windows that died with the process.',
    ).toBe(false);
  });
});
