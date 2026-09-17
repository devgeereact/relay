# DESIGN-2026-09-16-WAVE5 — the wave 5 browser-driven pass

**What a church finds on the shelf, driven rather than read.** Wave 5 merged seven code
tracks into `feat/wave5-impl` (A the forty seeded looks, B/F/H the editor, C channel roles,
D the names, E the seal, G the bare timer, I starter content). Both suites are green on that
tree, and the suites are blind to the class of defect that matters most here: the two
congregation-facing bugs the 2026-09-10 pass found — a first verse painted with its top and
bottom lines sliced through the middle (rule 42) and a screen that reconnected mid-service
coming back blank (rule 43) — were invisible to every static instrument in the repository,
and `qa-inventory` reported zero problems throughout and was right both times. It reports
zero again on this tree, and is right again.

This is **frozen evidence and findings only.** Nothing here records a fix. Three findings are
filed in `docs/qa/RELAY_GAP.md` as RG-158, RG-159 and RG-160, and the measured reach of RG-139
is written into that row. None of it is restated anywhere else.

**A fourth was written, then withdrawn before it was filed, and §8a is why.** The brief this
pass was given owed two findings; one of them was already caught, catalogued and pinned by a
test in the very wave this pass is auditing. Withdrawing it is the point of reading §4 of the
harness before filing anything, and leaving the withdrawal visible is the point of a frozen
audit.

> **RENUMBERED AT THE WAVE 3 / WAVE 5 MERGE, 2026-09-17.** This pass filed its rows as
> RG-143 to RG-147. Wave 3 branched from the same base and filed RG-143 to RG-155 for
> entirely different findings, so wave 5's five moved up in order and the ids in the body
> below were rewritten to match: **RG-143 → RG-156, RG-144 → RG-157, RG-145 → RG-158,
> RG-146 → RG-159, RG-147 → RG-160.** The ids are the only thing that changed. No finding
> was edited, withdrawn or closed here — closures still go in a fix log, never in a frozen
> audit — and **RG-156 is the one to re-read**: the operator's ruling in DECISIONS §91
> settled half of what it filed, and the register row says which half.

---

## 0. Method, and the line between two different claims

| | |
|---|---|
| Date | 2026-09-16 |
| Tree | `feat/wave5-track-j`, branched from the wave 5 integration branch with all seven code tracks merged (`7cc4454`) |
| Backend | `src-tauri/target/debug/relay`, built from that tree immediately before the pass |
| Profile | **fresh install.** The binary was launched with `HOME=/tmp/relayfresh5`, so `db::app_data_dir()` resolved to an empty tree and Relay seeded from scratch. Startup lines: `kiosk: WebSocket server listening on :8031` · `output http: serving output/stage pages on :8032` · `profile: active 'Default' · lang None · sensitivity 50` · `console: webview up (operator)` — one heartbeat line, per rule 26 |
| Frontend | `npm run build` first (RG-127); `npx vite --port 5232` for the console; the embedded HTTP server on `:8032`; the kiosk hub on `:8031` |
| Browser | Playwright 1.63 driving the machine's installed Google Chrome. This machine cannot screenshot the Tauri window, which is why all visual work happens in a browser |
| Not measured | the 2.4 m distance preview; word error rate; anything requiring a microphone, a projector or a second machine. Each is named where it falls below |

**Three different claims, and they must not be blurred.**

* **Fully unmocked.** A verse fired through the real LAN API (`POST /api/fire?ref=John%203:16`)
  onto a browser source opened afterwards. Real command, real router, real pipeline, real
  kiosk hub, real retained frame, real page. §6 is the whole of this claim.
* **Real page, real template data, harness-composed frames.** `output.html` served by Relay
  itself, connected to the real hub on `:8031`, taking its template and its `channel_roles`
  from the real backend's own `hello` — and then additionally handed content frames by the
  harness. `window.WebSocket` is wrapped so the real socket is kept and a second channel
  exists to push a frame; nothing about the page, the renderer or the template is mocked.
  §§1–5 are this claim. It is how a countdown, an announcement and a stage message were put
  on a screen without a person at the console.
