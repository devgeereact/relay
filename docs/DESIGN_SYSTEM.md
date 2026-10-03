# Relay — Design System

The visual and interaction language: tokens, type, the load-bearing colour meanings, and the
component vocabulary. For the *why* behind any rule here follow the link into
[DECISIONS.md](DECISIONS.md); for how the pieces fit see [ARCHITECTURE.md](ARCHITECTURE.md);
for the entities being rendered see [DATA_MODEL.md](DATA_MODEL.md).

**The source of truth is the shipped stylesheet, not this page.** It is two files:
[`src/tokens.css`](../src/tokens.css) carries the palette, the type ramp and the spacing scales,
and [`src/app.css`](../src/app.css) imports it and adds the console's own rules. Both carry the
reasoning inline, at each token. This document is the map and the rules a reader needs *before*
opening them. If the two disagree, the stylesheet is right and this page is a bug.

---

## 0. The one thing to understand first

Relay renders in a dark booth, and it is looked at by two different audiences with opposite
needs:

- **The operator**, in the console, who must be able to tell at a glance what is on a wall.
- **The congregation**, looking at a projector, who must be able to read scripture from the back
  of a room.

Those are different design problems and Relay keeps them in different systems:

| | Console chrome | Output surfaces |
|---|---|---|
| Styled by | `src/app.css` — the console's global stylesheet, which imports `src/tokens.css` | **Templates**, resolved by `TemplateRender.svelte`. There is no separate theme layer any more: a theme had no field a template does not already have, so the two were folded into one (DECISIONS §87) |
| Who changes it | Only a developer | The operator, in the app, per screen |
| Units | `px` | **`cqw`** — so a template looks identical at any output size |
| Background | Always dark | Whatever the template says, **including transparent** (so it keys out in OBS) |

Never style an output surface from `app.css`, and never put a console token into a template.
**Both halves of that sentence used to be unenforceable, and both are enforced now** (wave 5,
Track E). `output.js` and `stage.js` imported `app.css` in full, and Svelte does not scope a
global stylesheet, so every unscoped console rule in it was live on `output.html` and
`stage.html` — a class-name collision away from painting on a wall. They now import
`src/tokens.css`, which may declare custom properties and nothing else, so the palette is still
shared and no rule can cross. And a seeded `style_json` stored `"font":"var(--f-serif)"`, a
token declared in the console's chrome: `--f-display` was re-aliased once, from Space Grotesk to
Inter, and silently changed the typeface of every template naming it. Templates name real
families; `ensure_templates_name_real_families` carries that to an install that already exists.
`src/lib/seal.test.js` fails on the first rule added to the shared sheet and on the first
app-chrome token written back into template data.

---

## 1. Colour — the part that is safety-critical

**Four colours in this product carry a promise. Using one for decoration is a bug, not a taste
question.** They are enforced in code and pinned by tests, because a colour that lies to an
operator during a live service is the same class of failure as a control that lies
(DECISIONS §20, §21).

| Colour | Token | Means, and *only* means |
|---|---|---|
| 🟠 **Amber** `#ffa31a` | `--v-amber` | **ON AIR.** The congregation is looking at this right now. |
| 🟣 **Amethyst** `#a96bf5` | `--v-amethyst` | **Nothing here reaches a congregation** — rehearsal, safe mode, the launch sequence. See §1.1 and DECISIONS §93. |
| 🔵 **Cyan** `#4cc9f0` | `--v-cyan` | **A guess.** A paraphrase / semantic match. Never a heard reference. |
| ⚫ **Grey** `#8a929e` | `--v-grey` | **CUED** — this is where `→` resumes, and it is **not** on screen. |
| 🔷 **Steel** `#5b9cf8` | `--v-sel` | **The thing you are working on.** Selection, focus, tabs, keys. Carries no promise about a screen. |
| 🟡 **Caution** `#c9a24a` | `--v-caution` | **A warning that is not a failure**, and carries **no promise about a screen**: a screen taken down, a role unset, a font that did not load, a service lock holding something back. Never for emphasis (that is steel) and never for a failure (that is rose). DECISIONS §111. |

