# Relay — Roadmap, Deferrals & Technical-Debt Register

Everything Relay has *chosen not to build yet*, why, and on whose authority — plus the debt it
knowingly carries. This exists so a deferral is a recorded decision, not a gap someone
"discovers" and quietly starts building.

The governing rule, and the test every item on this page was measured against:

> **If a feature does not make Sunday morning smoother, it waits.** Optimise for the **first 10
> churches** — a volunteer, in a dark booth, with no training and no second take — not for
> enterprise scale.

For the current health scoring behind these calls, see [PRODUCT_AUDIT.md](PRODUCT_AUDIT.md); for
the reasoning that first set them, [DECISIONS.md](DECISIONS.md).

---

## 1. Blocked on the world, not on a commit

The highest-leverage items, and **not one of them is a coding task** (PRODUCT_AUDIT §1). The
code that would consume each already exists.

| Item | Unblocks | Cost |
|---|---|---|
| **Windows code-signing certificate** | Windows can ship at all — the release gate now *refuses* to publish an unsigned Windows build (DECISIONS §23). Windows is the target market's dominant platform on cost grounds. | ~$10/month (Azure Trusted Signing) |
| **30 minutes of real sermon audio on tape** | Word error rate (never measured, any language), the dormant STT bench (`stt::bench`, already scores through the real detector), the fine-tune evaluation, and the decoder-bias-prompt question. **Every claim about the moat is currently an assertion.** | Time + a recorder; audio never enters the repo (`bench/.gitignore` refuses it) |
| **Native-speaker review** | The 66×3 book aliases (unreviewed), Yorùbá numerals (unparsed, subtractive), and the three locale files (ship empty *on purpose*). This is the actual moat and no native speaker has read it. | Free — a language contribution, no code (see [../CONTRIBUTING.md](../CONTRIBUTING.md), [LANGUAGES.md](LANGUAGES.md)) |
| **One observed end-to-end update install** | Confidence that the updater mechanism — capable but never watched — actually delivers a fix to a church. | 30 minutes on a real machine ([RELEASING.md](RELEASING.md)) |
| **Watch one full service run by a non-author operator** | The one thing no amount of engineering substitutes for. | A real Sunday |
| **Brand / name decision** | README still says *"Working name — rename freely."* Decide **before** the first church installs, not after. | A decision, not a build |

---

## 2. Parked, honestly (built seam, not built feature)

These have a deliberate seam in the code and an honest "not built" signal — they are *paused*,
not missing. Do not fake them; do not delete the seam.

- **NDI output.** `render_target = 'ndi_encode'` is a valid channel type, but `open_ndi_output`
  returns a clear error — it needs a proprietary SDK (Blackmagic/NDI). Parked, and honest about
  it. Bridge to NDI/SDI with gear the church already owns (ATEM, converters).
- **Neural paraphrase embedder.** Paraphrase detection is TF-IDF today. The seam is
  `SemanticIndex::top_k` (`detection.rs`), and the `verses.embedding` column exists and **has
  never been written to** ([DOMAIN_MODEL.md](DOMAIN_MODEL.md) §2). Swapping the interface is ~½
  day; the real work is *recalibration* — the TF-IDF floors (`SEMANTIC_FLOOR`, the router's
  `suggest`) are tuned to a cosine whose baseline differs from a neural one — plus an offline
  embedding pipeline. Call it a week. Only *this* would let a paraphrase earn the right to
  auto-fire, which it is (correctly) forbidden from doing today.
- **African-language STT fine-tunes** — the stated moat, and still unbuilt, *correctly*: you
  cannot evaluate a fine-tune without the sermon-audio corpus (§1). Today's differentiator is a
  hand-curated multilingual **reference-parsing** table on stock Whisper base — real, tested,
  and honestly described in [LANGUAGES.md](LANGUAGES.md). Do not soften that.
- **Yorùbá numerals.** Swahili and Hausa parse in-language; Yorùbá is subtractive
  (16 = *ẹrìndínlógún*) and not yet parsed. Yorùbá is the largest addressable church market in
  the tier-1 list.
- **`related_scripture`** — built, registered, and now *surfaced* in the Intelligence Feed
  (previously zero callers). Keep it earning its place or delete it; dead built code rots.

---

## 3. Deferred features — valuable, but not for the first 10 churches

Recorded as **deferred**, with reasoning — this is where the "Engineering Blueprint" essay's
larger ambitions (plugin SDK, analytics, cloud) live until the core Sunday experience is
flawless. None of them make Sunday morning smoother today.

