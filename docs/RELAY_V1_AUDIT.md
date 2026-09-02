# Relay — V1 Production Audit

**2026-09-02 · branch `docs/final-consistency-sweep` · version `0.1.0-4`.**

An 85-phase production-readiness brief, answered against the code that exists rather than the
code described. Every number below came from a command that was run in this session, and the
command is printed beside it. Where no instrument here could reach the question, the row says
**UNVERIFIED** and names the instrument that would answer it. Nothing is scored around.

> **This document owns exactly three things**: the production decision (§1), the scorecard
> (§15), and the disposition of all 85 brief phases (§17). Everything else it cites.
> [qa/RELAY_GAP.md](qa/RELAY_GAP.md) remains the `RG-` register and is where findings are filed;
> [qa/QA_HARNESS.md](qa/QA_HARNESS.md) §0 remains the register of counts;
> [qa/audits/](qa/audits/) remains the frozen evidence. A second copy of any of those is how
> four documents came to disagree, which is the condition this sweep was called to end.

---

## 0. Method — and what it could not reach

**What ran.** The frontend suite on this machine (`npx vitest run`), the Rust suite
(`cargo test`), `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
`npm run build`, `npm run version:check`, `node scripts/qa-inventory.mjs`,
`cargo test eval::tests::print_scorecard -- --nocapture`, `npm audit`, `gh secret list`,
`gh release list`, a live `curl` against the updater endpoint, and a symbol-by-symbol read of
the seven live-path modules.

**What did not, and why.** There is no church, no projector, no congregation, no second
operator, and no code-signing certificate on this machine. There is no recording of a real
sermon — so word error rate remains unmeasured in every language, which is the same sentence
[LANGUAGES.md](LANGUAGES.md) has carried since the beginning and this audit does not soften it.
The packaged binary was not built or installed; `npm run tauri build` was out of scope for a
report-only pass, and CI's macOS job is compile-only and says so in its own comment.

**One thing this audit did change.** It found that `cargo clippy --all-targets -- -D warnings`
— a CI gate on both platforms — **failed on this branch**, and fixed it. That is recorded as
**F-01** and it is the only code change in the sweep.

**A note on method that turned out to matter more than any finding.** An exploratory pass
reported eleven documentation contradictions. Re-checking each one by hand disproved **three**
of them, and one of the three would have "corrected" a number that was right. The failure mode
in this repository is not carelessness; it is a plausible one-liner believed over the tool that
knows. Two examples from this session: `grep -c '#[ignore'` over-reports by seven because it
counts prose inside doc comments, and `grep -rl 'role="dialog"'` counts a comment explaining
why a panel is *not* a dialog. Both were caught only by reading the hits.

---

## 1. Executive summary and the production decision

> ## NOT READY for general release · READY WITH CONDITIONS for a supervised pilot

That is not a change of verdict. It is [qa/RELAY_GAP.md](qa/RELAY_GAP.md) §24's decision of
2026-08-31, re-verified, **with one item added to its blocking list**.

**What holds up.** The live path is genuinely well built and the safety architecture is not
decorative. Content reaches a wall through exactly one function
(`channels::broadcast_content`, one caller). Only `detection::DetectionMethod::Direct` can
auto-fire, and that is enforced in `router::decide` *before* any threshold is consulted, so no
number can promote a paraphrase. There are **zero** `unwrap()`/`expect()` calls on the
production path of the seven service modules. The detection gate scores **100% recall, 0 wrong
verses, 0 paraphrases auto-fired** over 74 labelled cases in four languages. The frontend suite
is 927 tests over 68 files and runs in five seconds; the Rust suite is 629. Production
dependencies carry **0 vulnerabilities**.

**What blocks a general release — four things, and only one is a commit.**

| | Blocker | Status |
|---|---|---|
| 1 | **A wrong verse reached a real congregation** on 2026-08-30 | Root-caused and fixed (`detection::anchor_for_bare_verses`), but one service is one sample |
| 2 | **Word error rate has never been measured, in any language** | Needs 30 minutes of real sermon audio. The ruler is built and runs in CI |
| 3 | **Neither platform has a code-signing certificate** | `gh secret list` holds zero of the fourteen. All releases went out unsigned on the pre-release path |
| 4 | **Nobody but the author has ever run a service** | Needs a Sunday |

**And the new one, found by this audit:**

| | Blocker | Status |
|---|---|---|
| 5 | **The auto-updater points at a URL that returns 404** | Verified live. Every installed copy that ever ships is un-updatable until a non-prerelease is published |

That fifth item is the reason this audit exists as a document rather than a paragraph: it is
the kind of failure that is invisible from the source, green in every test, and total in the
field.

---

## 2. Architecture, as discovered

A single native binary. No server, no account, no cloud dependency on the live path.

| Layer | What is actually there |
|---|---|
| Shell | Tauri v2, one webview window; `src-tauri/src/main.rs` registers **132** commands |
| Engine | Rust — capture, DSP, whisper, detection, routing, rendering, all in-process |
| UI | Svelte 4 + Vite, **48** components (47 reachable), **462** controls, one store (`capture.js`) |
| Data | SQLite via `rusqlite`, **21** tables, schema version 2, migrations as ordered rungs |
| Distribution out | A native output window, a WebSocket kiosk hub on `:8031`, an HTTP server on `:8032` |
| Control in | Tauri IPC, and an unauthenticated LAN HTTP control plane (`main::remote_api`) |

**Three threads carry the live path**, and the division is the interesting part:

- `relay-stt` — the decoder. 16 MiB stack (`whisper_full` overflows the default 2 MiB and
  SIGSEGVs silently after the first transcript).
- `relay-detect` — semantic scan, locks, SQLite write, event emit. Behind a **bounded**
  `sync_channel(8)` (`main::DETECT_QUEUE`) that sheds a PARTIAL and blocks on a FINAL, and
  counts what it sheds.
- `relay-history` — snapshots latency percentiles into `perf_samples` every 60 seconds.

**One event bus, 19 named events.** Eighteen have listeners — seventeen in `capture.js` and
`channel://retemplate` in `Output.svelte`, which is the surface that acts on it — and the
nineteenth, `model://done`, is deliberately unheard with its reason recorded, because the
download command already resolves on success and a listener would handle the same fact twice.
`ipc.test.js` enforces the contract in both directions and requires each named exception to
still be both emitted and unheard.

