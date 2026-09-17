# Wave 4 — Stage and Planner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This repository has never checked a box in a landed plan — completion is evidenced by commits and green suites, not by the boxes.

**Goal:** Finish the two surfaces wave 3 left half-answered. The stage page already carries a programme timer rail, and that rail cannot say a timer is nearly out, is finished, or is being held — so the one surface a preacher watches is the one surface where a clock can run out without changing colour. The Planner can already choose a cue's look and estimate a service's length, and it cannot bind a cue to a timer or make a slide readable before it is picked. Wave 4 closes those, drives both surfaces in a browser against the real backend, and writes down the one thing it deliberately does not build.

**Architecture:** Three code tracks and one verification pass, each landing as its own pull request off `feat/wave4-stage-planner`, which branches from `feat/wave3-timers` at `d9dcf51` — wave 3's tracks A, B and D merged. Tracks A (the stage) and B (the Planner) are independent of each other and run in parallel. Track C is documentation and files the descoped work. Track D is the browser-driven pass and runs last, against everything the other three landed.

**Tech Stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome via the browser driver named in Track D.

**Spec:** `docs/superpowers/specs/2026-09-15-timers-templates-stage-design.md` §4 (lines 726–801).

**Depends on:** Waves 0, 1 and 2, plus wave 3 tracks A, B and D, all verified present in this tree on 2026-09-16 — `src-tauri/src/timers.rs` (the registry, 11 unit tests), `channels.rs:1212` `timer_frame_json`, `channels.rs:1424` the timer frame's own retained slot, and the countdown warning threshold as a setting.

## Baseline, measured on this branch on 2026-09-16

Taken from each runner's own summary line, on `feat/wave4-stage-planner` at `d9dcf51`, with `npm run build` run first per RG-127.

| Count | Value | Command |
|---|---|---|
| Rust tests | **810 passed / 0 failed / 16 ignored** | `cd src-tauri && cargo test` |
| Frontend tests | **2301 passed, 149 files** | `npx vitest run` |
| Registered commands | **144** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |

Re-measure at the end of the wave; do not restate these figures anywhere but `docs/qa/QA_HARNESS.md` §0.

## Global Constraints

- **Do not touch any other worktree, branch or lock.** Wave 3's tracks C and E are in flight in their own worktrees (`feat/wave3-track-c`, `feat/wave3-track-e`) and are somebody else's work. Wave 4 branches from the wave 3 integration tip and reconciles at merge time, never by editing their trees.
- **Never commit to `main`.** Each track is a PR off `feat/wave4-stage-planner`.
- Each track works in its own worktree under `.claude/worktrees/`. Set `CARGO_TARGET_DIR=/Users/mrgee/WebstormProjects/relay/src-tauri/target` so a track reuses the built dependency graph rather than compiling whisper.cpp from source again; cargo takes a file lock, so parallel runs serialise rather than corrupt.
- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127) — two `channels` tests fail on a bare 404 otherwise, because `dist/` is gitignored.
- Read test counts from the runner's own summary line, never a grep.
- Every new test verified to FAIL when its defect is reintroduced. Test the bug, not the fix.
- **Rule 2:** never hold a `Mutex` across `emit` or `broadcast_content`.
- **Rule 15 / 44:** no panic control gains a question it can fail to answer, and no overlay this wave adds may paint over `Clear screens` or take `Esc` without consuming it.
- **Rule 18 and the colour law:** amber means ON AIR, amethyst means rehearsal, cyan means a guess, grey means CUED. `src/lib/colourlaw.test.js` enforces it and this wave does not amend it. See the operator decision below.
- **Rule 24:** any new fire-path function is generic over `tauri::Runtime`.
- **Rule 35:** a status line that reads the same when the thing behind it is broken is not a status line. Every new figure on the stage rail must be able to say it has no answer.
- **Rule 36:** the check goes at the choke point, not at the call sites.
- **Rule 43:** a retained frame decides what a screen is showing, and `is_screen_frame` / `is_timer_frame` are substring matches because `serde_json`'s map is a BTreeMap. Nothing in this wave adds a matcher; if that changes, the first version looking correct is the failure mode.
- `countdown.js::countdownRemainingMs` stays the ONLY arithmetic for how long is left, on every surface.
- `src/lib/errors.js` is the ONE backend-error humaniser.
- Commit message bodies and PR bodies are normal English prose.

