<script>
  // IMPORTING A WHOLE PROPRESENTER LIBRARY — the screen, and the report.
  //
  // ── WHY THIS IS NOT `ImportReview.svelte` ─────────────────────────────────
  //
  // `ImportReview` is the right product for two or three files: it builds an
  // editable copy of every song and every slide and lets the operator fix a title
  // or a slide break BEFORE anything reaches the library, so there is no
  // import-then-fix-then-replace cycle.
  //
  // At 726 files it is not a smaller version of that; it is a different product.
  // It would hold 726 songs and roughly 14,000 slides in memory as editable state,
  // render 726 accordion rows, and ask one volunteer to expand and read each one.
  // Nobody does that. And a review nobody performs is WORSE than no review,
  // because the library then contains content the product believes was checked.
  //
  // So the trade made here is deliberate and is stated on the screen:
  //
  //   review-BEFORE-save (a handful)  →  commit, then a report that NAMES THE
  //                                      FILES A HUMAN STILL HAS TO LOOK AT.
  //
  // DECISIONS §105.
  //
  // That is only an honest trade because the commit is reversible in the place the
  // operator already works: a song can be edited or deleted in the Library, and
  // re-importing the same file replaces it by title. What is NOT recoverable is a
  // partial import nobody can identify afterwards — which is exactly what the
  // "came in thin" list below exists to prevent. About forty of the 726 files in
  // the measured export carry a single RTF block, and once they are in the library
  // a one-line chorus and a file that barely parsed look identical.
  //
  // ── WHAT THIS PANEL DELIBERATELY IS NOT ───────────────────────────────────
  //
  // Not a dialog, not a menu, not a listbox. Mounting any of those makes
  // `shortcuts.js` stand down and takes the operator's Escape and their
  // `Clear screens` (CLAUDE.md rule 44) — for twenty minutes, over a shell that
  // may have a live microphone in it. It is an ordinary region inside the Library
  // workspace, so the dock and the panic keys are untouched the whole way through.
  // Pinned by `bulkimportui.test.js`.
  import { onMount, createEventDispatcher } from 'svelte';
  import { fileToBase64, parseImport, saveReviewedSongs } from '../../stores/capture.js';
  import { runBulkImport, describeRun } from '../../bulkimport.js';
  import { humanError } from '../../errors.js';

  /** The lyric files the operator picked. */
  export let files = [];

  const dispatch = createEventDispatcher();

  /** How many rows of a list are shown before the operator asks for the rest. */
  const CAP = 50;

  let progress = {
    filesTotal: files.length,
    filesDone: 0,
    current: '',
    songsParsed: 0,
    songsSaved: 0,
  };
  let report = null;
  let stopped = false;
  let openAll = { thin: false, unreadable: false, duplicates: false };

  $: pct = progress.filesTotal ? Math.round((progress.filesDone / progress.filesTotal) * 100) : 0;

  onMount(async () => {
    try {
      report = await runBulkImport(
        files,
        { read: fileToBase64, parse: parseImport, save: saveReviewedSongs },
        {
          onProgress: (p) => (progress = p),
          // Read live, so pressing Stop takes effect at the next file boundary
          // rather than at some point the operator cannot predict.
          shouldCancel: () => stopped,
        },
      );
    } catch (e) {
      // `runBulkImport` returns its failures rather than throwing, so this is the
      // unforeseen one. A panel with no way out is worse than any message.
      report = {
        ...progress,
        added: [],
        replaced: [],
        thin: [],
        unreadable: [],
        duplicates: [],
        cancelled: false,
        finished: false,
        error: humanError(e),
      };
    }
  });

  const shown = (list, key) => (openAll[key] ? list : list.slice(0, CAP));
</script>

