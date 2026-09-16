# Wave 3 — Timers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This repository has never checked a box in a landed plan — completion is evidenced by commits and green suites, not by the boxes.

**Goal:** Give a timer an identity and a lifetime of its own. Today the countdown is four fields riding on the one live `OutputContent`, held in a single slot, and any other fire forgets it — so a re-aim after a verse answers *"Nothing is counting down."* and there is no way back. Wave 3 replaces the slot with a registry of identified timers in two scopes: a congregation timer (`Both`) whose wire form stays exactly the four `countdown_*` fields it is today, and a programme timer (`Stage`) that publishes to the stage tablet only, survives any other content, and survives a panic control — the reversal of `DECISIONS.md` §27 the operator has taken.

**Architecture:** Five code tracks and one verification pass, each landing as its own pull request off `feat/wave3-timers`, which branches from `feat/wave5-shelf-names-seal` (waves 0, 1 and 2 plus the wave 5 design doc). **Track A is the base and lands first** — it creates `timers.rs` and the registry every other track reads. Tracks B (publication and retention), C (the §27 reversal), D (the warning threshold) and E (the eleventh content kind and the operator's surface) run in parallel off A. Track F is the browser-driven pass and runs last against everything the five landed.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome via the browser audit harness.

**Spec:** `docs/superpowers/specs/2026-09-15-timers-templates-stage-design.md` §3 (lines 533–726).

**Depends on:** Waves 0, 1 and 2, all verified landed in this tree on 2026-09-16 (`REHEARSAL_VERDICTS` at `channels.rs:3504`; `fire_media` generic at `main.rs:2979`; the dead `push_announcement` gone from both enumerations).

## Global Constraints

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127) — two `channels` tests fail on a bare 404 otherwise, because `dist/` is gitignored.
- Read test counts from the runner's own summary line, never a grep. Values live only in `docs/qa/QA_HARNESS.md` §0. The baseline measured 2026-09-16: Rust 784 passed / 16 ignored, frontend 2254 passed across 145 files, `e2e.rs` 66, 139 registered commands. Re-measure; do not restate.
- Every new test verified to FAIL when its defect is reintroduced. Test the bug, not the fix.
- **Rule 2:** never hold a `Mutex` across `emit` or `broadcast_content`. The registry is a `Mutex`; every reader clones and releases before it publishes. `CountdownState`'s doc comment states this discipline already — inherit it verbatim.
- **Rule 6:** lock order `Db` before `Session`. A new registry lock is innermost of all: take it last, release it first.
- **Rule 10 / 28 / 30 / 34:** no threshold moves in this wave. Nothing here touches detection or the router.
- **Rule 15:** a panic control may never report a success it did not achieve, and may never be made able to fail. Track C adds no question to `clear` or `black`.
- **Rule 24:** every new fire-path function is generic over `tauri::Runtime`. A concrete `AppHandle` re-welds the path and takes it out of `e2e.rs`'s reach.
- **Rule 33:** nothing new runs on the decoder thread and no queue is unbounded.
- **Rule 36:** the check goes at the choke point. `broadcast_with_clock` (`main.rs:612`) is the one caller of `channels::broadcast_content`; `note_countdown` is called from `broadcast_content`, `clear` and `black` — three doors, and all three stay.
- **Rule 38:** `main.rs:639` disarms the passage for any kind that is not `"scripture"`, using `is_some_and`, which is false for `None`. Track E adds the assertion this rule has never had.
- **Rule 43:** a retained frame decides what a screen is SHOWING. `is_screen_frame` (`channels.rs:938`) is a `contains`, not a `starts_with`, because `serde_json`'s map is a BTreeMap. Any new matcher has the same hazard, and its first version looking correct is the failure mode.
- **Rule 41 / 44:** no native `confirm()`/`alert()`/`prompt()`; any new overlay consumes `Esc` itself and never paints over `Clear screens`.
- `src/lib/errors.js` is the ONE backend-error humaniser. Never render a raw Rust `Err` string to a volunteer.
- `countdown.js::countdownRemainingMs` stays the ONLY arithmetic for how long is left, on every surface. A second reader is the bug `docs/REBRAND.md` phase 7 records as fixed once already.
- Commit message bodies and PR bodies are normal English prose.
- Never commit to `main`. Each track is a PR off `feat/wave3-timers`.

## Corrections to the design, verified against the working tree on 2026-09-16

Every structural claim in §3 was checked by reading the cited code. None was substantively wrong. The citations have drifted, one points at unrelated code, and three facts the design did not know have appeared since it was written.

