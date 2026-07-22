# Library — design-match loop log

Reference: `docs/relaydesign/relay-library-screen.png` (hi-fi full-screen mockup).

Gate on every iteration: `npm run build` clean, `npx vitest run` — **206 passed**.
Compare was **pixel**: the real `App.svelte` shell was mounted on the Library tab
in a headless Chromium against a fixture standing in for the Tauri bridge
(canonical book order + real chapter counts + the real KJV text of Genesis 1).
Captures: `library-1.png` → `library-3.png`.

---

## Iteration 1 — the rebuild

The Library had the right *content* and the wrong *shape*: one search box, a
row of coloured sub-tabs, a preview strip, and a two-pane book/chapter browser.
The reference is a three-pane catalogue with a filter bar.

Built:

- **Row 1** — content-type pills (`.r-pill`, new shared class) + Import / New.
- **Row 2** — the filter bar: translation · search · book · chapter.
  Book and chapter state was **lifted into `Library.svelte`** and bound down to
  `Browse.svelte`. They are the Bible pane's real navigation, and a select and a
  book row that disagree about which chapter is open is worse than either alone.
- **`Browse.svelte`** rebuilt as books · chapter · inspector, with a verse-card
  list, a `Slides / Read / Table` segment (`.r-seg`) and 25-per-page pagination.
- **`VerseInspector.svelte`** (new) — the right pane: verse hero, details,
  translations, actions, info grid.
- `.r-pill` · `.r-seg` · `.r-chip` added to `app.css` as shared classes, so the
  Planner cannot later drift on radius or height.

Diffs found in capture 1: inspector empty (nothing selected), cards darker than
their panel (the reference has it the other way round), no per-card action, and
the fixture only held 10 verses so the pager never appeared.

## Iteration 2 — surfaces and the on-air rule

- Panels moved to `--v-bg`, cards to `--v-surf` — the reference's stacking.
- Per-card **bookmark** button (real `saveScripture`); card became a
  `role="button"` div so a real button can live inside it.
- **The live row is an amber BAR and an amber badge, not an amber card.** The
  reference paints the whole live row gold. A gold field behind body text is
  hard to read and louder than the badge carrying the actual meaning; the bar
  survives hover and selection, so "what is the congregation looking at" is
  never overpainted.
- Fixture extended to the real 31 verses of Genesis 1, so the pager renders.

## Iteration 3 — widths

Books 186→180, inspector 396→384, giving the verse column the reference's
proportion. No new fixable diffs. **Stopped** (stop condition 1).

---

## Deliberate departures from the reference

Each one is a claim the app cannot honestly make, not a styling choice.

| Reference | What was built | Why |
|---|---|---|
| Per-verse chips `KJV` · **Direct** · **High Confidence** | `KJV` · reference · word count | Direct/High-Confidence are **detection** verdicts. A verse you scrolled to in a library was not detected by anything; printing a confidence claim over it is a lie about what the AI did. |
| **TAGS** with `Direct Match` / `Creation` | **DETAILS** with factual chips | There is no tag store. A tag UI over nothing is a feature that does not exist. |
| **TRANSLATIONS**: NIV · ESV · Yorùbá · Swahili | The translations that exist (KJV), plus the note | **Relay ships one Bible.** Painting four rows makes exactly the claim `docs/LANGUAGES.md` refuses to make. Detection *recognises* spoken Yorùbá/Kiswahili/Hausa references — the alias table is real — what is missing is verse TEXT, a sourcing and licensing problem. |
| Verse hero always gold | Gold **only while genuinely on air** | Amber is the tally light (CLAUDE.md). Amber that is always on cannot warn. |
| Selected book row gold | Selected book row amethyst | Same rule: selection is chrome. |
| `Queue Next` · `Add to Plan` | `Next Verse` · `Previous` | Both back the existing chapter-step path. A queue and a plan-add from this pane are functionality, and the brief was to match the design without advancing it. |
| `…` overflow button, `All Types`, `Filters ⇅` | omitted | Nothing defined sits behind them. An inert control that opens nothing is worse than an absent one. |
| Segment defaults to **Slides** | defaults to **Read** | The reference's "Slides" pane *is* a verse-card list. Relay's Slides view is the rendered-thumbnail grid (`SlideGrid`), which the reference never shows, so Read is the view that matches the picture. |

## Not verified

- **Never seen in the real desktop window.** The capture is the real component
  tree in a headless browser; the Tauri webview cannot be screenshotted on this
  machine (CLAUDE.md).
- **No verse was fired against a live output from this screen.** `Take to
  Screen` calls the same `manual_fire` the console uses and `e2e.rs` covers that
  path, but the click was not exercised end to end here.
- `npm run tauri build` was not run — nothing here is CSP- or bundle-sensitive.

## Inferred values (not read off the design system)

- `.r-pill` height 34px / `.r-seg` button height 28px / `.r-chip` height 21px.
- Panel column widths 180 / 1fr / 384, and the pane height
  `clamp(420px, calc(100vh - 296px), 900px)`.
- Card `--v-surf` on `--v-bg` panel; live row tint `rgba(255,176,0,.05)` with a
  3px `--v-amber` bar.

---

# Pass 2 — the run surface (asked for directly)

Captures `library-4.png` (one-click fire + live column), `library-range-1.png`
(the typed range), `library-5.png` / `library-6.png` (range cleared).
Gate: `npm run build` clean, **216 frontend tests** (10 new in `passage.test.js`).

## What changed

- **The right column is now Preview · Program · what the AI heard**
  (`LivePanel.svelte`), not the reference's info grid. Book, chapter, verse,
  word count and an ID are all readable off the card the operator just clicked;
  *what is on the wall* is the one question the Library could not answer without
  leaving the tab.
- **One click goes live.** Clicking a verse fires it.
- **A suggestion never does.** Detected references land in the column with
  Preview · Accept · Dismiss. A paraphrase carries **no percentage** and reads
  cyan (CLAUDE.md §18).
