// RG-03 — the SERVICE LOCK, on the frontend side.
//
// The Rust half decides what is held back and proves it (`servicelock.rs`, and the
// e2e test that drives the real commands with a real service recording). This file
// covers the three things only the frontend can get wrong:
//
//   1. Claiming the console is protected when it is not, or the reverse.
//   2. Reporting an unlock that did not happen.
//   3. Letting the updater offer a restart during a service, which is the gap the
//      microphone flag alone left open.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import fs from 'node:fs';
import path from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));
// A pending update, so `installUpdate` gets past its "nothing to install" guard
// and actually reaches the decision under test.
const installed = vi.fn();
vi.mock('@tauri-apps/plugin-updater', () => ({
  check: async () => ({ version: '9.9.9', body: '', downloadAndInstall: installed }),
}));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: async () => {} }));

const store = await import('./stores/capture.js');
const updater = await import('./updater.js');

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

beforeEach(() => {
  invoke.mockReset();
  store.capture.update((s) => ({ ...s, available: true, capturing: false }));
  store.serviceLock.set({ engaged: false, held_back: [] });
});

describe('reading the lock', () => {
  it('reports what Rust says, list and all', async () => {
    invoke.mockResolvedValue({ engaged: true, held_back: ['deleting a template'] });
    await store.loadServiceLock();
    expect(get(store.serviceLock)).toEqual({
      engaged: true,
      held_back: ['deleting a template'],
    });
  });

  it('falls back to UNPROTECTED, never to protected, when the backend is absent', async () => {
    // The safe direction is the one that cannot stop an operator working. A
    // console that wrongly believes it is protecting a service would refuse
    // nothing (Rust decides that) while telling the operator it had — a status
    // that lies in the more confusing direction.
    invoke.mockRejectedValue(new Error('no backend'));
    await store.loadServiceLock();
    expect(get(store.serviceLock).engaged).toBe(false);
  });

  it('starting a service reads the lock back rather than assuming it armed', async () => {
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'start_service') return 7;
      if (cmd === 'service_lock') return { engaged: true, held_back: ['deleting a song'] };
      throw new Error(`unexpected ${cmd}`);
    });
    await store.startService('Sunday Service', '2026-08-29');
    expect(get(store.serviceLock).engaged).toBe(true);
    expect(invoke.mock.calls.map(([c]) => c)).toContain('service_lock');
  });

  it('ending a service reads it back too', async () => {
    store.serviceLock.set({ engaged: true, held_back: [] });
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'end_service') return null;
      if (cmd === 'service_lock') return { engaged: false, held_back: [] };
      throw new Error(`unexpected ${cmd}`);
    });
    await store.endService();
    expect(get(store.serviceLock).engaged).toBe(false);
  });
});

describe('lifting the lock', () => {
  it('takes the new state from Rust, not from what was asked for', async () => {
    // The button says "unlock". If Rust comes back still engaged, the UI must show
    // engaged — otherwise the operator is looking at an unlocked console that keeps
    // refusing, which is the panic-control lie in a smaller costume.
    invoke.mockResolvedValue(true);
    const engaged = await store.setServiceLock(false);
    expect(engaged).toBe(true);
    expect(get(store.serviceLock).engaged).toBe(true);
  });

  it('unlocks when Rust agrees', async () => {
    store.serviceLock.set({ engaged: true, held_back: ['deleting a song'] });
    invoke.mockResolvedValue(false);
    await store.setServiceLock(false);
    expect(get(store.serviceLock).engaged).toBe(false);
    // …and the list is kept, because it is still true of the next service.
    expect(get(store.serviceLock).held_back).toEqual(['deleting a song']);
  });

  it('THROWS when the command fails — GROUP 1, and the caller renders the reason', async () => {
    invoke.mockRejectedValue(new Error('poisoned lock'));
    await expect(store.setServiceLock(false)).rejects.toThrow();
  });
});

describe('the updater may not offer a restart during a service', () => {
  it('is held off by the microphone', async () => {
    store.capture.update((s) => ({ ...s, capturing: true }));
    expect(await updater.checkForUpdate()).toBeNull();
  });

  it('is ALSO held off by a recorded service with the mic momentarily stopped', async () => {
    // THE GAP. A service can be recording while `capturing` is false — between
    // readings, while the operator changes an input, after an `audio://error`.
    // Every one of those is a moment when "restart to update" is the worst thing
    // Relay could put on screen.
    store.capture.update((s) => ({ ...s, capturing: false }));
    store.serviceLock.set({ engaged: true, held_back: [] });
    expect(await updater.checkForUpdate()).toBeNull();
  });

  it('refuses to INSTALL during a service, and says which reason to clear', async () => {
    installed.mockClear();
    // Find an update while nothing is happening…
    store.capture.update((s) => ({ ...s, capturing: false }));
    store.serviceLock.set({ engaged: false, held_back: [] });
    expect(await updater.checkForUpdate()).toBe('9.9.9');

    // …then the service starts before the operator presses Install.
    store.serviceLock.set({ engaged: true, held_back: [] });
    await updater.installUpdate();

    expect(installed).not.toHaveBeenCalled();
    // "Stop listening" is useless advice to someone whose microphone is already
    // off and whose service is still recording, so the message names the real one.
    expect(get(updater.updateError)).toMatch(/during a service/i);
    expect(get(updater.updateError)).not.toMatch(/while you're listening/i);
  });
});

describe('the contract with the Rust half', () => {
  it('the frontend keeps NO copy of what is held back', () => {
    // The list rides with the flag from `servicelock::PROTECTED`. A second copy
    // here would be a second answer to one question, and the drift would be
    // invisible: the UI would name actions that are not held back, or stay silent
    // about ones that are.
    const settings = read('src/lib/views/Settings.svelte');
    // The operator-facing phrases live in `servicelock::PROTECTED` and nowhere
    // else. Finding one hardcoded here means a second list has been born.
    for (const phrase of [
      'deleting a template',
      'downloading a speech model',
      'changing the Bible translation',
    ])
      expect(settings).not.toContain(phrase);
    // …and the real list is rendered from the store.
    expect(settings).toMatch(/\$serviceLock\.held_back/);
  });

  it('the unlock is reachable from the sentence the refusal prints', () => {
    // Rust's refusal says "unlock in Settings → Backup & Recovery". If the control
    // is not there, the refusal is a dead end and the lock becomes a wall.
    const rs = read('src-tauri/src/servicelock.rs');
    expect(rs).toMatch(/Settings → Backup & Recovery/);
    const settings = read('src/lib/views/Settings.svelte');
    expect(settings).toMatch(/section === 'backup'/);
    expect(settings).toMatch(/setServiceLock\(false\)|unlockService/);
  });
});