### Extensibility / plugins — *design for it, don't expose it in v1*
A public plugin SDK is powerful and one of the fastest ways to destabilise a live tool. For the
first releases Relay keeps **complete control** of performance, security, stability, and AI
safety. When it is opened, the intended seams (in priority order) are **Output** (OBS/NDI/LED/
projectors — note the existing WebSocket + LAN-HTTP integration *is* the current output-plugin
story), **Hardware** (Stream Deck / MIDI / ATEM / Companion), **AI** (speech models / language
packs / intent), and **Automation** (workflows / webhooks). **Plugins must never modify the
presentation engine or the operator interface directly** — that is what keeps the live
experience consistent and reliable. Not v1.

### Explicitly not being built (and not a gap — a decision)
Each contradicts the offline-first, single-operator, one-church-one-machine shape of the product
(PRODUCT_AUDIT §13). These are the shape of Relay, not omissions from it:

| Deferred / declined | Why it waits (or never comes) |
|---|---|
| Cloud sync · online backup · church-to-church sync | Offline-first is the moat, not a constraint. Nothing leaves the device without a visible reason. |
| Team collaboration · multi-tenancy · accounts · RBAC / SSO | There is one operator, standing in the room. No login. |
| AI analytics dashboards · advanced service reports | History already stores everything; a raw export exists. A dashboard doesn't help the volunteer at 10:29 on Sunday. |
| Plugin marketplace · community template sharing · public API | Post-stability. The API story today (OBS/kiosk over WebSocket + LAN HTTP) is deliberate and sufficient. |
| Mobile companion apps | The preacher's phone is already served as a stage view; a full companion app is not v1. |
| Volunteer management · licensing administration · billing | Relay is free/MIT. Not a church-management or commercial-SaaS product. |
| Compliance (SOC2 / HIPAA / gov) | No data leaves the device — there is nothing to certify. |
| Song-lyric / setlist *detection* | Separate subsystem, separate risk; would dilute the scripture-detection core that differentiates the product. |
| A general-purpose AI assistant | Scope discipline. Relay is one live-service workflow tool, not a platform. |

### Candidate, undecided
- **Post-service summary.** History stores everything; today's export is a raw markdown dump,
  not a summary. Maybe — low priority.

---

## 4. Technical-debt register (known, accepted, not actioned)

Carried deliberately. Listed so it is *tracked*, not so it is fixed on sight — several of these
were looked at and left for a stated reason.

| Debt | State & reasoning | Effort if actioned |
|---|---|---|
| **`main.rs` — 2,922 lines / 101 commands** | A god-file holding the whole IPC surface + app state + some orchestration. **No longer a correctness issue** — the fire engine is generic over `tauri::Runtime` and covered by `e2e.rs`, which was the actual point of "split main.rs" (PRODUCT_AUDIT §10.2). A readability complaint now. | Medium; split commands into per-domain modules. Regression risk on live-critical code — do it with tests, not in a rush. |
| **`Live.svelte` (1,201) & `stores/capture.js` (1,200)** | The frontend mirror of the same concentration. Works; large. | Medium |
| **`models.rs` name collision** | It is STT-model *download*, not domain *models*. A reader expects the wrong thing. | Small (rename) |
| **`db/mod.rs` (1,636)** mixes migrations + platform paths + inline tests | Cohesive but large. | Small–medium |
| **~150 lines of dead legacy CSS** (`--text-faint` et al.) | Can't be deleted without eyes on a running app — global (unscoped) rules with generic class names live components still carry (PRODUCT_AUDIT §6; [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) §1). Marked deprecated instead. | Small, but needs a running app |
| **First-run wizard can't be re-run** | Everything in it lives in Settings, but a skipper must know that. | Small |
| **88 × `Result<_, String>` remain in `main.rs`** | The typed error (`error.rs`) exists and crosses the bridge; not every command uses it yet. | Medium, incremental |
| **`docs/data/schema.sql` is transcribed, not generated** | Kept in sync by hand from `db/` (see its header). A generator would remove the drift risk. | Small (a build step) |

---

## 5. What is explicitly **not** on this roadmap

To keep scope honest, these were considered and rejected as roadmap items outright — see the
linked reasoning, and do not reopen without a human decision recorded in
[DECISIONS.md](DECISIONS.md):

- **Native SDI hardware output** — high SDK cost, narrow reach; bridged by hardware churches
  already own (DECISIONS).
- **Rewriting the stack** — Rust + Tauri + Svelte + SQLite is correct for this product and would
  be chosen again (PRODUCT_AUDIT §10.9).
- **Replacing OBS** — not a recording / scene-compositing tool. Relay sits *above* the AV chain.
