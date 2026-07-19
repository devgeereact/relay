# Live (operator console) — design loop log

Reference: `docs/relaydesign/relay-console-screen.png` (1536×1024)
Tokens: `docs/relaydesign/relay-designsystem.png` v1.0
Files: `src/lib/views/Live.svelte` (markup + styles rewritten), `src/app.css`,
`src/App.svelte` (mode-colour law only).

**Compare method: PIXEL, with one honest caveat.** The console DOES render in a
plain browser — it just has no backend, so every panel shows its empty state.
Captured with headless Chromium against `vite preview` of the production `dist`
at 1536×1024, DPR 1, `reducedMotion: reduce`, `relay.session.v1` seeded with
`{setupDone:true, activeTab:'live'}`. Zero page errors on every capture.

So: **layout, chrome, spacing, type, colour and the empty states are pixel-verified.
The POPULATED states are not.** A claim card, a plan rail with a live cue, output
rows, transcript lines and a lit meter have never been seen on this machine — they
need a Tauri window with a database. Nothing below claims otherwise.

Gate each iteration: `npm run build` clean + `npx vitest run` 140/140.

---

## Iteration 0 — tokens and the mode-colour law (`src/app.css`, `src/App.svelte`)

The `--v-*` palette was already retuned to the design sheet in the splash pass.
This pass closed the two things that pass deliberately left open.

1. **`--f-head` was Playfair Display → now Inter.** The design sheet publishes ONE
   family (Inter) across Display / Heading / Body / Label / Caption. This is one
   token, so every heading in the console, Planner, Library, Channels and the
   template editors moved together. `--f-serif` is untouched: that is *verse body
   on the wall*, template content, which the sheet does not govern.
2. **On Air is AMBER.** `App.svelte` rendered the On Air badge as `.r-badge rose` —
   the system's Error/Panic red — and "Screens clear" as amber. That is inverted
   against both the design sheet's MODE INDICATORS and CLAUDE.md's own colour law
   ("amber = ON AIR"). On Air is now `amber pulse`, "Screens clear" is the new grey
   chip, and the footer tally dot follows. Panic/red is left where it belongs (the
   panic bar).

Added tokens: the full type scale (`--v-fs-d1…cap`, `--v-lh-*`, `--v-tr-*`) read
off the sheet's TYPOGRAPHY block, plus `--v-emerald-soft` / `--v-grey-soft` (the
two semantic colours that had no soft fill) and `.r-badge.grey` / `.r-badge.green`.

## Iteration 1 — `live-1.png`

First build of the reference's two-row layout:

- Row A — `PREVIEW · rack · PROGRAM · OUTPUT STATUS` at `1.19fr 92px 1fr 300px`
- Row B — the four numbered panels at `1fr 1.21fr 1fr 300px`

Both preview panes render through `TemplateRender`, the same component the real
output window uses, so the pair is WYSIWYG by construction.

Diffs read back off the screenshot:

- Panel 4 **overflowed its column** — "Next →" and the countdown Start button ran
  past the right edge (grid cells were `1fr`, not `minmax(0,1fr)`, so nothing shrank).
- Panel 1 and 2 headings **truncated** ("LIVE TRA…", "AI DETECTION — CURRENT C…").
- The PROGRAM pane rendered `TemplateRender` with null content, which draws a bare
  em-dash — visually identical to a blackout, which is a different fact.
- AUDIO MONITOR clipped off the bottom of panel 4.

## Iteration 2 — `live-2.png`

`minmax(0,1fr)` on both quick-control grids; `min-width:0` + ellipsis on every
button label; heading dropped to `--v-fs-cap` at `.06em` (the reference console has
no sidebar, so its panels are ~25% wider than they can be inside Relay's shell —
see "Still differs"); PROGRAM now says **"Screens clear"** in words when nothing is
live.

Left: panel 1's heading still truncated (the trailing language chip), "Countdown"
truncated to "C." in a half-width cell, audio monitor still clipped.

## Iteration 3 — `live-3.png`

Language readout moved out of panel 1's header into its mic footer (it is a
*detected* language and changes mid-sermon — it belongs next to the meter, not the
title). Countdown given its own full-width row. Headings now fit.

Left: the audio-monitor row still cut off at the bottom of panel 4 by ~30px.

## Iteration 4 — `live-4.png`

Trimmed panel 4's vertical rhythm (`qb` 10→8, `md` 8→6, `sb` 9→7, `wide` 34→32,
gaps 6→5). Everything fits with no scroll.

## Iteration 5 — `live-5.png` — STOP

Rack column: the `margin-top:auto` moved from the mode chip to the MODE *label*, so
the gap falls between the button group and the label rather than inside the pair.

