# Design — timers, templates, stage, settings, routing

**Date:** 2026-09-15
**Status:** approved, not yet implemented
**Branch:** `feat/wave7-timers-templates-stage`

This document is the design for a seven-part change set. It is written for the
engineers and agents who will implement it. Every claim about current behaviour
in this document was verified against the working tree by reading the code at
the cited line, not inferred from the handbooks — several of the handbook
statements it corrects were wrong.

## What prompted this

An operator's review raised thirteen items: the timer, the splash screen, the
word to the preacher, multiple programme timers, the templates/themes
duplication, planner colour coding, content looks, the stage display, output
routing to OBS and ATEM, the Settings surface, template overflow, a slide-size
control, and colour in Quick tools.

Investigation found that **most of the substrate already exists.** The twelve-phase
work recorded in `docs/REBRAND.md` shipped a countdown with pause and a warning
rule, a word to the preacher as a stage-only hub message, a splash screen, a slide
grid, and a single template model. The work below is therefore mostly repair,
consolidation and completion, with exactly one genuinely new subsystem (the timer
registry).

Four things the review asked for were found to be already built and working, and
are **not** in scope except where a wave touches them for another reason:

- The word to the preacher already overrides the whole stage display and already
  flashes. `src/Stage.svelte:921-954` — `position: fixed; inset: 0; z-index: 50`,
  outside the zone layout on purpose, pulsing `#c8121c ↔ #7a0a11` under
  `prefers-reduced-motion: no-preference`. It reaches no congregation screen
  (`src/lib/r6-contracts.test.js:85`).
- The countdown warning already flashes on all three surfaces — wall, stage and
  dock (`src/lib/TemplateRender.svelte:1702`, `src/Stage.svelte:917`,
  `src/lib/Dock.svelte:1510`).
- Stage zones (`note`, `next`, `elapsed`) exist, persist per device under
  `relay.stage.zones`, and have defaulted **on** since 2026-09-14.
- STT model pinning exists and works: `select_stt_model`
  (`src-tauri/src/main.rs:4836`, service-lock guarded) writes
  `app_settings['stt.model']`, and `build_stt` (`main.rs:4714`) reads it at every
  launch, where the operator's choice beats `MODEL_CANDIDATES` order
  (`stt.rs:1120`).

### A correction this change set must make to CLAUDE.md

`CLAUDE.md` states, in the build-status block, that *"a pilot must pin the model,
and nothing currently does."* That is false as written and has been since
`select_stt_model` landed. Wave 1 corrects it. The genuinely missing control
beside it is a **pinned recognition language**: the picker at
`src/lib/views/Settings.svelte:1277` defaults to "Auto-detect (code-switching)",
and RG-116 records that automatic language election cost a whole service on
`ggml-small` — 17 incoherent transcripts against 161 with the language fixed.

## The four decisions this design rests on

These were put to the operator and answered. They are recorded here because three
of them change or supersede a written decision, and a later reader must be able to
see that they were deliberate.

1. **A stage timer survives a panic control; a congregation timer does not.**
   This reverses the refusal recorded in `docs/DECISIONS.md:538` (§27). See
   "Wave 3 — the §27 reversal" below for why this makes the code more honest
   rather than less.
2. **Themes are deleted as a concept.** The nine builtin themes become five named
   template families across the five content kinds.
3. **The seed becomes twenty-five templates** — five styles × five content kinds.
   Fourteen of the current thirty-nine are deleted.
4. **The Planner gains a screen choice per cue; no existing fire path is
   removed.** Removing them would break the operator override that CLAUDE.md
   names as a non-negotiable constraint.

## Constraints every wave must respect

These are the guarantees the fire path already carries. They were surveyed
specifically for this change set; each is cited so an implementer can re-read the
enforcement rather than trusting this list.

| Guarantee | Choke point | Pinned by |
|---|---|---|
| One caller of `channels::broadcast_content`, with the pre-air validator inside it | `main.rs:605` `broadcast_with_clock` | `hardrules.test.js:246`, `safescreen.test.js:124` |
| A passage may not outlive the content that replaced it | `main.rs:632`, a negated match on `kind` | `e2e.rs:1458` |
| Only `content`, `clear` and `black` are retained for a late-joining screen | `channels.rs:936` `is_screen_frame` | `channels.rs:3272`, `:3333`, `:3398`, `:3695` |
| The fire path is generic over `tauri::Runtime` | four names | `hardrules.test.js:114` |
| A panic control may never report a success it did not achieve | `main.rs:5755`, `:5765`, `:5779` | `panic.test.js` (14 tests) |
| Nothing leaves the machine during a rehearsal | `channels.rs:880` `rehearsing`, five call sites | `e2e.rs:1043`, `:1083`, `:1614`, `:2474`, `r6.rs:118` |
| An enumerated set of commands is held back while a service records | `servicelock.rs:88` `guard` | `servicelock.rs:200`, `:293` |
| Every command the frontend calls exists; every event is heard or excused | `ipc.test.js` | itself |

Three of these enforcements have gaps that this change set must close **before**
it relies on them. They are wave 0.

---

# Wave 0 — prerequisites

Small, and everything after it depends on the guarantees being real rather than
documented.

## 0.1 Rehearsal gating needs an enumeration test

