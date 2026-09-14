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
  // The workspace grammar (docs/REBRAND.md §2 · §11): rail · main · inspector,
  // one type scale, a row that is a name and a value. Shared with the Planner and
  // Settings so the three read as one desk rather than three designs.
  import WorkspaceFrame from './WorkspaceFrame.svelte';
  // The SAME rule Live uses. Two surfaces describing one screen must not be able
  // to reach different conclusions about it — that asymmetry is how this
  // repository has produced four separate bugs with one root cause.
  import { screenFault, FAULT_WORD, screenKind, screenTransport } from '../outputHealth.js';
  // Was: `error = String(err)`, rendered in a MONOSPACE font, five times over — a raw
  // Rust Err string shown to a church volunteer who has never seen one.
  import ErrorState from '../ui/ErrorState.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import Loading from '../ui/Loading.svelte';
  import TemplateRender from '../TemplateRender.svelte';
  // DEFAULT_TEMPLATE is the FLOOR, and it is the output page's floor too — a
  // screen that follows the content look when no look is set still has to paint
  // something legible. Imported here so the preview and the wall reach the same
  // answer rather than two different kinds of nothing.
  import { DEFAULT_TEMPLATE } from '../templates.js';
  import { CONTENT_KINDS, resolveOutputTemplate } from '../layers.js';
  import { outputUrl } from '../outputurl.js';
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
    readErrors,
  } from '../stores/capture.js';

  // Which pane. 'screens' is where an operator lives. All three now keep the
  // rail and the inspector: a section that drops two of the three columns is a
  // different workspace wearing the same tab, and that is what made Content
  // looks and Sharing read as a separate product.
  let view = 'screens'; // screens | looks | sharing
  const VIEWS = [
    { key: 'screens', label: 'Screens',
      lead: 'Every target Relay can paint: a projector on HDMI, an OBS or kiosk browser source over the network.' },
    { key: 'looks', label: 'Content looks',
      lead: 'Which template each kind of content wears on any screen that has no look of its own.' },
    { key: 'sharing', label: 'Sharing',
      lead: 'The addresses other devices in the building use to reach this machine.' },
  ];
  $: activeView = VIEWS.find((v) => v.key === view) ?? VIEWS[0];

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
  // ONE BUILDER (`lib/outputurl.js`). This was written out twice, four lines
  // apart, and only one copy was corrected when a screen gained the ability to
  // have no look of its own — so Copy URL and the inspector's readout said
  // different things about the same screen.
  const obsUrl = (c) => outputUrl(lanIp, c.id, c.template_id, c.name);
  const templateOf = (c) => $templates.find((t) => t.id === c.template_id) || null;
  /** What a content look currently resolves to, by name — for a following screen. */
  const lookName = (kind) =>
    $templates.find((t) => t.id === $contentTemplates[kind])?.name ?? 'the default look';
  const monitorOf = (c) => {
    const i = parseInt(c.display_target ?? '', 10);
    return Number.isFinite(i) ? monitors.find((m) => m.index === i) || null : null;
  };
  /** The kind label shown in the TYPE column. One definition, shared with Live's
      Output Status pane — see `outputHealth.js::screenKind`. */
  const kindOf = (c) => screenKind(c.render_target);
  const transportOf = (c) => screenTransport(c.render_target);

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
  $: selAddr = sel ? outputUrl(lanIp, sel.id, sel.template_id, sel.name) : '';
  $: onlineCount = channels.filter((c) => status[c.id]?.online).length;
  // Which screens a content look actually reaches. A screen's OWN template wins
  // (DECISIONS §29), so a look changes nothing on a screen that has one — and
  // that is precisely the defect phase 4 found: the map could be filled in, saved
  // and change nothing in the building. The inspector answers it with the list.
  $: followers = channels.filter((c) => c.template_id == null);

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

  // '' is the operator choosing FOLLOW THE CONTENT LOOK — a screen with no look
  // of its own (DECISIONS §70). It is a value, not an empty field.
  const assignTemplate = (c, e) =>
    act(() => setChannelTemplate(c.id, e.target.value === '' ? null : parseInt(e.target.value, 10)));
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

  // A screen's preview shows what that screen REALLY shows — the same renderer
  // the wall uses, resolved by the same resolver, so it is WYSIWYG rather than a
  // drawing of one. The stand-in is scripture, which is why the idle preview
  // resolves against the SCRIPTURE content look below.
  const PREVIEW = { reference: 'John 3:16', text: 'For God so loved the world…', translation: 'KJV' };

  // WHAT THIS SCREEN WOULD ACTUALLY WEAR.
  //
  // `templateOf(sel)` is `null` for a screen set to FOLLOW THE CONTENT LOOK, and
  // null is the answer, not a missing one (DECISIONS §70). This used to be
  // written `templateOf(sel) ?? {}`, which is truthy — so `resolveOutputTemplate`
  // never reached its `if (!channelTpl) return override` branch, `isKeyedTemplate({})`
  // said "keyed" (no layers, no background), the transparency law kept the empty
  // object, and a following screen previewed as a blank frame. The one screen
  // whose look you cannot read off its own row was the one the preview could not
  // answer for, on the panel built to answer it.
  //
  // `Output.svelte` does exactly this — `resolveOutputTemplate(t, override, pinned)
  // || DEFAULT_TEMPLATE` with a null `t` — and two surfaces describing one screen
  // must not be able to reach different conclusions about it.
  //
  // Idle, the override is the SCRIPTURE content look, because the stand-in content
  // is a verse: that is the look this screen would wear if scripture fired now. A
  // content look is never `pinned` (only a cue's deliberate choice is), so a screen
  // with a template of its own is unaffected — which is DECISIONS §29, visible.
  //
  // NAMED, not called. `lookName('scripture')` would read `$templates` and
  // `$contentTemplates` INSIDE a function, and Svelte tracks the identifiers in
  // the expression — so the preview would be correct once, by luck of ordering,
  // and never update when the look changed. This file has already been caught by
  // exactly that (`stageUrl()`, a few lines up), twice.
  $: scriptureLook = $templates.find((t) => t.id === $contentTemplates.scripture) ?? null;
  $: previewOverride = $live ? $liveTemplateOverride : scriptureLook;
  $: previewTemplate =
    resolveOutputTemplate(
      sel ? templateOf(sel) : null,
      previewOverride,
      $live ? $liveTemplatePinned : false,
    ) || DEFAULT_TEMPLATE;
  // What the preview is a preview OF. "Sample" said the same thing for a screen
  // with its own look and for one following a look it never showed — rule 35 in
  // small: a line that reads the same in two different situations is not a line.
  $: previewNote = $live
    ? 'Live — mirroring the program'
    : sel && sel.template_id == null
      ? `Sample — follows the content look · ${scriptureLook?.name ?? 'the default look'}`
      : 'Sample — nothing on screen';
