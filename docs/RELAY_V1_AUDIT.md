# Relay — V1 Production Audit

**2026-09-03 · branch `audit/v1-production-sweep` · version `0.1.0-4`.**

Two production-readiness briefs, answered against the code that exists rather than the code
described — a 42-phase **PWA master audit** and a **Relay live-service audit** numbered §00–§105 —
and then, unlike the pass before it, **acted on**. Fourteen defects were found; ten were fixed,
tested, and verified by reintroducing the original defect and watching the new test fail. Four
were deliberately left open with the reasoning recorded.

Every number below came from a command that was run in this session, and the command is printed
beside it. Where no instrument here could reach the question, the row says **UNVERIFIED** and
names the instrument that would answer it. Nothing is scored around.

> **This document owns exactly three things**: the production decision (§1), the scorecards
> (§15), and the disposition of every phase of both briefs (§17). Everything else it cites.
> [qa/RELAY_GAP.md](qa/RELAY_GAP.md) remains the `RG-` register and is where findings are filed;
> [qa/QA_HARNESS.md](qa/QA_HARNESS.md) §0 remains the register of counts;
> [qa/audits/](qa/audits/) remains the frozen evidence. A second copy of any of those is how
> four documents came to disagree.

---

## Contents

**If you read three sections, read [§1](#1-executive-summary-and-the-production-decision) (the
decision), [§6](#6-the-fix-process-start-to-finish) (what was actually changed) and
[§16](#16-remaining-risks--what-could-not-be-verified) (what nobody has verified).**

| | | |
|---|---|---|
| [0](#0-method--and-what-it-could-not-reach) | Method | Every command that ran, and what could not be reached |
| [1](#1-executive-summary-and-the-production-decision) | **The decision** | NOT READY general · READY WITH CONDITIONS for a pilot, and the five blockers |
| [2](#2-what-relay-is--and-what-the-two-briefs-assume-it-is) | Relay is not a PWA | Why a third of one brief is N/A, said with the reason |
| [3](#3-architecture-as-discovered) | Architecture | Layers, threads, the event bus, the data flow |
| [4](#4-surface-inventory--the-routes-a-desktop-app-has-instead-of-routes) | Surfaces | Eight tabs, three served pages, one LAN control plane |
| [5](#5-the-live-path-boundary-by-boundary) | The live path | Microphone to wall, every queue and bound drawn |
| [6](#6-the-fix-process-start-to-finish) | **The fixes** | Ten, each with the test that fails if it comes back |
| [7](#7-regression-results) | Regression | Before and after, every gate |
| [8](#8-the-packaged-build--what-was-verified-against-a-real-binary) | The packaged build | Signed, launched and probed — what the last pass could not do |
| [9](#9-security-audit) | Security | The threat model, and what was hardened around it |
| [10](#10-data-integrity-and-the-database) | Data integrity | Orphans, indexes, transactions, retention |
| [11](#11-offline-network-and-recovery) | Offline & recovery | Why most of the sync brief is N/A by design |
| [12](#12-performance-and-long-service-behaviour) | Performance | Measured latency, and the two hours nobody has run |
| [13](#13-accessibility-responsiveness-and-ux) | Accessibility & UX | |
| [14](#14-privacy-retention-and-observability) | Privacy | What leaves the device, and what can now be erased |
| [15](#15-the-scorecards) | **The scorecards** | Three, because one average hides what matters |
| [16](#16-remaining-risks--what-could-not-be-verified) | **Remaining risks** | Eleven, named plainly |
| [17](#17-brief-disposition--every-phase-both-briefs) | Brief disposition | Every PWA phase 01–42 and every Relay section 00–105 |
| [18](#18-recommended-next-steps-in-order) | Next steps | In order, and none of them is a large piece of engineering |
| [19](#19-the-launch-decision) | The launch decision | |

> **This document is checked by a test.** `src/lib/v1audit.test.js` asserts that every PWA phase
> 01–42 appears exactly once, that §17.2's ranges cover the Relay brief's sections 00–105 with no
> gap and no overlap, that each scorecard's rows add up to the total printed beside it, that every
> fix in §6 says what it changed and how it was proved, and that the counts quoted here match
> [qa/QA_HARNESS.md](qa/QA_HARNESS.md) §0 rather than restating them. **Two of the three scorecard
> totals were wrong in the first draft** — which is why the test exists rather than a promise to
> be careful.

---

## 0. Method — and what it could not reach

**What ran, in this session, on this machine.**

| | Command | Result |
|---|---|---|
| Frontend suite | `npx vitest run` | **964 passed**, 0 skipped, 71 files |
| Rust suite | `cd src-tauri && cargo test` | **660 passed**, 0 failed, 17 ignored |
| End-to-end fire path | `cargo test e2e::` | **38 passed, 0 ignored** |
| Format gate | `cargo fmt --all -- --check` | clean |
| Lint gate (a CI gate on both platforms) | `cargo clippy --all-targets -- -D warnings` | clean |
| Frontend build | `npm run build` | clean |
| **Production bundle** | `npm run tauri build` | **`Relay.app` + `Relay_0.1.0-4_aarch64.dmg`, zero warnings** |
| **Hardened-runtime signing** | `./scripts/sign-local.sh` | hardened runtime **ON**, mic entitlement **present**, usage string **present** |
| **The packaged binary, launched** | isolated `RELAY_DB_PATH`, killed after 12 s | booted clean; **exactly one** `console: webview up (operator)` |
| **The packaged LAN surface** | `curl` against the running bundle on `:8032` | see §8 — every header and status verified in production, not in a test |
| Detection gate | `cargo test eval::tests::print_scorecard -- --nocapture` | 74 cases · **100 % recall · 0 wrong verses · 0 paraphrases auto-fired** |
| Version agreement | `npm run version:check` | `0.1.0-4` consistent across all three files |
| Surface inventory | `node scripts/qa-inventory.mjs` | 48 components (47 reachable), 463 controls, 133 commands, **0 dead controls, 0 unreachable commands, 0 controls without an accessible name** |
| Production dependencies | `npm audit --omit=dev` | **0 vulnerabilities** |
| All dependencies | `npm audit` | 10, **every one in dev tooling** — see RG-98 |
| **Rust dependencies** | `cargo audit` (installed 2026-09-04 — it had **never been run**) | 3 advisories: `h2` fixed by a lockfile update; **two 7.5-HIGH `quick-xml`** advisories reach Relay through `plist` through Tauri and need an upstream bump — RG-101 |
| Update channel | `npm run updater:check` (written by this pass) | **HTTP 404 — the endpoint resolves to nothing** |

**What did not run, and why.** There is no church, no projector, no congregation, no second
operator, and no code-signing certificate on this machine. There is no recording of a real
sermon — so **word error rate remains unmeasured in every language**, which is the sentence
[LANGUAGES.md](LANGUAGES.md) has carried since the beginning and this audit does not soften it.
Windows was not built or run: CI covers it, this machine cannot.

**One thing about method that mattered more than any single finding.** The pass before this one
recorded that an exploratory sweep reported eleven contradictions and that re-checking by hand
disproved three. The same failure appeared again here, in the instrument rather than the prose:
the first version of the new *"no unbounded queue on the audio path"* test used `[^>]*` for a
turbofish, so it could not match `mpsc::channel::<Vec<f32>>()`, **matched nothing, and passed.**
It was caught only by deliberately reintroducing the defect. Every new test in this pass was
then verified the same way — the fix reverted, the test watched to fail — and two of them grew
a guard that fails when the scanner itself stops seeing anything.

---

## 1. Executive summary and the production decision

> ## NOT READY for general release · READY WITH CONDITIONS for a supervised pilot

Unchanged in verdict from [qa/RELAY_GAP.md](qa/RELAY_GAP.md) §24 (2026-08-31), re-verified, and
now with **one blocker removed and one added**.

**What holds up, and it is most of the product.** The live path is genuinely well built and the
safety architecture is not decorative. Content reaches a wall through exactly one function
(`channels::broadcast_content`, one caller, pinned). Only `DetectionMethod::Direct` can
auto-fire, enforced in `router::decide` *before* any threshold is consulted, so no number can
promote a paraphrase. There are **zero** `unwrap()`/`expect()` calls outside tests in the seven
service modules. The detection gate scores **100 % recall, 0 wrong verses, 0 paraphrases
auto-fired** over 74 labelled cases in four languages. Production dependencies carry **zero**
vulnerabilities. And — new evidence this pass — the thing actually **builds, signs under the
hardened runtime with the microphone entitlement intact, boots, serves its LAN surface with the
right headers, and refuses the drive-by**, all verified against the packaged binary rather than
against a test double.

**What blocks a general release.**

| | Blocker | Status |
|---|---|---|
| 1 | **A wrong verse reached a real congregation** on 2026-08-30 | Root-caused and fixed (`detection::anchor_for_bare_verses`). One service is one sample |
| 2 | **Word error rate has never been measured, in any language** | Needs 30 minutes of real sermon audio. The ruler is built and runs in CI |
| 3 | **Neither platform has a code-signing certificate** | `gh secret list` holds zero of the fourteen. Every release so far went out unsigned, on both platforms |
| 4 | **Nobody but the author has ever run a service** | Needs a Sunday |
| 5 | **The auto-updater points at a URL that returns 404** | Verified again this session. Still true — it is a *publishing* action, not a commit |

**What changed about #5, and why it is no longer invisible.** It is the same 404, but it can no
longer hide. `npm run updater:check` reads the endpoint out of both Tauri configs and fetches
it, and **Settings → Updates** now reports the state of the *channel* rather than the absence of
news. That row used to say *"up to date"* when no check had ever run, when the laptop was
offline, and when the manifest had been 404 since the day Relay was installed. One reassuring
sentence over four different situations, on the one path by which a fix reaches a church that
already has Relay.

**And the one this pass removed from the informal list**: a church now has a way to **erase a
recorded sermon** from inside Relay. Until 2026-09-03, [PRIVACY.md](PRIVACY.md)'s only answer to
*"remove that"* was *delete the folder* — every service ever recorded, or none. Relay's most
sensitive holding is verbatim text of what a preacher said to a congregation, every document
here promises it never leaves the device, and none of them could say how to get rid of it.

---

## 2. What Relay is — and what the two briefs assume it is

**Relay is a Tauri v2 desktop application. It is not a Progressive Web App, and the distinction
is not pedantry — it changes the answer to about a third of the PWA brief.**

Relay has **no service worker, no web app manifest, no installability prompt, no push
subscription, no background sync, no HTTPS origin, no server, no accounts, no roles, no
sessions, no tenants, no payments, no e-mail, and no cookies.** A phase of the brief that asks
about any of those is not a phase Relay is failing; it is a phase asking about a different kind
of program. Marking those **N/A — and saying exactly why** is the only honest disposition, and
§17 does it phase by phase rather than in a paragraph.

Three PWA phases *do* land, harder than they would on a web app, because Relay serves real HTML
over a real network to real browsers:

- The **output page** (`output.html`) and the **stage page** (`stage.html`) are served over
  plain HTTP on the church LAN to OBS, kiosk screens and the preacher's phone. Those are
  ordinary browsers on an untrusted-ish network, running a page whose look is assembled from
  template JSON that may have arrived in an e-mail. Content-Security-Policy, `nosniff`, CORS and
  method gating all apply, and all four were verified against the packaged binary (§8).
- **Offline-first** is not a nice-to-have that a service worker approximates; it is the
  product's first constraint. Every core feature works with the network unplugged because there
  is nothing to reach.
- **Update safety** is real and is the subject of blocker #5.

The Relay-specific brief is the one that fits, and §17 answers every one of its sections, §00 to §105.

---

## 3. Architecture, as discovered

A single native binary. No server, no account, no cloud dependency on the live path.

| Layer | What is actually there |
|---|---|
| Shell | Tauri v2, one webview window; `src-tauri/src/main.rs` (5,832 lines) registers **133** commands |
| Engine | Rust — capture, DSP, whisper, detection, routing, rendering, all in-process |
| UI | Svelte 4 + Vite, **48** components (47 reachable), **463** controls, one store (`capture.js`, 2,215 lines) |
| Data | SQLite via `rusqlite`, **21** tables, schema version 2, migrations as ordered retryable rungs |
| Distribution out | A native output window, a WebSocket kiosk hub on `:8031`, an HTTP server on `:8032` |
| Control in | Tauri IPC, and an unauthenticated LAN HTTP control plane (`main::remote_api`) |

**Three threads carry the live path**, and the division is the interesting part:

- `relay-stt` — the decoder. 16 MiB stack (`whisper_full` overflows the default 2 MiB and
  SIGSEGVs silently after the first transcript).
- `relay-detect` — semantic scan, locks, SQLite write, event emit. Behind a bounded
  `sync_channel(8)` that sheds a PARTIAL and blocks on a FINAL, and counts what it sheds.
- `relay-history` — snapshots latency percentiles into `perf_samples` every 60 seconds.

**One event bus, 19 named events**, eighteen with listeners; `ipc.test.js` enforces the contract
in both directions and requires each named exception to still be both emitted and unheard.

**Data flow, end to end.** Microphone → cpal callback → *(bounded queue, counted)* → denoise +
auto-gain + VAD + chunker → *(bounded queue, counted)* → whisper worker → transcript event to
the operator's eyes → *(bounded queue, counted)* → detection (direct + semantic + context) →
router gate → `pipeline::Fire` → `broadcast_with_clock` → `pipeline::preflight` →
`channels::broadcast_content` → native window · kiosk WebSocket · LAN HTTP. Nine named latency
stamps ride the whole way on one monotonic clock.

**The three queues in that chain were, before this pass, one bounded and two unbounded** — while
the handbook described the path as bounded. That is F-06 below.

---

## 4. Surface inventory — the routes a desktop app has instead of routes

There is no router, no URL, no deep link and no browser history. The equivalents are **eight
tabs**, three **served pages**, and one **LAN control plane**.

| Surface | What it is | Reachable how | Auth | Offline |
|---|---|---|---|---|
| Live | The run surface: console + plan, merged | Tab | none (local operator) | fully |
| Outputs (`channels`) | Screens, templates per screen, health | Tab | none | fully |
| Templates · Themes | The look, and the style layer beneath it | Tab | none | fully |
| Library | Bible · Saved · Lyrics · Media · Announcements · Graphics · History | Tab | none | fully |
| Planner | Build a plan. **Cannot fire to an output** | Tab | none | fully |
| Settings | Model, audio, rooms, privacy, updates, diagnostics, dashboard | Tab | none | fully |
| Help | Including *"what the AI is bad at"* | Tab | none | fully |
| `output.html` | The projector page | Native window **and** `http://<host>:8032` | none — by decision | fully |
| `stage.html` | The preacher's phone / confidence monitor | `http://<host>:8032` | none — by decision | fully |
| `/api/*` | search · live · fire · next · prev · clear · black | `http://<host>:8032` | **none — DECISIONS §35** | fully |

**Verified this session, against the packaged binary**: every registered command is addressed by
the frontend (133/133), no rendered control is dead, no control lacks an accessible name, and
exactly one component (`__r6probe.svelte`, a deliberate test probe) is rendered by nothing.

**The authorisation model is that there is no authorisation model, and it is a recorded
decision** (DECISIONS §35), not an oversight. There is one operator, on one machine, and the LAN
control plane is deliberately open so a preacher can drive their own reading from a phone. §9
says what that does and does not cost, and what was hardened around it.

---

## 5. The live path, boundary by boundary

Every arrow is a failure point, and the brief's closing rule is that each one needs **detection →
state → feedback → recovery → verification**. This is that path with the queues drawn in, because
the queues are where this audit found work:

```
  microphone
      │
      ▼  cpal real-time callback
  ╔═══════════════════════════════╗
  ║ CAPTURE_QUEUE   512 buffers   ║  ← was UNBOUNDED (F-06). Sheds, never blocks:
  ╚═══════════════════════════════╝    blocking a device callback kills the stream
      │
      ▼  relay-audio: RNNoise · auto-gain · VAD · 400 ms chunks, 200 ms hop
  ╔═══════════════════════════════╗
  ║ STT_QUEUE       256 chunks    ║  ← was UNBOUNDED (F-06). ≈ 51 s of grace,
  ╚═══════════════════════════════╝    ~13 MB ceiling instead of a growing one
      │
      ▼  relay-stt: whisper, 16 MiB stack, cadence = this machine's own decode cost
      │
      ├──────────────▶ emit stt://transcript ──▶ the operator's eyes
      │                (the ONE thing that runs on the decoder's thread, rule 33)
      ▼
  ╔═══════════════════════════════╗
  ║ DETECT_QUEUE    8 updates     ║   sheds a PARTIAL, blocks on a FINAL
  ╚═══════════════════════════════╝
      │
      ▼  relay-detect: direct + semantic (TF-IDF) + context memory
      │
      ▼  router::decide — ONLY DetectionMethod::Direct may auto-fire,
      │                   checked BEFORE any threshold is consulted
      ▼
   pipeline::Fire ──▶ broadcast_with_clock ──▶ pipeline::preflight
                          (one caller)            (the pre-air validator)
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
   native window      kiosk WS :8031       LAN HTTP :8032
          │                   │                   │
          └────── heartbeat every 2 s, stale after three ──────▶ OutputHealth
```

Every shed above is **counted**. Nine named latency stamps ride the whole chain on one monotonic
clock, and the trace id survives to the output page so the last leg — fire sent to pixels on a
projector — is measured rather than assumed.

| Boundary | Bound | What happens when it is exceeded | Counted? |
|---|---|---|---|
| cpal callback → clean/chunk thread | **512 buffers** (`audio::CAPTURE_QUEUE`) | shed, never blocked — blocking a device's real-time callback kills the capture stream | **yes**, `latency::note_dropped_audio` |
| chunk → whisper worker | **256 chunks** ≈ 51 s (`stt::STT_QUEUE`) | shed | **yes**, same counter |
| transcript → detection | **8** (`main::DETECT_QUEUE`) | PARTIAL shed, FINAL blocks | **yes**, `dropped_partials` |
| detection → router | in-process | — | — |
| router → `pipeline::Fire` | in-process | only `Direct` may auto-fire, before any threshold | — |
| `Fire` → wall | `broadcast_with_clock`, one caller, `preflight` inside it | refuses only what is unambiguously broken and silently so | timeline event |
| wall → screen | heartbeat every 2 s, stale after three | badge stops saying On Air | `OutputHealth` |

The first two rows are new: they were unbounded before this pass, which meant a stalled consumer
turned into memory and then into rule 31's failure — a transcript minutes behind the preacher
while every instrument reads a zero backlog.

---

## 6. The fix process, start to finish

Ten fixes. Each one is: **the problem → the root cause → the change → the files → the test →
how the test was proved to be a real test → the result.** No fix is reported here that was not
run.

### F-01 · The Library imported a file of any size, and the process died — P1 · RG-88

**Problem.** Dragging a large video into Library → Media killed Relay with no error, no message
and nothing in any log.

**Root cause.** An imported file does not arrive as a path. The webview's `<input type=file>`
yields bytes, so `capture.js::fileToBase64` builds the whole file as a `Uint8Array`, then as a
binary string, then as a base64 string; Tauri serialises that string across the IPC bridge; and
`main::import_media` base64-decodes it into a further complete copy before writing it to disk.
**Four copies of the file exist at the peak, and there was no size limit anywhere.**

**Change.** One limit, `256 MiB`, held on **both** sides of the bridge. The webview copy is the
one that prevents the allocation (it refuses before reading a byte, using `File.size`); the Rust
copy is the door that cannot be walked past, because a Tauri command is invokable from the
webview whatever the UI does. The Rust check reads the *base64* length so it refuses before
allocating the decoded copy. The guard lives in `fileToBase64` — the one function media,
graphics, documents and lyric files all import through — for the same reason
`broadcast_with_clock` holds the pre-air validator: a guard added per call site is the guard that
will be missing from the fifth one.

**Files.** `src/lib/stores/capture.js`, `src-tauri/src/main.rs`.

**Tests.** `main::import_guard_tests` (3 of 5) and `src/lib/mediaimport.test.js` (5), including
that the refused path never calls `arrayBuffer()` — a test that called it would be a test
proving the allocation still happens — and that both sides hold the same number.

**Proved real.** `if false &&` in front of the Rust guard: two tests fail.

### F-02 · A failed import left a Library entry that plays nothing — P1 · RG-88

**Problem.** When the file write failed, the `media_assets` row survived with `path = ''`.
`list_media` shows it exactly like a healthy asset, and `serve_media_file` never consults `path`
— it scans the media directory for an `{id}_` prefix, finds nothing, and answers 404. So the
failure surfaced on Sunday, as a blank output, with no message.

**Root cause.** The row has to be inserted first, because its id is half the on-disk name; the
write then followed, with no transaction and no undo. The realistic way to fail is a full disk,
which is also exactly when a church is importing the last thing before a service. This was a
**known** defect: `qa.rs` carried a test named
`a_half_finished_media_import_leaves_a_row_that_serves_a_404` pinning it since 2026-08-16.

**Change.** `main::write_media_file` deletes the row when the write fails. It is a separate
function specifically so the failure branch can be *executed* by a test rather than reasoned
about — a cleanup path that has never run is a cleanup path that does not work.

**Files.** `src-tauri/src/main.rs`, `src-tauri/src/qa.rs`.

**Tests.** `import_guard_tests::a_failed_write_leaves_no_media_row_behind` (the write is made to
fail by naming a directory that is not there — the same way a full disk fails), plus the QA test
**inverted, not deleted**, to `a_half_finished_media_import_leaves_no_row_behind`, keeping the
shape of the original defect in its doc comment.

**Proved real.** Removing the `delete_media` line: the test fails.

### F-03 · There was no way to erase a recorded sermon — P1 · RG-89

**Problem.** `transcripts.text` is verbatim, near-real-time text of what a preacher said to a
congregation, and `detections.heard_text` is the exact sentence behind every verse that reached
a wall. [PRIVACY.md](PRIVACY.md)'s only answer to *"remove that"* was **delete the folder** —
quit Relay, find `~/Library/Application Support/com.relay.app`, and destroy every service ever
recorded, or keep all of them.

**Root cause.** Nothing was ever built. A `grep` for retention or deletion across `db/`,
`main.rs`, `capture.js` and `History.svelte` returned **zero hits**.

**Change.** `db::delete_service` removes a service and everything under it — detections,
transcripts, cues, timeline events, latency samples — **children first, in one transaction**,
returning the number of transcript rows removed so the surface that asked can say what it did
rather than claim a success in the abstract. A `delete_service` command sits behind the service
lock like every other `delete_*`, including (necessarily) the service being recorded right now.
The UI is a two-step arm/confirm **Erase service** in History — never a native `confirm()`,
which Tauri's webview does not implement and which would therefore delete nothing while
reporting success.

**Files.** `src-tauri/src/db/services.rs`, `src-tauri/src/main.rs`, `src-tauri/src/servicelock.rs`,
`src/lib/stores/capture.js`, `src/lib/views/library/History.svelte`, `docs/PRIVACY.md`.

**Tests.** `db::services::erase_tests` (4), run against the **real schema with foreign keys ON**
— a delete tested with foreign keys off is a delete whose ordering was never checked.

**The question a reviewer should ask, answered.** *Can this erase the service Relay is currently
writing to?* No, from two directions. The service lock refuses it for the whole of a recording;
and `Session` — the only thing that carries a service id into a transcript insert — is written
in exactly one place (`start_service`, which creates a fresh row) and cleared in exactly one
(`end_service`, before the lock is released). Between services it is `None`, so there is no id
left pointing at a row that has gone. Frontend crash recovery restores position only and never
re-attaches a service id, which is the same separation that stops it restoring on-air-ness.

**Proved real.** Dropping the `WHERE service_id = ?1` from one child delete:
`erasing_one_service_leaves_the_others_untouched` fails.

### F-04 · The LAN HTTP server held a silent connection forever — P2 · RG-90

**Problem.** The first read of a connection had no deadline. A socket that connected and then
said nothing kept a task, an 8 KiB buffer and a file descriptor for the life of the process.

**Root cause.** `stream.read(&mut buf).await`, unguarded, on a server bound to `0.0.0.0`.
Nothing hostile is needed to produce one: a browser opening a speculative connection, a kiosk
sleeping between the handshake and the request, a scanner sweeping the church LAN. They
accumulate silently across a service and are freed only by quitting Relay.

**Change.** `REQUEST_READ_TIMEOUT` = 5 s — far longer than a LAN request line takes, far shorter
than a service.

**Files.** `src-tauri/src/channels.rs`.

**Test.** `a_connection_that_never_speaks_is_dropped`. The client's own read carries a deadline
**so that a regression is a failing test rather than a hanging one** — without the server-side
timeout that read never returns, and a test that hangs forever in CI is worse than no test.

**A second, smaller lesson inside the first.** The test was originally written under
`#[tokio::test(start_paused = true)]`, to assert the real five seconds without spending them —
and it was **flaky**, failing about one run in ten. With two tasks holding timers, tokio's
auto-advance can reach the *client's* deadline before the server task has registered its own.
The server now takes its deadline as a parameter, `run_output_http_server` passes the constant
and is its only production caller, and the test drives 200 ms while separately asserting that
the shipped constant is still five seconds.

**Proved real, twice over.** Replacing the timeout with a bare read: the test fails in 10 s with
*"the server never dropped an idle connection"*. And the parameterisation caught its own wiring
error — before the argument was actually threaded through, the test passed **in 5.16 seconds**,
which is the constant's value and not the test's. A green test that takes twenty-five times
longer than it should is a test measuring something other than what it claims.

### F-05 · Media files were read whole into memory, per request — P2 · RG-90

**Problem.** `serve_media_file` did `std::fs::read` — the entire file into a `Vec<u8>` before a
single byte went out.

**Root cause.** Written for images, kept for video. A 400 MB background loop cost 400 MB
resident **per request**, and a wall, a stage screen and an OBS machine asking for the same clip
during a service cost three copies of it on the laptop running the sermon.

**Change.** Open, measure, stream in 64 KiB frames. A client that disappears mid-clip is an
ordinary end, not an error; a read error truncates rather than pads, because the header already
promised a length and inventing bytes hands a player a corrupt file. `nosniff` added while
there.

**Files.** `src-tauri/src/channels.rs`.

**Test.** Verified end to end against the **packaged binary** (§8) rather than in a unit test:
`media_dir()` is app-data-derived and a unit test would have had to fake the thing being tested.
**Stated as the limit it is** — the streaming path is exercised by a 404 and by reading the code,
not by a test that serves a large file.

### F-06 · Two of the three queues on the audio path were unbounded — P2 · RG-84

**Problem.** `CLAUDE.md` rule 33 describes the path into detection as bounded, shedding partials
and counting what it sheds. That was true of `main::DETECT_QUEUE` and of **nothing in front of
it**: the capture callback → clean/chunk hop and the chunk → whisper hop were plain
`mpsc::channel()` with no capacity and no counter.

**Root cause.** The rule was written when the third queue was added and was never re-read against
the two in front of it.

**Change.** Both are `sync_channel` with a non-blocking `try_send`. A full queue **sheds and
counts** (`latency::note_dropped_audio`), surfaced in Settings → Diagnostics as **audio dropped
(never heard)** — worse news than a shed partial, and coloured accordingly, because a shed
partial is re-decoded a moment later and shed audio is a piece of the sermon Relay never heard.
The capture callback may never block: stalling a device's real-time callback kills the stream
outright. `Disconnected` is deliberately **not** counted — that is the ordinary end of a
capture, and counting it would put a four-figure number in the report every time an operator
presses Stop.

**Sizing, and why these numbers.** 512 capture buffers is several seconds of grace against a
consumer doing microsecond work. 256 chunks is about fifty seconds at one per 200 ms hop, and a
hard ceiling of roughly 13 MB rather than a figure that grows with the sermon.

**Files.** `src-tauri/src/audio.rs`, `src-tauri/src/stt.rs`, `src-tauri/src/main.rs`,
`src-tauri/src/latency.rs`, `src/lib/views/Settings.svelte`, `CLAUDE.md`.

**Test.** `hardrules.test.js` — *"rule 33 — no unbounded queue anywhere on the audio path"*.

**Proved real, and this is the one that nearly went wrong.** The first version of the regex used
`[^>]*` for the turbofish, so it could not match `mpsc::channel::<Vec<f32>>()`, **matched
nothing, and passed.** Restoring the unbounded channel did not fail it. It now also asserts that
its own scanner can still *see* both queues, so a check that has quietly narrowed fails instead
of reporting a clean path.

### F-07 · "Up to date" meant four different things — P2 · RG-92

**Problem.** Settings → Updates printed **Update status · up to date** whenever nothing was
waiting: when there genuinely was no newer version, when the laptop was offline, when no check
had ever run, and when the update manifest had been returning 404 since the day Relay was
installed.

**Root cause.** `checkForUpdate` swallowed every outcome into `null`, under a rule that is
*correct* — a failed update check must never interrupt an operator mid-service. The rule was
kept and the outcome was thrown away with it.

**Change.** `updateChannel` records `{ state, at, detail }` — `unchecked` / `ok` / `unavailable`
/ `failed` — and `describeChannel` is the one place that turns it into words. A build with no
updater at all (a browser, a dev build) is `unavailable`, **not** a fault, so the row is not
noisy on a dev machine and does not train the operator to ignore it. Failures still never throw
and are still invisible during a service.

**Files.** `src/lib/updater.js`, `src/lib/views/Settings.svelte`, `docs/RELEASING.md`, `CLAUDE.md`.

**Tests.** `src/lib/updatechannel.test.js` (7), including the RG-83 shape exactly — the plugin
throwing on a 404 manifest — and that a failed check still resolves rather than rejecting.

### F-08 · History said "No services yet" while loading, and while failing — P2 · RG-93

**Problem.** One sentence for three facts: before `list_services` answered, when it answered with
nothing, and when it **failed**.

**Root cause.** `listServices` is a GROUP 2 read that returns `[]` on failure, so the array
carried no information at all about which had happened. This was a **known** defect —
`surface.test.js` had pinned it since 2026-08-15, with the note that two views were fixed and
this one was not.

**Change.** An `asked` flag plus `readErrors.listServices`, rendering `Loading` / `ErrorState` /
the empty sentence — the same three-way `TemplateGallery` already used. `ErrorState` is
assertive, so a screen reader hears it too.

**Files.** `src/lib/views/library/History.svelte`, `src/lib/surface.test.js`.

**Tests.** The two `surface.test.js` cases that had pinned the defect, **inverted**:
*"History says Loading, not 'No services yet', before list_services answers"* and *"…and says the
REASON, not that sentence, when list_services FAILS"*. The second asserts `[role="alert"]`, so a
screen reader hears the failure too.

**Proved real.** The two pinning tests failed the moment the fix landed, and were only then
inverted. That ordering *is* the proof: the fix was confirmed by the defect's own tests before
they were rewritten to describe the guarantee.

### F-09 · The Rust suite could silently talk to somebody else's web server — P2 · RG-94

**Problem.** Mid-audit, the whole Rust suite failed on `kiosk_ws_forwards_published_content` with
a response carrying `x-powered-by: PHP/8.5.9`.

**Root cause.** The channel tests bound **fixed** ports (8199–8205). An unrelated PHP dev server
for a different project held `127.0.0.1:8199`. Relay's server bound `0.0.0.0:8199` and
**succeeded** — tokio sets `SO_REUSEADDR` — the more specific loopback binding won the
connection, and the test spent its life talking to a foreign server. Nothing errored, the server
printed that it was listening, and the failure read as a regression in code that had not changed.

**Change.** `free_port()` asks the OS, on loopback, so a foreign wildcard listener cannot shadow
it.

**Files.** `src-tauri/src/channels.rs`.

**Test.** There is no new assertion, and adding one would be theatre: the guarantee is that the
tests no longer *name* a port, and a test asserting that would be asserting the absence of a
literal. The evidence is the suite itself — **all 644 Rust tests pass with the foreign PHP server
still listening on 8199**, which is the exact condition that broke them.

**Result.** Green, with the collision still present on the machine.

### F-10 · Missing indexes on the three foreign keys every history query walks — P2 · RG-91

**Problem.** `transcripts.service_id`, `detections.transcript_id` and `cues.service_id` had no
index.

**Root cause.** SQLite indexes a PRIMARY KEY and a UNIQUE constraint automatically. It does
**not** index a `REFERENCES` column, and nothing had added one. So opening a service in History,
building its timeline, replaying a moment in it and — since F-03 — erasing it all scanned the
whole table, and those are the two tables that grow one row per utterance, without limit, for as
long as a church keeps using Relay.

**Change.** Added to `docs/data/schema.sql` (which **is** the shipped baseline, compiled into the
binary) and to a retryable `ensure_history_indexes` rung, so a fresh install gets them at
creation and an existing one on the next boot. `CREATE INDEX IF NOT EXISTS` only: no table
rebuild, nothing dropped, no intermediate state to strand (rule 25).

**Files.** `docs/data/schema.sql`, `src-tauri/src/db/services.rs`, `src-tauri/src/db/mod.rs`.

**Tests.** `db::services::index_tests` (3), including `EXPLAIN QUERY PLAN` — the only witness
that tells a *created* index from a *used* one — and a retryability test that runs the rung three
times against a pre-index database.

### Also fixed, smaller

| | | |
|---|---|---|
| **JSON replies carried no `nosniff`** | The static path set it; the control plane — the one surface that echoes attacker-influenceable strings — did not | `a_json_reply_is_never_content_sniffed`, plus a companion asserting the mutating routes still withhold the CORS wildcard |
| **A flaky frontend assertion** | A `surface.test.js` check one scheduler pass after mount failed about one full-suite run in five, on a subject that had not changed. A flaky test trains whoever sees it red to run it again rather than read it | `until()` waits for the DOM and throws with a message; verified to fail loudly by pointing it at a string that never appears |
| **`crossrefs.test.js` could not see a Markdown link** (RG-87) | Four dimensions of citation were checked and the fifth — `[text](path.md)` — was not, which is how twenty-eight broke in one reorganisation and every test stayed green | A fifth check, resolving every relative link in every tracked `.md`, honouring the frozen-audit redirect map; plus a guard counting the links it found, because a regex that matches nothing passes |
| **Rule 33 overclaimed** (RG-86) | *"Nothing else runs on the decoder's thread"* is absolute in the rule and not in the code: one `emit("stt://transcript")` runs there, deliberately | The rule now states its single named exception. A rule stated absolutely and held approximately is how the next person justifies the second exception |

---

### F-11 · The bundled KJV was six verses short, so correct references put the next verse on the wall — P0 · RG-99

**Problem.** `src-tauri/data/kjv.json` held **31,100** verses. The KJV has **31,102**. Six verses
were missing (Matthew 2:16, 22:1, 26:38; Mark 4:40, 7:11, 8:8) and four were split into two
(1 Samuel 20:42, 1 Kings 22:43, 3 John 14, and Revelation 13:1, whose opening clause stood alone
as a nineteenth "Revelation 12:18"). The two errors very nearly cancelled, which is how the total
stayed plausible.

**Root cause.** `import_full_kjv` numbers a verse by its POSITION in the array — `vi + 1`. A
chapter missing a verse therefore does not lose its last verse; it **renumbers every verse after
the gap**. "Matthew 22:37" — the great commandment, and about as commonly preached as a verse
gets — returned the words of 22:38. "Matthew 2:23" did not exist at all. Nothing caught it because
both instruments were derived from the same file: `db::tests::seeds_full_kjv` asserted
`> 31_000` beside a comment recording that the bundled file held 31,100 as though that were a
rounding difference, and `detection::VERSES_PER_CHAPTER` had been regenerated FROM the broken
corpus and was pinned to it by a test. Two green checks, both agreeing with the defect.

**Change.** The corpus was repaired verse by verse against an independent public-domain KJV whose
verses carry explicit NUMBERS rather than positions: the six missing verses inserted, the four
splits merged, Revelation 12 back to 17 verses and Revelation 13:1 whole. `detection::VERSES_PER_CHAPTER`
regenerated from the repaired file. `db::migrate` gained a forward-fill rung — any database whose
verse count is not exactly 31,102 is re-imported once.

> **CORRECTION, 2026-09-05.** The sentence that stood here — *"an install made before today is
> repaired on next launch"* — **was false when it was written**, and the audit that wrote it did
> not check it. The rung was placed in `baseline_forward_fill`, the `user_version == 0` branch,
> and every shipped build stamps `user_version = 2` on a fresh install: no copy of Relay in
> anyone's hands would ever have run it (RG-102). On the one branch it could run down, it opened
> a transaction inside `import_full_kjv`'s and returned
> `cannot start a transaction within a transaction`, which `db::open` propagates — so it had
> never worked anywhere (RG-103). Both are fixed: `SCHEMA_VERSION` is 3, the repair is a rung in
> `run_migrations` that probes the count **and** the gloss defect the count cannot see, and the
> insert no longer owns its own transaction. Two further defects fell out of making it run at
> all — it erased the verse reference from every past detection (RG-104), and it deletes 31,102
> parent rows against an unindexed child column, costing **8.3 s at boot** (RG-105). The claim is
> now held by tests that drive the real `migrate` on a v2 database and assert the DATA.

**Files.** `src-tauri/data/kjv.json`, `src-tauri/src/db/verses.rs`, `src-tauri/src/db/mod.rs`,
`src-tauri/src/detection.rs`, `src-tauri/src/qa.rs`.

**Test.** `db::verses::corpus_tests::the_bundled_kjv_matches_the_kjvs_own_versification` pins all
**1,189 chapters** against the KJV's own versification, taken from outside this repository — the
pin had to come from outside, because every table inside it was derived from the file it would be
checking. `the_repaired_references_hold_their_own_words` asserts the ten repaired references by
their words. Both were watched to fail: stashing the old `kjv.json` turns them red with
*"1 Samuel 20 has 43 verses, the KJV has 42"* and *"assertion failed: at(40, 2, 16).starts_with…"*.
`db::tests::seeds_full_kjv` now asserts `== 31_102`, and `qa::cold_start`'s seed audit with it.

**Result.** 649 Rust tests pass, 0 failed. The detection gate is unchanged at 74 cases · 100 %
recall · 0 wrong verses · 0 paraphrases auto-fired.

### F-12 · Marginal notes reached the wall as scripture, and supplied words were deleted from it — P1 · RG-100

**Problem.** Two failures of the same rule, in opposite directions. Eight verses rendered a
translator's note as though it were the text — Luke 17:36 ended *"…and the other left. this verse
is not found in most of the Greek copies"* — and seven verses had real words removed, so Genesis
30:27 read *"if I have found favour in thine eyes, I have learned by experience"* with "tarry: for"
gone.

**Root cause.** `verses::is_gloss` decided by wording: a brace group was a marginal note if it
contained `": "`. Seven verses carry a colon inside their italicised supplied words
(`{tarry: for}`, `{men: so}`, `{any: he is}`, and four more), and eight notes carry no colon at all
(`{feed or, rule}`). No wording rule separates the two classes, because the corpus does not mark
them.

**Change.** Position decides. Every marginal note in `kjv.json` sits in the run of brace groups at
the END of a verse — checked group by group over all 31,102 — so `is_trailing_run` identifies them,
and the wording markers are kept only for the single note that appears mid-verse (Hebrews 10:34,
caught by its `...` lead-in).

**Files.** `src-tauri/src/db/verses.rs`.

**Test.** `corpus_tests::supplied_words_containing_a_colon_survive` and
`corpus_tests::a_trailing_note_is_never_scripture_however_it_is_worded`. Restoring
`inner.contains(": ")` turns both red, verified. The whole corpus was then re-diffed against the
independent KJV: **15 verses changed and no others** — 7 restored, 8 cleaned.

## 7. Regression results

Run after the last change, in this order, from a clean tree:

| Gate | Before the pass | After |
|---|---|---|
| `cargo fmt --all -- --check` | clean | **clean** |
| `cargo clippy --all-targets -- -D warnings` | clean | **clean** |
| `cargo test` | 629 passed / 17 ignored | **649 passed / 0 failed / 17 ignored** |
| `cargo test e2e::` | 38 / 0 ignored | **38 / 0 ignored** |
| `npx vitest run` | 927 in 68 files | **952 in 71 files, 0 failed** |
| `npm run build` | clean | **clean** |
| `npm run tauri build` | not attempted by the previous pass | **`.app` + `.dmg`, zero warnings** |
| `npm run version:check` | consistent | **consistent** |
| `node scripts/qa-inventory.mjs` | 0 dead controls | **0 dead controls, 0 unreachable commands, 0 unnamed controls** |
| detection scorecard | 100 % / 0 wrong | **100 % / 0 wrong / 0 paraphrases auto-fired** |

The full frontend suite was run **four times** end to end after the last change; all four were
clean, including the assertion that had been intermittently failing.

**Adjacent functionality specifically re-tested**, because none of these fixes is as local as it
looks: the fire path (`e2e.rs`, 38), rehearsal containment, the panic controls, the service lock
(`delete_service` joins sixteen other held-back actions), the migration ladder
(`every_ensure_rung_survives_being_run_over_and_over` now covers eleven rungs), the IPC contract
in both directions, and the media import/serve pair from the webview to the LAN.

---

## 8. The packaged build — what was verified against a real binary

The previous pass could not do this and said so. This one did.

```
npm run tauri build      → Relay.app + Relay_0.1.0-4_aarch64.dmg, 0 warnings
./scripts/sign-local.sh  → flags=0x10002(adhoc,runtime)
                           hardened runtime: ON  — §17 conditions reproduced
                           mic entitlement : present
                           usage string    : present
```

**Rule 17 — "the microphone dies on the FIRST correctly-signed build" — is now reproduced and
passing on this machine**, without a certificate. That is the trap that is invisible under
`tauri dev` and under an unsigned pre-release, and becomes visible only on the one build handed
to a church.

The bundle was then launched against an isolated database and probed from outside:

| Check | Result |
|---|---|
| Boot heartbeat | **exactly one** `console: webview up (operator)` — rule 26 holds on a packaged build |
| Servers | `:8031` kiosk WS and `:8032` HTTP both up; STT model loaded; no errors on stderr |
| `GET /output.html` | `200`, **the kiosk CSP applied** (tighter than the desktop one — no `http:` in `img-src`/`media-src`), `X-Content-Type-Options: nosniff`, `Cache-Control: no-cache` |
| `GET /api/black` | **`405 Method Not Allowed`**, `Allow: POST`, **no** `Access-Control-Allow-Origin`, `nosniff`. The DECISIONS §35 drive-by defence works in production, not just in a test |
| `GET /api/live` | `200`, CORS wildcard present (read-only route), `nosniff`, `Cache-Control: no-store` |
| `GET /../../../../etc/passwd` | **`404`** |
| `GET /media/999999` | `404` |

**What this does not prove.** It was not installed from the `.dmg`, not opened through
Gatekeeper (an ad-hoc signature cannot be), not run on Windows, and no verse was put on a
physical screen. Those need a certificate, a second machine, and a projector.

---

## 9. Security audit

**PASS, within a threat model that is written down and deliberately narrow.**
[SECURITY.md](SECURITY.md) carries T1–T10, and two of its rows are honest absences rather than
mitigations.

| | Finding |
|---|---|
| Secrets in the repository | **None.** `.env` is gitignored and ignored-status confirmed; the only key material referenced is the updater signing key, which lives at `~/.relay/updater.key` and whose **public** half is committed on purpose |
| Production dependencies | **0 vulnerabilities** (`npm audit --omit=dev`). **25** direct Rust dependencies (21 cross-platform + 4 macOS-only), 4 npm runtime dependencies. Every one of them has a stated reason at its line in `Cargo.toml` |
| Dev dependencies | **10 vulnerabilities, all dev-only** — and two of them are reachable while `npm run tauri dev` runs, because that dev server is deliberately LAN-bound. Every fix is a semver major. **RG-98, left open with the reasoning at the line** |
| Path traversal | Rejected before touching disk in the dev-only branch; the embedded bundle is traversal-safe by construction. Verified `404` against the packaged binary |
| Media by id | The request is reduced to leading digits before it reaches the filesystem, so `../` cannot escape the media directory |
| Injection into the wall | The kiosk beat is parsed against a **closed enum**, so a hostile LAN client cannot put free text into the operator's status pane |
| CSRF-shaped drive-by | Closed. Mutating routes require `POST` and never send the CORS wildcard, so `<img src=".../api/black">` on any page anyone on the church network happens to open can no longer black out the congregation |
| CSP | Applied to **both** audiences — the packaged webview and the LAN pages — and the LAN policy is deliberately *tighter*. The desktop policy's `http:`/`ws:` grants are wider than the LAN they mean: **RG-85, open, and see below** |
| Rate limiting | **Absent.** RG-97, P3: not a new exposure, since anyone on the church wifi can already drive the wall by decision, but an accidental loop can saturate the laptop running the sermon |
| Authentication / authorisation | **None, by recorded decision** (DECISIONS §35). There is one operator on one machine. There is no multi-tenancy to isolate, no privilege ladder to escalate, and no session to steal |
| Telemetry | Opt-in, off by default, no DSN in OSS builds, content-scrubbed, and the scrubber learned its lesson as an allow-list rather than a blocklist |
| Logging | `service_events.detail` is a phrase Relay composes; `perf_samples` stores percentiles. Pinned from both sides so a future column cannot quietly widen it to carry what a preacher said |

**On RG-85, and why this pass declined it.** The desktop CSP grants `http:` and `ws:` to any
host where what it means is *the LAN*. The reason is real — the kiosk hub and the media server
cannot be TLS on a LAN appliance, and the LAN address is not knowable at build time. Narrowing
it is testable **only** against a packaged build driven through real kiosk media playback, on a
real second machine, because `tauri dev` does not exercise the CSP at all. This session could
build and probe a package; it could not put a video on a kiosk screen. **A CSP narrowing that
cannot be verified is a silent blank screen in a church**, so it stays open and recorded.

---

## 10. Data integrity and the database

21 tables, foreign keys **ON**, migrations as ordered retryable rungs, `docs/data/schema.sql`
compiled into the binary as the shipped baseline.

| | |
|---|---|
| Orphan records | **One class existed and is closed** (F-02): a media row surviving a failed write. It was the only place in the schema that could produce one |
| Missing indexes | **Three existed and are closed** (F-10), on the foreign keys every history query walks |
| Transactions | Multi-table deletes (`delete_media`, `delete_service`) are single transactions. `delete_service` was written that way from the start: a half-erased service is worse than either end |
| Retryability | Every `ensure_*` rung is `CREATE … IF NOT EXISTS` or sniffed before `ALTER`, and the whole ladder is run repeatedly by a test against both a fresh and a v0 database — rule 25 exists because a migration that could half-apply bricked every subsequent boot |
| Constraints | `detections.status` and `.method` carry `CHECK`s; the `'manual'` value has its own retryable rebuild migration, which is the rung rule 25 was written about |
| Race conditions | One writer process, one connection behind a `Mutex`, consistent global lock order (`Db` before `Session`). Concurrency is between threads, not between clients |
| Retention | **There is none, and that is now a choice rather than an absence.** Nothing is ever pruned. A church running weekly accumulates transcripts indefinitely — and, since F-03, can erase any service deliberately. An automatic retention policy is not proposed: silently deleting a church's history would be a worse default than keeping it |

---

## 11. Offline, network and recovery

**Offline is not a mode; it is the resting state.** Every core feature — capture, transcription,
detection, routing, rendering, history — runs with the network unplugged, because none of them
reaches for it. The only network calls in the whole product are the one-time model download and
the update check, and both are explicit.

| Brief phase | Relay |
|---|---|
| Offline-capable | STT, detection, routing, rendering, history, plans, templates, media — everything |
| Offline-read | All of it. The database is local |
| Offline-write | All of it. There is no server to queue for |
| Network-required | The model download and the update check. Nothing else |
| Never-cache | Not applicable — there is no cache and no service worker. Nothing is stored *because it might be needed*; it is stored because it is the data |
| Sync engine / conflict resolution / idempotency keys | **N/A.** There is no second writer anywhere in the system. §17 says why inventing one would be a liability, not a feature |

**Recovery** is `RecoverSession.svelte`, and its most important property is what it refuses to
restore: **position only, never on-air-ness.** `liveOnAir` is a separate fact precisely so a
crash cannot put a verse back on a wall unattended, and a test holds that separation.

**Degradation** is not silent. Every fallback produces a row saying what it costs the service
and what to do: denoise off below 48 kHz, audio-only with no model, a CPU-only build, a screen
that stopped answering, sheds counted and shown.

---

## 12. Performance and long-service behaviour

Quoted from [qa/audits/PERF-2026-08-24.md](qa/audits/PERF-2026-08-24.md) and
[PERF-MODELS-2026-08-30.md](qa/audits/PERF-MODELS-2026-08-30.md); **not re-measured this
session**, and nothing this pass changed touches the decode path.

| Target | Measured | Verdict |
|---|---|---|
| First visible partial ≤ 300 ms | **139 ms** median (`ggml-base`) | PASS |
| Perceived transcript lag ≤ 1 s | **P95 339 ms** | PASS |
| Dropped partials < 1 % | **0 of 1075** passes | PASS |
| Real-time factor < 0.7 | ~0.72 duty, and the cadence *is* the decoder's own speed by construction | MARGINAL, by design |
| **Mic → screen ≤ 2 s p95** | **NOT MEASURED** — needs a stopwatch in a room | UNVERIFIED |
| Model choice | `base` 59 ms · `small` 152 ms · `large-v3-turbo` **597 ms** ≈ 1.25 updates/s | A church that chooses `turbo` chooses a quarter of the cadence |

**Long-service behaviour.** The frontend transcript is bounded by construction (`MAX_FINALS` 12,
`MAX_DETECTIONS` 6), so the UI cannot grow with the sermon — there is no virtualisation problem
because there is no unbounded list. `perf_samples` snapshots percentiles once a minute rather
than accumulating traces. **The two genuine unbounded-growth risks were the audio queues, and
they are closed (F-06).** What remains unmeasured is a real multi-hour run on a church laptop:
`docs/qa/audits/FIELD-2026-08-30.md` covers 49.5 minutes with no drift, and that is the longest
evidence that exists.

---

## 13. Accessibility, responsiveness and UX

| | |
|---|---|
| Controls without an accessible name | **0 of 463** (`qa-inventory.mjs`) |
| Buttons with no handler | **0** |
| Modal surfaces trapping focus | **10**, all with `aria-modal` and a labelled dialog role |
| Native `confirm()` / `alert()` / `prompt()` | **0** — pinned, and for a hard reason: Tauri's webview returns `false` without showing a dialog, so a two-step delete guarded by one deletes nothing and reports success |
| `Esc` while a dialog is open | Suppressed, so dismissing an overlay cannot wipe the wall as a side effect |
| Status announcements | `aria-live` in the shell; `ErrorState` is assertive so a screen reader hears a failed read |
| Colour semantics | Enforced by convention and reviewed here: **amber means ON AIR and is spent on nothing else**; cyan is a guess, amethyst is rehearsal, rose is failure. Every new surface in this pass obeys it |
| Contrast, size at distance | `legibility.js` computes both, and is explicit about the two questions it deliberately cannot answer (RG-18) |
| Responsive | **Not applicable in the PWA sense.** Relay's console is a desktop window with a 960×640 minimum; its *served* pages are the output and stage pages, which scale by `cqw` so a template renders identically at any output size. There is no phone breakpoint for the console and there should not be one — see §17 |
| The one UX conflation left | The error third of Loading/Empty/Error exists on 3 of about 15 list surfaces. **RG-95, open**, and deliberately not swept: twelve files of mechanical edits is exactly where a real regression hides |

---

## 14. Privacy, retention and observability

| | |
|---|---|
| What leaves the device | Nothing, unless the operator turns on crash reporting (off by default, scrubbed) or presses Export |
| Audio | Never written, except behind `RELAY_RECORD_WAV`, which no UI sets |
| Transcripts | Local SQLite. **Erasable per service since 2026-09-03** (F-03) |
| The diagnostic bundle | Composed as an **allow-list**, home directory scrubbed — the one artefact meant to leave the building |
| Privacy centre | Settings → Privacy, read from live settings, stating the LAN exposure in the same size type as the reassuring half |
| Retention policy | **None, and named as a choice** (§10) |
| Observability | Nine named latency stamps per decode pass; `service_events` + `perf_samples` survive a quit; the Sunday report names what it does **not** measure; every `null` renders as "—", never as 0 |
| New this pass | `dropped_audio` (audio Relay never heard) and `updateChannel` (whether the update channel answers at all) — two things that were previously unobservable |

---

## 15. The scorecards

Three, because the briefs ask for three and because a single average hides exactly the thing
that matters.

### 15.1 PWA master audit — 81/100

Scored on the brief's own ten axes. **Where a phase is structurally N/A for a desktop
application, the axis is scored on the equivalent Relay actually has**, and the row says which.

| Axis | Score | Why |
|---|---|---|
| Functionality | **9**/10 | Every rendered control reaches a real command; 0 dead controls; the fire path is covered end to end. −1 for the import path that could kill the process until this pass |
| Security | **8**/10 | Clean production dependencies, a written threat model, the drive-by closed and verified in production. −2 for an unauthenticated control plane (a recorded decision, but still an exposure) and RG-85/RG-97 open |
| PWA | **6**/10 | Scored as *distribution and installability*: a signed installer, an updater, an offline bundle on a USB stick. −4 because the updater endpoint 404s and neither platform is code-signed |
| Offline | **10**/10 | Not approximated — it is the resting state. Nothing on the live path touches a network |
| Performance | **8**/10 | 139 ms median first partial, 0 dropped partials, bounded memory. −2 because mic→screen p95 has never been measured in a room |
| Accessibility | **8**/10 | 0 unnamed controls, focus traps, assertive errors, no native dialogs. −2 for no screen-reader run and no keyboard-only pass by a person |
| Responsiveness | **8**/10 | Scored as *output scaling*: `cqw` templates render identically at any output size, with a measured fit floor. Console breakpoints are N/A |
| SEO | **N/A** | There is no public surface, no crawler, and nothing indexable. Scored out of the total rather than as a zero — see the note below |
| UX | **8**/10 | Mode-aware transport, panic controls that cannot lie, honest degradation. −2 for RG-95 |
| Reliability | **8**/10 | Service lock, heartbeats, pre-air validation, crash recovery that refuses to restore on-air-ness. −2 because one wrong verse reached a real congregation and one service is one sample |

**73 of a possible 90 → 81/100 normalised.** SEO is excluded from the denominator rather than
scored 0/10: a private local application has no indexable surface, and awarding it zero would
report a failure where there is no requirement. That is stated rather than buried.

### 15.2 Relay production score — 80/100

| Area | Score | Note |
|---|---|---|
| Audio | **8**/10 | Levels learned not assumed; device fallback; debug recorder. −2: queues were unbounded until this pass, and a real room is still the only proof |
| Transcription | **6**/10 | Works, locally, offline, in four languages. **WER unmeasured in all of them** |
| Transcription latency | **9**/10 | Measured properly, on one clock, at the speed of speech |
| Offline operation | **10**/10 | |
| Synchronisation | **N/A → 10** | There is no second writer. Scored full because the *absence* is correct, not because it was built |
| Scripture detection | **9**/10 | 100 % recall, 0 wrong verses on 74 labelled cases, through the real router |
| Scripture validation | **8**/10 | Only `Direct` may auto-fire, enforced before thresholds. −2 for RG-32: a context-resolved bare verse still carries a `Direct` label it did not earn |
| Presentation routing | **9**/10 | One choke point, one validator, one renderer |
| OBS | **8**/10 | Channel-keyed URL, live template swap, transparent output for keying |
| ATEM | **5**/10 | Probed and reported, never driven. Bridging hardware is the recorded strategy |
| ProPresenter | **6**/10 | Import works. There is no live interop and none is claimed |
| Recovery | **9**/10 | Position restored, on-air-ness deliberately not |
| Live-service UX | **8**/10 | |
| PWA / distribution | **6**/10 | The 404 endpoint and the absent certificates |
| Performance | **8**/10 | |
| Accessibility | **8**/10 | |
| Security | **8**/10 | |
| Data integrity | **9**/10 | Orphans and missing indexes both closed this pass |
| Observability | **9**/10 | Two new instruments; the report names what it cannot see |
| Long-service stability | **6**/10 | 49.5 minutes of real evidence. Two hours is untested |

**Total: 159/200 → 80/100.**

### 15.3 Live-service reliability — 63/80

The score the brief insists must not be hidden by strong scores in cosmetic areas.

| | Score |
|---|---|
| Audio reliability | **8**/10 |
| Transcription reliability | **6**/10 — WER unmeasured |
| Scripture reliability | **8**/10 — one wrong verse in one real service |
| Presentation reliability | **9**/10 |
| Integration reliability | **6**/10 — OBS/kiosk real, ATEM and ProPresenter not driven |
| Offline reliability | **10**/10 |
| Recovery reliability | **9**/10 |
| Session reliability | **7**/10 — Service Lock, timeline and replay all exist; nobody but the author has run one |

**LIVE-SERVICE RELIABILITY: 63/80.**

---

## 16. Remaining risks — what could not be verified

Stated plainly, because a risk that is not named is a risk that is being hidden.

1. **Word error rate, in every language.** Zero measurements. This is the moat and it is still
   an assertion. `LANGUAGES.md` says so and this audit does not soften it.
2. **A projector.** RG-18's contrast and distance thresholds have never been checked against a
   wall.
3. **A Yorùbá, Swahili or Hausa speaker.** The alias table is unreviewed, the numerals are
   unparsed, and three of the four operator locales are deliberately empty stubs that say so in
   their own `_readme`. A wrong numeral does not fail safely; it shows a different verse.
4. **Two code-signing certificates.** Neither platform has one. Every release so far is unsigned
   on both. `sign-local.sh` reproduces the *conditions* ad hoc and passes; it cannot reproduce
   Gatekeeper.
5. **The updater endpoint.** Still 404. Now instrumented from two directions, but instrumenting
   a dead channel does not make it live.
6. **Windows.** Not built or run in this session. CI covers compile, format, lint and tests;
   nothing here covers behaviour.
7. **A second and third service, and one run by somebody who did not write Relay.** The largest
   unknown in the project.
8. **A two-hour run.** 49.5 minutes is the longest evidence that exists.
9. **The media streaming path (F-05)** is verified by a 404 and by reading the code, not by a
   test that serves a large file to three clients.
10. **RG-85's CSP narrowing** cannot be verified without a kiosk screen, and an unverified CSP
    change is a blank screen in a church.
11a. **The corpus repair now runs on real installs, and no real install has run it.** RG-102 …
    RG-105 were found and fixed by reading and by tests on 2026-09-05; what none of that reaches
    is a church laptop with a v2 database, a year of detections in it, and the boot that repairs
    the Bible underneath them. The check that settles it takes one line per machine:
    `sqlite3 "$HOME/Library/Application Support/com.relay.app/relay.db" "PRAGMA user_version; SELECT COUNT(*) FROM verses;"`
    before the upgrade, and the same again after.

11. **The bounded capture queue (F-06) has not been exercised by a real microphone.** The change
    is two lines inside cpal's real-time callback, and cpal's callback only runs with a real
    device open. The existing audio harness (`audio::gate`, `chunks_as_captured`) deliberately
    feeds the chunker and VAD *without* cpal, so it cannot reach this line, and opening the
    default input device from an unattended test would raise a macOS TCC prompt — which is not
    something an audit gets to do to somebody's machine. What **is** established: it compiles
    under `-D warnings`, the type change is total (an unbounded `Sender` no longer exists on
    that path), `Full` and `Disconnected` are distinguished so a Stop cannot inflate the
    counter, and the packaged binary still boots and loads its model. What is **not**: that
    audio flows through it from a physical microphone. **The first person to press Start
    Listening on this branch should check Settings → Diagnostics afterwards and confirm *audio
    dropped (never heard)* reads 0.**

---

## 17. Brief disposition — every phase, both briefs

### 17.1 The PWA master audit, phases 01–42

| Phase | Disposition |
|---|---|
| 01 Project discovery | **DONE** — §3. Framework, runtime, build, data, integrations, environment all mapped |
| 02 Route & page audit | **DONE as surfaces** — §4. There is no router, no URL and no deep link; the equivalents are eight tabs and three served pages, each with its auth, offline and error posture recorded |
| 03 Functional QA | **DONE** — 952 frontend and 644 Rust tests, 38 of them driving the real fire path against a real database. Duplicate submission is guarded by busy flags on every async control; destructive actions are two-step |
| 04 PWA installability | **N/A — there is no manifest and there should not be one.** Relay installs as a signed `.dmg`/`.msi`. Scored as distribution in §15.1 |
| 05 Service worker audit | **N/A — there is no service worker.** Nothing intercepts fetches; the LAN server serves from an embedded bundle with `Cache-Control: no-cache` |
| 06 Offline-first audit | **DONE** — §11. Offline is the resting state, not a fallback |
| 07 Offline data & sync | **N/A by design** — there is no second writer anywhere in the system. Building a queue and a conflict resolver would add failure modes to a system that has none |
| 08 App update strategy | **PARTIAL — blocker #5.** The strategy is right (never during a service, operator-confirmed, data snapshot before install, a way back) and the endpoint is dead |
| 09 Responsive design | **N/A for the console** (a desktop window, 960×640 minimum) / **DONE for outputs** (`cqw`, identical at any size, with a measured fit floor) |
| 10 Touch & mobile UX | **PARTIAL** — the stage page is the phone surface and is built for it; the console is not and is not claimed to be |
| 11 Accessibility | **DONE** — §13 |
| 12 Authentication | **N/A — there are no accounts.** One operator, one machine, no session to expire |
| 13 Authorisation | **N/A — there are no roles.** No horizontal or vertical escalation is possible where there is one privilege level. The LAN control plane's openness is a recorded decision, audited in §9 |
| 14 Database & data integrity | **DONE, two defects found and fixed** — §10, F-02, F-10 |
| 15 Security | **DONE** — §9 |
| 16 API security & reliability | **DONE** — the seven `/api/*` routes, method-gated, CORS-gated, timeout-bounded, verified in production (§8). Rate limiting absent: RG-97 |
| 17 Performance | **DONE from prior measurement** — §12 |
| 18 Network resilience | **DONE** — §11. Nothing on the live path waits on a network, so there is no infinite spinner to have |
| 19 Forms | **DONE** — the forms are Settings, Planner, Templates and the importers; validation is server-side in the sense that matters (the Rust command validates, not the webview), and F-01 added the missing one |
| 20 Error handling | **DONE, one defect fixed** — one humaniser, typed errors across the bridge, and F-08 closed the last Loading/Empty/Error conflation on a screen that mattered. RG-95 records the rest |
| 21 Notifications | **N/A** — no push, no permission request, no subscription. In-app status only, which is correct for a live tool |
| 22 SEO | **N/A** — no public surface |
| 23 PWA/SEO separation | **N/A** — the whole application is private by construction |
| 24 Accessibility + PWA | **DONE** — §13; the install experience is an installer, the offline state is the normal state |
| 25 File uploads | **DONE, defect found and fixed** — F-01/F-02. Type is checked by extension against the importer's list and the MIME table is pinned to it; size is now capped; there is no upload *server*, so malware scanning and signed URLs are N/A |
| 26 E-mail & external services | **N/A** — Relay integrates with no external service on the live path |
| 27 Analytics & monitoring | **DONE** — §14. Opt-in, off by default, scrubbed |
| 28 Privacy | **DONE, a gap found and closed** — §14, F-03. Deletion existed only as "delete the folder" |
| 29 UI consistency | **DONE** — one design system, one renderer, one error humaniser, one colour semantics rule |
| 30 Content audit | **DONE** — no lorem ipsum, no fake testimonials, no fabricated statistics, no dummy contact details. **Zero `TODO` and zero `FIXME` in `src/` or `src-tauri/src/`** — deferred work lives in `KNOWN_ISSUES.md` and the `RG-` register, where it has an owner, rather than in a marker nobody greps. British English in operator-facing text, checked against the usual tells (*behavior*, *optimize*, *canceled*, *catalog*) in `locales/en.json` and the Svelte tree: no hits |
| 31 Navigation | **DONE** — eight tabs, no dead links; the Markdown links in the documentation are now checked by a test (RG-87) |
| 32 UX friction | **DONE** — the Live/Planner merge exists precisely because an operator running a plan on a separate tab could not see the AI's suggestions |
| 33 Data-loss prevention | **DONE** — crash recovery, plan drafts in the database rather than in memory, snapshot before update, and the debug recorder's *press Stop, never force-quit* warning is in `CLAUDE.md` and the user guide |
| 34 Destructive actions | **DONE** — every `delete_*` is two-step, service-locked, and consequence-labelled. F-03 added the newest one under the same rules |
| 35 Build & deployment | **DONE, and further than the previous pass** — §8. Dev build, production build, bundle, ad-hoc hardened signing, launch, and a probe of the running binary |
| 36 Production configuration | **DONE** — there is no production environment to misconfigure. The one runtime configuration that matters is the updater endpoint, and it is wrong (blocker #5) |
| 37 Regression test | **DONE** — §7 |
| 38 Code quality | **DONE** — 0 clippy warnings at `-D warnings`, 0 dead commands, 0 dead controls, 1 deliberate test-only orphan component. One dead accessor introduced by this pass was removed rather than left |
| 39 Priority system | **APPLIED** — §6 is ordered P1 first |
| 40 Do not overengineer | **APPLIED** — one dev-only dependency feature was added (`tokio/test-util`, so a five-second timeout can be tested without a five-second test). No new runtime dependency. Three proposed sweeps were **declined** and recorded rather than performed |
| 41 Final production score | **DONE** — §15, three scorecards, and the sums are checked by `v1audit.test.js` because the first draft got two of them wrong |
| 42 Final report | **DONE** — this document |

### 17.2 The Relay live-service audit, sections 00–105

Grouped, because a hundred rows of "EXISTS" is not a report. **Every section from §00 to §105 is
accounted for by exactly one row, with no gap and no overlap** — which is a claim about
arithmetic, so `v1audit.test.js` checks it rather than asking you to.

| Sections | Disposition |
|---|---|
| **00–05** Command, principle, discovery, routes, roles, auth | **DONE.** §2, §3, §4. Roles and auth are N/A with the reason recorded, not skipped |
| **06–07** Session model, state integrity | **EXISTS.** A service is a row with a real lifecycle, a Service Lock, a timeline and a replay — not an `isLive` boolean. Multi-client state agreement is N/A: there is one client |
| **08–09** Audio input, device recovery | **EXISTS, and improved.** Device fallback when the preferred config will not open, `audio://error` for everything else, levels learned never assumed. F-06 closed the memory risk. **Device hot-swap mid-service is UNVERIFIED** — it needs a hand on a cable |
| **10–14** Transcription: accuracy, latency, state model, continuity, offline | **PARTIAL, and it is the moat.** Latency is measured properly and is good (§12). PARTIAL/FINAL exist end to end and render differently. **Accuracy has never been measured in any language.** Offline transcription is real and complete |
| **15–19** Network state, offline architecture, queue, sync, idempotency | **N/A by design**, §11/§17.1-07. The one idempotency question that *does* arise — a double-press on a fire — is answered by the router's per-reference debounce and by the transport's mode awareness |
| **20–24** Scripture detection, parsing, confidence, database, routing | **EXISTS, strongest area.** Recognition, interpretation, validation and routing are four separate stages with a gate between the last two. 100 % recall and 0 wrong verses on the labelled gate. RG-32 is the one honest hole: a context-resolved bare verse wears a `Direct` label it did not earn |
| **25–29** Presentation output, command state, OBS, ATEM, ProPresenter | **PARTIAL and honest about it.** OBS and kiosk are real and heartbeat-verified. ATEM and ProPresenter are **not driven** — bridging hardware is the recorded strategy (SDI is a permanent non-goal), and `open_ndi_output` returns a clear error rather than pretending |
| **30–34** Health centre, dashboard, operator control, manual override, automation safety | **EXISTS.** **23** launch probes on a four-level severity ladder, a one-sentence readiness verdict, manual fire always present, detection disarmable, safe mode. Automation cannot act above `Suggest` unless Relay *heard* the reference |
| **35–37** Transcript editing, session history, audit log | **EXISTS / N/A.** There is no transcript editing and none is proposed — editing a record of what was said is a different product. History and the event timeline are real and survive a quit |
| **38–40** Database integrity, concurrency, data loss | **DONE, two defects fixed** — §10 |
| **41–44** Manifest, service worker, cache strategy, update safety | **N/A / N/A / N/A / PARTIAL** — §17.1-04/05/08 |
| **45–48** Sleep-resume, network failure, recovery engine, recovery confirmation | **EXISTS for the states that can occur.** Wake-lock while a screen or a mic is live; the degraded-state line in the shell; recovery restores position and refuses to restore on-air-ness. **Sleep/resume mid-service is UNVERIFIED** |
| **49–51** Error handling, notifications, alerts | **DONE, one defect fixed** (F-08). No sound alerts, deliberately: an alarm during a sermon is a worse outcome than the fault it announces |
| **52–53** Mobile, responsive live interface | **PARTIAL.** The stage page is the mobile surface and works. The console is desktop-only and is not claimed otherwise |
| **54–57** Accessibility, performance, long service, large transcript | **DONE / DONE / PARTIAL / DONE.** The transcript UI is bounded by construction, so there is no virtualisation problem. 49.5 minutes is the longest real run |
| **58–64** Security, multi-tenant, credentials, rate limiting, media, API, webhooks | **DONE** — §9. Multi-tenancy and webhooks are N/A; rate limiting is RG-97 |
| **65–67** Observability, health checks, logging | **EXISTS, two instruments added** — §14 |
| **68–72** Privacy, retention, backup, destructive actions, search | **DONE, a real gap closed** (F-03). Backup is the update snapshot plus Export; a church-wide backup strategy is the church's, and Relay says where the folder is |
| **73–75** Settings, configuration validation, pre-service check | **EXISTS.** The path check — say one verse and watch six stages — is the strongest thing in this group |
| **76–78** Live mode, emergency mode, presentation safety | **EXISTS.** The console *is* live mode; manual operation always remains possible; the panic controls do not pass through the validator, because a validator that could refuse a blackout is a blackout that can fail |
| **79–81** False-positive dataset, transcription dataset, integration matrix | **PARTIAL.** The false-positive dataset exists and is a CI gate (74 cases, four languages). **The transcription dataset does not exist** — that is the WER hole. The integration matrix exists for OBS/kiosk and is marked BLOCKED for ATEM/ProPresenter rather than passed |
| **82** Twelve critical journeys | **10 of 12 covered by tests.** Journeys 7–8 (network failure and recovery) are structurally trivial here — nothing on the live path uses a network. Journeys 11–12 (device restart, multi-hour run) are **UNVERIFIED** |
| **83–86** P0/P1/P2/P3 blocker lists | **APPLIED** — §1 and §6 |
| **87–99** Regression, production build, console, storage, memory, CPU, installation, deep links, back/forward, crash recovery, security regression, performance regression | **DONE where reachable.** §7 and §8 cover regression, the production build and a clean console on a packaged launch. **Browser storage was audited and is small and deliberate**: `localStorage` holds one key (`relay.session.v1`) carrying the active tab, the open plan, the cue and slide position and the first-run flag — **ids and positions only, content-free by design**, so a reload mid-service does not drop the operator back on the Console tab with no idea where they were; plus the crash breadcrumb that reads the same key, the boot flags, and the operator's audio-output choice (which lives there because the fullscreen output window shares an origin with the console and that is how the choice reaches it). No `sessionStorage`, no IndexedDB, no cookies, no cache storage. Transcript and verse text are never in any of it — they stay in SQLite, per the local-first rule. Deep links and back/forward are N/A. **CPU/battery/thermal over hours is UNVERIFIED** |
| **100–102** The three scores and the final report | **§15 and this document** |
| **103–105** The release gate, the golden test, the engineering rule | **The golden test has been run once, in a real service, and it is `FIELD-2026-08-30.md`.** Relay knew what was happening, the operator knew, the operator could continue, and one wrong verse still reached a congregation. That is the whole basis of the supervised-pilot decision |

---

## 18. Recommended next steps, in order

**Nothing on this list is a large piece of engineering. That is the point: what stands between
Relay and a church is evidence, a certificate and a published release — not code.**

0. **Before anything else, on this branch: press Start Listening, say a verse, then open
   Settings → Diagnostics and confirm *audio dropped (never heard)* reads 0.** It takes two
   minutes and it closes the one change in this pass that no instrument here could reach
   (the eleventh remaining risk in [§16](#16-remaining-risks--what-could-not-be-verified)).
   Everything below assumes it does.
1. **Record thirty minutes of a real sermon and measure word error rate**, in at least one Tier-1
   language. The ruler exists (`stt::realtime::live_transcript_latency`, `eval.rs`, and the
   debug recorder). This is the single highest-value action available and it needs a microphone
   and a room, not a commit.
2. **Publish a full (non-prerelease) GitHub release**, then run `npm run updater:check` and watch
   it go green. Blocker #5 dies the moment that happens.
3. **Buy the two code-signing certificates** and put the fourteen secrets in the repository.
   Then rebuild and confirm the microphone still works under a *real* hardened-runtime signature
   — `sign-local.sh` says it will, and a real certificate is the only thing that proves it.
4. **Run a second and a third service**, and write every wrong verse into the register verbatim
   from `heard_text`. RG-32 is waiting on exactly this evidence.
5. **Have one service run by somebody who did not write Relay.** The largest unknown in the
   project, and it costs nothing.
6. **Have a native speaker review `book_aliases.json`** and translate the `live.*` keys in one
   locale. A partial translation is a working translation and ships the day it lands.
7. Then the open register rows. **RG-95, RG-96 and RG-97 were closed on 2026-09-04**, and
   RG-102 … RG-107 and RG-109 on 2026-09-05. What is left, in this order: **RG-101** (the two
   `quick-xml` advisories, which need a Tauri bump — and a CI job that runs `cargo audit`, so the
   next one is not found eleven months late), **RG-108** (the kiosk hub's missing `Origin` check)
   and **RG-85** (the CSP narrowing) — the last two both need a real kiosk screen to verify
   against, and neither should be taken on without one — then **RG-98** (decide the dev-server
   default).

---

## 19. The launch decision

> ### NOT READY FOR PRODUCTION (general release)
> ### READY WITH CONDITIONS (supervised pilot — two churches, named operators, every service watched by somebody who can take the wall back by hand)

The conditions are [qa/RELAY_GAP.md](qa/RELAY_GAP.md) §24's five, unchanged, plus the two this
audit adds:

6. **Publish a full release before handing any build to a church**, so that installation is not
   also a commitment to never receiving a fix.
7. **Tell the pilot churches, in writing, that Settings → Updates now reports the update channel
   honestly** — and ask them to look at it, because that row is the only thing in the product
   that can tell them their copy has gone stale.

**The reasoning has not changed and is worth restating.** An indefinite NO-GO is not caution; it
is a way of never being wrong. Relay ran a live sermon on 2026-08-30 for 49.5 minutes with no
drift and five of six auto-fires correct, and one wrong verse reached a congregation. The way to
learn whether that was typical is not another audit. It is a second service, watched.

What this pass changed is that the audit itself is now a smaller part of the answer. Ten real
defects were found and closed — one of which would have killed the process on a volunteer's
Saturday, one of which meant a church could never erase a sermon, and one of which meant the
only surface that could report a dead update channel was reporting *"up to date"*. The
instruments that found them are in the tree, and each one fails if its defect comes back.
