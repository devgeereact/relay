# RELAY REBRAND — the build specification

**Trigger word: `REBRAND`.**
Say `REBRAND` to start or continue this work. `REBRAND <n>` jumps to a phase (e.g. `REBRAND 4`).
`REBRAND status` reports what is done and what is next.

---

## Context

Across this session we designed the whole of Relay's new look and behaviour as a working prototype,
not a mock-up: **Relay Studio**, at `https://claude.ai/code/artifact/8abb5121-8411-4b07-9eb9-1c202991ac50`
(**V23**), source at
`/private/tmp/claude-501/-Users-mrgee-WebstormProjects-relay/6f9d56af-f7ed-4a8b-8708-045346d2c144/scratchpad/relay-studio.html`.

Every decision below was operated, measured in a browser, and revised — several were reversed after
measurement proved the first answer wrong. This file is the specification for bringing that into the
real app. The first act of `REBRAND` is to copy this file to **`docs/REBRAND.md`** so it lives with the
code rather than in a plan directory.

**The prototype is the reference; the repository's rules win where they disagree.** In particular
`CLAUDE.md`'s colour law, `TemplateRender.svelte` as the ONE renderer, cqw sizing, the panic-control
guarantees and every named test stay exactly as they are.

---

## 1 · Brand and visual system

- **Grammar**: OBS Studio's control room (studio split, dock row, status bar) crossed with
  ProPresenter 7's object inspector. Dark only — the booth is dark and the wall is black.
- **Type**: Inter (UI), IBM Plex Mono (every figure, so a changing number never reflows the row
  beside it), Fraunces (the serif templates render in).
- **Radius is 2px everywhere.** No pills. A pill in a control room reads as a toy.
- **Density**: 11–12px UI type, 3px corners on cards, hairline seams.
- **Colour law is unchanged and non-negotiable** (`CLAUDE.md` rule 18, DECISIONS §21):
  amber = ON AIR, amethyst = rehearsal, cyan = a guess, grey = CUED.
- **Colour added by this rebrand**, none of it overlapping the law:
  - steel blue `--sel` = the thing you are working on (selection, tabs, keys);
  - red = destructive (Clear screens, delete) and the stage alert;
  - collection colours in the Library: Scripture amber, Songs steel, Announcements red, Media amethyst.
- **Controls have four distinct colours** because the two most consequential buttons in the room used
  to look alike: Go Live green → **End service amber** (it owns the on-air session), **Clear screens
  red**, **Blackout black** with a light hairline, **Rehearse amethyst**.

## 2 · Workspace grammar

Six workspaces on one desk: **Live · Library · Planner · Templates · Outputs · Settings**, a persistent
dock row beneath (Live audio · Live transcript · **Quick tools** · Controls), and a status bar.

- **Live** is the run surface: search rail, Preview / TAKE / Program, slide grid, AI detection column.
- **Quick tools** (was "Playlist") holds the three things that change during a service: the
  **countdown**, the **name band**, and **word to the preacher**. `Load whole plan` stays in its header.
- **Single click sends to Program, double click previews.** A 190 ms timer on the single press so a
  double never fires both.
- **Nothing clears the programme.** Switching workspace, loading a plan, editing a template: the
  programme is content, not an index into a grid — this was a real bug and must not return.

## 3 · The template engine

### 3.1 One property, one home

A template stores only what it has changed; `resolve()` fills the rest. `migrateTpl()` runs once and
**deletes** the legacy whole-template keys (`font`, `lh`, `tr`, `uc`, `it`, `sh`) after writing the
per-element ones. A property with two homes is a property the editor can show while writing elsewhere.

Per element (verse, reference): `font · size · colour · weight · italic · upper case · letter spacing ·
line spacing · alignment · opacity · drop shadow · height in band`, plus `text width` (verse) and
`sits above/below` + `show` (reference).
Per slide: `background colour · background style (solid / vertical fade / centre glow / diagonal /
vignette) · block position · top-and-bottom safe area · side safe area · gap`.

### 3.2 The inspector is objects, not a ladder

A **tab strip** of the objects on this slide — they wrap, never scroll behind a hidden scrollbar —
then that object's properties grouped **Text / Position / Effects**, and **Reset this object**.
Object sets by kind: full-screen looks get Background · Verse · Reference; a lower third adds **Band**;
a stage monitor is **Zones · Background · Reading · Reference**; a composite is
**Layout · Camera · Word bg · Word · Ref**.

### 3.3 Roles — what a template is *for*

Ten roles in one register (`LOOKS`): scripture, lyrics, announcement, preservice, media, stage,
`lower.name`, `lower.lyric`, `lower.bible`, supersource. Assigned from the template's own inspector
("Used for"), shown as a tag on its card. A screen may be set to **Follow the content look** so
"Scripture wears Nocturne" is said once rather than on five screens.

Resolution order (matches DECISIONS §29): a screen's own template wins; a screen set to follow uses the
role register; **a lower third always swaps to the band for the content in hand**.

### 3.4 Auto-fit

Measured, not tabled. cqw is a share of width; a 16:9 frame is 56.25cqw tall. Shrink until the
estimated block fits, with a per-face advance (mono 0.62, serif 0.49, sans 0.52). Every nested context
passes its **real** aspect: a SuperSource word region, a stage reading, a band.

## 4 · Lower thirds — three, not one

| Template | Carries | Notes |
|---|---|---|
| **Name** (+ Light) | name over role | set once, fired from Quick tools |
| **Lyric** | words only | **no reference at all** |
| **Scripture** | verse, reference beneath | reference right-aligned, tracked, small |

- The band is a **real element** (`.lband`) from `top%` to the bottom, inset by the side safe area,
  baseline lift as bottom padding, **content centred inside it** (measured 7px above, 7px below).
- **The band gives ground before the words do**: it grows upward by up to 16 points — never past a
  third of the frame — before type shrinks below 78% of the designer's size. A short name at the same
  setting does not move.
- Band opacity means what it says: the body of the band sits at exactly the set alpha (this was capped
  at 0.9× and read as grey).
- Each band is a separate template with its own `lt`; editing one touches no other. Proved by
  measurement.

## 5 · Stage monitor

A stage monitor is not a congregation screen in other colours. Zones: **Reading · Next · Note ·
Countdown · Clock · Service elapsed**, each switchable, figures **beside the reading** or **across the
bottom**.

- **Nothing may leave the screen**: the reading takes what is left (`flex: 1 1 0`), the rows below are
  the only fixed sizes (`flex-basis`, not `height`), everything clipped.
- Clean by default: reading fills the screen, countdown and clock beneath; the rest is switched on.
- Beside the reading the countdown is **three stacked pairs** (HH / MM / SS), each filling the rail;
  the rail is its own container so the figure is a share of the rail, not the frame.
- **Word to the preacher**: an operator types a line and sends it to the **stage only** — the whole
  screen, red pulsing `#C8121C` → `#7A0A11`, 8.5cqw white type with a black shadow. It exists inside
  the stage renderer, so no congregation screen can show it.

## 6 · SuperSource

One composite: a camera region and a **real rendered slide**, side by side — the word region is its own
container, so the template inside scales to the region exactly as it would to a screen of that width.
Two arrangements (camera left / word left), camera share, gap, **top-and-bottom bars only** so both
regions keep full width, a background plate that fills the bars, corner radius, outline and outline
colour. A composite may not be another composite's fill.

