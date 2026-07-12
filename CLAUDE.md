# CLAUDE.md — Relay

Instructions for Claude Code (or any AI coding agent) working in this repo. Read this before touching code. It reflects real decisions made in a brainstorm session, not defaults — don't second-guess them without flagging it to the human first.

## What this is

Relay is AI-assisted live presentation software for churches. It listens to a live sermon, detects scripture references (direct quotes and paraphrases), and routes the right content to multiple independently-styled output screens in real time. It is built to interoperate with OBS, ATEM, and ProPresenter over NDI/HDMI/network — not to replace them.

Full context: `docs/SPEC.md` (canonical spec) and `docs/DECISIONS.md` (why, not just what).

## Non-negotiable constraints

- **No native SDI hardware integration.** Ever, unless a human explicitly reopens this. NDI + HDMI output only — SDI-based setups are served by existing bridging hardware (ATEM, converters) the church already owns.
- **Offline-first.** Every core feature (STT, verse detection, output rendering) must work with zero internet. Cloud STT is an optional fallback, never a requirement.
- **Operator override is a first-class control, never a fallback UI.** It must always be reachable in one action from the main console, at every stage of implementation — don't let it become an afterthought bolted on later.
- **Output channels are render targets of one shared template engine.** Never special-case rendering logic per channel type (main screen vs stage vs streaming vs lobby). If you find yourself writing an `if channel_type == "stage"` branch in rendering logic, stop — that's a template configuration problem, not a code problem.
- **Local-first data.** Transcripts, verse text, templates, and service history live in local SQLite by default. Nothing leaves the device without an explicit, visible reason (e.g. optional cloud STT).

## Tech stack

| Layer | Choice |
|---|---|
| Core engine | Rust |
| Desktop shell | Tauri (v2) |
| Frontend UI | Svelte + Vite |
| Local data | SQLite via `rusqlite` |
| Local realtime distribution | WebSocket (`tokio-tungstenite`) — powers networked/kiosk output channels |
| Speech-to-text | `whisper.cpp`-class local model, optional cloud fallback |
| Video-over-IP | NDI SDK via Rust FFI |
| Platforms | Windows + macOS, both from day one |
| License | MIT (project is free / open source — see `docs/DECISIONS.md`) |

## Priority languages (STT)

Tier 1, build and test against these first: **Yoruba, Swahili, Hausa**, plus English. Code-switching (English mixed mid-sentence with a local language) is the normal case, not an edge case — never write detection logic that assumes single-language input.

## Repo map

```
relay-project/
├── CLAUDE.md              — this file
├── README.md              — setup + run instructions
├── PROMPT.md              — full project brief / kickoff prompt for a build session
├── LICENSE                — MIT
├── docs/
│   ├── SPEC.md             — canonical technical spec
│   ├── DECISIONS.md        — decision log with reasoning
│   ├── data/schema.sql     — SQLite schema
│   └── design/             — visual mockups (open the .html files directly in a browser)
├── .github/workflows/ci.yml — fmt + clippy -D warnings + tests + **release build**, on macOS AND Windows
├── src-tauri/              — Rust core + Tauri backend
│   ├── .cargo/config.toml  — MACOSX_DEPLOYMENT_TARGET=11.0 (whisper.cpp needs std::filesystem)
│   └── src/
│       ├── main.rs         — Tauri commands, state, wiring (composition root)
│       ├── audio.rs        — cpal capture + VAD + overlapping chunker
│       ├── dsp.rs          — denoise + auto-gain + audio-quality warnings
│       ├── stt.rs          — whisper.cpp STT worker (rolling window, resample, language)
│       ├── detection.rs    — direct + semantic (TF-IDF) match, context memory, nav (DB/IO-free, heavily tested)
│       ├── router.rs       — the GATE: per-method routing, debounce, self-calibrating thresholds
│       ├── pipeline.rs     — the ONE place a verse becomes screen content (Fire → output + event)
│       ├── channels.rs     — output render targets: native window + kiosk WS hub + LAN HTTP
│       ├── telemetry.rs    — opt-in, content-scrubbed crash reporting (off by default)
│       └── db/             — SQLite, one module per aggregate; mod.rs owns connection + migrations
│           ├── mod.rs      — open/migrate (user_version ladder), per-OS paths, re-exports
│           └── templates · channels · profiles · verses · settings · plans · songs · library · services
├── src/                    — Svelte frontend
│   ├── lib/views/Live.svelte  — THE run surface (Console + Planner-run, merged)
│   ├── lib/views/ServicePlanner.svelte — BUILD a plan. Cannot fire to an output.
│   ├── lib/OutputWall.svelte  — the 4-monitor wall. ONE copy; both views had drifted.
│   ├── lib/plan.js         — cue/slide logic + the transport (pure, tested)
│   ├── lib/stores/capture.js — Tauri commands + event listeners + stores
│   ├── lib/shortcuts.js    — the ONE global keydown listener (panic keys live here)
│   ├── lib/crash.js        — crash boundary + recovery panel (plain DOM, not Svelte)
│   ├── lib/session.js      — operator position, persisted so a crash can resume
│   ├── lib/cues.js         — cue builders (one definition of each payload shape)
│   └── lib/*.test.js       — vitest, incl. the IPC contract test
├── data/kjv.json           — bundled full KJV (include_str!, committed)
└── models/                 — STT ggml models (gitignored, per-machine)
```