* **Console rendered in a browser against a mock bridge.** `:5232` has no backend, so
  `window.__TAURI_INTERNALS__.invoke` was given a dispatch table seeded from the real
  fresh-install SQLite file — the forty templates, the four channels with their roles, the
  33 media rows, the five announcements and the example plan are the real rows. Its **events
  are not relayed.** So a console reading below is a claim about a command surface behind a
  browser render, and never a claim about event-driven behaviour.

**The harness is temporary and is in no commit.** It lives entirely in the scratchpad: a
Playwright init script per context. Nothing was added to `src/`.

**Measurement, not eyeballing.** Geometry is `getBoundingClientRect`; the clipping box is
found by walking up from the text element to the first ancestor whose computed `overflow` is
not `visible`; overflow is `scrollHeight`/`scrollWidth` against `clientHeight`/`clientWidth`;
the painted size is `getComputedStyle(el).fontSize` compared against the layer's own
`data-base` and the box's `data-fitted`. Screenshots are corroboration, never the
measurement.

### 0a. Three measurement artefacts, named so they are not mistaken for findings

**A `Range` over wrapped lines returns the union of the line boxes**, which is a few pixels
taller and wider than the box on a centred or justified wrap. Every "clipped by ≤ 26 px"
reading that this pass discarded is that artefact: on `Scripture · Dayspring` the same
element reports `scrollHeight 433 / clientHeight 433` and `scrollWidth 1613 / clientWidth
1613`, a box that does not overflow at all. **Sixteen** of the forty produced one and none of
them is a finding.

**A `line-height` below 1 makes an element report an overflow it does not have.** All five
`Timer · *` looks declare `lineHeight: 1.0` on their digits, so the glyphs' content box
(ascent + descent, about 1.1 em) is taller than the line box and `.lfit` reports
`scrollHeight 466 / clientHeight 422` at 1920×1080. **`.lfit` is not a clipping box** —
`overflow: hidden` is set only on `.lfit.lscroll` — and the real clipping ancestor, `.ltext`,
is 518 px tall on the same render. Nothing is cut on any of the five, at either viewport.
This one nearly became a finding, and the thing that stopped it was walking up to the actual
clipping ancestor rather than trusting the element the fitter happens to touch.

**An empty text layer's Range collapses to the origin**, so `Stage · Rail`'s four
unpopulated layers report hundreds of pixels of apparent overflow. They contain no words.

---

## 1. All forty, at 1920×1080 and at 390×844 — and what the shelf actually is

Forty rows, every one layer-model, read straight out of the fresh database with no frontend
attached:

```
LAYER|40     ids 1–40, seed_keys scripture.* song.* media.* announce.*
             timer.* scroll.* source.* stage.*  — five each
```

That is a change worth stating plainly, because the previous pass could not say it: on
2026-09-16 the wave 2 shelf seeded **30 region-model and 7 layer-model** rows and converted
them only when somebody opened the Templates workspace, which is what RG-140 and RG-141 are
about. There is now one model and one rendering, before and after that click.

Each template was opened on `output.html` at both sizes and given the content its role is
for — scripture for `scripture.*`, `source.*` and `stage.*`, a four-line lyric for `song.*`,
a bundled picture for `media.*`, a 229-character notice for `announce.*` and `scroll.*`, a
running countdown for `timer.*`. Eighty renders, no page errors in any of them.

**Rule 37's floor: PASS, with 4.4 points of margin at the worst.** No layer on any of the
forty was driven below 45% of its designer's declared size at either viewport. The deepest
shrink measured is `Stage · Rail`'s verse at 1920×1080, declared `6.4cqw` and fitted
`3.1633cqw` — **49.4%**. Next deepest are `Timer · Titled`'s label (`3.4` → `1.9938`, 58.6%)
and `Announce · Bold` (`8.4` → `4.8719`, 58.0%).

**Clipping, once the three artefacts of §0a are set aside: seven of the forty**, and they are
two groups, both with their own section. The five crawls lose most of a long notice under
reduced motion (§4). Two of the six countdown looks clip a label the fit had shrunk, as soon
as the countdown ticks (§3). **Nothing else on the forty put content outside its clipping box
at either size**, on any of the eighty renders, once the fit had settled.

