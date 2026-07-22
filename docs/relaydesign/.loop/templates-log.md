# Templates — Output Designer — design loop log

Reference: `docs/relaydesign/relay-templetedesigner-screen.png`
Tokens: `docs/relaydesign/relay-designsystem.png` v1.0
File: `src/lib/views/Templates.svelte` (markup + styles rewritten; script kept, plus
view-only editor chrome). Tab title in `src/App.svelte`.

**Compare method: PIXEL, populated.** New this pass — Templates is a console screen
with no backend in a plain browser, so it would normally render as an empty shell.
Instead the capture rig stubs `window.__TAURI_INTERNALS__.invoke`, which is what
`@tauri-apps/api/core` dispatches through. That makes `capture.available` true and
lets the **real store code** run against four fixture templates. **No app code was
changed for this** — the stub lives entirely in the capture script.

Captured headless at 1536×1024, DPR 1, `reducedMotion: reduce`, against
`vite preview` of the production `dist`. Zero page errors on every iteration.
Gate each iteration: `npm run build` clean + `npx vitest run` 140/140.

---

## Iteration 1 — `templates-1.png`

Rebuilt to the reference's four zones: **Elements rail · canvas toolbar + stage ·
Text/Background inspector · bottom bar**. Every existing control was kept and
re-dressed into the reference's grammar (label-left / control-right rows, sectioned
inspector, element-visibility rail) rather than reduced to only what the mockup draws.

Diffs read back off the screenshot:

- **The artboard was far too small.** `zoom` defaulted to 72% of the pane, so the
  canvas floated in a large black field. The reference's canvas nearly fills its panel.
- **Bottom bar was cramped** — "Transparent background" wrapped to two lines, the
  hint truncated mid-word, "Content Type" wrapped.
- **No canvas grid.** The reference draws a fine graph-paper field behind the artboard.
- The "Detect installed fonts" link sat in the value column of an otherwise empty row.

## Iteration 2 — `templates-2.png`

- Zoom re-scaled: `ZOOMS = [40,55,70,85,100]`, defaulting to **100 = fills the pane**.
  The percentage is display scale against the pane, **not** against 1920 — noted
  below as an inferred meaning.
- Added the grid (`32px` lines in `--v-line`, radial-masked so it fades at the edges).
- Bottom bar: hint shortened, `white-space: nowrap` + `flex:0 0 auto` on the toggle
  and the label so the row can no longer wrap.
- Font-detect link promoted out of the row grid.

Left: artboard touched the pane edges; inspector rhythm tighter than the reference's.

## Iteration 3 — `templates-3.png` — STOP

- Stage padding `--v-sp-md → --v-sp-lg`, so the artboard sits clear of the pane.
- Inspector: padding 14→16, row gap 9→11, section headings `--v-fs-h3 → --v-fs-h2`
  with the sheet's `-0.01em` H2 tracking.
- Tab title `Template Editor` → **`Templates — Output Designer`**, matching the
  reference's own title. Label only.

**Stop condition met**: the last two iterations produced only spacing refinements,
and every remaining diff is a logged omission below.

---

## Deliberately NOT drawn

Same rule as the Live console: a control with no backend is not decoration, it is a
lie an operator will eventually trust.

| In the reference | Why it is absent |
|---|---|
| **Undo / redo** | No edit history in the editor. Adding one is a feature. |
| **Zoom as a dropdown**, aspect as a dropdown | Zoom is two buttons + a readout. **Aspect is a readout, not a picker** — `TemplateRender` sizes everything in `cqw`, so a template renders identically at any output size. A dropdown would imply a choice that does not exist. |
| **`Translation`, `Logo`** elements | Relay's template model has no such regions. The rail draws the five that ARE real. |
| **`Weight`** stepper under Text | Templates carry a family, not a weight. |
| **Background `Opacity`** slider | Not modelled. Fill colour and image are. |
| **`Content Type: Scripture`** dropdown | Templates are assigned per content type elsewhere; this is a readout of what the canvas is showing, so it is a chip, not a second place to set it. |

## Added, though NOT in the reference

- **`Template → Name`** field, **Saved-templates list** (with the ≤4 active stars and
  the two-step delete), **Ref size**, **Accent**, **Ref align**, **Reference above
  verse**, **Italic reference**, **Transition**. All pre-existing controls. The
  reference's inspector is sparser than Relay's actual template model; deleting
  controls to match a picture would delete the feature.

