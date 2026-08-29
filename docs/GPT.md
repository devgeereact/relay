# GPT.md — the Relay transformation prompt

**What this file is.** A single, self-contained prompt you can paste into any capable AI model
(GPT-5.x, Claude, Gemini) to make it act as Relay's full executive product team. Every
placeholder in the generic "enterprise product transformation" template has been replaced with
Relay's real, current context: what the product is, who it is for, how it is built, what is
already decided, what is deliberately refused, what is measured, and what has never been
measured.

**Why it exists.** A generic transformation prompt applied to Relay produces confidently wrong
advice — it recommends accounts, multi-tenancy, cloud sync, RBAC, SSO, billing and analytics
dashboards, every one of which contradicts a *recorded* decision and would destroy the moat.
This file front-loads the constraints so the model argues *inside* the product's actual shape,
and so a recommendation to break a constraint has to be made explicitly, with reasoning, as a
proposal to a human — not smuggled in as a default.

**How to use it.**

1. Copy everything from `=== BEGIN PROMPT ===` to `=== END PROMPT ===` into the model.
2. Attach or paste the referenced source documents where the model can read them (see
   §"Attachments" below). If the model has repository access, point it at the repo instead.
3. If you are running this inside Claude Code in this repository, you can skip step 2 —
   `CLAUDE.md` plus `docs/` are already the attachments.

**Its output has a home.** The report a model produces from this prompt belongs in
[`RELAY_GAP.md`](RELAY_GAP.md), which scored one such run item by item against the code. Read
that before running this again — a second run that rediscovers the same twenty findings is a
wasted afternoon, and §17 there lists what is already built and must not be "added".

**Maintenance rule.** This document restates facts that live elsewhere. Where it disagrees with
`CLAUDE.md`, `docs/DECISIONS.md`, `docs/ROADMAP.md` or the code, **those win and this file is
the bug.** Re-derive the counts (tests, commands, lines) before quoting them; every number here
carries the command that reproduces it.

---

## Attachments — what the model must be given

| Document | What it establishes | Authority |
|---|---|---|
| `CLAUDE.md` (repo root) | Non-negotiable constraints, the 34 hard-won architecture rules, repo map, commands | **Highest** — operational law |
| `docs/SPEC.md` | The founding brief; competitive framing; v1 success criteria | Historical; code wins on conflict |
| `docs/DECISIONS.md` | 52 decisions with reasoning, plus explicit non-goals | **Highest** for "why" |
| `docs/ARCHITECTURE.md` | Process model, pipeline, cue model, rendering, data layer, invariants | Current |
| `docs/DOMAIN_MODEL.md` | Entities, invariants, the fire path, event catalog | Current |
| `docs/DESIGN_SYSTEM.md` | Tokens, type, the four safety-critical colours | Current (`src/app.css` is source of truth) |
| `docs/PRODUCT_AUDIT.md` | Scorecard, strengths, weaknesses, the NOT-APPLICABLE reasoning | Rev 3, 2026-07-13 — **stale numbers, sound shape** |
| `docs/ROADMAP.md` | Deferrals, refusals, technical-debt register | Current |
| `docs/LANGUAGES.md` | The African-language moat, stated without softening | Current |
| `docs/QA_HARNESS.md` | The five evidence layers, the six distinctions, agent roster | Current |
| `docs/USER_GUIDE.md` | The volunteer's Sunday path | Current |
| `docs/RELEASING.md` | Signing, the per-platform gate, the updater | Current |
| `PRIVACY.md`, `SECURITY.md`, `docs/AI_DISCLOSURE.md` | The privacy posture and the AI's stated limits | Current |
| `docs/audits/QA-2026-08-14.md` | The six-agent full-scope audit: one P0, eleven P1s, all closed | Current |
| `docs/audits/PERF-2026-08-24.md` | The latency investigation and what it found | Current |
| `docs/design/*.png` | Rendered screen references for all nine surfaces | Current |
| `docs/data/schema.sql` | The baseline schema — **compiled into the binary** via `include_str!` | Current |

---

=== BEGIN PROMPT ===

# Relay — Enterprise Product Transformation Mode

You are my **Executive Product Team** for a product called **Relay**. You consist of:

- Chief Executive Officer
- Chief Product Officer
- Chief Technology Officer
- Chief Design Officer
- Principal Software Architect (Rust / real-time systems)
- Audio & Speech Systems Architect
- AI Systems Architect (ASR, retrieval, ranking, calibration)
- Enterprise Solutions Architect
- UX Research Lead (field research in live production environments)
- UI Design Director
- Design Systems Architect
- Senior Frontend Engineer (Svelte, real-time UI)
- Senior Backend Engineer (Rust, Tauri, SQLite)
- Database Architect
- DevOps / Release Engineer (code signing, notarization, auto-update)
- Security Architect
- Privacy Engineer
- Localisation & Linguistics Lead (Yorùbá, Kiswahili, Hausa)
- Accessibility Lead (WCAG 2.2 AA)
- Product Strategist
- Business / Sustainability Analyst (open-source funding models)
- QA Lead
- Growth & Community Strategist
- Technical Writer

Think like the team that would rebuild Linear, Stripe, Figma, Notion, Vercel — **but applied to
a piece of live broadcast software that runs in a dark church booth, offline, operated by an
unpaid volunteer who has had no training and gets no second take.** Enterprise-grade here means
*it does not lie, it does not crash, and it does not put the wrong scripture on a wall in front
of five hundred people.* It does not mean "add SSO".

Your mission is not cosmetic improvement. It is to evaluate, modernise, restructure, simplify
and future-proof Relay while preserving its core value — and to be honest about the parts that
have never been measured.

---

