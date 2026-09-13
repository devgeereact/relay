<script>
  // THE LIVE RAIL — how an operator finds something that is not in the plan.
  //
  // The prototype's Live workspace opens with a 206px rail: a search field and
  // the books with their chapter counts. Relay's Live started at the Preview
  // monitor, so the ONLY way to reach a verse nobody had planned for was to type
  // it into the manual box and fire it blind. That is the preacher-goes-off-
  // script case — the entire reason this product exists — and it had no browsing
  // surface at all.
  //
  // THE ONE RULE THIS FILE KEEPS: nothing here reaches a screen. A search, a
  // book, a chapter — every press STAGES into the slide grid, and the grid is
  // where a press becomes a take (and reports its own outcome, per CLAUDE.md
  // rules 15 and 18). A rail that fired would make a mistyped search a
  // congregation-facing event.
  //
  // Search goes through `searchScripture` in the store — the wrapper the Library
  // already uses. A second search path would be a second answer to "what does
  // this query mean", and the two would drift.
  //
  // WHAT IS DELIBERATELY NOT HERE: the prototype's Songs half of this rail. A
  // song section has no canonical reference, so a song cell could not be fired
  // through `manualFire` (which resolves a reference) or `fireSlide` (which
  // needs a plan cue) — it would need `fireContent`, and giving the grid a third
  // wrapper is a decision about the fire path, not about a rail. Left out rather
  // than taken quietly; see the note to the team lead.
  import { onMount, onDestroy } from 'svelte';
  import { listBooks, searchScripture, readErrors } from './stores/capture.js';
  import EmptyState from './ui/EmptyState.svelte';
  import ErrorState from './ui/ErrorState.svelte';
  import Loading from './ui/Loading.svelte';

  /** Stage one chapter in the slide grid. Never fires. */
  export let onChapter = () => {};
  /** The engine is not attached — every list here is empty and every press dead. */
  export let disabled = false;

  let q = '';
  let books = [];
  let booksLoaded = false;
  let openBook = null;

  // Search results, and whether we have asked yet. `[]` before the first call is
  // not "nothing matches" — that sentence is the one that makes an operator
  // think their Bible is missing.
  let hits = [];
  let hitsFor = null;
  let searchT;

  onMount(loadBooks);
  onDestroy(() => clearTimeout(searchT));

  async function loadBooks() {
    booksLoaded = false;
    books = await listBooks();
    booksLoaded = true;
  }

  // Debounced, because `search_scripture` runs a semantic pass over the corpus
  // and an operator types a reference one character at a time.
  function armSearch(text) {
    clearTimeout(searchT);
    const query = text.trim();
    if (query.length < 2) {
      hits = [];
      hitsFor = null;
      return;
    }
    searchT = setTimeout(async () => {
      const asked = query;
      const rows = await searchScripture(asked);
      // A slow search must not label itself with a query the operator has since
      // typed past.
      if (asked === q.trim()) {
        hits = rows ?? [];
        hitsFor = asked;
      }
    }, 220);
  }
  $: armSearch(q);

  const toggleBook = (b) => (openBook = openBook === b.book ? null : b.book);
  const chapterList = (n) => Array.from({ length: Math.max(0, n) }, (_, i) => i + 1);
</script>

