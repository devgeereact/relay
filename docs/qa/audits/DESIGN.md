# Browser-driven passes — the audits done by driving the app

**Four browser-driven audits, merged into one file on 2026-09-21** and otherwise untouched.
Each is **frozen**: closures go in `../RELAY_GAP.md`, never into a finding below. Nothing here is
edited except the paths of citations, retargeted so every citation still resolves after the merge.

These are the only audits that rendered the product and looked at it, which is why they are the
only ones that have ever caught a congregation-facing rendering fault. **Read each pass's own
header note before trusting an id in it**: waves 4 and 5 were renumbered differently
(`../RELAY_GAP.md` §23a), and one rewrote its body while the other did not.

**What is in this file, in order:**

- `DESIGN-2026-09-16-WAVE2.md` — DESIGN-2026-09-16-WAVE2 — the wave 2 browser-driven pass
- `DESIGN-2026-09-16-WAVE3.md` — DESIGN-2026-09-16-WAVE3 — the wave 3 browser-driven pass
- `DESIGN-2026-09-16-WAVE5.md` — DESIGN-2026-09-16-WAVE5 — the wave 5 browser-driven pass
- `2026-09-17-WAVE4-STAGE-PLANNER.md` — 2026-09-17-WAVE4-STAGE-PLANNER — the wave 4 browser-driven pass

---

<!-- ===== was docs/qa/audits/DESIGN.md, merged 2026-09-21, verbatim ===== -->

# DESIGN-2026-09-16-WAVE2 — the wave 2 browser-driven pass

**Six checks, driven rather than read.** Wave 2 merged five tracks into
`feat/wave2-integration` (Track A the default template reaching a screen, Track B three
overflow sources, Track C the editor's two registers, Track D the twenty-five, Track E the
removal of themes). Both suites are green on that tree, and the suites are blind to the
class of defect that matters most here: the two congregation-facing bugs the 2026-09-10
rendered pass found — a first verse painted with its top and bottom lines sliced through
the middle (rule 42) and a screen that reconnected mid-service coming back blank
(rule 43) — were invisible to every static instrument in the repository, and
`qa-inventory` reported zero problems throughout and was right both times.

This is **frozen evidence and findings only.** Nothing here records a fix. Three findings
are filed in `docs/qa/RELAY_GAP.md` as RG-139, RG-140 and RG-141 and are not restated
anywhere else.

**This document was corrected once, after review, and the correction is §0a.** Its first
version asserted that a fresh install seeds 37 layer-model templates. It seeds **30
region-model and 7 layer-model**, and the shelf this pass first measured had already been
converted. Three of the six checks were rescoped and one finding was withdrawn and
rewritten as a result. The corrected premise is stated where it belongs rather than
quietly folded in, because a frozen audit that has been wrong once is worth reading with
its own history attached.

---

## 0. Method, and the line between two different claims

| | |
|---|---|
| Date | 2026-09-16 |
| Tree | `feat/wave2-integration` at `2ec8000` (all five tracks merged). Working tree carried only the temporary audit harness described below |
| Backend | `src-tauri/target/debug/relay`, built **2026-09-16 11:16:24** from that tree — `find src-tauri/src src-tauri/data -newer src-tauri/target/debug/relay` returns nothing, so the binary is current with the commit |
| Profile | **fresh install.** The binary was launched with `HOME=/tmp/relayfresh`, so `db::app_data_dir()` resolved to an empty tree and Relay seeded from scratch. Startup line: `profile: active 'Default' · lang None · sensitivity 50` |
| Frontend | `npx vite` on `:5032`; the embedded HTTP server on `:8032`; the kiosk hub on `:8031` |
| Browser | gstack's headless driver (`~/.claude/skills/gstack/browse/dist/browse`). This machine cannot screenshot the Tauri window (`screencapture` has no screen-recording permission), which is why all visual work happens in a browser |
| Not finished | the **2.4 m distance preview** — check 1's last clause in `task-15-brief.md`. It was **not run**. It is an unfinished check of this pass, not something outside it, and nothing here should be read as a claim about it |

**Two different claims, and they must not be blurred.**

* **The output and stage pages were measured against the REAL backend.**
  `http://127.0.0.1:8032/output.html?channel=…` is served by Relay itself, it joins the
  real kiosk hub on `:8031`, and every fire below was performed by the real Tauri
  commands. Nothing on this surface is mocked.
* **The console at `:5032` was rendered in a browser, where it has no backend of its
  own.** It was given one: `window.__TAURI_INTERNALS__.invoke` relays each `{cmd, args}`
  over a local WebSocket into the **real running Tauri webview's own `invoke`**, so the
  console's *data and writes are real* and reach the same SQLite file. Its **events are
  not relayed** — `plugin:event|listen` registers a handler the page can fire by hand.
  So a console reading below is a claim about a real command surface behind a browser
  render, and never a claim about event-driven behaviour.

**The harness is temporary and is not part of the product.** `src/lib/__auditbridge.js`
plus one line in `src/main.js` open the relay socket, and the file returns immediately
unless `window.__TAURI_INTERNALS__` already exists — so a plain browser tab on `:5032`
cannot use it. It is scaffolding for this pass and belongs in no commit that ships.

**Measurement, not eyeballing.** Geometry is `getBoundingClientRect`; *clipping* is a
`Range` over the text node compared against the nearest ancestor whose computed
`overflow` is not `visible`; overflow is `scrollHeight`/`scrollWidth` against
`clientHeight`/`clientWidth`. Screenshots are corroboration, never the measurement.

### 0a. THE SHELF HAS TWO MODELS, AND THE FIRST RUN OF THIS PASS MEASURED ONLY ONE

**A fresh install seeds 30 region-model templates and 7 layer-model ones.** Read straight
out of SQLite with no frontend attached — the binary was launched with no Vite on `:5032`,
so the webview never loaded and nothing could convert anything:

```
LAYER|7      ids 31–37   (High Visibility, Notice Board, Media Frame, Lower Third · Lyric,
REGION|30    ids  1–30    SuperSource ×2, Stage · Large type)
```

The 5 built-ins and all **25 families** are region-model. `theme_templates()` holds them as
literal `{"regions":[…]}` strings and `ensure_preset_templates` inserts those strings
verbatim; nothing between the table and `list_templates` converts them.

**Booting the app does not convert them.** A virgin database was restored, the app
launched, and the shelf polled every 4 s for 64 s with nothing touched: `L|7 R|30` at every
one of the sixteen samples.

**Opening the Templates workspace does.** `TemplateGallery.upgradeLegacyToLayers` runs on
mount, by design and documented. Measured: `L|7 R|30` before the click, `L|37` six seconds
after it.

**And that is the correction this audit has to carry, because the harness changed the
thing it measured.** In the run that produced §1, §2, §3 and §5 the shelf had **already
been converted** by the time the first `list_templates` call was made, and this audit
originally recorded that state as "what a fresh install seeds". It is not. Every
measurement in those four sections is therefore a claim about the **post-conversion,
layer-model shelf** — the shelf a church has from the first time anyone opens Templates
onwards — and none of them was a claim about the shelf a fresh install actually boots
with. What triggered the conversion in that particular run is **not settled**: the
console's stored session read `activeTab: "live"`, and a clean repeat of the same boot did
not reproduce it. This audit does not guess.

The pre-conversion region branch was then measured separately and deliberately, on a
virgin shelf with the Templates workspace never opened. It is §1b and the second half of
§2, and it is where the worst thing in this pass is.

**One measurement artefact, named so it is not mistaken for a finding.** A `Range`
spanning wrapped lines returns the union of the line boxes, and that union is a few
pixels wider than the box on a centred or justified wrap. Every "clipped right by
≤ 41 px" reading below is that artefact: on `Classic · Lyrics` the same element reports
`scrollWidth 1613 / clientWidth 1613` and `scrollHeight 821 / clientHeight 821`, which is
a box that does not overflow at all.

---

## 1. The countdown, post-conversion — Track B's rebuilt default block PASSES

The block Track B rebuilt is `.cd-default`, and `showDefaultCountdown` requires
`layered`, so it renders only on a **layer-model** template with no layer bound to the
countdown. **On a fresh install that is 6 rows of 37** — the seven layer-model ones minus
`Stage · Large type`, which has its own bound layer. It becomes 36 of 37 only after the
Templates workspace has been opened once (§0a). Everything in this section is a claim
about that post-conversion shelf; §1b is the other one.

**The scope sentence and the measurement set name different templates, deliberately, and
a reader should not have to notice that.** The six natively layer-model rows are ids
31–36; the five measured below are ids 3, 6, 10, 25 and 30, which are region-model as
seeded and were already converted when this ran. So what follows is evidence that
`.cd-default` behaves correctly **for a layer-model template**, gathered on five rows that
became layer-model by conversion rather than by seed. It is not evidence about any
template in the state a fresh install ships it in — that is §1b.

Measured at **1920×1080** and **1280×720**, on `Classic · Scripture` (6), `Classic ·
Timer` (10), `Lower Third` (3), `Lower Third · Timer` (25) and `High Visibility · Timer`
(30) **as converted**, with a 60-character label and then a 140-character one:

* `.cd-default` fills the stage exactly (`x0 y0 w1920 h1080`) and its computed
  `overflow` is `hidden`. **It clips.**
* The template's own scripture text layers do **not** render alongside it. The failure
  mode this replaces — type painted straight over the template's own layers and off the
  edge of the screen — did not occur in any of the ten runs.
* Both lines carry a `data-fitted` value, so the fit loop ran and reported. On
  `High Visibility · Scripture` at 1280×720 with the 140-character label, the label's
  declared base is `3.2cqw`, the fit settled at `2.609375cqw`, **and that is the size the
  element is actually painting at** (`style.fontSize === "2.60938cqw"`), with
  `scrollHeight 143 / clientHeight 142`.
* No ink left the stage in any run.

Screenshot corroboration at 1920×1080 on `High Visibility · Timer`: label wrapped to two
lines, digits `1:34:15` centred, whole composition inside the frame.

### 1a. FINDING — a running countdown freezes every other text layer at its declared size

