<script>
  // Output channels — every one a render target of the SAME template engine.
  // Nothing here branches on channel type: native_window vs network_client
  // changes where pixels land, never how content is formatted. That is what
  // templates are for (docs/SPEC.md §5).
  //
  // ── The online light is computed, never stored ────────────────────────────
  //
  // `output_channels.status` exists in the schema and is a trap: both INSERTs
  // hardcode 'offline' and nothing in the codebase ever updates it, so it read
  // `offline` for every channel forever — including one filling a projector.
  // This screen asks `channel_status` instead, which derives liveness from two
  // facts the running app actually has: which output windows are open, and how
  // many kiosk clients are subscribed to each template.
  import { onMount, onDestroy } from 'svelte';
  import QRCode from 'qrcode';
  // Was: `error = String(err)`, rendered in a MONOSPACE font, five times over — a raw
  // Rust Err string shown to a church volunteer who has never seen one.
  import ErrorState from '../ui/ErrorState.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Loading from '../ui/Loading.svelte';
  import TemplateRender from '../TemplateRender.svelte';
  import {
    capture,
    templates,
    live,
    liveContent,
    liveTemplateOverride,
    loadTemplates,
    listOutputChannels,
    setChannelTemplate,
    listMonitors,
    openChannelOutput,
    closeChannelOutput,
    channelStatus,
    setChannelDisplay,
    addChannel,
    deleteChannel,
    localIp,
  } from '../stores/capture.js';

  let channels = [];
  // "Loading" vs "empty" — listOutputChannels swallows to [], so without this the
  // empty-state ("No output channels yet") flashes during a normal cold open.
  let loading = true;
  let monitors = [];
  let status = {}; // channel id → { online, clients, detail, supported }
  let error = null; // the TYPED error from Rust — ErrorState decides what to show
  let copiedId = null;
  let lanIp = 'localhost';
  let qrOpen = null;
  let qrData = '';

  let filter = 'all'; // all | native_window | network_client | ndi_encode
  let q = '';
  let selId = null;
  let showAdd = false;
  let newName = '';
  let newTarget = 'native_window';

  async function refresh() {
    channels = await listOutputChannels();
    if (selId && !channels.some((c) => c.id === selId)) selId = null;
  }
  async function pollStatus() {
    const rows = await channelStatus();
    status = Object.fromEntries(rows.map((r) => [r.id, r]));
  }

  // Liveness is polled, not pushed: a kiosk connecting or a window closing raises
  // no event Relay listens for, so the honest options are polling or a status
  // that goes stale. 2s is well under the time an operator takes to look away and
  // back, and the call is two in-memory reads.
  let poll;
  onMount(async () => {
    // Guarded: an unguarded reject here aborted mount before the poll was ever
    // scheduled, leaving status blank with no reason shown.
    try {
      await loadTemplates();
      monitors = await listMonitors();
      lanIp = (await localIp()) || 'localhost';
      await refresh();
      await pollStatus();
    } catch (e) {
      error = e; // the TYPED error; ErrorState humanises it (matches act())
    } finally {
      loading = false;
    }
    poll = setInterval(pollStatus, 2000);
  });
  onDestroy(() => clearInterval(poll));

  const isNative = (c) => c.render_target === 'native_window';
  const isNdi = (c) => c.render_target === 'ndi_encode';
  const obsUrl = (c) =>
    `http://${lanIp}:8032/output.html?template_id=${c.template_id ?? 1}&name=${encodeURIComponent(c.name)}`;
  const templateOf = (c) => $templates.find((t) => t.id === c.template_id) || null;
  const monitorOf = (c) => {
    const i = parseInt(c.display_target ?? '', 10);
    return Number.isFinite(i) ? monitors.find((m) => m.index === i) || null : null;
  };
  /** The kind label shown in the TYPE column. */
  const kindOf = (c) =>
    isNative(c) ? 'Native window' : isNdi(c) ? 'NDI' : 'Network client';
  const transportOf = (c) =>
    isNative(c) ? 'HDMI / display' : isNdi(c) ? 'unavailable' : 'WebSocket';

  $: counts = {
    all: channels.length,
    native_window: channels.filter(isNative).length,
    network_client: channels.filter((c) => c.render_target === 'network_client').length,
    ndi_encode: channels.filter(isNdi).length,
  };
  $: shown = channels
    .filter((c) => filter === 'all' || c.render_target === filter)
    .filter((c) => !q.trim() || c.name.toLowerCase().includes(q.trim().toLowerCase()));
  $: sel = channels.find((c) => c.id === selId) || null;
  $: selStatus = sel ? status[sel.id] : null;
  // Names `lanIp` directly so Svelte re-runs it when the address resolves. Via
  // `{obsUrl(sel)}` it was only ever correct by luck of ordering — the same trap
  // the stage-remote URL fell into, one selection away from showing `localhost`
  // to someone about to type it into a phone.
  $: selAddr = sel
    ? `http://${lanIp}:8032/output.html?template_id=${sel.template_id ?? 1}&name=${encodeURIComponent(sel.name)}`
    : '';
  $: onlineCount = channels.filter((c) => status[c.id]?.online).length;

  async function showQr(c) {
    if (qrOpen === c.id) { qrOpen = null; return; }
    try {
      qrData = await QRCode.toDataURL(obsUrl(c), { width: 190, margin: 1, color: { dark: '#0a0a0a', light: '#ffffff' } });
      qrOpen = c.id;
    } catch (e) {
      // The URL is shown on the row regardless, so a failed QR is cosmetic — but
      // log it rather than swallow, so a dead-looking button isn't invisible.
      console.warn('QR generation failed', e);
    }
  }

  // REACTIVE, not a function call in the markup.
  //
  // As `{stageUrl()}` this rendered once, before `local_ip` resolved, and then
  // never again — Svelte tracks the identifiers in a template expression, and
  // that one names `stageUrl`, not `lanIp`. So the operator was shown
  // `http://localhost:8032/stage.html` and told to open it on a phone, where
  // localhost is the phone. The QR was built on click and so was correct; only
  // the address anyone would actually type was wrong.
  $: stageUrl = `http://${lanIp}:8032/stage.html`;
  let stageQr = '';
  let stageQrOpen = false;
  let copiedStage = false;
  async function showStageQr() {
    if (stageQrOpen) { stageQrOpen = false; return; }
    try {
      stageQr = await QRCode.toDataURL(stageUrl, { width: 200, margin: 1, color: { dark: '#0a0a0a', light: '#ffffff' } });
      stageQrOpen = true;
    } catch (e) {
      console.warn('QR generation failed', e);
    }
  }
  async function copyStage() {
    try {
      await navigator.clipboard.writeText(stageUrl);
      copiedStage = true;
      setTimeout(() => (copiedStage = false), 1500);
    } catch (e) {
      // The address is on screen to type by hand; log rather than swallow.
      console.warn('Clipboard write blocked', e);
    }
  }

  /** Run a mutation, refresh, and hand any error to the ONE humaniser. */
  async function act(fn) {
    try {
      await fn();
      await refresh();
      await pollStatus();
      error = null;
    } catch (err) {
      error = err;
    }
  }

  const assignTemplate = (c, e) => act(() => setChannelTemplate(c.id, parseInt(e.target.value, 10)));
  const assignDisplay = (c, e) => act(() => setChannelDisplay(c.id, e.target.value === '' ? null : e.target.value));
  const openNative = (c) => act(() => openChannelOutput(c.id));
  const closeNative = (c) => act(() => closeChannelOutput(c.id));

  async function add() {
    const name = newName.trim();
    if (!name) return;
    await act(() => addChannel(name, newTarget, 1));
    newName = '';
    newTarget = 'native_window';
    showAdd = false;
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
    await act(() => deleteChannel(c.id));
  }

  async function copyUrl(c) {
    try {
      await navigator.clipboard.writeText(obsUrl(c));
      copiedId = c.id;
      setTimeout(() => (copiedId = null), 1500);
    } catch (e) {
      console.warn('Clipboard write blocked', e);
    }
  }

  // A channel's preview shows its OWN template with stand-in content — the same
  // renderer the wall uses, so it is WYSIWYG rather than a drawing of one.
  const PREVIEW = { reference: 'John 3:16', text: 'For God so loved the world…', translation: 'KJV' };
