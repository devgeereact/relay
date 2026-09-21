> **Archived 2026-09-21.** Historical. Wave 5 (shelf, names, seal) landed on 2026-09-16 and was audited in `docs/qa/audits/DESIGN-2026-09-16-WAVE5.md`; rulings are in `docs/DECISIONS.md` §87 to §90; the name register is `src/lib/names.test.js`. Its statement that waves 3 and 4 had not landed was true that day and is not now. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Design — the shelf, the names and the seal (Wave 5)

**Status:** approved by the operator on 2026-09-16, before any code was written.
**Predecessor:** `docs/archive/2026-09-15-timers-templates-stage-design.md`,
whose waves 0, 1 and 2 have landed and whose waves 3 (Timers) and 4 (Stage and
Planner) have not.

**This wave runs BEFORE waves 3 and 4, not after.** Nothing in it needs the timer
registry, and wave 3 keeps `countdown_*` as the wire form, so the templates this
wave seeds survive it unchanged. Where this design amends the predecessor, the
amendment is written down in §0.3 rather than left to be discovered.

---

## What prompted this

Ten requests from the operator, taken in one sitting after wave 2 was assembled
and driven in a browser. They are not ten unrelated items; they are four
questions wearing ten coats.

1. **What does a church actually find on the shelf?** Thirty-seven seeded
   templates across two rendering models, six of which have "High Visibility" in
   the name, and three open register rows (RG-139, RG-140, RG-141) saying that
   what a fresh install renders is not what the designer drew.
2. **Is a thing called the same thing everywhere?** One concept carries six
   labels today. `stage_alert` is "Word to the preacher"; `stage_note` is "Stage
   note" in the Planner, "Operator note (monitors only)" in the template editor
   and "Note" on the stage page.
3. **Does the app's own skin stop at the glass?** It does not. Seeded template
   styles store `var(--f-serif)`; `TemplateRender` falls back to the console's
   amber; `app.css` ships its legacy unscoped rules into `output.html`.
4. **Does a control do what it says?** "Create new template" creates one before
   you have saved anything. Arming a layer delete paints the word `Sure?` into a
   fixed 20px box. Running a timer silently attaches the words "Service begins
   in", which no interface can change.

---

## 0.1 The four decisions this design rests on

Each was put to the operator as a question with its cost stated, and answered.

**One — the shelf is rebuilt whole, at forty.** All thirty-seven seeded rows go.
Forty arrive: eight roles × five. Scripture · Song · Media · Announcement · Timer
· Scrolling Lower Third · SuperSource · Stage. Every one is layer-model, unique,
and clears one contrast floor rather than concentrating legibility in a single
family.

**Two — a seeded row gains an identity the conversion cannot destroy.** RG-142
measured the previous retirement at **0 of 21** against a copy of a real
database, because it matched on bytes and `TemplateGallery.upgradeLegacyToLayers`
had already rewritten them. "Delete all thirty-seven" cannot be built that way.
Two columns are added: `templates.seed_key` and `templates.edited_at`.

**Three — an output channel gains an explicit role.** "Main screen" is currently
a frontend guess (`Live.svelte:1357`, the first channel whose `render_target` is
`native_window`). Rename or delete that channel and Live's programme pane falls
through to `channels[0]` in silence. A role column makes it a setting — and it is
also the door that makes Stage Message safe in a template, which §3 explains.

**Four — a fresh install ships starter content.** This reverses
`db/mod.rs:662`'s *"NOTHING SEEDS DEMO CONTENT HERE, and nothing ever may."* The
reversal is deliberate, is the operator's to take, and gets a numbered DECISIONS
section of its own when it lands.

## 0.2 Constraints every track respects

These are not restated per track. A track that appears to need an exception has
found a design problem, not a rule problem.

- **`pipeline::Fire` stays the only way to build output**, `broadcast_with_clock`
  stays the only caller of `channels::broadcast_content` (rule 36), and the
  pre-air validator stays where it is.
- **Rule 10 is untouched.** Nothing here moves a threshold, adds an auto-fire
  route, or touches `detection.rs` or `router.rs`.
