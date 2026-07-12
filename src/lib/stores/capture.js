// Audio capture + transcript store — bridges the Rust audio/STT engines to the
// UI (Phases 3-4).
//
// Wraps the Tauri commands (list_audio_devices / start_capture / stop_capture /
// stt_status) and the `audio://chunk` + `stt://transcript` event streams.
// Degrades gracefully in a plain browser (vite dev, no Tauri): `available`
// stays false and controls disable, so the console still renders for design.

import { writable, derived, get } from 'svelte/store';
import { parseTemplateOverride } from '../templates.js';

/**
 * The audio meter — RMS level + voice-activity, arriving 10–50 times a second.
 *
 * Deliberately its OWN store, not fields on `capture`. When these lived on the
 * `capture` mega-object, every audio frame notified every `$capture` subscriber
 * in the app: `App.svelte` reads `$capture.detectionOn` to draw one dot in the
 * sidebar and was re-rendering the entire shell dozens of times a second, for
 * data it does not use. Only the Settings meter subscribes here.
 */
export const meter = writable({ level: 0, isVoice: false });

export const capture = writable({
  available: false, // Tauri backend attached?
  capturing: false,
  devices: [], // [{ name, is_default }]
  inputDevice: '', // operator-selected input device name ('' = default). Shared so Console + Settings agree.
  stt: { loaded: false, model: null, language: null }, // local STT model status (language null = auto)
  detectedLang: null, // language of the latest transcript window (code-switching)
  detectionOn: true, // is automatic detection armed?
  audioError: null, // last audio device error (surfaced, not fatal)
  outputError: null, // LAN output server failed to bind (OBS/kiosk/stage are dead)
  quality: null, // AudioQuality from dsp.rs: { input_rms, clip_ratio, snr_db, denoise, warning }
  // Router gate (self-calibrating). Placeholder only — the real values arrive
  // from `get_thresholds` on init. Kept in step with Thresholds::default() in
  // router.rs, which IS from_sensitivity(50); it used to say 0.9/0.6, which was
  // the other, contradictory baseline.
  thresholds: { auto_fire: 0.5, suggest: 0.35 },
});

// What is currently ON the output screens (last fired content, null = cleared).
// Mirrors the `output://content` / `output://clear` broadcast so the console
// previews show what's actually live.
export const live = writable(null);

// True when the operator has blacked out the screens (opaque, not a transparent
// clear). Reset by the next fire/clear. Mirrors the output://black broadcast.
export const screenBlack = writable(false);

// The last SPOKEN next/back that did nothing, and why (a NavResult). The console
// consumes it, shows it, and clears it. Null when there is nothing to say.
export const navBlocked = writable(null);

/**
 * REHEARSAL — the operator is practising, and nothing reaches the congregation.
 *
 * Backed by Rust (channels.rs) rather than by a flag in this file, because the
 * sandbox has to hold at the point content leaves the machine, not at the point a
 * button was clicked. This store only MIRRORS it, for the UI.
 *
 * It has to be impossible to be wrong about. Both mistakes are bad and they are
 * bad in opposite directions: rehearsing while you think you're live means the
 * projector stays blank all through the sermon; being live while you think you're
 * rehearsing means your practice run is on the wall in front of everyone. So the
 * app says so, loudly and constantly, whenever it is on.
 */
export const rehearsing = writable(false);

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

/**
 * THE PLAYHEAD — where the operator is in the service plan, and whether that is
 * what the congregation is actually looking at.
 *
 * Two separate facts, and conflating them causes real damage in both directions:
 *
 *   { cueId, slide }  the position. SURVIVES everything. It is where → resumes
 *                     from. Wiping it on Esc would mean the next → restarts the
 *                     plan at cue 1 — putting the opening countdown back on the
 *                     wall at the end of the service.
 *
 *   onAir             is plan content on the screens RIGHT NOW. Cleared the moment
 *                     anything else takes the screen (a cleared screen, a blackout,
 *                     a manual fire, an accepted AI suggestion).
 *
 * `onAir` is what the transport mode reads. With a plan cue live, → steps the
 * plan; once the preacher goes off-script and the operator accepts a suggested
 * verse, → walks that passage instead — and Esc hands the transport back to the
 * plan, at the position it was already at.
 *
 * This lives in the store, not in a view, because EVERY path that takes plan
 * content off the screen has to clear `onAir`, and a view will eventually forget.
 * One did: only the Planner's own ◼ button reset it, and the panic keys — which
 * are owned by the app shell — did not.
 */
