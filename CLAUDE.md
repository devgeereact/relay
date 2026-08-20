# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Read this before touching code. It reflects real decisions made in real sessions, not defaults — don't second-guess them without flagging it to the human first.

## What this is

Relay is AI-assisted live presentation software for churches. It listens to a live sermon, detects scripture references (direct quotes and paraphrases), and routes the right content to multiple independently-styled output screens in real time. It is built to interoperate with OBS, ATEM, and ProPresenter over NDI/HDMI/network — not to replace them.

Full context: `docs/SPEC.md` (canonical spec), `docs/DECISIONS.md` (why, not just what), `docs/PRODUCT_AUDIT.md` (current health, honestly scored).

## Non-negotiable constraints

- **No native SDI hardware integration.** Ever, unless a human explicitly reopens this. NDI + HDMI output only — SDI setups are served by bridging hardware (ATEM, converters) the church already owns.
- **Offline-first.** Every core feature (STT, verse detection, output rendering) must work with zero internet. Cloud STT is an optional fallback, never a requirement.
- **Operator override is a first-class control, never a fallback UI.** Always reachable in one action, at every stage.
- **Output channels are render targets of one shared template engine.** If you find yourself writing `if channel_type == "stage"` in rendering logic, stop — that's a template configuration problem, not a code problem.
- **Local-first data.** Transcripts, verse text, templates and service history live in local SQLite. Nothing leaves the device without an explicit, visible reason.

## Tech stack

| Layer | Choice |
|---|---|
| Core engine | Rust |
| Desktop shell | Tauri (v2) |
| Frontend UI | Svelte 4 + Vite |
| Local data | SQLite via `rusqlite` |
| Local realtime distribution | WebSocket (`tokio-tungstenite`) |
| Speech-to-text | `whisper.cpp` (`whisper-rs`), local model |
| Platforms | Windows + macOS, both from day one |
| License | MIT (free / open source) |

## Priority languages (STT)

Tier 1: **Yoruba, Swahili, Hausa**, plus English. Code-switching (English mixed mid-sentence with a local language) is the normal case, not an edge case — never write detection logic that assumes single-language input.

**Be honest about the moat**: today it is a hand-curated multilingual *reference-parsing* table (66 books × 3 languages, in `data/book_aliases.json`) on top of stock Whisper base. No fine-tuned acoustic model ships, Yoruba numerals are not parsed, no native speaker has reviewed the aliases, and word error rate has never been measured in any language. `docs/LANGUAGES.md` says all of this plainly — **do not soften it.**

## Commands

```bash
npm install
npm run tauri dev        # desktop app + Vite on :5032, kiosk WS on :8031

npm test                 # vitest (579 tests)
npx vitest run src/lib/nav.test.js          # one file
npx vitest run -t "Escape closes the cheat" # one test by name
npm run build            # vite build — catches Svelte compile errors fast

npm run version:check    # the 3 version files agree (CI runs this)
npm run version:set -- 0.2.0
```

Rust (**`whisper-rs` compiles whisper.cpp from source, so `cmake` must be on PATH**):

```bash
cmake --version          # any 3.x. `brew install cmake` if it is missing;
                         # a machine without Homebrew needs its own prefix on PATH
cd src-tauri
cargo test                                   # 502 tests (476 run, 26 ignored)
cargo test e2e                               # the fire → nav → clear path (23 tests)
cargo test detection::                       # one module
cargo test the_macos_build -- --nocapture    # one test
cargo fmt --all && cargo clippy --all-targets -- -D warnings   # CI enforces both
cargo test eval::tests::print_scorecard -- --nocapture         # detection scorecard
```