Retention is enumerated: `channels.rs:3304` `FRAME_VERDICTS` plus
`every_kind_this_module_publishes_has_an_explicit_verdict`, which scans the
module source and fails on any published kind with no verdict. Rehearsal gating
is not. The list of four gated publishers lives only in a doc comment at
`channels.rs:849-856`.

Wave 3 adds a publisher. A publisher with no `rehearsing()` check currently fails
no test, and the repository has already been bitten by exactly this — the comment
at `channels.rs:1113-1118` records that `stage_next` shipped ungated and was
invisible to `e2e.rs`'s `Wall` because it emits no Tauri event.

**Build:** a test in the same shape as `FRAME_VERDICTS` — scan `channels.rs` for
every function that calls `publish_kiosk` or `emit`, and require each to appear in
an explicit `REHEARSAL_VERDICTS` table with a gated/ungated verdict. The four
deliberately ungated publishers (`transition`, `set_template`, `set_themes`, and
`main.rs::set_channel_template`) each carry a reason at their call site; the table
must carry the same reason, so a future reader does not have to guess.

**Watch for:** the retention scanner reads `include_str!("channels.rs")` only, so
`channel_template` — published from `main.rs:5691` and `:5706` — has no verdict in
`FRAME_VERDICTS` today. The new rehearsal scanner must read both files, and the
retention scanner should be widened to match in the same change.

## 0.2 `fire_media` must become generic over `tauri::Runtime`

`main.rs:2972` takes a concrete `tauri::AppHandle`. It is a real fire path with no
`e2e.rs` coverage, and rule 24's enforcement (`hardrules.test.js:114`) checks four
names only — `fire_manual`, `handle_nav`, `clear_or_report`, `persist_cue` — so it
cannot see the omission.

**Build:** make it `fn fire_media<R: tauri::Runtime>(app: tauri::AppHandle<R>, …)`,
add `fire_media` to the name list in `hardrules.test.js:114`, and add an `e2e.rs`
case that drives it.

## 0.3 Two enumerations name a command that no longer exists

`push_announcement` appears in `servicelock.rs:217` and `transport.test.js:126`.
It is not a registered command. Neither assertion checks registration, so the
staleness is silent, and `transport.test.js`'s `SCREEN_COMMANDS` is the gate on
"every wrapper that fires decides about `liveCue.onAir`" — it only checks the
eight names it lists.

**Build:** remove the dead name from both; make both lists assert that every name
in them is registered in `generate_handler!`, so the next one cannot rot quietly.

**Verification for wave 0:** `cd src-tauri && cargo test` and `npx vitest run`,
both green, and each new test verified to fail when the defect it describes is
reintroduced. Per CLAUDE.md: test the bug, not the fix.

---

# Wave 1 — Settings, splash, routing

Three independent tracks. None blocks another and none blocks a later wave.

## 1.1 Settings truth pass

Ten findings, ordered by severity. Full evidence is in the audit that produced
them; the essentials are below.

### F-2 — Safe mode disarms nothing (P1, live safety)

`src/lib/views/Settings.svelte:914`. The row's own words are *"Outputs will not
open and detection is disarmed — nothing Relay does can reach a screen."*

What actually happens: `setSafeMode` patches a `localStorage` boot record. `$safeMode`
is read in `src/App.svelte` at lines 419 and 493 only, both inside `onMount`.
There is no reactive statement re-applying it. `src/lib/views/Live.svelte`
references `safeMode` zero times, so the run surface's fire path is not gated at
all. There is no `safe_mode` anywhere in `src-tauri/src/`. Already-open windows
stay open and the detector stays armed until the next launch.

**Build:** the enforcement belongs where the promise is made. `setSafeMode`
becomes the choke point: on the transition it disarms detection and closes output
windows, and it **reports a failure rather than flipping the label** — the same
contract as a panic control, because it makes the same kind of promise.

**Do not** fix this by adding a `$safeMode` check to each fire site. That is the
shape of the four "guarantee kept on one door, skipped on its twin" bugs CLAUDE.md
already records.

**Open question for the implementer to settle and record:** whether safe mode
should also exist in Rust so the engine cannot be armed while the record says
disarmed. The frontend choke point is sufficient for the stated promise; a Rust
flag would be stronger. Decide, and write the reason down.

### F-6 — The setup walk-through is unguarded during a recorded service (P2)

`Settings.svelte:1438` carries no `disabled` expression in any state. One click
sets `session.setupDone = false`, which mounts `FirstRun` full-screen over a live
console. From inside it, `stopMicTest()` and `chooseDevice()` each call
`stopCapture()` on the live microphone (`FirstRun.svelte:177`, `:201`), and "Try
it" fires `manualFire('John 3:16')` to the congregation's screens (`:222`).

The service lock cannot help, because `restartSetup` is a session write and
`servicelock::guard` is never consulted.

CLAUDE.md rule 44 already names this sentence. The Esc half of that rule is fixed
and pinned (`FirstRun.svelte:271`, `:283`; `panicoverlay.test.js:121`). The
"over a recorded service" half is open and has no RG row.

**Build:** disable it while `$serviceLock.engaged`, using the lock's own wording —
the service-lock section is six rows below it on the same page.

### F-1 — "Check for Updates" reports a check it never ran (P2)