- **Typed passages.** `Ps 23 1-5` → Psalms 23:1–5, filtered inside the chapter;
  clearing the box drops the filter and leaves Psalm 23 exactly where it was.
  Verified in the harness: `RANGE heading: Psalms 23:1–5 | cards: 5` →
  `CLEARED heading: Psalms 23 | cards: 25`.
- **Book · chapter · verse pickers** in the filter bar. The verse picker
  *scrolls*; it does not fire. Choosing where to look and choosing what a
  congregation sees are different decisions.

## The decision this reverses — recorded, not hidden

`PreviewProgram.svelte` was built because **Relay used to fire on a single
click, and one slip of a trackpad put the wrong scripture in front of a
congregation with no undo.** The switcher (stage → look → take) was the fix.

One-click firing was asked for deliberately, and there is a real argument for
it: browsing the Bible mid-sermon is a chosen act, and `SlideGrid` has fired on
click all along — so the two halves of this pane contradicted each other. They
now agree. What makes it survivable:

- the **Program monitor is beside the list**, so a misfire is seen, not discovered;
- the card says `Go live →` on hover, before it is clicked;
- **safe mode refuses** outright;
- `Esc` still clears.

If this proves wrong in a real service, the switcher is one line: drop the
`fire(v)` call from `select()` in `Browse.svelte`.

## Where the book name is resolved

In **Rust**, not here. `passage.js` decides only that a range was typed and
where it ends; `searchScripture(probeReference(p))` resolves "Ps" → "Psalms"
through `detection::detect_direct` and the alias table. A second alias table in
the frontend would drift, and typing a reference would eventually answer
differently from saying it out loud — which is the whole product.

## Still not verified

- Not seen in the Tauri window (cannot be captured on this machine). The dev app
  is running and prints `console: webview up (operator)`.
- **No verse has been fired at a real output from this screen.** The click path
  is the same `manual_fire` `e2e.rs` covers.

---

# Pass 3 — the two halves disagreed

Capture: `library-slides-1.png`.

The Slides view was the last thing still running on the OLD rule, and the
screenshots showed exactly what that costs: slide `Psalms 23:1` wearing a
**PREVIEW** badge while the Preview monitor two inches to its right read
**"Nothing staged"**. The same pane answered the same gesture two different
ways, and disagreed with itself in writing about what was about to reach a
congregation.

Cause: two sources of truth. `SlideGrid` staged on click into the *shell's*
preview state; the Preview monitor reads the *Bible pane's* staged state, which
browsing no longer writes to.

Fixed:

- **`SlideGrid` fires on click**, like the Read list. The double-click-to-take
  path is gone, and the header comment now records the reversal instead of
  describing behaviour the component no longer has.
- **`selectedKey` means one thing**: the slide staged in the Preview monitor.
  That only happens when an AI suggestion is staged there — never as a side
  effect of browsing. The dead `selectedKey` prop was removed from `Browse`.
- Cells `300px` → `204px` minimum. At 300 a six-verse psalm filled the pane with
  two enormous cards; it now reads as a contact sheet (3 columns beside the live
  column here, 4 on a wider window).