</script>

<WorkspaceFrame
  title="Outputs"
  standfirst={activeView.lead}
  columns="206px minmax(0,1fr) 320px">
  <svelte:fragment slot="head">
    {#if !$capture.available}
      <span class="r-badge rose"><span class="bd"></span>Backend not attached</span>
    {:else}
      <!-- GREEN, not amber. Green is "confirmed / connected"; amber means
           something is on the wall, and a screen being online does not put it
           there. -->
      <span class="r-badge green"><span class="bd"></span>{onlineCount} of {channels.length} live</span>
    {/if}
  </svelte:fragment>

  <!-- ══ RAIL ══ One vocabulary, three sections. Every output concern lives
       behind exactly one of these words, and the rail keeps all three in view
       rather than making one of them a mode you have to remember you are in. -->
  <aside class="rw-pane">
    <div class="rw-panehead"><h2 class="rw-panettl">Outputs</h2></div>
    <nav class="rw-panebody" aria-label="Outputs sections">
      {#each VIEWS as v (v.key)}
        <button class="rw-item r-focus" class:on={view === v.key}
          aria-current={view === v.key} on:click={() => (view = v.key)}>
          <span class="rw-itemname">{v.label}</span>
          {#if v.key === 'screens'}<span class="rw-itemn">{counts.all}</span>{/if}
          {#if v.key === 'looks'}<span class="rw-itemn">{followers.length}</span>{/if}
        </button>
      {/each}
    </nav>
    <!-- The two facts an operator asks this tab for without opening anything. -->
    <div class="rw-panefoot ch-railfacts">
      <div class="ch-railfact"><span class="ch-railk">Live</span><span class="ch-railv r-mono">{onlineCount} / {channels.length}</span></div>
      <div class="ch-railfact"><span class="ch-railk">This machine</span><span class="ch-railv r-mono">{lanIp}</span></div>
    </div>
  </aside>

  {#if view === 'screens'}
    <section class="rw-pane">
      <!-- Filter tabs. Relay's real taxonomy is the render target, so these ARE
           the render targets — the reference's separate "Network" and "Browser
           Sources" tabs are one thing here (a browser source IS a network
           client), and splitting them would imply a distinction the engine does
           not make. -->
      <!-- One clean pane head: name · search · Add. The old type-filter tab row
           (All / Network / Native) was chrome for a list of a handful of screens —
           removed to keep this surface calm. -->
      <div class="rw-panehead ch-panehead">
        <h2 class="rw-panettl">Screens</h2>
        <div class="ch-search">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
          <input placeholder="Search screens…" bind:value={q} aria-label="Search screens" />
        </div>
        <span class="rw-spring"></span>
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

      <div class="rw-panebody ch-tablewrap">
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

              <span class="ch-tpl r-mono">{c.template_id == null ? 'Content look' : (templateOf(c)?.name ?? 'None')}</span>

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
        {:else if !channels.length && $readErrors.listOutputChannels}
          <!-- RG-95, second pass. This view HAD an `<ErrorState>` and it could not
               fire: every read in `onMount` is a GROUP 2 wrapper that swallows to a
               safe default, so `error` was only ever set by `act()` — a mutation.
               A database that would not open therefore read "No screens yet — add
               one below.", and the operator's answer to that sentence is to add a
               screen they already have. -->
          <ErrorState error={$readErrors.listOutputChannels} onRetry={refresh} />
        {:else}
          <EmptyState message={channels.length ? 'No screen matches this filter.' : 'No screens yet — add one below.'} />
        {/if}

        <button class="ch-addcard" on:click={() => (showAdd = true)} disabled={!$capture.available}>
          <span class="ch-addmark">＋</span>
          <span class="ch-addttl">Add New Screen</span>
          <span class="ch-addsub">Configure a display for HDMI, or a networked OBS / kiosk source for your venue.</span>
        </button>
        <ErrorState {error} />
      </div>
    </section>

  {:else if view === 'looks'}
    <!-- ══ CONTENT LOOKS ══ THE one writer of the type → template default map.
         Every other surface that shows an assignment reads the shared store and
         is read-only (Decision §25). A row here is a name and a value, like every
         other row on the desk — the value happens to be a picker. -->
    <section class="rw-pane">
      <div class="rw-panehead"><h2 class="rw-panettl">Content looks</h2></div>
      <div class="rw-panebody">
        {#if !$templates.length}
          <EmptyState message="No templates yet — make one in the Templates tab first." />
        {:else}
          {#each CONTENT_KINDS as k (k.key)}
            <div class="rw-nv">
              <label class="rw-nvk" for="look-{k.key}">{k.label}</label>
              <select id="look-{k.key}" class="r-select rw-nvctl ch-lookselect"
                value={$contentTemplates[k.key] ?? ''}
                on:change={(e) => pickLook(k.key, e)}
                disabled={!$capture.available}>
                <option value="">Each screen's own template</option>
                {#each $templates as t (t.id)}
                  <option value={t.id}>{t.name}</option>
                {/each}
              </select>
            </div>
          {/each}
        {/if}
        <div class="ch-pad">
          <p class="rw-foot">
            When the AI fires a verse, a song or an announcement it wears the look set
            here — but only on a screen that has <b>no template of its own</b>. A
            screen's own look wins (DECISIONS §29), so setting one of these changes
            nothing on a screen you have already assigned. The inspector lists the
            screens this actually reaches.
          </p>
          <ErrorState {error} />
        </div>
      </div>
    </section>

  {:else}
    <!-- ══ SHARING ══ the addresses other devices in the building type in. -->
    <section class="rw-pane">
      <div class="rw-panehead"><h2 class="rw-panettl">This machine on the network</h2></div>
      <div class="rw-panebody">
        <div class="rw-nv">
          <span class="rw-nvk">This machine</span>
          <span class="rw-nvctl ch-addr-row">
            <span class="ch-addr">{lanIp}</span>
            <button class="r-btn ghost sm" on:click={copyLan}>{copiedLan ? 'Copied ✓' : 'Copy'}</button>
          </span>
        </div>
        <div class="rw-nv"><span class="rw-nvk">Output / stage pages</span><span class="rw-nvv">:8032 · http</span></div>
        <div class="rw-nv"><span class="rw-nvk">Live update channel</span><span class="rw-nvv">:8031 · websocket</span></div>
        <div class="ch-pad">
          <p class="rw-foot">
            Kiosk screens, the OBS machine and the preacher's phone all pull the live
            output from this computer over the same Wi-Fi. Point a browser source at a
            screen's <b>Copy URL</b> in the Screens section — a hand-built address will
            not follow a template change.
          </p>
        </div>
      </div>
    </section>
  {/if}

  <!-- ══ INSPECTOR ══ what is true of the thing in hand. One rail for all three
       sections: on Screens the selected screen, on Content looks the screens a
       look actually reaches, on Sharing the one output a church sets up by hand
       on somebody else's device every week. -->
  {#if view === 'screens'}
    <aside class="rw-pane rw-insp">
      {#if !sel}
        <div class="rw-panehead"><h2 class="rw-panettl">Screen</h2></div>
        <div class="ch-empty r-empty">Pick a screen to configure it.</div>
      {:else}
        <div class="rw-panehead">
          <h2 class="rw-panettl ch-inspttl">{sel.name}</h2>
          <span class="ch-status r-mono" class:on={selStatus?.online} class:un={selStatus && !selStatus.supported}>
            <span class="bd"></span>{selStatus ? (!selStatus.supported ? 'UNAVAILABLE' : selStatus.online ? 'LIVE' : 'IDLE') : '—'}
          </span>
          <button class="r-iconbtn ch-close" aria-label="Close panel" on:click={() => (selId = null)}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M18 6 6 18M6 6l12 12"/></svg>
          </button>
        </div>

        <div class="rw-panebody pad">
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
            <TemplateRender template={previewTemplate} content={$live ? $liveContent : PREVIEW} />
          </div>
          <p class="ch-prevnote r-mono">{previewNote}</p>

          <div class="r-lbl ch-flbl">Screen info</div>
          <dl class="ch-info">
            <dt>Type</dt><dd>{kindOf(sel)}</dd>
            <dt>Transport</dt><dd>{transportOf(sel)}</dd>
            <dt>Template</dt><dd>{sel.template_id == null ? 'Follows the content look' : (templateOf(sel)?.name ?? 'None')}</dd>
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
          <select class="r-select ch-fin" value={sel.template_id ?? ''} on:change={(e) => assignTemplate(sel, e)} disabled={!$capture.available}>
            <option value="">Follow the content look</option>
            {#each $templates as t (t.id)}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>
          {#if sel.template_id == null}
            <p class="ch-finhint">
              This screen has no look of its own: each kind of content wears whatever the
              content look says. Right now —
              {#each CONTENT_KINDS as k, i}{i ? ' · ' : ' '}{k.label}: {lookName(k.key)}{/each}
            </p>
          {:else}
            <p class="ch-finhint">
              This screen's own look. It wins over a content look — only a cue that pins its
              own template overrides it (DECISIONS §29). To let the content looks decide here,
              choose <b>Follow the content look</b>.
            </p>
          {/if}

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
          <p class="rw-foot ch-nomargin">
            Relay reports whether an output window is open and how many clients are
            connected. It does <b>not</b> measure latency, bandwidth, frame rate or
            dropped frames — nothing in the pipeline times or counts delivery, so any
            such figure here would be invented. A screen reading <b>LIVE</b> means
            something is attached, not that the picture is good.
          </p>
        </div>
      {/if}
    </aside>

  {:else if view === 'looks'}
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead"><h2 class="rw-panettl">Screens that follow</h2></div>
      <div class="rw-panebody">
        {#if followers.length}
          {#each followers as c (c.id)}
            <div class="rw-nv">
              <span class="rw-nvk">{c.name}</span>
              <span class="rw-nvv">{kindOf(c)}</span>
            </div>
          {/each}
        {:else}
          <div class="ch-empty r-empty">
            No screen follows the content look. Every screen has a template of its own,
            so nothing on the left changes what a congregation sees.
          </div>
        {/if}
        <div class="ch-pad">
          <div class="r-lbl ch-flbl">Resolving right now</div>
          <dl class="ch-info">
            {#each CONTENT_KINDS as k (k.key)}
              <dt>{k.label}</dt><dd>{lookName(k.key)}</dd>
            {/each}
          </dl>
          <p class="rw-foot">
            To let a screen follow these, open it in <b>Screens</b> and set its template
            to <b>Follow the content look</b>.
          </p>
        </div>
      </div>
    </aside>

  {:else}
    <!-- Preacher's stage remote — the one output a church sets up by hand on
         somebody else's device every week, so it belongs on the rail you hand
         the phone from rather than in a tile at the bottom of a page. -->
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead"><h2 class="rw-panettl">Preacher's stage remote</h2></div>
      <div class="rw-panebody pad">
        <p class="ch-stage-sub r-dim">
          The live verse on a phone or iPad, updating in real time. Scan the QR (same
          Wi-Fi) or open <code class="r-mono">{stageUrl}</code>.
        </p>
        <div class="ch-stage-actions">
          <button class="r-btn primary sm" on:click={showStageQr}>{stageQrOpen ? 'Hide QR' : 'Show QR'}</button>
          <button class="r-btn ghost sm" on:click={copyStage}>{copiedStage ? 'Copied ✓' : 'Copy link'}</button>
        </div>
        {#if stageQrOpen}
          <img class="ch-stage-qr" src={stageQr} alt="QR code to open the stage remote" width="150" height="150" />
        {/if}
        <p class="rw-foot">
          Anyone on the same Wi-Fi who has the address can open it — Relay does not ask
          the device who it is (<b>DECISIONS §35</b>), so treat the link the way you
          would treat the Wi-Fi password.
        </p>
      </div>
    </aside>
  {/if}
</WorkspaceFrame>

<style>
  /* OUTPUTS — laid out in the shared workspace grammar (`WorkspaceFrame.svelte`,
     docs/REBRAND.md §2). Everything here is the part that is specific to screens;
     the columns, the panes, the type roles and the name/value row come from the
     frame so this workspace and the Planner cannot drift apart.

     What changed with the rebrand: the pill nav across the top became the rail —
     Content looks and Sharing were two solo pages wearing the Outputs tab, and a
     section that drops two of the three columns is a different workspace. Every
     section now keeps the rail and gets an inspector that answers the question
     that section actually raises. */

  /* ── rail ── */
  .ch-railfacts{ gap:0; padding:0; }
  .ch-railfact{ display:flex; align-items:center; justify-content:space-between; gap:8px;
    padding:7px 12px; border-bottom:1px solid var(--v-line); }
  .ch-railfact:last-child{ border-bottom:0; }
  .ch-railk{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .ch-railv{ font-size:var(--v-fs-cap); color:var(--v-dim); font-variant-numeric:tabular-nums;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* ── the screens pane head ── */
  .ch-panehead{ min-height:40px; padding:5px 12px; gap:10px; flex-wrap:wrap; }
  .ch-search{ display:flex; align-items:center; gap:8px; background:var(--v-bg);
    border:1px solid var(--v-line2); border-radius:var(--v-r-sm); padding:0 10px; height:26px;
    flex:0 1 240px; min-width:140px; }
  .ch-search:focus-within{ border-color:var(--v-accent-line); box-shadow:0 0 0 3px var(--v-accent-soft); }
  .ch-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .ch-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none;
    color:var(--v-txt); font-size:var(--v-fs-b2); }
  .ch-search input::placeholder{ color:var(--v-faint); }

  .ch-addbar{ display:flex; gap:8px; align-items:center; flex:0 0 auto; padding:9px 12px;
    background:var(--v-bg); border-bottom:1px solid var(--v-accent-line); }
  .ch-addbar .r-input{ flex:1 1 200px; }

  /* Content that is prose rather than a row still needs a gutter; the pane body
     itself has none, because seamed rows must reach both edges. */
  .ch-pad{ padding:12px; }
  .ch-nomargin{ margin-top:0; }

  /* ── table ── */
  .ch-tablewrap{ overflow-y:auto; }
  .ch-thead, .ch-row{ display:grid;
    grid-template-columns:26px minmax(172px,1fr) 110px 124px 112px 100px 128px;
    align-items:center; gap:10px; padding:0 12px; }
  .ch-thead{ height:28px; position:sticky; top:0; z-index:2; background:var(--v-bg);
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

  /* Dense, with a hairline seam — 52px per row was a card pretending to be a row,
     and on a booth laptop it cost two screens' worth of list. */
  .ch-row{ min-height:42px; border-bottom:1px solid var(--v-line); cursor:pointer;
    transition:background .12s, box-shadow .12s; }
  .ch-row:last-child{ border-bottom:0; }
  .ch-row:hover{ background:var(--v-surf2); }
  /* Steel blue = the thing you are working on. Never amber: amber means live on
     the wall, and selecting a screen to configure it puts nothing anywhere. */
  .ch-row.sel{ background:var(--v-sel-soft); box-shadow:inset 2px 0 0 var(--v-sel); }
  .ch-num{ font-size:var(--v-fs-lbl); color:var(--v-faint); text-align:center; }

  .ch-namecell{ display:flex; align-items:center; gap:10px; min-width:0; }
  .ch-ico{ width:26px; height:26px; border-radius:var(--v-r-sm); display:grid; place-items:center;
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-faint); flex:0 0 auto; }
  .ch-ico.live{ color:var(--v-emerald); border-color:var(--v-emerald-soft); background:var(--v-emerald-soft); }
  .ch-nametxt{ min-width:0; }
  .ch-name{ display:block; font-size:var(--v-fs-b2); font-weight:500; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .ch-sub{ display:block; font-size:var(--v-fs-cap); color:var(--v-faint); margin-top:1px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-ty{ font-size:var(--v-fs-cap); color:var(--v-dim); min-width:0; }
  .ch-ty i{ display:block; font-style:normal; color:var(--v-faint); margin-top:1px; }
  .ch-tpl, .ch-out{ font-size:var(--v-fs-cap); color:var(--v-dim);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-status{ display:inline-flex; align-items:center; gap:6px; font-size:var(--v-fs-cap);
    letter-spacing:.06em; color:var(--v-faint); flex:0 0 auto; }
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
  .ch-lookselect{ width:min(230px, 52vw); }

  /* ── sharing / stage remote ── */
  .ch-addr-row{ display:flex; align-items:center; gap:10px; min-width:0; }
  .ch-addr-row .ch-addr{ flex:1; min-width:0; }
  .ch-stage-sub{ margin:0 0 10px; font-size:var(--v-fs-b2); line-height:1.45; }
  .ch-stage-actions{ display:flex; gap:6px; flex-wrap:wrap; }
  .ch-stage-qr{ display:block; margin-top:12px; border-radius:var(--v-r-sm); }

  /* ── inspector ── */
  .ch-inspttl{ flex:1; text-transform:none; letter-spacing:var(--v-tr-h2);
    font-size:var(--v-fs-h3); line-height:var(--v-lh-h3); }
  .ch-close{ width:22px; height:22px; flex:0 0 auto; }

  /* position:relative is load-bearing — TemplateRender's root is
     position:absolute; inset:0, so without it the preview escapes this box and
     lays itself out against the page. It supplies its own container-type. */
  .ch-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-sm);
    border:1px solid var(--v-line2); overflow:hidden; background:var(--v-void); }
  .ch-prevnote{ margin:6px 0 0; font-size:var(--v-fs-cap); color:var(--v-faint); }

  .ch-flbl{ margin:15px 0 6px; }
  .ch-fin{ width:100%; }
  .ch-finhint{ margin:6px 0 0; font-size:var(--v-fs-cap); line-height:1.45; color:var(--v-faint); }

  /* A name and a VALUE (§11) — one hairline per fact, the value on the right
     edge so a column of them can be read down rather than hunted through. */
  .ch-info{ display:grid; grid-template-columns:auto minmax(0,1fr); gap:0 12px; margin:0;
    font-size:var(--v-fs-b2); }
  .ch-info dt{ color:var(--v-faint); padding:6px 0; border-bottom:1px solid var(--v-line); }
  .ch-info dd{ margin:0; color:var(--v-txt); text-align:right; overflow-wrap:anywhere;
    padding:6px 0; border-bottom:1px solid var(--v-line); }
  /* One line, truncated. Wrapping "anywhere" broke it mid-word into
     `output.h / tml?` — an address split across a line break invites being
     mis-typed, and Copy URL is right there for the real thing. */
  .ch-addr{ font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-accent2);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .ch-actions{ display:flex; flex-wrap:wrap; gap:6px; }
  .ch-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  .ch-del{ color:var(--v-rose); }
  .ch-del:hover:not(:disabled), .ch-del.arm{ border-color:var(--v-rose); background:var(--v-rose-soft); }

  .ch-empty{ margin:auto; padding:20px 14px; text-align:center; line-height:1.5; }

  /* Sits at the end of the list, where the eye lands after reading it. */
  .ch-addcard{ display:flex; flex-direction:column; align-items:center; gap:5px;
    width:calc(100% - 24px); margin:12px; padding:16px; cursor:pointer;
    background:transparent; border:1px dashed var(--v-line2); border-radius:var(--v-r-sm);
    color:inherit; transition:.14s; }
  .ch-addcard:hover:not(:disabled){ border-color:var(--v-accent); background:var(--v-accent-soft); }
  .ch-addcard:disabled{ opacity:.45; cursor:not-allowed; }
  .ch-addmark{ width:24px; height:24px; border-radius:50%; display:grid; place-items:center;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-accent2);
    font-size:var(--v-fs-h2); line-height:1; }
  .ch-addttl{ font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt); }
  .ch-addsub{ font-size:var(--v-fs-cap); color:var(--v-faint); text-align:center; line-height:1.5; }
</style>
