# Decisions — Relay

Every decision below was made deliberately, with reasoning, in a brainstorm session. If you're an AI agent or a new contributor and something in the code contradicts this list, the code is wrong — flag it, don't silently "fix" the decision.

| Decision | Reasoning |
|---|---|
| No native SDI hardware output, ever (unless explicitly reopened) | High engineering cost (Blackmagic DeckLink-class SDK) for narrow reach. Anyone with SDI gear already owns hardware (ATEM, converter) that accepts NDI/HDMI and re-outputs SDI — interoperability is preserved without owning the SDI problem. |
| Core engine: Rust | Real-time audio/video/inference workloads suit Rust's performance and FFI story to C SDKs (NDI, whisper.cpp) better than a Node-based core. |
| Desktop shell: Tauri, not Electron | ~10–20x smaller install size, lower idle memory — concretely matters for the target market's modest hardware and unreliable power. |
| Output channels modeled as render targets of one shared template engine | Maximizes code reuse across preview/output/remote-screen use cases; enables ultra-low-cost output hardware (Raspberry Pi kiosk clients), which matters for the target market. |
| Local-first, hybrid STT (on-device model, optional cloud fallback) | Matches offline-reliability needs of the target market and keeps African-language model swapping architecturally simple — the model is a pluggable component, not baked into the pipeline. |
| ATEM's multiview/program-bus model rejected as the mental model | Structurally different problem — N independently-styled destinations fanned from one AI decision, not one program feed. Renamed internally as "output channels." |
| Priority STT languages, tier 1: Yoruba, Swahili, Hausa. Tier 2: Igbo. Parked: Zulu, Amharic, Twi, Shona, others | Ranked by (a) existing Whisper/community fine-tune coverage, (b) speaker population + church-market size, (c) frequency of English code-switching in real preaching. Swahili has the best existing Whisper coverage; Yoruba has the largest addressable church market; Hausa is large (~80M speakers) with growing dataset support. |
| Windows and macOS both, day one | Rust+Tauri makes this near-free — same core, same webview UI on both. Target market skews Windows on cost grounds but macOS exists in wealthier/urban churches. |
| Business model: free / open source, no tiers | User decision. Sustainability path (donations, grants, optional paid cloud add-on) not yet decided — parked, not blocking v1. |
| Confidence-threshold mechanism: self-calibrating per install, not one static global number | Accent, mic quality, and room noise vary too much across churches for a single global threshold to be right for most. Nudge thresholds per install using operator confirm/reject signal over the first few live services, always leave a manual override slider in Settings. **Amended:** the seed is now auto-fire ≥0.50 / suggest ≥0.35 (operator preference: push anything above ~50% straight to the screens), not the 0.90/0.60 originally logged here. There is exactly ONE baseline — `Thresholds::default()` is *defined as* `from_sensitivity(50)`, so the dial and the default cannot drift apart. They previously did, and a profile save silently reset the gate from one scale to the other, wiping the operator's calibration. |

## Build-out decisions (presentation suite)

Made while building the Library + Planner + output layer. Same rule: if the code contradicts these, flag it.

