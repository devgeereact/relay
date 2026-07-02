<script>
  // Phase 1 target (see PROMPT.md): blank window boots, topbar renders with
  // static data, tab navigation between the 5 screens works with no live
  // data behind any of them yet. Reference docs/design/relay-app-screens.html
  // for the full visual spec — this is intentionally minimal until Phase 2+
  // wires in real data through Tauri commands.

  const tabs = ['Console', 'Channels', 'Templates', 'Library', 'Settings'];
  let active = 'Console';
</script>

<main>
  <header>
    <div class="brand">
      <div class="mark">R</div>
      <span class="name">Relay</span>
    </div>
    <span class="status">scaffold — no live data yet</span>
  </header>

  <nav>
    {#each tabs as tab}
      <button class:active={tab === active} on:click={() => (active = tab)}>{tab}</button>
    {/each}
  </nav>

  <section class="content">
    <p>{active} screen — build this out per <code>docs/design/relay-app-screens.html</code> and <code>docs/SPEC.md</code>.</p>
  </section>
</main>

<style>
  :global(body) {
    margin: 0;
    background: #14161a;
    color: #edeef0;
    font-family: 'Inter', sans-serif;
  }
  main {
    max-width: 1320px;
    margin: 0 auto;
    padding: 18px 22px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 18px;
    background: #1b1e24;
    border: 1px solid #282c34;
    border-radius: 12px;
    margin-bottom: 12px;
  }
  .brand { display: flex; align-items: center; gap: 10px; }
  .mark {
    width: 28px; height: 28px; border-radius: 8px;
    background: linear-gradient(135deg, #e8a33d, #c97f22);
    color: #1b1204; font-weight: 700;
    display: flex; align-items: center; justify-content: center;
  }
  .name { font-weight: 600; }
  .status { font-family: monospace; font-size: 12px; color: #9ba0ac; }
  nav {
    display: flex; gap: 4px; padding: 5px; background: #1b1e24;
    border: 1px solid #282c34; border-radius: 11px; margin-bottom: 16px;
  }
  nav button {
    font-family: inherit; font-weight: 600; font-size: 12.5px; color: #9ba0ac;
    background: transparent; border: none; padding: 9px 16px; border-radius: 7px; cursor: pointer;
  }
  nav button.active { background: #22262e; color: #edeef0; }
  .content {
    background: #1b1e24; border: 1px solid #282c34; border-radius: 12px; padding: 24px;
    color: #9ba0ac; font-size: 14px;
  }
  code { background: #22262e; padding: 2px 5px; border-radius: 4px; }
</style>