## Corrections to the design, verified against this tree on 2026-09-16

Every structural claim in §4 was checked by reading the cited code. The substance held in most places; the citations have drifted, three claims are false as written, and two facts the design did not know have appeared since.

1. **`docs/REBRAND.md` has its own wave numbering and it is not this one.** REBRAND's "fourth wave · T2" is the template shelf, landed 2026-09-14. Where a sentence could be read either way, say which numbering it means.

2. **The stage timer rail is already built.** The spec reads as though wave 4 builds it. Wave 3 Track B landed it: the frame at `channels.rs:1212-1227`, its own retained slot at `channels.rs:1424` kept out of `last_screen` so a clock cannot erase a retained verse, the hello replay at `channels.rs:1936-1953` ordered before the retained screen frame, the derivation at `Stage.svelte:260-263` and the markup at `Stage.svelte:658-668`, with seven tests in `src/lib/timers.test.js`. **Do not rebuild it.**

3. **`Stage.svelte:481` is not where "renders no template at all" lives.** That line is inside the wave-3 `timer` branch of the socket handler. The real statements are `Stage.svelte:413`, `:525-528` and `:805-806`, and the claim is independently pinned by `src/lib/r6-contracts.test.js:115`.

4. **The stage zone key holds a seventh value the spec does not mention.** `relay.stage.zones` also carries `figures: 'bottom' | 'beside'` (`Stage.svelte:168`, written `:186`, restored `:179`). A layout pass that moves the figures is changing persisted state, not only CSS.

5. **`Stage.svelte:114-134` records that REBRAND §5 asked for three zones to ship OFF and that they were deliberately flipped ON.** A layout pass that re-reads §5 will find a sentence the code has already overruled on purpose.

6. **There is no channel dimension on the wire, so §4.2's "screen choice per cue" is not a column plus a UI.** `OutputContent` carries no channel field; `channels.rs:1089` emits `output://content` app-wide and `:1090` publishes one string to every kiosk client; `src/Output.svelte:293` never compares its own channel. The only channel-addressed message that exists is `channel://retemplate` (`main.rs:6067`, filtered at `Output.svelte:330`), which proves addressing is possible and has never been done for content. See the operator decision below — this wave specifies it and does not build it.

7. **Cue colour is not merely unset; it is test-locked to one shared ink.** `src/lib/colourlaw.test.js:96-100` asserts every `TYPE[*].color` is `TAXONOMY_INK` by identity, so any per-kind ramp fails CI until that assertion is deliberately rewritten. `--v-col-scripture` **is** `--v-amber` (`app.css:222`, `:169`), which means ON AIR. `src/lib/plan.js:13-48` records the measurement that produced the rule and says in as many words not to reach for a promise colour because it is the one that reads well.

8. **The `--v-col-*` exemption cannot extend to the Dock.** The spec grants it to the Planner because the Planner is never on air. Quick tools lives in `Dock.svelte`, which renders on every workspace including Live (`App.svelte:756`), so the same argument does not transfer. `src/lib/quicktools.test.js:370` already asserts the block spends none of the colour law, with its own slice boundaries asserted at `:376-378` so it cannot pass vacuously.

9. **`ZOOMS` is half the pattern the spec asks for.** `TemplateEditor.svelte:759-761` (not `:772`) is a component-local `let` with no session key, no localStorage and no keyboard binding. The markup idiom at `:993-997` is worth copying; the persistence must be invented, and the idiom for that is `liveFullscreen` — declared in `EMPTY` with a comment (`session.js:56`), read as a reactive derivation (`Live.svelte:1389`), written through a named one-liner (`Live.svelte:1390`).

