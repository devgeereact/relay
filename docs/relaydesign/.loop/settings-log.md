# Settings — design-match log

Reference: `docs/relaydesign/relay-settings-screen.png` (hi-fi) + `relay-designsystem.png` (tokens).
Screen: operator console → **cannot be captured on this machine** (CLAUDE.md: no
screenshot path for the Tauri webview). Every compare below is **code-level** —
component markup + computed `--v-*` tokens against the reference pixels, not a
rendered screenshot. Marked plainly per the loop's honesty rule.

## Iteration 1 — full rebuild (code-level)

The old Settings was a flat 2-column card grid. The reference is a three-column
console: **section rail · active panel · overview rail**, with a page header. Rebuilt
`src/lib/views/Settings.svelte` to that structure; kept every real backend wiring.

### What now matches the reference
- **Page header** — "Settings" (H1, `--v-fs-h1`) + subtitle, top-left.
- **Left section rail** — 12 sections with 2px-stroke icons, amethyst-tinted active
  state (`--v-accent-soft` bg + `--v-accent-line` border + `--v-accent2` text), and a
  ghost **Reset to Defaults** button beneath, exactly as the ref.
- **Center panel** — section title + description, then the settings. General is built
  row-for-row against the ref: Application Language (dropdown), Theme (segmented
  Light/Dark/System), four toggle rows (Auto Start, Minimize to Tray, Confirm Before
  Going Live, Auto Save), Default Content Type, Time Format (segmented), Date Format,
  and a **Startup** group (Restore Previous Session, Default Startup Screen). Each row
  is its own bordered card floating on the page — no outer panel box — matching the ref.
- **Toggle switch** — 44×24 pill, white knob, `--v-accent-fill` when on. Segmented
  control uses `--v-accent-fill` for the selected segment with `--v-shadow-sm`, like the
  ref's Dark / 24-hour selection.
- **Right overview rail** — System Overview (Version, Environment, Licence, Uptime),
  Quick Links (Keyboard Shortcuts, Check for Updates, Service History, Support & Guide —
  each an icon + title + subtitle + chevron row), and a red-tinted **Danger Zone** card.
- **Colour law respected** — no amber anywhere (nothing here is on air); amethyst is the
  only accent; Environment/Licence use the design **green** (`--v-emerald`) badge for a
  positive/connected state, per the design sheet's usage guide.
- **Tokens** — every value is a `--v-*` token (radii `--v-r-md/lg`, type scale
  `--v-fs-h1/h2`, shadows `--v-shadow-sm`). No raw hex, no arbitrary px for colour.

### Real wiring preserved (not faked)
Language (`setLocale`), audio device + level meter + Start/Stop listening, detection
thresholds (auto-fire ≥ suggest invariant kept), Bible translations, per-content-type
templates, network/kiosk info + offline model (`ModelSetup`), crash reporting, safe
mode, setup walk-through, updater check, live session uptime.

### History moved into Settings (user request)
`History.svelte` is now rendered as the **Service History** section (self-contained:
its own list/detail/search/export, same local SQLite store). Removed it as a top-level
tab in `App.svelte`; the sidebar is back to the seven surfaces an operator *runs*.
All 300 frontend + nav/ipc tests pass with the tab gone.

## Deliberate deviations from the reference (logged, not accidental)
1. **Performance card omitted.** The ref right-rail has a Performance card (CPU/Memory/
   GPU/Disk with sparklines and hard percentages). Relay ships **no live-metrics
   backend**, so rendering "CPU 18%" + fake sparklines would be inventing data — exactly
   what CLAUDE.md forbids. Left out rather than faked. If a diagnostics backend lands,
   drop the card in here.
2. **Theme control is visual-only for Light/System.** The whole console is a single dark
   surface; there is no light stylesheet. The 3-way control persists the choice and
   stamps `data-theme` on `<html>` so a future light sheet can key off it, but picking
   Light/System does **not** repaint today. Dark is the real, styled state and is the
   default. Noted so it isn't mistaken for wired.
3. **General preference toggles (Auto Start, Minimize to Tray, Confirm Before Live, Auto
   Save, Default Content Type, Time/Date Format, Startup) persist to `localStorage`** but
   the OS-integration ones (Auto Start, Minimize to Tray) have no Tauri hook yet — stored
   as intent, applied when that integration ships. Language/thresholds/templates/safe
   mode are the fully-wired controls.
4. **Section set** — kept the ref's rail labels (General, Outputs, Audio, Scripture &
   Bible, AI & Detection, Shortcuts, Network, Backup & Recovery, Updates, Advanced,
   Account) and inserted **Service History**. Dropped "Integrations" (its content —
   OBS/kiosk/stage routing — already lives under Network + the Channels tab) to avoid an
   empty invented panel.
5. **Topbar chrome not in scope.** The ref screenshot includes the app shell (sidebar,
   top bar with "Outputs: 3 Online / Stage Display: Connected", footer status strip).
   That is the separate app-shell / status-strip task; this screen owns only the content
   region. The shell topbar still renders its own title/On-Air badge.

## Inferred values (not read from the design system)
- Toggle geometry (44×24, 18px knob, 20px travel) — the design sheet shows a toggle in
  the INPUTS block but not exact dimensions; sized to the ref's on-screen proportion.
- Segmented-control padding (4px frame, 7×14 segments) — inferred from the ref's Theme /
  Time Format controls.
- Quick-link row height and icon size (17px) — inferred from the ref's right rail.
- Uptime is this app-run's real elapsed time; the ref's "7d 14h 32m" is a mock value.

## Verification
- `npm run build` — clean.
- `npx vitest run` — **300 passed** (incl. `nav.test.js`, `ipc.test.js` with the History
  tab removed).
- Not rendered: could not screenshot the console webview on this machine. No claim is
  made that it renders — a human on a machine that can capture the Tauri window should
  eyeball populated states (real services in History, a live meter, a loaded model).

## Still off / for a human's eyes
- Exact vertical rhythm between non-row sections (audio/AI) vs the ref's General-only
  view — General is the only section the ref shows, so other sections are a reasonable
  extrapolation of the same card language, unconfirmed against a mock.
- Whether the section rail should scroll independently on a short window (currently the
  page scrolls as one) — needs a rendered check.
