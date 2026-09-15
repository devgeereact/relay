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
    /* WRAP RATHER THAN SQUEEZE. This is a horizontal flex row, so in a narrow
       rail the message and the retry button compete for the same line and the text
       collapses to one or two words per line: measured 170 x 317px in Live's
       180px search rail — twenty lines — with the button floating at the
       vertical middle. This is the component whose entire purpose is to be
       readable when something has gone wrong, and the handbook already records a
       run surface that "rendered a failing screen's name seven pixels wide" as a
       defect worth fixing. Below about 240px the button drops to its own line. */
    flex-wrap: wrap;
    padding: 9px 11px;
    border-radius: var(--v-r-lg);
    /* Rose, not amber. Amber is the tally light and means ON AIR — an error that
       borrows it is a tally light that lies (docs/DECISIONS.md §18). */
    background: var(--v-red-soft);
    border: 1px solid var(--v-red-line);
    color: var(--v-red);
    font-size:var(--v-fs-h3);
    line-height: 1.55;
  }
  .es.compact {
    font-size:var(--v-fs-b2);
    padding: 6px 9px;
  }
  .es-msg {
    flex: 1 1 100%;
    min-width: 0;
  }
  /* …and the control goes to the end of whatever line it lands on. */
  .es :global(button) {
    margin-left: auto;
  }
</style>