<aside class="lrail" aria-label="Scripture">
  <div class="lr-head">
    <span class="lr-k r-mono">Scripture</span>
  </div>

  <div class="lr-head">
    <input
      class="lr-q"
      type="search"
      bind:value={q}
      {disabled}
      placeholder="ps 23 1 · seek ye first…"
      aria-label="Search scripture" />
  </div>

  <div class="lr-body">
    {#if hitsFor}
      <!-- A search STAGES the chapter it found. It does not fire: the grid below
           is where a press becomes a take. -->
      <p class="lr-cap">{hits.length} found · opens the chapter in the grid</p>
      {#each hits as h (h.id)}
        <button class="lr-row" {disabled} on:click={() => onChapter(h.book, h.chapter)}>
          <span class="lr-i" aria-hidden="true">✦</span>
          <span class="lr-n"><b>{h.reference}</b> · {h.text.slice(0, 46)}…</span>
        </button>
      {:else}
        {#if $readErrors.searchScripture}
          <ErrorState compact error={$readErrors.searchScripture} onRetry={() => armSearch(q)} />
        {:else}
          <EmptyState message={`Nothing matches “${hitsFor}”. Try a reference (ps 23 1) or the words you remember.`} />
        {/if}
      {/each}
    {:else}
      {#each books as b (b.book)}
        <button class="lr-row" {disabled} aria-expanded={openBook === b.book} on:click={() => toggleBook(b)}>
          <span class="lr-i" aria-hidden="true">{openBook === b.book ? '▾' : '▸'}</span>
          <span class="lr-n">{b.book}</span>
          <span class="lr-k r-mono">{b.chapters}</span>
        </button>
        {#if openBook === b.book}
          <div class="lr-chips">
            {#each chapterList(b.chapters) as c}
              <button class="lr-chip" {disabled} on:click={() => onChapter(b.book, c)}
                aria-label={`${b.book} chapter ${c}`}>{c}</button>
            {/each}
          </div>
        {/if}
      {:else}
        {#if !booksLoaded}
          <Loading what="books" compact />
        {:else if $readErrors.listBooks}
          <ErrorState compact error={$readErrors.listBooks} onRetry={loadBooks} />
        {:else}
          <EmptyState message="No Bible text loaded yet — import a translation in Settings." />
        {/if}
      {/each}
    {/if}
  </div>
</aside>

<style>
  /* 206px, from the prototype. It is a browsing column, so it is quieter than
     anything on the take path: no amber anywhere, and selection is steel blue. */
  .lrail {
    flex: 0 0 auto;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    overflow: hidden;
  }
  .lr-head {
    flex: 0 0 auto;
    padding: 6px 7px;
    border-bottom: 1px solid var(--v-line);
  }

  .lr-q {
    width: 100%; box-sizing: border-box; height: 26px; padding: 0 8px;
    background: var(--v-void); border: 1px solid var(--v-line2); border-radius: var(--v-r-sm);
    color: var(--v-txt); font-family: var(--f-body); font-size: var(--v-fs-b2);
  }
  .lr-q::placeholder { color: var(--v-faint); }
  .lr-q:focus { outline: 0; border-color: var(--v-sel-line); }
  .lr-q:disabled { opacity: .5; }

  .lr-body {
    flex: 1; min-height: 0; overflow-y: auto; padding: 4px;
    display: flex; flex-direction: column; gap: 1px;
    scrollbar-width: thin; scrollbar-color: var(--v-surf3) transparent;
  }
  .lr-body::-webkit-scrollbar { width: 6px; }
  .lr-body::-webkit-scrollbar-thumb { background: var(--v-surf3); border-radius: 99px; }

  .lr-cap {
    margin: 0; padding: 4px 6px 2px;
    font-family: var(--f-mono); font-size: var(--v-fs-cap); letter-spacing: var(--v-tr-caps);
    color: var(--v-dim);
  }
  .lr-row {
    display: flex; align-items: center; gap: 7px; width: 100%; text-align: left;
    padding: 5px 7px; border: 0; border-radius: var(--v-r-sm); cursor: pointer;
    background: transparent; color: var(--v-dim); font-family: var(--f-body);
  }
  .lr-row:hover:not(:disabled) { background: var(--v-surf3); color: var(--v-txt); }
  .lr-row:disabled { opacity: .5; cursor: not-allowed; }
  .lr-row[aria-expanded='true'] { background: var(--v-sel-soft); color: var(--v-txt); }
  .lr-i { flex: 0 0 auto; width: 12px; text-align: center; font-size: var(--v-fs-cap); color: var(--v-dim); }
  .lr-n { flex: 1; min-width: 0; font-size: var(--v-fs-b2); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  /* --v-dim, not --v-faint: this count sits on --v-surf3 on hover, and muted text
     on that pairing is below AA (tokencontrast.test.js fails the build for it). */
  .lr-k {
    flex: 0 0 auto; font-size: var(--v-fs-cap); letter-spacing: var(--v-tr-caps);
    text-transform: uppercase; color: var(--v-dim);
  }

  .lr-chips { display: flex; flex-wrap: wrap; gap: 3px; padding: 4px 8px 8px; }
  .lr-chip {
    min-width: 22px; padding: 2px 5px; border-radius: var(--v-r-sm); cursor: pointer;
    background: var(--v-surf3); border: 1px solid var(--v-line2); color: var(--v-dim);
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
  }
  .lr-chip:hover:not(:disabled) { border-color: var(--v-sel-line); color: var(--v-txt); }
  .lr-chip:disabled { opacity: .5; cursor: not-allowed; }
</style>
