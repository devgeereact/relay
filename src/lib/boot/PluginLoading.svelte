<script>
  // LAUNCH & STARTUP · Plugin Loading
  //
  // A NAMING CORRECTION, made visible rather than hidden: Relay has no plugin
  // loader, and executes no third-party code at boot. What the screen list calls
  // "plugins" are Relay's OUTPUT and CONTROL surfaces — the kiosk hub, the HTTP
  // output server, ProPresenter import, NDI, OBS, ATEM.
  //
  // So this screen says "Integrations" and reports what each one can actually do
  // in THIS build. NDI is shown as unavailable, in words, because CLAUDE.md says
  // it needs a proprietary SDK that is not in an MIT repo — the same thing
  // `open_ndi_output` returns at runtime. A launch screen that implied NDI was
  // ready would be discovered to be lying at the worst possible moment.

  import BootShell from './BootShell.svelte';
  import CheckList from './CheckList.svelte';
  import { checks, rollUp } from './boot.js';

  export let version = '';
  export let safe = false;
  export let onContinue = () => {};
  // Accepted for a uniform stage contract; only Diagnostics offers a retry.
  export let onRetry = () => {};

  $: items = $checks.plugins;
  $: done = items.filter((c) => c.state !== 'pending' && c.state !== 'running').length;
  $: pct = items.length ? Math.round((done / items.length) * 100) : 0;
  $: verdict = rollUp(items);
</script>

<BootShell
  stage="plugins"
  {version}
  {safe}
  footer={verdict === 'running' ? 'Starting integrations' : 'Integrations ready'}
>
  <p class="b-eyebrow">Step 3 of 4</p>
  <h1 class="b-h1">Integrations</h1>
  <p class="b-lead">
    Relay does not load plugins and runs no third-party code at startup. These are the
    surfaces it can send to or read from, and what each one is capable of in this build.
  </p>
  <p class="b-lead" style="font-size:var(--v-fs-b2);margin-top:var(--v-sp-sm);">
    OBS and ATEM are checked by looking for something listening on their default ports.
    Relay speaks neither control protocol, so it can tell you a port answered — it cannot
    tell you what answered, and it does not pretend to.
  </p>

  <div class="b-prog">
    <div class="track"><div class="fill" style="transform:scaleX({pct / 100})"></div></div>
    <span class="pct">{pct}%</span>
  </div>

  <CheckList {items} />

  <p class="b-lead" style="margin-top:var(--v-sp-md);font-size:var(--v-fs-b2);">
    Relay is built to sit alongside OBS, ATEM and ProPresenter over NDI, HDMI and the
    network — not to replace them. Point a browser source at the output server and it
    behaves like any other source.
  </p>

  {#if verdict !== 'running'}
    <div class="b-actions">
      <button class="r-btn primary" on:click={onContinue}>Continue</button>
    </div>
  {/if}
</BootShell>
