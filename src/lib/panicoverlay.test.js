// RULE 44 · an overlay that disarms Escape must consume it.
//
// `panic.test.js` holds that `Clear screens` can never be SCROLLED out of reach.
// Nothing held that it can never be PAINTED OVER, and nothing held that Escape,
// once withheld from the shell, is delivered to anybody. This file is that
// tripwire, and it exists because the gap was found at FOUR separate doors on one
// night, by five people looking at five different workspaces:
//
//   · `crash.js` — `role="alertdialog"`, full-screen, opaque, `z-index:99999`.
//     Driven in a real browser: `elementFromPoint` over `Clear screens` returned
//     the panel, and Escape fired neither `clear_screens` nor `blackout` and did
//     not dismiss the panel. Its own copy reads "Your output screens are still
//     live." So at the one moment the product GUARANTEES the wall is hot, the
//     operator had neither the key nor the button.
//   · `FirstRun.svelte` — same two failures, same measurement, and at the time it
//     was reachable in one UNGUARDED click from Settings ("Run the setup
//     walk-through"), over a recorded service. That click is guarded now: the
//     button is disabled while the microphone is live or the SERVICE LOCK IS
//     ENGAGED, and says why. (Engaged, not recording: `main.rs` documents at
//     length that the two are deliberately different — an operator who lifts the
//     lock has `engaged: false` over a service that is still open.) THAT CHANGES NOTHING THIS FILE HOLDS. The guard narrows when
//     the wizard can be opened; it does not give the operator back the key or the
//     button once it is open, and every other route to it — first launch, and safe
//     mode — is untouched. A door that is harder to open still owes an outcome for
//     the panic key it takes.
//   · `DetectionInspector.svelte` — opens from Live, DURING a service.
//   · four menus in `TemplateEditor` / `TemplateGallery`, each carrying a comment
//     claiming "Escape is handled globally — shortcuts.js gives Escape to any
//     mounted [role=menu]". It does not. `shortcuts.js` RETURNS. It gives the key
//     to nobody, which is the whole point of this file.
//
// The rule is one sentence: if you mount something that makes `shortcuts.js`
// stand down, you have taken the operator's panic key, and you owe them an
// outcome for it.
//
// WHY A SOURCE SCAN AND NOT A MOUNT. The failure is that a door was never
// enumerated — so the instrument has to be the enumeration itself, over the whole
// tree, finding doors nobody thought to list. A test that mounts the six overlays
// we currently know about would have passed on the night all four were broken.
// Same reasoning as `ipc.test.js` and `hardrules.test.js`.

import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { resolve, join } from 'node:path';
import { codeOnly } from './codeonly.js';

const SRC = resolve(process.cwd(), 'src');

/** The four roles `shortcuts.js` stands down for. Kept in sync by a test below. */
const DISARMING_ROLES = ['dialog', 'alertdialog', 'menu', 'listbox'];

/** Every .svelte/.js under src/, minus tests. */
function sources(dir = SRC, out = []) {
  for (const name of readdirSync(dir)) {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) sources(p, out);
    else if (/\.(svelte|js)$/.test(name) && !/\.test\.js$/.test(name)) out.push(p);
  }
  return out;
}

const rel = (p) => p.slice(resolve(process.cwd()).length + 1);

/** Strip comments — a claim in prose is not a handler, which is the bug itself. */
function code(text) {
  return codeOnly(text);
}

/**
 * Files that MOUNT one of the disarming roles.
 *
 * `shortcuts.js` and `focus.js` name the roles to reason about them rather than to
 * mount one, so they are matched by the selector strings they contain and excluded
 * by name. Both are covered by their own suites.
 */
const MOUNTERS = sources()
  .filter((p) => !/\/(shortcuts|focus)\.js$/.test(p))
  .map((p) => ({ path: rel(p), text: readFileSync(p, 'utf8') }))
  .filter(({ text }) =>
    DISARMING_ROLES.some(
      (r) =>
        // Markup, and — because `crash.js` is deliberately plain DOM and set its
        // role this way — the imperative form too. The first version of this scan
        // read only the markup form and therefore could not see the very panel
        // that prompted the rule. A scanner that quietly narrows passes everything.
        code(text).includes(`role="${r}"`) ||
        new RegExp(`setAttribute\\(\\s*['"]role['"]\\s*,\\s*['"]${r}['"]`).test(code(text)),
    ),
  );

/**
 * The boot gates are the one honest exemption, and they are exempt for a REASON
 * rather than by convenience: they render before `App.svelte` mounts, so there is
 * no shell behind them, no dock, no `Clear screens` and nothing on any screen that
 * a panic key could clear. Taking Escape there takes nothing away.
 *
 * Anything that is not a boot gate belongs in the list above it, not here — and the
 * test below refuses an entry that is not actually under `src/lib/boot/`, so this
 * cannot become a place to put an inconvenient finding.
 */
const NO_SHELL_BEHIND_THEM = [
  'src/lib/boot/CrashReportRecovery.svelte',
  'src/lib/boot/RecoverSession.svelte',
  'src/lib/boot/SafeModeStartup.svelte',
  'src/lib/boot/UpdateAvailable.svelte',
];

/**
 * The second exemption, and the better one: an overlay whose Escape is consumed
 * INSIDE `shortcuts.js`, by the one listener that owns the key.
 *
 * The cheatsheet is the only member and it is the model the other six should be
 * read against — `shortcuts.js:143` closes it and returns, so the key is neither
 * withheld nor duplicated, and there is exactly one place that decides. It is a
 * named list rather than a pattern because a file landing here by accident is a
 * file whose overlay is silently disarmed.
 */
