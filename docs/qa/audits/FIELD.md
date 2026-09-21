# Field services — the record every real Sunday left

**Four field audits, merged into one file on 2026-09-21** and otherwise untouched. Each was
written on the day it is dated and is **frozen**: closures go in `../RELAY_GAP.md`, never into a
finding below. Nothing here is edited except the paths of citations, retargeted so every citation
still resolves after the merge.

They are together because they answer one question — *what happened when Relay ran in a room* —
and four separate files made the series hard to read as a series. That series is the argument:
one wrong verse, then four, then none, then two, on three different models with the model pinned
for the last two.

**What is in this file, in order:**

- `FIELD-2026-08-30.md` — FIELD-2026-08-30 — the first real service
- `FIELD-2026-09-06.md` — FIELD-2026-09-06 — two services, two models, four wrong verses
- `FIELD-2026-09-13.md` — FIELD-2026-09-13 — the first service with no wrong verse
- `FIELD-2026-09-20.md` — FIELD-2026-09-20 — the fifth service, and two wrong verses ten minutes apart

---

<!-- ===== was docs/qa/audits/FIELD.md, merged 2026-09-21, verbatim ===== -->

# FIELD-2026-08-30 — the first real service

**Stage F11, and the first minutes of Stage C.** A live sermon, a real preacher, a real
room, a real machine, measured while it happened.

Everything in this document was recorded by Relay itself, read back from
`~/Library/Application Support/com.relay.app/relay.db` **read-only while the service was
running** — nothing was written to that database by the audit. Numbers here are quoted with
the query that produced them.

> **On quoting the preacher.** Relay's own rule is that nothing leaves the device without an
> explicit, visible reason, and a sermon is the church's material, not diagnostic data. Every
> quotation below is a fragment that **names a scripture reference** and is here because it
> is the evidence and, in the failure case, the regression test. Sentences that carried no
> reference are described, not quoted. The owner of this service reviewed the decision to
> publish these fragments; if that ever stops being true, redact §3 and this audit still
> stands on its numbers.

> **This is the number that was 0 / 10.** `docs/RELAY_GAP.md` §22 scores twelve dimensions
> and gives no average, on the grounds that an average would hide the two that matter.
> **Field validation** was the zero. It is no longer zero. It is also not ten: one service,
> one preacher, one language, one machine, one room.

---

## 0. Method, and what it cannot see

| | |
|---|---|
| Date | 2026-08-30 |
| Build under test | **0.1.0-4**, built from `main` at `895a976` and installed to `/Applications` mid-session |
| Model | **`ggml-large-v3-turbo`** — *not* `ggml-base` |
| Service | id 13, **49.5 minutes**, 102 final transcripts, ended cleanly |
| Instrument | `perf_samples` + `service_events` (RG-04 / RG-14), read with `sqlite3 … ?mode=ro` |
| Sampling | one row per metric per 60 s, written by `snapshot_latency` |

**What no instrument here could see:** whether the congregation could read the screen, what
the room sounded like, how many references the preacher spoke that Relay never noticed
(recall — see §4, which is honest about being an upper bound), and word error rate, which
still has never been measured in any language.

**One confound, recorded rather than smoothed over.** The first three minutes of the service
overlapped a full `cargo build` and several `cargo test` runs on the same machine. The p95
jump between 11:23 and 11:24 lines up with them exactly. Compilation was stopped for the rest
of the service and every trend figure below is drawn from the quiet period. An audit that
contaminates its own measurement and does not say so is worse than no audit.

---

## 1. The build swap, which was itself a test

The app that was running when the service began was **0.1.0-3** — three rounds behind, with
no `service_events` and no `perf_samples`. Stage F11 asks whether latency drifts upward over
a long service, and that question cannot be asked of a build with no persisted latency.

So 0.1.0-4 was built from `main`, the live database was backed up, and the app was replaced
and relaunched between sections.

**The migration ran on a real database with real history and lost nothing:**

```
12 services · 147 transcripts · 31,100 verses · 31 templates   — all intact
service_events · perf_samples · environment_profiles           — created
song_arrangements.built_shape                                  — added
```

That is the first time CLAUDE.md rule 25's retryable-migration path has run on anything but a
test fixture. It is a small result and it is real.

---

## 2. Stage F11 — the answer is *no drift*

The question is not "is Relay fast". It is **"is the per-minute line rising?"** — because a
pipeline permanently one cadence step behind the preacher reports a zero backlog for a whole
service (CLAUDE.md rule 31), and only a rising line distinguishes "constant lag" from
"falling further behind".

`stt_decode`, cumulative, over the whole service:

| minute | samples | p50 ms | p95 ms | worst ms |
|---|---|---|---|---|
| 0.3 | 6 | 627 | 818 | 817 |
| 4.3 | 200 | 697 | 2690 | 3016 |
| 8.3 | 392 | 696 | 2684 | 3016 |
| 12.3 | 616 | 688 | 2680 | 3016 |
| 16.3 | 816 | 686 | 2680 | 3016 |
| 20.3 | 1024 | 689 | 2682 | 3016 |
| 24.3 | 1222 | 688 | 2681 | 3016 |
| 28.3 | 1424 | 688 | 2677 | 3016 |
| 33.3 | 1688 | **687** | **2673** | 3016 |

**Flat, under a denominator that grew 280×.** p50 moved 697 → 687 — *downward*, and by less
than the width of one 200 ms chunker hop. `worst` has not moved since minute four. Dropped
partials: **0** for the entire service. Transcript rate: **1.4 updates/s**, which is what a
~690 ms decode dictates once the cadence is rounded to whole hops — rule 32 behaving exactly
as designed, in a room, for the first time.

### The whole picture, by stage — FINAL, whole service

| metric | samples | p50 | p99 | worst |
|---|---|---|---|---|
| `stt_decode` | 2423 | 627–699 ms | 2801 ms | 3016 ms |
| `transcript_cadence` | 2326 | 653–945 ms | 3276 ms | 5972 ms |
| `audio_to_partial_transcript` | 2423 | 1073–1503 ms | 5365 ms | 5614 ms |
| `audio_to_visible_transcript` | 411 | 1473–9860 ms | 12690 ms | 12690 ms |
| `transcript_to_reference_detection` | 1005 | **7–9 ms** | 18 ms | 55 ms |
| `reference_detection_to_fire` | 6 | **6 ms** | 9 ms | 8 ms |
| `end_to_end_speech_to_scripture` | 5 | **39117 ms** | 89848 ms | 89848 ms |

**Detection is not the problem, again.** 7–9 ms from transcript to reference over 1,005
samples, and 6 ms from that decision to the fire. Rule 31 warns that twice the reflex answer
was "STT is slow" and twice more than half of it was not; this time the reflex answer is
right, and the instrument says *which* stage rather than leaving it to be guessed.

**`end_to_end_speech_to_scripture` reads 39–90 SECONDS, and it cannot be true.** Every stage
it is supposed to span sums to roughly 1.1 s: 1073 ms of audio-to-partial, 9 ms of detection,
6 ms to fire. A metric that disagrees with the sum of its own parts by a factor of forty is
measuring something other than what its name says — most likely a trace whose clock starts at
audio that has nothing to do with the fire, which is exactly what a MANUAL fire is (there
were two, and only five samples in total). Recorded as **F-6**. It is an instrument defect
until proven otherwise, and it must be resolved before any figure from it is quoted anywhere.

The 3.4 s `audio_to_visible_transcript` is **mostly the 8-second window, not lag**: rule 31
deliberately starts that clock at the *oldest* audio still waiting, so a median near half the
window is the expected reading. Measuring from the freshest 200 ms lump would report 158 ms
and would be a lie.

### The model is the headline, and it was never measured before

Every latency figure this project quotes — the 139 ms P50, 4.74 updates/s of
`PERF-2026-08-24.md` — is **`ggml-base`**. The machine in this church runs
**`ggml-large-v3-turbo`**, and nobody had ever measured it in a room. Decode is **~5×** the
`base` figure and the transcript rate is **1.4/s against 4.74/s**.

That is not a regression. It is the cost of a better model, quantified for the first time,
and it is a decision a church should make with the number in front of them.

---

## 3. What reached the wall

Six auto-fires and two manual fires in ~34 minutes. **Five of the six auto-fires were
correct.**

| heard (`detections.heard_text`) | fired | conf | |
|---|---|---|---|
| "David did the same thing in 2 Samuel 24, 24" | 2 Samuel 24:24 | 0.55 | ✅ |
| "Look at Deuteronomy 15, 7 to 8" | Deuteronomy 15:7 | 0.55 | ✅ |
| "…the bible says exodus 35 verse 21" | Exodus 35:21 | 0.95 | ✅ |
| "Look at what the Bible says in 2 Corinthians 9, 8" | 2 Corinthians 9:8 | 0.55 | ✅ |
| "That's why Isaiah 54, 2 to 3 was screaming at us" | Isaiah 54:2 | 0.55 | ✅ |
| "…what was going through in **Luke 10**. If you read from **verse 32, 37**." | **Proverbs 3:32** | **0.88** | ❌ |

