# Stage Displays — design-match log

References:
- `docs/relaydesign/relay-stagedisplay-screen.png` (gallery, hi-fi)
- `docs/relaydesign/relay-stagedisplayeditor-screen.png` (editor, hi-fi)
- ProPresenter stage-display editor screenshots (user-supplied) — layers, objects
  palette, ruled canvas, layouts strip, per-object inspector.
- `docs/relaydesign/relay-designsystem.png` (tokens).

Screen: operator console → **cannot be captured on this machine** (CLAUDE.md). All
compares are **code-level** (markup + `--v-*` tokens vs reference pixels), not a
rendered screenshot.

## What was built

A new **Stage Displays** tab (`src/lib/views/StageDisplays.svelte`) with the same
gallery/editor split as Templates and Planner:

### Gallery — `stagedisplays/DisplayGallery.svelte` (screen 1)
- Toolbar: "All Displays" + live-count chip, search, Group, grid/list toggle, **Add Display**.
- Card grid seeded to mirror the reference: Stage Display (PRIMARY), Confidence Monitor,
  Preacher View, Musician View, Timer View, Countdown View, Remote Stage Display,
  Countdown Clock. Each card = icon + name + PRIMARY/LIVE badges, a **live preview**, a
  meta footer (res · FPS · 16:9), and gear / **Edit Layout** / delete controls.
- Previews are real: scripture displays render through the ONE renderer
  (`TemplateRender`) so they are WYSIWYG; timer / countdown / confidence / lyrics / clock
  displays draw a bespoke preview matching the reference (each of those is computed live,
  not a fired verse).
- Right **Display Settings** rail: General/Layout/Content/Advanced tabs; General has
  Display Name, Type, Status, Resolution, Refresh Rate, Connection, Colour Profile, a
  preview + VU meter, Quick Actions (Send Test Pattern / Black Screen / Clear Content /
  Restart Output), and Delete. Name/Type/Resolution/FPS/Colour/Status are wired to the
  store and persist.

### Editor — `stagedisplays/DisplayEditor.svelte` (screen 2)
Reuses the **existing** ProPresenter-style editing machinery (`layers.js` model +
`TemplateRender` + the template editor's drag/resize maths):
- **Layers** panel — typed layers with visibility eye, reorder, delete, Add Layer.
- **Objects** palette — Text, Rectangle, Circle, Line, Image, Icon, Countdown, Clock,
  Logo, QR Code, filterable (All/Text/Shape/Media). Each adds a real, editable layer.
- **Ruled canvas** — top + left rulers (0–2000), the artboard is the live renderer, with
  a selection overlay: drag to move, corner/edge handles, resize from the SE handle,
  live layer tag.
- **Layouts** strip — Custom + the four starters (`STARTERS`: Full-Screen, Lower Third,
  Announcement, Freestyle) with live thumbnails + New Layout.
- **Inspector** — per-layer: Content (static text / live binding + source), Typography
  (Font Family, **Weight**, Size, Line Height, Letter Spacing, Align, Colour), Appearance
  (Opacity, Text Shadow toggle + softness, Italic), and Position & Size in **pixels** on a
  1920×1080 canvas (drag or type). Shape and Background layers get their own inspectors.
- Top bar: Layout / Content / Advanced tabs + Cancel / Preview / **Save Changes** (enabled
  only when dirty). Content tab edits static text per layer; Advanced shows hardware meta.

### One renderer change (makes a control real, not decorative)
`TemplateRender.svelte` gained `font-weight:{L.weight||400}` on the text layer, so the
inspector's **Weight** control actually changes the type. One line; back-compatible
(defaults to 400, the previous fixed weight).

## Backing / architecture
Displays are a **local, offline-first** store (`stores/stagedisplays.js`, localStorage) —
CLAUDE.md's local-first rule. Each display OWNS a layer-model template; the editor edits a
deep clone and Save commits it back. No new Rust commands, no backend coupling: the whole
feature works with zero backend, and previews/edits go through the same template engine as
the wall. Wiring a display to a real output screen stays the explicit job of the Channels
tab (noted in-UI).

## Deliberate deviations / inferred (logged, not accidental)
1. **Objects the renderer can't natively draw** (Image, Icon, Logo, QR Code) are added as
   styled **placeholders** (shape/text) you position and restyle — the model renders
   text/shape/background/timer. They are real, editable layers, just not yet true image/QR
   objects. Logged rather than faked.
2. **Text Shadow** is a toggle + a single softness slider (the renderer's `shadow` scalar).
   The reference's separate Blur / Offset Y / Colour fields are collapsed into that one
   working control instead of shipping three inert inputs.
3. **Quick Actions** (Send Test Pattern / Black Screen / Clear Content / Restart Output)
   show a confirmation toast but do not yet drive a real output — they need the Channels
   output wiring. Kept visible per the reference; behaviour is honest (a toast, not a lie).
4. **VU meter / Connection "Connected"** in the settings rail are static indicators — there
   is no per-display audio/return-feed backend. Presentation only.
5. **Confidence / lyrics / timer previews** use representative sample content (92%, Amazing
   Grace, 00:32:45) matching the reference mock — these are gallery previews, not live feeds.
6. **Position & Size shows pixels** on a 1920×1080 canvas (the reference shows 320/1360/600);
   the underlying model is percent, converted both ways, so a layer sits identically at any
   output size.
7. **Sidebar**: added "Stage Displays" after Planner (the reference sidebar shows it there).
   History remains inside Settings per the earlier explicit instruction, so the sidebar is
   Dashboard · Live · Channels · Templates · Library · Planner · Stage Displays · Settings ·
   Help.
8. **Topbar chrome** (Outputs/Stage-Display status, clock) is the separate app-shell task;
   this screen owns only the content region.

## Verification
- `npm run build` — clean (only pre-existing a11y warnings in the Templates editor/gallery;
  none in the new files).
- `npx vitest run` — **300 passed**.
- Fixed before finalizing: a Svelte cyclical-reactive (`sel → selId`) in the gallery, and
  nested `<button>` in the card (card is now a role=button div).
- **Not rendered**: could not screenshot the console webview on this machine. No claim that
  it renders — a human on a capture-capable machine should drive it (drag a layer, change a
  weight/colour, apply a starter, Save, reopen) to confirm the editing loop end-to-end.

## Still off / for a human's eyes
- Real image/icon/logo/QR layer types (need renderer support + asset picking).
- Real output-driving Quick Actions + live VU / return feed.
- Exact ruler tick density and canvas padding vs the mock — reasonable, unconfirmed against
  pixels.
