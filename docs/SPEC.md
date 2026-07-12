# Relay — technical specification (v0.1)

Status: draft, from brainstorm session 2026-07-02. "Relay" is a placeholder product name — rename freely, it's one find-replace across this repo.

## 1. Overview & positioning

Three existing tool categories cover live church production today: **OBS** (capture, scene compositing, recording, free), **ATEM** (hardware video switching, one program bus from many inputs), and **ProPresenter** (templated lyrics, stage display, timers). **Pewbeam** is a funded competitor adding real-time AI scripture detection on top of a presentation layer, with paying churches in 30 countries and a roadmap toward a full presentation suite.

Relay's wedge is not rebuilding all four categories to parity. It's the combination of **AI content intelligence** with **independent multi-screen templating**, with **African-language speech understanding as a first-class target**, not an afterthought.

| Capability | OBS | ATEM | ProPresenter | Pewbeam | Relay (v1) |
|---|---|---|---|---|---|
| Recording / scene compositing | Yes | — | Limited | — | Not v1 — use OBS |
| Hardware video switching | — | Yes | — | — | Not built — interop only |
| Templated lyrics / stage display | — | — | Yes | Basic (roadmap) | Yes — per-channel templates |
| Live AI scripture detection | — | — | — | Yes | Yes — plus context memory |
| Independent output channels, own template each | — | Partial (one bus) | Partial | Unclear | Core design principle |
| African-language STT priority | — | — | — | English-first today | Yoruba / Swahili / Hausa tier 1 |
| Offline-first | N/A | N/A | Partial | Yes | Yes |

**Design stance:** Relay sits above the existing AV chain, not in place of it. It talks to OBS, ATEM, and ProPresenter over NDI, HDMI, and the local network.

## 2. Goals & non-goals

### v1 success criteria
- ≥90% correct verse ID on direct quotes, ≥75% on paraphrase, <3s (p50), real live-room audio
- Wrong-verse rate <5% of AI triggers, always-visible one-tap operator override
- Full service runs 100% offline, zero internet dependency
- 3+ simultaneous output channels, each own template, <100ms cross-channel sync drift
- 1+ output channel confirmed running on a $50 Raspberry Pi kiosk client
- NDI output confirmed ingestible by both OBS and vMix
- Runs smoothly on an 8GB RAM Windows laptop
- Survives 3 consecutive live Sunday services, zero crash, projection team reports it beats manual control

### Explicit non-goals (v1)
- Not a recording / scene-compositing replacement for OBS.
- Not building native SDI hardware output (Blackmagic DeckLink-class) — see §9.
- Not a general-purpose AI assistant — strictly live-service content routing.
- Not attempting song-lyric/setlist detection in v1.

## 3. System architecture

One AI decision, fanned out to independently-styled output channels — deliberately breaking from ATEM's single-program-bus model.

```
Audio input           Manual control
(mic/mixer feed)      (search & override)
        \                   /
         v                 v
        AI understanding core
   (speech-to-text · language ID · verse/topic match)
                    |
                    v
              Content router
  (decides what each output channel shows, and when)
                    |
      -------------------------------
      |         |          |        |
      v         v          v        v
  Main       Stage     Streaming   Lobby
  screen     display               screen
      \         |          |        /
       \        |          |       /
        --  NDI / HDMI / same network  --
                    |
                    v
       Your existing studio gear (unchanged)
        OBS Studio · ATEM switcher · ProPresenter
```

## 4. AI detection pipeline

1. **Chunking** — audio → 200–500ms overlapping chunks → VAD gate, skip silence.
2. **Streaming STT** — local whisper.cpp-class model → rolling partial + final transcript, per-chunk language ID (code-switching is the normal case in target-market preaching).
3. **Direct pattern match** — regex for `<book> <chapter>:<verse>` shapes, multilingual book-name alias tables per priority language, phonetic-ASR-error tolerance (e.g. "John free sixteen" → John 3:16).
4. **Semantic match** — embed the rolling window, vector-similarity search against a pre-embedded verse corpus, top-k candidates with confidence score. Catches paraphrase and topical reference.
5. **Context memory** — track "current passage" state; a bare "verse 4" resolves against the last active book/chapter rather than requiring a fresh full reference.
6. **Confidence gating, two-tier, self-calibrating** — seed defaults: auto-fire ≥0.90, suggest ≥0.60 (mid confidence surfaces as a one-tap operator-confirmable chip, never auto-touches output; low confidence dropped silently). Thresholds nudge per install based on operator confirm/reject signal over the first few live services. Manual override slider always available.
7. **Debounce** — ~4–6s cooldown on repeat auto-fire of the same verse, overridden instantly by any new explicit direct-quote match.
8. **Operator override** — first-class, top-of-UI, one tap.

> Exact seed confidence numbers are placeholders until tuned against a real transcript + verse corpus.

## 5. Output channels & template system

A **channel** is a configured destination. A **template** is a layout + style + data-binding definition any channel can be assigned. The same content event renders differently per channel because each channel points at a different template — never because the pipeline treats channels differently.

### Render targets

| Target type | What it is | Maps to |
|---|---|---|
| Native window | Borderless fullscreen window pinned to a display | HDMI output |
| Headless render → NDI encode | Off-screen render, encoded as an NDI source | NDI into OBS / ATEM / vMix / ProPresenter |
| Networked browser client | Any LAN device hitting a local URL, state pushed over WebSocket | Kiosk screens — e.g. a $50 Raspberry Pi |