10. **`.sgrid` is at `Live.svelte:2512`, not `:2502`**, and the cell is inlined in `Live.svelte:1787-1826` rather than living in a component. `.sg-thumb` carries `container-type: inline-size` (`:2534-2540`) and is the container-query root `TemplateRender`'s `cqw` sizes resolve against — a size control must move the grid track floor, never the thumb.

11. **The deleted density segment is recorded in four places, not one** — `Live.svelte:1372-1388`, `:1771-1777`, `:2187-2191`, `:2237-2244` — and one of them is a live constraint: the row once wanted 135px inside 114px and clipped `Compact` to `Compa`. A size control landing in the same rail can clip the same way.

12. **The Quick tools colour claim is stated at six places, not four**: `Dock.svelte:1032-1033`, `:1084-1085`, `:1105-1107`, `:1446-1447`, `:1524-1526`, `:1552-1555`.

13. **`plan_items` has `section_title` and `duration_sec` and `docs/data/schema.sql:85-94` does not show them.** The live shape is `db/plans.rs:59-72`. Any schema work in this wave corrects the doc in the same commit.

14. **`duration_sec` is the running-time estimate and nothing else reads it.** `db/plans.rs:32-34`, written at `:266-274`, read only by `plan.js:256-289` for the estimate. No timer code mentions it. Do not overload it.

15. **`plan_item_id` on a `Timer` is write-only today.** It exists at `timers.rs:76`, is set only by `start_timer` (`main.rs:3054`), rides out through `TimerView`'s flatten, and nothing reads or filters on it. Binding a cue to a timer means giving it a reader.

16. **`plannerbuildonly.test.js` has a list that must not rot.** `TAKES_A_SCREEN` (`:62-73`) names ten commands and does not include `start_timer`, `show_timer`, `adjust_timer` or `stop_timer`. `show_timer` is a second door onto a wall (`main.rs:3152-3178`). Any wave-4 work that gives the Planner a timer control adds the right names to that list in the same commit, or the guarantee has a hole in it.

17. **Numbering.** The last DECISIONS section is 88 and the highest register id filed is RG-144, and wave 3's own tracks may take the next of each before this wave lands. `crossrefs.test.js` resolves every `DECISIONS §N` and every `RG-` id against the real documents, so a plan that cites a section it has not written turns the suite red. Write the citation in the same commit that writes the section, and take the next free id at that moment rather than reserving one here.

## Operator decisions carried into this wave

Taken 2026-09-16, after the three verification passes above.

- **Per-cue screen targeting is descoped to its own wave.** It is a wire change at the one choke point wave 3's tracks C and E are still editing, and it forces rule 43's retained frame to become per-channel, which is congregation-facing. Wave 4 writes the specification and files the gap; it builds none of it. **A stored-but-unread preference is explicitly not an acceptable middle** — that is the defect the 2026-09-10 pass closed on seven Settings controls.
- **The colour law stands and cue colour is not built.** `TAXONOMY_INK` stays the one ink, the kind stays carried by the words already printed beside every cue, and `colourlaw.test.js` is not amended. `--v-col-timer` is not created.
- **Quick tools gains no colour.** Grouping, if it is done at all, is carried by the caption and the neutral surface ramp the card already uses.
- **The stage half is Stage.svelte's own states plus a layout pass.** Wave 3 Track E owns the control that starts a programme timer and is in progress elsewhere; this wave does not build one, and touches neither `Live.svelte`'s timer surface nor `layers.js`.

---

# Track A — the stage says what its clocks are doing

**Spec:** §4.1. **Files are `src/Stage.svelte` and its tests only.** That file is not touched by wave 3 tracks C or E, which is what makes this track safe to run in parallel with them.

Today a programme row renders a label and digits and nothing else. `warn_ms` is published per row (`channels.rs:1224`) and explicitly not read (`Stage.svelte:256-259`). `countdown_done` is published (`channels.rs:1223`) and never rendered, so a finished programme timer shows `0:00`. A held row freezes, correctly, and says nothing about being held. The congregation figure row has a warning state (`.fig.warn`, `Stage.svelte:640`, CSS `:498-500`) and the preacher's own rail does not.