`Settings.svelte:1580`. While a service is recording, `checkForUpdate()`
(`src/lib/updater.js:92`) returns `null` **without calling `noteChannel`**, so
`doCheckUpdates` falls through to `ch.state === 'ok'` and prints "You're on the
latest version." The status row six pixels above it is honest, because it goes
through `describeChannel`. The two disagree and the louder one is wrong.

This is rule 35 on the one path by which a fix reaches a church that already has
Relay — the same category as RG-83 and RG-133.

**Build:** the refusal is a **third outcome**. Either `noteChannel('skipped')` and
let `describeChannel` own the sentence, or disable the button with the reason. The
button must not reuse a stale channel state as its own result.

### The remaining six

- **F-3** (P3) — "Detect speakers" (`Settings.svelte:1077`) is pixel-identical when
  the permission is refused and when the machine genuinely has one output.
  `audioOutput.js:81` already knows which happened; return it and say so, with the
  OS path to reverse a refusal.
- **F-5** (P4) — `ModelSetup.svelte:54` renders a Rust `Err(String)` verbatim,
  bypassing `errors.js`, the one humaniser. Latent today because those strings are
  volunteer-worded; nothing constrains the next one.
- **F-7** (P3) — `History.svelte:580` "End current service" swallows every failure
  in a bare `catch {}` (`capture.js:605`) and repaints the same list. Same shape as
  rule 15, on a smaller control.
- **F-8** (P3) — the Sentry DSN field (`Settings.svelte:1853`) has no commit path
  while crash reporting is already on: `setCrashReporting` is called only from
  `toggleCrash`. The one control that decides where data leaves the machine can be
  edited and silently keep pointing at the old place. Commit on blur, or add a Save.
- **F-9** (P3) — `Settings.svelte:1455` paints a demo-content count in
  `var(--v-amber)` by inline style, on a page that is never on air, in a file whose
  own comments say "Rose, never amber" three times. Use `.s-netwarn`.
- **F-10** (P3) — `updateMsg` (`:1583`) and `crashMsg` (`:1890`) have no live
  region, while six other message surfaces on the same page do. Add `role="status"`.

### 1.2 The CLAUDE.md corrections

Two, both in the build-status block:

1. *"a pilot must pin the model, and nothing currently does"* — false. Replace
   with what is true: pinning exists (`select_stt_model` → `app_settings['stt.model']`
   → `build_stt`), and what is missing is a pinned **language** and a pre-service
   confirmation of which model and language a service will run on.
2. The event count. CLAUDE.md and `docs/ARCHITECTURE.md` §6 both say nineteen.
   The scanner's own set is **twenty**; §6's table lists eighteen, omitting
   `output://error` and `output://transition`. Reproduce with
   `grep -rhoE '"[a-z_]+://[a-z_]+"' src-tauri/src/*.rs | sort -u`, which yields
   twenty-one including `tauri://localhost`, an origin string at `channels.rs:2278`
   and not an event.

Also add: a recognition-language pin, defaulting to something other than
auto-detect, and a pre-service statement of model + language. `Dashboard.svelte:267`
currently says "Ready for a service." over `ggml-base` without naming it.

## 1.3 Splash and boot

`src/lib/Splash.svelte` exists — 399 lines, amethyst brand (never amber, stated at
`:9`), `BOOT_HOLD_MS` 900 and `BOOT_CAP_MS` 4000 (`App.svelte:393`). A clean boot
collapses to the splash alone; the four stage screens and four gates appear only
when they have something to say (`BootSequence.svelte:23`).

This is a design-quality pass, not a build. Scope: the hero lockup, the stage and
detail lines, the spinner, and the transition into the console. Nothing about the
boot ladder's logic changes, and `boot.js`'s "a stub can never render green" rule
is untouched.

## 1.4 The output routing map

Research conclusion: **Relay already has both realistic paths. What is missing is
the map, not the technology.**

The two real paths:

1. **A fullscreen Tauri webview on a physical monitor** —
   `channels::open_native_window` (`channels.rs:419`) places a borderless webview
   inside the target monitor's bounds and fullscreens it; `auto_open_outputs`
   (`main.rs:5164`) opens only `native_window` channels, only onto a connected
   display, and never onto the operator's own monitor.
2. **The `:8032` URL in a browser source** — and this one costs **zero GPU display
   pipes**, which is the entire answer to "my laptop has one HDMI port".

Facts that settle the ATEM question, each confirmed against Blackmagic's own
tech-spec pages:

- Every ATEM Mini has HDMI inputs and no SDI inputs. Every rack-mount ATEM has SDI
  inputs and no HDMI inputs. The single HDMI connector on an ATEM Television
  Studio HD8 is an **output**.
- A **Blackmagic Micro Converter HDMI to SDI 3G, $75**, dissolves the whole SDI
  question. Relay emits a plain HDMI display signal — which it already does — and
  the dongle does the rest. Nothing SDI-aware is needed in software, which is why
  CLAUDE.md's "no native SDI hardware integration" constraint costs nothing.
- **No ATEM accepts NDI.** None of them. Blackmagic's IP direction is SMPTE 2110.
  NDI would buy OBS, vMix, TriCaster and ProPresenter — a different set of
  destinations from the one the ATEM question is about.
- The ATEM **Media Player is not a live input**: stills only, roughly three seconds
  per 1080p frame, RLE-compressed so dense text is slower, a pool lock, twenty
  slots. A pre-service graphics store, not a live text renderer.
