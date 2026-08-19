# Commit Log

Full history, 92 commits, oldest to newest. Generated 2026-08-19.

## a35d7d2 — 2026-07-02
devgeereact

Phase 0: green baseline — scaffold builds on both toolchains

Fix the hand-written skeleton so `cargo check` and `vite build` both pass:
- Add missing src-tauri/build.rs (tauri_build::build) — was causing
  "OUT_DIR env var is not set".
- Add src-tauri/capabilities/default.json (Tauri v2 permission system,
  core:default on the main window).
- Generate the full app icon set from a placeholder Relay brand mark
  (orange tile + R on charcoal, matching docs/design mockups) and wire
  bundle.icon in tauri.conf.json — was failing on missing icons/icon.png.
- Commit Cargo.lock (application, not library → reproducible builds);
  update .gitignore comment accordingly.

Rust core + Svelte frontend now compile clean (stub dead-code warnings
only). Git initialized on main.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 3362ea2 — 2026-07-02
devgeereact

Phase 1: Tauri shell boots — real 5-screen console UI

Rebuild the placeholder App.svelte into the full operator console from
docs/design/relay-app-screens.html, as componentized Svelte:
- App.svelte shell: topbar (brand, live/standby pill, listening indicator,
  live en-GB clock), numbered tab strip, view routing via svelte:component.
