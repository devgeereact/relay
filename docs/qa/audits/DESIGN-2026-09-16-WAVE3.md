# DESIGN-2026-09-16-WAVE3 — the wave 3 browser-driven pass

**Six scenarios, driven rather than read.** Wave 3 merged five tracks into
`feat/wave3-timers` (Track A the registry, Track B publication and retention, Track C the
`DECISIONS.md` §27 reversal, Track D the warning threshold, Track E the eleventh content
kind and the operator's surface). Both suites are green on that tree, and the suites are
blind to the class of defect that matters here: a timer is a moving render on three
surfaces, and `qa-inventory` reported **0 handlerless buttons, 0 unnamed controls and 0
unaddressed commands** throughout this pass — as it did through the two passes that found
this project's worst defects, and it was right every time.

Two things in this wave had never been in a layout engine at all, and both are named by
the agents that wrote them: the stage page's `.progrow` sizing
(`92cqw / --tmrs / 6 / 0.62`, capped at `9cqh`) and Live's programme timer band's wrap
behaviour at narrow widths. **Both were measured here, and there is a finding in each.**

This is **frozen evidence and findings only.** Nothing here records a fix. Ten findings
are filed in `docs/qa/RELAY_GAP.md` as RG-146 … RG-155 and are not restated anywhere else.

---

## 0. Method, and the line between two different claims

| | |
|---|---|
| Date | 2026-09-16 |
| Tree | `feat/wave3-track-f`, branched from `feat/wave3-timers` at `1a242d8` (all five tracks merged, plus Track C's own follow-up fix). Working tree carried only the temporary audit scaffolding described below |
| Backend | `src-tauri/target/debug/relay`, built from that tree and run through `npx tauri dev --no-watch` with a config overlay pointing `devUrl` at a Vite server on **:5133** — port 5032 was held by another checkout of this repository, and pointing the webview at it would have run a different tree's console against this backend |
| Profile | **The developer's existing dev database**, `~/Library/Application Support/com.relay.app/relay.db` — **not** a fresh install. The wave 2 pass used `HOME=/tmp/relayfresh`; that is not available to this agent, and the difference matters for exactly one thing below (the default template resolved to `Ember · Scripture`, id 24, which is this machine's configured default and not a seed value). **The timer registry is process memory with no database behind it at all**, so every timer measurement here is independent of the profile. Startup line: `profile: active 'Default' · lang Some("en") · sensitivity 0` |
| Frontend | Vite on `:5133`; the embedded HTTP server on `:8032`; the kiosk hub on `:8031` |
| Browser | gstack's headless driver (`~/.claude/skills/gstack/browse/dist/browse`). This machine cannot screenshot the Tauri window, which is why all visual work happens in a browser |
| Not run | **`prefers-reduced-motion` was never actually flipped.** The driver has no media-emulation command, so §5's reduced-motion half is read out of the live CSSOM and is NOT a claim that the page was rendered under that setting. It is marked PARTIAL there and nothing here should be read as more |
| Not run | **The dock's own countdown warning** (`Dock.svelte:589`). Reaching it needs a live countdown through the console's event path, and the console's events were not relayed. Marked NOT TESTED in §5 |

**Three different claims, and they must not be blurred.**

* **The output and stage pages were measured against the REAL backend.**
  `http://127.0.0.1:8032/output.html` and `/stage.html` are served by Relay itself, they
  join the real kiosk hub on `:8031`, and every timer, fire and panic control below was
  performed by the real Tauri commands. Nothing on this surface is mocked.
* **The hub wire was read directly.** A plain WebSocket client on `:8031` recorded every
  frame with a timestamp, exactly as an OBS browser source or a stage tablet receives it.
  `channels::kiosk_origin_allowed` returns `true` for an absent `Origin`, so the recorder
  needed no special case. **Where this document quotes a frame, that is the bytes that
  left the machine**, not an inference from the code.
* **The console at `:5133` was rendered in a browser, where it has no backend of its
  own.** It was given a MOCK one: a fixed dispatch table of canned answers, installed by
  hand after the page had loaded. **Nothing the console did in this pass reached Relay.**
  A console reading below is a claim about LAYOUT and never about behaviour.

**The scaffolding is temporary and is not part of the product.** Two files —
`src/lib/__auditbridge.js` (a FIXED scenario of real commands, run once on load inside the
Tauri webview, taking no instructions from anywhere and emitting only an outbound step log)
and `src/lib/__auditmock.js` (the console's fake bridge) — plus one line in `src/main.js`
and a `tauri.audit.conf.json` overlay. All four were removed before the suites were run and
before anything was committed. Same shape and the same precedent as the wave 2 pass.

**Measurement, not eyeballing.** Geometry is `getBoundingClientRect`; text width is a
`Range` over the text node compared against the box; overflow is
`scrollWidth`/`scrollHeight` against `clientWidth`/`clientHeight`; *coverage* is
`elementFromPoint` over a control's own centre. Screenshots are corroboration, never the
measurement. One trap this pass hit and names so the next reader does not: on the stage
row `.tval` is a block filling its flex item, so **`scrollWidth === clientWidth` is true
while the text inside it is clipped** — the same artefact the browser harness note records
about a truncated `<select>`. Every clipping claim below is a `Range`.

---

## 1. The re-aim after a verse — Track A's whole reason for existing — PASSES

**The defect this wave exists to fix.** Read off the wire, `EARLY` client, one run:

| at | what the hub received | |
|---|---|---|
| 54.91 s | `content` · `content_kind: countdown` · `countdown_to: 1789590877514` · `countdown_from: 1789590577514` · `template_id: 24` · `template_pinned: false` | a five-minute congregation countdown, span 300 000 ms |
| 57.69 s | `content` · `content_kind: scripture` · `John 3:16` | the verse replaces it on the wall |
| 60.36 s | — | `adjust_countdown{remaining_ms: 600000}` returned **`ok`**. **NOTHING WAS PUBLISHED.** There is no frame between 57.69 and 63.08 |
| 62.40 s | — | `list_timers` shows the `both` timer at `target_ms 1789591182943`, `from_ms 1789590577514` |
| 63.08 s | `content` · `content_kind: countdown` · `countdown_to: **1789591182943**` · `countdown_from: 1789590577514` | `show_timer` — the explicit way back, carrying the **adjusted** figure |

Every clause of the ruling in the plan's item 8 holds, measured rather than argued:

* The re-aim **succeeds** where it used to answer *"Nothing is counting down."*
* It **changes the registry** — the target moved by 605 429 ms from the original aim.
* It **paints nothing**: the wall count does not move and `John 3:16` is still the last
  thing on it.
* `show_timer` is the way back and it carries the adjusted figure, not the original five
  minutes.
* `countdown_from` is **not re-stamped** — same value in both countdown frames — so the
  warning span stays the countdown's own length.
* The template triple rides over verbatim (`template_id: 24`, `template_json: null`,
  `template_pinned: false`) on both, so nothing was silently re-skinned.

---

## 2. A programme timer under traffic — Track B — PASSES

A `Stage` timer was started and then a verse, a song and a notice were fired over it:

```
65.64  timer    timers:[{id:7,label:"Sermon",countdown_to:1789592088235,…}]
67.32  content  content_kind:"scripture"
68.85  content  content_kind:"song"
70.44  content  content_kind:"announce"
72.13  list_timers → both the `both` timer (id 6) and the `stage` timer (id 7) still there
```

**One `timer` frame and no others.** The set is published when it changes and at no other
time, so a stage tablet holds the row it has and the figure ticks locally. **It does not
blink**, and the mechanism is that there is nothing to blink: no frame arrives.

The congregation timer survived the verse, the song and the notice too — which is §1's
guarantee seen from the registry side rather than the wire side.

---

## 3. `Esc`, then `B` — Track C — PASSES on the congregation guarantee, and §7 is the exception

| at | what happened | |
|---|---|---|
| 72.95 s | `content` · countdown `Back in` | a congregation countdown is put up |
| 75.07 s | `{"kind":"clear"}` | `Esc` |
| 76.71 s | `list_timers` → **only** id 7 `Sermon` (`scope: stage`) | the congregation timer is gone; the programme timer stays |
| 77.40 s | `{"kind":"black"}` | `B` |
| 79.02 s | `list_timers` → still only id 7 | the programme timer survives the harsher control too |
| 79.58 s | `adjust_countdown` → **`Err("Nothing is counting down.")`** | the transport still cannot bring back what a panic control took |

So DECISIONS §89 holds on the wire, in both directions: everything a congregation can see
goes on both controls, the preacher's clock does not, and the refusal string is still true
in the only case that can now produce it.

**The word to the preacher.** §89's last part decides that a stage `alert` comes down with
the screens. That was **not** reached from this pass — no `stage_alert` was sent in the
recorded run, and the console mirror of it (RG-145, closed by `1a242d8`) is a console
claim which this pass did not drive. **Not tested.**

**What §7 found instead is the other half of the same sentence**, and it is a finding
rather than a pass: `B` does not take the preacher's *up next* down, while `Esc` does.

---

## 4. A stage tablet that joins mid-service — Track B — PASSES, in the stated order

A reading was put up, then a second WebSocket client connected and sent
`{"kind":"hello","template_id":null}` — the shape `Output.svelte` sends for a
channel-keyed URL. Three frames came back, in this order:

```
{"kind":"default_template","template":{…}}
{"kind":"timer","timers":[{"id":7,"label":"Sermon","countdown_to":1789592088235,…}]}
{"kind":"content","content_kind":"scripture",…}          ← the reading, LAST
```

**Configuration, then the timers, then the screen frame.** The reading is painted last, so
there is no moment at which a clock stands over the verse. Rule 43's trap 1 holds from the
other side too: no `timer` frame ever replaced the retained screen frame, and the
`stage_next` frames published in the same run were never retained.

Rendered rather than only read: the stage page was reloaded against the running backend
and came back with both the programme row and the reading on it.

---

## 5. The warning threshold on all three surfaces — MIXED

### 5a. The wall — PASSES

A congregation countdown with a two-minute span, sampled at `0:10` remaining
(`output.html?channel=1`, 1920×1080, real backend):

```
.countdown  class="lfit countdown svelte-1loewgc warn"
            color: rgb(244, 81, 91)
            animation-name: svelte-1loewgc-cdwarn
            font-size: 199.68px      box 1651.2 × 209.7
```

The default rule's short-countdown branch is the one in force — a tenth of a 120 s span is
a 12 s window — and it came on when it should.

### 5b. Reduced motion — PARTIAL, and the reason is stated

The CSSOM on the live page carries **both** branches, read out of `document.styleSheets`:

```
@media (prefers-reduced-motion: no-preference) { .countdown.warn { animation: 2s … cdwarn } }
@media (prefers-reduced-motion: reduce)        { .countdown.warn { text-shadow: rgba(244,81,91,.85) 0 0 0.25em } }
```

The colour is applied inline either way, so the warning is visible under both settings.
**The page was never rendered under an actual reduced-motion setting** — the driver cannot
flip the media feature — so this is a claim about the rules the engine has parsed, not
about a render. Track D Task 13 said *assert this, do not rebuild it*; this is the assert,
and it is one step short of the rendered proof.

The dock's warning is colour only, with no reduced-motion glow, **deliberately**, and its
own comment says so. That is not reported here as a defect.

### 5c. The stage — FAILS, and nothing warns at all

`warn_ms` **does** ride the wire. Read off the hub:

```
{"kind":"timer","timers":[…,{"id":9,"label":"Wrap up","countdown_to":…,"warn_ms":30000}]}
```

Nothing reads it. `Stage.svelte`'s `programme` mapping takes `id`, `label` and
`countdownRemainingMs` and stops there, and `.progrow` was measured with
`querySelectorAll('.warn').length === 0` at every viewport tried, including with a timer
sitting inside its own explicit 30 s window and with an expired one at `0:00`. The comment
at `Stage.svelte:257` still says the threshold is *"wave 3 Track D's, on all three surfaces
at once"* and *"deliberately not read here yet"*. Track D has landed. Filed as **RG-148**.

### 5d. The threshold a person chooses reaches no congregation screen and no stage — FAILS

Three separate breaks in one chain, each checked rather than assumed:

* **No call site passes the argument.** `countdownWarning(remainingMs, totalMs, warnMs)`
  gained its third parameter in Track D, and all three readers call it with two —
  `TemplateRender.svelte:1145`, `Stage.svelte:220`, `Dock.svelte:589`.
* **A `Both` timer has no field to carry one.** `start_countdown` hard-codes
  `warn_ms: None` (`main.rs:2867`), and neither `timers::project_both` nor
  `main::countdown_content` has a warn field at all, so the congregation wire form cannot
  express a chosen threshold even if a surface read one.
* **The Settings default is console-only, and this is the bundle-level half.**
  `layers.js`'s `warnDefaultMs` is module state set by `setCountdownWarnDefault`, whose one
  caller is `stores/capture.js`. `output.html` and `stage.html` import the shared
  `layers-*.js` chunk but **not** `capture.js`, and the settings key proves it: after
  `npm run build`, `countdown.warn_ms` appears **once** in `assets/main-*.js` and **zero
  times** in `assets/output-*.js` and `assets/stage-*.js`. So an operator who sets
  `Settings → General → Countdown warning` changes the console and changes neither the wall
  nor the preacher's page, which keep the shipped 60 s.

`ServicePlanner.addCountdownCue` gained no per-cue field either (`grep -c warn` over that
file returns matches only for unrelated stale-arrangement wording). Filed as **RG-149**.

---

## 6. A rehearsal — Track B — PASSES on the tablet, FAILS on what it leaves behind

With `set_rehearsal(true)`, a `Stage` timer was started and a verse fired. The backend
printed both refusals and **the hub received nothing at all**:

```
rehearsal: publish_timers SUPPRESSED — nothing left the machine
rehearsal: broadcast SUPPRESSED — nothing left the machine
```

No `timer` frame and no `content` frame reached either client, watched at the hub itself
rather than at a Tauri event — which is the only assertion surface that can see this, per
the rehearsal bug `stage_next` carried for months.

**What the rehearsal leaves behind is the finding.** The timer created inside it
(`Rehearsal only`) is still in the registry after `set_rehearsal(false)`, and ending the
rehearsal published `clear` and `stage_next` and **no `timer` frame**. So the stage
tablet's set stayed at `[7, 9]` while the registry held `[7, 9, 11]`, and the rehearsal's
timer will appear on the preacher's monitor, unannounced, at the next unrelated publish.
Filed as **RG-150**.

---

## 7. FINDING — `B` leaves the preacher's "up next" standing, and `Esc` takes it down

Driven deliberately with **no clear before it**, because the console's own
*"the up next must not outlive the content"* watcher keys off `live` going falsy and a
preceding clear would already have taken that edge:

```
27.13  content     content_kind:"scripture"   (Romans 8:28)
28.55  stage_next  {"label":"Up next","text":"Psalm 23"}
30.18  black       {"kind":"black"}            ← and NOTHING else
32.75  clear       {"kind":"clear"}
32.75  stage_next  {"label":null,"text":null}  ← only now does the word come down
```

The root is one line apart from its twin, in `stores/capture.js:507-508`:

```js
await listen('output://clear', () => { live.set(null); screenBlack.set(false); leavePlan(); … });
await listen('output://black', () => {                 screenBlack.set(true);  leavePlan(); … });
```

`output://clear` clears `live`; `output://black` does not. `Live.svelte`'s subscription
fires on the truthy→falsy edge of `live`, so a blackout never reaches it and
`setStageNext(null, null)` is never called. The comment directly above those two lines
documents this exact defect being fixed **for the clear**, in detail, and the black
listener has it unfixed underneath — the repository's signature shape, a guarantee kept on
one door and skipped on its twin.

It is also §89's own quoted rule, one control along: *"the harsher control must never do
less than the milder one… an operator who has just hit the emergency key cannot be asked to
remember that it reaches three screens out of four."*

A second consequence, not measured as a rendered defect but following from the same line:
`$live` stays populated after a blackout, so any console surface reading `$live` as *what
is on the screens* is wrong after `B`. Filed as **RG-151**.

---

## 8. FINDING — a programme timer past one hour is clipped on the preacher's screen

The `.progrow` sizing had never been in a layout engine. Six programme timers on the real
backend, one of them 95 minutes so `formatCountdown` grows to `h:mm:ss`:

| viewport | `.tval` font-size | text (`Range`) | box | clipped |
|---|---|---|---|---|
| 1920 × 1080 | 64 px (the clamp cap) | 268.8 px | 305.7 px | none |
| 1280 × 720 | 51.276 px | **215.3 px** | **199 px** | **16.3 px** |
| 1024 × 768 | 40.724 px | **171.0 px** | **156.3 px** | **15.0 px** |

`.tmr` is `overflow: hidden`, and `t.scrollWidth - t.clientWidth` reports `16` at 1280.
The screenshot is unambiguous: the row reads **`1:30:1`** with the final digit sliced
through, running straight into its neighbour's `0:00` with no gap, so the two figures read
as one number.

The arithmetic: `.tval` is
`min(clamp(16px, calc(92cqw / var(--tmrs) / 6 / 0.62), 64px), 9cqh)`. The `/ 6` is a
six-character budget and `formatCountdown` emits **seven** past an hour. The measured mono
advance is **0.600**, so a seven-character value is `7 × 0.6 / (6 × 0.62) = 1.129` of the
box — it overflows by about 12.9 % **whenever the formula rather than the 64 px cap is in
force**, which is any width below roughly `259 × timers` CSS pixels. Six timers therefore
clip on anything narrower than about 1554 px: a 1280 or a 1024 tablet, which is what a
stage screen usually is.

A ninety-five-minute clock is an ordinary thing for a church to set — a livestream, a
whole service. Filed as **RG-147**.

Two things measured alongside it and **not** defects: the long label ellipsises exactly as
its comment says (`scrollWidth 1014` in a `clientWidth 199` box, `text-overflow: ellipsis`),
and a label-less timer renders digits alone with no collapsed box.

---

## 9. FINDING — the height cap measures the frame, which is the thing its comment says it replaced

The same declaration's `min(…, 9cqh)`. `.progrow` declares `container-type: inline-size`,
which establishes an **inline-axis** container only, so `cqh` inside it has no eligible
container and falls through to the viewport. Decisive measurement, at 1920 × 500 with three
timers:

```
row height           74.4 px      → 9 % of the row would be 6.70 px
viewport height      500 px       → 9 % of the viewport is  45.00 px
.tval font-size                                            45.00 px   ← the viewport
```

The comment above it reads *"Its own container, so the digits are a share of THIS row and
not of the frame — … the bug that rule replaces is a figure that looked right at one width
and overflowed at every other."* The `cqw` half is true and resolves against the row; the
`cqh` half — the one that is supposed to stop the digits outgrowing the row's height — does
not.

**What this does NOT cause, stated so the row is not read as worse than it is.** No
clipping was produced by it at any viewport tried, including 844 × 390 and 1600 × 300,
because a cap that tracks the viewport shrinks with it. The defect is that the cap is not
the cap anybody wrote, so nothing bounds the digits against the row if the row's own height
ever changes. Filed as **RG-154**.

---

## 10. FINDING — the wave's own Start control is not clickable at its centre in the default window

Live's programme timer band, the second thing that had never been in a layout engine.
Measured on the console (mock bridge — **a layout claim, not a behaviour claim**) with six
programme timers:

| viewport | `elementFromPoint` over the centre of `Start timer` |
|---|---|
| 1920 × 1080 | the button |
| 1600 × 900 | the button |
| **1320 × 860** — `tauri.conf.json`'s own default window | **`.r-select.xpick.xdur`** |
| **1280 × 800** | **`.r-select.xpick.xdur`** |

At 1280 × 800 the geometry is exact: the button occupies `x 567.4–641.6, y 327–349`; the
transport column's transition-duration `<select>` occupies `x 562–648, y 335.2–357.2`. They
overlap over a 74 × 13.8 px region containing the button's centre and its bottom-right
corner. Only the button's top-left corner still hits the button. Both elements are
`position: static, z-index: auto`, so this is a plain layout collision and not a stacking
order anybody chose.

The cause is one level up and **pre-existing**: `.rack` is 320.2 px tall inside a `.con-top`
that is 268 px tall, and `.con-top` is `overflow: visible`, so the transport column paints
52.2 px past the bottom of the monitors row at 1280 and 36.4 px at 1320. What is new is
that the wave put an interactive control in the overflow. Filed as **RG-146**.

`Clear screens` was hit-tested at every viewport in this pass and returned itself every
time, including at 960 × 640 — rule 15 and rule 44 hold. **This is not a panic-control
finding.** It is the same shape one control along.

**The band's own claim, measured.** Its comment says it *"wraps rather than scrolling"* and
that this *"costs it one row of height and nothing else"*. With six timers it is **two**
lines at 1920 × 1080 (60 px) and **three** at 1320 × 860 and 1280 × 800 (92 px), and the
slide grid below it — *"the thing an operator picks from most often"* — falls from 381.6 px
at 1920 to **158 px** at 1280. The band never scrolls and never clips a chip, so the
mechanism is as described; the cost is not. Recorded inside RG-146 rather than as a row of
its own.

One thing checked and **not** filed: at 960 × 640, the smallest window
`tauri.conf.json` permits, the slide grid starts below the fold and the document does not
scroll. With the timer band emptied to zero chips the grid still starts below the fold
(618.2 px against a 640 px viewport), so that is the pre-existing Live layout at the
minimum size and not something this wave caused. The band makes it 34 px worse.

---

## 11. FINDING — an expired programme timer sits at `0:00` for the rest of the service

`done_msg` is carried on the timer frame (`"countdown_done": ""` in every row above) and
**nothing on the stage page reads it**. `programme` maps `id`, `label` and the remaining
milliseconds; a timer that reaches zero keeps `countdownRemainingMs` of `0`, survives the
`!= null` filter, and paints `0:00`. Observed on the real backend in both hold runs: a
45-second `Offering` timer read `0:00` for the following twelve minutes, and the screenshot
at 1280 × 720 shows two of the six rows in that state.

The congregation path has an answer for this — the `Both` wire form's `countdown_done`
replaces the digits, and it was seen working on the stage mirror in the same screenshot,
which reads `Welcome` where the congregation countdown had run out. The programme row has
none. Filed as **RG-153**.

---

## 12. The unreachable half of the wave — FINDING

`show_timer` and `adjust_timer` have wrappers in `stores/capture.js`, in the documented
THROWS group, held there by `timerwrappers.test.js`. **Neither has a rendered control.**
`grep` over `src/**/*.svelte` returns a single hit between the two names — a comment in
`views/Live.svelte` explaining that neither is reachable from there. By
CLAUDE.md's stricter test — *whether a rendered control can get there, not whether a
wrapper exists* — the wave's headline way back onto a congregation screen is
operator-unreachable.

`qa-inventory` reports **0 commands registered in Rust that no frontend caller addresses**
and is right: it traces to the wrapper, which is exactly one hop short of the question.

What an operator can actually do today, enumerated:

* **Start a programme timer** — yes. Live's band, `scope` hard-coded to `'stage'`.
* **See what is running** — yes, on the same band, with a real three-way answer
  (`Reading…` / `No programme timer.` / the reason, keeping the last good list).
* **Stop one** — yes, per chip.
* **Re-aim the congregation countdown** — yes, but only through the dock's transport
  (`adjust_countdown`), which addresses *the newest `Both` timer* and cannot name one.
* **Put a timer back on the congregation screens after a verse** — **no.** This is the
  action the plan's item 8 specifies as the way back, and it exists only as a command.
* **Start a `Both` timer with a chosen `warn_ms` or a `plan_item_id`** — **no.** Live
  passes `scope: 'stage'` and no `warnMs`, and `start_countdown` (the dock's one-press
  countdown) takes neither.

Filed as **RG-152**.

---

## 13. Site 7 of the content-kind sweep — checked on a real screen, and it holds

The exposure `layers.js` records is that a template carrying an explicit `layout.shows`
allow-list drops any kind not on it, silently, before `resolveOutputTemplate` is consulted.
The congregation timer is broadcast as `countdown`, which every seeded list already names,
so nothing needed touching — and this pass confirmed it on a screen rather than in the
source: `output.html?channel=1` resolved to the configured default, id 24 `Ember ·
Scripture`, a `theme_templates()` row whose seeded layout carries
`"shows":["scripture","song","media","announce","countdown"]`, and it **painted the
countdown** in every run in §1, §3 and §5.

The count in the comment is wrong, though, and it is wrong twice. `layers.js` calls
`db/templates.rs::theme_templates()` *"the thirteen on the preset shelf"* in two places;
it returns **twenty-five** (five families × five kinds, listed by name in the source). The
claim the comment exists to make is unaffected — all twenty-five do carry an explicit list
of exactly the five current kinds, pinned by `every_seeded_template_says_which_kinds_it_
renders`, whose own `all` array is a third hand-mirrored copy of `CONTENT_KINDS`. Filed
with one other stale count as **RG-155**.

---

## 14. What this pass does not change

The release decision, the model question and word error rate are all untouched by
everything above. None of the ten findings is a detection fault, and none was reached by
the Rust or frontend suites, which were green on this tree throughout:

```
Rust      814 passed; 0 failed; 16 ignored
frontend  2327 passed (2327), 151 files
e2e.rs     76 passed; 0 ignored
```

**Attribution, stated per finding rather than blanketed.** Two are wholly pre-existing and
the wave merely put something in front of them: **RG-146** (the `.rack` overflow predates
the band that now sits in it) and **RG-151** (`output://black` has never cleared `live`).
Seven are wholly wave 3's own, in code this wave wrote: **RG-147, RG-148, RG-149, RG-150,
RG-152, RG-153, RG-154**. **RG-155 is one of each** — its `layers.js` half is Track E's
comment, and its CLAUDE.md half was made false by wave 2 Track A's
`output://default_template`, so the count in CLAUDE.md's own reproducing command was
already one short before wave 3 began and the wave 2 pass did not catch it.

**Two of the seven are the wave contradicting its own plan** rather than getting code
wrong. Track D's Task 11 landed the third argument and nothing passes it; Task 12 landed
the Settings default and not the per-cue field. Track E's Task 15 landed a control for the
half of the registry that never touches a congregation screen, and the half that does has
none.

## 15. What this pass could not see

* **`prefers-reduced-motion` actually set.** §5b is the CSSOM, not a render.
* **The dock's countdown warning.** It needs a live countdown through the console's event
  path, which was not relayed.
* **`show_timer`'s refusal of a `Stage` timer.** Read in the source, never driven.
* **§89's stage `alert`.** No `stage_alert` was sent in the recorded runs.
* **A fresh install.** Everything here ran against an existing dev database (§0). Nothing
  about the timer registry depends on it, and the template resolution in §13 does.
* Anything that requires the native Tauri window's own pixels. The console was rendered in
  a browser; its *layout* under the packaged webview is not evidenced here.
* Behaviour on Windows. Every measurement is macOS, one machine, one build.