## One deliberate deviation from the reference's pixels

**Save is green, not violet.** The reference paints the Save button violet. In this
app **amethyst means REHEARSAL** and **amber means ON AIR** — neither is true of
saving a template, and a violet primary button in the editor would read as a mode
indicator. The design sheet's own button row publishes a **CONFIRM ACTION** in green
(`--v-emerald`), which is exactly what Save is. Colour law outranks one mockup.

Same reasoning for the bottom-bar toggle: green (confirmed/on), per the sheet's
toggle spec, never amber.

## Still differs, and why

- **Panel widths.** The reference is a full-window designer with no app sidebar;
  Relay's shell takes 238px. Rail 252 / inspector 316 here vs ~355 each there.
- **Toolbar density.** With undo/redo and two dropdowns removed, the left of the
  toolbar is lighter than the reference's.
- **The reference's artboard imagery** (a navy/gold line-art plate under white bold
  Inter) is a background asset this repo does not ship. The canvas renders whatever
  the selected template actually is — here `Classic Serif`.

## Inferred rather than read from the design system

- **Zoom percentages mean display scale against the pane**, not against a 1920 canvas.
  The reference shows "72%" with a nearly-full canvas, which cannot be 72% of 1920 in
  that panel; treating it as fit-scale is the only self-consistent reading.
- **Canvas grid**: 32px, `--v-line`, radial-masked. The sheet publishes no grid token.
- **Safe-area guide inset 5%** — a conventional title-safe margin; the reference draws
  a dashed rect but publishes no figure.
- **`.el.off` uses `--v-disabled`** for the icon and eye of a hidden element. The sheet
  publishes the token but not this usage.
- **Stepper unit chips** (`cqw`, `ms`) — the sheet shows a `px` unit chip on the Size
  field; the units here are the ones Relay's model actually stores.

## Harness note (reusable)

`docs/relaydesign/.loop/` capture of any console screen can now render **populated**
by stubbing `window.__TAURI_INTERNALS__.invoke` in an init script and returning
fixture rows per command. This is a test harness, not a code path — the app is
unmodified, and the real store/view code runs. It does not make the Tauri webview
capturable; it makes the *browser* render the same components with data.

---

# Templates v2 — Gallery + Editor split (this session)

References: `relay-templetes-screen.png` (gallery) + `relay-templeteeditor-screen.png`
(editor). These are two NEW mockups that define Templates as two screens, superseding
the single Output Designer above.

Gate: `npm run build` clean, **270 frontend** (+7), `npx vitest run` green. No Rust
touched — this is a frontend rework over the unchanged template model. Compare:
**pixel**, gallery + editor + Usage-tab variants, captured via the stubbed-IPC rig
against the five seeded built-ins + real channel/content-template fixtures.

Screens: `templates-base.png` (before) → `templates-new-1.png` (gallery),
`templates-new-3.png` (editor), `templates-new-usage.png` (Usage tab).

## What was built

- `src/lib/templateKind.js` (+ 7 tests) — **derives** a template's kind from its
  region shape: lower-third band → Lower Third; reference+verse → Scripture; verse
  alone → Song; else Custom. This is the whole answer to the gallery's type tabs,
  and it needs no migration and invents no data — the tab reflects what the
  template genuinely renders and can never disagree with it.
- `TemplateGallery.svelte` (new) — cards rendered through the **real
  `TemplateRender`**, so a card is what the wall shows, not a drawing of it. Type
  tabs (only kinds that occur), search, sort, grid/list, preview inspector with
  Details + Usage. Star = console-active (≤4), ⋮ = edit/duplicate/delete.
- `TemplateEditor.svelte` (reworks the old Output Designer) — the mockup's layers +
  design grammar. LAYERS lists the template's REAL parts; the DESIGN panel edits
  the selected layer's real style props. Every control that existed is kept.
- `Templates.svelte` — thin gallery↔editor switch (same shape as Planner).

## Real data the inspector shows

- **Assigned to outputs** — REAL: a channel stores `template_id`, so the Usage tab
  lists the channels actually pointing at this template (Main Projector + Lobby
  Screen for Classic Serif in the fixture).
