# Wave 1 — Settings, Splash and Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every Settings control tell the truth about what it did, correct two false statements in CLAUDE.md, give the splash a design pass, and write down the output-routing map a church actually needs.

**Architecture:** Three independent tracks. Track A repairs ten Settings findings, one of which (safe mode) is a live-safety defect fixed at a choke point rather than at each caller. Track B is documentation: two CLAUDE.md corrections and the routing map, plus one small helper in Outputs. Track C is a visual pass on the existing splash. Nothing here changes the fire path, the router, or any threshold.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`.

**Spec:** `docs/superpowers/specs/2026-09-15-timers-templates-stage-design.md` (Wave 1)

**Depends on:** Wave 0 complete and green.

## Global Constraints

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127).
- Read test counts from the runner's own summary line, never a grep. Values live only in `docs/qa/QA_HARNESS.md` §0.
- Every new test verified to FAIL when its defect is reintroduced. Test the bug, not the fix.
- **Rule 41:** never `confirm()`, `alert()` or `prompt()`. Tauri's webview does not implement them — `confirm()` returns `false` without showing a dialog. Use an in-app arm/confirm or a mounted `[role="dialog"]`.
- **Rule 44:** any overlay must consume `Esc` for itself on the first press and must not paint over `Clear screens`. Pinned by `panicoverlay.test.js`, which enumerates the whole tree.
- **Rule 18 / DESIGN_SYSTEM §1.1:** amber means ON AIR and nothing else. Amethyst means rehearsal. Cyan means a guess. Grey means cued. Settings is never on air.
- **`src/lib/errors.js` is the ONE backend-error humaniser.** Never render a raw Rust `Err` string to a volunteer.
- Commit message bodies are normal English prose.
- Never commit to `main`. Work on `feat/wave7-timers-templates-stage`.

---

## Track A — Settings truth pass

### Task 1: Safe mode enforces its own promise (P1)

**Why this exists:** `src/lib/views/Settings.svelte:914` offers a switch whose row says *"Outputs will not open and detection is disarmed — nothing Relay does can reach a screen."* `setSafeMode` (`src/lib/boot/boot.js:115`) calls `patchRecord({ safeMode })` and nothing else. `$safeMode` is read in `src/App.svelte` at lines 419 and 493 **inside `onMount` only**; there is no reactive statement re-applying it. `src/lib/views/Live.svelte` references `safeMode` zero times, so the run surface's fire path is not gated. `grep -rn "safe_mode" src-tauri/src/` returns nothing.

So the switch flips a label, already-open windows stay open, and the detector stays armed until the next launch.

**The fix must be at a choke point.** CLAUDE.md records four separate bugs whose single root cause is a rule enforced on one surface and skipped on its twin. Adding a `$safeMode` check to each fire site would be the fifth.

**Files:**
- Modify: `src/lib/boot/boot.js:115` — `setSafeMode` stops being the public door
- Modify: `src/lib/stores/capture.js` — add `applySafeMode` and `safeModeError`
- Modify: `src/lib/views/Settings.svelte:914-920` — call the new door, render the error
- Modify: `src/App.svelte:493` — the mount path calls the same door
- Create: `src/lib/safemode.test.js`

**Interfaces:**
- Consumes: `setDetection` (`capture.js:559`), `listOutputChannels` (`:1978`), `closeChannelOutput` (`:2249`).
- Produces:
  - `export async function applySafeMode(on: boolean): Promise<boolean>` in `capture.js` — returns `true` when the promise was kept, `false` otherwise, **and** sets `safeModeError`. Both, for the same reason `panicRun` does both: the caller may be a view that has crashed.
  - `export const safeModeError` — a `writable(null)` holding a humanised reason.

- [ ] **Step 1: Write the failing test**

Create `src/lib/safemode.test.js`:

```js
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

// SAFE MODE IS A PROMISE, NOT A LABEL.
//
// The switch's own row says "detection is disarmed — nothing Relay does can
// reach a screen". Before this test, setSafeMode patched a localStorage record
// and nothing else: App.svelte honoured it inside onMount only, Live.svelte
// never mentioned it, and Rust had no notion of it at all. The switch flipped,
// aria-checked flipped, and a live detector stayed armed over open projector
// windows until the next launch.
//
// A control that reports a success it did not achieve is rule 15's failure in
// a different costume, and this one makes a bigger promise than a panic key.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

