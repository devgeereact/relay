# Relay — Data Model & Event Architecture

*What Relay **is**, as opposed to what it does.* This document centralizes the entities, their
lifecycle, the invariants that govern them, and the events that connect the Rust core to the
webview. It is the reference the database schema, the IPC surface, and the AI pipeline all
descend from.

Entities in Relay live in **three places by design**, and this doc names all three so you never
have to guess which one you're holding:

1. **Persisted shape** — a `#[derive(Serialize)]` row struct in `src-tauri/src/db/*.rs`, one
   file per table-family. This is the on-disk truth.
2. **Runtime shape** — a struct that lives *with its algorithm* (detection, routing, the fire
   pipeline). Often richer or narrower than the row; it exists only in memory during a service.
3. **Live-session state** — `Session` / `SessionState` in `main.rs`, the ephemeral "what is
   happening right now" that is never persisted verbatim.

For the *why* behind any rule here, follow the link to [DECISIONS.md](DECISIONS.md); for the
end-to-end wiring see [ARCHITECTURE.md](ARCHITECTURE.md); the canonical DDL is
[data/schema.sql](data/schema.sql). Line numbers drift — each anchor names the **file and the
symbol**, which is what to grep for if the line has moved.

---

## 1. Entity map

```
Reference data            Presentation              Live service
─────────────             ────────────              ────────────
Translation ──┐           Template ──┐              Service
Verse ────────┘           OutputChannel             ├─ Transcript (STT output)
(bundled KJV)             (render target)           ├─ Detection  (AI/manual decision)
                                                     └─ Cue        (operator action log)
Content Library           Service Plan              Calibration
───────────────           ────────────              ───────────
SavedScripture            ServicePlan               VoiceProfile
Song ─ Section            └─ PlanItem  ── the unified cue    EnvironmentProfile
     └ Arrangement           (scripture│song│media│announcement│countdown)   (a room)
Announcement
MediaAsset

The service record                       Safety state (runtime)
──────────────────                       ──────────────────────
ServiceEvent  (an ordered timeline       ServiceLock   (what may not happen now)
               that survives the app)    OutputHealth  (is each screen still painting?)
PerfSample    (percentiles, not traces)  Degradation   (which capability is reduced)

Runtime-only (never persisted): VerseRef · RefMatch · Cand · Fire · DetectionEvent ·
ContextMemory · PassageNav · SemanticIndex · Thresholds · OutputContent · SessionState ·
ServiceLock · OutputHealth
```

**Three of these carry a rule the schema alone does not state.** `ServiceEvent.detail` is a
phrase Relay composes and never a phrase a preacher said; `PerfSample` stores percentiles and
never traces; and `EnvironmentProfile` deliberately omits the audio levels, because nothing in
Relay may compare a signal to a stored level ([DECISIONS.md](DECISIONS.md) §19, §44, §46).

---

## 2. Reference data

### Translation — `db/verses.rs` `Translation`
The Bible translation a verse belongs to. Bundled corpus today is **KJV only** (66 books,
31,100 verses, committed at `src-tauri/data/kjv.json`, glosses stripped at import). Fields:
`id, name, abbreviation, language` (ISO code), `license_type`. There is deliberately no import
path for a second translation — which is also why there is no licensing exposure.

