Based on the screens already designed, Relay has a solid MVP, but a production-grade application for live church environments requires   operational, administrative, recovery, and collaboration screens.

Below is a complete screen architecture for a version 1.0 production release.

---

## Build status

Screens are built one at a time against `relay-design-loop-prompt.md`. A screen is
marked **DONE** only when it is built, gated (`npm run build` + `npx vitest run`),
compared against its reference, and logged in `docs/relaydesign/.loop/<screen>-log.md`.

| marker | meaning |
|---|---|
| **✅ DONE** | built, gated, compared against its design reference, logged |
| **🟡 WIP** | in progress this session |
| *(unmarked)* | not started |

Where a screen is marked done from a **code-level** compare rather than a pixel one,
its log says so — this machine cannot capture the Tauri console webview.

## Progress

| § | Section | State |
|---|---|---|
| 1 | Launch & Startup | **✅ complete** — 8 screens · `.loop/launch-log.md` |
| 2 | First Run | **✅ complete** — one 6-step wizard · `.loop/firstrun-log.md` |
| 3 | Dashboard | **✅ complete** — `.loop/dashboard-log.md` |
| 4 | Live Production | **✅ resolved** — 2 built, rest already existed or refused · `.loop/liveproduction-log.md` |
| 5 | AI Detection | **✅ resolved** — Inspector built · `.loop/aidetection-log.md` |
| 6 | Transcript | **✅ resolved** — search built; the real work was the audio fix (DECISIONS §23) |
| 7 | Scripture Library | **✅ resolved** — Bible browser built · `.loop/scripturelibrary-log.md` |
| 8 | Service Planner | **✅ resolved** — three-column rebuild + sections/durations migration · `.loop/planner-log.md` |
| 9 | Templates | **✅ resolved** — gallery + reworked editor, honest to the model · `.loop/templates-log.md` |
| 10 | Output Management | **✅ resolved** — Channels rebuilt + liveness made real · `.loop/channels-log.md` |
| 11+ | Stage Display onward | not started |

Also done outside the numbered sections: the app-wide token and mode-colour
rebrand, the application icon and brand mark (`.loop/rebrand-log.md`,
DECISIONS §22), and the Live Surface / Template Designer / Help screens.

**A note on what "resolved" means.** Several sections list screens Relay cannot
honestly build — a licence activation screen for MIT software, OBS/ATEM setup for
protocols it does not speak, a speaker timeline with no diarization, a translation
downloader with no translations to download. Those are marked NOT BUILT with the
reason, rather than shipped as UI that implies a capability. Each section's log
records which, and why.

**Done so far:** all of **§1 Launch & Startup** (`.loop/launch-log.md`) ·
**§2 First Run** (`.loop/firstrun-log.md`) · **§3 Dashboard** (`.loop/dashboard-log.md`) · **§4 Live Production** (`.loop/liveproduction-log.md`) · **§5 AI Detection** (`.loop/aidetection-log.md`) · **§6 Transcript** · **§7 Scripture Library** (`.loop/scripturelibrary-log.md`) · **§8 Service Planner** (`.loop/planner-log.md`) · **§9 Templates** (`.loop/templates-log.md`) · **§10 Output Management** (`.loop/channels-log.md`) ·
Live Surface (the operator console) · Template Designer · Help / Shortcuts ·
the app-wide token and mode-colour rebrand (`.loop/rebrand-log.md`).

---

# 1. Launch & Startup

**Section complete.** All eight built, gated and compared — `.loop/launch-log.md`.
There is no mockup PNG for any of them, so they were built against the design
system sheet rather than pixel-matched to a screen reference.

* Splash Screen — **✅ DONE** (`.loop/splash-log.md`, pixel)
* Boot Diagnostics — **✅ DONE** (`.loop/launch-log.md`, pixel)
* Hardware Check — **✅ DONE** (pixel; all 7 rows are **real reads** —
  `system_hardware` in `src-tauri/src/sysprobe.rs`. GPU reports the backends
  compiled into the binary, never the card in the machine)
