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
  });

  it('every command the frontend calls is registered in Rust', () => {
    const registered = new Set(commandsRegisteredInRust());
    const missing = commandsCalledByFrontend().filter((c) => !registered.has(c));
    expect(missing, `frontend calls commands that do not exist in Rust: ${missing.join(', ')}`).toEqual([]);
  });
});

describe('Tauri event contract', () => {
  // Events the Rust side emits, and where the frontend must be listening.
  const listeners = [
    captureJs,
    read('src/Output.svelte'),
    read('src/Stage.svelte'),
  ].join('\n');

  const emitted = new Set(
    [...mainRs.matchAll(/emit\(\s*"([a-z]+:\/\/[a-z]+)"/g)].map((m) => m[1]),
  );

  it('finds the emitted events', () => {
    expect(emitted.size).toBeGreaterThan(3);
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