The four promises are unchanged. The rebrand retuned their hexes and gave
interactive chrome a colour of its own; it did not move a meaning.

The rules that follow from that:

- **Amber is never used for "selected", "active", "primary" or "success".** It is the tally
  light. A colour that is always lit cannot also be a warning.
- **A cued position is grey, never amber.** `liveCue` is `{ cueId, slide, onAir }`, and position
  and on-air-ness are separate facts — panic keys clear only `onAir`. A cue that is where `→`
  resumes but is not on screen reads **CUED**, in grey (CLAUDE.md, frontend shape).
- **Amethyst's promise is broader than the word "rehearsal" and narrower than "anything purple".**
  `app.css` used to say "the rehearsal colour, and nothing else uses it", in a file spending
  amethyst on fourteen other surfaces. DECISIONS §93 settles it by reading those fourteen: safe
  mode ("outputs disabled") and the launch sequence (no console, no output window, nothing on any
  wall) are the same fact said where "rehearsal" does not fit, and a boot ladder and a rehearsal
  badge can never be on screen together. **Caution was the one question §93 left open**, and
  it was paid on 2026-09-21 (DECISIONS §111): the palette now publishes `--v-caution`, the five
  surfaces that borrowed amethyst for a warning and the three that borrowed amber wear it, and
  `colourlaw.test.js` enumerates every caution surface with what it warns about.
- **A claim card is steel, not amber, until the wall says otherwise** (RG-192, 2026-09-21). A heard suggestion wore the tally light while the Program pane read CLEAR; a heard claim is *the thing you are working on* and gets `--v-sel` on its rule, its chip and its confidence bar. `uncertain_book` gets a dashed rule so it never reads as a paraphrase.
- **A paraphrase is cyan and shows no percentage at all.** A TF-IDF cosine is not a probability,
  and a number that lies is worse than no number (DECISIONS §21). It is never amethyst, because
  amethyst already promises "rehearsal — this cannot reach the congregation", and a colour
  carrying a promise cannot be borrowed for a hunch.
- **`detect.js` owns the mapping** from a detection method to the language the operator reads
  (`methodKey`); `Live.svelte` renders it. There are three distinguishable methods —
  `direct` (heard), `semantic` (a guess), `uncertain_book` (chapter and verse heard, the book
  not) — and they must stay visually distinguishable.

### 1.1 Why the interactive accent is steel blue, not amber and no longer amethyst

Every piece of interactive chrome — the selected nav item, a focus ring, a switch, a hover, a
primary button — points at `--v-accent`, which is now an alias of `--v-sel`, steel blue.

It was amber first, and that was the bug: spending the tally colour on "this tab is selected"
and on twenty ordinary Save buttons meant the loudest colour in the product was lit
permanently, everywhere, whether or not anything was on a wall.

It was then amethyst, which was better and still wrong. Amethyst means **rehearsal** — the one
state in which nothing can reach a congregation — and a colour that says that cannot also be
what every hover, switch and Save button wears. Telling a rehearsal from a live service is a
safety distinction, so the rebrand gave "the thing you are working on" a colour of its own and
handed the promise colours back their single meanings.

Amber survives only as `.r-badge.amber`, `.r-btn.amber` and `.r-stat.amber` — the on-air cases,
named explicitly at the call site so reaching for one is a **decision** rather than a default.

Two steels exist, for the same reason two amethysts used to:

- `--v-sel` `#5b9cf8` — borders, dots, and text **on** a dark surface (the thing being read).
- `--v-sel-fill` `#2e6fd4` — a fill **behind** white text. White on `#2e6fd4` is 4.84:1, above
  WCAG AA for button text; on the lighter `#5b9cf8` it is not.

**The four controls.** The two most consequential buttons in the room used to look alike, so
each now owns a colour nothing else wears: **End service** amber (it owns the on-air session),
**Clear screens** red, **Blackout** black with a light hairline, **Rehearse** amethyst.
Blackout was grey until this rebrand, which meant the control that takes the wall to black
shared a colour with a cue position.

### 1.2 The neutral ramp

