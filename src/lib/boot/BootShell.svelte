<script>
  // Shared chrome for every LAUNCH & STARTUP stage screen.
  //
  // The four stage screens (Boot Diagnostics, Hardware Check, Plugin Loading,
  // Database Migration) are one screen changing, not four apps — so the brand
  // row, the stage rail and the footer live here exactly once.
  //
  // Nothing in here is amber. See app.css `.b-*`.

  import { STAGES } from './boot.js';
  import BrandMark from '../ui/BrandMark.svelte';

  export let stage = 'diagnostics';
  export let version = '';
  /** Bottom-left status line — what the sequence is doing right now. */
  export let footer = 'Starting Relay';
  /** True when nothing this app does can reach a congregation. */
  export let safe = false;

  const LABELS = {
    diagnostics: 'Diagnostics',
    hardware: 'Hardware',
    plugins: 'Integrations',
    migration: 'Database',
  };
  $: index = STAGES.indexOf(stage);
</script>

<section class="b-shell" role="status" aria-live="polite" aria-busy="true">
  <header class="b-bar">
    <BrandMark size="16px" />
    <b>RELAY</b>
    {#if version}<span class="b-ver r-mono">v{version}</span>{/if}
    <span class="b-spring"></span>

    <!-- Where in the sequence we are. Decoration for sighted operators only —
         the live region above announces the real state. -->
    <nav class="b-rail" aria-hidden="true">
      {#each STAGES as s, i}
        {#if i > 0}<span class="sep"></span>{/if}
        <span class="s" class:on={i === index} class:done={i < index}>
          <span class="n">{i < index ? '✓' : i + 1}</span>{LABELS[s]}
        </span>
      {/each}
    </nav>
  </header>

  <div class="b-body">
    <div class="b-inner">
      <slot />
    </div>
  </div>

  <footer class="b-foot">
    <span class="dot"></span>
    <span>{footer}</span>
    <span class="b-spring"></span>
    {#if safe}
      <span style="color:var(--v-amethyst);">Safe mode — outputs disabled</span>
    {:else}
      <span>Offline · all processing local</span>
    {/if}
  </footer>
</section>
