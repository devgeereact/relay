// "UP TO DATE" MUST MEAN SOMEBODY ASKED AND GOT AN ANSWER.
//
// Settings printed **Update status · up to date** whenever `updateAvailable` was
// null — and `checkForUpdate` swallowed every outcome into null. So one reassuring
// sentence covered four completely different situations:
//
//   • there genuinely is no newer version
//   • the laptop is offline
//   • no check has ever run
//   • the update manifest has returned 404 since the day this copy was installed
//
// The last one is not hypothetical. Both configs point at
// `…/releases/latest/download/latest.json`; GitHub's `/latest/` excludes
// pre-releases, and every Relay release so far is a pre-release, so the endpoint
// resolves to nothing (RG-83, verified with `curl`). Every installed copy is
// un-updatable, and the only surface that could have said so said "up to date".
//
// That is CLAUDE.md rule 35 — a status badge that cannot detect its own failure —
// on the one path by which a fix reaches a church that already has Relay.
//
// The rule ABOVE it is untouched and this file pins that too: a failed check still
// never interrupts an operator and still never throws. It is written down, for the
// one screen where somebody goes to look.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const check = vi.fn();
let importFails = false;
vi.mock('@tauri-apps/plugin-updater', () => ({
  get check() {
    if (importFails) throw new Error('no plugin here');
    return check;
  },
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const { checkForUpdate, updateChannel, describeChannel, updateAvailable } = await import(
  './updater.js'
);
const { capture, serviceLock } = await import('./stores/capture.js');

describe('the update channel reports on itself', () => {
  beforeEach(() => {
    check.mockReset();
    importFails = false;
    updateChannel.set({ state: 'unchecked', at: null, detail: '' });
    updateAvailable.set(null);
    capture.update((s) => ({ ...s, capturing: false }));
    serviceLock.set({ engaged: false });
  });

  it('starts as UNCHECKED, not as up to date', () => {
    // The state a fresh launch is really in. Printing "up to date" here is a claim
    // about a server nobody has spoken to.
    expect(get(updateChannel).state).toBe('unchecked');
    expect(describeChannel(get(updateChannel))).toBe('not checked yet');
  });

  it('records a successful check that found nothing', async () => {
    check.mockResolvedValue(null);
    expect(await checkForUpdate()).toBeNull();
    expect(get(updateChannel).state).toBe('ok');
    expect(describeChannel(get(updateChannel))).toBe('up to date');
  });

  it('records a successful check that found a version', async () => {
    check.mockResolvedValue({ version: '0.2.0', body: 'notes' });
    expect(await checkForUpdate()).toBe('0.2.0');
    expect(get(updateChannel).state).toBe('ok');
    expect(describeChannel(get(updateChannel))).toBe('0.2.0 available');
    expect(get(updateAvailable)).toEqual({ version: '0.2.0', notes: 'notes' });
  });

  it('a dead endpoint is FAILED — never "up to date"', async () => {
    // What RG-83 looks like from inside the app: the plugin fetches the manifest,
    // GitHub answers 404, the plugin throws.
    check.mockRejectedValue(new Error('Could not fetch a valid release JSON: 404'));
    expect(await checkForUpdate()).toBeNull();
    expect(get(updateChannel).state).toBe('failed');
    expect(describeChannel(get(updateChannel))).toBe('could not reach the update server');
    expect(get(updateChannel).detail).toMatch(/404/);
  });

  it('a failed check still never throws at the operator', async () => {
    // The rule this store was NOT allowed to break. `checkForUpdate` runs on launch
    // and from a Settings button; a rejection here would surface mid-setup as an
    // unhandled error about a server the operator cannot do anything about.
    check.mockRejectedValue(new Error('offline'));
    await expect(checkForUpdate()).resolves.toBeNull();
  });

  it('no updater in this build is UNAVAILABLE, not a failure', async () => {
    // A browser, a dev build, an unsigned build. Nothing here is broken, and
    // colouring it as a fault would train the operator to ignore the row.
    importFails = true;
    expect(await checkForUpdate()).toBeNull();
    expect(get(updateChannel).state).toBe('unavailable');
    expect(describeChannel(get(updateChannel))).toBe('no update channel in this build');
  });

  it('never checks during a service, and does not overwrite what it last knew', async () => {
    check.mockResolvedValue(null);
    await checkForUpdate();
    expect(get(updateChannel).state).toBe('ok');

    serviceLock.set({ engaged: true });
    check.mockRejectedValue(new Error('should never be called'));
    expect(await checkForUpdate()).toBeNull();
    expect(check).toHaveBeenCalledTimes(1);
    expect(get(updateChannel).state).toBe('ok');
  });
});