`Stage · Large type` (id 37) is the one shipped template with a layer bound to
`countdown`, so `hasTimerLayer` is true and `.cd-default` is correctly suppressed. The
countdown's **label** is delivered through the `reference` binding, which on this
template is a 12 %-high strip (`slt-ref`, declared `3.4cqw`), while the 48 %-high
`verse` layer stays empty.

**Two runs, and the figures are reported per run rather than pooled**, because the label
lengths differ and the fit therefore lands in a different place:

*Run A — 155-character label, 1920×1080:*

| element | declared base | fit computed (`data-fitted`) | size actually painted |
|---|---|---|---|
| label (`reference`) | 3.4cqw | **1.9234375cqw** | **3.4cqw** |
| `COUNTDOWN` (static) | 1.6cqw | 2.4671875cqw | 1.6cqw |
| digits (`countdown`) | 5cqw | 6.2535156cqw | **6.2535156cqw** |
| `NEXT` (static) | 1.5cqw | 2.266796875cqw | 1.5cqw |

*Run B — 118-character label, 1920×1080:* label declared `3.4cqw`, fit computed
**`2.005cqw`**, painted **`3.4cqw`**, box `scrollHeight 221 / clientHeight 130`.

**221 px of text in a 130 px `overflow:hidden` box — 91 px of it clipped.** The screenshot
shows the first line gone entirely and two more sliced through the middle. At 1280×720 the
same template with a shorter label reports `Range` cuts of `top 9 px, bottom 8.8 px` and
the second line is visibly sliced.

**The same template with a verse on it instead of a countdown fits correctly** — every
element's painted size equals its `data-fitted` value. So the variable is the running
countdown, not the template:

```
verse on screen:      base 7   → painted 2.2668cqw  (fit 2.266796875)  ✓
                      base 1.6 → painted 2.46719cqw (fit 2.4671875)    ✓
                      base 1.5 → painted 2.2668cqw  (fit 2.266796875)  ✓
countdown on screen:  base 3.4 → painted 3.4cqw     (fit 1.9234375)    ✗
                      base 1.6 → painted 1.6cqw     (fit 2.4671875)    ✗
                      base 1.5 → painted 1.5cqw     (fit 2.266796875)  ✗
                      base 5   → painted 6.25352cqw (fit 6.2535156)    ✓ (the bound layer)
```

The element is **not** rebuilt — it was tagged with an attribute and the tag survived
four seconds of ticking with the wrong size still on it — so this is not the
`{#key text}` path `reapplyFitted` exists to cover. `data-sized` is already `'1'`, so
`reapplyFitted` declines to touch it, and `fitSig()` folds a ticking layer in by text
*length*, which does not move as digits count down, so no re-fit is ever scheduled.
The observable behaviour is what is asserted here; the mechanism is stated as consistent
with it, not as measured.

**Very probably pre-existing, and the decisive experiment was not run.** `reapplyFitted`,
`isTicking`, `fitSig`, `fitLayers` and the `.lfit` `style` attribute are byte-identical to
the merge base `7851a79`, so nothing on the path that assigns a size changed. What would
settle it — check out the base, fire the same countdown, measure — was available and was
not done, so this is a strong inference from a diff rather than a measurement. The reason
to leave the door open at all: **wave 2 did change this file's reactive graph** (175 lines,
including new reactive state around the countdown block), and the mechanism here is a
re-render overwriting an imperative style write, which is precisely the kind of thing a
changed reactive graph can start or stop doing. Filed as **RG-139**.

A clock layer alone does not reproduce it: a `clock`-bound layer beside a verse painted
its fitted size and still had it eight seconds later. `startClock()` is entered from the
countdown, so a clock on its own is not ticking the component.

### 1b. FINDING — the region countdown a fresh install actually renders collapses to a sixteenth of its designed size

This is the branch §0a says was never measured, and it is the worst thing in this pass.

On a **virgin, unconverted shelf** — Templates never opened, `L|7 R|30` verified in SQLite
before and after every fire — a countdown was fired at 1920×1080 on the seeded region-model
Timer templates:

| template | `.content` box | label painted | digits painted | box |
|---|---|---|---|---|
| 10 `Classic · Timer` | `312.2 × 43.2` | **0.410226cqw = 7.88 px** | **1.57779cqw = 30.3 px** | `scrollHeight 44 / clientHeight 43` |
| 6 `Classic · Scripture` | `83.2 × 34.1` | 0.301554cqw | 1.2758cqw = 24.5 px | `35 / 34` |
| 30 `High Visibility · Timer` | `364.9 × 49.6` | — | — | `51 / 50` |

`Classic · Timer` declares `refSize 2.6` and `verseSize 5`, and the renderer draws a
countdown at **twice** the verse size — so the designed figure is 10cqw, 192 px on a 1920
wall. It paints at **30.3 px**, about a sixth of that, in a 312 × 43 px blob in the middle
of an otherwise empty screen. The label is 7.88 px **and still clipped at both ends**. The
screenshot is unambiguous.

**It is not the label.** Three label lengths — 7, 17 and 60 characters — all produced
byte-identical sizes (`0.410226cqw` / `1.57779cqw`). **And it is not the region fitter in
general**: on the same template a verse fits correctly (`John 3:16` at the full declared
`5.225cqw` / 100.3 px; `Esther 8:9`, the longest verse in the bundled KJV, shrinks to
`2.82338cqw` / 54.2 px and settles with `scrollHeight == clientHeight`). Only the countdown
collapses.

**A first version of this section explained the collapse and the explanation was wrong.**
It said the loop "never converges" and runs to its guard, reading `scrollHeight 44 /
clientHeight 43` as a pixel of unfitted residue. `fitOne`'s own predicate is
`box.scrollHeight > box.clientHeight + 1`, so `44 / 43` is the loop's **stop** condition —
the block measured as fitting — and the floor it would have run to is `FIT_MIN_SCALE =
0.01`, while 30.3 px of a designed 192 px is scale **0.158**, thirty steps clear of it.
So what is recorded here is the observation and not a mechanism: **the fit settled on a
size it measured as fitting, at about a sixth of the designed size, and the result is
unreadable on a wall.** That is rule 37's shape, and this pass did not measure the cause.

One thing that follows and was **not measured**: 0.158 is below `MIN_LEGIBLE_SCALE`
(0.45), so `onFit` should be reporting `legible: false`. This pass did not capture `onFit`
on the output page, so whether anything was reported, and whether any surface showed it,
is unknown.

**And on a lower third, a region countdown paints nothing at all.** `Lower Third` (3) and
`Lower Third · Timer` (25) render `.content` at `1920 × 0` with no text —
`countdownAllowed = !!countdownTo && !bandMode` and the markup's own comment says
*"Deliberately nothing: a countdown does not go out on a lower third."* That is intended
behaviour, and it is recorded here only because **the same template, after conversion,
paints a full-screen countdown over the band** (§1). One template, two answers to the same
fire.

Filed as **RG-141**, and the attribution has two halves that must be read together.

**The DEFECT is pre-existing**: the whole region `fitOne` body is byte-identical to the
merge base, and the only change in that function's neighbourhood is `fitTicker`, appended
after it.

**The SHIPPING SURFACE is new in this wave.** `git show 7851a79:src-tauri/src/db/
templates.rs` contains the string `Timer` **zero times**: before wave 2 each family had
four kinds and a fresh install shipped no template whose purpose was a countdown. It now
ships five, all region-model, and RG-141 is what all five render. `templates.rs` says so
itself at the Aurora block — *"Media and Timer are new"*. So the wave did not write this
defect, and it did put a congregation-facing surface in front of it where there was none.

---

## 2. The ticker — Track B's label budget PASSES, on the path a fresh install takes

`fitTicker` and the 45 % label cap live in the **region** branch of `TemplateRender`
(`.ticker`, `.ticker-label`, `.ticker-track`). **This is an ordinary path, not a
laboratory one.** `Classic · Announcement` (9), `Aurora · Announcement` (14), `Ember ·
Announcement` (19) and `Lower Third · Announcement` (24) all ship region-model with
`"scroll":true` in their seeded style, so an operator who assigns one from Outputs and
fires an announcement without ever opening the Templates workspace gets the ticker band.
(`High Visibility · Announcement` does not carry `scroll` — the five families are not
uniform here.)

Measured first on a **synthetic** region template and then, after §0a corrected the
premise, **on seeded `Classic · Announcement` (9) with the shelf verified unconverted**.
Both agree; the seeded figures are the ones quoted.

At 1920×1080, announcement label 92 characters, body 133 characters, template 9 as seeded:

| | |
|---|---|
| band | `x0 y940.1 w1920 h139.9`, computed `overflow: hidden` |
| label | computed `max-width: 45%`; `clientWidth 778` = **40.5 %** of the stage — inside the cap |
| label overflow | `scrollWidth 778` vs `clientWidth 778` — **nothing clipped**; shrunk to **12.4972 px** |
| body | `.ticker-run` width **5098.6 px**, animation `relay-ticker 56.7s` — **still scrolling** |
| body position | `.ticker-track` starts at `x 883.1`, right of the label's `835.1` — **not shoved off the band** |

The earlier synthetic run, kept for the record: band `1920 × 114.6`, label `clientWidth
802` (41.8 %), `scrollWidth == clientWidth`, shrunk 46.08 px → 14.163 px, `.ticker-run`
5016.7 px at `relay-ticker 60s`.

At 1920×1080, on the synthetic template, label 84 characters, body 184 characters:

| | |
|---|---|
| band | `x0 y965.4 w1920 h114.6`, computed `overflow: hidden` |
| label | computed `max-width: 45%`, `overflow: hidden`; `clientWidth 802` = 41.8 % of the stage — **inside the cap** |
| label overflow | `scrollWidth 802` vs `clientWidth 802` — **nothing clipped**; `fitTicker` shrank it from the declared `2.4cqw` (46.08 px) to **14.163 px** |
| body | `.ticker-run` width **5016.7 px**, animation `relay-ticker 60s` — **still scrolling** |
| body position | `.ticker-track` starts at `x 907.2`, immediately right of the label's `859.2` — **not shoved off the band** |

