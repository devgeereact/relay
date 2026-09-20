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
//   `fireContent`, `startCountdown`, `adjustCountdown`, `endService`, `setSttLanguage`,
//   `setDefaultTemplate`, and the five timer wrappers — `startTimer`, `adjustTimer`,
//   `stopTimer`, `listTimers`, `showTimer` (held there by `timerwrappers.test.js`).
//   The caller MUST handle it and tell the operator.
//   `setDefaultTemplate` was missed when it stopped being a bare `set_setting` and
//   became the `set_default_template` COMMAND — which writes the row, pushes the
//   frame to every kiosk client and emits `output://default_template` to every
//   native window. It changes the LOOK every screen following the content look is
//   wearing, live, so a failure an operator is not told about is a gallery that
//   says one thing and a wall that says another.
//   `showBackground` is the most literal member there is: it puts a picture on every
//   congregation screen and takes it off again. A swallowed failure on the take-down
//   is the worse half — the operator believes the church's backdrop has gone and it
//   is still up behind the next thing they fire.
//   `setSttLanguage` joined this group with RG-138, when it stopped being a setting on
//   a live engine and became a WRITE to the active voice profile. It changes what the
//   AI hears, and it is the control RG-116 names as the mitigation for a service lost
//   to whisper electing the wrong language — a pin that silently did not happen is
//   exactly the failure it exists to prevent.
//   `endService` is the quiet member: it puts nothing on a wall, but it releases the
//   service lock, so a swallowed failure leaves Relay refusing deletions, imports and
//   model changes with nothing on screen saying why.
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
//   `applySafeMode` is in this group for the same reason and sets `safeModeError`:
//   it makes a LARGER promise than a panic key — "nothing Relay does can reach a
//   screen" — and the surface that flips it may be one that has already crashed.
//
// If you add a wrapper, put it in a group deliberately. "It seemed fine" is how a
// panic key came to do nothing.

import { writable, derived, get } from 'svelte/store';
import { parseTemplateOverride } from '../templates.js';
import { migrateTemplate } from '../templatemodel.js';
import { tNow } from '../i18n.js';
import { humanError } from '../errors.js';
import { markTranscript } from '../latency.js';
// The ONE reader of how long a countdown has left, and of whether it is being held
// (docs/REBRAND.md §7). The console reads it through the same function the wall and
// the stage page do, so a held countdown cannot go on ticking on one of the three.
import { countdownRemainingMs, countdownIsPaused } from '../countdown.js';
// The warning WINDOW, as distinct from how long is left. `layers.js` holds the
// one number the wall, the stage page and the dock all measure against; this file
// is its one writer, because this file is the only one that can read the row.
import { COUNTDOWN_WARN_MS, resolveContentOverride, setCountdownWarnDefault } from '../layers.js';
// X1 · the transition override's store lives beside its register — see the block
// further down for why it is not declared in this file.
import { liveTransition } from '../transitions.js';
// Safe mode's record lives in the boot record, and this file is the only thing
// allowed to write it — see `applySafeMode` below. `boot.js` imports only
// `svelte/store` and `../errors.js`, so there is no cycle here.
import { setSafeMode, safeMode } from '../boot/boot.js';

/**
 * The audio meter — RMS level + voice-activity, arriving 10–50 times a second.
 *
 * Deliberately its OWN store, not fields on `capture`. When these lived on the
 * `capture` mega-object, every audio frame notified every `$capture` subscriber
 * in the app: `App.svelte` reads `$capture.detectionOn` to draw one dot in the
 * sidebar and was re-rendering the entire shell dozens of times a second, for
 * data it does not use. Only the Settings meter subscribes here.
 */
export const meter = writable({ level: 0, isVoice: false, peaks: [] });

export const capture = writable({
  available: false, // Tauri backend attached?
  capturing: false,
  devices: [], // [{ name, is_default }]
  inputDevice: '', // operator-selected input device name ('' = default). Shared so Console + Settings agree.
  // The remembered microphone that is NOT attached today, or null. RG-121: Relay
  // falls back to the system default, and this is how it says so rather than
  // moving a church onto a laptop microphone in silence.
  inputDeviceMissing: null,
  stt: { loaded: false, model: null, language: null }, // local STT model status (language null = auto)
  detectedLang: null, // language of the latest transcript window (code-switching)
  // Auto-detect is not settling on a language — [codes] once per session, else null.
  // Whisper re-elects a language every window from ~99 candidates, and on accented
  // speech it wanders (one real service: en·yo·pt·sw·sv·ms). The label IS the decode,
  // so a wandering label degrades the transcript — and that looks exactly like the AI
  // being bad. The operator has the control that fixes it (Settings → Before the
  // & Languages → Recognition language) and no reason to suspect they should
  // touch it. See stt.rs.
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
  // THE DIAL POSITION, AND WHETHER ANYBODY HAS ACTUALLY ASKED. `sensitivity` is
  // `to_sensitivity(thresholds)` — the one inverse mapping, computed in Rust so
  // the two directions cannot drift — and `sensitivityKnown` is the answer to a
  // different question: has the engine ever told us? 50 is both the shipped
  // default and a perfectly ordinary real setting, so the number alone cannot
  // separate "the gate is at 50" from "nobody has asked since launch". A surface
  // that cannot tell those apart is rule 35 on the one control governing what the
  // AI may put on a wall unasked.
  sensitivity: 50,
  sensitivityKnown: false,
  // DOES THAT DIAL POSITION ACTUALLY PRODUCE THAT GATE?
  //
  // A third fact again, and for the same reason the second one exists.
  // `to_sensitivity` reports the dial position NEAREST the gate, and three things
  // move the gate without anybody touching the dial — a voice profile restoring
  // what it learned, a room being applied, and the self-calibration on every
  // confirm and dismiss. So the dial can be drawn at 38 over a gate that 38 would
  // never produce, and the number reads as the operator's own setting.
  //
  // `Thresholds::follows_dial` answers it in Rust, beside the one curve it is a
  // question about; `gate.js::describeGate` is the only place that turns the
  // answer into words. Meaningful only when `sensitivityKnown` — an unread gate
  // has not drifted, it is simply unread, and claiming otherwise would be
  // inventing the worse of the two facts.
  gateOnDial: true,
});

// What is currently ON the output screens (last fired content, null = cleared).
// Mirrors the `output://content` / `output://clear` broadcast so the console
// previews show what's actually live.
export const live = writable(null);

// True when the operator has blacked out the screens (opaque, not a transparent
// clear). Reset by the next fire/clear. Mirrors the output://black broadcast.
export const screenBlack = writable(false);

// THE STANDING BACKGROUND — `{ media_url, media_kind }`, or null.
//
// A SECOND payload beside `live`, not a field on it, and the separation is the
// whole feature: `live` is what the screens are showing and this is what they are
// showing it ON, so a verse arriving replaces one and leaves the other. Until it
// existed a church could have scripture or its own backdrop and never both.
//
// Mirrors `output://background`, and — the line that matters — is set to null by
// the `output://clear` and `output://black` listeners below, because a panic
// control takes everything.
export const background = writable(null);

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

/**
 * CLAIMS THAT HAVE BEEN DECIDED — the receipt, not a second claim list.
 *
 * `detections` above holds only what is still awaiting an operator, which is
 * right and must stay that way. But it leaves one hole, and it is the one the
 * whole product turns on: **an auto-fire never enters it at all.** A `Direct`
 * hit above the bar goes straight to the screens and the pending suggestion is
 * REMOVED, so the only thing an operator sees is a verse appearing on the
 * programme with nothing anywhere saying the AI put it there or what kind of
 * claim it was. Rule 18 says the operator must see which kind of claim the AI is
 * making; on the one path where the AI acts alone, they could not.
 *
 * So this is a bounded, newest-first log of what HAPPENED to a claim, and every
 * entry names who acted:
 *
 *   'auto'      the AI fired it unprompted. Only ever `status === 'auto'` from
 *               the backend — a manual fire emits the same event with
 *               `status: 'manual'` and must never be written up as the AI's.
 *   'accepted'  the operator pressed Accept AND the fire resolved. Recorded
 *               after the await, never before: a card reading "put on the
 *               screens" over a fire that failed is rule 15 in another coat.
 *   'dismissed' the operator dismissed it.
 *
 * It is a RECEIPT, so it is not a trap under the `A` key the way a stale
 * suggestion is — nothing here is actionable. It is capped rather than pruned by
 * age for the same reason: the operator asking "what just went out, and did a
 * person decide it" is asking about the last few seconds either way.
 */
export const resolvedDetections = writable([]);

/** How many receipts are kept. Small: this is the last few, not a history. */
export const MAX_RESOLVED = 4;

/** Write one receipt. Never called with anything but a claim that really ended. */
function noteResolved(d, outcome) {
  if (!d?.reference) return;
  resolvedDetections.update((list) =>
    [{ ...d, outcome, resolvedAt: Date.now() }, ...list.filter((x) => x.reference !== d.reference)].slice(
      0,
      MAX_RESOLVED,
    ),
  );
}

// Output templates (Phase 8), loaded from the DB.
export const templates = writable([]);

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
  // The COMMAND, not the raw setting. Rust owns the fact now: it writes the
  // row, pushes `default_template` to every kiosk client and emits
  // `output://default_template` to native windows, so a screen following the
  // content look re-resolves at once instead of at its next reload.
  await call('set_default_template', { templateId: id ?? null });
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
        // Both halves of the countdown model reach the console preview, or the
        // preview is the one surface that disagrees with the wall — it would go on
        // ticking a countdown the operator has held, and the preview exists to be
        // the thing they can trust.
        countdown_from: $l.countdown_from,
        countdown_paused_ms: $l.countdown_paused_ms,
        countdown_done: $l.countdown_done,
      }
    : null,
);

/**
 * The per-content-type template override riding on the live content (lyrics
 * render as lyrics, scripture as scripture), or null to use the channel's own
 * template. Malformed JSON falls back to the channel template rather than
 * throwing — a bad template must never take the screens down mid-service.
 *
 * ── IT DEPENDS ON `templates` NOW, AND THAT IS THE FIX ──────────────────────
 *
 * An override arrives in two shapes and this used to read only one of them.
 * A Planner cue's PINNED choice ships its own JSON; a per-kind CONTENT LOOK
 * ships an id and nothing else, deliberately, because a look carrying an
 * embedded `data:` image has been 13 MB and serialising it onto every fire made
 * verses take seconds (`main::cue_or_content_tpl`). `parseTemplateOverride` is
 * null BY CONSTRUCTION for the second shape — so the console's program pane and
 * the Outputs tile both resolved a content look to nothing, and both of them
 * are the surfaces an operator uses to CHECK that a look is working.
 *
 * Two surfaces, one store, so they cannot reach different conclusions about the
 * same screen. `Channels.svelte` records being caught by that class of drift
 * three times; the program pane's claim is stronger still, because it is what an
 * operator looks at instead of the wall.
 *
 * `templates` is the console's own list and is loaded by the dock at launch, so
 * it is populated on every workspace rather than only where a view happens to
 * fetch it. An empty list resolves to null — the old behaviour, which paints the
 * screen's own template — rather than to anything invented.
 */
