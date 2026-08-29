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

/** { version, notes } when an update is waiting, else null. */
export const updateAvailable = writable(null);
/** 0–100 while downloading, else null. */
export const updateProgress = writable(null);
export const updateError = writable(null);

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
 * Check for an update. Safe to call on launch.
 *
 * Silently does nothing when: there's no Tauri backend (browser/dev), the app
 * isn't signed with an updater key yet, or a service is running. A failed update
 * check must never surface as an error to an operator — it is not their problem
 * and there is nothing they can do about it.
 */
export async function checkForUpdate() {
  if (!idle()) return null;
  try {
    const { check } = await import('@tauri-apps/plugin-updater');
    const update = await check();
    if (!update) return null;
    pending = update;
    updateAvailable.set({ version: update.version, notes: update.body ?? '' });
    return update.version;
  } catch {
    // No backend, no updater config, or offline. All fine — Relay's whole
    // premise is that it works with the network unplugged.
    return null;
  }
}

/**
 * Download and install the pending update, then relaunch.
 *
 * Only ever called from an explicit operator click, and refuses outright if the
 * microphone is live — a mis-click during a sermon must not restart the app.
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
