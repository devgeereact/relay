<script>
  import { onMount } from 'svelte';
  import ModelSetup from '../ModelSetup.svelte';
  import { capture, meter, templates, initAudio, startCapture, stopCapture, setThresholds, setSttLanguage, setInputDevice, listTranslations, getActiveTranslation, setActiveTranslation, localIp, loadTemplates, getContentTemplates, setContentTemplate, getCrashReporting, setCrashReporting } from '../stores/capture.js';

  // Crash reporting — OFF by default. The only thing in Relay that can send
  // anything off the device, so the UI states plainly what is and isn't sent.
  let crash = { enabled: false, dsn: '' };
  let crashMsg = '';
  async function toggleCrash(enabled) {
    crashMsg = '';
    try {
      crash = await setCrashReporting(enabled, crash.dsn);
      crashMsg = crash.enabled
        ? 'Crash reporting on.'
        : enabled
          ? 'Add a Sentry DSN above to turn this on.'
          : 'Crash reporting off.';
    } catch (e) {
      crashMsg = String(e);
    }
  }

  // Per-content-type default templates (ProPresenter-style).
  const contentTypes = [
    { key: 'scripture', label: 'Scripture' },
    { key: 'song', label: 'Lyrics' },
    { key: 'media', label: 'Media' },
    { key: 'announce', label: 'Announcements' },
  ];
  let ctMap = { scripture: null, song: null, media: null, announce: null };
  async function pickCt(kind, val) {
    const id = val ? parseInt(val, 10) : null;
    ctMap[kind] = id;
    await setContentTemplate(kind, id);
  }

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
  // Device selection is shared app-wide (capture.inputDevice) so the Console's
  // "Listen" uses the same device the operator picked here.
  onMount(initAudio);
  // NOTE: capture is app-level state, NOT tied to this view's lifetime — do not
  // stop it on unmount, or switching to the Console tab would kill the mic
  // mid-service. Capture stops only when the operator clicks Stop.

  async function toggleCapture() {
    if ($capture.capturing) await stopCapture();
    else await startCapture($capture.inputDevice || null);
  }

  // RMS on speech sits well below 1.0; scale so normal talking fills the meter.
  $: levelPct = Math.min(100, Math.round($meter.level * 320));

  // Real translations from the corpus + which one to read from.
  let translations = [];
  let activeTranslation = null;
  let lanIp = '';
  onMount(async () => {
    translations = await listTranslations();
    activeTranslation = await getActiveTranslation();
    crash = await getCrashReporting();
    await loadTemplates();
    ctMap = await getContentTemplates();
    try {
      lanIp = await localIp();
    } catch {
      lanIp = '';
    }
  });
  async function pickTranslation(id) {
    activeTranslation = id;
    await setActiveTranslation(id);
  }
</script>

<p class="r-lead s-lead">Configure AI recognition parameters, hardware routing, and network visibility for the live broadcasting environment.</p>

