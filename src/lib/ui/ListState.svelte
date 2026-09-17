<script>
  // EMPTY, LOADING AND ERROR ARE THREE DIFFERENT FACTS — made structural.
  //
  // `EmptyState`, `Loading` and `ErrorState` have existed for several waves and
  // they are correct. What did not exist is anything making a view CHOOSE between
  // them, so the choice stayed a thing each list wrote by hand, and a list that
  // wrote it by hand could leave a branch out.
  //
  // Measured on this tree: `views/library/Arrangements.svelte` renders four
  // different situations through one class and has no error branch at all, so a
  // failed `listArrangements()` is indistinguishable from a song with no
  // arrangements saved. The operator is told "None yet" about work they did last
  // Tuesday. That is the same defect as Live's "No service plans yet" before the
  // database had answered, which is the defect `Loading` was written for, arriving
  // from the other direction.
  //
  // The rule, restated from `ui/EmptyState.svelte` because it is the whole point:
  // if you do not KNOW the list is empty, you are Loading. If asking failed, you
  // are an error. Empty is what is left.
  //
  // ── THE ORDER IS NOT ARBITRARY ──────────────────────────────────────────────
  //
  // Error outranks loading, and loading outranks empty. A refresh that fails while
  // a stale list is on screen must say so rather than quietly showing the old rows,
  // and a list that is still being fetched must never say it is empty. Writing the
  // precedence here once is the point: four call sites writing three `{#if}`
  // branches each is four chances to order them differently, and two of them would.
  import EmptyState from './EmptyState.svelte';
  import ErrorState from './ErrorState.svelte';
  import Loading from './Loading.svelte';

  /** True while the answer is still outstanding. Not "the list is short". */
  export let loading = false;
  /** A typed `{ kind, message }` from Rust, or a plain string, or null. */
  export let error = null;
  /**
   * The rows. `null`/`undefined` means "nobody has asked yet", which is NOT empty
   * and is treated as loading, so a view that mounts before its fetch starts
   * cannot flash "nothing here" at an operator.
   */
  export let items = null;
  /** The sentence for the empty case. Say what is missing AND what to do. */
  export let empty = '';
  /** What is being fetched, announced by `Loading`: "Loading arrangements…". */
  export let what = '';
  /** Retry, passed through to `ErrorState`, which decides whether to offer it. */
  export let onRetry = null;
  export let compact = false;

  $: pending = loading || items == null;
  $: isEmpty = !pending && !error && Array.isArray(items) && items.length === 0;
</script>

{#if error}
  <ErrorState {error} {onRetry} {compact} />
{:else if pending}
  <Loading {what} {compact} />
{:else if isEmpty}
  <EmptyState message={empty} {compact}><slot name="empty" /></EmptyState>
{:else}
  <slot />
{/if}
