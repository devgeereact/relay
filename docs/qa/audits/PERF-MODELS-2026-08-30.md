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
