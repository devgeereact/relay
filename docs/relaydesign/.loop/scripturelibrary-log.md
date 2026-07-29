# Scripture Library (§7) — design loop log

Reference: `relay-production-interface.png` **panel 6 — Library / Verse &
Translation Manager**.

Gate: `npm run build` clean, **206 frontend**, **315 Rust** (5 new), fmt +
clippy clean, detection scorecard unchanged (100% / 0 wrong verses).

---

## What was missing

The Library could **search** scripture and could list what an operator had
**saved**. It could not open a Bible and read it — which is the one thing the
word "library" promises, and the whole of panel 6.

Built `src/lib/views/library/Browse.svelte`: book tree in canonical order,
chapter list, the chapter's verses, and per-verse **Save** / **Put on screen**.
New backend: `list_books` and `chapter_verses` (+ `db::active_translation_id`).

**Canonical order is the substantive part.** `GROUP BY book` returns
alphabetical, and a Bible whose contents open *"Acts, Amos, Chronicles"* is not
one anyone can navigate — the order is part of what the book is. Ordered through
`detection::CANONICAL_BOOKS` and pinned by a test that fails if the list ever
comes back sorted.

## The departure from the reference

The mockup puts **four language tabs** across the top: English · Yoruba ·
Swahili · Hausa.

**Relay ships one Bible.** The corpus is the KJV and nothing else — confirmed
against the live database, which holds exactly one translation row. Three of
those four tabs would open on an empty shelf.

That is not cosmetic. Relay's stated differentiator is African-language
scripture, and a UI showing those languages as present would make precisely the
claim `docs/LANGUAGES.md` is careful not to make. So the picker is built from the
translations that **actually exist**, and the gap is stated in words underneath:
detection already *recognises* spoken Yorùbá, Kiswahili and Hausa references —
that alias table is real — what is missing is verse TEXT, which is a sourcing and
licensing problem, not a feature.

Also: the reference paints the selected verse **gold**. Gold means ON AIR here
(DECISIONS §22), and a verse being read in the library is on nobody's screen, so
selection uses the accent. The one genuinely amber control on the screen is
**Put on screen**, because that is a real fire to the real outputs.

## Screens covered

| Listed | State |
|---|---|
| **Library** | ✅ built — browse + saved + search |
| **Verse Comparison** | ✅ **as far as the data allows.** Comparing translations needs ≥2 translations; there is 1. The picker appears the moment a second exists |
| **Favourite Scriptures** | ✅ already built — the Saved tab (`save_scripture`), now reachable per-verse from the browser |
| **Recent Scriptures** | ✅ already built — Service History lists every fired verse per service |
| **Translation Manager** | ✅ partly — active translation is chosen in Settings and honoured here. There is nothing to *manage* with one translation |
| **Bible Metadata** | ✅ partly — book/chapter counts are real and shown; there is no per-book metadata (author, date) in the corpus and none is invented |
| **Search History** | **NOT BUILT** — nothing persists queries. It needs a store that does not exist, and inventing one from the detection log would show fired verses, not searches |
| **Download Translations** · **Offline Packages** · **Translation Import** | **NOT BUILT — one blocked problem, not three screens.** All three are the same missing capability: getting a second Bible onto the machine. That needs a source, a licence, and a format decision (`docs/DECISIONS.md` records none). A downloader pointed at nothing, or an importer with no defined format, would be three screens pretending a capability exists |

## Still off / not verified

- **The browser has never been seen.** The console webview cannot be captured on
  this machine, so `Browse.svelte` is a code-level build. The *backend* it stands
  on is verified against the real 31,100-verse corpus (canonical order, real
  chapter counts, Genesis 1 = 31 verses in order, missing chapters return empty
  rather than erroring).
- **Put on screen** calls the same `manual_fire` the console uses. It was not
  exercised from this screen against a running output.

---

# Pass 2 — the ProPresenter rebuild

Asked for: *"a full rebuild of the Library, ProPresenter style, where you can see
what was pushed to the screen from the library."*

## What ProPresenter actually gets right

Not the layout — the two things underneath it:

1. **You pick a slide by LOOKING at it.** A list of references ("1:1, 1:2, 1:3")
   makes you read and imagine. A grid of rendered slides makes you *recognise*.
   Under pressure, in a dark booth, recognition is the only one that works.
2. **The live slide is visibly the live slide.** You never have to remember what
   you sent.

## Why Relay can do this honestly

`TemplateRender` is THE renderer — the same component draws the fullscreen
output, the Templates editor preview, and now these thumbnails. A thumbnail is
therefore **not an approximation** of what will appear; it is the same code at a
different size. Change the template and every thumbnail changes with it, by
construction. Verified in the capture: the real *Classic Serif* template, its
gold verse colour, its reference-first layout and its auto-fit, all in a 190px
cell.

Built:

- **`SlideGrid.svelte`** — rendered slides, click to fire, paginated at 24
  because 176 live template instances (Psalm 119) is not free and pretending
  otherwise would stall the tab.
- **`LiveStrip.svelte`** — what is on the wall, *inside the Library*. The console
  has a program monitor, but it is on the Live tab; an operator browsing the
  Bible mid-service had to leave what they were doing to answer the single most
  important question in the product.
- `Browse.svelte` is now **slides-first**, with Read as the alternate view.

## The colour law did the design work here

- **Amber ring + ON AIR badge** — the congregation is looking at *this* slide.
  This is the one legitimate use of amber (DECISIONS §22) and it is what makes
  the library answer "what is up there?".
- **Amethyst instead, in rehearsal** — the same slide is showing, but nothing is
  reaching anyone, so it must not wear the tally colour. The badge reads
  REHEARSAL.
- Clicking a slide **fires it** — not a preview, not a selection. Labelled that
  way, and refused outright in safe mode.

## A bug the capture caught — in the harness, not the app

The first render produced **empty thumbnails**: background gradient, no text. The
cause was my capture fixture reading `style_json` straight out of SQLite and
never supplying `layout`. The backend serialises `Template { layout: Value, style:
Value }` — already parsed — so the real app was always passing the right shape.

Worth recording because the failure looked exactly like a broken component, and
the wrong conclusion ("TemplateRender can't render small") would have sent the
next hour into the renderer instead of the fixture.

## Still not verified

- The assembled Library **tab** has not been seen — the console webview cannot be
  captured here. `SlideGrid` and `LiveStrip` were mounted directly against real
  data pulled from the running app's database (real Psalm 23, real active
  template), which is the closest available.
- **No slide has been clicked against a live output.** Firing goes through the
  same `manual_fire` the console uses, and the e2e tests cover that path, but the
  click was not exercised from this screen.
