> **Archived 2026-09-21.** Historical work order. Wave 5 landed 2026-09-16 and was audited in `docs/qa/audits/DESIGN-2026-09-16-WAVE5.md`; findings RG-156 to RG-160. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Wave 5 — the shelf, the names and the seal: implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development` or `superpowers:executing-plans` to work this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking. This repository has never checked a box in a landed plan — completion is evidenced by commits and green suites, not by boxes.

**Goal:** answer the four questions the design opens with. What a church finds on the shelf becomes forty layer-model templates with a stable identity the legacy conversion cannot destroy; one concept carries one name everywhere; the operator console's own skin stops at the glass; and four controls that lie about what they do stop lying.

**Spec:** `docs/archive/2026-09-16-shelf-names-and-seal-design.md`, approved by the operator on 2026-09-16 before any code was written.

**Architecture:** seven code tracks and one browser-driven verification pass. Each lands as its own branch merged into `feat/wave5-impl`, which branches from `feat/wave5-shelf-names-seal` (waves 0, 1 and 2 plus the wave 5 design doc). Wave 3 is in flight on `feat/wave3-timers` in other worktrees and **is not touched by this wave**: wave 3 branches from the same base, keeps `countdown_*` as the wire form, and every template this wave seeds survives it unchanged.

**Tech stack:** Rust (Tauri v2, `rusqlite`), Svelte 4 + Vite, vitest + jsdom, `cargo test`, Chrome through the browser audit harness.

## Track map

| Track | Branch | Scope | Depends on |
|---|---|---|---|
| A | `feat/wave5-track-a` | the shelf becomes forty; `seed_key` + `edited_at`; retirement that survives conversion | — |
| B+F+H | `feat/wave5-track-b` | Create New Template stops creating one; the three editor layout defects; the "Used for" block leaves the editor | — |
| C | `feat/wave5-track-c` | `output_channels.role`; Live stops guessing the main screen; `stage_message` binding filtered at the receiver | — |
| D | `feat/wave5-track-d` | the name register (Stage Message · Stage Note · Up Next) and `names.test.js` | — |
| E | `feat/wave5-track-e` | the seal: template data, renderer fallback, and `app.css` shipping to the congregation | — |
| G | `feat/wave5-track-g` | the timer runs bare; the Planner gains the fields it never had | — |
| I | `feat/wave5-track-i` | starter content on a fresh install; the tripwire asserts identity instead of zero | **A** |
| J | `feat/wave5-track-j` | the browser-driven pass against the real backend | all of the above |

Tracks A, B+F+H, C, D, E and G are independent of each other and run in parallel. Track I lands after A, because its example plan pins a template by `seed_key`. Track J runs last.

## Global constraints

Every one of these is a rule this repository learned the hard way. A track that appears to need an exception has found a design problem, not a rule problem.

- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` clean before every commit; both suites green before a track is offered for merge.
- Run `npm run build` before `cargo test` on a fresh tree (RG-127): two `channels` tests fail on a bare 404 otherwise, because `dist/` is gitignored.
- Read test counts from **the runner's own summary line**, never a grep. Values live only in `docs/qa/QA_HARNESS.md` §0. Re-measure; do not restate.
- Every new test is verified to **fail when its defect is reintroduced**. Test the bug, not the fix.
- **Rule 10, 28, 30, 34:** no threshold moves in this wave; nothing here touches `detection.rs` or `router.rs`.
- **Rule 15 and 44:** nothing added here may paint over `Clear screens`, and any overlay that disarms `Esc` consumes it and gives it back on the second press.
- **Rule 41:** no native `confirm()` / `alert()` / `prompt()`. The unsaved-draft guard in Track B is an in-app two-step.
- **Rule 36:** `pipeline::Fire` stays the only way to build output; `broadcast_with_clock` stays the only caller of `channels::broadcast_content`; the pre-air validator stays where it is.
- **Rule 37 and 42:** the 45% legibility floor stays a ratio of the designer's own size, `TemplateRender` keeps shrinking rather than blanking, and the two-try refit bound stays.
- **Rule 43:** no frame introduced here is retained into `last_screen`, and `is_screen_frame` is a `contains`, not a `starts_with`.
- **Rule 25:** every migration is retryable and idempotent — a second run is a no-op and a third is identical to the second.
- **Rule 18:** a congregation screen never inherits the console's ON AIR amber.
- **DECISIONS §29, §35, §70, §78 are not reversed.** A template's role stays derived from what it renders; the kiosk hub still records nothing about who connected.
- `src/lib/errors.js` is the ONE backend-error humaniser. Never render a raw Rust `Err` string to a volunteer.
- Commit messages, PR bodies and documentation are normal English prose, never caveman.
- Never commit to `main`. Each track is a branch merged into `feat/wave5-impl`.

