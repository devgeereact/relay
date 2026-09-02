# Relay — Gap Report against the Product Transformation Brief

**2026-08-29 · verified against `0338244` (branch `main`), version `0.1.0-4`.**

This document answers one question: **the transformation brief proposes ~80 things — which of
them already exist, which are half-built, which are missing, and which would break a decision
this product has already made and recorded?**

It began as the *output* of `docs/GPT.md` — since deleted (see below) — a paste-into-any-model prompt that front-loaded
Relay's constraints so a model argued inside the product's shape instead of recommending
accounts, RBAC and cloud sync at it.

> **`docs/GPT.md` was deleted on 2026-08-30, deliberately.** It was 770 lines that restated
> `CLAUDE.md`, `AGENTS.md` and this file for a model with no repository access — and it carried
> its own copy of every count, all of which had drifted (§18). `AGENTS.md` is now the
> tool-agnostic contract for an agent working IN the repo, and this document is what the
> repository answered. A third copy of the same constraints was a third thing to keep true, and
> it was already the least true of the three.

> **This report changed no engine code.** Its §21 buckets and §25 checklist are proposals to a
> human, not work in progress. That is the brief's own §79 rule — *establish what exists, then
> what is wrong, then what is missing, then implement* — and Relay's own scoping rule.

> ## Fix log — 2026-08-29
>
> **RG-01 … RG-05 are closed**, in the order the register ranked them, with the
> reasoning recorded as [DECISIONS](../DECISIONS.md) §39–§42 and the tests named below.
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
> | **Two code-signing certificates** | shipping at all. **Neither platform has one** (RG-73) — Windows is the platform most churches are on, and macOS is the one that has actually run a service |
>
> That is the same list `KNOWN_ISSUES.md` §1 has carried since before any of this work, and
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
>
> ---
>
> ## Fix log — 2026-08-30 · the two open defects
>
> With the register empty, the only in-repo work left that was **not** blocked on the
> world was the pair of defects `e2e.rs` had been carrying as `#[ignore]`d tests since
> the R2 audit — each with its own repair direction written into the test. Both are
> closed, reasoning in [DECISIONS](../DECISIONS.md) §54, and `e2e.rs` now has **no ignored
> tests at all**.
>
> | ID | The defect | Fix |
> |---|---|---|
> | **R2-C** | A spoken in-passage jump that could not move said nothing — no toast, no banner, no log line. The fourth door into the bug `NavResult` was built to prevent | It reports `NoPassage`/`NotInLibrary`, and **announces its own outcome** rather than returning it to its one caller |
> | **R2-D** | A song, notice, picture or countdown left the previous reading armed, so `nav("next")` walked a passage the congregation had stopped looking at and answered `Fired` | `ContextMemory::forget`, called at `broadcast_with_clock` — the choke point, so every content path is covered at once |
>
> Both tests were re-run with the defect deliberately reintroduced, and both fail again.
> **The release decision is still NO-GO**, unchanged and for the unchanged reason: two
> closed defects are not a Sunday.
>
> ---
>
> ## Fix log — 2026-08-30 · RG-21 · RG-22 · the packaged build
>
> **The application was rebuilt and re-verified from source** before this round:
> `npm run tauri build` produced `Relay.app` and `Relay_0.1.0-4_aarch64.dmg`, and
> `./scripts/sign-local.sh` reproduced §17's conditions on the bundle — hardened
> runtime ON, microphone entitlement present, usage string present. That is the
> check that cannot be made from source, because the trap it exists to catch only
> appears on a signed build.
>
> Two new register entries, closed together. **RG-20 (doc drift) is also ticked** —
> its own validation command has returned nothing since the first round and had
> simply never been marked.
>
> | ID | What changed | Pinned by |
> |---|---|---|
> | **RG-21** | A song's running order can be built at all — the editor the whole arrangement chain was missing. `song_arrangements` moves from WRAPPER ONLY to a create path, and the repository has no dead command left | `arrangements.test.js` (13), `qa.rs::a_component_can_create_a_song_arrangement`, `surface.test.js` R3-12 |
> | **RG-22** | An arrangement whose sections moved is marked, refused into a plan, and repaired by a person — never remapped by guessing. The same rule on the plan-cue door | `db/mod.rs` (3 new), both re-run with the check disabled and failing |
>
> **They had to ship together.** RG-22 was latent only because nothing could create
> an arrangement; the editor is what would have made it reachable. Shipping RG-21
> alone would have introduced a path to the wrong words on a wall.
>
> **What was deliberately not built: automatic remapping.** Matching an old section
> index to a new one means guessing whether a section was moved or replaced, and a
> wrong guess is indistinguishable from a right one until it is on a screen in front
> of a congregation. DECISIONS §55.
>
> Counts **on 2026-08-30**, and left at that date on purpose: 595 Rust (26 ignored)
> + 857 frontend, `fmt` and `clippy -D warnings` clean, inventory at zero unnamed
> controls and zero handlerless buttons. A fix log is a dated record, so its numbers
> are not maintained — §0 holds the current ones and the commands that produce them.
> **Still NO-GO** — the list below did not move.

---

## WHERE THIS IS UP TO — read this first

*Regenerate the counts with `npx vitest run src/lib/relaygap.test.js` (it checks the table)
and by reading §23. Last updated 2026-08-31.*

**81 entries. 77 closed, 1 withdrawn as wrong, 3 not closed — and only two of those three are
work** (RG-32 open, RG-41 and RG-50 flagged).

*These four numbers are asserted against the table itself by `relaygap.test.js`; they drifted
apart twice (this block once said 54 and 51 in consecutive sentences) and a count in prose beside
the table it counts is the easiest possible thing to check automatically.*

| | |
|---|---|
| ✅ **77 closed** | Every gap this report could reach from the code. **The last thirty-four were not in the brief at all — they were found by auditing the things that audit Relay**, and that is now the most productive seam in the project: a contract test that scanned one Rust file, another that read four frontend files out of nine, a CI job that ran one Node version so the fix for every other one was never exercised, a launch screen whose checks could not fail, an edit-time gate whose watch list had drifted behind four safety files, an agent brief describing a deleted component and a closed defect, four citations pointing at a section that does not exist, sixteen more pointing at a directory that is not in the repository, and a register table this report corrupted itself. **The instrument is wrong more often than the code it audits.** |
| ~~1 withdrawn~~ | **RG-27** was filed from a mid-service snapshot and was wrong. Struck through, not deleted |
| ⚠️ **RG-41** | Not work. A correction kept on the record: two of six views were wrong to "fix" — they are routers, and a heading there would give one screen two |
| ⏳ **RG-32** | **Open on purpose.** A context-resolved bare verse is labelled `Direct` at a hardcoded 0.88 — by rule 10 that label is a lie, because Relay inferred the book rather than hearing it. Changing it makes every in-passage *"verse eighteen"* cost a click, and **one service is not enough evidence to spend that**. Wants a second and third Sunday |
| ⚠️ **RG-50** | **Needs a human, not a commit.** `translations` is the last table an operator cannot fill. The reason first recorded here was wrong — see §19b for the decision it actually needs |
| ❓ **§19b** | **Two open questions, neither a commit.** The translation one above, and — added 2026-08-31 — whether a detection should carry the audio quality it was heard in. The measurement exists (`dsp::AudioQuality`); this report previously said it did not, and that error is corrected in place |

### What is left is not a list of bugs

Nothing in the six rows above is what stands between Relay and a church. **This is:**

| | Why no commit closes it |
|---|---|
| **A projector** | RG-18's contrast and distance thresholds have never been checked against a wall |
| **The audio half of a real room** | A preacher has now been transcribed for 49.5 minutes — but **word error rate has never been measured, in any language.** That is the moat, and it is still an assertion |
| **A Yorùbá speaker** | The numerals are unparsed and the alias table is unreviewed. A wrong numeral does not fail safely; it silently shows a different verse |
| **Two code-signing certificates** | **Neither platform has one** — Apple's six `APPLE_*` secrets are absent as well as Windows's, so every release so far is unsigned on both. The macOS chain is wired and `sign-local.sh` reproduces its conditions ad-hoc, which is not the same as having shipped one (RG-73) |
| **A second and third service** | RG-32 waits on them, and so does any claim that 2026-08-30 was typical |
| **One service run by somebody who did not write Relay** | The largest unknown in the project |

### The one number that moved

**Field validation, 0 → 2 / 10.** Relay ran a live sermon on 2026-08-30
([`audits/FIELD-2026-08-30.md`](audits/FIELD-2026-08-30.md)): Stage F11 answered — **no
drift** across 49.5 minutes and 2,423 decodes — and five of six auto-fires correct.

**That morning is the argument for NO-GO, not against it.** Fifty minutes in a room produced
**seven findings** that months of reading source had not, including a wrong verse on a
congregation's wall. One of the seven was itself wrong and was withdrawn. That ratio is what
"half of this product has never been reached by an instrument" means when somebody finally
reaches it.

---

## 0. Method, and what this report cannot see

**How every claim here was produced:**