### Task 1: a programme row that is nearly out says so (P1)

**Why this exists:** The rail is the one surface in the building whose whole job is to tell a preacher how long is left, and it is the only timer surface with no warning state. Rule 35 in its general form: a figure that looks the same at four minutes and at ten seconds is not telling anybody anything.

**Files:**
- Modify: `src/Stage.svelte` — the `programme` derivation (`:260-263`) and the `.progrow` markup (`:658-668`) and CSS (`:923-950`)
- Test: `src/lib/timers.test.js`

- [ ] **Step 1: Write the failing tests.** A row whose `warn_ms` is crossed carries the warning class; a row with `warn_ms: null` never does, at any figure; the threshold is read from the frame and never re-derived on the page; a row that is held does not flash. Name each after the field failure it prevents.
- [ ] **Step 2: Implement.** Reuse the congregation warning treatment rather than inventing a second one — `.fig.warn` and its CSS are the precedent, and `countdownWarning` already exists as the shared rule. The colour is the one already used for a warning figure; no new token.
- [ ] **Step 3: Reduced motion is a cut**, exactly as the existing warning motion is. `countdownwarnmotion.test.js` is the file that already holds that guarantee for the other surfaces.
- [ ] **Step 4: Verify each test fails with the change reverted.**

### Task 2: a finished programme timer says its message, and a held one says it is held (P1)

**Why this exists:** `countdown_done` crosses the wire per row and is dropped on the floor. The congregation countdown substitutes `cdDone` at zero (`Stage.svelte:639`); the rail prints `0:00`, which reads as a clock that is still running and has just arrived. And `countdownIsPaused` is already imported into this file (`Stage.svelte:3`) and used only for the congregation figure at `:214`.

**Files:**
- Modify: `src/Stage.svelte`
- Test: `src/lib/timers.test.js`

- [ ] **Step 1: Write the failing tests.** A row at zero with a done message shows the message; a row at zero with an empty message shows `0:00` and not an empty box; a held row is marked held and keeps its frozen figure; releasing a hold takes the mark away.
- [ ] **Step 2: Implement** through the existing readers. `countdownRemainingMs` stays the only arithmetic and `countdownIsPaused` the only held test — a second copy of either is the defect `docs/REBRAND.md` phase 7 records as fixed once already.
- [ ] **Step 3:** The held mark is not a colour from the law. Held is a state the operator caused deliberately, and `Dock.svelte:1524-1526` already records which treatment that gets.

### Task 3: the rail is a zone, and it has a floor (P2)

**Why this exists:** `ZONES` (`Stage.svelte:150-157`) has six keys and the programme rail is not one of them, so a lobby screen running the stage page cannot switch the preacher's bookkeeping off. And `.tmr { flex: 1 1 0 }` inside a `max-height: 20%` row with `overflow: hidden` divides the width by the row count with no floor — the rail has never been rendered above about three rows.

**Files:**
- Modify: `src/Stage.svelte` — `ZONES`, `DEFAULT_ZONES`, `loadZones`, the zone panel (`:673-692`), `.progrow` CSS
- Test: `src/lib/stagezones.test.js`, `src/lib/timers.test.js`

- [ ] **Step 1: Write the failing tests.** The new zone appears in the panel and toggles; it defaults ON, like every other zone; a stored `relay.stage.zones` written before this key existed still loads and gets the default rather than `undefined`; a switched-off rail gives up its room, which is the guarantee `stagezones.test.js:627` already holds for the others; `figures` survives the migration untouched.
- [ ] **Step 2: Implement.** The zone key is content-free. Persisting is `saveZones` as it stands — one key, whole object, per device, `try/catch` both ways.
- [ ] **Step 3: Give the row a floor.** Decide a minimum legible width per timer and what happens past it — wrap, scroll or a count. Whichever is chosen, a row that cannot show every timer must say so rather than shrinking them all below reading size. **Measure it in Task 10 rather than asserting it here.**
- [ ] **Step 4:** Keep every existing stage-layout invariant green: nothing leaves the screen (`stagezones.test.js:220`), the reading takes what is left (`:231`), rows below are `flex-basis` and not `height` (`:236`), each row is its own container (`:263`).

