<script>
  // LIBRARY — VERSE & TRANSLATION MANAGER (§7)
  // Reference: relay-production-interface.png panel 6.
  //
  // Book tree on the left, chapter's verses on the right, translation picker
  // above. The Library could search and it could list what an operator had
  // SAVED — but it could not open a Bible and read it, which is the one thing
  // the word "library" promises.
  //
  // ── Where this departs from the reference, and why ────────────────────────
  //
  // The mockup puts four language tabs across the top: English · Yoruba ·
  // Swahili · Hausa. **Relay ships one Bible.** The corpus is the KJV and
  // nothing else, so three of those four tabs would open on an empty shelf.
  //
  // That is not a small cosmetic difference. Relay's stated differentiator is
  // African-language scripture, and a UI that displays the languages as though
  // they were present would be making exactly the claim the project is careful
  // not to make (docs/LANGUAGES.md). So the picker is built from the
  // translations that ACTUALLY exist, and the gap is stated in words underneath
  // rather than implied by a tab that does nothing.
  //
  // Detection already understands spoken Yoruba, Kiswahili and Hausa references
  // — that is the alias table, and it is real. What is missing is verse TEXT in
  // those languages, which is a licensing and sourcing problem, not a UI one.

  import { onMount } from 'svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import SlideGrid from './SlideGrid.svelte';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import {
    listBooks,
    chapterVerses,
    listTranslations,
    getActiveTranslation,
    setActiveTranslation,
    saveScripture,
    manualFire,
    listActiveTemplates,
    searchScripture,
  } from '../../stores/capture.js';

  let books = [];
  let translations = [];
  let activeTranslation = null;
  let openBook = null;
  let book = null;
  let chapter = 1;
  let verses = [];
  let loading = true;
  let loadingChapter = false;
  let error = '';
  let msg = '';
  let firing = '';
  /** Read/Slides. Slides is the default — see SlideGrid on why. */
  let view = 'slides';
  /** Search text from the Library's one search bar. */
  export let query = '';
  let results = [];
  let searching = false;
  let lastQuery = null;

  // A search REPLACES the chapter in the slide pane rather than opening a second
  // surface: the operator is looking for something to put on the screen, and it
  // should land in the same grid, behaving the same way, whether they browsed to
  // it or searched for it.
  $: if (query !== lastQuery) {
    lastQuery = query;
    runSearch(query);
  }

  async function runSearch(q) {
    if (!q?.trim()) {
      results = [];
      return;
    }
    searching = true;
    try {
      results = (await searchScripture(q.trim())) ?? [];
    } catch (e) {
      error = humanError(e);
      results = [];
    }
    searching = false;
  }
  // The template the OUTPUT would actually use, so a thumbnail is the real thing
  // rather than a drawing of it.
  let template = null;

  onMount(async () => {
    try {
      const [b, t, a, tpls] = await Promise.all([
        listBooks(),
        listTranslations(),
        getActiveTranslation(),
        listActiveTemplates().catch(() => []),
      ]);
      books = b;
      translations = t;
      activeTranslation = a;
      template = tpls[0] ?? null;
      // Open on Genesis 1 rather than an empty pane: a Bible that opens closed
      // makes the operator do a click that has exactly one sensible answer.
      if (books.length) await open(books[0].book, 1);
    } catch (e) {
      error = humanError(e);
    }
    loading = false;
  });

  async function open(b, ch) {
    book = b;
    openBook = b;
    chapter = ch;
    loadingChapter = true;
    error = '';
    try {
      verses = await chapterVerses(b, ch);
    } catch (e) {
      error = humanError(e);
      verses = [];
    }
    loadingChapter = false;
  }

  const toggle = (b) => (openBook = openBook === b.book ? null : b.book);

  async function pickTranslation(id) {
    activeTranslation = id;
    await setActiveTranslation(id);
    books = await listBooks();
    if (book) await open(book, chapter);
  }

  async function save(v) {
    msg = '';
    try {
      await saveScripture(`${v.book} ${v.chapter}:${v.verse}`);
      msg = `Saved ${v.book} ${v.chapter}:${v.verse}`;
    } catch (e) {
      error = humanError(e);
    }
  }

  // Putting a verse on the wall FROM THE LIBRARY is a real fire, on the real
  // outputs — it is the same `manual_fire` the console uses, and it is labelled
  // so nobody clicks it thinking they are previewing.
  async function fire(v) {
    const ref = v.reference ?? `${v.book} ${v.chapter}:${v.verse}`;
    msg = '';
    error = '';
    firing = v.key ?? ref;
    try {
      await manualFire(ref);
      msg = `${ref} is on the screens`;
    } catch (e) {
      error = humanError(e);
    }
    firing = '';
  }

  // One shape for the grid, whatever the source is — browsed chapter or search
  // results.
  $: source = query?.trim() ? results : verses;
  $: slides = source.map((v) => ({
    key: `${v.book} ${v.chapter}:${v.verse}`,
    reference: `${v.book} ${v.chapter}:${v.verse}`,
    text: v.text,
    translation: v.abbreviation,
    book: v.book,
    chapter: v.chapter,
    verse: v.verse,
  }));

  $: chapterList = books.find((b) => b.book === openBook)?.chapters ?? 0;
