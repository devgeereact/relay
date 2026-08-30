# Relay — Gap Report against the Product Transformation Brief

**2026-08-29 · verified against `0338244` (branch `main`), version `0.1.0-4`.**

This document answers one question: **the transformation brief proposes ~80 things — which of
them already exist, which are half-built, which are missing, and which would break a decision
this product has already made and recorded?**

It is the *output* of `docs/GPT.md`, which is the *input* — a paste-into-any-model prompt that
front-loads Relay's real constraints so a model argues inside the product's shape instead of
recommending accounts, RBAC and cloud sync at it. Keep both: the prompt is how the question gets
asked, this file is what the repository answered.

> **This report changed no engine code.** Its §21 buckets and §25 checklist are proposals to a
> human, not work in progress. That is the brief's own §79 rule — *establish what exists, then
> what is wrong, then what is missing, then implement* — and Relay's own scoping rule.

> ## Fix log — 2026-08-29
>
> **RG-01 … RG-05 are closed**, in the order the register ranked them, with the
> reasoning recorded as [DECISIONS](DECISIONS.md) §39–§42 and the tests named below.
> Everything else in §23 stands. **The release decision is unchanged and is still
> NO-GO**, for the reason §24 gives: none of this was a defect count, and none of it
> is a Sunday.
>
> | ID | What changed | Pinned by |
> |---|---|---|
> | **RG-01** | Live shows real per-channel health; a screen that is not answering can never read amber. Live and the Outputs tab now decide from one backend fact through one shared helper | `outputhealth.test.js` (26), `describeScreen` |
> | **RG-02** | Every output page reports that it is still painting — the native window over the bridge, kiosk/OBS over the socket it already has. Anonymous; §35's *who* and *where* untouched | `channels::tests` (6 new, incl. the WS round trip and the malformed-beat case) |
> | **RG-03** | Service Lock: 17 irreversible or engine-stopping actions held back while a service records. Nothing on the fire path. The operator lifts it in one action | `servicelock.rs` (9), `e2e::a_recorded_service_holds_back_a_deletion_but_never_the_wall`, `servicelock.test.js` (12) |
> | **RG-04** | `service_events` + `perf_samples`: an ordered record that survives the app, merged with detections and cues on the way out, carrying nothing a preacher said | `timeline_tests` (6), `e2e::a_service_records_what_happened_and_it_survives_the_service`, `timeline.test.js` (7) |
> | **RG-05** | Safe Screen: the one door content leaves by now refuses a payload that would paint an empty screen or carry an unreadable template; the fit loop reports when it has shrunk below legibility | `pipeline::tests` (6 new), two `e2e` tests, `safescreen.test.js` (11) |
>
> Three of them changed what an existing decision had drawn, so each is written up
> rather than absorbed: §39 narrows one word of §35 (**when**, anonymously — never
> *who*), §40 introduces a lock and then subordinates it to the operator, and §42 adds
> a gate in front of the wall that deliberately cannot touch a panic control.
>
> ### P1 — RG-06 … RG-12, closed the same day
>
> | ID | What changed | Pinned by |
> |---|---|---|
> | **RG-06** | Preflight, snapshot (`VACUUM INTO`), verify-on-next-launch, and a restore that happens before the database is opened. **Not** binary rollback — the installers are public; the church's data is what cannot be got back | `updates.rs` (12), `updatesafety.test.js` (13) |
> | **RG-07** | A timeline row opens to show what was being said around it, what Relay decided, and how fast it was going | `report.test.js` replay group |
> | **RG-08** | The Sunday report, derived and never stored — and it names what it does **not** measure | `report.test.js` (23 total) |
> | **RG-09** | The half-dozen graceful fallbacks are now one line in the shell, on every tab, opened for the detail | `degraded.test.js` (21) |
> | **RG-10** | A room, remembered — microphone, language, length, voice profile, displays. **Not** the audio levels | `db::environments` (6), `rooms.test.js` (19) |
> | **RG-11** | Settings → Languages, every number derived from the shipped data; accuracy and native review render as absences | `detection::language_report_tests` (4), `languages.test.js` (11) |
> | **RG-12** | One file a church can send, composed as an allow-list, with the home directory scrubbed | `diagnostics.rs` (5), `diagnostic_bundle_tests` (2), `diagnostics.test.js` (11) |
>
> Six more decisions were earned and written up: §43 (the binary is replaceable, the
> data is not) · §44 (only what was measured appears) · §45 (an invisible fallback is
> indistinguishable from a fault) · §46 (a room may be remembered; its audio levels
> may not) · §47 (the moat is measured from the shipped data) · §48 (the one artefact
> meant to leave the building gets the strict rule).
>
> ### P2 — RG-13 … RG-17, closed 2026-08-30
>
> | ID | What changed | Pinned by |
> |---|---|---|
> | **RG-13** | Both accessibility lists at zero — **and eleven of the thirteen findings were the instrument's own bugs**, which is the larger half of the fix | `inventory.test.js` (10), `surface.test.js` R3-12 inverted |
> | **RG-14** | p99 live, stored and shown; week-on-week trend across services; and the live Diagnostics screen stopped printing an unreached stage as `0ms` | `timeline_tests` (+3), `report.test.js` (31 total) |
> | **RG-15** | Say one verse, watch six stages. Runs in rehearsal or not at all | `pathcheck.test.js` (17) |
> | **RG-16** | Six drills with the real controls, panic first. **Not** a simulated service | `training.test.js` (21) |
> | **RG-17** | Settings → Privacy, read from the live settings, stating the LAN exposure in the same size type as the reassuring half | `privacy.test.js` (8) |
>
> Four more decisions: §49 (an instrument that cries wolf) · §50 (p95 and the worst
> sample bracket the tail; neither answers it) · §51 (twenty-one checks that pass on
> a machine where nothing works) · §52 (practice is drills, not a simulation).
>
> ### P3 — RG-18 and RG-19, closed 2026-08-30. **The register is now empty.**
>
> | ID | What changed | Pinned by |
> |---|---|---|
> | **RG-18** | Contrast (exact over a solid background, **"cannot be checked" over a picture**), size at distance from two numbers only a person can know, a distance preview, and High Visibility as a THEME rather than a parallel renderer | `legibility.test.js` (21) |
> | **RG-19** | Install a model from a file already on the machine, a three-folder scan, and `scripts/offline-bundle.mjs`. **Signed language packs deliberately NOT built** | `models::tests` (+3), `offline.test.js` (13) |
>
> One more decision: §53 (offline installation is one missing file, and language packs
> are not that).
>
> **RG-18's thresholds are unverified and say so in the UI.** WCAG is a specification
> for screens at arm's length and the character-height rule is broadcast practice;
> neither has been checked against a projector in a church. That is Stage B, and the
> caveat rides with the verdict rather than living here.
>
> **RG-19 shipped its offline half and refused its signed half.** The word doing the
> work in "signed language packs" is *signed*, and signing needs a key, a ceremony and
> a distribution channel that do not exist. An unsigned pack that can rewrite the book
> aliases is a wrong-verse-on-a-wall vector, and the operator cannot check 66 names in
> a language they may not read. It waits on the same thing §47 names: a native speaker
> who has actually reviewed the tables.
>
> ### Where that leaves the register
>
> **Every RG item is closed.** Three of the nineteen shipped deliberately narrower than
> written — RG-10's audio seed, RG-16's audio replay, RG-19's language packs — and all
> three are waiting on the same thing, which is not a commit:
>
> | Waiting on | Blocks |
> |---|---|
> | **Stage C** — a person, a microphone, a real room | RG-10 (seeding the audio gate), RG-16 (replaying a service), the WER measurement that is the moat |
> | **Stage B** — a projector | RG-18's thresholds |
> | **Stage F11** — one full service | the long-service latency question |
> | **A native speaker** | RG-19's language packs, and §47's empty column |
> | **A Windows certificate** | shipping to the platform most churches are on |
>
> That is the same list `ROADMAP.md` §1 has carried since before any of this work, and
> none of it moved. **The register being empty is not the same as being ready**, and
> §24 below still says NO-GO for exactly that reason.
>
> **RG-16 is deliberately narrower than the register described.** "Replay recorded
> audio through the real pipeline" is not buildable here: Relay cannot produce a
> sermon, and a simulation would teach a volunteer the shape of a fake. Replaying a
> church's *own* recorded service audio would work — and needs the audio corpus that
> Stage C has never produced. What shipped is the half that does not need it.
>
> **RG-10 deliberately stops short of its most attractive option.** Seeding the audio
> learner from a stored floor may well be right, and it is not being done, because the
> instrument that could show it safe (`cargo test audio::gate -- --ignored`, against
> real room audio) has never been pointed at a real room. That is Stage C, and it is
> still unrun.