**Five of the forty paint no text layer at all, correctly.** `Media · Full` renders a picture
and no words; the five `Source · *` are composites that render a built-in inside a region
layer, which takes the region branch and has no `.ltext` element. Neither is a defect; both
are noted so the next reader of this pass's raw output does not file them.

---

## 2. The seal — PASS

`output.js` and `stage.js` import `tokens.css` and no longer import the console's stylesheet.
Driven on the real page rather than argued from the diff:

* **No console rule is live on a congregation screen.** Every loaded stylesheet was walked on
  all eighty renders, looking for the six unscoped rules the seal was written against
  (`.prev-main .verse`, `.prev-stage .verse`, `.prev-stream .lower-third`, `.prev-lobby
  .verse`, `.tmpl-row.active`, `.toggle.on`). **Zero matches, on every render.** Four
  stylesheets are present and none of them is `app.css`.
* **The page declares the type base it used to inherit.** `body` computes to
  `line-height: 17.4px` and `font-family: Inter` on `output.html` — the same values
  `app.css`'s `body{}` produced (`--v-fs-b1` × 1.45) — declared by `Output.svelte`'s own
  `:global(html, body)` block. That matters because `line-height` is inherited by any
  template element that does not set its own and is an input to the fit loop: dropping it
  with the stylesheet would have changed the size the binary search settles on, on a wall,
  silently.
* **Templates name real families.** On the wall, the forty render in `Fraunces`, `Inter` and
  `IBM Plex Mono` by name. No `var(--f-*)` token was observed resolving a template's face.

What is **not** sealed, deliberately and correctly: the palette. `tokens.css` still declares
`--f-serif`, `--f-display` and `--f-body` at `:root`, so an output page still carries the
console's colours and its self-hosted fonts. The seal is against rules and template data.

---

## 3. RG-139 on the shipped looks — the reach Track A widened to six is a reach of two, and it is worse than the row said

**Six of the forty carry a `countdown`-bound layer**: `Timer · Monolith`, `Timer · Rail`,
`Timer · Titled`, `Timer · Panel`, `Timer · Contrast` and `Stage · Rail`. Only two of the six
also carry a text layer the fit had to shrink, and those two are the two that clip. The other
four carry digits and a clock, which the fit GREW, so the same revert happens and costs
nothing visible.

**`Timer · Titled`, 1920×1080, a 118-character label, sampled at 300 ms, 1.2 s, 2.5 s and
5 s:**

| | painted | `data-fitted` | content in its box |
|---|---|---|---|
| 300 ms | **38.28 px** (= the fitted `1.99375cqw`) | `1.99375` | 100 px in a 97 px box |
| 1.2 s → 5 s | **65.28 px** (= the declared `3.4cqw`) | `1.99375` | **339 px in a 97 px `overflow:hidden` box** |

The fit lands correctly and the **first countdown tick** puts the element back on its
declared size, where it stays for the whole pre-service countdown. `data-fitted` reads
`1.99375` the entire time: the element knows the size it should be wearing and paints
something else, and `onFit` has already reported a successful fit.

**`Stage · Rail`, same viewport and label**: 38.13 px at 300 ms, then **61.44 px from the
first tick, 319 px in a 108 px box**.

**Isolated rather than inferred.** The identical template, viewport and words were pushed a
second time as an `announce` frame — everything the same except that no countdown is running.
Both templates then hold their fitted size for the whole five seconds and clip nothing:
`Timer · Titled` stays at 38.28 px with 100 px of content, `Stage · Rail` at 38.13 px with
99 px. The running countdown is the variable.

**The revert is not confined to one layer.** `Stage · Rail`'s three static labels
(`Countdown`, `Remaining`, `Up Next`) drop from their fitted `2.2773cqw` to their declared
`1.6cqw` at the same tick — 43.73 px to 30.72 px. It is harmless there because the fit had
grown them. What reverts is every non-ticking text layer on the template; what clips is
whichever of them the fit had shrunk.

