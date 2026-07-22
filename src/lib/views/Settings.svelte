<script>
  import { onMount, onDestroy } from 'svelte';
  import ModelSetup from '../ModelSetup.svelte';
  import History from './library/History.svelte';
  import { locale, setLocale, LOCALES, t } from '../i18n.js';
  import { restartSetup, setSession } from '../session.js';
  import { humanError } from '../errors.js';
  import { safeMode, setSafeMode } from '../boot/boot.js';
  import { checkForUpdate, updateAvailable } from '../updater.js';
  import en from '../locales/en.json';
  import yo from '../locales/yo.json';
  import sw from '../locales/sw.json';
  import ha from '../locales/ha.json';

  // How much of the console each language actually covers, computed from the catalogues
  // themselves rather than claimed. Shown because it is TRUE — 0% is an invitation, not a
  // failure to hide.
  const CATALOGUES = { en, yo, sw, ha };
  const TOTAL = Object.keys(en).filter((k) => !k.startsWith('_')).length;
  const coverage = (code) =>
    Math.round(
      (Object.keys(CATALOGUES[code] ?? {}).filter((k) => !k.startsWith('_')).length / TOTAL) * 100,
    );
  import { capture, meter, templates, initAudio, startCapture, stopCapture, setThresholds, setSttLanguage, setInputDevice, listTranslations, getActiveTranslation, setActiveTranslation, localIp, loadTemplates, getContentTemplates, setContentTemplate, getCrashReporting, setCrashReporting } from '../stores/capture.js';

  // ─────────────────────────────────────────────────────────────────────────
  // SECTION NAV. The screen is one big config surface split into ref-matched
  // sections; the rail on the left picks which one is shown.
  // ─────────────────────────────────────────────────────────────────────────
  const SECTIONS = [
    { key: 'general',   label: 'General',           desc: 'Basic application preferences and behaviour', icon: 'gear' },
    { key: 'outputs',   label: 'Outputs',           desc: 'Per-content-type templates and output routing', icon: 'monitor' },
    { key: 'audio',     label: 'Audio',             desc: 'Microphone input and live level', icon: 'mic' },
    { key: 'scripture', label: 'Scripture & Bible', desc: 'Recognition language and Bible translations', icon: 'book' },
    { key: 'ai',        label: 'AI & Detection',    desc: 'Detection thresholds and the run engine', icon: 'sparkle' },
    { key: 'shortcuts', label: 'Shortcuts',         desc: 'Keyboard controls for the live desk', icon: 'keyboard' },
    { key: 'network',   label: 'Network',           desc: 'Kiosk, output and stage distribution', icon: 'nodes' },
    { key: 'history',   label: 'Service History',   desc: 'Past services recorded locally', icon: 'clock' },
    { key: 'backup',    label: 'Backup & Recovery', desc: 'Setup walk-through and safe mode', icon: 'shield' },
    { key: 'updates',   label: 'Updates',           desc: 'App version and update channel', icon: 'refresh' },
    { key: 'advanced',  label: 'Advanced',          desc: 'Crash reporting and privacy', icon: 'terminal' },
    { key: 'account',   label: 'Account',           desc: 'Licence and machine details', icon: 'user' },
  ];
  let section = 'general';
  $: activeSection = SECTIONS.find((s) => s.key === section) ?? SECTIONS[0];

  const ICONS = {
    gear: '<circle cx="12" cy="12" r="3.2"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M19.1 4.9L17 7M7 17l-2.1 2.1"/>',
    monitor: '<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>',
    mic: '<path d="M12 2a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z"/><path d="M19 10v1a7 7 0 0 1-14 0v-1"/><line x1="12" y1="18" x2="12" y2="22"/>',
    book: '<path d="M4 4.5A2.5 2.5 0 0 1 6.5 2H20v15H6.5A2.5 2.5 0 0 0 4 19.5z"/><path d="M4 19.5A2.5 2.5 0 0 0 6.5 22H20"/>',
    sparkle: '<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8L12 3Z"/>',
    keyboard: '<rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M18 10h.01M6 14h12"/>',
    nodes: '<rect x="9" y="2" width="6" height="6" rx="1"/><rect x="2" y="16" width="6" height="6" rx="1"/><rect x="16" y="16" width="6" height="6" rx="1"/><path d="M12 8v4M5 16v-2h14v2"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    shield: '<path d="M12 3l7 3v6c0 4-3 7-7 9-4-2-7-5-7-9V6l7-3Z"/>',
    refresh: '<path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>',
    terminal: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9l3 3-3 3M13 15h4"/>',
    user: '<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4 4-6 8-6s8 2 8 6"/>',
  };

  // ─────────────────────────────────────────────────────────────────────────
  // GENERAL PREFERENCES. Harmless UI preferences, persisted to localStorage.
  // (Auto-start / tray hooks require an OS-level integration Relay does not yet
  // ship, so those are stored as intent and applied when that lands — noted in
  // the design log. The functional controls — language, safe mode, thresholds,
  // templates — live in their own sections and are wired to the real engine.)
  // ─────────────────────────────────────────────────────────────────────────
  const PREFS_KEY = 'relay.prefs.v1';
  const DEFAULT_PREFS = {
    theme: 'dark',
    autoStart: false,
    minimizeTray: true,
    confirmLive: true,
    autoSave: true,
    defaultContent: 'scripture',
    timeFormat: '24',
    dateFormat: 'DD/MM/YYYY',
    restoreSession: true,
    startupScreen: 'dashboard',
  };
  let prefs = { ...DEFAULT_PREFS };
  function loadPrefs() {
    try {
      prefs = { ...DEFAULT_PREFS, ...(JSON.parse(localStorage.getItem(PREFS_KEY) || '{}')) };
    } catch {
      prefs = { ...DEFAULT_PREFS };
    }
  }
  function savePrefs() {
    try {
      localStorage.setItem(PREFS_KEY, JSON.stringify(prefs));
    } catch {
      /* locked-down webview — the defaults are a fine answer */
    }
  }
  function setPref(key, value) {
    prefs = { ...prefs, [key]: value };
    savePrefs();
    if (key === 'theme') applyTheme(value);
  }
  // Only Dark is fully styled today (the whole console is a dark surface). The
  // control persists the choice and stamps data-theme so a future light sheet
  // can key off it; picking Light/System does not yet repaint. See design log.
  function applyTheme(v) {
    try {
      document.documentElement.dataset.theme = v;
    } catch {
      /* no DOM (test env) */
    }
  }

  // The four toggles rendered as a data-driven list, matching the reference's
  // stacked switch rows.
  const GENERAL_TOGGLES = [
    { key: 'autoStart',    title: 'Auto Start on Login',   note: 'Launch Relay automatically when you log in to your computer.' },
    { key: 'minimizeTray', title: 'Minimize to System Tray', note: 'Minimize the application to the system tray instead of the taskbar.' },
    { key: 'confirmLive',  title: 'Confirm Before Going Live', note: 'Show a confirmation dialog before sending content live.' },
    { key: 'autoSave',     title: 'Auto Save',             note: 'Automatically save changes in templates, plans and settings.' },
  ];

  // ─────────────────────────────────────────────────────────────────────────
  // Crash reporting — OFF by default. The only thing in Relay that can send
  // anything off the device, so the UI states plainly what is and isn't sent.
  // ─────────────────────────────────────────────────────────────────────────
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
      crashMsg = humanError(e);
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

  // --- live audio input (real cpal capture through the Rust engine) ---
  onMount(initAudio);
  // NOTE: capture is app-level state, NOT tied to this view's lifetime — do not
  // stop it on unmount, or switching tabs would kill the mic mid-service.

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

  // ─── System overview (right rail) ───────────────────────────────────────
  let appVersion = '';
  const environment = import.meta.env?.DEV ? 'Development' : 'Production';
  let bootAt = 0;
  let uptime = '—';
  let uptimeTimer = null;
  function fmtUptime(ms) {
    const s = Math.floor(ms / 1000);
    const d = Math.floor(s / 86400);
    const h = Math.floor((s % 86400) / 3600);
    const m = Math.floor((s % 3600) / 60);
    if (d) return `${d}d ${h}h ${m}m`;
    if (h) return `${h}h ${m}m`;
    return `${m}m`;
  }

  let updateMsg = '';
  let checking = false;
  async function doCheckUpdates() {
    checking = true;
    updateMsg = '';
    try {
      const v = await checkForUpdate();
      updateMsg = v ? `Relay ${v} is available.` : "You're on the latest version.";
    } catch (e) {
      updateMsg = humanError(e);
    }
    checking = false;
  }

  // Two-step arm/confirm, NOT confirm(): the Tauri webview does not implement the
  // native confirm() dialog (see Channels.svelte), so it may never return true and
  // the button would be silently dead — or behave differently across platforms.
  // First click arms for 3s (the button asks for confirmation), second click does
  // it. Same pattern as deleting a channel.
  let resetArmed = false;
  let resetArmT;
  function resetAllSettings() {
    if (!resetArmed) {
      resetArmed = true;
      clearTimeout(resetArmT);
      resetArmT = setTimeout(() => (resetArmed = false), 3000);
      return;
    }
    clearTimeout(resetArmT);
    resetArmed = false;
    prefs = { ...DEFAULT_PREFS };
    savePrefs();
    applyTheme(prefs.theme);
  }

  onMount(async () => {
    loadPrefs();
    applyTheme(prefs.theme);
    // Session uptime — a real, honest number (this run of the app).
    bootAt = performance.now();
    uptime = fmtUptime(0);
    uptimeTimer = setInterval(() => { uptime = fmtUptime(performance.now() - bootAt); }, 30000);
    try {
      const { getVersion } = await import('@tauri-apps/api/app');
      appVersion = await getVersion();
    } catch {
      appVersion = '';
    }
    // Guarded as a block: an unguarded reject on any one of these aborts the rest
    // of mount, so crash state, content-type templates and the LAN IP would all
    // silently fail to initialise off a single backend hiccup.
    try {
      translations = await listTranslations();
      activeTranslation = await getActiveTranslation();
      crash = await getCrashReporting();
      await loadTemplates();
      ctMap = await getContentTemplates();
    } catch (e) {
      crashMsg = humanError(e);
    }
    try {
      lanIp = await localIp();
    } catch {
      lanIp = '';
    }
  });
  onDestroy(() => uptimeTimer && clearInterval(uptimeTimer));

  async function pickTranslation(id) {
    const prev = activeTranslation;
    activeTranslation = id;
    try {
      await setActiveTranslation(id);
    } catch (e) {
      activeTranslation = prev; // revert so the UI never claims a switch that failed
      crashMsg = humanError(e);
    }
  }

  // Keyboard shortcuts shown in the Shortcuts section — the panic + transport
  // keys the app actually binds (lib/shortcuts.js), plus Help points to more.
  const SHORTCUTS = [
    { keys: ['Space'], label: 'Advance — step the plan / walk the passage' },
    { keys: ['→'], label: 'Next (mode-aware: plan slide or verse)' },
    { keys: ['←'], label: 'Previous' },
    { keys: ['Esc'], label: 'Clear all output screens' },
    { keys: ['B'], label: 'Blackout every output' },
    { keys: ['?'], label: 'Open Help & full shortcut list' },
  ];
