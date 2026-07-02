<script>
  import { onMount, onDestroy } from 'svelte';
  import { capture } from './lib/stores/capture.js';
  import Console from './lib/views/Console.svelte';
  import Channels from './lib/views/Channels.svelte';
  import Templates from './lib/views/Templates.svelte';
  import Library from './lib/views/Library.svelte';
  import Settings from './lib/views/Settings.svelte';

  // Phase 1 goal (PROMPT.md): shell boots, topbar renders, tab nav between all
  // 5 screens works with no live data behind them yet. Visual language tracks
  // docs/design/relay-app-screens.html exactly.
  const tabs = [
    { num: '01', key: 'console',   label: 'Console',   view: Console },
    { num: '02', key: 'channels',  label: 'Channels',  view: Channels },
    { num: '03', key: 'templates', label: 'Templates', view: Templates },
    { num: '04', key: 'library',   label: 'Library',   view: Library },
    { num: '05', key: 'settings',  label: 'Settings',  view: Settings },
  ];
  let active = 'console';
  $: current = tabs.find((t) => t.key === active).view;

  // Live clock (topbar), matching the mockup's en-GB HH:MM:SS.
  let clock = '';
  let timer;
  function tick() {
    clock = new Date().toLocaleTimeString('en-GB');
  }

  // Prove the frontend<->backend bridge is alive. In `tauri dev` this resolves;
  // in a plain browser (vite dev) there's no Tauri runtime, so we fall back
  // quietly. Replace `greet` with real status commands as the core lands.
  let engineOnline = false;
  onMount(async () => {
    tick();
    timer = setInterval(tick, 1000);
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('greet', { name: 'operator' });
      engineOnline = true;
    } catch {
      engineOnline = false; // browser preview — backend not attached
    }
  });
  onDestroy(() => clearInterval(timer));
</script>

<div class="app">
  <div class="topbar">
    <div class="brand">
      <div class="brand-mark">R</div>
      <div class="brand-name">Relay</div>
      {#if engineOnline}
        <div class="status-pill live"><span class="status-dot"></span> LIVE · Sunday Service</div>
      {:else}
        <div class="status-pill idle"><span class="status-dot"></span> STANDBY · no service</div>
      {/if}
    </div>
    <div class="topbar-right">
      <div class="listen">
        <div class="listen-rings" class:idle={!$capture.capturing}><span></span><span></span><div class="dot"></div></div>
        {#if $capture.capturing}
          <div class="listen-label">Listening — <b>{$capture.isVoice ? 'voice' : 'silence'}</b></div>
        {:else}
          <div class="listen-label">Not listening</div>
        {/if}
      </div>
      <div class="topbar-meta"><span>4/5 channels online</span><span class="clock">{clock}</span></div>
    </div>
  </div>

  <div class="tabstrip">
    {#each tabs as t}
      <button class="tab" class:active={t.key === active} on:click={() => (active = t.key)}>
        <span class="num">{t.num}</span>{t.label}
      </button>
    {/each}
  </div>

  <svelte:component this={current} />
</div>
