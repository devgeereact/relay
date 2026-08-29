// Audio capture + transcript store — bridges the Rust audio/STT engines to the
// UI (Phases 3-4).
//
// Wraps the Tauri commands (list_audio_devices / start_capture / stop_capture /
// stt_status) and the `audio://chunk` + `stt://transcript` event streams.
// Degrades gracefully in a plain browser (vite dev, no Tauri): `available`
// stays false and controls disable, so the console still renders for design.
//
// ── THE THROW-vs-SWALLOW CONTRACT ───────────────────────────────────────────────
//
// This file is the ONLY place the frontend talks to Rust, and for months it had no
// contract at all: ~34 `catch {}` blocks against exactly ONE `throw` in the whole of
// src/. Half the wrappers swallowed and returned `[]`, half threw, and a caller could
// not tell which — so a button could quietly do nothing, forever, with no error and
// no log. That is how `clearScreens` came to flash "Screens cleared" over a wall that
// still had scripture on it.
//
// The rule is ONE question: **can the congregation see the difference?**
//
//   GROUP 1 — THROWS. Anything that changes what is on the screens, what the AI is
//   allowed to do, or whether the microphone is live. `manualFire`, `confirmDetection`,
//   `setDetection`, `setRehearsal`, `navVerse`, `startCapture`, `stopCapture`,
//   `fireContent`, `startCountdown`. The caller MUST handle it and tell the operator.
//   A silent failure here is a lie told to someone standing in front of a congregation.
//
//   GROUP 2 — SWALLOWS, and returns a safe default. Reads: `listPlans`, `listSongs`,
//   `listMonitors`, `searchScripture`, `loadTemplates`, … A backend that is absent
//   (a plain browser) or a list that fails to load costs the operator nothing they
//   cannot see for themselves — the list is visibly empty. Nothing on any screen
//   changes, so nothing is being hidden from them.
//
//   GROUP 3 — REPORTS VIA A STORE, never throws. The panic controls (`clearScreens`,
//   `blackScreen`): they are fired from a global keydown handler and from a shell
//   button that must survive a crashed view, and NEITHER CAN CATCH. A throw there is
//   an unhandled rejection — silence with extra steps. They return a boolean and set
//   `panicError`, so a failure surfaces however the control was triggered.
//
// If you add a wrapper, put it in a group deliberately. "It seemed fine" is how a
// panic key came to do nothing.

import { writable, derived, get } from 'svelte/store';
import { parseTemplateOverride } from '../templates.js';
import { tNow } from '../i18n.js';
import { humanError } from '../errors.js';
import { markTranscript } from '../latency.js';

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
  // Auto-detect is not settling on a language — [codes] once per session, else null.
  // Whisper re-elects a language every window from ~99 candidates, and on accented
  // speech it wanders (one real service: en·yo·pt·sw·sv·ms). The label IS the decode,
  // so a wandering label degrades the transcript — and that looks exactly like the AI
  // being bad. The operator has the control that fixes it (Settings → Recognition
  // Language) and no reason to suspect they should touch it. See stt.rs.
  langUnstable: null,
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
// `finalsAt[i]` is the arrival wall-clock of `finals[i]`. Stamped here, at the
// source, and sliced in lockstep with `finals` — so a consumer can never drift
// the two apart (the length-based alignment in Live did, once the rolling cap
// froze `finals.length` at MAX_FINALS and every new line shifted the array left).
export const transcript = writable({ partial: '', finals: [], finalsAt: [] });

/**
 * THE TRANSCRIPT REDUCER — the one place the rolling-transcript rule lives.
 *
 * Extracted from inside the `stt://transcript` listener on 2026-08-15 because it
 * was untestable there, and it was the ONLY live surface in the app with no test
 * of any kind: a grep for `transcript.set|transcript.update|$transcript` across
 * every `*.test.js` returned zero.
 *
 * That matters more than a coverage number. The run rail's own comment says this
 * panel is "the difference between 'the preacher has not said a reference' and
 * 'Relay has gone deaf', and those need opposite responses" — so an operator reads
 * it to decide whether to intervene. Four things have to hold, and none of them was
 * pinned:
 *
 *   1. A FINAL clears the partial. Otherwise the half-heard fragment that became
 *      the final sits underneath it, and the operator reads the tail twice.
 *   2. `finals` and `finalsAt` are sliced in LOCKSTEP. They are stamped here, at
 *      the source, precisely because a consumer that aligned them by length once
 *      drifted them — the rolling cap froze `finals.length` and every new line
 *      shifted the array left, so every timestamp labelled the wrong line.
 *   3. A partial NEVER appends to finals. It is one line being revised, not a new
 *      one, and whisper revises the same utterance several times a second.
 *   4. The cap is a WINDOW ON THE NEWEST. A sermon is an hour long; the panel shows
 *      the last few lines and the array must not grow without bound.
 *
 * Pure on purpose: `at` is passed in rather than read from the clock, so the
 * ordering and alignment can be asserted deterministically.
 */
export function applyTranscript(t, { text, is_final }, at) {
  if (!is_final) return { ...t, partial: text };
  return {
    partial: '',
    finals: [...t.finals, text].slice(-MAX_FINALS),
    // `?? []` because a session restored from an older build has no `finalsAt`,
    // and a missing timestamp must degrade to an unlabelled line, never to a crash
    // on the surface an operator is watching to decide whether Relay has gone deaf.
    finalsAt: [...(t.finalsAt ?? []), at].slice(-MAX_FINALS),
  };
}


// PENDING SUGGESTIONS awaiting an operator decision (status 'suggested'),
// de-duplicated by reference. Auto/manual fires do NOT land here — they go
// straight to the screens (see `live`). Keeps the console focused on what needs
// a decision, not a history of recents.
export const detections = writable([]);

// Output templates (Phase 8), loaded from the DB.
export const templates = writable([]);

// CUSTOM themes (the style layer beneath templates — see lib/themes.js). Builtin
// themes live in the frontend (themes.js); the operator's own themes are
// persisted as a JSON blob in the settings KV under THEMES_KEY. The store holds
// ONLY the custom ones — surfaces concatenate BUILTIN_THEMES + $customThemes.
export const customThemes = writable([]);

// Planned service length in MINUTES (0 = no target). Drives a monitor's REMAINING
// timer. Persisted in the settings KV and read by the backend at start_service.
export const serviceTargetMinutes = writable(0);

// THE default template — the one every slide wears unless a screen or a content
// look overrides it. Replaces the old "console-active (max 4)" star: a template
// is not limited to four, and any template can be a screen's output; this is just
// the single fallback look. Persisted in the settings KV (`default_template_id`).
export const defaultTemplateId = writable(null);