- **Desktop app = a native window.** `localhost:5032` in a plain browser has NO backend (dead UI). `:5032` exists for the app's webview + OBS/kiosk browser sources.
- **`tauri dev` does NOT exercise the CSP.** In dev, Tauri loads the Vite `devUrl`; `app.security.csp` only applies to bundled assets. **Any CSP change must be verified with `npm run tauri build`** against the packaged binary.
- **This machine cannot screenshot the app.** Verify via `cargo test`, `vite build`, backend stdout, and the **boot heartbeat**: `App.svelte` calls `greet` on mount, `main.rs` prints `console: webview up (operator)`. No line = blank/broken console. **Exactly one line per launch** — the count is the signal, so `greet` has exactly ONE caller and anything else asking "is the bridge up?" calls `ping`, which is silent. Pinned by `ipc.test.js`; see §26.
- **STT models** live in app-data (`db::app_data_dir()/models`), gitignored. Downloaded in-app (`models.rs`), or resolved via `RELAY_MODEL_PATH` → repo `models/`.
- **Full KJV** is bundled at `src-tauri/data/kjv.json` (`include_str!`, committed — required to build).
- **SQLite** dev DB: `~/Library/Application Support/com.relay.app/relay.db`.

### Debugging audio / STT without a human at the mic

Every audio bug so far was invisible in the code and reproducible only with a specific mic in a specific room.

- `RELAY_RECORD_WAV=/path/x.wav` — write the CLEANED stream (what the VAD and whisper actually see) on Stop. Off by default, never uploaded.
- `RELAY_STT_TIMING=1`, `RELAY_AUDIO_RMS=1` — decode lag / what the voice gate sees. Content-free.
- `RELAY_SENTRY_DSN=…` — **debug builds only** — point crash reporting at your own Sentry project without touching Settings. Still scrubbed; compiled out of release. Empty = unset.
- `RELAY_BENCH_WAV=… cargo test audio::gate stt::bench -- --ignored --nocapture` — replay real speech through the real front-end at any mic level (`RELAY_BENCH_SCALE`) and noise (`RELAY_BENCH_NOISE`). This is how the "deaf to a quiet preacher" bug was proved.

## Repo map

```
├── CLAUDE.md · README.md · LICENSE · PRIVACY.md · SECURITY.md
├── docs/  README (index) · SPEC · DECISIONS · ARCHITECTURE · DOMAIN_MODEL
│          DESIGN_SYSTEM · PRODUCT_AUDIT · ROADMAP · LANGUAGES · QA_HARNESS
│          RELEASING · USER_GUIDE · AI_DISCLOSURE · data/schema.sql
├── scripts/version.mjs      — the ONLY place the version is read or written (3 files)
│   scripts/qa-inventory.mjs — controls · orphan components · command map · create paths
├── .github/workflows/       — ci.yml (fmt·clippy·tests·scorecard·build, macOS+Windows)
│                              release.yml (per-platform signing gate; tags only)
├── src-tauri/
│   ├── Info.plist           — NSMicrophoneUsageDescription. NOT optional. See §17.
│   ├── relay.entitlements   — com.apple.security.device.audio-input. See §17.
│   ├── tauri.conf.json      — base; tauri.updater.conf.json overlays it at release
│   └── src/
│       ├── main.rs          — Tauri commands + the live-fire engine (4.0k lines, 114 cmds)
│       ├── qa.rs · qa_r5.rs · r6.rs — TEST-ONLY. qa.rs owns THE fixture: bare_app() =
│       │                      a fresh install and nothing else, + Wall (Tauri events)
│       │                      and Kiosk (the WS door). The other two are audit suites.
│       ├── e2e.rs           — TEST-ONLY. Drives the real fire → nav → clear commands
│       │                      against a real in-memory DB via qa::bare_app()
│       ├── audio.rs · dsp.rs · stt.rs      — capture · denoise/gain · whisper worker
│       ├── detection.rs     — direct + semantic (TF-IDF) + context memory. DB/IO-free, heavily tested
│       ├── router.rs        — THE GATE: per-method routing, debounce, self-calibrating thresholds
│       ├── pipeline.rs      — the ONE place a verse becomes screen content (Fire → output + event)
│       ├── channels.rs      — output render targets: native window + kiosk WS + LAN HTTP
│       ├── models.rs        — in-app STT model download (resumable, checksummed, cancellable)
│       ├── error.rs          — the ONE typed error across the bridge { kind, message }
│       ├── proimport.rs · songs.rs — ProPresenter import; pure song-lyric parsing
│       ├── sysprobe.rs       — host capability probe (can this machine run this model?)
│       ├── eval.rs          — CI-gated detection benchmark, scored THROUGH the router
│       ├── telemetry.rs     — opt-in, content-scrubbed, no DSN in OSS builds
│       └── db/              — one module per aggregate; mod.rs owns connection + migrations
└── src/
    ├── App.svelte           — shell: tabs, panic keys, aria-live, panic banner, update banner
    ├── Output.svelte · Stage.svelte   — the projector, and the preacher's phone
    └── lib/
        ├── views/Live.svelte          — THE run surface (console + plan, merged)
        ├── views/ServicePlanner.svelte— BUILD a plan. Cannot fire to an output.
        ├── stores/capture.js          — all Tauri commands + event listeners + stores
        ├── shortcuts.js               — the ONE global keydown listener (panic keys)
        ├── detect.js                  — heard vs guessed (the frontend half of the gate)
        ├── errors.js                  — the ONE backend-error humaniser
        ├── themes.js · layers.js       — the style layer beneath templates; layer starters
        ├── plan.js · cues.js · session.js · crash.js · updater.js
        └── *.test.js                  — vitest, incl. the IPC contract test
```

