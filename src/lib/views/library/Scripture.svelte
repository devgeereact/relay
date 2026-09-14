<script>
  // LIBRARY → SAVED. Verses the operator has kept, as a deck.
  //
  // The same card, rail and one-click rule as the Bible tab. Searching looks for
  // something to KEEP; an empty box shows what is already kept. There is one
  // search box in the Library — this pane used to carry a second one of its own.
  import { onMount } from 'svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import ErrorState from '../../ui/ErrorState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import VerseDeck from './VerseDeck.svelte';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import {
    searchScripture,
    listSavedScripture,
    saveScripture,
    deleteSavedScripture,
    manualFire,
    listActiveTemplates,
    live,
    screenBlack,
    rehearsing,
    readErrors,
  } from '../../stores/capture.js';

  export let query = '';
  export let queue = [];
  export let onQueueChange = () => {};
  /** Hand the selected verse up for the inspector (REBRAND §10). */
  export let onSelect = () => {};

  /**
   * ONE PRESS SELECTS — see the note on `VerseDeck`'s `press` prop. The Library
   * is a build surface; the take is the inspector's `Cue in Live` and the card's
   * own kebab. A SAVED VERSE IS SCRIPTURE, so the reference is content and rides
   * to the preview (DECISIONS §73).
   */
  let selectedRef = '';
  function selectVerse(v) {
    selectedRef = v.reference;
    onSelect({
      kind: 'scripture',
      title: v.reference,
      titleLabel: 'Reference',
      translation: v.translation ?? null,
      words: v.text ?? '',
      slide: { reference: v.reference, text: v.text ?? '', translation: v.translation ?? null },
      reference: v.reference,
      plan: {
        cueType: 'scripture',
        label: v.reference,
        payload: {
          book: v.book,
          chapter: v.chapter,
          verse: v.verse,
          reference: v.reference,
          text: v.text,
          translation: v.translation,
        },
      },
    });
  }

  let saved = [];
  let results = [];
  let template = null;
  let loading = true;
  let searching = false;
  let firing = '';
  let error = '';
  let msg = '';
  let checked = new Set();
  let layout = 'grid';
  let page = 0;
  let perPage = 12;
  onMount(load);

  // Named so the error state has something to retry with (RG-95). A read that
  // failed is not an empty library, and the operator cannot tell the difference
  // from a sentence that says "no saved verses yet".
  async function load() {
    loading = true;
    saved = (await listSavedScripture()) ?? [];
    template = (await listActiveTemplates().catch(() => []))[0] ?? null;
    loading = false;
  }

  let lastQuery = null;
  $: if (query !== lastQuery) {
    lastQuery = query;
    page = 0;
    // The shelf under the rail has just been replaced, so the book on it has
    // too. Keeping the old selection would silently filter a fresh search.
    book = null;
    doSearch(query);
  }
  async function doSearch(q) {
    if (!q?.trim()) {
      results = [];
      return;
    }
    searching = true;
    results = (await searchScripture(q.trim())) ?? [];
    searching = false;
  }

  async function keep(v) {
    error = '';
    try {
      await saveScripture(v.book, v.chapter, v.verse);
      saved = (await listSavedScripture()) ?? saved;
      msg = `Saved ${v.reference}`;
    } catch (e) {
      error = humanError(e);
    }
  }

  async function toggleSave(item) {
    const hit = saved.find((s) => s.reference === item.reference);
    error = '';
    try {
      if (hit) {
        await deleteSavedScripture(hit.id);
        saved = saved.filter((s) => s.id !== hit.id);
        msg = `Removed ${item.reference}`;
      } else if (item.book) {
        await keep(item);
      }
    } catch (e) {
      error = humanError(e);
    }
  }

  async function fire(item) {
    if ($safeMode) return;
    firing = item.reference;
    error = '';
    msg = '';
    try {
      await manualFire(item.reference);
      msg = `${item.reference} is on the screens`;
    } catch (e) {
      error = humanError(e);
    }
    firing = '';
  }

  function toggleQueue(item) {
    if (queue.some((q) => q.reference === item.reference)) {
      onQueueChange(queue.filter((q) => q.reference !== item.reference));
      msg = `Removed ${item.reference} from the queue`;
    } else {
      onQueueChange([...queue, { reference: item.reference, text: item.text }]);
      msg = `Queued ${item.reference}`;
    }
  }

  function toggleCheck(item) {
    const next = new Set(checked);
    next.has(item.reference) ? next.delete(item.reference) : next.add(item.reference);
    checked = next;
  }

  function queueChecked() {
    const add = deck
      .filter((d) => checked.has(d.reference) && !queue.some((q) => q.reference === d.reference))
      .map((d) => ({ reference: d.reference, text: d.text }));
    if (add.length) onQueueChange([...queue, ...add]);
    msg = `Queued ${add.length} verse${add.length === 1 ? '' : 's'}`;
    checked = new Set();
  }

  // ── THE BOOK RAIL (REBRAND §10) ───────────────────────────────────────────
  //
  // Same grammar as every other collection: the items down the left, the slides
  // in the grid. For scripture the item is a BOOK, because that is what a saved
  // deck is organised by — an operator looking for the Romans verse they kept
  // does not want to page through Genesis to reach it.
  //
  // The rail is derived from what the grid is ACTUALLY showing (saved verses, or
  // the search results), never from a separate query. A rail that lists books
  // the grid cannot show is a rail that sends you somewhere empty.
  let book = null; // null = every book
  const bookOf = (r) =>
    r.book || String(r.reference ?? '').replace(/\s+\d+(?::\d+)?(?:\s*[-–]\s*\d+)?\s*$/, '').trim();

  $: searchMode = !!query?.trim();
  $: rows = searchMode ? results : saved;
  $: shelf = (() => {
    const by = new Map();
    for (const r of rows) {
      const b = bookOf(r);
      if (b) by.set(b, (by.get(b) ?? 0) + 1);
    }
    return [...by].map(([b, n]) => ({ book: b, count: n }));
  })();
  // A book that is no longer on the shelf cannot stay selected: the grid would
  // be empty with no visible reason why.
  $: if (book && !shelf.some((s) => s.book === book)) book = null;

  function pickBook(b) {
    book = b;
    page = 0;
  }

  $: base = rows
    .filter((r) => !book || bookOf(r) === book)
    .map((r) => ({
    key: r.reference,
    reference: r.reference,
    label: r.reference,
    text: r.text,
    sub: r.text,
    translation: r.translation ?? r.abbreviation,
    book: r.book,
    chapter: r.chapter,
    verse: r.verse,
  }));
  // Saved verses are scripture: nothing here is edited or inserted.
  $: deck = base;
  $: numbered = deck.map((d, i) => ({ ...d, slideNo: i + 1 }));
  $: pages = Math.max(1, Math.ceil(numbered.length / perPage));
  $: if (page > pages - 1) page = 0;
  $: pageItems = numbered.slice(page * perPage, page * perPage + perPage);
  $: firstShown = numbered.length ? page * perPage + 1 : 0;
  $: lastShown = Math.min(numbered.length, (page + 1) * perPage);
  $: savedRefs = new Set(saved.map((s) => s.reference));
  $: queuedRefs = new Set(queue.map((q) => q.reference));
  $: liveRef = !$screenBlack && $live ? ($live.reference ?? null) : null;
