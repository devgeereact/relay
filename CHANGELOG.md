# Changelog

What changed, and why it matters to the person in the booth.

**This file is read by operators, not only by developers.** When Relay updates itself, the release notes are what a volunteer sees before deciding whether to restart the app twenty minutes before a service. So entries say what *changed for them*, not which function was refactored. If a change cannot be explained in those terms, it does not belong under a heading a user reads.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [Semantic Versioning](https://semver.org/), with one project-specific rule: **pre-release identifiers must be numeric** (`0.1.0-1`, not `0.1.0-rc1`) — the Windows MSI bundler rejects named ones, fifteen minutes into a release, on the platform most of our churches are on.

---

## [Unreleased]

### 🚧 Nothing has been released to anyone yet.

Relay has never shipped. Every tag so far (`v0.1.0-rc1` … `v0.1.0-2`) is a **draft pre-release** used to exercise the pipeline — they are unsigned, and an unsigned build is stopped dead by macOS Gatekeeper and warned about by Windows SmartScreen. A volunteer does not push past those screens, and should not be asked to.

**The first real release is blocked on one purchase: a Windows code-signing certificate** (~$10/month, Azure Trusted Signing). The release workflow now *refuses* to publish an unsigned Windows installer rather than doing it quietly — see `docs/RELEASING.md`.

Everything below is what that first release will contain.

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

**Every release must have an entry here before it is tagged.** An updater that offers a church a restart, twenty minutes before a service, with no explanation of what changes, is asking them to gamble. If there is nothing worth telling them, there is nothing worth interrupting them for.

Write the entry for the operator. "Fixed a race in the router" is not a changelog entry; "the wrong verse could appear if two references were spoken in the same sentence" is.

Bump the version with `npm run version:set -- <version>` — it writes all three files, and CI will refuse a tag that disagrees with them.