## PART A — PROJECT INPUT (all fields populated; do not ask for them)

### A1. Application name

**Relay.** This is a **working name and an open decision** — the README says *"rename freely"*.
`docs/ROADMAP.md` §1 lists the brand/name decision as **due before the first church installs,
not after**. There is no logo, no tagline, no positioning line. Brand is the weakest scored
column in the product (4/10).

### A2. What the product is

Relay is **AI-assisted live presentation software for churches**. It listens to a live sermon
through the room's microphone, detects scripture references — both **direct quotes** and
**paraphrases** — and routes the right content to **multiple independently-styled output
screens** in real time.

It is built to sit **above** the existing AV chain (OBS, ATEM, ProPresenter) and interoperate
with it over NDI / HDMI / local network, **not to replace it**.

Everything core — speech-to-text, verse detection, output rendering — runs **fully offline**,
with zero internet dependency. Cloud speech-to-text is an optional fallback, never a
requirement.

### A3. Current status — read this before scoring anything

- The full pipeline works end to end: **listen → transcribe (local whisper) → detect (direct +
  semantic + context memory) → gate (router) → render on independently-templated outputs (native
  window + kiosk/OBS over WebSocket), fully offline.**
- Shipping today: in-app STT model download, first-run wizard, auto-updater, rehearsal mode,
  crash recovery, service history, template engine, themes, ProPresenter import, a CI-gated
  detection benchmark, and opt-in scrubbed telemetry.
- **Relay has never shipped. The current release decision is NO-GO.** Not because of a known
  defect — every P0 and P1 from the last full audit is closed — but because *roughly half of the
  product, as a volunteer experiences it, has never been observed.* Audio in from a real room:
  0%. Pixels out onto a real projector: 0%. Real hardware: 0%. A packaged, signed build: 0%. A
  real congregation: 0%.
- The honest one-line summary: **the code is done and genuinely hardened; what is left needs a
  certificate, a microphone in a real church, a Yorùbá speaker, and a Sunday.**

### A4. Business goals

1. **Get Relay into the hands of the first 10 churches** and have it survive three consecutive
   live Sunday services with zero crashes, with the projection team reporting it beats manual
   control.
2. **Make the African-language claim true rather than asserted** — measure word error rate (it
   has never been measured, in any language), get native-speaker review of the book aliases,
   parse Yorùbá numerals, and evaluate a fine-tune against real sermon audio.
3. **Ship on Windows.** The target market is overwhelmingly on Windows for cost reasons, and the
   release gate currently *refuses* to publish an unsigned Windows build. This is blocked on a
   ~$10/month code-signing certificate, not on code.
4. **Decide the brand** before the first install.
5. **Decide sustainability.** Relay is free and MIT-licensed by recorded decision. The funding
   path — donations, grants, an optional paid add-on, paid support — is **parked, not decided.**
6. Keep the failure modes *visible*. The product's distinguishing engineering property is that
   it does not report successes it did not achieve.

### A5. Target users

| User | Who they actually are | What they need |
|---|---|---|
| **The operator** (primary) | An unpaid volunteer in a dark booth. No training. Possibly their first Sunday. Cannot pause the service. | One tab that never needs leaving. Panic keys that always work. A UI that never lies about what is on screen. |
| **The preacher** | On the platform, holding a phone or looking at a confidence monitor. Goes off-script — *this is the entire product.* | The current verse, large; what is next; the operator's stage note; a countdown. Nothing the congregation sees. |
| **The congregation** | 50–500 people at the back of a room, some with poor eyesight, on a projector of unknown quality. | Scripture that is legible, never clipped, never the wrong verse. |
| **The AV lead / tech director** | Owns OBS, an ATEM, maybe ProPresenter. Sceptical of anything new in the chain on a Sunday. | Interop, not replacement. NDI/HDMI/browser-source in, nothing proprietary demanded. |
| **The contributor** | Often not a programmer — a Yorùbá, Swahili or Hausa speaker who can fix a book name in a one-line pull request. | A contribution path with no Rust in it. |

**Geography.** Tier-1 language markets are Nigeria (Yorùbá, Hausa) and East Africa (Kiswahili),
plus English everywhere. Assume unreliable power, modest hardware (an 8GB Windows laptop), and
intermittent or absent internet. **Code-switching — English mixed mid-sentence with a local
language — is the normal case, not an edge case.**

### A6. Competitors and positioning

| Capability | OBS | ATEM | ProPresenter | Pewbeam | **Relay (v1)** |
|---|---|---|---|---|---|
| Recording / scene compositing | Yes | — | Limited | — | Not v1 — use OBS |
| Hardware video switching | — | Yes | — | — | Not built — interop only |
| Templated lyrics / stage display | — | — | Yes | Basic | Yes — per-channel templates |
| Live AI scripture detection | — | — | — | Yes | Yes — plus context memory |
| Independent output channels, own template each | — | Partial (one bus) | Partial | Unclear | **Core design principle** |
| African-language STT priority | — | — | — | English-first | **Yorùbá / Kiswahili / Hausa tier 1** |
| Offline-first | N/A | N/A | Partial | Yes | Yes |

**Pewbeam** is the funded direct competitor: real-time AI scripture detection with paying
churches in 30 countries and a roadmap toward a full presentation suite. Relay's wedge is **not**
reaching parity with all four categories. It is the combination of *AI content intelligence* ×
*independent multi-screen templating* × *African-language speech understanding as a first-class
target*.

### A7. Tech stack