- **Rules 37 and 42 are untouched.** The 45% legibility floor stays a ratio of the
  designer's own size; `TemplateRender` keeps shrinking and showing rather than
  blanking; the two-try refit bound stays.
- **Rule 41**: no native `confirm()` / `alert()` / `prompt()`. The unsaved-template
  guard in §2 is an in-app two-step, not a browser dialog.
- **Rule 43**: a screen joining mid-service is shown what is on the screens. No
  new frame introduced here may be retained into `last_screen`, and
  `is_screen_frame` is a `contains`, not a `starts_with`.
- **Rule 15 and rule 44**: nothing added here may paint over `Clear screens` or
  take `Esc` without giving it back.
- **DECISIONS §29 and §70 are not reversed.** The resolution order — transparency
  law, pinned cue template, the screen's own, the content look, the configured
  default — is unchanged by every track in this wave.
- **DECISIONS §35 is not reversed.** The kiosk hub still records nothing about who
  connected. §3's per-channel filter therefore lives at the receiver.
- **DECISIONS §78 is not reversed.** A template's role stays derived from what it
  renders. The forty names carry a kind prefix for findability; nothing reads the
  prefix to decide a role.

## 0.3 Where this amends the predecessor spec

Three statements in `2026-09-15-timers-templates-stage-design.md` no longer
describe what is being built. They are amended here and the predecessor is left
as the record of what was true when it was written.

| Predecessor | Said | Now |
|---|---|---|
| §2.4 | "The seed becomes twenty-five", five families × five kinds | Forty, eight roles × five. The family idea survives only as a visual through-line, not as a seeding rule |
| §2.5 | "Both rows are real features and neither is deleted" | The **"Used for"** row is deleted from the template editor. The per-screen `layout.shows` switch list stays. `Channels.svelte` keeps the one authoritative content-look matrix, so the feature is not deleted — its second writer is |
| §3 (Wave 3) | The timer design keeps `label` as a field on the timer | Still true, and the DOCK stops supplying one. §7 below |

---

# Track A — the shelf becomes forty

## A.1 Why the count is not the point

A fresh install currently holds thirty-seven templates from three sources:
`builtin_templates()` (5, region model), `theme_templates()` (25, four of them
layer model), and `shelf_templates.json` (7, layer model). Thirty of the
thirty-seven are region-model, which is the fact underneath all three open
rendering rows:

- **RG-141** — a fresh install's countdown takes the region branch and paints a
  `312 × 43` blob: 30.3px digits against 192px designed.
- **RG-140** — `Classic · Announcement` paints a footer ticker before the
  Templates workspace is opened and a mid-screen crawl after, switched by a
  silent one-way conversion.
- **RG-139** — a running countdown makes every other text layer paint its
  declared size instead of its fitted one, clipping 91px of a label on
  `Stage · Large type`.

RG-140 and RG-141 both name the same repair and call it a decision rather than a
patch: **decide whether a seeded row's region form or its converted form is the
product.** This track answers that. The converted form is the product, and the
region form stops shipping. That closes RG-141 and RG-140 at the source — not by
fixing the region fitter, but by no longer seeding anything that reaches it.

RG-139 is **not** closed by this track and must not be claimed as closed. It is a
fit-loop defect on the layer path, reproducible on any template with a
`countdown`-bound layer, and this track seeds five of those on purpose (`Timer ·
*`) plus one more (`Stage · Rail`). **Track A therefore widens RG-139's blast
radius and must say so in the register when it lands.** Either RG-139 is fixed
first, or the Timer group ships with the mitigation the row already names. This
is called out here rather than discovered during implementation.

## A.2 The identity problem, and the two columns

`ensure_retired_presets_are_gone` compares `region_config_json` and `style_json`
against frozen bytes. Two independent rewrites defeat it, either fatal alone:
`upgradeLegacyToLayers` converts and **saves** on mount, with layer ids from
`newId()` (which folds in `performance.now()`); and `style_json` returns through
`serde_json`'s BTreeMap alphabetically ordered against the seed's hand-written
key order. Measured result, against a real pre-wave database: 31 in, 57 out,
**0 of 21 retired**.

RG-142's own closing sentence is the design constraint: *"Anything surviving the
conversion needs an identity the conversion preserves, not a byte comparison."*

