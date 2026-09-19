# ProPresenter Stage Display and Timers: a feature model

Research reference, written for engineers who have never used ProPresenter. Compiled 2026-09-19 from Renewed Vision's own current knowledge base, an archived copy of their official user guide, their published OpenAPI specification, and two community sources that are marked as such.

**Read the honesty notes.** Every major claim carries its source URL. Anything I could not confirm is marked **UNCONFIRMED** rather than smoothed over, and the three places where sources actively disagree are called out.

---

## 0. Scope, version and naming

| Fact | Detail | Source |
|---|---|---|
| Product naming | "ProPresenter 7" is the historic name. Renewed Vision now version the product as 20.x / 21.x. The current release at time of writing is **21.4** (1 July 2026). The knowledge base still titles articles "…in ProPresenter 7". | https://renewedvision.com/propresenter/whats-new |
| Canonical docs today | The Zendesk knowledge base, section "ProPresenter" (133 articles), which is also what the site links as "ProPresenter 7 (Mac & Win Web)". | https://support.renewedvision.com/hc/en-us/sections/360002412274-ProPresenter |
| The old user guide | `learn.renewedvision.com/propresenter/*` was the structured official user guide. It is **retired**: every URL now 302s to the knowledge base home. The archived copy is still the most complete element catalogue anyone has published, so this document uses it and labels it *archived official guide*. | https://web.archive.org/web/2024/https://learn.renewedvision.com/propresenter/screen-configuration |
| The PDF manual | `files.renewedvision.com/propresenter/support/Pro7UserGuide.pdf` is still linked from the support home but returns **HTTP 522** (origin timeout). Could not be read. | Link at https://support.renewedvision.com/hc/en-us |
| API reference | Renewed Vision publish a Swagger UI and a machine-readable spec (OpenAPI 3.0.2, "ProPresenter API 1.0"). Endpoints quoted here come from that spec, not from guesswork. | https://openapi.propresenter.com/ and https://openapi.propresenter.com/swagger.json |

Two community sources are used, and only where nothing official exists. Both are explicitly labelled where they appear:

- `jeffmikels/ProPresenter-API` documents the **undocumented** WebSocket protocols as of ProPresenter 7.6 (https://jeffmikels.github.io/ProPresenter-API/Pro7/). Its own front matter warns that invalid messages can crash ProPresenter.
- `L2N6H5B3/ProWebStage`, an HTML/CSS/JS stage display client for ProPresenter 7 (https://github.com/L2N6H5B3/ProWebStage), which corroborates the WebSocket endpoint and the separate stage password.

---

## 1. Stage Display model

### 1.1 What a screen is, before layouts enter the picture

ProPresenter separates **Screens** (logical render targets) from **Outputs** (physical or virtual destinations). Screens are configured under `Screens → Configure Screens`, or `Option-Command-1` on Mac (`Control-Alt-1` on Windows). Every screen is declared as one of exactly two classes:

- **Audience** — crowd-facing.
- **Stage** — "typically seen by those on stage or producing the program… also referred to as Confidence Monitors, Foldback screens, or DSMs (down stage monitors)".

Source: https://support.renewedvision.com/hc/en-us/articles/360041879173-Screen-Configuration-in-ProPresenter

Each screen targets a system display, an NDI virtual output, an SDI output (with hardware), a Syphon output (Mac only), or a **Placeholder** (a screen defined by resolution alone, for building a show off-site). A screen can be **Single**, **Mirror** (one screen to several outputs), **Grouped** (one screen stretched across a grid of outputs), or **Edge Blend**. Same source.

All stage screens can be toggled on and off together: `Command-2` / `Control-2` (https://support.renewedvision.com/hc/en-us/articles/360042123293-Keyboard-Shortcuts-in-ProPresenter), and over the API via `GET`/`PUT /v1/status/stage_screens`, which is a single boolean for all of them.

### 1.2 What a Stage Layout is

A **Stage Layout** is a free-form design canvas, edited very much like a slide, whose objects are *linked* to live application data rather than holding static content. It is a global object: layouts live in one list and any stage screen can be pointed at any layout.

A layout has exactly three layout-level properties (visible when nothing on the canvas is selected):

- **Name** — shown wherever layouts are chosen.
- **Background** — a colour, default black.
- **Size** — the resolution the layout renders at. The guide recommends matching the output's resolution.

Source (archived official guide, "Stage Layouts"): https://web.archive.org/web/2024/https://learn.renewedvision.com/propresenter/screen-configuration

Opening the editor, three equivalent routes:

- `Command-4` (Mac) / `Control-4` (Windows)
- `Screens → Edit Layouts…` in the menu bar
- `More` in the toolbar → `Stage Editor`

Sources: https://support.renewedvision.com/hc/en-us/articles/360041407794-Using-a-Stage-Screen-to-its-Full-Potential ; https://support.renewedvision.com/hc/en-us/articles/360053250613-Using-Timers-on-Stage-Screens ; shortcut confirmed in the keyboard shortcuts article.

A fresh install ships with one pre-loaded layout. `+` next to the "Stage Layouts" heading offers either a **pre-loaded layout** to start from or a **blank** one. Right-click a layout to copy, paste, duplicate, delete or rename. Source: the "Full Potential" article above.

### 1.3 How a layout is assigned to a stage screen

Four mechanisms, and the distinction matters because two of them are operator actions and two are automation.

| Mechanism | Where | Notes | Source |
|---|---|---|---|
| **Show button** | Bottom right of the Stage Editor. Pick which stage screen gets this layout. | Manual, immediate. | "Full Potential" article |
| **Stage panel in Show Controls** | Bottom-right show-controls area. Each active stage screen is shown with a thumbnail of its current layout; click the thumbnail for a dropdown of layouts, or to jump to the Stage Editor. | Manual, immediate. This is the operator's normal route during a service. | Archived official guide, `interface#stage-controls` |
| **Stage slide action** | Right-click a slide → `Add Action` → `Stage`. A popover lists every stage screen with a layout dropdown. | Fires whenever that slide is selected. Also carries the audience-suppression option below. | "Full Potential" article |
| **Action Palette** | `View → Action Palette`, drag a Stage Layout cue onto a slide. | Same effect, different UI. | Renewed Vision blog transcript, https://www.renewedvision.com/blog/video-how-to-create-stage-screens-in-propresenter-7 |

Two behaviours engineers should note:

- **Layout changes are sticky.** "Stage Display Layout actions will stay applied until you manually change the Stage Display Layout in the Layouts Editor or another slide with a different Stage Display Layout Action is selected." There is no automatic revert. Source: "Full Potential" article.
- **Stage Only vs Stage + Audience.** A Stage action can also set the slide's destination. `Stage Only` means the slide goes to stage screens and the audience screens keep whatever was last sent to them. You return to normal by adding a `Stage + Audience` action to the first slide that should go back. There is also a keyboard shortcut, `Command-0` / `Control-0`, "Show Slide on Stage Display Only". Sources: "Full Potential" article; keyboard shortcuts article.

API equivalents, from the official spec:

```
GET    /v1/stage/screens                      list configured stage screens
GET    /v1/stage/layouts                      list configured stage layouts
GET    /v1/stage/layout_map                   current layout per stage screen
PUT    /v1/stage/layout_map                   set layouts for screens (by uuid, name or index)
GET    /v1/stage/screen/{id}/layout           current layout of one screen
GET    /v1/stage/screen/{id}/layout/{layout_id}   set the layout of one screen
DELETE /v1/stage/layout/{id}                  delete a layout
GET    /v1/stage/layout/{id}/thumbnail        thumbnail of a layout
```

Source: https://openapi.propresenter.com/swagger.json. Worth noting for anyone building against it: `PUT /v1/stage/layout_map` accepts `[{screen: {...}, layout: {...}}]` and the spec's own examples show matching **by name** as well as by index, so a client does not have to resolve UUIDs first. The spec's summary text for that endpoint reads "Sets the specified stage **message** to the corresponding stage screens", which is a copy-paste error in Renewed Vision's own documentation; the request body is unambiguously a layout map.

### 1.4 Multiple stage screens with different layouts

There is no special handling and no concept of a "primary" stage screen. The layout map is a plain per-screen assignment, every stage screen is independent, and each layout carries its own **Size**, so two stage screens at different resolutions or aspect ratios get two layouts designed at those resolutions. A single Stage action can set a different layout on every screen in one click.

One consequence worth understanding: because **Screen Preview** is itself a layout element (see the catalogue), a stage layout may contain a live preview of *any other screen, including another stage screen*. That is how ProPresenter builds a multiviewer operator display without a separate feature. Source: archived official guide, and the blog transcript describes exactly that setup ("multi-view" stage screen on a placeholder output).

The **stage message is global, not per-screen.** The API exposes a single string (`GET/PUT/DELETE /v1/stage/message`), and the archived guide describes it as sending to "your screens". Any layout containing a Stage Message object shows it; layouts without one do not. There is no documented way to send different messages to different stage screens.

---

## 2. Element catalogue

These are the objects offered by the `+` button in the top-left of the Stage Editor canvas. Names are as the official guide writes them.

The canonical list below comes from the **archived official user guide** (https://web.archive.org/web/2024/https://learn.renewedvision.com/propresenter/screen-configuration). The current knowledge base article (https://support.renewedvision.com/hc/en-us/articles/360041407794-Using-a-Stage-Screen-to-its-Full-Potential) corroborates most of it but is written as prose and omits Audio Countdown, Playback Marker, Timecode and Label. Where only one source names an item, the table says so.

| Element | What it shows | Configuration options | Both sources? |
|---|---|---|---|
| **Current Slide** | The slide currently on the audience screen. Four sub-types: **Text** (slide text, styled by your text settings), **Preview Image** (static thumbnail of the slide), **Notes** (slide notes entered in the Slide Editor), **Text + Chords** (slide text plus chords, MultiTracks integration only) | Sub-type; full text styling for the Text variants | Yes (KB names Text / preview image / notes) |
| **Next Slide** | The slide after the current one. Same four sub-types | Same | Yes |
| **Screen Preview** | A live render of any configured screen: an audience screen, the announcement layer's screen, or another stage screen | Which screen | Yes |
| **Chord Chart** | A PDF or JPG chord chart attached to the presentation | — | Yes |
| **Stage Message** | The operator's free-text message. Never appears on audience screens | Text styling. Legacy protocol also carries a "message flash" colour and flag (see §5) — **UNCONFIRMED** whether the current editor exposes flash | Yes |
| **Planning Center Live** | A web object showing the PCO Live countdown, current item and next item | **Dark / Light** theme; time format: **Full Item Length**, **End Item on Time**, **End Service on Time**. Multiple instances allowed on one layout | Yes (detail from the PCO article) |
| **Timers** | Any timer created in the Timers show control | Which timer; format (hours/minutes/seconds/milliseconds tokens); **Color Triggers**; text styling | Yes |
| **System Clock** | The computer's clock | Show date and/or time, time format, 12 vs 24-hour | Yes |
| **Video Countdown** | Time remaining on the currently playing video | Format, Color Triggers, **Visibility** condition (e.g. show only when a video countdown "Is playing") | Yes |
| **Audio Countdown** | Time remaining on the currently playing audio file | Presumed same as Video Countdown; **UNCONFIRMED** | Archived guide only |
| **Playback Marker** | Time remaining until the next playback marker fires | **UNCONFIRMED** detail | Archived guide only |
| **Timecode** | Current position of a configured timecode input | **UNCONFIRMED** detail | Archived guide only |
| **Group** | The group of the current slide, the next slide, or the next arrangement | Which of the three. KB adds that a shape can be *filled* with the group colour | Yes (KB frames it as "group color and name of your current or next slide") |
| **Label** | The label of the current or next slide | Which | Archived guide only |
| **Capture Status** | Whether integrated streaming/recording is Active or Inactive | KB adds that a shape can be filled with the **Capture Status Color**, and linked text can show status **or elapsed capture time** | Yes |
| **Shape** | A drawn shape, for branding or for filling with something | Fill (see below), stroke, corner radius etc., same as the Slide Editor | Yes |
| **Text** | A plain static text box, typically used to label the other elements | Full text styling | Yes |

### 2.1 Shape fill sources

A shape's `Fill` checkbox (Shape tab of the inspector) can be set to any of: a colour, a gradient, a media element, a web page, a video input, a specific slide object, the current or next slide image, a screen preview, a chord chart, the Planning Center Live timer, Group Colour, or Capture Status Colour. This is mostly a second route to the same elements above, which is why the `+` menu and the fill menu overlap.

Source: https://support.renewedvision.com/hc/en-us/articles/360041407794-Using-a-Stage-Screen-to-its-Full-Potential

### 2.2 Linked Text sources (the text-side equivalent)

Any text box can be bound to a data source by ticking **Linked Text** in the Text tab. That menu carries the `+`-menu items plus several that appear nowhere else:

- **Slide Count** — the current slide number, the number of slides remaining, or the total.
- **Playlist** — the current item's name, the next item's name, the current header, or the next header.
- **Auto Advance Time** — the remaining auto-advance time on the presentation or announcement layer.
- **Operator Notes** — operator notes for the current item.
- **Capture** — capture status, or elapsed capture time.

Source: same article.

This is architecturally important: **timers on a stage layout are not a distinct object type.** "Timers on a stage layout use Linked Text… We make adding timers a little bit easier on stage layouts by pre-building Timer objects, but they are still created with a text box and linked text, so you can edit them the same way after adding them." Source: https://support.renewedvision.com/hc/en-us/articles/360053250613-Using-Timers-on-Stage-Screens

### 2.3 Positioning and styling

- **Free-form drag on a canvas.** The Stage Layout Editor "works similarly to the main Slide Editor". A new object lands in the centre of the layout; you drag to move and resize, or set size and position numerically in the **Shape** tab. Sources: archived guide; "Full Potential" article.
- **Per-element text styling, identical to the Slide Editor.** The Text tab gives font, font style, size, attributes, colour, scaling options, alignment, stroke, shadow, line settings, list attributes, and the Linked Text source. Source: "Full Potential" article.
- **Linked objects are visually marked** in the editor with a yellow outline and a yellow label at the upper left naming what they are bound to. Source: archived guide.
- **No Build tab.** A stage layout is a single static composition with no animation. Source: archived guide.
- **Per-element Visibility conditions.** Shape tab → `Visibility`, then a condition such as `Video Countdown` / `Is playing`. Objects can therefore be overlapped and shown only when relevant. Source: https://support.renewedvision.com/hc/en-us/articles/360053250613-Using-Timers-on-Stage-Screens
- **Aspect ratio is per-layout, not per-element**: the layout's `Size` property. There is no per-element aspect ratio setting. Source: archived guide.

---

## 3. Timer model

Timers are a **global application feature**, not a stage feature. They live in the **Timers** show control (bottom-right panel), and stage layouts, audience messages, presentations, props and themes all merely *display* them.

Open Timers: the Timers button in Show Controls, `View → Timers`, or `Control-C` on Mac / `Control-Shift-C` on Windows. There is no limit on how many timers you can create. Sources: https://support.renewedvision.com/hc/en-us/articles/360050782494-Setting-up-Timers-in-ProPresenter-7 ; keyboard shortcuts article.

### 3.1 The three types

| Type | What it does | Configuration | Behaviour at the end |
|---|---|---|---|
| **Countdown** | Counts down from a fixed duration | `Duration` in `HOUR:MINUTE:SECONDS`. Typing `5:00` becomes `00:05:00`; typing `1:37:30` becomes `01:37:30`. Leading zeros are added automatically | Reaches zero. With Overrun on, continues into **negative numbers** |
| **Count Down To Time** | Counts down to a wall-clock time of day, using the computer's internal clock | A time, plus a period: **AM**, **PM** or **24-hour**. Entry assumes hours first, so `12` becomes `12:00`. If the target has passed, it counts to that time *tomorrow* (the guide's own example: switching the New Year example from AM to PM makes it count to noon tomorrow) | Reaches zero. With Overrun on, goes negative |
| **Elapsed Time** | A stopwatch, counting up | `Start` time, and an optional `End`. Leave `End` empty to run indefinitely | Stops at `End`. With Overrun on, keeps running past it |

Sources: https://support.renewedvision.com/hc/en-us/articles/360050782494-Setting-up-Timers-in-ProPresenter-7 ; archived official guide `interface#timers`.

The API models these as three mutually exclusive payload shapes on the same object, which is a clean confirmation of the model:

```json
{ "id": {...}, "allows_overrun": true, "countdown": { "duration": 300 } }
{ "id": {...}, "allows_overrun": true, "count_down_to_time": { "time_of_day": 3600, "period": "pm" } }
{ "id": {...}, "allows_overrun": true, "elapsed": { "start_time": 0, "end_time": 120 } }
```

`period` is an enum of `am | pm | 24_hour`. Times are integer seconds. Source: https://openapi.propresenter.com/swagger.json, `POST /v1/timers`.

The legacy WebSocket protocol used a numeric type on the same three: `0 = Countdown, 1 = CountDown to Time, 2 = Elapsed Time` (community source, https://jeffmikels.github.io/ProPresenter-API/Pro7/, `clockUpdate`).

### 3.2 Behaviour

- **Controls.** Each timer has a **Reset** button (circular arrow, reverts to the configured settings) and a **Start** button that toggles to **Stop** while running. A collapsed timer shows only name, reset, remaining time and Start/Stop. Source: the Timers article.
- **Overrun is a per-timer checkbox.** "Check `Allows Overrun` if you would like the Timer to keep running after it reaches zero or its end; uncheck `Allows Overrun` to have the Timer stop when it reaches its end." Source: archived official guide `interface#timers`.
- **Colour in the operator UI.** A running timer is highlighted **green** in the Timers panel; with overrun enabled, it highlights **red** once past its original end time. Source: archived official guide.
- **Colour on screen is not automatic — it is configured.** Stage/linked-text timers carry **Color Triggers**: a list of times, each with a colour. Values are `minutes:seconds.milliseconds` by default; prefix another number and colon to add hours (`1:00.00` → `1:01:00.00`). For Countdown and Count Down To Time the colours apply as time descends; for Elapsed Time as it ascends. The final colour persists through overrun, and a trigger set at `0:00.00` colours the whole overrun period. Source: https://support.renewedvision.com/hc/en-us/articles/360053250613-Using-Timers-on-Stage-Screens
- **Runtime states, from the API:** `stopped | running | complete | overrunning | overran`. Source: `GET /v1/timers/current` in the spec. **Sources disagree**: the same endpoint's worked example uses `"state": "overrun"`, a value not in its own enum, and shows a negative time string `"-00:00:02"`. Treat the enum as authoritative and the example as a documentation bug, but a client should tolerate both.
- **Negative display is real.** The API returns times as strings including the leading minus, so a countdown in overrun genuinely reports negative time rather than clamping.
- **Looping: not documented.** No official source describes a repeat or loop option on a timer. Treat as **not present**, marked **UNCONFIRMED** because absence of documentation is not proof of absence.
- **Persistence: partially confirmed.** Timers are saved configuration, listed on next launch, and migrate from ProPresenter 6 ("Depending on whether you imported anything from ProPresenter 6… you may or may not have existing timers listed"). Whether a *running* timer's elapsed state survives a relaunch is **UNCONFIRMED**.

### 3.3 Display format tokens

Linked-text timers expose four segment controls, left to right: hours, minutes, seconds, milliseconds. Each segment has the same five options (shown here for minutes):

| Option | Meaning |
|---|---|
| `m` | Always show minutes, hide leading zeros |
| `mm` | Always show minutes, show leading zeros |
| `m` (conditional variant) | Show minutes only if there are any, no leading zeros |
| `mm` (conditional variant) | Show minutes only if there are any, with leading zeros |
| `- -` | Hide this segment; its value rolls down into the next-smaller unit |

The guide's worked example: one hour fifteen minutes can be rendered as `01:15:00.00`, `1:15:00.00`, `1:15:00`, `1:15`, `75:00`, `4500`, "plus a few other combinations". Note that `4500` is seconds, which shows what `- -` does: it converts rather than truncates.

Source: https://support.renewedvision.com/hc/en-us/articles/360050786794-How-to-Create-a-Countdown-for-an-Audience-Screen

The placeholder text shown in the editor for a linked timer is a fixed series of numbers and "cannot currently be changed". Same source.

### 3.4 How a timer is started (six routes)

| Route | Detail | Source |
|---|---|---|
| **Timers panel** | The Start/Stop button. The guide itself calls this "the least-used option in most cases" | Timers article |
| **Messages** | A Message containing a timer token has a `Show` button that starts it | Audience countdown article |
| **Playlist Header** | `+` → `New Header` in a playlist. A header can **Start, Stop or Reset** a chosen timer, and with `Set Configuration` ticked can also **rewrite the timer's duration and even change its clock type** before triggering. Click `Trigger` to fire it | Audience countdown article |
| **Slide Action** | Right-click a slide → `Add Action` → `Timer` → pick the timer. Same Start/Stop/Reset plus Set Configuration options. **Fires every time that slide is selected**, so the guide warns against putting it on an active slide in a looping presentation; the workaround is a disabled slide, which is skipped by the loop but can still be clicked manually | Audience countdown article |
| **Remote app / ProPresenter Control** | Both expose start, stop, reset, and editing of duration and overrun | Remote interface and Control articles |
| **API / Macros / Network Link** | `GET /v1/timer/{id}/{operation}` where operation is `start`, `stop` or `reset`; `GET /v1/timers/{operation}` for all timers at once; `PUT /v1/timer/{id}/{operation}` to set details and operate in one call; `GET /v1/timer/{id}/increment/{time}` to modify a running timer. Network Link propagates timer type, values, and start/stop/reset to other machines **by index** | OpenAPI spec; https://support.renewedvision.com/hc/en-us/articles/4412193892627-Network-Link |

### 3.5 How a timer is displayed

| Surface | Mechanism |
|---|---|
| **Stage screen** | A pre-built Timer object on a stage layout, which is really a text box with Linked Text bound to a timer |
| **Audience screen, route 1: Message** | Create a **Theme** with a styled text box, then a **Message** with a timer **token** added via `Add a Token`. The Message's `Show` button puts it over whatever is on screen. Dismiss options control clearing: manual, automatic when the timer expires, or after a set time |
| **Audience screen, route 2: Linked Text** | Bind a text box inside a presentation, theme or **Prop** to the timer directly. More design freedom, and the timer becomes part of the slide rather than an overlay |
| **Operator UI** | The Timers show control panel: name, reset, running value, Start/Stop, green when running, red when overrunning |
| **Web / mobile** | ProPresenter Control panel 9; Remote app Timers tab |

Sources: https://support.renewedvision.com/hc/en-us/articles/360050786794-How-to-Create-a-Countdown-for-an-Audience-Screen ; https://support.renewedvision.com/hc/en-us/articles/6032278869011-Using-ProPresenter-Control ; https://support.renewedvision.com/hc/en-us/articles/43051104879379-ProPresenter-Remote-App-Interface

A design note the documentation makes explicitly, because it catches people out: a countdown shown on an **announcement loop** slide must be started from a Header or the Timers panel, not from a slide action, "to prevent the clock from resetting every time" the loop comes round.

### 3.6 Clocks, and how they differ from timers

**System Clock** is a separate stage element, not a timer:

- It reads the computer's clock. It cannot be started, stopped or reset.
- Its options are date on/off, time format, and 12 vs 24-hour.
- Over the API it is `GET /v1/timer/system_time`, returning a plain integer: seconds since the Unix epoch. It is filed under the `Timer` tag but has no operations.

Sources: https://support.renewedvision.com/hc/en-us/articles/360053250613-Using-Timers-on-Stage-Screens ; OpenAPI spec.

Note the terminology trap: **Count Down To Time is a timer, not a clock.** It reads the system clock to compute its target but it is a startable, stoppable, resettable timer object.

Three further read-only "clock-like" elements exist and are also not timers, because nothing starts them:

- **Video Countdown** — time remaining on the currently playing video. `GET /v1/timer/video_countdown` returns a string like `"00:00:01"`. It runs only while a video is playing, which is why the Visibility condition exists for it.
- **Audio Countdown** — same idea for the currently playing audio file.
- **Playback Marker** — time until the next playback marker fires.

### 3.7 "Timer as a cue" in the playlist

ProPresenter has three distinct things in this area and conflating them is easy:

1. **Timer actions on playlist Headers** — the closest thing to "a timer cue in the plan". A header is a labelled, colourable divider in the playlist that carries a timer operation and optionally a whole timer configuration. Clicking `Trigger` fires it.
2. **Timer slide actions** — the same payload attached to a slide instead of a header, firing on slide selection.
3. **Go To Next Timers** — a completely different feature: per-slide auto-advance. It is not part of the Timers system. Worth knowing because Network Link documents a specific gap: "Go to Next Timers do not trigger content when the timer triggers a slide."

Sources: https://support.renewedvision.com/hc/en-us/articles/360050786794-How-to-Create-a-Countdown-for-an-Audience-Screen ; https://support.renewedvision.com/hc/en-us/articles/4412193892627-Network-Link

### 3.8 Planning Center Live timers (a fourth, external timer kind)

If the operator is signed in to Planning Center and has selected a plan in `View → Planning Center`, a **Planning Center Live** object can be added to a stage layout. It is a web object, not a ProPresenter timer, and it offers three time formats:

- **Full Item Length** — the item's scheduled duration, ignoring real elapsed time.
- **End Item on Time** — dynamically adjusts to show ahead/behind for the current item.
- **End Service on Time** — adjusts the current item so the *service* ends on time without shifting the remaining items.

Plus a Dark or Light theme. Multiple instances can sit on one layout. Control of PCO Live requires clicking `Take Control`, which is gated on the Planning Center account's permission (Editor or higher by default).

Source: https://support.renewedvision.com/hc/en-us/articles/1500006143281-Planning-Center-Live-and-Planning-Center-Live-Stage-Timers

---

## 4. Stage Message

The clearest statement of the model is the archived official guide's, and it is worth quoting because it settles the lifecycle question:

> "Below the selection menu, you can send a Stage Message to your screens. Any object using the 'Stage Message' linked text data link will see this message appear on their screen. Enter your text and then click to **Show** to send this message to the screen. The Show button will toggle to a **Hide** button when the message is active and clicking it will remove the Message from the screens."

And from the element catalogue on the same site: "Hide removes the message from the Stage Layout, but doesn't delete the message."

Sources: https://web.archive.org/web/2024/https://learn.renewedvision.com/propresenter/interface and .../screen-configuration

So the model is:

| Question | Answer |
|---|---|
| Who sends it | The operator, from the **Stage** panel of Show Controls (`Shift-Command-S` on Mac, `Alt-S` on Windows). Also from **ProPresenter Control** panel 14, from the **Remote app** (`More → Stage Screen Layouts`, which can "show/hide a stage message"), and over the API |
| How it appears | Only inside layout objects bound to the **Stage Message** linked-text source. A layout without such an object shows nothing. Never on an audience screen |
| Scope | One global string for all stage screens. No per-screen messages documented |
| How it is cleared | Click **Hide** (same button, toggled). The text stays in the field for reuse. `DELETE /v1/stage/message` over the API |
| Persistence | It stays up until hidden. No auto-clear timer is documented for stage messages, unlike audience **Messages**, which have Dismiss options. **UNCONFIRMED** whether any auto-dismiss exists |

**A trap for anyone mapping this onto a clear model.** `F6` is "Clear Message" in the Presentation menu, and `/v1/clear/layer/{layer}` accepts `messages` as one of `audio, props, messages, announcements, slide, media, video_input`. Both of those are the **audience Messages layer**. The stage message has its own separate endpoint (`/v1/stage/message`) and is not in the clear-layer enum. Whether `F6` also drops the stage message is **UNCONFIRMED**; the API structure strongly suggests it does not.

Sources: keyboard shortcuts article; OpenAPI spec; https://support.renewedvision.com/hc/en-us/articles/6032278869011-Using-ProPresenter-Control ; https://support.renewedvision.com/hc/en-us/articles/43051104879379-ProPresenter-Remote-App-Interface

---

## 5. Remote and mobile model

### 5.1 Network settings: the one place everything is enabled

Current documentation (v20+):

- `Settings → Network`. Toggle the network on; the **IP Address** and **Port Number** are shown there.
- Scroll to the **Remote** section, toggle Remote on, set a password, press Enter to lock it in.

Source: https://support.renewedvision.com/hc/en-us/articles/42892551789843-How-to-Connect-the-ProPresenter-Remote-App

The archived 7.x guide describes a richer, older settings model, which is useful history because it explains the legacy clients:

> "In order to use the ProPresenter Remote app for iOS and Android you will need to check the box to **Enable ProPresenter Remote** and enable the mode(s) that you want available of **Controller** and/or **Observer**. You can also set a Password for each login option. **A password is only required for the Controller login option.** … **Enable Stage Display App** allows you to use the ProPresenter Stage Display app for iOS, Apple TV and Android. You can set a Password here to be used to login to the App."

Source: https://web.archive.org/web/2024/https://learn.renewedvision.com/propresenter/preferences

So the password model changed: **two roles with two passwords (observer optional) in 7.x, one Remote password in v20+.** The Remote Classic article confirms the old model was live: "You can set separate passwords for controlling or observing the ProPresenter application from this screen… In 'Observe' mode, the application will follow along with the slides as they are advanced by the operator. In 'Control' mode, selecting a slide… will cause ProPresenter to switch to that presentation and select that slide immediately." Source: https://support.renewedvision.com/hc/en-us/articles/43163349444755-ProPresenter-Remote-Classic

**Port numbers: sources disagree.** The Control article's worked example uses **1025** ("the IP Address would be 192.168.1.3 and the Port Number would be 1025"). The OpenAPI spec's server variable defaults to **50001**, described as "Port set in ProPresenter's Network Preferences". Both are just defaults in documentation; the authoritative value is whatever the Network tab shows on the machine. Do not hard-code either. Sources: https://support.renewedvision.com/hc/en-us/articles/6024791423763-Connecting-to-ProPresenter-Control ; https://openapi.propresenter.com/swagger.json

### 5.2 ProPresenter Remote (the current app)

| Aspect | Detail |
|---|---|
| Platforms | iOS App Store, Google Play, and Apple Silicon Macs via the macOS App Store's iPhone/iPad apps filter (not Intel) |
| Requires | ProPresenter **version 20 or higher**. Older installs use **ProPresenter Remote Classic**, which connects up to version 20 and is no longer sold |
| Discovery | The computer "should appear under **Available Connections**". Tap it, enter the password, tap Connect. Swipe left on a connection to delete it |
| Discovery mechanism | **UNCONFIRMED.** Renewed Vision's docs never name the protocol. Bonjour/mDNS is the obvious candidate and the app's behaviour matches it, but no official source says so. The macOS Network Link article does tell users to allow ProPresenter under `System Settings → Privacy & Security → Local Network`, which is macOS's mDNS/local-discovery permission, which is suggestive but not proof |
| Manual fallback | `Manual Connection` → IP Address, Port Number and Password, all taken from the Network tab |
| Password model | **One** password, set alongside the Remote toggle. No observer/controller split documented in the current app |
| Connection limit | "There is no technical limit on the ProPresenter side for number of remote connections", though the troubleshooting section suggests network bandwidth can be the real limit |

Source: https://support.renewedvision.com/hc/en-us/articles/42892551789843-How-to-Connect-the-ProPresenter-Remote-App

**What the app shows**, from https://support.renewedvision.com/hc/en-us/articles/43051104879379-ProPresenter-Remote-App-Interface:

- **Presentation tab** (default). Toggles between **Library view** (browse all presentations, trigger slides) and **Playlist view** (expand any library playlist, tap an item to open, tap a slide or a play button to trigger).
- **Eraser icon** (bottom right) → Clear Audio, Clear Messages, Clear Props, Clear Announcements, Clear Slides, Clear Media, Clear Live Video; tap the layers icon at the bottom of that menu to reach **Clear Groups** instead.
- **Three-dot menu** (top right) → Grid/List view, **Follow Presentation** toggle (the app follows the operator's selection), slide size slider.
- **Bottom tabs**, user-reorderable via `More → Edit` (drag an orange element onto the tab you want to replace):
  - **Timers** — start, stop, reset any timer; tap one to configure type, duration and overrun.
  - **Messages** — trigger pre-built messages; tap a message with a timer in it to configure that timer.
  - **Props** — tap to show, tap again to hide.
  - **More** — Audio Bin, **Stage Screen Layouts** (change the active layout per stage screen, and show/hide a stage message), Looks, Macros, Settings.
- **Remote tab** (under More) — current slide and next slide only, tap the next slide to advance. This is the presenter-facing view: "great for presenters who want to control slides directly without navigating through the full presentation interface."

### 5.3 QR-code pairing

**Not found.** No Renewed Vision source describes QR-code pairing for ProPresenter Remote, ProPresenter Control, or the Stage app. Pairing is by discovery-plus-password or by IP/port/password. Marked **UNCONFIRMED** rather than "does not exist", since absence of documentation is weaker evidence than a denial.

### 5.4 ProPresenter Control (the browser page)

This is the closest thing to an official web client, and it is an **operator** page, not a stage view.

- Requires ProPresenter **7.9.1 or above** and the same local network. Renewed Vision recommend a wired connection.
- Enable the network in `Settings → Network`, note IP and Port.
- Go to **control.propresenter.com** (needs internet), or, fully offline, `View → ProPresenter Control` in the app's menu bar. Enter IP and Port, click Connect.
- **No password is documented for ProPresenter Control.** The connect flow is IP plus port only. That is a meaningful security note for anyone modelling this. **UNCONFIRMED** whether it silently reuses the Remote password.
- The page is **not customisable** as of 7.9.1; the panels cannot be removed or resized.

Sources: https://support.renewedvision.com/hc/en-us/articles/6024791423763-Connecting-to-ProPresenter-Control ; https://support.renewedvision.com/hc/en-us/articles/6032278869011-Using-ProPresenter-Control

Its sixteen panels, verbatim from the article, because the set is itself a good summary of ProPresenter's controllable surface: Looks, Macros, Audio Transport Control, Audio Playlists, System Time, Capture and Streaming, Screen Control (toggles graphics outputs assigned as audience or stage screens; explicitly **does not** affect SDI, Syphon or NDI outputs), Library Playlist (**view only**), **Timers** (view, configure, start/stop/reset, change length, allow or disallow overrun), Messages (select, fill tokens, Show), Announcement Layer Transport, Presentation Layer Transport, **Stage Screen Controls** (per-screen layout dropdown), **Stage Message** (type and show), Props, and the connected instance name with a disconnect gear.

### 5.5 A browser-based or mobile *stage* view

This is the weakest-documented area, and the honest answer is that Renewed Vision's current public documentation does not describe one.

- **ProPresenter Control is not a stage view.** It controls stage screens; it does not render a stage layout.
- **The ProPresenter Stage app existed.** The archived preferences page documents an `Enable Stage Display App` toggle with its own password, for "the ProPresenter Stage Display app for iOS, Apple TV and Android". A 2022 Renewed Vision blog transcript says "all of those Stage Layouts can be viewed on the Stage App for iOS and Android". An App Store listing at `apps.apple.com/us/app/propresenter-stage/id1131223314` still resolves.
- **Its documentation has been withdrawn.** The knowledge base article "Stage Display mobile app instructions" (`/articles/360011795553-…`) now redirects to a **sign-in wall**. The `renewedvision.com/propresenter/mobile-apps/` page 302s to the general ProPresenter page, which markets only ProPresenter Remote. The Mobile Apps knowledge-base section contains only two articles, both about ProPresenter Remote Classic.
- **Verdict: UNCONFIRMED whether the ProPresenter Stage app is still supported in versions 20/21.** The signals point to quiet retirement, but no source says so, and the App Store listing is still up.

Sources: https://web.archive.org/web/2024/https://learn.renewedvision.com/propresenter/preferences ; https://www.renewedvision.com/blog/video-how-to-create-stage-screens-in-propresenter-7 ; https://support.renewedvision.com/hc/en-us/sections/360002436033-Mobile-Apps

**The underlying protocol (community-documented, ProPresenter 7.6 era — treat as historical).** Third-party stage clients such as ProWebStage work by speaking a WebSocket protocol ProPresenter has never officially documented:

- Endpoint: `ws://<host>:<port>/stagedisplay`, alongside `ws://<host>:<port>/remote`. Both channels run on the **same** port, the one in Network settings.
- Authenticate first: `{"action":"authenticate","protocol":701,"password":"…"}`. The stage channel uses the **Stage** password, which is separate from the Remote password.
- Plain `ws://`, never `wss://`, which is why ProWebStage's README says the client cannot be hosted over HTTPS.
- The server pushes two message kinds: `sl` (the selected stage layout) and `fv` (frame values). A layout carries `nme` (name), `uid`, `brd` (draw borders and labels), `zro` (strip zeros from times), `ovr` (use overrun colour), `oCl` (overrun colour), and `fme`, an array of frames.
- Each frame carries `ufr` (geometry as `{{x1,y1},{x2,y2}}` in **screen percentages**, deliberately not valid JSON), `mde` (0 static image, 1 text, 2 live slide), `tCl` (text colour), `tSz` (font size), `nme`, and `typ`.
- `typ` enumerates the element kinds: **1 current slide, 2 next slide, 3 current slide notes, 4 next slide notes, 5 Stage Message, 6 Clock, 7 Timer Display (uid names the timer), 8 Video Countdown, 9 Chord Chart.** Message frames additionally carry `fCh` (use message flash) and `fCl` (flash colour).
- Frame *values* arrive with an `acn` code: `cs` current slide text, `csn` current slide notes, `ns` next slide text, `nsn` next slide notes, `msg` stage message, `sys` system clock, `vid` video countdown, `tmr` timer value, `cc` chord chart.

Source: https://jeffmikels.github.io/ProPresenter-API/Pro7/ ; corroborated by https://github.com/L2N6H5B3/ProWebStage

Note the gap between that nine-value `typ` enum (7.6) and today's element catalogue, which adds Screen Preview, Planning Center Live, Audio Countdown, Playback Marker, Timecode, Group, Label and Capture Status. Either the protocol grew or the newer elements never got protocol codes. **UNCONFIRMED**, and a reason to treat the legacy WebSocket protocol as unreliable for a modern integration.

---

## 6. The official API surface, for reference

Renewed Vision publish an HTTP API (Swagger UI at https://openapi.propresenter.com/, spec at https://openapi.propresenter.com/swagger.json, and in-app via `Settings → Network → API Documentation`). There is also a TCP/IP wrapper for systems with no HTTP client: each request is a one-line JSON object terminated CRLF, of the shape `{"url":"v1/stage/message","method":"PUT","body":"This is the stage message","chunked":false}`, with `chunked: true` turning an endpoint into a stream of one-line updates. Source: https://support.renewedvision.com/hc/en-us/articles/31606866768147-TCP-IP-Connections-with-ProPresenter-API

Endpoints relevant to this document:

**Stage**
```
GET    /v1/stage/message                        current stage message (empty string = none)
PUT    /v1/stage/message                        body is a plain JSON string
DELETE /v1/stage/message                        hide it
GET    /v1/stage/layout_map                     layout per screen
PUT    /v1/stage/layout_map                     set layouts (match by uuid, name or index)
GET    /v1/stage/screens
GET    /v1/stage/screen/{id}/layout
GET    /v1/stage/screen/{id}/layout/{layout_id} (a GET that mutates — yes, really)
GET    /v1/stage/layouts
DELETE /v1/stage/layout/{id}
GET    /v1/stage/layout/{id}/thumbnail
```

**Timer**
```
GET  /v1/timers                       all timer definitions
POST /v1/timers                       create
GET  /v1/timers/current               current time and state for every timer
GET  /v1/timers/{operation}           start | stop | reset, all timers
GET  /v1/timer/{id}                   one definition
PUT  /v1/timer/{id}                   update
DELETE /v1/timer/{id}
GET  /v1/timer/{id}/{operation}       start | stop | reset
PUT  /v1/timer/{id}/{operation}       update and operate in one call
GET  /v1/timer/{id}/increment/{time}  adjust a running timer
GET  /v1/timer/system_time            epoch seconds
GET  /v1/timer/video_countdown        "00:00:01"
```

**Status, useful for anything that mirrors a stage display**
```
GET /v1/status/slide            { current: {text, notes, uuid}, next: {text, notes, uuid} }
GET /v1/status/stage_screens    boolean, all stage screens on/off
PUT /v1/status/stage_screens
GET /v1/status/screens
GET /v1/status/layers
POST /v1/status/updates         aggregate several streaming endpoints into one stream
GET /v1/presentation/chord_chart/updates    chunked chord chart updates
```

`GET /v1/status/slide` is the single most useful endpoint for anyone building a confidence monitor: it hands over current and next slide text **and notes** in one call, which is the core of most stage layouts.

---

## 7. Summary of everything marked UNCONFIRMED

1. Whether the **ProPresenter Stage** mobile/Apple TV app is still supported in versions 20/21. Its KB article is behind a sign-in wall and the mobile-apps product page is gone, but the App Store listing resolves.
2. Whether the legacy `ws://host:port/stagedisplay` protocol still exists and still works in v20/21, and whether it ever gained codes for the newer element types.
3. Whether `Enable Stage Display App` and its separate password still exist in the v20+ Network settings.
4. Whether ProPresenter Remote's automatic discovery uses Bonjour/mDNS. Strongly implied by the macOS Local Network permission requirement; never stated.
5. Whether **ProPresenter Control** is protected by any password at all.
6. Whether `F6` (Clear Message) clears the **stage** message as well as the audience Messages layer. The API's separation of `/v1/clear/layer/messages` from `/v1/stage/message` suggests not.
7. Whether stage messages support any auto-dismiss, and whether the "message flash" colour from the legacy protocol is exposed in the current Stage Editor.
8. Whether a timer supports **looping**. Nothing documents one.
9. Whether a **running** timer's state survives an application relaunch. The definitions certainly persist.
10. Configuration detail for **Audio Countdown**, **Playback Marker** and **Timecode** stage elements. Only the archived guide names them, in one sentence each.
11. The default port. Documentation says 1025 in one place and 50001 in another.

## 8. Points where sources actively disagree

| Topic | Source A | Source B | Resolution |
|---|---|---|---|
| Default network port | KB Control article: **1025** | OpenAPI spec server variable: **50001** | Neither. Read it from `Settings → Network` at runtime |
| Timer state values | Spec enum: `stopped, running, complete, overrunning, overran` | Spec's own example: `"overrun"` | Enum is authoritative; tolerate both in a client |
| `PUT /v1/stage/layout_map` | Summary says it sets "the specified stage **message**" | Request body is unambiguously a layout map | Documentation bug in the summary text |
| Stage element list | Archived official guide: 17 element kinds | Current KB prose: omits Audio Countdown, Playback Marker, Timecode, Label | Archived guide is more complete; the KB article is prose, not a catalogue |