| Decision | Reasoning |
|---|---|
| One unified **cue model** for every content type (`plan_items.cue_type` + `payload_json`) | Scripture, song, media, announcement, and countdown all reduce to the same polymorphic cue, so the Planner, monitors, and one renderer never branch per type. Adding a content type is a new payload shape, not new plumbing. |
| **Snapshot vs. reference** per content type | Scripture live-resolves from its reference at fire time (always current text/translation). Songs and announcements snapshot into the cue for offline reliability, but edits **propagate** (`sync_*_in_plans`) so a Library edit is never stale in a plan. Best of both, no per-type special case at fire time. |
| **Arrangements** = named play-orders stored as section-index sequences; "Standard" implicit | ProPresenter-parity feature. Storing indices (not copied lyrics) keeps edits propagating; storing the sequence on the cue lets a lyric edit re-expand into the right (possibly repeated) slots. Standard is never persisted — it's just "all sections in order." |
| **Countdown ticks locally** in each output from a broadcast target epoch | Broadcasting every second would spam the WS hub and drift; broadcasting the target once and ticking client-side is offline-clean, sync-correct, and updates digits in place (no crossfade per tick, no reactive-loop freeze). |
| **Verse auto-fit** (measure + shrink) instead of fixed/length-bucketed sizing | Real live verses vary wildly; a heuristic clips or overflows. Measuring the box and shrinking guarantees scripture always fits at any output size. Font-size is set imperatively so it can't re-enter Svelte's scheduler and loop. |
| **FTS5** added *behind* the existing reference/phrase/semantic ranker, not replacing it | bm25 full-text catches loose, non-contiguous word queries a substring `LIKE` misses, but precise reference/phrase/semantic matches must still rank first. FTS is the recall tail, self-healing via an idempotent index-rebuild migration. |
| **Strip KJV translator glosses** at import, keep supplied-word italics | The bundled corpus brackets both marginal notes (`{…: Heb. …}`, not verse text) and supplied words (`{it was}`, real text). Drop the former, unbracket the latter — in code (versioned, re-runnable via migration), source data untouched. |
| **No native `confirm()`/`alert()`** anywhere | Tauri's webview doesn't implement JS dialogs (returns false) — they silently break actions. All confirmations are in-app two-step ("arm → confirm"). |
| **Per-content-type templates** carried as a `template_json` override on the cue | Lyrics should look like lyrics and scripture like scripture without a per-channel branch. The override is just data on the broadcast; the one renderer honors it, else the channel template. |
| Console migrated to the global `--v-*` design tokens | The console had a private palette; unifying to the shared tokens keeps one design system across every surface. |

## Live-safety decisions (hardening pass)

Made while hardening for the first real service. Same rule: if the code contradicts these, flag it.

| Decision | Reasoning |
|---|---|
| **Only `DetectionMethod::Direct` may ever auto-fire.** Semantic (paraphrase) and Ambiguous candidates are capped at `Suggest`, at any confidence, at any sensitivity | A TF-IDF cosine is a distance in an arbitrary vector space, **not a calibrated probability** — gating it with a numeric threshold is gating it against noise. A sermon window sharing two or three rare words with some verse could score above the bar and put the wrong scripture in front of the congregation with no human in the loop. Confidences from different detection methods are on incomparable scales, so the *method* is the gate and the threshold only applies within the one method whose confidence means something. Revisit only when a genuinely calibrated embedder replaces TF-IDF — the seam is `SemanticIndex::top_k`. |
| **One threshold baseline: `Thresholds::default() == from_sensitivity(50)`, by construction** | Two baselines existed and disagreed (0.50/0.35 vs 0.90/0.60), and the dial's range could not even express the default. Any profile save — even a rename — snapped the live gate onto the other scale and discarded every feedback nudge. A single definition makes that class of bug unrepresentable. Saving a profile now only re-derives thresholds when the sensitivity dial *actually moved*. |
| **`db::app_data_dir()` is the only way to locate Relay's files** | `stt.rs` hand-rolled a macOS-only `$HOME/Library/Application Support` path, so on packaged **Windows** — a day-one platform — the STT model was never found and Relay ran with speech recognition silently dead. One per-OS helper, used everywhere (DB, media, models, exports). |
| **Relay must degrade to a working MANUAL tool, never to a dead one** | When no STT model loads, the operator gets a visible banner explaining the AI is off and that manual fire and plan playback still work — rather than an app that looks fine and simply never detects anything. Silent degradation is the worst failure mode for live software. |
| **Panic keys are global, owned by the app shell** | `Esc` (clear) and `B` (blackout) were bound per-view, so the operator's panic key did nothing while they were on the Templates, Library, or Settings tab. A panic control that works on *some* tabs is not a panic control. `Space` also meant two different things (advance vs. push-the-AI's-guess-live); it now means *advance*, everywhere, and accepting a suggestion has its own key (`A`). |
| **Manual fires are recorded as `status = 'manual'`, never `'auto'`** | Operator overrides were logged in `detections` as AI decisions. The self-calibrating router *learns from that column* — it was training on a record that could not distinguish a machine decision from a human one. |
| **LAN servers bind `0.0.0.0` with no authentication — accepted, and now recorded** | Kiosk screens, the OBS machine and the preacher's phone are all *other devices* on the church LAN, so a loopback bind would defeat the entire output-channel feature. The exposure is bounded and was checked, not assumed: the WebSocket hub is **broadcast-only** (the sole inbound message it honours is `hello`), so a stranger on the network can *read* the live content feed but can **never push to the screens**; and media path-traversal is defended (digit-prefix ids only). So the worst case is someone on the church wifi seeing the verse that is already on a projector in front of them. Accepted for a LAN appliance. **Revisit if Relay ever runs on an untrusted network** (a laptop that also joins café wifi is the realistic risk — the media server would serve imported assets to that network too). A bind failure is now surfaced to the operator instead of being swallowed to stderr. |
| **The console has a crash boundary; the outputs are the thing that must survive** | An uncaught UI error used to white-screen the console mid-service. It now shows a calm recovery panel that states the one fact that matters: the output screens are separate webviews and are still live, so the congregation sees nothing. Operator position (tab, plan, cue, slide) is persisted so Recover resumes where they were. |