- A `GO LIVE →` hint on hover, matching the Read cards.
- The native `title` tooltip is gone — it drew a browser tooltip over the
  sidebar (visible in the operator's screenshot, sitting on top of "Dashboard")
  and only repeated what the card already says. `aria-label` carries it now.

**Lyrics, Media and Announcements also fire on click**, because they share
`SlideGrid`. They keep the Preview/Program strip above them, so a misfire is
still visible immediately.

---

# Pass 4 — the dead strip, and responsiveness

Captures: `library-resp-wide.png` · `library-resp-mid.png` · `library-resp-narrow.png`.

## The bug

Both monitors stopped short of the right edge of their own panel, leaving a
dead strip beside them. Cause: `aspect-ratio: 16/9` **plus** a `max-height`
cap. Given both, the box shrinks its **width** to keep the ratio — so capping
the height silently narrowed the screen.

Removed the cap. The screen is 16:9 and full width, which is the only honest
shape for it: a preview that is not the shape of the wall is not a preview. The
column scrolls instead when it is short.

Measured at three widths — screen width vs panel width, borders aside:

```
wide    screen 382 / pane 384  FULL WIDTH ok
mid     screen 318 / pane 320  FULL WIDTH ok
narrow  screen 298 / pane 300  FULL WIDTH ok
```

## Also

- The empty suggestion panel was a tall blank box under the monitors. It now
  takes one line when there is nothing to show and gives the height back.
- **≤1140px** the BOOKS column is dropped — the filter bar already carries a
  book picker, so nothing becomes unreachable, and the width goes to the verse
  list and to what is on the wall.
- **≤860px** the columns stack and the live column goes **above** the list
  (`order`), with the two monitors side by side. What is on the wall must not be
  the thing you scroll to the bottom to find. Below 560px they stack too.

---

# Pass 5 — one shape for every content type

Captures: `library-tab-media.png` · `library-tab-announcements.png` · `library-tab-lyrics.png`.

The Bible tab had the layout; the others still had the old one — a pair of
enormous side-by-side monitors above a small grid, which on the Media tab meant
two 700px-wide scripture previews above three thumbnails.

## The shell

The live column moved OUT of the Bible pane and INTO `Library.svelte`, so every
tab is now `catalogue | Preview · Program · what the AI heard`. Panes render only
their own content.

**Preview holds AI suggestions only.** Browsing fires, so staging something the
operator already chose would just mirror Program and make the pair meaningless —
two monitors are worth having only when they can differ. That is why it reads
"Nothing staged" while Program is live.

## Media, rebuilt (`MediaLibrary.svelte`)

- **Type rail** — All · Images · Video · Documents with counts, the book list's shape.
- **Real thumbnails.** An `<img>` for a picture and a `<video preload="metadata">`
  for a video, both fetched from the app's own server on :8032 — the browser
  paints the first frame. No stored poster exists and none is invented: the
  tile is the file, not a drawing of it.
- **Nothing invented.** `media_assets` stores id · kind · filename · path ·
  created_at and NOTHING else — no duration, dimensions, size or thumbnail. So
  none are printed.
- **Documents are visible and plainly not armed.** `fire_media` refuses anything
  that is not an image or video ("documents can't be shown as an output
  background yet"); a PDF tile that looks armed and errors on click is worse.
- **FILE MISSING.** A row is a POINTER to a file on disk, and the two can part
  company. `/media/<id>` then 404s and the tile would ship a broken-image box,
  which tells an operator nothing. It now says so and refuses to fire. Verified
  by accident and then on purpose: the capture ran against the real running app's
  media server, and the two fixture ids with no file on disk rendered the
  missing state while the three real ones rendered their photos.
- One-click go live, amber tally, two-step delete.

## Announcements, rebuilt

Same card language as the verse list — one click fires, amber bar for on air,
edit and delete as separate buttons. The old markup nested a `<button>` inside a
`<button>` (invalid HTML) and painted its "To output" action amethyst, which is
chrome, not an on-air action.

`New → Draft announcement` works again: the tab rendered a generic slide grid
with no editor, so the menu item led nowhere.

## Two file-format bugs, both real

1. **`mime_for` did not know four extensions the importer accepts** — `bmp`,
   `mkv`, `pdf`, `ppt`/`pptx` were served as `application/octet-stream`, so a
   browser source would not play the video or draw the image; it would offer a
   download and the screen would stay black. Added, plus `ogv`, and pinned by
   `channels::mime_covers_every_imported_kind`, which fails if the importer ever
   accepts an extension this table does not know.
2. **The file picker was narrower than the importer.** `accept` omitted `.bmp`,
   `.avif`, `.svg`, `.mkv`, `.m4v`, `.pro5` and `.key` — all handled by the
   routing code — so those files were greyed out in the dialog and could not be
   chosen, with nothing to say why. The picker list is now derived from the
   router's own arrays: one list, one place.

Dead code removed: `ContentSlides.svelte` and the old `Media.svelte`.

Gate: `cargo fmt` · `clippy -D warnings` clean · Rust tests pass · `npm run build`
clean · 216 frontend tests.

## Still not verified

- No media has been fired at a real output from this pane. `fire_media` is the
  same command the Planner uses.
- `/media/<id>` reads the whole file into memory and has **no range support**
  (`channels.rs`), so a long video will be slow to start and cannot be seeked.
  That is pre-existing and untouched here.

---

# Pass 6 — the black Program monitor

Captures: `library-final-lyrics.png` · `library-final-media.png`.

## The bug the screenshots showed

A picture was on the wall. The topbar said **ON AIR**. The Program monitor —
the one control whose entire job is answering *what are they looking at* —
was **black**, and the label read `content`.

Two causes, both real:

1. **`PreviewProgram` dropped `media_url` and `media_kind`.** It passed only
   `reference`, `text` and `translation` into `TemplateRender`, so a fired
   picture rendered as an empty template. Both monitors now pass the media
   fields through, and there is a raw `<img>`/`<video>` fallback for the case
   where no template has loaded yet — a picture is still a picture, and the
   monitor must never go black while something is on a screen.
2. **`fire_media` sends an EMPTY reference** (`main.rs`), so every image was
   labelled "content" in the topbar and "Content" on the monitor. Both now name
   what it actually is — *picture* or *video*. Done in the frontend on purpose:
   putting the filename in `reference` would print `Screenshot 2026-07-19 at
   00.33.33.png` across the congregation's screen, because the template renders
   that field.

Verified in the harness against a real image served by the running app:

```
PROGRAM label: "Picture" | renders media: YES
topbar live: picture
```

## Lyrics, brought into line

It was the last pane still on its own layout: a fixed `height: clamp(...)` that
ran the slide grid off the bottom of the card with the final row cut in half and
nothing to scroll, no panel header, and — when there were no songs — bare text
on the void, which reads as broken rather than empty.

Now the same shape as every other tab: a **Songs** rail with a count in its
footer, a header naming the song and its slide count, internal scrolling, and
the loading and empty states wearing the same panel as the loaded pane.
Selection is the solid accent, matching the book and type rails.

Gate: `npm run build` clean, 216 frontend tests, `cargo fmt`/`clippy` clean
(no Rust changed in this pass).

---

# Pass 7 — Lyrics rebuilt, and the Library finished

Captures: `library-lyrics-edit.png` · `library-lyrics-deck.png`.
Gate: `npm run build` clean, **233 frontend tests** (17 new in `reflow.test.js`).

## The thing that was actually wrong

Relay stored SECTIONS and projected them verbatim. A section is what a song
*is* — Verse 1, Chorus. A slide is what fits on a wall. They are not the same
object, and conflating them has one of two outcomes, both bad: a nine-line verse
goes up as nine lines of unreadable text, or the operator hand-splits it into
fake "sections" that then lie about the song's structure in the plan, in
history, and in every arrangement built on it.

ProPresenter, EasyWorship and OpenLP all keep the two apart. Relay now does too.

## `lib/reflow.js` — pure, tested, 17 cases

`parseLyrics(text) → sections`, `toText(sections) → text`,
`reflow(sections, {linesPerSlide, maxChars}) → slides`.

No Svelte, no Tauri, no DB — the rule that decides what a congregation reads is
testable without a window, exactly like `detection.rs`. Pinned behaviours:

- A blank line ends a section; `[Chorus]`, `Chorus`, `chorus:` and `V2` all name one.
- Unnamed blocks number as verses, counting **only** the unnamed ones.
- A named-but-empty section keeps its slide — deleting what someone just typed
  is the one thing an editor may never do.
- A broken section says **which part** it is (`Verse 1 (1/2)`), because an
  operator who cannot see that a chorus is 1 of 2 will cut it short.
- `maxChars` splits at a LINE boundary, never mid-word.
- A nonsense rule (0, −3, NaN) cannot empty the deck or loop.
- `parseLyrics(toText(s))` is stable.

## The pane (`LyricsPane.svelte`)

Songs rail · deck · the shell's live column. Edit opens a split: the lyric text
on the left, and **the deck rebuilding on the right as you type**. Verified in
the harness — retyping the song re-broke the deck live and the header switched
to `unsaved`; changing lines-per-slide re-broke it under the operator's eyes
(3 slides at 4 lines → 5 at 2 → 3 at 6).

Two decisions worth keeping:

- **Lines-per-slide is persisted with the session.** It decides what a
  congregation reads; resetting it to a default every time the tab opens would
  silently change the deck.
- **Editing does not move the wall.** `fire_content` sends the TEXT of the slide
  at the moment it is fired, so edits reach the screen only when the operator
  fires again. While a slide of the open song is live, the editor says so in an
  amber strip — a statement of fact, not an alarm, so it is a strip and not a
  modal nobody reads under pressure.

## Saved scripture, brought into line

It carried its OWN search box, so the Library had two: the shell's, which did
nothing on that tab, and this one, which looked like a duplicate and behaved
differently. There is one search box now. Saved verses fire on click like every
other card, with the amber tally and a two-step remove.

## A real bug found on the way

`Browse.svelte`'s bookmark button called `saveScripture(refOf(v))` — ONE
argument against a `(book, chapter, verse)` signature. `"Genesis 1:1"` arrived
as the book with chapter and verse undefined, so **every bookmark click failed**.
Now passes the three fields.

Dead code removed: `LyricSlides.svelte`, and the unused `Lyrics.svelte` import.

## Known regression — say it plainly

`Lyrics.svelte` was the only mount point for `SongEditor.svelte`, which is where
**arrangements** (named section sequences) were edited. That screen is now
unreachable from the Library. It was not deleted. Wiring arrangements into the
new pane is a deliberate next step, not an oversight — putting the whole old
editor back would give the Library two competing lyric editors, which is the
inconsistency this pass existed to remove.

---

# Pass 8 — rebuilt from the reference, literally

Asked for: rebuild the Library screen from scratch against
`relay-library-screen.png`, chosen over keeping the Preview/Program column.
Capture: `library-rebuild-final.png`.

`Browse.svelte` and `VerseInspector.svelte` written from zero.

## Now the mockup's, section for section

- Three panes: **BOOKS** rail (counts, Browse All Books footer) · the chapter ·
  the **verse inspector**.
- Inspector: reference + tally + bookmark + close · gold hero with the
  translation under it · **TAGS** · **TRANSLATIONS** · **ACTIONS**
  (Take to Screen · Queue Next / Add to Plan · Add to Favorites · Clear Screens)
  · **INFO** two-column grid. Verified: `['TAGS','TRANSLATIONS','ACTIONS','INFO']`.
- Filter bar: translation · **content type** · search · book · chapter · verse ·
  **Filters**. Plus the `…` overflow beside Import / New.
- `Slides / Read / Table`, 25 per page, `Showing 1–25 of 31 verses`.
- **The Preview/Program column is gone from the Bible tab** — the inspector has
  that space, as in the mockup. Every other tab keeps it (they have no
  inspector, and losing the answer to "what is on the wall" everywhere would be
  a straight regression).

## Controls that were inert in the mockup now do something real

Rather than draw a control that opens nothing:

- **All Types** → the content type, bound to the sub-tab. One source of truth
  instead of two selectors that could disagree.
- **Filters** → Favourites only. The one axis the Bible pane has that is not
  already a control on this bar.
- **…** → Reload this list · Data health (jumps to Settings).
- **Queue Next** → sets where the ‹ › step walks from. Nothing reaches a screen,
  and the inspector reads **CUED in grey** — never amber, which would be a lie.
- **Add to Plan** → a menu of the real plans; adds a scripture cue via
  `addPlanItem`.

## What is still NOT the mockup, and will not be

Three values, each of which would be false:

1. **TRANSLATIONS** — the mockup lists NIV, ESV, Yorùbá and Swahili. Relay ships
   the KJV and nothing else. Printing verse text under those four names means
   printing words those translations do not contain, on the screen an operator
   reads from before speaking to a room. That is misattributed scripture, not a
   styling shortcut. The section is built from the translations that exist and
   states the gap.
2. **`Direct` / `High Confidence` chips** — detection verdicts. Nothing detected
   a verse someone scrolled to and no confidence was computed for it. The chips
   carry facts: translation, reference, length.
3. **Gold selection** — gold means ON AIR. Selection is the accent; only the
   live row and a genuinely live hero are amber.

Everything else in the reference is built.

Gate: `npm run build` clean · 233 frontend tests. The build gate earned its keep
mid-pass: a `setSession` call without its import failed the build outright
(`missing-declaration`) instead of shipping a button that throws on click.

---

# Pass 9 — rebuilt against `relay-main-library-screen.png`

A new, much richer reference. Capture: `library-main-1.png`.
Gate: `npm run build` clean · **247 frontend tests** (14 new in `queue.test.js`).

## Built

- **Six pills** — Bible · Saved · Lyrics · Media · Announcements · **Graphics** —
  Import, **New Item**, `…`.
- **BOOKS (n)** rail with an *All Books* row and totals.
- **The deck** (`VerseDeck.svelte`): cards that ARE the slide, drawn by
  `TemplateRender` — the same component that paints the projector, so a card is
  the thing itself at a different size. Select checkbox, favourite star, footer
  (number · reference · kebab), three layouts (grid · large · list), Sort,
  pagination, **Items per page**.
- **Bulk select** → *Queue N selected*.
- **The right rail** (`LiveOutputRail.svelte`): LIVE OUTPUT preview + Take to
  screen · OUTPUT HEALTH · VERSES IN QUEUE · QUICK ACTIONS.

## The queue is real (`lib/queue.js`, 14 tests)

It did not exist. Relay had a passage CURSOR (walk on from the verse you fired)
and a service PLAN (built on Tuesday) and nothing in between for *"these four,
in this order, in a minute"* — which is precisely the moment a preacher names
four references in one sentence.

Pure array operations, tested without a window. Pinned: no duplicates; **move
never wraps** (wrapping would send the head of the queue to the bottom on a
mis-click, mid-service, with no undo); drag-drop clamps rather than dropping the
row; `take` on an empty queue is not an error and does not mutate.

Verified end to end in the harness: queue two verses from card menus → 2 rows →
reorder → head becomes `Genesis 1:3`.

## OUTPUT HEALTH: the reference's numbers do not exist, and are not drawn

The mockup prints **FPS 60 · Latency 32 ms · Dropped Frames 0 · Bitrate
15.2 Mbps**. Relay measures none of them and cannot: output is a webview
painting a template, not an encoder — there is no frame pipeline to instrument
and no stream to sample. NDI, which would have one, is parked for want of a
proprietary SDK.

Printing them anyway would be the worst class of dishonest UI. It is not
decoration: an operator glancing at "0 dropped frames" reads it as *the
projector is fine*, and would keep believing it while a screen sat black. **A
health panel that cannot fail is a picture of one.**

So the panel reports what Relay genuinely knows, and each line can say something
is wrong: open output windows against configured channels, whether anything is
on screen, the kiosk and browser ports, safe mode, rehearsal. It states in one
line why there is no frame rate.

## Quick actions — every one of them does something real

`Send to Preview` · `Go Live` · `Hold AI` (stops the AI firing by itself; manual
control untouched) · Blank Screen · Clear Screens · Countdown · Rehearsal.

Dropped from the mockup: **Logo Overlay** and **Test Pattern** — neither exists,
and a button that does nothing is worse than an absent one. **Go Live is amber,
not the mockup's green**: green means *confirmed* in this app, and a control
that puts scripture in front of people is amber, always.

## Graphics

Not a new store — the image half of `media_assets`. ProPresenter draws the same
line: a still you put behind words is a different job from a video you play, and
mixing them means hunting past twenty MP4s for a logo. Media now shows video and
documents; Graphics shows stills. One table, two views, nothing invented.

## Not built from the reference

- **Topbar chips** (`Outputs: 3 Online`, `Stage Display: Connected`) — app shell,
  not the Library screen. The same facts now live in OUTPUT HEALTH.
- **Sidebar STORAGE meter** (`248 GB free of 1 TB`) — nothing reads disk space;
  it needs a Rust command that does not exist yet.
- Fabricated translations and detection-verdict chips, as in every pass before.

Removed: `VerseInspector.svelte`, `LivePanel.svelte` (both superseded by the rail).

---

# Pass 10 — the card fires, the kebab edits, the numbers count

Capture: `library-cards-edit.png`.

## 1. Clicking a card goes LIVE

It selected the verse. A card is not a thumbnail to enlarge — it is already the
slide at a readable size, and the operator's next action after finding it is
always the same one. The list layout fires too, so both agree. `GO LIVE →`
appears on hover, and safe mode refuses. Verified: clicking card 5 →
`Genesis 1:5 is on the screens`.

## 2. The kebab edits the deck

`Take to screen · Add to queue · Add to favourites` — then a separator, then
**Edit slide… · Duplicate slide · Add slide after**.

**Editing never rewrites scripture.** `chapter_verses` returns the KJV, and the
KJV is not something an app may quietly amend: an edit saved into the corpus
would silently change what every future service, plan and detection match reads.
So an edit is an OVERLAY — a slide of its own, badged **EDITED** in amethyst,
fired through `fire_content` as ordinary text with the scripture template, while
the verse underneath stays exactly as printed. Duplicates and blanks are
inserted after their verse the same way.

These live for the session, like the queue. Persisting them needs a store that
does not exist, and inventing one silently is how a library ends up with two
Genesis 1:1s that disagree.

The editor is `role="dialog"`, so **Esc dismisses it instead of clearing the
wall** (CLAUDE.md §16 — `shortcuts.js` checks for a mounted dialog).

## 3. Slide numbers are slide numbers

The footer printed the VERSE number. That drifts the moment anything is
inserted, filtered, sorted or searched — and then two slides on screen wear the
same number, which is the one thing a numbered deck must never do. It is now the
position in the deck, counting on across pages, with the reference beside it.

Verified: `1,2,3,…,12` → duplicate inserted at position 3 → still `1,2,3,…,12`
with the deck at **32 slides**, and the copy reading `Genesis 1:2 (copy)`.

Gate: `npm run build` clean · 247 frontend tests.

---

# Pass 11 — "it still enlarges when I click" was not the click

The operator's screenshot showed the whole window filled by one giant verse with
`Send to Preview` and `Go Live` floating at the bottom. Read as "the card
enlarges on click". It was neither the card nor the click.

**What it actually was.** The webview was **zoomed in**, so its CSS viewport fell
under the Library's 860px breakpoint. At that width my own rule stacked the live
rail ABOVE the deck — and the rail leads with a 16:9 monitor, which at full
width is a ~400px-tall slide. The giant verse was the LIVE OUTPUT preview. The
deck was still there, 1300px below the fold.

Measured before the fix:

```
1728px  pane top=192   | live screen 370x208
 820px  pane top=1360  | live screen 704x396   <-- deck off screen
```

After:

```
 820px  pane top=292   | live screen 418x235
```

**Two rules, both learned the hard way in this file:**

1. **The deck stays first when stacked.** "What is on the wall matters most"
   read well in theory, but it put a 400px slide where the library should be and
   left the operator on a screen with no library on it and no way to tell why.
2. **The rail is WIDTH-capped, not height-capped.** Capping the height of a box
   with `aspect-ratio` shrinks its WIDTH instead and leaves a dead strip beside
   it — that exact bug is in this log twice already (pass 4).

The click itself was verified twice over before this: the compiled component the
running dev server serves contains `const click_handler_4 = v => onFire(v)`, the
compiler reported `onPick` unused, and the harness produced
`Genesis 1:5 is on the screens`. `onPick` and `pick` are now deleted outright, so
a card click has exactly one possible destination.

**Worth knowing:** webview zoom persists across restarts, so a stray Cmd-+ will
keep the console in its narrow layout until it is reset.

Gate: `npm run build` clean · 247 frontend tests.

---

# Pass 12 — the real cause: an absolutely-positioned slide with nowhere to anchor

Two screenshots, four seconds apart, settled it: the Library rendered correctly,
then a card was clicked and one verse covered the entire console. So the click
was firing all along — and the LIVE monitor's slide was escaping its own box.

**`TemplateRender`'s root is `position: absolute; inset: 0`.** `.lo-screen` — the
monitor I wrote from scratch this pass — had no `position: relative`. So the
slide resolved its `inset: 0` against the nearest positioned ancestor, the shell,
and painted across the whole main area. `PreviewProgram`'s `.pp-screen` had
always carried that `position: relative`; writing a new monitor dropped it, and
nothing failed until content was actually fired into it.

That is why every earlier diagnosis missed: the deck, the grid and the click
handler were all correct. Verified by measurement rather than by eye —

```
lo-screen           {"w":370,"h":208}
stage inside it     {"w":370,"h":208}    CONTAINED ✓
```

**And the kebab.** `.vd-card` had `overflow: hidden`, which CLIPPED the menu to
the card — the edit actions opened half-cut, which is what "the 3-dot button is
not aligned" was describing. The card no longer clips; `.vd-shot` does its own
rounding, and the footer rounds its own corners. Menu measured fully on screen.

**The lesson, for the next monitor anyone writes:** an element that hosts
`TemplateRender` must be `position: relative`. It is load-bearing, it is
invisible until something is fired, and it fails by covering the entire
application rather than by breaking anything the eye can trace back.

Gate: `npm run build` clean · 247 frontend tests.

---

# Pass 13 — the same treatment for all five remaining tabs

Captures: `library-all-{saved,lyrics,media,announcements,graphics}.png`.
Gate: `npm run build` clean · 247 frontend tests.

`VerseDeck` became THE deck — one card for every content type in the Library.
The operator's job is identical in all six cases (find it, see it, put it on the
wall), and six browsing metaphors would be six things to learn under pressure.

Every tab now has: the rail · the deck · the shared live column · one-click go
live · checkbox multi-select · the kebab · slide numbers by deck position · the
amber tally · the queue.

| Tab | Cards | Kebab offers |
|---|---|---|
| Saved | 2 | take · queue · favourite · edit · duplicate · add |
| Lyrics | 2 | take · queue · edit · duplicate · add |
| Media | 2 | take · queue · **delete** |
| Announcements | 2 | take · queue · edit · duplicate · delete |
| Graphics | 1 | take · queue · **delete** |

## The menu offers only what the type can honestly do

`can={{ queue, favourite, edit, duplicate, add }}` per pane, rather than one menu
everywhere with dead items in it.

- **Media and Graphics have no edit or duplicate.** There is no rename command
  and no file-copy command; a picture is a file on disk. Delete is real, so
  delete is offered.
- **Announcements have no "add slide after"** — a notice is a whole row, not a
  slide in a deck.
- **Favourites are scripture-only.** `saved_scripture` is a scripture table.

## Where an edit WRITES, and where it overlays

- **Lyrics edit the SONG.** A lyric is the operator's own text. Duplicate and
  add-after go through the same `parseLyrics`/`toText` round trip the text
  editor uses, so the deck and the editor can never disagree about what the song
  is, and Save persists it.
- **Announcements duplicate for real** — `save_announcement` with a new row.
  Again: the operator's own words.
- **Scripture (Bible and Saved) overlays.** The KJV is not ours to rewrite; an
  edit becomes its own slide marked EDITED and the verse stays as printed.

That distinction is the whole rule: **Relay edits what the church wrote, and
never what it did not.**

## Media in the queue stays media

A queued picture carries its `mediaId`, so firing it from the rail is still
`fire_media` — not a text cue that happens to be named after a file.

Dead code removed: `SlideGrid.svelte`, `Lyrics.svelte`, `SongEditor.svelte`
(the last two were the orphaned pre-rebuild lyric screens).

**Arrangements are still unreachable** — `SongEditor` was their only entry point
and it is now deleted. That is a real gap, recorded in pass 7 and still open.

---

# Pass 14 — bigger to read, pinned to reach, named to understand

Captures: `library-scaled-bible.png` · `library-scaled-lyrics.png`.
Gate: `npm run build` clean · 247 frontend tests.

## Previews sized to be READ

Cards `210px → 268px` (large `300 → 380`), 16:10 → 16:9, and the fallback type up
with them. `TemplateRender` scales in container units, so a wider card is
literally larger text — which is the only thing that decides whether an operator
recognises a slide at a glance in a dark booth. Three across that can be read
beats five they have to lean in for. The rail went `372 → 400`, taking the
monitor to 398×224.

## Quick actions are a FOOTER, and carry the transport

Pinned at the bottom of the rail, always in the same place. They now include the
controls an operator used to change tabs for:

- **‹ / Next ›** — the same `nav` command as the console, and it reports the
  outcome: *"End of the passage"* is a boundary, not a button that did nothing.
- **The microphone** — start/stop listening, lit while live. Not amber: amber
  means the congregation is looking at something, and listening is not that.

**Sticky was wrong and got fixed.** `position: sticky` floated the panel OVER the
health readout and hid the queue beneath it — an operator would have been
reading a panel with its bottom half under a row of buttons. It is a flex footer
below a scrolling region instead.

## Output health: one line, not a column

It was a paragraph, a four-cell grid and a disclaimer — a lot of a narrow column
to spend on something that is usually just "fine", and it pushed the queue off
the bottom. The judgement, the chip and `0/7 · :8031 · :8032` stay visible; the
rest is a tooltip, because ports are looked up once, not monitored.

## "LINES 2 3 4 6" → SLIDE SIZE: Large · Medium · Small

The number was an implementation detail of the reflow, and four of them is a
choice nobody wants to make mid-service. Three named sizes in the same segmented
control the Bible pane uses, with the line count in the tooltip
(*"2 lines a slide — biggest type, most slides"*). A stored value outside the
three falls back to Medium rather than leaving the control showing nothing.

---

# Pass 15 — the list view said nothing, and scripture stopped being editable

## The list layout was empty

It printed `text` — which a picture does not have, and a media row therefore
rendered as a number and three icons with no name on it (the operator's
screenshot of Graphics in list mode). Rows now lead with the NAME, carry a
thumbnail for media, and put the text underneath as the secondary line.

## Every pane has the same three layouts

Grid · Large · List, the same control everywhere. Saved, Announcements, Media
and Graphics had two; Lyrics had none.

Verified across the matrix:

```
Bible          grid=12  large=12  list=12(“Genesis 1:1”)
Saved          grid=2   large=2   list=2(“John 3:16”)
Lyrics         grid=2   large=2   list=2(“Verse 1”)
Media          grid=2   large=2   list=2(“welcome-loop.mp4”)
Announcements  grid=2   large=2   list=2(“Midweek service —”)
Graphics       grid=1   large=1   list=1(“lake.jpg”)
```

## SCRIPTURE IS NOT EDITABLE, AND THAT IS THE FEATURE

Edit, duplicate and insert are gone from **Bible** and **Saved** — the whole
custom-slide overlay built two passes ago is deleted, not merely hidden.

Genesis 1:2 follows Genesis 1:1 and says what it has always said. That is what a
Bible IS. An app that lets an operator quietly reword or reshuffle it is not
showing scripture any more, it is showing something that looks like scripture —
and the person in the third row has no way to tell the difference. Relay edits
what the church wrote, and never what it did not.

## …and everything the church DID write is fully editable

**Lyrics** gained **Move up / Move down** on top of edit, duplicate and add — a
chorus goes wherever the band takes it. It writes through the same
`parseLyrics`/`toText` round trip as the text editor, so the deck, the editor and
what gets saved cannot disagree, and it does not wrap at the ends (a mis-click at
the top must not send the first verse last). Verified:

```
before: Blessed Assurance · Verse 1 | Blessed Assurance · Chorus
after : Blessed Assurance · Chorus | Blessed Assurance · Verse 1   MOVE WORKS ✓
```

Menus by type, each offering only what its content can honestly do:

| Tab | Menu |
|---|---|
| Bible / Saved | take · queue · favourite |
| Lyrics | take · queue · edit · duplicate · add · **move up/down** |
| Announcements | take · queue · edit · duplicate · delete |
| Media / Graphics | take · queue · delete |

## Responsiveness, measured

```
1728px card=391 rail=400 | rail on screen ✓ | quick actions visible ✓ | no h-scroll
1400px card=470 rail=400 | ✓ | ✓ | no h-scroll
1200px card=346 rail=344 | ✓ | ✓ | no h-scroll
1000px card=265 rail=300 | ✓ | ✓ | no h-scroll
 820px card=331 rail=460 | ✓ | scroll | no h-scroll
```

Below 860px the rail stacks under the deck and is capped at 78vh, so the pinned
actions are a short scroll away rather than a page away. No horizontal overflow
at any width.

---

# Pass 16 — the lyric title stopped going on the wall

Gate: `cargo fmt` · `clippy -D warnings` clean · **334 Rust tests** (2 new e2e) ·
`npm run build` clean · 247 frontend tests.

## The song title was being projected

`fire_content` set `OutputContent.reference = label`, and the Lyrics pane's label
is `"Blessed Assurance · Verse 1"` — the operator's bookkeeping, printed across
the top of a wall in front of a congregation. Fixed **in Rust**, where the rule
belongs:

```rust
let projected = if kind == "song" { String::new() } else { label.clone() };
```

The label still names the cue in history and in the plan; it just does not go
out. Scripture is the opposite case — the reference IS part of what is being
shown — so it is still projected. Two e2e tests pin both halves:

```
e2e::a_lyric_slide_projects_the_lyric_and_not_the_song_title ... ok
e2e::an_announcement_still_shows_its_title ... ok
```

With the reference gone, `TemplateRender`'s auto-fit gives the lyric the whole
box, so it is bigger and wraps properly — which was the other half of the
complaint. The card's no-template fallback follows the same rule, so a card
still shows what the wall will show.

**`fire_content` is now generic over `tauri::Runtime`** (CLAUDE.md §24). Welded
to the concrete handle it could not be driven from `e2e.rs` at all — the code
that decides what a congregation reads had no test, and could not have had one.

## Removed rather than explained

- **SLIDE SIZE (Large/Medium/Small)** — gone. It was a question the operator does
  not want while looking for a chorus, and neither version of it (2/3/4/6, then
  three names) said what it would do until you tried it. Four lines a slide is
  the size a congregation reads. Still session-configurable, and it belongs in
  Settings if it comes back — a once-a-year decision does not go on the run
  surface.
- **The third layout ("Large cards")** — gone. Grid already sizes itself to the
  column; a third option was one more thing to try.

## The list view names things

Rows lead with the file or slide name; a media row also carries its thumbnail.
Verified: Graphics in list mode reads `lake.jpg`, not a bare row number.

## The monitor can watch any output

Channels are render targets of ONE template engine — main screen, stage display
and a stream lower-third are the same cue drawn three ways. The monitor could
only ever show one, so an operator could not check what the stage display would
look like without walking to it. A picker in the LIVE OUTPUT header now draws it
with any channel's template: `As fired / Main screen / Stage display / Stream
lower third`. It hides itself when there is only one channel.

## Countdown restarts instead of refusing

It threw *"a countdown is already running — clear the screen first"*, which is a
rule the operator has to satisfy by hand while a service is starting — and
restarting IS the common case, because the service slipped five minutes. The
tile now offers 5 · 10 · 15 · 30 minutes, clears first if one is running, and
says which it did.

---

# Pass 17 — the transparency law, and a rail worth the space

Gate: `cargo fmt` · `clippy -D warnings` clean · **332 Rust tests** ·
`npm run build` clean · **256 frontend tests** (9 new in `lowerthird.test.js`).

## THE TRANSPARENCY LAW, enforced in the renderer

A lower-third channel exists to be composited — OBS or an ATEM keys it over the
shot of the preacher. Every pixel it fills that it did not need to fill removes
the preacher from the stream, and **nobody in the building can see that happen**,
because the failure is only visible on the broadcast. So these are renderer
rules, not per-screen settings:

1. **A band template's own background is IGNORED.** Not defaulted — ignored. An
   operator picking a background in the Templates editor must not be able to
   black out a stream by accident.
2. **The band draws only where there are words.** It was painting a coloured
   strip across the bottom of a full-frame photo that had nothing written in it.
3. **A countdown never goes out on a band.** A clock ticking over the preacher
   belongs on the lobby screen, not on the keyed layer.
4. **Media is the one exception.** A fired picture or video is a deliberate
   full-frame choice, so it becomes the background and transparency yields.
5. **Blackout does not black out a band** (`Output.svelte`). On a keyed channel
   "black" would paint an opaque rectangle over the live camera — the opposite
   of what the operator pressed it for. There, the panic control removes the
   BAND, which is all that channel was contributing, and the camera keeps going
   out. Every other channel goes properly black.

`lowerthird.test.js` pins all five. Confirmed the tests fail when the law is
reverted — 4 of 9 go red with the old behaviour restored, so they are pinning
behaviour and not implementation.

## OUTPUT HEALTH deleted

It reported ports and a window count: facts that do not change during a service
and that nobody watches. The reference it came from filled the same panel with
frame rate and bitrate, which Relay cannot measure at all. A panel that is
either static or fictional is worse than no panel — it occupied the space the
queue and the transcript needed. Channel state lives on the Channels tab, which
is where an operator goes to fix it.

## The rail is now the run surface

```
LIVE OUTPUT   pinned top — never scrolls (verified: it does not move when the
              column is scrolled 400px)
HEARD         AI suggestions with Approve / Dismiss in one press
UP NEXT       the queue, any content type, reorderable
TRANSCRIPT    what the microphone is actually getting
ACTIONS       pinned bottom
```

**TRANSCRIPT earns its place**: when detection goes quiet, this is the difference
between *"the preacher has not said a reference"* and *"Relay has gone deaf"* —
and those need opposite responses. Without it an operator cannot tell which they
are looking at.

Actions: **Go Live** (naming what it will send) · Listen · Clear Screens · Blank
Screen · Countdown · Rehearsal. "Send to Preview" was dropped — it did the same
job as *Take to screen* under the monitor, and cost a row the transcript needed.
Four tiles on one row rather than two rows of two gave the scrolling half 75px
back (126px → 201px).

---

# Pass 18 — an empty panel was hiding a live decision

## UP NEXT only exists when there is something in it

An empty panel reading "(0)", plus a sentence explaining that it was empty, was
taking a third of the column — and it pushed the **Approve** button of a live AI
suggestion below the fold. The operator could see that Relay had heard something
and could not reach the control to act on it, which is the worst possible place
to spend space.

The panel now appears only when the queue has contents. `Go Live` names the next
queued item, so nothing is hidden. Measured: Approve now sits at 662px inside a
region ending at 735px — reachable without scrolling.

## The countdown menu was clipped by its own panel

`overflow: hidden` on `.lo-panel` cut the 5/10/15/30 menu in half as it opened
upward — the same bug the card kebab had two passes ago, in a different panel.
`.lo-quick` is `overflow: visible`. Measured: 144px tall, fully inside the
viewport.

## The fitter now shrinks for WIDTH as well as height

It only ever checked height. That is right for prose, which wraps and therefore
gets taller — but a countdown does not wrap: it is one line of tabular digits,
so it can overflow sideways at its own natural height and the loop never
noticed. The check is now `scrollHeight || scrollWidth`, and `.countdown` is
`white-space: nowrap` so the fitter scales the digits instead of letting a
number break across two lines.

**Honest note on this one**: I could not reproduce a countdown overflowing in a
960×540 probe — the height pass already shrank `4:59` to 5cqw, and reverting the
width check changed nothing there. So this is hardening that is correct on its
own terms (a long unbroken word overflows the same way), and the visible
countdown problem in the screenshot was the clipped MENU, which is fixed and
measured. If the timer still renders badly on a real output, it is a different
bug and I have not found it yet.

Gate: `npm run build` clean · 256 frontend tests · 332 Rust tests.

---

# Pass 19 — the app stopped booting, and it was the §25 failure again

Not a UI bug. The background dev task exited 101:

```
thread 'main' panicked at src/main.rs:102:27:
failed to open Relay database: SqliteFailure(..., "duplicate column name: section_title")
```

**A migration panicked the app before the window was shown** — the exact class of
failure CLAUDE.md §25 exists to forbid, arriving from a direction §25 did not
cover.

## What happened

`add_plan_item_column` already does the right thing: it asks
`pragma_table_info` first, so it is idempotent across boots. That is what §25
asks for. But **"already checked" is not "cannot happen"**: two Relay processes
can hold the same file. `tauri dev` respawns on a rebuild while the previous
binary is still shutting down (which is exactly what the log shows — a `cargo
fmt` temp file triggered a rebuild), and a church laptop can have the app
launched twice. Both read the pragma, both saw the column missing, both ran the
`ALTER`. The loser panicked at startup, and would have panicked on **every**
subsequent boot until someone edited the database by hand.

## The fix

The column existing IS the desired end state. Whoever created it, the migration
has succeeded — so a "duplicate column name" error is now tolerated rather than
fatal, in `plans.rs` and in `templates.rs` (the other guarded ADD COLUMN).

Three tests, including one that reproduces the race by adding the column between
the check and the write, and asserts that the bare `ALTER` still fails so the
test cannot pass vacuously.

## Worth stating plainly

I caused this by restarting the dev app repeatedly while an old instance was
still alive. The trigger was mine; **the fragility was already there**, and it
would have found a church eventually — two icons double-clicked on a Sunday is
not an exotic scenario. The operator's database was not damaged: both columns
were present and correct throughout.

Gate: `cargo fmt` · `clippy -D warnings` clean · **340 Rust tests** (3 new) ·
256 frontend tests · app boots clean (`console: webview up (operator)`, no panic).
