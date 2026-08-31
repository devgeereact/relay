<script>
  // LIBRARY → MEDIA. Pictures, video and documents, in the same shape as the
  // Bible pane: a type rail, a grid, and the shell's live column beside it.
  //
  // ── What is REAL here, and what the backend does not store ────────────────
  //
  // `media_assets` holds id · kind · filename · path · created_at and nothing
  // else (src-tauri/src/db/library.rs). There is no duration, no dimensions, no
  // file size and no generated thumbnail anywhere in the codebase. So this pane
  // does not print any of them.
  //
  // The thumbnail is therefore the FILE ITSELF, fetched from the app's own HTTP
  // server on :8032 — an <img> for a picture, a <video preload="metadata"> for a
  // video (the browser paints its first frame). Not a stand-in icon that might
  // not be what is on disk.
  //
  // ── Documents are shown, and cannot be fired, and say why ─────────────────
  //
  // `fire_media` refuses anything that is not an image or a video
  // ("documents can't be shown as an output background yet"). A PDF tile that
  // looks armed and errors on click is worse than one that is plainly not.
  import { onMount } from 'svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import { live, screenBlack, rehearsing } from '../../stores/capture.js';
  import { listMedia, deleteMedia, fireMedia, localIp } from '../../stores/capture.js';
  import VerseDeck from './VerseDeck.svelte';

  export let query = '';
  /**
   * 'image'  — Graphics: stills you put behind words.
   * 'moving' — Media: video and documents.
   * Two views of ONE table (`media_assets`); nothing is duplicated or invented.
   * The split exists because hunting past twenty MP4s for a logo is the job
   * this tab is supposed to make easy.
   */
  export let only = null;
  export let queue = [];
  export let onQueueChange = () => {};

  let rows = [];
  let host = 'localhost';
  let loading = true;
  let error = '';
  let msg = '';
  let firing = 0;
  let filter = 'all';
  /** Two-step delete: Tauri's webview does not implement window.confirm. */
  let armed = 0;
  let armedT;
  /**
   * Files whose bytes did not load. The row is a POINTER to a file on disk
   * (offline-first), and the two can part company — a deleted file, a moved
   * app-data folder, a failed import. `/media/<id>` then 404s and the tile would
   * ship a broken-image box, which tells the operator nothing. It says so
   * instead, and firing it is refused: a missing file cannot reach a screen.
   */
  let missing = {};
  let checked = new Set();
  let layout = 'grid';
  const lost = (m) => (missing = { ...missing, [m.id]: true });

  $: KINDS =
    only === 'image'
      ? [{ key: 'image', label: 'Graphics' }]
      : only === 'moving'
        ? [
            { key: 'all', label: 'All' },
            { key: 'video', label: 'Video' },
            { key: 'document', label: 'Documents' },
          ]
        : [
            { key: 'all', label: 'All' },
            { key: 'image', label: 'Images' },
            { key: 'video', label: 'Video' },
            { key: 'document', label: 'Documents' },
          ];
  $: if (only === 'image' && filter !== 'image') filter = 'image';
  $: if (only === 'moving' && filter === 'image') filter = 'all';

  onMount(refresh);

  async function refresh() {
    try {
      const [list, ip] = await Promise.all([listMedia(), localIp().catch(() => null)]);
      rows = list ?? [];
      if (ip) host = ip;
    } catch (e) {
      error = humanError(e);
    }
    loading = false;
  }

  // The SAME URL an OBS browser source would use — the app's HTTP server on
  // 8032, never the Vite port (which does not exist in a packaged build).
  const url = (m) => `http://${host}:8032/media/${m.id}`;
  const ext = (m) => (m.filename.split('.').pop() || '').toUpperCase();

  async function fire(m) {
    if (m.kind === 'document' || missing[m.id] || $safeMode) return;
    firing = m.id;
    error = '';
    msg = '';
    try {
      await fireMedia(m.id);
      msg = `${m.filename} is on the screens`;
    } catch (e) {
      error = humanError(e);
    }
    firing = 0;
  }

  async function remove(m) {
    if (armed !== m.id) {
      armed = m.id;
      clearTimeout(armedT);
      armedT = setTimeout(() => (armed = 0), 4000);
      return;
    }
    armed = 0;
    error = '';
    try {
      await deleteMedia(m.id);
      rows = rows.filter((r) => r.id !== m.id);
      msg = `Deleted ${m.filename}`;
    } catch (e) {
      error = humanError(e);
    }
  }

  const matches = (m) =>
    !query?.trim() || m.filename.toLowerCase().includes(query.trim().toLowerCase());

  // `only` is the tab; `filter` is the rail inside it. The tab wins.
  $: scoped = rows.filter((m) =>
    only === 'image' ? m.kind === 'image' : only === 'moving' ? m.kind !== 'image' : true,
  );
  $: shown = scoped.filter((m) => (filter === 'all' || m.kind === filter) && matches(m));
  $: counts = KINDS.map((k) => ({
    ...k,
    n: k.key === 'all' ? scoped.length : scoped.filter((r) => r.kind === k.key).length,
  }));
  // What is on the wall right now, so the grid can wear the tally.
  $: liveUrl = !$screenBlack && $live?.media_url ? $live.media_url : null;
  const isLive = (m, u) => !!u && u.endsWith(`/media/${m.id}`);

  // ONE card for every content type. A picture is its own thumbnail; a document
  // gets its extension, because `fire_media` refuses to put one on a screen and
  // a tile that looks armed and errors on click is worse than one that is not.
  $: deck = shown.map((m, i) => ({
    key: `m${m.id}`,
    id: m.id,
    reference: m.filename,
    label: m.filename,
    text: '',
    slideNo: i + 1,
    media: m.kind !== 'document' && !missing[m.id] ? url(m) : null,
    mediaKind: m.kind,
    icon: missing[m.id] ? 'MISSING' : m.kind === 'document' ? ext(m) : null,
  }));
  $: queuedRefs = new Set(queue.map((q) => q.reference));
  $: liveDeckRef = deck.find((d) => isLive({ id: d.id }, liveUrl))?.reference ?? null;

  function toggleQueue(item) {
    if (queue.some((q) => q.reference === item.reference)) {
      onQueueChange(queue.filter((q) => q.reference !== item.reference));
    } else {
      // A queued picture carries its id, so firing it later is still fire_media
      // and not a text cue that happens to be named after a file.
      onQueueChange([...queue, { reference: item.reference, text: '', mediaId: item.id }]);
    }
  }
  function toggleCheck(item) {
    const next = new Set(checked);
    next.has(item.reference) ? next.delete(item.reference) : next.add(item.reference);
    checked = next;
  }
  const fireCard = (d) => fire(rows.find((r) => r.id === d.id) ?? {});
  const removeCard = (d) => remove(rows.find((r) => r.id === d.id) ?? {});
