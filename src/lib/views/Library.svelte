<script>
  import { onMount } from 'svelte';
  // Library — the unified content catalog. Every content type lives behind a
  // sub-tab: Scripture (verses the operator saved), Lyrics (songs), Media
  // (images/video/documents), Announcements, and service History. One Import
  // button ingests ANY file and auto-routes it by type; one New menu creates
  // content by hand. Import/New are constant across the whole library.
  import { tick } from 'svelte';
  import Lyrics from './library/Lyrics.svelte';
  import Scripture from './library/Scripture.svelte';
  import Browse from './library/Browse.svelte';
  import LyricSlides from './library/LyricSlides.svelte';
  import ContentSlides from './library/ContentSlides.svelte';
  import LiveStrip from './library/LiveStrip.svelte';
  import { listActiveTemplates } from '../stores/capture.js';
  import Media from './library/Media.svelte';
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
    { key: 'browse', label: 'Bible', color: 'var(--v-accent)' },
    { key: 'scripture', label: 'Saved', color: 'var(--v-accent)' },
    { key: 'lyrics', label: 'Lyrics', color: 'var(--v-accent)' },
    { key: 'media', label: 'Media', color: 'var(--v-amethyst)' },
    { key: 'announcements', label: 'Announcements', color: 'var(--v-rose)' },
  ];
  let active = 'browse';
  // The template the OUTPUT actually uses, so the live strip is the real thing.
  let liveTemplate = null;
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
  onMount(async () => {
    liveTemplate = (await listActiveTemplates().catch(() => []))[0] ?? null;
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
      importMsg = String(err);
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
  <div class="lib-topline">
    <!-- WHAT IS ON THE WALL, from inside the Library. The console's program
         monitor lives on the Live tab; an operator browsing the Bible mid-service
         should not have to leave what they are doing to answer the one question
         that matters most. -->
    <LiveStrip template={liveTemplate} />

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

    <div class="subtabs">
      {#each tabs as t}
        <button class="subtab r-focus" class:on={active === t.key} on:click={() => (active = t.key)}>
          <span class="c" style="background:{t.color};"></span>{t.label}
        </button>
      {/each}
    </div>

    <div class="lib-topactions">
      <button class="r-btn ghost sm" on:click={() => fileInput.click()} disabled={!$capture.available || importing}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 3v12M8 11l4 4 4-4M4 17v2a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-2"/></svg>
        {importing ? 'Importing…' : 'Import'}
      </button>
      <div class="lib-newwrap">
        <button class="r-btn primary sm" on:click={() => (showNew = !showNew)}>＋ New</button>
        {#if showNew}
          <button class="lib-newscrim" tabindex="-1" aria-label="Close menu" on:click={() => (showNew = false)}></button>
          <div class="lib-newmenu">
            <button class="lib-newitem" on:click={newPasteSong}>Paste / draft song</button>
            <button class="lib-newitem" on:click={newSaveScripture}>Save scripture</button>
            <button class="lib-newitem" on:click={newDraftAnnouncement}>Draft announcement</button>
          </div>
        {/if}
      </div>
      <input type="file" multiple accept=".pro,.pro6,.proplaylist,.txt,.md,.png,.jpg,.jpeg,.gif,.webp,.mp4,.mov,.webm,.pdf,.pptx,.ppt" bind:this={fileInput} on:change={onFiles} style="display:none" />
    </div>
  </div>

  {#if importMsg}<div class="lib-importmsg r-mono">{importMsg}</div>{/if}

  {#key active + '-' + reload}
    {#if active === 'browse'}
      <Browse query={debounced} />
    {:else if active === 'scripture'}
      <Scripture startSave={scriptureAction} />
    {:else if active === 'lyrics'}
      <LyricSlides query={debounced} />
    {:else if active === 'media'}
      <ContentSlides kind="media" query={debounced} />
    {:else}
      <ContentSlides kind="announce" query={debounced} />
    {/if}
  {/key}
{/if}
</div>

<style>
  .lib-search{ position:relative; display:flex; align-items:center; margin-bottom:12px; }
  .lib-search svg{ position:absolute; left:12px; color:var(--v-faint); pointer-events:none; }
  .lib-search input{ padding-left:34px; max-width:520px; }

  .lib-shell{ display:flex; flex-direction:column; gap:16px; }
  .lib-topline{ display:flex; align-items:center; justify-content:space-between; gap:16px; flex-wrap:wrap; }
  .lib-topactions{ display:flex; gap:8px; flex-shrink:0; align-items:center; }
  .lib-importmsg{ font-size:11.5px; color:var(--v-emerald); margin-top:-6px; }

  .lib-newwrap{ position:relative; }
  .lib-newscrim{ position:fixed; inset:0; z-index:40; background:transparent; border:0; cursor:default; }
  .lib-newmenu{ position:absolute; right:0; top:calc(100% + 6px); z-index:50; min-width:180px; padding:6px;
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:11px; box-shadow:0 18px 44px -18px #000;
    display:flex; flex-direction:column; gap:2px; }
  .lib-newitem{ text-align:left; padding:9px 11px; border-radius:8px; border:0; background:transparent; color:var(--v-txt);
    font-family:var(--f-body); font-size:13px; cursor:pointer; }
  .lib-newitem:hover{ background:var(--v-surf3); color:var(--v-accent); }
</style>
