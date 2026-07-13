# bench — turning the moat from a claim into a number

Relay's stated differentiator is African-language speech: **Yorùbá, Swahili, Hausa.**

Its word error rate in those languages has never been measured. Not once, in any language, including English.

That is not because the measurement is hard. **The ruler is built, unit-tested, and runs in CI today** (`stt::bench::wer`). It is because there has never been a single second of real sermon audio to point it at — and a decoder scored on a developer reading a verse into a MacBook in a quiet room tells you nothing about a preacher, at a lectern, over a cheap desk mic, in a hall with a ceiling fan.

**This directory is where a real recording goes. Everything below is one hour of work, and it unblocks the entire moat.**

---

## ⚠️ Audio never gets committed. Ever.

`bench/.gitignore` refuses `*.wav`, `*.f32`, `*.mp3`, `*.m4a` and `*.txt`. **Do not override it.**

`PRIVACY.md` promises a church that sermon audio never leaves their device. That promise is not conditional on the device being a church's — it is the promise. A recording of a real congregation, in a public repository, would break it in the most literal way possible, and no amount of "it was only for testing" repairs that.

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

`RELAY_BENCH_SCALE=0.2` is not a made-up number. It is roughly the level at which Relay used to go **silently deaf** — 94% of speech detected at studio level, **2%** at a church-laptop level, with no error and no warning, just a transcript quietly turning to nonsense (DECISIONS §19). If the WER at ×0.2 is not close to the WER at ×1.0, the audio front-end has regressed and a church will never tell you.

---

## What the number is for

The first run is a **baseline, not a pass mark.** The bench asserts nothing on purpose: inventing a target before the first measurement is choosing the number you would *like* rather than the one that is *true*.

Once a baseline exists, four questions that are currently unanswerable become arithmetic:

1. **Is the decoder-bias prompt helping or hurting?** `stt.rs` currently primes whisper with all 66 book names — and the code's own comment argues that `initial_prompt` is *prior context, not a vocabulary list*, and that a noun-dump "actively harms accuracy… it starts hallucinating them." Nobody knows which is true. `prompt_sweep` settles it in one run.
2. **Would a fine-tuned Yorùbá model actually help, and by how much?** Community fine-tunes exist. Relay ships none, deliberately, because none has been *verified against real sermon audio* (`LANGUAGES.md:164`). This is that verification.
3. **Does the front-end hold up at church levels?** See ×0.2 above.
4. **What is the real detection recall?** `eval.rs` scores 100% — on hand-written clean text. Recall on *ASR output* is a different and much harder number, and it is the one that decides what reaches a wall.

Every one of those is currently an assertion in a document. Thirty minutes of tape turns all four into measurements.

---

## Also useful, and also missing

Real transcripts from a service — even without the audio — would let `eval.rs`'s 50-case corpus grow from hand-written examples into **things a preacher actually said**. Its ten most valuable cases today are the garbled ones taken from a real rehearsal, and it shows.
