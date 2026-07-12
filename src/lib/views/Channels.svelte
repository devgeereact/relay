<script>
  import { onMount } from 'svelte';
  import QRCode from 'qrcode';
  // Was: `error = String(err)`, rendered in a MONOSPACE font, five times over — a raw
  // Rust Err string shown to a church volunteer who has never seen one.
  import { humanError } from '../errors.js';
  import {
    capture,
    templates,
    loadTemplates,
    listOutputChannels,
    setChannelTemplate,
    listMonitors,
    openChannelOutput,
    setChannelDisplay,
    addChannel,
    deleteChannel,
    localIp,
  } from '../stores/capture.js';

  // Channels are DB-backed and every output is freely assignable to any template.
  // native_window = borderless fullscreen on a chosen physical display (HDMI).
  // network_client (OBS/vMix/kiosk) = add a Browser Source pointing at the
  // channel URL; it live-updates over the kiosk WebSocket (:8031). NDI is parked.
  let channels = [];
  let monitors = [];
  let error = '';
  let copiedId = null;
  // LAN address so the output URL/QR points at a real IP other devices can reach.
  let lanIp = 'localhost';
  let qrOpen = null; // channel id whose QR is showing
  let qrData = '';

  // Add-channel form state.
  let newName = '';
  let newTarget = 'native_window';

  async function refresh() {
    channels = await listOutputChannels();
  }
  onMount(async () => {
    await loadTemplates();
    monitors = await listMonitors();
    lanIp = (await localIp()) || 'localhost';
    await refresh();
  });

  const chipFor = (i) => ['var(--amber)', 'var(--teal)', 'var(--violet)', 'var(--rose)', 'var(--text-faint)'][i % 5];
  const obsUrl = (c) => `http://${lanIp}:8032/output.html?template_id=${c.template_id ?? 1}&name=${encodeURIComponent(c.name)}`;
  const isNative = (c) => c.render_target === 'native_window';

  async function showQr(c) {
    if (qrOpen === c.id) { qrOpen = null; return; }
    try {
      qrData = await QRCode.toDataURL(obsUrl(c), { width: 190, margin: 1, color: { dark: '#0a0a0b', light: '#ffffff' } });
      qrOpen = c.id;
    } catch { /* qr gen failed */ }
  }

  // Preacher's mobile stage-display remote.
  const stageUrl = () => `http://${lanIp}:8032/stage.html`;
  let stageQr = '';
  let stageQrOpen = false;
  let copiedStage = false;
  async function showStageQr() {
    if (stageQrOpen) { stageQrOpen = false; return; }
    try {
      stageQr = await QRCode.toDataURL(stageUrl(), { width: 200, margin: 1, color: { dark: '#0a0a0b', light: '#ffffff' } });
      stageQrOpen = true;
    } catch { /* qr gen failed */ }
  }
  async function copyStage() {
    try {
      await navigator.clipboard.writeText(stageUrl());
      copiedStage = true;
      setTimeout(() => (copiedStage = false), 1500);
    } catch { /* clipboard blocked */ }
  }

  async function assignTemplate(c, e) {
    try {
      await setChannelTemplate(c.id, parseInt(e.target.value, 10));
      await refresh();
      error = '';
    } catch (err) { error = humanError(err); }
  }

  async function assignDisplay(c, e) {
    const v = e.target.value;
    try {
      await setChannelDisplay(c.id, v === '' ? null : v);
      await refresh();
      error = '';
    } catch (err) { error = humanError(err); }
  }

  async function openNative(c) {
    try {
      await openChannelOutput(c.id);
      error = '';
    } catch (err) { error = humanError(err); }
  }

  async function add() {
    const name = newName.trim();
    if (!name) return;
    try {
      await addChannel(name, newTarget, 1);
      newName = '';
      newTarget = 'native_window';
      await refresh();
      error = '';
    } catch (err) { error = humanError(err); }
  }

  // Two-step delete (no native confirm — Tauri's webview doesn't implement it).
  // First click arms the row; second within 3s deletes.
  let delArm = null;
  let delArmT;
  async function remove(c) {
    if (delArm !== c.id) {
      delArm = c.id;
      clearTimeout(delArmT);
      delArmT = setTimeout(() => (delArm = null), 3000);
      return;
    }
    clearTimeout(delArmT);
    delArm = null;
    try {
      await deleteChannel(c.id);
      await refresh();
      error = '';
    } catch (err) { error = humanError(err); }
  }

  async function copyUrl(c) {
    try {
      await navigator.clipboard.writeText(obsUrl(c));
      copiedId = c.id;
      setTimeout(() => (copiedId = null), 1500);
    } catch { /* clipboard blocked */ }
  }