---

## 3. The live path, boundary by boundary

Microphone to wall, with every queue, bound and failure point named.

| # | Stage | Where | Crosses | Bounded? |
|---|---|---|---|---|
| 1 | cpal capture callback | `audio::AudioEngine` | `Vec<f32>` mono | **unbounded** `mpsc` |
| 2 | Denoise + auto-gain | `dsp::FrontEnd` | samples + `AudioQuality` | n/a |
| 3 | Chunker | `audio::Chunker` | 400 ms window / 200 ms hop, 50% overlap | n/a |
| 4 | Voice gate | `audio::Vad` | `bool` | n/a |
| 5 | Chunk to worker | `main::start_capture` | `audio::AudioChunk` | **unbounded** `mpsc` |
| 6 | Decode | `relay-stt`, 16 MiB stack | `stt::TranscriptUpdate` | drains the batch, decodes once |
| 7 | Hand-off to detection | `main::DETECT_QUEUE` | `TranscriptUpdate` | **`sync_channel(8)`**, sheds partials, counts them |
| 8 | Detect | `main::emit_detections` | `Vec<Cand>` | n/a |
| 9 | Route | `router::decide_live` → `router::decide` | `RouteDecision` | debounce per reference |
| 10 | Rank | `main::rank_for_wall` | at most one wall fire per window | n/a |
| 11 | Build content | `pipeline::Fire` | `channels::OutputContent` | n/a |
| 12 | Validate + broadcast | `main::broadcast_with_clock` → `pipeline::preflight` → `channels::broadcast_content` | one door | n/a |
| 13 | Fan out | Tauri emit + `channels::kiosk_content_json` over `:8031` | struct / JSON | n/a |

### What this audit found on that path

**F-02 · The audio side of the pipeline is unbounded, and only the far half is documented.**
Stages 1 and 5 are plain `mpsc::channel` with no capacity and no shed counter. The mitigation is
real — the worker drains the whole batch and decodes it once, so falling behind costs one decode
however deep the queue — but the mitigation bounds *latency*, not *memory*. `CLAUDE.md` rule 33
describes the bounded `relay-detect` queue and its shed counter and is silent about the two
unbounded ones in front of it, which reads as though the whole path is bounded. **PARTIAL** ·
P2. No observed failure; the honest statement is that nothing has run long enough on a machine
slow enough to find out.

**F-03 · "The decoder decodes; nothing else runs on its thread" is very nearly true.** The STT
callback still performs one Tauri `emit("stt://transcript")` on the decoder thread before the
hand-off. It is deliberate and documented in the code. The rule as written in `CLAUDE.md` is
absolute and the code is not; the rule should say so. **PARTIAL** · P3.

**Not a finding, and worth stating because it is the strongest thing here:** `preflight` is
called in exactly one place, `broadcast_content` has exactly one caller, and rehearsal is
gated inside `channels.rs` at all four publishers rather than at any call site. That is the
architecture rule this repository calls *"the choke point is where the check goes, not the call
sites"* (DECISIONS §42) and it is genuinely held, not aspirational. The panic controls
deliberately bypass the validator, because a validator that can refuse a blackout is a blackout
that can fail.

---

## 4. Voice and transcription

### Measured, on this machine, this session

`RELAY_BENCH_MODEL=… cargo test --release decode_cost -- --ignored --nocapture` — one whisper
pass, best of three, by model and window length:

| Model | load | 2 s | 4 s | 8 s | hops of 200 ms | update rate |
|---|---|---|---|---|---|---|
| `ggml-base` | 93 ms | 64 ms | 56 ms | **56 ms** | 1 | ~5/s |
| `ggml-small` | 148 ms | 145 ms | 151 ms | **151 ms** | 1 | ~5/s |
| `ggml-large-v3-turbo` | 793 ms | 586 ms | 599 ms | **594 ms** | 3–4 | **~1.25/s** |