## Corrections to the design, verified against the working tree on 2026-09-16

Every structural claim cited below was checked by reading the code in this tree. None was substantively wrong; three citations have drifted and one is incomplete in a way that changes a track's scope.

1. **The template components moved.** They are `src/lib/views/templates/TemplateGallery.svelte` and `src/lib/views/templates/TemplateEditor.svelte`, not `src/lib/views/`. `newFrom` is `TemplateGallery.svelte:324` (the save is at `:329`). `armedDelete` renders at `TemplateEditor.svelte:1140-1144`.
2. **"Used for" has THREE writers, not two.** `toggleUsedFor` exists at `TemplateEditor.svelte:535` *and* at `TemplateGallery.svelte:399` (rendered at `:780`), beside the authoritative matrix in `Channels.svelte`. Track H's design text names only the editor. **Decide the gallery's copy explicitly and say which way you went in the commit body** — do not delete it silently, and do not leave it unexamined. The editor's block goes, per the design; the gallery's is a finding this plan is filing, not a licence to widen scope by reflex.
3. **Live's guess is where the design says it is:** `Live.svelte:1357`, `channels.find((c) => c.render_target === 'native_window') ?? channels[0] ?? null`.
4. **The dock's hard-coded label is `Dock.svelte:625`**, and the defaults behind it are `capture.js:1431-1432` (`label = 'Service begins in'`, `doneMsg = 'Welcome'`). The Planner's copy is `ServicePlanner.svelte:289`, inside `addCountdownCue`, which already seeds `setPlanDuration(added.id, m * 60)` — leave that alone.
5. **The seed tripwire is real and load-bearing:** `db/mod.rs:662` carries the "NOTHING SEEDS DEMO CONTENT HERE" comment, and `preset_template_count()` is `db/templates.rs:708`, asserted at `db/mod.rs:1020` and `:1053` and `db/templates.rs:1473`. Track I reverses the comment's rule and Track A moves the count; both update every assertion in the same commit that changes the thing it asserts, never afterwards.
6. **Numbering:** the last `DECISIONS.md` section is 88 and the highest register id filed is RG-142. The starter-content reversal and the channel-role decision each take the next free section number; anything filed takes the next free RG id. **Write the citation in the same commit that writes the section** — `crossrefs.test.js` resolves every `DECISIONS §N` and every `RG-` id against the real documents, so a citation written ahead of its section turns the suite red.
7. **RG-139 is not closed by this wave and must not be claimed as closed.** Track A seeds six templates with a `countdown`-bound layer, which widens that row's reach. Track A says so in the register when it lands.

## Track A — the shelf becomes forty