## 7 · The countdown

One timer, one formatter, read by the slide, the stage rail and the transport so they cannot drift.
Fields are **hh : mm : ss** with a format picker (auto / m:ss / h:mm:ss). A countdown cue **starts** it.
Transport: Start/Pause · Reset · ±1 · Clear — and **Clear resets it without removing the tool**.
The warning threshold is a setting; below it the figure turns red and pulses on a 2 s cycle
(reduced-motion gets a glow instead).

## 8 · Transitions

Seven, and they actually apply: Cut · Crossfade · Dissolve · Fade through black · Push left · Slide up ·
Materialise, with a duration picker feeding `--xd`. Only transform / opacity / filter are animated.
Choosing one replays it on the programme at once.

## 9 · Search

Two questions in one box, scored separately, **a reference always outranking a phrase**.

- **References**: full name, any prefix ≥ 2 letters (`psa`, `rom`), or a non-prefix alias (`jn`, `mt`,
  `php`); digits split from letters so `ps23:1` parses like `ps 23 1`; `chapter`/`verse`/`v`/`ch` dropped.
- **Phrases**: each word may land on a near word (shared prefix, or one edit), common words weigh 0.3,
  below 55% coverage it returns nothing rather than guessing.
- **One click does the whole job**: the verse goes to the programme, its chapter loads into the grid,
  that verse is the active slide. Each hit says why it matched.

## 10 · Library

Same grammar as Live: **collections across the top** (colour-coded, square, with counts), **items down
the left rail**, slides in the big grid, inspector on the right. Clicking an item opens its slides.

- **Songs**: sections as slides, each tagged with the key that fires it.
- **Reflow editing** (double-click, or Edit lyrics): the song is one piece of text, a blank line starts
  a slide, `[Chorus:c]` names the section and its key. Live read-out of the slides as you type. Two
  sections cannot share a key.
- **Section keys**: `v c b t i o`, numbered only when a kind repeats. On Live the key sends that
  section to the programme. A half-typed key wins the next keystroke so `v2` can be typed; panic keys
  are never shadowed; typing in a field fires nothing.
- **The section label is operator-only.** It rides on the slide for the grid, the card and the program
  label, and is suppressed on the way to the glass. Scripture is the opposite: the reference is content.
- **Media uploads**: a real file, read locally into the item, previewed before it is added, with a
  caption; the slide *is* the picture.
- **Announcements**: title (for the operator) separated from the words that reach the room.
- **No "add all to Live"** — a song joins a service through the plan.

## 11 · Settings

Eleven sections, merged from sixteen: General · Screens & looks · Audio · AI & Detection ·
Scripture & Languages · Network & Integrations · History & Backup · Shortcuts · Updates · Diagnostics ·
Privacy & Advanced. One type scale, three roles: **page title / standfirst / row**, with a footnote
behind a hairline. A list row is a name and a **value** — never an em dash standing in for one.

## 12 · Controls, sliders, switches

One instrument everywhere: a 3px track filled to the value with a 13px thumb (`--rp`, kept current on
render, on input and on every panel rebuild), colour wells as 38×21 swatches, switches the same size so
a mixed column lines up on one right edge. `accent-color` is not enough — it let every platform draw
its own idea of a slider.

---

## Implementation phases

Each phase ends green: `cd src-tauri && cargo test`, `npx vitest run`, `npm run build`.

1. **Tokens and chrome** — palette, radius, type scale, sliders, switches, colour wells, the four
   control colours. `src/app.css`, `docs/DESIGN_SYSTEM.md`. *Watch:* `tokencontrast.test.js` requires
   `--v-txt/--v-dim/--v-faint` as hex literals at ≥ 4.5:1; the colour-law class names are pinned by
   eight test files.
2. **Template model** — per-element properties, `migrate`, `resolve`, `slideBG`, measured auto-fit.
   `src/lib/TemplateRender.svelte` stays the one renderer. New tests beside `templatefit.test.js`.
3. **The object inspector** — tab strip, grouped properties, reset-per-object, new/duplicate/delete
   (deep copy; two-step delete, never `confirm()` — rule 41).
4. **Roles and the look register** — `LOOKS`, "Used for", screens that follow. Extends the existing
   content-look resolution (DECISIONS §29); `resolveOutputTemplate` is the one resolver.
5. **Lower thirds** — three templates, the band element, band-gives-ground, opacity fix.
6. **Stage monitor** — zones, geometry that cannot overflow, the stacked rail clock, the stage alert.
   New Tauri event + kiosk frame for the alert; it must never reach a congregation channel.
7. **Countdown** — one timer, one formatter, the transport, the warning threshold.
8. **Transitions** — seven, driven by the picker, reduced-motion honoured.
9. **Search** — reference parser + fuzzy phrase over the real corpus, scored through the existing
   detection helpers rather than a second parser. This one needs Rust tests: it is the same class of
   code as `detection.rs` and must never auto-fire anything.
10. **Library** — layout, reflow editor, section keys, media upload, announcements.
11. **Settings** — eleven sections, the type scale, values in the value column.
12. **SuperSource** — composite rendering as a template kind.

**Rules that bound every phase**: no native `confirm()`/`alert()`/`prompt()`; panic controls never
behind a validator and never scrolled out of reach; a status line that reads the same when broken as
when fine is not a status line; nothing a spoken control does may be invisible; and the section label,
the stage note and the stage alert must be unable to reach a congregation channel.

## Status

`REBRAND status` reads this table. One row per phase; a phase is **done** only when the three
gates in the phases section are green on it — `cargo test`, `npx vitest run`, `npm run build`.

**Third wave, 2026-09-14 — nine agents against a measured gap list, and three of the nine gaps were wrong.**
The lead rendered the console at 1600x1000 against the prototype, workspace by workspace, and briefed
eight agents (shell, Live, Library, Planner, Templates, Outputs, Settings, countdown) plus a ninth for
the transition control once `channels.rs` was clear. Assembled tree: **722 Rust** (0 failed, 16 ignored)
and **1729 frontend across 117 files**, with `cargo fmt`, `clippy -D warnings` and `npm run build` clean.

**What the wave built.** The chrome carries `RELAY studio`, the screen lamps and, for the first time,
the transition picker and its duration, applied end to end: a retained hub frame of its own (never
`last_screen`, which would erase the verse a late screen is shown), `output://transition` for the native
window, and one ranking function that says which authority answered (DECISIONS §84). **End service**
became the fourth control in the dock, driven by a new `recording` fact on `service_lock` read from
`Session` rather than from `engaged`, because an operator may lift the lock mid-service and a button off
`engaged` would say "nothing to end" over an open record. Live's 286px column is the AI's claims alone:
the manual reference box moved to the rail as §9's one box, the announcement to Quick tools, and the two
whole-room facts a lamp cannot compose stayed. Quick tools gained the name band. Pause is built on
`countdown_paused_ms`, and `countdown_from` is now WRITTEN, so §7's warning rule fires for the first time.
The Library's collection colour became one 3px edge. The Planner's cue inspector renders the cue rather
than a chequer plate. The Templates gallery's caption stopped clipping a name to three characters.
Settings' three binary rows became the one switch.

