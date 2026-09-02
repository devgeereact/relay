# Relay — Design System

The visual and interaction language: tokens, type, the load-bearing colour meanings, and the
component vocabulary. For the *why* behind any rule here follow the link into
[DECISIONS.md](DECISIONS.md); for how the pieces fit see [ARCHITECTURE.md](ARCHITECTURE.md);
for the entities being rendered see [DATA_MODEL.md](DATA_MODEL.md).

**The source of truth is [`src/app.css`](../src/app.css), not this page.** That file is the
shipped stylesheet and it carries the reasoning inline, at each token. This document is the map
and the rules a reader needs *before* opening it. If the two disagree, `app.css` is right and
this page is a bug.

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
| Styled by | `src/app.css` — one global stylesheet | **Themes → Templates**, resolved by `TemplateRender.svelte` |
| Who changes it | Only a developer | The operator, in the app, per screen |
| Units | `px` | **`cqw`** — so a template looks identical at any output size |
| Background | Always dark | Whatever the template says, **including transparent** (so it keys out in OBS) |

Never style an output surface from `app.css`, and never put a console token into a template.
The one file both share is `app.css` itself (the output window imports it for its reset), which
is why deleting a legacy rule from it is riskier than it looks — see §6.

---

## 1. Colour — the part that is safety-critical

**Four colours in this product carry a promise. Using one for decoration is a bug, not a taste
question.** They are enforced in code and pinned by tests, because a colour that lies to an
operator during a live service is the same class of failure as a control that lies
(DECISIONS §20, §21).

| Colour | Token | Means, and *only* means |
|---|---|---|
| 🟠 **Amber** `#ffb000` | `--v-amber` | **ON AIR.** The congregation is looking at this right now. |
| 🟣 **Amethyst** `#8b5cf6` | `--v-amethyst` | **Rehearsal** — and, separately, all interactive chrome (§1.1). |
| 🔵 **Cyan** `#22d3ee` | `--v-cyan` | **A guess.** A paraphrase / semantic match. Never a heard reference. |
| ⚫ **Grey** `#6b7280` | `--v-grey` | **CUED** — this is where `→` resumes, and it is **not** on screen. |

The rules that follow from that:

- **Amber is never used for "selected", "active", "primary" or "success".** It is the tally
  light. A colour that is always lit cannot also be a warning.
- **A cued position is grey, never amber.** `liveCue` is `{ cueId, slide, onAir }`, and position
  and on-air-ness are separate facts — panic keys clear only `onAir`. A cue that is where `→`
  resumes but is not on screen reads **CUED**, in grey (CLAUDE.md, frontend shape).
- **A paraphrase is cyan and shows no percentage at all.** A TF-IDF cosine is not a probability,
  and a number that lies is worse than no number (DECISIONS §21). It is never amethyst, because
  amethyst already promises "rehearsal — this cannot reach the congregation", and a colour
  carrying a promise cannot be borrowed for a hunch.
- **`detect.js` owns the mapping** from a detection method to the language the operator reads
  (`methodKey`); `Live.svelte` renders it. There are three distinguishable methods —
  `direct` (heard), `semantic` (a guess), `uncertain_book` (chapter and verse heard, the book
  not) — and they must stay visually distinguishable.

### 1.1 Why the interactive accent is amethyst, not amber

Every piece of interactive chrome — the selected nav item, a focus ring, a switch, a hover, a
primary button — points at `--v-accent`, which is amethyst.

It used to be amber, and that was the bug: spending the tally colour on "this tab is selected"
and on twenty ordinary Save buttons meant the loudest colour in the product was lit
permanently, everywhere, whether or not anything was on a wall.

Amber survives only as `.r-badge.amber`, `.r-btn.amber` and `.r-stat.amber` — the on-air cases,
named explicitly at the call site so reaching for one is a **decision** rather than a default.

Two amethysts exist, deliberately:

- `--v-accent` `#8b5cf6` — borders, dots, and text **on** a dark surface (the thing being read).
- `--v-accent-fill` `#7c3aed` — a fill **behind** white text. White on `#8b5cf6` is 4.22:1,
  under WCAG AA for 13px semibold. `#7c3aed` is 5.70:1 and reads as the same colour at a glance.

### 1.2 The neutral ramp

A 950→500 greyscale ramp. `--bg` is what the body **and the output-window canvas** paint, so it
is the deepest step.

| Token | Hex | Role |
|---|---|---|
| `--v-void` | `#0a0a0a` | Shell, main, output canvas |
| `--v-bg` | `#141414` | Sidebar, topbar, input fields |
| `--v-surf` | `#1b1b1b` | Default card |
| `--v-surf2` | `#262626` | Raised / selected card |
| `--v-surf3` | `#333333` | Hover, lightest chrome |
| `--v-500` | `#4d4d4d` | Hairline emphasis |
| `--v-line` / `--v-line2` | `rgba(255,255,255,.075)` / `.13` | Borders |