Two things this independently confirms. **The published figures are right** — 
[qa/audits/PERF-MODELS-2026-08-30.md](qa/audits/PERF-MODELS-2026-08-30.md) records 59 / 152 /
597 ms on a different run, and the measurements above land within 5% of each of them. And
**window length is free**: 4 s and 8 s cost the same to within noise on all three models,
because whisper pads the mel window internally. Do not "optimise" `stt::WINDOW_SECS`.

The operational consequence is the one an operator actually chooses between: `small` is a
better model than `base` **at the same transcript cadence**, because both decode inside a single
200 ms hop. `turbo` costs three to four hops and about a quarter of the update rate.

### Transcript states — two, not three, and that is a decision

`stt::TranscriptUpdate` carries `is_final: bool` and nothing else. There is no STABLE state; the
brief's Phase 04 asks for one. It was **declined**, with the reason recorded (DECISIONS §62):
the harm a stable state would prevent — a wavering decoder putting a half-formed reference on a
wall — is already prevented by the corroboration rule in `router::decide_live`, which holds a
reference seen in a PARTIAL window at `Suggest` until a second pass agrees. Adding a third state
would add a second mechanism for one guarantee. **NOT APPLICABLE — declined with reasoning.**

### What is not measured, and cannot be from here

**First-partial latency, useful-partial latency, and end-to-end p95 are UNVERIFIED.** The rig
exists — `stt::realtime::live_transcript_latency` is the only harness that feeds the real worker
at wall-clock pace through the real chunker and voice gate — and it requires `RELAY_BENCH_WAV`,
a recording of real speech. There is none. The brief's Phase 06 targets (≤300 ms first partial,
≤1 s perceived, ≤2 s end-to-end p95) therefore have **no measurement to compare against on this
machine**, only the field service's numbers, which are `turbo` on one laptop.

**Word error rate is unmeasured in every language, including English.** This is the moat, and it
is an assertion. `bench/README.md` states exactly what to record.

---

## 5. Scripture detection

### The gate, verified by reading it

`detection::DetectionMethod` has five variants. `detection::may_auto_fire` returns true for
exactly one:

| Method | May auto-fire | Enforced |
|---|---|---|
| `Direct` | **Yes** — and only when corroborated by a second pass, unless the window is FINAL | `router::decide_live` |
| `Semantic` | No — `Suggest` at any score | `router::decide`, before thresholds |
| `Ambiguous` | No | as above |
| `UncertainBook` | No | as above |
| `UncertainNumber` | No | as above |

The order matters and is the point: `router::decide` tests `may_auto_fire()` **before** it
consults `Thresholds`, so raising a number cannot promote a paraphrase. `detection::from_wire`
defaults an unknown method string to `Semantic`, which is the cautious reading rather than the
permissive one. Two further gates sit behind it — `main::rank_for_wall` allows at most one wall
fire per decode window, and `pipeline::may_broadcast` refuses content with no resolved verse.

### Measured

`cargo test eval::tests::print_scorecard -- --nocapture`:

```
  lang   cases   recall    verses found   wrong verses on screen
  en        36     100%     13/13         0
  ha        14     100%     13/13         0
  sw        12     100%     11/11         0
  yo        12     100%     10/10         0
  TOTAL     74     100%     47/47         0

  wrong-verse rate: 0.0%  (SPEC target: <5%)
  paraphrases auto-fired: 0  (must be 0)
```

**Read that honestly.** It is 74 hand-written cases scored over **text**, through the real
router. It is a regression gate against the parser and the gate; it is not an accuracy
measurement over audio, and it says nothing about what whisper hears in a room. The corpus is
also partly self-referential — some cases were written from the one real service — which is the
right way to build it and also a reason not to read 100% as a product claim.

### Where it has actually been wrong

Once, in front of a congregation, and the write-up is
[qa/audits/FIELD-2026-08-30.md](qa/audits/FIELD-2026-08-30.md). A bare *"verse 32"* was resolved
against the passage last put on screen — correct for the case it was written for — while the
preacher had just named Luke 10 in the same sentence. `detection::anchor_for_bare_verses` now
prefers a reference named in the window over memory. Two things about that fix are worth
carrying: the **first diagnosis was wrong**, and the regression test written against it passed
with the supposed fix reverted; and the confidence label on that path is still a hardcoded
`Direct` at 0.88, which is still a lie — Relay inferred the book, it did not hear it — left
deliberately because one service is not enough evidence to make every in-passage *"verse
eighteen"* ask for a click. **PARTIAL by choice, with the reasoning recorded.**

---

## 6. Output

**One door.** `channels::broadcast_content` has exactly one caller. Six production paths reach
it — the AI, the manual box, spoken navigation, plan cues, media, the emergency announcement and
the countdown — and all six pass through `pipeline::preflight` because the validator lives at
the choke point rather than at the call sites.

**Health is real, and its limits are stated.** `channels::OutputHealth` holds a last-painted
instant per screen; every output page reports that it is still painting — the native window over
the bridge, kiosk/OBS over the socket it already has. `channels::ClientRegistry` is a **count**
per template id and nothing else: no device identity, no address, deliberately (DECISIONS §35,
narrowed by §39 to *when*, anonymously, never *who*).