## Non-goals, with reasoning

- **Not a recording/scene-compositing replacement for OBS.** OBS already does this well and free — not a place to spend differentiation effort.
- **Not a general AI assistant.** Scope discipline — this is a single live-service workflow tool, not a platform.
- **Not attempting song-lyric/setlist detection in v1.** Separate subsystem, separate risk, would dilute focus on the scripture-detection core loop that actually differentiates the product.

## Competitive framing (why this exists)

Pewbeam is a live, funded competitor with paying churches in 30 countries and a stated roadmap toward a full presentation suite. This project is a deliberate bet on out-executing a moving target on two specific axes — independent multi-screen templating, and African-language speech understanding as a first-class priority rather than an English-first afterthought — not an attempt to fill an empty market.

## 18. Rehearsal mode gates at the broadcast, not at the caller

**Decision.** Rehearsal is a single flag (`channels::Rehearsal`) read inside
`channels::broadcast_content` / `clear` / `black` — the three functions content
leaves the machine through. In rehearsal they emit to the operator console window
(`main`) only: no output window, no kiosk WebSocket, no LAN HTTP.

**Why not gate at the fire sites.** There are seven of them and there will be more.
A rehearsal that a new fire path can forget about is not a sandbox — it is a
promise that will be broken by the next feature. Gating at the choke point makes
every future caller sandboxed by construction.

**Why nothing upstream changes.** Detection, the router, the pipeline and the plan
transport all run exactly as they do live. A rehearsal that behaves differently
from a service does not rehearse the service.

**It fails OPEN.** `rehearsing()` returns false wherever the state is not
registered. The dangerous failure is silently swallowing content the operator
believes is live — not the reverse.

**Mutually exclusive with a recorded service.** `start_service` refuses while
rehearsing and `set_rehearsal` refuses while a service is recording. A practice run
filed under last Sunday is a record nobody can trust afterwards.

**The router does not learn from it.** `record_feedback` is skipped in rehearsal.
The volunteer is accepting verses they chose themselves, against speech that may be
them reading aloud from a phone. That is not evidence, and the self-calibrating gate
would carry the fiction into the real service.

**Leaving rehearsal CLEARS the screens.** The outputs have been showing whatever
they were showing before the rehearsal began, while the operator watched a console
preview saying something else. Handing back a live wall they have not looked at in
twenty minutes, silently, is how the wrong thing ends up in front of a congregation.

**Amber is not used for it.** Amber means ON AIR. A rehearsal is not on air, so it
is amethyst — in the top bar, on the output wall tally, and in a permanent band
across the console. A tally light that lies is worse than no tally light.

## 19. Audio levels are learned, never assumed

**Decision.** Nothing in the audio path may compare a signal against an absolute
level. The voice gate (`audio::Vad`) and the auto-gain (`dsp::FrontEnd`) both track
the room's noise floor and gate *relative* to it, with hysteresis.

**What went wrong.** Three absolute thresholds, each individually reasonable, each
tuned on a developer's machine:

- `VAD_RMS_THRESHOLD = 0.008` — "this is speech"
- `energy_prob = rms / TARGET_RMS` with `SPEECH_PROB = 0.55` — i.e. speech must
  reach RMS 0.066 before the auto-gain will believe in it
- `MAX_GAIN = 6.0` — +16 dB

