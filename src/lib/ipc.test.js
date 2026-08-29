// The contract test between the Svelte frontend and the Rust backend.
//
// `capture.js` addresses ~76 Tauri commands by STRING. Nothing checks those
// strings against the Rust side, and almost every wrapper swallows failures in a
// `catch {}` — so renaming a `#[tauri::command]` in Rust does not break the
// build, does not fail a test, and does not even log. It just makes a button
// quietly stop working, and you find out during a service.
//
// This closes that hole: every command the frontend calls must be registered in
// Rust's `invoke_handler!`, and (the other direction) every event the backend
// emits must be listened for somewhere.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, it, expect } from 'vitest';

const root = resolve(__dirname, '../..');
const read = (p) => readFileSync(resolve(root, p), 'utf8');

const captureJs = read('src/lib/stores/capture.js');
// The launch checks (LAUNCH & STARTUP) call Tauri DIRECTLY rather than through
// the store, because a boot probe wants the raw result — not a wrapper that
// swallows the failure it is trying to measure. That put them outside this
// contract entirely: a renamed command would have made a boot check report
// `fail` with a Tauri "command not found" string, on the first screen an
// operator ever sees, and nothing would have caught it. Now it is covered.
const probesJs = read('src/lib/boot/probes.js');
// The output pages report that they are still painting by calling Tauri directly
// too, for the same reason the probes do: a health signal routed through a store
// wrapper that swallows failures would be a health signal that cannot report its
// own. Added to the contract the moment it existed — a command called from a third
// file that this test did not read is precisely the door nobody checks.
const outputHealthJs = read('src/lib/outputHealth.js');
// The updater is the fourth file that calls Tauri directly, for the same reason
// again: an update-safety check routed through a wrapper that swallows failures
// would be a safety check that cannot report its own.
const updaterJs = read('src/lib/updater.js');
const mainRs = read('src-tauri/src/main.rs');

