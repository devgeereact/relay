# Running a service with Relay

For the person in the booth. No technical knowledge assumed — if a sentence here needs you to know what a "localhost" is, it is a bug in this page and not in you.

If you are looking for how Relay is *built*, that is [ARCHITECTURE.md](ARCHITECTURE.md).

---

## The 10-minute setup, once

You only do this the first time.

### 1. Install it, and open it

Download the installer for your computer and run it. Open Relay. It will walk you through the next three steps itself — this page is here in case you skipped the wizard or want to change something later.

> **If your computer warns you** that Relay is "damaged", "unrecognised", or from an "unknown developer" — that means you have an unsigned test build, not a real release. Ask whoever gave it to you. Do not fight the warning.

### 2. Download the speech model — Relay cannot hear without it

**This is the one step nobody expects, and Relay does nothing useful until it is done.**

Relay listens to the sermon *on your computer* — it does not send audio anywhere, and it does not need the internet during a service. But to do that, it needs a **speech model**: a single file, about **148 MB**, downloaded once.

Go to **Settings → Speech**, and press **Download**. It shows a progress bar. You can cancel it and it will pick up where it left off next time.

Until you do this, Relay still works as a *manual* tool — you can type a verse and put it on the screen, run a service plan, everything except listening. It will tell you so, plainly, rather than pretending.

Do this **before** Sunday. It is 148 MB over the church wifi.

#### If Relay keeps mishearing

The recommended model is the small one, because it runs on any laptop. If Relay is
mishearing a lot — especially over a poor microphone, or in Yoruba, Swahili or
Hausa — Settings → Speech lists larger models that hear more accurately. They are
bigger downloads and they need a faster computer.

Two things worth knowing before you switch:

- Relay marks the one it is actually using **In use**. Downloading a model switches
  to it; you can switch back at any time, and it takes effect immediately.
- If a model would be too slow for your computer, Relay says so **on that model**,
  before you download it. Take that seriously: a model that cannot keep up does not
  stop — the transcript just quietly thins out, which is much harder to notice
  mid-service than an error would be.

### 3. Point it at the projector

Open the **Outputs** tab → **Screens**.

Relay treats every output as a *screen* — the main projector, a stage monitor, an OBS stream, the preacher's phone. Each one can look different.

For a projector plugged into your laptop: pick the display, press **Open**. A fullscreen window appears on it.

### 4. Choose the microphone

**Settings → Audio.** Pick the input. **A bar should move when someone speaks.**

If the bar does not move, Relay cannot hear, and nothing else on this page will work. Fix that first.

> **Use the sound desk feed if you can**, not the laptop's built-in mic. The laptop mic mostly hears the room; the desk feed hears the preacher.

---

## The Sunday morning path

### Before the service

**Build a plan** (optional) — the **Planner** tab. Songs, scripture, announcements, a countdown. This is a Tuesday job, not a Sunday one. **Nothing in the Planner can reach a screen** — it is a workbench, deliberately.

**Rehearse it** — on **Live**, turn on **Rehearsal**. Everything works exactly as it will in the service, and **nothing reaches the congregation's screens**. Practise the whole thing. The screen turns purple and says so, constantly, so you cannot forget which mode you are in.

> Leaving rehearsal **clears the screens**. That is on purpose: they have been showing whatever was on them before you started, and handing you back a live wall you have not looked at in twenty minutes is how the wrong thing ends up in front of a congregation.

**Two things worth doing once, before your first Sunday:**

- **Practise the drills.** The **Help** tab has six of them, and they use the real controls on the
  real surface, in rehearsal. The panic controls come first — before anything about firing
  verses — because the key you need under pressure is the one you should not be reading about at
  the time. It is not a simulated service: Relay cannot produce a sermon, and practising against
  a fake would teach you the shape of the fake.
- **Run the path check.** **Settings → Dashboard → say one verse.** Relay watches the six stages
  between the microphone and the screen and tells you which of them were reached. The twenty-one
  launch checks can all pass on a machine where nothing works end to end — a microphone the
  operating system has muted, an output window on a display that is asleep. This is the thing
  that finds that at 10:05 instead of 10:31. It only runs in rehearsal.

**Save your room.** **Settings → Audio → Rooms → Save this room** remembers the microphone,
the language, the service length, the voice profile and which display each screen is on. Applying
it back reports **which pieces did not take** — a projector that moved to another port comes back
as four of six restored, named, rather than as a green tick over a dark wall. It deliberately does
**not** store the audio levels: Relay learns those live, every service, because a level that was
right in an empty hall is wrong once it fills with people.

### During the service — you live on one tab

**Live** is the only tab you need. Everything is on it.