**Three defects nothing else could see.** `describeScreen`'s ok-branch still decided On Air from
`wall.live`, so a screen beating punctually whose own answer was `clear` read **On Air, in amber**: RG-129's
shape, and a blackout that did not land, and a `clear_screens` that returned `Ok` over a screen still
showing the last verse. The two claims must now agree or the badge says so and prints both.
**Deleting a service plan had no confirmation at all**, one click straight to `delete_plan`.
And Settings' measuring row read `lat?.enabled ?? true`, printing *on* over a backend that had never
answered. All three are rule 35 or rule 41 in a new coat.

**Two gaps the agents refused, with evidence, and they were right.** The prototype's cue-kind colour ramp
cannot be built: `colourlaw.test.js` names those exact six mappings as the defect it exists to stop, and
PR #73 removed them the same morning after measuring them on the running console. And Settings' rows stay
hairline seams inside one pane rather than eleven bordered cards, which wave 2 chose deliberately.
A third was withdrawn by the lead: the Library's empty chapter was the harness's own missing fixture data.

**What this wave does NOT change: the release decision, the model question, or word error rate.**

**Second wave, 2026-09-14 — the first pass driven by RENDERING the app rather than reading it.**
The prototype was opened in a headless browser and operated, workspace by workspace, and the
console was rendered beside it against a mock Tauri bridge; the gap list the seven agents worked
from was measured from those two renders, not inferred from the source. That matters because it
found things no suite could: the Templates gallery's cards rendered BLANK (and the first diagnosis
was wrong — the fixture was passing `layout`/`style` as JSON strings, which `db::Template` cannot
produce, and the real defect underneath was `templateKind` never having been taught the layer
model, so every one of the twelve starters classified as `custom`); the Planner printed a section
heading on EVERY row; a keyed template previewed against nothing was an empty black box on three
surfaces; and Live cut a screen's name in half — `Stream` / `ing` — inside the 286px column.
Each was fixed, re-rendered by the integrator, and only then called done. **What the wave did not
touch: the release decision, the model question, or word error rate.**