## Working conventions

- `rustfmt` + `clippy -D warnings` clean before any commit — CI enforces both.
- **No `unwrap()`/`expect()` in code that runs during a live service.** A panic mid-sermon is the worst possible failure. (Currently zero in the seven service modules; `main.rs` has 5, all startup-only.)
- Every module in `src-tauri/src/` has a doc comment stating its single responsibility. Keep it that way — don't let `router.rs` grow detection logic.
- **Never build an `OutputContent` or a `DetectionEvent` by hand.** Go through `pipeline::Fire`. Five hand-rolled copies drifted apart, and two silently dropped the scripture template.
- Confidence thresholds are configuration, not constants — and there is exactly ONE baseline: `Thresholds::default() == from_sensitivity(50)`, **by construction** (`router.rs`). Never introduce a second.
- Before implementing a feature, check `docs/DECISIONS.md`. If the decision isn't there, it hasn't been made. Ask, don't assume.
- **Measure before optimising.** `detection.rs` has an `#[ignore]`d benchmark showing the "obviously slow" 31k-verse semantic scan costs 2.6 ms/query at ~1 query/sec. It stays a linear scan. Beam search is benchmarked and deliberately NOT used.

## Build status

Full pipeline works end to end: **listen → transcribe (local whisper) → detect (direct + semantic + context) → gate (router) → render on independently-templated outputs (native window + kiosk/OBS over WebSocket)**, fully offline.

Shipping: in-app model download, first-run wizard, auto-updater, rehearsal mode, crash recovery, service history, template engine, ProPresenter import, CI-gated detection benchmark, opt-in scrubbed telemetry.

Parked, honestly (not faked): **NDI** (needs proprietary SDK — `open_ndi_output` returns a clear error), **neural paraphrase embedder** (TF-IDF is the seam behind `SemanticIndex::top_k`; the `verses.embedding` column exists and has never been written to), **African-language STT fine-tunes**.

**No dead-but-built commands — with ONE known exception, recorded rather than hidden.** Every one of the 114 registered `#[tauri::command]`s has a frontend caller **in `capture.js`**; that is the level `ipc.test.js` checks, and it is not the level that matters. `save_arrangement` is the exception: the wrapper `saveArrangement` exists and **no component imports it**, so a user cannot save a song arrangement at all. Found by `scripts/qa-inventory.mjs`, which traces the chain one hop further — to a control something actually renders. Building the arrangement editor is a feature, not a fix; until it exists, this sentence is the honest version of the claim. The last thirteen were closed together: five superseded ones were deleted (`lookup_verse`, `close_output_window`, `current_service`, and the `*_template_active` pair — the console Output grid became per-channel templates), and eight were given the UI they had always lacked — voice profiles (SPEC §4.6), the emergency announcement, and the "shown earlier" badge. `related_scripture` was wired by the new-design merge.

## Architecture rules learned the HARD WAY — do not regress these

These caused real crashes, freezes, or silent failures in front of people. Keep them.