Together they made Relay **deaf to a quiet preacher, silently**. A church laptop mic
sitting at RMS 0.005 was never recognised as speech, so the auto-gain never lifted
it — a deadlock in which you had to already be loud enough not to need gain in order
to be granted any. And the VAD then discarded two thirds of the sermon as "silence".

Measured, same words, real speech through the real front-end:

```text
   studio level   94% voiced        looks perfect on the developer's machine
   ×0.2           17% voiced        a church laptop. Most of the sermon deleted.
   ×0.05           2% voiced        a lightly-driven desk feed. Effectively deaf.
```

Nothing errored. The level meter still moved. The transcript just quietly turned to
nonsense — `John, 3, 6, Linn.`, the language detector flipping to Russian mid-sermon
— and the operator would conclude the AI "isn't very good".

**The rule.** Speech is a *rise above the room*, and it is *contrast*, not volume. A
steady tone is room tone however loud it is; a quiet voice over a quiet room is a
voice. Any future tuning knob that hard-codes "this many dB = speech" is the same bug
again, and is wrong on a microphone nobody in this repo has ever heard.

**Consequence, and the reason to hold this line:** it fails on exactly the churches we
built this for — the ones with a cheap mic at the back of a hall — and it fails
invisibly, so they will never report it as a bug. They will just stop using Relay.

Verify with `cargo test audio::gate -- --ignored --nocapture` (`RELAY_BENCH_WAV`,
`RELAY_BENCH_SCALE`): the voiced ratio must stay flat across a 100× range of input
level. It now runs 39–55%; it used to collapse to 0%.

## 20. A panic control may never report a success it did not achieve

**Decision.** Clear and blackout must be *incapable* of silent failure. `clear_screens`
and `blackout` return `Result`; `channels::clear`/`black` propagate the emit error
instead of `let _ =`; the frontend wrappers return a boolean **and** raise a global
`panicError` banner. No caller may announce success without checking.

**What went wrong.** A failed clear was structurally unrepresentable. `channels::clear`
discarded the emit error, `clear_screens` returned `()`, `clearScreens()` swallowed
what was left — so `Live.svelte` flashed **"Screens cleared"** over a `catch {}` that
could not even fire. If the clear failed, the operator was told the wall was clean
while the verse was still in front of the congregation.

**Why that is the worst class of bug we can ship.** A control that *fails* is survivable
— the operator looks at the screen, sees the verse, presses it again. A control that
*lies* is not: it teaches the operator to stop looking at the screen and trust the
toast. Every subsequent failure is then invisible, by their own trained habit.

**Both a return value AND a store, deliberately.** The panic controls are fired from a
global keydown handler and from a shell button that must keep working when the current
view has crashed. Neither can `catch`. A thrown error there is an unhandled rejection —
silence with extra steps. The store means the failure surfaces no matter who pulled the
trigger; the boolean means a caller that wants to flash success has to ask.

**The fire-and-forget paths get a voice too.** The spoken "clear the screen" and the exit
from rehearsal (which hands the wall back to the congregation) have no caller to return
an error to. They emit `output://panic_failed`, which raises the same banner. A spoken
panic control that fails silently is exactly as dangerous as a keyed one.

**The banner is not a toast.** Top of screen, `role="alert"`, rose (never amber — amber
is a tally light and is never allowed to lie), and it **does not auto-dismiss**. A
message that fades after 2.6 seconds is how the operator misses it.

**Escape does not clear the screens as a side-effect of closing the help overlay.** It
used to. An operator checking a binding mid-service, then dismissing the cheatsheet,
wiped the wall. Dismissing a read-only overlay is not a live action.

**And `B` does not work while typing — so the help must not say it does.** Typing
"Habakkuk" into the reference box cannot be allowed to black out the room on the `b`.
The behaviour was right; the cheatsheet's promise was wrong. Help text about a panic
key is read only under pressure, and a false line there is worse than no line.

Pinned by `src/lib/panic.test.js` and the cheatsheet tests in `src/lib/shortcuts.test.js`
— both verified to fail if the original bugs are reintroduced.

## 21. The operator must be shown WHICH KIND of claim the AI is making