- **Default for content type** — REAL: the per-content-type template map
  (`get_content_templates`), so "scripture" shows when this template is that default.

## Refused — the editor mockup is a Canva/Photoshop compositor; the model is not

The renderer reads exactly 12 style keys + 4 layout keys. Everything below has no
field to store it and no code to render it, so a control for it would be a lie:

| In the mockup | Why absent |
|---|---|
| **Assets library** (backgrounds/gradients/ornaments/shapes/lines/frames/dividers/quotes/badges) | No shared asset store exists; a background is one per-template fill or uploaded image. The layers panel says so in a note instead of drawing a fake browser. |
| **Arbitrary layers** (Ornament L/R, Gradient Overlay, Bottom Glow, Watermark) + **Add Layer** | The layout has a fixed region set (`verse_text`, `reference`) + the band. No z-ordered free layers. Layers map to the real parts only. |
| **Effects** (drop shadow, outer glow, background vignette) | `TemplateRender` reads no shadow/filter property. |
| **ANIMATION tab** | The only motion is one crossfade `transitionMs`, kept in the Motion section. No keyframes. |
| **ADVANCED tab**, **blend mode**, **opacity**, **letter-spacing**, **divider style/colour/thickness** | None are in the style model. |
| **Weight** stepper | Templates carry a family, not a weight. |

## Refused/substituted on the gallery

- **Created / Last modified / "used 26 times"** (Details) — templates carry no
  timestamps and Relay keeps no per-template usage count. Omitted, not invented.
- **Orientation / Resolution / FPS** — every template is 16:9 by construction
  (cqw). Shown as a `16:9 · 1920×1080` READOUT, never a picker.
- **Announcements / Countdowns type tabs** — not derivable from a template's shape
  (an announcement is just `verse_text`; countdown is content-driven), so no tab
  for them. Only kinds that actually occur are shown.
- **Import** — template import does not exist (ProPresenter import brings in
  *songs*). Omitted; only New Template is offered.
- The card's **ON AIR** badge → an accent **star** meaning console-active. Amber is
  ON AIR and a template being active is not live; the star is the honest claim.

## One deliberate pixel deviation (unchanged rule)

**Save is green, not violet.** Amethyst = REHEARSAL, amber = ON AIR; saving a
template is neither. The design sheet's CONFIRM ACTION is green, which is what Save
is. Colour law outranks the mockup.

## Polish found by the pixels

- The Font dropdown showed the raw stored value `var(--f-serif)`. The seeds store
  CSS variables, not family names, so a `FONT_LABEL` map now shows "Fraunces
  (serif)" while the stored value is untouched.

## Not verified here

Stubbed-IPC webview. Not exercised end-to-end: a real `save_template` round-trip
from the reworked editor (the store wrapper and upsert are unchanged and covered by
existing Rust tests), and the packaged-build CSP.

---

# Templates v2.1 — menu fix, live-apply, 14 presets (this session)

Follow-up on three requests. Gate: **361 Rust** (+6), **270 frontend**, build +
clippy clean, scorecard 100%/0.

## 1. The ⋮ row menu was clipped and unusable

The dropdown was drawn INSIDE the card, which is `overflow:hidden` (rounded
thumbnail) inside a scrolling grid — so on the bottom row it was cut off entirely
and its items could not be reached. Rewritten as a single **fixed-position** menu
anchored to the button's screen rect (`getBoundingClientRect`), so it escapes
every overflow context and flips upward near the bottom edge. Closes on outside
click, scroll, and resize. Verified in the DOM (150×97 at a valid on-screen
position) and visually (Edit / Duplicate / Delete open below the button, unclipped).

## 2. Edits now reach live outputs without saving-and-exiting

Requested: "update on the live screen directly once it's changed, rather than
exit first." The editor now **debounced-auto-applies** every edit (400ms) through
the existing `save_template` path — which persists, emits `template://updated`
(native output windows re-render) and pushes the JSON to the kiosk hub (OBS/kiosk
clients re-render). So an output currently showing the template restyles live as
you edit. Scoped and safe: only outputs already displaying that template id react;
nothing new is put on a wall. The Save button becomes "Save now" (immediate, same
push) and reads "Saved · live ✓". A load seeds the change-signature so merely
opening a template pushes nothing; a pending push is cancelled on destroy.

## 3. Fourteen ready-to-use presets