</script>

<div class="s-page">
  <header class="s-pagehead">
    <div>
      <h1 class="s-title">Settings</h1>
      <p class="s-sub">Configure Relay to match your environment and workflow.</p>
    </div>
  </header>

  <div class="s-layout">
    <!-- ════ SECTION RAIL ════ -->
    <aside class="s-rail">
      <nav class="s-railnav">
        {#each SECTIONS as s}
          <button
            class="s-railbtn r-focus"
            class:on={section === s.key}
            aria-pressed={section === s.key}
            on:click={() => (section = s.key)}
          >
            <svg class="s-railic" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html ICONS[s.icon]}</svg>
            <span class="s-raillbl">{s.label}</span>
          </button>
        {/each}
      </nav>
      <button class="r-btn ghost sm s-reset" class:arm={resetArmed} on:click={resetAllSettings}>
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
        {resetArmed ? 'Click again to reset' : 'Reset to Defaults'}
      </button>
    </aside>

    <!-- ════ ACTIVE PANEL ════ -->
    <main class="s-panel">
      <div class="s-panelhead">
        <h2 class="s-paneltitle">{activeSection.label}</h2>
        <p class="s-paneldesc">{activeSection.desc}</p>
      </div>

      {#if section === 'general'}
        <!-- Application language -->
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Application Language</div>
            <div class="s-rownote">Choose the language for the operator console. Missing words stay in English.</div>
          </div>
          <select class="r-select s-rowctl" value={$locale} on:change={(e) => setLocale(e.target.value)}>
            {#each LOCALES as l}
              {@const pct = coverage(l.code)}
              <option value={l.code}>{l.label}{pct === 100 ? '' : ` · ${pct}%`}</option>
            {/each}
          </select>
        </div>

        <!-- Theme -->
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Theme</div>
            <div class="s-rownote">Select your preferred colour theme. Only Dark is styled today.</div>
          </div>
          <div class="s-seg" role="group" aria-label="Theme">
            {#each [['light','Light','sun'],['dark','Dark','moon'],['system','System','monitor']] as [val, lbl, ic]}
              <button class="s-segbtn" class:on={prefs.theme === val} aria-pressed={prefs.theme === val} on:click={() => setPref('theme', val)}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  {#if ic === 'sun'}<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4 12H2M22 12h-2M5 5l1.5 1.5M17.5 17.5L19 19M19 5l-1.5 1.5M6.5 17.5L5 19"/>
                  {:else if ic === 'moon'}<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z"/>
                  {:else}<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M8 20h8M12 16v4"/>{/if}
                </svg>
                {lbl}
              </button>
            {/each}
          </div>
        </div>

        <!-- Toggles -->
        {#each GENERAL_TOGGLES as tg}
          <div class="s-row">
            <div class="s-rowtext">
              <div class="s-rowtitle">{tg.title}</div>
              <div class="s-rownote">{tg.note}</div>
            </div>
            <button
              class="s-toggle"
              class:on={prefs[tg.key]}
              role="switch"
              aria-checked={prefs[tg.key]}
              aria-label={tg.title}
              on:click={() => setPref(tg.key, !prefs[tg.key])}
            ><span class="s-knob"></span></button>
          </div>
        {/each}

        <!-- Default content type -->
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Default Content Type</div>
            <div class="s-rownote">Set the default content type when creating new items.</div>
          </div>
          <select class="r-select s-rowctl" value={prefs.defaultContent} on:change={(e) => setPref('defaultContent', e.target.value)}>
            <option value="scripture">Scripture</option>
            <option value="song">Lyrics</option>
            <option value="media">Media</option>
            <option value="announce">Announcements</option>
          </select>
        </div>

        <!-- Time format -->
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Time Format</div>
            <div class="s-rownote">Choose how time is displayed across the application.</div>
          </div>
          <div class="s-seg" role="group" aria-label="Time format">
            <button class="s-segbtn" class:on={prefs.timeFormat === '12'} aria-pressed={prefs.timeFormat === '12'} on:click={() => setPref('timeFormat', '12')}>12-hour</button>
            <button class="s-segbtn" class:on={prefs.timeFormat === '24'} aria-pressed={prefs.timeFormat === '24'} on:click={() => setPref('timeFormat', '24')}>24-hour</button>
          </div>
        </div>

        <!-- Date format -->
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Date Format</div>
            <div class="s-rownote">Choose how dates are displayed across the application.</div>
          </div>
          <select class="r-select s-rowctl" value={prefs.dateFormat} on:change={(e) => setPref('dateFormat', e.target.value)}>
            <option value="DD/MM/YYYY">DD/MM/YYYY</option>
            <option value="MM/DD/YYYY">MM/DD/YYYY</option>
            <option value="YYYY-MM-DD">YYYY-MM-DD</option>
          </select>
        </div>

        <div class="s-grouphead">Startup</div>
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Restore Previous Session</div>
            <div class="s-rownote">Automatically restore the last active screen on startup.</div>
          </div>
          <button class="s-toggle" class:on={prefs.restoreSession} role="switch" aria-checked={prefs.restoreSession} aria-label="Restore previous session" on:click={() => setPref('restoreSession', !prefs.restoreSession)}><span class="s-knob"></span></button>
        </div>
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Default Startup Screen</div>
            <div class="s-rownote">Choose which screen to show when Relay starts.</div>
          </div>
          <select class="r-select s-rowctl" value={prefs.startupScreen} on:change={(e) => setPref('startupScreen', e.target.value)}>
            <option value="dashboard">Dashboard</option>
            <option value="live">Live</option>
            <option value="planner">Planner</option>
            <option value="library">Library</option>
          </select>
        </div>

      {:else if section === 'outputs'}
        <p class="s-lead">Each content type can use its own template automatically — lyrics in a lower-third, scripture full-screen. “Channel default” leaves the look to each output's own template.</p>
        <div class="s-cardbox">
          {#each contentTypes as ct}
            <div class="s-netrow">
              <span class="s-netk">{ct.label}</span>
              <select class="r-select s-ctsel" value={ctMap[ct.key] ?? ''} on:change={(e) => pickCt(ct.key, e.target.value)}>
                <option value="">Channel default</option>
                {#each $templates as tpl}<option value={tpl.id}>{tpl.name}</option>{/each}
              </select>
            </div>
          {/each}
        </div>
        <p class="s-note">Add and manage network outputs (OBS · kiosk · stage remote) with copy-links and QR codes in the <b>Channels</b> tab.</p>

      {:else if section === 'audio'}
        <div class="s-inline">
          {#if $capture.available}
            <span class="s-count">{$capture.devices.length} device{$capture.devices.length === 1 ? '' : 's'}</span>
          {:else}
            <span class="s-count">backend not attached</span>
          {/if}
        </div>
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
          <button class="r-btn primary" on:click={toggleCapture} disabled={!$capture.available}>
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

      {:else if section === 'scripture'}
        <div class="s-grouphead first">Recognition Language</div>
        <select class="r-select" value={$capture.stt.language ?? ''} on:change={(e) => setSttLanguage(e.target.value || null)} disabled={!$capture.stt.loaded}>
          <option value="">Auto-detect (code-switching)</option>
          <option value="en">English</option>
          <option value="yo">Yoruba</option>
          <option value="sw">Swahili</option>
          <option value="ha">Hausa</option>
        </select>
        <p class="s-note">Auto-detect handles English mixed with a local language mid-sentence — the normal case. Tier-1: Yoruba · Swahili · Hausa.</p>

        <div class="s-grouphead">Bible Translations</div>
        <div class="s-checklist">
          {#if translations.length}
            {#each translations as tr}
              <button class="s-tr" class:on={tr.id === activeTranslation} aria-pressed={tr.id === activeTranslation} on:click={() => pickTranslation(tr.id)}>
                <span class="s-tr-dot" class:on={tr.id === activeTranslation}></span>
                <span class="s-check-code">{tr.abbreviation}</span>
                <span class="s-tr-name">{tr.name}</span>
                {#if tr.id === activeTranslation}<span class="s-tr-active r-mono">active</span>{/if}
              </button>
            {/each}
          {:else}
            <div class="r-empty" style="font-size:12.5px;">No translations loaded.</div>
          {/if}
        </div>
        <div class="s-tr-note r-mono">Only public-domain <b>KJV</b> is bundled. Additional versions need their verse data added to the corpus.</div>

      {:else if section === 'ai'}
        <div class="s-inline"><span class="s-count">self-calibrating</span></div>
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
        <p class="s-note">Only a direct, high-confidence quotation can ever auto-fire. A paraphrase is always a suggestion — a cosine is not a probability.</p>

      {:else if section === 'shortcuts'}
        <p class="s-lead">The live desk is driven from the keyboard. These bindings are always active; the full list lives in Help.</p>
        <div class="s-cardbox">
          {#each SHORTCUTS as sc}
            <div class="s-scrow">
              <span class="s-sckeys">{#each sc.keys as k}<kbd class="s-kbd">{k}</kbd>{/each}</span>
              <span class="s-scnote">{sc.label}</span>
            </div>
          {/each}
        </div>
        <button class="r-btn ghost sm" on:click={() => setSession({ activeTab: 'help' })}>Open Help &amp; Shortcuts</button>

      {:else if section === 'network'}
        <div class="s-cardbox">
          <div class="s-netrow"><span class="s-netk">This machine</span><span class="s-netv r-mono">{lanIp || '—'}</span></div>
          <div class="s-netrow"><span class="s-netk">Output / stage pages</span><span class="s-netv r-mono">:8032 · http</span></div>
          <div class="s-netrow"><span class="s-netk">Live update channel</span><span class="s-netv r-mono">:8031 · websocket</span></div>
        </div>
        <p class="s-note">Connected devices (OBS · kiosk · stage remote) pull the live output from this machine on the same Wi-Fi. Manage them in the <b>Channels</b> tab.</p>

        <div class="s-grouphead">Offline speech model</div>
        {#if $capture.stt.loaded}
          <div class="s-status ok s-model"><span class="s-sdot"></span>loaded</div>
          <div class="s-modelpath">{$capture.stt.model}</div>
        {:else}
          <ModelSetup />
        {/if}

      {:else if section === 'history'}
        <!-- History moved into Settings. The view is self-contained (its own list,
             detail, search, export) and reads from the same local SQLite store. -->
        <div class="s-history"><History /></div>

      {:else if section === 'backup'}
        <p class="s-lead">Relay stores everything locally. Use the walk-through to re-check your projector and microphone, and safe mode to disarm every output.</p>
        <div class="s-note" style="margin-top:0">
          <b>New here?</b> The setup walk-through picks your projector, checks the microphone is actually hearing something, and ends by putting a real verse on your real screen — so you have <i>seen</i> it work before Sunday.
        </div>
        <button class="r-btn ghost sm s-mt" on:click={restartSetup}>Run the setup walk-through</button>

        {#if $safeMode}
          <hr class="s-rule" />
          <p class="s-note">
            <b style="color:var(--v-amethyst);">Safe mode is on.</b> Outputs will not open and detection is disarmed — nothing Relay does can reach a screen. Turn it off before you run a service.
          </p>
          <button class="r-btn ghost sm s-mt" on:click={() => setSafeMode(false)}>Turn off safe mode</button>
        {:else}
          <hr class="s-rule" />
          <p class="s-note">Safe mode disarms every output and detection — a way to open the console without any risk of putting something on a wall.</p>
          <button class="r-btn ghost sm s-mt" on:click={() => setSafeMode(true)}>Turn on safe mode</button>
        {/if}

      {:else if section === 'updates'}
        <div class="s-cardbox">
          <div class="s-netrow"><span class="s-netk">Installed version</span><span class="s-netv r-mono">{appVersion || '—'}</span></div>
          <div class="s-netrow"><span class="s-netk">Environment</span><span class="s-netv r-mono">{environment}</span></div>
          <div class="s-netrow"><span class="s-netk">Update status</span><span class="s-netv r-mono">{$updateAvailable ? `${$updateAvailable.version} available` : 'up to date'}</span></div>
        </div>
        <button class="r-btn primary sm s-mt" on:click={doCheckUpdates} disabled={checking}>
          {checking ? 'Checking…' : 'Check for Updates'}
        </button>
        {#if updateMsg}<div class="s-note" style="margin-top:10px">{updateMsg}</div>{/if}

      {:else if section === 'advanced'}
        <div class="s-grouphead first">Crash Reporting</div>
        <p class="s-tr-note">
          Relay is offline software: nothing you do here leaves this computer. Crash reporting is the one exception, and it is <b>off unless you turn it on</b>.
          <br /><br />
          If you turn it on, Relay sends only the technical details of a crash — the error, where in the code it happened, and your operating system. <b>Sermon transcripts, verse text, song lyrics, announcements and service names are never sent</b>, and are stripped from every report before it leaves. Reports are queued and sent later, so a bad network can never slow down a live service.
        </p>
        <label class="r-lbl s-mt" for="crash-dsn">Sentry DSN (your own project)</label>
        <input id="crash-dsn" class="r-input" type="text" placeholder="https://…@…ingest.sentry.io/…" bind:value={crash.dsn} disabled={!$capture.available} />
        <button class="r-btn" class:danger={crash.enabled} style="margin-top:10px;" on:click={() => toggleCrash(!crash.enabled)} disabled={!$capture.available}>
          {crash.enabled ? 'Turn crash reporting off' : 'Turn crash reporting on'}
        </button>
        {#if crashMsg}<div class="s-tr-note" style="margin-top:8px;">{crashMsg}</div>{/if}

      {:else if section === 'account'}
        <div class="s-cardbox">
          <div class="s-netrow"><span class="s-netk">Licence</span><span class="s-netv r-mono">MIT · open source</span></div>
          <div class="s-netrow"><span class="s-netk">Environment</span><span class="s-netv r-mono">{environment}</span></div>
          <div class="s-netrow"><span class="s-netk">Version</span><span class="s-netv r-mono">{appVersion || '—'}</span></div>
        </div>
        <p class="s-note">Relay is free and open source. There is no account to sign in to and nothing to pay — every feature works offline, on this machine.</p>
      {/if}
    </main>

    <!-- ════ OVERVIEW RAIL ════ -->
    <aside class="s-over">
      <div class="s-ocard">
        <div class="s-ohead">System Overview</div>
        <div class="s-orow"><span class="s-ok">Version</span><span class="s-ov r-mono">{appVersion || '—'}</span></div>
        <div class="s-orow"><span class="s-ok">Environment</span><span class="r-badge" class:emerald={environment === 'Production'} class:grey={environment !== 'Production'}>{environment}</span></div>
        <div class="s-orow"><span class="s-ok">Licence</span><span class="r-badge emerald">MIT</span></div>
        <div class="s-orow"><span class="s-ok">Uptime</span><span class="s-ov r-mono">{uptime}</span></div>
      </div>

      <div class="s-ocard">
        <div class="s-ohead">Quick Links</div>
        <button class="s-qlink" on:click={() => (section = 'shortcuts')}>
          <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 10h.01M10 10h.01M14 10h.01M6 14h12"/></svg>
          <span class="s-qtext"><b>Keyboard Shortcuts</b><em>View and customise shortcuts</em></span>
          <svg class="s-qarr" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 18l6-6-6-6"/></svg>
        </button>
        <button class="s-qlink" on:click={() => (section = 'updates')}>
          <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
          <span class="s-qtext"><b>Check for Updates</b><em>{$updateAvailable ? 'An update is waiting' : "You're on the latest version"}</em></span>
          <svg class="s-qarr" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 18l6-6-6-6"/></svg>
        </button>
        <button class="s-qlink" on:click={() => (section = 'history')}>
          <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>
          <span class="s-qtext"><b>Service History</b><em>Review past services</em></span>
          <svg class="s-qarr" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 18l6-6-6-6"/></svg>
        </button>
        <button class="s-qlink" on:click={() => setSession({ activeTab: 'help' })}>
          <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M9.5 9a2.5 2.5 0 0 1 5 .3c0 1.7-2.5 2-2.5 3.7M12 17h.01"/></svg>
          <span class="s-qtext"><b>Support &amp; Guide</b><em>Get help and documentation</em></span>
          <svg class="s-qarr" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M9 18l6-6-6-6"/></svg>
        </button>
      </div>

      <div class="s-ocard danger">
        <div class="s-ohead danger">Danger Zone</div>
        <div class="s-drow">
          <span class="s-qtext"><b>Reset All Settings</b><em>{resetArmed ? 'Click the button again to confirm' : 'Restore local preferences to default'}</em></span>
          <button class="s-dbtn" class:arm={resetArmed} on:click={resetAllSettings} aria-label={resetArmed ? 'Confirm reset all settings' : 'Reset all settings'}>
            <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/></svg>
          </button>
        </div>
      </div>
    </aside>
  </div>
</div>

<style>
  .s-page{ display:flex; flex-direction:column; gap:22px; }
  .s-pagehead{ display:flex; align-items:flex-start; justify-content:space-between; gap:24px; }
  .s-title{ margin:0; font-family:var(--f-head); font-size:var(--v-fs-h1); line-height:var(--v-lh-h1);
    font-weight:700; letter-spacing:var(--v-tr-tight); color:var(--v-txt); }
  .s-sub{ margin:6px 0 0; font-size:13.5px; color:var(--v-dim); }

  /* 3-column layout: section rail · panel · overview rail */
  .s-layout{ display:grid; grid-template-columns:212px minmax(0,1fr) 288px; gap:20px; align-items:start; }

  /* ── SECTION RAIL ── */
  .s-rail{ position:sticky; top:0; display:flex; flex-direction:column; gap:14px; }
  .s-railnav{ display:flex; flex-direction:column; gap:3px; }
  .s-railbtn{ display:flex; align-items:center; gap:11px; width:100%; text-align:left; cursor:pointer;
    padding:9px 12px; border-radius:var(--v-r-md); border:1px solid transparent; background:transparent;
    color:var(--v-dim); font-family:var(--f-body); font-size:13.5px; font-weight:500;
    transition:background .13s, color .13s, border-color .13s; }
  .s-railbtn:hover{ background:var(--v-surf); color:var(--v-txt); }
  .s-railbtn.on{ background:var(--v-accent-soft); border-color:var(--v-accent-line); color:var(--v-accent2); font-weight:600; }
  .s-railic{ flex:0 0 auto; }
  .s-raillbl{ overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .s-reset{ justify-content:center; width:100%; margin-top:2px; }

  /* ── ACTIVE PANEL ── the cards float on the page, no outer box (matches ref) */
  .s-panel{ min-width:0; display:flex; flex-direction:column; gap:12px; }
  .s-panelhead{ margin-bottom:6px; }
  .s-paneltitle{ margin:0; font-family:var(--f-head); font-size:var(--v-fs-h2); line-height:var(--v-lh-h2);
    font-weight:600; letter-spacing:var(--v-tr-h2); color:var(--v-txt); }
  .s-paneldesc{ margin:5px 0 0; font-size:13px; color:var(--v-dim); }

  .s-lead{ margin:0 0 4px; font-size:13px; line-height:1.6; color:var(--v-dim); }
  .s-inline{ display:flex; justify-content:flex-end; }

  /* Setting rows (General) — each its own bordered card */
  .s-row{ display:flex; align-items:center; justify-content:space-between; gap:20px;
    padding:16px 20px; border:1px solid var(--v-line); border-radius:var(--v-r-lg);
    background:var(--v-surf); }
  .s-rowtext{ min-width:0; }
  .s-rowtitle{ font-size:14px; font-weight:600; color:var(--v-txt); }
  .s-rownote{ margin-top:3px; font-size:12px; line-height:1.5; color:var(--v-faint); }
  .s-rowctl{ flex:0 0 auto; min-width:170px; max-width:220px; }

  .s-grouphead{ margin:14px 0 2px; font-family:var(--f-mono); font-size:11px; font-weight:600;
    letter-spacing:.14em; text-transform:uppercase; color:var(--v-faint); }
  .s-grouphead.first{ margin-top:0; }

  /* Segmented control (theme, time format) */
  .s-seg{ display:inline-flex; gap:4px; padding:4px; border-radius:var(--v-r-md);
    background:var(--v-void); border:1px solid var(--v-line); flex:0 0 auto; }
  .s-segbtn{ display:inline-flex; align-items:center; gap:6px; padding:7px 14px; border:0; cursor:pointer;
    border-radius:6px; background:transparent; color:var(--v-dim); font-family:var(--f-body);
    font-size:12.5px; font-weight:500; transition:background .13s, color .13s; }
  .s-segbtn:hover{ color:var(--v-txt); }
  .s-segbtn.on{ background:var(--v-accent-fill); color:var(--v-accent-ink); box-shadow:var(--v-shadow-sm); }

  /* Toggle switch */
  .s-toggle{ position:relative; flex:0 0 auto; width:44px; height:24px; border-radius:99px; cursor:pointer;
    border:1px solid var(--v-line2); background:var(--v-surf3); padding:0; transition:background .16s, border-color .16s; }
  .s-toggle.on{ background:var(--v-accent-fill); border-color:var(--v-accent-fill); }
  .s-knob{ position:absolute; top:2px; left:2px; width:18px; height:18px; border-radius:50%;
    background:#fff; transition:transform .16s; box-shadow:0 1px 2px rgba(0,0,0,.4); }
  .s-toggle.on .s-knob{ transform:translateX(20px); }

  /* Boxed rows (outputs, network, updates, account, shortcuts) */
  .s-cardbox{ display:flex; flex-direction:column; gap:8px; }
  .s-netrow{ display:flex; align-items:center; justify-content:space-between; gap:12px;
    padding:12px 14px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line); }
  .s-netk{ font-size:13px; color:var(--v-dim); }
  .s-netv{ font-size:11px; color:var(--v-txt); }
  .s-ctsel{ max-width:200px; height:34px; }

  .s-note{ margin:14px 0 0; font-size:12px; line-height:1.6; color:var(--v-dim); }
  .s-note b{ color:var(--v-accent); }
  .s-mt{ margin-top:14px; }
  .s-rule{ border:0; border-top:1px solid var(--v-line); margin:18px 0 0; }

  /* Shortcuts */
  .s-scrow{ display:flex; align-items:center; gap:14px; padding:11px 14px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); }
  .s-sckeys{ flex:0 0 118px; display:flex; gap:5px; }
  .s-kbd{ font-family:var(--f-mono); font-size:11px; color:var(--v-txt); background:var(--v-void);
    border:1px solid var(--v-line2); border-bottom-width:2px; border-radius:5px; padding:2px 7px; }
  .s-scnote{ font-size:12.5px; color:var(--v-dim); }

  /* level meter */
  .s-meterwrap{ margin-top:16px; }
  .s-meter{ height:7px; border-radius:99px; background:var(--v-surf3); overflow:hidden; }
  .s-meter i{ display:block; height:100%; border-radius:99px;
    background:linear-gradient(90deg,var(--v-accent),var(--v-accent2)); }
  .s-meter-scale{ display:flex; justify-content:space-between; margin-top:7px;
    font-family:var(--f-mono); font-size:9.5px; letter-spacing:.05em; color:var(--v-faint); }
  .s-listen{ display:flex; align-items:center; gap:14px; margin-top:18px; flex-wrap:wrap; }
  .s-rms{ font-family:var(--f-mono); font-size:11px; letter-spacing:.03em; color:var(--v-faint);
    display:inline-flex; align-items:center; gap:7px; }
  .s-rms.voice{ color:var(--v-emerald); }
  .s-dot{ width:7px; height:7px; border-radius:50%; background:var(--v-faint); }
  .s-dot.on{ background:var(--v-emerald); box-shadow:0 0 7px var(--v-emerald); }
  .s-count{ font-family:var(--f-mono); font-size:10px; letter-spacing:.05em; color:var(--v-faint); }

  /* sliders */
  .s-slider{ margin-top:22px; }
  .s-slider:first-of-type{ margin-top:4px; }
  .s-slider-top{ display:flex; align-items:baseline; justify-content:space-between; margin-bottom:12px; }
  .s-slider-name{ color:var(--v-dim); }
  .s-slider-val{ font-family:var(--f-mono); font-size:18px; font-weight:500; color:var(--v-accent);
    font-variant-numeric:tabular-nums; }
  .s-slider-ends{ display:flex; justify-content:space-between; margin-top:9px;
    font-family:var(--f-mono); font-size:9.5px; letter-spacing:.06em; text-transform:uppercase; color:var(--v-faint); }

  /* bible translations */
  .s-checklist{ display:flex; flex-direction:column; gap:6px; }
  .s-check-code{ font-family:var(--f-mono); font-size:11px; font-weight:600; letter-spacing:.05em; color:var(--v-txt); }
  .s-tr{ display:flex; align-items:center; gap:11px; width:100%; text-align:left; cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line); border-radius:9px; padding:10px 12px;
    color:var(--v-txt); font-family:var(--f-body); font-size:13px; transition:border-color .14s, background .14s; }
  .s-tr:hover{ border-color:var(--v-line2); }
  .s-tr.on{ border-color:var(--v-accent-line); background:var(--v-accent-soft); }
  .s-tr-dot{ width:14px; height:14px; border-radius:50%; flex:0 0 auto; border:2px solid var(--v-faint); }
  .s-tr-dot.on{ border-color:var(--v-accent); background:radial-gradient(circle,var(--v-accent) 40%,transparent 45%); }
  .s-tr-name{ color:var(--v-dim); flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .s-tr-active{ font-size:9px; letter-spacing:.1em; text-transform:uppercase; color:var(--v-accent); }
  .s-tr-note{ font-size:10.5px; color:var(--v-faint); margin-top:12px; line-height:1.6; }
  .s-tr-note b{ color:var(--v-dim); }

  /* network / model */
  .s-status{ display:inline-flex; align-items:center; gap:7px; margin-top:2px;
    font-family:var(--f-mono); font-size:11px; letter-spacing:.04em; }
  .s-status.ok{ color:var(--v-emerald); }
  .s-sdot{ width:7px; height:7px; border-radius:50%; background:currentColor; }
  .s-status.ok .s-sdot{ box-shadow:0 0 7px var(--v-emerald); }
  .s-modelpath{ margin-top:6px; font-family:var(--f-mono); font-size:10px; line-height:1.5;
    color:var(--v-faint); word-break:break-all; }

  /* embedded History */
  .s-history{ margin:-4px -4px 0; }

  /* ── OVERVIEW RAIL ── */
  .s-over{ position:sticky; top:0; display:flex; flex-direction:column; gap:16px; }
  .s-ocard{ background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); padding:16px 18px; }
  .s-ocard.danger{ border-color:var(--v-red-soft); background:linear-gradient(180deg,rgba(239,68,68,.05),var(--v-surf)); }
  .s-ohead{ font-family:var(--f-head); font-size:14px; font-weight:600; color:var(--v-txt); margin-bottom:14px; }
  .s-ohead.danger{ color:var(--v-red); }
  .s-orow{ display:flex; align-items:center; justify-content:space-between; gap:12px; padding:7px 0; }
  .s-ok{ font-size:12.5px; color:var(--v-dim); }
  .s-ov{ font-size:12px; color:var(--v-txt); }

  .s-qlink{ display:flex; align-items:center; gap:12px; width:100%; text-align:left; cursor:pointer;
    padding:11px 4px; border:0; border-top:1px solid var(--v-line); background:transparent; color:var(--v-dim);
    transition:color .13s; }
  .s-qlink:first-of-type{ border-top:0; padding-top:2px; }
  .s-qlink:hover{ color:var(--v-accent2); }
  .s-qtext{ display:flex; flex-direction:column; gap:1px; min-width:0; flex:1; }
  .s-qtext b{ font-size:13px; font-weight:600; color:var(--v-txt); }
  .s-qtext em{ font-style:normal; font-size:11px; color:var(--v-faint); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .s-qarr{ flex:0 0 auto; color:var(--v-faint); }
  .s-qlink:hover .s-qarr{ color:var(--v-accent2); }

  .s-drow{ display:flex; align-items:center; justify-content:space-between; gap:12px; }
  .s-dbtn{ flex:0 0 auto; width:34px; height:34px; display:grid; place-items:center; cursor:pointer;
    border-radius:var(--v-r-md); background:var(--v-red-soft); border:1px solid var(--v-red-soft); color:var(--v-red);
    transition:background .13s; }
  .s-dbtn:hover{ background:rgba(239,68,68,.2); }
  /* Armed: the destructive action is one click from happening — make it read red. */
  .s-dbtn.arm{ background:var(--v-red); border-color:var(--v-red); color:#fff; }
  .s-reset.arm{ border-color:var(--v-red); color:var(--v-red); }

  /* ── responsive ── */
  @media (max-width:1180px){
    .s-layout{ grid-template-columns:200px minmax(0,1fr); }
    .s-over{ grid-column:1 / -1; flex-direction:row; flex-wrap:wrap; position:static; }
    .s-over .s-ocard{ flex:1 1 260px; }
  }
  @media (max-width:820px){
    .s-layout{ grid-template-columns:1fr; }
    .s-rail{ position:static; }
    .s-railnav{ flex-direction:row; flex-wrap:wrap; }
    .s-railbtn{ width:auto; }
    .s-row{ flex-direction:column; align-items:stretch; gap:10px; }
    .s-rowctl{ max-width:none; }
  }
</style>