export const liveTemplateOverride = derived([live, templates], ([$l, $tpls]) =>
  resolveContentOverride($l, $tpls),
);

/** Whether the live override is a PINNED cue choice (overrides the screen) vs a
 *  content-type default (defers to the screen's own template). Mirrors the exact
 *  resolution the real output window uses, so the console program pane matches. */
export const liveTemplatePinned = derived(live, ($l) => !!$l?.template_pinned);

/**
 * How many closed transcript lines are kept.
 *
 * IT WAS 12, AND 12 IS THE REASON THE TRANSCRIPT CARD LOOKED EMPTY. The dock
 * capped its own render at 40 lines and that cap could never bite, because the
 * store never held more than twelve. Measured on the service of 2026-09-20: 147
 * closed utterances across 5611 seconds, one about every 38 seconds, so twelve
 * lines is roughly seven minutes of a service and four of them fill a 152px
 * card. An operator scrolling back "to read what was said in the past few
 * minutes" reached the top almost at once.
 *
 * 240 is about two and a half hours at that rate, which is longer than any
 * service Relay has run, and it is bounded rather than unbounded on purpose: an
 * uncapped list on a surface that updates every few seconds is a leak with a
 * nice view. The cost is strings — the whole 93-minute service was 816 lines and
 * under 200 kB of text — and the array is rebuilt per line either way, which is
 * what the slice was already doing at 12.
 */
const MAX_FINALS = 240;
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

/**
 * THE BRIDGE, RESOLVED ONCE — and the reason is a measured CI failure, not tidiness.
 *
 * This did `await import('@tauri-apps/api/core')` on every call. In a browser and in
 * the packaged app that is free: the module registry caches it, so N callers cost one
 * load. It is not free when several callers race a COLD registry under vitest, where
 * the mocked-module resolution hands the SECOND concurrent importer `undefined` and
 * `core.invoke` then throws `Cannot read properties of undefined (reading 'invoke')`.
 * `readstates.test.js` recorded that symptom in prose long before anything failed on
 * it; it took three CI rounds to connect the two, because every read here is GROUP 2
 * and SWALLOWS, so the failure surfaced as a screen politely saying it could not tell.
 *
 * A mounting view is exactly that race: the dock fires its channel read, its template
 * read and its default-template read in one go.
 *
 * ONE in-flight promise, memoised on the PROMISE rather than on its result, so N
 * concurrent callers await the same import instead of starting N of them. A rejection
 * is deliberately NOT cached: a plain browser has no bridge at all and must stay
 * askable, and caching the failure would turn a transient into a permanent one.
 */
let corePromise = null;
async function invoke() {
  if (!corePromise) {
    corePromise = import('@tauri-apps/api/core').catch((e) => {
      corePromise = null; // throws in a plain browser — stay askable
      throw e;
    });
  }
  const core = await corePromise;
  return core.invoke;
}

/**
 * Is the Rust core answering, right now?
 *
 * GROUP 2 — swallows, and its safe default is `false`: a probe that cannot reach
 * the backend has not proved the backend is there. Deliberately `ping` and never
 * `greet` (CLAUDE.md rule 26): `greet` prints `console: webview up` and its whole
 * value is that it does so exactly once per console mount, so nothing that repeats
 * may call it.
 */
