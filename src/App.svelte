<script>
  import { onMount, onDestroy } from 'svelte';
  import { capture, initAudio, clearScreens } from './lib/stores/capture.js';
  import Console from './lib/views/Console.svelte';
  import Channels from './lib/views/Channels.svelte';
  import Templates from './lib/views/Templates.svelte';
  import Library from './lib/views/Library.svelte';
  import ServicePlanner from './lib/views/ServicePlanner.svelte';
  import Settings from './lib/views/Settings.svelte';

  const tabs = [
    { key: 'console',   label: 'Console',   title: 'Mission Control',  view: Console },
    { key: 'channels',  label: 'Channels',  title: 'Output Channels',  view: Channels },
    { key: 'templates', label: 'Templates', title: 'Template Editor',  view: Templates },
    { key: 'library',   label: 'Library',   title: 'Content Library',  view: Library },
    { key: 'planner',   label: 'Planner',   title: 'Service Planner',  view: ServicePlanner },
    { key: 'settings',  label: 'Settings',  title: 'System Settings',  view: Settings },
  ];
  let active = 'console';
  $: currentTab = tabs.find((t) => t.key === active);
  $: current = currentTab.view;

  // Inline icons keyed by tab (SVG so they stay crisp on retina, themeable).
  const icons = {
    console: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7"><rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/></svg>',
    channels: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3"/><circle cx="4" cy="12" r="2"/><circle cx="12" cy="6" r="2"/><circle cx="20" cy="14" r="2"/></svg>',
    templates: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="m12 2 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5M3 17l9 5 9-5"/></svg>',
    library: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2V5Z"/><path d="M9 3v14"/></svg>',
    planner: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4"/><path d="M7 13h4M7 17h7"/></svg>',
    settings: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7"><circle cx="12" cy="12" r="3.2"/><path d="M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-2.7 1.1V21a2 2 0 1 1-4 0v-.1A1.6 1.6 0 0 0 7 19.4a1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0-1.1-2.7H1a2 2 0 1 1 0-4h.1A1.6 1.6 0 0 0 2.6 7a1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.6 1.6 0 0 0 7 2.6h.1A1.6 1.6 0 0 0 8 1.1V1a2 2 0 1 1 4 0v.1A1.6 1.6 0 0 0 15 2.6a1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0 1.1 2.7h.1a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1Z"/></svg>',
  };

  let clock = '';
  let timer;
  function tick() {
    clock = new Date().toLocaleTimeString('en-GB');
  }

  let engineOnline = false;
  onMount(async () => {
    tick();
    timer = setInterval(tick, 1000);
    await initAudio();
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('greet', { name: 'operator' });
      engineOnline = true;
    } catch {
      engineOnline = false;
    }
  });
  onDestroy(() => clearInterval(timer));
</script>

<div class="shell">
  <!-- Sidebar -->
  <aside class="side">
    <div class="side-brand">Relay</div>
    <div class="side-profile">
      <div class="side-avatar">
        <svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 2 4 7v13h16V7l-8-5Z"/><path d="M12 6v5M9.5 8.5h5M9 20v-4a3 3 0 0 1 6 0v4"/></svg>
      </div>
      <div class="who"><b>Relay Console</b><span>Live Service</span></div>
    </div>

    <nav class="nav">
      {#each tabs as t}
        <button class="nav-item r-focus" class:active={t.key === active} on:click={() => (active = t.key)}>
          <span class="ic">{@html icons[t.key]}</span>
          <span class="nav-label">{t.label}</span>
        </button>
      {/each}
    </nav>

    <div class="side-foot">
      <div class="row">
        <span class="k">AI Signal</span>
        <span class="dot" style="background:{engineOnline ? 'var(--v-amber)' : 'var(--v-faint)'};"></span>
      </div>
      <div class="m">Engine {engineOnline ? 'online' : 'offline'}</div>
      <div class="m">Detection {$capture.detectionOn ? 'active' : 'off'}</div>
    </div>
  </aside>

  <!-- Main -->
  <div class="main-v">
    <header class="topbar-v">
      <span class="topbar-title">{currentTab.title}</span>
      {#if $capture.capturing}
        <span class="r-badge rose pulse"><span class="bd"></span>On Air</span>
      {:else}
        <span class="r-badge amber"><span class="bd" style="box-shadow:none;"></span>Standby</span>
      {/if}
      <span class="topbar-spring"></span>
      <div class="topbar-icons">
        <span class="ib" title="Signal"><svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4.9 16.1a10 10 0 0 1 14.2 0M8 13a5.5 5.5 0 0 1 8 0"/><circle cx="12" cy="19" r="1.4" fill="currentColor" stroke="none"/></svg></span>
        <span class="ib" title="Clock"><svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.7"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2" stroke-linecap="round"/></svg></span>
        <span class="clock r-mono" style="font-size:13px;color:var(--v-dim);">{clock}</span>
      </div>
      <button class="r-btn danger sm" on:click={clearScreens} title="Blank every output screen">Emergency Stop</button>
    </header>

    <div class="mainscroll r-scroll">
      <svelte:component this={current} />
    </div>

    <footer class="footer-v">
      <div class="fl">
        <b>Relay AI</b>
        <span>Detection {$capture.detectionOn ? 'ACTIVE' : 'OFF'}</span>
      </div>
      <div style="display:flex;align-items:center;gap:8px;">
        <span class="dot" style="width:6px;height:6px;border-radius:50%;background:{$capture.capturing ? 'var(--v-rose)' : 'var(--v-faint)'};"></span>
        {$capture.capturing ? 'ON AIR' : 'SYSTEM STABLE'}
      </div>
    </footer>
  </div>

  <!-- Mobile bottom nav -->
  <nav class="botnav">
    {#each tabs as t}
      <button class="bn r-focus" class:active={t.key === active} on:click={() => (active = t.key)}>
        <span class="ic">{@html icons[t.key]}</span>
        {t.label}
      </button>
    {/each}
  </nav>
</div>
