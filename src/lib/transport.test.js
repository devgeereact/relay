// The playhead: where → resumes from, and whether the plan is ON AIR.
//
// These are two different facts and every path that takes plan content off the
// screen must clear the second WITHOUT destroying the first. Getting either half
// wrong puts the wrong thing in front of a congregation:
//
//   forget to clear onAir  → the operator accepts an AI-suggested verse, presses
//                            → to read on, and jumps back into the song instead.
//   destroy the position   → the operator presses Esc at cue 9, presses →, and the
//                            plan RESTARTS at cue 1 — the opening countdown, back
//                            on the wall, at the end of the service.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { liveCue, clearScreens, blackScreen, manualFire, confirmDetection } = await import(
  './stores/capture.js'
);

describe('taking plan content off the screen', () => {
  // Cue 7, slide 2, on the congregation's wall.
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(undefined);
    liveCue.set({ cueId: 7, slide: 2, onAir: true });
  });

  it('Esc / clear takes the plan off air but REMEMBERS the position', async () => {
    await clearScreens();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: false });
  });

  it('blackout does the same', async () => {
    await blackScreen();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: false });
  });

  /// A hand-typed verse is not a plan cue. If the transport still thought the plan
  /// was on air, → would jump back into the plan instead of walking the passage
  /// the operator just put up.
  it('firing a verse by hand takes the plan off air', async () => {
    await manualFire('John 3:16');
    expect(get(liveCue).onAir).toBe(false);
    expect(get(liveCue).cueId).toBe(7); // still where we were
  });

  it('accepting an AI suggestion takes the plan off air', async () => {
    await confirmDetection('John 3:16');
    expect(get(liveCue).onAir).toBe(false);
    expect(get(liveCue).cueId).toBe(7);
  });

  // The opposite of a hand-typed verse: STEPPING the plan onto a scripture cue in
  // Slide mode is still the plan. keepPlan must hold the plan ON AIR, or the
  // transport flips to Verse mode and the next → walks the passage instead of
  // advancing the plan — the Slide-mode bug.
  it('firing a PLAN scripture slide (keepPlan) stays ON AIR — Slide mode holds', async () => {
    await manualFire('John 3:16', null, null, true);
    expect(get(liveCue).onAir).toBe(true);
    expect(get(liveCue).cueId).toBe(7);
  });

  // THE TRANSPORT FOLLOWS THE WALL, NOT THE INTENT.
  //
  // These two used to reset the playhead BEFORE calling the backend, so a fire that
  // failed still marked the plan off air — while the plan's slide was still sitting on
  // the congregation's screen. The next → would then walk a verse passage nobody could
  // see, firing content the operator never asked for.
  //
  // Nothing moved on the wall, so nothing moves here.
  it('a FAILED hand-fire leaves the plan exactly as it was — it is still on the wall', async () => {
    invoke.mockRejectedValue({ kind: 'not_found', message: "that isn't in the Bible text" });
    await expect(manualFire('John 3:99')).rejects.toBeTruthy();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: true });
  });

  it('a FAILED accept leaves the plan on air, and keeps the suggestion', async () => {
    invoke.mockRejectedValue({ kind: 'internal', message: 'boom' });
    await expect(confirmDetection('John 3:16')).rejects.toBeTruthy();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: true });
  });

  /// The PANIC controls are the deliberate exception: they reset the cursor BEFORE
  /// the backend call, so a half-working panic key still hands the transport back.
  /// The failure itself is not swallowed — it raises `panicError` (DECISIONS §20).
  it('a panic key holds even when the backend call fails', async () => {
    invoke.mockRejectedValue({ kind: 'internal', message: 'no backend' });
    await clearScreens();
    expect(get(liveCue).onAir).toBe(false);
  });

  it('is idempotent — clearing twice does not lose the position', async () => {
    await clearScreens();
    await clearScreens();
    expect(get(liveCue)).toEqual({ cueId: 7, slide: 2, onAir: false });
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// THE CONTRACT, not the four instances of it.
//
// This file used to pin exactly the four wrappers that behaved and enumerate none
// of their twins. That is how the 2026-08-14 audit found FIVE more paths taking
// the wall while the plan rail stayed amber — `fireContent`, `fireMedia`,
// `startCountdown`, `pushAnnouncement`, `navVerse` — and it is the third time this
// repo has shipped an eight-of-nine: rehearsal gated three of four kiosk
// publishers, the throw-vs-swallow contract held for eight of nine wrappers, and
// `NavResult` was honoured by the console and discarded by the remote.
//
// So the rule is asserted over the WHOLE SET, derived from the source. Add a
// wrapper that puts something in front of a congregation and this test names it
// until it has decided what happens to the plan.
// ─────────────────────────────────────────────────────────────────────────────
describe('every path that takes the wall has an answer for the plan rail', () => {
  // `resolve` from the repo root, matching ipc.test.js. `new URL(…, import.meta.url)`
  // is not a file: URL for every module under vite-node.
  const src = readFileSync(resolve(process.cwd(), 'src/lib/stores/capture.js'), 'utf8');

  /** Commands that change what a congregation is looking at. */
  const SCREEN_COMMANDS = [
    'manual_fire',
    'fire_content',
    'fire_media',
    'start_countdown',
    'push_announcement',
    'nav',
    'clear_screens',
    'blackout',
  ];

  it('every wrapper that fires one either clears the plan or opts out on purpose', () => {
    const heads = [...src.matchAll(/export\s+(?:async\s+)?function\s+([A-Za-z0-9_]+)\s*\(/g)];
    const undecided = [];

    heads.forEach((h, i) => {
      const body = src.slice(h.index, i + 1 < heads.length ? heads[i + 1].index : src.length);
      const fires = SCREEN_COMMANDS.some((c) =>
        new RegExp(`\\bcall\\(\\s*['"]${c}['"]`).test(body),
      );
      if (!fires) return;
      // Three acceptable answers: clear it, offer the caller an opt-out (the plan's
      // own take path passes `keepPlan`), or route through `panicRun`, which clears
      // it for the panic controls.
      const decided =
        /leavePlan\(\)/.test(body) || /keepPlan/.test(body) || /panicRun\(/.test(body);
      if (!decided) undecided.push(h[1]);
    });

    expect(
      undecided,
      'These wrappers put something in front of a congregation and say nothing about ' +
        '`liveCue.onAir`, so the plan rail will keep drawing amber "On Air" over a cue ' +
        'nobody is looking at. Amber means live and is never allowed to lie. Either call ' +
        'leavePlan(), or take a `keepPlan` flag if this is the plan\'s own take path.',
    ).toEqual([]);
  });

  it('and the listeners cover the clears this console did not initiate', () => {
    // /api/clear from the preacher's phone, the spoken "clear the screen", and the
    // exit from a rehearsal all reach `channels::clear` directly. No wrapper runs;
    // this event is the console's only report.
    const block = src.slice(
      src.indexOf("listen('output://content'"),
      src.indexOf("listen('nav://blocked'"),
    );
    expect(block).toMatch(/output:\/\/clear[\s\S]*?leavePlan\(\)/);
    expect(block).toMatch(/output:\/\/black[\s\S]*?leavePlan\(\)/);
  });
});
