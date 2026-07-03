<script>
  import { onMount } from 'svelte';
  import { capture, listServices, serviceDetail, endService, exportService } from '../stores/capture.js';

  let exportMsg = '';
  async function doExport() {
    if (!selected) return;
    try {
      const path = await exportService(selected.id);
      exportMsg = `Saved to ${path}`;
    } catch (e) {
      exportMsg = `Export failed: ${e}`;
    }
  }

  // Service history is local-first (CLAUDE.md) — transcripts, fired detections,
  // and operator overrides recorded to SQLite during a service, read back here.
  let services = [];
  let selected = null; // { id, title } of the open detail
  let detail = null; // { transcripts, detections }
  let loading = false;

  async function refresh() {
    services = await listServices();
  }
  onMount(refresh);

  function fmtDur(secs) {
    const s = Math.round(secs || 0);
    const m = Math.floor(s / 60);
    return `${m}:${String(s % 60).padStart(2, '0')}`;
  }
  function fmtTs(secs) {
    return fmtDur(secs);
  }

  async function open(svc) {
    selected = svc;
    detail = null;
    exportMsg = '';
    loading = true;
    try {
      detail = await serviceDetail(svc.id);
    } catch (e) {
      detail = { transcripts: [], detections: [], error: String(e) };
    }
    loading = false;
  }
  function back() {
    selected = null;
    detail = null;
    refresh();
  }
  async function stopRecording() {
    await endService();
    refresh();
  }
</script>

{#if selected}
  <div class="panel">
    <div class="panel-title">
      <span><button class="btn-ghost" on:click={back}>← Library</button> &nbsp; {selected.title} · {selected.date}</span>
      <span style="display:flex; align-items:center; gap:10px;">
        <span class="count">{selected.verses} verses · {selected.overrides} overrides · {fmtDur(selected.duration_secs)}</span>
        <button class="btn-confirm" on:click={doExport}>Export .md</button>
      </span>
    </div>
    {#if exportMsg}<div style="font-family:var(--f-mono); font-size:11px; color:var(--green); margin-bottom:10px; word-break:break-all;">{exportMsg}</div>{/if}

    {#if loading}
      <div style="color:var(--text-faint); font-family:var(--f-mono); font-size:12px;">Loading…</div>
    {:else if detail}
      <div style="display:grid; grid-template-columns:1fr 320px; gap:16px; align-items:start;">
        <div>
          <div class="field-label">Transcript</div>
          {#if detail.transcripts.length}
            <div class="transcript" style="height:auto; max-height:360px; overflow:auto;">
              {#each detail.transcripts as t}
                <div style="margin-bottom:6px;">
                  <span style="color:var(--text-faint); font-family:var(--f-mono); font-size:11px;">{fmtTs(t.timestamp)} · {t.language}</span><br />
                  {t.text}
                </div>
              {/each}
            </div>
          {:else}
            <div style="color:var(--text-faint); font-size:13px;">No transcript recorded.</div>
          {/if}
        </div>
        <div>
          <div class="field-label">Detected verses <span style="color:var(--text-faint);">({detail.detections.length})</span></div>
          {#if detail.detections.length}
            {#each detail.detections as d}
              <div class="detect-card is-live" style="margin-bottom:7px;">
                <div class="detect-top">
                  <div class="detect-ref">{d.reference ?? 'unresolved'}</div>
                  <div class="detect-tag">{d.method}</div>
                </div>
                <div class="detect-bottom">
                  <span class="detect-conf">{d.confidence.toFixed(2)} · fired {fmtTs(d.fired_at)}</span>
                </div>
              </div>
            {/each}
          {:else}
            <div style="color:var(--text-faint); font-size:13px;">No verses fired.</div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{:else}
  <div class="panel">
    <div class="panel-title">
      Service library <span class="count">{services.length} service{services.length === 1 ? '' : 's'}</span>
    </div>
    {#if !$capture.available}
      <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); margin-bottom:10px;">backend not attached</div>
    {/if}
    <table class="data-table">
      <tr><th>Date</th><th>Title</th><th>Duration</th><th>Verses detected</th><th>Overrides</th><th></th></tr>
      {#if services.length}
        {#each services as s, i}
          <tr>
            <td class="mono" style={i === 0 ? 'color:var(--green);' : ''}>{s.date}</td>
            <td>{s.title}</td>
            <td class="mono">{fmtDur(s.duration_secs)}</td>
            <td class="mono">{s.verses}</td>
            <td class="mono">{s.overrides}</td>
            <td><button class="btn-ghost" on:click={() => open(s)}>Open</button></td>
          </tr>
        {/each}
      {:else}
        <tr><td colspan="6" style="color:var(--text-faint); font-size:13px; padding:16px 10px;">No services yet — start listening in Settings to record one.</td></tr>
      {/if}
    </table>
    <div class="controls">
      <button class="ctrl-btn" on:click={refresh} disabled={!$capture.available}>Refresh</button>
      <button class="ctrl-btn" on:click={stopRecording} disabled={!$capture.available}>End current service</button>
    </div>
  </div>
{/if}
