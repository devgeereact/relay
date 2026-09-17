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