- **SuperSource composites inputs already on the switcher.** It is not an ingest
  path.
- A **virtual camera is not a signal.** Both the Windows and macOS APIs register a
  device with the local OS camera stack; nothing reaches a cable. The macOS route
  additionally needs a restricted Apple entitlement, so it sits behind the
  code-signing certificate Relay does not have (RG-73). Syphon and Spout are the
  same story: same-machine texture sharing.
- **Apple Silicon has no DisplayPort MST extended desktop at any chip.** Base
  M1/M2 drive one external display; M3/M4 base drive two; only Max tiers reach
  four. DisplayLink is the only workaround, and granting it the macOS Screen
  Recording permission **disables HDCP system-wide** — harmless for scripture
  pages, a trap for a church that also plays a licensed clip.

**Build:** documentation, plus one helper in Outputs that answers "how do I reach
this screen?" for the four cases a church actually has — a projector, an ATEM, an
OBS machine, and a screen with nothing but wifi behind it.

**Also fix:** `docs/SPEC.md:25` claims Relay "talks to OBS, ATEM, and ProPresenter
over NDI, HDMI, and the local network." NDI is parked and `open_ndi_output`
returns an error. `SPEC.md:151`, `:169`, `:171` and `Settings.svelte:1421` all
state it correctly; line 25 does not.

**Not in scope, and worth stating:** NDI is genuinely reachable without shipping a
proprietary byte — Vizrt documents `NDIlib_v5_load()` runtime loading as being
"of value in Open-Source projects", with a redistributable-URL constant provided
for exactly the case where the runtime is absent. It stays parked because it buys
nothing toward ATEM, which is what was asked about.

---

# Wave 2 — Templates ⊕ Themes

## 2.1 Why the merge is correct rather than cosmetic

A theme has **no field a template does not already have**. `THEME_STYLE_KEYS`
(`src/lib/themes.js:29`) is a twenty-one-entry subset of the same flat `style`
keys a template carries. There is no `themes` table — custom themes are a single
JSON array in `app_settings['themes.custom']`. There is no Rust `Theme` struct.
The only theme-exclusive concepts are `builtin: true` and a negative id.

And `LAYER_THEME_KEYS` (`src/lib/themes.js:552`) records that **nine of the
fourteen theme controls move nothing on a layer template**, because `applyTheme`
resolves tokens on `color`, `fill` and `font` only (`:344-356`).

`docs/DECISIONS.md:501` (§27) established themes as "a style layer BENEATH
templates, not a parallel system". The merge does not contradict that; it
completes it. A layer beneath templates whose only content is a subset of the
template's own keys is a layer that has finished being useful.

## 2.2 What is deleted

- `src/lib/views/themes/ThemeGallery.svelte`, `src/lib/views/themes/ThemeEditor.svelte`
- The Themes desk in `src/lib/views/Templates.svelte`'s two-way switch
- `style.themeRef` and `templateThemeRef` / `resolveThemed` / `applyThemeToTemplate`
- `app_settings['themes.custom']`, `loadThemes` / `saveTheme` / `deleteTheme` /
  `exportTheme` / `importThemeFromFile` (`capture.js:1820-1946`)
- The `themes` hub frame and `KioskHub::set_themes` (`channels.rs:1430`), plus its
  `FRAME_VERDICTS` row and its `hello` reply (`channels.rs:1730`)
- `THEME_PREVIEW_TEMPLATE`, the shim canvas the two galleries painted through

`src/lib/themes.js` keeps `THEME_FONTS` / `fontLabel` and the token resolvers, which
are about fonts and layer tokens rather than about themes, and moves to a name
that says so.

**What must NOT be deleted:** `session.js:227` `MOVED_TABS.themes = 'templates'`.
A stored session naming the old desk must still land somewhere valid.

## 2.3 The migration

Every template carrying `style.themeRef` has its resolved style keys inlined once
and the ref removed. The effective look is unchanged by construction, because
`{ ...theme.style, ...template.style }` is what `applyTheme` already computes.

**It must be retryable** — CLAUDE.md rule 25. `DROP TABLE IF EXISTS` any scratch
table first, roll back on failure, and never leave a transaction open for the
following `PRAGMA foreign_keys = ON` to no-op inside. A migration that fails
mid-batch and then fails every subsequent boot, before the window is shown, is a
failure mode this repository has already had.

Custom themes in `themes.custom` are migrated to real templates before the key is
dropped, one template per (theme × kind) the operator actually used. A theme
nobody applied is dropped with its name recorded in the migration's log line.

## 2.4 The seed becomes twenty-five

Five style families × five content kinds — Scripture, Songs/Lyrics, Media,
Announcements, Timer/Countdown.

The pattern already exists: `theme_templates()` (`src-tauri/src/db/templates.rs:371`)
seeds twelve as three families × four kinds, and
`every_theme_is_a_complete_coordinated_family` (`:860`) already enforces
completeness. Extend it to five families × five kinds and delete the fourteen
leftovers — the fourteen ad-hoc presets at `:271` and the parts of the eight-item
shelf that no family claims.

`preset_template_count()` (`:524`) is the single source of the number and must
agree. `the_bare_fixture_is_a_first_launch_and_nothing_more` (`qa.rs`) is the
tripwire that will catch a seed that drifts from what a fresh install contains —
it is what caught `tpl_song` being seeded on purpose, and it will fail loudly here
if the count and the fixture disagree.

