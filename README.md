# Relay

> AI-assisted live presentation software for churches — real-time scripture detection routed to independently-styled output screens, built to interoperate with OBS, ATEM, and ProPresenter rather than replace them.

**Status:** builds and runs, full pipeline end to end. Operator console, SQLite data layer, audio capture + VAD, local STT (multilingual + code-switching), direct + semantic + context-memory detection, confidence-gating router with first-class manual override, and output channels (native fullscreen + kiosk/OBS WebSocket + preacher stage remote; NDI parked on the external SDK).

Built out into a lightweight presentation suite: a **Content Library** (saved scripture, songs with a slide-flow editor + named arrangements, media, announcements, service history), a **Service Planner** (Mission-Control run editor over a unified cue model — scripture, song, media, announcement, countdown — with drag-reorder, per-cue stage notes, and plan duplication), **FTS5 + semantic scripture search**, **per-content-type templates** with a WYSIWYG editor, verse **auto-fit**, crossfade transitions, blackout, a **pre-service countdown timer**, and the full KJV corpus (66 books, 31,100 verses, bundled offline, translator glosses stripped). Next: African-language STT fine-tunes, neural paraphrase embedder, NDI, document (PDF/PPTX) presentation, real-service hardening. Working name — rename freely.

## Start here

0. **[docs/README.md](docs/README.md)** — the documentation index: the whole spec mapped as a professional hierarchy, with a "start here" path for whoever you are (engineer / operator / contributor / designer).
1. **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** — how the app works, in detail (process model, pipeline, cue model, rendering, data layer, command/event reference, invariants).
2. **[docs/USER_GUIDE.md](docs/USER_GUIDE.md)** — how to operate it: every screen and the typical Sunday flow.
3. `CLAUDE.md` — working conventions and non-negotiable constraints; read first if you're using an AI coding agent in this repo.
4. `PROMPT.md` — the build-phase reference (the module docs cite its phase numbers); the full brief now lives in `docs/SPEC.md`.
5. `docs/SPEC.md` — canonical technical spec (original brief). `docs/DECISIONS.md` — every major decision, with reasoning.
6. `docs/design/` — visual mockups. Open the `.html` files directly in a browser; no build step needed.

## Tech stack

Rust core · Tauri v2 shell · Svelte + Vite frontend · SQLite (`rusqlite`) · WebSocket (`tokio-tungstenite`) · local whisper.cpp-class STT with optional cloud fallback · NDI SDK via Rust FFI. Windows + macOS, both from day one. MIT licensed.

## Prerequisites