1. **Line numbers moved.** Use these: `CountdownState` `channels.rs:1019`; `note_countdown` `:1031`; `kiosk_content_json` `:896`; `is_screen_frame` `:938`; `FRAME_VERDICTS` `:3389`; `rehearsing` `:881` with five call sites (`:1048, :1077, :1094, :1127, :1150`); `last_transition` `:1285`; the `hello` block `:1703–1791`. In `main.rs`: `broadcast_with_clock` `:612`; the rule-38 match `:639`; `start_countdown` `:2800`; the label payload `:2826`; `adjust_countdown` `:2877` with its refusal string at `:2883`; `cue_or_content_tpl` `:3036`; `ContentTemplates` `:3108`. In `pipeline.rs`: `is_countdown` `:247–249`, and the `Unsafe::Nothing` return it guards is at `:260`, eleven lines later, not adjacent as the spec's fused quote implies. Cite what you find if they move again.
2. **§3.6 site 8 is FALSE as written.** `src/Output.svelte:180` and `:279` are `applyTemplateUpdate` and a kiosk-reconnect `catch`. The two real kind filters are `Output.svelte:195` (`m.content_kind`, the kiosk door) and `:296` (`e.payload?.kind`, the Tauri door). Both still need the sweep; the line numbers in the spec do not point at them.
3. **Wave 0 landed, so `REHEARSAL_VERDICTS` already exists** (`channels.rs:3504`, a three-column table with a reason, completeness-checked by `every_publisher_in_this_module_has_an_explicit_rehearsal_verdict` at `:3580`). Wave 3's job there is one additive row, not a new table.
4. **`plan_items.duration_sec` is already taken** (`db/plans.rs:45`, migrated at `:71`) and means the Planner's running-time estimate, not a timer span. Do not reuse the name for anything a timer counts.
5. **Numbering:** the last DECISIONS section is §88, so the §27 reversal is **§89**. The highest register id is RG-142, so anything filed by this wave starts at **RG-143**.
6. **`servicelock.rs` needs nothing by default.** `guard()` returns `Ok(())` for any command absent from `PROTECTED`, and `start_countdown`/`adjust_countdown` are already named in `LIVE_PATH`. Track A adds the new commands to `LIVE_PATH` so the "the lock can never reach the live path" assertion keeps covering them — it does not add them to `PROTECTED`.
7. **Existing coverage to keep green, counted from the files themselves:** `e2e::r7_*` seven tests (`e2e.rs:3105, :3163, :3197, :3240, :3278, :3339, :3385`), `pipeline::a_countdown_is_not_an_empty_screen` (`:600`), `channels::the_kiosk_wire_form_carries_every_monitor_bindable_field` (`:4401`), and on the frontend `countdown.test.js` (9 describes), `countdownwiring.test.js` (6), plus countdown blocks in `layers.test.js:292, :315`, `templatestyle.test.js:228`, `fitcoverage.test.js:45`, `rendercontent.test.js:249`, `lowerthird.test.js:109`, `quicktools.test.js:350, :485, :588`. **None of these may change shape.** If one has to, that is a signal the wire form moved and the plan is being broken, not followed.

## Operator decisions carried into this wave

- **A `Stage`-scoped timer survives a panic control; a `Both`-scoped timer does not.** Recorded as DECISIONS §89, superseding §27's refusal.
- **The split is a property of the timer, never a question inside `clear` or `black`.** A panic control that has to ask which screen it is talking to can fail to answer.
- **`label` stays a field on the timer.** Wave 5 Track G removes the dock's hard-coded supply of one; this wave does not pre-empt it, and the two orders are compatible either way.
- **The `Both` timer's wire form does not change.** It stays the four `countdown_*` fields, which is what keeps `pipeline::is_countdown` (`pipeline.rs:247`) from refusing a timer as `Unsafe::Nothing`.
- **Delivery: one PR per track**, six PRs off `feat/wave3-timers`, each green on both suites.

---

# Track A — the registry

**Spec:** §3.2. **The base. Lands first; every other track branches from it.**

Today `CountdownState(Mutex<Option<OutputContent>>)` (`channels.rs:1019`) holds the one live countdown, and `note_countdown` (`:1031`) forgets it the moment the live content is anything else. That is the reported defect and it cannot be patched where it lives.

`timers.rs` is a new module with one responsibility: **hold the timers and answer questions about them.** It is DB-free and IO-free and does not know what a Tauri handle is, in the same discipline as `detection.rs` and `router.rs`, so the arithmetic and the lifetime rules are unit-testable without a window.

### Task 1: `timers.rs` — the registry, pure (P1)