A 950→500 greyscale ramp. `--bg` is what the body **and the output-window canvas** paint, so it
is the deepest step.

A 950→500 ramp with a slight blue cast, so a grey surface reads as equipment rather than as
paper. `--v-bg` is what the body **and the output-window canvas** paint, so it is the deepest step.

| Token | Hex | Role |
|---|---|---|
| `--v-void` | `#131418` | Shell, main, output canvas |
| `--v-bg` | `#1a1c21` | Sidebar, dock, topbar, input fields |
| `--v-surf` | `#212429` | Default card |
| `--v-surf2` | `#292d34` | Raised / selected card |
| `--v-surf3` | `#343942` | Hover, lightest chrome |
| `--v-500` | `#3a404a` | Hairline emphasis (a visible edge on a card) |
| `--v-rule` | `#0c0d10` | The seam between two regions — **darker** than either |
| `--v-line` / `--v-line2` | `rgba(255,255,255,.075)` / `.13` | Borders |

A control room divides by shadow, not by a light rule: `--v-rule` is darker than anything it
separates, because a pale divider on a dark desk is the first thing the eye goes to.

Text: `--v-txt` `#e8eaee` · `--v-dim` `#a9b0bc` · `--v-faint` `#8c94a1` · `--v-disabled` `#5a6270`.

> **`--v-faint` is `#8c94a1`, not the prototype's `#7e8695`, and the difference is load-bearing.**
> At `#7e8695` it measures **4.25:1 on `--v-surf`** and **3.77:1 on `--v-surf2`** — below WCAG AA
> on two of the four surfaces muted text sits on. `#8c94a1` is 4.52:1 at worst and looks
> identical. The prototype is the reference for this rebrand; the repository's tests win where
> the two disagree, and this is the place they did.
> **Do not round it back.** `tokencontrast.test.js` measures every text token against
> every surface it is placed on and will fail (RG-74); the comment beside the token in
> `app.css` carries the full matrix. Muted is deliberately kept off `--v-surf3`
> (3.76:1) — that exclusion is asserted, not assumed.

**Every text token passes WCAG AA on every surface it sits on.** `--v-faint` was `#5f6470`
(2.27:1, a failure everywhere); it is now 4.52:1 at worst. Do not darken a text token without
re-checking it against `--v-void`, `--v-surf` **and** `--v-surf2`.

> **A `--v-*` this file never defines is the quietest failure the palette has.** `var(--v-nope)`
> with no fallback is invalid-at-computed-value-time: the declaration is not dropped, it becomes
> `unset` — **`inherit`** for an inherited property, **`initial`** for one that is not. Nothing
> throws, nothing logs, the build stays green, the class is spelled right, and the element
> renders in a colour nobody chose. Two were live and neither was visible to
> `tokencontrast.test.js`, which checks the palette a developer reaches for and says so:
> `History.svelte` armed a two-step **Delete** with `color:var(--v-ink)` and therefore inherited
> `--v-txt` onto the red fill at **2.82:1** (`--v-inverse`, which it meant, is 5.41:1), and
> `Live.svelte` hovered a button with `border-color:var(--v-txt-dim)`, fell back to
> `currentColor`, and gave the control its own comment calls *deliberately quiet* the loudest
> border in the pane. `tokendefs.test.js` now resolves every `var(--v-…)` under `src/` against
> this palette.

### 1.3 The remaining semantic colours

| Token | Hex | Means |
|---|---|---|
| `--v-emerald` | `#3fcf6a` | Confirmed / success / connected |
| `--v-red` (`--v-rose`) | `#f4515b` | Failure, destructive, the panic banner, the stage alert |

Every semantic colour carries a `-soft` fill and a `-line` border alongside it
(`--v-amber-soft`, `--v-amber-line`, and so on). **Use them.** Thirty-two rules had hand-written
`rgba()` copies of a law colour; when a hex moved, each was a chip whose fill and border were
quietly two different colours.

Badge classes exist for each (`.r-badge.{amber,cyan,rose,amethyst,grey,green}`), each a soft
fill + a 32%-alpha border + the colour as text. `.r-badge.pulse` adds a glow animation — reserve
it for live states.