**Two columns on `templates`:**

```sql
seed_key   TEXT,     -- set by the seeder, never by an operator. NULL = hand-made.
edited_at  TEXT      -- stamped when a save does not come from the seeder.
```

- `seed_key` is a stable slug (`scripture.dayspring`, `timer.monolith`). It is
  written once, at insert, and `upsert_template` never changes it — an operator
  editing a seeded row keeps the key and gains an `edited_at`.
- The migration that installs it back-fills by name against the OLD seed lists,
  which is the one moment name-matching is correct: it runs before anything in
  this wave has renamed a row, and a name collision at that instant is exactly
  the case RG-142 says the migration should decline.
- **Retirement becomes**: delete rows whose `seed_key` is in the retired set AND
  whose `edited_at IS NULL` AND which nothing points at through the four doors
  `ensure_retired_presets_are_gone` already checks (content look ×5, configured
  default, `output_channels.template_id`, `plan_items.template_id`).
- A row the operator edited survives, which was always the right half of the old
  rule. A row they never touched goes, which is the half that never worked.

**Idempotence and retryability, rule 25.** The migration adds columns and
back-fills in one `unchecked_transaction`; the scratch-table hazard does not
arise because nothing is rebuilt, but the rollback path is still explicit and the
test asserts a second run is a no-op and a third is identical to the second.

**`retired_presets.json` is superseded, not deleted.** It stays as the frozen
record of the twenty-one, and its byte comparison stays for installs that never
opened Templates — where it is the only thing that works, and where it is
correct. The `seed_key` path is additive.

## A.3 The forty

Eight roles, five each. Names are `Role · Name`, unique across all forty. The
prefix is for finding, not for deciding: **`templateKind` still derives the role
from what the template renders**, DECISIONS §78 unchanged, and
`templateKind.test.js` names the role each of the forty lands on so a seed that
drifts is caught by a failing test rather than quietly becoming Custom.

Every one of the forty:

- is **layer model**;
- declares **`layout.shows` explicitly** (the predecessor spec's §3.6 trap 7 — a
  template with a narrow `shows` list drops content at `Output.svelte:195`
  *before* `resolveOutputTemplate` ever runs);
- clears **7:1 body-text contrast against its own ground**, measured by the same
  helper `legibility.test.js` uses today. This is the "high visibility as a
  floor, not a family" decision: the accessibility family becomes the accessibility
  *standard*, and `Contrast` variants exist where a church needs the 21:1 maximum.
  **What "its own ground" means for a keyed or translucent template**, because the
  floor is otherwise unmeasurable: the ground is the band's declared plate colour
  **at full alpha**, never the camera behind it. Relay cannot know what a camera is
  pointed at, and a contrast figure that pretends otherwise would be a number that
  lies — which is worse than no number (rule 18). A translucent band is therefore
  held to its plate, and `Scroll · Glass`'s alpha is bounded so that plate still
  clears the floor at the alpha it ships with;
- names a **real font family**, never `var(--f-serif)` — see Track E;
- carries **no law colour** as a band or ground: not amber (ON AIR), not amethyst
  (rehearsal), not cyan (a guess), per rule 18 and DECISIONS §21.

### Scripture — verse and reference

| Name | `seed_key` | Design |
|---|---|---|
| Scripture · Dayspring | `scripture.dayspring` | Warm ivory serif on deep charcoal; reference beneath, small, tracked, right-aligned |
| Scripture · Meridian | `scripture.meridian` | White display sans on pure black, centred. The 21:1 maximum |
| Scripture · Vellum | `scripture.vellum` | Dark ink on a cream ground — the one light-ground scripture look, for daylight rooms |
| Scripture · Indigo | `scripture.indigo` | White sans on deep indigo with a vertical gradient; reference in the same white, not a tint |
| Scripture · Column | `scripture.column` | Left-aligned with a hairline rule; reference above the verse, not below |

### Song — words only, no reference at all

Per `docs/REBRAND.md` §4: a congregation is not singing the title. None of the
five binds `reference`.

