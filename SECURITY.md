# Security Policy

## Reporting a vulnerability

**Please report privately first — do not open a public issue.**

Use GitHub's private reporting:
<https://github.com/devgeereact/relay/security/advisories/new>

Or email **gideonakinlotan@gmail.com**.

We will acknowledge within **72 hours** and keep you updated. If you want credit,
say so and you'll get it.

---

## What we consider most serious

Relay is a small offline desktop app, so the usual SaaS threat model mostly does
not apply. There is no server, no account, and no database of users to breach. But
there are three things we treat as **critical**, because of what this software is:

### 1. Anything that sends content off the device

Relay listens to sermons. **A bug that leaks a transcript, a verse, lyrics, or an
announcement to any third party is the most serious bug this project can have** —
more serious than a crash, more serious than remote code execution in most other
apps.

This has already happened once, and it is instructive: the crash-report scrubber
tried to blank *quoted* spans of an error message and keep the rest. An apostrophe
is a quote character and scripture is full of them, so `'God's word'` closed the
span early and the rest of the sentence went out in the clear. It is fixed
(free text is now dropped wholesale, not sifted), but **if you find anything like
it, please tell us.**

See [`PRIVACY.md`](PRIVACY.md) for exactly what is and isn't sent.

### 2. Anything that can put content on the screen that the operator did not choose

Relay drives a projector in front of a congregation. **An attacker who can control
what appears on that screen can do real harm to a real church.**

The LAN output hub is deliberately **broadcast-only** — the only inbound message it
accepts is a `hello` — precisely so that a stranger on the church WiFi can *read*
the feed but never *push* to it. If you find a way to push, that is critical.

### 3. Anything that compromises the update channel

Updates are signed, and Relay will only install an update signed with the
maintainer's private key. **If you find a way to make Relay install code we did not
sign, that is critical** — it would mean pushing arbitrary code onto every church
running Relay.

---

## Known and accepted

These are **recorded tradeoffs**, not undiscovered bugs. Reporting them is welcome
but they are already understood — see [`docs/DECISIONS.md`](docs/DECISIONS.md).

- **The LAN servers (ports 8031/8032) bind `0.0.0.0` with no authentication.**
  Kiosk screens, OBS machines and the preacher's phone are *other devices* on the
  church network, so a loopback bind would defeat the whole feature. The exposure
  is bounded: broadcast-only (no screen takeover) and path-traversal-defended.
  Someone on the church WiFi can see the verse that is already on the projector in
  front of them.

  **This is a reasonable tradeoff for a LAN appliance and an unreasonable one on an
  untrusted network.** If Relay ever ships a "public WiFi" mode, this must change.

- **Relay is not hardened against a malicious operator.** The person running it can
  already put anything they like on the screen — that is their job.

---

## Supported versions

Relay is pre-1.0. **Only the latest release is supported.** There is an
auto-updater precisely so a fix can actually reach a church; please stay current.

---

## Scope

In scope: the Relay desktop app, its LAN output servers, the update channel, the
crash-reporting scrubber, and the model download (which is checksum-verified —
report anything that can make Relay accept a model we didn't publish).

Out of scope: the security of Hugging Face, GitHub, or Sentry themselves; and
attacks that require physical access to an already-unlocked machine.
