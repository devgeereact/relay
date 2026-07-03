# Relay

> AI-assisted live presentation software for churches — real-time scripture detection routed to independently-styled output screens, built to interoperate with OBS, ATEM, and ProPresenter rather than replace them.

**Status:** builds and runs, full pipeline end to end. Phases 0–10 complete — shell + 5-screen console, SQLite data layer, audio capture + VAD, local STT (multilingual + code-switching), direct + semantic + context-memory detection, confidence-gating router with manual override, output channels (native fullscreen + kiosk WebSocket; NDI parked on the external SDK), local service-session history, and the full KJV corpus (66 books, ~31k verses, bundled offline). Next: African-language STT fine-tunes, neural paraphrase embedder, real-service hardening. Working name — rename freely.

## Start here

1. `CLAUDE.md` — working conventions and non-negotiable constraints, read this first if you're using an AI coding agent in this repo.
2. `PROMPT.md` — the full project brief and suggested build-phase order.
3. `docs/SPEC.md` — canonical technical spec.
4. `docs/DECISIONS.md` — every major decision, with reasoning.
5. `docs/design/` — visual mockups. Open the `.html` files directly in a browser; no build step needed.

## Tech stack

Rust core · Tauri v2 shell · Svelte + Vite frontend · SQLite (`rusqlite`) · WebSocket (`tokio-tungstenite`) · local whisper.cpp-class STT with optional cloud fallback · NDI SDK via Rust FFI. Windows + macOS, both from day one. MIT licensed.

## Prerequisites

- Rust (stable) + Cargo
- Node.js + npm
- Tauri CLI (`cargo install tauri-cli` or `npm install -D @tauri-apps/cli`)
- Platform build tools ([Tauri prerequisites](https://tauri.app/start/prerequisites/) — WebView2 on Windows, Xcode command line tools on macOS)
- **CMake** — required to build `whisper-rs` (compiles whisper.cpp). `brew install cmake`, or download from [cmake.org](https://cmake.org/download/) and put it on your PATH.
- NDI SDK (for NDI render-target work, later phase)

## STT model (offline speech-to-text)

The local speech model is a large binary and is **not** committed. Download a
model into `models/` before running (auto-detected there; the **multilingual**
`ggml-base.bin` is preferred so Yoruba/Swahili/Hausa + code-switching work,
falling back to `ggml-base.en.bin`; override with `RELAY_MODEL_PATH`):

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
- **Kiosk / network client** — a LAN browser (e.g. a $50 Raspberry Pi) points at
  `http://<app-host>:5032/output.html?template_id=<n>` and receives state over
  the WebSocket hub on **port 8031**. Live now. (Serve the dev page to the LAN
  with `vite --host`, or a static host of `dist/` in production.)
- **NDI encode** — into OBS/vMix/ATEM/ProPresenter. **Not yet available:**
  requires the proprietary NDI SDK (native lib + FFI). The command returns a
  clear error; integration path is documented in `src-tauri/src/main.rs`
  (`open_ndi_output`) and docs/SPEC.md §9.

## Repo structure

```
CLAUDE.md
README.md
PROMPT.md
LICENSE
docs/
  SPEC.md
  DECISIONS.md
  data/schema.sql
  design/               -- open the .html files in a browser
src-tauri/               -- Rust core + Tauri backend
  src/
    main.rs
    audio.rs
    stt.rs
    detection.rs
    router.rs
    channels.rs
    db.rs
src/                      -- Svelte frontend
```

## Design principles worth re-reading before you build

- Output channels are render targets of one shared template engine — never special-case per channel type.
- The operator override is a first-class control, not a fallback.
- Offline-first, always. Cloud is optional, never required.
- African-language STT (Yoruba, Swahili, Hausa) is a v1 priority, not a stretch goal.
