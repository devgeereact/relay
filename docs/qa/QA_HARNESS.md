# Relay — the QA harness

How Relay is audited, by whom, with which instrument, and what the repository can already
prove. This is not part of the specification hierarchy — [SPEC.md](../SPEC.md),
[DECISIONS.md](../DECISIONS.md) and [PRODUCT_AUDIT.md](audits/SUPERSEDED.md) own the product; this
document owns how it gets checked.

It supersedes `Working-Agent.md`, `Working-Agent-PROMPT.md` and `Working-Agent-COVERAGE.md`,
which said the same things three times and drifted apart.

> **Read §4 before filing anything.** It is the evidence baseline: what the existing tests
> already pin, and what no instrument here can reach. An agent that re-derives it burns a run
> rediscovering deliberate decisions — or "finds" a bug that was fixed.

**Run it with `/qa-audit`** — changed surface by default, `--full` before a release, `--live` to
drive the running app over `:8032`. Reports land in [`audits/`](audits/). The cheap half runs on
every edit: `.claude/hooks/relay-fast-gate.mjs`, path-filtered and report-only (§1.6).

---

## 0. Current inventory

Re-measured **2026-09-21**, on `phase2-service-readiness` (stacked on `stage-timers-mobile`),
after the Phase 1 audit's batches 1 to 6a. Every value below was produced by the command
beside it in this session; the 2026-09-17 block that follows is kept as history.

| Count | Value | Command |
|---|---|---|
| Rust tests | **1019 passed / 0 failed / 17 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **3447 passed, 232 files** | `npx vitest run` |
| `e2e.rs` tests | **104 passed / 1 ignored** (the ignored one is `stt::e2e_latency`, matched by substring) | `cd src-tauri && cargo test e2e` |
| Registered commands | **164** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 164/164 addressed, 0 handlerless, 0 unnamed, 1 intentional orphan (`__r6probe.svelte`) | `node scripts/qa-inventory.mjs` |
| Controls | **492**, 0 in components nothing renders | `node scripts/qa-inventory.mjs` |
| Tauri events | **27** (28 literals; `tauri://localhost` is an origin string) | `grep -rhoE '"[a-z_]+://[a-z_]+"' src-tauri/src/*.rs \| sort -u` |
| Numbered decisions | **§18 – §108** | `grep -cE '^## [0-9]+\. ' docs/DECISIONS.md` → 91 |
| Register entries | **205** (189 closed, 1 withdrawn, 15 not closed) | `docs/qa/RELAY_GAP.md` §23, pinned by `relaygap.test.js` |
| Dated audits | **4 files, 12 audits** | `ls docs/qa/audits \| wc -l` — merged 2026-09-21: FIELD, DESIGN, PERF, SUPERSEDED |
| `#[ignore]`d benches | **17** | the `cargo test` summary line above |
| `hardrules.test.js` rules | **8** | `grep -o "it('rule [0-9]*" src/lib/hardrules.test.js \| sort -u \| wc -l` |

**One parallel-run flake, recorded rather than hidden.** One full `cargo test` on 2026-09-21
failed `e2e::a_verse_that_no_screen_painted_is_counted_rather_than_silently_absent` alone; it
passes alone, passes in `cargo test e2e`, and the next two full runs passed. It reads the global
latency recorder under `latency::test_lock()`, and something else in the suite touches that
recorder without the lock. Filed here so the next person who sees it does not re-diagnose it.

### 2026-09-17 · `feat/consolidation` — kept as history

Re-measured **2026-09-17**, on `feat/consolidation` — **the assembled tree**, with all
three wave roots and all four agent branches merged and nothing in flight. This is the
first block in this file that is a claim about a whole branch rather than about one
pass over it, and it is the tree the packaged bundle was built from.

| Count | Value | Command |
|---|---|---|
| Rust tests | **886 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2692 passed, 180 files** | `npx vitest run` |
| `e2e.rs` tests | **90 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **152** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| Commands the frontend addresses | **152** | `node scripts/qa-inventory.mjs` |
| Svelte components | **58** (57 reachable) | `node scripts/qa-inventory.mjs` |
| Controls | **469** (0 in components nothing renders) | `node scripts/qa-inventory.mjs` |
| Tauri events | **25** | `grep -rhoE '"[a-z_]+://[a-z_]+"' src-tauri/src/*.rs \| sort -u`, minus `tauri://localhost` |
| Numbered decisions | **78** (§18–§95) | `grep -cE '^## [0-9]+\. ' docs/DECISIONS.md` |
| Dated audits | **11** (at the time) | `ls docs/qa/audits \| wc -l` |

`cargo fmt --all -- --check`, `clippy --all-targets -- -D warnings`, `npm run build`,
`npm run version:check` and `npm run updater:check` all clean on the same tree.
`npm run tauri build` produced `Relay.app` and `Relay_0.2.0-3_aarch64.dmg`, and
`scripts/sign-local.sh` reproduced rule 17's conditions against that bundle: hardened
runtime ON, microphone entitlement present, usage string present.

**`npm run build` ran before `cargo test`** (RG-127 — two `channels` tests serve the
real built frontend and `dist/` is gitignored).

**What this block is not.** No browser-driven pass was run against this tree. jsdom
computes no layout, so no figure here is a claim about a painted pixel — and every
defect that has reached a congregation in this project was invisible to every static
instrument while `qa-inventory` reported zero problems and was right.

---

Re-measured **2026-09-17**, on `feat/cons-docs` at `cd8322a` — the documentation pass over
`feat/consolidation`. **Read the branch before the numbers.** Three other branches were in
flight on this machine at the time, changing code this pass deliberately did not touch, so
every figure below is of the consolidation branch at that commit and of nothing else. It is
not a claim about `main`, about any wave branch, or about what the next merge will report.
`npm run build` ran first, per RG-127 — two `channels` tests read `dist/` and do not say so.
Every figure is the runner's own summary line, not a grep.

| Count | Value | Command |
|---|---|---|
| Rust tests | **853 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2601 passed, 170 files** | `npx vitest run` |
| `e2e.rs` tests | **80 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **146** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 146/146 addressed, 0 handlerless, 0 unnamed, 1 orphan | `node scripts/qa-inventory.mjs` |
| Svelte components | **51**, 50 reachable from an entry point | `node scripts/qa-inventory.mjs` |
| Controls | **473**, 0 in components nothing renders | `node scripts/qa-inventory.mjs` |
| Tauri events | **23** | `grep -rhoE '"[a-z_]+://[a-z_]+"' src-tauri/src/*.rs \| sort -u` — yields 24; `tauri://localhost` is the webview's origin string, not an event |
| Numbered decisions | **§18 – §92** | `grep -cE '^## [0-9]+\. ' docs/DECISIONS.md` → 75 |
| Register entries | **165** | `docs/qa/RELAY_GAP.md` §23, pinned by `relaygap.test.js` |
| `#[ignore]`d benches | **16** | the `cargo test` summary line above |
| `hardrules.test.js` rules | **8** | `grep -o "it('rule [0-9]*" src/lib/hardrules.test.js \| sort -u \| wc -l` |

**This pass changed documents and comments only** — no behaviour — so these figures are the
ones the consolidation branch already had. It states them because **the block below it names a
branch and a method and carries no figures at all**, which is the one thing §0 exists not to
do: a section headed *the register of counts* whose current entry has none sends every reader
back to the stale block underneath it.

**The orphan is unchanged and deliberate.** `src/lib/__r6probe.svelte` is the test probe
the V1 audit (`docs/archive/RETIRED-AUDIT-DOCS.md`) already names, excluded by name from `r6-contracts.test.js`'s own walk.

**Read the qa-inventory line sceptically, as the wave 3 block below already says.** It reports
every command addressed because it traces to a WRAPPER, which is the weaker of the two tests;
the stricter one is whether a rendered control can reach it, and RG-152 is what that
distinction caught.

---

### 2026-09-17 · `feat/waves-3-5-merge` — the block that carries no table

Re-measured **2026-09-17**, on `feat/waves-3-5-merge` — wave 3 (timers) merged into wave 5
(the shelf, the names and the seal), which is the first time the two have met. Both waves fork
from `140a2cb`, both re-measured this section against their own integration branch, and neither
figure survives the merge, so this is taken on the ASSEMBLED tree rather than chosen between
them. Wave 3 is merged at **`6bee145`**; that branch has moved since (see the note under the
wave 3 block below). `npm run build` ran first, per RG-127.

---

### 2026-09-17 · `feat/wave4-track-e` — kept as history

*This note said the opposite of what the block does, and a reader who trusted it landed
on the wrong row.* The ids below are the REGISTER's — RG-161 … RG-165. Wave 4 filed them
as RG-145 … RG-149 and they were renumbered when the branches met; `RELAY_GAP.md` §23a is
the map. The note formerly here claimed the block still carried wave 4's own ids, which
would have sent a reader from RG-162 back to RG-146 — a real row, wave 3's, about a
different finding. A redirect pointing the wrong way is worse than none, because the id
it lands on exists.

Re-measured **2026-09-17**, on `feat/wave4-track-e` — wave 4 Track E, which closes the four
findings the browser-driven pass filed as RG-146 … RG-149 and the register carries as RG-162 … RG-165. Every figure is the runner's own
summary line. `npm run build` ran first, per RG-127. Several agents were building on this
machine at the time; both suites were run to completion with nothing else compiling.

| Count | Value | Command |
|---|---|---|
| Rust tests | **817 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2351 passed, 152 files** | `npx vitest run` |
| `e2e.rs` tests | **75 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **145** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 145/145 addressed, 0 handlerless, 0 unnamed | `node scripts/qa-inventory.mjs` |

`cargo fmt --all`, `clippy --all-targets -- -D warnings` and `npm run build` all clean on the
same tree.

**The delta is taken from the row immediately below** (816 Rust / 2336 frontend across 151 files
/ 145 commands, on `feat/wave4-track-d`): **+1 Rust**, **+15 frontend across +1 file**, **+0
commands**. The Rust one is the end-of-service sweep in `e2e.rs`, which is why `e2e::` is +1 as
well. Of the fifteen frontend tests, ten are `progtimerjoin.test.js` — a new file, and the
reason it is a new file is the point of it: RG-162 and RG-163 are joins between two tracks that
were each individually green, so the test drives Live's real fire path and mounts the REAL stage
page on the frame that comes out, rather than adding cases to either side. Five are in
`stagezones.test.js`. **No command was added**, deliberately: the cue-clock lifecycle is closed
with `list_timers` and `stop_timer`, which were already registered and until now reached no
rendered control.

**One existing test was corrected rather than added to.** `stagezones.test.js` asserted that a
240-character stage alert renders at `.alert.sm` — green, and over a defect: `send_stage_alert`
caps the line at 140, so neither the message nor the step could ever exist (RG-165). The
instrument carried the same mistake as the code it covered, which is this file's own recurring
subject.

---


Re-measured **2026-09-17**, on `feat/wave4-track-d` — wave 4's three code tracks (A the stage
rail, B the cue clock and the slide sizer, C the descoped targeting) merged at `89104fd`, plus
this pass's documents. Every figure is the runner's own summary line. `npm run build` ran first,
per RG-127. The machine was shared with other agents throughout, so the frontend suite was run
twice and agreed both times.

| Count | Value | Command |
|---|---|---|
| Rust tests | **816 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2336 passed, 151 files** | `npx vitest run` |
| `e2e.rs` tests | **74 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **145** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 145/145 addressed, 0 handlerless, 0 unnamed | `node scripts/qa-inventory.mjs` |

`cargo fmt --all`, `clippy --all-targets -- -D warnings` and `npm run build` all clean on the
same tree.

**The delta is taken from the row immediately below** (784 Rust / 2254 frontend across 145 files
/ 139 commands, on `feat/wave2-integration`). Since then, across wave 3 and wave 4: **+32 Rust**,
**+82 frontend across +6 files**, **+6 commands**. The wave 4 tracks account for the last
part of it — Track A added eight stage tests and a fourth surface to
`countdownwarnmotion.test.js`'s register, Track B added `cuetimer.test.js` and
`slidesizer.test.js` (18 between them) plus five `db/plans.rs` tests and one in `timers.rs`. The
browser-driven pass that produced `audits/DESIGN.md` added **no tests**,
deliberately: it is a verification pass and its four findings are filed as RG-162 to RG-165
(RG-146 … RG-149 on the day) rather than fixed here. `e2e.rs` is +8 and qa-inventory's handlerless/unnamed counts are
unchanged; its one orphan component is still `src/lib/__r6probe.svelte`, the deliberate test
probe.

Neither suite failed. **What the suites cannot see is the point of the audit above them**: both
of wave 4's surfaces are moving renders, and the two findings that matter most — RG-162 and
RG-163 — are joins between two tracks that are each individually green. (Those two read
RG-146 and RG-147 until 2026-09-17: wave 4's own ids, which now name wave 3's hit-test and
clipping findings. The sentence resolved, to the wrong rows.)

---


Re-measured **2026-09-16**, on `feat/wave2-integration` — the whole of Wave 2 (six tracks,
fifteen tasks) merged into one tree, plus the final whole-branch fix pass. The block this
replaces was measured on `feat/wave2-track-d` alone, BEFORE tracks A, B and C merged, and every
figure in it was wrong at the integration head: it said 774 Rust / 2224 frontend / 141 files /
138 commands against 784 / 2254 / 145 / 139. The command count disagreed with the block twelve
lines below it, in the one register this repository keeps counts in. `npm run build` ran first,
per RG-127.

| Count | Value | Command |
|---|---|---|
| Rust tests | **839 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2492 passed, 163 files** | `npx vitest run` |
| `e2e.rs` tests | **77 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **145** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 145/145 addressed, 0 handlerless, 0 unnamed, 1 orphan | `node scripts/qa-inventory.mjs` |

`cargo fmt --all`, `clippy --all-targets -- -D warnings` and `npm run build` all clean on the
same tree.

**Every figure is the sum of the two waves, and that was checked rather than assumed** — a merge
is exactly where a count can quietly double-count or lose a file. The two parents' own deltas are
+30 / +73 / +10 / +5 (wave 3) and +25 / +163 / +1 / +1 (wave 5), against a base of 784 Rust, 2254
frontend, 66 `e2e.rs` and 139 commands. Rust 784 + 30 + 25 = **839**; `e2e.rs` 66 + 10 + 1 =
**77**; commands 139 + 5 + 1 = **145**. All three land on the measured figure exactly.

