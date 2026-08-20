<script>
  // THE DECK — reference: relay-main-library-screen.png, centre pane.
  //
  // ONE card for every content type in the Library: scripture, saved verses,
  // lyric slides, pictures, video and notices. The operator's job is identical
  // in all six cases — find the thing, see it, put it on the wall — and six
  // browsing metaphors would be six things to learn under pressure.
  //
  // Cards that are the SLIDE, not a description of it: each one is drawn by
  // `TemplateRender`, the same component that paints the projector and the
  // Templates preview. A thumbnail is therefore not an approximation of what
  // will appear — it is the same code at a different size. Change the template
  // and every card changes with it, by construction.
  //
  // Card chrome, per the reference: a select checkbox top-left, a favourite
  // star top-right, and a footer with the verse number, its reference and a
  // kebab menu. The tally ring is amber and means one thing — the congregation
  // is looking at this.
  import TemplateRender from '../../TemplateRender.svelte';
  import { safeMode } from '../../boot/boot.js';

  export let items = [];
  export let template = null;
  export let liveRef = null;
  export let rehearsing = false;
  export let selectedRef = '';
  export let checked = new Set();
  export let savedRefs = new Set();
  export let queuedRefs = new Set();
  export let busyRef = '';
  export let layout = 'grid';
  export let onCheck = () => {};
  export let onFire = () => {};
  export let onQueue = () => {};
  export let onSave = () => {};
  export let onEdit = () => {};
  export let onDuplicate = () => {};
  export let onAddAfter = () => {};
  export let onDelete = null;
  export let onMove = null;
  /** Which kebab actions this content type can honestly offer. */
  export let can = { queue: true, favourite: true, edit: true, duplicate: true, add: true, move: false };
  /** Favourite stars only make sense where something can be favourited. */
  export let showStar = true;

  let menuFor = '';

  /**
   * ENTER fires a list row. SPACE DOES NOT — Space is the transport, app-wide.
   *
   * CLAUDE.md rule 11: *"`Space` means advance, app-wide, and nothing else."* The
   * GRID card is a native `<button>`, and `shortcuts.js` calls `preventDefault` on
   * Space globally, which suppresses the button's own activation — so in the grid,
   * Space advances the service and nothing else. This row is a `role="button"` div
   * with its own handler, which ran FIRST and answered Space by putting scripture
   * in front of a congregation. Same deck, same content, two layouts, one key, two
   * meanings — and the extra meaning was the dangerous one. Six views render this.
   *
   * Note what the repair is NOT: adding `stopPropagation` so the row fires and the
   * transport does not. That closes the double-action and leaves the two layouts
   * still disagreeing, which is the actual finding. Space now falls through here
   * exactly as it does on the grid card.
   *
   * The ARIA authoring practices say a `role="button"` should answer both keys.
   * This app deliberately overrides Space everywhere, native buttons included, and
   * an operator who has learned "Space advances" must not meet an exception on a
   * live surface. Enter remains the activation key, which is what a keyboard
   * operator reaches for to act on the row they are focused on.
   */
  function rowKey(v) {
    return (e) => {
      if (e.key !== 'Enter') return;
      e.preventDefault();
      e.stopPropagation();
      onFire(v);
    };
  }

  /** Escape closes the kebab menu and goes NO FURTHER — never to the panic key. */
  function menuEsc(e) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      e.preventDefault();
      menuFor = '';
    }
  }
</script>