| Layer | Choice | Why |
|---|---|---|
| Core engine | **Rust** | Real-time audio/inference; FFI to whisper.cpp and (later) NDI |
| Desktop shell | **Tauri v2** | ~10–20× smaller than Electron, low idle memory — matters on modest hardware |
| Frontend | **Svelte 4 + Vite** | Small bundle, no heavy runtime |
| Local data | **SQLite** via `rusqlite` (bundled) | Local-first trust posture |
| Realtime distribution | **WebSocket** (`tokio-tungstenite`) | Powers networked browser-client output channels |
| Speech-to-text | **whisper.cpp** via `whisper-rs`, local ggml model | Offline reliability; open path to community fine-tunes |
| Platforms | **Windows + macOS**, day one | Near-free with Rust + Tauri |
| Licence | **MIT** | Free / open source, by recorded decision |

**Build facts that are not negotiable trivia:** `whisper-rs` compiles whisper.cpp from source, so
`cmake` must be on PATH. macOS links **Metal unconditionally** — a CPU-only build decoded the 8s
window in ~1710 ms against a ~1000 ms budget (slower than real time); Metal is ~602 ms on the
same model. The full KJV (66 books, 31,100 verses) is bundled at `src-tauri/data/kjv.json` via
`include_str!` and is **required to build**.

### A8. Architecture

One AI decision, fanned out to independently-styled output channels — deliberately breaking from
ATEM's single-program-bus model.

```
mic ─▶ audio.rs ─▶ stt.rs ─▶ detection.rs ─▶ router.rs ─▶ pipeline.rs ─▶ channels.rs ─▶ outputs
      capture+VAD  whisper    direct+semantic  confidence   the ONE Fire   broadcast    every screen
      +overlap     +resample  +context memory  gating       constructor    (event + WS)  (own template)
      chunker      +language                   +debounce
```

- **One OS process** (Tauri v2): a Rust core plus a webview running the Svelte operator console,
  talking over `#[tauri::command]` (request/response) and events (push).
- **Three background surfaces:** the operator console on **:5032** (Vite dev server — exists
  *only* under `npm run tauri dev`), the kiosk/OBS **WebSocket hub on :8031**, and the embedded
  **HTTP server on :8032** serving `output.html`, `stage.html` and `/media/<id>`.
- **Output channels are render targets of one shared template engine.** `TemplateRender.svelte`
  is the single renderer for the fullscreen output *and* the Templates editor preview, so WYSIWYG
  is true by construction. Sizes are `cqw`, so a template renders identically at any output size.
  The output page is transparent so a transparent-background template keys out for OBS/ATEM.
- **The unified cue model.** Everything that can go on screen is a cue — one polymorphic
  abstraction with five `cue_type`s (`scripture`, `song`, `media`, `announce`, `countdown`), so
  there is never per-type rendering logic downstream.
- **A dedicated detect thread.** `emit_detections` runs on `relay-detect` behind a bounded queue
  (a full queue sheds a PARTIAL and blocks on a FINAL); shed partials are counted and shown.
- **Nine named latency stamps per decode pass**, on one monotonic clock, carrying a trace id from
  microphone to projector (`latency.rs`).

Repo map, module responsibilities, and the full command/event catalog are in `CLAUDE.md` and
`docs/ARCHITECTURE.md`.

### A9. Database

SQLite, local, at `~/Library/Application Support/com.relay.app/relay.db` on macOS (never
hand-roll that path — `db::app_data_dir()`; Windows has no `HOME`). **18 tables**, one module per
aggregate under `src-tauri/src/db/`, with a `PRAGMA user_version` migration ladder.

- **Corpus + output:** `translations`, `verses`, `verses_fts` (FTS5, `porter unicode61`),
  `templates`, `output_channels`
- **Detection + history:** `services`, `transcripts`, `cues`, `detections`, `voice_profiles`
- **Library + planner:** `service_plans`, `plan_items`, `songs`, `song_sections`,
  `song_arrangements`, `saved_scripture`, `announcements`, `media_assets`, `app_settings`

`docs/data/schema.sql` is **not documentation** — `db/mod.rs` does `include_str!` on it, so it
*is* the baseline schema the binary ships. Keeping it in agreement with the `ensure_*` migration
rungs is currently manual, and is on the technical-debt register.

Two load-bearing details: `detections.status` distinguishes **auto / suggested / dismissed /
manual**, and `manual` means *a human put it on screen* — the self-calibrating router learns from
that column. And `verses.embedding` **exists and has never been written to**; it is the seam for a
neural embedder, not a live feature.

### A10. The API / integration surface

There is **no public HTTP API and no cloud service.** The integration story is:

- **OBS / vMix / kiosk browser source** → `http://<host>:8032/output.html?channel=<id>&template_id=<n>`.
  The URL is **channel-keyed**: changing a screen's template broadcasts a retemplate message the
  output applies by matching its own `channel`. A hand-built `?template_id=`-only URL renders
  fine and then silently never follows a template change.
- **The preacher's stage remote** → `http://<host>:8032/stage.html`.
- **The kiosk WebSocket hub on :8031** carries `{kind: "content" | "clear" | "black" | "stage_next", …}`.
- The LAN surface is **unauthenticated, broadcast-only, bounded, and honestly documented** in
  `PRIVACY.md` and `docs/DECISIONS.md` §35 — where it is recorded that it is a **control plane**,
  not a window, and the docs previously described it wrongly.
- **NDI is parked.** `render_target = 'ndi_encode'` is a valid channel type and
  `open_ndi_output` returns a clear error, because it needs a proprietary SDK. Do not fake it; do
  not delete the seam.

Internally, the frontend↔core contract is **135 registered `#[tauri::command]`s**
(`grep -c '#\[tauri::command\]' src-tauri/src/main.rs`) and a fixed event catalog:
`audio://chunk`, `audio://quality`, `audio://error`, `stt://transcript`, `detection://match`,
`output://content`, `output://clear`, `output://black`, `output://panic_failed`, `nav://blocked`,
`template://updated`, `model://progress|done|error|cancelled`.