/** Load the default-template id into the store (null if unset). */
export async function loadDefaultTemplate() {
  return guardedRead(
    'loadDefaultTemplate',
    async (call) => {
      const n = parseInt(await call('get_setting', { key: 'default_template_id' }), 10);
      defaultTemplateId.set(Number.isFinite(n) ? n : null);
      return n;
    },
    // The old catch also did `defaultTemplateId.set(null)`. A fallback VALUE cannot
    // carry a side effect, so the reset is explicit — without it a failed read left
    // the store holding the last good id and every surface resolved a template the
    // backend could no longer confirm.
    null,
    () => defaultTemplateId.set(null),
  );
}

/** Set (or clear, with null) the default template. */
export async function setDefaultTemplate(id) {
  const call = await invoke();
  await call('set_setting', { key: 'default_template_id', value: id == null ? '' : String(id) });
  defaultTemplateId.set(id ?? null);
}

/**
 * THE content-type → template default map, as a LIVE store (Decision §25).
 *
 * A "content look" answers: when the AI fires scripture (or a song, media,
 * announcement, countdown), which template does it wear on any screen that has
 * not overridden it. It is a wiring fact, not template decoration.
 *
 * This used to be read straight from the backend by three separate surfaces
 * (Settings › Outputs, the Templates editor, the gallery), each holding its own
 * cached copy with no shared state — so they silently disagreed and overwrote
 * one another. Now there is ONE store and ONE writer (`setContentTemplate`); the
 * Content-looks matrix in the Outputs hub edits it, everything else subscribes
 * and is read-only. Keys are the canonical CONTENT_KINDS (see lib/layers.js).
 */
export const EMPTY_CONTENT_LOOKS = {
  scripture: null,
  song: null,
  media: null,
  announce: null,
  countdown: null,
};
export const contentTemplates = writable({ ...EMPTY_CONTENT_LOOKS });

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

/** Whether the live override is a PINNED cue choice (overrides the screen) vs a
 *  content-type default (defers to the screen's own template). Mirrors the exact
 *  resolution the real output window uses, so the console program pane matches. */
export const liveTemplatePinned = derived(live, ($l) => !!$l?.template_pinned);

const MAX_FINALS = 12;
const MAX_DETECTIONS = 6;

/**
 * How long a pending suggestion stays actionable, in ms.
 *
 * A suggestion is a claim about what the preacher is saying RIGHT NOW. Forty-five
 * seconds later they have moved on, and accepting it puts the wrong thing on the
 * wall — so an old card is not merely clutter, it is a trap sitting under the `A`
 * key. In one live service the queue held six of these at once, all stale, while
 * the one that mattered scrolled out of view.
 *
 * Comfortably outlives the router's repeat cooldown (WINDOW_SECS + 2 = 10s), so
 * the operator always gets a real chance to read and decide.
 */
export const SUGGESTION_TTL_MS = 45_000;

/**
 * Drop suggestions that have gone stale. Pure — takes `now` so it is testable
 * without a clock, and so the whole list shares one timestamp.
 */
