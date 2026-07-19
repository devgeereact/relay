<script>
  // THE SLIDE GRID — the ProPresenter idea, made honest.
  //
  // ── Why the thumbnails are real ───────────────────────────────────────────
  //
  // ProPresenter's actual insight is not its layout, it is that an operator
  // picks the slide by LOOKING at it. A list of references — "1:1, 1:2, 1:3" —
  // makes you read and imagine; a grid of rendered slides makes you recognise.
  // Under pressure, in a dark booth, recognition is the only one that works.
  //
  // Relay can do this without lying, because `TemplateRender` is THE renderer:
  // the same component draws the fullscreen output, the Templates editor preview
  // and these thumbnails. A thumbnail is therefore not an approximation of what
  // will appear — it is the same code, at a different size. If the template
  // changes, every thumbnail changes with it, by construction.
  //
  // ── What the amber ring means ─────────────────────────────────────────────
  //
  // Exactly one thing: THIS slide is on the congregation's screen right now.
  // That is the one legitimate use of amber (DECISIONS §22), and it is what
  // makes the library answer the question an operator actually has mid-service —
  // "what is up there?" — without leaving the tab they are working in.
  //
  // Clicking a slide FIRES it. Not a preview, not a selection: it goes on the
  // wall. Labelled as such, and refused outright in safe mode.

  import TemplateRender from '../../TemplateRender.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import { live, screenBlack, rehearsing } from '../../stores/capture.js';
  import { safeMode } from '../../boot/boot.js';

  /**
   * One per slide: `{ key, reference, text, translation, media, kind }`.
   *
   * `reference` is the IDENTITY — what the live-match compares against, and what
   * gets fired. `label` is what the bar shows; when a song is already selected in
   * the rail, repeating its title on all twelve slides is noise.
   * `hideReference` keeps the reference out of the RENDERED slide: a lyric slide
   * projects the lyric, not the song title.
   * `media` (a URL) renders a picture instead of a template — a media slide has
   * no verse to typeset, and drawing one through the scripture template would
   * show an empty frame with a filename under it.
   */
  export let items = [];
  /** The template to draw them with. Null → a plain text card. */
  export let template = null;
  /** (item) => Promise — what firing this slide does. */
  export let onFire = () => {};
  export let busyKey = '';

  // Rendering 176 live template instances (Psalm 119) at once is not free, so
  // the grid pages rather than pretending the cost is zero.
  const PAGE = 18;
  let page = 0;
  $: pageCount = Math.max(1, Math.ceil(items.length / PAGE));
  $: if (page > pageCount - 1) page = 0;
  $: shown = items.slice(page * PAGE, page * PAGE + PAGE);
  // Reset to the first page whenever the source changes underneath us.
  $: if (items) page = page;

  /** Is this slide the one currently on the wall? */
  function isLive(it, $live, $screenBlack) {
    if (!$live || $screenBlack) return false;
    return (
      !!it.reference &&
      !!$live.reference &&
      it.reference.trim().toLowerCase() === String($live.reference).trim().toLowerCase()
    );
  }
</script>

{#if !items.length}
  <EmptyState message="Nothing here yet." />
{:else}
  <div class="sg">
    {#each shown as it (it.key)}
      {@const onAir = isLive(it, $live, $screenBlack)}
      <button
        class="sg-cell"
        class:on-air={onAir}
        class:rehearse={onAir && $rehearsing}
        disabled={$safeMode || busyKey === it.key}
        title={$safeMode
          ? 'Safe mode — nothing can reach a screen'
          : `Put ${it.reference} on the output screens`}
        on:click={() => onFire(it)}
      >
        <span class="sg-thumb">
          {#if it.media}
            <img class="sg-img" src={it.media} alt="" loading="lazy" />
          {:else if template}
            <!-- The SAME renderer as the projector. Not a mock-up of it. -->
            <TemplateRender
              {template}
              content={{
                reference: it.hideReference ? null : it.reference,
                text: it.text,
                translation: it.translation,
              }}
            />
          {:else}
            <span class="sg-plain">{it.text}</span>
          {/if}
        </span>

        <span class="sg-bar">
          <span class="sg-ref">{it.label ?? it.reference}</span>
          {#if onAir}
            <!-- AMBER means one thing: they are looking at this. Except in
                 rehearsal, where nothing reaches them — so it says so. -->
            <span class="sg-badge" class:reh={$rehearsing}>
              {$rehearsing ? 'REHEARSAL' : 'ON AIR'}
            </span>
          {:else if busyKey === it.key}
            <span class="sg-badge busy">…</span>
          {/if}
        </span>
      </button>
    {/each}
  </div>

  {#if pageCount > 1}
    <div class="sg-pages">
      <button disabled={page === 0} on:click={() => (page -= 1)}>Previous</button>
      <span class="r-mono">{page + 1} / {pageCount}</span>
      <button disabled={page >= pageCount - 1} on:click={() => (page += 1)}>Next</button>
    </div>
  {/if}
{/if}

<style>
  .sg {
    display: grid;
    /* Bigger on purpose. At 190px the verse text was legible only to someone who
       already knew what it said, which defeats the point of rendering it at all —
       an operator must be able to READ the slide to recognise it. */
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 12px;
  }
  .sg-cell {
    display: flex;
    flex-direction: column;
    padding: 0;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    overflow: hidden;
    cursor: pointer;
    font: inherit;
    text-align: left;
    transition: border-color 0.14s, transform 0.1s;
  }
  .sg-cell:hover:not(:disabled) {
    border-color: var(--v-accent-line);
  }
  .sg-cell:active:not(:disabled) {
    transform: scale(0.99);
  }
  .sg-cell:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  /* THE TALLY. One meaning: this is on the congregation's screen. */
  .sg-cell.on-air {
    border-color: var(--v-amber);
    box-shadow: 0 0 0 1px var(--v-amber), 0 6px 20px -8px var(--v-amber-glow);
  }
  /* ...unless nothing is reaching them, in which case it must not wear amber. */
  .sg-cell.rehearse {
    border-color: var(--v-amethyst);
    box-shadow: 0 0 0 1px var(--v-amethyst);
  }

  .sg-thumb {
    display: block;
    aspect-ratio: 16 / 9;
    container-type: inline-size;
    background: #000;
    overflow: hidden;
    position: relative;
  }
  .sg-img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
  .sg-plain {
    display: block;
    padding: 10px;
    font-family: var(--f-serif);
    font-size: 11px;
    line-height: 1.45;
    color: var(--v-dim);
  }

  .sg-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border-top: 1px solid var(--v-line);
  }
  .sg-ref {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--v-txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sg-badge {
    flex: 0 0 auto;
    padding: 2px 7px;
    border-radius: 99px;
    background: var(--v-amber-soft);
    border: 1px solid rgba(255, 176, 0, 0.35);
    color: var(--v-amber);
    font-family: var(--f-mono);
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.1em;
  }
  .sg-badge.reh {
    background: var(--v-amethyst-soft);
    border-color: rgba(139, 92, 246, 0.42);
    color: var(--v-amethyst);
  }
  .sg-badge.busy {
    background: var(--v-surf2);
    border-color: var(--v-line2);
    color: var(--v-faint);
  }

  .sg-pages {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    margin-top: 14px;
    font-size: 12px;
    color: var(--v-faint);
  }
  .sg-pages button {
    padding: 6px 12px;
    border-radius: var(--v-r-md);
    background: var(--v-surf2);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .sg-pages button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .sg-pages button:hover:not(:disabled) {
    color: var(--v-txt);
  }
</style>