Shipped by that wave: six workspaces on the strip in the prototype's order (Themes folded into
Templates as a desk, Help still a real route — `qa-inventory` says nothing became unreachable),
screen lamps and a transition-free chrome, a status bar whose every figure can say `no data`, four
equal 178px docks, a slide grid of RENDERED slides, the AI's claims in the 286px inspector with no
percentage on a guess, Outputs as cards that repaint what each screen is showing, a Library of real
slides with one row of chrome, a Planner of drag rows with a cue inspector, a Settings reading
column with the last two dead controls removed (DECISIONS §69's tally: eleven), and the roles
register reconciled against what actually renders (DECISIONS §78–§81).

**Reconciled on 2026-09-14 by the integrator**, against the combined branch with all seven
workspace branches merged. Two agents had already found this table wrong in BOTH directions: W2
found the collection rail and reflow editing built while the row said neither existed, and W7 found
three §9 claims absent that the row implied were half done. Three more rows were found claiming
work absent that another agent had since built. **Evidence is now per-FILE counts, never a whole
suite total**: ten rows quoted a suite figure that was true on one agent's branch and stale the
moment anything else merged, which is exactly how the four registers came to disagree
(`CLAUDE.md`, "do not restate any of the four in a fifth place"). Whole-suite figures live in
`docs/qa/QA_HARNESS.md` §0 beside the command that produces them. After the second wave the
combined branch runs **709 Rust** (0 failed, 16 ignored) and **1604 frontend across 114 files** —
and that figure was measured on the assembled tree, because **not one of the seven agents' own
suite totals survived the merge**.

| # | Phase | State | Evidence |
|---|---|---|---|
| 2* | Workspace grammar (§2 — **no phase number in the brief**) | **done** | the sidebar became a 34px chrome bar with the workspaces in it, the footer became a 26px status bar, a dock row (audio · transcript · quick tools · controls) lives in the SHELL, and the studio split is two equal monitors either side of a 118px take column. The **slide grid** and **single click to Programme** are built too: `slidegrid.js` (`planCells` · `passageCells` · `gridSource` · `pressArbiter`, `PRESS_MS` 190) rendered by `Live.svelte`'s `.sgrid`, single press sends and double previews. `slidegrid.test.js` 19, `slidegridwiring.test.js` 9. **This row said "not built" until 2026-09-14 and was wrong.** |
| 1 | Tokens and chrome | **done** | palette, radius, type scale, one slider / switch / colour well, the four control colours. `tokencontrast.test.js` 7, `rangefill.test.js` 13 |
| 2 | Template model | **done** | `templatemodel.js` (migrate · resolve · slideBG · fit estimate), migration on three doors, the renderer reads the model. `templatemodel.test.js` 37 |
| 3 | The object inspector | **done** | object tab strip (measured: 17 objects wrap onto 8 rows, none past the edge, nothing behind a scrollbar), ONE Position group with real numbers, Duplicate (a deep copy that is now load-bearing), Reset this object, two-step Delete. `layerops.test.js` 14, `templateinspector.test.js` 3, `band.test.js` 24 |
| 4 | Roles and the look register | **done** | a screen may follow the content look (DECISIONS §70), "Used for" on the template, a tag on each gallery card. `e2e::r4_a_screen_may_follow_the_content_look`, `e2e::r4_a_following_screen_wears_a_different_look_for_each_kind` |
| 5 | Lower thirds | **done** | three starters (Name · Lyric · Scripture), each keyed, each its own template. A non-hex shape fill no longer paints black. The band is now a real `band` layer running to the bottom edge, naming the words inside it (`members`), and **giving ground** before they shrink. `band.test.js` 24, measured in the browser. DECISIONS §75 |
| 6 | Stage monitor | **done** | a word to the preacher (new `stage_alert` hub message, stage-only by contract, and NEVER a retained frame — `channels::tests` holds `("stage_alert", false)` so a preacher's private message cannot replay to a lobby TV joining late), the reading can no longer push the clock off the top, **switchable zones** persisted per device (`relay.stage.zones`) and the **stacked rail clock**. `e2e::r5_a_word_to_the_preacher_reaches_the_stage_and_not_a_rehearsal`, `e2e::r5_a_word_to_the_preacher_reaches_no_congregation_channel`, `stagezones.test.js` 9, `screenpreview.test.js` 7. **This row said zones and the clock were not built until 2026-09-14 and was wrong.** |
| 7 | Countdown | **done** | one formatter (`formatCountdown`), one warning rule, and now ONE reader of how long is left (`countdown.js::countdownRemainingMs`) — the wall, the stage page and the console all go through it, because the subtraction acquired an exception. **Pause IS built**: `countdown_paused_ms` on `OutputContent`, held and released through `adjust_countdown`, which also carries Reset and ±1 so a re-aim can never drop the hold. `countdown_from` is now WRITTEN by `start_countdown`, so §7's short-countdown warning rule finally fires. `countdown.test.js` 31, `countdownwiring.test.js` 13, `e2e::r7_*` 7, `layers.test.js` 32, `templatestyle.test.js` 29. **The button landed in wave 3**: `Dock.svelte`'s transport row draws Start · Pause · Reset · ±1 · Clear, wired to `countdownPress('pause'/'resume')` and `capture.js::pauseCountdown`, with `paused` passed to `countdownCan` so a `+1` cannot release a hold. Pause and Resume are two actions rather than one toggle, because a toggle computed from state the caller might hold stale is how a press does the opposite of what it says |
| 8 | Transitions | **done** | seven in one register (`transitions.js`), played by the renderer, migrated from the three old names, reduced motion is a cut. DECISIONS §71. `transitions.test.js` 15. **Wave 3 gave it an operator**: the chrome bar carries the picker and its duration, the override rides its own retained hub frame (never `last_screen`, which would erase the verse a late screen is shown), the native window gets `output://transition`, and the duration control is disabled while a template is followed rather than offered as a number that changes nothing. `resolveTransition` is the ONE place the two authorities are ranked and it reports which answered. DECISIONS §84 |
| 9 | Search | **done** | glued digits parse, a literal hit must cover 55% of the query, and nothing a search does reaches a screen (DECISIONS §72). `search.rs` — one pure module, five named match kinds, **every hit says why it matched** and a guess says so in words with no percentage; a ≥2-letter book prefix resolves, **search-only** (`detection.rs` was not opened, and `e2e::r9_nothing_a_search_offers_can_reach_an_auto_fire` states rule 10 at the boundary); one click takes a hit the whole way. `search::tests` 8, `e2e::r9_*` ×9, `livesearchrail.test.js` 11 |
| 10 | Library | **partly done** | the operator label reaches the record again and still not the glass (DECISIONS §73); announcements say which fields the room sees; the collection rail and reflow editing were already built when the previous line said they were not. **Section keys** (`v c b t i o`, numbered on repeat, per song, `[Bridge:g]` to ask for one), the key printed on the slide it fires, **media looked at before it is added** with the name the operator gives it, and the dead selection tick box off three panes. DECISIONS §76. `sectionkeys.test.js` 42, `medialook.test.js` 4, `db::giving_a_section_its_own_fire_key_flags_the_arrangement_rather_than_repointing_it`. **`b` is BLACKOUT, so a Bridge is on `r`** — CLAUDE.md beats the spec's alphabet. **The caption is CLOSED WITHOUT A COLUMN** (P2, wave 4): `media_assets.filename` is already the operator's own words — the add sheet writes what they typed into it and keeps the real file name only as a hint on screen — so a caption would be a SECOND operator-authored string beside the one that exists, and §10's own sentence for media is *the slide is the picture*, which means none of it reaches a congregation. The card's second line is `collections.js::mediaSub` instead — the kind and the date added, which are facts the name and the thumbnail could not already say, and the date is printed exactly as stored because an ISO date with no time run through `toLocaleDateString` is the day before it west of Greenwich. `librarysubline.test.js` 8. **Still not built: a key that fires from the Live tab's own grid** — and P2 looked at it, did not build it, and leaves the answer rather than a guess. What IS settled: `assignKeys` / `resolveKeystroke` are pure and tested, `Cell` carries a `tag`, and `gridSource` has three sources, of which exactly ONE has a clean namespace — `source: 'song'` is a single hand-picked song, so the song owns the letters exactly as it does in `LyricsPane`, and `stageSong` already holds `full.sections`, which is what `assignKeys` wants. `source: 'plan'` still has no answer: the grid is flat across every cue, two songs in one plan both want `c`, and nothing says which owns the namespace. **The obstacle that stopped P2 is a SECOND one, and it is the harder of the two**: `shortcuts.js` calls `preventDefault()` and dispatches to `ctx.sectionKey` whenever the handler is registered at all, and `Live.svelte` registers its context ONCE at mount, before any await, deliberately and pinned by `liveunmount.test.js`. So adding `sectionKey` to that registration swallows every `a`–`z0`–`9` keystroke on the run surface for a whole service, staged song or not — the exact thing the branch's own comment forbids (*a dead branch must not eat a keystroke the browser had a use for*). The two ways out are both bigger than a parity detail and neither may be chosen quietly: either `sectionKey` returns whether it CONSUMED the key and `shortcuts.js` defers `preventDefault` until it says so — a contract change inside the file that owns the panic keys — or Live re-registers its context reactively when `grid.source` changes, which is last-writer-wins on one global slot on the run surface, and is the shape of the bug `liveunmount.test.js` exists for. Whoever takes it: answer the plan namespace FIRST, decide the `preventDefault` contract SECOND, and print the key only on the cells it actually fires — a key printed on a slide that does nothing is the cheatsheet-that-lies failure `sectionkeys.js` spends a paragraph avoiding. And one thing to say out loud when it lands: the grid's press path arms a send on a **190 ms** double-click timer (`pressArbiter`), so a key that fires at once and a click that fires after a beat are two latencies on one cell |
| 11 | Settings | **done** | eleven sections merged from eighteen (two deleted as duplicates, not merged); the three roles now come from `WorkspaceFrame` rather than from five private copies; Theme and a twice-offered Reset removed (DECISIONS §69, taking the tally to nine); Screens & looks says which screens actually follow a content look (§70). DECISIONS §77. `settingssections.test.js` 19 |
| 12 | SuperSource | **done** | a `region` layer that is its own container, a depth cap, a built-ins-only inner template, the SuperSource starter and its inspector block. DECISIONS §74. `composite.test.js` 9 |

**Where phase 1 departed from the prototype, and why.** Both are cases of the rule in Context —
the repository wins where the two disagree:

- **The muted text step.** The prototype's `#7e8695` measures 4.25:1 on `--v-surf` and 3.77:1 on
  `--v-surf2`, below WCAG AA on two of the four surfaces muted text sits on.
  `tokencontrast.test.js` (RG-74) exists precisely to catch that, so the token ships at
  `#8c94a1` — same hue, 4.52:1 at worst.
- **Radius.** Section 1 says "2px everywhere" and then "3px corners on cards"; the prototype,
  which was measured rather than sketched, uses 3px with 5px on containers. That is what
  shipped, and `--v-r-round` survives only for a slider thumb, a status dot and a switch.

**Phase 2 departed from the spec once, and added one thing it did not ask for.** The spec's
auto-fit is an estimate; this repository already MEASURES the box in the DOM, which is strictly
better, so the estimate became the loop's seed rather than its replacement — same answer, far
fewer forced reflows, and the first real use of the aspect argument. And `bgStyle` would have
been a property no control could set, so the theme editor gained a Background style row (and
`THEME_STYLE_KEYS` gained the key, without which the control would have saved into a draft the
wall never receives).

**Phase 3's object sets by kind are not all here yet.** The tab strip lists the objects a
template actually has, which is the whole set for a full-screen look. The kind-specific starters
the spec names — a lower third's **Band**, a stage monitor's **Zones**, a composite's **Camera /
Word bg / Word / Ref** — arrive with the phases that build those kinds (5, 6 and 12). Listing
them now would be a tab strip offering objects no renderer draws.

**And the strip was only ever in the EDITOR.** The surface an operator browses from — the gallery's
inspector — was a preview, three buttons and five read-only rows, two of which were facts that
could be read and not changed from the surface they were read on. One of those printed *"Content
looks are set in Outputs → Content looks"*, a signpost standing where a control belongs. **Used
for** now binds from the inspector through the same single writer, and the **object strip** names
the template's real objects there, a press from that object's own properties. The per-object
**property groups** stayed in the editor, deliberately and at a stated cost: DECISIONS §80 records
what the extraction would take and why it is not a job for the tail of another pass. §3.2 is
therefore closed in the editor and **half** closed in the gallery, and is to be read as exactly
that.

**Phase 4 found the reason the look register existed and did nothing.** `set_channel_template`
took a non-null id, so every screen always had a template of its own — and since a screen's own
template wins over a content-type default (§29), the content-look map could be filled in, saved,
and change nothing on any screen in the building. The sentence under the picker said the
opposite of what the code did, which is how it survived. A screen can now be set to **Follow the
content look**; §29 is unchanged for a screen that has one.

**The ten roles are not all here** — written when phases 5, 6 and 12 were still ahead, and
**reconciled on 2026-09-14 now that they have landed** (DECISIONS §78). Two registers were being
run together and the reconciliation separated them:

- The register an operator **binds** is still `CONTENT_KINDS` — scripture, songs, media,
  announcements, timer — because that is what the fire path, the database and the matrix speak. It
  is unchanged, and so is §29's resolution order.
- The register that says what a template **is for** is derived from the template's own shape and
  stored nowhere. It is what the rail's rows and a card's tag read.

And what the reconciliation actually found was not the register. `templateKind` — the one function
every rail row, card tag and inspector row reads — had only ever been taught the legacy REGION
model, so **all twelve starters classified as `custom`**, the three lower thirds and the SuperSource
composite included. A role could have a renderer, have a row, and never appear on it. The five
seeded built-ins are region templates, which is the only reason the rail showed a role at all: the
defect was invisible on a fresh install and total for anything an operator created.

Of the spec's ten: seven are offered; the three lower-third variants collapse to **one**, because
`lower.name` and `lower.bible` are the same shape and only the words pointed at them differ;
`preservice` is **refused**, because nothing in a shape identifies one and a row for it would claim
templates it cannot tell apart. `timer` is offered and the spec's list omits it. The rule that a
role offered before its renderer exists is a control saving a setting nothing reads is intact — and
it now has a second edge: **a role you can filter to but not create is the same defect in the other
direction**, which is what `Songs` was until a `lyrics` starter landed beside the built-in that had
always been there.

**Phase 5 was left deliberately incomplete, and W4 built the concept it was waiting for.**
The reason it was deferred stands as written: "the band gives ground before the words do" means a
shape grows when a text layer beside it needs room, and in this repository a template is a flat
list of independently placed objects with no parent, no child and no "these two belong together".
The shortcut it refused — a rule that fires when a shape happens to be *named* `Band` — was the
right thing to refuse.

What W4 added is the missing relationship, declared rather than inferred: a `band` layer type
whose `members` array names the ids of the objects inside it. It is in the saved file, it survives
a rename, an object in no band is in no band, and every template that never opted in is untouched
by the whole mechanism. The band is then a real element (§4): it runs from `top` to the BOTTOM
edge, inset by the side safe area, with the baseline lift as bottom padding and its words centred
in what is left — measured in a browser as equal gaps above and below, not asserted. It climbs by
at most 16 points, never past a third of the frame, and only while the type would otherwise fall
below 78% of its designed size; a short name at the same setting does not move it.

A member is **not** drawn inside the band element. The band computes the boxes and the words are
drawn by the one text path every other layer uses, because two text paths is how a shadow, a
transform or a fit fix lands on one kind of layer and not the other. The editor's canvas reads the
same derived geometry through `drawBoxes`, so a selection handle cannot sit where the words are
not. Zones (phase 6) and a composite's regions (phase 12) can use the same shape; nothing here
assumes a lower third.

**One thing the new list broke and the tests caught**: `duplicateLayer`'s deep copy had never
actually mattered — no layer property was an object or an array, so a shallow spread would have
behaved identically. `members` is the first one that is, and `structuredClone` alone is exactly
wrong for it: it copies the ids faithfully, so the duplicate points at the ORIGINAL's words. Both
bands would lay the same two objects out and editing either would move the other's type. Deleting
a word now tells its band, too, or the band keeps a dead id in its saved JSON for ever while
rendering perfectly.

**The opacity defect the spec names is not this repository's.** The prototype capped a band's
alpha at 0.9×; nothing here does — `hexA` applies the alpha exactly. What IS here is worse and
was found while checking: a shape fill that is not a hex (a gradient, a CSS var, a theme token
that resolves to one) was parsed two characters at a time with a fallback of 0 per component, so
it rendered as **black** at the requested alpha. Silently, and looking deliberate. The layer most
likely to carry a gradient is a lower-third band, and that is the layer keyed over a live camera.

**Phase 6 built the half that carries a guarantee.** "A word to the preacher" is a new hub
message rather than content, which is what makes *no congregation screen can show it* a property
of the system: `r6-contracts.test.js` requires every hub message to have an explicit verdict per
client, and the output page's verdict is `false`. It is suppressed in a rehearsal like every
other publisher here — the defect `stage_next` had once, where the wall does not move so the
sandbox looks intact while the preacher's own tablet is handed a practice message. It is
deliberately **not retained** (rule 43 retains what decides what a screen is SHOWING): a tablet
reconnecting ten minutes later must not be handed an instruction meant for a moment that passed.

**One deviation, on purpose.** The spec says the stage clips; this scrolls the reading inside
itself instead. On a platform monitor the two are identical because the reading is sized to fit,
and on the preacher's PHONE — which is the other thing this page is — clipping would take the end
of a passage away from the person reading it aloud. What the spec is really asking for is that
the header cannot be pushed off the top, and that is what changed.

**Switchable zones and the stacked rail clock ARE built** (corrected 2026-09-14; this paragraph
said they were not). The blocker recorded here was that zones had nowhere to persist. They persist
per DEVICE, in `localStorage` under `relay.stage.zones`, which is the right home: a stage tablet
and a lobby TV are different screens with different jobs, and a preference that followed the
service record would make one of them wear the other's choices. `stagezones.test.js` holds that
the choice survives a reload.

**Phase 7 removed a second clock.** The m:ss arithmetic lived twice — the wall and the
preacher's phone each had their own copy. They agreed, which is exactly what made it worth
deleting: two clocks that agree are indistinguishable from one clock right up until somebody
fixes a rounding edge in one of them. Both also stopped at minutes, so a 90-minute pre-service
countdown read `90:00`.

**The warning is a rule, not a setting, and that is on purpose for now.** The spec asks for a
configurable threshold. The place that control belongs is the Settings pass (phase 11), and a
setting with no screen to set it on is precisely the defect phase 4 closed. The rule is the last
minute, or the last tenth of a countdown shorter than ten minutes — a minute's warning on a
two-minute countdown is a colour that is lit for half its life, and a colour that is always on
says nothing. Reduced motion gets the glow without the pulse: the information is the colour.

**Pause is built now, and the field is the whole story** (this paragraph said it was absent, and
said why; the why was right). `countdown_to` is an absolute instant that rides with the content, so
nudging it is re-aiming that instant, which the transport did with no backend change. **Pausing is
genuinely different**: a paused countdown is not an instant at all, so it is said by a field the
engine owns — `countdown_paused_ms`, the ms left at the moment it was held. Every reader shows that
figure instead of ticking, and `countdown_to` stays set only as where the countdown would land if it
were resumed, so the content still reads as a countdown to `preflight`, to the retained screen frame
and to the slide key.

**Three things that shape fell out of, each of which would otherwise have been a defect.** *(a)* The
subtraction now has an EXCEPTION, and it lived in three places — the wall, the stage page and the
console each did their own `countdown_to - now`. Three copies of one subtraction is survivable;
three copies of one with an exception is not, because the copy that has never heard of the hold goes
on counting while the other two hold, and one of the three is the congregation's. `countdown.js`
owns it once. *(b)* Reset and ±1 re-broadcast the countdown, and the console used to rebuild that
broadcast out of its own mirror — label, done message and template read back off the event. That
works while every caller remembers every field, and forgetting THIS one restarts a timer the
operator deliberately stopped, from a button that says "+1". So the engine keeps the countdown
(`channels::CountdownState`, noted at the same three doors as `WallState`) and `adjust_countdown`
changes one thing about it; it can never create one, which keeps Start the only control that puts a
countdown in front of people. *(c)* The hold rides in the kiosk wire form, so a screen that rejoins
mid-service is sent a held countdown rather than a running one (rule 43). Clear already existed (the
screen clears).

**`countdown_from` is written now too.** It was read by `TemplateRender` and written by nothing, so
§7's short-countdown warning — the last tenth of a countdown under ten minutes — had never once
fired in the product. `start_countdown` stamps it and no re-aim re-stamps it, because re-stamping
would shrink the warning window to whatever is left each time somebody pressed a button.

**Phase 8 found two halves of a feature that each looked finished.** The theme editor offered a
transition and a duration, both saved; the renderer ignored them and said so in a comment — a
control that changes nothing, documented instead of fixed. And `layers.js` carried
`slideRevealCss`: three modes, its own test, no caller. A helper with tests and nothing rendering
it reads exactly like working code. Transitions now play, the default is a cut (which is what an
operator asked for), and the old helper is deleted rather than left beside the new one.

**Phase 9 measured the one function nobody had measured.** `search_verses` — the Planner's box,
the Library's search and the preacher's remote all go through it — had no tests, and a probe found
two defects immediately: `ps23:1` returned an empty list, and a query with one real word in five
returned nineteen confident verses. Both are fixed and pinned; the second one is the search-shaped
version of the defect this product exists to prevent.

**"Each hit says why it matched" IS built** (corrected 2026-09-14; this paragraph said it was not,
while the Status row two pages up said it was). It did change the shape of what `search_scripture`
returns, and the way it changed it is the point: the verse row is `#[serde(flatten)]`ed inside a
`SearchHit`, so all four readers keep reading the fields they read before and the explanation is
purely additive. `e2e::r9_a_hit_is_still_a_verse_row_on_the_wire` pins it. DECISIONS §72.

**Phase 10 took the half of the Library that was a defect, not a redesign.** The rule that a
song's section label must not reach the glass was held at the backend and implemented AGAIN in
Live, which passed an empty label — so the wall was right and the service record could not name
what had been on it. One rule, one home; both halves are now tested.

**The rest of §10 was called "not started" and most of it was already built** (corrected
2026-09-14). The collections rail and reflow lyric editing existed when that sentence was written;
section keys and media-look-before-add landed on 2026-09-14. What is genuinely not built is a key
that fires from the **Live** tab's own grid, and a caption stored apart from an item's name. See
the Status row for why the first is design rather than wiring.
Section keys in particular add a global keystroke path that can put content on a wall, next to
the panic keys — that is `shortcuts.js` territory, where this repository has already had one
bug of exactly that shape (Escape wiping the wall from behind a menu), and it deserves its own
pass rather than the tail of another.

**Phase 11 took the row-level defect and left the reorganisation.** Settings printed `—` for the
LAN address, the installed version and the recognition language, and an empty value there has
three different causes an operator needs to tell apart: not fetched yet, the fetch failed, and
genuinely nothing there. One glyph over three situations is rule 35 — and the same shape as RG-83,
where "up to date" was printed over a channel that had been 404ing for months.

**Merging sixteen sections into eleven is a reorganisation, not a repair**, and it moves every
control an operator has learned where to find. It is worth doing with somebody watching the
screens rather than at the end of a long pass.

**Phase 12 is the container-inside-a-container concept phases 5 and 6 were waiting for.** A
`region` layer renders a real template inside its own `container-type: inline-size` box, so `cqw`
inside it is a share of the region rather than of the frame. That is the primitive band-gives-
ground and the stage's stacked rail clock both need; neither is built on it yet, and building them
on it is now a smaller job than it was.

**A review pass over this branch's own diff found one more, and it is the same shape as the defect
phase 4 fixed.** `Copy URL` wrote `template_id=1` for a screen that follows the content look, and
the output page read a missing `template_id` as 1 — so a follower's OBS source wore a built-in
while the operator's own window followed correctly. The URL was also built twice in the same
component, four lines apart, with only one copy corrected. One builder now (`lib/outputurl.js`),
tested, and `ipc.test.js` fails if a view starts building its own again.

**§2 had no phase number, and that is why twelve finished phases did not look like the prototype.**
The brief's implementation list covers the subsystems; the LOOK of Relay Studio is mostly §2's
workspace grammar, which the list never mentions. It is now partly built — chrome bar, workspaces,
dock row, status bar, equal monitors — and what remains is the slide grid and single-click-to-air.
That last one changes what a click does on the surface that puts scripture in front of people, so
it is a pass of its own rather than the tail of a layout commit.

**A visual audit compared the running console against the prototype on 2026-09-13, and the palette
and type scale MATCH — byte-identical ground, panel and take-amber, 12px Inter throughout. What
differs is structure, and this is the list, so the next pass starts from it rather than
rediscovering it:**

- **No screen ladder and no transition control in the chrome bar.** The prototype's right-hand
  chrome carries five screen lamps (Main · Streaming · Lobby · Stage · Overflow), a transition
  picker and its duration. The app has a clock and an emergency stop.
- **The dock's rhythm is wrong.** The prototype's four cards are equal (377px) and 178px tall on a
  darker trough; the app's are unequal and 151px, flush to the ground.
- **The Library item area is a text list**, where the prototype renders a grid of slide
  thumbnails with reference-plus-first-line captions.
- **Library reaches an item through four rows of chrome** (collections, Bible/Saved, filters,
  panel header) against the prototype's two.
- **AI detection is a card in the middle row**, not the 286px right-hand inspector the prototype
  gives it.
- **`Go Live` is amber, deliberately.** §1 of this brief puts green on Go Live and saves amber for
  End service. This repository's own rule is different and older: `.r-btn.amber` is reserved for
  "a control that puts something in front of a congregation, or takes it away", which is exactly
  what that button does. It is a divergence from the prototype, NOT a colour-law violation, and it
  was left alone rather than changed by somebody who could not see the screens.

**A limit of the new instrument — widened in wave 4, and still a limit.**
`workspacegrammar.test.js` asserted no raw hex and no px font sizes over its `DESKS` array of six.
It now carries a second block covering **53 components**, in three tiers, because the three defects
do not have the same blast radius and one list would have to be the smallest of them:

- **no pill** and **no hand-typed copy of a scale step** hold over *every* component, because after
  the wave-4 sweep there are none left anywhere. The type scale is read out of `src/app.css` rather
  than restated in the test, so the assertion cannot drift from the thing it is about.
- **no raw hex** holds over a named list of **43**, and **no literal radius** over **34** of those.
  A file is on a list because it has none; a file that needs an exception is in the paragraph
  below, with its reason, rather than in an allowlist inside the test.

Each of the five assertions was watched to fail with its defect reintroduced — including the inline
`style="font-size:12px"` variant, which is where four of them were hiding from a stylesheet-only
scan. **What it still does not cover** is written into the test as well as here: off-scale font
sizes and off-scale radii (neither is a conversion — see below), `rgba()` (not scanned at all, and
seven retired-amethyst glows in `Splash.svelte` were found by eye, not by it), and template
content, which is deliberately out of scope.

**Carried forward, deliberately — rewritten after wave 4's token sweep, which paid most of it
down.** Phase 1 owned `src/app.css` and made the shared layer token-only. What phase 1 could not
reach was the components, and this paragraph used to record that debt: "about sixty radius
literals, twenty of them `99px`".

**Paid.** Every `99px`/`999px` pill is gone — **eleven** of them. (The wave-4 brief said thirteen.
Grepping the base for `border-radius: ?9{2,3}px` under `src` returns eleven, and the number in this
paragraph is the one that was measured rather than the one that was handed over.) They were in
`LiveRail`, `ModelSetup`,
`DetectionInspector`, `BrandMark`, `Arrangements`, `History`, `Help` and `Live`. Each was judged
rather than swept: the five shapes that are genuinely round (a slider thumb, a status dot, a switch
knob, a scrollbar thumb, and the 2px bars of the brand mark) kept their shape and now ask for
`--v-r-round`; the rest were pills by accident and are `--v-r-sm`. **"No pills" is now true of every
rendered screen, not only of the design system.** Also paid: **95** font sizes that were a scale
step typed as a number, **9** radii that were a token's value typed as a number, and every raw hex
in **43** components.

Three things the sweep found that were not on its list, all of them chrome painted from a palette
that no longer exists:

- **`--f-mono` was declared twice in `src/app.css`**, as `'JetBrains Mono'` in the legacy `:root`
  and `'IBM Plex Mono'` in the design-system one. Same specificity, later wins, so §1's figure face
  was already what shipped and the legacy line was dead — but a reader looking it up found the wrong
  answer first, and reordering the two blocks would have silently changed every clock, confidence
  and latency in the app. Resolved in a real DOM to confirm which won, then reduced to one
  declaration. JetBrains Mono stays in the bundle: `TemplateEditor` offers it to templates BY NAME,
  and that is a choice an operator saved into a slide.
- **`Arrangements.svelte` fell back to an `--r-*` token family `app.css` does not define**, so
  eleven `var(--r-dim, #8b8f98)`-shaped literals were not fallbacks at all — they were the live
  paint, from the pre-rebrand palette.
- **`Splash.svelte` glowed in the retired amethyst** (`rgba(139,92,246,…)`, seven times; the live
  token is `#a96bf5`). `ModelSetup` and `DetectionInspector` did the same with the old amber,
  emerald and cyan as raw `rgba()`; those had exact `--v-*-soft` / `--v-*-line` tokens and now use
  them. No scanner here finds an `rgba()`, which is why this is written down.

**Still owed, and deliberately not forced.** Two classes remain, and both are a *decision* rather
than a conversion — converting one restyles a screen, which is not what a token sweep is for:

- **~101 off-scale font sizes** (8, 8.5, 9, 10, 10.5, 13, 13.5, 15, 16, 18, 22, 26px). None is in
  `--v-fs-*`. The heaviest are `Live.svelte` (17), `History.svelte` (14), `FirstRun.svelte` (8) and
  `TemplateEditor.svelte` (8). Either the scale grows steps or these screens move to it; that is
  §11's call, not a sweep's.
- **39 off-scale radii** (2, 4, 7, 8, 9, 10, 11, 12, 13px), in 18 components. Note the awkward one:
  **§1 says "radius is 2px everywhere" and `--v-r-sm` is 3px**, so the eleven literal `2px` corners
  are simultaneously what §1 asks for and not a token. That contradiction is §1's to settle.

**Eighteen components still carry a raw hex or a literal radius, each for a judged reason** — a wall
preview that is really black (`Live`, `Output`, `VerseDeck`, `TemplateEditor`), `#fff` as ink on a
filled destructive button (`Help`, `Live`, `FirstRun`, `VerseDeck`), `#000` as a *mask* channel
rather than a colour (`Splash`), a gradient's second stop the palette does not publish (`Help`'s
`#c8302f`, `Stage`'s alert red), and a chip that documents why it carries its own ground (`Live`'s
`#cfd6e2`). `src/lib/crash.js` is exempt from all of it, on its own stated premise: it renders when
the stylesheet may not have loaded, so a token there is a blank panel at the worst possible moment.

**One thing phase 1 changed that the spec did not ask for.** Blackout on the run surface wore
`--v-grey`, and grey means CUED. The control that takes the wall to black shared a colour with a
position marker, so it is now black with a hairline, per the four-control rule in section 1.

---

## Verification

- Drive the packaged app the way we drove the prototype: the browser harness in
  `relay-browser-audit-harness` memory (mock Tauri bridge for the console, the real backend for the
  output and stage pages).
- Per phase: no overflow on any template at any setting; every control changes what it claims to;
  every preview matches the wall.
- Repo gates: `cargo fmt`, `clippy -D warnings`, both suites, `npm run version:check`, and the doc
  consistency tests (`crossrefs.test.js`, `relaygap.test.js`).
- Work lands as a PR, never a direct commit to `main`.

---

# Dispatch — the agent briefs

Nine agents. **Two foundations run first and alone** (everything else compiles against them), then six
workspaces in parallel, then one integrator. Each returns a branch and a review note; nobody merges.

## A · The preamble every agent inherits verbatim

> You are building one part of Relay's rebrand. The design is settled and proven — it was operated and
> measured as a working prototype. Your job is to bring it into the real app without inventing,
> widening or softening it.
>
> **Read first, in this order:** `docs/REBRAND.md` (the specification — your section is named in your
> brief), `CLAUDE.md` (the forty-three rules; they win over anything in the spec), `docs/DECISIONS.md`
> for any §  your brief cites, and the file list in your brief. Do not read the whole repo.
>
> **Sources of truth.** `docs/REBRAND.md` describes WHAT. `CLAUDE.md` describes what may not change.
> Where they disagree, `CLAUDE.md` wins and you say so in your review note. The prototype is a
> reference for behaviour and proportion, not for code — it shares no code with Relay.
>
> **Hard rules, all of them from real failures:**
> - `src/lib/TemplateRender.svelte` is the ONE renderer. Do not fork it, do not add a second.
> - Sizes are **cqw**. Never px, never vw.
> - No native `confirm()` / `alert()` / `prompt()` — the webview does not implement them, so a guard
>   built on one guards nothing and reports success (rule 41).
> - Panic controls (`Esc` clear, `B` blackout, Clear screens, Blackout) may never sit behind a
>   validator, may never be scrolled out of reach, and may never report a success they did not achieve
>   (rule 15, DECISIONS §20).
> - The colour law is fixed: amber = ON AIR, amethyst = rehearsal, cyan = a guess, grey = CUED. Eight
>   test files pin those token and class names as literal strings. Do not rename them.
> - `--v-txt` / `--v-dim` / `--v-faint` stay hex literals in `src/app.css` at ≥ 4.5:1 on
>   `--v-void/--v-bg/--v-surf/--v-surf2` (`tokencontrast.test.js`).
> - Nothing an operator types for themselves — a section label, a stage note, a stage alert — may reach
>   a congregation channel.
> - Loading, switching workspace or editing a template may never change what is on the programme.
>
> **Definition of done.** All four green, run by you, output quoted in your note:
> `cd src-tauri && cargo test` · `npx vitest run` · `npm run build` · `cargo fmt --all && cargo clippy
> --all-targets -- -D warnings`. Plus: every new behaviour has a test, and **each test was watched to
> fail** with the change reverted. A test you did not watch fail is a theory you did not test.
>
> **Deliverable.** One branch, `rebrand/<your-area>`, off `main`. Open a PR; do not merge, do not
> commit to `main`. End with a review note in this shape:
> `SCOPE` what you built · `EVIDENCE` the four command outputs and the tests you watched fail ·
> `DECISIONS` anything you resolved that the spec left open · `NOT DONE` what you deliberately left,
> and why · `RISK` what a reviewer should look at hardest · `BLOCKED` anything you could not verify.
> Report honestly. "NOT TESTED" is an acceptable answer; a claim without evidence is not.
>
> **Scope discipline.** Touch only the files your brief names, plus tests. If you need a change in
> someone else's area, write it in `DECISIONS` and leave it alone. Do not reformat files you are not
> changing. Do not bump versions.

## B · Wave 1 — foundations (sequential, in this order)

**A1 · Tokens and chrome** — `docs/REBRAND.md` §1, §12.
Files: `src/app.css`, `docs/DESIGN_SYSTEM.md`, plus the token tests.
Build: the palette and 2px radius, the type scale, the custom range slider (3px track filled to value
via `--rp`, 13px thumb, hover/active/focus), 38×21 colour wells, switches sized to match, and the four
control colours (Go Live green · End service amber · Clear screens red · Blackout black). Update
`DESIGN_SYSTEM.md` in the same PR — it is the document those choices are cited from.
Acceptance: contrast test green; no control in the app still renders a platform-default slider; a
column of mixed controls lines up on one right edge at 1280 and at 900px wide.

**A2 · The template model** — §3, §3.4.
Files: `src/lib/TemplateRender.svelte`, `src/lib/themes.js`, `src/lib/layers.js`, new
`src/lib/template.js` if a pure module helps, and tests beside `templatefit.test.js`.
Build: per-element properties, the one-time migration that deletes the legacy whole-template keys, the
background-style builder, and the measured auto-fit with per-face advance, taking a real aspect for
nested contexts. Keep `needsRefit()` and rule 42's two-pass refit exactly as they are.
Acceptance: an old template renders identically after migration; no template overflows its box at any
setting; the fit's aspect argument is exercised by a test with a non-16:9 box.

## C · Wave 2 — the workspaces (parallel, each off A2)

Every brief below inherits §A, cites its spec section, and owns its view file.

**W1 · Live** — §2, §7, §8.
`src/lib/views/Live.svelte`, `src/lib/shortcuts.js`, `src/lib/stores/capture.js`.
The run surface: search rail, Preview/TAKE/Program with the programme mirroring the main screen,
the slide grid, single-click-to-air with the 190 ms double-click guard, Quick tools (countdown · name
band · word to the preacher), the seven transitions, the four-colour control dock.
Acceptance: the programme survives every workspace switch and every plan load; `Space`, `Esc`, `B`,
`R`, `←`/`→` behave exactly as `shortcuts.test.js` requires; a transition set to Cut animates nothing.

**W2 · Library** — §10.
`src/lib/views/Library.svelte` (or the current file), `src/lib/songs`-adjacent helpers, `songs.rs` if
parsing moves.
Collections across the top colour-coded and square, items down the rail, slides in the grid, the reflow
editor, section keys (`v c b t i o`, numbered on repeat, per song), media upload read locally into the
item, announcements with the operator title separated from the words.
Acceptance: a section key fires that section on Live and nothing while a field has focus; a two-line
slide round-trips through reflow; **the section label never renders to an output**; no "add all to
Live" path exists.

**W3 · Planner** — §2, §10.
`src/lib/views/ServicePlanner.svelte`, `src/lib/plan.js`, `src/lib/cues.js`.
The running order in the new chrome; nothing here may reach an output (that is the whole point of the
tab). Arrangement staleness (rule 39) is untouched.
Acceptance: a plan built here runs unchanged in Live; `cargo test` plan/arrangement tests green.

**W4 · Templates** — §3.2, §3.3, §4, §6.
`src/lib/views/TemplateGallery.svelte` + the inspector components.
The object tab strip, grouped properties, reset-per-object, new/duplicate (deep copy)/delete (two-step,
in-app), the roles register and "Used for", the three lower thirds with the band element and
band-gives-ground, and SuperSource as a template kind.
Acceptance: editing one band changes no other; a duplicate is independent; a screen set to *follow the
content look* renders scripture, lyrics and announcements through three different templates without
anyone touching it.

**W5 · Outputs and the stage monitor** — §5, §3.3.
`src/lib/views/Channels.svelte`, `src/Output.svelte`, `src/Stage.svelte`, `src-tauri/src/channels.rs`.
Screen cards previewing what that screen really shows, *Follow the content look* in the picker, the
stage zones, geometry that cannot overflow, the stacked rail clock, and the stage alert as its own
frame kind.
Acceptance: the alert reaches the stage page and **no congregation channel** — prove it with a hub-level
test in the shape of `nothing_reaches_the_stage_monitor_during_a_rehearsal`; rule 43's retained frame
still replays to a client that joins mid-service.

**W6 · Settings** — §11.
`src/lib/views/Settings.svelte`, `src/lib/views/Dashboard.svelte`.
Eleven sections, the three type roles, values in the value column, no duplicated control, no section
that is three rows on a full-height page. Every control must move something real or not exist
(DECISIONS §69).
Acceptance: no setting writes a preference nothing reads; the update line still cannot say "up to date"
when no check has run (`updatechannel.test.js`).

**W7 · Search** — §9.
`src-tauri/src/detection.rs` adjacent, or a new pure module; `src/lib/views/Live.svelte` for the rail.
Reference parsing and fuzzy phrase matching, scored so a reference always outranks a phrase, one click
loading the chapter and putting the verse on the programme.
Acceptance: Rust tests for `ps 23 1`, `ps23:1`, `psalm 23`, `rom 8 28`, `mt 6 33`, `1 cor 13 4`,
`see ye first the kingdom` → Matthew 6:33, `lamp unto my feet` → Psalms 119:105. **Nothing here may
auto-fire** — search is an operator action; rule 10 is untouched.

## D · Wave 3 — the integrator

**I1 · Assemble and review.** Takes the seven branches, resolves collisions in `app.css` and the store,
runs the full gate on the combined branch, drives the packaged app through the browser harness
(`relay-browser-audit-harness`), and files one report: what matches the spec, what drifted, what is
still owed. It opens the final PR. It does not merge.

## E · Launch order

Run A1 → A2 alone. Then W1–W7 together. Then I1. Do not start wave 2 before A2's PR exists, or seven
agents will each invent their own template model.