| Name | `seed_key` | Design |
|---|---|---|
| Song · Anthem | `song.anthem` | Large centred display sans, white on black |
| Song · Chorus | `song.chorus` | Ivory on deep plum, generous leading |
| Song · Open | `song.open` | White on dark teal, wide measure, lightest weight that still clears 7:1 |
| Song · Plain | `song.plain` | Pure black, tightest margins, the largest type in the set |
| Song · Ember | `song.ember` | Warm white on near-black brown, serif |

### Media — a `media` layer, and as few words as the kind allows

| Name | `seed_key` | Design |
|---|---|---|
| Media · Full | `media.full` | The picture fills the frame. Nothing else |
| Media · Caption | `media.caption` | Picture full-frame, one caption line on a solid plate at the foot |
| Media · Frame | `media.frame` | Picture inset on a ground plate with a hairline outline |
| Media · Split | `media.split` | Picture one half, words the other, via a `region` at real aspect |
| Media · Wash | `media.wash` | Picture full-frame under a scrim so type over it stays legible |

### Announcement — a full-screen notice

Not scrolling. Scrolling is its own role below, which is what keeps
`templateKind`'s announcement rule (a text layer that SCROLLS) from claiming
these.

| Name | `seed_key` | Design |
|---|---|---|
| Announce · Board | `announce.board` | Title over body, left aligned, strong hierarchy |
| Announce · Card | `announce.card` | Centred card on a ground, generous padding |
| Announce · Bold | `announce.bold` | One line, as large as the frame allows |
| Announce · List | `announce.list` | Title over stacked lines |
| Announce · Contrast | `announce.contrast` | White on black, the 21:1 maximum |

### Timer — a placed timer layer

Each carries a real layer bound to `countdown`, so none takes the `.cd-default`
fallback and none takes the region branch. `Timer · Monolith` is the one the dock
fires at by default: digits and nothing else, which is §7's whole point.

| Name | `seed_key` | Design |
|---|---|---|
| Timer · Monolith | `timer.monolith` | Digits alone, full frame. No label layer at all |
| Timer · Rail | `timer.rail` | Digits with the clock beneath |
| Timer · Titled | `timer.titled` | A `reference`-bound label above the digits — the one a plan cue's label lands in |
| Timer · Panel | `timer.panel` | Digits on a panel over a ground |
| Timer · Contrast | `timer.contrast` | White on black, the largest digits in the set |

### Scrolling Lower Third — announcements while the camera is live

A `band` layer with declared `members`, plus `scroll` on the text. These are the
five the operator asked for: notices that can run during a live service and are
equally correct on a stream, a kiosk page or a lobby TV.

| Name | `seed_key` | Design |
|---|---|---|
| Scroll · Banner | `scroll.banner` | Solid band, one crawling line |
| Scroll · Ribbon | `scroll.ribbon` | Thin band, a fixed label chip at the left and the crawl beside it |
| Scroll · Glass | `scroll.glass` | Translucent band at a declared alpha — the body sits at exactly the set value, per REBRAND §4 |
| Scroll · Bold | `scroll.bold` | Tall band, large type, for a room that reads from the back |
| Scroll · Clear | `scroll.clear` | Transparent ground, band only. Keys cleanly for OBS and for an ATEM fed over HDMI |

**The transparency law applies to all five** (`resolveOutputTemplate`, keyed
templates): a keyed template is never replaced by an unkeyed one, or a lower third
becomes a full-screen slide over a live camera.

**A crawl and reduced motion.** `prefers-reduced-motion` turns the animation off
and leaves `text-overflow: ellipsis` over `white-space: nowrap`, which **silently
truncates a notice**. That is exactly why the old `High Visibility · Announcement`
carried no `scroll`. So each of the five declares a reduced-motion fallback that
**pages** rather than truncates: the text steps through in full-width pages on a
timer rather than crawling. A notice that cannot be read in full is not an
accessible notice, and an ellipsis is a silent failure of the kind rule 35 exists
to refuse.

### SuperSource — a `region` layer whose fill is a real template

Per DECISIONS §74: a composite may not be another composite's fill, a region may
name only a built-in, and the region is its own container so the inner template
scales to the region exactly as it would to a screen of that width.