### Task 4: the layout-quality pass, read before it is driven (P2)

- [ ] **Step 1:** Read `Stage.svelte`'s nine top-level regions against each other at 1024×768, 1280×800 and a phone in portrait, and list what a preacher cannot read. Do not change anything yet — Track D drives it, and a change made from reading and then confirmed by driving is a different claim from one made from driving.
- [ ] **Step 2:** Anything fixed here must keep `stagezones.test.js`'s size table (`:403`) green, or change it deliberately with its reason.

**PR 1 ends here.** Title: *the stage rail says what its clocks are doing*.

---

# Track B — the Planner binds a cue to a clock, and a slide can be read before it is picked

**Spec:** §4.2, minus the two descoped halves. **Runs in parallel with Track A.**

### Task 5: a cue can carry a timer, and Live is what starts it (P1)

**Why this exists:** `plan_item_id` exists on every `Timer` and nothing reads it. `duration_sec` is the running-time estimate and must not be overloaded. The Planner cannot fire to an output and that guarantee is load-bearing (`plannerbuildonly.test.js`), so the Planner stores the binding and the run surface acts on it.

**Files:**
- Modify: `src-tauri/src/db/plans.rs` — one additive column through the existing idempotent `add_plan_item_column`
- Modify: `docs/data/schema.sql` — correct it for this column **and** for `section_title` and `duration_sec`, which it already omits (correction 13)
- Modify: `src-tauri/src/main.rs` — the command that writes it; `src-tauri/src/timers.rs` — a reader keyed on `plan_item_id`
- Modify: `src/lib/views/ServicePlanner.svelte` (the inspector), `src/lib/stores/capture.js` (the wrapper)
- Test: `src-tauri/src/db/plans.rs`, `src/lib/views/plannerbuildonly.test.js`, a new frontend test file

- [ ] **Step 1: Write the failing tests first.** The migration is retryable and runs twice without error (rule 25). A cue with no binding is untouched by the migration and reads as unbound, not as zero. The Planner writes the binding and invokes nothing from `TAKES_A_SCREEN`. The registry can answer "which timer belongs to this cue" and returns nothing rather than a wrong timer when none does.
- [ ] **Step 2: The column.** Name it for what it is — a timer the cue asks for — so it can never be confused with `duration_sec`. State in the doc comment, beside the column, what each of the two means and why there are two.
- [ ] **Step 3: The Planner control** goes in the cue inspector beside the template select, and stores. It must not be able to start anything: `plannerbuildonly.test.js`'s behavioural sweep (`:239-269`) clicks every control in the component and asserts none of them reaches a screen-taking command.
- [ ] **Step 4: The list that must not rot.** Add `start_timer`, `show_timer`, `adjust_timer` and `stop_timer` to `TAKES_A_SCREEN` in the same commit, with the reason. `show_timer` is a second door onto a wall and the test's own anti-rot assertion (`:195-207`) will hold the names honest.
- [ ] **Step 5: Live starts it, at one call site.** When a bound cue goes on air, Live calls `startTimer({ …, planItemId })`. One call site, in the cue fire path, so wave 3 Track E's Live surface composes with it rather than racing it. If Track E has landed by the time this task runs, read its surface first and add nothing that duplicates it.
- [ ] **Step 6: Verify each test fails with the change reverted**, including the migration's retryability — drop the column and run it twice.

### Task 6: a slide can be read before it is picked (P2)

**Why this exists:** `.sgrid` is fixed at `minmax(158px, 1fr)` and `runsurface.test.js` names 158px in its own describe title as the size at which a cell must be unmistakable. An operator cannot make a slide bigger to read it, and the thing they are choosing between is the words on it.

