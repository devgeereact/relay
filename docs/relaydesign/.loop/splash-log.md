# Splash — design loop log

Reference: `docs/relaydesign/relay-splash-screen.png` (1536×1024)
Tokens: `docs/relaydesign/relay-designsystem.png` v1.0

**Compare method: PIXEL.** The splash needs no backend, so it renders standalone.
Captured with headless Chromium against `vite preview` of the production `dist`
at 1536×1024, DPR 1, `reducedMotion: reduce`. The boot flag is held open for the
capture by stalling the two teardown timers — the screen itself is unmodified.
(This is the exception to CLAUDE.md's "cannot screenshot the app": the *console*
still cannot be captured from the Tauri webview here, but the splash can, because
it has no live dependencies.)

Build gate each iteration: `npm run build` clean + `npx vitest run` 140/140.

---

## Iteration 0 — tokens (`src/app.css`)

Retuned the `--v-*` palette to the published design system. **Names unchanged**, so
every consumer moves together (console, Output window via `output.js`, Stage via
`stage.js`) and no component had to be edited.

| token | was | now | design source |
|---|---|---|---|
| `--v-void` | `#0a0a0b` | `#0a0a0a` | Neutral 950 |
| `--v-bg` | `#0e0e0f` | `#141414` | Neutral 900 |
| `--v-surf` | `#141416` | `#1b1b1b` | Neutral 800 |
| `--v-surf2` | `#1a1a1d` | `#262626` | Neutral 700 |
| `--v-surf3` | `#212125` | `#333333` | Neutral 600 |
| `--v-txt` | `#e8e6e7` | `#f2f2f2` | Text Primary |
| `--v-dim` | `#a4a2a7` | `#b3b3b3` | Text Secondary |
| `--v-faint` | `#88888d` | `#8a8a8a` | Text Muted |
| `--v-amber` | `#f5a623` | `#ffb000` | On Air |
| `--v-amethyst` | `#c08bff` | `#8b5cf6` | Rehearsal |
| `--v-cyan` | `#3fb6e6` | `#22d3ee` | AI Guess |
| `--v-emerald` | `#10b981` | `#22c55e` | Confirmed |
| `--v-rose` | `#f4718b` | `#ef4444` | Error / Panic |

Added: `--v-500`, `--v-disabled`, `--v-inverse`, `--v-red(+soft)`, `--v-grey`
(cued), the radius scale `--v-r-sm…2xl`, the 8pt spacing scale `--v-sp-xs…5xl`,
and `--v-shadow-sm/md/lg`.

**Two things flagged rather than silently changed** (this pass is a rebrand, not a
behaviour change):

1. **The On Air badge does not use amber.** `App.svelte` renders On Air as
   `.r-badge rose` and "Screens clear" as `.r-badge amber` — the inverse of the
   design sheet's MODE INDICATORS, where amber *is* ON AIR. I retuned rose to the
   system's Error/Panic red but did **not** repoint which token On Air uses; that
   is a semantic decision about the tally light and needs a human call.
2. **`--v-surf3` got lighter** (`#212125` → `#333333`, Neutral 600). Muted text at
   `#8a8a8a` is ~5:1 on `--v-surf`/`--v-bg` (passes AA) but ~3.8:1 on the new
   surf3. Nothing currently puts faint text on surf3; new work must not.

Deferred, deliberately: the design sheet specifies **Inter for everything**, but
the app still uses Playfair Display for `--f-head` (sidebar wordmark, panel
titles — visible in `shell-rebrand-2.png`). Flipping that restyles every heading
on every console screen, which cannot be verified from here. Left for the
app-shell screen.

---

## Iteration 1 — `splash-1.png`

First build of `src/lib/Splash.svelte`, wired into `App.svelte`.

Measured against the reference:

- wordmark **522px** wide vs ref **601px** — tracking too tight (`0.2em`)
- centre stack **781px** tall vs ref **735px**, and sitting ~25px high
- title bar 44px vs ref ~68px
- edge line-art swept through the **centre** of the screen, behind the status text
- footer 79px tall vs ref 99px; content sat too low inside it
- logo outer bars were neutral grey (`--v-500`), ref is a violet-tinted slate

## Iteration 2 — `splash-2.png`

Tracking `0.2em → 0.28em`; tightened the four inter-element gaps (word 4.5→3.5vh,
tag 2.6→1.35vh, spinner 4.4→2.9vh, detail 3.5→2.35vh); bar 44→68px; footer
padding up; narrowed the glow rule 78%→62%; added the wordmark's vertical sheen;
outer logo bars → `--v-amethyst-mute`.

Left: wordmark still 38px narrow (563 vs 601); logo bar **gaps** 15px vs ref 23px
(the `1.5vh` clamp under-resolved); whole stack still ~10px high — the reference
is not perfectly centred, it sits ~11px *below* the optical centre.

## Iteration 3 — `splash-3.png`

Tracking → `0.355em` (wordmark now 600px vs ref 601). Logo gap → `2.25vh` (bar
group now 215px, matching). `.core` given `padding-top: 2.2vh` to reproduce the
reference's downward nudge. Footer padding made top-weighted.

