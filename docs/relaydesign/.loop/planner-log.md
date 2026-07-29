# Service Planner (§8) — design loop log

Reference: `docs/relaydesign/relay-planner-screen.png` (a dedicated hi-fi mockup,
not the panel-7 crop the loop prompt originally pointed at).

Gate: `npm run build` clean (no CSS warnings), **263 frontend** (+16),
**340 Rust** (+5), `cargo fmt` + `clippy -D warnings` clean, detection scorecard
unchanged (100% recall, 0 wrong verses).

Compare: **pixel**, 7 iterations at 1536×1024 plus four state variants. Captured
from the Vite webview with a stubbed `window.__TAURI_INTERNALS__.invoke` — the
console has no backend in a plain browser, so the plans list would otherwise be
empty and there would be nothing to compare. **No app code was changed for the
capture**; the stub lives entirely in the rig.

Screens: `planner-0.png` (baseline, before) → `planner-7.png` (final), plus
`planner-slides.png`, `planner-notes.png`, `planner-add.png`,
`planner-narrow.png` (1280×800 booth laptop).

---

## What was missing

The Planner was a two-step screen: a full-page grid of plan cards that you
navigated *into*, then a two-column editor (cue rail + slide flow). The reference
is a three-column workspace — plans rail · running order · cue inspector — and
carries two things the data model simply did not have:

- **Sections.** `plan_items` was a flat ordered list with no grouping.
- **Durations.** No per-cue length, so no running-time estimate.

Both were built (approved as full scope, not frontend-only).

## Backend

New columns on `plan_items`, added by `ensure_service_plans`:

- `section_title TEXT NOT NULL DEFAULT ''` — a **non-empty value means this cue
  BEGINS a section**, which runs until the next cue that has one. Grouping is
  therefore *derived from the same ordered list the transport walks*, so
  drag-reorder, `move_plan_item` and `stepFrom` need no section awareness at all,
  and a section can never desynchronise from the cues it claims to contain. That
  is the whole reason it is not a second table.
- `duration_sec INTEGER NOT NULL DEFAULT 0` — 0 means **untimed**, which is the
  normal case for scripture (it fires when the preacher reaches it, not on a
  clock).

New: `set_plan_section`, `set_plan_duration`, `set_plan_template` (commands +
`db::` functions + `capture.js` wrappers). `duplicate_plan` now copies both new
columns — dropping them would silently gut the operator's main reason for
duplicating last week's order. Both columns are registered in `schema_report`, so
the Database Migration screen actually verifies them.

**The migration is retryable**, per CLAUDE.md §25 — but the failure mode one
layer down from §25: a bare `ALTER TABLE … ADD COLUMN` errors with *"duplicate
column name"* on the second run, and this runs on **every boot**, so it would
have panicked the app at startup forever. `add_plan_item_column` asks
`pragma_table_info` first. Deliberately a plain ADD COLUMN and **not** a table
rebuild — the rebuild path is the one that stranded a scratch table and bricked
every subsequent boot.

`ensure_service_plans_is_retryable` was verified the hard way: removing the
pragma guard makes it fail with the real
`SqliteFailure(… "duplicate column name: section_title")`, not merely pass by
accident.

Five new Rust tests, all written against the failure and not the fix:
retryability, a pre-sections DB gaining the columns without losing rows, section
heading **inherited by the next cue** when its first cue is deleted, that
inheritance **not clobbering** an existing heading below, and duration clamping +
duplication.

## Frontend

`sectionsOf`, `planRuntime`, `fmtDuration`, `parseDuration` added to `plan.js`
(pure, no Svelte, no backend) with 11 new tests. `ServicePlanner.svelte` rebuilt
as the three-column shell; the plans list is now a persistent rail, so comparing
last week's order with this week's is one click instead of three.

Notable: **`planRuntime` reports `partial`**, and the header renders "(est.)"
whenever any cue is untimed. Most real plans contain scripture, so most totals
are a floor — presenting a partial sum as the service length is how a service
runs long.

## Departures from the reference — and why

**Refused, because CLAUDE.md forbids them** (the rule wins over the mockup):

- **"Test on Outputs"** (inspector) and the **Rehearsal / Live** toggles
  (toolbar). `ServicePlanner` *cannot fire to an output* — building is a Tuesday
  job, running is a Sunday one, and an operator arranging next week's songs must
  not be able to put one on the wall by clicking the wrong thing. The sanctioned
  path is the existing `runPlan` handoff, kept as **Run in Live**. The toolbar
  states the constraint in words: *"Build only — never reaches an output."*