**The five families are Classic, Aurora, Ember, Lower Third and High Visibility.**

This choice needs stating, because "five styles" and "twenty-five templates" can
be read two ways and the wrong reading loses tested capability.

A lower third is not a sixth content kind — it is a *keyed variant*, and the
transparency law in `resolveOutputTemplate` (`src/lib/layers.js:262`) depends on
keyed templates existing at all. Making it one of the five families means every
content kind gets a keyed option, which is strictly more capability than the two
keyed presets that ship today. High Visibility is likewise a family rather than an
extra: CLAUDE.md names it as an accessibility feature beside `legibility.js` and
the distance preview, and it has to cover every kind to be worth having.

**SuperSource and Stage · Large type do not take seed slots.** Both are already
`STARTERS` entries in `src/lib/layers.js:729` — the SuperSource starter and its
inspector block landed in REBRAND phase 12 — so they stay reachable when creating
a template, and `composite.test.js`'s coverage of the `region` layer is untouched.
A starter is a way to make a template; a seed row is a template a church did not
ask for. Twenty-five seed rows, starters unchanged.

Nocturne is dropped. Its style keys are close enough to Ember and Classic that
keeping it would cost a family slot that Lower Third and High Visibility each use
better.

## 2.5 Content looks

The operator reported that clicking a content look "activates all". It does not —
no handler in the repository writes more than one kind per click, and every call
site of `setContentTemplate` passes exactly one key.

What actually happens: the template editor stacks **two visually identical
five-chip grids over the same five labels**, both `.te-showgrid`
(`TemplateEditor.svelte:1697` and `:1717`). The second, "Content this template
renders", starts **all five ticked**, because `templateShows` returns `true` for
every kind when `layout.shows` is absent (`src/lib/layers.js:246`). The first
click materialises the list as all-kinds-minus-one. The file's own comment at
`:1676` records that the two rows were already known to look alike.

Both rows are real features and neither is deleted:

- **"Used for"** binds a kind to this template — one row per kind in `app_settings`
  under `tpl_{kind}` (`src-tauri/src/db/settings.rs:36`). A `set_setting` upsert,
  so binding a kind to template B silently replaces A; no unset-the-previous-owner
  step exists and none is needed.
- **"Content this template renders"** is the per-screen allow-list — a stage or
  confidence monitor that should ignore a picture and hold what it had.

**Build:** give the second row a different control shape — a switch list in its own
section with its own heading, not a chip grid — and write `layout.shows`
explicitly on every seeded template so it never starts in the all-ticked state
that made the first click look destructive.

**Also:** `Settings.svelte:186` holds a **private `ctMap`** that omits `countdown`,
contradicting the "one store" comment at `capture.js:274`. It must read
`$contentTemplates` like the other three surfaces.

## 2.6 The default template actually applying

`default_template_id` is a settings-KV integer. It is read by **two of twelve
render surfaces**. Rust never reads it at all — `grep default_template_id
src-tauri/src/` returns nothing. `src/Output.svelte:77` — the real wall, the kiosk
and every OBS browser source — falls back to a hard-coded `DEFAULT_TEMPLATE`,
which is `BUILTINS[0]`, Classic Serif.

The default reaches a screen exactly once, at creation: `addChannel(name,
newTarget, $defaultTemplateId ?? 1)` (`Channels.svelte:348`). Afterwards, changing
the default broadcasts nothing — `setDefaultTemplate` is a plain `set_setting`
(`capture.js:264`) with no `channel://retemplate` and no hub push.

That is the whole explanation of both reported symptoms: the default does not
activate on all screens, and the preview does not show it.

**Build:**

1. One resolver that every surface calls, taking the default as the last link in
   the chain the way `Live.svelte:1349` already does: channel template → content
   look → configured default → bundled builtin. Never a bare `BUILTINS[0]`.
2. Rust reads it, so `cue_or_content_tpl` (`main.rs:3029`) can answer with it.
3. Changing the default broadcasts `channel://retemplate` to screens that follow
   the content look, so the change is visible without reopening anything.
4. The gallery inspector preview (`TemplateGallery.svelte:729`) renders through
   the same resolver instead of being hard-bound to the selected row.

**Do not** break `resolveOutputTemplate`'s existing precedence
(`src/lib/layers.js:262`) — transparency law first, then a pinned cue template,
then the screen's own, then the content look. DECISIONS §29 and §70. The default is
a new final fallback, not a new winner.

## 2.7 Overflow

Three concrete sources, all outside the fit loop:

1. **The default countdown block.** `TemplateRender.svelte:1483-1497` emits
   `.cd-default` in layer mode, outside the `{#each layerViews}`, carrying no
   `.ltext` / `.lfit` class. `fitLayers`'s `querySelectorAll('.ltext')` (`:1239`)
   never sees it, `fitBoxes()` (`:580`) never sees it, so it is invisible to
   `overflowing()` **and** to `onFit`. Its CSS (`:1649`) sets no `overflow` at all,
   and it renders at `verseSize * 2`.
