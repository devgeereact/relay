# How the AI works, and what it will never do

**Relay puts scripture on a wall in front of a congregation, and it decides some of
that by itself.** A church deserves a plain account of what the machine does, what
it refuses to do, and where it is weak.

No hedging below. If a line here is wrong, that's a bug — please report it.

---

## What the AI actually does

1. **It listens.** A speech model running *on your machine* turns the preacher's
   voice into text. Nothing is uploaded. (See [`PRIVACY.md`](../PRIVACY.md).)
2. **It looks for scripture references** in that text — *"John chapter three verse
   sixteen"*, *"Sáàmù 23:1"*, *"Zabura sura ta ashirin da uku"*.
3. **It decides** whether it is confident enough to put the verse on screen by
   itself, or whether to offer it to you first.
4. **You can always overrule it**, instantly, from any screen.

---

## The rule that matters most

> ### The AI never guesses a verse onto the screen.

Relay detects scripture two ways, and it treats them **completely differently**:

| How it was found | Can it go on screen by itself? |
|---|---|
| The preacher **said the reference** — "John 3:16" | **Yes**, if the parse is confident. |
| The preacher **paraphrased a verse** without naming it | **Never.** It is offered to you, and waits. |

A paraphrase match is a similarity score, **not a probability.** A sermon that
happens to share a few rare words with some verse can score highly and mean nothing.
So no threshold on it is meaningful, and **no paraphrase ever reaches a congregation
without a human agreeing to it first** — at any score, at any sensitivity setting.

This is enforced in the code (`router.rs`), not by policy, and there is a test that
fails the build if it is ever violated.

---

## Where it is weak, honestly

**African-language speech recognition is the weakest part of the product, and it is
also the headline claim.** We would rather you heard that from us.

- The underlying speech model (Whisper) was trained on ~117,000 hours across 96
  languages — but **Yorùbá and Hausa together contribute under 600 of them.**
  Accuracy on those languages is meaningfully worse than on English.
- Relay ships **no fine-tuned African-language model**, because none has been
  verified against real sermon audio. Shipping an unmeasured model and calling it a
  feature would be marketing, not engineering.
- **Detection** in Yorùbá, Swahili and Hausa *is* measured, and currently scores
  100% recall with zero wrong verses on our benchmark. **Transcription is not
  measured at all** — we have no real sermon audio to measure it against.

See [`docs/LANGUAGES.md`](LANGUAGES.md). If you speak one of these languages, the
most valuable thing you can give this project is 30 minutes of recorded preaching
with a transcript.

---

## Where the AI is not involved at all

- **Songs, media, announcements and countdowns** are never AI-decided. You put them
  in a plan, you fire them.
- **The verse text itself** is not generated. It is read verbatim from a bundled
  King James Version. **Relay never writes scripture. It only finds it.**
- **Templates and styling** are yours.

---

## Your controls

- **Escape** clears every screen, from any tab, even mid-typing.
- **B** blacks out every screen.
- **Emergency Stop** is in the top bar of every screen.
- **Detection can be switched off entirely** — Relay still works as a fast manual
  tool, and every other feature is unaffected.
- **Sensitivity is yours to set.** Relay also learns from every time you accept or
  reject a suggestion, and that learning is per-preacher and stays on your machine.

---

## What we record about the AI's decisions

Every fire is logged locally as **`auto`** (the AI decided), **`suggested`** (it
offered), or **`manual`** (a human decided). We keep that distinction carefully,
because the system learns from it — and a machine that mistakes your decisions for
its own is a machine learning from a lie.

None of it leaves your computer.
