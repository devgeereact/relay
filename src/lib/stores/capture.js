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
  stt: { loaded: false, model: null, language: null }, // local STT model status (language null = auto)
  detectedLang: null, // language of the latest transcript window (code-switching)
  detectionOn: true, // is automatic detection armed?
  audioError: null, // last audio device error (surfaced, not fatal)
  thresholds: { auto_fire: 0.9, suggest: 0.6 }, // router gate (self-calibrating)
});

// What is currently ON the output screens (last fired content, null = cleared).
// Mirrors the `output://content` / `output://clear` broadcast so the console
// previews show what's actually live.
export const live = writable(null);

// Rolling transcript: `partial` is the in-progress line, `finals` are closed
// utterances (silence-delimited). Kept across capture stop/start.
export const transcript = writable({ partial: '', finals: [] });

// PENDING SUGGESTIONS awaiting an operator decision (status 'suggested'),
// de-duplicated by reference. Auto/manual fires do NOT land here — they go
// straight to the screens (see `live`). Keeps the console focused on what needs
// a decision, not a history of recents.
export const detections = writable([]);

// Output templates (Phase 8), loaded from the DB.
export const templates = writable([]);

const MAX_FINALS = 12;
const MAX_DETECTIONS = 6;
let unlistenAudio = null;
let unlistenStt = null;
let unlistenDetect = null;
let outputListenersUp = false; // always-on output mirror (set once)

async function invoke() {
  const core = await import('@tauri-apps/api/core'); // throws in a plain browser
  return core.invoke;
}

/** Probe the backend, load devices + STT status. Safe to call on mount.
 *  Resilient: as long as the Tauri bridge is present, `available` is true —
 *  a single failing command (or the event listeners) never disables the app. */
export async function initAudio() {
  let call;
  try {
    call = await invoke(); // throws only in a plain browser (no Tauri)
  } catch {
    capture.update((s) => ({ ...s, available: false }));
    return;
  }
  // Backend is attached. Load status pieces independently.
  const [devices, stt, thresholds, detectionOn] = await Promise.all([
    call('list_audio_devices').catch(() => []),
    call('stt_status').catch(() => ({ loaded: false, model: null, language: null })),
    call('get_thresholds').catch(() => ({ auto_fire: 0.9, suggest: 0.6 })),
    call('get_detection_enabled').catch(() => true),
  ]);
  capture.update((s) => ({ ...s, available: true, devices, stt, thresholds, detectionOn }));

  // Mirror output state into `live` (set once). A listener failure must NOT
  // disable the app — hence a separate try that leaves `available` alone.
  if (!outputListenersUp) {
    outputListenersUp = true;
    try {
      const { listen } = await import('@tauri-apps/api/event');
      await listen('output://content', (e) => live.set(e.payload));
      await listen('output://clear', () => live.set(null));
      // A device failure (permission denied, unplugged) is non-fatal: surface
      // it and reflect that capture stopped, but never freeze.
      await listen('audio://error', (e) =>
        capture.update((s) => ({ ...s, audioError: e.payload, capturing: false }))
      );
    } catch {
      /* events unavailable — previews just won't mirror; app still works */
    }
  }
}

/** Arm/disarm automatic detection (manual override is unaffected). */
export async function setDetection(enabled) {
  try {
    const call = await invoke();
    const on = await call('set_detection_enabled', { enabled });
    capture.update((s) => ({ ...s, detectionOn: on }));
  } catch {
    /* backend absent */
  }
}

/** Start (or resume) recording a service. Returns its id. */
export async function startService(title, date) {
  const call = await invoke();
  return call('start_service', { title, date });
}

/** Stop recording the current service (history kept). */
export async function endService() {
  try {
    const call = await invoke();
    await call('end_service');
  } catch {
    /* backend absent */
  }
}

/** All recorded services (Library list). */
export async function listServices() {
  try {
    const call = await invoke();
    return await call('list_services');
  } catch {
    return [];
  }
}

/** Transcript + fired detections for one service. */
export async function serviceDetail(id) {
  const call = await invoke();
  return call('service_detail', { id });
}

/** Export a service to a Markdown file. Returns the written path. */
export async function exportService(id) {
  const call = await invoke();
  return call('export_service', { id });
}

