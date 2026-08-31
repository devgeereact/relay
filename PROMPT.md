# Relay — build-phase reference

> **What this file is now.** Originally the full build-agent kickoff prompt. The product is
> built, so the duplicated pitch / architecture / AI-pipeline / data-model / tech-stack / success
> criteria have been removed — all of that is the canonical spec in [docs/SPEC.md](docs/SPEC.md),
> with working conventions in [CLAUDE.md](CLAUDE.md) and the *why* in
> [docs/DECISIONS.md](docs/DECISIONS.md). What remains is the **build-phase map**, kept because
> the Rust module docs (`audio.rs`, `stt.rs`, `detection.rs`, `router.rs`, `channels.rs`) and
> `src-tauri/capabilities/default.json` cite these phase numbers as their rationale anchor.

For the full documentation hierarchy, start at [docs/README.md](docs/README.md).

---

## Build order (suggested phases — reorder if you have a reason, but don't skip the boring early phases to get to the AI part)

**Phase 0 — repo hygiene**
Confirm the scaffold builds: `cargo check` in `src-tauri/`, `npm install` + dev server boots in the frontend. Fix whatever's missing from this hand-written skeleton — it's a starting point, not a guarantee it compiles as-is.

**Phase 1 — Tauri shell boots**
Blank window opens, topbar renders with static data, tab navigation between the screens works with no live data behind any of them yet.

**Phase 2 — data layer**
Implement `db.rs` against `docs/data/schema.sql`. Seed with one translation (KJV) and enough verse data to test against (doesn't need to be the full Bible yet — Genesis 1, Psalm 23, John 3, Romans 8 is enough to develop against).

**Phase 3 — audio capture + VAD**
`audio.rs`: list input devices, capture a stream, implement chunking + a basic VAD gate. Get raw audio flowing into the pipeline before touching STT.

**Phase 4 — local STT**
`stt.rs`: wire up a local whisper.cpp-class model, English only to start. Get a rolling transcript showing up in the console's transcript panel before adding language ID or African-language models.

**Phase 5 — direct match detection**
`detection.rs`: regex-based reference detection against the seeded verse data. This alone should already demo the core loop end-to-end: speak "John 3:16," see it appear.

**Phase 6 — content router + confidence gating**
`router.rs`: implement the two-tier gating and debounce logic. Wire the auto-fire vs suggested-chip distinction into the console UI. (Seed thresholds: the "Confidence-threshold mechanism" row in [docs/DECISIONS.md](docs/DECISIONS.md) — an unnumbered row, because the numbered log starts at §18. The "only Direct auto-fires" rule: [docs/DOMAIN_MODEL.md](docs/DOMAIN_MODEL.md) §6.)

**Phase 7 — output channels, one render target at a time**
`channels.rs`: start with the native-window (HDMI) render target only — it's the simplest. Get one real second screen showing a live-updating template before adding NDI encode or the WebSocket/kiosk path.

**Phase 8 — template engine**
Build the shared renderer against the template JSON shape in [docs/SPEC.md](docs/SPEC.md) §5. Prove it by pointing two different channels at two different templates and seeing genuinely different renders from the same detection event.

**Phase 9 — semantic match + context memory**
Add embedding-based paraphrase detection and the "current passage" state tracking. This is the hardest correctness problem in the whole project — budget real time for it, and test against actual sermon-style speech, not read-aloud scripture.

**Phase 10 — NDI + kiosk render targets, language expansion**
Add the remaining render targets, then start layering in Yoruba/Swahili/Hausa STT and code-switching handling.

Everything past Phase 6 should already be demoable to a real church for feedback — don't wait for Phase 10 to show someone the console.