- Five screens as lib/views/*: Console (transcript + AI detection cards +
  first-class manual-override search + channel previews), Channels, Templates
  editor, Library, Settings (thresholds, tier-1 language priority, kiosks).
- app.css: full design system ported to a global sheet (tokens + shared
  component classes) so Svelte scoping doesn't strip reused styles.
- Fonts self-hosted via @fontsource (offline-first — no Google Fonts CDN,
  which the mockup used; woff2 now bundled by Vite).
- App.svelte probes the Tauri `greet` bridge on mount; drives live vs
  standby pill, falls back quietly under plain vite (browser preview).

Verified: `vite build` clean, `cargo tauri dev` compiles and the native
window runs (target/debug/relay). Static demo data only — live pipeline
wiring lands Phase 2+. Also gitignore .idea/.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 1378cd9 — 2026-07-02
devgeereact

Phase 2: SQLite data layer — db.rs + verified KJV dev seed

Build out db.rs against docs/data/schema.sql (single-responsibility
persistence; nothing else touches SQLite directly):
- open(): per-OS app-data path (RELAY_DB_PATH override for dev/tests),
  creates parent dir, applies schema + seed on first run only, idempotent
  on reopen. Schema baked in via include_str! (offline-first — no runtime
  dependency on the docs/ file shipping beside the binary).
- lookup_verse() / verse_count() query helpers + VerseRow (Serialize) for
  the Tauri bridge; errors returned, never panicked (live-path safety).
- Curated, verbatim, public-domain KJV seed: Genesis 1:1-3, full Psalm 23
  (complete so Phase 9 context-memory can resolve a bare "verse 4"),
  John 3:16-17, Romans 8:28/31/38-39. Deliberately NOT hand-typing the
  full Bible — that's a later *sourced* import, not scripture typed from
  memory in a product whose job is showing the right verse.

main.rs: open DB at startup (loud failure by design, not mid-service),
manage as Mutex<Connection> Tauri state, expose lookup_verse + data_health
commands alongside the greet bridge probe.

5 tests pass (verbatim John 3:16, complete Psalm 23, missing-verse None,
real on-disk create + idempotent reopen). rustfmt + clippy clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## dfeb987 — 2026-07-02
devgeereact

Phase 3: audio capture + VAD, on assigned port 5032

Audio engine (audio.rs) — real cross-platform capture via cpal:
- list_input_devices(): enumerate inputs, flag the default.
- AudioEngine: one dedicated thread owns the non-Send cpal stream; the
  realtime callback only downmixes to mono + forwards, DSP runs off the
  realtime path. start() blocks until the stream is confirmed playing;
  stop()/Drop tear it down cleanly. No unwrap() on the running path.
- Pure, hardware-free DSP seams: rms(), Vad (energy gate, seam for a real
  webrtc/silero VAD later), Chunker (400ms/200ms overlapping chunks,
  sample-count timestamps). 6 unit tests cover these.
- Handles F32/I16/U16 sample formats; downmixes any channel count to mono.

main.rs: AudioEngine in managed state; list_audio_devices / start_capture /
stop_capture commands; each chunk emitted to the UI as `audio://chunk`
(metadata only — raw samples go to STT via a separate path in Phase 4).

Frontend: lib/stores/capture.js bridges the commands + event stream and
degrades gracefully with no backend (browser preview). Settings audio panel
now lists real devices, starts/stops capture, and shows a live RMS level
meter + voice/silence VAD state; topbar listening indicator reflects real
capture. 11 Rust tests pass, rustfmt + clippy clean, vite build clean.

Ports: Relay claims registry slot NN=03 — app pinned to 5032 (Vite
strictPort), kiosk/WS api reserved 8031. tauri.conf devUrl updated to match.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 2f31b7e — 2026-07-02
devgeereact

Phase 4: local STT — live English transcript from whisper.cpp

stt.rs — streaming speech-to-text on a dedicated worker thread:
- try_load(): load a ggml whisper model via whisper-rs; missing model is a
  soft failure (audio-only mode), never a crash.
- Worker accumulates VOICED audio into a rolling ~8s window, re-transcribes
  every ~1s (partial results), and finalizes an utterance after ~1s of
  silence — the standard whisper streaming shape. whisper's blocking full()
  stays off the audio + UI threads.
- resample_linear(): device-rate → 16kHz mono seam (linear now; windowed-sinc
  is the drop-in quality upgrade). Pure + unit-tested.
- default_model_path(): RELAY_MODEL_PATH override → repo-local dev model
  (compile-time path) → app-data dir.

main.rs: load STT in the Tauri setup hook (worker needs an AppHandle to emit);
start_capture forwards each chunk to the persistent STT worker; transcript
updates emitted as `stt://transcript`; stt_status command for Settings.

Frontend: capture store gains a transcript feed (partials + silence-delimited
finals, history kept) and STT status. Console transcript panel is now LIVE;
Settings shows real model load state. Graceful audio-only / no-backend paths.

Model (ggml-base.en, 147MB) is gitignored + documented in README dev setup;
CMake now a build prerequisite (whisper.cpp). 15 Rust tests pass (4 new
resample), clippy + fmt clean, vite build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 73f3b9a — 2026-07-02
devgeereact

Phase 5: direct-match detection — speak a reference, see the verse

detection.rs — direct scripture-reference detection, DB/IO-free and fully
unit-tested (12 tests):
- Parses written and spoken forms: "John 3:16", "John three sixteen",
  "Romans chapter eight verse twenty-eight", "psalm one hundred nineteen".
- Spoken-number FSM resolves the key ambiguity: "three sixteen" → 3:16
  (chapter 3, verse 16), NOT nineteen; "twenty eight" → 28; hundreds work.
- Book-alias table (canonical DB spelling → variants), multilingual-ready
  for the tier-1 languages; conservative ASR-homophone tolerance
  ("free" → three) with a confidence penalty.
- Confidence heuristic by form (colon-digit > keywords > bare digits >
  spoken); real auto-fire/suggest gating is the router's job (Phase 6).

Pipeline: main.rs runs detect_direct on each STT transcript update, resolves
each reference against the seeded corpus (db::lookup_verse), and emits
`detection://match` (with verse text + in_library flag). detection.rs stays
pure — main.rs is the composition root that joins it to the DB.

Frontend: detections store (de-dup by reference, most-recent-first); Console
AI-detection list is now LIVE — real cards with confidence meter, method,
and the resolved verse text; falls back to the demo cards at rest.

27 Rust tests pass, clippy + fmt clean, vite build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## f1eff80 — 2026-07-02
devgeereact

Phase 6: content router — gating, debounce, manual override

router.rs — turns raw detections into fire/suggest/drop decisions (7 tests):
- Two-tier gate: auto-fire >=0.90, suggest >=0.60 (seed defaults, config not
  constants per DECISIONS.md).
- ~5s repeat-debounce on the same verse; a new verse fires immediately; an
  explicit (>=0.95) direct match overrides the cooldown instantly.
- manual_fire(): operator override always wins, bypassing gate + debounce —
  first-class control, never a fallback (CLAUDE.md).
- record_feedback(): self-calibrating nudge — confirming a suggestion loosens
  `suggest`, rejecting an auto-fire tightens `auto_fire`; bounded, invariant
  auto_fire >= suggest preserved. Seed of the per-install calibration.

Pipeline: detections now route through the gate before surfacing; dropped
ones are silent. DetectionEvent carries status (auto/suggested/manual).
Commands: confirm_detection / dismiss_detection (feed calibration, return
updated thresholds), get/set_thresholds, manual_fire (parse + resolve + fire).

Frontend: detection cards styled by status with working Confirm (promote +
calibrate) / Undo (dismiss + calibrate); manual-override search box fires a
typed reference on Enter; Settings threshold sliders are live and reflect
auto-calibration. 34 Rust tests pass, clippy + fmt clean, vite build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 55c71e6 — 2026-07-02
devgeereact

Phase 7: output channels — native fullscreen render target

channels.rs — the native_window render target: opens a borderless fullscreen
webview per output channel, loading the shared output view with a template id
in the query. One `output://content` broadcast fans out to N windows; each
renders through its own template — no per-channel-type branching (CLAUDE.md).
- open_native_window / close_window / list_open (output-* labels)
- broadcast_content + clear (operator "Clear all screens")
- output_url() pure + unit-tested (2 tests)

Shared template engine (frontend): src/lib/templates.js defines the 4 built-in
templates (main/stage/stream/lobby) per SPEC §5 shape; src/Output.svelte is ONE
data-driven renderer that interprets a template config — genuinely different
fullscreen renders from the same content event. Second Vite entry (output.html)
+ shared fonts (lib/fonts.js).

Pipeline: every fire path now pushes to output — auto-fire in the gate,
manual override, and operator Confirm all broadcast content. Capabilities
extended to the output-* window glob.

UI: Channels tab "Open" launches a real native output window on that channel's
template; Console gains working "Clear all screens" (+ Esc) and "Open output
screen". 36 Rust tests pass, clippy + fmt clean, vite build clean (2 entries).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 76bf872 — 2026-07-02
devgeereact

Phase 8: template engine — DB-backed, editable, live-updating

Templates now live in SQLite and drive the shared renderer:
- db.rs: Template model (id/name + opaque layout & style JSON blobs so the
  shape stays editable without migrations); list/get/upsert queries; seed the
  4 built-ins (Classic Serif / Stage Mono / Lower Third / Lobby Warm). Forward-
  fills templates on open for DBs created before Phase 8. 3 new tests.
- main.rs: list_templates / get_template / save_template commands; saving
  broadcasts `template://updated`.
- channels.rs: output window URL now carries template_id (looked up in the DB
  by the window), not a hardcoded key.

Frontend: Output.svelte fetches its template by id and re-fetches on
`template://updated` — so editing a template updates any open output window
LIVE. Templates tab is a real editor (name, regions, typeface, background,
accent, alignment, ref-first, lower-third, italic) with a live preview and
Save. Channels/Console resolve template ids from the DB list. templates.js is
now just the browser-preview fallback.

Proves the core design principle end to end: two channels pointing at two
templates render the same detection differently, from ONE renderer — no
per-channel branching. 39 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 13b9e18 — 2026-07-02
devgeereact

Phase 9: semantic match + context memory

detection.rs — paraphrase detection and "current passage" state (5 new tests,
17 total in module; stays DB/IO-free and fully unit-tested):
- ContextMemory: tracks last-referenced book+chapter; resolve_bare_verse()
  turns a heard "verse 4" into a full reference. detect_bare_verses() reads
  "verse <n>" (digits or words).
- SemanticIndex: TF-IDF bag-of-words embedding + cosine top_k over the verse
  corpus — a real vector embedding/search that runs fully offline with no
  model. A neural sentence-embedder is a drop-in behind the same top_k seam
  (and the verses.embedding BLOB) later; lexical overlap is the honest limit
  now (catches shared content words, weak on pure synonyms).

Pipeline (main.rs): semantic index built once at startup from the corpus;
context-memory state managed. emit_detections now fuses three sources —
direct (updates context), context-resolved bare verses, and the top semantic
paraphrase (cosine ≥ 0.30) — dedups by reference keeping the strongest, then
gates each through the router. Detection cards show method (direct/semantic).

44 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## ab6d454 — 2026-07-02
devgeereact

Phase 10: kiosk WS target + multilingual/code-switching STT; NDI seam

Network_client render target (channels.rs): a WebSocket hub (KioskHub) +
tokio-tungstenite server on port 8031, bound 0.0.0.0 for LAN kiosks. Every
content/clear broadcast now fans out to native windows (Tauri event) AND kiosk
clients (WS JSON). Output.svelte runs from ONE renderer in two modes — desktop
(Tauri, DB template, live edits) and kiosk (built-in template by id, state over
WS). A $50 Pi in Chromium kiosk mode is now a real output channel.

Multilingual + code-switching STT (stt.rs): language is a live setting
(Some(code) forces it, None auto-detects per window — code-switching, the
normal case per CLAUDE.md); whisper reports the detected language back. Default
model now prefers multilingual ggml-base.bin over ggml-base.en.bin. Commands:
set_stt_language; stt_status carries language. Settings gets a recognition-
language selector + live "hearing: <lang>" readout. Honest caveat (README):
base multilingual is weak on Yoruba/Hausa — real quality needs Masakhane/
Common Voice fine-tunes, which drop in by filename.

NDI render target: honest seam. open_ndi_output returns a clear "needs the NDI
SDK" error with the integration path documented — not faked. NDI is blocked on
the proprietary native SDK (no pure-Rust crate), so it stays parked.

deps: futures-util (WS sink/stream). 44 Rust tests pass, clippy + fmt clean,
build clean. README updated (models, render targets, ports).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 4e460d8 — 2026-07-03
devgeereact

Fix console: make the dead controls actually operate

Three console controls were inert — now live:
- AI detection On/Off: real toggle. New backend Detecting(AtomicBool) flag +
  set/get_detection_enabled commands; emit_detections early-returns when
  disarmed (transcription continues, manual override still fires — it bypasses
  the gate, a first-class control). Button reflects real state.
- Channel previews: were hardcoded John 3:16 demo. Now mirror the live output
  broadcast — a `live` store fed by output://content / output://clear (always-
  on listener), each preview renders the actually-fired verse in its own style,
  "— cleared —" when blank.
- Keyboard shortcuts: only Esc worked. Now "/" focuses manual override, Space
  confirms the top suggestion, Z undoes the last fired detection, Esc clears.
  Handlers yield to text fields (Esc excepted).

44 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 6b54bd7 — 2026-07-03
devgeereact

Service-session persistence — record services to the Library

Transcripts, fired detections, and operator cues are now recorded to the
current service (local-first, SQLite) and surfaced in the Library tab.

db.rs: create_service / insert_transcript / insert_detection / insert_cue,
and Library reads — list_services (with derived duration = last transcript
timestamp, verses = fired-detection count, overrides = manual-override cue
count), service_transcripts, service_detections (verse ref resolved via join).
1 new test covering the full round-trip + counts.

main.rs: Session state (id + start Instant + last-transcript id). start/end/
current_service + list_services + service_detail commands. Persistence wired
into the pipeline — finals → transcripts; auto-fires → detections; manual
override → detection + manual_override cue; clear-screens → cue. A service
auto-starts when capture begins (reused across pause/resume) and can be ended
from the Library. Relative timestamps use a monotonic Instant so they survive
capture restarts.

Concurrency: unified a global lock order (db before session) across all
persist paths + start_service to avoid a lock-ordering deadlock between the
STT worker thread and command threads.

Library.svelte: real service list + a detail view (transcript lines +
detected verses per service). 45 Rust tests pass, clippy + fmt clean, build
clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## e4de254 — 2026-07-03
devgeereact

Full KJV corpus import + fix capture stopping on tab switch

Full-Bible import (replaces the 15-verse dev seed):
- Bundle the public-domain KJV (66 books, 31,100 verses) as src-tauri/data/
  kjv.json, compiled in via include_str! (offline-first — no runtime file dep).
- db.rs: import_full_kjv bulk-inserts in one transaction, stripping KJV's {…}
  supplied-word markers; book names come from CANONICAL_BOOKS by index so a
  stored verse and a detected reference always agree on spelling. Forward-fill
  migration on open() re-imports for DBs still on the old 15-verse seed
  (FK-safe: nulls detection verse links first).
- detection.rs: CANONICAL_BOOKS (all 66) + a lazily-built alias map replacing
  the 4-book const — covers every book, numbered-book spoken/written forms
  ("first corinthians", "1 john", "1john"), multi-word names (Song of Solomon),
  and variants (Revelations→Revelation). Direct match now works Bible-wide;
  the semantic index builds over the full corpus at startup.

Bug fix: capture stopped whenever you left the Settings tab. Settings had
onDestroy(stopCapture) — but capture is app-level state, so switching to the
Console killed the mic mid-service. Removed it; capture stops only on explicit
Stop.

47 Rust tests pass (new: numbered/multi-word books, full-corpus seed), clippy
+ fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## f4fe46b — 2026-07-03
devgeereact

Handle single-chapter books spoken as a bare verse

Obadiah, Philemon, 2 John, 3 John, Jude have one chapter, so preachers cite
them by bare verse ("Jude 4", "Philemon verse 6"). detection.rs now resolves
these to chapter 1: for a single-chapter book, a leading "verse" keyword or a
lone number is read as the verse (chapter 1), while an explicit second number
("Jude 1 4") and the colon form ("Jude 1:4") are still honored. Works for the
numbered single-chapter books too ("second john four" → 2 John 1:4).

4 new tests (bare verse, spoken/written, numbered book, no-number ignored).
49 Rust tests pass, clippy + fmt clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## ff02f08 — 2026-07-03
devgeereact

Fix STT transcription (overlap garbling) + service Markdown export

Transcription fix (the "it doesn't transcribe / doesn't post the verse" bug):
the STT worker was fed the detection chunker's 50%-overlapping chunks and
concatenated them verbatim, so whisper heard every 200ms twice — garbled or
blank output, so no transcript and nothing to detect. Now the worker appends
only the NON-overlapping tail of each chunk (computed from timestamps, robust
to any overlap ratio), reconstructing a clean continuous stream. Also lowered
the VAD threshold 0.012 → 0.008 so quieter mics still register as voice, and
log finalized transcripts to stdout (`stt[lang]: …`) for diagnosis.

Service export: export_service writes a service as Markdown (summary +
detected verses + timestamped transcript) to ~/Downloads (fallback app-data/
exports) via std::fs — no fs plugin needed, nothing leaves the device.
Library detail gains an "Export .md" button showing the saved path.

49 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## b302f09 — 2026-07-03
devgeereact

Smarter detection: voice nav, ambiguous suggestions, phonetic books

- Voice navigation: a short spoken "next"/"back"/"previous" now fires the
  next/previous verse relative to the current on-screen verse (ContextMemory
  tracks the full current VerseRef; next_verse/prev_verse step it). Runs only
  on short final utterances so it won't trigger mid-sermon. Bypasses the gate
  (operator intent), persists to the service, and updates context.
- Ambiguous references → operator-pickable suggestions: "revelation twenty two"
  with no verse now surfaces Revelation 22:1 AND Revelation 2:2 (two-digit
  split), only when no full reference matched. Suggest tier.
- Phonetic/accent book aliases: the ASR-dropped silent P in Psalms ("sam",
  "salm", "salms") maps to Psalms; plus mathew→Matthew, proverb→Proverbs.
- Current verse now tracked on every fire (auto/manual/confirm/nav), so bare
  "verse 4" and next/back work after any of them.

7 new detection tests (nav commands, next/prev, ambiguous split, phonetic).
54 Rust tests pass, clippy + fmt clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 31e8547 — 2026-07-03
devgeereact

Console focus + assignable channels + OBS output

Console (show suggestions, not recents):
- The AI-detection panel is now a SUGGESTIONS queue (only status 'suggested',
  awaiting a decision) with Confirm/Dismiss — fired verses resolve out of it.
  Added a dedicated "Now live" card showing what's on the screens (from the
  output broadcast) with a Clear button. Demo cards removed.
- Transcript auto-scrolls to the latest word as it streams.
- Fixed the Streaming channel preview to render the actual lower-third (ref +
  verse band) from live content.
- Space now confirms the top suggestion; keyboard hints updated.

Channels (assignable + OBS-ready):
- output_channels are DB-backed and seeded (Main/Stage/Streaming/Lobby); each
  is freely ASSIGNABLE to any template via a dropdown (persisted). Forward-fill
  migration seeds channels for existing DBs.
- Every channel works three ways: "Open" (borderless fullscreen / HDMI),
  "Copy URL" for an OBS/vMix Browser Source (http://localhost:5032/output.html
  ?template_id=…, live over the kiosk WS on :8031 — no NDI needed), and the
  same URL for a Raspberry Pi kiosk. In-tab guidance explains each.
- commands: list_output_channels, set_channel_template.

54 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 2dfab3b — 2026-07-03
devgeereact

Fix: console controls dead — initAudio never ran on the Console tab

$capture.available (which enables nearly every button) was only set by
Settings' onMount(initAudio). Landing on the Console — the default tab —
left available=false, so all controls looked unresponsive. Now App.svelte
runs initAudio() on mount so the backend attaches app-wide regardless of tab.

Also hardened initAudio: as long as the Tauri bridge resolves, available is
true — a single failing command or the event-listener setup no longer flips
the whole app to disabled (previously a Promise.all reject or a listen()
throw could disable everything).

Smoke test green: console + output.html serve 200, kiosk WebSocket (:8031)
accepts connections (OBS browser-source path), 54 Rust tests pass.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## a8b4dc4 — 2026-07-03
devgeereact

Test: kiosk WS forwards a fired verse to a connected client (OBS path)

Proves the exact OBS/vMix browser-source chain end to end: connect a WS
client to the kiosk hub, publish content, assert the client receives it.
55 Rust tests pass.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## f2a4f6c — 2026-07-03
devgeereact

Kiosk/OBS output: auto-reconnect the WebSocket

An OBS/vMix browser source (or Pi kiosk) on the output page dropped its
connection when the app restarted and never came back without a manual
refresh. The kiosk WS now reconnects automatically (1.5s backoff) on close/
error, and stops cleanly on unmount — so output surfaces stay live across app
restarts.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 7030f3c — 2026-07-03
devgeereact

Fix app freeze on Start listening; add console Listen (auto-drive) control

Root cause of the freeze: start_capture is a synchronous Tauri command, so it
runs on the UI thread — and AudioEngine::start BLOCKED it waiting for the audio
stream to confirm ready (ready_rx.recv()). If device/stream init stalled (e.g.
a macOS mic-permission prompt), the whole app froze and every button died.

Fix: AudioEngine::start is now NON-BLOCKING — it spawns the capture thread and
returns immediately; the stream is built on that thread. Device errors are
reported asynchronously via an `audio://error` event (surfaced in the console,
never fatal), instead of blocking the caller. start_capture now returns
instantly.

Console control (operator drives detection from the main screen, per request):
- New "Start listening / Listening — Stop" button — mic on/off from the
  console. With AI detection armed that's auto-drive; disarmed it's
  transcribe-and-suggest; mic off is pure manual override.
- AI-detection toggle kept alongside it; audio errors show as a banner.

55 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## cee9b2f — 2026-07-03
devgeereact

Fix freeze on Start listening: never hold locks across emit; cap STT threads

The real freeze cause: emit_detections and handle_nav ran on the STT worker
thread and called handle.emit / broadcast_content WHILE holding the db + router
+ context locks. Emitting from a background thread while holding a lock that a
main-thread command also needs deadlocks the macOS main run loop → the whole
app freezes and every button dies. Both now compute everything under the locks,
RELEASE them, then emit/broadcast.

Also:
- Cap whisper to half the cores (max 4) so continuous transcription doesn't peg
  every core and starve the UI thread (which reads as a freeze).
- start_capture / stop_capture are now async, so they run off the main thread
  regardless.

55 Rust tests pass, clippy + fmt clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 365354d — 2026-07-03
devgeereact

Silence whisper's per-token stderr flood (the "freeze"); fix ASR possessives

Debugging via a backend-forwarded frontend heartbeat proved the app was NOT
actually deadlocked — the heartbeat kept ticking through capture + transcription.
The real culprit: whisper.cpp logs THOUSANDS of `whisper_full_with_state`
per-token lines to stderr on every transcription (~1/sec), and that I/O flood
pegs the process and makes the UI feel frozen/janky. Now silenced once via
whisper_rs::install_logging_hooks (routed to the `log` crate with no subscriber
→ dropped).

Detection: whisper mishears "Psalms 23" as "Sam's 23"; the apostrophe was
splitting it into `sam` + `s` and breaking the parse. normalize() now DROPS
apostrophes (straight and curly) so possessives stay one token — "Sam's" →
"sams" → Psalms. 2 new tests.

Kept a lightweight frontend error reporter (window.onerror/onunhandledrejection
→ backend log) for future diagnosis; removed the noisy heartbeat.

56 Rust tests pass, clippy + fmt clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 414b198 — 2026-07-03
devgeereact

Debug: boot-trace logging to diagnose blank/broken console

Register the frontend error reporter BEFORE imports and log each boot
milestone (main.js start → modules imported → App mounted OK) to the backend,
so a blank console is diagnosable without devtools. Confirmed the app mounts
and renders cleanly — the earlier "blank console" was a stale leftover window
from rapid restarts, not a render failure.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 5629afd — 2026-07-03
devgeereact

Fix silent crash after first transcript: bigger STT worker stack

Debugged by isolating the pipeline headlessly: a synthetic-audio STT smoke
test transcribes and SURVIVES with no Tauri, and a background firing-loop emits
fine — but the real app died silently (no panic) right after the first
transcript. Cause: the STT worker runs whisper_full() (very stack-hungry) and
THEN serializes a Tauri `emit` on the same thread, overflowing the default 2MB
thread stack → SIGSEGV, which reads as a frozen window (dead backend).

Fix: spawn the STT worker with a 16MB stack (Thread::Builder). Also fixed
SttEngine::drop, which joined the worker while still holding a sender clone and
hung forever (only bit tests; the app never drops the engine) — now detaches.

56 Rust tests pass; the headless STT smoke test completes cleanly.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 7b5cc74 — 2026-07-03
devgeereact

Fix the real freeze: tick() in a reactive block (infinite loop); add fast-search abbreviations

ROOT CAUSE of the freeze (found by instrumenting a frontend heartbeat forwarded
to the backend log): the Console auto-scroll used `tick().then(...)` INSIDE a
reactive `$:` block. That re-enters Svelte's scheduler and infinite-loops the
webview JS thread the moment transcript/detection events start flowing — a hard
freeze with no exception. The backend was always fine (it kept transcribing).
Repro was deterministic: heartbeat froze at beat 5 when events began; with the
fix it climbs straight through a 40-event burst.

Fix: use `afterUpdate` (the correct hook for DOM side-effects) to scroll — no
re-entrancy, no loop.

Also this session:
- Fast-search abbreviations in the manual override: "ps 23 1" → Psalms 23:1,
  "rom 8 1" → Romans 8:1, "1 jn 3 1" → 1 John 3:1, "mt", "rev", "2 co", etc.
  (7 new tests, 28 in the detection module).
- Throttle the audio://chunk level event ~5/sec → ~1.6/sec (webview load).
- Removed all debug instrumentation (heartbeat, boot logs, fire loop, audio
  eprintlns, log_frontend command).

Prior fixes in this thread that also stuck: 16MB STT worker stack (the earlier
SIGSEGV), whisper stderr flood silenced, apostrophe-safe normalize
("Sam's"→Psalms), non-blocking audio start.

57 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 361004d — 2026-07-03
devgeereact

Templates: WYSIWYG + colors + text-wrap + alpha lower-third; console next/prev; accent aliases

Templates / output:
- New shared TemplateRender.svelte drives BOTH the fullscreen output and the
  editor preview → true WYSIWYG (what you save is what shows). Sizes are cqw
  (container-query %) so a template scales identically at any output size.
- Text wrap + auto-shrink: long verses wrap (overflow-wrap:anywhere) and shrink
  by length so scripture never runs off the screen.
- Colors: editor now offers Black / White / Charcoal / Warm / Deep blue / Lobby
  / Transparent backgrounds (each pairs a readable text color), a text-color
  picker, accent picker, and verse/ref size sliders.
- Alpha lower-third: the output page is transparent, so a Transparent-background
  template keys out for OBS/ATEM — camera shows behind the lower-third band.
  Editor preview shows a checkerboard so alpha reads clearly.
- Migration: existing DBs on the old "vw" sizes are reset IN PLACE (ids kept, so
  output_channels FKs stay valid) to the new cqw defaults.

Console: manual ◀ Prev / Next ▶ buttons on the Now-live card (backend `nav`
command) — same path as the spoken "next"/"back".

Detection: more accent/spelling book aliases (collosians→Colossians,
phillipians→Philippians, efesians→Ephesians, deutronomy→Deuteronomy, …). Full
accent adaptation still needs a fine-tuned model (parked).

57 Rust tests pass, clippy + fmt clean, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 59dff0d — 2026-07-03
devgeereact

Docs: bring CLAUDE.md up to date with build state + hard-won knowledge

The modules are no longer stubs — full pipeline works end to end. Added:
- Current build status + what's parked (NDI, neural embedder, accent fine-tune).
- Run/build: native-window reality, cmake at ~/.local/bin (no brew), model +
  bundled-KJV locations, dev DB path + migrations, ports (5032 app / 8031 WS),
  OBS browser-source URL.
- "Architecture rules learned the hard way" — the 8 crash/freeze traps and their
  fixes (tick()-in-reactive, lock-across-emit, STT stack size, whisper log hooks,
  non-blocking audio start, db-before-session lock order, app-level initAudio,
  non-overlapping STT feed).
- Frontend shape (single store, TemplateRender WYSIWYG + cqw, transparent output
  for OBS keying, event names) and detection notes (aliases, spoken numbers, nav).
- Fixed the outdated "(stub)" labels in the repo map.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 64c5adf — 2026-07-03
devgeereact

Detection/audio intelligence (Phases C/B/A/D) + HDMI channel screen assignment

Audio front-end (C): new dsp.rs — offline RNNoise denoise + smart auto-gain +
quality metering before STT; capture prefers 48 kHz so RNNoise runs
frame-aligned; additive audio://quality event.

Accent & speaker calibration (B): per-preacher voice_profiles (language hint,
decoder-bias prompt, sensitivity dial, self-calibrating thresholds) persisted in
SQLite and applied to STT + router; whisper initial_prompt biasing toward
scripture vocabulary; feedback now persists per profile.

Detection intelligence (A): verse ranges + whole-chapter walk ("Psalm 23" fires
verse 1 and stages the chapter; "next" walks to the end then stops); topical
concordance / cross-references (related_scripture); series repeat tracker
(verse_repeat_count). A7 multi-translation deferred (needs corpus data).

AI control (D): master AI on/off semantics documented; spoken clear/blackout
command; emergency announcement push. D6 logo overlay deferred (needs renderer).

Channels / HDMI: enumerate displays (list_monitors), pin a native output window
to an assigned physical display, per-channel display assignment persisted,
add/delete channels. Channels UI gains a display picker + add/delete.

87 tests pass; console/output renderer unchanged.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## f03276f — 2026-07-07
devgeereact

Library + Planner: unified cue model, countdown, FTS5, stage notes, output polish

Content types (all create in Library, add to a plan, reorder, fire through the one shared pipeline):
- Lyrics: song editor with slide-flow, drag-reorder, colour-coded sections; named arrangements (play-orders w/ repeats), picker on add-to-plan in both Lyrics and Planner; edits propagate to plan cues
- Media: import images/video, fire full-screen backgrounds; plannable + fireable
- Announcements: new content type (CRUD + edit propagation to plans)
- Countdown: pre-service timer, ticks locally in output (offline, no per-second traffic), on every surface (output, kiosk/OBS, planner monitors, stage remote); one-at-a-time guard + two-step arm

Service Planner: mission-control run editor (cues | slide flow | 4 live monitors), transport + keyboard + drag-reorder, per-cue stage notes (confidence-monitor only), unified add search (scripture/songs/media/announcements/countdown), plan duplication

Detection/search: FTS5 full-text scripture search (bm25, porter stemmer) behind the existing reference/phrase/semantic ranker

Output: verse auto-fit (measure + shrink, never clip/overflow), crossfade transitions, per-content-type templates, blackout, transparent keying for OBS/ATEM

Data: full KJV import now strips translator marginal glosses (keeps supplied-word italics); self-healing migrations for FTS index, channels, templates, gloss re-clean

UX: Console unified to the global --v-* design tokens; channel add/delete; two-step delete guards (Tauri webview has no native confirm); Settings de-mocked

123 Rust tests pass; clippy + rustfmt clean; vite builds.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 02fea0a — 2026-07-07
devgeereact

docs: architecture + operator guide; refresh README and decision log

- docs/ARCHITECTURE.md (new): full how-it-works — process model, live pipeline,
  unified cue model, output/rendering (one renderer, auto-fit, countdown),
  data layer + scripture search, command/event reference, invariants, limits
- docs/USER_GUIDE.md (new): operator guide for every screen + typical Sunday flow
- README.md: status reflects the presentation suite; points to the new docs;
  accurate repo tree; render targets include the stage remote + :8032
- docs/DECISIONS.md: build-out decision log (cue model, snapshot-vs-reference,
  arrangements, local-tick countdown, auto-fit, FTS5, gloss strip, no native
  confirm, per-content templates, Console token unification)

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 01dbdf5 — 2026-07-12
Gideon Akinlotan

Harden for first live service: fix 6 screen-facing bugs, add CI + tests (#1)

* Harden for first live service: fix 6 screen-facing bugs, add CI + tests

Relay was functionally complete but had never run in front of a
congregation. This makes what exists unbreakable for one service.

Three of these were found only by reading verses aloud into a real
microphone. None were findable by a test anyone would have thought to
write — they exist only where real speech meets real STT.

Bugs that reached (or blanked) the screen:

* Semantic paraphrases could auto-fire. A raw TF-IDF cosine was passed
  straight in as a confidence, but a cosine is not a probability — a
  sermon window sharing a few rare words with some verse could put the
  wrong scripture in front of the congregation with no human in the
  loop. The gate is now the detection METHOD, not a number: only Direct
  may auto-fire, at any score, at any sensitivity.

* The same verse re-fired once per second. STT re-transcribes a rolling
  8s window, and a >=0.95 match was exempted from its own debounce, so
  one spoken reference re-fired 9 times — re-crossfading on the
  projector. The exemption is gone (re-firing a verse already on screen
  achieves nothing), and the cooldown is now DERIVED from the STT window
  rather than picked independently: it was 5s, shorter than the 8s
  window generating the repeats.

* A verse that does not exist was broadcast anyway. Garbled speech
  yields "Psalms 23:99"; it parsed, auto-fired, and rendered empty —
  blanking the projector mid-service. Now gated by Fire::may_broadcast()
  and demoted to a suggestion, so it is surfaced, never silent.

* On packaged Windows, STT never loaded. The model path was hardcoded to
  macOS $HOME/Library/Application Support. Windows has no HOME, so it
  resolved to nothing and Relay ran with speech recognition silently
  dead — on a day-one platform. One per-OS db::app_data_dir() now.

* Saving a voice profile wiped all calibration. Two threshold baselines
  coexisted and disagreed (0.50/0.35 vs 0.90/0.60), and any profile save
  — even a rename — snapped between them. Now one baseline by
  construction: default() IS from_sensitivity(50).

* Nav dropped the scripture template. A verse reached by saying "next"
  rendered differently from the same verse reached by saying its
  reference. Cause was structural: the fire payload was hand-built in
  five places. Now built once, in pipeline.rs.

Could not ship at all:

* tauri build had NEVER succeeded. whisper.cpp needs std::filesystem
  (macOS 10.15+) while Tauri's release profile passes 10.13. cargo test
  and tauri dev were unaffected, so nobody noticed — which is also why
  the Windows model-path bug survived: you cannot find a packaging bug
  in a project that cannot be packaged.

Operator safety:

* Panic keys are global. Esc/blackout were bound per-view, so they did
  nothing on the Templates, Library or Settings tabs. Space also meant
  two different things (advance vs push-the-AI's-guess-live).
* Crash boundary. An uncaught UI error white-screened the console
  mid-service. It now shows a recovery panel stating the one fact that
  matters — the output screens are still live — and resumes position.
* Audio-quality warnings ("your mic is muted") were computed, emitted,
  and listened to by nobody. Found by the new IPC contract test.
* Manual fires were recorded as AI decisions, so the self-calibrating
  router was training on a log that could not tell a human from itself.

Also: opt-in content-scrubbed crash reporting (off by default; sermon
transcripts and verse text can never leave the device), CI on macOS AND
Windows incl. the release build, calibration decay (it was a one-way
ratchet), and a recorded decision for the LAN bind posture.

Structure: db.rs (2,765 lines) split by aggregate; migrations moved to a
user_version ladder; the $capture firehose (50/sec) no longer re-renders
the app shell. Measured the "obviously slow" 31k-verse semantic scan at
2.6ms/query and deliberately did NOT optimise it.

161 Rust tests, 28 frontend tests, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(ci): pin vitest to the vite-5 line so npm ci works

CI failed on `npm ci` before it reached a single test, and it was right
to. vitest@4 pulls in vite@8, which needs esbuild ^0.27||^0.28, but the
app builds on vite@5 (esbuild 0.21). npm deduped the two into an invalid
tree locally and only warned; `npm ci` refuses it outright.

So this was never a CI quirk — the dependency tree I committed was
genuinely broken, and would have failed on any fresh clone. `npm install`
happened to tolerate it on a machine that already had node_modules.

Pin vitest to ^2 (the line that supports vite 5) rather than dragging the
app's build onto vite 8 for the sake of the test runner. Also drop
@testing-library/svelte: the tests are plain JS and never rendered a
component with it.

Verified with a clean `rm -rf node_modules && npm ci` — the exact thing
CI runs. 28 tests pass, build clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(ci): use --no-bundle, not --bundles none

`--bundles none` isn't a valid value; Tauri v2 spells it `--no-bundle`.
The macOS job otherwise passed everything — fmt, clippy -D warnings, and
all 161 tests — and only tripped on this flag at the packaging step.

--no-bundle compiles the release binary but skips installer packaging.
The compile is the part that was broken (whisper.cpp vs the 10.13
deployment target), and it needs no signing identity to prove.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix: close a content leak in crash reports, and 6 more review findings

CodeRabbit caught a real hole in the one guarantee this PR makes loudest:
that sermon text can never leave the device.

redact() tried to blank QUOTED SPANS and keep the rest of the message.
That is a blocklist — and telemetry.rs's own doc comment says, correctly,
that a blocklist fails open and the cost of failing open here is
publishing somebody's sermon. I wrote the right principle and then
violated it one function down.

It failed open immediately, because an apostrophe is a quote character
and scripture is full of them:

    in:  no verse for 'God's word to the church'
    out: no verse for "<redacted>"s word to the church"
                                   ^^^^^^^^^^^^^^^^^^^ sent in the clear

The ' in God's closed the span early and the rest went out verbatim. My
tests passed because I tested the case I was thinking of; the leak was in
the case I wasn't.

There is no safe way to sift content out of a free-text field that is
ALLOWED to contain content. So it is no longer sifted — it is dropped.
Message and exception value are replaced wholesale; the exception TYPE and
the stack trace stay, because those are code. Stack-frame LOCALS are now
cleared too: a local at the moment of a crash is very often the exact
verse that caused it, and nothing was clearing them.

A crash is still fully actionable from type + stack + module + OS +
version — which is exactly what this module always claimed it sent, and
now actually does.

Also fixed, all from the same review:

* forget_last_fire() cleared the debounce key but not last_fire_conf, so
  a dismiss AFTER a clear/blackout tuned the gate using the score of an
  auto-fire that was no longer on screen.

* db/profiles.rs hardcoded the 0.50/0.35 gate baseline in FIVE more
  places (serde defaults, table DDL, seed row, re-baseline UPDATE,
  create_voice_profile). That is the exact drift that caused the original
  calibration bug — fixing the router while leaving five copies in the DB
  layer would have re-armed the same trap. All now derive from
  router::Thresholds::default(), with a test that fails if a sixth
  appears.

* The monitor badge lost its colour: I renamed the accent gold -> amber
  when unifying the Console and Planner lists, and the CSS still only
  defined .b-gold. A regression I introduced in the P2 dedup.

* An unmapped audio-quality warning kind would throw on
  QUALITY[kind].title and take the whole console down mid-service, over a
  mic warning.

* capture.js set outputListenersUp BEFORE registration succeeded, so one
  failed listen() latched the flag forever — listeners never registered,
  never retried, console silently stopped mirroring the screens.

* parseTemplateOverride accepted any JSON: "42" and "null" both parse
  fine and are not templates.

* CI: persist-credentials: false on both checkouts, and the release step
  is renamed to say what it actually is — a COMPILE, not packaging.
  --no-bundle never touches the msi/dmg installers, and the old name
  implied coverage we don't have.

164 Rust tests, 28 frontend tests, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---------

Co-authored-by: devgeereact <292055051+devgeereact@users.noreply.github.com>
Co-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
---
## 0851ab0 — 2026-07-12
Gideon Akinlotan

Live console, rehearsal mode, distribution — and Relay was deaf to a quiet preacher (#2)

* docs: product audit — the engine is ahead of the product

Full audit, scoped to the two decisions made before writing it: strategy
unchanged (free, MIT, offline-first), and optimise for the first 10
churches rather than for enterprise scale.

Headline: Relay is a good engine wrapped around a product that cannot be
delivered to the people it was built for.

To turn the AI on, a volunteer must run `curl` to fetch a ~148 MB model
into a folder that does not exist in the packaged app — the instruction
only works if you cloned the repo with git. The operator guide never
mentions the model at all, and Settings tells a church volunteer to "see
README dev setup". The download would be blocked by Gatekeeper and
SmartScreen anyway (nothing is signed), and there is no updater, so the
six screen-facing bugs fixed this week cannot reach anyone who already
installed it.

Those three are one epic and they outrank every feature on the roadmap.

Also recorded, with file:line evidence:

* Console's "Open output" passes 2 args to a 3-arg signature, so
  monitor_index is None — the button can ONLY open on the primary
  display, and a projector is by definition the second one.
* The ON AIR badge keys off $capturing (mic) rather than $live (screen),
  so Relay can shout ON AIR at an operator whose screens are blank.
* registerContext() REPLACES the handler table, so A/D// are dead keys on
  the Planner tab while the cheatsheet still lists them.
* --v-faint (#6c6b71, ~3.4:1) fails WCAG AA and is the colour of every
  empty state — the text a new operator most needs to read.
* aria-live: zero. A screen reader is told nothing when scripture goes
  live.
* Relay detects Yoruba/Swahili/Hausa but its own UI is hardcoded English.

The SaaS phases of the brief (billing, RBAC/SSO, multi-tenancy, audit
logs, enterprise/gov/healthcare) are marked NOT APPLICABLE with
reasoning. They are not gaps — they are the shape of the product, and
adopting them would destroy the offline-first moat.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat: install the speech model from inside the app

C1 from the audit: Relay's entire reason to exist was unreachable by the
people it was built for.

To turn the AI on, a church volunteer had to open a terminal and run:

    mkdir -p models
    curl -L -o models/ggml-base.bin https://huggingface.co/.../ggml-base.bin

They will not do that. And in a PACKAGED app there is no repo `models/`
folder at all — that instruction only ever worked for someone who had
cloned the repository with git. The operator guide never mentioned the
model, and Settings told them, in the product, to go and read the
developer README.

So for a real user the AI silently did not exist.

Now: one button. models.rs downloads the model into the per-OS app-data
dir with progress the operator can watch, and speech recognition comes up
IN PLACE — no restart, because ending a 148 MB download with "now quit and
reopen the app" is a miserable last step for a first-time user.

Built for the target market, not for a fast office connection:

* RESUMABLE. A 148 MB download over a church's line WILL be interrupted.
  Verified against the real server: it answers 206 Partial Content, so a
  Range request genuinely continues rather than starting over.
* VERIFIED. Checked against a known SHA-256 before it is accepted. This
  is not paranoia: a truncated model does not fail loudly — whisper loads
  it and transcribes nonsense, which is far worse than not working. The
  checksums are the real values of the two models this project has
  actually been run against, computed from the files on disk.
* ATOMIC. Lands in <name>.part and is renamed into place only after the
  checksum passes. Relay can never see a half-written model.
* Cancellable, throttled to ~1 event/MB, and never on the UI thread.

Also fixed, from the same audit — the projector bug:

Console's "Open output" called openOutput(id, name) — two arguments to a
THREE-argument signature — so monitorIndex arrived as undefined and the
window ALWAYS opened on the primary display. A projector is by definition
the second display. The first button an operator ever presses could not do
the one thing they needed, and the only workaround (Channels tab, set the
display, press a *different* Open button) is undiscoverable.

It now opens the real Main-screen channel, honouring the template and
display the operator configured — and when no display is set yet it picks
the first non-primary monitor, because a second screen plugged into a
church laptop is a projector essentially every time.

169 Rust tests, 28 frontend tests, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat: signed releases + auto-update (C2 + C3)

The last two of the three things standing between Relay and a church.

C3 — there was no way to ship a fix. We fixed six screen-facing bugs in a
week and could not deliver one of them to anyone who had already installed
Relay. For software that fails LIVE, in front of a congregation, an update
path is not a nice-to-have: it is how a fix becomes a fix.

The rule this is built around, and it outranks everything else in the
feature:

    RELAY NEVER UPDATES DURING A SERVICE.

Not a dialog, not a toast, not a background download competing for a
laptop's last 300 MB of RAM while whisper is running. An updater that
interrupts a sermon is WORSE than no updater — it takes a tool that merely
lacks a fix and turns it into a tool that actively causes a failure. So
updater.js refuses to even CHECK while the mic is live, refuses to INSTALL
if capture starts, and the banner says so out loud: "Installing restarts
the app, so do it before the service — not during."

Updates are signed. Relay will only ever install an update signed with our
private key — without that, anyone who can MITM the update endpoint can
push code onto a church's machine.

C2 — the download is blocked by the operating system. An unsigned build
gets "Relay is damaged and can't be opened" from Gatekeeper and "Windows
protected your PC" from SmartScreen. A volunteer does not push past those
screens; they close the tab and go back to PowerPoint. release.yml builds,
signs and notarizes macOS (universal, so one download covers Apple Silicon
and Intel) and Windows, and opens a DRAFT release — you look at it before
a church does.

The updater config is a separate overlay (tauri.updater.conf.json) merged
only at release time. It needs a public key that does not exist in a fresh
clone, and putting it in the main config would break every cargo build and
every CI run on a placeholder. CI stays green; the release job fails loudly
if the key is still unset, which is the correct place to fail.

docs/RELEASING.md has the runbook. Two things only the maintainer can do —
generate the updater keypair, and buy the certificates — and the private
key must never be pasted anywhere, including to an AI. A key that has been
seen by anything but you and GitHub's secret store is a compromised key.

169 Rust tests, 28 frontend tests, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat(i18n): detect scripture in Yorùbá, Kiswahili and Hausa

The moat was not blocked on the acoustic model. It was blocked on a
lookup table.

Relay's stated differentiator is African-language speech. But the DETECTOR
spoke only English: zero non-English book names, zero non-English numerals,
and a decoder-bias prompt that fed whisper ENGLISH book names no matter what
was being preached.

So picture a perfect Yorùbá fine-tune, before this commit:

    Preacher:   "Ẹ ṣí Jòhánù orí kẹta, ẹsẹ kẹrìndínlógún."
    Whisper:    transcribes it flawlessly
    Detector:   looks for "john" ... finds nothing
    Screen:     stays blank. Every time.

Relay could have understood Yorùbá perfectly and still detected zero verses.
Fine-tuning the model would not have fixed that by a single verse. Worse, the
bias prompt was actively pushing whisper AWAY from the Yorùbá words — priming
it to hear "John" where the preacher said "Jòhánù".

What this adds:

* Book names for all three tier-1 languages. Swahili is complete (66/66,
  Biblia Takatifu); Yorùbá (29) and Hausa (11) are partial and marked as such.

* Diacritic-insensitive matching. Whisper emits "Jòhánù", "Johánù" or
  "Johanu" for the same audio depending on the recording — if those are three
  tokens, the table matches none of them. normalize() now NFD-folds tone marks
  and dots-below, and maps the Hausa hooked consonants (ɓ ɗ ƙ ƴ) by hand,
  since they are distinct letters that NFD will not decompose.

* A language-aware bias prompt. The decoder is now primed in the language
  actually being preached — plus English, always, because code-switching is
  the normal case for this market and not an edge case (CLAUDE.md).

The names are DATA, not Rust (src-tauri/data/book_aliases.json).

That is deliberate. The maintainer does not speak all three of these
languages fluently, and a wrong alias does not fail safely — it puts the
WRONG SCRIPTURE on a wall in front of a congregation. Keeping the names in
JSON means a native speaker can fix them in a one-line pull request without
touching Rust, without a build, and without knowing what a HashMap is. That
is the only path by which this table ever becomes trustworthy.

Where a name could not be verified against a published Bible-society
translation, it was LEFT OUT rather than guessed. Omission is safe — it just
means no detection yet. A wrong entry is not.

Ships no fine-tuned model. Whisper's ~117k training hours cover 96 languages,
but Yorùbá and Hausa together contribute under 600 of them — that is the real
reason base multilingual is weak, and it is not fixable in code. Relay's
African-language accuracy is currently UNMEASURED; shipping an unbenchmarked
fine-tune and calling it a feature would be a marketing claim, not an
engineering one. docs/LANGUAGES.md says exactly that, and says what would
actually move the needle.

178 Rust tests (8 new, all tier-1), 28 frontend, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat(eval): benchmark detection — and fix the 2 bugs it immediately found

docs/SPEC.md sets a target of a <5% wrong-verse rate. Nothing anywhere
checked it. The product's headline claim — that it hears scripture in
Yorùbá, Kiswahili and Hausa — was entirely unmeasured. You cannot improve
what you have never baselined, and you cannot defend a moat you cannot put
a number on.

The harness scores through the REAL ROUTER, not just the parser. What
matters is not what detect_direct found — it is what would have been put on
a wall in front of a congregation. So the headline metric is the one the
product lives or dies on: how often Relay shows a verse nobody asked for.

It runs in CI and FAILS THE BUILD on regression.

It found two real bugs on its first run, and both put wrong scripture on
the screen at full confidence.

1. "one hundred AND thirteen" parsed as 100.

   "sam one hundred and thirteen verse one" auto-fired PSALM 100:1. The
   spoken-number FSM broke on the word "and" and returned what it had.

   This is not an edge case for this market. Nigerian, Kenyan and British
   English all say "a hundred AND thirteen" as the DEFAULT form. American
   English drops it — which is presumably why it was never noticed.

2. Garbled speech fired a confident wrong verse.

   This real transcript, from the live rehearsal we ran:

       "Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,"

   scored 0.92 and put Psalms 2:3 on the wall, unasked.

   Root cause: bare digits with no "chapter"/"verse" keyword were trusted
   at 0.92, and make_match's confidence floor was clamp(0.50, ..) — which
   is EXACTLY the auto-fire threshold, so nothing could ever be demoted to
   a suggestion no matter how weak the parse. The confidence scale was
   decorative below that line.

   The bare-digit form exists for TYPED shorthand ("ps 23 1") — and typed
   input goes through manual_fire, which bypasses the gate entirely. So
   demoting it costs the operator nothing. Nobody SAYS "Psalms two three";
   they say "Psalms two verse three". It now reaches the operator, not the
   congregation, and a human decides.

Before → after:

    recall           94%  →  100%
    wrong verses       2  →  0
    wrong-verse rate 8.7% →  0.0%   (SPEC target: <5%)

The corpus (data/eval_corpus.json) is seeded with real garbled transcripts
from the rehearsal as NEGATIVE cases, real ASR mishears, all three tier-1
languages, and paraphrases that must never auto-fire. Adding cases is the
most useful contribution anyone can make — a failing case is not a problem,
it is the point.

Still NOT measured: transcription quality (WER). That needs real sermon
audio, which does not exist. docs/LANGUAGES.md says so, and says that
recording 30 minutes of it is worth more than any code in this commit.

184 Rust tests, 28 frontend, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat(i18n): complete the Yorùbá and Hausa book names (66/66, all three)

All three tier-1 languages now cover the full canon. Every name came from a
published translation, not from memory:

  Kiswahili  66/66  Biblia Takatifu / Neno (Biblica)
  Yorùbá     66/66  Yoruba Contemporary Bible + Bibeli Mímọ́
  Hausa      66/66  Bible Society of Nigeria 1932/2010 + HCB

Yorùbá carries TWO translations at once, and both matter. Biblica's YCB calls
Psalms "Sáàmù"; the older Bibeli Mímọ́ calls it "Psalmu"; many churches say
"Orin Dáfídì". A preacher says whatever their own Bible says, so all three are
listed. Same for Genesis, and for most of the canon.

THE TRAP THAT NEARLY GOT IN

Some book names are also ordinary words:

    Iṣẹ́   (Acts)             also just means "work"
    Orin  (Song of Solomon)  also just means "song" — in a church

Listing those bare would fire scripture off normal speech. "Iṣẹ́ wa ni lati sin
Ọlọrun" — "our work is to serve God" — would have put the book of Acts on the
wall. So only the full forms are in the table (Ìṣe àwọn Àpọ́sítélì, Orin
Solomoni), both of those sentences are now NEGATIVE cases in the benchmark, and
a test fails the build if a bare everyday word is ever added.

New safety tests, because a 198-name table is exactly where a silent wrong
answer hides:

  * all 66 books present in all three languages
  * no alias maps to two different books (one would win arbitrarily and the
    other would silently show the wrong scripture)
  * no alias hijacks an English book name
  * no alias is a bare everyday word
  * the books churches actually read resolve, in every language

Benchmark grew 23 → 38 cases. Yorùbá 12, Hausa 8, Swahili 6.
Still 100% recall, still 0 wrong verses, still 0.0% wrong-verse rate.

WHAT IS STILL NOT DONE, and it is the thing that matters now:

None of these names has been checked by someone who actually SPEAKS the
language. They are sourced, not reviewed. If one is wrong, Relay will
confidently show the wrong scripture — the exact failure this project exists to
prevent. docs/LANGUAGES.md says so plainly and tells a native speaker how to fix
a name in a one-line PR without touching Rust.

189 Rust tests, 28 frontend, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat(i18n): Swahili and Hausa numerals — a sermon can now be fully in-language

A preacher can say the book, the chapter AND the verse without a word of
English:

    "Yohana sura ya tatu, mstari wa kumi na sita"   → John 3:16
    "Zabura sura ta ashirin da uku, aya ta farko"   → Psalms 23:1

Adds in-language numbers, chapter words (sura, babi), verse words (mstari,
aya) and the grammatical linkers that glue them together (sura YA tatu,
aya TA farko).

THE TRAP, and it is not cosmetic:

    THE HUNDRED MULTIPLIER COMES AFTER THE HUNDRED WORD.

        mia moja  = 100   (literally "hundred one")   NOT 101
        mia mbili = 200                               NOT 102
        ɗari biyu = 200                               NOT 102

English puts it first ("two hundred"). So the existing English parser, run
on Swahili, would have read "mia mbili" as 100 + 2 = 102 — and put PSALM 102
on the wall when the preacher said PSALM 200. Silently. With confidence.

That is exactly the class of failure this project exists to prevent, and it
is invisible unless you know the grammar. NumWord::HundredPost handles it,
and a connector disambiguates the two readings: "mia moja" (no connector) is
1×100, while "mia na tatu" (connector) is 100+3.

I verified every numeral against published sources rather than trusting my
memory — which is how I found that Hausa 90 is "tis'in", not what I would
have written from recall.

The words are DATA (data/numerals.json), not Rust. Same reason as the book
names: a wrong numeral does not fail safely, it silently shows a DIFFERENT
VERSE. If "tisa" were mapped to 8 instead of 9, nobody would find out until
a service. A native speaker can fix a number in a one-line PR.

Safety tests, because this is where a silent wrong answer lives:

  * ones are 1-9, tens are multiples of 10 — catches a fat-finger
  * no word is both a number and a connector (it would be counted twice)
  * "mia mbili" is 200 and NOT 102, asserted by name
  * a bare "na"/"da"/"ya"/"ta" never parses as a number — these are among
    the commonest words in both languages, and ordinary speech must never
    manufacture a verse reference

Benchmark 38 → 50 cases. Swahili 12, Hausa 14, Yorùbá 12, English 12.
Still 100% recall. Still 0 wrong verses. Still 0.0% wrong-verse rate.

Yorùbá numerals remain TODO and are genuinely hard: they are subtractive —
16 is ẹrìndínlógún, literally "four less than twenty". That is a parsing
problem, not a lookup table, and it is a great first contribution for a
Yorùbá speaker. Until then Yorùbá relies on code-switching, which is the
normal case rather than an edge case (CLAUDE.md) and already works.

194 Rust tests, 28 frontend, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat: first-run setup, privacy/security policies — and fix a startup panic

Closes Phase 1 of the audit ("make it installable"), except code-signing,
which is blocked on certificates only the maintainer can buy.

FIRST, A BUG I INTRODUCED AND ALMOST SHIPPED.

The app PANICKED ON STARTUP:

    PluginInitialization("updater", "invalid type: null, expected struct Config")

I registered the updater plugin unconditionally but put its config in the
release-only overlay, so `plugins.updater` was null. `tauri dev` died on
boot — and the PACKAGED APP WOULD HAVE TOO, because CI only ever COMPILES
the release build and never launches it. A compile is not a boot.

Every test passed. Clippy passed. The release build compiled. And the app
did not start. Fixed (the base config now carries an inert updater block;
the real key is still injected at release time), and there is now a test
that asserts the block exists, because that is the only thing that would
have caught it.

FIRST-RUN SETUP

The audit's exit criterion is one sentence: a volunteer who has never seen
a terminal installs Relay and gets a verse on a projector in under ten
minutes. There was no onboarding of any kind — zero hits for onboard,
wizard, welcome, tutorial.

Two questions, because only two of them need a human standing in the room:

  1. Which screen does the congregation see?   (a real monitor picker)
  2. Which microphone hears the preacher?      (with a LIVE METER — a
     dropdown proves nothing, a moving bar proves it can hear)

Then it puts John 3:16 on the actual screen, because "setup complete" is
not proof and a verse on a wall is.

Everything Relay can decide for itself, it already has: templates and
channels are seeded before the operator ever sees them. And the wizard can
be skipped and NEVER returns — a wizard that reappears is a wizard that
gets clicked through blindly, and every setting in it also lives
permanently in Settings.

POLICIES — the biggest trust asset, previously undocumented

PRIVACY.md. Relay listens to sermons, so this is the most important
document in the project. Nothing leaves the machine. The audio is never
even saved. Crash reporting is off by default, and when on, free text is
DROPPED rather than sifted — with the apostrophe leak written up honestly,
because a blocklist that failed open is exactly what a reader deserves to
know about.

SECURITY.md. What we consider critical, in order: anything that sends
content off the device; anything that can put content on a screen the
operator did not choose; anything that compromises the update channel. The
LAN bind is listed as a KNOWN, RECORDED tradeoff — not an undiscovered bug.

docs/AI_DISCLOSURE.md. What the AI decides alone, what it will never do (a
paraphrase never reaches a congregation without a human agreeing — enforced
in router.rs, not in policy), and where it is genuinely weak: African-
language transcription is the headline claim and the least-finished part,
and a church deserves to hear that from us.

195 Rust tests, 31 frontend tests, clippy -D warnings clean, app boots.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix: ON AIR now means the SCREEN, the cheatsheet stops lying, a11y

Phase 2's three live-safety bugs. None is polish; each one could mislead an
operator during a service.

1. "ON AIR" was reporting the MICROPHONE, not the screen.

   The two loudest indicators in the product — the pulsing topbar badge and
   the footer — both keyed off $capturing. So Relay would sit there pulsing
   ON AIR at an operator whose projector was completely blank. The single
   most prominent thing in the UI was confidently answering the wrong
   question.

   The truth was available: it was in a 10px monospace footer inside a
   monitor tile.

   Now the topbar says what is ACTUALLY on the wall, and NAMES it — "On Air
   · John 3:16" — with distinct states for Blackout and Screens clear. The
   microphone gets its own, quieter indicator, because it is a different
   fact and always was.

2. The keyboard cheatsheet was lying.

   registerContext() REPLACED the whole handler table. The Planner registers
   only next/prev, so on that tab `A` (accept the AI's suggestion), `D`
   (dismiss) and `/` (search) were DEAD KEYS — while the cheatsheet
   cheerfully listed all three.

   An operator pressing `A` mid-sermon to put a suggestion on screen would
   have got nothing, and no explanation. A help screen that lists a key which
   does nothing is worse than no help screen at all: it teaches something
   false, under pressure, to someone who has no time to investigate.

   Shortcuts now declare which action they need, and the cheatsheet renders
   only the ones the current surface can actually handle. The panic keys are
   marked always-on and appear everywhere, forever.

3. Accessibility.

   * aria-live count in the entire app was ZERO. A screen-reader operator was
     told NOTHING when scripture went on the wall or when the screens were
     cleared — the one thing they most need to know was the one thing the app
     never said. There is now a polite live region announcing exactly that.

   * --v-faint was #6c6b71 = 3.49:1, which FAILS WCAG AA (4.5:1) — and it is
     the colour of EVERY empty state and every placeholder, i.e. precisely the
     text a brand-new operator most needs to read. Now #88888d, solved against
     the LIGHTEST surface so it passes on all four (5.47 / 5.22 / 4.92 / 4.55).

195 Rust tests, 35 frontend tests, clippy -D warnings clean, app boots.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(db): close the 5 data-integrity gaps — one of them empties the Bible

All five were found by CodeRabbit on PR #1, all pre-existing, none
screen-facing. But one of them is genuinely nasty in this market.

1. reimport_full_kjv was NOT ATOMIC.

   It DELETES every verse and then re-imports 31,100 of them — during a
   migration, on app start. With no transaction, a crash or A POWER CUT
   leaves the church with an EMPTY BIBLE and an app that can no longer show
   a single verse.

   Power cuts are not an edge case for the churches Relay is built for. They
   are Tuesday. Now all-or-nothing: either the new corpus lands or the old
   one is still there, and there is no state in between.

2. move_plan_item silently did nothing after a delete.

   It looked for a neighbour at exactly position±1. Deleting a cue leaves a
   gap (0, 1, 3), so moving the cue at 3 looked for position 2, found
   nothing, and gave up — SILENTLY. The operator drags a cue and it simply
   doesn't budge, with no error, and rebuilding the plan is the only way out.

   It now finds the adjacent cue by ORDER, not by arithmetic. Gaps are
   irrelevant.

3. delete_media orphaned plan cues.

   It deleted the asset and the file but left every plan_items cue pointing
   at it. Those cues then sat in the service plan looking perfectly healthy
   and failed with "media not found" AT THE MOMENT THE OPERATOR FIRED THEM,
   live. A broken cue that looks fine until you press it is worse than one
   that is visibly gone. Songs and announcements already propagated their
   edits into plans; media was the one that didn't.

4. import_song was not transactional. A half-imported song is a song whose
   second chorus is missing — discovered mid-song, on a Sunday.

5. The Lower-Third forward-fill matched on NAME alone, so it would rewrite a
   template the OPERATOR built and happened to call "Lower Third" — silently
   changing their congregation's screen during a migration they never asked
   for. Now id-scoped to the built-ins, like its sibling check.

198 Rust tests (3 new, one per behaviour), clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(transport): Esc left the plan cursor pointing at a live cue — next → refired it

Three fixes, all in the seam between the panic keys and the Service Planner.

1. The panic keys did not reset the transport.

   Esc and B go through the raw clearScreens/blackScreen. Only the Planner's own
   ◼ button reset liveCueId/liveSlide. So: slide 2 of cue 7 is live, the operator
   panics and hits Esc, the screens clear — but the plan cursor still says cue 7,
   slide 2. They press → and slide 3 goes straight back up in front of the
   congregation. Exactly the thing they just pressed Esc to stop.

   Fixed at the store, not in the view: liveCue is now a store, and leavePlan()
   runs at the top of clearScreens, blackScreen, manualFire and confirmDetection —
   BEFORE the backend call, so it holds even if the call fails. No view can forget
   it, and the upcoming Console+Planner merge inherits it for free.

2. The crash-recovery promise was a lie.

   ServicePlanner wrote the session reactively:

       $: setSession({ planId: openPlan?.id ?? null, ... })

   which fires on mount with openPlan === null and overwrites the saved planId
   with null. And nothing read it back — crash.js only *displayed* it. So the
   crash panel told the operator "you'll come back to plan 7, cue 3, slide 2" and
   the app had already thrown it away.

   Now guarded on openPlan, and restored on mount. The cue/slide is restored too,
   without re-firing: the output windows are separate webviews and survive a
   console crash, so the verse is still on the wall. The transport must match what
   the congregation is ACTUALLY looking at.

3. In-app Help. There was none.

   The operator guide is a markdown file on GitHub — no use to a volunteer in a
   dark booth on a Sunday with the service starting in five minutes. Help that
   needs a network is missing precisely when Relay is most useful: offline.

198 Rust tests, 40 frontend tests (transport.test.js is new and covers 1).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat(live): merge Console + Planner — the operator never changes tabs mid-sermon

Console and the Planner's run mode should never have been two tabs.

THE FAILURE THIS REMOVES. The operator is on the Planner, running the plan. The
preacher goes off-script and quotes a verse. Relay detects it — and puts the
suggestion on a DIFFERENT TAB, which the operator is not looking at. The single
thing this product exists to do was happening somewhere they could not see it.

THE SECOND FAILURE. `→` meant two different things depending on which tab was
mounted: "next slide of the plan" in the Planner, "next verse of the passage" on
the Console. Same key, same finger, two outcomes, no way to tell which one you
were about to get. The transport now has an explicit MODE, printed in the bar,
and it follows what is actually on the wall:

    plan cue on air          → steps SLIDE
    detected/manual verse    → steps VERSE

so accepting a suggestion mid-plan hands the arrows to the passage, and Esc hands
them back to the plan — at the cue it was already on.

The split is now the one that exists in the church's week. BUILD a plan in the
Planner: a Tuesday job, unhurried, lots of searching and dragging, and nothing on
that screen can reach an output — an operator arranging next Sunday's songs must
not be able to fire one at a projector by clicking the wrong thing. RUN it in
LIVE: one surface, big targets, no typing, no drag handles.

PLAYHEAD vs ON AIR — I had this wrong, and the fix was worse than the bug.

My first pass wiped the plan cursor on Esc. That means the next → RESTARTS THE
PLAN AT CUE 1 — the opening countdown, back on the wall, at the end of the
service. Position and on-air-ness are two different facts:

  { cueId, slide }  survives everything. Where → resumes from.
  onAir             cleared the instant anything else takes the screen.

Only `onAir` is cleared by the panic keys, by a manual fire, by an accepted
suggestion. The position is remembered. A cue that is where → will resume but is
NOT on the screen reads CUED, in grey — never amber. Amber means live and is
never allowed to lie.

  · src/lib/plan.js       — cue/slide logic, pure, out of the component. The rules
                            deciding what goes on the congregation's screen next
                            were welded to a Svelte file and could not be tested.
                            11 tests, incl. "steps back onto the LAST slide of the
                            previous cue" and "never wraps at either end".
  · src/lib/OutputWall.svelte — the 4-monitor wall. Console and Planner each kept
                            a copy and they had ALREADY drifted. Two versions of
                            "what is on screen right now" is the one thing you
                            cannot have two versions of.
  · one registerContext for the whole live surface — A/D/→/←// all work together.
                            Before, whichever tab you were on had half of them dead.

A COMPILE IS NOT A BOOT — again. `vite build` exited 0, 51 tests passed, clippy
was clean, and the app booted. Vite had *warned* that a store import I dropped in
the refactor left `$capture.available` dangling in the Planner markup, which would
have thrown the moment an operator clicked that tab. Nothing in the pipeline was
looking, because the frontend has no type checker.

`missing-declaration` is now FATAL in vite.config.js. Verified by reintroducing the
bug: build exits 1. Restored: exits 0.

198 Rust tests · 52 frontend tests · clippy -D warnings clean · app boots, heartbeat
present, no panics.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* feat(rehearsal): practise a whole service with nothing reaching the congregation

A volunteer has to be able to learn this software, and the only realistic place to
practise is the room it runs in — the real projector, the real sound desk, the real
plan. Which is also a room where a stray verse on the wall during the 9am service is
exactly the thing we cannot allow.

GATED AT THE BROADCAST, NOT AT THE CALLERS. `channels::Rehearsal` is read inside
broadcast_content / clear / black — the three functions content leaves the machine
through. In rehearsal they emit to the operator console window ("main") and NOTHING
else: no output window, no kiosk WebSocket, no LAN HTTP.

There are seven fire sites today and there will be more. A rehearsal a new fire path
can forget about is not a sandbox, it is a promise waiting to be broken by the next
feature. At the choke point, every future caller is sandboxed by construction.

Everything upstream — detection, the router, the pipeline, the plan transport —
runs completely unchanged. A rehearsal that behaves differently from a service does
not rehearse the service.

It FAILS OPEN: `rehearsing()` is false wherever the state isn't registered. The
dangerous failure is silently swallowing content the operator believes is live, not
the reverse.

THE TWO WAYS TO BE WRONG ABOUT IT ARE BOTH BAD, IN OPPOSITE DIRECTIONS. Rehearsing
while you think you're live = the projector stays dark through the whole sermon.
Live while you think you're rehearsing = your practice run is on the wall in front of
everyone. So the app says so constantly and everywhere: a permanent band across the
console, REHEARSAL in the top bar on EVERY tab (the one indicator an operator glances
at cannot be tab-specific), the output-wall tally dashed instead of lit, and the
screen-reader live region.

Amethyst, never amber. Amber means ON AIR. A tally light that lies is worse than no
tally light.

  · Mutually exclusive with a recorded service, both ways. `start_service` refuses
    while rehearsing; `set_rehearsal` refuses while a service is recording. A
    practice run filed under last Sunday is a record nobody can trust afterwards.
  · The self-calibrating router does NOT learn from it. The volunteer is accepting
    verses they chose themselves, against speech that may be them reading aloud from
    a phone. That is not evidence, and the gate would carry the fiction into Sunday.
  · Leaving rehearsal CLEARS the screens. The wall has been showing whatever it was
    showing before, while the operator watched a console preview saying something
    else. Handing back a wall they have not looked at in twenty minutes, silently, is
    how the wrong thing ends up in front of a congregation.
  · `setRehearsal` is the one wrapper in capture.js allowed to THROW. Every other one
    swallows into `catch {}` — right for a device list, catastrophic here: a swallowed
    refusal leaves the operator believing they are practising.

Also: LIVE restyled into the old Console's "Spiritual High-Tech" language, as asked —
the framed Intelligence Feed, the gold AI suggestion card, the tally-bordered monitor
wall, the command bar. Same merged behaviour and mode-aware transport, the broadcast-
instrument look it had before.

docs/DECISIONS.md §18. 201 Rust tests · 57 frontend tests · clippy -D warnings clean ·
app boots, heartbeat present, no panics.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(audio): Relay was deaf to a quiet preacher, and said nothing about it

You said the transcript "delays to catch the next and next verse". It was not
delay. It was starvation, and it had three causes stacked on top of each other.
Every one of them was an ABSOLUTE LEVEL threshold tuned on a developer's machine.

MEASURED FIRST (per CLAUDE.md). Both of my first two theories were wrong, and the
benchmarks killed them before I shipped either:

  · whisper is NOT slow. An 8s window decodes in 207ms on 4 threads — a fifth of
    its budget — at every window size and thread count. (stt::bench, #[ignore]d)
  · the dev build is NOT the problem. Debug measures 156-214ms, same as release;
    whisper-rs builds its C library optimized either way.

THE ACTUAL BUG, measured end to end on real speech through the real front-end —
same words, differing only in level:

    studio level   94% voiced    perfect on the machine it was written on
    x0.2           17% voiced    a church laptop mic. Most of the sermon deleted.
    x0.05           2% voiced    a lightly-driven desk feed. Effectively deaf.

1. THE VOICE GATE WAS A LOUDNESS GATE. `rms >= 0.008` — a guess about a microphone
   and a room the developer never heard. Ordinary speech dips under that BETWEEN
   WORDS, so two thirds of a continuously-speaking preacher was classified as
   silence. Now it learns the room's noise floor and gates relative to it, with
   hysteresis so a breath mid-sentence cannot slam it shut.

2. THE AUTO-GAIN COULD NEVER LIFT A QUIET MIC. `energy_prob = rms / TARGET_RMS`
   with SPEECH_PROB = 0.55 means a frame had to reach RMS 0.066 to count as speech
   at all — so a feed at 0.005 produced no speech frames, `speech_level` never
   updated, and the gain stayed frozen at 1.0. A perfect deadlock: to be granted
   gain you had to already be loud enough not to need it. The front-end whose
   entire job is to lift a quiet feed left it exactly as quiet as it found it.

   Now the probability is measured against the room: contrast between the tracked
   floor and the recent peak. Speech is a RISE, not a volume. A steady tone is room
   tone however loud; a quiet voice over a quiet room is a voice. MAX_GAIN 6 -> 24
   (+16 dB could not reach the target from 0.005 anyway).

3. THE NOISE FLOOR ONLY LEARNED FROM FRAMES ALREADY JUDGED "NOT SPEECH" — so on a
   quiet mic, where nothing was judged speech, the preacher's own voice was folded
   into the noise estimate, which raised the floor, which kept him below the speech
   threshold, which kept him in the estimate. The front-end talked itself into
   believing the room was empty. It is now a minimum statistic: falls fast, rises
   over ~20s, tracked from every frame, needing no speech decision to make one.

And two more found on the way:

4. STT re-ran whisper once per BACKLOGGED SECOND. The audio channel is unbounded;
   if a decode ever overran the second of speech that triggered it, the worker
   drained the queue by decoding again, and again, each time on an already-stale
   window. It could never catch up and the lag grew for the whole sermon. It now
   drains the queue and decodes ONCE on the freshest window — catching up costs one
   decode however deep the backlog. No audio is dropped. (Not your bottleneck
   today; it is the one that bites the moment someone loads the medium model.)

5. THE STT WINDOW EXCISED SILENCE. Only VAD-passed chunks were appended, so whisper
   was handed speech with the gaps CUT OUT and spliced end to end. It is an acoustic
   model — the pauses are part of the signal. Now every chunk goes in once the
   speaker has started; the VAD finds the EDGES of an utterance, not its middle.
   SILENCE_FINALIZE 5 -> 7 (~1s -> ~1.4s): a pause for breath was ending the
   utterance and CLEARING the window, so "Romans chapter eight ... verse twenty-
   eight" was decoded with no memory of its first half.

6. ONE NaN SAMPLE KILLED THE AUDIO PATH FOREVER. NaN compares false against
   everything, so the gain, floor and peak all latched to NaN and stayed there —
   silently, for the rest of the service. A flaky driver or a device unplugged
   mid-callback is enough. Guarded.

RESULT — same speech, 100x range of mic level, voiced ratio now FLAT at 39-55%
where it used to collapse to 0%, and the auto-gain lands a 100x quieter input
within 3x of the loud one instead of passing it straight through.

None of this errored. Nothing warned. The level meter moved the whole time. It
failed invisibly, and it failed on exactly the churches this is built for — a cheap
mic at the back of a hall — who would never have reported it as a bug. They would
just have stopped using Relay.

docs/DECISIONS.md §19: audio levels are learned, never assumed.

New: audio::gate and stt::bench (#[ignore]d, real-speech harnesses). 206 Rust tests,
57 frontend tests, clippy -D warnings clean, app boots.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* perf(stt): benchmark the decoder — and change nothing, on purpose

The transcript heard "Romans 8 verse 2" where you said twenty-eight, so the obvious
next move was a stronger decoder: whisper is running `Greedy { best_of: 1 }`, its
cheapest and least accurate mode, while using only a fifth of its latency budget.

Benchmarked it instead of shipping it. Beam-5 buys NOTHING.

Scored through the REAL detector — not by grepping the transcript, but by asking the
question that matters: WHICH VERSE WOULD RELAY PUT ON THE SCREEN? — across clean,
quiet, noisy, very-quiet and quiet+noisy audio, both decoders and all three prompt
strategies recover 20/20 references with ZERO wrong verses. Beam-5 costs ~50% more
time to find exactly the same twenty.

So `Decode::Beam` exists, is benchmarked, and is deliberately not used. The next
person tempted by "just turn on beam search" gets the numbers instead of an opinion.

MY FIRST SCORING FUNCTION WAS WORSE THAN USELESS. It grepped the text for "28" and
"16", which scored a hallucinated `Peter 8 verse 28` (wrong book, right number) as a
SUCCESS, and scored a correct "chapter eight verse twenty-eight" as a FAILURE —
spelled out, and detection.rs parses spoken numbers perfectly well. It flattered the
option that hallucinated and punished the option that worked, and I nearly ripped out
the scripture bias prompt on the strength of it. Score through the detector, always:
what the transcript LOOKS like is not the product.

DEBUG AUDIO RECORDER (`RELAY_RECORD_WAV=path`, off by default, no UI for it).

Every audio bug in this session was invisible from the code and reproducible only
with a specific microphone in a specific room. Synthetic speech is too clean to
trigger them and the developer's laptop is too loud, so the only debugging tool left
was asking a human to say the same sentence over and over. Now the cleaned stream —
the exact samples the VAD and whisper see — can be written to a local WAV and the
failure reproduced offline, forever.

It is sermon audio: off unless an env var names a path, never uploaded, never
attached to a crash report, never enabled by anything in the UI. PRIVACY.md documents
it as what it is — a recording of the service.

206 Rust tests, clippy -D warnings clean, app boots.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* docs: CLAUDE.md was describing an app that no longer exists

CLAUDE.md is the context every future session loads before touching this code, so
when it is wrong it is worse than absent — it actively misleads. After this session
it described a Console tab that has been deleted, and carried none of what the
session actually cost us to learn.

  · Tabs are Live · Channels · Templates · Library · Planner · Settings · Help.
    BUILD a plan in Planner (a Tuesday job — and nothing there can reach an output).
    RUN it in Live (a Sunday job).
  · The transport is mode-aware and says so. `→` steps a SLIDE or a VERSE depending
    on what is actually on air, and the bar prints which.
  · `liveCue = { cueId, slide, onAir }` — position and on-air-ness are separate facts.
    The panic keys clear only `onAir`. Wiping the position makes the next `→` restart
    the plan at cue 1.

Three new hard-won rules, all of which cost real time today:

  12. AUDIO LEVELS ARE LEARNED, NEVER ASSUMED. Speech is a rise above the room, and
      it is contrast, not volume. Three individually-reasonable absolute thresholds
      together made Relay deaf to a quiet preacher, silently.
  13. SCORE STT CHANGES THROUGH THE DETECTOR, NOT BY READING THE TRANSCRIPT. A
      grep-the-text scorer rated a hallucinated `Peter 8 verse 28` a success and a
      correct "chapter eight verse twenty-eight" a failure. It flattered the option
      that hallucinated.
  14. MEASURE THE DECODER BEFORE "IMPROVING" IT. Beam-5 finds exactly the same
      references as greedy, for 50% more time.

And the tools to debug audio without a human at the microphone — RELAY_RECORD_WAV,
RELAY_STT_TIMING, RELAY_AUDIO_RMS, and the real-speech benchmarks. The next session
should not have to ask someone to say the same sentence eight times to find a bug.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(docs): every doc pointed OBS at a port that does not exist in the shipped app

Verified the PACKAGED build for the first time this session — `tauri build`, then
launching the actual release binary rather than `tauri dev`. Per CLAUDE.md, dev does
not exercise the CSP and dev is not what a church installs. It found a real one.

THE OUTPUT PORT IS 8032. THE DOCS ALL SAID 5032.

`5032` is the Vite dev server. It exists ONLY while a developer runs `npm run tauri
dev`. In the installed app there is no server on it at all — so an OBS/vMix browser
source, or a Raspberry-Pi kiosk, pointed at `http://host:5032/output.html` shows a
BLANK SCREEN. No error in the app. Nothing in any log. No way for a volunteer to work
out why the stream has no lower-third.

README.md, docs/USER_GUIDE.md and CLAUDE.md all said 5032. Channels.svelte has always
emitted 8032 correctly, so the app's own Copy-URL and QR buttons were right the whole
time while every document telling a human what to TYPE was wrong. And nothing could
have caught it in development, because in development there IS a server on 5032 and
everything looks fine.

Guarded now: a test asserts the port Channels.svelte hands out matches the port the
docs tell an operator to use. Verified by reintroducing the bug (test fails) and
removing it (passes).

WHAT THE PACKAGED BUILD PROVED (none of it exercised by `tauri dev`):
  · boots, with the webview heartbeat — so the CSP does not block the IPC bridge
  · the embedded frontend serves output.html + stage.html + every asset (200, not 404)
  · the kiosk WebSocket hub accepts an OBS-style client
  · the STT model resolves, the LAN HTTP server binds, no panics

Also docs/RELEASING.md: `npm run tauri build` bundles the .app and then DIES on the
DMG step with "Finder got an error: AppleEvent timed out (-1712)" — that is
bundle_dmg.sh driving Finder over AppleScript to prettify the disk-image window, and
it hangs on any Mac that will not grant Automation permission. Cosmetic. `CI=1 npm run
tauri build` skips it and produces a perfectly good DMG. CI already sets CI=true, so
the release workflow was never affected — this bites only a maintainer on their own
Mac, where it looks exactly like a broken release pipeline and is not one.

206 Rust tests, 59 frontend tests, clippy -D warnings clean, packaged binary verified.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(windows): make the platform nobody here can run defensible anyway

Windows is a day-one platform (docs/DECISIONS.md) that no one on this project owns a
machine for. Every Windows bug in this repo's history has had the same shape: it
COMPILED, every test PASSED, and the failure existed only in a bundle nobody had made
— `stt.rs` once hand-rolled `$HOME/Library/Application Support/…`, which shipped a
Windows build with speech recognition SILENTLY DEAD, because Windows has no HOME and
the model was simply never found. A doc comment saying "don't re-derive this path"
was already sitting right above it. It did not help.

So stop relying on people reading comments.

1. THE OS PATH LOGIC IS NOW PURE. `app_data_root(os, env)` takes the OS name and the
   environment as ARGUMENTS. It was welded to `cfg!(target_os)`, which means a Mac
   could only ever test the Mac branch and CI's Windows runner had no test to run —
   the bug was found by a human reading code, which is not a strategy. Every
   platform's behaviour is now tested from every platform, including the ones nobody
   here owns.

   And it pins down THE TRAP: "Windows has no HOME" is what everyone believes, and it
   is FALSE. Git Bash, MSYS2 and Cygwin all set HOME to a Unix-shaped path. A Windows
   build that reaches for HOME "because it works on my machine" writes the database
   and the 148 MB model somewhere the packaged app will never look. On Windows, HOME
   is not deprioritised — it is never read at all. There is a test named after it.

2. THE RULE IS ENFORCED, NOT DOCUMENTED. A test walks every .rs file and fails if any
   module except db/mod.rs reads HOME / APPDATA / USERPROFILE / XDG_DATA_HOME.
   Verified by reintroducing the original stt.rs bug: the test fails and names the
   file and line. Removed: passes.

   `export_service` was the last offender — it read HOME first, and survived on
   Windows only by accident (its `is_dir()` filter happened to reject Git Bash's
   `/c/Users/...`). Now `db::downloads_dir()`, which prefers USERPROFILE on Windows.

3. CI NOW BUILDS THE WINDOWS INSTALLER, every push. The .msi is what a church
   actually installs and it had NEVER ONCE BEEN BUILT — the first one would have been
   produced by a release tag, which is the worst imaginable moment to find out WiX is
   unhappy. CI's own comment admitted it: "don't read this step as packaging works".
   Now it does, and the .msi is uploaded as an artifact. (macOS stays compile-only:
   DMG bundling is proven, and it drives Finder over AppleScript — see RELEASING.md.)

4. THE FIREWALL FAILURE RELAY CANNOT DETECT. Windows Defender does not stop Relay
   from binding its ports; it stops OTHER MACHINES from reaching them. So if the
   operator clicks Cancel on the "allow Relay on your network" prompt, the bind
   SUCCEEDS, Relay reports itself perfectly healthy, the Channels tab looks normal,
   nothing appears in any log — and no OBS source, kiosk screen or stage monitor can
   EVER connect. Silently. Forever.

   It cannot be detected from inside the app, so it is documented where a volunteer
   will actually meet it (README + USER_GUIDE), with the symptom stated first:
   "networked screens blank, HDMI fine → it is the firewall". The bind-failure message
   is now Windows-aware too.

214 Rust tests (7 new for platform paths), 59 frontend, clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(release): the updater could never have updated anyone

CI's first-ever Windows bundle failed, and pulling that thread found that the auto-
update feature — the entire point of C3, "there was no way to ship a fix" — would not
have worked at all. Neither bug was findable without actually running `tauri build`,
which is why neither had been found: CI only ever compiled (`--no-bundle`), and
release.yml has never run, because no tag has ever been pushed.

1. `bundle.targets` DID NOT INCLUDE `app`.

   Tauri's macOS update bundle is `Relay.app.tar.gz`, derived from the `app` target.
   With targets `["msi", "dmg"]`, `tauri build` produced a DMG, exited 0, and created
   NO UPDATE BUNDLE AND NO SIGNATURE. Verified with a throwaway signing key: zero .sig
   files, zero updater artifacts.

   So a tagged release would have shipped installers that no existing installation
   could ever update from. Silently — the DMG works, the app runs, and every church is
   simply stranded on the version they first installed. Forever. Nothing errors and
   nothing appears in any log.

   Targets are now ["app", "dmg", "nsis", "msi"], and the same throwaway-key build now
   produces Relay.app.tar.gz AND Relay.app.tar.gz.sig.

2. `createUpdaterArtifacts` WAS `true` IN THE BASE CONFIG.

   Signed update bundles need TAURI_SIGNING_PRIVATE_KEY, which exists only as a CI
   secret — so with this true, ANY plain `tauri build` dies with "a public key has
   been found, but no private key". A contributor's, a maintainer's, CI's. Now false
   in the base config and true only in the release overlay. Releases sign updates.
   Nothing else should be trying to.

3. THE RELEASE OVERLAY WOULD HAVE FAILED OUTRIGHT ANYWAY.

   tauri.updater.conf.json carried a `"//"` comment block — and Tauri's schema forbids
   unknown top-level keys. `tauri build --config src-tauri/tauri.updater.conf.json`
   errors with "Additional properties are not allowed ('//' was unexpected)", so the
   FIRST EVER release tag would have failed on config parsing. JSON has no comments;
   the reasoning moved to docs/RELEASING.md, where it can be read anyway.

   The irony: that comment block existed to explain why the updater config is split
   out "so every CI run doesn't fail on a placeholder" — while itself guaranteeing the
   release would fail, and while leaving createUpdaterArtifacts true in the base config
   so CI failed on the placeholder regardless.

docs/RELEASING.md now documents how to VERIFY a release can update anyone, with a
throwaway key, in about a minute — because "it built" does not mean "it can ship a fix",
and that distinction is invisible until a church needs a fix and never gets one.

No key material is committed. The maintainer's real signing key remains theirs to
generate and never to share (docs/RELEASING.md).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---------

Co-authored-by: devgeereact <292055051+devgeereact@users.noreply.github.com>
Co-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
---
## 5b52a71 — 2026-07-12
devgeereact

chore(release): the real updater public key — releases can now ship a fix

The updater config carried a placeholder, so the first release tag would have failed
outright. It now carries the real PUBLIC key (minisign A829B89BEF1F2CBD), which is
public by design and safe to commit — it is what an installed Relay uses to verify
that an update genuinely came from us and was not swapped in transit.

The matching PRIVATE key was written straight to a file (`tauri signer generate -w`),
never printed, and piped into the GitHub secret from disk — so it never appeared in a
terminal, a log, or a conversation. It lives only at ~/.relay/updater.key (0600) and
in the repo secret TAURI_SIGNING_PRIVATE_KEY.

Verified end to end with the real key: `tauri build --config tauri.updater.conf.json`
now reports "Finished 1 updater signature" and produces

    macos/Relay.app.tar.gz
    macos/Relay.app.tar.gz.sig

which is the bundle an already-installed Relay downloads to update itself. Until the
previous commit it produced neither, silently — every church would have been stranded
forever on the version they first installed.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 96e882e — 2026-07-12
devgeereact

ci(release): unsigned builds allowed — but ONLY on a pre-release tag

Code-signing certificates take days to buy and Windows SmartScreen reputation takes
weeks of downloads to earn. release.yml refused to run without them, which meant the
release path could not be exercised AT ALL until the day it actually mattered. That is
precisely how release pipelines break — and this one already had three latent bugs in
it that only appeared when something finally ran the build.

But an unsigned build must never reach a church. macOS Gatekeeper says "Relay is
damaged and can't be opened"; SmartScreen warns. A volunteer does not push past those
screens — they close the tab and go back to typing verses into PowerPoint.

So the rule is now: unsigned is allowed ONLY on a PRE-RELEASE tag (one with a hyphen —
v0.1.0-rc1). A plain version tag with no APPLE_CERTIFICATE still FAILS, loudly. You
cannot ship an unsigned build to a church by accident, because you would first have to
type a tag that says, in the tag itself, that it is not a real release.

The release notes on an unsigned build say so in the body, in the first line, rather
than letting someone discover it when their Mac refuses to open it.

An unsigned pre-release still exercises everything that matters: real .dmg/.msi built
exactly as a real release builds them, and a SIGNED UPDATE BUNDLE — updater signing is
a different key from OS code signing, and that key now exists.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 9f14d10 — 2026-07-12
devgeereact

fix(release): an EMPTY signing identity is not the same as no signing identity

macOS release failed with:

    failed codesign application: failed to run command codesign: failed to sign app

Skipping the certificate-import step was not enough. tauri-action still passes
APPLE_SIGNING_IDENTITY, and when the secret is absent it passes the EMPTY STRING — an
empty-but-SET identity sends Tauri down the codesign path anyway, with nothing to sign
with. It reads like a certificate problem and is actually an empty-variable problem.

Building on a laptop works for the one reason that makes this hard to see: there the
variable is ABSENT, not empty.

Unsigned builds now pass `-` (ad-hoc signing) explicitly, so codesign succeeds and the
bundle is produced. Ad-hoc buys no Gatekeeper trust — the build is still unopenable
without `xattr -cr`, which is exactly what an unsigned pre-release should be. The
identity now says what it is instead of being blank and hoping.

Windows was unaffected and produced a complete release on the first attempt: .msi,
NSIS .exe, both .sig update bundles, and latest.json — the auto-updater works there
end to end, which is the first time that has ever been true.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 8ae01dc — 2026-07-12
devgeereact

product audit

---
## 828ce8c — 2026-07-12
Gideon Akinlotan

Fix the five criticals from the product audit: the product was reporting successes it did not achieve (#3)

* fix(release): the signing gate knew about Apple and nothing else (D1)

The pre-flight tested one secret — APPLE_CERTIFICATE — and called the whole
release "signed". There was no bundle.windows block anywhere in the repo, so the
WINDOWS_CERTIFICATE secrets the workflow passed to tauri-action were read by
nothing at all.

A real tag with the Apple secrets set therefore sailed through the gate, shipped
a correctly notarized .dmg, and shipped an MSI that had never been signed — and
the "unsigned build" warning in the release notes was keyed off the same single
flag, so it stayed silent too. Windows is the platform most of our churches are
on, on cost grounds.

Two platforms, two certificates, two independent verdicts:

- The gate is per-platform and fails loud on a real tag, naming the exact missing
  secrets. macOS additionally requires the NOTARIZATION credentials, not just the
  certificate: a signed-but-un-notarized app is still blocked by Gatekeeper, so
  from a church's point of view it is unsigned.
- Windows signing actually exists now. Two schemes, chosen by which secrets are
  set: Azure Trusted Signing (a signCommand invoking trusted-signing-cli) or a
  classic OV/EV .pfx (imported to the runner's store, found by thumbprint). The
  config is GENERATED per build and merged over tauri.conf.json with a second
  --config — it cannot be committed, because a thumbprint in the base config
  would break `tauri build` for every contributor on Windows who does not hold
  the certificate. It is gitignored.
- There is deliberately no combined `signed` output any more. One global "is it
  signed?" boolean standing in for two independent certificates IS the bug; every
  consumer now has to name a platform.

Verified by executing the extracted gate against every secret combination,
including the one that shipped: a real tag with only the Apple secrets now
refuses with "Windows is UNSIGNED".

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(release): the updater could never have delivered an update (D2)

The version was hard-coded to 0.1.0 in THREE files — tauri.conf.json,
package.json and src-tauri/Cargo.toml — and nothing in the release workflow ever
read any of them, or compared them to the tag.

tauri.conf.json's copy is what the update manifest advertises, and Tauri decides
"is there an update?" by comparing it as semver against what the church is
running. So `git tag v0.2.0 && git push` would have built the new binaries and
published a latest.json that stamped them version 0.1.0. Every existing install
would have compared that to its own 0.1.0, concluded it was already up to date,
and never updated. No error, no warning, no symptom: just a fix that never
arrives at the church that needed it.

That is the exact failure the updater exists to prevent.

- scripts/version.mjs owns all three files: `npm run version:set -- 0.2.0`.
- CI asserts they agree on every PR, so drift is caught by the commit that causes
  it rather than the tag that ships it.
- The release gate asserts they also equal the tag, before it builds anything.
- It rejects a version Tauri cannot parse as semver (a version it cannot compare
  is a version no church ever updates past), and refuses a workflow_dispatch
  fired from a branch, which would otherwise stamp a release "main".

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(live): the console told the operator things that were not true (D3, D4)

Two bugs, one family, and the family is the dangerous one: not a control that
fails, but a control that LIES about failing. An operator who is told the screens
are clear stops looking at the screens.

D3 — the panic path reported a success it did not achieve.

A failed clear was structurally unrepresentable. channels::clear discarded the
emit error with `let _ =`, clear_screens returned (), and clearScreens() swallowed
whatever was left — so Live.svelte flashed "Screens cleared" over a `catch {}`
that could never even fire. If the clear failed, the operator was told the wall
was clean while the verse was still in front of the congregation.

- channels::clear/black return Result; clear_screens/blackout are Result commands.
  The debounce is forgotten and the cue recorded ONLY on success: if the screens
  did not clear, the verse IS still up, and "forget what is on screen" would be a
  lie told to the router as well as to the operator.
- The two fire-and-forget paths got a voice. The SPOKEN "clear the screen" and the
  exit from rehearsal (which hands the wall back to the congregation) have no
  caller to return an error to, and both used to `let _ =` it. They now emit
  output://panic_failed. A spoken panic control that fails silently is exactly as
  dangerous as a keyed one.
- The frontend wrappers return a boolean AND set a global panicError store. Both,
  deliberately: the panic controls fire from a global keydown handler and from a
  shell button that must work when the current view has crashed, and neither can
  catch. A throw there is an unhandled rejection — silence with extra steps.
- The banner is not a toast: top of screen, role="alert", rose (never amber —
  amber is a tally light and is never allowed to lie), and it does not auto-
  dismiss. A message that fades after 2.6s is how the operator misses it.
- Escape no longer wipes the wall as a side-effect of closing the help overlay.
- The cheatsheet no longer claims `B` works while typing. It cannot: an operator
  typing "Habakkuk" into the reference box must not black out the room on the `b`.
  The behaviour was right; the promise was wrong.

D4 — the safety architecture was invisible at the moment of decision.

Relay's whole correctness story is that only a DIRECT match may auto-fire, at any
score, at any sensitivity, because a TF-IDF cosine is not a probability. The gate
is airtight in Rust and property-tested. The console rendered both kinds as
"AI suggestion — 92% match". We built a careful machine for keeping a human in the
loop, then showed the human nothing to be in the loop WITH.

- matched_text now crosses the IPC bridge (Cand -> Fire -> DetectionEvent, the one
  pipeline a verse already takes, so a sixth fire path gets it by construction).
- A paraphrase can explain itself: SemanticIndex::top_k_explained returns the terms
  that actually produced the cosine, ranked by their contribution to it. "Matched
  on: shepherd · lord" is something a volunteer can agree or disagree with. 0.61 is
  not.
- A guess looks like a guess: no gold, no glow, and NO NUMBER. Printing "61%"
  beside a cosine invites it to be read as "61% likely to be right", which is
  exactly what it is not. A number that lies is worse than no number, because it
  looks like information and gets acted on.
- Cyan, not amethyst: amethyst already means REHEARSAL (DECISIONS §18), and a
  colour meaning "nothing is reaching the congregation" cannot also mean "this
  guess is shaky".
- The presentation rule lives in lib/detect.js, pure and tested, not buried in a
  .svelte file where it could not be pinned.

Tests were checked against the bugs, not the fix: reintroducing each original bug
fails them.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(models): the download could hang forever, and the mic would die when signed (D5)

D5 — the model download neither succeeded nor failed, and could not be escaped.

The failure this module must survive is not "the download fails". It is "the
download neither succeeds nor fails, forever, and the operator cannot get out of
it" — a volunteer, an hour before the service, with no terminal.

- The stall deadline is ours, not the HTTP client's. The read loop waits on
  tokio::time::timeout(CANCEL_POLL, stream.next()) and gives up only after
  STALL_TIMEOUT with no byte at all. Deliberately NOT a whole-request reqwest
  .timeout(), which would abort a legitimately slow 148 MB download on exactly the
  connections this feature exists for. A stall is the gap between bytes, not the
  length of the download.
- Cancel works when the network is dead, which is the only time it matters. It was
  checked only AFTER a chunk arrived, so on a half-open TCP connection (a dropped
  wifi — the single most likely event in a church hall) the check was never
  reached and Cancel did nothing at all. A cancelled download keeps its .part:
  cancel means "stop", not "throw away my 90 MB".
- Cancel is no longer an error. It emitted model://error, so stopping your own
  download painted a red failure box — one with no dismiss, sitting directly above
  a working "Try again" button.
- `running` clears via a Drop guard, so it releases however we leave, panic
  included. The old bare store(false) after the await was never reached by the
  infinite hang, so the flag stayed set for the life of the process and every
  retry — even after the wifi came back — was refused with "A model download is
  already running" until Relay was quit and reopened. A recoverable blip became a
  dead feature.
- The 416 brick is gone. A .part of EXACTLY model.bytes is now settled by checksum,
  never by asking the server to resume from the end of it. The guard was
  `> model.bytes`, so an exactly-full part file sent Range: bytes=147951465-, got
  416 Range Not Satisfiable, hard-errored — and did not delete the file. Every
  retry hit the same 416, forever. The only escape was deleting a file the user did
  not know existed.

macOS microphone entitlement — the bug no build we can make locally would reveal.

Notarization REQUIRES the hardened runtime, and Tauri enables it by default. Under
it, opening an audio input device without com.apple.security.device.audio-input is
killed by TCC; and without NSMicrophoneUsageDescription, macOS terminates the app
the instant it ASKS.

    tauri dev             microphone works    (no hardened runtime)
    unsigned pre-release  microphone works    (ad-hoc signed, no hardened runtime)
    SIGNED + NOTARIZED    microphone DEAD

The first build correct enough to hand to a church would have been the first one
that could not hear the preacher. Nothing else is granted: Relay is not sandboxed,
and library validation stays ON (whisper.cpp is statically linked — we must not
weaken the hardening to pretend otherwise).

The usage string is not boilerplate; it IS the permission dialog, and the only
explanation a church ever gets for why this software wants to listen to their
service. It says the audio is transcribed on this computer and never sent
anywhere, which matches PRIVACY.md. If that stops being true, that string changes
first.

Both are pinned by models::config_boots — the module for invariants a compile
cannot catch. Note: the first version of those tests PASSED on a broken file,
because both plists quote the very keys being asserted on while explaining
themselves, so a grep matched the comment. They strip XML comments now, and that
is mutation-verified.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* docs: name the licensor, and record what the audit found

LICENSE carried an MIT grant with no copyright holder — it still said
"[Your name / organization]". An MIT licence naming nobody is the one outright
legal defect in the repo.

- LICENSE: Copyright (c) 2026 Gideon Akinlotan.
- PRODUCT_AUDIT.md: revision 2, re-verified against the code rather than carried
  forward on trust. Revision 1's three critical blockers had all shipped, along
  with most of phase 2 and all five data-integrity gaps — retiring stale findings
  is as much an audit's job as raising new ones. The new findings are a different
  species: bugs of FALSE CONFIDENCE, where the product reports a success it did
  not achieve. Each D-item keeps its original diagnosis next to its fix, because a
  fixed bug whose reasoning is deleted is a bug that gets rewritten.
- DECISIONS.md §20: a panic control may never report a success it did not achieve.
- DECISIONS.md §21: the operator must be shown WHICH KIND of claim the AI is
  making — no number is better than a misleading number.
- RELEASING.md: the version bump is now enforced, not a comment in a code block;
  Windows signing is documented for both schemes; the false claim that the updater
  "can be tested end to end today" from a pre-release is corrected (GitHub's
  /releases/latest skips prereleases, so the endpoint could not serve exactly the
  builds we can make before owning certificates) and replaced with a recipe that
  works; and the macOS microphone trap is written down so nobody "cleans up" the
  entitlements file.

Still owed by a human, and not by a commit: buy a Windows certificate, watch one
update install end to end, and run a real service with an operator who is not the
author.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(live): the transport key could do nothing, silently, mid-sermon

`nav` was a `()` Tauri command wrapping a `()` function with THREE silent bail-outs
inside it: a poisoned Context lock, stepping off the end of the passage, and
`fire_manual`'s return `bool` discarded outright. The frontend wrapper then swallowed
anything left into a `catch {}`.

So the operator presses -> mid-sermon, the wall does not change, and there is no
error, no toast and no log. On the key they press more than any other. It is the same
silent-no-op class as the "Screens cleared" lie (DECISIONS §20) and it survived that
fix.

The subtlety, and why a bool would have been the wrong repair: these outcomes are NOT
all failures. Reaching the end of a passage is a correct, expected boundary — the
operator simply needs to know that is why nothing moved. A verse missing from the
corpus is a real fault. Flattening them into "worked / didn't" is what hid this.

- handle_nav returns Result<NavResult, String>. NavResult distinguishes Fired,
  EndOfPassage, NoPassage and NotInLibrary — each gets its own sentence.
- The SPOKEN "next"/"back" runs on the STT thread, which has no caller to return to,
  so a nav that did nothing is pushed as `nav://blocked` and the console says why.
  Same treatment the spoken clear already gets.
- navVerse no longer swallows; the transport always reports what the key did.

Also fixes a bug in the new test file, which is worth naming because it cost more
time than the fix: `beforeEach(() => invoke.mockReset())` returns the mock (mockReset
returns it, and a concise arrow returns that implicitly), and vitest treats a value
returned from beforeEach as a TEARDOWN function — so it CALLED invoke() after every
test, with the rejecting implementation still installed, and reported the resulting
unhandled rejection as a failure of a test that had actually passed. A block body
returns undefined, which is what the other test files do.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

* fix(release): do not interpolate a tag name into a shell (review)

`${{ github.ref_name }}` inside a `run:` block is substituted by Actions BEFORE
bash parses the script, so a tag containing shell metacharacters would be parsed as
code. The gate's logic already read $GITHUB_REF_NAME from the environment; two
message/log lines still interpolated it directly.

`github.ref_type` is left as-is deliberately — it can only ever be "tag" or "branch".

Verified: the gate's nine cases behave identically, and a tag of
`v0.1.0; touch /tmp/pwned` no longer executes anything.

Also retires stale claims in PRODUCT_AUDIT.md that this PR itself fixed — the macOS
mic entitlement and the unnamed LICENSE holder were still listed as outstanding.
An audit that contradicts the tree it ships with is worse than no audit.

Both raised by CodeRabbit on #3.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---------

Co-authored-by: devgeereact <292055051+devgeereact@users.noreply.github.com>
Co-authored-by: Claude Opus 4.8 (1M context) <noreply@anthropic.com>
---
## a592019 — 2026-07-12
devgeereact

chore(release): 0.1.0-rc1

First exercise of the release pipeline end to end. A pre-release tag, deliberately:
no code-signing certificates exist yet, and the gate in release.yml only permits an
unsigned build on a tag that says, in the tag itself, that it is not a real release.

What this RC is for — proving the pipeline BEFORE money is spent on a certificate:

- the per-platform signing gate reports macOS and Windows independently (D1)
- the tag/repo version assertion runs for the first time (D2)
- real .dmg and .msi installers are produced, built exactly as a real release
  builds them
- a signed update bundle (.app.tar.gz + .sig) is produced, so the updater can be
  tested against this tag's manifest by exact URL (docs/RELEASING.md)

Expect the release notes to carry the UNSIGNED warning for BOTH platforms. That is
the gate working, not failing.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 92fa056 — 2026-07-12
devgeereact

chore(release): 0.1.0-rc3

rc1 and rc2 already exist as tags (and draft releases) from an earlier attempt,
pointing at 96e882e — before any of the audit fixes. Moving a published tag would
rewrite an artifact someone may already have pulled, so this takes the next free
number rather than reusing one.

This is the first RC that contains the D1–D5 fixes, and therefore the first one that
actually exercises the per-platform signing gate and the tag/repo version assertion.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 3159330 — 2026-07-12
devgeereact

fix(release): an empty Actions expression in a COMMENT invalidated the workflow

The comment added in the previous commit — warning against interpolating a tag name
into a shell — contained a literal, empty Actions expression as an illustration.

Actions evaluates expressions EVERYWHERE in a workflow file, including inside
comments in a run: block. An empty one is a parse error, so the whole file became
invalid. GitHub then could not even apply the `on:` filter, and emitted a zero-job
"startup failure" run for every push to any branch — which is why release.yml
appeared to be triggering on pushes to main, a thing its `on:` block forbids.

The v0.1.0-rc3 tag therefore built nothing. Retagging as rc4 once this lands.

The comment warning against interpolating into a shell broke the build by
interpolating into a shell. It now says so, so the next person does not repeat it.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 06e1766 — 2026-07-12
devgeereact

chore(release): 0.1.0-rc4

rc3's tag built nothing — the workflow file was invalid (see previous commit), so
GitHub emitted a startup failure instead of a build. Tags are not reused, so this
takes the next number.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## d9b2e98 — 2026-07-12
devgeereact

fix(release): two bugs the rc4 tag found — both invisible until you tag

The RC did its job. Neither of these can be reproduced by CI, by `cargo test`, or by
building locally; both only appear when a tag actually drives the release workflow.

1. macOS: "failed to notarize app: Error: Team ID must be at least 3 characters"

   APPLE_ID / APPLE_PASSWORD / APPLE_TEAM_ID were passed to tauri-action from an
   `env:` block unconditionally. On an unsigned build the secrets are absent, so they
   arrived as EMPTY STRINGS — and an empty variable is not an absent variable. Tauri
   reads "the notarization variables are present", tries to notarize, and dies on the
   empty Team ID.

   This is the identical trap as 9f14d10 ("an EMPTY signing identity is not the same
   as no signing identity"), one level deeper. There is no way to conditionally omit a
   key from an `env:` block — an expression yielding '' still sets it — so they are now
   exported to $GITHUB_ENV by a step that only runs when the build is genuinely signed.

2. Windows: "optional pre-release identifier in app version must be numeric-only ...
   for msi target"

   `0.1.0-rc4` is valid semver, builds a perfectly good .dmg, and is then rejected by
   the MSI bundler fifteen minutes into the release — on the platform most of our
   churches are on.

   And it cannot be dodged by avoiding pre-releases: the gate REQUIRES a hyphenated
   pre-release tag for any unsigned build, and D2 requires the version to equal the
   tag. Three constraints that only collide at the MSI step.

   So pre-releases are numbered, not named: 0.1.0-1, 0.1.0-2. `version.mjs` now
   rejects a named one, in a second, locally — instead of a quarter of an hour later
   in CI. Uglier version string; the trade is obviously worth it.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## fe460d4 — 2026-07-12
devgeereact

fix(ui): phase 2 — the wizard that proved nothing, and Escape wiping the wall again

Six operator-facing fixes from the audit's phase 2, plus one bug found while doing
them that is worse than any of the six.

FOUND WHILE FIXING: Escape cleared the congregation's screens from inside a modal.

The cheatsheet was guarded in the D3 pass; the arrangement pickers were not. Their
Escape handler is bound to the BACKDROP element, which never holds focus, so it never
fired — and Escape fell through to the global panic key. Opening an arrangement picker
and pressing Escape wiped the wall and left the picker open: the operator got the one
outcome they did not ask for and none of the one they did.

shortcuts.js now refuses to clear while ANY [role="dialog"] is mounted, read from the
DOM rather than from a registry of open overlays — a registry is a list somebody has to
remember to add the next dialog to, and the entire point is that this must not depend
on anybody remembering. The pickers bind Escape at the window, so it works wherever
focus happens to be.

THE FIRST-RUN MIC METER WAS DEAD.

`$meter` is only fed by the audio://chunk listener, which registers inside
startCapture() — and FirstRun never called it. So on the one screen whose stated
purpose is "a moving bar proves the microphone is actually hearing something", the bar
never moved and the hint fell through to "You can test this from the Live tab". The
step that exists to PROVE the microphone works proved nothing.

It now runs the microphone for real — with detection DISARMED, because an operator who
already has a speech model installed would otherwise have a detected verse auto-fire
onto the projector they were taught to open thirty seconds earlier, while they say
"testing, testing" into the mic. Restored on the way out, including on skip and on
unmount.

The rest:

- Mobile bottom nav was assigning to `active`, which is a DERIVATION of $session. The
  tab change was never persisted, and the next setSession() from anywhere (Live writes
  one on every slide) recomputed it and yanked the operator back. It calls go() now,
  like the desktop sidebar always did.
- Live showed "No service plans yet" on every mount, before listPlans() resolved — the
  one message that makes a new operator think they have lost their work. "Have we asked
  the database yet" is not the same question as "is the list empty".
- Stage.svelte (the PREACHER'S PHONE, read at arm's length in a lit auditorium) had the
  worst contrast in the product: 2.25:1 standby text, and #6c6b71 — the exact value
  app.css documents as REMOVED for failing AA. The console was fixed; the phone was
  left behind because it hardcodes hexes instead of tokens.
- A role="button" that answered to Enter but not Space is focusable and half-operable.
- Five raw Rust Err strings rendered to volunteers in a MONOSPACE font in Channels.
  One shared lib/errors.js humaniser now says what happened AND what to do — and still
  shows an unrecognised error, framed as a sentence, because hiding it is worse.

92 frontend tests (+6). Verified locally: `npm run tauri build` produces a working
0.1.0-1 bundle whose Info.plist carries the microphone usage string, and ad-hoc signing
it with the hardened runtime embeds com.apple.security.device.audio-input = true.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## eaac930 — 2026-07-12
devgeereact

docs(claude): bring CLAUDE.md up to the code it describes

It was written before distribution existed and had drifted badly: no updater, no
signing, no entitlements, no version script, a stale repo map, and none of the rules
this week's bugs taught us.

Added rules 15-23, each one a bug that reached (or would have reached) a congregation:

  15  a panic control may never report a success it did not achieve
  16  Esc must not clear the screens while a dialog is open
  17  the microphone DIES on the first correctly-signed macOS build
  18  the operator must see WHICH KIND of claim the AI is making
  19  the version lives in three files; drift means nothing ever updates, silently
  20  pre-release versions must be numeric or the Windows MSI rejects them
  21  an empty env var is not an absent env var
  22  Actions evaluates ${...} inside comments — an empty one invalidates the workflow
  23  a release is signed per-platform, or not at all

Also: honest "build status" (what ships, what is parked, what is built-but-dead),
the real test counts, single-test invocations, and the vitest beforeEach footgun that
cost forty minutes.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## fc84b19 — 2026-07-12
devgeereact

test(e2e): the path that puts scripture on a wall now has tests, and a migration brick

Phase 3. Two items, both from the audit's "largest known gaps".

1. main.rs had ZERO tests, and there was no integration test anywhere.

Every module BELOW the orchestration layer is well covered — detection, router,
pipeline, db. The wiring that turns "the preacher said John 3:16" into "John 3:16 is
on the wall" was verified only by a human driving the app by hand. That is the one
path where a regression is measured in Sundays.

src/e2e.rs drives the REAL commands (manual_fire, nav, clear_screens, blackout,
set_rehearsal) against a REAL in-memory database, through the REAL router and
pipeline, and asserts on the events that actually leave the machine. Nothing is
mocked but the window.

To make that possible, the fire engine is now generic over tauri::Runtime rather than
welded to the desktop shell — which is the useful half of "split main.rs": the point
was never the line count, it was that the engine could not be driven without a window.

Seven tests: a fired verse reaches the outputs with its text AND its template; next/
back walk the passage; nav REPORTS when it cannot move instead of doing nothing; clear
and blackout blank the screens and say that they did; a verse that does not exist never
reaches the wall (a garbled "Psalms 23:99" would blank the projector); and nothing
whatsoever escapes during a rehearsal.

Two of the seven failed on the first run, and both times the TEST was wrong, which is
worth recording:
  - a fresh install does not seed `tpl_scripture` — the per-content-type template is an
    override the operator sets, and without it the channel's own template is used. The
    test now sets one, so the "every fire carries its template" invariant is actually
    exercised instead of asserted against a vacuous None.
  - stepping past Jude 1:25 reports NotInLibrary, not EndOfPassage: an UNBOUNDED
    passage has no known end. Both answers are honest; only a bounded passage (a whole
    chapter) can report EndOfPassage. Both are now tested, separately.

The rehearsal test was mutation-verified: breaking the gate in broadcast_content fails
it. (My first mutation attempt silently didn't apply, and the test "passed" — which
would have left a vacuous test guarding the single most important promise in the
product.)

2. The detections migration could brick every future boot.

`ensure_manual_detection_status` rebuilds the table (SQLite cannot ALTER a CHECK). It
had no ROLLBACK: execute_batch stops at the first failing statement, so a failure left
the transaction OPEN, the following `PRAGMA foreign_keys = ON` executed inside it as a
documented no-op, and the Err propagated to open()'s expect and panicked the app at
startup — with FKs off and a transaction dangling. Worse, the leftover `detections_new`
scratch table made the next boot fail with "table already exists". Forever. Before the
window is even shown.

Now: DROP TABLE IF EXISTS first (so a crashed attempt is retryable), ROLLBACK on
failure, and best-effort cleanup. Five tests, including the leftover-scratch-table
brick — mutation-verified.

240 Rust (+12) · 92 frontend · clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 53f7e7e — 2026-07-12
devgeereact

docs(claude): fold in phase 3 — the e2e path, and two rules it taught

CLAUDE.md was refreshed earlier this session; phase 3 then invalidated three claims in
it. Corrected rather than rewritten.

- main.rs no longer "has zero tests, verified only by hand" — e2e.rs drives the real
  fire -> nav -> clear commands against a real in-memory DB. Added to the repo map,
  the commands, and the testing section, with the two footguns that cost time:
  generate_context!() cannot be expanded twice (_EMBED_INFO_PLIST), and a fresh install
  does not seed tpl_scripture, so the template assertion is vacuous unless the fixture
  sets one.
- 228 -> 240 Rust tests.
- The "largest known gap" is now typed errors + the throw-vs-swallow contract, not the
  untested fire path.

New rules:
  24  the fire engine is generic over tauri::Runtime — keep it that way, or the path
      that puts scripture on a wall becomes untestable again
  25  a migration must be RETRYABLE (DROP IF EXISTS the scratch table, ROLLBACK on
      failure) — the detections rebuild could brick every future boot

No Cursor or Copilot rules exist in this repo.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## d10d5c3 — 2026-07-12
devgeereact

refactor: typed errors, and a written throw-vs-swallow contract

Phase 3, finished. Both halves of the same disease: the frontend could not tell what
had gone wrong, or whether anything had gone wrong at all.

1. Typed errors (src-tauri/src/error.rs)

Every command returned Result<_, String>, built by map_err(|e| e.to_string()). So the
console could not distinguish

    "that verse isn't in your Bible"   (the operator can fix this)
    "the database is busy"             (retrying works)
    "the disk is full"                 (retrying will NEVER work)
    "a service is being recorded"      (a deliberate refusal, not a fault)

They were all just text. Which is exactly why Channels.svelte ended up rendering
String(err) in a monospace font at a volunteer: given a sentence and nothing else,
there is nothing else you can do with it.

Now `{ kind, message }` — refused / not_found / busy / io / internal — with From impls
so the compiler did the migration (92 signatures, 178 map_err sites). rusqlite errors
carry the one distinction that matters live: SQLITE_BUSY/LOCKED becomes Busy ("try that
again"), QueryReturnedNoRows becomes NotFound. The 12 literal validation refusals are
classified as Refused, keeping the sentences already written for a volunteer.

`Internal` is not a cop-out. A From<String> that labelled every legacy stringly error
as a user-fixable refusal would be WORSE than the strings were — it would look like
structure while lying about it. An unclassified error says it is unclassified.

Frontend: errors.js branches on kind, exports isRetryable/isRefusal, and no longer
renders "[object Object]" now that errors are objects.

2. The throw-vs-swallow contract, written down (capture.js)

~34 catch {} against exactly ONE throw in all of src/, with no rule. Half the wrappers
swallowed and returned [], half threw, and a caller could not tell which. The rule is
now one question — CAN THE CONGREGATION SEE THE DIFFERENCE? — and three groups:
throws (changes the screens / the AI / the mic), swallows (reads, where an empty list
is its own error message), reports-via-store (the panic controls, which fire from
places that cannot catch).

Two silent liars fixed by applying it:

- confirmDetection removed the suggestion card and called leavePlan() BEFORE the
  backend call, then swallowed any failure. The operator pressed A, the card vanished,
  acceptTop flashed "Now live: John 3:16" without awaiting anything — and the wall was
  unchanged. Same shape as the "Screens cleared" lie.
- setDetection swallowed, so the dot could read "off" while the AI was still armed and
  firing verses at a congregation.

3. And the transport was following intent instead of the wall

manualFire and confirmDetection reset the playhead BEFORE calling the backend. A fire
that FAILED therefore marked the plan off air — while the plan's slide was still on the
congregation's screen. The next → would walk a verse passage nobody could see, firing
content the operator never asked for. The old comment claimed this ordering was the
safe one. It was not.

Nothing moved on the wall, so nothing moves here. The panic controls remain the
deliberate exception (a panic key that half-works is worse than one that does not) —
and they now report their own failure loudly rather than swallowing it.

246 Rust (+6) · 101 frontend (+9) · clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 3863bab — 2026-07-13
devgeereact

fix(a11y): focus traps, a heading structure, and the AI announcing itself

The audit scored accessibility 4/10. The specific proof was that a screen-reader
operator was told nothing about the one thing this product exists to do.

FOCUS. Three dialogs, ZERO focus traps, and only the crash overlay even moved focus
into itself. A keyboard operator opening the first-run wizard could Tab straight past
it into the console behind — driving controls they cannot see, in a modal they cannot
leave. lib/focus.js is one Svelte action (`use:trapFocus`), now on all four dialogs
(first-run, cheatsheet, both arrangement pickers). It wraps Tab at both ends, focuses
the first real control rather than the container, and — the half everyone forgets —
gives focus BACK to whatever opened it.

It deliberately does NOT bind Escape. Escape is a panic key in this app; exactly one
place decides who may consume it (shortcuts.js, which refuses to clear the screens
while any [role="dialog"] is mounted). Two opinions about a panic key is how the wall
gets wiped by accident.

Two things the tests caught that I had wrong:
  - `offsetParent !== null` is the usual "is it visible" idiom and it is a trap: it
    depends on layout, so under jsdom it reports EVERY element hidden — the focus trap
    silently becomes a no-op and its tests pass by finding nothing to do. Uses
    checkVisibility() with an attribute fallback.
  - a trap whose dialog has been detached kept grabbing Tab (found when a leaked trap
    in the test file hijacked the NEXT test's Tab). Guarded on node.isConnected.

HEADINGS. No <h1> anywhere in the shell, and Live ran h3 → h2 → h3. Heading navigation
— a screen reader's primary way of skimming a page — was useless across the whole app.
The page title is the h1; section headings are peer h2s. Semantics and visual weight
were coupled through the tag (the h3 rule carried a dimmer colour), so the step down is
now a class, and the design is unchanged.

ANNOUNCEMENTS. The AI hearing a verse — the product's entire reason to exist — arrived
in complete silence. So did every transport message ("Now live: John 3:16", "End of the
passage") and every error. Now: the suggestion feed announces what was heard and how to
act on it (polite: an offer, not an emergency), the transport flash announces what the
keypress DID, and errors are assertive because an operator acting on a failed command is
about to make it worse.

CONTRAST. The legacy --text-faint was 2.27:1 at worst — below AA on every surface it
sits on. Every text token in the app now passes 4.5:1.

I did NOT delete the ~150 lines of dead legacy CSS around it, though the audit called
for it. Svelte does not scope a global stylesheet, and those rules use generic class
names (.tab, .dot, .live, .chip) that components still carry — deleting them could
silently restyle the app, and this machine cannot screenshot one to check. The gun is
unloaded, not removed. It needs eyes on a running app.

110 frontend tests (+9).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 3ceb241 — 2026-07-13
devgeereact

feat(live): surface related scripture — the feature that was built and never called

19 themes, keyword-scored, resolved to verse text, a registered Tauri command, fully
unit-tested — and ZERO frontend callers. Its own doc said "the console CAN poll this."
It never did.

Built code that nothing calls rots. It drifts out of step with the payloads around it
and nobody finds out, because nothing exercises it. It was one of two options the audit
gave — surface it or delete it — and it earns its place: the preacher is talking about
fear, and the operator has four verses on fear one click away, without leaving Live and
without typing.

It is the QUIETEST thing in the feed, deliberately.

Nobody SAID these references. It is a keyword match against 19 themes — the weakest
evidence anywhere in the product, weaker even than the TF-IDF paraphrase above it,
which at least keys on the preacher's own words. So:

  - no tally colour. Amber means ON AIR, cyan means "a paraphrase guess", amethyst
    means rehearsal. A colour that already carries a promise cannot be borrowed for a
    hunch (DECISIONS §18, §21).
  - no confidence, of any kind. There is no number here that would mean anything.
  - it says out loud, on screen, that nobody said them: "a topical suggestion".
  - it does nothing until the operator clicks it — and then it goes through manualFire,
    so it is recorded as a HUMAN's decision, never the AI's. The self-calibrating
    router learns from that column.

Pull-based and debounced (1.5s, on the tail of the transcript, skipped when nothing new
was said). Each call does a DB lookup per reference; polling on every transcript update
would be a query storm for a feature nobody asked for. It respects the detection toggle
— an operator who disarms the AI has disarmed this too — and the pending timer is
cleared on destroy.

113 frontend tests (+3). The IPC contract test now covers the command, which is exactly
the protection it was missing: a rename of `related_scripture` in Rust would have
failed silently inside a catch {} and the chips would simply have stopped appearing.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 8844721 — 2026-07-13
devgeereact

feat(ui): EmptyState / Loading / ErrorState — three facts that were being conflated

"No rows on screen" was being used to mean three different things:

    we asked, and there is nothing here   → EmptyState
    we have not finished asking           → Loading
    asking failed                         → ErrorState

Live rendered the FIRST when it meant the SECOND. So an operator with a full plan
library was told "No service plans yet" on every single visit, for the few frames
before listPlans() resolved — the one message that makes a new operator think they have
lost their work. And only TWO views in the whole app had a loading state at all.

Four competing classes for "empty" (.r-empty, .empty, .chan-empty, .cat-empty), and the
most important screen in the product used the one that was NOT the shared one.

The components deliberately render the EXISTING global .r-empty class rather than
inventing a new look. Consolidating markup must not silently restyle eight screens, and
this machine cannot screenshot one to check.

WHAT EACH ONE PROMISES, and why they differ:

  EmptyState  — NOT announced. An empty list is not news; it is already read out when
                the operator navigates to it, and announcing it again talks over them
                for nothing.
  Loading     — IS announced, politely, with aria-busy. A sighted operator sees the
                word; a screen-reader operator was told nothing, so a slow query was
                indistinguishable from a dead button. The pulse is grey, NOT amber —
                amber is the tally light and means ON AIR (DECISIONS §18). A spinner
                that borrows the on-air colour is a tally light that lies.
  ErrorState  — ASSERTIVE. The only one of the three that interrupts, because an
                operator acting on a command that silently failed is about to make it
                worse, and mid-service they will not go looking for a message they were
                never told about.

AND IT PAYS OFF THE TYPED ERRORS. ErrorState only offers "Try again" when retrying could
actually help — the backend now says whether a fault is transient (error.rs: Busy vs
Io). A retry button that cannot possibly work is worse than no button: the operator
presses it instead of fixing the real problem. As bare strings, "the database is busy"
and "the disk is full" were indistinguishable sentences.

Also fixes an audit finding in passing: History's empty state told the operator to
"start listening in Settings". The transport is on LIVE. An empty state that points a
new operator at the wrong screen is worse than no empty state.

124 frontend tests (+11), and the build now has zero unused CSS selectors.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## cfa2aa5 — 2026-07-13
devgeereact

feat(i18n): Relay can finally speak to its operator, not just listen to the preacher

Relay listens to a sermon in Yorùbá, detects the verse, and then talks to the volunteer
running it — in English. It understands three African languages and cannot say a word of
any of them to its own operator. LANGUAGES.md is honest about the acoustic gap; this was
the gap nobody had written down.

WHAT SHIPPED

- src/lib/i18n.js — 60 lines, no dependency. The whole feature is a lookup table and a
  store; every i18n library worth the name brings a bundler plugin, a message-format
  parser and an ICU runtime, which is more code than the thing it does, in an app whose
  first commitment is working offline on a donated laptop.
- src/lib/locales/{en,yo,sw,ha}.json — 57 keys, live surface first.
- A language picker in Settings, showing HONEST coverage per language.
- The live surface, the shell tabs and the nav notices now speak through the catalogue.

I DID NOT WRITE THE YORÙBÁ, SWAHILI OR HAUSA.

Those files ship EMPTY, on purpose, and say so at the top. I would be guessing. Same rule
as book_aliases.json and numerals.json, and for the same reason: the volunteer reads these
words under pressure, in a dark booth, with a congregation waiting, and a plausible-looking
wrong word from a non-speaker is worse than an honest English one. A translation is now a
DATA contribution — one JSON file, no Rust, no Svelte, no build — which is the whole point
of building the layer this way.

A PARTIAL TRANSLATION IS A WORKING TRANSLATION.

Missing keys fall back to English, key by key, forever. That is the design, not a
limitation: a locale ships the day it has one useful string in it. Hiding a language until
it is "complete" means it never ships, because a live product is never complete. The one
thing that must never happen is a BLANK label — a missing key resolves to English, and a
key missing from English resolves to the key itself, which is ugly, visible, and therefore
gets fixed.

Settings shows "0% translated" against Yorùbá today. That number is shown because it is
TRUE: an operator who picks Yorùbá and gets an English console would otherwise conclude
the feature is broken. It is not broken — it is unwritten, and the number says so and
invites them to be the one who writes it.

THREE BUGS THE TESTS CAUGHT, ALL MINE

- `t` was both the i18n store and the {#each tabs as t} loop variable. Svelte caught it.
- Renaming that loop variable, my regex rewrote `tabs.some((t) => t.key === …)` into
  `(t) => tab.key`, referencing an undefined name. The BUILD PASSED — it is a runtime
  ReferenceError, and the shell would simply have failed to render.
- The key-scanner test then found a doc-comment in i18n.js citing a key that does not
  exist. Harmless, but it is exactly the drift the test exists to catch.

That scanner now walks every .svelte/.js file and asserts every rendered key exists: a
typo in $t('…') does not fail the build, it silently prints "live.no_plans" at a
volunteer.

138 frontend tests (+14).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## c3caa64 — 2026-07-13
devgeereact

docs(audit): revision 3 — everything a commit could close, is closed

Rev 1 said Relay had to become INSTALLABLE. Rev 2 said it had to become HONEST — it was
reporting successes it had not achieved. Both are now done, and the second was the harder
and more valuable of the two.

Rev 3's finding is short: THERE IS NOTHING LEFT TO FIX BY TYPING.

Retires 14 findings, with evidence:
  D1 unsigned Windows · D2 the updater that could never update · D3 the panic path that
  lied · D4 the invisible safety architecture · D5 the download that hung forever · the
  macOS mic entitlement · the silent nav key · main.rs with zero tests · 88 stringly
  errors · 34 catch{} with no contract · the migration that could brick every boot ·
  accessibility at 4/10 · Empty-vs-Loading-vs-Error · related_scripture built and never
  called · no i18n layer at all.

Scorecard: overall 6.5 → 8. Accessibility 4 → 8, Testing 6 → 9, Architecture 6 → 8.
Every number re-verified against the tree, not carried forward: 246 Rust + 138 frontend,
main.rs 2,922 lines, 5 dialogs trapped, yo.json 0 keys, LICENSE named.

The NEGATIVE claims were re-verified too, because an audit that flatters the code it
ships with is worse than no audit: README still says "rename freely", USER_GUIDE still
names a Console tab that does not exist and still never mentions the speech model, there
is still no CONTRIBUTING.md, and there is still not one second of sermon audio in the
repo — which is why the moat is still a claim rather than a number.

What is left, in full:
  a Windows certificate (~$10/mo — the gate now REFUSES to ship without it)
  a GitHub billing page (the repo is private; macOS runners bill at x10, and Relay is
    MIT open-source by recorded decision)
  thirty minutes of a real preacher on tape (the bench that consumes it is already
    built and dormant)
  people who speak Yorùbá
  and a real service, run by an operator who is not the author.

First time in three revisions that the honest answer to "what is blocking Relay?" is not
a line of code.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 40572f3 — 2026-07-13
devgeereact

docs+bench: close the last three audit findings — and build the ruler for the moat

Three things the audit called out. Two were documents. The third was not, and it is the
important one.

1. USER_GUIDE.md was written for a developer.

It opened by explaining what `localhost:5032` is, said "the five screens" and listed six
(there are seven), named a Console tab that has not existed for weeks — and NEVER ONCE
MENTIONED THE SPEECH MODEL, which is the one step nobody expects and without which Relay
cannot hear a thing.

Rewritten for the person who actually reads it: a volunteer, on a Sunday, who has never
seen a terminal. Leads with the 10-minute setup and the model download. Has a real
troubleshooting table — the bar doesn't move, the transcript is nonsense, "the screens
may still be live", Relay crashed mid-service. And it explains the one thing an operator
must understand: HEARD vs GUESSED, and that a paraphrase can never reach the wall by
itself.

2. CONTRIBUTING.md did not exist, on a project whose docs actively solicit PRs.

It now leads with the two most valuable contributions to this project, BOTH OF WHICH NEED
NO CODE AT ALL:

  - a native speaker, for locales/{yo,sw,ha}.json (shipped empty on purpose), the 66x3
    book aliases (never reviewed by anyone who speaks the languages), and the Yoruba
    numerals (subtractive, vigesimal, and deliberately not hand-authored by an AI — a
    wrong numeral does not fail safely, it silently shows a DIFFERENT VERSE)
  - thirty minutes of a real sermon on tape

3. THE MOAT. This is the one that was not a document.

Relay's word error rate has never been measured. In any language. Including English.
Every revision of the audit has said so. The reason was never that the maths is hard.

So: the RULER is now built, and it is pure — no audio, no model, no whisper.
`stt::bench::wer` is Levenshtein over words, folding punctuation exactly the way the
detector folds it (a scorer that normalises the two sides differently is not measuring
anything), and deliberately NOT clamped at 1.0 — a decoder that hallucinates is worse
than one that says nothing, and the number must be allowed to say so. It is unit-tested
and runs in CI TODAY, with no recording in existence.

A test I wrote failed, correctly, and taught me something: "John 3:16" and "john 3 16"
are different tokens, because normalize() keeps a colon-joined reference as one word.
That is right, not a bug — and the consequence is now written into bench/README.md in
capitals: THE REFERENCE TRANSCRIPT MUST BE WRITTEN AS SPOKEN, or the scorer charges the
decoder for the transcriber's formatting.

bench/README.md says exactly what to record (a real preacher, the church's own mic, the
room as it is, code-switching included) and how to run it — including at RELAY_BENCH_SCALE
=0.2, the level at which Relay once went SILENTLY DEAF (94% voiced at studio level, 2% at
a church laptop, no error, no warning, just a transcript quietly turning to nonsense).

bench/.gitignore refuses *.wav, *.f32, *.mp3 AND *.txt. Verified: `git add bench/` stages
the README and the guard and REFUSES the audio. PRIVACY.md promises a church that sermon
audio never leaves their device, and that promise is not conditional on the device
belonging to a church. Keep the recording. Commit the number.

The instrument is calibrated. It is pointed at nothing.

250 Rust (+4) · 138 frontend · clippy -D warnings clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## c18ec7d — 2026-07-13
devgeereact

docs: CODE_OF_CONDUCT.md and CHANGELOG.md — the last two contributor gaps

CODE_OF_CONDUCT.md — Contributor Covenant 2.1, unmodified.

The standard text on purpose. A code of conduct someone invented themselves is one
nobody has read, and the entire value of this document is that you already know what it
says before you open it. Enforcement contact matches SECURITY.md.

One line added before it, because it IS specific to Relay: much of this project's most
important work will come from people who are not programmers — a Yoruba speaker
correcting a book name, a volunteer describing what went wrong in the booth on Sunday,
a sound engineer explaining the desk feed. Those contributions are worth more than most
pull requests, and the people making them are often new to GitHub. "Did you even read
the docs" is never the answer. If someone had to ask, the docs failed, not the person.

CHANGELOG.md — and this one is LOAD-BEARING, not paperwork.

Relay auto-updates. Which means the changelog is not a developer artifact: it is what a
church volunteer reads in the update banner, twenty minutes before a service, while
deciding whether to restart the app. An update that offers them that gamble with no
explanation of what changes is asking them to take it blind.

So it is written for the operator, and the maintainer note says so explicitly:
"Fixed a race in the router" is not a changelog entry. "The wrong verse could appear if
two references were spoken in the same sentence" is.

It is also HONEST about where the product stands. The top section says, plainly, that
nothing has ever been released — every tag so far is a draft pre-release used to
exercise the pipeline, and the first real release is blocked on one purchase. Verified
before writing it: zero non-draft releases exist.

The 0.1.0 section lists the bugs fixed BEFORE anyone was hurt by them — Relay silently
deaf to a quiet preacher, panic keys that lied, a transport key that did nothing, an
updater that could never have updated, a signed macOS build with a dead microphone, a
migration that could brick every boot. They are listed because they are the reason to
trust the next release, not despite it.

That closes the last contributor-infrastructure gap in the audit. Only GitHub issue/PR
templates remain, and those are genuinely minor.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 38733c0 — 2026-07-13
devgeereact

docs(github): issue forms and a PR template — built around who actually files them

Three issue forms, a config, and a PR template. Not boilerplate: each one is shaped by
who opens it and what Relay actually gets wrong.

bug_report.yml — the FIRST question is "did this happen during a live service?"

Because that is the question that decides how fast we act. Relay fails in front of a
congregation, and nothing else it does matters more. The form also asks about the
microphone and the room, in detail, because every audio bug in this project so far was
invisible in the code and reproducible only with a specific mic in a specific room —
Relay was once silently deaf to a quiet preacher and nobody noticed for months.

It says, up front: you do not need to be a programmer to file a useful bug. "The screen
went blank when the preacher said John 3:16" beats most stack traces.

And it says: DO NOT ATTACH SERMON AUDIO. We cannot accept it. PRIVACY.md promises a
church their audio never leaves their device, and that promise is not conditional on who
is asking. It points at bench/README.md instead: run the measurement locally, send the
number.

language.yml — the most valuable issue anyone can open, and it needs no code.

66 books x 3 languages, never checked by anyone who speaks them. A wrong book name fails
SILENTLY: the preacher says it, Relay hears it perfectly, matches nothing, and no verse
reaches the screen. Nobody ever finds out why.

It has one required checkbox: "I speak this language (or I'm checking with someone who
does)". We do not accept guesses on language data — a wrong numeral silently shows a
DIFFERENT VERSE, and nobody finds out until a Sunday. That is the same reason an AI did
not just fill these files in.

feature_request.yml — points at DECISIONS.md first.

Half of all feature requests to this project will be things already decided against, with
reasons. Better they read the reasoning than get a one-line "no" — and if they think the
reasoning is wrong, that is a genuinely useful issue.

config.yml — blank issues stay ENABLED, deliberately.

A volunteer who cannot make their problem fit our forms should not be turned away at the
door. That is our failure of imagination, not theirs.

PULL_REQUEST_TEMPLATE.md — the checklist is the project's real rules.

Not "did you add tests". No unwrap() in a live path. No error swallowed where the
congregation can see. No Mutex held across an emit (that deadlocked the macOS main loop,
twice). No borrowing a colour that already carries a promise — amber is ON AIR, amethyst
is REHEARSAL. A paraphrase still cannot auto-fire at any score.

And the one that matters most: "I reintroduced the bug and checked my test fails."
Several tests in this repo initially PASSED ON BROKEN CODE — a focus trap whose
visibility check reported every element hidden under jsdom, an entitlement test that
grepped a comment instead of the config. A test that cannot fail is not a test.

Translators get told to delete the whole template.

Verified: all four YAML files parse against GitHub's issue-form schema, every field has a
unique id, and every one of the 6 linked documents exists.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## c3f9999 — 2026-07-13
devgeereact

docs(audit): fold in the docs, the templates, and the ruler

Rev 3 was written before the last three commits landed. Aligning it, and re-verifying
every claim against the tree rather than carrying any of them forward.

Retired from the weaknesses list (a struck-through weakness is clutter, not a finding):
  - USER_GUIDE.md written for a developer  -> rewritten for a volunteer
  - no contributor infrastructure          -> CONTRIBUTING / CoC / CHANGELOG / issue
                                              forms / PR template all ship
Seven weaknesses remain, down from nine, and only two of them can be fixed by typing.

Added to the change table, and it is the one that matters:

  THE RULER FOR THE MOAT IS NOW BUILT.

  Relay's word error rate has never been measured, in any language, and every revision
  of this document has said so. It was never that the maths is hard. `stt::bench::wer`
  is now written, unit-tested, and runs in CI TODAY — with no recording in existence.
  Levenshtein over words, folding punctuation exactly the way the detector folds it (a
  scorer that normalises the two sides differently is not measuring anything), and
  deliberately NOT clamped at 1.0, so a hallucinating decoder scores worse than a silent
  one.

  bench/.gitignore refuses to let sermon audio into the repository at all — PRIVACY.md's
  promise that audio never leaves a church's device is not conditional on whose device it
  is. Keep the recording. Commit the number.

  The instrument is calibrated. It is pointed at nothing.

Scores: Documentation 7 -> 9, Developer experience 8 -> 9, Overall 8 -> 8.5.

Verified before committing: 250 Rust tests (the audit says 250), and every one of the six
documents the audit claims ships actually exists.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 3840d9e — 2026-07-13
devgeereact

fix(ui): the setup wizard can be asked for; delete 89 orphaned CSS rules

Two of the three remaining code weaknesses in the audit. The third (main.rs is one big
file) is a readability complaint, not a correctness one, and can wait.

1. The first-run wizard could not be re-run.

It never appears uninvited — that is deliberate and stays: a wizard that reappears is a
wizard that gets clicked through blindly. But it was also unreachable. An operator who
skipped it, or who inherited the laptop from whoever ran the desk last year, could not
get it back AT ALL — and it is the only place that walks you through the projector, the
microphone and a real verse on a real screen in one go, ending with you having SEEN it
work.

Never showing up uninvited and never being reachable are two different things, and only
the first one is a good idea. `restartSetup()` + a button in Settings.

It is deliberately NOT a reset: it touches setupDone and nothing else. An operator may
open Settings while a service is running, and losing the playhead would restart the plan
at cue 1 — the opening countdown, back on the wall, at the end of the service. Tested.

2. 89 orphaned CSS rules deleted — carefully, because the naive version was wrong.

The audit twice refused to do this, on the grounds that Svelte does NOT scope a global
stylesheet, so deleting a rule whose class is still on an element silently restyles the
app — and this machine cannot screenshot one to check.

That caution was correct. A rigorous scan of every class rendered by every component
(and the root HTML entry points) found that 13 of the 86 legacy classes ARE still live:
.active .brand .channels .clock .dot .idle .live .lower-third .mono .on .ref .sel .verse.
Deleting the block wholesale, as originally proposed, would have restyled all of them.

So only rules whose classes are ALL provably unrendered were removed: 89 of 121, 486 ->
396 lines. Verified afterwards that not one class the app renders lost its styling.

Worth recording how close this came to going wrong: my first attempt started the block
mid-`:root{`, so the brace depth was off, 159 lines parsed as 5 "rules", and it deleted
live ones. Caught by asserting brace balance and by re-checking every rendered class
against the result — not by looking at it, which I cannot do. A refactor you cannot see
the result of needs a check that does not require seeing it.

--text-faint stays (Channels uses it as a chip colour, and the output window shares this
file) — but at 4.54:1 rather than the 2.27:1 it failed AA with.

250 Rust · 140 frontend (+2) · build clean, zero unused-CSS warnings.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 022e08a — 2026-07-19
devgeereact

Add tests for DetectionInspector, implement BrandMark component, and create Dashboard view

- Introduced `inspector.test.js` to validate the behavior of the DetectionInspector component, ensuring it handles confidence percentages correctly based on detection methods.
- Created `BrandMark.svelte` to encapsulate the Relay waveform mark, ensuring consistent branding across the application.
- Developed `Dashboard.svelte` to provide a comprehensive overview of system readiness, integrating health checks, quick actions, and recent services, while maintaining a clear user interface.

---
## dbbe2ea — 2026-07-19
Oluwawunmi

feat(audio): choose the speaker video sound plays on

Videos rendered with `muted`, so a clip fired to the wall was silent and
there was no way to pick an output. Adds an Audio Output card stacked
directly under Audio Input in Settings.

The speaker list comes from the WEBVIEW (navigator.mediaDevices), not
cpal. Routing a <video>'s sound needs setSinkId(deviceId), and cpal's
device NAMES are a different namespace setSinkId can never accept — a
cpal-backed picker would have looked correct and routed nothing.

Measured on this app's WKWebView: setSinkId IS supported, but macOS
returns no audiooutput entries (blank ids even for inputs) until a media
permission has been granted once. So an empty list means "not unlocked
yet", never "no speakers" — the picker is never disabled, "System
default" is always a real working choice, and a Detect speakers button
trips the permission then releases the mic immediately, because cpal
owns the real capture and must not share it.

Sound is opt-in per surface (the `audio` prop). The fullscreen output
window gets it; the Templates editor preview stays muted (editing must
not blast audio), and the kiosk/OBS page stays muted because OBS mixes
browser-source audio itself.

If unmuted autoplay is blocked, playback falls back to MUTED rather than
not playing at all — a sound problem must never become a blank screen in
front of a congregation.

The two audio cards share one grid cell so the speaker picker stays
under the mic picker at every breakpoint; the 2-column grid otherwise
flows them side by side.

15 tests, including that applySink never reports a routing it did not
achieve (CLAUDE.md §15) and that the mic is always released.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>

---
## 6d3877c — 2026-07-19
Oluwawunmi

Merge pull request #4 from devgeereact/feat/audio-output-device

feat(audio): choose the speaker video sound plays on
---
## cbc0245 — 2026-07-19
Oluwawunmi

feat(detection): find the story when the preacher tells it in his own words

The premise of this product is that the preacher goes off-script. Nothing
measured whether that worked. eval_corpus.json holds 50 cases of which
exactly TWO are paraphrases, and both are near-verbatim quotes of famous
verses — so the 100% scorecard said nothing at all about paraphrase, the
one thing the product claims.

Adds data/paraphrase_corpus.json: 43 narrative retellings labelled with
the passage they came from, split three ways so the result is honest —
  kjv       reuses KJV words
  scattered KJV words pulled from verses that do NOT sit together
  modern    today's words for 1611 concepts

MEASURED FIRST, THEN BUILT. The plan was story-level (windowed) retrieval:
score whole stories, then narrow to the verse. The `scattered` bucket
exists to test exactly that, and flat per-verse search already scores
100% on it — because IDF is doing the job already. Biblical narrative
nouns ("Meribah", "husks", "singed") are so rare that ONE of them pins
the passage outright; there is nothing left for aggregation to add.
So windowing was not built: ~3x index memory and a two-stage search to
move a number that is already 100%. Benchmarked and deliberately not
used, like beam search in this same module.

The real gap was vocabulary, not aggregation. A natural retelling scored
18% at rank 1 — "the paralysed man lowered through the roof" returned
Joshua 2:8 — because not one content word appears in a 1611 text. That is
a glossary, not a neural network.

data/kjv_gloss.json is a hand-curated modern→KJV table, same shape as
book_aliases.json: offline, no new dependency, reviewable by anyone who
knows the text. Applied to the QUERY ONLY — glossing the corpus would
change the document frequencies that make rare nouns strong signals, and
a test pins that invariant.

  vocab       @1              @5
  kjv         95% →  95%     100% → 100%
  scattered  100% → 100%     100% → 100%
  modern      18% →  59%      53% →  88%
  TOTAL       65% →  81%      81% →  95%

No regression: the detection gate is unchanged (100% recall, 0.0%
wrong-verse, 0 paraphrase auto-fires). Semantic remains capped at Suggest
in the router — a cosine is still not a probability, and a better cosine
does not change that.

The modern cases were authored BEFORE the glossary existed so the set
stays held-out, and a test rejects story-specific keys ("samaritan",
"sycomore") so the gloss cannot name an answer.

This closes the cheap half of the gap. The rest is genuine synonymy and
still wants a sentence embedder behind the top_k seam.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>

---
## a9a33e5 — 2026-07-19
devgeereact

Enhance reference detection with fuzzy matching for book names

- Updated `match_book` function to return fuzzy match status alongside canonical book names.
- Introduced `fuzzy_book` function to repair misheard book names based on context.
- Added a list of common words that should never be repaired into book names.
- Modified `parse_reference` to account for fuzzy matches, adjusting confidence levels accordingly.
- Implemented `repair_query` to handle out-of-vocabulary tokens, allowing for better query matching.
- Enhanced `top_k_explained` to blend story and verse scores, improving relevance in search results.
- Added comprehensive tests for fuzzy matching and query repair to ensure robustness.

---
## 44bd9d5 — 2026-07-19
Oluwawunmi

feat(detection): stem the semantic index, and make its scores deterministic

Three changes, one measured, two found on the way.

1. STEMMING. tokenize() did none, so "pigs"/"pig", "flames"/"flame" and
   "physicians"/"physician" were unrelated words. Applied to BOTH index
   and query — it is a normalisation, and normalising one side only would
   stop them matching at all.

   Deliberately a standard Snowball stemmer (rust-stemmers, pure Rust, no
   native deps) rather than a hand-rolled suffix stripper: anything
   written here would have been tuned against our own benchmark until the
   number looked good, which measures nothing. An off-the-shelf algorithm
   cannot be fitted to our test.

   vocab       @1              @5
   kjv         95% → 100%     100% → 100%
   modern      59% →  53%      88% →  94%
   scattered  100% → 100%     100% → 100%
   TOTAL       81% →  81%      95% →  98%

   kjv reaches 100% and @5 rises to 98%; modern loses one case at rank 1.
   Kept because @5 is where the headroom went and the one remaining total
   miss is a synoptic parallel (Matthew 8:24 for a Mark 4 label) — the
   right story in the wrong gospel, which Relay has no concept of yet.

   The gloss shrank from 78 word-forms to 49 CONCEPTS with identical
   scores: the stemmer now covers the plurals and tenses that were being
   listed by hand. A table of concepts is reviewable; a table of word
   forms is a chore.

2. NON-DETERMINISTIC SCORES (a real bug, surfaced by a test that had been
   quietly fragile). `cosine` and the tf-idf L2 norm both summed over a
   HashMap. Iteration order varies per map instance and float addition is
   not associative, so the SAME query scored differently between two
   calls — 0.57233125 vs 0.5723312. SEMANTIC_FLOOR gates on that number,
   so a borderline paraphrase could be suggested on one run and dropped
   on the next from identical words. Live software has to be predictable.
   The query is now a sorted slice and the norm sums in sorted order
   (which also lowers rounding error). Pinned by a test that compares raw
   bits across 25 runs.

3. The explanation is DE-STEMMED before the operator sees it. Snowball
   turns "belly" into "belli"; rule #18 says the operator must be able to
   judge the claim, and nobody can judge "belli · husk". The index keeps a
   stem → most-common-surface-form map for display only.

Detection gate unchanged: 100% recall, 0.0% wrong-verse, 0 paraphrase
auto-fires. fmt + clippy clean, 285 tests.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>

---
## 2b58a45 — 2026-07-19
devgeereact

Add library components for browsing scripture, lyrics, and media

- Implemented Browse.svelte for scripture browsing with translation support and search functionality.
- Created ContentSlides.svelte for displaying announcements and media as slides.
- Added LiveStrip.svelte to show current content on screens during live sessions.
- Developed LyricSlides.svelte for managing and displaying song lyrics.
- Introduced SlideGrid.svelte for rendering slides in a grid format, enhancing visual recognition.

---
## 456456b — 2026-07-20
Oluwawunmi

feat(detection): offer the paraphrase alternatives instead of hiding them

Relay asked the index for ONE paraphrase candidate and threw the rest
away. Measured on the paraphrase corpus, the right passage is in the top
5 for 98% of retellings but ranked first for only 81% — and for a
retelling in modern words, 53%. The index had already found it; the
operator was simply never shown.

Chosen on evidence, not taste. eval::suggestion_policy_scorecard sweeps
the policy and prints what each setting costs the person reading it:

  keep within   ALL recall  avg shown | MODERN recall  avg shown
     100%           77%       0.88 |         41%       0.71   <- was
      60%           81%       2.56 |         53%       1.65   <- now
      50%           84%       2.88 |         59%       1.71

Two limits, because a longer list is not free — every row costs a
volunteer attention in a dark booth mid-service:

  * RELATIVE floor (0.60 of the best score), so the list widens only when
    Relay is genuinely torn and stays at ONE when a verse wins outright.
    A fixed top-N is the wrong shape: it pads the list exactly when the
    first answer was already correct.
  * hard CAP of 3. A well-quoted verse matches many verses strongly, and
    the sweep shows the extra rows land mostly in the kjv bucket, which
    is already 100% correct at rank 1 — pure noise.

Both are configuration, not constants.

Also recorded, because it is the more important finding: the LIMIT on
paraphrase recall is SEMANTIC_FLOOR, not the length of the list. 98% of
retellings have the right passage in the top 5, but only 84% survive the
0.30 cut. Lowering it would trade that back for noise, and the corpus has
no negative cases yet — transcript that mentions no scripture at all — so
that noise is UNMEASURED. The constant now says so, so nobody lowers it
on a hunch.

Filtering is extracted to `worth_suggesting` so it is testable at all,
with tests for the runaway winner, the near-tie, the sub-floor noise and
the empty case.

Semantic remains capped at Suggest in the router: more suggestions can
never mean more auto-fires. Detection gate unchanged (100% recall, 0.0%
wrong-verse, 0 paraphrase auto-fires). fmt + clippy clean, 290 tests.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>

---
## ef0d96d — 2026-07-22
devgeereact

fix(ui,db): harden error handling and kill three latent live-service faults

Smoke-test pass over the NEW-DESIGN branch. Every automated gate was already
green (329 frontend / 366 Rust / 0.0% wrong-verse); the gaps were runtime
faults and error-handling drift a test suite doesn't catch.

HIGH
- Live: mark a plan cue live only AFTER the fire resolves. Setting onAir before
  the await left a failed fire showing amber "On Air" on a cue that never
  reached the wall — and the reactive setSession persisted the lie. Amber may
  never claim a screen it did not reach (rule 18).
- Library: the take()/fireQueued live-fire path swallowed rejections (try/finally,
  no catch) — a dead Fire button with no feedback. Now catches and surfaces;
  staged slide is kept on failure so the operator can retry.
- ServicePlanner: route every backend mutation (~18 handlers) through one act()
  wrapper. Rejected invoke()s were unhandled — silently dead buttons. Errors are
  humanised and rendered rose, never the success-green slot.
- Settings/Library: raw String(e) shown to the operator, some styled green;
  unguarded onMount chains aborted init silently. Humanised; guarded.
- db: create app_settings BEFORE ensure_lyrics_template writes to it. A
  pre-app_settings v0 DB hit "no such table", propagated out of migrate, and
  panicked at every boot forever (rule 25 class). Adds a regression test proven
  to fail without the reorder.
- channels: reject ".." on the debug LAN disk-serve — it binds 0.0.0.0 and
  streamed any readable file to the church network on dev/unsigned builds.

MED / polish
- Live: transcript arrival times stamped in the store (finalsAt), trimmed in
  lockstep with finals — the length-based view logic froze after MAX_FINALS and
  mislabelled every line.
- Live: related-scripture debounce depends only on relatedWindow; audio churn no
  longer starves the 1.5s timer so the feature actually fires.
- App: sidebar AI-signal dot emerald, not a permanent amber (tally-light colour).
- Settings: replace native confirm() (webview ignores it) with in-app two-step
  arm/confirm; aria-pressed on segmented controls; guard pickTranslation.
- Channels/Planner: real Loading states so a cold open no longer flashes the
  empty state; guard Channels onMount.
- Library: aria-haspopup/expanded + role=menu on the New/More menus.
- Browse: safe-mode Fire explains why instead of a silent no-op.

Not committed here: the LAN unauthenticated fire/clear/black is a product
decision (DECISIONS §47), not a bug.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 4347004 — 2026-07-22
devgeereact

fix(ui): clear the low-severity polish from the smoke-test review

- capture: reset the `meter` store on Stop. capture.level/isVoice (which nothing
  reads) were being cleared instead, so the input-level bars stayed frozen lit
  at the last value after listening stopped.
- Browse: auto-clear msg/error (4s / 6s) so a "Saved …" or an error fades instead
  of lingering across later navigation as if it still describes the screen.
- Channels: log the QR-generation and clipboard failures instead of swallowing
  them in empty catches — the URL is on screen regardless, but a dead-looking
  button should not be invisible to logs.
- Settings: distinguish "loading translations" from a genuinely empty list so the
  translation panel does not flash "No translations loaded" on a cold open.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 5642cd7 — 2026-07-22
devgeereact

fix(ui): surface Listen failures; make Library menus keyboard-dismissable

- Live: toggleListen no longer swallows a command-level rejection. Capture start
  is non-blocking (rule 5), so DEVICE errors arrive on audio://error — a rejection
  HERE is a start_service/start_capture failure that event never carries, so the
  Listen button was silently doing nothing. Now flashed through humanError.

- Library: the New/More dropdowns are now keyboard-operable — trapFocus moves
  focus into the menu on open and restores it to the trigger on close, and Escape
  closes the menu. Crucially the Escape handler stopPropagations so it never
  reaches the global panic handler on window: dismissing a menu must not clear the
  congregation's screens (rule 16). role=menu/menuitem + tabindex for AT.

Deferred deliberately: a light/system theme repaint is a real feature, not a bug —
the control is already labelled "Only Dark is styled today", so it is honest, not
silent.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 31e9c4a — 2026-07-22
devgeereact

feat(ui,core): land the new-design overhaul (source)

The bulk of the NEW-DESIGN branch, committed as one coherent source drop — the
whole tree builds and passes (329 frontend / 367 Rust) at this state.

Frontend
- New shared primitives and helpers: backgrounds, layers, reflow, passage,
  queue, templateKind, plus their tests.
- Stage Displays: StageDisplays view + stagedisplays/ editor & gallery and the
  stagedisplays store.
- Template editor/gallery split into views/templates/{TemplateEditor,
  TemplateGallery}; Templates.svelte reduced to the host.
- Library refactor: retire the per-type panes (ContentSlides, LiveStrip,
  LyricSlides, Lyrics, Media, SlideGrid, SongEditor) in favour of the unified
  Browse / LyricsPane / MediaLibrary / VerseDeck / PreviewProgram / LiveOutputRail.
- Output/Stage/app.css/TemplateRender updated for the new render + template model;
  bundled template backgrounds under src/backgrounds (full-res originals ignored).

Core (Rust)
- main.rs, db/{channels,plans,templates}, detection, pipeline, proimport,
  telemetry updated for the above; examples/displays.rs; e2e coverage extended.
- Cargo deps updated.

This is the in-progress design work that the earlier fix commits (ef0d96d,
4347004, 5642cd7) were layered on top of; landing it so a clean checkout builds.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 0e42182 — 2026-07-22
devgeereact

docs: fold in the new-design decisions, screen notes and design-loop logs

- CLAUDE.md / DECISIONS / PRIVACY / relayscreens updated for the new-design work
  (incl. the §47 LAN-remote note and the design-loop prompt).
- Per-surface design-loop logs (channels/library/planner/settings/stagedisplays/
  templates) captured as markdown.
- .gitignore: ignore backgrounds-originals/ (full-res source art kept out of git;
  the optimized set ships under src/backgrounds).
- Remove the stale relay-templetedesigner-screen.png.

Design-loop screenshots (.loop/*.png) are committed separately.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 978327f — 2026-07-22
devgeereact

docs(design): add the design-loop reference screenshots

The per-surface reference renders and iteration snapshots from the design loop
(channels/library/planner/settings/stagedisplays/templates + baselines). ~30 MB
of PNGs, isolated in their own commit so they can be dropped from history later
without touching source or the markdown logs if repo size becomes a concern.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 5972e0d — 2026-07-25
devgeereact

feat(themes,outputs,perf): themes engine, per-screen templates, role monitors, instant fire

Presentation-suite build-out on the TEMPLETE branch, plus a run of live-tested fixes.

Themes (DECISIONS §27)
- Theme = a style layer BENEATH templates: default `style` keys a template overrides
  per key; `TemplateRender` resolves `style.themeRef` against builtins itself, so every
  surface themes with no per-surface wiring. Layer colours bind to tokens (`theme:accent`).
- Themes tab (gallery + editor), 8 builtins, custom CRUD persisted in the settings KV,
  theme/template export-import (marker-gated, shape-not-identity), custom themes shipped
  to kiosk/OBS over the WS hub. `applyThemeToTemplate` re-tokenises a template to a theme.

Role-output monitors
- Stage / Confidence / Preacher / Countdown presets as render-profiles of the ONE engine
  (starters in layers.js), not a parallel system. Monitor-only fields ride to output but no
  congregation template renders: next-verse (passage-bounded), operator note, elapsed +
  remaining service timers.

Output template model (DECISIONS §29 — reverses §25)
- The SCREEN'S OWN template wins; a content-type default DEFERS to it; a cue's deliberate
  choice is `template_pinned` and overrides. One resolver, `resolveOutputTemplate`, shared
  by the wall, the console program pane and the Outputs inspector preview.
- Single Default template (settings KV) replaces the 4-star `console_active` cap — any
  template to any screen.
- Output URL is channel-keyed: changing a screen's template live-swaps its outputs with no
  URL change (native event + kiosk `channel_template` broadcast, filtered per channel).

Performance
- A content-look default rides as an ID only — never serialises/broadcasts its template
  JSON (a default template with an embedded `data:` image was 13 MB, making every fire take
  seconds). Only a pinned cue template ships JSON.
- No slide transition — instant cut (also fixes a fit/overflow bug from measuring mid-crossfade).
- Auto-fit only runs on-screen (IntersectionObserver); re-fit on fonts.ready gated to the
  still-loading first view. Bible defaults to the plain-text list, whole chapter in one scroll.

UX
- Dashboard moved into Settings (no longer a sidebar tab). Settings gains Integrations +
  Diagnostics; Themes tab added (fixed a missing nav icon that printed "undefined").
- Network outputs read LIVE when serving (viewer count in the detail line), not only when a
  browser is connected. Verbatim text — no auto-added quotation marks. Template version
  history (bounded, deduped) in the editor.

Docs: DECISIONS §27–29, USER_GUIDE, CLAUDE.md updated. 374 Rust + 384 frontend tests, clippy clean.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>

---
## 4024838 — 2026-07-27
devgeereact

feat(router): implement per-reference cooldown for auto-fire decisions

- Changed `last_fire` to a `HashMap` to track when each reference last reached the screen, allowing for independent cooldowns.
- Updated `decide` method to utilize the new structure, ensuring that alternating verses do not erase each other's cooldowns.
- Added tests to verify that the new cooldown mechanism works correctly with alternating verses and that it does not leak memory over time.

feat(stt): introduce language stability check for auto-detect

- Added `LanguageStability` struct to monitor the detected languages over recent windows.
- Implemented logic to report instability when multiple distinct languages are detected, helping operators identify when to manually select a language.
- Updated `is_hallucination` function to include checks for stuck decodes, improving detection accuracy.

feat(capture): manage stale suggestions in the capture store

- Introduced `SUGGESTION_TTL_MS` to define how long pending suggestions remain actionable.
- Implemented `pruneStaleSuggestions` function to filter out stale suggestions based on their timestamps.
- Updated the capture store to expire stale suggestions during the capture process and when new suggestions arrive.

fix(live): notify operator of unstable language detection

- Added a warning in the Live view to inform operators when the auto-detect language is unstable, guiding them to manually select a language.
- Ensured that the warning is displayed only when necessary, improving the user experience during live sessions.

test(suggestions): add unit tests for suggestion expiration logic

- Created tests to validate the behavior of the `pruneStaleSuggestions` function, ensuring it correctly handles the expiration of suggestions based on their timestamps.

---
## ce01d83 — 2026-07-29
Gideon Akinlotan

Merge pull request #5 from devgeereact/feat/transcript-improvements

Feat/transcript improvements
---
## 1c61b69 — 2026-07-29
Gideon Akinlotan

Merge branch 'NEW-DESIGN-IMPLEMEMTATION' into main

Lands the new-design overhaul on top of the detection work from #5. Five files
conflicted; none of them were a pick-a-side, because both branches had evolved
the same code in complementary directions.

- detection.rs — main's stemming + KJV gloss + `surface` and the design branch's
  story/passage windows + `repair_query` are now ONE pipeline:
  tokenize -> stem -> repair -> gloss. The order matters: `repair_query` and the
  gloss both look words up in the INDEX's vocabulary, and the index is stemmed,
  so they can only be asked about stems. Story scoring uses the sorted query
  vector, keeping the determinism fix (`cosine` over a sorted slice, not a
  HashMap) that #5 added for exactly this reason.
- TemplateRender.svelte / Settings.svelte — the design branch rewrote both
  wholesale, so its version is the base and the audio-output feature (speaker
  choice for video sound) was re-applied onto it: all three <video> elements are
  routed, and the Audio Output card now lives in the new Settings rail.
- Output.svelte — band-aware blackout, plus the `audio` prop.
- main.rs — two independently-added test modules, both kept.

DECISIONS.md §25 records the one real product disagreement: the design branch
refused any one-word paraphrase, and the gloss feature's whole premise is that
a modern retelling reaches its verse through exactly one rare KJV noun
("pigs" -> "swine"). Neither is wrong about its own case, so the bar is now
evidence rather than arithmetic — two shared words, OR one rare enough to be
evidence alone (<=0.1% of the corpus). Still capped at Suggest, so this changes
what a human is offered and never what reaches a congregation.

Verified: fmt + clippy clean, 380 Rust tests, 344 frontend tests, vite build
clean. Detection scorecard unchanged — 50/50 cases, 100% recall, 0 wrong verses,
0 paraphrases auto-fired. The two new evidence tests are mutation-verified:
removing the rarity exception fails one, treating every term as rare fails the
other.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 9c11b65 — 2026-07-29
Gideon Akinlotan

fix(boot): the heartbeat counts console mounts again — probes call ping

`console: webview up` printed THREE times per launch. It was not a triple
boot: the console mounts exactly once. `greet` had picked up two more callers.

`probes.js:engine()` (new with the design merge) called `greet` to ask "is the
bridge attached?", and that probe runs from BOTH the launch sequence and the
Dashboard health panel. One real mount + two liveness probes = three lines.

That matters because `greet` is not a health check, it is a COUNTER. On a
machine that cannot screenshot the app it is the only proof the webview loaded
and reached the Tauri bridge (CLAUDE.md), and its whole value is that one line
means one mount. Nothing was broken — which is exactly the problem: the one
instrument for diagnosing a blank console now read identically to a webview
reloading twice, so a real double-mount would have been invisible in the noise.

- `ping` — new, silent, returns true. What anything polling liveness should call.
- `probes.js:engine()` uses it; `greet` is back to a single caller.
- `ipc.test.js` fails if any file other than App.svelte mentions `greet`.
  Mutation-verified: putting `greet` back in probes.js fails it by name.

Also folds in doc drift found by the same smoke test:
- test counts 330/247 -> 380/345 (measured)
- `related_scripture` is no longer "dead-but-built, zero frontend callers" —
  the design merge wired it into Live.svelte via `relatedScripture()`
- CLAUDE.md §26 records the rule

Verified: fmt + clippy clean, 380 Rust, 345 frontend, and the packaged binary
prints exactly one heartbeat on 3/3 clean launches.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 90c9967 — 2026-07-30
devgeereact

Merge remote-tracking branch 'origin/main' into TEMPLETE

# Conflicts:
#	CLAUDE.md
#	docs/DECISIONS.md
#	src-tauri/src/detection.rs
#	src-tauri/src/eval.rs
#	src/Output.svelte
#	src/lib/TemplateRender.svelte
#	src/lib/views/Settings.svelte

---
## 75a416c — 2026-07-31
Gideon Akinlotan

build(macos): make the §17 microphone trap testable without a certificate

Gatekeeper acceptance is NOT what this changes, and cannot be: that needs a
Developer ID Application certificate and notarization by Apple. The release
workflow already does all of it correctly (per-platform gate, cert import,
notarization creds via $GITHUB_ENV to dodge the empty-env-var trap), and
docs/RELEASING.md §2 already documents the setup. The only thing missing is the
certificate itself — none of the six APPLE_* secrets is configured, so a real
tag fails the gate loudly, exactly as intended.

What WAS missing is the ability to test rule §17 before owning one. Notarization
requires the hardened runtime, and under it a process opening an audio input
without `com.apple.security.device.audio-input` is TCC-killed rather than
refused. RELEASING.md claimed "no build you can make locally would have shown
it". That is not true: the hardened runtime is a flag on the signature, not a
property of who signed it, and `codesign --options runtime` sets it for an
ad-hoc signature exactly as for a Developer ID one.

`scripts/sign-local.sh` re-signs a local bundle ad-hoc WITH the hardened runtime
and the real entitlements, then asserts the three things that actually differ
from `tauri build`: hardened runtime on, mic entitlement embedded, usage string
present in the bundled Info.plist. It exits non-zero on any of them, so it
cannot pass vacuously — mutation-verified both ways (stripping the entitlement
and stripping the usage string each fail it by name).

Side effect worth having: `tauri build` unsigned produces a bundle with no
_CodeSignature at all, so `codesign --verify` reports the confusing "code has no
resources but signature indicates they must be present". After this it reports a
valid signature, and Gatekeeper gives an honest "rejected" instead.

Docs corrected: RELEASING.md's "no local build would have shown it" row, and
CLAUDE.md §17 now points at the script.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## ad23b19 — 2026-07-31
Gideon Akinlotan

Merge branch 'TEMPLETE' into main

The last unmerged branch. Brings the themes engine, per-screen templates, role
monitors and instant fire, plus a per-reference cooldown on auto-fire decisions
in the router.

Merged clean — no conflicts. TEMPLETE had already merged main up to 9c11b65, so
the only divergence was 75a416c (the local-signing script), which does not touch
anything it changes.

Verified on the merged tree: fmt + clippy clean, 408 Rust tests, 404 frontend
tests, vite build clean, versions consistent.

The router change was the one to watch, since rule #10 says only
DetectionMethod::Direct may ever auto-fire and a cooldown edits exactly that
decision path. The CI-gated scorecard is unchanged: 50/50 cases, 100% recall,
0 wrong verses, and 0 paraphrases auto-fired.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 94fa3d7 — 2026-07-31
Gideon Akinlotan

feat: close the dead-but-built gap — 5 deleted, 8 given their UI

Thirteen registered `#[tauri::command]`s had no frontend caller at all. Every one
of the 113 that remain is now reachable from the console.

DELETED — superseded, not missing:
- `lookup_verse`      -> the console uses `chapter_verses` + the manual fire path
- `close_output_window` -> `close_channel_output`; windows are addressed per channel
- `current_service`   -> the frontend tracks this in the session store
- `list_active_templates` / `set_template_active` -> the "console Output grid, max
  4" concept became per-channel templates. Their db helpers and the feature test
  went too. `ensure_template_active` STAYS: `console_active` is on every installed
  database already, and a migration that stops running is a schema that forks.
  Its idempotence test now counts with SQL instead of the deleted helper.

WIRED — spec'd features that only ever lacked a UI:
- Voice profiles (SPEC §4.6), a new Settings section: list, add, use, edit, delete,
  with language and decoder-bias vocabulary. The form edits the SENSITIVITY DIAL
  and only displays the learned auto_fire/suggest, because those are what the
  router worked out from the operator's confirmations — moving the dial
  re-baselines them (`thresholds_on_profile_save`) and every other edit must
  preserve them. Conflating the two once wiped calibration on every save.
- `push_announcement` — the emergency message, over live scripture on every
  channel. Armed in two steps like the countdown: a stray Enter must not be able
  to interrupt every screen in the building.
- `verse_repeat_count` — "shown earlier" on the PREVIEW pane, not the on-air pane;
  by the time it is on air the repeat has already happened. Slate, never amber.

Each new wrapper was placed in capture.js's throw-vs-swallow contract
deliberately, and `announce.test.js` pins the choice: the announcement THROWS
(a silent failure tells the operator the room was warned when it was not), the
repeat badge SWALLOWS to 0, profile writes throw, profile reads swallow.
Mutation-verified — making the announcement swallow fails the suite.

Verified: fmt + clippy clean, 407 Rust, 413 frontend, vite build clean, and the
packaged binary boots with one heartbeat.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 86fbc9c — 2026-07-31
Gideon Akinlotan

fix(nav): a tab that MOVED sends the operator where it went, not to Live

Started as cosmetic — a leftover `stagedisplays` icon and locale key — and turned
out to be a real, silent regression sitting behind them.

`activeTab` is persisted, so it outlives the layout it was written under. The
redirect for a relocated surface lived inline in App.svelte as a one-key ternary:

    activeTab === 'stagedisplays' ? 'channels' : activeTab

but `dashboard` and `history` had ALSO stopped being top-level tabs — both became
sections INSIDE Settings. Neither was in the ternary, so `tabs.some(...)` called
them unknown and dropped the operator on Live. Not a blank screen, which is why
nobody noticed: they just never arrived at the thing they asked for. The comment
above the ternary states the intent exactly — "sent to Outputs, not dumped back
on Live" — and two thirds of it had quietly stopped being true.

Nothing could catch it. App.svelte is not unit-testable and the map was not a
value, so staleness was unobservable. Both are fixed:

- `MOVED_TABS` + `resolveActiveTab()` in session.js, pure and exported.
- Four tests, including one asserting every redirect TARGET is itself a real tab
  — a Settings rename would otherwise turn all three redirects into a silent
  bounce back to Live, which is this same bug one level up.
- Mutation-verified: restoring the stagedisplays-only map fails, and pointing a
  redirect at a non-existent tab fails.

Also cleared what led here:
- three orphaned tab icons (dashboard, history, stagedisplays) — `icons` is keyed
  by tab and none of them is one
- their three `tab.*` locale keys, which existed only in en.json and so inflated
  the denominator every other language's coverage is measured against
- session.js claimed twice that a fresh install lands on the Dashboard. The value
  has been 'live' since the Dashboard stopped being a tab; only the comments went
  stale, which is the more dangerous half — they read as intent, and the next
  person to honour them would reintroduce a tab that no longer exists.

Verified: 417 frontend tests, vite build clean, packaged binary boots with one
heartbeat.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 38eeb46 — 2026-08-03
Gideon Akinlotan

fix(detect): a reference cut off mid-sentence is no longer a reference

The STT window is re-decoded about once a second and detection runs on every
partial, so every citation is parsed at least once before its verse number has
arrived. That manufactured wrong verses and put them on the wall:

    t+0s  "…John chapter 3 verse"     → John 3:1   0.88 → ON THE WALL
    t+1s  "…John chapter 3 verse 16"  → John 3:16  0.95 → ON THE WALL

Whether it happened depended only on where the window boundary landed, so it was
not an edge case — it was a coin toss on every citation of the commonest form in
English preaching. Two defects (DECISIONS §34):

1. A dangling verse marker was PROMOTING the mistake. The parser consumes
   verse/verses/vs/v/: and sets the keyword bonus; when the number then failed to
   arrive it fell through to a whole-chapter reading carrying that bonus. Bare
   "Romans 8" scores 0.45 and asks a human — "Romans 8 verse" scored 0.88 and went
   straight to the screen. The most truncated reading outranked the honest one.
   Now the parse fails: the grammar committed to a verse number.

2. A whole chapter at the end of a partial is provisional. RefMatch::is_provisional
   suppresses only a whole-chapter reading, only with nothing after it, only while
   the text can still grow. A complete "John 3:16" at the tail still fires
   instantly, so this costs no latency on the path that matters.

is_provisional is called by both emit_detections and the bench, so the benchmark
cannot score a policy the live path does not run. emit_detections now takes
is_final; it previously could not tell a closed utterance from a growing one.

Measured through the real pipeline: wrong verses 4→2 (base) and 5→2 (small), recall
unchanged, eval.rs still 100% recall / 0 wrong verses in all four languages. The
survivors are mishearings of the number under extreme attenuation — a decoder
question, measured not patched (rule 13).

Also, because none of the above was findable without a ruler:

- stt::bench::engine_shootout — drives the REAL SttEngine through sender() and
  scores through the REAL Router, so "wrong verse" means reached the wall. Feeds in
  real time: pushing the clip in at once collapses it to one final window (the
  batch drain doing its job), which made every model score identically and is what
  hid this bug at first.
- Deoverlap — rule #8 extracted from the worker loop into a tested type. It had no
  direct test.
- Metal enabled for the macOS release AND in CI. Every shipped build was CPU-only:
  the feature was wired up and never turned on by anything producing an artifact.
- Larger models (small, large-v3-turbo, +q5_0) with a machine-aware caution, and the
  model choice is now persisted — resolution used MODEL_CANDIDATES order alone, so
  downloading a 1.6 GB model changed nothing. Settings only rendered the model UI
  when nothing was loaded, so a second model could never be seen or picked.
- ipc.test.js event regex used [a-z]+, excluding underscores, so
  stt://language_unstable and output://panic_failed — a panic path — were silently
  outside the contract.

428 Rust + 418 frontend tests, fmt and clippy clean on both feature sets. Every new
test mutation-verified: it fails when the defect is reintroduced, and the narrowness
guards fail if is_provisional is widened or made to ignore is_final.

Verified on a signed hardened-runtime build (scripts/sign-local.sh, §17) installed
to /Applications and exercised by hand.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 95556a1 — 2026-08-03
Gideon Akinlotan

fix: address CodeRabbit review — two real bugs, five sharpenings

The guard from the previous commit was incomplete, and one test proved nothing.

**The single-chapter path was missed entirely.** `parse_reference` has TWO branches
that consume verse markers. The guard only covered the general chapter path, so
"jude chapter 1 verse" still answered Jude 1:1 at 0.95 — a HIGHER score than the
0.88 defect that had just been fixed, on the five books whose names are ordinary
English words (Jude, Philemon, Obadiah, 2 John, 3 John). Guard applied to both;
the truncation test now exercises both.

**Deoverlap's zero-rate handling was worse than the crash it avoided.** Clamping
the sample rate to 1 avoids a divide-by-zero and then silently computes the chunk's
duration as one sample per second: 128 samples advanced the mark by 128 SECONDS,
after which every real chunk read as already-covered and Relay went deaf for the
rest of the service, with a transcript that simply stopped. Without a rate there is
no time and no overlap to compute, so the audio passes through and the mark is left
alone. The test asserted only "does not panic" — it now asserts the stream still
works afterwards, and fails if the clamp comes back.

Also:

- models.rs: the model-ordering test was a tautology. `position()` returns the FIRST
  heavy index, so every element before it is non-heavy by construction and the loop
  could not fail — moving a 1.6 GB model to index 1 would have passed. Now compares
  every pair; verified it fails on exactly that edit.
- ci.yml: carry MACOSX_DEPLOYMENT_TARGET=11.0 from release.yml. A CI job compiling
  against a different deployment target than the one shipped is not testing the
  build that ships — the failure mode the step exists to catch.
- ModelSetup: `selectModel` can resolve false (chosen, then would not load) without
  throwing, so the badge vanished with no explanation. Rule #15.
- stt.rs: RELAY_BENCH_SPEED > 1 now actually prints the warning its comment promised.
- bench/README: "all four" → "all five" after the list grew.
- DECISIONS §34: said the guard was total when it covered one of two branches.

428 Rust + 418 frontend, fmt and clippy clean. Scorecard still 100% recall / 0 wrong
verses; shootout unchanged at 2 wrong verses on both models.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## d689957 — 2026-08-03
Gideon Akinlotan

Merge pull request #6: a reference cut off mid-sentence is no longer a reference

A reference cut off mid-sentence is no longer a reference
---
## 2cdc9b5 — 2026-08-11
Gideon Akinlotan

fix: three things that were quietly lying to the operator

Nothing here was red. Every test passed, clippy was clean, and each of these
had a comment nearby asserting the opposite of what the code did.

**Rehearsal leaked the "up next" preview to a live stage monitor.**
`channels.rs` documents rehearsal as gated "in the one function content leaves
the machine through", and it was four functions, one of which was not gated.
`stage_next` is the only content publisher with no Tauri emit — a stage monitor
is always a network client — so it went straight to the kiosk hub. An operator
rehearsing on the real desk pushed the real upcoming verse to whatever tablet
was still connected from the last service, while the congregation wall stayed
correctly frozen. The sandbox looked intact, which is the worse failure.

It survived because `e2e.rs`'s `Wall` listens for Tauri events, so the test
guarding the rehearsal guarantee could not see the door it went out of. The new
e2e case subscribes to the kiosk hub itself and asserts the live path first, so
it cannot pass by the publish being broken outright. Verified by reintroducing
the bug: it fails.

**`stopCapture` swallowed a stop that did not happen.** `capture.js`'s own
header puts it in the THROWS group ("changes whether the microphone is live"),
and it wrapped both the bridge import and the command in one bare catch. The
comment excused the plain-browser case; the catch also ate a real failure, and
`stop_capture` takes a lock, so a panicked audio thread poisons it and leaves
the engine running. The console then detached its listeners and printed "Start
listening" over a live microphone with detection still auto-firing. Five callers
had `catch (e) { humanError(e) }` that could never fire.

Now a failed stop rethrows and leaves the UI reading LIVE, because it is. The
one caller with no handler (Settings) got one; FirstRun still catches — it runs
from onDestroy — but shows the error instead of assuming "already stopped".

**Settings told operators to build an OBS URL that cannot live-swap.** The
integrations panel printed `output.html?template_id=<n>`, which parses to
channel 0. It renders, so it looks right, and then it is the one browser source
in the building that ignores every template change. README and USER_GUIDE said
the same. All three now show the channel-keyed form.

Also fixed
- `npm test` failed on Node >= 22 (60 tests), passing only on CI's Node 20. Node
  defines Web Storage as globals that return undefined without a flag, and
  vitest's jsdom environment leaves keys the global already owns. A contributor
  on a current Node saw a red suite on a clean checkout with nothing explaining
  it. `src/test-setup.js` installs a real jsdom Storage when the ambient one is
  missing; no-ops on Node 20.
- The documented pre-release procedure could not be run. `version.mjs` requires
  a numeric pre-release identifier (the MSI bundler rejects names), while
  release.yml and RELEASING.md instructed `v0.2.0-rc1` throughout — so step one,
  `npm run version:set -- 0.2.0-rc1`, fails immediately.
- `version.mjs --check` did not enforce the MSI's 65535 ceiling that `--set`
  did, so a hand-edited version could clear the release gate and die in the
  Windows bundler fifteen minutes later. One validator now serves both.
- A leaked announcement arm timer in Live's onDestroy, next to the countdown
  timer that was already cleared.
- CLAUDE.md drift: test counts, command count, main.rs size, a cmake PATH for a
  machine this is not, and two "largest remaining gaps" that had both shipped.

Verified: 429 Rust + 421 frontend, fmt, clippy -D warnings, vite build, version
check, detection scorecard (0.0% wrong-verse, 0 paraphrases auto-fired), on both
Node 20 and Node 26. Both new tests were checked to fail when their bug is put
back.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 7aa34d3 — 2026-08-11
Gideon Akinlotan

chore: ignore the local agent-tooling directory

`.claude/` is 244 files and 2 MB of claude-flow scaffolding — helpers, generated
commands, per-machine settings. None of it is Relay, and committing it would
land the lot in an open pull request.

Ignored alongside the three tool-state directories already listed. Reverse by
deleting the line if any of it should actually be tracked.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 6196d9c — 2026-08-11
Gideon Akinlotan

fix(remote): the preacher's phone says WHY nothing moved

Found by driving the packaged app at 180 fires/sec — 40 taps on a six-verse
psalm produced 10 frames, and every single tap had answered `{"ok":true}`.

`NavResult` exists because `nav` used to return `()`: an operator pressed Next
mid-sermon, the wall did not change, and there was no error, no toast and
nothing in any log. That was repaired for the console and left standing on the
preacher's remote — `remote_api` matched `Ok(_)` and threw the outcome away.

So the end of a reading, a verse missing from the library, and a successful step
were indistinguishable over HTTP. `Stage.svelte`'s only handler was a `catch`,
which fires on a transport error and never on this, so the preacher tapped Next
at the end of a reading and got silence. The original bug, one surface along.

The outcome now rides on the response (`NavResult` already derived `Serialize`)
and the remote names it: end of the reading, nothing on screen yet, or not in
the library. Only `fired` moved the wall.

Third instance of one pattern this week — rehearsal gated three of four kiosk
publishers, the throw-vs-swallow contract held for eight of nine group-1
wrappers, and now this. CLAUDE.md says so, next to the nav note.

New e2e case asserts the remote names a boundary rather than merely reporting
ok, and that the wall does not move when it has nowhere to move to. Verified by
restoring `Ok(_)`: it fails.

Verified: 430 Rust + 421 frontend, fmt, clippy -D warnings, vite build. Then
rebuilt, re-signed and re-installed the app and re-ran the full realtime smoke
test against it — all pass, 40/40 steps now name their outcome (fired,
end_of_passage, not_in_library) and the wall never moves on a boundary.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## cdb1ca2 — 2026-08-16
Gideon Akinlotan

docs(qa): the audit apparatus, and three shipped documents that were untrue

Adds a QA apparatus Relay can re-run, and corrects the docs the first run
found lying.

## The apparatus

`/qa-audit` runs six agents (`.claude/agents/relay-qa-*.md`), each holding a
different INSTRUMENT rather than a different topic — the distinction matters
because Relay is a Tauri binary with no browser, no clickable surface and no
screenshot on the build machine. "Click every button" cannot be executed here,
and an agent told to do it will read the source, form an impression, and file
PASS rows it never observed. So every claim names the layer that produced it,
and "no instrument reaches this" is a printed BLOCKED outcome, not a gap.

`scripts/qa-inventory.mjs` is the layer-C instrument: controls, orphaned
components, the command map, and the create-path chain from an INSERT to a
control a person can actually reach. `scripts/install-claude-hooks.mjs`
registers the fast-gate hook per machine (idempotent; refuses to write over a
settings file it cannot parse) because `.claude/settings.json` is one
developer's claude-flow wiring and does not belong in this repo.

Design, mandates and the evidence baseline: docs/Working-Agent*.md.
The first run's report, with a fix log: docs/audits/QA-2026-08-14.md.

## The documents

PRIVACY.md told churches that people on their network "**cannot** push content
to your screens — the connection is broadcast-only". SECURITY.md said the same.
Both were true when written and stopped being true when the preacher's phone
remote shipped: `remote_api` serves unauthenticated fire/next/prev/clear/black
on 0.0.0.0:8032, with a touch UI at the well-known /stage.html.

The code cited "DECISIONS §47" in three places. This file's sections end at §34
— 47 was the LINE NUMBER of a table row, and the citation is what made the
claim look checked.

DECISIONS §35 now records the decision that was actually made: the HTTP API is
an unauthenticated control plane on a trusted LAN, deliberately, because a
password on a device shared between a preacher, a tech volunteer and a stand-in
every Sunday is a password on a sticky note. It separates the two guarantees the
old text conflated — the WebSocket hub really is still broadcast-only — records
the drive-by that composition allows (side-effecting GETs plus wildcard CORS
means an <img> tag on any web page can black out the wall), and names the cheap
fix if that is judged unacceptable.

PRIVACY.md carries a dated note saying it previously said the opposite, because
a church may have made a network decision on the strength of that sentence.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## a1a00cb — 2026-08-16
Gideon Akinlotan

fix(engine): ordinary church speech no longer auto-fires the wrong verse

The P0 from the 2026-08-14 audit, plus the Rust half of its P1s. Measured
through the real router at the shipped default, before:

  "please turn to hymn number three sixteen" -> Numbers 3:16  @0.840
  "the youth meet in room two twelve"        -> Romans 2:12   @0.840
  "van/day three sixteen"                    -> Daniel 3:16   @0.840

Nobody pressed anything. A 118-noun sweep found 37 such phrases. Root cause,
two parts: `fuzzy_book` repairs a misheard token against the table built for
TYPING shortcuts (`num`, `rom`, `dan`), and one edit from a three-letter alias
is an enormous set of everyday English words — guarded only by "the next token
is a number", which is exactly the shape of an announcement. The documented
mitigation ("marked FUZZY, costs confidence downstream") was worth 0.06 against
a 0.34 margin. A contract stated in a comment is not a contract.

## The repair is structural, not a threshold (CLAUDE.md rule 10)

`DetectionMethod::UncertainBook` joins Semantic and Ambiguous on the wrong side
of `may_auto_fire()`, so no score, no sensitivity-dial position and no
calibrator drift can undo it. Two routes reach it, governed differently because
they are different facts: an edit-distance repair is never rescued (a guess
about the acoustics), while an ordinary word that is also a one-token alias
(`song`, `job`) is rescued by an explicit chapter/verse keyword. `psalm` is
deliberately excluded — R6's sweep flagged four TRUE positives there, and
"Psalm three sixteen" must keep firing.

## The cap did nothing until the instrument existed

It went green at the router while the product still put Numbers 3:16 on the
wall, because THREE places hardcoded `DetectionMethod::Direct` in place of the
real one: `emit_detections` (production), `eval.rs`'s scorer, and the detection
harness. Between them they hid the P0 and made its first repair inert. Caught
by the first e2e test of the auto-fire path — `emit_detections` and
`confirm_detection` are now generic over `R: Runtime` per architecture rule 24,
so the two paths where the AI decides can be driven at last.

## Also fixed

- `confirm_detection` returned Ok on two paths that put nothing on a screen —
  the parse falling through, and `fire_manual`'s bool DISCARDED with no binding.
  Reachable: a garbled "Psalms 23:99" is emitted as a suggestion with
  `in_library: false`, no frontend reads that flag, and the console flashed
  "Now live: Psalms 23:99" over the verse still on the wall.
- `/api/live` answered from the passage ANCHOR, which survives a clear by
  design, so the preacher's phone named a verse over cleared screens, blacked-out
  screens, and during a rehearsal — byte-identically to a real fire. Containment
  held on all four PUSH doors; this is a PULL, which is why every enumeration
  missed it. New `channels::WallState`, maintained at the three choke points and
  nowhere else; the anchor still rides as `cued`.
- `telemetry::scrub` was documented as an allow-list and implemented as a
  blocklist, so `logentry`, `tags` and `threads[].frames[].vars` survived — the
  same carrier the exception path clears, on the other stacktrace field. Now
  destructures and rebuilds, so a field added by a future SDK arrives empty.
- Spoken in-passage nav now speaks Swahili and Hausa: `detect_bare_verses` and
  `detect_passage_nav` matched English literals inline while `parse_reference`
  two functions away asked the multilingual helpers. "mstari wa nne" did nothing
  after a reference that had parsed perfectly.

## The instrument, so the next one is caught

`qa.rs` is THE fixture — a genuine first launch, plus `Wall` (Tauri events) and
`Kiosk` (the WebSocket door, which a Wall is blind to). `e2e.rs` and `r6.rs` now
share it; `r6.rs` had hand-rolled a copy that was out of date within a day.
`eval_corpus.json` gains five English negatives and TWO controls, so a future
over-correction fails the build as loudly as the original defect.

Rust 476/476. fmt and clippy -D warnings clean.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---
## 0f4bdc9 — 2026-08-16
Gideon Akinlotan

fix(console): Escape, amber, and the lists that said Empty when they meant broken

The frontend half of the 2026-08-14 audit. Every item here is a rule that held
on some of its surfaces and not the twin nobody enumerated.

## Escape (7 doors) — CLAUDE.md rule 16

`shortcuts.js` probed the DOM for `[role="dialog"]` alone, so six popup menus and
the console crash panel were invisible to it: pressing Escape in any of them
CLEARED THE CONGREGATION'S SCREENS and left the overlay open. The crash panel is
the sharpest — `role="alertdialog"`, and its own copy reads "Your output screens
are still live". The guard now covers the whole overlay class, and each menu also
consumes Escape, so the operator gets the outcome they asked for rather than
merely the absence of the one they did not. The bug was never "one role was
missed"; it was that the list was a list.

Its mirror image: the Announcements editor wore `role="dialog"` with no scrim, no
focus trap and no Escape handler, which DISARMED the panic key entirely — and Esc
is the only panic key that survives a focused text field, in a panel that is
nothing but text fields.

## Amber, which is never allowed to lie (rule 18)

`leavePlan()` had three callers against nine paths that take the wall, so the
plan rail kept drawing amber "On Air" over a cue nobody was looking at. Fixed in
three parts: five wrappers (three take `keepPlan`, because they are also the
plan's own take path), the `output://clear|black` listeners (a clear from the
LAN remote or a spoken command reaches `channels::clear` directly and this event
is the console's only report of it), and the transport MODE — which read `$live`,
"content is armed", where it meant "a congregation is looking at it". That last
one made `B` and `Esc` give opposite transports, so after a blackout mid-plan the
next arrow fired a stale verse AND cancelled the blackout.

`transport.test.js` pinned exactly the four wrappers that behaved and enumerated
none of their twins. It now asserts the rule over the whole set, derived from
source: add a tenth path and it names it until somebody decides.

## Blackout reached three screens out of four

`Stage.svelte` handled `clear` and not `black`, so the wall went dark and the
screen the PREACHER reads from kept the verse — while the console correctly
reported success, because the message had left the machine. The contract test now
derives the kind list from the Rust and demands a per-client verdict for each:
`false` with a reason is fine, silence is not.

## Empty vs Loading vs Error

Every read wrapper was GROUP 2, returning `[]` on failure, so no list could tell
"the query failed" from "there is nothing here" — the Templates tab said "No
templates yet" on an install that ships five built-ins. The rationale written in
capture.js IS the bug. Reads still return `[]`; what changed is that the reason
is no longer discarded (`readErrors`, same shape as `panicError`). All 22 are
routed, not the ones that mattered.

Six sites rendered a raw rejection — and since error.rs sends a typed OBJECT,
what a volunteer actually read was "[object Object]", in monospace on two of
them. History captured the reason a service failed to open and rendered "No
transcript recorded" instead, telling an operator their Sunday was never
recorded with the reason one property away. Seven error lines are now announced,
the worst being the run rail's.

## The transcript, which had no tests at all

A grep for the store across every test file returned zero — on the panel whose
own comment says it is "the difference between 'the preacher has not said a
reference' and 'Relay has gone deaf', and those need opposite responses". The
reducer is extracted (`applyTranscript`, pure) and pinned: a final clears the
partial, a partial never appends, and `finals`/`finalsAt` slice in LOCKSTEP —
asserted pairwise, because aligning them by length is a bug this repo has had.

## Two surfaces removed rather than fixed

`PreviewProgram.svelte` (312 lines) was imported by nothing; 14 tests had been
written against it. Its replacement's preview half then turned out to have no
producer either — `stage()` had zero callers — so `preview` was permanently null
and 17 more tests stood over a state the app could not reach. Both are gone.
`Live.svelte` owns Preview != Programme for real; the QUEUE is the Library's
staging area and always was. Accepting an AI suggestion stays one press.

Also: `resolve.conditions: ['browser']` in vitest.config.js — without it Svelte 4
resolves to the SSR build where `onMount` is literally `function onMount() {}`,
so no test in this repo had ever observed a load-on-mount path. Space no longer
fires scripture from a VerseDeck list row. Imported templates are sanitised at
the boundary, so a shared template cannot beacon or blank the wall offline.

Frontend 579/579. Build clean.

Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>

---