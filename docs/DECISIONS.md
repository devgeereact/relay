# Decisions — Relay

Every decision below was made deliberately, with reasoning, in a brainstorm session. If you're an AI agent or a new contributor and something in the code contradicts this list, the code is wrong — flag it, don't silently "fix" the decision.

| Decision | Reasoning |
|---|---|
| No native SDI hardware output, ever (unless explicitly reopened) | High engineering cost (Blackmagic DeckLink-class SDK) for narrow reach. Anyone with SDI gear already owns hardware (ATEM, converter) that accepts NDI/HDMI and re-outputs SDI — interoperability is preserved without owning the SDI problem. |
| Core engine: Rust | Real-time audio/video/inference workloads suit Rust's performance and FFI story to C SDKs (NDI, whisper.cpp) better than a Node-based core. |
| Desktop shell: Tauri, not Electron | ~10–20x smaller install size, lower idle memory — concretely matters for the target market's modest hardware and unreliable power. |
| Output channels modeled as render targets of one shared template engine | Maximizes code reuse across preview/output/remote-screen use cases; enables ultra-low-cost output hardware (Raspberry Pi kiosk clients), which matters for the target market. |
| Local-first, hybrid STT (on-device model, optional cloud fallback) | Matches offline-reliability needs of the target market and keeps African-language model swapping architecturally simple — the model is a pluggable component, not baked into the pipeline. |
| ATEM's multiview/program-bus model rejected as the mental model | Structurally different problem — N independently-styled destinations fanned from one AI decision, not one program feed. Renamed internally as "output channels." |
| Priority STT languages, tier 1: Yoruba, Swahili, Hausa. Tier 2: Igbo. Parked: Zulu, Amharic, Twi, Shona, others | Ranked by (a) existing Whisper/community fine-tune coverage, (b) speaker population + church-market size, (c) frequency of English code-switching in real preaching. Swahili has the best existing Whisper coverage; Yoruba has the largest addressable church market; Hausa is large (~80M speakers) with growing dataset support. |
| Windows and macOS both, day one | Rust+Tauri makes this near-free — same core, same webview UI on both. Target market skews Windows on cost grounds but macOS exists in wealthier/urban churches. |
| Business model: free / open source, no tiers | User decision. Sustainability path (donations, grants, optional paid cloud add-on) not yet decided — parked, not blocking v1. |
| Confidence-threshold mechanism: self-calibrating per install, not one static global number | Accent, mic quality, and room noise vary too much across churches for a single global threshold to be right for most. Ship conservative seed defaults (auto-fire ≥0.90, suggest ≥0.60), nudge thresholds per install using operator confirm/reject signal over the first few live services, always leave a manual override slider in Settings. |

## Build-out decisions (presentation suite)

Made while building the Library + Planner + output layer. Same rule: if the code contradicts these, flag it.

| Decision | Reasoning |
|---|---|
| One unified **cue model** for every content type (`plan_items.cue_type` + `payload_json`) | Scripture, song, media, announcement, and countdown all reduce to the same polymorphic cue, so the Planner, monitors, and one renderer never branch per type. Adding a content type is a new payload shape, not new plumbing. |
| **Snapshot vs. reference** per content type | Scripture live-resolves from its reference at fire time (always current text/translation). Songs and announcements snapshot into the cue for offline reliability, but edits **propagate** (`sync_*_in_plans`) so a Library edit is never stale in a plan. Best of both, no per-type special case at fire time. |
| **Arrangements** = named play-orders stored as section-index sequences; "Standard" implicit | ProPresenter-parity feature. Storing indices (not copied lyrics) keeps edits propagating; storing the sequence on the cue lets a lyric edit re-expand into the right (possibly repeated) slots. Standard is never persisted — it's just "all sections in order." |
| **Countdown ticks locally** in each output from a broadcast target epoch | Broadcasting every second would spam the WS hub and drift; broadcasting the target once and ticking client-side is offline-clean, sync-correct, and updates digits in place (no crossfade per tick, no reactive-loop freeze). |
| **Verse auto-fit** (measure + shrink) instead of fixed/length-bucketed sizing | Real live verses vary wildly; a heuristic clips or overflows. Measuring the box and shrinking guarantees scripture always fits at any output size. Font-size is set imperatively so it can't re-enter Svelte's scheduler and loop. |
| **FTS5** added *behind* the existing reference/phrase/semantic ranker, not replacing it | bm25 full-text catches loose, non-contiguous word queries a substring `LIKE` misses, but precise reference/phrase/semantic matches must still rank first. FTS is the recall tail, self-healing via an idempotent index-rebuild migration. |
| **Strip KJV translator glosses** at import, keep supplied-word italics | The bundled corpus brackets both marginal notes (`{…: Heb. …}`, not verse text) and supplied words (`{it was}`, real text). Drop the former, unbracket the latter — in code (versioned, re-runnable via migration), source data untouched. |
| **No native `confirm()`/`alert()`** anywhere | Tauri's webview doesn't implement JS dialogs (returns false) — they silently break actions. All confirmations are in-app two-step ("arm → confirm"). |
| **Per-content-type templates** carried as a `template_json` override on the cue | Lyrics should look like lyrics and scripture like scripture without a per-channel branch. The override is just data on the broadcast; the one renderer honors it, else the channel template. |
| Console migrated to the global `--v-*` design tokens | The console had a private palette; unifying to the shared tokens keeps one design system across every surface. |

## Non-goals, with reasoning

- **Not a recording/scene-compositing replacement for OBS.** OBS already does this well and free — not a place to spend differentiation effort.
- **Not a general AI assistant.** Scope discipline — this is a single live-service workflow tool, not a platform.
- **Not attempting song-lyric/setlist detection in v1.** Separate subsystem, separate risk, would dilute focus on the scripture-detection core loop that actually differentiates the product.

## Competitive framing (why this exists)

Pewbeam is a live, funded competitor with paying churches in 30 countries and a stated roadmap toward a full presentation suite. This project is a deliberate bet on out-executing a moving target on two specific axes — independent multi-screen templating, and African-language speech understanding as a first-class priority rather than an English-first afterthought — not an attempt to fill an empty market.