| What | How |
|---|---|
| **Start listening** | Press **Start listening**. Relay transcribes as the preacher speaks. |
| **The preacher quotes a verse** | Relay hears it and offers it. Press **A** to put it on the screen, **D** to dismiss it. |
| **Type a verse yourself** | The box at the bottom. `John 3:16`, `Ps 23`, `rom 8 1` — it understands all of those. Press Enter. |
| **Read on** | `→` walks to the next verse. `←` goes back. |
| **Run the plan** | `→` steps the plan when plan content is on screen. **The bar tells you which** — it says SLIDE or VERSE. |
| **Get it off the screen NOW** | **`Esc`**. Works on every tab, always. |
| **Kill the screen completely** | **`B`** (blackout). |

### The two things worth understanding

**Relay tells you *how* it heard something, and you should look.**

- **"Heard the reference"** (gold) — the preacher *said* "John three sixteen". Relay shows you the words it heard and how confident it is.
- **"Paraphrase — a guess"** (blue) — nobody said a reference. Relay matched the *meaning* against a verse, and it shows you which words made it think so. **There is no confidence number**, because the number would not mean anything. Read it, and decide.

A paraphrase **never** goes on screen by itself. Ever. Only a reference Relay actually *heard* can do that, and only if you have left auto-fire on.

**A half-reference waits for you.** If the preacher says *"turn to Psalm twenty-three"* — a book and a number, with no "chapter" or "verse" — Relay **offers** it rather than putting it up on its own. That is deliberate. Preachers say book names and numbers constantly without meaning a reference: *"Matthew, one of the twelve"*, *"number one… number two…"* — and "Numbers" is a book of the Bible. Say **"Psalm chapter twenty-three"** or **"Psalm twenty-three verse one"** and it goes straight up, because now you have said you mean it.

**"Related" chips are not detections.** At the bottom of the feed you may see a few verses under a theme — *"Related · Fear & Anxiety"*. Nobody said those. Relay is offering them because of what is being preached about. They go nowhere until you click one.

---

## When it goes wrong

Live software fails live. These are the things that actually happen.

| What you see | What it means | What to do |
|---|---|---|
| **The bar doesn't move when someone speaks** | Relay cannot hear. Nothing else will work. | Settings → Audio. Try a different input. Check the cable and that the desk is sending. |
| **The transcript is nonsense** | Usually the mic is too quiet, not the AI being bad. | Get a stronger feed. Relay adapts to a quiet room, but it cannot invent a signal that is not there. |
| **"Relay keeps changing its mind about the language"** | On a strong accent, leaving the language on **Auto** makes Relay re-guess every few seconds — and a wrong guess garbles the transcript, which is where most wrong verses come from. | **Settings → Scripture & Bible → Recognition Language.** Pick the language instead of Auto. This is the single biggest accuracy win for an accented preacher. |
| **Wrong verses keep appearing on the wall** | Relay heard a book name and a number in ordinary speech — *"Matthew, one of the twelve"*, *"number one… number two…"*. | Turn the **sensitivity dial down** on Live. If it is still noisy, press the **Armed** chip to disarm auto-fire — everything else keeps working and you fire by hand. And fix the language above first, because that is usually the real cause. |
| **"No speech model loaded"** | Step 2 above never happened. | Settings → Speech → Download. Manual override still works meanwhile. |
| **Nothing appears on the projector** | The output window is not open, or it is on the wrong display. | Outputs → Screens → pick the display → **Open**. |
| **OBS / the kiosk screen is blank** | The browser source is pointed at the wrong address. | Use the **Copy URL** button in Outputs → Screens — it is the only thing that fills in the right numbers. The address looks like `http://<this-computer>:8032/output.html?channel=1&template_id=1`. **Not 5032**, and do not drop the `channel=` part: without it the screen still shows verses, and then quietly ignores you every time you change its template. |
| **"The screens may still be live"** (red bar) | A clear or blackout **failed**. Relay is telling you rather than pretending. | **Look at the actual screen.** Clear it from the output window if you have to. |
| **`→` says it did nothing** | End of the passage, or nothing is on screen yet. Relay says which. | Fire a verse first, or step back. |
| **Relay crashed mid-service** | The console crashed. **The output screens are separate — the congregation still sees the verse.** | Press **Recover**. It puts you back where you were. |

### The panic keys

**`Esc` clears everything. `B` blacks everything out. They work on every tab, always** — even if the screen you are on has broken.

`Esc` works even while you are typing. **`B` does not** — otherwise typing "Habakkuk" would black out the church on the second letter.

---

## The other tabs

You will not need these during a service.

