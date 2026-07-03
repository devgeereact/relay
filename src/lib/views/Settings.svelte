<script>
  import { onMount } from 'svelte';
  import { capture, initAudio, startCapture, stopCapture, setThresholds, setSttLanguage } from '../stores/capture.js';

  // Threshold sliders push to the router; keep the invariant auto_fire ≥ suggest.
  function onAuto(v) {
    const suggest = Math.min($capture.thresholds.suggest, v);
    setThresholds(v, suggest);
  }
  function onSuggest(v) {
    const suggest = Math.min(v, $capture.thresholds.auto_fire);
    setThresholds($capture.thresholds.auto_fire, suggest);
  }

  // --- Phase 3: live audio input (real cpal capture through the Rust engine) ---
  let selectedDevice = ''; // '' = default input
  onMount(initAudio);
  // NOTE: capture is app-level state, NOT tied to this view's lifetime — do not
  // stop it on unmount, or switching to the Console tab would kill the mic
  // mid-service. Capture stops only when the operator clicks Stop.

  async function toggleCapture() {
    if ($capture.capturing) await stopCapture();
    else await startCapture(selectedDevice || null);
  }

  // RMS on speech sits well below 1.0; scale so normal talking fills the meter.
  $: levelPct = Math.min(100, Math.round($capture.level * 320));

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
    <div class="panel-title">
      Audio input
      {#if $capture.available}
        <span class="count">{$capture.devices.length} device{$capture.devices.length === 1 ? '' : 's'}</span>
      {:else}
        <span class="count">backend not attached</span>
      {/if}
    </div>
    <select class="select-mock" bind:value={selectedDevice} disabled={!$capture.available || $capture.capturing}>
      <option value="">Default input</option>
      {#each $capture.devices as d}
        <option value={d.name}>{d.name}{d.is_default ? ' — default' : ''}</option>
      {/each}
    </select>

    <div class="level-meter"><i style="width:{levelPct}%;"></i></div>

    <div style="display:flex; align-items:center; justify-content:space-between; margin-top:10px;">
      <button class="ctrl-btn" class:primary={!$capture.capturing} on:click={toggleCapture} disabled={!$capture.available}>
        <span class="dot" style="background:{$capture.capturing ? 'var(--red)' : '#1b1204'};"></span>
        {$capture.capturing ? 'Stop listening' : 'Start listening'}
      </button>
      {#if $capture.capturing}
        <span style="font-family:var(--f-mono); font-size:11px; color:{$capture.isVoice ? 'var(--green)' : 'var(--text-faint)'};">
          {$capture.isVoice ? '● voice' : '○ silence'} · {$capture.level.toFixed(3)} rms
        </span>
      {/if}
    </div>

    <div class="panel-title" style="margin-top:18px;">
      AI detection thresholds
      <span class="count">self-calibrating</span>
    </div>
    <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); display:flex; justify-content:space-between;">
      <span>Auto-fire above</span><span style="color:var(--green);">{$capture.thresholds.auto_fire.toFixed(2)}</span>
    </div>
    <input class="range" type="range" min="0.5" max="0.99" step="0.01"
      value={$capture.thresholds.auto_fire}
      on:input={(e) => onAuto(+e.target.value)} disabled={!$capture.available} />
    <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); margin-top:10px; display:flex; justify-content:space-between;">
      <span>Suggest above</span><span style="color:var(--amber);">{$capture.thresholds.suggest.toFixed(2)}</span>
    </div>
    <input class="range" type="range" min="0.3" max="0.9" step="0.01"
      value={$capture.thresholds.suggest}
      on:input={(e) => onSuggest(+e.target.value)} disabled={!$capture.available} />
  </div>

  <div class="panel">
    <div class="panel-title">
      Recognition language
      {#if $capture.capturing && $capture.detectedLang}
        <span class="count" style="color:var(--green);">hearing: {$capture.detectedLang}</span>
      {/if}
    </div>
    <select class="select-mock" value={$capture.stt.language ?? ''} on:change={(e) => setSttLanguage(e.target.value || null)} disabled={!$capture.stt.loaded}>
      <option value="">Auto-detect (code-switching)</option>
      <option value="en">English</option>
      <option value="yo">Yoruba</option>
      <option value="sw">Swahili</option>
      <option value="ha">Hausa</option>
    </select>
    <div style="font-family:var(--f-mono); font-size:10.5px; color:var(--text-faint); margin-top:8px;">
      Auto-detect handles English mixed with a local language mid-sentence — the normal case. Tier-1: Yoruba · Swahili · Hausa.
    </div>

    <div class="panel-title" style="margin-top:16px;">Language priority</div>
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
    <div style="font-family:var(--f-mono); font-size:11.5px; color:var(--text-dim); margin-bottom:10px;">Kiosk server — port 8031 — running</div>
    {#each kiosks as k}
      <div class="kiosk-row">
        <span>{k.name}</span>
        {#if k.online}<span class="status-ok">● connected</span>{:else}<span class="status-off">○ offline</span>{/if}
      </div>
    {/each}
    <div class="panel-title" style="margin-top:16px;">Offline speech model</div>
    {#if $capture.stt.loaded}
      <div style="font-family:var(--f-mono); font-size:11.5px; color:var(--green);">● loaded</div>
      <div style="font-family:var(--f-mono); font-size:10.5px; color:var(--text-faint); margin-top:4px; word-break:break-all;">{$capture.stt.model}</div>
    {:else}
      <div style="font-family:var(--f-mono); font-size:11.5px; color:var(--text-faint);">○ no model — audio-only (see README dev setup)</div>
    {/if}
  </div>
</div>
