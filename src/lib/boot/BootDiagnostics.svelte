<script>
  // LAUNCH & STARTUP · Boot Diagnostics
  //
  // The first screen with facts on it. Six probes, all real Tauri commands (see
  // lib/boot/probes.js): the engine, the build, the database, the STT model, the
  // audio devices, the network.
  //
  // "Network: offline" is rendered GREEN. That is not a bug. Relay's premise is
  // that every core feature works with the cable out, and a boot screen that
  // paints the designed state amber teaches an operator that normal is broken.

  import BootShell from './BootShell.svelte';
  import CheckList from './CheckList.svelte';
  import { checks, rollUp } from './boot.js';

  export let version = '';
  export let safe = false;
  /** Fired when the operator chooses to continue past a failed check. */
  export let onContinue = () => {};
  export let onRetry = () => {};

  $: items = $checks.diagnostics;
  $: done = items.filter((c) => c.state !== 'pending' && c.state !== 'running').length;
  $: pct = items.length ? Math.round((done / items.length) * 100) : 0;
  $: verdict = rollUp(items);
  $: failed = items.filter((c) => c.state === 'fail');
</script>

<BootShell
  stage="diagnostics"
  {version}
  {safe}
  footer={verdict === 'running' ? 'Running diagnostics' : `Diagnostics ${verdict}`}
>
  <p class="b-eyebrow">Step 1 of 4</p>
  <h1 class="b-h1">Checking that Relay can do its job</h1>
  <p class="b-lead">
    Six things have to be true before a service: the engine is attached, the scripture is
    readable, a microphone exists, and speech recognition has a model. Relay checks them now,
    while nothing is on any screen — not at 10:58 on a Sunday.
  </p>

  <div class="b-prog">
    <div class="track"><div class="fill" style="transform:scaleX({pct / 100})"></div></div>
    <span class="pct">{pct}%</span>
  </div>

  <CheckList {items} />

  {#if verdict === 'fail'}
    <!-- A failed diagnostic is NOT a locked door. Relay is a tool someone may be
         holding open at the last minute; the operator gets to walk past it,
         having been told plainly what will not work. -->
    <p class="b-lead" style="color:var(--v-red);margin-top:var(--v-sp-md);">
      {failed.length === 1 ? failed[0].label : `${failed.length} checks`} failed.
      {#if failed.some((c) => c.id === 'database')}
        Scripture cannot be looked up, so detection and manual fires will not work.
      {:else if failed.some((c) => c.id === 'audio')}
        There is no microphone, so nothing will be transcribed. You can still fire verses by hand.
      {:else}
        You can continue, but the affected feature will not work.
      {/if}
    </p>
    <div class="b-actions">
      <button class="r-btn primary" on:click={onRetry}>Run the checks again</button>
      <button class="r-btn ghost" on:click={onContinue}>Continue anyway</button>
    </div>
  {:else if verdict !== 'running'}
    <div class="b-actions">
      <button class="r-btn primary" on:click={onContinue}>Continue</button>
    </div>
  {/if}
</BootShell>