Text: `--v-txt` `#f2f2f2` · `--v-dim` `#b3b3b3` · `--v-faint` `#8c8c8c` · `--v-disabled` `#555`.

> **`--v-faint` is `#8c8c8c`, not `#8a8a8a`, and the two steps are load-bearing.** At
> `#8a8a8a` it was **4.38:1 on `--v-surf2`** — below WCAG AA — with five rules putting
> muted text on chips and badges there. `#8c8c8c` is 4.50:1 and looks identical.
> **Do not round it back.** `tokencontrast.test.js` measures every text token against
> every surface it is placed on and will fail (RG-74); the comment beside the token in
> `app.css` carries the full matrix. Muted is deliberately kept off `--v-surf3`
> (3.76:1) — that exclusion is asserted, not assumed.

**Every text token passes WCAG AA on every surface it sits on.** `--v-faint` was `#5f6470`
(2.27:1, a failure everywhere); it is now 4.54:1 at worst. Do not darken a text token without
re-checking it against `--v-void`, `--v-surf` **and** `--v-surf2`.

### 1.3 The remaining semantic colours

| Token | Hex | Means |
|---|---|---|
| `--v-emerald` | `#22c55e` | Confirmed / success / connected |
| `--v-red` (`--v-rose`) | `#ef4444` | Failure, destructive, the panic banner |

Badge classes exist for each (`.r-badge.{amber,cyan,rose,amethyst,grey,green}`), each a soft
fill + a 32%-alpha border + the colour as text. `.r-badge.pulse` adds a glow animation — reserve
it for live states.

---

## 2. Typography

**One UI family: Inter.** `--f-display` (formerly Space Grotesk) is *aliased* to Inter rather
than deleted, because `templates.js` and a couple of views still name it and a font in the
bundle for one preset is dead weight.

| Purpose | Token | Family |
|---|---|---|
| UI / body | `--f-body`, `--f-head` | Inter |
| Scripture (default template face) | `--f-serif` | Fraunces |
| Numbers, IDs, URLs | `--f-mono` | JetBrains Mono |

**Fonts are self-hosted via `@fontsource`, imported once in [`src/lib/fonts.js`](../src/lib/fonts.js)
and shared by every entry point (console + output windows). Never a CDN link, anywhere** —
offline-first is non-negotiable, and a `fonts.googleapis.com` link means a church with no
internet gets a fallback face on its projector.

### The scale

| Step | Size / line-height | Weight | Tracking |
|---|---|---|---|
| Display 1 | 56 / 64 | Bold | `-0.02em` |
| Display 2 | 40 / 48 | Bold | `-0.02em` |
| H1 | 28 / 36 | Semi Bold | `-0.02em` |
| H2 | 20 / 28 | Semi Bold | `-0.01em` |
| H3 | 16 / 24 | Medium | 0 |
| Body 1 | 14 / 20 | Regular | 0 |
| Body 2 | 13 / 18 | Regular | 0 |
| Label | 12 / 16 | Medium | `+0.02em` |
| Caption | 11 / 14 | Regular | `+0.02em` |
| Mono | 12 / 16 | Mono | 0 |

Tokens: `--v-fs-*` / `--v-lh-*`, tracking `--v-tr-tight` `--v-tr-h2` `--v-tr-wide`. The brand
lockup is the exception — 19px, weight 700, `letter-spacing: .18em`.

---

## 3. Space, radius, elevation

- **Spacing** — `--v-sp-xs` 4 · `sm` 8 · `md` 16 · `lg` 24 · `xl` 32 · `2xl` 48 · `3xl` 64 ·
  `4xl` 96 · `5xl` 128. Use the token; do not invent a 10 or an 18.
- **Radius** — `--v-r-sm` 4 · `md` 8 · `lg` 12 · `xl` 16 · `2xl` 24. Pills use `99px`.
- **Shadow** — `--v-shadow-sm/md/lg`, all pure black at 40–60% alpha. On a `#0a0a0a` ground,
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

- **Focus is always visible.** `outline: 2px solid var(--v-accent2)` with `2px` offset, on every
  interactive class (`.r-btn`, `.r-iconbtn`, `.nav-item`, `.r-input`, `.r-select`, `.r-switch`,
  `.r-focus`). Never remove an outline without replacing it with an equally visible one.
- **All five dialogs trap focus and restore it on close** (`src/lib/focus.js`, `use:trapFocus`).
  Restore is the half everyone forgets.
- **`Esc` must not clear the screens while a dialog is open.** `shortcuts.js` checks for a
  mounted `[role="dialog"]`. Dismissing a help overlay is not a live action (CLAUDE.md §16).