**Two of those base numbers were read off the tree here and three were inherited, and the
difference is worth stating** because the two waves do not agree about where they forked from.
`git merge-base` puts both at **`140a2cb`**; wave 5's block below names `2ec8000`, one commit
earlier, and the two differ by one test file (144 against 145). The command count (**139**) and
the test-file count (**145**) were read directly off `140a2cb`. The Rust, `e2e.rs` and frontend
base figures are the recorded wave 2 ones at `2ec8000` and were NOT re-measured here; they
reconcile anyway, which is evidence and not proof.

Test files: 145 + 6 (wave 3 at `6bee145`) + 12 (wave 5) = **163**, with no file added by both
waves and none removed by either — checked as a set, not as a sum. The frontend TEST count is
the one figure that is not purely additive: 2254 + 73 + 163 = 2490, and the merge itself added
**+2**. Those two are in `src/lib/stagealertpanic.test.js`, which had to be rewritten because
wave 5 wrote it to assert the opposite of the ruling that now stands (DECISIONS §91), and which
now also drives `output.html` — four cases became six.

**Three counts that the merge had to fix rather than add up**, all three found by a runner and not
by a reader:
`channels::tests::the_hello_order_puts_the_screen_frame_last` enumerates the hello reply in order
and each wave added one slot to it — `timer` and `channel_roles` — so the assertion was short by
one on a tree where the ORDER was right; `countdownwarnmotion.test.js` read `--v-red` out of
`src/app.css` and wave 5 lifted the palette into `src/tokens.css`, so a correct claim failed on a
file rather than on a colour; and `names.test.js`'s one-name register caught five wave 3 files
still saying *"word to the preacher"* and one saying *"up-next"*, which is exactly the surface that
register exists for. None of the three was a broken behaviour and all three were a green branch
each.

qa-inventory's one orphan component, `src/lib/__r6probe.svelte`, is the pre-existing, deliberate
test probe the V1 audit (`docs/archive/RETIRED-AUDIT-DOCS.md`) already names — unchanged by either wave.

Neither suite failed. The counts above are what each runner's own summary line reported.

### The wave 5 measurement this replaces

Re-measured **2026-09-16**, on `feat/wave5-track-j` — the wave 5 integration branch with all
seven code tracks merged (A the forty seeded looks, B/F/H the editor, C channel roles, D the
names, E the seal, G the bare timer, I starter content), taken during the browser-driven pass
that wave's plan leaves to integration. Seven tracks deliberately did not touch this section,
because all seven were moving the same numbers and a register that is rewritten seven times in
one wave records the last agent's worktree rather than the assembled tree. `npm run build` ran
first, per RG-127.

| Count | Value | Command |
|---|---|---|
| Rust tests | **809 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2417 passed, 157 files** | `npx vitest run` |
| `e2e.rs` tests | **67 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **140** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 140/140 addressed, 0 handlerless, 0 unnamed | `node scripts/qa-inventory.mjs` |

`cargo fmt --all`, `clippy --all-targets -- -D warnings` and `npm run build` all clean on the
same tree.

**The delta is taken from the wave 2 block** (784 / 0 / 16 Rust; 2254 passing; 139
commands; 66 `e2e.rs`), measured on `feat/wave2-integration` at `2ec8000`, which this block
called the point the wave forks from. *(It is one commit short: `git merge-base` puts both
wave 3 and wave 5 at `140a2cb`. The sentence below about which row is wrong about the file
count is about that same one-commit gap — kept as written, because it is the frozen reasoning
of the measurement it belongs to.)* Since then: **+25 Rust**, **+163 frontend**, **+1 `e2e.rs`** and **+1
registered command**.

**The command delta was derived rather than assumed**, because a net figure hides a swap —
that is what the row below had to correct last time. Diffing the `#[tauri::command]` functions
at `2ec8000` against this tree gives exactly one addition, `set_channel_role` (Track C), and
no removal. One signature changed shape (`delete_channel` became generic over
`tauri::Runtime`) without changing the count.

**The frontend file delta does not reconcile with the row below, and the row below is the one
that is wrong.** `git ls-tree -r --name-only <ref> -- src | grep -cE '\.test\.js$'` reads
**144** at `2ec8000` and **157** here, and the diff is thirteen files added and none removed:
`bundledbackgrounds`, `bundledmedia`, `channelroles`, `names`, `outputdefault`, `seal`,
`shelfcontrast`, `stagealertpanic`, `stagemessage`, `timerwords`, and under
`views/templates/` `contentlookwriters`, `editorlayout` and `templatedraft`. The runner and
the tree agree at 157 on this branch; the row below recorded **145** against a tree that held
144. It is one file, and it is recorded here rather than silently smoothed over, because a
count that disagrees with its own tree is precisely what this section exists to catch and the
correction is worthless if the next reader cannot see which figure moved.

qa-inventory's one orphan component, `src/lib/__r6probe.svelte`, is the pre-existing,
deliberate test probe the V1 audit (`docs/archive/RETIRED-AUDIT-DOCS.md`) already names — unchanged by this wave, and
already excluded by name from `r6-contracts.test.js`'s own walk of `src/`.

Neither suite failed. The counts above are what each runner's own summary line reported.
`docs/qa/audits/DESIGN.md` §9 is the same table, taken in the same run.

### The wave 3 measurement this replaces

**Merged at `6bee145`, and `feat/wave3-timers` has advanced past it since.** Seven commits landed
on that branch while this merge was being resolved — the fix pass closing RG-146, RG-147, RG-148
and RG-154, and a decisions note. They are NOT in this tree and the figures below do not include
them. Whoever merges that branch again should expect the same three documents to conflict, and
should know that wave 3's `DECISIONS §89` is `§91` here.

Re-measured **2026-09-16**, on `feat/wave3-track-f` at `1a242d8` — the whole of Wave 3 (five
code tracks) merged into `feat/wave3-timers`, plus Track C's own follow-up fix, with the Track F
browser pass branched off it. `npm run build` ran first, per RG-127, and the temporary audit
scaffolding that pass installed was removed before either runner was started.

| Count | Value | Command |
|---|---|---|
| Rust tests | **814 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2327 passed, 151 files** | `npx vitest run` |
| `e2e.rs` tests | **76 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **144** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 144/144 addressed, 0 handlerless, 0 unnamed, 1 orphan | `node scripts/qa-inventory.mjs` |