- [ ] **Step 1:** add `templates.seed_key TEXT` and `templates.edited_at TEXT` in one retryable migration that back-fills `seed_key` by name against the OLD seed lists, declining on a name collision.
- [ ] **Step 2:** `upsert_template` never rewrites `seed_key`, and stamps `edited_at` on any save that does not come from the seeder.
- [ ] **Step 3:** retirement becomes `seed_key` in the retired set AND `edited_at IS NULL` AND unreferenced through the four doors `ensure_retired_presets_are_gone` already checks. `retired_presets.json` and its byte comparison stay, additively, for installs that never opened Templates.
- [ ] **Step 4:** seed the forty — eight roles × five, every one layer-model, every one declaring `layout.shows` explicitly, every one clearing 7:1 body contrast against its own ground, every name unique and prefixed `Role · Name`.
- [ ] **Step 5:** stop seeding the region-model rows. A test asserts **no seeded row is region-model**, which is what keeps RG-141 closed against a later tidy-up.
- [ ] **Step 6:** evidence — `preset_template_count()` returns 40 with the assertion beside it; `the_bare_fixture_is_a_first_launch_and_nothing_more` updated in the same commit; `templateKind.test.js` names the derived role of all forty; the contrast instrument walks all forty; a retirement fixture that **converts before it retires**.
- [ ] **Step 7:** file RG-139's widened reach in `docs/qa/RELAY_GAP.md`.

## Track B+F+H — the editor stops lying

- [ ] **Step 1 (B):** `newFrom` builds the starter in memory and dispatches a draft `{ id: null, name, layout, style }`. The editor renders a draft as it renders a saved row, with an unsaved header, a Save that inserts, and a Discard that writes nothing.
- [ ] **Step 2 (B):** leaving a dirty draft raises an in-app two-step, never `confirm()`. Duplicate and import keep today's behaviour.
- [ ] **Step 3 (B):** a test mounts the gallery, chooses a starter, closes without saving and asserts `save_template` was never called — written to fail against today's code first.
- [ ] **Step 4 (F):** the armed layer delete gets styling and room. `Sure?` must not land in a 20px box inside a clipped, horizontally scrolling list.
- [ ] **Step 5 (F):** `.te-objtabs` stops growing unbounded inside `.te-pane{overflow:hidden}`; the add-layer menu's absolute positioning is measured and, if it clips, moved to `position:fixed` the way `TemplateGallery` already did.
- [ ] **Step 6 (H):** delete `toggleUsedFor` and the "Used for" block from the editor. **Keep** the row beneath it — `layout.shows`, a per-screen allow-list, is a different feature. Rule on the gallery's third copy explicitly (correction 2).
- [ ] **Step 7:** nothing in the engine changes: `tpl_{kind}`, `set_content_template`, `ContentTemplates`, `cue_or_content_tpl` and `resolveOutputTemplate` are untouched.

## Track C — channel roles and the door Stage Message needs

- [ ] **Step 1:** `output_channels.role TEXT CHECK (role IS NULL OR role IN ('main','stage'))`, seeded `Main screen` → `main`, `Stage display` → `stage`, others NULL. At most one `main`, enforced in the helper so the error is readable.
- [ ] **Step 2:** Live reads the role instead of guessing, and the programme pane names the screen it is rendering. Outputs gains the picker.
- [ ] **Step 3:** a `stage_message` binding in `layers.js`, labelled **Stage Message**.
- [ ] **Step 4:** `Output.svelte` accepts a `stage_alert` frame **only when its own channel's role is `stage`**. The value never rides on `OutputContent`; it stays its own frame kind in renderer state. `FRAME_VERDICTS` keeps `("stage_alert", false)`. The rehearsal gate is unchanged.
- [ ] **Step 5:** `e2e::r5_a_word_to_the_preacher_reaches_no_congregation_channel` gains a sibling driving a channel whose role is **not** stage, asserting the frame paints nothing.
- [ ] **Step 6:** settle and write down whether `alert` surviving a panic control on the stage page is intended (`Stage.svelte:406-438` resets six fields and not that one). The behaviour need not change; it must stop being an accident.

## Track D — the names

