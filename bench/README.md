# bench — turning the moat from a claim into a number

Relay's stated differentiator is African-language speech: **Yorùbá, Swahili, Hausa.**

Its word error rate in those languages has never been measured. Not once, in any language, including English.

That is not because the measurement is hard. **The ruler is built, unit-tested, and runs in CI today** (`stt::bench::wer`). It is because there has never been a single second of real sermon audio to point it at — and a decoder scored on a developer reading a verse into a MacBook in a quiet room tells you nothing about a preacher, at a lectern, over a cheap desk mic, in a hall with a ceiling fan.

**This directory is where a real recording goes. Everything below is one hour of work, and it unblocks the entire moat.**

---

## ⚠️ Audio never gets committed. Ever.

`bench/.gitignore` refuses `*.wav`, `*.f32`, `*.mp3`, `*.m4a` and `*.txt`. **Do not override it.**

`../docs/PRIVACY.md` promises a church that sermon audio never leaves their device. That promise is not conditional on the device being a church's — it is the promise. A recording of a real congregation, in a public repository, would break it in the most literal way possible, and no amount of "it was only for testing" repairs that.

Keep the file locally. Point the bench at it. Commit **the number**, never the recording.

---

## What to record

**Thirty minutes.** Less is not enough to be representative; more is not needed to be conclusive.

What matters far more than the length:

| Get this right | Why |
|---|---|
| **A real preacher, preaching.** Not someone reading a list of verses. | Read speech and preached speech are different signals — pace, volume, emphasis, and the pauses are in different places. The whole audio front-end was rebuilt because a *quiet* preacher was silently undetectable (DECISIONS §19). |
| **The mic the church actually uses.** A desk feed, a lapel, a laptop across the room. | A studio recording measures the one case Relay already handles. |
| **Code-switching, if that is how they preach.** English mid-sentence in a Yorùbá sermon is the normal case, not an edge case. | It is written into `CLAUDE.md` as a constraint, and it has never been tested against real audio. |
| **Actual scripture references, spoken naturally.** | This is the thing being measured. |
| **The room, as it is.** Fans, children, air conditioning, a PA hum. | Do not clean it up. The noise *is* the test. |

**Get their permission, and mean it.** Tell them what it is for, that it stays on one machine, and that it will not be published. Then keep to that.

---

## Format

Whisper wants raw **f32 mono @ 16 kHz** — the exact bytes the live worker feeds it, so the number is real and not a proxy.

```bash
ffmpeg -i sermon.m4a -ac 1 -ar 16000 -f f32le bench/sermon.f32
```

### If the recording came out of Relay itself

`RELAY_RECORD_WAV` writes the CLEANED capture stream at the device's rate, which on the desk
feed used in the field is **48 kHz mono f32**. Whisper wants 16 kHz, and nothing in the bench
resamples, so a 48 kHz file replayed as-is decodes at three times speed and every number it
prints is nonsense. Convert first. On a Mac with no ffmpeg, `afconvert` ships with the OS:

```bash
afconvert -f WAVE -d LEF32@16000 -c 1 --src-quality 127 service.wav sermon16k.wav
```

> **`afconvert` writes a 4044-byte `FLLR` padding chunk, so the audio starts at byte 4096 and
> not at 44.** `bench::load_f32` walks the chunk list rather than assuming a 44-byte header,
> and refuses a file that is not 16 kHz mono 32-bit float, because both of those read as
> *noise* rather than as an error: this bench asserts nothing, so a misread prints a
> confident-looking table of zeroes. Either feed it the converted WAV directly or strip the
> header yourself; both work.


And a reference transcript — **what was actually said** — in `bench/sermon.txt`.

> ### Write the transcript AS SPOKEN, not as printed.
>
> `john three sixteen`, **not** `John 3:16`.
>
> The scorer folds punctuation and casing (whisper varies them run to run, and charging the decoder for that would drown the errors that matter) — but `3:16` and `three sixteen` are genuinely different words, and it will count them as an error. That would be measuring the *transcriber's* formatting choice, not the decoder's accuracy.
>
> **Keep the tone marks.** In Yorùbá they are not decoration; they change the word.

---

## Run it