## Working conventions

- Rust: `rustfmt` and `clippy -D warnings` clean before any commit — CI enforces both. No `unwrap()` in code paths that run during a live service; a panic mid-sermon is the worst possible failure mode. Prefer explicit error surfacing to the operator UI over silent failure.
- Every module in `src-tauri/src/` has a doc comment stating its single responsibility — keep it that way. Don't let `router.rs` grow detection logic, don't let `channels.rs` grow detection logic, etc.
- **Never build an `OutputContent` or a `DetectionEvent` by hand.** Go through `pipeline::Fire`. Five hand-rolled copies drifted apart, and two of them silently dropped the scripture template.
- Confidence thresholds are configuration, not constants — and there is exactly ONE baseline: `Thresholds::default() == from_sensitivity(50)`, by construction (`router.rs`). Never introduce a second.
- Before implementing a new feature, check `docs/DECISIONS.md` — if the decision isn't there, it hasn't been made yet. Ask, don't assume.
- **Measure before optimising.** `detection.rs` has an `#[ignore]`d benchmark showing the "obviously slow" 31k-verse semantic scan costs 2.6 ms/query at ~1 query/sec. It stays a linear scan.

---

## Build status (as of this session) — the modules are NOT stubs anymore

Full pipeline works end to end: **listen → transcribe (local whisper) → detect (direct + semantic + context memory) → gate (router) → render on independently-templated output screens (native window + kiosk/OBS over WebSocket)**, fully offline. All `src-tauri/src/*.rs` are implemented.

Done: Phases 0–10 + service-session persistence + full KJV corpus + template engine + console UX + **rehearsal mode** (practise a whole service with nothing reaching the congregation — gated in `channels::broadcast_content`, the one function content leaves the machine through; see `docs/DECISIONS.md` §18). Parked (honest limits, not faked): **NDI** (needs proprietary SDK — `open_ndi_output` returns a clear error), **neural paraphrase embedder** (TF-IDF is the current seam behind `SemanticIndex::top_k`), **African-language STT fine-tunes** (base multilingual is weak on Yoruba/Hausa), and writing full detection history is service-scoped only.

## Run / build

```bash
npm install
npm run tauri dev        # desktop app + dev server on :5032, kiosk WS on :8031
```

