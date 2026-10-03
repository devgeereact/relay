<script>
  // THE CHAPTER PICKER — one grid, mounted by Live's rail and by the Library.
  //
  // It was Live's alone, and the Library offered a `<select>` of "Chapter 1 …
  // Chapter 150" instead: the same job in a shape that cannot do it. Finding 119
  // of 150 in a dropdown is scrolling a list and reading every entry on the way.
  //
  // ── THE RULES, ALL OF WHICH WERE LEARNED IN THE RAIL ──────────────────────
  //
  // **A FIXED GRID, NOT A WRAP.** These were a `flex-wrap` of chips sized by
  // their own labels, so a one-digit chapter and a two-digit one drew different
  // boxes, every row held a different count, and no column lined up with the one
  // above it. `auto-fill` + `1fr` gives every cell the same width at any rail
  // width, so the rows line up and a chapter can be found by counting columns.
  //
  // **A CHAPTER CHIP IS NOT A BUTTON-IN-A-ROW**, which is why it is not `.r-btn`:
  // it is a fixed square cell in a numeric picker, sized by the grid rather than
  // by its label, and `.r-btn`'s 11px of side padding is the exact property that
  // made these ragged. It draws `.r-btn`'s rest and hover fills so it still
  // belongs to the same family.
  //
  // **BOUNDED.** Psalms has 150 chapters — 25 rows — and inline in a scroller
  // that pushed the 47 books after it clean off the column. The picker scrolls
  // itself; the list it came from stays where the operator left it.
  //
  // **THE MARK IS STEEL.** `aria-current` says which chapter this picker last
  // opened. Never amber, which means ON AIR, and never cyan, which means the AI
  // guessed: opening a chapter is not a claim about any screen (rule 18).
  //
  // ── WHAT IT DELIBERATELY DOES NOT DECIDE ──────────────────────────────────
  //
  // What a press MEANS. Live opens a chapter into the slide grid and touches no
  // screen; the Library moves the pane to that chapter. Both are "open this
  // chapter here" and neither is a fire, but the sentence a surface prints about
  // it is its own — `caption` — because one legend over two meanings is the line
  // that reads the same whether or not a congregation is looking at something.

  /** The book these chapters belong to; used for the accessible names. */
  export let book = '';
  /** How many chapters it has. */
  export let count = 0;
  /** The chapter this picker last opened, or null for none. */
  export let current = null;
  /** Nothing is pressable — a rail with no backend, a safe-mode surface. */
  export let disabled = false;
  /** What a press does here, said where the press happens. Omit for none. */
  export let caption = '';
  /**
   * The `title` for a chapter that is not the current one.
   *
   * A function, and for the same reason `caption` is a prop: the sentence is the
   * surface's, not the picker's. Live's chips each say *"nothing reaches a
   * screen"* — a per-chip guarantee, not decoration, because the caption above
   * them scrolls out of the picker's own 168px box and the promise has to be
   * readable on the thing being pressed.
   */
  export let openTitle = (b, c) => `Open ${b} ${c}`;
  /** Called with the chapter NUMBER. The one thing this component produces. */
  export let onPick = () => {};

  const list = (n) => Array.from({ length: Math.max(0, n) }, (_, i) => i + 1);
</script>

<div class="cp">
  {#if caption}
    <p class="cp-cap r-mono">{caption}</p>
  {/if}
  <div class="cp-chips">
    {#each list(count) as c}
      <button
        class="cp-chip"
        {disabled}
        aria-current={current === c ? 'true' : undefined}
        title={current === c ? `${book} ${c} — the chapter you opened from here` : openTitle(book, c)}
        aria-label={`${book} chapter ${c}`}
        on:click={() => onPick(c)}>{c}</button>
    {/each}
  </div>
</div>

<style>
  .cp { padding: 2px 8px 8px; }
  .cp-cap {
    margin: 0; padding: 2px 0 4px;
    font-size: var(--v-fs-cap); letter-spacing: var(--v-tr-caps);
    text-transform: uppercase; color: var(--v-faint);
  }
  .cp-chips {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(26px, 1fr));
    gap: 3px;
    max-height: 168px; overflow-y: auto;
    scrollbar-width: thin; scrollbar-color: var(--v-surf3) transparent;
  }
  .cp-chips::-webkit-scrollbar { width: 6px; }
  .cp-chips::-webkit-scrollbar-thumb { background: var(--v-surf3); border-radius: var(--v-r-round); }
  .cp-chip {
    display: grid; place-items: center;
    width: 100%; height: 24px; padding: 0;
    border-radius: var(--v-r-sm); cursor: pointer;
    background: var(--v-surf2); border: 1px solid var(--v-500); color: var(--v-dim);
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
    /* Tabular figures, or 1 and 11 sit at different optical centres inside cells
       that are finally the same size. */
    font-variant-numeric: tabular-nums;
  }
  .cp-chip:hover:not(:disabled) { background: var(--v-surf3); border-color: var(--v-sel-line); color: var(--v-txt); }
  .cp-chip:disabled { opacity: .5; cursor: not-allowed; }
  .cp-chip[aria-current='true'] {
    background: var(--v-sel-soft); border-color: var(--v-sel-line);
    color: var(--v-txt); font-weight: 600;
  }
</style>
