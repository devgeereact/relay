// Auto-update.
//
// Why this exists: we fixed six screen-facing bugs in one week, and there was no
// way on earth to get any of them to a church that had already installed Relay.
// For software that fails LIVE, in front of a congregation, an update path is not
// a nice-to-have — it is how a fix becomes a fix.
//
// ── The rule that outranks everything else in this file ─────────────────────
//
//   RELAY NEVER UPDATES DURING A SERVICE.
//
// Not a dialog, not a toast, not a background download that competes for a
// laptop's last 300 MB of RAM while whisper is running. An updater that
// interrupts a sermon is worse than no updater at all — it takes a tool that
// merely lacks a fix and turns it into a tool that actively causes a failure.
//
// So: we check on launch, we check only while idle, and we NEVER apply anything
// without the operator pressing the button. If the mic is live, we do not even
// look.

import { writable, get } from 'svelte/store';
import { capture, serviceLock } from './stores/capture.js';
import { humanError } from './errors.js';

/** { version, notes } when an update is waiting, else null. */
export const updateAvailable = writable(null);
/** 0–100 while downloading, else null. */
export const updateProgress = writable(null);
export const updateError = writable(null);
/** How many pre-update copies Relay keeps. Must match `updates::KEEP_SNAPSHOTS`. */
export const KEEP_SNAPSHOTS = 3;

/** Where the pre-update copy of the database was written, once one exists. */
export const snapshotPath = writable(null);
/**
 * The verdict on the LAST update, asked once on the launch after one.
 * `null` until asked; then one of Idle / DidNotInstall / Landed / Broken.
 */
export const updateVerdict = writable(null);

async function currentVersion() {
  try {
    return await (await import('@tauri-apps/api/app')).getVersion();
  } catch {
    return '';
  }
}

/**
 * Did the last update actually work?
 *
 * Called once at launch. It deliberately does NOT act on its own conclusion: a
 * restore replaces a church's entire history, and that is a decision with a person's
 * name on it.
 */
export async function verifyLastUpdate() {
  try {
    const { invoke } = await import('@tauri-apps/api/core');
    const v = await invoke('update_verify', { currentVersion: await currentVersion() });
    updateVerdict.set(v?.verdict === 'idle' ? null : v);
    return v;
  } catch {
    // No backend, or the command is gone. An update-verification that shouts on a
    // dev machine would be noise; silence is right.
    return null;
  }
}

/** The operator accepts the update. GROUP 1 — THROWS. */
export async function acceptUpdate() {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('update_accept');
  updateVerdict.set(null);
}

/**
 * The operator wants their history back. GROUP 1 — THROWS.
 *
 * Takes effect on the NEXT LAUNCH, and the caller must say so: an operator who
 * believes it has already happened will not restart, and will conclude Relay ignored
 * them.
 */
export async function restoreSnapshot(snapshot) {
  const { invoke } = await import('@tauri-apps/api/core');
  await invoke('update_restore', { snapshot });
  updateVerdict.set(null);
}

let pending = null; // the Update handle from the plugin

/** Is it safe to touch the updater right now? */
function idle() {
  // TWO facts, because the microphone alone was not enough. A service can be
  // recording with the mic momentarily stopped — between readings, while the
  // operator changes an input, after an `audio://error` — and every one of those
  // is a moment when restarting the app is the worst thing Relay could offer.
  // The service lock is armed for the whole of a recorded service, so it closes
  // the gap the mic flag left open.
  return !get(capture).capturing && !get(serviceLock).engaged;
}

/**
 * WHAT THE LAST UPDATE CHECK ACTUALLY FOUND.
 *
 * `{ state, at, detail }` where `state` is one of:
 *
 *   `unchecked`   — no check has completed this session (or a service was running).
 *   `ok`          — the update server answered. `detail` is the version, or ''.
 *   `unavailable` — there is no updater here at all: a browser, a dev build, an
 *                   unsigned build. NOT a fault, and not worth a word to anyone.
 *   `failed`      — a check ran and could not get an answer. Offline is the common
 *                   and harmless reason; a manifest that does not exist is not.
 *
 * ── Why this store had to exist ──────────────────────────────────────────────
 *
 * `checkForUpdate` swallowed every outcome into `null`, and Settings printed
 * **"Update status · up to date"** whenever nothing was waiting. So "there is no
 * newer version", "you are offline", "you have never checked" and "the update
 * server has been returning 404 since the day this was installed" were one
 * sentence, and it was the reassuring one.
 *
 * That is precisely the defect CLAUDE.md rule 35 names: a status badge that cannot
 * detect its own failure. It matters more here than almost anywhere, because the
 * update channel is how a fix reaches a church that has already installed Relay —
 * and if it is broken, nothing else in the product will ever say so.
 *
 * It does NOT change the rule above it: a failed check still never interrupts an
 * operator, still never becomes a toast, and is still invisible during a service.
 * It is written down, for the one screen where somebody goes to look.
 */