export async function ping() {
  try {
    const call = await invoke();
    return (await call('ping')) === true;
  } catch {
    return false;
  }
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
  const [devices, stt, gate, detectionOn, storedDevice] = await Promise.all([
    call('list_audio_devices').catch(() => []),
    call('stt_status').catch(() => ({ loaded: false, model: null, language: null })),
    // THE WHOLE READ-OUT, NOT ONLY THE TWO NUMBERS, and it has to be read here
    // rather than waited for. `setup` applies the active profile's LEARNED gate
    // before the window exists, so the emit that would have announced it has
    // nobody to reach (the named exception in `hardrules.test.js`). A console that
    // only listened would open showing the learned gate, drawn at whatever dial
    // position is nearest it, with no caveat anywhere.
    // `null` on failure, never a fabricated pair: the store's placeholder stays,
    // and `sensitivityKnown` stays false so nothing mistakes it for a reading.
    call('get_thresholds').catch(() => null),
    call('get_detection_enabled').catch(() => true),
    // RG-121. Every launch used to start on the system default, whatever was
    // selected last time, and nothing said so.
    call('get_setting', { key: INPUT_DEVICE_KEY }).catch(() => null),
  ]);
  const chosen = chooseInputDevice({ stored: storedDevice, devices });
  // `get_thresholds` returns the same four facts `detection://thresholds` carries,
  // deliberately: the event is how a surface hears about a change and this is how
  // it starts out, and a surface that learned two different things from the two
  // would be the drift the pair exists to prevent.
  const gateRead = Number.isFinite(Number(gate?.sensitivity));
  capture.update((s) => ({
    ...s,
    available: true,
    devices,
    stt,
    thresholds: gate ? { auto_fire: gate.auto_fire, suggest: gate.suggest } : s.thresholds,
    sensitivity: gateRead ? Number(gate.sensitivity) : s.sensitivity,
    sensitivityKnown: s.sensitivityKnown || gateRead,
    gateOnDial: gateRead ? gate.on_dial !== false : s.gateOnDial,
    detectionOn,
    inputDevice: chosen.device,
    inputDeviceMissing: chosen.missing,
  }));

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
      await listen('output://content', (e) => { live.set(e.payload); screenBlack.set(false); noteOperatorAction('content', e.payload); });
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
      // THE STANDING BACKGROUND, and the two controls that take it away.
      //
      // `background.set(null)` sits on both panic listeners for the same reason
      // `leavePlan()` does: these events are the console's ONLY report of a clear
      // that did not originate here — the preacher's phone, the spoken "clear the
      // screen", the exit from a rehearsal — and the program pane renders through
      // the same `TemplateRender` the wall does. A backdrop left in this store
      // would paint the church's picture in the pane over a wall that had none,
      // which is the console disagreeing with the room about what a congregation
      // is looking at.
      await listen('output://background', (e) => {
        const p = e.payload;
        background.set(p?.media_url ? { media_url: p.media_url, media_kind: p.media_kind || 'image' } : null);
      });
      await listen('output://clear', () => { live.set(null); screenBlack.set(false); background.set(null); leavePlan(); noteOperatorAction('clear'); });
      // RG-151. `live` IS TAKEN DOWN HERE TOO, and for the reason stated one line
      // up rather than a new one. `Live.svelte` takes the preacher's `stage_next`
      // panel off the monitor by watching the truthy→falsy edge of `live`, and
      // this listener set `screenBlack` and nothing else — so `Esc` took the hint
      // down and `B` left it standing, and the two panic controls disagreed about
      // a screen the congregation cannot see and the preacher is reading from.
      //
      // Nothing is on the screens after a blackout, so `live` being null is the
      // truth and not a convenience. `screenBlack` is what keeps a blackout
      // distinguishable from a clear, and it still does. The backdrop goes with it:
      // a blackout that left the church's picture up would be the panic control
      // failing at the one thing it is for.
      await listen('output://black', () => { live.set(null); screenBlack.set(true); background.set(null); leavePlan(); noteOperatorAction('black'); });
      // A SPOKEN "next"/"back" that did nothing. The STT thread has no caller to
      // return a NavResult to, so it pushes it here — the preacher says "next", the
      // wall does not move, and the console explains why instead of staying silent.
      await listen('nav://blocked', (e) => navBlocked.set(e.payload));
      // A clear that failed on a path with nobody to return an error to — the
      // spoken "clear the screen", and the exit from rehearsal (which hands the
      // wall back to the congregation). Same banner as a failed key or button.
      // Humanised, for the same reason the wrapper above is: two of the four
      // emitters send raw Rust text, so a poisoned lock used to read
      // "internal lock error: poisoned lock: …" inside an assertive live region.
      await listen('output://panic_failed', (e) => panicError.set(humanError(e.payload)));
      await listen('rehearsal://changed', (e) => rehearsing.set(e.payload === true));
      // THE GATE MOVED, AND EVERY SURFACE SHOWING IT HEARS HERE.
      //
      // Four things in Rust move `Router.thresholds` — the dial, a profile saved,
      // a profile selected or a room applied, and the learning on every confirm
      // and dismiss — and until 2026-09-17 not one of them announced it. (It was
      // five until the two Settings sliders were deleted: a second control over
      // one fact, pointing the opposite way, DECISIONS §96.) Live's dial was read once at `onMount` into a plain
      // `let`, and the dock is mounted OUTSIDE the workspace router, so unlike
      // every view it is never rebuilt. It therefore showed its launch reading for
      // the rest of the session while the engine moved underneath it, and Settings
      // — holding the figure IT loaded when the tab opened — silently reverted the
      // operator's change on the next profile save.
      //
      // One event, one store, every surface derived. A surface that keeps its own
      // copy of this number is the defect, not the fix.
      await listen('detection://thresholds', (e) => {
        const p = e.payload || {};
        const auto_fire = Number(p.auto_fire);
        const suggest = Number(p.suggest);
        const sensitivity = Number(p.sensitivity);
        capture.update((s) => ({
          ...s,
          thresholds: {
            auto_fire: Number.isFinite(auto_fire) ? auto_fire : s.thresholds.auto_fire,
            suggest: Number.isFinite(suggest) ? suggest : s.thresholds.suggest,
          },
          // A malformed payload leaves the READING alone and does not claim to
          // know it: answering a broken frame with 50 would put a number on screen
          // that is nobody's setting.
          sensitivity: Number.isFinite(sensitivity) ? sensitivity : s.sensitivity,
          sensitivityKnown: s.sensitivityKnown || Number.isFinite(sensitivity),
          // The gate having MOVED is exactly when this can change, so it rides on
          // the announcement that it moved. A missing field leaves the last answer
          // alone rather than defaulting to "on the dial" — the reassuring one.
          gateOnDial: typeof p.on_dial === 'boolean' ? p.on_dial : s.gateOnDial,
        }));
      });
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

/**
 * Close the open service record — the history is kept.
 *
 * THROWS (contract group 1). It does not change what is on a screen, but it
 * releases the SERVICE LOCK — the list of things Relay is currently refusing to
 * do — and `end_service` takes `session.0.lock()?`, so a poisoned session mutex
 * refuses it for real. Swallowed, the operator presses End current service, the
 * list repaints unchanged, and the console goes on refusing deletions and model
 * changes for a reason that has scrolled out of view. Pinned by
 * `endservice.test.js`.
 */
export async function endService() {
  let call = null;
  try {
    call = await invoke();
  } catch {
    /* no Tauri bridge at all (a plain browser) — there is no service to end */
  }
  // Deliberately NOT in a try: a rejection must reach the caller, and must reach
  // it before the lock is re-read — re-reading over a failure repaints an
  // unchanged surface, which is the original defect's whole disguise.
  if (call) await call('end_service');
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

/**
 * Is Relay getting slower week by week?
 *
 * The question a single service cannot answer. A church that adds a bigger model,
 * or whose laptop fills up over a winter, degrades gradually and every individual
 * Sunday looks fine.
 */
export async function perfHistory(metric, limit) {
  return guardedRead('perfHistory', (call) => call('perf_history', { metric, limit }), []);
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
 * DEMO CONTENT — the sample service a church can load to try Relay, and remove.
 *
 * Three wrappers, in two different groups on purpose.
 *
 * `demoStatus` is a read and swallows: a panel that cannot say what is loaded is a
 * cosmetic loss, and the safe default (`loaded: false`) makes the surface offer
 * *Load* rather than *Remove* — the direction that cannot destroy anything.
 *
 * `loadDemoContent` and `removeDemoContent` are GROUP 1 — THROWS. Neither changes
 * what a congregation sees, so they are here for the reason `exportDiagnostics` is:
 * a bulk write or a bulk delete that failed in silence leaves an operator pressing
 * a button and watching nothing happen, with no reason given. `removeDemoContent`
 * also RETURNS counts the caller has to render — `{ removed, kept }` — because a
 * demo item they had edited is kept rather than deleted, and finding that out for
 * themselves later is exactly the kind of surprise this feature must not create.
 *
 * Both are refused outright while a service is recording (`servicelock.rs`); the
 * typed `Refused` error carries the sentence that says how to proceed, and
 * `errors.js` is what turns it into words.
 */
export async function demoStatus() {
  return guardedRead('demoStatus', (call) => call('demo_status'), {
    loaded: false,
    total: 0,
    edited: 0,
    groups: [],
  });
}

/** Load the demo dataset. GROUP 1 — THROWS. Returns the new status. */
export async function loadDemoContent(date) {
  const call = await invoke();
  return call('load_demo_content', { date: date ?? new Date().toISOString().slice(0, 10) });
}

/** Remove it again. GROUP 1 — THROWS. Returns `{ removed, kept, files }`. */
export async function removeDemoContent() {
  const call = await invoke();
  return call('remove_demo_content');
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

/**
 * Model files already on this machine, waiting to be installed.
 *
 * The offline path: a church on a poor line copies the 148 MB model from a USB
 * stick, and Relay finds it in Downloads or its own data folder. Read-only, so it
 * swallows — a failed scan must show "none found", not take the screen down.
 */
export async function findModelFiles() {
  return guardedRead('findModelFiles', (call) => call('find_model_files'), []);
}

/** Install one of them. GROUP 1 — THROWS: a silent failure here leaves an
 *  operator believing they have a speech model when they do not. */
export async function installModelFile(path) {
  const call = await invoke();
  const id = await call('install_model_file', { path });
  await listModels();
  return id;
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

/**
 * Erase one recorded service — transcript, detections, operator actions, timeline,
 * latency samples. Returns how many transcript rows went, so the caller can say
 * what it did rather than claim a success in the abstract.
 *
 * GROUP 1 — THROWS. It is irreversible, it is refused outright while a service is
 * recording, and a delete that silently did nothing is the worst outcome available
 * on this screen: the operator would believe a sermon had been erased.
 */
export async function deleteService(id) {
const call = await invoke();
return await call('delete_service', { id });
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

/**
 * The setting that remembers the microphone between launches (RG-121).
 *
 * `''` is a real stored value and means "the system default", which is a different
 * fact from never having chosen.
 */
export const INPUT_DEVICE_KEY = 'audio.input_device';

/**
 * Which device to select at launch, and whether to say something about it.
 *
 * Pure, and separated from the store for that reason: this is the whole of RG-121's
 * rule and it has to be testable without a backend or a microphone.
 *
 * The rule is that a remembered device which is NOT here today must not be selected
 * silently. Relay would then capture from whatever the OS calls default — on
 * 2026-09-06 that was a laptop microphone at the back of a booth while a Blackmagic
 * desk feed sat plugged in and selected in the previous session — and rule 12's
 * learned gate would do its best with it, which is exactly the failure that is
 * invisible until someone listens back to the service.
 *
 * So a missing device falls back to the default AND is reported. Falling back
 * without reporting is the bug with an extra step.
 */
export function chooseInputDevice({ stored, devices } = {}) {
  const names = (devices ?? []).map((d) => d?.name).filter(Boolean);
  // Never chosen, or deliberately the default: nothing to restore, nothing to say.
  if (stored === null || stored === undefined || stored === '') {
    return { device: '', missing: null };
  }
  if (names.includes(stored)) return { device: stored, missing: null };
  return { device: '', missing: stored };
}

/**
 * Set the shared input device (name, or '' for default). Used by Console + Settings.
 *
 * Persists it, so the next launch starts on the microphone this room actually uses.
 * GROUP 2: the write never throws at the caller. A setting that would not save must
 * not stop an operator changing microphone thirty seconds before a service.
 */
export function setInputDevice(name) {
capture.update((s) => ({ ...s, inputDevice: name || '', inputDeviceMissing: null }));
void persistInputDevice(name || '');
}

async function persistInputDevice(name) {
try {
  const call = await invoke();
  await call('set_setting', { key: INPUT_DEVICE_KEY, value: name });
} catch {
  /* no backend, or the write failed. The choice still applies to this run. */
}
}

/**
 * Drop the three capture listeners if they are attached. Safe to call when they
 * are not. Used by `stopCapture` and, because capture can also end without any
 * command being issued, by `startCapture` on its way in.
 */
function detachCaptureListeners() {
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

// DETACH BEFORE ATTACHING. `stopCapture` is not the only way capture ends: a
// device that dies mid-service arrives as `audio://error`, which clears
// `capturing` so the operator can press the microphone again — and it tears
// nothing down, because it is an event, not a command. Without this, the second
// Start overwrote three live handles and every transcript event was then
// delivered TWICE, to two listeners, for the rest of the service. Idempotent, so
// the ordinary Stop-then-Start path is unchanged.
detachCaptureListeners();

// Last language pushed to `capture` — guards against re-notifying subscribers
// on every transcript when the detected language hasn't changed.
let lastLang = null;
// The hot path. Goes to `meter`, never to `capture` — see the note on `meter`.
unlistenAudio = await listen('audio://chunk', (e) => {
  const { rms, is_voice, peaks } = e.payload;
  // `peaks` is the chunk's own envelope, sixteen readings across its 400 ms
  // (`audio::CHUNK_PEAKS`). It rides on the event that was already being sent
  // rather than on one of its own. An older backend sends none, so the console
  // must still work from `level` alone — hence a default rather than a guard.
  meter.set({ level: rms, isVoice: is_voice, peaks: Array.isArray(peaks) ? peaks : [] });
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
  // THE AI ACTED BY ITSELF. This is the only moment that fact exists on the
  // frontend — the suggestion is about to be removed and nothing else records
  // it. Strictly `'auto'`: a manual fire emits this same event with
  // `status: 'manual'`, and writing that up as the AI's decision would be the
  // `persist_fire` bug (rule 14) reproduced in the UI.
  if (d?.status === 'auto') noteResolved(d, 'auto');
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

detachCaptureListeners();
capture.update((s) => ({ ...s, capturing: false }));
// The live level lives on the `meter` store, not `capture` — resetting
// capture.level/isVoice (which nothing reads) left the input bars frozen lit at
// the last value after Stop. Reset the store that actually drives them.
meter.set({ level: 0, isVoice: false, peaks: [] });
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
// Read the claim BEFORE the round trip — the receipt below needs the method and
// the words, and by the time it is written the card is gone from the list.
const claim = get(detections).find((d) => d.reference === reference) ?? null;
const thresholds = await call('confirm_detection', { reference });
// Accepting an AI suggestion also takes us out of the plan — same reason as
// manualFire.
leavePlan();
detections.update((list) => list.filter((d) => d.reference !== reference));
// AFTER the await, never before. "Accepted — put on the screens" over a fire
// that threw is rule 15 in another coat, and this function's own doc comment
// records that exact bug happening to the card itself.
if (claim) noteResolved(claim, 'accepted');
capture.update((s) => ({ ...s, thresholds }));
}

/** Operator dismisses a suggestion → drop it + tighten the gate. */
export async function dismissDetection(reference) {
const claim = get(detections).find((d) => d.reference === reference) ?? null;
if (claim) noteResolved(claim, 'dismissed');
detections.update((list) => list.filter((d) => d.reference !== reference));
// Noted before the round trip: dismissing is a decision the operator has already
// made, and the practice drill is about the decision, not about whether the
// calibration write succeeded.
noteOperatorAction('dismiss', reference);
try {
  const call = await invoke();
  // The reference rides to the backend so the rejection lands in the service
  // record as a rejection OF SOMETHING. It was already in this function's
  // signature and was being dropped on the floor at the one line that mattered.
  const thresholds = await call('dismiss_detection', { reference });
  capture.update((s) => ({ ...s, thresholds }));
} catch {
  /* backend absent */
}
}

/** Manual override: fire a free-text reference now (throws if unparseable).
 *  `stageNote` is this cue's optional Stage Note, for the monitors only.
 *  `keepPlan` — when a PLAN slide is being fired (the operator stepping the plan
 *  in Slide mode), the transport must STAY on the plan. Without this, firing a
 *  scripture plan cue ran `leavePlan()` below and flipped the transport out of
 *  Slide mode into Verse mode — so the very next → walked the passage instead of
 *  advancing the plan. Songs/media/countdown never hit this (they don't fire
 *  through `manual_fire`); only scripture cues did, which is exactly what made
 *  Slide mode "break" on a scripture item. Hand-typed fires keep the default. */
export async function manualFire(
reference,
stageNote = null,
templateId = null,
keepPlan = false,
// WHICH SCREENS (RG-161). `null` is every screen, which is what the operator's
// own reference box and the preacher's phone always are — only a plan cue has
// anywhere anybody could have said otherwise.
channels = null,
) {
const call = await invoke();
await call('manual_fire', { reference, stageNote, templateId, channels });
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

/**
 * Ordered cues of a plan.
 *
 * GUARDED (RG-95). It swallowed into a bare `catch {}`, so the run surface and the
 * Planner both rendered "This plan has no cues yet" over a read that failed — a
 * sentence that, ten minutes before a service, tells an operator their plan is
 * empty and their Tuesday evening is gone.
 *
 * **It stays a GROUP 2 read and must not be promoted to one that throws.**
 * `Live.svelte::loadPlan` sets `itemsLoaded = false`, awaits this, and sets it
 * true afterwards with no `finally` — a throw there leaves the run surface saying
 * *Loading cues…* for the rest of the service. The fallback is what keeps that
 * flag moving; `readErrors` is what makes the failure visible.
 */
export async function planItems(planId) {
return guardedRead('planItems', async (call) => {
    return await call('plan_items', { planId });
}, []);
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

/** Set/clear a cue's Stage Note (confidence-monitor only; blank clears). */
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

/**
 * Bind a cue to a Stage Timer of `minutes`, or clear it with `null`.
 *
 * A REQUEST, STORED — not a clock started. Nothing about this reaches a screen or
 * a preacher's rail: the Planner may not, and does not (`plannerbuildonly.test.js`).
 * Live is what reads the binding and starts the timer when the cue goes on air.
 *
 * This is NOT `setPlanDuration`. That is the running-time estimate the Planner adds
 * up; this is a clock somebody will be watching. `db/plans.rs`'s `PlanItem` records
 * why there are two.
 */
export async function setPlanTimer(id, minutes) {
const call = await invoke();
await call('set_plan_timer', { id, minutes: minutes ?? null });
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


/** True while a countdown is live on the outputs (its target is still in the
 *  future). Derived from the mirrored output content, so it clears the moment
 *  the screen is cleared or any other content goes live. */
export function countdownRunning() {
return countdownRemaining() !== null;
}

/** Is the countdown on the wall being HELD? Through the one reader, so the
 *  transport, the wall and the stage page cannot disagree about it. */
export function countdownHeld() {
return countdownIsPaused(get(live));
}

/** How long the countdown ON THE WALL has left, in ms — or null when there is no
 *  countdown on the wall. The transport reads THIS, never its own clock: one
 *  timer, so the figure in the dock and the figure on the screen cannot drift
 *  (docs/REBRAND.md §7). */
export function countdownRemaining(atMs = Date.now()) {
const left = countdownRemainingMs(get(live), atMs);
// `countdownRemainingMs` distinguishes "finished" (0) from "there is no countdown"
// (null) because a renderer has to show a done message for one and nothing for the
// other. The TRANSPORT does not: a countdown that has run out is not something ±1
// can re-aim, so both are null here.
return left != null && left > 0 ? left : null;
}

/**
 * RE-AIM THE RUNNING COUNTDOWN — Reset and ±1 on the transport.
 *
 * Separate from `startCountdown` on purpose. That one REFUSES while a countdown
 * is running, which is right for "Start" (a second countdown over the first is
 * always a mistake) and wrong for every transport press, all of which are about
 * the countdown that is already there. One broadcast per press; the outputs go
 * on ticking locally, so this adds no per-second traffic.
 *
 * THROWS (contract group 1) — it changes what a congregation is looking at.
 *
 * **The carry-over now happens in the engine, not here** (`main::adjust_countdown`).
 * This used to rebuild the whole fire out of `$live` — the label, the done message
 * and the template read back off the event and handed to `start_countdown` again —
 * and it worked exactly as long as every caller remembered every field. A held
 * countdown added one more to forget, and forgetting THAT one restarts a paused
 * timer in front of a congregation from a press of `+1`. The engine keeps the
 * countdown and this asks it to change one thing about it; the guarantees that used
 * to be pinned here (the label does not change, and an UNPINNED template is never
 * re-pinned — DECISIONS §29) are pinned in `e2e.rs` instead, where they now hold for
 * every caller rather than for this one.
 */
export async function adjustCountdown(ms, keepPlan = true) {
const remainingMs = Math.round(Number(ms));
if (!Number.isFinite(remainingMs) || remainingMs <= 0) {
  throw new Error('A countdown needs a length greater than zero.');
}
const call = await invoke();
await call('adjust_countdown', { remainingMs, paused: null });
// `keepPlan` DEFAULTS TRUE here, and it is the only wrapper in this file that
// does. Every other take replaces what is on the wall, so the plan cue that was
// amber is no longer what anyone is looking at. This one changes a NUMBER on
// content that is already up: if a plan's countdown cue is on air, it is still on
// air afterwards, and clearing `onAir` would grey out the correct cue and send
// the next `→` back to cue 1. A countdown started from the dock already left the
// plan when it started, so there is nothing left to clear either way.
if (!keepPlan) leavePlan();
}

/**
 * HOLD OR RELEASE THE COUNTDOWN ON THE SCREENS — the half of §7's transport that
 * did not exist until the engine had a field for it.
 *
 * Every other press on that row re-aims an absolute instant, which is something
 * `countdown_to` can already say. "Stopped" is not an instant, so it is said by
 * `countdown_paused_ms` instead, and it is said by the engine: a held countdown must
 * stay held through a `+1`, through a screen reconnecting mid-service, and through
 * anything else that re-broadcasts it.
 *
 * THROWS (contract group 1) — it changes what a congregation is looking at. It
 * cannot start a countdown: with nothing counting the engine refuses, in words.
 *
 * `keepPlan` defaults true for the same reason `adjustCountdown`'s does — holding a
 * plan's countdown cue leaves that cue exactly as on-air as it was.
 */
export async function pauseCountdown(paused, keepPlan = true) {
const call = await invoke();
await call('adjust_countdown', { remainingMs: null, paused: !!paused });
if (!keepPlan) leavePlan();
}

/** Start a pre-service countdown on every output. Outputs tick MM:SS locally
 *  from the broadcast target; `label` shows above, `doneMsg` replaces it at 0.
 *  Guarded: refuses to start a second countdown while one is still running —
 *  clear the screen (or let it finish) first.
 *
 *  THE WORDS DEFAULT TO NOTHING, and that is the whole of them. They used to
 *  default to "Service begins in" and "Welcome", which meant a caller with no
 *  field for either — the dock had none for months — put words on a wall that
 *  nobody in the building had chosen and nobody could change. The words are
 *  payload, not template: they ride in `content.reference` and a template's
 *  `reference`-bound layer draws them, so a caller that has nothing to say says
 *  nothing and the screens show the digits alone. The Planner is the surface
 *  that does have something to say, and it passes it here.
 *
 *  `warnMs` is a threshold chosen for THIS countdown — how long before zero the
 *  figure turns red — and null means "nobody chose one", which falls to the
 *  configured default and then to the tenth-of-span rule. That ranking is
 *  `layers.js::countdownWarning`'s and is not restated anywhere. The engine used to
 *  hard-code `None` here, so a `Both` timer started from the transport could not
 *  express one at all (RG-149(b)); `startTimer` has taken the same figure since
 *  wave 3. **No control chooses one yet** — a per-cue field in the planner is what
 *  would, and that is recorded as still open in RG-149's row rather than implied by
 *  this parameter. */
export async function startCountdown(
minutes,
label = '',
doneMsg = '',
templateId = null,
keepPlan = false,
warnMs = null,
untilMs = null,
// WHICH SCREENS (RG-161). `null` is every screen. The engine stamps it onto the
// TIMER rather than onto this one broadcast, because `adjust_countdown` and
// `show_timer` put the same countdown out again later — see `Timer::channels`.
channels = null,
) {
if (countdownRunning()) {
  throw new Error('A countdown is already running — clear the screen to start a new one.');
}
const call = await invoke();
// `untilMs` is an absolute instant, worked out by `atClockTime` where the
// machine's timezone and DST rules are actually known. When it is given it wins
// over `minutes`; the engine stores it so Reset goes back to the appointment.
await call('start_countdown', { minutes, label, doneMsg, templateId, warnMs, untilMs, channels });
if (!keepPlan) leavePlan();
}

// ── TIMERS ────────────────────────────────────────────────────────────────────
//
// A timer has an identity and a lifetime of its own (`src-tauri/src/timers.rs`).
// `startCountdown` above is still the dock's one-press congregation countdown and
// still creates one; these five address timers by id, which is what lets a console
// show several and move the one the operator is pointing at.
//
// **All five are GROUP 1 — THROWS.** They change what is on a screen, what a
// preacher is being told, or what an operator believes about either, and a failure
// the caller cannot see is a control that lies about what it did. `timerwrappers.
// test.js` is what holds them in this group; a comment on its own does not.

/**
 * START A TIMER AND HAND BACK ITS IDENTITY. It puts nothing in front of anybody.
 *
 * `scope` is `'both'` (a congregation countdown) or `'stage'` (a Stage Timer
 * for the preacher's monitor). Putting a `'both'` timer on the screens is
 * `showTimer`; there are exactly two doors onto a congregation wall and this is
 * deliberately not one of them.
 *
 * THROWS (contract group 1).
 */
export async function startTimer({
minutes,
label = '',
doneMsg = '',
scope = 'both',
warnMs = null,
planItemId = null,
untilMs = null,
}) {
const call = await invoke();
return call('start_timer', { minutes, label, doneMsg, scope, warnMs, planItemId, untilMs });
}

/**
 * RE-AIM OR HOLD ONE TIMER, BY ITS IDENTITY.
 *
 * `adjustCountdown` is this same action aimed at "whichever congregation countdown
 * is running", which is what the dock's transport means. This one names the timer.
 *
 * It repaints a wall only when that timer is what the screens are already showing.
 * Changing a number on a timer that is not up must not put it up — the way back is
 * `showTimer`, an action that says what it does.
 *
 * THROWS (contract group 1).
 */
export async function adjustTimer(timerId, { remainingMs = null, paused = null } = {}) {
const call = await invoke();
await call('adjust_timer', { timerId, remainingMs, paused });
}

/**
 * THE STAGE LAYOUTS AN OPERATOR CAN CHOOSE BETWEEN. Global, by name.
 *
 * A read for a picker: an empty list is a usable answer (no layouts yet) and a
 * failed read must not take the Outputs desk down with it, so this swallows and
 * answers `[]` — group 2.
 */
export async function listStageLayouts() {
return guardedRead('listStageLayouts', async (call) => {
    const rows = await call('list_stage_layouts');
    // ALWAYS A LIST. `guardedRead`'s fallback covers a THROW; it does not cover
    // a bridge that answers with something that is not a list, and the one
    // consumer is an `{#each}`. A picker handed a non-list takes the whole
    // Outputs desk down with it — which is a screen-configuration surface
    // failing because a list of layouts could not be read.
    return Array.isArray(rows) ? rows : [];
}, []);
}

/**
 * CREATE A STAGE LAYOUT, or rename and re-zone one that exists.
 *
 * `id` null creates. Returns the layout's identity so the caller can select
 * what it just made without re-reading the list and guessing which row is new.
 *
 * THROWS (contract group 1). Every refusal it can raise is one the operator has
 * to see and act on — a name already taken, a layout with no name — and a save
 * that silently did nothing is the worst of them.
 */
export async function upsertStageLayout(id, name, zones) {
const call = await invoke();
return await call('upsert_stage_layout', { id: id ?? null, name, zones });
}

/**
 * REMOVE A STAGE LAYOUT.
 *
 * Refused when a screen is wearing it (the screens are named) and when it is
 * one Relay ships with (the seed would put it back on the next launch, and a
 * delete that undoes itself overnight is worse than a refusal).
 *
 * THROWS (contract group 1).
 */
export async function deleteStageLayout(id) {
const call = await invoke();
await call('delete_stage_layout', { id });
}

/**
 * POINT ONE STAGE SCREEN AT ONE LAYOUT, or at none.
 *
 * `null` is the way back and a real answer: the screen returns to the zones the
 * DEVICE itself has, which is the arrangement a church may already be running.
 * Nothing is erased by assigning, and nothing is reset by clearing.
 *
 * THROWS (contract group 1). It changes what a preacher sees, and a failure the
 * operator cannot see is a control that lies about what it did.
 */
export async function setChannelStageLayout(channelId, layoutId) {
const call = await invoke();
await call('set_channel_stage_layout', { channelId, layoutId: layoutId ?? null });
}

/**
 * WHICH SCREENS A PLAN CUE IS FOR (RG-161), or every screen.
 *
 * `null` clears the targeting. An empty array reaches NO screen and is NOT the
 * same thing — a cue that goes nowhere is a real thing to ask for, and folding
 * the two together would make it unsayable.
 *
 * It STORES and it fires nothing: the Planner may not reach an output, so this
 * is a fact about the plan and Live acts on it when the cue goes on air.
 *
 * THROWS (contract group 1).
 */
export async function setPlanChannels(id, channels) {
const call = await invoke();
await call('set_plan_channels', { id, channels: channels ?? null });
}

/**
 * PUT A TIMER BACK TO THE LENGTH IT WAS STARTED AT.
 *
 * The third transport verb, and not a Stop: the timer, its label, its chosen
 * warning threshold and its cue binding all survive. It answers how long, never
 * running-or-not — a held timer is reset where it stands and stays held.
 *
 * It names no figure on purpose. The length lives on the registry row
 * (`configured_ms`), because a re-aim moves `target_ms` and leaves `from_ms`, so
 * nothing on this side of the bridge can reconstruct what was originally chosen.
 *
 * THROWS (contract group 1).
 */
export async function resetTimer(timerId) {
const call = await invoke();
await call('reset_timer', { timerId });
}

/**
 * TAKE A TIMER OFF THE REGISTRY. It does not touch a screen — `Clear screens` is
 * how a wall is taken back, and it is one key away at every moment (rule 15).
 *
 * THROWS (contract group 1).
 */
export async function stopTimer(timerId) {
const call = await invoke();
await call('stop_timer', { timerId });
}

/**
 * EVERY TIMER, OLDEST FIRST, WITH HOW LONG IS LEFT ON EACH.
 *
 * THROWS (contract group 1) — and this one is worth saying out loud, because the
 * obvious swallow returns `[]`, which is exactly what a console with no timers
 * renders. A broken bridge would look like a quiet Sunday on the one surface an
 * operator would use to find a clock counting down to the wrong thing.
 *
 * `remaining_ms` on each row is the engine's own figure. The frontend still ticks
 * through `countdown.js::countdownRemainingMs`, which stays the only arithmetic on
 * this side of the bridge.
 */
export async function listTimers() {
const call = await invoke();
return call('list_timers');
}

/**
 * PUT A CONGREGATION TIMER BACK IN FRONT OF PEOPLE — the explicit way back.
 *
 * A timer outlives the content that replaced it now, so after a reading there is
 * something to return to. This is how an operator returns to it, on purpose. It
 * carries whatever the timer says NOW, so what goes back up is the figure in the
 * list rather than the length it started as. A `'stage'` timer is refused by the
 * engine, in words: it has no congregation wire form.
 *
 * THROWS (contract group 1) — it is one of two doors onto a congregation wall, so
 * a failure nobody is told about is an operator believing in a countdown that is
 * not there.
 */
export async function showTimer(timerId, templateId = null) {
const call = await invoke();
await call('show_timer', { timerId, templateId });
}

/** Fire arbitrary content to the screens. `kind` ('song'|'announce') selects the
 *  content-type default template (per-content-type templates). `stageNote` is an
 *  optional Stage Note for this cue, monitors only. `templateId`, when set, is the
 *  cue's OWN template override (Planner) — it wins over the content-type default. */
export async function fireContent(
label,
text,
kind = 'announce',
stageNote = null,
templateId = null,
keepPlan = false,
// WHICH SCREENS (RG-161). `null` is every screen.
channels = null,
) {
const call = await invoke();
await call('fire_content', { label, text, kind, stageNote, templateId, channels });
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
 * WHAT EACH SCREEN WEARS FOR EACH KIND — `{ 1: { scripture: 9, song: 12 } }`.
 *
 * DECISIONS §97. A per-kind look is the SCREEN'S own template for one kind, not
 * an override: it joins one notch above the screen's blanket template, which is
 * what leaves §29 and the transparency law intact.
 *
 * ONE store and ONE writer, the same discipline `contentTemplates` above records
 * and for the reason recorded there: three surfaces each holding their own cached
 * copy of a map is three surfaces that silently disagree about what a screen is
 * wearing, on the desk an operator opens to check exactly that.
 *
 * A screen with no per-kind look is ABSENT from this map rather than present with
 * an empty object, and a kind with no look is absent from its screen's entry —
 * the shape the backend sends, because no row is the only spelling of "this kind
 * inherits" and two spellings of one fact is what the schema already refuses.
 */
export const channelLooks = writable({});

/** Load the per-kind look map into the shared store. */
export async function loadChannelLooks() {
  return guardedRead(
    'loadChannelLooks',
    async (call) => {
      const map = await call('list_channel_looks');
      // AN ANSWER THAT IS NOT AN OBJECT IS A FAILED READ, not "no screen has a
      // per-kind look" — the same distinction `loadTemplates` draws for a
      // non-array, and the same reason: the second reads as a setup nobody has
      // made, over a setup somebody made and cannot see.
      if (!map || typeof map !== 'object' || Array.isArray(map)) {
        throw new Error('the per-kind look map came back as something else');
      }
      channelLooks.set(map);
      return map;
    },
    null,
    // A failed read must not leave the store holding a map the backend can no
    // longer confirm — every surface would go on naming looks for screens out of
    // it. A fallback VALUE cannot carry a side effect, so the reset is explicit;
    // the same shape as `loadDefaultTemplate` above.
    () => channelLooks.set({}),
  );
}

/**
 * THE ONE writer of the per-kind look map. `templateId` of null CLEARS the look
 * for that kind, which means the kind INHERITS — the screen's own template, then
 * the content look, then the configured default.
 *
 * It does NOT mean "this screen skips this kind". That question is answered by
 * `layout.shows` on the template and by the screen's own `shows` set; conflating
 * the two would make a look picker into a routing control, which is RG-161
 * arriving through a door built for something else.
 *
 * GROUP 1 (throws). It is an operator action with a visible result and the
 * backend refuses an unknown kind by name, so a swallowed refusal would leave the
 * picker showing a look the screen does not wear — the same failure
 * `setChannelTemplate` describes, on the setting that decides what a congregation
 * sees. The caller renders it through `src/lib/errors.js`, never as a raw Rust
 * string.
 *
 * Optimistic, then reconciled: the store moves first so every subscribed surface
 * reflects the change instantly, and a failure reloads truth so the UI can never
 * lie about what is stored.
 */
export async function setChannelLook(channelId, kind, templateId) {
  const id = templateId ?? null;
  const before = get(channelLooks);
  channelLooks.update((m) => {
    const mine = { ...(m[channelId] ?? m[String(channelId)] ?? {}) };
    if (id == null) delete mine[kind];
    else mine[kind] = id;
    const next = { ...m };
    // The backend omits a screen with nothing to say about it, so the store does
    // too — one shape, both ends, or `lookIdFor` would have two empties to know
    // about.
    if (Object.keys(mine).length) next[channelId] = mine;
    else {
      delete next[channelId];
      delete next[String(channelId)];
    }
    return next;
  });
  try {
    const call = await invoke();
    await call('set_channel_look', { channelId, kind, templateId: id });
  } catch (e) {
    channelLooks.set(before);
    throw e;
  }
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

// ── Voice profiles (Settings → Preachers) ────────────────────────────────────
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

// ── ONE FACT, ONE STORE (RG-138) ─────────────────────────────────────────────
//
// `voice_profiles.language` is the ONLY place the recognition language lives, and
// `capture.stt.language` is the console's copy of it — what Settings → Before the
// Languages renders, and what the Privacy overview reads. `setSttLanguage` keeps
// them in step; these two did not, and both of them change that column and apply
// it to the live engine:
//
//   - `update_voice_profile` writes the row and calls `apply_profile` when the
//     profile is the active one, so saving the profile editor with a different
//     Language moved the database and the engine and left the other tab's select
//     displaying the language it had just moved away from.
//   - `select_voice_profile` applies the newly-active profile by construction,
//     which is the same disagreement reached by a different door — and it is the
//     door `rooms.js::applyRoom` goes through.
//   - `delete_voice_profile` is the THIRD door, and it is the one that is easiest
//     to miss because nobody chose a language on it: deleting the ACTIVE profile
//     promotes the next remaining one and applies it live (`main.rs`), so the
//     engine moves to a language the operator never picked while Scripture &
//     Languages goes on showing the deleted profile's. This enumeration named two
//     doors when there were three, which is the mistake CLAUDE.md records four
//     times over — *"enumerate every caller of the thing you fixed"*.
//
// `is_active` is the BACKEND'S answer, not the caller's copy of it: `main.rs`
// stamps it from the database after the write, because the payload's own field is
// whatever the frontend happened to be holding.
function noteProfileLanguage(landed) {
if (landed?.is_active)
  capture.update((s) => ({ ...s, stt: { ...s.stt, language: landed.language ?? null } }));
return landed;
}

export async function updateVoiceProfile(profile) {
const call = await invoke();
return noteProfileLanguage(await call('update_voice_profile', { profile }));
}

export async function selectVoiceProfile(id) {
const call = await invoke();
return noteProfileLanguage(await call('select_voice_profile', { id }));
}

export async function deleteVoiceProfile(id) {
const call = await invoke();
return noteProfileLanguage(await call('delete_voice_profile', { id }));
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
export async function fireMedia(id, templateId = null, keepPlan = false, channels = null) {
const call = await invoke();
// WHICH SCREENS (RG-161). `null` is every screen; `[]` is no screen. This
// argument was missing while `fireContent`'s and `manualFire`'s were not, so a
// media cue ignored the `Screens` row the Planner renders for it.
await call('fire_media', { id, templateId, channels });
if (!keepPlan) leavePlan();
}

/**
 * PUT A PICTURE BEHIND THE WORDS — or take it away (`id: null`).
 *
 * GROUP 1: it throws. It changes what every congregation screen is showing, and
 * the take-down is the half a swallowed failure hurts most — the operator believes
 * the backdrop has gone and it is still there behind the next thing they fire.
 *
 * ONE wrapper for both directions, mirroring the one command: a separate
 * `clearBackground` would be a second door onto one piece of state, and this
 * repository's register of that mistake runs to four entries.
 *
 * It does NOT call `leavePlan()`. A backdrop does not replace the cue on the wall,
 * so the plan is exactly where it was — the same reasoning that keeps the passage
 * armed on the Rust side.
 */
export async function showBackground(id = null) {
const call = await invoke();
await call('show_background', { id });
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
/**
 * THE LARGEST FILE THE LIBRARY WILL ACCEPT — and the reason there is a limit.
 *
 * An imported file does not arrive as a path. The webview's `<input type=file>`
 * hands back bytes, so this function builds the WHOLE file as a binary string,
 * then as a base64 string, and Tauri then serialises that string across the IPC
 * bridge for Rust to decode into a third complete copy before writing it to disk.
 * Four copies of one file exist at the peak.
 *
 * For a 1.5 GB service video on a church laptop that is not a slow import. It is
 * the operating system killing Relay, with no error, no message and nothing in
 * any log, while a volunteer is setting up for Sunday. Refusing the file is a
 * worse experience than importing it and an enormously better one than dying.
 *
 * Must equal `main::MAX_IMPORT_BYTES`; `mediaimport.test.js` fails if they drift.
 */
export const MAX_IMPORT_BYTES = 256 * 1024 * 1024;

/** Human size, for the sentence the operator reads. */
function mib(bytes) {
  return `${Math.round((bytes / (1024 * 1024)) * 10) / 10} MB`;
}

/**
 * Read a file as base64 for the import bridge.
 *
 * THE CHOKE POINT. Media, graphics, documents and lyric files all import through
 * here, so the size guard lives here rather than at the four call sites — the same
 * reasoning as `broadcast_with_clock` holding the pre-air validator (CLAUDE.md
 * rule 36). A guard added per call site is a guard that will be missing from the
 * fifth one.
 *
 * Throws a REFUSAL — the shape `humanError` prints verbatim — before allocating
 * anything, because the allocation is the failure.
 */
export async function fileToBase64(file) {
const size = file?.size ?? 0;
if (size > MAX_IMPORT_BYTES) {
  const msg =
    `${file?.name ?? 'That file'} is ${mib(size)}, and Relay imports files up to ` +
    `${mib(MAX_IMPORT_BYTES)}. Shorten or compress it and try again.`;
  throw Object.assign(new Error(msg), { kind: 'refused', message: msg });
}
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
    // THE DATABASE DOOR. Every template is migrated on the way in, so nothing
    // downstream — the gallery, the editor, a save — ever handles a style with
    // two homes for one property (docs/REBRAND.md §3.1). Migrating only at the
    // renderer would keep the WALL correct while the legacy key sat in the
    // database for ever, waiting for the next reader that does not resolve.
    // AN ANSWER THAT IS NOT A LIST IS A FAILED READ, not an empty gallery.
    //
    // This passed a non-array STRAIGHT THROUGH into a store declared
    // `writable([])`, so a backend answering `null` set `$templates = null` and
    // every `$templates.find(...)` in the app threw — `Dock.svelte`'s
    // `cdFallbackTpl` among them, which takes the whole Live audio card down with
    // it. It never fired in anger because the reads it needed were failing
    // earlier for an unrelated reason, and it surfaced the moment they started
    // working (RG-170).
    //
    // Throwing hands it to `guardedRead`, which records `readErrors.loadTemplates`
    // and leaves the store holding what it already had. That is the right pair:
    // the surfaces say the list could not be READ rather than that there is
    // nothing in it, and good data is not replaced by a bad answer.
    if (!Array.isArray(list)) {
      throw new Error(`list_templates answered ${list === null ? 'null' : typeof list}, not a list`);
    }
    const migrated = list.map(migrateTemplate);
    templates.set(migrated);
    return migrated;
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


/** Delete a template; reloads the store. Also refreshes the content-look map —
 *  a content default (or a channel) may have pointed at the deleted template and
 *  the backend nulls those references. */
export async function deleteTemplate(id) {
const call = await invoke();
await call('delete_template', { id });
await loadTemplates();
await loadContentTemplates();
}

// ══ X1 · THE TRANSITION CONTROL (docs/REBRAND.md §8 · DECISIONS §84) ═══════════
// One block, deliberately self-contained: this file is being edited by more than
// one agent this wave, so an integrator can move these lines whole.
//
// GROUP 1 — THROWS. A congregation CAN see the difference: an operator who asked
// for a cut and got an 800 ms crossfade is watching something they turned off.
// The caller (the chrome picker in `App.svelte`) catches and puts the control back
// where it was, so a failed change is visible as the picker refusing to move
// rather than as a preference that quietly did not take.
//
// The STORE is `liveTransition` in `lib/transitions.js`, not here, and that is not
// a drift from the one-store rule: `TemplateRender` reads it and also renders
// `output.html`, which is served to a browser source with no backend. Importing
// this module there would put the whole command surface on a congregation screen.
// The bridge stays here; the value sits beside the register that defines it.
export async function setLiveTransition(mode, ms) {
  const call = await invoke();
  await call('set_live_transition', { mode: mode ?? null, ms: ms ?? null });
  liveTransition.set(mode ? { mode, ms } : null);
}

/**
 * What override the BACKEND says is in force. The console can reload mid-service;
 * the screens do not.
 *
 * GROUP 2, through `guardedRead` — and the guard is the point, not the swallow.
 * A bare `catch` here would make "nobody has overridden anything" and "I could not
 * ask" the same answer, and the second one is the state in which this picker would
 * read `Follow template` over a building full of crossfading screens (rule 35).
 * `readErrors.liveTransition` carries the reason, the way every other read does.
 */
export async function loadLiveTransition() {
  const cur = await guardedRead('liveTransition', (call) => call('live_transition'), null);
  liveTransition.set(
    Array.isArray(cur) && cur[0] ? { mode: cur[0], ms: cur[1] ?? undefined } : null,
  );
}
// ══ end X1 block ══════════════════════════════════════════════════════════════

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

// ── THE COUNTDOWN WARNING WINDOW ───────────────────────────────────────────────
//
// How long before zero a countdown turns red. Shipped as the last minute; an
// operator can move it in Settings → Getting started. Persisted in the settings KV under
// `countdown.warn_ms` and READ, which is the whole point of it: seven controls
// were removed from that page on 2026-09-10 for saving a preference nothing
// opened (DECISIONS §69), and a threshold nobody reads is that defect with a
// congregation-facing colour attached.
//
// The reader is `layers.js::countdownWarning`, through `setCountdownWarnDefault`,
// which is the one rule the wall, the preacher's page and the dock all ask. A
// figure carried by one timer still beats this default — that ranking lives in
// `countdownWarning` and is not restated here.
const COUNTDOWN_WARN_MIN_MS = 5_000;
const COUNTDOWN_WARN_MAX_MS = 60 * 60_000;

/** The warning window in force, in ms. Mirrors what `layers.js` is using. */
export const countdownWarnMs = writable(COUNTDOWN_WARN_MS);

/** A readable window, or the shipped minute. Never zero — a window of zero is a
 *  warning colour that never comes on, on the one surface whose job is to. */
function clampCountdownWarn(ms) {
  const n = Number(ms);
  return Number.isFinite(n) && n > 0
    ? Math.max(COUNTDOWN_WARN_MIN_MS, Math.min(COUNTDOWN_WARN_MAX_MS, Math.round(n)))
    : COUNTDOWN_WARN_MS;
}

/** Apply a figure to the store AND to the rule, so the two cannot come apart. */
function applyCountdownWarn(ms) {
  const ok = clampCountdownWarn(ms);
  setCountdownWarnDefault(ok);
  countdownWarnMs.set(ok);
  return ok;
}

/**
 * Load the configured warning window. GROUP 2 — SWALLOWS: a console that could not
 * ask falls back to the shipped minute, which is what it had before.
 *
 * The fallback is applied OUT HERE rather than through a fourth argument to
 * `guardedRead`, which takes three: `loadDefaultTemplate` and `loadServiceTarget`
 * each pass a reset closure that is silently dropped, so on a failed read their
 * stores keep the last good value while a comment beside them says otherwise.
 * Not fixed here — that is three other surfaces' behaviour — but not copied either.
 */
export async function loadCountdownWarnMs() {
  const raw = await guardedRead(
    'countdownWarnMs',
    (call) => call('get_setting', { key: 'countdown.warn_ms' }),
    null,
  );
  return applyCountdownWarn(parseInt(raw, 10));
}

/**
 * Set the warning window (ms). Persisted in the KV and applied at once, so the
 * dock and the programme pane turn red at the new figure without a relaunch.
 *
 * THE ROW IS WRITTEN FIRST, and only then is the figure applied. The other order
 * moves what the wall does while leaving the row at the old value, so a write that
 * failed would show an operator a setting that is in force this session and gone
 * at the next launch — a control saying one thing and the machine another.
 */
export async function setCountdownWarnMs(ms) {
  const n = clampCountdownWarn(ms);
  const call = await invoke();
  await call('set_setting', { key: 'countdown.warn_ms', value: String(n) });
  return applyCountdownWarn(n);
}


/** All configured output channels. */
export async function listOutputChannels() {
return guardedRead('listOutputChannels', async (call) => {
    return await call('list_output_channels');
}, []);
}

/**
 * THE WORD CURRENTLY SENT TO THE PREACHER — what Relay last put on the stage
 * monitor, or null.
 *
 * A store rather than a component variable, and the reason is the dock: the shell
 * renders it as `{#if !liveFullscreen}<Dock />{/if}`, so pressing Full screen
 * DESTROYS the panel. With the flag living in the component, a word that was still
 * on the preacher's monitor came back as "nothing sent" — which disabled the only
 * control that takes it down. The operator could send a new one and never clear
 * the old one.
 *
 * It is a mirror of what Relay SENT, written only after the call resolves, and it
 * claims nothing more than that.
 */
export const stageAlert = writable(null);

/**
 * Is there a ProPresenter library on this computer already?
 *
 * Returns `[{ path, songs, truncated }]`, best first, and an empty list when there
 * is nothing to offer. **It finds and counts; it imports nothing.**
 *
 * GROUP 2 (swallows). A scan that failed is not worth interrupting anybody for —
 * the operator can still import a folder by hand, which is the path this only
 * shortens. An empty list and a failed scan are deliberately the same answer to
 * the caller, and the surface says "nothing found" for both, because it cannot
 * tell them apart and should not pretend to.
 */
export async function findProPresenter() {
  try {
    const call = await invoke();
    const found = await call('find_propresenter');
    return Array.isArray(found) ? found : [];
  } catch {
    return [];
  }
}

/**
 * WHAT THE OPERATOR HAS ASKED THE CLIP TO DO — `{ paused, loop }`.
 *
 * A mirror of what Relay SENT, and nothing more. **It is not evidence that a
 * screen obeyed**: the screens report where their clip actually is on the beat,
 * and `describeMediaClock` reads the effect from that. A control that reported its
 * own instruction back as an outcome is rule 35 with extra steps, which is exactly
 * what this store would become if a readout were derived from it.
 */
export const mediaTransport = writable({ paused: false, loop: false });

/**
 * Hold the clip, loop it, or start it again.
 *
 * Each field is a re-aim in `adjust_countdown`'s sense: omit one and it is left
 * alone, so Pause cannot un-loop and Loop cannot un-pause. An operator presses one
 * control at a time and the others have to survive it.
 *
 * `replay` is an event rather than a state and the engine carries it as a counter,
 * so pressing it twice on a clip already at its start is two instructions rather
 * than one frame sent twice.
 *
 * GROUP 1 (throws). The congregation can see the difference: a Pause that failed
 * silently leaves a clip running under an operator who believes they stopped it,
 * and the next cue goes out over the top of it.
 */
export async function setMediaTransport({ paused, loop, replay } = {}) {
  const call = await invoke();
  await call('set_media_transport', {
    paused: paused ?? null,
    // `looping` across the bridge: the wire says `loop` and Rust cannot.
    looping: loop ?? null,
    replay: replay ?? null,
  });
  // After, never before, and only what was actually asked for.
  mediaTransport.update((t) => ({
    paused: replay ? false : (paused ?? t.paused),
    loop: loop ?? t.loop,
  }));
}

/**
 * WHAT THE PREACHER'S OWN SCREEN IS HOLDING — the media id, or `null`.
 *
 * A mirror of what Relay SENT, written only after the call resolves, and claiming
 * nothing more than that. The hub records nothing about who connected
 * (DECISIONS §35), so this can never be a claim that a screen is painting it.
 */
export const stageMedia = writable(null);

/**
 * Put a slide on the preacher's screen, or take it off (`null`).
 *
 * An announcement to read out, or the preacher's own deck. **Not a background**:
 * `showBackground` puts the church's picture behind the words on every screen,
 * and this puts one person's reference material on one screen.
 *
 * Scripture overrides it on the device and does not remove it, so the slide comes
 * back when the reading is cleared rather than needing a second push.
 *
 * GROUP 1 (throws). Same reasoning as `sendStageAlert` beside it: the operator is
 * putting something in front of a person and is looking at the result, and a
 * swallowed failure leaves them believing the preacher can see something they
 * cannot.
 */
export async function sendStageMedia(id) {
  const call = await invoke();
  await call('send_stage_media', { id: id ?? null });
  // After, never before: a failed send must not leave a control claiming a slide
  // is on the stage screen.
  stageMedia.set(id ?? null);
}

/**
 * The Stage Message — one line, the whole stage monitor, and no other
 * screen (docs/REBRAND.md §5). Empty or whitespace clears it.
 *
 * GROUP 1 (throws). The operator is sending a message to a person and is looking
 * at the result; a failure that is swallowed leaves them believing the preacher
 * has been told something they have not.
 */
export async function sendStageAlert(text) {
const call = await invoke();
await call('send_stage_alert', { text: text ?? null });
// After, never before: a failed send must not leave the dock saying a word is on
// the preacher's monitor.
stageAlert.set(text?.trim() ? text.trim() : null);
}

/**
 * Assign a template to a screen — or `null`, which means THIS SCREEN HAS NO LOOK
 * OF ITS OWN and follows the content look (DECISIONS §70).
 *
 * GROUP 1 (throws). It is an operator action with a visible result; a failure
 * that is swallowed leaves the picker showing a look the screen is not wearing.
 */
export async function setChannelTemplate(id, templateId) {
const call = await invoke();
await call('set_channel_template', { id, templateId: templateId ?? null });
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

/**
 * One chapter's verses, in order.
 *
 * GUARDED (RG-95). It used to swallow into a bare `catch {}`, so a chapter that
 * failed to load was indistinguishable from a chapter with no verses in it — and
 * the Bible pane's sentence for that is *"That chapter is empty"*, which is a
 * claim about scripture rather than about the database.
 */
export async function chapterVerses(book, chapter) {
return guardedRead('chapterVerses', async (call) => {
    return await call('chapter_verses', { book, chapter });
}, []);
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
/** Interface enumeration. Throws so a failed refresh cannot look successful. */
export async function networkAddresses() {
  const call = await invoke();
  return await call('network_addresses');
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

/**
 * Open a channel's output on its assigned display (HDMI). Returns the label.
 *
 * REFUSES IN SAFE MODE, here rather than at the four call sites (Channels'
 * Open button, the first-run wizard, the Dashboard's Open main screen, and
 * whatever is written next). Safe mode's row says "outputs will not open", and
 * that clause is an ONGOING promise, not a one-off transition — `applySafeMode`
 * closes what is already open, and this is what keeps it closed. Channels.svelte
 * did not import `safeMode` at all, so the Outputs workspace opened a projector
 * window with safe mode on, which is the capability `degraded.js` reports as
 * blocked. DECISIONS §86.
 *
 * It REFUSES rather than quietly doing nothing: a button that silently no-ops is
 * the defect this whole area was fixed for. All four callers already humanise
 * what they catch, so the sentence reaches the operator wherever they pressed.
 */
export async function openChannelOutput(channelId) {
if (get(safeMode)) {
  const msg =
    'Safe mode is on, so Relay will not open an output screen. ' +
    'Turn it off in Settings → Before the service if you want screens back.';
  throw Object.assign(new Error(msg), { kind: 'refused', message: msg });
}
const call = await invoke(); // throws in browser
return call('open_channel_output', { channelId });
}

/** Re-open the physical output windows assigned to a display, so HDMI/projector
 *  screens restore themselves after a launch/update/rebuild. Backend only opens
 *  onto connected, non-primary displays, so it never covers the operator's
 *  console. Best-effort — a plain browser (no backend) just no-ops. */
export async function autoOpenOutputs() {
// The OTHER way a screen opens, and it is a different backend command, so the
// refusal in `openChannelOutput` does not cover it. No refusal here: nobody
// pressed anything, this runs from the launch sequence, and not restoring the
// screens IS what safe mode means. `App.svelte` states the same thing at the
// call site; this is the half that a new caller inherits. DECISIONS §86.
if (get(safeMode)) return null;
try {
  const call = await invoke();
  return await call('auto_open_outputs');
} catch {
  return [];
}
}

/**
 * WHAT A SCREEN IS FOR — `'main'`, `'stage'`, or `null` for no special role.
 *
 * GROUP 1 (throws), and the throw is the point. The backend refuses a second main
 * screen by name ("Main screen is already the main screen…"), and a refusal that
 * is swallowed leaves the picker showing a role the screen does not hold — the
 * same failure `setChannelTemplate` describes, on the setting that decides
 * whether a word meant for the preacher may be painted. The caller renders it
 * through `src/lib/errors.js`, never as a raw Rust string.
 */
export async function setChannelRole(id, role) {
const call = await invoke();
await call('set_channel_role', { id, role: role || null });
}

/**
 * WHICH KINDS OF CONTENT A SCREEN SHOWS AT ALL — `null` for NO OPINION.
 *
 * DECISIONS §98. `null` clears the column and means "follow the template", which
 * is what every install did before this existed. An EMPTY ARRAY is a different
 * thing and is sent as one: an operator who unticks every kind has said this
 * screen shows nothing, and the two must not collapse into each other — one of
 * them is a screen that paints everything its template allows and the other is a
 * screen that paints nothing.
 *
 * It can only NARROW. The backend stores it, the output page ANDs it with the
 * template's own `layout.shows`, and nothing anywhere lets it force a template to
 * paint a kind it has no regions for.
 *
 * It is NEVER on the panic path. `clearScreens` and `blackout` address every
 * screen and ask nothing about which; a screen an operator could configure out of
 * one is rule 15's exact failure.
 *
 * GROUP 1 (throws). The backend refuses an unknown kind by name, and a swallowed
 * refusal would leave the checkboxes showing a set the screen does not have — on
 * the control that decides whether a congregation sees something at all.
 */
export async function setChannelShows(id, kinds) {
  const call = await invoke();
  await call('set_channel_shows', { id, kinds: kinds ?? null });
}

/**
 * RENAME A SCREEN.
 *
 * GROUP 1 (throws). It is an operator action with a visible result and the
 * backend refuses a blank name, an over-long one and a screen that has been
 * deleted on another surface — each in a sentence. A swallowed refusal would
 * leave the field showing a name the screen does not have, which is worse than
 * the rename failing, because the desk is where everybody else in the building
 * looks that name up. The caller renders it through `src/lib/errors.js`.
 */
export async function renameChannel(id, name) {
  const call = await invoke();
  await call('rename_channel', { id, name });
}

/**
 * TAKE ONE SCREEN OUT OF THE WALL, OR PUT IT BACK.
 *
 * Three wrappers, and they are NOT panic controls. `clearScreens` and `blackout`
 * are — first, largest, one action, every screen, and they never ask which (rule
 * 15, DECISIONS §20). These are the ordinary control beside them: "take the lobby
 * TV down but leave the wall live". Nothing here is bound to a key, and nothing
 * here goes near `panicError`.
 *
 * GROUP 1 (throws), all three. Every one of them is an operator action with a
 * visible result, and a failure that is swallowed leaves the Outputs desk saying
 * a screen is down while a congregation is looking at it — the worse half of the
 * two ways this can go wrong. The backend refuses during a rehearsal, by name,
 * and the caller renders that through `src/lib/errors.js` like every other
 * refusal on that desk. Never a raw Rust string.
 */
export async function clearScreen(channelId) {
  const call = await invoke();
  await call('clear_screen', { channelId });
}

/** Blackout ONE screen (opaque), leaving every other screen as it is. */
export async function blackoutScreen(channelId) {
  const call = await invoke();
  await call('blackout_screen', { channelId });
}

/**
 * Put one screen back into the wall: it shows whatever the wall is showing.
 *
 * The way back is a CONTROL and not a side effect of the next fire. A screen
 * taken down stays down across every fire in between — a one-shot would be undone
 * within a minute of being used — so there has to be something that undoes it,
 * and it has to be as easy to find as the control that did it.
 */
export async function restoreScreen(channelId) {
  const call = await invoke();
  await call('restore_screen', { channelId });
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
// ── PRACTICE (RG-16) ─────────────────────────────────────────────────────────
//
// One stream of "the operator just did something", for the drills to watch. It
// exists because the alternative is `training.js` importing four listeners of its
// own, which would be a second set of subscriptions to the same events — and the
// two would drift about what counts as a clear.
//
// Deliberately a plain event bus and not a store of state: a drill is satisfied by
// an ACTION, and an action is a moment, not a value that can be read later.
const operatorActions = new Set();
export function onOperatorAction(fn) {
  operatorActions.add(fn);
  return () => operatorActions.delete(fn);
}
export function noteOperatorAction(kind, payload = null) {
  for (const fn of operatorActions) {
    try {
      fn({ kind, payload });
    } catch {
      // A practice panel that threw must never take a live control with it.
    }
  }
}

export const channelHealth = writable({}); // channel id → ChannelLiveness
/** When each channel was first seen attached but not answering. */
export const channelWaiting = writable({});
let healthPoll = null;

/**
 * Ask the backend what each screen is doing, once, now.
 *
 * The 2-second poller below covers the steady state. This exists for the moment
 * an operator has just switched a screen on or off and needs the pane to answer
 * from the backend rather than from the console's assumption that the command
 * that returned did the thing.
 */
export async function refreshChannelHealth() {
  try {
    await pollChannelHealth();
  } catch {
    // GROUP 2: a health read never throws at a caller. The pane going stale is
    // the correct failure — it degrades toward "not answering", never toward
    // "all is well" (outputHealth.js rule 2).
  }
}

async function pollChannelHealth() {
  // `channelStatus()` swallows a missing backend and returns `[]`, but it cannot
  // vouch for the SHAPE of an answer that did arrive — and this ran bare on a
  // 2000 ms timer (below), so a `channel_status` payload that was not an array
  // threw `TypeError: rows is not iterable` as an unhandled rejection, which
  // `crash.js` turned into the full-screen crash panel. Every two seconds. For the
  // rest of the service. Reproduced in a browser against a running console.
  //
  // Degrade toward "not answering", never toward "all is well" — the guarantee
  // `refreshChannelHealth` states twenty lines up and this door did not keep.
  const answer = await channelStatus();
  const rows = Array.isArray(answer) ? answer : [];
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
  // Through the wrapped door, both times. `pollChannelHealth` is guarded from the
  // inside now, but a health poll must not be the one caller that can raise an
  // unhandled rejection at the shell — there is exactly one door out of this
  // module and this is it (rule 36's reasoning, applied to a timer).
  refreshChannelHealth();
  healthPoll = setInterval(refreshChannelHealth, 2000);
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
// The DRILL is "you pressed the transport", and that is true whichever outcome
// came back — reaching the end of a passage is a correct answer, not a miss.
noteOperatorAction('nav', outcome);
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
 * The reason safe mode could not keep its promise, humanised — or null.
 *
 * Module scope, and set by `applySafeMode` itself rather than returned to the
 * caller, for the same reason `panicError` is: the control that flips safe mode
 * can be a view that has crashed, and a view that cannot `catch` cannot report.
 */
export const safeModeError = writable(null);

/**
 * TURN SAFE MODE ON OR OFF, AND MAKE THE PROMISE TRUE. (GROUP 3.)
 *
 * Safe mode's row says "outputs will not open and detection is disarmed —
 * nothing Relay does can reach a screen". `setSafeMode` writes that into the
 * boot record; for as long as it was the only thing that happened, the sentence
 * was false until the next launch — App.svelte honoured it inside onMount only,
 * Live.svelte never mentioned it, and Rust has no notion of it.
 *
 * So the enforcement lives HERE, at the one door, and not as a `$safeMode` check
 * at each fire site. This repository has had four separate bugs whose single
 * root cause is a rule enforced on one surface and skipped on its twin; a check
 * per caller would be the fifth. DECISIONS §86.
 *
 * Returns whether the promise was kept, AND sets `safeModeError`. Turning safe
 * mode OFF restores the operator's freedom to arm things and deliberately arms
 * nothing for them: a detector that switches itself back on is a different
 * surprise from the one this control prevents.
 *
 * EVERY screen is attempted, even after one refuses to close. Stopping at the
 * first failure would leave the operator reading one screen's name while the
 * ones behind it are still lit and nothing ever asked them to go dark; the
 * message names each one that would not, so what is left to do by hand is the
 * whole of it.
 */
export async function applySafeMode(on) {
  safeModeError.set(null);
  setSafeMode(on);
  if (!on) return true;

  const failures = [];

  try {
    await setDetection(false);
  } catch (e) {
    failures.push(humanError(e));
  }

  // TAKE THE SCREENS DOWN FIRST, and not only the ones with a window.
  // `close_channel_output` closes a native webview; a channel with no window is
  // a silent no-op there, so an OBS browser source or a lobby TV on the kiosk hub
  // would keep its RETAINED frame (rule 43) and go on showing the last verse
  // while this function returned true. `clear_screens` reaches every render
  // target and its `clear` becomes the retained frame in its turn, so a screen
  // that reconnects afterwards comes back blank rather than to the verse.
  //
  // Deliberate, and worth being deliberate about: safe mode takes a congregation's
  // screen down. It is an explicit operator action asking for exactly that, not
  // something Relay decides on its own — which is the line §20 draws.
  if (!(await clearScreens())) {
    failures.push('the screens could not be cleared');
  }

  // `listOutputChannels` is GROUP 2: it swallows and returns `[]`, so a `catch`
  // around it can never fire and a door that trusted the empty list would report
  // "every screen closed" having never been told about one. The swallowed reason
  // is in `readErrors`, which is exactly what that store exists for.
  const chans = await listOutputChannels();
  const listFailed = get(readErrors).listOutputChannels;
  if (listFailed) {
    failures.push(`the list of screens could not be read (${humanError(listFailed)})`);
  }
  for (const c of chans ?? []) {
    try {
      await closeChannelOutput(c.id);
    } catch (e) {
      failures.push(`${c.name ?? `screen ${c.id}`}: ${humanError(e)}`);
    }
  }

  if (failures.length) {
    safeModeError.set(
      `Safe mode is recorded, and it could not be enforced: ${failures.join('; ')}. ` +
        'Something may still be able to reach a screen.',
    );
    return false;
  }
  return true;
}

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
async function guardedRead(key, run, fallback, onFail) {
try {
  const value = await run(await invoke());
  readErrors.update((m) => (m[key] ? { ...m, [key]: null } : m));
  return value;
} catch (e) {
  readErrors.update((m) => ({ ...m, [key]: e }));
  // THE FOURTH ARGUMENT WAS BEING DROPPED ON THE FLOOR, AND TWO CALL SITES WERE
  // ALREADY PASSING IT. A fallback VALUE cannot carry a side effect: a read that
  // populates a store has nothing to hand back, so letting go of the stale value
  // is something the catch has to DO. Without this, `loadDefaultTemplate` left
  // `defaultTemplateId` holding an id the backend could no longer confirm and
  // `loadServiceTarget` left the stopwatch counting against a length nobody had
  // answered for — each under a comment saying the reset was explicit. Pinned by
  // `readstates.test.js`, "a failed read resets the store its call site asked to
  // reset". Optional: most reads degrade to a value and want nothing here.
  if (typeof onFail === 'function') onFail();
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
  // AND THE CONSOLE'S MIRROR OF THE STAGE MESSAGE GOES WITH IT (RG-145).
  // DECISIONS §91: a panic control takes back every sentence anybody put on a
  // screen, so `Stage.svelte` clears the alert on both controls. This store is what
  // Quick tools paints its "on stage" badge and its Take down button from, so
  // leaving it set offered the operator a control for a word that was already down,
  // under a comment claiming the badge says what the monitor is painting right now.
  // After the call resolves and never before — the same discipline `sendStageAlert`
  // keeps — because a panic that FAILED has taken nothing off any screen, and a
  // console that said otherwise is rule 15 one surface along.
  stageAlert.set(null);
  return true;
} catch (e) {
  // In a plain browser there is no backend AND no output screen, so there is
  // nothing to warn about — don't cry wolf in a dev tab.
  if (get(capture).available) {
    // `humanError`, not `String(e)`. Every Tauri command on this path returns the
    // typed bridge error (`error.rs`, `{ kind, message }`), and `String({…})` is
    // "[object Object]" — so the most serious sentence in the product ended
    // "…clear it there. ([object Object])". Six other files already document this
    // trap; the panic path was the one door nobody checked, which is rule 15 and
    // "a guarantee is only kept on the doors you checked" arriving together.
    panicError.set(
      `${label} FAILED — the congregation may still be seeing the last thing you put up. ` +
        `Check the output screen and clear it there. (${humanError(e)})`,
    );
  }
  return false;
}
}

/**
 * Operator has read the safe-mode warning.
 *
 * Says "I have looked", never "it is fixed" — same contract as
 * `dismissPanicError`. It clears again on the next `applySafeMode`, which is the
 * only thing that can honestly say the promise is being kept.
 */
export function dismissSafeModeError() {
safeModeError.set(null);
}

/** Operator has read the panic warning (or a later panic control succeeded). */
export function dismissPanicError() {
panicError.set(null);
}

/**
 * Acknowledge an audio device failure (RG-117).
 *
 * Dismissing says "I have read this", never "it is fixed": capture is already
 * stopped by the time this banner exists, and the way back is to plug the
 * microphone in and press start. It clears again on the next successful start.
 */
export function dismissAudioError() {
capture.update((s) => ({ ...s, audioError: null }));
}

/** Push the "Up Next" preview to the stage/confidence monitor (null clears).
 *
 *  GROUP 1 (THROWS), moved out of GROUP 2 on 2026-08-14 (R5-8) — and the reason is
 *  a correction to the group rule itself, not just to this wrapper.
 *
 *  GROUP 2's test is *"can the congregation see the difference?"*. For this call the
 *  honest answer is **no, but the preacher can, and he is the one acting on it.**
 *  The stage monitor is a real screen on a stand in front of a person, and
 *  `setStageNext(null, null)` is how the "Up Next" panel comes DOWN. A swallowed
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

/** Set the recognition language: a code ("yo"/"sw"/"ha"/"en") or null for
 *  auto-detect. Returns the voice profile it was written to.
 *
 *  GROUP 1 — THROWS, and it was in GROUP 2 until RG-138. It changes what the AI
 *  hears, which is the same class as `setDetection`, and it is now also a WRITE:
 *  the language is stored on the active voice profile, which is what makes the
 *  choice survive a relaunch. A swallowed failure here leaves the select showing
 *  a language nothing was told about — on the one control RG-116 names as the
 *  mitigation for a service lost to whisper's language election wandering.
 *
 *  `rooms.js` already depended on this throwing: its per-step report exists to say
 *  which pieces of a room did not come back, and a wrapper that cannot fail
 *  reported "recognition language" as applied every time, unconditionally. */
export async function setSttLanguage(language) {
const call = await invoke();
const profile = await call('set_stt_language', { language: language ?? null });
// Only after the backend agreed. An optimistic update is the same lie one step
// earlier: the select would move and nothing would have been stored.
capture.update((s) => ({ ...s, stt: { ...s.stt, language: language ?? null } }));
return profile ?? null;
}

/** The single operator sensitivity dial (0..100), read from the live thresholds.
 *  One forward mapping (`from_sensitivity`) and its inverse both live in Rust —
 *  the frontend never duplicates the curve. */
export async function getSensitivity() {
try {
  const call = await invoke();
  const sensitivity = await call('get_sensitivity');
  // THE ANSWER GOES IN THE STORE, and the store records that an answer arrived.
  // This used to return the number to one caller and tell nothing else, which is
  // how the dock came to hold the only copy of it.
  if (Number.isFinite(Number(sensitivity))) {
    capture.update((st) => ({
      ...st,
      sensitivity: Number(sensitivity),
      sensitivityKnown: true,
    }));
  }
  return sensitivity;
} catch {
  // 50 is still returned for a caller that wants a number, and `sensitivityKnown`
  // stays false so nothing can mistake this fallback for a reading. The two facts
  // are kept apart deliberately: a substitute "unknown" VALUE would put a figure
  // on screen that is nobody's setting, which is a second lie covering the first.
  return 50;
}
}
/** Set sensitivity (0..100) — THE one way the gate is set by hand, reached from
 *  the dock's card on Live and from Settings → AI & Detection. Two doors onto one
 *  control: the pair of Settings sliders that used to be the other way of doing
 *  this were a second control over the same fact, pointing the opposite way, and
 *  were deleted with their command (DECISIONS §96).
 *
 *  Keeps the local `thresholds` mirror in step. Returns the LANDED dial position.
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
const gate = await call('get_thresholds');
// `detection://thresholds` will say the same thing a moment later and this is not
// redundant with it: the event is how OTHER surfaces find out, and this is how the
// surface that just acted stops showing a stale figure between the command
// returning and the event arriving.
//
// `on_dial` comes back true here by construction — the dial is the one control
// that puts the gate ON its own curve — and it is read from the answer rather
// than assumed, because assuming it is how a surface comes to report the
// reassuring case over a state nobody checked.
capture.update((s) => ({
  ...s,
  thresholds: { auto_fire: gate.auto_fire, suggest: gate.suggest },
  sensitivity: Number.isFinite(Number(landed)) ? Number(landed) : s.sensitivity,
  sensitivityKnown: s.sensitivityKnown || Number.isFinite(Number(landed)),
  gateOnDial: gate.on_dial !== false,
}));
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
// GROUP 2 THROUGH `guardedRead`, and the reason is not tidiness. This swallowed
// into a bare `catch` and returned the safe default, which Settings takes as the
// truth: `savedDsn` became `''`. Flipping the switch then sent `('', true)`, and
// `set_crash_reporting` writes the string unconditionally — so a read that failed
// on mount DESTROYED the stored DSN one click later, and the operator's only clue
// was an address field that had gone empty. (Nothing leaked: `telemetry::enable`
// returns early on an empty DSN.) The reason now lands in `readErrors`, which is
// what Settings disables the switch and Save on.
return guardedRead('getCrashReporting', async (call) => {
    return await call('get_crash_reporting');
}, { enabled: false, dsn: '' });
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