| Kind of claim | How it was established |
|---|---|
| Counts | A command, named beside the number. Re-run it; if it disagrees, this file is the bug — and see the standing note below about what that disagreement means |
| "EXISTS / PARTIAL / MISSING / DECLINED" | Read the code, and cite a **file and a symbol** — never a line number (§2's preamble says why) |
| Latency figures | Quoted from `audits/PERF-2026-08-24.md` and `audits/PERF-MODELS-2026-08-30.md`, not re-measured. Those documents state what their own numbers do not establish, and none of that changed |
| Detection behaviour | Quoted from the corpus gate (`eval.rs`), the closed findings in `audits/QA-2026-08-14.md`, and the one live service in `audits/FIELD-2026-08-30.md` |

**Counts: this document does not keep its own copy any more.**

[`QA_HARNESS.md`](QA_HARNESS.md) §0 is **the** register of counts for this repository —
every row there carries the command that reproduces it. This section used to restate a
dozen of those values and, on 2026-08-31, restated them wrongly **within the same day**:
`main.rs` was measured at 5,573 lines in the morning and was 5,640 by the afternoon,
because the afternoon's own commits changed it. That is the fifth time these numbers have
been corrected in a week (§18) and the fourth time this document did the correcting.

So the two that drift fastest are named by their command and nothing else:

| | Command |
|---|---|
| `main.rs` size · registered `#[tauri::command]` | `wc -l src-tauri/src/main.rs` · `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| `stores/capture.js` size | `wc -l src/lib/stores/capture.js` |
| Rust and frontend test totals | `cd src-tauri && cargo test` · `npx vitest run` — **the runner's own summary line**, never a grep |
| Components · controls · the command map | `node scripts/qa-inventory.mjs` |
| Tables in the schema | `grep -c 'CREATE TABLE' docs/data/schema.sql` |
| Numbered decisions | `grep -cE '^## [0-9]' docs/DECISIONS.md` |
| Labelled detection cases | `python3 -c "import json;print(len(json.load(open('src-tauri/data/eval_corpus.json'))['cases']))"` |

> ### The standing note about counts — read this before "correcting" one
>
> **Every number this document ever wrote down was wrong within a week, and twice within
> a day.** That is not carelessness; it is what a number in prose *is*. RG-20 was filed
> because six documents disagreed about the size of `main.rs`; this report corrected them
> to 4,369 / 118 / 1,941 on 2026-08-29; §18 corrected *those* to 5,723 / 137 / 2,195 on
> 2026-08-30; a fourth pass said 5,573 / 132 / 2,156 on 2026-08-31 — and by that
> afternoon it was 5,640 / 132 / 2,159.
>
> **The durable half of every row is the command, not the value.** Three counts here were
> also wrong because a plausible one-liner was believed over the tool that actually knows:
> `npx vitest list | wc -l` counts compiler warnings on the same stream, and
> `grep -n '#\[ignore'` counts the phrase inside doc comments (`e2e.rs` has **three** such
> lines and **zero** ignored tests). **A grep is not a test runner.**

**What no instrument in this repository reached, and this report therefore does not claim:**

pixels out · hardware other than one M4 Pro · a second operator · a congregation that did not
know it was a test. **Audio in and a packaged build are no longer on that list** — a live
sermon was transcribed for 49.5 minutes on 2026-08-30 (`audits/FIELD-2026-08-30.md`) and the
signed-bundle conditions were reproduced on a real `.dmg` with `scripts/sign-local.sh`.
Anything below marked **BLOCKED** is blocked on the world, not on a commit.

---

## 1. The verdict, on one page

**The brief's single highest-priority recommendation — "Phase 1: make Relay extremely fast",
"⚡ Real-Time Speech Engine 2.0", "faster partial transcription (P0, build now)" — was already
built, already measured, and already hit most of the brief's own targets five days before the
brief was written.**

| Brief's target (§10) | Relay, measured | Source |
|---|---|---|
| First visible partial ≤ 300 ms | **139 ms** median on `ggml-base` | PERF §4 |
| Perceived transcript lag ≤ 1 s | **P95 339 ms** on `base` | PERF §4 |
| Dropped partials < 1 % | **0 of 1075** passes | PERF §4 |
| Real-time factor < 0.7 | decode 144 ms per 200 ms cadence step → **~0.72 duty**, and the cadence *is* the decoder's own speed by construction | DECISIONS §38 |
| Mic → screen ≤ 2 s p95 | still **NOT MEASURED** — it needs a stopwatch in a room, not an instrument (§61) | PERF §5 |

Two of the brief's specific mechanisms are worse than what shipped: a **two-speed / short-window
fast path** (§6, §8) buys nothing, because whisper pads its mel window internally and an 8 s and
a 4 s window cost the same (DECISIONS §36); and **adaptive window sizing** (§11) is superseded by
adapting the *cadence* to measured decode cost, which is the lever that actually moved 349 ms →
139 ms.

**The five weaknesses this section named on 2026-08-29 have all been closed.** They are listed
here rather than deleted, because a verdict page that quietly loses what it used to claim cannot
be checked:

| What was weak on 2026-08-29 | Now |
|---|---|
| **Output truth** — Live derived every badge from global state, so a kiosk that went away still read *On Air* | Closed, RG-01/RG-02. Live and the Outputs tab read one backend fact through one helper, and every output page reports that it is still painting |
| **Nothing survived a quit** — every latency measurement died with the app; no timeline, no replay, no report | Closed, RG-04/RG-07/RG-08/RG-14. `service_events` + `perf_samples`, a replay, a Sunday report, p99 and a week-on-week trend |
| **No rollback** — a bad install had no way back | Closed, RG-06 — for the **data**, which is the half that cannot be got back. Binary rollback is deliberately not built (DECISIONS §43) |
| **No pre-air validation** — fit was measured, never *validated before* air | Closed, RG-05. One validator at the one door content leaves by (DECISIONS §42) |
| **The moat is unmeasured** | **Unchanged, and it is now the whole list.** WER: zero, in every language. Native-speaker review: zero. Yorùbá numerals: absent |

**And two of the brief's proposals would reverse recorded decisions** — LAN device pairing
(DECISIONS §35) and an optional cloud layer (ROADMAP §3). Both are written up as reversal
*proposals* in §20. Neither is adopted, and §2 now records them as **DECLINED** with the reason
in the row rather than as gaps.

**So the honest one-line summary has inverted since the first pass.** On 2026-08-29 this report
said *the fast half is done, the trustworthy half is thin*. Both halves are now built, and
**every remaining item is blocked on something that is not a commit**: a projector, a Yorùbá
speaker, two code-signing certificates (neither platform has one — RG-73), and nine more Sundays.

**Release decision: NO-GO for general release · GO for a supervised pilot** — made on
2026-08-31, recorded in §24 and DECISIONS §60, and unchanged by this pass. Not because of
anything in this report: because one service, watched by the person who wrote the software, is
one service.

---

## 2. Status matrix — the brief's §4–§77, one row each

*Re-verified 2026-08-31 against the working tree, after RG-01 … RG-22 and the two `e2e`
defects closed. **31 rows moved.** The 2026-08-29 version of this matrix is not preserved:
every row it got wrong it got wrong by being a week old, and a stale matrix beside a current
one is two answers to the same question.*

> **The evidence column names a FILE and a SYMBOL, never a line number.** §18 is the reason:
> this document has already corrected three counts that were wrong again within the week, and
> `file:line` rots the same way — faster, and more convincingly, because a wrong line number
> still looks like a citation. A symbol survives an edit above it; where a test pins the claim,
> the test is named instead, because that one cannot rot silently at all.

Legend: **EXISTS** (implemented and reachable from a rendered control) · **PARTIAL** ·
**BROKEN** · **MISSING** · **DECLINED** (would build the wrong thing, with the reason) ·
**N/A** (conflicts with the product's scope) · **FUTURE** (valid, deferred) ·
**BLOCKED** (waits on the world, not on a commit).

### Reliability, readiness and the live-service envelope

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 4 | Relay Reliability Engine — one unified READY / DEGRADED / ACTION REQUIRED state | **EXISTS** | Three layers, one answer: 21 launch probes on a 4-level severity ladder (`boot/probes.js`, `boot/boot.js`), a rolled-up verdict on the readiness screen (`views/Dashboard.svelte`), and — since RG-09 — a live one-line degraded state in the **shell**, on every tab, opened for the detail (`degraded.js`, mounted in `App.svelte`; DECISIONS §45) | Nothing that should be built. **An engine-side health state is deliberately NOT built** — see DECISIONS §61. The only thing Rust could do with one is refuse to put content on a screen, and a gate that can refuse the wall is the failure mode §20 and §42 both exist to prevent |
| 5 | Sunday Readiness screen | **EXISTS** | The Dashboard hero says "Ready for a service." / "N things not working." over the same `makeProbes()` the launch ladder runs — and RG-15 added the part that was actually missing: **say one verse and watch six stages** between a microphone and a screen (`pathcheck.js`, driven from `Dashboard.svelte`, pinned by `pathcheck.test.js`). It runs in rehearsal or not at all | Nothing. The 21 probes can all pass on a machine where nothing works end to end; that is precisely what the walk exists to catch, and it says which stage was not reached rather than blaming the next one |
| 6 | Service Lock — block destructive config while live | **EXISTS** | `servicelock.rs`. **16 actions** held back while a service records — nine `delete_*`, plus the six that take the engine away mid-sermon (model download / swap / reload / install-from-file, the Bible translation, media import, saving an import). Enumerated, never predicate-matched. The operator lifts it in one action. `e2e::a_recorded_service_holds_back_a_deletion_but_never_the_wall`, `servicelock.test.js` | Nothing. **Nothing on the fire path is protected and that is the design** (DECISIONS §40) — panic, nav, fire, rehearsal, sensitivity, template swap and template editing are all deliberately unaffected, because each is a repair tool at 10:31 |
| 30/63 | Graceful degradation + a formal failure matrix | **EXISTS** | RG-09. Every fallback that was previously silent now produces a row saying what it costs the service and what to do: denoise off below 48 kHz, audio-only with no model, a CPU-only build, detection disarmed, safe mode, rehearsal. **Nothing is inferred** — a row appears only when something Relay measured says so (`degraded.js`, `degraded.test.js`; DECISIONS §45) | Nothing. The failure matrix the brief asked for is this table plus SECURITY.md's T1–T10, both in-repo |
| 31/32 | Automatic recovery, operator-confirmed | **EXISTS** | `RecoverSession.svelte` restores **position only** and says so; `liveOnAir` is a separate fact precisely so it cannot be restored (`qa-r5-onair.test.js`). Since RG-04 the service's own record survives the crash as well (`service_events`) | A crashed service still leaves an open `services` row with no in-app way to close it. It is cosmetic in the history list and has never cost anything live |
| 33/50 | Panic system: Clear · Black · Restore, and never claim an unachieved success | **EXISTS** | DECISIONS §20. `panicRun` returns a boolean **and** sets `panicError`; shell-level panic bar in `App.svelte`; `shortcuts.js` suppresses `Esc` while a dialog is mounted; the pre-air validator deliberately cannot touch a panic control (DECISIONS §42) | **Restore-what-was-there is still not a control, deliberately.** Putting back content the operator cleared, without them choosing it again, is the one panic action that can itself put a wrong verse on a wall. Clearing is safe by construction; un-clearing is not |
| 34/46 | Emergency manual mode when the AI is unavailable | **EXISTS** | Manual fire is first-class and always present (`Live.svelte`); detection can be disarmed (`toggleDetection`); safe mode disarms it wholesale — and RG-09 is what closed this: a lost model, a disarmed detector or a CPU-only build now each say so **in the shell, on every tab**, in the operator's words | Nothing. There is still no *mode* called "manual", and there should not be: manual is not a mode in Relay, it is the floor the product stands on |

### Speech, transcription and detection

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 7 | PARTIAL / STABLE / FINAL transcript states | **DECLINED** | PARTIAL and FINAL exist end to end (`TranscriptUpdate.is_final`, `applyTranscript`, and a visually distinct render — partial in `<mark>` with a caret, finals as timestamped rows) | **STABLE is the wrong instrument for the problem it was proposed to solve.** A stable-prefix state describes the *text*; the harm is a wrong *verse*, and the router already holds a reference from a partial window at `Suggest` until a second pass agrees (`router::decide_live`, CLAUDE.md rule 28). A text-level state would add a third vocabulary to the same fact and could not gate anything the corroboration rule does not already gate |
| 6/8 | Two-speed transcription (fast path + accuracy path) | **N/A as framed** | DECISIONS §36 and PERF §4: whisper pads the mel window internally, so a shorter window costs the same; above `base` the model is the entire remaining latency | A genuinely cheaper second decoder is a different *model*, not a different window — a large piece of work, and not what §8 describes |
| 9 | Streaming / rolling / incremental decoding | **MISSING** | Every emission is a whole-window `state.full()` re-decode (`stt.rs`) | Token-level streaming is a whisper.cpp capability question, not a Relay wiring question |
| 10 | Hard latency acceptance targets | **EXISTS** | `audits/PERF-2026-08-24.md` §5 scores against a target table per model and marks three rows **MISS** and four **NOT MEASURED** rather than rounding them off; `audits/PERF-MODELS-2026-08-30.md` adds what each model costs an operator in cadence | Targets on **church hardware**. Every number is one M4 Pro (§72) |
| 11 | Adaptive inference | **PARTIAL — superseded** | The cadence adapts to measured decode cost (`step_samples_for`, EMA, clamped to one chunker hop … 1000 ms). That is the lever that moved 349 ms → 139 ms (DECISIONS §38) | `sysprobe.rs` is **advisory only** — nothing in the pipeline branches on probed hardware. `gpu_backends` is a compile-time fact, deliberately (DECISIONS §36) |
| 12 | Scripture candidate prefetch during the utterance | **DECLINED** | Detection already runs off the decoder's thread behind a bounded queue (CLAUDE.md rule 33) and costs ~2.6 ms/query on 31k verses | Prefetch optimises the cheapest stage in the pipeline. §3 and CLAUDE.md rule 31 |
| 13 | Voice confidence shown separately from claim type | **PARTIAL** | Claim type is first-class and correct: amber chip + meter for Direct, cyan chip and **no number at all** for a guess (`detect.js`, `Live.svelte`; DECISIONS §21). RG-63 added the second question beside it — *is there a verse behind this reference at all?* — because a confidently-heard "Psalms 23:99" is a claim the operator could not previously distinguish from a working one | There is still no **voice** confidence *per detection*. Audio quality is surfaced continuously (clipping / too quiet / noisy) but is never attached to the claim it was measured beside. **An earlier draft of this row said that would need an SNR "the decoder does not currently expose" — that was wrong and is corrected here rather than quietly amended:** `dsp::AudioQuality` already carries `snr_db`, `clip_ratio`, `speech_prob` and a `warning` naming the single most important thing wrong right now. What is missing is a *decision*, not a measurement — see §19b |
| 14 | Explainable detection — "why this verse?" | **EXISTS** | `DetectionInspector.svelte`, opened from Live. Renders real evidence only — the parsed span, or the shared rare-word chips — and explicitly refuses to fabricate reasoning. `inspector.test.js` | Nothing |
| 15 | One Scripture Safety Firewall | **EXISTS** | Closed by RG-05. `Fire` is constructed only via `resolve_fire`; `broadcast_with_clock` is the **one** caller of `channels::broadcast_content`, and `pipeline::preflight` sits in it — so the AI path, the manual box, spoken nav, plan cues, media, the emergency announcement and the countdown are validated by one gate (CLAUDE.md rule 36, DECISIONS §42) | Nothing. It refuses only what is unambiguously broken and silently so; it never checks that a screen is attached, and the panic controls do not pass through it at all — both deliberate |
| 27 | Church-local vocabulary learning | **PARTIAL** | `voice_profiles.bias_terms` is editable in Settings and feeds whisper's decoder prompt | No learning. Nothing observes what this church actually says and adapts ranking. Blocked on the same thing as §43: nobody has measured whether it would help |
| 28 | Speaker profiles | **PARTIAL** | `voice_profiles`: name, language, bias vocabulary, sensitivity, learned `auto_fire`/`suggest` pair. RG-10 attaches one to a room | No speaking-rate, typical-volume, microphone or common-books calibration |

### Outputs, screens and templates

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 16 | Safe Screen — validate before air | **EXISTS** | RG-05. `pipeline::preflight` refuses a payload that would paint an empty screen, or one carrying a template the output page cannot parse, at the single choke point — and the screens are left exactly as they were (DECISIONS §42). `safescreen.test.js`, two `e2e` tests | Nothing that should be added. It deliberately does **not** refuse for an unreachable screen (a service runs on the console preview all the time) and deliberately cannot refuse a panic control |
| 17 | Output health monitoring | **EXISTS** | RG-01. Live and the Outputs tab now decide from one backend fact through one shared helper (`outputHealth.js::describeScreen`, imported by `views/Live.svelte`), so a screen that is not answering can never read amber on the one surface an operator watches. `outputhealth.test.js` | Nothing. `network_client` is still reported as served-not-seen, which is what it is |
| 18 | Output heartbeats | **EXISTS** | RG-02. Every output page reports that it is still painting — the native window over the bridge (`output_beat`), kiosk/OBS over the socket it already has (`{"kind":"beat"}`). The grace window is derived from the interval, not written beside it, and a lost beat degrades to "silent", the safe direction (`channels.rs::OutputHealth`) | Nothing. It is **anonymous by construction** — a beat says "the screen for channel N painted", never who or from where (DECISIONS §39 narrows §35 by one word: *when*, never *who*) |
| 35 | Automatic text-fit safety | **EXISTS** | Fit is measured, not assumed (`TemplateRender.svelte::fitOne`, binary search for layered templates) — and since RG-05 a fit that has shrunk below **45 % of the size the template's designer asked for** reports it, and Live says how small it went. The floor is a RATIO, because cqw is a share of the output's width (CLAUDE.md rule 37) | Nothing. It still shrinks and still shows the verse below the floor — blanking a screen is strictly worse for a congregation |
| 36 | Distance preview (5/10/15/20 m) | **EXISTS** | RG-18. `legibility.js::PREVIEW_DISTANCES_M` + `previewScale`, rendered in `views/templates/TemplateEditor.svelte`. The arithmetic uses two numbers only a person can know — image width and back-row distance — remembered with the room, and with no numbers there is **no verdict** rather than a guess | **The thresholds are unverified and the UI says so.** WCAG is a specification for screens at arm's length and the character-height rule is broadcast practice; neither has been checked against a projector. Stage B |
| 37 | Accessibility mode (high visibility) | **EXISTS** | RG-18. High Visibility is a **theme** (`themes.js`), not a parallel renderer — so it inherits the one template engine and cannot drift from it (`legibility.test.js`). Contrast is computed exactly over a solid background and reported as **"cannot be checked"** over a picture, never guessed. The console side was already served: `sr-only` live regions, `role="meter"`, `trapFocus`, reduced motion | Nothing in the product. The same Stage B caveat as §36 |
| 54 | Signed template library / marketplace | **FUTURE** | Deferred with reasoning (`KNOWN_ISSUES.md`) — plugins must never modify the presentation engine | Agreed; not v1. Blocked on the same absent signing ceremony as §44 |
| 44/45 | Distance/fit/contrast gate before air | **EXISTS** | Consolidates §16, §35, §36, §37 — one validator (`pipeline::preflight`), one refusal path, one honest message, plus the editor-side review that catches it before Sunday | Nothing |

### Security, privacy and trust

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 19/20/21 | Local device pairing · trusted devices · authenticated LAN | **DECLINED — would reverse DECISIONS §35** | The `:8032` control plane is unauthenticated **deliberately**; the `:8031` hub is broadcast-only and counts clients without recording who they are. What *was* closed (2026-08-20) is the bystander-browser vector: mutating routes require `POST` and are denied the CORS wildcard (SECURITY.md T2, T3) | Nothing to build. §20 (a) is the written-up reversal proposal and it is **not adopted** — the preacher's phone has no way to hold a credential |
| 22 | Local security event log | **DECLINED — follows §19** | No table, no events | It would log an identity that, by §35 and §39, is deliberately never established. A log of anonymous events is a log of nothing |
| 23 | Service history as an event timeline | **EXISTS** | RG-04. `service_events` + `perf_samples` — an ordered record that survives the app, merged with detections and cues on the way out (`db/services.rs`, `views/library/History.svelte`). Output-lost and output-recovered are real events now that §18 detects them | Nothing. **It carries nothing a preacher said**, pinned from both sides (`timeline_tests::nothing_a_preacher_said_reaches_the_timeline`, `timeline.test.js`) |
| 24 | Tamper-evident service record | **DECLINED** | The prerequisite (§23) now exists, so this became cheap — and cheap is not the test | A hash chain defends against somebody with the machine, and the malicious operator is explicitly out of scope (SECURITY.md T10). Building it would claim a guarantee against the one actor Relay has already said it does not defend against |
| 25 | Service replay | **EXISTS** | RG-07. A timeline row opens to show what was being said around it, what Relay decided, and how fast it was going (`report.js::replayAt`, `History.svelte`) | Nothing. It replays the **record**, not the audio — Relay never stores audio (SECURITY.md T1) |
| 26 | Sunday report | **EXISTS** | RG-08. `report.js::sundayReport`, derived and never stored, and it names what it does **not** measure. Every field can come back `null`, and `null` renders as "—", never as 0 (DECISIONS §44) | Nothing. There is deliberately no "crash-free" line: crashes are recorded per launch, not per service, and there is no honest way to attribute one |
| 56 | Diagnostic bundle | **EXISTS** | RG-12. `diagnostics.rs` — one file a church can send, composed as an **allow-list**, with the home directory scrubbed (DECISIONS §48). `diagnostics.test.js` | Nothing. `telemetry.rs` learned the blocklist lesson expensively and this is the correction |
| 57 | Privacy centre | **EXISTS** | RG-17. Settings → Privacy, read from the live settings, stating the LAN exposure in the same size type as the reassuring half (`privacy.test.js`) | Nothing |
| 58 | AI transparency centre | **EXISTS** | `docs/AI_DISCLOSURE.md`, the per-detection inspector (§14) — and, as of this pass, the honest half **in the app and offline**: Help's *"What the AI is bad at"* carries the never-generates-text rule, the African-language weakness and the unmeasured word error rate. `aidisclosure.test.js` (RG-59) fails if either document loses a claim the other keeps | Nothing. A link to a docs file would have been useless: the operator who needs this is offline in a hall |
| 64 | Security threat model T1–T10 | **EXISTS** | `SECURITY.md` now carries the T1–T10 table: content leaving the device, LAN screen control (accepted), the closed drive-by, the broadcast-only hub, path traversal, template injection, the update channel, model integrity, language packs (no surface yet, on purpose) and the malicious operator (out of scope) | Nothing. Two rows are honest absences rather than mitigations, and say so |

### Environment, language and operators

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 29 | Room / environment profiles | **EXISTS** | RG-10. `environment_profiles` + `rooms.js` — microphone, language, service length, voice profile, displays, and the two room numbers §36 needs. Applying is a **list, not a call**, so a room applied to a machine where the projector moved reports which four of six pieces took (DECISIONS §46) | Nothing that should be added yet — see §30 |
| 30 | Automatic room calibration | **PARTIAL, deliberately** | Levels are **learned, never assumed** (DECISIONS §19, CLAUDE.md rule 12) and that is load-bearing: three individually-reasonable absolute thresholds once made Relay deaf to a quiet preacher, silently. `FirstRun` step 3 runs a live meter | **The audio floor is deliberately not seeded from a stored room.** It may well be right, and the instrument that could show it safe (`cargo test audio::gate -- --ignored`, against real room audio) has never been pointed at a real room. Stage C |
| 41 | Language Mode (primary + secondary + code-switching) | **N/A as framed** | Recognition language is a single choice — Auto / en / yo / sw / ha — because that is what the decoder accepts: whisper takes **one** language token per decode. `Auto` *is* the code-switching path, and instability is surfaced (`stt://language_unstable`) | A declared primary/secondary pair would be a setting the model cannot consume. What genuinely helps a bilingual congregation already exists and is per-preacher: `bias_terms` (§27) |
| 42 | Language Quality Centre | **EXISTS** | RG-11. Settings → Languages, every number derived from the shipped data — 66/66 books in all three, alias counts, which numerals are parsed — and **accuracy and native review render as absences, not as zeroes** (`detection::language_report_tests`, `languages.test.js`; DECISIONS §47) | Nothing. The instrument now exists and its most important column is empty on purpose |
| 43 | African-language validation programme | **BLOCKED** | ROADMAP §1: 30 minutes of real sermon audio and a native speaker. `bench/.gitignore` refuses audio into the repo by design | Not a coding task. **This is the moat and it is entirely unmeasured** |
| 44 | Signed language packs | **FUTURE — refused on purpose** | RG-19 shipped the offline half and refused this one | The word doing the work is *signed*: signing needs a key, a ceremony and a distribution channel that do not exist. An unsigned pack that can rewrite the book aliases is a wrong-verse-on-a-wall vector (SECURITY.md T9), and an operator cannot proof-read 66 names in a language they may not read |
| 39/40/48 | Operator training mode · live simulation · rehearsal replay | **EXISTS — narrower than written, deliberately** | RG-15/RG-16. Six drills with the **real** controls on the **real** surface, in rehearsal, panic first — and each knows it was done because the same event fired that would fire on a Sunday (`training.js`, `practice.js`, driven from `Help.svelte`; `training.test.js`; DECISIONS §52) | **Not a simulated service.** Relay cannot produce a sermon, and a simulation would teach a volunteer the shape of a fake. Replaying a church's *own* recorded service audio would work and needs the corpus Stage C has never produced |
| 59/60 | Church onboarding · saved environment | **EXISTS** | The 6-step first-run wizard (welcome · screen · audio · model · language · finish), re-runnable from Settings → Backup; RG-10's rooms for the saved half; the wizard's last step already **fires a real verse to the real screen**, which is the brief's "test scripture"; and RG-61 added the hand-off it was missing — the drills, the six-stage path check and rehearsal, each named with the tab it lives on (`firstrunmic.test.js`) | Nothing. **The three remaining items were deliberately NOT made into wizard steps.** This wizard's rule is that it asks as little as it can — Welcome and Finish ask nothing — and a drill, a path check and a rehearsal are things to *do* on another day, not answers to give now. The gap was never the features: all three shipped. It was that nothing told a new volunteer they existed |
| 61 | Time-to-first-verse metric | **MISSING — needs a stopwatch, not a commit** | — | It is a **human** span: from the preacher saying it to the congregation reading it. `latency.rs` measures machine stages, and the one span that claimed to cover this end to end was measuring how long the preacher had been talking (FIELD F-6, RG-31) |

### Distribution, updates and the cloud question

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 32/48 | Update preflight (signature · platform · integrity · DB compat) | **EXISTS** | RG-06. `updates::preflight` refuses to start an update onto a database that is not already healthy, and checks free disk against the snapshot it is about to take; the payload's minisign signature is verified by the updater plugin; model downloads are separately SHA-256 verified before rename. `updatesafety.test.js` | Nothing |
| 33 | Never update during a service | **EXISTS** | `updater.js::idle()` is now `!capturing && !serviceLock.engaged`, checked on both check and install — so a recorded service with the microphone momentarily stopped is protected, which is the gap this row used to name | Nothing |
| 49 | Update rollback | **EXISTS — and it rolls back the right thing** | RG-06. Snapshot (`VACUUM INTO`) before, verify on the next launch, and a restore that happens **before the database is opened**. DECISIONS §43 | **Binary rollback is deliberately not built.** The installers are public and signed; reinstalling a previous version is a five-minute job. What cannot be got back is the church's database, and that is what this protects |
| 47 | Release channels (stable/beta/nightly) | **FUTURE** | One endpoint: `releases/latest/download/latest.json` | Low value before there is a first release |
| 45 | Offline / air-gapped installation | **EXISTS** | RG-19. Install a model from a file already on the machine, a three-folder scan, and `scripts/offline-bundle.mjs` — installers + model + README on a USB stick (`offline.test.js`) | Nothing. The signed-language-pack half was refused, separately: §44 |
| 46 | Signed distribution, both platforms | **BLOCKED — on BOTH, which this row used to hide** | The chain is wired and pinned on macOS (`relay.entitlements`, `Info.plist`, `models::config_boots`, and `scripts/sign-local.sh` reproduces the hardened runtime **ad-hoc**, which is what makes rule 17 testable without a certificate), and the release gate is **per-platform** and refuses a real tag not covered on both (CLAUDE.md rule 23) | **Two certificates, not one.** `gh secret list` holds only the two `TAURI_SIGNING_PRIVATE_KEY*` **updater** keys; the gate wants six `APPLE_*` secrets and a Windows one, and finds none — so all four releases to date went out on the unsigned pre-release path. **Rule 17's trap is still ahead of this project.** ROADMAP §1 |
| 55 | Deliverability as its own discipline | **EXISTS** | `docs/RELEASING.md` covers signing, the gate, the updater and the version-in-three-files rule; RG-06 and RG-19 added the data-rollback and offline halves this row used to be missing | One **observed** end-to-end install on a machine that is not this one. That is Stage A, not a commit |
| 34–38, 50–53, 77 | Relay Cloud · church account · device registry · fleet management · multi-campus · optional backup | **N/A — declined, with reasoning** | `KNOWN_ISSUES.md` declines cloud sync, accounts, RBAC/SSO, multi-tenancy, analytics dashboards, marketplaces, billing and compliance, each with a stated reason; `PRODUCT_AUDIT.md` marks the same set NOT APPLICABLE | §20 (b) is the written-up reversal proposal if this is ever reopened. It is not adopted |
| 51 | Cloud must never become a live dependency | **EXISTS — as the product's shape** | Offline-first is enforced by construction; `probes.js` reports offline as **ok**, not as a problem | Nothing |
| 76 | Do-not-build list | **EXISTS** | `KNOWN_ISSUES.md` §3 and §5 | Nothing |

### Process, IA and measurement

| § | Requirement | Status | Evidence | What is actually missing |
|---|---|---|---|---|
| 38/49 | Sunday-mode / simplified live screen | **PARTIAL** | Live has a density toggle (Normal/Compact) and a full-screen mode | The brief's stripped layout is a *different* proposal from density. Worth prototyping against a real operator — which is a pilot question (§24), not a design one |
| 65 | Database audit | **See §13** | 21 tables + 1 FTS | Two concepts have no home, and both are declined rather than missing |
| 66 | Event architecture audit | **See §14** | Producers now exist for four of the six candidates | Two remain producerless, and both follow §19 |
| 67 | UI information architecture | **EXISTS** | 8 tabs, **18** Settings sections — Languages (§42) and Privacy (§57) are two of them — retired keys remapped rather than 404'd (`session.js`) | The brief's suggested Settings → Devices would need §19, which is declined |
| 68/69 | Live screen states · design system colour law | **EXISTS** | The four promise-carrying colours are defined once (`src/app.css`) and the six distinctions are each pinned by tests (QA_HARNESS §4.2). Cued is grey and never amber; a guess is cyan and never amethyst | Nothing. This remains one of the strongest parts of the product |
| 70/71 | Performance observability + per-stage budgets | **EXISTS** | Nine named stamps on one monotonic clock, one trace id from microphone to projector (`latency.rs`), 1 ms-bucket histograms with counted tails, and four anti-flattery properties each pinned by a test. RG-14 added **p99**, persisted it to `perf_samples`, and stopped the live Diagnostics screen printing an unreached stage as `0ms` | Nothing. A stage never reached is an ABSENCE, not a zero — in the histogram, in the table, and in the report |
| 72 | Hardware matrix | **BLOCKED** | Every figure is one M4 Pro | Windows low/mid/high, and a real church laptop |
| 73/74 | Real church field test · first ten churches | **PARTIAL — 2 / 10** | Relay ran a live sermon on 2026-08-30 (`audits/FIELD-2026-08-30.md`): Stage F11 answered, no drift across 49.5 minutes and 2,423 decodes, five of six auto-fires correct — **and one wrong verse reached a congregation** | Nine more services, and at least one run by somebody who did not write Relay. That is the largest unknown in the project |
| 75 | Feature priority model (P0/P1/P2/P3/REFUSE) | **EXISTS in substance** | ROADMAP sorts into blocked-on-world / parked / deferred / declined, and this matrix now carries **DECLINED** as a first-class verdict with the reason in the row | Adopting the brief's live-critical / operator-critical labels in `CONTRIBUTING.md` would be an improvement, not a gap |
| 62 | Success metrics | **PARTIAL** | Latency is instrumented **and persisted**, with a week-on-week trend (`report.js::weekOnWeek`). Detection is a CI build gate over a labelled corpus scored **through the real router**, failing above SPEC's 5 % wrong-verse rate, across the whole sensitivity slider. **Acceptance and override are now real** (RG-62, DECISIONS §63): the report separates a verse the operator typed from a suggestion they took, and counts the rejections — which were previously recorded nowhere at all, while the report printed `0` for them | Time-to-first-verse (§61) still needs a stopwatch. **The acceptance denominator is answers, not offers** — a suggestion that scrolls away unanswered is deliberately not recorded, because suggestions are not debounced and one paraphrase would write hundreds of rows a minute. The report says so itself. Language metrics need §43 |

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
| Queue behaviour | `sync_channel(8)`. A full queue **sheds a PARTIAL** and **blocks on a FINAL** — finals carry persistence and spoken commands and are never dropped (`main.rs`, the `relay-detect` handoff) |
| Dropped partials | Counted (`latency.rs`) and shown in Diagnostics. 0 in 1075 passes on a dev machine, 0 across a real 49.5-minute service |
| RTF | Not reported as a named metric; derivable from `stt_decode` median vs the cadence |
| Bottleneck | The model, above `base`. Below that, the batch decoder's floor of 1.0–1.6 × decode cost |

**Gaps, corrected 2026-08-31.** Two of the four this row used to name are closed: **p99 ships**
and **samples persist** to `perf_samples`, with a week-on-week trend (RG-14). What remains:

- **No STABLE state — and it is now DECLINED rather than missing** (§2 row 7). The corroboration
  rule already holds a reference from a partial window at `Suggest` until a second pass agrees,
  which is the harm STABLE was proposed to prevent.
- **No measurement with a webview in the path.** `audio_to_visible_transcript` in the rig means
  *"a consumer was handed the text"*, not *"a person could read it"*. Closing that needs a camera
  pointed at a screen, which is the same blocker as **pixels out** — §15, and it is why §61
  (time-to-first-verse) needs a stopwatch rather than a commit.
- **RTF is still not a named metric**, and deliberately: it is derivable from `stt_decode` median
  against the cadence, and the cadence *is* the decoder's own speed by construction (DECISIONS
  §38), so a reported RTF would be a restatement rather than a measurement.

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
| Content delivery | Fire-and-forget `broadcast::Sender`; `Lagged` silently skipped |
| Client → server | Three inbound kinds, none of which can carry content: `hello` (registers, answered with the template and themes), `beat` (RG-02 — "the screen for channel N painted", anonymous) and `rendered` (a latency mark, documented as inert). Everything else ignored |
| Liveness | **Real, since RG-02.** A per-channel beat with a grace window derived from the interval rather than written beside it; a lost beat degrades to "silent", which is the safe direction |
| Identity | Still none, by design. The hub records *when* a screen last painted and never *who* connected or from where (DECISIONS §35, narrowed by §39) |
| `output_channels.status` | Still a dead column — seeded `'offline'`, never written at runtime. Health is derived live, deliberately |
| Where health is shown | Outputs tab (polled), **and Live** — both through `outputHealth.js::describeScreen`, so they cannot disagree |
| Pre-air validation | `pipeline::preflight` at `broadcast_with_clock`, the one caller of `broadcast_content` (RG-05, DECISIONS §42) |
| Honesty | Unchanged and still right: *"A screen reading LIVE means something is attached, not that the picture is good."* |