| Name | `seed_key` | Design |
|---|---|---|
| Source · Word right | `source.wordright` | Camera left, words right |
| Source · Word left | `source.wordleft` | Words left, camera right |
| Source · Tall | `source.tall` | Camera above, words below |
| Source · Inset | `source.inset` | A small camera inset on a word slide |
| Source · Even | `source.even` | Equal halves |

### Stage — monitor-only bindings

These render on a screen whose channel role is `stage`. Four of the five carry
bindings no congregation template renders (`next`, `note`, `elapsed`,
`remaining`), and one carries the new `stage_message` binding from §3.

| Name | `seed_key` | Design |
|---|---|---|
| Stage · Reading | `stage.reading` | The reading takes what is left; clock beneath |
| Stage · Next | `stage.next` | Reading with the next reference under it |
| Stage · Message | `stage.message` | Reading with a Stage Message zone that fills the screen when one is live |
| Stage · Rail | `stage.rail` | Reading with the figures beside it in a rail of their own |
| Stage · Plain | `stage.plain` | The reading alone, at the largest size the frame allows |

## A.4 Evidence this track owes

- `preset_template_count()` returns 40 and the assertion beside it says so.
- `the_bare_fixture_is_a_first_launch_and_nothing_more` (`qa.rs`) is updated to
  the new contents in the same commit that changes them, never after.
- `templateKind.test.js` names the derived role of every one of the forty.
- A contrast test walks all forty and fails below 7:1 — the instrument
  `legibility.test.js` already has, pointed at the whole shelf instead of one
  family.
- A test asserts **no seeded row is region-model**, which is what keeps RG-141
  closed against a later "tidy-up".
- A test asserts **every seeded row declares `layout.shows`**.
- Retirement: a fixture that **converts before it retires**, which RG-142 names as
  the missing instrument — every existing test in that module inserts the frozen
  bytes verbatim and so has never asked the question this wave is answering.

---

# Track B — Create New Template stops creating one

`TemplateGallery.svelte:322`:

```js
async function newFrom(starter) {
    const t = starter.make();
    const id = await saveTemplate({ name: starter.label, layout: t.layout, style: t.style });
    selId = id;
    dispatch('edit', { id });
}
```

The row exists before the editor opens. Abandon the editor and it stays — and
because the seed inserts by name, a gallery can accumulate several
`Countdown Timer` rows that a church never asked for.

**Build.** `newFrom` builds the starter in memory and dispatches a **draft** —
`{ id: null, name, layout, style }`. The editor renders a draft exactly as it
renders a saved row, with three differences:

1. the header says the template is unsaved;
2. **Save** inserts (and from then on the editor holds a real id);
3. **Discard** closes without writing anything.

Leaving a dirty draft raises an in-app two-step, never `confirm()` — Tauri's
webview returns `false` from `confirm()` without showing anything, which is rule
41 and the reason a two-step delete once deleted nothing while reporting success.

**Duplicate and import keep their current behaviour.** Both are acts on content
that already exists; a duplicate the operator asked for is not a draft.

**Evidence.** A test that mounts the gallery, chooses a starter, closes the editor
without saving, and asserts `save_template` was never called — written to fail
against today's code before the fix lands.

---

# Track C — channel roles, Main Screen, and the door Stage Message needs

## C.1 The guess

```js
$: mainChannel = channels.find((c) => c.render_target === 'native_window') ?? channels[0] ?? null;
```

`Live.svelte:1357`. `output_channels` has no primary column;
`channels.rs:149`'s `pub primary: bool` is a **monitor** property and unrelated.
So the programme pane's idea of the main screen is a render-target heuristic over
a seeded name, and it degrades silently.

## C.2 The role column

```sql
role TEXT CHECK (role IS NULL OR role IN ('main','stage'))
```

Seeded: `Main screen` → `main`, `Stage display` → `stage`. `Streaming` and
`Lobby screen` stay `NULL`. At most one row may hold `main` — enforced in the
helper, not by a partial index, because the enforcement has to produce a readable
error rather than a constraint violation. Several rows may hold `stage`: a church
may have a confidence monitor and a preacher's tablet.

Live reads the role instead of guessing, and the programme pane names the screen
it is rendering. `Outputs` gains the picker.

## C.3 Stage Message in a template, safely