**The delta from the block below is Wave 3**: **+30 Rust** (the pure registry in `timers.rs`,
the hub's timer frame and retained slot, the panic-control scope split, and the five new
commands' own coverage), **+73 frontend across +6 files**, **+10 `e2e.rs`** and **+5 registered
commands** (`start_timer`, `adjust_timer`, `stop_timer`, `list_timers`, `show_timer`).

**The qa-inventory line is the one to read sceptically, and the Track F pass says why.** It
reports every command addressed because it traces to a WRAPPER; two of the five new commands
(`show_timer`, `adjust_timer`) have no rendered control at all, which is the stricter test
CLAUDE.md states and RG-152 records. It also reported 0 handlerless buttons throughout a pass
that found a button whose centre hit-tests to a different control (RG-146). Both readings are
correct for what the instrument measures. The one orphan component is still
`src/lib/__r6probe.svelte`, the deliberate pre-existing test probe.

### The Wave 2 measurement this replaces

Measured **2026-09-16** on `feat/wave2-integration`. Kept because the note attached to it is the
point: the block IT replaced had been measured on `feat/wave2-track-d` alone, BEFORE tracks A, B
and C merged, and every figure in it was wrong at the integration head — 774 Rust / 2224
frontend / 141 files / 138 commands against the real 784 / 2254 / 145 / 139, with the command
count disagreeing with a block twelve lines below it in the one register this repository keeps
counts in.

| Count | Value |
|---|---|
| Rust tests | 784 passed / 0 failed / 16 ignored |
| Frontend tests | 2254 passed, 145 files |
| `e2e.rs` tests | 66 passed / 0 ignored |
| Registered commands | 139 |

**Its own delta, kept with it, was taken from the row immediately below** (754 passed / 0 failed / 16 ignored Rust;
2265 passing, 144 files frontend; 139 commands) — the last measurement in this file that was
taken on a merged tree, and still reproducible at `be3f01c`. Since then: **+30 Rust** (the family
seed, the retirement migration and the theme inlining each carry their own suite in
`db/templates.rs`, alongside the countdown, transition and stage work in the other tracks), **-11
frontend across +1 file** — four theme-surface files deleted whole (`themerender.test.js`,
`themes.test.js`, `views/templatesdesk.test.js`, `views/themes/themedesk.test.js`) against five
new ones (`defaulttemplate.test.js`, `fitcoverage.test.js`, `showsregister.test.js`,
`thememerge.test.js`, `outputdefault.test.js`) — and **no net change in registered commands**:
`sync_kiosk_themes` was deleted with the theme gallery it served and `set_default_template` was
added. That pair is what the replaced block recorded as **-1**; it had only seen one of the two
tracks, which is the same reason every other figure in it was short. `e2e.rs` and qa-inventory's handlerless/unnamed counts are unchanged.
qa-inventory's one orphan component, `src/lib/__r6probe.svelte`, is the pre-existing, deliberate
test probe the V1 audit (`docs/archive/RETIRED-AUDIT-DOCS.md`) already names — not something this wave introduced, and
already excluded by name from `r6-contracts.test.js`'s own walk of `src/`.

Neither suite failed on either tree. Every count above is what that runner's own summary line
reported, and `cargo fmt --all`, `clippy --all-targets -- -D warnings` and `npm run build` were
clean on both.

---

Re-measured **2026-09-15**, on `feat/wave7-timers-templates-stage` after Wave 1 (Task 1 to
Task 8: safe mode enforced at a choke point with a shell banner, the shell's flex direction
corrected, the walk-through guard, the refused update check, five Settings repairs, `endService`
moved to the throwing group, the recognition language persisted on the voice profile, the
`docs/OUTPUT_ROUTING.md` write-up, the ATEM/NDI documentation sweep, and the splash redesign) and
after the Wave 1 review fixes (the room apply order and its failed-profile tail, the dock's
Detection switch under safe mode, the profile editor's stale language and the three doors onto the
column it writes, the walk-through's third term, the updater's own third term, the
crash-reporting read, and the documentation corrections). `npm run build` ran first, per RG-127: two `channels` tests serve
the real `dist/`, which is gitignored.

| Count | Value | Command |
|---|---|---|
| Rust tests | **754 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2265 passed, 144 files** | `npx vitest run` |
| `e2e.rs` tests | **66 passed / 0 ignored** | `cd src-tauri && cargo test e2e::` |
| Registered commands | **139** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| qa-inventory | 139/139 addressed, 0 handlerless, 0 unnamed | `node scripts/qa-inventory.mjs` |

`cargo fmt --all`, `clippy --all-targets -- -D warnings` and `npm run build` all clean on the
same tree.

**The delta is taken from the one baseline in this file that was MEASURED and can still be
re-measured.** That is the worktree pinned to the fork commit, three paragraphs down: **750
passed / 0 failed / 16 ignored** (Rust) and **2186 passing, 141 files** (frontend) at `7851a79`.
So Wave 1 and its review fixes together account for **+4 Rust, +79 frontend, +3 files**, and
`e2e.rs`, the registered commands and qa-inventory are unchanged — no command was added or
deleted. Neither suite failed.

**Why the delta is not taken from the row this block replaced.** That row was deleted when it was
superseded, and the block which replaced it cited it twice as *752 / 2189* while the row itself
had said *2187 passing, 141 files*. One of those two numbers was wrong, the row is gone, and so
nobody can now say which — which is §0's own failure mode, inside §0. The rule this settles on:
**a delta is derived from a baseline that is still reproducible, or it is not derived at all.** A
superseded row is kept in the chain below precisely so that stays possible.

**This block REPLACES the previous same-day row — the one whose frontend figure is disputed
above — rather than being stacked above it**, and that row in turn REPLACED a same-day
`rebrand/wave3` row (748 / 2179). §0 is described
everywhere else in this repository as THE register of counts, and a register that carries two
"Current inventory" blocks with different numbers for the same day is not one — the reader has no
way to tell which row is current, which is the failure this section exists to prevent. The older
chain below is kept, because a chain of superseded measurements is the evidence that the numbers
were ever taken.

**The 748 / 2179 row was never true of this branch's tree, and that was established by
measurement rather than by arithmetic.** Building a worktree at the fork point itself
(`git worktree add … 7851a79`, then `npm ci && npx vitest run` and `npm run build && cd src-tauri
&& cargo test`) reads **750 passed / 0 failed / 16 ignored** (Rust) and **2186 passing, 141
files** (frontend) — not 748 / 2179. Wave 0's diff adds exactly what the gap requires and nothing
more. The 748 / 2179 baseline was simply stale by the time this branch forked: three commits
landed on `main` after it was last measured (at `5387bb3`) and before the fork (`c6525f3`,
`4d9fb5a`, `7851a79` itself), and at least one of them added a test it never saw. This is the
failure this file already names: a count that was true on one tree and is quoted after something
else merged.

Neither suite failed. The counts above are what each runner's own summary line reported, on this
branch and, separately, on a worktree pinned to the exact fork commit.

---

Measured **2026-09-14, after the seven-workspace rebrand wave**, on `rebrand/all` with every
workspace branch merged — the tree the integrator ran the gates on, not any one agent's branch.
Seven agents each quoted a suite total measured on their own worktree (1386 · 1389 · 1395 · 1398 ·
1400 · 1405 · 1549); **not one of those figures survived the merge**, and the row that matters is
the one below, produced by the command beside it on the assembled tree. That is the whole reason
this register exists and why `docs/REBRAND.md`'s Status table now carries per-FILE counts instead.
The wave's own gates on that tree: `cargo test` 709 passed / 0 failed / 16 ignored, `npx vitest run`
1604 across 114 files, `npm run build` clean, `cargo fmt --check` and `clippy -D warnings` both
silent, `npm run version:check` consistent, and `qa-inventory` at 134/134 commands addressed with 0
handlerless buttons and 0 unnamed controls. The previous measurement is kept below for the chain.

Before that, it was measured against `rebrand/base` — `new_look_refresh` with `audit/field-2026-09-13` merged into it; both branches' own figures were measured on their own tree and neither survived the merge. Every number here is produced by a command,
and the command is named — a count you cannot reproduce is a rumour.

> **Expect these to be wrong, and reach for the command rather than the value.** Every count in
> this repository has been corrected three times in a week and been wrong again each time; two of
> those corrections were wrong because a plausible one-liner was believed over the tool that
> actually knows. `RELAY_GAP.md` §18 keeps the evidence. **This table is the register of counts
> for the whole repository** — other documents cite it rather than restating it.

| | Count | How to reproduce |
|---|---|---|
| Rust tests | **709 passing**, 16 ignored (725 declared) | `cd src-tauri && cargo test`, **after `npm run build`** — two `channels` tests serve the real `dist/`, and `dist/` is gitignored, so on a fresh clone they fail with a bare `404` that never names the reason (RG-127). Measured on the merged tree, which is 691 plus the one test the merge itself needed (`channels::tests::every_kind_this_module_publishes_has_an_explicit_verdict` — the rebrand added a hub message while the audit was writing the rule that enumerates them). **Neither branch's figure survived the merge and neither chain below is the whole story**: `new_look_refresh` read 674 / 17 and `audit/field-2026-09-13` read 683 / 16, each counting only its own additions on top of a shared 654 / 671. Both chains are kept rather than reconciled into a third, because a reconciled chain is arithmetic and these are measurements. *Rebrand chain:* It read **663 / 680** before the 2026-09-13 rebrand, which added 11: 7 in `e2e` (a screen following the content look, a word to the preacher and its rehearsal gate, the four search tests, a suppressed label still in the record) and 4 elsewhere. It read **654 / 671** before the 2026-09-05 pass, which added 9 (3 in `db` proving the corpus repair reaches a v2 database and keeps every past detection's reference — RG-102 … RG-104 — 2 in `channels` for the kiosk deadline and the SVG policy, 1 in `proimport` for the zip bomb (RG-111), and 3 for the two the audit had declined: the kiosk hub's origin gate (RG-108, two tests) and the narrowed console policy (RG-85)). It read **649 / 666** before the 2026-09-04 LAN pass, which added 5 in `channels` (ranged media, RG-96; the connection cap and the whole-head deadline, RG-97). It read **644 / 661** before the 2026-09-04 corpus pass, which added 5 (4 in `db::verses::corpus_tests` pinning the bundled KJV to the KJV's own versification and the gloss rule to the fifteen verses it got wrong, 1 asserting no bundled verse is empty). It read **629 / 646** before the 2026-09-03 fix pass, which added 15 (5 import guard, 4 service erase, 3 history indexes, 3 LAN server). It read **630 / 647** until 2026-09-02, and that extra one was not a test: a duplicated `#[test]` attribute in `db/services.rs` registered one function twice. The same duplicate made `cargo clippy --all-targets -- -D warnings` fail, which is a CI gate — so the count register and the build gate were wrong in the same place, for the same reason *Audit chain:* It read **680 / 16** before the 2026-09-10 design pass, which added 3 in `channels::tests`: two for the frame a screen that joins mid-service is sent, and one holding the matcher to what `kiosk_content_json` actually serialises (RG-129). **after `npm run build`**, because two `channels` tests serve the real `dist/` and `dist/` is gitignored; on a fresh clone they fail with a bare `404` that never names the reason (RG-127). It read **663 / 17** before the 2026-09-09 pass, which added 8 and un-ignored 1: 4 in `db::verses::corpus_tests` sweeping all 31,102 verses for a brace and for a subscription (RG-123, RG-124), 3 in `db::tests` driving the real `migrate` from v3 and from v4 (RG-125 — only the v4-start one can see the bug), and `r4_05b` for the Yorùbá cap; `r4_05` itself stopped being ignored, which is where the ignored count went from 17 to 16 (RG-126). It read **654 / 671** before the 2026-09-05 pass, which added 9 (3 in `db` proving the corpus repair reaches a v2 database and keeps every past detection's reference — RG-102 … RG-104 — 2 in `channels` for the kiosk deadline and the SVG policy, 1 in `proimport` for the zip bomb (RG-111), and 3 for the two the audit had declined: the kiosk hub's origin gate (RG-108, two tests) and the narrowed console policy (RG-85)). It read **649 / 666** before the 2026-09-04 LAN pass, which added 5 in `channels` (ranged media, RG-96; the connection cap and the whole-head deadline, RG-97). It read **644 / 661** before the 2026-09-04 corpus pass, which added 5 (4 in `db::verses::corpus_tests` pinning the bundled KJV to the KJV's own versification and the gloss rule to the fifteen verses it got wrong, 1 asserting no bundled verse is empty). It read **629 / 646** before the 2026-09-03 fix pass, which added 15 (5 import guard, 4 service erase, 3 history indexes, 3 LAN server). It read **630 / 647** until 2026-09-02, and that extra one was not a test: a duplicated `#[test]` attribute in `db/services.rs` registered one function twice. The same duplicate made `cargo clippy --all-targets -- -D warnings` fail, which is a CI gate — so the count register and the build gate were wrong in the same place, for the same reason |
| Frontend tests | **1604 passing**, 0 skipped, 114 files | `npx vitest run` — read the runner's own summary line. Measured on the merged tree; `new_look_refresh` read 1099 / 80 and `audit/field-2026-09-13` read 975 / 72, from a shared 952 / 71. **Not** `vitest list \| wc -l`: that stream carries Svelte compiler warnings too and over-counted by 7. *Rebrand chain:* It read **965 / 71** before the 2026-09-13 rebrand, which added 134 across nine new files — `rangefill`, `templatemodel`, `templatedoors`, `layerops`, `templateinspector`, `transitions`, `settingvalue`, `composite`, `outputurl` — plus cases in `layers`, `templatestyle`, `rendercontent`, `surface` and `r2livepath`. One of those files RESTORED 11 tests: `inspector.test.js` was overwritten during the rebrand and the suite stayed green, because a suite cannot report the tests it no longer has. It read **952 / 71** before the 2026-09-04 pass, which added 7 `surface.test.js` cases (RG-95: one per list surface that used to show its empty sentence over a failed read) and 6 more on 2026-09-05 (RG-110: Channels' unreachable error state, a failed save shown in success green, and the six success lines a screen reader never heard; RG-95's last two surfaces: the Planner's cue table and the run surface's three lists; RG-98: the dev server's default host). **Not** `vitest list \ *Audit chain:* It read **968 / 71** before the 2026-09-10 design pass, which added 7 in 3 files: 4 in the new `templatefit.test.js` pinning the re-fit rule (RG-128), 2 in `outputhealth.test.js` holding a failing screen to a NAME rather than an id (RG-130), and 1 in `updatechannel.test.js` scanning every view's markup for the hard-coded update sentence (RG-133). It read **965 / 71** before the 2026-09-09 pass (+3: two in `languages.test.js` for the Yorùbá block and the third state the Languages column now needs, and one register row). It read **952 / 71** before the 2026-09-04 pass, which added 7 `surface.test.js` cases (RG-95: one per list surface that used to show its empty sentence over a failed read) and 6 more on 2026-09-05 (RG-110: Channels' unreachable error state, a failed save shown in success green, and the six success lines a screen reader never heard; RG-95's last two surfaces: the Planner's cue table and the run surface's three lists; RG-98: the dev server's default host). **Not** `vitest list \ |
| `e2e.rs` tests | **53** (53 run, **0 ignored** — the file has carried no ignored test since R2-C and R2-D closed, DECISIONS §54) | `cd src-tauri && cargo test e2e::`. 39 on `new_look_refresh` plus the audit's `r136_a_recovery_in_the_record_always_has_a_loss_to_recover_from`; no test from either branch was dropped and no two names collided |
| Registered `#[tauri::command]` | **134** (five dead ones deleted 2026-08-30; `delete_service` added 2026-09-03, RG-89; `send_stage_alert` added 2026-09-13) | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| `.svelte` files | **54**, **26** of them views | `find src -name '*.svelte' \| wc -l`, and `find src/lib/views -name '*.svelte' \| wc -l` for the second — it read 22 with no reproducer beside it, and the views live in three directories |
| `<button>` occurrences | **359** | `grep -ro '<button' --include='*.svelte' src | wc -l` |
| Tables in the schema | **21** | `grep -c 'CREATE TABLE' docs/data/schema.sql` |
| Cases in the detection gate | **74** | `python3 -c "import json;print(len(json.load(open('src-tauri/data/eval_corpus.json'))['cases']))"` — it was 50, then 57, then 63; three documents still quoted an older one |
| Modal surfaces that trap focus | **10** | `grep -rl trapFocus src \| grep -c svelte` — **not** a `role="dialog"` grep, which counts a comment and misses an `alertdialog` |

> **`node scripts/qa-inventory.mjs` reports `perf_samples` as BACKEND ONLY — no command reaches
> the insert. That is correct and it is not a defect.** The table is written by the
> `relay-history` thread every 60 seconds and once more at `end_service` (`main::snapshot_latency`
> → `db::log_perf_sample`); there is deliberately no command, because latency history is not
> something an operator authors. It is listed here so the red flag reads as understood rather
> than as unnoticed.

**Status: BUILT.** What shipped:

| | |
|---|---|
| Shared Rust harness | `src-tauri/src/qa.rs` — `bare_app()` (a genuine first launch), `Wall` (Tauri events), `Kiosk` (the WebSocket door), `settle()`. `e2e.rs` uses it |
| Surface inventory | `node scripts/qa-inventory.mjs` — controls, orphans, command map, create-path chain |
| Six agents | `.claude/agents/relay-qa-{cold-start,live-path,surface,detection,failure,auditor}.md` |
| The command | `/qa-audit` — changed-surface by default, `--full`, `--live` |
| The hook | `.claude/hooks/relay-fast-gate.mjs` — path-filtered, report-only (§1.6). **Off until installed**: it lives in `.claude/settings.json`, which is gitignored because it is per machine, so a fresh clone does not have it — `npm run hooks:install` (idempotent), `npm run hooks:check` to see. **Its watch list is itself pinned** (`fastgate.test.js`, RG-69): a rule whose path stops matching does not error, it goes quiet, and four safety files had already gained tests without gaining a rule |

---

# Part 1 · The design

### 1.1 The one thing that has to change

Your prompt is written for a clickable web application: visit every screen, click every
button, watch the console, refresh, check the database. That describes a browser and a server.
Relay is neither.

- **Relay is a native desktop binary.** `localhost:5032` in a plain browser is a dead UI with
  no backend behind it — it exists only for the app's own webview and for OBS browser sources.
  There is no URL an agent can drive.
- **This machine cannot screenshot the app** (CLAUDE.md is explicit about it; the boot
  heartbeat line in stdout exists precisely because screenshots are unavailable).
- **There is no browser driver, no Playwright, no `@testing-library/svelte`** in
  `package.json`, and adding one does not help: a driver needs a page, and the page needs the
  Tauri bridge, which only exists inside the packaged webview.
- **The surface is hundreds of `<button>` occurrences across dozens of `.svelte` files** — §0 carries the two numbers and the commands that produce them; this sentence carried a stale copy of both. No agent in this
  environment can press one of them.

So a literal execution of your prompt produces exactly the failure your prompt was written to
prevent. An agent told to "click every button and report" in an app with no clickable surface
will read the source, form a confident impression, and file a report full of PASS rows that
were never observed. That is *"treating visual presence as functionality"* — your own rule —
committed by the auditor instead of the developer. It is worse than no audit, because it comes
with a score out of 100.

The fix is not to lower the bar. It is to say, for every single claim, **which instrument saw
it** — and to make "no instrument reaches this" a first-class, loudly-printed outcome rather
than a gap the agent papers over.

---

### 1.2 Five evidence layers

Every finding, and every PASS, carries a layer tag. The layers are ordered by how close they
sit to a congregation actually seeing something.

| Layer | Instrument | What it genuinely proves | What it cannot prove |
|---|---|---|---|
| **A — Command E2E** | `src-tauri/src/e2e.rs` harness: `tauri::test::mock_builder`, a real in-memory DB from `db::init_fresh`, the real router, the real pipeline, real emitted events | The backend does the right thing: the verse that reaches the outputs, the template it carries, what `nav` returns at a boundary, that a paraphrase cannot auto-fire, that rehearsal contains the broadcast | Nothing about the UI. No button, no layout, no colour |
| **B — Component mount** | vitest + jsdom, mounting real `.svelte` components with a *recording* `invoke` mock (already done by `inspector.test.js` and `layers.test.js`) | A control exists, renders, is enabled/disabled when it should be, is announced to a screen reader, and dispatches *this* command with *these* arguments | That the command does anything. The backend is fake |
| **C — Static contract** | The `ipc.test.js` pattern: parse source, assert relationships | Joins A and B. Every command a button calls is registered in Rust; every event Rust emits is listened for; every table has a create path that terminates in a rendered control; colour tokens keep their meanings | Runtime behaviour of any kind |
| **D — Live app over the wire** | `npm run tauri dev`, then HTTP `GET :8032/api/{search,fire,next,prev,clear,black,live}` and the kiosk WebSocket on `:8031` | Real backend, real DB on disk, real broadcast — end to end, for the surfaces that leave the machine. This is the preacher's-phone remote and it is genuinely black-box | Only seven routes and one socket. The console UI is not reachable |
| **E — Human** | A person, a mic, a room, hardware | Everything else | — |

**The honest join:** B + C together say *"this button is wired to a command that exists and
takes these arguments"*. A says *"that command does the right thing"*. That is not the same as
a click, and the report must never print it as one. What it does buy is the class of bug that
actually ships in this repo — a renamed command inside a `catch {}`, a rule enforced on one
surface and skipped on its twin — which is the failure mode CLAUDE.md names three separate
times.

Layer E is not a cop-out bucket. It is a deliverable: the audit's final artefact includes a
**human test script**, the ordered list of things only a person in a room can check, which is
the thing you actually need before handing a build to a church.

---

### 1.3 "No seed data" — your instinct is right, the target is different

You are right that a system which only looks alive because someone pre-filled it is a product
defect, and right that it is the single most under-tested thing in most apps. But Relay's
fresh install is not a demo fixture. `db::init_fresh` seeds:

- 31,102 KJV verses and their translation row (bundled, `include_str!`, required to build)
- 5 built-in templates plus the presets
- the default output channels
- one active voice profile

That is **product content**, not demo data. A church with an empty verse table has a broken
install, not a clean one. Deleting it and demanding the UI recreate it would be testing a
requirement that does not exist.

The version of your question that *does* apply, and applies hard:

> **Which of the tables in `docs/data/schema.sql` can only be filled by the seeder or by an
> importer — with no path a new user can reach from a rendered control?**

