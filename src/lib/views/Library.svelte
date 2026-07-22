<script>
  import { onMount } from 'svelte';
  import { setSession } from '../session.js';
  import { humanError } from '../errors.js';
  // Library — the unified content catalog. Every content type lives behind a
  // sub-tab: Scripture (verses the operator saved), Lyrics (songs), Media
  // (images/video/documents), Announcements, and service History. One Import
  // button ingests ANY file and auto-routes it by type; one New menu creates
  // content by hand. Import/New are constant across the whole library.
  import { tick } from 'svelte';
  import Scripture from './library/Scripture.svelte';
  import Browse from './library/Browse.svelte';
  import LyricsPane from './library/LyricsPane.svelte';
  import LiveOutputRail from './library/LiveOutputRail.svelte';
  import {
    listActiveTemplates,
    loadTemplates,
    templates,
    confirmDetection,
    manualFire,
    fireMedia,
    listBooks,
    listTranslations,
    getActiveTranslation,
    setActiveTranslation,
  } from '../stores/capture.js';
  import MediaLibrary from './library/MediaLibrary.svelte';
  import Announcements from './library/Announcements.svelte';
  import ImportReview from './library/ImportReview.svelte';
  import {
    capture,
    parseImport,
    importMedia,
    fileToBase64,
  } from '../stores/capture.js';

  const tabs = [
    // BROWSE is first: the Library could search, and could list what had been
    // saved, but could not open a Bible and read it — which is the thing the
    // word "library" promises.
    { key: 'browse', label: 'Bible' },
    { key: 'scripture', label: 'Saved' },
    { key: 'lyrics', label: 'Lyrics' },
    { key: 'media', label: 'Media' },
    { key: 'announcements', label: 'Announcements' },
    // GRAPHICS is the reference's sixth pill. It is not a new store: it is the
    // image half of Media. ProPresenter draws the same line — a still you put
    // BEHIND words is a different job from a video you play, and mixing them
    // means hunting past twenty MP4s for a logo. Both read the same table, so
    // nothing is duplicated and nothing is invented.
    { key: 'graphics', label: 'Graphics' },
  ];
  let active = 'browse';
  // The template the OUTPUT actually uses, so the live strip is the real thing.
  let liveTemplate = null;
  // ── THE LIVE COLUMN ───────────────────────────────────────────────────────
  //
  // ONE shape for every content type: the list on the left, and on the right
  // what is coming, what the congregation can see, and what the AI heard. The
  // operator's job is identical whether the thing is a verse, a song, a picture
  // or a notice, and four different browsing metaphors would be four things to
  // learn under pressure.
  //
  // PREVIEW holds AI SUGGESTIONS ONLY. Browsing FIRES (see Browse.svelte), so
  // staging a slide the operator already chose would only mirror Program and
  // make the pair meaningless — the point of two monitors is that they differ.
  let staged = null;
  let taking = false;
  // A FAILURE surface, humanised and rose — kept separate from the green
  // importMsg so a broken fire or import can never be shown in success colour.
  let errMsg = '';
  const select = () => {};
  function stage(d) {
    staged = {
      key: d.reference,
      reference: d.reference,
      text: d.text,
      translation: d.translation,
      _fire: () => confirmDetection(d.reference),
    };
  }
  /** Fire something the operator queued. Same manual_fire as every other path.
      This is a LIVE-FIRE path — a swallowed rejection is a Fire button that does
      nothing to the wall and says nothing about why, so it must surface. */
  async function fireQueued(item) {
    errMsg = '';
    try {
      if (item.mediaId) await fireMedia(item.mediaId);
      else await manualFire(item.reference);
    } catch (e) {
      errMsg = humanError(e);
    }
  }

  async function take() {
    if (!staged?._fire) return;
    errMsg = '';
    taking = true;
    try {
      await staged._fire();
      staged = null;
    } catch (e) {
      // Do NOT clear `staged` — the operator's chosen slide stays put so they can
      // retry, rather than vanishing as if it fired.
      errMsg = humanError(e);
    } finally {
      taking = false;
    }
  }
  // ONE search box for the whole Library. Each pane decides what the words mean
  // for its own content — a reference or a phrase in scripture, a title or a
  // line in a song — but the operator only has to find one box.
  let query = '';
  let debounced = '';
  let searchTimer;
  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => (debounced = query), 200);
  }
  $: placeholder =
    active === 'browse' || active === 'scripture'
      ? 'Search scripture — a reference or a phrase'
      : active === 'lyrics'
        ? 'Search songs — title, author or a line'
        : active === 'media'
          ? 'Search media by filename'
          : 'Search announcements';
  // ── THE FILTER BAR ────────────────────────────────────────────────────────
  // The reference puts translation · search · book · chapter on one row above
  // the panes. Book and chapter are the Bible pane's real navigation, so the
  // state is held HERE and bound down rather than duplicated: a select and a
  // book row in the tree that disagree about which chapter is open is worse
  // than either control on its own.
  let books = [];
  let translations = [];
  let activeTranslation = null;
  let book = null;
  let chapter = 1;
  // The verse PICKER — where to look, not what to fire. Bound to the Bible pane
  // in both directions: it offers the verses the open chapter actually has.
  let verse = null;
  let verseCount = 0;
  /** The reference's "Filters" control. Favourites is the one real filter the
      Bible pane has — every other axis in the mockup (type, book, chapter) is
      already a control on this bar. */
  let favouritesOnly = false;
  let showMore = false;
  // THE QUEUE lives here, beside the rail that renders it, so it survives a
  // sub-tab change — an operator queueing verses does not expect them dropped
  // because they looked at the songs.
  let queue = [];

  $: chapterCount = books.find((b) => b.book === book)?.chapters ?? 0;
  // A verse number means nothing once the chapter under it changed.
  let lastPlace = '';
  $: if (`${book}|${chapter}` !== lastPlace) {
    lastPlace = `${book}|${chapter}`;
    verse = null;
  }

  async function pickTranslation(id) {
    const prev = activeTranslation;
    activeTranslation = id;
    errMsg = '';
    try {
      await setActiveTranslation(id);
      books = await listBooks();
    } catch (e) {
      activeTranslation = prev; // don't leave the wrong translation active + stale books
      errMsg = humanError(e);
    }
  }

  onMount(async () => {
    liveTemplate = (await listActiveTemplates().catch(() => []))[0] ?? null;
    await loadTemplates().catch(() => {});
    // Guarded: an unguarded reject here aborts the rest of mount, leaving the
    // book/translation selects empty with no reason shown — the Bible pane
    // silently never populates.
    try {
      const [b, t, a] = await Promise.all([
        listBooks(),
        listTranslations(),
        getActiveTranslation(),
      ]);
      books = b;
      translations = t;
      activeTranslation = a;
      // Open on the first book rather than an empty pane: a Bible that opens
      // closed makes the operator do a click that has exactly one answer.
      if (!book && books.length) book = books[0].book;
    } catch (e) {
      errMsg = humanError(e);
    }
  });
  let reload = 0; // bump to remount the active pane after an import
  let fileInput;
  let importing = false;
  let importMsg = '';
  let showNew = false;

  // pre-save review of parsed lyric files
  let reviewSongs = [];
  let reviewing = false;

  // pane actions passed on (re)mount
  let lyricAction = null; // 'paste' when New → paste/draft song
  let scriptureAction = false; // true when New → save scripture
  let announceAction = false; // true when New → draft announcement

  function goTab(t) {
    active = t;
    reload += 1;
  }

  // File-type routing — the heart of "import anything, sorted automatically".
  const IMG = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'avif', 'svg'];
  const VID = ['mp4', 'mov', 'webm', 'mkv', 'm4v'];
  const DOC = ['pdf', 'pptx', 'ppt', 'key'];
  const TXT = ['txt', 'text', 'md', 'lyric', 'lyrics'];
  const PRO = ['pro', 'pro6', 'pro5', 'proplaylist'];
  // The file picker and the router are ONE list. They were two, and the picker's
  // was shorter — .bmp, .avif, .svg, .mkv, .m4v, .pro5 and .key were greyed out
  // in the dialog even though the importer handles them, so choosing one was
  // impossible and nothing said why.
  const ACCEPT = [...PRO, ...TXT, ...IMG, ...VID, ...DOC].map((e) => `.${e}`).join(',');

  async function onFiles(e) {
    const files = Array.from(e.target.files || []);
    if (!files.length) return;
    importing = true;
    importMsg = '';
    const parsed = []; // lyric songs → pre-save review
    let media = 0;
    try {
      for (const file of files) {
        const ext = (file.name.split('.').pop() || '').toLowerCase();
        if (PRO.includes(ext) || TXT.includes(ext)) {
          const got = await parseImport(file.name, await fileToBase64(file));
          parsed.push(...got);
        } else if (IMG.includes(ext)) {
          await importMedia('image', file.name, await fileToBase64(file));
          media += 1;
        } else if (VID.includes(ext)) {
          await importMedia('video', file.name, await fileToBase64(file));
          media += 1;
        } else if (DOC.includes(ext)) {
          await importMedia('document', file.name, await fileToBase64(file));
          media += 1;
        } else {
          importMsg = `Skipped .${ext} (unsupported)`;
        }
      }
      if (parsed.length) {
        // Lyrics go through the pre-save review (edit before committing).
        reviewSongs = parsed;
        reviewing = true;
      } else if (media) {
        importMsg = `Imported ${media} to Media.`;
        goTab('media');
      }
    } catch (err) {
      errMsg = humanError(err);
    }
    importing = false;
    e.target.value = '';
  }

  function onReviewDone(ev) {
    reviewing = false;
    const res = ev.detail || {};
    const a = res.added?.length || 0;
    const r = res.replaced?.length || 0;
    const parts = [];
    if (a) parts.push(`${a} new`);
    if (r) parts.push(`${r} replaced`);
    importMsg = parts.length ? `Saved — ${parts.join(', ')}.` : 'Nothing saved.';
    goTab('lyrics');
  }

  async function newPasteSong() {
    showNew = false;
    lyricAction = 'paste';
    goTab('lyrics');
    await tick();
    lyricAction = null;
  }
  async function newSaveScripture() {
    showNew = false;
    scriptureAction = true;
    goTab('scripture');
    await tick();
    scriptureAction = false;
  }
  async function newDraftAnnouncement() {
    showNew = false;
    announceAction = true;
    goTab('announcements');
    await tick();
    announceAction = false;
  }
