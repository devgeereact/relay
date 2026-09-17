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
  // AND THERE IS NO CAPTION COLUMN. REBRAND §10 asks for "a caption stored apart
  // from the item's name"; `filename` is ALREADY the operator's own words — the
  // add sheet writes what they typed into it and keeps the real file name only as
  // a hint on screen — so a caption would be a SECOND operator-authored string
  // beside the one that exists, and §10's own sentence for media is "the slide
  // *is* the picture", which means none of it reaches a congregation. Not built,
  // deliberately; the card's second line comes from `collections.js::mediaSub`,
  // rather than a second name.
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
  import ErrorState from '../../ui/ErrorState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import { live, screenBlack, rehearsing, background } from '../../stores/capture.js';
  import {
    listMedia,
    deleteMedia,
    fireMedia,
    showBackground,
    localIp,
    readErrors,
  } from '../../stores/capture.js';
  import VerseDeck from './VerseDeck.svelte';
  // The card's second line, and the reason there is no caption column, both
  // live in the register beside `countWords` — pure, so they can be asserted
  // without mounting a pane that needs a backend to list anything.
  import { mediaSub } from './collections.js';
  import { mediaUrl } from '../../bundledbackgrounds.js';

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
  /** Hand the selected item up for the inspector (REBRAND §10). */
  export let onSelect = () => {};

  /**
   * ONE PRESS SELECTS — see the note on `VerseDeck`'s `press` prop.
   *
   * THE SLIDE IS THE PICTURE (REBRAND §10). The inspector is handed the media
   * URL rather than a template and a caption, so the preview is the frame the
   * room will see; the filename is the operator's name for it and never goes to
   * a screen. A DOCUMENT has no frame at all, which is why `fire_media` refuses
   * one, so it is selectable and carries no `reference` for `Cue in Live`.
   */
  let selectedRef = '';
  function selectItem(m) {
    selectedRef = m.reference;
    onSelect({
      kind: 'media',
      title: m.label,
      titleLabel: 'Name',
      words: m.icon ? `${m.icon} — this cannot be put on a screen.` : m.label,
      slide: { reference: null, text: m.label },
      media: m.media,
      mediaKind: m.mediaKind,
      reference: m.media ? m.reference : null,
      mediaId: m.id,
      plan: m.media
        ? {
            cueType: 'media',
            label: m.reference,
            payload: { media_id: m.id, kind: m.mediaKind, filename: m.reference },
          }
        : null,
    });
  }

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
  //
  // AND THE SAME RULE THE BACKEND USES, out of one module, because there are two
  // doors and this is the second. `main.rs::media_url` builds this for every
  // output screen; this pane builds it again because the thumbnail IS the file.
  // A picture Relay ships has no file under `/media/<id>` at all (DECISIONS
  // §90), so a rule kept on one door and not the other would have left every
  // seeded background a broken-image box here — and `lost()` below would then
  // have marked it missing and refused to fire a file that was never missing.
  const url = (m) => mediaUrl(host, m);
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

  /**
   * PUT THIS PICTURE BEHIND EVERYTHING — the standing background.
   *
   * The difference from `fire()` above is the whole of it, and it is a difference
   * of LIFETIME rather than of look. Firing a picture makes it the slide: the next
   * verse replaces it. Setting it as the background paints it UNDER everything
   * that follows, so a reading and the church's own backdrop can be on a wall at
   * the same time — which until now could not be expressed at all.
   *
   * A screen only shows it if its template has a Backdrop layer (Templates → add a
   * layer). That is the opt-in, and the message says so rather than leaving an
   * operator watching a wall that did not change.
   */
  let settingBg = 0;
  async function setAsBackground(m) {
    if (m.kind === 'document' || missing[m.id] || $safeMode) return;
    settingBg = m.id;
    error = '';
    msg = '';
    try {
      await showBackground(m.id);
      msg = `${m.filename} is behind everything on screens with a Backdrop layer`;
    } catch (e) {
      error = humanError(e);
    }
    settingBg = 0;
  }

  /** TAKE IT OFF. One control, both directions — see `showBackground`. */
  async function clearBackgroundNow() {
    error = '';
    msg = '';
    try {
      await showBackground(null);
      msg = 'Background cleared';
    } catch (e) {
      error = humanError(e);
    }
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
    sub: mediaSub(m),
  }));
  $: queuedRefs = new Set(queue.map((q) => q.reference));
  $: liveDeckRef = deck.find((d) => isLive({ id: d.id }, liveUrl))?.reference ?? null;

  function toggleQueue(item) {
    if (queue.some((q) => q.reference === item.reference)) {
      onQueueChange(queue.filter((q) => q.reference !== item.reference));
    } else {
      // A queued picture carries its id, so firing it later is still fire_media
      // and not a text cue that happens to be named after a file.
      onQueueChange([...queue, { reference: item.reference, text: '', mediaId: item.id, kind: 'media' }]);
    }
  }
  // THE SELECTED ROW, and only when it can actually become a background. A
  // document has no frame to paint and `show_background` refuses one, and a row
  // whose bytes did not load would hand every screen a URL that 404s — so the
  // control is plainly disabled rather than armed and erroring on click, which is
  // the same judgement the fire path already makes one function above.
  $: selectedPicture =
    rows.find((r) => r.filename === selectedRef && r.kind !== 'document' && !missing[r.id]) ?? null;
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
      <!-- ── THE STANDING BACKGROUND ────────────────────────────────────────
           A picture that outlives the words painted on it. It lives HERE, beside
           the pictures, rather than on the run surface: choosing which picture is
           behind everything is a Library act, and Live is already the most
           crowded surface in the product.

           Two controls, and the take-down is always reachable — never conditional
           on a selection, never hidden behind a menu. A background an operator
           cannot get off a congregation screen in one press is the shape of the
           thing the panic controls exist to prevent, and `Esc` (Clear screens)
           takes it too. -->
      <div class="ml-panelfoot">
        <p class="r-lbl">Background</p>
        <p class="ml-hint">
          {#if $background}
            A picture is behind everything on screens whose template has a
            Backdrop layer. Clear screens takes it off too.
          {:else}
            Put a picture behind the words. It stays there while verses, songs
            and notices are fired over it, on screens whose template has a
            Backdrop layer.
          {/if}
        </p>
        <div class="ml-bgrow">
          <button
            class="r-btn sm"
            disabled={!selectedPicture || settingBg === selectedPicture?.id || $safeMode}
            title={selectedPicture
              ? `Put ${selectedPicture.filename} behind everything`
              : 'Pick a picture in the grid first'}
            on:click={() => setAsBackground(selectedPicture)}>Use as background</button>
          <button
            class="r-btn sm ghost"
            disabled={!$background}
            on:click={clearBackgroundNow}>Clear background</button>
        </div>
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
        {:else if $readErrors.listMedia}
          <!-- RG-95. `listMedia` swallows to `[]`, so an empty deck cannot tell a
               library with nothing in it from a database that did not answer. The
               operator was told "No media yet — add some with the Import button",
               and would have imported the file again. -->
          <ErrorState error={$readErrors.listMedia} onRetry={refresh} />
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
            {queuedRefs}
            busyRef={firing ? deck.find((d) => d.id === firing)?.reference ?? '' : ''}
            {layout}
            showStar={false}
            press="select"
            {selectedRef}
            onSelect={selectItem}
            can={{ queue: true, favourite: false, edit: false, duplicate: false, add: false, select: false }}
            onFire={fireCard}
            onQueue={toggleQueue}
            onDelete={removeCard} />
        {/if}
      </div>
    </section>
  </div>

  <!-- Announced. "John 3:16 is on the screens" is the confirmation that content
         reached a congregation, and it was silent to a screen reader — the error
         half of these panes carries `role="alert"` and this half carried nothing. -->
  {#if msg}<p class="ml-msg" role="status" aria-live="polite">{msg}</p>{/if}
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
  /* A LIST ROW, not a button — B2. One media kind per row in the rail, a name
     and a count, filtering the grid beside it. Selected, not pressed. */
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
    font-size: var(--v-fs-pr);
    text-align: left;
    cursor: pointer;
  }
  .ml-kind .nm {
    flex: 1;
  }
  .ml-kind .ct {
    font-size:var(--v-fs-lbl);
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
  /* The two background controls. They WRAP rather than shrink: the rail is 180px
     and a button squeezed to fit prints a label nobody can read, which is the
     seven-pixel-wide screen name the 2026-09-10 pass found on the run surface. */
  .ml-bgrow {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 8px 0 10px;
  }
  .ml-bgrow :global(.r-btn) {
    flex: 1 1 auto;
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
    font-size: var(--v-fs-ttl);
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
