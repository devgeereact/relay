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

The **WebSocket hub** (`:8031`) is deliberately broadcast-only — the only inbound
message it accepts is a `hello`. If you find a way to push content through *that*
socket, it is critical.

The **HTTP API** (`:8032/api/…`) is a different matter and is deliberately a
control plane: `fire`, `next`, `prev`, `clear`, `black`, `live`, `search`, with no
authentication, because it is what the preacher's phone talks to. Anyone on the
church network can therefore change what is on the wall. **That is a known,
recorded design decision ([DECISIONS §35](DECISIONS.md)), not a
vulnerability** — please do not spend your time reporting it.

What we *do* want to hear about on that surface: a route that reaches something
other than the outputs, a path that escapes `media_dir()`, a way to read
transcripts / plans / history, or a way to reach it from **outside** the LAN.

### 3. Anything that compromises the update channel

Updates are signed, and Relay will only install an update signed with the
maintainer's private key. **If you find a way to make Relay install code we did not
sign, that is critical** — it would mean pushing arbitrary code onto every church
running Relay.

---

## Threat model — T1–T10

The three sections above are the *priorities*. This table is the whole surface, so
that a gap is visible as a row rather than as an absence. **Where a row says
ACCEPTED it is a recorded decision, not an oversight** — the reasoning is in
[`DECISIONS.md`](DECISIONS.md), and the conditions that would change it
are written there too.

| | Threat | Where it lands | State |
|---|---|---|---|
| **T1** | Sermon content — transcript, verse, lyric, announcement, plan, service title — leaves the machine | Crash reports, the diagnostic bundle, any future export | **MITIGATED.** Offline by construction; crash reporting is opt-in, off by default and has no DSN in an OSS build; the scrubber drops free text wholesale rather than sifting it; the diagnostic bundle is composed as an **allow-list**, never a blocklist (`diagnostics.rs`). Pinned from both sides — `timeline_tests::nothing_a_preacher_said_reaches_the_timeline`, `timeline.test.js` |
| **T2** | Somebody on the church network changes what is on the projector | `:8032` HTTP control plane (`fire`, `next`, `prev`, `clear`, `black`) | **ACCEPTED — DECISIONS §35.** It is unauthenticated because the preacher's phone is a device on that network and has no way to hold a credential. Do not report it; read §35 for the conditions that would reverse it |
| **T3** | A *bystander's browser* is used as the weapon — a drive-by from an unrelated web page, with no attacker on the LAN at all | `<img src="http://<relay>:8032/api/black">` | **CLOSED 2026-08-20.** Mutating routes require `POST` and answer without the CORS wildcard. An image, script, stylesheet, prefetch or plain link can only ever issue `GET`. This is *not* authentication and does not pretend to be |
| **T4** | Content pushed through the kiosk/OBS socket | `:8031` WebSocket hub | **MITIGATED.** Broadcast-only. Exactly three inbound message kinds are honoured — `hello` (registers, and is answered with the template), `beat` (a liveness mark, anonymous) and `rendered` (a latency mark, documented as inert). None of them can carry content. Everything else is ignored. A way to push content through this socket is critical; please report it |
| **T5** | Reading something other than the outputs — a transcript, a plan, the database, an arbitrary file | `:8032` routes, `/media/<id>` | **MITIGATED.** Path traversal is rejected; `/media/` takes only leading digits as an id and resolves inside `media_dir()`. A route that escapes either is critical |
| **T6** | Hostile text reaching the wall as markup rather than as words — a song title, an imported lyric, a template field | The output page and `TemplateRender` | **MITIGATED.** No `{@html}` in any renderer that reaches a screen (`TemplateRender.svelte`, `Output.svelte`, `Stage.svelte`), enforced by an allow-list test; the kiosk page sets a CSP and `X-Content-Type-Options: nosniff`. Pinned by `qa-r5-template-injection.test.js` |
| **T7** | Relay installs code the maintainer did not sign | The auto-updater | **MITIGATED.** The payload is minisign-verified by the Tauri updater plugin against a public key committed in the repo, and the release gate refuses a real tag that is not signed **on both platforms independently** (CLAUDE.md rule 23). A way to make Relay install unsigned code is critical |
| **T8** | A corrupted or substituted speech model | In-app download, and install-from-file | **MITIGATED.** SHA-256 verified against the published digest before the file is renamed into place; a failed check leaves the previous model untouched. A way to make Relay accept a model we did not publish is critical |
| **T9** | A hostile language pack silently remapping book names, so a *different verse* reaches the wall | Would be the alias table | **DOES NOT EXIST YET, ON PURPOSE.** RG-19 shipped the offline bundle and **refused** the signed-language-pack half: signing needs a key, a ceremony and a distribution channel that do not exist, and an operator cannot proof-read 66 book names in a language they may not read. Until that exists, the aliases ship in the binary and this row has no attack surface |
| **T10** | A malicious operator | The console itself | **OUT OF SCOPE.** The person running Relay can already put anything they like on the screen — that is the job |

**What has no mitigation, and is a consequence of T2 rather than an oversight:**
there is **no device identity** on the LAN (the hub counts clients and deliberately
records nothing about *who* connected — [DECISIONS §35](DECISIONS.md), narrowed
by §39 to record only *when*, anonymously), and therefore **no security event log**.
Both would become possible, and worth building, only if §35 is reversed;
[`RELAY_GAP.md`](RELAY_GAP.md) §20 (a) is the written-up reversal proposal
and it is not adopted.

## Known and accepted

These are **recorded tradeoffs**, not undiscovered bugs. Reporting them is welcome
but they are already understood — see [`DECISIONS.md`](DECISIONS.md).

- **The LAN servers (ports 8031/8032) bind `0.0.0.0` with no authentication.**
  Kiosk screens, OBS machines and the preacher's phone are *other devices* on the
  church network, so a loopback bind would defeat the whole feature. Someone on the
  church WiFi can see the verse already on the projector — **and can also drive the
  screens**, via the same unauthenticated remote the preacher uses. Path traversal
  is defended; screen control is not, on purpose. Full reasoning and the conditions
  that would change it: [DECISIONS §35](DECISIONS.md).
- **A web page can no longer drive the screens — closed 2026-08-20.** It used to:
  every action was a side-effecting `GET` answered with
  `Access-Control-Allow-Origin: *`, so `<img src="http://<relay>:8032/api/black">`
  on any unrelated website blacked out the wall, with no attacker on the LAN beyond
  a victim's browser. **The mutating routes (`fire`, `next`, `prev`, `clear`,
  `black`) now require `POST` and answer without the wildcard.** An image, a script,
  a stylesheet, a prefetch and a plain link can only ever issue `GET`, so that class
  is gone. `search` and `live` change nothing and stay `GET`.

  **This is not authentication and does not pretend to be.** Anyone actually on the
  church network can still drive the screens deliberately — that is the recorded
  decision above, and the preacher's phone depends on it. What changed is that a
  *bystander's browser* can no longer be used as the weapon.

  **Still a reasonable tradeoff for a LAN appliance and an unreasonable one on an
  untrusted network.** If Relay ever ships a "public WiFi" mode, the no-auth
  decision itself must change.

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