---

## 2. Typography

**One UI family: Inter.** `--f-display` (formerly Space Grotesk) and `--f-head` (formerly
Playfair) are *aliased* to Inter rather than deleted, because `templates.js` and a couple of
views still name them.

**Every figure is mono, and that is a layout rule rather than a taste one.** A clock, a
confidence, a latency and a verse number all change while someone is watching them; in a
proportional face each change reflows the row beside it. IBM Plex Mono's digits are tabular, so
a number that changes moves nothing. JetBrains Mono stays in the bundle because **templates**
still offer it by name — that is a choice an operator saved, not chrome.

| Purpose | Token | Family |
|---|---|---|
| UI / body | `--f-body`, `--f-head` | Inter |
| Scripture (default template face) | `--f-serif` | Fraunces |
| **Every figure** — clocks, confidences, latencies, verse numbers | `--f-mono` | IBM Plex Mono |

**Fonts are self-hosted via `@fontsource`, imported once in [`src/lib/fonts.js`](../src/lib/fonts.js)
and shared by every entry point (console + output windows). Never a CDN link, anywhere** —
offline-first is non-negotiable, and a `fonts.googleapis.com` link means a church with no
internet gets a fallback face on its projector.

### The scale

The register is a control room's, not a web page's: **11–12px is the body size**, and the
document scale steps down from it.

| Step | Size / line-height | Weight | Tracking |
|---|---|---|---|
| Display | 30 / 34 | Bold | `-0.022em` |
| Page title | 21 / 26 | Bold | `-0.022em` |
| H1 | 17 / 22 | Semi Bold | `-0.022em` |
| H2 | 14 / 19 | Semi Bold | `-0.01em` |
| H3 | 12.5 / 17 | Medium | 0 |
| Body 1 | 12 / 17 | Regular | 0 |
| Body 2 | 11.5 / 16 | Regular | 0 |
| Label | 11 / 15 | Medium | `+0.02em` |
| Caption | 9.5 / 13 | Semi Bold | `+0.09em`, uppercase |
| Mono | 11 / 15 | Mono | 0 |

Tokens: `--v-fs-*` / `--v-lh-*`, tracking `--v-tr-tight` `--v-tr-h2` `--v-tr-wide` `--v-tr-caps`.
**The old scale was 14px body**, and the two extra pixels cost a row of the slide grid — on a
1366×768 booth laptop they cost the status bar itself. The brand lockup is the exception — 19px,
weight 700, `letter-spacing: .18em`.

---

## 3. Space, radius, elevation

- **Spacing** — `--v-sp-xs` 4 · `sm` 8 · `md` 16 · `lg` 24 · `xl` 32 · `2xl` 48 · `3xl` 64 ·
  `4xl` 96 · `5xl` 128. Use the token; do not invent a 10 or an 18.
- **Radius** — `--v-r-sm` 3 · `md` 3 · `lg` 5 · `xl` 5 · `2xl` 6. **No pills.** A pill in a
  control room reads as a toy, so `--v-r-round` (99px) survives for the three shapes that are
  genuinely round — a slider thumb, a status dot, a switch — and for nothing else. Every literal
  radius in `app.css` reads a token; if you are typing `border-radius: 10px`, the answer is a
  token you have not found yet.
- **Shadow** — `--v-shadow-sm/md/lg`, all pure black at 45–60% alpha. On a `#131418` ground,
  elevation reads through *surface step*, not through shadow; shadows are a secondary cue.

---

## 4. Layout and the shell

The console **fits the viewport and never scrolls the body** — the shell owns the height and
panels scroll internally (`html,body{height:100%}`, `body{overflow:hidden}`). Sidebar is a fixed
`238px`.

**The panic bar is the one piece of chrome allowed to move the whole app.** It is
`position: fixed` at the top; the shell offsets by its *measured* height (`--panic-h`, set by
`App.svelte`) rather than sitting underneath it. It used to cover the first ~56px — the brand,
and part of the top bar including the On Air badge and the name of what is on the wall, which is
the single thing an operator needs most at the exact moment that bar appears.