Every element within ~5px of the reference. One artifact left: all wave paths
terminated on a shared x, which read as a **hard vertical seam** at x≈470 and
x≈1090, most visible across the footer.

## Iteration 4 — `splash-4.png` — STOP

Staggered each wave's endpoint by index and added a centre-fading mask to
`.waves`; thinned the divider glow (the old spread read as a band, not a line);
kept more highlight in the wordmark through the baseline.

Seam gone. Screenshot and reference are indistinguishable at a glance.
**Stop condition met** (last iteration produced only tonal refinements).

---

## Still differs, and why

- **Window controls.** The reference draws its own minimise / maximise / close at
  top-right. Relay uses native window decorations; drawing three non-functional
  glyphs would be worse than omitting them. Only the identity half of the
  reference title bar is reproduced. Changing window decorations is a
  functionality change and out of scope for a rebrand.
- **`v1.0.0`.** Shown from Tauri's `getVersion()`, so it is correct in the desktop
  app but empty in the headless capture (no backend) — hence absent from the PNGs.
  It reads the *same* version the updater compares against, so it cannot drift
  (CLAUDE.md §19).
- **Spinner.** Captured with `reducedMotion: reduce`, so all eight dots are at
  equal opacity; the reference shows the graded comet, which is the animation's
  mid-frame. Motion, not geometry.

## Inferred (not read from the design sheet)

- `--v-amethyst2: #a78bfa` — the sheet publishes one amethyst; the splash's
  wordmark gradient and tagline are a step lighter than `#8b5cf6`.
- `--v-amethyst-mute: #4a4658` — the muted outer pair of the logo bars.
- `--v-amethyst-soft: rgba(139,92,246,.13)` — matches the existing `*-soft` pattern.
- Wave line-art geometry, the corner violet lift, and the divider glow falloff are
  all approximations; the reference appears to use a rendered asset.
- All splash-internal spacing is expressed in `vh` clamps solved against the
  reference's 1024px height, so the screen scales instead of only matching at one size.

## Behaviour notes (reviewed, not incidental)

- The splash comes down on a **hard 4s cap** as well as on success. A boot screen
  that outlives its boot is indistinguishable from a hung app, and it would be
  covering the console. Verified: it auto-dismisses with no backend at all.
- It sits at `z-index: 1100` — **below** the panic bar (1200). Nothing may hide
  "the screens may still be live", not even the brand.
- Nothing on it is amber. Amber is the tally light; during boot nothing is on any
  wall.

---

# Second pass (fresh session) — iterations 5–7

Re-verified rather than re-done: the token rebrand (iteration 0) and the splash
build were already in the working tree, uncommitted. Gate re-run clean:
`npm run build` OK, `npx vitest run` 140/140. **Compare method: PIXEL**, same rig
as before (headless Chromium, `vite preview` of production `dist`, 1536×1024,
DPR 1, `reducedMotion: reduce`).

## Iteration 5 — `splash-5.png` (verification capture, no code change)

Confirms the iteration-4 state still renders with zero console errors. Reading it
back against the reference surfaced two diffs the earlier pass had called done:

1. **The divider had no centre hotspot.** The reference lights the rule from a
   bright violet bloom at its midpoint; ours was a flat 1px gradient with a thin
   box-shadow, which read as a hairline, not a light source.
2. **The edge line-art was far too faint** (`opacity: .13`). In the reference the
   left sweep and right fan are clearly legible against the void; ours were
   almost invisible at the left edge.

## Iteration 6 — `splash-6.png`

- Added `.rule::after`: a 34%-wide, 46px-tall radial bloom centred on the rule.
- Waves `opacity: .13 → .30`.

Bloom now matches. New diff introduced: at .30 the waves ran straight through the
**footer**, which is clean in the reference.

## Iteration 7 — `splash-7.png` — STOP

- Waves `opacity: .30 → .26`.
- Second mask on `.waves` — a `180deg` fade from 84% to 96% height — composited
  with the existing centre fade (`mask-composite: intersect`, plus the `-webkit-`
  `source-in` fallback, which is the equivalent for that older syntax).

Footer clean, waves legible, divider lit. **Stop condition met**: indistinguishable
at a glance, and the remaining diffs are the ones listed above under "Still
differs" — all of them out of a rebrand's scope or absent-by-design.

## Inferred this pass

- `rgba(196,176,255,.5)` — the divider hotspot's core colour. The design sheet
  publishes no "glow" token; this is one step lighter than `--v-amethyst2` and is
  local to the splash, so it was not promoted to a `--v-*` token.
- Bloom geometry (34% × 46px) and the wave mask stops (84%/96%) are solved against
  the reference by eye; the reference appears to use a rendered asset.
- Wave opacity `.26` — the value at which the left sweep is as legible as the
  reference without reading as a foreground element.

## Design-hook finding, left unchanged

`impeccable` flags `gradient-text` on the `.word` (RELAY wordmark). **Left as is,
deliberately**: the reference mockup renders the wordmark with a vertical metallic
sheen, and reproducing it is the entire point of this task. It is a brand lockup
on a boot screen, not a heading or a metric. It is the only gradient text in the
app.
