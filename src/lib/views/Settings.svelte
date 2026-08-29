<script>
  import { onMount, onDestroy } from 'svelte';
  import ModelSetup from '../ModelSetup.svelte';
  import History from './library/History.svelte';
  import Dashboard from './Dashboard.svelte';
  import { locale, setLocale, LOCALES, t } from '../i18n.js';
  import { restartSetup, setSession } from '../session.js';
  import { humanError } from '../errors.js';
  import { safeMode, setSafeMode } from '../boot/boot.js';
  import { checkForUpdate, updateAvailable } from '../updater.js';
  import {
    listVoiceProfiles,
    createVoiceProfile,
    updateVoiceProfile,
    selectVoiceProfile,
    deleteVoiceProfile,
  } from '../stores/capture.js';
  import {
    listOutputDevices,
    getAudioOutput,
    setAudioOutput,
    supportsSinkId,
    ensureDeviceAccess,
  } from '../audioOutput.js';
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
  import { capture, meter, templates, initAudio, startCapture, stopCapture, setThresholds, setSttLanguage, setInputDevice, listTranslations, getActiveTranslation, setActiveTranslation, localIp, loadTemplates, getContentTemplates, setContentTemplate, getCrashReporting, setCrashReporting, serviceTargetMinutes, loadServiceTarget, setServiceTarget, latencyReport, latencyReset, latencySetEnabled, serviceLock, loadServiceLock, setServiceLock, rooms, loadRooms, saveRoom, useRoom, deleteRoom,
    listOutputChannels, setChannelDisplay, activeVoiceProfile, languageReport } from '../stores/capture.js';
  import { captureRoom, observedNote, applyRoom, describeApply } from '../rooms.js';
  import { snapshotPath, KEEP_SNAPSHOTS } from '../updater.js';
  import { diagnose, drift } from '../latency.js';

  // ─────────────────────────────────────────────────────────────────────────
  // SECTION NAV. The screen is one big config surface split into ref-matched
  // sections; the rail on the left picks which one is shown.
  // ─────────────────────────────────────────────────────────────────────────
  const SECTIONS = [
    { key: 'dashboard', label: 'Dashboard',         desc: 'Service overview and quick actions', icon: 'grid' },
    { key: 'general',   label: 'General',           desc: 'Basic application preferences and behaviour', icon: 'gear' },
    { key: 'outputs',   label: 'Outputs',           desc: 'Per-content-type templates and output routing', icon: 'monitor' },
    { key: 'audio',     label: 'Audio',             desc: 'Microphone input, live level and video sound output', icon: 'mic' },
    { key: 'voice',     label: 'Voice Profiles',    desc: 'Per-preacher language, bias vocabulary and gate calibration', icon: 'user' },
    { key: 'scripture', label: 'Scripture & Bible', desc: 'Recognition language and Bible translations', icon: 'book' },
    { key: 'languages', label: 'Languages',        desc: 'How much of each language Relay actually knows', icon: 'book' },
    { key: 'ai',        label: 'AI & Detection',    desc: 'Detection thresholds and the run engine', icon: 'sparkle' },
    { key: 'shortcuts', label: 'Shortcuts',         desc: 'Keyboard controls for the live desk', icon: 'keyboard' },
    { key: 'network',   label: 'Network',           desc: 'Kiosk, output and stage distribution', icon: 'nodes' },
    { key: 'integrations', label: 'Integrations',   desc: 'OBS, vMix, NDI and SDI switchers', icon: 'nodes' },
    { key: 'history',   label: 'Service History',   desc: 'Past services recorded locally', icon: 'clock' },
    { key: 'backup',    label: 'Backup & Recovery', desc: 'Setup walk-through and safe mode', icon: 'shield' },
    { key: 'updates',   label: 'Updates',           desc: 'App version and update channel', icon: 'refresh' },
    { key: 'diagnostics', label: 'Diagnostics',     desc: 'Live status for a support request', icon: 'terminal' },
    { key: 'advanced',  label: 'Advanced',          desc: 'Crash reporting and privacy', icon: 'terminal' },
    { key: 'account',   label: 'Account',           desc: 'Licence and machine details', icon: 'user' },
  ];
  let section = 'general';
  $: activeSection = SECTIONS.find((s) => s.key === section) ?? SECTIONS[0];

  const ICONS = {
    grid: '<rect x="3" y="3" width="7.5" height="8.5" rx="1.6"/><rect x="13.5" y="3" width="7.5" height="5.5" rx="1.6"/><rect x="3" y="14.5" width="7.5" height="6.5" rx="1.6"/><rect x="13.5" y="11.5" width="7.5" height="9.5" rx="1.6"/>',
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
  // `soon` toggles need OS-level integration that does not ship yet (a Tauri
  // autostart plugin, a system-tray icon). A switch that flips and "sticks" but
  // does nothing is exactly the lying control this app refuses everywhere else,
  // so these are shown disabled with a "Soon" tag instead of pretending to work.
  const GENERAL_TOGGLES = [
    { key: 'autoStart',    title: 'Auto Start on Login',   note: 'Launch Relay automatically when you log in to your computer.', soon: true },
    { key: 'minimizeTray', title: 'Minimize to System Tray', note: 'Minimize the application to the system tray instead of the taskbar.', soon: true },
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

  // Both halves can reject — `startCapture` always could, and `stopCapture` now
  // does rather than swallowing a stop that did not happen. Unhandled here, that
  // is a rejection nobody sees: the button appears to do nothing and the operator
  // is left guessing whether the microphone is live.
  let micErr = '';
  async function toggleCapture() {
    micErr = '';
    try {
      if ($capture.capturing) await stopCapture();
      else await startCapture($capture.inputDevice || null);
    } catch (e) {
      micErr = humanError(e);
    }
  }

  // --- Audio output (speakers for video sound) ---
  // Enumerated from the WEBVIEW, not cpal: routing a <video>'s sound needs
  // setSinkId(deviceId), and cpal's device names are a different namespace that
  // setSinkId can never accept. See lib/audioOutput.js.
  let outDevices = [];
  let outDevice = getAudioOutput();
  let sinkOk = true;
  let outBusy = false;
  // The webview hides the speaker list until a media permission has been granted
  // (measured: no audiooutput entries at all before that). So an empty list is
  // "not unlocked yet", NOT "this machine has no speakers" — the default output
  // always exists and always works, and stays selectable either way.
  $: outLocked = sinkOk && outDevices.length === 0;
  // The lock is armed by Rust when recording starts, so the truth is over there.
  // Read it when this screen opens rather than trusting a store that may have been
  // set before the service began.
  onMount(loadServiceLock);

  // ── ROOMS ─────────────────────────────────────────────────────────────────
  //
  // Save what this space needs; put it back next time. Applying is a LIST of
  // steps, not one call, and the result names every piece that did not take —
  // a room applied on a machine where the projector moved will restore most of
  // itself, and the operator needs to know which part to go and fix.
  let roomName = '';
  let roomMsg = '';
  let roomBusy = false;
  onMount(loadRooms);

  // ── LANGUAGES ─────────────────────────────────────────────────────────────
  //
  // The moat, measured rather than asserted. Every number comes from the data the
  // binary actually ships, so this cannot flatter the product — the only way to
  // improve a figure here is to improve the table the detector uses.
  //
  // Two fields are deliberately ALWAYS empty: word error rate has never been
  // measured in any language, and no native speaker has reviewed any of these
  // tables. They render as "not measured" and "not reviewed", never as a score. A
  // number in either would be the single most misleading thing in this product.
  let langs = [];
  onMount(async () => {
    langs = await languageReport();
  });

  async function doSaveRoom() {
    roomMsg = '';
    const name = roomName.trim();
    if (!name) {
      roomMsg = 'Give the room a name first.';
      return;
    }
    roomBusy = true;
    try {
      const channels = await listOutputChannels();
      const active = await activeVoiceProfile();
      const settings = captureRoom({
        inputDevice: $capture.inputDevice,
        language: $capture.stt?.language ?? null,
        targetMinutes: $serviceTargetMinutes,
        voiceProfileId: active?.id,
        channels,
      });
      await saveRoom(name, settings, observedNote($capture.quality));
      roomMsg = `Saved “${name}”.`;
      roomName = '';
    } catch (e) {
      roomMsg = humanError(e);
    }
    roomBusy = false;
  }

  async function doUseRoom(r) {
    roomMsg = '';
    roomBusy = true;
    try {
      const room = await useRoom(r.id);
      const channels = await listOutputChannels();
      const result = await applyRoom(JSON.parse(room.settings_json || '{}'), {
        setInputDevice,
        setSttLanguage,
        setServiceTarget,
        selectVoiceProfile,
        setChannelDisplay,
        channels,
        humanError,
      });
      roomMsg = describeApply(result, `“${room.name}”`);
    } catch (e) {
      roomMsg = humanError(e);
    }
    roomBusy = false;
  }

  async function doDeleteRoom(r) {
    roomMsg = '';
    try {
      await deleteRoom(r.id);
      roomMsg = `Removed “${r.name}”.`;
    } catch (e) {
      roomMsg = humanError(e);
    }
  }

  // The update readiness readout. Read when this screen opens rather than polled:
  // a database does not become unhealthy while somebody looks at a settings page,
  // and `update_begin` re-checks at the moment it matters anyway.
  let updReady = null;
  onMount(async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      updReady = await invoke('update_preflight');
    } catch {
      // No backend (a plain browser). Showing an empty checklist is better than
      // showing a fabricated healthy one.
      updReady = null;
    }
  });

  let lockErr = '';
  async function unlockService() {
    lockErr = '';
    try {
      await setServiceLock(false);
    } catch (e) {
      // GROUP 1 throws. An unlock that failed must not leave the button claiming
      // it worked while every protected action keeps refusing.
      lockErr = humanError(e);
    }
  }

  onMount(async () => {
    sinkOk = supportsSinkId();
    await refreshOutputs();
    // Devices change when a monitor/USB/Bluetooth speaker comes or goes.
    navigator.mediaDevices?.addEventListener?.('devicechange', refreshOutputs);
  });
  onDestroy(() => {
    navigator.mediaDevices?.removeEventListener?.('devicechange', refreshOutputs);
  });
  async function refreshOutputs() {
    outDevices = await listOutputDevices();
  }
  /** Unlock real speaker names by tripping the media permission once. */
  async function detectSpeakers() {
    outBusy = true;
    try {
      await ensureDeviceAccess();
      await refreshOutputs();
    } finally {
      outBusy = false;
    }
  }
  function pickOutput(id) {
    outDevice = id;
    setAudioOutput(id); // reaches the open output window via localStorage + event
  }

  // ── Voice profiles (SPEC §4.6) ──────────────────────────────────────────────
  //
  // `sensitivity` is the OPERATOR'S DIAL. `auto_fire`/`suggest` are what the
  // router has LEARNED from their confirmations. They are different things, and
  // the form treats them differently on purpose: the dial is editable, the
  // learned pair is shown read-only.
  //
  // Moving the dial is the operator deliberately re-baselining the gate, and the
  // backend re-derives the thresholds from it (`thresholds_on_profile_save`).
  // Every other edit — a rename, a language change — must PRESERVE the learning.
  // Conflating the two once wiped an operator's calibration on every save, so
  // this form never sends a threshold it made up.
  let profiles = [];
  let profileErr = '';
  let profileBusy = false;
  let editing = null; // a working copy; null = nothing open
  let newName = '';

  async function refreshProfiles() {
    profiles = await listVoiceProfiles();
  }
  onMount(refreshProfiles);

  /** Every write goes through here: these throw by contract (they change what the
   *  AI may put on a screen), so the operator is told rather than left guessing. */
  async function profileAction(fn) {
    profileBusy = true;
    profileErr = '';
    try {
      await fn();
      await refreshProfiles();
    } catch (e) {
      profileErr = humanError(e);
    } finally {
      profileBusy = false;
    }
  }

  const addProfile = () => {
    const name = newName.trim();
    if (!name) return;
    return profileAction(async () => {
      await createVoiceProfile(name, null);
      newName = '';
    });
  };
  const useProfile = (id) => profileAction(() => selectVoiceProfile(id));
  const removeProfile = (id) =>
    profileAction(async () => {
      await deleteVoiceProfile(id);
      if (editing?.id === id) editing = null;
    });
  const saveProfile = () =>
    profileAction(async () => {
      await updateVoiceProfile(editing);
      editing = null;
    });
  // Edit a COPY. Binding the row itself would show edits that were never saved —
  // and on this form an unsaved "change" reads as a calibration that is live.
  const openEditor = (p) => (editing = { ...p });

  // RMS on speech sits well below 1.0; scale so normal talking fills the meter.
  $: levelPct = Math.min(100, Math.round($meter.level * 320));

  // Real translations from the corpus + which one to read from.
  let translations = [];
  let activeTranslation = null;
  let dataLoaded = false; // async settings data has resolved at least once
  let lanIp = '';

  // ── LIVE LATENCY ────────────────────────────────────────────────────────────
  //
  // The numbers a field test is graded on, on the machine and in the room where
  // it matters. Polled only while this section is open — a diagnostic that costs
  // a bridge round-trip every two seconds for the whole service is a diagnostic
  // that changes what it measures.
  let lat = null;
  let latTimer = null;
  $: latVerdict = lat ? diagnose(lat) : null;
  $: latDrift = lat ? drift(lat) : null;
  $: latRows = (lat?.metrics ?? []).filter((m) => m.samples > 0);
  async function refreshLatency() {
    lat = await latencyReport(0);
  }
  // Start and stop with the section, not with the component.
  $: if (section === 'diagnostics') startLatencyPoll();
  else stopLatencyPoll();
  function startLatencyPoll() {
    if (latTimer) return;
    refreshLatency();
    latTimer = setInterval(refreshLatency, 2000);
  }
  function stopLatencyPoll() {
    if (!latTimer) return;
    clearInterval(latTimer);
    latTimer = null;
  }
  async function resetLatency() {
    await latencyReset();
    await refreshLatency();
  }
  async function toggleLatency(on) {
    // Reflect what Rust says is in force, never what was asked for.
    const now = await latencySetEnabled(on);
    if (now !== null && lat) lat = { ...lat, enabled: now };
    await refreshLatency();
  }

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

  onMount(loadServiceTarget);
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
    } finally {
      dataLoaded = true; // distinguish "loading" from a genuinely empty list
    }
    try {
      lanIp = await localIp();
    } catch {
      lanIp = '';
    }
  });
  onDestroy(() => {
    if (uptimeTimer) clearInterval(uptimeTimer);
    // A polling timer that outlives its view is exactly the kind of thing a
    // long-service stability test is supposed to catch, so this one does not.
    stopLatencyPoll();
  });

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
      {#if section !== 'dashboard'}
        <div class="s-panelhead">
          <h2 class="s-paneltitle">{activeSection.label}</h2>
          <p class="s-paneldesc">{activeSection.desc}</p>
        </div>
      {/if}

      {#if section === 'dashboard'}
        <!-- Dashboard moved into Settings — a records/overview surface, not a run
             tab. Rendered full-bleed so its own layout is not squeezed by the
             settings panel padding. -->
        <div class="s-dash"><Dashboard /></div>

      {:else if section === 'general'}
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

        <!-- Service length — drives the REMAINING timer on a stage/confidence
             monitor. 0 = no target (the remaining line stays blank). Read by the
             backend when the next service starts. -->
        <div class="s-row">
          <div class="s-rowtext">
            <div class="s-rowtitle">Service Length</div>
            <div class="s-rownote">Planned length in minutes. Shows a “time remaining” timer on stage/confidence monitors. 0 = no target. Applies to the next service you start.</div>
          </div>
          <div class="s-rowctl s-lenctl">
            <input class="r-input s-leninput" type="number" min="0" max="600" step="5"
              value={$serviceTargetMinutes}
              on:change={(e) => setServiceTarget(e.target.value)}
              aria-label="Service length in minutes" />
            <span class="s-lenunit r-mono">min</span>
          </div>
        </div>

        <!-- Toggles -->
        {#each GENERAL_TOGGLES as tg}
          <div class="s-row">
            <div class="s-rowtext">
              <div class="s-rowtitle">{tg.title}{#if tg.soon}<span class="s-soon">Soon</span>{/if}</div>
              <div class="s-rownote">{tg.note}{#if tg.soon} <span class="s-dim">— not available yet.</span>{/if}</div>
            </div>
            <button
              class="s-toggle"
              class:on={prefs[tg.key] && !tg.soon}
              role="switch"
              aria-checked={prefs[tg.key] && !tg.soon}
              aria-label={tg.title}
              disabled={tg.soon}
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
        {#if micErr}
          <p class="s-note s-err" role="alert">{micErr}</p>
        {/if}

        <!-- AUDIO OUTPUT (speakers for video sound). Sits under the mic on the
             same panel: input and output are one operator question. -->
        <div class="s-grouphead">Audio Output</div>
        <div class="s-inline">
          {#if !sinkOk}
            <span class="s-count">system default only</span>
          {:else if outLocked}
            <span class="s-count">system default</span>
          {:else}
            <span class="s-count">{outDevices.length + 1} device{outDevices.length === 0 ? '' : 's'}</span>
          {/if}
        </div>
        <!-- Never disabled: "System default" is always a real, working choice — it is
             where video sound already plays. A greyed-out picker would read as "no
             speakers found", which is never true. -->
        <select class="r-select" value={outDevice} on:change={(e) => pickOutput(e.target.value)}>
          <option value="">System default — computer speakers</option>
          {#each outDevices as d}
            <option value={d.id}>{d.label || 'Speaker'}{d.is_default ? ' — default' : ''}</option>
          {/each}
        </select>

        {#if !sinkOk}
          <p class="s-note">
            This webview can't switch speakers, so video sound plays on whatever macOS
            has selected. Change it in <b>System Settings → Sound → Output</b>.
          </p>
        {:else if outLocked}
          <div class="s-listen">
            <button class="r-btn" on:click={detectSpeakers} disabled={outBusy}>
              {outBusy ? 'Detecting…' : 'Detect speakers'}
            </button>
            <span class="s-rms">names need mic permission once</span>
          </div>
          <p class="s-note">
            Sound plays on your <b>system default</b> speakers right now. macOS hides
            the list of other outputs until this app has been granted the microphone
            once — <b>Detect speakers</b> asks for it, then releases the mic straight
            away (capture still runs through the audio engine, not the browser).
          </p>
        {:else}
          <p class="s-note">
            Where video sound plays on the <b>fullscreen output window</b>. OBS/kiosk
            browser sources are left muted — OBS mixes their audio itself.
          </p>
        {/if}

        <hr class="s-rule" />
        <div class="r-lbl">Rooms</div>
        <p class="s-note">
          Save this space — microphone, recognition language, planned length, voice
          profile and which display each screen goes to — and put it all back with one
          press next time. <b>The audio levels are not saved.</b> Relay learns those
          fresh every time on purpose: a level measured three weeks ago, in a room
          that now has the heating on and forty more people in it, is a guess, and
          guessing is what once made Relay deaf to a quiet preacher.
        </p>
        <div class="s-row s-mt">
          <input class="r-input" placeholder="Main hall" bind:value={roomName} aria-label="Room name" />
          <button class="r-btn ghost sm" on:click={doSaveRoom} disabled={roomBusy}>Save this room</button>
        </div>
        {#each $rooms as r (r.id)}
          <div class="s-row s-roomrow">
            <span class="s-roomname">
              <b>{r.name}</b>
              {#if r.notes}<span class="s-roomnote">{r.notes}</span>{/if}
            </span>
            <button class="r-btn ghost sm" on:click={() => doUseRoom(r)} disabled={roomBusy}>Use</button>
            <button class="r-btn ghost sm" on:click={() => doDeleteRoom(r)} disabled={roomBusy}>Remove</button>
          </div>
        {:else}
          <p class="s-note">No rooms saved yet.</p>
        {/each}
        {#if roomMsg}<p class="s-note" role="status">{roomMsg}</p>{/if}

      {:else if section === 'voice'}
        <p class="s-lead">
          One profile per preacher. Each remembers the language they preach in, the
          names and places Relay should expect to hear, and how cautious the gate
          should be for that voice — so calibration is not relearned from scratch
          every Sunday.
        </p>

        {#if profileErr}
          <p class="s-note s-err" role="alert">{profileErr}</p>
        {/if}

        <div class="s-cardbox">
          {#each profiles as p (p.id)}
            <div class="s-row">
              <div class="s-rowtext">
                <div class="s-rowtitle">
                  {p.name}
                  {#if p.is_active}<span class="s-vpactive r-mono">active</span>{/if}
                </div>
                <div class="s-rownote">
                  {p.language ? p.language.toUpperCase() : 'Auto-detect (code-switching)'}
                  · sensitivity {p.sensitivity}
                  · gate {Math.round(p.auto_fire * 100)}% / {Math.round(p.suggest * 100)}%
                </div>
              </div>
              <div class="s-rowctl s-vpbtns">
                {#if !p.is_active}
                  <button class="r-btn ghost sm" disabled={profileBusy} on:click={() => useProfile(p.id)}>Use</button>
                {/if}
                <button class="r-btn ghost sm" disabled={profileBusy} on:click={() => openEditor(p)}>Edit</button>
                <!-- Deleting the profile in use would leave the gate calibrated by
                     nothing, so the backend refuses it and says why. -->
                <button class="r-btn ghost sm" disabled={profileBusy} on:click={() => removeProfile(p.id)}>Delete</button>
              </div>
            </div>
          {:else}
            <p class="s-note">No profiles yet. The first one you add becomes the active calibration.</p>
          {/each}
        </div>

        <div class="s-grouphead">Add a profile</div>
        <div class="s-vpadd">
          <input
            class="r-input"
            placeholder="Preacher's name"
            bind:value={newName}
            on:keydown={(e) => e.key === 'Enter' && addProfile()} />
          <button class="r-btn" disabled={profileBusy || !newName.trim()} on:click={addProfile}>Add</button>
        </div>

        {#if editing}
          <div class="s-grouphead">Editing “{editing.name}”</div>

          <label class="r-lbl" for="vp-name">Name</label>
          <input id="vp-name" class="r-input" bind:value={editing.name} />

          <label class="r-lbl" for="vp-lang">Language</label>
          <select id="vp-lang" class="r-select" bind:value={editing.language}>
            <option value={null}>Auto-detect (code-switching)</option>
            <option value="en">English</option>
            <option value="yo">Yoruba</option>
            <option value="sw">Swahili</option>
            <option value="ha">Hausa</option>
          </select>

          <label class="r-lbl" for="vp-bias">Expected names and places</label>
          <input
            id="vp-bias"
            class="r-input"
            placeholder="Habakkuk, Ekiti, Oyelaran…"
            bind:value={editing.bias_terms} />
          <p class="s-note">
            Comma-separated. These are fed to the decoder as a hint, which is how an
            unusual name stops being transcribed as something else. It biases
            recognition — it does not force it.
          </p>

          <div class="s-slider">
            <div class="s-slider-top">
              <span class="r-lbl s-slider-name">Sensitivity</span>
              <span class="s-slider-val">{editing.sensitivity}</span>
            </div>
            <input class="r-range" type="range" min="0" max="100" step="1" bind:value={editing.sensitivity} />
            <div class="s-slider-ends"><span>CAUTIOUS</span><span>EAGER</span></div>
          </div>
          <!-- THE ONE THING THIS FORM MUST NOT GET WRONG. The learned pair is shown,
               never edited: it is what the router worked out from this operator's
               confirmations. Moving the dial above is the operator deliberately
               re-baselining, and only then does the backend re-derive these. A
               rename must never cost them their calibration. -->
          <p class="s-note">
            Learned gate for this voice: <b>auto-fire {Math.round(editing.auto_fire * 100)}%</b>,
            <b>suggest {Math.round(editing.suggest * 100)}%</b> — set by Relay from what you
            have confirmed, not by hand. Renaming or changing the language keeps them.
            <b>Moving the sensitivity dial resets them</b>, because that is you saying the
            gate is wrong.
          </p>

          <div class="s-vpadd">
            <button class="r-btn primary" disabled={profileBusy} on:click={saveProfile}>Save profile</button>
            <button class="r-btn ghost" disabled={profileBusy} on:click={() => (editing = null)}>Cancel</button>
          </div>
        {/if}

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
          {:else if !dataLoaded}
            <div class="r-empty" style="font-size:12.5px;">Loading translations…</div>
          {:else}
            <div class="r-empty" style="font-size:12.5px;">No translations loaded.</div>
          {/if}
        </div>
        <div class="s-tr-note r-mono">Only public-domain <b>KJV</b> is bundled. Additional versions need their verse data added to the corpus.</div>

      {:else if section === 'languages'}
        <p class="s-lead">
          What Relay actually knows about each language, counted from the data it
          ships with. Nothing here is a claim — improving a number means improving
          the table the detector uses, which is a one-line change anyone who speaks
          the language can make.
        </p>
        {#if langs.length}
          <table class="s-lang">
            <thead>
              <tr><th>Language</th><th>Books</th><th>Ways to say them</th><th>Numbers in-language</th><th>Console text</th><th>Checked by a speaker</th><th>Accuracy</th></tr>
            </thead>
            <tbody>
              {#each langs as l (l.code)}
                <tr>
                  <td><b>{l.name}</b> <span class="r-mono s-langcode">{l.code}</span></td>
                  <td class="r-mono">{l.books} / {l.books_total}</td>
                  <td class="r-mono">{l.aliases}</td>
                  <!-- Yorùbá numerals are subtractive (16 = ẹrìndínlógún) and are
                       not parsed. Saying "no" is the point of this column. -->
                  <td class="r-mono" class:s-langgap={!l.numerals}>{l.numerals ? 'yes' : 'no'}</td>
                  <td class="r-mono" class:s-langgap={coverage(l.code) === 0}>{coverage(l.code)}%</td>
                  <!-- ABSENCES, not scores. Nothing observes a native speaker's
                       judgement, and none has looked at these tables. -->
                  <td class="r-mono s-langgap">not yet</td>
                  <td class="r-mono s-langgap">not measured</td>
                </tr>
              {/each}
            </tbody>
          </table>
          <p class="s-note">
            <b>“Accuracy” is empty because it has never been measured</b> — in any
            language, including English. Measuring it needs about thirty minutes of
            real preaching on tape and somebody who speaks the language to write down
            what was actually said. Until that exists, any figure here would be a
            guess wearing a percentage sign.
          </p>
          <p class="s-note">
            Every book name came from a published translation, and <b>none has been
            checked by somebody who speaks the language.</b> That is the gap that
            matters most: a wrong alias does not fail safely — it puts the wrong
            scripture on a wall. Fixing one is a one-line change to
            <span class="r-mono">data/book_aliases.json</span>, no code required.
          </p>
        {:else}
          <p class="s-note">The language tables could not be read.</p>
        {/if}

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
        <p class="s-note">Connected devices (OBS · kiosk · stage remote) pull the live output from this machine on the same Wi-Fi. Manage them in the <b>Outputs</b> tab.</p>

        <div class="s-grouphead">Offline speech model</div>
        {#if $capture.stt.loaded}
          <div class="s-status ok s-model"><span class="s-sdot"></span>loaded</div>
          <div class="s-modelpath">{$capture.stt.model}</div>
        {/if}
        <!-- ALWAYS rendered, not only when nothing is loaded. This used to be the
             `{:else}` branch, which was right when there was one model and wrong the
             moment there were several: the operator could install a more accurate
             model and then had no way to see which one was running, let alone choose.
             ModelSetup shows the picker once something is installed and the
             download prompt when nothing is. -->
        <ModelSetup />

      {:else if section === 'integrations'}
        <p class="s-lead">Relay sends its output to other software over your local network — no plugins to install. Add a <b>Browser Source</b> pointing at Relay; the exact per-channel URL is in <b>Outputs → Sharing</b>.</p>
        <div class="s-cardbox">
          <!-- The URL is CHANNEL-keyed (DECISIONS §29). Changing a screen's template
               broadcasts a channel_template message the output applies by matching its
               OWN `channel` — so a template swap is live with no re-copying of the URL.
               This row used to show a `?template_id=<n>`-only shape, which parses to
               channel 0 ("no channel"): it renders, so it looks right, and then it is
               the one browser source in the building that never follows a template
               change. Copy URL in Outputs → Sharing is still the only thing that fills
               in the real ids. -->
          <div class="s-netrow"><span class="s-netk">OBS / vMix (browser source)</span><span class="s-netv r-mono">http://{lanIp || 'this-pc'}:8032/output.html?channel=&lt;screen&gt;&amp;template_id=&lt;n&gt;</span></div>
          <div class="s-netrow"><span class="s-netk">Kiosk screen / stage tablet</span><span class="s-netv r-mono">:8032 · http</span></div>
          <div class="s-netrow"><span class="s-netk">NDI</span><span class="s-netv r-mono">not available</span></div>
          <div class="s-netrow"><span class="s-netk">ATEM / SDI switcher</span><span class="s-netv r-mono">via HDMI</span></div>
        </div>
        <p class="s-note"><b>NDI is parked</b> — it needs a proprietary SDK Relay does not bundle, so there is no NDI source to select. For an <b>ATEM or other SDI switcher</b>, open a Relay output window on an HDMI screen and feed that HDMI into the switcher — Relay does not speak SDI directly (and won't; that is served by the hardware you already own).</p>

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

        <hr class="s-rule" />
        <!-- SERVICE LOCK. Reachable from the sentence the refusal itself prints,
             which is the whole reason it lives here and not somewhere tidier. -->
        {#if $serviceLock.engaged}
          <p class="s-note">
            <b style="color:var(--v-amber);">A service is being recorded.</b>
            Relay is holding back a few things that cannot be undone, or that would take
            the speech engine away mid-sermon: {$serviceLock.held_back.join(', ')}.
            Firing, the transport, clearing and blacking out are unaffected.
          </p>
          <button class="r-btn ghost sm s-mt" on:click={unlockService}>Unlock for this service</button>
          {#if lockErr}<p class="s-note" role="alert" style="color:var(--v-rose)">{lockErr}</p>{/if}
        {:else}
          <p class="s-note">
            While a service is being recorded, Relay holds back deletions, speech-model
            changes and imports — an accident at 10:31 has no undo. It arms itself when you
            start listening and lifts when the service ends. Nothing on the live path is
            ever held back.
          </p>
        {/if}

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

        <!-- WHAT AN UPDATE WOULD DO TO YOUR HISTORY.
             Shown before the operator presses anything, because the question they
             actually have — "is this safe right now?" — was previously answerable
             only by trying it. Nothing here refuses on its own; `update_begin`
             re-runs the same checks at the moment of truth. -->
        <hr class="s-rule" />
        <div class="r-lbl">Before an update</div>
        <p class="s-note">
          Relay copies your entire history — services, plans, songs, saved verses and
          templates — before it installs anything, and keeps the last
          {KEEP_SNAPSHOTS} copies. The app itself can always be reinstalled from a
          release page; your history cannot.
        </p>
        {#if updReady}
          {#if updReady.during_service}
            <p class="s-note"><b>A service is being recorded.</b> Relay will not update until it ends — an update restarts the app.</p>
          {/if}
          <div class="s-cardbox s-mt">
            {#each updReady.checks as c (c.id)}
              <div class="s-netrow">
                <span class="s-netk">{c.label}</span>
                <span class="s-netv r-mono" class:bad={c.state === 'fail'} class:warn={c.state === 'warn'}>
                  {c.note}
                </span>
              </div>
            {/each}
          </div>
        {/if}
        {#if $snapshotPath}
          <p class="s-note s-mt">Your history was copied to <span class="r-mono">{$snapshotPath}</span>.</p>
        {/if}

      {:else if section === 'diagnostics'}
        <p class="s-lead">The facts a support request needs, in one place. Nothing here leaves this machine.</p>
        <div class="s-cardbox">
          <div class="s-netrow"><span class="s-netk">Backend</span><span class="s-netv r-mono">{$capture.available ? 'connected' : 'not connected'}</span></div>
          <div class="s-netrow"><span class="s-netk">Speech model</span><span class="s-netv r-mono">{$capture.stt.loaded ? ($capture.stt.model || 'loaded') : 'not loaded'}</span></div>
          <div class="s-netrow"><span class="s-netk">Recognition language</span><span class="s-netv r-mono">{$capture.stt.language || '—'}</span></div>
          <div class="s-netrow"><span class="s-netk">Microphone</span><span class="s-netv r-mono">{$capture.inputDevice || 'system default'}</span></div>
          <div class="s-netrow"><span class="s-netk">Detection</span><span class="s-netv r-mono">{$capture.detectionOn ? 'armed' : 'off'}</span></div>
          <div class="s-netrow"><span class="s-netk">This machine (LAN)</span><span class="s-netv r-mono">{lanIp || '—'}</span></div>
          <div class="s-netrow"><span class="s-netk">Ports</span><span class="s-netv r-mono">5032 console · 8031 ws · 8032 http</span></div>
          <div class="s-netrow"><span class="s-netk">Version</span><span class="s-netv r-mono">{appVersion || '—'} · {environment}</span></div>
          <div class="s-netrow"><span class="s-netk">Uptime (this run)</span><span class="s-netv r-mono">{uptime}</span></div>
        </div>

        <div class="s-grouphead">Live latency</div>
        <p class="s-tr-note">
          How long it takes a spoken word to reach the operator's screen, and a spoken reference to reach the wall — measured on <b>this</b> machine, in <b>this</b> room, on the model you are actually running. Milliseconds. Nothing here leaves the computer.
          <br /><br />
          The clock starts when audio reaches the speech engine. Assembling it from the microphone adds a further {lat?.capture_front_end_ms ?? 400}ms at most (about half that on average), and the end-to-end row already includes it.
        </p>
        {#if latVerdict}
          <div class="s-netrow"><span class="s-netk">Verdict</span><span class="s-netv">{latVerdict.verdict}</span></div>
          <p class="s-note">{latVerdict.detail}</p>
        {/if}
        {#if latRows.length}
          <div class="s-cardbox">
            <div class="s-netrow"><span class="s-netk">measurement</span><span class="s-netv r-mono">n · median · P95 · worst</span></div>
            {#each latRows as m}
              <div class="s-netrow">
                <span class="s-netk">{m.metric.replace(/_/g, ' ')}</span>
                <span class="s-netv r-mono">{m.samples} · {Math.round(m.p50_ms ?? 0)}ms · {Math.round(m.p95_ms ?? 0)}ms · {Math.round(m.worst_ms ?? 0)}ms</span>
              </div>
            {/each}
            <div class="s-netrow"><span class="s-netk">transcript updates / second</span><span class="s-netv r-mono">{(lat?.transcript_updates_per_s ?? 0).toFixed(2)}</span></div>
            <div class="s-netrow"><span class="s-netk">partials dropped (queue full)</span><span class="s-netv r-mono">{lat?.dropped_partials ?? 0}</span></div>
          </div>
        {:else}
          <p class="s-note">Nothing measured yet. Start listening and speak for a few seconds.</p>
        {/if}
        {#if latDrift}
          <p class="s-note">
            {#if latDrift.growing}
              <b>Latency is growing.</b> It averaged {Math.round(latDrift.early)}ms early in this session and {Math.round(latDrift.late)}ms recently — the pipeline is falling further behind the longer it runs.
            {:else}
              Steady: {Math.round(latDrift.early)}ms early in this session, {Math.round(latDrift.late)}ms recently.
            {/if}
          </p>
        {/if}
        <div style="display:flex; gap:10px; flex-wrap:wrap; margin-top:10px;">
          <button class="r-btn" on:click={resetLatency} disabled={!$capture.available}>Start a fresh measurement</button>
          <button
            class="r-btn"
            on:click={() => toggleLatency(!(lat?.enabled ?? true))}
            disabled={!$capture.available}
          >{(lat?.enabled ?? true) ? 'Stop measuring' : 'Start measuring'}</button>
        </div>
        <p class="s-note">Measuring is on by default and costs a handful of timestamps per decode. Turning it off is here so a field test can prove the instrument is not the delay.</p>

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
        <div class="s-grouphead">Operators</div>
        <p class="s-note" style="margin-top:0">Relay is a <b>single-operator, on-device</b> app — there are no user accounts, roles or logins, by design. The one control that matters mid-service (operator override) is always reachable, and the preacher's stage remote is a separate, LAN-only surface (set up in <b>Outputs → Sharing</b>). Nothing about who is at the desk is recorded.</p>
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
          <span class="s-qtext"><b>Keyboard Shortcuts</b><em>View the full shortcut reference</em></span>
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
  /* Dashboard embed — let it fill and scroll within the settings panel. */
  .s-dash{ flex:1; min-height:0; overflow:auto; }
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
  .s-lenctl{ display:flex; align-items:center; gap:8px; justify-content:flex-end; }
  .s-leninput{ width:90px; text-align:right; }
  .s-lenunit{ color:var(--v-faint); font-size:var(--v-fs-cap); }

  .s-grouphead{ margin:14px 0 2px; font-family:var(--f-mono); font-size:11px; font-weight:600;
    letter-spacing:.14em; text-transform:uppercase; color:var(--v-faint); }
  .s-grouphead.first{ margin-top:0; }

  /* Voice profiles. `s-vpactive` marks the profile the gate is calibrated by —
     EMERALD, never amber: amber is spent only on air (CLAUDE.md / DECISIONS §22),
     and a selected profile is configuration, not something on a screen. */
  .s-vpactive{ margin-left:8px; padding:1px 6px; border-radius:var(--v-r-sm);
    font-size:10px; letter-spacing:.08em; text-transform:uppercase;
    color:var(--v-emerald); border:1px solid color-mix(in srgb, var(--v-emerald) 40%, transparent); }
  .s-vpbtns{ display:flex; gap:6px; justify-content:flex-end; }
  .s-vpadd{ display:flex; gap:8px; align-items:center; margin-top:8px; }
  .s-vpadd .r-input{ flex:1 1 auto; min-width:0; }
  .s-err{ color:var(--v-red); }

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
  .s-toggle:disabled{ opacity:.4; cursor:not-allowed; }

  /* "Soon" — a control shown for shape but not yet wired, marked so it can't lie. */
  .s-soon{ display:inline-block; margin-left:8px; padding:1px 7px; border-radius:99px;
    background:var(--v-surf3); border:1px solid var(--v-line2); color:var(--v-faint);
    font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.04em; vertical-align:middle; }
  .s-dim{ color:var(--v-faint); }

  /* Boxed rows (outputs, network, updates, account, shortcuts) */
  .s-cardbox{ display:flex; flex-direction:column; gap:8px; }
  .s-netrow{ display:flex; align-items:center; justify-content:space-between; gap:12px;
    padding:12px 14px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line); }
  .s-netk{ font-size:13px; color:var(--v-dim); }
  .s-netv{ font-size:11px; color:var(--v-txt); }

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
  .s-roomrow{ align-items:flex-start; }
  .s-lang{ width:100%; border-collapse:collapse; font-size:var(--v-fs-b2); margin-top:10px; }
  .s-lang th{ text-align:left; font-weight:500; font-size:9px; letter-spacing:.06em;
    text-transform:uppercase; color:var(--v-faint); padding:6px 8px;
    border-bottom:1px solid var(--v-line); }
  .s-lang td{ padding:7px 8px; border-bottom:1px solid var(--v-line2); color:var(--v-dim); }
  .s-langcode{ color:var(--v-faint); font-size:10px; }
  /* An absence is dim, not red: nobody has failed here — the work has not been
     done, and saying so is the whole point of the column. */
  .s-langgap{ color:var(--v-faint); font-style:italic; }
  .s-roomname{ flex:1; min-width:0; display:flex; flex-direction:column; gap:2px; }
  .s-roomnote{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  @media (min-width:1px){
  }
</style>