**It has no transition, deliberately.** Sliding the console for 200ms while someone is trying to
read *"the screens may still be live"* is motion for its own sake.

---

## 5. Motion and accessibility

- **Focus is always visible.** `outline: 2px solid var(--v-sel)` with `2px` offset, on every
  interactive class (`.r-btn`, `.r-iconbtn`, `.nav-item`, `.r-input`, `.r-select`, `.r-switch`,
  `.r-cbtn`, `.r-pill`, `.r-focus`). Never remove an outline without replacing it with an equally
  visible one.
- **One slider, one switch, one colour well.** `accent-color` was not enough — it let every
  platform draw its own idea of a slider, so the sensitivity dial an operator learned in Settings
  was a different instrument on the run surface. `app.css` draws a 3px track filled to `--rp`
  with a 13px thumb; `src/lib/rangefill.js` keeps `--rp` current on render, on input **and** on
  a rebuilt panel (setting `.value` in code fires no `input` event, which is the case a listener
  alone misses). Switches and colour wells are both 38×21.
  - **`appearance:none` does not take back the browser's margin.** Chromium's UA sheet gives
    `input[type=range]` a `margin:2px` that survives it, so for as long as the block did not say
    `margin:0` every slider in the product sat two pixels inside the edge its neighbours sat on
    — measured in the Theme editor at x=1251 against x=1253 for the select and the three colour
    wells beside it, and 871 against 873 at a 900px viewport. Two pixels cannot be seen by
    reading the stylesheet. Pinned by `rangefill.test.js`.
  - **What "lines up on one right edge" is, and is not, a promise about.** The token layer sizes
    the controls and zeroes their margins; a row lines up when the control is the last thing in
    it. Measured after the margin fix: the Theme editor's column puts sliders, colour wells,
    inputs and selects on ONE edge at both 1280 and 900. The Template inspector's rows are flush
    with each other but their controls are not, because `.te-rangerow` and `.te-swatch` place a
    value readout after the control — deliberate, per §11's "a name and a value". The one that
    drifted was `.te-swatch`: the 38px well is pinned to the LEFT of that pair, so with nothing
    flexible between them the pair packed left and the row's CONTENT ended 158px short of the
    column at 1280 and 730px short at 900. (The box always spanned the column — what was short
    was everything in it, which is why it read as a ragged column rather than as a broken
    control.) **Fixed in `TemplateEditor.svelte` by W4**: `justify-content: space-between` puts
    the hex readout on the row's right edge, the same shape `.te-rangerow` and `.te-swrow`
    already keep. Re-measured in the browser at 1280: the swatch row's content reaches 1251,
    level with the range rows — 0px short, against 158px before.
- **Every modal surface traps focus and restores it on close** (`src/lib/focus.js`,
  `use:trapFocus`). This line used to say *five*; it is ten now and will be wrong again, so
  count rather than trust it: `grep -rl trapFocus src | grep -c svelte`. Note that grepping for
  `role="dialog"` instead gives a different and misleading answer — `Announcements.svelte`
  carries the string only inside a comment explaining why that panel is deliberately **not** a
  dialog, and `CrashReportRecovery.svelte` is an `alertdialog`.
  Restore is the half everyone forgets.
- **`Esc` must not clear the screens while a dialog is open.** `shortcuts.js` checks for a
  mounted `[role="dialog"]`. Dismissing a help overlay is not a live action (CLAUDE.md §16).
- **Motion is opt-in.** Decorative animation sits inside
  `@media (prefers-reduced-motion: no-preference)`, and the spinner explicitly stops under
  `reduce`. A new animation goes in the same guard.
- **On the WALL, the default is a cut.** A template may choose one of seven
  transitions — Cut · Crossfade · Dissolve · Fade through black · Push left · Slide up ·
  Materialise — from the one register in [`src/lib/transitions.js`](../src/lib/transitions.js).
  Three rules hold it: only `opacity`, `transform` and `filter` animate (none of them moves
  `scrollHeight`, so the measured auto-fit reads the same box mid-transition as it does at rest);
  every mode ends exactly settled, or the verse stays slightly wrong for as long as it is up;
  and **reduced motion is a cut**, not a faster animation. An unknown mode is a cut too — an
  imported template must not be able to stop a verse rendering (DECISIONS §71).
