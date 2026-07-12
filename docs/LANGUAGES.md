# Languages — Yorùbá, Kiswahili, Hausa

**This is Relay's differentiator, and it is the part most in need of people who
actually speak these languages.** If that's you, the most valuable ten minutes you
can spend on this project are below. You do not need to know Rust.

---

## The thing that was actually broken

Relay's pitch is African-language speech recognition. But until recently the
**detector spoke only English.**

A preacher could say:

> **"Ẹ ṣí Jòhánù orí kẹta, ẹsẹ kẹrìndínlógún."**

…with a *perfect* Yorùbá acoustic model behind them — and Relay would detect
**nothing.** The transcript would be flawless, and the alias table had never heard
of `Jòhánù`, so it matched no book, and no verse ever reached the screen.

**Fine-tuning the speech model would not have fixed that by a single verse.** The
moat was blocked on a lookup table, not on machine learning. That table now
exists.

Second problem, same shape: the STT **decoder-bias prompt** primed whisper with
the *English* book names regardless of what was being preached — actively pushing
it to hear "John" where the preacher said "Jòhánù". It now speaks the language of
the active voice profile.

---

## How to fix a book name (no Rust required)

All the names live in one file:

### 📄 [`src-tauri/data/book_aliases.json`](../src-tauri/data/book_aliases.json)

```json
"yo": {
  "John":   ["Jòhánù", "Johanu"],
  "Psalms": ["Sáàmù", "Saamu"]
}
```

- The **key** is the English book name (exactly as in `CANONICAL_BOOKS`).
- The **value** is every way a preacher might say it. First entry = the properly
  accented spelling.
- **Write it properly, with its diacritics.** Matching is diacritic-*insensitive*:
  `Jòhánù`, `Johánù` and `Johanu` all fold to the same token, because whisper
  emits all three depending on the recording. You don't have to think about it.

Edit, open a pull request, done. No build, no Rust, no `HashMap`.

### ⚠️ Please don't guess

**Omission is safe. A wrong alias is not.**

A missing name means Relay doesn't detect that book in that language yet — the
status quo, harmless. A *wrong* name means **the wrong scripture appears on a wall
in front of a congregation.** That is the failure this whole project is built to
avoid.

If you're not sure, leave it out and say so in the PR. Someone who is sure will
add it.

---

## Current state

| Language | Books covered | Status |
|---|---|---|
| **Kiswahili** (`sw`) | **66 / 66** | ✅ Complete — Biblia Takatifu / Neno |
| **Yorùbá** (`yo`) | 29 / 66 | ⚠️ **New Testament only.** The Old Testament is missing. |
| **Hausa** (`ha`) | 11 / 66 | ⚠️ **Gospels + a few.** Most books are missing. |

**Yorùbá and Hausa need a native speaker.** The Swahili list was verifiable from
published Bible-society translations; the other two were only partially so, and
the maintainer chose to leave gaps rather than invent names.

---

## Numbers are the next gap

Chapter and verse numbers spoken **in-language** are not yet parsed:

> "Yohana **sura ya tatu**" (Swahili: John chapter three) → not detected.

In practice this matters less than it sounds, because **code-switching is the
normal case, not an edge case** (`CLAUDE.md`): a Yorùbá sermon routinely names the
book in Yorùbá and the numbers in English, which already works:

> ✅ "Ẹ ṣí **Jòhánù** chapter **three** verse **sixteen**" → John 3:16

Swahili and Hausa numerals are regular and would be straightforward. **Yorùbá
numerals are subtractive** (16 = *ẹrìndínlógún*, literally "four less than
twenty"), which is a genuinely interesting parsing problem and a great first
contribution for a Yorùbá speaker.

---

## The acoustic model

Whisper was trained on ~117,000 hours across 96 languages — but **Yorùbá and Hausa
together contribute under 600 hours** of that. That is the quantitative reason the
base multilingual model is weak on them, and it is not something Relay can fix in
code.

The model is a **pluggable component, not baked into the pipeline**
(`docs/DECISIONS.md`), so a better one can be dropped in as community fine-tunes
mature. Converting a Hugging Face fine-tune to the `ggml` format whisper.cpp needs
is done with
[`convert-h5-to-ggml.py`](https://github.com/ggml-org/whisper.cpp/tree/master/models)
from whisper.cpp.

**Relay ships no fine-tune today, because none has been verified against real
sermon audio.** Shipping an unmeasured model and calling it an African-language
feature would be a marketing claim, not an engineering one.

---

## What would actually move the needle

In order of value:

1. **Finish the Yorùbá and Hausa book names.** Pure data. Ten minutes. Unblocks
   detection entirely for those languages.
2. **Record real sermon audio** with a known transcript — even 30 minutes.
   Relay's African-language accuracy is currently **unmeasured**, and you cannot
   improve what you have never baselined. This is the single most useful thing
   anyone can contribute.
3. **In-language numerals**, starting with Swahili and Hausa.
4. **A verified fine-tune**, once (2) exists to measure it against.

---

## Sources

- Kiswahili — [Biblia Takatifu / Neno (Biblica)](https://www.biblica.com/bible/nen/matayo/1/), [Bible book names list](http://www.cos-had.org/wp-content/uploads/2020/04/Swahili_-_A_Bible_Book_Names_List_English_to_Swahili.pdf)
- Yorùbá — [Yoruba Contemporary Bible (Biblica)](https://www.biblica.com/bible/ycb/saamu/1/)
- Hausa — [Hausa Contemporary Bible (Biblica)](https://www.biblica.com/bible/hcb/galatiyawa/1/), [Bible Society of Nigeria](https://find.bible/bibles/HAUBSN/)
- Whisper low-resource coverage — [whisper.cpp models](https://github.com/ggml-org/whisper.cpp/tree/master/models)
