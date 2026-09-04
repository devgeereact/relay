---
description: Run Relay's autonomous QA audit (changed surface by default; --full for everything, --live to drive the running app)
argument-hint: "[--full] [--live] [<path or area>]"
allowed-tools: Read, Grep, Glob, Bash, Write, Edit, Agent
---

# Relay QA audit

Arguments: `$ARGUMENTS`

Run the audit described in `docs/qa/QA_HARNESS.md`. You are the orchestrator; the six
`relay-qa-*` agents do the work. **You do not audit anything yourself** — an
orchestrator that starts investigating stops orchestrating, and its opinions then leak
into R6's control group.

## 1 · Read the plan

Read, in this order, before dispatching anything:

- `docs/qa/QA_HARNESS.md` — the five evidence layers and the roster.
- `docs/qa/QA_HARNESS.md` Part 2 — the shared preamble the agents inherit.
- `docs/qa/QA_HARNESS.md` Part 4 — what is already pinned. An audit that re-derives
  this burns a run rediscovering deliberate decisions, and may "find" a fixed bug.

## 2 · Decide the scope

**Default: changed surface.** Compute it, do not guess:

```bash
git diff --name-only main...HEAD ; git status --porcelain
```

Map the changed paths to agents:

| Changed | Dispatch |
|---|---|
| `src-tauri/src/db/**`, any `ensure_*`, a create/save/import command, `models.rs`, `src/lib/boot/**`, `FirstRun.svelte` | **R1** cold-start |
| `main.rs` fire path, `pipeline.rs`, `router.rs`, `channels.rs`, `shortcuts.js`, `cues.js`, `plan.js`, `LiveOutputRail.svelte`, `Live.svelte` | **R2** live-path |
| any `.svelte`, `errors.js`, `src/lib/ui/**`, `themes.js`, `templates.js` | **R3** surface |
| `detection.rs`, `eval.rs`, `router.rs`, `detect.js`, `data/book_aliases.json` | **R4** detection |
| `audio.rs`, `stt.rs`, `dsp.rs`, `telemetry.rs`, `proimport.rs`, `crash.js`, `session.js`, `remote_api` | **R5** failure |

**`--full`** dispatches all five regardless. Use it before a release, or after a merge
big enough that the diff is not a useful signal.

**`--live`** enables layer D for R2 and R5: they may run `npm run tauri dev` and drive
`GET :8032/api/*` and the kiosk hub on `:8031`. Without it, tell them layer D is
disabled — do not leave them to discover it and improvise.

If an explicit path or area was given in the arguments, that overrides the diff.

**R6 always runs**, at any scope. An audit with no independent pass is five opinions.

## 3 · Dispatch

R1–R5 run **in parallel** — one message, multiple `Agent` calls. They have different
instruments and no dependency on each other.

Each prompt states: the scope, whether layer D is enabled, and the explicit instruction
**not to fix anything**. Nothing else — the agent definitions carry their own mandates,
and paraphrasing them here creates a second source of truth that will drift.

Collect the reports. Do not summarise, edit, or reconcile them; that is R6's job and
pre-digesting them destroys what R6 is for.

## 4 · Then R6, alone

Dispatch R6 **after** the others have finished, with their reports attached and with the
instruction it already carries: **do your own pass first, read theirs second**. If R6
reads first, the run has produced five opinions and an echo.

## 5 · Report back

R6 writes `docs/qa/audits/QA-<ISO date>.md`. In the terminal, give the human:

- the release decision (GO / GO WITH CONDITIONS / NO-GO) and the one sentence behind it,
- every P0 and P1, in full,
- the count of everything else by severity,
- the BLOCKED list, because that is the work only they can do,
- the path to the report.

Do not print the whole report to the terminal. It is a document; they will read it.

## Non-negotiables for this run

- **No agent fixes anything.** If a fix is obvious and small, it goes in the report as a
  recommendation with the diff sketched, not applied. The human decides what to do
  during a week they are not about to run a service.
- **BLOCKED is never PASS.** If a run comes back with no BLOCKED items on a desktop app
  that cannot be seen, screenshotted, or heard on this machine, the run is wrong.
- **Nothing writes to `docs/qa/audits/PRODUCT-2026-07-13.md`.**