const CONSUMED_BY_THE_ONE_LISTENER = [['src/App.svelte', 'cheatsheet']];

describe('rule 44 — an overlay that disarms Escape must consume it', () => {
  it('the scan can still see the overlays it is about', () => {
    // A scanner that quietly narrows passes everything. If a refactor moves these
    // files or changes how a role is spelled, this fails rather than going green
    // over an unexamined tree.
    const found = MOUNTERS.map((m) => m.path);
    for (const known of [
      'src/lib/crash.js',
      'src/lib/FirstRun.svelte',
      'src/lib/DetectionInspector.svelte',
      'src/lib/views/templates/TemplateEditor.svelte',
      'src/lib/views/templates/TemplateGallery.svelte',
      'src/lib/views/Library.svelte',
    ]) {
      expect(found).toContain(known);
    }
    expect(found.length).toBeGreaterThanOrEqual(10);
  });

  it('every overlay outside the boot gates handles Escape itself', () => {
    const byListener = CONSUMED_BY_THE_ONE_LISTENER.map(([p]) => p);
    const offenders = MOUNTERS.filter(({ path, text }) => {
      if (NO_SHELL_BEHIND_THEM.includes(path)) return false;
      if (byListener.includes(path)) return false;
      // A real handler, in code, not a sentence about one. Four of the doors that
      // prompted this rule carried a comment saying "Escape is handled globally —
      // shortcuts.js gives Escape to any mounted [role=menu]", which was false.
      return !/['"]Escape['"]/.test(code(text));
    }).map((m) => m.path);

    expect(offenders).toEqual([]);
  });

  it('…and an overlay handed to the one listener is really handled there', () => {
    // Otherwise this list becomes the place a disarmed overlay goes to hide.
    const guard = code(readFileSync(resolve(SRC, 'lib/shortcuts.js'), 'utf8'));
    const esc = guard.slice(guard.indexOf("e.key === 'Escape'"));
    for (const [path, store] of CONSUMED_BY_THE_ONE_LISTENER) {
      expect(MOUNTERS.map((m) => m.path)).toContain(path);
      // Named, and resolved BEFORE the blanket stand-down, or it is not consumed.
      // The stand-down is `overlayOpen()` since 2026-09-21, when the same probe
      // was hoisted so the CONTEXT keys stand down too (Live D1); it used to be an
      // inline `document.querySelector(`, which is still accepted here so a revert
      // of that hoist cannot make this test pass for the wrong reason.
      const stand = Math.max(esc.indexOf('overlayOpen()'), esc.indexOf('document.querySelector('));
      expect(stand, 'the Escape branch no longer stands down for an overlay at all').toBeGreaterThan(-1);
      expect(esc.indexOf(store)).toBeGreaterThan(-1);
      expect(esc.indexOf(store)).toBeLessThan(stand);
    }
  });

  it('…and the exemption list cannot be used for anything but a boot gate', () => {
    for (const p of NO_SHELL_BEHIND_THEM) {
      expect(p.startsWith('src/lib/boot/')).toBe(true);
      // And it must still be a real file that really mounts a role, or the list is
      // carrying a stale name that hides the next offender behind it.
      expect(MOUNTERS.map((m) => m.path)).toContain(p);
    }
  });

  it('the disarming roles here are the ones shortcuts.js actually stands down for', () => {
    // If somebody adds a fifth role to the guard, this file must learn about it in
    // the same commit — otherwise the new door is disarmed and unenumerated, which
    // is exactly how the four above happened.
    const guard = readFileSync(resolve(SRC, 'lib/shortcuts.js'), 'utf8');
    // One list, `OVERLAY_ROLES`, read by `overlayOpen()` from BOTH the Escape
    // branch and the context switch — so there is one place to add a fifth role.
    const at = guard.indexOf('OVERLAY_ROLES =');
    expect(at, 'shortcuts.js no longer keeps its overlay roles in one named list').toBeGreaterThan(-1);
    const line = guard.slice(at);
    const roles = [...line.slice(0, line.indexOf(';')).matchAll(/role="([a-z]+)"/g)].map(
      (m) => m[1],
    );
    expect(roles.sort()).toEqual([...DISARMING_ROLES].sort());
  });
});

describe('rule 44 — the crash panel, driven rather than read', () => {
  // `crash.js` is plain DOM by design ("the Svelte app may be the thing that just
  // broke"), so unlike the six Svelte overlays it can be mounted and pressed here.
  // This is the one that was measured in a browser, so it is the one that gets a
  // behavioural test rather than a source scan.

  it('Escape dismisses the panel, so the NEXT press reaches the screens', async () => {
    document.body.innerHTML = '';
    const { installCrashGuard } = await import('./crash.js');
    installCrashGuard();

    window.dispatchEvent(
      new ErrorEvent('error', { error: new Error('probe'), message: 'probe' }),
    );
    expect(document.getElementById('relay-crash-panel')).not.toBeNull();

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(document.getElementById('relay-crash-panel')).toBeNull();
  });

  it('and it never clears the screens ITSELF — one press, one outcome', async () => {
    // Rule 16's other half. The panel dismisses; it does not also wipe the wall,
    // because an operator who pressed Escape to get rid of a message did not ask
    // for a blank screen. The second press is theirs to make.
    document.body.innerHTML = '';
    const { installCrashGuard } = await import('./crash.js');
    installCrashGuard();
    window.dispatchEvent(
      new ErrorEvent('error', { error: new Error('probe'), message: 'probe' }),
    );

    const panel = document.getElementById('relay-crash-panel');
    expect(panel.textContent).toContain('still live');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(document.getElementById('relay-crash-panel')).toBeNull();
  });
});
