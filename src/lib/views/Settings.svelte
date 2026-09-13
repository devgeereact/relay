<script>
  import { onMount, onDestroy } from 'svelte';
  import { rangeFill } from '../rangefill.js';
  import { get } from 'svelte/store';
  import ModelSetup from '../ModelSetup.svelte';
  // The shared workspace grammar (docs/REBRAND.md §2 · §11) — the same columns,
  // panes, type roles and name/value row the Planner and Outputs desks use.
  import WorkspaceFrame from './WorkspaceFrame.svelte';
  import History from './library/History.svelte';
  import Dashboard from './Dashboard.svelte';
  import { locale, setLocale, LOCALES, t } from '../i18n.js';
  import { restartSetup, setSession } from '../session.js';
  import { humanError } from '../errors.js';
  import { settingValue, CHECKING } from '../settingvalue.js';
  import { safeMode, setSafeMode } from '../boot/boot.js';
  import { checkForUpdate, updateAvailable, updateChannel, describeChannel } from '../updater.js';
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
    listOutputChannels, setChannelDisplay, activeVoiceProfile, languageReport, exportDiagnostics, readErrors } from '../stores/capture.js';
  import Loading from '../ui/Loading.svelte';
  import ErrorState from '../ui/ErrorState.svelte';
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
    { key: 'privacy',   label: 'Privacy',           desc: 'What is on this machine, and what can leave it', icon: 'shield' },
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
  // The Privacy screen reads the LIVE value, never a literal. A page that says
  // "off" because somebody typed "off" is worth less than no page at all — it is
  // the one row a person opens it to check.
  $: crashOn = !!crash.enabled;
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

  // The diagnostic bundle. Says where the file went, because "saved" with no path
  // sends an operator hunting through a Downloads folder.
  let diagBusy = false;
  let diagMsg = '';
  async function doExportDiagnostics() {
    diagBusy = true;
    diagMsg = '';
    try {
      const path = await exportDiagnostics();
      diagMsg = `Saved to ${path}. It contains no transcript, verse text, lyric or service name — you can read it before you send it.`;
    } catch (e) {
      diagMsg = humanError(e);
    }
    diagBusy = false;
  }

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
  // The third fact the array cannot carry (RG-95): asked-and-empty, still-asking,
  // and asked-and-failed all render `[]`. Without this, the pane claimed "the
  // language tables could not be read" in the frames before the read returned —
  // an error message for a state that is not an error.
  let langsAsked = false;
  onMount(async () => {
    langs = await languageReport();
    langsAsked = true;
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
  /**
   * Milliseconds, or an em dash. **A stage never reached is an absence, not a zero.**
   *
   * The same helper Rust's diagnostic bundle uses, for the same reason: printing
   * `0ms` for a stage that never ran makes it the fastest number on the screen.
   */
  const msOrDash = (v) => (v === null || v === undefined ? '—' : `${Math.round(v)}ms`);

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
  // WHICH KIND OF NOTHING. An empty `lanIp` used to render as an em dash, and an
  // em dash is the same glyph for "not fetched yet", "the fetch failed" and "this
  // machine is not on a network" — three things an operator needs to tell apart
  // (rule 35, and RG-83 in another costume).
  let lanState = 'loading';

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
  let versionState = 'loading';
  const environment = import.meta.env?.DEV ? 'Development' : 'Production';
  let bootAt = 0;
  // Never a dash, not even for the instant before the first tick: a row that
  // says nothing is a row an operator has to guess about.
  let uptime = CHECKING;
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
      // The check can complete without having reached anything. Saying "you're on
      // the latest version" after a failed check is the same lie the status row
      // used to tell, moved into a button.
      const ch = get(updateChannel);
      updateMsg = v
        ? `Relay ${v} is available.`
        : ch.state === 'failed'
          ? "Relay could not reach the update server, so it does not know whether there is a newer version. This is normal offline — if you are online, the update channel may be broken."
          : ch.state === 'unavailable'
            ? 'This build has no update channel.'
            : "You're on the latest version.";
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
      versionState = 'ok';
    } catch {
      appVersion = '';
      versionState = 'failed';
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
      lanState = 'ok';
    } catch {
      lanIp = '';
      lanState = 'failed';
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

<!-- Settings is laid out in the shared workspace grammar (`WorkspaceFrame.svelte`,
     docs/REBRAND.md §2 · §11) — the same columns, panes and type roles the
     Planner and Outputs desks use.
     §11 asks for ONE type scale with three roles: page title, standfirst, row.
     The page title is "Settings" and never changes; the STANDFIRST is the
     section's own sentence, so the two roles say different things instead of the
     title being repeated a size smaller directly beneath itself, which is what
     the page head and the panel head were doing to each other.
     The sixteen sections are deliberately NOT merged into §11's eleven: that
     reorganisation moves every control an operator has learned where to find, and
     it is worth doing with somebody watching the screens rather than at the tail
     of a styling pass. -->
<WorkspaceFrame
  title="Settings"
  standfirst={activeSection.desc}
  columns="212px minmax(0,1fr) 288px">
    <!-- ════ SECTION RAIL ════ -->
    <aside class="rw-pane">
      <div class="rw-panehead">
        <h2 class="rw-panettl">Sections</h2>
        <span class="rw-spring"></span>
        <span class="rw-itemn">{SECTIONS.length}</span>
      </div>
      <nav class="rw-panebody s-railnav">
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
      <div class="rw-panefoot">
        <button class="r-btn ghost sm s-reset" class:arm={resetArmed} on:click={resetAllSettings}>
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
          {resetArmed ? 'Click again to reset' : 'Reset to Defaults'}
        </button>
      </div>
    </aside>

    <!-- ════ ACTIVE PANEL ════ -->
    <main class="rw-pane">
      <div class="rw-panehead"><h2 class="rw-panettl">{activeSection.label}</h2></div>
      <div class="rw-panebody s-panel">

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
            <input class="r-range" type="range" min="0" max="100" step="1" bind:value={editing.sensitivity} use:rangeFill={editing.sensitivity} />
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
            <div class="r-empty" style="font-size:var(--v-fs-b1);">Loading translations…</div>
          {:else}
            <div class="r-empty" style="font-size:var(--v-fs-b1);">No translations loaded.</div>
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
        {:else if !langsAsked}
          <Loading what="the language tables" />
        {:else if $readErrors.languageReport}
          <ErrorState error={$readErrors.languageReport} />
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
            on:input={(e) => onAuto(+e.target.value)} disabled={!$capture.available} use:rangeFill={$capture.thresholds.auto_fire} />
          <div class="s-slider-ends"><span>LAX (50%)</span><span>STRICT (100%)</span></div>
        </div>
        <div class="s-slider">
          <div class="s-slider-top">
            <span class="r-lbl s-slider-name">Suggest above</span>
            <span class="s-slider-val">{Math.round($capture.thresholds.suggest * 100)}%</span>
          </div>
          <input class="r-range" type="range" min="0.3" max="0.9" step="0.01"
            value={$capture.thresholds.suggest}
            on:input={(e) => onSuggest(+e.target.value)} disabled={!$capture.available} use:rangeFill={$capture.thresholds.suggest} />
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
          <div class="s-netrow"><span class="s-netk">This machine</span><span class="s-netv r-mono">{settingValue(lanIp, {
              loading: lanState === 'loading',
              missing: lanState === 'failed' ? 'could not be read' : 'not on a network',
            })}</span></div>
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
          <div class="s-netrow"><span class="s-netk">Installed version</span><span class="s-netv r-mono">{settingValue(appVersion, {
              loading: versionState === 'loading',
              missing: 'could not be read',
            })}</span></div>
          <div class="s-netrow"><span class="s-netk">Environment</span><span class="s-netv r-mono">{environment}</span></div>
          <!-- The status of the CHANNEL, not the absence of news. This row used to
               read "up to date" whenever nothing was waiting — which was also what
               it said when the check had never run, when the laptop was offline,
               and when the update manifest had been returning 404 since the day
               Relay was installed. A badge that cannot detect its own failure
               (CLAUDE.md rule 35), on the one path by which a fix reaches a church
               that already has Relay. -->
          <div class="s-netrow"><span class="s-netk">Update status</span><span class="s-netv r-mono" class:s-netbad={$updateChannel.state === 'failed'}>{describeChannel($updateChannel)}</span></div>
          {#if $updateChannel.state === 'failed'}
            <div class="s-netrow"><span class="s-netk">Last attempt</span><span class="s-netv r-mono">{$updateChannel.detail || 'no reason given'}</span></div>
          {/if}
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
        <p class="s-lead">The facts a support request needs, in one place. Nothing here leaves this machine unless you send it.</p>
        <!-- A FILE, NOT A SCREEN. This table has shown the right facts for a while
             and been useless for the job it exists for: nobody can email a screen.
             What actually happens is somebody photographs it, losing half the table
             and all of the latency history. -->
        <button class="r-btn ghost sm" on:click={doExportDiagnostics} disabled={diagBusy}>
          {diagBusy ? 'Writing…' : 'Save a diagnostic file'}
        </button>
        {#if diagMsg}<p class="s-note" role="status">{diagMsg}</p>{/if}
        <div class="s-cardbox">
          <div class="s-netrow"><span class="s-netk">Backend</span><span class="s-netv r-mono">{$capture.available ? 'connected' : 'not connected'}</span></div>
          <div class="s-netrow"><span class="s-netk">Speech model</span><span class="s-netv r-mono">{$capture.stt.loaded ? ($capture.stt.model || 'loaded') : 'not loaded'}</span></div>
          <div class="s-netrow"><span class="s-netk">Recognition language</span><span class="s-netv r-mono">{settingValue($capture.stt.language, { missing: 'not set yet' })}</span></div>
          <div class="s-netrow"><span class="s-netk">Microphone</span><span class="s-netv r-mono">{$capture.inputDevice || 'system default'}</span></div>
          <div class="s-netrow"><span class="s-netk">Detection</span><span class="s-netv r-mono">{$capture.detectionOn ? 'armed' : 'off'}</span></div>
          <div class="s-netrow"><span class="s-netk">This machine (LAN)</span><span class="s-netv r-mono">{settingValue(lanIp, {
              loading: lanState === 'loading',
              missing: lanState === 'failed' ? 'could not be read' : 'not on a network',
            })}</span></div>
          <div class="s-netrow"><span class="s-netk">Ports</span><span class="s-netv r-mono">5032 console · 8031 ws · 8032 http</span></div>
          <div class="s-netrow"><span class="s-netk">Version</span><span class="s-netv r-mono">{settingValue(appVersion, {
              loading: versionState === 'loading',
              missing: 'could not be read',
            })} · {environment}</span></div>
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
            <div class="s-netrow"><span class="s-netk">measurement</span><span class="s-netv r-mono">n · median · P95 · P99 · worst</span></div>
            {#each latRows as m}
              <div class="s-netrow">
                <span class="s-netk">{m.metric.replace(/_/g, ' ')}</span>
                <!-- `?? 0` used to be here, and it rendered a stage that was never
                     reached as `0ms` — the fastest thing on the screen. That is the
                     absence-is-not-a-zero rule (DECISIONS §38, §44) failing at the
                     last hop, on the one screen a field tester reads. -->
                <span class="s-netv r-mono">{m.samples} · {msOrDash(m.p50_ms)} · {msOrDash(m.p95_ms)} · {msOrDash(m.p99_ms)} · {msOrDash(m.worst_ms)}</span>
              </div>
            {/each}
            <div class="s-netrow"><span class="s-netk">transcript updates / second</span><span class="s-netv r-mono">{(lat?.transcript_updates_per_s ?? 0).toFixed(2)}</span></div>
            <div class="s-netrow"><span class="s-netk">partials dropped (queue full)</span><span class="s-netv r-mono">{lat?.dropped_partials ?? 0}</span></div>
            <!-- RG-84. A shed PARTIAL is re-decoded a moment later; shed AUDIO is a
                 piece of the sermon Relay never heard. Both queues in front of the
                 decoder were unbounded — a stall became memory and a transcript
                 minutes behind, rather than a number. Non-zero here is worse news
                 than the row above it, so it is coloured and the row above is not. -->
            <div class="s-netrow"><span class="s-netk">audio dropped (never heard)</span><span class="s-netv r-mono" class:s-netbad={(lat?.dropped_audio ?? 0) > 0}>{lat?.dropped_audio ?? 0}</span></div>
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

      {:else if section === 'privacy'}
        <!-- WHAT IS LEAVING THIS MACHINE, ANSWERED FROM THE LIVE SETTINGS.
             Relay's privacy story is the strongest thing about it and it has been
             invisible — it lives in PRIVACY.md, which nobody in a booth reads. Every
             row below is read from the actual state, never hardcoded: a screen that
             says "off" because somebody typed "off" is worth less than no screen.
             It also states the LAN exposure plainly, because a privacy page that
             lists only the reassuring half is an advert. -->
        <p class="s-lead">
          Read from this machine right now — not from a promise. Nothing here is a
          setting you change; it is a report on the settings you have.
        </p>
        <div class="s-cardbox">
          <div class="s-netrow">
            <span class="s-netk">What you say</span>
            <span class="s-netv">Never leaves this computer. Audio is processed in memory and is not written to disk.</span>
          </div>
          <div class="s-netrow">
            <span class="s-netk">Transcripts &amp; history</span>
            <span class="s-netv">Stored on this computer only, in Relay's own database.</span>
          </div>
          <div class="s-netrow">
            <span class="s-netk">Speech recognition</span>
            <span class="s-netv">
              {$capture.stt.loaded
                ? 'Runs entirely on this machine. No audio is sent anywhere.'
                : 'No model loaded — nothing is being transcribed.'}
            </span>
          </div>
          <div class="s-netrow">
            <span class="s-netk">Crash reporting</span>
            <!-- The live value. This is the ONE thing Relay can send, and the one
                 row somebody opens this page to check. -->
            <span class="s-netv" class:on={crashOn}>
              {crashOn
                ? 'ON — a crash sends the error and where it happened. Never a transcript, verse, lyric or announcement.'
                : 'OFF — nothing is sent when Relay crashes.'}
            </span>
          </div>
          <div class="s-netrow">
            <span class="s-netk">Accounts &amp; cloud</span>
            <span class="s-netv">There are none. Relay has no account, no server, and works with the network unplugged.</span>
          </div>
          <div class="s-netrow">
            <span class="s-netk">Your church network</span>
            <!-- The unflattering half, in the same size type. -->
            <span class="s-netv">
              Relay serves your screens at <span class="r-mono">{lanIp || 'this computer'}:8032</span>.
              Anyone already on the same WiFi can see what is on the projector — <b>and can
              change it</b>: the preacher's remote has no password, by design. They cannot
              reach your transcripts, plans or history.
            </span>
          </div>
          <div class="s-netrow">
            <span class="s-netk">Diagnostic file</span>
            <span class="s-netv">Only written when you press the button in Diagnostics, and only where you can read it first.</span>
          </div>
        </div>
        <p class="s-note">
          The full account, including what would make the network tradeoff change, is
          in <span class="r-mono">PRIVACY.md</span> and <span class="r-mono">docs/DECISIONS.md</span> §35.
        </p>

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
          <div class="s-netrow"><span class="s-netk">Version</span><span class="s-netv r-mono">{settingValue(appVersion, {
              loading: versionState === 'loading',
              missing: 'could not be read',
            })}</span></div>
        </div>
        <p class="s-note">Relay is free and open source. There is no account to sign in to and nothing to pay — every feature works offline, on this machine.</p>
        <div class="s-grouphead">Operators</div>
        <p class="s-note" style="margin-top:0">Relay is a <b>single-operator, on-device</b> app — there are no user accounts, roles or logins, by design. The one control that matters mid-service (operator override) is always reachable, and the preacher's stage remote is a separate, LAN-only surface (set up in <b>Outputs → Sharing</b>). Nothing about who is at the desk is recorded.</p>
      {/if}
      </div>
    </main>

    <!-- ════ OVERVIEW RAIL ════ the facts that are true whatever section is open,
         so they belong in the inspector column rather than being repeated inside
         each section that happens to care about one of them. -->
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead"><h2 class="rw-panettl">Overview</h2></div>
      <div class="rw-panebody s-overbody">
      <div class="s-ocard">
        <div class="s-ohead">System Overview</div>
        <div class="s-orow"><span class="s-ok">Version</span><span class="s-ov r-mono">{settingValue(appVersion, {
              loading: versionState === 'loading',
              missing: 'could not be read',
            })}</span></div>
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
      </div>
    </aside>
</WorkspaceFrame>

<style>
  /* SETTINGS — the workspace grammar (`WorkspaceFrame.svelte`, docs/REBRAND.md
     §2 · §11): section rail · panel · overview rail, as three panes.

     ── ONE TYPE SCALE, THREE ROLES ──────────────────────────────────────────
     §11's three roles are page title / standfirst / row, with a footnote behind
     a hairline. The first two live in the frame. This file owns the third, and
     the reason it is worth writing down is that it used to be eleven: 13.5px,
     14px, 13px, 12.5px, 12px, 11px, 10.5px, 10px, 9px, 18px and 30px, none of
     them from the token scale, all of them chosen one control at a time. A scale
     with eleven steps is not a scale. Every size here is now a `--v-fs-*` token.

     ── SEAMS, NOT GUTTERS ───────────────────────────────────────────────────
     A settings section was a column of bordered cards with 12px trenches between
     them. On a booth laptop that trench is about a third of the screen spent
     saying "these two settings are not related", which is false — they are the
     same section. Rows now bleed to the pane's edges with a hairline between
     them, which is how the running order and the screens list read, and gives
     the width back to the words. The bleed is `margin:0 -14px` against the
     panel's own 14px gutter: prose keeps the gutter, rows reach the seam. */

  /* ── SECTION RAIL ── */
  .s-railnav{ display:flex; flex-direction:column; }
  .s-railbtn{ display:flex; align-items:center; gap:10px; width:100%; text-align:left; cursor:pointer;
    min-height:32px; padding:6px 12px; border:0; border-bottom:1px solid var(--v-line); background:transparent;
    color:var(--v-dim); font-family:var(--f-body); font-size:var(--v-fs-b2); line-height:var(--v-lh-b2);
    font-weight:500; transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .s-railbtn:last-child{ border-bottom:0; }
  .s-railbtn:hover:not(.on){ background:var(--v-surf2); color:var(--v-txt); }
  /* Steel blue = the thing you are working on (docs/REBRAND.md §1). */
  .s-railbtn.on{ background:var(--v-sel-soft); color:var(--v-txt); font-weight:600;
    box-shadow:inset 2px 0 0 var(--v-sel); }
  .s-railic{ flex:0 0 auto; }
  .s-raillbl{ overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .s-reset{ justify-content:center; width:100%; }

  /* ── ACTIVE PANEL ── */
  .s-panel{ min-width:0; display:flex; flex-direction:column; gap:0; padding:14px; }
  /* Dashboard and History bring their own layout; let them fill the pane rather
     than sitting inside the panel's gutter at a second, smaller width. */
  .s-dash{ flex:1; min-height:0; overflow:auto; margin:-14px -14px 0; }
  .s-history{ margin:-14px -14px 0; }

  .s-lead{ margin:0 0 10px; font-size:var(--v-fs-b2); line-height:1.55; color:var(--v-dim);
    max-width:74ch; }
  .s-inline{ display:flex; justify-content:flex-end; padding:8px 0; }

  /* ── ROLE 3 · a setting row is a name and a value ── */
  .s-row{ display:flex; align-items:center; justify-content:space-between; gap:20px;
    margin:0 -14px; padding:10px 14px; border:0; border-bottom:1px solid var(--v-line);
    background:transparent; }
  .s-row:last-child{ border-bottom:0; }
  .s-rowtext{ min-width:0; }
  .s-rowtitle{ font-size:var(--v-fs-b2); line-height:var(--v-lh-b2); font-weight:600; color:var(--v-txt); }
  .s-rownote{ margin-top:2px; font-size:var(--v-fs-cap); line-height:var(--v-lh-cap); color:var(--v-faint);
    max-width:74ch; }
  .s-rowctl{ flex:0 0 auto; min-width:170px; max-width:220px; }
  .s-lenctl{ display:flex; align-items:center; gap:8px; justify-content:flex-end; }
  .s-leninput{ width:90px; text-align:right; }
  .s-lenunit{ color:var(--v-faint); font-size:var(--v-fs-cap); }

  /* A group heading inside a section. Furniture, so it sits on the darker ground
     with a seam under it — the same shape as the frame's `.rw-group`. */
  .s-grouphead{ margin:0 -14px; padding:8px 14px 6px; background:var(--v-bg);
    border-bottom:1px solid var(--v-line);
    font-family:var(--f-mono); font-size:var(--v-fs-cap); line-height:var(--v-lh-cap); font-weight:600;
    letter-spacing:var(--v-tr-caps); text-transform:uppercase; color:var(--v-faint); }
  .s-grouphead.first{ margin-top:0; }

  /* Voice profiles. `s-vpactive` marks the profile the gate is calibrated by —
     EMERALD, never amber: amber is spent only on air (CLAUDE.md / DECISIONS §22),
     and a selected profile is configuration, not something on a screen. */
  .s-vpactive{ margin-left:8px; padding:1px 6px; border-radius:var(--v-r-sm);
    font-size:var(--v-fs-cap); letter-spacing:.08em; text-transform:uppercase;
    color:var(--v-emerald); border:1px solid color-mix(in srgb, var(--v-emerald) 40%, transparent); }
  .s-vpbtns{ display:flex; gap:6px; justify-content:flex-end; }
  .s-vpadd{ display:flex; gap:8px; align-items:center; margin-top:8px; }
  .s-vpadd .r-input{ flex:1 1 auto; min-width:0; }
  .s-err{ color:var(--v-red); }

  /* Segmented control (theme, time format) */
  .s-seg{ display:inline-flex; gap:2px; padding:3px; border-radius:var(--v-r-md);
    background:var(--v-void); border:1px solid var(--v-line); flex:0 0 auto; }
  .s-segbtn{ display:inline-flex; align-items:center; gap:6px; height:22px; padding:0 11px; border:0;
    cursor:pointer; border-radius:2px; background:transparent; color:var(--v-faint);
    font-family:var(--f-body); font-size:var(--v-fs-lbl); font-weight:600;
    transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .s-segbtn:hover:not(.on){ color:var(--v-txt); }
  .s-segbtn.on{ background:var(--v-accent-fill); color:var(--v-accent-ink); }

  /* Toggle switch. `--v-r-round` is one of the two shapes the rebrand allows to
     stay round (a slider thumb and a switch) — everything else is 3px. */
  .s-toggle{ position:relative; flex:0 0 auto; width:38px; height:21px; border-radius:var(--v-r-round);
    cursor:pointer; border:1px solid var(--v-500); background:var(--v-surf3); padding:0;
    transition:background var(--v-dur) var(--v-ease), border-color var(--v-dur) var(--v-ease); }
  .s-toggle:hover:not(:disabled){ border-color:var(--v-sel); }
  .s-toggle.on{ background:var(--v-sel); border-color:transparent; }
  .s-knob{ position:absolute; top:2px; left:2px; width:15px; height:15px; border-radius:50%;
    background:var(--v-dim); transition:transform 190ms var(--v-ease), background var(--v-dur) var(--v-ease);
    box-shadow:0 1px 2px rgba(0,0,0,.5); }
  .s-toggle.on .s-knob{ transform:translateX(17px); background:var(--v-sel-ink); }
  .s-toggle:disabled{ opacity:.4; cursor:not-allowed; }

  /* "Soon" — a control shown for shape but not yet wired, marked so it can't lie.
     Sits on --v-surf2, not --v-surf3: muted text on surf3 is 3.76:1, below WCAG AA,
     and this was the only rule in the app that did it (RG-74). Surf2 is 4.50:1.
     Not a pill any more: §1 is explicit that a pill in a control room reads as a
     toy, and this one marks something that does not work yet. */
  .s-soon{ display:inline-block; margin-left:8px; padding:1px 6px; border-radius:var(--v-r-sm);
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-faint);
    font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.04em; vertical-align:middle; }
  .s-dim{ color:var(--v-faint); }

  /* Boxed rows (outputs, network, updates, account, shortcuts) — seamed, like
     every other list on the desk. */
  .s-cardbox{ display:flex; flex-direction:column; gap:0; margin:0 -14px; }
  .s-netrow{ display:flex; align-items:center; justify-content:space-between; gap:12px;
    padding:8px 14px; background:transparent; border:0; border-bottom:1px solid var(--v-line); }
  .s-netrow:last-child{ border-bottom:0; }
  .s-netk{ font-size:var(--v-fs-b2); color:var(--v-dim); min-width:0; }
  /* The VALUE half of the row: mono so a figure that changes cannot reflow the
     name beside it, and right-aligned so a column of them reads down one edge. */
  .s-netv{ font-family:var(--f-mono); font-size:var(--v-fs-mono); line-height:var(--v-lh-mono);
    font-variant-numeric:tabular-nums; color:var(--v-txt); text-align:right; }
  /* Rose, not amber: amber means ON AIR and is never spent on anything else. */
  .s-netbad{ color:var(--v-rose); }

  /* THE FOOTNOTE, behind a hairline (§11). */
  .s-note{ margin:14px -14px 0; padding:12px 14px 0; border-top:1px solid var(--v-line);
    font-size:var(--v-fs-cap); line-height:1.6; color:var(--v-faint); max-width:none; }
  .s-note b{ color:var(--v-dim); font-weight:600; }
  .s-mt{ margin-top:14px; }
  .s-rule{ border:0; border-top:1px solid var(--v-line); margin:14px -14px 0; }

  /* Shortcuts */
  .s-scrow{ display:flex; align-items:center; gap:14px; padding:8px 14px;
    background:transparent; border:0; border-bottom:1px solid var(--v-line); }
  .s-scrow:last-child{ border-bottom:0; }
  .s-sckeys{ flex:0 0 118px; display:flex; gap:5px; }
  .s-kbd{ font-family:var(--f-mono); font-size:var(--v-fs-mono); color:var(--v-txt); background:var(--v-void);
    border:1px solid var(--v-line2); border-bottom-width:2px; border-radius:var(--v-r-sm); padding:1px 6px; }
  .s-scnote{ font-size:var(--v-fs-b2); color:var(--v-dim); }

  /* level meter */
  .s-meterwrap{ margin-top:16px; }
  .s-meter{ height:6px; border-radius:var(--v-r-round); background:var(--v-surf3); overflow:hidden; }
  .s-meter i{ display:block; height:100%; border-radius:var(--v-r-round);
    background:linear-gradient(90deg,var(--v-accent),var(--v-accent2)); }
  .s-meter-scale{ display:flex; justify-content:space-between; margin-top:7px;
    font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.05em; color:var(--v-faint); }
  .s-listen{ display:flex; align-items:center; gap:14px; margin-top:18px; flex-wrap:wrap; }
  .s-rms{ font-family:var(--f-mono); font-size:var(--v-fs-mono); letter-spacing:.03em; color:var(--v-faint);
    display:inline-flex; align-items:center; gap:7px; }
  .s-rms.voice{ color:var(--v-emerald); }
  .s-dot{ width:7px; height:7px; border-radius:50%; background:var(--v-faint); }
  .s-dot.on{ background:var(--v-emerald); box-shadow:0 0 7px var(--v-emerald); }
  .s-count{ font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.05em; color:var(--v-faint); }

  /* sliders */
  .s-slider{ margin-top:20px; }
  .s-slider:first-of-type{ margin-top:4px; }
  .s-slider-top{ display:flex; align-items:baseline; justify-content:space-between; margin-bottom:10px; }
  .s-slider-name{ color:var(--v-dim); font-size:var(--v-fs-b2); }
  .s-slider-val{ font-family:var(--f-mono); font-size:var(--v-fs-h1); font-weight:500; color:var(--v-accent);
    font-variant-numeric:tabular-nums; }
  .s-slider-ends{ display:flex; justify-content:space-between; margin-top:8px;
    font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.06em; text-transform:uppercase;
    color:var(--v-faint); }

  /* bible translations */
  .s-checklist{ display:flex; flex-direction:column; gap:0; margin:0 -14px; }
  .s-check-code{ font-family:var(--f-mono); font-size:var(--v-fs-mono); font-weight:600; letter-spacing:.05em;
    color:var(--v-txt); }
  .s-tr{ display:flex; align-items:center; gap:11px; width:100%; text-align:left; cursor:pointer;
    background:transparent; border:0; border-bottom:1px solid var(--v-line); padding:8px 14px;
    color:var(--v-txt); font-family:var(--f-body); font-size:var(--v-fs-b2);
    transition:background var(--v-dur) var(--v-ease); }
  .s-tr:last-child{ border-bottom:0; }
  .s-tr:hover:not(.on){ background:var(--v-surf2); }
  .s-tr.on{ background:var(--v-sel-soft); box-shadow:inset 2px 0 0 var(--v-sel); }
  .s-tr-dot{ width:13px; height:13px; border-radius:50%; flex:0 0 auto; border:2px solid var(--v-faint); }
  .s-tr-dot.on{ border-color:var(--v-accent); background:radial-gradient(circle,var(--v-accent) 40%,transparent 45%); }
  .s-tr-name{ color:var(--v-dim); flex:1; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .s-tr-active{ font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.1em;
    text-transform:uppercase; color:var(--v-accent); }
  .s-tr-note{ font-size:var(--v-fs-cap); color:var(--v-faint); margin-top:12px; line-height:1.6; }
  .s-tr-note b{ color:var(--v-dim); }

  /* network / model */
  .s-status{ display:inline-flex; align-items:center; gap:7px; margin-top:2px;
    font-family:var(--f-mono); font-size:var(--v-fs-mono); letter-spacing:.04em; }
  .s-status.ok{ color:var(--v-emerald); }
  .s-sdot{ width:7px; height:7px; border-radius:50%; background:currentColor; }
  .s-status.ok .s-sdot{ box-shadow:0 0 7px var(--v-emerald); }
  .s-modelpath{ margin-top:6px; font-family:var(--f-mono); font-size:var(--v-fs-cap); line-height:1.5;
    color:var(--v-faint); word-break:break-all; }

  /* ── OVERVIEW RAIL ── three groups in one pane, seamed, rather than three
     cards floating in a column with gutters between them. */
  .s-overbody{ display:flex; flex-direction:column; }
  .s-ocard{ display:flex; flex-direction:column; border-bottom:1px solid var(--v-line); }
  .s-ocard:last-child{ border-bottom:0; }
  .s-ohead{ padding:8px 12px 6px; background:var(--v-bg); border-bottom:1px solid var(--v-line);
    font-family:var(--f-mono); font-size:var(--v-fs-cap); line-height:var(--v-lh-cap); font-weight:600;
    letter-spacing:var(--v-tr-caps); text-transform:uppercase; color:var(--v-faint); }
  .s-ohead.danger{ color:var(--v-red); }
  .s-ocard.danger{ background:linear-gradient(180deg,var(--v-red-soft),transparent); }
  .s-orow{ display:flex; align-items:center; justify-content:space-between; gap:12px;
    padding:7px 12px; border-bottom:1px solid var(--v-line); }
  .s-orow:last-child{ border-bottom:0; }
  .s-ok{ font-size:var(--v-fs-b2); color:var(--v-dim); }
  .s-ov{ font-family:var(--f-mono); font-size:var(--v-fs-mono); font-variant-numeric:tabular-nums;
    color:var(--v-txt); text-align:right; }

  .s-qlink{ display:flex; align-items:center; gap:10px; width:100%; text-align:left; cursor:pointer;
    padding:8px 12px; border:0; border-bottom:1px solid var(--v-line); background:transparent;
    color:var(--v-dim); transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .s-qlink:last-child{ border-bottom:0; }
  .s-qlink:hover{ background:var(--v-surf2); color:var(--v-accent2); }
  .s-qtext{ display:flex; flex-direction:column; gap:1px; min-width:0; flex:1; }
  .s-qtext b{ font-size:var(--v-fs-b2); line-height:var(--v-lh-b2); font-weight:600; color:var(--v-txt); }
  .s-qtext em{ font-style:normal; font-size:var(--v-fs-cap); color:var(--v-faint);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .s-qarr{ flex:0 0 auto; color:var(--v-faint); }
  .s-qlink:hover .s-qarr{ color:var(--v-accent2); }

  .s-drow{ display:flex; align-items:center; justify-content:space-between; gap:12px; padding:9px 12px; }
  .s-dbtn{ flex:0 0 auto; width:30px; height:30px; display:grid; place-items:center; cursor:pointer;
    border-radius:var(--v-r-sm); background:var(--v-red-soft); border:1px solid var(--v-red-soft); color:var(--v-red);
    transition:background var(--v-dur) var(--v-ease); }
  .s-dbtn:hover{ background:var(--v-red-soft); }
  /* Armed: the destructive action is one click from happening — make it read red. */
  .s-dbtn.arm{ background:var(--v-red); border-color:var(--v-red); color:#fff; }
  .s-reset.arm{ border-color:var(--v-red); color:var(--v-red); }

  /* ── responsive ── */
  /* The frame hides the inspector column below 1240px and stacks below 900px;
     these are the rules that are this workspace's own. A setting row that cannot
     hold its control beside its name puts the control underneath rather than
     crushing the sentence that explains what it does. */
  @media (max-width:820px){
    .s-row{ flex-direction:column; align-items:stretch; gap:10px; }
    .s-rowctl{ max-width:none; }
  }

  .s-roomrow{ align-items:flex-start; }
  .s-lang{ width:100%; border-collapse:collapse; font-size:var(--v-fs-b2); margin-top:10px; }
  .s-lang th{ text-align:left; font-weight:500; font-size:var(--v-fs-cap); letter-spacing:.06em;
    text-transform:uppercase; color:var(--v-faint); padding:6px 8px;
    border-bottom:1px solid var(--v-line); }
  .s-lang td{ padding:7px 8px; border-bottom:1px solid var(--v-line2); color:var(--v-dim); }
  .s-langcode{ color:var(--v-faint); font-size:var(--v-fs-cap); }
  /* An absence is dim, not red: nobody has failed here — the work has not been
     done, and saying so is the whole point of the column. */
  .s-langgap{ color:var(--v-faint); font-style:italic; }
  /* The one row on the Privacy page that can say something is leaving. Emerald is
     "confirmed/connected" in the design system; here it marks the state that is
     ACTIVE, not the state that is good — the copy carries the judgement. */
  .s-netv.on{ color:var(--v-emerald); }
  .s-roomname{ flex:1; min-width:0; display:flex; flex-direction:column; gap:2px; }
  .s-roomnote{ font-size:var(--v-fs-cap); color:var(--v-faint); }
</style>
