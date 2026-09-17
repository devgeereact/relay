<script>
  // THE ORDINARY BUTTON, as a component rather than as a class somebody remembered.
  //
  // `src/app.css` has published the house button for several waves and the shape of
  // the problem never moved: a class is opt-in, and 146 of the 352 buttons in this
  // tree opt out. `buttonshapes.test.js` and `workspacegrammar.test.js` between them
  // hold what happens once a button HAS reached for the shared control, which is the
  // right half to police mechanically and is not the half an operator complains
  // about. The complaint is the row that steps, and a row steps because one of its
  // members was hand-rolled.
  //
  // So this exists to make the shared control the path of least resistance rather
  // than a thing to look up. It renders `.r-btn` and the variant classes app.css
  // already publishes. It invents no look: adopting it on a call site that was
  // already correct must be a no-op in the rendered box, and adopting it on one that
  // was hand-rolled is the whole point.
  //
  // ── WHAT IT DELIBERATELY DOES NOT DO ────────────────────────────────────────
  //
  // It does not accept a height, a radius, a fill or a font. Those are the six
  // properties `buttonshapes.test.js` calls THE BOX, and the shared control owns
  // every one of them (docs/REBRAND.md §1: "a component may override a shared
  // control's width and padding, never its height"). A `class` prop is forwarded, so
  // a caller can still place the button in its row, and the existing scanners keep
  // judging whatever it puts there.
  //
  // It is also not the only legitimate way to draw a button. A slide card, a
  // favourite star, a kebab and a menu row are native `<button>`s that are not
  // buttons, and forcing them through here would change how those surfaces read.
  // `buttonshapes.test.js` already pins nine of them by name; that ruling stands.

  /**
   * One of the fills `app.css` publishes, or '' for the bare house surface.
   *
   * `amber` is here because app.css publishes it and leaving it out would send the
   * on-air case back to a hand-rolled class. It is RESERVED: use it only for a
   * control that puts something in front of a congregation or takes it away
   * (CLAUDE.md rule 18, DESIGN_SYSTEM §1). Naming it at the call site is what keeps
   * reaching for it a decision instead of a default.
   */
  export let variant = '';
  /** 'md' (26px) or 'sm' (22px). The two steps the ladder publishes, and no others. */
  export let size = 'md';
  export let disabled = false;
  export let type = 'button';
  /** A class for POSITION only: width, flex, margin. Never a box property. */
  let klass = '';
  export { klass as class };

  /**
   * WHY A DISABLED BUTTON MUST SAY WHY, AND WHY IT IS A PROP RATHER THAN A HABIT.
   *
   * Measured on this tree: 27 buttons are gated on `!$capture.available`,
   * `$safeMode` or `$serviceLock`, and 16 of them say nothing at all. One of the 16
   * is `Clear screens`. A volunteer mid-service sees the single control the whole
   * product is arranged around sitting at 45% opacity with no explanation, and the
   * reasonable conclusion from that is that Relay has crashed, which is the worst
   * available conclusion at that moment and is also false.
   *
   * The reason goes to BOTH channels because neither reaches everybody. `title` is
   * the pointer answer and is invisible to a keyboard or screen-reader operator;
   * `aria-describedby` is the assistive answer and is invisible to a mouse. A
   * `<span class="sr-only">` carries the text for the second, because a `hidden`
   * element has no accessible name and would have made the attribute point at
   * nothing, which is the same failure as a dead citation.
   *
   * It is rendered only while the button is actually disabled: a reason shown over a
   * working control is a tooltip explaining a problem that does not exist.
   */
  export let disabledReason = '';

  // A stable id per instance. `Math.random` is deliberate rather than a counter: a
  // module-level counter is shared across test files in one vitest worker, so two
  // components in two suites can agree on an id and the second one's
  // `aria-describedby` then resolves to the first one's span.
  const rid = `rb-${Math.random().toString(36).slice(2, 9)}`;
  $: showReason = disabled && !!disabledReason;
  // The spread goes FIRST in the markup and the computed attributes after it, so a
  // caller's `title` can never outrank the reason a disabled control owes the
  // operator. Svelte applies a spread in source order, so the other way round the
  // one attribute this component exists to guarantee would be the easiest to lose.
</script>

<button
  {...$$restProps}
  {type}
  {disabled}
  class="r-btn {variant} {size === 'sm' ? 'sm' : ''} {klass}"
  title={showReason ? disabledReason : $$restProps.title}
  aria-describedby={showReason ? rid : $$restProps['aria-describedby']}
  on:click
  on:pointerdown
  on:keydown
>
  <slot name="icon" />
  <slot />
</button>
{#if showReason}
  <span class="sr-only" id={rid}>{disabledReason}</span>
{/if}