/** Start capture from `device` (name string, or null for the default input). */
export async function startCapture(device) {
  const call = await invoke();
  const { listen } = await import('@tauri-apps/api/event');
  // Begin (or resume) recording this service so transcripts + detections persist.
  try {
    await startService('Sunday Service', new Date().toISOString().slice(0, 10));
  } catch {
    /* recording is best-effort — capture proceeds regardless */
  }
  await call('start_capture', { device: device ?? null });

  unlistenAudio = await listen('audio://chunk', (e) => {
    const { rms, is_voice } = e.payload;
    capture.update((s) => ({ ...s, level: rms, isVoice: is_voice }));
  });
  unlistenStt = await listen('stt://transcript', (e) => {
    const { text, is_final, language } = e.payload;
    if (language) capture.update((s) => ({ ...s, detectedLang: language }));
    transcript.update((t) => {
      if (is_final) {
        const finals = [...t.finals, text].slice(-MAX_FINALS);
        return { partial: '', finals };
      }
      return { ...t, partial: text };
    });
  });
  capture.update((s) => ({ ...s, audioError: null }));
  unlistenDetect = await listen('detection://match', (e) => {
    const d = e.payload;
    detections.update((list) => {
      const rest = list.filter((x) => x.reference !== d.reference);
      // Only suggestions queue up; a fired verse resolves (removes) its pending
      // suggestion since it's already on screen.
      if (d.status === 'suggested') {
        return [{ ...d, at: Date.now() }, ...rest].slice(0, MAX_DETECTIONS);
      }
      return rest;
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
  if (unlistenDetect) {
    unlistenDetect();
    unlistenDetect = null;
  }
  capture.update((s) => ({ ...s, capturing: false, level: 0, isVoice: false }));
  transcript.update((t) => ({ ...t, partial: '' }));
}

/** Operator confirms a suggestion → fire it to the screens + nudge the gate. */
export async function confirmDetection(reference) {
  detections.update((list) => list.filter((d) => d.reference !== reference));
  try {
    const call = await invoke();
    const thresholds = await call('confirm_detection', { reference });
    capture.update((s) => ({ ...s, thresholds }));
  } catch {
    /* backend absent */
  }
}

/** Operator dismisses a suggestion → drop it + tighten the gate. */
export async function dismissDetection(reference) {
  detections.update((list) => list.filter((d) => d.reference !== reference));
  try {
    const call = await invoke();
    const thresholds = await call('dismiss_detection');
    capture.update((s) => ({ ...s, thresholds }));
  } catch {
    /* backend absent */
  }
}

/** Manual override: fire a free-text reference now (throws if unparseable). */
export async function manualFire(reference) {
  const call = await invoke();
  await call('manual_fire', { reference });
}

/** Load all output templates from the DB into the store. */
export async function loadTemplates() {
  try {
    const call = await invoke();
    const list = await call('list_templates');
    templates.set(list);
    return list;
  } catch {
    return [];
  }
}

/** Save a template (insert or update). Returns its id; reloads the store. */
export async function saveTemplate(t) {
  const call = await invoke();
  const id = await call('save_template', { template: t });
  await loadTemplates();
  return id;
}

/** All configured output channels. */
export async function listOutputChannels() {
  try {
    const call = await invoke();
    return await call('list_output_channels');
  } catch {
    return [];
  }
}

/** Assign a template to a channel. */
export async function setChannelTemplate(id, templateId) {
  const call = await invoke();
  await call('set_channel_template', { id, templateId });
}

/** Open a native fullscreen output window for a template id. Returns its label. */
export async function openOutput(templateId, name) {
  const call = await invoke(); // throws in browser
  return call('open_output_window', { templateId, name });
}

/** Manual next/previous verse (same as spoken "next"/"back"). */
export async function navVerse(direction) {
  try {
    const call = await invoke();
    await call('nav', { direction });
  } catch {
    /* backend absent */
  }
}

/** Blank every output channel (operator "Clear all screens" / Esc). */
export async function clearScreens() {
  try {
    const call = await invoke();
    await call('clear_screens');
  } catch {
    /* backend absent */
  }
}

/** Set STT language: a code ("yo"/"sw"/"ha"/"en") or null for auto-detect. */
export async function setSttLanguage(language) {
  try {
    const call = await invoke();
    await call('set_stt_language', { language: language ?? null });
    capture.update((s) => ({ ...s, stt: { ...s.stt, language: language ?? null } }));
  } catch {
    /* backend absent */
  }
}

/** Manual threshold override (Settings sliders). */
export async function setThresholds(auto_fire, suggest) {
  try {
    const call = await invoke();
    const thresholds = await call('set_thresholds', { thresholds: { auto_fire, suggest } });
    capture.update((s) => ({ ...s, thresholds }));
  } catch {
    /* backend absent */
  }
}
