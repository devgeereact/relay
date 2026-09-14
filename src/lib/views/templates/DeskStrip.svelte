<script context="module">
  /** The desks the Templates workspace holds, in strip order. Exported so the
   *  workspace root and its tests name them from here rather than retyping the
   *  list — a third copy is how a desk gets dropped from one and not the other. */
  export const DESKS = [
    { key: 'templates', label: 'Templates' },
    { key: 'themes', label: 'Themes' },
  ];
</script>

<script>
  // THE DESK STRIP — how one workspace holds more than one desk.
  //
  // Themes stopped being a workspace of its own (docs/REBRAND.md §2: the shell's
  // strip is the six workspaces, and Themes is not one of them). It is the style
  // layer BENEATH templates — a theme sets default `style` keys and a template
  // overrides them key by key (DECISIONS §27) — so it belongs inside the
  // Templates workspace rather than beside it.
  //
  // ONE COMPONENT, TWO DESKS. Both desks render this, so the strip they show is
  // the same strip: a segmented control cannot offer a desk on one side and not
  // on the other, and a desk cannot be dropped from one copy and survive in the
  // other. Two hand-rolled copies four lines apart is the shape of the `Copy URL`
  // defect the rebrand's own notes record.
  //
  // NOT A STATUS. Steel blue is "the thing you are working on" and that is
  // exactly what a chosen desk is, so `.r-seg`'s own selected treatment is
  // correct here and no promised colour is borrowed.
  import { createEventDispatcher } from 'svelte';

  /** The desk currently showing. */
  export let desk = 'templates';

  const dispatch = createEventDispatcher();
</script>

<div class="r-seg ds-strip" role="tablist" aria-label="Templates workspace desks">
  {#each DESKS as d (d.key)}
    <button
      role="tab"
      aria-selected={desk === d.key}
      class:on={desk === d.key}
      on:click={() => desk !== d.key && dispatch('desk', { desk: d.key })}>{d.label}</button>
  {/each}
</div>

<style>
  /* It sits at the LEFT of the head slot, before Import / New — the desks are
     what this workspace IS, and the buttons are what you can do on the one you
     are looking at. A separator keeps the two groups from reading as one row of
     five equal controls. */
  .ds-strip{ flex:0 0 auto; margin-right:var(--v-sp-sm); }
</style>