- **Per-cue "Auto fire when detected" + "Confidence threshold"** (inspector
  BEHAVIOR block). Rule 10 caps semantic/ambiguous at Suggest **at any score**,
  and there is exactly ONE threshold baseline *by construction*. A per-cue
  threshold is a second one, and a per-cue auto-fire toggle invites raising a
  paraphrase to auto-fire. Not built, not greyed out — a disabled control still
  advertises a capability that does not exist. (Same treatment as OBS/ATEM in §2.)

**Not built, no backing data:**

- **OUTPUTS column** ("All Screens", "Stage, Lobby"). There is no per-cue output
  routing in the model — a cue broadcasts to every channel, and the only per-cue
  override is the template. A column that reads "All Screens" on all 14 rows is
  noise; one that implies per-cue targeting is a lie. Dropped.
- **"Auto Arrange"** — no such feature exists.
- **"Import"** in the header — ProPresenter import exists, but it imports *songs*
  into the Library, not a plan. Wiring it here would misdescribe it.

**Substituted:**

- **STATUS → TRIGGER.** The reference's `UP NEXT` / `PENDING` / `AUTO` badges are
  **run states**, and this screen cannot know them: on a Tuesday nothing is up
  next and nothing is pending. What *is* true at build time is how each cue will
  be triggered, which is what the column now shows (`AUTO-DETECT`,
  `SUGGEST-ONLY`, `MANUAL/TIMER`, `MANUAL/LOOP`) — read from the existing `TYPE`
  table rather than invented.
- **Inspector tabs General · Outputs · Conditions · Notes → General · Slides ·
  Notes.** "Outputs" has no per-cue data (above); "Conditions" is §16 Automation,
  which does not exist. "Slides" replaces the old centre column and is real.
- **Type label reads `NOTICE`, not `ANNOUNCEMENT`** — `TYPE` in `plan.js` is
  shared with Live, and renaming it for one screen would split the vocabulary the
  operator sees between build and run.
- Selected row is **amethyst**, not amber. Amber means on air; a cue being edited
  on a Tuesday is not.

## Iteration notes — what the pixels caught that the code did not

1. **Templates never loaded.** Every cue read "Template 4": the Planner names a
   cue's template and offers the picker, but nothing called `loadTemplates()` on
   this tab, so the store was empty on a cold open. A real bug, invisible in the
   markup. Fixed in `onMount`.
2. **The table overflowed and clipped the row buttons**, and ran the DURATION and
   TRIGGER headings together. The column count was keyed to the *viewport*, but
   what decides how many columns fit is the table's own width — viewport minus
   ~814px of fixed chrome (sidebar + rail + inspector). Rewritten with
   breakpoints derived from that.
3. **`@container` was the right tool and had to be abandoned.** It expresses the
   above directly, but **esbuild's CSS minifier cannot parse it** and emitted
   broken rules with only a warning: dev looked correct, the packaged build would
   not have been. Reverted to media queries. Worth remembering — this is a
   CSS-level instance of the same class as the CSP trap (`tauri dev` does not
   exercise it).
4. **The preview rendered as a dead black box.** Two causes, found by probing the
   DOM rather than guessing: `TemplateRender`'s root is `position:absolute;
   inset:0`, so without a positioned ancestor it escaped the preview box
   entirely; and the capture rig's stub templates had `layout:{}`, while
   `TemplateRender` renders `layout.regions` — so it correctly drew *nothing*.
   The first was a real bug (`position:relative` added, with a comment, since it
   is load-bearing and looks removable); the second was stub data no real row
   looks like, and the rig now mirrors the seeded shapes from `db/templates.rs`.
5. **Cue names were truncated to "Pre Se…", "Great …".** Rows carried a subtitle
   restating the cue type that the TYPE column already shows — it doubled the row
   height and squeezed the one thing an operator scans for. Now single-line
   unless the cue has a stage note.

## Still different, and left that way

- `Scripture - Classic Serif` truncates in the TEMPLATE column on two rows.
  Widening it further would cost the cue-name column, and the inspector shows the
  full name for the selected cue. Accepted.
- The reference shows small per-type glyphs in the TYPE column; this uses the
  colour-coded dot already used elsewhere in the app.
- The reference's own section totals do not match the sum of the cue durations it
  prints (its "Welcome & Worship" reads 25:00 over cues of 2:00 + 4:45 + 6:15).
  Ours are computed, so they differ from the mockup's numbers by design. Sections
  that contain an untimed cue render `13:00+` — the `+` says the total is a floor.
- The reference puts the scripture reference *above* the verse in the preview;
  the real seeded "Classic Serif" template is `refFirst:false`. That is template
  configuration, not code — "Stage Mono" is ref-first — so it was left alone.

## Not verified here

Everything above is the Vite webview under a stubbed IPC layer. Not exercised:
the real SQLite round-trip for the new columns beyond the Rust tests, and the
packaged-build CSP. The migration has never run against a real
`~/Library/Application Support/com.relay.app/relay.db` — only in-memory fixtures.