**Panic controls are exempt from the validator and must stay exempt.** `clear` and `black` call
`channels::clear` / `channels::black` directly. They return `Result`, the frontend wrappers
return a boolean *and* set a global error store, and a failure emits `output://panic_failed` —
because a panic control fires from a global keydown handler and from a shell button that must
survive a crashed view, and neither can `catch`.

**Rehearsal is gated at four publishers**, including `channels::stage_next`, which publishes
only to the kiosk hub and emits no Tauri event — the one that leaked historically, because the
test watching for leaks could only see events.

**NDI is parked, honestly.** `open_ndi_output` is a registered command that returns a clear
error naming the reason. It is not a stub that pretends.

---

## 7. Offline

**Nothing on the live path touches the network.** Every outbound URL in the Rust tree is either
a HuggingFace model download (`models.rs`, an explicit operator action), the Tauri updater
endpoint, or a Sentry DSN that is empty in the open-source build. The KJV, the book aliases, the
numerals and the modern-English gloss are all `include_str!`d into the binary. Verse text,
templates, service history and transcripts are local SQLite.

| Function | Offline |
|---|---|
| Start, capture, transcribe, detect, route, render | ✅ — no network path exists |
| Manual fire, search, panic controls, plan playback | ✅ |
| Kiosk / OBS output, stage display, LAN remote | ✅ — LAN, not internet |
| Service history, replay, the Sunday report, diagnostics export | ✅ |
| Model download, update check, crash reporting | ❌ by design — each is optional and says so |

**UNVERIFIED:** the transitions. Nothing here pulled the cable mid-service and watched. The
degraded-state line (`degraded.js`) exists and is tested at the unit level; whether it *reads*
correctly to an operator during a real drop is a Sunday question.

---

## 8. Security

The threat register is [SECURITY.md](SECURITY.md) T1–T10 and this audit re-checked it rather
than restating it. Three things to add.

**F-04 · The CSP carries three unscoped `http:` allowances.** `img-src`, `media-src` and
`connect-src` each permit plain `http:` to any host, plus `ws:`. The reason is real — the kiosk
hub is `ws://<lan-ip>:8031` and media is served over `http://<lan-ip>:8032`, and neither can be
TLS on a LAN appliance — but the grant is written as "any host" rather than as the LAN. The
tight parts are tight: `script-src 'self'`, `object-src 'none'`, `frame-src 'none'`,
`form-action 'none'`. **PARTIAL** · P2. Note that `tauri dev` does not exercise the CSP at all;
any change must be verified against a packaged build.

**The LAN control plane is unauthenticated, and that is a recorded decision** (DECISIONS §35) —
the preacher driving their own reading from a phone is the feature. What this audit confirms is
that the decision is *implemented* carefully: `main::remote_mutates` names the five mutating
routes, they require POST, and CORS is withheld from them **even on success**, so a cross-origin
page cannot read what it just did to the wall. That closes the drive-by; it is not
authentication and the code says so in those words.

**F-05 · Zero of fourteen code-signing secrets exist.** `gh secret list` returns only
`TAURI_SIGNING_PRIVATE_KEY` and its password — the *updater* key, not a code-signing
certificate. The per-platform release gate is correctly written and will hard-fail a real tag,
but **it has never been exercised**, because only pre-release tags have been pushed and a
pre-release bypasses its `exit 1`. Rule 17's trap — the microphone dies on the first correctly
signed macOS build, because the hardened runtime is only switched on then — is therefore still
ahead, not behind. `scripts/sign-local.sh` reproduces it without a certificate and should be run
before that first signed build. **MISSING** · P1 · blocks general release.

**Dependencies.** `npm audit --omit=dev` → **0 vulnerabilities**. The full audit reports 10
(1 critical, 1 high, 8 moderate) and every one is `vite` / `vitest` / `svelte-hmr` / `esbuild` —
the dev toolchain, none of it shipped. Vite is four majors behind and Svelte one; that is
technical debt, not exposure. No Rust advisory scan was run (`cargo-audit` is not installed) —
**UNVERIFIED**.

**No secrets in the tree.** `.env` is untracked and ignored, `.env.example` is tracked, and the
only DSN-shaped string in the Rust source is a test fixture (`https://k@…`).

---

## 9. Privacy

**Sermon audio is never written to disk unless an operator sets `RELAY_RECORD_WAV`**, and that
variable buffers the whole service in RAM (~570 MB for 50 minutes) and writes once, at Stop.
A force-quit loses it — which is exactly the morning you most wanted it.

**Crash reporting drops free text wholesale rather than filtering it** (`telemetry::scrub`).
That is the right shape: a filter that misses once has leaked a sermon. Breadcrumbs, extras and
stack-frame locals are dropped entirely; the frames remain. There is no built-in DSN in the
open-source build, so the feature is inert unless a DSN is supplied.

**The service record is deliberately narrower than the service.** `service_events.detail` is a
phrase Relay composes, never something a preacher said, and `perf_samples` stores percentiles
rather than traces. Both halves are pinned from both sides, because this is the part of the
history most likely to be emailed to somebody. The one exception is `detections.heard_text`,
which *is* a fragment of speech — and RG-81 makes it reachable on the local history screen and
nowhere else, on purpose.

---

## 10. Recovery, crash safety, panic controls

