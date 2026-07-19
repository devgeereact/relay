<script>
  // LAUNCH & STARTUP · Safe Mode Startup
  //
  // Offered after three bad exits in a row, and reachable any time the operator
  // asks. Safe mode is a PROMISE, not a label:
  //
  //   Nothing this app does can reach a congregation.
  //
  // So it is not decoration on a splash footer — App.svelte honours it by
  // disarming detection and refusing to open output windows, and the whole shell
  // wears the amethyst "not reaching the screens" mode for as long as it is on.
  // If you add a code path that can put pixels on a wall, it must check
  // `$safeMode` first or this screen becomes a lie.

  export let streak = 0;
  export let onEnter = () => {};
  export let onNormal = () => {};
</script>

<div class="b-gate" role="dialog" aria-modal="true" aria-labelledby="b-safe-h">
  <div class="b-card">
    <p class="b-kicker" style="color:var(--v-amethyst);"><span class="d"></span>Safe mode</p>

    <h2 id="b-safe-h">
      {#if streak >= 3}
        Relay has failed to start properly {streak} times.
      {:else}
        Start Relay with the outputs disabled?
      {/if}
    </h2>

    <p class="lead">
      Safe mode starts the console with <strong style="color:var(--v-txt);">nothing able to
      reach a screen</strong>. You can look at settings, templates, the library and the logs —
      and find out what is wrong — without any risk of something appearing on the projector.
    </p>

    <div class="b-facts">
      <dl>
        <dt>Outputs</dt>
        <dd>Will not open. Nothing can be fired.</dd>
        <dt>Detection</dt>
        <dd>Disarmed. The microphone stays off.</dd>
        <dt>Your data</dt>
        <dd>Untouched — plans, library and history all load normally.</dd>
      </dl>
    </div>

    <div class="b-btn-row">
      <button class="r-btn primary" on:click={onEnter}>Start in safe mode</button>
      <button class="r-btn ghost" on:click={onNormal}>Start normally</button>
    </div>

    <p style="margin:16px 0 0;font-size:12px;color:var(--v-faint);">
      Safe mode stays on until you turn it off in Settings — it will not quietly switch
      itself back the next time you launch. Do not run a service in it.
    </p>
  </div>
</div>