1. **Never call `tick()` inside a reactive `$:` block** (Svelte) — re-enters the scheduler and infinite-loops the webview JS thread → hard freeze, no error. Use `afterUpdate`.
2. **Never hold a `Mutex` across `handle.emit` / `channels::broadcast_content`** — deadlocks the macOS main run loop against a command wanting the same lock. Compute under lock, release, THEN emit.
3. **The STT worker thread needs a big stack** (16MB via `thread::Builder`). `whisper_full()` is stack-hungry; the default 2MB overflows → silent SIGSEGV after the first transcript.
4. **Call `whisper_rs::install_logging_hooks()` once** — otherwise whisper.cpp floods stderr with thousands of lines per transcription.
5. **Audio capture start must be NON-BLOCKING.** `AudioEngine::start` spawns and returns; device errors come back via the `audio://error` event.
6. **Consistent global lock order: `Db` before `Session`** everywhere.
7. **`initAudio()` runs at app level** (`App.svelte` onMount), not only in Settings — otherwise `$capture.available` is false on the default tab and every button looks dead.
8. **STT is fed the NON-overlapping tail of each chunk** — the detection chunker emits 50%-overlapping chunks; feeding them verbatim garbles whisper.
9. **Never hand-roll an app-data path.** Use `db::app_data_dir()`. A macOS-only `$HOME/…` variant meant packaged **Windows** never found the STT model and ran with speech recognition silently dead. Windows has no `HOME`.
10. **Only `DetectionMethod::Direct` may auto-fire — and "Direct" means Relay HEARD it, not that it parsed confidently.** A TF-IDF cosine is not a probability. Semantic/Ambiguous/**UncertainBook** are capped at `Suggest` in `router.rs::decide`, at any score. Do not "fix" this by raising a number. `UncertainBook` was added 2026-08-14 after a P0: `fuzzy_book` repairs a misheard book token by edit distance, gated only on "the next token is a number" — which is the exact shape of a church announcement. "please turn to **hymn number** three sixteen" put **Numbers 3:16** on the wall at 0.840 against a 0.50 bar, unattended; `room`→Romans, `row`, `van`, `day`→Daniel and 33 more did the same. The claimed mitigation ("marked FUZZY, costs confidence downstream") was worth **0.06** against a 0.34 margin. Its confidence was never wrong — it was a real parse confidence, about a word nobody said, which is why no threshold could have saved it. Two routes now yield `UncertainBook`: an edit-distance repair (never rescued — a guess about the acoustics), and an ordinary English word that is also a one-token alias (`song`, `job` — rescued by an explicit chapter/verse keyword, because the word genuinely *was* heard). **`psalm` is deliberately not on that list** and must never be: "Psalm twenty three" is how preachers say it.
11. **Panic keys live in `lib/shortcuts.js`, mounted once at `App.svelte`** — never per-view. `Space` means *advance*, app-wide, and nothing else.
12. **Audio levels are LEARNED, never assumed** (DECISIONS §19). Nothing may compare a signal to an absolute level. Three individually-reasonable thresholds together made Relay **deaf to a quiet preacher, silently** — 94% voiced at studio level, **2% at a church-laptop level**. Speech is *contrast*, not volume. Verify: `cargo test audio::gate -- --ignored`.
13. **Score STT changes through the DETECTOR, never by reading the transcript.** A grep-the-text scorer rated a hallucinated `Peter 8 verse 28` a success and a correct spelled-out reference a failure. The only question is *which verse would Relay put on the screen*.
14. **`persist_fire` takes the real `status`.** Manual fires are `'manual'`, never `'auto'` — the self-calibrating router learns from that column.
15. **A panic control may never report a success it did not achieve** (DECISIONS §20). `clear_screens`/`blackout` return `Result`; the frontend wrappers return a boolean **and** set the global `panicError` store — both, because panic controls fire from a global keydown handler and a shell button that must survive a crashed view, and neither can `catch`. `Live.svelte` once flashed "Screens cleared" over a `catch {}` that could never fire.
16. **`Esc` must not clear the screens while any dialog is open.** `shortcuts.js` checks for a mounted `[role="dialog"]`. Dismissing a help overlay or an arrangement picker is not a live action — it used to wipe the wall as a side-effect.
17. **macOS: the microphone dies on the FIRST correctly-signed build.** Notarization requires the hardened runtime; under it, opening an input device without `com.apple.security.device.audio-input` is TCC-killed, and without `NSMicrophoneUsageDescription` the app is *terminated the instant it asks*. `tauri dev` and unsigned pre-releases both work fine — so this is invisible until the one build you hand to a church. `src-tauri/relay.entitlements` + `src-tauri/Info.plist`, pinned by `models::config_boots`. **Do not "clean up" those files.** **Reproduce it locally without a certificate**: `npm run tauri build && ./scripts/sign-local.sh` — `codesign --options runtime` turns the hardened runtime on for an ad-hoc signature too, and the script fails loudly if the entitlement or the usage string is missing.
18. **The operator must see WHICH KIND of claim the AI is making** (DECISIONS §21). `matched_text` + `method` cross the IPC bridge and are rendered. A paraphrase shows **no percentage at all** — a cosine is not a probability, and a number that lies is worse than no number. Cyan for a guess, **never amethyst** (that means rehearsal) and never amber (that means ON AIR).
19. **The version lives in THREE files** (`tauri.conf.json`, `package.json`, `Cargo.toml`) and `tauri.conf.json`'s copy is what the updater manifest advertises. If they drift from the tag, `latest.json` stamps the new build with the OLD version, every install compares equal, and **nothing ever updates — silently**. Use `npm run version:set`; CI asserts agreement on every PR, the release gate asserts it equals the tag.
20. **Pre-release versions must be NUMERIC** (`0.1.0-1`, not `0.1.0-rc1`). Valid semver, builds a fine `.dmg` — and the Windows MSI bundler rejects a named identifier fifteen minutes into the release. `scripts/version.mjs` refuses it locally in a second.
21. **An empty env var is not an absent env var.** Passing `APPLE_ID`/`APPLE_TEAM_ID` as `''` on an unsigned build makes Tauri believe notarization was requested, and it dies on `Team ID must be at least 3 characters`. There is no way to conditionally omit a key from an Actions `env:` block, so they are exported to `$GITHUB_ENV` only when actually signing. (Same trap as the signing identity, one level deeper.)
22. **GitHub Actions evaluates `${{ }}` EVERYWHERE — including inside comments in a `run:` block.** An empty one is a parse error that invalidates the whole workflow, so GitHub can't even apply the `on:` filter and every push gets a zero-job "startup failure". Never write an Actions expression in a workflow comment, even to illustrate one.
23. **A release is signed per-platform, or not at all.** One global "is it signed?" flag *is* the bug: it tested `APPLE_CERTIFICATE` and shipped an unsigned Windows MSI in silence. Two certificates, two independent verdicts, and the gate fails loud on a real tag.
24. **The fire engine is generic over `tauri::Runtime`** (`fire_manual`, `handle_nav`, `clear_or_report`, `persist_cue`, and `channels::{broadcast_content, clear, black, stage_next}`). That is what makes `e2e.rs` possible: welded to the concrete desktop runtime, the one path that puts scripture on a wall could not be driven without a window, and so was never tested. **Keep new fire-path code generic** — a concrete `AppHandle` quietly re-welds it.
25. **A migration must be RETRYABLE.** `ensure_manual_detection_status` rebuilds a table (SQLite cannot `ALTER` a `CHECK`). It had no `ROLLBACK`, so a mid-batch failure left the transaction open — the following `PRAGMA foreign_keys = ON` then ran *inside* it, where the pragma is a documented no-op, and the error panicked the app at startup with FKs off. The leftover `detections_new` scratch table then made **every subsequent boot** fail with "table already exists", forever, before the window is even shown. Always `DROP TABLE IF EXISTS` the scratch table first, and roll back on failure.
26. **`greet` is a COUNTER, not a health check — exactly one caller, forever.** It prints `console: webview up`, and on a machine that cannot screenshot the app that line is the only proof the webview loaded and reached the Tauri bridge. Its entire value is the *count*: one line, one console mount. The new-design branch added `probes.js:engine()` calling `greet` to ask "is the bridge attached?", and that probe runs from **both** the launch sequence and the Dashboard — so every launch printed the heartbeat **three times**. Nothing was broken, which is the point: the one instrument for diagnosing a blank console now read exactly like a webview reloading twice, and a real double-mount would have been invisible in the noise. **Liveness probes call `ping`** (silent, returns `true`). Pinned by `ipc.test.js`, which fails if any file other than `App.svelte` mentions `greet`.

## Frontend shape

- **Tabs: Live · Outputs · Templates · Themes · Library · Planner · Settings · Help.** (The Outputs tab's internal key is still `channels`, and its view file is `Channels.svelte` — the label is what an operator reads.) There is no Console tab — `Live` IS the console. **Themes** is the style layer beneath templates (DECISIONS §27): a theme sets default `style` keys, a template overrides them per key, and `TemplateRender` resolves a template's `style.themeRef` against builtins itself, so every surface is themed with no per-surface wiring. Layer colours may bind to theme tokens (`theme:accent`). Stage/confidence monitors are render-profiles of the one engine (starters in `layers.js`), not a parallel system — they show monitor-only fields (`next`, `note`, `elapsed`) that ride to output but no congregation template renders. **Build** a plan in Planner (a Tuesday job; nothing there can reach an output). **Run** it in Live (a Sunday job). The merge exists because an operator running a plan on a separate tab could not see the AI's suggestions — and the preacher going off-script is the entire product.
- **The transport is MODE-AWARE and says so.** `→` steps a plan SLIDE when plan content is on air, and walks the passage (VERSE) when a detected/manual verse is. The mode is printed in the transport bar; the same key silently meaning two things is how the wrong thing reaches a congregation.
- **`liveCue` = `{ cueId, slide, onAir }` — position and on-air-ness are SEPARATE facts.** Panic keys clear only `onAir`. Wiping the position would make the next `→` restart the plan at cue 1. A cue that is where `→` resumes but is NOT on screen reads **CUED**, in grey. Never amber. Amber means live and is never allowed to lie.
- One store: `src/lib/stores/capture.js` (`capture`, `transcript`, `detections` = pending suggestions only, `live` = what's on screen, `panicError`, `templates`). All Tauri command wrappers + event listeners live here.
- **`src/lib/TemplateRender.svelte` is the ONE renderer** for the fullscreen output and the Templates editor preview → WYSIWYG by construction. Sizes are **cqw** so a template scales identically at any output size. The output page is **transparent** so a Transparent-background template keys out for OBS/ATEM.
- **`src/lib/errors.js` is the ONE backend-error humaniser.** Never render a raw Rust `Err` string to a volunteer — Channels did, in monospace, five times.
- Tauri events: `audio://chunk`, `stt://transcript`, `detection://match`, `output://content`, `output://clear`, `output://black`, `output://panic_failed`, `nav://blocked`, `template://updated`, `audio://error`, `model://progress|done|error|cancelled`.

## Detection notes

- `detection.rs` is DB/IO-free and heavily unit-tested. Book aliases: full names, numbered ("1 john"/"first john"/"1jn"), fast abbreviations ("ps 23 1"), ASR mishears ("sam"→Psalms).
- Spoken-number FSM: "three sixteen" → 3:16 (not 19). Single-chapter books ("Jude 4" → 1:4). Ambiguous "revelation 22" → suggests 22:1 AND 2:2.
- Voice/manual nav: "next"/"back" and the console Prev/Next both go through the `nav` command → `handle_nav`, which returns a **`NavResult`** (Fired / EndOfPassage / NoPassage / NotInLibrary). Not every outcome is a failure — reaching the end of a passage is a correct boundary, and the operator must be told *which*. It used to return `()` and silently do nothing.
- **A guarantee is only kept on the doors you checked.** Three separate bugs this repo has now had are the same bug: a rule enforced on one surface and skipped on its twin. Rehearsal gated three of four kiosk publishers. The throw-vs-swallow contract held for eight of nine group-1 wrappers. And `NavResult` — built precisely so nav could never again silently do nothing — was thrown away by `remote_api` with `Ok(_)`, so the preacher's phone answered `{"ok":true}` at the end of a reading and moved nothing, which is the original bug verbatim. When you fix something, **enumerate every caller of the thing you fixed**, and write the test on the surface that was missed.

## Ports (global registry, NN=03)

`5032` = operator console (Vite dev server) · `8031` = kiosk/OBS WebSocket hub · `8032` = embedded HTTP server (output/stage pages + `/media/<id>`).

**OBS/vMix/kiosk browser source → `http://<host>:8032/output.html?channel=<id>&template_id=<n>`.** The URL is CHANNEL-keyed (DECISIONS §29): changing a screen's template broadcasts a `channel_template` message the output applies by matching its own `channel`, so a template swap is live with no re-copying of the URL. `template_id` is only the first render before any push. Use **Copy URL** in Outputs → Screens; a hand-built `?template_id=`-only URL will not live-swap.

**NOT 5032.** `5032` is Vite. It exists ONLY under `npm run tauri dev`; in the packaged app there is no server on it at all, so a browser source pointed there shows a blank screen with no error and nothing in any log. The docs said 5032 for months.

## Testing

**476 Rust** (26 ignored) + **579 frontend** (0 skipped), re-measured 2026-08-20. CI runs both on **macOS and Windows**, plus `fmt`, `clippy -D warnings`, the detection scorecard, and a release build.

- **`qa.rs` owns the fixture. Do not write another one.** `qa::bare_app()` is a fresh install and nothing else — real schema, real seed, no operator has touched it — and `qa::{Wall, Kiosk, settle}` are the two doors out of the machine plus the drain. `e2e::app()` is now `bare_app()` **plus one documented difference** (a content-look override, without which its template assertion is vacuous), which is exactly the shape a deviation should have: three visible lines, not a fifty-line copy that drifts. A second fixture is how two suites start disagreeing about what a fresh install contains. `the_bare_fixture_is_a_first_launch_and_nothing_more` is the tripwire; it is what caught that `tpl_song` **is** seeded on purpose (every other built-in is scripture-shaped, so a lyric rendered through one showed the song title instead of the words).
- **The QA apparatus is documented, not folklore.** `docs/QA_HARNESS.md` — Part 0 the current counts (each with the command that reproduces it), Part 1 the design and the five evidence layers, Part 2 the shared preamble every `relay-qa-*` agent inherits verbatim, Part 3 the roster, **Part 4 what is already pinned — read it before filing anything, so you don't "find" a fixed bug**. Run `/qa-audit`; `node scripts/qa-inventory.mjs` prints the control/orphan/create-path report on its own.
- **A component nothing renders is not covered, however green its tests.** Fourteen passing tests were written against `PreviewProgram.svelte` before `qa-inventory.mjs` reported that nothing imports it — the shipped surface is `LiveOutputRail.svelte`. Before writing a component test, check something actually renders the component.

- **`vitest.config.js` MUST set `resolve: { conditions: ['browser'] }`. Do not "tidy" it away.** Svelte 4 maps its `.` export to `src/runtime/ssr.js` under every condition except `browser`, and that file defines `onMount`, `beforeUpdate` and `afterUpdate` as literal empty functions. `environment: 'jsdom'` does not imply the condition. Without the line, a component is compiled for the DOM and handed the SSR stubs: it mounts, it renders, and it silently skips every load-on-mount path — no list fetches, no subscription runs, and the test passes by doing nothing. It hid for the entire life of the project because the asymmetry is invisible: `svelte/internal` has only a `default` condition, so rendering, `onDestroy` and `tick` were always real, and only the mount half was dead. Guarded from three directions now — `r6-lifecycle-probe.test.js` asserts the runtime, `r2livepath.test.js` asserts that mounting the run column actually reaches `list_output_channels`, and `surface.test.js`'s `LIFECYCLE_LIVE` gate reads the runtime so its tests skip loudly rather than pass vacuously.
- **The frontend suite must pass on a CURRENT Node, not only on CI's Node 20.** Node ≥ 22 defines `localStorage`/`sessionStorage` on the global as own accessors that return `undefined` without `--localstorage-file`; vitest's jsdom environment leaves keys the global already owns, so Node's dead stub won and 60 tests — every one covering what an operator sees on relaunch — failed on a clean checkout with `Cannot read properties of undefined`. `src/test-setup.js` hands the global a real jsdom Storage when the ambient one is missing. Don't delete it because "jsdom provides that".

- **`e2e.rs` is the one test that exercises what a congregation actually sees.** It drives the real commands (`manual_fire`, `nav`, `clear_screens`, `blackout`, `set_rehearsal`) against a real in-memory DB, through the real router and pipeline, and asserts on the events that leave the machine. Nothing is mocked but the window (`tauri::test::mock_builder`, dev-dependency `tauri/test`). **Add a test here whenever you touch the fire path.**
  - Use `mock_context(noop_assets())`, **not** `generate_context!()` — the real macro embeds `Info.plist` as a link symbol and expanding it twice fails with `_EMBED_INFO_PLIST is already defined`.
  - Template resolution reversed in **DECISIONS §29**: a SCREEN'S OWN template now WINS; a content-type default (a "content look") DEFERS to it, and a cue's deliberate choice is `template_pinned` and overrides. `resolveOutputTemplate(channelTpl, override, pinned)` is the one resolver; the console program pane and the Outputs inspector preview both call it so they cannot disagree with the wall.
  - A content-look default rides as an ID ONLY — it never serializes/broadcasts its template JSON, because it defers to per-screen. This is also a hard PERF rule: a default template carrying an embedded image (`data:` URL) can be MEGABYTES (one was 13 MB), and serializing + broadcasting that on every fire made verses take seconds. Only a PINNED cue template ships its JSON.

- **`eval.rs` is a CI build gate**, not shipped code: a 50-case labelled corpus scored **through the real router**, failing the build above SPEC's 5% wrong-verse rate. It measures detection over TEXT, not accuracy over AUDIO — WER has never been measured, in any language.
- **`ipc.test.js` is the contract test**: every Tauri command the frontend calls by string must exist in Rust. Renaming a `#[tauri::command]` otherwise fails silently inside a `catch {}` and a button just quietly stops working.
- **Test the bug, not the fix.** When fixing something, verify the new test FAILS if you reintroduce the original defect. Several tests in this repo were written that way on purpose; one entitlement test initially passed on a broken file because it grepped a comment.
- **A contract stated in a comment is not a contract.** Both of the gaps this line used to name were fixed by writing the rule down — `error.rs` for typed errors, the throw-vs-swallow groups at the top of `capture.js` — and then only ONE of them was pinned by a test. `stopCapture` sat in the THROWS group, swallowing, for as long as the comment existed: one bare `catch {}` around both the bridge import and the command, so a `stop_capture` that failed on a poisoned audio lock printed "Start listening" over a live microphone and no caller's `catch` could fire. `micstop.test.js` pins it. When you place a wrapper in a group, add the test that holds it there.
- **A test's assertion surface is part of its claim.** `e2e.rs`'s `Wall` listens for Tauri events, so it can only ever see content that leaves through one of the two doors. `channels::stage_next` publishes to the kiosk hub and emits nothing — so the rehearsal guarantee was tested, passing, and false for the preacher's stage tablet the whole time. `nothing_reaches_the_stage_monitor_during_a_rehearsal` watches the hub itself. Anything new that publishes to the kiosk needs the same.
- Vitest gotcha: `beforeEach(() => invoke.mockReset())` returns the mock, and vitest treats a value returned from a hook as a **teardown function** — so it calls `invoke()` after every test. Use a block body.

# gstack

Installed at `~/.claude/skills/gstack`. Run `~/.claude/skills/gstack/setup` after
cloning it if the skills aren't registered yet.

## Web browsing

Use the `/browse` skill from gstack for **all** web browsing. Never use the
`mcp__claude-in-chrome__*` tools.

## Available skills

`/office-hours`, `/plan-ceo-review`, `/plan-eng-review`, `/plan-design-review`,
`/design-consultation`, `/design-shotgun`, `/design-html`, `/review`, `/ship`,
`/land-and-deploy`, `/canary`, `/benchmark`, `/browse`, `/connect-chrome`, `/qa`,
`/qa-only`, `/design-review`, `/setup-browser-cookies`, `/setup-deploy`,
`/setup-gbrain`, `/retro`, `/investigate`, `/document-release`,
`/document-generate`, `/codex`, `/cso`, `/autoplan`, `/plan-devex-review`,
`/devex-review`, `/careful`, `/freeze`, `/guard`, `/unfreeze`,
`/gstack-upgrade`, `/learn`.