export const liveCue = writable({ cueId: null, slide: 0, onAir: false });

/** Plan content is no longer what the congregation is looking at. Keeps the position. */
function leavePlan() {
  liveCue.update((c) => (c.onAir ? { ...c, onAir: false } : c));
}

// Narrow slices of `capture`. A component that only needs one flag should
// subscribe to one flag — `derived` only notifies when the value it selects
// actually changes, so the app shell no longer re-renders because a device list
// was refreshed, or an error banner was cleared.
export const capturing = derived(capture, ($c) => $c.capturing);
export const detectionOn = derived(capture, ($c) => $c.detectionOn);
export const backendUp = derived(capture, ($c) => $c.available);

/**
 * What's on the screens right now, shaped for `TemplateRender`.
 *
 * Derived once here rather than re-derived in each view. Console, Planner and
 * Output.svelte each had their own copy of this reshape AND of the
 * `template_json` parse below — three chances for the console preview to stop
 * agreeing with what the congregation is actually seeing, which is the one thing
 * the preview exists to guarantee.
 */
export const liveContent = derived(live, ($l) =>
  $l
    ? {
        reference: $l.reference,
        text: $l.text,
        translation: $l.translation,
        media_url: $l.media_url,
        media_kind: $l.media_kind,
        countdown_to: $l.countdown_to,
        countdown_done: $l.countdown_done,
      }
    : null,
);

/**
 * The per-content-type template override riding on the live content (lyrics
 * render as lyrics, scripture as scripture), or null to use the channel's own
 * template. Malformed JSON falls back to the channel template rather than
 * throwing — a bad template must never take the screens down mid-service.
 */
