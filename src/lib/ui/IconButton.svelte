<script>
  // THE ICON-ONLY BUTTON, and the 4px step that was missing from the ladder.
  //
  // This is the component that fixes a misalignment rather than merely preventing
  // one, so the mechanism is worth stating plainly.
  //
  // docs/REBRAND.md §1 publishes two button heights, 26 and 22, and app.css carries
  // both for the text button (`.r-btn`, `.r-btn.sm`). It carries only ONE for the
  // icon button: `.r-iconbtn` is 26x26 and there was no small step at all. Earlier
  // waves then did exactly the right thing and stripped the local size overrides off
  // every icon button that had one, each with its reason recorded in the file it was
  // taken from (`Browse.svelte`'s `.br-fav`, `Library.svelte`'s `.lib-more`). That
  // was correct and it made the gap permanent: every icon button in the product is
  // now 26px, including the ones sitting in rows of 22px small buttons, and the 4px
  // step cannot be closed at a call site any more without reintroducing the override
  // the sweep removed.
  //
  // So the step is published here, once, as `.r-iconbtn.sm` in app.css, and this
  // component is how a caller asks for it. There is no third size and there must not
  // be: the thing the ladder protects is a COLUMN, and a height chosen one control at
  // a time is what made the column step in the first place.
  //
  // ── A LABEL IS NOT OPTIONAL ─────────────────────────────────────────────────
  //
  // An icon-only button has no accessible name unless somebody gives it one, and
  // `qa-inventory.mjs` reports unnamed controls for exactly this reason. `label` is
  // required, it becomes `aria-label`, and it also becomes the `title` so a volunteer
  // who does not recognise a glyph can hover it. Passing a `title` explicitly wins,
  // because a keyboard hint ("Undo (Ctrl/Cmd+Z)") is a better tooltip than the bare
  // name and a worse accessible name.

  /** The accessible name. Required: an unnamed icon button is a dead control. */
  export let label = '';
  /** 'md' (26x26) or 'sm' (22x22). */
  export let size = 'md';
  export let disabled = false;
  export let type = 'button';
  /** A class for POSITION only: margin, flex, grid placement. Never a box property. */
  let klass = '';
  export { klass as class };
  /** See Button.svelte: a disabled control owes the operator a reason. */
  export let disabledReason = '';

  const rid = `ib-${Math.random().toString(36).slice(2, 9)}`;
  $: showReason = disabled && !!disabledReason;
  // The spread goes FIRST and the computed attributes after it, so a caller's
  // `title` can never outrank the reason a disabled control owes the operator.
  // Svelte applies a spread in source order, so the other way round the one
  // attribute this component exists to guarantee would be the easiest to lose.
  $: tip = showReason ? disabledReason : $$restProps.title || label || undefined;
</script>

<button
  {...$$restProps}
  {type}
  {disabled}
  class="r-iconbtn {size === 'sm' ? 'sm' : ''} {klass}"
  aria-label={label || undefined}
  title={tip}
  aria-describedby={showReason ? rid : $$restProps['aria-describedby']}
  on:click
  on:pointerdown
  on:keydown
>
  <slot />
</button>
{#if showReason}
  <span class="sr-only" id={rid}>{disabledReason}</span>
{/if}