- **Library** — your songs, saved verses, media, announcements, and the history of past services.
- **Planner** — build a service plan. Cannot reach a screen.
- **Outputs** — three panes: **Screens** (one row per output screen — the projector, a stage monitor, OBS, the preacher's phone), **Content looks** (which template scripture, lyrics, media and announcements wear by default), and **Sharing** (the LAN address and the preacher's stage remote). Set up once.
- **Templates** — what scripture *looks like* on the wall. What you see in the editor is exactly what the projector shows. **Import** a design someone shared, or **Export** one of yours to a file (⋮ menu / the preview panel) to hand to another church.
- **Themes** — the *look beneath* your templates: fonts, colours, spacing, motion. Pick a theme (eight are built in), tweak a copy in the theme editor, then apply it to a template — the template inherits the theme and you override only what you want. Themes export and import as files too. A theme never reaches a screen on its own; it dresses a template, and the template is what fires.
- **Settings** — audio, speech model, sensitivity, console language, and eight more sections. The
  four worth knowing about: **Dashboard** (is this machine ready, and the path check above),
  **Languages** (how much of Yorùbá, Kiswahili and Hausa Relay actually knows — including the
  columns that are honestly empty), **Privacy** (what is on this machine and what can leave it),
  and **Diagnostics** (the live numbers, and a **one-file export** you can attach to an email
  when something goes wrong — it contains nothing about your church, by construction).
- **Help** — the same guide, inside the app, where you can read it without the internet, plus the
  six practice drills and an honest account of **what the AI is bad at**.

### Two things you will notice mid-service and should not worry about

**Some buttons stop working while a service is recording.** Every **Delete**, and anything that
would take the speech engine away — changing or downloading a speech model, switching the Bible
translation, importing media — is held back until the service ends. It tells you why, and you can
lift it in one action. **Nothing you use to run a service is affected**: firing, the arrow keys,
clear, blackout, rehearsal, the sensitivity dial, opening and closing screens, and changing a
screen's template all work exactly as normal — including editing a template, because an
unreadable verse on the wall is fixed by changing its look, during the service, which is when you
find out.

**A verse can be reported as "shrunk".** Relay fits text by shrinking it until it stops
overflowing, and it will always still show the verse rather than blanking a screen. But if it has
had to shrink below **45% of the size your template asked for**, it says so and tells you how
small it went — because a template that had quietly stopped working used to look exactly like one
that was working. If you see it, that passage is longer than the template was designed for.

---

## Afterwards

**Library → History** holds every service. Open one and you get:

- **The Sunday report** — how long, how many verses, how many Relay decided by itself versus how
  many you fired, and how fast the transcript was keeping up. **Only what was actually measured
  appears.** A blank is shown as "—" and never as 0, because a report that shows zero for
  something nobody measured is a report that gets better as the system gets worse.
- **Replay** — click any line in the timeline to see what was being said around it, what Relay
  decided, and how fast it was going. It replays the *record*, not the audio: Relay never stores
  what was said aloud.
- **Week on week** — whether the transcript is keeping up better or worse than last Sunday.

**If something went wrong, send the diagnostic file** (Settings → Diagnostics → export) rather
than a photograph of the screen. It is built by naming every field that may be in it, so it
cannot leak a transcript, a verse, a lyric, an announcement or your service titles.

---

## Screens for the platform and the booth

The congregation's wall is not the only screen you can drive. A **stage display**
(facing the preacher) and a **confidence monitor** (facing the booth) are just
templates — start a new template from the **Stage Display** or **Confidence Monitor**
preset, then assign it to a screen in **Outputs** exactly like the projector.

These monitors show what the wall cannot:

- the **current verse and its reference**, large enough to read from the platform;
- the **verse coming up next** — bounded by the reading, so a "John 3:16–17" reading
  shows nothing after 3:17 rather than running on into the next verse;
- the **wall clock** and a **service timer** counting up from when you started recording;
- your **private operator note** for the cue (never shown to the congregation).

They inherit their theme like any template, so a monitor matches your house style.
One thing to know: pressing **Clear all screens** clears the monitors too — the panic
key is deliberately total, so it clears *everything*, monitor timers included.

---

## Things Relay promises you

- **It works with the internet unplugged.** All of it. The listening, the detecting, the screens.
- **Your sermon audio never leaves the computer.** It is transcribed on the machine and thrown away. See [PRIVACY.md](../PRIVACY.md), which says exactly what is and is not sent, and means it.
- **It will not put a verse on the wall because it *guessed*.** Only because it *heard*, or because you told it to.
- **It tells you when something failed.** A control that lies is worse than one that breaks, and this is software that fails in front of five hundred people.
