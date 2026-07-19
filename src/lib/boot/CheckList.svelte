<script>
  // The check rows shared by all four stage screens.
  //
  // The whole reason this is one component: the `unknown` state — a check with
  // NO probe behind it — must render identically everywhere, and it must never
  // be mistakable for a pass. Four copies of this markup is four chances for one
  // of them to paint an unprobed GPU green.

  /** [{ id, label, detail, probe, state, note }] — see lib/boot/boot.js */
  export let items = [];
</script>

<ul class="b-checks">
  {#each items as c (c.id)}
    <li class="b-check {c.state}">
      <span class="ico" aria-hidden="true">
        {#if c.state === 'running'}
          <span class="b-spinner"></span>
        {:else if c.state === 'ok'}
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="9" /><path d="m8.5 12 2.5 2.5 4.5-5" />
          </svg>
        {:else if c.state === 'warn'}
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="9" /><path d="M12 7.5v5" /><path d="M12 16.2h.01" />
          </svg>
        {:else if c.state === 'fail'}
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="9" /><path d="m9 9 6 6M15 9l-6 6" />
          </svg>
        {:else if c.state === 'unknown'}
          <!-- A dash, not a tick and not a cross. Nothing was measured. -->
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round">
            <circle cx="12" cy="12" r="9" stroke-dasharray="3 3" /><path d="M8.5 12h7" />
          </svg>
        {:else}
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor"
            stroke-width="2" opacity=".5"><circle cx="12" cy="12" r="9" /></svg>
        {/if}
      </span>

      <span class="txt">
        <b>
          {c.label}
          {#if c.probe === 'stub'}<span class="b-stub">not probed</span>{/if}
        </b>
        <span>{c.detail}</span>
      </span>

      <span class="note">{c.note}</span>
    </li>
  {/each}
</ul>
