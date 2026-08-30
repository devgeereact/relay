<script>
  // LIBRARY → SAVED. Verses the operator has kept, as a deck.
  //
  // The same card, rail and one-click rule as the Bible tab. Searching looks for
  // something to KEEP; an empty box shows what is already kept. There is one
  // search box in the Library — this pane used to carry a second one of its own.
  import { onMount } from 'svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
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
  } from '../../stores/capture.js';

  export let query = '';
  export let queue = [];
  export let onQueueChange = () => {};

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
  onMount(async () => {
    saved = (await listSavedScripture()) ?? [];
    template = (await listActiveTemplates().catch(() => []))[0] ?? null;
    loading = false;
  });

  let lastQuery = null;
  $: if (query !== lastQuery) {
    lastQuery = query;
    page = 0;
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

  $: searchMode = !!query?.trim();
  $: rows = searchMode ? results : saved;
  $: base = rows.map((r) => ({
    key: r.reference,
    reference: r.reference,
    label: r.reference,
    text: r.text,
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
  <section class="sv-panel">
    <header class="sv-head">
      <div class="sv-where">
        <!-- A real heading, not a bold span. It sits inside Library's h1, and it
             is the VISIBLE title — so the accessible name and the text on screen
             are the same string and cannot drift apart, which is the same reason
             the two unlabelled controls above were fixed natively. -->
        <h2>{searchMode ? 'Search results' : 'Saved scripture'}</h2>
        <span>
          {searchMode
            ? `${numbered.length} match${numbered.length === 1 ? '' : 'es'} · star one to keep it`
            : `${numbered.length} slide${numbered.length === 1 ? '' : 's'}`}
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

  {#if msg}<p class="sv-msg">{msg}</p>{/if}
  {#if error}<p class="sv-err" role="alert">{error}</p>{/if}
</div>

<style>
  .sv { display: flex; flex-direction: column; gap: 10px; min-height: 0; flex: 1; }
  .sv-panel { display: flex; flex-direction: column; min-height: 0; flex: 1;
    background: var(--v-bg); border: 1px solid var(--v-line); border-radius: var(--v-r-lg); }
  .sv-head { display: flex; align-items: center; gap: 12px; padding: 11px 14px;
    border-bottom: 1px solid var(--v-line); }
  .sv-where { flex: 1; min-width: 0; }
  /* Was a <b>; it is an <h2> now (R3-12). Same pixels, so the change is purely
     what a screen reader is told. */
  .sv-where h2 { display: block; margin: 0; font-size: 15px; font-weight: 600; color: var(--v-txt); }
  .sv-where span { font-size: var(--v-fs-cap); color: var(--v-faint); }
  .sv-body { flex: 1; min-height: 0; overflow-y: auto; padding: 12px; }
  .sv-pager { display: flex; align-items: center; gap: 12px; padding: 10px 14px;
    border-top: 1px solid var(--v-line); flex-wrap: wrap; }
  .sv-pages { display: flex; align-items: center; gap: 4px; flex: 1; }
  .sv-pg { min-width: 28px; height: 28px; padding: 0 7px; border-radius: var(--v-r-sm); border: 0;
    background: transparent; color: var(--v-dim); font-family: var(--f-mono); font-size: 12px;
    font-variant-numeric: tabular-nums; cursor: pointer; }
  .sv-pg:hover:not(:disabled):not(.on) { background: var(--v-surf2); color: var(--v-txt); }
  .sv-pg.on { background: var(--v-accent-fill); color: var(--v-accent-ink); font-weight: 600; }
  .sv-pg:disabled { opacity: 0.35; cursor: not-allowed; }
  .sv-count { font-size: var(--v-fs-cap); color: var(--v-faint); }
  .sv-ctl { display: flex; align-items: center; gap: 7px; }
  .sv-ctl .r-lbl { margin: 0; }
  .sv-ctl .r-select { width: auto; height: 30px; padding: 0 30px 0 10px; font-size: 12px;
    background-position: calc(100% - 14px) 13px, calc(100% - 9px) 13px; }

  .sv-modal { position: fixed; inset: 0; z-index: 60; display: grid; place-items: center;
    background: rgba(0, 0, 0, 0.6); padding: 24px; }
  .sv-sheet { width: min(560px, 100%); display: flex; flex-direction: column; gap: 12px;
    padding: 18px; background: var(--v-surf); border: 1px solid var(--v-line2);
    border-radius: var(--v-r-xl); box-shadow: var(--v-shadow-lg); }
  .sv-sheet header { display: flex; align-items: center; gap: 8px; }
  .sv-sheet header .r-lbl { margin: 0; }
  .sv-spring { flex: 1; }
  .sv-field { display: flex; flex-direction: column; gap: 5px; }
  .sv-text { height: auto; padding: 11px 13px; line-height: 1.6; resize: vertical;
    font-family: var(--f-body); font-size: 13.5px; }
  .sv-fine { margin: 0; font-size: var(--v-fs-cap); line-height: 1.6; color: var(--v-faint); }
  .sv-fine b { color: var(--v-dim); }

  .sv-msg, .sv-err { margin: 0; font-size: var(--v-fs-b2); }
  .sv-msg { color: var(--v-emerald); }
  .sv-err { color: var(--v-red); }
</style>