**Why this exists:** Putting the map inside `channels.rs` would make every lifetime rule reachable only through a broadcast, which is how the current bug got its test coverage shape — `e2e::r7_a_cleared_countdown_cannot_be_brought_back_by_the_transport` pins the defect end to end because there was nowhere smaller to pin it.

**Files:**
- Create: `src-tauri/src/timers.rs` with a module doc comment stating the single responsibility
- Modify: `src-tauri/src/main.rs` — `mod timers;`

**Interfaces:**
- Produces:
  - `pub type TimerId = i64;`
  - `pub enum Scope { Both, Stage }` — `Both` reaches every screen through the content frame; `Stage` reaches the stage tablet only.
  - `pub struct Timer { id, label, done_msg, target_ms, from_ms, paused_ms, warn_ms, scope, plan_item_id }` exactly as §3.2 names them. `warn_ms: Option<i64>` is Track D's; carry the field now so Track D is a reader and a writer, never a migration.
  - `pub struct TimerRegistry(Mutex<HashMap<TimerId, Timer>>)` with: `start(&self, Timer) -> TimerId`, `get`, `stop(&self, TimerId) -> bool`, `stop_scope(&self, Scope) -> usize`, `adjust(&self, TimerId, remaining_ms: Option<i64>, paused: Option<bool>, now_ms: i64) -> Result<Timer, TimerError>`, `snapshot(&self) -> Vec<Timer>`, `snapshot_scope(&self, Scope) -> Vec<Timer>`.
  - `pub fn remaining_ms(t: &Timer, now_ms: i64) -> i64` — the held figure when held, otherwise `target_ms - now_ms`, clamped at 0. **The same rule as `countdown.js::countdownRemainingMs` and `adjust_countdown`'s inline `current`.** One rule, stated twice across the bridge and pinned on both sides; never three times on one side.
- Consumes: nothing. No `tauri::`, no `rusqlite::`, no clock — `now_ms` is passed in, which is what makes the tests deterministic.

- [ ] **Step 1: Write the failing tests** in `timers.rs`'s own `mod tests`, each naming the field failure it prevents:
  - `a_timer_outlives_content_that_is_not_a_timer` — start a `Both` timer, then prove the registry still answers for it after anything else has been broadcast. (The registry has no broadcast; this test asserts the registry does not itself forget, and Task 3 is where the door stops forgetting.)
  - `a_held_timer_reports_the_figure_it_was_held_at` — `paused_ms` wins over the instant, at any `now_ms`.
  - `a_re_aim_does_not_release_a_hold` — `adjust(id, Some(x), None, now)` on a held timer leaves it held with `x` left. This is the `+1`-restarts-a-held-countdown failure `adjust_countdown`'s doc comment records.
  - `a_resume_re_aims_the_instant_from_the_figure_it_was_holding` — `adjust(id, None, Some(false), now)` sets `target_ms = now + held`, so releasing a hold does not teleport the deadline.
  - `adjusting_a_timer_that_is_not_there_is_refused_rather_than_creating_one` — `TimerError::NoSuchTimer`. Start is the only creator; there must be exactly one of those.
  - `a_timer_shorter_than_a_second_is_refused` — the `next < 1000` rule `adjust_countdown:2883`-onward already keeps, moved into the pure layer with its reason.
  - `two_timers_have_two_identities` — the whole point of the wave: ids are distinct and `stop` takes one without touching the other.
  - `stopping_a_scope_takes_every_timer_in_it_and_no_other` — Track C depends on this being a property of the registry rather than a branch in a panic control.
- [ ] **Step 2: Implement** until green. Ids monotonic from 1 per process; never reuse an id inside a run — a reused id is a re-aim landing on the wrong clock.
- [ ] **Step 3: Verify each test fails when its defect is reintroduced.** Revert the hold-precedence line and watch two of them fail; if only one does, one of them is not testing what it says.
- [ ] **Step 4:** `cargo fmt --all && cargo clippy --all-targets -- -D warnings`, then the Rust suite.

### Task 2: `start_countdown` becomes a thin wrapper over a `Both` timer (P1)

**Why this exists:** The registry is only the source of truth if the shipped path writes to it. The projection direction matters: the registry owns the facts and the four `countdown_*` fields become its projection, not a second copy that can drift.

**Files:**
- Modify: `src-tauri/src/main.rs` — `start_countdown` (`:2800`), `adjust_countdown` (`:2877`)
- Modify: `src-tauri/src/channels.rs` — `CountdownState` and `note_countdown` become registry-backed (see Task 3)
- Modify: `src-tauri/src/timers.rs` — `pub fn project_both(t: &Timer) -> (i64, i64, Option<i64>, String, String)` or an explicit struct; one function, so the four fields are filled in exactly one place
- Test: `src-tauri/src/e2e.rs`

