<script>
  // Phase 1 static controls. Thresholds here are the manual-override slider
  // that must ALWAYS exist (CLAUDE.md / DECISIONS.md self-calibrating gate):
  // seed auto-fire ≥0.90, suggest ≥0.60, nudged per install, never a hardcoded
  // global. Language priority reflects tier-1 Yoruba/Swahili/Hausa + English.
  let languages = [
    { name: 'Yoruba',  on: true,  dim: false },
    { name: 'Swahili', on: true,  dim: false },
    { name: 'Hausa',   on: true,  dim: false },
    { name: 'English', on: true,  dim: false },
    { name: 'Igbo',    on: false, dim: true },
  ];

  const translations = [
    { code: 'KJV', name: 'King James Version',       on: true },
    { code: 'NIV', name: 'New International Version', on: true },
    { code: 'AMP', name: 'Amplified',                on: false },
    { code: 'BSY', name: 'Bíbélì Mímọ́ Yorùbá',      on: true },
  ];

  const kiosks = [
    { name: 'Kiosk-Lobby-01', online: true },
    { name: 'Kiosk-Kids-01',  online: false },
  ];

  function toggleLang(i) {
    languages[i].on = !languages[i].on;
    languages = languages;
  }
</script>

<div class="settings-grid">
  <div class="panel">
    <div class="panel-title">Audio input</div>
    <select class="select-mock"><option>Shure MV7 — USB mic</option><option>Behringer X32 — Aux out</option></select>
    <div class="level-meter"><i></i></div>

    <div class="panel-title" style="margin-top:18px;">AI detection thresholds</div>
    <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint);">Auto-fire above</div>
    <div class="slider-mock"><i style="width:90%;"></i></div>
    <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); margin-top:10px;">Suggest above</div>
    <div class="slider-mock"><i style="width:65%;"></i></div>
  </div>

  <div class="panel">
    <div class="panel-title">Language priority</div>
    {#each languages as l, i}
      <div class="lang-row" style={l.dim ? 'opacity:.5;' : ''}>
        <span><span class="handle">⠿</span>{l.name}</span>
        <button class="toggle" class:on={l.on} on:click={() => toggleLang(i)} aria-label="Toggle {l.name}"></button>
      </div>
    {/each}
  </div>

  <div class="panel">
    <div class="panel-title">Bible translations</div>
    {#each translations as t}
      <label class="check-row"><input type="checkbox" checked={t.on} /> {t.code} — {t.name}</label>
    {/each}
  </div>

  <div class="panel">
    <div class="panel-title">Network &amp; kiosks</div>
    <div style="font-family:var(--f-mono); font-size:11.5px; color:var(--text-dim); margin-bottom:10px;">Local server — port 5173 — running</div>
    {#each kiosks as k}
      <div class="kiosk-row">
        <span>{k.name}</span>
        {#if k.online}<span class="status-ok">● connected</span>{:else}<span class="status-off">○ offline</span>{/if}
      </div>
    {/each}
    <div class="panel-title" style="margin-top:16px;">Offline speech model</div>
    <div style="font-family:var(--f-mono); font-size:11.5px; color:var(--text-dim);">medium.en + yoruba-ft — 1.4GB — up to date</div>
  </div>
</div>