export const liveTemplateOverride = derived(live, ($l) =>
  parseTemplateOverride($l?.template_json),
);

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
    call('get_thresholds').catch(() => ({ auto_fire: 0.5, suggest: 0.35 })),
    call('get_detection_enabled').catch(() => true),
  ]);
  capture.update((s) => ({ ...s, available: true, devices, stt, thresholds, detectionOn }));

  // Mirror output state into `live` (set once). A listener failure must NOT
  // disable the app — hence a separate try that leaves `available` alone.
  if (!outputListenersUp) {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      await listen('output://content', (e) => { live.set(e.payload); screenBlack.set(false); });
      await listen('output://clear', () => { live.set(null); screenBlack.set(false); });
      await listen('output://black', () => screenBlack.set(true));
      // A SPOKEN "next"/"back" that did nothing. The STT thread has no caller to
      // return a NavResult to, so it pushes it here — the preacher says "next", the
      // wall does not move, and the console explains why instead of staying silent.
      await listen('nav://blocked', (e) => navBlocked.set(e.payload));
      // A clear that failed on a path with nobody to return an error to — the
      // spoken "clear the screen", and the exit from rehearsal (which hands the
      // wall back to the congregation). Same banner as a failed key or button.
      await listen('output://panic_failed', (e) => panicError.set(String(e.payload)));
      await listen('rehearsal://changed', (e) => rehearsing.set(e.payload === true));
      // A device failure (permission denied, unplugged) is non-fatal: surface
      // it and reflect that capture stopped, but never freeze.
      await listen('audio://error', (e) =>
        capture.update((s) => ({ ...s, audioError: e.payload, capturing: false }))
      );
      // A LAN server failed to bind → every networked output (OBS, kiosk
      // screens, the stage monitor) is dead. This used to be swallowed to
      // stderr, so the operator's only symptom was screens that never came up.
      await listen('output://error', (e) =>
        capture.update((s) => ({ ...s, outputError: e.payload }))
      );
      // Audio-quality telemetry (clipping / mic muted / too noisy). dsp.rs has
      // computed and emitted this all along and NOTHING was listening — so the
      // one signal that tells an operator "your mic is muted" was dead. It is
      // data, never a rendering decision: the console shows it, the pipeline
      // keeps running regardless.
      await listen('audio://quality', (e) =>
        capture.update((s) => ({ ...s, quality: e.payload }))
      );
      // Only NOW are they actually up. Setting this before registration meant a
      // single failed `listen` latched the flag forever: the listeners were never
      // registered and never retried, so the console silently stopped mirroring
      // what was on the screens for the rest of the session.
      outputListenersUp = true;
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

/** Read rehearsal state from the backend (which owns it). */
export async function loadRehearsal() {
  try {
    const call = await invoke();
    rehearsing.set((await call('get_rehearsal')) === true);
  } catch {
    /* backend absent */
  }
}

/**
 * Enter or leave rehearsal.
 *
 * This THROWS on refusal, and the caller must show the message. Rust refuses to
 * rehearse while a service is being recorded, and refuses to record a service while
 * rehearsing — and a refusal that is swallowed into a `catch {}` (as most wrappers
 * here do) would leave the operator believing they had flipped a switch that had
 * not moved. That is the one thing this feature cannot afford.
 */
export async function setRehearsal(on) {
  const call = await invoke();
  await call('set_rehearsal', { on });
  rehearsing.set(on);
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

/** Set the shared input device (name, or '' for default). Used by Console + Settings. */
export function setInputDevice(name) {
  capture.update((s) => ({ ...s, inputDevice: name || '' }));
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

  // The hot path. Goes to `meter`, never to `capture` — see the note on `meter`.
  unlistenAudio = await listen('audio://chunk', (e) => {
    const { rms, is_voice } = e.payload;
    meter.set({ level: rms, isVoice: is_voice });
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
  // Accepting an AI suggestion also takes us out of the plan — same reason as
  // manualFire.
  leavePlan();
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

/** Manual override: fire a free-text reference now (throws if unparseable).
 *  `stageNote` is an optional confidence-monitor note for this cue. */
export async function manualFire(reference, stageNote = null) {
  // A hand-typed verse is not a plan cue. If the arrows still thought we were in
  // the plan, the next → would jump back to a slide the congregation has moved on
  // from.
  leavePlan();
  const call = await invoke();
  await call('manual_fire', { reference, stageNote });
}

// ── Service Planner ──────────────────────────────────────────────────────────
// Plans are ordered lists of cues of any content type. Scripture is the first
// wired type: search the bundled corpus, add a verse as a cue. All calls degrade
// to no-ops / empty in a plain browser (no Tauri), so the sketch still renders.

/** Search the bundled Bible — reference ("john 3:16", "ps 23") or free text. */
export async function searchScripture(query) {
  try {
    const call = await invoke();
    return await call('search_scripture', { query });
  } catch {
    return [];
  }
}

/** All service plans, newest first. */
export async function listPlans() {
  try {
    const call = await invoke();
    return await call('list_plans');
  } catch {
    return [];
  }
}

/** Create a plan; returns its id. */
export async function createPlan(title, date) {
  const call = await invoke();
  return await call('create_plan', { title, date });
}

/** Delete a plan and its cues. */
export async function deletePlan(id) {
  const call = await invoke();
  await call('delete_plan', { id });
}

/** Duplicate a plan (with all its cues). Returns the new plan id. */
export async function duplicatePlan(id, title) {
  const call = await invoke();
  return await call('duplicate_plan', {
    id,
    title,
    date: new Date().toISOString().slice(0, 10),
  });
}

/** Ordered cues of a plan. */
export async function planItems(planId) {
  try {
    const call = await invoke();
    return await call('plan_items', { planId });
  } catch {
    return [];
  }
}

/** Append a cue of any type. `payload` is serialized to JSON here. */
export async function addPlanItem(planId, cueType, label, payload, templateId = null) {
  const call = await invoke();
  return await call('add_plan_item', {
    planId,
    cueType,
    label,
    payloadJson: JSON.stringify(payload ?? {}),
    templateId,
  });
}

/** Remove a cue. */
export async function removePlanItem(id) {
  const call = await invoke();
  await call('remove_plan_item', { id });
}

/** Reorder a cue: direction -1 (up) / +1 (down). */
export async function movePlanItem(id, direction) {
  const call = await invoke();
  await call('move_plan_item', { id, direction });
}

/** Set/clear a cue's operator stage note (confidence-monitor only; blank clears). */
export async function setPlanNote(id, note) {
  const call = await invoke();
  await call('set_plan_note', { id, note: note ?? '' });
}

/** Apply a drag-reorder: the full new order of cue ids. */
export async function reorderPlan(planId, ids) {
  const call = await invoke();
  await call('reorder_plan', { planId, ids });
}

// ── Songs (Lyrics) ───────────────────────────────────────────────────────────

/** All songs with section counts. */
export async function listSongs() {
  try {
    const call = await invoke();
    return await call('list_songs');
  } catch {
    return [];
  }
}

/** Search songs by title/author (empty query = all). */
export async function searchSongs(query) {
  try {
    const call = await invoke();
    return await call('search_songs', { query });
  } catch {
    return [];
  }
}

/** A full song with ordered sections. */
export async function getSong(id) {
  const call = await invoke();
  return await call('get_song', { id });
}

/** Import a song from pasted lyrics; the backend parses sections. Returns id. */
export async function importSong({ title, author, ccli, key, bpm, lyrics }) {
  const call = await invoke();
  return await call('import_song', {
    title,
    author: author ?? '',
    ccli: ccli ?? '',
    songKey: key ?? '',
    bpm: bpm ?? null,
    lyrics,
    date: new Date().toISOString().slice(0, 10),
  });
}

/** Save edits to a song — metadata + full ordered section list. */
export async function saveSong(song) {
  const call = await invoke();
  await call('save_song', {
    id: song.id,
    title: song.title,
    author: song.author ?? '',
    ccli: song.ccli ?? '',
    songKey: song.song_key ?? '',
    bpm: song.bpm ?? null,
    sections: song.sections.map((s) => ({ tag: s.tag, label: s.label, lyrics: s.lyrics })),
  });
}

/** Delete a song. */
export async function deleteSong(id) {
  const call = await invoke();
  await call('delete_song', { id });
}

/** Arrangements: named play-orders of a song's sections (ProPresenter-style).
 *  A sequence is a list of 0-based section indices; repeats are allowed. */
export async function listArrangements(songId) {
  const call = await invoke();
  return await call('list_arrangements', { songId });
}

/** Create (id null) or update (id set) an arrangement. Returns its id. */
export async function saveArrangement(songId, id, name, sequence) {
  const call = await invoke();
  return await call('save_arrangement', { songId, id: id ?? null, name, sequence });
}

export async function deleteArrangement(id) {
  const call = await invoke();
  await call('delete_arrangement', { id });
}

/** Expand a song's sections into a play order. `sequence` is a list of 0-based
 *  section indices (an arrangement); repeats allowed, out-of-range dropped.
 *  Empty/no sequence = the sections verbatim (the implicit "Standard" order). */
export function expandSections(sections, sequence) {
  if (!Array.isArray(sequence) || sequence.length === 0) return sections;
  return sequence.map((i) => sections[i]).filter(Boolean);
}

/** Import songs from a ProPresenter file. `dataB64` is the file's bytes, base64
 *  encoded by the webview (a .proplaylist yields many songs). Returns titles. */
export async function importProFile(filename, dataB64) {
  const call = await invoke();
  return await call('import_pro', {
    filename,
    data: dataB64,
    date: new Date().toISOString().slice(0, 10),
  });
}

/** True while a countdown is live on the outputs (its target is still in the
 *  future). Derived from the mirrored output content, so it clears the moment
 *  the screen is cleared or any other content goes live. */
export function countdownRunning() {
  const l = get(live);
  return !!(l && l.countdown_to && l.countdown_to > Date.now());
}

/** Start a pre-service countdown on every output. Outputs tick MM:SS locally
 *  from the broadcast target; `label` shows above, `doneMsg` replaces it at 0.
 *  Guarded: refuses to start a second countdown while one is still running —
 *  clear the screen (or let it finish) first. */
export async function startCountdown(minutes, label = 'Service begins in', doneMsg = 'Welcome') {
  if (countdownRunning()) {
    throw new Error('A countdown is already running — clear the screen to start a new one.');
  }
  const call = await invoke();
  await call('start_countdown', { minutes, label, doneMsg });
}

/** Fire arbitrary content to the screens. `kind` ('song'|'announce') selects the
 *  content-type default template (per-content-type templates). `stageNote` is an
 *  optional confidence-monitor note for this cue. */
export async function fireContent(label, text, kind = 'announce', stageNote = null) {
  const call = await invoke();
  await call('fire_content', { label, text, kind, stageNote });
}

/** The content-type → template default mapping. */
export async function getContentTemplates() {
  try {
    const call = await invoke();
    return await call('get_content_templates');
  } catch {
    return { scripture: null, song: null, media: null, announce: null };
  }
}
/** Map a content type to a template (null clears → channel default). */
export async function setContentTemplate(kind, templateId) {
  const call = await invoke();
  await call('set_content_template', { kind, templateId });
}

// ── Saved scripture (Library → Scripture) ────────────────────────────────────

export async function listSavedScripture() {
  try {
    const call = await invoke();
    return await call('list_saved_scripture');
  } catch {
    return [];
  }
}
export async function saveScripture(book, chapter, verse) {
  const call = await invoke();
  return await call('save_scripture', {
    book,
    chapter,
    verse,
    date: new Date().toISOString().slice(0, 10),
  });
}
export async function deleteSavedScripture(id) {
  const call = await invoke();
  await call('delete_saved_scripture', { id });
}

// ── Announcements (Library → Announcements) ──────────────────────────────────
export async function listAnnouncements() {
  try {
    const call = await invoke();
    return await call('list_announcements');
  } catch {
    return [];
  }
}
/** Create (id null) or update an announcement. Returns its id. */
export async function saveAnnouncement(id, title, body) {
  const call = await invoke();
  return await call('save_announcement', {
    id: id ?? null,
    title,
    body,
    date: new Date().toISOString().slice(0, 10),
  });
}
export async function deleteAnnouncement(id) {
  const call = await invoke();
  await call('delete_announcement', { id });
}

// ── Media (Library → Media) ──────────────────────────────────────────────────

export async function listMedia() {
  try {
    const call = await invoke();
    return await call('list_media');
  } catch {
    return [];
  }
}
export async function importMedia(kind, filename, dataB64) {
  const call = await invoke();
  return await call('import_media', {
    kind,
    filename,
    data: dataB64,
    date: new Date().toISOString().slice(0, 10),
  });
}
export async function deleteMedia(id) {
  const call = await invoke();
  await call('delete_media', { id });
}
/** Fire a media asset (image/video) to the output screens as a background. */
export async function fireMedia(id) {
  const call = await invoke();
  await call('fire_media', { id });
}

/** Parse a lyric file into songs WITHOUT saving — for the pre-save review. */
export async function parseImport(filename, dataB64) {
  const call = await invoke();
  return await call('parse_import', { filename, data: dataB64 });
}
/** Commit reviewed/edited songs to the library. Returns { added, replaced }. */
export async function saveReviewedSongs(songs) {
  const call = await invoke();
  return await call('save_reviewed_songs', { songs, date: new Date().toISOString().slice(0, 10) });
}

// Read a File (from an <input type=file>) as base64 for the import commands.
export async function fileToBase64(file) {
  const buf = new Uint8Array(await file.arrayBuffer());
  let bin = '';
  const chunk = 0x8000;
  for (let i = 0; i < buf.length; i += chunk) {
    bin += String.fromCharCode.apply(null, buf.subarray(i, i + chunk));
  }
  return btoa(bin);
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

/** The active templates (max 4) previewed on the console Output grid. */
export async function listActiveTemplates() {
  try {
    const call = await invoke();
    return await call('list_active_templates');
  } catch {
    return [];
  }
}

/** Activate/deactivate a template on the console (throws past the 4 cap). */
export async function setTemplateActive(id, active) {
  const call = await invoke();
  await call('set_template_active', { id, active });
  await loadTemplates();
}

/** Create a new template; returns its id and reloads the store. */
export async function createTemplate(name) {
  const call = await invoke();
  const id = await call('create_template', { name });
  await loadTemplates();
  return id;
}

/** Delete a template; reloads the store. */
export async function deleteTemplate(id) {
  const call = await invoke();
  await call('delete_template', { id });
  await loadTemplates();
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
export async function openOutput(templateId, name, monitorIndex) {
  const call = await invoke(); // throws in browser
  return call('open_output_window', { templateId, name, monitorIndex });
}

/** Connected physical displays for HDMI screen assignment. */
export async function listMonitors() {
  try {
    const call = await invoke();
    return await call('list_monitors');
  } catch {
    return [];
  }
}

/** This machine's LAN IP so output URLs work on other devices. Null if offline. */
export async function localIp() {
  try {
    const call = await invoke();
    return await call('local_ip');
  } catch {
    return null;
  }
}

/** Bible translations available in the corpus. */
export async function listTranslations() {
  try {
    const call = await invoke();
    return await call('list_translations');
  } catch {
    return [];
  }
}

/** Currently active translation id (null if none). */
export async function getActiveTranslation() {
  try {
    const call = await invoke();
    return await call('get_active_translation');
  } catch {
    return null;
  }
}

/** Choose which translation to read from — every lookup then prefers it. */
export async function setActiveTranslation(id) {
  const call = await invoke();
  await call('set_active_translation', { id });
}

/** Open a channel's output on its assigned display (HDMI). Returns the label. */
export async function openChannelOutput(channelId) {
  const call = await invoke(); // throws in browser
  return call('open_channel_output', { channelId });
}

/** Assign a physical display (monitor index string, or null) to a channel. */
export async function setChannelDisplay(id, display) {
  const call = await invoke();
  await call('set_channel_display', { id, display });
}

/** Add a new output channel. Returns its id. */
export async function addChannel(name, renderTarget, templateId) {
  const call = await invoke();
  return call('add_channel', { name, renderTarget, templateId });
}

/** Delete an output channel. */
export async function deleteChannel(id) {
  const call = await invoke();
  await call('delete_channel', { id });
}

/**
 * Manual next/previous verse (same as spoken "next"/"back").
 *
 * Returns a NavResult — `{kind}` is one of `fired` / `end_of_passage` /
 * `no_passage` / `not_in_library`. The caller MUST tell the operator which.
 *
 * This used to return nothing and swallow every error. `nav` was a `()` command
 * wrapping a `()` function with three silent bail-outs inside it, so the operator
 * pressed Next mid-sermon, the wall did not change, and there was no error, no toast
 * and no log — on the key they press more than any other. Same silent-no-op class as
 * the "Screens cleared" lie (docs/DECISIONS.md §20).
 */
export async function navVerse(direction) {
  const call = await invoke();
  return call('nav', { direction });
}

/**
 * Turn a NavResult into the sentence the operator gets.
 *
 * Not every outcome is a failure, and flattening them is what hid this bug for
 * months: reaching the end of a passage is a normal boundary and the operator just
 * needs to know that is why nothing moved. A verse missing from the corpus is a real
 * fault. `null` means it worked and the screens changed — the wall is the feedback.
 */
export function navNotice(r) {
  switch (r?.kind) {
    case 'fired':
      return null;
    case 'end_of_passage':
      return 'End of the passage — nothing further to show.';
    case 'no_passage':
      return 'No passage on screen yet. Fire a verse first, then use ← →.';
    case 'not_in_library':
      return `${r.reference} isn't in your Bible, so the screen was left as it is.`;
    default:
      return null;
  }
}

/** Blank every output channel (operator "Clear all screens" / Esc). */
export async function clearScreens() {
  return panicRun('clear_screens', 'Clear screens');
}

/** Blackout every output (opaque). Next fire/clear cancels it. Returns true on success. */
export async function blackScreen() {
  return panicRun('blackout', 'Blackout');
}

/**
 * A panic control (clear / blackout) FAILED, and the congregation may still be
 * looking at whatever was on the wall. Null when the last one worked.
 *
 * This is a STORE and not a thrown error on purpose. The panic controls are fired
 * from places that cannot catch: a global keydown handler, and a button on a shell
 * that must keep working even when the current view has crashed. A `throw` there is
 * an unhandled rejection in the console — which is to say, silence.
 */
export const panicError = writable(null);

/**
 * Run a panic control and tell the truth about whether it worked.
 *
 * Both of these used to swallow every error into a `catch {}` and return void, so
 * `clearScreens()` resolved identically whether it had cleared the wall or not —
 * and Live.svelte flashed "Screens cleared" on the strength of that. The operator
 * was told the screens were clean while the verse was still up. In live software
 * that is the worst class of bug there is: the operator stops looking at the screen
 * and starts trusting the toast.
 *
 * Returns true on success. Callers that report success to the operator MUST check it.
 */
async function panicRun(cmd, label) {
  // Reset the transport FIRST, so it happens even if the backend call fails. A
  // panic key that half-works is worse than one that doesn't.
  leavePlan();
  try {
    const call = await invoke();
    await call(cmd);
    panicError.set(null);
    return true;
  } catch (e) {
    // In a plain browser there is no backend AND no output screen, so there is
    // nothing to warn about — don't cry wolf in a dev tab.
    if (get(capture).available) {
      panicError.set(
        `${label} FAILED — the congregation may still be seeing the last thing you put up. ` +
          `Check the output screen and clear it there. (${String(e).replace(/^Error:\s*/, '')})`,
      );
    }
    return false;
  }
}

/** Operator has read the panic warning (or a later panic control succeeded). */
export function dismissPanicError() {
  panicError.set(null);
}

/** Push the "up next" preview to the stage/confidence monitor (null clears). */
export async function setStageNext(label, text) {
  try {
    const call = await invoke();
    await call('set_stage_next', { label: label ?? null, text: text ?? null });
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

/**
 * Crash reporting (opt-in, off by default).
 *
 * These are the ONLY two calls in this file that can cause anything to leave the
 * device. Turning it on is an explicit operator action, and even then the Rust
 * side scrubs every crash report of transcript, verse, lyric and announcement
 * text before it is sent (see src-tauri/src/telemetry.rs).
 */
export async function getCrashReporting() {
  try {
    const call = await invoke();
    return await call('get_crash_reporting');
  } catch {
    return { enabled: false, dsn: '' };
  }
}

export async function setCrashReporting(enabled, dsn) {
  const call = await invoke();
  return await call('set_crash_reporting', { enabled, dsn: dsn ?? '' });
}

// ── Speech model acquisition ────────────────────────────────────────────────
//
// The single most important flow in the product for a new user. Until this
// existed, turning the AI on meant opening a terminal and running `curl` to
// fetch a 148 MB file into a folder that doesn't exist in a packaged app — so
// for an actual church volunteer, Relay's whole reason to exist silently did
// not work.

/** { id, downloaded, total } while a model download is in flight, else null. */
export const modelProgress = writable(null);
/** Last download error, in plain language. */
export const modelError = writable(null);

/** The catalogue, with `installed` resolved for this machine. */
export async function listModels() {
  try {
    const call = await invoke();
    return await call('list_models');
  } catch {
    return [];
  }
}

/**
 * Download a model. Resolves when it is installed AND speech recognition has
 * been brought up — no restart. Rejects with a sentence a volunteer can act on.
 */
export async function downloadModel(id) {
  const call = await invoke();
  const { listen } = await import('@tauri-apps/api/event');

  modelError.set(null);
  modelProgress.set({ id, downloaded: 0, total: 0 });

  let cancelled = false;
  const stop = [
    await listen('model://progress', (e) => modelProgress.set(e.payload)),
    await listen('model://error', (e) => modelError.set(e.payload)),
    // Cancelling is something the operator CHOSE. It used to come down the error
    // channel, so stopping your own download painted a red failure box — which had
    // no dismiss, so it sat there until the component remounted.
    await listen('model://cancelled', () => {
      cancelled = true;
      modelError.set(null);
    }),
  ];
  try {
    await call('download_model', { id });
    // A cancelled download resolves normally — the operator got what they asked
    // for. But there is no model on disk, so loading it would fail and report an
    // error for something that is not one.
    if (cancelled) return false;
    // Bring STT up in-place. A 148 MB download that ends in "now quit and
    // reopen the app" is a miserable last step for a first-time user.
    const loaded = await call('load_stt_model');
    const stt = await call('stt_status');
    capture.update((s) => ({ ...s, stt }));
    return loaded;
  } finally {
    stop.forEach((fn) => fn());
    modelProgress.set(null);
  }
}

/** Operator has read the download error. */
export function dismissModelError() {
  modelError.set(null);
}

export async function cancelModelDownload() {
  try {
    const call = await invoke();
    await call('cancel_model_download');
  } catch {
    /* nothing running */
  }
}