**Files:**
- Modify: `src/lib/views/Live.svelte` — the view-controls wrapper (`:1771-1777`), the `.sgrid` rule (`:2512`)
- Modify: `src/lib/session.js` — one field in `EMPTY`
- Test: `src/lib/slidegridwiring.test.js` or a new file, `src/lib/session.test.js`

- [ ] **Step 1: Write the failing tests.** The control steps through its sizes and stops at each end; the chosen size survives a remount through the session; a size nobody has chosen is the current 158px behaviour exactly; the control is a real button with an accessible name.
- [ ] **Step 2: Implement the stepper** on the `ZOOMS` markup idiom (`TemplateEditor.svelte:993-997`) — two `r-iconbtn` steppers and a readout, each end disabled at its end — and the `liveFullscreen` persistence idiom (`session.js:56`, `Live.svelte:1389-1390`). The value is a number or an enum, never text.
- [ ] **Step 3: Move the grid track, never the thumb.** `.sg-thumb` is the container-query root; changing it resolves every `cqw` in the cell against the wrong box.
- [ ] **Step 4: Watch the rail it lands in.** `Live.svelte:2283` records a row that wanted 135px inside 114px and clipped a label to `Compa`. Measure the control's own width in the rail, at the narrowest supported window, in Track D.
- [ ] **Step 5: If the control is ever removed**, its session key goes in `migrateSession`'s drop list in the same commit (`session.js:66-82`). Write that instruction beside the field now.
- [ ] **Step 6:** Keep `slidegridwiring.test.js` and `runsurface.test.js` green. Both locate CSS declarations **by literal string match**, so reformatting a rule or inserting a class between two of them breaks them by spelling rather than by meaning.

### Task 7: the Planner's own legibility, and nothing that paints a promise (P2)

- [ ] **Step 1:** The kind stays carried by the words. If a cue is hard to scan, fix it with type, spacing, rule weight or the heading it already sits under.
- [ ] **Step 2:** `colourlaw.test.js` is not amended by this wave. If a change here makes it fail, the change is wrong.

**PR 2 ends here.** Title: *a cue can carry a clock, and a slide can be read before it is picked*.

---

# Track C — what wave 4 does not build, written down where it will be found

**Documentation only. No code.** Runs in parallel with A and B.

### Task 8: specify per-cue screen targeting (P1)

**Why this exists:** The spec says "add it, defaulting to all" about a facility that has no wire, no receiver filter and no per-channel retention. Leaving that sentence in a spec with no correction beside it is how the next wave under-scopes it again.

**Files:**
- Modify: `docs/superpowers/specs/2026-09-15-timers-templates-stage-design.md` — a correction beside §4.2's paragraph, stating what it costs
- Modify: `docs/qa/RELAY_GAP.md` — file it at the next free id

- [ ] **Step 1: State the four pieces** it actually needs, each with the citation that proves it is missing: a channel set on the content (`channels.rs` `OutputContent`), a routed publish at the choke point (`channels.rs:1089-1090`), a receiver-side filter (`src/Output.svelte:293`, which today never compares its own channel — the precedent that it can is `channel://retemplate`, filtered at `:330`), and per-channel retention so a screen that joins mid-service is shown what is on **its** screen (rule 43, `channels.rs:1424`).
- [ ] **Step 2: State what must not move.** The panic controls stay global — a `clear` that has to ask which screens it is talking to is a `clear` that can fail (rule 15). `template_pinned`'s meaning on a screen a cue does not target must be answered before any code is written.
- [ ] **Step 3: File the gap** with its priority and the reason it was descoped, and keep `relaygap.test.js` and `crossrefs.test.js` green. Take the next free id at the moment of writing.
- [ ] **Step 4: Do not restate the scope in a fifth place.** `RELAY_V1_AUDIT.md`, `RELAY_GAP.md`, `QA_HARNESS.md` §0 and the audit files are the four registers that already exist.

### Task 9: record the two decisions this wave took (P2)

