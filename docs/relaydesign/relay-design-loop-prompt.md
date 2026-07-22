# Relay — Design-Match Loop Prompt

Paste the block below into `/loop`. Swap `<SCREEN>` and `<REF>` per screen.

## Screens with a design reference

Every screen below has a mockup PNG to match pixel-for-pixel. The hi-fi PNGs are
full-screen mockups; the rest are numbered panels cropped from the master contact sheet
`docs/relaydesign/relay-production-interface.png` — match only that panel's region.

**A hi-fi mockup always supersedes its contact-sheet panel.** Several arrived after
this table was first written, so check `ls docs/relaydesign/*.png` before starting a
screen rather than trusting a panel reference here.

| `<SCREEN>` | `<REF>` | state |
|---|---|---|
| splash | `docs/relaydesign/relay-splash-screen.png` | **✅ done** |
| live | `docs/relaydesign/relay-console-screen.png` | **✅ done** |
| templates | `docs/relaydesign/relay-templetedesigner-screen.png` | **✅ done** |
| help | `docs/relaydesign/relay-helpandshortcut-screen.png` | **✅ done** |
| firstrun | `docs/relaydesign/relay-production-interface.png` (panel 2 — First-Run Setup Wizard) | **✅ done** |
| library | `docs/relaydesign/relay-main-library-screen.png` (hi-fi; supersedes panel 6) | **✅ done** — hi-fi arrived after; a re-compare is unclaimed |
| planner | `docs/relaydesign/relay-planner-screen.png` (hi-fi; supersedes panel 7) | **✅ done** |
| inspector | `docs/relaydesign/relay-production-interface.png` (panel 8 — AI Detection Detail / Inspector) | **✅ done** |
| channels | `docs/relaydesign/relay-channels-screen.png` (hi-fi; supersedes panel 4) | **✅ done** |
| template gallery | `docs/relaydesign/relay-templetes-screen.png` (hi-fi) | **✅ done** |
| template editor | `docs/relaydesign/relay-templeteeditor-screen.png` (hi-fi) | **✅ done** |
| output | `docs/relaydesign/relay-production-interface.png` (panel 9 — Output / Projector) | not started |
| stage | `docs/relaydesign/relay-production-interface.png` (panel 10 — Stage Display / Preacher View) | not started |
| settings | `docs/relaydesign/relay-production-interface.png` (panel 11 — Settings) | not started |
| history | `docs/relaydesign/relay-production-interface.png` (panel 12 — Service History) | not started |
| statusstrip | `docs/relaydesign/relay-production-interface.png` (panel 14 — Global Status strip) | not started |

---

| app shell (sidebar · tabs · top bar · footer) | `docs/relaydesign/relay-production-interface.png` (panel 3 chrome) |
| tokens only | `docs/relaydesign/relay-designsystem.png` |


## The prompt

ONE SCREEN AT A TIME.

PICK A SCREEN AND BUILD

Build the **<SCREEN>** screen so it visually matches `<REF>` as closely as possible.


### Ground rules (read before writing code)

1. Read `CLAUDE.md`. It is binding. Notably: this is **Tauri v2 + Svelte 4 + Vite**, no
   Tailwind — style with the `--v-*` CSS custom properties and `.r-*` classes already in
   `src/app.css`. Mode colors are law: **amber = ON AIR, amethyst = rehearsal, cyan = a
   guess** (a paraphrase shows no percentage). `src/lib/TemplateRender.svelte` is the ONE
   renderer for the fullscreen Output and the Templates preview. `src/lib/errors.js` is the
   ONE backend-error humaniser — no raw Rust `Err` strings on screen.
2. **Tokens before pixels.** The app currently ships an OLDER `--v-*` palette; the design
   system PNG is the target. Before touching any screen, read `<REF>` and extract its
   colors, spacing scale, radii, shadows, and type scale into `src/app.css` `:root` as named
   `--v-*` tokens. Every value you use afterwards must be a token — no raw hex, no arbitrary
   `padding:13px`. If the design system PNG does not define a value the screen needs, add the
   token and note it in the log (below) as an inferred value. When you change a shared token,
   grep every consumer so the Output window (`output.js`) and Stage (`stage.js`) move with it.
3. **You may run the app for this task.** The usual "don't start the app" rule is lifted
   here — you need pixels. Reuse an already-running `npm run tauri dev` if one exists; do not
   boot a second. The operator console has **no backend in a plain browser** — the Output and
   Stage pages, however, render standalone at `http://<host>:8032/output.html?template_id=<n>`.

### Match target

Layout · spacing · typography (family, size, weight, line height, letter spacing) ·
colors and mode semantics · button styles and heights · input fields · border radius ·
shadows and elevation · icons (2px stroke, rounded caps) · imagery and its cropping ·
alignment and padding · the sidebar / top-bar / footer chrome · visual hierarchy.

Do not redesign or improvise. If the reference is ambiguous or something is
missing from it, implement the closest reasonable thing **and log it** rather than
inventing a different layout.

### The loop

Each iteration:

1. Implement / refine the screen.
2. `npm run build` (Vite — catches Svelte compile errors) and `npx vitest run` for any
   touched `*.test.js` — both must be clean before you screenshot. A compile error means the
   iteration is not done. If the change is CSP- or bundle-sensitive, note that only
   `npm run tauri build` truly exercises it.
3. Screenshot the running app into `docs/relaydesign/.loop/<SCREEN>-<N>.png`
   (`<N>` = iteration number, starting at 1. Create `docs/relaydesign/.loop/` if absent):
   - **Output / Stage / any page servable on :8032** — load it in a headless browser and
     capture there.
   - **Operator console screens** — capture from the Tauri webview (or an external machine
     that can). **If this machine cannot capture the console window** (per `CLAUDE.md` it
     can't), say so plainly, mark the iteration "unrendered — code-level match only," and do
     the compare against the component markup + computed `--v-*` values. **Never claim a
     screen renders when you did not see it.**
4. **Read your own screenshot back** with the Read tool, side by side with
   `<REF>`. Do not trust the code — trust the pixels.
5. Write the diffs to `docs/relaydesign/.loop/<SCREEN>-log.md`, appending a section per
   iteration: what differed, what you changed, what is still off, what you
   deliberately inferred, and whether the compare was pixel or code-level. Read this log at
   the start of every iteration so you do not re-fix the same thing or oscillate between two
   wrong values.
6. Repeat.

Be strict. Look for: text baseline and vertical centering, button height and
horizontal padding, gap between stacked elements, corner radius (4 vs 8 vs 12 is
visible), shadow spread and opacity, icon weight and size, image crop and aspect,
exact font weight (500 vs 600 is visible), tabular-nums on timestamps/levels, and color
accuracy (sample the hex from both images, do not eyeball it) — and confirm the mode colors
are exactly the semantic tokens (amber on-air, amethyst rehearsal, cyan guess, green
confirmed, red panic, grey cued).

### Stop conditions — stop when ANY of these is true

- The screenshot and the reference are indistinguishable at a glance, and the last
  two iterations produced no new fixable diffs.
- You have completed **8 iterations**.
- The remaining diffs are all things you cannot fix from code (e.g. the reference
  uses an asset you do not have, or a font not in the project), or the screen is
  console-only and could not be rendered on this machine.

On stop, output: a short list of what still differs and why, plus every value you
inferred rather than read from the design system. If you could not run the
app, say so plainly — do not claim the screen renders.
