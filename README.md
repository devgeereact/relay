# Relay

> AI-assisted live presentation software for churches — real-time scripture detection routed to independently-styled output screens, built to interoperate with OBS, ATEM, and ProPresenter rather than replace them.

**Status:** builds and runs. Phases 0–4 complete (shell + console UI, SQLite data layer, live audio capture + VAD, local English STT). Detection pipeline is next. Working name — rename freely.

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

The local speech model is a large binary and is **not** committed. Download the
English base model into `models/` before running (the app auto-detects it there;
override with the `RELAY_MODEL_PATH` env var):

```bash
mkdir -p models
curl -L -o models/ggml-base.en.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin
```

Without a model the app still runs — audio capture works, transcript panel shows
"no model", and manual override is fully functional. Larger/multilingual models
(`ggml-small.en`, `ggml-medium`, African-language fine-tunes) drop in the same way.

## Running

```bash
npm install
npm run tauri dev      # opens the desktop app; dev server on http://localhost:5032
```

The operator console (app surface) is pinned to port **5032** (Relay = slot
NN=03 in the global dev-port registry). Audio + STT only work inside the desktop
app window — a plain browser at `:5032` renders the UI but has no Rust backend.

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
