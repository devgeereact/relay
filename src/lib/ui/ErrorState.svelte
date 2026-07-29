<script>
  // "Asking failed" — the third fact, and the one that used to be shown as a raw Rust
  // error string in a MONOSPACE font to a church volunteer.
  //
  // It runs everything through lib/errors.js, the ONE humaniser, so an error says what
  // happened AND what to do — and, now that the backend sends `{ kind, message }`
  // (error.rs), it can also say whether pressing the button again is worth the
  // operator's time. As bare strings, "the database is busy" and "the disk is full"
  // were indistinguishable sentences.
  //
  // ── It is ASSERTIVE ─────────────────────────────────────────────────────────
  //
  // `role="alert"` — the only one of these three that interrupts. An operator acting on
  // a command that silently failed is about to make it worse, and mid-service they will
  // not go looking for a message they were never told about.
  import { humanError, isRetryable } from '../errors.js';

  /** The error: a typed `{ kind, message }` from Rust, or a plain string. */
  export let error = null;
  /** Optional retry. Only shown when retrying could actually help — see below. */
  export let onRetry = null;
  export let compact = false;

  $: message = error ? humanError(error) : '';
  // Do NOT offer "Try again" for a full disk or a missing file: a button that cannot
  // work is worse than no button, because the operator will keep pressing it instead of
  // fixing the actual problem. Offer it when the backend says the fault is transient,
  // or when the caller explicitly asked for it.
  $: canRetry = !!onRetry && (isRetryable(error) || error?.kind === undefined);
</script>

{#if error}
  <div class="es" class:compact role="alert">
    <span class="es-msg">{message}</span>
    {#if canRetry}
      <button class="r-btn ghost sm" on:click={onRetry}>Try again</button>
    {/if}
  </div>
{/if}

<style>
  .es {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 11px;
    border-radius: 8px;
    /* Rose, not amber. Amber is the tally light and means ON AIR — an error that
       borrows it is a tally light that lies (docs/DECISIONS.md §18). */
    background: rgba(239,68,68, 0.16);
    border: 1px solid rgba(239,68,68, 0.3);
    color: var(--v-red);
    font-size: 12.5px;
    line-height: 1.55;
  }
  .es.compact {
    font-size: 11.5px;
    padding: 6px 9px;
  }
  .es-msg {
    flex: 1;
    min-width: 0;
  }
</style>