Today `channels::stage_alert` publishes to **every** kiosk client through
`publish_kiosk` (`channels.rs:1149`). It is stage-only because `Output.svelte`
has no handler for `stage_alert` — a guarantee kept by omission. Adding a
`stage_message` binding to the template model means a renderer would start
reading it, and the omission stops protecting anything.

**Build, and the order matters:**

1. A new binding `stage_message` in `layers.js`, labelled **Stage Message**.
2. `Output.svelte` accepts a `stage_alert` frame **only when its own channel's
   role is `stage`**. The filter is at the receiver because the hub records no
   client identity and DECISIONS §35 is not being reversed; the output page already
   knows its own channel, because the URL is channel-keyed (DECISIONS §29).
3. The value never rides on `OutputContent`. It stays its own frame kind, held in
   renderer state, so it cannot travel on a content frame to a congregation
   screen.
4. `FRAME_VERDICTS` keeps `("stage_alert", false)` — never retained, so a private
   message cannot replay to a lobby TV joining late (rule 43).
5. The rehearsal gate is unchanged: `stage_alert` is suppressed in a rehearsal
   like every other publisher beside it.

**Evidence.** `e2e::r5_a_word_to_the_preacher_reaches_no_congregation_channel`
gains a sibling that drives a channel whose role is **not** `stage` and asserts
the frame paints nothing — the surface that was missed, per the repository's own
"a guarantee is only kept on the doors you checked".

**Settle and write down** what the predecessor spec §3.4 left open: whether
`alert` surviving a panic control on the stage page is intended. It currently
survives because the `clear`/`black` branch at `Stage.svelte:406-438` resets six
fields and not that one, and nothing in the tree records a reason. It is a silent
third answer to a question §27 asked and wave 3 answers. This wave does not need
to change the behaviour; it needs to stop it being an accident.

---

# Track D — the names

## D.1 The register

| Concept | One name | Where it must read that way |
|---|---|---|
| `stage_alert` — typed live, stage only, not saved | **Stage Message** | Dock card heading and aria, the `stage_message` binding, the stage page, any status line |
| `stage_note` — saved on a cue | **Stage Note** | Planner field, the `note` binding label (today "Operator note (monitors only)"), the stage page zone |
| `stage_next` + `next_*` | **Up Next** | Live's control, the stage zone, the binding labels |

Neither of the first two becomes the other. They behave differently — one is
persisted per cue and one is a live instruction — and one name over two behaviours
is the defect this track exists to remove, not a tidier version of it.

## D.2 The instrument

A `names.test.js` that holds, for each concept, the single user-visible string and
the files allowed to contain it — failing if a second label for the same concept
appears anywhere in `src/`. The same shape as `ipc.test.js`: a scanner that reads
every `src/` file rather than a hand-written list, plus a test that the scanner
can still see a known instance, because **a scanner that quietly narrows passes
everything** and this repository has had that exact failure twice.

## D.3 Template names

The forty `seed_key`s are stable and machine-facing; the forty display names are
unique and human-facing. The gallery rail, the gallery card, the inspector, the
Planner's cue template picker, Outputs' per-screen picker and Live's programme
pane all render the same string — which is already true, because all of them read
`templates` from one store. What this track adds is the test that keeps it true.

---

# Track E — the seal

Three leaks, both directions, all measured.

1. **App chrome reaching the wall through the template's own data.** Seeded
   `style_json` stores `"font":"var(--f-serif)"` (`db/templates.rs:136`),
   `var(--f-display)` (`:141`) and `var(--f-body)` (`:152`). Those are declared
   only in `src/app.css:54-55`, and `--f-display` has already been re-aliased once
   from Space Grotesk to Inter — an app-chrome edit that silently changed the
   typeface of every template naming it. **The forty name real families.**
2. **App chrome reaching the wall through the renderer.**
   `TemplateRender.svelte:1358` falls back to `var(--v-amber)` for `--accent`. A
   congregation screen inheriting the operator console's ON AIR amber is rule 18
   in the one place it is least visible. **The fallback becomes a template-side
   default.**
