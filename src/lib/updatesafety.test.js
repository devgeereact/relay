// RG-06 — update safety, on the frontend side.
//
// Rust owns the snapshot, the preflight and the verdict (`updates.rs`, 12 tests).
// This file covers the three things only the console can get wrong, and the first
// is the one that would hurt:
//
//   1. Installing before the copy of the church's history exists.
//   2. Telling an operator their history has been restored when it has not.
//   3. Deciding on their behalf that a broken update is fine.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import fs from 'node:fs';
import path from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));
vi.mock('@tauri-apps/api/app', () => ({ getVersion: async () => '0.1.0-4' }));
const installed = vi.fn();
vi.mock('@tauri-apps/plugin-updater', () => ({
  check: async () => ({ version: '0.2.0', body: '', downloadAndInstall: installed }),
}));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: async () => {} }));

const store = await import('./stores/capture.js');
const updater = await import('./updater.js');

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

beforeEach(() => {
  invoke.mockReset();
  installed.mockClear();
  store.capture.update((s) => ({ ...s, available: true, capturing: false }));
  store.serviceLock.set({ engaged: false, recording: false, held_back: [] });
  updater.updateError.set(null);
  updater.updateVerdict.set(null);
  updater.snapshotPath.set(null);
});

async function findUpdate() {
  invoke.mockResolvedValue(null);
  return updater.checkForUpdate();
}

describe('the copy comes first', () => {
  it('snapshots the database BEFORE downloading anything', async () => {
    await findUpdate();
    const order = [];
    invoke.mockImplementation(async (cmd) => {
      order.push(cmd);
      return '/data/snapshots/pre-update-0.1.0-4-1.db';
    });
    installed.mockImplementation(async () => order.push('download'));

    await updater.installUpdate();

    expect(order[0]).toBe('update_begin');
    expect(order).toContain('download');
    expect(order.indexOf('update_begin')).toBeLessThan(order.indexOf('download'));
    expect(get(updater.snapshotPath)).toMatch(/pre-update/);
  });

  it('does NOT install when the copy could not be made', async () => {
    // The whole point. The binary can be reinstalled from a release page; the
    // church's history cannot be got back from anywhere.
    await findUpdate();
    invoke.mockRejectedValue({ kind: 'refused', message: 'the disk is full' });

    await updater.installUpdate();

    expect(installed).not.toHaveBeenCalled();
    expect(get(updater.updateError)).toMatch(/could not first make a copy of your history/i);
    expect(get(updater.updateProgress)).toBeNull();
  });

  it('passes the version it is updating FROM, so a restore knows what it goes back to', async () => {
    await findUpdate();
    const args = [];
    invoke.mockImplementation(async (cmd, a) => {
      args.push([cmd, a]);
      return '/snap.db';
    });
    await updater.installUpdate();
    expect(args[0]).toEqual(['update_begin', { fromVersion: '0.1.0-4' }]);
  });
});

