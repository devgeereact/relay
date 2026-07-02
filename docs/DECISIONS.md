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

## Non-goals, with reasoning

- **Not a recording/scene-compositing replacement for OBS.** OBS already does this well and free — not a place to spend differentiation effort.
- **Not a general AI assistant.** Scope discipline — this is a single live-service workflow tool, not a platform.
- **Not attempting song-lyric/setlist detection in v1.** Separate subsystem, separate risk, would dilute focus on the scripture-detection core loop that actually differentiates the product.

## Competitive framing (why this exists)

Pewbeam is a live, funded competitor with paying churches in 30 countries and a stated roadmap toward a full presentation suite. This project is a deliberate bet on out-executing a moving target on two specific axes — independent multi-screen templating, and African-language speech understanding as a first-class priority rather than an English-first afterthought — not an attempt to fill an empty market.
