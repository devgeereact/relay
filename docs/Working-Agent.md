# Working Agent — design brief

**Status: BUILT.** The five decisions in §8 were answered and the tooling exists. What
shipped:

| | |
|---|---|
| Shared Rust harness | `src-tauri/src/qa.rs` — `bare_app()` (a genuine first launch), `Wall` (Tauri events), `Kiosk` (the WebSocket door), `settle()`. `e2e.rs` now uses it |
| Surface inventory | `node scripts/qa-inventory.mjs` — controls, orphans, command map, create-path chain |
| The uncovered distinction | `src/lib/liveoutputrail.test.js` — 12 tests, 1 skipped known defect (§3) |
| Six agents | `.claude/agents/relay-qa-{cold-start,live-path,surface,detection,failure,auditor}.md` |
| The command | `/qa-audit` — changed-surface by default, `--full`, `--live` |
| The hook | `.claude/hooks/relay-fast-gate.mjs` — path-filtered, report-only (§5) |

Three real findings came out of building it, before a single agent ran. They are in §3
and §9.

This is my answer to "create an agent or hook that does a full autonomous QA / E2E / CRUD /
production-readiness audit of Relay". It keeps your standard — which is a good standard, and
harder than most teams apply to themselves — and changes the instrument, because the
instrument you described cannot exist in this repository. §0 explains why. If you disagree
with §0, stop there, because everything after it follows from it.

Companion documents:

- [Working-Agent-PROMPT.md](Working-Agent-PROMPT.md) — the actual agent prompts, ready to
  paste into `.claude/agents/`.
- [Working-Agent-COVERAGE.md](Working-Agent-COVERAGE.md) — what the existing 33 frontend test
  files and 14 e2e tests already prove, and where the holes are. The audit's starting line.

---

## 0. The one thing that has to change

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
- **The surface is 334 `<button>` occurrences across 47 `.svelte` files.** No agent in this
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

## 1. Five evidence layers

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

## 2. "No seed data" — your instinct is right, the target is different

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

## 3. The six distinctions — and what closing the sixth turned up

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

Full state of all six, with the file and test names, is in
[Working-Agent-COVERAGE.md](Working-Agent-COVERAGE.md).

---

## 4. Agent roster: 13 → 6

Your thirteen workstreams are the right *topics*. As thirteen agents they are a mistake here,
because eleven of them would be doing the same thing — reading the same source files — and
producing eleven independently-worded opinions about it. Parallel agents pay off when they hold
**different instruments**, not different subject headings. So: six, each owning a layer and a
question nobody else can answer.

| | Agent | Layers | Owns | Forbidden to claim |
|---|---|---|---|---|
| **R1** | Cold Start | A, C | The empty-system build: create-path matrix for all 18 tables, seed audit, persistence across a real reopen, first-run/onboarding order | That a screen "works". It never sees one |
| **R2** | Live Path | A, D | The six distinctions, the transport, panic, rehearsal containment, recovery after a kill, `NavResult` on every surface that exposes nav | That an operator *understood* anything. Legibility is R3's |
| **R3** | Surface Inventory | B, C | Every control in 47 files: enumerated, classified, mounted where mountable. Dead controls, missing empty/loading/error states, focus order, colour semantics, the humaniser on every error path | That any backend call succeeded. Its backend is a mock |
| **R4** | Detection & Language | A | Scoring **through the router**, never by reading the transcript. False positives, ambiguity, code-switching, the paraphrase-shows-no-percentage rule, and honesty about Yoruba numerals and unmeasured WER | Any claim about audio or accents. WER over speech is layer E |
| **R5** | Failure & Boundaries | A, D | Offline, process kill mid-service, poisoned locks, migration retry, concurrent writes, injection through text fields, unicode/Yoruba/emoji round-trips, and whether the LAN remote's *decided* threat model still holds | That an integration passes. OBS/ATEM/ProPresenter hardware is BLOCKED, always |
| **R6** | Independent Auditor | all | Runs last, reads none of R1–R5 until it has produced its own list, then reconciles and writes the report and the GO / NO-GO | Nothing. It is the one allowed to contradict the others |

