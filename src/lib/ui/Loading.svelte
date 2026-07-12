<script>
  // "We are still asking" — a fact, and a different one from "there is nothing here".
  //
  // Only TWO views in the whole app had a loading state (History, SongEditor). Every
  // other list rendered its empty state the instant it mounted, before the database had
  // answered — so Live told an operator with a full plan library "No service plans
  // yet", on every single visit, for the few frames before `listPlans()` resolved.
  // That is the one message that makes a new operator think they have lost their work.
  //
  // The rule: if you do not KNOW the list is empty, you are not Empty. You are here.
  //
  // ── It IS announced ─────────────────────────────────────────────────────────
  //
  // A sighted operator sees the word. A screen-reader operator was told nothing at all,
  // so a slow query was indistinguishable from a dead button. `role="status"` +
  // `aria-live="polite"` — polite, because "loading" must never talk over the operator,
  // and `aria-busy` so assistive tech knows the region is still settling.

  /** What is being fetched. Announced, so name the thing: "Loading plans…". */
  export let what = '';
  /** Smaller type, for inline/tight spots. */
  export let compact = false;
</script>

<div
  class="r-empty ld"
  class:compact
  role="status"
  aria-live="polite"
  aria-busy="true"
>
  <span class="r-mono">Loading{what ? ` ${what}` : ''}…</span>
</div>

<style>
  .ld {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 10px 2px;
    font-size: 12.5px;
  }
  .ld.compact {
    font-size: 11.5px;
    padding: 6px 2px;
  }
  /* A quiet pulse. It is doing something, and the operator should not wonder whether
     the app has hung — but it is NOT amber, because amber is the tally light and it
     means ON AIR. A spinner that borrows the on-air colour is a tally light that lies. */
  .ld::before {
    content: '';
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--v-faint, #88888d);
    flex: 0 0 auto;
    animation: ld-pulse 1.4s ease-in-out infinite;
  }
  @keyframes ld-pulse {
    0%,
    100% {
      opacity: 0.35;
    }
    50% {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ld::before {
      animation: none;
    }
  }
</style>
