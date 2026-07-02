# Relay

> AI-assisted live presentation software for churches — real-time scripture detection routed to independently-styled output screens, built to interoperate with OBS, ATEM, and ProPresenter rather than replace them.

**Status:** early scaffold, not yet buildable end-to-end. Working name — rename freely.

## Start here

1. `CLAUDE.md` — working conventions and non-negotiable constraints, read this first if you're using an AI coding agent in this repo.
2. `PROMPT.md` — the full project brief and suggested build-phase order.
3. `docs/SPEC.md` — canonical technical spec.
4. `docs/DECISIONS.md` — every major decision, with reasoning.
5. `docs/design/` — visual mockups. Open the `.html` files directly in a browser; no build step needed.

## Tech stack

Rust core · Tauri v2 shell · Svelte + Vite frontend · SQLite (`rusqlite`) · WebSocket (`tokio-tungstenite`) · local whisper.cpp-class STT with optional cloud fallback · NDI SDK via Rust FFI. Windows + macOS, both from day one. MIT licensed.

## Prerequisites (once you start building)

- Rust (stable) + Cargo
- Node.js + npm
- Tauri CLI (`cargo install tauri-cli` or `npm install -D @tauri-apps/cli`)
- Platform build tools ([Tauri prerequisites](https://tauri.app/start/prerequisites/) — WebView2 on Windows, Xcode command line tools on macOS)
- NDI SDK (for NDI render-target work, later phase)

## Running (once the scaffold actually compiles — see Phase 0 in `PROMPT.md`)

```bash
npm install
cargo tauri dev
```

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
