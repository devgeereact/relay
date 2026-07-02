// Audio capture store — bridges the Rust audio engine (Phase 3) to the UI.
//
// Wraps the Tauri commands (list_audio_devices / start_capture / stop_capture)
// and the `audio://chunk` event stream. Degrades gracefully in a plain browser
// (vite dev with no Tauri runtime): `available` stays false and the controls
// disable, so the console still renders for design work.

import { writable } from 'svelte/store';

export const capture = writable({
  available: false, // Tauri backend attached?
  capturing: false,
  level: 0, // latest chunk RMS (0..~1)
  isVoice: false, // VAD gate result for the latest chunk
  devices: [], // [{ name, is_default }]
});

let unlisten = null;

async function tauri() {
  // Throws in a plain browser — callers treat that as "backend absent".
  const core = await import('@tauri-apps/api/core');
  return core.invoke;
}

/** Probe the backend and load the device list. Safe to call on mount. */
export async function initAudio() {
  try {
    const invoke = await tauri();
    const devices = await invoke('list_audio_devices');
    capture.update((s) => ({ ...s, available: true, devices }));
  } catch {
    capture.update((s) => ({ ...s, available: false }));
  }
}

/** Start capture from `device` (name string, or null for the default input). */
export async function startCapture(device) {
  const invoke = await tauri();
  const { listen } = await import('@tauri-apps/api/event');
  await invoke('start_capture', { device: device ?? null });
  unlisten = await listen('audio://chunk', (e) => {
    const { rms, is_voice } = e.payload;
    capture.update((s) => ({ ...s, level: rms, isVoice: is_voice }));
  });
  capture.update((s) => ({ ...s, capturing: true }));
}

/** Stop capture and detach the event listener. Idempotent. */
export async function stopCapture() {
  try {
    const invoke = await tauri();
    await invoke('stop_capture');
  } catch {
    /* backend gone — nothing to stop */
  }
  if (unlisten) {
    unlisten();
    unlisten = null;
  }
  capture.update((s) => ({ ...s, capturing: false, level: 0, isVoice: false }));
}