<div class="s-grid">
  <!-- AUDIO INPUT -->
  <section class="r-tile s-card">
    <header class="s-head">
      <span class="s-head-l">
        <svg class="s-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 2a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z"/><path d="M19 10v1a7 7 0 0 1-14 0v-1"/><line x1="12" y1="18" x2="12" y2="22"/></svg>
        Audio Input
      </span>
      {#if $capture.available}
        <span class="s-count">{$capture.devices.length} device{$capture.devices.length === 1 ? '' : 's'}</span>
      {:else}
        <span class="s-count">backend not attached</span>
      {/if}
    </header>

    <select class="r-select" value={$capture.inputDevice} on:change={(e) => setInputDevice(e.target.value)} disabled={!$capture.available || $capture.capturing}>
      <option value="">Default input</option>
      {#each $capture.devices as d}
        <option value={d.name}>{d.name}{d.is_default ? ' — default' : ''}</option>
      {/each}
    </select>

    <div class="s-meterwrap">
      <div class="s-meter"><i style="width:{levelPct}%;"></i></div>
      <div class="s-meter-scale"><span>-60dB</span><span>-18dB</span><span>0dB</span></div>
    </div>

    <div class="s-listen">
      <button class="r-btn amber" on:click={toggleCapture} disabled={!$capture.available}>
        {#if $capture.capturing}
          <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
          Stop listening
        {:else}
          <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><path d="M7 5.5v13l11-6.5-11-6.5z"/></svg>
          Start Listening
        {/if}
      </button>
      {#if $capture.capturing}
        <span class="s-rms" class:voice={$meter.isVoice}>
          <span class="s-dot" class:on={$meter.isVoice}></span>
          {$meter.isVoice ? 'voice' : 'silence'} · {$meter.level.toFixed(3)} rms
        </span>
      {/if}
    </div>
  </section>

  <!-- AI DETECTION THRESHOLDS -->
  <section class="r-tile s-card">
    <header class="s-head">
      <span class="s-head-l">
        <svg class="s-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3.2"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9L17 7M7 17l-2.1 2.1"/></svg>
        AI Detection Thresholds
      </span>
      <span class="s-count">self-calibrating</span>
    </header>

    <div class="s-slider">
      <div class="s-slider-top">
        <span class="r-lbl s-slider-name">Auto-fire above</span>
        <span class="s-slider-val">{Math.round($capture.thresholds.auto_fire * 100)}%</span>
      </div>
      <input class="r-range" type="range" min="0.5" max="0.99" step="0.01"
        value={$capture.thresholds.auto_fire}
        on:input={(e) => onAuto(+e.target.value)} disabled={!$capture.available} />
      <div class="s-slider-ends"><span>LAX (50%)</span><span>STRICT (100%)</span></div>
    </div>

    <div class="s-slider">
      <div class="s-slider-top">
        <span class="r-lbl s-slider-name">Suggest above</span>
        <span class="s-slider-val">{Math.round($capture.thresholds.suggest * 100)}%</span>
      </div>
      <input class="r-range" type="range" min="0.3" max="0.9" step="0.01"
        value={$capture.thresholds.suggest}
        on:input={(e) => onSuggest(+e.target.value)} disabled={!$capture.available} />
      <div class="s-slider-ends"><span>PASSIVE</span><span>HYPER-AWARE</span></div>
    </div>
  </section>

  <!-- RECOGNITION LANGUAGE -->
  <section class="r-tile s-card">
    <header class="s-head">
      <span class="s-head-l">
        <svg class="s-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 5h9M8.5 3v2M6 5c0 5 4 8 7 9M11 5c0 4-4 7-7 8"/><path d="M13 21l4-9 4 9M14.5 17.5h5"/></svg>
        Recognition Language
      </span>
      {#if $capture.capturing && $capture.detectedLang}
        <span class="s-count s-count-live">hearing: {$capture.detectedLang}</span>
      {/if}
    </header>

    <select class="r-select" value={$capture.stt.language ?? ''} on:change={(e) => setSttLanguage(e.target.value || null)} disabled={!$capture.stt.loaded}>
      <option value="">Auto-detect (code-switching)</option>
      <option value="en">English</option>
      <option value="yo">Yoruba</option>
      <option value="sw">Swahili</option>
      <option value="ha">Hausa</option>
    </select>
    <p class="s-note">Auto-detect handles English mixed with a local language mid-sentence — the normal case. Tier-1: Yoruba · Swahili · Hausa.</p>
  </section>

  <!-- BIBLE TRANSLATIONS -->
  <section class="r-tile s-card">
    <header class="s-head">
      <span class="s-head-l">
        <svg class="s-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M4 4.5A2.5 2.5 0 0 1 6.5 2H20v15H6.5A2.5 2.5 0 0 0 4 19.5z"/><path d="M4 19.5A2.5 2.5 0 0 0 6.5 22H20"/><path d="M12 6v6M9.5 8.5h5"/></svg>
        Bible Translations
      </span>
      <span class="r-lbl" style="letter-spacing:.06em;">read from</span>
    </header>
    <div class="s-checklist">
      {#if translations.length}
        {#each translations as t}
          <button class="s-tr" class:on={t.id === activeTranslation} on:click={() => pickTranslation(t.id)}>
            <span class="s-tr-dot" class:on={t.id === activeTranslation}></span>
            <span class="s-check-code">{t.abbreviation}</span>
            <span class="s-tr-name">{t.name}</span>
            {#if t.id === activeTranslation}<span class="s-tr-active r-mono">active</span>{/if}
          </button>
        {/each}
      {:else}
        <div class="r-empty" style="font-size:12.5px;">No translations loaded.</div>
      {/if}
    </div>
    <div class="s-tr-note r-mono">Only public-domain <b>KJV</b> is bundled. Additional versions need their verse data added to the corpus.</div>
  </section>

  <!-- CONTENT TEMPLATES -->
  <section class="r-tile s-card">
    <header class="s-head">
      <span class="s-head-l">
        <svg class="s-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m12 2 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5M3 17l9 5 9-5"/></svg>
        Content Templates
      </span>
      <span class="s-count">by type</span>
    </header>
    <p class="s-note" style="margin:0 0 12px">Each content type can use its own template automatically — lyrics in a lower-third, scripture full-screen. “Channel default” leaves the look to each output's own template.</p>
    <div class="s-net">
      {#each contentTypes as ct}
        <div class="s-netrow">
          <span class="s-netk">{ct.label}</span>
          <select class="r-select s-ctsel" value={ctMap[ct.key] ?? ''} on:change={(e) => pickCt(ct.key, e.target.value)}>
            <option value="">Channel default</option>
            {#each $templates as t}<option value={t.id}>{t.name}</option>{/each}
          </select>
        </div>
      {/each}
    </div>
  </section>

  <!-- NETWORK & KIOSKS -->
  <section class="r-tile s-card">
    <header class="s-head">
      <span class="s-head-l">
        <svg class="s-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="9" y="2" width="6" height="6" rx="1"/><rect x="2" y="16" width="6" height="6" rx="1"/><rect x="16" y="16" width="6" height="6" rx="1"/><path d="M12 8v4M5 16v-2h14v2"/></svg>
        Network &amp; Kiosks
      </span>
      <span class="s-count s-count-live">servers running</span>
    </header>

    <div class="s-net">
      <div class="s-netrow"><span class="s-netk">This machine</span><span class="s-netv r-mono">{lanIp || '—'}</span></div>
      <div class="s-netrow"><span class="s-netk">Output / stage pages</span><span class="s-netv r-mono">:8032 · http</span></div>
      <div class="s-netrow"><span class="s-netk">Live update channel</span><span class="s-netv r-mono">:8031 · websocket</span></div>
    </div>
    <p class="s-note">Add and manage network outputs (OBS · kiosk · stage remote) with copy-links and QR codes in the <b>Channels</b> tab — connected devices pull the live output from this machine, same Wi-Fi.</p>

    <div class="s-subhead r-lbl">Offline speech model</div>
    {#if $capture.stt.loaded}
      <div class="s-status ok s-model"><span class="s-sdot"></span>loaded</div>
      <div class="s-modelpath">{$capture.stt.model}</div>
    {:else}
      <!-- This used to read "no model — audio-only (see README dev setup)", i.e.
           the product told a church volunteer to go and read a developer README.
           Now it just installs it. -->
      <ModelSetup />
    {/if}
  </section>

  <!-- CRASH REPORTING — the only thing in Relay that sends anything anywhere. -->
  <section class="r-tile s-card">
    <header class="s-head">
      <span class="s-head-l">
        <svg class="s-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 3l7 3v6c0 4-3 7-7 9-4-2-7-5-7-9V6l7-3Z"/></svg>
        Crash Reporting
      </span>
      <span class="s-count">{crash.enabled ? 'on' : 'off'}</span>
    </header>

    <p class="s-tr-note">
      Relay is offline software: nothing you do here leaves this computer. Crash
      reporting is the one exception, and it is <b>off unless you turn it on</b>.
      <br /><br />
      If you turn it on, Relay sends only the technical details of a crash — the
      error, where in the code it happened, and your operating system.
      <b>Sermon transcripts, verse text, song lyrics, announcements and service
      names are never sent</b>, and are stripped from every report before it
      leaves. Reports are queued and sent later, so a bad network can never slow
      down a live service.
    </p>

    <label class="r-lbl" for="crash-dsn">Sentry DSN (your own project)</label>
    <input
      id="crash-dsn"
      class="r-input"
      type="text"
      placeholder="https://…@…ingest.sentry.io/…"
      bind:value={crash.dsn}
      disabled={!$capture.available} />

    <button
      class="r-btn"
      class:danger={crash.enabled}
      style="margin-top:10px;"
      on:click={() => toggleCrash(!crash.enabled)}
      disabled={!$capture.available}>
      {crash.enabled ? 'Turn crash reporting off' : 'Turn crash reporting on'}
    </button>

    {#if crashMsg}
      <div class="s-tr-note" style="margin-top:8px;">{crashMsg}</div>
    {/if}
  </section>
</div>

<style>
  .s-lead{ margin:0 0 20px; }

  .s-grid{ display:grid; grid-template-columns:1fr 1fr; gap:16px; align-items:start; }

  .s-card{ padding:20px 22px; display:flex; flex-direction:column; }

  .s-head{ display:flex; align-items:center; justify-content:space-between; gap:12px; margin-bottom:16px; }
  .s-head-l{ display:flex; align-items:center; gap:9px;
    font-family:var(--f-mono); font-size:11px; font-weight:600; letter-spacing:.13em;
    text-transform:uppercase; color:var(--v-amber); }
  .s-ic{ color:var(--v-amber); flex:0 0 auto; }
  .s-count{ font-family:var(--f-mono); font-size:10px; letter-spacing:.05em; color:var(--v-faint); }
  .s-count-live{ color:var(--v-amber); }

  /* level meter */
  .s-meterwrap{ margin-top:16px; }
  .s-meter{ height:7px; border-radius:99px; background:var(--v-surf3); overflow:hidden; }
  .s-meter i{ display:block; height:100%; border-radius:99px;
    background:linear-gradient(90deg,var(--v-amber),var(--v-amber2)); }
  .s-meter-scale{ display:flex; justify-content:space-between; margin-top:7px;
    font-family:var(--f-mono); font-size:9.5px; letter-spacing:.05em; color:var(--v-faint); }

  .s-listen{ display:flex; align-items:center; gap:14px; margin-top:18px; flex-wrap:wrap; }
  .s-rms{ font-family:var(--f-mono); font-size:11px; letter-spacing:.03em; color:var(--v-faint);
    display:inline-flex; align-items:center; gap:7px; }
  .s-rms.voice{ color:var(--v-emerald); }
  .s-dot{ width:7px; height:7px; border-radius:50%; background:var(--v-faint); }
  .s-dot.on{ background:var(--v-emerald); box-shadow:0 0 7px var(--v-emerald); }

  /* sliders */
  .s-slider{ margin-top:22px; }
  .s-slider:first-of-type{ margin-top:4px; }
  .s-slider-top{ display:flex; align-items:baseline; justify-content:space-between; margin-bottom:12px; }
  .s-slider-name{ color:var(--v-dim); }
  .s-slider-val{ font-family:var(--f-mono); font-size:18px; font-weight:500; color:var(--v-amber);
    font-variant-numeric:tabular-nums; }
  .s-slider-ends{ display:flex; justify-content:space-between; margin-top:9px;
    font-family:var(--f-mono); font-size:9.5px; letter-spacing:.06em; text-transform:uppercase; color:var(--v-faint); }

  /* recognition language */
  .s-note{ margin:10px 0 0; font-size:12px; line-height:1.6; color:var(--v-dim); }
  .s-note b{ color:var(--v-amber); }
  .s-subhead{ margin-top:20px; margin-bottom:10px; }

  /* bible translations */
  .s-checklist{ display:flex; flex-direction:column; gap:2px; }
  .s-check-code{ font-family:var(--f-mono); font-size:11px; font-weight:600; letter-spacing:.05em; color:var(--v-txt); }

  /* network info */
  .s-net{ display:flex; flex-direction:column; gap:8px; }
  .s-netrow{ display:flex; align-items:center; justify-content:space-between; gap:12px;
    padding:11px 13px; border-radius:9px; background:var(--v-surf2); border:1px solid var(--v-line); }
  .s-netk{ font-size:13px; color:var(--v-dim); }
  .s-netv{ font-size:11px; color:var(--v-txt); }
  .s-ctsel{ max-width:180px; height:34px; }
  .s-status{ display:inline-flex; align-items:center; gap:7px;
    font-family:var(--f-mono); font-size:11px; letter-spacing:.04em; }
  .s-status.ok{ color:var(--v-emerald); }
  .s-sdot{ width:7px; height:7px; border-radius:50%; background:currentColor; }
  .s-status.ok .s-sdot{ box-shadow:0 0 7px var(--v-emerald); }
  .s-model{ margin-top:2px; }
  .s-modelpath{ margin-top:6px; font-family:var(--f-mono); font-size:10px; line-height:1.5;
    color:var(--v-faint); word-break:break-all; }

  @media (max-width:820px){
    .s-grid{ grid-template-columns:1fr; }
  }

  /* Bible translation picker */
  .s-tr{ display:flex; align-items:center; gap:11px; width:100%; text-align:left; cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line); border-radius:9px; padding:10px 12px;
    color:var(--v-txt); font-family:var(--f-body); font-size:13px; transition:border-color .14s, background .14s; }
  .s-tr:hover{ border-color:var(--v-line2); }
  .s-tr.on{ border-color:rgba(245,166,35,.4); background:var(--v-amber-soft); }
  .s-tr-dot{ width:14px; height:14px; border-radius:50%; flex:0 0 auto; border:2px solid var(--v-faint); }
  .s-tr-dot.on{ border-color:var(--v-amber); background:radial-gradient(circle,var(--v-amber) 40%,transparent 45%); }
  .s-tr-name{ color:var(--v-dim); flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .s-tr-active{ font-size:9px; letter-spacing:.1em; text-transform:uppercase; color:var(--v-amber); }
  .s-tr-note{ font-size:10px; color:var(--v-faint); margin-top:10px; line-height:1.6; }
  .s-tr-note b{ color:var(--v-dim); }
</style>