| Failure | What happens | Verified how |
|---|---|---|
| Console UI crash | Crash boundary; the outputs are separate webviews and stay live; operator position persisted | `crash.js`, DECISIONS §26 |
| Process death mid-service | Session recovered at boot behind an explicit operator confirmation; stale content is never silently re-aired | `boot/` gates |
| Migration failure | Rungs are retryable; the scratch table is dropped first and the transaction rolls back | `db::ensure_service_plans_is_retryable` |
| Bad update | Preflight, `VACUUM INTO` snapshot, verify on next launch, restore **before** the database opens | `updates.rs` (12 tests) |
| STT unavailable | Visible banner; manual fire and plan playback keep working | DECISIONS §22 |
| Screen disappears | Per-channel health goes stale and the badge says so | `channels::OutputHealth` |

**Not binary rollback, and deliberately so** — the installers are public; the church's data is
what cannot be got back.

**UNVERIFIED:** none of this was exercised by actually killing the process, pulling a device, or
corrupting a database on a running app. Every row above is unit- or integration-tested. The
failure-injection column of the brief (Phase 78) is answered by tests, not by a running system.

---

## 11. Accessibility

Ten modal surfaces trap focus and restore it on close (`grep -rl trapFocus src | grep -c
svelte`). The shell and the run surface both carry `aria-live` regions, so the AI's suggestions,
the transport and errors are announced rather than arriving in silence — which is the product's
whole reason to exist and used to be inaudible. `prefers-reduced-motion` is honoured in
`src/app.css`. Panic keys are global and owned by the shell, and `Esc` is suppressed while any
`[role="dialog"]` is mounted, so dismissing a help overlay cannot wipe the wall as a
side-effect.

Critical state does not rely on colour alone — the four promise-carrying colours (amber ON AIR,
amethyst REHEARSAL, cyan a guess, grey CUED) are each paired with a word.

**UNVERIFIED:** contrast was checked against tokens, not against a projector. `legibility.js`
computes exact contrast over a solid background and **refuses to answer over a picture**, which
is the honest behaviour; its distance thresholds are WCAG (a specification for screens at arm's
length) and broadcast practice, and neither has been checked in a room. No screen reader has
been run against the app.

---

## 12. Hardware, packaging, distribution

**F-06 · The updater endpoint returns 404. Verified live.**

```
curl -sI https://github.com/devgeereact/relay/releases/latest/download/latest.json
HTTP/2 404
```

Both `tauri.conf.json` and `tauri.updater.conf.json` point there. GitHub's `/releases/latest/`
**excludes pre-releases**, and `gh release list` shows one published release — a pre-release —
plus four drafts. So the auto-updater, which is built, signed with a real minisign key, and
covered by tests, currently resolves to nothing. **BROKEN** · P1. It costs nothing to fix
(publish a non-prerelease, or pin the endpoint at a tagged manifest) and it cannot be found by
any test in the repository, because the URL is data.

**Version state is clean.** `0.1.0-4` in all three files, `npm run version:check` agrees, and it
matches the newest tag and the only published release.

**CI is strong where it can be.** `fmt` + `clippy -D warnings` + the full Rust suite on macOS
**and** Windows; the frontend suite on Node 20, 22 **and** 24; the detection scorecard; a
Windows MSI built on every push. macOS packaging is compile-only and the workflow says so in its
own comment rather than implying otherwise.

**UNVERIFIED:** no clean-machine install, no observed update, no projector, no second monitor, no
OBS, no microphone other than this laptop's. `scripts/offline-bundle.mjs` exists and verifies the
model against the checksum in `models.rs` — refusing rather than warning on a mismatch — but has
not been run onto a stick and carried to a church.

---

## 13. Test coverage

| Layer | Count | Command |
|---|---|---|
| Rust unit + integration | **629 passing**, 17 ignored (646 declared) | `cd src-tauri && cargo test` |
| Frontend | **927 passing**, 0 skipped, 68 files | `npx vitest run` |
| Command E2E (real commands, real DB, real router, mock window only) | **38**, 0 ignored | `cargo test e2e::` |
| Detection gate (CI-enforced) | **74** cases, 100% recall, 0 wrong verses | `cargo test eval::tests::print_scorecard -- --nocapture` |
| Surface inventory | 48 components · 462 controls · 132/132 commands · 1 intentional orphan | `node scripts/qa-inventory.mjs` |
| Performance | decode cost per model, measured this session | `RELAY_BENCH_MODEL=… cargo test --release decode_cost -- --ignored` |
| Real-time latency | **rig exists, no input** | needs `RELAY_BENCH_WAV` |
| Failure injection | **tests only** | no running system was broken on purpose |
| Field | **one service**, one machine, one model, one operator | `qa/audits/FIELD-2026-08-30.md` |

**The gaps that matter are not in the count.** Everything above the last three rows is static or
in-process. Nothing in this repository has ever driven the packaged binary, rendered a pixel, or
heard a microphone other than in one 49.5-minute service run by the author.

---

## 14. Findings

Six, none of them P0. Each is filed in [qa/RELAY_GAP.md](qa/RELAY_GAP.md) as an `RG-` row; the
`F-` numbers here are for reading this document only.