- **The AI announces itself.** The suggestion feed, the transport, and errors all reach an
  `aria-live` region in `App.svelte` — the product's whole reason to exist used to arrive in
  total silence.
- **Heading structure is real**, starting at a single `<h1>`. Do not use a heading level for its
  size; use the type scale.
- **A list row is a name and a VALUE — never an em dash standing in for one.**
  [`settingValue`](../src/lib/settingvalue.js) turns an absent value into words, and the wording
  belongs to the call site, because only it knows what absence means there: "not on a network" is
  a different fact from "could not be read", and both are different from "checking…". An em dash
  was the same glyph for all three — rule 35's defect (a status that reads the same when broken as
  when fine is not a status), and the same shape as RG-83. Pinned by `surface.test.js` R3-13.
  The one place a dash survives is the latency percentile row, where it means *this stage was never
  reached* and a word in a five-value numeric row would read worse (CLAUDE.md rule 31).

---

## 6. The legacy palette — why the dead CSS is still there

`src/tokens.css` opens with a legacy `:root` block. Every legacy colour name is now an **alias**
of the design-system token it maps to (`--amber` → `--v-amber`, and so on), so anything still on an old
name is on-brand by construction and each hex lives in exactly one place.

89 orphaned rules were deleted by checking every class name against every class a component
actually renders. **~150 lines remain, deliberately.** The reason is mechanical:
**Svelte does not scope a global stylesheet**, and the survivors use generic class names
(`.tab`, `.dot`, `.live`, `.chip`) that live components still carry. Deleting a rule whose class
is still on an element silently restyles the app, and verifying that needs eyes on a running
window — which the build machine cannot produce.

So the gun is unloaded rather than removed: the contrast failure is fixed, and the rules stay
until someone can look at a running app. Tracked in [KNOWN_ISSUES.md](KNOWN_ISSUES.md) §4.

