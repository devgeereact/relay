# Help / Shortcuts — design loop log

Reference: `docs/relaydesign/relay-helpandshortcut-screen.png`
Tokens: `docs/relaydesign/relay-designsystem.png` v1.0
Files: `src/lib/views/Help.svelte` (board added; troubleshooting kept), tab title in
`src/App.svelte`.

**Compare method: PIXEL, populated.** Captured headless at 1536×1024, DPR 1,
`reducedMotion: reduce`, against `vite preview` of the production `dist`, with the
`window.__TAURI_INTERNALS__` stub from the Templates pass (see `templates-log.md`).
Zero page errors. Gate: `npm run build` clean + `npx vitest run` 140/140.

---

## The thing that mattered more than the pixels

**This is the most dangerous screen in the set to copy literally.** The reference
prints bindings Relay does not have, and several that contradict the real ones:

| reference says | Relay actually binds |
|---|---|
| `Ctrl+Shift+C` — Clear Screens | **`Esc`** |
| `Ctrl+Shift+B` — Blackout | **`B`** |
| `Space` — Play / Pause | **`Space` = ADVANCE**, app-wide, and nothing else (CLAUDE.md §11) |
| `Enter` — Confirm & Fire | **`A`** accepts the top suggestion |
| `S` — Suggest (No Fire) | no such binding |
| `M` / `I` / `L` / `P` | no such bindings |

`shortcuts.js` says of its table: *"also what the cheatsheet renders, so the help can
never drift out of sync with the actual bindings."* And CLAUDE.md, on a previous bug
in this exact screen: *"A help screen that teaches a false fact about a PANIC key, to
someone who will only read it under pressure, is the worst line in the app."*

So: **the layout is the reference's, the content is `SHORTCUTS`.** Every row on the
board is generated from the same table the keydown handler reads. Nothing is typed
in by hand.

Grouping is by key with a **safe default**: `Esc` and `B` are Panic; everything else
that is `always` goes to Other; everything with a `needs` goes to Transport. A
binding added to `shortcuts.js` tomorrow still appears on this screen — but can never
silently claim to be a panic control.

## Iteration 1 — `help-1.png`

Built the three-column board: **Panic controls · Transport (Live) · Other shortcuts**,
with the reference's red gradient panic cards, icon/title/subtitle/key rows, the
green "Live shortcuts" chip, the amethyst priority callout, and the `Esc`-closes-overlay
note. Troubleshooting topics kept below it.

Diffs read back off the screenshot:

- **`Jump to the manual reference box` printed its own label twice** — the subtitle
  map repeated the table's label.
- **Row order** followed the table (`A`, `D`, `→`, `←`, `/`); the reference leads with
  the nav keys, which are also the most-pressed.
- **Panic cards were list-row height**, not the reference's heavier blocks.

## Iteration 2 — `help-2.png` — STOP

- Distinct subtitle for `/`.
- Display-order map `['→','←','A','D','/']`. Presentation only — it reorders what is
  already in `SHORTCUTS` and cannot add or rename a binding.
- Panic-card padding 16→22, title line-height set.

**Stop condition met**: second iteration produced only refinements; everything left is
listed below.

---

## Deliberately NOT drawn

| In the reference | Why |
|---|---|
| `Play / Pause` (Space) | Space is *advance*. A pause control does not exist, and binding a second meaning to Space is explicitly forbidden. |
| `Suggest (No Fire)` (S) | No such action. The AI's suggestion already appears without firing — that IS the default. |
| `Toggle Mode (M)` | **Mode is derived, not chosen.** It follows what is on the wall. A key that "toggles" it would let the operator desync the transport from reality. Explained in words in the Mode block instead. |
| `Open Inspector (I)`, `Open Library (L)`, `Show / Hide Plan (P)` | No such bindings. |
| `Ctrl+Shift+…` panic combos | The real ones are `Esc` and `B`, and they are single keys **on purpose** — a panic control you need two hands for is not a panic control. |

The "Other shortcuts" column is therefore shorter than the reference's six rows. It
holds the two that exist. Padding it out was the whole failure mode this screen is
guarding against.

## Added, though NOT in the reference

- **The troubleshooting accordion** (7 topics + search) below the board. The reference
  is shortcuts-only; this is the rest of the Help tab and the reason it works offline.
- **The `Esc`-in-a-dialog exception**, stated on the panic card rather than left to be
  discovered: `Esc` dismisses an open dialog instead of clearing, and `B` does not fire
  while the cursor is in a text box. Both are real behaviours of `shortcuts.js` and both
  are the sort of thing that is only ever read under pressure.
- **A line saying where the keys come from**, so a future reader knows the board is
  generated rather than transcribed.

## Colour decisions

- **Panic cards are red** (`--v-red`, the sheet's Error/Panic) — matching the reference.
  Never amber (the congregation is looking at it) and never amethyst (rehearsal).
- **The priority callout is amethyst** as in the reference. Acceptable here: it is a
  note *about* panic controls, not a mode indicator, and no output state is implied.
- **The Mode block gets no semantic colour at all.** The reference tints "VERSE"
  violet; amethyst means rehearsal, so VERSE and SLIDE are set in mono on the neutral
  ramp instead.

## Flagged, deliberately NOT changed

`Live.svelte`'s transport-mode chip colours **SLIDE amber** (`.rack-mode.slide`) and
**VERSE cyan**. This predates this session — the original Console had the same
`--s-gold` / `--s-cyan` pair — but it does put amber on something that is not an
on-air state, which CLAUDE.md reserves it for. Raising it rather than silently
changing it: it is a live-surface semantics call, not a rebrand one.

## Still differs, and why

- **Column heights.** The reference's three columns are balanced because it invents
  six "Other" shortcuts. Ours ends short — the honest consequence of only having two.
- **Panel width.** The reference is a full-window overlay with no app sidebar.
- **Window controls** (minimise/maximise/close) in the reference title bar: Relay uses
  native decorations.

## Inferred rather than read from the design system

- **Panic gradient** `linear-gradient(100deg, var(--v-red), #c8302f)` — the sheet
  publishes one flat Error/Panic red; the reference's card is clearly a gradient. The
  darker stop is the only raw hex on this screen and is a shade of the token.
- **Icon glyphs and the per-key subtitles** are view copy; the sheet publishes an icon
  *style* (2px stroke, rounded caps) but not these symbols.
- **Row order** — see above.
- **Column ratio 1 / 1.25 / 1**, measured off the reference.