* Plugin Loading — **✅ DONE** (pixel; built as **Integrations** — Relay has no
  plugin loader and runs no third-party code at boot. OBS/ATEM are real TCP
  reachability probes, worded as "something is listening", never "OBS is running")
* Database Migration — **✅ DONE** (pixel; a *verification* screen, not a progress
  bar — `run_migrations` finishes before the webview exists, so it asks SQLite
  what actually exists via `migration_status`)
* Recover Previous Session — **✅ DONE** (pixel)
* Safe Mode Startup — **✅ DONE** (pixel; honoured by `App.svelte`, exit in Settings)
* Crash Report Recovery — **✅ DONE** (pixel)
* Update Available — **✅ DONE** (pixel)
* ~~License Verification~~ — **DROPPED.** Relay is MIT / free / open source
  (`CLAUDE.md`), and `docs/DECISIONS.md` contains no activation or seat decision.
  An activation screen would be the first thing in the product to contradict its
  own licence. Dropped by a human decision, recorded in `.loop/launch-log.md`.
  This also reconciles the count table below, which says 8 for this module.

---

# 2. First Run

**Section complete** — built as ONE six-step wizard against
`relay-production-interface.png` panel 2. Log: `.loop/firstrun-log.md`.
A step that asks the operator nothing does not need its own screen.

* Welcome — **✅ DONE** (step 1)
* Output Detection — **✅ DONE** (step 2 "Screen" — real `list_monitors`;
  added to the reference, which has no screen step, because it is the only step
  that puts anything in front of a congregation)
* Audio Setup — **✅ DONE** (step 3)
* Audio Calibration — **✅ DONE** (step 3 — live segmented level meter)
* STT Download — **✅ DONE** (step 4 — the real resumable, checksummed download)
* GPU Detection — **✅ DONE** (a line on step 4 from the real `system_hardware`
  probe; reports the backends compiled into THIS BUILD, not the card in the machine)
* Language Setup — **✅ DONE** (step 5 — single-choice, not the reference's
  checkbox list: whisper takes one language or auto, so multi-select would be a
  control that cannot do what it offers)
* Test Recognition — **✅ DONE**, folded into step 6: the proof verse exercises the
  real output path without needing a model, a mic and someone to preach at a laptop
* Keyboard Shortcuts — **✅ DONE** (step 6)
* Finish Wizard — **✅ DONE** (step 6 — fires John 3:16 at the real screen)
* ~~OBS Connection Setup~~ — **NOT BUILT.** Relay does not implement the OBS
  WebSocket protocol.
* ~~ATEM Discovery~~ — **NOT BUILT.** Relay does not implement ATEM's protocol.
* ~~ProPresenter Connection~~ — **NOT BUILT.** ProPresenter support is file
  import only.

  The three above would be setup flows for connections that do not exist — see
  the §1 Integrations screen, which reports the same thing to the operator's
  face. They become real screens if control channels are ever implemented.

---

# 3. Dashboard

**Section complete** — one screen, `src/lib/views/Dashboard.svelte`.
Log: `.loop/dashboard-log.md`. No reference panel exists, so it was built against
the design system sheet. It answers the one question no other tab does: *is this
machine going to work, before anyone is in the room?*

**Nothing on it can put anything on a screen** — firing content stays in Live.

* Home Dashboard — **✅ DONE** (pixel)
* System Health — **✅ DONE** — literally the Boot Diagnostics checks, re-run on
  demand (`runChecks()` is shared). Two health panels would eventually disagree
* Quick Actions — **✅ DONE** — open the output window, arm the mic, rehearse,
  go to Live. None of them fire content
* Recent Services — **✅ DONE** — real `list_services` rows
* ~~Recent Projects~~ — **DROPPED.** Relay has no "projects": it has service
  plans and service history, both already on this screen. A third noun for the
  same thing is admin-template vocabulary, not Relay's. Reconciles the count
  table, which says 4 for this module.

---

# 4. Live Production

