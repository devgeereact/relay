# Changelog

What changed, and why it matters to the person in the booth.

**This file is read by operators, not only by developers.** When Relay updates itself, the release notes are what a volunteer sees before deciding whether to restart the app twenty minutes before a service. So entries say what *changed for them*, not which function was refactored. If a change cannot be explained in those terms, it does not belong under a heading a user reads.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versions follow [Semantic Versioning](https://semver.org/), with one project-specific rule: **pre-release identifiers must be numeric** (`0.1.0-1`, not `0.1.0-rc1`) — the Windows MSI bundler rejects named ones, fifteen minutes into a release, on the platform most of our churches are on.

---

## [Unreleased]

### Connecting the preacher's phone

When Relay cannot find a local network address, Sharing now explains the problem
instead of offering a stage link that points back at the phone itself. The stage
QR code is larger, has a wider white border, and reports generation failures with
a copy-link recovery action. Sharing also lists this computer's network adapters
so the right one can be chosen when there are several, and lets a church with more
than one stage screen pick which screen the link is for.

The same guard now covers the QR code in Outputs -> Screens, which previously could
produce a code for `http://localhost:8032/...` — an address that names whichever
device scans it. The output URL itself is unchanged and still copyable, because
that address is correct for OBS running on this same computer; only the QR code,
which exists to be photographed by a second device, is withheld and explained.

The preacher's screen now reports that it is working. Every other output screen
has told Relay every two seconds that it is still painting; the stage page never
did, so a phone or tablet that was set up correctly and showing the reading
perfectly was described on the operator's console as a screen that had never
painted, alongside advice to fix a problem it did not have.

Controls on the preacher's phone now give up after six seconds instead of
waiting for ever. Previously a request that never came back left every button on
that panel disabled for the rest of the service, with a page reload as the only
way out. And when the phone cannot reach Relay, it now says so, rather than
saying "No next verse" — which is what Relay says when a reading has genuinely
ended, and which could appear over a wall that had in fact just advanced.

The preacher's screen now shows Relay's time, not the phone's. A countdown is
sent as the moment it ends, and the phone worked out the minutes and seconds
itself — so a tablet whose clock was a minute out showed a minute of error on
the figure a sermon is paced against. The phone now takes the time from Relay.
It also notices when Relay has stopped answering and says "not answering"
instead of showing a live indicator over frozen content, which a sleeping or
roaming phone could previously do for the rest of a service.

A phone that has been asleep now reconnects the moment you pick it up, rather
than on the next retry, and a phone left alone during a long network outage
stops retrying every second and a half for the whole service. When Relay has
stopped answering, the stage screen says for how long, because "not answering"
reads the same after four seconds as after ten minutes.

A sermon timer that has run over can now be held. Until now the only things an
operator could do to a clock past zero were stop it, which throws away how far
over it is, or add five minutes, which re-aims it — neither of which is "note
where we got to". The preacher's screen has always been able to show a held
timer; nothing could put one into that state. Holding freezes the figure being
read, including past zero, and letting go carries on from there. It touches no
congregation screen, and a countdown in front of a room still cannot show a
negative.

Sermon timers gained Reset, which puts a clock back to the length it was started
at. Doing that by hand meant stopping the timer and starting a new one, which
also threw away its name, its warning setting and its link to the service plan.
Reset keeps all of those, and leaves a held timer held.

Countdowns can now count down to a time of day. Until now every timer in Relay
was a length — "twenty minutes" — so "the service starts at 10:30" meant working
out the minutes in your head, and the screen went on counting confidently once
the service slipped. Both the congregation countdown and the preacher's Stage
Timer accept a clock time now; leave the field empty and the length beside it
works exactly as before.

If the time has already gone, the clock starts counting up from it rather than
jumping to tomorrow, so a mistyped time is obvious straight away instead of
showing 23 hours and something.

The desk can now decide what a preacher's screen shows. Until now those choices
lived on the device itself, so the operator could not set them, could not see
them, and a tablet reset lost the arrangement. Outputs now offers a stage layout
per stage screen, with three to start from: Preacher, Confidence monitor and
Timer focus.

Screens you have not given a layout to are left exactly as they are, set from
the device as before — nothing is migrated and nothing is overwritten. Clearing
a screen's layout hands it back to the device rather than resetting it.

Stage layouts can be created and edited, not only chosen. Outputs has a Stage
layouts section: name a layout, pick what it shows, and assign it to a stage
screen. Changing the switches does not affect any screen until you press Save,
and the editor says when there is something unsaved. A layout a screen is
wearing cannot be deleted until that screen has been given a different one, and
the layouts Relay ships with cannot be deleted at all — they would come back the
next time Relay started, so they can be renamed and changed instead.

Physical phone and tablet scanning remains to be verified.

### Lower thirds had nothing behind the words

A lower third is the caption bar Relay puts over a live camera on the stream. All of
them were painting the words with no bar behind them, so on a dark shot the caption
was barely there and on three of the supplied designs — dark type on a light bar —
it was effectively invisible. Nothing at the desk showed this, because the console
preview and the projector are a different path from the stream.

Fixed, and the bar is now a neutral near-black rather than the purple it used to be.
Purple means *rehearsal* everywhere else in Relay, and two copies of the design
disagreed about which purple, so the same template could look different depending on
whether a screen was a browser source or the projector.

### The reference was bigger than the verse

On five of the supplied looks the citation — "Romans 8:28" — was rendering **larger**
than the scripture it labels, because a short line was allowed to grow to fill its
box and the citation is always the shortest line. On the High Visibility look, the
one meant for a brightly lit room, the reference came out 17% larger than the verse.
A label now shrinks to fit and never grows past the thing it labels.

### A long reading squeezed the words instead of growing the bar

The caption bar was meant to grow a little before the type gets smaller. It never
did — the rule could not fire on either of the supplied bars — so a long verse
shrank to about two thirds of its intended size instead. It grows now.

### Help sent you to a button that does not exist

The help topic for *something wrong is on the screen* told you to click **Emergency
Stop**, "top-right of every screen, always". That control was removed when the top
bar was cleared down to the six workspaces. It now names the red **Clear screens**
button along the bottom of the Controls card, which is on every workspace.

### The shortcuts page offered a key that blacks out the church

It listed `b` as an example of a song-section key. `b` is **Blackout**. Pressing it
on a song would have taken every screen to black. The examples are now keys that a
song can actually be given, and the page says where section keys work.

### A microphone that stops is now announced everywhere

If the microphone failed — a cable pulled, an interface unplugged — the only sign was
a line of technical text at the bottom of the Live tab. On any other workspace there
was nothing at all, and the transcript simply stopped. It now appears in the status
bar on every workspace, in plain words, with what to check.

### The listening control says "Listen"

Every instruction in Relay told you to press *Start listening*. No button said that:
it was an unlabelled microphone icon captioned `off`. It now reads **Listen**, and
**Listening** once it is.

### Colours mean what they say again

**TAKE** was amber. Amber means *on air* in Relay, so the brightest thing on the
screen was lit even with the screens clear and even during a rehearsal, which made
the one colour that should catch your eye useless. TAKE is now blue, and amber is
back to meaning a congregation is looking at something.

**Dismiss** was solid red while **Clear screens** — the control that takes the wall
down — was a faint tint. That is the wrong way round, and red now belongs to the
panic controls. The **Preview** badge and the **Rehearsal** tag were also hard to
read; both were corrected and measured.

### The desks line up

Panel headings across a workspace were three different heights, so the lines under
them did not match. The left and right columns were a different width on every
workspace, so both edges of the screen moved whenever you changed tab. They are one
measurement now. An error message in a narrow column used to wrap to one word a line;
it wraps properly.

### Escape now works inside every dialog

Four places — the crash panel, the setup walk-through, the AI detection detail and
the template menus — took the Escape key away without doing anything with it. In two
of them the **Clear screens** button was also covered. So at those moments neither
way of clearing the screens worked. One press now closes what is in front of you; a
second clears the screens, as it always has everywhere else.

### Other fixes

- The **AI detection** card now says which kind of claim it is making. A misheard
  *book* used to be labelled the same as a paraphrase, and the note underneath said
  "not a spoken reference", which was untrue of it.
- Opening **"why this match?"** on one suggestion and pressing *Accept & fire* sent a
  different verse — the top one in the list. It sends the one you were looking at.
- A song or notice in **Up Next** whose title contained a reference (a hymn called
  "Psalm 23") put the Bible chapter on the screen instead of the song.
- The panic warning could end in `[object Object]`.
- The sermon text is no longer printed to the terminal.
- A screen set to follow the content look no longer opens wearing the wrong one.
- Converting old templates now keeps a restorable version first, says how many it
  converted, and will not run while a service is being recorded.
- Deleting a **cue** during a service is now held back like deleting a plan.
- Pressing **Space** on a button you have tabbed to now presses that button, instead
  of advancing the programme.

### The first verse of a service could go up cut in half

On a screen that had just been opened — an OBS browser source, a kiosk page, the
projector page loaded before the service — the **first** thing put on it was sized
wrongly and painted with its top and bottom lines sliced through the middle. Nothing
fixed it afterwards: it stayed like that until something happened to resize the
window. Measured on the default Classic Serif template at 1920×1080, Romans 8:28
painted a block half as tall again as the box it was in.

The cause is that a font is only fetched the moment something first uses it, so the
very first verse was measured in a stand-in typeface, wrapped into fewer lines than
the real one, and was therefore allowed to be too big. Relay now checks that it
measured the typeface it is about to paint with, and measures again if it did not.

### A screen that reconnected during a service came back blank

If an output went away and came back — OBS restarting a source, a kiosk page
reloading, the lobby television dropping off the wifi for a moment — it reconnected
and then showed **nothing at all** until the operator happened to put the next thing
up. On the congregation's screen, for as long as the reading lasted.

A screen that joins now receives what is on the screens at that moment. If the
operator has cleared or blacked out, that is what it receives: coming back late can
never undo Clear or Blackout.

### The Live screen said which screen had died, in a word you can act on

Four things on the run surface were being cut off mid-word, and all four mattered:

- The **Output Status** panel could not fit a screen's name beside its badge, and the
  screen with the worst news was the worst affected — "Streaming" was rendered seven
  pixels wide next to "NOT RESPONDING". Names now read in full, on their own line.
- The warning strip along the bottom said **"3 is not responding"**. It says
  "Streaming is not responding".
- On a 1366-wide laptop the panel headings collapsed ("AI Detection — Current Claim"
  down to five pixels) and the **Clear screens** button read "Clear scree…". Both fit
  now, wrapping rather than truncating.
- A screen whose own report was blank showed `native_window`, a word out of the
  database. It says "Native window".

### Accept and Dismiss were below the bottom of the panel

With one ordinary suggestion showing, the two buttons the whole product exists to
offer sat just off the bottom of the AI Detection panel, out of sight unless you
scrolled. The card repeated the reference and the match type twice over; that
repetition is gone, and the two buttons are now pinned to the bottom of the panel
where they cannot scroll away.

### Settings stopped offering seven switches that did nothing

Seven controls saved a preference nothing in Relay ever read. The worst was
**"Confirm Before Going Live"**, which was on by default and promised a confirmation
step between the operator and the congregation's screen — there has never been one.
Also removed: Auto Save, Default Content Type, Time Format, Date Format, Restore
Previous Session and Default Startup Screen. Nothing you could do with them is lost,
because nothing they did ever happened.

**Settings → Overview** also said *"You're on the latest version"* whether or not
anything had been asked — including when Relay could not reach the update server. It
now reports what the last check actually found, which the Updates page already did.

### Smaller things you may notice

- The Template editor's **Save Template** button was off the right-hand edge of the
  window at 1440 wide, and the layer list showed one and a half layers out of three.
- The Template editor's "Select a layer" hint was drawn as grey text on a solid
  purple bar, sitting on top of the panel heading.
- **Service History** squeezed the service title to about four characters, so every
  service in the list read "Sunda…".
- The **Dashboard's** system-health rows were cut off mid-word on every line.
- Sliders across Settings, the Template editor and the Theme editor were drawing the
  browser's own white bar on a dark panel; the detection dial's was four pixels tall,
  which is hard to hit and easy to miss.
- The Theme editor listed a typeface as `var(--f-display)`. It says "Display".
- On a phone-width window the **Emergency Stop** button was off the edge of the
  screen, and the warning strip covered the tab bar.
- The preacher's stage page said "connecting…" for ever when it could not reach
  Relay. After a few attempts it says so.
- The sidebar's "Engine offline" line could not recover: it was decided once at
  startup and never asked again.
- A plan cue of a kind Relay does not recognise was drawn as **Scripture**, which is
  the one kind that fires by itself. It is drawn as Unknown.

### Eighteen verses were showing a translator's note as though it were scripture

Some verses in the bundled KJV ended with a note the translators wrote, rendered on
the screen in the same type as the verse — so a congregation read it as part of what
was being preached.

Fourteen were the notes at the end of a letter. Hebrews 13:25 read *"Grace be with you
all. Amen. **«Written to the Hebrews from Italy, by Timothy.»**"*, and Romans, both
Corinthians, Galatians, Ephesians, Philippians, Colossians, both Thessalonians, both
Timothys, Titus and Philemon each did the same.

Four were marginal notes. **Micah 7:12** ended with a whole one — *"…from mountain to
mountain. **{and from the fortified cities: or, even to the fortified cities}**"* — and
Hebrews 10:34, Romans 16:27 and 1 Corinthians 16:24 each ended with a fragment of one.

**All eighteen are fixed, and an existing install repairs itself on the next launch.**
Nothing else about your service history, your plans or your templates is touched; the
repair replaces only the verse text. If you have already recorded services, their
detections keep pointing at the same references they always did.

### Yorùbá chapter and verse numbers are now understood — as suggestions

Relay already recognised Yorùbá **book** names. It now recognises Yorùbá **numbers**
too, so *"Jòhánù orí kẹta ẹsẹ̀ kẹrìndínlógún"* finds John 3:16 without anyone switching
to English mid-sentence.

**It will offer the verse; it will not put it on the screen by itself.** No Yorùbá
speaker has checked those number words yet, and a wrong number does not fail safely —
it shows a different verse. So Relay asks rather than acts, and you accept it with one
press, exactly like a paraphrase. **Settings → Languages** says *"suggest only"* in the
numerals column so this is visible rather than a surprise.

Swahili and Hausa are unchanged and still fire on their own.

### A new look for the booth

**A new look, and eight things that were quietly not working.** The whole console has been
retuned for a dark booth — denser type, flatter corners, one slider and one switch everywhere
instead of whatever each screen drew for itself. None of the four colours that carry a promise
changed meaning: amber is still ON AIR, amethyst still rehearsal, cyan still a guess, grey still
CUED. What did change is that the colour for "the thing you are clicking" is now its own steel
blue, so the rehearsal colour is no longer lit on every hover and Save button.

What you may notice most, in the order you would meet it:

- **A screen can now follow the content look.** Set "Scripture wears Nocturne" once, and every
  screen set to follow wears it. Until now every screen always had a look of its own, which meant
  the content-look settings could be filled in and change nothing anywhere.
- **A word to the preacher.** Type one line in Live and it takes over the stage monitor — and only
  the stage monitor. No congregation screen can show it.
- **Blackout is black.** It used to wear the same grey that means CUED.
- **Deleting an object in the Templates editor now takes two presses.** It was one click on a
  small button between Lock and Visibility.
- **Transitions work.** The theme editor has offered a transition and a duration for a long time
  and the wall ignored both. There are seven now, they apply, and the default is still an instant
  cut. If you have reduced motion turned on, everything cuts.
- **Search understands `ps23:1`**, and stops answering when it has not understood: a search whose
  words are mostly not in any verse now returns nothing rather than a confident list.
- **Three lower thirds** instead of one — a name band, a lyric band with no reference at all, and
  a scripture band.
- **Settings rows say which kind of nothing they have.** "—" beside *This machine* could mean
  "no network", "could not be read" or "still checking"; now it says which.
- **A composite screen.** A camera on one side and a real rendered slide on the other, keyed —
  your switcher supplies the camera. The slide half scales to its own box, so the same look works
  at full screen and in half of one.
- **A song cue is named in the service record again.** Firing a song from a plan recorded a blank,
  so a Sunday report said "Manual override" about nothing. The words on the screen are unchanged —
  a song's section label still never reaches the glass.

Nothing in that list changes how verses are detected, which model you run, or the release decision.


_Nothing else that changes what you see in the booth._

---

## [0.2.0-2] — 2026-09-05 · pre-release

**A pre-release, not a release**, and the reason has not changed: neither the macOS
nor the Windows build carries a code-signing certificate, so macOS will say *"Relay
is damaged and can't be opened"* and Windows SmartScreen will warn. That is expected
and it is why this is not offered as a general download. It is for the supervised
pilot described below, on macOS, with somebody watching.

### Relay can receive a fix now

Every copy of Relay ever installed has been **unable to update itself**. The updater
was built, wired and signed — and the address it looked at returned "not found",
every time, since the day it was written. Nothing said so: the screen simply never
offered an update, which looks exactly like being up to date.

There is now one permanent address that always describes the newest version, and
every future build asks it. **This is the first build that can be told about a
newer one** — a copy you already have cannot, because the address it was built with
does not exist. Updating to this version by hand is the last time you will have to.

**Settings → Updates** tells you the truth about the channel either way, including
when it cannot reach it at all.

### A list that fails to load no longer tells you it is empty

If Relay could not read your songs, your media, your saved verses, your
announcements, your service plans or a chapter of the Bible, it said *"No songs yet
— import or paste one"*, *"No plans yet"*, *"That chapter is empty"*. Every one of
those sentences invites you to do work you have already done, and one of them is a
claim about scripture rather than about a database.

Eleven screens now say **why** the read failed, and offer to try again — including
the plan rail on the Live tab and the cue list in the Planner, which are the two you
would be looking at ten minutes before a service. The model chooser used to show a
blank panel in the same situation; it now says what went wrong.

Errors in the template and theme editors used to appear as `[object Object]`. They
are sentences now. The one you are most likely to have seen is Relay refusing to
delete something while a service is recording, which was always meant to explain
itself.

### Announcements and confirmations are spoken aloud

*"John 3:16 is on the screens"* — the line that confirms content reached a
congregation — was invisible to a screen reader on six screens. It is announced now.
A save that failed on the Announcements screen used to appear in the same green as a
save that worked, and a delete that was refused said nothing at all.

### Video on a kiosk screen or OBS can be scrubbed

A clip served to a browser source could not be seeked or resumed — dragging the
position bar started it again from the beginning. It behaves properly now.

### ⚠️ If your kiosk screen is not served by Relay, it needs one setting

Relay's output pages are normally loaded **from Relay** — an OBS browser source or a
kiosk screen pointed at `http://<relay-ip>:8032/output.html`. Those are unaffected.

If your church serves its own page from somewhere else — a Raspberry Pi, an existing
signage box — it will now be refused, because Relay no longer accepts a connection
from a page it did not serve. That is deliberate: anyone on the church wifi could
previously open a page on their phone and receive the service feed, **including the
preacher's stage notes and the verse coming next**.

To allow your own page again, start Relay with `RELAY_KIOSK_ANY_ORIGIN=1`. Relay
prints the reason and that setting whenever it refuses a connection, so if a screen
goes blank the answer is in the log rather than in a guess. **Find out on the
Saturday, not on the Sunday.**

### Under the hood

The Bible repair described below now actually reaches a copy of Relay you already
have — the version that shipped it could not, on any machine, which was found and
fixed before this build. It also keeps the verse references in your service history
intact while it repairs itself; the first attempt would have blanked every one of
them.

**0.2.0-1 was built and never published.** Its updater pointed at its own release,
which answers with its own version — so it would have reported itself up to date for
ever, and this entry would have been wrong. Caught before anyone could install it.

### Six verses were missing from the bundled Bible, and it shifted the ones after them

**This is the important one.** Relay's copy of the King James Version was short six
verses — Matthew 2:16, Matthew 22:1, Matthew 26:38, Mark 4:40, Mark 7:11 and Mark
8:8 — and four other verses had been split in two. Because Relay numbers a verse by
where it sits in the file, a missing verse does not just go missing: **everything
after it in that chapter moves up by one.**

What that meant in a service: asking for **Matthew 22:37** ("Thou shalt love the
Lord thy God…") put the words of 22:38 on the screen. **Matthew 2:23** could not be
found at all. The same shift ran through the rest of Matthew 2, Matthew 22, the
Gethsemane passage in Matthew 26, and three chapters of Mark. Everything else in
the Bible was correct, and no other book was affected.

It is fixed. Every one of the 1,189 chapters now matches the King James Version's
own verse numbering, checked verse by verse against an independent copy. **You do
not need to do anything**: the next time you open Relay it repairs its own copy of
the Bible, once, and then never again. Your services, plans, templates and settings
are untouched.

**Also fixed, in the same sweep.** A handful of verses carried the translators'
margin notes into the verse itself — Luke 17:36 ended with "this verse is not found
in most of the Greek copies" as though that were scripture — and seven other verses
had real words dropped, so Genesis 30:27 read "if I have found favour in thine eyes,
I have learned by experience" with "tarry: for" missing. Fifteen verses in total
now read as they should.


### You can now delete a service — and a big file no longer takes Relay down with it

**Erasing a sermon.** Until now, the only way to remove a recorded service was to
quit Relay and delete the folder holding *every* service you had ever recorded.
There was no middle setting. If a pastoral conversation was read into the room, or
a visiting speaker asked, the answer was all of it or none of it.

Open a service in **Library → History** and there is now an **Erase service**
button. It removes that service's transcript, the verses that were detected in it,
what you pressed during it, its timeline and its timing samples — and it tells you
how many transcript lines went. Click once to arm it, once more to confirm. It
cannot be undone and there is no hidden copy, which is the point; export the
service to Markdown first if you want to keep a record. Like every other delete, it
is refused while a service is being recorded.

**Importing a large file.** Dragging a big video into the Library used to make
Relay disappear — no error, no message, nothing in any log, usually while somebody
was setting up on a Saturday. Relay now says how big the file is and what its limit
is (256 MB), and carries on. And if a file cannot be written — a full disk is the
usual reason — the Library no longer keeps an entry pointing at a file that is not
there, which used to show up on Sunday as a screen that stayed blank.

**"Up to date" now means somebody actually asked.** The Updates screen said *up to
date* whether there was genuinely no new version, or you were offline, or nothing
had checked yet, or the update server had never once answered. It now says which,
and says so in plain words. Nothing about this interrupts you during a service — it
never did and it still does not.

**History no longer says you have no services while it is still looking.** If the
list is loading it says so, and if it could not be read it says why, instead of
telling you that you have never recorded a service.

Under the hood, and only worth knowing if something goes wrong: Relay now counts
audio it had to drop because a queue filled up, and shows it in
**Settings → Diagnostics** as *audio dropped (never heard)*. It should always be
zero. If it is not, part of the sermon did not reach the transcript, and now you
can see that instead of guessing.

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

### Service history now shows what Relay actually heard

When a wrong verse goes up, the useful question is *what words made it think that*
— and Relay has been recording exactly that on every fire since the first real
service. It just had no way to show you: the answer was in the database and nowhere
on screen.

Now it sits under the reference in **Library → History**, in quotation marks. It is
not the transcript. Relay decides on what it has heard *so far*, so the transcript
line nearest a detection is often a completely different moment — this is the
sentence the detector was actually looking at.

If a verse you did not want appears during a service, this is the line to copy into
a bug report. It is what turned the one wrong verse from the first real service into
a permanent test.

**It stays on your machine.** It is not in the service timeline, not in the
diagnostic file you can send for support, and not in a crash report.

### Small text is a little lighter

Captions and labels on raised panels were very slightly below the accessibility
contrast standard. They are two shades lighter now — you are unlikely to see the
difference, and a screen reader user or anyone in a bright room will.

### A suggestion for a verse that does not exist now says so

When speech is garbled Relay sometimes parses a reference that is real-looking and
does not exist — "Psalms 23:99". It deliberately still shows you that suggestion,
because it is the clearest possible sign that it misheard a number, and hiding it
would leave you guessing.

What it should not have done is offer an **Approve** button that looked exactly
like a working one. Pressing it failed a moment later with an error, and pressing
<kbd>A</kbd> did the same.

Now the suggestion is marked before you touch it — "Not in your Bible — Relay
misheard a number" — the Approve button is disabled and says *why* rather than just
going grey, and the keyboard shortcut will not fire it either. The suggestion stays
on screen, because it is still telling you something worth knowing.

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
numbers do *not* prove: `docs/qa/audits/PERF-2026-08-24.md`.

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

- **Sermon audio never leaves the device.** It is transcribed on your computer and thrown away. See [PRIVACY.md](docs/PRIVACY.md).
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