3. **The console stylesheet shipping to the congregation.** `src/output.js` and
   `src/stage.js` both `import './app.css'`, and Svelte does not scope a global
   stylesheet, so legacy unscoped rules (`.prev-main .verse`, `.prev-stage
   .verse`, `.prev-stream .lower-third`, `.prev-lobby .verse`, `.tmpl-row.active`,
   `.toggle.on`) are live on `output.html`. `app.css:35-37` records that the
   removal was blocked on *"eyes on a running app, and this machine cannot
   screenshot one"* — which Track I's browser pass supplies. **The tokens stay
   shared; the rules go.**

What is **not** being done: the design tokens are not forked. Output and stage
keep importing the token block and the self-hosted fonts. The seal is against
rules and against template data, not against the palette.

---

# Track F — the editor stops painting text over text

## F.1 The armed delete, reproduced in source

`TemplateEditor.svelte:1144` renders `{armedDelete === L.id ? 'Sure?' : '✕'}` into
`.te-lmini`, which is `width:20px; height:20px` with no `overflow`
(`:1937`). The only `.armed` rule in the file is `.te-objacts .armed`
(`:1826`) — scoped to the **inspector** row at `:1363`. So the layer row's armed
button gets neither the red confirm styling nor any extra width, and five
characters land in a 20px box inside `.te-layerlist{overflow-y:auto}` (`:1859`),
which computes `overflow-x: auto`. The CSS comment at `:1933-1936` claims this
state is covered. It is not.

## F.2 The other two

- **The object tab strip is unbounded inside a clipped pane.** `.te-objtabs`
  (`:1813`) wraps with no `flex`, no `max-height` and no `overflow`, inside
  `.te-pane{overflow:hidden}` (`:1809`). Only `.te-designbody` can shrink
  (`:2024`), so on a template with many layers the strip grows and the properties
  body collapses toward zero.
- **The add-layer menu is absolutely positioned inside that same clipped pane**
  (`.te-addmenu:1847` within `aside.te-pane.te-layers:1050`). `TemplateGallery`
  already abandoned this construction for `position:fixed` after measuring it
  (`TemplateGallery.svelte:183-186`, `:914`). Reported as an asymmetry; the
  browser pass in Track I decides whether it clips in practice.

## F.3 Evidence

Measured in a real layout engine, not argued — the same method the 2026-09-10 and
2026-09-16 passes used, because both of the worst defects those passes found were
invisible to every static instrument while `qa-inventory` reported zero problems
and was right.

---

# Track G — the timer runs bare

`Dock.svelte:625`:

```js
? startCountdown(r.broadcastMs / 60_000, 'Service begins in', 'Welcome')
```

Hard-coded, and there is no interface anywhere to type a different one. The label
is **payload**, not template: it rides in `content.reference` (`main.rs:2825`), and
a template's `reference`-bound layer is what draws it.

**Build.**

- The dock's Start passes **no label and no done-message**. Digits alone, wherever
  a timer object sits. `Timer · Monolith` has no label layer at all, so it is
  unchanged either way; the other four simply render an empty label box, which the
  fit loop already handles.
- **The Planner keeps both fields**, and gains the interface it never had:
  `addCountdownCue` currently writes the same two constants
  (`ServicePlanner.svelte:289`) with nothing to edit them. A cue that wants words
  beside the clock says so, and fires them through `Live.svelte:600-607` exactly as
  today.
- `capture.js:1429`'s defaults become empty strings rather than the two constants,
  so a caller that omits them gets a bare timer rather than a surprise.

**What does not change.** `countdown.js::countdownRemainingMs` stays the only
arithmetic. `countdown_from` keeps being written, so the warning rule keeps firing.
`adjust_countdown` still cannot create a countdown. None of the sixteen countdown
tests changes shape.

---

# Track H — the Content Looks section leaves the template editor

`toggleUsedFor` (`TemplateEditor.svelte:530`) and the **"Used for"** block
(`:1670-1690`) are deleted. That block is the editor's second writer of the
content look; `Channels.svelte:854` is the authoritative one and keeps the matrix.

**What stays, and why the distinction matters.** The row immediately beneath it —
"Content this template renders" — is `layout.shows`, a **per-screen allow-list**,
not a content look. They were two visually identical five-chip grids over the same
five labels, which is how the first click on one came to look like it had
activated all five on the other. Wave 2 gave the second one a different control
shape. This wave removes the first.