---

## 0. Method, and what this report cannot see

**How every claim here was produced:**

| Kind of claim | How it was established |
|---|---|
| Counts | A command, named beside the number. Re-run it; if it disagrees, this file is the bug |
| "EXISTS / PARTIAL / MISSING" | Read the code, and cite `file:line`. Every reference below resolves at `0338244` |
| Latency figures | Quoted from `audits/PERF-2026-08-24.md`, not re-measured. That document's own §6 states what its numbers do not establish, and none of that changed |
| Detection behaviour | Quoted from the existing corpus gate (`eval.rs`) and the closed findings in `audits/QA-2026-08-14.md` |

**Counts, re-measured 2026-08-29:**

| | Count | Command |
|---|---|---|
| Registered `#[tauri::command]` | **118** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| `main.rs` | **4,369** lines | `wc -l src-tauri/src/main.rs` |
| `stores/capture.js` | **1,941** lines | `wc -l src/lib/stores/capture.js` |
| Rust tests | **547** declared (519 run, 28 ignored) | `cd src-tauri && cargo test` |
| Frontend tests | **594** passing, 0 skipped, 45 files | `npx vitest run` — the runner's summary line |
| Svelte components | **47** (46 reachable) | `node scripts/qa-inventory.mjs` |
| Controls | **426**, 0 in unrendered components | `node scripts/qa-inventory.mjs` |
| Tables in the schema | **18** (+1 FTS virtual) | `grep -c 'CREATE TABLE' docs/data/schema.sql` |
| Decisions | **38** numbered (§18–§38) + 28 unnumbered table rows | `grep -cE '^## [0-9]' docs/DECISIONS.md` |

> **Two counts in this report were wrong on the first pass, and both are recorded rather than
> quietly fixed, because they are the same lesson twice.**
>
> 1. **Frontend tests.** `npx vitest list | wc -l` gave **601**; the runner gives **594**. The
>    difference is seven Svelte compiler warnings on the same stream. The docs were right and
>    this report was wrong. **The command is `npx vitest run`, and the number is the runner's
>    own summary line.**
> 2. **`e2e.rs` ignored tests.** `grep -n '#\[ignore'` returns five lines; only **two** are
>    attributes — the other three are inside doc-comments *describing* tests that were
>    un-ignored. `QA_HARNESS.md` §0 was right at 26 run / 2 ignored.
>
> In both cases a plausible one-liner disagreed with the tool that actually knows, and the
> plausible one-liner was believed. **A count is only as good as the command beside it, and a
> grep is not a test runner.**

**What no instrument in this repository reached, and this report therefore does not claim:**

audio in · pixels out · hardware · a packaged build · a real congregation. Unchanged from
`audits/QA-2026-08-14.md` §16 and `audits/PERF-2026-08-24.md` §6. Anything below marked
**BLOCKED** is blocked on that, not on a commit.

---

## 1. The verdict, on one page

**The brief's single highest-priority recommendation — "Phase 1: make Relay extremely fast",
"⚡ Real-Time Speech Engine 2.0", "faster partial transcription (P0, build now)" — is already
built, already measured, and already hit most of the brief's own targets five days before the
brief was written.**

| Brief's target (§10) | Relay, measured | Source |
|---|---|---|
| First visible partial ≤ 300 ms | **139 ms** median on `ggml-base` | PERF §4 |
| Perceived transcript lag ≤ 1 s | **P95 339 ms** on `base` | PERF §4 |
| Dropped partials < 1 % | **0 of 1075** passes | PERF §4 |
| Real-time factor < 0.7 | decode 144 ms per 200 ms cadence step → **~0.72 duty**, and the cadence *is* the decoder's own speed by construction (`stt.rs:89`) | DECISIONS §38 |
| Mic → screen ≤ 2 s p95 | **NOT MEASURED** — needs an app, an output page and a room | PERF §5 |

So the brief's Phase 1 is largely a lap already run. Two of its specific mechanisms are worse
than what shipped: a **two-speed / short-window fast path** (§6, §8) buys nothing, because
whisper pads its mel window internally and an 8 s and a 4 s window cost the same (DECISIONS §36);
and **adaptive window sizing** (§11) is already superseded by adapting the *cadence* to measured
decode cost, which is the lever that actually moved 349 ms → 139 ms.

**Where the brief is right, and Relay is genuinely weak, is everything downstream of "it works":**

1. **Output truth.** Live's Output Status pane derives every badge from *global* state
   (`Live.svelte:973-979`) and never asks `channel_status`. A kiosk that went away still reads
   **On Air**. This is the exact failure the brief's §17 names, and it is real.
2. **Nothing survives a quit.** Every latency measurement is in-memory (`latency.rs`), so the
   number a church would send you dies when they close the app. There is no service event
   timeline, no replay, no post-service report.
3. **No rollback.** The updater is gated against running during a service (`updater.js:33-37`)
   and its payload is minisign-verified by the plugin — but a bad install has no way back.
4. **No pre-air content validation.** Text *fit* is measured properly (`TemplateRender.svelte:131-160`);
   fit, contrast and output-reachability are never *validated before* content goes to air.
5. **The moat is still unmeasured.** WER: zero, in every language. Native-speaker review: zero.
   Yorùbá numerals: absent (`numerals.json` has `sw` and `ha` only).

**And two of its proposals would reverse recorded decisions** — LAN device pairing (DECISIONS §35)
and an optional cloud layer (ROADMAP §3). Both are written up as reversal *proposals* in §20.
Neither is adopted here.

**Release decision: NO-GO, unchanged.** Not because of anything in this report — because
`audits/QA-2026-08-14.md` §16 Stage F has not been run.

---

## 2. Status matrix — the brief's §4–§77, one row each