- [ ] **Step 1: Write the failing e2e test** `r7_a_countdown_survives_a_verse_and_can_still_be_re_aimed` — start a countdown, fire a scripture, then `adjust_countdown`. Today this returns `Err("Nothing is counting down.")`; after this task it re-aims, and the `Wall` sees a countdown frame again. **This is the reported defect and it is the test the wave exists to pass.**
- [ ] **Step 2:** `start_countdown` creates the timer in the registry (scope `Both`, `from_ms` = now, `label`, `done_msg`, `warn_ms: None`, `plan_item_id: None`) and broadcasts a content frame projected from it. Every existing field keeps its current value and meaning, including `template_id`/`template_json`/`template_pinned` resolution through `cue_or_content_tpl` — **do not re-resolve the template on a re-aim**, which is the silent re-skin `adjust_countdown`'s doc comment forbids.
- [ ] **Step 3:** `adjust_countdown` reads the registry rather than `live_countdown`, applies through `TimerRegistry::adjust`, and re-broadcasts the projection carrying the ORIGINAL content verbatim apart from the four fields. Its refusal string stays exactly `"Nothing is counting down."` when the registry holds no `Both` timer — an operator reads that sentence, and it is still true in the only case that can now produce it.
- [ ] **Step 4:** Keep all seven `r7_*` tests green **unchanged**. `r7_a_cleared_countdown_cannot_be_brought_back_by_the_transport` must still pass: a cleared `Both` timer is stopped, and the transport still cannot create one.
- [ ] **Step 5:** Verify the new test fails with Task 2 reverted, and that it fails for the right reason (the refusal string, not a panic).

### Task 3: the three doors stop forgetting, and `CountdownState` retires (P1)

**Why this exists:** `note_countdown` is called at three doors — `broadcast_content` (`:1042`), `clear` (`:1077`-ish) and `black`. Its filter is the lifetime bug. Removing the filter without removing the slot leaves two answers to one question.

**Files:**
- Modify: `src-tauri/src/channels.rs` — `CountdownState`, `live_countdown`, `note_countdown`
- Modify: `src-tauri/src/main.rs` — `setup` manages `TimerRegistry`
- Modify: `src-tauri/src/qa.rs` — `bare_app()` manages `TimerRegistry` too

**`qa::bare_app()` is not optional.** A fixture without the registry is an app in a state no church could be in, and a command reading it would panic rather than fail readably. `the_bare_fixture_is_a_first_launch_and_nothing_more` is the tripwire and it must keep passing.

- [ ] **Step 1: Write the failing tests** in `channels.rs`'s `mod tests`: `firing_a_verse_does_not_forget_the_congregation_timer`, and `a_clear_takes_the_congregation_timer_and_leaves_the_programme_timer` (the second one goes green in Track C; write it here as `#[ignore]` **only if** Track C is a separate PR, and Track C's first act is to un-ignore it — an ignored test with no owner is how `e2e.rs` carried two defects for months).
- [ ] **Step 2:** `broadcast_content` stops filtering. A `Both` timer is stopped by `clear`/`black` and by a new `Both` timer replacing it, and by nothing else.
- [ ] **Step 3:** `live_countdown` becomes a registry read that projects the newest `Both` timer, or is deleted in favour of a registry call at its two call sites. Prefer deletion: a compatibility shim over a new source of truth is how five hand-rolled `OutputContent` copies drifted apart.
- [ ] **Step 4:** `setup` and `qa::bare_app()` both `manage(TimerRegistry::default())`. Grep every `mock_builder` fixture in the tree for a third one; the repo's signature bug is a guarantee kept on one door and skipped on its twin.
- [ ] **Step 5:** Full Rust suite green; `cargo test e2e` green.

### Task 4: the commands, and the lock (P2)

**Files:**
- Modify: `src-tauri/src/main.rs` — new `#[tauri::command]`s, all generic over `R: tauri::Runtime`, all registered in `generate_handler!`
- Modify: `src-tauri/src/servicelock.rs` — `LIVE_PATH`
- Modify: `src/lib/stores/capture.js` — wrappers in the documented throw group
- Test: `src/lib/ipc.test.js` (contract, both directions)

**Interfaces:**
- `start_timer(app, db, minutes: f64, label: String, done_msg: String, scope: String, warn_ms: Option<i64>, plan_item_id: Option<i64>, template_id: Option<i64>) -> error::Result<i64>` — returns the id.
- `adjust_timer(app, timer_id: i64, remaining_ms: Option<i64>, paused: Option<bool>) -> error::Result<()>`
- `stop_timer(app, timer_id: i64) -> error::Result<()>`
- `list_timers(app) -> error::Result<Vec<TimerView>>` — a serialisable view, so the console can render a list without a second arithmetic.

