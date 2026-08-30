# Relay — the QA harness

How Relay is audited, by whom, with which instrument, and what the repository can already
prove. This is not part of the specification hierarchy — [SPEC.md](SPEC.md),
[DECISIONS.md](DECISIONS.md) and [PRODUCT_AUDIT.md](PRODUCT_AUDIT.md) own the product; this
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

Re-measured **2026-08-29** against `0338244`. Every number here is produced by a command, and
the command is named — a count you cannot reproduce is a rumour.

| | Count | How to reproduce |
|---|---|---|
| Rust tests | **623 passing**, 18 ignored | `cd src-tauri && cargo test` |
| Frontend tests | **874 passing**, 0 skipped, 63 files | `npx vitest run` — read the runner's own summary line. **Not** `vitest list \| wc -l`: that stream carries Svelte compiler warnings too and over-counted by 7 |
| `e2e.rs` tests | **35** (35 run, 0 ignored — R2-C and R2-D closed, DECISIONS §54; three added for the calibrator and the service record) | `cd src-tauri && cargo test e2e::` |
| Registered `#[tauri::command]` | **137** | `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` |
| `.svelte` files | **47**, 22 of them views | `find src -name '*.svelte' | wc -l` |
| `<button>` occurrences | **338** | `grep -ro '<button' --include='*.svelte' src | wc -l` |
| Tables in the schema | **21** | `grep -c 'CREATE TABLE' docs/data/schema.sql` |

**Status: BUILT.** What shipped:

| | |
|---|---|
| Shared Rust harness | `src-tauri/src/qa.rs` — `bare_app()` (a genuine first launch), `Wall` (Tauri events), `Kiosk` (the WebSocket door), `settle()`. `e2e.rs` uses it |
| Surface inventory | `node scripts/qa-inventory.mjs` — controls, orphans, command map, create-path chain |
| Six agents | `.claude/agents/relay-qa-{cold-start,live-path,surface,detection,failure,auditor}.md` |
| The command | `/qa-audit` — changed-surface by default, `--full`, `--live` |
| The hook | `.claude/hooks/relay-fast-gate.mjs` — path-filtered, report-only (§1.6) |

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
- **The surface is 319 `<button>` occurrences across 47 `.svelte` files.** No agent in this
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

- 31,100 KJV verses and their translation row (bundled, `include_str!`, required to build)
- 5 built-in templates plus the presets
- the default output channels
- one active voice profile

That is **product content**, not demo data. A church with an empty verse table has a broken
install, not a clean one. Deleting it and demanding the UI recreate it would be testing a
requirement that does not exist.

The version of your question that *does* apply, and applies hard:

> **Which of the 18 tables in `docs/data/schema.sql` can only be filled by the seeder or by an
> importer — with no path a new user can reach from a rendered control?**

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

**First: the component that reads like the safety model is not in the product.**
`src/lib/views/library/PreviewProgram.svelte` is 312 lines of two-pane switcher whose header
comment states the danger exactly — *"Relay used to fire on a single click. One slip of a
trackpad put the wrong scripture on a wall in front of a congregation, instantly, with no
undo"* — and **nothing imports it**. Fourteen tests were written against it and passed before
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

**FIXED — option (b).** The badge now describes the **pane**: staged → grey "Preview", and
amber is reachable only when the pane is showing content a congregation can actually see. The
fact the badge can no longer carry rides in a second, deliberately smaller chip (`.lo-behind`):
**"Wall live"** in amber, or **"Wall: rehearsal"** in amethyst.

Option (a) — badge-only — was rejected. It trades a wrong signal for a **missing** one on the
single question this panel exists to answer, and staging is precisely the moment an operator
forgets what is still up. A panel titled LIVE OUTPUT that goes quiet about the wall the instant
you touch the library is not more honest than one that shouts the wrong thing; it is quieter
about the same failure.

The chip is a warning, not decoration, so it stays absent when the wall is genuinely clear and
when the screens are blacked out. `src/lib/liveoutputrail.test.js` is now **17 passing tests,
none skipped**, five of them on this state alone.

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

Written to `docs/audits/QA-<ISO date>.md`. It never touches `PRODUCT_AUDIT.md` — that document
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
| **8.4** | Report location | **Committed**, `docs/audits/QA-<ISO date>.md` |
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