(`grep -c 'CREATE TABLE' docs/data/schema.sql`, and `node scripts/qa-inventory.mjs` prints the
verdict per table. The count is not written here for the reason §0 gives.)

That is mechanically answerable, and it is the first job of agent R1. The chain, per table:

```
INSERT in src-tauri/src/db/*.rs
  → the #[tauri::command] in main.rs that reaches it
  → the call('…') in src/lib/stores/capture.js
  → the component that calls that wrapper
  → a control the user can actually reach, in a view that is actually routed
```

A break anywhere in that chain is the finding, and the break is reported at the link where it
happens — "backend only", "store wrapper with no caller", "component exists but is not
routed", "reachable only from the importer". Half of link 1→2→3 is already computed by
`ipc.test.js`; the rest is new and cheap.

**One trap, from this repo, that the agent must be told about explicitly.** The `app()` fixture
in `e2e.rs` deliberately does something a fresh install does *not* — it assigns a content-type
template override, and says so in a comment, because otherwise an assertion would be vacuous.
That is correct for that suite and disqualifying for this one. The cold-start agent starts from
`init_fresh` and nothing else, or it inherits exactly the convenience it was hired to find.

---

### 1.4 The six distinctions — and what closing the sixth turned up

Your acceptance list is the right one and I would not change a word of it:

```
Preview ≠ Programme · Cued ≠ On Air · Paraphrase ≠ Direct
Suggestion ≠ Auto-fire · Clear ≠ Blackout · Rehearsal ≠ Live
```

Five of the six were already pinned by tests. Preview ≠ Programme was not, and closing it
produced the two most interesting results of this whole build.

> **Read this section as history, and check the date.** It records what was found in
> August 2026 and what was done about it *then*. Both of its subjects have since been
> **deleted from the tree**: `PreviewProgram.svelte` on 2026-08-15, and with it the staging half
> of the run rail and every test about a TAKE button. The paragraphs below are left because the
> reasoning is worth keeping; the present tense in them is not. **§4.2 is the current state.**

**First: the component that read like the safety model was not in the product.**
`src/lib/views/library/PreviewProgram.svelte` **was** 312 lines of two-pane switcher whose header
comment states the danger exactly — *"Relay used to fire on a single click. One slip of a
trackpad put the wrong scripture on a wall in front of a congregation, instantly, with no
undo"* — and **nothing imported it**. It was deleted on 2026-08-15; the file is not in the
repository. Fourteen tests were written against it and passed before
`scripts/qa-inventory.mjs` reported it unreachable on its first run. Fourteen green tests
about a screen no operator can open is the audit's own failure mode, caught by the audit's own
tool, which is the best argument for the tool I can offer.

The surface that ships is `LiveOutputRail.svelte`, and it is **not a two-pane switcher** — it
is one pane, time-multiplexed: staged content when something is staged, live content otherwise.

**Second: on that surface, amber can lie.** `src/lib/liveoutputrail.test.js` now pins twelve
invariants that hold, and carries one skipped test for one that does not:

> Verse A is on the wall. The operator clicks verse B in the library to stage it. The pane now
> renders **verse B** (line 174) while the badge — computed from
> `onAir = !!$live && !$screenBlack`, which knows nothing about `preview` (line 167) — renders
> **amber, a pulsing dot, and the word "Live"**.

The header does say "· Preview", in small grey label text, beside a pulsing amber badge. Two
indicators disagree and the louder one is wrong. Same class as the media bug already closed
once ("the wall showed a photo, the topbar said ON AIR, and the monitor showed black").

**FIXED at the time — option (b), and then SUPERSEDED.** The badge was made to describe the
**pane**: staged → grey "Preview", with the wall's own state in a second, smaller chip
(`.lo-behind`). **Neither survives**: the staging half was removed a fortnight later (audit
P1-2) because nothing could reach it, so there is no staged state for a badge to describe and
`grep -rn 'lo-behind' src/` finds nothing. The rail's amber rule is the one that remained, and
it is the one `liveoutputrail.test.js` pins today.

Option (a) — badge-only — was rejected. It trades a wrong signal for a **missing** one on the
single question this panel exists to answer, and staging is precisely the moment an operator
forgets what is still up. A panel titled LIVE OUTPUT that goes quiet about the wall the instant
you touch the library is not more honest than one that shouts the wrong thing; it is quieter
about the same failure.

The chip was a warning, not decoration, so it stayed absent when the wall was genuinely clear
and when the screens were blacked out. `src/lib/liveoutputrail.test.js` read **17 passing
tests** at that point, and none of them is about staging — the five that were about this state
went with the feature. Reproduce the current figure with
`grep -cE '^\s*(it|test)\(' src/lib/liveoutputrail.test.js`.

**SUPERSEDED AGAIN, 2026-09-14 — and the whole section above is now history.** The rebrand
(`docs/REBRAND.md` §10) took the programme monitor out of the Library altogether. It was a
second run surface inside a browsing workspace: a monitor, a HEARD panel, a transcript and five
run controls, every one of which already had an owner on `Live.svelte` or in the dock row. The
Library's right column is the item INSPECTOR now, and `LiveOutputRail.svelte` is the queue and
Go Live — which is all it uniquely owned. So **the badge this section is about no longer
exists**, and neither does the surface on which it could lie; the amber rule is held where the
programme pane actually lives, on Live. The retired claims and where each went are listed at
the top of `liveoutputrail.test.js`, deliberately, rather than being silently dropped.

Full state of all six, with the file and test names, is in **§4.2**.

---

### 1.5 Agent or hook

Both, and the hook is the small one.

**Agent — on demand, expensive, exploratory.** A `/qa-audit` slash command that runs the six.
This is a release gate and a post-big-merge gate, not a routine. It costs real tokens and real
minutes and should feel like it.

**Hook — every edit, cheap, deterministic, one of them.** `PostToolUse` on `Write|Edit`, firing
only when the edited path is on the fire path (`main.rs`, `pipeline.rs`, `router.rs`,
`channels.rs`, `capture.js`, `shortcuts.js`) and running the fast contract gate —
`npx vitest run src/lib/ipc.test.js` plus the module's own test file — surfacing failures
inline.

The constraint that decides everything about the hook is **latency**. `settings.json` already
wires twelve hook points to the claude-flow handler. A hook that adds more than a few seconds
to every edit gets disabled within a week, and a disabled safety net is worse than none because
you still believe it is there. So: one hook, path-filtered, seconds not minutes.

**Do not make the audit a `Stop` hook.** A full audit per turn is an enormous cost for a signal
you will learn to scroll past, and habituation to a red line is how the line stops working.

#### What was built, and how to turn it on

`.claude/hooks/relay-fast-gate.mjs`. Path-filtered, **report only** (never exits 2, never
blocks), and split by cost:

- **Frontend files** — runs the guarding vitest files directly. Two to three seconds, and it
  is the half that catches a renamed `#[tauri::command]` before it becomes a button that
  quietly stopped working.
- **Rust fire-path files** — prints the command to run (`cd src-tauri && cargo test e2e`) and
  does **not** run it. A ~50 s compile-and-test on every edit is how this hook would get
  deleted, and a reminder you read beats a gate you turned off.

It is committed, and registering it is **one command**:

```bash
npm run hooks:install     # idempotent — safe to run repeatedly
npm run hooks:check       # exits 1 if it is not registered
```

Registration has to be per machine because a hook can only be declared in a Claude Code
settings file, and this repo's `.claude/settings.json` is one developer's claude-flow wiring —
twelve hook points, a status line, model preferences, daemon schedules. None of that is
Relay's, so it stays gitignored, and `scripts/install-claude-hooks.mjs` adds the single entry
Relay needs to whatever settings file the machine already has.

The installer is deliberately narrow, and each of these paths is exercised: it appends **one**
hook to the `PostToolUse` → `Write|Edit|MultiEdit` group and leaves every other key untouched;
it creates a minimal settings file when there is none; it merges into a foreign one without
disturbing its model, permissions or existing hooks; it backs the file up before writing; it
**refuses and writes nothing** when the settings file is not valid JSON, because a malformed
settings file is somebody's broken session and not an invitation to replace it; and it is a
no-op when already registered.

---

### 1.6 What it produces

Your report structure survives, with two changes:

1. **Every row carries its layer and its evidence command.** Not "Create: PASS" but
   "Create: PASS (A) — `cargo test qa::cold_start::a_new_operator_can_create_a_service_plan`".
   A PASS you cannot re-run is an opinion.
2. **The BLOCKED section is promoted to a deliverable.** It becomes the human test script:
   ordered, specific, "plug in the ATEM, do this, expect that". That list is the actual output
   of an honest audit of a desktop app on a machine with no screen.

Written to `docs/qa/audits/QA-<ISO date>.md`. It never touches `PRODUCT_AUDIT.md` — that document
is a human's, written at a different altitude, and an agent overwriting it would be the worst
kind of quiet damage.

---

### 1.7 Build order — as executed

1. **Harness.** `src-tauri/src/qa.rs`: `bare_app()` (a genuine first launch, no convenience
   overrides), `Wall` (Tauri events), `Kiosk` (the WebSocket door), `settle()`. `e2e.rs` was
   refactored onto it and now holds only the *one* documented difference — the content-look
   override it needs to make its template assertion non-vacuous. Two harness self-tests keep
   the fixture honest: one asserts the fresh install has no content-look chosen except the
   deliberate `tpl_song`, the other asserts the kiosk is a genuinely different door from the
   wall. The Rust suite passes, `fmt` and `clippy -D warnings` clean.
2. **The Preview ≠ Programme gap.** Closed on the surface that ships — see §3.
3. **Six agents plus `/qa-audit`.**
4. **The hook** — §5.

---

### 1.8 Decisions — answered

| | Question | Decided |
|---|---|---|
| **8.1** | Where the agents live | **Committed.** `.gitignore` un-ignores exactly the eight QA files and nothing else; the claude-flow scratch stays out |
| **8.2** | Default scope | **Changed-surface**, computed from `git diff --name-only main...HEAD`; `--full` for a release |
| **8.3** | May it drive the running app? | **Yes, behind `--live`.** R2 and R5 get layer D only when the flag is passed, and are told when it is off |
| **8.4** | Report location | **Committed**, `docs/qa/audits/QA-<ISO date>.md` |
| **8.5** | Does the hook block? | **Report only.** It never exits 2 |

---

### 1.9 Findings from building the tooling

Three, none of which needed an agent — which is itself the argument for building the
instruments before running the audit.

| | Finding | Severity | Decision | Evidence |
|---|---|---|---|---|
| **F1** | **Amber lied while staging.** A staged slide rendered in the monitor with an amber "Live" badge beside it, because the badge read `$live` and the pane read `preview` | **P1** — the one control that answers "what are they looking at" disagreed with the wall | **FIXED**, option (b) — §3 | `src/lib/liveoutputrail.test.js`, 17 tests. Failed before the fix with `expected 'r-badge amber' not to match /amber/` |
| **F2** | **`PreviewProgram.svelte` was orphaned** — 312 lines, safety-critical by its own comment, imported by nothing | **P3** as dead code, but it caused an F1-shaped near-miss during this build: fourteen green tests against a screen no operator can open | **DELETED.** The single-pane rail is a deliberate design evolution (`LiveOutputRail.svelte` says so: "two buttons, one action, and a row of height the transcript needed more"), so the two-pane version is superseded, not pending. It is in git if it is ever wanted back; leaving it in the tree only invites the next person to fix a bug in a component nobody renders | orphan count now 0 |
| **F3** | **`song_arrangements` has no create path.** `save_arrangement` is registered, `saveArrangement` exists in the store, no component imports it — a user cannot save a song arrangement | **P2** — a built feature no user can reach | **RECORDED, not built.** An arrangement editor is a feature, and shipping one in the same session as a QA harness is how both get done badly. CLAUDE.md's "no dead-but-built commands" claim has been corrected to state the exception rather than hide it; the work belongs in ROADMAP | `node scripts/qa-inventory.mjs` → create-path table |

The rule these three share: **each was found by an instrument, not by an opinion**, and each was
invisible to the test suite that already existed.

---

---

# Part 2 · The shared preamble

> **Every `relay-qa-*` agent reads this section first, every run, and inherits it verbatim.**
> It is the part that stops the audit from lying. If you move or rename this section, update
> the six agent files in `.claude/agents/` — they cite it by name.



> Every agent gets this verbatim. It is the part that stops the audit from lying.

#### Who you are

You are auditing **Relay**, AI-assisted live presentation software for churches. It listens to
a live sermon, detects scripture, and routes content to independently-styled output screens.
The bar is not "a developer can make it work". The bar is:

> **A volunteer, in a dark booth, with no training and no second take.**

A defect here is measured in Sundays and in front of a congregation. Weigh severity that way.

#### The environment, and what it forbids you to claim

Relay is a Tauri desktop binary. There is no browser to drive, no page to visit, no screenshot
to take. `localhost:5032` is Vite's dev server; in a plain browser it is a dead UI with no
backend, and in a packaged build nothing serves that port at all.

**You cannot click anything. Never write a finding, a PASS, or a step that implies you did.**

You have five instruments. Every claim you make must name the one that saw it.

| Layer | How you use it |
|---|---|
| **A — Command E2E** | Write and run Rust tests in the `e2e.rs` style: `tauri::test::mock_builder`, a real in-memory DB, the real router and pipeline, assertions on the events that actually leave the machine. `cd src-tauri && cargo test` |
| **B — Component mount** | Write and run vitest tests that mount real `.svelte` components in jsdom with a **recording** `invoke` mock. Precedent: `src/lib/inspector.test.js`, `src/lib/layers.test.js`. `npx vitest run <file>` |
| **C — Static contract** | Parse source and assert relationships between files. Precedent: `src/lib/ipc.test.js` |
| **D — Live app** | Only if the run was launched with layer D enabled: `npm run tauri dev`, then `GET http://127.0.0.1:8032/api/{search,fire,next,prev,clear,black,live}` and the kiosk WebSocket on `:8031` |
| **E — Human** | You cannot use this. You can only write the instructions for it |

**B + C together prove that a control is wired to a command that exists and takes the right
arguments. They do not prove the command works — that is A.** Say so in the report. Do not
merge the two into "works".

#### The rules you may not talk yourself out of

1. **A screen existing is not a feature. A row in a list is not CRUD. A seeded state is not a
   state a new user can reach.** Every core workflow must be demonstrated from an empty system
   through the app's own paths.
