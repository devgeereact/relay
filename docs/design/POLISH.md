# Repolish pass — design notes

Concrete polish applied to the new/edited surfaces, all against the
"Spiritual High-Tech" system (`src/app.css` `--v-*` tokens, `.r-*` components,
Playfair `--f-head` headings / Inter `--f-body` / JetBrains Mono `--f-mono`).
Retina + responsiveness are inherited from `app.css` (global font smoothing,
`@media` breakpoints, container-relative units).

## Typography (side-nav system, app-wide)
- Headings use `--f-head` (Playfair) — same face as the sidebar brand and the
  topbar title (`.topbar-title`). Section/label chrome uses `--f-mono`; body
  uses `--f-body`. Verified consistent across Library, Planner, Templates,
  Channels, and the new panes.

## Template Editor
- **Real fix:** the three panes used an undefined `.tile` class → they rendered
  as flat, borderless surfaces. Gave `.pane` proper card surfaces
  (`--v-surf` + `--v-line` + 14px radius).
- Selected template row now carries the app's amber left-accent bar (matches
  cue / plan / song selection everywhere else); cleaner hover.

## Output Channels
- Already fully on-system (`.r-tile`, `.r-row` accent bars, `--f-head` titles).
- Added the missing interaction cue: the "Add channel" dropzone lifts on hover.
- Channel delete: ✕ per row → confirm → delete → refresh (was already wired).

## Library
- **Cards are image-free** (lyrics have no artwork): dense, typographic — accent
  bar, badge + count, serif title, divider footer. Song / plan / media / saved-
  scripture cards share one radius (13px), gap (12px), and hover treatment.
- **Scripture search** now separates the **Best match** (proud amber card) from
  **Other places this appears** (suggestion list) — mirrors the requirement that
  a phrase/paraphrase surfaces the verse first, with suggestions beneath.
- **Import review** (pre-save) uses the same tokens: Playfair title inputs,
  mono tags, serif lyric textareas, accordion cards.

## Service Planner
- Run editor is the Mission-Control 3-pane layout (plan cues / slide flow /
  live output monitors) using the real `TemplateRender` + active templates.
- Transport bar: title left, status + Prev/Next/Clear grouped on the right; the
  live-status is a pulsing emerald pill; transient status text is width-capped so
  the transport buttons never shift.
- Add flow is a single search (scripture + songs) with grouped results.

## Output rendering (design intent)
- Live congregation screens show **no titles or slide numbers** — lyrics fire
  text-only; those details stay in the operator UI / confidence view.
- Lower-third **band is a citation device**: it renders only when there's a
  reference (scripture). Lyrics (no reference) render **centered full text**,
  keeping the one renderer content-agnostic.
