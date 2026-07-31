<script>
  // LIBRARY → BIBLE. Rebuilt from docs/relaydesign/relay-library-screen.png.
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
  } from '../../stores/capture.js';

  /** Canonical book list + chapter counts, loaded by the Library shell. */
  export let books = [];
  /** The translations that ACTUALLY exist in the corpus. */
  export let translations = [];
  /** Bound to the shell's filter bar — one piece of state, two controls. */
  export let book = null;
  export let chapter = 1;
  export let verse = null;
  export let verseCount = 0;
  /** The Library's one search box. */
  export let query = '';
  /** Only verses already in Favourites (the filter-bar toggle). */
  export let favouritesOnly = false;
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
  // LIST by default — plain text, like the Lyrics pane, so a chapter opens
  // instantly. The grid view (a live TemplateRender thumbnail per verse) is
  // gorgeous but renders a dozen fit loops at once; it stays one click away.
  let layout = 'list';
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
  // The verse picker scrolls; it does not fire. Where to look and what a
  // congregation sees are different decisions.
  $: if (verse) jumpTo(verse);

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
      onQueueChange([...queue, { reference: ref, text: v.text }]);
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
      .map((v) => ({ reference: refOf(v), text: v.text }));
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

  const refOf = (v) => `${v.book} ${v.chapter}:${v.verse}`;
  const shape = (v) => ({
    key: refOf(v),
    reference: refOf(v),
    text: v.text,
    translation: v.abbreviation,
    book: v.book,
    chapter: v.chapter,
    verse: v.verse,
  });
  const words = (t) => (t ? t.trim().split(/\s+/).length : 0);
  const isSaved = (v, list) => list.some((s) => s.reference === refOf(v));

  $: searchMode = !!query?.trim();
  $: base = passage ? inRange(verses, passage) : searchMode ? results : verses;
  $: source = favouritesOnly ? base.filter((v) => isSaved(v, savedList)) : base;
  $: verseCount = verses.length;
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
  {#if !books.length}
    <EmptyState message="No scripture is loaded. Check Settings → data health." />
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
        <div class="br-panelfoot">
          <button class="r-btn ghost sm" on:click={() => (book = books[0].book)}>
            Browse All Books
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M14 4h6v6M20 4l-8 8M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4" /></svg>
          </button>
        </div>
      </nav>

      <!-- THE CHAPTER. -->
      <section class="br-panel br-main">
        <header class="br-mainhead">
          <div class="br-where">
            <b>{heading}</b>
            <span>{subheading}</span>
          </div>

          {#if checked.size}
            <button class="r-btn primary sm" on:click={queueChecked}>
              Queue {checked.size} selected
            </button>
            <button class="r-btn ghost sm" on:click={() => (checked = new Set())}>Clear</button>
          {:else}
            <label class="br-ctl">
              <span class="r-lbl">Sort</span>
              <select class="r-select sm" bind:value={sort} aria-label="Sort">
                <option value="verse">Verse order</option>
                <option value="length">Shortest first</option>
              </select>
            </label>
          {/if}

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

  {#if msg}<p class="br-msg">{msg}</p>{/if}
  {#if error}<p class="br-err">{error}</p>{/if}
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
  .br-book {
    display: flex; align-items: center; gap: 8px; width: 100%; padding: 6px 10px;
    border-radius: var(--v-r-md); background: none; border: 0; color: var(--v-dim);
    font-family: var(--f-body); font-size: 13px; text-align: left; cursor: pointer;
  }
  .br-book .nm { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .br-book .ct { font-size: 11px; color: var(--v-faint); }
  .br-book:hover:not(.on) { background: var(--v-surf2); color: var(--v-txt); }
  .br-book.on { background: var(--v-accent-fill); color: var(--v-accent-ink); font-weight: 600; }
  .br-book.on .ct { color: rgba(255, 255, 255, 0.75); }
  .br-all {
    display: flex; flex-direction: column; align-items: flex-start; gap: 2px;
    width: calc(100% - 16px); margin: 0 8px 8px; padding: 8px 10px;
    border-radius: var(--v-r-md); border: 1px solid var(--v-line);
    background: var(--v-surf); color: var(--v-txt); font-family: var(--f-body);
    font-size: 13px; text-align: left; cursor: pointer;
  }
  .br-all:hover { border-color: var(--v-line2); }
  .br-all.on { border-color: var(--v-accent-line); background: var(--v-accent-soft); }
  .br-all .ct { font-size: 10.5px; color: var(--v-faint); }
  .br-ctl { display: flex; align-items: center; gap: 7px; flex: 0 0 auto; }
  .br-ctl .r-lbl { margin: 0; }
  .br-ctl .r-select { width: auto; height: 30px; padding: 0 30px 0 10px; font-size: 12px;
    background-position: calc(100% - 14px) 13px, calc(100% - 9px) 13px; }
  .br-pager { flex-wrap: wrap; }
  .br-panelfoot { padding: 10px; border-top: 1px solid var(--v-line); }
  .br-panelfoot .r-btn { width: 100%; }

  /* ── The chapter ───────────────────────────────────────────────────────── */
  .br-mainhead {
    display: flex; align-items: center; gap: 12px; padding: 11px 14px;
    border-bottom: 1px solid var(--v-line);
  }
  .br-where { flex: 1; min-width: 0; }
  .br-where b { display: block; font-size: 15px; font-weight: 600; color: var(--v-txt); }
  .br-where span { font-size: var(--v-fs-cap); color: var(--v-faint); }
  .br-step { display: flex; align-items: center; gap: 4px; }
  .br-step span { font-size: 10.5px; color: var(--v-faint); min-width: 84px; text-align: center; }
  .br-step button {
    width: 24px; height: 24px; border-radius: var(--v-r-sm); border: 1px solid var(--v-line2);
    background: var(--v-surf2); color: var(--v-dim); cursor: pointer; font-size: 13px; line-height: 1;
  }
  .br-step button:hover:not(:disabled) { color: var(--v-txt); }
  .br-step button:disabled { opacity: 0.4; cursor: not-allowed; }
  .br-body {
    flex: 1; min-height: 0; overflow-y: auto; padding: 12px;
    display: flex; flex-direction: column; gap: 8px;
  }

  .br-card {
    position: relative; display: grid; grid-template-columns: 30px minmax(0, 1fr) auto;
    gap: 12px; align-items: start; width: 100%; padding: 13px 15px; text-align: left;
    background: var(--v-surf); border: 1px solid var(--v-line); border-radius: var(--v-r-md);
    cursor: pointer; transition: border-color 0.14s, background 0.14s;
  }
  .br-card:hover { border-color: var(--v-line2); background: var(--v-surf2); }
  .br-card.on { border-color: var(--v-accent-line); background: var(--v-accent-soft); }
  /* ON AIR is an amber bar and an amber badge, never an amber field behind body
     text: gold under 13px type is unreadable, and the badge carries the meaning. */
  .br-card.air { border-color: rgba(255, 176, 0, 0.42); background: rgba(255, 176, 0, 0.05); }
  .br-card.air::before {
    content: ''; position: absolute; left: 0; top: 10px; bottom: 10px; width: 3px;
    border-radius: 0 3px 3px 0; background: var(--v-amber);
  }
  .br-n { font-size: 14px; color: var(--v-faint); padding-top: 1px; text-align: right; }
  .br-card.air .br-n { color: var(--v-amber); }
  .br-c { display: flex; flex-direction: column; gap: 9px; min-width: 0; }
  .br-t { font-size: 13.5px; line-height: 1.55; color: var(--v-txt); }
  .br-chips { display: flex; flex-wrap: wrap; gap: 6px; }
  .br-cardact { display: flex; align-items: center; gap: 4px; }
  .br-ic {
    width: 26px; height: 26px; display: grid; place-items: center; border: 0;
    border-radius: var(--v-r-sm); background: transparent; color: var(--v-faint);
    cursor: pointer; transition: color 0.14s, background 0.14s;
  }
  .br-ic:hover:not(:disabled) { color: var(--v-txt); background: var(--v-surf3); }
  .br-ic.on { color: var(--v-accent2); }
  .br-ic:disabled { opacity: 0.4; cursor: not-allowed; }

  /* ── Read ──────────────────────────────────────────────────────────────── */
  .br-read { font-size: 15px; line-height: 1.8; color: var(--v-txt); max-width: 68ch; padding: 4px 2px; }
  .br-rv { cursor: pointer; border-radius: var(--v-r-sm); transition: background 0.14s; }
  .br-rv sup { font-size: 9.5px; color: var(--v-faint); padding-right: 4px; vertical-align: super; }
  .br-rv:hover { background: var(--v-surf2); }
  .br-rv.on { background: var(--v-accent-soft); }
  .br-rv.air { background: var(--v-amber-soft); color: var(--v-amber2); }

  /* ── Table ─────────────────────────────────────────────────────────────── */
  .br-table { width: 100%; border-collapse: collapse; font-size: var(--v-fs-b2); }
  .br-table th {
    text-align: left; padding: 0 10px 8px; font-family: var(--f-mono); font-size: 10px;
    font-weight: 600; letter-spacing: 0.16em; text-transform: uppercase; color: var(--v-faint);
    border-bottom: 1px solid var(--v-line);
  }
  .br-table td { padding: 9px 10px; border-bottom: 1px solid var(--v-line); color: var(--v-dim); vertical-align: top; }
  .br-table td.tx { color: var(--v-txt); line-height: 1.5; }
  .br-table .num { text-align: right; }
  .br-table tbody tr { cursor: pointer; }
  .br-table tbody tr:hover td { background: var(--v-surf2); }
  .br-table tbody tr.on td { background: var(--v-accent-soft); }
  .br-table tbody tr.air td { background: var(--v-amber-soft); }

  /* ── Pager ─────────────────────────────────────────────────────────────── */
  .br-pager { display: flex; align-items: center; gap: 12px; padding: 10px 14px; border-top: 1px solid var(--v-line); }
  .br-pages { display: flex; align-items: center; gap: 4px; flex: 1; }
  .br-pg {
    min-width: 28px; height: 28px; padding: 0 7px; border-radius: var(--v-r-sm); border: 0;
    background: transparent; color: var(--v-dim); font-family: var(--f-mono); font-size: 12px;
    font-variant-numeric: tabular-nums; cursor: pointer;
  }
  .br-pg:hover:not(:disabled):not(.on) { background: var(--v-surf2); color: var(--v-txt); }
  .br-pg.on { background: var(--v-accent-fill); color: var(--v-accent-ink); font-weight: 600; }
  .br-pg:disabled { opacity: 0.35; cursor: not-allowed; }
  .br-gap { color: var(--v-faint); padding: 0 2px; }
  .br-count { font-size: var(--v-fs-cap); color: var(--v-faint); }

  .br-modal {
    position: fixed; inset: 0; z-index: 60; display: grid; place-items: center;
    background: rgba(0, 0, 0, 0.6); padding: 24px;
  }
  .br-sheet {
    width: min(560px, 100%); display: flex; flex-direction: column; gap: 12px;
    padding: 18px; background: var(--v-surf); border: 1px solid var(--v-line2);
    border-radius: var(--v-r-xl); box-shadow: var(--v-shadow-lg);
  }
  .br-sheet header { display: flex; align-items: center; gap: 8px; }
  .br-sheet header .r-lbl { margin: 0; }
  .br-spring { flex: 1; }
  .br-field { display: flex; flex-direction: column; gap: 5px; }
  .br-text {
    height: auto; padding: 11px 13px; line-height: 1.6; resize: vertical;
    font-family: var(--f-body); font-size: 13.5px;
  }
  .br-fine { margin: 0; font-size: var(--v-fs-cap); line-height: 1.6; color: var(--v-faint); }
  .br-fine b { color: var(--v-dim); }

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
