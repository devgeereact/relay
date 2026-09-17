# 2026-09-17-WAVE4-STAGE-PLANNER — the wave 4 browser-driven pass

**Eight scenarios and one deferred legibility list, driven rather than read.** Wave 4
merged three tracks into `feat/wave4-stage-planner` — Track A gave the stage's programme
rail a warning state, a finished state, a held state, a zone switch and a floor; Track B
bound a plan cue to a clock and gave the run surface a slide sizer; Track C specified the
targeting the wave deliberately did not build. Both suites are green on that tree, and
both of wave 4's surfaces are moving renders that no static instrument in this repository
can see. The two congregation-facing defects the 2026-09-10 pass found — a first verse
sliced through the middle (rule 42) and a screen that reconnected coming back blank
(rule 43) — were invisible to every one of them, and `qa-inventory` reported zero
problems throughout and was right both times.

This is **frozen evidence and findings only.** Nothing here records a fix. Four findings
are filed in `docs/qa/RELAY_GAP.md` as RG-146 to RG-149 and are not restated anywhere
else.


> **RENUMBERED, NOT REWRITTEN.** The four findings this pass filed as RG-146 … RG-149
> are **RG-162 … RG-165** in the register, and the gap it filed as RG-145 is **RG-161**.
> Wave 3 had already filed RG-145 … RG-149 for entirely different findings from the same
> branch point, and the collision only became visible when the branches met. The map is
> `docs/qa/RELAY_GAP.md` §23a. **Every id below is left exactly as it was written on the
> day**, because a frozen document that follows a renumbering stops being evidence of
> what was found.


---

## 0. Method, and the line between two different claims

| | |
|---|---|
| Date | 2026-09-17 |
| Tree | `feat/wave4-track-d`, branched from `feat/wave4-stage-planner` at `89104fd` (tracks A, B and C merged). The working tree carried only the temporary harness described below, and carries none of it now |
| Backend | `src-tauri/target/debug/relay`, built from this tree on 2026-09-17 at 02:19. Startup line: `profile: active 'Default' · lang None · sensitivity 50` |
| Profile | **fresh install.** `HOME=/tmp/relayw4d`, so `db::app_data_dir()` resolved to an empty tree and Relay seeded from scratch |
| Frontend | `npx vite` on **`:5132`**, not `:5032` — see §0b. The embedded HTTP server on `:8032`; the kiosk hub on `:8031` |
| Browsers | gstack's headless driver (`~/.claude/skills/gstack/browse/dist/browse`) for the stage page, and Google Chrome 153 driven over CDP where the driver could not do what was needed — reduced motion, a recorder installed before first paint, and a second page open at the same time |
| Not driven | **the Templates and Library workspaces, the Planner's own cue inspector, and the output page's template editor.** This pass drove the stage page, the output page and Live. Track B's Planner-side control was exercised through its command (`set_plan_timer`) and read back out of SQLite, not clicked |

**Two different claims, and they must not be blurred.**

* **The stage page and the output page were measured against the REAL backend.**
  `http://127.0.0.1:8032/stage.html` and `output.html` are served by Relay itself, they
  join the real kiosk hub on `:8031`, and every timer, fire and panic control below was
  performed by the real Tauri commands against the real registry and the real SQLite
  file. Nothing on these two surfaces is mocked.
* **The console at `:5132` was rendered in a browser, where it has no backend of its
  own.** It was given one, rebuilt from the wave 2 audit's prose (`DESIGN-2026-09-16-WAVE2.md`
  §0): `window.__TAURI_INTERNALS__.invoke` relays each `{cmd, args}` to a local server,
  which hands it to the **real running Tauri webview's own `invoke`**. So the console's
  *data and writes are real* and reach the same database and the same registry. Its
  **events are not relayed** — `plugin:event|listen` registers a handler the page can
  fire by hand. A console reading below is therefore a claim about a real command
  surface behind a browser render, and never a claim about event-driven behaviour.