export const updateChannel = writable({ state: 'unchecked', at: null, detail: '' });

const noteChannel = (state, detail = '') =>
  updateChannel.set({ state, at: Date.now(), detail });

/**
 * Check for an update. Safe to call on launch.
 *
 * Returns the available version, or `null`. Does nothing while a service is
 * running. A failed check must never surface as an error to an operator — it is
 * not their problem and there is nothing they can do about it mid-sermon — so the
 * outcome is RECORDED (`updateChannel`) rather than raised.
 */
export async function checkForUpdate() {
  if (!idle()) return null;
  let check;
  try {
    ({ check } = await import('@tauri-apps/plugin-updater'));
  } catch {
    // No backend and no plugin: a browser, or a build with no updater wired. There
    // is nothing here to be broken, so this is not a failure.
    noteChannel('unavailable');
    return null;
  }
  try {
    const update = await check();
    if (!update) {
      noteChannel('ok');
      return null;
    }
    pending = update;
    updateAvailable.set({ version: update.version, notes: update.body ?? '' });
    noteChannel('ok', update.version);
    return update.version;
  } catch (e) {
    // Offline is the ordinary reason and Relay's whole premise is that it works
    // with the network unplugged. A manifest that does not exist looks identical
    // from here — which is exactly why the reason is kept rather than discarded.
    noteChannel('failed', String(e?.message ?? e ?? '').slice(0, 200));
    return null;
  }
}

/** The sentence for a channel state. One place, so no surface invents a second. */
export function describeChannel(ch) {
  switch (ch?.state) {
    case 'ok':
      return ch.detail ? `${ch.detail} available` : 'up to date';
    case 'failed':
      return 'could not reach the update server';
    case 'unavailable':
      return 'no update channel in this build';
    default:
      return 'not checked yet';
  }
}

/**
 * Download and install the pending update, then relaunch.
 *
 * Only ever called from an explicit operator click, and refuses outright if the
 * microphone is live — a mis-click during a sermon must not restart the app.
 */
/**
 * Take the snapshot, then install.
 *
 * THE ORDER IS THE WHOLE POINT. The binary can always be got back — the installers
 * are public and signed. What cannot is the church's database if a migration in the
 * new version goes wrong on their particular data: there is no undo and no copy
 * anywhere else, because that is what offline-first means. So the copy is taken
 * BEFORE the download starts, and a preflight that cannot copy stops the update
 * rather than proceeding without one.
 */
export async function installUpdate() {
  if (!pending) return;
  if (!idle()) {
    // Names BOTH reasons, because the operator has to know which one to clear.
    // "Stop listening" is unhelpful advice to someone whose microphone is already
    // off and whose service is still recording.
    updateError.set(
      get(serviceLock).engaged
        ? "Relay won't update during a service — an update restarts the app. End the service first."
        : "Relay won't update while you're listening. Stop listening first — an update restarts the app.",
    );
    return;
  }
  updateError.set(null);
  updateProgress.set(0);
  try {
    // Snapshot first, and let a refusal stop the update. `update_begin` re-runs the
    // preflight itself, so a database that went unhealthy between the operator
    // reading the check and pressing the button is still caught.
    const { invoke } = await import('@tauri-apps/api/core');
    const version = await currentVersion();
    snapshotPath.set(await invoke('update_begin', { fromVersion: version }));
  } catch (e) {
    updateProgress.set(null);
    updateError.set(
      `Relay did not update, because it could not first make a copy of your history: ${humanError(e)}`,
    );
    return;
  }
  try {
    let total = 0;
    let got = 0;
    await pending.downloadAndInstall((e) => {
      if (e.event === 'Started') total = e.data.contentLength ?? 0;
      else if (e.event === 'Progress') {
        got += e.data.chunkLength ?? 0;
        if (total > 0) updateProgress.set(Math.round((got / total) * 100));
      } else if (e.event === 'Finished') updateProgress.set(100);
    });
    const { relaunch } = await import('@tauri-apps/plugin-process');
    await relaunch();
  } catch (e) {
    updateProgress.set(null);
    updateError.set(
      `The update couldn't be installed: ${String(e)}. Relay will keep working on this version.`,
    );
  }
}

/** Operator chose "not now". Don't nag mid-preparation. */
export function dismissUpdate() {
  updateAvailable.set(null);
  updateError.set(null);
}
