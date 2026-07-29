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
| **The audio itself** | **Nowhere. It is never saved.** Audio is transcribed in memory and discarded. | **No.** |

Everything is in one folder:

- **macOS** — `~/Library/Application Support/com.relay.app/`
- **Windows** — `%APPDATA%\com.relay.app\`

Delete that folder and every trace of every service is gone. There is no copy
anywhere else, because there is nowhere else.

---

## The speech model runs on your machine

Relay does **not** send audio to a speech-recognition service. It uses a local
[Whisper](https://github.com/ggml-org/whisper.cpp) model that runs on your own
CPU. The sermon is transcribed on the same computer that heard it.

The **only** time Relay talks to the internet is when you press the button to
download that model (a one-time ~148 MB file from Hugging Face), or when it checks
for an app update. Neither sends any of your content anywhere.

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
are enough to fix a crash. (`src-tauri/src/telemetry.rs`, and the tests beneath it.)

---

## Your church's network

Relay serves the output screens over your local network, so a projector machine,
an OBS computer, or the preacher's phone can display what you fire.

**This is LAN-only and unauthenticated**, and you should know what that means:
anyone already on the same church WiFi can open the output page and see the verse
that is *already on the projector in front of them.* They **cannot** push content
to your screens — the connection is broadcast-only — and they cannot reach your
transcripts, plans, or history.

We record that as a deliberate tradeoff, not an oversight
([`docs/DECISIONS.md`](docs/DECISIONS.md)). **Revisit it if you run Relay on an
untrusted network** — a laptop that also joins café WiFi would serve imported media
to that network too.

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