**What wave 5 changed is the blast radius, not the rules.** Six of the survivors —
`.prev-main .verse`, `.prev-stage .verse`, `.prev-stream .lower-third`, `.prev-lobby .verse`,
`.tmpl-row.active` and `.toggle.on` — were reaching `output.html` and `stage.html` as well as the
console, because both entry points imported this file. They are still here, still un-deleted, and
they can now only restyle the console: the congregation-facing pages import `src/tokens.css`
instead, and the built bundles show it (`dist/output.html` and `dist/stage.html` reference the
token sheet, not the console's). The judgement that deleting them needs eyes on a running app is
unchanged.

---

## 7. The template is the whole style — themes were folded into it

**A template is the only style layer** (DECISIONS §87, completing §27). A theme used to sit
beneath one and fill the keys it left unset:

```js
{ ...theme.style, ...template.style }   // template wins, key by key
```

Every field on the left of that merge was a key the template already had. There was no themes
table and no Rust struct — a theme was a bag of defaults for the same flat `style` keys
`TemplateRender` reads, whitelisted to a subset of them, and nine of its fourteen controls moved
nothing at all on a layered template. So it could only ever say less than the template above it,
and it could disagree with it about what a screen wears. It is gone: `ensure_themes_are_inlined`
writes each pinned theme's style into the template's own, under the same precedence the renderer
applied, so **no look changed**.

**What survives is the half that was never about themes.** A layer's colour, fill or font may be
a TOKEN (`theme:accent`) rather than a literal, so a stage, confidence or countdown starter
follows whatever template it is dropped into. [`src/lib/styletokens.js`](../src/lib/styletokens.js)
resolves it against that template's own style, which is what the merge already produced once the
theme was out of it. The token keeps its spelling: it is written into every saved layer in every
install, and renaming it would need a migration to buy a nicer word.

**One property, one home** (`src/lib/templatemodel.js`, docs/REBRAND.md §3.1). A template stores
only what it has changed and the model fills the rest:

- `migrateStyle` writes the legacy WHOLE-TEMPLATE keys (`font`, `textShadow`) onto the elements
  that use them and then **deletes** them. `style.font` was a second home for `verseFont` and
  `refFont`, reached through a fallback chain — so the editor could show one and save the other.
  Nothing crashed; the preview and the wall simply stopped agreeing.
- `resolveStyle` fills every per-element default in one place, so "unset" looks the same on every
  surface. It deliberately answers for neither `background` nor alignment: an unset background is
  **transparent**, which is what keys a lower third over a camera, and alignment falls back to
  `layout.align` first. An absence that means something cannot be defaulted away.
- `slideBG` turns a colour and a treatment into CSS — solid, vertical fade, centre glow,
  diagonal, vignette — and returns **null** when a template names no background. A background
  written as raw CSS is passed through untouched; wrapping a pasted gradient in another gradient
  produces an invalid value that paints nothing.
- The migration runs on the **doors**, not only in the renderer: `loadTemplates` (the database),
  `parseImportedTemplate` (a file), and `resolveStyle` itself. Migrating at the renderer alone
  would keep the wall correct while the legacy key sat in the database for the next reader that
  does not resolve.
- **A deletion is only safe if every reader moved with it**, and two did not. `regionsToLayers`
  (which `TemplateGallery` runs on mount and **saves**) and the `theme:font` token resolver (now
  `styletokens.js`) both read `style.font` — real in the shape they were written against, absent in the shape that
  now reaches them. Every seeded template carries a `font`, so the first visit to the Templates
  tab after an upgrade would have re-typefaced the whole shelf to serif, silently, once, for good.
  Both now read the model (`migrateStyle` / `resolveStyle`), which is idempotent, so they are
  correct whether the style handed to them is old or new. **Read the model; never read a key the
  model owns.**

**Auto-fit is measured, and now seeded** (§3.4). The DOM loop still decides — it reads
`scrollHeight` against `clientHeight` and shrinks until the box holds the text — but it starts
from `fitScale`, which predicts the answer from the text, the face's own advance (mono 0.62,
serif 0.49, sans 0.52) and the box's **real** aspect. Each measured step forces a synchronous
reflow on the page that is on the wall, so a long passage used to cost twenty of them. If the
estimate is pessimistic the loop grows the text back while it genuinely fits, so a seeded fit
lands where the plain loop would have. **jsdom has no layout**, so the measured half cannot be
tested; `templatemodel.test.js` holds the arithmetic, and the floor it reports is rule 37's.

The three box arguments are **one description and must not be mixed**: `aspect` is the
CONTAINER's aspect (what `cqw` is a share of), and `widthPct`/`heightPct` are the text box's
share of that container. `.stage` carries `container-type`; the box measured is `.content`,
about three quarters of it — so passing the BOX's own aspect with the shares left at 100
describes a container the size of the box and over-states the room by that ratio.

**Both loops stop on the answer, not on a round count.** They shared a `guard < 40`, and 0.95^40
is 0.1285: a box needing less than that got the loop's last guess and kept it, still overflowing,
inside an `overflow: hidden` box — rule 42's sliced verse reached by running out of rounds. It
takes only one short line at a large designed size in a shallow box, which is an ordinary band or
stage zone, not a pathological template. `keepShrinking` / `FIT_STEP` / `FIT_MIN_SCALE` are one
home for the curve and its floor. The floor is on the **arithmetic**; rule 37's 45% is a separate
line that **reports** rather than stops.

**`TemplateRender.svelte` is the ONE renderer** — the fullscreen output *and* the Templates
editor preview both use it, so the editor is WYSIWYG by construction. Stage displays and
confidence monitors are **render profiles of that same engine** (starters in `layers.js`), not a
parallel system: they add monitor-only fields (`next`, `note`, `elapsed`) that ride to output but
that no congregation template renders.

### Rules for anyone touching output rendering