### The one that was wrong, and why it matters more than the five that were right

The preacher cited **Luke 10:32–37** — the Good Samaritan, which is also what the surrounding
sentence is about ("the man that was wounded"). Relay put **Proverbs 3:32** on the screen at
0.88 against a 0.50 bar, with nobody asked.

### The mechanism, and the first diagnosis was wrong

The first reading of this — written into an earlier revision of this document — was that the
parser mis-read the sentence and `rank_for_wall` ordered the wrong candidate first. **That was
wrong, and the test proved it wrong:** `detect_direct` does not produce Proverbs from this
text at all, and a regression test written against that theory passed with the supposed fix
reverted. A test that cannot fail is a theory that was never tested.

The real path:

1. The operator **manually fired Proverbs 3:6** at t=521 s. `ContextMemory.current` becomes
   Proverbs 3.
2. Five minutes later the preacher says *"…going through in **Luke 10**. If you read from
   **verse 32**, 37."*
3. `detect_bare_verses` sees `32`. `ContextMemory::resolve_bare_verse` hangs it on the
   remembered passage → **Proverbs 3:32**, pushed as a candidate at a **hardcoded 0.88** with
   a **hardcoded `DetectionMethod::Direct`**.
4. 0.88 clears the 0.50 bar. It fires.

Both hardcoded values are in the record: the database row reads exactly `0.88` and `direct`.

**Luke 10 was in the same sentence and lost to a memory five minutes old.** Memory is what
Relay has when the words do not say; when the words do say, the words must win. That is now
the rule — `detection::anchor_for_bare_verses` — and it costs the useful case nothing, because
"and verse eighteen" with no book named still has nothing to anchor to and still falls back to
memory.

### One thing deliberately left open

A context-resolved bare verse is labelled `Direct` at a constant 0.88. Relay did not *hear*
"Proverbs 3:32"; it heard "verse 32" and inferred the rest. By rule 10's own principle — only
what was actually heard may reach a congregation unattended — that label is a lie, and 0.88 is
a number rather than a measurement. It is **not** changed here, because a preacher walking a
passage saying "verse eighteen" is the case the path exists for and one service is not
evidence enough to make all of those ask for a click. Recorded as an open question, not fixed
by silence.

Two things make this the most valuable defect this project has recorded:

1. **The correct reference was in the same window and lost to five-minute-old memory.**
2. **The exact utterance is preserved**, in `heard_text` — the column that exists precisely
   because a service once put wrong verses up and the log could not say what it had heard.
   It becomes a regression test verbatim, which is the difference between a story about a bug
   and a test that fails.

### The four at 0.55 vindicate a decision that looked risky

"Deuteronomy 15, 7" — a bare book-chapter-verse with no keyword — is deliberately scored 0.55
and deliberately still fires at the default dial, over the objection that nobody says it.
**Four of the five correct fires were exactly that shape.** It is how this preacher speaks.

That changes the advice about the sensitivity dial: dragging it toward cautious would have
held back the wrong verse *and four of the five right ones*. One wrong verse in thirty
minutes against five correct is a trade a church may reasonably accept — and it is their
trade to make, with the number visible.

---

## 4. Recall is not measured here, and the reason is itself a finding

Counting what Relay *missed* means reading every sentence the preacher spoke. That cannot be
done from this record, because of the finding below.

**Seventy-two final transcripts (of the eventual 102) contained the words "verse", "chapter"
or "bible" exactly zero times** — while four `heard_text` values contain all three. Verified
mid-service:

```sql
select count(*) from transcripts where service_id = 13;                     -- 72
select count(*) from transcripts where service_id = 13
  and (lower(text) like '%verse%' or lower(text) like '%chapter%'
    or lower(text) like '%bible%');                                          -- 0
```

Detections born in a **partial** window are stored against the last **final** transcript,
because only finals are persisted. So the service record shows `Proverbs 3:32` beside a
sentence about something else entirely — one that contains no book, no number and no
keyword. Every history and replay surface built on `detections → transcripts` will show
verses next to sentences that did not produce them, and any future attempt to score accuracy
from the transcript table will score the wrong text.

`heard_text` is the only honest column. That it exists is the reason this was findable at
all.

---

## 5. Findings, in the order they should be fixed

| # | Finding | Evidence |
|---|---|---|
| **F-1** | A real sermon auto-fired **Proverbs 3:32** for a spoken **Luke 10:32–37**, at 0.88, unattended. A bare "verse 32" was hung on a passage fired by hand five minutes earlier, while Luke 10 sat in the same sentence. **FIXED** — a reference named in the window now outranks memory | §3 |
| **F-2** | Detections from partial windows are attributed to the wrong transcript row, so the service record misreports what was heard | §4 |
| **F-3** | `perf_samples` persists **cumulative** percentiles (`latency::report(0)`), which are structurally insensitive to drift — the exact question Stage F11 asks. `worst` can only ever rise | §2 |
| ~~**F-4**~~ | **WITHDRAWN.** Mid-service this metric sat at 28 samples while `stt_decode` passed 1,688, and it was filed as a stage that had stopped reporting. It had not: it finished the service with **411 samples**. It is sparse, not stopped — it only stamps when a transcript becomes visible, which is far rarer than a decode. **The finding was drawn from a snapshot and the snapshot was not the run.** Left in place struck through rather than deleted, because a register that quietly loses its wrong entries teaches nothing | §2 |
| **F-6** | `end_to_end_speech_to_scripture` reports 39–90 s where its own constituent stages sum to ~1.1 s. Five samples, two of the service's eight fires were manual, and a manual fire has no audio origin to measure from | §2 |
| **F-7** | The main output screen was **lost and recovered twice** during the service (`service_events`: `output_lost` · `output_recovered` ×2). RG-01/RG-02's health tracking caught it, in the field, exactly as designed — and a sleeping display is the leading candidate for the cause, which is what RG-28 (`wake.rs`) exists to prevent | `service_events`, service 13 |
| **F-5** | Every published latency figure is `ggml-base`; the model a church actually chose costs ~5× per decode and a third of the update rate, and had never been measured | §2 |

**F-1 and F-2 are defects. F-3 and F-4 are defects in the instruments.** F-5 is not a defect
at all — it is a missing measurement, now taken.

---

## 6. What this does and does not change

**Changed:** field validation is no longer zero. Stage F11's specific question — *does the
per-minute line rise over a long service* — is answered **no**, on this machine, with this
model, across **49.5 minutes and 2,423 decodes**.

**And the output-health work earned itself in the field.** `service_events` records the main
screen `output_lost` and `output_recovered`, twice. Before RG-01/RG-02 the console would have
read **On Air** over a screen that had stopped answering, twice, and nobody would have known.

**Changed on 2026-08-31, and this service is why:** the release decision moved from an
indefinite **NO-GO** to **NO-GO for general release, GO for a supervised pilot**
(`docs/RELAY_GAP.md` §24). Not because this run went well — it put a wrong verse on a wall —
but because it produced seven findings that months of reading source had not, and the evidence
the remaining blockers need cannot be manufactured without more services like it.
One service is one service. Word error rate is still unmeasured in every language, no native
speaker has reviewed the alias tables, the Windows certificate does not exist, and nothing
here was run by an operator who did not write the software.

**One wrong verse reached a congregation during this service.** That is the sentence to keep.

---

## 7. What the findings became

| # | Outcome |
|---|---|
| **F-1** | Fixed. A bare verse hangs on the reference this window names; memory is the fallback. DECISIONS §56, RG-24. **The first diagnosis was wrong and is recorded as wrong.** |
| **F-2** | Fixed. A detection points at the words that produced it. DECISIONS §57, RG-25 |
| **F-3** | Fixed. `perf_samples.last_minute_ms` persists the per-minute line that already existed and was thrown away on quit. DECISIONS §57, RG-26 |
| ~~F-4~~ | **Withdrawn.** Filed from a mid-service snapshot; the metric finished with 411 samples, not 28 |
| **F-5** | Open, RG-30. Every published latency figure is `ggml-base`; this service is the only measurement of `large-v3-turbo` that exists |
| **F-6** | Fixed. The end-to-end clock ran from the start of the utterance, so it measured how long the preacher had been talking. DECISIONS §57, RG-31 |
| **F-7** | The main screen was lost and recovered twice. RG-28 (`wake.rs`) is the leading candidate for the cause and is now in |