`preset_templates()` in `db/templates.rs`, added by a new idempotent, additive
`ensure_preset_templates` (matched BY NAME so the five originals keep their stable
ids and no channel/plan foreign key is ever repointed — same discipline as
`ensure_lyrics_template`). Runs on every boot, so existing installs get them too.

The set, across every screen type + stage:

- **Scripture** (dark, light-on-dark, high contrast): Midnight Blue, Royal
  Amethyst, Deep Teal, Crimson Grace, Emerald Word, Indigo Night, Slate Minimal,
  Pure Contrast, Lobby Sunrise.
- **Songs / lyrics** (verse only, large, no citation): Lyric Bold, Lyric Glow.
- **Lower thirds** (solid keyed-out band): Lower Third Light, Lower Third Night.
- **Stage / confidence monitor** (ref-first, left, big): Stage Confidence.

Two constraints shaped every one: the model has **no image asset store**, so a
background is a CSS gradient or solid, never a photo — which is also what keeps
scripture legible (a photo behind a verse is the commonest way it becomes
unreadable). And a projector in a lit room needs **contrast**, so every non-band
preset is light text on a dark field. Sizes are cqw, so they scale to any output
and auto-shrink rather than overflow.

Four Rust tests: idempotent, 12–15 presets covering scripture/song/lower-third,
every preset valid JSON with a light-on-dark verse colour (bands excepted) and no
embedded image, and no name collides with a built-in. Two existing seed-count
tests now assert against `5 + preset_template_count()` rather than a hard number.

## Not verified here

The live-apply push to a REAL output window/kiosk end-to-end (stub rejects
`save_template`, so only the no-error path and the unchanged save wrapper are
exercised here); worth a real edit-while-output-open pass in the running app. The
presets are proven to seed and render (19-template gallery screenshot), not
inspected on a physical 4K wall.

---

# Templates v2.2 — Planner fix, style properties, fonts, backgrounds, themes

Gate: **363 Rust** (+2), **282 frontend** (+12), build + clippy clean, scorecard 100%/0.

## 1. Planner now uses the template set for each cue (the reported bug)

A plan item stored a `template_id` but every fire path resolved the template from
the content TYPE only (`content_tpl(kind)`), so the per-cue choice was dead data —
a scripture cue always rendered with the one scripture default. Fixed end to end:
- `cue_or_content_tpl(conn, cue_template_id, kind)` — the cue's own template wins,
  else the content default; a since-deleted template falls back to the default,
  not the channel.
- Threaded `template_id` through `resolve_fire` / `fire_manual` and the
  `manual_fire` / `fire_content` / `fire_media` / `start_countdown` commands; the
  store wrappers and `Live.svelte:fireSlide` now send `item.template_id`.
- Auto-detect and nav pass `None` (no cue behind them → content default).
- **Proven by a new e2e test** that drives the real fire path and asserts the
  cue's exact template id (and its style JSON) reach the wall.

## 2. New template style properties — renderer + editor + tests

`TemplateRender` now honours, and the editor exposes per section:
- **Background opacity** (`bgOpacity`) — the background is its own `.bglayer`, so
  it dims for readability over an image without fading the text.
- **Capitalization** (`verseTransform` / `refTransform`) — as-typed / UPPER /
  lower / Capitalize.
- **Line height** (`verseLineHeight`) and **letter spacing**
  (`verseLetterSpacing` / `refLetterSpacing`).
- **Text shadow** (`textShadow`, 0–1) — a soft drop shadow for legibility over a
  gradient/image; em-scaled so it tracks the text size.
- **Per-region text colour** — explicit `refColor` overrides the accent-derived
  reference colour.
- **Announcement scroll** (`scroll`) — the verse runs as a right-to-left ticker;
  the auto-fitter leaves it alone (it is meant to overflow), reduced-motion users
  get a static line.
- **Text scaling** — the existing verse/ref size controls, fully exposed.

12 renderer tests mount the real `TemplateRender` and assert the computed styles.
The existing lower-third tests were updated for the new `.bglayer`.

## 3. Fonts — every font on the computer, with honest fallback

The editor now enumerates the machine's installed fonts automatically on open
(`queryLocalFonts`, with the button as the gesture fallback) and offers them all.
When a chosen font is **not** installed, the editor says so and asks for it to be
installed; the renderer appends a generic fallback to a bare family name so an
absent font degrades to the computer's default rather than something arbitrary.
Bundled families (Fraunces/Inter/…) never count as missing.