**Decision.** A detection carries its evidence to the console and the console renders
it. `DetectionEvent` gained `matched_text`; a heard reference shows the transcript span
it was parsed from, a paraphrase shows the overlapping words that produced its cosine.
Confidence is shown as a percentage **only** for a `Direct` match. A paraphrase shows
**no number at all**, at any score.

**Why this is a safety decision and not a UI one.** Relay's entire correctness story is
§ "only `Direct` may auto-fire" — a rule the router enforces at any score, at any
sensitivity, and which is property-tested. It exists because a TF-IDF cosine is not a
probability and the two confidences are on incomparable scales.

And the console rendered both as **"AI suggestion — 92% match"**. The gate was airtight
in Rust and invisible in the one place a human could act on it. We built a careful
machine for keeping a human in the loop, then showed the human nothing to be in the loop
*with*: they were asked to accept or reject the AI's judgement on the strength of a
number that means one thing for one kind of match and nothing whatsoever for the other.

**No number is better than a misleading number.** "61%" beside a cosine invites the
operator to read it as "61% likely to be right". A number that lies is worse than no
number, because it looks like information and therefore gets acted on. The words —
`shepherd · lord` — are something a volunteer can actually judge in the second they
have to judge it.

**A paraphrase's evidence is its overlapping terms, ranked by contribution.**
`SemanticIndex::top_k_explained` returns the terms that genuinely produced the score
(each term's `q_weight * d_weight`, the summands of the cosine itself), not words that
merely appear in both. An explanation that is not the real reason is worse than none.

**Cyan, not amethyst — even though amethyst is the obvious colour for "uncertain".**
Amethyst already means REHEARSAL (§18), and a tally colour must mean exactly one thing.
A colour that means "nothing is reaching the congregation" cannot also mean "this guess
is shaky", or on the day both are true the operator reads the wrong one. Amber remains
ON AIR, and is never allowed to lie.

Pinned by `src/lib/detect.test.js` (a paraphrase never shows a percentage, at any score
— the frontend mirror of `router.rs::semantic_can_never_auto_fire`) and by
`pipeline.rs::the_event_carries_the_evidence_the_operator_must_judge`.

---

## 22. Chrome is amethyst; amber is spent only on air

**Decision:** the interactive accent of the whole application — the selected nav
item, focus rings, switches, sliders, hovers, and the ordinary primary button —
is **amethyst**, exposed as one token, `--v-accent`. **Amber survives only where
something is, or is about to be, in front of a congregation.**

Before this, amber *was* the chrome accent. The active tab was amber. The focus
ring was amber. Twenty-three ordinary buttons — Save song, Add channel, Import,
Continue, Run the setup walk-through — were amber. So was the sidebar avatar, the
Settings section headings, the threshold sliders, every hover state, and the
`file paths in Channels`.

**That is a colour that is always on.** And a colour that is always on cannot also
be a warning. §18, §20 and §21 all lean on the same premise — that amber means one
thing, *the congregation is looking at something*, and is never allowed to lie —
while the application itself was lighting the tally colour on every screen, at
rest, permanently. The one rule the product's safety story depends on was being
broken by its own Save buttons.

The design system settles it in two places: the USAGE GUIDE says amber is
"anything that is live/on the wall", and `relay-production-interface.png` draws
the active sidebar item **amethyst-tinted, not amber**.

**Why amethyst and not a new neutral accent.** Amethyst already means "not
reaching the screens" (§18, rehearsal). For chrome — a tab, a text field, a
toggle — that is exactly right: touching it does not put anything on a wall.
The two readings agree rather than compete.

**The token, not the hex, is the rule.** `--v-accent` exists so this is
enforceable: chrome points at the accent, and reaching past it to `--v-amber`
is a visible, reviewable act at the call site. `.r-btn.primary` is the default
button; `.r-btn.amber` is documented as ON AIR ACTION and is deliberately not
the easy choice.

**Kept amber, deliberately:** the On Air badge (`App.svelte`, `Live.svelte`), the
TAKE and Fire controls, and the on-air plan-position chip. **Moved off amber:**
"Engine ready" in Channels → green, the design sheet's connected colour; and the
detection-method badge in Service History → grey, because nothing in a record of
last Sunday is on air.

The application icon changed for the same reason: it was an amber "R", sitting in
the Dock in the tally colour whether or not Relay was even running. It is now the
design sheet's amethyst waveform (`src-tauri/icons/relay-mark.svg`, the source of
truth for every generated size), and `src/lib/ui/BrandMark.svelte` is the one
copy of that mark inside the app.