Six findings, five fixed, one withdrawn as wrong, one open — from **one service**. That
ratio is the argument for Stage C and Stage F11 in a sentence.

---

<!-- ===== was docs/qa/audits/FIELD.md, merged 2026-09-21, verbatim ===== -->

# FIELD-2026-09-06 — two services, two models, four wrong verses

**Stage F11 again, and the first service Relay has run twice in one morning.** Two live
services on one machine, the second of them 85.5 minutes, measured while they happened.

Everything here was recorded by Relay itself and read back from
`~/Library/Application Support/com.relay.app/relay.db`. Numbers are quoted with the query or
the log line that produced them. Where this audit is uncertain it says so, and where it
reversed its own first reading it says that too, because it did so twice.

> **On quoting the preacher.** Same rule as `FIELD-2026-08-30.md` §0: every quotation below is
> a fragment that names a scripture reference, and it is here because it is the evidence and,
> in the four failure cases, the regression test. Sentences carrying no reference are
> described, not quoted.

> **This is the first service where the audit was running live rather than afterwards.** The
> operator asked for monitoring during the service, so faults were watched as they happened
> and two of the findings below were caught inside the hour rather than in the database
> afterwards. That is a change in method, and §7 is honest about what it cost.

---

## 0. Method, and what it cannot see

| | |
|---|---|
| Date | 2026-09-06 |
| Build under test | **0.2.0-2**, packaged, installed at `/Applications/Relay.app`, built from `main` at `f3f2ce8` |
| Signature | **adhoc, linker-signed** (`codesign -dvvv`: `flags=0x20002(adhoc,linker-signed)`, `TeamIdentifier=not set`). No hardened runtime, so rule 17's trap is not in play and the microphone works. Still no certificate on either platform (RG-73) |
| Models | **both, and this is the headline.** `ggml-large-v3-turbo` for service 14; `turbo` then **`ggml-base`** for service 15, switched mid-service |
| Services | id **14**, sensitivity 50, 37 finals; id **15**, sensitivity 25, 83 finals, 85.5 minutes |
| Input | **Blackmagic Web Presenter 4K**, the desk feed. Both launches defaulted to the laptop microphone first (F-15) |
| Output | **the console program pane only.** One monitor, no projector, no kiosk client, nothing established on `:8031`. Supported by design: `TemplateRender.svelte` is the one renderer, so what the operator watched is what a wall would have shown |
| Instruments | `detections`, `perf_samples`, `service_events`, the process log, and an 85.5 minute WAV of the desk feed |

**What no instrument here could see:** whether a congregation could read anything (nothing
reached a congregation-facing screen today), what the room sounded like, how many references
the preacher spoke that Relay never noticed, and word error rate, which still has never been
measured in any language.

**One confound, recorded rather than smoothed over.** The model changed inside service 15, so
that service is not one measurement. Every figure below that separates the two models does so
by service, and service 14 is the only clean single-model sample of the day.

---

## 1. What reached the output

### Service 14, `turbo`, auto-fire bar 0.50

| t | heard | fired | conf | |
|---|---|---|---|---|
| 181.9 s | "They did everything. But in **Joshua chapter 1**, God said to Joshua, arise, go over the" | Joshua 1:1 | 0.88 | ✅ |
| 212.0 s | "of power, and of a sand mind. **Joshua 1.9**, be strong, and of a good country." | Joshua 1:9 | 0.55 | ✅ |
| 460.4 s | "Doors open. Praise He the Lord. Open with me to the book of **Genesis 4 2 to 7a**." | Genesis 4:2 | 0.55 | ✅ |

**Three of three correct**, plus nine manual fires. Two of the three transcripts are visibly
wrong ("of a sand mind" for *sound mind*, "of a good country" for *good courage*) and the
reference survived both, which is CLAUDE.md rule 13 working exactly as written: the only
question is which verse would reach a screen.

### Service 15, `base` for most of it, auto-fire bar 0.70

| t | heard | fired | conf | should have been | |
|---|---|---|---|---|---|
| 237.3 s | "dedicated this morning before God and God's people is from **Psalm 89 verse 20 to 23**" | Psalms 89:20 | 0.95 | | ✅ |
| 263.9 s | "as I'm declaring this prophetic word from the Word of God. **This is verse 22**." | Psalms 89:22 | 0.88 | | ✅ |
| 386.3 s | "**21 verse 27, 21**, it says, You shall increase my greatness and comfort me in" | Psalms 89:27 | 0.88 | **Psalms 71:21** | ❌ |
| 387.4 s | "**Verse 7, 21**, it says, You shall increase my greatness and comfort me on every side." | Psalms 89:7 | 0.88 | **Psalms 71:21** | ❌ |
| 668.3 s | "You know the Sami says in **Psalm 150 verse 6**." | Psalms 150:6 | 0.95 | | ✅ |
| 868.4 s | "the Psalmist puts his dissuade in **Psalm 92, verses 1 to 2**." | Psalms 92:1 | 0.95 | | ✅ |
| 870.6 s | "the Samus, put his dissuade in **Psalm 92, verse 12**, please be seated." | Psalms 92:12 | 0.95 | **Psalms 92:1** | ❌ |
| 879.5 s | "We will dance, we will praise and we will seek. **Psalms 92, verse 1 to 2**." | Psalms 92:1 | 0.95 | | ✅ |
| 2356.2 s | "joining my faith with the faith of our Father and the Lord, is taken from **4th Peter chapter 5 verse 10**." | Psalms 92:10 | 0.88 | **1 Peter 5:10** | ❌ |

**Five of nine correct. Four wrong verses reached the output.** SPEC's bar is a 5% wrong-verse
rate. This is 44%.

Rows 153 and 154 are confirmed against the bundled KJV, not inferred: *"Thou shalt increase my
greatness, and comfort me on every side"* is **Psalms 71:21**. Relay showed Psalms 89:27
(*"Also I will make him my firstborn"*) and then Psalms 89:7 (*"God is greatly to be feared"*).

---

## 2. The dial is not the lever, and this is the first time that can be shown with numbers

Service 15 ran at sensitivity **25**, which puts the auto-fire bar at **0.70**
(`from_sensitivity`: `lerp(0.90, 0.50, 0.5)`). It is the cautious half of the dial, and the
four wrong verses were at 0.88, 0.88, 0.95 and 0.88.

Wind the dial to its most cautious end, sensitivity 0, bar **0.90**, and replay the nine:

| bar | fires kept | correct | wrong | wrong-verse rate |
|---|---|---|---|---|
| 0.50 (default) | 9 | 5 | 4 | 44% |
| 0.70 (as run) | 9 | 5 | 4 | 44% |
| **0.90 (most cautious the dial goes)** | **5** | **4** | **1** | **20%** |

**The most cautious setting Relay offers still leaves a wrong verse on the screen**, because
Psalms 92:12 fired at 0.95. It also throws away Psalms 89:22, which was correct. This is
CLAUDE.md rule 10 restated by measurement rather than by argument: the confidence was never
wrong, it was a real parse confidence about a number nobody said, and no threshold can tell
those apart.

Note also which direction the evidence runs. Service 14 used the **looser** dial (0.50) and
got three of three. Service 15 used a **tighter** dial (0.70) and got five of nine. The
variable that changed was the model.

---

## 3. The model is an accuracy decision offered as a speed setting

The log for the second launch carries two model loads:

```
stt: model loaded from …/models/ggml-large-v3-turbo.bin
stt: model loaded from …/models/ggml-base.bin
```

`app_settings.stt.model` finished the day as `ggml-base.bin`, and RSS falling 2.25 GB to
984 MB confirms turbo was unloaded rather than kept alongside.

| | `turbo` (service 14) | `base` (service 15) |
|---|---|---|
| `stt_decode` p50 | 682 ms | **124 ms** |
| `stt_decode` p95 | 2686 ms | 1060 ms |
| `transcript_cadence` p50 | 968 ms | **206 ms** |
| updates per second | ~1.03 | **~4.85** |
| auto-fires correct | **3 of 3** | **5 of 9** |

The speed side of that table was already known. `PERF-MODELS-2026-08-30.md` measured decode
cost per model, and `FIELD-2026-08-30.md` F-5 priced turbo in a room for the first time. The
accuracy side had never been measured at all, and Relay's own in-product advice only points
one way. When a decode overruns its budget the app prints:

> *"Switch to a smaller model in Settings → Speech (measured on an M4 Pro with Metal:
> large-v3-turbo ~602ms, small ~153ms, base ~59ms per window)."*

Three millisecond figures, no accuracy figure, and an operator who follows it lands on the
configuration that produced four wrong verses. **That advice is the part of this finding that
is straightforwardly a defect**, and it is cheaper to fix than the model question behind it.

### Two mechanisms, and neither is a threshold