- [ ] **Step 1:** The colour law was re-examined against §4.2's proposal and kept. Record it where the law lives — a short note beside `plan.js:44-47`'s escape hatch is enough, and it must say that the proposal was considered and refused, not that nobody asked.
- [ ] **Step 2:** If wave 3 Track C has landed by now, check whether the scope-split panic control has a DECISIONS section yet. The behaviour is live at `channels.rs:1067-1072` and was undocumented at the time this plan was written. **Do not write that section for another track** — say so in the PR body if it is still missing, and leave it with its owner.

**PR 3 ends here.** Title: *the targeting wave 4 does not build, specified where the next wave will find it*.

---

# Track D — the browser-driven pass

**Runs last, against everything the three tracks landed.** Lands as PR 4.

Static instruments in this repository reported zero problems throughout the two passes that found the worst defects, and were right both times. Both of wave 4's surfaces are moving renders; nothing else here can see them.

### Task 10: drive it (P1)

**The harness is not in this tree and that is a real cost to budget for.** `docs/qa/QA_HARNESS.md:187` says there is no browser driver, no Playwright and no `@testing-library/svelte`, and `src/lib/__auditbridge.js` — the console bridge the wave 2 pass used — was deliberately never committed (`docs/qa/audits/DESIGN-2026-09-16-WAVE2.md:52-62`). **The stage page needs none of it**: `:8032` is served by Relay itself and joins the real hub on `:8031`, so it can be driven with the browser driver alone. Only the console needs the bridge rebuilt from that audit's prose.

- [ ] **Step 1:** `npm run tauri dev`. The stage page and the output page against the real backend on `:8032`. `5032` is Vite and exists only in dev — never point a browser source at it.
- [ ] **Step 2: The scenarios, each stated as what a room would see.**
  - A programme timer crosses its warning threshold on the stage rail, with `prefers-reduced-motion` on and off.
  - A programme timer reaches zero with a done message, and with none.
  - A programme timer is held and released; the rail says which it is.
  - The programme rail is switched off and the reading takes the room back.
  - Six programme timers at once, at 1024×768 and on a phone in portrait: measure whether the row is still readable and what the floor did.
  - A stage tablet reloads mid-service and comes back to the timers and the reading, in that order, with no flash of a clock over the verse.
  - The slide sizer at each step, at the narrowest supported window, with its own width measured in the rail that once clipped `Compact` to `Compa`.
  - A bound cue goes on air and its timer appears on the preacher's rail; the same cue in a rehearsal reaches the stage tablet not at all.
- [ ] **Step 3: Measure, do not eyeball.** Geometry from `getBoundingClientRect`; clipping from a `Range` over the text node against the nearest ancestor whose computed `overflow` is not `visible`; an overlay proven with `elementFromPoint` over the control's centre.
- [ ] **Step 4: Record which claims were measured against the real backend and which against a mock.** They are different claims and the audit says which is which.

### Task 11: the evidence file (P1)

- [ ] **Step 1:** Write `docs/qa/audits/<DATE>-WAVE4-STAGE-PLANNER.md` — frozen evidence and findings only. Closures go in a fix log, never in the findings.
- [ ] **Step 2:** File every finding in `docs/qa/RELAY_GAP.md` at the next free ids; keep `relaygap.test.js` and `crossrefs.test.js` green.
- [ ] **Step 3:** Re-measure `docs/qa/QA_HARNESS.md` §0 from the runners' own summary lines.
- [ ] **Step 4:** Update `CLAUDE.md` only where this wave made an existing sentence false. **Do not add a wave 4 narrative to it.**

**PR 4 ends here.** Title: *the wave 4 browser-driven pass*.

---

## What this wave does not claim

- It does not move the release decision. Word error rate is unmeasured in every language, neither platform has a code-signing certificate (RG-73), and nobody but the author has run a service.
- It does not touch detection, the router, or any threshold.
- It does not build per-cue screen targeting, and says so in the spec and the register rather than in a commit message.
- It does not colour a taxonomy, in the Planner or in the Dock.
- It does not build the control that starts a programme timer. That is wave 3 Track E's, and it was in progress elsewhere while this plan was written.
- It does not change the congregation countdown's wire form, so nothing downstream of `countdown_*` is re-tested by this wave rather than trusted.