Your Agent 8 (independent auditor reading nothing first) is the single best idea in the brief
and it survives intact as R6. Inherited false assumptions are the standard failure of
multi-agent review and that structure is the only known defence.

Two rules apply to all six, both yours, both kept:

- **No agent fixes anything during the audit.** Findings only. A fixing auditor stops auditing.
- **BLOCKED is a valid outcome and PASS is not its synonym.** Anything requiring hardware, a
  microphone, a notarized build, a second monitor, or a human eye is BLOCKED with the exact
  manual steps written out.

---

## 5. Agent or hook — your actual question

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

### What was built, and how to turn it on

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

## 6. What it produces

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

## 7. Build order — as executed

1. **Harness.** `src-tauri/src/qa.rs`: `bare_app()` (a genuine first launch, no convenience
   overrides), `Wall` (Tauri events), `Kiosk` (the WebSocket door), `settle()`. `e2e.rs` was
   refactored onto it and now holds only the *one* documented difference — the content-look
   override it needs to make its template assertion non-vacuous. Two harness self-tests keep
   the fixture honest: one asserts the fresh install has no content-look chosen except the
   deliberate `tpl_song`, the other asserts the kiosk is a genuinely different door from the
   wall. 432 Rust tests pass, `fmt` and `clippy -D warnings` clean.
2. **The Preview ≠ Programme gap.** Closed on the surface that ships — see §3.
3. **Six agents plus `/qa-audit`.**
4. **The hook** — §5.

---

## 8. Decisions — answered

| | Question | Decided |
|---|---|---|
| **8.1** | Where the agents live | **Committed.** `.gitignore` un-ignores exactly the eight QA files and nothing else; the claude-flow scratch stays out |
| **8.2** | Default scope | **Changed-surface**, computed from `git diff --name-only main...HEAD`; `--full` for a release |
| **8.3** | May it drive the running app? | **Yes, behind `--live`.** R2 and R5 get layer D only when the flag is passed, and are told when it is off |
| **8.4** | Report location | **Committed**, `docs/audits/QA-<ISO date>.md` |
| **8.5** | Does the hook block? | **Report only.** It never exits 2 |

---

## 9. Findings from building the tooling

Three, none of which needed an agent — which is itself the argument for building the
instruments before running the audit.

| | Finding | Severity | Decision | Evidence |
|---|---|---|---|---|
| **F1** | **Amber lied while staging.** A staged slide rendered in the monitor with an amber "Live" badge beside it, because the badge read `$live` and the pane read `preview` | **P1** — the one control that answers "what are they looking at" disagreed with the wall | **FIXED**, option (b) — §3 | `src/lib/liveoutputrail.test.js`, 17 tests. Failed before the fix with `expected 'r-badge amber' not to match /amber/` |
| **F2** | **`PreviewProgram.svelte` was orphaned** — 312 lines, safety-critical by its own comment, imported by nothing | **P3** as dead code, but it caused an F1-shaped near-miss during this build: fourteen green tests against a screen no operator can open | **DELETED.** The single-pane rail is a deliberate design evolution (`LiveOutputRail.svelte` says so: "two buttons, one action, and a row of height the transcript needed more"), so the two-pane version is superseded, not pending. It is in git if it is ever wanted back; leaving it in the tree only invites the next person to fix a bug in a component nobody renders | orphan count now 0 |
| **F3** | **`song_arrangements` has no create path.** `save_arrangement` is registered, `saveArrangement` exists in the store, no component imports it — a user cannot save a song arrangement | **P2** — a built feature no user can reach | **RECORDED, not built.** An arrangement editor is a feature, and shipping one in the same session as a QA harness is how both get done badly. CLAUDE.md's "no dead-but-built commands" claim has been corrected to state the exception rather than hide it; the work belongs in ROADMAP | `node scripts/qa-inventory.mjs` → create-path table |

The rule these three share: **each was found by an instrument, not by an opinion**, and each was
invisible to the 850-test suite that already existed.

---

## 10. Where these docs sit

Added to [docs/README.md](README.md)'s index under engineering tooling. They are not part of
the specification hierarchy — SPEC, DECISIONS and PRODUCT_AUDIT own the product; these three
own how it gets audited.