describe('safe mode keeps its own promise', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(true);
  });

  it('disarms detection and closes every open screen when turned ON', async () => {
    const { applySafeMode } = await import('./stores/capture.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') {
        return Promise.resolve([
          { id: 1, name: 'Main screen', render_target: 'native_window' },
          { id: 2, name: 'Lobby', render_target: 'native_window' },
        ]);
      }
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(true);
    const cmds = invoke.mock.calls.map((c) => c[0]);
    expect(cmds).toContain('set_detection');
    expect(
      invoke.mock.calls.find((c) => c[0] === 'set_detection')[1],
    ).toMatchObject({ enabled: false });
    expect(cmds.filter((c) => c === 'close_channel_output')).toHaveLength(2);
  });

  it('reports failure and does not claim a success it did not achieve', async () => {
    const { applySafeMode, safeModeError } = await import('./stores/capture.js');
    safeModeError.set(null);
    invoke.mockImplementation((cmd) => {
      if (cmd === 'set_detection') return Promise.reject(new Error('audio lock poisoned'));
      if (cmd === 'list_output_channels') return Promise.resolve([]);
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(false);
    expect(get(safeModeError)).toBeTruthy();
  });

  it('turning safe mode OFF does not re-arm anything by itself', async () => {
    // Coming out of safe mode restores the operator's freedom to arm things; it
    // must not arm them FOR them. A detector that switches itself back on is a
    // different surprise from the one this control exists to prevent.
    const { applySafeMode } = await import('./stores/capture.js');
    invoke.mockImplementation((cmd) =>
      cmd === 'list_output_channels' ? Promise.resolve([]) : Promise.resolve(true),
    );

    await applySafeMode(false);

    const armed = invoke.mock.calls.filter(
      (c) => c[0] === 'set_detection' && c[1]?.enabled === true,
    );
    expect(armed).toHaveLength(0);
  });

  it('setSafeMode has exactly one caller, and it is applySafeMode', async () => {
    // THE CHOKE POINT IS WHERE THE CHECK GOES, NOT THE CALL SITES (rule 36).
    // The record write must not be reachable without the enforcement beside it,
    // or the next surface to flip safe mode reproduces the original defect.
    const { readFileSync, readdirSync, statSync } = await import('node:fs');
    const { join } = await import('node:path');
    const root = new URL('..', import.meta.url).pathname;

    const files = [];
    const walk = (d) => {
      for (const e of readdirSync(d)) {
        const p = join(d, e);
        if (statSync(p).isDirectory()) walk(p);
        else if (/\.(js|svelte)$/.test(e) && !e.endsWith('.test.js')) files.push(p);
      }
    };
    walk(root);

    const callers = files.filter((f) => {
      if (f.endsWith('boot/boot.js')) return false; // the definition
      return /\bsetSafeMode\s*\(/.test(readFileSync(f, 'utf8'));
    });

    expect(
      callers.map((f) => f.slice(root.length)),
      'setSafeMode must be reached only through applySafeMode',
    ).toEqual(['stores/capture.js']);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/safemode.test.js
```

Expected: FAIL — `applySafeMode` is not exported.

- [ ] **Step 3: Add the choke point**

In `src/lib/stores/capture.js`, near `panicError` (`:2322`), add:

```js
/**
 * The reason safe mode could not keep its promise, humanised — or null.
 *
 * Module scope, and set by `applySafeMode` itself rather than returned to the
 * caller, for the same reason `panicError` is: the control that flips safe mode
 * can be a view that has crashed, and a view that cannot `catch` cannot report.
 */
export const safeModeError = writable(null);

/**
 * TURN SAFE MODE ON OR OFF, AND MAKE THE PROMISE TRUE.
 *
 * Safe mode's row says "outputs will not open and detection is disarmed —
 * nothing Relay does can reach a screen". `setSafeMode` writes that into the
 * boot record; for as long as it was the only thing that happened, the sentence
 * was false until the next launch — App.svelte honoured it inside onMount only,
 * Live.svelte never mentioned it, and Rust has no notion of it.
 *
 * So the enforcement lives HERE, at the one door, and not as a `$safeMode` check
 * at each fire site. This repository has had four separate bugs whose single
 * root cause is a rule enforced on one surface and skipped on its twin; a check
 * per caller would be the fifth.
 *
 * Returns whether the promise was kept, AND sets `safeModeError`. Turning safe
 * mode OFF restores the operator's freedom to arm things and deliberately arms
 * nothing for them: a detector that switches itself back on is a different
 * surprise from the one this control prevents.
 */
export async function applySafeMode(on) {
  safeModeError.set(null);
  setSafeMode(on);
  if (!on) return true;

  const failures = [];

  try {
    await setDetection(false);
  } catch (e) {
    failures.push(humanError(e));
  }

  try {
    const chans = await listOutputChannels();
    for (const c of chans ?? []) {
      try {
        await closeChannelOutput(c.id);
      } catch (e) {
        failures.push(`${c.name ?? `screen ${c.id}`}: ${humanError(e)}`);
      }
    }
  } catch (e) {
    failures.push(humanError(e));
  }

  if (failures.length) {
    safeModeError.set(
      `Safe mode is recorded, and it could not be enforced: ${failures.join('; ')}. ` +
        'Something may still be able to reach a screen.',
    );
    return false;
  }
  return true;
}
```

Add `import { setSafeMode } from '../boot/boot.js';` to `capture.js`'s imports if it is not already present, and confirm `humanError` is already imported from `../errors.js`.

- [ ] **Step 4: Point the two callers at the new door**

In `src/lib/views/Settings.svelte`, change the switch handler at `:919` from `setSafeMode(!$safeMode)` to `applySafeMode(!$safeMode)`, update the import, and render `$safeModeError` beneath the row with `role="alert"`:

```svelte
{#if $safeModeError}<p class="rw-foot s-netbad" role="alert">{$safeModeError}</p>{/if}
```

Use `.s-netbad` (rose), never amber — the file's own comments say "Rose, never amber" three times and this page is never on air.

In `src/App.svelte:493`, replace the inline `setDetection(false)` block with a call to the same door, so the mount path and the switch cannot drift:

```js
    // SAFE MODE IS A PROMISE, NOT A LABEL. One door, so a console reopened in
    // safe mode and a switch flipped in Settings enforce exactly the same thing.
    if ($safeMode) {
      await applySafeMode(true);
    }
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/safemode.test.js
```

Expected: PASS, all four cases.

- [ ] **Step 6: Verify the tests catch the bug**

Temporarily change `applySafeMode` to `setSafeMode(on); return true;` and re-run.

Expected: the first two cases FAIL. Restore.

Then temporarily add a second `setSafeMode(` call in `src/lib/views/Settings.svelte` and re-run.

Expected: the fourth case FAILS naming the extra caller. Restore.

- [ ] **Step 7: Record the decision**

Append a section to `docs/DECISIONS.md` recording that safe mode's enforcement lives at one door in `capture.js`, that it reports failure like a panic control, and the open question the spec named: whether safe mode should also exist in Rust so the engine cannot be armed while the record says disarmed. Answer it in this commit and write the reason down. `crossrefs.test.js` resolves every `DECISIONS §N` against the real headings, so use the next free number.

- [ ] **Step 8: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/stores/capture.js src/lib/views/Settings.svelte src/App.svelte src/lib/safemode.test.js docs/DECISIONS.md
git commit -F - <<'EOF'
fix: safe mode enforces its own promise instead of flipping a label

The switch's own row says "outputs will not open and detection is disarmed —
nothing Relay does can reach a screen". setSafeMode patched a localStorage boot
record and nothing else. App.svelte honoured it inside onMount only, with no
reactive statement re-applying it; Live.svelte never mentioned it, so the run
surface's fire path was not gated at all; and there is no notion of safe mode
anywhere in Rust. Already-open projector windows stayed open and the detector
stayed armed until the next launch, while aria-checked said otherwise.

The enforcement now lives at one door, applySafeMode, which writes the record,
disarms detection, closes every open screen, and reports failure rather than
claiming a success it did not achieve — the same contract as a panic control,
because it makes a larger promise than one. A check per fire site was the
obvious alternative and is the shape of the four bugs this repository already
records, where a rule was kept on one surface and skipped on its twin.

Turning safe mode off deliberately arms nothing: it restores the operator's
freedom to arm things, and a detector that switched itself back on would be a
different surprise from the one this control prevents.

A test holds setSafeMode to exactly one caller, so the next surface to offer
safe mode cannot reproduce the original defect.
EOF
```

---

### Task 2: The setup walk-through is guarded during a recorded service (P2)

**Why this exists:** `src/lib/views/Settings.svelte:1438` carries no `disabled` expression in any state. One click sets `session.setupDone = false`, which mounts `FirstRun` full-screen over a live console (`App.svelte:564`). From inside it, `stopMicTest()` and `chooseDevice()` each call `stopCapture()` on the live microphone (`FirstRun.svelte:177`, `:201`), and "Try it" fires `manualFire('John 3:16')` to the congregation's screens (`:222`). The service lock cannot help, because `restartSetup` is a session write and `servicelock::guard` is never consulted. CLAUDE.md rule 44 already names this sentence; its `Esc` half is fixed and pinned, and this half is open.

**Files:**
- Modify: `src/lib/views/Settings.svelte:1438`
- Modify: `src/lib/settingssections.test.js`

**Interfaces:**
- Consumes: the `serviceLock` store already read at `Settings.svelte:1502` for "Unlock for this service".
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write the failing test**

Add to `src/lib/settingssections.test.js`:

```js
  it('the setup walk-through cannot be started over a recorded service', async () => {
    // It mounts FirstRun full-screen over a live console, and from inside it
    // stopMicTest() and chooseDevice() each stop the LIVE microphone while
    // "Try it" fires John 3:16 to the congregation. The service lock cannot
    // reach it, because restartSetup is a session write and never consults
    // servicelock::guard — so the guard has to be on the button.
    const { container } = render(Settings);
    serviceLock.set({ engaged: true, reason: 'a service is being recorded' });
    await tick();

    const btn = [...container.querySelectorAll('button')].find((b) =>
      /setup walk-through/i.test(b.textContent ?? ''),
    );
    expect(btn, 'the walk-through button is rendered').toBeTruthy();
    expect(btn.disabled).toBe(true);
    expect(
      btn.closest('.s-prose')?.textContent ?? '',
      'a disabled control must carry its reason',
    ).toMatch(/service is being recorded|while a service/i);
  });
```

Match the existing file's import style and render helper rather than inventing one.

- [ ] **Step 2: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/settingssections.test.js -t "walk-through"
```

Expected: FAIL — `expected false to be true`.

- [ ] **Step 3: Guard the button**

In `src/lib/views/Settings.svelte`, replace the button at `:1438` with:

```svelte
<button
  class="r-btn ghost sm"
  on:click={restartSetup}
  disabled={$serviceLock.engaged}>Run the setup walk-through</button>
{#if $serviceLock.engaged}
  <p class="rw-foot s-netwarn">Not while a service is being recorded — the walk-through stops the microphone and puts a verse on your screens. End the service first, or unlock it below.</p>
{/if}
```

Use `.s-netwarn` (amethyst), not amber.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/settingssections.test.js
```

Expected: PASS.

- [ ] **Step 5: Verify the test catches the bug**

Remove the `disabled` attribute and re-run. Expected: FAIL. Restore.

- [ ] **Step 6: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/views/Settings.svelte src/lib/settingssections.test.js
git commit -F - <<'EOF'
fix: the setup walk-through is held back while a service is recorded

One click set session.setupDone = false, which mounts the first-run wizard
full-screen over a live console. From inside it, two paths stop the live
microphone and "Try it" fires John 3:16 to the congregation's screens, under a
label that says so. There was no arm, no confirm and no disabled state in any
condition.

The service lock could not help: restartSetup is a session write, so
servicelock::guard is never consulted, which means the guard has to be on the
button. It now carries the lock's own wording, and the unlock control is six
rows below it on the same page.

CLAUDE.md rule 44 already names this exact sentence. Its Escape half was fixed
and pinned; this is the other half.
EOF
```

---

### Task 3: "Check for Updates" stops reporting a check it never ran (P2)

**Why this exists:** `Settings.svelte:1580`. While a service is recording, `checkForUpdate()` (`src/lib/updater.js:92`) returns `null` **without calling `noteChannel`**, so `doCheckUpdates` falls through to `ch.state === 'ok'` and prints "You're on the latest version." The status row six pixels above goes through `describeChannel` and is honest. The two disagree and the louder one is wrong. This is rule 35 on the one path by which a fix reaches a church that already has Relay — the same category as RG-83 and RG-133.

**Files:**
- Modify: `src/lib/updater.js:92` — the idle refusal becomes a recorded outcome
- Modify: `src/lib/views/Settings.svelte:1580-1583`
- Modify: `src/lib/updatechannel.test.js`

**Interfaces:**
- Consumes: `noteChannel`, `describeChannel` (`updater.js`).
- Produces: a `'skipped'` state on `updateChannel`, described by `describeChannel`.

- [ ] **Step 1: Write the failing test**

Add to `src/lib/updatechannel.test.js`:

```js
  it('a check refused because a service is recording is its own outcome', () => {
    // RULE 35. The button printed "You're on the latest version." whenever the
    // refusal happened after a successful launch check, because the refusal
    // returned null without recording anything and the caller fell back to the
    // stale 'ok'. One reassuring sentence over two different situations, on the
    // one path by which a fix reaches a church that already has Relay.
    noteChannel({ state: 'ok', at: Date.now() });
    noteChannel({ state: 'skipped', reason: 'service', at: Date.now() });

    const said = describeChannel(get(updateChannel));
    expect(said.tone).not.toBe('ok');
    expect(said.text).toMatch(/service/i);
    expect(said.text).not.toMatch(/latest version/i);
  });
```

Match the file's existing import and helper style.

- [ ] **Step 2: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/updatechannel.test.js -t "refused"
```

Expected: FAIL — `describeChannel` has no `skipped` branch.

- [ ] **Step 3: Record the refusal and describe it**

In `src/lib/updater.js`, at the idle guard around `:92`, replace the bare `return null` with:

```js
  if (!idle()) {
    // A REFUSAL IS A THIRD OUTCOME, NOT AN ABSENCE. Returning null here let the
    // caller fall back to whatever the last successful check said, so a button
    // pressed mid-service printed "You're on the latest version." about a check
    // that never ran. Rule 35: if the line reads the same when the thing behind
    // it did not happen, it is not a status line.
    noteChannel({ state: 'skipped', reason: 'service', at: Date.now() });
    return null;
  }
```

Add the `skipped` branch to `describeChannel`:

```js
    case 'skipped':
      return {
        tone: 'warn',
        text: 'Not checked — Relay does not check for updates while a service is being recorded.',
      };
```

In `src/lib/views/Settings.svelte`, make `doCheckUpdates` render `describeChannel($updateChannel).text` rather than composing its own sentence from `ch.state === 'ok'`.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/updatechannel.test.js
```

Expected: PASS.

- [ ] **Step 5: Verify the test catches the bug**

Remove the `noteChannel({ state: 'skipped' … })` line and re-run. Expected: FAIL. Restore.

- [ ] **Step 6: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/updater.js src/lib/views/Settings.svelte src/lib/updatechannel.test.js
git commit -F - <<'EOF'
fix: a refused update check is its own outcome, not a stale success

While a service is being recorded, checkForUpdate returns null without
recording anything, so the button fell through to whatever the last successful
check had said and printed "You're on the latest version." about a check that
never ran. The status row six pixels above it goes through describeChannel and
was honest the whole time, so the page disagreed with itself and the louder
half was wrong.

The refusal is now a third state, described in the one place that turns channel
state into words. This is the same failure RG-83 and RG-133 were about, on the
same path: how a fix reaches a church that already has Relay.
EOF
```

---

### Task 4: The five smaller Settings findings

Each is independent. Commit them together — a reviewer would accept or reject them as one pass.

**Files:**
- Modify: `src/lib/views/Settings.svelte` (`:1077`, `:1455`, `:1583`, `:1853`, `:1890`, `:186`)
- Modify: `src/lib/audioOutput.js:81`
- Modify: `src/lib/ModelSetup.svelte:54`

**Interfaces:**
- Consumes: `humanError` (`src/lib/errors.js`), `$contentTemplates` (`capture.js:274`).
- Produces: nothing later tasks depend on.

- [ ] **Step 1: F-3 — "Detect speakers" says what happened**

`audioOutput.js:81` `ensureDeviceAccess` returns `false` both when the permission was refused and when there was nothing to find, so `Settings.svelte:1077` re-renders identically in three different situations. Return which it was, and render it. Include the OS path to reverse a refusal.

- [ ] **Step 2: F-5 — the offline model install uses the one humaniser**

`ModelSetup.svelte:54` is `installMsg = e?.message ?? String(e);`, rendering a Rust `Err(String)` verbatim while its six siblings on the same surface go through `humanError`. Route it through `humanError`. The defect is latent — today's strings are volunteer-worded — and nothing constrains the next one, because `install_from_file` returns `Result<String, String>` rather than the typed `{ kind, message }`.

- [ ] **Step 3: F-8 — the Sentry DSN commits**

`setCrashReporting` is called from exactly one place, `toggleCrash` (`Settings.svelte:469`). With the switch already on, editing the DSN at `:1853` writes only the local object, and leaving the section re-reads `getCrashReporting` and overwrites it. The one control that decides where data leaves the machine can be edited and silently keep pointing at the old place. Commit on `change`/blur, or add a Save beside the field.

- [ ] **Step 4: F-9 — amber is not spent on a page that is never on air**

`Settings.svelte:1455` sets `style="color:var(--v-amber)"` inline on the demo-content edited count. The file's own comments say "Rose, never amber" at `:2032`, `:2040` and `:2056`, and define `.s-netbad` (rose) and `.s-netwarn` (amethyst) for exactly this. Use `.s-netwarn`. Leave `:1497` ("A service is being recorded.") alone — that one is arguably the on-air session.

- [ ] **Step 5: F-10 — two results are announced**

`updateMsg` (`:1583`) and `crashMsg` (`:1890`) have no live region, while six other message surfaces on the same page do — `micErr :1042`, `roomMsg :1110`, `profileErr :1171`, `demoErr/demoNote :1487`, `lockErr :1503`, `diagMsg :1642`. Add `role="status"` to both. Crash reporting is the one control that decides whether data leaves the machine.

- [ ] **Step 6: The private `ctMap`**

`Settings.svelte:186` holds a private content-look map that omits `countdown`, contradicting the "one store" comment at `capture.js:274`. Read `$contentTemplates` instead, like the other three surfaces do.

- [ ] **Step 7: Write the tests**

One case per finding, in `src/lib/settingssections.test.js` (or the file that already covers the surface). Each must be verified to fail with its defect reintroduced. The `role="status"` pair and the amber one are assertions about the rendered markup; the DSN one drives an edit and a re-read; the `ctMap` one asserts that Settings and the Templates gallery agree about all five kinds.

- [ ] **Step 8: Run the suite and commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run 2>&1 | tail -10
git add -A
git commit -F - <<'EOF'
fix: five Settings controls report what actually happened

Detect speakers was pixel-identical when the permission was refused, when it
worked, and when the machine genuinely has one output; audioOutput.js already
knew which and did not say. The offline model install rendered a Rust error
string verbatim, bypassing the one humaniser its six siblings on the same
surface use. The Sentry DSN had no commit path while
crash reporting was already on, so the one control that decides where data
leaves the machine could be edited and keep pointing at the old place. A demo
count was painted amber by an inline style on a page that is never on air, in a
file whose own comments say "Rose, never amber" three times. Two results — the
update check and the crash-reporting toggle — were announced to nobody while
six other message surfaces on the same page have live regions.

Settings also held a private content-look map that omitted the countdown kind,
contradicting the one-store comment it was meant to follow. It reads the store.
EOF
```

---

### Task 5: "End current service" reports a failure (P3)

**Why this exists:** `capture.js:605` `endService` swallows every failure in a bare `catch {}`, and `History.svelte:580` then calls `refresh()` and repaints the same list, so a refused `end_service` leaves the operator believing the record is closed. Same shape as rule 15 on a smaller control. This is its own task because it is not a cosmetic fix: it decides which of `capture.js`'s two documented wrapper groups this wrapper belongs to, and CLAUDE.md records that **a contract stated in a comment is not a contract** — `stopCapture` sat in the THROWS group, swallowing, for as long as its comment existed.

**Files:**
- Modify: `src/lib/stores/capture.js:605`
- Modify: `src/lib/views/library/History.svelte:245`, `:580`
- Create: `src/lib/endservice.test.js`

**Interfaces:**
- Consumes: `humanError` (`src/lib/errors.js`).
- Produces: `endService` in whichever group you place it, **and the test that holds it there**.

- [ ] **Step 1: Read the groups before deciding**

Read the throw-vs-swallow group comment at the top of `src/lib/stores/capture.js`, and read `src/lib/micstop.test.js`, which is the pattern to copy — it exists because `stopCapture` printed "Start listening" over a live microphone while sitting in the group that says it throws.

- [ ] **Step 2: Write the failing test**

Create `src/lib/endservice.test.js`:

```js
import { describe, it, expect, vi, beforeEach } from 'vitest';

// A CONTRACT STATED IN A COMMENT IS NOT A CONTRACT.
//
// endService swallowed every failure in a bare catch {}, and History then
// repainted the same list, so a refused end_service left the operator believing
// the record was closed. The service lock is re-read afterwards, so the truth
// was on the page — in a different section, in smaller type.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

describe('ending a service cannot report a success it did not achieve', () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it('propagates a refused end_service to its caller', async () => {
    const { endService } = await import('./stores/capture.js');
    invoke.mockRejectedValue(new Error('the service lock is held'));

    await expect(endService()).rejects.toThrow();
  });

  it('resolves normally when the command succeeds', async () => {
    const { endService } = await import('./stores/capture.js');
    invoke.mockResolvedValue(true);

    await expect(endService()).resolves.not.toThrow();
  });
});
```

- [ ] **Step 3: Run it to verify it fails**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/endservice.test.js
```

Expected: FAIL on the first case — the bare `catch {}` resolves rather than rejecting.

- [ ] **Step 4: Move the wrapper and show the reason**

Remove the bare `catch {}` from `endService` so it throws, and in `src/lib/views/library/History.svelte`, catch it in `stopRecording` (`:245`) and render `humanError(e)` beside the button with `role="alert"`. Do not re-`refresh()` over a failure — repainting an unchanged list is how the original defect read as success.

- [ ] **Step 5: Run the test to verify it passes**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/endservice.test.js src/lib/transport.test.js
```

Expected: PASS. `transport.test.js` is in the list because it holds the wrapper-group enumeration Wave 0 Task 3 repaired.

- [ ] **Step 6: Verify the test catches the bug**

Restore the bare `catch {}` and re-run. Expected: the first case FAILS. Remove it again.

- [ ] **Step 7: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/stores/capture.js src/lib/views/library/History.svelte src/lib/endservice.test.js
git commit -F - <<'EOF'
fix: ending a service reports a refusal instead of repainting the list

endService swallowed every failure in a bare catch {}, and History then called
refresh() and repainted an unchanged list, so a refused end_service left the
operator believing the record was closed. The truth was on the page the whole
time — the service lock is re-read afterwards — in a different section.

The wrapper now throws, its caller shows the humanised reason beside the button,
and a test holds it in that group. capture.js documents its throw-vs-swallow
groups in a comment at the top of the file, and stopCapture sat in the throwing
group while swallowing for as long as that comment existed: a contract stated in
a comment is not a contract, so the test is the half that matters.
EOF
```

---

## Track B — documentation and the routing map

### Task 6: Two corrections to CLAUDE.md

**Why this exists:** CLAUDE.md is the first thing every agent and contributor reads. Two of its statements are false, and both are the kind that stop somebody asking a question they should ask.

**Files:**
- Modify: `CLAUDE.md` — the build-status block
- Modify: `docs/ARCHITECTURE.md` §6 — the event table

**Interfaces:** none.

- [ ] **Step 1: Correct the model-pinning claim**

CLAUDE.md states *"A pilot must pin the model, and nothing currently does."* Verify it is false before changing it:

```bash
cd /Users/mrgee/WebstormProjects/relay
grep -n "select_stt_model\|STT_MODEL_KEY\|stt_model_setting" src-tauri/src/main.rs
```

Expected: `select_stt_model` at `:4836` (service-lock guarded at `:4838`), `STT_MODEL_KEY = "stt.model"` at `:4817`, and `build_stt` reading it at `:4714`.

Replace the claim with what is true: pinning exists and works — "Use this one" in `ModelSetup.svelte:172` writes `app_settings['stt.model']`, `build_stt` reads it at every launch, and the operator's choice beats `MODEL_CANDIDATES` order in `stt::resolve_model` (`stt.rs:1120`). Then state what is genuinely missing, which is the sentence worth keeping: **a pinned recognition language**. The picker at `Settings.svelte:1277` defaults to "Auto-detect (code-switching)", and RG-116 records that automatic language election cost a whole service on `ggml-small` — 17 incoherent transcripts against 161 with the language fixed. Add that no pre-service surface states which model and language a service will run on: `Dashboard.svelte:267` says "Ready for a service." over `ggml-base` without naming it.

- [ ] **Step 2: Correct the event count**

CLAUDE.md and `docs/ARCHITECTURE.md` §6 both say nineteen events. Reproduce the real set:

```bash
cd /Users/mrgee/WebstormProjects/relay
grep -rhoE '"[a-z_]+://[a-z_]+"' src-tauri/src/*.rs | sort -u
```

That yields twenty-one, of which `tauri://localhost` is an origin string at `channels.rs:2278` and not an event — so **twenty**. `docs/ARCHITECTURE.md` §6's table lists eighteen, omitting `output://error` and `output://transition`, one of which is an error path.

Add the two missing rows to §6's table. In CLAUDE.md, state twenty and keep the existing instruction to reproduce the set rather than trust the number — that instruction is why this was findable.

- [ ] **Step 3: Verify the cross-references still resolve**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/crossrefs.test.js
```

Expected: PASS. A citation that resolves to nothing is worse than an uncited claim.

- [ ] **Step 4: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add CLAUDE.md docs/ARCHITECTURE.md
git commit -F - <<'EOF'
docs: correct two statements in the handbook that were false

CLAUDE.md said a pilot must pin the STT model and that nothing does. Pinning
exists and has since select_stt_model landed: "Use this one" writes
app_settings['stt.model'], build_stt reads it at every launch, and the
operator's choice beats MODEL_CANDIDATES order in resolve_model. What is
genuinely missing beside it is a pinned recognition LANGUAGE — the picker
defaults to auto-detect, and RG-116 records that automatic election cost a
whole service on ggml-small, 17 incoherent transcripts against 161 with the
language fixed — and a pre-service statement of which model and language a
service will run on, since the Dashboard says "Ready for a service." over
ggml-base without naming it.

The event count said nineteen in two places. The scanner's own set is twenty,
and ARCHITECTURE section 6's table listed eighteen, omitting output://error and
output://transition. One of those is an error path.

Both are the kind of statement that stops somebody asking a question they
should ask, which is why they are worth correcting rather than footnoting.
EOF
```

---

### Task 7: The output routing map

**Why this exists:** Relay already has both realistic routing paths and no document says so, so the question "how do I get this onto the ATEM" has no answer in the repository. The research behind this task is in the spec; this task writes it down where a church can find it.

**Files:**
- Create: `docs/OUTPUT_ROUTING.md`
- Modify: `docs/README.md` — the index
- Modify: `docs/SPEC.md:25`
- Modify: `src/lib/views/Channels.svelte` — one helper

**Interfaces:**
- Consumes: `outputurl.js:29` (the one URL builder), `screenKind`/`screenTransport` (`outputHealth.js:473`).
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write `docs/OUTPUT_ROUTING.md`**

Four sections, one per case a church actually has, each stating what Relay emits and what the church supplies:

1. **A projector or a TV.** A fullscreen Tauri webview on a physical monitor — `channels::open_native_window` (`channels.rs:419`) places a borderless webview inside the target monitor's bounds and fullscreens it; `auto_open_outputs` (`main.rs:5164`) opens only `native_window` channels, only onto a connected display, and never onto the operator's own monitor.
2. **An ATEM.** Every ATEM Mini has HDMI inputs and no SDI inputs; every rack-mount ATEM has SDI inputs and no HDMI inputs, and the single HDMI connector on an ATEM Television Studio HD8 is an **output**. A Blackmagic Micro Converter HDMI to SDI 3G, about $75, bridges Relay's HDMI into any SDI ATEM. Relay emits a plain HDMI display signal and needs nothing SDI-aware, which is why the "no native SDI" constraint costs nothing. State plainly that **no ATEM accepts NDI** — Blackmagic's IP direction is SMPTE 2110 — that the ATEM **Media Player is stills only** at roughly three seconds per 1080p frame with a pool lock and twenty slots, and that **SuperSource composites inputs already on the switcher** and is not an ingest path.
3. **OBS, vMix or a kiosk screen.** The `:8032` URL in a browser source, and the fact that matters most: it costs **zero GPU display pipes**. Give the canonical URL and say to use **Copy URL** in Outputs — a hand-built `template_id`-only URL will not live-swap, because the swap is keyed on `channel` (DECISIONS §29).
4. **More screens than ports.** Apple Silicon has no DisplayPort MST extended desktop at any chip: base M1/M2 drive one external display, M3/M4 base two, only Max tiers four. DisplayLink is the only workaround, and granting it the macOS Screen Recording permission **disables HDCP system-wide** — harmless for scripture, a trap for a church that also plays a licensed clip. Windows supports MST natively. The real answer for most churches is section 3, because a network screen costs no pipe at all.

Close with what Relay does **not** do and why: NDI is parked (`open_ndi_output` returns a clear error), and it is worth recording that it is reachable without shipping a proprietary byte — Vizrt documents `NDIlib_v5_load()` runtime loading as being "of value in Open-Source projects", with a redistributable-URL constant for the absent-runtime case — but that it buys nothing toward an ATEM, which is what churches ask about.

- [ ] **Step 2: Fix the SPEC overstatement**

`docs/SPEC.md:25` claims Relay "talks to OBS, ATEM, and ProPresenter over NDI, HDMI, and the local network." NDI is parked and `open_ndi_output` returns an error. `SPEC.md:151`, `:169`, `:171` and `Settings.svelte:1421` all state it correctly; line 25 does not. Correct it and point at `docs/OUTPUT_ROUTING.md`.

- [ ] **Step 3: Add the helper in Outputs**

In `src/lib/views/Channels.svelte`, beside **Copy URL**, add a short "How do I reach this screen?" disclosure that names the case from the screen's own `render_target` — HDMI for `native_window`, a browser source for `network_client` — and links to the new document. No new command, no new state; it reads what the card already has.

- [ ] **Step 4: Index it and run the cross-reference test**

Add `OUTPUT_ROUTING` to `docs/README.md`'s index, then:

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/crossrefs.test.js src/lib/surface.test.js
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add docs/OUTPUT_ROUTING.md docs/README.md docs/SPEC.md src/lib/views/Channels.svelte
git commit -F - <<'EOF'
docs: write down how Relay reaches a projector, an ATEM, OBS and a fourth screen

Relay already has both realistic routing paths — a fullscreen window on a
physical monitor, and a URL in a browser source — and nothing in the repository
said so, so "how do I get this onto the ATEM" had no answer here.

The short version is that a 75-dollar Blackmagic Micro Converter dissolves the
whole SDI question: every ATEM Mini is HDMI-only, every rack-mount ATEM is
SDI-only, Relay emits a plain HDMI display signal either way, and nothing
SDI-aware is needed in software. Three things churches are told elsewhere are
dead ends and are named as such: no ATEM accepts NDI, the ATEM Media Player is
a stills pool at roughly three seconds a frame, and SuperSource composites
inputs that are already on the switcher rather than accepting a new one.

For a laptop with one HDMI port, the answer is the browser source, because a
network screen costs zero GPU display pipes — which matters most on Apple
Silicon, where no chip supports MST extended desktop and the base parts drive
one or two external displays.

SPEC line 25 claimed Relay talks to ATEM over NDI. NDI is parked and
open_ndi_output returns an error; three other lines in the same document said so
correctly and that one did not.
EOF
```

---

## Track C — the splash

### Task 8: Splash design pass

**Why this exists:** `src/lib/Splash.svelte` exists and works — 399 lines, amethyst brand (never amber, stated at `:9`), held `BOOT_HOLD_MS` 900 and capped `BOOT_CAP_MS` 4000 (`App.svelte:393`). On a healthy machine the operator sees the splash and then the console: the four stage screens and four gates appear only when they have something to say (`BootSequence.svelte:23`). This is a visual quality pass, not a build.

**Files:**
- Modify: `src/lib/Splash.svelte`
- Modify: `src/lib/boot/BootShell.svelte` — only if the splash and the stage screens disagree visually

**Interfaces:**
- Consumes: `BrandMark.svelte`, the `--v-*` tokens in `src/app.css`.
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Render it and look at it**

This machine cannot screenshot the Tauri window, so drive the splash in a browser against the mock bridge — the harness that found the two defects of the 2026-09-10 pass. Capture the splash at 1920×1080, 1366×768 and 1024×768.

- [ ] **Step 2: Make the changes**

Scope: the hero lockup, the wordmark and tagline, the stage and detail lines, the spinner, and the handover into the console. Keep amethyst; amber is ON AIR and the splash is not. Keep `prefers-reduced-motion` handling on the spinner and the pulse glyph.

**Change nothing about the boot ladder's logic.** `boot.js`'s "a stub can never render green" rule, the stage/gate ordering, and the `Esc`-skips-a-stage-never-a-gate behaviour are all out of scope.

- [ ] **Step 3: Confirm the timings still hold**

`BOOT_HOLD_MS` 900 and `BOOT_CAP_MS` 4000 exist so a fast machine does not flash the splash and a slow one is not held behind it. If a change makes either wrong, say so and change the constant deliberately rather than as a side effect.

- [ ] **Step 4: Run the suite**

```bash
cd /Users/mrgee/WebstormProjects/relay
npx vitest run src/lib/views/workspacegrammar.test.js src/lib/r2livepath.test.js
npx vitest run 2>&1 | tail -10
```

Expected: 0 failed.

- [ ] **Step 5: Commit**

```bash
cd /Users/mrgee/WebstormProjects/relay
git add src/lib/Splash.svelte src/lib/boot/BootShell.svelte
git commit -F - <<'EOF'
design: a pass over the splash screen

Visual only. The boot ladder's logic is untouched: the stages and gates still
appear only when they have something to say, a stub still cannot render green,
and Escape still skips a stage and never a gate. The hold and cap timings are
unchanged, so a fast machine still does not flash the splash and a slow one is
still not held behind it.

Rendered in a browser at three widths, because this machine cannot screenshot
the Tauri window.
EOF
```

---

### Task 9: Wave 1 verification

**Files:** `docs/qa/QA_HARNESS.md` §0 only.

- [ ] **Step 1: Build, then run both suites**

```bash
cd /Users/mrgee/WebstormProjects/relay
npm run build
cd src-tauri && cargo test 2>&1 | tail -20
cargo fmt --all && cargo clippy --all-targets -- -D warnings
cd .. && npx vitest run 2>&1 | tail -20
```

Expected: 0 failed on both, clippy clean.

- [ ] **Step 2: Drive the manual checks that no test can reach**

Three things in this wave are only true on a running app, and two of them are the findings themselves:

1. **Safe mode.** With a verse on a second screen and the mic live, turn safe mode on. The screen must go clear and the detector must stop, **now**, not at the next launch. Then relaunch with safe mode still on and confirm no screen reopens.
2. **The walk-through guard.** Start a service, go to Settings → History & Backup, and confirm the button is disabled and says why.
3. **The update refusal.** Online, let the launch check succeed, start recording a service, then press Check for Updates. It must name the service. Read the status row above the button at the same moment and confirm the two agree.

- [ ] **Step 3: Record the counts and commit**

Update `docs/qa/QA_HARNESS.md` §0 with both counts beside their commands, and nowhere else.

```bash
cd /Users/mrgee/WebstormProjects/relay
git add docs/qa/QA_HARNESS.md
git commit -m "docs: record the suite counts after wave 1"
```
