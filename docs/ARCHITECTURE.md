# Relay — Architecture & How It Works

How the application is built and how the pieces fit together, end to end. For the *why* behind decisions see [DECISIONS.md](DECISIONS.md); for operating the app see [USER_GUIDE.md](USER_GUIDE.md); for the original brief see [SPEC.md](SPEC.md).

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
mic ─▶ audio.rs ─▶ stt.rs ─▶ detection.rs ─▶ router.rs ─▶ channels.rs ─▶ outputs
      capture+VAD  whisper    direct+semantic  confidence   broadcast    every screen
      +overlap     +resample  +context memory  gating       (event + WS)  (own template)
      chunker      +language                   +debounce
```

1. **`audio.rs`** — `cpal` microphone capture on a dedicated thread. A voice-activity gate drops silence; an **overlapping chunker** emits ~50%-overlapping windows so a reference spanning a chunk boundary is never lost. `dsp.rs` adds noise suppression, auto-gain, and quality metrics (`audio://quality`). Capture start is **non-blocking** — the command spawns the thread and returns immediately; device errors come back on `audio://error`.
2. **`stt.rs`** — a `whisper.cpp`-class local model (`whisper-rs`) transcribes on a **big-stack worker thread** (16 MB — `whisper_full()` is stack-hungry). It is fed the **non-overlapping tail** of each chunk (feeding the overlapping chunks verbatim garbles whisper). Multilingual, tuned first for **Yoruba, Swahili, Hausa + English**, with **code-switching as the normal case**. Emits `stt://transcript`.
3. **`detection.rs`** — pure, DB/IO-free, heavily unit-tested. Turns text into verse references three ways:
   - **Direct** — book aliases (full names, numbered `1 John`/`first john`/`1jn`, fast abbreviations `ps 23 1`, ASR mishears like `sam`→Psalms), a spoken-number FSM (`three sixteen`→3:16), single-chapter books (`Jude 4`→1:4), ambiguity handling (`revelation 22`→suggests 22:1 *and* 2:2).
   - **Semantic** — a TF-IDF `SemanticIndex.top_k` turns paraphrases ("there is therefore no condemnation…") into the real verse (Romans 8:1). This is the seam where a neural embedder will later drop in.
   - **Context memory** — recent passages bias interpretation and enable "next"/"back" navigation.
4. **`router.rs`** — confidence gating with **self-calibrating thresholds** (config, not hardcoded — seed `0.90`/`0.60`), debounce, and the decision of what actually fires. High-confidence auto-fires; mid-confidence becomes an operator *suggestion*; low is dropped.
5. **`channels.rs`** — one `broadcast_content()` pushes the chosen content to **every** output: a Tauri event (`output://content`) for native windows **and** a JSON frame over the kiosk WS for networked clients. N independently-styled renders from one broadcast; the pipeline never formats per channel.

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
| `voice_profiles` | Per-operator STT/threshold profiles |
| `app_settings` | Key/value (active translation, per-content-type template map, …) |

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

**Events (core → UI/outputs):** `audio://chunk` (throttled level meter), `audio://quality`, `audio://error`, `stt://transcript`, `detection://match`, `output://content`, `output://clear`, `output://black`, `template://updated`. Networked clients get the equivalent as JSON frames over the WS hub (`{kind:"content"|"clear"|"black"|"stage_next", …}`).

**Commands by area:**