**Section resolved** — log: `.loop/liveproduction-log.md`. Two modes built; the
rest already exist elsewhere, describe concepts Relay does not have, or (the
confidence screens) would manufacture the one number the product refuses to show.

* Live Surface — **✅ DONE** (`.loop/live-log.md`, pixel for layout/chrome; populated states code-level)
* Compact Mode — **✅ DONE** (pixel at 1280×800 — a booth laptop. A density
  change, not a screen; **nothing is hidden**, the preview row just stops eating
  a third of a short window)
* Full Screen Live Control — **✅ DONE** (pixel. **Escape does NOT exit it** —
  Escape clears the congregation's screens and always will, so the way out is a
  visible button. Applies only while Live is active)
* Active Outputs Overview — **✅ ALREADY BUILT**: the Output Status panel in Live
* Emergency Control Panel — **NOT BUILT.** Panic already has three entry points
  (Quick Controls, global `Esc`/`B`, top-bar Emergency Stop) onto ONE code path.
  A fourth surface is a fourth path to keep correct on the one control that may
  never report a success it did not achieve (DECISIONS §20)
* Dual Monitor Mode · Multi Monitor Control — **NOT BUILT.** Channels owns output
  targets and display assignment; a second place to choose the congregation's
  screen is a second source of truth
* Operator View · Producer View — **NOT BUILT.** Relay has no roles. One operator,
  one desk (User Management is §21 and unbuilt)
* Confidence View · AI Confidence Timeline — **DEFERRED to §5, with a warning.**
  A paraphrase shows **no percentage, at any score** — a cosine is not a
  probability (CLAUDE.md §18, DECISIONS §21). A confidence *chart* needs a
  y-value per point, so it pressures you into inventing exactly that number.
  §5 must plot the KIND of claim and its evidence, never a confidence curve

---

# 5. AI Detection

**Section resolved** — log: `.loop/aidetection-log.md`. This is the one section
where the **mockup contradicts the product's own law**: panel 8 draws a
percentage on every claim and explains matches with bullets describing an
algorithm Relay does not have. Building it literally would have shipped the bug —
proved by reverting the guard and watching two tests fail.

* Inspector — **✅ DONE** (pixel + 11 component tests). Cyan claim chip, **no
  percentage on a paraphrase at any score**, real evidence only (the transcript
  span for a heard reference; the actual TF-IDF terms for a guess), and it says
  out loud that accepting or dismissing **retunes the gate** — a loop that has
  been running for months with nothing on screen admitting it
* Verse Match Comparison — **✅ FOLDED into the Inspector**: an ambiguous
  reference's other candidates, side by side, at the only moment comparing them
  is useful
* Confidence Tuning — **already in Settings.** Shown read-only in the Inspector
  with a link. Thresholds have exactly ONE baseline (`router.rs`); a second set
  of sliders is a second source of truth for the gate
* Detection History — **already in Library → History** (fired detections per service)
* Recognition Logs — **deferred to §19 Logs** so it is built once; the live view
  of it is Live's transcript panel
* False Positive Review · AI Learning Feedback — **partly real, now surfaced.**
  Dismissing already IS the false-positive signal and already retunes the gate.
  A review *queue* is **NOT BUILT**: it needs a persisted per-detection verdict
  that does not exist (`detections.status` supports self-calibration, not
  reconstructing "the AI was wrong about this one"). Not faked

---

# 6. Transcript

**Section resolved.** The transcript work in this section became mostly an AUDIO
fix rather than a UI one — see `docs/DECISIONS.md` §23.

* Live Transcript — **✅ ALREADY BUILT** — panel 1 of the Live surface
* Search Transcript — **✅ DONE** — search within the open service's transcript
  in Library → History, with match highlighting and a count. It says plainly that
  it searches **that service only**; there is no backend cross-service transcript
  search, and a box that silently covers less than the operator assumes is worse
  than no box
* Transcript Export — **✅ ALREADY BUILT** — `export_service` writes the
  transcript and fired detections to Markdown
