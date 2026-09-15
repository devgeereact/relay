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
  // WHAT A PRESS MEANS HERE (docs/REBRAND.md §2 and §9).
  //
  // BROWSING — a book, a chapter — only ever STAGES into the slide grid. There
  // is no single verse in "Psalms 23" to put on a wall, so there is nothing for
  // a press to send.
  //
  // A SEARCH HIT names one verse, and §9 asks one click to do the whole job:
  // the verse goes to the programme, its chapter loads into the grid, and that
  // verse is the active slide. So a single press does exactly that and a DOUBLE
  // press stages the chapter without putting anything on a screen — the same
  // grammar as the grid, through the same `pressArbiter`, so the 190ms beat
  // means a double can never also fire. Three things keep this honest:
  //
  //   · it is not a new fire path. `onVerse` lands on the SAME `manualFire` the
  //     grid's verse cells use, so the fire is `'manual'` (rule 14), passes the
  //     pre-air validator (rule 36) and reports its own outcome (rule 15);
  //   · nothing here auto-fires. A search is an operator action, start to
  //     finish; rule 10 is untouched, and `search.rs` holds that boundary from
  //     the Rust side;
  //   · every hit says WHICH KIND of claim it is and why (rule 18). A guess is
  //     cyan and carries no percentage — never amber, which means ON AIR.
  //
  // Search goes through `searchScripture` in the store — the wrapper the Library
  // already uses. A second search path would be a second answer to "what does
  // this query mean", and the two would drift.
  //
  // ── THE SONGS HALF (docs/REBRAND.md §2) ────────────────────────────────────
  //
  // The rail used to be scripture only, and said so: a song section has no
  // canonical reference, so a song cell cannot be fired through `manualFire`.
  // That objection was about the GRID, not about the rail. The rail only ever
  // STAGES — pressing a song loads its slides into the grid and touches no
  // screen — and the grid's press then goes through `fireContent`, which is the
  // path `LyricsPane` has fired songs down since phase 1. No third wrapper, no
  // new fire path, and nothing here auto-fires.
  //
  // ── ONE BOX, TWO JOBS (§9) ─────────────────────────────────────────────────
  //
  // The console's right-hand column used to carry a second scripture box with a
  // `Fire` button — the one path that still works when the AI is wrong, the
  // model is missing or the verse is a RANGE the corpus cannot offer as a single
  // hit. §9 asks for one box, so that job came here rather than being deleted:
  // `Fire` beside this field sends the reference exactly as typed, ranges
  // included, through the same `manualFire`. It is offered only when the query
  // holds a digit, because a reference has a number in it and a half-remembered
  // phrase does not — a fire nobody could satisfy is not a control.
  //
  // ── THE CHAPTER PICKER (L5) ────────────────────────────────────────────────
  //
  // Browsing is two presses — a book, then a chapter — and it stays two. The
  // search box already takes `ps 119 105` the whole way in one go (`search.rs`,
  // tested), so the grid is for the operator who is LOOKING rather than typing,
  // and a third input would be a second answer to "what does this query mean".
  // What was wrong was not the number of presses but that neither one was
  // certain:
  //
  //   · the chapter grid was a `flex-wrap` of chips sized by their own labels,
  //     so `1` and `50` drew different boxes, every row held a different count,
  //     and no column lined up with the one above it. Fifty of those, in seven
  //     ragged rows, is the screenshot this work started from;
  //   · it was inline in the one scroller the 66 books share, so opening Psalms
  //     inserted 25 rows of chips into the middle of the book list and pushed
  //     the 47 books after it off the rail;
  //   · opening a book near the fold put its chapters below the fold, with
  //     nothing to say they had appeared;
  //   · a one-chapter book (Jude, Obadiah, Philemon, 2 and 3 John) drew a
  //     picker offering exactly one choice — a press that asks nothing;
  //   · and coming back to a book started again at the top, however far down
  //     the operator had been.
  //
  // All five are fixed below and none of them moves what a press MEANS.
  import { onMount, onDestroy, tick } from 'svelte';
  import { listBooks, searchScripture, listSongs, searchSongs, readErrors } from './stores/capture.js';
  import { pressArbiter } from './slidegrid.js';
  import EmptyState from './ui/EmptyState.svelte';
  import ErrorState from './ui/ErrorState.svelte';
  import Loading from './ui/Loading.svelte';

  /** Stage one chapter in the slide grid. Never fires. */
  export let onChapter = () => {};
  /** Stage one SONG in the slide grid, by id. Never fires — see the note above. */
  export let onSong = () => {};
  /**
   * Fire the reference exactly as it was typed — ranges included. This is the
   * manual box that used to live in the inspector column, and it is still the
   * floor under the AI: the caller owns the fire, so the caller owns the
   * outcome, and nothing here says a screen changed.
   */
  export let onReference = () => {};
  /**
   * ONE SEARCH HIT, taken the whole way: the chapter into the grid AND the verse
   * to the programme (§9). The caller owns the fire, so the caller owns the
   * outcome — this component never says a screen changed.
   */
  export let onVerse = () => {};
  /** The engine is not attached — every list here is empty and every press dead. */
  export let disabled = false;

  let q = '';
  let books = [];
  let booksLoaded = false;
  let openBook = null;

  /** Which collection the rail is showing — 'bible' or 'songs' (§2). */
  let tab = 'bible';
  let songs = [];
  let songsLoaded = false;
  let songsAsked = false;

  /**
   * The search field, so the `/` shortcut can still reach it.
   *
   * The console's `search` context used to focus the inspector's manual box.
   * That box is this one now, so the focus has to come with it — otherwise the
   * shortcut survives as a key that quietly does nothing, which is the failure
   * `NavResult` exists to prevent, one surface along.
   */
  let qEl;
  export function focus() {
    qEl?.focus();
    qEl?.select?.();
  }

  /**
   * Is this query a REFERENCE the operator wants fired as typed?
   *
   * A display test, never a fire. It decides whether the `Fire` button is
   * offered beside the box — nothing about it reaches a screen on its own, and a
   * query it lets through that the backend cannot parse comes back as an error
   * the caller renders. A reference has a number in it; the words you half
   * remember do not.
   */
  export const looksLikeReference = (text) => /\d/.test(String(text ?? ''));
  /**
   * TWO QUESTIONS, NOT ONE (L2), because they have different answers on screen.
   *
   *   `fireable`  — is this query a reference at all? If it is not, there is
   *                 nothing for a Fire button to do and the prototype's rail
   *                 draws none. A permanently greyed button beside an empty
   *                 search box is a control that spends its whole life saying
   *                 no, and it teaches an operator to stop reading the rail.
   *   `canFire`   — and is the engine there to take it? THIS one still greys the
   *                 button rather than removing it: an operator who has typed a
   *                 real reference and cannot send it must be shown the control
   *                 they are looking for, disabled, with the reason in `title`.
   *                 Removing it would read as "Relay has no manual fire", which
   *                 is the floor under the AI (§9) and is exactly what it has.
   */
  $: fireable = tab === 'bible' && looksLikeReference(q) && q.trim().length >= 2;
  $: canFire = fireable && !disabled;

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
    books = (await listBooks()) ?? [];
    booksLoaded = true;
  }

  /**
   * The song list, fetched the first time the Songs half is opened and on every
   * query after that.
   *
   * `songsAsked` is the same distinction the hits keep: `[]` before the first
   * call is not "you have no songs", and that sentence is the one that makes an
   * operator think their library is gone.
   */
  async function loadSongs(query) {
    songsLoaded = false;
    songsAsked = true;
    const text = String(query ?? '').trim();
    songs = (text ? await searchSongs(text) : await listSongs()) ?? [];
    songsLoaded = true;
  }
  $: if (tab === 'songs') loadSongs(q);

  function setTab(next) {
    if (tab === next) return;
    tab = next;
    // The query means a different thing on each half, and carrying it across
    // shows "nothing matches" over a collection the operator has not searched.
    q = '';
    hits = [];
    hitsFor = null;
  }

  // Debounced, because `search_scripture` runs a semantic pass over the corpus
  // and an operator types a reference one character at a time.
  function armSearch(text) {
    clearTimeout(searchT);
    const query = tab === 'bible' ? text.trim() : '';
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
  $: armSearch(q, tab);

  /**
   * WHERE YOU WERE — the chapter this rail last opened, `{ book, chapter }`.
   *
   * It is a fact about the rail's OWN last action: it is written by `stage()`,
   * the one function that calls `onChapter`, so unlike a derived badge it cannot
   * quietly describe something that is no longer true (rule 35). It is not a
   * claim about a screen, and it is drawn in steel for that reason — never
   * amber, which means ON AIR, and never cyan, which means the AI guessed.
   */
  let openedChapter = null;

  /** The scroller, so an opened book can be brought into view. */
  let bodyEl;

  /**
   * Open a chapter in the slide grid. The ONE place `onChapter` is called, so
   * the mark above and the action can never disagree.
   */
  function stage(book, chapter) {
    openedChapter = { book, chapter };
    onChapter(book, chapter);
  }

  async function toggleBook(b) {
    if (openBook === b.book) {
      openBook = null;
      return;
    }
    // ONE PRESS FOR A ONE-CHAPTER BOOK. A picker offering exactly one choice is
    // a decision that asks nothing; this is the same action the single chip
    // would have taken, one press earlier. It still only stages.
    if (b.chapters === 1) {
      openBook = null;
      stage(b.book, 1);
      return;
    }
    openBook = b.book;
    // `tick` in an EVENT HANDLER, not in a reactive block — rule 1 is about
    // `$:`, where it re-enters the scheduler and hard-freezes the webview.
    await tick();
    reveal();
  }

  /**
   * Put the book that was just opened, and the chapter it was left on, where the
   * operator can see them. Two scrollers: the rail's own, and the picker's.
   * `scrollIntoView` is absent in jsdom, hence the optional call.
   */
  function reveal() {
    bodyEl?.querySelector('.lr-row[aria-expanded="true"]')?.scrollIntoView?.({ block: 'nearest' });
    bodyEl?.querySelector('.lr-chip[aria-current="true"]')?.scrollIntoView?.({ block: 'nearest' });
  }

  const chapterList = (n) => Array.from({ length: Math.max(0, n) }, (_, i) => i + 1);

  // Single press = the whole job; double press = stage the chapter and nothing
  // more. ONE arbiter, the grid's, so a double can never also fire — the
  // alternative is that every attempt to look at a verse puts it on the wall on
  // the way past. A rejected fire goes to `onError`, never to a claim of success.
  const hitPress = pressArbiter({
    send: (h) => onVerse(h.book, h.chapter, h.verse, h.reference),
    preview: (h) => stage(h.book, h.chapter),
  });
  onDestroy(hitPress.cancel);

  /**
   * What this hit is offering, in one line the operator can read (§9, rule 18).
   *
   * The sentence is the BACKEND's — composed once in `search.rs` so the Library,
   * the Planner, this rail and the preacher's remote cannot describe the same
   * match four different ways. The fallback exists only for a backend older than
   * this field; it says nothing rather than inventing a reason.
   */
  const why = (h) => h?.why ?? '';
</script>

<aside class="lrail" aria-label="Library rail">
  <!-- BOTH COLLECTIONS (docs/REBRAND.md §2). The rail named itself SCRIPTURE and
       offered nothing else, so the one thing a service is half made of could
       only be reached by leaving the run surface. -->
  <div class="lr-head">
    <div class="seg lr-seg" role="group" aria-label="Collection">
      <button class:on={tab === 'bible'} aria-pressed={tab === 'bible'} on:click={() => setTab('bible')}>Bible</button>
      <button class:on={tab === 'songs'} aria-pressed={tab === 'songs'} on:click={() => setTab('songs')}>Songs</button>
    </div>
  </div>

  <div class="lr-head lr-qrow">
    <input
      class="lr-q"
      type="search"
      bind:this={qEl}
      bind:value={q}
      {disabled}
      on:keydown={(e) => { if (e.key === 'Enter' && canFire) onReference(q.trim()); }}
      placeholder={tab === 'bible' ? 'ps 23 1 · seek ye first…' : 'song title or author'}
      aria-label={tab === 'bible' ? 'Search scripture' : 'Search songs'} />
    <!-- THE FLOOR UNDER THE AI, in the one box §9 asks for. It sends the
         reference exactly as typed — `ps 23`, `John 3:16-18` — through the same
         `manualFire` a grid cell takes. Amber is forbidden here: this button
         does not mean ON AIR, it means "send this", and the fire reports its own
         outcome through the caller (rules 15 and 18). -->
    {#if fireable}
      <button
        class="r-btn lr-fire"
        disabled={!canFire}
        title={disabled
          ? 'Relay\u2019s engine is not attached, so nothing here can reach a screen.'
          : 'Send this reference to the programme, exactly as typed \u2014 ranges included'}
        on:click={() => onReference(q.trim())}>Fire</button>
    {/if}
  </div>

  <div class="lr-body" bind:this={bodyEl}>
    {#if tab === 'songs'}
      <!-- A press STAGES the song's slides in the grid and touches no screen.
           The grid's own press is what puts a section on the wall. -->
      {#each songs as s (s.id)}
        <button class="lr-row" {disabled} on:click={() => onSong(s.id, s.title)}
          title="Open “{s.title}” in the slide grid — nothing reaches a screen until you press a slide">
          <span class="lr-i" aria-hidden="true">♪</span>
          <span class="lr-n">{s.title}</span>
          <span class="lr-k r-mono">{s.section_count}</span>
        </button>
      {:else}
        {#if !songsAsked || !songsLoaded}
          <Loading what="songs" compact />
        {:else if $readErrors.listSongs || $readErrors.searchSongs}
          <ErrorState compact error={$readErrors.listSongs ?? $readErrors.searchSongs} onRetry={() => loadSongs(q)} />
        {:else if q.trim()}
          <EmptyState message={`No song matches “${q.trim()}”.`} />
        {:else}
          <EmptyState message="No songs yet — add one in the Library." />
        {/if}
      {/each}
    {:else if hitsFor}
      <!-- Single press sends the verse to the programme and opens its chapter in
           the grid; double press only opens the chapter. Both through the grid's
           own arbiter — see the note at the top of this file. -->
      <p class="lr-cap">{hits.length} found · click sends · double click opens the chapter</p>
      {#each hits as h (h.id)}
        <button
          class="lr-row lr-hit"
          {disabled}
          on:click={() => hitPress.press(h)}
          on:dblclick={() => hitPress.double(h)}
          title="Click to send {h.reference} to the programme · double click to open its chapter">
          <span class="lr-i" aria-hidden="true">✦</span>
          <span class="lr-n">
            <span class="lr-ref"><b>{h.reference}</b> · {h.text.slice(0, 46)}…</span>
            {#if why(h)}
              <!-- WHY IT MATCHED. Cyan when Relay guessed, muted when it simply
                   read what was typed. Never amber: amber means ON AIR. -->
              <span class="lr-why" class:guess={h.guess}>{why(h)}</span>
            {/if}
          </span>
        </button>
      {:else}
        {#if $readErrors.searchScripture}
          <ErrorState compact error={$readErrors.searchScripture} onRetry={() => armSearch(q, tab)} />
        {:else}
          <EmptyState message={`Nothing matches “${hitsFor}”. Try a reference (ps 23 1) or the words you remember.`} />
        {/if}
      {/each}
    {:else}
      {#each books as b (b.book)}
        <button
          class="lr-row"
          {disabled}
          aria-expanded={openBook === b.book}
          title={b.chapters === 1
            ? `Open ${b.book} in the slide grid — nothing reaches a screen`
            : `Show the ${b.chapters} chapters of ${b.book}`}
          on:click={() => toggleBook(b)}>
          <span class="lr-i" aria-hidden="true">{openBook === b.book ? '▾' : '▸'}</span>
          <span class="lr-n">{b.book}</span>
          <span class="lr-k r-mono">{b.chapters}</span>
        </button>
        {#if openBook === b.book}
          <div class="lr-chapters">
            <!-- WHAT A PRESS DOES, SAID WHERE THE PRESS HAPPENS. The search half
                 above carries its own, different sentence, and the two are
                 deliberately not the same: §9 makes a single press on a SEARCH
                 HIT send the verse to the programme, while browsing only ever
                 opens a chapter in the grid. One legend over both meanings is
                 the sentence that reads the same whether or not a congregation
                 is looking at something (rule 35). -->
            <p class="lr-cap lr-chapcap">Opens in the grid · no screen changes</p>
            <div class="lr-chips">
              {#each chapterList(b.chapters) as c}
                <button
                  class="lr-chip"
                  {disabled}
                  aria-current={openedChapter?.book === b.book && openedChapter?.chapter === c
                    ? 'true'
                    : undefined}
                  title={openedChapter?.book === b.book && openedChapter?.chapter === c
                    ? `${b.book} ${c} — the chapter you opened from here`
                    : `Open ${b.book} ${c} in the slide grid — nothing reaches a screen`}
                  on:click={() => stage(b.book, c)}
                  aria-label={`${b.book} chapter ${c}`}>{c}</button>
              {/each}
            </div>
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

  /* The collection switch. `.seg` is the shell's own segmented control (app.css),
     so this only makes the two halves share the width. */
  /* THE COLLECTION SWITCH. `.seg` is defined in `Live.svelte`, and Svelte scopes a
     component's styles to that component — so this rail's two buttons matched no
     rule at all and rendered as PLATFORM buttons: measured `rgb(239,239,239)` on
     black text with square corners, in a dark control room. The prototype's own
     `.seg` is the shape copied here (§12's one instrument): a well on the app
     ground, a hairline, and the pressed half filled with selection blue. */
  .lr-seg {
    display: flex; width: 100%; gap: 2px; padding: 2px;
    background: var(--v-void); border: 1px solid var(--v-line2);
    border-radius: var(--v-r-sm);
  }
  .lr-seg :global(button) {
    flex: 1 1 0; height: 22px; padding: 0 10px; border: 0; cursor: pointer;
    border-radius: var(--v-r-sm); background: transparent; color: var(--v-faint);
    font-family: var(--f-body); font-size: var(--v-fs-b2); font-weight: 600;
    transition: background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease);
  }
  .lr-seg :global(button:hover) { color: var(--v-dim); }
  .lr-seg :global(button.on),
  .lr-seg :global(button[aria-pressed='true']) {
    background: var(--v-accent-fill); color: var(--v-accent-ink);
  }
  /* Feedback on the press itself, not on the release (apple-design §1). */
  .lr-seg :global(button:active) { transform: scale(0.97); }
  @media (prefers-reduced-motion: reduce) {
    .lr-seg :global(button:active) { transform: none; filter: brightness(1.15); }
  }

  /* The box and its one action on one row: §9's "one box", with the reference
     fire beside it rather than in a second field on another panel. */
  .lr-qrow { display: flex; align-items: center; gap: 5px; }
  /* THE SHARED BUTTON (B1's `.r-btn`), with the console's mono caps on it. This
     was a hand-rolled 26px box drawing `--v-surf3` on `--v-line2` — which is
     `.r-btn`'s HOVER fill and a different hairline from the search box it sits
     beside, so it opened one shade too light with nowhere to go under the
     cursor. The metrics, the fill, the hover and the disabled state are the
     shared control's now; only the type is local. */
  .lr-fire {
    flex: 0 0 auto;
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
    letter-spacing: var(--v-tr-caps); text-transform: uppercase;
  }
  .lr-fire:hover:not(:disabled) { border-color: var(--v-sel-line); }

  .lr-q {
    width: 100%; min-width: 0; box-sizing: border-box; height: 26px; padding: 0 8px;
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
  .lr-body::-webkit-scrollbar-thumb { background: var(--v-surf3); border-radius: var(--v-r-round); }

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
  /* A hit is two lines — the verse, then why it is here — so it stops being a
     single centred row. */
  .lr-hit { align-items: flex-start; }
  .lr-hit .lr-i { line-height: 1.5; }
  .lr-hit .lr-n { display: flex; flex-direction: column; gap: 1px; }
  .lr-ref { min-width: 0; overflow: hidden; text-overflow: ellipsis; }
  /* --v-dim, not --v-faint: this line sits on --v-surf3 on hover, and muted text
     on that pairing is below AA (tokencontrast.test.js fails the build for it). */
  .lr-why {
    min-width: 0; overflow: hidden; text-overflow: ellipsis;
    font-family: var(--f-mono); font-size: var(--v-fs-cap); color: var(--v-dim);
  }
  /* Cyan is the colour law's "this is a guess" (CLAUDE.md rule 18). Never amber
     — that means ON AIR — and never amethyst, which means rehearsal. */
  .lr-why.guess { color: var(--v-cyan); }
  /* --v-dim, not --v-faint: this count sits on --v-surf3 on hover, and muted text
     on that pairing is below AA (tokencontrast.test.js fails the build for it). */
  .lr-k {
    flex: 0 0 auto; font-size: var(--v-fs-cap); letter-spacing: var(--v-tr-caps);
    text-transform: uppercase; color: var(--v-dim);
  }

  /* ── THE CHAPTER PICKER ────────────────────────────────────────────────────
     A FIXED GRID, NOT A WRAP. These were a `flex-wrap` of chips sized by their
     own labels — `min-width:22px` plus `padding:0 5px` — so a one-digit chapter
     and a two-digit one drew different boxes, every row held a different count,
     and no column lined up with the one above it. Reading 119 out of 150 meant
     reading every chip on the way. `auto-fill` + `1fr` gives every cell the same
     width at any rail width, so the rows line up and a chapter can be found by
     counting columns.

     A CHAPTER CHIP IS NOT A BUTTON-IN-A-ROW, which is why it is not `.r-btn`:
     it is a fixed square cell in a numeric picker, sized by the grid rather than
     by its label, and `.r-btn`'s 11px of side padding is the exact property that
     made these ragged. It draws `.r-btn`'s rest and hover fills so it still
     belongs to the same family. */
  .lr-chapters { padding: 2px 8px 8px; }
  .lr-chapcap { padding: 2px 0 4px; }
  .lr-chips {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(26px, 1fr));
    gap: 3px;
    /* BOUNDED, so a long book does not bury the list it came from. Psalms has
       150 chapters — 25 rows at this width — and inline in the rail's one
       scroller that pushed the 47 books after it clean off the column. The
       picker scrolls itself; the book list stays where the operator left it. */
    max-height: 168px; overflow-y: auto;
    scrollbar-width: thin; scrollbar-color: var(--v-surf3) transparent;
  }
  .lr-chips::-webkit-scrollbar { width: 6px; }
  .lr-chips::-webkit-scrollbar-thumb { background: var(--v-surf3); border-radius: var(--v-r-round); }
  .lr-chip {
    display: grid; place-items: center;
    width: 100%; height: 24px; padding: 0;
    border-radius: var(--v-r-sm); cursor: pointer;
    background: var(--v-surf2); border: 1px solid var(--v-500); color: var(--v-dim);
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
    /* Tabular figures, or 1 and 11 sit at different optical centres inside cells
       that are finally the same size. */
    font-variant-numeric: tabular-nums;
  }
  .lr-chip:hover:not(:disabled) { background: var(--v-surf3); border-color: var(--v-sel-line); color: var(--v-txt); }
  .lr-chip:disabled { opacity: .5; cursor: not-allowed; }
  /* WHERE YOU WERE. Steel — the colour of the thing being worked on. NEVER
     amber, which means ON AIR, and never cyan, which means the AI guessed: this
     marks a chapter this rail opened into the grid, which is not a claim about
     any screen. */
  .lr-chip[aria-current='true'] {
    background: var(--v-sel-soft); border-color: var(--v-sel-line);
    color: var(--v-txt); font-weight: 600;
  }
</style>