</script>

<div class="lib-shell">
{#if reviewing}
  <ImportReview songs={reviewSongs} on:done={onReviewDone} on:cancel={() => (reviewing = false)} />
{:else}
  <!-- ROW 1 — the content type, and the two things you can do to the library
       as a whole. Constant across every pane. -->
  <div class="lib-topline">
    <div class="subtabs" role="tablist" aria-label="Content type">
      {#each tabs as t}
        <button
          class="r-pill r-focus"
          role="tab"
          aria-selected={active === t.key}
          class:on={active === t.key}
          on:click={() => (active = t.key)}>{t.label}</button>
      {/each}
    </div>

    <span class="lib-spring"></span>

    <div class="lib-topactions">
      <button class="r-btn ghost sm" on:click={() => fileInput.click()} disabled={!$capture.available || importing}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12M8 11l4 4 4-4M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"/></svg>
        {importing ? 'Importing…' : 'Import'}
      </button>
      <div class="lib-newwrap">
        <button class="r-btn primary sm" aria-haspopup="menu" aria-expanded={showNew} on:click={() => (showNew = !showNew)}>
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
          New Item
        </button>
        {#if showNew}
          <button class="lib-newscrim" tabindex="-1" aria-label="Close menu" on:click={() => (showNew = false)}></button>
          <div class="lib-newmenu" role="menu">
            <button class="lib-newitem" on:click={newPasteSong}>Paste / draft song</button>
            <button class="lib-newitem" on:click={newSaveScripture}>Save scripture</button>
            <button class="lib-newitem" on:click={newDraftAnnouncement}>Draft announcement</button>
          </div>
        {/if}
      </div>
      <div class="lib-newwrap">
        <button class="r-iconbtn lib-more" aria-label="More actions" aria-haspopup="menu" aria-expanded={showMore} on:click={() => (showMore = !showMore)}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="5" cy="12" r="1.7" /><circle cx="12" cy="12" r="1.7" /><circle cx="19" cy="12" r="1.7" /></svg>
        </button>
        {#if showMore}
          <button class="lib-newscrim" tabindex="-1" aria-label="Close menu" on:click={() => (showMore = false)}></button>
          <div class="lib-newmenu" role="menu">
            <button class="lib-newitem" on:click={() => { showMore = false; goTab(active); }}>Reload this list</button>
            <button class="lib-newitem" on:click={() => { showMore = false; setSession({ activeTab: 'settings' }); }}>
              Data health…
            </button>
          </div>
        {/if}
      </div>
      <input type="file" multiple accept={ACCEPT} bind:this={fileInput} on:change={onFiles} style="display:none" />
    </div>
  </div>

  <!-- ROW 2 — where you are (translation · book · chapter) and what you are
       looking for. One search box for the whole library. -->
  <div class="lib-filters">
    {#if active === 'browse' || active === 'scripture'}
      <select
        class="r-select lib-tr"
        aria-label="Translation"
        disabled={translations.length < 2}
        value={activeTranslation}
        on:change={(e) => pickTranslation(Number(e.currentTarget.value))}>
        {#each translations as t}
          <option value={t.id}>{t.abbreviation || t.name}</option>
        {/each}
        {#if !translations.length}<option>KJV</option>{/if}
      </select>
    {/if}

    <select class="r-select lib-f" aria-label="Content type" bind:value={active}>
      {#each tabs as t}<option value={t.key}>{t.label}</option>{/each}
    </select>

    <div class="lib-search">
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor"
        stroke-width="2" stroke-linecap="round" aria-hidden="true">
        <circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" />
      </svg>
      <input
        class="r-input"
        type="search"
        bind:value={query}
        on:input={onSearch}
        {placeholder}
        aria-label={placeholder} />
    </div>

    {#if active === 'browse'}
      <select class="r-select lib-f" aria-label="Book" bind:value={book}>
        {#each books as b}<option value={b.book}>{b.book}</option>{/each}
      </select>
      <select class="r-select lib-f" aria-label="Chapter" bind:value={chapter}>
        {#each Array(chapterCount) as _, i}
          <option value={i + 1}>Chapter {i + 1}</option>
        {/each}
      </select>
      <select class="r-select lib-v" aria-label="Verse" bind:value={verse} disabled={!verseCount}>
        <option value={null}>All verses</option>
        {#each Array(verseCount) as _, i}
          <option value={i + 1}>Verse {i + 1}</option>
        {/each}
      </select>
      <!-- The mockup's "Filters" control. It toggles a filter that exists
           rather than opening a panel of options that do not. -->
      <button class="r-btn ghost lib-filter" class:on={favouritesOnly}
        aria-pressed={favouritesOnly}
        on:click={() => (favouritesOnly = !favouritesOnly)}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill={favouritesOnly ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="2" stroke-linejoin="round" aria-hidden="true"><path d="M6 3h12v18l-6-4.5L6 21z" /></svg>
        Favourites
      </button>
    {/if}
  </div>

  {#if errMsg}<div class="lib-importerr r-mono" role="alert">{errMsg}</div>{/if}
  {#if importMsg}<div class="lib-importmsg r-mono">{importMsg}</div>{/if}



  <!-- The BIBLE pane owns its own right column (the reference's verse
       inspector); every other content type gets the shared live column. -->
  <div class="lib-body">
    <div class="lib-pane">
      {#key active + '-' + reload}
        {#if active === 'browse'}
          <Browse
            query={debounced}
            {books}
            {translations}
            bind:book
            bind:chapter
            bind:verse
            bind:verseCount
            {favouritesOnly}
            {queue}
            onQueueChange={(q) => (queue = q)}
            onSelect={select} />
        {:else if active === 'scripture'}
          <Scripture query={debounced} {queue} onQueueChange={(q) => (queue = q)} />
        {:else if active === 'lyrics'}
          <LyricsPane query={debounced} onSelect={select} {queue} onQueueChange={(q) => (queue = q)} />
        {:else if active === 'media' || active === 'graphics'}
          <MediaLibrary
            query={debounced}
            only={active === 'graphics' ? 'image' : 'moving'}
            {queue}
            onQueueChange={(q) => (queue = q)} />
        {:else}
          <Announcements query={debounced} startDraft={announceAction} {queue} onQueueChange={(q) => (queue = q)} />
        {/if}
      {/key}
    </div>

    <LiveOutputRail
      preview={staged}
      template={liveTemplate}
      {queue}
      busy={taking}
      onTake={take}
      onQueueChange={(q) => (queue = q)}
      onFireQueued={fireQueued}
      allTemplates={$templates} />
  </div>
{/if}
</div>

<style>
  .lib-shell{ display:flex; flex-direction:column; gap:14px; min-height:0; }
  /* ONE layout for every content type: the catalogue, and the live column. */
  .lib-body{ display:grid; grid-template-columns:minmax(0,1fr) 400px; gap:12px; min-height:0;
    height:clamp(420px, calc(100vh - 296px), 900px); }
  .lib-pane{ display:flex; flex-direction:column; min-height:0; }
  @media (max-width:1360px){ .lib-body{ grid-template-columns:minmax(0,1fr) 344px; } }
  @media (max-width:1140px){ .lib-body{ grid-template-columns:minmax(0,1fr) 300px; } }
  /* STACKED — one column. Two rules learned the hard way here:
     1. The DECK stays first. Putting the live rail on top read well in theory
        ("what is on the wall matters most"), but the rail leads with a 16:9
        monitor: at full width that is a 400px-tall slide, so the entire window
        became one enormous verse and the deck sat 1300px below the fold. The
        operator saw a screen with no library on it and no way to know why.
     2. The rail is WIDTH-capped, not height-capped. Capping the height of a
        box with `aspect-ratio` shrinks its WIDTH instead and leaves a dead
        strip beside it — that bug is in this log twice already. */
  @media (max-width:860px){
    .lib-body{ grid-template-columns:1fr; height:auto; }
    .lib-pane{ min-height:60vh; }
  }
  .lib-topline{ display:flex; align-items:center; gap:16px; flex-wrap:wrap; }
  .lib-spring{ flex:1; }
  .subtabs{ display:flex; gap:8px; flex-wrap:wrap; }
  .lib-topactions{ display:flex; gap:8px; flex-shrink:0; align-items:center; }
  .lib-importmsg{ font-size:11.5px; color:var(--v-emerald); margin-top:-4px; }
  /* Failures are rose — never the emerald success line above them. */
  .lib-importerr{ font-size:11.5px; color:var(--v-red); margin-top:-4px; }

  /* The filter bar. Every control is 40px so the row has one baseline. */
  .lib-filters{ display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .lib-tr{ width:120px; flex:0 0 auto; }
  .lib-f{ width:150px; flex:0 0 auto; }
  .lib-v{ width:126px; flex:0 0 auto; }
  .lib-filter{ height:40px; flex:0 0 auto; }
  .lib-filter.on{ border-color:var(--v-accent-line); color:var(--v-accent2); background:var(--v-accent-soft); }
  .lib-more{ width:40px; height:40px; }
  .lib-search{ position:relative; display:flex; align-items:center; flex:1 1 280px; min-width:220px; max-width:420px; }
  .lib-search svg{ position:absolute; left:13px; color:var(--v-faint); pointer-events:none; }
  .lib-search input{ padding-left:36px; }

  .lib-newwrap{ position:relative; }
  .lib-newscrim{ position:fixed; inset:0; z-index:40; background:transparent; border:0; cursor:default; }
  .lib-newmenu{ position:absolute; right:0; top:calc(100% + 6px); z-index:50; min-width:180px; padding:6px;
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:11px; box-shadow:0 18px 44px -18px #000;
    display:flex; flex-direction:column; gap:2px; }
  .lib-newitem{ text-align:left; padding:9px 11px; border-radius:8px; border:0; background:transparent; color:var(--v-txt);
    font-family:var(--f-body); font-size:13px; cursor:pointer; }
  .lib-newitem:hover{ background:var(--v-surf3); color:var(--v-accent); }
</style>
