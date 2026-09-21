<script>
  // LIBRARY → BIBLE. Rebuilt from docs/design/relay-main-library-screen.png.
  //
  //   BOOKS rail  ·  the chapter  ·  the verse inspector
  //
  // The mockup's three panes, its Slides/Read/Table segment, its 25-per-page
  // footer and its right-hand inspector, section for section.
  //
  // ── Where the CONTENT differs from the mockup ─────────────────────────────
  //
  // Per-verse chips read `KJV · Direct · High Confidence` in the reference.
  // Direct and High Confidence are DETECTION verdicts — claims about what the
  // AI heard. A verse someone scrolled to was not detected by anything and no
  // confidence was computed for it, so those chips would be the app lying about
  // its own work on the screen the operator trusts most.
  //
  // The reference paints selection and the live row gold. Gold means ON AIR
  // (CLAUDE.md), so selection is the accent and only the live row is amber.
  //
  // ── SCRIPTURE IS NOT EDITABLE, AND THAT IS THE FEATURE ────────────────────
  //
  // No edit, no duplicate, no insert, no reorder. Genesis 1:2 follows Genesis
  // 1:1 and says what it has always said — that is what a Bible IS, and an app
  // that lets an operator quietly reword or reshuffle it is not showing
  // scripture any more, it is showing something that looks like scripture.
  // Lyrics, notices and plans are the church's own words and are fully
  // editable; the corpus is not ours to touch.
  import { onMount } from 'svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import ErrorState from '../../ui/ErrorState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import VerseDeck from './VerseDeck.svelte';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import { parsePassage, probeReference, inRange } from '../../passage.js';
  import {
    chapterVerses,
    searchScripture,
    saveScripture,
    listSavedScripture,
    deleteSavedScripture,
    manualFire,
    clearScreens,
    listActiveTemplates,
    live,
    screenBlack,
    rehearsing,
    readErrors,
  } from '../../stores/capture.js';

  /** Canonical book list + chapter counts, loaded by the Library shell. */
  export let books = [];
  /** The translations that ACTUALLY exist in the corpus. */
  export let translations = [];
  /** Bound to the shell's filter bar — one piece of state, two controls. */
  export let book = null;
  export let chapter = 1;
  /** The Library's one search box. */
  export let query = '';
  /** Only verses already in Favourites. Bound: the control is in this pane's
      head now, but the state belongs to the shell so it survives a look at the
      songs and back. */
  export let favouritesOnly = false;
  /** The translations pick, hoisted out of the shell's deleted filter bar. */
  export let activeTranslation = null;
  export let onTranslation = () => {};
  /** Hand the selected verse up for the inspector. */
  export let onSelect = () => {};
  /** The queue lives in the shell, beside the rail that renders it. */
  export let queue = [];
  export let onQueueChange = () => {};

  let verses = [];
  let results = [];
  let savedList = [];
  let template = null;
  let checked = new Set();
  let sort = 'verse';
  // ── GRID BY DEFAULT (REBRAND §10) ─────────────────────────────────────────
  //
  // "Slides in the big grid." A card is the verse drawn through the SAME
  // `TemplateRender` that paints the projector, so what an operator picks is
  // what the room will see; a numbered text row is a description of a slide and
  // an operator reading one still has to imagine the wall.
  //
  // This said `list` and gave a reason — "the grid renders a dozen fit loops at
  // once". That cost was real when it was written and is not any more: the
  // font-refit guard added for rule 42 bounded the retry to two, and the fit
  // loop no longer runs again on every card when a webfont lands. The deck
  // itself records that ("the font-refit guard already removed the real
  // per-grid cost"), so the default and the note beside it had disagreed for a
  // while. `list` is still one click away for a long chapter.
  let layout = 'grid';
  let perPage = 12;