### Template shape (draft)

```json
{
  "id": "tmpl_main_screen_classic",
  "layout": {
    "regions": [
      { "id": "verse_text", "binding": "content.text", "align": "center" },
      { "id": "reference", "binding": "content.ref", "align": "center" }
    ]
  },
  "style": {
    "font_display": "Fraunces",
    "background": "radial-warm-dark",
    "transition": "fade",
    "transition_ms": 400
  }
}
```

Stage display and lobby templates use the same shape with different region sets (stage adds a `timer` region; lobby adds a `next_event` region).

## 6. Data model

See `docs/data/schema.sql` for the DDL, and `src-tauri/src/db/` for the queries — one module per aggregate. Local-first throughout, matching the offline-first / local-trust posture.

**Corpus + output**
`translations`, `verses` (+ `verses_fts`, the FTS5 index), `templates`, `output_channels`

**Detection + history**
`services`, `transcripts`, `cues`, `voice_profiles` (per-preacher accent + learned gate calibration), and `detections` — `method`: direct/semantic, `status`: **auto / suggested / dismissed / manual**. `manual` means a *human* put it on screen (an override, a confirmed suggestion, a next/back nav). The self-calibrating router learns from that column, so the distinction is load-bearing rather than archival.

**Library + Planner** (the presentation suite, added after v0.1)
`service_plans`, `plan_items` (the unified cue: `cue_type` + `payload_json`), `songs`, `song_sections`, `song_arrangements`, `saved_scripture`, `announcements`, `media_assets`, `app_settings`

**Migrations.** `PRAGMA user_version` ladder (`db::SCHEMA_VERSION`). Databases created before versioning existed are brought to the baseline once by `baseline_forward_fill` — a set of sniff-based checks kept only for them — and then stamped, after which the ladder takes over.

## 7. Platform & tech stack

| Layer | Choice | Why |
|---|---|---|
| Core engine | Rust | Real-time audio/inference workloads suit Rust's performance and FFI story to C SDKs (NDI, whisper.cpp) |
| Desktop shell | Tauri | ~10–20x smaller install than Electron, lower idle memory — matters on modest hardware and unreliable power |
| UI framework | Svelte | Small bundle, no heavy runtime, consistent with the lean-footprint priority |
| Speech-to-text | whisper.cpp-class, local-first | Offline reliability; optional cloud fallback; open path to community African-language fine-tunes |
| Local data | SQLite (`rusqlite`) | Local-first trust posture |
| Local distribution | WebSocket (`tokio-tungstenite`) | Powers networked-browser-client output channels |
| Video-over-IP | NDI SDK via Rust FFI | Free SDK, industry-standard broadcast interop |
| Platforms | Windows + macOS, day one | Near-free with Rust+Tauri |
| License | MIT | Free / open source (see decision log) |

## 8. Priority languages & STT strategy

| Tier | Language | Rationale |
|---|---|---|
| 1 | Swahili | Strongest existing Whisper coverage of any African language |
| 1 | Yoruba | Largest addressable church market (Nigeria), reasonable Common Voice data, heavy English code-switching |
| 1 | Hausa | ~80M speakers, growing dataset support |
| 2 (post-v1) | Igbo | Large market, dataset maturity lower today |
| Parked | Zulu, Amharic, Twi, Shona, others | Low-resource, need dedicated fine-tuning effort |

Community datasets to evaluate: Masakhane (African NLP research), Mozilla Common Voice. Code-switching handling is a first-class requirement, not an edge case.

## 9. Integration & interoperability

- **NDI** — free SDK, cross-platform. Relay is an NDI source; OBS, ATEM (NDI-enabled), vMix, and ProPresenter ingest it directly.
- **HDMI** — not a special integration, a borderless fullscreen window on whichever display is physically connected. No hardware SDK needed.
- **SDI — explicitly out of scope.** True SDI I/O needs dedicated hardware (Blackmagic DeckLink-class) and a C++ SDK — high cost, narrow reach. Anyone with SDI gear already owns hardware (an ATEM, or a converter) that accepts NDI/HDMI and re-outputs SDI. Relay's NDI/HDMI output preserves full SDI-setup compatibility without owning the SDI hardware problem.

## 10. Roadmap & open items

### Parked, not eliminated
- Native SDI hardware output — revisit only if a real target segment has SDI gear with no ATEM/converter at all
- Recording / full scene compositing — not a differentiation target, OBS already does this well
- Song-lyric / setlist detection — separate subsystem, not scoped for v1
- Sustainability path for the free/open-source model (donations, grants, optional paid add-on)

### Still needs real data, not guessable in a spec
- Exact confidence-score seed numbers
- Book-name alias tables + phonetic-error tolerance lists for the three tier-1 languages

## Glossary

- **Output channel** — an independently-styled destination that receives its own template rendering of the same underlying content decision. Not modeled as an ATEM-style single program bus.
- **Content router** — decides what each output channel shows, and when.
- **Render target** — how a channel's template is actually output (native window / NDI encode / networked browser client).
- **Direct match** — regex-based detection of an explicit spoken reference.
- **Semantic match** — embedding + vector-similarity detection of a paraphrased or topical reference.
- **Context memory** — tracking the "current passage" so a bare "verse 4" resolves correctly.
- **Confidence gating** — two-tier, self-calibrating trigger logic.
