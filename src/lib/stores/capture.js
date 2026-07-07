// Audio capture + transcript store — bridges the Rust audio/STT engines to the
// UI (Phases 3-4).
//
// Wraps the Tauri commands (list_audio_devices / start_capture / stop_capture /
// stt_status) and the `audio://chunk` + `stt://transcript` event streams.
// Degrades gracefully in a plain browser (vite dev, no Tauri): `available`
// stays false and controls disable, so the console still renders for design.

import { writable, get } from 'svelte/store';

export const capture = writable({
  available: false, // Tauri backend attached?
  capturing: false,
  level: 0, // latest chunk RMS (0..~1)
  isVoice: false, // VAD gate result for the latest chunk
  devices: [], // [{ name, is_default }]
  inputDevice: '', // operator-selected input device name ('' = default). Shared so Console + Settings agree.
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

// True when the operator has blacked out the screens (opaque, not a transparent
// clear). Reset by the next fire/clear. Mirrors the output://black broadcast.
export const screenBlack = writable(false);

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
      await listen('output://content', (e) => { live.set(e.payload); screenBlack.set(false); });
      await listen('output://clear', () => { live.set(null); screenBlack.set(false); });
      await listen('output://black', () => screenBlack.set(true));
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

/** Manual override: fire a free-text reference now (throws if unparseable).
 *  `stageNote` is an optional confidence-monitor note for this cue. */
export async function manualFire(reference, stageNote = null) {
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

/** Blackout every output (opaque). Next fire/clear cancels it. */
export async function blackScreen() {
  try {
    const call = await invoke();
    await call('blackout');
  } catch {
    /* backend absent */
  }
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
