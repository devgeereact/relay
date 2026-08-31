# Relay — Architecture & How It Works

How the application is built and how the pieces fit together, end to end. For the *why* behind decisions see [DECISIONS.md](DECISIONS.md); for the entities, invariants, and event catalog see [DOMAIN_MODEL.md](DOMAIN_MODEL.md); for the visual/interaction system see [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md); for operating the app see [USER_GUIDE.md](USER_GUIDE.md); for the original brief see [SPEC.md](SPEC.md); for what is deferred see [ROADMAP.md](ROADMAP.md). The whole doc hierarchy is indexed in [README.md](README.md).

Relay is **AI-assisted live presentation software for churches**. It listens to a live sermon, detects scripture references (direct quotes *and* paraphrases), and routes the right content to multiple independently-styled output screens in real time — built to sit **above** the AV chain (OBS, ATEM, ProPresenter) over NDI/HDMI/network, not replace it. Everything core runs **fully offline**.

---

## 1. Process model

Relay is a **Tauri v2 desktop app** — one native window per machine.

```
┌─────────────────────────────────────────────────────────────┐
│  Tauri app (one OS process)                                   │
│                                                               │
│   Rust core  ───────────────  Webview (Svelte UI)             │
│   src-tauri/src/*.rs          src/*  (operator console)       │
│        │   ▲                       │  ▲                        │
│        │   │  #[tauri::command]    │  │  invoke()             │
│        │   └───────────────────────┘  │  events (listen)      │
│        │                              │                        │
│   ┌────┴─────────┐            ┌───────┴──────────┐            │
│   │ SQLite (disk)│            │ TemplateRender    │            │
│   │ audio in     │            │ (WYSIWYG output)  │            │
│   └──────────────┘            └───────────────────┘            │
│        │                                                       │
│   ┌────┴───────────────── background servers ────────────┐    │
│   │  Kiosk WebSocket hub  :8031                           │    │
│   │  Output/stage HTTP    :8032  (+ /media/<id>)          │    │
│   └──────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
        │                         │
   native output windows     OBS / vMix / Raspberry-Pi kiosk / phone
   (HDMI displays)           (browser sources over LAN)
```

- The **operator console** is the Svelte UI in the app's main webview. It talks to the Rust core through Tauri **commands** (request/response) and **events** (push).
- **Output surfaces** are render targets of one shared template engine:
  - **Native windows** — fullscreen on attached HDMI displays, driven by Tauri events.
  - **Networked clients** — OBS/vMix browser sources, Raspberry-Pi kiosks, and the preacher's phone, driven by the **kiosk WebSocket hub**. The HTML pages themselves are served by the embedded HTTP server.
- `localhost:5032` is the **Vite dev/app surface** (the webview + browser output pages). Opening it in a plain browser shows a dead console — only the app window has the Rust backend attached. The output/stage pages, however, work in any browser because they get their state over the WebSocket hub.

### Ports

| Port | Purpose |
|------|---------|
| `5032` | Operator console (Vite, `strictPort`) + browser output/stage pages |
| `8031` | Kiosk/OBS **WebSocket** hub — live content state to networked clients |
| `8032` | Embedded **HTTP** server — serves `output.html` / `stage.html` and `/media/<id>` asset files |

---

## 2. The live pipeline

The core loop, fully local:

```
mic ─▶ audio.rs ─▶ stt.rs ─┊─▶ detection.rs ─▶ router.rs ─▶ pipeline.rs ─▶ channels.rs ─▶ outputs
      capture+VAD  whisper  ┊   direct+semantic  confidence   Fire +         broadcast     every screen
      +overlap     +resample┊   +context memory  gating       preflight      (event + WS)  (own template)
      chunker      +language┊                    +debounce    (DECISIONS §42)
                            ┊
              the `relay-detect` thread boundary (CLAUDE.md rule 33):
              a bounded queue, a shed PARTIAL is counted, a FINAL never dropped
```

`latency.rs` stamps **nine named instants per decode pass on one monotonic clock** and carries a
trace id from the microphone to the projector, across every box above. A stage never reached is
recorded as an **absence**, never as a zero.