| F | Finding | Class | P | Fixed? |
|---|---|---|---|---|
| **F-01** | `cargo clippy --all-targets -- -D warnings` failed on this branch — a duplicated `#[test]` attribute, which also inflated the reported test count by one | **BROKEN** | P1 | **Yes**, this sweep |
| **F-06** | The auto-updater endpoint returns 404: `/releases/latest/` excludes pre-releases and only a pre-release is published | **BROKEN** | P1 | No — publishing is not a commit |
| **F-05** | Zero of fourteen code-signing secrets exist; the per-platform gate has never been exercised because pre-release tags bypass it | **MISSING** | P1 | No — needs money |
| **F-02** | The audio→worker channels are unbounded with no shed counter, and the documentation describes only the bounded one behind them | **PARTIAL** | P2 | No — documented here |
| **F-04** | The CSP grants `http:` and `ws:` to any host where it means the LAN | **PARTIAL** | P2 | No — needs a packaged-build verification loop |
| **F-03** | *"The decoder decodes; nothing else runs on its thread"* is absolute in the rule and not in the code — one `emit` remains, deliberately | **PARTIAL** | P3 | No — the rule should say so |

### Documentation findings, fixed in this sweep

| | What | Now |
|---|---|---|
| D-1 | `docs/audits/` was staged for deletion with 19 live citations into it, one from `detection.rs` and one from a CI corpus | Restored, then moved intact to `docs/qa/audits/` |
| D-2 | Eighteen `file.rs:NNN` citations, every one of eight sampled landing on a brace or a blank line | Replaced with `module::symbol`, which the citation test resolves — it caught a wrong one on the first run |
| D-3 | The eval corpus was quoted as 50, 57 and 63 in four places; it is 74 | One owner, `qa/QA_HARNESS.md` §0, beside the command |
| D-4 | `gh pr list --state merged --jq length` answers 30, beside a sentence claiming 45 | `--limit 500`, with the trap named |
| D-5 | *"All five dialogs trap focus"* — it is ten | A command, plus the two ways the obvious command lies |
| D-6 | Four `.github` links would 404 once the policy documents moved under `docs/` | Repointed |
| D-7 | Twenty-eight relative markdown links broke in the move; no instrument in this repository checks them | Repaired by hand — and this is itself a gap, see §16 |

### Three reported contradictions that were not

Recorded because a wrong correction is worse than none. **"28 earlier decision table rows"** is
right — there are three pre-numbering tables, not two, and a pass that counted two would have
replaced a correct number with 18. **The Stage F11 figures** are two windows of one service, not
two answers. **The 2026-08-30 counts in the register** say on the line above that they are dated
and deliberately left.

---

## 15. Scorecard

Sixteen categories, scored against the stated bar — *the first ten churches: a volunteer, in a
dark booth, with no training and no second take* — not against a broadcast facility. **A
category that has never been measured is not scored high because the code looks right.**

| Category | Score | Why that number |
|---|---|---|
| Core functionality | **9** / 10 | The whole path works end to end, offline, and has done so in a real service |
| Live safety | **9** / 10 | One door, one gate, method-before-threshold, panic controls exempt from the validator. Not 10: the bare-verse label is still `Direct` at 0.88 for a book Relay inferred |
| Transcription | **6** / 10 | Decode cost measured and good; cadence adaptive and correct. But there is no partial-latency number, no WER, and the transcript has two states where the brief asks for three |
| Scripture detection | **8** / 10 | 100% recall / 0 wrong verses over 74 cases through the real router — over **text**. One wrong verse has reached a real wall |
| Output reliability | **8** / 10 | Real per-channel heartbeats, honest badges, one broadcast door. Never driven against a projector or OBS |
| Offline operation | **9** / 10 | No network path exists on the live path; everything needed is compiled in. Not 10: the transitions were never exercised |
| Security | **7** / 10 | Careful, recorded decisions and a real threat register; POST-only mutations with CORS withheld. Held down by the unscoped CSP and by an update channel that has never been used |
| Privacy | **9** / 10 | Free text dropped rather than filtered; the timeline provably carries no speech; audio never written unless asked |
| Performance | **7** / 10 | Decode cost measured on three models and reproduced published figures within 5%. The end-to-end p95 the brief asks for has no measurement |
| Recovery | **7** / 10 | Every path has a designed answer and a test. Not one was exercised by actually breaking a running system |
| Accessibility | **8** / 10 | Focus traps with restore, live regions, reduced motion, colour never alone. No screen-reader run; contrast unverified on a projector |
| UX | **8** / 10 | Mode-aware transport, one run surface, panic keys global. Judged from source, not from use by anyone but the author |
| Hardware compatibility | **3** / 10 | Two operating systems build in CI. One laptop, one microphone, one model, one room have ever been tried |
| Distribution | **4** / 10 | The gate is right, the version discipline is right — **and the updater points at a 404 and nothing is signed** |
| Test coverage | **9** / 10 | 629 + 927, contract tests in both directions, a CI-gated detection benchmark, and instruments that catch their own drift |
| Field validation | **2** / 10 | One service. One operator, who wrote it |