* Transcript Editor · Timestamp Editor — **NOT BUILT.** No backend exists to
  update a transcript row, and editing one is not a neutral feature: the
  transcript is the RECORD of what a person said from a pulpit. Making it
  editable is a decision about that record, and `docs/DECISIONS.md` contains no
  such decision
* Speaker Timeline — **NOT BUILT, and it cannot be faked.** It needs speaker
  diarization. There is **zero** diarization anywhere in the codebase (grepped);
  whisper.cpp does not provide it and `voice_profiles` is per-preacher
  calibration, not "who is talking now". A timeline drawn without it would be
  invented data about who said what

**Also fixed here, from a live report:** the transcript was emitting Chinese and
other unspoken languages. That is whisper hallucinating on non-speech — see
DECISIONS §23 for the three-layer fix (a neural speech-probability veto on the
gate, whisper's own suppression parameters which were never switched on, and a
script check). Service History was additionally showing `conf 0.61` on
paraphrases — the forbidden number, decimal-formatted.

---

# 7. Scripture Library

**Section resolved** — log: `.loop/scripturelibrary-log.md`. Built against panel
6. The Library could search and could list what was saved, but could not **open a
Bible and read it** — which is what the word promises.

* Library — **✅ REBUILT, ProPresenter-style** — a **slide grid of real rendered
  thumbnails** (same `TemplateRender` the projector uses, so WYSIWYG by
  construction), click-to-fire, and a **live strip showing what is on the wall
  right now** without leaving the tab. The on-air slide carries the amber ring;
  in rehearsal it goes amethyst, because nothing is reaching anyone.
  Book tree in canonical order, chapter list, Read view alongside Slides
  (`list_books`, `chapter_verses`).
  **Canonical order is the substantive part**: `GROUP BY book` returns
  alphabetical, and a Bible opening "Acts, Amos, Chronicles" is unnavigable.
  Pinned by a test against the real 31,100-verse corpus
* Favourite Scriptures — **✅ ALREADY BUILT** (the Saved tab), now reachable
  per-verse from the browser
* Recent Scriptures — **✅ ALREADY BUILT** — Service History lists every fired verse
* Verse Comparison — **✅ as far as the data allows.** Comparing translations
  needs ≥2; there is 1. The picker appears the moment a second exists
* Translation Manager — **✅ partly.** The active translation is chosen in
  Settings and honoured here; there is nothing to *manage* with one translation
* Bible Metadata — **✅ partly.** Book and chapter counts are real and shown. The
  corpus carries no author/date metadata and none is invented
* Search History — **NOT BUILT.** Nothing persists queries. Reconstructing it
  from the detection log would show fired verses, not searches
* ~~Download Translations~~ · ~~Offline Packages~~ · ~~Translation Import~~ —
  **NOT BUILT: one blocked problem, not three screens.** All three are the same
  missing capability — getting a second Bible onto the machine — which needs a
  source, a licence and a format decision that `docs/DECISIONS.md` does not
  record. A downloader pointed at nothing is three screens pretending

> **Relay ships the KJV only.** Verified against the live database: one
> translation row. Detection already *recognises* spoken Yorùbá, Kiswahili and
> Hausa references — that alias table is real — what is missing is verse TEXT in
> those languages. The reference's four language tabs are therefore **not** drawn;
> the picker is built from translations that exist, and the gap is stated in
> words. Showing three empty tabs would make exactly the claim
> `docs/LANGUAGES.md` is careful not to make.

---

# 8. Service Planner

**Section resolved.** Built and compared against `relay-planner-screen.png` —
`.loop/planner-log.md` (pixel, 7 iterations + 4 state variants). The Planner was
rebuilt as a three-column workspace (plans rail · running order · cue inspector),
and `plan_items` gained **sections** and **durations** via a retryable migration.

* Planner — **✅ DONE** (pixel; three-column rebuild, `.loop/planner-log.md`)
* Cue Library — **✅ DONE** — the ＋ Add Cue panel: one search across scripture,
  songs, media and announcements, plus the countdown quick-add