2. **Ticker mode.** `.ticker` / `.ticker-label` / `.ticker-run` (`:1528`) render
   *instead of* `.content`, and `fitText` queries `.slide .content` (`:361`), so
   the loop finds zero boxes and `lastFitScale` stays 1. `.ticker-label` is
   `white-space: nowrap` at raw `refSize`.
3. **The contrast panel.** `.content.panel { overflow: visible }` (`:1729`)
   removes the clip that `.content` (`:1719`, `overflow: hidden`, `max-height: 92%`)
   otherwise provides.

**Build:** give `.cd-default` the `.ltext` / `.lfit` contract so the binary search
sees it; give ticker mode its own fit pass; clip `.content.panel`.

**Do not weaken rules 37 or 42.** The 45% legibility floor is a *ratio* of the
template's own chosen size, not a point size, and `TemplateRender` must keep
shrinking and showing rather than blanking — a blank screen is strictly worse for
a congregation. The two-try refit bound stays.

**Note for the implementer:** `templatefit.test.js` asserts `needsRefit` and the
pure `templatemodel.js` helpers, and asserts **nothing** about `fitLayers`, the
binary search, `FIT_FLOOR_CQW`, `MIN_LEGIBLE_SCALE`, `onFit`, `verifyFit` or
`document.fonts.check`. The new coverage has to be written, not extended. And
`fittedWithTheRealFont` (`:602`) is **inert in the shipped product** — Relay
bundles no webfont, so an empty `FontFaceSet` answers `true` for every family. Its
own comment says so. Do not delete it (rule 42 exists for a real measured bug) and
do not count it as protection.

---

# Wave 3 — Timers

## 3.1 What is wrong today

The countdown is **four fields riding on the one live `OutputContent`**
(`channels.rs:94`, `:107`, `:121`, `:123`), held in a single slot,
`CountdownState(Mutex<Option<OutputContent>>)` (`channels.rs:1017`). It has no
identifier. `grep timer_id` and `grep countdown_id` return nothing anywhere in
`src/` or `src-tauri/src/`.

It is a wall-clock deadline broadcast once and ticked locally by each renderer —
three `setInterval`s, at 250 ms on the wall, 500 ms in the dock and 1000 ms on the
stage page. Rust owns no tick. A paused countdown is not an instant, which is why
`countdown_paused_ms` exists as the one exception; `countdown.js:76` is the single
reader of how long is left, and every surface goes through it. That part of the
design is good and survives this wave unchanged.

What is wrong is its lifetime. `note_countdown` (`channels.rs:1029`) keeps state
only while the live content is a countdown:

```rust
let next = content
    .filter(|c| c.countdown_to.is_some() && c.kind.as_deref() == Some("countdown"))
    .cloned();
```

Fire a verse, a song or a notice and the countdown is forgotten. `adjust_countdown`
then answers `"Nothing is counting down."` (`main.rs:2875`), and there is no
resume path — `start_countdown` is the only place a countdown is created. Pinned
by `e2e::r7_a_cleared_countdown_cannot_be_brought_back_by_the_transport`.

That is precisely the reported defect, and it cannot be patched. It is a
consequence of where the state lives.

## 3.2 The design: one registry, two scopes

`TimerRegistry(Mutex<HashMap<TimerId, Timer>>)` replaces the single slot.

```
Timer {
    id:           TimerId,
    label:        String,
    done_msg:     String,
    target_ms:    i64,          // epoch; the deadline, as today
    from_ms:      i64,          // when it was aimed; the warning rule's span input
    paused_ms:    Option<i64>,  // held with n ms left, as today
    warn_ms:      Option<i64>,  // per-timer threshold; None = the default rule
    scope:        Scope,        // Both | Stage
    plan_item_id: Option<i64>,
}
```

**`start_countdown` becomes a thin wrapper.** It creates a `scope: Both` timer and
fires a content frame that projects it into the four existing `countdown_*` fields.
Everything downstream is unchanged: `Output.svelte`, `Stage.svelte`,
`TemplateRender`, `kiosk_content_json`, `countdown.js`, and all sixteen existing
countdown tests keep working against the same wire form. The registry becomes the
source of truth and the fields become its projection.

**Programme timers are `scope: Stage` and fire no content frame at all.** That is
exactly why they survive a verse: nothing about them rides on the live content, so
replacing the live content cannot forget them.

**One formatter, one reader.** `countdown.js::countdownRemainingMs` stays the only
arithmetic. Adding a second would reproduce the bug `docs/REBRAND.md` phase 7
records as already fixed once.

## 3.3 Publication and retention

Timers publish a `{"kind":"timer", …}` hub frame carrying the registry's
stage-visible entries.

**It gets its own retained slot — `last_timers` — and is never `last_screen`.**
The precedent is `last_transition` (`channels.rs:1281`), which has its own slot for
exactly this reason: `last_screen` holds one frame, newest wins, so retaining a
non-content frame there means the next screen to join is sent the extra and paints
no verse. That is rule 43's trap 1, and it is why `stage_next` is excluded.

On `hello`, the order becomes: template, themes *(removed by wave 2)*, transition,
**timers**, then the retained screen frame last (`channels.rs:1719-1763`). Timers
go before the screen frame so a late-joining stage tablet paints the reading last
and does not flash a timer over it.

`FRAME_VERDICTS` (`channels.rs:3304`) gains a `("timer", false)` row — not retained
into `last_screen` — and the new `REHEARSAL_VERDICTS` table from wave 0 gains a
gated row, because a rehearsal must reach no stage tablet.

