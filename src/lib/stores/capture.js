// Audio capture + transcript store — bridges the Rust audio/STT engines to the
// UI (Phases 3-4).
//
// Wraps the Tauri commands (list_audio_devices / start_capture / stop_capture /
// stt_status) and the `audio://chunk` + `stt://transcript` event streams.
// Degrades gracefully in a plain browser (vite dev, no Tauri): `available`
// stays false and controls disable, so the console still renders for design.

import { writable } from 'svelte/store';

export const capture = writable({
  available: false, // Tauri backend attached?
  capturing: false,
  level: 0, // latest chunk RMS (0..~1)
  isVoice: false, // VAD gate result for the latest chunk
  devices: [], // [{ name, is_default }]
  stt: { loaded: false, model: null }, // local STT model status
});

// Rolling transcript: `partial` is the in-progress line, `finals` are closed
// utterances (silence-delimited). Kept across capture stop/start.
export const transcript = writable({ partial: '', finals: [] });

const MAX_FINALS = 12;
let unlistenAudio = null;
let unlistenStt = null;

async function invoke() {
  const core = await import('@tauri-apps/api/core'); // throws in a plain browser
  return core.invoke;
}

/** Probe the backend, load devices + STT status. Safe to call on mount. */
export async function initAudio() {
  try {
    const call = await invoke();
    const [devices, stt] = await Promise.all([
      call('list_audio_devices'),
      call('stt_status').catch(() => ({ loaded: false, model: null })),
    ]);
    capture.update((s) => ({ ...s, available: true, devices, stt }));
  } catch {
    capture.update((s) => ({ ...s, available: false }));
  }
}

/** Start capture from `device` (name string, or null for the default input). */
export async function startCapture(device) {
  const call = await invoke();
  const { listen } = await import('@tauri-apps/api/event');
  await call('start_capture', { device: device ?? null });

  unlistenAudio = await listen('audio://chunk', (e) => {
    const { rms, is_voice } = e.payload;
    capture.update((s) => ({ ...s, level: rms, isVoice: is_voice }));
  });
  unlistenStt = await listen('stt://transcript', (e) => {
    const { text, is_final } = e.payload;
    transcript.update((t) => {
      if (is_final) {
        const finals = [...t.finals, text].slice(-MAX_FINALS);
        return { partial: '', finals };
      }
      return { ...t, partial: text };
    });
  });

  capture.update((s) => ({ ...s, capturing: true }));
}

/** Stop capture and detach listeners. Keeps transcript history. Idempotent. */
export async function stopCapture() {
  try {
    const call = await invoke();
    await call('stop_capture');
  } catch {
    /* backend gone — nothing to stop */
  }
  if (unlistenAudio) {
    unlistenAudio();
    unlistenAudio = null;
  }
  if (unlistenStt) {
    unlistenStt();
    unlistenStt = null;
  }
  capture.update((s) => ({ ...s, capturing: false, level: 0, isVoice: false }));
  transcript.update((t) => ({ ...t, partial: '' }));
}