- *Audio / STT:* `list_audio_devices`, `start_capture`, `stop_capture`, `stt_status`, `set_stt_language`
- *Detection / routing:* `set_detection_enabled`, `get_detection_enabled`, `confirm_detection`, `dismiss_detection`, `get_thresholds`, `set_thresholds`, `manual_fire`, `nav`, `related_scripture`, `verse_repeat_count`
- *Scripture / search:* `lookup_verse`, `search_scripture`, `list_translations`, `get_active_translation`, `set_active_translation`
- *Output / channels:* `clear_screens`, `blackout`, `start_countdown`, `set_stage_next`, `open_output_window`, `close_output_window`, `list_output_windows`, `list_output_channels`, `add_channel`, `delete_channel`, `set_channel_template`, `set_channel_display`, `open_channel_output`, `list_monitors`, `local_ip`
- *Templates:* `list_templates`, `list_active_templates`, `set_template_active`, `create_template`, `delete_template`, `get_template`, `save_template`, `get_content_templates`, `set_content_template`
- *Planner:* `list_plans`, `create_plan`, `delete_plan`, `duplicate_plan`, `plan_items`, `add_plan_item`, `remove_plan_item`, `move_plan_item`, `reorder_plan`, `set_plan_note`
- *Lyrics:* `list_songs`, `search_songs`, `get_song`, `import_song`, `save_song`, `delete_song`, `import_pro`, `parse_import`, `save_reviewed_songs`, `list_arrangements`, `save_arrangement`, `delete_arrangement`
- *Library (other):* `list_saved_scripture`, `save_scripture`, `delete_saved_scripture`, `list_announcements`, `save_announcement`, `delete_announcement`, `list_media`, `import_media`, `delete_media`, `push_announcement`
- *Service history:* `start_service`, `end_service`, `current_service`, `list_services`, `service_detail`, `export_service`, `data_health`
- *Voice profiles:* `list_voice_profiles`, `active_voice_profile`, `create_voice_profile`, `update_voice_profile`, `select_voice_profile`, `delete_voice_profile`

---

## 7. Frontend shape

- **One store** — `src/lib/stores/capture.js` — holds all writable stores (`capture`, `transcript`, `detections` = pending suggestions, `live` = what's on screen, `templates`, `screenBlack`) plus every command wrapper and event listener.
- **`TemplateRender.svelte`** is the single renderer (see §4).
- **Views** — `Console`, `Library` (Scripture / Lyrics / Media / Announcements / History sub-tabs + `SongEditor`, `ImportReview`), `ServicePlanner`, `Channels`, `Templates`, `Settings`; plus standalone `Output` and `Stage` pages.
- **Design system** — global `--v-*` tokens in `src/app.css`; all views (Console included) share them.

---

## 8. Invariants — the rules that keep it from breaking

These were learned the hard way (hours-long freezes/crashes). Do not regress them.

1. **Never call `tick()` inside a reactive `$:` block** (Svelte) — it re-enters the scheduler and hard-freezes the webview. Use `afterUpdate` for DOM side-effects; use `setInterval` (not `tick`) for clocks.
2. **Never hold a `Mutex` lock across `emit` / `broadcast_content`** on a background thread — deadlocks the macOS main run loop. Compute under lock, release, *then* emit.
3. **The STT worker thread needs a big stack** (16 MB) — `whisper_full()` overflows the default 2 MB → silent SIGSEGV.
4. **Call `whisper_rs::install_logging_hooks()` once** — else whisper floods stderr with per-token lines (an I/O storm that looks like a freeze).
5. **Audio capture start must be non-blocking** — spawn the thread, return immediately, surface device errors via `audio://error`.
6. **Consistent global lock order: `Db` before `Session`** everywhere.
7. **`initAudio()` at app level** (not only in Settings) so `$capture.available` is true on the default Console tab.
8. **Feed STT the non-overlapping tail** of each chunk (the detection chunker's overlap garbles whisper).
9. **No `unwrap()` in live paths** — a panic mid-sermon is the worst failure. Surface errors to the operator.
10. **No per-channel-type rendering branches** — output differences are template *configuration*, never `if channel_type == …` in render code.
11. **No native `confirm()`/`alert()`** — Tauri's webview doesn't implement them (returns false). Use in-app two-step confirmations.

---

## 9. Parked / honest limits

Not faked — clearly bounded:

- **NDI output** — needs the proprietary SDK; `open_ndi_output` returns a clear error. NDI + HDMI only; **no native SDI** (served by existing ATEM/converter hardware).
- **Neural paraphrase embedder** — TF-IDF is the current seam behind `SemanticIndex::top_k`.
- **African-language STT fine-tunes** — base multilingual model is weak on Yoruba/Hausa; fine-tunes pending.
- **Document (PDF/PPTX) rendering** — stored as media pointers; slide extraction/presentation is a later phase.
- **Detection-history writes** are service-session scoped.