- **Desktop app = a native macOS/Windows window.** `localhost:5032` in a plain browser has NO backend (shows a dead UI) — only the app window works. `:5032` exists for the app's webview + OBS/kiosk browser sources.
- **CMake is required** to build `whisper-rs` (compiles whisper.cpp). This machine has **no Homebrew**; cmake 3.31.6 is installed at `~/.local/bin/cmake`. Prefix cargo with `PATH="/Users/gideonakinlotan/.local/bin:$PATH"` for any build/test/clippy. `rustfmt`+`clippy` added via `rustup component add`. (See global memory `relay-build-toolchain`.)
- **This machine cannot screenshot the app** (Chrome ext account mismatch + no Screen Recording perm). Verify via `cargo test`, `vite build`, backend stdout, and — for the webview — the **boot heartbeat**: `App.svelte` calls `greet` on mount and `main.rs` prints `console: webview up (operator)`. That line in the log is the proof the webview loaded, ran its JS, and reached the Tauri bridge. **No line = blank/broken console.** (Global memory `relay-no-screenshot-path`.)
- **`tauri dev` does NOT exercise the CSP.** In dev, Tauri loads the Vite `devUrl`; the `app.security.csp` policy only applies when Tauri serves the bundled assets. **Any CSP change must be verified with `npm run tauri build`** and by launching the packaged binary — a policy that blocks media, fonts or the IPC bridge looks completely fine in `tauri dev`.
- **Debugging audio / STT without a human at the mic.** Every audio bug so far was invisible in the code and reproducible only with a specific microphone in a specific room. The tools:
  - `RELAY_RECORD_WAV=/path/x.wav` — write the CLEANED stream (exactly what the VAD and whisper see) to a WAV on Stop. Sermon audio: off by default, no UI, never uploaded (PRIVACY.md).
  - `RELAY_STT_TIMING=1` — per-decode lag / decode ms / gap between updates / window size / voiced-vs-silent counts. Content-free.
  - `RELAY_AUDIO_RMS=1` — what the voice gate actually sees (rms distribution, voiced ratio, denoise on/off).
  - `RELAY_BENCH_WAV=… cargo test audio::gate stt::bench -- --ignored --nocapture` — replay real speech through the real front-end at any mic level (`RELAY_BENCH_SCALE`) and noise (`RELAY_BENCH_NOISE`). This is how the "deaf to a quiet preacher" bug was found and proved.
- **STT models** live in `/models/` (gitignored). `ggml-base.bin` (multilingual, preferred) and `ggml-base.en.bin`. Resolved via `RELAY_MODEL_PATH` → repo `models/` → app-data. See README.
- **Full KJV** is bundled at `src-tauri/data/kjv.json` (`include_str!`, ~4.5MB, committed — required for build). 66 books, 31,100 verses.
- **SQLite** dev DB: `~/Library/Application Support/com.relay.app/relay.db`. `open()` runs forward-fill migrations (templates, channels, full-Bible reimport, vw→cqw template reset).

## Ports (global registry, NN=03)

`5032` = operator console (app surface, Vite strictPort) · `8031` = kiosk/OBS WebSocket hub. OBS/vMix: add a **Browser Source** → `http://localhost:5032/output.html?template_id=<n>` (live over WS, no NDI). Same URL for a Raspberry-Pi kiosk (swap `localhost` for the host IP).

## Architecture rules learned the HARD WAY — do not regress these

These caused real, hours-long crashes/freezes. Keep them.

1. **Never call `tick()` inside a reactive `$:` block** (Svelte) — it re-enters the scheduler and infinite-loops the webview JS thread → hard freeze, no error. Use `afterUpdate` for DOM side-effects. (Was the transcript auto-scroll freeze.)
2. **Never hold a `Mutex` lock across `handle.emit` / `channels::broadcast_content`** on a background thread — deadlocks the macOS main run loop against a command wanting the same lock. Compute under lock, release, THEN emit. (`emit_detections`, `handle_nav` do this.)
3. **The STT worker thread needs a big stack** (16MB via `thread::Builder`). `whisper_full()` is stack-hungry; running it then serializing a Tauri emit on the same thread overflows the default 2MB → silent SIGSEGV right after the first transcript.
4. **Call `whisper_rs::install_logging_hooks()` once** — otherwise whisper.cpp floods stderr with thousands of per-token lines per transcription (I/O storm that looks like a freeze).
5. **Audio capture start must be NON-BLOCKING.** `start_capture` is a Tauri command; sync commands run on the UI thread. `AudioEngine::start` spawns the capture thread and returns immediately; device errors come back via the `audio://error` event.
6. **Consistent global lock order: `Db` before `Session`** everywhere (`persist_transcript`, `persist_fire`, `start_service`) — avoids a lock-ordering deadlock between the STT thread and command threads.
7. **`initAudio()` runs at app level** (`App.svelte` onMount), not only in Settings — otherwise `$capture.available` is false on the Console (default tab) and every button looks dead.
8. **STT is fed the NON-overlapping tail of each chunk** (the detection chunker emits 50%-overlapping chunks; feeding them verbatim garbles whisper). Computed from timestamps in `stt.rs` worker.
9. **Never hand-roll an app-data path.** Use `db::app_data_dir()`. `stt.rs` once had its own macOS-only `$HOME/Library/Application Support` variant, so on packaged **Windows** the STT model was never found and Relay ran with speech recognition silently dead. Windows has no `HOME`.
10. **Only `DetectionMethod::Direct` may auto-fire.** A TF-IDF cosine is not a probability — gating it with a threshold gates it against noise. Semantic/Ambiguous candidates are capped at `Suggest` in `router.rs::decide`, at any score. Do not "fix" this by raising a number.
11. **Panic keys live in `lib/shortcuts.js`, mounted once at `App.svelte`** — never per-view. When they were per-view, Escape did nothing on the Templates/Library/Settings tabs. And `Space` means *advance* app-wide, nothing else.
12. **Audio levels are LEARNED, never assumed** (`docs/DECISIONS.md` §19). Nothing in the audio path may compare a signal to an absolute level. Three thresholds that each looked reasonable (`VAD_RMS_THRESHOLD = 0.008`; `energy_prob = rms / 0.12` needing 0.066 to register as speech; `MAX_GAIN = 6`) together made Relay **deaf to a quiet preacher, silently** — the auto-gain would not lift him because it did not believe he was speaking, and it did not believe he was speaking because he had not been lifted. Measured, same words: 94% voiced at studio level, **2% at a church-laptop level**. Nothing errored; the meter still moved; the transcript just turned to nonsense. Speech is a *rise above the room*, and it is **contrast, not volume**. Verify with `cargo test audio::gate -- --ignored` — the voiced ratio must stay flat across a 100× range of input level.
13. **Score STT changes through the DETECTOR, never by reading the transcript.** A grep-the-text scorer rated a hallucinated `Peter 8 verse 28` a success (wrong book, right number) and a correct `chapter eight verse twenty-eight` a failure (spelled out — which `detection.rs` parses fine). It flattered the option that hallucinated. The only question is *which verse would Relay put on the screen*. (`stt::bench::prompt_sweep`.)
14. **Measure the decoder before "improving" it.** Whisper decodes the 8s window in ~207 ms against a 1000 ms budget, and beam-5 recovers exactly the same references as greedy for ~50% more time. `Decode::Beam` exists, is benchmarked, and is deliberately NOT used.
15. **`persist_fire` takes the real `status`.** Manual/confirmed fires are `'manual'`, not `'auto'` — the self-calibrating router learns from that column, so mislabelling human decisions as machine ones trains it on a falsified log.