---

## 23. A voice gate, not a sound gate — and whisper is never asked to transcribe silence

**Reported from a real service:** *"the transcript is getting Chinese words and
other languages that aren't heard."*

That symptom is not a language bug. It is what a sequence model does when it is
handed audio containing no words.

**Whisper has no way to say "nothing was said."** Fed a door, a chair, an
air-conditioner surge or a music bed, it emits the most likely continuation — and
its training data is full of subtitle boilerplate, much of it Chinese, Korean and
Russian. The model is completing a subtitle file. Relay was asking it to.

Three things were true at once, and each one is fixed:

**1. The gate could not tell a voice from a sound.** It was an energy gate —
adaptive and hysteretic (§19), but energy nonetheless, and a slammed door has
plenty of energy. Meanwhile `dsp.rs` was already computing RNNoise's per-frame
**speech probability** and using it only for auto-gain and the level meter. That
number now vetoes OPENING the gate.

The asymmetry is load-bearing: **the probability may veto opening an utterance;
it may never close one.** Once the preacher is speaking, only energy and its
hysteresis decide when they stop. If an unsure model could shut the gate, every
shout, whisper, sung line and heavy accent would chop the sentence — the exact
failure `stt.rs`'s "append every chunk, silence inside an utterance is audio"
rule exists to prevent. Being wrong about the START of an utterance costs one
late word. Being wrong about the MIDDLE mangles the transcript.

The bar (`SPEECH_OPEN_MIN = 0.30`) is deliberately low. It asks "is this
definitely *not* speech?", never "is this definitely speech?" — because §19 is
the scar from a gate that went silently deaf to a quiet preacher, and a confident
threshold here would rebuild it in a new place. It applies only when the real
neural VAD is running; below 48 kHz `speech_prob` is an energy proxy and judging
energy by energy is circular, so the gate behaves exactly as before.

**2. Whisper's own hallucination guards were never switched on.**
`FullParams::new` does not apply whisper.cpp's defaults, so `suppress_blank`,
`suppress_nst` (the "♪ / [Music] / 字幕" token family), `no_speech_thold`, the
temperature fallback, `logprob_thold` and `entropy_thold` were all unset. They
are now at whisper.cpp's own defaults.

**3. Nothing checked the output was even in the right script.**

**The last one is a script check, not a phrase blocklist, and that is the whole
point.** Blocklisting the Chinese strings whisper happens to emit fails the moment
it emits a different one, and encodes the false assumption that Chinese is the
only wrong answer. The invariant is the script: every language Relay ships
recognition for — English and Tier-1 Yoruba, Kiswahili, Hausa — is written in
**Latin**. A CJK, Hangul, Kana, Cyrillic, Arabic, Hebrew, Thai or Devanagari
letter in the output is not a mis-hearing of a word; it is the model completing a
subtitle file. One such character condemns the line.

Two things this must never break, both pinned by tests:

- **Yoruba and Hausa are Latin.** `ẹ ọ ṣ` live in Latin Extended Additional and
  `ɓ ɗ ƙ` in the IPA range. A naive `is_ascii_alphabetic` check would discard
  every Tier-1 transcript as "foreign" — silently making Relay deaf to the
  languages it exists to serve.
- **Code-switching mid-sentence is normal here, not an edge case.** A line that
  mixes English and Yoruba must survive.

An explicitly chosen non-Latin language is respected: the guard exists because
AUTO-DETECT picks badly on short, quiet or noisy audio, not to refuse languages.

**Not verified with real audio.** Every test here is a unit test over strings and
levels. Per §13, an STT change is scored through the DETECTOR, not by reading the
transcript — and that requires a real recording through `RELAY_BENCH_WAV`, which
has not been run for this change.

## 24. A bare spoken number pair fires; a garbled RUN of numbers does not

Preachers say **"Romans eight one"** and **"Psalm 23, 1"**. They do not reliably say
"verse". Whisper renders the pauses as commas and full stops, `normalize` strips them,
and all of it arrives at the parser as the same bare pair: `book <num> <num>`.