</script>

<div class="ch-view">
  <p class="r-lead">Every output is a render target of one shared template engine — main screen, stage, streaming, and lobby all pull from the same source, styled per channel. Assign a template, pick a display or copy a browser-source URL.</p>

  <div class="ch-actionbar">
    {#if !$capture.available}
      <span class="r-badge rose"><span class="bd"></span>Backend not attached</span>
    {:else}
      <span class="r-badge amber pulse"><span class="bd"></span>Engine ready</span>
    {/if}
    <span class="ch-count r-mono">{channels.length} configured · {monitors.length} display{monitors.length === 1 ? '' : 's'}</span>
  </div>

  <!-- Column labels -->
  <div class="ch-head r-lbl">
    <span class="c-name">Channel</span>
    <span class="c-tmpl">Template</span>
    <span class="c-out">Output target</span>
    <span class="c-act">Actions</span>
  </div>

  <!-- Channel rows -->
  <div class="ch-list">
    {#each channels as c, i}
      <div class="r-row ch-row">
        <span class="bar" style="background:{chipFor(i)};"></span>

        <div class="c-name">
          <b>{c.name}</b>
          <span class="r-mono ch-target">{c.render_target.replace('_', ' ')}</span>
        </div>

        <div class="c-tmpl">
          <select class="r-select" value={c.template_id} on:change={(e) => assignTemplate(c, e)} disabled={!$capture.available}>
            {#each $templates as t}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
        </div>

        <div class="c-out">
          {#if isNative(c)}
            <svg class="ch-oico" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
            <select class="r-select" value={c.display_target ?? ''} on:change={(e) => assignDisplay(c, e)} disabled={!$capture.available}>
              <option value="">Primary display</option>
              {#each monitors as m}
                <option value={String(m.index)}>{m.name} · {m.width}×{m.height}{m.primary ? ' (primary)' : ''}</option>
              {/each}
            </select>
          {:else}
            <svg class="ch-oico" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18"/></svg>
            <span class="r-mono ch-nettxt">network client · OBS / kiosk</span>
          {/if}
        </div>

        <div class="c-act">
          {#if isNative(c)}
            <button class="r-btn amber sm" on:click={() => openNative(c)} disabled={!$capture.available}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/></svg>
              Open
            </button>
          {:else}
            <button class="r-btn ghost sm" on:click={() => copyUrl(c)}>
              {#if copiedId === c.id}
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M20 6 9 17l-5-5"/></svg>
                Copied
              {:else}
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                Copy URL
              {/if}
            </button>
            <button class="r-iconbtn" title="Show QR — scan to open on another device" aria-label="Show QR code" on:click={() => showQr(c)} class:qr-on={qrOpen === c.id}>
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><path d="M14 14h3v3M20 14v.01M14 20h.01M17 20h.01M20 17v3"/></svg>
            </button>
          {/if}
          <button class="r-iconbtn ch-del" class:arm={delArm === c.id} title={delArm === c.id ? 'Click again to confirm' : 'Delete channel'} on:click={() => remove(c)} disabled={!$capture.available}>
            {#if delArm === c.id}
              <span class="ch-delconf r-mono">Sure?</span>
            {:else}
              <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 6 6 18M6 6l12 12"/></svg>
            {/if}
          </button>
        </div>
      </div>

      {#if qrOpen === c.id}
        <div class="ch-qr">
          <img class="ch-qr-img" src={qrData} alt="QR code to open {c.name} output" width="150" height="150" />
          <div class="ch-qr-info">
            <div class="r-lbl">Scan on the other device</div>
            <div class="ch-qr-url r-mono">{obsUrl(c)}</div>
            <div class="ch-qr-actions">
              <button class="r-btn ghost sm" on:click={() => copyUrl(c)}>{copiedId === c.id ? 'Copied ✓' : 'Copy URL'}</button>
              <button class="r-btn ghost sm" on:click={() => (qrOpen = null)}>Close</button>
            </div>
            <div class="ch-qr-hint r-mono">Open Camera / a QR app on the phone or kiosk and point it here. Same Wi-Fi network required.</div>
          </div>
        </div>
      {/if}
    {/each}

    {#if channels.length === 0}
      <div class="r-row"><span class="r-empty">No output channels yet — add one below.</span></div>
    {/if}
  </div>

  <!-- Add a channel -->
  <div class="ch-add">
    <div class="ch-add-mark">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>
    </div>
    <div class="ch-add-title">Add New Output Channel</div>
    <div class="ch-add-sub r-dim">Configure an HDMI display or a networked OBS / kiosk source for your venue.</div>
    <div class="ch-add-form">
      <input class="r-input" placeholder="New channel name" bind:value={newName} on:keydown={(e) => e.key === 'Enter' && add()} disabled={!$capture.available} />
      <select class="r-select" bind:value={newTarget} disabled={!$capture.available}>
        <option value="native_window">native window (HDMI)</option>
        <option value="network_client">network client (OBS/kiosk)</option>
      </select>
      <button class="r-btn amber" on:click={add} disabled={!$capture.available || !newName.trim()}>Add channel</button>
    </div>
  </div>

  {#if error}<div class="ch-error" role="alert">{error}</div>{/if}

  <!-- Preacher's stage remote -->
  <div class="r-tile ch-stage">
    <div class="ch-stage-info">
      <div class="ch-stage-mark">
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true"><rect x="7" y="2" width="10" height="20" rx="2"/><path d="M11 19h2"/></svg>
      </div>
      <div class="ch-stage-txt">
        <div class="ch-stage-title">Preacher's stage remote</div>
        <div class="ch-stage-sub r-dim">Open the live verse on a phone or iPad — big and readable, updates in real time. Scan the QR (same Wi-Fi) or open <code class="r-mono">{stageUrl()}</code>.</div>
        <div class="ch-stage-actions">
          <button class="r-btn amber sm" on:click={showStageQr}>{stageQrOpen ? 'Hide QR' : 'Show QR'}</button>
          <button class="r-btn ghost sm" on:click={copyStage}>{copiedStage ? 'Copied ✓' : 'Copy link'}</button>
        </div>
      </div>
    </div>
    {#if stageQrOpen}
      <img class="ch-stage-qr" src={stageQr} alt="QR code to open the stage remote" width="164" height="164" />
    {/if}
  </div>

  <!-- Real-data stat strip -->
  <div class="r-tile ch-foot">
    <div class="ch-stats">
      <div class="ch-stat">
        <span class="r-lbl">Channels</span>
        <b class="r-mono">{channels.length} configured</b>
      </div>
      <span class="ch-div"></span>
      <div class="ch-stat">
        <span class="r-lbl">Displays</span>
        <b class="r-mono">{monitors.length} detected</b>
      </div>
    </div>
    <div class="ch-notes r-mono">
      <div><b>This machine:</b> <code>{lanIp}</code> — network outputs use this address so other devices can reach them.</div>
      <div><b>Kiosk / another screen:</b> tap the <b>QR</b> button on a network channel and scan it on the phone/kiosk — it opens the live output, no typing. Same Wi-Fi required.</div>
      <div><b>OBS / vMix:</b> add a <b>Browser Source</b> with the copied URL (1920×1080). Live-updates over the kiosk WebSocket on :8031.</div>
      <div><b>HDMI:</b> set the channel's <b>Display</b>, then “Open” for a borderless fullscreen window on that screen.</div>
    </div>
  </div>
</div>

<style>
  .ch-view{ display:flex; flex-direction:column; gap:18px; }

  .ch-actionbar{ display:flex; align-items:center; justify-content:flex-end; gap:12px; }
  .ch-count{ font-size:11px; color:var(--v-faint); }

  /* Column labels — grid mirrors each row */
  .ch-head{
    display:grid; align-items:center;
    grid-template-columns:minmax(180px,1.3fr) minmax(150px,1fr) minmax(200px,1.4fr) auto;
    gap:16px; padding:0 18px; }
  .ch-head .c-act{ text-align:right; }

  .ch-list{ display:flex; flex-direction:column; gap:10px; }

  .ch-row{
    display:grid; align-items:center;
    grid-template-columns:minmax(180px,1.3fr) minmax(150px,1fr) minmax(200px,1.4fr) auto;
    gap:16px; padding-left:26px; }

  .c-name{ display:flex; flex-direction:column; gap:3px; min-width:0; }
  .c-name b{ font-size:14px; font-weight:600; color:var(--v-txt);
    white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .ch-target{ font-size:10px; letter-spacing:.05em; text-transform:uppercase; color:var(--v-faint); }

  .c-tmpl :global(.r-select){ height:36px; }

  .c-out{ display:flex; align-items:center; gap:9px; min-width:0; color:var(--v-dim); }
  .c-out :global(.r-select){ height:36px; min-width:0; }
  .ch-oico{ flex:0 0 auto; color:var(--v-faint); }
  .ch-nettxt{ font-size:11px; color:var(--v-dim); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }

  .c-act{ display:flex; align-items:center; justify-content:flex-end; gap:8px; }
  .ch-del:hover:not(:disabled){ color:var(--v-rose); border-color:rgba(244,113,139,.4); }
  .ch-del.arm{ width:auto; padding:0 9px; color:var(--v-rose); border-color:var(--v-rose); background:var(--v-rose-soft); }
  .ch-delconf{ font-size:9px; font-weight:700; letter-spacing:.04em; }
  .ch-del:disabled{ opacity:.4; cursor:not-allowed; }
  .c-act :global(.qr-on){ color:var(--v-amber); border-color:rgba(245,166,35,.4); }

  /* QR panel (scan to open on another device) */
  .ch-qr{ display:flex; gap:18px; align-items:center; padding:16px 18px 16px 26px; margin-top:-4px;
    background:var(--v-surf); border:1px solid rgba(245,166,35,.28); border-radius:12px; }
  .ch-qr-img{ border-radius:8px; background:#fff; padding:6px; flex:0 0 auto; }
  .ch-qr-info{ min-width:0; display:flex; flex-direction:column; gap:8px; }
  .ch-qr-url{ font-size:11px; color:var(--v-amber); word-break:break-all; }
  .ch-qr-actions{ display:flex; gap:8px; }
  .ch-qr-hint{ font-size:10px; color:var(--v-faint); line-height:1.6; }

  /* Preacher's stage remote card */
  .ch-stage{ display:flex; align-items:center; justify-content:space-between; gap:20px; padding:18px 20px; flex-wrap:wrap; }
  .ch-stage-info{ display:flex; gap:14px; min-width:0; flex:1 1 320px; }
  .ch-stage-mark{ width:42px; height:42px; border-radius:11px; flex:0 0 auto; display:grid; place-items:center;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-amber); }
  .ch-stage-txt{ min-width:0; }
  .ch-stage-title{ font-family:var(--f-head); font-size:17px; font-weight:700; color:var(--v-txt); }
  .ch-stage-sub{ font-size:12.5px; line-height:1.6; margin-top:3px; }
  .ch-stage-sub code{ color:var(--v-amber); font-size:11px; }
  .ch-stage-actions{ display:flex; gap:8px; margin-top:11px; }
  .ch-stage-qr{ border-radius:10px; background:#fff; padding:7px; flex:0 0 auto; }

  /* Dashed add card */
  .ch-add{
    display:flex; flex-direction:column; align-items:center; text-align:center; gap:6px;
    padding:26px 20px; border:2px dashed var(--v-line2); border-radius:16px;
    background:var(--v-surf); transition:border-color .2s, background .2s; }
  .ch-add:hover{ border-color:rgba(245,166,35,.35); background:var(--v-surf2); }
  .ch-add:focus-within, .ch-add:hover{ border-color:rgba(245,166,35,.4); background:var(--v-surf2); }
  .ch-add-mark{ width:48px; height:48px; border-radius:50%; display:grid; place-items:center;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-amber); margin-bottom:4px; }
  .ch-add-title{ font-family:var(--f-head); font-size:19px; font-weight:700; color:var(--v-txt); }
  .ch-add-sub{ font-size:13px; }
  .ch-add-form{ display:flex; gap:10px; align-items:center; margin-top:12px; width:100%;
    max-width:560px; justify-content:center; flex-wrap:wrap; }
  .ch-add-form :global(.r-input){ flex:1 1 200px; min-width:160px; }
  .ch-add-form :global(.r-select){ flex:0 1 210px; }

  .ch-error{ color:var(--v-rose); font-size:12px; }

  /* Footer stat strip */
  .ch-foot{ display:flex; align-items:flex-start; justify-content:space-between; gap:24px;
    padding:18px 20px; flex-wrap:wrap; }
  .ch-stats{ display:flex; align-items:center; gap:20px; flex:0 0 auto; }
  .ch-stat{ display:flex; flex-direction:column; gap:5px; }
  .ch-stat b{ font-size:15px; color:var(--v-txt); font-weight:600; }
  .ch-div{ width:1px; align-self:stretch; background:var(--v-line2); }
  .ch-notes{ flex:1 1 340px; min-width:0; font-size:10.5px; line-height:1.7; color:var(--v-dim); }
  .ch-notes b{ color:var(--v-txt); font-weight:600; }
  .ch-notes code{ color:var(--v-amber); }

  @media (max-width:820px){
    .ch-head{ display:none; }
    .ch-row{ grid-template-columns:1fr; gap:12px; padding:16px 16px 16px 26px; align-items:stretch; }
    .c-out :global(.r-select){ width:100%; }
    .c-act{ justify-content:flex-start; }
    .ch-foot{ flex-direction:column; }
    .ch-div{ display:none; }
    .ch-stats{ gap:24px; }
  }
</style>
