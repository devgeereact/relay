# Relay — developer / build-agent kickoff prompt

Paste this whole file into a fresh Claude Code session (or hand it to a developer) to start building. It contains everything decided so far. Read `CLAUDE.md` first for working conventions, then this file for the actual task breakdown.

---

## The pitch, in one paragraph

Relay is AI-assisted live presentation software for churches. It listens to a live sermon, detects Bible references — spoken as direct quotes or loose paraphrases — and automatically routes the right verse to every connected screen, each rendering it in its own style (a full serif verse card on the main screen, a plain high-contrast confidence monitor on stage, a lower-third for the stream, an ambient card in the lobby). It runs fully offline, prioritizes African-language speech understanding (Yoruba, Swahili, Hausa first), and is built to sit *above* existing gear — OBS, ATEM, ProPresenter — connecting to it over NDI, HDMI, and the local network, not replacing it.

## Why it exists / competitive context

A funded competitor, Pewbeam, already does real-time AI scripture detection for churches (offline-first, whisper-class STT, embeddings + RAG, NDI/HDMI output, paying customers in 30 countries). Relay's differentiation is **not** trying to out-build Pewbeam's detection feature alone — it's combining that kind of detection with **independent multi-screen templating** (the ProPresenter piece Pewbeam is only now adding) and **African-language STT as a first-class priority rather than an English-first afterthought**.

## Explicit non-goals

- Not a recording/scene-compositing replacement for OBS.
- Not building native SDI hardware output — NDI/HDMI only, SDI setups are served by hardware the church already owns (ATEM, converters).
- Not a general-purpose AI assistant — strictly live-service content routing.
- Not attempting song-lyric/setlist detection in v1 — scripture only.

## Architecture, one paragraph

One AI decision, fanned out to N independently-templated **output channels**. A channel is a configured destination; a template is a layout + style + data-binding definition; any channel can be assigned any template. Channels render through one shared template engine to one of three **render targets**: a native fullscreen window (HDMI), a headless render encoded to NDI (ingested by OBS/ATEM/ProPresenter/vMix), or a networked browser client driven by WebSocket state pushes (cheap kiosk hardware, e.g. a Raspberry Pi in Chromium kiosk mode). Never special-case logic per channel type — the render target and template are configuration, not branches in business logic.

## AI detection pipeline

1. Audio → 200–500ms overlapping chunks → VAD gate, skip silence.
2. Streaming local STT → rolling partial + final transcript, per-chunk language ID (code-switching is the normal case).
3. **Direct match**: regex for `<book> <chapter>:<verse>` shapes, multilingual book-name alias tables, phonetic-ASR-error tolerance.
4. **Semantic match**: embed the rolling window, vector-similarity search against a pre-embedded verse corpus, top-k + confidence score.
5. **Context memory**: track "current passage" state so a bare "verse 4" resolves against the last-referenced book/chapter.
6. **Confidence gating, two-tier, self-calibrating per install**: seed defaults auto-fire ≥0.90, suggest ≥0.60 (mid confidence → one-tap operator-confirmable chip, never auto-touches output). Nudge thresholds per install based on operator confirm/reject signal over the first few live services. Manual override slider in Settings always available.
7. **Debounce** ~4–6s on repeat auto-fire of the same verse, overridden instantly by any new explicit direct match.
8. **Operator override**: top-of-console, one action, always reachable — first-class control, not an escape hatch.

## Data model (SQLite — see `docs/data/schema.sql` for the real DDL)

`translations`, `verses`, `templates`, `output_channels`, `services`, `transcripts`, `detections` (`method`: direct/semantic; `status`: auto/suggested/dismissed), `cues`.

## Tech stack (already decided — see `CLAUDE.md`)

Rust core, Tauri v2 shell, Svelte + Vite frontend, SQLite via `rusqlite`, WebSocket via `tokio-tungstenite`, `whisper.cpp`-class local STT with optional cloud fallback, NDI SDK via Rust FFI, Windows + macOS both from day one, MIT license.

## Success criteria for v1 (what "done" means)

- ≥90% correct verse ID on direct quotes, ≥75% on paraphrase, <3s p50, real live-room audio
- Wrong-verse rate <5% of AI triggers, always-visible one-tap override
- Full service runs 100% offline
- 3+ simultaneous output channels, each own template, <100ms cross-channel sync drift
- 1+ output channel confirmed running on a $50 Raspberry Pi kiosk client
- NDI output confirmed ingestible by OBS and vMix
- Runs smoothly on an 8GB RAM Windows laptop
- Survives 3 consecutive live Sunday services, zero crash, projection team says it beats manual

## Design reference

Two mockups exist in `docs/design/` — open directly in a browser:
- `relay-app-mockup.html` — the operator console in detail
- `relay-app-screens.html` — clickable, all 5 screens (Console, Channels, Templates, Library, Settings)

Match the visual language when building the real frontend: dark charcoal console UI, Space Grotesk for chrome/headers, Inter for body/data, Fraunces (serif) for scripture render panels specifically — the serif-for-scripture / sans-for-console contrast is a deliberate signature, keep it.

---

## Build order (suggested phases — reorder if you have a reason, but don't skip the boring early phases to get to the AI part)

**Phase 0 — repo hygiene**
Confirm the scaffold builds: `cargo check` in `src-tauri/`, `npm install` + dev server boots in the frontend. Fix whatever's missing from this hand-written skeleton — it's a starting point, not a guarantee it compiles as-is.

**Phase 1 — Tauri shell boots**
Blank window opens, topbar renders with static data, tab navigation between the 5 screens works with no live data behind any of them yet.

**Phase 2 — data layer**
Implement `db.rs` against `docs/data/schema.sql`. Seed with one translation (KJV) and enough verse data to test against (doesn't need to be the full Bible yet — Genesis 1, Psalm 23, John 3, Romans 8 is enough to develop against).

**Phase 3 — audio capture + VAD**
`audio.rs`: list input devices, capture a stream, implement chunking + a basic VAD gate. Get raw audio flowing into the pipeline before touching STT.

**Phase 4 — local STT**
`stt.rs`: wire up a local whisper.cpp-class model, English only to start. Get a rolling transcript showing up in the console's transcript panel before adding language ID or African-language models.

**Phase 5 — direct match detection**
`detection.rs`: regex-based reference detection against the seeded verse data. This alone should already demo the core loop end-to-end: speak "John 3:16," see it appear.

**Phase 6 — content router + confidence gating**
`router.rs`: implement the two-tier gating and debounce logic described above. Wire the auto-fire vs suggested-chip distinction into the console UI.

**Phase 7 — output channels, one render target at a time**
`channels.rs`: start with the native-window (HDMI) render target only — it's the simplest. Get one real second screen showing a live-updating template before adding NDI encode or the WebSocket/kiosk path.

**Phase 8 — template engine**
Build the shared renderer against the template JSON shape in `docs/SPEC.md` §5. Prove it by pointing two different channels at two different templates and seeing genuinely different renders from the same detection event.

**Phase 9 — semantic match + context memory**
Add embedding-based paraphrase detection and the "current passage" state tracking. This is the hardest correctness problem in the whole project — budget real time for it, and test against actual sermon-style speech, not read-aloud scripture.

**Phase 10 — NDI + kiosk render targets, language expansion**
Add the remaining render targets, then start layering in Yoruba/Swahili/Hausa STT and code-switching handling.

Everything past Phase 6 should already be demoable to a real church for feedback — don't wait for Phase 10 to show someone the console.
