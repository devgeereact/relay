<script>
  // The Outputs hub — one place to wire everything that puts pixels somewhere.
  // Three panes, one vocabulary (Decision §25):
  //
  //   Screens        every output target — a render target of the SAME template
  //                  engine. Nothing here branches on screen type: native_window
  //                  vs network_client changes where pixels land, never how
  //                  content is formatted. That is what templates are for.
  //   Content looks  the type → template default map ("when scripture fires,
  //                  which look does it wear on any screen that hasn't overridden
  //                  it"). THE one writer of that map, backed by the shared
  //                  `contentTemplates` store — three surfaces used to write it
  //                  with no shared state and silently overwrote each other.
  //   Sharing        the LAN address + the preacher's stage remote — the outputs
  //                  a church sets up by hand on other devices every week.
  //
  // ── The online light is computed, never stored ────────────────────────────
  //
  // `output_channels.status` exists in the schema and is a trap: both INSERTs
  // hardcode 'offline' and nothing in the codebase ever updates it, so it read
  // `offline` for every screen forever — including one filling a projector.
  // This screen asks `channel_status` instead, which derives liveness from facts
  // the running app actually has: which output windows are open, how many kiosk
  // clients are subscribed to each template — and, since the screens began
  // answering for themselves, whether each one has reported that it is still
  // PAINTING within the last few seconds. The first two are true of a frozen
  // projector; only the third can go false on its own.
  import { onMount } from 'svelte';
  import QRCode from 'qrcode';
  // The SAME rule Live uses. Two surfaces describing one screen must not be able
  // to reach different conclusions about it — that asymmetry is how this
  // repository has produced four separate bugs with one root cause.
  import { screenFault, FAULT_WORD } from '../outputHealth.js';
  // Was: `error = String(err)`, rendered in a MONOSPACE font, five times over — a raw
  // Rust Err string shown to a church volunteer who has never seen one.
  import ErrorState from '../ui/ErrorState.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Loading from '../ui/Loading.svelte';
  import TemplateRender from '../TemplateRender.svelte';
  import { CONTENT_KINDS, resolveOutputTemplate } from '../layers.js';
  import {
    capture,
    templates,
    live,
    liveContent,
    liveTemplateOverride,
    liveTemplatePinned,
    contentTemplates,
    setContentTemplate,
    loadTemplates,
    listOutputChannels,
    setChannelTemplate,
    listMonitors,
    openChannelOutput,
    closeChannelOutput,
    channelHealth,
    startChannelHealth,
    setChannelDisplay,
    addChannel,
    deleteChannel,
    localIp,
    defaultTemplateId,
    loadDefaultTemplate,
  } from '../stores/capture.js';

  // Which pane. 'screens' is where an operator lives; the inspector aside only
  // has meaning there, so the other two panes collapse to a single column.
  let view = 'screens'; // screens | looks | sharing

  let channels = [];
  // "Loading" vs "empty" — listOutputChannels swallows to [], so without this the
  // empty-state ("No screens yet") flashes during a normal cold open.
  let loading = true;
  let monitors = [];
  let error = null; // the TYPED error from Rust — ErrorState decides what to show
  let copiedId = null;
  let lanIp = 'localhost';
  let qrOpen = null;
  let qrData = '';

  let filter = 'all'; // all | native_window | network_client
  let q = '';
  let selId = null;
  let showAdd = false;
  let newName = '';
  let newTarget = 'native_window';

  async function refresh() {
    channels = await listOutputChannels();
    if (selId && !channels.some((c) => c.id === selId)) selId = null;
  }
  // Liveness is polled, not pushed: a kiosk connecting or a window closing raises
  // no event Relay listens for, so the honest options are polling or a status that
  // goes stale. The poll itself lives in the store and is started by the shell —
  // the Live pane and the degraded banner want the same answer, and three timers
  // asking one question would let three surfaces disagree about one screen.
  $: status = $channelHealth;
  onMount(async () => {
    // Guarded: an unguarded reject here aborted mount before the poll was ever
    // scheduled, leaving status blank with no reason shown.
    try {
      await loadTemplates();
      await loadDefaultTemplate();
      monitors = await listMonitors();
      lanIp = (await localIp()) || 'localhost';
      await refresh();
      // Make sure the poller is running even if this tab was opened before the
      // shell got there — idempotent, so this cannot create a second timer.
      startChannelHealth();
    } catch (e) {
      error = e; // the TYPED error; ErrorState humanises it (matches act())
    } finally {
      loading = false;
    }
  });

  const isNative = (c) => c.render_target === 'native_window';
  // NDI is parked (no proprietary SDK ships) and has no UI affordance, but an
  // older DB row could still carry `ndi_encode`, so rendering stays defensive.
  const isNdi = (c) => c.render_target === 'ndi_encode';
  // The URL carries the CHANNEL id (not just the template). That is what lets a
  // template change reach this output live — the output filters a channel-retemplate
  // broadcast by its own `channel`, so switching a screen's template needs no
  // re-copying of the URL. `template_id` stays for the first render before any push.
  const obsUrl = (c) =>
    `http://${lanIp}:8032/output.html?channel=${c.id}&template_id=${c.template_id ?? 1}&name=${encodeURIComponent(c.name)}`;
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
    ? `http://${lanIp}:8032/output.html?channel=${sel.id}&template_id=${sel.template_id ?? 1}&name=${encodeURIComponent(sel.name)}`
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
      error = null;
    } catch (err) {
      error = err;
    }
  }

  const assignTemplate = (c, e) => act(() => setChannelTemplate(c.id, parseInt(e.target.value, 10)));
  const assignDisplay = (c, e) => act(() => setChannelDisplay(c.id, e.target.value === '' ? null : e.target.value));
  const openNative = (c) => act(() => openChannelOutput(c.id));
  const closeNative = (c) => act(() => closeChannelOutput(c.id));

  // The ONE writer of the content-look map. `setContentTemplate` updates the
  // shared store optimistically and persists; on failure it reloads truth and
  // throws, so we only have to surface the error. No refresh() — content looks
  // are not channels.
  async function pickLook(kind, e) {
    const v = e.target.value;
    try {
      await setContentTemplate(kind, v === '' ? null : parseInt(v, 10));
      error = null;
    } catch (err) {
      error = err;
    }
  }

  async function add() {
    const name = newName.trim();
    if (!name) return;
    // A new screen adopts the DEFAULT template (falling back to the first built-in
    // if none is set) — the operator can reassign it per screen afterwards.
    await act(() => addChannel(name, newTarget, $defaultTemplateId ?? 1));
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

  let copiedLan = false;
  async function copyLan() {
    try {
      await navigator.clipboard.writeText(lanIp);
      copiedLan = true;
      setTimeout(() => (copiedLan = false), 1500);
    } catch (e) {
      console.warn('Clipboard write blocked', e);
    }
  }

  // A screen's preview shows its OWN template with stand-in content — the same
  // renderer the wall uses, so it is WYSIWYG rather than a drawing of one.
  const PREVIEW = { reference: 'John 3:16', text: 'For God so loved the world…', translation: 'KJV' };
</script>

<div class="ch-page">
  <!-- One vocabulary, three panes. The nav is the whole hub's spine — every
       output concern lives behind exactly one of these words. -->
  <nav class="ch-viewnav" aria-label="Outputs sections">
    {#each [['screens', 'Screens', counts.all], ['looks', 'Content looks', null], ['sharing', 'Sharing', null]] as [key, label, n]}
      <button class="ch-viewbtn" class:on={view === key} on:click={() => (view = key)}>
        {label}{#if n !== null}<span class="ch-viewn r-mono">{n}</span>{/if}
      </button>
    {/each}
  </nav>

  {#if view === 'screens'}
  <div class="ch-shell">
    <section class="ch-main">
      <!-- Filter tabs. Relay's real taxonomy is the render target, so these ARE
           the render targets — the reference's separate "Network" and "Browser
           Sources" tabs are one thing here (a browser source IS a network
           client), and splitting them would imply a distinction the engine does
           not make. -->
      <!-- One clean toolbar: search · live count · Add. The old type-filter tab row
           (All / Network / Native) was chrome for a list of a handful of screens —
           removed to keep this surface calm. -->
      <div class="ch-toolbar">
        <div class="ch-search">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
          <input placeholder="Search screens…" bind:value={q} aria-label="Search screens" />
        </div>
        <span class="ch-spring"></span>
        {#if !$capture.available}
          <span class="r-badge rose"><span class="bd"></span>Backend not attached</span>
        {:else}
          <!-- GREEN, not amber. Green is "confirmed / connected"; amber means
               something is on the wall, and a screen being online does not put it
               there. -->
          <span class="r-badge green"><span class="bd"></span>{onlineCount} of {channels.length} live</span>
        {/if}
        <button class="r-btn primary sm" on:click={() => (showAdd = !showAdd)} disabled={!$capture.available}>
          ＋ Add Screen
        </button>
      </div>

      {#if showAdd}
        <div class="ch-addbar">
          <input class="r-input" placeholder="New screen name" bind:value={newName} on:keydown={(e) => e.key === 'Enter' && add()} />
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
          <Loading what="screens" />
        {:else if shown.length}
          <div class="ch-thead r-lbl">
            <span class="ch-th-n">#</span>
            <span>Screen</span>
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

              <!-- Resolution is shown ONLY for a native screen with a display
                   assigned, because that is the only case where Relay knows one:
                   it is the monitor's size, read from the OS. A networked screen's
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

              <span
                class="ch-status r-mono"
                class:on={screenFault(st) === 'ok'}
                class:un={screenFault(st) === 'unsupported'}
                class:down={screenFault(st) === 'silent' || screenFault(st) === 'never'}
              >
                <span class="bd"></span>{FAULT_WORD[screenFault(st)]}
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
          <EmptyState message={channels.length ? 'No screen matches this filter.' : 'No screens yet — add one below.'} />
        {/if}

        <button class="ch-addcard" on:click={() => (showAdd = true)} disabled={!$capture.available}>
          <span class="ch-addmark">＋</span>
          <span class="ch-addttl">Add New Screen</span>
          <span class="ch-addsub">Configure a display for HDMI, or a networked OBS / kiosk source for your venue.</span>
        </button>
      </div>

      <ErrorState {error} />
    </section>

    <!-- ══ INSPECTOR ══ -->
    <aside class="ch-insp">
      {#if !sel}
        <div class="ch-insphead"><span class="ch-inspttl">Screen</span></div>
        <div class="ch-empty r-empty">Pick a screen to configure it.</div>
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
               screen's template + any content-type override), exactly as this
               output is rendering it right now. Only when nothing is live does it
               fall back to a sample so the template is still previewable. This is
               what makes "select a screen" agree with what is actually on air. -->
          <div class="ch-preview">
            <!-- Resolve EXACTLY like the real output: the screen's OWN template
                 wins (so a lower-third previews as a band, not a full screen), a
                 pinned cue choice overrides, a content look defers. The preview
                 shows how THIS screen actually looks live, not the program feed. -->
            <TemplateRender
              template={resolveOutputTemplate(templateOf(sel) ?? {}, $live ? $liveTemplateOverride : null, $live ? $liveTemplatePinned : false)}
              content={$live ? $liveContent : PREVIEW} />
          </div>
          <p class="ch-prevnote r-mono">{$live ? 'Live — mirroring the program' : 'Sample — nothing on screen'}</p>

          <div class="r-lbl ch-flbl">Screen info</div>
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
            <!-- The screen's own last word, kept separate from Relay's. When the
                 two disagree, that disagreement is the finding. -->
            <dt>Screen says</dt>
            <dd>
              {#if !selStatus}Unknown
              {:else if !selStatus.supported}—
              {:else if selStatus.last_beat_ms === null}has never reported painting
              {:else}{selStatus.paint_state ?? 'unknown'} · {Math.round(selStatus.last_beat_ms / 1000)}s ago
              {/if}
            </dd>
          </dl>

          <div class="r-lbl ch-flbl">Template</div>
          <select class="r-select ch-fin" value={sel.template_id} on:change={(e) => assignTemplate(sel, e)} disabled={!$capture.available}>
            {#each $templates as t (t.id)}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
          <p class="ch-finhint">This screen's own look. A content look (Scripture, Lyrics…) overrides it for that content type.</p>

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
              {delArm === sel.id ? 'Click again to confirm' : 'Remove screen'}
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
            such figure here would be invented. A screen reading <b>LIVE</b> means
            something is attached, not that the picture is good.
          </p>
        </div>
      {/if}
    </aside>
  </div>

  {:else if view === 'looks'}
  <!-- ══ CONTENT LOOKS ══ THE one writer of the type → template default map.
       Every other surface that shows an assignment reads the shared store and is
       read-only (Decision §25). -->
  <div class="ch-solo r-scroll">
    <div class="r-tile ch-looks">
      <div class="ch-lookshead">
        <h2 class="ch-looksttl">Content looks</h2>
        <p class="ch-looksub r-dim">
          When the AI fires a verse, a song or an announcement, it wears the look you
          set here — on <b>every</b> screen that hasn't been given its own template.
          Leave one on “Each screen's own template” to let each screen decide.
        </p>
      </div>

      {#if !$templates.length}
        <EmptyState message="No templates yet — make one in the Templates tab first." />
      {:else}
        <div class="ch-looksgrid">
          {#each CONTENT_KINDS as k (k.key)}
            <label class="ch-lookrow" for="look-{k.key}">
              <span class="ch-lookname">{k.label}</span>
              <select id="look-{k.key}" class="r-select ch-lookselect"
                value={$contentTemplates[k.key] ?? ''}
                on:change={(e) => pickLook(k.key, e)}
                disabled={!$capture.available}>
                <option value="">Each screen's own template</option>
                {#each $templates as t (t.id)}
                  <option value={t.id}>{t.name}</option>
                {/each}
              </select>
            </label>
          {/each}
        </div>
      {/if}
      <ErrorState {error} />
    </div>
  </div>

  {:else}
  <!-- ══ SHARING ══ the LAN address + the preacher's stage remote. -->
  <div class="ch-solo r-scroll">
    <div class="r-tile ch-share">
      <div class="r-lbl ch-flbl">This machine on the network</div>
      <p class="ch-looksub r-dim">
        Kiosk screens, the OBS machine and the preacher's phone all pull the live
        output from this computer over the same Wi-Fi. Point a browser source at a
        screen's <b>Copy URL</b> (in the Screens pane), or use the addresses below.
      </p>
      <dl class="ch-info ch-shareinfo">
        <dt>This machine</dt>
        <dd class="ch-addr-row">
          <span class="ch-addr">{lanIp}</span>
          <button class="r-btn ghost sm" on:click={copyLan}>{copiedLan ? 'Copied ✓' : 'Copy'}</button>
        </dd>
        <dt>Output / stage pages</dt><dd class="r-mono">:8032 · http</dd>
        <dt>Live update channel</dt><dd class="r-mono">:8031 · websocket</dd>
      </dl>
    </div>

    <!-- Preacher's stage remote — the one output a church sets up by hand every
         week. Not a screen row; a share tile of its own. -->
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
  </div>
  {/if}
</div>

<style>
  .ch-page{ display:flex; flex-direction:column; gap:var(--v-sp-md); height:100%; min-height:0; }

  /* ── view nav (Screens · Content looks · Sharing) ── */
  .ch-viewnav{ display:inline-flex; gap:4px; padding:4px; border-radius:var(--v-r-md);
    background:var(--v-surf); border:1px solid var(--v-line); flex:0 0 auto; align-self:flex-start; }
  .ch-viewbtn{ display:inline-flex; align-items:center; gap:7px; padding:7px 15px; border:0; cursor:pointer;
    border-radius:var(--v-r-sm); background:transparent; color:var(--v-dim);
    font-size:var(--v-fs-b2); font-weight:500; transition:.12s; }
  .ch-viewbtn:hover{ color:var(--v-txt); }
  .ch-viewbtn.on{ background:var(--v-accent-fill); color:var(--v-accent-ink); }
  .ch-viewn{ font-size:var(--v-fs-cap); padding:1px 6px; border-radius:99px;
    background:var(--v-surf3); color:var(--v-dim); }
  .ch-viewbtn.on .ch-viewn{ background:rgba(0,0,0,.28); color:var(--v-accent-ink); }

  .ch-shell{ display:grid; grid-template-columns:minmax(0,1fr) 330px; gap:var(--v-sp-md);
    flex:1; min-height:0; }
  @media (max-width:1180px){ .ch-shell{ grid-template-columns:1fr; } }

  /* Single-column panes (Content looks, Sharing) — centred, comfortable measure. */
  .ch-solo{ flex:1; min-height:0; overflow-y:auto; display:flex; flex-direction:column; gap:var(--v-sp-md); }

  .ch-main{ display:flex; flex-direction:column; min-height:0; gap:12px; }

  /* ── tabs ── */
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
     inspector for the selected screen, so neither is the last copy. */
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
     a screen to configure it puts nothing anywhere. */
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
  /* A screen that stopped answering. Rose is the failure colour (DESIGN_SYSTEM);
     amber is never spent here because amber means on air. */
  .ch-status.down{ color:var(--v-rose); }

  .ch-rowbtns{ display:flex; gap:5px; justify-content:flex-end; align-items:center; }

  .ch-qr{ display:flex; align-items:center; gap:14px; padding:12px;
    background:var(--v-bg); border-bottom:1px solid var(--v-line); }
  .ch-qr-img{ border-radius:var(--v-r-sm); flex:0 0 auto; }
  .ch-qr-info{ flex:1; min-width:0; }
  .ch-qr-url{ font-size:var(--v-fs-cap); color:var(--v-accent2); margin:4px 0;
    overflow-wrap:anywhere; }
  .ch-qr-hint{ font-size:var(--v-fs-cap); color:var(--v-faint); }

  /* ── content looks ── */
  .ch-looks{ padding:20px 22px; max-width:720px; width:100%; }
  .ch-lookshead{ margin-bottom:18px; }
  .ch-looksttl{ font-family:var(--f-head); font-size:var(--v-fs-h2); font-weight:600;
    color:var(--v-txt); margin:0 0 6px; }
  .ch-looksub{ font-size:var(--v-fs-b2); line-height:1.5; margin:0; }
  .ch-looksgrid{ display:flex; flex-direction:column; gap:8px; }
  .ch-lookrow{ display:grid; grid-template-columns:minmax(120px,180px) minmax(0,1fr);
    align-items:center; gap:14px; padding:10px 12px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); }
  .ch-lookname{ font-size:var(--v-fs-b1); font-weight:500; color:var(--v-txt); }
  .ch-lookselect{ width:100%; }

  /* ── sharing ── */
  .ch-share{ padding:18px 20px; max-width:640px; width:100%; }
  .ch-shareinfo{ margin-top:12px; }
  .ch-addr-row{ display:flex; align-items:center; gap:10px; }
  .ch-addr-row .ch-addr{ flex:1; }

  /* ── stage remote ── */
  .ch-stage{ display:flex; align-items:center; gap:14px; flex-wrap:wrap; flex:0 0 auto; padding:13px 15px;
    max-width:640px; width:100%; }
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
  .ch-finhint{ margin:6px 0 0; font-size:var(--v-fs-cap); line-height:1.45; color:var(--v-faint); }
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