</script>

<div class="br">
  <header class="br-bar">
    {#if translations.length > 1}
      <div class="br-tabs" role="tablist" aria-label="Translation">
        {#each translations as t}
          <button
            role="tab"
            aria-selected={activeTranslation === t.id}
            class:on={activeTranslation === t.id}
            on:click={() => pickTranslation(t.id)}>
            {t.abbreviation || t.name}
          </button>
        {/each}
      </div>
    {:else if translations.length === 1}
      <span class="br-one r-mono">{translations[0].abbreviation || translations[0].name}</span>
    {/if}
    <span class="spring"></span>
    {#if query?.trim()}
      <span class="br-where">{slides.length} result{slides.length === 1 ? '' : 's'} for “{query.trim()}”</span>
    {:else if book}
      <span class="br-where">{book} {chapter}</span>
    {/if}
    <div class="br-view" role="group" aria-label="View">
      <button class:on={view === 'slides'} on:click={() => (view = 'slides')}>Slides</button>
      <button class:on={view === 'read'} on:click={() => (view = 'read')}>Read</button>
    </div>
  </header>

  {#if loading}
    <Loading what="the library" />
  {:else if !books.length}
    <EmptyState message="No scripture is loaded. Check Settings → data health." />
  {:else}
    <div class="br-grid">
      <!-- BOOKS. Canonical order, from the backend — never alphabetical. -->
      <nav class="br-books r-scroll" aria-label="Books">
        {#each books as b}
          <button class="br-book" class:open={openBook === b.book} on:click={() => toggle(b)}>
            <span class="br-caret" class:down={openBook === b.book}>›</span>
            {b.book}
          </button>
          {#if openBook === b.book}
            <div class="br-chapters">
              {#each Array(chapterList) as _, i}
                <button
                  class="br-ch"
                  class:on={book === b.book && chapter === i + 1}
                  on:click={() => open(b.book, i + 1)}>
                  Chapter {i + 1}
                </button>
              {/each}
            </div>
          {/if}
        {/each}
      </nav>

      <!-- THE CHAPTER. -->
      <div class="br-read r-scroll">
        {#if loadingChapter}
          <Loading what="the chapter" />
        {:else if searching}
          <Loading what="matching verses" />
        {:else if query?.trim() && !slides.length}
          <EmptyState message={`No scripture matching “${query.trim()}”.`} />
        {:else if view === 'slides'}
          <!-- SLIDES FIRST. An operator picks by looking, not by reading a list
               of numbers — see SlideGrid. Clicking one FIRES it. -->
          <SlideGrid items={slides} {template} onFire={fire} busyKey={firing} />
        {:else if verses.length}
          {#each verses as v}
            <div class="br-v">
              <span class="br-n r-mono">{v.chapter}:{v.verse}</span>
              <p class="br-t">{v.text}</p>
              <span class="br-acts">
                <button on:click={() => save(v)} title="Save to the library">Save</button>
                <button class="go" on:click={() => fire(v)} title="Put this on the output screens">
                  Put on screen
                </button>
              </span>
            </div>
          {/each}
        {:else}
          <EmptyState message="That chapter is empty." />
        {/if}
      </div>
    </div>

    <!-- The honest note about the three missing Bibles. See the header comment. -->
    <p class="br-note">
      Relay ships the <b>King James Version</b> only. It already <em>recognises</em> spoken
      references in Yorùbá, Kiswahili and Hausa — what is missing is verse text in those
      languages, which is a sourcing and licensing problem rather than a feature. Adding one
      is a data import, not a code change.
    </p>
  {/if}

  {#if msg}<p class="br-msg">{msg}</p>{/if}
  {#if error}<p class="br-err">{error}</p>{/if}
</div>

<style>
  .br {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }
  .br-bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .spring {
    flex: 1;
  }
  .br-tabs {
    display: flex;
    gap: 4px;
  }
  .br-tabs button {
    padding: 7px 14px;
    background: none;
    border: 0;
    border-bottom: 2px solid transparent;
    color: var(--v-faint);
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .br-tabs button.on {
    color: var(--v-accent2);
    border-bottom-color: var(--v-accent);
  }
  .br-one,
  .br-where {
    font-size: 11px;
    color: var(--v-faint);
  }

  .br-grid {
    display: grid;
    grid-template-columns: 218px minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
    height: clamp(360px, 58vh, 620px);
  }
  .br-books,
  .br-read {
    overflow-y: auto;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    padding: 8px;
  }
  .br-book {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--v-r-md);
    background: none;
    border: 0;
    color: var(--v-dim);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .br-book:hover {
    background: var(--v-surf2);
    color: var(--v-txt);
  }
  .br-book.open {
    color: var(--v-txt);
  }
  .br-caret {
    display: inline-block;
    transition: transform 0.12s;
    color: var(--v-faint);
  }
  .br-caret.down {
    transform: rotate(90deg);
  }
  .br-chapters {
    display: flex;
    flex-direction: column;
    padding: 2px 0 6px 22px;
  }
  .br-ch {
    padding: 6px 10px;
    border-radius: var(--v-r-sm);
    background: none;
    border: 0;
    color: var(--v-faint);
    font: inherit;
    font-size: 12.5px;
    text-align: left;
    cursor: pointer;
  }
  .br-ch:hover {
    color: var(--v-txt);
  }
  /* SELECTION is chrome, so it is the accent — not amber. The reference paints
     the selected verse gold; gold means on air in this app, and a verse being
     READ in the library is not on anybody's screen (DECISIONS §22). */
  .br-ch.on {
    background: var(--v-accent-soft);
    color: var(--v-accent2);
    font-weight: 600;
  }

  .br-v {
    display: grid;
    grid-template-columns: 46px minmax(0, 1fr) auto;
    gap: 12px;
    align-items: start;
    padding: 10px 8px;
    border-radius: var(--v-r-md);
  }
  .br-v:hover {
    background: var(--v-surf2);
  }
  .br-v:hover .br-acts {
    opacity: 1;
  }
  .br-n {
    font-size: 11px;
    color: var(--v-faint);
    padding-top: 3px;
  }
  .br-t {
    margin: 0;
    font-family: var(--f-serif);
    font-size: 15px;
    line-height: 1.6;
    color: var(--v-txt);
  }
  .br-acts {
    display: flex;
    gap: 6px;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .br-acts button {
    padding: 5px 10px;
    border-radius: var(--v-r-sm);
    background: var(--v-surf2);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
    white-space: nowrap;
  }
  .br-acts button:hover {
    color: var(--v-txt);
  }
  /* AMBER, deliberately: this button puts scripture in front of a congregation.
     It is the one control on this screen that is genuinely an ON AIR ACTION. */
  .br-acts button.go {
    border-color: rgba(255, 176, 0, 0.4);
    color: var(--v-amber);
  }
  .br-acts button.go:hover {
    background: var(--v-amber-soft);
  }

  .br-note,
  .br-msg,
  .br-err {
    margin: 0;
    font-size: var(--v-fs-b2);
    line-height: 1.6;
  }
  .br-note {
    color: var(--v-faint);
    max-width: 78ch;
  }
  .br-note b {
    color: var(--v-dim);
  }
  .br-msg {
    color: var(--v-emerald);
  }
  .br-err {
    color: var(--v-red);
  }

  @media (max-width: 860px) {
    .br-grid {
      grid-template-columns: 1fr;
      height: auto;
    }
  }
</style>
