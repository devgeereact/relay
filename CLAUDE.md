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
├── src-tauri/              — Rust core + Tauri backend
│   └── src/
│       ├── main.rs
│       ├── audio.rs        — capture + VAD (stub)
│       ├── stt.rs          — speech-to-text (stub)
│       ├── detection.rs    — direct + semantic match, context memory (stub)
│       ├── router.rs       — content router, confidence gating (stub)
│       ├── channels.rs     — output channel render targets (stub)
│       └── db.rs           — SQLite access layer (stub)
└── src/                    — Svelte frontend (operator console + other screens)
```

## Working conventions

- Rust: `rustfmt` and `clippy` clean before any commit. No `unwrap()` in code paths that run during a live service — a panic mid-sermon is the worst possible failure mode. Prefer explicit error surfacing to the operator UI over silent failure.
- Every module stub in `src-tauri/src/` has a doc comment stating its single responsibility — keep it that way as you build it out. Don't let `router.rs` grow detection logic, don't let `channels.rs` grow detection logic, etc.
- Confidence thresholds are configuration, not hardcoded constants — see `docs/DECISIONS.md` on the self-calibrating threshold mechanism. Seed values are placeholders (`0.90` / `0.60`) until tuned against real corpus data.
- Before implementing a new feature, check `docs/DECISIONS.md` — if the decision isn't there, it hasn't been made yet. Ask, don't assume.