```bash
export PATH="$HOME/.local/bin:$PATH"          # cmake, for whisper-rs
cd src-tauri

RELAY_BENCH_WAV=../bench/sermon.f32 \
RELAY_BENCH_TRANSCRIPT=../bench/sermon.txt \
RELAY_BENCH_LANG=yo \
  cargo test --release stt::bench::word_error_rate -- --ignored --nocapture
```

Then do it again as a **church laptop**, which is the case that actually matters:

```bash
RELAY_BENCH_SCALE=0.2 RELAY_BENCH_NOISE=0.01 ...   # quiet mic, in a noisy room
```

### The one that compares engines and models

`word_error_rate` scores the transcript. **`engine_shootout` scores the wall** — it runs every
installed model through the *real* `SttEngine`, over the whole degradation grid, and ranks
them on the only question that matters: which one puts the wrong verse in front of a
congregation. It needs a second, much smaller file — the references actually cited in the
recording, one per line:

```
# bench/refs.txt — references actually cited in sermon.f32
Romans 8:28
John 3:16
```

```bash
RELAY_BENCH_WAV=../bench/sermon.f32 \
RELAY_BENCH_REFS=../bench/refs.txt \
  cargo test --release --features metal stt::bench::engine_shootout -- --ignored --nocapture
```

**On a real service recording, pick a subset or it will not run.** Each condition is one
real-time replay, so the five-condition grid over 85.5 minutes of sermon is **seven hours per
model** and twenty-one for three. `RELAY_BENCH_CONDS` and `RELAY_BENCH_MODELS` cut it to the
question in hand, and both fail loudly on a name that matches nothing:

```bash
RELAY_BENCH_CONDS=clean RELAY_BENCH_MODELS=base,small,turbo ...   # 3 x 85.5 min
```

`RELAY_BENCH_MODELS` matches a SUBSTRING of the model filename, so `base` also selects
`ggml-base.en` if it is installed. Read the `engines scored:` line the run prints rather than
assuming what the filter caught.