1. **`audio.rs`** — `cpal` microphone capture on a dedicated thread. A voice-activity gate drops silence; an **overlapping chunker** emits ~50%-overlapping windows so a reference spanning a chunk boundary is never lost. `dsp.rs` adds noise suppression, auto-gain, and quality metrics (`audio://quality`). Capture start is **non-blocking** — the command spawns the thread and returns immediately; device errors come back on `audio://error`.
2. **`stt.rs`** — a `whisper.cpp`-class local model (`whisper-rs`) transcribes on a **big-stack worker thread** (16 MB — `whisper_full()` is stack-hungry). It is fed the **non-overlapping tail** of each chunk (feeding the overlapping chunks verbatim garbles whisper). Multilingual, tuned first for **Yoruba, Swahili, Hausa + English**, with **code-switching as the normal case**. Emits `stt://transcript`.
3. **`detection.rs`** — pure, DB/IO-free, heavily unit-tested. Turns text into verse references three ways:
   - **Direct** — book aliases (full names, numbered `1 John`/`first john`/`1jn`, fast abbreviations `ps 23 1`, ASR mishears like `sam`→Psalms), a spoken-number FSM (`three sixteen`→3:16), single-chapter books (`Jude 4`→1:4), ambiguity handling (`revelation 22`→suggests 22:1 *and* 2:2).
   - **Semantic** — a TF-IDF `SemanticIndex.top_k` turns paraphrases ("there is therefore no condemnation…") into the real verse (Romans 8:1). This is the seam where a neural embedder will later drop in.
   - **Context memory** — recent passages bias interpretation and enable "next"/"back" navigation.
4. **`router.rs`** — confidence gating with **self-calibrating thresholds** (config, not hardcoded — seed `0.50`/`0.35` at sensitivity 50, the single baseline `Thresholds::default()`; see the "Confidence-threshold mechanism" row under *Build-out decisions* in [DECISIONS.md](DECISIONS.md) — the numbered log starts at §18, and this decision predates it), debounce, and the decision of what actually fires. High-confidence auto-fires; mid-confidence becomes an operator *suggestion*; low is dropped. Only `Direct` matches may auto-fire (see [DOMAIN_MODEL.md](DOMAIN_MODEL.md) §6).
5. **`pipeline.rs`** — the ONE place a verse becomes screen content. `pipeline::Fire` is the only way an `OutputContent` or a `DetectionEvent` is built (five hand-rolled copies once drifted apart and two silently dropped the scripture template), and `pipeline::preflight` is the **pre-air validator**: it refuses a payload that would paint an empty screen, or one carrying a template the output page cannot parse, and leaves the screens exactly as they were ([DECISIONS.md](DECISIONS.md) §42). It lives at the one choke point, so the AI path, the manual box, spoken nav, plan cues, media, announcements and the countdown are covered at once — and the panic controls deliberately do not pass through it, because a validator that could refuse a blackout is a blackout that can fail.
6. **`channels.rs`** — one `broadcast_content()` pushes the chosen content to **every** output: a Tauri event (`output://content`) for native windows **and** a JSON frame over the kiosk WS for networked clients. N independently-styled renders from one broadcast; the pipeline never formats per channel. It also owns **`OutputHealth`**: every output page beats back that it is still painting — the native window over the bridge, kiosk/OBS over the socket it already has — so a screen that has gone away can be *detected* rather than assumed. The beat is anonymous by construction: it says "the screen for channel N painted", never who or from where ([DECISIONS.md](DECISIONS.md) §39).

**Operator override is first-class**, never a fallback — reachable in one action at every stage. Manual fire, confirm/dismiss suggestion, prev/next nav, clear, and blackout all go through the same broadcast path.

---

## 3. The cue model (unified content)

Everything that can go on screen is a **cue** — one polymorphic abstraction, so there is never per-type rendering logic downstream.

A cue is a row in `plan_items`: `{ cue_type, label, payload_json, template_id }`. Five `cue_type`s:

| Cue type | Payload (JSON) | Fires via | Notes |
|----------|----------------|-----------|-------|
| `scripture` | `{book, chapter, verse, reference, text, translation}` | `manual_fire` (re-resolves live) | AUTO-DETECT; live-resolved at fire time |
| `song` | `{song_id, title, sections[], arrangement_name, arrangement_seq}` | `fire_content(kind:"song")` | snapshot; edits propagate; arrangements |
| `media` | `{media_id, kind, filename}` | `fire_media` | full-screen image/video background |
| `announce` | `{announce_id, title, body}` | `fire_content(kind:"announce")` | snapshot; edits propagate |
| `countdown` | `{minutes, label, done}` | `start_countdown` | pre-service timer, ticks locally |

**Snapshot vs. reference.** Scripture live-resolves from its reference at fire time (always current translation/text). Songs and announcements **snapshot** their content into the cue when added, but edits **propagate**: `sync_song_in_plans` / `sync_announcement_in_plans` rewrite every matching cue when the source is edited (songs re-expand through the cue's stored arrangement sequence).

**Arrangements** (songs) are named play-orders of a song's sections, repeats allowed (`V1 C V2 C`). Stored in `song_arrangements` as a sequence of section indices. "Standard" (all sections in order) is implicit, never stored. Adding an arranged song to a plan **expands** the sequence into the snapshot; the stored `arrangement_seq` lets edits re-expand correctly.

---

## 4. Output & rendering

**One renderer for everything**: `src/lib/TemplateRender.svelte` draws both the fullscreen output *and* the Templates-editor preview, guaranteeing **WYSIWYG** — what you save is exactly what shows.

- **`cqw` sizing** (container-query width units) — a template scales identically at any output size (full screen, a 4-up planner monitor, a small preview).
- **Auto-fit** — after every render and on container resize, the verse/reference shrink until the content box no longer overflows, so scripture is **never clipped and never spills off screen** at any size. Font-size is set *imperatively* on the element (not via a reactive var) so it can't re-enter Svelte's scheduler and loop.
- **Transparent output page** — a transparent-background template keys out for OBS/ATEM camera-over.
- **Crossfade transitions** — `{#key}` + `transition:fade`, duration is template config (`style.transitionMs`); reduced-motion users get an instant cut.
- **Lower-third band** — a *template* choice (like ProPresenter's "Lower 3rd" templates), not a content choice. Lyrics on a lower-third template render in the band, centered, never floating mid-screen.
- **Per-content-type templates** — a cue can carry a `template_json` override (lyrics use the lyric template, scripture the scripture template); the output honors it, else the channel's own template. Still one renderer — the override is just data.
- **No on-screen chrome for the congregation** — titles, section labels, and slide numbers stay in the operator UI and on stage/confidence monitors, never on the main output.

**Countdown** rides the same renderer: `start_countdown` broadcasts a target epoch once; each output **ticks MM:SS locally** (a client-side interval), so there is zero per-second network traffic and the digits update in place without re-keying (no crossfade per tick). At zero the "begins in" label drops and only the done message ("Welcome") shows. A one-at-a-time guard prevents a second countdown starting while one runs.

`OutputContent` (the broadcast payload) carries: `reference, text, translation, media_url, media_kind, template_id, template_json, stage_note, countdown_to, countdown_done`. All render surfaces read from this one struct.

### Output surfaces

- **`src/Output.svelte`** → `output.html` — the fullscreen page. Two modes, one renderer: **desktop** (Tauri events + DB template, live edits) and **kiosk/OBS** (built-in template by id, state over the WS hub). Transparent by default.
- **`src/Stage.svelte`** → `stage.html` — the preacher's phone/iPad confidence monitor. Connects to the WS hub, shows the live verse/reference large and readable, plus an **"up next"** preview, the operator's **stage note** for the current cue, and the countdown — information deliberately kept *off* the congregation screen.

---

## 5. Data layer

SQLite via `rusqlite` (bundled), at `~/Library/Application Support/com.relay.app/relay.db` (macOS). `db::open()` runs **idempotent, forward-filling migrations** so an existing DB self-heals on launch — new tables, template resets, full-Bible reimport, KJV gloss re-clean, and FTS index (re)build all happen automatically.

### Tables

| Table | Holds |
|-------|-------|
| `translations`, `verses` | Bible corpus (full KJV bundled, 66 books / 31,100 verses) |
| `verses_fts` | FTS5 index over `verses` (external-content, `porter unicode61`) |
| `templates` | Output templates (region + style JSON); `is_active` for the console's 4 monitors |
| `output_channels` | Configured render targets (native window / network client) |
| `service_plans`, `plan_items` | Service Planner: a plan and its ordered cues |
| `songs`, `song_sections`, `song_arrangements` | Lyrics library + named play-orders |
| `saved_scripture` | Verses the operator saved to the Library |
| `media_assets` | Imported image/video/document pointers (files on disk) |
| `announcements` | Notice slides (title + body) |
| `services`, `cues`, `transcripts`, `detections` | Service-session history (what was said, detected, fired) |
| `service_events` | The ordered service record — an append-only timeline that survives the app. **It carries nothing a preacher said**: `detail` is a phrase Relay composes, and that is pinned from both sides |
| `perf_samples` | Latency that survives a quit — percentiles, never traces. Written by the engine; read through `service_events` |
| `environment_profiles` | A room, remembered: microphone, language, service length, voice profile, displays, and the two numbers legibility needs. **Not** the audio levels ([DECISIONS.md](DECISIONS.md) §46) |
| `voice_profiles` | Per-operator STT/threshold profiles |
| `app_settings` | Key/value (active translation, per-content-type template map, …) |

Count them with `grep -c 'CREATE TABLE' docs/data/schema.sql` — the number is deliberately not
written here, because every count restated in prose in this repository has drifted
([RELAY_GAP.md](RELAY_GAP.md) §18). `docs/data/schema.sql` is not documentation: `db/mod.rs`
`include_str!`s it, so it **is** the baseline schema the binary ships, and
`docs/data/schema-baseline.sql` is the oldest schema Relay can upgrade from — checked in so a
test can prove every column added since has a migration behind it.

### Scripture search (`search_scripture`)

Type a word, a phrase, or a paraphrase → the verse plus ranked suggestions. Candidates are scored and merged, best first:

1. **Explicit references** (`john 3:16`, `ps 23`) — score 1.0
2. **Exact phrase** (verbatim substring) — 0.95
3. **Semantic paraphrase** (TF-IDF `top_k`) — 0.5–0.9 band — turns "the lord is my shepherd"-style paraphrases into the real verse
4. **FTS5 full-text** (`search_verses_fts`) — bm25-ranked, terms quoted then OR'd — catches loose, non-contiguous word queries a substring `LIKE` misses. 0.33–0.45 band.
5. Substring `LIKE` as a last-ditch fallback.

The KJV importer strips translator **marginal glosses** (`{green…: Heb. pastures of tender grass}` — not verse text) while keeping supplied-word italics (`{it was}` → `it was`).

---

## 6. Command & event reference

Frontend↔core contract. Commands are `invoke()` (camelCase JS args → snake_case Rust); events are `listen()`.

> **The authoritative list is the code, and this section deliberately does not restate it.** An
> earlier version of this page listed nine commands that no longer exist — `lookup_verse`,
> `create_template`, `import_song`, `import_pro`, `open_output_window`, `close_output_window`,
> `list_output_windows`, `current_service` and the `*_template_active` pair — every one deleted
> because nothing could reach it, which is a security reduction as much as a tidy-up
> ([RELAY_GAP.md](RELAY_GAP.md) RG-51). A list of commands in prose is the single fastest-rotting
> thing in this repository. Read it live:
>
> ```bash
> grep -n '#\[tauri::command\]' -A1 src-tauri/src/main.rs   # every command, in order
> node scripts/qa-inventory.mjs                              # and which rendered control reaches it
> ```
>
> Two tests keep the contract honest, and between them they mean the list cannot silently rot:
> `ipc.test.js` fails if the frontend calls a command Rust does not register (or the reverse), and
> `qa-inventory.mjs` traces one hop further — to a control something actually renders.

**Areas, which do not rot:** audio & STT · detection & routing · scripture search · outputs and
channels · templates and themes · planner · lyrics and arrangements · library (saved scripture,
announcements, media) · service history and the service record · voice profiles · rooms ·
service lock · update safety · diagnostics · models.

**Events (core → UI/outputs):**

| Event | Carries |
|---|---|
| `audio://chunk` · `audio://quality` · `audio://error` | Throttled level meter · clipping/quiet/noise assessment · a device failure, because capture start is non-blocking |
| `stt://transcript` | A whole-window transcript with `is_final`. **Deliberately one event, not two** — splitting it would be a second vocabulary for the same fact |
| `stt://language_unstable` | Auto language detection is flapping, which the operator should know before blaming the AI |
| `detection://match` | A candidate, with `matched_text` and `method` — so the operator can see *which kind* of claim is being made ([DECISIONS.md](DECISIONS.md) §21) |
| `output://content` · `output://clear` · `output://black` | The three things a screen can be told |
| `output://panic_failed` | A panic control that did **not** achieve what it claimed ([DECISIONS.md](DECISIONS.md) §20) |
| `nav://blocked` | A nav that could not move, and which of the four reasons it was |
| `template://updated` | A template changed; every surface re-renders from one engine |
| `model://progress` · `done` · `error` · `cancelled` | The in-app STT model download. **`done` has no listener on purpose** — `download_model` resolves when the file is installed and verified, so the command's own return *is* the completion signal; a listener as well would handle it twice |
| `channel://retemplate` | A screen's template was reassigned. The native output filters it by its own `channel` id, which is why a template swap is live and needs no new URL (DECISIONS §29) |
| `rehearsal://changed` | Rehearsal was turned on or off. Pushed rather than polled, because every surface must agree about it at the same instant |

Networked clients get the content events as JSON frames over the WS hub
(`{kind:"content"|"clear"|"black"|"stage_next"|"channel_template", …}`), and send exactly three
kinds back — `hello`, `beat`, `rendered` — none of which can carry content
([SECURITY.md](../SECURITY.md) T4).

---

## 7. Frontend shape

- **One store** — `src/lib/stores/capture.js` — holds all writable stores (`capture`, `transcript`, `detections` = pending suggestions, `live` = what's on screen, `templates`, `screenBlack`, `panicError`, `serviceLock`) plus every command wrapper and event listener. The file's header states which wrappers **throw** and which **swallow**, and a test holds each one in its group — a contract stated only in a comment was false for `stopCapture` for as long as the comment existed.
- **`TemplateRender.svelte`** is the single renderer (see §4).
- **Tabs** — **Live · Outputs · Templates · Themes · Library · Planner · Settings · Help.** There is **no Console tab**: `Live` *is* the console, and the plan runs there, because an operator running a plan on a separate tab could not see the AI's suggestions — and the preacher going off-script is the entire product. (The Outputs tab's internal key is still `channels` and its file is `Channels.svelte`; the label is what an operator reads.)
- **Sub-surfaces** — `Library` (Scripture / Lyrics / Media / Announcements / History, plus `SongEditor`, `ImportReview`, the arrangement editor and the Sunday report), `Settings` (18 sections, including **Dashboard** — the readiness screen, which is inside Settings and not on the tab bar — **Languages**, **Privacy** and **Diagnostics**); plus standalone `Output` and `Stage` pages.
- **Cross-cutting shell state** — the panic bar, the rehearsal band, the update banner, and the one-line **degraded** state are mounted once in `App.svelte`, on every tab, never per view. So is `shortcuts.js`, the single global keydown listener.
- **Design system** — global `--v-*` tokens in `src/app.css`; every view shares them, and the four promise-carrying colours are defined once (amber = on air, amethyst = rehearsal, cyan = a guess, grey = cued).

---

## 8. Invariants — the rules that keep it from breaking

These were learned the hard way (hours-long freezes/crashes). Do not regress them.

1. **Never call `tick()` inside a reactive `$:` block** (Svelte) — it re-enters the scheduler and hard-freezes the webview. Use `afterUpdate` for DOM side-effects; use `setInterval` (not `tick`) for clocks.
2. **Never hold a `Mutex` lock across `emit` / `broadcast_content`** on a background thread — deadlocks the macOS main run loop. Compute under lock, release, *then* emit.
3. **The STT worker thread needs a big stack** (16 MB) — `whisper_full()` overflows the default 2 MB → silent SIGSEGV.
4. **Call `whisper_rs::install_logging_hooks()` once** — else whisper floods stderr with per-token lines (an I/O storm that looks like a freeze).
5. **Audio capture start must be non-blocking** — spawn the thread, return immediately, surface device errors via `audio://error`.
6. **Consistent global lock order: `Db` before `Session`** everywhere.
7. **`initAudio()` at app level** (not only in Settings) so `$capture.available` is true on the default **Live** tab.
8. **Feed STT the non-overlapping tail** of each chunk (the detection chunker's overlap garbles whisper).
9. **No `unwrap()` in live paths** — a panic mid-sermon is the worst failure. Surface errors to the operator.
10. **No per-channel-type rendering branches** — output differences are template *configuration*, never `if channel_type == …` in render code.
11. **No native `confirm()`/`alert()`** — Tauri's webview doesn't implement them (returns false). Use in-app two-step confirmations.

---

## 9. Parked / honest limits

Not faked — clearly bounded. The full deferral + technical-debt register is [ROADMAP.md](ROADMAP.md); the highlights:

- **NDI output** — needs the proprietary SDK; `open_ndi_output` returns a clear error. NDI + HDMI only; **no native SDI** (served by existing ATEM/converter hardware).
- **Neural paraphrase embedder** — TF-IDF is the current seam behind `SemanticIndex::top_k`.
- **African-language STT fine-tunes** — base multilingual model is weak on Yoruba/Hausa; fine-tunes pending.
- **Document (PDF/PPTX) rendering** — stored as media pointers; slide extraction/presentation is a later phase.
- **Detection-history writes** are service-session scoped.
- **Signed language packs** — refused rather than deferred: signing needs a key, a ceremony and a distribution channel that do not exist, and an unsigned pack that can rewrite the book aliases is a wrong-verse-on-a-wall vector ([SECURITY.md](../SECURITY.md) T9). The *offline* half shipped: install a model from a file, and `scripts/offline-bundle.mjs`.
- **Binary update rollback** — deliberately not built. The installers are public and signed; what cannot be got back is the church's database, and that is what `updates.rs` protects ([DECISIONS.md](DECISIONS.md) §43).
- **Device identity on the LAN** — declined, not missing. `:8032` is an unauthenticated control plane by decision, because the preacher's phone has no way to hold a credential ([DECISIONS.md](DECISIONS.md) §35, narrowed by §39 to record *when* a screen painted and never *who*).

### Supporting modules, and the single question each answers

Not on the fire path, and each exists because something failed in front of people:

| Module | The one question |
|---|---|
| `servicelock.rs` | What may **not** happen while a service is recording? (16 actions; nothing on the fire path) |
| `updates.rs` | Is this update safe to attempt, and what is the way back? (the data, not the binary) |
| `diagnostics.rs` | What can a church send when something went wrong? (one file, composed as an allow-list) |
| `wake.rs` | Is a screen or a microphone live, and should the display therefore stay up? |
| `latency.rs` | Where did the time actually go, microphone to projector? |
| `sysprobe.rs` | Can this machine run this model? (advisory only — nothing branches on it) |
| `error.rs` | Is pressing it again worth the operator's time? (the one typed error across the bridge) |
| `telemetry.rs` | Opt-in, scrubbed, no DSN in an OSS build |
| `eval.rs` | Did detection quality regress? (a CI build gate over a labelled corpus, scored through the real router) |
| `qa.rs` · `qa_r5.rs` · `r6.rs` · `e2e.rs` | Test-only. `qa::bare_app()` is **the** fixture — a fresh install and nothing else |
