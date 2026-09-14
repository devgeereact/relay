<script>
  /**
   * THE COLLECTION RAIL — the Library's top row (REBRAND §10).
   *
   * Four square, colour-coded collections with counts, and beneath them the
   * views that collection holds (only where it holds more than one — a row of
   * one button is a label pretending to be a choice).
   *
   * WHAT THE COLOUR IS ALLOWED TO DO. It tints the 22px icon tile and nothing
   * else. Amber on this console means ON AIR, red means destructive, amethyst
   * means rehearsal; a collection wearing one of those as a filled badge would
   * be a status claim about a shelf of content, which is exactly the class of
   * lie rule 35 exists to stop. Selection is steel blue, as it is everywhere
   * else, and it is carried by the chip's border and ground — never by the
   * collection colour, so the two can never be confused for one another.
   */
  import { COLLECTIONS, countMark, countWords } from './collections.js';

  /** The collection key currently open. */
  export let collection = 'scripture';
  /** The view key currently rendering, inside that collection. */
  export let view = 'browse';
  /** `{ [collectionKey]: number | null }` — `null` means "not loaded", a
      negative number means the query failed. Never coerce either to 0. */
  export let counts = {};
  export let onCollection = () => {};
  export let onView = () => {};

  $: open = COLLECTIONS.find((c) => c.key === collection) ?? COLLECTIONS[0];
</script>

<div class="cr">
  <div class="cr-row" role="tablist" aria-label="Collections">
    {#each COLLECTIONS as c (c.key)}
      {@const n = counts[c.key]}
      <button
        type="button"
        class="cr-c r-focus cc-{c.colour}"
        class:on={collection === c.key}
        role="tab"
        aria-selected={collection === c.key}
        on:click={() => onCollection(c.key)}>
        <span class="cr-i" aria-hidden="true">
          {#if c.key === 'scripture'}
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M4 4h7a2 2 0 0 1 2 2v14a2 2 0 0 0-2-2H4z" /><path d="M20 4h-7a2 2 0 0 0-2 2v14a2 2 0 0 1 2-2h7z" /></svg>
          {:else if c.key === 'songs'}
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M9 18V5l11-2v13" /><circle cx="6" cy="18" r="3" /><circle cx="17" cy="16" r="3" /></svg>
          {:else if c.key === 'notices'}
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><path d="M3 10v4h4l6 4V6l-6 4z" /><path d="M17 9a4 4 0 0 1 0 6" /></svg>
          {:else}
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="1" /><path d="m3 16 5-5 4 4 3-3 6 6" /><circle cx="9" cy="9" r="1.4" /></svg>
          {/if}
        </span>
        <span class="cr-n">{c.label}</span>
        <span class="cr-k r-mono" aria-hidden="true">{countMark(n)}</span>
        <span class="sr-only">{countWords(c, n)}</span>
      </button>
    {/each}
  </div>

  <!-- The views inside the open collection. Rendered only when there is a real
       choice: scripture is a Bible you read AND the verses you saved, media is
       the moving half and the still half. The other two hold one pane each. -->
  {#if open.views.length > 1}
    <div class="cr-views" role="tablist" aria-label="{open.label} views">
      {#each open.views as v (v.key)}
        <button
          type="button"
          class="cr-v r-focus"
          class:on={view === v.key}
          role="tab"
          aria-selected={view === v.key}
          on:click={() => onView(v.key)}>{v.label}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .cr { display: flex; flex-direction: column; gap: 8px; min-width: 0; }
  .cr-row { display: flex; gap: 8px; flex-wrap: wrap; }

  /* Square-shouldered, 3px like everything else on this desk. No pills. */
  .cr-c {
    display: inline-flex; align-items: center; gap: 8px;
    height: 34px; padding: 0 11px 0 6px;
    border: 1px solid var(--v-line2); border-radius: var(--v-r-sm);
    background: var(--v-surf2); color: var(--v-dim);
    font-family: var(--f-body); font-size: var(--v-fs-lbl, 11px); font-weight: 600;
    letter-spacing: .01em; cursor: pointer;
    transition: background var(--v-dur, .12s) var(--v-ease, ease), border-color var(--v-dur, .12s) var(--v-ease, ease);
  }
  .cr-c:hover:not(.on) { background: var(--v-surf3); color: var(--v-txt); }
  /* SELECTION IS STEEL, always and only. */
  .cr-c.on { border-color: var(--v-sel-line); background: var(--v-sel-soft); color: var(--v-txt); }

  /* The one place a collection colour is allowed to land: a small square tile.
     Soft ground, full-strength glyph — a tint, not a badge. */
  .cr-i {
    display: grid; place-items: center; width: 22px; height: 22px;
    border-radius: 2px; border: 1px solid var(--cc-line); background: var(--cc-soft); color: var(--cc);
    flex: 0 0 auto;
  }
  .cc-scripture { --cc: var(--v-col-scripture); --cc-soft: var(--v-amber-soft); --cc-line: var(--v-amber-line); }
  .cc-song      { --cc: var(--v-col-song);      --cc-soft: var(--v-sel-soft);   --cc-line: var(--v-sel-line); }
  .cc-notice    { --cc: var(--v-col-notice);    --cc-soft: var(--v-red-soft);   --cc-line: var(--v-red-line); }
  .cc-media     { --cc: var(--v-col-media);     --cc-soft: var(--v-amethyst-soft); --cc-line: var(--v-amethyst-line); }

  .cr-n { white-space: nowrap; }
  /* Mono, so a count that changes never reflows the name beside it. It sits on
     --v-surf3, so it uses --v-dim: --v-faint on --v-surf3 is below WCAG AA and
     `tokencontrast.test.js` fails the build for it. */
  .cr-k {
    min-width: 20px; padding: 0 5px; height: 16px; line-height: 16px; text-align: center;
    border-radius: 2px; background: var(--v-surf3); color: var(--v-dim);
    font-size: 10px; flex: 0 0 auto;
  }

  .cr-views { display: flex; gap: 4px; flex-wrap: wrap; }
  .cr-v {
    height: 22px; padding: 0 9px; border: 1px solid transparent; border-radius: var(--v-r-sm);
    background: transparent; color: var(--v-dim);
    font-family: var(--f-body); font-size: 11px; cursor: pointer;
  }
  .cr-v:hover:not(.on) { color: var(--v-txt); background: var(--v-surf2); }
  .cr-v.on { color: var(--v-txt); background: var(--v-surf3); border-color: var(--v-line2); }
</style>
