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

| # | Phase | State | Evidence |
|---|---|---|---|
| 2* | Workspace grammar (§2 — **no phase number in the brief**) | **partly done** | the sidebar became a 34px chrome bar with the workspaces in it, the footer became a 26px status bar, a dock row (audio · transcript · quick tools · controls) lives in the SHELL, and the studio split is two equal monitors either side of a 118px take column. **Slide grid and single-click-to-air not built** |
| 1 | Tokens and chrome | **done** | palette, radius, type scale, one slider / switch / colour well, the four control colours. `tokencontrast.test.js` 7 green, `rangefill.test.js` 9 new, suite 974 |
| 2 | Template model | **done** | `templatemodel.js` (migrate · resolve · slideBG · fit estimate), migration on three doors, the renderer reads the model. 34 + 6 new tests, suite 1014 |
| 3 | The object inspector | **done** | object tab strip (measured: 17 objects wrap onto 8 rows, none past the edge, nothing behind a scrollbar), ONE Position group with real numbers, Duplicate (a deep copy that is now load-bearing), Reset this object, two-step Delete. `layerops.test.js` 14, `templateinspector.test.js` 3, `band.test.js` 24 |
| 4 | Roles and the look register | **done** | a screen may follow the content look (DECISIONS §70), "Used for" on the template, a tag on each gallery card. `e2e::r4_a_screen_may_follow_the_content_look`, suite 1034 / cargo 668 |
| 5 | Lower thirds | **done** | three starters (Name · Lyric · Scripture), each keyed, each its own template. A non-hex shape fill no longer paints black. The band is now a real `band` layer running to the bottom edge, naming the words inside it (`members`), and **giving ground** before they shrink. `band.test.js` 24, measured in the browser. DECISIONS §75 |
| 6 | Stage monitor | **partly done** | a word to the preacher (new `stage_alert` hub message, stage-only by contract), the reading can no longer push the clock off the top. `e2e::r5_a_word_to_the_preacher_reaches_the_stage_and_not_a_rehearsal`, cargo 669, suite 1043. **Switchable zones and the stacked rail clock not built** |
| 7 | Countdown | **partly done** | one formatter (`formatCountdown`), one warning rule, read by the wall and the stage. `layers.test.js` +8, `templatestyle.test.js` +2, suite 1053. **Pause / ±1 / Reset not built** |
| 8 | Transitions | **done** | seven in one register (`transitions.js`), played by the renderer, migrated from the three old names, reduced motion is a cut. DECISIONS §71. `transitions.test.js` 15 + 6 elsewhere, suite 1072 |
| 9 | Search | **partly done** | glued digits parse, a literal hit must cover 55% of the query, and nothing a search does reaches a screen. DECISIONS §72. `e2e::r9_*` ×4, cargo 673. **"Why it matched" not built** |
| 10 | Library | **partly done** | the operator label reaches the record again and still not the glass (DECISIONS §73); announcements say which fields the room sees. `e2e::r10_*`, `r2livepath` R2-H, cargo 674 / suite 1074. **Collections rail, reflow editing, section keys and media upload not built** |
| 11 | Settings | **partly done** | rows say which kind of nothing they have (`settingValue`, pinned by `surface.test.js` R3-13). `settingvalue.test.js` 7, suite 1083. **The eleven-section merge and the type-role pass not built** |
| 12 | SuperSource | **done** | a `region` layer that is its own container, a depth cap, a built-ins-only inner template, the SuperSource starter and its inspector block. DECISIONS §74. `composite.test.js` 9, suite 1092 |

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

**Phase 4 found the reason the look register existed and did nothing.** `set_channel_template`
took a non-null id, so every screen always had a template of its own — and since a screen's own
template wins over a content-type default (§29), the content-look map could be filled in, saved,
and change nothing on any screen in the building. The sentence under the picker said the
opposite of what the code did, which is how it survived. A screen can now be set to **Follow the
content look**; §29 is unchanged for a screen that has one.

**The ten roles are not all here.** The register is `CONTENT_KINDS` — scripture, songs, media,
announcements, timer — which is what the fire path, the database and the matrix already speak.
The spec's remaining five (`preservice`, `stage`, `lower.name`, `lower.lyric`, `lower.bible`,
`supersource`) name kinds nothing renders yet; they arrive with phases 5, 6 and 12. A role
offered before its renderer exists is a control that saves a setting nothing reads, which is the
defect this phase just closed.

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

**Switchable zones and the stacked rail clock are not built.** Zones are a per-screen setting
with nowhere yet to persist them, and the three-pair rail needs the same container-inside-a-
container concept phases 5 and 12 are waiting on. Offering a switch that saves nothing is the
defect phase 4 just closed.

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

**Start/Pause, Reset and ±1 are not built.** `countdown_to` is an absolute instant that rides
with the content; pausing and nudging need a countdown the engine OWNS rather than a timestamp it
broadcast once, which is a backend model rather than a transport row. Clear already exists (the
screen clears).

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

**"Each hit says why it matched" is not built.** It changes the shape of what `search_scripture`
returns, and three surfaces plus the preacher's remote read it. It belongs with the Library pass
(phase 10), which reworks that UI anyway.

**Phase 10 took the half of the Library that was a defect, not a redesign.** The rule that a
song's section label must not reach the glass was held at the backend and implemented AGAIN in
Live, which passed an empty label — so the wall was right and the service record could not name
what had been on it. One rule, one home; both halves are now tested.

**The rest of §10 is a redesign, and it is not started.** The collections rail, reflow lyric
editing, section keys (`v c b t i o`) and media upload are new surfaces rather than repairs.
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

**A limit of the new instrument, recorded rather than closed by force.** `workspacegrammar.test.js`
asserts no raw hex and no px font sizes — over its `DESKS` array only. Editors, Live and Library
sit outside it. Widening the array to swallow a stray literal would either force an unrelated
conversion or dilute what the array asserts, so the array stays honest and the gap is written
down. A literal `#141417` survived in the template editor exactly this way, and was found by eye.

**Carried forward, deliberately.** Phase 1 owns `src/app.css`, and the shared layer is now
token-only: every literal radius there reads `--v-r-*`. Components still hold about sixty
radius literals of their own, twenty of them `99px` — so a few pills survive on surfaces phases
2, 10 and 11 rewrite anyway. "No pills" is true of the design system as of phase 1; it is not
yet true of every rendered screen, and saying otherwise would be the kind of claim this
repository files as a finding.

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
