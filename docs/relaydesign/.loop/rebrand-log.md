# App-wide rebrand pass — `docs/relaydesign/relay-designsystem.png` v1.0

Companion to `live-log.md` (which covers the Live console layout). This one covers
**every other surface**: making the design system the only source of colour and
type in the app, not just on one screen.

Gate: `npm run build` clean, `npx vitest run` 140/140 after every step.

---

## What was actually wrong

The `--v-*` tokens had been retuned to the design sheet, but almost nothing
*consumed* them. The old palette was still baked into components as raw `rgba()`
literals and hardcoded hexes, so most of the app was still rendering the
pre-rebrand colours regardless of what the tokens said.

Counted before the pass — distinct old-palette triples still hardcoded in `src/`:

| old value | what it was | count | now |
|---|---|---|---|
| `rgba(245,166,35)` | amber `#f5a623` | 31 | `255,176,0` — On Air `#FFB000` |
| `rgba(244,113,139)` | rose `#f4718b` | 14 | `239,68,68` — Error/Panic `#EF4444` |
| `rgba(192,139,255)` + `176,128,224` + `168,85,247` | violet | 8 | `139,92,246` — Rehearsal `#8B5CF6` |
| `rgba(16,185,129)` + `76,175,125` | green | 7 | `34,197,94` — Confirmed `#22C55E` |
| `rgba(63,182,230)` + `79,168,201` + `0,133,190` | teal/cyan | 6 | `34,211,238` — AI Guess `#22D3EE` |
| `rgba(255,157,148)` + `226,125,147` + `217,105,95` + `147,0,10` | assorted reds | 7 | `239,68,68` |

Remapped across 16 files by script, alpha preserved. Final scan for any
pre-rebrand hex outside a comment: **zero**.

## The three that actually mattered

1. **`TemplateRender.svelte` built its accent from `var(--amber)` — the LEGACY
   token.** That is the ONE renderer for the fullscreen output and the Templates
   preview, so **scripture on the actual wall was still the pre-rebrand gold**,
   no matter what the console showed. Now `var(--v-amber)`.
2. **`ModelSetup.svelte` used `var(--s-rose, #f4718b)`.** `--s-rose` only ever
   existed inside Live's old `.stx` scope; rewriting Live deleted that scope, so
   this silently fell through to its hardcoded pre-rebrand fallback.
3. **`Stage.svelte` — the preacher's phone — hardcoded every colour and font**,
   including `#6c6b71` at 3.75:1. That is the exact value `app.css` documents as
   REMOVED for failing WCAG AA, surviving on the one screen read at arm's length in
   a lit auditorium. Now `--v-faint` (5.61:1). Fonts moved to `--f-head` /
   `--f-serif` / `--f-body` / `--f-mono`.

## Legacy names are now aliases, not a second palette

The old `--amber` / `--rose` / `--surface` / `--text` family in `app.css` is kept —
deleting it would silently restyle anything a grep does not reach cleanly
(`TemplateRender` builds an inline `--accent`; `Channels` picks chip colours out of
an array). Every one of them is now `var(--v-*)`. So:

- there is exactly ONE place any brand hex is written down;
- anything still on an old name is on-brand by construction;
- `--f-display` (Space Grotesk) aliases to Inter, per the sheet's single-family rule.

## Other fixes in this pass

- **`crash.js`** — the crash-recovery dialog is a raw HTML string injected before
  Svelte mounts, so it cannot depend on a stylesheet. Its literal hexes are
  intentional; all ten were moved to the design-system values.
- **`templates.js`** — the four shipped output presets (Classic Serif, Stage Mono,
  Lower Third, Lobby Warm) had pre-rebrand accents. Repointed to amber / cyan /
  amethyst / green. These seed a **fresh install only** — an existing operator's
  saved templates in SQLite are untouched.
- **`Channels.svelte`** — channel chips were picked from the legacy
  `--amber/--teal/--violet/--rose/--text-faint` array; now the semantic tokens.
- **`FirstRun.svelte`** — `--f-display` → `--f-head`.
- **QR codes** (`Channels.svelte`) keep literal hexes (they draw a bitmap, not CSS)
  but moved to Neutral 950 `#0a0a0a`.

## A bug caught in the act

The bulk replace turned an `<input type="color">` default into
`value="var(--v-amber)"`. A colour input silently resets to `#000000` on any value
it cannot parse, so the Templates accent swatch would have shown black with no
error. Reverted to the literal `#ffb000`. **Any hex inside an `<input type="color">`,
a canvas call, or a QR generator must stay literal.**

## Fonts were NOT trimmed, deliberately

Space Grotesk and Playfair Display are no longer used by app chrome, so dropping
them from `fonts.js` looked like free bundle savings. They stay: `Templates.svelte`
offers both in the operator's **font picker for output templates**. The design sheet
governs the app's chrome, not the typography an operator chooses for the wall.

---

## Verification

