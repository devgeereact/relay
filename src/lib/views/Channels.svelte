<script>
  import { onMount } from 'svelte';
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
  } from '../stores/capture.js';

  // Channels are DB-backed and every output is freely assignable to any template.
  // native_window = borderless fullscreen on a chosen physical display (HDMI).
  // network_client (OBS/vMix/kiosk) = add a Browser Source pointing at the
  // channel URL; it live-updates over the kiosk WebSocket (:8031). NDI is parked.
  let channels = [];
  let monitors = [];
  let error = '';
  let copiedId = null;

  // Add-channel form state.
  let newName = '';
  let newTarget = 'native_window';

  async function refresh() {
    channels = await listOutputChannels();
  }
  onMount(async () => {
    await loadTemplates();
    monitors = await listMonitors();
    await refresh();
  });

  const chipFor = (i) => ['var(--amber)', 'var(--teal)', 'var(--violet)', 'var(--rose)', 'var(--text-faint)'][i % 5];
  const obsUrl = (c) => `http://localhost:5032/output.html?template_id=${c.template_id ?? 1}&name=${encodeURIComponent(c.name)}`;
  const isNative = (c) => c.render_target === 'native_window';

  async function assignTemplate(c, e) {
    try {
      await setChannelTemplate(c.id, parseInt(e.target.value, 10));
      await refresh();
      error = '';
    } catch (err) { error = String(err); }
  }

  async function assignDisplay(c, e) {
    const v = e.target.value;
    try {
      await setChannelDisplay(c.id, v === '' ? null : v);
      await refresh();
      error = '';
    } catch (err) { error = String(err); }
  }

  async function openNative(c) {
    try {
      await openChannelOutput(c.id);
      error = '';
    } catch (err) { error = String(err); }
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
    } catch (err) { error = String(err); }
  }

  async function remove(c) {
    if (!confirm(`Delete channel "${c.name}"?`)) return;
    try {
      await deleteChannel(c.id);
      await refresh();
      error = '';
    } catch (err) { error = String(err); }
  }

  async function copyUrl(c) {
    try {
      await navigator.clipboard.writeText(obsUrl(c));
      copiedId = c.id;
      setTimeout(() => (copiedId = null), 1500);
    } catch { /* clipboard blocked */ }
  }
</script>

<div class="panel">
  <div class="panel-title">
    Output channels <span class="count">{channels.length} configured · {monitors.length} display{monitors.length === 1 ? '' : 's'}</span>
  </div>
  {#if !$capture.available}
    <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); margin-bottom:10px;">backend not attached</div>
  {/if}

  <table class="data-table">
    <tr><th>Channel</th><th>Render target</th><th>Template</th><th>Display (HDMI)</th><th>Output</th><th></th></tr>
    {#each channels as c, i}
      <tr>
        <td><span class="chip" style="background:{chipFor(i)};"></span> {c.name}</td>
        <td class="mono">{c.render_target.replace('_', ' ')}</td>
        <td>
          <select class="select-mock" style="width:140px;" value={c.template_id} on:change={(e) => assignTemplate(c, e)} disabled={!$capture.available}>
            {#each $templates as t}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
        </td>
        <td>
          {#if isNative(c)}
            <select class="select-mock" style="width:170px;" value={c.display_target ?? ''} on:change={(e) => assignDisplay(c, e)} disabled={!$capture.available}>
              <option value="">Primary display</option>
              {#each monitors as m}
                <option value={String(m.index)}>{m.name} · {m.width}×{m.height}{m.primary ? ' (primary)' : ''}</option>
              {/each}
            </select>
          {:else}
            <span class="mono" style="color:var(--text-faint);">—</span>
          {/if}
        </td>
        <td>
          {#if isNative(c)}
            <button class="btn-confirm" on:click={() => openNative(c)} disabled={!$capture.available}>Open</button>
          {:else}
            <button class="btn-ghost" on:click={() => copyUrl(c)}>{copiedId === c.id ? 'Copied ✓' : 'Copy URL'}</button>
          {/if}
        </td>
        <td><button class="btn-ghost" title="Delete channel" on:click={() => remove(c)} disabled={!$capture.available}>✕</button></td>
      </tr>
    {/each}
  </table>

  <!-- Add a channel -->
  <div style="display:flex; gap:8px; align-items:center; margin-top:12px;">
    <input class="search-input" style="width:180px;" placeholder="New channel name" bind:value={newName} on:keydown={(e) => e.key === 'Enter' && add()} disabled={!$capture.available} />
    <select class="select-mock" style="width:150px;" bind:value={newTarget} disabled={!$capture.available}>
      <option value="native_window">native window (HDMI)</option>
      <option value="network_client">network client (OBS/kiosk)</option>
    </select>
    <button class="btn-confirm" on:click={add} disabled={!$capture.available || !newName.trim()}>Add channel</button>
  </div>

  {#if error}<div style="color:var(--red); font-size:12px; margin-top:10px;">{error}</div>{/if}

  <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-dim); margin-top:14px; line-height:1.7;">
    <div><b>HDMI:</b> set the channel's <b>Display</b>, then “Open” for a borderless fullscreen window pinned to that screen. Leave on “Primary” for this machine's main display.</div>
    <div><b>OBS / vMix:</b> add a <b>Browser Source</b> with the channel's copied URL (1920×1080). Live-updates over the kiosk WebSocket on port 8031 — no NDI needed.</div>
    <div><b>Kiosk (Raspberry Pi):</b> open the same URL in Chromium kiosk mode on the LAN (replace <code>localhost</code> with this machine's IP).</div>
  </div>
</div>