**Nothing in the engine changes.** `tpl_{kind}`, `set_content_template`,
`ContentTemplates`, `cue_or_content_tpl` and the content-look link inside
`resolveOutputTemplate` are all untouched. DECISIONS §25, §29 and §70 stand.

---

# Track I — starter content on a fresh install

## I.1 The reversal, stated plainly

`db/mod.rs:662-664` says *"NOTHING SEEDS DEMO CONTENT HERE, and nothing ever
may."* `qa.rs:257-274` enforces it, asserting `COUNT(*) == 0` for
`service_plans`, `songs`, `announcements`, `saved_scripture` and `media_assets`.

The operator has taken the opposite decision, and it gets a numbered DECISIONS
section of its own when it lands. The reasons worth recording:

1. An empty install is not neutral. A church opening Relay for the first time has
   no announcement to fire, no background to choose and no plan to run, so the
   surfaces that exist to be operated cannot be operated at all — which is how a
   volunteer decides a workspace is broken.
2. The instruments this repository trusts are all about **drift**, not about
   emptiness. The tripwire's value is that it fails when the seed changes without
   anybody saying so. That value is kept by asserting the starter set **exactly**,
   not by asserting zero.
3. Starter content is not demo content. `demo.rs` stays exactly as it is, behind
   its Settings button, writing no services, transcripts, detections, cues,
   `service_events` or `perf_samples`.

## I.2 What ships

- **Announcements** — a small set a church can fire or edit on the first Sunday,
  with the operator-facing title separated from the words that reach the room
  (REBRAND §10).
- **Backgrounds as real media** — the 34 files already bundled at
  `src/backgrounds/` become `media_assets` rows, so a background is reachable from
  the Library and from a template's media layer rather than only from the editor's
  build-time picker. This closes the gap the template-editor background picker has
  today: it offers only `BACKGROUNDS` and cannot reach `media_assets` at all.
- **One example service plan**, carrying a countdown cue with its own label and
  duration — which is also the first thing that exercises Track G's Planner fields
  and wave 3's `plan_items.duration_sec`.

## I.3 What the tripwire becomes

`the_bare_fixture_is_a_first_launch_and_nothing_more` keeps its name and its job.
It stops asserting zero and starts asserting the starter set by identity: this
many announcements with these keys, this many media rows, one plan with this
shape. A seed that drifts still fails it. **It is not deleted, and it is not
weakened to a range.**

---

# Ordering and verification

Tracks A, B, F, G and H are independent of each other and of C, D, E and I.
Track C must land before the `stage_message` binding is offered anywhere. Track A
must land before Track I's example plan pins a template by `seed_key`.

Every track ends green on both suites, using **the runner's own summary line** and
never a grep:

```
npm run build
cd src-tauri && cargo test
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings
npx vitest run
```

`npm run build` runs before the Rust suite on a fresh tree, or two `channels`
tests fail on a bare 404 because `dist/` is gitignored (RG-127).

Every new test is verified to **fail when the defect it describes is
reintroduced**. This repository has had a test that passed against a broken file
because it grepped a comment, and a regression test written against a diagnosis
that turned out to be wrong, which passed with the supposed fix reverted.

**This wave needs a browser-driven pass against the real backend**, for the same
reason waves 2 and 4 do: forty new templates, a new band-with-crawl rendering, a
reduced-motion paging fallback and three editor-layout fixes are all things no
static instrument in this repository has ever been able to see. `qa-inventory`
reported zero problems throughout the two passes that found the worst defects, and
was right both times.

## What this change set does not claim

- It does not move the release decision. Word error rate remains unmeasured in
  every language, neither platform has a code-signing certificate (RG-73), and
  nobody but the author has run a service.
- It does not touch detection, the router, or any threshold. Rules 10, 28, 30 and
  34 are untouched.
- It does not close RG-139. Track A widens that row's reach by seeding six
  templates with a `countdown`-bound layer, and the register must say so.
- It does not unpark NDI, and it adds no SDI integration.
- It does not reverse DECISIONS §29, §35, §70 or §78.