- **Motion is opt-in.** Decorative animation sits inside
  `@media (prefers-reduced-motion: no-preference)`, and the spinner explicitly stops under
  `reduce`. A new animation goes in the same guard.
- **The AI announces itself.** The suggestion feed, the transport, and errors all reach an
  `aria-live` region in `App.svelte` — the product's whole reason to exist used to arrive in
  total silence.
- **Heading structure is real**, starting at a single `<h1>`. Do not use a heading level for its
  size; use the type scale.

---

## 6. The legacy palette — why the dead CSS is still there

`app.css` opens with a legacy `:root` block. Every legacy colour name is now an **alias** of the
design-system token it maps to (`--amber` → `--v-amber`, and so on), so anything still on an old
name is on-brand by construction and each hex lives in exactly one place.

89 orphaned rules were deleted by checking every class name against every class a component
actually renders. **~150 lines remain, deliberately.** The reason is mechanical:
**Svelte does not scope a global stylesheet**, and the survivors use generic class names
(`.tab`, `.dot`, `.live`, `.chip`) that live components still carry. Deleting a rule whose class
is still on an element silently restyles the app, and verifying that needs eyes on a running
window — which the build machine cannot produce.

So the gun is unloaded rather than removed: the contrast failure is fixed, and the rules stay
until someone can look at a running app. Tracked in [KNOWN_ISSUES.md](KNOWN_ISSUES.md) §4.

---

## 7. Themes and templates — the output style layer

**Themes are the style layer beneath templates** (DECISIONS §27). The whole model is one line:

```js
{ ...theme.style, ...template.style }   // template wins, key by key
```

- A **theme** is a named bag of defaults for the exact same flat `style` keys `TemplateRender`
  already reads — typography, `accent`, `verseColor`, `refColor`, `background`, shadows,
  transition, `refGap`. The permitted list is `THEME_STYLE_KEYS` in
  [`src/lib/themes.js`](../src/lib/themes.js), and it is explicit so a theme can never smuggle
  in a key that changes an unrelated template.
- A **template** overrides the theme per key, and owns everything a theme may not touch —
  per-region overrides, background image, panel, layout.
- Resolving a theme produces **a normal template object**. It is not a new renderer, not a new
  content type, and there is no `if theme == …` anywhere. A themed template and a hand-styled
  one are indistinguishable downstream, which is what keeps WYSIWYG and *"outputs are render
  targets of one engine"* intact.
- Eight builtins ship (`BUILTIN_THEMES`): Modern Dark, Minimal, Light, Classic, Youth,
  Conference, Wedding, Livestream. Their ids are **negative** so they can never collide with a
  saved custom theme, and a template's `style.themeRef` is unambiguous. They live in the JS (not
  only the DB) so a kiosk or OBS client with no database can still resolve one.
- Layer colours may bind to a theme token (`theme:accent`) rather than a hex.

**`TemplateRender.svelte` is the ONE renderer** — the fullscreen output *and* the Templates
editor preview both use it, so the editor is WYSIWYG by construction. Stage displays and
confidence monitors are **render profiles of that same engine** (starters in `layers.js`), not a
parallel system: they add monitor-only fields (`next`, `note`, `elapsed`) that ride to output but
that no congregation template renders.

### Rules for anyone touching output rendering

1. **Sizes are `cqw`.** A template must look identical at 1280×720 and at 3840×2160.
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
| `.r-iconbtn` | Square icon-only button |
| `.r-badge` (+ colour) | Pill status chip; `.bd` is its 6px dot, `.pulse` adds a glow |
| `.r-input`, `.r-select`, `.r-switch` | Form controls |
| `.r-stat` (+ `.amber`) | A number-plus-label readout |
| `.r-scroll`, `.mainscroll` | Internally-scrolling panel (slim dark scrollbars) |
| `.r-focus` | Opt into the standard focus ring on a custom element |

**Three shared state components, and they are not interchangeable:** `EmptyState`, `Loading`,
`ErrorState`. *Empty* ≠ *loading* ≠ *error* — Live once said "No plans yet" before the database
had answered. `ErrorState` offers **Try again** only when the backend says the fault is
transient, which is the first place typed errors (`error.rs`) earn their keep.

**`src/lib/errors.js` is the ONE backend-error humaniser.** Never render a raw Rust `Err` string
to a volunteer — Channels did, in monospace, five times.

---

## 9. Reference material

Rendered screen references live in [`design/`](design/) as PNGs — the console, the live
production interface, Templates and its editor, Channels, Planner, Library, Settings, Stage
display and its editor, Help, the splash, and a design-system sheet. They are a **record of
intent**, not a spec: where a PNG and `app.css` disagree, the stylesheet shipped and the PNG did
not.
