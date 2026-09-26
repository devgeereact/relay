# Performance — latency measured, and what each speech model costs

**Two performance audits, merged into one file on 2026-09-21** and otherwise untouched. Both
are **frozen**; nothing is edited except citation paths. The first measures the pipeline, the
second measures the models that run in it, and the second is the one a church's choice of model
is answered from.

**What is in this file, in order:**

- `PERF-2026-08-24.md` — Relay — real-time latency, measured
- `PERF-MODELS-2026-08-30.md` — PERF-MODELS-2026-08-30 — what each speech model costs, and what it costs the operator

---

<!-- ===== was docs/qa/audits/PERF.md, merged 2026-09-21, verbatim ===== -->

# Relay — real-time latency, measured

**2026-08-24.** What was measured, on what, with which instrument, and — at least as
importantly — what was **not**.

> **Nobody spoke into a microphone to produce any number in this document.** Every
> figure comes from text-to-speech audio, fed at wall-clock pace through the real
> chunker, the real voice gate and the real whisper worker, on a development machine
> running a `cargo --release` binary. There is no room, no preacher, no accent, no
> projector and no packaged build in any of it. The acceptance test is **Stage F14**
> of the human test script and it has not been run.

The engineering rationale is `docs/DECISIONS.md` §38. This document is the evidence.

---

## 1. The complaint, and why the last measurement missed it

> "The live transcript appears noticeably later than the words are spoken."

The previous field test measured a **0 ms mean backlog, 15 ms worst case**, and a
**778 ms decode median**, and concluded the machine was keeping up. Both facts were
true. Neither answers the complaint.

A backlog is audio *waiting to be decoded*. Zero means the worker is not falling
**further** behind. A pipeline that is permanently one cadence step, one decode and
one corroboration pass behind the preacher reports a zero backlog for a whole service
while the operator watches text land a second and a half late.

Relay's instruments all measured a **piece**, and none of them ran in real time:

| instrument | what it measures | why it could not see this |
|---|---|---|
| `stt::decode_cost` | one whisper pass | the decoder was never the whole story |
| `stt::bench::decode_latency` | decode vs window and threads | greedy decoding, not the shipped beam search |
| `e2e_latency::walk` | detection over a file, as fast as possible | consumes 30 s of audio in 4 s; never has a queue |
| worker backlog log | queue depth | see above |

So the honest state of the evidence was three green numbers and no way to attribute
the delay to a stage.

---

## 2. The instrument

`src-tauri/src/latency.rs`. Nine named stamps per decode pass, one monotonic clock,
one trace id carried from the microphone to the projector:

```
audio_received → voice_detected → stt_started → partial_transcript
  → transcript_rendered → reference_detected → fire_authorised
  → fire_sent → output_rendered
```

Read it in the app at **Settings → Diagnostics → Live latency**. It is on by default
on a packaged build, because a measurement that needs a developer build is one no
church will ever take.

Four properties that exist to stop the report flattering itself:

- **A stage never reached is an absence, not a zero.** Most windows contain no
  reference; counting "detection → fire" as 0 ms on those would report a 0 ms median
  forever. Pinned: `latency::tests::a_stage_never_reached_is_absent_not_zero`.
- **The clock starts at the OLDEST audio still waiting**, not the newest in the batch.
  Measuring from the freshest 200 ms lump describes the word that waited least.
  Switching to the honest end moved the reported base-model median from 158 ms to
  349 ms with no code change to the pipeline at all.
- **Histogram tails are counted, never truncated** — a P95 computed over a silently
  dropped tail improves as the pipeline gets worse.
- **The gap after a closed utterance is not a cadence.** That is a person not
  talking. Including it made the worst-case cadence read 8604 ms against a 201 ms
  median on five minutes of continuous speech.

The frontend reports its own paint (`Date.now()`, placed on Rust's monotonic
timeline); Rust also stamps the arrival, so the IPC hop is shown rather than folded
in. A kiosk browser source has no bridge and reports back over the same WebSocket the
content arrived on — that message stamps a diagnostic and touches nothing else, so
the hub's read-only guarantee is intact.

The rig that produces the numbers below is `stt::realtime::live_transcript_latency`:

```bash
RELAY_BENCH_WAV=…/sermon.wav RELAY_MODEL_PATH=…/ggml-base.bin \
  cargo test --release realtime::live_transcript_latency -- --ignored --nocapture
```

---

## 3. What changed

Three causes, none of them the decoder. Details in DECISIONS §38.