**RG-01 and RG-02 were the report's most actionable findings and both are closed.** The failure
this section used to describe — an operator watching Live during a service, shown a badge that
could not detect the failure it appeared to be reporting — cannot happen now, and the test that
proves it (`outputhealth.test.js`) was re-run with the old global-state derivation restored and
fails.

## 7. Security audit

The LAN posture is deliberate, recorded (DECISIONS §35), and correctly described in `SECURITY.md`
and `PRIVACY.md`. `:8031` is broadcast-only; `:8032` is an unauthenticated control plane; mutating
routes require `POST` and are denied the CORS wildcard, which closed the bystander-browser vector
without pretending to be authentication.

Template injection is treated as untrusted input reaching the wall and is pinned by five tests,
plus an allow-list test that fails if `{@html}` appears in any renderer that reaches a screen.
The kiosk CSP and `X-Content-Type-Options: nosniff` are set; path traversal is rejected; `/media/`
takes only leading digits as an id.

**`SECURITY.md` now carries the T1–T10 threat model** the brief's §64 asked for — including the
two rows that are honest absences rather than mitigations. **No device identity (§19) and no
security event log (§22) are consequences of §35, and §2 now records both as DECLINED rather than
missing**: a log of events attributed to nobody is a log of nothing, and building the identity
first is the reversal §20 (a) proposes and this report does not adopt.

## 8. Privacy audit

Still the strongest-documented part of the product, and no longer only documented. Crash
reporting is off by default, has no DSN in OSS builds, is scrubbed of transcript, verse, lyric,
announcement, service and plan text, and is compiled out of release unless configured.
`session.js` persists position and never content. `crash.js` renders technical detail with
`textContent`, never `innerHTML`. Offline is reported as a **normal** state, not a fault.

**Both gaps are closed.** RG-17 added Settings → Privacy — one screen answering "what is on this
machine, and what can leave it", read from the live settings, stating the LAN exposure in the
same size type as the reassuring half. RG-12 added the diagnostic **export**: one file a church
can attach to an email, composed as an allow-list with the home directory scrubbed
(DECISIONS §48). Neither changed what leaves the machine; both changed whether an operator can
see that nothing does.

## 9. Reliability and recovery audit

| Mechanism | State |
|---|---|
| Crash boundary | Plain-DOM, deliberately not Svelte, because Svelte may be what broke. Says *"Your output screens are still live."* |
| Crash record | `localStorage['relay.boot.v1']`: `cleanExit`, `lastCrash`, `crashStreak`, `safeMode` |
| Safe mode | Auto-offered after 3 crashes; disarms outputs and detection; banner on the shell |
| Session resume | Operator-confirmed modal; **position only**, never on-air. Pinned by `qa-r5-onair.test.js` |
| Leave guard | `markCleanExit()` always; `preventDefault` only while the mic is live |
| Migrations | Individually retryable, with `DROP TABLE IF EXISTS` on the scratch table and rollback on failure (CLAUDE.md rule 25) — and since #41/#42 a test proves an old database really receives the columns a new one has |
| Service lock | **New (RG-03).** 16 irreversible or engine-stopping actions held back while a service records; nothing on the fire path |
| Degraded state | **New (RG-09).** Every silent fallback now says what it costs and what to do, in the shell, on every tab |
| Update safety | **New (RG-06).** Preflight, snapshot, verify-on-next-launch, restore before the database is opened |
| Latency history | **Survives a quit (RG-14)** — `perf_samples`, with p99 and a week-on-week trend |
| Rust-side session | Still not persisted. A crashed service leaves an open `services` row — cosmetic in the history list, never observed to cost anything live |

## 10. Update / distribution audit

Wired: minisign-verified payload, per-platform release gate, three-file version agreement asserted
in CI and against the tag, resumable checksummed model downloads, an in-app update banner that
never appears while capturing **or while a service is locked**, a database-compatibility preflight,
a snapshot-and-restore path for the data, and an offline USB bundle (`scripts/offline-bundle.mjs`).

Still missing: release **channels** (deferred, low value before a first release), and **one
observed end-to-end install on a machine that is not this one** — which ROADMAP §1 lists as
blocked on thirty minutes with a real machine, not on code.

Deliberately not built: **binary rollback.** The installers are public and signed; what cannot be
got back is the church's database, and that is what RG-06 protects (DECISIONS §43).

## 11. Language audit

`docs/LANGUAGES.md` is honest and should not be softened. Verified against the data files, and
the same numbers are now derived live in **Settings → Languages** (RG-11) rather than only
written down here:

| | Kiswahili | Yorùbá | Hausa |
|---|---|---|---|
| Book aliases | 68 entries | 69 | 69 |
| In-language numerals | ✅ (`numerals.json` `sw`) | ❌ **absent** | ✅ (`ha`) |
| Native-speaker review | ❌ | ❌ | ❌ |
| WER | never measured | never measured | never measured |
| UI locale file | 1 key (ships near-empty on purpose) | 1 key | 1 key |

*(Unchanged 2026-08-31. A supervised pilot with `RELAY_RECORD_WAV` set is what turns the WER row
into numbers — §24, §25.)*

The brief's §42 "Language Quality Centre" **now exists**, and its most important column is empty
on purpose: accuracy and native review render as absences, not as zeroes (DECISIONS §47). The
brief's §43 validation programme is correct and is **blocked on the world**. The moat is the
one thing on this page that no commit has moved.

## 12. UI / UX audit

48 components, 47 reachable (the one orphan is a test probe). 462 controls, none in an unrendered
component. From `node scripts/qa-inventory.mjs`:

- **0 buttons with no handler.**
- **0 controls with no accessible name.**
- **0 commands registered in Rust that no frontend caller addresses**, and 0 in the other
  direction.

**Both lists reached zero at RG-13, and eleven of the thirteen findings that stood here were the
instrument's own bugs** — `type=submit` inside a form counted as handlerless, a label supplied by
`aria-labelledby` counted as unnamed. Fixing the instrument was the larger half of that work, and
it is why the numbers above can be trusted now: a scanner that cries wolf is a scanner whose
zero means nothing (DECISIONS §49).

Colour law, empty/loading/error separation, error humanisation and the six live distinctions are
all pinned by tests and are in good shape.

## 13. Database audit

**21 tables + 1 FTS virtual table.** Present and adequate for: services, transcripts, detections,
cues, plans, songs, sections, arrangements, saved scripture, announcements, media, templates,
channels, settings, voice profiles, translations, verses — **and the three this section used to
list as absent**:

| Concept | Added by | Note |
|---|---|---|
| `service_events` | RG-04 | The append-only ordered record §23/§25/§26/§61 all needed. It carries nothing a preacher said, pinned from both sides |
| `perf_samples` | RG-04 / RG-14 | Latency that survives a quit; percentiles, never traces |
| `environment_profiles` | RG-10 | A room that survives a restart — **without** its audio levels (DECISIONS §46) |

**Still absent, and both declined rather than missing:**

| Concept | Why not |
|---|---|
| `devices` | Requires reversing DECISIONS §35. §20 (a) is the proposal; it is not adopted |
| `security_events` | Follows `devices`. Events attributed to nobody are not a security log |

`cues` gained two kinds on 2026-08-31 — `suggestion_accepted` and
`suggestion_dismissed` — because the operator's answer to a suggestion was recorded
**nowhere**: not in `detections` (whose only insert runs for a fire that reaches a
screen), not in `cues`, not in `service_events`. DECISIONS §63.

`perf_samples` is written by the backend and reached by no command, which
`scripts/qa-inventory.mjs` correctly reports as **BACKEND ONLY** — that is right for a table the
engine samples into and the report reads through `service_events`, and it is recorded here so the
next reader does not "fix" it.