**The harness is temporary and is in no commit.** It was `src/lib/__auditbridge.js` plus
one line in `src/main.js`, and a Node relay in a scratch directory. Both have been
removed; `git status` on this tree is clean of them.

**Measurement, not eyeballing.** Geometry is `getBoundingClientRect`. *Clipping* is a
`Range` over the text node compared against the nearest ancestor whose computed
`overflow` is not `visible`; where the answer mattered, the **ink** box was computed
separately from canvas `TextMetrics` (`actualBoundingBoxAscent`/`Descent` about the line
box's baseline), because a line box that overflows is not the same fact as a glyph that
is cut. Overlays are `elementFromPoint` / `elementsFromPoint` over a control's centre.

### 0a. THREE THINGS THIS PASS GOT WRONG BEFORE IT GOT THEM RIGHT

Recorded because a pass that corrects itself is worth reading with its own history
attached, and because two of the three would have been filed as defects.

1. **"A bound cue starts a clock during a rehearsal."** It does not. The first run set
   rehearsal by invoking `set_rehearsal` through the bridge, which never reaches the
   console's `$rehearsing` store because **events are not relayed**. Driven through the
   app's own `Rehearse` control, the fire runs and no `start_timer` follows, and the
   registry stays empty. Track B's guard holds. The harness had produced the defect it
   was looking for.
2. **"A bound song cue never starts its clock."** The song cue in the fixture carried
   `sections[].lines`, and `plan.js::slidesOf` reads `sections[].lyrics`, so
   `fire_content` was refused for a missing `text` and the fire threw — which correctly
   starts no clock. The fixture was wrong, not the product.
3. **The verse cut at 1024×768 with two timers.** The `.tval` line box overflows its
   cell by 10px and `.tmr` is `overflow: hidden`, which reads as a cut. The ink box is
   inside the cell with room to spare (`inkCutTop 0`, `inkCutBottom 0`). Not a defect;
   it is why every clipping claim below is an ink measurement.

### 0b. THE DEV PORT WAS MOVED, AND THAT IS PART OF THE EVIDENCE

`:5032` was already held by another worktree's Vite on this machine, bound to `[::1]`.
`localhost` resolves to `::1` first on macOS, so a Tauri webview pointed at the compiled
`devUrl` would have loaded **another tree's console into this tree's backend**. The
`devUrl` in `src-tauri/tauri.conf.json` was therefore moved to `http://127.0.0.1:5132`
for the duration and the binary rebuilt; it is back at `http://localhost:5032` now and
the file is unchanged in this branch. Nothing else about the ports moved: `:8031` and
`:8032` are the real ones throughout.

---

## 1. The programme rail, driven

Every figure in this section is the real rail on `stage.html`, fed by the real
`TimerRegistry` through the real hub.

### 1.1 The warning threshold is the one the frame carries, and it crosses where it should

A `stage`-scoped timer with `warn_ms: 60000`, re-aimed by `adjust_timer` across its own
threshold:

| Remaining | `.tmr.warn` | `.tval` colour | `animation-name` |
|---|---|---|---|
| 9:59 | false | `rgb(232, 234, 238)` | `none` |
| 1:01 | false | `rgb(232, 234, 238)` | `none` |
| 0:58 | **true** | **`rgb(244, 81, 91)`** | `cdwarn` |

A timer with `warn_ms: null` never warned at any figure, which is Track A's stated rule —
a threshold the operator chose, or no warning at all. **§2.1 is what that rule costs.**

### 1.2 Reduced motion is a cut, and the colour survives it

Measured in Chrome launched with `--force-prefers-reduced-motion`, on the same warned row:

```
{"reduced":true,"warn":true,"color":"rgb(244, 81, 91)",
 "anim":"none","shadow":"rgba(244, 81, 91, 0.85) 0px 0px 16px"}
```

The pulse is gone, the colour is unconditional, and the glow replaces the motion. A
viewer who asked for no motion still learns the clock is running out.

### 1.3 Zero, with a message and without

Two timers run to zero in the same rail at 1024×768:

| Label | `.tval` | `.msg` | size | face |
|---|---|---|---|---|
| Notices | `Hand over` | yes | 30px | Inter |
| Offering | `0:00` | no | 64px | IBM Plex Mono |

Neither is an empty box, and the two treatments are visibly different. Worth recording
against Track A's own open question — whether the finished message reads at a distance:
in one rail, at one moment, the operator's words are **30px against the digits' 64px**,
so a finished row is less than half the height of a running one.

### 1.4 Held

| State | `.tval` | `.tstate` | colour | warn |
|---|---|---|---|---|
| running | 7:59 | — | `rgb(232, 234, 238)` | false |
| held | 7:59 | `Held` | `rgb(232, 234, 238)` | false |
| held, 2s later | **7:59** | `Held` | — | false |
| released | 7:57 | — | — | false |

The figure is genuinely frozen, and `Held` is in the page's own ink — not amber, not
cyan, not amethyst. **A held row inside its own warning window is not warned**: at 0:59
against a 60000ms threshold, running gave `warn: true` and red; held gave `warn: false`
and the page's ink.

At a cell right on the floor (132.6px), a 299px label ellipses to 82px and `Held` stays
**fully inside the cell** (`stateRightOfCell: 0`, `stateFullyInside: true`). The ordering
Track A chose — the state word is never the thing that gets cut — holds in a real layout
engine.

### 1.5 The floor

Fourteen `stage` timers at once:

| Viewport | capacity | cells shown | cell width | clock size | label size | count cell | row overflow |
|---|---|---|---|---|---|---|---|
| 1280×800 | 9 | 8 + count | 133.7px | — | — | `+6 more` | 0 |
| 1024×768 | 7 | 6 + count | 138.7px | 34.9px | 14.6px | `+8 more` | 0 |
| 390×844 | 2 | 1 + count | 204.5px | 43.8px | **9px** | `+13 more` | 0 |
| 360×740 | 2 | 1 + count | 186.3px | 40.1px | **9px** | `+13 more` | 0 |

Nothing clipped, no horizontal overflow anywhere, and at least one clock always survives.
Six timers at 1024×768 fit without the floor engaging at all (156.3px cells, 40.7px
digits). The floor works exactly as Track A describes it. **§2.2 is about which cells it
keeps.**

### 1.6 The rail is a zone, and switching it off gives the room back

The panel lists seven zones plus the two figure placements, `Programme` among them at
`aria-pressed="true"`. With `John 3:16` on screen at 1024×768:

| | reading height | rail |
|---|---|---|
| rail on | 523.9px | 70px |
| rail off | **593.8px** | absent |
| rail on again | 523.9px | 70px |

`relay.stage.zones` came back
`{"reading":true,…,"programme":false,"figures":"bottom"}` — the new key persists and
`figures` survives beside it.

### 1.7 A stage tablet that joins mid-service

**On the wire**, joining the real hub as a stage tablet does, with a verse on screen and
timers running:

```
0ms  >> {"kind":"hello","template_id":1}
6ms  << template
6ms  << default_template
6ms  << timer      {"kind":"timer","timers":[…]}
6ms  << content    {"content_kind":"scripture",…}
```

**In the page**, recorded from an init script installed before any document script ran,
sampled every animation frame:

| t | rail | verse | reading top |
|---|---|---|---|
| 4ms | — | — | — |
| 20ms | — | — | 58 |
| 64ms | seven rows | `"For God so loved …` | 59 |

The timers and the reading arrive in the **same frame**, in the order the wire sends
them, and the reading's top moves by one pixel. There is no interval in which a clock
stands where the verse belongs. The `02:35` the recorder saw at t=20ms is the **wall
clock** zone, which is local and needs no socket — it is not a countdown.

After a panic control, with stage timers still running, the hello is `template →
default_template → timer → clear`, and a screen joining then paints nothing. A retained
timer frame cannot resurrect a cleared wall.

---

## 2. Findings

Four, filed as RG-146 to RG-149. Two of them are the same kind of thing: wave 4 built
states on the preacher's rail and wave 4 built the only thing that starts a clock, and
the two do not meet.

### 2.1 RG-146 — the rail's warning and its finished message cannot be reached by any control the product ships

§1.1 and §1.3 are true of a timer that carries a `warn_ms` and a done message. **Nothing
in the shipped product gives a programme timer either.**

`warn_ms` reaches the registry only through `start_timer`. `startTimer` is exported once
(`src/lib/stores/capture.js:1487`) and called from exactly one place in the tree —
`Live.svelte:680`, inside `startCueTimer` — and that call passes `minutes`, `label`,
`scope` and `planItemId` and nothing else. Driven, the call goes out as:

```
start_timer {"minutes":12,"label":"Reading","doneMsg":"","scope":"stage",
             "warnMs":null,"planItemId":2}
```

Read back out of the registry: `"warn_ms": null, "done_msg": ""`. Nothing in `src/`
passes `warnMs` to `startTimer`; nothing in `src-tauri/src/` sets `warn_ms` to anything
but `None` outside tests. So on a shipped copy of Relay a programme row can never turn
red and can never say anything at zero but `0:00` — the two states PR #80 exists to add.
`Settings → General → Countdown warning` cannot help: `programmeWarn` returns false for a
non-finite or non-positive `warn_ms` before `countdownWarning`'s default rule is ever
consulted, which is Track A's deliberate choice and is correct given that the stage page
has no bridge to read the setting through.

### 2.2 RG-147 — nothing stops a programme timer, and the rail keeps the oldest

`TimerRegistry` never reaps; a `Stage` timer lives until `stop`, `stop_scope` or the
process ends. The panic controls take `Both` and deliberately leave `Stage` — measured:
`/api/clear` and `/api/black` left all three clocks on the rail, which is the documented
design. And **no component calls `stopTimer`, `adjustTimer`, `showTimer` or `listTimers`**:
all four are exported from `capture.js` and `grep -rln … src --include='*.svelte'`
returns nothing.

Driven: one walk of a three-cue plan, every cue bound (5, 12 and 2 minutes), run to zero.

```
1024×768:  Opening song = 0:00 | Reading = 0:00 | Notices = 0:00
```

A fourth cue then goes on air with a 25-minute clock:

```
1024×768:  Opening song = 0:00 | Reading = 0:00 | Notices = 0:00 | Sermon = 24:55
390×844:   Opening song = 0:00 | +3 more
```

On the preacher's phone the one clock he can see is a dead one from the beginning of the
service, and the live sermon clock is inside the count. `progCells` keeps
`programme.slice(0, keep)` and the registry's snapshot is ordered by id, which is start
order — so the cells the floor keeps are the **oldest**, and by the middle of a service
the oldest are the ones that have finished. This is rule 35 one rail along: the rail says
it could not show three, and shows the three that no longer matter.

### 2.3 RG-148 — the rail plus both stage panels cuts the verse on the preacher's screen

At 1024×768 with the `Zones` and `Control` panels both open — two taps, both operator-
initiated:

| | reading | verse size | verse text block | ink cut |
|---|---|---|---|---|
| rail on, panels closed | 372.1px | 43.1px | 163.3px | none |
| rail **off**, both panels open | 170.3px | 26px | 98.5px | none |
| rail **on**, both panels open | **74.2px** | 26px | 98.5px | **38.9px off the bottom** |

`.reading` reports `scrollHeight 138` against `clientHeight 74` and the page is
`overflow: hidden` by design, so there is nothing to scroll and nothing says the verse is
incomplete. The verse is already on its 26px floor, so `TemplateRender`'s shrink has
nothing left to give. **The rail is the contributing cause**: with the zone switched off
the same two panels leave the verse whole.

Track A's own read predicted this differently — it expected the two panels to sum to
106dvh and push content off the screen. Measured, they sum to **38.8%** and nothing
leaves the screen (`documentElement.scrollHeight === innerHeight`). The panels are not
too tall; the reading is what pays for them, and wave 4 added a row to the bill.

### 2.4 RG-149 — `.alert.sm` cannot render

`ALERT_STEPS` falls through to `sm` only above 150 characters
(`Stage.svelte:502-507`), and `send_stage_alert` caps the text at **140**
(`main.rs:6178`). Driven with a 166-character message: the page received 140 characters
and rendered `.alert.md`. The 3cqw step is unreachable through the only path that sets
`alert`. Measured smallest reachable step: **16.4px** at 390×844, 43.0px at 1024×768 —
not the 11.7px Track A's deferred item 5 describes.

---

## 3. Track B, driven

### 3.1 A cue can carry a clock, and it is not `duration_sec`

Written through `set_plan_timer` and read back out of the real SQLite file:

```
2 scripture Reading       timer_minutes= 12    duration_sec= 0
3 song      Opening song  timer_minutes= 5     duration_sec= 0
4 announce  Notices       timer_minutes= 2     duration_sec= 0
(before binding)          timer_minutes= None  duration_sec= 0
```

An unbound cue reads `None` and not `0`, which is the distinction the column exists for.

### 3.2 A bound cue going on air

Clicking the bound scripture cue in Live's slide grid, against the real backend:

```
… manual_fire
   start_timer {"minutes":12,"label":"Reading","doneMsg":"","scope":"stage",
                "warnMs":null,"planItemId":2}
   set_stage_next …
```

One `start_timer`, `scope: "stage"` and never `both`, carrying the cue's id. The registry
gained `id 1, label "Reading", plan_item_id 2`, and the preacher's rail read
`Reading/11:56` about two seconds later. Walking the song's three sections started **one**
clock, not three.

### 3.3 A rehearsal starts nothing

Through the app's own `Rehearse` control (see §0a.1 for the run that got this wrong):
banner reads `Rehearsal`, the cue fires (`manual_fire`), **no `start_timer` follows**, the
registry is empty, and the stage rail is absent. `get_rehearsal` confirms `true`
throughout.

### 3.4 The slide sizer

Four steps, driven at 1024×768 and at **960×640, the app's own `minWidth`/`minHeight`**:

| step | readout | grid track @1024 | grid track @960 | `−` disabled | `+` disabled |
|---|---|---|---|---|---|
| S | `− S +` | 189.2px | 176.4px | **yes** | no |
| M | `− M +` | 238.5px | 222.5px | no | no |
| L | `− L +` | 320.7px | 299.3px | no | no |
| XL | `− XL +` | 485.0px | 453.0px | no | **yes** |

`.sg-thumb` carries no inline width at any step and keeps `container-type: inline-size`,
so the grid track moved and the container-query root did not. The session stored `0, 1,
2, 3` — an index, never a label.

**The rail that once clipped `Compact` to `Compa`**: the control is **68px wide** at every
step, in a 930px pane head at 960 and a 994px one at 1024, with
`scrollWidth - clientWidth === 0` on both the control and the head at every step and both
widths. It does not clip. Track B's Step 4 is answered.

**Rule 44 over the dock**: with the launch ladder finished and the Recover gate answered,
`elementFromPoint` over the centre of `Clear screens` (179.3 × 41.7px at 960×640) returns
the button itself at both ends of the sizer — `r-cbtn[z=auto]` on top of the stack. The
sizer paints over nothing.

---

## 4. Track A's deferred legibility list, measured

PR #80 deferred nine items to this pass. Every figure below is the real page with a verse,
a stage note, an up-next and four programme clocks all live at once — the state the list
asks about.

| # | Track A's read | Measured | Verdict |
|---|---|---|---|
| 1 | header never got the label pass; `.status` at 11px | `.brand` 16px, `.status` 11px at **every** viewport, against a verse at 43.1px | **confirmed** |
| 2 | the two header controls are under 44px on a phone | `.ctl-toggle` is **30px** tall, 11px text, at 1024, 1280 and 390 alike; `.zonebtn` keeps its 44px | **confirmed** |
| 3 | rows sum to 77%, leaving the reading 23% (177px) | at 1024×768 with every zone live: figrow 115.2 + progrow 96.1 + noterow 50.3 + next 75.3, reading **372.1px = 48.5%**, verse 43.1px | **not reproduced** — the rows do not take their ceilings |
| 4 | `9cqh` in an `inline-size` container resolves against the viewport | `.progrow` is `container-type: inline-size`; at 1280×800 the clamp caps at **64px** before 9cqh's 72px | **confirmed, and harmless today** |
| 5 | `.alert.sm` is 11.7px on a phone | `.alert.sm` **cannot render** — RG-149 | **corrected** |
| 6 | region labels floor at 9px on a phone | `.figk`, `.tlabel` and `.note-lbl` all **9px** at 390×844; 14.6px at 1024, 15.2px at 1280 | **confirmed** |
| 7 | the up-next is a hairline on a platform monitor | `.next-ref` 13.8px, `.next-text` 18.7px at 1024×768 | **confirmed as a figure**; the ratio to the verse is 2.3×, not the ten stated |
| 8 | two panels open exceed the screen (106dvh) | they sum to **38.8%**; nothing leaves the screen. The reading pays instead — RG-148 | **corrected, and worse than the read** |
| 9 | the `Held` word and a long label share a 130px cell | at 132.6px: label ellipses 299px → 82px, `Held` fully inside, `thead` overflow 0 | **confirmed, and the ordering holds** |

---

## 5. The congregation's two rules, re-checked because this wave touched the hub

### 5.1 Rule 42 — the first verse of the service on a freshly opened output page

`output.html` at 1920×1080, opened with nothing on it, then the first fire:

```
at mount:   document.fonts.status === "loaded"    ← the trap condition
after fire: font-size 103.26px, family Fraunces, document.fonts.check → true
            text block 535.9px inside a 583.2px .ltext box
            23.0px clear at the top, 24.3px clear at the bottom
```

The deferred re-fit is doing its job. Nothing is sliced.

### 5.2 Rule 43 — an OBS source restarting mid-service

Recorded from before first paint: nothing at 6ms, the retained verse painted at **23ms**,
and no blank interval after it. After a panic control, a page joining paints nothing and
keeps painting nothing — a cleared wall stays cleared, with stage timers live on the same
hub.

---

## 6. One thing observed once and not reproduced

On the first walk of the plan, `start_timer` was called **twice** for one song cue. The
backend absorbed it (`start_timer` stops the cue's previous clock when `plan_item_id`
matches), so the registry held one timer and the visible effect would have been a clock
restarting rather than two clocks appearing. It did not recur in **six** subsequent runs —
deliberate gaps of 200, 400, 800 and 1200ms between two slides of the same cue, and two
further full walks — every one of which produced exactly one start per cue. The run that
produced it was the first after the plan fixture was rebuilt underneath the open grid.
Recorded rather than filed, because a defect nobody can reproduce is a report, not a
finding.

---

## 7. What this pass did not drive, and what it therefore does not claim

* **The Planner's cue inspector was not clicked.** Track B's control was exercised
  through `set_plan_timer` and verified in SQLite. That the control writes that command
  and nothing from `TAKES_A_SCREEN` is Track B's own test's claim, not this pass's.
* **The Templates and Library workspaces were not driven at all.**
* **Nothing here is a claim about event-driven console behaviour** — §0 says why.
* **No word error rate, no detection, no threshold, no latency.** This pass put no audio
  through Relay.
* **It does not move the release decision.** `docs/RELAY_V1_AUDIT.md` owns that.