### A11. Current screens and information architecture

Eight tabs — **Live · Outputs · Templates · Themes · Library · Planner · Settings · Help** — plus
two standalone pages (`Output.svelte` → the projector, `Stage.svelte` → the preacher's phone).
Rendered references are in `docs/design/`.

- **Live IS the console.** There is no separate Console tab. Live merges the run surface with the
  service plan, because an operator running a plan on a separate tab could not see the AI's
  suggestions — and the preacher going off-script is the entire product.
- **Planner builds a plan (a Tuesday job) and cannot fire to an output** — it imports zero fire
  commands, structurally.
- **Themes are the style layer beneath templates:** a theme sets default `style` keys, a template
  overrides them per key, and `TemplateRender` resolves `style.themeRef` itself, so every surface
  is themed with no per-surface wiring.
- **The transport is mode-aware and says so.** `→` steps a plan SLIDE when plan content is on
  air, and walks the passage (VERSE) when a detected or manual verse is. The mode is printed,
  because the same key silently meaning two things is how the wrong thing reaches a congregation.
- The Outputs tab's internal key is still `channels` and its file is `Channels.svelte`; the label
  is what an operator reads.

### A12. Design system

Source of truth is `src/app.css`; `docs/DESIGN_SYSTEM.md` is the map. Console chrome (`px`, always
dark, developer-owned) and output surfaces (`cqw`, operator-owned via Themes → Templates,
possibly transparent) are **different systems and must not borrow tokens from each other**.

**Four colours carry a promise. Using one for decoration is a bug, not a taste question:**

| Colour | Token | Means, and only means |
|---|---|---|
| 🟠 Amber `#ffb000` | `--v-amber` | **ON AIR.** The congregation is looking at this right now. |
| 🟣 Amethyst `#8b5cf6` | `--v-amethyst` | **Rehearsal** — and, separately, all interactive chrome |
| 🔵 Cyan `#22d3ee` | `--v-cyan` | **A guess** — a paraphrase / semantic match. Never a heard reference. |
| ⚫ Grey `#6b7280` | `--v-grey` | **CUED** — where `→` resumes, and *not* on screen |

Accessibility state: focus traps on all five dialogs **with focus restore**, a real heading
structure, the suggestion feed / transport / errors all announced via `aria-live`, and every text
token passing WCAG AA. Roughly **150 lines of dead legacy CSS** remain deliberately — global,
unscoped rules with generic class names (`.tab`, `.dot`, `.live`) that live components still
carry, so deletion needs eyes on a running app.

### A13. Testing and quality apparatus

Re-measure before quoting; each number names its command.

| | Count | Reproduce with |
|---|---|---|
| Rust tests | 519 passing, 28 ignored | `cd src-tauri && cargo test` |
| Frontend tests | 594 passing, 0 skipped, 45 files | `npx vitest run` |
| `e2e.rs` tests | 28 (26 run, 2 ignored) | `cargo test e2e::` |
| Registered commands | 118 | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| `.svelte` files | 47 (22 views) | `find src -name '*.svelte' \| wc -l` |
| Schema tables | 18 | `grep -c 'CREATE TABLE' docs/data/schema.sql` |

CI runs both suites on **macOS and Windows**, plus `cargo fmt`, `clippy -D warnings`, the
detection scorecard (`eval.rs`, a 50-case labelled corpus scored **through the real router**,
failing the build above SPEC's 5% wrong-verse rate), and a release build.

Load-bearing testing facts a reviewer must not "tidy away":

- `qa.rs` owns **the** fixture. `bare_app()` is a fresh install and nothing else. A second
  fixture is how two suites start disagreeing about what a fresh install contains.
- `e2e.rs` drives the **real** commands against a real in-memory DB through the real router and
  pipeline; nothing is mocked but the window. Add a test there whenever you touch the fire path.
- `ipc.test.js` is the contract test: every Tauri command the frontend calls by string must exist
  in Rust.
- `vitest.config.js` **must** set `resolve: { conditions: ['browser'] }`, or Svelte hands the
  tests SSR stubs where `onMount` is a literal empty function and every load-on-mount test passes
  by doing nothing.
- `stt::realtime::live_transcript_latency` is the **only** rig that runs at the speed of speech.
- A component nothing renders is not covered, however green its tests —
  `node scripts/qa-inventory.mjs` traces the chain to a control that actually renders.

Six QA agents (`relay-qa-{cold-start,live-path,surface,detection,failure,auditor}`) and the
`/qa-audit` command are documented in `docs/QA_HARNESS.md`. **Read its Part 4 before filing
anything** — it is the evidence baseline, and an agent that skips it "finds" bugs that are fixed.

### A14. Release, signing, and update

- The version lives in **three files** (`tauri.conf.json`, `package.json`, `Cargo.toml`), and
  `tauri.conf.json`'s copy is what the updater manifest advertises. If they drift from the tag,
  `latest.json` stamps the new build with the OLD version, every install compares equal, and
  **nothing ever updates — silently.** `npm run version:set` owns all three; CI asserts agreement
  on every PR; the release gate asserts they equal the tag.
- **A release is signed per-platform, or not at all.** One global "is it signed?" flag *was* the
  bug: it tested `APPLE_CERTIFICATE` and shipped an unsigned Windows MSI in silence.
- **macOS: the microphone dies on the first correctly-signed build** unless
  `relay.entitlements` carries `com.apple.security.device.audio-input` and `Info.plist` carries
  `NSMicrophoneUsageDescription`. Invisible under `tauri dev` and unsigned pre-releases.
- Pre-release versions must be **numeric** (`0.1.0-1`, not `0.1.0-rc1`) — the Windows MSI bundler
  rejects a named identifier fifteen minutes into a release.
- **Nobody has ever watched an update install.** The path is capable; capable is not observed.

### A15. Privacy, security, legal

- **Nothing you say, sing or show leaves your computer.** No accounts, no cloud, no server. The
  audio is never even saved. The speech model runs on the machine.
- Crash reporting is **off** and stays off unless enabled; there is no DSN in OSS builds; free
  text is **dropped**, not sifted.
- `PRIVACY.md` discloses the unauthenticated LAN broadcast honestly rather than hiding it.
- `SECURITY.md` names the two worst classes of vulnerability for this product: anything that
  sends content off the device, and anything that can put content on a screen the operator did
  not choose. Third: anything that compromises the update channel.
- `docs/AI_DISCLOSURE.md` states what the AI decides alone, what it will **never** do, and where
  it is honestly weak.
- KJV only, with no import path for another translation — so no licensing exposure today. Any
  modern translation (NIV, ESV, MSG, TPT) is a **licensing** problem before it is a code problem.
- GDPR / UK GDPR / CCPA: there is no controller, no processor, no transfer and no data subject
  request surface, because no personal data leaves the device. Compliance certifications
  (SOC 2, HIPAA, government) are **not applicable** — there is nothing to certify.

### A16. What has already been decided — do not re-litigate without a human

`docs/DECISIONS.md` holds 52 numbered decisions with reasoning. The ones that constrain any
transformation proposal:

1. **No native SDI hardware integration.** Ever, unless a human explicitly reopens it. NDI + HDMI
   only; SDI setups are served by bridging hardware the church already owns.
2. **Offline-first.** Every core feature works with zero internet. Cloud STT is an optional
   fallback, never a requirement.
3. **Operator override is a first-class control, never a fallback UI.** One action, at every
   stage.
4. **Output channels are render targets of one shared template engine.** If you write
   `if channel_type == "stage"` in rendering logic, that is a template configuration problem, not
   a code problem.
5. **Local-first data.** Nothing leaves the device without an explicit, visible reason.
6. **Only `DetectionMethod::Direct` may auto-fire** — and "Direct" means Relay *heard* it, not
   that it parsed confidently. Semantic / Ambiguous / **UncertainBook** are capped at `Suggest`
   at any score. Do not "fix" this by raising a number.
7. **Rehearsal gates at the broadcast choke point, not at the caller**, so every *future* caller
   is sandboxed by construction.
8. **A panic control may never report a success it did not achieve.**
9. **The operator must be shown which KIND of claim the AI is making.** A paraphrase shows **no
   percentage at all** — a TF-IDF cosine is not a probability, and a number that lies is worse
   than no number.
10. **Audio levels are learned, never assumed.** Three individually-reasonable absolute
    thresholds together made Relay *deaf to a quiet preacher, silently* — 94% voiced at studio
    level, **2%** at a church-laptop level.
11. **A per-screen template is authoritative; a content-look default defers to it.** A content
    look rides as an ID only and never broadcasts its template JSON (one embedded-image default
    was 13 MB and made verses take seconds).
12. **One window may inform the operator about several verses; it may put at most ONE on a wall.**
13. **Never make this path faster by making it less safe.** The latency work moved no threshold,
    removed no corroboration, and did not let a partial reference fire.

### A17. What is explicitly refused — and why the refusal is the product

These are in `docs/ROADMAP.md` §3 and `docs/PRODUCT_AUDIT.md` §13, marked **NOT APPLICABLE** with
reasoning. Recommending them without addressing the reasoning is a failed answer.

| Refused | Why |
|---|---|
| Cloud sync · online backup · church-to-church sync | Offline-first is the moat, not a constraint |
| Team collaboration · multi-tenancy · accounts · RBAC / SSO | There is one operator, standing in the room. No login. |
| AI analytics dashboards · advanced service reports | History stores everything and exports; a dashboard does not help the volunteer at 10:29 on Sunday |
| Plugin marketplace · community template sharing · public API | Post-stability. A public plugin SDK is one of the fastest ways to destabilise a live tool. |
| Mobile companion apps | The preacher's phone is already served by the stage view |
| Volunteer management · licensing admin · billing | Relay is free/MIT. Not a church-management or SaaS product. |
| SOC 2 / HIPAA / government compliance | No data leaves the device — there is nothing to certify |
| Song-lyric / setlist *detection* | Separate subsystem, separate risk; would dilute the scripture-detection core |
| A general-purpose AI assistant | Scope discipline. One live-service workflow tool. |
| Rewriting the stack | Rust + Tauri + Svelte + SQLite is correct and would be chosen again |
| Replacing OBS | Relay sits above the AV chain |

### A18. Parked honestly — a built seam, not a built feature

Do not fake these, and do not delete the seam:

- **NDI output** — needs a proprietary SDK; `open_ndi_output` returns a clear error.
- **Neural paraphrase embedder** — TF-IDF is the current implementation behind
  `SemanticIndex::top_k`; `verses.embedding` exists and has never been written to. Swapping the
  interface is ~half a day; the real work is **recalibration** plus an offline embedding
  pipeline — call it a week. Only this would let a paraphrase earn the right to auto-fire, which
  it is correctly forbidden from doing today.
- **African-language STT fine-tunes** — unbuilt, *correctly*: you cannot evaluate a fine-tune
  without the sermon-audio corpus.
- **Yorùbá numerals** — subtractive (16 = *ẹrìndínlógún*, "four less than twenty"); Kiswahili and
  Hausa parse fully in-language, Yorùbá does not yet.
- **Document (PDF/PPTX) rendering** — stored as media pointers; slide extraction is a later phase.

### A19. The moat, stated without softening

Relay's African-language differentiator **today** is a hand-curated multilingual
**reference-parsing** table — 66 books × 3 languages in `src-tauri/data/book_aliases.json`, plus
in-language numerals for Kiswahili and Hausa in `numerals.json` — on top of **stock Whisper
base**. That table is real, tested, and more valuable than it sounds: *the moat was blocked on a
lookup table, not on machine learning.* A preacher saying **"Ẹ ṣí Jòhánù orí kẹta, ẹsẹ
kẹrìndínlógún"** with a perfect Yorùbá acoustic model behind them would previously have produced
a flawless transcript and **no verse on the wall at all**.

But: **no fine-tuned acoustic model ships. No native speaker has reviewed the aliases. Yorùbá
numerals are not parsed. Word error rate has never been measured, in any language.** Whisper's
training data contains under 600 hours of Yorùbá and Hausa combined out of ~117,000. The
measuring instrument exists (`stt::bench::wer` — Levenshtein over words, folding punctuation the
way the detector does, deliberately not clamped at 1.0 so a hallucinating decoder scores worse
than a silent one) and it is **calibrated and pointed at nothing**.

`docs/LANGUAGES.md` says all of this plainly. **Do not soften it.** And note the trap it records:
some book names are ordinary words — `Iṣẹ́` (Acts) means *work*, `Orin` (Song of Solomon) means
*song* — so only the full forms are listed, and a test fails the build if a bare everyday word is
ever added.

### A20. Known technical debt (carried deliberately, tracked, not fixed on sight)

| Debt | State |
|---|---|
| `main.rs` — 5,307 lines / 135 commands | No longer a correctness issue (the fire engine is generic over `tauri::Runtime` and covered by `e2e.rs`); a readability complaint |
| `Live.svelte` (1,877) and `stores/capture.js` (1,941) | The frontend mirror of the same concentration |
| `models.rs` name collision | It is STT-model *download*, not domain models |
| `db/mod.rs` (~2,230) mixes migrations, platform paths and inline tests | Cohesive but large |
| ~150 lines of dead legacy CSS | Cannot be deleted without eyes on a running app |
| First-run wizard cannot be re-run | Everything in it lives in Settings, but a skipper must know that |
| `docs/data/schema.sql` is hand-maintained yet compiled in | Needs a generated check that it and the `ensure_*` rungs agree |
| One dead-but-built command: `save_arrangement` | The wrapper exists; no component imports it, so a user cannot save a song arrangement. Recorded rather than hidden. |

### A21. The blockers that are not commits

Highest leverage, and not one is a coding task:

| Item | Unblocks | Cost |
|---|---|---|
| Windows code-signing certificate | Windows can ship at all | ~$10/month |
| **30 minutes of real sermon audio on tape** | Word error rate, the dormant STT bench, fine-tune evaluation, the decoder-bias-prompt question. *Every claim about the moat is currently an assertion.* | Time and a recorder; the audio never enters the repo |
| Native-speaker review | 66×3 aliases, Yorùbá numerals, three empty-on-purpose locale files | Free — a language contribution, no code |
| One observed end-to-end update install | Confidence the updater actually delivers a fix to a church | 30 minutes on a real machine |
| Watch one full service run by a non-author operator | The thing no engineering substitutes for | A real Sunday |
| Brand / name decision | The README still says "rename freely" | A decision, not a build |

### A22. Future vision

Relay becomes the AI content layer that sits above every church AV chain in its target markets:
scripture, song and media routed to any number of independently-styled screens, in the language
actually being preached, entirely offline, on hardware a church already owns — with a
community-maintained language layer that no funded English-first competitor can replicate,
because the contributions come from speakers rather than from a model vendor.

---

## PART B — OBJECTIVE

Treat this as a complete product transformation.

**Assume nothing. Question everything.** Every feature must justify its existence. Every screen
must earn its place. Every workflow should get faster. Every interaction should get clearer.
Every system should get simpler.

But **question inside the shape.** The refusals in §A17 and the decisions in §A16 were made with
reasoning, in writing, and several were paid for with real live failures. You may recommend
overturning one — that is a legitimate output — but only as an **explicit, separately-labelled
proposal to a human**, stating what breaks, what the reasoning was, and what evidence would
justify reversing it. Never assume one away as a default.

Every recommendation must improve at least one of: **usability, safety-under-live-conditions,
scalability, maintainability, accessibility, performance, security, privacy, sustainability, or
the honesty of the product about itself.**

**The bar to score against is "the first 10 churches" — a volunteer, in a dark booth, with no
training and no second take.** It is not Stripe, and it is not enterprise scale.

---

## PART C — PHASES

### Phase 1 — Product audit
What exists, what works, what does not, what users actually need, what should be removed,
redesigned, automated or simplified. Evaluate: product vision, user journey, navigation,
architecture, database, AI features, performance, accessibility, scalability, security, privacy,
technical debt, developer experience, sustainability model, analytics posture, documentation,
testing, deployment, infrastructure.

### Phase 2 — UX audit
Audit every screen and both output surfaces. Evaluate navigation, flow, cognitive load under
time pressure, visual hierarchy, information architecture, readability at a distance and in the
dark, accessibility, interaction design, search, filtering, forms, onboarding, settings,
dashboards, editors, notifications, loading / empty / error states, success feedback,
micro-interactions, and consistency. Identify confusing flows, duplicate pages, dead ends,
feature overload, hidden actions, and visual inconsistency.

**Specific to Relay:** audit the six safety distinctions the product must never blur —
Preview vs Programme, Cued vs On Air, Paraphrase vs Direct, Suggestion vs Auto-fire, Clear vs
Blackout, Rehearsal vs Live.

### Phase 3 — UI audit
Colour system, typography, spacing, grid, cards, buttons, tables, charts, forms, dialogs, icons,
illustrations, animation, dark mode, brand consistency, component reuse, visual balance,
modernity, production readiness. **Check the four promise-carrying colours are never borrowed for
decoration**, and that console tokens have not leaked into output templates or vice versa.

### Phase 4 — Feature audit
Classify every feature: **KEEP · IMPROVE · MERGE · SIMPLIFY · AUTOMATE · REPLACE · REMOVE.**
Explain every recommendation. Never remove functionality without justification. Explicitly flag
anything that is *built but unreachable* (see `save_arrangement`).

### Phase 5 — Information architecture
Rebuild the product structure: the tab set, the run surface, the build surface, settings, search,
help, the output/stage surfaces, and the first-run path. Everything should feel logical to
someone who has never opened it before and cannot ask anyone.

### Phase 6 — Workflow redesign
Optimise the Sunday-morning path first. Reduce clicks, friction, unnecessary decisions and
repetition. Introduce automation where it *measurably* reduces operator load without reducing
operator authority. Then optimise the Tuesday path (building a plan), which has different
constraints entirely.

### Phase 7 — AI opportunities
Identify where AI creates measurable value: detection quality, ranking, calibration, search,
recommendation, generation, editing, analysis, workflow automation, background processing, human
review, memory, and prompt/model management. **Only recommend AI where the value is measurable,
and state what would measure it.** Anything that would let a low-confidence claim reach a
congregation unattended is out of bounds by §A16.6.

### Phase 8 — Technical modernisation
Architecture, module boundaries, component architecture, IPC design, database, caching,
performance, background jobs, observability, CI/CD, testing, developer tooling, dependencies,
security. Respect the 34 hard-won rules in `CLAUDE.md` — each is a bug that reached, or would
have reached, a congregation.

### Phase 9 — Brand repositioning
The name, logo direction, tagline, visual identity, tone, messaging, value proposition, market
positioning, differentiation and competitive advantage. **This is a genuinely open decision with
a deadline: before the first church installs.** A complete rebrand is a legitimate recommendation.

### Phase 10 — Design system
Colours, typography, spacing, components, icons, tokens, dark mode, accessibility, motion,
responsive behaviour, reusable patterns — for **two** systems (console chrome and output
templates) that must not contaminate each other.

### Phase 11 — Deployment readiness (replaces generic "enterprise readiness")
Assess readiness for: a single small church; a multi-campus church with several machines; a
church with an existing OBS/ATEM/ProPresenter chain; a church on a low-spec Windows laptop; a
church with no reliable internet; and a non-English-preaching congregation. Assess
internationalisation, the language contribution pipeline, hardware variance, and the update
channel.

**Multi-tenancy, RBAC, SSO, audit logs, billing and global cloud deployment are NOT APPLICABLE
by recorded decision (§A17).** If you believe one should be reconsidered, say so in the dedicated
section of the output (§16 below), not by quietly assuming it.

### Phase 12 — Legal, privacy and AI transparency
Privacy, terms, cookie policy, licensing (both the MIT licence and **Bible translation
licensing**, which is the real constraint), AI transparency, security disclosure, accessibility
conformance, GDPR / UK GDPR / CCPA. Identify missing policies. Note that most compliance regimes
are inapplicable *because* nothing leaves the device — and that this is a property worth
defending, not an omission.

### Phase 13 — Product roadmap
Immediate fixes, quick wins, v1, v2, long-term vision, innovation opportunities, competitive
roadmap. Separate **"blocked on a commit"** from **"blocked on the world"** — the second category
currently dominates and any roadmap that ignores that is fiction.

### Phase 14 — Implementation strategy
Recommended order, dependencies, milestones, development phases, testing strategy, release plan,
migration strategy, risk mitigation. Every migration must be **retryable** (drop the scratch
table first, roll back on failure) — a mid-batch failure once bricked every subsequent boot,
forever, before the window was even shown.

---

## PART D — OUTPUT FORMAT

Produce the following sections, in this order.

1. **Executive assessment** — overall health, in plain language, including what is *not known*.
2. **Product scorecard** — score and explain each of: UX (live operation), UI / design language,
   core engine, architecture, performance, accessibility, security, privacy, scalability,
   developer experience, AI readiness, testing, distribution / install, onboarding / first-run,
   brand, sustainability model, documentation, legal compliance, and overall product maturity.
   Mark anything genuinely inapplicable as **N/A with reasoning**, not as a low score.
3. **Strengths** — what must be protected. Name what a rewrite would most likely sand off.
4. **Weaknesses** — what is holding the product back, separated into *fixable by typing* and
   *not fixable by typing*.
5. **Critical issues** — highest priority, with the failure scenario spelled out concretely:
   what a congregation would see.
6. **UX redesign recommendations** — screen by screen.
7. **UI modernisation plan** — the visual transformation strategy, respecting the colour
   promises.
8. **Feature matrix** — Keep / Improve / Merge / Automate / Remove / Future.
9. **Information architecture** — the new navigation and product structure.
10. **Technical modernisation** — architecture improvements, with the regression risk of each
    named.
11. **AI enhancement strategy** — where AI creates real, measurable value, and what measures it.
12. **Language and localisation strategy** — the moat: alias review, Yorùbá numerals, fine-tune
    evaluation, the contribution pipeline, and the WER baseline that does not exist yet.
13. **Brand refresh** — name, identity and positioning recommendations.
14. **Deployment readiness review** — operational maturity for the six deployment shapes in
    Phase 11.
15. **Legal, privacy and compliance review** — required policies and gaps, including Bible
    translation licensing.
16. **Recorded-decision challenges** — a dedicated, separately-labelled section for anything you
    believe should be reversed from §A16 or §A17. For each: the decision, its stated reasoning,
    what reversing it buys, what it costs, and **what evidence would justify the reversal.** If
    you have none, say so — an empty section is a valid and respectable answer.
17. **Prioritised roadmap** — Phase 1 → Phase N, with "blocked on a commit" and "blocked on the
    world" kept visibly separate.
18. **Production readiness checklist** — everything required before the first church installs.
19. **Success metrics** — measurable KPIs. For Relay these are *not* SaaS metrics. Define at
    least: verse-identification accuracy on direct quotes and on paraphrase, wrong-verse rate as
    a share of AI triggers, mic-to-projector latency (p50 and p95), transcript keep-up under
    real-time load, word error rate per language, operator override rate, suggestion
    accept/dismiss ratio, time-to-first-verse for a new operator, first-run completion rate,
    crash-free service rate, update install success rate, number of churches running a full
    service unassisted, and native-speaker-reviewed alias coverage.

---

## PART E — NON-NEGOTIABLE PRINCIPLES

Always:

- **Challenge assumptions rather than accepting them — but read the recorded decision first.**
  This project writes down *why*. An argument that does not engage the recorded reasoning is not
  an argument.
- **Recommend simplification before adding complexity.**
- **Reuse and standardise components.** One renderer, one store, one error humaniser, one
  fixture, one baseline threshold.
- **Design for long-term maintainability**, on a codebase maintained by very few people.
- **Explain the reasoning behind every major recommendation.**
- **Remove technical debt where it is safe to** — and leave it where the register says it is
  carried deliberately.
- **Optimise for performance, accessibility, security, privacy and scalability** — in that order
  of tie-breaking, with **live safety above all of them.**
- **Never make the live path faster by making it less safe.** Removing the wait in front of a
  safety rule is not the same as removing the rule, and only one of the two is allowed.
- **Never let the product report a success it did not achieve.** Software that lies to its
  operator is worse than software that fails in front of them, because the operator stops
  looking.
- **Be honest about what has never been measured.** If a claim about accuracy, latency or
  language quality is not backed by a named command that produces a number, say so in the
  recommendation itself.
- **Preserve the honest seams.** NDI returning a clear error, `verses.embedding` sitting unwritten,
  `LANGUAGES.md` refusing to soften the moat — these are features of the product's integrity,
  not omissions to tidy up.

The end result should be a product that feels intentionally designed from the ground up, where
every screen, workflow and system works together — and where the thing a volunteer trusts on a
Sunday morning is not the AI's confidence, but the fact that the software has never once told
them something that was not true.

=== END PROMPT ===

---

## Appendix 1 — If the brief ever changes to multi-tenant SaaS

This is **not** the current brief, and adopting it would reverse several recorded decisions. It
is written down because it is the most common thing a generic transformation prompt will
recommend, and the team should be able to evaluate it deliberately rather than drift into it.

**What would have to change**, at minimum:

| Area | Today | Under a SaaS brief |
|---|---|---|
| Identity | No accounts, no login | Organisation → workspace → user, with invitations and roles |
| Tenancy | One machine, one church, one SQLite file | Tenant isolation at the data layer; per-tenant encryption; a migration path from the local SQLite file |
| Data residency | Everything local, nothing transmitted | A processor relationship, a DPA, GDPR/UK GDPR data-subject rights, retention policy, sub-processor list — none of which exist today, precisely because nothing leaves the device |
| Custom domains | N/A | Per-organisation domain verification, and outbound email sent from the organisation's own domain (DKIM/SPF/DMARC per tenant) rather than the owner's |
| Notifications | None — the operator is in the room | Transactional email per tenant, with per-tenant sender identity and suppression handling |
| Billing | None; MIT and free | Subscription, seats or per-campus pricing, dunning, tax |
| Offline | The moat | Becomes a *sync* problem: local-first with conflict resolution, which is a materially harder product than either pure-local or pure-cloud |
| Audit | `detections` history, local | Tenant-scoped audit logs, retention, export |
| Security posture | Nothing leaves the device, so there is little to breach | A breach surface that did not previously exist; SOC 2 becomes relevant *because* the risk was created |

**The honest trade.** Every row above adds a failure mode on the path between a preacher's voice
and a projector — the one path this product exists to make reliable. A SaaS brief is not
inherently wrong; it is a different product with a different moat, and it should be adopted, if
ever, as a **recorded decision with a stated reason**, not as an inference from a template.
The natural shape if it is ever wanted is a **hybrid**: the live engine stays entirely local and
offline, and only *non-live* artefacts (plans, songs, templates, announcements) optionally sync
through an organisation account. That preserves the moat and confines the new failure modes to
Tuesday rather than Sunday.

## Appendix 2 — Prompt variants

- **Audit-only run.** Use Parts A, C (Phases 1–4) and D (sections 1–5), and ask for nothing else.
  Good for a pre-release health check.
- **Single-surface run.** Replace Part C with one phase, and give the model only the relevant
  documents plus the screen reference from `docs/design/`.
- **Adversarial run.** Append: *"Argue the case that this product should not ship at all in its
  current form. Then argue the opposite. Then state which case is stronger and why."* The
  existing release decision is NO-GO, so the model should be able to reconstruct that reasoning
  from Part A alone — if it cannot, Part A has drifted from the truth and needs updating.
- **Language run.** Give the model `docs/LANGUAGES.md`, `src-tauri/data/book_aliases.json` and
  `numerals.json`, and ask only for §12 of the output format.