**Trap 2, restated:** `is_screen_frame` is a `contains`, not a `starts_with`,
because `serde_json`'s map is a BTreeMap and a content frame begins
`{"content_kind":…`. The first version of that matcher matched nothing while
looking exactly like the bug it fixed. Any new matcher in this wave has the same
hazard.

## 3.4 The §27 reversal

`docs/DECISIONS.md:538` says:

> The panic "Clear all screens" stays TOTAL — a monitor is not exempt. A persistent
> service timer that survives the clear was considered and REFUSED: it would mean
> adding an "except monitors" branch to a life-critical control… Whether a stage
> monitor should ignore the congregation clear is a real product decision, left
> open rather than resolved by a quiet special-case.

The operator has taken that decision: **a `Stage`-scoped timer survives a panic
control; a `Both`-scoped timer does not.**

This is recorded as a new DECISIONS section that explicitly supersedes §27's
refusal, for three reasons worth writing down:

1. §27 itself left the question open and named it a real product decision rather
   than a settled one. It is being answered, not overruled.
2. **§27's "no exceptions" is already not literally true in shipped code.**
   `src/Stage.svelte:436` made the opposite call locally — the service elapsed
   clock deliberately survives, on the stated grounds that *"a cleared or blacked
   wall is not the end of a service."* And the word to the preacher survives both
   panic controls too: the `clear`/`black` branch at `Stage.svelte:406-438` resets
   `visible`, `note`, `cdTo`, `cdFrom`, `cdPaused` and `next`, and does **not**
   reset `alert`. That one appears to be undecided rather than decided — nothing
   in the tree records a reason.
3. The scope split keeps the congregation guarantee exactly as strong as it is
   today. Clear and Blackout still take back every congregation screen totally.

**The rule that must hold:** the split lives in **one place**, and it is a property
of the timer (`scope`), never a branch inside `channels::clear` or
`channels::black` asking which screen it is talking to. A panic control that has to
ask a question can fail to answer it.

While implementing, settle and write down whether `alert` surviving a panic on the
stage page is intended. It is currently a silent third answer to the same question.

## 3.5 The warning threshold becomes a setting

`COUNTDOWN_WARN_MS = 60_000` and `countdownWarning(remaining, total)` —
the last minute, or the last tenth of a countdown shorter than ten minutes
(`src/lib/layers.js:318-338`). The comment at `:325` says it is *"a rule rather
than a setting, deliberately: the control belongs in the Settings pass, and a
setting with nowhere to set it is worse than a sensible default."*

This is the Settings pass. `warn_ms` becomes per-timer, settable per cue in the
Planner and as a default in Settings, and `countdownWarning` takes the override
when present and falls back to today's rule when absent.

**`countdownTotalMs` returns null rather than a guess** when either end is missing
(`countdown.js:105`), because a made-up span puts the warning colour on at the
wrong moment. Keep that. A colour that is on at the wrong moment is worse than one
that is on a minute early.

The flash itself needs no new work — `cdwarn` keyframes already exist on all three
surfaces, with a `prefers-reduced-motion` glow fallback on each.

## 3.6 What a new content kind must satisfy

Eleven sites consume the kind string. This wave adds one, and the list is here so
none is missed — missing one produces this repository's signature bug shape.

| # | Site | If skipped |
|---|---|---|
| 1 | `main.rs:632` rule 38 match | anything not literally `"scripture"` disarms the passage; a timer is disarmed for free |
| 2 | `pipeline.rs:247` `is_countdown` | **see below — this is the one that bites** |
| 3 | `main.rs:3029` `cue_or_content_tpl` | no content look for the kind |
| 4 | `main.rs:3057` `ContentTemplates` (five hard-coded fields) | no row in the content-look UI |
| 5 | `channels.rs:895` `kiosk_content_json` | native windows get the field, kiosk and OBS do not |
| 6 | `src/lib/layers.js:228` `CONTENT_KINDS` | the kind cannot be toggled per screen |
| 7 | `src/lib/layers.js:246` `templateShows` | **a template with an explicit `layout.shows` hides an unknown kind silently** |
| 8 | `src/Output.svelte:180` and `:279` | the filter is applied at both doors |
| 9 | `src/Stage.svelte:406-445` | the stage page branches on message kind |
| 10 | `TemplateRender.svelte:1117` `slideKey` | a kind varying in none of the five keyed fields will not re-key, so no transition and no refit |
| 11 | `persist_cue` → `cues.type` | free-form TEXT, no CHECK; no migration needed |

**Site 2 is the trap.** `pipeline.rs:240`:

```rust
let is_countdown = content.countdown_to.is_some()
    || content.countdown_paused_ms.is_some()
    || content.kind.as_deref() == Some("countdown");
if !is_countdown && !has_text && !has_media && !has_reference { return Err(Unsafe::Nothing); }
```

A new timer kind with its own field name and no text would be **refused by the
pre-air validator** as `Unsafe::Nothing`. The design avoids this by keeping the
`Both`-scoped timer's wire form as the existing `countdown_*` fields; if that
changes, `is_countdown` changes with it, in the same commit.

Site 7 is the second trap: a new kind is visible or invisible depending on a
per-template field nobody will think to update. Wave 2.5 writes `layout.shows`
explicitly on every seeded template, which is what makes this safe.