// ── RELAY NEVER UPDATES DURING A SERVICE ────────────────────────────────────
//
// `updater.js`'s own rule, in capitals at the top of the file, and `installUpdate`
// is the call that makes it matter: it downloads and RESTARTS THE APPLICATION.
// The guard read `!capturing && !engaged`, under a comment claiming the lock is
// armed for the whole of a recorded service — which `main.rs` says is false the
// moment the operator lifts it, deliberately, because unlocking is a first-class
// override and does not end the service. The update banner sits in the shell the
// whole time, so the press is one click away from wherever the operator is.
describe('an update cannot restart Relay in the middle of a sermon', () => {
  it('refuses while a service is recording even though the lock has been LIFTED', async () => {
    await findUpdate();
    store.capture.update((s) => ({ ...s, capturing: false }));
    store.serviceLock.set({ engaged: false, recording: true, held_back: [] });
    invoke.mockResolvedValue('/snap.db');

    await updater.installUpdate();

    expect(installed, 'Relay downloaded an update mid-service').not.toHaveBeenCalled();
    expect(invoke, 'it got as far as taking a snapshot').not.toHaveBeenCalled();
    // And it names the SERVICE. "Stop listening" is unhelpful advice to somebody
    // whose microphone is already off and whose service is still running.
    expect(get(updater.updateError)).toMatch(/won't update during a service/i);
  });

  it('and still names the microphone when that is the reason', async () => {
    // The paired case, so a fix that simply reported "a service" for everything
    // would not pass. The operator has to know which one to clear.
    await findUpdate();
    store.capture.update((s) => ({ ...s, capturing: true }));
    store.serviceLock.set({ engaged: false, recording: false, held_back: [] });

    await updater.installUpdate();

    expect(installed).not.toHaveBeenCalled();
    expect(get(updater.updateError)).toMatch(/while you're listening/i);
  });

  it('installs perfectly well once nothing is running', async () => {
    // The control. A guard that refused everything would pass the two above.
    await findUpdate();
    store.capture.update((s) => ({ ...s, capturing: false }));
    store.serviceLock.set({ engaged: false, recording: false, held_back: [] });
    invoke.mockResolvedValue('/snap.db');

    await updater.installUpdate();

    expect(installed).toHaveBeenCalled();
  });
});

describe('the launch after an update', () => {
  it('says nothing at all when no update was pending', async () => {
    invoke.mockResolvedValue({ verdict: 'idle' });
    await updater.verifyLastUpdate();
    expect(get(updater.updateVerdict)).toBeNull();
  });

  it('surfaces a broken database with the snapshot it can go back to', async () => {
    invoke.mockResolvedValue({
      verdict: 'broken',
      from_version: '0.1.0-4',
      snapshot: '/snap.db',
      reason: 'a detections_new table was left behind',
    });
    await updater.verifyLastUpdate();
    expect(get(updater.updateVerdict)).toMatchObject({ verdict: 'broken', snapshot: '/snap.db' });
  });

  it('stays silent when there is no backend, rather than shouting on a dev machine', async () => {
    invoke.mockRejectedValue(new Error('no backend'));
    expect(await updater.verifyLastUpdate()).toBeNull();
    expect(get(updater.updateVerdict)).toBeNull();
  });

  it('a restore THROWS if it failed — an operator must not wait for history that is not coming', async () => {
    invoke.mockRejectedValue(new Error('that snapshot is no longer on this machine'));
    await expect(updater.restoreSnapshot('/gone.db')).rejects.toThrow();
  });

  it('accepting clears the verdict so it is asked once, not every launch', async () => {
    updater.updateVerdict.set({ verdict: 'landed', from_version: '0.1.0-4' });
    invoke.mockResolvedValue(null);
    await updater.acceptUpdate();
    expect(get(updater.updateVerdict)).toBeNull();
  });
});

describe('what the console may claim', () => {
  const app = read('src/App.svelte');

  it('a restore is described as taking effect on the NEXT LAUNCH', () => {
    // Copying a file over an open database corrupts both, so the restore happens
    // before the database is opened. An operator who believes it has already
    // happened will not restart, and will conclude Relay ignored them.
    expect(app).toMatch(/Restored on the next launch/);
  });

  it('never restores on its own — the operator chooses', () => {
    // A restore replaces a church's entire history. That is a decision with a
    // person's name on it.
    expect(app).toMatch(/Restore my history/);
    expect(app).toMatch(/Keep this and continue/);
    const rs = read('src-tauri/src/updates.rs');
    expect(rs).toMatch(/does NOT act on its own\s+.{0,4}conclusion/is);
  });

  it('a broken update outranks "another update is available"', () => {
    expect(app.indexOf('updateVerdict')).toBeLessThan(app.indexOf('$updateAvailable && !$capturing'));
  });

  it('the broken banner is rose, never amber', () => {
    const css = read('src/app.css');
    const rule = css.slice(css.indexOf('.upd.upd-bad{'));
    expect(rule.slice(0, 200)).toMatch(/--v-rose/);
    expect(rule.slice(0, 200)).not.toMatch(/--v-amber/);
  });
});

describe('the snapshot count is one decision in two languages', () => {
  it('matches updates::KEEP_SNAPSHOTS', () => {
    const rs = read('src-tauri/src/updates.rs');
    const keep = Number(/KEEP_SNAPSHOTS: usize = (\d+)/.exec(rs)[1]);
    expect(updater.KEEP_SNAPSHOTS).toBe(keep);
  });
});