**Misheard numbers.** `base` heard *"1 to 2"* as *"12"* (row 158) and *"71 verse 21"* as
*"21 verse 27, 21"* (rows 153 and 154). The parser then did exactly its job on the words it
was given. A deterministic parser is confidently right about a number nobody said, which is
the same shape as rule 30's stop-word defect and cannot be reached by moving a bar.

**A faster cadence weakens corroboration rather than leaving it neutral.** Rows 153 and 154
fired **different verses 1.1 seconds apart from one utterance**. DECISIONS §36 holds a
reference from a partial window at `Suggest` until a second pass agrees, and that rests on the
two passes failing independently. At a 206 ms cadence consecutive passes see nearly the same
audio and repeat the same mishearing, so agreement is cheap. CLAUDE.md rule 34 forbids trading
safety for speed deliberately. It does not cover a safety margin shrinking as a side effect of
a faster cadence, and that is what happened here.

### What this audit deliberately does not claim

One service is one sample per model, and the two samples are of different services as well as
different models. This is not a measured accuracy ranking of Whisper models. It is evidence
that the choice has an accuracy cost large enough to break SPEC's bar, taken on one church's
audio, and the instrument to settle it properly now exists: see §6.

---

## 4. F-8, the wrong verse that is fixed

Row 161 is the one worth a code change rather than a measurement. The preacher said *"is taken
from **4th Peter chapter 5 verse 10**"*, which is 1 Peter 5:10 misheard into a book that does
not exist. What followed:

1. `detect_direct` produced nothing, because "4th Peter" is not a book.
2. `anchor_for_bare_verses` was therefore `None`.
3. `detect_bare_verses` saw `10`.
4. That bare `10` was resolved against `ContextMemory`, which still held **Psalms 92** from
   ten minutes earlier.
5. **Psalms 92:10 auto-fired at 0.88, unattended.**

**The first reading of this was wrong and is recorded rather than quietly replaced.** The
first hypothesis was "an unparseable book name", and a probe killed it: `book_named` is
**false** for this sentence, because bare "peter" is not an alias at all (only "1 peter",
"first peter", "i peter", "1peter"). Had that hypothesis gone straight into a fix, the fix
would have been gated on a predicate that is false in the failing case, and it would have
passed a test while changing nothing. That is precisely the failure CLAUDE.md rule 40 records
against the first F-1 diagnosis, and the only reason it did not happen twice is that the probe
was run before the edit.

**The real mechanism is narrower.** The bare-verse path took the verse number and ignored a
chapter the window stated out loud. The sentence said chapter 5. Memory said chapter 92. Relay
believed memory. This is F-1's own repair meeting the case it did not cover: F-1 established
that a book named in this breath beats memory, and *"the words do not say"* turns out not to
mean the same thing as *"the words did not parse"*.

Nothing else was going to catch it. `detect_passage_nav` handles "chapter 5 verse 10" with no
book named, and correctly, but it caps at 8 tokens on purpose so that sermon prose full of
numbers cannot trigger a jump. This sentence is 9 tokens after normalisation.

**The fix declines rather than correcting.** `detection::resolve_bare_verse_for_window` now
owns the whole decision as one pure function: a reference parsed from this window wins, a
stated chapter with no parsed reference resolves to nothing, and memory answers only when the
words do not say. Pairing the heard chapter with the remembered book would have shown
Psalms 5:10 instead, which is a different wrong verse and rule 10's lesson again.

Verified the way rule 40 says the first F-1 diagnosis was not. With the check removed:

```
assertion `left == right` failed: FIELD F-8 reproduced
  left: Some(VerseRef { book: "Psalms", chapter: 92, verse: 10 })
 right: None
```

That is the verse that reached the output, reproduced by the test that guards it. Filed as
RG-115, closed.

---

## 5. What held, and one of these matters more than it looks

**Zero panics, zero errors, zero dropped audio** across 121 minutes of capture in two
services. `latency::note_dropped_audio` never fired, so no queue on the audio path shed
anything: rule 33's bounded queues were never under pressure, even at a 206 ms cadence.

**Rule 10's paraphrase cap held through an entire choir set, and the transcript did not.**
Whisper hallucinated on the music, which is its known non-speech failure:

```
stt[en]: It is fantastic if you send us a message. Watch my videos.
stt[en]: God bless you, God bless you.
stt[en]: ip music
```

The first is YouTube outro boilerplate straight out of training data. "ip music" is a
collapsed `[Hip music]` tag. **Zero detections came out of the entire set.** No book token
appeared in any hallucination, so nothing was there to parse, and sung scripture can only ever
reach the operator as a suggestion. This is the closest thing the project has to a test of the
"hymn number three sixteen" class in a real room, and it passed. It is also not proof: the
protection that mattered here was that no hallucination happened to contain a book name.

**The console-only setup is a real configuration and worked.** The program pane renders
through `TemplateRender.svelte` and resolves templates through `resolveOutputTemplate`, the
same two paths a projector uses, so the operator was watching the wall rather than a
representation of it.

---

## 6. The recording, which is the day's most useful output

`RELAY_RECORD_WAV` was set for both launches. The surviving artefact is
`service-20260906-112459-seg01-5127.9s.wav`: **85.5 minutes** of this church's own desk feed,
939 MB, 48 kHz mono 32-bit float, RIFF header verified field by field (`riff size` 984556836
against a file of 984556844; `data` 984556800 against 984556800).

This is the first audio this project has ever had from a real service, and it is what turns
RG-116 from an argument into a measurement. Replaying it through `base`, `small` and `turbo`
with `RELAY_BENCH_WAV` and scoring **through the router** rather than by reading the
transcript (rule 13) gives a per-model wrong-verse rate on real church audio. It still cannot
give word error rate, and it is one room, one preacher, one language.

**It very nearly did not exist, twice**, and both near-misses are findings: F-11 and F-10.

---

## 7. Findings

| # | Finding | Status | Evidence |
|---|---|---|---|
| **F-8** | A chapter the window stated out loud was answered from memory, and **Psalms 92:10** auto-fired at 0.88 for a spoken 1 Peter 5:10. Second wrong verse on a real output in two field services | **FIXED**, RG-115, held by a test that reproduces it when reverted | §4 |
| **F-9** | The STT model is an accuracy trade the product offers as a speed setting: **3 of 3 on `turbo`, 5 of 9 on `base`**, and the most cautious dial setting still keeps a wrong verse. The in-product warning names three millisecond figures and no accuracy cost | **OPEN**, RG-116 | §2, §3 |
| **F-10** | A runtime audio-stream error is one line on stderr. `audio.rs:592` is the whole handler; it sets no stop flag, emits no `audio://error`, and reaches no operator. Observed when the desk feed was unplugged. The transcript stops mid-sermon with no banner, and because the recorder writes after the capture loop exits, a device drop destroys the recording | **OPEN**, RG-117 | §6, log |
| **F-11** | `RELAY_RECORD_WAV` names one path and the capture thread **truncates it on every Stop**. A second Start/Stop cycle destroyed **1837.8 s** of service 14 and left a valid-looking **170.0 s** file in its place, verified at 32,647,724 bytes. CLAUDE.md documents the force-quit hazard and not this one, and this one is worse because it looks like it worked | **OPEN**, in RG-117. Worked around outside the app by archiving each segment on its write line | §6 |
| **F-12** | `Main screen` was **lost and recovered three times** in service 15, with gaps of 0.1 s, **641.0 s** and **519.0 s**. That is **19.3 minutes, 22.6% of an 85.5 minute service**, during which no output was reporting that it was painting. Service 14 had one 2.0 s pair. This is `FIELD-2026-08-30.md` F-7 recurring and far worse | **OPEN**, not yet filed | `service_events`, services 14 and 15 |
| **F-13** | `end_to_end_speech_to_scripture` stamped **0 samples in service 14** despite three auto-fires, and 7 in service 15 despite nine. The stage is unreliable, not merely sparse. Its service 15 figures, **p50 2282 ms / p99 3855 ms**, are the first credible end-to-end numbers this project has, and they do **not** reproduce F-6's 39 to 90 s | **OPEN**, not yet filed. Does not close F-6: a different model and a different service | `perf_samples` |
| **F-14** | **`transcript_cadence` counts silence, and it is a hole in rule 31's guarantee rather than a regression of it.** `latency::transcript_emitted` admits a gap only when the previous pass was **not** final (`last_partial_was_final`), so *"the gap after a closed utterance is silence"* holds. A silence **inside an unclosed utterance** is admitted, because the guard cannot see it. **Confirmed**, see §9 | **OPEN**, RG-118 | §9, `latency.rs:614-621`, `perf_samples` |
| **F-15** | The audio input device is **not persisted**. `capture.js:62` holds `inputDevice` in memory only, so every launch silently reverts to the system default. Both launches today started on `MacBook Pro Microphone` with the Blackmagic desk feed plugged in and selected the previous session. Nothing tells the operator it reset | **OPEN**, not yet filed | `capture.js:62`, both launch logs |
| **F-16** | **No log line names the input device.** `audio: capture @ 48000 Hz · denoise on (RNNoise)` is all there is, and the laptop microphone and the Blackmagic both run at 48 kHz, so no instrument in this audit could confirm which input a service used. It is confirmed for today only because the operator said so | **OPEN**, not yet filed. One `println!` closes it | log |