> ### Pin the language, or you are measuring the language detector
>
> Unset means auto, which is what a church gets by default and is worth measuring — but it
> is a **different measurement**, and one model can lose to another purely by electing a
> different language. `ggml-small` did exactly that on 85.5 minutes of real service audio:
> it scored 1 of 8 where `base` scored 4 and `turbo` 6. On a 70 second slice of the same
> recording it produced **17** distinct transcripts of multilingual noise ("sous
> interpersonal work", "pee Samus putzein thrilled") against `base`'s **231** of coherent
> English, at 3.4 s lag against 0.3 s, and found nothing.
>
> With `RELAY_BENCH_LANG=en` on the identical slice: **161** transcripts, coherent English,
> the verse found, lag 0.4 s. Same model, same audio, same rig.
>
> The header prints `language: pinned to en` or `language: auto (whisper re-elects one per
> window)` on every run. Quote it with the number, because the two are not comparable.

`clean` is the right first cut on field audio: the recording already contains the room, the
microphone and the preacher, so degrading it further asks a different question. Both subsets
are printed on every run, and `total` is derived from the conditions actually run, so numbers
from two differently-filtered runs are not comparable.

> **The bench scores `detect_direct` only, and the live path does not.** `emit_detections`
> also runs `detect_bare_verses` against `ContextMemory` and the window anchor, which is the
> path that produced two of the four wrong verses on 2026-09-06 (RG-115 / F-8). So this bench
> cannot reproduce that class at all: a reference the preacher gave as a bare "verse 22" reads
> here as a MISS, and a wrong verse invented from memory cannot appear. Its wrong-verse rate is
> therefore about the *decoder plus the direct parser*, not about everything that can reach a
> wall, and it is not the same quantity as a field audit's.

> **The reference list is a CEILING, not a transcript.** It can only hold what Relay noticed
> or the operator fired. A reference the preacher spoke and nothing caught cannot be in it, so
> the rate it yields says nothing at all about recall. Where the live run fired the *wrong*
> verse, the list must carry what was **said**, not what appeared, or the replay scores the
> original defect as a success.


Unlike every other bench here it drives audio in through `SttEngine::sender()`, so the
measurement contains the whole pipeline — the voice gate, `Deoverlap`, the rolling window,
the batch drain — and not just a call into whisper. That is what makes it able to compare a
future non-whisper backend at all, and it is why each row also prints **`voiced N/M`**: if
the gate passed nothing, the decoder was never given a sample and no model will fix it.
Results are then scored **through the real `Router`**, like `eval.rs`, so "wrong verse"
means *would have reached the wall*, not *was briefly considered*.

Read the output in this order: **WRONG VERSES first**, lag second, correct third.

> ### It runs in real time, and that is not an oversight
>
> Audio is fed at the pace a room produces it. Pushing the whole clip in at once is not a
> faster version of this measurement, it is a different one: the worker drains the backlog
> in a single batch and decodes **once**, on the freshest 8 seconds, which is correct live
> behaviour and is what stops lag compounding through a sermon. The first version of this
> bench did exactly that, and every model scored identically — because it was scoring one
> window per model, and every reference spoken earlier had simply never been transcribed.
>
> So thirty minutes of tape takes thirty minutes per condition. `RELAY_BENCH_SPEED=4` will
> go faster and will start re-creating that collapse; the numbers get pessimistic and stop
> being comparable to a real service.
>
> `RELAY_BENCH_VERBOSE=1` prints every distinct transcript and anything the router offered
> but did not auto-fire. A reference can go missing two ways — the decoder never said the
> words, or it said them and detection did not parse them — and the score alone cannot tell
> those apart.

`RELAY_BENCH_SCALE=0.2` is not a made-up number. It is roughly the level at which Relay used to go **silently deaf** — 94% of speech detected at studio level, **2%** at a church-laptop level, with no error and no warning, just a transcript quietly turning to nonsense (DECISIONS §19). If the WER at ×0.2 is not close to the WER at ×1.0, the audio front-end has regressed and a church will never tell you.

---

## What the number is for

The first run is a **baseline, not a pass mark.** The bench asserts nothing on purpose: inventing a target before the first measurement is choosing the number you would *like* rather than the one that is *true*.

Once a baseline exists, five questions that are currently unanswerable become arithmetic:

1. **Is the decoder-bias prompt helping or hurting?** `stt.rs` currently primes whisper with all 66 book names — and the code's own comment argues that `initial_prompt` is *prior context, not a vocabulary list*, and that a noun-dump "actively harms accuracy… it starts hallucinating them." Nobody knows which is true. `prompt_sweep` settles it in one run.
2. **Would a fine-tuned Yorùbá model actually help, and by how much?** Community fine-tunes exist. Relay ships none, deliberately, because none has been *verified against real sermon audio* (`LANGUAGES.md:164`). This is that verification.
3. **Does the front-end hold up at church levels?** See ×0.2 above.
4. **What is the real detection recall?** `eval.rs` scores 100% — on hand-written clean text. Recall on *ASR output* is a different and much harder number, and it is the one that decides what reaches a wall.
5. **Is `base` costing us verses?** Relay ships `ggml-base`, the smallest useful whisper, and nothing has ever compared it against a larger one on real speech. Before anyone concludes the recognition engine needs replacing, this is the cheaper question, and `engine_shootout` answers it in one run — including whether the larger model still fits inside the realtime budget on a church laptop, which is the reason a bigger model is not automatically better.

Every one of those is currently an assertion in a document. Thirty minutes of tape turns all five into measurements.

---

## Also useful, and also missing

~~Real transcripts from a service — even without the audio — would let `eval.rs`'s corpus grow from hand-written examples into **things a preacher actually said**.~~ **Done, 2026-08-30:** six verbatim lines from a live service are in the corpus (`source: FIELD-2026-08-30`), taken from `detections.heard_text`. **The audio is still missing, and that is the gap that matters** — transcripts measure detection over TEXT; word error rate needs the WAV. Set `RELAY_RECORD_WAV` for one service and that changes.

---

## The two variables nothing documented

Every other `RELAY_*` knob is named above; these two were in no document in the
repository until 2026-09-05, and they belong to the cheapest experiment here.
`router::tests` replays a transcript through the REAL router and prints which
verse would have reached a wall at each sensitivity, so you can see what a
threshold change would have done to a service that already happened:

```bash
RELAY_SWEEP_TRANSCRIPT=/path/lines.txt RELAY_SWEEP_TRUTH="Romans 10:17" \
  cargo test sweep -- --ignored --nocapture
```

`RELAY_SWEEP_TRANSCRIPT` is one utterance per line — `detections.heard_text` from
a real service is exactly the right input. `RELAY_SWEEP_TRUTH` is the reference a
human says was correct.
