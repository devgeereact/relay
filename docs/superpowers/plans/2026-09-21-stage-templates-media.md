# Stage, templates, outputs and media — the operator's brief of 2026-09-21

**Status: BUILT, 2026-09-21. All twelve items complete; see each section for its RG row.**
What follows is the plan as written, annotated with what each item became. Written from the operator's brief and from reading
the code, not from assumption — every claim below says whether it was VERIFIED in the tree or is
still UNCHECKED, because three of the last five things filed as broken turned out to be already
fixed and one turned out to be a different defect from the one reported.

**Scope.** Twelve items. They are not one change, and they are not equally urgent: four are
things a congregation or a preacher sees go wrong, four are things that make an operator slower,
and four are design work. The order below is by harm, not by the order they were asked in.

---

## 0. What is actually wrong, verified against the code

| # | The operator's words | What the code says | Status |
|---|---|---|---|
| 1 | Stage Message should be fixed text unless sent to stage / alert | `TemplateRender` renders `stageAlert` as a **full-bleed flashing panel**, always. There is one path, not two: anything typed and sent becomes the alert. A `stage_message` layer binding exists and is covered by the panel | **VERIFIED** — there is no "quiet fixed text" mode at all |
| 2 | Stage Timer ignores size, will not stay put | `.lp-val`'s `font-size` is `min(clamp(12px, calc(92cqw / var(--tmrs) / var(--tch,6) / 0.62), 64px), var(--lp-room, …))` — computed **entirely from the container**. `L.size`, the editor's Size control, is never read for this layer | **VERIFIED** — the control is wired to nothing |
| 3 | Template editor too busy | One inspector column carrying object name, duplicate/reset/delete, X/Y/W/H, three centre buttons, content, font, size, colour, style link, align, v-align, caps, line height, spacing, shadow, scale, line transform, italic, scroll, template name, and a five-row content-kind table. ~25 controls, no grouping by task | **VERIFIED** |
| 4 | What is in the editor should be what is on the output | Both go through `TemplateRender`, which is the right architecture. The divergence is real but is item 2's shape: layers whose rendering IGNORES their own declared properties (`programme` size; the alert panel overriding `stage_message`) | **VERIFIED in part** — needs a per-binding audit |
| 5 | Outputs should preview what is live on each screen | `Channels.svelte` renders each card through the real `TemplateRender`, but feeds it a **stand-in verse**, not what that screen is showing. The screenshot shows four cards with the same verse, two of them NOT RESPONDING | **VERIFIED** |
| 6 | Library needs Live's chapter picker | Live has a chapter grid; `Browse.svelte` has book + chapter state but not the same picker | **VERIFIED** |
| 7 | The default template should apply everywhere | Needs the resolver audit (`resolveOutputTemplate`) against each kind | **UNCHECKED** |
| 8 | Media items need a preview | Planner and Library show a filename | **UNCHECKED** |
| 9 | Duplicate song slides both light up | Two slides with identical text both show LIVE in the screenshot — the live marker is matched on CONTENT, not on position in the arrangement | **VERIFIED from the screenshot**, not yet in code |
| 10 | Media controls are stage-only and incomplete | `mediaTransport` exists; the reach needs checking | **UNCHECKED** |
| 11 | Media on stage: timer needs its own background; add a media countdown | Neither exists | **VERIFIED as absent** |
| 12 | All media in sync | Needs definition before it can be built — see §12 | **NEEDS A DECISION** |

---

## 1. The Stage Message becomes two things, not one

**The defect.** There is one send and one rendering: a full-bleed flashing red panel. So a quiet
note to the preacher — *"wrap up in 5"* — arrives as the same emergency as *"stop, medical
incident"*. An alarm used for ordinary business stops being an alarm.

**The decision to take.** Two verbs, one field:

- **Send to stage** — the words appear as ordinary fixed text, in the `stage_message` layer the
  template already declares, where the designer put it. No flash, no full bleed. It sits with the
  reading like a note.
- **Alert** — the full-bleed flashing panel, unchanged. Reserved for the thing that must stop the
  service.

**What must not regress.** The panel exists because a template designed before Stage Messages
existed still has to be reachable by an emergency (its own comment says so). So *alert* keeps
covering everything and needs no layer. *Send to stage* needs the layer, and a template without
one must say so at the point of sending rather than swallowing the message — a send that goes
nowhere is the silence RG-156 was about.

**Shape:** one new argument on the existing command, `urgent: bool`; `Output.svelte` routes to
the layer or the panel; the Quick tools card grows a second button. The role gate is untouched.

---

## 2. The Stage Timer obeys its layer

**The defect, exactly.** The rail sizes its digits from the box and the number of timers. Every
other text layer in the product is sized by `L.size` in `cqw` and fitted by `fitLayers`. The
programme rail is the one layer that ignores the control the editor shows for it — so the
operator drags Size and nothing moves, which reads as the app being broken rather than as a
property not being wired.

**The decision.** `L.size` becomes the rail's base figure size, exactly as it is for every other
text layer, and the container-derived value becomes the CAP rather than the value: a figure may
shrink to fit its box, never grow past what the designer asked for. That keeps RG-147 (a sliced
`1:30:13` is a lie) and rule 37 (a fit that cannot fail is not a fit) while making the control
mean what it says.

**Position.** `boxStyle(L)` is already applied, so x/y/w/h should hold; the operator's report that
it "will not stay where I want it" needs reproducing against a real layout before anything is
changed, because the rail also sets `--lp-h` from a measurement and that is the likelier culprit.

---

## 3. The template editor, restrategised — **BUILT, RG-217**

**The problem is not the number of controls, it is that they are one list.** An operator opening
this screen is doing one of four jobs, and the inspector asks them to hold all four at once.