- [ ] **Step 1:** Write `ipc.test.js`'s side first: the four commands must exist in Rust and be called from `capture.js`. `ipc.test.js` fails in both directions, which is the point.
- [ ] **Step 2:** Implement, `#[tauri::command]` each, register each, and add each to `servicelock.rs`'s `LIVE_PATH` with the reason: a timer control is a live control and the lock may never reach it.
- [ ] **Step 3:** `capture.js` wrappers go in **Group 1 (THROWS)** beside `startCountdown` and `adjustCountdown`, with the group named in each docstring. A wrapper placed in a group without the test that holds it there is the gap `micstop.test.js` exists for — add the test.
- [ ] **Step 4:** Both suites green. Re-measure the registered-command count; do not restate 139.

**PR 1 ends here.** Title: *the timer gets an identity and stops being forgotten*.

---

# Track B — publication and retention

**Spec:** §3.3. **Off Track A.** Lands as PR 2.

A `Stage` timer publishes no content frame — that is exactly why it survives a verse. It needs its own hub frame and its own retained slot.

### Task 5: the `timer` hub frame and the `last_timers` slot (P1)

**Why this exists:** A stage tablet that reloads mid-service must come back to the programme timer it was showing. `last_screen` holds one frame and the newest wins, so retaining a timer there would replace the verse and the next screen to join would be sent a clock and a blank wall. That is rule 43's trap 1 and the reason `stage_next` is excluded.

**Files:**
- Modify: `src-tauri/src/channels.rs` — `KioskHub`, `publish_timers`, `last_timers`, the `hello` reply (`:1703–1791`), `FRAME_VERDICTS` (`:3389`), `REHEARSAL_VERDICTS` (`:3504`)
- Test: `src-tauri/src/channels.rs` `mod tests`

- [ ] **Step 1: Write the failing tests:**
  - `a_stage_tablet_that_joins_mid_service_is_sent_the_programme_timers` — hello replays `last_timers`.
  - `a_timer_frame_is_never_retained_as_a_screen_frame` — `is_screen_frame(timer_json) == false`, and a published timer does not overwrite `last_screen`. **Write the json with `serde_json` and not by hand**: a BTreeMap serialises `{"kind":…}` in key order, and the first version of `is_screen_frame` matched nothing while looking exactly like the bug it fixed.
  - `a_timer_published_during_a_rehearsal_reaches_no_stage_tablet`, watching the hub itself and not `e2e::Wall` — `stage_next` was gated-in-theory and ungated-in-fact precisely because the Wall sees only Tauri events.
  - `the_hello_order_puts_the_screen_frame_last` — template, default_template, transition, **timers**, then the retained screen frame. A late stage tablet must paint the reading last and not flash a clock over it.
- [ ] **Step 2:** Implement `publish_timers` as a gated publisher: `rehearsing(app)` early-return, then `publish_kiosk`. Clone out of the registry and drop the lock before publishing (rule 2).
- [ ] **Step 3:** Add `("timer", false)` to `FRAME_VERDICTS` with the same shape of comment its neighbours carry, and `("publish_timers", true, "a rehearsal has no stage tablet waiting for a programme clock")` to `REHEARSAL_VERDICTS`. Both completeness tests must pass without being edited — if either needs editing to accept the new row, the row is in the wrong table.
- [ ] **Step 4:** Verify each test fails against the pre-fix code.

### Task 6: the stage page renders the programme timers (P1)

**Files:**
- Modify: `src/Stage.svelte` — a `timer` branch beside `stage_next` and `stage_alert` (`:406–445`)
- Modify: `src/lib/countdown.js` — nothing. **It already answers how long is left and it stays the only thing that does.**
- Test: `src/lib/` — a new `timers.test.js`, plus the existing `countdownwiring.test.js` unchanged

- [ ] **Step 1: Write the failing test:** a `{"kind":"timer", timers:[…]}` message paints one row per stage timer, each through `countdownRemainingMs`, with the existing 1000 ms tick (`Stage.svelte:521`) and no second interval.
- [ ] **Step 2:** Implement. Reuse `formatCountdown`; add no formatter.
- [ ] **Step 3:** A timer with no label renders digits alone without a collapsed or clipped box — wave 5 Track G makes label-less the dock's default and this page must already survive it.
- [ ] **Step 4:** Frontend suite green.

### Task 7: `ARCHITECTURE.md` §6 and the event register (P2)

