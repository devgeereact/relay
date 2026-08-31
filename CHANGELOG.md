# Changelog

What changed, and why it matters to the person in the booth.

**This file is read by operators, not only by developers.** When Relay updates itself, the release notes are what a volunteer sees before deciding whether to restart the app twenty minutes before a service. So entries say what *changed for them*, not which function was refactored. If a change cannot be explained in those terms, it does not belong under a heading a user reads.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [Semantic Versioning](https://semver.org/), with one project-specific rule: **pre-release identifiers must be numeric** (`0.1.0-1`, not `0.1.0-rc1`) — the Windows MSI bundler rejects named ones, fifteen minutes into a release, on the platform most of our churches are on.

---

## [Unreleased]

### Relay may now be used in a church — with someone watching

The decision on whether Relay is fit to run a service used to be an open-ended
"not yet". It is now specific: **not for general release, yes for a supervised
pilot** — two churches, a named operator at each, and every service watched by
somebody who can clear the wall by hand.

What that means if you are the operator: Relay is good enough to help you, and it
is not good enough to leave alone. It put a wrong verse on a screen during its
first real service. That particular fault is fixed and cannot come back without a
test failing, but nobody has yet measured how well it hears in any language, and
you would be the first person outside its author to run a service on it.

Before your first live use: run a rehearsal, use the path check, and read
Settings → Diagnostics afterwards. If a wrong verse does reach the screen, the
"heard" text beside it in the service history is exactly what is needed to stop
it happening again — that is how the one from the first service became a
permanent test.

Windows is not signed yet, so this applies to macOS only.

### Your service report can finally say whether Relay was any use

The report after a service used to say **0 suggested · 0 dismissed** — for every
service, always. Not because Relay never suggested anything, but because nothing
was writing it down. A zero there does not read as "we didn't record that". It
reads as "Relay never offered you anything", which is close to the opposite of
what happened.

Now Relay records what **you** did: when you took a suggestion, and when you turned
one down. Rejecting one used to leave no trace anywhere at all.

Two numbers come out of that, and they are the ones that say whether the AI is
earning its place:

- **Suggestions taken** — separated from verses you typed in yourself. Before this,
  the history could tell you how many verses a person put up and could not tell you
  how many of them were Relay's idea.
- **Suggestions rejected**, and what share of the ones you answered you took.

Two things it deliberately does not do. It does not count suggestions that scrolled
past while you were busy — those are genuinely not recorded, and the report says so
rather than folding them into the percentage. And **nothing is counted during a
rehearsal**: practising means accepting verses you chose yourself, and a score
inflated by practice is worse than no score.

### The setup walk-through now tells you what to do before your first Sunday

Relay has practice drills, a check that says whether the whole chain from the
microphone to the screen actually works, and a rehearsal mode that runs a full
service without touching the projector. All three have been there for a while, and
nothing told a new operator any of them existed.

The last step of the setup walk-through — the one where a verse goes up on your
real screen — now names all three and says which tab each one is on. It does not
add any more questions to the walk-through: they are things to do on another day,
not answers to give now, and everything in there is still in Settings afterwards.

The one worth doing first is the chain check (**Settings → Dashboard**): say one
verse out loud, and Relay tells you which of the six stages between your
microphone and your screen were reached. Everything the walk-through sets up can
pass on a machine where the chain still does not work end to end — a microphone
the operating system has muted, an output window on a display that is asleep. That
is the difference between finding out at 10:05 and finding out at 10:31.

### Relay now tells you, in the app, what it is bad at

- **Help has a new topic: "What the AI is bad at."** It says four things plainly, and they
  are the four a church should hear before it trusts this: Relay **never writes scripture**
  (the words are read verbatim from the bundled King James Version, so a wrong verse means
  a wrong *reference*, never invented text); **African-language listening is the weakest
  part of the product**, and it is also the headline claim; **nobody has measured how often
  it mishears**, in any language, English included; and therefore keep `Esc` under your
  hand and watch the wall rather than the app.
- **Why this is a change worth reading.** All of that was already written down — honestly,
  at length — in a file on the internet that no operator was ever going to open. What the
  app itself carried was the reassuring half: the rule that a guess never reaches a screen
  on its own. Publishing only the reassuring half is worse than publishing neither, and it
  is the same failure as a status light that cannot detect its own fault. Both halves are
  now in the app, offline, in the search box on the Help tab.

### The first real service, and the eight things it changed

Relay listened to a live sermon for the first time — fifty minutes, a real preacher, a
real room. Five of the six references it put up on its own were right. Everything below
came out of that morning or out of checking the rest of the code against it.

- **A verse nobody said no longer reaches the screen.** The preacher said "look at Luke
  10, read from verse 32" and Relay put up Proverbs 3:32 — because a passage the operator
  had put up by hand half an hour earlier was still what a bare verse number attached to.
  Relay now uses the book named in the sentence it is listening to, and only falls back on
  memory when no book was said.
- **Ordinary preaching no longer puts a verse up by itself.** "Nehemiah, fifty two days
  they built the wall" used to put Nehemiah 5:2 on the screen. So did "Romans eight one
  two". And at the top of the sensitivity slider, every one of these protections stopped
  working entirely.
- **The screen stays awake** while the microphone is on, a service is recording, or an
  output window is open — including when Relay reopens your screens by itself at startup,
  which is the case that matters most and the one that was missed first time.
- **You can switch a screen on or off from the Live tab.** The status panel could tell you
  a screen had stopped responding and gave you nowhere to press.
- **Accepting a suggestion now actually teaches Relay.** It is supposed to learn your
  preacher, and on that side it never had.
- **Song running orders.** You can build one — verse, chorus, verse, chorus, bridge — and
  Relay tells you when the song has changed underneath it rather than quietly playing the
  wrong parts.
- **A damaged settings file no longer makes Relay forget your machine was ever set up**,
  and the first-run wizard stops appearing over a service.
- **Relay says when its engine is not running**, on the Live tab, instead of just going
  grey. And a screen reader can now navigate the app: six screens had no headings at all.

### Relay now tells you when a screen has stopped listening to it

- **Your screens report back.** Every output — the projector on HDMI and every browser
  source in OBS — now tells Relay twice a second that it is still showing something.
  If one stops, the Live tab says so, in red, within a few seconds. Before this,
  a screen that had frozen, crashed or gone to sleep still read **On Air** in amber,
  because Relay was only ever reporting what it had *sent*.
- **During a service, Relay holds a few things back.** Deleting anything, changing or
  downloading a speech model, and bulk imports are unavailable while you are recording
  — an accident at 10:31 has no undo. **Nothing you use to run the service is affected:**
  firing, next/back, clear, blackout and rehearsal all work exactly as before. You can
  lift it in one click (Settings → Backup & Recovery) and it comes back for the next
  service.
- **Relay will not restart to update during a service.** It already refused while the
  microphone was on; it now also refuses while a service is recording, which covers the
  gaps between readings.
- **Every service keeps a record of what happened.** Service History now shows an
  ordered list: when it started, what fired and whether it was Relay or you, when the
  screens were cleared, when a screen stopped responding and when it came back — and,
  the one nobody had before, *a panic control that did not reach the screens*. Speed is
  kept too, so you can see whether it slowed down over the service. **Nothing anyone
  said is in it** — no transcript, no verse text, no lyrics.
- **A cue that would show an empty screen no longer goes out.** It leaves what is on the
  screens where it is and tells you why, instead of quietly blanking the projector.
- **Relay warns when a verse is being squeezed to fit.** Long passages used to shrink
  until they fitted, however small that got. The verse still goes up, but you are told
  when it has gone below a readable size, so you can pick a shorter passage or a
  roomier template.

### Relay can now tell you what happened, and get your history back if an update goes wrong

- **Updating copies your history first.** Every service, plan, song, saved verse and
  template is copied before an update installs, and Relay keeps the last three copies.
  If the new version comes up with a database that is not right, it says so on the next
  launch and offers to put your history back — you decide, not Relay. The app itself can
  always be reinstalled from a release page; your history cannot, which is why it is the
  thing that gets copied. Settings → Updates shows whether it is safe to update *before*
  you press anything.
- **Service History now replays.** Click any moment in a service and see what was being
  said around it, what Relay decided, who decided it, and how fast it was going.
- **A report for each service.** How long, what Relay fired, what you fired, what it
  suggested and what you took, panic controls that failed, screens that stopped, and
  whether it slowed down over the service. **It also says what it does not tell you** —
  including that nothing here checks whether the verse shown was the right one.
- **Relay says when it is working at less than full strength.** One line at the bottom
  of the window, on every tab. No speech model, noise reduction switched off because the
  microphone will not run at 48 kHz, detection disarmed, a build without graphics
  acceleration, a screen that stopped answering. Each one says what it means for the
  service and what to do about it. All of these already happened; none of them used to
  be visible, so "it isn't hearing anything" got blamed on the AI.
- **Rooms.** Save the main hall — microphone, recognition language, planned length,
  voice profile, and which display each screen uses — and put it back with one press
  next Wednesday. **Audio levels are deliberately not saved**: Relay learns those fresh
  every time, because a level measured three weeks ago in a room that now has the
  heating on and forty more people in it is a guess, and guessing is what once made
  Relay deaf to a quiet preacher.
- **Settings → Languages.** What Relay actually knows about Yorùbá, Kiswahili and
  Hausa, counted from the data it ships with. Two columns are empty on purpose:
  nobody who speaks these languages has checked the book names yet, and accuracy has
  never been measured in any language, including English. Both say so.
- **Save a diagnostic file.** Settings → Diagnostics writes one file you can email when
  something goes wrong. It contains no transcript, no verse text, no lyrics and no
  service names — you can read it before you send it.

### Practice before Sunday, and a check that tests the whole path

- **Test the whole path.** Settings → Dashboard has a new check: press start, say
  "John chapter three, verse sixteen", and watch six stages light up — microphone,
  voice heard, words, reference recognised, allowed through, on a screen. The startup
  checks tell you each part is there; this tells you they work *together*, which is
  the thing you actually want to know at 10:05. **Relay switches itself to rehearsal
  first**, so the test cannot reach your screens — and if it cannot switch, it refuses
  to run rather than firing a verse at your congregation to test itself.
- **Practice before your first Sunday.** Help → six short drills using the real
  controls: clearing the screens and blacking them out come first, because those are
  the two that save a service. Relay stays in rehearsal the whole time. It is not a
  pretend service — there is no preacher in there — it just makes sure your hands know
  where the controls are before the moment you need them.
- **Settings → Privacy.** One page answering "what is on this machine, and what can
  leave it", read from your actual settings rather than from a promise. It includes
  the uncomfortable part: anyone on your church WiFi can see what is on the projector
  **and can change it**, because the preacher's remote has no password by design.
- **Service History now shows the one-in-a-hundred figure**, and whether Relay is
  getting slower week by week rather than only during one service. It stays quiet
  until it has seen three services, because two is not a trend.
- **Fixed:** the live speed readout showed `0ms` for a stage that never ran, which
  made the part that did not happen look like the fastest thing on the screen.
- **Fixed:** several buttons and text boxes had no name for screen readers.

### Installing without internet, and knowing the back row can read it

- **Relay can now be installed with no internet at all.** Everything except the
  speech model already worked offline — the whole Bible is inside the app. The model
  is 148 MB and could only ever be downloaded, which meant a church on a poor line
  could not get Relay working. Copy the model file onto the computer (Downloads is
  fine) and Settings → Network offers to install it under **"Found on this
  computer"**. Relay checks the file is exactly the one it expects first, so a copy
  that went wrong tells you rather than mishearing everything afterwards.
- **Templates now tell you whether the back row can read them.** The editor shows the
  contrast between your text and its background, works out how tall the letters
  actually are on your screen, and says whether that is big enough for the distance
  you give it — plus a preview of how it looks from 5, 10, 15 and 20 metres. Over a
  photograph or a video it says it **cannot** check, rather than guessing: only your
  eyes can judge that one.
- **A High Visibility theme.** White on black, larger type, no shadow and no
  transition — the highest contrast a projector can produce. Pick it like any other
  theme and every screen uses it.

### 🚧 Nothing has been released to anyone yet.

Relay has never shipped. Every tag so far (`v0.1.0-rc1` … `v0.1.0-4`) is a **draft pre-release** used to exercise the pipeline — they are unsigned, and an unsigned build is stopped dead by macOS Gatekeeper and warned about by Windows SmartScreen. A volunteer does not push past those screens, and should not be asked to.

**The first real release is blocked on one purchase: a Windows code-signing certificate** (~$10/month, Azure Trusted Signing). The release workflow now *refuses* to publish an unsigned Windows installer rather than doing it quietly — see `docs/RELEASING.md`.

Everything below is what that first release will contain.

---

## [0.1.0-4] — 2026-08-24 · pre-release

**The live transcript now arrives about two and a half times sooner, and for the
first time you can see the delay for yourself.**

- **Words appear on screen sooner.** Relay was waiting between decodes for two
  reasons that turned out to protect nothing: it asked for work in a size the
  microphone cannot deliver, and it left itself spare time it never needed. On the
  default speech model the transcript went from arriving about a third of a second
  after the words to about a seventh of a second, and it updates roughly twice as
  often. **Nothing about safety changed** — no threshold moved, and Relay still
  waits for a second look before it puts a heard reference on a screen.
- **Deciding what was said no longer holds up hearing the next thing.** Looking up a
  verse used to happen on the same thread that runs the speech model. It now runs
  beside it, so a busy moment delays a lookup rather than the transcript.
- **Settings → Diagnostics → Live latency.** New. It times nine points between the
  microphone and the projector, on the machine in the room, and tells you in one
  sentence whether the pipeline is keeping up, whether the speech model you chose is
  the bottleneck, and whether the delay is growing over the length of a service. It
  is on by default, because a measurement that needs a developer build is one no
  church will ever take.

**Read before you change models.** Diagnostics will now tell you plainly that on a
bigger model the wait is the model itself and not something Relay can fix — a trade
you were always making and could not previously see. Evidence, and everything these
numbers do *not* prove: `docs/audits/PERF-2026-08-24.md`.

---

## [0.1.0-3] — 2026-08-23 · pre-release

**Two wrong-verse bugs, both found by running a real service and then reading
Relay's own record of what it did.**

- **"Chapter nine and verse twenty-four" now shows verse 24.** It was showing
  **verse 1**. The word "and" between the chapter and the verse made Relay throw the
  verse away and fall back to the start of the chapter — confidently, with no sign
  anything was wrong. Say the same sentence without "and" and it had always worked.
  In one service this put 1 Corinthians 9:1, 2 Chronicles 15:1 and 26:1, Proverbs
  3:1, Isaiah 61:1, Hebrews 6:1, Genesis 12:1 and Psalms 23:1 on the screens.
- **The wall no longer flickers between two verses.** If Relay heard more than one
  reference in the same moment of speech it put them all up, one erasing the next
  before anyone could read it. It now shows the strongest and *offers* the rest, so
  you can still fire them with one press.

Both were invisible to every check already in the product: they are not mis-hearings
— the transcript was right and the confidence was high. Relay was reading the
sentence wrongly, the same way, every time.

---

## [0.1.0-2] — 2026-08-23 · pre-release

**Relay now keeps up with the preacher.** On this Mac the speech model was running on
the processor instead of the graphics chip, which made every decode about three times
slower than it needed to be — slower than real time, so the transcript, the detected
verse and the firing all fell further behind the longer someone spoke. macOS builds now
use the graphics chip automatically. Nothing to switch on.

- **The transcript keeps pace with speech.** It also updates more often on a fast
  machine instead of once a second regardless — Relay now measures how quickly your
  computer can transcribe and paces itself to match.
- **Fewer wrong verses from half-heard numbers.** Listening more often means Relay
  sometimes glimpses a reference before it has heard all of it — "verse twenty eight"
  can look like "verse sixteen" for a moment. A verse now has to be heard twice before
  Relay puts it on a screen by itself. It still *offers* it immediately, so you can fire
  it by hand the instant you see it.
- **A web page can no longer black out your wall.** Anyone on the church network could
  previously blank the congregation's screen just by loading a page containing a hidden
  image link. Closed. Driving Relay from the preacher's phone works exactly as before.
- **The Hardware Check screen tells the truth about the graphics chip.** It was
  reporting "CPU" on builds that were using the GPU.

**Still a draft pre-release, still unsigned**, so macOS will warn you it cannot verify
the app. Nothing here changes that — it needs the certificates in `docs/RELEASING.md`.

---

## [0.1.0] — pending

The first thing a church could actually use.

### It listens, and puts the verse on the screen

- **Live scripture detection.** Relay hears the preacher, recognises the reference — spoken as `"John three sixteen"`, `"first John four eight"`, or in Yorùbá, Swahili or Hausa — and puts it on the screens.
- **It works with the internet unplugged.** All of it. Transcription happens on your computer; nothing is sent anywhere.
- **It tells you *how* it heard something.** A reference it actually **heard** looks different from a **paraphrase it guessed**, and a paraphrase never reaches the wall on its own. Relay shows you the words that made it think so.
- **Related scripture.** The preacher is talking about fear; four verses on fear are one click away. Nobody said them, and Relay says so.
- **Manual override, always.** Type `John 3:16`, `Ps 23`, `rom 8 1` — it fires instantly, whatever the AI is doing.
- **Voice navigation.** Say "next" or "back" and Relay walks the passage.

### Screens

- **Any number of outputs, each styled differently** — the main projector, a stage monitor, an OBS stream, the preacher's phone — all driven from one template engine. What you see in the editor is exactly what the congregation sees.
- **Songs, media, announcements and countdowns**, alongside scripture, in one service plan.
- **ProPresenter import.** Bring your existing songs.

### Running a service

- **One tab.** Build the plan in the **Planner** (a Tuesday job — it cannot reach a screen). Run it on **Live** (a Sunday job), where the AI's suggestions and the plan sit side by side, because the preacher going off-script is the entire point.
- **Rehearsal mode.** Practise the whole service with nothing reaching the congregation.
- **Panic keys that work everywhere.** `Esc` clears every screen. `B` blacks them out. From any tab, even one that has crashed.
- **Crash recovery.** If the console crashes, the output screens are separate windows — **the congregation still sees the verse** — and Relay puts you back where you were.

### Setup

- **One-button speech-model download** (~148 MB, once). No terminal, ever.
- **A first-run wizard** that ends by putting a real verse on your real projector, so you have *seen it work* before Sunday.
- **Auto-update**, which never runs during a service.

### Your privacy

- **Sermon audio never leaves the device.** It is transcribed on your computer and thrown away. See [PRIVACY.md](PRIVACY.md).
- **Crash reporting is off by default**, has no destination in the open-source build, and *drops* free text rather than trying to filter it — so a transcript, a verse or a lyric cannot leak through it even by accident.

### Fixed before anyone was hurt by them

These were all found and fixed before a single church ran Relay. They are listed because they are the reason to trust the next release, not despite it.

- **Relay was silently deaf to a quiet preacher.** Measured: 94% of speech detected at studio level, **2%** at a real church-laptop level — with no error, no warning, and a transcript that just quietly turned to nonsense. The audio front-end now *learns* the room instead of assuming a level.
- **The panic keys could lie.** "Screens cleared" was shown whether or not the screens cleared. `Esc` pressed inside a help overlay or an arrangement picker **wiped the congregation's screens** as a side-effect.
- **The transport key could silently do nothing** — you press `→` mid-sermon, the wall does not change, and nothing anywhere says why.
- **The AI's confidence number meant two different things** depending on how it had matched, and both were rendered identically.
- **The first-run microphone test proved nothing** — the meter whose entire purpose was to prove the mic works never moved.
- **A garbled reference could blank the projector** mid-service.
- **On Windows, speech recognition was silently dead** — the model was looked for at a macOS-only path.
- **The updater could never have delivered an update.** The version was hard-coded in three files, so every install would have decided it was already up to date. Forever.
- **A signed macOS build would have had a dead microphone**, and no build we could make locally would have shown it.
- **A database migration could brick every future boot**, before the window was even shown.

---

## Notes for maintainers

**Every release must have an entry here before it is tagged.** `0.1.0-4` was tagged without one and the entry was written back five days later, from the commit — which is how it should never happen: the person who made the change is the only one who knows what it meant to an operator. An updater that offers a church a restart, twenty minutes before a service, with no explanation of what changes, is asking them to gamble. If there is nothing worth telling them, there is nothing worth interrupting them for.

Write the entry for the operator. "Fixed a race in the router" is not a changelog entry; "the wrong verse could appear if two references were spoken in the same sentence" is.

Bump the version with `npm run version:set -- <version>` — it writes all three files, and CI will refuse a tag that disagrees with them.