</script>

<div class="sv">
 <div class="sv-split">
  <!-- The items. Hidden only when there is nothing to list — a rail offering
       one book called "All" is a label pretending to be a choice. -->
  {#if shelf.length > 1}
    <nav class="sv-rail" aria-label="Books">
      <div class="sv-railhead">Books</div>
      <div class="sv-raillist r-scroll">
        <button class="sv-book r-focus" class:on={!book} aria-pressed={!book} on:click={() => pickBook(null)}>
          <span class="sv-bn">All books</span>
          <span class="sv-bk r-mono">{rows.length}</span>
        </button>
        {#each shelf as s (s.book)}
          <button
            class="sv-book r-focus"
            class:on={book === s.book}
            aria-pressed={book === s.book}
            on:click={() => pickBook(s.book)}>
            <span class="sv-bn">{s.book}</span>
            <span class="sv-bk r-mono">{s.count}</span>
          </button>
        {/each}
      </div>
    </nav>
  {/if}

  <section class="sv-panel">
    <header class="sv-head">
      <div class="sv-where">
        <!-- A real heading, not a bold span. It sits inside Library's h1, and it
             is the VISIBLE title — so the accessible name and the text on screen
             are the same string and cannot drift apart, which is the same reason
             the two unlabelled controls above were fixed natively. -->
        <h2>{searchMode ? 'Search results' : 'Saved scripture'}</h2>
        <!-- HOW MANY, AND WHAT A PRESS DOES — the Bible pane's caption has said
             both since §10 and this one said only the first, on a deck with the
             same cards and the same one-press rule.

             It is NOT the Bible pane's sentence, deliberately. That one also
             names what a DOUBLE press does, and `onOpen` is not passed to the
             deck here — a double press on a saved verse lands on `VerseDeck`'s
             default no-op, so borrowing that half would describe a control
             nobody built. This says only the half that is true. (Wiring the
             other half means loading the verse's chapter, which this pane has
             no reader for; it belongs to whoever owns the deck, and L5 left it
             rather than claiming it.) -->
        <span>
          {searchMode
            ? `${numbered.length} match${numbered.length === 1 ? '' : 'es'} · star one to keep it`
            : `${numbered.length} slide${numbered.length === 1 ? '' : 's'}`} · single click selects
        </span>
      </div>
      {#if checked.size}
        <button class="r-btn primary sm" on:click={queueChecked}>Queue {checked.size} selected</button>
        <button class="r-btn ghost sm" on:click={() => (checked = new Set())}>Clear</button>
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

    <div class="sv-body r-scroll">
      {#if loading}
        <Loading what="saved scripture" />
      {:else if searching}
        <Loading what="matching verses" />
      {:else if searchMode && $readErrors.searchScripture}
        <!-- RG-95. A failed search returned `[]`, which read as "no such verse" —
             so an operator hunting a reference mid-service retypes it instead of
             being told the search itself did not run. -->
        <ErrorState error={$readErrors.searchScripture} onRetry={() => doSearch(query)} />
      {:else if !searchMode && $readErrors.listSavedScripture}
        <ErrorState error={$readErrors.listSavedScripture} onRetry={load} />
      {:else if !numbered.length}
        <EmptyState
          message={searchMode
            ? `No scripture matching “${query.trim()}”.`
            : 'No saved verses yet — search above, then star one.'} />
      {:else}
        <VerseDeck
          items={pageItems}
          {template}
          {liveRef}
          rehearsing={$rehearsing}
          {checked}
          {savedRefs}
          {queuedRefs}
          busyRef={firing}
          {layout}
          press="select"
          {selectedRef}
          onSelect={selectVerse}
          onCheck={toggleCheck}
          onFire={fire}
          onQueue={toggleQueue}
          onSave={toggleSave}
          can={{ queue: true, favourite: true, edit: false, duplicate: false, add: false }} />
      {/if}
    </div>

    <footer class="sv-pager">
      <div class="sv-pages">
        <button class="sv-pg" disabled={page === 0} aria-label="Previous page" on:click={() => (page -= 1)}>‹</button>
        {#each Array(pages) as _, n}
          <button class="sv-pg" class:on={page === n} on:click={() => (page = n)}>{n + 1}</button>
        {/each}
        <button class="sv-pg" disabled={page >= pages - 1} aria-label="Next page" on:click={() => (page += 1)}>›</button>
      </div>
      <span class="sv-count">Showing {firstShown}–{lastShown} of {numbered.length} slides</span>
      <label class="sv-ctl">
        <span class="r-lbl">Items per page</span>
        <select class="r-select sm" bind:value={perPage} aria-label="Items per page">
          {#each [12, 24, 48] as n}<option value={n}>{n}</option>{/each}
        </select>
      </label>
    </footer>
  </section>
 </div>

  <!-- Announced. "John 3:16 is on the screens" is the confirmation that content
         reached a congregation, and it was silent to a screen reader — the error
         half of these panes carries `role="alert"` and this half carried nothing. -->
  {#if msg}<p class="sv-msg" role="status" aria-live="polite">{msg}</p>{/if}
  {#if error}<p class="sv-err" role="alert">{error}</p>{/if}
</div>

<style>
  .sv { display: flex; flex-direction: column; gap: 10px; min-height: 0; flex: 1; }
  /* Items left, slides right — the Library's one grammar (REBRAND §10). */
  .sv-split { display: grid; grid-template-columns: 180px minmax(0, 1fr); gap: 10px;
    flex: 1; min-height: 0; }
  @media (max-width: 1140px) { .sv-split { grid-template-columns: 156px minmax(0, 1fr); } }
  /* Below this the rail is stacked, never squeezed: a book list narrower than a
     book name is a column of ellipses. */
  @media (max-width: 860px) { .sv-split { grid-template-columns: minmax(0, 1fr); } }

  .sv-rail { display: flex; flex-direction: column; min-height: 0;
    background: var(--v-bg); border: 1px solid var(--v-line); border-radius: var(--v-r-lg); }
  .sv-railhead { padding: 10px 12px 8px; font-size: var(--v-fs-cap); font-weight: 600;
    letter-spacing: .06em; text-transform: uppercase; color: var(--v-dim);
    border-bottom: 1px solid var(--v-line); }
  .sv-raillist { flex: 1; min-height: 0; overflow-y: auto; padding: 6px; display: flex;
    flex-direction: column; gap: 2px; }
  /* A LIST ROW, not a button — B2. One book per row in the rail, selected into
     the deck beside it rather than pressed. */
  .sv-book { display: flex; align-items: center; gap: 8px; width: 100%; height: 28px;
    padding: 0 8px; border: 1px solid transparent; border-radius: var(--v-r-sm);
    background: transparent; color: var(--v-dim); font-family: var(--f-body);
    font-size:var(--v-fs-b1); text-align: left; cursor: pointer; }
  .sv-book:hover:not(.on) { background: var(--v-surf2); color: var(--v-txt); }
  /* Steel = the thing you are working on. The collection's amber lives on the
     rail's left edge only, where it tints rather than states. */
  .sv-book.on { background: var(--v-sel-soft); border-color: var(--v-sel-line); color: var(--v-txt);
    box-shadow: inset 2px 0 0 var(--v-col-scripture); }
  .sv-bn { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  /* --v-dim, not --v-faint: --v-faint on --v-surf3 is below WCAG AA and
     `tokencontrast.test.js` fails the build for it. */
  .sv-bk { flex: 0 0 auto; font-size: 10px; color: var(--v-dim); }

  .sv-panel { display: flex; flex-direction: column; min-height: 0; flex: 1;
    background: var(--v-bg); border: 1px solid var(--v-line); border-radius: var(--v-r-lg); }
  .sv-head { display: flex; align-items: center; gap: 12px; padding: 11px 14px;
    border-bottom: 1px solid var(--v-line); }
  .sv-where { flex: 1; min-width: 0; }
  /* Was a <b>; it is an <h2> now (R3-12). Same pixels, so the change is purely
     what a screen reader is told. */
  .sv-where h2 { display: block; margin: 0; font-size: var(--v-fs-ttl); font-weight: 600; color: var(--v-txt); }
  .sv-where span { font-size: var(--v-fs-cap); color: var(--v-faint); }
  .sv-body { flex: 1; min-height: 0; overflow-y: auto; padding: 12px; }
  .sv-pager { display: flex; align-items: center; gap: 12px; padding: 10px 14px;
    border-top: 1px solid var(--v-line); flex-wrap: wrap; }
  .sv-pages { display: flex; align-items: center; gap: 4px; flex: 1; }
  /* A PAGER, not a row of buttons — B2. `‹`, then one cell PER PAGE, then `›`.
     The numbered cells grow with their digits (`min-width` plus padding, not a
     fixed width), and the whole strip is one control reading "you are here, of
     this many" — the shared button's fixed metrics would make it a toolbar of
     numbered actions. Deliberate; not drift. */
  .sv-pg { min-width: 28px; height: 28px; padding: 0 7px; border-radius: var(--v-r-sm); border: 0;
    background: transparent; color: var(--v-dim); font-family: var(--f-mono); font-size:var(--v-fs-b1);
    font-variant-numeric: tabular-nums; cursor: pointer; }
  .sv-pg:hover:not(:disabled):not(.on) { background: var(--v-surf2); color: var(--v-txt); }
  .sv-pg.on { background: var(--v-accent-fill); color: var(--v-accent-ink); font-weight: 600; }
  .sv-pg:disabled { opacity: 0.35; cursor: not-allowed; }
  .sv-count { font-size: var(--v-fs-cap); color: var(--v-faint); }
  .sv-ctl { display: flex; align-items: center; gap: 7px; }
  .sv-ctl .r-lbl { margin: 0; }
  /* Width and padding only — the height is the shared control's. It was 30px,
     four pixels taller than every button on the same bar. */
  .sv-ctl .r-select { width: auto; padding: 0 30px 0 10px; font-size:var(--v-fs-b1);
    background-position: calc(100% - 14px) 11px, calc(100% - 9px) 11px; }

  .sv-msg, .sv-err { margin: 0; font-size: var(--v-fs-b2); }
  .sv-msg { color: var(--v-emerald); }
  .sv-err { color: var(--v-red); }
</style>