2. **BLOCKED is a real outcome.** Microphones, rooms, accents, OBS, ATEM, ProPresenter, NDI,
   a notarized macOS build, a second monitor, anything visual — BLOCKED, with the exact manual
   steps. Never PASS. Never "appears to work".
3. **Do not fix anything.** Findings only, unless the human explicitly says otherwise. An
   auditor who starts fixing stops auditing.
4. **A PASS you cannot re-run is an opinion.** Every PASS carries the command that produces it.
5. **Enumerate every caller of anything you check.** Three of this repo's real bugs are the
   same bug: a rule enforced on one surface and skipped on its twin. Rehearsal gated three of
   four kiosk publishers. The throw-vs-swallow contract held for eight of nine wrappers.
   `NavResult` was honoured by the console and discarded by the remote with `Ok(_)`. When you
   verify a guarantee, list the doors — then check the one nobody mentioned.
6. **A contract stated in a comment is not a contract.** If a rule lives only in prose, that is
   a finding, and the remedy is a test.
7. **Score detection through the detector, never by reading the transcript.** The only question
   is *which verse would Relay put on the screen*. A grep-the-text scorer once rated a
   hallucinated "Peter 8 verse 28" a success.
8. **Suspected is allowed.** If you cannot reproduce it, file it as SUSPECTED with the
   evidence. Do not upgrade it and do not bury it.

#### What is already decided, and is therefore not a finding

Read `docs/DECISIONS.md` before filing anything architectural. These in particular are
deliberate, and reporting them as bugs wastes the human's attention:

- **No native SDI.** HDMI only; NDI if it is ever unparked. SDI is bridged with a converter,
  which a church may already own or can buy for about the price of a microphone cable.
- **NDI is parked** — needs a proprietary SDK. `open_ndi_output` returns a clear error on
  purpose. That is BLOCKED-BY-DESIGN, not broken.
- **The paraphrase embedder is TF-IDF**, the `verses.embedding` column exists and has never
  been written to, and this is documented.
- **Yoruba/Swahili/Hausa locale files ship empty on purpose.** Word error rate has never been
  measured in any language and `docs/LANGUAGES.md` says so plainly. Do not soften it — and do
  not "discover" it as though it were hidden.
- **The LAN remote (`:8032/api/*`) has no authentication**, deliberately: anyone already on the
  church network can drive the wall. Your job is not to report it — it is to check the decision
  still holds, that the surface has not silently grown routes beyond
  `search / fire / next / prev / clear / black / live`, and that it is still bound where the
  decision says it is.
- **Only `DetectionMethod::Direct` may auto-fire.** Semantic and Ambiguous are capped at
  `Suggest` at any score. If you find a way past that cap, it is a **P0**.

#### Finding format

```
ID · TITLE
Severity   P0 blocker | P1 critical | P2 high | P3 medium | P4 low
Category   correctness | crud | data | live-safety | recovery | ux | a11y | security | perf | feature-gap
Layer      A | B | C | D | (E = manual, so: BLOCKED)
Surface    file:line
Precondition   exact state, starting from a fresh install unless stated
Steps          numbered, each one runnable by someone else
Expected / Actual
Evidence       the command, and its output
Frequency      always | often | intermittent | once
Impact         what a volunteer experiences, mid-service, in front of people
Recommendation the fix direction — not the fix
```

Severity, calibrated for Relay:

- **P0** — wrong or stale content can reach a congregation; a panic control can fail silently;
  the app cannot start; data loss.
- **P1** — a core Sunday workflow is broken or dangerous; a safety distinction is not legible.
- **P2** — important functionality broken, workaround exists.
- **P3 / P4** — as usual.

---

# Part 3 · The agent roster

Thirteen workstreams would have been thirteen agents reading the same source files and
producing thirteen independently-worded opinions about it. Parallel agents pay off when they
hold **different instruments**, not different subject headings. So: six, each owning a layer
and a question nobody else can answer.

| | Agent | Layers | Owns | Forbidden to claim |
|---|---|---|---|---|
| **R1** | Cold Start | A, C | The empty-system build: a create-path matrix for **every** table in the schema, seed audit, persistence across a real reopen, first-run order, migration retryability | That a screen "works". It never sees one |
| **R2** | Live Path | A, D | The six distinctions, the transport, panic, rehearsal containment, recovery after a kill, `NavResult` on every surface that exposes nav | That an operator *understood* anything. Legibility is R3's |
| **R3** | Surface Inventory | B, C | Every control in every `.svelte` file: enumerated, classified, mounted where mountable. Dead controls, missing empty/loading/error states, focus order, colour semantics, the humaniser on every error path | That any backend call succeeded. Its backend is a mock |
| **R4** | Detection & Language | A | Scoring **through the router**, never by reading the transcript. False positives, ambiguity, code-switching, the paraphrase-shows-no-percentage rule, honesty about Yorùbá numerals and unmeasured WER | Any claim about audio or accents. WER over speech is layer E |
| **R5** | Failure & Boundaries | A, D | Offline, process kill mid-service, poisoned locks, migration retry, concurrent writes, injection through text fields, unicode round-trips, and whether the LAN remote's *decided* threat model still holds | That an integration passes. OBS/ATEM/ProPresenter hardware is BLOCKED, always |
| **R6** | Independent Auditor | all | Runs last, reads none of R1–R5 until it has produced its own list, then reconciles and writes the report and the GO / NO-GO | Nothing. It is the one allowed to contradict the others |

**The full mandate for each agent lives in its own file under `.claude/agents/`** — that is what
actually runs, and it is the source of truth. They are not duplicated here; a mandate copied
into a document is a mandate that drifts from the agent.

R6 — the independent auditor that reads nothing first — is the load-bearing one. Inherited
false assumptions are the standard failure of multi-agent review, and reading last is the only
known defence.

Two rules apply to all six:

- **No agent fixes anything during the audit.** Findings only. A fixing auditor stops auditing.
- **BLOCKED is a valid outcome and PASS is not its synonym.** Anything requiring hardware, a
  microphone, a notarized build, a second monitor, or a human eye is BLOCKED, with the exact
  manual steps written out.

---

# Part 4 · The evidence baseline

What the repository can already prove, verified by reading the tests rather than trusting the
count. This is the audit's starting line.

> **Read [`RELAY_GAP.md`](RELAY_GAP.md) §2 and §17 alongside this.** Part 4 says what the *tests*
> pin; RELAY_GAP §2 says what the *product* has, scored against an outside expansion brief, and
> §17 lists the things that are already built and must not be "added". Its §23 gap register is
> the standing list of known-open defects — confirm those rather than re-discovering them.

### 4.1 The instruments that already exist

| Layer | Present today | Where |
|---|---|---|
| **A — Command E2E** | Yes. **38 tests, 0 ignored** (`cargo test e2e::`) driving the real commands against a real in-memory DB through the real router and pipeline | `src-tauri/src/e2e.rs` |
| **B — Component mount** | Yes, and **no longer under-used: 14 files mount a real component** (`grep -rln 'new [A-Z][A-Za-z]*({' src/lib/*.test.js`) | `inspector`, `layers`, `liveoutputrail`, `arrangements`, `firstrunmic`, `lowerthird`, `qa-r5-onair`, `qa-r5-template-injection`, `r2livepath`, `r6-lifecycle-probe`, `rendercontent`, `templatestyle`, `surface` |
| **C — Static contract** | Yes, one exemplar | `src/lib/ipc.test.js` — command names both directions, event listeners, and a `greet`-has-one-caller assertion |
| **D — Live app** | Exists as a surface, is not exercised by any test | `channels.rs` serves `:8032`; `main.rs::remote_api` handles `search / fire / next / prev / clear / black / live`. Kiosk hub on `:8031` |
| **E — Human** | The bench harness is built and pointed at nothing | `bench/README.md` says what to record; `bench/.gitignore` refuses to let sermon audio into the repo |
| **F — Real-time latency** | Built 2026-08-24. Nine stamps per decode pass, mic → projector, readable in the shipped app | `src-tauri/src/latency.rs`; the rig is `stt::realtime::live_transcript_latency`; the surface is Settings → This machine |

**Layer F is new and it is the only layer that measures TIME.** Every other instrument here
answers "is the answer right"; none of them could answer "how long did it take", and for two
releases running that was the complaint. It is deliberately a shipped surface, not a test
fixture: the numbers that matter are produced by a church laptop in a church, and an instrument
that needs `cargo` is one nobody in a church will ever run. See
[`audits/PERF.md`](audits/PERF.md) for what it has and has not measured,
and Stage F of the human test script for the part that needs a room.

Totals are in §0 and are re-measured, not inherited.

**Layer B was the biggest under-used asset in the repo and is not any more** — it went from two
files to fourteen, and it is still the only instrument that can see a control at all. Two rules
came out of using it, and both are load-bearing: **`vitest.config.js` must set
`resolve: { conditions: ['browser'] }`**, or Svelte hands the test the SSR stubs and every
load-on-mount path silently does nothing while the test passes; and **a component nothing renders
is not covered, however green its tests** — fourteen passing tests were written against
`PreviewProgram.svelte` before `qa-inventory.mjs` reported that nothing imported it.

---

### 4.2 The six distinctions

| Distinction | Pinned? | Evidence |
|---|---|---|
| **Rehearsal ≠ Live** | **Yes, both doors** | `e2e.rs::nothing_reaches_the_congregation_during_a_rehearsal` and `::nothing_reaches_the_stage_monitor_during_a_rehearsal` — the second exists because the first watched Tauri events and was therefore blind to `channels::stage_next`, which publishes to the kiosk and emits nothing. Frontend side: `rehearsal.test.js` (off by default; throws rather than lying when the backend refuses; a nonsense answer counts as NOT rehearsing) |
| **Suggestion ≠ Auto-fire** | **Yes** | `router::decide` caps Semantic and Ambiguous at `Suggest` by construction; `e2e.rs` drives a paraphrase at maximum confidence and asserts it cannot reach the wall |
| **Paraphrase ≠ Direct** | **Yes** | `detect.test.js`: a spoken reference is HEARD, a paraphrase is not "however high its score", the three methods get three distinguishable keys, and *"a paraphrase NEVER shows a percentage — at any score"* |
| **Clear ≠ Blackout** | **Yes, as separate contracts** | `panic.test.js`: `clearScreens` returns FALSE on backend failure and the caller must not flash success; a failed clear raises the panic banner; *"blackout has the identical contract — it is a panic control too"*; a success clears a stale warning; no crying wolf with no backend at all |
| **Cued ≠ On Air** | **Yes** | `transport.test.js`: Esc/clear takes the plan off air but REMEMBERS the position; blackout the same; a FAILED hand-fire leaves the plan exactly as it was; clearing twice is idempotent and does not lose the position |
| **Preview ≠ Programme** | **Yes — on the surface that has one** | **Corrected 2026-09-05.** This row used to cite *"`liveoutputrail.test.js` — 17 tests"* about staging and a TAKE button, and **both the component and those tests were deleted on 2026-08-15** (audit P1-2): `PreviewProgram.svelte` had no importer and `Library.svelte::stage()` had no caller, so the preview half could not render. Scoring a safety distinction against deleted evidence is the failure this table exists to prevent. What holds it today: **`Live.svelte` owns the distinction for the plan path**, where it is implemented and reachable (`previewNext` / `previewCue` / `previewSlide` beside the program pane, both rendered through the one `TemplateRender`), and `r2livepath.test.js` §R2-E pins the Library's side as an ABSENCE — *"the Library run column has no preview half at all"*, *"the rail declares no preview prop and offers no Take button"*, and Go Live fires the queue, which is reachable. **Restated 2026-09-14**: the Library's monitor and panic tiles were removed with the §10 rebuild (they duplicated `Live.svelte` and the dock row), so this row rests on Live alone — which is where the distinction was always implemented — plus `r2livepath.test.js` §R2-E and `r6-contracts.test.js` §R6-6, both of which still pin the Library's side as an ABSENCE. `liveoutputrail.test.js` is now about the queue and Go Live: order, a failed take that leaves the item in the queue rather than looking like a success, and the absence of the panels themselves |
| **A Library press ≠ a take** | **Yes, and it was not before** | Added 2026-09-14 (REBRAND §2 · §10). A single click on a Library card used to put scripture on a congregation's wall; on a BROWSING surface that is the same press as looking. `VerseDeck`'s `press` prop defaults to the old fire-on-press so every other caller is unmoved, and the five Library panes opt into `select`. `librarypress.test.js` pins both halves — a Library press reaches no fire path, and a deck given no prop still fires — plus a source scan that names any Library pane which stops opting in. Each watched to fail against its own reverted defect |

**Preview ≠ Programme was the gap you flagged, and closing it found two things.**