### Verse — `db/verses.rs` `VerseRow`
A single verse: `translation_id, book` (canonical name), `chapter, verse, text`, and
`embedding BLOB`. **The `embedding` column exists and has never been written to** — it is the
waiting seam for a future neural paraphrase model (see [KNOWN_ISSUES.md](KNOWN_ISSUES.md)). Lookup is
indexed by `(translation_id, book, chapter, verse)`; full-text recall rides an FTS5 virtual
table `verses_fts` *behind* the reference/phrase/semantic ranker (DECISIONS: "FTS5 added behind
the existing ranker").

**`VerseRef`** (`detection.rs` `VerseRef`) is the runtime counterpart — a parsed `book /
chapter / verse` pointer with no text attached. It is what the detector produces and the
canonical key `"Book C:V"` is derived from it via `Fire::key_for` (`pipeline.rs`), the *one*
definition so the debounce key and the displayed reference can never disagree.

---

## 3. Presentation

### Template — `db/templates.rs` `Template`
The look of what a congregation sees: `region_config_json` (layout regions) + `style_json`
(fonts, colours, transitions) + `console_active` (one of up to four styles shown on the console
output grid). **One renderer honours every template** — `src/lib/TemplateRender.svelte` drives
the editor preview, the console wall, *and* the real output, so WYSIWYG is true by construction
(DECISIONS; [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md)).

### OutputChannel — `db/channels.rs` `OutputChannel`
A configured destination, not a branch in logic. `render_target ∈ {native_window, ndi_encode,
network_client}`, an assigned `template_id`, a `display_target` (display index / NDI name /
kiosk id), and `status ∈ {online, offline}`. **Any channel can carry any template**; the render
target and template are *configuration*, never per-type code (a core non-negotiable). `ndi_encode`
is a valid target in the schema but returns a clear "not built" error at runtime — an honest
seam, not a lie (see [KNOWN_ISSUES.md](KNOWN_ISSUES.md)).

**`OutputContent`** (`channels.rs` `OutputContent`) is the runtime payload actually broadcast to
a channel; **`MonitorInfo`** (`channels.rs` `MonitorInfo`) is the stage-monitor view.

---

## 4. The unified cue model — the most important shape in the product

Scripture, song, media, announcement, and countdown **all reduce to one polymorphic cue** so
the Planner, the monitors, and the one renderer never branch per type. Adding a content type is
a new payload shape, not new plumbing (DECISIONS, "Build-out decisions").

### ServicePlan / PlanItem — `db/plans.rs` `PlanSummary`, `PlanItem`
A `ServicePlan` (`service_plans` table) is an ordered list of typed cues. A **`PlanItem`** is
the cue:

```
PlanItem { id, plan_id, position, cue_type, label, payload_json, template_id? }
              cue_type ∈ { "scripture", "song", "media", "announcement", "countdown" }
```

The `cue_type` selects how `payload_json` is interpreted; `template_id` is an optional
per-content-type override (§3). There is **no separate `Cue` struct** — a cue *is* a `PlanItem`
row on the build side, and a `Fire` on the run side (§6). The frontend mirror is
`src/lib/cues.js`.

**Snapshot vs. reference, per type** (DECISIONS): scripture **live-resolves** from its reference
at fire time, so it is always the current text/translation. Songs and announcements **snapshot**
into the cue for offline reliability — but Library edits **propagate** back into plans
(`sync_*_in_plans`), so a snapshot is never stale. Best of both, with no per-type special case
at fire time.

**Arrangements** (`db/songs.rs` `Arrangement`) are named play-orders stored as *section-index
sequences* (`sequence TEXT`, default `'[]'`), not copied lyrics — so a lyric edit re-expands
into the right (possibly repeated) slots. "Standard" is implicit and never persisted.

### Library entities
- **Song / SongSection / SongSummary / Arrangement** — `db/songs.rs`. A song is a title +
  metadata (`author, ccli, song_key, bpm, tags`) with ordered `song_sections` (`tag, label,
  lyrics`) and zero or more `song_arrangements`.
- **SavedScripture** — `db/library.rs` `SavedScripture`. A pinned verse (`reference UNIQUE,
  book, chapter, verse, text, translation`).
- **Announcement** — `db/library.rs` `Announcement` (`title, body`).
- **MediaAsset** — `db/library.rs` `MediaAsset` (`kind, filename, path`). Served over LAN at
  `/media/<id>` with digit-prefix-id path-traversal defence (DECISIONS, LAN-bind decision).

---

## 5. Live service

### Service — `db/services.rs` `ServiceSummary`
One row per run (`date, title`). Parent of the two live logs:

- **Transcript** — `db/services.rs` `TranscriptRow` (`service_id, timestamp, text, language,
  confidence`). The rolling STT output; `language` is per-chunk because **code-switching is the
  normal case**, not an edge case.
- **Detection** — `db/services.rs` `ServiceDetection` (persisted in `detections`). Every
  AI-or-human decision: `transcript_id, verse_id?, method ∈ {direct, semantic}, confidence,
  status ∈ {auto, suggested, dismissed, manual}, fired_at?`.

  > **Invariant — the `status` column is training data.** `'manual'` means a **human** put this
  > on screen (override, confirmed suggestion, or next/back nav) — it is *not* an AI decision
  > and must never be counted as one. The self-calibrating router learns from this column;
  > `persist_fire` takes the real status. (CLAUDE §14; DECISIONS "Manual fires are recorded as
  > `status='manual'`".)

- **Cue (action log)** — `cues` table (`service_id, type, payload_json, triggered_at`). This is
  the *operator-action* log for a running service (`manual_override`, `clear_screens`,
  `template_change`) — distinct from a `PlanItem` build-time cue. Same word, two lifecycles;
  the model keeps them apart.

### SessionState / Session — `main.rs` `SessionState`, `Session`
The ephemeral "right now": which service is live, current passage, position. Held as
`Session(Mutex<Option<SessionState>>)`. It is state, not a record — never persisted verbatim;
what survives a crash is the frontend's own position store (`src/lib/session.js`) plus the
`services` / `detections` / `cues` rows. **Global lock order is `Db` before `Session`,
everywhere** (CLAUDE §6).

---

## 6. The fire path — how a decision becomes pixels

This is the one path that puts scripture in front of a congregation. It has exactly one
pipeline, and everything joins it — five hand-rolled copies once drifted apart and two silently
dropped the scripture template, which is why **nothing may build an `OutputContent` or a
`DetectionEvent` by hand** (CLAUDE). The types, in order:

**`Cand`** (`pipeline.rs` `Cand`) — a gate candidate: the anchor `VerseRef`, its confidence,
`method`, and passage span (`verse_end`, `whole_chapter`), plus `matched` (the evidence). The
router decides on `Cand`s.

**`Fire`** (`pipeline.rs` `Fire`) — a verse about to be shown, everything resolved: the
`reference`, canonical `key`, resolved `verse_id / text / translation` (all `Option` — a
reference can parse cleanly and still not be in the corpus, and the console still shows it was
heard), `confidence`, `method`, `status`, the operator's private `stage_note` (rides to the
stage monitor, **never** the congregation), the **scripture `template_id` + `template_json`
that every fire path must carry**, and `matched_text` — *why* the machine thinks this verse.

**`DetectionEvent`** (`pipeline.rs` `DetectionEvent`) — what crosses the IPC bridge to the
console: the same facts plus `in_library` and `matched_text`. It carries `method` and
`matched_text` precisely because *a heard reference and a paraphrase guess are different kinds
of claim on incomparable scales*, and the operator who can overrule the AI must see which one
they're being offered (DECISIONS §21; CLAUDE §18).

### The two enums that carry the safety story
- **`DetectionMethod`** (`detection.rs`): `Direct` (a spoken reference the parser heard;
  confidence is a real parse confidence — **may auto-fire**) · `Semantic` (a TF-IDF cosine — a
  distance in an arbitrary space, **not a probability, may never auto-fire**) · `Ambiguous` (a
  reference that parsed but is genuinely ambiguous, capped at suggest).
- **`FireStatus`** (`pipeline.rs`): `Auto` (AI, unprompted, above threshold) · `Suggested` (AI
  offered it; **not on screen**, waiting for a human) · `Manual` (a human did it).

### Thresholds — `router.rs` `Thresholds`
The gate's numbers: `{ auto_fire, suggest }`. There is exactly **one baseline, by
construction** — `Thresholds::default()` is *defined as* `from_sensitivity(DEFAULT_SENSITIVITY)`
where `DEFAULT_SENSITIVITY = 50`, which yields `auto_fire 0.50 / suggest 0.35`. The dial and the
default cannot drift apart; two baselines once existed, disagreed (0.50/0.35 vs 0.90/0.60), and
a profile save silently reset the gate from one scale to the other (DECISIONS "One threshold
baseline"; CLAUDE). **Confidence thresholds are configuration, never constants — never
introduce a second baseline.**

### VoiceProfile — `db/profiles.rs` `VoiceProfile`
Per-preacher calibration that persists accent + gate learning across services:
`name, language?` (null = auto-detect / code-switch), `sensitivity` (0–100 dial →
threshold baseline), the live feedback-adapted `auto_fire / suggest`, `bias_terms` (extra
decoder-bias vocabulary), and `is_active` (exactly one row active at a time). Saving a profile
re-derives thresholds **only when the sensitivity dial actually moved** — so a rename can't wipe
calibration.

### The gate, in one paragraph (`router.rs` `Router::decide`)
Detections from `detection.rs` become `Cand`s. **Only `DetectionMethod::Direct` may auto-fire**
— `Semantic` and `Ambiguous` are capped at `Suggest` *before any threshold is consulted*, at any
score, at any sensitivity (DECISIONS §"Only Direct may auto-fire"; CLAUDE §10). A property test
sweeps every sensitivity × every confidence to prove it. This makes "the AI put the wrong verse
on the wall" close to **unrepresentable**, not merely unlikely — the single best decision in the
product. A `Fire` that clears the gate goes through `pipeline` → an `OutputContent` broadcast +
a `DetectionEvent` — and **the fire engine is generic over `tauri::Runtime`** so this exact path
is driven headlessly by `e2e.rs` (CLAUDE §24). Keep new fire-path code generic.

### Position vs. on-air — the frontend's `liveCue`
`liveCue = { cueId, slide, onAir }` — **position and on-air-ness are separate facts** (CLAUDE).
Panic keys clear only `onAir`; wiping the position would restart the plan at cue 1 on the next
`→`. A cue that is where `→` resumes but is *not* on screen reads **CUED** in grey — never amber
(amber means live and is never allowed to lie; see [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md)).

---

## 7. Domain invariants (the rules the model must never violate)

Consolidated here as *domain facts*; the code-level statements and their war stories live in
[../CLAUDE.md](../CLAUDE.md) "Architecture rules learned the HARD WAY" and
[DECISIONS.md](DECISIONS.md) — the numbered log now runs §18–§62. This list references
those decisions; it does not fork them.

1. **Only `Direct` auto-fires.** A cosine is not a probability. `Semantic`/`Ambiguous` are
   capped at `Suggest` structurally, not numerically.
2. **One threshold baseline**, `Thresholds::default() == from_sensitivity(50)`, by construction.
3. **Every fire goes through `pipeline::Fire`.** Never hand-build an `OutputContent` or
   `DetectionEvent`.
4. **`matched_text` + `method` cross the bridge and are shown.** The operator must see *which
   kind* of claim the AI is making. A paraphrase shows **no percentage at all**.
5. **`detections.status = 'manual'` for every human action.** The router trains on that column.
6. **A panic control never reports a success it didn't achieve.** `clear_screens` / `blackout`
   return `Result`; the frontend returns a boolean **and** sets `panicError` (DECISIONS §20).
7. **`liveCue` keeps position ⟂ on-air.** Panic clears on-air only.
8. **Locate files only via `db::app_data_dir()`.** No hand-rolled `$HOME/…` path — Windows has
   no `HOME` and a day-one platform ran with STT silently dead because of one.
9. **Lock order `Db` before `Session`; never emit under a lock** — deadlocks the macOS run loop.
10. **`detection.rs` is DB/IO-free and pure** — keep it that way; it is the heavily-tested core.

---

## 8. Event & command architecture

The Rust core and the Svelte webview talk over two channels: **commands** (request/response,
`#[tauri::command]`, all in `main.rs`) and **events** (push, `handle.emit`). **The count is
deliberately not written here** — restated counts in this repository have drifted every time
(`grep -c '#\[tauri::command\]' src-tauri/src/main.rs`). The command reference lives in
[ARCHITECTURE.md](ARCHITECTURE.md) §6, which no longer restates the list either, for the same
reason. What follows is the **event catalog**: the push side, which is where the live pipeline
actually surfaces.

| Event | Producer → Consumer | Carries / means |
|---|---|---|
| `audio://chunk` | audio/dsp → console meters | a captured, cleaned audio frame (level display) |
| `audio://quality` | dsp → Settings/Live | front-end quality metrics (gain, noise) |
| `audio://error` | audio engine → console | device/capture failure (start is non-blocking; errors arrive here) |
| `stt://transcript` | stt worker → console | a rolling partial/final transcript line |
| `detection://match` | pipeline → console | a `DetectionEvent` — the AI's suggestion or auto-fire |
| `output://content` | channels → outputs | an `OutputContent` to render (the verse hits the wall) |
| `output://clear` | channels → outputs | clear the screens (a panic control; success is a `Result`) |
| `output://black` | channels → outputs | blackout (a panic control) |
| `output://panic_failed` | channels → shell | a fire-and-forget panic path could not complete — raises the global banner |
| `output://error` | channels → shell | a non-panic output failure |
| `nav://blocked` | nav → console | a `→`/`←` could not move, with the reason (a boundary is not a failure) |
| `template://updated` | templates → renderer/editor | a template changed; re-render live |
| `model://progress` | models → Settings/first-run | STT model download progress |
| `model://done` | models → Settings/first-run | download complete and checksum-verified |
| `model://error` | models → Settings/first-run | download failed (dismissable) |
| `model://cancelled` | models → Settings/first-run | operator cancelled — **not** an error; keeps the `.part` |
| `stt://language_unstable` | stt worker → console | auto language detection is flapping — the operator should know before blaming the AI |
| `channel://retemplate` | main → native output window | this screen's template changed; the page filters by its own `channel` id so a swap is live with no new URL (DECISIONS §29) |
| `rehearsal://changed` | main → every console surface | rehearsal on/off, pushed so no surface can be a poll interval behind the others |

**`model://done` is emitted and deliberately has no listener.** `download_model`
resolves only once the file is installed and checksum-verified, so the command's own
return is the completion signal; adding a listener would handle the same fact twice.
It stays because the trio `done`/`cancelled`/`error` is the protocol `models.rs`
documents, and an outbound event with no consumer costs nothing — unlike a *command*
nothing calls, which is attack surface (RG-51).

Two events encode safety, not just plumbing: `output://panic_failed` exists because the panic
controls fire from a global keydown handler and a shell button that **cannot `catch`** — a
silent failure there is the exact class of lie this product refuses (DECISIONS §20). And
`nav://blocked` carries a *reason* (`EndOfPassage` / `NoPassage` / `NotInLibrary`) because
reaching the end of a passage is a correct boundary and the operator is entitled to know which
— `nav` used to return `()` and silently do nothing.

---

## 9. Where each entity lives (quick index)

| Entity | Persisted (`db/`) | Runtime | Notes |
|---|---|---|---|
| Translation / Verse | `verses.rs` | `VerseRef` (`detection.rs`) | KJV bundled; `embedding` unwritten |
| Template | `templates.rs` | — | one renderer honours it |
| OutputChannel | `channels.rs` | `OutputContent`, `MonitorInfo` (`channels.rs`) | render target, not a branch |
| ServicePlan / PlanItem | `plans.rs` | `Fire` at run time (`pipeline.rs`) | the unified cue |
| Song / Section / Arrangement | `songs.rs` | — | arrangements = index sequences |
| SavedScripture / Announcement / MediaAsset | `library.rs` | — | Library content |
| Service / Transcript / Detection | `services.rs` | `Cand`, `DetectionEvent` (`pipeline.rs`) | `status='manual'` is training data |
| VoiceProfile | `profiles.rs` | `Thresholds` (`router.rs`) | one baseline by construction |
| ServiceEvent / PerfSample | `services.rs` | — | the ordered record and its percentiles; **neither may carry what a preacher said**, pinned from both sides |
| EnvironmentProfile | `environments.rs` | — | a room, applied back one piece at a time so a partial apply reports *which* piece did not take |
| Session | — (`main.rs` `SessionState`) | `SessionState` | ephemeral; never persisted verbatim |
| ServiceLock | — | `servicelock.rs` | 16 named actions held back while recording; **nothing on the fire path** |
| OutputHealth | — | `channels.rs` | per-channel liveness from an anonymous beat; a lost beat degrades to "silent", the safe direction |
| ContextMemory / PassageNav / SemanticIndex | — | `detection.rs` | pure, DB-free detection state |
