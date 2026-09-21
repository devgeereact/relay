<script>
  // THE HOUSE SWITCH, as a component rather than as a class somebody remembered.
  //
  // `Button` closed half of one gap on 2026-09-18: a control that greys itself out
  // and says nothing. Nineteen hand-rolled `.r-btn`s in Settings went through the
  // component and every one of them now carries its reason. **The switches could
  // not follow, because there was nothing for them to go through** — `src/app.css`
  // publishes `.r-switch` and `src/lib/ui/` published no `Switch` (RG-168).
  //
  // The measured instance is **Send crash reports** in Settings → Privacy:
  // `disabled={!$capture.available || !!crashReadFailed}` on a raw
  // `<button class="r-switch">` with no `title` and no `aria-describedby`. The
  // `crashReadFailed` half was explained by a rose `[role="alert"]` beside it; the
  // `!$capture.available` half was explained by nothing at all, so the one control
  // that decides whether anything leaves this machine sat at 45% opacity in
  // silence.
  //
  // ── WHAT IT OWNS THAT A HAND-ROLLED SWITCH DID NOT ──────────────────────────
  //
  // A switch is not a button, and three call sites each had to remember that: the
  // `role`, the `aria-checked` that makes the role mean anything, and the
  // `aria-label` a control with no text of its own cannot do without. Remembering
  // is what a component is for. `aria-checked` is written from the same `checked`
  // that draws the `on` class, so the picture and the announcement cannot disagree
  // — which is the shape of the status-badge rule (CLAUDE.md rule 35) at the size
  // of one control.
  //
  // ── WHAT IT DELIBERATELY DOES NOT DO ────────────────────────────────────────
  //
  // It invents no look. It renders `.r-switch` and `.on`, both of which app.css
  // has published for several waves, and it accepts no width, height, radius or
  // colour: the switch is 38×21 everywhere in this product (app.css's control
  // ladder) and docs/REBRAND.md §12's one instrument is the whole point of it.
  // Adopting it on a call site that was already correct is a no-op in the rendered
  // box; adopting it on one that was hand-rolled is what it is for.
  //
  // It also does not own the WORD beside it. Every switch in Settings sits next to
  // an `on`/`off` value that says what the position means, and that word is read
  // from the same store the switch throws from. A component that rendered its own
  // label would give the page two answers to one question.

  /** Which way it is thrown. Drives the `on` class AND `aria-checked`, together. */
  export let checked = false;
  /**
   * The accessible name. REQUIRED in practice: a switch renders no text, so
   * without it a screen reader announces "switch" and nothing else — the same
   * dead control `IconButton` exists to prevent.
   */
  export let label = '';
  export let disabled = false;
  /** A class for POSITION only: margin, flex. Never a box property. */
  let klass = '';
  export { klass as class };

  /**
   * WHY A DISABLED SWITCH MUST SAY WHY — the same argument `Button` makes, and the
   * same two channels, because neither reaches everybody. `title` is the pointer
   * answer and is invisible to a keyboard or screen-reader operator;
   * `aria-describedby` is the assistive answer and is invisible to a mouse. The
   * text rides on a `<span class="sr-only">` rather than a `hidden` element,
   * because a hidden element has no accessible name and the attribute would then
   * point at nothing — the same failure as a citation that resolves to silence.
   *
   * Rendered only while the switch is actually disabled: a reason shown over a
   * working control is a tooltip about a problem that does not exist.
   *
   * The sentences come from `whydisabled.js`, never from the call site. Sixteen
   * controls each writing their own sentence about `!$capture.available` is
   * sixteen descriptions of one fact, and they would not agree.
   */
  export let disabledReason = '';

  // A stable id per instance. `Math.random` rather than a module counter, for the
  // reason `Button` records: a counter is shared across test files in one vitest
  // worker, so two components in two suites can agree on an id and the second
  // one's `aria-describedby` then resolves to the first one's span.
  const rid = `rs-${Math.random().toString(36).slice(2, 9)}`;
  $: showReason = disabled && !!disabledReason;
  // The spread goes FIRST and the computed attributes after it, so a caller's own
  // `title` can never outrank the reason a disabled control owes the operator.
  // Svelte applies a spread in source order; the other way round, the one
  // attribute this component exists to guarantee would be the easiest to lose.
</script>

<button
  {...$$restProps}
  type="button"
  {disabled}
  class="r-switch {klass}"
  class:on={checked}
  role="switch"
  aria-checked={checked}
  aria-label={label}
  title={showReason ? disabledReason : $$restProps.title}
  aria-describedby={showReason ? rid : $$restProps['aria-describedby']}
  on:click
  on:keydown
></button>
{#if showReason}
  <span class="sr-only" id={rid}>{disabledReason}</span>
{/if}