**Stop condition met**: the last two iterations produced only spacing refinements,
and every remaining diff is in the list below — none of them fixable from code at
this layout, or unverifiable without a backend.

---

## Deliberately NOT drawn (the reference implies features Relay does not have)

A dead button in a live console is the exact failure this codebase keeps fixing, so
none of these were drawn as decoration:

| In the reference | Why it is absent |
|---|---|
| TRANSITION rack — Cut / Fade / Wipe / Stinger 1 / Stinger 2 / Duration | Relay has no transition engine. The column is instead the real take path: TAKE, ‹ ›, Dismiss, and the transport MODE. |
| `Fit` / `Safe Area` dropdowns on both monitors | No such control exists. |
| `Quick Transition` dropdown on PROGRAM | Same as the transition rack. |
| `HOLD OUTPUTS` / `OVERRIDE MODE` in Quick Controls | No backend. Replaced with the two real global modes Relay does have: **Rehearse** (amethyst) and **Detection on/off** (cyan). |
| `Back 5s` / `Forward 5s` (audio scrub) | Relay does not buffer or replay captured audio. |
| `Monitor Out` slider / `Mute Monitor` | There is no monitor bus. Input level is real and is drawn. |
| `Sensitivity` slider + 90% on panel 2 | The router's thresholds are self-calibrating and live in Settings; a slider here would need a new command. The armed/off chip is real and is drawn. |
| `SUGGEST — Add to plan` (middle of the three action buttons) | Writing to a plan from Live is a new command. Two buttons: **Accept & fire** (green) and **Dismiss** (red). |
| `End Service`, `Expand`, `Next Cue`, `View All`, bookmark, `Show more` | No callers. Panel 3's footer carries the flash message and the loaded plan's name instead. |
| Per-cue clock times (9:45, 10:00 …) in the plan | Plan cues carry no wall-clock time in the schema. The rail keeps the reference's dot + the cue ordinal. |
| Status strip (Recording · Auto Backup · Service Notes) | That is the app **shell** footer — panel 14, a different screen. |

## Added, though NOT in the reference

- **The manual reference box** (panel 2 footer). The mockup has no manual entry at
  all. It is the one path that works when the AI is wrong, the model is missing, or
  the plan has run out — deleting it to match a picture would delete the product's
  floor.
- **Related scripture** chips (quietest block on panel 2, no tally colour, no
  confidence — nobody *said* those references).

## Still differs, and why

- **Panel widths.** The reference console is a full 1536px window with **no
  sidebar**; Relay's shell owns a 238px sidebar plus 22px gutters, so the four
  panels get ~1252px, not 1536. Every column is therefore ~20–25% narrower than the
  reference at the same window size. Headings are set one step tighter to
  compensate. Reproducing the reference exactly would mean the Live tab hiding the
  app shell — a navigation change, not a rebrand.
- **Populated panels are unverified.** See the caveat at the top. The claim card,
  plan rail, output rows, transcript lines and lit meter are code-level matches
  only.
- **Imagery.** The reference's monitors show a photographic mountain plate under the
  verse. That is a template background asset this repo does not ship; the panes
  render whatever the operator's active template actually is.
- **Window controls / service selector / clock in the reference's top bar.** Shell
  chrome — the app-shell screen's job, not Live's.

## Inferred rather than read from the design sheet

- **Panel number chips (1–4)** — grey (`--v-surf3` on `--v-line2`). They are
  ordinals, not status, so they were kept off every semantic colour.
- **`--v-emerald-soft` / `--v-grey-soft`** — the sheet publishes the two hexes but
  no soft fills; these follow the existing `*-soft` alpha convention (.13/.16).
- **Meter segment breakpoints** (green < 15, amber 15–19, red ≥ 20 of 24). The sheet
  draws a green→amber→red level meter but publishes no thresholds. These are
  *display* stops on an already-computed level — nothing here compares a signal to
  an absolute level (DECISIONS §19).
- **Row A height** `clamp(268px, 33vh, 364px)` — solved from the reference's
  340/1024 proportion.
- **Column ratios** `1.19fr / 92px / 1fr / 300px` and `1fr / 1.21fr / 1fr / 300px` —
  measured off the reference (588/90/495/305 and 372/452/372/305) and renormalised.
- **The current transcript line is amethyst.** The reference highlights it in a
  violet tint. Amethyst normally means rehearsal, so this was checked rather than
  copied: the line is being *considered*, not fired, and amber (which would claim it
  is on the wall) would be the lie. Kept.
- **Transcript timestamps** are the moment the final arrived in front of *this*
  operator, stamped in the view. The store keeps no times; the alternative was
  changing the store or printing an invented time.
