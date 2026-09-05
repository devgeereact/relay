# Privacy

**Relay listens to sermons. So this is the most important document in the project.**

Short version:

> **Nothing you say, sing, or show leaves your computer.**
> Relay has no accounts, no cloud, and no server. Unplug the internet and every
> feature still works — that is not a limitation, it is the design.

---

## What Relay records, and where it goes

| What | Where it lives | Does it leave the machine? |
|---|---|---|
| **The sermon transcript** | Your computer, in a local database | **No.** |
| **Which verses were detected** | Your computer | **No.** |
| **Songs, media, announcements, service plans** | Your computer | **No.** |
| **Service history** | Your computer | **No.** |
| **The audio itself** | **Nowhere, unless you turn on debug recording.** Audio is transcribed in memory and discarded — **except** while `RELAY_RECORD_WAV` is set, which writes the service to a file you name (see [Debug audio recording](#debug-audio-recording-off-by-default) below). Off unless you set it, no button turns it on. | **No.** |

Everything is in one folder:

- **macOS** — `~/Library/Application Support/com.relay.app/`
- **Windows** — `%APPDATA%\com.relay.app\`

Delete that folder and every trace of every service is gone — with one exception,
and it is one you have to have created deliberately: a WAV written by
`RELAY_RECORD_WAV` lands wherever you named it, which is normally not that folder.
Nothing else Relay writes lives outside it.

### Erasing one service, from inside Relay

Deleting the folder was, until 2026-09-03, the **only** answer this page had — and
it is a bad one. It means every service Relay has ever recorded, or none. A church
that wants one sermon gone, because a pastoral conversation was read into the room
or a visiting speaker asked, could not have it.

**Library → History → open the service → Erase service.** Two clicks, the second
one confirming, and it removes that service's transcript, the detections under it,
the operator's actions, the timeline and the latency samples — in one transaction,
so there is no half-erased state. Relay tells you how many transcript lines went.

It is **not** reversible and there is no undo, which is the point: the words are
gone from the database, not hidden in it. Export first (**Export .md**) if you want
a copy. Like every other delete, it is refused while a service is being recorded.

---

## The speech model runs on your machine

Relay does **not** send audio to a speech-recognition service. It uses a local
[Whisper](https://github.com/ggml-org/whisper.cpp) model that runs on your own
computer. The sermon is transcribed on the same computer that heard it.

The **only** time Relay talks to the internet is when you press the button to
download that model (a one-time file from Hugging Face — about 148 MB for the
recommended model, up to 1.6 GB if you choose the most accurate one), or when it
checks for an app update. Neither sends any of your content anywhere.

---

## Crash reporting is OFF, and stays off unless you turn it on

Relay can send crash reports, so that when it breaks in front of a congregation we
can find out why. **It is disabled by default and does nothing until you switch it
on in Settings** and supply your own Sentry project.

If you do turn it on, **content is stripped before anything is sent.** This is not
a promise, it is enforced in code and covered by tests:

**Sent:** the error type · the stack trace (function names and line numbers) · your
operating system · the app version.

**Never sent:** sermon transcripts · verse text · song lyrics · announcements ·
service or plan names · file paths · your name or email · your IP or machine name ·
**the free text of the error message itself.**

That last one deserves an explanation. An earlier version tried to be clever — it
blanked the *quoted* parts of an error and kept the rest, so a crash stayed
readable. That is a **blocklist**, and blocklists fail open. It failed immediately:
an apostrophe is a quote character, and scripture is full of them, so
`no verse for 'God's word to the church'` leaked *"s word to the church"* in the
clear.

There is no safe way to sift content out of a field that is *allowed* to contain
content. So it isn't sifted — **it is dropped.** The error type and the stack trace
are enough to fix a crash. (`../src-tauri/src/telemetry.rs`, and the tests beneath it.)

---

## Your church's network

Relay serves the output screens over your local network, so a projector machine,
an OBS computer, or the preacher's phone can display what you fire.

**This is LAN-only and unauthenticated**, and you should know exactly what that
means, because it is more than reading:

- Anyone already on the same church WiFi can open the output page and **see** the
  verse that is already on the projector in front of them.
- They can also **change what is on your screens.** Relay serves a remote control
  at `http://<your-computer>:8032/stage.html` — it exists so the preacher can drive
  their own reading from a phone — and it needs no password. Whoever opens it can
  put a verse up, step forwards and backwards, clear the screens, or black them
  out.
- They **cannot** reach your transcripts, your service plans, or your history.
  Nothing leaves your computer either way.

That second point is a deliberate tradeoff for a device on a church's own network,
and it is recorded as one — see **[DECISIONS §35](DECISIONS.md)**, which also
lists what would make us change it. It is written here in plain words because a
privacy document that undersells the exposure is worse than one that says nothing.

> **Until 2026-08-14 this file said the opposite** — that people on your network
> "cannot push content to your screens". That was true when it was written and
> stopped being true when the phone remote shipped, and nobody updated it. If you
> made a decision about running Relay on a shared network based on that sentence,
> please re-read this one.

**Revisit this if you run Relay on an untrusted network** — a laptop that also joins
café WiFi serves both the media files *and* the remote control to that network.

---

## What Relay does NOT have

No account. No login. No telemetry-by-default. No analytics. No ads. No tracking.
No "anonymous usage statistics". No cloud sync. **No server for your data to be
breached from.**

This is the whole point. A sermon is a named person speaking to their congregation,
and that is theirs, not ours.

---

## GDPR / UK GDPR / CCPA

Relay processes personal data (a recording of a person preaching) **entirely on the
device of the person who chose to run it.** No data is transmitted to us, because
there is no "us" to transmit it to.

If your church is a data controller, Relay does not add a processor to your chain.
It is, in that sense, no different from a Word document on the same laptop.

---

## Questions or concerns

Open an issue: <https://github.com/devgeereact/relay/issues>

If you think Relay is sending something it shouldn't be, **that is a security
issue** — please read [`SECURITY.md`](SECURITY.md) and report it privately first.
We will treat it as the most serious kind of bug there is.

## Debug audio recording (off by default)

Relay can write the microphone audio it hears to a local file, for diagnosing a
setup where the transcript is wrong. It is **off unless you explicitly turn it on**,
and there is deliberately no button for it in the app:

```bash
RELAY_RECORD_WAV=/path/to/session.wav npm run tauri dev
```

While that is set, the cleaned audio stream is buffered in memory and written to the
path you named when capture stops. It is never uploaded, never sent with a crash
report, and never enabled by anything you click.

This is sermon audio. Treat the file as you would a recording of the service —
because that is what it is. Delete it when you are done, and do not enable this
during a service you have not told people is being recorded.

## `RELAY_SENTRY_DSN` (development builds only)

A developer running Relay from source can point crash reporting at their own
Sentry project without going through Settings:

```bash
RELAY_SENTRY_DSN=https://…@…ingest.sentry.io/… npm run tauri dev
```

This does **not** weaken anything the rest of this document promises. Reports
still go through the same scrubber, so no transcript, verse text, lyric,
announcement or plan name is sent. An empty value counts as unset.

The variable is compiled out of release builds entirely — an installed copy of
Relay has no code path that reads it. An environment variable is not consent,
and the app you install must never start reporting because of something set
outside it.