- [ ] **Step 1:** If Track B adds a Tauri event (it should not — the stage path is a hub frame), it goes in `docs/ARCHITECTURE.md` §6 and `ipc.test.js` catches it both ways. Reproduce the set with the command CLAUDE.md gives rather than trusting any number in prose.
- [ ] **Step 2:** Record the new hub frame kind wherever `docs/ARCHITECTURE.md` enumerates them.

**PR 2 ends here.** Title: *a programme timer publishes to the stage and is replayed to a screen that joins late*.

---

# Track C — the §27 reversal

**Spec:** §3.4. **Off Track A.** Lands as PR 3.

### Task 8: scope decides what a panic control takes (P1)

**Why this exists:** The congregation guarantee must stay exactly as strong as it is today. It does, because the split is a property of the timer and the panic control asks nothing.

**Files:**
- Modify: `src-tauri/src/channels.rs` — `clear`, `black`
- Test: `src-tauri/src/e2e.rs`, `src-tauri/src/channels.rs`

- [ ] **Step 1:** Un-ignore (or write) `a_clear_takes_the_congregation_timer_and_leaves_the_programme_timer`, and add `a_blackout_answers_the_same_way_as_a_clear`. **Both controls, deliberately, in both branches** — `Stage.svelte:406` carries a comment saying in as many words that if Relay ever lets the stage survive a panic, it must survive both, not by one of them being forgotten. This is that day.
- [ ] **Step 2:** `clear` and `black` call `registry.stop_scope(Scope::Both)` and nothing else. **No branch asks which screen it is talking to.** A panic control that has to ask a question can fail to answer it.
- [ ] **Step 3:** Prove the congregation guarantee is unchanged: every existing panic test stays green, unedited, and `panic.test.js`'s fourteen stay green.
- [ ] **Step 4:** Verify the new tests fail if `stop_scope` is replaced by a full stop.

### Task 9: DECISIONS §89, and the `alert` question settled (P1)

**Files:**
- Modify: `docs/DECISIONS.md` — a new §89 that explicitly supersedes §27's refusal
- Modify: `src/Stage.svelte` — the `alert` branch, with its answer written down
- Test: `src/lib/crossrefs.test.js` resolves the new citation

- [ ] **Step 1:** Write §89 with the three reasons §3.4 gives: §27 left the question open and named it a real product decision; §27's "no exceptions" was already not literally true in shipped code (`Stage.svelte`'s `svcStart` survives, on a stated ground); and the congregation guarantee is untouched. Cite §27 by number so `crossrefs.test.js` can resolve it.
- [ ] **Step 2: Settle `alert`.** The clear/black branch resets `visible, note, cdTo, cdFrom, cdPaused, next` and does **not** reset `alert`, and nothing in the tree records why. Decide it in §89 and make the code say so either way — the current state is a silent third answer to the question §89 exists to answer. Whichever way it goes, pin it with a test named after the decision.
- [ ] **Step 3:** `crossrefs.test.js` green: a citation that resolves to nothing is worse than an uncited claim.

### Task 10: the registers stop disagreeing (P2)

- [ ] **Step 1:** `docs/REBRAND.md` phase 7 says *"The button landed in wave 3"* about the rebrand's own wave 3, which is a different numbering from this spec's. Add the distinction where it is ambiguous, or leave it and say why. **Do not restate the wave 3 scope in a fifth place** — `RELAY_V1_AUDIT.md`, `RELAY_GAP.md`, `QA_HARNESS.md` §0 and the audit files are the four that already exist.
- [ ] **Step 2:** File anything this track found but did not fix as RG-143 onward, in `docs/qa/RELAY_GAP.md`, and keep `relaygap.test.js` green.

**PR 3 ends here.** Title: *a stage timer survives a panic control, and the congregation guarantee does not move*.

---

# Track D — the warning threshold becomes a setting

**Spec:** §3.5. **Off Track A** (it reads `Timer::warn_ms`, which Track A carries). Lands as PR 4.

`COUNTDOWN_WARN_MS = 60_000` is at `src/lib/layers.js:323` and `countdownWarning` at `:335–343`; the comment at `:332–333` says the rule is deliberately not a setting because *"the control belongs in the Settings pass, and a setting with nowhere to set it is worse than a sensible default."* This is the Settings pass.

### Task 11: `countdownWarning` takes an override (P1)

**Files:**
- Modify: `src/lib/layers.js`
- Test: `src/lib/layers.test.js:315`