Finally, `main.rs:632` uses `is_some_and`, which is false for `None` — a payload
built with `..Default::default()` and no `kind` does **not** disarm the passage.
Every current caller sets it; nothing enforces it. Add the assertion.

---

# Wave 4 — Stage and Planner

## 4.1 Stage display

Zones exist (`reading`, `next`, `note`, `countdown`, `clock`, `elapsed`), persist
per device under `relay.stage.zones`, and default on. The alert already takes the
whole screen. The transport, the verse search and tap-to-fire already work over
`:8032/api`.

This is a layout-quality pass plus the timer rail from wave 3. It must be driven
in a browser against the real backend rather than reviewed by reading, because the
two worst defects the 2026-09-10 pass found — a first verse fitted in the wrong
typeface, and a reconnecting screen coming back blank — were invisible to every
static instrument, and `qa-inventory` reported zero problems throughout and was
right both times.

`Stage.svelte` renders **no template at all** (`:481`) and is unaffected by wave 2.

## 4.2 Planner

**Screen choice per cue.** `plan_items` has `template_id` but no channel column.
The Planner can already assign a template per cue (`ServicePlanner.svelte:986` →
`set_plan_template` → `plan_items.template_id`), and that template becomes
`template_pinned: true` on the wire, which is what lets a cue's look beat a
screen's own. The missing half is which screens. Add it, defaulting to all.

Nothing is removed. Ten-plus surfaces can fire or set a template today, and Live's
manual box and slide grid are the Sunday-morning override that CLAUDE.md names as
a non-negotiable constraint: *"Operator override is a first-class control, never a
fallback UI."* The Planner becomes the place where the **plan** decides, not the
only place anything can reach a screen.

**Multi-timer cues** bind to the wave-3 registry through `plan_items.duration_sec`,
which exists (`db/plans.rs:72`) and currently drives no running clock.

**Colour coding.** Cue colour is deliberately zero today —
`TAXONOMY_INK = 'var(--v-faint)'` and every `TYPE[*].color` is that same token
(`src/lib/plan.js:49`, `:63-75`). The rationale at `:14-47` records which hues were
spent and withdrawn, and states that the only unclaimed hues on the wheel are
magenta ~310° and lime ~80°, for which no tokens exist.

Five kinds need five colours, so hue-per-kind cannot be invented cleanly. Use the
`--v-col-*` family (`src/app.css:222-225`), which is already kind-mapped and
already used for exactly this in the Library, and add `--v-col-timer`.

**This reuses reserved hues and is acceptable only because the Planner, like the
Library, is never on air.** `--v-col-scripture` aliases amber, which means ON AIR
anywhere a congregation's state is being reported. The Planner cannot fire to an
output (`plannerbuildonly.test.js`), which is what makes this safe — and that test
is what must keep it safe. If the Planner ever gains a fire path, this decision
must be revisited in the same change.

**Slide sizer.** `.sgrid` is fixed at `repeat(auto-fill, minmax(158px, 1fr))`
(`Live.svelte:2502`). Add a 2×2 / 3×3 / 4×4 control, following the `ZOOMS` stepper
pattern in `TemplateEditor.svelte:772` — the only zoom idiom in the app.

Note that a `Normal | Compact` density segment was built here and **deliberately
deleted**, handler, session key and CSS (`Live.svelte:1762`). This is not that: it
is a size rather than a density, it persists to session, and it exists to make a
slide readable before it is selected.

**Quick tools colour.** The card is deliberately colourless — no amber and no
amethyst anywhere, stated at `Dock.svelte:1009`, `:1447`, `:1081` and `:1520`. Its
only colour is the stage-alert red and two steel-blue primaries.

Colour-code by **tool group** using the neutral ramp and the red already present.
It cannot borrow amber (ON AIR), amethyst (rehearsal), cyan (a guess) or grey
(cued). The Controls card next door is where the four-colour treatment lives and
must stay the only place it does.

---

# Ordering and verification

Waves run in order. 0 → 1 → 2 → 3 → 4. Wave 1's three tracks are independent of
each other; wave 4's two are independent of each other.

Every wave ends green on both suites, using **the runner's own summary line** and
never a grep:

```
cd src-tauri && cargo test
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings
npx vitest run
npm run build
```

Note `npm run build` must run before the Rust suite on a fresh tree, or two
`channels` tests fail on a bare 404 because `dist/` is gitignored (RG-127).

Every new test is verified to **fail when the defect it describes is
reintroduced**. CLAUDE.md: test the bug, not the fix. This repository has had a
test that passed against a broken file because it grepped a comment, and a
regression test written against a diagnosis that turned out to be wrong, which
passed with the supposed fix reverted.

Waves 2 and 4 additionally need a browser-driven pass against the real backend,
because this machine cannot screenshot the Tauri window and the defects that
matter most in those waves are ones no static instrument has ever caught.

## What this change set does not claim

- It does not move the release decision. Word error rate remains unmeasured in
  every language, neither platform has a code-signing certificate (RG-73), and
  nobody but the author has run a service.
- It does not touch detection, the router, or any threshold. Rules 10, 28, 30 and
  34 are untouched, and nothing here makes the fire path faster by making it less
  safe.
- It does not unpark NDI, and it adds no SDI integration.