- [ ] **Step 1:** one name per concept — **Stage Message** (`stage_alert`), **Stage Note** (`stage_note`), **Up Next** (`stage_next` / `next_*`) — applied to the dock card heading and aria, the binding labels, the Planner field, the stage page zones and any status line.
- [ ] **Step 2:** `names.test.js` holds, per concept, the single user-visible string and the files allowed to contain it, failing when a second label for the same concept appears anywhere in `src/`.
- [ ] **Step 3:** the scanner reads every `src/` file rather than a hand-written list, **plus a test that the scanner can still see a known instance** — a scanner that quietly narrows passes everything, and this repository has had that exact failure twice.
- [ ] **Step 4:** neither of the first two concepts becomes the other. One is persisted per cue, one is a live instruction.

## Track E — the seal

- [ ] **Step 1:** seeded `style_json` stops storing `var(--f-serif)` / `var(--f-display)` / `var(--f-body)`. Templates name real families. (Coordinate with Track A, which authors the forty: A owns the new rows, E owns any legacy row and the rule that keeps it true.)
- [ ] **Step 2:** `TemplateRender`'s `--accent` fallback stops being `var(--v-amber)` and becomes a template-side default.
- [ ] **Step 3:** the legacy unscoped rules in `app.css` (`.prev-main .verse`, `.prev-stage .verse`, `.prev-stream .lower-third`, `.prev-lobby .verse`, `.tmpl-row.active`, `.toggle.on`) stop reaching `output.html` and `stage.html`. The token block and the self-hosted fonts stay shared — the seal is against rules and template data, not against the palette.
- [ ] **Step 4:** a test that fails when a console-only rule or an app-chrome token reappears in template data.

## Track G — the timer runs bare

- [ ] **Step 1:** the dock's Start passes no label and no done-message. Digits alone.
- [ ] **Step 2:** `capture.js`'s defaults become empty strings, so a caller that omits them gets a bare timer rather than a surprise.
- [ ] **Step 3:** the Planner keeps both fields and gains the interface it never had — a cue that wants words beside the clock says so, and fires them through Live exactly as today.
- [ ] **Step 4:** `countdown.js::countdownRemainingMs` stays the only arithmetic; `countdown_from` keeps being written; `adjust_countdown` still cannot create a countdown; none of the sixteen countdown tests changes shape.

## Track I — starter content on a fresh install (after A)

- [ ] **Step 1:** reverse `db/mod.rs:662` deliberately, with a numbered `DECISIONS.md` section of its own, written in the same commit as its citation.
- [ ] **Step 2:** seed announcements with the operator-facing title separated from the words that reach the room (REBRAND §10).
- [ ] **Step 3:** the 34 files at `src/backgrounds/` become `media_assets` rows, reachable from the Library and from a template's media layer.
- [ ] **Step 4:** one example service plan carrying a countdown cue with its own label and duration.
- [ ] **Step 5:** `the_bare_fixture_is_a_first_launch_and_nothing_more` keeps its name and its job, and asserts the starter set **by identity** — not zero, not a range. `demo.rs` is untouched.

## Track J — the browser-driven pass

- [ ] **Step 1:** render the console against the mock bridge and the output and stage pages against the real backend, per `docs/qa/QA_HARNESS.md`.
- [ ] **Step 2:** drive all forty templates at 1920×1080 and at a phone width, watching for the rule 42 first-verse defect and for anything a static instrument cannot see.
- [ ] **Step 3:** confirm the three editor layout fixes in a real layout engine, not in argument.
- [ ] **Step 4:** file what is found in `docs/qa/RELAY_GAP.md` with fresh RG ids, and write the pass up as a frozen audit under `docs/qa/audits/`.

## What this wave does not claim

- It does not move the release decision. Word error rate remains unmeasured in every language, neither platform has a code-signing certificate (RG-73), and nobody but the author has run a service.
- It does not touch detection, the router, or any threshold.
- It does not close RG-139.
- It does not unpark NDI and it adds no SDI integration.
- It does not reverse DECISIONS §29, §35, §70 or §78.
