<script>
  // A ROW OF CONTROLS THAT CANNOT STEP.
  //
  // This is the smallest component in the set and it is the one aimed straight at
  // the complaint that started this pass, which was "buttons don't align together".
  //
  // docs/REBRAND.md §1 gives the reason a ladder exists at all, and it is worth
  // quoting because it is not the obvious one: "one height per control, because the
  // thing being protected is a COLUMN. A rail stacks a select, an input, a switch
  // and a button in one list, and four heights picked one control at a time make
  // that column step in and out by two pixels a row." The same is true sideways. A
  // pane head with a 26px button, a 24px search well and a 22px segment strip in it
  // does not read as three deliberate sizes; it reads as a row that failed to line
  // up, and an operator cannot name why.
  //
  // So this fixes ONE height for the row and states it, rather than each member
  // being separately correct and collectively ragged.
  //
  // ── WHAT IT DOES AND DOES NOT DO ────────────────────────────────────────────
  //
  // It sets `align-items:center` and publishes the row's height as a CSS custom
  // property, `--tb-h`, which members may read. It does NOT reach into its children
  // and force a height on them: a `:global` rule that overrode `.r-btn`'s height
  // from a wrapper would be exactly the override `workspacegrammar.test.js` forbids
  // a component from writing, and it would beat the shared control from a file the
  // next person will not think to look in. The ladder wins; this is a row that
  // agrees with it and says so.
  //
  // Alignment on the cross axis is the half that can be fixed from here and is the
  // half that was actually broken: a taller member used to stretch its shorter
  // neighbours, because `align-items` defaults to `stretch`.

  /** 'md' (26px members) or 'sm' (22px members). The two steps the ladder has. */
  export let size = 'md';
  /** Gap between members, in px. 8 is `--v-sp-sm` and is the house default. */
  export let gap = 8;
  /** Push everything after this point to the right edge. */
  export let spread = false;
  /** A class for POSITION: margin, flex, grid placement. */
  let klass = '';
  export { klass as class };

  $: h = size === 'sm' ? 22 : 26;
</script>

<div
  class="tb {klass}"
  class:spread
  style="--tb-h:{h}px; --tb-gap:{gap}px"
  {...$$restProps}
>
  <slot />
</div>

<style>
  .tb {
    display: flex;
    /* The whole point. `stretch` is the default and it is what let one tall member
       pull its neighbours out of shape. */
    align-items: center;
    gap: var(--tb-gap);
    min-height: var(--tb-h);
    flex-wrap: wrap;
  }
  /* A spring, so a row can put its last group on the right edge without each
     caller inventing a `margin-left:auto` on a different child. */
  .tb.spread :global(.tb-spring) {
    flex: 1;
  }
</style>
