<script>
  // A FIELD WELL: a bordered box that holds an input and whatever sits beside it.
  //
  // It renders `.r-well`, which app.css publishes with the measurement behind it.
  // The short version: four search boxes in this tree each invented their own focus
  // treatment after switching the input's own outline off, and one of the four
  // measures 2.28:1 against its own ground where WCAG 2.2 asks 3:1. The class was
  // already named in app.css's focus group, so it already had a compliant outline
  // promised to it and no rule body to hang it on.
  //
  // ── IT DOES NOT OWN THE INPUT ───────────────────────────────────────────────
  //
  // The `<input>` stays in the caller's markup and arrives through the default
  // slot. That is deliberate and it is the difference between this being adoptable
  // and being a rewrite: every one of these fields has a `bind:value`, a
  // `placeholder`, an `on:input`, an `on:keydown` and often a `bind:this` that the
  // view needs. A component that owned the element would have to forward all of
  // them, would get one wrong, and would change what a control does. The rule here
  // is "behaviour must not change".
  //
  // `.r-well > input` in app.css strips the input's own box and hands its outline
  // to the well's `:focus-within`, so the caller's element needs no class at all.
  //
  // A label is optional because most of these are search boxes with a placeholder
  // and a magnifier glyph. When one is given it is rendered as a real `<label>`
  // above the well, because a placeholder is not a label: it disappears the moment
  // somebody types, which is exactly when a volunteer looks up to check what they
  // are filling in.

  /** Visible label. Omit for a search box that already says what it is. */
  export let label = '';
  /** `for`/`id` pairing. Only used when `label` is set. */
  export let id = '';
  /** A class for POSITION: flex, width, max-width. Never the box. */
  let klass = '';
  export { klass as class };
</script>

{#if label}
  <label class="fl" for={id || undefined}>{label}</label>
{/if}
<div class="r-well {klass}" {...$$restProps}>
  <slot name="icon" />
  <slot />
  {#if $$slots.tail}
    <span class="r-well-tail"><slot name="tail" /></span>
  {/if}
</div>

<style>
  /* The label is the type scale's label step, and it is the only thing this
     component draws for itself. Everything else is `.r-well` in app.css, so
     adopting this cannot restyle a field in a way the stylesheet does not say. */
  .fl {
    display: block;
    margin-bottom: 5px;
    font-size: var(--v-fs-lbl);
    font-weight: 500;
    color: var(--v-dim);
  }
</style>