/** Every `call('name', …)` in the store, and every `invoke('name', …)` in the probes. */
function commandsCalledByFrontend() {
  const names = new Set();
  for (const m of captureJs.matchAll(/\bcall\(\s*['"]([a-z0-9_]+)['"]/g)) {
    names.add(m[1]);
  }
  for (const m of probesJs.matchAll(/\binvoke\(\s*['"]([a-z0-9_]+)['"]/g)) {
    names.add(m[1]);
  }
  for (const m of outputHealthJs.matchAll(/\binv\(\s*['"]([a-z0-9_]+)['"]/g)) {
    names.add(m[1]);
  }
  for (const m of updaterJs.matchAll(/\binvoke\(\s*['"]([a-z0-9_]+)['"]/g)) {
    names.add(m[1]);
  }
  return [...names].sort();
}

/** Everything registered in `tauri::generate_handler![ … ]`. */
function commandsRegisteredInRust() {
  const block = mainRs.match(/generate_handler!\[([\s\S]*?)\]/);
  if (!block) throw new Error('could not find generate_handler! in main.rs');
  return block[1]
    .split(',')
    .map((s) => s.trim())
    .filter((s) => /^[a-z0-9_]+$/.test(s))
    .sort();
}

describe('Tauri IPC contract', () => {
  it('finds a non-trivial number of commands (the regexes still work)', () => {
    // Guards against this whole test silently passing because a refactor changed
    // the call shape and both sets became empty.
    expect(commandsCalledByFrontend().length).toBeGreaterThan(50);
    expect(commandsRegisteredInRust().length).toBeGreaterThan(50);
  });

  it('still sees the launch probes (the regex covers invoke(), not just call())', () => {
    // Guards the guard: if probes.js is refactored to a different call shape,
    // this test must fail loudly rather than start covering nothing.
    expect(commandsCalledByFrontend()).toContain('data_health');
    expect(probesJs).toMatch(/invoke\(\s*['"]stt_status['"]/);
    // …and the output pages' beat, which is the third file that calls Tauri
    // without going through the store.
    expect(commandsCalledByFrontend()).toContain('output_beat');
    expect(commandsCalledByFrontend()).toContain('update_verify');
  });

  it('every command the frontend calls is registered in Rust', () => {
    const registered = new Set(commandsRegisteredInRust());
    const missing = commandsCalledByFrontend().filter((c) => !registered.has(c));
    expect(missing, `frontend calls commands that do not exist in Rust: ${missing.join(', ')}`).toEqual([]);
  });

  // ── The boot heartbeat has exactly one caller ──
  //
  // `greet` prints "console: webview up" and is the ONLY way to tell, on a machine
  // that cannot screenshot the app, that the webview loaded and reached the Tauri
  // bridge (CLAUDE.md). Its value is entirely in the COUNT: one line, one mount.
  //
  // The new-design branch added `probes.js:engine()` calling `greet` as a liveness
  // check, and that probe runs from BOTH the launch sequence and the Dashboard — so
  // every launch printed the heartbeat three times. Nothing was wrong with the app,
  // but the one instrument for diagnosing a blank console now read like a webview
  // reloading twice, and a real double-mount would have been invisible in the noise.
  //
  // Liveness probes call `ping` (silent). This keeps it that way.
  it('the boot heartbeat (greet) is called from exactly one place', () => {
    const sources = [
      ['src/App.svelte', read('src/App.svelte')],
      ['src/lib/boot/probes.js', probesJs],
      ['src/lib/stores/capture.js', captureJs],
      ['src/lib/boot/BootSequence.svelte', read('src/lib/boot/BootSequence.svelte')],
      ['src/lib/views/Dashboard.svelte', read('src/lib/views/Dashboard.svelte')],
    ];
    const callers = sources.filter(([, text]) => /['"]greet['"]/.test(text)).map(([f]) => f);
    expect(
      callers,
      `greet is the boot heartbeat and must have ONE caller (App.svelte). ` +
        `Liveness probes call 'ping' instead. Found: ${callers.join(', ')}`,
    ).toEqual(['src/App.svelte']);
  });
});

describe('Tauri event contract', () => {
  // Events the Rust side emits, and where the frontend must be listening.
  const listeners = [
    captureJs,
    read('src/Output.svelte'),
    read('src/Stage.svelte'),
  ].join('\n');

  // The name part must allow `_`. It did not, and two events were silently
  // outside this contract for as long as it has existed: `stt://language_unstable`
  // and `output://panic_failed` — the second being a PANIC path, the one place a
  // dropped event is least affordable. They matched nothing, so they were never
  // checked, and the test still passed and still looked exhaustive.
  const emitted = new Set(
    [...mainRs.matchAll(/emit\(\s*"([a-z_]+:\/\/[a-z_]+)"/g)].map((m) => m[1]),
  );

  it('finds the emitted events', () => {
    expect(emitted.size).toBeGreaterThan(3);
  });

  // Guards the regex itself, not the app. A character class that quietly stops
  // matching is invisible: the suite goes green because it checked nothing.
  it('the scanner sees the underscored events, not just the simple ones', () => {
    expect([...emitted]).toEqual(
      expect.arrayContaining(['stt://language_unstable', 'output://panic_failed']),
    );
  });

  it('every event the backend emits is listened for on the frontend', () => {
    const unheard = [...emitted].filter((e) => !listeners.includes(e));
    expect(unheard, `backend emits events nobody listens to: ${unheard.join(', ')}`).toEqual([]);
  });
});

// ── The output port ──
//
// `5032` is the Vite dev server. It exists ONLY under `npm run tauri dev`. In the
// packaged app there is no server on that port at all — so an OBS browser source
// pointed at `http://host:5032/output.html` shows a blank screen, with no error in
// the app, nothing in any log, and no way for a volunteer to work out why.
//
// The README, the user guide and CLAUDE.md all said 5032 for months. Channels.svelte
// always emitted 8032 correctly, so the Copy-URL button in the app was right while
// every document telling a human what to type was wrong — and nothing could catch it,
// because `tauri dev` HAS a server on 5032 and everything looks fine.
//
// It was found by launching the actual release binary. This keeps it found.
describe('the output URL handed to OBS / kiosks', () => {
  const channels = read('src/lib/views/Channels.svelte');
  const docs = ['README.md', 'docs/USER_GUIDE.md', 'CLAUDE.md'].map((f) => [f, read(f)]);

  it('is served from the embedded HTTP server (8032), not the dev server (5032)', () => {
    expect(channels).toMatch(/:8032\/output\.html/);
    expect(channels).not.toMatch(/:5032\/output\.html/);
  });

  it('is not misdocumented anywhere a human would read it', () => {
    for (const [file, text] of docs) {
      const bad = text
        .split('\n')
        .filter((l) => /5032\/(output|stage)\.html/.test(l))
        // The warnings that exist precisely to say "not 5032" are allowed to name it.
        .filter((l) => !/NOT 5032|not 5032|does not exist|blank screen/.test(l));
      expect(bad, `${file} points an operator at the dev server`).toEqual([]);
    }
  });
});