{#if layout === 'list'}
  <div class="vd-list">
    {#each items as v (v.reference)}
      {@const air = liveRef === v.reference}
      <div
        class="vd-row r-focus"
        class:air
        class:on={selectedRef === v.reference}
        role="button"
        tabindex="0"
        data-verse={v.verse}
        aria-label={$safeMode
          ? `Safe mode — ${v.reference} cannot reach a screen`
          : `Put ${v.reference} on the screens`}
        on:click={() => onFire(v)}
        on:keydown={rowKey(v)}>
        <span class="vd-n r-mono">{v.slideNo}</span>
        {#if v.media}
          <span class="vd-rthumb">
            {#if v.mediaKind === 'video'}
              <!-- svelte-ignore a11y-media-has-caption -->
              <video src={v.media} preload="metadata" muted playsinline></video>
            {:else}
              <img src={v.media} alt="" loading="lazy" />
            {/if}
          </span>
        {/if}
        <span class="vd-rbody">
          <b>{v.label ?? v.reference}</b>
          {#if v.text}<span class="vd-rtext">{v.text}</span>{/if}
        </span>
        <span class="vd-racts">
          {#if air}
            <span class="r-badge {rehearsing ? 'amethyst' : 'amber'}"><span class="bd"></span>{rehearsing ? 'Rehearsal' : 'On Air'}</span>
          {/if}
          <button class="vd-ic r-focus" class:on={savedRefs.has(v.reference)} aria-label="Favourite {v.reference}" on:click|stopPropagation={() => onSave(v)}>★</button>
          <button class="vd-ic r-focus" class:on={queuedRefs.has(v.reference)} aria-label="Queue {v.reference}" on:click|stopPropagation={() => onQueue(v)}>+</button>
          <button class="vd-ic r-focus" aria-label="Put {v.reference} on the screens" disabled={$safeMode} on:click|stopPropagation={() => onFire(v)}>→</button>
        </span>
      </div>
    {/each}
  </div>
{:else}
  <div class="vd" class:big={layout === 'large'}>
    {#each items as v (v.reference)}
      {@const air = liveRef === v.reference}
      <article
        class="vd-card"
        class:air
        class:reh={air && rehearsing}
        class:on={selectedRef === v.reference}
        class:checked={checked.has(v.reference)}>
        <!-- CLICKING A CARD FIRES IT. The card is not a thumbnail to enlarge —
             it is already the slide at a readable size, and the operator's next
             action after finding it is always the same one. -->
        <button
          class="vd-shot r-focus"
          disabled={$safeMode}
          aria-label={$safeMode
            ? `Safe mode — ${v.reference} cannot reach a screen`
            : `Put ${v.reference} on the screens`}
          on:click={() => onFire(v)}>
          {#if v.media}
            <!-- A picture or a video is its own thumbnail. Drawing it through a
                 text template would show an empty frame with a filename under it. -->
            {#if v.mediaKind === 'video'}
              <!-- svelte-ignore a11y-media-has-caption -->
              <video class="vd-media" src={v.media} preload="metadata" muted playsinline></video>
              <span class="vd-play" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="20" height="20" fill="currentColor"><path d="M8 5v14l11-7z" /></svg>
              </span>
            {:else}
              <img class="vd-media" src={v.media} alt="" loading="lazy" />
            {/if}
          {:else if v.icon}
            <span class="vd-doc">
              <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"><path d="M6 2h9l5 5v15H6z" /><path d="M15 2v5h5" /></svg>
              <b class="r-mono">{v.icon}</b>
            </span>
          {:else if template}
            <TemplateRender
              {template}
              content={{
                reference: v.hideReference ? null : (v.label ?? v.reference),
                text: v.text,
                translation: v.translation,
              }} />
          {:else}
            <!-- Even without a template the card must show what the WALL will
                 show: a lyric slide projects the lyric, not the section name. -->
            <span class="vd-plain">
              {#if !v.hideReference}<b>{v.label ?? v.reference}</b>{/if}
              {v.text}
            </span>
          {/if}
          <span class="vd-go">Go live →</span>
        </button>

        <label class="vd-check" title="Select for a bulk action">
          <input
            type="checkbox"
            checked={checked.has(v.reference)}
            on:change={() => onCheck(v)} />
          <span></span>
        </label>

        {#if showStar}
        <button
          class="vd-star r-focus"
          class:on={savedRefs.has(v.reference)}
          aria-label={savedRefs.has(v.reference) ? `Remove ${v.reference} from favourites` : `Add ${v.reference} to favourites`}
          on:click|stopPropagation={() => onSave(v)}>
          <svg viewBox="0 0 24 24" width="15" height="15" fill={savedRefs.has(v.reference) ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 3.5l2.6 5.6 6 .8-4.4 4.2 1.1 6-5.3-2.9-5.3 2.9 1.1-6L3.4 9.9l6-.8z" /></svg>
        </button>
        {/if}

        {#if air}
          <span class="r-badge {rehearsing ? 'amethyst' : 'amber'} vd-tally">
            <span class="bd"></span>{rehearsing ? 'Rehearsal' : 'On Air'}
          </span>
        {:else if busyRef === v.reference}
          <span class="r-badge grey vd-tally">Sending…</span>
        {:else if v.edited}
          <span class="r-badge amethyst vd-tally">Edited</span>
        {:else if queuedRefs.has(v.reference)}
          <!-- QUEUED is grey. It is what happens next, which is not what the
               congregation is looking at, and amber may never lie. -->
          <span class="r-badge grey vd-tally">Queued</span>
        {/if}

        <footer class="vd-foot">
          <span class="vd-n r-mono">{v.slideNo}</span>
          <span class="vd-ref">{v.reference}</span>
          <div class="vd-menuwrap">
            <button
              class="vd-kebab r-focus"
              aria-label="Actions for {v.reference}"
              on:click|stopPropagation={() => (menuFor = menuFor === v.reference ? '' : v.reference)}>
              <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor"><circle cx="12" cy="5" r="1.6" /><circle cx="12" cy="12" r="1.6" /><circle cx="12" cy="19" r="1.6" /></svg>
            </button>
            {#if menuFor === v.reference}
              <button class="vd-scrim" tabindex="-1" aria-label="Close" on:click={() => (menuFor = '')}></button>
              <!-- `role="menu"` is load-bearing: `shortcuts.js` probes the DOM to
                   decide whether Escape belongs to an overlay or to the panic key,
                   so a popup with no role let Escape wipe the wall. -->
              <div class="vd-menu" role="menu" tabindex="-1" on:keydown={menuEsc}>
                <button class="vd-mi air" disabled={$safeMode} on:click={() => { menuFor = ''; onFire(v); }}>
                  Take to screen
                </button>
                {#if can.queue}
                  <button class="vd-mi" on:click={() => { menuFor = ''; onQueue(v); }}>
                    {queuedRefs.has(v.reference) ? 'Remove from queue' : 'Add to queue'}
                  </button>
                {/if}
                {#if can.favourite}
                  <button class="vd-mi" on:click={() => { menuFor = ''; onSave(v); }}>
                    {savedRefs.has(v.reference) ? 'Remove from favourites' : 'Add to favourites'}
                  </button>
                {/if}
                {#if can.edit || can.duplicate || can.add || onDelete}
                  <span class="vd-msep"></span>
                {/if}
                {#if can.edit}
                  <button class="vd-mi" on:click={() => { menuFor = ''; onEdit(v); }}>Edit slide…</button>
                {/if}
                {#if can.duplicate}
                  <button class="vd-mi" on:click={() => { menuFor = ''; onDuplicate(v); }}>Duplicate slide</button>
                {/if}
                {#if can.add}
                  <button class="vd-mi" on:click={() => { menuFor = ''; onAddAfter(v); }}>Add slide after</button>
                {/if}
                {#if can.move && onMove}
                  <button class="vd-mi" on:click={() => { menuFor = ''; onMove(v, -1); }}>Move up</button>
                  <button class="vd-mi" on:click={() => { menuFor = ''; onMove(v, 1); }}>Move down</button>
                {/if}
                {#if onDelete}
                  <button class="vd-mi danger" on:click={() => { menuFor = ''; onDelete(v); }}>Delete</button>
                {/if}
              </div>
            {/if}
          </div>
        </footer>
      </article>
    {/each}
  </div>
{/if}

<style>
  /* SIZED TO BE READ, not to fit the most cards.
     `TemplateRender` scales its type in container units, so a wider card is
     literally larger text — the only thing that decides whether an operator can
     recognise a slide at a glance in a dark booth. Three across on a normal
     console beats five they have to lean in for. */
  .vd {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(268px, 1fr));
    gap: 14px;
  }
  .vd.big {
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
  }

  .vd-card {
    position: relative;
    display: flex;
    flex-direction: column;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    /* NOT `overflow: hidden`. That clipped the kebab menu to the card, so the
       edit actions opened half-cut and looked misaligned. The shot does its own
       clipping instead, which is all the rounding needed. */
    /* NOT `content-visibility: auto` — it FIGHTS the auto-fit. The fit loop reads
       `scrollHeight`, which forces the browser to synchronously render the very
       cards content-visibility was skipping, so interactions (like firing a verse)
       janked. The font-refit guard already removed the real per-grid cost. */
  }
  .vd-card:hover {
    border-color: var(--v-line2);
  }
  /* Selection is chrome — the accent. The reference paints it amethyst too. */
  .vd-card.on,
  .vd-card.checked {
    border-color: var(--v-accent);
    box-shadow: 0 0 0 1px var(--v-accent);
  }
  /* THE TALLY. One meaning: this is on the congregation's screen. */
  .vd-card.air {
    border-color: var(--v-amber);
    box-shadow: 0 0 0 1px var(--v-amber), 0 6px 20px -8px var(--v-amber-glow);
  }
  .vd-card.reh {
    border-color: var(--v-amethyst);
    box-shadow: 0 0 0 1px var(--v-amethyst);
  }

  .vd-shot {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 16 / 9;
    padding: 0;
    border: 0;
    background: #000;
    border-radius: calc(var(--v-r-lg) - 1px) calc(var(--v-r-lg) - 1px) 0 0;
    container-type: inline-size;
    cursor: pointer;
    overflow: hidden;
  }
  .vd-plain {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 16px;
    height: 100%;
    justify-content: center;
    text-align: center;
    font-family: var(--f-serif);
    font-size: 12px;
    line-height: 1.5;
    color: var(--v-dim);
  }
  .vd-media {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .vd-play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #fff;
    text-shadow: 0 2px 10px rgba(0, 0, 0, 0.6);
    pointer-events: none;
  }
  .vd-doc {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 6px;
    color: var(--v-faint);
    background: var(--v-surf2);
  }
  .vd-doc b {
    font-size: 10px;
    letter-spacing: 0.12em;
  }
  .vd-mi.danger {
    color: var(--v-rose);
  }
  .vd-mi.danger:hover {
    background: var(--v-rose-soft);
    color: var(--v-rose);
  }
  .vd-plain b {
    font-size: 15px;
    color: var(--v-amber2);
  }
  /* The card fires, so it says so before it is clicked. */
  .vd-go {
    position: absolute;
    right: 8px;
    bottom: 8px;
    font-family: var(--f-mono);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--v-amber);
    opacity: 0;
    transition: opacity 0.14s;
  }
  .vd-shot:hover .vd-go,
  .vd-shot:focus-visible .vd-go {
    opacity: 1;
  }
  .vd-shot:disabled {
    cursor: not-allowed;
  }

  .vd-check {
    position: absolute;
    top: 8px;
    left: 8px;
    width: 20px;
    height: 20px;
    cursor: pointer;
  }
  .vd-check input {
    position: absolute;
    opacity: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    cursor: pointer;
  }
  .vd-check span {
    display: block;
    width: 20px;
    height: 20px;
    border-radius: var(--v-r-sm);
    border: 1px solid var(--v-line2);
    background: rgba(10, 10, 10, 0.72);
    transition: background 0.14s, border-color 0.14s;
  }
  .vd-check input:checked + span {
    background: var(--v-accent-fill);
    border-color: var(--v-accent-fill);
  }
  .vd-check input:checked + span::after {
    content: '';
    display: block;
    width: 5px;
    height: 9px;
    margin: 3px auto 0;
    border: solid var(--v-accent-ink);
    border-width: 0 2px 2px 0;
    transform: rotate(45deg);
  }
  .vd-check input:focus-visible + span {
    outline: 2px solid var(--v-accent2);
    outline-offset: 2px;
  }

  .vd-star {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--v-r-sm);
    background: rgba(10, 10, 10, 0.55);
    color: var(--v-faint);
    cursor: pointer;
    transition: color 0.14s;
  }
  .vd-star:hover {
    color: var(--v-txt);
  }
  .vd-star.on {
    color: var(--v-amber2);
  }
  /* The tally sits on the FOOTER edge, not over the slide. Top-right put it
     across the verse text — and a badge that obscures the words is a badge
     that makes the operator lean in to read past it. */
  .vd-tally {
    position: absolute;
    left: 8px;
    bottom: 42px;
  }

  .vd-foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 11px;
    border-top: 1px solid var(--v-line);
    border-radius: 0 0 calc(var(--v-r-lg) - 1px) calc(var(--v-r-lg) - 1px);
    background: var(--v-surf);
  }
  .vd-n {
    font-size: 11px;
    color: var(--v-faint);
  }
  .vd-ref {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    color: var(--v-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .vd-menuwrap {
    position: relative;
    z-index: 5;
  }
  .vd-kebab {
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--v-r-sm);
    background: transparent;
    color: var(--v-faint);
    cursor: pointer;
  }
  .vd-kebab:hover {
    background: var(--v-surf3);
    color: var(--v-txt);
  }
  .vd-scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: 0;
    cursor: default;
  }
  .vd-msep {
    height: 1px;
    margin: 4px 2px;
    background: var(--v-line);
  }
  .vd-menu {
    position: absolute;
    right: 0;
    bottom: calc(100% + 6px);
    z-index: 50;
    min-width: 178px;
    padding: 6px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line2);
    border-radius: var(--v-r-lg);
    box-shadow: var(--v-shadow-lg);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .vd-mi {
    text-align: left;
    padding: 8px 10px;
    border: 0;
    border-radius: var(--v-r-md);
    background: transparent;
    color: var(--v-txt);
    font-family: var(--f-body);
    font-size: 12.5px;
    cursor: pointer;
    white-space: nowrap;
  }
  .vd-mi:hover:not(:disabled) {
    background: var(--v-surf3);
    color: var(--v-accent2);
  }
  /* The one item in the menu that reaches a congregation. */
  .vd-mi.air {
    color: var(--v-amber);
  }
  .vd-mi.air:hover:not(:disabled) {
    background: var(--v-amber-soft);
    color: var(--v-amber);
  }
  .vd-mi:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  /* ── List layout ───────────────────────────────────────────────────────── */
  .vd-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .vd-row {
    position: relative;
    display: grid;
    grid-template-columns: 26px auto minmax(0, 1fr) auto;
    gap: 10px;
    align-items: center;
    padding: 10px 12px;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-md);
    cursor: pointer;
  }
  .vd-row:hover {
    border-color: var(--v-line2);
    background: var(--v-surf2);
  }
  .vd-row.on {
    border-color: var(--v-accent-line);
    background: var(--v-accent-soft);
  }
  .vd-row.air {
    border-color: rgba(255, 176, 0, 0.42);
    background: rgba(255, 176, 0, 0.05);
  }
  .vd-row.air::before {
    content: '';
    position: absolute;
    left: 0;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--v-amber);
  }
  .vd-rbody {
    min-width: 0;
  }
  .vd-rbody b {
    display: block;
    font-size: 13px;
    font-weight: 600;
    color: var(--v-txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .vd-rtext {
    display: -webkit-box;
    margin-top: 2px;
    font-size: 12.5px;
    line-height: 1.45;
    color: var(--v-dim);
    overflow: hidden;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .vd-rthumb {
    width: 64px;
    height: 36px;
    border-radius: var(--v-r-sm);
    overflow: hidden;
    background: #000;
    flex: 0 0 auto;
  }
  .vd-rthumb img,
  .vd-rthumb video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .vd-racts {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .vd-ic {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--v-r-sm);
    background: transparent;
    color: var(--v-faint);
    font-size: 13px;
    cursor: pointer;
  }
  .vd-ic:hover:not(:disabled) {
    background: var(--v-surf3);
    color: var(--v-txt);
  }
  .vd-ic.on {
    color: var(--v-accent2);
  }
  .vd-ic:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
