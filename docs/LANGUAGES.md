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

| Language | Books | Sourced from | Native-speaker reviewed? |
|---|---|---|---|
| **Kiswahili** (`sw`) | **66 / 66** | Biblia Takatifu / Neno (Biblica) | ❌ not yet |
| **Yorùbá** (`yo`) | **66 / 66** | Yoruba Contemporary Bible + Bibeli Mímọ́ | ❌ **not yet** |
| **Hausa** (`ha`) | **66 / 66** | Bible Society of Nigeria 1932/2010 + HCB | ❌ **not yet** |

All three are complete and every name came from a published translation — **but
none has been checked by someone who actually speaks the language.** That is the
gap that matters now, and it is the one thing no amount of engineering closes.

> **The same table is now in the app**, at **Settings → Languages**, and every number
> in it is derived from the shipped data files rather than typed in — so this page
> and the running application cannot disagree about how many aliases exist or which
> numerals parse. **Its two most important columns are empty on purpose:** native
> review and word error rate render as *absences*, not as zeroes, because a screen
> that shows 0% accuracy and a screen that shows "never measured" say opposite
> things to a church deciding whether to trust this.
>
> Alias counts, live: `python3 -c "import json;d=json.load(open('src-tauri/data/book_aliases.json'));print({k:len(v) for k,v in d.items() if k!='_readme'})"`
> — 66 canonical books in each, with some carrying more than one accepted name.

**Yorùbá carries two translations at once.** Biblica's *Yoruba Contemporary Bible*
calls Psalms **Sáàmù**; the older *Bibeli Mímọ́* calls it **Psalmu**; many churches
say **Orin Dáfídì**. A preacher says whatever their own Bible says, so all three
are listed. Add more — there is no cost to an extra alias that is correct.

### The trap that nearly got in

Some book names are also **ordinary words**:

| Word | "Means" | Actually means |
|---|---|---|
| `Iṣẹ́` (yo) | Acts | **work** |
| `Orin` (yo) | Song of Solomon | **song** — in a church |

Listing those bare would fire scripture off normal speech: *"Iṣẹ́ wa ni lati sin
Ọlọrun"* ("our work is to serve God") would have put the book of Acts on the wall.
So only the full forms are in the table — **`Ìṣe àwọn Àpọ́sítélì`**, **`Orin
Solomoni`** — and there is a test that fails the build if a bare everyday word is
ever added.

**Before you add a name, ask: could a preacher say this word without meaning the
book?**

---

## Numbers

**Swahili and Hausa now work fully in-language.** A preacher can say the book, the
chapter and the verse without a word of English:

> ✅ "Yohana **sura ya tatu**, **mstari wa kumi na sita**" → John 3:16
> ✅ "Zabura **sura ta ashirin da uku**, **aya ta farko**" → Psalms 23:1

The words live in [`src-tauri/data/numerals.json`](../src-tauri/data/numerals.json)
— data, not Rust, for the same reason as the book names. **A wrong numeral does not
fail safely: it silently shows a different verse.** If `tisa` were mapped to 8
instead of 9, nobody would find out until a service.

### The one thing that is not like English

**The hundred multiplier comes AFTER the hundred word.**

```
  mia moja  = 100   (literally "hundred one")   NOT 101
  mia mbili = 200                               NOT 102
  ɗari biyu = 200                               NOT 102
```

English puts it first ("two hundred"). So the English parser, run on Swahili, would
read **"mia mbili" as 100 + 2 = 102** — and put **Psalm 102 on the wall when the
preacher said Psalm 200.** There is a test that asserts exactly this, by name.

A connector disambiguates: `mia moja` (no connector) is 1×100, while `mia na tatu`
(connector) is 100+3. Both are handled.

### Yorùbá numerals are still to do

**Yorùbá is subtractive**, and genuinely hard: 16 is *ẹrìndínlógún* — literally
*"four less than twenty"*. It is a real parsing problem, not a lookup table, and it
is a great first contribution for a Yorùbá speaker.

Until then Yorùbá relies on code-switching, which is **the normal case rather than
an edge case** (`CLAUDE.md`) — a Yorùbá sermon routinely names the book in Yorùbá
and the numbers in English, and that already works:

> ✅ "Ẹ ṣí **Jòhánù** chapter **three** verse **sixteen**" → John 3:16

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

1. **Review the Yorùbá and Hausa names.** They are complete and sourced, but not
   verified by a speaker. If one is wrong, Relay will confidently show the wrong
   scripture — that is the failure this whole project exists to prevent.
2. **Record real sermon audio** with a known transcript — even 30 minutes.
   Relay's African-language accuracy is currently **unmeasured**, and you cannot
   improve what you have never baselined. This is the single most useful thing
   anyone can contribute.
3. **Yorùbá numerals** — subtractive, and the last piece of in-language parsing.
   (Swahili and Hausa are done.)
4. **A verified fine-tune**, once (2) exists to measure it against.

---

## Sources

- Kiswahili — [Biblia Takatifu / Neno (Biblica)](https://www.biblica.com/bible/nen/matayo/1/), [Bible book names list](http://www.cos-had.org/wp-content/uploads/2020/04/Swahili_-_A_Bible_Book_Names_List_English_to_Swahili.pdf)
- Yorùbá — [Yoruba Contemporary Bible (Biblica)](https://www.biblica.com/bible/ycb/saamu/1/)
- Hausa — [Hausa Contemporary Bible (Biblica)](https://www.biblica.com/bible/hcb/galatiyawa/1/), [Bible Society of Nigeria](https://find.bible/bibles/HAUBSN/)
- Whisper low-resource coverage — [whisper.cpp models](https://github.com/ggml-org/whisper.cpp/tree/master/models)