**F-8 is a defect and is fixed. F-9 is a decision, not a defect, and it needs a human.**
F-10, F-11, F-15 and F-16 are all the same shape: the audio path fails or changes silently.
F-13 and F-14 are defects in the instruments, which is the most repeated category in this
project's history.

---

## 8. What this changes

**Changed.** Field validation has a second and third data point, and one of them is bad
enough to matter: a supervised pilot on `ggml-base` at default settings would have put four
wrong verses in front of a congregation in 85 minutes. The pilot decision in
`docs/RELAY_V1_AUDIT.md` was taken on evidence from a single service on `turbo`, and §2 of
this document shows the sensitivity dial cannot compensate.

**Not changed.** The GO for a supervised pilot and the NO-GO for general release both stand,
and F-8 is fixed. Nothing here argues for stopping the pilot; it argues that **the pilot must
pin the model**, which nothing currently does, and that a named operator watching every
service is doing more work than the previous audit implied.

**Not measured, still.** Word error rate in any language. Recall, meaning references spoken
and never noticed. Anything about a projector, because nothing reached one today.

**Owed.** F-12 through F-16 are now filed as RG-118 to RG-122. The model replay in §6 is the single
highest-value piece of work this audit produces, and it is the only one that can settle F-9.

---

## 9. F-14, verified after the fact, and it is the instruments again

F-14 was filed above as a candidate and is now confirmed. The verification is worth recording
because the answer was the opposite of the obvious one and took two steps to reach.

`transcript_cadence` worst read **34829 ms** in service 15. Rule 31's fourth guarantee is that
the gap after a closed utterance is silence and not cadence, so the reflex reading was a
regression of that guarantee. It is not. The guard is present and correct
(`latency::transcript_emitted`):

```rust
// Only WITHIN an utterance. See `last_partial_was_final`.
if let (Some(prev), false) = (g.last_partial_us, g.last_partial_was_final) {
```

Every `stt:` line in the launch log carries `gap_since_last_emit` and `final`, so the guard's
own decision can be replayed against the run. The five gaps at or above 20 s:

| gap | previous pass was final | counted as cadence |
|---|---|---|
| 389122 ms | yes | no |
| 121714 ms | yes | no |
| 76276 ms | yes | no |
| **43480 ms** | **no** | **YES** |
| 30692 ms | yes | no |

**The guard excluded the four largest and admitted the fourth-largest.** 12663 samples were
recorded and 71 were excluded, so it is doing its job on the case it was written for.

The admitted sample is silence, not a stall, and this is the second step. It carries
`decode=3ms` on a `window=600ms`, and across the gap `voiced` does not move at all
(19115 to 19115) while `silent` increments. The decoder was idle with nothing to decode. A
600 ms window on the far side also says the utterance buffer had restarted. Had `voiced` climbed
across the gap this would have been a 43-second pipeline stall and a far more serious finding,
which is why the check was worth making rather than assuming.

**So the rule needs widening, not repairing.** *"The gap after a closed utterance is silence"*
should be *"a gap with no voiced audio in it is silence"*. The practical cost today is small
and specific: `worst` and `p99` for the one metric an operator reads as *is it keeping up* are
inflated by pauses in speech. At 12663 samples the p50 of 206 ms cannot be moved by a handful
of leaked gaps, so **every median in §3 stands**, and only the tails are affected.

This is the most repeated category in this project's history. Of the nine findings in this
audit, **three are defects in the instruments** (F-13, F-14, F-16) and one is a defect in the
advice the product gives (part of F-9). CLAUDE.md's own summary of the 2026-09-05 sweep says
it: *the instrument is wrong more often than the code it audits.*

---

<!-- ===== was docs/qa/audits/FIELD.md, merged 2026-09-21, verbatim ===== -->

# FIELD-2026-09-13 — the first service with no wrong verse

**Thanksgiving Sunday: music, scriptures called, no sermon.** One service, 110.5 minutes,
watched live from the first minute rather than reconstructed afterwards. **Eight auto-fires,
eight correct**, on `ggml-large-v3-turbo` pinned before the microphone was opened.

Everything here was recorded by Relay itself and read back from
`~/Library/Application Support/com.relay.app/relay.db` and the process log. Numbers are quoted
with the query or the log line that produced them. Where this audit cannot settle something it
says so rather than choosing the flattering reading, and §6 is a correction to an existing
register entry that today's evidence puts in doubt.

> **On quoting the preacher.** Same rule as `FIELD-2026-08-30.md` §0 and `FIELD-2026-09-06.md`:
> every quotation below is a fragment that names a scripture reference, and it is here because
> it is the evidence. Sentences carrying no reference are described, not quoted.

---

## 0. Method, and what it cannot see

| | |
|---|---|
| Date | 2026-09-13 |
| Service | id **16**, "Sunday Service", 496 transcripts, 110.5 minutes of audio |
| Build under test | **dev profile**, `cargo run --no-default-features`, branch `fix/marginal-notes-still-reaching-the-wall` at `1530a4b`. `Finished \`dev\` profile [unoptimized + debuginfo]` |
| Model | **`ggml-large-v3-turbo`**, set in `app_settings.stt.model` before capture started and not changed at any point |
| Sensitivity | default. `app_settings` holds no sensitivity key, so `Thresholds::default() == from_sensitivity(50)` and the auto-fire bar is **0.50** |
| Input | the machine's configured default input. **Which physical device is not recorded anywhere** — RG-122, unchanged |
| Output | a screen the operator connected, reported by Relay as the **Streaming** channel. See §5: it was lost at 3245.4 s and never recovered |
| Recording | `RELAY_RECORD_WAV`, `RELAY_STT_TIMING=1`, `RELAY_AUDIO_RMS=1`. **6631.7 s written and verified** |
| Instruments | `detections`, `perf_samples`, `service_events`, the process log, and a 110.5 minute WAV |

**What no instrument here could see:** whether the congregation could read the screen, what the
room sounded like, how many references were spoken that Relay never noticed, and word error
rate, which still has never been measured in any language. This audit adds 110.5 minutes to the
corpus that could settle the last of those and does not settle it.

**One difference from every previous field audit.** This is a **debug build**, not a packaged
one. **An earlier revision of this table said decode costs were therefore not comparable with
`PERF-MODELS-2026-08-30.md` or with either earlier field audit, and that was an assumption, not
a measurement. It has since been measured and it is false** — see §4: the same audio through the
same rig costs 1169 ms at p50 in debug and 1188 ms in release, because `whisper.cpp` is built by
cmake with its own optimisation and never carried the Rust profile's penalty. The claim is
corrected here rather than deleted, because it shaped the first reading of §4 and a reader
should be able to see that.

---

## 1. What reached the output

Eight auto-fires. Times are seconds from service start.

| t | heard | fired | conf | |
|---|---|---|---|---|
| 2008.3 s | "Your blessings will be secured in the name of Jesus Christ. I'm going to read from **Psalm 27 verse 1 to 2**." | Psalms 27:1 | 0.95 | ✅ |
| 2043.0 s | "We are starting with that scripture. **Psalm 100 verse 4**. We gotta end." | Psalms 100:4 | 0.95 | ✅ |
| 2090.3 s | "The Bible says in **Psalm 67 from verse 5**. Hallelujah. Let the people pray." | Psalms 67:5 | 0.88 | ✅ |
| 2946.3 s | "I don't have too much time. **Hebrews 11 verse 1** says," | Hebrews 11:1 | 0.95 | ✅ |
| 2984.4 s | "Amen. Praise the Lord. Let's just look at **Hebrews 11, 19** quickly." | Hebrews 11:19 | **0.55** | ✅ |
| 3003.0 s | "Why did he go? The Bible explains to us in **verse 19**. It says, account" | Hebrews 11:19 | 0.88 | ✅ |
| 3016.4 s | "Let's read it in the TPT. He kind of explained it better there. **TPT says verse 19**." | Hebrews 11:19 | 0.88 | ✅ |
| 3025.4 s | "Now let's start from **verse 17**. The faith operated powerfully in Abraham for when it was" | Hebrews 11:17 | 0.88 | ✅ |