</script>

<div class="ml">
  <div class="ml-grid">
    <!-- The TYPE RAIL — the Bible pane's book list, for file kinds. -->
    <nav class="ml-panel ml-rail" aria-label="Media type">
      <p class="r-lbl ml-panelhead">Type</p>
      <div class="ml-raillist r-scroll">
        {#each counts as k}
          <button class="ml-kind r-focus" class:on={filter === k.key} on:click={() => (filter = k.key)}>
            <span class="nm">{k.label}</span>
            <span class="ct r-mono">{k.n}</span>
          </button>
        {/each}
      </div>
      <div class="ml-panelfoot">
        <p class="ml-hint">
          Import handles pictures, video and documents. A document can be stored
          and found here, but cannot be put on a screen yet.
        </p>
      </div>
    </nav>

    <section class="ml-panel ml-main">
      <header class="ml-mainhead">
        <div class="ml-where">
          <b>{counts.find((k) => k.key === filter)?.label ?? 'All'}</b>
          <span>{shown.length} file{shown.length === 1 ? '' : 's'}</span>
        </div>
        {#if checked.size}
          <span class="r-chip amethyst">{checked.size} selected</span>
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

      <div class="ml-body r-scroll">
        {#if loading}
          <Loading what="the media library" />
        {:else if !deck.length}
          <EmptyState
            message={query?.trim()
              ? `No media matching “${query.trim()}”.`
              : only === 'image'
                ? 'No graphics yet — import a still with the Import button.'
                : 'No media yet — add some with the Import button.'} />
        {:else}
          <VerseDeck
            items={deck}
            liveRef={liveDeckRef}
            rehearsing={$rehearsing}
            {checked}
            {queuedRefs}
            busyRef={firing ? deck.find((d) => d.id === firing)?.reference ?? '' : ''}
            {layout}
            showStar={false}
            can={{ queue: true, favourite: false, edit: false, duplicate: false, add: false }}
            onCheck={toggleCheck}
            onFire={fireCard}
            onQueue={toggleQueue}
            onDelete={removeCard} />
        {/if}
      </div>
    </section>
  </div>

  {#if msg}<p class="ml-msg">{msg}</p>{/if}
  {#if error}<p class="ml-err" role="alert">{error}</p>{/if}
</div>

<style>
  .ml {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    flex: 1;
  }
  .ml-grid {
    display: grid;
    grid-template-columns: 180px minmax(0, 1fr);
    gap: 12px;
    min-height: 0;
    flex: 1;
  }
  .ml-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--v-bg);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
  }
  .ml-panelhead {
    margin: 0;
    padding: 13px 14px 9px;
  }
  .ml-raillist {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 8px 8px;
  }
  .ml-kind {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border-radius: var(--v-r-md);
    background: none;
    border: 0;
    color: var(--v-dim);
    font-family: var(--f-body);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .ml-kind .nm {
    flex: 1;
  }
  .ml-kind .ct {
    font-size: 11px;
    color: var(--v-faint);
  }
  .ml-kind:hover:not(.on) {
    background: var(--v-surf2);
    color: var(--v-txt);
  }
  .ml-kind.on {
    background: var(--v-accent-fill);
    color: var(--v-accent-ink);
    font-weight: 600;
  }
  .ml-kind.on .ct {
    color: rgba(255, 255, 255, 0.75);
  }
  .ml-panelfoot {
    padding: 12px 14px;
    border-top: 1px solid var(--v-line);
  }
  .ml-hint {
    margin: 0;
    font-size: var(--v-fs-cap);
    line-height: 1.6;
    color: var(--v-faint);
  }

  .ml-mainhead {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 14px;
    border-bottom: 1px solid var(--v-line);
  }
  .ml-where b {
    display: block;
    font-size: 15px;
    font-weight: 600;
    color: var(--v-txt);
  }
  .ml-where span {
    font-size: var(--v-fs-cap);
    color: var(--v-faint);
  }
  .ml-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 12px;
  }

  /* THE TALLY. One meaning: this is on the congregation's screen. */

  .ml-msg,
  .ml-err {
    margin: 0;
    font-size: var(--v-fs-b2);
  }
  .ml-msg {
    color: var(--v-emerald);
  }
  .ml-err {
    color: var(--v-red);
  }

  @media (max-width: 1140px) {
    .ml-grid {
      grid-template-columns: minmax(0, 1fr);
    }
    .ml-rail {
      display: none;
    }
  }
</style>