**Do not average these.** The two threes and fours are the product; the nines are the
engineering. Relay is a well-built thing that has barely met the world.

---

## 16. Unverified — and the instrument that would answer each

| Question | Instrument | Cost |
|---|---|---|
| Word error rate, any language | `RELAY_BENCH_WAV=… cargo test --release realtime -- --ignored` | 30 minutes of sermon on tape |
| First-partial / end-to-end p95 latency | `stt::realtime::live_transcript_latency` | the same recording |
| Do the African-language aliases work? | A native speaker reading `src-tauri/data/book_aliases.json` | free, and nobody has done it |
| Does a signed macOS build still hear the microphone? | `npm run tauri build && ./scripts/sign-local.sh` | free — **run this before buying a certificate** |
| Does an update actually install? | one observed end-to-end update | 30 minutes, blocked on F-06 |
| Does a clean machine install cleanly? | the offline bundle onto a stick | half a day |
| Does it survive a projector, a second monitor, OBS? | a room | a Sunday |
| Can a volunteer who did not write it run a service? | a volunteer | a Sunday |
| Does latency drift over a *second* and *third* service? | Diagnostics, watched | two more Sundays |
| Rust dependency advisories | `cargo audit` | install it |
| Do relative markdown links resolve? | **no instrument exists** — see below | a test |
| Does a screen reader make sense of the console? | VoiceOver / NVDA | an afternoon |

**The one gap this audit recommends closing with code.** `crossrefs.test.js` checks four kinds
of citation and none of them is a relative markdown link. Twenty-eight broke in this
reorganisation and were found only by a throwaway script. A fifth dimension — resolving every
Markdown link target in every tracked file — would have caught all twenty-eight, and would
catch the next move automatically. It is deliberately **not built here**, because this pass is a
report; it is filed as a register row so the decision is somebody's rather than nobody's.

---

## 17. The 85 phases, dispositioned

**EXISTS** · **PARTIAL** · **BROKEN** · **MISSING** · **N/A** (declined with reasoning) ·
**UNVERIFIED** (no instrument here could reach it).

