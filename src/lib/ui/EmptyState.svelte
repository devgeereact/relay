<script>
  // "There is nothing here" — said once, the same way, everywhere.
  //
  // There were FOUR competing classes for this (.r-empty, .empty, .chan-empty,
  // .cat-empty), and the most important screen in the product — Live — used the one
  // that was NOT the shared one. So the text a new operator most needs to read was
  // styled by whichever view they happened to be looking at.
  //
  // This deliberately renders the EXISTING global `.r-empty` class rather than
  // inventing a new look: consolidating the markup should not silently restyle eight
  // screens, and this machine cannot screenshot one to check.
  //
  // ── Empty is not an error, and it is not a loading state ─────────────────────
  //
  // Three different facts that all look like "no rows on screen", and conflating them
  // is how Live came to tell an operator "No service plans yet" — the one message that
  // makes them think they have lost their work — during the few frames before the
  // database had answered. If you do not KNOW the list is empty, you are Loading, not
  // Empty. If asking failed, you are an ErrorState.
  //
  // Not announced to a screen reader: an empty list is not news, and it is already read
  // out when the operator navigates to it. Loading and errors ARE announced.

  /** The sentence. Say what is missing AND what to do about it. */
  export let message = '';
  /** Smaller type, for inline/tight spots (a sidebar column, a chip row). */
  export let compact = false;
</script>

<div class="r-empty es" class:compact>
  {#if message}<span>{message}</span>{/if}
  <slot />
</div>

<style>
  .es {
    line-height: 1.6;
    padding: 10px 2px;
  }
  .es.compact {
    font-size: 12.5px;
    padding: 6px 2px;
  }
  /* An action inside an empty state ("Build one in Planner") gets room to breathe. */
  .es :global(.r-btn) {
    margin-top: 10px;
  }
</style>