* Song Library — **✅ DONE** — searchable from ＋ Add Cue, with the arrangement
  picker when a song has saved arrangements
* Worship Set Builder — **✅ DONE** — this IS the running order: drag-reorder,
  sections, per-cue template and duration. Not a separate screen; a worship set
  is a service plan with song cues in it
* Announcement Builder — **✅ DONE** — authored in the Library, added here as a cue
* Countdown Builder — **✅ DONE** — the quick-add in ＋ Add Cue; a countdown is the
  one cue type whose length is known at build time, so it seeds its own duration
* Speaker Notes — **✅ DONE** — the inspector's **Notes** tab (`stage_note`).
  Confidence-monitor only; it never reaches an output screen
* Media Attachments — **✅ DONE** — media cues from the Library
* Duplicate Service — **✅ DONE** — `duplicate_plan`, now carrying section
  headings and durations across (it silently dropped them until this pass)
* Service Templates — **✅ RESOLVED as Duplicate.** A "service template" and a
  duplicated plan are the same object in Relay's model — an ordered list of cues
  with headings. Shipping both would be two names for one table
* ~~Calendar~~ — **NOT BUILT.** A plan has a `plan_date` and the rail is ordered
  by it, but there is no scheduling, no recurrence and no reminders behind a
  month grid. A calendar that only filters a five-item list is decoration
* ~~Archive~~ — **NOT BUILT.** Nothing is archived: plans are kept until deleted,
  and `service_plans` has no archived flag. Deferred with §17 Service History,
  which is where a past service actually lives

---

# 9. Templates

**Section resolved.** The Templates tab is now two modes matching the two
mockups: a **gallery** (`relay-templetes-screen.png`) of every template as a live
thumbnail, and an **editor** (`relay-templeteeditor-screen.png`) for one.
`.loop/templates-log.md` (pixel; the gallery is new, the editor reworks the prior
Output Designer). The template MODEL is unchanged — layout(regions) + style — so
everything the editor mockup shows beyond that model is refused, not faked.

* Template Gallery — **✅ DONE** (pixel) — grid/list of cards rendered through the
  real `TemplateRender`, so a card is what the wall shows; derived type tabs;
  search; sort; preview inspector with Details + Usage
* Template Designer / Editor — **✅ DONE** (pixel) — reworked into the mockup's
  layers + design grammar. Layers map to the template's REAL parts (background,
  lower-third band, reference, verse text); the design panel edits real style
* Lower Third Designer — **✅ DONE** — the Lower-Third band is a real layer/toggle;
  a lower-third template is derived and filtered as its own kind
* Scripture Layouts / Song Layouts — **✅ DONE** — these ARE the derived kinds
  (Scripture = reference + verse; Song = verse only), shown as gallery type tabs
* Safe Area Preview — **✅ DONE** — the safe-area guide is a layer in the editor
  (editor-only; never reaches an output)
* Responsive Preview — **✅ RESOLVED as a readout.** Every template is 16:9 by
  construction (`TemplateRender` sizes in cqw, so it scales identically at any
  output size). There is nothing to preview responsively; the gallery and editor
  state `16:9 · 1920×1080` as a readout rather than offering an orientation that
  does not exist
* ~~Theme Manager~~ / ~~Typography Presets~~ — **NOT BUILT.** There are no themes
  or type presets in the model — a template carries a font family and colours
  directly, and that is the whole of it. A preset system is a feature, not a screen
* ~~Background Library~~ / ~~Brand Assets~~ — **NOT BUILT.** A background is a
  per-template fill or a single uploaded image; there is no shared asset store,
  media browser, or reusable element gallery to manage. The editor mockup's Assets
  panel (backgrounds/gradients/ornaments/shapes/frames/dividers) has no backing
  and is omitted with a note in the layers panel
* ~~Animation Presets~~ — **NOT BUILT.** The only motion a template has is one
  crossfade duration (`transitionMs`), edited in the editor's Motion section.
  There are no keyframes, entrance/exit animations, or an Animation tab to preset