// { id?, ref, label, text, verse }
  let passage = null;
  let selected = null;
  let page = 0;
  let loadingChapter = false;
  let searching = false;
  let firing = '';
  let error = '';
  let msg = '';
  // Transient by design: a "Saved …" / "Queued …" or an error should fade, not
  // linger across later navigation as if it still describes what is on screen.
  // Errors sit longer so they can actually be read. Re-armed on each new message.
  let msgT;
  let errT;
  $: if (msg) {
    clearTimeout(msgT);
    msgT = setTimeout(() => (msg = ''), 4000);
  }
  $: if (error) {
    clearTimeout(errT);
    errT = setTimeout(() => (error = ''), 6000);
  }
  let lastQuery = null;
  let lastPlace = '';

  onMount(async () => {
    savedList = (await listSavedScripture()) ?? [];
    // The template the OUTPUT actually uses, so a card is the real slide.
    template = (await listActiveTemplates().catch(() => []))[0] ?? null;
  });

  // Typed reference → the chapter it lives in. "Ps 23 1-5" filters to 1–5;
  // clearing the box drops the filter and leaves Psalm 23 where it was.
  $: if (query !== lastQuery) {
    lastQuery = query;
    page = 0;
    runSearch(query);
  }
  $: if (book && `${book}|${chapter}` !== lastPlace) {
    lastPlace = `${book}|${chapter}`;
    open(book, chapter);
  }

  async function runSearch(q) {
    if (!q?.trim()) {
      results = [];
      passage = null;
      return;
    }
    searching = true;
    error = '';
    try {
      const typed = parsePassage(q);
      if (typed) {
        // The BOOK is resolved by Rust — "Ps", "psalm", "Sáàmù" and "Zaburi"
        // all mean Psalms, and that alias table drives live detection too. A
        // second copy here would drift, and typing a reference would eventually
        // answer differently from saying it out loud.
        const hit = (await searchScripture(probeReference(typed)))?.[0];
        if (hit) {
          passage = { ...typed, book: hit.book };
          results = [];
          book = hit.book;
          chapter = hit.chapter;
          searching = false;
          return;
        }
      }
      passage = null;
      results = (await searchScripture(q.trim())) ?? [];
    } catch (e) {
      error = humanError(e);
      results = [];
      passage = null;
    }
    searching = false;
  }

  async function open(b, ch) {
    loadingChapter = true;
    error = '';
    page = 0;
    try {
      verses = await chapterVerses(b, ch);
    } catch (e) {
      error = humanError(e);
      verses = [];
    }
    loadingChapter = false;
  }

  function jumpTo(n) {
    const target = Math.floor((n - 1) / perPage);
    if (target !== page && target < pages) page = target;
    requestAnimationFrame(() =>
      document.querySelector(`[data-verse="${n}"]`)?.scrollIntoView({ block: 'center' }),
    );
  }

  async function fire(v) {
    if ($safeMode) {
      // Not a silent no-op: tell the operator WHY the click did nothing, or a
      // disarmed desk reads as a broken one.
      error = 'Safe mode is on — outputs are disarmed. Turn it off in Settings to fire.';
      msg = '';
      return;
    }
    const ref = v.reference ?? refOf(v);
    msg = '';
    error = '';
    firing = ref;
    try {
      await manualFire(ref);
      msg = `${ref} is on the screens`;
    } catch (e) {
      error = humanError(e);
    }
    firing = '';
  }

  /** Favourites toggle — the bookmark in the card and in the inspector. */
  async function toggleSave(v) {
    error = '';
    const hit = savedList.find((s) => s.reference === refOf(v));
    try {
      if (hit) {
        await deleteSavedScripture(hit.id);
        savedList = savedList.filter((s) => s.id !== hit.id);
        msg = `Removed ${refOf(v)} from favourites`;
      } else {
        await saveScripture(v.book, v.chapter, v.verse);
        savedList = (await listSavedScripture()) ?? savedList;
        msg = `Saved ${refOf(v)}`;
      }
    } catch (e) {
      error = humanError(e);
    }
  }

  /** Queue / unqueue. Nothing here reaches a screen. */
  function toggleQueue(v) {
    const ref = refOf(v);
    if (queue.some((q) => q.reference === ref)) {
      onQueueChange(queue.filter((q) => q.reference !== ref));
      msg = `Removed ${ref} from the queue`;
    } else {
      onQueueChange([...queue, { reference: ref, text: v.text, kind: 'scripture' }]);
      msg = `Queued ${ref}`;
    }
  }

  function toggleCheck(v) {
    const ref = refOf(v);
    const next = new Set(checked);
    next.has(ref) ? next.delete(ref) : next.add(ref);
    checked = next;
  }

  function queueChecked() {
    const add = source
      .filter((v) => checked.has(refOf(v)) && !queue.some((q) => q.reference === refOf(v)))
      .map((v) => ({ reference: refOf(v), text: v.text, kind: 'scripture' })); // tagged (RG-185)
    if (add.length) onQueueChange([...queue, ...add]);
    msg = `Queued ${add.length} verse${add.length === 1 ? '' : 's'}`;
    checked = new Set();
  }

  // A panic control never reports a success it did not achieve (CLAUDE.md §15):
  // `clearScreens` returns a boolean and sets the global panicError store.
  async function clear() {
    msg = '';
    if (await clearScreens()) msg = 'Screens cleared.';
  }

  /**
   * ONE PRESS SELECTS. The Library is a build surface (REBRAND §2/§10): a single
   * click fills the inspector and reaches no output, and `Cue in Live` there is
   * the deliberate press that stages it. `VerseDeck`'s `press` prop is what says
   * so, and its default is still the old fire-on-press for everything else.
   *
   * SCRIPTURE IS THE CONTENT, so the reference rides to the inspector's preview:
   * a verse on a wall carries its reference and a lyric does not (DECISIONS §73).
   */
  function selectVerse(v) {
    selected = v;
    onSelect({
      kind: 'scripture',
      title: v.reference ?? refOf(v),
      titleLabel: 'Reference',
      translation: v.translation ?? null,
      words: v.text ?? '',
      slide: { reference: v.reference ?? refOf(v), text: v.text ?? '', translation: v.translation ?? null },
      reference: v.reference ?? refOf(v),
      plan: {
        cueType: 'scripture',
        label: v.reference ?? refOf(v),
        payload: {
          book: v.book,
          chapter: v.chapter,
          verse: v.verse,
          reference: v.reference ?? refOf(v),
          text: v.text,
          translation: v.translation,
        },
      },
    });
  }

  /** The double press. Scripture is not editable (see the note at the top of
      this pane), so "open" means: put the whole chapter in front of me. */
  function openVerse(v) {
    if (v.book && v.chapter) {
      book = v.book;
      chapter = v.chapter;
    }
    jumpTo(v.verse);
  }

  const refOf = (v) => `${v.book} ${v.chapter}:${v.verse}`;
  const shape = (v) => ({
    key: refOf(v),
    reference: refOf(v),
    text: v.text,
    // The card's second line. The thumbnail renders this same verse, but through
    // the operator's own template and fitted to a 268px box — a long verse shrinks
    // to a grey smudge there (rule 37's floor is about exactly this). The sub line
    // is the one legible copy of the words at card size.
    sub: v.text,
    translation: v.abbreviation,
    book: v.book,
    chapter: v.chapter,
    verse: v.verse,
  });
  const words = (t) => (t ? t.trim().split(/\s+/).length : 0);
  const isSaved = (v, list) => list.some((s) => s.reference === refOf(v));

  $: chapterCount = books.find((b) => b.book === book)?.chapters ?? 0;
  $: searchMode = !!query?.trim();
  $: base = passage ? inRange(verses, passage) : searchMode ? results : verses;
  $: source = favouritesOnly ? base.filter((v) => isSaved(v, savedList)) : base;
  $: sorted =
    sort === 'length'
      ? [...source].sort((a, b) => a.text.length - b.text.length)
      : source;
  // THE DECK IS THE CHAPTER. Nothing is inserted into it and nothing is
  // rewritten — see the note above the pane.
  $: deck = sorted.map(shape);
  // SLIDE NUMBERS are the position in the deck and keep counting across pages —
  // they were the verse number, which drifts the moment anything is inserted,
  // filtered or sorted, and then two slides on screen wear the same number.
  $: numbered = deck.map((d, i) => ({ ...d, slideNo: i + 1 }));
  $: pages = Math.max(1, Math.ceil(numbered.length / perPage));
  $: pageItems = numbered.slice(page * perPage, page * perPage + perPage);
  $: firstShown = numbered.length ? page * perPage + 1 : 0;
  $: lastShown = Math.min(numbered.length, (page + 1) * perPage);
  $: savedRefs = new Set(savedList.map((s) => s.reference));
  $: totalChapters = books.reduce((n, b) => n + (b.chapters ?? 0), 0);
  $: queuedRefs = new Set(queue.map((q) => q.reference));
  $: if (page > pages - 1) page = 0;
  $: pageNums =
    pages <= 7
      ? Array.from({ length: pages }, (_, i) => i)
      : [0, 1, 2, 3, 4, -1, pages - 1].filter((n, i, a) => a.indexOf(n) === i);
  $: liveRef = !$screenBlack && $live ? ($live.reference ?? null) : null;
  $: heading = passage
    ? passage.from == null
      ? `${passage.book} ${passage.chapter}`
      : `${passage.book} ${passage.chapter}:${passage.from}${passage.to > passage.from ? `–${passage.to}` : ''}`
    : searchMode
      ? `“${query.trim()}”`
      : book
        ? `${book} ${chapter}`
        : 'Bible';
  $: subheading = passage
    ? `${source.length} of ${verses.length} verses · clear the search for the whole chapter`
    : searchMode
      ? `${source.length} result${source.length === 1 ? '' : 's'}`
      : `${source.length} verse${source.length === 1 ? '' : 's'}${favouritesOnly ? ' in favourites' : ''}`;
