# Contributing to Relay

Relay puts scripture on a wall in front of a congregation, live, with a volunteer driving and no second take. That one sentence explains every rule below.

**The two most valuable contributions to this project require no code at all.** They are first, because they are genuinely the ones we need.

---

## 1. Speak Yorùbá, Swahili or Hausa? Start here.

Relay listens to a sermon in Yorùbá, detects the verse — and then talks to the volunteer running it **in English**. It understands three African languages and cannot say a word of any of them to its own operator.

That is not because it is hard. It is because **nobody who can do it has done it yet.**

Everything below is a **JSON file**. No Rust. No Svelte. No build. No development environment. Edit it on GitHub in your browser, open a pull request, done.

### The console's language — `src/lib/locales/{yo,sw,ha}.json`

These files ship **empty on purpose.** They were not written by an AI or a non-speaker, and they will not be.

Copy any key you like from `src/lib/locales/en.json`, paste it in, and translate the **value**:

```json
{
  "live.push_to_stage": "…",
  "live.no_suggestions": "…"
}
```

Leave a key out and it simply stays English — so **one key is a useful pull request**, and a part-translated console is a working console. Start with the `live.*` keys: that is the screen a volunteer actually uses on a Sunday, under pressure, in a dark booth.

Keep every `{placeholder}` — word order may change, the placeholder must survive.

### The book names — `src-tauri/data/book_aliases.json`

66 books × 3 languages, and **not one of them has been checked by a native speaker.** They were assembled from public sources by someone who does not speak these languages. Some are probably wrong.

A wrong book name does not fail loudly. It fails **silently**: the preacher says the name, Relay hears it perfectly, matches nothing, and no verse ever reaches the screen. Nobody knows why. This table *is* the moat, and it is unreviewed.

### The numbers — `src-tauri/data/numerals.json`

Swahili and Hausa are done. **Yorùbá is not, and it is the hard one.**

Yorùbá is subtractive and vigesimal: 16 is *ẹrìndínlógún* — literally "four less than twenty". That is a real parsing problem, not a lookup table, and it needs someone who actually speaks the language to get right.

> **Why an AI has not just done this:** a wrong numeral does not fail safely. It silently shows **a different verse**. If `tisa` were mapped to 8 instead of 9, nobody would find out until a Sunday. This is exactly the kind of thing that must come from a person who knows, not a machine that guesses convincingly.

---

## 2. Have thirty minutes of a real sermon on tape?

Relay's word error rate has **never been measured. In any language. Including English.**

The ruler is built, unit-tested, and runs in CI. There has simply never been a recording to point it at, and a developer reading verses into a MacBook in a quiet room measures nothing — the whole audio front-end had to be rebuilt once because a *quiet preacher* was silently undetectable and nobody noticed for months.

**[`bench/README.md`](bench/README.md) tells you exactly what to record and how to run it.** It is about an hour of work and it unblocks the entire African-language differentiator.

Audio is **never** committed to this repository — `PRIVACY.md` promises a church that their sermon stays on their device, and that promise is not conditional. Keep the recording. Commit the number.

---

## 3. Code

### Before you start

Read **[`CLAUDE.md`](CLAUDE.md)**. It is the real map, and its "Architecture rules learned the HARD WAY" section is 25 numbered rules — **each one is a bug that reached, or would have reached, a congregation.** They are not style preferences. Regressing one of them is how a wrong verse ends up on a wall.

Read **[`docs/DECISIONS.md`](docs/DECISIONS.md)** before proposing a feature. If a decision is not in there, it has not been made yet — ask, don't assume. If the code contradicts it, the **code** is wrong: flag it, don't silently "fix" the decision.

### Setup

```bash
npm install
npm run tauri dev
```

You need **cmake** (whisper.cpp compiles from source) and a Rust toolchain. The desktop window is the app — `localhost:5032` in a browser is a dead console with no engine attached.

### The bar

```bash
cd src-tauri
cargo fmt --all && cargo clippy --all-targets -- -D warnings   # CI enforces both
cargo test                                                     # 250 tests
cd .. && npm test                                              # 138 tests
```

Beyond green:

- **No `unwrap()` in anything that runs during a service.** A panic mid-sermon is the worst possible failure. There are currently zero in the seven live modules; keep it that way.
- **Never build an `OutputContent` or a `DetectionEvent` by hand.** Go through `pipeline::Fire`. Five hand-rolled copies drifted apart, and two silently dropped the scripture template.
- **A control may never report a success it did not achieve.** If it can fail, it says so. This is `DECISIONS.md` §20 and it is the single most important rule in the project.
- **Test the bug, not the fix.** After you fix something, *reintroduce the bug and check your test fails.* Several tests in this repo initially passed on broken code — a focus trap whose visibility check reported every element hidden under jsdom, an entitlement test that grepped a comment instead of the config. Both were caught this way, and neither would have been caught otherwise.

### What gets a PR rejected

- **A number that means one thing in one place and another somewhere else.** A TF-IDF cosine is not a probability, and rendering it as a percentage next to a real confidence is how the wrong verse reaches a wall.
- **Borrowing a colour that already carries a promise.** Amber means ON AIR. Amethyst means REHEARSAL. A tally light that lies is worse than no tally light.
- **Swallowing an error on any path the congregation can see.** `src/lib/stores/capture.js` states the contract at the top: *can the congregation see the difference?*
- **Adding a dependency for something that is 60 lines.** This app runs offline on a donated laptop.

---

## Reporting a bug

If it happened **during a live service**, say so — that moves it to the front of the queue, every time.

Please include what was on the screen, what you expected, and what Relay told you (or didn't). If it was an audio or detection problem, `CLAUDE.md` documents the environment variables that make it reproducible without a human at the microphone.

Security issues go to **[SECURITY.md](SECURITY.md)**, privately, not to the issue tracker.

---

## The one thing to keep in mind

Every bug in this project has a person attached to it: a volunteer, in a dark booth, with five hundred people waiting and no second take. Write code that is kind to them.
