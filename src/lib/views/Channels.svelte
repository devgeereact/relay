<script>
  import { onMount } from 'svelte';
  import {
    capture,
    templates,
    loadTemplates,
    listOutputChannels,
    setChannelTemplate,
    openOutput,
  } from '../stores/capture.js';

  // Channels are DB-backed and every output is freely assignable to any
  // template. Native window = fullscreen on this machine (HDMI). OBS/vMix =
  // add a Browser Source pointing at the channel's URL; it connects to the
  // kiosk WebSocket hub (:8031) and renders live. NDI is parked on the SDK.
  let channels = [];
  let error = '';
  let copiedId = null;

  async function refresh() {
    channels = await listOutputChannels();
  }
  onMount(async () => {
    await loadTemplates();
    await refresh();
  });

  const chipFor = (i) => ['var(--amber)', 'var(--teal)', 'var(--violet)', 'var(--rose)', 'var(--text-faint)'][i % 5];
  const obsUrl = (c) => `http://localhost:5032/output.html?template_id=${c.template_id ?? 1}&name=${encodeURIComponent(c.name)}`;

  async function assign(c, e) {
    const templateId = parseInt(e.target.value, 10);
    try {
      await setChannelTemplate(c.id, templateId);
      await refresh();
      error = '';
    } catch (err) {
      error = String(err);
    }
  }

  async function openNative(c) {
    try {
      await openOutput(c.template_id ?? 1, c.name);
      error = '';
    } catch (err) {
      error = String(err);
    }
  }

  async function copyUrl(c) {
    try {
      await navigator.clipboard.writeText(obsUrl(c));
      copiedId = c.id;
      setTimeout(() => (copiedId = null), 1500);
    } catch {
      /* clipboard blocked */
    }
  }
</script>

<div class="panel">
  <div class="panel-title">
    Output channels <span class="count">{channels.length} configured</span>
  </div>
  {#if !$capture.available}
    <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); margin-bottom:10px;">backend not attached</div>
  {/if}

  <table class="data-table">
    <tr><th>Channel</th><th>Render target</th><th>Template</th><th>Native</th><th>OBS / browser source</th></tr>
    {#each channels as c, i}
      <tr>
        <td><span class="chip" style="background:{chipFor(i)};"></span> {c.name}</td>
        <td class="mono">{c.render_target.replace('_', ' ')}</td>
        <td>
          <select class="select-mock" style="width:150px;" value={c.template_id} on:change={(e) => assign(c, e)} disabled={!$capture.available}>
            {#each $templates as t}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
        </td>
        <td><button class="btn-confirm" on:click={() => openNative(c)} disabled={!$capture.available}>Open</button></td>
        <td>
          <button class="btn-ghost" on:click={() => copyUrl(c)}>{copiedId === c.id ? 'Copied ✓' : 'Copy URL'}</button>
        </td>
      </tr>
    {/each}
  </table>
  {#if error}<div style="color:var(--red); font-size:12px; margin-top:10px;">{error}</div>{/if}

  <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-dim); margin-top:14px; line-height:1.7;">
    <div><b>OBS / vMix:</b> add a <b>Browser Source</b> with the channel's copied URL (1920×1080). It live-updates over the kiosk WebSocket on port 8031 — no NDI needed.</div>
    <div><b>Kiosk (Raspberry Pi):</b> open the same URL in Chromium kiosk mode on the LAN (replace <code>localhost</code> with this machine's IP).</div>
    <div><b>HDMI:</b> use “Open” for a borderless fullscreen window on a second display.</div>
  </div>
</div>
