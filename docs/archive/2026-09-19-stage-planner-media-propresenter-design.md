> **Archived 2026-09-21.** Historical. Eleven of its thirteen items shipped on 2026-09-20 (RG-161 to RG-177, DECISIONS §104, §105); its open questions now live at the tail of `docs/superpowers/plans/2026-09-19-stage-timers-mobile.md`. Its "Verification status" block describes the evening it was written and is false about the code as it stands. Nothing here is edited except the paths of citations into other archived files, retargeted so every citation still resolves; the rulings and findings it led to live where the line above says.

# Stage reach, Planner craft, media transport and ProPresenter

Date: 2026-09-19. Status when written: design, approved in outline, no code written.

> **Status on 2026-09-21: largely SHIPPED, and this document is now history.** Thirty commits
> dated 2026-09-20 delivered SP1, 2b, 2c, 2d, 3a, 3b, 4a, 4b, 4c, 4d, 4f, 5a, 5b and 5d
> (see the register RG-161 to RG-177 and DECISIONS §104, §105). Still not built: **4e**
> (click semantics, contradicted by DECISIONS §81 on the run surface), **5c** (playlists into
> plans), **5e** (media import, BLOCKED on the church's media folder) and the elapsed timer.
> The "Verification status" block at the end describes the evening this was written and is
> false about the code as it stands; it is kept because a frozen design should not be edited
> to agree with what happened after it. Rulings that only lived here are now in
> `docs/DECISIONS.md`.

The operator chose "spec everything first, build nothing yet" on the evening before a
service test.

Companion document: [`../superpowers/plans/2026-09-19-stage-timers-mobile.md`](../superpowers/plans/2026-09-19-stage-timers-mobile.md).
That plan owns the connection journey, timer state and saved stage layouts, and
several of its phases are already delivered. **This document does not restate it.**
Where the two meet, this one says which phase it depends on and stops.

## What this is

Thirteen requests, arriving together, spanning five subsystems. They are too many
for one implementation plan, so this document decomposes them into five
sub-projects with a dependency order. Each sub-project gets its own implementation
plan afterwards. The decomposition is the deliverable; the per-item design below is
deep enough that a plan can be written from it without re-investigating the code.

Every file:line reference in this document was read on branch `stage-timers-mobile`
at `3956d26`. Four of the load-bearing ones were then verified a second time by
hand, because a previous session recorded that three of eleven agent findings were
false. Those four are marked **(verified)**.

## The decisions taken before writing this

Recorded here because each one closes off an alternative that a later reader would
otherwise reopen.

| Question | Decision | Consequence |
|---|---|---|
| Stage TV timer | Fix the address now; **role-gate** the refusal later | `timer: false` becomes `timer: role === 'stage'`, not `timer: true` |
| Screen Countdown | Leave Quick tools; become a Planner cue aimed at the streaming screen | `quicktools.test.js` drops from three blocks to two, on the operator's instruction |
| Planner colour | **Sections only**, in hues nothing else owns | `plan.js:13-77`'s refusal of per-kind colour stands |
| Planner firing | **No.** The Planner still cannot reach an output | The Tuesday/Sunday separation in `CLAUDE.md` survives untouched |
| Click semantics | Single click previews, double click previews large, a deliberate action goes live | No single click anywhere can reach a congregation screen |
| ProPresenter | Songs, then playlists into plans, stage layouts, discovery, media | Needs real protobuf decoding, which does not exist today |

## The corrections this work rests on

Three claims that were believed at the start of the session and are false.

**The stage TV timer is not a defect. (verified)** `src/Output.svelte` handles
thirteen frame kinds at `:647-759` and contains the string `'timer'` zero times.
`src/lib/r6-contracts.test.js:165` pins `timer: false` with a stated reason: a
programme timer is one person's bookkeeping, and painting "Sermon · 4:12 left"
behind a preacher puts the running order in front of the whole building. There are
two stage pages, not one:

| Page | Address built by | Stage Timer |
|---|---|---|
| `stage.html?channel=N` | `channelroles.js:146`, surfaced in Outputs → **Sharing** | Yes, `Stage.svelte:952` |
| `output.html?channel=N` | `outputurl.js:27`, surfaced as Screens → **Copy URL** | Never |

`src/lib/stagetimerreach.test.js:94` already names this trap in prose. A TV wired
from Copy URL is online, is attached, reports healthy, and will never show a rail.

**Per-cue screen targeting lies for two of five cue kinds. (verified)** The Screens
row at `ServicePlanner.svelte:1118` is not gated by cue type. Scripture
(`Live.svelte:917`), song (`:951`) and announce (`:953`) pass `cueChannels`. Media
(`:923`) and countdown (`:930`) do not, and neither `fire_media`
(`main.rs:3867`) nor `startCountdown` (`capture.js:1646`) accepts such an argument.
`OutputContent.channels` therefore stays `None` through `..Default::default()`, and
the cue reaches every screen under a hint reading "Other screens keep what they are
showing". **Not yet filed in the register**, and it should be: it is the fourth instance of the shape `CLAUDE.md`
records under "a guarantee is only kept on the doors you checked".

**The Planner's neutral palette is a refusal, not an oversight.** `plan.js:13-77`
records declining per-kind colour twice, most recently 2026-09-16, with the
measurements. `--v-col-scripture` **is** `--v-amber` and `--v-col-media` **is**
`--v-amethyst` (`tokens.css:194-197`), so colouring cue kinds from the existing set
puts ON AIR colour on a row that is not on air and rehearsal colour on one that is
not a rehearsal. `plan.js:35` records magenta (~310°) and lime (~80°) as the only
free hues.

## The source material

`~/Downloads/rrr.zip`, 62 MB, is a ProPresenter 7 library and configuration export
from `/Users/hop-media-001/Documents/ProPresenter/`.

- 726 `.pro` files under `rrr/SONGS/`. ProPresenter 7 protobuf, not XML.
- Root configuration files, all protobuf: `Library` (playlists), `Media`, `Theme`,
  `Stage`, `Timers`, `Props`, `Workspace`, `Macros`, `CCLI`, `Groups`, `Labels`.
- **The media is not in the archive.** One `.mp4` and one `.png`. `rrr/Media` holds
  paths under `/Users/hop-media-001/` that do not exist on this machine. Importing
  it yields broken references. The church's media folder is needed separately, and
  this is a blocker on one request rather than a design problem.
- `rrr/Timers` names the operator's own two concepts: `PRE SERVICE COUNTDOWN` and
  `SEGMENT COUNT DOWN`.
- `rrr/Stage` holds their layouts: `Current + Next Text`, `Seg Timer`,
  `Video Timer`, `Video Countdown`, `Segment Countdown`, built on a `${timer}` token.
- `rrr/Workspace` names their screens: `ONLINE SCREEN` (Samsung, 1080p60),
  `STAGE SCREEN` (LF24T35), `ONLINE + STAGE SCREEN`.

**The existing importer already reads the songs.** `proimport.rs` does not parse the
protobuf at all (`proimport.rs:5-7`); it scans raw bytes for `{\rtf1 … }` blocks
(`:204-240`) and strips RTF to text (`:25-100`). Measured against the real archive:
all 726 files contain RTF, 14,364 blocks in total, zero files with none.
`JESUS.pro` carries 332 blocks and about forty files carry exactly one, which is
the set worth eyeballing after an import rather than a reason to doubt the method.

ProPresenter is **not installed on this machine**. No `/Applications/ProPresenter.app`,
no `~/Documents/ProPresenter`. Discovery must therefore be designed against a
folder the operator points at, with Spotlight as an accelerator, not against an
application bundle.

---

# Sub-project 1: screen targeting truth

**The smallest piece of work here, and the only one that is a defect.**
It is first because a control that reports a success it did not achieve is the
failure this repository has recorded four times, and because sub-project 3 needs
`start_countdown` to carry screens before a countdown can be aimed at one.

## Design

Give `fire_media` and `start_countdown` the argument their neighbours already have,
and pass it at the two call sites.

```
fire_media(app, db, id, template_id)
  -> fire_media(app, db, id, template_id, channels: Option<Vec<i64>>)

start_countdown(..., warn_ms, until_ms)
  -> start_countdown(..., warn_ms, until_ms, channels: Option<Vec<i64>>)
```

Both already end at `broadcast_with_clock`, which is the choke point
(`main.rs:785`), so the field rides the existing route and no receiver changes.
`Live.svelte:923` and `:930` pass `cueChannels`, which is already computed three
lines above them at `:909`.

`show_background` is deliberately **not** given the argument. A standing backdrop is
room furniture rather than a targeted cue, and `channels` on it would be a second
answer to a settled question. Record that at the call site so the next reader does
not read the omission as the same bug.

## What this must not break

- `null` means every screen and `[]` means no screen (`plan.js:487`). Neither
  meaning may change; a cue written before targeting existed still reaches
  everything.
- Rule 36. The check stays at `broadcast_with_clock`, never at the call sites.
- A screen a cue does not name keeps what it was showing. It is never cleared.

## Tests

- Extend the existing per-cue targeting tests rather than writing a parallel set.
- The test that matters is the one on the surface that was missed: fire a **media**
  cue naming one of three screens and assert the other two are untouched. Write the
  same for **countdown**. Watch both fail against the current code first.
- An `e2e.rs` case, because this is the fire path.

## Acceptance

A media cue and a countdown cue each naming one screen of three reach that screen
and no other, asserted at the kiosk hub and at the Tauri door.

---

# Sub-project 2: stage reach

Everything that decides what the preacher can see. Depends on nothing; can start
immediately.

## 2a. The address, and the affordance that stops the trap recurring

No code is required to fix tonight's TV: point it at the Sharing address. The
product change is that **Screens → Copy URL must stop being able to mislead**.

Design: in the Screens inspector, when a channel holds `role === 'stage'`, the
address row offers the **stage address** as well as the output address, labelled for
what each one does, with the Copy URL affordance saying plainly that it produces a
congregation-shaped page. `channelroles.js:146` already builds the stage URL and
`Channels.svelte:1770-1812` already owns the address row.

`describeStageReach` (`channelroles.js:214`) already produces the right sentence
when a stage screen has never reported painting. It is currently only read by Live
(`:520`). Read it in the Outputs inspector too, which is where an operator is
standing when they wire a screen.

## 2b. Stage-shaped templates carry the programme timer

The reversal, gated on role so the recorded safety reason survives.

```
r6-contracts.test.js:165   timer: false
                        -> timer: 'stage-role only'
```

- `Output.svelte` gains a `timer` branch in the ladder at `:647-759`, guarded by
  `roleOf(roles, channelId) === 'stage'`. Roles already ride the wire
  (`channel_roles`, read at `Output.svelte:740`) and are already retained and
  replayed on hello (`channels.rs:3033`), so no new plumbing is needed.
- `TemplateRender.svelte` gains a `programme` bind beside the existing monitor-only
  binds (`layers.js:51,53,62,63`). It renders the timer **set**, which is what makes
  it different from `bind:'countdown'`: `Stage.svelte:530-544` already derives the
  set, and `timers.js:35` already filters by scope. Extract that derivation into
  `timers.js` so the two surfaces cannot disagree, exactly as `outputHealth.js`
  serves Live and Outputs from one helper.
- `layers.js` stage starters (`stageDisplay` `:940`, `confidenceMonitor` `:962`,
  the preacher view `:982`) gain a programme layer.
- The congregation refusal is unchanged and must be re-asserted by a test: a screen
  with `role === 'main'` wearing a stage template still shows no rail.

**The overflow rule comes with it.** `Stage.svelte:605-618` collapses to a "+N more"
cell below `MIN_TIMER_PX`, driven by `innerWidth`. A TV webview reporting a small
width is a recorded suspect (plan finding S10). Whatever `TemplateRender` does here
must be measured against its container, not the window, because a template renders
inside a region.

## 2c. A stage alert the preacher cannot miss

`@keyframes stagealert` exists only on the phone (`Stage.svelte:1907-1913`): a
1.4 s background pulse between `#c8121c` and `#7a0a11`, behind
`prefers-reduced-motion: no-preference`. `TemplateRender` has no equivalent; its
only pulses are `cdwarn` (`:2159`) and the ticker crawl (`:2125`).

Design: one shared alert presentation, defined once and used by both pages. The
operator asked for "very strong flashing", so the stage form is deliberately louder
than anything a congregation screen is allowed to do: full-bleed, high-contrast,
and a pulse that does not stop until it is taken down.

Three constraints that are not negotiable:

- **Reduced motion still gets an answer.** The phone's existing rule swaps the pulse
  for a static treatment. Keep that. A preacher with vestibular sensitivity must
  still know a message arrived.
- **A panic control takes it down.** `Stage.svelte:908` already clears the alert on
  `clear` and `black` (DECISIONS §91). The template path must do the same.
- **`stage_alert` is not retained** (`channels.rs:5537`) and must stay that way. A
  phone that reconnects an hour later must not be flashed a message from before the
  sermon.

## 2d. Media and images on the stage display, with scripture overriding

A new zone in the shared vocabulary at `stagelayout.js:15-26`, alongside `reading`,
`next`, `note`, `countdown`, `clock`, `elapsed`, `programme`.

The precedence rule is the whole design, and the operator stated it: **scripture
overrides everything**. Written out:

1. A reading on the stage display always wins the primary area.
2. Stage media occupies the primary area only while no reading holds it.
3. When a reading arrives, stage media is displaced, not destroyed. When the reading
   is cleared, it returns.
4. A panic control takes both down. No exception, per rule 15.

This needs a retained frame of its own, in its own slot, so a device joining
mid-service is shown what is on the stage (the same reasoning as rule 43). It must
**not** share the single screen slot with `content`, or a stage image would stand in
for the reading that displaced it.

Note honestly: this is the request with the least existing scaffolding in this
sub-project. It needs a new frame kind, a new zone, a retention slot, a precedence
rule and receiver handling on one page. It is not a small addition to 2b.

## Tests

`stageprogrow.test.js`, `stagezones.test.js`, `stagealertpanic.test.js` and
`r6-contracts.test.js` are the four that already hold this ground. Extend them.
`r6-contracts.test.js` in particular must keep enumerating the whole frame ladder,
because a test that checked only the kinds we remembered would have passed on the
night all four overlays were broken (rule 44's lesson).

## Acceptance

A stage-role screen on `output.html` shows the programme rail; a main-role screen
wearing the same template does not. An alert flashes on both stage surfaces, stands
down under reduced motion, and is taken by both panic keys. Stage media yields to a
reading and returns when it is cleared. A device joining mid-service sees what is on
the stage.

---

# Sub-project 3: timer surfaces

## 3a. Screen Countdown leaves Quick tools

Quick tools is pinned at three blocks by `quicktools.test.js:319` and `:427`, on the
operator's 2026-09-14 instruction. **This change is that instruction being revised,
not a test being worked around.** Record it as such.

Design:

- Remove the countdown block (`Dock.svelte:1181-1329`). The card becomes two
  blocks: Live transcript's neighbour tools and the Stage Message. Update
  `quicktools.test.js` to assert **two**, with the reason in the file.
- A countdown is started from a Planner `countdown` cue, which already exists
  (`ServicePlanner.svelte:316`, fired at `Live.svelte:924`).
- The cue carries screens, which sub-project 1 makes real. Aimed at the streaming
  screen, a countdown reaches the stream and no congregation screen, which is what
  "only when needed for the live service that goes online" means in code.
- The transport (pause, resume, reset, ±1) has to remain reachable during a service.
  It moves to the Live run surface beside the existing Stage Timer band
  (`Live.svelte:2298-2345`), not into Settings and not into Outputs.

**Open question for the operator, to settle before the plan is written:** with the
dock block gone, how is a countdown started when there is no plan? A pre-service
countdown on a Sunday with an empty plan is a real case. Options are a cue added to
an ad-hoc plan, or a small starter on the Live surface itself. This document does
not choose.

## 3b. Timer typography

Scope: the congregation-facing countdown only. The operator asked that the stage one
"stay inline for readability", so `Stage.svelte`'s four-step discrete sizing
(`:786`, `:1601-1611`) is untouched.

Today, `.countdown` (`TemplateRender.svelte:2262-2271`) is `tabular-nums`, weight
700, `line-height: 1.05`, `letter-spacing: 0.01em`, `nowrap`, at `verseSize * 2`.
The work is craft rather than architecture: optical weight at very large sizes,
letter-spacing that suits digits rather than prose, the treatment of the separator,
and how the figure settles when it changes.

Four existing guards constrain it, and all four are load-bearing:

- **Rule 37.** The fit floor is a ratio, 45% of the designer's cqw
  (`MIN_LEGIBLE_SCALE`, `:324`), reported through `onFit`, never a blank screen.
- **Rule 42.** A fit measured in the fallback face must re-fit, bounded to two
  retries (`needsRefit`, `:23`).
- **RG-141.** The countdown is fitted against a definite rectangle,
  `.content.cdbox` (`:2218`), because a `nowrap` line makes a shrink-to-fit box
  scale with its own type.
- **The tick gate.** `fitSig()` ends `…|${countdownTo ? 1 : 0}` (`:590`), so a
  per-second tick does not retrigger a fit. Any animation added to the digits must
  not defeat this, or a 4 Hz reflow storm returns.

One honest gap worth closing while here: `legibility.js` reviews the template's
declared verse size and does not know the countdown paints at `verseSize * 2`
(`:1919`). Its contrast and distance verdicts are therefore silent about the
largest thing on the screen.

---

# Sub-project 4: Planner craft and media transport

The largest sub-project, and the one that should be split again when its plan is
written. Its first four items are craft; its last is architecture.

## 4a. Section colour

Colour the section bands, never the cue kinds. `sectionsOf` (`plan.js:285`) already
derives sections and `ServicePlanner.svelte:812-818` already renders the caption and
hairline, so the structure exists and only its presentation changes.

Constraints:

- Hues come from magenta (~310°) and lime (~80°), the two `plan.js:35` records as
  unclaimed, plus neutrals. A section band may never wear amber, amethyst, cyan,
  grey, rose or emerald, because each already means something exact
  (`tokens.css:136-178`).
- The cue kind chip `.sp-ck` stays neutral. `TAXONOMY_INK` (`plan.js:78`) does not
  move.
- `colourlaw.test.js` must be strengthened while this is done: its `TYPE` sweep is a
  substring match today, which `plan.js:67-76` already notes would let
  `var(--v-col-…)` slip past.
- Colour is never the only signal. A section must read correctly in greyscale, for
  the same reason a paraphrase shows no percentage.

## 4b. Adding and deleting a cue

The operator asked for adding and deleting to be clean and straightforward.
Measured against the code:

- **Adding** is already one click per result row (`ServicePlanner.svelte:923-948`),
  behind a mode toggle at `:781`. The friction is the toggle and the search, not the
  commit.
- **Deleting** is the real gap. A cue can only be removed from the **inspector**
  (`:1175`); there is no affordance in the running order at all. Deleting three cues
  means three select-then-travel-to-inspector round trips.
- **Duplicate** has a defect worth fixing in passing: `duplicateCue()` (`:594`)
  appends at the end despite a comment claiming it copies "a cue in place".

Design: a per-row delete in the running order, and multi-select so a run of cues can
go at once. No native `confirm()` (rule 41: Tauri's webview returns `false` without
showing anything, so a guarded delete deletes nothing and reports success). Use the
in-app two-step arm and confirm that `TemplateGallery` and the plan rail
(`:186`) already use.

`ServicePlanner.svelte` is 1607 lines and this work will grow it. Two blocks are
already self-contained and should move out first: the add and search block
(`:243-345`) and the drag block (`:376-460`).

## 4c. Media preview on a Planner slide

The exact cause, traced end to end:

1. `plan.js:169` returns `[{ tag: 'BG', label: item.label, text: '' }]` for a media
   cue. **No `media_id`, no URL, empty text.**
2. `ServicePlanner.svelte:636-643` builds `previewContent` without `media_url` or
   `media_kind`, the two fields `TemplateRender` actually reads.
3. `previewState` (`plan.js:449`) sees empty text, takes the media branch at `:453`
   and returns `plate: false`.
4. `ServicePlanner.svelte:1004` therefore never enters the `TemplateRender` branch
   at `:1021`, and paints the sentence at `:1037` instead.

`TemplateRender` would have rendered it correctly all along:
`TemplateRender.svelte:1786-1793` and `:1925-1933` branch on
`content.media_kind === 'video'`.

The fix already has a model in the repo. `mediaUrl(host, asset)`
(`bundledbackgrounds.js:87`) is the shared builder and mirrors Rust's `media_url`
(`main.rs:4036`) including the bundled case; `MediaLibrary.svelte:161` uses it for
thumbnails. The Planner already holds `allMedia` (`:126`, loaded `:230`). The only
missing piece is the host, obtained by `localIp()` as `MediaLibrary.svelte:142`
does. A video thumbnail uses `preload="metadata" muted playsinline`, as the Library
already does.

## 4d. Drag a media file into the Planner

**No file-drop exists anywhere in `src/`.** Zero hits for `dataTransfer.files`, and
zero for `tauri://drag-drop`, `dragDropEnabled` or `onDragDropEvent` in `src-tauri/`.
The only HTML5 drag in the product is the Template editor's layer reorder
(`TemplateEditor.svelte:572-580`). The Planner's own reorder is pointer-based and
was deliberately migrated away from HTML5 drag (`ServicePlanner.svelte:376-386`).

Two candidate mechanisms:

- **Webview drop.** A `drop` handler reading `dataTransfer.files`, then the existing
  base64 path (`fileToBase64` → `importMedia`). No new Rust, no new permission. The
  file is read into the webview, so the 256 MB `MAX_IMPORT_BYTES` cap
  (`main.rs:2807`) applies and a 4K background is a real memory event.
- **Tauri's native drag-drop event**, which hands over a **path** rather than bytes.
  Far better for large video, needs `dragDropEnabled` and a Rust command that reads
  from a path, which is a new capability decision.

Recommendation: the webview drop first, because it reuses the whole existing import
path and adds no permission surface. Revisit the native event when a real 4K file
proves the cap is a problem.

Two rules apply to the drop itself: it must not collide with the pointer-based
reorder, and dropping onto a running order must show where the cue will land before
the mouse is released.

## 4e. Click semantics

The operator's model, as decided: **single click previews, double click previews
large, going live is a deliberate separate action.** No single click anywhere may
reach a congregation screen, and the Planner still cannot reach an output at all.

| Surface | Single click | Double click | Live |
|---|---|---|---|
| Library | load into preview | large preview | existing fire control |
| Planner | select and preview | large preview | **not available** |
| Live | select into the preview pane | large preview | Enter, or the GO control |

This is a change to a shared interaction vocabulary and must be applied everywhere
at once, or the same gesture means two things on two surfaces, which is exactly the
failure the transport's mode label exists to prevent.

## 4f. Media transport and remaining time

**The largest single item in this document, and the one with no existing wire.**

What exists: nothing. The `<video>` is mounted `autoplay loop` with no `controls`
(`TemplateRender.svelte:1790`, `:1887`, `:1931`). A repo-wide search over `src/`
excluding tests returns **zero** hits for `currentTime`, `timeupdate`, `.duration`,
`.pause()` and `playbackRate`. The only `play()` calls are the two inside
`routeAudio` (`:1045-1058`). There is no `media://` event; media rides
`output://content` as two fields. `loop` is a hard-coded attribute, not a setting.

Five things must be built, in this order:

1. **A player that can be asked.** `TemplateRender` exports duration on
   `loadedmetadata` and position on `timeupdate`, and accepts `paused`, `loop` and
   `seek`. The `{#key slideKey}` remount at `:1447` destroys playback state on any
   content change, so the media URL must be isolated from the text fields in that
   key or a caption edit restarts the clip.
2. **Fields on the one frame, following the countdown precedent exactly.**
   `OutputContent` already carries `countdown_to`, `countdown_from` and
   `countdown_paused_ms`: an absolute deadline plus a frozen figure, so the console
   subtracts locally and never polls. The media equivalents are `media_started_at`,
   `media_paused_at` and `media_loop`, threaded through `broadcast_content` at the
   choke point exactly as `channels` was.
3. **A duration the console can read.** This is where the countdown analogy runs
   out: a countdown's length is chosen and a clip's is a property of the file.
   Either `media_assets` (`db/library.rs:203`) grows a `duration_ms` column
   populated at import, which needs a probe the crate does not have, or the screen
   reports it.
4. **A reverse channel, which does not exist.** The hub's only client-to-server
   messages are `hello` (`channels.rs:2870`) and `beat` (`:2860`). **This is the
   decisive constraint.** Without a screen reporting its real playback state,
   remaining time on Live is computed from `now - media_started_at` against a stored
   duration, and a screen that buffered, stalled or failed to autoplay makes the
   console's clock a confident lie. That is rule 35: a status line that cannot
   detect its own failure is not a status line. **Design the reverse report first,
   or do not ship the readout.**
5. **Commands and receivers.** `pause_media`, `replay_media`, `seek_media`,
   `set_media_loop`; none exist (`main.rs:512-519` registers only `list_media`,
   `import_media`, `delete_media`, `fire_media`). All three doors need handling:
   `output.html`'s kiosk branch, its Tauri branch, and `stage.html`.

Colour, because it is already decided elsewhere: the transport may not use amber for
"playing", since amber means ON AIR and nothing else. Grey (CUED) and steel blue
(selection) are the only honest candidates in the existing set.

**Recommendation:** split 4f out of this sub-project when its plan is written. It is
a wire change, a schema change and a new client-to-server direction, and it should
not travel with thumbnail work.

---

# Sub-project 5: Library and ProPresenter

## 5a. Library scripture numbering

The operator asked that Library chapters show numbers as Live does. The
investigation changes the request, so it needs restating before it is built:

- **Neither surface shows verse numbers.** Both show a deck ordinal. Library prints
  `{v.slideNo}` bare (`VerseDeck.svelte:260`, `:450`); Live prints
  `String(c.n).padStart(2, '0')` (`Live.svelte:2569`). **(verified)**
- Library's ordinal is deliberately decoupled from the verse number. The comment at
  `Browse.svelte:341-345` records why: a verse number drifts the moment anything is
  sorted or filtered, and then two slides on screen wear the same number.
- Library re-derives the reference locally (`Browse.svelte:313`) while Live consumes
  the backend string (`slidegrid.js:123`). Same output today
  (`db/verses.rs:131` uses the identical format), but two copies of one rule.
- Live keeps a real verse number in `tag` holding `v` plus the verse number (`slidegrid.js:126`) and
  never renders it.

Three separable pieces of work, and the operator should choose which they meant:

1. **Padding.** Make Library's ordinal `01`-shaped like Live's. One line each at two
   sites, and the consistency asked for.
2. **Real verse numbers.** Render `v.verse` on both surfaces, beside or instead of
   the ordinal. Genuinely more useful for finding a verse, and it must not
   reintroduce the drift the comment warns about, so it is an addition to the
   ordinal rather than a replacement.
3. **One source for the reference string.** Point Library at `v.reference`. Not
   visible to anyone, and it removes a duplicated formatting rule.

There is no shared card component: `VerseDeck.svelte` and Live's inline `.sg-cell`
grid are two implementations. Only `TemplateRender` is shared. Extracting a shared
card is the right fix and is larger than the request.

## 5b. Bulk song import

Works today, needs only volume. `parse_import` (`main.rs:3007`) and
`save_reviewed_songs` (`:3068`) are the one path, fed from
`Library.svelte:389`, byte-capped at 256 MB (`main.rs:2807`, mirrored
`capture.js:2226`).

What 726 files need that one file does not: progress, cancellation, a review step
that does not require reading 726 entries one at a time, and a report at the end
naming what came in thin. About forty files yield a single RTF block, and those are
where a silent failure would hide.

Titles come from the file stem (`proimport.rs:313`) and every slide is renamed
`Slide N` (`main.rs:3036`), so section structure is lost. That matters for 5c,
because an arrangement is a list of indices into sections (rule 39).

## 5c. Playlists into plans, and stage layouts

**Both need real protobuf decoding, which does not exist.** `proimport.rs:5-7`
states it outright. ProPresenter 7's `.proto` schemas are not published by
RenewedVision, so this is reverse-engineering from the wire format.

`rrr/Library` holds the playlists, including their references
(`Libraries/SONGS/30 Billion.pro`, `Libraries/SCRIPTURES/Announcement.pro`).
`rrr/Stage` holds the layouts. Mapped onto Relay:

| ProPresenter | Relay | Note |
|---|---|---|
| Playlist | a plan (`plans`, `plan_items`) | order preserved; each entry becomes a cue |
| Playlist entry → `.pro` | a song cue | needs the song imported first, so 5b gates this |
| Stage layout | a stage layout (`stage_layouts`, `db/stage.rs:96`) | zone vocabulary differs; `${timer}` maps to `programme` |
| `Timers` | nothing yet | `PRE SERVICE COUNTDOWN` and `SEGMENT COUNT DOWN` map onto `Scope::Both` and `Scope::Stage` respectively |

Sequencing: 5b before 5c, because a playlist entry that names a song Relay does not
have is a cue that cannot be built. An entry whose song is missing must be imported
as a visibly incomplete cue rather than silently dropped.

**Honest scoping note.** This is the item most likely to overrun. A defensible
smaller first delivery is playlist **order and titles** only, matched against songs
already imported by name, with everything unmatched listed for the operator. That
needs far less of the schema than full decoding and delivers most of the value.

## 5d. Discovery

Not possible today, and the reasons are specific:

- The frontend has **no filesystem API at all**. `capabilities/default.json` grants
  only `core:default`, `updater:default` and `process:allow-restart`. Neither
  `tauri-plugin-fs` nor `tauri-plugin-dialog` is in `package.json` or `Cargo.toml`.
- Rust is unsandboxed (`relay.entitlements` says so explicitly), so a Rust-side
  `read_dir` needs **no capability change**.
- But `~/Documents` is TCC-gated on macOS 10.15+, and `Info.plist` carries only
  `NSMicrophoneUsageDescription`. A scan there needs
  `NSDocumentsFolderUsageDescription` added. **Rule 17 applies directly**: this is
  exactly the class of failure that is invisible until the one signed build handed
  to a church. `scripts/sign-local.sh` reproduces it without a certificate and must
  be run before this is called done.

Design: a new Rust command that scans a folder the operator points at, defaulting to
`~/Documents/ProPresenter` and its known siblings, reporting what it found without
importing anything. Spotlight (`mdfind`) is an accelerator for the default case and
must never be the only mechanism, because it is disabled on some machines and
returns nothing on an external drive.

**A scan and not a file picker needs its reason recorded**, the way
`models.rs:595` already records the same decision for the STT model directory.

## 5e. Media import and preview

Design is ordinary and mostly exists: `import_media` (`main.rs:2874`),
`media_assets` (`db/library.rs:203`), `/media/<id>` (`channels.rs:3324`, range
requests supported), thumbnails in `MediaLibrary.svelte:161`.

**The blocker is data, not code.** The archive holds one `.mp4` and one `.png`.
`rrr/Media` references files under `/Users/hop-media-001/` that are not present. An
importer pointed at this archive will produce a media library of broken references.
The church's media folder has to be supplied separately, and until it is, this item
is `BLOCKED` rather than pending.

Note also: there is **no audio kind**. `image`, `video` and `document` only
(`Library.svelte:307-310`), and documents are refused as output backgrounds
(`main.rs:3903`, `:4007`).

---

# Order of work

```
SP1 targeting truth  ──────────────┐
  (smallest, and a real defect)    │
                                   ├──► SP3a countdown scoping
SP2 stage reach  ──────────────────┤
  2a address affordance            │
  2b role-gated programme timer    │
  2c alert flash                   │
  2d stage media + override        │
                                   │
SP3 timer surfaces  ───────────────┘
  3b typography (independent)

SP4 planner craft         SP5 library + propresenter
  4a section colour         5a numbering (independent, tiny)
  4b add/delete             5b bulk song import
  4c media thumbnails       5c playlists + stage layouts (needs 5b)
  4d drag-drop              5d discovery (needs Info.plist, rule 17)
  4e click semantics        5e media import (BLOCKED on the media folder)
  4f media transport  ◄──── split this out; it is a wire change
```

SP1 first because it is a defect and because SP3a needs it. SP2 and SP5 can run in
parallel with each other. SP4's 4f should become its own plan.

# What this document deliberately does not decide

- How a countdown is started when there is no plan (3a).
- Which of the three numbering changes the operator meant (5a).
- Whether media transport ships without a reverse channel. The recommendation is
  that it does not, and that is a recommendation rather than a decision.
- Anything covered by [`../superpowers/plans/2026-09-19-stage-timers-mobile.md`](../superpowers/plans/2026-09-19-stage-timers-mobile.md),
  whose phases 1, 2, 4, 5 and 6 remain open. Phase 6 in particular is a written
  rehearsal script that has never been run, and no claim here about a physical
  stage TV, a phone or a projector can be made until it is.

# Verification status

- **NOT TESTED:** every item in this document. No code has been written.
- **PASS (measured this session):** 726 of 726 `.pro` files contain RTF, 14,364
  blocks, zero empty. Command recorded in the session log.
- **PASS (verified by hand):** `Output.svelte` contains no `timer` branch;
  `fire_media` and `startCountdown` take no `channels` argument while three sibling
  call sites pass one; the Planner Screens row is not gated by cue type; Library
  prints a bare ordinal where Live pads to two digits.
- **BLOCKED:** 5e, on the church's media folder, which is not in the archive.
- **NOT APPLICABLE:** any claim about a physical stage TV, a phone, a projector or a
  packaged build. This machine cannot screenshot the Tauri window and no device has
  been driven.