That form used to score **0.45** — deliberately below auto-fire — because of a real
live-rehearsal transcript:

```
"Verse 1, Psalms 2, 3, 1, Next verse, chapter 2,"
```

which had scored 0.92 and put **Psalms 2:3 on the wall, unasked**. The demotion was the
right call at the time, and its reasoning was recorded in the code. But it also meant a
preacher who never says "verse" **never reaches the screen at all** — the product missed
ordinary preaching, which is a failure of the same size, just quieter.

**The two cases are not separable by confidence.** The parser sees the identical shape.
What separates them is the **leftover number**: `"Psalm 23 1"` ends cleanly, while
`"Psalms 2, 3, 1"` parses 2:3 and strands a `1` that no range could absorb (a range end
must be `>=` the verse). Numbers that do not line up are garble; numbers that do are a
reference.

So:

- A bare pair scores **0.55** — above the default auto-fire line (0.50), and still fully
  governed by the sensitivity dial (a cautious install at auto-fire 0.90 demotes it
  exactly as before). It is not a new baseline; it is one value moved across an existing,
  operator-controlled line.
- A bare pair **followed by another loose number** stays at **0.45** — it reaches the
  operator, never the congregation. That is the rehearsal transcript, and it is pinned by
  `a_garbled_number_run_never_auto_fires`, which was **mutation-verified**: deleting the
  guard makes the garbled run score 0.55 and the test fail.
- A **repaired** book name plus bare digits lands at `0.55 - 0.06 = 0.49`, still under the
  line. A misheard book *and* loose digits is two guesses stacked, and always asks a human.

This does not touch the structural rule in the live-safety decisions above: **only
`DetectionMethod::Direct` may ever auto-fire**, and no threshold change can promote a
paraphrase — `Semantic`/`Ambiguous` are capped at `Suggest` before any number is consulted. The detection scorecard is unchanged — 50/50
cases, 100% recall, **0 wrong verses, 0 paraphrases auto-fired**.

## 25. Two shared words, or one rare one — evidence, not word count

**Decided during the `NEW-DESIGN-IMPLEMEMTATION` merge, because the two branches
disagreed and each shipped a test that failed if the other won.**

`top_k_explained` refuses a paraphrase candidate it cannot justify. The bar was a flat
count: at least `MIN_EVIDENCE_TERMS` (2) shared content words, so a match is
*corroborated* rather than merely confident. One shared word is usually a coincidence
with a good score, and — under §21 — a one-word explanation gives the operator nothing
they can actually judge in the second they have to judge it.

The KJV gloss (`expand_with_gloss`) landed independently and its entire premise is the
opposite case: a modern retelling reaches its verse through **exactly one** rare KJV
noun. "he ended up feeding pigs" shares precisely `swine` with Luke 15:16 — no second
word exists to corroborate it, in the test corpus or in the real KJV. A flat 2-word floor
does not make that match weaker; it makes the gloss inert.

Both are right about different words, so the rule is about **evidence, not arithmetic**:

- **Two or more shared words** — offered, as before.
- **Exactly one shared word** — offered *only* if that word is rare: it appears in at most
  `RARE_DF_FRACTION` (0.1%) of the corpus, floored at one document so the rule still
  bites on the small corpora the tests build. On the full KJV that is ~31 of 31,102
  verses: `swine` (~30) and `ossifrage` (2) clear it; `lord` (~7,800) is nowhere near.
- Rarity is computed **at build time, from raw document frequency**, and judged on the
  **stem** — `surface` deliberately maps several stems onto one readable word, and `idf`
  is a float nobody should be inverting on the query path.

A word that names one story is corroboration all by itself, because there is nowhere else
in the corpus it could have come from. A word that names half the Bible is not, however
high it scores.

This changes what is *suggested*, never what fires: `Semantic` is still capped at
`Suggest` in `router.rs::decide` (live-safety rule #10), so every one of these reaches a
human and none reaches a congregation on its own.

Pinned by `a_common_single_shared_word_is_not_offered_as_a_paraphrase` and
`a_rare_single_shared_word_is_evidence_enough`, both **mutation-verified**: removing the
rarity exception fails the second, and treating every term as rare fails the first.
