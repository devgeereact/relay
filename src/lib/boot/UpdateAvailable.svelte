<script>
  // LAUNCH & STARTUP · Update Available
  //
  // The LAUNCH-TIME face of lib/updater.js. The at-rest banner in App.svelte is
  // the other one; this is the version an operator sees before the console opens,
  // which is the only genuinely safe moment to restart the app.
  //
  // The rule from updater.js is repeated here because it is the whole point:
  //
  //   RELAY NEVER UPDATES DURING A SERVICE.
  //
  // Installing restarts the app. So this screen exists at boot precisely so the
  // question is asked when the answer is cheap, and "Not now" is a first-class
  // button, not a link in small print. Nothing here is amber — an update is not
  // on air, and the tally colour is never borrowed for emphasis.

  export let update = null; // { version, notes }
  export let current = '';
  export let progress = null; // 0–100 while downloading
  export let error = null;
  export let onInstall = () => {};
  export let onLater = () => {};
</script>

<div class="b-gate" role="dialog" aria-modal="true" aria-labelledby="b-upd-h">
  <div class="b-card">
    <p class="b-kicker" style="color:var(--v-amethyst);"><span class="d"></span>Update available</p>

    <h2 id="b-upd-h">Relay {update?.version} is ready to install.</h2>

    <p class="lead">
      Installing restarts Relay. Doing it now — before anything is set up and before anyone is
      in the room — costs you nothing. Relay will never offer this during a service.
    </p>

    <div class="b-facts">
      <dl>
        <dt>Installed</dt>
        <dd class="r-mono">v{current || '—'}</dd>
        <dt>Available</dt>
        <dd class="r-mono">v{update?.version ?? '—'}</dd>
      </dl>
    </div>

    {#if update?.notes}
      <details open>
        <summary>What changed</summary>
        <pre>{update.notes}</pre>
      </details>
    {/if}

    {#if error}
      <p style="color:var(--v-red);font-size:13px;line-height:1.55;margin:16px 0 0;">{error}</p>
      <div class="b-btn-row" style="margin-top:16px;">
        <button class="r-btn ghost" on:click={onLater}>Continue without updating</button>
      </div>
    {:else if progress !== null}
      <div class="b-prog" style="margin-top:20px;">
        <div class="track"><div class="fill" style="transform:scaleX({progress / 100})"></div></div>
        <span class="pct">{progress}%</span>
      </div>
      <p style="margin:12px 0 0;font-size:12px;color:var(--v-faint);">
        Downloading. Relay will restart on its own when this finishes.
      </p>
    {:else}
      <div class="b-btn-row" style="margin-top:20px;">
        <button class="r-btn primary" on:click={onInstall}>Update and restart</button>
        <button class="r-btn ghost" on:click={onLater}>Not now</button>
      </div>
    {/if}
  </div>
</div>