**The component that read like the safety model was not in the product.**
`src/lib/views/library/PreviewProgram.svelte` — 312 lines, two panes, and a header comment
stating the danger exactly (*"Relay used to fire on a single click. One slip of a trackpad put
the wrong scripture on a wall in front of a congregation, instantly, with no undo"*) — was
imported by **nothing**, and fourteen tests were written against it before
`scripts/qa-inventory.mjs` said so. It has been deleted; the single-pane rail is a deliberate
design evolution, not an unfinished migration. The surface that ships is
`LiveOutputRail.svelte`: one pane, time-multiplexed — staged content when something is staged,
live content otherwise.

**On that surface, amber lied.** With verse A live and verse B staged, the pane rendered verse
B while the badge — `onAir = !!$live && !$screenBlack`, which knew nothing about `preview` —
rendered amber, a pulsing dot, and "Live". The header said "· Preview" in small grey text
beside it; the louder indicator was the wrong one.

**Fixed.** The badge now describes the **pane** (staged → grey "Preview"), and a second smaller
chip carries the fact the badge no longer can: `.lo-behind`, reading "Wall live" in amber or
"Wall: rehearsal" in amethyst, present only when the wall is genuinely hot — absent on clear
screens and during a blackout, because a warning that fires in the ordinary case stops being
read. Verified to fail before the fix with `expected 'r-badge amber' not to match /amber/`.

---

### 4.3 Adjacent guarantees already pinned

Useful to know so no agent re-files them:

- **`NavResult` is four distinguishable outcomes** and each is explained to the operator —
  `nav.test.js` (eight tests, including "a successful step says nothing — the wall IS the
  feedback" and "an unknown outcome degrades to silence, never to a crash"). The remote surface
  was the door that discarded it with `Ok(_)`; that is fixed and covered by
  `e2e.rs::the_remote_says_which_outcome_its_nav_had_not_merely_ok`.
- **Suggestion lifetime** outlives the router's repeat cooldown so a human can read it, and an
  undated suggestion is treated as stale rather than immortal — `suggestions.test.js`.
- **`stopCapture` cannot swallow** — `micstop.test.js`, written because one bare `catch {}`
  around both the bridge import and the command printed "Start listening" over a live mic.
- **`greet` has exactly one caller** — `ipc.test.js` fails if any file other than `App.svelte`
  mentions it. The heartbeat's value is the count.
- **Fresh-install seeding** — `db/mod.rs::seeds_full_kjv` (>31,000 verses) and
  `::seeds_the_builtin_templates` (five built-ins plus presets, and the lyrics template by
  name).
- **Migration retryability** — `ensure_service_plans_is_retryable`,
  `ensure_voice_profiles_is_idempotent`, and the schema-report tests that guard the Database
  Migration screen against drawing green ticks from a hard-coded list.
- **macOS mic entitlement + usage string** — `models::config_boots`.
- **The display cannot sleep while a screen or a microphone is live** — and the rule is
  enforced by ENUMERATION, not by memory:
  `wake::every_function_that_opens_or_closes_a_screen_refreshes_the_wake_state` walks `main.rs`
  and fails by name if a function touching `open_native_window`/`close_window` forgets. It was
  written because the original wiring missed `auto_open_outputs`, the path that runs at every
  launch (RG-47). Same shape as `servicelock::every_protected_command_actually_guards_itself`.
- **Ordinary preaching does not auto-fire a verse** — `r4_01` · `r4_02` · `r4_03`, all three
  formerly `#[ignore]`d defects. The repair is `DetectionMethod::UncertainNumber`, refused by
  `may_auto_fire` at any score and any dial: **a demotion expressed as a number is a demotion a
  dial can erase** (DECISIONS §56).
- **A bare verse belongs to the book this sentence names** — `anchor_for_bare_verses`; memory is
  the fallback, not the default. From a real service, where a five-minute-old passage beat the
  book named in the same breath (FIELD F-1).
- **Confirming a suggestion teaches the gate what was ACCEPTED** —
  `e2e::confirming_a_suggestion_teaches_the_gate_what_was_accepted`, and its twin proving a
  paraphrase's cosine moves nothing. The router's own unit test passed throughout; the bug was
  one call site up (DECISIONS §58).
- **A tie in `rank_for_wall` falls to what was said first** — asserted in BOTH directions, so it
  cannot pass vacuously. The comparator used to claim `a < b` and `b < a` (DECISIONS §59).
- **A song's running order can be created, and something renders the editor** —
  `qa.rs::a_component_can_create_a_song_arrangement` asserts both halves, because a
  component nothing renders is not a create path. This was the repository's one dead
  command (RG-21); `song_arrangements` is a create path in `qa-inventory.mjs` now.
- **An arrangement whose sections moved is marked, not remapped** — on the editor,
  in the Planner's picker, and on the plan cue that carries the same indices.
  `db/mod.rs::a_lyric_edit_keeps_an_arrangement_and_a_structural_edit_flags_it`,
  `::a_plan_cue_does_not_re_expand_through_a_drifted_arrangement`,
  `::an_arrangement_with_no_recorded_shape_is_not_called_stale`,
  `arrangements.test.js`. RG-22, DECISIONS §55.
- **Six real sermon lines are in the CI detection gate** — `eval_corpus.json`,
  `source: FIELD-2026-08-30`, verbatim from `detections.heard_text`. Five references a preacher
  really made, and the Luke 10 line that produced a wrong verse, as a NEGATIVE. **Four of the
  five correct ones are the bare "Deuteronomy 15, 7" shape this codebase demotes on purpose** —
  do not tighten that demotion without reading the corpus note.
  The corpus also says what it CANNOT see: the scorer drives `detect_direct` and the router, not
  `ContextMemory`, so a green scorecard is not evidence about the context path that actually failed.
- **A spoken in-passage jump reports itself** — `e2e.rs::r2_a_spoken_passage_jump_that_cannot_move_must_say_so`.
  This was R2-C, an open defect until 2026-08-30; the jump was the fourth door into the bug
  `NavResult` exists to prevent.
- **A passage does not outlive the content that replaced it** — a song, notice, picture or
  countdown disarms it at `broadcast_with_clock`, so `nav("next")` afterwards returns
  `NoPassage`. `e2e.rs::r2_a_passage_must_not_stay_armed_under_unrelated_content`. This was
  R2-D. Both are DECISIONS §54, and **`e2e.rs` now has no ignored tests at all.**

---

### 4.4 What a fresh install actually contains

From `db::init_fresh` — schema, then `seed`, then `ensure_tables`, then a stamped
`user_version`:

| Seeded | Why it is content, not demo data |
|---|---|
| 31,102 KJV verses + the translation row | Bundled at `src-tauri/data/kjv.json` via `include_str!`, required to build. A church with an empty verse table has a broken install |
| 5 built-in templates + presets | `templates.rs::seed_templates`. Includes "Worship Lyrics", added because every earlier built-in was scripture-shaped and put the song title where the words should be |
| Default output channels | `channels.rs::seed_channels` |
| One active voice profile | `ensure_tables` guarantees it even on a bare in-memory DB |

**Not seeded, therefore the real subject of the cold-start audit:** `service_plans`,
`plan_items`, `songs`, `song_sections`, `song_arrangements`, `saved_scripture`,
`announcements`, `media_assets`, `services`, `transcripts`, `detections`, `cues`,
`app_settings`.

**First pass, from `node scripts/qa-inventory.mjs`.** It traces
`INSERT → db fn → #[tauri::command] → capture wrapper → a component that imports it`, following
store-internal calls (so `startService`, which only `beginService` ever calls, resolves
correctly). Structure is reliable; intent is heuristic. **R1 verifies before filing, including
the rows this passes** — a tool that agrees with you is not evidence.

Every table above resolves to a create path except one:

- ~~**`song_arrangements` — no create path.**~~ **CLOSED 2026-08-30.** `save_arrangement` was
  registered, `saveArrangement` existed in the store, and no component imported it — so a user
  could not save a song arrangement at all, and "every registered command has a frontend
  caller" was true at the wrapper level and false at the level that matters. The editor
  shipped (RG-21). The other five commands in that state were **deleted** rather than given a
  UI, so the claim is now unqualified: `qa-inventory.mjs` reports zero unreachable commands.
  Was **F3** in §1.9.

Still worth R1's attention even though the tool is content:

- **`translations`** — only KJV is seeded, and the tool marks it `seeded-only` correctly. The
  question it cannot answer is whether that is a gap: the Library and the planner both treat
  translation as a first-class concept, so is multi-translation effectively "the bundled one"?
- **`app_settings`** — writable through eight wrappers. Worth confirming nothing user-visible
  depends on a key only ever written by a code path that no longer runs.

---

### 4.5 The fixture trap, written down

`e2e.rs::app()` does one thing a fresh install does not:

```rust
// A fresh install seeds templates but does NOT assign a per-content-type
// override — `tpl_scripture` is only written when the operator picks one …
db::set_content_template(&conn, "scripture", Some(tpl))
```

That is correct there: without it the "every fire carries its template" assertion would be
vacuous. It is disqualifying for a cold-start audit — an audit that starts from it inherits the
convenience it exists to detect.

**Closed.** `src-tauri/src/qa.rs::bare_app()` is `init_fresh` and nothing else, and `e2e::app()`
is now that fixture plus its one documented difference, so the difference is visible in three
lines instead of buried in a fifty-line copy. The fixture is held honest by a test rather than
a comment: `qa::tests::the_bare_fixture_is_a_first_launch_and_nothing_more` asserts no
content-look is chosen except `tpl_song` — which **is** seeded, deliberately, because every
other built-in is scripture-shaped and a lyric rendered through one put the song title where
the words should be. Writing that test is how that fact was found; it had been assumed absent.

The second harness test, `the_kiosk_door_is_watchable_and_is_not_the_wall`, asserts that
`stage_next` reaches the kiosk and emits **no** Tauri event — so if the two doors ever merge,
the rehearsal-containment tests built on `qa::Kiosk` cannot start passing by seeing nothing.

---

### 4.6 What no instrument here can reach

This list is the audit's most valuable output, not its excuse. Each item is BLOCKED and needs a
person.

> **Two rows left this list on 2026-08-30 and are recorded rather than deleted.** *Audio in* —
> a real preacher was transcribed for 49.5 minutes in a real room, and the packaged, ad-hoc-signed
> build ran the whole service ([`audits/FIELD.md`](audits/FIELD.md)). **That
> morning produced seven findings that months of reading source had not**, one of them a wrong
> verse on a congregation's wall — which is the argument for taking the rest of this table
> seriously, not for trusting it less. *Word error rate is still on the list*: being transcribed
> is not the same as being measured.

| Area | Why it is blocked | What a human must do |
|---|---|---|
| Anything visual | This machine cannot screenshot the app | Open the app; check layout, contrast, spacing, the dark palette, and that amber only ever appears when something is genuinely live |
| Window resize, multi-monitor, high DPI | No window | Resize the Live console to a small laptop screen; confirm no critical control disappears |
| Microphone, rooms, accents | No audio device, no room | `RELAY_RECORD_WAV`, `RELAY_AUDIO_RMS=1`, `RELAY_STT_TIMING=1`; then replay through `RELAY_BENCH_WAV` at church-laptop levels. Audio levels are LEARNED, never assumed — three individually reasonable thresholds once made Relay deaf to a quiet preacher, 94% voiced at studio level and 2% at a church laptop |
| Word error rate, any language | Never measured. The ruler is built (`stt::bench::wer`) and pointed at nothing | Thirty minutes of a real preacher on tape, per language. `bench/README.md` says what to record |
| Yoruba / Swahili / Hausa aliases | No native speaker has reviewed the 66×3 table | A native speaker, per language |
| OBS, ATEM, ProPresenter, Companion, Stream Deck | Hardware and software not present | Connect each; verify a failed connection shows a humanised message with a recovery action, never a raw socket error |
| NDI | Parked by decision — needs a proprietary SDK; `open_ndi_output` returns a clear error on purpose | Nothing. Confirm the error is still clear and still honest |
| The macOS microphone under a signed build | The mic dies on the **first correctly-signed build**: notarization forces the hardened runtime, under which opening an input device without `com.apple.security.device.audio-input` is TCC-killed, and without `NSMicrophoneUsageDescription` the app is terminated the instant it asks. `tauri dev` and unsigned pre-releases both work fine | `npm run tauri build && ./scripts/sign-local.sh`, then actually speak into it |
| CSP | `tauri dev` does not exercise it — Tauri loads the Vite `devUrl`, and `app.security.csp` only applies to bundled assets | `npm run tauri build`, then run the packaged binary |
| Pixels out | Nothing here has ever measured what a projector actually showed | A projector, and RG-18's contrast and distance thresholds checked against a real wall — they are WCAG (a spec for screens at arm's length) and broadcast safe-title practice, neither verified in a hall |
| An operator who did not write Relay | — | **The largest unknown in the project.** One person, one service |
| An actual congregation | — | A Sunday |

---

### 4.7 The first hour, spent

Not on an agent — and it produced three findings before one ran, which is the argument for
building instruments first.

1. ✅ **The bare cold-start fixture** (layer A) — `qa.rs`, §5. Found that `tpl_song` is
   deliberately seeded, a fact this document had asserted the opposite of.
2. ✅ **The Preview ≠ Programme test** (layer B) — found that the component it was written
   against is not in the product, and that amber lies on the one that is. §2.
3. ✅ **The create-path trace** (layer C) — `scripts/qa-inventory.mjs`. Found
   `song_arrangements` has no path from any rendered control.

**Still open, and the natural next step:** turn the create-path trace into an *assertion*.
Right now it is a report someone has to run and read. A test in the `ipc.test.js` style —
every table that is neither seeded nor runtime-only must terminate in a rendered control,
with today's one known gap listed explicitly so a **new** gap fails the build — would keep
holding after the audit is over, and runs in a second. It was deliberately not written yet:
an assertion whose expected-value list was never checked by a human is a test that pins
whatever happened to be true the day it was generated.

Then the agents, for the things a test cannot enumerate in advance.

---


> **Brought here from the V1 production audit on 2026-09-21** (`docs/archive/RETIRED-AUDIT-DOCS.md` §15, scored 2026-09-03 and re-scored 2026-09-05). **These figures are frozen at that date and are not re-scored by a commit**: every row that could move is capped by a room, a purchase or a publishing action, which is the audit's own argument. The sums are held by `v1audit.test.js`, which reads this file. The `§N` citations inside the rows refer to the archived audit's own sections.

# Part 5 · The scorecards

Three, because the briefs ask for three and because a single average hides exactly the thing
that matters.

### 15.1 PWA master audit — 84/100

Scored on the brief's own ten axes. **Where a phase is structurally N/A for a desktop
application, the axis is scored on the equivalent Relay actually has**, and the row says which.

| Axis | Score | Why |
|---|---|---|
| Functionality | **9**/10 | Every rendered control reaches a real command; 0 dead controls; the fire path is covered end to end. −1 for the import path that could kill the process until this pass |
| Security | **9**/10 | ▲ +1, 2026-09-05. RG-85 and RG-97 are closed, and three more with them: the kiosk hub refuses a page Relay did not serve (RG-108), an imported SVG is no longer an active document (RG-107), and `cargo audit` — which had never been run — reports **0 vulnerabilities** with both audits now in CI. −1 for the unauthenticated control plane, which is a recorded decision and still an exposure, and because an `Origin` is a claim a browser makes honestly and other software can forge |
| PWA | **6**/10 | Scored as *distribution and installability*: a signed installer, an updater, an offline bundle on a USB stick. −4 because the updater endpoint 404s and neither platform is code-signed |
| Offline | **10**/10 | Not approximated — it is the resting state. Nothing on the live path touches a network |
| Performance | **8**/10 | 139 ms median first partial, 0 dropped partials, bounded memory. −2 because mic→screen p95 has never been measured in a room |
| Accessibility | **9**/10 | ▲ +1, 2026-09-05. Six *"it is on the screens"* lines were silent to a screen reader and are now announced; five controls rendered `[object Object]`; a failed save was shown in success green with no role. −1, and it does not move until somebody does it: **no screen-reader run and no keyboard-only pass by a person** |
| Responsiveness | **8**/10 | Scored as *output scaling*: `cqw` templates render identically at any output size, with a measured fit floor. Console breakpoints are N/A |
| SEO | **N/A** | There is no public surface, no crawler, and nothing indexable. Scored out of the total rather than as a zero — see the note below |
| UX | **9**/10 | ▲ +1, 2026-09-05. RG-95 is closed across eleven surfaces: a list that failed to load no longer tells an operator to redo work they have already done. −1 because nobody but the author has operated any of it |
| Reliability | **8**/10 | Service lock, heartbeats, pre-air validation, crash recovery that refuses to restore on-air-ness. −2 because one wrong verse reached a real congregation and one service is one sample |

**76 of a possible 90 → 84/100 normalised.** SEO is excluded from the denominator rather than
scored 0/10: a private local application has no indexable surface, and awarding it zero would
report a failure where there is no requirement. That is stated rather than buried.

### 15.2 Relay production score — 81/100

| Area | Score | Note |
|---|---|---|
| Audio | **8**/10 | Levels learned not assumed; device fallback; debug recorder. −2: queues were unbounded until this pass, and a real room is still the only proof |
| Transcription | **6**/10 | Works, locally, offline, in four languages. **WER unmeasured in all of them** |
| Transcription latency | **9**/10 | Measured properly, on one clock, at the speed of speech |
| Offline operation | **10**/10 | |
| Synchronisation | **N/A → 10** | There is no second writer. Scored full because the *absence* is correct, not because it was built |
| Scripture detection | **9**/10 | 100 % recall, 0 wrong verses on 74 labelled cases, through the real router |
| Scripture validation | **8**/10 | Only `Direct` may auto-fire, enforced before thresholds. −2 for RG-32: a context-resolved bare verse still carries a `Direct` label it did not earn |
| Presentation routing | **9**/10 | One choke point, one validator, one renderer |
| OBS | **8**/10 | Channel-keyed URL, live template swap, transparent output for keying |
| ATEM | **5**/10 | Probed and reported, never driven. Bridging hardware is the recorded strategy |
| ProPresenter | **6**/10 | Import works. There is no live interop and none is claimed |
| Recovery | **9**/10 | Position restored, on-air-ness deliberately not |
| Live-service UX | **9**/10 | ▲ +1 — see the UX row in 15.1 |
| PWA / distribution | **6**/10 | The 404 endpoint and the absent certificates |
| Performance | **8**/10 | |
| Accessibility | **9**/10 | ▲ +1 — see 15.1 |
| Security | **9**/10 | ▲ +1 — see 15.1 |
| Data integrity | **8**/10 | ▼ **−1, and it is the only score that went DOWN.** The previous 9 was awarded without knowing that the corpus repair it was scoring **had never run on any path**: it was placed on the `user_version == 0` branch that no shipped install is on, and on that branch it opened a transaction inside one and errored out of `migrate` altogether (RG-102, RG-103). Both are fixed, with tests that drive the real `migrate` on a v2 database and assert the DATA rather than the schema — but a 9 for a repair nobody had run was a number about a hope. Seven smaller findings are filed and deliberately unfixed (RG-113), including no `busy_timeout` on a file two Relay windows can open |
| Observability | **9**/10 | Two new instruments; the report names what it cannot see |
| Long-service stability | **6**/10 | 49.5 minutes of real evidence. Two hours is untested |

**Total: 161/200 → 81/100.**

### 15.3 Live-service reliability — 63/80

The score the brief insists must not be hidden by strong scores in cosmetic areas.

> **Not one row moved on 2026-09-05, and that is the finding.** Two P0s were closed that day,
> five surfaces stopped lying about failed reads, the kiosk hub stopped handing the preacher's
> monitor to the wifi and the dependency tree went to zero vulnerabilities — and **none of it is
> field evidence.** This scorecard only moves for a room: a measured word error rate, a second
> service, an operator who did not write Relay, a projector, two hours. Every one of the eight
> rows below is capped by something that costs a Sunday morning rather than a commit, which is
> exactly why the brief asks for it separately.

| | Score |
|---|---|
| Audio reliability | **8**/10 |
| Transcription reliability | **6**/10 — WER unmeasured |
| Scripture reliability | **8**/10 — one wrong verse in one real service |
| Presentation reliability | **9**/10 |
| Integration reliability | **6**/10 — OBS/kiosk real, ATEM and ProPresenter not driven |
| Offline reliability | **10**/10 |
| Recovery reliability | **9**/10 |
| Session reliability | **7**/10 — Service Lock, timeline and replay all exist; nobody but the author has run one |

**LIVE-SERVICE RELIABILITY: 63/80.**


---


> **Brought here from the V1 production audit on 2026-09-21** (`docs/archive/RETIRED-AUDIT-DOCS.md` §17). Frozen at 2026-09-05: it dispositions the two briefs as they stood then, and the `§N`, `F-NN` and blocker numbers inside the rows are the archived audit's. `v1audit.test.js` holds the arithmetic — every PWA phase 01 to 42 once, every Relay section 00 to 105 once with no gap and no overlap.

# Part 6 · Brief disposition — every phase, both briefs

### 17.1 The PWA master audit, phases 01–42

| Phase | Disposition |
|---|---|
| 01 Project discovery | **DONE** — §3. Framework, runtime, build, data, integrations, environment all mapped |
| 02 Route & page audit | **DONE as surfaces** — §4. There is no router, no URL and no deep link; the equivalents are eight tabs and three served pages, each with its auth, offline and error posture recorded |
| 03 Functional QA | **DONE** — 952 frontend and 644 Rust tests, 38 of them driving the real fire path against a real database. Duplicate submission is guarded by busy flags on every async control; destructive actions are two-step |
| 04 PWA installability | **N/A — there is no manifest and there should not be one.** Relay installs as a signed `.dmg`/`.msi`. Scored as distribution in §15.1 |
| 05 Service worker audit | **N/A — there is no service worker.** Nothing intercepts fetches; the LAN server serves from an embedded bundle with `Cache-Control: no-cache` |
| 06 Offline-first audit | **DONE** — §11. Offline is the resting state, not a fallback |
| 07 Offline data & sync | **N/A by design** — there is no second writer anywhere in the system. Building a queue and a conflict resolver would add failure modes to a system that has none |
| 08 App update strategy | **PARTIAL — blocker #5.** The strategy is right (never during a service, operator-confirmed, data snapshot before install, a way back) and the endpoint is dead |
| 09 Responsive design | **N/A for the console** (a desktop window, 960×640 minimum) / **DONE for outputs** (`cqw`, identical at any size, with a measured fit floor) |
| 10 Touch & mobile UX | **PARTIAL** — the stage page is the phone surface and is built for it; the console is not and is not claimed to be |
| 11 Accessibility | **DONE** — §13 |
| 12 Authentication | **N/A — there are no accounts.** One operator, one machine, no session to expire |
| 13 Authorisation | **N/A — there are no roles.** No horizontal or vertical escalation is possible where there is one privilege level. The LAN control plane's openness is a recorded decision, audited in §9 |
| 14 Database & data integrity | **DONE, two defects found and fixed** — §10, F-02, F-10 |
| 15 Security | **DONE** — §9 |
| 16 API security & reliability | **DONE** — the seven `/api/*` routes, method-gated, CORS-gated, timeout-bounded, verified in production (§8). Rate limiting absent: RG-97 |
| 17 Performance | **DONE from prior measurement** — §12 |
| 18 Network resilience | **DONE** — §11. Nothing on the live path waits on a network, so there is no infinite spinner to have |
| 19 Forms | **DONE** — the forms are Settings, Planner, Templates and the importers; validation is server-side in the sense that matters (the Rust command validates, not the webview), and F-01 added the missing one |
| 20 Error handling | **DONE, one defect fixed** — one humaniser, typed errors across the bridge, and F-08 closed the last Loading/Empty/Error conflation on a screen that mattered. RG-95 records the rest |
| 21 Notifications | **N/A** — no push, no permission request, no subscription. In-app status only, which is correct for a live tool |
| 22 SEO | **N/A** — no public surface |
| 23 PWA/SEO separation | **N/A** — the whole application is private by construction |
| 24 Accessibility + PWA | **DONE** — §13; the install experience is an installer, the offline state is the normal state |
| 25 File uploads | **DONE, defect found and fixed** — F-01/F-02. Type is checked by extension against the importer's list and the MIME table is pinned to it; size is now capped; there is no upload *server*, so malware scanning and signed URLs are N/A |
| 26 E-mail & external services | **N/A** — Relay integrates with no external service on the live path |
| 27 Analytics & monitoring | **DONE** — §14. Opt-in, off by default, scrubbed |
| 28 Privacy | **DONE, a gap found and closed** — §14, F-03. Deletion existed only as "delete the folder" |
| 29 UI consistency | **DONE** — one design system, one renderer, one error humaniser, one colour semantics rule |
| 30 Content audit | **DONE** — no lorem ipsum, no fake testimonials, no fabricated statistics, no dummy contact details. **Zero `TODO` and zero `FIXME` in `src/` or `src-tauri/src/`** — deferred work lives in `KNOWN_ISSUES.md` and the `RG-` register, where it has an owner, rather than in a marker nobody greps. British English in operator-facing text, checked against the usual tells (*behavior*, *optimize*, *canceled*, *catalog*) in `locales/en.json` and the Svelte tree: no hits |
| 31 Navigation | **DONE** — eight tabs, no dead links; the Markdown links in the documentation are now checked by a test (RG-87) |
| 32 UX friction | **DONE** — the Live/Planner merge exists precisely because an operator running a plan on a separate tab could not see the AI's suggestions |
| 33 Data-loss prevention | **DONE** — crash recovery, plan drafts in the database rather than in memory, snapshot before update, and the debug recorder's *press Stop, never force-quit* warning is in `CLAUDE.md` and the user guide |
| 34 Destructive actions | **DONE** — every `delete_*` is two-step, service-locked, and consequence-labelled. F-03 added the newest one under the same rules |
| 35 Build & deployment | **DONE, and further than the previous pass** — §8. Dev build, production build, bundle, ad-hoc hardened signing, launch, and a probe of the running binary |
| 36 Production configuration | **DONE** — there is no production environment to misconfigure. The one runtime configuration that matters is the updater endpoint, and it is wrong (blocker #5) |
| 37 Regression test | **DONE** — §7 |
| 38 Code quality | **DONE** — 0 clippy warnings at `-D warnings`, 0 dead commands, 0 dead controls, 1 deliberate test-only orphan component. One dead accessor introduced by this pass was removed rather than left |
| 39 Priority system | **APPLIED** — §6 is ordered P1 first |
| 40 Do not overengineer | **APPLIED** — one dev-only dependency feature was added (`tokio/test-util`, so a five-second timeout can be tested without a five-second test). No new runtime dependency. Three proposed sweeps were **declined** and recorded rather than performed |
| 41 Final production score | **DONE** — §15, three scorecards, and the sums are checked by `v1audit.test.js` because the first draft got two of them wrong |
| 42 Final report | **DONE** — this document |

### 17.2 The Relay live-service audit, sections 00–105

Grouped, because a hundred rows of "EXISTS" is not a report. **Every section from §00 to §105 is
accounted for by exactly one row, with no gap and no overlap** — which is a claim about
arithmetic, so `v1audit.test.js` checks it rather than asking you to.

| Sections | Disposition |
|---|---|
| **00–05** Command, principle, discovery, routes, roles, auth | **DONE.** §2, §3, §4. Roles and auth are N/A with the reason recorded, not skipped |
| **06–07** Session model, state integrity | **EXISTS.** A service is a row with a real lifecycle, a Service Lock, a timeline and a replay — not an `isLive` boolean. Multi-client state agreement is N/A: there is one client |
| **08–09** Audio input, device recovery | **EXISTS, and improved.** Device fallback when the preferred config will not open, `audio://error` for everything else, levels learned never assumed. F-06 closed the memory risk. **Device hot-swap mid-service is UNVERIFIED** — it needs a hand on a cable |
| **10–14** Transcription: accuracy, latency, state model, continuity, offline | **PARTIAL, and it is the moat.** Latency is measured properly and is good (§12). PARTIAL/FINAL exist end to end and render differently. **Accuracy has never been measured in any language.** Offline transcription is real and complete |
| **15–19** Network state, offline architecture, queue, sync, idempotency | **N/A by design**, §11/§17.1-07. The one idempotency question that *does* arise — a double-press on a fire — is answered by the router's per-reference debounce and by the transport's mode awareness |
| **20–24** Scripture detection, parsing, confidence, database, routing | **EXISTS, strongest area.** Recognition, interpretation, validation and routing are four separate stages with a gate between the last two. 100 % recall and 0 wrong verses on the labelled gate. RG-32 is the one honest hole: a context-resolved bare verse wears a `Direct` label it did not earn |
| **25–29** Presentation output, command state, OBS, ATEM, ProPresenter | **PARTIAL and honest about it.** OBS and kiosk are real and heartbeat-verified. ATEM and ProPresenter are **not driven** — bridging hardware is the recorded strategy (SDI is a permanent non-goal), and `open_ndi_output` returns a clear error rather than pretending |
| **30–34** Health centre, dashboard, operator control, manual override, automation safety | **EXISTS.** **23** launch probes on a four-level severity ladder, a one-sentence readiness verdict, manual fire always present, detection disarmable, safe mode. Automation cannot act above `Suggest` unless Relay *heard* the reference |
| **35–37** Transcript editing, session history, audit log | **EXISTS / N/A.** There is no transcript editing and none is proposed — editing a record of what was said is a different product. History and the event timeline are real and survive a quit |
| **38–40** Database integrity, concurrency, data loss | **DONE, two defects fixed** — §10 |
| **41–44** Manifest, service worker, cache strategy, update safety | **N/A / N/A / N/A / PARTIAL** — §17.1-04/05/08 |
| **45–48** Sleep-resume, network failure, recovery engine, recovery confirmation | **EXISTS for the states that can occur.** Wake-lock while a screen or a mic is live; the degraded-state line in the shell; recovery restores position and refuses to restore on-air-ness. **Sleep/resume mid-service is UNVERIFIED** |
| **49–51** Error handling, notifications, alerts | **DONE, one defect fixed** (F-08). No sound alerts, deliberately: an alarm during a sermon is a worse outcome than the fault it announces |
| **52–53** Mobile, responsive live interface | **PARTIAL.** The stage page is the mobile surface and works. The console is desktop-only and is not claimed otherwise |
| **54–57** Accessibility, performance, long service, large transcript | **DONE / DONE / PARTIAL / DONE.** The transcript UI is bounded by construction, so there is no virtualisation problem. 49.5 minutes is the longest real run |
| **58–64** Security, multi-tenant, credentials, rate limiting, media, API, webhooks | **DONE** — §9. Multi-tenancy and webhooks are N/A; rate limiting is RG-97 |
| **65–67** Observability, health checks, logging | **EXISTS, two instruments added** — §14 |
| **68–72** Privacy, retention, backup, destructive actions, search | **DONE, a real gap closed** (F-03). Backup is the update snapshot plus Export; a church-wide backup strategy is the church's, and Relay says where the folder is |
| **73–75** Settings, configuration validation, pre-service check | **EXISTS.** The path check — say one verse and watch six stages — is the strongest thing in this group |
| **76–78** Live mode, emergency mode, presentation safety | **EXISTS.** The console *is* live mode; manual operation always remains possible; the panic controls do not pass through the validator, because a validator that could refuse a blackout is a blackout that can fail |
| **79–81** False-positive dataset, transcription dataset, integration matrix | **PARTIAL.** The false-positive dataset exists and is a CI gate (74 cases, four languages). **The transcription dataset does not exist** — that is the WER hole. The integration matrix exists for OBS/kiosk and is marked BLOCKED for ATEM/ProPresenter rather than passed |
| **82** Twelve critical journeys | **10 of 12 covered by tests.** Journeys 7–8 (network failure and recovery) are structurally trivial here — nothing on the live path uses a network. Journeys 11–12 (device restart, multi-hour run) are **UNVERIFIED** |
| **83–86** P0/P1/P2/P3 blocker lists | **APPLIED** — §1 and §6 |
| **87–99** Regression, production build, console, storage, memory, CPU, installation, deep links, back/forward, crash recovery, security regression, performance regression | **DONE where reachable.** §7 and §8 cover regression, the production build and a clean console on a packaged launch. **Browser storage was audited and is small and deliberate**: `localStorage` holds one key (`relay.session.v1`) carrying the active tab, the open plan, the cue and slide position and the first-run flag — **ids and positions only, content-free by design**, so a reload mid-service does not drop the operator back on the Console tab with no idea where they were; plus the crash breadcrumb that reads the same key, the boot flags, and the operator's audio-output choice (which lives there because the fullscreen output window shares an origin with the console and that is how the choice reaches it). No `sessionStorage`, no IndexedDB, no cookies, no cache storage. Transcript and verse text are never in any of it — they stay in SQLite, per the local-first rule. Deep links and back/forward are N/A. **CPU/battery/thermal over hours is UNVERIFIED** |
| **100–102** The three scores and the final report | **§15 and this document** |
| **103–105** The release gate, the golden test, the engineering rule | **The golden test has been run once, in a real service, and it is `audits/FIELD.md` (the 2026-08-30 service).** Relay knew what was happening, the operator knew, the operator could continue, and one wrong verse still reached a congregation. That is the whole basis of the supervised-pilot decision |

---

# Part 7 · The rehearsal script — stage, timers and the preacher's phone

> **Was `docs/qa/QA_HARNESS.md`, folded in here on 2026-09-21 and otherwise
> untouched.** It is the one part of this harness that no machine can run: it needs a phone, a
> projector and a packaged build. It lives with the rest of the apparatus rather than beside it
> so that "how Relay is checked" is one document.

**Status: NOT RUN.** This is phase 6 of
[`superpowers/plans/2026-09-19-stage-timers-mobile.md`](../superpowers/plans/2026-09-19-stage-timers-mobile.md).
Everything phases 0–5 changed was verified by unit tests, in-process WebSockets and a real
SQLite. **None of it has been seen by a phone, a projector or a packaged build.** This
document is what turns that into evidence.

Work through it in order — the early steps are prerequisites for the later ones. Record what
you see beside each check, including the ones that pass. A pass nobody wrote down is a pass
nobody can cite later.

---

## 0. Before anything: a build that contains the work

**This is the step most likely to waste your morning, so it is first.**
`network_addresses` is a command added during this work, and the wrapper that calls it is
written to throw. If you run against a binary that predates it, the Sharing pane offers **no
link and no QR at all** — which looks exactly like the bug you are trying to test. That is
plan finding S13.

```bash
cd /Users/mrgee/WebstormProjects/relay
git checkout stage-timers-mobile
npm install
npm run build
cd src-tauri && cargo build --release && cd ..
```

Then either `npm run tauri dev` (fastest) or `npm run tauri build` for the packaged app.
**Do the packaged build at least once** — §12 below is about the CSP, and `tauri dev` does
not exercise it (CLAUDE.md).

Record: which you ran, and the time you built.

---

## 0b. What has already been checked from this machine — and what that is worth

Run against the **packaged build** on 2026-09-19, over the LAN address
`192.168.1.144`, with no mocks anywhere. This is not a phone and does not
replace any check below; it means the parts a phone depends on were working
before you started.

| Checked | Result |
|---|---|
| `GET /stage.html` over the LAN address | HTTP 200 |
| `GET /output.html` over the LAN address | HTTP 200 |
| CSP header on the packaged build (§12b) | present, `default-src 'self'` … plus `X-Content-Type-Options: nosniff` |
| `GET /api/stage_zones` | `{"ok":true,"zones":{}}` — valid JSON |
| `GET /api/live`, `/api/search` | 200 |
| `GET /api/next` | 405, as it must be — mutators are POST only |
| WebSocket `hello` on channel 2 | replies `template, default_template, channel_roles, channel_looks, channel_shows, screen_state` |
| `beat` → `beat_ack` (§4) | answered, host clock within **1 ms** |
| Unparseable frames in the whole exchange | 0 |

**This found a real defect before you did.** `/api/stage_zones` was returning
`"zones":{}` — a fragment, not JSON. `Stage.svelte` would have thrown parsing
it, swallowed the error by design, and fallen back to the device's own zones —
so **§9b would have failed with no explanation anywhere**. Both test suites were
green: the frontend one mocks `fetch`, and the Rust route inventory only checked
that a route answered, not that the answer could be read. Fixed, and the
inventory now parses every route's body.

**What this does NOT tell you**, and why the rest of this document still stands:
nothing here rendered a pixel, decoded a QR with a camera, ran on iOS or
Android, survived a sleeping phone, or watched a timer for an hour. A socket
that answers correctly to a script is not a screen a preacher can read.

---

## 1. Record the room

Before touching Relay, write down:

- Computer: model, OS version.
- Phone/tablet: model, OS version, browser (Safari or Chrome — test both if you have both).
- Network: the wifi name, whether the computer is on wifi or ethernet, whether there is a
  guest network, whether the router has client isolation on.
- Relay build: `dev` or packaged, and the time from §0.

Most connection failures are the network, and none of this is diagnosable afterwards without
those five lines.

---

## 2. The address and the QR

**Outputs → Screens** → the stage screen → set **Role** to *Stage display*.
Then **Outputs → Sharing**.

| Check | Expected | If not |
|---|---|---|
| 2a | The address picker lists a real adapter (`Wi-Fi: 192.168.x.x`), not `localhost` | Press **Refresh addresses**. If it still says no address, record the whole pane |
| 2b | The link reads `http://<that address>:8032/stage.html?channel=<n>` | Record what it says instead |
| 2c | **Show QR** draws a code, about 240px, with a clear white border | Record whether the button did nothing, or an error appeared |
| 2d | **Scan it with the phone's camera.** It offers the same address | **This is the one thing no test can substitute.** If the camera will not read it, say so — it is a generation problem, not a network one |

Now open it. The page should load and say **live** in the header within a second or two.

Record: the address, and how long it took.

---

## 3. The console stops lying about the phone

With the phone connected, look at **Outputs → Screens** and at the Live desk.

| Check | Expected | Why |
|---|---|---|
| 3a | The stage screen reports as painting / attached — NOT "has never reported painting" | Plan S11. Before this work a correctly wired tablet always read as dead |
| 3b | Live's Stage Timer band does not warn that a Stage Timer needs the stage address | Same finding, seen from the run surface |

---

## 4. The clock

Start a **Screen Countdown** of 5 minutes from Quick tools.

| Check | Expected |
|---|---|
| 4a | The figure on the phone matches the console to within a second |
| 4b | Deliberately set the PHONE's clock a minute out (Settings → Date & Time, manual), reload the page, and compare again — it should still match | 

4b is the whole of plan S6. Before this the phone showed its own error. Put the phone's
clock back afterwards.

---

## 5. Losing the connection, and getting it back

| Check | Do this | Expected |
|---|---|---|
| 5a | Turn the phone's wifi off | Within ~6 seconds the header stops saying **live** and says **not answering · Ns**, with the seconds counting up. The verse stays on screen |
| 5b | Turn wifi back on | It returns to **live** on its own |
| 5c | Lock the phone for two minutes, then unlock | It reconnects **immediately** on unlock, not after a wait |
| 5d | Quit Relay, watch the phone, start Relay again | It reconnects and the current verse comes back |
| 5e | While disconnected, check the verse did not silently advance | Whatever was last on screen is still there |

5c is the reason the retry backs off — the long waits are for a page nobody is looking at.

---

## 6. The control panel

Open **Control** on the phone.

| Check | Do this | Expected |
|---|---|---|
| 6a | Search a reference, tap a result | It appears on the wall |
| 6b | **Next** / **Prev** | The wall moves; the phone says nothing when it worked |
| 6c | Reach the end of a reading, tap **Next** | "End of the reading." |
| 6d | Put the phone in airplane mode and tap **Next** | Within 6 seconds: **"Relay did not answer. Look at the screen — it may or may not have moved."** The buttons come back |
| 6e | | It must NOT say "No next verse." |

6d/6e are plan S9. The old behaviour left the panel disabled for the rest of the service and
described a network failure in the words of a correct passage boundary.

---

## 7. The Stage Timers

From the Live desk, **Stage Timer** band.

| Check | Do this | Expected |
|---|---|---|
| 7a | Start a 1-minute timer named "Sermon" | Appears on the phone's rail and on the band |
| 7b | Let it run past zero | Both read `+0:05 over` and keep counting. The rail marks it |
| 7c | Press **Hold** while it is over | Both freeze at the same figure, and both say held. **This is RG-175 and it could not be done before** |
| 7d | Press **Resume** | It carries on from where it was held, not from zero |
| 7e | Press **+5** | Five minutes from now |
| 7f | Press **Reset** | Back to 1 minute, keeping its name |
| 7g | Press **Clear screens** | The reading goes from every screen. **The Stage Timer keeps running** — this is deliberate, and the user guide used to say the opposite |

---

## 8. Counting down to a time of day

| Check | Do this | Expected |
|---|---|---|
| 8a | Quick tools → countdown → type a clock time ~3 minutes ahead in **or at** → Start | The wall counts down to that time |
| 8b | Type a time that has already passed → Start | It starts **already over**, counting up — it does NOT jump to 23-something |
| 8c | Type nonsense ("half ten") | **Start** is disabled and the field is marked |
| 8d | Same three on the Live Stage Timer band's **or at** field | Same behaviour |

---

## 9. Stage layouts

**Outputs → Stage layouts.**

| Check | Do this | Expected |
|---|---|---|
| 9a | Before assigning anything, set some zones on the PHONE's own Zones panel | They apply, and the desk cannot see them |
| 9b | Assign **Timer focus** to the stage screen | The phone changes within a second. Its Zones panel is now disabled and says the desk set it |
| 9c | Set the screen back to **Whatever the device is set to** | The phone returns to **the zones you set in 9a** — not to defaults |
| 9d | Make a new layout, name it, pick zones, **Save** | Nothing changes on the phone until Save |
| 9e | Try to delete a layout the screen is wearing | Refused, naming the screen |
| 9f | Try to delete **Preacher** | Refused — it is a shipped starter and would come back |
| 9g | Restart Relay | The assignment survives |

9c is the one that matters most: it is the promise that this feature did not quietly destroy
an arrangement a church was already using.

---

## 10. Stage Messages

| Check | Expected |
|---|---|
| 10a | Quick tools → type a message → **Send to stage** → it appears full-bleed on the phone |
| 10b | It appears on **no** congregation screen |
| 10c | **Take down** removes it |
| 10d | Press **Clear screens** while one is up → it comes down |
| 10e | Reload the phone → the old message does NOT come back |

---

## 11. A second stage screen

Add a second screen, set its Role to *Stage display*, give it a **different** layout, and
open it on another device (or a second browser).

| Check | Expected |
|---|---|
| 11a | Sharing lets you pick which stage screen the link is for |
| 11b | Each device shows its own layout |
| 11c | A Stage Message reaches both |

---

## 11b. A cue that names its screens (RG-161)

In the Planner, select a cue and use its **Screens** row.

| Check | Do this | Expected |
|---|---|---|
| 11b-i | Leave a cue alone and fire it | Reaches every screen, exactly as before. This is the half that must not regress |
| 11b-ii | Point a cue at the main screen only, fire it | It appears on the main screen |
| 11b-iii | Look at the preacher's screen at the same moment | **It still shows whatever it had.** It must NOT go blank — targeting narrows what a cue reaches and can never clear a screen |
| 11b-iv | Untick every screen, fire it | Reaches nothing. The row says so before you fire |
| 11b-v | Tick every screen again | The row goes back to saying "Every screen" |
| 11b-vi | With a targeted cue live, reload a screen it did NOT name | It comes back to what IT was showing, not to the targeted cue |
| 11b-vii | With a targeted cue live, press **Clear all screens** | Every screen clears, targeted or not |

11b-iii and 11b-vii are the two that matter. The first is the product decision —
an unnamed screen carries on — and the second is the guarantee that a panic
control is never narrowed by any of this.

---

## 12. The packaged build

Only meaningful on `npm run tauri build`.

| Check | Expected |
|---|---|
| 12a | The stage page loads from the packaged app, not just from `tauri dev` |
| 12b | No CSP errors in the phone's browser console (Safari: Develop menu; Chrome: `chrome://inspect`) |
| 12c | The microphone still works (CLAUDE.md rule 17 — signing kills it, and this is the build that would show it) |

---

## What to send back

For anything that failed, the useful report is:

1. Which numbered check.
2. What you saw, in your words — including the exact words on screen.
3. The five lines from §1.
4. Whether it happened once or every time.

For anything where the behaviour was *right* but the wording was confusing, say so too. Half
the findings in this work were a control that did the correct thing and described it wrongly.