</script>

<div class="br">
  {#if !books.length && $readErrors.listBooks}
    <!-- RG-95. An empty book list is either a corpus that is not installed or a
         database that did not answer, and only one of those is fixed in Settings →
         Diagnostics. Sending an operator to the wrong screen costs the minutes
         before a service. -->
    <ErrorState error={$readErrors.listBooks} />
  {:else if !books.length}
    <EmptyState message="No scripture is loaded. Check Settings → This machine." />
  {:else}
    <div class="br-grid">
      <!-- BOOKS. Canonical order, from the backend — never alphabetical. A Bible
           whose contents open "Acts, Amos, Chronicles" is not one anyone can
           navigate; the order is part of what the book is. -->
      <nav class="br-panel br-books" aria-label="Books">
        <p class="r-lbl br-panelhead">Books ({books.length})</p>
        <button class="br-all r-focus" class:on={!book} on:click={() => (book = null)}>
          <span>All Books</span>
          <span class="ct r-mono">{totalChapters} chapters</span>
        </button>
        <div class="br-booklist r-scroll">
          {#each books as b (b.book)}
            <button
              class="br-book r-focus"
              class:on={book === b.book}
              on:click={() => {
                book = b.book;
                chapter = 1;
              }}>
              <span class="nm">{b.book}</span>
              <span class="ct r-mono">{b.chapters}</span>
            </button>
          {/each}
        </div>
        <!-- The footer held a "Browse All Books" button that set `book` to the
             FIRST book. Its label named the control directly above it ("All
             Books", which really does clear the filter) and its behaviour was a
             different thing entirely, so whichever one an operator believed, one
             of them was lying. Removed rather than relabelled: the rail is the
             book picker, and it needed no third way to pick one. -->
        <!-- THE TRANSLATION LIVES WITH THE BOOKS. It is a fact about the corpus
             on the rail above it — which Bible these books ARE — not a filter on
             the chapter in the grid, and putting it here is what let the pane
             head become one row instead of two. The count that used to sit here
             was a third copy of "Books (13)", six pixels above it. -->
        <div class="br-panelfoot">
          <label class="br-tr">
            <span class="r-lbl">Translation</span>
            {#if translations.length}
              <select
                class="r-select sm"
                aria-label="Translation"
                disabled={translations.length < 2}
                value={activeTranslation}
                on:change={(e) => onTranslation(Number(e.currentTarget.value))}>
                {#each translations as t}
                  <option value={t.id}>{t.abbreviation || t.name}</option>
                {/each}
              </select>
            {:else}
              <!-- An EMPTY SELECT is not a translation picker; it is a blank box
                   that reads as broken. Until the corpus answers, say what is
                   bundled and offer no choice, because there is not one yet. -->
              <span class="r-mono br-tronly">KJV</span>
            {/if}
          </label>
        </div>
      </nav>

      <!-- THE CHAPTER. -->
      <section class="br-panel br-main">
        <!-- THE DOCK HEAD (REBRAND §10). What is in the grid, how many of them,
             and what a press on one does — said where the press happens, because
             a legend an operator has to remember is a legend they will not. -->
        <header class="br-mainhead">
          <!-- ONE LINE. The heading used to carry a second line saying "4
               verses" while the legend beside it said "4 ITEMS" and the footer
               below said "4 verses" — one count, three places, and the stack of
               them is what made this head two rows deep. The count now lives in
               the legend, where §10 puts it, and the heading is the place. -->
          <b class="br-where">{heading}</b>

          {#if checked.size}
            <!-- The bulk actions REPLACE THE LEGEND, not the navigation. They
                 used to replace the Sort control, and when the translation,
                 chapter picker moved onto this row that same `{:else}` would have
                 taken the Bible's navigation away from an operator who had ticked
                 a verse — reachable again only by clearing a selection they might
                 want to keep. The legend is the one thing here that is furniture. -->
            <button class="r-btn primary sm" on:click={queueChecked}>
              Queue {checked.size} selected
            </button>
            <button class="r-btn ghost sm" on:click={() => (checked = new Set())}>Clear</button>
          {:else}
            <!-- THE DOCK HEAD'S CAPTION (§10): how many, and what a press does.
                 It is the flexible element on this row and ellipsises before any
                 control is pushed off it, because it is the one thing here an
                 operator reads once and then knows. -->
            <span class="br-legend r-mono">
              {subheading} · single click cues · double click opens
            </span>
          {/if}

          <!-- Moved off the shell's deleted filter bar. The translation went one
               step further, to the rail footer, because it describes the corpus
               rather than the chapter — see the note there. The VERSE picker is
               gone: it scrolled to a verse and fired nothing, and the Library's
               one search box already does that better ("Genesis 1:3" jumps
               there, and a range filters the grid). Two controls for one job,
               and one of them was the box the operator is already looking at —
               the same argument that deleted the book select. -->
          <select class="r-select sm br-sel" aria-label="Chapter" bind:value={chapter}>
            {#each Array(chapterCount) as _, i}
              <option value={i + 1}>Chapter {i + 1}</option>
            {/each}
          </select>
          <!-- Icon-only, and named twice over: a toggle whose STATE is the whole
               message does not need to spell its name beside it on a row that
               has no room. `aria-pressed` and the label carry it for anyone who
               cannot see the fill. -->
          <button
            class="r-iconbtn br-fav"
            class:on={favouritesOnly}
            aria-pressed={favouritesOnly}
            aria-label="Favourites only"
            title={favouritesOnly ? 'Showing favourites only' : 'Show favourites only'}
            on:click={() => (favouritesOnly = !favouritesOnly)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill={favouritesOnly ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true"><path d="M6 3h12v18l-6-4.5L6 21z" /></svg>
          </button>
          <select class="r-select sm br-sel" bind:value={sort} aria-label="Sort">
            <option value="verse">Verse order</option>
            <option value="length">Shortest first</option>
          </select>

          <div class="r-seg" role="group" aria-label="Layout">
            <button class:on={layout === 'grid'} aria-label="Grid" on:click={() => (layout = 'grid')}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="3" y="3" width="7" height="7" rx="1.4" /><rect x="14" y="3" width="7" height="7" rx="1.4" /><rect x="3" y="14" width="7" height="7" rx="1.4" /><rect x="14" y="14" width="7" height="7" rx="1.4" /></svg>
            </button>
            <button class:on={layout === 'list'} aria-label="List" on:click={() => (layout = 'list')}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><rect x="3" y="5" width="18" height="2.6" rx="1.3" /><rect x="3" y="10.7" width="18" height="2.6" rx="1.3" /><rect x="3" y="16.4" width="18" height="2.6" rx="1.3" /></svg>
            </button>
          </div>
        </header>

        <div class="br-body r-scroll">
          {#if loadingChapter}
            <Loading what="the chapter" />
          {:else if searching}
            <Loading what="matching verses" />
          {:else if searchMode && $readErrors.searchScripture}
            <ErrorState error={$readErrors.searchScripture} onRetry={() => runSearch(query)} />
          {:else if !searchMode && $readErrors.chapterVerses}
            <!-- Not "that chapter is empty": no chapter of any Bible is. -->
            <ErrorState error={$readErrors.chapterVerses} onRetry={() => open(book, chapter)} />
          {:else if !sorted.length}
            <EmptyState
              message={favouritesOnly
                ? 'No favourites here yet — star a verse to keep it.'
                : searchMode
                  ? `No scripture matching “${query.trim()}”.`
                  : 'That chapter is empty.'} />
          {:else}
            <VerseDeck
              items={numbered}
              {template}
              {liveRef}
              rehearsing={$rehearsing}
              selectedRef={selected ? refOf(selected) : ''}
              {checked}
              {savedRefs}
              {queuedRefs}
              busyRef={firing}
              {layout}
              press="select"
              onSelect={selectVerse}
              onOpen={openVerse}
              onCheck={toggleCheck}
              onFire={fire}
              onQueue={toggleQueue}
              onSave={toggleSave}
              can={{ queue: true, favourite: true, edit: false, duplicate: false, add: false }} />
          {/if}
        </div>

        <!-- NO pagination — the whole chapter is one scroll (operator request). A
             chapter is a bounded list, and paging a Bible chapter is friction the
             reader never wanted. The count stays as a quiet footer. -->
        <footer class="br-pager">
          <span class="br-count">{numbered.length} {numbered.length === 1 ? 'verse' : 'verses'}</span>
        </footer>
      </section>
    </div>
  {/if}

  <!-- Announced. "John 3:16 is on the screens" is the confirmation that content
         reached a congregation, and it was silent to a screen reader — the error
         half of these panes carries `role="alert"` and this half carried nothing. -->
  {#if msg}<p class="br-msg" role="status" aria-live="polite">{msg}</p>{/if}
  {#if error}<p class="br-err" role="alert">{error}</p>{/if}
</div>

<style>
  .br { display: flex; flex-direction: column; gap: 10px; min-height: 0; flex: 1; }
  .br-grid {
    display: grid;
    grid-template-columns: 196px minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
    flex: 1;
  }
  .br-panel {
    display: flex; flex-direction: column; min-height: 0;
    background: var(--v-bg); border: 1px solid var(--v-line); border-radius: var(--v-r-lg);
  }

  /* ── Books ─────────────────────────────────────────────────────────────── */
  .br-panelhead { margin: 0; padding: 13px 14px 9px; }
  .br-booklist { flex: 1; min-height: 0; overflow-y: auto; padding: 0 8px 8px; }
  /* A LIST ROW, not a button — B2. One of sixty-six book rows in a scrolling
     rail, a name and a count, selected rather than pressed. */
  .br-book {
    display: flex; align-items: center; gap: 8px; width: 100%; padding: 6px 10px;
    border-radius: var(--v-r-md); background: none; border: 0; color: var(--v-dim);
    font-family: var(--f-body); font-size: var(--v-fs-pr); text-align: left; cursor: pointer;
  }
  .br-book .nm { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .br-book .ct { font-size:var(--v-fs-lbl); color: var(--v-faint); }
  .br-book:hover:not(.on) { background: var(--v-surf2); color: var(--v-txt); }
  .br-book.on { background: var(--v-accent-fill); color: var(--v-accent-ink); font-weight: 600; }
  .br-book.on .ct { color: rgba(255, 255, 255, 0.75); }
  /* A LIST ROW, not a button — B2. The "All books" row that heads the rail;
     the same shape as `.br-book` with the seam above it instead of below. */
  .br-all {
    display: flex; flex-direction: column; align-items: flex-start; gap: 2px;
    width: calc(100% - 16px); margin: 0 8px 8px; padding: 8px 10px;
    border-radius: var(--v-r-md); border: 1px solid var(--v-line);
    background: var(--v-surf); color: var(--v-txt); font-family: var(--f-body);
    font-size: var(--v-fs-pr); text-align: left; cursor: pointer;
  }
  .br-all:hover { border-color: var(--v-line2); }
  .br-all.on { border-color: var(--v-accent-line); background: var(--v-accent-soft); }
  .br-all .ct { font-size: 10.5px; color: var(--v-faint); }
  .br-sel { width: auto; max-width: 122px; height: 24px; padding: 0 24px 0 8px; font-size:var(--v-fs-lbl);
    flex: 0 0 auto;
    background-position: calc(100% - 12px) 10px, calc(100% - 7px) 10px; }
  /* NO SIZE — B2. It renders `.r-iconbtn br-fav` and then overrode the shared
     26px to 24px; Library's `.lib-more` overrode the same control to 30px. Three
     icon-button sizes in two workspaces, each internally consistent, which is
     why none of them looked wrong from inside its own file. What is left here is
     position and the pressed state, which is the shape a legitimate override
     has. */
  .br-fav { flex: 0 0 auto; }
  .br-fav.on { border-color: var(--v-accent-line); color: var(--v-accent2); background: var(--v-accent-soft); }
  /* The translation, in the rail footer with the books it describes. */
  .br-tr { display: flex; align-items: center; gap: 8px; }
  .br-tr .r-lbl { margin: 0; flex: 1; min-width: 0; }
  /* Width and padding only: the HEIGHT is the shared control's, so this select
     is the same 26px as every button beside it. It was 24px. */
  .br-tr .r-select { width: auto; max-width: 96px; padding: 0 24px 0 8px;
    font-size:var(--v-fs-lbl); flex: 0 0 auto;
    background-position: calc(100% - 12px) 11px, calc(100% - 7px) 11px; }
  .br-tronly { font-size:var(--v-fs-lbl); color: var(--v-dim); }
  .br-pager { flex-wrap: wrap; }
  .br-panelfoot { padding: 10px; border-top: 1px solid var(--v-line); }

  /* ── The chapter ───────────────────────────────────────────────────────── */
  /* ONE ROW, and `nowrap` is the assertion. This head wrapped, and what it
     wrapped INTO was a second band of chrome above the first slide — the thing
     §10 is about. Nothing here may wrap: the two flexible items shrink and
     ellipsise instead, and every control keeps its own width. */
  .br-mainhead {
    display: flex; align-items: center; gap: 8px; padding: 8px 12px; flex-wrap: nowrap;
    border-bottom: 1px solid var(--v-line);
  }
  .br-where {
    flex: 0 1 auto; min-width: 0; font-size:var(--v-fs-h2); font-weight: 600; color: var(--v-txt);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  /* The legend is furniture, not a heading: mono, faint, and it is the FIRST
     thing to give up width, before any control loses its label or its place. */
  .br-legend {
    flex: 1 1 auto; min-width: 0; font-size: var(--v-fs-cap); color: var(--v-faint);
    text-transform: uppercase; letter-spacing: .08em;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .br-body {
    flex: 1; min-height: 0; overflow-y: auto; padding: 12px;
    display: flex; flex-direction: column; gap: 8px;
  }

  /* ON AIR is an amber bar and an amber badge, never an amber field behind body
     text: gold under 13px type is unreadable, and the badge carries the meaning. */

  /* ── Read ──────────────────────────────────────────────────────────────── */

  /* ── Table ─────────────────────────────────────────────────────────────── */

  /* ── Pager ─────────────────────────────────────────────────────────────── */
  .br-pager { display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-top: 1px solid var(--v-line); }
  .br-count { font-size: var(--v-fs-cap); color: var(--v-faint); }

  .br-msg, .br-err { margin: 0; font-size: var(--v-fs-b2); line-height: 1.6; }
  .br-msg { color: var(--v-emerald); }
  .br-err { color: var(--v-red); }

  @media (max-width: 1360px) { .br-grid { grid-template-columns: 176px minmax(0, 1fr); } }
  @media (max-width: 1140px) {
    .br-grid { grid-template-columns: minmax(0, 1fr); }
    .br-books { display: none; }
  }
  @media (max-width: 860px) { .br-grid { grid-template-columns: 1fr; } }
</style>