## 14. Event architecture audit

Candidates from the brief, and whether they have a producer **now**:

| Proposed event | Producer exists? |
|---|---|
| `stt://partial` / `stt://final` | **Already covered** — `stt://transcript` carries `is_final`. Do not split it |
| `stt://stable` | No producer, and **declined** — the corroboration rule already gates what STABLE was proposed to gate (§2 row 7) |
| `output://heartbeat` | **Producer exists (RG-02)** — every output page beats, and the state reaches Live through one helper |
| `service://locked` / `unlocked` | **Producer exists (RG-03)** — the lock is engaged and lifted through `set_service_lock`, and its state is recorded in `service_events` |
| `readiness://changed` | No producer — readiness is frontend-only, deliberately (DECISIONS §61) |
| `device://connected` / `disconnected` | No producer, and **declined** — the hub counts, it does not identify (DECISIONS §35 / §39) |

**Recommendation unchanged in shape, inverted in effect:** add no event until its producer
exists. Four of the six now have one, and the two that do not are the two the product has
decided against.

**The full catalogue is larger than this table and was not being checked.** `ipc.test.js`
asserts that every emitted event has a frontend listener, and until RG-64 it built its list
from `main.rs` alone — so `models.rs`'s four events and everything `channels.rs` emits were
outside it. Two events are listened for and were missing from every document until this pass:
`channel://retemplate` (a screen's template was reassigned; the output filters by its own
channel id, which is what makes a template swap live — DECISIONS §29) and `rehearsal://changed`
(pushed, so no surface can be a poll interval behind the others). And **`model://done` is
emitted with no listener on purpose**, because `download_model` resolves only once the file is
installed and verified — allow-listed with that reason, and a test requires it to still be
both emitted and unheard, so the exception cannot outlive it.

## 15. Field-readiness audit

**Moved for the first time: 2 / 10.** Relay ran a live sermon on 2026-08-30
(`audits/FIELD-2026-08-30.md`) — Stage F11 answered, no drift across 49.5 minutes and 2,423
decodes, five of six auto-fires correct, and **one wrong verse reached a congregation**. The
signed-bundle conditions were separately reproduced on a real `.dmg` with
`scripts/sign-local.sh`.

Still at zero: **pixels out** (nothing has ever measured what a projector actually showed),
**hardware other than one M4 Pro**, **a second operator**, and **nine more services**.
`audits/QA-2026-08-14.md` §16 Stages A–F remains the script; F6/F7/F8 are regression tests and
green in CI.

**Fifty minutes in a room produced seven findings that months of reading source had not.** That
ratio is the argument for the pilot in §24, and it is also the argument against calling anything
here finished.

---

## 16. Missing functionality — consolidated

> **Rewritten 2026-08-30, re-checked 2026-08-31.** Everything the original list named is
> now built except three, and leaving the old list in place would have made this document
> say "missing" about features that shipped the same week — the exact failure §26 is about.

**Still missing, and each for a stated reason:**

| Item | Why it is still missing |
|---|---|
| **Signed language packs** | The signing infrastructure does not exist, and an *unsigned* pack is a wrong-verse-on-a-wall vector (SECURITY.md T9). RG-19 shipped the offline bundle without them, deliberately |
| **Explicit room calibration** | DECISIONS §19 / rule 12: nothing may compare a signal to a stored level. The instrument that could show a seeded floor safe (`cargo test audio::gate -- --ignored`, against real room audio) has never been pointed at a real room — Stage C |
| **Time-to-first-verse** | A HUMAN metric: from the preacher saying it to the congregation reading it. `latency.rs` measures machine stages, and the one span that claimed to cover this end to end was measuring how long the preacher had been talking (FIELD F-6, RG-31). It needs a stopwatch in a room, not a commit |

**Built since this list was written:** service replay · Sunday report · service event
timeline · output heartbeat and per-channel liveness on Live · Service Lock · update
data-rollback · DB-compat preflight · offline installer · pre-air Safe Screen validation ·
contrast validation · distance preview · output accessibility mode · room / environment
profiles · language quality centre · training drills · diagnostic bundle export · privacy
screen · persisted latency · p99 · the T1–T10 threat model · the AI's own weaknesses stated
in the app, offline.

**Three things in §2 moved to DECLINED rather than being built**, and that is a result, not
an omission: STABLE transcript state (§7), candidate prefetch (§12), and the device
identity / security event log pair (§19, §22). Each row carries the reason. A register
where nothing is ever declined is a register that has not been read.

## 17. Redundant functionality — do not build

| Brief item | Why it is redundant |
|---|---|
| §6/§8 two-speed transcription | The window is not the lever (DECISIONS §36) |
| §11 adaptive window sizing | Superseded by adaptive **cadence** (DECISIONS §38) |
| §12 candidate prefetch | Optimises a 2.6 ms stage inside a 144 ms budget (CLAUDE.md rule 31) |
| §7 splitting `stt://transcript` into two events | `is_final` already carries it |
| §7 a STABLE text state | The corroboration rule already gates the harm STABLE was proposed to gate (CLAUDE.md rule 28) |
| §5 a second readiness implementation | `boot/probes.js` is the implementation; extend it, do not fork it |
| §4 an engine-side health state | Its only possible action is refusing the wall, which §20 and §42 both forbid (DECISIONS §61) |
| §14 explainable detection | Built: `DetectionInspector.svelte` |
| §33 never update during a service | Built: `updater.js::idle()`, and it now reads the service lock as well as the microphone |
| §13 claim-type display | Built, and stricter than proposed (DECISIONS §21) |
| §69 design system | Built, and pinned by tests |
| §24 tamper-evident record | Would claim a guarantee against the one actor Relay says it does not defend against (SECURITY.md T10) |

## 18. Technical debt

No new debt is recorded here. `KNOWN_ISSUES.md` §4 owns the register.

**The one piece of debt this document is itself responsible for is counts in prose, and the
history below is kept as evidence rather than maintained.** RG-20 was filed because six
documents disagreed about how big `main.rs` was. Every correction since has been wrong within
the week:

| | RG-20 filed | 1st correction | 2nd correction | 2026-08-31 |
|---|---|---|---|---|
| `main.rs` lines | 4,024 | 4,369 | 5,723 | **5,573** |
| registered commands | 114 | 118 | 137 | **132** |
| `capture.js` lines | 1,908 | 1,941 | 2,195 | **2,156** |

**This table is deliberately not maintained past this row.** Its point is the shape, not the
values: three of the four columns went up and the last went *down*, because five dead commands
were deleted — so even the direction of drift is not guessable. A number in prose is wrong the
moment somebody commits, and re-correcting it is not a fix, it is the same bill paid again.

**The durable version is the command beside the number.** Every count in §0, in `QA_HARNESS.md`
Part 0 and in `CLAUDE.md` now carries one:

```bash
wc -l src-tauri/src/main.rs
grep -c '#\[tauri::command\]' src-tauri/src/main.rs
cd src-tauri && cargo test          # the runner's own summary line, not a grep
npx vitest run                      # likewise
node scripts/qa-inventory.mjs
```

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

## 19b. Decisions this report needs and cannot make

**Two open. Neither is a commit.**

**One, added 2026-08-30.**

> **Should Relay ship a second Bible translation, an import path for one, or neither?**
>
> `translations` is the only table an operator still cannot fill (RG-50). The plumbing
> has always existed — the table, `active_translation`, `listTranslations`,
> `setActiveTranslation` — and DECISIONS **§32.4** records that **WEB, ASV, YLT and BBE
> are public domain and addable as data today, with no licence**. The legal blocker it
> describes is real but applies to TPT, MSG, NIV and ESV.
>
> So three different things are possible and they are not the same decision:
>
> 1. **Seed a second public-domain corpus** (≈31,000 more verses per translation). No
>    licence needed. Costs bundle size and a validation pass nobody has done.
> 2. **Ship an import path** — `add_translation` plus a screen — so a church brings its
>    own. Touches the one table detection reads, so a malformed import is a
>    wrong-verse-on-a-wall vector and would need the same care as `pipeline::preflight`.
> 3. **Neither**, and say so where an operator can read it, which is roughly where
>    Settings already lands.
>
> **This report does not choose.** CLAUDE.md is explicit: if a decision is not in
> DECISIONS it has not been made, and an agent's job is to ask rather than assume — and
> the last time this report reasoned about it unprompted, it got the reason backwards
> (see RG-50).

**Two, added 2026-08-31.**

> **Should a detection carry the audio quality it was heard in?**
>
> The brief's §13 asks for voice confidence shown separately from claim type. Claim type
> is built and is stricter than proposed. The other half is not built — and the reason
> this report previously gave for that was **wrong**: it said the measurement did not
> exist. It does. `dsp::AudioQuality` carries `snr_db`, `clip_ratio`, `speech_prob` and
> a `warning` that already names *the single most important thing wrong right now*, and
> it reaches the console continuously on `audio://quality`.
>
> What is missing is the join: the quality is shown for **now**, and never attached to
> the claim it was measured beside. So an operator reading *"heard the reference · 91%"*
> cannot tell whether Relay heard it cleanly or through a clipping microphone — and the
> field service's wrong verse is exactly the case where somebody would want to know.
>
> **Why this is a decision and not a task.** Three of this product's hardest-won rules
> pull against each other here:
>
> 1. **A number that lies is worse than no number** (DECISIONS §21). `snr_db` is
>    documented as "rough". Printing *"SNR 12 dB"* beside a verse invites a volunteer to
>    read it as a probability, which is the paraphrase-percentage mistake in a new
>    costume. A *word* — "heard in a noisy signal" — carries the fact without the false
>    precision, and is what §45 does for degradation already.
> 2. **An instrument that cries wolf stops being read** (DECISIONS §49). A church PA
>    feed may sit near a warning threshold for a whole service. A caveat on every
>    suggestion is a caveat on none, and this repo already has one finding about
>    warnings an operator learns to scroll past.
> 3. **Only what was measured appears** (DECISIONS §44). The quality is sampled
>    continuously; a detection spans a window. "The quality when the window closed" and
>    "the worst quality during the window" are different claims and only one of them can
>    be called *the* audio the verse was heard in.
>
> So the shape is genuinely open: a chip only when the window carried a real warning; or
> a field on the detection that only the inspector shows; or nothing on the live surface
> and the quality recorded to the service record instead, where a post-mortem would find
> it and a Sunday operator would not be interrupted.
>
> **This report does not choose**, for the same reason as the one above — and
> specifically because option 2's failure mode is invisible until a real service in a
> real room, which is Stage C. It is written down so the next reader inherits the
> question rather than the wrong reason.

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

> **Rewritten 2026-08-31. The first bucket used to hold RG-01 … RG-19 as work to start; all
> nineteen shipped.** Leaving them listed as "can start today" is the exact failure §16 and §26
> are about, so what shipped moved to the closed register (§23) and this bucket now holds only
> what is genuinely un-started.

### BLOCKED ON CODE — can start today, no external dependency

**The bucket is nearly empty, and that is the finding.** Nothing below is on the critical path to
a pilot; §24's conditions are all about rooms, not commits. The one item that *was* on it —
finishing the onboarding wizard — shipped as RG-61, and shipped **narrower than written**: a
hand-off naming three instruments and the tab each lives on, rather than three more wizard steps.

| P | Item | Why, and why it is not urgent |
|---|---|---|
| **P2** | **Prototype the stripped Sunday layout** (§38/49) | Live has density and full-screen; the brief's layout is a different proposal. It should be judged by an operator who is not the author, which makes it a pilot question rather than a design one |
| **P3** | **Release channels** (§47) | Low value before a first release exists |
| — | ~~**Per-detection voice confidence** (§13)~~ | **Moved to §19b — it is a decision, not a task.** The measurement exists (`dsp::AudioQuality` carries `snr_db`, `clip_ratio`, `speech_prob` and a named warning); what is unresolved is what an operator should be shown, and three recorded rules pull against each other over it (§21 a number that lies, §49 an instrument that cries wolf, §44 only what was measured). **This row previously said the measurement did not exist. It was wrong** |
| **P3** | **Token-level streaming decode** (§9) | A whisper.cpp capability question, not a Relay wiring question. The cadence work already took the latency it would have bought |

> **"Nearly empty" is about PRODUCT features, and it should not be read as "nothing left to do".**
> Nine of the entries closed on 2026-08-31 came from auditing the instruments rather than the
> product — contract tests that scanned one file, a CI job on one Node version, an edit-time gate
> whose watch list had drifted behind four safety files, an agent brief describing a deleted
> component, citations pointing at a section that does not exist. **That seam is still open**, and
> it is not listed as a task here because it is not one: it is a habit. AGENTS.md carries it.

**Deliberately not in this bucket, and each recorded as DECLINED in §2 with its reason:** a STABLE
transcript state (§7), candidate prefetch (§12), device identity and a security event log
(§19–§22), a tamper-evident record (§24), and an engine-side health state (§4 · DECISIONS §61).

**Open on purpose, not un-started:** RG-32 — a context-resolved bare verse is labelled `Direct` at
a hardcoded 0.88, and by rule 10 that label is a lie. Changing it makes every in-passage
*"verse eighteen"* cost a click, and one service is not enough evidence to spend that. It wants a
second and third Sunday, which puts it in the next bucket rather than this one.

**Flagged for a human:** RG-50 — whether Relay ships a second public-domain corpus, an import
path, or neither, is a product decision `DECISIONS.md` does not contain. §19b holds it.

### BLOCKED ON REAL-WORLD VALIDATION — no commit can close these

- **Stage F11 — answered.** One full service, 49.5 minutes, 2,423 decodes, no drift
  (`audits/FIELD-2026-08-30.md`). Kept here because one service is one service.
- Stages F1–F5, F9–F10, F12–F14 — church hardware, a quiet room, a noisy room, a quiet speaker.
- Stages A–E — the projector, the ATEM, the eyes. **Pixels out remains at zero.**
- **Word error rate, any language.** Thirty minutes of real preaching on tape. This is the moat
  and it is the highest-value unrun item in the project.
- One observed end-to-end update install, on a machine that is not this one.
- **One full service run by a non-author operator.** The largest unknown in the project.
- RG-18's contrast and distance thresholds, against a real wall.
- RG-32's second and third services.

### BLOCKED ON EXTERNAL DEPENDENCY

- **Windows code-signing certificate** (~$10/month, Azure Trusted Signing) — ROADMAP §1. It
  blocks the platform most churches are on.
- **A native speaker** of Yorùbá, Kiswahili or Hausa, for the alias table and Yorùbá numerals —
  and, until one exists, §44's signed language packs stay refused.
- **A decision on the product's name**, due before the first church installs.

---

## 22. Production readiness — scored per dimension, no hiding average

Scored against **the first ten churches**, not against enterprise scale.
**Re-scored 2026-08-31 (second pass)**, after one live service and **77 closed** register entries of 81.
Each row keeps the previous score beside it so movement is visible rather than asserted.

> ### Not one score moved this pass, and that is the result
>
> **24 commits** landed between the first re-score and this one (`git log --oneline 436050c..HEAD`).
> **Nothing in the table changed**, because a score here is meant to track *evidence*, not effort — and the evidence
> that would move any row is a room, a certificate, a native speaker or a second Sunday. A
> readiness table that rises whenever somebody is busy is a morale chart.
>
> Two rows would have been *entitled* to rise on the work done, and are deliberately held:
> Documentation (its guarantees became machine-checked) and Security (its posture became
> reviewable). Both are argued in place below. Raising them would have widened the gap between
> the ten rows that read 8–9 and the two that decide anything, which is the failure this table's
> last paragraph already warns about.

> ### The caveat that changed, even though no number did
>
> **Nine of this pass's closures were defects in the instruments, not in the product** — a
> contract test that scanned one Rust file, its other half reading four frontend files out of
> nine, a CI job on a single Node version, an edit-time gate whose watch list had drifted behind
> four safety files, an agent brief describing a deleted component, four dead `DECISIONS §N`
> citations and sixteen dead `docs/…` paths.
>
> That matters *here* more than anywhere else in this document, because **several of these scores
> were partly self-reported by those instruments.** The confidence behind the high rows was
> resting, in part, on checks that were reading less than they claimed. They now read what they
> claim, and each is pinned so it cannot narrow again. **Read the 8s and 9s below as
> better-founded than they were, not as higher.**
>
> **An earlier version of this paragraph said the missed gaps "turned out to be closed or benign".
> Two hours later that was false, and it is corrected here rather than quietly dropped** — which
> is the rule the rest of this document runs on:
>
> * **RG-73** — the readiness checklist ticked *"Signed + notarized macOS build"*. **No Apple
>   certificate has ever existed**; all four releases went out unsigned. That is release-blocking,
>   it was hiding on the page a release decision is read from, and rule 17's
>   first-signed-build trap is therefore still ahead of this project.
> * **RG-74** — *"every text token at AA"* had been ticked since July and measured by nothing.
>   When finally measured it was **false**: `--v-faint` sat at 4.38:1 on `--v-surf2`, below AA,
>   with five shipped rules using that pairing.
>
> **So the honest statement is the stronger one: two unchecked claims were load-bearing and wrong,
> and both were found only by building the instrument that could disagree with them.**
>
> **The rest of §16 was then checked the same way, and the rest of it holds.** In-app model
> download really is resumable, SHA-256-verified and cancellable (`sha2::{Digest, Sha256}`,
> per-model constants, verified before the rename); crash reporting really is opt-in
> (`telemetry::tests::disabled_by_default`, and the gate requires the setting to be exactly
> `"1"`, so an absent setting is off); the LICENSE really does name a copyright holder. **Two
> wrong ticks out of twenty-three is the finding — not "the checklist was fiction", and not
> "the checklist was fine."** Neither
> moved a score — the first is counted under Distribution, which was already held down for exactly
> this, and the second was one marginal token, now fixed and measured. But "we checked and it was
> fine" would have been the comfortable sentence, and it was not true.