Screenshot corroboration: the full label reads left of the band, the body crawls through
the remaining width.

### 2a. FINDING — one template renders an announcement two ways, and opening a workspace switches every screen from one to the other

`Classic · Announcement` (9) was fired with the **same** announcement before and after the
conversion, at 1920×1080, on the same channel:

| | before (seeded, region) | after (converted, layer) |
|---|---|---|
| path | `.ticker` band | `.lrun` crawl inside a text layer |
| placement | footer band, `y 940.1`, `h 139.9` | full-frame block; label heading at `y 259.2`, crawl at `y 388.8` in the **middle of the screen** |
| label cap | CSS `max-width: 45%`; `clientWidth 778` = **40.5 %** | **no cap of any kind** — the label layer is `1612.8` wide, **84 %** of the stage |
| label size | shrunk by `fitTicker` to 12.4972 px | fitted `2.3937cqw` and applied |
| `.ticker-label` in the DOM | present | **absent** |

Screenshots of both are unambiguous and they are different designs, not different sizes: a
bottom-third news band, versus a centred two-line heading with a large crawl sliding
across the middle of the wall.

**Nothing measured is broken on either side.** The finding is the split itself: the same
row renders a congregation-facing surface two ways, and the switch is a saving conversion
an operator triggers by opening a workspace. The 45 % budget this wave rebuilt governs the
first rendering and does not exist in the second.

**The operator is not left with nothing, and an earlier draft of this section said they
were.** `TemplateGallery` calls `snapshotTemplateVersion(t)` before each conversion
precisely so there is a way back, and it renders *"Converted N classic templates to
layers. Earlier versions are in each template's History."* into a `role="status"` pane.
So it is neither silent nor irreversible. What it does not say — and this is the whole
finding — is that **the look of every announcement in the building has just changed**. A
line about converting classic templates to layers is a statement about a data model; an
operator has no way to read it as "your notices now paint across the middle of the wall
instead of along the bottom". **Neither the pane nor a History restore was measured by
this pass**: both are read off the source, and whether a restore actually returns the
region rendering is unknown.

Its test belongs on **both** paths. Today nothing distinguishes `.ticker` from `.lrun`, so
a green suite says nothing about which of the two a church is looking at. Filed as
**RG-140**.

---

## 3. The contrast panel — PASSES

Region-model, and reached on a synthetic template rather than a seeded one — no seeded row
carries `textPanel` at all (`grep -c textPanel src-tauri/src/db/templates.rs` returns
**0**), so this is a claim about the panel code, not about a shipped look. `Esther 8:9` (the longest verse in the
bundled KJV) on a panelled template over a `#fff6d8` background, `panelOpacity 0.5`:

| | 1920×1080 | 1280×720 |
|---|---|---|
| `.content.panel` computed | `overflow: hidden`, `max-width: 82%`, `max-height: 92%` | same |
| box | `x283 y152.8 w1354 h774.3` — inside the stage | — |
| overflow | `scrollWidth 1354 / clientWidth 1354`, `scrollHeight 774 / clientHeight 774` | no clipped ink |
| ink | `x379.2 y222 w1171.5 h578.5`, wholly inside the plate | `x252.9 y146.7 w780.9 h386.6` |

The words never leave the plate, and because the box clips, `overflowing()` has a signal
to read — which is the half of this that the fit loop depends on.

---

## 4. The default template — Track A PASSES, both halves

Set up on the real backend: channel 1 `template_id = NULL` (*Follow the content look*),
all five content looks `NULL`, so the default is the only thing left in the chain.
`output.html?channel=1` open, `John 3:16` fired.

**Live re-resolve, no reopen.** The default was changed three times through
`set_default_template` and the already-open page re-resolved each time, with the verse
still on it:

| default | accent | background | verse colour |
|---|---|---|---|
| 6 `Classic · Scripture` | `#e8a33d` | `radial-gradient(… #2a2013 …)` | `rgb(244,228,200)` |
| 26 `High Visibility · Scripture` | `#ffffff` | `#000000` | `rgb(255,255,255)` |
| 16 `Ember · Scripture` | `#ffb066` | `radial-gradient(130% 130% …)` | `rgb(253,238,222)` |
| 11 `Aurora · Scripture` | `#6ee7c4` | `radial-gradient(130% 130% …)` | `rgb(234,255,248)` |

**The `hello` reply.** Read off the wire rather than inferred: a raw WebSocket client
connected to `:8031` with `Origin: http://127.0.0.1:8032` and sent
`{"kind":"hello","template_id":null}` — the shape `Output.svelte` sends for a
channel-keyed URL. Two frames came back, in this order:

```
{"kind":"default_template","template":{"id":11,"name":"Aurora · Scripture","layout":{…}}}
{"kind":"content","reference":"John 3:16","text":"For God so loved the world…","template_id":6,
 "template_json":null,"template_pinned":false,"content_kind":"scripture",…}
```

So the hub carries the operator's configured default on `hello`, **and** the retained
content frame behind it. Reloading the browser source confirmed the same thing end to
end: the page came back painting Aurora with `John 3:16` already on it — rule 43 holding
alongside Track A.

Note for the record: the retained content frame carries `template_id: 6`, the default as
it stood when the fire happened. It defers (`template_pinned: false`, `template_json:
null`) and the page resolved to 11, which is DECISIONS §29 behaving as written.

---

## 5. The twenty-five — PASSES

**Counted on the fresh profile.** The shelf is **37** rows: the 5 built-ins (ids 1–5), the
**25 families** (ids 6–30, five families × five kinds, `Classic · Aurora · Ember · Lower
Third · High Visibility` × `Scripture · Lyrics · Media · Announcement · Timer`), and 7
shelf presets (ids 31–37). Twenty-five is the family seed, not the gallery total, and the
gallery total is 37 by construction. Per §0a, 30 of those 37 are region-model as seeded.

**Every family rendered at every kind**, 25 runs at 1920×1080 against the real backend,
**on the converted layer-model shelf** — the `.lrun` crawl reported for every
`· Announcement` row is itself the evidence of that, and it is the second of the two
renderings in §2a, never the first —
scripture by `manual_fire`, lyrics/media/announcement by `fire_content`, timer by
`start_countdown`. Each produced its own content on the stage. No ink left the stage in
any of the 25, with two readings that are not defects:

* the announcement crawl (`.lrun`) extends past the band while it scrolls — that is what
  a crawl is;
* the wrapped-line `Range` artefact described in §0.

---

## 6. The two registers — Track C PASSES

Measured on the console at `:5032` with the relayed bridge (commands real, events
stubbed), on `Classic Serif` in the template editor.

The two registers are now visibly and semantically different things:

* **USED FOR** — the content-look register, a chip row, with the line *"A kind ticked
  here wears this template on every screen set to Follow the content look. A screen with
  a look of its own keeps it."*
* **Content this template renders** — its own heading, five `role="switch"` rows
  (`te-showrow`), each reading `Scripture Shows`, `Songs / Lyrics Shows`, `Media Shows`,
  `Announcements Shows`, `Timer / Countdown Shows`, with the line *"An unticked kind is
  not blanked — a screen wearing this template holds what it already had."*

**The first click.** Before: all five `aria-checked="true"`. One click on the first row:

```
Scripture        aria-checked=false   "Scripture Ignores"
Songs / Lyrics   aria-checked=true    "Songs / Lyrics Shows"
Media            aria-checked=true    "Media Shows"
Announcements    aria-checked=true    "Announcements Shows"
Timer/Countdown  aria-checked=true    "Timer / Countdown Shows"
```

Exactly one row changed, and it changed to a word — `Ignores` — that says what it now
does. Nothing reads as though four kinds were wiped. The toggle was clicked back and the
template was **never saved**.

Also observed on this surface, and noted rather than filed: the Templates workspace shows
no Themes desk, consistent with Track E, and the five content looks all read
*"Default · Aurora · Scripture"* — the default falling through for every kind, which is
what §4 set up.

---

## 7. What this pass does not change

The release decision, the model question and word error rate are all untouched by
everything above. None of the three findings is a detection fault, and none was reached by
the Rust or frontend suites, which were green on this tree throughout.

**No defect here was written by this wave**, and that is not the same as the wave having
no part in any of them. RG-141 and RG-140 are pre-existing by diff over the functions
involved, RG-139 by diff with the caveat in §1a. What the wave built — `.cd-default`,
`fitTicker`, the panel clip — holds everywhere it is reached.

Two qualifications, because the blanket sentence flatters the wave:

* **RG-141's shipping surface is new in it.** The Timer kind did not exist at the merge
  base; five region-model countdown templates now ship, and a fresh install therefore has
  a congregation-facing countdown where it previously had none (§1b).
* **RG-140's asymmetry is new in it.** Both renderings pre-date the wave; the 45 % label
  budget that makes them differ in the way the row describes was built in it (§2a).

## 8. What this pass could not see

* The **distance preview** at 2.4 m — an unfinished check from this pass's own brief (§0).
* Anything that requires the native Tauri window's own pixels. The console was rendered
  in a browser; its *layout* under the packaged webview is not evidenced here.
* Anything event-driven on the console, because the console's events were stubbed.
* Behaviour on Windows. Every measurement is macOS, one machine, one build.

---

<!-- ===== was docs/qa/audits/DESIGN.md, merged 2026-09-21, verbatim ===== -->

# DESIGN-2026-09-16-WAVE3 — the wave 3 browser-driven pass