## Frontend shape

- **Tabs: Live · Channels · Templates · Library · Planner · Settings · Help.** There is no Console tab any more — `Live` IS the console. **Build** a plan in Planner (a Tuesday job; nothing there can reach an output). **Run** it in Live (a Sunday job). The merge exists because an operator running a plan on a separate tab could not see the AI's suggestions — and the preacher going off-script is the entire product.
- **The transport is MODE-AWARE and says so.** `→` steps a plan SLIDE when plan content is on air, and walks the passage (VERSE) when a detected/manual verse is. The mode is printed in the transport bar; the same key silently meaning two things is how the wrong thing reaches a congregation.
- **`liveCue` = `{ cueId, slide, onAir }` — position and on-air-ness are SEPARATE facts.** The panic keys clear only `onAir`. Wiping the position would make the next `→` restart the plan at cue 1 — the opening countdown, back on the wall, at the end of the service. A cue that is where `→` resumes but is NOT on screen reads **CUED**, in grey. Never amber. Amber means live and is never allowed to lie.
- One store: `src/lib/stores/capture.js` (writables: `capture`, `transcript`, `detections` = pending suggestions only, `live` = what's on screen, `templates`). All Tauri command wrappers + event listeners live here.
- **`src/lib/TemplateRender.svelte` is the ONE renderer** for both the fullscreen output and the Templates editor preview → WYSIWYG. Sizes are **cqw** (container-query %) so a template scales identically at any output size. Output page is **transparent** so a Transparent-background template keys out for OBS/ATEM.
- Tauri events: `audio://chunk` (throttled level meter), `stt://transcript`, `detection://match`, `output://content`, `output://clear`, `template://updated`, `audio://error`.

## Detection notes

- `detection.rs` is DB/IO-free and heavily unit-tested. Book aliases: full names, numbered ("1 john"/"first john"/"1jn"), fast abbreviations ("ps 23 1", "rom 8 1"), ASR mishears ("sam"→Psalms, apostrophes dropped so "Sam's"→Psalms).
- Spoken-number FSM: "three sixteen" → 3:16 (not 19). Single-chapter books ("Jude 4" → 1:4). Ambiguous "revelation 22" → suggests 22:1 AND 2:2.
- Voice/manual nav: "next"/"back" (short utterances) and the console Prev/Next buttons both go through the `nav` command → `handle_nav`.
