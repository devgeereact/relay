<script>
  // LYRICS, as slides.
  //
  // Same structure as the Bible browser, for the same reason: an operator picks
  // the next slide by LOOKING at it. A song is already stored as sections
  // (verse 1, chorus, bridge…), which is exactly a deck — the Library just never
  // drew it as one.
  //
  // Songs on the left, that song's sections as rendered slides on the right,
  // clicking a section puts it on the wall. The section currently on screen
  // wears the amber ring, so an operator mid-chorus can see where they are
  // without looking away.

  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import SlideGrid from './SlideGrid.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import { humanError } from '../../errors.js';
  import {
    listSongs,
    searchSongs,
    getSong,
    fireContent,
    listActiveTemplates,
    getContentTemplates,
    loadTemplates,
    templates,
  } from '../../stores/capture.js';

  /** Search text from the Library's one search bar. */
  export let query = '';

  let songs = [];
  let song = null;
  let sections = [];
  let template = null;
  let loading = true;
  let loadingSong = false;
  let error = '';
  let firing = '';

  onMount(async () => {
    try {
      const [list, tpls, ct] = await Promise.all([
        listSongs(),
        listActiveTemplates().catch(() => []),
        getContentTemplates().catch(() => ({})),
      ]);
      songs = list ?? [];
      // THE SONG TEMPLATE, not the scripture one. Relay already models a
      // per-content-type template; rendering lyrics through the scripture
      // template put a huge gold REFERENCE where the lyric should be and shrank
      // the words the congregation is meant to sing.
      await loadTemplates().catch(() => {});
      const all = get(templates) ?? [];
      template = all.find((t) => t.id === ct?.song) ?? tpls[0] ?? null;
      if (songs.length) await open(songs[0]);
    } catch (e) {
      error = humanError(e);
    }
    loading = false;
  });

  // The Library's search box drives the song list. Debounced by the caller.
  let lastQuery = null;
  $: if (query !== lastQuery) {
    lastQuery = query;
    refresh(query);
  }

  async function refresh(q) {
    try {
      songs = (q?.trim() ? await searchSongs(q.trim()) : await listSongs()) ?? [];
      if (songs.length && !songs.some((s) => s.id === song?.id)) await open(songs[0]);
      if (!songs.length) {
        song = null;
        sections = [];
      }
    } catch (e) {
      error = humanError(e);
    }
  }

  async function open(s) {
    song = s;
    loadingSong = true;
    error = '';
    try {
      const full = await getSong(s.id);
      sections = full?.sections ?? [];
    } catch (e) {
      error = humanError(e);
      sections = [];
    }
    loadingSong = false;
  }

  async function fire(it) {
    firing = it.key;
    error = '';
    try {
      // `fire_content` with kind 'song' so the router and the templates treat it
      // as lyrics, not scripture. Never hand-build the payload — pipeline::Fire
      // is the one place content becomes screen content (CLAUDE.md).
      await fireContent(it.reference, it.text, 'song');
    } catch (e) {
      error = humanError(e);
    }
    firing = '';
  }

  // A section's label is what an operator calls it out loud — "chorus", not "#3".
  $: slides = sections.map((sec, i) => ({
    key: `${song?.id}-${sec.id ?? i}`,
    // IDENTITY carries the song, so two songs' choruses never collide when the
    // grid works out which slide is on the wall.
    reference: `${song?.title ?? 'Song'} · ${sec.label || sec.tag || `Section ${i + 1}`}`,
    // DISPLAY is just the section — the song is already selected in the rail.
    label: sec.label || sec.tag || `Section ${i + 1}`,
    text: sec.lyrics ?? '',
    translation: null,
    // A lyric slide projects the LYRIC. The congregation is not singing the title.
    hideReference: true,
  }));
</script>

{#if loading}
  <Loading what="songs" />
{:else if !songs.length}
  <EmptyState
    message={query?.trim()
      ? `No songs matching “${query.trim()}”.`
      : 'No songs yet — import or paste one with the Import button.'} />
{:else}
  <div class="ly">
    <nav class="ly-list r-scroll" aria-label="Songs">
      {#each songs as s}
        <button class="ly-song" class:on={song?.id === s.id} on:click={() => open(s)}>
          <b>{s.title}</b>
          {#if s.author}<span>{s.author}</span>{/if}
        </button>
      {/each}
    </nav>

    <div class="ly-slides r-scroll">
      {#if loadingSong}
        <Loading what="the song" />
      {:else if slides.length}
        <SlideGrid items={slides} {template} onFire={fire} busyKey={firing} />
      {:else}
        <EmptyState message="This song has no sections yet." />
      {/if}
    </div>
  </div>
{/if}

{#if error}<p class="ly-err">{error}</p>{/if}

<style>
  .ly {
    display: grid;
    grid-template-columns: 232px minmax(0, 1fr);
    gap: 12px;
    height: clamp(380px, 60vh, 680px);
    min-height: 0;
  }
  .ly-list,
  .ly-slides {
    overflow-y: auto;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    padding: 8px;
  }
  .ly-slides {
    padding: 12px;
  }
  .ly-song {
    display: block;
    width: 100%;
    padding: 9px 11px;
    border-radius: var(--v-r-md);
    background: none;
    border: 0;
    color: var(--v-dim);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .ly-song:hover {
    background: var(--v-surf2);
    color: var(--v-txt);
  }
  /* Selection is chrome — the accent, never amber (DECISIONS §22). */
  .ly-song.on {
    background: var(--v-accent-soft);
    color: var(--v-accent2);
  }
  .ly-song b {
    display: block;
    font-size: 13px;
    font-weight: 600;
  }
  .ly-song span {
    display: block;
    margin-top: 2px;
    font-size: 11px;
    color: var(--v-faint);
  }
  .ly-err {
    margin: 10px 0 0;
    font-size: var(--v-fs-b2);
    color: var(--v-red);
  }

  @media (max-width: 860px) {
    .ly {
      grid-template-columns: 1fr;
      height: auto;
    }
  }
</style>