**Eight of eight correct. Zero wrong verses.** Plus **eight manual navigations** between
3089.2 s and 3097.1 s, all persisted as `manual/1.00`, none as `auto` — CLAUDE.md rule 14 held,
which matters because the self-calibrating router learns from that column.

This is the first field service with no wrong verse. `FIELD-2026-08-30.md` put one in front of
a congregation; `FIELD-2026-09-06.md` produced four. **One clean service is not a rate**, and
this one had eight fires against that morning's twelve, in a service with no sermon. It is
evidence, not a result.

### The three things worth reading off this table

**The model was pinned before the microphone opened, and this is the first service where that
was true.** RG-116 records that the model is an accuracy decision offered to operators as a
speed setting, and that nothing in Relay pins it. Nothing pins it today either. It was set by
hand, outside the application, by writing `app_settings`. The register entry stands.

**0.55 is the thinnest margin any correct fire has been recorded at.** "Hebrews 11, 19" is the
comma form, and `is_ref_connector` (`detection.rs:1690`) accepts `and`, `,` and `&` only when a
**verse word** follows — "19" is a bare number, so this did not take the clean chapter-verse
route. It landed correctly at 0.05 above the bar. Compare `FIELD-2026-08-30.md`, where "Joshua
1.9" and "Genesis 4 2 to 7a" also came in at 0.55 and were also correct. Three correct fires at
0.55 across two services is now a small but consistent signal that the 0.50 bar is not
obviously too low. It is not evidence that it is high enough.

**Four of the eight resolved through the bare-verse path at a hardcoded 0.88** — rows 3, 6, 7
and 8. That is RG-32, open on purpose: the label `Direct` is a lie on this path, because Relay
inferred the book rather than hearing it. All four were correct today, and one of them
(row 3, "Psalm 67 from verse 5") had the book and chapter in the same breath. RG-32's row asks
for "a second and third Sunday" before spending an operator click on every in-passage "verse
eighteen". **This is that second Sunday, and it argues for leaving RG-32 open**: the path was
right eight times out of eight across two consecutive references, including the F-8 shape that
broke it a week ago.

### The F-8 repair, exercised in a room

Rows 6, 7 and 8 are the path that produced the wrong verse on 2026-09-06. The preacher said
"verse 19", "verse 19" and "verse 17" with no book and no chapter spoken, and each resolved
against Hebrews 11 — the chapter the window itself had established, not something fired
earlier. `detection::resolve_bare_verse_for_window` is the one place that decides this, and it
decided correctly three times.

This is the first time RG-115's fix has been exercised by a preacher rather than by a test.

---

## 2. A translation the preacher named, which Relay does not have and never mentioned

At 3016.4 s: *"Let's read it in the **TPT**. He kind of explained it better there. **TPT says
verse 19**."*

Relay fired `Hebrews 11:19` at 0.88 — the right reference — from the **King James Version**,
the only row in `translations`:

```
sqlite> select * from translations;
1|King James Version|KJV|en|public domain
```

The preacher then read The Passion Translation aloud while the screen showed King James. The
tail of the next window is TPT wording — *"The faith operated powerfully in Abraham for when it
was"* — against the KJV's *"By faith Abraham, when he was tried, offered up Isaac"*, which is
how this was identified rather than assumed.

**Nothing anywhere told the operator this had happened.** The fire looked identical to the
seven around it: same `direct` method, same 0.88, same badge. An operator watching the AI
Detection panel had no signal that a translation was named in the audio and silently ignored.

This is the **rule 35 shape** — a surface that cannot distinguish its success case from a
failure case — and it is filed as **RG-135**. It is not the same as RG-50, which is about an
operator being unable to *add* a translation. This is about Relay being unable to *say* that
the one it used was not the one that was asked for.

Two things this audit deliberately does not claim. It does not claim a wrong verse: the
reference was right and the words were scripture. And it does not propose shipping TPT, which
is a licensed translation and a commercial decision, not a repair.

---

## 3. The screen that went away and stayed away

`service_events` for service 16, in full — three rows, and the shape of them is the finding:

| seq | at | kind | detail |
|---|---|---|---|
| 1 | 0.0 s | `service_started` | Sunday Service |
| 2 | **3207296.6 ms** | `output_recovered` | Streaming · screen's own clock: silent 2s, never hidden |
| 3 | **3245372.7 ms** | `output_lost` | Streaming |

**An `output_recovered` with no `output_lost` before it.** The timeline records a screen coming
back from a loss it never recorded. Whatever took the Streaming output away the first time
produced no event, so the timeline understates the outage count by at least one — and the
timeline is the instrument the Sunday report and replay are both built on.

**And the last event of the service is a loss that never recovers.** The Streaming output went
at 3245.4 s of a 6631.7 s service. Nothing recovered it. **The screen the operator had
connected was dead for the final 56.5 minutes**, and the only reason that did not put anything
in front of a congregation is that no scripture was called after 3097.1 s.

This is RG-119 again — the main output lost and recovered three times in 85.5 minutes on
2026-09-06 — but with a worse ending and a missing event. RG-119 asked whether the screen stops
painting or the heartbeat stops arriving. Today added a third question: why one of the two
transitions produced no row at all. Filed as **RG-136**, and **answered and closed after this
audit was written** — the paragraph below is the finding, and it is left standing because the
mechanism is worth reading even now it is fixed.

**Root cause: the edge was consumed even though it could not be recorded.**
`OutputHealth::transition` advances `reported` whether or not anything writes the event down,
and `log_event` is a silent no-op with no service running — its whole body sits inside
`if let Some(st) = sess.as_ref()`. A screen already dead before the operator pressed record had
its `output_lost` computed, thrown away, and marked as reported. The matching recovery then
landed inside the service with no partner. The status poll's body is now `record_output_edges`,
which **forgets** an edge outside a service rather than consuming it, and
`e2e::r136_a_recovery_in_the_record_always_has_a_loss_to_recover_from` reproduces the field
artefact exactly against the pre-fix body: `["service_started", "output_recovered"]`.

**The unrecovered final loss is not a defect.** The screen died at 3245.4 s and stayed dead, and
the record says so correctly. It is an operational fact about this service, not a bug, and the
fix above does not touch it.

The detail string on the recovery is worth keeping: *"screen's own clock: silent 2s, never
hidden"*. The output page was not backgrounded by the operating system — it went quiet while
visible, which is the case `OutputHealth` exists to catch, and it caught it.

---

## 4. Latency, in a debug build, with a tail nobody should read past

Final `perf_samples` row for service 16. **Every figure here is a debug build** and is quoted
for its shape, not as a number to compare against a shipped one.

| metric | samples | p50 | p95 | p99 | worst |
|---|---|---|---|---|---|
| `stt_decode` | 4046 | 1161 ms | 2692 ms | 3339 ms | **9326 ms** |
| `transcript_cadence` | 3936 | 1322 ms | 3344 ms | 5441 ms | **20978 ms** |
| `audio_to_partial_transcript` | 4046 | 2602 ms | 4694 ms | 5647 ms | **28656 ms** |
| `audio_to_visible_transcript` | 4040 | 2611 ms | 4756 ms | 6074 ms | **28650 ms** |
| `transcript_to_reference_detection` | 1568 | 40 ms | 90 ms | 101 ms | 115 ms |
| `reference_detection_to_fire` | 8 | 9 ms | 20 ms | 20 ms | 19 ms |
| `end_to_end_speech_to_scripture` | 8 | 1521 ms | 1578 ms | 1578 ms | 1578 ms |

**The good news is the bottom three rows.** Detection costs 40 ms at the median against a
2.6 ms benchmark claim that only ever covered the semantic scan, the router costs 9 ms, and
**microphone to projector is 1521 ms at the median with a worst case of 1578 ms** — a tight
distribution over eight fires. Nothing on the fire path is slow. Rule 31's warning that the
reflex answer "STT is slow" is usually half wrong holds again: STT is the entire delay here,
and detection and routing together are 49 ms of 1521 ms.

**The bad news is the tail, and it is a 28.6 second tail.** `audio_to_visible_transcript` is
the honest end-to-end number by rule 31's own definition — the clock starts at the oldest audio
still waiting — and its worst case is **28650 ms**. For 28 seconds at some point in this
service, the oldest unprocessed audio had been waiting nearly half a minute. A reference spoken
in that window would have reached the wall half a minute late, or not at all.

The 21 s `transcript_cadence` worst matches what the process log shows independently
(`grep -oE 'LAG=[0-9]+'` over 4416 passes: median 1287 ms, p90 2628 ms, **max 20977 ms**), so
the two instruments agree and this is not a stamping artefact.

**This audit's first reading was that the music caused it. That reading is wrong, and it was
disproved after the audit was written rather than left standing.**

