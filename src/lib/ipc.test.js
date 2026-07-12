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
const mainRs = read('src-tauri/src/main.rs');

/** Every `call('name', …)` in the store. */
function commandsCalledByFrontend() {
  const names = new Set();
  for (const m of captureJs.matchAll(/\bcall\(\s*['"]([a-z0-9_]+)['"]/g)) {
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
