<script>
  // ANNOUNCEMENTS and MEDIA, as slides.
  //
  // The same structure as scripture and lyrics, because the operator's job is
  // the same in all four cases: find the thing, look at it, put it on the wall.
  // Four different browsing metaphors for four content types would be four
  // things to learn under pressure.
  //
  // Media renders its actual picture rather than the scripture template — a
  // photo has no verse to typeset, and drawing one through a text template would
  // show an empty frame with a filename underneath.

  import { onMount } from 'svelte';
  import SlideGrid from './SlideGrid.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import { humanError } from '../../errors.js';
  import {
    listAnnouncements,
    listMedia,
    fireContent,
    fireMedia,
    listActiveTemplates,
    localIp,
  } from '../../stores/capture.js';

  /** 'announce' | 'media' */
  export let kind = 'announce';
  export let query = '';

  let rows = [];
  let template = null;
  let host = 'localhost';
  let loading = true;
  let error = '';
  let firing = '';

  onMount(async () => {
    try {
      const [list, tpls, ip] = await Promise.all([
        kind === 'media' ? listMedia() : listAnnouncements(),
        listActiveTemplates().catch(() => []),
        localIp().catch(() => null),
      ]);
      rows = list ?? [];
      template = tpls[0] ?? null;
      if (ip) host = ip;
    } catch (e) {
      error = humanError(e);
    }
    loading = false;
  });

  async function fire(it) {
    firing = it.key;
    error = '';
    try {
      if (kind === 'media') await fireMedia(it.id);
      else await fireContent(it.reference, it.text, 'announce');
    } catch (e) {
      error = humanError(e);
    }
    firing = '';
  }

  const matches = (hay) => !query?.trim() || String(hay ?? '').toLowerCase().includes(query.trim().toLowerCase());

  $: slides = rows
    .filter((r) => (kind === 'media' ? matches(r.filename) : matches(r.title) || matches(r.body)))
    .map((r) =>
      kind === 'media'
        ? {
            key: `m${r.id}`,
            id: r.id,
            reference: r.filename,
            text: '',
            // Served by the app's own HTTP server — the same URL an OBS browser
            // source would use (:8032, never the Vite port).
            media: `http://${host}:8032/media/${r.id}`,
          }
        : {
            key: `a${r.id}`,
            id: r.id,
            reference: r.title,
            text: r.body,
            translation: null,
          },
    );
</script>

{#if loading}
  <Loading what={kind === 'media' ? 'media' : 'announcements'} />
{:else if !slides.length}
  <EmptyState
    message={query?.trim()
      ? `Nothing matching “${query.trim()}”.`
      : kind === 'media'
        ? 'No media yet — add some with the Import button.'
        : 'No announcements yet.'} />
{:else}
  <SlideGrid items={slides} template={kind === 'media' ? null : template} onFire={fire} busyKey={firing} />
{/if}

{#if error}<p class="cs-err">{error}</p>{/if}

<style>
  .cs-err {
    margin: 10px 0 0;
    font-size: var(--v-fs-b2);
    color: var(--v-red);
  }
</style>