The original paragraph argued from `drained=10` to `drained=16` per pass during worship against
`drained=1` during speech, and concluded that dense full-band audio in a debug build explained
the tail. Two measurements killed it.

**First, the event is not in the music.** `worst_ms` only ever increases, so the minute in which
each maximum was set can be read straight out of `perf_samples`. Three instruments put it in one
120-second window **at around 100 minutes**, an hour after the last scripture was called and
long after worship:

| metric | jumped | at |
|---|---|---|
| `stt_decode` | 3870 → **9326 ms** | t = 6002 s |
| `audio_to_visible_transcript` | 13818 → **28650 ms** | t = 6062 s |
| `transcript_cadence` | 11961 → **20978 ms** | t = 6062 s |

One decode of **9326 ms** against a 1161 ms median, and everything downstream inherits it.

**Second, that audio reproduces nothing.** The window was cut out (5900-6100 s), converted to
16 kHz mono float, and replayed through `stt::realtime::live_transcript_latency` in a **release**
build on the same model at wall-clock pace:

| | live service | release rig, same 200 s |
|---|---|---|
| `stt_decode` p50 | 1161 ms | **1188 ms** |
| `stt_decode` worst | **9326 ms** | **1264 ms** |
| `audio_to_visible` p50 | 2602 ms | **2289 ms** |
| `audio_to_visible` worst | **28650 ms** | **2461 ms** |
| dropped partials | 0 | 0 |

**The medians match to within 27 ms and only the tail disappears.** Steady-state decode cost is
the same, so the tail is not compute, not the model and not the content. Something
intermittently stalled the running application.

**Third, the build profile is not the variable.** Two things separated the columns above — the
release profile, and everything the rig does not have. The same slice was therefore replayed
again through a **debug** rig, and the profile half is settled:

| | debug rig | release rig |
|---|---|---|
| `stt_decode` p50 | 1169 ms | 1188 ms |
| `stt_decode` worst | 1328 ms | 1264 ms |
| `audio_to_visible` p50 | 2256 ms | 2289 ms |
| `audio_to_visible` worst | 2706 ms | 2461 ms |
| updates/s | 0.83 | 0.82 |

**Indistinguishable, and debug is marginally faster.** `whisper.cpp` is compiled by cmake with
its own optimisation settings and never carried the Rust profile's penalty, so the decode was
never debug-penalised at all. This audit's own method table asserted the opposite and has been
corrected.

**What is left, and the audit will not guess at it.** One variable remains: everything the rig
does not have — the capture thread, the webview, the kiosk hub, SQLite writes, the detect
thread, and a 110 minute uptime rather than 200 seconds. A single 9326 ms decode inside an
otherwise flat distribution is the shape of a stall, not of slow compute. RG-137 stays open with
that as its remaining question, and with three things now ruled out by measurement rather than
argument: the model, the audio, and the build profile.

One incidental observation from cutting the slice, recorded because it was not looked for: the
window peaks at **1.0163**, so the input was clipping at that point in the service. Nothing here
connects it to the tail, and it is written down rather than built upon.

**Zero dropped audio across the whole service** — no `note_dropped_audio` in 110.5 minutes, so
the bounded queues of rule 33 never shed. A tail this long with no shedding means the backlog
was absorbed in the decode, not at a queue.

---

## 5. What did not go wrong

Stated because a field audit that only lists faults stops being a measurement.

- **Zero panics.** No `panicked`, in 110.5 minutes, in a build with `debug_assertions` on.
- **Zero preflight refusals.** `pipeline::preflight` refused no broadcast.
- **Zero panic-control failures.** No `clear failed`, no `spoken nav failed`.
- **Zero dropped audio.** Both bounded queues held.
- **Zero language-instability warnings.** `stt://language_unstable` never fired in a service
  whose speech was English with Yorùbá and pidgin mixed through it.
- **`end_to_end_speech_to_scripture` recorded 8 samples against 8 auto-fires.** RG-120 records
  this stage stamping 0 against three fires and 7 against nine. **Today it was 8 of 8.** That
  does not close RG-120 — one clean service does not establish a flaky stage is fixed, and
  nothing was changed — but it is the first sample where the stage and the fire count agree,
  and it belongs in that row.

---

## 6. A correction owed to RG-117, which this audit cannot finish

RG-117 says a runtime audio-stream error means *"the debug recording is lost"*, and that
`RELAY_RECORD_WAV` *"writes after the loop exits, so a device drop means the recording is never
written"*.

Today the input device disappeared mid-service:

```
audio stream error: The requested device is no longer available. For example, it has been unplugged.
audio: wrote 6631.7s to /Users/mrgee/relay-service-audio/service-2026-09-13.wav
```

**The recording was written.** 1273286444 bytes, verified with `afinfo` as
`WAVE, 1 ch, 48000 Hz, Float32, 6631.700000 sec` — the hand-rolled RIFF header read back
correctly, which is the check that matters, since a single wrong field yields a file that opens
as noise.

`err_fn` has not changed: `audio.rs:592` is still
`let err_fn = |e| eprintln!("audio stream error: {e}");`, and `git log --since=2026-09-06 --
src-tauri/src/audio.rs` is empty. **So the code that RG-117 describes is the code that ran.**

**What this audit cannot establish is why the capture loop exited.** RG-117 itself records that
on 2026-09-06 *"only a deliberate Stop released it"*, and the operator was at the console today
and had been told the microphone was dead. Two readings fit the evidence:

1. The operator pressed Stop on being told, the loop exited normally, and RG-117 is intact and
   simply describes a hazard that a watching operator avoided.
2. The loop exited by itself on this failure mode, and RG-117's absolute claim is too strong.

The log cannot separate them: the capture loop prints nothing while spinning on
`Timeout => continue`, so the adjacency of the two lines proves nothing about elapsed time
between them. **RG-117 is therefore left open and unchanged, with this audit named in it as
evidence that needs one question answered by the operator**, rather than edited on a reading
this audit cannot support. If the answer is (1), nothing changes. If it is (2), the impact
column needs rewriting and a test becomes possible.

**The related hazard in RG-117's solution column did not bite and was worked around anyway.**
`RELAY_RECORD_WAV` names one path and the capture thread truncates it on every Stop, so a
second Start/Stop cycle destroys the first segment. The file was copied to
`service-2026-09-13.SAFE.wav` before the application was closed, for exactly that reason.

**What this is worth.** 110.5 minutes of real church audio at 48 kHz — music, congregational
response, and eight spoken scripture references with known correct answers. That is the largest
single sample the project has, and RG-116 names the measurement it enables. The audio is not in
this repository and must never be: it is a recording of a congregation.

---

## 7. What this audit does not change

**Not the release decision.** `docs/RELAY_V1_AUDIT.md` owns it and it does not move: NO-GO for
general release, GO for a supervised pilot. Nothing here touches the reasons — no code-signing
certificate on either platform (RG-73), word error rate unmeasured in every language, and one
operator who is also the author.

**Not the model question.** RG-116 stays open. Today is one service on one model with no
comparison arm, run by someone who knew which model to pin and pinned it by editing the
database. A church cannot do that, and nothing in Relay does it for them.

**Not RG-32.** The bare-verse path was right eight times out of eight today, which is an
argument for patience, not for closing it. The label is still `Direct` and is still a lie.

**Not word error rate.** It is still unmeasured. There is now 196 minutes of church audio to
measure it against.

---

## 8. New register entries

| id | one line |
|---|---|
| **RG-135** | A translation the preacher names and Relay does not have is invisible: the fire looks identical to a matched one |
| **RG-136** | `output_recovered` was stamped with no matching `output_lost`, and the service ended on a loss that never recovered |
| **RG-137** | A 28.6 s worst case on `audio_to_visible_transcript`, unexplained, in a debug build, with a replay rig and the audio to drive it |

---

<!-- ===== was docs/qa/audits/FIELD.md, merged 2026-09-21, verbatim ===== -->

# FIELD-2026-09-20 — the fifth service, and two wrong verses ten minutes apart

**A full Sunday service with a sermon.** One service, 93.9 minutes, run by the author,
reconstructed afterwards from what Relay recorded rather than watched live. **Twenty auto-fires,
eighteen correct, two wrong**, on `ggml-large-v3-turbo` pinned before the microphone opened and
the recognition language pinned to English. Both wrong verses reached a congregation-facing
screen unattended. One (RG-178) was found and fixed the same day. The other (RG-179) was found
the next day by the Phase 1 audit reading the database, and is the reason this file exists:
CLAUDE.md said "four live services" for a day after the fifth had put a verse nobody said on a
wall.

Everything here was read back from `~/Library/Application Support/com.relay.app/relay.db` with
`sqlite3 -readonly`. Numbers are quoted with the query that produced them. This audit was written
on 2026-09-21 and is frozen; closures go in the register, not here.