1. **The cadence floor was 250 ms in a unit the pipeline cannot deliver.** Audio
   arrives in 200 ms hops and no other size, so a 250 ms floor took two hops: the real
   cadence was 400 ms and the oldest word in each pair waited through 200 ms of it.
   Now exactly one hop, with the coupling pinned by
   `the_floor_is_exactly_one_hop_of_audio`.
2. **`STEP_SAFETY` was 1.5, and the headroom protected nothing.** The worker's loop
   already drains a batch whole and decodes it once; falling behind costs one decode
   however deep the queue. The slack was idle time. Now 1.0 — the cadence is the
   decoder's own speed. Pinned by
   `the_cadence_is_the_decoders_own_speed_not_a_multiple_of_it`, which asserts the
   opposite of what it used to.
3. **Detection ran on the decoder's thread** — semantic scan, three locks, a SQLite
   write, the Tauri emit and the kiosk fan-out, all between one decode and the next
   `recv()`. Now its own thread behind a **bounded** queue that sheds partials (and
   counts them) and blocks on finals.

**No threshold moved and no corroboration was removed.** The corroboration delay does
shrink, because it costs one cadence step and the cadence got shorter — which is the
difference between removing a safety rule and removing the wait in front of it.

---

## 4. Measured

M4 Pro, 14 cores, Metal, `--release`. 31.5 s of TTS speech containing four references,
fed at wall-clock pace. Milliseconds. **Before** and **after** differ by exactly the
two constants in §3; same binary otherwise.

### Audio received → transcript emitted

| model | | median | P95 | worst | updates/s |
|---|---|---|---|---|---|
| `ggml-base` (default) | before | 349 | 548 | 741 | 2.43 |
| | **after** | **139** | **339** | **543** | **4.74** |
| `ggml-small` | before | 787 | 1191 | 1269 | 1.44 |
| | **after** | **573** | **989** | **1328** | **2.18** |
| `ggml-large-v3-turbo` | before | 2397 | 2951 | 2951 | 0.80 |
| | **after** | **2360** | **2556** | **2887** | **0.82** |

Whisper decode itself, on the same runs: `base` 139 ms, `small` 370 ms, `turbo`
1240 ms per pass. **The decoder did not get faster. It stopped being made to wait.**

`turbo` barely moves because it was already decode-bound: its cadence was clamped by
the one-second ceiling before the change and by its own 1240 ms decode after it.

### Five minutes of continuous speech (`ggml-base`)

1075 transcript updates, **0 shed partials**.

| | median | P95 | worst |
|---|---|---|---|
| audio → transcript | 145 | 358 | 746 |
| whisper decode | 144 | 176 | 459 |

Per-minute means, first to last: **156 · 172 · 188 · 185 · 158 · 144 ms.** No growth.
A rise of ~30 ms in minutes 2–4 and a return; nothing that compounds.

### The floor a batch decoder cannot go below

`audio → transcript` settles at roughly **1.0–1.6 × the decode cost**: the oldest
audio in a pass has already been waiting through the previous pass. Whisper pads its
mel window internally, so a shorter window costs the same (§36) and there is no
cheaper pass available. **Above `base`, the model is the entire remaining latency**,
and that is a trade an operator makes when they pick a bigger model for accuracy — one
that was invisible to them until Diagnostics existed.

---

## 5. Against the acceptance criteria

| criterion | target | status |
|---|---|---|
| live transcript P50 | ≤ 300 ms | **139 ms** on `base`; 573 ms `small`; 2360 ms `turbo`. **Development machine, no webview.** |
| live transcript P95 | ≤ 700 ms | **339 ms** on `base`; `small` and `turbo` MISS |
| worst case | investigate > 1 s | 543 ms on `base`; `small` 1328 ms and `turbo` 2887 ms are the models, not the pipeline |
| no progressive growth | — | 6 minutes: flat. **A service is 90 minutes and has not been run.** |
| no unbounded backlog | — | queue bounded at 8; 0 shed partials in 1075 passes |
| detection ≤ 200 ms | ideally | **NOT MEASURED end to end.** The span is instrumented and unit-verified; no real-service sample exists |
| detection → fire ≤ 100 ms | ideally | as above |
| spoken reference → visible ≤ 1 s, P95 < 1.5 s | — | **NOT MEASURED.** Needs an app, an output page and a room |
| measured on the packaged build | — | **NOT DONE** |

---

## 6. What this does not establish — read this before quoting any number above

- **No microphone, no room, no preacher.** TTS audio is cleaner than any church
  signal: no reverb, no congregation, no accent, no code-switching, no plosives, no
  distance. Word error rate remains unmeasured in every language (`docs/LANGUAGES.md`
  says so, and this changes nothing about that).