**The threshold a church actually meets.** The seeded example plan pins `Timer · Titled` for
its countdown cue with the label `Service begins in` — 17 characters, which fits at the
declared `3.4cqw` and never shows the defect. Measured on the same template at the same
viewport:

| label | characters | `data-fitted` | painted | lost |
|---|---|---|---|---|
| `Service begins in` | 17 | `3.4` | 65.28 px | 0 px |
| `Service begins in — please take your seats` | 42 | `2.943` | 65.28 px | **73 px of a 97 px box** |
| `Welcome to Grace Community Church — the service begins in` | 57 | `2.041` | 65.28 px | **73 px** |

So the shipped default is safe and the first time a church types its own words it is not.

**This pass did not measure the cause and does not guess at one.** What it adds to the row is
the measured reach on the shelf a church now gets, the isolation, and the threshold.

---

## 4. The reduced-motion crawl — the fallback was not built, and the words go

Five of the forty are crawls (`Scroll · Banner`, `Ribbon`, `Glass`, `Bold`, `Clear`). Each
was opened on `output.html` at 1920×1080 and given the same 229-character announcement, in
two browser contexts differing only in `reducedMotion`.

**With motion**, all five behave: `animation-name: relay-ticker`, `padding-left: 1651.19px`,
and the run travels its full width through the box.

**Under `prefers-reduced-motion: reduce`**, `animation-name: none` and `padding-left: 0`
inside a `white-space: nowrap; overflow: hidden` box. Nothing pages. The run simply sits
still, and everything past the right edge is never shown:

| template | run width | box width | never painted |
|---|---|---|---|
| `Scroll · Banner` | 6365 px | 1651 px | 4714 px — 74% |
| `Scroll · Ribbon` | 5516 px | 1651 px | 3865 px — 70% |
| `Scroll · Glass` | 6153 px | 1613 px | 4540 px — 74% |
| `Scroll · Bold` | 9759 px | 1690 px | 8069 px — 83% |
| `Scroll · Clear` | 6153 px | 1651 px | 4502 px — 74% |

A 31-character notice fits in every one of the five and is unaffected. The boundary at
1920 px is around sixty characters.

**And no instrument in the product can see it.** `fitLayers` returns early for `.lscroll`
(`TemplateRender.svelte:1332`) and sets the declared base without measuring, so a crawl never
gets a fit, `data-fitted` is absent on every one of the ten runs, and `onFit` never fires.
Rule 37's whole argument is that a fit loop with no notion of failure always succeeds; here
there is no loop at all, and the surface that reports fit problems to Live has nothing to
report. Filed as **RG-159**.

---

## 5. Channel roles, the stage message, and the panic control

**The role filter works, measured on the real hub's own role map.** The hub's `hello` carries
`channel_roles: {"1":"main","2":"stage"}` — read off a captured socket, not assumed. A
`stage_alert` frame was then pushed to `output.html?template_id=38` (`Stage · Message`) on
each of the four seeded channels in turn:

| channel | role | what painted |
|---|---|---|
| 1 Main screen | `main` | verse + reference. **No message.** |
| 2 Stage display | `stage` | verse + reference + `WRAP UP — five minutes left` |
| 3 Streaming | *(none)* | verse + reference. **No message.** |
| 4 Lobby screen | *(none)* | verse + reference. **No message.** |

No role is not a stage, which is the refusing default a filter needs. **PASS.**

**The two stage surfaces still disagree about a panic control, and that is RG-156, confirmed
not re-filed.** With a message standing on channel 2, a `clear` frame removes the verse, the
reference and the message together; a `black` frame does the same and paints the blackout.
`Stage.svelte` deliberately keeps its `alert` through both (DECISIONS §89). *[Measured 2026-09-16 and true then. The operator has since ruled the other way — DECISIONS §91 — and the stage page now clears `alert` on both controls, so the two surfaces agree about the panic itself. RG-156 carries what is left of the divergence.]* Wave 5 makes
`output.html` the supported route for a Stage Message, so the disagreement is now between two
paths a church can plausibly use for the same screen rather than between a used one and a
theoretical one. The row already carries it.