</script>

<div class="ch-shell">
  <section class="ch-main">
    <!-- Filter tabs. Relay's real taxonomy is the render target, so these ARE the
         render targets — the reference's separate "Network" and "Browser Sources"
         tabs are one thing here (a browser source IS a network client), and
         splitting them would imply a distinction the engine does not make. -->
    <div class="ch-tabs">
      {#each [['all', 'All Channels'], ['network_client', 'Network'], ['native_window', 'Native Windows'], ['ndi_encode', 'NDI']] as [key, label]}
        <button class="ch-tab" class:on={filter === key} on:click={() => (filter = key)}>
          {label}<span class="ch-tabn r-mono">{counts[key]}</span>
        </button>
      {/each}
      <span class="ch-spring"></span>
      <button class="r-btn primary sm" on:click={() => (showAdd = !showAdd)} disabled={!$capture.available}>
        ＋ Add Channel
      </button>
    </div>

    <div class="ch-toolbar">
      <div class="ch-search">
        <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
        <input placeholder="Search channels…" bind:value={q} aria-label="Search channels" />
      </div>
      {#if !$capture.available}
        <span class="r-badge rose"><span class="bd"></span>Backend not attached</span>
      {:else}
        <!-- GREEN, not amber. Green is "confirmed / connected"; amber means
             something is on the wall, and a channel being online does not put it
             there. -->
        <span class="r-badge green"><span class="bd"></span>{onlineCount} of {channels.length} live</span>
      {/if}
    </div>

    {#if showAdd}
      <div class="ch-addbar">
        <input class="r-input" placeholder="New channel name" bind:value={newName} on:keydown={(e) => e.key === 'Enter' && add()} />
        <select class="r-select" bind:value={newTarget}>
          <option value="native_window">Native window (HDMI / display)</option>
          <option value="network_client">Network client (OBS / kiosk)</option>
        </select>
        <button class="r-btn primary sm" on:click={add} disabled={!newName.trim()}>Add</button>
        <button class="r-btn ghost sm" on:click={() => (showAdd = false)}>Cancel</button>
      </div>
    {/if}

    <div class="ch-tablewrap r-scroll">
      {#if loading}
        <Loading what="output channels" />
      {:else if shown.length}
        <div class="ch-thead r-lbl">
          <span class="ch-th-n">#</span>
          <span>Channel</span>
          <span>Type</span>
          <span class="ch-th-tpl">Template</span>
          <span class="ch-th-out">Output target</span>
          <span>Status</span>
          <span></span>
        </div>

        {#each shown as c (c.id)}
          {@const st = status[c.id]}
          {@const mon = monitorOf(c)}
          <div class="ch-row" class:sel={c.id === selId}
            on:click={() => (selId = selId === c.id ? null : c.id)} role="button" tabindex="0"
            on:keydown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = selId === c.id ? null : c.id; }
            }}>
            <span class="ch-num r-mono">{channels.findIndex((x) => x.id === c.id) + 1}</span>

            <span class="ch-namecell">
              <span class="ch-ico" class:live={st?.online}>
                {#if isNative(c)}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
                {:else if isNdi(c)}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M4 12h16M12 4v16"/><circle cx="12" cy="12" r="9"/></svg>
                {:else}
                  <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a15 15 0 0 1 0 18M12 3a15 15 0 0 0 0 18"/></svg>
                {/if}
              </span>
              <span class="ch-nametxt">
                <span class="ch-name">{c.name}</span>
                <span class="ch-sub r-mono">{st?.detail ?? '—'}</span>
              </span>
            </span>

            <span class="ch-ty r-mono">{kindOf(c)}<i>{transportOf(c)}</i></span>

            <span class="ch-tpl r-mono">{templateOf(c)?.name ?? 'None'}</span>

            <!-- Resolution is shown ONLY for a native channel with a display
                 assigned, because that is the only case where Relay knows one:
                 it is the monitor's size, read from the OS. A networked channel's
                 resolution is a property of the browser source at the other end,
                 which Relay has never been told. -->
            <span class="ch-out r-mono">
              {#if isNative(c)}
                {mon ? `${mon.width}×${mon.height}` : 'Primary display'}
              {:else if isNdi(c)}
                —
              {:else}
                :8032 / :8031
              {/if}
            </span>

            <span class="ch-status r-mono" class:on={st?.online} class:un={st && !st.supported}>
              <span class="bd"></span>{st ? (!st.supported ? 'UNAVAILABLE' : st.online ? 'LIVE' : 'IDLE') : '—'}
            </span>

            <span class="ch-rowbtns" on:click|stopPropagation role="presentation">
              {#if isNative(c)}
                {#if st?.online}
                  <button class="r-btn ghost sm" on:click={() => closeNative(c)}>Close</button>
                {:else}
                  <button class="r-btn ghost sm" on:click={() => openNative(c)} disabled={!$capture.available}>Open</button>
                {/if}
              {:else if !isNdi(c)}
                <button class="r-btn ghost sm" on:click={() => copyUrl(c)}>{copiedId === c.id ? 'Copied ✓' : 'Copy URL'}</button>
                <button class="r-iconbtn" title="Show QR — scan to open on another device" aria-label="Show QR code" on:click={() => showQr(c)} class:qr-on={qrOpen === c.id}>
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round" aria-hidden="true"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><path d="M14 14h3v3M20 14v.01M14 20h.01M17 20h.01M20 17v3"/></svg>
                </button>
              {/if}
            </span>
          </div>

          {#if qrOpen === c.id}
            <div class="ch-qr">
              <img class="ch-qr-img" src={qrData} alt="QR code to open {c.name} output" width="132" height="132" />
              <div class="ch-qr-info">
                <div class="r-lbl">Scan on the other device</div>
                <div class="ch-qr-url r-mono">{obsUrl(c)}</div>
                <div class="ch-qr-hint r-mono">Open Camera or a QR app and point it here. Same Wi-Fi required.</div>
              </div>
              <button class="r-btn ghost sm" on:click={() => (qrOpen = null)}>Close</button>
            </div>
          {/if}
        {/each}
      {:else}
        <EmptyState message={channels.length ? 'No channel matches this filter.' : 'No output channels yet — add one below.'} />
      {/if}

      <button class="ch-addcard" on:click={() => (showAdd = true)} disabled={!$capture.available}>
        <span class="ch-addmark">＋</span>
        <span class="ch-addttl">Add New Output Channel</span>
        <span class="ch-addsub">Configure a display for HDMI, or a networked OBS / kiosk source for your venue.</span>
      </button>
    </div>

    <ErrorState {error} />

    <!-- Preacher's stage remote — real, and not in the reference, but it is the
         one output a church sets up by hand every week. -->
    <div class="r-tile ch-stage">
      <div class="ch-stage-info">
        <div class="ch-stage-mark">
          <svg width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" aria-hidden="true"><rect x="7" y="2" width="10" height="20" rx="2"/><path d="M11 19h2"/></svg>
        </div>
        <div class="ch-stage-txt">
          <div class="ch-stage-title">Preacher's stage remote</div>
          <div class="ch-stage-sub r-dim">The live verse on a phone or iPad, updating in real time. Scan the QR (same Wi-Fi) or open <code class="r-mono">{stageUrl}</code>.</div>
        </div>
      </div>
      <div class="ch-stage-actions">
        <button class="r-btn primary sm" on:click={showStageQr}>{stageQrOpen ? 'Hide QR' : 'Show QR'}</button>
        <button class="r-btn ghost sm" on:click={copyStage}>{copiedStage ? 'Copied ✓' : 'Copy link'}</button>
      </div>
      {#if stageQrOpen}
        <img class="ch-stage-qr" src={stageQr} alt="QR code to open the stage remote" width="150" height="150" />
      {/if}
    </div>
  </section>

  <!-- ══ INSPECTOR ══ -->
  <aside class="ch-insp">
    {#if !sel}
      <div class="ch-insphead"><span class="ch-inspttl">Channel</span></div>
      <div class="ch-empty r-empty">Pick a channel to configure it.</div>
    {:else}
      <div class="ch-insphead">
        <span class="ch-inspttl">{sel.name}</span>
        <span class="ch-status r-mono" class:on={selStatus?.online} class:un={selStatus && !selStatus.supported}>
          <span class="bd"></span>{selStatus ? (!selStatus.supported ? 'UNAVAILABLE' : selStatus.online ? 'LIVE' : 'IDLE') : '—'}
        </span>
        <button class="r-iconbtn ch-close" aria-label="Close panel" on:click={() => (selId = null)}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="ch-inspbody r-scroll">
        <!-- Same renderer as the wall, showing the SAME content the wall is
             showing: when something is live it mirrors the program (through this
             channel's template + any content-type override), exactly as this
             output is rendering it right now. Only when nothing is live does it
             fall back to a sample so the template is still previewable. This is
             what makes "select an output" agree with what is actually on screen. -->
        <div class="ch-preview">
          <TemplateRender
            template={($live ? $liveTemplateOverride : null) ?? templateOf(sel) ?? {}}
            content={$live ? $liveContent : PREVIEW} />
        </div>
        <p class="ch-prevnote r-mono">{$live ? 'Live — mirroring the program' : 'Sample — nothing on screen'}</p>

        <div class="r-lbl ch-flbl">Channel info</div>
        <dl class="ch-info">
          <dt>Type</dt><dd>{kindOf(sel)}</dd>
          <dt>Transport</dt><dd>{transportOf(sel)}</dd>
          <dt>Template</dt><dd>{templateOf(sel)?.name ?? 'None'}</dd>
          {#if isNative(sel)}
            <dt>Display</dt><dd>{monitorOf(sel) ? `${monitorOf(sel).name} · ${monitorOf(sel).width}×${monitorOf(sel).height}` : 'Primary'}</dd>
          {:else if !isNdi(sel)}
            <dt>Address</dt><dd class="ch-addr">{selAddr}</dd>
            <dt>Clients</dt><dd>{selStatus?.clients ?? 0}</dd>
          {/if}
          <dt>State</dt><dd>{selStatus?.detail ?? 'Unknown'}</dd>
        </dl>

        <div class="r-lbl ch-flbl">Template</div>
        <select class="r-select ch-fin" value={sel.template_id} on:change={(e) => assignTemplate(sel, e)} disabled={!$capture.available}>
          {#each $templates as t (t.id)}
            <option value={t.id}>{t.name}</option>
          {/each}
        </select>

        {#if isNative(sel)}
          <div class="r-lbl ch-flbl">Display</div>
          <select class="r-select ch-fin" value={sel.display_target ?? ''} on:change={(e) => assignDisplay(sel, e)} disabled={!$capture.available}>
            <option value="">Primary display</option>
            {#each monitors as m (m.index)}
              <option value={String(m.index)}>{m.name} · {m.width}×{m.height}{m.primary ? ' (primary)' : ''}</option>
            {/each}
          </select>
        {/if}

        <div class="r-lbl ch-flbl">Actions</div>
        <div class="ch-actions">
          {#if isNative(sel)}
            {#if selStatus?.online}
              <button class="r-btn ghost sm" on:click={() => closeNative(sel)}>Close output</button>
            {:else}
              <button class="r-btn primary sm" on:click={() => openNative(sel)} disabled={!$capture.available}>Open output</button>
            {/if}
          {:else if !isNdi(sel)}
            <button class="r-btn ghost sm" on:click={() => copyUrl(sel)}>{copiedId === sel.id ? 'Copied ✓' : 'Copy URL'}</button>
            <button class="r-btn ghost sm" on:click={() => showQr(sel)}>Show QR</button>
          {/if}
          <button class="r-btn ghost sm ch-del" class:arm={delArm === sel.id} on:click={() => remove(sel)} disabled={!$capture.available}>
            {delArm === sel.id ? 'Click again to confirm' : 'Remove channel'}
          </button>
        </div>

        <!-- WHAT RELAY DOES NOT MEASURE.
             The reference puts a CHANNEL HEALTH panel here — bandwidth, dropped
             frames, uptime, latency, "Excellent". None of it exists: nothing in
             Relay times a delivery, counts a frame, or records a connect time.
             Inventing plausible numbers on a screen an operator uses to decide
             whether the projector is working would be the worst possible place
             to be decorative, so the panel states the limit instead. -->
        <div class="r-lbl ch-flbl">What this panel can tell you</div>
        <p class="ch-fhelp">
          Relay reports whether an output window is open and how many clients are
          connected. It does <b>not</b> measure latency, bandwidth, frame rate or
          dropped frames — nothing in the pipeline times or counts delivery, so any
          such figure here would be invented. A channel reading <b>LIVE</b> means
          something is attached, not that the picture is good.
        </p>
      </div>
    {/if}
  </aside>
</div>

<style>
  .ch-shell{ display:grid; grid-template-columns:minmax(0,1fr) 330px; gap:var(--v-sp-md);
    height:100%; min-height:0; }
  @media (max-width:1180px){ .ch-shell{ grid-template-columns:1fr; height:auto; } }

  .ch-main{ display:flex; flex-direction:column; min-height:0; gap:12px; }

  /* ── tabs ── */
  .ch-tabs{ display:flex; align-items:center; gap:6px; flex:0 0 auto; flex-wrap:wrap; }
  .ch-tab{ display:inline-flex; align-items:center; gap:7px; padding:7px 13px; border-radius:var(--v-r-md);
    background:var(--v-surf); border:1px solid var(--v-line); color:var(--v-dim); cursor:pointer;
    font-size:var(--v-fs-b2); font-weight:500; transition:.12s; }
  .ch-tab:hover{ border-color:var(--v-line2); color:var(--v-txt); }
  .ch-tab.on{ background:var(--v-accent-fill); border-color:var(--v-accent-fill); color:var(--v-accent-ink); }
  .ch-tabn{ font-size:var(--v-fs-cap); padding:1px 6px; border-radius:99px;
    background:var(--v-surf3); color:var(--v-dim); }
  .ch-tab.on .ch-tabn{ background:rgba(0,0,0,.28); color:var(--v-accent-ink); }
  .ch-spring{ flex:1; }

  .ch-toolbar{ display:flex; align-items:center; gap:10px; flex:0 0 auto; }
  .ch-search{ display:flex; align-items:center; gap:8px; background:var(--v-bg);
    border:1px solid var(--v-line2); border-radius:var(--v-r-md); padding:0 11px; height:32px;
    flex:0 1 280px; }
  .ch-search:focus-within{ border-color:var(--v-accent-line); box-shadow:0 0 0 3px var(--v-accent-soft); }
  .ch-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .ch-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none;
    color:var(--v-txt); font-size:var(--v-fs-b2); }
  .ch-search input::placeholder{ color:var(--v-faint); }
  .ch-toolbar .r-badge{ margin-left:auto; }

  .ch-addbar{ display:flex; gap:8px; align-items:center; flex:0 0 auto; padding:10px 12px;
    background:var(--v-surf); border:1px solid var(--v-accent-line); border-radius:var(--v-r-md); }
  .ch-addbar .r-input{ flex:1 1 200px; }

  /* ── table ── */
  .ch-tablewrap{ flex:1; min-height:0; overflow-y:auto; background:var(--v-surf);
    border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  .ch-thead, .ch-row{ display:grid;
    grid-template-columns:26px minmax(172px,1fr) 110px 124px 112px 100px 128px;
    align-items:center; gap:10px; padding:0 12px; }
  .ch-thead{ height:30px; position:sticky; top:0; z-index:2; background:var(--v-surf);
    border-bottom:1px solid var(--v-line); color:var(--v-faint); }
  .ch-th-n{ text-align:center; }
  /* Template and Output target drop first — both are shown in full in the
     inspector for the selected channel, so neither is the last copy. */
  @media (max-width:1520px){
    .ch-thead, .ch-row{ grid-template-columns:26px minmax(172px,1fr) 110px 124px 100px 128px; }
    .ch-out, .ch-th-out{ display:none; }
  }
  @media (max-width:1330px){
    .ch-thead, .ch-row{ grid-template-columns:26px minmax(150px,1fr) 110px 100px 128px; }
    .ch-tpl, .ch-th-tpl{ display:none; }
  }

  .ch-row{ min-height:52px; border-bottom:1px solid var(--v-line); cursor:pointer;
    transition:background .12s, box-shadow .12s; }
  .ch-row:last-child{ border-bottom:0; }
  .ch-row:hover{ background:var(--v-surf2); }
  /* Amethyst = selected. Never amber: amber means live on the wall, and selecting
     a channel to configure it puts nothing anywhere. */
  .ch-row.sel{ background:var(--v-accent-soft); box-shadow:inset 3px 0 0 var(--v-accent); }
  .ch-num{ font-size:var(--v-fs-lbl); color:var(--v-faint); text-align:center; }

  .ch-namecell{ display:flex; align-items:center; gap:10px; min-width:0; }
  .ch-ico{ width:30px; height:30px; border-radius:var(--v-r-md); display:grid; place-items:center;
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-faint); flex:0 0 auto; }
  .ch-ico.live{ color:var(--v-emerald); border-color:var(--v-emerald-soft); background:var(--v-emerald-soft); }
  .ch-nametxt{ min-width:0; }
  .ch-name{ display:block; font-size:var(--v-fs-b1); font-weight:500; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .ch-sub{ display:block; font-size:var(--v-fs-cap); color:var(--v-faint); margin-top:1px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-ty{ font-size:var(--v-fs-cap); color:var(--v-dim); min-width:0; }
  .ch-ty i{ display:block; font-style:normal; color:var(--v-faint); margin-top:1px; }
  .ch-tpl, .ch-out{ font-size:var(--v-fs-cap); color:var(--v-dim);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-status{ display:inline-flex; align-items:center; gap:6px; font-size:var(--v-fs-cap);
    letter-spacing:.06em; color:var(--v-faint); }
  .ch-status .bd{ width:6px; height:6px; border-radius:50%; background:currentColor; flex:0 0 auto; }
  /* Green = connected (the design sheet's own usage guide), not amber. */
  .ch-status.on{ color:var(--v-emerald); }
  .ch-status.un{ color:var(--v-500); }

  .ch-rowbtns{ display:flex; gap:5px; justify-content:flex-end; align-items:center; }

  .ch-qr{ display:flex; align-items:center; gap:14px; padding:12px;
    background:var(--v-bg); border-bottom:1px solid var(--v-line); }
  .ch-qr-img{ border-radius:var(--v-r-sm); flex:0 0 auto; }
  .ch-qr-info{ flex:1; min-width:0; }
  .ch-qr-url{ font-size:var(--v-fs-cap); color:var(--v-accent2); margin:4px 0;
    overflow-wrap:anywhere; }
  .ch-qr-hint{ font-size:var(--v-fs-cap); color:var(--v-faint); }

  /* ── stage remote ── */
  .ch-stage{ display:flex; align-items:center; gap:14px; flex-wrap:wrap; flex:0 0 auto; padding:13px 15px; }
  .ch-stage-info{ display:flex; align-items:center; gap:12px; flex:1; min-width:220px; }
  .ch-stage-mark{ width:34px; height:34px; border-radius:var(--v-r-md); display:grid; place-items:center;
    background:var(--v-accent-soft); color:var(--v-accent2); border:1px solid var(--v-accent-line); flex:0 0 auto; }
  .ch-stage-title{ font-size:var(--v-fs-b1); font-weight:600; color:var(--v-txt); }
  .ch-stage-sub{ font-size:var(--v-fs-b2); margin-top:2px; line-height:1.45; }
  .ch-stage-actions{ display:flex; gap:6px; flex:0 0 auto; }
  .ch-stage-qr{ border-radius:var(--v-r-sm); }

  /* ── inspector ── */
  .ch-insp{ display:flex; flex-direction:column; min-height:0; background:var(--v-surf);
    border:1px solid var(--v-line); border-radius:var(--v-r-lg); overflow:hidden; }
  .ch-insphead{ display:flex; align-items:center; gap:10px; padding:12px 14px;
    border-bottom:1px solid var(--v-line); flex:0 0 auto; }
  .ch-inspttl{ font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt);
    flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .ch-close{ width:26px; height:26px; flex:0 0 auto; }
  .ch-inspbody{ flex:1; min-height:0; overflow-y:auto; padding:14px; }

  /* position:relative is load-bearing — TemplateRender's root is
     position:absolute; inset:0, so without it the preview escapes this box and
     lays itself out against the page. It supplies its own container-type. */
  .ch-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md);
    border:1px solid var(--v-line2); overflow:hidden; background:var(--v-void); }
  .ch-prevnote{ margin:6px 0 0; font-size:var(--v-fs-cap); color:var(--v-faint); }

  .ch-flbl{ margin:15px 0 6px; }
  .ch-fin{ width:100%; }
  .ch-fhelp{ margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .ch-fhelp b{ color:var(--v-dim); font-weight:600; }

  .ch-info{ display:grid; grid-template-columns:auto 1fr; gap:5px 12px; margin:0;
    font-size:var(--v-fs-b2); }
  .ch-info dt{ color:var(--v-faint); }
  .ch-info dd{ margin:0; color:var(--v-txt); overflow-wrap:anywhere; }
  /* One line, truncated. Wrapping "anywhere" broke it mid-word into
     `output.h / tml?` — an address split across a line break invites being
     mis-typed, and Copy URL is right there for the real thing. */
  .ch-addr{ font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-accent2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-actions{ display:flex; flex-wrap:wrap; gap:6px; }
  .ch-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  .ch-del{ color:var(--v-rose); }
  .ch-del:hover:not(:disabled), .ch-del.arm{ border-color:var(--v-rose); background:var(--v-rose-soft); }

  .ch-empty{ margin:auto; padding:24px; text-align:center; }

  /* Sits at the end of the list, where the eye lands after reading it. */
  .ch-addcard{ display:flex; flex-direction:column; align-items:center; gap:5px;
    width:calc(100% - 24px); margin:12px; padding:20px 16px; cursor:pointer;
    background:transparent; border:1px dashed var(--v-line2); border-radius:var(--v-r-md);
    color:inherit; transition:.14s; }
  .ch-addcard:hover:not(:disabled){ border-color:var(--v-accent); background:var(--v-accent-soft); }
  .ch-addcard:disabled{ opacity:.45; cursor:not-allowed; }
  .ch-addmark{ width:28px; height:28px; border-radius:50%; display:grid; place-items:center;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-accent2);
    font-size:15px; line-height:1; }
  .ch-addttl{ font-size:var(--v-fs-b1); font-weight:600; color:var(--v-txt); }
  .ch-addsub{ font-size:var(--v-fs-b2); color:var(--v-faint); text-align:center; }
</style>
