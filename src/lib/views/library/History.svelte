<script>
  import { onMount } from 'svelte';
  import Loading from '../../ui/Loading.svelte';
  import { capture, listServices, serviceDetail, endService, exportService } from '../../stores/capture.js';

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

  // Keep the screen clean: 10 services per page, paginate the rest.
  let page = 0;
  const PER = 10;
  $: pageCount = Math.max(1, Math.ceil(services.length / PER));
  $: if (page > pageCount - 1) page = pageCount - 1;
  $: pageServices = services.slice(page * PER, page * PER + PER);

  async function refresh() {
    services = await listServices();
    page = 0;
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
  <!-- ══ DETAIL STATE ══ -->
  <div class="lib-view">
    <div class="lib-detail-top">
      <button class="r-btn ghost sm" on:click={back}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
        History
      </button>
      <div class="lib-detail-head">
        <div class="lib-detail-title">{selected.title}</div>
        <div class="lib-detail-date r-mono">{selected.date}</div>
      </div>
      <div class="lib-detail-actions">
        <span class="lib-detail-count r-mono">{selected.verses} verses · {selected.overrides} overrides · {fmtDur(selected.duration_secs)}</span>
        <button class="r-btn amber sm" on:click={doExport}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/></svg>
          Export .md
        </button>
      </div>
    </div>

    {#if exportMsg}<div class="lib-exportmsg r-mono">{exportMsg}</div>{/if}

    {#if loading}
      <Loading what="services" />
    {:else if detail}
      <div class="lib-detail-grid">
        <div class="lib-transcript-col">
          <div class="r-lbl lib-collabel">Transcript</div>
          {#if detail.transcripts.length}
            <div class="r-tile lib-transcript r-scroll">
              {#each detail.transcripts as t}
                <div class="lib-tline">
                  <span class="lib-tmeta r-mono">{fmtTs(t.timestamp)} · {t.language}</span>
                  <span class="lib-ttext">{t.text}</span>
                </div>
              {/each}
            </div>
          {:else}
            <div class="r-tile lib-emptytile"><span class="r-empty">No transcript recorded.</span></div>
          {/if}
        </div>

        <div class="lib-detect-col">
          <div class="r-lbl lib-collabel">Detected verses <span class="lib-collabel-n">({detail.detections.length})</span></div>
          {#if detail.detections.length}
            <div class="lib-detect-list">
              {#each detail.detections as d}
                <div class="r-tile lib-detect">
                  <div class="lib-detect-top">
                    <div class="lib-detect-ref">{d.reference ?? 'unresolved'}</div>
                    <span class="r-badge amber lib-detect-method">{d.method}</span>
                  </div>
                  <div class="lib-detect-bottom r-mono">
                    <span class="lib-detect-conf">conf {d.confidence.toFixed(2)}</span>
                    <span class="lib-detect-fired">fired {fmtTs(d.fired_at)}</span>
                  </div>
                </div>
              {/each}
            </div>
          {:else}
            <div class="r-tile lib-emptytile"><span class="r-empty">No verses fired.</span></div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{:else}
  <!-- ══ LIST STATE ══ -->
  <div class="lib-view">
    <div class="lib-actionbar">
      <p class="r-lead">Every processed service is recorded locally to SQLite — transcript, fired detections, and operator overrides — and read back here.</p>
      <div class="lib-actions">
        <button class="r-btn ghost" on:click={refresh} disabled={!$capture.available}>
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
          Refresh
        </button>
        <button class="r-btn danger" on:click={stopRecording} disabled={!$capture.available}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
          End current service
        </button>
      </div>
    </div>

    {#if !$capture.available}
      <div class="lib-warn"><span class="r-badge rose"><span class="bd"></span>Backend not attached</span></div>
    {/if}

    <!-- Real-data stat cards -->
    <div class="lib-stats">
      <div class="r-stat">
        <span class="r-lbl">Total Services</span>
        <div class="n">{services.length}</div>
      </div>
      <div class="r-stat cyan">
        <span class="r-lbl">Verses Detected</span>
        <div class="n">{services.reduce((a, s) => a + (s.verses || 0), 0)}</div>
      </div>
      <div class="r-stat rose">
        <span class="r-lbl">Operator Overrides</span>
        <div class="n">{services.reduce((a, s) => a + (s.overrides || 0), 0)}</div>
      </div>
    </div>

    <!-- Column labels -->
    <div class="lib-head r-lbl">
      <span class="c-date">Date</span>
      <span class="c-title">Service Title</span>
      <span class="c-dur">Duration</span>
      <span class="c-verses">Verses</span>
      <span class="c-over">Overrides</span>
      <span class="c-open"></span>
    </div>

    <!-- Service rows (10 per page) -->
    <div class="lib-list">
      {#if services.length}
        {#each pageServices as s, i}
          {@const gi = page * PER + i}
          <div class="r-row lib-row">
            <span class="bar" style="background:{gi === 0 ? 'var(--v-amber)' : 'var(--v-line2)'};"></span>
            <span class="c-date r-mono" class:is-latest={gi === 0}>{s.date}</span>
            <span class="c-title lib-svctitle">{s.title}</span>
            <span class="c-dur r-mono">{fmtDur(s.duration_secs)}</span>
            <span class="c-verses"><span class="lib-pill r-mono">{s.verses}</span></span>
            <span class="c-over r-mono">{s.overrides}</span>
            <span class="c-open">
              <button class="r-iconbtn lib-openbtn" title="Open service" on:click={() => open(s)}>
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 18l6-6-6-6"/></svg>
              </button>
            </span>
          </div>
        {/each}
      {:else}
        <div class="r-row"><span class="r-empty">No services yet — press Start listening on the Live tab to record one.</span></div>
      {/if}
    </div>

    {#if pageCount > 1}
      <div class="lib-pager">
        <span class="r-mono">Showing {page * PER + 1}–{Math.min(services.length, page * PER + PER)} of {services.length}</span>
        <div class="pg">
          <button class="pgbtn" disabled={page === 0} on:click={() => (page -= 1)} aria-label="Previous page">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 18l-6-6 6-6"/></svg>
          </button>
          <span class="pgnum r-mono">Page {page + 1} / {pageCount}</span>
          <button class="pgbtn" disabled={page >= pageCount - 1} on:click={() => (page += 1)} aria-label="Next page">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 18l6-6-6-6"/></svg>
          </button>
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .lib-view{ display:flex; flex-direction:column; gap:18px; max-width:1080px; }

  /* ── List: action bar ── */
  .lib-actionbar{ display:flex; align-items:flex-end; justify-content:space-between; gap:24px; flex-wrap:wrap; }
  .lib-actionbar .r-lead{ margin:0; }
  .lib-actions{ display:flex; gap:10px; flex-shrink:0; }

  .lib-warn{ margin-top:-6px; }

  /* ── Stat cards ── */
  .lib-stats{ display:grid; grid-template-columns:repeat(3, 1fr); gap:14px; }

  /* ── Table-like list ── */
  .lib-head{
    display:grid; grid-template-columns:130px 1fr 100px 84px 90px 46px;
    align-items:center; gap:16px; padding:0 18px;
  }
  .lib-list{ display:flex; flex-direction:column; gap:8px; }
  .lib-row{
    display:grid; grid-template-columns:130px 1fr 100px 84px 90px 46px;
    gap:16px; cursor:default;
  }
  .c-verses, .c-over{ text-align:center; }
  .lib-head .c-verses, .lib-head .c-over{ text-align:center; }
  .c-open{ display:flex; justify-content:flex-end; }

  .c-date{ color:var(--v-dim); font-size:12px; }
  .c-date.is-latest{ color:var(--v-amber); }
  .lib-svctitle{ font-weight:600; color:var(--v-txt); font-size:14px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .c-dur{ color:var(--v-dim); font-size:12px; }
  .c-over{ color:var(--v-dim); font-size:12px; }
  .lib-pill{
    display:inline-block; min-width:30px; text-align:center; padding:3px 9px; border-radius:99px;
    background:var(--v-cyan-soft); border:1px solid rgba(63,182,230,.32); color:var(--v-cyan); font-size:11px;
  }
  .lib-openbtn svg{ transition:transform .15s; }
  .lib-row:hover .lib-openbtn{ color:var(--v-amber); border-color:var(--v-line2); }
  .lib-row:hover .lib-openbtn svg{ transform:translateX(2px); }

  /* ── Detail ── */
  .lib-detail-top{ display:flex; align-items:center; gap:16px; flex-wrap:wrap; }
  .lib-detail-head{ display:flex; align-items:baseline; gap:12px; min-width:0; flex:1; }
  .lib-detail-title{ font-family:var(--f-head); font-size:22px; font-weight:700; color:var(--v-txt); line-height:1.1;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .lib-detail-date{ font-size:12px; color:var(--v-faint); flex-shrink:0; }
  .lib-detail-actions{ display:flex; align-items:center; gap:12px; flex-shrink:0; }
  .lib-detail-count{ font-size:11px; color:var(--v-dim); }

  .lib-exportmsg{ font-size:11px; color:var(--v-emerald); word-break:break-all; margin-top:-8px; }

  .lib-detail-grid{ display:grid; grid-template-columns:1fr 340px; gap:16px; align-items:start; }
  .lib-collabel{ margin-bottom:10px; }
  .lib-collabel-n{ color:var(--v-faint); letter-spacing:0; }

  .lib-transcript{ padding:14px 16px; max-height:420px; overflow:auto; font-size:13px; line-height:1.6; }
  .lib-tline{ margin-bottom:12px; }
  .lib-tline:last-child{ margin-bottom:0; }
  .lib-tmeta{ display:block; font-size:10px; color:var(--v-faint); margin-bottom:3px; }
  .lib-ttext{ color:var(--v-dim); }

  .lib-detect-list{ display:flex; flex-direction:column; gap:9px; }
  .lib-detect{ padding:12px 14px; }
  .lib-detect-top{ display:flex; align-items:center; justify-content:space-between; gap:10px; margin-bottom:8px; }
  .lib-detect-ref{ font-family:var(--f-head); font-weight:700; font-size:15px; color:var(--v-txt); }
  .lib-detect-method{ text-transform:uppercase; }
  .lib-detect-bottom{ display:flex; align-items:center; justify-content:space-between; font-size:10px; color:var(--v-faint); }
  .lib-detect-conf{ color:var(--v-amber); }

  .lib-emptytile{ padding:18px 16px; }

  /* ── Pager ── */
  .lib-pager{ display:flex; align-items:center; justify-content:space-between; gap:14px; padding:4px 6px; }
  .lib-pager .r-mono{ font-size:10.5px; color:var(--v-faint); }
  .pg{ display:flex; align-items:center; gap:8px; }
  .pgnum{ font-size:11px; color:var(--v-dim); }
  .pgbtn{ width:32px; height:32px; display:grid; place-items:center; border-radius:8px; cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-dim); }
  .pgbtn:hover:not(:disabled){ color:var(--v-amber); border-color:var(--v-line2); }
  .pgbtn:disabled{ opacity:.35; cursor:not-allowed; }

  /* ── Responsive ── */
  @media (max-width:820px){
    .lib-stats{ grid-template-columns:1fr; }
    .lib-detail-grid{ grid-template-columns:1fr; }
    .lib-transcript{ max-height:300px; }
    .lib-actionbar{ flex-direction:column; align-items:stretch; }
    .lib-actions{ width:100%; }
    .lib-actions .r-btn{ flex:1; }

    .lib-head{ display:none; }
    .lib-row{
      grid-template-columns:1fr auto; grid-auto-rows:auto;
      grid-template-areas:
        "title open"
        "meta  meta";
      gap:8px 12px; padding:14px 16px;
    }
    .lib-row .c-title{ grid-area:title; }
    .lib-row .c-open{ grid-area:open; align-self:start; }
    .lib-row .c-date, .lib-row .c-dur, .lib-row .c-verses, .lib-row .c-over{
      grid-area:meta; display:inline-flex; align-items:center; text-align:left;
    }
    .lib-row .c-date::before, .lib-row .c-dur::before, .lib-row .c-over::before{ content:""; }
    .lib-row{ align-items:start; }
    .lib-row .c-date, .lib-row .c-dur, .lib-row .c-verses, .lib-row .c-over{ margin-right:14px; }
  }
</style>