---

# 10. Output Management

**Section resolved.** Built and compared against `relay-channels-screen.png` —
`.loop/channels-log.md` (pixel, 3 iterations + 2 variants). The screen was rebuilt
as a filtered table plus a channel inspector, and **per-channel liveness was made
real**: it is computed from open output windows and connected kiosk clients, not
read from `output_channels.status`, which is written once at insert and had always
said `offline` for every channel — including one filling a projector.

* Channels — **✅ DONE** (pixel; `.loop/channels-log.md`)
* Output Preview — **✅ DONE** — the inspector renders the channel's own template
  through `TemplateRender`, the same engine the wall uses, so it is WYSIWYG
* Browser Source Manager — **✅ DONE** — network channels carry their copyable
  `:8032` URL and a QR to open it on a phone or kiosk without typing
* HDMI Outputs — **✅ DONE** — native-window channels, with a real display picker.
  Displays now carry **the name macOS itself shows** (`NSScreen.localizedName`,
  verified as "HP 532sf" on real hardware) instead of tao's raw EDID model number
  `Monitor #1234555`; elsewhere the OS string is humanised (`\\.\DISPLAY1` →
  "Display 1", `HDMI-1` → "HDMI 1", `eDP-1` → "Built-in display"). A channel's
  assigned display is also actually honoured now — it was silently ignored and
  fell back to primary. `cargo run --example displays` is the standing probe.
  **Open:** the Windows half (EDID lookup) and multi-display position matching
  are both untested — see `.loop/channels-log.md`
* Display Arrangement — **✅ RESOLVED as the display picker.** Relay does not
  arrange displays; the OS does. What it needs is to name them recognisably and
  pin a channel to one, which the picker does (name · resolution · primary)
* NDI Outputs — **⚠️ SURFACED, NOT BUILT.** NDI is parked (needs a proprietary
  SDK; `open_ndi_output` returns a clear error). The tab exists and any NDI
  channel reports **UNAVAILABLE** — deliberately a different word from "offline",
  which would read as a fault rather than an absent capability
* ~~Output Monitor~~ — **NOT BUILT.** Would mean a live thumbnail of what each
  screen is showing. Relay pushes content to outputs and never reads a frame
  back; there is no capture path, so a "monitor" could only re-render a guess
* ~~Output Diagnostics~~ — **NOT BUILT, deliberately.** This is the reference's
  CHANNEL HEALTH panel: bandwidth, dropped frames, uptime, latency, "Excellent".
  **None of it exists** — nothing in the pipeline times a delivery, counts a
  frame, or records a connect time. On the one screen an operator uses to decide
  whether the projector is working, invented numbers are the worst possible
  decoration. The panel states the limit in words instead
* ~~Multi Screen Mapping~~ — **NOT BUILT.** Every channel already targets its own
  display or client independently; a mapping matrix would be a second way to say
  the same thing
* ~~Virtual Displays~~ — **NOT BUILT.** Creating a virtual display is an OS-level
  driver concern, not something an app can offer honestly

---

# 11. Stage Display

* Stage Display
* Confidence Monitor
* Preacher View
* Musician View
* Timer View
* Countdown View
* Remote Stage Display

Stage Displays
│
├── Display Manager
├── Display Preview
├── Layout Library
├── Layout Editor
├── Layout Variables
├── Theme Manager
├── Remote Displays
├── Mobile Displays
├── Display Diagnostics
└── Display Settings

---

# 12. Presentation Output

* Projector Output
* Transparent Output
* Alpha Key Output
* Lower Third Output
* Announcement Output
* Song Output
* Scripture Output
* Full Screen Preview

---

# 13. Media

* Image Library
* Video Library
* Audio Library
* Background Videos
* Motion Graphics
* Logos
* Church Branding
* Media Collections

---

# 14. Connections

* OBS
* ATEM
* ProPresenter
* NDI
* Companion
* HTTP API
* OSC
* MIDI
* WebSocket
* Stream Deck