**Six scenarios, driven rather than read.** Wave 3 merged five tracks into
`feat/wave3-timers` (Track A the registry, Track B publication and retention, Track C the
`DECISIONS.md` §27 reversal, Track D the warning threshold, Track E the eleventh content
kind and the operator's surface). Both suites are green on that tree, and the suites are
blind to the class of defect that matters here: a timer is a moving render on three
surfaces, and `qa-inventory` reported **0 handlerless buttons, 0 unnamed controls and 0
unaddressed commands** throughout this pass — as it did through the two passes that found
this project's worst defects, and it was right every time.

Two things in this wave had never been in a layout engine at all, and both are named by
the agents that wrote them: the stage page's `.progrow` sizing
(`92cqw / --tmrs / 6 / 0.62`, capped at `9cqh`) and Live's programme timer band's wrap
behaviour at narrow widths. **Both were measured here, and there is a finding in each.**

This is **frozen evidence and findings only.** Nothing here records a fix. Ten findings
are filed in `docs/qa/RELAY_GAP.md` as RG-146 … RG-155 and are not restated anywhere else.

> **RENUMBERED AT THE WAVE 3 / WAVE 5 MERGE, 2026-09-17.** The RG ids in this pass are
> unchanged — wave 3 filed first and keeps them. What moved is the decision it cites:
> **wave 3's DECISIONS §89 became §91**, because wave 5 had already written a §89 and a §90
> and is in an open pull request. Every `§89` in the body below now reads `§91` and means
> the same section. Wave 5's own §89 is a different decision about the same page, and its
> third part — which ratified the stage `alert` SURVIVING a panic control — was reversed by
> the operator in favour of §91's ruling.

---

## 0. Method, and the line between two different claims

| | |
|---|---|
| Date | 2026-09-16 |
| Tree | `feat/wave3-track-f`, branched from `feat/wave3-timers` at `1a242d8` (all five tracks merged, plus Track C's own follow-up fix). Working tree carried only the temporary audit scaffolding described below |
| Backend | `src-tauri/target/debug/relay`, built from that tree and run through `npx tauri dev --no-watch` with a config overlay pointing `devUrl` at a Vite server on **:5133** — port 5032 was held by another checkout of this repository, and pointing the webview at it would have run a different tree's console against this backend |
| Profile | **The developer's existing dev database**, `~/Library/Application Support/com.relay.app/relay.db` — **not** a fresh install. The wave 2 pass used `HOME=/tmp/relayfresh`; that is not available to this agent, and the difference matters for exactly one thing below (the default template resolved to `Ember · Scripture`, id 24, which is this machine's configured default and not a seed value). **The timer registry is process memory with no database behind it at all**, so every timer measurement here is independent of the profile. Startup line: `profile: active 'Default' · lang Some("en") · sensitivity 0` |
| Frontend | Vite on `:5133`; the embedded HTTP server on `:8032`; the kiosk hub on `:8031` |
| Browser | gstack's headless driver (`~/.claude/skills/gstack/browse/dist/browse`). This machine cannot screenshot the Tauri window, which is why all visual work happens in a browser |
| Not run | **`prefers-reduced-motion` was never actually flipped.** The driver has no media-emulation command, so §5's reduced-motion half is read out of the live CSSOM and is NOT a claim that the page was rendered under that setting. It is marked PARTIAL there and nothing here should be read as more |
| Not run | **The dock's own countdown warning** (`Dock.svelte:589`). Reaching it needs a live countdown through the console's event path, and the console's events were not relayed. Marked NOT TESTED in §5 |

**Three different claims, and they must not be blurred.**

* **The output and stage pages were measured against the REAL backend.**
  `http://127.0.0.1:8032/output.html` and `/stage.html` are served by Relay itself, they
  join the real kiosk hub on `:8031`, and every timer, fire and panic control below was
  performed by the real Tauri commands. Nothing on this surface is mocked.
* **The hub wire was read directly.** A plain WebSocket client on `:8031` recorded every
  frame with a timestamp, exactly as an OBS browser source or a stage tablet receives it.
  `channels::kiosk_origin_allowed` returns `true` for an absent `Origin`, so the recorder
  needed no special case. **Where this document quotes a frame, that is the bytes that
  left the machine**, not an inference from the code.
* **The console at `:5133` was rendered in a browser, where it has no backend of its
  own.** It was given a MOCK one: a fixed dispatch table of canned answers, installed by
  hand after the page had loaded. **Nothing the console did in this pass reached Relay.**
  A console reading below is a claim about LAYOUT and never about behaviour.

**The scaffolding is temporary and is not part of the product.** Two files —
`src/lib/__auditbridge.js` (a FIXED scenario of real commands, run once on load inside the
Tauri webview, taking no instructions from anywhere and emitting only an outbound step log)
and `src/lib/__auditmock.js` (the console's fake bridge) — plus one line in `src/main.js`
and a `tauri.audit.conf.json` overlay. All four were removed before the suites were run and
before anything was committed. Same shape and the same precedent as the wave 2 pass.

**Measurement, not eyeballing.** Geometry is `getBoundingClientRect`; text width is a
`Range` over the text node compared against the box; overflow is
`scrollWidth`/`scrollHeight` against `clientWidth`/`clientHeight`; *coverage* is
`elementFromPoint` over a control's own centre. Screenshots are corroboration, never the
measurement. One trap this pass hit and names so the next reader does not: on the stage
row `.tval` is a block filling its flex item, so **`scrollWidth === clientWidth` is true
while the text inside it is clipped** — the same artefact the browser harness note records
about a truncated `<select>`. Every clipping claim below is a `Range`.

---

## 1. The re-aim after a verse — Track A's whole reason for existing — PASSES

**The defect this wave exists to fix.** Read off the wire, `EARLY` client, one run:

| at | what the hub received | |
|---|---|---|
| 54.91 s | `content` · `content_kind: countdown` · `countdown_to: 1789590877514` · `countdown_from: 1789590577514` · `template_id: 24` · `template_pinned: false` | a five-minute congregation countdown, span 300 000 ms |
| 57.69 s | `content` · `content_kind: scripture` · `John 3:16` | the verse replaces it on the wall |
| 60.36 s | — | `adjust_countdown{remaining_ms: 600000}` returned **`ok`**. **NOTHING WAS PUBLISHED.** There is no frame between 57.69 and 63.08 |
| 62.40 s | — | `list_timers` shows the `both` timer at `target_ms 1789591182943`, `from_ms 1789590577514` |
| 63.08 s | `content` · `content_kind: countdown` · `countdown_to: **1789591182943**` · `countdown_from: 1789590577514` | `show_timer` — the explicit way back, carrying the **adjusted** figure |

Every clause of the ruling in the plan's item 8 holds, measured rather than argued:

* The re-aim **succeeds** where it used to answer *"Nothing is counting down."*
* It **changes the registry** — the target moved by 605 429 ms from the original aim.
* It **paints nothing**: the wall count does not move and `John 3:16` is still the last
  thing on it.
* `show_timer` is the way back and it carries the adjusted figure, not the original five
  minutes.
* `countdown_from` is **not re-stamped** — same value in both countdown frames — so the
  warning span stays the countdown's own length.
* The template triple rides over verbatim (`template_id: 24`, `template_json: null`,
  `template_pinned: false`) on both, so nothing was silently re-skinned.

---

## 2. A programme timer under traffic — Track B — PASSES

A `Stage` timer was started and then a verse, a song and a notice were fired over it:

```
65.64  timer    timers:[{id:7,label:"Sermon",countdown_to:1789592088235,…}]
67.32  content  content_kind:"scripture"
68.85  content  content_kind:"song"
70.44  content  content_kind:"announce"
72.13  list_timers → both the `both` timer (id 6) and the `stage` timer (id 7) still there
```

**One `timer` frame and no others.** The set is published when it changes and at no other
time, so a stage tablet holds the row it has and the figure ticks locally. **It does not
blink**, and the mechanism is that there is nothing to blink: no frame arrives.

The congregation timer survived the verse, the song and the notice too — which is §1's
guarantee seen from the registry side rather than the wire side.

---

## 3. `Esc`, then `B` — Track C — PASSES on the congregation guarantee, and §7 is the exception

| at | what happened | |
|---|---|---|
| 72.95 s | `content` · countdown `Back in` | a congregation countdown is put up |
| 75.07 s | `{"kind":"clear"}` | `Esc` |
| 76.71 s | `list_timers` → **only** id 7 `Sermon` (`scope: stage`) | the congregation timer is gone; the programme timer stays |
| 77.40 s | `{"kind":"black"}` | `B` |
| 79.02 s | `list_timers` → still only id 7 | the programme timer survives the harsher control too |
| 79.58 s | `adjust_countdown` → **`Err("Nothing is counting down.")`** | the transport still cannot bring back what a panic control took |

So DECISIONS §91 holds on the wire, in both directions: everything a congregation can see
goes on both controls, the preacher's clock does not, and the refusal string is still true
in the only case that can now produce it.

**The word to the preacher.** §91's last part decides that a stage `alert` comes down with
the screens. That was **not** reached from this pass — no `stage_alert` was sent in the
recorded run, and the console mirror of it (RG-145, closed by `1a242d8`) is a console
claim which this pass did not drive. **Not tested.**

**What §7 found instead is the other half of the same sentence**, and it is a finding
rather than a pass: `B` does not take the preacher's *up next* down, while `Esc` does.

---

## 4. A stage tablet that joins mid-service — Track B — PASSES, in the stated order

A reading was put up, then a second WebSocket client connected and sent
`{"kind":"hello","template_id":null}` — the shape `Output.svelte` sends for a
channel-keyed URL. Three frames came back, in this order:

```
{"kind":"default_template","template":{…}}
{"kind":"timer","timers":[{"id":7,"label":"Sermon","countdown_to":1789592088235,…}]}
{"kind":"content","content_kind":"scripture",…}          ← the reading, LAST
```

**Configuration, then the timers, then the screen frame.** The reading is painted last, so
there is no moment at which a clock stands over the verse. Rule 43's trap 1 holds from the
other side too: no `timer` frame ever replaced the retained screen frame, and the
`stage_next` frames published in the same run were never retained.

Rendered rather than only read: the stage page was reloaded against the running backend
and came back with both the programme row and the reading on it.

---

## 5. The warning threshold on all three surfaces — MIXED

### 5a. The wall — PASSES

A congregation countdown with a two-minute span, sampled at `0:10` remaining
(`output.html?channel=1`, 1920×1080, real backend):

```
.countdown  class="lfit countdown svelte-1loewgc warn"
            color: rgb(244, 81, 91)
            animation-name: svelte-1loewgc-cdwarn
            font-size: 199.68px      box 1651.2 × 209.7
```

The default rule's short-countdown branch is the one in force — a tenth of a 120 s span is
a 12 s window — and it came on when it should.

### 5b. Reduced motion — PARTIAL, and the reason is stated

The CSSOM on the live page carries **both** branches, read out of `document.styleSheets`:

```
@media (prefers-reduced-motion: no-preference) { .countdown.warn { animation: 2s … cdwarn } }
@media (prefers-reduced-motion: reduce)        { .countdown.warn { text-shadow: rgba(244,81,91,.85) 0 0 0.25em } }
```

The colour is applied inline either way, so the warning is visible under both settings.
**The page was never rendered under an actual reduced-motion setting** — the driver cannot
flip the media feature — so this is a claim about the rules the engine has parsed, not
about a render. Track D Task 13 said *assert this, do not rebuild it*; this is the assert,
and it is one step short of the rendered proof.

The dock's warning is colour only, with no reduced-motion glow, **deliberately**, and its
own comment says so. That is not reported here as a defect.

### 5c. The stage — FAILS, and nothing warns at all

`warn_ms` **does** ride the wire. Read off the hub:

```
{"kind":"timer","timers":[…,{"id":9,"label":"Wrap up","countdown_to":…,"warn_ms":30000}]}
```

Nothing reads it. `Stage.svelte`'s `programme` mapping takes `id`, `label` and
`countdownRemainingMs` and stops there, and `.progrow` was measured with
`querySelectorAll('.warn').length === 0` at every viewport tried, including with a timer
sitting inside its own explicit 30 s window and with an expired one at `0:00`. The comment
at `Stage.svelte:257` still says the threshold is *"wave 3 Track D's, on all three surfaces
at once"* and *"deliberately not read here yet"*. Track D has landed. Filed as **RG-148**.

### 5d. The threshold a person chooses reaches no congregation screen and no stage — FAILS

Three separate breaks in one chain, each checked rather than assumed:

* **No call site passes the argument.** `countdownWarning(remainingMs, totalMs, warnMs)`
  gained its third parameter in Track D, and all three readers call it with two —
  `TemplateRender.svelte:1145`, `Stage.svelte:220`, `Dock.svelte:589`.
* **A `Both` timer has no field to carry one.** `start_countdown` hard-codes
  `warn_ms: None` (`main.rs:2867`), and neither `timers::project_both` nor
  `main::countdown_content` has a warn field at all, so the congregation wire form cannot
  express a chosen threshold even if a surface read one.
* **The Settings default is console-only, and this is the bundle-level half.**
  `layers.js`'s `warnDefaultMs` is module state set by `setCountdownWarnDefault`, whose one
  caller is `stores/capture.js`. `output.html` and `stage.html` import the shared
  `layers-*.js` chunk but **not** `capture.js`, and the settings key proves it: after
  `npm run build`, `countdown.warn_ms` appears **once** in `assets/main-*.js` and **zero
  times** in `assets/output-*.js` and `assets/stage-*.js`. So an operator who sets
  `Settings → General → Countdown warning` changes the console and changes neither the wall
  nor the preacher's page, which keep the shipped 60 s.

`ServicePlanner.addCountdownCue` gained no per-cue field either (`grep -c warn` over that
file returns matches only for unrelated stale-arrangement wording). Filed as **RG-149**.

---

## 6. A rehearsal — Track B — PASSES on the tablet, FAILS on what it leaves behind

With `set_rehearsal(true)`, a `Stage` timer was started and a verse fired. The backend
printed both refusals and **the hub received nothing at all**:

```
rehearsal: publish_timers SUPPRESSED — nothing left the machine
rehearsal: broadcast SUPPRESSED — nothing left the machine
```

No `timer` frame and no `content` frame reached either client, watched at the hub itself
rather than at a Tauri event — which is the only assertion surface that can see this, per
the rehearsal bug `stage_next` carried for months.

**What the rehearsal leaves behind is the finding.** The timer created inside it
(`Rehearsal only`) is still in the registry after `set_rehearsal(false)`, and ending the
rehearsal published `clear` and `stage_next` and **no `timer` frame**. So the stage
tablet's set stayed at `[7, 9]` while the registry held `[7, 9, 11]`, and the rehearsal's
timer will appear on the preacher's monitor, unannounced, at the next unrelated publish.
Filed as **RG-150**.

---

## 7. FINDING — `B` leaves the preacher's "up next" standing, and `Esc` takes it down

Driven deliberately with **no clear before it**, because the console's own
*"the up next must not outlive the content"* watcher keys off `live` going falsy and a
preceding clear would already have taken that edge:

```
27.13  content     content_kind:"scripture"   (Romans 8:28)
28.55  stage_next  {"label":"Up next","text":"Psalm 23"}
30.18  black       {"kind":"black"}            ← and NOTHING else
32.75  clear       {"kind":"clear"}
32.75  stage_next  {"label":null,"text":null}  ← only now does the word come down
```

The root is one line apart from its twin, in `stores/capture.js:507-508`:

```js
await listen('output://clear', () => { live.set(null); screenBlack.set(false); leavePlan(); … });
await listen('output://black', () => {                 screenBlack.set(true);  leavePlan(); … });
```

`output://clear` clears `live`; `output://black` does not. `Live.svelte`'s subscription
fires on the truthy→falsy edge of `live`, so a blackout never reaches it and
`setStageNext(null, null)` is never called. The comment directly above those two lines
documents this exact defect being fixed **for the clear**, in detail, and the black
listener has it unfixed underneath — the repository's signature shape, a guarantee kept on
one door and skipped on its twin.

It is also §91's own quoted rule, one control along: *"the harsher control must never do
less than the milder one… an operator who has just hit the emergency key cannot be asked to
remember that it reaches three screens out of four."*

A second consequence, not measured as a rendered defect but following from the same line:
`$live` stays populated after a blackout, so any console surface reading `$live` as *what
is on the screens* is wrong after `B`. Filed as **RG-151**.

---

## 8. FINDING — a programme timer past one hour is clipped on the preacher's screen

The `.progrow` sizing had never been in a layout engine. Six programme timers on the real
backend, one of them 95 minutes so `formatCountdown` grows to `h:mm:ss`:

| viewport | `.tval` font-size | text (`Range`) | box | clipped |
|---|---|---|---|---|
| 1920 × 1080 | 64 px (the clamp cap) | 268.8 px | 305.7 px | none |
| 1280 × 720 | 51.276 px | **215.3 px** | **199 px** | **16.3 px** |
| 1024 × 768 | 40.724 px | **171.0 px** | **156.3 px** | **15.0 px** |

`.tmr` is `overflow: hidden`, and `t.scrollWidth - t.clientWidth` reports `16` at 1280.
The screenshot is unambiguous: the row reads **`1:30:1`** with the final digit sliced
through, running straight into its neighbour's `0:00` with no gap, so the two figures read
as one number.

The arithmetic: `.tval` is
`min(clamp(16px, calc(92cqw / var(--tmrs) / 6 / 0.62), 64px), 9cqh)`. The `/ 6` is a
six-character budget and `formatCountdown` emits **seven** past an hour. The measured mono
advance is **0.600**, so a seven-character value is `7 × 0.6 / (6 × 0.62) = 1.129` of the
box — it overflows by about 12.9 % **whenever the formula rather than the 64 px cap is in
force**, which is any width below roughly `259 × timers` CSS pixels. Six timers therefore
clip on anything narrower than about 1554 px: a 1280 or a 1024 tablet, which is what a
stage screen usually is.

A ninety-five-minute clock is an ordinary thing for a church to set — a livestream, a
whole service. Filed as **RG-147**.

Two things measured alongside it and **not** defects: the long label ellipsises exactly as
its comment says (`scrollWidth 1014` in a `clientWidth 199` box, `text-overflow: ellipsis`),
and a label-less timer renders digits alone with no collapsed box.

---

## 9. FINDING — the height cap measures the frame, which is the thing its comment says it replaced

The same declaration's `min(…, 9cqh)`. `.progrow` declares `container-type: inline-size`,
which establishes an **inline-axis** container only, so `cqh` inside it has no eligible
container and falls through to the viewport. Decisive measurement, at 1920 × 500 with three
timers:

```
row height           74.4 px      → 9 % of the row would be 6.70 px
viewport height      500 px       → 9 % of the viewport is  45.00 px
.tval font-size                                            45.00 px   ← the viewport
```

The comment above it reads *"Its own container, so the digits are a share of THIS row and
not of the frame — … the bug that rule replaces is a figure that looked right at one width
and overflowed at every other."* The `cqw` half is true and resolves against the row; the
`cqh` half — the one that is supposed to stop the digits outgrowing the row's height — does
not.

**What this does NOT cause, stated so the row is not read as worse than it is.** No
clipping was produced by it at any viewport tried, including 844 × 390 and 1600 × 300,
because a cap that tracks the viewport shrinks with it. The defect is that the cap is not
the cap anybody wrote, so nothing bounds the digits against the row if the row's own height
ever changes. Filed as **RG-154**.

---

## 10. FINDING — the wave's own Start control is not clickable at its centre in the default window

Live's programme timer band, the second thing that had never been in a layout engine.
Measured on the console (mock bridge — **a layout claim, not a behaviour claim**) with six
programme timers:

| viewport | `elementFromPoint` over the centre of `Start timer` |
|---|---|
| 1920 × 1080 | the button |
| 1600 × 900 | the button |
| **1320 × 860** — `tauri.conf.json`'s own default window | **`.r-select.xpick.xdur`** |
| **1280 × 800** | **`.r-select.xpick.xdur`** |

At 1280 × 800 the geometry is exact: the button occupies `x 567.4–641.6, y 327–349`; the
transport column's transition-duration `<select>` occupies `x 562–648, y 335.2–357.2`. They
overlap over a 74 × 13.8 px region containing the button's centre and its bottom-right
corner. Only the button's top-left corner still hits the button. Both elements are
`position: static, z-index: auto`, so this is a plain layout collision and not a stacking
order anybody chose.

The cause is one level up and **pre-existing**: `.rack` is 320.2 px tall inside a `.con-top`
that is 268 px tall, and `.con-top` is `overflow: visible`, so the transport column paints
52.2 px past the bottom of the monitors row at 1280 and 36.4 px at 1320. What is new is
that the wave put an interactive control in the overflow. Filed as **RG-146**.

`Clear screens` was hit-tested at every viewport in this pass and returned itself every
time, including at 960 × 640 — rule 15 and rule 44 hold. **This is not a panic-control
finding.** It is the same shape one control along.

**The band's own claim, measured.** Its comment says it *"wraps rather than scrolling"* and
that this *"costs it one row of height and nothing else"*. With six timers it is **two**
lines at 1920 × 1080 (60 px) and **three** at 1320 × 860 and 1280 × 800 (92 px), and the
slide grid below it — *"the thing an operator picks from most often"* — falls from 381.6 px
at 1920 to **158 px** at 1280. The band never scrolls and never clips a chip, so the
mechanism is as described; the cost is not. Recorded inside RG-146 rather than as a row of
its own.

One thing checked and **not** filed: at 960 × 640, the smallest window
`tauri.conf.json` permits, the slide grid starts below the fold and the document does not
scroll. With the timer band emptied to zero chips the grid still starts below the fold
(618.2 px against a 640 px viewport), so that is the pre-existing Live layout at the
minimum size and not something this wave caused. The band makes it 34 px worse.

---

## 11. FINDING — an expired programme timer sits at `0:00` for the rest of the service

`done_msg` is carried on the timer frame (`"countdown_done": ""` in every row above) and
**nothing on the stage page reads it**. `programme` maps `id`, `label` and the remaining
milliseconds; a timer that reaches zero keeps `countdownRemainingMs` of `0`, survives the
`!= null` filter, and paints `0:00`. Observed on the real backend in both hold runs: a
45-second `Offering` timer read `0:00` for the following twelve minutes, and the screenshot
at 1280 × 720 shows two of the six rows in that state.

The congregation path has an answer for this — the `Both` wire form's `countdown_done`
replaces the digits, and it was seen working on the stage mirror in the same screenshot,
which reads `Welcome` where the congregation countdown had run out. The programme row has
none. Filed as **RG-153**.

---

## 12. The unreachable half of the wave — FINDING

`show_timer` and `adjust_timer` have wrappers in `stores/capture.js`, in the documented
THROWS group, held there by `timerwrappers.test.js`. **Neither has a rendered control.**
`grep` over `src/**/*.svelte` returns a single hit between the two names — a comment in
`views/Live.svelte` explaining that neither is reachable from there. By
CLAUDE.md's stricter test — *whether a rendered control can get there, not whether a
wrapper exists* — the wave's headline way back onto a congregation screen is
operator-unreachable.

`qa-inventory` reports **0 commands registered in Rust that no frontend caller addresses**
and is right: it traces to the wrapper, which is exactly one hop short of the question.

What an operator can actually do today, enumerated:

* **Start a programme timer** — yes. Live's band, `scope` hard-coded to `'stage'`.
* **See what is running** — yes, on the same band, with a real three-way answer
  (`Reading…` / `No programme timer.` / the reason, keeping the last good list).
* **Stop one** — yes, per chip.
* **Re-aim the congregation countdown** — yes, but only through the dock's transport
  (`adjust_countdown`), which addresses *the newest `Both` timer* and cannot name one.
* **Put a timer back on the congregation screens after a verse** — **no.** This is the
  action the plan's item 8 specifies as the way back, and it exists only as a command.
* **Start a `Both` timer with a chosen `warn_ms` or a `plan_item_id`** — **no.** Live
  passes `scope: 'stage'` and no `warnMs`, and `start_countdown` (the dock's one-press
  countdown) takes neither.

Filed as **RG-152**.

---

## 13. Site 7 of the content-kind sweep — checked on a real screen, and it holds

The exposure `layers.js` records is that a template carrying an explicit `layout.shows`
allow-list drops any kind not on it, silently, before `resolveOutputTemplate` is consulted.
The congregation timer is broadcast as `countdown`, which every seeded list already names,
so nothing needed touching — and this pass confirmed it on a screen rather than in the
source: `output.html?channel=1` resolved to the configured default, id 24 `Ember ·
Scripture`, a `theme_templates()` row whose seeded layout carries
`"shows":["scripture","song","media","announce","countdown"]`, and it **painted the
countdown** in every run in §1, §3 and §5.

The count in the comment is wrong, though, and it is wrong twice. `layers.js` calls
`db/templates.rs::theme_templates()` *"the thirteen on the preset shelf"* in two places;
it returns **twenty-five** (five families × five kinds, listed by name in the source). The
claim the comment exists to make is unaffected — all twenty-five do carry an explicit list
of exactly the five current kinds, pinned by `every_seeded_template_says_which_kinds_it_
renders`, whose own `all` array is a third hand-mirrored copy of `CONTENT_KINDS`. Filed
with one other stale count as **RG-155**.

---

## 14. What this pass does not change

The release decision, the model question and word error rate are all untouched by
everything above. None of the ten findings is a detection fault, and none was reached by
the Rust or frontend suites, which were green on this tree throughout:

```
Rust      814 passed; 0 failed; 16 ignored
frontend  2327 passed (2327), 151 files
e2e.rs     76 passed; 0 ignored
```

**Attribution, stated per finding rather than blanketed.** Two are wholly pre-existing and
the wave merely put something in front of them: **RG-146** (the `.rack` overflow predates
the band that now sits in it) and **RG-151** (`output://black` has never cleared `live`).
Seven are wholly wave 3's own, in code this wave wrote: **RG-147, RG-148, RG-149, RG-150,
RG-152, RG-153, RG-154**. **RG-155 is one of each** — its `layers.js` half is Track E's
comment, and its CLAUDE.md half was made false by wave 2 Track A's
`output://default_template`, so the count in CLAUDE.md's own reproducing command was
already one short before wave 3 began and the wave 2 pass did not catch it.

**Two of the seven are the wave contradicting its own plan** rather than getting code
wrong. Track D's Task 11 landed the third argument and nothing passes it; Task 12 landed
the Settings default and not the per-cue field. Track E's Task 15 landed a control for the
half of the registry that never touches a congregation screen, and the half that does has
none.

## 15. What this pass could not see

* **`prefers-reduced-motion` actually set.** §5b is the CSSOM, not a render.
* **The dock's countdown warning.** It needs a live countdown through the console's event
  path, which was not relayed.
* **`show_timer`'s refusal of a `Stage` timer.** Read in the source, never driven.
* **§91's stage `alert`.** No `stage_alert` was sent in the recorded runs.
* **A fresh install.** Everything here ran against an existing dev database (§0). Nothing
  about the timer registry depends on it, and the template resolution in §13 does.
* Anything that requires the native Tauri window's own pixels. The console was rendered in
  a browser; its *layout* under the packaged webview is not evidenced here.
* Behaviour on Windows. Every measurement is macOS, one machine, one build.

---

<!-- ===== was docs/qa/audits/DESIGN.md, merged 2026-09-21, verbatim ===== -->

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

---

<!-- ===== was docs/qa/audits/DESIGN.md, merged 2026-09-21, verbatim ===== -->

# 2026-09-17-WAVE4-STAGE-PLANNER — the wave 4 browser-driven pass

**Eight scenarios and one deferred legibility list, driven rather than read.** Wave 4
merged three tracks into `feat/wave4-stage-planner` — Track A gave the stage's programme
rail a warning state, a finished state, a held state, a zone switch and a floor; Track B
bound a plan cue to a clock and gave the run surface a slide sizer; Track C specified the
targeting the wave deliberately did not build. Both suites are green on that tree, and
both of wave 4's surfaces are moving renders that no static instrument in this repository
can see. The two congregation-facing defects the 2026-09-10 pass found — a first verse
sliced through the middle (rule 42) and a screen that reconnected coming back blank
(rule 43) — were invisible to every one of them, and `qa-inventory` reported zero
problems throughout and was right both times.

This is **frozen evidence and findings only.** Nothing here records a fix. Four findings
are filed in `docs/qa/RELAY_GAP.md` as **RG-162 to RG-165**, and the gap this pass filed
alongside them is **RG-161**; none of it is restated anywhere else. **Those are the
REGISTER's ids and not this document's** — this sentence sits above the freeze and so has
to point where a reader can actually look. It said RG-146 to RG-149, which are real rows
belonging to wave 3, so it resolved and resolved wrongly; it also said four where five
were filed. Everything from the note below downward keeps the ids of the day, and the
note says why.


> **RENUMBERED, NOT REWRITTEN.** The four findings this pass filed as RG-146 … RG-149
> are **RG-162 … RG-165** in the register, and the gap it filed as RG-145 is **RG-161**.
> Wave 3 had already filed RG-145 … RG-149 for entirely different findings from the same
> branch point, and the collision only became visible when the branches met. The map is
> `docs/qa/RELAY_GAP.md` §23a. **Every id below is left exactly as it was written on the
> day**, because a frozen document that follows a renumbering stops being evidence of
> what was found.


---

## 0. Method, and the line between two different claims

| | |
|---|---|
| Date | 2026-09-17 |
| Tree | `feat/wave4-track-d`, branched from `feat/wave4-stage-planner` at `89104fd` (tracks A, B and C merged). The working tree carried only the temporary harness described below, and carries none of it now |
| Backend | `src-tauri/target/debug/relay`, built from this tree on 2026-09-17 at 02:19. Startup line: `profile: active 'Default' · lang None · sensitivity 50` |
| Profile | **fresh install.** `HOME=/tmp/relayw4d`, so `db::app_data_dir()` resolved to an empty tree and Relay seeded from scratch |
| Frontend | `npx vite` on **`:5132`**, not `:5032` — see §0b. The embedded HTTP server on `:8032`; the kiosk hub on `:8031` |
| Browsers | gstack's headless driver (`~/.claude/skills/gstack/browse/dist/browse`) for the stage page, and Google Chrome 153 driven over CDP where the driver could not do what was needed — reduced motion, a recorder installed before first paint, and a second page open at the same time |
| Not driven | **the Templates and Library workspaces, the Planner's own cue inspector, and the output page's template editor.** This pass drove the stage page, the output page and Live. Track B's Planner-side control was exercised through its command (`set_plan_timer`) and read back out of SQLite, not clicked |

**Two different claims, and they must not be blurred.**

* **The stage page and the output page were measured against the REAL backend.**
  `http://127.0.0.1:8032/stage.html` and `output.html` are served by Relay itself, they
  join the real kiosk hub on `:8031`, and every timer, fire and panic control below was
  performed by the real Tauri commands against the real registry and the real SQLite
  file. Nothing on these two surfaces is mocked.
* **The console at `:5132` was rendered in a browser, where it has no backend of its
  own.** It was given one, rebuilt from the wave 2 audit's prose (`DESIGN-2026-09-16-WAVE2.md`
  §0): `window.__TAURI_INTERNALS__.invoke` relays each `{cmd, args}` to a local server,
  which hands it to the **real running Tauri webview's own `invoke`**. So the console's
  *data and writes are real* and reach the same database and the same registry. Its
  **events are not relayed** — `plugin:event|listen` registers a handler the page can
  fire by hand. A console reading below is therefore a claim about a real command
  surface behind a browser render, and never a claim about event-driven behaviour.

**The harness is temporary and is in no commit.** It was `src/lib/__auditbridge.js` plus
one line in `src/main.js`, and a Node relay in a scratch directory. Both have been
removed; `git status` on this tree is clean of them.

**Measurement, not eyeballing.** Geometry is `getBoundingClientRect`. *Clipping* is a
`Range` over the text node compared against the nearest ancestor whose computed
`overflow` is not `visible`; where the answer mattered, the **ink** box was computed
separately from canvas `TextMetrics` (`actualBoundingBoxAscent`/`Descent` about the line
box's baseline), because a line box that overflows is not the same fact as a glyph that
is cut. Overlays are `elementFromPoint` / `elementsFromPoint` over a control's centre.

### 0a. THREE THINGS THIS PASS GOT WRONG BEFORE IT GOT THEM RIGHT

Recorded because a pass that corrects itself is worth reading with its own history
attached, and because two of the three would have been filed as defects.

1. **"A bound cue starts a clock during a rehearsal."** It does not. The first run set
   rehearsal by invoking `set_rehearsal` through the bridge, which never reaches the
   console's `$rehearsing` store because **events are not relayed**. Driven through the
   app's own `Rehearse` control, the fire runs and no `start_timer` follows, and the
   registry stays empty. Track B's guard holds. The harness had produced the defect it
   was looking for.
2. **"A bound song cue never starts its clock."** The song cue in the fixture carried
   `sections[].lines`, and `plan.js::slidesOf` reads `sections[].lyrics`, so
   `fire_content` was refused for a missing `text` and the fire threw — which correctly
   starts no clock. The fixture was wrong, not the product.
3. **The verse cut at 1024×768 with two timers.** The `.tval` line box overflows its
   cell by 10px and `.tmr` is `overflow: hidden`, which reads as a cut. The ink box is
   inside the cell with room to spare (`inkCutTop 0`, `inkCutBottom 0`). Not a defect;
   it is why every clipping claim below is an ink measurement.

### 0b. THE DEV PORT WAS MOVED, AND THAT IS PART OF THE EVIDENCE

`:5032` was already held by another worktree's Vite on this machine, bound to `[::1]`.
`localhost` resolves to `::1` first on macOS, so a Tauri webview pointed at the compiled
`devUrl` would have loaded **another tree's console into this tree's backend**. The
`devUrl` in `src-tauri/tauri.conf.json` was therefore moved to `http://127.0.0.1:5132`
for the duration and the binary rebuilt; it is back at `http://localhost:5032` now and
the file is unchanged in this branch. Nothing else about the ports moved: `:8031` and
`:8032` are the real ones throughout.

---

## 1. The programme rail, driven

Every figure in this section is the real rail on `stage.html`, fed by the real
`TimerRegistry` through the real hub.

### 1.1 The warning threshold is the one the frame carries, and it crosses where it should

A `stage`-scoped timer with `warn_ms: 60000`, re-aimed by `adjust_timer` across its own
threshold:

| Remaining | `.tmr.warn` | `.tval` colour | `animation-name` |
|---|---|---|---|
| 9:59 | false | `rgb(232, 234, 238)` | `none` |
| 1:01 | false | `rgb(232, 234, 238)` | `none` |
| 0:58 | **true** | **`rgb(244, 81, 91)`** | `cdwarn` |

A timer with `warn_ms: null` never warned at any figure, which is Track A's stated rule —
a threshold the operator chose, or no warning at all. **§2.1 is what that rule costs.**

### 1.2 Reduced motion is a cut, and the colour survives it

Measured in Chrome launched with `--force-prefers-reduced-motion`, on the same warned row:

```
{"reduced":true,"warn":true,"color":"rgb(244, 81, 91)",
 "anim":"none","shadow":"rgba(244, 81, 91, 0.85) 0px 0px 16px"}
```

The pulse is gone, the colour is unconditional, and the glow replaces the motion. A
viewer who asked for no motion still learns the clock is running out.

### 1.3 Zero, with a message and without

Two timers run to zero in the same rail at 1024×768:

| Label | `.tval` | `.msg` | size | face |
|---|---|---|---|---|
| Notices | `Hand over` | yes | 30px | Inter |
| Offering | `0:00` | no | 64px | IBM Plex Mono |

Neither is an empty box, and the two treatments are visibly different. Worth recording
against Track A's own open question — whether the finished message reads at a distance:
in one rail, at one moment, the operator's words are **30px against the digits' 64px**,
so a finished row is less than half the height of a running one.

### 1.4 Held

| State | `.tval` | `.tstate` | colour | warn |
|---|---|---|---|---|
| running | 7:59 | — | `rgb(232, 234, 238)` | false |
| held | 7:59 | `Held` | `rgb(232, 234, 238)` | false |
| held, 2s later | **7:59** | `Held` | — | false |
| released | 7:57 | — | — | false |

The figure is genuinely frozen, and `Held` is in the page's own ink — not amber, not
cyan, not amethyst. **A held row inside its own warning window is not warned**: at 0:59
against a 60000ms threshold, running gave `warn: true` and red; held gave `warn: false`
and the page's ink.

At a cell right on the floor (132.6px), a 299px label ellipses to 82px and `Held` stays
**fully inside the cell** (`stateRightOfCell: 0`, `stateFullyInside: true`). The ordering
Track A chose — the state word is never the thing that gets cut — holds in a real layout
engine.

### 1.5 The floor

Fourteen `stage` timers at once:

| Viewport | capacity | cells shown | cell width | clock size | label size | count cell | row overflow |
|---|---|---|---|---|---|---|---|
| 1280×800 | 9 | 8 + count | 133.7px | — | — | `+6 more` | 0 |
| 1024×768 | 7 | 6 + count | 138.7px | 34.9px | 14.6px | `+8 more` | 0 |
| 390×844 | 2 | 1 + count | 204.5px | 43.8px | **9px** | `+13 more` | 0 |
| 360×740 | 2 | 1 + count | 186.3px | 40.1px | **9px** | `+13 more` | 0 |

Nothing clipped, no horizontal overflow anywhere, and at least one clock always survives.
Six timers at 1024×768 fit without the floor engaging at all (156.3px cells, 40.7px
digits). The floor works exactly as Track A describes it. **§2.2 is about which cells it
keeps.**

### 1.6 The rail is a zone, and switching it off gives the room back

The panel lists seven zones plus the two figure placements, `Programme` among them at
`aria-pressed="true"`. With `John 3:16` on screen at 1024×768:

| | reading height | rail |
|---|---|---|
| rail on | 523.9px | 70px |
| rail off | **593.8px** | absent |
| rail on again | 523.9px | 70px |

`relay.stage.zones` came back
`{"reading":true,…,"programme":false,"figures":"bottom"}` — the new key persists and
`figures` survives beside it.

### 1.7 A stage tablet that joins mid-service

**On the wire**, joining the real hub as a stage tablet does, with a verse on screen and
timers running:

```
0ms  >> {"kind":"hello","template_id":1}
6ms  << template
6ms  << default_template
6ms  << timer      {"kind":"timer","timers":[…]}
6ms  << content    {"content_kind":"scripture",…}
```

**In the page**, recorded from an init script installed before any document script ran,
sampled every animation frame:

| t | rail | verse | reading top |
|---|---|---|---|
| 4ms | — | — | — |
| 20ms | — | — | 58 |
| 64ms | seven rows | `"For God so loved …` | 59 |

The timers and the reading arrive in the **same frame**, in the order the wire sends
them, and the reading's top moves by one pixel. There is no interval in which a clock
stands where the verse belongs. The `02:35` the recorder saw at t=20ms is the **wall
clock** zone, which is local and needs no socket — it is not a countdown.

After a panic control, with stage timers still running, the hello is `template →
default_template → timer → clear`, and a screen joining then paints nothing. A retained
timer frame cannot resurrect a cleared wall.

---

## 2. Findings

Four, filed as RG-146 to RG-149. Two of them are the same kind of thing: wave 4 built
states on the preacher's rail and wave 4 built the only thing that starts a clock, and
the two do not meet.

### 2.1 RG-146 — the rail's warning and its finished message cannot be reached by any control the product ships

§1.1 and §1.3 are true of a timer that carries a `warn_ms` and a done message. **Nothing
in the shipped product gives a programme timer either.**

`warn_ms` reaches the registry only through `start_timer`. `startTimer` is exported once
(`src/lib/stores/capture.js:1487`) and called from exactly one place in the tree —
`Live.svelte:680`, inside `startCueTimer` — and that call passes `minutes`, `label`,
`scope` and `planItemId` and nothing else. Driven, the call goes out as:

```
start_timer {"minutes":12,"label":"Reading","doneMsg":"","scope":"stage",
             "warnMs":null,"planItemId":2}
```

Read back out of the registry: `"warn_ms": null, "done_msg": ""`. Nothing in `src/`
passes `warnMs` to `startTimer`; nothing in `src-tauri/src/` sets `warn_ms` to anything
but `None` outside tests. So on a shipped copy of Relay a programme row can never turn
red and can never say anything at zero but `0:00` — the two states PR #80 exists to add.
`Settings → General → Countdown warning` cannot help: `programmeWarn` returns false for a
non-finite or non-positive `warn_ms` before `countdownWarning`'s default rule is ever
consulted, which is Track A's deliberate choice and is correct given that the stage page
has no bridge to read the setting through.

### 2.2 RG-147 — nothing stops a programme timer, and the rail keeps the oldest

`TimerRegistry` never reaps; a `Stage` timer lives until `stop`, `stop_scope` or the
process ends. The panic controls take `Both` and deliberately leave `Stage` — measured:
`/api/clear` and `/api/black` left all three clocks on the rail, which is the documented
design. And **no component calls `stopTimer`, `adjustTimer`, `showTimer` or `listTimers`**:
all four are exported from `capture.js` and `grep -rln … src --include='*.svelte'`
returns nothing.

Driven: one walk of a three-cue plan, every cue bound (5, 12 and 2 minutes), run to zero.

```
1024×768:  Opening song = 0:00 | Reading = 0:00 | Notices = 0:00
```

A fourth cue then goes on air with a 25-minute clock:

```
1024×768:  Opening song = 0:00 | Reading = 0:00 | Notices = 0:00 | Sermon = 24:55
390×844:   Opening song = 0:00 | +3 more
```

On the preacher's phone the one clock he can see is a dead one from the beginning of the
service, and the live sermon clock is inside the count. `progCells` keeps
`programme.slice(0, keep)` and the registry's snapshot is ordered by id, which is start
order — so the cells the floor keeps are the **oldest**, and by the middle of a service
the oldest are the ones that have finished. This is rule 35 one rail along: the rail says
it could not show three, and shows the three that no longer matter.

### 2.3 RG-148 — the rail plus both stage panels cuts the verse on the preacher's screen

At 1024×768 with the `Zones` and `Control` panels both open — two taps, both operator-
initiated:

| | reading | verse size | verse text block | ink cut |
|---|---|---|---|---|
| rail on, panels closed | 372.1px | 43.1px | 163.3px | none |
| rail **off**, both panels open | 170.3px | 26px | 98.5px | none |
| rail **on**, both panels open | **74.2px** | 26px | 98.5px | **38.9px off the bottom** |

`.reading` reports `scrollHeight 138` against `clientHeight 74` and the page is
`overflow: hidden` by design, so there is nothing to scroll and nothing says the verse is
incomplete. The verse is already on its 26px floor, so `TemplateRender`'s shrink has
nothing left to give. **The rail is the contributing cause**: with the zone switched off
the same two panels leave the verse whole.

Track A's own read predicted this differently — it expected the two panels to sum to
106dvh and push content off the screen. Measured, they sum to **38.8%** and nothing
leaves the screen (`documentElement.scrollHeight === innerHeight`). The panels are not
too tall; the reading is what pays for them, and wave 4 added a row to the bill.

### 2.4 RG-149 — `.alert.sm` cannot render

`ALERT_STEPS` falls through to `sm` only above 150 characters
(`Stage.svelte:502-507`), and `send_stage_alert` caps the text at **140**
(`main.rs:6178`). Driven with a 166-character message: the page received 140 characters
and rendered `.alert.md`. The 3cqw step is unreachable through the only path that sets
`alert`. Measured smallest reachable step: **16.4px** at 390×844, 43.0px at 1024×768 —
not the 11.7px Track A's deferred item 5 describes.

---

## 3. Track B, driven

### 3.1 A cue can carry a clock, and it is not `duration_sec`

Written through `set_plan_timer` and read back out of the real SQLite file:

```
2 scripture Reading       timer_minutes= 12    duration_sec= 0
3 song      Opening song  timer_minutes= 5     duration_sec= 0
4 announce  Notices       timer_minutes= 2     duration_sec= 0
(before binding)          timer_minutes= None  duration_sec= 0
```

An unbound cue reads `None` and not `0`, which is the distinction the column exists for.

### 3.2 A bound cue going on air

Clicking the bound scripture cue in Live's slide grid, against the real backend:

```
… manual_fire
   start_timer {"minutes":12,"label":"Reading","doneMsg":"","scope":"stage",
                "warnMs":null,"planItemId":2}
   set_stage_next …
```

One `start_timer`, `scope: "stage"` and never `both`, carrying the cue's id. The registry
gained `id 1, label "Reading", plan_item_id 2`, and the preacher's rail read
`Reading/11:56` about two seconds later. Walking the song's three sections started **one**
clock, not three.

### 3.3 A rehearsal starts nothing

Through the app's own `Rehearse` control (see §0a.1 for the run that got this wrong):
banner reads `Rehearsal`, the cue fires (`manual_fire`), **no `start_timer` follows**, the
registry is empty, and the stage rail is absent. `get_rehearsal` confirms `true`
throughout.

### 3.4 The slide sizer

Four steps, driven at 1024×768 and at **960×640, the app's own `minWidth`/`minHeight`**:

| step | readout | grid track @1024 | grid track @960 | `−` disabled | `+` disabled |
|---|---|---|---|---|---|
| S | `− S +` | 189.2px | 176.4px | **yes** | no |
| M | `− M +` | 238.5px | 222.5px | no | no |
| L | `− L +` | 320.7px | 299.3px | no | no |
| XL | `− XL +` | 485.0px | 453.0px | no | **yes** |

`.sg-thumb` carries no inline width at any step and keeps `container-type: inline-size`,
so the grid track moved and the container-query root did not. The session stored `0, 1,
2, 3` — an index, never a label.

**The rail that once clipped `Compact` to `Compa`**: the control is **68px wide** at every
step, in a 930px pane head at 960 and a 994px one at 1024, with
`scrollWidth - clientWidth === 0` on both the control and the head at every step and both
widths. It does not clip. Track B's Step 4 is answered.

**Rule 44 over the dock**: with the launch ladder finished and the Recover gate answered,
`elementFromPoint` over the centre of `Clear screens` (179.3 × 41.7px at 960×640) returns
the button itself at both ends of the sizer — `r-cbtn[z=auto]` on top of the stack. The
sizer paints over nothing.

---

## 4. Track A's deferred legibility list, measured

PR #80 deferred nine items to this pass. Every figure below is the real page with a verse,
a stage note, an up-next and four programme clocks all live at once — the state the list
asks about.

| # | Track A's read | Measured | Verdict |
|---|---|---|---|
| 1 | header never got the label pass; `.status` at 11px | `.brand` 16px, `.status` 11px at **every** viewport, against a verse at 43.1px | **confirmed** |
| 2 | the two header controls are under 44px on a phone | `.ctl-toggle` is **30px** tall, 11px text, at 1024, 1280 and 390 alike; `.zonebtn` keeps its 44px | **confirmed** |
| 3 | rows sum to 77%, leaving the reading 23% (177px) | at 1024×768 with every zone live: figrow 115.2 + progrow 96.1 + noterow 50.3 + next 75.3, reading **372.1px = 48.5%**, verse 43.1px | **not reproduced** — the rows do not take their ceilings |
| 4 | `9cqh` in an `inline-size` container resolves against the viewport | `.progrow` is `container-type: inline-size`; at 1280×800 the clamp caps at **64px** before 9cqh's 72px | **confirmed, and harmless today** |
| 5 | `.alert.sm` is 11.7px on a phone | `.alert.sm` **cannot render** — RG-149 | **corrected** |
| 6 | region labels floor at 9px on a phone | `.figk`, `.tlabel` and `.note-lbl` all **9px** at 390×844; 14.6px at 1024, 15.2px at 1280 | **confirmed** |
| 7 | the up-next is a hairline on a platform monitor | `.next-ref` 13.8px, `.next-text` 18.7px at 1024×768 | **confirmed as a figure**; the ratio to the verse is 2.3×, not the ten stated |
| 8 | two panels open exceed the screen (106dvh) | they sum to **38.8%**; nothing leaves the screen. The reading pays instead — RG-148 | **corrected, and worse than the read** |
| 9 | the `Held` word and a long label share a 130px cell | at 132.6px: label ellipses 299px → 82px, `Held` fully inside, `thead` overflow 0 | **confirmed, and the ordering holds** |

---

## 5. The congregation's two rules, re-checked because this wave touched the hub

### 5.1 Rule 42 — the first verse of the service on a freshly opened output page

`output.html` at 1920×1080, opened with nothing on it, then the first fire:

```
at mount:   document.fonts.status === "loaded"    ← the trap condition
after fire: font-size 103.26px, family Fraunces, document.fonts.check → true
            text block 535.9px inside a 583.2px .ltext box
            23.0px clear at the top, 24.3px clear at the bottom
```

The deferred re-fit is doing its job. Nothing is sliced.

### 5.2 Rule 43 — an OBS source restarting mid-service

Recorded from before first paint: nothing at 6ms, the retained verse painted at **23ms**,
and no blank interval after it. After a panic control, a page joining paints nothing and
keeps painting nothing — a cleared wall stays cleared, with stage timers live on the same
hub.

---

## 6. One thing observed once and not reproduced

On the first walk of the plan, `start_timer` was called **twice** for one song cue. The
backend absorbed it (`start_timer` stops the cue's previous clock when `plan_item_id`
matches), so the registry held one timer and the visible effect would have been a clock
restarting rather than two clocks appearing. It did not recur in **six** subsequent runs —
deliberate gaps of 200, 400, 800 and 1200ms between two slides of the same cue, and two
further full walks — every one of which produced exactly one start per cue. The run that
produced it was the first after the plan fixture was rebuilt underneath the open grid.
Recorded rather than filed, because a defect nobody can reproduce is a report, not a
finding.

---

## 7. What this pass did not drive, and what it therefore does not claim

* **The Planner's cue inspector was not clicked.** Track B's control was exercised
  through `set_plan_timer` and verified in SQLite. That the control writes that command
  and nothing from `TAKES_A_SCREEN` is Track B's own test's claim, not this pass's.
* **The Templates and Library workspaces were not driven at all.**
* **Nothing here is a claim about event-driven console behaviour** — §0 says why.
* **No word error rate, no detection, no threshold, no latency.** This pass put no audio
  through Relay.
* **It does not move the release decision.** `docs/RELAY_V1_AUDIT.md` owns that.

---