> **On quoting the preacher.** Same rule as every field audit before it: every quotation below is
> a fragment that names a scripture reference, and it is here because it is the evidence.
> Sentences carrying no reference are described, not quoted.

---

## 0. Method, and what it cannot see

| | |
|---|---|
| Date | 2026-09-20 |
| Service | id **24**, "Sunday Service", 147 transcripts, 93.9 minutes (`MAX(at_ms)` of its `service_events`) |
| Build under test | not recorded by Relay. The packaged `Relay.app` in `/Applications` was built at 21:38 that evening, **after** the service; the service ran an earlier build of branch `stage-timers-mobile`. Which commit is unknown, and that gap is itself a finding (S13 in the stage plan: no build marker in diagnostics) |
| Model | **`ggml-large-v3-turbo`**, `app_settings.stt.model`, set before capture and not changed |
| Language | **`en`**, pinned on the active voice profile (`voice_profiles.language`) |
| Sensitivity | default, so the auto-fire bar is **0.50** |
| Input | `audio.input_device = F998`; which physical device that is, Relay does not record (RG-122) |
| Outputs | four channels: **Main screen** (native, display 1), **Streaming** and **Lobby screen** (network clients, both `output_lost` at 0.1 min and never recovered: nothing was pointed at them), **STAGE MONITOR** (native, display 0) |
| Recording | not set. There is no WAV of this service; `heard_text` on each detection is the only transcript evidence |
| Instruments | `detections` joined to `transcripts` and `verses`, `service_events`, `perf_samples` |

Six more `services` rows carry the same date (ids 25–30): short re-runs after the service, 0 to
17 minutes each, one with 12 auto-fires in 17 minutes. They are testing, not the service, and are
not read here.

**What no instrument here could see:** which build ran; what the room sounded like; whether the
congregation could read the wall; word error rate, still unmeasured in every language; and,
because nothing was recorded, whether any of the eighteen correct fires was correct for the
right reason.

---

## 1. What reached the output

`SELECT d.id, d.confidence, d.status, v.book, v.chapter, v.verse, t.timestamp, d.heard_text FROM
detections d JOIN transcripts t ON d.transcript_id = t.id JOIN verses v ON v.id = d.verse_id
WHERE t.service_id = 24 AND d.status = 'auto' ORDER BY d.id;`

| min | Fired | Conf | Heard (fragment) | Verdict |
|---|---|---|---|---|
| 1.5 | Acts 4:31 | 0.95 | *"Acts chapter 4 verse 31"* | correct |
| 3.0 | Acts 4:31 | 0.95 | *"That's Acts chapter 4 verse 31"* | correct (repeat) |
| 6.3 | Numbers 10:29 | 0.88 | *"Numbers chapter 10. I'll read verse 29."* | correct |
| 6.3 | **Genesis 10:29** | 0.88 | *"Genesis 10, I'll read verse 29. It's the New King, New King James Version."* | **WRONG** — the same sentence decoded again 3.2 s later into another book. RG-178 |
| 17.1 | Psalms 55:22 | 0.55 | *"Psalm 55, 22"* | correct |
| 19.0 | Psalms 55:1 | 0.88 | *"…from verses one to three"* | correct: a bare verse against the psalm on the wall |
| 19.4 | Psalms 55:1 | 0.88 | *"Let's read together verse 1."* | correct (repeat) |
| 23.5 | **Psalms 55:1** | 0.88 | *"Out of a prophet called Osir. In verse 1 he says, Come and let us return unto the Lord."* | **WRONG** — Hosea 6:1. The book was misheard into a word no alias knows, nothing parsed, no chapter was stated, and memory answered with the psalm from four minutes earlier, labelled `Direct`. RG-179 |
| 32.5 | Romans 10:17 | 0.55 | *"Romans 10, 17 is in your Bible"* | correct |
| 33.3 | Romans 10:17 | 0.55 | *"Romans 10, 17, faith comes…"* | correct (repeat) |
| 35.9 | Luke 15:17 | 0.55 | *"Luke 15, 17, a day came"* | correct |
| 39.2 | Psalms 139:23 | 0.88 | *"David prays Psalm 139. Look at verse 23 and 24."* | correct |
| 39.4 | Psalms 139:23 | 0.95 | *"Psalm 139 verse 23 says, Search me, O God"* | correct (repeat) |
| 45.9 | Malachi 3:7 | 0.55 | (reference in the next fragment) | correct |
| 46.1 | Malachi 3:7 | 0.55 | *"Malachi 3, 7 Return unto me and I will return unto you"* | correct (repeat) |
| 52.2 | Joshua 24:15 | 0.95 | *"Joshua 24 verse 15"* | correct |
| 60.5 | Luke 22:42 | 0.55 | *"Luke 22 42"* | correct |
| 64.7 | John 12:24 | 0.55 | *"John 12, 24 is in your Bible"* | correct |
| 65.6 | Psalms 126:5 | 0.95 | *"Psalm 126, verse 5"* | correct |
| 81.0 | Hosea 6:1 | 0.55 | *"Hosea 6, 1"* | correct — the same passage the preacher had quoted at 23.5 min, now with the book said clearly |

Eleven manual fires: Numbers 10:29–10:32 walked by hand at 6.9–7.1 min (the operator correcting
RG-178 and reading on), Psalms 55:1 at 19.4 min, and five more.

### The three things worth reading off this table

**1. Two wrong verses, and no threshold separates either from the correct ones.** Both wrong
fires scored 0.88. Nine correct fires scored 0.55. The sensitivity dial is not the lever; it was
not on 2026-09-06 either (RG-116), and this is the third service to say so.

**2. The two wrong verses are different defects in the same window.** RG-178 is the decoder
reading one utterance two ways 3.2 seconds apart, both confidently, both `Direct`, and the
cooldown keyed per reference could not see it. RG-179 is rule 40's memory answer wearing a label
it had not earned: the resolution was right by all three of the rule's own tests, and `Direct`
means heard. Rule 40 had kept that label on purpose "until a second Sunday". This was the second
Sunday.

**3. The preacher named a translation Relay does not have, again.** *"New King James Version"*
at 6.3 min. Relay carries the KJV alone and said nothing (RG-135's shape; RG-50 is the ruling
nobody has made).

---

## 2. The screens

`SELECT seq, at_ms, kind, detail FROM service_events WHERE service_id = 24 ORDER BY seq;`

- **Streaming** and **Lobby screen** were `output_lost` at 0.1 min and never recovered. Nothing
  was connected to either; the events are honest and the room did not need them.
- **STAGE MONITOR** was lost and recovered **five times** between 59.8 and 93.9 min, each
  recovery reporting *screen's own clock: silent Ns, hidden Ns* with silent equal to hidden. The
  page was hidden, not disconnected: a covered or backgrounded window on display 0, which is the
  laptop. Whether a preacher was reading it is not recorded.
- The **Main screen** recorded no loss. On the projector path the service was continuous.

---

## 3. Latency

`SELECT metric, samples, p50_ms, p95_ms, worst_ms FROM perf_samples WHERE service_id = 24 ORDER BY at_ms DESC LIMIT 6;` (the last snapshot)

| Metric | Samples | p50 | p95 | worst |
|---|---|---|---|---|
| `stt_decode` | 4415 | 675 ms | 2662 ms | 3236 ms |
| `audio_to_visible_transcript` | 4410 | 1474 ms | 4325 ms | 5270 ms |
| `end_to_end_speech_to_scripture` | 19 | 1389 ms | 3384 ms | 3383 ms |
| `reference_detection_to_fire` | 20 | 4 ms | 7 ms | 8 ms |
| `transcript_cadence` | 4143 | 944 ms | 2941 ms | 12373 ms |

`turbo` at p50 675 ms is the figure `PERF-MODELS-2026-08-30.md` predicted; the p95 tail is where
the 2026-09-13 audit left it. Nothing here is new, and it is quoted so the next audit has a row
to compare against.

---

## 4. What this audit does not change

The release decision (NO-GO general, GO supervised pilot). Word error rate, unmeasured. The
model question: `turbo` produced eighteen of twenty correct, and both misses were decisions the
router made about text, not the model's transcription of it.

---

## 5. Register entries

- **RG-178** (Genesis 10:29): filed and closed 2026-09-20, `Router::decide` offers a second book
  at the same chapter and verse inside the cooldown rather than firing it.
- **RG-179** (Psalms 55:1 for Hosea 6:1): filed and closed 2026-09-21, a memory answer is
  `UncertainBook` and offered, never fired. DECISIONS §106.
- The translation named and not carried is RG-135's shape and waits on RG-50.
- The unknown build is the stage plan's S13, still open.

---