**Stage (the preacher's phone) — PIXEL, against the live running engine.**
`docs/relaydesign/.loop/stage-2.png`, captured headless at 430×932 from
`http://localhost:8032/stage.html` served by the actual Tauri app with its real
backend. Zero page errors. Brand is Inter, amber is `#FFB000`, the connected dot is
`#22C55E`. `output-2.png` is the wall at 1920×1080 (screens clear at capture time).

**Console — code-level.** Still cannot be captured from the Tauri webview on this
machine; `live-log.md` has the headless empty-state captures.

## A dev-loop trap worth knowing

`channels.rs` embeds `dist/` at **compile time**
(`include_dir!("$CARGO_MANIFEST_DIR/../dist")`). So under `npm run tauri dev`:

- the **operator console** loads from the Vite dev server and hot-reloads;
- **`/output.html` and `/stage.html` on :8032 do NOT** — they serve a snapshot baked
  into the Rust binary.

Frontend changes are invisible on the wall and the phone until Rust is rebuilt.
`npm run build` then `touch src-tauri/src/channels.rs` re-embeds it (~3s). This is
how the first capture in this pass came back still showing the old serif wordmark.

Related: a **packaged** `.app` bakes the frontend in permanently — editing source
can never change a running release bundle, only a rebuild can.

---

# Pass 2 — the application mark, the icon, and the amber purge

Trigger: the human reported the laptop app "still has the old interface and the
colour and all", and asked for the design system applied everywhere including the
application icon.

## First, the boring cause

`/Applications/Relay.app` and `src-tauri/target/release/bundle/` were dated
**12 July**. The entire rebrand landed 17–18 July. The installed app was simply
older than the work — no amount of CSS would have changed it. Rebuilt.

## But three things were genuinely still wrong

### 1. The application icon was the old amber "R"

`src-tauri/icons/*` had never been regenerated. Amber is the tally light: an icon
sitting in the Dock in the ON AIR colour, permanently, whether or not Relay is
even running, is that colour telling its first lie before the app opens.

Now `src-tauri/icons/relay-mark.svg` — a committed, editable **source of truth**
traced from the design sheet's BRAND block — rendered to
`relay-icon-1024.png` and expanded with `npx tauri icon` (every macOS, Windows,
iOS and Android size). Verified by extracting `icon.icns` back out of the built
`Relay.app`.

### 2. The mark inside the app had the wrong number of bars — where it appeared at all

The design sheet's mark is **seven** bars. The splash and the boot shell each
hand-drew their own with **five**, and the sidebar had no mark at all — the app's
own logo appeared nowhere in the app. `src/lib/ui/BrandMark.svelte` is now the one
copy, geometrically identical to the icon, sized by any CSS length (the splash
still scales its hero mark with `clamp()`).

### 3. Amber was the chrome accent — see DECISIONS §22

The active nav item, focus rings, switches, sliders, every hover, the sidebar
avatar, the Settings headings, the code spans in Channels, and **23 ordinary
buttons** (Save song, Add channel, Import, Continue) were amber. A colour that is
always on cannot also be a warning, and §18/§20/§21 all depend on amber meaning
exactly one thing.

`relay-production-interface.png` settles it: the active sidebar item is
**amethyst-tinted, not amber**.

Introduced `--v-accent` (+ `-2`, `-fill`, `-soft`, `-line`, `-glow`, `-ink`) so
the rule is enforceable in one place, and `.r-btn.primary` as the default button.
`.r-btn.amber` survives, documented as ON AIR ACTION, deliberately not the easy
choice. Two badges also moved: **Engine ready** → green (the sheet's connected
colour), **detection method in Service History** → grey (nothing in last Sunday's
record is on air).

**Accessibility:** white on `--v-amethyst` (#8b5cf6) is **4.22:1** — under AA for
13px semibold. Button fills use `--v-accent-fill` (#7c3aed, **5.70:1**); the
lighter accent stays for borders and text on dark, where it is the thing being
read rather than the thing behind it.

## Verified

Rendered the real shell in headless Chromium at 1440×900 and read the pixels
back: `rebrand-shell-3.png`, `rebrand-sidebar-3.png`, and one per tab
(`rebrand-{channels,templates,library,planner,settings,help}.png`). Computed
style of the active nav item is `rgb(167,139,250)` = `--v-amethyst2`. No page
errors. Amber now appears only on TAKE, Fire and the On Air badges.

Gate: `cargo fmt --check` clean, `clippy -D warnings` clean, **264 Rust** and
**191 frontend** tests passing, `npm run build` clean, and `npm run tauri build`
produces a working `.app` + `.dmg`.

## Not verified, and worth a human's eyes

- The console still cannot be captured from the Tauri webview here. Everything
  above was rendered in a browser, where there is no backend — so populated
  states (a real plan, real channels, a live verse) were not seen.
## Pass 2b — the panic bar, and two more amber leaks

Found by reading the pass-2 screenshots rather than the code.

**The panic bar was covering the app.** `position:fixed; top:0` meant it sat on
the first ~56px: the sidebar brand, and part of the top bar — including the On
Air badge and the name of what is currently on the wall. That readout is the
single thing an operator most needs at the exact moment this bar appears, and the
bar was on top of it.

The shell now offsets by the bar's **measured** height (`bind:clientHeight` →
`--panic-h` → `.shell.has-panic`), not a constant: the message wraps to two or
three lines on a narrow window, so a fixed offset would either clip it or leave a
gap. No transition — this is an emergency message, and sliding the console for
200ms while someone reads "the screens may still be live" is motion for its own
sake. Verified in the browser with the bar up: bar bottom 62px, brand top 79px.

**Two more chrome-amber leaks**, both invisible until rendered:

- `ModelSetup.svelte` — the "Relay can't hear the sermon yet" banner. A missing
  speech model is a degraded install, not something on a wall.
- `FirstRun.svelte` — the current-step indicator and the selected-monitor card.
  Selecting a monitor in a wizard puts nothing on it. (The *completed* step stays
  green: the design sheet's confirmed colour, which is correct.)

Amber now survives only in `Stage.svelte`, `App.svelte` (the On Air badge),
`OutputWall.svelte`, `TemplateRender.svelte` and `Live.svelte` — every one an
on-air surface.

Installed to `/Applications/Relay.app` and restarted the Dock (macOS caches icons
aggressively, and a stale icon would look exactly like the change not landing).