1. **Sizes are `cqw`.** A template must look identical at 1280×720 and at 3840×2160. A **slide
   region** (a composite's word half) is its own container, so `cqw` inside it is a share of the
   REGION's width — which is what lets one template render correctly at screen size and at region
   size with no second rule. A region renders only at depth 0: a composite may not be another
   composite's fill (DECISIONS §74).
2. **The output page background is transparent** so a Transparent-background template keys out
   for OBS/ATEM. Do not paint a fallback colour on it.
3. **No `if channel_type == …` in rendering logic.** A per-channel difference is a template
   configuration problem (CLAUDE.md, non-negotiables).
4. **A content-look default rides as an ID only** — it never serializes or broadcasts its
   template JSON. This is a hard performance rule as well as an architectural one: a default
   template carrying an embedded `data:` image can be megabytes (one was 13 MB), and
   broadcasting that on every fire made verses take seconds. Only a **pinned** cue template
   ships its JSON.
5. **Imported templates are sanitised at the boundary** so a shared template cannot beacon out or
   blank the wall offline.

---

## 8. Component vocabulary

Prefixed `r-` in `app.css`. Reach for one of these before writing a new class.

| Class | What it is |
|---|---|
| `.r-btn` (+ `.amber`) | Button. The `.amber` variant is the on-air case and is named at the call site. |
| `.r-iconbtn` (+ `.sm`) | Square icon-only button, 26px with a 22px step. |
| `.r-menu`, `.r-menuitem` | The popover shell and one row of it |
| `.r-well` | A field well: a bordered box holding an input |
| `.r-badge` (+ colour) | Pill status chip; `.bd` is its 6px dot, `.pulse` adds a glow |
| `.r-input`, `.r-select`, `.r-switch` | Form controls |
| `.r-stat` (+ `.amber`) | A number-plus-label readout |
| `.r-scroll`, `.mainscroll` | Internally-scrolling panel (slim dark scrollbars) |
| `.r-focus` | Opt into the standard focus ring on a custom element |

**The `src/lib/ui/` kit is how those classes are asked for**, and reaching for a component rather
than remembering a class name is the point: 41% of the `<button>` elements in this tree touch no
shared class at all, and a class is opt-in in a way a component is not.

| Component | What it is for |
|---|---|
| `Button`, `IconButton` | The two shapes on the ladder, with `disabledReason` |
| `Menu`, `MenuItem` | The popover, and **the one place rule 44's `Esc` contract lives** |
| `Field` | A `.r-well` around a caller's own `<input>` |
| `Toolbar` | A row that fixes one control height so a mixed row cannot step |
| `ListState` | *empty ≠ loading ≠ error*, with the precedence fixed once |

**A disabled control owes the operator a reason.** `disabledReason` renders `title` **and**
`aria-describedby`, because neither channel reaches everybody: `title` is invisible to a keyboard
or screen-reader operator, and `aria-describedby` is invisible to a mouse. The sentences live in
[`src/lib/ui/whydisabled.js`](../src/lib/ui/whydisabled.js) — one home, for the same reason
`errors.js` and `settingvalue.js` have one — and they may not overstate the damage: a dropped
Tauri bridge does not take down a screen that is already lit, so the sentence says the control
cannot act **from here** and that whatever is up is still up.

**Three shared state components, and they are not interchangeable:** `EmptyState`, `Loading`,
`ErrorState`. *Empty* ≠ *loading* ≠ *error* — Live once said "No plans yet" before the database
had answered. `ErrorState` offers **Try again** only when the backend says the fault is
transient, which is the first place typed errors (`error.rs`) earn their keep.

**`src/lib/errors.js` is the ONE backend-error humaniser.** Never render a raw Rust `Err` string
to a volunteer — Channels did, in monospace, five times.

---

## 9. Reference material

**There is none, and that is the current state rather than an omission.** Thirteen rendered
screen references lived in `docs/design/` as PNGs — the console, the live production interface,
Templates and its editor, Channels, Planner, Library, Settings, Stage display and its editor,
Help, the splash and a design-system sheet. They were **deleted on 2026-09-21**, on the
operator's instruction, because the product no longer works from that design.

They were only ever a **record of intent**, never a spec: this document and `src/app.css` always
outranked them, and the twelve source comments that cited one by name have each been rewritten
to state what the reference gave rather than to point at it. Nothing about the shipped look
changed when they went. **This document and the stylesheet are the reference now**, which is
what they were in practice already.