**Live's programme pane names its screen by role. PASS.** Rendered in the console:
`Program · Clear · as Main screen`, with the title *"This pane renders through Main screen's
template"* and `guessed: false`. `programmeScreen(channels)` returns
`{channel: Main screen, byRole: true}` against the seeded `output_channels.role`. The
labelled fallback for an install that has never opened Outputs was not exercised — the seeded
rows always carry a role.

---

## 6. Rules 42 and 43 against the real backend

**Rule 43 — a screen that joins mid-service. PASS, and this is the one fully unmocked
measurement in the pass.** `POST /api/fire?ref=John%203:16` was issued against the running
backend; a browser source was then opened for the first time afterwards. It is sent the
retained frame on `hello` and paints: verse and reference present, `Fraunces` at 93.14 px,
`data-fitted 4.8508` — 93.14 / 1920 × 100 = 4.851, the fitted size — and 499 px of content in
a 497 px box, which is the Range artefact of §0a.

**Rule 42 — a fit measured in the fallback face. PASS, tested by forcing the race.** The
defect needs the real face to land after the first fit, which does not happen on a warm cache,
so every `*.woff2` request was delayed 2.5 s and a 405-character passage fired at 250 ms.
Five templates, four faces:

| template | while the face is loading | after it lands |
|---|---|---|
| `Scripture · Dayspring` | `3.5852cqw`, 607 px in a 605 px box | **refit** to `3.216cqw`, 545 px, nothing lost |
| `Stage · Next` | `3.332cqw`, 564 px in 562 px | **refit** to `3.216cqw`, 545 px, nothing lost |
| `Stage · Plain` | `3.8594cqw`, 735 px in 734 px | **refit** to `3.5535cqw`, 677 px, nothing lost |
| `Scripture · Meridian` | `3.0156cqw`, 519 px in 518 px | **refit** to `3.005cqw`, 517 px, nothing lost |
| `Announce · Bold` | `3.5008cqw`, 565 px in 562 px | **refit** to `3.1422cqw`, 507 px, nothing lost |

Every one refits when the face arrives and ends inside its box. The 1–3 px transient while
the fallback is still on screen is the line-box artefact and resolves. `needsRefit` is doing
what `templatefit.test.js` says it does, on the real page.

**The crawls are the exception, and it is the same hole as §4**: `.lscroll` is skipped by the
fitter, so a crawl is never fitted, never refitted when the face lands, and never reported.

---

## 7. The console: the draft path, the editor, and a fresh install's starter content

**Create New Template no longer writes a row. PASS.** Driven in the gallery:
`＋ New template` opens a `START FROM` list of eight starters and calls `save_template` zero
times. Choosing `Full-Screen Scripture` opens the editor with the header **`Not saved yet`**,
a `Discard` and a `Save template`, and still zero calls. Renaming the draft and clicking
`Back to Templates` raises an in-app two-step reading **`Leave without saving?`**, keeps the
operator in the editor, and writes nothing: `save_template` 0, `list_templates` still 40.
No native `confirm()` was involved (rule 41).

**The three editor layout defects, measured in a real layout engine on the eleven-layer
`Stage · Rail` at 1600×1000:**

* `.te-objtabs` is **88 px tall with a `scrollHeight` of 104 and `overflow-y: auto`**, sitting
  570 px inside its `overflow:hidden` pane with eleven children. It does not grow unbounded.
  **PASS.**
* The add-layer menu is **`position: fixed`**, 186 × 621, entirely inside its clipping
  ancestor (33 px clear of the bottom, 14 px of the right) and 251 px above the viewport
  floor. `Esc` dismisses it; `Clear screens` returns itself from `elementFromPoint` before and
  after, so rule 44 holds on this overlay. **PASS.**