- [ ] **Step 1: Write the failing tests:** an explicit `warnMs` wins; `null`/`undefined` falls back to today's rule exactly (the last minute, or the last tenth of a countdown shorter than ten minutes); and **`countdownTotalMs` returning null still yields no warning colour rather than a guessed span** (`countdown.js:105`, whose body already refuses a guess — keep it). A colour on at the wrong moment is worse than one on a minute early.
- [ ] **Step 2:** Implement as a third argument with a default, so every existing call site keeps working unedited.
- [ ] **Step 3:** Update the `:332` comment to say the setting now exists and where. A comment that describes a decision the code has since reversed is the failure `docs/` has been corrected for four times this month.

### Task 12: the default in Settings, the override per cue (P1)

**Files:**
- Modify: `src/lib/views/Settings.svelte` — a warning-threshold default under the countdown/timer grouping
- Modify: `src/lib/views/ServicePlanner.svelte` — `addCountdownCue` (`:287`, payload at `:289`) gains the per-cue field
- Modify: `src/lib/stores/capture.js` — carry `warn_ms` through `startTimer`/`startCountdown`
- Test: `src/lib/settingssections.test.js`, `src/lib/plan.test.js` or the planner's own suite

- [ ] **Step 1: Write the failing tests:** the Settings control **saves a preference something reads** — seven controls that saved a preference nothing read were closed on 2026-09-10 and that is the defect shape to avoid; and a cue carrying `warn_ms` fires a timer that warns at that figure.
- [ ] **Step 2:** Implement. The Settings default is the fallback when a timer carries no `warn_ms`; the per-cue value wins. Two authorities, ranked in one place, and the ranking reported — the same shape as `resolveTransition`.
- [ ] **Step 3:** No native `confirm`/`prompt` anywhere near it (rule 41).
- [ ] **Step 4:** Frontend suite green; `qa-inventory` reports no handlerless control.

### Task 13: the flash itself (P2)

- [ ] **Step 1:** Nothing to build — `cdwarn` keyframes exist on all three surfaces with a `prefers-reduced-motion` glow fallback each (`TemplateRender.svelte:1790–1796`, `Stage.svelte:842–843, :917`, `Dock.svelte:1531–1534`). **Assert that, do not rebuild it.** Add a test only if none pins the reduced-motion fallback.

**PR 4 ends here.** Title: *the warning threshold becomes a setting, per timer and per cue*.

---

# Track E — the eleventh site sweep, and the operator's surface

**Spec:** §3.6. **Off Track A.** Lands as PR 5.

Eleven sites consume the kind string. Missing one produces this repository's signature bug shape. The list, with the corrected citations:

| # | Site | Now at | If skipped |
|---|---|---|---|
| 1 | rule 38 match | `main.rs:639` | a kind that is not literally `"scripture"` disarms the passage; a timer is disarmed for free |
| 2 | `is_countdown` | `pipeline.rs:247–249`, guarding `:260` | **the pre-air validator refuses the frame as `Unsafe::Nothing`** |
| 3 | `cue_or_content_tpl` | `main.rs:3036` | no content look for the kind |
| 4 | `ContentTemplates` | `main.rs:3108` (five fields) | no row in the content-look UI |
| 5 | `kiosk_content_json` | `channels.rs:896` | native windows get the field, kiosk and OBS do not |
| 6 | `CONTENT_KINDS` | `layers.js:228` | the kind cannot be toggled per screen |
| 7 | `templateShows` | `layers.js:246` | **a template with an explicit `layout.shows` hides the kind silently** |
| 8 | the two kind filters | `Output.svelte:195` and `:296` (**not** `:180`/`:279`) | the filter is applied at one door and not its twin |
| 9 | the stage branch | `Stage.svelte:406–445` | the stage page ignores the message |
| 10 | `slideKey` | `TemplateRender.svelte:1161` | a kind varying in none of the five keyed fields does not re-key: no transition, no refit |
| 11 | `cues.type` / `cue_type` | free-form TEXT, `schema.sql:219`; the comment at `:89` enumerates five kinds | no migration needed; the comment is stale the moment a sixth is written |

### Task 14: the sweep, and the assertion rule 38 never had (P1)

- [ ] **Step 1: Write the failing test first:** a payload built with `..Default::default()` and **no** `kind` must not reach `broadcast_with_clock` — `is_some_and` is false for `None`, so such a payload does not disarm the passage, every current caller sets the field, and nothing enforces it. Add the assertion and the test that fails without it.
- [ ] **Step 2:** Walk all eleven sites. For each, either make the change or write down at the site why the kind needs nothing there. **A site skipped silently is indistinguishable from a site nobody looked at.**
- [ ] **Step 3:** Site 2 is the trap and the design's answer is to keep the `Both` wire form as the existing `countdown_*` fields. If anything in this wave changes that, `is_countdown` changes **in the same commit**.
- [ ] **Step 4:** Site 7 is the second trap: wave 2 writes `layout.shows` explicitly on every seeded template, which is what makes a new kind safe. Verify that is still true of the seed as it stands today rather than assuming it.
- [ ] **Step 5:** Update `schema.sql:89`'s comment if a sixth `cue_type` is written.

