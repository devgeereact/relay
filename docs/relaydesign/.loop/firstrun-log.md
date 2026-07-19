# First Run — design loop log

Reference: `docs/relaydesign/relay-production-interface.png`, **panel 2 —
First-Run Setup Wizard**. Tokens: `relay-designsystem.png` v1.0.

**Compare method: PIXEL.** The wizard mounts over the shell and needs no backend
to render, so it was captured in headless Chromium at 1280×860, DPR 1,
`reducedMotion: reduce`, against the Vite dev server, stepping through every
stage with the real Continue button. Captures: `firstrun-{welcome,screen,audio,
model,language,finish}-3.png`.

Gate: `npm run build` clean, **192 frontend** tests (was 191 — one added),
**264 Rust**, `cargo fmt --check` + `clippy -D warnings` clean.

---

## What the reference specifies, and what was built

The reference is a **vertical step rail** on the left, a single content pane on
the right, and Back / Continue at the bottom. Its rail reads
Welcome · Audio Input · Model Download · Language · Finish.

The wizard before this was a **horizontal three-step strip** (Screen ·
Microphone · Try it) in a 560px dialog. It is now the reference's shape: an
860px two-column dialog, numbered rail, amethyst disc on the current step, green
tick on the completed ones.

## Two deliberate deviations

**1. A SCREEN step was added — the reference has none.**

Without it the wizard never does the one thing it exists for. The audit's exit
criterion is *"a volunteer who has never seen a terminal gets a verse on a
projector in under 10 minutes"*, and choosing which display the congregation sees
is the only step that produces something a congregation can actually see. It is
second, before anything about audio, for that reason.

**2. Language is SINGLE-choice; the reference draws a checkbox list.**

`set_stt_language` takes one language or `null` for auto — whisper cannot listen
for four languages at once. A multi-select would be a control that cannot do what
it appears to offer, on the screen where an operator forms their first model of
how Relay works. Rendered as a radio group with the same visual weight as the
reference's list.

Auto-detect is the default and is labelled the recommendation, because
code-switching mid-sentence is the normal case here, not an edge case
(CLAUDE.md). The step also carries the honest caveat that accuracy in Yorùbá,
Kiswahili and Hausa **has not been formally measured** — `docs/LANGUAGES.md` says
so plainly and the wizard must not imply more than the repo does.

## Bugs found by looking at the render

**A raw JS `TypeError` was on the first screen a new operator ever sees.**
The wizard did `error = String(e)`, so the Audio step displayed
`Cannot read properties of undefined (reading 'invoke')` — the Tauri bridge's
"there is no engine behind this page" failure — verbatim. CLAUDE.md is explicit
that `errors.js` is the ONE humaniser and a volunteer never sees a raw error.

Fixed twice over: every `String(e)` in the wizard now goes through
`humanError()`, **and** `errors.js` learned the no-engine case, which it had no
pattern for and was falling back to `"That didn't work: TypeError…"`. It now
reads:

> Relay's engine is not running, so nothing on this screen can work yet. If you
> opened this in a web browser, use the Relay app instead — the browser page has
> no engine behind it.

Pinned by a test that fails if `TypeError` or `undefined` reaches the operator.

**The microphone no longer runs across unrelated steps.** It starts when the
Audio step opens and stops when it closes (`go()`), rather than staying live for
the rest of the wizard. Detection stays disarmed for the duration, as before — a
hot mic plus an installed model could otherwise auto-fire a detected verse onto
the projector the operator was just taught to open, while they say "testing" into
it.

## Which of the 13 listed screens this covers

`relayscreens.md` §2 lists thirteen; the count table says nine. Built as six
steps in one wizard, because a step that asks nothing does not need its own
screen:

| Listed | Where it went |
|---|---|
| Welcome | step 1 |
| Output Detection | step 2 (Screen) — real `list_monitors` |
| Audio Setup · Audio Calibration | step 3 — device picker + live segmented meter |
| STT Download | step 4 — the real resumable, checksummed download |
| GPU Detection | a line on step 4, from the real `system_hardware` probe. It is a **build** fact ("this build runs whisper on the CPU"), not a hardware boast — see the §1 log |
| Language Setup | step 5 |
| Keyboard Shortcuts · Finish Wizard | step 6 — the proof verse, then Esc / B / Space / ? |
| Test Recognition | **folded into step 6.** The proof fires a real verse to the real screen. A separate "say something and see if it detects" step needs a model, a microphone and a person willing to preach at a laptop; the manual fire proves the output path without any of them |
| **OBS Connection Setup** | **not built** |
| **ATEM Discovery** | **not built** |
| **ProPresenter Connection** | **not built** |

The last three are not built **because Relay cannot do them**. It implements
neither the OBS WebSocket protocol nor ATEM's, and ProPresenter support is
file import only (see the §1 Integrations screen, which says exactly this). A
wizard step called "OBS Connection Setup" would be a setup flow for a connection
that does not exist. If control channels are ever implemented, these become real
screens; until then they would be the most convincing lie in the product.

## Still off / not verified

- **Never seen inside the Tauri window.** Captured standalone in a browser, where
  there is no backend — so the monitor list, the level meter, the model download
  and the proof verse were all rendered in their *empty* or *failed* state. The
  populated wizard has not been seen by anyone.
- The reference's exact rail proportions were matched by eye against the panel
  crop, not measured — the panel is ~340px wide in the contact sheet.
- The reference shows a Shure MV7 and a half-complete 2.48 GB download; those are
  mock values in the mockup, not states this build was posed into.