| Dimension | Was | Now | Why it did or did not move |
|---|---|---|---|
| Code | 9 | **9 / 10** | Held. 260 lines of dead component CSS deleted — proved inert by rebuilding and diffing the emitted bundle, not assumed — and the repository carries **zero** `TODO`/`FIXME` markers and zero unused CSS selectors. Against that: RG-62 found that two of `detections.status`'s four documented values were **structurally unwritable**, which is a correctness defect that lived a long time in a heavily-tested area. Fixed, but the ceiling is unchanged: one god-file (`wc -l src-tauri/src/main.rs`) |
| Performance | 8 | **9 / 10** | Held, and nothing was measured this pass. Stage F11 still answers it: **no drift across 49.5 minutes and 2,423 decodes** in a real room, every model on this machine measured, the bench predicting the room within 15%. Not 10 for the unchanged reason — one machine |
| Live safety | 9 | **9 / 10** | Held. RG-63 closed a real defect on the path the operator actually touches: a suggestion for a verse that does not exist rendered an identical amber Approve and failed *after* the click. That is the console's version of a badge that cannot detect its own failure. It does not move the score, because **a wrong verse reached a real congregation on 2026-08-30** and only a second service can show that class is closed |
| UX | 8 | **9 / 10** | Held, and now partly measured rather than asserted. RG-61 gave the setup walk-through the hand-off it never had; RG-63 made a dead control say why it is dead rather than merely go grey; **RG-74 found a real WCAG AA failure in shipped text** (`--v-faint` at 4.38:1 on `--v-surf2`, five rules using it) that a ticked box had covered since July — fixed, and every text token is now checked against every surface it is placed on. Still loses a point for surfaces no instrument here has ever seen, which is the same reason it does not gain one |
| Security | 7 | **8 / 10** | **Held on purpose, and this is the row most tempting to raise.** `SECURITY.md` now carries the T1–T10 threat model (RG-60), and two of its rows are honest absences rather than mitigations. But a threat model documents a posture; it does not change one. The LAN control plane is still unauthenticated by decision (§35), and that is the ceiling. The one genuine hardening was small and is counted under Privacy |
| Privacy | 9 | **9 / 10** | Held — with a near-miss recorded rather than smoothed over. RG-62's first cut passed a **raw string from the webview** into `cues.payload_json`, which `service_timeline` reads back and which is the part of the history most likely to be emailed. Every other cue writer guarantees its shape by construction; that one would have trusted it. Caught in review before it shipped, now parsed to a canonical reference or not stored at all, and pinned by a third e2e test |
| Reliability / recovery | 7 | **9 / 10** | Held, untouched this pass. Rollback, DB-compat preflight, Service Lock, degraded state, and a migration path proved from both sides against a database that predates it |
| Distribution | 5 | **6 / 10** | Held — and the reason is worse than it read. **Neither platform has a code-signing certificate**: the readiness checklist ticked "signed + notarized macOS build" and that was wrong (RG-73). All four releases went out unsigned on the pre-release path, and no update has ever been watched installing on a machine that is not this one. CI builds the Windows MSI on every push, so the *build* is not the risk — the *signature* and the *install* are, and nobody has seen either |
| **Language (the moat)** | 3 | **3 / 10** | **Unmoved, twice running, and this is the honest number.** Yorùbá numerals are still unparsed, no native speaker has reviewed the aliases, and **word error rate has never been measured in any language.** Nothing in twenty-six commits could touch it, and nothing in the next twenty-six will either |
| Observability | 7 | **9 / 10** | Held, and the 9 is better-founded than it was. RG-62 found the Sunday report printing **`0 suggested · 0 dismissed` for every service ever recorded** — a number for something nothing was recording, which is an observability defect of the worst kind: it reads as a measurement. Now it records what the operator actually did, and **names its own denominator** ("of the suggestions you answered") instead of implying the larger one |
| Documentation | 8 | **9 / 10** | **Held, and this is the other row that could have risen.** The change is real and structural: documentation stopped being *corrected by sweeps* and started being *checked by tests* — cross-references in three dimensions (`DECISIONS §N`, `RG-` ids, `docs/…` paths), the register's shape and its own summary counts, the AI disclosure against the app, and the edit-time gate's watch list. This pass alone that machinery caught a fourth dead citation a careful hand-sweep had missed. It stays at 9 because prose accuracy about *behaviour* is still unmechanised, and because a 10 next to a 3 is how a table like this starts lying |
| **Field validation** | 0 → 2 | **2 / 10** | **Unmoved.** One real service, 49.5 minutes, packaged build, instrumented — [audits/FIELD-2026-08-30.md](audits/FIELD-2026-08-30.md). Stage F11 answered; 5 of 6 auto-fires correct; **one wrong verse reached a congregation**. Still one church, one preacher, one language, one machine, and an operator who wrote the software |

**No overall score is given.** An average would hide the **3** and the **2**, which remain the only
two that decide anything. Twelve dimensions, two passes, and those two have not moved once —
because neither is a code problem, and **45 merged pull requests** (`gh pr list --state merged --jq length`) could not touch them.