- **No native SDI.** NDI + HDMI only; bridging hardware covers the rest.
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
| **R1** | Cold Start | A, C | The empty-system build: create-path matrix for all 18 tables, seed audit, persistence across a real reopen, first-run order, migration retryability | That a screen "works". It never sees one |
| **R2** | Live Path | A, D | The six distinctions, the transport, panic, rehearsal containment, recovery after a kill, `NavResult` on every surface that exposes nav | That an operator *understood* anything. Legibility is R3's |
| **R3** | Surface Inventory | B, C | Every control in 47 files: enumerated, classified, mounted where mountable. Dead controls, missing empty/loading/error states, focus order, colour semantics, the humaniser on every error path | That any backend call succeeded. Its backend is a mock |
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
| **A — Command E2E** | Yes. 32 tests (32 run, 0 ignored) driving the real commands against a real in-memory DB through the real router and pipeline | `src-tauri/src/e2e.rs` |
| **B — Component mount** | Yes, and used — but only twice | `src/lib/inspector.test.js` mounts `DetectionInspector`; `src/lib/layers.test.js` mounts `TemplateRender` |
| **C — Static contract** | Yes, one exemplar | `src/lib/ipc.test.js` — command names both directions, event listeners, and a `greet`-has-one-caller assertion |
| **D — Live app** | Exists as a surface, is not exercised by any test | `channels.rs` serves `:8032`; `main.rs::remote_api` handles `search / fire / next / prev / clear / black / live`. Kiosk hub on `:8031` |
| **E — Human** | The bench harness is built and pointed at nothing | `bench/README.md` says what to record; `bench/.gitignore` refuses to let sermon audio into the repo |
| **F — Real-time latency** | Built 2026-08-24. Nine stamps per decode pass, mic → projector, readable in the shipped app | `src-tauri/src/latency.rs`; the rig is `stt::realtime::live_transcript_latency`; the surface is Settings → Diagnostics |

**Layer F is new and it is the only layer that measures TIME.** Every other instrument here
answers "is the answer right"; none of them could answer "how long did it take", and for two
releases running that was the complaint. It is deliberately a shipped surface, not a test
fixture: the numbers that matter are produced by a church laptop in a church, and an instrument
that needs `cargo` is one nobody in a church will ever run. See
[`audits/PERF-2026-08-24.md`](audits/PERF-2026-08-24.md) for what it has and has not measured,
and Stage F of the human test script for the part that needs a room.

Totals are in §0 and are re-measured, not inherited.

Layer B is the biggest under-used asset in the repo. The pattern works, it is proven twice, and
it is the only instrument that can see a control at all.

---

### 4.2 The six distinctions

| Distinction | Pinned? | Evidence |
|---|---|---|
| **Rehearsal ≠ Live** | **Yes, both doors** | `e2e.rs::nothing_reaches_the_congregation_during_a_rehearsal` and `::nothing_reaches_the_stage_monitor_during_a_rehearsal` — the second exists because the first watched Tauri events and was therefore blind to `channels::stage_next`, which publishes to the kiosk and emits nothing. Frontend side: `rehearsal.test.js` (off by default; throws rather than lying when the backend refuses; a nonsense answer counts as NOT rehearsing) |
| **Suggestion ≠ Auto-fire** | **Yes** | `router::decide` caps Semantic and Ambiguous at `Suggest` by construction; `e2e.rs` drives a paraphrase at maximum confidence and asserts it cannot reach the wall |
| **Paraphrase ≠ Direct** | **Yes** | `detect.test.js`: a spoken reference is HEARD, a paraphrase is not "however high its score", the three methods get three distinguishable keys, and *"a paraphrase NEVER shows a percentage — at any score"* |
| **Clear ≠ Blackout** | **Yes, as separate contracts** | `panic.test.js`: `clearScreens` returns FALSE on backend failure and the caller must not flash success; a failed clear raises the panic banner; *"blackout has the identical contract — it is a panic control too"*; a success clears a stale warning; no crying wolf with no backend at all |
| **Cued ≠ On Air** | **Yes** | `transport.test.js`: Esc/clear takes the plan off air but REMEMBERS the position; blackout the same; a FAILED hand-fire leaves the plan exactly as it was; clearing twice is idempotent and does not lose the position |
| **Preview ≠ Programme** | **Yes, now** | `src/lib/liveoutputrail.test.js` — 17 tests, none skipped: staging reaches nobody, TAKE hands the slide to the parent and fires nothing itself, TAKE is dead with nothing staged / in safe mode / mid-take, the monitor is honest in every state, amber never sits beside a staged slide, and the operator is still told when the wall is hot behind one |

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
| 31,100 KJV verses + the translation row | Bundled at `src-tauri/data/kjv.json` via `include_str!`, required to build. A church with an empty verse table has a broken install |
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
