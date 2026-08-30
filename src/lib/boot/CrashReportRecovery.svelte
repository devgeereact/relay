<script>
  import { trapFocus } from '../focus.js';
  // LAUNCH & STARTUP · Crash Report Recovery
  //
  // Shown on the boot AFTER the crash guard fired. Its counterpart, lib/crash.js,
  // is the panel that appears at the MOMENT of a crash; this is the one that
  // appears the next time the app starts.
  //
  // Two jobs, in this order:
  //
  //   1. Lower the operator's heart rate. Same as crash.js: the output windows
  //      are separate webviews with their own reconnect loop, so a console crash
  //      is invisible to the congregation. That sentence is the largest thing on
  //      the card because it is the one that stops someone panicking.
  //   2. Offer — never assume — to send the report. Telemetry in Relay is
  //      opt-in and content-scrubbed (telemetry.rs, PRIVACY.md), and a crash
  //      screen is the single most tempting place in an app to quietly flip that
  //      default. It is not flipped here. The toggle starts OFF and says what
  //      leaves the machine if it is turned on.

  export let crash = null; // { at, message }
  export let streak = 0;
  export let onContinue = () => {};
  export let onSafeMode = () => {};
  /** (send: boolean) => void — called with the operator's explicit choice. */
  export let onSendChanged = () => {};

  let send = false;
  $: onSendChanged(send);

  $: when = crash?.at ? new Date(crash.at).toLocaleString() : 'unknown';
  $: firstLine = String(crash?.message ?? '').split('\n')[0] || 'Unknown error';
</script>

<!-- `use:trapFocus` — a boot gate is the FIRST thing a keyboard operator meets, and
     without the trap Tab walks straight out of it into an app that is not ready yet.
     `focus.js` is the one implementation; it deliberately does not bind Escape,
     which stays in `shortcuts.js`. -->
<div class="b-gate" role="alertdialog" aria-modal="true" aria-labelledby="b-crash-h" use:trapFocus>
  <div class="b-card">
    <p class="b-kicker" style="color:var(--v-red);"><span class="d"></span>Last session ended badly</p>

    <h2 id="b-crash-h">Relay closed unexpectedly.</h2>

    <p class="lead">
      <strong style="color:var(--v-emerald);">Your output screens were never affected.</strong>
      The projector, the stage display and any browser sources are separate windows with their
      own connection — they kept showing whatever was up, and a congregation would have seen
      nothing happen.
    </p>

    <div class="b-facts">
      <dl>
        <dt>When</dt>
        <dd class="r-mono">{when}</dd>
        <dt>Error</dt>
        <dd class="r-mono">{firstLine}</dd>
        {#if streak > 1}
          <dt>In a row</dt>
          <dd style="color:var(--v-red);">{streak} crashes</dd>
        {/if}
      </dl>
    </div>

    <!-- Opt-in, off by default, and it says what travels. -->
    <label style="display:flex;gap:11px;align-items:flex-start;cursor:pointer;margin-bottom:20px;">
      <input class="b-check-box" type="checkbox" bind:checked={send} style="margin-top:2px;" />
      <span style="font-size:13px;line-height:1.55;color:var(--v-dim);">
        Send this crash report to the Relay developers.
        <span style="display:block;color:var(--v-faint);font-size:12px;margin-top:3px;">
          The error text and the app version only. No transcript, no sermon audio, no verse
          text, no church name. Off unless you tick it, and you can change it later in Settings.
        </span>
      </span>
    </label>

    <div class="b-btn-row">
      <button class="r-btn primary" on:click={onContinue}>Continue starting Relay</button>
      {#if streak > 1}
        <button class="r-btn ghost" on:click={onSafeMode}>Start in safe mode</button>
      {/if}
    </div>

    <details>
      <summary>Technical detail (for a bug report)</summary>
      <pre>{crash?.message ?? ''}</pre>
    </details>
  </div>
</div>