- **No webview.** `audio_to_visible_transcript` in the rig is "a consumer was handed
  the text" — the console's own paint is not in it. The wiring for the real number
  exists and is pinned by tests; the number does not.
- **No output page, no projector, no LAN.** `end_to_end_speech_to_scripture` and
  `fire sent → output rendered` have no sample here at all.
- **Not the packaged build.** A signed, hardened-runtime `.app` has not been measured,
  and §17 of CLAUDE.md is a standing reminder that packaged builds behave differently
  in ways that are invisible until the one build handed to a church.
- **One machine.** An M4 Pro is not a church laptop. Every conclusion above about
  which model meets which target is a conclusion about this machine.
- **A new risk, deliberately taken and not yet tested.** With the cadence at the
  decoder's own speed, a model slower than one chunker hop keeps the decoder busy
  continuously for the length of a service. Thermal throttling would appear as a
  rising per-minute line in Diagnostics. Six minutes cannot produce it. **Stage F11 is
  the test, and it is the highest-value unrun item in the script.**

---

## 7. Release decision

**NO-GO stands**, unchanged and for the same reason as `QA-2026-08-14.md` §20: the
half of this product a volunteer actually experiences has still not been exercised by
anybody. What has changed is that the latency complaint now has a diagnosis, a fix
with before-and-after numbers, and an instrument that will answer it in the room
rather than in a benchmark.

**Stage F of the human test script is the path to GO.** F6, F7 and F8 are regression
tests and are green in CI. F1–F5 and F9–F14 need a person, a room and a packaged
build, and no amount of further work in this repository can turn them green.

---

<!-- ===== was docs/qa/audits/PERF.md, merged 2026-09-21, verbatim ===== -->

# PERF-MODELS-2026-08-30 — what each speech model costs, and what it costs the operator

**RG-30.** Measured on one machine (Apple Silicon, Metal), same binary, same window sizes,
three models, immediately after the first real service — which ran on
`ggml-large-v3-turbo` and produced the only field numbers this project has.

Reproduce:

```bash
cd src-tauri
RELAY_BENCH_MODEL="$HOME/Library/Application Support/com.relay.app/models/ggml-base.bin" \
  cargo test --release decode_cost -- --ignored --nocapture
```

---

> ### Standing, as of 2026-09-06 — read before §1
>
> **Nothing below is edited. This document measured speed, and §5 said in its first line
> that it measured nothing about accuracy. That gap has now been partly filled, from a
> real service rather than from a bench, and the missing column belongs beside the one
> that is here because the two were being read as one decision.**
>
> `FIELD-2026-09-06.md` ran two services on one machine and switched model mid-morning.
> Scored **through the router**, which is the only question SPEC sets a bar for:
>
> | model | 8 s window (bench, §1) | cadence in a service | auto-fires correct | wrong verses |
> |---|---|---|---|---|
> | `ggml-base` | 59 ms | 206 ms p50 | **5 of 9** | **4** |
> | `ggml-small` | 152 ms | not run in a service | **never measured** | **never measured** |
> | `ggml-large-v3-turbo` | 597 ms | 968 ms p50 | **3 of 3** | **0** |
>
> **Read that as evidence of a cost, not as a ranking.** It is one sample per model, the
> two samples are of different services as well as different models, and the reference
> list can only contain what Relay noticed or the operator fired, so it is a ceiling on
> correctness and says nothing about recall. `ggml-small` has no accuracy figure at all,
> which matters because §2's arithmetic makes it the cheapest row in the table: 152 ms and
> 59 ms both round up to the same single 200 ms hop, so `small` costs no cadence over
> `base`.
>
> The number that settles this is a replay of the 85.5 minute recording through all three
> models with `stt::bench::engine_shootout`, filed as **RG-116**. Until that has run,
> **44% of the auto-fires on `ggml-base` were wrong against SPEC's 5% bar**, and it is the
> model this project ships as recommended.
>
> **That replay ran on 2026-09-07, and it is 4.3 hours of real-time decoding on this one
> recording.** On the same audio, scored through the real router: `ggml-base` **4 of 8**
> correct with **3** wrong verses that could have reached a wall; `ggml-large-v3-turbo`
> **6 of 8** with **2**. The bench's raw counts are higher (5 and 3) because it stops at
> `router::decide`, and the live path then demotes a reference that is not in the corpus —
> `Psalms 721:27` and `Psalms 71:27` are both absent from the bundled KJV and could never
> have reached a congregation.
>
> **`ggml-small` returned 1 of 8 and that number is not usable.** It emitted 2583 distinct
> transcripts against `base`'s 13029 on identical input while decoding faster than `turbo`,
> and never once transcribed "Psalm 92", which the other two produced 23 and 13 times. That
> is a rig or model interaction rather than an accuracy result, so the model §2's
> arithmetic makes free is still the one nobody has measured. RG-116 carries the detail and
> stays open for it.
>
> **The ceiling is not 8.** Three of the eight references cannot be reached by this bench at
> all: one was spoken as a bare "verse 22" and resolved live against `ContextMemory`, and
> two were operator fires rather than parsed references.
>
> Word error rate is still unmeasured in every language. This does not change that.

