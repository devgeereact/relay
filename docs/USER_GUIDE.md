# Relay — Operator Guide

How to run a service with Relay. For how it's built see [ARCHITECTURE.md](ARCHITECTURE.md).

> **The app is a native window.** `localhost:5032` in a plain browser is a dead console — only the desktop app window has the engine attached. Browser sources (OBS/kiosk/phone) use the *output* and *stage* pages, which work anywhere on the LAN.

---

## The five screens

The left nav switches between: **Console**, **Library**, **Planner**, **Channels**, **Templates**, **Settings**. Operator override is always one action away.

---

## 1. Console — drive the live service

The default screen. What you use during a sermon.

- **Listen** — turns the mic on. With **Detection** also on, Relay auto-drives from what it hears.
- **Live transcript** — a rolling view of what's being said.
- **Suggestions** — mid-confidence scripture matches appear as cards. **Confirm** pushes one live; **Dismiss** drops it. High-confidence matches fire automatically (configurable).
- **Manual override** — type any reference (`John 3:16`, `Ps 23`, `rom 8 1`) and fire it instantly. Always available, even with detection off.
- **Prev / Next** — step through a passage (also driven by "next"/"back" spoken aloud).
- **Monitors** — up to 4 live output previews (the real thing, same renderer as the screens).
- **Control bar:**
  - **Clear all** (`Esc`) — clears every screen to the template background (transparent templates key out for OBS).
  - **Black** — opaque blackout (kills the screen entirely); next content/clear cancels it.
  - **Countdown** — set minutes, click **Start** → it arms ("Confirm?") → click again to fire a pre-service countdown on every screen. One runs at a time; **Clear all** to start another.
  - **Open output** — opens a fullscreen output window.

---

## 2. Library — your content catalog

Sub-tabs across the top. Two constant actions:

- **Import** (any file, sorted automatically): `.pro`/`.proplaylist`/text → Lyrics (with a pre-save review), images/video → Media, PDF/PPTX → Media documents.
- **＋ New** menu: **Paste / draft song**, **Save scripture**, **Draft announcement**.

### Scripture
Verses you've **saved** (not the whole Bible). Search a word, phrase, or paraphrase — the matching verse shows immediately with suggestions ("the lord is my shepherd" → Psalm 23:1). Save the ones you want on hand.

### Lyrics
Your song catalog.
- Click a song → the **Song Editor**: a ProPresenter-style **slide-flow grid**, colour-coded by section (Verse/Chorus/Bridge…). Edit text/tag/order, **drag to reorder**, **Shift+Enter** (or the Split button) splits a slide at the cursor. Save replaces the song's sections everywhere in real time.
- **Arrangements** — named play-orders of the sections (e.g. "Sunday AM" = `V1 C V2 C Bridge C`). Build one in the editor's Arrangements bar (click sections to append, reorder, save). "Standard" (all sections in order) is always available.
- **＋ Plan** on a song card adds it to a service plan; if it has arrangements you pick which one.

### Media
Imported images, video, and documents. **▶ To output** fires an image/video full-screen as a background. Two-step **delete**.

### Announcements
Notice slides (title + body). Draft/edit inline, **▶ To output** to fire, edits propagate to any plan that uses them.

### History
Past service sessions — what was said, detected, and fired.

---

## 3. Planner — build and run a service

A **Mission-Control** editor: the plan's cues on the left, the selected cue's full slide flow in the centre, and the live output on all four styled monitors on the right.

- **Plans list** — create, **duplicate** (clone last week's order as a starting point), delete.
- **Add** — one search across **scripture, songs, media, announcements**, plus a **Countdown** quick-add (set minutes → ＋ Add). Songs with arrangements show the picker.
- **Cues** — drag to reorder (or ↑/↓), remove. Each cue can carry a **stage note** (e.g. "hold for prayer") — shown only on the stage/confidence monitor, never on the congregation screen.
- **Run it** — click a slide to fire it live; **transport** (Prev/Next/Clear) and keyboard (→/PageDown/Space = next, ←/PageUp = prev, Esc = clear) drive the service. The four monitors show exactly what each styled screen is showing. The **LIVE** pill marks what's on screen; "up next" is pushed to the stage monitor.

Every cue type — scripture, song, media, announcement, countdown — creates, plans, reorders, and fires through the same pipeline.

---

## 4. Channels — where output goes

Every output is a render target of one shared template engine — main screen, stage, streaming, lobby all pull from the same source, styled per channel.

- **Add a channel** — name it, pick **native window (HDMI)** or **network client (OBS/kiosk)**.
- **Assign a template** per channel.
- **Native** → pick which HDMI display, then **Open**.
- **Network** → **Copy URL** or show a **QR** for a browser source / kiosk / phone (same Wi-Fi). Point OBS/vMix at `http://<host>:5032/output.html?template_id=<n>`.
- Two-step **delete** (first click arms → "Sure?" → confirm).
- **Preacher's stage remote** — open `http://<host>:5032/stage.html` (or scan the QR) on a phone/iPad: big live verse, "up next", your stage notes, and the countdown — the confidence view, kept off the main output.

---

## 5. Templates — how output looks

Edit the output templates; the preview is the **exact** renderer used live (WYSIWYG).

- **Regions** (reference / verse text) and order, alignment, **lower-third band** toggle.
- **Style** — font, background (color/gradient/image or transparent), accent, verse/reference colors and sizes (in `cqw`, so they scale to any screen), italic reference, **transition** duration.
- **Content Templates** (in Settings) map each content type (scripture / song / announce) to a default template — so lyrics use the lyric look, scripture the scripture look, automatically.
- Two-step **delete**; deleting a template unassigns it from any channel.

Long verses **auto-fit** — they shrink to fit, never clip or spill off screen. The congregation screen shows no titles or slide numbers (those live in the operator UI and stage monitor).

---

## 6. Settings

- **Audio** — input device, STT language.
- **Detection** — sensitivity / confidence thresholds (self-calibrating; seeds are placeholders until tuned).
- **Translation** — active Bible translation.
- **Content Templates** — per-content-type template defaults.
- **Voice profiles** — per-operator STT/threshold profiles.
- **Network info** — the LAN address for kiosk/OBS/stage URLs.

---

## Typical Sunday flow

1. **Planner** → duplicate last week's plan (or build a new one): countdown → opening songs → sermon (detection on) → closing → announcements.
2. **Channels** → open the main HDMI output; add an OBS browser source for the stream; open the stage remote on the preacher's tablet.
3. Pre-service: **fire the countdown** cue.
4. Service: run the plan; during the sermon, **Listen + Detection** auto-suggest scripture — **Confirm** to push, or **manual override** to type a reference. **Prev/Next** through passages.
5. Anything unexpected: **Clear** or **Black** in one tap.

Everything works with **zero internet**. Nothing leaves the device without an explicit, visible reason.