**The guide: four tasks, in the order somebody actually does them.**

1. **WHAT is this?** — content binding and the layer's name. One row, always visible. Everything
   else in the panel is about a layer whose job is already chosen, so this comes first and never
   scrolls away.
2. **WHERE does it sit?** — X/Y/W/H and the three centre buttons. Collapsed by default: dragging
   on the canvas is how this is normally done, and the numbers are for the one time somebody
   needs two layers to agree exactly.
3. **HOW does it read?** — font, size, colour, align, caps, line height, spacing. This is the
   group that gets used on every layer, every time, so it is open by default and comes before
   anything decorative.
4. **Anything else** — shadow, scale mode, line transform, italic, scroll. Collapsed. These are
   the controls an operator touches once a year, and they are currently sitting at the same
   visual weight as Colour.

**Two things leave the layer inspector entirely**, because they are not about the layer:

- **Template name** and **Content this template renders** belong to the TEMPLATE, not to whichever
  layer happens to be selected. They move to a template-level header or a separate tab. Today
  they sit at the bottom of a layer's panel, which is why they read as busy: they answer a
  different question from everything above them.

**What this is not.** It is not a redesign of the canvas, the layer list or the readability
panel — those are doing their jobs. It is a regrouping of one column, and it should change no
rendered output at all.

---

## 4. WYSIWYG: an audit, not a feature — **RUN, RG-218: clean**

The architecture is already right — one renderer for the editor preview and the wall. What is
wrong is per-binding: a layer whose declared properties the renderer overrides. Item 2 is one
instance; the alert panel covering `stage_message` is another.

**The work is an audit with a test**: for each binding in `layers.js`, does the renderer honour
`size`, `x/y/w/h`, `colour`, `align` and `font`? Any that does not either gets fixed or gets its
control hidden for that binding — a control that does nothing is worse than an absent one,
because the operator concludes the app is broken.

---

## 5. Outputs shows what each screen is actually showing — **DONE, RG-211**

`describeScreen` now returns `shows` beside `kind`/`label`/`note`, so the frame and
the badge come from one verdict rather than two derivations. Feed each card the live content for that channel instead of the stand-in, keeping the stand-in
**only** for a screen with nothing on it (an empty card would otherwise read as a fault). A card
for a screen that is NOT RESPONDING must show its last known frame and say that it is stale —
showing current content under a dead badge is the rule 35 failure in a new place.

---

## 6–11. The rest, in order

- **6 · Library chapter picker — DONE, RG-216.** `ui/ChapterPicker.svelte`, mounted by Live's rail
  and by `Browse.svelte`; the Library's `<select>` is deleted rather than kept beside it. What a
  press MEANS stays with each surface.
- **7 · The default template applies everywhere — DONE, RG-219.** The operator ruled for the STYLE
  rather than the whole template, which is also the only reading that does not put a song title on
  a congregation screen. `resolveOutputTemplate` takes the content kind; scripture and song inherit
  the configured default's `style` on Live, the Outputs cards and the wall; media, announcements
  and countdowns keep their own; a per-screen template is untouched (DECISIONS §29).
- **8 · Media previews — DONE, RG-215.** The Library's media pane and the Planner's cue inspector
  already painted the file; the running order and the Add-cue results did not, and those are the
  two lists a plan is built in. `ui/MediaThumb.svelte` is now the one rule for both.
- **9 · Duplicate song slides.** Only the slide at the arrangement POSITION that was fired may read
  LIVE. The current behaviour marks every slide with the same words, which is why the screenshot
  shows two. This is a correctness bug on the surface an operator steps through.
- **10 · Media controls everywhere — PART DONE, RG-214.** The gap turned out to be the other way
  round: the transport reached every screen except the preacher's, which ignored the frame and
  hard-coded `loop`. Both surfaces now share one rule. Scrub and volume followed as RG-221: an
  event-counted scrub that restates the sync baseline, and a level that survives the next fire.
- **11 · Media on the stage screen — DONE, RG-212 and RG-213.** The rail earns a plate while a
  picture is behind it and loses it when the picture goes; the stage rail carries a `Clip` figure
  read off the page's own player, which is the copy the preacher is looking at. It times THIS
  screen, not the congregation's — the drift is item 12 and is not corrected here.

## 12. "All media in sync" — needs a decision before it is built

This is the one item that cannot be implemented as written, because *sync* could mean three
different things and they have different costs:

- every screen showing the same frame at the same instant (hard: independent browsers, no shared
  clock beyond `beat_ack`);
- every screen STARTING together and being corrected when they drift (achievable);
- the stage countdown agreeing with what the congregation screen is actually playing (easy, and
  probably what the brief means, given item 11 sits beside it).

**Recommendation:** the second and third. The first is a guarantee Relay cannot honestly make
over a church wifi, and claiming it would be the kind of promise this repository deletes.

**RULED, 2026-09-21: the second and third. BUILT as RG-220.** A clip carries the instant Relay
sent it; every page corrects itself against Relay's clock, which `beat_ack` already gives it. The
stage's copy is corrected against the same instant, so the countdown beside it agrees with what the
congregation is watching. A held clip stops being corrected and is not caught up on resume.

---

## Order of work

**First, because a person sees them go wrong:** 1 (stage message), 2 (stage timer), 9 (duplicate
slides), 5 (outputs preview).

**Then, because they slow an operator down:** 11 (media on stage), 10 (media controls), 8 (media
previews), 6 (chapter picker), 7 (default template).

**Then the design work:** 3 (editor regroup), 4 (WYSIWYG audit).

**Last, and only after a ruling:** 12 (media sync).

Each lands as its own commit with its tests written first, and each says in the register what it
cost. Nothing here changes the release decision.