---

## 1. The table

| model | load | 2 s window | 4 s window | 8 s window |
|---|---|---|---|---|
| `ggml-base` | 83 ms | 65 ms | 58 ms | **59 ms** |
| `ggml-small` | 172 ms | 149 ms | 151 ms | **152 ms** |
| `ggml-large-v3-turbo` | 810 ms | 581 ms | 593 ms | **597 ms** |

**The window length does not matter, for any of the three.** 58/59 ms for `base`,
593/597 ms for `turbo`. Whisper pads the mel window internally, so a shorter window buys
nothing — CLAUDE.md rule 27 has said so since it was measured on one model, and it now
holds across an order of magnitude of model size. Do not "optimise" `WINDOW_SECS`.

---

## 2. What it costs the operator, which is not the decode time

Decode cost is not the number a church experiences. The cadence is: the worker's step is
the measured decode cost rounded **up to a whole number of 200 ms chunker hops** (rule 32
— a floor finer than the delivery granularity is unachievable, and one between one hop
and two costs two).

| model | decode | cadence | transcript updates / s |
|---|---|---|---|
| `ggml-base` | 59 ms | 200 ms (1 hop) | ~5 |
| `ggml-small` | 152 ms | 200 ms (1 hop) | ~5 |
| `ggml-large-v3-turbo` | 597 ms | 800 ms (4 hops) | ~1.25 |

**`small` is free.** It costs 2.6× the decode of `base` and lands in the same single hop,
so an operator gets a better model at the same update rate. That is the most useful
sentence in this document and nothing in the repository said it before.

**`turbo` costs about four-fifths of the update rate**, and every reference has to survive
a corroboration pass (rule 28) — which is one cadence step, so it is four times longer on
`turbo` too. A verse takes noticeably longer to reach the wall. Whether that is worth the
accuracy is a church's decision, and it can now be made with the number in front of them.

---

## 3. The bench predicted the room

The live service on 2026-08-30 ran `ggml-large-v3-turbo` and measured
`stt_decode` **p50 687 ms across 2,423 decodes** (`FIELD-2026-08-30.md`).

This bench, on synthetic speech-shaped noise, on the same machine, says **597 ms**.

**~15% apart, in the right direction** — the room adds real speech, a webview, output
windows and a service recording. Two things follow, and the second matters more:

* the field measurement is not anomalous, and the bench is not a fiction;
* **a lab number and a room number can now be compared at all**, which was the whole
  argument for Stage F11.

Measured cadence in the field was **1.4 updates/s** against the 1.25 predicted here.

---

## 4. What this corrects

RG-30 was filed as *"every published latency figure is `ggml-base`"*. **That was too
strong**, and is recorded rather than quietly narrowed: DECISIONS §36 already carried a
per-model CPU-vs-Metal table, and `RELAY_GAP.md` §2 already listed `base` / `small` /
`turbo` audio-to-transcript figures.

The real gap was narrower and worse. The **headline** numbers — CLAUDE.md rule 32's
"139 ms median, 4.74 updates/s", the ones quoted in every summary — are `base`-only, and
nothing anywhere converted a model choice into **the update rate an operator actually
gets**. A church picked `turbo` and had no way to know it was choosing a quarter of the
cadence.

---

## 5. What this does not measure

* **Accuracy.** Not one word of this is about whether a bigger model hears better. Word
  error rate has never been measured in any language, and this changes nothing about that.
  The only reason to accept `turbo`'s cadence is accuracy, and that reason is currently
  an assumption.
* **Any machine but this one.** A church laptop without Metal is the case rule 27 exists
  for: the same `turbo` model measured **~1710 ms** on CPU, which is slower than real time
  and cannot keep up at all.
* **Thermals over a long service.** That is Stage F11, and `FIELD-2026-08-30.md` answers
  it for `turbo` on this machine: no drift across 49.5 minutes.

---
