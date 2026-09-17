<script>
  // ONE ROW OF A POPOVER MENU.
  //
  // It renders `.r-menuitem`, which app.css publishes with the measurement behind
  // it: five row paddings, four font sizes and three radii existed across the six
  // menus in this tree, and four of the six already agreed. This is that majority,
  // asked for by name.
  //
  // `role="menuitem"` because the shell is a `role="menu"` and a menu whose
  // children are plain buttons is a menu to the eye and a group of buttons to a
  // screen reader. It stays a native `<button>` underneath, so Enter, Space, focus
  // and `:disabled` all work without being reimplemented.
  //
  // It is NOT `.r-btn`, and that is deliberate rather than an omission:
  // `buttonshapes.test.js` already rules that a menu row is "not a button" and
  // names nine of them. Forcing one into the shared control would centre its label
  // and draw a hairline round every row, which is a different control on a surface
  // that is meant to read as a list.

  export let disabled = false;
  /** `danger` for the one destructive row. Same red as `.r-btn.danger`. */
  export let variant = '';
  /** A class for POSITION or for content layout inside the row. Never the box. */
  let klass = '';
  export { klass as class };
  /** See Button.svelte: a disabled control owes the operator a reason. */
  export let disabledReason = '';

  const rid = `mi-${Math.random().toString(36).slice(2, 9)}`;
  $: showReason = disabled && !!disabledReason;
</script>

<button
  {...$$restProps}
  type="button"
  role="menuitem"
  {disabled}
  class="r-menuitem {variant} {klass}"
  title={showReason ? disabledReason : $$restProps.title}
  aria-describedby={showReason ? rid : $$restProps['aria-describedby']}
  on:click
  on:keydown
>
  <slot />
</button>
{#if showReason}
  <span class="sr-only" id={rid}>{disabledReason}</span>
{/if}