> **The trap in this table.** Ten of twelve rows read 8 or 9, which is exactly the shape that makes
> somebody ship. **Do not read it that way.** The two low rows are not weak spots in an otherwise
> finished product — they are the two that say whether it works at all in a room, and the strongest
> evidence in this document is still that fifty minutes in one produced seven findings that months
> of reading source had not.
>
> **And the second pass adds a sharper version of the same warning.** Nine of its closures were
> defects in Relay's own instruments. If the checks that produce this table were reading less than
> they claimed, the honest inference is not that the product is worse than the scores say — it is
> that **a high score is a statement about what has been looked at, and the looking is younger than
> it appears.**

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
| ✅ RG-20 | **Doc drift** — 114 / 4,024 / "35 decisions" / e2e layer count wrong across six files | `CLAUDE.md` is read first by every agent and is on the wrong side | §26 | Fix, and add the reproducing command beside every count | — | P0 | S | `grep -rn '114 cmds\|4,024\|4.0k lines\|35 decisions' CLAUDE.md docs/` returns nothing — **verified clean 2026-08-30** |
| ✅ RG-21 | **No arrangement editor** — the one dead command. A song's running order (verse · chorus · verse · chorus · bridge) could not be created at all | A worship team cannot express how they actually sing a song; the Planner's picker is unreachable code and always falls to "Standard" | `qa-inventory.mjs` (`song_arrangements`: WRAPPER ONLY); `qa.rs::a_component_can_create_a_song_arrangement` | Build the editor in Library → Lyrics; it is the only missing link in a chain that already had a table, three commands, a wrapper, an expander and a picker | — | P1 | M | The inventory reports a create path, and a test asserts something RENDERS the editor — not merely that it exists |
| ✅ RG-22 | **Arrangement index drift** — the sequence is section positions, so reordering, inserting, deleting or renaming a section silently repoints it | The wrong words on a wall, on a Sunday, with nothing saying so. Latent until RG-21 shipped, which is why they shipped together | `db/songs.rs` (`sync_song_in_plans` re-expanded through stored indices unconditionally) | Record `built_shape` per arrangement and per plan cue; mark drift, refuse it into a plan, fall a drifted cue back to the song's own order. Never remap by guessing | RG-21 | P1 | M | Both Rust tests fail with the check disabled; the picker offers a stale arrangement and will not add it |
| ✅ RG-23 | **Ordinary preaching auto-fires a wrong verse.** A run the book cannot support, split, scored 0.83 — *higher* than the 0.45 given to the reading it replaced. The garble guard saw digits only, while whisper writes words on accented speech. And at dial 100 the auto-fire bar is 0.30, which IS the confidence floor, so every deliberate demotion was inert | A verse nobody said, on a wall, unattended | `detection::r4_audit` R4-01 · R4-02 · R4-03, all three reproducing | `DetectionMethod::UncertainNumber` — the numbers were inferred, repaired or did not line up. Capped at Suggest by `may_auto_fire`, at any score and any dial. **A demotion expressed as a number is a demotion a dial can erase** | — | P0 | M | The three tests pass; the detection suite does not regress; the CI scorecard holds |
| ✅ RG-24 | **FIELD F-1** — a live sermon citing **Luke 10:32-37** auto-fired **Proverbs 3:32** at 0.88. A bare "verse 32" was hung on a passage fired by hand five minutes earlier, with Luke 10 in the same sentence | The failure this product exists to prevent, observed in a real service | `docs/qa/audits/FIELD-2026-08-30.md` §3; `detections.heard_text`, verbatim | `anchor_for_bare_verses` — a reference named in the window outranks memory; memory is the fallback, not the default. **The first diagnosis blamed the parser and was wrong**: the test written against it passed with the fix reverted | — | P0 | S | `field_a_bare_verse_belongs_to_the_book_this_sentence_names`, and the fallback case still works |
| ✅ RG-25 | **FIELD F-2** — a detection born in a partial window is stored against the last FINAL transcript, so the service record shows a verse beside a sentence that did not produce it | History and replay misreport what Relay heard; any accuracy scored from `transcripts` scores the wrong text | 72 finals in the service contain "verse"/"chapter"/"bible" **zero** times; four `heard_text` values contain all three | `persist_fire` reuses the last final only when it really is the words the detector read, and otherwise persists the window in its own right — six extra rows in a fifty-minute service, and the join stops lying | — | P1 | M | `e2e::a_detection_points_at_the_words_that_produced_it` |
| ✅ RG-26 | **FIELD F-3** — `perf_samples` persists CUMULATIVE percentiles (`latency::report(0)`), which are structurally insensitive to drift; `worst` can only rise | Stage F11 asks whether the per-minute line rises, and the persisted series cannot answer it. The answer had to be inferred from a flat p50 under a growing denominator | §2 of the field audit | **Narrower than filed: the per-minute means already existed** in the live report (`Drift`) and were never written down. `perf_samples.last_minute_ms` persists the last COMPLETE bucket — the one still filling reads as a dip, and a dip is the shape somebody mistakes for good news | RG-04 | P1 | S | `a_perf_sample_carries_the_last_minute_not_only_the_whole_service`, and a pre-column row reports an absence not a zero |
| ~~RG-27~~ | **WITHDRAWN — FIELD F-4 was wrong.** Filed mid-service when the metric sat at 28 samples against `stt_decode`'s 1,877. It finished the service with **411**: sparse, not stopped. **The finding was drawn from a snapshot and the snapshot was not the run.** Struck through rather than deleted — a register that quietly loses its wrong entries teaches nothing | — | — | — | — | — | — | — |
| ✅ RG-28 | **The display sleeps during a service.** Nothing held the screen awake, so a projector could go black mid-sermon with Relay working perfectly and nothing anywhere saying why | A dark wall in front of a congregation, with no fault to find | Requested from the field, 2026-08-30 | `wake.rs` — an OS assertion held while the mic is live, a service is recording, or an output window is open, and released when none is true. No new dependency: IOKit on macOS, `SetThreadExecutionState` on Windows | — | P1 | S | Held and released on the real rule; merely being open never holds it |
| ✅ RG-29 | **A screen could be reported down with no way to bring it back.** Live's Output Status pane was read-only, so restoring a screen meant leaving the run surface mid-service | The pane's whole purpose is to report a screen that is down; it was a dead end | Requested from the field, 2026-08-30 | Per-screen on/off on Live, from one pure rule (`screenSwitch`) shared with the Outputs tab so badge and button cannot disagree. A browser source says where to go instead of showing a button that would do nothing | RG-01 | P1 | S | Both commands are already Service-Lock exempt; the control is not, and must never look like, a panic control |
| ✅ RG-30 | **FIELD F-5** — **filed too strongly, and corrected rather than quietly narrowed.** Per-model figures did exist (DECISIONS §36, §2 of this report). The real gap: the HEADLINE numbers everything quotes are `base`-only, and nothing converted a model choice into the update rate an operator actually gets | A church picked `turbo` with no way to know it was choosing a quarter of the cadence | §2 of the field audit | `docs/qa/audits/PERF-MODELS-2026-08-30.md` — all three models measured on one machine. **`small` is free** (152 ms, same single 200 ms hop as `base`); `turbo` is 597 ms → four hops → ~1.25 updates/s. The bench predicts the field within 15% | — | P2 | S | `RELAY_BENCH_MODEL=… cargo test --release decode_cost -- --ignored` |
| ✅ RG-31 | **FIELD F-6** — `end_to_end_speech_to_scripture` reports 39–90 s where its own constituent stages sum to ~1.1 s. Five samples; two of the service's eight fires were manual, and a manual fire has no audio origin to measure from | The one number a congregation would recognise is the one that cannot be trusted | §2 of the field audit | **Not manual fires — the clock.** It ran from `VoiceDetected`, and `voice_opened_us` is cleared only when an utterance CLOSES, so unbroken preaching pinned it: a verse quoted sixty seconds in was reported as sixty seconds of Relay latency. It now runs from `AudioReceived`, bounded by the window rather than by a sermon | — | P1 | S | `the_end_to_end_span_starts_at_this_windows_audio_not_at_the_utterance` |
| ⏳ RG-32 | A context-resolved bare verse is pushed as `Direct` at a hardcoded **0.88**. Relay inferred the book and chapter; it did not hear them | By rule 10's own principle the label is a lie, and 0.88 is a constant rather than a measurement | `main.rs`, the `detect_bare_verses` loop | Unresolved on purpose: "verse eighteen" while walking a passage is what the path is FOR, and one service is not evidence enough to make all of those ask for a click | RG-24 | P2 | ? | Needs a second and third service before it can be decided |
| ✅ RG-33 | **The confirm arm of the self-calibrating gate never fired.** `confirm_detection` re-parsed the reference STRING and taught `record_feedback` that parse's score — a constant for every canonical `Book C:V`, and always above the auto-fire bar, so the correction never ran | Relay's headline claim is that it learns your preacher. On the confirm side it never had | R4-09; `router.rs`'s own unit test passed throughout by calling `record_feedback` directly — the bug was one call site up | The console sends the suggestion's own confidence and method. Clamped; a paraphrase carries no number (a cosine is not a probability); an unknown method reads as `Semantic` | — | P1 | S | `e2e::confirming_a_suggestion_teaches_the_gate_what_was_accepted`, failing when the defect is restored |
| ✅ RG-34 | **Which of two verses in one window reached the wall was unspecified.** A `HashMap` threw away the order the preacher spoke in — and beneath it the sort comparator asked `pipeline::better` both ways, which is `>=`, so on a tie it claimed `a < b` AND `b < a` | A window may fire at most one verse (DECISIONS §37), so the tie IS what the congregation sees, and two runs of one sentence could differ | R4-07 | `Vec` dedup keeps first-seen order; the comparator orders explicitly, ties `Equal`. **Fixing the HashMap alone would have left it unspecified** | — | P1 | S | `main::rank_for_wall_tests::a_tie_keeps_the_order_the_preacher_spoke_in`, asserted in both directions |
| ✅ RG-35 | **The two Settings sliders left the voice profile in a state the router was never in.** `set_thresholds` moved the gate and neither the baseline nor the profile row; its twin `set_sensitivity` did all three | Calibration dragged the bar back to the dial position the operator had just overruled — inside the same service, and again at the next launch | R4-10 | One `apply_thresholds`, used by both. **The rule lives in the doorway both use** rather than copied into one of them | — | P1 | S | `r4_10_…`, asserting consistency to one dial step and that the BASELINE moved |
| ✅ RG-36 | **The rehearsal colour had a second meaning.** `VerseDeck` painted an EDITED slide amethyst — six lines from where the same badge means REHEARSAL. In a rehearsal the deck showed two identical amethyst pills meaning two unrelated things, one of them the signal that says nothing can reach the congregation | A colour that means two things means neither, and this one is a safety signal (DECISIONS §22) | `surface.test.js` R3-08, written as a FINDING | "Edited" is a fact about a slide, not a state of the wall — grey, with Sending and Queued | — | P2 | S | The test is inverted: it now fails if the colour is spent again, and asserts never-amber too |
| ✅ RG-37 | **Five `aria-modal` dialogs with no focus trap** — the four boot gates and the template preview. Tab walked out of them into the app behind, and focus was never restored | A boot gate is the FIRST thing a keyboard operator meets, and it opens onto an app that is not ready yet | `surface.test.js` R3-09 / R3-12 | `use:trapFocus` — `focus.js` was already the one implementation and one attribute to opt in. It deliberately does not bind Escape; that stays in `shortcuts.js`, because two opinions about a panic key is how a wall gets wiped | — | P2 | S | Both tests inverted; the mounted one asserts focus is INSIDE the dialog |
| ✅ RG-38 | **Two of VerseDeck's four fire controls were disabled in safe mode without saying why** — and the list row's label still promised "Put John 3:16 on the screens", which it could not do | Disabled and silent reads as a bug; disabled and still promising reads as a broken promise | `surface.test.js` R3-11 | One sentence across all four doors, so an operator learns it once — and it appears ONLY when true, because a control that always mentions safe mode teaches them to stop reading it | — | P2 | S | Inverted, plus a new test for the safe-mode-OFF case so it cannot pass by always explaining |
| ✅ RG-39 | **Six views a screen reader lands on had no heading at all** — the whole Library tab, Themes, Templates | A screen-reader operator navigates by heading. With none there is nothing to jump to and no way to tell where you have landed | `surface.test.js` R3-12, written as a FINDING | A heading in each; Scripture's existing bold title became a real `<h2>` so the visible text and the accessible name are the SAME string and cannot drift | — | P2 | S | Inverted — they now fail if a heading is removed |
| ✅ RG-40 | **The RUN surface never said the engine was missing.** 18 controls on Live disable on `!$capture.available`; Channels, ServicePlanner and History each explain it, and Live rendered nothing | The one tab an operator is looking at when something has gone wrong simply appeared broken | `surface.test.js` R3-12 | A rose `role="alert"` banner naming the one thing still true: the manual controls need the engine too, so there is no reassuring half-truth to offer | — | P2 | S | Inverted, and asserts the alert role — every control on the tab is dead, so it is not a quiet note |
| ⚠️ RG-41 | **Two of the original six "no heading" views are ROUTERS** — `Themes.svelte` and `Templates.svelte` are three lines that pick a child. Giving them a heading would produce two headings for one screen, so a reader jumping by heading lands twice on the same view | — | The finding as filed was wrong for those two | The requirement belongs to the children, and both children now carry one. **Recorded rather than silently dropped from the list** | RG-39 | — | — | Asserted explicitly: the routers must have NO heading, and must really be just the switch |
| ✅ RG-42 | **The Templates tab told a new operator "No templates yet" before it had asked.** A fresh install ships five built-ins, so an empty list before the first answer is not an empty list — it is a list nobody has read | It is the one sentence that makes somebody go and build five more, and it was the first thing they saw | `surface.test.js` R3-04 | An `asked` flag; `Loading` until the read returns. `EmptyState`'s own doc already said it — *if you do not KNOW the list is empty, you are Loading* | — | P2 | S | Inverted: it now fails if Empty appears before the answer |
| ✅ RG-43 | **The empty state told a volunteer what to do and made them find the control.** `EmptyState` styles a button in its slot — it was built expecting an action — and **not one of ~15 call sites passed one** | A minute lost in a dark booth, at the one moment a new operator has no idea where anything is | `surface.test.js` R3-05 | The Templates one now offers **New template**. The FILTER case deliberately gets no button: the templates exist, the filter is the problem. **The other call sites still pass no action, and the test says so** — this closes one door, not the class | — | P2 | S | Inverted, and asserts the button's text |
| ✅ RG-44 | **`surface.test.js` described itself as a file where every test asserts a DEFECT** — so a green run meant "every defect is still here". After this session most are closed | A future agent reads that header first and mistrusts every assertion in the file, or trusts the wrong ones | The header itself, plus one `FINDING` label left behind by a fix in August | Header rewritten to say what it is now: closed findings are INVERTED, never deleted, and a `CLOSED` header means the assertion guards the repair | — | P3 | S | 9 `CLOSED` headers; no stale `FINDING` labels remain |
| ✅ RG-45 | **A corrupt session payload turned the install into a fresh one — permanently.** `load()`'s corrupt branch is commented *"A CORRUPT payload is NOT a fresh install"* and returned `{...EMPTY}`, whose `setupDone: false` IS the fresh-install signal and the only thing `App.svelte` reads | The six-step modal wizard opens over a console that may be mid-service — and `session.subscribe` persists immediately, so the fallback was written back over the corrupt payload before anything could look at it | `coldstart.test.js`, and `session.test.js`'s own test was NAMED for the guarantee it asserted the opposite of | A key that EXISTS is proof the app has run here; a genuinely fresh install has no key and is handled a branch earlier. `setupDone` survives, and the unreadable bytes are kept under a sidecar key | — | P2 | S | Both tests inverted, and the mis-named one in `session.test.js` corrected rather than the finding being filed twice |
| ✅ RG-46 | **Three test files described themselves as recording defects after those defects were fixed** — `surface.test.js`'s header, and two `THE FINDING` headers in `qa-r5-groups.test.js` over tests fixed in August | A reader takes the header first: a green run then appears to mean *"every defect is still here"*, and the assertions get mistrusted or trusted wrongly | The files themselves | Headers say what they are now. **Closed findings are INVERTED, never deleted** — the header is the finding title and the assertion is the guard | RG-44 | P3 | S | No `FINDING` label remains over a passing repair |
| ✅ RG-47 | **RG-28 was fixed at the call sites and two of three doors were missed** — including `auto_open_outputs`, which `App.svelte` calls on mount, so **every launch** reopened the projector windows without telling the OS to keep the display up | The exact failure `wake.rs` exists to prevent, on the most common path there is — and introduced by the commit that introduced the module | Found by applying CLAUDE.md's own rule to my own change: *enumerate every caller of the thing you fixed* | Both doors call `refresh_wake`, and a test now **enumerates them mechanically** instead of a person doing it from memory — same shape as `servicelock::every_protected_command_actually_guards_itself` | RG-28 | P1 | S | Re-run with the miss restored: the guard fails and names `auto_open_outputs` |
| ✅ RG-48 | **This report had itself drifted.** §16 listed as "missing" twenty features that shipped during this session, and §18's corrected counts — the very numbers RG-20 was filed to fix — were wrong again: `main.rs` 4,369 → **5,723**, commands 118 → **137**, `capture.js` 1,941 → **2,195** | A gap register that says "missing" about built features is worse than no register: it is the same failure as a test file whose header says every defect is still present | The document itself, measured | §16 rewritten to the three that really are missing, each with the reason, and the old list preserved below the line. §18 states the drift as the finding rather than re-correcting the numbers a third time — **the durable form is the command beside the number, not the number** | RG-20 · RG-44 · RG-46 | P2 | S | The reproducing commands are in §18; run them |
| ✅ RG-49 | **Two of the three entries in Library → New Item did nothing.** `newPasteSong` set `lyricAction` and `newSaveScripture` set `scriptureAction`, and neither was ever passed to the pane it was meant to drive — `<LyricsPane>` and `<Scripture>` declared no such prop. The third IS wired, which is what made the other two look correct at a glance | **A cold-start blocker**: "Paste / draft song" is the only create path for `songs` that does not need a FILE, so a church whose lyrics live on a website had no way to add a song at all | `qa.rs::all_three_new_item_menu_entries_do_something`, a GAP asserted as open since the cold-start audit | Wired to the work, not to a flag: pasting opens a sheet and hands the text to the **same `parse_import` review a file goes through**; "Save scripture" puts the cursor in the search box on the Saved tab, because saving happens by starring a result and a second editor would be a second create path for a table that already has a good one. The dead flags are **deleted**, not left unread | — | P1 | M | Test inverted; it asserts the assignment is gone, the review path is shared, and the sheet traps focus |
| ⚠️ RG-50 | **`translations` is the last table an operator cannot fill** — there is no `add_translation` command, so a church cannot add a Bible version even when it is free to | **The reason I first recorded was WRONG and is corrected here rather than quietly amended.** I wrote that no corpus exists to import and called it "a rights question before it is a code question". DECISIONS **§32.4** says the opposite in as many words: **WEB, ASV, YLT and BBE are public domain and addable as data today, no licence.** The legal blocker applies to TPT, MSG, NIV and ESV — I took the licensed case's reasoning and applied it to the whole table | `qa.rs::a_fresh_install_still_cannot_be_given_a_second_translation`; DECISIONS §32.4 | **Not built, and deliberately not decided by me.** Whether Relay ships a second public-domain corpus, or an import path, or neither, is a product decision and DECISIONS does not contain it. CLAUDE.md: *if the decision isn't there, it hasn't been made — ask, don't assume.* **Flagged for a human** | — | P3 | M | The test fails the day an importer ships. A wrong reason in a register parks work forever, which is why this row now carries the correction and not the tidier sentence |
| ✅ RG-51 | **Five registered commands no rendered control could reach** — `create_template`, `import_song`, `import_pro`, `list_output_windows`, `open_output_window`. `ipc.test.js` checks wrappers; `qa-inventory.mjs` traces one hop further, and that is the level they failed at | **A security reduction, not only a tidy-up**: every registered command is invokable from the webview, and `open_output_window` opened an arbitrary fullscreen window on any monitor. A command nothing calls is attack surface nobody is watching | `qa.rs::no_registered_command_is_unreachable_from_a_rendered_component`, a GAP asserted as open since the cold-start audit | **Deleted**, with their wrappers, their Service-Lock entries, the `Outputs` label counter and two db functions left dead behind them — rather than given a UI, because each was superseded by a path every control already used. Same precedent as the five deleted before them | — | P2 | M | Both tests inverted; `qa-inventory.mjs` reports **0** unreachable commands, 132 registered |
| ✅ RG-52 | **The register table itself was corrupted by my own conflict resolutions.** Nine rows had been concatenated onto the end of a neighbour's line, so the table rendered them as one cell and the entry was invisible; three were outright duplicated | A gap register whose rows silently vanish is not a register. RG-47 was unreadable for two merges and I only noticed because a rebase conflicted on it | The file, checked by ID: RG-01…RG-51 with no gaps and no repeats | Split and de-duplicated. **The trap is that the "depends on" column reads `\| RG-02 \|`** — identical to a row start — so the first repair split rows at their own dependency cells. A row start is now identified by its STATUS MARKER, never by the id alone | — | P2 | S | **`src/lib/relaygap.test.js`** — five assertions: one contiguous table, nothing glued, ids complete AND in order, full cell count, and an open entry must carry its reason. It found three more faults the moment it ran |
| ✅ RG-53 | **Two launch-screen rows could only ever say `ok`.** The kiosk row counted configured channels and printed `ws://…:8031`; the HTTP row printed `http://<ip>:8032`. **Neither asked whether anything was listening** | On the one screen whose job is answering "is this machine going to work?". These are ordinary TCP ports on a volunteer's laptop and binding can fail — and when it does, every OBS browser source and the preacher's stage page are dead while the check says fine | `probes.js`; `sysprobe::probe_integrations` probed OBS and ATEM but not Relay's own ports | `probe_own_ports` rides on the command the launch screen already calls. The kiosk row **fails** when a browser source needs the hub and only **warns** when none does — painting red at a church with one HDMI projector teaches an operator to ignore red | RG-01 | P1 | S | Six tests including "neither row can report ok without the port answering", plus a Rust test that a really-bound port reads as listening — without it the prober could return false for everything and every other assertion would still pass |
| ✅ RG-54 | **`qa_r5.rs` said two of its tests were expected to be RED.** All three findings are closed and all six tests are green | A reader takes the header first: it says a green run is wrong, so either the tests get mistrusted or the closures go unnoticed | The module header | Corrected, with the same rule as RG-46: closed findings are inverted, never deleted | RG-46 | P3 | S | No file in the suite now claims a defect that is fixed |
| ✅ RG-55 | **One fact, two verdicts.** `degraded.js` calls a CPU-only macOS build *reduced* — "roughly three times slower, the transcript will lag the preacher" — and the launch screen's GPU row reported the identical fact as **`ok`, in green**, on the screen an operator reads before a service | CLAUDE.md rule 27 measured it: **~1710 ms per window against a ~1000 ms budget**, which is slower than real time. A green row over that is the launch screen's own version of a badge that cannot be wrong | `probes.js::gpu` vs `degraded.js` | The rule is exported once (`gpuIsReduced`) and **asked** by both, so they cannot drift. macOS only, deliberately: no equivalent measurement exists off it, and a warning nobody measured is one an operator learns to scroll past | RG-09 · RG-53 | P2 | S | Four tests, including one that asserts both surfaces agree by calling the same function |
| ✅ RG-56 | **Nothing compared a FRESH install's schema to an UPGRADED one.** `schema.sql` is `include_str!`d so it cannot drift from the code; the upgrade path — a sniffed `ALTER … ADD COLUMN` per late column — is separate and was unchecked | Invisible exactly where it is written: every test and developer database is fresh, so a forgotten migration works perfectly here and is missing on every machine that has run an older build. The first symptom is a church's laptop failing a query that works everywhere else | `db/mod.rs`; per-column migration tests existed, no comparison did | **Two tests, and the second closes what the first could not.** One builds an old database honestly — drop every column an `ALTER` claims to add, wind `user_version` to 0, run the real `migrate`, require them all back. The other diffs today's schema against `schema-baseline.sql` — the schema at the project's first commit, checked in — and fails if a column was added to an existing table with no migration behind it | RG-04 | P2 | M | Breaking a pragma sniff fails the first; adding a column with no `ALTER` fails the second. **I recorded this as unclosable for want of an old schema — wrongly, git had one all along** |
| ✅ RG-57 | **The CI detection gate contained only invented sentences.** 57 cases, every one written by somebody imagining how a preacher talks, guarding SPEC's <5% wrong-verse rate | The gate that decides whether detection quality regressed had never seen a real ASR transcript. The only real-world data the project has was sitting in `detections.heard_text` and in one Rust test | `eval_corpus.json`; `audits/FIELD-2026-08-30.md` | Six verbatim lines added, `source: FIELD-2026-08-30` — five references the preacher really made, and the Luke 10 line that produced a wrong verse, as a NEGATIVE. The corpus note also records what the scorer **cannot** see: it drives `detect_direct` and the router, not `ContextMemory`, so a green scorecard is not evidence about the path that actually failed | RG-24 | P2 | S | 63 cases, 100% recall, 0 wrong verses, 0 paraphrase auto-fires — `cargo test eval::tests::print_scorecard -- --nocapture` |
| ✅ RG-58 | **The wrong-verse gate ran at ONE dial position.** `eval::run` builds `Router::default()`, so SPEC's <5% was enforced at sensitivity 50 and nowhere else — while R4-03 had just proved that at dial 100 the auto-fire bar equals the confidence floor and every deliberate demotion goes inert | A church that moves the slider was running a configuration nothing had ever measured, and the one check that would have noticed was not looking there | `r4_04`, `#[ignore]`d as "a measurement" since the R4 audit | Un-ignored — it costs 20 ms and asserts six positions from 0 to 100. **And it needed teeth:** run as it was, it passed with the `UncertainNumber` repair torn out, because no corpus case fires without it. Eleven ordinary-preaching negatives added (`source: R4-audit`) — the sentences that really did put verses on walls | RG-23 · RG-57 | P1 | S | With the repair removed the gate reports **11.8% at dial 63** and fails — the band where the demotion sits. 74 cases, 0.0% at every position |
| ✅ RG-59 | **The AI disclosure never reached the operator.** `docs/AI_DISCLOSURE.md` is the honest account of what the AI does and where it is weak, and it was readable only from the repository. In the app, Help carried the reassuring half — the never-guess rule — and none of the honest half | A church that never opens GitHub was shown only the part that builds confidence. Publishing the reassuring half alone is worse than publishing neither: it is the same asymmetry as a status badge that cannot detect its own failure | `Help.svelte` TOPICS vs `docs/AI_DISCLOSURE.md`; no link, and a link would have been useless — the operator who needs this is offline in a hall | A Help topic, **What the AI is bad at**: Relay never writes scripture and reads the KJV verbatim; African-language listening is the weakest part *and* the headline claim; word error rate has never been measured in any language; keep `Esc` under your hand | RG-11 · §58 | P2 | S | `aidisclosure.test.js` — four claims, asserted in **both** documents by substance rather than wording, so the prose can improve and only deleting a claim breaks it |
| ✅ RG-60 | **No threat model in the shape the brief asked for.** `SECURITY.md` argued three priorities well and left the rest of the surface as an absence rather than as rows | An absence cannot be reviewed. Two of the ten rows are honest gaps that follow DECISIONS §35, and they were indistinguishable from oversights until they were written down as decisions | `SECURITY.md` before this pass: no T1–T10 table; RELAY_GAP §2 §64 read PARTIAL | T1–T10 added: content leaving the device · LAN screen control (ACCEPTED, §35) · the closed drive-by · the broadcast-only hub · path traversal · template injection · the update channel · model integrity · language packs (no surface yet, on purpose) · the malicious operator (out of scope) | RG-19 for T9 | P2 | S | Each row names the mechanism or the test that holds it; T9 and T10 are recorded as absences with the condition that would change them |
| ✅ RG-61 | **A volunteer finishing the first-run wizard had no way to learn that the practice drills, the six-stage path check and rehearsal exist.** All three shipped; nothing pointed at any of them | The wizard is the last moment somebody is guaranteed to be looking. An instrument nobody can find is an instrument nobody runs — and the path check in particular is the one thing that catches a chain that fails end to end while every part passes | `FirstRun.svelte` STEPS; brief §59/60, carried as PARTIAL in §2 | A hand-off block on the finish step, under the verse it just fired: each instrument named **with the tab it lives on**, plus a line saying this is not the operator's last chance to find them. **Not three more steps** — the wizard's own rule is that it asks as little as it can, and a drill is a thing to do on another day | RG-15 · RG-16 · RG-10 | P2 | S | `firstrunmic.test.js` — four tests: all three named with their location, the path-check sentence that stops it reading as a duplicate of the wizard, the not-your-last-chance line, and **that the step count is still six** |
| ✅ RG-62 | **Two of `detections.status`'s four values had never been written, and the Sunday report counted them anyway.** The only production insert is inside `persist_fire`, which runs for a fire that reaches a screen — so a real service can write `'auto'` or `'manual'` and nothing else. `'suggested'` and `'dismissed'` were structurally unreachable | The report printed **`0 suggested · 0 dismissed`** for every service ever recorded. Zero is a claim, and it reads as *"Relay never offered you anything"* — the exact inversion DECISIONS §44 forbids, broken two fields away from where it was written. It hid because the report's own tests fed it synthetic rows the product cannot produce: **a test whose fixture is impossible is not a test** | `persist_fire` call sites (`main.rs`), `report.js::sundayReport`, `report.test.js`'s `det(…, 'suggested', …)` | Record **what the operator did**, not what the AI offered: `confirm_detection` writes a `suggestion_accepted` cue, `dismiss_detection` a `suggestion_dismissed` cue, neither during a rehearsal. Uptake is of the suggestions **answered**, and the report names that limit itself. **Persisting suggestions was rejected** — they are not debounced (rule 28), so one paraphrase writes hundreds of rows a minute | RG-04 · RG-08 | P1 | M | Three `e2e` tests, each re-run with its defect reintroduced and each failing; plus a `report.test.js` case that feeds the OLD impossible rows and requires `null`, so the previous shape cannot come back by accident. **The third is a privacy test on this fix's own new door:** `dismiss_detection` takes a reference as a STRING across the bridge and `cues.payload_json` is read back by `service_timeline`, so it would have been the first cue payload whose shape was trusted rather than guaranteed — it is parsed and stored canonical, or not stored at all |
| ✅ RG-63 | **A suggestion whose verse does not exist looked exactly like one that does.** `emit_detections` deliberately keeps a reference that parsed cleanly but resolves to nothing ("Psalms 23:99" out of garbled speech) and marks it `in_library: false` — **and no frontend file read the flag** | The card rendered with the same amber Approve beside it, and the click failed *afterwards* with a backend error. A control that looks identical to its working neighbours and cannot work is the RG-01 defect class on the console instead of the wall. `main.rs` carried a comment naming this exact case, unfixed | `pipeline.rs` sets it; `grep in_library src/` returned only the unrelated `not_in_library` nav result | `detect.js::inLibrary`, read by Live's primary card, its "also pending" rows and `LiveOutputRail`. The suggestion **stays visible** — it is the operator's evidence that a number was misheard, and dropping it would be silence — but the Approve is disabled and **says why**, and `acceptTop`/`accept` refuse it so the `A` key cannot walk past the warning the markup renders | RG-13 | P2 | S | `detect.test.js` (3) — including that an ABSENT flag means present, so the warning can only ever be added on evidence; `liveoutputrail.test.js` (4), mounted, incl. that the handler refuses even when called directly |
| ✅ RG-64 | **The event contract test scanned one file.** `ipc.test.js` asserts that every event the backend emits is listened for on the frontend — and built its list from `main.rs` alone, so the four `model://` events and everything `channels.rs` emits were outside a contract whose entire claim is exhaustiveness | The second version of the same mistake: this test had already been widened once, because its regex did not allow `_` and had silently excluded `stt://language_unstable` and `output://panic_failed` — **a panic path**. A scanner that reads one file cannot report on a repository, and eleven of thirteen findings in the accessibility pass were the instrument's own bugs | `const emitted = … mainRs.matchAll(…)`; `models.rs` emits four events | Scan **every** `src-tauri/src/*.rs`. `model://done` turned out to be genuinely unheard and is allow-listed **with its reason** — `download_model` resolves only on success, so a listener would double-handle it — and documented at `models.rs::download`, in `ARCHITECTURE.md` and in `DATA_MODEL.md` | RG-53 · RG-13 | P2 | S | A test that the scanner sees `model://*` at all (so a re-narrowing to `main.rs` fails), a test that each allow-listed event is **still emitted and still unheard** (so the list cannot outlive its reason), and a bogus event added to `channels.rs` — which the widened scanner caught and the old one could not have |
| ✅ RG-65 | **The other half of the same contract read four files out of nine.** `ipc.test.js` asserts every command the frontend calls is registered in Rust, from a hand-written list — `capture.js`, `probes.js`, `outputHealth.js`, `updater.js` — while **nine** files import `@tauri-apps/api/core`. `App.svelte`, `Output.svelte`, `FirstRun.svelte`, `latency.js` and `Settings.svelte` were outside it | The list grew by one entry each time somebody happened to notice another direct caller, and the file's own comment says why that fails: *"a command called from a third file that this test did not read is precisely the door nobody checks."* It was written beside a list that was already missing five. **No live defect** — all six commands outside the contract do exist — and "nothing was broken" is exactly what an unchecked door looks like until it is not | `grep -rl '@tauri-apps/api/core' src/` returns nine files; the test named four | The inputs are **derived from the tree**: every `.js`/`.svelte` under `src/` that imports the Tauri core, so a tenth file joins the contract by existing. The same for the `greet`-has-one-caller test, which was scanning five named files while the whole value of `greet` is its count | RG-64 | P2 | S | A test naming four commands each called only from a previously-unread file, so a re-narrowing fails rather than passing quietly; and a bogus command planted in `Settings.svelte` — caught now, invisible before |
| ✅ RG-66 | **CI ran the frontend suite on Node 20 and nothing else, so the fix that exists for Node ≥ 22 was never exercised.** `src/test-setup.js` hands the global a real jsdom Storage because Node ≥ 22 defines `localStorage` as an own accessor returning `undefined`, and vitest's jsdom environment leaves keys the global already owns | The 60-odd tests it rescues are exactly the ones covering **what an operator sees when the app relaunches** — the session, the crash record, the first-run flag. CLAUDE.md says "don't delete it because 'jsdom provides that'", and deleting it would have left CI green while every contributor on a current Node saw a broken checkout. **A guard nothing exercises is a guard that guards nothing** | `ci.yml`'s `frontend` job: `node-version: 20`, no matrix | A Node matrix — **20** (the floor the Rust job also uses), **22** (where the break was) and **24** (a current runtime, so the fix is proved forward and not only backward), with a comment saying why it must not be trimmed back to one | RG-64 · RG-65 | P2 | S | Measured, not assumed: disabling the shim on this machine's Node 26 fails **66 tests in 6 files** and passes again when restored. On Node 20 it is a no-op, which is exactly why one runtime could not see it |
| ✅ RG-67 | **Four citations pointed at a DECISIONS section that does not exist.** `ARCHITECTURE.md`, `SPEC.md` (×2) and `PROMPT.md` cited "DECISIONS.md §16" for the 0.50/0.35 threshold seed — **the numbered log starts at §18**, and that decision is one of the unnumbered rows that predate the numbering | **A citation that resolves to nothing is worse than no citation**, which at least announces that it needs checking. A reader following it lands on nothing and cannot tell whether the decision is missing or the document is wrong. This repository leans on cross-references more than most — CLAUDE.md cites DECISIONS, DECISIONS cites the register, code comments cite all three — so they are load-bearing, and three sweeps corrected counts without once checking whether a section number still pointed anywhere | Resolving every `DECISIONS §N` against the actual headings | Each now names the row and says why it has no number. **And the check is a test, not a sweep** — which is what caught the fourth: a hand pass over the docs missed `PROMPT.md`, and the test found it on its first run | RG-20 | P3 | S | `crossrefs.test.js` — every `DECISIONS §N` resolves to a heading, every `RG-` id resolves to a register row, plus a guard that the scan reads a real slice of the repository (both assertions would also pass if it found nothing) |
| ✅ RG-68 | **A QA agent's own instructions described a deleted component and a fixed defect, both in the present tense.** `relay-qa-live-path.md` told the agent that `PreviewProgram.svelte` "reads exactly like the safety model … and nothing imports it" — it was deleted — and that `liveoutputrail.test.js` "carries one skipped test for a known defect … do not re-file it". That defect is closed and the file has **zero** skipped tests | This is RG-54 again, one directory across: a file in the audit apparatus asserting a defect that is fixed. It is worse in an agent brief than in a test header, because it does not merely mislead a reader — **it instructs an agent not to file something, and points it at a file that is not there** | `grep PreviewProgram .claude/`; `grep -c 'it.skip' src/lib/liveoutputrail.test.js` → 0 | Both corrected, and the paragraph **kept rather than deleted** — the near-miss it describes (fourteen green tests against a component nothing rendered) is the durable lesson, so it now ends on the rule instead of on the stale fact | RG-54 · RG-46 | P2 | S | The other five agent briefs were scanned for the same pattern and carry none. `crossrefs.test.js` covers `.claude/` for `RG-` and `DECISIONS §` citations |
| ✅ RG-69 | **The edit-time fast gate's watch list had drifted behind the code it watches.** `outputHealth.js` (the ONE rule for what Live and Outputs may say about a screen), `degraded.js` (the shell line on every tab), `updater.js` (refuses to install while a service is locked) and `servicelock.rs` all shipped with RG-01 … RG-09 with tests, and **none of them was added to the gate** | The gate's whole value is that it is quiet when things are fine — which is also how it fails. A rule whose path no longer matches, or that names a renamed test file, **does not error: it goes quiet**, and quiet is what "everything is fine" looks like. Four service-critical files gained tests and no edit-time guard | The `WATCHED` table vs the files added since it was written | Four rules added, each justified against the gate's own criterion (*a silent break measured in Sundays*) — and the drift itself is now pinned | RG-64 · RG-65 · RG-66 | P2 | S | `fastgate.test.js` — every watched path matches a real file, every named test file exists, the named service-critical files are watched, plus a guard that the rules parsed at all (an empty list would pass every other assertion). Verified by renaming one rule's path: two assertions fail |
| ✅ RG-70 | **Sixteen source comments cited `docs/relaydesign/`, a directory that is not in this repository**, plus three that cited the `Working-Agent-*` documents `QA_HARNESS.md` superseded and one that named an HTML mock never checked in | A comment pointing at a file nobody can open is **worse than no comment**: it reads as *"the reasoning is written down over there"* and sends the next person looking for something that was never there. The same class as RG-67, in the dimension that had the most instances — file paths rather than section numbers | Resolving every cited `docs/…` path against the filesystem | Six were the right file under the wrong directory name (`docs/design/`) and one had also been renamed (`relay-library-screen` → `relay-main-library-screen`) — all repointed. Four named documents that were never here (a design log ×2, a screens reference, an HTML mock): each now **says so in place**, and where it carried a rule the rule is written out instead of cited. Three `Working-Agent-*` citations now point at `QA_HARNESS.md`, which replaced them | RG-67 | P2 | S | `crossrefs.test.js` gained a third dimension — every cited `docs/…` path must exist, unless the citation **says** the file is gone (the register records its own deletions on purpose, and losing that would be worse). Verified by breaking one citation: it fails |
| ✅ RG-71 | **The contributor's front door quoted July's test counts.** `CONTRIBUTING.md`'s setup block said `cargo test # 250 tests` and `npm test # 138 tests` — the real numbers are **627** and **908** — and told a new contributor that CLAUDE.md's hard-way section is "25 numbered rules" when it is **40**. `PRODUCT_AUDIT.md` carried a "(35 today)" that was itself stale | This is the drift that costs the most per instance: it is read by somebody who has never seen the repository, at the moment they are deciding whether their checkout is broken. A count wrong by 3× reads as *"something is missing on my machine"* | `CONTRIBUTING.md` §setup, §the-rules; `PRODUCT_AUDIT.md` developer-experience row | **The numbers were removed rather than corrected** — the block now says the runner prints the count and points at `QA_HARNESS.md` §0, which is the one register. The rule count carries its `grep`. Same treatment the `main.rs` line got, for the same reason: this is the fifth time these have been wrong | RG-20 | P2 | S | No count in `CONTRIBUTING.md` now, and a repo-wide sweep for the old values returns nothing |
| ✅ RG-72 | **`PRODUCT_AUDIT.md` §10 listed four finished pieces of work as outstanding**, in the present tense, in the one section a reader opens to decide what to build next: make the live commands able to fail (`nav` has returned `error::Result<NavResult>` for weeks), add an e2e test (`e2e.rs` has 38), introduce a typed error (`error.rs`; **zero** `Result<_, String>` remain), and normalise the throw-vs-swallow contract (stated at the top of `capture.js` **and pinned** by `micstop.test.js`) | RG-54 and RG-68 again, in the most expensive place. A reader acting on §10 would have spent a day rebuilding four things that exist — and the section already had the convention for this: item 2 was struck through and marked done, and the other four simply never were | Each claim checked against the code before striking it | All four inverted with a ✅ and **their original wording kept beneath**, per RG-46: closed findings are inverted, never deleted. The section header now says all five are closed and why leaving them was costly | RG-54 · RG-68 | P2 | S | Every claim verified: `nav`'s signature, `cargo test e2e::` at 38, the `Result<_, String>` grep returning only `error::Result<String>` (a String *value*, not a String *error*), and both group tests present |
| ✅ RG-73 | **The production-readiness checklist ticked "Signed + notarized macOS build". No Apple certificate has ever existed.** `gh secret list` holds two secrets, both `TAURI_SIGNING_PRIVATE_KEY*` — the **updater** minisign keys. The release gate requires six `APPLE_*` secrets for macOS and finds none, so all four releases (`v0.1.0-1` … `-4`) went out on the **unsigned pre-release path** | **The only wrong tick in that checklist, and it was release-blocking.** §16 is the page a release decision is read from, and this made an absent certificate look closed. Worse: **CLAUDE.md rule 17 says the microphone dies on the FIRST correctly-signed build** — so the trap is still ahead of this project, and the tick implied it was behind | `gh secret list`; `release.yml`'s `MAC_SIGNED` loop; `gh release view v0.1.0-4` | Unticked, with what IS true stated in its place: the chain is wired and `scripts/sign-local.sh` reproduces the hardened runtime **ad-hoc**, which is what makes rule 17 testable without a certificate — and is not the same as having shipped a signed build. Corrected in four places that had inherited the claim: §2 row 46, §22's Distribution row, this report's header table, and `KNOWN_ISSUES.md` §1 | RG-72 | **P1** | S | `gh secret list` returns two updater keys and no `APPLE_*` or Windows certificate; the gate's own `MAC_SIGNED` check is the specification |
| ✅ RG-74 | **"Every text token at AA" was a ticked box with no instrument, and it was false.** `--v-faint` was `#8a8a8a`: **4.38:1 on `--v-surf2`**, below WCAG AA's 4.5, with five real rules putting muted text on chips and badges there — plus one rule in Settings pairing it with `--v-surf3` at **3.76:1** | `PRODUCT_AUDIT.md` §16 has claimed it as done since July. `contrastRatio` existed in `legibility.js` and was pointed at TEMPLATES — what a congregation reads — and never at the console's own palette. **The hand-written comment beside the token knew about the wrong surface**: it warned about `--v-surf3`, where nothing pairs them, and never checked `--v-surf2`, where five rules do. An analysis naming two of four surfaces reads exactly like one naming all four | `contrastRatio(parseColor('#8a8a8a'), parseColor('#262626')) = 4.38` | Token to **`#8c8c8c`** — two steps, invisible, and the difference between failing and passing on surf2 (now 4.50). The Settings "Soon" pill moved from surf3 to surf2. `--v-surf3` stays **excluded and asserted**, not assumed: clearing it needs `#9b9b9b`, which collapses muted into `--v-dim` | RG-18 | P2 | S | `tokencontrast.test.js` — every text token × every surface it is placed on, **plus** that nothing pairs muted with surf3, plus a guard that the ratio is a finite number. **That guard earned itself immediately: the first version passed hex strings, `contrastRatio` returned `NaN`, `NaN < 4.5` is false, and all four token assertions passed by checking nothing** |
| ✅ RG-75 | **Three register rows pointed their own validation at tests that no longer exist.** This repository **inverts a test's name when the defect closes** — `two_of_the_three_new_item_menu_entries_are_dead` became `all_three_new_item_menu_entries_do_something` — which is exactly right (RG-46), but the rows citing the old names were never updated | **A register row whose evidence cannot be found reads as an unproven claim**, and the reader has no way to tell *"the test was renamed"* from *"there was no test"*. Verifying RG-51 by searching for the name it cites returns nothing, on the row whose entire point is that a security reduction was proved | Resolving all **57** `module::item` citations in the repository against the Rust tree | Three repointed to their current names. **And a fourth dimension added to `crossrefs.test.js`** — module-prefixed citations must resolve, with the module set **derived from the tree** rather than an allow-list of external crates, because an allow-list goes quiet the moment it falls behind | RG-67 · RG-70 | P3 | S | Verified by restoring one old name: three assertions fail. `docs/qa/audits/` is excluded **because those documents may not be edited** — the one dangling citation in there (`detection::r4_07`, which has never existed) is recorded in that audit's own fix log instead, which is the mechanism it already uses |
| ✅ RG-76 | **The forty "learned the HARD WAY" rules were guarded by prose alone.** Their own header says *"These caused real crashes, freezes, or silent failures in front of people"* — and nothing enforced any of them. Five are mechanically checkable and were protected by nothing: a regression would land, pass CI, and be found by whoever next ran a service | These are not style preferences. Rule 1's violation is a **hard freeze of the webview with no error**; rule 3's is a **silent SIGSEGV** that stops speech recognition mid-sermon; rule 9's left packaged **Windows** unable to find the STT model; rule 24's makes the fire path undrivable by `e2e.rs`; rule 36's would let the pre-air validator be missing from the sixth call site, **which is the shape of four separate bugs already in this repository** | No test referenced any of them; `grep` says none is currently regressed | `hardrules.test.js` pins rules **1, 3, 9, 24 and 36** — and *only* those five. **A test that guessed at the other thirty-five would be worse than none**: it would fail on legitimate code, get weakened, and take the five real ones down with it. Rule 26 was already pinned by `ipc.test.js` | RG-64 · RG-69 | P2 | S | **Each rule was broken and the test confirmed to fail** — including rule 36's second half, that the single call stays *inside* `broadcast_with_clock`, where the preflight lives. Plus a guard that the file walk found a real tree, since all five would pass over an empty list. **A sixth assertion covers a trap this session nearly walked into**: `docs/` is not purely documentation — `db/mod.rs` `include_str!`s `docs/data/schema.sql` and `schema-baseline.sql`, so deleting them breaks `cargo build` while **the whole frontend suite stays green**. This session was asked to delete unneeded folders under `docs/`; it checked first, and the next person might not |
| ✅ RG-77 | **The pilot's single highest-value instruction had no test and two undocumented failure modes.** `RELAY_RECORD_WAV` is what converts *"word error rate has never been measured in any language"* into a number — `ROADMAP` §1 calls it the highest-value line on that page and §24 makes it a pilot condition. Its WAV writer is a **hand-rolled 44-byte RIFF header with zero coverage**, it **buffers the entire service in RAM** (~570 MB for 50 minutes), and it writes **once, at Stop** | A wrong header field yields a file that opens as noise — discovered *after* the service, with no second take. A force-quit or crash loses the whole recording, which is exactly the morning somebody most wanted it. **Neither was written anywhere a church would read**, and the instruction as given was "set one environment variable" | `grep RELAY_RECORD_WAV src-tauri/src` → two hits, no test; `write_wav_f32` untested; the buffer is a `Vec<f32>` flushed after the capture loop | Round-trip test through this module's own `load_f32` — the reader every bench here uses — asserting format 3 (IEEE float), mono, the capture rate, 32 bits, **the `36 + data` RIFF size field** (the classic hand-rolled bug) and sample-for-sample fidelity. The RAM cost and the **press Stop, never force-quit** requirement are now in `CLAUDE.md`, `ROADMAP` §1 and §25's pilot checklist | RG-73 | **P1** | S | Verified by breaking the header two ways — the format tag and the RIFF size — and confirming each fails |
| ✅ RG-78 | **Two lists of invariants, and each was missing what the other had.** `ARCHITECTURE.md` §8 was titled *"Invariants — the rules that keep it from breaking"* and listed **eleven** while `CLAUDE.md` carries **forty-one**. The eleven omit *only `Direct` may auto-fire*, *levels are learned never assumed*, *a panic control may never report a success it did not achieve*, *a migration must be retryable*, and *the choke point is where the check goes* — **the five that decide whether a wrong verse reaches a congregation.** Meanwhile ARCHITECTURE held one rule CLAUDE.md did not have at all | `docs/README.md` sends a new engineer to CLAUDE.md **then ARCHITECTURE**. A subset presented as the set, in the second document they read, is how somebody learns eleven rules and believes they have learned the rules | Diffed the two lists item by item | ARCHITECTURE §8 renamed to *"the ARCHITECTURAL ones"* and now **names the five it omits** and says to read CLAUDE.md first — pointing, not duplicating, which is the rule this repository already applies to counts. The orphan (`confirm()`/`alert()`/`prompt()` do not work in Tauri's webview — `confirm()` returns `false` **without showing a dialog**, so a two-step delete guarded by one deletes nothing and reports success) is promoted to **CLAUDE.md rule 41** | RG-70 · RG-76 | P2 | S | Rule 41 pinned in `hardrules.test.js`, verified by planting a real `confirm()` — and verified NOT to trip on the six existing comments that explain why it is not used, since a test that cries wolf gets weakened |
| ✅ RG-79 | **Four documents hard-code the default sensitivity, and nothing tied them to the constant.** `Thresholds::default()` calls `from_sensitivity(DEFAULT_SENSITIVITY)`, so the *"exactly ONE baseline"* invariant is unrepresentable otherwise — that part is sound. But `CLAUDE.md`, `DATA_MODEL`, `DECISIONS` and `ARCHITECTURE` write the **number** out, and quote the seed thresholds (0.50 auto-fire / 0.35 suggest) beside it | **Two baselines once existed and disagreed** — 0.50/0.35 against 0.90/0.60 — and a profile save silently snapped the live gate from one scale to the other, wiping the operator's calibration. Changing the constant without the docs puts that second number back into circulation on paper, next to the sentence that says there is only one | `DEFAULT_SENSITIVITY: u8 = 50` in `router.rs`; the claim restated in four files | A test that **reads the constant and requires the documents to agree** — drift in either direction fails, and nothing needs updating when the constant legitimately moves | RG-76 | P3 | S | Verified by changing the constant to 60: it fails. **The first version was over-broad and would have shipped a false failure** — it flagged `from_sensitivity(100)` in DECISIONS, which is a legitimate statement about the TOP of the dial. Narrowed to match the *claim* (`default() == from_sensitivity(N)`), not the call, plus an assertion that the claim has not vanished |
| ✅ RG-80 | **A reader could not tell which of the forty-one hard-way rules are enforced and which rest on discipline.** Only twelve named any instrument, and several that ARE pinned did not say so — rule 32's cadence floor and headroom, rule 10's auto-fire cap, rule 16's `Esc`-inside-a-dialog suppression, rule 25's retryable migration, rule 38's disarmed passage | The section header says these caused real crashes *"in front of people"*. Without knowing which are held by a test, every one reads as equally fragile — so a careful engineer spends attention re-verifying the enforced ones, and a hurried one assumes all of them are covered. **Both readings are wrong in the expensive direction** | Parsed the rules and looked for a named test in each | **Ten now name their instrument** (search *"Pinned by"*), each verified to exist before it was cited — no claim was made from memory. The header states plainly that the rest rest on discipline, and that **a rule with no named instrument is not a weaker rule; it is one where you are the instrument** | RG-76 · RG-78 | P3 | S | Rule 32's pin was verified by breaking BOTH halves — restoring `STEP_SAFETY = 1.5` and setting a floor finer than one chunker hop — and confirming `a_fast_decoder_steps_at_the_floor_not_at_a_fixed_second` fails on each. `crossrefs.test.js` resolves every `module::item` cited |
| ✅ RG-81 | **`heard_text` — the column `db/services.rs` calls THE EVIDENCE — was written on every fire and returned by no query.** It appears in the INSERT, in the schema, and in four documents; it was in **no SELECT** and referenced by **no frontend file**. The only way to read it was to open the SQLite file by hand | **§24 makes *"write every wrong verse into the register, verbatim from `heard_text`"* a condition of the supervised pilot** — and that condition was unimplementable by the operator it is addressed to. It is the loop that turned the field service's Luke 10 wrong verse into a regression test *and* a CI corpus case; a pilot that cannot close it loses its single most valuable output. It is also **not** the transcript: detection runs on partials and only finals persist, so nine auto-fires were once logged against a final from three minutes earlier that yields no matches at all | `grep heard_text` → INSERT and struct only; `ServiceDetection` had five fields, none of them it | Added to `service_detections` and rendered on the History screen under the reference it explains, quoted in serif like every other quotation of speech | RG-77 | **P1** | S | **One test asserts both halves, so neither can be satisfied alone**: the operator can read it, and it still never reaches the timeline — the part of the record most likely to be emailed. Verified by removing it from the SELECT (fails) and by wiring a real leak into the timeline query (fails, quoting the sentence) |
---

## 24. GO / NO-GO — the decision, made

**Decided 2026-08-31.** Earlier revisions of this section said **NO-GO** and left it there.
That was right when nothing had ever run in a room and wrong to leave standing now, because a
product that is never allowed out never gets the only evidence that would let it out. **An
indefinite NO-GO is not caution; it is a way of never being wrong.**

### The decision

> ## ⛔ NO-GO for general release · ✅ GO for a supervised pilot
>
> **Two churches. Named operators. Every service supervised by someone who can take the wall
> back by hand. For the length of one season.**

Both halves are load-bearing, and neither is a compromise between them.

### Why NOT general release

Relay is not shippable to a church that finds it and installs it alone, and one line decides
that: **on 2026-08-30 it put a verse nobody said in front of a congregation.** A preacher
cited Luke 10:32–37; the wall showed Proverbs 3:32. The cause is fixed and pinned, but the
*class* is not closed — closing it needs services, not commits.

Three more, each sufficient on its own:

- **Word error rate has never been measured, in any language.** The product's entire premise
  is that it hears well enough. That is an assertion, and the moat scores 3/10 because of it.
- **No code-signing certificate on EITHER platform** (RG-73). Most churches are on the one Relay cannot sign for, and the one it has run a service on is unsigned too — so rule 17's first-signed-build trap has never been sprung.
- **Nobody but the author has ever run a service on it.** Every claim about whether a
  volunteer can operate this under pressure is a claim about a person who wrote it.

### Why GO for a pilot, and why that is the *right* call rather than the brave one

The evidence that would move any of the four blockers **cannot be manufactured here**. Word
error rate needs real preaching on tape. Whether a volunteer can run it needs a volunteer. The
second and third services that RG-32 waits on need second and third services. Twenty-one merged
pull requests this week did not move field validation or language by a single point, and they
never could have.

Against that, the one service that did happen produced **seven findings** — a wrong verse, an
end-to-end metric measuring how long the preacher had been talking, a service record pointing
at the wrong sentence — that months of reading source had not. **That ratio is the argument.**
Fifty minutes in a room is worth more than another week of audit, and the next fifty will be too.

And the risk is bounded in a way that general release is not: with an operator watching and one
key that clears the wall, the worst outcome is a wrong verse for a few seconds and a note in the
register. Without a pilot the worst outcome is the same wrong verse, in a church nobody is
watching, discovered by nobody, fixed never.

### The conditions, and they are the deliverable

A pilot is **GO only with all of these.** Any one missing and it is NO-GO again:

| | |
|---|---|
| **An operator at the desk, every service** | Not a fallback. The panic keys are the product's honesty, and they need a hand |
| **`RELAY_RECORD_WAV` set for at least one full service** | This is how word error rate stops being an assertion. It is one environment variable and it is the single highest-value thing on this page |
| **Rehearsal before the first live use** | The path check exists for this; run it |
| **Diagnostics read after every service** | The per-minute line persists now. A rising one is the thing F11 could only answer once |
| **Every wrong verse written into `RELAY_GAP.md`** | Verbatim from `heard_text`. The Luke 10 case became a regression test and a CI corpus case; the next one should too |
| **macOS only** | Windows is unsigned. Do not hand out an unsigned build |

### What converts a pilot into GO for general release

Not a date and not a feeling. **All five:**

1. **Word error rate measured** on real sermon audio, in at least English and one Tier-1 language.
2. **A native speaker** has reviewed the Yorùbá, Kiswahili and Hausa alias tables.
3. **Six consecutive services with no wrong verse on a wall**, across at least two churches.
4. **A Windows code-signing certificate**, and one update watched installing on a machine that is not the author's.
5. **Three services run end to end by an operator who did not write Relay**, without the author in the room.

Until then this section says exactly what it says now, and **anyone quoting "GO" from it without
the word "pilot" is quoting it wrong.**

---

## 25. Checklist

**Rewritten 2026-08-31.** The old checklist tracked a report being written and P0–P3 buckets
being cleared. Both are finished. What follows is what is actually left, in the order it
matters, and **not one line of it is a commit.**

### Do next — the pilot (§24), in order

- [ ] **Pick two churches and name the operator at each.** The conditions in §24 are the
      deliverable, not the software.
- [ ] **Set `RELAY_RECORD_WAV` for one full service.** One environment variable. It converts
      *"word error rate has never been measured"* — the sentence behind the moat's 3/10 — into
      a number, and nothing else on this page does that. **End the service with Stop and check
      the file exists before closing the laptop**: it buffers in RAM (~570 MB for 50 minutes)
      and writes once, at Stop, so a force-quit loses the whole recording (RG-77).
- [ ] **Run the path check in rehearsal before the first live use.** It exists for this.
- [ ] **Read Settings → Diagnostics after every service.** The per-minute line persists now;
      a rising one is what Stage F11 could only answer once.
- [ ] **Write every wrong verse into §23, verbatim from `heard_text`.** It is on the service's
      own History screen, under the reference, since RG-81 — before that it could only be read
      by opening the database by hand. The Luke 10 case became
      a regression test *and* a CI corpus case. The next one should too.

### Then — what converts the pilot into general release

- [ ] Word error rate measured on real audio, English **and** one Tier-1 language.
- [ ] A native speaker has reviewed the Yorùbá, Kiswahili and Hausa alias tables — and Yorùbá
      numerals (`r4_05`), which are still unparsed.
- [ ] Six consecutive services, two churches, no wrong verse on a wall.
- [ ] A Windows code-signing certificate, and one update watched installing elsewhere.
- [ ] Three services run end to end by an operator who did not write Relay.

### Needs a person, not a Sunday

- [ ] **§19b — should Relay ship a second Bible translation, an import path, or neither?**
      Public-domain corpora are addable today with no licence (DECISIONS §32.4). This report
      got that reason wrong once (RG-50) and will not decide it.
- [ ] **A projector**, for RG-18's contrast and distance thresholds.
- [ ] **The product's name.** Still undecided (`docs/SPEC.md`), and §24 now permits real
      churches to install it. The old README line four documents kept quoting is gone.

### Done, and kept here so the shape of the week is legible

- [x] The report, `AGENTS.md`, and the doc corrections — 2026-08-29
- [x] **RG-01 … RG-22** — the register as originally filed, P0 through P3
- [x] **RG-23 … RG-31** — the field service's own findings, plus the three R4 detection defects
- [x] **RG-33 … RG-48** — the calibrator that never calibrated, a comparator that said `a < b`
      and `b < a`, five untrapped modals, six headingless views, and three documents that
      described defects already fixed
- [x] **RG-49 … RG-56** — two dead menu entries, five unreachable commands deleted, launch
      checks that could not fail, and the migration path proved from both sides
- [x] **Stage F11** — run 2026-08-30. No drift across 49.5 minutes and 2,423 decodes
- [x] **Six real sermon lines in the CI detection corpus** — the first real-world data it has
      ever contained

## 26. Document actions — delete · merge · update

> **Re-run 2026-08-30.** The original round's actions are all done and are kept below the
> line. This is what a second pass over every tracked document found.

### Delete: **`docs/GPT.md`** — done

770 lines restating `CLAUDE.md`, `AGENTS.md` and this file for a model with **no repository
access**. That audience was real when it was written and is not any more: `AGENTS.md` now
carries the tool-agnostic contract, and it deliberately states **no counts at all** — which is
the whole reason it does not drift. `GPT.md` carried its own copy of every count and every one
of them was stale (§18).

Three copies of one set of constraints is three things to keep true, and this was reliably the
least true of the three. The question it existed to ask has been asked; the answer is this file.

**Nothing else is deleted.** Every other tracked document was checked for inbound references:

| Looks orphaned | Why it stays |
|---|---|
| `.claude/agents/relay-qa-*.md` (6) | Loaded by NAME by the `/qa-audit` command, not by link |
| `.claude/commands/qa-audit.md` | A slash command; invoked, never referenced |
| `.github/PULL_REQUEST_TEMPLATE.md` | GitHub loads it by path |
| `PROMPT.md` | Cited by five Rust module docs and `capabilities/default.json` for its phase numbers. **Still must not be deleted** |
| `docs/qa/audits/*` (4) | Frozen evidence. An audit that edits its own history stops being evidence — closures go in its fix log, never in its findings |

### Merge: two duplicates, both mine, both fixed

| Duplicate | Resolution |
|---|---|
| **RG-44 appeared twice** in the register, and **RG-47's row was concatenated onto RG-44's line** with no newline — so the table rendered one malformed row and RG-47 was invisible | Both from a conflict I resolved badly while landing PR #35. De-duplicated by ID; the row split |
| §16 listed twenty features as missing that were **built the same week** | Rewritten to the three genuinely missing, each with its reason; the original list kept below the line |

### The first round's actions — all complete

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

### Update: the drift table — executed, and deliberately not maintained

The 2026-08-30 pass listed sixteen file-and-line corrections. **All sixteen were applied, and
every one of the values in them is already stale** — including two that named `docs/GPT.md`, a
file that no longer exists. The table is not reproduced here, because reproducing it would be the
seventeenth wrong number in a document about wrong numbers.

**What was done instead of maintaining it:**

| Change | Effect |
|---|---|
| `CLAUDE.md` stopped stating `main.rs`'s size and command count | It now says the file *"has outgrown every number ever written here"* and names `wc -l` and `grep -c`. A line that cannot drift |
| `QA_HARNESS.md` §0 became **the** register of counts | Every row carries the command that produces it, and every other document cites the table rather than restating the values |
| `AGENTS.md` states **no counts at all** | Which is why it is the only document that has never drifted |
| `relaygap.test.js` grew a sixth assertion | The register's own summary sentence — entries, closed, withdrawn, not closed — is now checked against the table it summarises. It had said 54 and 51 in consecutive sentences |
| Four documents stopped quoting a deleted README line | `README.md` has not said *"Working name — rename freely"* for some time; `RELAY_GAP`, `ROADMAP` and `PRODUCT_AUDIT` (×4) all still quoted it as current. Corrected to cite `docs/SPEC.md`, where the position actually lives |

**The rule this earns, and it is the one worth keeping:** *a count that appears in more than one
file will drift, and the copy that drifts is the one an agent reads first.* The correction is not
a better number — it is one register, one command per row, and a citation everywhere else.

**And the same rule applies to a status, not only to a count.** §2 of this document carried
thirty-one rows that were true on 2026-08-29 and false a week later, while the fix logs directly
above them said so. A stale matrix is more dangerous than a stale number, because it reads as a
decision rather than as a measurement — which is why §2 now cites a **file and a symbol** instead
of a line number, and why every row that resolved by *deciding not to build* says **DECLINED**
rather than going quiet.