<section class="bi" aria-label="Importing songs">
  <div class="bi-top">
    <div class="bi-title">{report ? 'Import report' : 'Importing songs'}</div>
    <span class="bi-sub r-mono">
      {#if report}
        {describeRun(report)}
      {:else}
        {progress.filesDone} of {progress.filesTotal} files · {progress.songsSaved} saved so far
      {/if}
    </span>
    <span class="bi-spring"></span>
    {#if report}
      <button class="r-btn primary sm" on:click={() => dispatch('done', report)}>Done</button>
    {:else}
      <button class="r-btn ghost sm" on:click={() => (stopped = true)} disabled={stopped}>
        {stopped ? 'Stopping…' : 'Stop import'}
      </button>
    {/if}
  </div>

  {#if !report}
    <!-- The bar is the cheap half. The FILENAME under it is the half that tells an
         operator whether anything is happening at all, and which file to blame if
         it stops. -->
    <div
      class="bi-bar"
      role="progressbar"
      aria-label="Import progress"
      aria-valuemin="0"
      aria-valuemax={progress.filesTotal}
      aria-valuenow={progress.filesDone}>
      <div class="bi-fill" style="width:{pct}%"></div>
    </div>
    <div class="bi-now r-mono">Reading {progress.current || '…'}</div>
    <p class="bi-note">
      Relay is reading one file at a time and saving them in batches. Stopping keeps
      everything already saved — it only stops the files it has not read yet.
    </p>
  {:else}
    {#if report.error}
      <div class="bi-err" role="alert">{report.error}</div>
    {/if}

    {#if report.thin.length}
      <div class="bi-block warn">
        <div class="bi-blockhead">
          {report.thin.length} came in thin — open these and check
        </div>
        <p class="bi-blocknote">
          One slide each. That is either a one-line chorus, which is fine, or a file
          Relay could barely read, which is not — and once they are in the Library
          those two look exactly the same. Nothing else can tell you which is which.
        </p>
        <ul class="bi-list r-mono">
          {#each shown(report.thin, 'thin') as t}
            <li>{t.file} · {t.title} · {t.slides} slide{t.slides === 1 ? '' : 's'}</li>
          {/each}
        </ul>
        {#if report.thin.length > CAP && !openAll.thin}
          <button class="r-btn ghost sm" on:click={() => (openAll = { ...openAll, thin: true })}>
            Show all {report.thin.length}
          </button>
        {/if}
      </div>
    {/if}

    {#if report.unreadable.length}
      <div class="bi-block">
        <div class="bi-blockhead">{report.unreadable.length} with nothing Relay could read</div>
        <p class="bi-blocknote">
          These were skipped and the rest of the import carried on. Nothing was saved
          for them.
        </p>
        <ul class="bi-list r-mono">
          {#each shown(report.unreadable, 'unreadable') as u}
            <li>{u.file} · {u.message}</li>
          {/each}
        </ul>
        {#if report.unreadable.length > CAP && !openAll.unreadable}
          <button
            class="r-btn ghost sm"
            on:click={() => (openAll = { ...openAll, unreadable: true })}>
            Show all {report.unreadable.length}
          </button>
        {/if}
      </div>
    {/if}

    {#if report.duplicates.length}
      <div class="bi-block">
        <div class="bi-blockhead">
          {report.duplicates.length} title{report.duplicates.length === 1 ? '' : 's'} arrived from
          more than one file
        </div>
        <p class="bi-blocknote">
          Relay keeps one song per title, so for each of these the LAST file read is
          what is in the Library now.
        </p>
        <ul class="bi-list r-mono">
          {#each shown(report.duplicates, 'duplicates') as d}
            <li>{d.title} · {d.files.join(', ')}</li>
          {/each}
        </ul>
        {#if report.duplicates.length > CAP && !openAll.duplicates}
          <button
            class="r-btn ghost sm"
            on:click={() => (openAll = { ...openAll, duplicates: true })}>
            Show all {report.duplicates.length}
          </button>
        {/if}
      </div>
    {/if}

    <!-- SAY THE LIMIT OUT LOUD. `proimport.rs` takes the title from the file stem
         and every slide is named `Slide N`, because ProPresenter's section names are
         not in the text Relay can read out of a `.pro` container. An operator who
         opens a song to a list of "Slide 1 … Slide 9" and has not been told this
         reads it as a bug in the import, and the honest answer is that it is a limit
         of what was imported. -->
    <p class="bi-note">
      Titles came from the file names, and slides are numbered — <span class="r-mono">Slide 1</span>,
      <span class="r-mono">Slide 2</span> and so on. ProPresenter's own section names
      (Verse, Chorus) are not in the text Relay can read, so <strong>no arrangements
      were imported</strong>. Build those in the Library when you need them.
    </p>
  {/if}
</section>

<style>
  /* Tokens only — `views/workspacegrammar.test.js` §1/§11 hold the desk to the
     one type scale and the one palette, and a new literal on any view fails it.
     Deliberately NO amber anywhere: amber means a congregation is looking at
     something (CLAUDE.md rule 18), and a Library report is not that. */
  .bi{ display:flex; flex-direction:column; gap:14px; }
  .bi-top{ display:flex; align-items:center; gap:12px; flex-wrap:wrap; }
  .bi-title{ font-family:var(--f-head); font-size:var(--v-fs-ttl); font-weight:700; color:var(--v-txt); }
  .bi-sub{ font-size:var(--v-fs-b3); color:var(--v-faint); }
  .bi-spring{ flex:1; }

  .bi-bar{ height:6px; border-radius:var(--v-r-sm); background:var(--v-surf2); overflow:hidden; }
  .bi-fill{ height:100%; background:var(--v-accent); transition:width .2s linear; }
  .bi-now{ font-size:var(--v-fs-b3); color:var(--v-faint); }

  .bi-note{ margin:0; font-size:var(--v-fs-b1); line-height:var(--v-lh-pr); color:var(--v-dim); }
  .bi-err{ padding:9px 11px; border-radius:var(--v-r-lg); font-size:var(--v-fs-b1); line-height:var(--v-lh-pr);
    color:var(--v-txt); background:var(--v-rose-soft); border:1px solid var(--v-red-line); }

  .bi-block{ display:flex; flex-direction:column; gap:7px; align-items:flex-start;
    padding:11px 13px; border-radius:var(--v-r-lg); border:1px solid var(--v-line); background:var(--v-surf); }
  /* The one that needs a person. A brighter hairline, not a colour — the words
     carry the urgency and the palette keeps its meanings. */
  .bi-block.warn{ border-color:var(--v-line2); background:var(--v-surf2); }
  .bi-blockhead{ font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .bi-blocknote{ margin:0; font-size:var(--v-fs-b1); line-height:var(--v-lh-pr); color:var(--v-dim); }
  .bi-list{ margin:0; padding-left:18px; font-size:var(--v-fs-b3); line-height:1.7;
    color:var(--v-faint); max-height:260px; overflow:auto; }
</style>