### Task 15: the operator can start and see a programme timer (P1)

**Why this exists:** A registry nothing can reach from a control is a command nobody calls, which CLAUDE.md counts as attack surface rather than a feature.

**Files:**
- Modify: `src/lib/Dock.svelte` — Quick tools / the transport row
- Modify: `src/lib/views/Live.svelte` — if the list belongs on the run surface, it goes where an operator already looks
- Test: `src/lib/timers.test.js`, and `scripts/qa-inventory.mjs` must report the control as reachable

- [ ] **Step 1: Write the failing test:** a rendered control starts a `Stage` timer, the list shows it, and stopping it takes that one and no other.
- [ ] **Step 2:** Implement. **The Controls card never scrolls** — an operator may never have to scroll to reach Clear screens. If the list cannot fit without scrolling that card, it goes somewhere else.
- [ ] **Step 3:** Colour semantics: amber means ON AIR and is never allowed to lie; cyan means a guess; amethyst means rehearsal. A programme timer is none of those — pick from the existing token set and say why in the component.
- [ ] **Step 4:** `node scripts/qa-inventory.mjs` — zero handlerless buttons, zero unnamed controls, and the new command traced to a rendered control.

### Task 16: the status line question (P2)

- [ ] **Step 1:** If this track adds any status text about a timer, ask what it says when the thing behind it is broken (rule 35). If the answer is the same as when everything is fine, it is not a status line.

**PR 5 ends here.** Title: *the eleventh content kind, swept at every door, with a control an operator can reach*.

---

# Track F — the browser-driven pass

**Runs last, against everything the five tracks landed.** Lands as PR 6.

Static instruments in this repository reported zero problems throughout the two passes that found the worst defects, and were right both times. A timer is a moving render on three surfaces; nothing else here can see it.

### Task 17: drive it (P1)

- [ ] **Step 1:** `npm run tauri dev`. Console in Chrome against the mock bridge; **the output and stage pages against the real backend on `:8032`** (`http://localhost:8032/output.html?channel=<id>&template_id=<n>`, and the stage page from the same server). `5032` is Vite and exists only in dev — never point a browser source at it.
- [ ] **Step 2:** The scenarios, each stated as what a church would see:
  - A congregation countdown runs; a verse is fired; the countdown is re-aimed. **The defect this wave exists to fix.**
  - A programme timer runs while verses, songs and notices come and go on the wall. It does not blink.
  - `Esc`, then `B`. The congregation screens go; the programme timer stays; the congregation timer does not come back.
  - A stage tablet reloads mid-service and comes back to the timer AND the reading, in that order, with no flash of a clock over the verse.
  - A timer crosses its warning threshold on all three surfaces, at the default and at an override, with `prefers-reduced-motion` on and off.
  - A rehearsal: nothing reaches the stage tablet.
- [ ] **Step 3:** Record what was measured against the real backend and what was measured against the mock bridge. **They are different claims.**

### Task 18: the evidence file (P1)

- [ ] **Step 1:** Write `docs/qa/audits/<DATE>-WAVE3-TIMERS.md` — frozen evidence. Closures go in a fix log, never in the findings.
- [ ] **Step 2:** File every finding as `RG-143` onward in `docs/qa/RELAY_GAP.md`; keep `relaygap.test.js` and `crossrefs.test.js` green.
- [ ] **Step 3:** Re-measure `docs/qa/QA_HARNESS.md` §0 from the runners' own summary lines.
- [ ] **Step 4:** Update `CLAUDE.md` only where this wave made an existing sentence false. **Do not add a wave 3 narrative to it.**

**PR 6 ends here.** Title: *the wave 3 browser-driven pass*.

---

## What this wave does not claim

- It does not move the release decision. Word error rate is unmeasured in every language, neither platform has a code-signing certificate (RG-73), and nobody but the author has run a service.
- It does not touch detection, the router, or any threshold.
- It does not change the `Both` timer's wire form, so nothing downstream of `countdown_*` is re-tested by this wave rather than trusted.
- It does not close RG-139, RG-140 or RG-141 — the region-model rendering rows are wave 5's, and a timer rendered through a region-model template is still the same defect afterwards.
- It does not pre-empt wave 5 Track G: `label` stays a field, and whether the dock supplies one is that track's decision.
