// R2 · LIVE PATH AUDIT — the run column, 2026-08-14
//
// Companion to `liveoutputrail.test.js`. That file pins Preview ≠ Programme on
// the component. This one attacks the things AROUND it: who is allowed to take
// the plan off air, whether the shipped app can reach the preview surface at
// all, and which overlays disarm the panic key.
//
// Tests that assert a CORRECT behaviour the code does not have are marked
// `it.fails` (vitest's "this must throw"), so they are green while the defect
// stands and go RED the moment somebody fixes it without updating this file.
// Precedent: the skipped known-defect test at the bottom of
// `liveoutputrail.test.js`.
//
//   npx vitest run src/lib/r2livepath.test.js
//
//   CLAUDE.md (frontend shape · §11 · §15 · §16 · §18) · docs/DECISIONS.md §20
//   the coverage matrix, now `docs/QA_HARNESS.md` Part 4 (it superseded the three
//   Working-Agent documents, which are no longer in the repository)

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const cap = await import('./stores/capture.js');
const { liveCue } = cap;
const { installShortcuts } = await import('./shortcuts.js');

const read = (p) => readFileSync(new URL(p, import.meta.url), 'utf8');

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue({});
  liveCue.set({ cueId: 7, slide: 2, onAir: true });
});

// ─────────────────────────────────────────────────────────────────────────────
// R2-D · `liveCue.onAir` IS CLEARED BY FOUR WRAPPERS AND TAKEN BY NINE PATHS
//
// capture.js documents the rule itself:
//
//   "onAir — is plan content on the screens RIGHT NOW. Cleared the moment
//    anything else takes the screen … This lives in the store, not in a view,
//    because EVERY path that takes plan content off the screen has to clear
//    `onAir`, and a view will eventually forget."
//
// `leavePlan()` has three callers: manualFire, confirmDetection, panicRun. Five
// other wrappers put something on the congregation's screen and none of them
// clears it, so the plan rail in Live.svelte keeps drawing
//
//   <span class="r-badge" class:amber={planOnAir}>On Air</span>
//
// on a cue the congregation stopped looking at. Amber means live and is never
// allowed to lie (CLAUDE.md §18). The topbar, reading `$live`, simultaneously
// names the announcement — two indicators in one window, disagreeing.
//
// `transport.test.js` pins exactly the four that behave and enumerates none of
// the twins, which is how this survived: eight-of-nine wrappers, again.
// ─────────────────────────────────────────────────────────────────────────────
describe('R2-D · who takes the plan off air', () => {
  const onAir = () => get(liveCue).onAir;

  describe('the four that do — the control group', () => {
    it('manualFire', async () => {
      await cap.manualFire('John 3:16');
      expect(onAir()).toBe(false);
    });
    it('confirmDetection', async () => {
      await cap.confirmDetection('John 3:16');
      expect(onAir()).toBe(false);
    });
    it('clearScreens', async () => {
      await cap.clearScreens();
      expect(onAir()).toBe(false);
    });
    it('blackScreen', async () => {
      await cap.blackScreen();
      expect(onAir()).toBe(false);
    });
  });

  // FIXED 2026-08-14 (P1-6). These five took the wall and left the plan rail
  // amber. The repair is the one this block already prescribed: `manualFire`'s
  // `keepPlan` flag, extended to the three that are ALSO the plan's own take path
  // (Live.svelte::fireSlide passes `true`), rather than an unconditional
  // leavePlan() that would have broken Slide mode.
  describe('the five that did not — each one takes the wall', () => {
    it('fireContent — a song from LyricsPane, a notice from Announcements', async () => {
      await cap.fireContent('Notice', 'Wednesday at 7pm', 'announce');
      expect(onAir()).toBe(false);
    });
    it('fireMedia — a picture from MediaLibrary, or a queued media item', async () => {
      await cap.fireMedia(3);
      expect(onAir()).toBe(false);
    });
    it('startCountdown — the console tile and the run rail tile', async () => {
      await cap.startCountdown(5);
      expect(onAir()).toBe(false);
    });
    it('pushAnnouncement — the EMERGENCY announcement, over every screen', async () => {
      await cap.pushAnnouncement('Fire alarm — leave by the side door');
      expect(onAir()).toBe(false);
    });
    it('navVerse — the transport step the backend performs', async () => {
      invoke.mockResolvedValue({ kind: 'fired', reference: 'John 3:17' });
      await cap.navVerse('next');
      expect(onAir()).toBe(false);
    });
  });

  // The sharpest instance, and the one no wrapper can fix: a clear that did not
  // originate in this console. The LAN remote's /api/clear and the spoken
  // "clear the screen" both reach `channels::clear` directly, and the console
  // learns about it through the `output://clear` event — whose listener sets
  // `live` and `screenBlack` and nothing else.
  it('the output:// listeners report a remote/spoken clear, and they DO leavePlan', () => {
    const src = read('./stores/capture.js');
    const listeners = src.slice(src.indexOf("listen('output://content'"), src.indexOf("listen('nav://blocked'"));
    expect(listeners).toContain("listen('output://clear'");
    expect(listeners).toContain("listen('output://black'");
    // FIXED 2026-08-14 (P1-6). This is the half no wrapper can reach: /api/clear
    // from the preacher's phone, the spoken "clear the screen", and the exit from
    // a rehearsal all reach `channels::clear` directly, so this event is the
    // console's only report of them. It set `live` and `screenBlack` and nothing
    // else, and the plan rail went on drawing amber over a wall nobody was
    // looking at.
    expect(listeners).toContain('leavePlan');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R2-E · THE PREVIEW SURFACE IN THE RUN COLUMN HAS NO PRODUCER
//
// `liveoutputrail.test.js` mounts `LiveOutputRail` with a `preview` prop and
// pins seventeen laws about it: the badge describes the pane, the `.lo-behind`
// chip reports a hot wall, TAKE hands the slide to the parent, TAKE is dead in
// safe mode. All seventeen hold — on a prop.
//
// `Library.svelte` is the only thing that renders the component. It passes
// `preview={staged}`; `staged` is declared `null`, assigned `null` on a
// successful take, and set to a real object by exactly one function — `stage(d)`
// — which NOTHING CALLS. `LyricsPane` and `Browse` are handed `onSelect={select}`
// and `select` is `() => {}`.
//
// So in the shipping app `preview` is permanently null: the Take button is
// permanently disabled, `goLive()`'s `if (preview) return onTake()` branch is
// dead, the new badge can never read "Preview", and the `.lo-behind` chip can
// never appear. This is the same failure as `PreviewProgram.svelte` being
// imported by nothing — one level further in, and now with a passing test suite
// standing behind it.
//
// (Live.svelte has a REAL preview/program pair, derived from the plan and the
// detections. That one ships. This one is the Library's, and it is unreachable.)
// ─────────────────────────────────────────────────────────────────────────────
describe('R2-E · the Library run column has no preview half at all', () => {
  const lib = read('./views/Library.svelte');
  const rail = read('./views/library/LiveOutputRail.svelte');

  // CLOSED 2026-08-15 (audit P1-2) by REMOVAL, not by wiring.
  //
  // `stage()` had zero callers, so `preview` was permanently null: the Take button
  // permanently disabled, `goLive()`'s `if (preview)` branch dead, and seventeen
  // tests standing over a prop the shipping app could not supply. It was built for
  // AI suggestions only — `stage(d)` took a detection whose `_fire` was
  // `confirmDetection` — and the Heard panel fires on one press instead.
  //
  // The decision was to keep one press and delete the half: `Live.svelte` already
  // implements Preview ≠ Programme for the plan path, and the QUEUE is the staging
  // area that exists. Two implementations of one safety distinction is the shape
  // that produced most of this audit.
  it('nothing stages, because there is nothing to stage into', () => {
    expect(lib).not.toMatch(/function stage\(/);
    expect(lib).not.toMatch(/preview=\{staged\}/);
    expect(lib).not.toMatch(/const select = \(\) => \{\};/);
  });

  it('the rail declares no preview prop and offers no Take button', () => {
    expect(rail).not.toMatch(/export let preview/);
    expect(rail).not.toMatch(/export let onTake/);
    expect(rail).not.toMatch(/class="lo-take"/);
  });

  it('and Go Live fires the queue, which is reachable', () => {
    expect(rail).toMatch(/const \{ item, rest \} = take\(queue\)/);
    expect(rail).toMatch(/disabled=\{\$safeMode \|\| !queue\.length\}/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R2-F · role="alertdialog" IS NOT role="dialog", AND ESC CLEARS THE WALL
//
// CLAUDE.md §16: Esc must not clear the screens while a dialog is open.
// shortcuts.js reads the DOM for `[role="dialog"]` — deliberately, so nobody has
// to remember to register the next overlay. Three overlays are `alertdialog`,
// which is the ARIA role for exactly the modals that most need the guard:
//
//   lib/crash.js                      — the console-crash panel
//   lib/boot/CrashReportRecovery.svelte — the crash gate on the next launch
//
// The crash panel's own copy reads "Your output screens are still live … it will
// not blank the screens." It is the one moment in the product where the wall is
// GUARANTEED to still be hot and the console is GUARANTEED to be in a state the
// operator wants out of — and Esc, the reflex for closing a modal, blanks the
// congregation's screens while leaving the panel exactly where it was.
//
// (The popup-menu half of this — role="menu" and no role at all — is already
// filed by R3 in surface.test.js; this is the alertdialog twin.)
// ─────────────────────────────────────────────────────────────────────────────
describe('R2-F · Escape over an alertdialog', () => {
  let clearScreens;
  let teardown;
  let el;

  beforeEach(() => {
    clearScreens = vi.fn();
    teardown = installShortcuts({ clearScreens, blackScreen: vi.fn() });
  });
  afterEach(() => {
    teardown?.();
    el?.remove();
    el = null;
  });

  const press = (target) =>
    target.dispatchEvent(
      new window.KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true }),
    );

  it('CONTROL — role="dialog" disarms Esc, as designed', () => {
    el = document.createElement('div');
    el.setAttribute('role', 'dialog');
    document.body.appendChild(el);
    press(el);
    expect(clearScreens).not.toHaveBeenCalled();
  });

  it('role="alertdialog" disarms it too', () => {
    el = document.createElement('div');
    el.setAttribute('role', 'alertdialog');
    el.setAttribute('aria-modal', 'true');
    document.body.appendChild(el);
    press(el);
    expect(clearScreens).not.toHaveBeenCalled();
  });

  it('the crash panel really is an alertdialog, and really is modal', () => {
    const src = read('./crash.js');
    expect(src).toContain(`setAttribute('role', 'alertdialog')`);
    expect(src).toContain(`setAttribute('aria-modal', 'true')`);
    // …and it promises the outputs are untouched, in the largest words on it.
    expect(src).toMatch(/Your output screens are still live/);
  });

  it('and so is the boot gate that shows on the next launch', () => {
    expect(read('./boot/CrashReportRecovery.svelte')).toContain('role="alertdialog"');
  });

  it('shortcuts.js now probes for BOTH roles', () => {
    // FIXED 2026-08-14 (P1-3). `alertdialog` is the ARIA role for exactly the
    // modals that matter most, and the crash panel — the one overlay that
    // guarantees in its own copy that the wall is still hot — was the overlay it
    // could not see.
    const src = read('./shortcuts.js');
    expect(src).toContain('[role="dialog"]');
    expect(src).toContain('[role="alertdialog"]');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R2-G · onMount AND afterUpdate ARE NO-OPS IN EVERY COMPONENT TEST IN THIS REPO
//
// `svelte`'s package exports map the `browser` condition to the DOM runtime and
// everything else to `ssr.js`, where `onMount`/`onDestroy`/`beforeUpdate`/
// `afterUpdate` are literally `function onMount() {}`. vitest.config.js sets no
// `resolve.conditions`, so under test a component is compiled to DOM and then
// handed the SSR lifecycle stubs.
//
// This is not cosmetic. It caps what layer B can ever prove:
//
//   · `liveoutputrail.test.js` comments that "onMount polls list_output_channels";
//     it does not. `channels` stays `[]`, so the output picker and the
//     `monitorTemplate = watched?.template ?? template` resolution — the thing
//     that decides WHICH screen the run column is showing you — are unexercised.
//   · `Live.svelte` cannot be mounted meaningfully at all: no plan loads, the
//     session playhead is never restored, and `registerContext` never runs, so
//     the transport keys are unregistered in every test that mounts it.
//   · CLAUDE.md rule #1 ("never call tick() in a reactive block — use
//     afterUpdate") names a hook that no test can observe.
//
// A mount test whose setup never runs passes by doing nothing, which is the
// exact failure mode this audit exists to catch.
//
// Repair direction: `resolve: { conditions: ['browser'] }` in vitest.config.js.
// ─────────────────────────────────────────────────────────────────────────────
// R2-G — CLOSED. These three asserted the defect and now assert the repair.
//
// `vitest.config.js` had no `resolve.conditions`, so svelte's `.` export resolved
// to `src/runtime/ssr.js`, where the mount hooks are literal empty functions. R2
// filed it, R3 and R6 found it independently, and the one-line fix landed with
// P1-11. Kept rather than deleted because the deepest test here — "mounting the
// run column reaches the backend" — is the one that fails first if the config
// line is ever removed, and it fails in terms of the *application* rather than of
// the runtime, which is the failure a reader will understand.
describe('R2-G · the component-test apparatus is real', () => {
  it('onMount is the real one', async () => {
    const { onMount } = await import('svelte');
    expect(String(onMount)).not.toBe('function onMount() {}');
  });

  it('afterUpdate is the real one', async () => {
    const { afterUpdate } = await import('svelte');
    expect(String(afterUpdate)).not.toBe('function afterUpdate() {}');
  });

  it('…and it shows: mounting the run column really does reach the backend', async () => {
    const LiveOutputRail = (await import('./views/library/LiveOutputRail.svelte')).default;
    invoke.mockResolvedValue([]);
    const host = document.createElement('div');
    document.body.appendChild(host);
    const app = new LiveOutputRail({ target: host, props: { queue: [] } });
    await new Promise((r) => setTimeout(r, 20));
    // Its onMount awaits `listOutputChannels()`. This used to be zero calls — so
    // `channels` stayed `[]` in every test, and `monitorTemplate`, which decides
    // WHICH screen the run column is showing you, was never exercised once.
    expect(invoke.mock.calls.map((c) => c[0])).toContain('list_output_channels');
    app.$destroy();
    host.remove();
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R2-H · AT THE END OF THE PLAN, `→` GOES SILENT AND TAKE GOES BACKWARDS
//
// Live.svelte states the contract: "PREVIEW = what the next TAKE would put
// there: the AI's top pending claim if there is one, otherwise the slide `→`
// would fire." Two things break at the last slide of the last cue.
//
// 1. `→` SAYS NOTHING. In VERSE mode a step that cannot move returns
//    EndOfPassage and the operator is told (`nav.test.js`, `e2e.rs`). In SLIDE
//    mode the same key runs
//
//        async function stepLive(dir) {
//          const to = stepFrom(items, liveCueId, liveSlide, dir);
//          if (!to) return;              // ← no flash, no notice, nothing
//          await fireSlide(to.item, to.slide);
//        }
//
//    The whole point of NavResult, on the transport's other half.
//
// 2. TAKE FIRES A SLIDE THE SERVICE HAS ALREADY PASSED. `previewCue` falls back
//    to the SELECTED cue at slide 0 when `stepFrom` returns null — and
//    `fireSlide` sets `selId = item.id` after every take, so the selected cue is
//    the live one. At the end of the plan the Preview monitor therefore shows
//    slide 1 of the cue that is currently on the wall, labels it as what TAKE
//    will do, and TAKE puts it back up: the first verse of the closing song,
//    after the closing song has finished.
//
// The two controls the rack presents as the same action disagree, and the
// louder one is wrong.
// ─────────────────────────────────────────────────────────────────────────────
const { stepFrom } = await import('./plan.js');

describe('R2-H · the end of the plan', () => {
  const live = read('./views/Live.svelte');

  const song = (id, label, n) => ({
    id,
    label,
    cue_type: 'song',
    payload_json: JSON.stringify({
      title: label,
      sections: Array.from({ length: n }, (_, i) => ({
        tag: `V${i + 1}`,
        label: `Verse ${i + 1}`,
        lyrics: `line ${i + 1}`,
      })),
    }),
  });
  const items = [song(1, 'Opening', 2), song(2, 'Closing', 3)];

  it('stepFrom returns null off the end — ends of the plan are hard stops', () => {
    expect(stepFrom(items, 2, 2, 1)).toBe(null);
  });

  it('and stepLive swallows that null without a word', () => {
    const body = live.slice(live.indexOf('async function stepLive'));
    expect(body.slice(0, 220)).toContain('if (!to) return;');
    // Its VERSE-mode twin, three lines above, always speaks.
    const step = live.slice(live.indexOf('async function step(dir)'), live.indexOf('async function stepLive'));
    expect(step).toContain('flash(notice)');
  });

  it('the preview falls back to the SELECTED cue at slide 0', () => {
    expect(live).toContain('$: previewCue = previewNext ?? (selCue ? { item: selCue, slide: 0 } : null);');
  });

  it('…and every take makes the LIVE cue the selected one', () => {
    const fire = live.slice(live.indexOf('async function fireSlide'), live.indexOf('async function clearAll'));
    expect(fire).toContain('setLive(item.id, i);');
    expect(fire).toContain('selId = item.id;');
  });

  it('so TAKE at the end of the plan re-fires slide 1 of the cue already on the wall', () => {
    // Compose the four facts above: live = cue 2, slide 2 (its last); selId = 2.
    const previewNext = stepFrom(items, 2, 2, 1);
    const selCue = items.find((i) => i.id === 2);
    const previewCue = previewNext ?? (selCue ? { item: selCue, slide: 0 } : null);
    expect(previewCue).toEqual({ item: selCue, slide: 0 });
    // TAKE runs `fireSlide(previewCue.item, previewCue.slide)` — Closing, verse 1,
    // three slides behind where the service actually is.
    expect(previewCue.slide).toBe(0);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R2-I · THE PREACHER'S STAGE TABLET IGNORES A BLACKOUT
//
// Clear ≠ Blackout, and the two panic controls are supposed to differ only in
// how hard they hit: clear blanks to the template background, blackout paints
// every output opaque and "kills the screen entirely" (channels.rs). Both are
// published to the kiosk hub, so both reach every network output.
//
// `Output.svelte` honours both. `Stage.svelte` — the preacher's phone, and the
// stage/confidence tablet, both served off :8032 and driven off the :8031 hub —
// handles `content`, `clear` and `stage_next`, and has no branch for `black` at
// all. So the operator hits B, every congregation screen goes dark, and the
// verse stays on the preacher's tablet.
//
// It may be that a confidence monitor SHOULD survive a blackout. But then it
// should survive a clear too, and it does not: `clear` sets `visible = false`
// there. As it stands the milder control is honoured and the harsher one is
// dropped, which is not a decision anybody made.
// ─────────────────────────────────────────────────────────────────────────────
describe('R2-I · Clear ≠ Blackout on the stage tablet', () => {
  const stage = read('../Stage.svelte');
  const output = read('../Output.svelte');

  it('the wall output handles both panic messages', () => {
    expect(output).toContain("m.kind === 'clear'");
    expect(output).toContain("m.kind === 'black'");
  });

  it('the stage tablet handles clear', () => {
    expect(stage).toContain("m.kind === 'clear'");
  });

  it('…and blackout, which it did not', () => {
    // FIXED 2026-08-14 (P1-1). The stage tablet honoured the MILDER control and
    // dropped the harsher one, which is not a decision anyone made. `R6-3` in
    // r6-contracts.test.js now holds the general rule — every hub message needs a
    // per-client verdict — so the next kind cannot be forgotten quietly.
    expect(stage).toContain("m.kind === 'black'");
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// R2-J · THE CONSOLE MOUNTS UNDERNEATH THE BOOT GATES
//
// Adjacent to R5's finding that Esc past the boot sequence leaves `liveOnAir`
// set — but NOT the same defect, and the obvious repair for theirs does not
// close this one.
//
// `App.svelte` renders the splash and the launch sequence as overlays; the shell
// itself, and therefore `<svelte:component this={current} />`, is OUTSIDE both
// guards. So `Live.svelte` mounts — and its onMount runs — while the Recover
// Session gate is still on screen asking the question. That onMount does:
//
//     liveCue.set({ cueId: saved.liveCueId, slide: …, onAir: saved.liveOnAir === true })
//
// reading the session as it was BEFORE the operator answered. `resume()` then
// patches `session.liveOnAir = false` and touches `liveCue` not at all — and
// `liveCue` is what the cue rail's amber "On Air" badge is drawn from.
//
// So a fix that clears `liveOnAir` on every boot exit (R5's) still leaves the
// store holding `onAir: true`, because the copy was taken first. The two facts
// need one owner.
//
// Layer C: the render order is proven here. The timing — that Live's four
// awaited round-trips finish before a human answers a four-stage gate — is layer
// E, and is why this is filed SUSPECTED rather than confirmed.
// ─────────────────────────────────────────────────────────────────────────────
describe('R2-J · the shell renders outside the boot guards', () => {
  const appSrc = read('../App.svelte');

  it('the boot sequence is guarded', () => {
    expect(appSrc).toContain('{#if !booting && !launched}');
    expect(appSrc).toContain('<BootSequence');
  });

  it('the shell that hosts the active view is not', () => {
    const shellAt = appSrc.indexOf('<div class="shell"');
    const guardAt = appSrc.lastIndexOf('{#if', shellAt);
    const between = appSrc.slice(guardAt, shellAt);
    // The nearest preceding block is FirstRun's, and it is closed before the shell.
    expect(between).toContain('{/if}');
    expect(appSrc).toContain('<svelte:component this={current} />');
  });

  it('and Live.svelte copies the pre-answer session value into the store', () => {
    const src = read('./views/Live.svelte');
    expect(src).toContain('onAir: saved.liveOnAir === true,');
  });

  it('while the boot exit patches only the session, never the store', () => {
    // R5-3 moved the clearing from `resume()` to `finish()`, the single exit every
    // route runs through — which closes R5-3 and does NOT close this one. R2 said
    // so when it filed the finding: `liveCue` is what the amber badge is drawn
    // from, `Live.svelte` copies `saved.liveOnAir` into it at mount, and the shell
    // mounts OUTSIDE the boot guards — so the copy is taken before the gate is
    // answered. Clearing the session on every exit still leaves the store holding
    // `onAir: true`. Two facts, one owner needed.
    const boot = read('./boot/BootSequence.svelte');
    const finish = boot.slice(boot.indexOf('function finish()'), boot.indexOf('function resume()'));
    expect(finish).toContain('setSession({ liveOnAir: false })');
    expect(finish).not.toContain('liveCue');
  });
});