## 4. Backgrounds — the pipeline is wired, waiting on files

Per the decision: images dropped into `src/backgrounds/` are globbed by
`src/lib/backgrounds.js` (`import.meta.glob`, eager `?url`) and offered in the
editor's **Background → Library** picker. Vite bundles each with a hashed URL that
resolves the same on the operator console, native output windows and kiosk/OBS
(`:8032`). The folder ships with a README; the picker shows a "drop files here"
hint until images exist. **Add the images you posted to `src/backgrounds/` and
they appear automatically** — then I'll build image-backed presets on top.

## 5. Themes — cohesive families

Three coordinated theme families (Aurora · teal, Ember · amber/crimson,
Nocturne · indigo), each a complete set — **Scripture · Lyrics · Lower Third ·
Announcement** — sharing one palette and named `Theme · Kind` so the gallery
groups them. They double as working examples of the new properties (soft shadow,
uppercase lower thirds, a scrolling announcement ticker). Seeded idempotently by
name alongside the 14 standalone presets (26 presets total). A test enforces that
every theme is complete — a half-built theme would drop one content type to a
mismatched default mid-service.

## Not verified here

The live style push and the Planner template on a REAL output window/kiosk end to
end (stubbed-IPC webview here; the fire path itself is covered by the e2e test).
Backgrounds are unproven until image files exist. `queryLocalFonts` behaviour in
the packaged Tauri webview (permission model) is not exercised in this browser
harness.

---

# Templates v2.3 — per-region editing, footer ticker, readability, bug fixes

## Bugs fixed
- **Editing one template bled into others.** `load()` used `structuredClone`,
  which THROWS on any non-cloneable value and left `edit` pointing at the
  PREVIOUS template — so edits to B mutated A. Now a JSON deep-clone: always a
  private copy, never throws on plain template data.
- **Turning a section off left its text (and shadow) in the footer ticker.** The
  ticker rendered `content.reference` unconditionally; it now obeys
  `show('reference')` / `show('verse_text')` like every other region.

## Per-region, independent editing
- **Font per region** — `verseFont` / `refFont`; each layer picks its own,
  falling back to the template `font`. The base font moved under "Template", each
  text layer has its own Font control.
- **Shadow per region** — `verseShadow` / `refShadow` (fall back to `textShadow`).
- These join the already-independent size / colour / align / caps / spacing.

## Footer ticker (ProPresenter-style)
`scroll` now renders a band pinned to the very BOTTOM of the screen — a fixed
label (the reference, if shown) plus the body crawling right-to-left at a constant
reading speed — instead of a centred marquee. Reduced-motion → static line.

## Readability tools for bright backgrounds
- **Background Opacity** (fade) and **Dim** (a black scrim over the background,
  behind the text) — Dim is the real "knock down a bright screen" control.
- **Text panel** (a shape behind the words): colour + opacity + corner radius.
- **Background height** and **lower-third band height**.
- **Ref gap** — adjustable space between the verse and its reference.
- The Opacity/Dim/Height controls were moved ABOVE the (long) background image
  library and the library grid was height-capped — they were being buried.

## Gallery
- **Active templates float to the top**, reshuffled first.
- **"Use this template for"** (Usage tab) — assign the template as the default for
  Scripture / Songs / Announcements / Media straight from the gallery.

## Backgrounds pipeline confirmed
The user dropped images into `src/backgrounds/`; they now populate the editor's
Image Library automatically (Vite glob → bundled hashed URLs, resolve on every
screen).

## Still open (deferred, needs its own effort)
- **Arbitrary add/remove layers.** The model is fixed regions (background,
  reference, verse, band). A true dynamic layer system (add text/shape layers,
  reorder, per-layer transform) is a new `layers[]` model + renderer rewrite +
  migration — a dedicated build, not a tweak. Flagged to the operator.

## Gate
Frontend: build clean, **290 tests**. Rust: compiles clean, **clippy 0**; the
test *binary* could not be launched at write time due to a macOS dyld
shared-cache failure (`OSAKit … no dyld cache`) — an OS loader state, not a code
failure (the same suite passed earlier this session). A reboot clears it.
