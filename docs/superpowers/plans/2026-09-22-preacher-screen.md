# The preacher's screen, rebuilt — 2026-09-22

**Status: BUILT and superseded in one place — closed 2026-09-22.** Every item below shipped as
RG-239 … RG-253. Read the register rows rather than this document for what the code does: a plan
kept as a description of shipped code becomes a second source of truth, and this repository has
been bitten by that more than once. What this file is still good for is the REASONING, which the
register rows cite.

**The one item that did not survive contact with the operator is §5's press-and-hold.** It
shipped as RG-242 and was retired eight hours later by RG-246 on the operator's own instruction —
*"there's no need to have any Hold button on the screen"*. The transport is a permanent bar along
the foot instead, three 52px targets, which is a harder thing to hit by accident than the 26px
header control the hold was protecting. The guard was a correct answer to the wrong shape.

**Two later instructions overrode the drawing** and are recorded here so this file does not read
as the last word: the wall clock is a small line BELOW the Stage Timer rather than beside it
(RG-248, RG-249), and at rest the timer is the whole screen with no standby line above it
(RG-253).

<details><summary>The plan as written, 2026-09-22</summary>

**Status: plan. No production code yet**, which is the brief's own first instruction. Every
claim below says whether it was VERIFIED against the tree or is a DECISION this plan is taking,
because the last two briefs each contained one thing that was already built and one that was a
different defect from the one reported.

**The surface.** `src/Stage.svelte`, served as `stage.html`, opened on a phone or a platform
monitor. It draws its own zones and renders no template — that is deliberate and does not change
here. The operator's side is `Outputs → Screens`, which already assigns a stage layout per screen
and publishes it as a `stage_zones` frame.

---

## 0. What is actually there now, verified

| # | The brief | What the code says | Status |
|---|---|---|---|
| 2 | A message alone on screen should be LARGE | `.quietmsg` is a fixed strip along the foot — `bottom: 2vh`, `padding: 1.1vh 2.4vw`, one line. It is the same size whether the screen is full of verse or completely empty | **VERIFIED** — there is no large state at all |
| 3 | Every message should be noticeable | `.quietmsg` does not animate. Only `.alert` (full-bleed, `alertSize` stepped by length) flashes | **VERIFIED** |
| 4 | Build around the TIMER, not the clock | `figureList` is `[countdown?, clock?, elapsed?]`, all three the same size through `figCh`, and the programme rail is a separate 26%/15% region. The wall clock is as large as the countdown | **VERIFIED** |
| 5 | Only the operator picks zones | `stage.html`'s header carries a **Zones** button (`showZones`) writing `localStorage`; the operator's layout arrives over `stage_zones` and the device can override it | **VERIFIED** |
| 6 | Operator-set timer size | Nothing anywhere. The rail sizes itself from its own box | **VERIFIED as absent** |
| 7 | The remaining control should be solid | The **Control** panel (prev/next/search/fire) opens from the same header, one tap, mid-sermon | **VERIFIED** |

---

## 1. The shape, in one sentence

**Two things matter on this screen: how long is left, and what the operator needs to say.**
Everything else is secondary and most of it is switchable. The rebuild makes that literal — the
timer and the message are the only two regions that may take the screen's height, and the clock
becomes a small figure that can be switched off entirely.

## 2. The three states, drawn

See the canvas linked in the summary. In words:

**Resting** — nothing fired. The timer fills the reading's room. The clock is a small line under
it. Up Next and the Stage Note keep their strips.

**A message** — an ordinary Stage Message with nothing else on screen. The message takes the
room the reading would have had and the timer shrinks to the figure row. The message is coloured
and pulses.

**An alert** — unchanged from today: full-bleed, red, flashing, over everything including the
timer. It is the state that must stop a service.

## 3. The size rule, stated

- A message **alone** on screen: `clamp(4vh, 11cqw, 13vh)` per line, laid out in the reading's
  box with `overflow: hidden` — the same box the verse uses, so it can never take more room than
  a reading would have.
- A message **beside a reading**: the strip it is today, unchanged. A reading is why the preacher
  is looking at the screen.
- The timer at rest: `clamp(6vh, 26cqw, 34vh)`, capped by its own box exactly as RG-223 records.
- The clock, when shown: never larger than `0.42 ×` the timer's own size, by construction rather
  than by a second number.

## 4. Noticeable, without spending a law colour

Rule 18 leaves amber, cyan and amethyst spent. The message pulses between the page's own ink and
**ochre** (`--v-caution`), which is the caution ink DECISIONS §111 added and the only one free.
The alert keeps rose. **What separates them**, stated here and to be repeated in the code:

| | Ordinary message | Alert |
|---|---|---|
| Room | The reading's box, or a strip beside a reading | The whole screen |
| Ink | Ochre on the page's ground | Rose, full-bleed |
| Motion | A 2s pulse of the text | The panel itself flashes |
| Reduced motion | A steady ochre panel with a left rule | A steady rose panel |

## 5. Who chooses what

**The Zones button leaves `stage.html`.** The same switches are already in Outputs; a phone that
has been handed a layout may not override it. What stays on the device is the **Control** panel,
and it gets the protection the brief asks for: a press-and-hold to open rather than a tap, so a
sleeve cannot open it mid-sermon.

**Timer size joins the layout.** `stage_layouts` grows a `timer_size` column, the frame carries
it, the page honours it, and `Outputs → Stage layout` offers it. One delivery path, one writer.

## 6. Order of work

1. The size rule and the two message states (items 2 and 3) — the thing a preacher sees.
2. The timer-first layout (item 4).
3. Zones off the device, timer size on the layout (items 5 and 6) — one migration, one frame.
4. The Control panel's guard (item 7).

Each lands with its test written first, and nothing here changes the kiosk hub's silence about
who connected (DECISIONS §35).

</details>