Legend: **EXISTS** (implemented and reachable) · **PARTIAL** · **BROKEN** · **MISSING** ·
**DUPLICATE** (already provided elsewhere) · **N/A** (conflicts with the product's scope) ·
**FUTURE** (valid, deferred).

### Reliability, readiness and the live-service envelope

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 4 | Relay Reliability Engine — one unified READY / DEGRADED / ACTION REQUIRED state | **PARTIAL** | 21 probes with a 4-level severity ladder (`boot/boot.js:129-171`, `boot/probes.js`); a rolled-up verdict already exists (`Dashboard.svelte:136-176`) | Rust has **no** health state at all (grep: zero hits for readiness/degraded/service lock). The roll-up is frontend, launch-time + Dashboard, and nothing in the pipeline consults it |
| 5 | Sunday Readiness screen | **PARTIAL** | Same as §4. The Dashboard hero already says "Ready for a service." / "N things not working." | No **synthetic end-to-end test** ("say John 3:16", verify mic→audio→STT→detection→router→output). The pieces exist — `FirstRun.svelte:212-222` already fires a real `manualFire('John 3:16')` — but nothing walks the *spoken* path |
| 6 | Service Lock — block destructive config while live | **MISSING** | The only two analogues: `set_rehearsal` refuses while a service is recording (`main.rs:2872-2881`), and `updater.js:33-37` refuses while capturing | No lock state, no blocked-action explanation, no protection of model changes / template editing / DB maintenance during a service |
| 30/63 | Graceful degradation + a formal failure matrix | **PARTIAL** | Real fallbacks exist and are documented in code: denoise off below 48 kHz (`dsp.rs:15`), audio-only with no model (`main.rs:2635`), heard-but-unresolvable → suggestion (`pipeline.rs:131`), rehearsal fail-**open** (`channels.rs:505-509`), safe mode after 3 crashes (`boot.js:215`) | There is **no `Degraded` type, flag or state** anywhere. Each fallback is local and invisible. No failure matrix document |
| 31/32 | Automatic recovery, operator-confirmed | **EXISTS** | `RecoverSession.svelte:36-51` — a modal with Resume / Start fresh that restores **position only** and states "nothing is put back on any screen". `liveOnAir` is a separate fact precisely so it cannot be restored | Rust-side `SessionState` (`main.rs:92-118`) is not persisted — a crashed service leaves an open `services` row with no resume |
| 33/50 | Panic system: Clear · Black · Restore, and never claim an unachieved success | **EXISTS** | DECISIONS §20. `panicRun` returns a boolean **and** sets `panicError` (`capture.js:1712-1732`); shell-level panic bar (`App.svelte:443-451`); `shortcuts.js` suppresses Esc while a dialog is mounted | **Restore** does not exist as a control (Clear and Black do) |
| 34/46 | Emergency manual mode when the AI is unavailable | **PARTIAL** | Manual fire is first-class and always present (`Live.svelte:1183-1195`); detection can be disarmed (`toggleDetection`); safe mode disarms detection wholesale | There is no *state* called manual mode and no banner that says "AI detection unavailable — you are driving" when STT dies |

### Speech, transcription and detection

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 7 | PARTIAL / STABLE / FINAL transcript states | **PARTIAL** | PARTIAL and FINAL exist end to end: `TranscriptUpdate.is_final` (`stt.rs:129`), reducer `applyTranscript` (`capture.js:147-157`), and a visually distinct render — partial in `<mark>` with a caret, finals as timestamped rows (`Live.svelte:1004-1025`) | **STABLE does not exist.** There is no stable-prefix detection and no reconciliation between passes. The nearest thing is the router's corroboration count (`router.rs:268-297`), which is a sighting counter, not a text state |
| 6/8 | Two-speed transcription (fast path + accuracy path) | **N/A as framed** | DECISIONS §36 and PERF §4: whisper pads the mel window internally, so a shorter window costs the same; audio→transcript settles at 1.0–1.6 × decode cost, and above `base` **the model is the entire remaining latency** | A genuinely cheaper second decoder would be a different *model*, not a different window. That is a real (large) piece of work and is not what §8 describes |
| 9 | Streaming / rolling / incremental decoding | **MISSING** | Every emission is a whole-window `state.full()` re-decode (`stt.rs:895-901`) | Real token-level streaming is a whisper.cpp capability question, not a Relay wiring question |
| 10 | Hard latency acceptance targets | **EXISTS** | PERF §5 already scores against a target table, per model, and marks three rows **MISS** and four **NOT MEASURED** rather than rounding them off | Targets on **church hardware**; every number is one M4 Pro |
| 11 | Adaptive inference | **PARTIAL — superseded** | The cadence adapts to measured decode cost: `step_samples_for` (`stt.rs:109-113`), EMA at `stt.rs:623-628`, clamped `MIN_STEP_SAMPLES` (one chunker hop = 200 ms) to 1000 ms | `sysprobe.rs` is **advisory only** — nothing in the pipeline branches on probed hardware; its two consumers set a warning string (`models.rs:191`) and pick warning wording (`stt.rs:640`). Also: `gpu_backends` is a **compile-time** fact, deliberately (DECISIONS §36) |
| 12 | Scripture candidate prefetch during the utterance | **MISSING** | Detection runs per completed decode pass on `relay-detect` behind a bounded queue (`main.rs:3176`, `:3284-3298`) | Detection is already off the decoder's thread and costs 2.6 ms/query on 31k verses (`detection.rs` benchmark). **Prefetch would optimise the cheapest stage in the pipeline** — see §3 |
| 13 | Voice confidence shown separately from claim type | **PARTIAL** | Claim type is first-class and correct: `detect.js:24-53`, amber chip + meter for Direct, cyan chip and **no number at all** for a guess (`Live.svelte:1086-1101`). DECISIONS §21 | There is no **voice** confidence anywhere. Audio quality is surfaced as warnings (clipping/too quiet/noisy, `Live.svelte:642-668`), not as a per-detection signal-strength claim |
| 14 | Explainable detection — "why this verse?" | **EXISTS** | `DetectionInspector.svelte` (517 lines), opened from Live (`:1131-1133`). Renders real evidence only — the parsed span, or the shared rare-word chips — and explicitly refuses to fabricate reasoning (`:10-22`, `:182-212`). Pinned by `inspector.test.js` (11 tests) | Nothing |
| 15 | One Scripture Safety Firewall | **PARTIAL, and honestly documented** | `Fire` is constructed in exactly **two** places, both via `resolve_fire` (`main.rs:604`, `:920`); `broadcast_content` has exactly **one** caller (`main.rs:577`). But content leaves through **four** gated publishers plus a fifth *pull* door — `channels.rs:469-488` names all four, and records that `stage_next` leaked during a rehearsal because it was not on the list | The routing gate applies **only** to the AI path (`main.rs:911`); every human path bypasses the router by design. That is correct — but it means "one choke point" is not true and should not be claimed. §16's pre-air checks are the thing that would make it one |
| 27 | Church-local vocabulary learning | **PARTIAL** | `voice_profiles.bias_terms` exists and is editable (`Settings.svelte:753-856`), and feeds whisper's decoder prompt | No learning. Nothing observes what this church actually says and adapts ranking |
| 28 | Speaker profiles | **PARTIAL** | `voice_profiles`: name, language, bias vocabulary, sensitivity, learned `auto_fire`/`suggest` pair (read-only in the UI) | No speaking-rate, no typical-volume, no microphone, no common-books calibration |

### Outputs, screens and templates

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 16 | Safe Screen — validate before air | **MISSING** | Fit is *measured*: `fitOne` shrinks until it stops overflowing in **both** dimensions (`TemplateRender.svelte:131-160`), and layered templates binary-search (`:557-590`) | Nothing **blocks** a render. There is no pre-air check that the reference is valid, the template is sane, the text fits, the contrast is legible or the output is reachable — and no path that refuses to fire when it is not |
| 17 | Output health monitoring | **PARTIAL — and this is the report's most actionable finding** | `channel_status` (`main.rs:3760-3800`) is derived live from open windows + subscribed clients, deliberately not from the dead `output_channels.status` column. Outputs tab polls it every 2 s (`Channels.svelte:88-108`) and shows LIVE / IDLE / UNAVAILABLE + client count | **Live never calls it.** `Live.svelte:973-979` derives every per-channel badge from `$live && !$rehearsing && !$screenBlack`. A screen that vanished still reads *On Air* on the one surface an operator watches during a service. Also: no last-seen, no reconnect count, and `network_client` online is **always true** ("it is being served") |
| 18 | Output heartbeats | **MISSING** | The WS hub honours exactly two inbound kinds: `hello` and `rendered` — and `rendered` is a latency mark documented as *"deliberately inert"* (`channels.rs:963-991`). No ping/pong, no idle timeout, no keepalive | A heartbeat is the mechanism that would make §17 true rather than inferred |
| 35 | Automatic text-fit safety | **PARTIAL** | See §16 — fit is real and well engineered | Fit that *shrinks forever* is not the same as fit that *refuses*. A 40-iteration cap exists; what happens at the cap is "very small text", silently |
| 36 | Distance preview (5/10/15/20 m) | **MISSING** | Grep across `src/`: zero hits for distance/legibility simulation | — |
| 37 | Accessibility mode (high visibility) | **PARTIAL** | The **operator console** is served: `sr-only` live regions, `role="meter"`, `trapFocus`, reduced-motion on the crawl. The **output** has none | No high-contrast or large-text output mode, no contrast **validation** (contrast is a render control — scrim, plate, shadow — never a checked ratio; the only ratio arithmetic in the repo is in comments) |
| 54 | Signed template library / marketplace | **FUTURE** | Already deferred with reasoning (`ROADMAP.md:67-75`) — plugins must never modify the presentation engine | Agreed; not v1 |
| 44/45 | Distance/fit/contrast gate before air | **MISSING** | Consolidates §16, §35, §36, §37 | One pre-air validator, one refusal path, one honest message |

### Security, privacy and trust

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 19/20/21 | Local device pairing · trusted devices · authenticated LAN | **MISSING — and reverses DECISIONS §35** | The HTTP control plane on `:8032` is unauthenticated **deliberately** (`main.rs:1398-1401`, `channels.rs:1207-1211`). The WS hub on `:8031` is broadcast-only and counts clients without recording who they are (`channels.rs:706-711`). What *was* fixed on 2026-08-20: mutating routes require `POST` and are denied the CORS wildcard, killing the `<img src=…/api/black>` drive-by | See **§20 (a)** — a reversal proposal, not a task |
| 22 | Local security event log | **MISSING** | No table, no events | Depends on §19 having identities to log |
| 23 | Service history as an event timeline | **PARTIAL** | `services`, `transcripts`, `detections` (with `status` ∈ auto/suggested/dismissed/**manual** and `heard_text`), `cues` (operator actions with `payload_json` + `triggered_at`). A markdown export exists (`export_service`) | No **single ordered timeline**. Output-lost / output-recovered are not events at all, because nothing detects them (§17/§18) |
| 24 | Tamper-evident service record | **FUTURE** | — | Sequence + hash chain over an append-only event table. Cheap *once §23 exists*; meaningless before |
| 25 | Service replay | **MISSING** | Zero hits for replay across `src/` | The single highest-value item in the brief that is genuinely absent and genuinely buildable from data Relay already stores |
| 26 | Sunday report | **MISSING** | The data mostly exists (detections with status, service duration); the latency half does **not survive a quit** | See §23 and RG-04 |
| 56 | Diagnostic bundle | **PARTIAL** | Settings → Diagnostics shows version, OS, model, language, mic, ports, uptime, and the full latency table (`Settings.svelte:998-1054`) | It is a screen, not an export. Nothing produces a file a church can attach to an email |
| 57 | Privacy centre | **PARTIAL** | `PRIVACY.md` is thorough and honest; crash reporting is opt-in, off by default, and content-scrubbed (`telemetry.rs:1-28`); Settings → Advanced states it plainly | Not a single screen answering "what is leaving this machine right now" |
| 58 | AI transparency centre | **EXISTS** | `docs/AI_DISCLOSURE.md` plus the per-detection inspector (§14) | A link from Help to AI_DISCLOSURE would close it |
| 64 | Security threat model T1–T10 | **PARTIAL** | `SECURITY.md` covers the LAN surface, ports, the origin of the CORS fix, and template injection (pinned by `qa-r5-template-injection.test.js`, 5 tests) | Not in the T1–T10 shape; no malicious-language-pack or corrupted-update rows (neither exists yet) |

### Environment, language and operators

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 29 | Room / environment profiles | **MISSING** | The VAD noise floor and AGC state are per-process and reset every start (`audio.rs:145-148`, `dsp.rs:146-153`) | Nothing persists a room. A church re-learns its own hall every Sunday — which *works* (DECISIONS §19) but costs the first minutes of every service |
| 30 | Automatic room calibration | **PARTIAL** | Levels are **learned, never assumed** — this is DECISIONS §19 and CLAUDE.md rule 12, and it is load-bearing. `FirstRun.svelte` step 3 already runs a live meter | No explicit "calibrate now" pass that measures ambient noise, clipping and speech level and *stores* the result |
| 41 | Language Mode (primary + secondary + code-switching) | **PARTIAL** | Recognition language is a single choice: Auto / en / yo / sw / ha (`Settings.svelte:858-886`). Auto-detect is real and instability is surfaced (`stt://language_unstable`) | No declared primary/secondary pair. CLAUDE.md says code-switching is the normal case; the settings model says one language |
| 42 | Language Quality Centre | **MISSING** | `docs/LANGUAGES.md` states the truth in prose: 66/66 books in all three, **none native-reviewed**, WER **unmeasured**, Yorùbá numerals absent (`numerals.json` has `sw` + `ha` only; `book_aliases.json` has `sw` 68, `yo` 69, `ha` 69) | A measured, in-app status per language. The honesty exists; the instrument does not |
| 43 | African-language validation programme | **BLOCKED** | ROADMAP §1: 30 minutes of real sermon audio + a native speaker. `bench/.gitignore` refuses audio into the repo by design | Not a coding task. This is the moat and it is entirely unmeasured |
| 44 | Signed language packs | **FUTURE** | Locale files ship near-empty on purpose (`ha`/`sw`/`yo` have 1 key each); aliases are one JSON file | Would let a contributor improve a language without Rust. Real value, after §43 |
| 39/40/48 | Operator training mode · live simulation · rehearsal replay | **MISSING** | Rehearsal mode exists and is properly gated (DECISIONS §18) — but it needs a live preacher | A recorded-audio simulation would be the same rig as `stt::realtime::live_transcript_latency`, pointed at the UI instead of a report |
| 59/60 | Church onboarding · saved environment | **PARTIAL** | 6-step first-run wizard (`FirstRun.svelte:62-69`), re-runnable from Settings → Backup | Steps 5–10 of the brief's list (stage display, template, test scripture, readiness check, rehearsal) are not in the wizard |
| 61 | Time-to-first-verse metric | **MISSING** | — | Cheap once §23 exists |

### Distribution, updates and the cloud question

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 32/48 | Update preflight (signature · platform · integrity · DB compat) | **PARTIAL** | The Tauri updater plugin verifies a minisign signature against `pubkey` (`tauri.updater.conf.json`); a test pins that the base config must always carry an updater block or the app panics (`models.rs:747-780`). Model downloads are separately SHA-256 verified before rename (`models.rs:475-479`) | No **database-compatibility** preflight and no explicit preflight screen. `SCHEMA_VERSION` exists (`db/mod.rs:51`) and nothing compares it to the incoming build |
| 33 | Never update during a service | **EXISTS** | `updater.js:33-37` — `idle()` is `!capturing`, checked on both check and install; the banner only renders when not capturing (`App.svelte:455-471`) | It is frontend-only and keyed on the microphone, not on `Session`. A recorded service with the mic momentarily stopped is not protected |
| 49 | Update rollback | **MISSING** | Grep for rollback across `src-tauri/src` and `src`: **zero hits**. On failure the app says *"Relay will keep working on this version."* | The whole mechanism. Also: no migration rollback path, although migrations are individually retryable (CLAUDE.md rule 25) |
| 47 | Release channels (stable/beta/nightly) | **FUTURE** | One endpoint: `releases/latest/download/latest.json` | Low value before there is a first release |
| 45 | Offline / air-gapped installation | **MISSING** | Models are downloaded in-app (`models.rs`), resumable and checksummed. KJV is bundled (`include_str!`) | A church with poor internet cannot receive app + model + corpus on a USB stick. **High fit with the actual market** |
| 46 | Signed distribution, both platforms | **BLOCKED** | macOS signing + notarization + the hardened-runtime entitlement chain are wired and pinned (`models::config_boots`, `scripts/sign-local.sh`). The release gate is **per-platform** and refuses a real tag that is not covered on both (DECISIONS, CLAUDE.md rule 23) | A Windows code-signing certificate (~$10/mo). ROADMAP §1 |
| 55 | Deliverability as its own discipline | **PARTIAL** | `docs/RELEASING.md` (413 lines) covers signing, the gate, the updater and the version-in-three-files rule | Rollback, offline installer, channels, migration preflight |
| 34–38, 50–53, 77 | Relay Cloud · church account · device registry · fleet management · multi-campus · optional backup | **N/A — already declined, with reasoning** | `ROADMAP.md:77-91` declines cloud sync, accounts, RBAC/SSO, multi-tenancy, analytics dashboards, marketplaces, billing, and compliance — each with a stated reason. `PRODUCT_AUDIT.md` §13 marks the same set NOT APPLICABLE | See **§20 (b)** if this is to be reopened |
| 51 | Cloud must never become a live dependency | **EXISTS — as the product's shape** | Offline-first is enforced by construction; `probes.js` reports offline as **ok**, not as a problem: *"offline — every core feature still works"* | Nothing |
| 76 | Do-not-build list | **EXISTS** | ROADMAP §3 and §5 already hold it | Nothing |

### Process, IA and measurement

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 38/49 | Sunday-mode / simplified live screen | **PARTIAL** | Live already has a density toggle (Normal/Compact) and full-screen mode (`Live.svelte:841-849`) | The brief's stripped layout is a *different* proposal from density. Worth prototyping, not obviously better than what shipped |
| 65 | Database audit | **See §13 below** | 18 tables + 1 FTS | 5 concepts have no home |
| 66 | Event architecture audit | **See §14 below** | 15 events today | 6 candidate events, 4 of which have no producer |
| 67 | UI information architecture | **EXISTS** | 8 tabs (`App.svelte:46-78`), 16 Settings sections (`Settings.svelte:46-63`), retired keys remapped rather than 404'd (`session.js:118-140`) | The brief's suggested Settings → Devices / Security / Languages are the right shape **if** §19/§22/§42 are ever built |
| 68/69 | Live screen states · design system colour law | **EXISTS** | The four promise-carrying colours are defined once (`src/app.css:117-131`) and the six distinctions are each pinned by tests (QA_HARNESS §4.2). Cued is grey and never amber; a guess is cyan and never amethyst | Nothing. This is one of the strongest parts of the product |
| 70/71 | Performance observability + per-stage budgets | **EXISTS** | Nine named stamps on one monotonic clock, one trace id from microphone to projector (`latency.rs:131-166`), 7 metrics, 1 ms-bucket histograms with counted tails, per-minute drift, and four anti-flattery properties each pinned by a test | **p99 is not reported** (p50/p95/worst/over-ceiling only, `latency.rs:728-729`) and **nothing persists** |
| 72 | Hardware matrix | **BLOCKED** | Every figure is one M4 Pro | Windows low/mid/high, a real church laptop |
| 73/74 | Real church field test · first ten churches | **BLOCKED** | `audits/QA-2026-08-14.md` §16 is the script, unrun | A room, a person, a Sunday |
| 75 | Feature priority model (P0/P1/P2/P3/REFUSE) | **EXISTS in substance** | ROADMAP already sorts into blocked-on-world / parked / deferred / declined | Adopting the brief's live-critical / operator-critical / service-support / admin labels would be an improvement to `CONTRIBUTING.md` |
| 62 | Success metrics | **PARTIAL** | Latency: instrumented. Detection: `eval.rs` is a CI build gate over a 50-case labelled corpus scored **through the real router**, failing above SPEC's 5 % wrong-verse rate | Human metrics (override rate, acceptance, time-to-first-verse) need §23. Language metrics need §43 |

---

## 3. Performance audit

Nothing in this report re-measured performance. `audits/PERF-2026-08-24.md` is current, was
produced five days ago, and its §6 already states what it does not establish.

The one thing worth adding, because the brief asks for optimisations that would be wasted:

**Do not prefetch scripture candidates (brief §12) and do not shorten the window (§6, §8, §11).**
Detection is not the bottleneck and has not been since 2026-08-24. It costs ~2.6 ms per query on
a 31k-verse linear scan, runs on its own thread behind a bounded queue (`main.rs:3176`), and shed
**zero** partials across 1075 passes. Above `ggml-base`, PERF §4 is explicit: *the model is the
entire remaining latency.* Optimising the 2.6 ms stage to make the 144 ms stage feel faster is
the classic version of this mistake, and CLAUDE.md rule 31 exists because this project already
made it once ("twice now the reflex answer was 'STT is slow' and twice more than half of it was
not").

**The one performance unknown that matters is Stage F11** — a full service, watching for a rising
per-minute line in Diagnostics. The cadence now runs the decoder continuously for a model slower
than one 200 ms hop, and thermal throttling cannot be produced in six minutes.

## 4. Voice / transcription audit

| Question | Answer |
|---|---|
| Current latency | `base` P50 **139 ms**, P95 339 ms, worst 543 ms (PERF §4). `small` 573/989. `turbo` 2360/2556 |
| Partial behaviour | Whole-window re-decode on a cadence of `clamp(decode_ema, 200 ms, 1000 ms)`; a partial *replaces*, a final *appends* and is capped at 12 lines (`capture.js:311`) |
| Final transcript | Closed by `SILENCE_FINALIZE = 7` (~1.4 s of silence), then the window is cleared |
| Queue behaviour | `sync_channel(8)`. A full queue **sheds a PARTIAL** and **blocks on a FINAL** — finals carry persistence and spoken commands and are never dropped (`main.rs:3313-3336`) |
| Dropped partials | Counted (`latency.rs:453-457`) and shown in Diagnostics. 0 in 1075 passes on a dev machine |
| RTF | Not reported as a named metric; derivable from `stt_decode` median vs the cadence |
| Bottleneck | The model, above `base`. Below that, the batch decoder's floor of 1.0–1.6 × decode cost |

**Gap:** no STABLE state, no p99, nothing persisted, and no measurement with a webview in the
path — `audio_to_visible_transcript` in the rig means "a consumer was handed the text".

## 5. Detection audit

| Method | May auto-fire | Where capped |
|---|---|---|
| `Direct` | **Yes**, and only when corroborated by a second pass unless the window is FINAL | `router.rs:268-277` |
| `Semantic` | No — `Suggest` at any score | `detection.rs:79-81`, `router.rs:307-313` |
| `Ambiguous` | No | same |
| `UncertainBook` | No — added after the "hymn number three sixteen" → **Numbers 3:16** P0 | same; CLAUDE.md rule 10 |

One window may inform about several verses and put **at most one** on a wall (`rank_for_wall`,
`main.rs:714-737`, `:902-905`). The debounce is per-reference and derived from the window
(`DEFAULT_DEBOUNCE_MS = (WINDOW_SECS + 2) × 1000`). Manual fires are recorded as `'manual'`, and
rehearsal feedback is explicitly excluded from calibration — *"a rehearsal is not evidence"*
(`main.rs:2803-2814`).

**This is the strongest subsystem in the product and the brief adds nothing to it.** Its §14
(explainability) is already built; its §7 (safety) is already stricter than proposed. The gap is
not in the gate, it is that **detection accuracy over audio has never been measured** — `eval.rs`
scores detection over *text*.

## 6. Output audit

Surfaces: native Tauri window · kiosk WebSocket `:8031` · embedded HTTP `:8032` (output/stage
pages, `/media/<id>`, and the seven-route preacher API).

| Property | State |
|---|---|
| Content delivery | Fire-and-forget `broadcast::Sender`; `Lagged` silently skipped (`channels.rs:956`) |
| Client → server | Two message kinds only: `hello` (registers, replies with template + themes) and `rendered` (a latency mark, *deliberately inert*). Everything else ignored (`channels.rs:1023`) |
| Liveness | Inferred from a **count** per `template_id` (`channels.rs:741-763`). `network_client` online is hardcoded true |
| Identity | None, by design: *"Relay does not record who connected, from what address, or when"* (`channels.rs:706-711`) |
| `output_channels.status` | A dead column — seeded `'offline'`, never written at runtime |
| Where health is shown | Outputs tab, polled 2 s. **Not on Live** |
| Honesty | The Outputs tab already says the right thing: *"A screen reading LIVE means something is attached, not that the picture is good."* (`Channels.svelte:524-539`) |

**Finding RG-01 is here**, and it is the one place where the brief's diagnosis lands exactly on
a real defect: an operator watching Live during a service is shown a badge that cannot detect the
failure it appears to be reporting.

## 7. Security audit

The LAN posture is deliberate, recorded (DECISIONS §35), and correctly described in `SECURITY.md`
and `PRIVACY.md` — both of which were repaired on 2026-08-14 after they had repeated an outdated
claim for months. `:8031` is broadcast-only; `:8032` is an unauthenticated control plane; mutating
routes require `POST` and are denied the CORS wildcard, which closed the bystander-browser vector
without pretending to be authentication.

Template injection is treated as untrusted input reaching the wall and is pinned by five tests.
The kiosk CSP and `X-Content-Type-Options: nosniff` are set; path traversal is rejected; `/media/`
takes only leading digits as an id.

**Gaps:** no device identity (§19), no security event log (§22), no threat-model document in the
T1–T10 shape (§64). All three are consequences of the §35 decision, not oversights.

## 8. Privacy audit

Strong, and the strongest-documented part of the product. Crash reporting is off by default, has
no DSN in OSS builds, is scrubbed of transcript, verse, lyric, announcement, service and plan text
(`telemetry.rs:21-28`), and is compiled out of release unless configured. `session.js` persists
position and never content. `crash.js` renders technical detail with `textContent`, never
`innerHTML`. Offline is reported as a **normal** state, not a fault.

**Gap:** a single Privacy screen (§57) and a diagnostic *export* (§56). Neither changes what
leaves the machine; both change whether an operator can see that nothing does.

## 9. Reliability and recovery audit

| Mechanism | State |
|---|---|
| Crash boundary | Plain-DOM, deliberately not Svelte, because Svelte may be what broke (`crash.js:17-19`). Says *"Your output screens are still live."* |
| Crash record | `localStorage['relay.boot.v1']`: `cleanExit`, `lastCrash`, `crashStreak`, `safeMode` |
| Safe mode | Auto-offered after 3 crashes (`boot.js:215`); disarms outputs and detection; banner on the shell |
| Session resume | Operator-confirmed modal; **position only**, never on-air. Pinned by `qa-r5-onair.test.js` |
| Leave guard | `markCleanExit()` always; `preventDefault` only while the mic is live |
| Migrations | Individually retryable, with `DROP TABLE IF EXISTS` on the scratch table and rollback on failure (CLAUDE.md rule 25) |
| Rust-side session | **Not persisted.** A crashed service leaves an open `services` row |
| Latency history | **Lost on quit** |

## 10. Update / distribution audit

Wired: minisign-verified payload, per-platform release gate, three-file version agreement asserted
in CI and against the tag, resumable checksummed model downloads, an in-app update banner that
never appears while capturing.

Missing: rollback, DB-compat preflight, offline installer, release channels — and one observed
end-to-end install, which ROADMAP §1 already lists as blocked on thirty minutes with a real
machine.

## 11. Language audit

`docs/LANGUAGES.md` is honest and should not be softened. Verified against the data files:

| | Kiswahili | Yorùbá | Hausa |
|---|---|---|---|
| Book aliases | 68 entries | 69 | 69 |
| In-language numerals | ✅ (`numerals.json` `sw`) | ❌ **absent** | ✅ (`ha`) |
| Native-speaker review | ❌ | ❌ | ❌ |
| WER | never measured | never measured | never measured |
| UI locale file | 1 key (ships near-empty on purpose) | 1 key | 1 key |

The brief's §42 "Language Quality Centre" is the right instrument and it does not exist. The brief's
§43 validation programme is correct and is **blocked on the world**, not on code.

## 12. UI / UX audit

47 components, 46 reachable (the one orphan is a test probe). 426 controls, none in an unrendered
component. From `node scripts/qa-inventory.mjs`, two real defect classes remain:

- **4 buttons with no handler** — `Stage.svelte:223` and `ServicePlanner.svelte:482` are
  `type=submit` inside forms (false positives); `ModelSetup.svelte:131` ("In use") and the
  `VerseDeck.svelte:52` match are worth a look.
- **9 controls with no accessible name**, including `Live.svelte:1038` (the mic toggle) and two
  Settings buttons. These are real accessibility findings on the run surface.

Colour law, empty/loading/error separation, error humanisation and the six live distinctions are
all pinned by tests and are in good shape.

## 13. Database audit

18 tables + 1 FTS virtual table. Present and adequate for: services, transcripts, detections,
cues, plans, songs, arrangements, saved scripture, announcements, media, templates, channels,
settings, voice profiles, translations, verses.

**Absent, and each is a prerequisite for a brief feature:**

| Concept | Needed by | Note |
|---|---|---|
| `service_events` (append-only timeline) | §23 replay, §24 tamper-evidence, §26 report, §61 metric | The single highest-leverage table. `cues` + `detections` + `transcripts` are three partial views of it |
| `devices` | §19 pairing, §20 trusted devices, §22 security log | Only if §35 is reversed |
| `security_events` | §22 | Depends on `devices` |
| `perf_samples` | §26, §70 persistence | Latency is currently in-memory only |
| `environment_profiles` | §29, §30 | Room calibration that survives a restart |

`song_arrangements` remains the one table with a wrapper (`saveArrangement`) that **no rendered
component imports** — the arrangement editor is a feature, not a fix, and CLAUDE.md already
records it honestly.

## 14. Event architecture audit

15 Tauri events today. Candidates from the brief, and whether they have a producer:

| Proposed event | Producer exists? |
|---|---|
| `stt://partial` / `stt://final` | **Already covered** — `stt://transcript` carries `is_final`. Do not split it |
| `stt://stable` | No producer. Requires §7 |
| `output://heartbeat` | No producer. Requires §18 |
| `device://connected` / `disconnected` | No producer — the hub counts, it does not identify |
| `readiness://changed` | No producer — readiness is frontend-only |
| `service://locked` / `unlocked` | No producer — no lock exists |

**Recommendation:** add no events until their producers exist. Five of six would be decorative.

## 15. Field-readiness audit

Unchanged: **0 %** on audio in, pixels out, hardware, packaged build, and a real congregation.
`audits/QA-2026-08-14.md` §16 Stages A–F is the script. F6/F7/F8 are regression tests and green in
CI; **F1–F5 and F9–F14 need a person, a room and a packaged build.**

---

## 16. Missing functionality — consolidated

Service replay · Sunday report · service event timeline · output heartbeat + per-channel liveness
on Live · Service Lock · update rollback · DB-compat preflight · offline installer · pre-air Safe
Screen validation · contrast validation · distance preview · output accessibility mode · room /
environment profiles · explicit room calibration · language quality centre · signed language packs
· training mode · rehearsal replay · diagnostic bundle export · privacy screen · persisted latency
· p99 · time-to-first-verse.

## 17. Redundant functionality — do not build

| Brief item | Why it is redundant |
|---|---|
| §6/§8 two-speed transcription | The window is not the lever (DECISIONS §36) |
| §11 adaptive window sizing | Superseded by adaptive **cadence** (DECISIONS §38) |
| §12 candidate prefetch | Optimises a 2.6 ms stage inside a 144 ms budget |
| §7 splitting `stt://transcript` into two events | `is_final` already carries it |
| §5 a second readiness implementation | `boot/probes.js` is the implementation; extend it, do not fork it |
| §14 explainable detection | Built: `DetectionInspector.svelte` |
| §33 never update during a service | Built: `updater.js:33-37` |
| §13 claim-type display | Built, and stricter than proposed (DECISIONS §21) |
| §69 design system | Built, and pinned by tests |

## 18. Technical debt

No new debt is recorded here. `ROADMAP.md` §4 owns the register; the only correction this report
makes to it is that `main.rs` is **4,369** lines / **118** commands, not 4,024 / 114, and
`capture.js` is **1,941**, not 1,908.

---

## 19. Decisions that stay untouched

Every one of these was reaffirmed by this audit, and each is cited by code:

| Constraint | Where |
|---|---|
| No native SDI, ever, unless reopened | DECISIONS (brainstorm table), ROADMAP §5 |
| Offline-first; cloud never in the live path | CLAUDE.md, `probes.js:98-104` |
| Operator override is first-class | DECISIONS §20, §21 |
| One template engine; output channels are render targets | CLAUDE.md, `TemplateRender.svelte` |
| Local-first data | `PRIVACY.md`, `telemetry.rs` |
| Only `Direct` may auto-fire; a cosine is not a probability | DECISIONS §21, `detection.rs:79-81` |
| Rehearsal gates at the broadcast, not the caller | DECISIONS §18, `channels.rs:469-488` |
| Never report a success that did not happen | DECISIONS §20, `capture.js:1712-1732` |
| Audio levels are learned, never assumed | DECISIONS §19, CLAUDE.md rule 12 |
| One window, one wall | DECISIONS §37, `main.rs:714-737` |
| Never make the live path faster by making it less safe | CLAUDE.md rule 34 |
| Planner builds; Live runs; Planner cannot reach an output | `ServicePlanner.svelte:1-18` |
| Relay sits above OBS/ATEM/ProPresenter, never replaces them | SPEC §9, ROADMAP §5 |
| No accounts, no RBAC, no SSO, no billing in the live app | ROADMAP §3, PRODUCT_AUDIT §13 |
| Free, MIT | README, DECISIONS |

---

## 20. Reversal proposals — for a human, not for an agent

### (a) DECISIONS §35 — device pairing on the LAN control plane

- **Existing decision.** The HTTP API on `:8032` is an unauthenticated control plane on the local
  network, deliberately. The WebSocket hub on `:8031` is broadcast-only and records nothing about
  who connected.
- **Original reasoning.** *"A password on a device shared between a preacher, a tech volunteer and
  a stand-in every Sunday is a password written on a sticky note behind the desk."* The threat
  model accepted is a LAN appliance in a building whose network the congregation already trusts.
- **Proposed reversal.** A 4-digit pairing code with a short-lived token, a trusted-device list
  with revoke, and identity on the WS hub — the brief's §19–§21.
- **Benefit.** Closes the on-network attacker, and is the prerequisite for output heartbeats with
  identity (§18), a security log (§22) and per-device health.
- **Cost.** A new `devices` table and token lifecycle; a pairing step in the preacher's-phone flow
  that must survive a dead battery ten minutes before a service; and the sticky-note failure the
  original decision names. It also contradicts *"Relay does not record who connected"*
  (`channels.rs:706-711`), which is currently a privacy guarantee.
- **New risks.** A church that cannot pair a phone at 10:29 loses the remote entirely. Token
  expiry during a 90-minute service. A revoke UI a volunteer can misuse.
- **Evidence required before deciding.** One church actually asking for it; or Relay being run on
  a guest/shared network, which §35 already names as the realistic trigger.
- **Why the current approach may be insufficient.** A laptop that also joins café WiFi serves the
  media files *and* the remote to that network. That is §35's own stated tripwire, and it has not
  been tested.
- **Note.** §18 output heartbeats do **not** require this. A heartbeat can be anonymous — a
  connection id, not an identity — and that version is compatible with §35 as it stands.
  **Build the anonymous heartbeat first; it is not a reversal.**

### (b) ROADMAP §3 — an optional cloud administration layer

- **Existing decision.** Cloud sync, accounts, multi-tenancy, RBAC/SSO, analytics dashboards,
  marketplaces and billing are declined, each with a stated reason.
- **Original reasoning.** Offline-first is the moat, not a constraint; there is one operator,
  standing in the room; nothing leaves the device without a visible reason.
- **Proposed reversal.** A strictly optional Relay Cloud outside the live path: device registry,
  configuration/template/plan backup, update management, language-pack distribution, support
  diagnostics.
- **Benefit.** Real, for the two items that are *already* pain: distributing language packs (§44)
  and receiving a diagnostic bundle from a church (§56).
- **Cost.** An entire second product — a server, an account model, a privacy surface, a security
  boundary, and a support burden — before the first product has run one Sunday.
- **New risks.** The strongest one: once a cloud exists, features drift into it. Every "optional"
  cloud in this category eventually became required.
- **Evidence required.** Ten churches running three consecutive services each. Not before.
- **Recommendation.** **Do not reopen now.** The two genuine needs it serves are better met
  offline: a signed language pack on a USB stick, and a diagnostic bundle the operator emails.

---

## 21. Prioritised plan, in the three buckets the brief asks for

### BLOCKED ON CODE — can start today, no external dependency

| P | Item | Why now |
|---|---|---|
| **P0** | **RG-01** Live shows real per-channel output health | A badge that cannot detect its own failure is worse than no badge. The backend command already exists |
| **P0** | **RG-02** Anonymous output heartbeat + last-seen | Makes RG-01 true instead of inferred. Not a §35 reversal |
| **P0** | **RG-03** Service Lock | Blocks model changes, template edits, DB maintenance and updates while a service is recording |
| **P0** | **RG-04** `service_events` table + persist latency samples | The prerequisite for replay, the Sunday report, and any human metric |
| **P0** | **RG-05** Safe Screen — validate before air | Reference · template · fit · output reachable. Refuse, and say so |
| **P1** | **RG-06** Update rollback + DB-compat preflight | The updater can deliver a fix; it cannot undo one |
| **P1** | **RG-07** Service replay | Built entirely from RG-04's data |
| **P1** | **RG-08** Sunday report | Same |
| **P1** | **RG-09** Degraded-mode state + banner | Make the fallbacks that already exist *visible* |
| **P1** | **RG-10** Room / environment profiles | Persist what the gate already learns |
| **P1** | **RG-11** Language Quality Centre | Turns LANGUAGES.md's honesty into an instrument |
| **P1** | **RG-12** Diagnostic bundle export | A file, not a screen |
| **P2** | **RG-13** 9 unnamed controls + the 2 real handlerless buttons | Accessibility, on the run surface |
| **P2** | **RG-14** p99 + persisted latency history | One line in `latency.rs`, plus RG-04 |
| **P2** | **RG-15** Synthetic end-to-end readiness test | Extends `boot/probes.js`; do not fork it |
| **P2** | **RG-16** Training mode / rehearsal replay | Same rig as `stt::realtime`, pointed at the UI |
| **P2** | **RG-17** Privacy screen | Answers "what is leaving this machine" in one place |
| **P3** | **RG-18** Contrast validation · distance preview · output accessibility mode | Needs a designer and a real projector |
| **P3** | **RG-19** Offline installer + signed language packs | High market fit; do after RG-06 |

### BLOCKED ON REAL-WORLD VALIDATION — no commit can close these

- **Stage F11** — a full service, watching for a rising per-minute line. *The highest-value unrun
  item in the project*, named as such by three documents and by none of the registers.
- Stages F1–F5, F9–F10, F12–F14 — church hardware, a quiet room, a noisy room, a quiet speaker.
- Stages A–E — the packaged build, the projector, the ATEM, the eyes.
- Word error rate, any language. Thirty minutes of real preaching on tape.
- One observed end-to-end update install.
- One full service run by a **non-author** operator.

### BLOCKED ON EXTERNAL DEPENDENCY

- Windows code-signing certificate (~$10/month, Azure Trusted Signing) — ROADMAP §1.
- A native speaker of Yorùbá, Kiswahili or Hausa, for the alias table and Yorùbá numerals.
- A decision on the product's name (README still says *"working name — rename freely"*).

---

## 22. Production readiness — scored per dimension, no hiding average

Scored against **the first ten churches**, not against enterprise scale.

| Dimension | Score | Note |
|---|---|---|
| Code | **9 / 10** | Typed errors, generic fire path, zero `unwrap` in the seven service modules |
| Performance | **8 / 10** | Excellent and measured — on one dev machine, on TTS audio, with no webview |
| Live safety | **9 / 10** | The strongest part. Six distinctions, each pinned |
| UX | **8 / 10** | Loses a point for the Live output badge and 9 unnamed controls |
| Security | **7 / 10** | Deliberate posture, honestly documented, correctly fixed once. No identity, no log |
| Privacy | **9 / 10** | Scrubbed, opt-in, offline-normal |
| Reliability / recovery | **7 / 10** | Good crash story. No rollback, no persisted history, no degraded state |
| Distribution | **5 / 10** | Blocked on a certificate; no rollback, no offline installer |
| Language (the moat) | **3 / 10** | Complete alias tables, zero measurement, zero native review, no Yorùbá numerals |
| Observability | **7 / 10** | A genuinely good instrument that forgets everything on quit |
| Documentation | **8 / 10** | Excellent, and drifting — see §26 |
| **Field validation** | **0 / 10** | Nothing. This is the number that decides the release |

**No overall score is given.** An average would hide the 0 and the 3, which are the only two that
matter right now.

---

## 23. Gap register

| ID | Gap | Impact | Evidence | Solution | Depends on | P | Complexity | Validation |
|---|---|---|---|---|---|---|---|---|
| ✅ RG-01 | Live's per-channel badge derives from global state | Operator believes a dead screen is On Air | `Live.svelte:973-979` vs `Channels.svelte:88-108` | Call `channelStatus()` on Live; badge per channel | RG-02 for truth | P0 | S | A component test that mounts Live and asserts a stale channel does not read On Air |
| ✅ RG-02 | No output heartbeat; liveness is a client count | "LIVE" cannot detect a frozen browser source | `channels.rs:963-991`, `:741-763` | Periodic anonymous ping/pong; last-seen per connection | — | P0 | M | Kill a kiosk client; assert the status flips within one interval |
| ✅ RG-03 | No Service Lock | A template edit or model change mid-service | grep: zero hits | Lock keyed on `Session`, not on the mic; every blocked action explains itself | — | P0 | M | e2e: start a service, assert the blocked commands refuse with a typed error |
| ✅ RG-04 | No event timeline; latency dies on quit | No replay, no report, no human metric, no evidence from a church | `docs/data/schema.sql`, `latency.rs` | Append-only `service_events` + `perf_samples`; retryable migration | — | P0 | M | Migration retryability test (CLAUDE.md rule 25); a service produces an ordered event list |
| ✅ RG-05 | No pre-air validation | Unfittable or unreachable content goes to air silently | `TemplateRender.svelte:131-160` | One validator in front of `Fire::output`; refuse and report | — | P0 | M | e2e: an over-long verse on a tiny template refuses rather than shrinking to unreadable |
| ✅ RG-06 | No update rollback, no DB-compat preflight | A bad update bricks a church until someone drives there | grep: zero hits; `db/mod.rs:51` | Keep the previous bundle; health-check after relaunch; compare `SCHEMA_VERSION` before install | — | P1 | L | Install a deliberately broken build; assert recovery |
| ✅ RG-07 | No service replay | Nothing can be reconstructed after Sunday | — | Timeline viewer over RG-04 | RG-04 | P1 | M | Replay a recorded service; every fire has a trace |
| ✅ RG-08 | No Sunday report | Churches cannot report, and you cannot learn | — | Derived view over RG-04 | RG-04 | P1 | S | Only metrics actually measured appear |
| ✅ RG-09 | No degraded state | Fallbacks are invisible to the operator | `dsp.rs:15`, `main.rs:2635`, `pipeline.rs:131` | One `Degraded` enum surfaced to the shell | — | P1 | S | Kill the model mid-service; assert the banner |
| ✅ RG-10 | Room calibration is lost on every start | The hall is re-learned each Sunday | `audio.rs:145-148`, `dsp.rs:146-153` | `environment_profiles`; store the learned floor, never a fixed threshold | RG-04 | P1 | M | Reopen; assert the floor is restored and still adapts (DECISIONS §19 must survive) |
| ✅ RG-11 | Language coverage is prose, not an instrument | The moat cannot be tracked or improved | `docs/LANGUAGES.md`, `numerals.json` | A per-language status view: aliases, numerals, review state, WER (or "not measured") | — | P1 | S | The view must render "NOT MEASURED" rather than a plausible number |
| ✅ RG-12 | Diagnostics is a screen, not an export | A church cannot send you what you need | `Settings.svelte:998-1054` | Export a scrubbed bundle; never include audio or transcript unless ticked | — | P1 | S | Assert the bundle contains no verse, lyric, announcement or transcript text |
| ✅ RG-13 | 9 controls with no accessible name | Screen-reader users cannot operate Live | `qa-inventory.mjs` | Add labels | — | P2 | S | Re-run the inventory; zero |
| ✅ RG-14 | No p99; no latency history | Tail behaviour and long-service drift are invisible after a restart | `latency.rs:728-729` | Report p99; persist samples | RG-04 | P2 | S | Diagnostics shows p99 and a prior-service comparison |
| ✅ RG-15 | Readiness never tests the spoken path | Green checks, dead microphone chain | `boot/probes.js` | Synthetic "say John 3:16" walk | RG-01 | P2 | M | Fail it by unplugging the mic; assert it goes red |
| ✅ RG-16 | No training / simulation mode | A volunteer's first Sunday is their first attempt | — | Replay recorded audio through the real pipeline into a sandboxed UI | RG-07 | P2 | M | Nothing reaches a real output — watch the **hub**, not the wall |
| ✅ RG-17 | No privacy screen | The best privacy story in the product is invisible | `PRIVACY.md` | One screen stating current state | — | P2 | S | Reflects the real setting, never a hardcoded "off" |
| ✅ RG-18 | No contrast validation, distance preview or output accessibility mode | Unreadable from row 20, and nothing says so | grep: zero | Ratio check + a distance simulator | — | P3 | M | Needs a real projector |
| ✅ RG-19 | No offline installer or language packs | A church with poor internet cannot install | `models.rs` | Bundle app + model + corpus; sign language packs | RG-06 | P3 | L | Install with the network cable out |
| RG-20 | **Doc drift** — 114 / 4,024 / "35 decisions" / e2e layer count wrong across six files | `CLAUDE.md` is read first by every agent and is on the wrong side | §26 | Fix, and add the reproducing command beside every count | — | P0 | S | `grep -rn '114 cmds\|4,024\|4.0k lines\|35 decisions' CLAUDE.md docs/` returns nothing |

---

## 24. GO / NO-GO

**NO-GO**, unchanged, and for the same reason as `audits/QA-2026-08-14.md` §20 and
`audits/PERF-2026-08-24.md` §7: **roughly half of Relay, as a volunteer experiences it, has never
been reached by any instrument in this repository.**

Nothing in this report changes that, and nothing in this report is a reason to change it. RG-01
through RG-05 are real defects worth fixing, and fixing all five would still not produce a GO —
because the condition is not a defect count, it is a Sunday.

**What would change it:** Stage A (a packaged, signed build that survives the microphone
entitlement), Stage B (a projector), Stage C (a person at a microphone), Stage F11 (a full
service), and one church.

---

## 25. Checklist

### Do now — documentation truth (this branch)

- [x] Write this report
- [x] `CLAUDE.md` — 114 → 118 (two places), 4.0k → 4,369
- [x] `CLAUDE.md` — document the boot ladder + Dashboard readiness surface under *Frontend shape*
- [x] `CLAUDE.md` — add rule 35 (the Live output badge), and a pointer to this file
- [x] `AGENTS.md` — create; tool-agnostic; restates **no** counts
- [x] `.claude/agents/relay-qa-live-path.md` — add the Live-vs-Outputs health asymmetry
- [x] `.claude/agents/relay-qa-surface.md` — add the boot/Dashboard readiness surface and the inventory's open findings
- [x] `.claude/agents/relay-qa-failure.md` — add "no rollback" and "latency dies on quit"
- [x] `.claude/agents/relay-qa-auditor.md` — make doc drift a first-class finding class
- [x] `docs/README.md` — index `GPT.md`, `RELAY_GAP.md`, `audits/PERF-2026-08-24.md`; 35 → 38; re-stamp the sweep
- [x] `docs/ROADMAP.md` — fix the §23 miscitation; 4,024 → 4,369; 114 → 118; capture.js 1,908 → 1,941; add Stage F11 to §1
- [x] `docs/QA_HARNESS.md` — §4.1's "23 tests" → 28 (26 run, 2 ignored); drop the stale 114 quote; buttons 319 → 321; name the right command for the frontend count
- [x] `docs/PRODUCT_AUDIT.md` — one line in the existing staleness banner. Do not rewrite Rev 3
- [x] `CHANGELOG.md` — add the missing `[0.1.0-4]` entry
- [x] `docs/GPT.md` — track it, and fix the one stale number (`main.rs` ~4,000 → 4,369)

### Then — P0 engineering

- [x] RG-01 · RG-02 · RG-03 · RG-04 · RG-05 — done 2026-08-29, see the fix log above
- [x] RG-06 … RG-12 (P1) — done 2026-08-29, same fix log
- [x] RG-13 … RG-17 (P2) — done 2026-08-30, same fix log
- [x] RG-18 · RG-19 (P3) — done 2026-08-30, same fix log

### What is left, and none of it is a commit

- [ ] **Stage C** — a person, a microphone, a real room. Blocks RG-10's audio seed,
      RG-16's replay, and the word-error-rate measurement the moat rests on.
- [ ] **Stage B** — a projector, to check RG-18's thresholds against a wall.
- [ ] **Stage F11** — one full service, watching Diagnostics for a rising line.
- [ ] **A native speaker** — RG-19's language packs and §47's empty column.
- [ ] **A Windows code-signing certificate** — the platform most churches are on.

### Always — before any of the above ships

- [ ] Stage F11. Everything else is a smaller number than this one.

---

## 26. Document actions — delete · merge · update

### Delete: **nothing**

Verified with `git check-ignore -v`: `ruvector.db`, `.DS_Store`, `docs/.DS_Store` and
`.claude/RESUME.md` are all untracked **and** gitignored. No stray file is tracked.

**`PROMPT.md` must NOT be deleted**, despite reading like a superseded build prompt. The Rust
module docs (`audio.rs`, `stt.rs`, `detection.rs`, `router.rs`, `channels.rs`) and
`src-tauri/capabilities/default.json` cite its phase numbers as their rationale anchor. Recorded
here so a future tidy-up does not remove the target of live citations.

### Merge: **nothing**

`SPEC.md` (the frozen 2026-07-02 brief), `PRODUCT_AUDIT.md` (a human scorecard) and `audits/*`
(frozen machine audits) are deliberately different altitudes, and `docs/README.md` records why an
audit that edits its own history stops being evidence. This file joins them; it replaces none.

### Update: the drift table

| File · line | Says | Should say |
|---|---|---|
| `CLAUDE.md:101` | `4.0k lines, 114 cmds` | `4,369 lines, 118 cmds` |
| `CLAUDE.md:153` | "the 114 registered commands" | 118 |
| `docs/QA_HARNESS.md:29` | 594 frontend | **594 is correct — keep it**, and name `npx vitest run` as the command. See the box in §0 |
| `docs/ROADMAP.md:25` | "(DECISIONS §23)" for the Windows signing gate | §23 is the voice gate — cite the live-safety table row, not §23 |
| `docs/ROADMAP.md:106` | `4,024 lines / 114 commands` | `4,369 / 118` |
| `docs/ROADMAP.md:107` | `capture.js` 1,908 | 1,941 |
| `docs/ROADMAP.md` §1 | — | Add Stage F11 |
| `docs/README.md:4` | "last full sweep 2026-08-20, against `07654a7`" | Re-stamp |
| `docs/README.md:62` | "35 decisions deep" | 38 numbered (+28 unnumbered rows) |
| `docs/README.md` §hierarchy | — | Index `GPT.md`, `RELAY_GAP.md`, `audits/PERF-2026-08-24.md` |
| `docs/QA_HARNESS.md:479` | e2e "23 tests" | **28 (26 run, 2 ignored)** — §0 line 30 is the reproducible one and was right. A first pass here got this backwards by counting `#[ignore]` inside doc-comments: `grep -n '#\[ignore' src-tauri/src/e2e.rs` returns 5 lines and only 2 are attributes |
| `docs/QA_HARNESS.md:32` | 319 buttons | 321 |
| `docs/QA_HARNESS.md:589` | quotes CLAUDE.md's 114 as current | 118 |
| `docs/PRODUCT_AUDIT.md:18` | `4,024 / 114` | Add a current column to the existing banner |
| `CHANGELOG.md` | no `[0.1.0-4]` | Add it |
| `docs/GPT.md:489` | `main.rs` "~4,000 lines" | 4,369 (its e2e figure at `:332` is correct — it copied the right table) |

**The rule this drift earns, and it is the one worth keeping:** *a count that appears in more than
one file will drift, and the copy that drifts is the one an agent reads first.* `QA_HARNESS.md`
§0 is the register of counts, and every count carries its command. Other documents should cite
that table rather than restating it — which is exactly why the new `AGENTS.md` states no numbers
at all.