| # | Phase | Status | Answered in |
|---|---|---|---|
| 01 | Project discovery | EXISTS | §2 |
| 02 | Critical live-path audit | EXISTS | §3 |
| 03 | Voice performance audit | PARTIAL | §4 — decode measured, partial latency not |
| 04 | Partial/stable/final transcript | N/A | §4 — declined, DECISIONS §62 |
| 05 | Fast path + accuracy path | N/A | §4 — one adaptive path; cadence follows measured decode cost |
| 06 | Transcription targets | UNVERIFIED | §4, §16 |
| 07 | Transcription load & stress | UNVERIFIED | §16 — one 49.5-minute service is the only long run |
| 08 | Audio engine | EXISTS | §3 |
| 09 | Audio quality monitor | EXISTS | `audio://quality`, `dsp::FrontEnd` |
| 10 | Adaptive calibration | EXISTS | `audio::Vad` learns its floor; DECISIONS §19 |
| 11 | Detection types | EXISTS | §5 |
| 12 | Direct-detection safety | EXISTS | §5 — method before threshold |
| 13 | Semantic/paraphrase safety | EXISTS | §5 |
| 14 | No fake confidence | PARTIAL | §5 — a paraphrase shows no percentage; the bare-verse path still shows 0.88 |
| 15 | Scripture prefetch | N/A | declined — optimises a 2.6 ms stage inside a 144 ms budget |
| 16 | Scripture data integrity | EXISTS | full KJV compiled in, 66 books |
| 17 | Translation audit | PARTIAL | one translation ships; a second is an open decision |
| 18 | Safety firewall | EXISTS | §3, §6 — `pipeline::preflight` at the one door |
| 19 | Rehearsal vs live | EXISTS | §6 — gated at four publishers |
| 20 | Output system | EXISTS | §6 |
| 21 | Output health | EXISTS | §6 — sent / received / rendered are distinct |
| 22 | Output heartbeats | EXISTS | `channels::OutputHealth` |
| 23 | Device pairing & LAN security | N/A | DECISIONS §35 — unauthenticated by decision, mitigations in §8 |
| 24 | Security audit | PARTIAL | §8 |
| 25 | Update / supply chain | PARTIAL | §8, §12 — signed manifest, unsigned binaries, dead endpoint |
| 26 | Privacy audit | EXISTS | §9 |
| 27 | Offline-first | EXISTS | §7 |
| 28 | Failure degradation | EXISTS | §10 — `degraded.js`, one line in the shell |
| 29 | Recovery & crash safety | PARTIAL | §10 — designed and tested, never exercised live |
| 30 | Panic controls | EXISTS | §6 — and they may never report an unachieved success |
| 31 | Manual fallback | EXISTS | DECISIONS §22 |
| 32 | Template engine | EXISTS | one renderer, `TemplateRender.svelte`, no code execution |
| 33 | Automatic text-fit safety | EXISTS | shrink-with-a-floor; reports below 45% of the designed size |
| 34 | Display readability | PARTIAL | `legibility.js`; thresholds unverified on a projector |
| 35 | Live UI | EXISTS | one run surface, console + plan merged |
| 36 | Transcript UI | PARTIAL | no flicker measurement |
| 37 | Responsive / operator device | UNVERIFIED | never opened at another size by anyone else |
| 38 | Keyboard / operator control | EXISTS | one global keydown owner; `Esc` suppressed inside dialogs |
| 39 | Accessibility | PARTIAL | §11 |
| 40 | Service planner | EXISTS | Planner cannot reach an output, by construction |
| 41 | Library | EXISTS | |
| 42 | Event system | EXISTS | 19 events, contract-tested both ways |
| 43 | Database & data integrity | EXISTS | 21 tables, retryable rungs, FK enforcement |
| 44 | Service history & audit trail | EXISTS | `service_events`, and it carries no speech |
| 45 | Service replay | EXISTS | `report.js` |
| 46 | Performance observability | EXISTS | `latency::report`, nine stamps on one clock |
| 47 | Resource management | UNVERIFIED | §16 — no long soak on a slow machine |
| 48 | Hardware compatibility | UNVERIFIED | §12 |
| 49 | Installation & packaging | UNVERIFIED | §12 — never installed on a clean machine |
| 50 | Update safety | PARTIAL | data snapshot/restore built; the channel is dead (F-06) |
| 51 | Crash recovery | PARTIAL | §10 |
| 52 | Privacy & retention | EXISTS | §9 |
| 53 | Diagnostic export | EXISTS | `diagnostics.rs`, an allow-list with the home directory scrubbed |
| 54 | Sunday readiness check | EXISTS | `boot/` ladder + `views/Dashboard.svelte`; a stub can never render green |
| 55 | End-to-end synthetic test | EXISTS | `e2e.rs`, 38 tests, real commands and a real DB |
| 56 | Real-world field test | PARTIAL | one service |
| 57 | Operator testing | MISSING | nobody but the author |
| 58 | Long-duration service | PARTIAL | 49.5 minutes, once, no drift |
| 59 | Language audit | PARTIAL | aliases ship, unreviewed; Yorùbá numerals unparsed; WER unmeasured |
| 60 | Language pack integrity | N/A | signed language packs deliberately not built |
| 61 | Threat model | EXISTS | `SECURITY.md` T1–T10, two of them honest absences |
| 62 | Error states | EXISTS | one typed error across the bridge, one humaniser |
| 63 | UI state consistency | EXISTS | RG-01 closed this — Live derives from the same backend fact as Outputs |
| 64 | Accessible live status | EXISTS | §11 — colour is never alone |
| 65 | Test-suite audit | EXISTS | §13 |
| 66 | Regression testing | EXISTS | both suites re-run at every stage of this sweep |
| 67 | Code quality | EXISTS | zero `unwrap` on the live path; one duplicate attribute found and fixed |
| 68 | Dependency audit | PARTIAL | §8 — npm done, `cargo audit` not run |
| 69 | Documentation audit | EXISTS | this sweep |
| 70 | Supportability | EXISTS | in-app Help, the path check, drills, the diagnostic bundle |
| 71 | Product friction | UNVERIFIED | needs an operator who is not the author |
| 72 | Observability dashboard | EXISTS | Settings → Diagnostics |
| 73 | Service report | EXISTS | derived, never stored, and it names what it does not measure |
| 74 | Sunday lifecycle | EXISTS | readiness → rehearsal → live → monitor → end → report |
| 75 | Release engineering | PARTIAL | §12 |
| 76 | Production environment | EXISTS | no debug DSN, no test endpoints, no dev URLs in the packaged config |
| 77 | Real-world acceptance test | MISSING | §16 — the whole script, run by somebody else |
| 78 | Failure injection | UNVERIFIED | §10 — tests, not a broken running system |
| 79 | Gap classification | EXISTS | this table and §14 |
| 80 | Priority system | EXISTS | §14 |
| 81 | Do not over-engineer | EXISTS | six things declined with reasons, DECISIONS §62 |
| 82 | Do not break the live path | EXISTS | no engine change in this sweep; one test-module fix |
| 83 | Fix policy | EXISTS | F-01 root-caused, fixed, re-verified |
| 84 | Release score | EXISTS | §15 |
| 85 | Final report | EXISTS | this document |

---

## 18. What this sweep changed

`docs/` was nineteen files in one flat list holding the specification, the audits, the register
and the harness, with nothing in the shape of the directory saying which was which — and four of
them carrying the same verdict, three of the four stale. It is now three groups: `docs/` is the
specification, `docs/qa/` is how Relay is checked, `docs/qa/audits/` is frozen evidence.

Six moves, 154 path citations and 28 relative links swept, the frozen audits verified
byte-identical with `shasum` after every step, and one new mechanism: because a frozen document
cannot be edited to follow a file that moves, `crossrefs.test.js` now carries a `MOVED` redirect
map honoured only for `docs/qa/audits/` and checked at both ends, so it cannot outlive its
reason.

**The single most useful thing found was not in the documentation.** It was that
`cargo clippy --all-targets -- -D warnings` — a CI gate on two platforms — failed on this
branch, and that the same duplicated attribute had been inflating the reported Rust test count
by one. A count register that tells you to trust the command, and a command reporting a phantom
test, is the exact failure this repository has spent months building instruments against. It was
found by running the command.
