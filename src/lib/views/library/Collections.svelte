<script>
  /**
   * THE COLLECTION RAIL — the Library's top row (REBRAND §10).
   *
   * Four square, colour-coded collections with counts, and beneath them the
   * views that collection holds (only where it holds more than one — a row of
   * one button is a label pretending to be a choice).
   *
   * WHAT THE COLOUR IS ALLOWED TO DO. It paints ONE 3px left edge of the chip
   * and nothing else — the form REBRAND §1 and §10 ask for, and the form the
   * prototype uses. Amber on this console means ON AIR, red means destructive,
   * amethyst means rehearsal; `--v-col-scripture` IS `--v-amber`, so a chip
   * whose ground went amber-soft would be the on-air wash, at chip size, on a
   * shelf of content that is not on air. That is exactly the class of lie rule
   * 35 exists to stop.
   *
   * So this deviates from the prototype in two places, deliberately, and
   * CLAUDE.md wins where the two disagree (rule 18, DECISIONS §21): the
   * prototype fills a pressed chip with the collection colour and writes the
   * chip's text in it. Relay does neither. Selection is steel blue, as it is
   * everywhere else, carried by the chip's other three borders and its ground —
   * and the left edge is restated afterwards, because an open collection is the
   * one chip that must still say WHICH collection it is.
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

  <!-- The views inside the open collection, ON THE SAME LINE as the collections.
       They used to sit on a row of their own, and that row was the second of
       FOUR stacked above the first item an operator could see. A choice between
       two panes of one collection is a refinement of the chip beside it, not a
       tier of navigation — so it reads as one, the way a breadcrumb does.

       Rendered only when there is a real choice: scripture is a Bible you read
       AND the verses you saved, media is the moving half and the still half. The
       other two hold one pane each. -->
  {#if open.views.length > 1}
    <span class="cr-sep" aria-hidden="true"></span>
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
  /* ONE ROW. Collections, then a hairline, then the views inside the open one. */
  .cr { display: flex; align-items: center; gap: 8px; min-width: 0; flex-wrap: wrap; }
  .cr-row { display: flex; gap: 8px; flex-wrap: wrap; }
  .cr-sep { width: 1px; height: 20px; background: var(--v-line2); flex: 0 0 auto; }

  /* Square-shouldered, 3px radius like everything else on this desk. No pills. */
  .cr-c {
    display: inline-flex; align-items: center; gap: 7px;
    height: 34px; padding: 0 11px 0 9px;
    border: 1px solid var(--v-line2); border-radius: var(--v-r-sm);
    /* THE COLLECTION COLOUR, AND THE WHOLE OF ITS SPEND. Declared after the
       shorthand above, which would otherwise reset it. */
    border-left: 3px solid var(--cc);
    background: var(--v-surf2); color: var(--v-dim);
    font-family: var(--f-body); font-size: var(--v-fs-lbl, 11px); font-weight: 600;
    letter-spacing: .01em; cursor: pointer;
    transition: background var(--v-dur, .12s) var(--v-ease, ease), border-color var(--v-dur, .12s) var(--v-ease, ease);
  }
  .cr-c:hover:not(.on) { background: var(--v-surf3); color: var(--v-txt); }
  /* SELECTION IS STEEL, always and only — and `border-color` is four-sided, so
     the collection's own edge is restated after it. Without that line the open
     chip is the one chip that has stopped saying which collection it is. */
  .cr-c.on {
    border-color: var(--v-sel-line); border-left-color: var(--cc);
    background: var(--v-sel-soft); color: var(--v-txt);
  }

  .cc-scripture { --cc: var(--v-col-scripture); }
  .cc-song      { --cc: var(--v-col-song); }
  .cc-notice    { --cc: var(--v-col-notice); }
  .cc-media     { --cc: var(--v-col-media); }

  /* The glyph is a recognition aid, not a second colour. It takes the chip's
     own ink, which is what changes when the chip opens. */
  .cr-i { display: grid; place-items: center; width: 14px; height: 14px; flex: 0 0 auto; }

  .cr-n { white-space: nowrap; }
  /* Mono, so a count that changes never reflows the name beside it. Bare, on the
     chip's own --v-surf2 ground: a badge around a number on a square chip is the
     pill §1 spent a paragraph removing. A count is a fact the operator reads,
     so it is --v-dim, not --v-faint. */
  .cr-k { color: var(--v-dim); font-size: 10px; flex: 0 0 auto; }

  .cr-views { display: flex; gap: 4px; flex-wrap: wrap; }
  .cr-v {
    height: 22px; padding: 0 9px; border: 1px solid transparent; border-radius: var(--v-r-sm);
    background: transparent; color: var(--v-dim);
    font-family: var(--f-body); font-size: 11px; cursor: pointer;
  }
  .cr-v:hover:not(.on) { color: var(--v-txt); background: var(--v-surf2); }
  .cr-v.on { color: var(--v-txt); background: var(--v-surf3); border-color: var(--v-line2); }
</style>