- Rust (stable) + Cargo
- Node.js + npm
- Tauri CLI (`cargo install tauri-cli` or `npm install -D @tauri-apps/cli`)
- Platform build tools ([Tauri prerequisites](https://tauri.app/start/prerequisites/) — WebView2 on Windows, Xcode command line tools on macOS)
- **CMake** — required to build `whisper-rs` (compiles whisper.cpp). `brew install cmake`, or download from [cmake.org](https://cmake.org/download/) and put it on your PATH.
- NDI SDK (for NDI render-target work, later phase)

## Privacy, security, and what the AI does

- **[PRIVACY.md](PRIVACY.md)** — *nothing you say, sing or show leaves your computer.* No accounts, no cloud, no server. The audio is never even saved.
- **[SECURITY.md](SECURITY.md)** — how to report a vulnerability, and what we consider most serious (anything that leaks sermon content, or puts content on a screen the operator didn't choose).
- **[docs/AI_DISCLOSURE.md](docs/AI_DISCLOSURE.md)** — what the AI decides by itself, what it will **never** do (a paraphrase never reaches a congregation without a human agreeing), and where it is honestly weak.
- **[docs/LANGUAGES.md](docs/LANGUAGES.md)** — Yorùbá / Kiswahili / Hausa. Fix a book name in a one-line PR, no Rust required.

## STT model (offline speech-to-text)

**Users don't need this section — Relay downloads the model for you, with one
button, on first run.** This is for developers running from source.

The local speech model is a large binary and is **not** committed. Either let the
app fetch it, or download one into `models/` yourself (auto-detected there; the
**multilingual** `ggml-base.bin` is preferred so Yoruba/Swahili/Hausa +
code-switching work, falling back to `ggml-base.en.bin`; override with
`RELAY_MODEL_PATH`):

```bash
mkdir -p models
# Multilingual (recommended — enables the tier-1 African languages):
curl -L -o models/ggml-base.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin
# English-only fallback (smaller quality edge on English):
curl -L -o models/ggml-base.en.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin
```

Recognition language is set in Settings (Auto-detect / English / Yoruba /
Swahili / Hausa); Auto handles code-switching. **Note:** base multilingual has
weak Yoruba/Hausa accuracy — production quality needs community fine-tunes
(Masakhane, Common Voice), which drop in by filename. Without any model the app
still runs audio-only (transcript shows "no model"; manual override works).

## Running

```bash
npm install
npm run tauri dev      # opens the desktop app; dev server on http://localhost:5032
```

The operator console (app surface) is pinned to port **5032** (Relay = slot
NN=03 in the global dev-port registry). Audio + STT only work inside the desktop
app window — a plain browser at `:5032` renders the UI but has no Rust backend.

## Render targets (output channels)

One shared template engine renders to three target types (docs/SPEC.md §5):

- **Native window** — borderless fullscreen webview (HDMI). Open from the
  Channels tab. Live now.
- **Kiosk / network client** — a LAN browser (e.g. a $50 Raspberry Pi) or an
  OBS/vMix **browser source** points at
  `http://<app-host>:8032/output.html?template_id=<n>` and receives live state
  over the WebSocket hub on **port 8031**. Add channels and copy the URL/QR from
  the **Channels** tab. Live now.
- **Preacher stage remote** — `http://<app-host>:8032/stage.html` on a phone or
  iPad: the live verse large + "up next" + operator stage notes + countdown, kept
  off the congregation screen. Uploaded media is served from the same port
  (`/media/<id>`).

> **Windows:** allow Relay through the firewall when Windows asks (tick *Private
> networks*). If you decline, HDMI output still works but **no networked output ever
> can** — and Relay cannot detect this or warn you, because the firewall blocks other
> machines from reaching Relay's servers rather than stopping Relay from starting
> them. See `docs/USER_GUIDE.md`.

> **The output port is 8032, not 5032.** `5032` is the Vite dev server and exists
> **only** while a developer is running `npm run tauri dev`. In the installed app it
> does not exist at all, so an OBS browser source pointed at `:5032` shows a blank
> screen with no error. The embedded HTTP server on **`8032`** serves the output and
> stage pages in both dev and production — and it is what the **Channels** tab's
> Copy URL / QR actually hand you, so prefer those over typing a URL by hand.
- **NDI encode** — into OBS/vMix/ATEM/ProPresenter. **Not yet available:**
  requires the proprietary NDI SDK (native lib + FFI). The command returns a
  clear error; integration path is documented in `src-tauri/src/main.rs`
  (`open_ndi_output`) and docs/SPEC.md §9.

## Repo structure

```
CLAUDE.md  README.md  PROMPT.md  LICENSE
docs/
  ARCHITECTURE.md        -- how it works, in detail
  USER_GUIDE.md          -- how to operate it
  SPEC.md  DECISIONS.md  -- original brief + decision log
  data/schema.sql
  design/                -- open the .html files in a browser
src-tauri/               -- Rust core + Tauri backend
  src/
    main.rs              -- Tauri commands, state, pipeline wiring (composition root)
    audio.rs  dsp.rs     -- cpal capture + VAD + chunker; noise/gain/quality
    stt.rs               -- whisper.cpp STT worker
    detection.rs         -- direct + semantic (TF-IDF) + context memory (DB/IO-free, tested)
    router.rs            -- confidence gating, debounce, self-calibrating thresholds
    channels.rs          -- output render targets: native window + kiosk WS hub + HTTP server
    db.rs                -- SQLite: KJV, FTS5, templates, plans, songs, library, history
    proimport.rs songs.rs-- ProPresenter import; song lyric parsing
data/kjv.json            -- bundled full KJV (include_str!, committed)
src/                      -- Svelte frontend
  Output.svelte Stage.svelte      -- fullscreen output + preacher stage remote pages
  lib/
    TemplateRender.svelte         -- THE one renderer (output + editor preview)
    stores/capture.js             -- all stores + command wrappers + event listeners
    views/  Console Library Planner Channels Templates Settings
    views/library/  Scripture Lyrics Media Announcements History SongEditor ImportReview
models/                   -- STT ggml models (gitignored, per-machine)
```

## Design principles worth re-reading before you build

- Output channels are render targets of one shared template engine — never special-case per channel type.
- The operator override is a first-class control, not a fallback.
- Offline-first, always. Cloud is optional, never required.
- African-language STT (Yoruba, Swahili, Hausa) is a v1 priority, not a stretch goal.
