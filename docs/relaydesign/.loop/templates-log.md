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
