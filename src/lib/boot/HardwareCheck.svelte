<script>
  // LAUNCH & STARTUP · Hardware Check
  //
  // Seven real reads. CPU, memory, GPU and disk were once marked "not probed";
  // they are now `system_hardware` (src-tauri/src/sysprobe.rs).
  //
  // Two of them are judgement calls, not just numbers, and the thresholds are in
  // probes.js with the reasoning attached: fewer than 4 threads means whisper
  // will lag the preacher, and under ~1.5 GB free means the laptop swaps
  // mid-sermon. A number with no verdict attached is a number an operator has to
  // interpret under pressure.
  //
  // The GPU row reports the backends compiled into THIS BINARY, never the card
  // in the machine. Naming an RTX 4090 next to a CPU-only build would be the
  // most convincing lie on the screen.

  import BootShell from './BootShell.svelte';
  import CheckList from './CheckList.svelte';
  import { checks, rollUp } from './boot.js';

  export let version = '';
  export let safe = false;
  export let onContinue = () => {};
  // Accepted for a uniform stage contract; only Diagnostics offers a retry.
  export let onRetry = () => {};

  $: items = $checks.hardware;
  $: done = items.filter((c) => c.state !== 'pending' && c.state !== 'running').length;
  $: pct = items.length ? Math.round((done / items.length) * 100) : 0;
  $: verdict = rollUp(items);
  $: warnings = items.filter((c) => c.state === 'warn');
</script>

<BootShell
  stage="hardware"
  {version}
  {safe}
  footer={verdict === 'running' ? 'Reading hardware' : 'Hardware read'}
>
  <p class="b-eyebrow">Step 2 of 4</p>
  <h1 class="b-h1">What this machine can do</h1>
  <p class="b-lead">
    Relay runs on whatever laptop the church already owns. This is what it found, measured
    now — not what it assumes.
  </p>

  <div class="b-prog">
    <div class="track"><div class="fill" style="transform:scaleX({pct / 100})"></div></div>
    <span class="pct">{pct}%</span>
  </div>

  <CheckList {items} />

  <!-- A warning here is ACTIONABLE, and the action is always "before the service,
       not during it". Say what to do, not just that something is amiss. -->
  {#if warnings.length}
    <p class="b-lead" style="margin-top:var(--v-sp-md);font-size:var(--v-fs-b2);">
      This machine will run Relay, but {warnings.length === 1
        ? 'one thing is worth sorting out'
        : `${warnings.length} things are worth sorting out`} before Sunday — see the rows above.
      None of it stops you continuing now.
    </p>
  {:else}
    <p class="b-lead" style="margin-top:var(--v-sp-md);font-size:var(--v-fs-b2);">
      Whisper runs on the CPU in this build, so the processor and memory rows are the ones
      that decide whether transcription keeps up with a preacher.
    </p>
  {/if}

  {#if verdict !== 'running'}
    <div class="b-actions">
      <button class="r-btn primary" on:click={onContinue}>Continue</button>
    </div>
  {/if}
</BootShell>
