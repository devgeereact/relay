<script>
  // THE POPOVER MENU, and the one place rule 44's Escape contract is written.
  //
  // There are six popover menus in this tree and the shell each one draws is in
  // `app.css` as `.r-menu` now, with the measurement that justified it. This
  // component is the other half, and it is the more important half, because the
  // thing being consolidated is not a corner radius.
  //
  // ── RULE 44 ─────────────────────────────────────────────────────────────────
  //
  // `shortcuts.js` stands down for a mounted `[role="dialog"|"alertdialog"|"menu"|
  // "listbox"]`, correctly, per rule 16: dismissing a menu is not a live action and
  // must not wipe the congregation's screens. The consequence is that MOUNTING one
  // takes the operator's panic key, and an overlay that takes the panic key owes
  // them an outcome for it. Four template menus once bound nothing at all under a
  // comment claiming Escape was "handled globally"; it is not, `shortcuts.js`
  // RETURNS, and the key reached nobody.
  //
  // That is fixed everywhere in this tree already and `panicoverlay.test.js`
  // enumerates the whole component tree to keep it fixed. What was NOT fixed is
  // that the contract exists in four separate copies (`Library.svelte`'s `menuEsc`,
  // `VerseDeck.svelte`'s `menuEsc`, and an `onKey` each in `TemplateGallery` and
  // `TemplateEditor`). Four copies of a guarantee is how three of them come to
  // disagree, which is the shape CLAUDE.md records under "a guarantee is only kept
  // on the doors you checked".
  //
  // The contract, in full:
  //
  //   · Escape closes THIS menu and goes no further. `stopPropagation` so the
  //     dismissal never reaches the shell, `preventDefault` so it is not also a
  //     browser gesture. One press dismisses the menu and nothing else.
  //   · The SECOND press is the operator's. By then this is unmounted,
  //     `shortcuts.js` has the key back, and Escape clears the screens.
  //   · Every other key is returned untouched. Space still means advance (rule 11)
  //     and a focused row still activates on it, because this handler does nothing
  //     at all unless the key is Escape. The old comment's reason for binding
  //     nothing — that a handler "would have to stopPropagation, which would
  //     swallow Space" — did not follow, and cost four menus their outcome.
  //
  // ── FOCUS ───────────────────────────────────────────────────────────────────
  //
  // `use:trapFocus` moves focus into the menu on open, which is what makes the
  // keydown above fire at all, and restores it to the trigger on close. Restore is
  // the half everyone forgets (DESIGN_SYSTEM §5).
  import { trapFocus } from '../focus.js';

  /** Close this menu. Required: a menu with no way out is rule 44's defect. */
  export let close = () => {};
  /** The accessible name, usually the trigger's own label ("New", "More actions"). */
  export let label = '';
  /**
   * Swallow clicks inside the menu.
   *
   * Four of the six call sites write `on:click|stopPropagation` on the shell,
   * because they are opened from a row that is itself clickable and a press on a
   * menu row would otherwise also select the row behind it. Two do not. It is
   * therefore a real behavioural difference between call sites and NOT something a
   * shared component may decide for them, so it defaults to the behaviour of a
   * plain element and each caller opts in as it already did.
   */
  export let stopClicks = false;
  /** A class for POSITION: the anchor, the width, the z-index. See app.css. */
  let klass = '';
  export { klass as class };

  function onKey(e) {
    if (e.key !== 'Escape') return;
    e.stopPropagation();
    e.preventDefault();
    close();
  }
</script>

<!-- svelte-ignore a11y-no-noninteractive-element-interactions -->
<div
  class="r-menu {klass}"
  role="menu"
  tabindex="-1"
  aria-label={label || undefined}
  use:trapFocus
  on:keydown={onKey}
  on:click={(e) => stopClicks && e.stopPropagation()}
  {...$$restProps}
>
  <slot />
</div>