* The armed layer delete: the `Sure?` control measures **45 × 20 px**, is not clipped
  (18 px clear of its container's right edge), and `elementFromPoint` over its centre returns
  itself. The list no longer scrolls horizontally (`overflow-x: hidden`). The stated
  defect — `Sure?` landing in a 20 px box inside a clipped, horizontally scrolling list — is
  fixed on both counts that made it unreachable. **PARTIAL**: it is 20 px tall, which is a
  smaller target than this repository's own guidance likes, and that is a judgement rather
  than a measured failure.

**Starter content, as an operator meets it.** Read out of the fresh database and then driven
in the console:

* **Library** — `Scripture` (count unavailable in the harness, see below) · `Songs 0` ·
  **`Announcements 5`** · **`Media 33`**. The 33 bundled pictures are real `media_assets` rows
  with `path = bundled:backgrounds/<file>`, and `http://127.0.0.1:8032/backgrounds/01-2.jpg`
  serves **200** on the real backend. `/media/1` returns 404 for those rows and is *correct*:
  `media_url` routes a `bundled:` marker to the stable bundle path instead, which
  `main.rs`'s own doc comment says is the difference between a picture and a black wall.
* **Planner** — `Sunday Morning (example)`, `3 cues · 12m est`, in two sections:
  `GATHERING` (`Countdown · 10 min` 10:00, `Welcome` 2:00) and `THE WORD`
  (`Psalms 100:1-5`). The countdown cue pins `template_id 23` — `Timer · Titled`, the
  template of §3, with the 17-character label that fits.
* **The dock's timer runs bare.** `startCountdown(minutes, label = '', doneMsg = '')`, and
  the dock's Start passes neither. The card renders `COUNTDOWN 5:00 NOT COUNTING` with no
  label field, which is Track G's claim. **PASS.**
* **The names are on the surfaces.** The dock carries `STAGE MESSAGE` with `Send to stage` /
  `Take down`; `Stage · Rail`'s layers read `Stage Note`, `Up Next`, `Up Next label`,
  `Remaining`, `Countdown`. One name per concept, except the one in §8.

**Three console readings are harness artefacts and are recorded as such**, because two of
them looked exactly like findings until the bridge was completed: the Library's
`count unavailable` and a raw `TypeError` rendered through `errors.js`'s own fallback, both
caused by the bridge returning `null` where the backend returns a list; the Planner's
`Cue count unknown`, because the bridge's `list_plans` omitted the `cue_count` the real
`db::plans::list_plans` computes in SQL; and Live's programme pane initially showing no
screen name, which appeared the moment the missing commands were filled in. A mock that
answers `null` produces defects that are not there. None of the three is filed.

---

## 8. The Announce role has no bucket, and Up Next means two things

Both were driven in the gallery and the Library, and both are filed.

**Not one of the forty derives the `announcement` kind.** `templateKind` applied to all forty
rows, imported from the running page: the five `Announce · *` come back `scripture` (four)
and `song` (`Announce · Bold`), and the five `Scroll · *` crawls — which do carry `scroll` —
are caught first by the earlier `band` rule and come back `lower-third`. The rendered KINDS
rail therefore reads `All 40 · Scripture 9 · Songs 6 · Lower Thirds 5 · SuperSource 5 ·
Stage 5 · Media 5 · Timer / Countdown 5`, with **no Announcements row at all**, because
`kindsPresent` lists only kinds something derives. Before wave 5 the seeded
`Classic · Announcement` did derive it. The split is deliberate and argued in `shelf.test.js`;
what that argument does not address is an operator who cannot filter to the notices they were
handed. **RG-160.**

**Two things are called Up Next, and the name register now ratifies it.** The Library's
staging queue is headed `Up Next` and its own empty state says *"Cue in Live on a selected
item stages it here — cueing reaches no screen"*. The preacher's monitor has an Up Next too
(`stage_next` / `next_reference`, a layer named `Up Next` on `Stage · Rail`), and
`setStageNext` is called from `Live.svelte` and from nowhere else — twice, once to clear it
and once from the plan's own next cue.

This is not an uncatalogued collision, which is what makes it worth a row. Track D's
`names.test.js` holds `Up Next` as **one** concept and lists `src/lib/views/Library.svelte`,
`src/lib/views/library/Inspector.svelte` and `src/lib/views/library/LiveOutputRail.svelte`
among the files allowed to carry it. The track unified the CASING of six labels and, in doing
so, ratified a name that covers two different behaviours — the shape it exists to remove,
kept by the instrument built to remove it. **RG-158.**

### 8a. The fourth content-look writer — WITHDRAWN, because the wave already caught it

This pass was told a fourth writer of the content-look map was owed and uncatalogued:
`Settings → Screens` renders one `<select>` per content kind through `pickCt`
(`Settings.svelte:263`, rendered `:1126`), beside `Channels.svelte`'s authoritative matrix and
the gallery's inspector, where the wave 5 plan's own correction counted three. All of that is
true, and it was verified in the running console.

**It was already found, by Track B/F/H, in this same wave.**
`src/lib/views/templates/contentlookwriters.test.js` is a census over every `.js` and
`.svelte` file under `src/` — derived from the tree rather than from a list, with the
"can still see a known instance" guard this repository has learned to demand — and it asserts
the caller set is exactly `Channels.svelte`, `Settings.svelte` and `TemplateGallery.svelte`.
Its comment says so in as many words: *"Settings is the FOURTH caller, which the plan for this
wave did not know about when it counted three."*

So the row this pass drafted would have filed a fixed finding, and its proposed validation —
a scanner over `src/` that fails when an unregistered writer appears — is the test that
already exists. It is withdrawn. The general point survives and belongs to the wave, not to
the register: the surface that found it is a census written the same week, and it found it
because it read the tree instead of a list.

---

## 9. The gates, on this tree

| | |
|---|---|
| `npm run build` | clean, ran first per RG-127 |
| `cargo fmt --all` | clean, no diff |
| `cargo clippy --all-targets -- -D warnings` | clean |
| `cd src-tauri && cargo test` | **809 passed / 0 failed / 16 ignored** |
| `cd src-tauri && cargo test e2e::` | **67 passed / 0 ignored** |
| `npx vitest run` | **2417 passed, 157 files** |
| `grep -c '#\[tauri::command\]' src-tauri/src/main.rs` | **140** |
| `node scripts/qa-inventory.mjs` | 140/140 addressed, 0 handlerless, 0 unnamed, 1 orphan (`__r6probe.svelte`, the pre-existing deliberate probe) |

Neither suite failed. The counts are each runner's own summary line, and they are carried into
`docs/qa/QA_HARNESS.md` §0, which is the one register that holds values.

---

## 10. What this pass did NOT measure, and says nothing about

* **Word error rate**, in any language. Unchanged and still unmeasured.
* **The 2.4 m distance preview**, again. Not run, for the second pass in a row.
* **Anything event-driven in the console.** The mock bridge relays no Tauri events, so every
  console reading here is about a command surface behind a browser render.
* **The native output window.** Everything on a screen was measured through `output.html`;
  `open_channel_output` was never called, so a projector opened as a native webview is
  covered only by what the code shares with the browser path.
* **The real Tauri console window.** It was running throughout — its single `console: webview
  up (operator)` line is in §0 — and nothing was clicked in it, because this machine cannot
  screenshot it. That is the whole reason this pass exists.
* **A second machine, a church network, a projector, a camera or a microphone.**
* **Whether RG-139 is a wave 5 regression.** It is not: the row records the renderer as
  byte-identical to the merge base, and this pass measured the shelf, not the history.

## 11. Verdict

**Nothing found here blocks wave 5.** The three filed rows are one P1, one P2 and one P3. The
P1 (RG-159) is a pre-existing gap in a fallback that was specified and not built, and wave 5
enlarged its surface from one seeded crawl to five; it hurts only under a setting a church
turns on deliberately, and it is silent, which is why it is a P1 rather than a P2. RG-139's
measured reach is worse than the row's prose in one respect (every non-ticking layer reverts,
not only the labelled one) and narrower in another (two shipped looks clip, not six), and the
shipped default label does not trigger it.

**The release decision does not move**, and nothing in this pass touches it: word error rate
is still unmeasured in every language, neither platform has a code-signing certificate
(RG-73), and nobody but the author has run a service.