export function pruneStaleSuggestions(list, now) {
  return list.filter((d) => now - (d.at ?? 0) < SUGGESTION_TTL_MS);
}
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

  // Seed the shared content-look map ONCE at boot so every surface (the Outputs
  // hub matrix, the gallery "Default for" badges, the live preview) reads one
  // source of truth. Best-effort — a failure leaves the empty default, never
  // disables the app.
  loadContentTemplates();

  // Mirror output state into `live` (set once). A listener failure must NOT
  // disable the app — hence a separate try that leaves `available` alone.
  if (!outputListenersUp) {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      await listen('output://content', (e) => { live.set(e.payload); screenBlack.set(false); });
      // `leavePlan()` HERE, not only in the wrappers — this is the half no wrapper
      // can reach. A clear that did not originate in this console still takes plan
      // content off the wall: `/api/clear` from the preacher's phone, the spoken
      // "clear the screen", and the exit from a rehearsal all reach
      // `channels::clear` directly, and the console's only report of them is this
      // event. It set `live` and `screenBlack` and nothing else, so the plan rail
      // went on drawing amber "On Air" over a wall the congregation had stopped
      // looking at — while the topbar, reading `$live`, simultaneously said the
      // screens were clear. Two indicators in one window, disagreeing, and amber
      // is never allowed to be the wrong one (CLAUDE.md §18).
      await listen('output://clear', () => { live.set(null); screenBlack.set(false); leavePlan(); });
      await listen('output://black', () => { screenBlack.set(true); leavePlan(); });
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
      // Auto-detect is wandering between languages. Same contract as `quality`:
      // it is DATA, never a rendering decision — the pipeline keeps running, the
      // console just stops being silent about the one thing the operator can fix.
      await listen('stt://language_unstable', (e) =>
        capture.update((s) => ({ ...s, langUnstable: e.payload }))
      );
      // A template was edited → make the console mirror change LIVE, without
      // waiting for a re-fire. Two things can be showing it: the console's
      // preview/program panes resolve their template from the reactive `$templates`
      // store (so reloading it updates them), and the content currently on screen
      // may carry that template as a content-type/cue OVERRIDE snapshot — refresh
      // that snapshot so the program pane re-renders the live verse at once.
      await listen('template://updated', async (e) => {
        const id = e.payload;
        await loadTemplates();
        const cur = get(live);
        const ov = cur && parseTemplateOverride(cur.template_json);
        if (ov && ov.id === id) {
          try {
            const fresh = await call('get_template', { id });
            if (fresh) live.set({ ...cur, template_json: JSON.stringify(fresh) });
          } catch {
            /* backend hiccup — next fire will carry the fresh template anyway */
          }
        }
      });
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

/**
 * Arm/disarm automatic detection (manual override is unaffected).
 *
 * THROWS (contract group 1). The store is updated from the value the BACKEND returns,
 * never from what we asked for — and a failure must not leave the dot saying "off"
 * while the AI is still armed and firing verses at the congregation.
 */
export async function setDetection(enabled) {
  const call = await invoke();
  const on = await call('set_detection_enabled', { enabled });
  capture.update((s) => ({ ...s, detectionOn: on }));
}

/** Read rehearsal state from the backend (which owns it). */
export async function loadRehearsal() {
  // Neither of the last two reads feeds a list with an empty state, so neither was
  // part of R3-04's finding. They are routed anyway: a rule that covers 20 of 22
  // reads is the shape this repo keeps shipping bugs in, and "which reads are
  // guarded?" should not be a question anybody has to look up.
  return guardedRead(
    'loadRehearsal',
    async (call) => rehearsing.set((await call('get_rehearsal')) === true),
    undefined,
  );
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
  const id = await call('start_service', { title, date });
  // Recording arms the lock in Rust. Read it back rather than assuming: an
  // assumption here would show PROTECTED over a console that is not protecting
  // anything, which is the same class of lie as a badge that cannot be wrong.
  await loadServiceLock();
  return id;
}

/** Stop recording the current service (history kept). */
export async function endService() {
  try {
    const call = await invoke();
    await call('end_service');
  } catch {
    /* backend absent */
  }
  await loadServiceLock();
}

// ── SERVICE LOCK ──────────────────────────────────────────────────────────────
//
// While a service is being recorded, Relay holds back a short list of actions that
// are irreversible or that take the speech engine away mid-sermon. The list lives
// in Rust (`servicelock::PROTECTED`) and rides here with the flag — a second copy
// in the frontend would be a second answer to one question, and the two would drift.
//
// Nothing on the fire path is affected, and the operator can lift it in one action:
// it exists to catch an ACCIDENT, not to overrule the person standing in the room.
export const serviceLock = writable({ engaged: false, held_back: [] });

/** Ask Rust whether a service is being protected. Never throws; a status readout
 *  that can take the console down is worse than no status readout. */
export async function loadServiceLock() {
  const v = await guardedRead('serviceLock', (call) => call('service_lock'), {
    engaged: false,
    held_back: [],
  });
  serviceLock.set(v ?? { engaged: false, held_back: [] });
  return v;
}

/**
 * The operator lifts (or re-applies) the lock. GROUP 1 — THROWS.
 *
 * A failed unlock that reported success would leave a volunteer pressing a button
 * that keeps refusing, with the UI insisting it is now unlocked. The store is set
 * from the value RUST returns, never from what was asked for.
 */
export async function setServiceLock(on) {
  const call = await invoke();
  const engaged = await call('set_service_lock', { on: !!on });
  serviceLock.set({ ...get(serviceLock), engaged: !!engaged });
  return !!engaged;
}

/**
 * Everything that happened in one service, in order — the replay's spine.
 *
 * Merged in Rust from three tables, each row saying which it came from: a
 * `detection` is what the AI claimed, a `cue` is what the operator pressed, an
 * `event` is what Relay observed about itself. Read-only history, so it degrades
 * to an empty list rather than taking the Library down.
 */
export async function serviceTimeline(id) {
  return guardedRead('serviceTimeline', (call) => call('service_timeline', { id }), []);
}

/** The latency snapshots kept for one service. Percentiles only, never traces. */
export async function servicePerf(id) {
  return guardedRead('servicePerf', (call) => call('service_perf', { id }), []);
}

/**
 * Write the diagnostic bundle and return where it landed. GROUP 1 — THROWS.
 *
 * An export that silently failed would leave an operator hunting a Downloads folder
 * for a file that was never written, while a support conversation waits on it.
 */
export async function exportDiagnostics() {
  const call = await invoke();
  return call('export_diagnostics');
}

/**
 * The state of Relay's African-language support, measured from the shipped data.
 *
 * Read-only, so it swallows: a language report that could take Settings down would
 * be worse than no language report. `wer` is always null and `native_reviewed`
 * always false — both are absences, and the UI must render them as such.
 */
export async function languageReport() {
  return guardedRead('languageReport', (call) => call('language_report'), []);
}

// ── ROOMS (RG-10) ────────────────────────────────────────────────────────────
//
// A church that runs in the main hall on Sunday and the youth room on Wednesday
// rebuilds the same configuration twice a week — and the microphone choice is not
// persisted anywhere at all today, so it is gone every time Relay closes.
//
// Reads swallow (a room list that takes the Settings screen down is worse than no
// room list); writes THROW, because an operator who is told their room was saved
// and finds it gone next Sunday has been lied to about the one thing this feature
// promises.
export const rooms = writable([]);

export async function loadRooms() {
  const list = await guardedRead('rooms', (call) => call('list_environments'), []);
  rooms.set(list ?? []);
  return list ?? [];
}

/** GROUP 1 — THROWS. */
export async function saveRoom(name, settings, notes = '') {
  const call = await invoke();
  const id = await call('save_environment', {
    name,
    settingsJson: JSON.stringify(settings ?? {}),
    notes,
  });
  await loadRooms();
  return id;
}

/** Switch to a room and get its settings back. GROUP 1 — THROWS. */
export async function useRoom(id) {
  const call = await invoke();
  const room = await call('use_environment', { id });
  await loadRooms();
  return room;
}

/** GROUP 1 — THROWS. */
export async function deleteRoom(id) {
  const call = await invoke();
  await call('delete_environment', { id });
  await loadRooms();
}

/** All recorded services (Library list). */
export async function listServices() {
  return guardedRead('listServices', (call) => call('list_services'), []);
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

// Last language pushed to `capture` — guards against re-notifying subscribers
// on every transcript when the detected language hasn't changed.
let lastLang = null;
// The hot path. Goes to `meter`, never to `capture` — see the note on `meter`.
unlistenAudio = await listen('audio://chunk', (e) => {
  const { rms, is_voice } = e.payload;
  meter.set({ level: rms, isVoice: is_voice });
});
unlistenStt = await listen('stt://transcript', (e) => {
  const { text, is_final, language, trace_id } = e.payload;
  // Only touch `capture` when the detected language actually CHANGES. A Svelte
  // writable notifies every subscriber on every `set`, so updating it on each
  // transcript event re-rendered the whole app shell several times a second for
  // a value almost nothing reads — the same churn the `meter` split-out fixed.
  if (language && language !== lastLang) {
    lastLang = language;
    capture.update((s) => ({ ...s, detectedLang: language }));
  }
  const at = new Date().toLocaleTimeString('en-GB');
  transcript.update((t) => applyTranscript(t, { text, is_final }, at));
  // Tell Rust when this actually reached the operator's eyes. Everything before
  // this point the backend can time itself; the webview's own share of the delay
  // is only visible from inside the webview. Never throws — see lib/latency.js.
  markTranscript(trace_id);
  // Expire stale suggestions here too, not only when a NEW one arrives. The
  // preacher moving on quietly is the commonest way a card goes stale, and it
  // produces no detection event at all — so without this a dead suggestion sat
  // under the `A` key indefinitely. Transcript events tick about once a second
  // while listening, which is all the resolution this needs and costs no timer.
  detections.update((list) => {
    const fresh = pruneStaleSuggestions(list, Date.now());
    return fresh.length === list.length ? list : fresh; // no-op → no re-render
  });
});
capture.update((s) => ({ ...s, audioError: null }));
unlistenDetect = await listen('detection://match', (e) => {
  const d = e.payload;
  detections.update((list) => {
    const now = Date.now();
    // Sweep the stale ones out on the way past. A new suggestion is the moment
    // the operator's attention moves, so it is exactly the moment the previous
    // sentence's leftovers stop being offers and start being traps.
    const rest = pruneStaleSuggestions(list, now).filter(
      (x) => x.reference !== d.reference
    );
    // Only suggestions queue up; a fired verse resolves (removes) its pending
    // suggestion since it's already on screen.
    if (d.status === 'suggested') {
      return [{ ...d, at: now }, ...rest].slice(0, MAX_DETECTIONS);
    }
    return rest;
  });
});

capture.update((s) => ({ ...s, capturing: true }));
}

/**
 * Stop capture and detach listeners. Keeps transcript history. Idempotent.
 *
 * THROWS (contract group 1). This changes whether the microphone is live, which is
 * the group's own definition, and it used to swallow — one bare `catch {}` around
 * both the bridge import AND the command.
 *
 * The comment on that catch said "backend gone — nothing to stop", which is true of
 * exactly one case: a plain browser, where `invoke()` fails to import and there was
 * never an engine. It was ALSO catching a real `stop_capture` failure — and
 * `stop_capture` can fail: it takes a lock, so an audio thread that panicked while
 * holding it leaves the mutex poisoned and the engine running. The frontend then
 * detached its listeners, set `capturing: false`, and every caller's
 * `catch (e) { flash(humanError(e)) }` never ran. The operator read "Start
 * listening" on a live microphone with detection still auto-firing behind it —
 * rule 15, from the other end: a control reporting a success it did not achieve.
 *
 * So a failed stop leaves the UI saying `capturing` and rethrows. Nothing is torn
 * down, because nothing stopped, and the operator can press it again.
 */
export async function stopCapture() {
let call = null;
try {
  call = await invoke();
} catch {
  /* no Tauri bridge at all (a plain browser) — there is no engine to stop */
}
// Deliberately NOT in a try: a rejection must reach the caller, and must reach it
// before any local teardown claims the microphone is off.
if (call) await call('stop_capture');

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
capture.update((s) => ({ ...s, capturing: false }));
// The live level lives on the `meter` store, not `capture` — resetting
// capture.level/isVoice (which nothing reads) left the input bars frozen lit at
// the last value after Stop. Reset the store that actually drives them.
meter.set({ level: 0, isVoice: false });
transcript.update((t) => ({ ...t, partial: '' }));
}

/**
 * Operator confirms a suggestion → fire it to the screens + nudge the gate.
 *
 * THROWS (contract group 1). This puts scripture in front of a congregation.
 *
 * And note the ORDER. It used to drop the suggestion from the list and call
 * `leavePlan()` FIRST, then swallow any failure — so a fire that never happened still
 * removed the suggestion from the operator's screen. They pressed A, the card
 * vanished, and nothing went up. Everything now happens only once the backend has
 * confirmed the verse is actually live.
 */
export async function confirmDetection(reference) {
const call = await invoke();
const thresholds = await call('confirm_detection', { reference });
// Accepting an AI suggestion also takes us out of the plan — same reason as
// manualFire.
leavePlan();
detections.update((list) => list.filter((d) => d.reference !== reference));
capture.update((s) => ({ ...s, thresholds }));
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
 *  `stageNote` is an optional confidence-monitor note for this cue.
 *  `keepPlan` — when a PLAN slide is being fired (the operator stepping the plan
 *  in Slide mode), the transport must STAY on the plan. Without this, firing a
 *  scripture plan cue ran `leavePlan()` below and flipped the transport out of
 *  Slide mode into Verse mode — so the very next → walked the passage instead of
 *  advancing the plan. Songs/media/countdown never hit this (they don't fire
 *  through `manual_fire`); only scripture cues did, which is exactly what made
 *  Slide mode "break" on a scripture item. Hand-typed fires keep the default. */
export async function manualFire(reference, stageNote = null, templateId = null, keepPlan = false) {
const call = await invoke();
await call('manual_fire', { reference, stageNote, templateId });
if (keepPlan) return; // a plan slide fire — stay on the plan (Slide mode holds)
// A hand-typed verse is not a plan cue. If the arrows still thought we were in
// the plan, the next → would jump back to a slide the congregation has moved on
// from.
//
// AFTER the call, not before. The transport must follow what is ACTUALLY on the
// wall. If the fire failed — an unparseable reference, a verse outside the corpus —
// then nothing changed, the plan slide is still up there, and taking the plan "off
// air" would leave `→` walking a verse passage that the congregation cannot see,
// firing content they did not ask for. Nothing moved, so nothing here moves either.
//
// (The panic controls are the deliberate exception: `clearScreens`/`blackScreen`
// reset the cursor FIRST, because a panic key that half-works is worse than one
// that does not work at all — and they now report their own failure loudly.)
leavePlan();
}

// ── Service Planner ──────────────────────────────────────────────────────────
// Plans are ordered lists of cues of any content type. Scripture is the first
// wired type: search the bundled corpus, add a verse as a cue. All calls degrade
// to no-ops / empty in a plain browser (no Tauri), so the sketch still renders.

/** Search the bundled Bible — reference ("john 3:16", "ps 23") or free text. */
export async function searchScripture(query) {
return guardedRead('searchScripture', async (call) => {
    return await call('search_scripture', { query });
}, []);
}

/**
 * Topical cross-references for what is being preached right now.
 *
 * `{ theme, refs: [...] }`, or null when no theme is clearly indicated — which is most
 * of the time, and is the correct answer. Group 2 (swallows): this is an OFFER, not a
 * detection. If it fails, the operator loses nothing they can see.
 *
 * The backend for this (19 themes, keyword-scored, a registered Tauri command) has
 * existed and been fully tested for months with ZERO frontend callers. Built code that
 * nothing calls rots: it drifts out of step with the payloads around it and nobody
 * finds out, because nothing exercises it.
 */
export async function relatedScripture(text, exclude = null) {
try {
  const call = await invoke();
  return await call('related_scripture', { text, exclude });
} catch {
  return null;
}
}

/** All service plans, newest first. */
export async function listPlans() {
return guardedRead('listPlans', async (call) => {
    return await call('list_plans');
}, []);
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

/** Begin a section at this cue. A blank title merges it into the section above. */
export async function setPlanSection(id, title) {
const call = await invoke();
await call('set_plan_section', { id, title: title ?? '' });
}

/** Set a cue's planned length in seconds. 0 = untimed (fires on cue, not a clock). */
export async function setPlanDuration(id, seconds) {
const call = await invoke();
await call('set_plan_duration', { id, seconds });
}

/** Override the template a cue renders with. `null` re-inherits the channel's. */
export async function setPlanTemplate(id, templateId) {
const call = await invoke();
await call('set_plan_template', { id, templateId: templateId ?? null });
}

// ── Songs (Lyrics) ───────────────────────────────────────────────────────────

/** All songs with section counts. */
export async function listSongs() {
return guardedRead('listSongs', async (call) => {
    return await call('list_songs');
}, []);
}

/** Search songs by title/author (empty query = all). */
export async function searchSongs(query) {
return guardedRead('searchSongs', async (call) => {
    return await call('search_songs', { query });
}, []);
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
export async function startCountdown(
minutes,
label = 'Service begins in',
doneMsg = 'Welcome',
templateId = null,
keepPlan = false,
) {
if (countdownRunning()) {
  throw new Error('A countdown is already running — clear the screen to start a new one.');
}
const call = await invoke();
await call('start_countdown', { minutes, label, doneMsg, templateId });
if (!keepPlan) leavePlan();
}

/** Fire arbitrary content to the screens. `kind` ('song'|'announce') selects the
 *  content-type default template (per-content-type templates). `stageNote` is an
 *  optional confidence-monitor note for this cue. `templateId`, when set, is the
 *  cue's OWN template override (Planner) — it wins over the content-type default. */
export async function fireContent(
label,
text,
kind = 'announce',
stageNote = null,
templateId = null,
keepPlan = false,
) {
const call = await invoke();
await call('fire_content', { label, text, kind, stageNote, templateId });
if (!keepPlan) leavePlan();
}

/**
 * Load the content-type → template default map from the DB into the shared
 * `contentTemplates` store. Call once at boot; surfaces then read the store.
 */
export async function loadContentTemplates() {
return guardedRead('loadContentTemplates', async (call) => {
    const map = await call('get_content_templates');
    contentTemplates.set({ ...EMPTY_CONTENT_LOOKS, ...map });
    return map;
}, null);
}

/** The content-type → template default mapping (one-shot read; also seeds the
 *  store). Prefer subscribing to `contentTemplates`. */
export async function getContentTemplates() {
const map = await loadContentTemplates();
return map ?? { ...EMPTY_CONTENT_LOOKS };
}

/**
 * THE ONE writer of the content-look map (Decision §25). Maps a content type to
 * a template (null clears → the screen's own template). Updates the shared store
 * optimistically so every subscribed surface reflects the change instantly, then
 * persists; on failure it reloads truth so the UI can never lie about what is
 * stored. Do NOT add a second writer of this map.
 */
export async function setContentTemplate(kind, templateId) {
const id = templateId ?? null;
contentTemplates.update((m) => ({ ...m, [kind]: id }));
try {
  const call = await invoke();
  await call('set_content_template', { kind, templateId: id });
} catch (e) {
  await loadContentTemplates();
  throw e;
}
}

// ── Saved scripture (Library → Scripture) ────────────────────────────────────

export async function listSavedScripture() {
return guardedRead('listSavedScripture', async (call) => {
    return await call('list_saved_scripture');
}, []);
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
return guardedRead('listAnnouncements', async (call) => {
    return await call('list_announcements');
}, []);
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

/** EMERGENCY announcement — over whatever is on the wall, on every channel.
 *
 *  GROUP 1 (throws). This is the most literal "the congregation can see the
 *  difference" there is: it is used for a fire alarm or a blocked car park, and
 *  it paints over live scripture on every screen at once. A silent failure means
 *  the operator believes the room has been told something it has not been told.
 *
 *  Distinct from `saveAnnouncement`, which is Library CONTENT planned in advance.
 *  This one does not touch the library and is not part of any plan. */
export async function pushAnnouncement(message) {
const call = await invoke();
await call('push_announcement', { message });
// No `keepPlan`: the emergency announcement is never a plan cue, and it covers
// every screen. If the plan rail stayed amber under it, the one indicator that
// says what a congregation is looking at would be naming a cue nobody can see.
leavePlan();
}

/** How many times this verse already went out in the CURRENT service.
 *
 *  GROUP 2 (swallows, returns 0). A "shown earlier" badge is an affordance, not a
 *  control: if it fails the operator sees a verse without a badge, which is what
 *  they saw before the badge existed. Nothing on any screen changes, so nothing
 *  is being hidden. 0 also means "not recording a service", which reads the same. */
export async function verseRepeatCount(reference) {
return guardedRead('verseRepeatCount', async (call) => {
    return (await call('verse_repeat_count', { reference })) ?? 0;
}, 0);
}

// ── Voice profiles (Settings → Voice Profiles) ───────────────────────────────
//
// Per-preacher accent + gate calibration: the STT language hint, the decoder-bias
// vocabulary, the operator's sensitivity dial, and the thresholds the router has
// LEARNED. SPEC.md §4.6.
//
// The reads swallow (GROUP 2) — an empty list is visibly empty. The writes THROW
// (GROUP 1): selecting or editing a profile changes the STT language and the gate
// thresholds, i.e. what the AI is allowed to put on a screen without asking. A
// selection that silently failed would leave the operator calibrated for the wrong
// preacher, and nothing on screen would say so.

export async function listVoiceProfiles() {
return guardedRead('listVoiceProfiles', async (call) => {
    return (await call('list_voice_profiles')) ?? [];
}, []);
}

export async function activeVoiceProfile() {
try {
  const call = await invoke();
  return (await call('active_voice_profile')) ?? null;
} catch {
  return null;
}
}

export async function createVoiceProfile(name, language = null) {
const call = await invoke();
return await call('create_voice_profile', { name, language });
}

export async function updateVoiceProfile(profile) {
const call = await invoke();
return await call('update_voice_profile', { profile });
}

export async function selectVoiceProfile(id) {
const call = await invoke();
return await call('select_voice_profile', { id });
}

export async function deleteVoiceProfile(id) {
const call = await invoke();
return await call('delete_voice_profile', { id });
}

// ── Media (Library → Media) ──────────────────────────────────────────────────

export async function listMedia() {
return guardedRead('listMedia', async (call) => {
    return await call('list_media');
}, []);
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
/** Fire a media asset (image/video) to the output screens as a background.
 *  `templateId`, when set, is the cue's own Planner template override. */
export async function fireMedia(id, templateId = null, keepPlan = false) {
const call = await invoke();
await call('fire_media', { id, templateId });
if (!keepPlan) leavePlan();
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
return guardedRead('loadTemplates', async (call) => {
    const list = await call('list_templates');
    templates.set(list);
    return list;
}, []);
}

/**
 * THE LATENCY REPORT — where the time went, this session.
 *
 * GROUP 2 (swallows). It is a diagnostic: a backend that is absent or a command
 * that fails costs the operator nothing they can see, and the panel renders an
 * honest "no data" instead of throwing on the Settings screen.
 */
export async function latencyReport(recent = 12) {
  return guardedRead('latencyReport', (call) => call('latency_report', { recent }), null);
}

/** Start a clean measurement run — for a field test that wants THIS service. */
export async function latencyReset() {
  return guardedRead('latencyReset', (call) => call('latency_reset'), null);
}

/**
 * Turn measurement on or off.
 *
 * Returns the state that is ACTUALLY in force, as Rust reports it — not the state
 * that was asked for. A toggle that flips itself on a failed call tells the
 * operator the instrument is running when it is not, which is the same class of
 * lie as "Screens cleared" over a live verse, in a much smaller room.
 */
export async function latencySetEnabled(on) {
  return guardedRead('latencySetEnabled', (call) => call('latency_set_enabled', { on }), null);
}

/** Save a template (insert or update). Returns its id; reloads the store. */
export async function saveTemplate(t) {
const call = await invoke();
const id = await call('save_template', { template: t });
await loadTemplates();
return id;
}

/** Save without reloading the store — for bulk operations that reload once at
 *  the end (e.g. the one-time legacy→layers upgrade). */
export async function saveTemplateQuiet(t) {
const call = await invoke();
return call('save_template', { template: t });
}

// ── TEMPLATE VERSION HISTORY ─────────────────────────────────────────────────
// Snapshots persisted per template in the settings KV under `tplver.<id>`. The
// list/trim/dedup logic is pure (templates.js); these wrappers own persistence.
const versionsKey = (id) => `tplver.${id}`;

/** Snapshot a template into its version history (deduped, bounded). Best-effort —
 *  a failure to record history must never block the save it follows. `at` is the
 *  timestamp (Date.now() by default; the app may use it, unlike workflow code). */
export async function snapshotTemplateVersion(template, at = Date.now()) {
if (!template?.id) return;
try {
  const call = await invoke();
  const { parseTemplateVersions, appendTemplateVersion } = await import('../templates.js');
  const raw = await call('get_setting', { key: versionsKey(template.id) });
  const next = appendTemplateVersion(parseTemplateVersions(raw), template, at);
  await call('set_setting', { key: versionsKey(template.id), value: JSON.stringify(next) });
} catch {
  /* history is a convenience — never surface or block on its failure */
}
}

/** The saved versions of a template, newest first. [] if none/unavailable. */
export async function listTemplateVersions(id) {
return guardedRead('listTemplateVersions', async (call) => {
    const { parseTemplateVersions } = await import('../templates.js');
    return parseTemplateVersions(await call('get_setting', { key: versionsKey(id) }));
}, []);
}

/** Restore a template to a saved version's shape (a normal save, so it live-
 *  updates outputs like any edit). Returns the saved id. */
export async function restoreTemplateVersion(template, version) {
return saveTemplate({ ...template, layout: version.layout, style: version.style });
}

/** Download a template as a portable `.relaytemplate.json` file. Pure client-side
 *  (Blob + transient anchor), so it needs no backend. */
export async function exportTemplate(t) {
const { serializeTemplate } = await import('../templates.js');
const safeName = String(t?.name ?? 'template').replace(/[^a-z0-9]+/gi, '-').toLowerCase();
const blob = new Blob([serializeTemplate(t)], { type: 'application/json' });
const url = URL.createObjectURL(blob);
const a = document.createElement('a');
a.href = url;
a.download = `${safeName}.relaytemplate.json`;
document.body.appendChild(a);
a.click();
a.remove();
URL.revokeObjectURL(url);
}

/** Read a picked template file, validate it, and save it as a NEW template.
 *  Returns the new id. Throws a plain-language Error (parseImportedTemplate) the
 *  caller shows through the ONE humaniser. */
export async function importTemplateFromFile(file) {
const { parseImportedTemplate } = await import('../templates.js');
const text = await file.text();
const t = parseImportedTemplate(text); // throws on a non-template file
return saveTemplate(t); // fresh id (no id in the payload), reloads the store
}

/** The DEFAULT template object (by `defaultTemplateId`), else the first template,
 *  else null. The single fallback look — the console preview and the Library
 *  previews all resolve their stand-in template from this. */
export async function resolveDefaultTemplate() {
await loadDefaultTemplate();
let list = get(templates);
if (!list.length) list = await loadTemplates();
const id = get(defaultTemplateId);
return list.find((t) => t.id === id) || list[0] || null;
}

/** Back-compat shim: the old "console-active (max 4)" concept is gone. Callers
 *  that used the first active template as a preview/fallback now get the single
 *  DEFAULT template, so nothing has to know the star system was removed. */
export async function listActiveTemplates() {
const t = await resolveDefaultTemplate().catch(() => null);
return t ? [t] : [];
}

/** Create a new template; returns its id and reloads the store. */
export async function createTemplate(name) {
const call = await invoke();
const id = await call('create_template', { name });
await loadTemplates();
return id;
}

/** Delete a template; reloads the store. Also refreshes the content-look map —
 *  a content default (or a channel) may have pointed at the deleted template and
 *  the backend nulls those references. */
export async function deleteTemplate(id) {
const call = await invoke();
await call('delete_template', { id });
await loadTemplates();
await loadContentTemplates();
}

// ── THEMES ───────────────────────────────────────────────────────────────────
// Custom themes persist as ONE JSON blob in the settings KV. This deliberately
// reuses the generic get_setting/set_setting commands rather than adding a
// themes table + five commands: a theme is small, edited rarely, and always read
// as a whole set. Builtins never touch persistence — they live in themes.js.
const THEMES_KEY = 'themes.custom';

/** Load the operator's custom themes into the store. Degrades to [] (builtins
 *  only) if the backend has no get_setting yet or the blob is corrupt — a bad
 *  themes blob must never break boot. Mirrors loadTemplates' resilience. */
export async function loadThemes() {
return guardedRead(
    'loadThemes',
    async (call) => {
      const raw = await call('get_setting', { key: THEMES_KEY });
      const { parseThemes } = await import('../themes.js');
      const list = parseThemes(raw);
      customThemes.set(list);
      return list;
    },
    [],
    () => customThemes.set([]),
  );
}

/** Persist the whole custom-theme set (the store IS the source of truth here),
 *  then push it to any connected kiosk/OBS client so a browser source resolves a
 *  custom-themed template live. The kiosk sync is best-effort — a failure to
 *  reach the hub must never block saving a theme locally. */
async function persistThemes(list) {
  const call = await invoke();
  const value = JSON.stringify(list);
  await call('set_setting', { key: THEMES_KEY, value });
  try {
    await call('sync_kiosk_themes', { themesJson: value });
  } catch {
    /* no hub / no clients — the blob is saved; kiosks get it on next connect */
  }
}

/**
 * Insert or update a custom theme; returns its id. A theme with no id (or a
 * builtin's negative id) is treated as NEW and gets a fresh positive id, so
 * "duplicate a builtin" always creates rather than trying to overwrite a
 * read-only builtin. Ids are max+1 (never Date-based — the app forbids it).
 */
export async function saveTheme(theme) {
  const list = get(customThemes);
  const isExisting = typeof theme.id === 'number' && theme.id > 0 && list.some((t) => t.id === theme.id);
  let next;
  if (isExisting) {
    next = list.map((t) => (t.id === theme.id ? { ...theme, builtin: false } : t));
  } else {
    const id = list.reduce((m, t) => Math.max(m, t.id), 0) + 1;
    next = [...list, { ...theme, id, builtin: false }];
    theme = { ...theme, id };
  }
  await persistThemes(next);
  customThemes.set(next);
  return theme.id;
}

/** Load the configured service length (minutes) into the store. Degrades to 0
 *  (no target) if the backend/setting is absent. */
export async function loadServiceTarget() {
  return guardedRead(
    'loadServiceTarget',
    async (call) => {
      const raw = await call('get_setting', { key: 'service.target_minutes' });
      const n = parseInt(raw, 10);
      serviceTargetMinutes.set(Number.isFinite(n) && n > 0 ? n : 0);
    },
    undefined,
    () => serviceTargetMinutes.set(0),
  );
}

/** Set the service length (minutes; 0 clears the target). Persisted in the KV,
 *  read by the backend when the next service starts. */
export async function setServiceTarget(minutes) {
  const n = Math.max(0, Math.min(600, Math.floor(Number(minutes) || 0)));
  const call = await invoke();
  await call('set_setting', { key: 'service.target_minutes', value: String(n) });
  serviceTargetMinutes.set(n);
}

/** Delete a custom theme by id. Builtins (negative ids) are not stored, so this
 *  is a no-op for them by construction. */
export async function deleteTheme(id) {
  const next = get(customThemes).filter((t) => t.id !== id);
  await persistThemes(next);
  customThemes.set(next);
}

/** Download a theme (builtin or custom) as a portable `.relaytheme.json` file.
 *  Pure client-side — a Blob + a transient anchor click, so it needs no backend
 *  and works the same in the app webview and a plain browser. */
export async function exportTheme(theme) {
  const { serializeTheme } = await import('../themes.js');
  const safeName = String(theme?.name ?? 'theme').replace(/[^a-z0-9]+/gi, '-').toLowerCase();
  const blob = new Blob([serializeTheme(theme)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `${safeName}.relaytheme.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

/** Read a picked theme file, validate it, and save it as a NEW custom theme.
 *  Returns the new id. Throws a plain-language Error (via parseImportedTheme) the
 *  caller shows through the ONE humaniser — a bad file must never blank the UI. */
export async function importThemeFromFile(file) {
  const { parseImportedTheme } = await import('../themes.js');
  const text = await file.text();
  const theme = parseImportedTheme(text); // throws on a non-theme file
  return saveTheme(theme); // fresh positive id, persisted, pushed to kiosks
}

/** Labels of the output windows that are actually OPEN right now. */
export async function listOutputWindows() {
  return guardedRead('listOutputWindows', (call) => call('list_output_windows'), []);
}

/** All configured output channels. */
export async function listOutputChannels() {
return guardedRead('listOutputChannels', async (call) => {
    return await call('list_output_channels');
}, []);
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
return guardedRead('listMonitors', async (call) => {
    return await call('list_monitors');
}, []);
}

/** Books available to browse, in canonical order (Library §7). */
export async function listBooks() {
return guardedRead('listBooks', async (call) => {
    return await call('list_books');
}, []);
}

/** One chapter's verses, in order. */
export async function chapterVerses(book, chapter) {
try {
  const call = await invoke();
  return await call('chapter_verses', { book, chapter });
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
return guardedRead('listTranslations', async (call) => {
    return await call('list_translations');
}, []);
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

/** Re-open the physical output windows assigned to a display, so HDMI/projector
 *  screens restore themselves after a launch/update/rebuild. Backend only opens
 *  onto connected, non-primary displays, so it never covers the operator's
 *  console. Best-effort — a plain browser (no backend) just no-ops. */
export async function autoOpenOutputs() {
try {
  const call = await invoke();
  return await call('auto_open_outputs');
} catch {
  return [];
}
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

// ── OUTPUT HEALTH, POLLED ONCE FOR THE WHOLE APP ─────────────────────────────
//
// `channel_status` is a poll, not a push: nothing raises an event when a browser
// source connects or a window dies. Three surfaces want the answer — the Live run
// pane, the Outputs table, and the shell's degraded banner (which has to be right
// on every tab, because a volunteer may well be in Settings when a screen dies).
//
// One poller, one store. Three timers asking the same question would triple the
// work and let the three surfaces disagree about the same screen for up to two
// seconds, which is the asymmetry RG-01 exists to end.
export const channelHealth = writable({}); // channel id → ChannelLiveness
/** When each channel was first seen attached but not answering. */
export const channelWaiting = writable({});
let healthPoll = null;

async function pollChannelHealth() {
  const rows = await channelStatus();
  const next = {};
  for (const r of rows) next[r.id] = r;
  const now = Date.now();
  channelWaiting.update((w) => {
    const out = { ...w };
    for (const r of rows) {
      const attached = r.supported && r.online;
      if (attached && !r.painting) out[r.id] ??= now;
      else delete out[r.id];
    }
    return out;
  });
  channelHealth.set(next);
}

/**
 * Start polling, at the beat interval, so a screen that stops answering shows up
 * within about three beats. Idempotent: called from the shell, and calling it again
 * must not create a second timer.
 */
export function startChannelHealth() {
  if (healthPoll) return;
  pollChannelHealth();
  healthPoll = setInterval(pollChannelHealth, 2000);
}

export function stopChannelHealth() {
  clearInterval(healthPoll);
  healthPoll = null;
}

/**
 * What is actually live on each channel, right now.
 *
 * Computed by the backend from open output windows and connected kiosk clients —
 * NOT from `output_channels.status`, which is written once at insert and has
 * always read `offline` for every channel. Returns `[]` without a backend rather
 * than throwing: a dead status strip must not take the Channels screen down.
 */
export async function channelStatus() {
try {
  const call = await invoke();
  return await call('channel_status');
} catch {
  return [];
}
}

/** Close a channel's native output window, if it has one open. */
export async function closeChannelOutput(channelId) {
const call = await invoke();
await call('close_channel_output', { channelId });
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
const outcome = await call('nav', { direction });
// Only when it ACTUALLY fired. `NavResult` exists precisely because not every
// outcome moves the wall — EndOfPassage and NotInLibrary leave the screens
// exactly as they were, and clearing `onAir` on those would take the plan off
// air because the operator pressed a key that did nothing.
if (outcome?.kind === 'fired') leavePlan();
return outcome;
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
    return tNow('nav.end_of_passage');
  case 'no_passage':
    return tNow('nav.no_passage');
  case 'not_in_library':
    return tNow('nav.not_in_library', { reference: r.reference });
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
 * WHY A LIST WAS EMPTY — failure, or genuinely nothing.
 *
 * Every read wrapper below is GROUP 2: it swallows and returns a safe default. The
 * rationale written at the top of this file — *"a list that fails to load costs the
 * operator nothing they cannot see for themselves — the list is visibly empty"* —
 * is the sentence that produced the lie. A fresh install ships **five** built-in
 * templates, and with `list_templates` failing the Templates tab said *"No templates
 * yet — create one to start."* An operator told their five templates do not exist is
 * about to make five more.
 *
 * The wrappers still return `[]`, because that is what keeps every caller working and
 * a broken read must never take a view down. What changes is that the reason is no
 * longer thrown away: it is recorded here, keyed by wrapper name, and a view can ask
 * `readErrors` which of the three facts to show — Empty, Loading, or Error. Same
 * shape as `panicError`, for the same reason: the caller is not in a position to act
 * on it, and the person who is looks at a screen.
 *
 * Cleared on the next SUCCESSFUL read of the same key, so a transient failure does
 * not leave a permanent banner.
 */
export const readErrors = writable({});

/** Run a GROUP 2 read, remembering why it failed instead of discarding it. */
async function guardedRead(key, run, fallback) {
try {
  const value = await run(await invoke());
  readErrors.update((m) => (m[key] ? { ...m, [key]: null } : m));
  return value;
} catch (e) {
  readErrors.update((m) => ({ ...m, [key]: e }));
  return fallback;
}
}


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

/** Push the "up next" preview to the stage/confidence monitor (null clears).
 *
 *  GROUP 1 (THROWS), moved out of GROUP 2 on 2026-08-14 (R5-8) — and the reason is
 *  a correction to the group rule itself, not just to this wrapper.
 *
 *  GROUP 2's test is *"can the congregation see the difference?"*. For this call the
 *  honest answer is **no, but the preacher can, and he is the one acting on it.**
 *  The stage monitor is a real screen on a stand in front of a person, and
 *  `setStageNext(null, null)` is how the "up next" panel comes DOWN. A swallowed
 *  failure there leaves a preacher reading a stale next-verse for the rest of the
 *  service with nothing, anywhere, reporting it.
 *
 *  Throwing makes each CALL SITE state its choice, which is the point of having
 *  groups at all: the push after a fire may reasonably shrug (there is nothing to
 *  correct and the wall is unaffected); the clear may not. */
export async function setStageNext(label, text) {
const call = await invoke();
await call('set_stage_next', { label: label ?? null, text: text ?? null });
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

/** The single operator sensitivity dial (0..100), read from the live thresholds.
 *  One forward mapping (`from_sensitivity`) and its inverse both live in Rust —
 *  the frontend never duplicates the curve. */
export async function getSensitivity() {
try {
  const call = await invoke();
  return await call('get_sensitivity');
} catch {
  return 50;
}
}
/** Set sensitivity (0..100). Applies the same thresholds Settings would and keeps
 *  the local `thresholds` mirror in step. Returns the LANDED dial position.
 *
 *  GROUP 1 (THROWS). It used to be GROUP 2 with `catch { return sensitivity; }` —
 *  returning the value the caller ASKED for, as if it had landed, under a doc
 *  comment promising the opposite. `set_sensitivity` really can fail:
 *  `routing.0.lock()?` on a poisoned router mutex, the same failure shape that
 *  produced the `stopCapture` bug.
 *
 *  This is the third wrapper repaired for the rule behind DECISIONS §20 — after
 *  `clearScreens` and `stopCapture` — and it is on the one control that governs
 *  **what the AI may put on a wall without asking**. An operator who drags the dial
 *  to 80 over a gate still sitting at 50 has been told the machine is more cautious,
 *  or more eager, than it is, and there is nothing on any screen that would show
 *  them otherwise. Fabricating the answer is worse here than anywhere except a
 *  panic control. */
export async function setSensitivity(sensitivity) {
const call = await invoke();
const landed = await call('set_sensitivity', { sensitivity });
const thresholds = await call('get_thresholds');
capture.update((s) => ({ ...s, thresholds }));
return landed;
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
return guardedRead('listModels', async (call) => {
    return await call('list_models');
}, []);
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
  // Bring STT up in-place, ON THE MODEL THAT WAS JUST DOWNLOADED. A 148 MB
  // download that ends in "now quit and reopen the app" is a miserable last step
  // for a first-time user — and once more than one model exists, merely reloading
  // would bring up whichever the DEFAULT ORDER picks, which is the small one. The
  // operator would have waited out a 1.6 GB download to keep running `base`, with
  // the list showing the new model as installed and nothing saying which was live.
  const filename = (await call('list_models').catch(() => [])).find((m) => m.id === id)
    ?.filename;
  const loaded = filename
    ? await call('select_stt_model', { filename })
    : await call('load_stt_model');
  const stt = await call('stt_status');
  capture.update((s) => ({ ...s, stt }));
  return loaded;
} finally {
  stop.forEach((fn) => fn());
  modelProgress.set(null);
}
}

/**
 * Switch to an already-installed model, now — not on next launch.
 *
 * `filename` of `null` clears the choice and returns to the default order.
 * Resolves to whether speech recognition came back up.
 */
export async function selectModel(filename) {
const call = await invoke();
modelError.set(null);
try {
  const loaded = await call('select_stt_model', { filename: filename ?? null });
  const stt = await call('stt_status');
  capture.update((s) => ({ ...s, stt }));
  return loaded;
} catch (e) {
  // Swallowing this would leave the operator looking at a list that says they
  // switched, running the model they switched away from.
  modelError.set(humanError(e));
  return false;
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