---

# 15. Audio

* Input Devices
* Audio Mixer
* Noise Reduction
* Audio Monitoring
* Channel Mapping
* Gain Calibration

---

# 16. Automation

* Automation Rules
* Trigger Builder
* Verse Auto Fire Rules
* Cue Automation
* Scheduled Actions
* Conditions
* Delay Actions

---

# 17. Service History

* Service History
* Timeline Replay
* Statistics
* Manual Overrides
* AI Suggestions
* Export Service
* Restore Service

---

# 18. Analytics

* Detection Accuracy
* Manual Override Frequency
* Average Recognition Time
* Translation Usage
* Output Usage
* Church Activity
* Weekly Reports

---

# 19. Logs

* Application Logs
* Audio Logs
* Recognition Logs
* Output Logs
* API Logs
* Error Logs
* Crash Reports

---

# 20. Settings

* Settings

 
## General

* Appearance
* Startup
* Updates

## Audio

* Devices
* Sample Rate
* Calibration

## AI

* Models
* Thresholds
* Languages

## Outputs

* Channels
* Monitors
* Browser Sources

## Integrations

* OBS
* ATEM
* Companion
* ProPresenter

## Security

* Local Encryption
* Backup
* Restore

## Telemetry

* Privacy
* Diagnostics

## Licensing

* Activation
* Seats

---

# 21. User Management

 
* Sign In
* Local Accounts
* Roles
* Permissions
* Operator Profiles

---

# 22. Backup


* Automatic Backup
* Manual Backup
* Restore
* Import
* Export

---

# 23. Notifications


* Update Notifications
* Connection Alerts
* Recognition Alerts
* Hardware Warnings

---

# 24. Help

* Shortcuts — **✅ DONE** (`.loop/help-log.md`, pixel — board generated from the real binding table)
* Documentation
* Interactive Tutorials
* Demo Mode
* Keyboard Reference — **✅ DONE** (same board; `.loop/help-log.md`)
* About Relay
* System Diagnostics

---

# 25. Recovery

* Crash Recovery — rebranded only (`.loop/rebrand-log.md`); layout not yet built to a reference
* Restore Timeline
* Missing Device Recovery
* Missing Translation Recovery
* Offline Recovery

---

# 26. Onboarding

* Feature Tour
* Sample Service
* Practice Mode
* Demo Church

---

# 27. Developer

* Debug Console
* Performance Monitor
* AI Diagnostics
* Memory Usage
* GPU Status
* Network Status

---

# 28. Mobile Companion (Future)

* Remote Operator
* Stage Display
* Pastor View
* Confidence View
* Emergency Blackout
* Manual Verse Trigger

---

# Screen Count

| Module              | Screens |
| ------------------- | ------: |
| Launch & Startup    |       8 |
| First Run           |       9 |
| Dashboard           |       4 |
| Live Production     |      10 |
| AI Detection        |       6 |
| Transcript          |       6 |
| Scripture Library   |       8 |
| Service Planner     |      10 |
| Templates           |      10 |
| Outputs             |       9 |
| Stage Display       |       5 |
| Presentation Output |       6 |
| Media               |       8 |
| Connections         |       9 |
| Audio               |       6 |
| Automation          |       6 |
| Service History     |       6 |
| Analytics           |       7 |
| Logs                |       6 |
| Settings            |      12 |
| User Management     |       4 |
| Backup              |       5 |
| Notifications       |       4 |
| Help                |       6 |
| Recovery            |       5 |
| Onboarding          |       4 |
| Developer           |       6 |
| Mobile Companion    |       6 |

**Estimated total:** **approximately 200 distinct screens**.

This scope is comparable to mature broadcast and production software such as OBS Studio, vMix, ProPresenter, Ross XPression, and Blackmagic ATEM Software Control. It provides the operational depth expected for an application designed to be reliable in live production while remaining focused on Relay's core capability: AI-assisted scripture detection and routing during live church services.
