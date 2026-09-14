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
  // THE shortcut table — one array, shared with the keydown handler, the
  // cheatsheet, Help and `sectionkeys.js::RESERVED`. See the note further down.
  import { SHORTCUTS } from '../shortcuts.js';
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
  // SECTION NAV — ELEVEN SECTIONS, MERGED FROM EIGHTEEN (docs/REBRAND.md §11).
  //
  // The rail used to carry eighteen entries, seven of which were one screen cut
  // in half: Network and Integrations, Scripture and Languages, Privacy and
  // Advanced, History and Backup, Audio and Voice Profiles. A section that is
  // three rows on a full-height page teaches an operator that the rail is long
  // and mostly empty, and that is how a control gets lost.
  //
  // Two sections were not merged but DELETED, because everything on them was a
  // second copy of something else:
  //   · `dashboard` — the readiness surface itself is untouched (it is the boot
  //     ladder's own probes, re-run on demand); it now opens Diagnostics, which
  //     is the section an operator reaches for when they ask "is this machine
  //     going to work?". One question, one section.
  //   · `account` — its Licence, Version and Environment rows are already in the
  //     Overview rail on every section, and its "there are no accounts" sentence
  //     is already a row on the Privacy report. Three rows, all duplicates.
  //
  // The `desc` is role two of §11's three: the STANDFIRST, one sentence about
  // what the section is for. It is rendered once, by the frame, and no section
  // repeats it a size smaller at the top of its own panel.
  // ─────────────────────────────────────────────────────────────────────────
  const SECTIONS = [
    { key: 'general',     label: 'General',                desc: 'How this copy of Relay behaves, and whether it is armed at all.', icon: 'gear' },
    { key: 'screens',     label: 'Screens & looks',        desc: 'Which template each kind of content wears, and which screens follow it.', icon: 'monitor' },
    { key: 'audio',       label: 'Audio',                  desc: 'Microphone, live level, video sound output, and the rooms you run in.', icon: 'mic' },
    { key: 'ai',          label: 'AI & Detection',         desc: 'What the gate lets through, and the calibration it keeps per preacher.', icon: 'sparkle' },
    { key: 'scripture',   label: 'Scripture & Languages',  desc: 'Recognition language, Bible translations, and what Relay really knows.', icon: 'book' },
    { key: 'network',     label: 'Network & Integrations', desc: 'How screens, OBS and the speech model reach this machine.', icon: 'nodes' },
    { key: 'history',     label: 'History & Backup',       desc: 'Past services, the setup walk-through, and what is held back during one.', icon: 'clock' },
    { key: 'shortcuts',   label: 'Shortcuts',              desc: 'The keys the live desk is driven from.', icon: 'keyboard' },
    { key: 'updates',     label: 'Updates',                desc: 'This build, the update channel, and what an update would do to your history.', icon: 'refresh' },
    { key: 'diagnostics', label: 'Diagnostics',            desc: 'Is this machine going to work — and the facts a support request needs.', icon: 'terminal' },
    { key: 'privacy',     label: 'Privacy & Advanced',     desc: 'What is on this machine, what can leave it, and the one thing that does.', icon: 'shield' },
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
  };

  // ─────────────────────────────────────────────────────────────────────────
  // THERE ARE NO UI PREFERENCES LEFT, AND THAT IS THE POINT (DECISIONS §69).
  //
  // This block used to hold `relay.prefs.v1` — a localStorage object, a loader, a
  // saver, a setter, and a `GENERAL_TOGGLES` table that rendered the last two
  // survivors as switch rows: **Auto Start on Login** and **Minimize to System
  // Tray**. Both were `disabled` with a "Soon" chip beside them, which was offered
  // as the honest middle ground between shipping a lie and shipping nothing.
  //
  // It is not a middle ground. §69's defect is a control that saves a preference
  // nothing reads, and these saved a preference nothing reads while ALSO looking
  // like the live switch two rows above them — same 38×21 body, same steel-blue
  // track, greyed rather than absent. An operator scanning this page reads a
  // column of switches; the two at the bottom are furniture. "Soon" is a promise
  // as well, and neither of these is on any roadmap: autostart needs a Tauri
  // plugin Relay does not bundle and a tray icon needs a tray Relay does not have.
  //
  // Removed rather than deferred, which takes the tally on this page to ELEVEN.
  // With them went the whole preference store: `DEFAULT_PREFS` held only these
  // two, so the object, the key and its three functions had no other reader. The
  // settings that MATTER — language, service length, safe mode, thresholds,
  // translation, model, rooms, crash reporting, channel templates — live in the
  // database or in the engine and never went through this file's localStorage at
  // all. `settingssections.test.js` holds the absence from both ends: nothing in
  // `src/` opens that key, and this file declares no preference object.
  //
  // THEME WAS REMOVED HERE TOO, and it was the eighth to go. It was a three-way
  // segmented control (Light · Dark · System) that wrote `prefs.theme` and stamped
  // `data-theme` on the document element. **Nothing in this application reads
  // either** — no rule in `app.css`, no component — so picking Light saved a
  // preference, changed a dataset attribute no stylesheet consults, and repainted
  // nothing. Its own comment admitted as much. It is not "Soon" either:
  // docs/REBRAND.md §1 is explicit — **dark only**, "the booth is dark and the
  // wall is black" — so a light sheet is not a deferred feature, it is a decision
  // against, and a picker offering two choices the product has decided not to have
  // is worse than no picker.
  //
  // 'Confirm Before Going Live' and 'Auto Save' were on this list as well, both
  // defaulting to ON and neither read by anything. The first was the worse by a
  // distance: it promised a confirmation step between the operator and the
  // congregation's screen, and there has never been one. It must not come back
  // because the prototype's General section shows one — a prototype cannot promise
  // a guard the engine does not have.
  // ─────────────────────────────────────────────────────────────────────────

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

  // WHICH SCREENS ACTUALLY FOLLOW THE MAP ABOVE. A read-only row per screen, and
  // the reason it is here is DECISIONS §70: a screen's OWN template wins over a
  // content look (§29), so a screen that has one ignores every choice on this
  // page. For most of this product's life every screen always had one, and the
  // content-look map could be filled in, saved, and change nothing in the
  // building — with no way to tell from this screen.
  //
  // Nothing here is editable; the screens are configured in the Outputs tab. It
  // is the answer to "will what I just set do anything?", which the map alone
  // cannot give.
  let screens = [];
  let screensState = 'loading';
  async function loadScreens() {
    try {
      screens = await listOutputChannels();
      screensState = 'ok';
    } catch {
      screens = [];
      screensState = 'failed';
    }
  }
  onMount(loadScreens);
  // Reactive on purpose: the names come from `$templates`, which loads after this
  // list does. A plain function would resolve once and keep printing
  // "template #3" for the rest of the session.
  /** The look a screen is wearing: its own template, or the content look. */
  $: screenLook = (ch) =>
    ch.template_id == null
      ? 'follows the content look'
      : ($templates.find((t) => t.id === ch.template_id)?.name ?? `template #${ch.template_id}`);

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

  // RESET ALL SETTINGS WAS REMOVED, and it was two controls, not one: the same
  // `resetAllSettings` hung off the rail's footer button AND off the Danger Zone
  // card in the Overview rail, so the page offered the identical destructive
  // action twice, six inches apart, with two different labels.
  //
  // Both are gone, because with `theme` deleted above there is nothing left for
  // it to restore. `DEFAULT_PREFS` now holds only `autoStart` and `minimizeTray`,
  // and both of those switches are permanently disabled — they cannot be moved,
  // so they can never differ from their defaults, so resetting them to their
  // defaults is a no-op with a red button and a two-step confirmation on it.
  // Every setting that MATTERS (language, thresholds, translation, model, rooms,
  // crash reporting, channel templates) lives in the database and was never
  // touched by this control: an operator pressing it was being told they had
  // restored their settings while their settings sat exactly where they were.
  // DECISIONS §69, and CLAUDE.md rule 15 in its quietest form.

  onMount(loadServiceTarget);
  onMount(async () => {
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

  // THE SHORTCUT TABLE IS NOT WRITTEN HERE. It is imported, at the top of this
  // file, from `lib/shortcuts.js` — the same array the one global keydown handler
  // switches on, the cheatsheet overlay renders, `views/Help.svelte` renders and
  // `sectionkeys.js::RESERVED` subtracts from before it hands a letter to a song
  // section.
  //
  // This section used to carry a SECOND, hand-maintained copy of six rows, and it
  // had already drifted from the bindings it claimed to describe:
  //
  //   · `A` (accept the top AI suggestion) and `D` (dismiss it) were MISSING —
  //     the two keys that put a machine's guess in front of a congregation or
  //     throw it away, absent from the page an operator opens to learn the keys.
  //     They are also two of the letters `RESERVED` withholds from song sections,
  //     so this page could not explain why `a` does not fire verse A either.
  //   · `/` (jump to the manual reference box) was missing.
  //   · `PgDn` / `PgUp` were missing.
  //   · `?` was described as "Open Help & full shortcut list"; it opens the
  //     cheatsheet overlay.
  //
  // That is the repository's own named failure — a guarantee kept on one door and
  // skipped on its twin — applied to a help screen, which is the worst place for
  // it: it teaches an operator something false, under pressure. `shortcuts.js`'s
  // own comment says a help screen listing a key that does nothing is worse than
  // no help screen; the inverse, a help screen omitting a key that DOES something,
  // is the same defect with the sign flipped.
  //
  // The keys are rendered from `SHORTCUTS` (every binding, always), not from
  // `liveShortcuts` (only those the mounted surface registered) — Settings is not
  // the run surface, so "what does this key do on Live" is the question being
  // asked here, and filtering by what Settings itself registers would empty it.
</script>

<!-- Settings is laid out in the shared workspace grammar (`WorkspaceFrame.svelte`,
     docs/REBRAND.md §2 · §11) — the same columns, panes and type roles the
     Planner and Outputs desks use.

     ── ONE TYPE SCALE, THREE ROLES ──────────────────────────────────────────
     §11's roles are page title / standfirst / row, with a footnote behind a
     hairline, and they are the FRAME'S: `.rw-h1`, `.rw-lead`, `.rw-nv` /
     `.rw-nvk` / `.rw-nvv` / `.rw-nvnote`, `.rw-group` and `.rw-foot`. This file
     used to define its own copies of all six — `.s-lead`, `.s-row` with
     `.s-rowtitle`/`.s-rownote`, a SECOND row grammar in `.s-netrow`/`.s-netk`/
     `.s-netv`, `.s-grouphead`, and two footnotes (`.s-note` and `.s-tr-note`,
     one of them without the hairline). The frame's own comment says why that
     cannot stand: a type scale defined three times is three type scales, and
     the two row grammars had already drifted to different paddings and a
     different key colour. Everything below uses the frame's roles; what is left
     private to this file is the handful of things only Settings has (a level
     meter, the threshold sliders, the translation list, the language table).

     THE PAGE TITLE IS THE SECTION, and it used to be the word "Settings" — which
     never changed, while the rail beside it highlighted the section, the pane
     header above the rows repeated the section, and the standfirst described the
     section. Three of those four said the same thing and the fourth said nothing:
     the one role that is meant to name what you are looking at was the only one
     that could not. So the title is `activeSection.label`, the standfirst stays
     its sentence, and the panel's own pane header is gone rather than printing the
     title again eight pixels smaller. That is the prototype's arrangement too.

     ELEVEN SECTIONS, merged from eighteen. The reasoning, and the two sections
     deleted rather than merged, are in the SECTIONS array above.

     TWO COLUMNS, NOT THREE (§11): a rail plus one reading column, centred and
     capped at 880px. The third column's own tombstone is below the panel. -->
<WorkspaceFrame
  title={activeSection.label}
  standfirst={activeSection.desc}
  columns="212px minmax(0,1fr)">
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
    </aside>

    <!-- ════ ACTIVE PANEL ════ -->
    <main class="rw-pane s-read">
      <div class="rw-panebody s-panel">

      {#if section === 'general'}
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Application language</div>
            <p class="rw-nvnote">The language the operator console is written in. Missing words stay in English.</p>
          </div>
          <select class="r-select rw-nvctl s-sel" value={$locale} on:change={(e) => setLocale(e.target.value)} aria-label="Application language">
            {#each LOCALES as l}
              {@const pct = coverage(l.code)}
              <option value={l.code}>{l.label}{pct === 100 ? '' : ` · ${pct}%`}</option>
            {/each}
          </select>
        </div>

        <!-- Service length — drives the REMAINING timer on a stage/confidence
             monitor. 0 = no target (the remaining line stays blank). Read by the
             backend when the next service starts. -->
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Service length</div>
            <p class="rw-nvnote">Planned length in minutes. Shows a “time remaining” timer on stage and confidence monitors. 0 = no target. Applies to the next service you start.</p>
          </div>
          <div class="rw-nvctl s-lenctl">
            <input class="r-input s-leninput" type="number" min="0" max="600" step="5"
              value={$serviceTargetMinutes}
              on:change={(e) => setServiceTarget(e.target.value)}
              aria-label="Service length in minutes" />
            <span class="s-lenunit r-mono">min</span>
          </div>
        </div>

        <!-- SAFE MODE. Moved here from Backup & Recovery, which is where it was
             least likely to be looked for: safe mode is not a backup and not a
             recovery, it is whether this copy of Relay is ARMED — the one switch
             that decides whether anything Relay does can reach a screen at all.
             `degraded.js` names the new home in the sentence it prints. -->
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Safe mode</div>
            <p class="rw-nvnote">Outputs will not open and detection is disarmed — nothing Relay does can reach a screen. A way to open the console with no risk of putting something on a wall.</p>
          </div>
          <div class="rw-nvctl s-nvpair">
            <span class="rw-nvv" class:s-armed={$safeMode}>{$safeMode ? 'on' : 'off'}</span>
            <button class="r-btn ghost sm" on:click={() => setSafeMode(!$safeMode)}>
              {$safeMode ? 'Turn off' : 'Turn on'}
            </button>
          </div>
        </div>

        <!-- SCREENS AT LAUNCH. A statement of what Relay already does, in the
             place an operator asks the question — NOT a switch.
             `App.svelte` calls `autoOpenOutputs()` on mount unless safe mode is
             on, so every screen that was open when Relay last closed comes back
             by itself. The prototype's General section offers this as a toggle;
             building one would mean a persisted preference and a reader for it,
             and there is no reader — which is DECISIONS §69's defect exactly.
             So the behaviour is stated instead, and it is stated from the LIVE
             value of the one thing that changes it, which is the row above. -->
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Screens at launch</div>
            <p class="rw-nvnote">Every screen that was open when Relay last closed is reopened automatically. Safe mode is the only thing that stops it.</p>
          </div>
          <span class="rw-nvv" class:s-armed={$safeMode}>{$safeMode ? 'held back by safe mode' : 'reopened automatically'}</span>
        </div>

        <div class="s-prose">
          <!-- The absence is stated, and the list of names is not: an operator
               needs to know that a switch they remember never did anything, not to
               read a changelog on the page they came here to use. The eleven names
               and the reason each one went live in DECISIONS §69 and in the
               comments beside the code that used to render them. -->
          <p class="rw-foot">
            <b>Eleven preference controls used to be on this page and are not any
            more</b>, each because it saved a setting nothing in Relay ever read.
            One of them was on by default and promised a confirmation step between
            you and the congregation's screen — there has never been one. The last
            two, <i>Auto Start on Login</i> and <i>Minimize to System Tray</i>, were
            greyed out with a “Soon” tag beside them, which is a promise as well.
            Nothing was lost, because nothing they did ever happened.
          </p>
        </div>

      {:else if section === 'screens'}
        <div class="rw-group">Content looks</div>
        {#each contentTypes as ct}
          <div class="rw-nv">
            <span class="rw-nvk">{ct.label}</span>
            <select class="r-select rw-nvctl s-sel" value={ctMap[ct.key] ?? ''} on:change={(e) => pickCt(ct.key, e.target.value)} aria-label="{ct.label} content look">
              <option value="">Channel default</option>
              {#each $templates as tpl}<option value={tpl.id}>{tpl.name}</option>{/each}
            </select>
          </div>
        {/each}

        <!-- WILL ANY OF THAT DO ANYTHING? Read-only, and the reason it is here is
             DECISIONS §70: a screen's OWN template wins over a content look (§29),
             so this map only reaches screens that have no template of their own.
             For most of this product's life every screen always had one and the
             map could be filled in, saved, and change nothing in the building —
             with nothing on this page able to say so. -->
        <div class="rw-group">Screens</div>
        {#if screensState === 'loading'}
          <div class="rw-nv"><span class="rw-nvk">Screens</span><span class="rw-nvv">{settingValue(null, { loading: true })}</span></div>
        {:else if screensState === 'failed'}
          <div class="rw-nv"><span class="rw-nvk">Screens</span><span class="rw-nvv">{settingValue(null, { missing: 'could not be read' })}</span></div>
        {:else}
          {#each screens as ch (ch.id)}
            <div class="rw-nv">
              <span class="rw-nvk">{ch.name}</span>
              <span class="rw-nvv">{screenLook(ch)}</span>
            </div>
          {:else}
            <div class="rw-nv"><span class="rw-nvk">Screens</span><span class="rw-nvv">{settingValue(null, { missing: 'none configured yet' })}</span></div>
          {/each}
        {/if}

        <div class="s-prose">
          <p class="rw-foot">
            A screen's own template <b>wins</b> over a content look (DECISIONS §29),
            so the settings above reach only the screens that say <i>follows the
            content look</i>. “Channel default” leaves the look to each output's own
            template. Screens are added, given a look and given their copy-links and
            QR codes in the <b>Outputs</b> tab — nothing on this page changes a
            screen.
          </p>
        </div>

      {:else if section === 'audio'}
        <div class="rw-group">Microphone</div>
        <div class="s-prose">
          <div class="s-inline">
            {#if $capture.available}
              <span class="s-count">{$capture.devices.length} device{$capture.devices.length === 1 ? '' : 's'}</span>
            {:else}
              <span class="s-count">backend not attached</span>
            {/if}
          </div>
          <select class="r-select" value={$capture.inputDevice} on:change={(e) => setInputDevice(e.target.value)} disabled={!$capture.available || $capture.capturing} aria-label="Microphone input device">
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
            <p class="s-alert" role="alert">{micErr}</p>
          {/if}
        </div>

        <!-- AUDIO OUTPUT (speakers for video sound). Same section as the mic on
             purpose: input and output are one operator question. -->
        <div class="rw-group">Audio output</div>
        <div class="s-prose">
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
          <select class="r-select" value={outDevice} on:change={(e) => pickOutput(e.target.value)} aria-label="Speakers for video sound">
            <option value="">System default — computer speakers</option>
            {#each outDevices as d}
              <option value={d.id}>{d.label || 'Speaker'}{d.is_default ? ' — default' : ''}</option>
            {/each}
          </select>

          {#if !sinkOk}
            <p class="rw-foot">
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
            <p class="rw-foot">
              Sound plays on your <b>system default</b> speakers right now. macOS hides
              the list of other outputs until this app has been granted the microphone
              once — <b>Detect speakers</b> asks for it, then releases the mic straight
              away (capture still runs through the audio engine, not the browser).
            </p>
          {:else}
            <p class="rw-foot">
              Where video sound plays on the <b>fullscreen output window</b>. OBS/kiosk
              browser sources are left muted — OBS mixes their audio itself.
            </p>
          {/if}
        </div>

        <div class="rw-group">Rooms</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            Save this space — microphone, recognition language, planned length, voice
            profile and which display each screen goes to — and put it all back with one
            press next time. <b>The audio levels are not saved.</b> Relay learns those
            fresh every time on purpose: a level measured three weeks ago, in a room
            that now has the heating on and forty more people in it, is a guess, and
            guessing is what once made Relay deaf to a quiet preacher.
          </p>
          <div class="s-addrow">
            <input class="r-input" placeholder="Main hall" bind:value={roomName} aria-label="Room name" />
            <button class="r-btn ghost sm" on:click={doSaveRoom} disabled={roomBusy}>Save this room</button>
          </div>
          {#if roomMsg}<p class="rw-foot" role="status">{roomMsg}</p>{/if}
        </div>
        {#each $rooms as r (r.id)}
          <div class="rw-nv">
            <div class="s-nvtext">
              <div class="rw-nvk">{r.name}</div>
              {#if r.notes}<p class="rw-nvnote">{r.notes}</p>{/if}
            </div>
            <div class="rw-nvctl s-nvpair">
              <button class="r-btn ghost sm" on:click={() => doUseRoom(r)} disabled={roomBusy}>Use</button>
              <button class="r-btn ghost sm" on:click={() => doDeleteRoom(r)} disabled={roomBusy}>Remove</button>
            </div>
          </div>
        {:else}
          <div class="rw-nv"><span class="rw-nvk">Saved rooms</span><span class="rw-nvv">{settingValue(null, { missing: 'none yet' })}</span></div>
        {/each}

      {:else if section === 'ai'}
        <div class="rw-group">Detection thresholds</div>
        <div class="s-prose">
          <div class="s-inline"><span class="s-count">self-calibrating</span></div>
          <div class="s-slider">
            <div class="s-slider-top">
              <span class="r-lbl s-slider-name">Auto-fire above</span>
              <span class="s-slider-val">{Math.round($capture.thresholds.auto_fire * 100)}%</span>
            </div>
            <input class="r-range" type="range" min="0.5" max="0.99" step="0.01"
              value={$capture.thresholds.auto_fire}
              on:input={(e) => onAuto(+e.target.value)} disabled={!$capture.available} use:rangeFill={$capture.thresholds.auto_fire} aria-label="Auto-fire above" />
            <div class="s-slider-ends"><span>LAX (50%)</span><span>STRICT (100%)</span></div>
          </div>
          <div class="s-slider">
            <div class="s-slider-top">
              <span class="r-lbl s-slider-name">Suggest above</span>
              <span class="s-slider-val">{Math.round($capture.thresholds.suggest * 100)}%</span>
            </div>
            <input class="r-range" type="range" min="0.3" max="0.9" step="0.01"
              value={$capture.thresholds.suggest}
              on:input={(e) => onSuggest(+e.target.value)} disabled={!$capture.available} use:rangeFill={$capture.thresholds.suggest} aria-label="Suggest above" />
            <div class="s-slider-ends"><span>PASSIVE</span><span>HYPER-AWARE</span></div>
          </div>
          <p class="rw-foot">Only a direct, high-confidence quotation can ever auto-fire. A paraphrase is always a suggestion — a cosine is not a probability.</p>
        </div>

        <!-- VOICE PROFILES. Merged into this section rather than carrying their own:
             a profile IS a calibration of the gate above it, and splitting the dial
             from the thing it calibrates across two rail entries is how an operator
             comes to believe they are unrelated. -->
        <div class="rw-group">Voice profiles</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            One profile per preacher. Each remembers the language they preach in, the
            names and places Relay should expect to hear, and how cautious the gate
            should be for that voice — so calibration is not relearned from scratch
            every Sunday.
          </p>
          {#if profileErr}
            <p class="s-alert" role="alert">{profileErr}</p>
          {/if}
        </div>

        {#each profiles as p (p.id)}
          <div class="rw-nv">
            <div class="s-nvtext">
              <div class="rw-nvk">
                {p.name}
                {#if p.is_active}<span class="s-vpactive r-mono">active</span>{/if}
              </div>
              <p class="rw-nvnote">
                {p.language ? p.language.toUpperCase() : 'Auto-detect (code-switching)'}
                · sensitivity {p.sensitivity}
                · gate {Math.round(p.auto_fire * 100)}% / {Math.round(p.suggest * 100)}%
              </p>
            </div>
            <div class="rw-nvctl s-nvpair">
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
          <div class="rw-nv"><span class="rw-nvk">Profiles</span><span class="rw-nvv">{settingValue(null, { missing: 'none yet' })}</span></div>
        {/each}

        <div class="s-prose">
          <div class="s-addrow">
            <input
              class="r-input"
              placeholder="Preacher's name"
              aria-label="New voice profile name"
              bind:value={newName}
              on:keydown={(e) => e.key === 'Enter' && addProfile()} />
            <button class="r-btn" disabled={profileBusy || !newName.trim()} on:click={addProfile}>Add</button>
          </div>
          {#if !profiles.length}
            <p class="rw-foot">The first profile you add becomes the active calibration.</p>
          {/if}
        </div>

        {#if editing}
          <div class="rw-group">Editing “{editing.name}”</div>
          <div class="s-prose">
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
            <p class="rw-foot">
              Comma-separated. These are fed to the decoder as a hint, which is how an
              unusual name stops being transcribed as something else. It biases
              recognition — it does not force it.
            </p>

            <div class="s-slider">
              <div class="s-slider-top">
                <span class="r-lbl s-slider-name">Sensitivity</span>
                <span class="s-slider-val">{editing.sensitivity}</span>
              </div>
              <input class="r-range" type="range" min="0" max="100" step="1" bind:value={editing.sensitivity} use:rangeFill={editing.sensitivity} aria-label="Sensitivity" />
              <div class="s-slider-ends"><span>CAUTIOUS</span><span>EAGER</span></div>
            </div>
            <!-- THE ONE THING THIS FORM MUST NOT GET WRONG. The learned pair is shown,
                 never edited: it is what the router worked out from this operator's
                 confirmations. Moving the dial above is the operator deliberately
                 re-baselining, and only then does the backend re-derive these. A
                 rename must never cost them their calibration. -->
            <p class="rw-foot">
              Learned gate for this voice: <b>auto-fire {Math.round(editing.auto_fire * 100)}%</b>,
              <b>suggest {Math.round(editing.suggest * 100)}%</b> — set by Relay from what you
              have confirmed, not by hand. Renaming or changing the language keeps them.
              <b>Moving the sensitivity dial resets them</b>, because that is you saying the
              gate is wrong.
            </p>

            <div class="s-addrow">
              <button class="r-btn primary" disabled={profileBusy} on:click={saveProfile}>Save profile</button>
              <button class="r-btn ghost" disabled={profileBusy} on:click={() => (editing = null)}>Cancel</button>
            </div>
          </div>
        {/if}

      {:else if section === 'scripture'}
        <div class="rw-group">Recognition language</div>
        <div class="s-prose">
          <select class="r-select" value={$capture.stt.language ?? ''} on:change={(e) => setSttLanguage(e.target.value || null)} disabled={!$capture.stt.loaded} aria-label="Recognition language">
            <option value="">Auto-detect (code-switching)</option>
            <option value="en">English</option>
            <option value="yo">Yoruba</option>
            <option value="sw">Swahili</option>
            <option value="ha">Hausa</option>
          </select>
          <p class="rw-foot">Auto-detect handles English mixed with a local language mid-sentence — the normal case. Tier-1: Yoruba · Swahili · Hausa.</p>
        </div>

        <div class="rw-group">Bible translations</div>
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
        <div class="s-prose">
          <p class="rw-foot">Only public-domain <b>KJV</b> is bundled. Additional versions need their verse data added to the corpus.</p>
        </div>

        <!-- LANGUAGES. Merged into this section from its own rail entry: the
             recognition language above is chosen FROM this table, and the honest
             answer to "should I pick Yoruba?" is two rows down from the picker
             rather than two clicks away. -->
        <div class="rw-group">Language coverage</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            Counted from the data Relay ships with. Nothing here is a claim —
            improving a number means improving the table the detector uses, which is
            a one-line change anyone who speaks the language can make.
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
                    <!-- Three states, not two, and the middle one is the point of
                         this column. Yorùbá numerals PARSE (they are vigesimal and
                         subtractive — 16 is ẹrìndínlógún) but no native speaker has
                         checked the table, so they are capped at suggest and can
                         never reach a wall unattended. Printing a bare "yes" beside
                         Kiswahili would claim the two behave the same. -->
                    <td
                      class="r-mono"
                      class:s-langgap={!l.numerals || !l.numerals_auto_fire}
                      title={l.numerals && !l.numerals_auto_fire
                        ? 'Parsed, but no native speaker has reviewed these numbers — a reference resolved through them is offered to you and never fired on its own.'
                        : null}
                    >{!l.numerals ? 'no' : l.numerals_auto_fire ? 'yes' : 'suggest only'}</td>
                    <td class="r-mono" class:s-langgap={coverage(l.code) === 0}>{coverage(l.code)}%</td>
                    <!-- ABSENCES, not scores. Nothing observes a native speaker's
                         judgement, and none has looked at these tables. -->
                    <td class="r-mono s-langgap">not yet</td>
                    <td class="r-mono s-langgap">not measured</td>
                  </tr>
                {/each}
              </tbody>
            </table>
            <p class="rw-foot">
              <b>“Accuracy” is empty because it has never been measured</b> — in any
              language, including English. Measuring it needs about thirty minutes of
              real preaching on tape and somebody who speaks the language to write down
              what was actually said. Until that exists, any figure here would be a
              guess wearing a percentage sign.
            </p>
            <p class="rw-foot">
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
            <p class="rw-foot">The language tables could not be read.</p>
          {/if}
        </div>

      {:else if section === 'network'}
        <div class="rw-group">This machine</div>
        <div class="rw-nv"><span class="rw-nvk">Found on this computer</span><span class="rw-nvv">{settingValue(lanIp, {
            loading: lanState === 'loading',
            missing: lanState === 'failed' ? 'could not be read' : 'not on a network',
          })}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Output / stage pages</span><span class="rw-nvv">:8032 · http</span></div>
        <div class="rw-nv"><span class="rw-nvk">Live update channel</span><span class="rw-nvv">:8031 · websocket</span></div>

        <div class="rw-group">Offline speech model</div>
        <div class="s-prose">
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
        </div>

        <!-- INTEGRATIONS. Merged in from their own rail entry: every row on it was
             an address on this machine's network, which is what the three rows at
             the top of this section already are. -->
        <div class="rw-group">Other software</div>
        <!-- The URL is CHANNEL-keyed (DECISIONS §29). Changing a screen's template
             broadcasts a channel_template message the output applies by matching its
             OWN `channel` — so a template swap is live with no re-copying of the URL.
             This row used to show a `?template_id=<n>`-only shape, which parses to
             channel 0 ("no channel"): it renders, so it looks right, and then it is
             the one browser source in the building that never follows a template
             change. Copy URL in Outputs → Sharing is still the only thing that fills
             in the real ids. -->
        <div class="rw-nv"><span class="rw-nvk">OBS / vMix (browser source)</span><span class="rw-nvv">http://{lanIp || 'this-pc'}:8032/output.html?channel=&lt;screen&gt;&amp;template_id=&lt;n&gt;</span></div>
        <div class="rw-nv"><span class="rw-nvk">Kiosk screen / stage tablet</span><span class="rw-nvv">:8032 · http</span></div>
        <div class="rw-nv"><span class="rw-nvk">NDI</span><span class="rw-nvv">not available</span></div>
        <div class="rw-nv"><span class="rw-nvk">ATEM / SDI switcher</span><span class="rw-nvv">via HDMI</span></div>
        <div class="s-prose">
          <p class="rw-foot">Relay sends its output to other software over your local network — no plugins to install. Add a <b>Browser Source</b> pointing at Relay; the exact per-channel URL is in <b>Outputs → Sharing</b>. Connected devices (OBS · kiosk · stage remote) pull the live output from this machine on the same Wi-Fi.</p>
          <p class="rw-foot"><b>NDI is parked</b> — it needs a proprietary SDK Relay does not bundle, so there is no NDI source to select. For an <b>ATEM or other SDI switcher</b>, open a Relay output window on an HDMI screen and feed that HDMI into the switcher — Relay does not speak SDI directly (and won't; that is served by the hardware you already own).</p>
        </div>

      {:else if section === 'history'}
        <!-- History moved into Settings. The view is self-contained (its own list,
             detail, search, export) and reads from the same local SQLite store. -->
        <div class="s-history"><History /></div>

        <div class="rw-group">Setup</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            <b>New here?</b> The setup walk-through picks your projector, checks the microphone is actually hearing something, and ends by putting a real verse on your real screen — so you have <i>seen</i> it work before Sunday.
          </p>
          <button class="r-btn ghost sm" on:click={restartSetup}>Run the setup walk-through</button>
        </div>

        <!-- SERVICE LOCK. Reachable from the sentence the refusal itself prints,
             which is the whole reason it lives here and not somewhere tidier. -->
        <div class="rw-group">Service lock</div>
        <div class="s-prose">
          {#if $serviceLock.engaged}
            <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
              <b style="color:var(--v-amber);">A service is being recorded.</b>
              Relay is holding back a few things that cannot be undone, or that would take
              the speech engine away mid-sermon: {$serviceLock.held_back.join(', ')}.
              Firing, the transport, clearing and blacking out are unaffected.
            </p>
            <button class="r-btn ghost sm" on:click={unlockService}>Unlock for this service</button>
            {#if lockErr}<p class="s-alert" role="alert">{lockErr}</p>{/if}
          {:else}
            <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
              While a service is being recorded, Relay holds back deletions, speech-model
              changes and imports — an accident at 10:31 has no undo. It arms itself when you
              start listening and lifts when the service ends. Nothing on the live path is
              ever held back.
            </p>
          {/if}
        </div>

      {:else if section === 'shortcuts'}
        <!-- ALWAYS ON, wherever you are — the panic keys and the cheatsheet.
             Split out because the distinction is the whole point of `always` in
             the table: these three fire from a global handler that survives a
             crashed view, and the rest only work where the surface offers the
             action (rule 15, DECISIONS §20). A list that ran them together would
             be telling an operator that `A` is as reliable as `Esc`. -->
        <div class="rw-group">Always active</div>
        {#each SHORTCUTS.filter((s) => s.always) as sc}
          <div class="rw-nv">
            <span class="rw-nvk">{sc.label}</span>
            <span class="s-sckeys rw-nvctl">{#each sc.keys as k}<kbd class="s-kbd">{k}</kbd>{/each}</span>
          </div>
        {/each}

        <div class="rw-group">On the run surface</div>
        {#each SHORTCUTS.filter((s) => !s.always) as sc}
          <div class="rw-nv">
            <span class="rw-nvk">{sc.label}</span>
            <span class="s-sckeys rw-nvctl">{#each sc.keys as k}<kbd class="s-kbd">{k}</kbd>{/each}</span>
          </div>
        {/each}

        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            The second group works on a surface that offers the action — Live offers
            all of them; the Planner registers only next and previous, so <kbd class="s-kbd">A</kbd>,
            <kbd class="s-kbd">D</kbd> and <kbd class="s-kbd">/</kbd> do nothing there and the
            cheatsheet does not claim otherwise. <kbd class="s-kbd">→</kbd> is mode-aware: it
            steps a plan slide when plan content is on air and walks the passage when a verse
            is. The transport bar on Live always prints which.
            <br /><br />
            A surface showing a song's sections also takes single letters
            (<kbd class="s-kbd">v</kbd> <kbd class="s-kbd">c</kbd> <kbd class="s-kbd">b</kbd> …)
            — never one this page lists, and never while a field has focus.
          </p>
          <button class="r-btn ghost sm" on:click={() => setSession({ activeTab: 'help' })}>Open Help &amp; Shortcuts</button>
        </div>

      {:else if section === 'updates'}
        <div class="rw-nv"><span class="rw-nvk">Installed version</span><span class="rw-nvv">{settingValue(appVersion, {
            loading: versionState === 'loading',
            missing: 'could not be read',
          })}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Environment</span><span class="rw-nvv">{environment}</span></div>
        <!-- The status of the CHANNEL, not the absence of news. This row used to
             read "up to date" whenever nothing was waiting — which was also what
             it said when the check had never run, when the laptop was offline,
             and when the update manifest had been returning 404 since the day
             Relay was installed. A badge that cannot detect its own failure
             (CLAUDE.md rule 35), on the one path by which a fix reaches a church
             that already has Relay. `describeChannel` is the ONE place a check
             outcome becomes words, and both surfaces that talk about it — this
             row and the Overview rail's quick link — call it. -->
        <div class="rw-nv"><span class="rw-nvk">Update status</span><span class="rw-nvv" class:s-netbad={$updateChannel.state === 'failed'}>{describeChannel($updateChannel)}</span></div>
        {#if $updateChannel.state === 'failed'}
          <div class="rw-nv"><span class="rw-nvk">Last attempt</span><span class="rw-nvv">{$updateChannel.detail || 'no reason given'}</span></div>
        {/if}
        <div class="s-prose">
          <button class="r-btn primary sm" on:click={doCheckUpdates} disabled={checking}>
            {checking ? 'Checking…' : 'Check for Updates'}
          </button>
          {#if updateMsg}<p class="rw-foot">{updateMsg}</p>{/if}
        </div>

        <!-- WHAT AN UPDATE WOULD DO TO YOUR HISTORY.
             Shown before the operator presses anything, because the question they
             actually have — "is this safe right now?" — was previously answerable
             only by trying it. Nothing here refuses on its own; `update_begin`
             re-runs the same checks at the moment of truth. -->
        <div class="rw-group">Before an update</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            Relay copies your entire history — services, plans, songs, saved verses and
            templates — before it installs anything, and keeps the last
            {KEEP_SNAPSHOTS} copies. The app itself can always be reinstalled from a
            release page; your history cannot.
          </p>
          {#if updReady?.during_service}
            <p class="rw-foot"><b>A service is being recorded.</b> Relay will not update until it ends — an update restarts the app.</p>
          {/if}
          {#if $snapshotPath}
            <p class="rw-foot">Your history was copied to <span class="r-mono">{$snapshotPath}</span>.</p>
          {/if}
        </div>
        {#if updReady}
          {#each updReady.checks as c (c.id)}
            <div class="rw-nv">
              <span class="rw-nvk">{c.label}</span>
              <!-- `class:bad` / `class:warn` used to be here, and neither class was
                   ever defined in this file or in `app.css` — so a pre-update check
                   that FAILED was painted in exactly the same grey as one that
                   passed, on the screen whose entire job is to say whether an update
                   is safe right now. Rule 35, in a class attribute. Rose for a
                   failure and amethyst for "worth a look", matching `.b-check.warn`
                   in the boot ladder; never amber, which means ON AIR. -->
              <span class="rw-nvv" class:s-netbad={c.state === 'fail'} class:s-netwarn={c.state === 'warn'}>
                {c.note}
              </span>
            </div>
          {/each}
        {/if}

      {:else if section === 'diagnostics'}
        <!-- READINESS FIRST. The Dashboard is the boot ladder's own probes, re-run
             on demand — the same `freshChecks()` through the same `makeProbes()`,
             never a second health panel. It used to be its own rail entry called
             "Dashboard", which is a name for a shape rather than for a question;
             the question it answers is "is this machine going to work?", and that
             is what an operator opens Diagnostics to ask. -->
        <div class="s-dash"><Dashboard /></div>

        <div class="rw-group">Support facts</div>
        <!-- A FILE, NOT A SCREEN. This table has shown the right facts for a while
             and been useless for the job it exists for: nobody can email a screen.
             What actually happens is somebody photographs it, losing half the table
             and all of the latency history. -->
        <div class="s-prose">
          <button class="r-btn ghost sm" on:click={doExportDiagnostics} disabled={diagBusy}>
            {diagBusy ? 'Writing…' : 'Save a diagnostic file'}
          </button>
          {#if diagMsg}<p class="rw-foot" role="status">{diagMsg}</p>{/if}
        </div>
        <div class="rw-nv"><span class="rw-nvk">Backend</span><span class="rw-nvv">{$capture.available ? 'connected' : 'not connected'}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Speech model</span><span class="rw-nvv">{$capture.stt.loaded ? ($capture.stt.model || 'loaded') : 'not loaded'}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Recognition language</span><span class="rw-nvv">{settingValue($capture.stt.language, { missing: 'not set yet' })}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Microphone</span><span class="rw-nvv">{$capture.inputDevice || 'system default'}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Detection</span><span class="rw-nvv">{$capture.detectionOn ? 'armed' : 'off'}</span></div>
        <div class="rw-nv"><span class="rw-nvk">This machine (LAN)</span><span class="rw-nvv">{settingValue(lanIp, {
            loading: lanState === 'loading',
            missing: lanState === 'failed' ? 'could not be read' : 'not on a network',
          })}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Ports</span><span class="rw-nvv">5032 console · 8031 ws · 8032 http</span></div>
        <div class="rw-nv"><span class="rw-nvk">Version</span><span class="rw-nvv">{settingValue(appVersion, {
            loading: versionState === 'loading',
            missing: 'could not be read',
          })} · {environment}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Uptime (this run)</span><span class="rw-nvv">{uptime}</span></div>

        <div class="rw-group">Live latency</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            How long it takes a spoken word to reach the operator's screen, and a spoken reference to reach the wall — measured on <b>this</b> machine, in <b>this</b> room, on the model you are actually running. Milliseconds. Nothing here leaves the computer.
            <br /><br />
            The clock starts when audio reaches the speech engine. Assembling it from the microphone adds a further {lat?.capture_front_end_ms ?? 400}ms at most (about half that on average), and the end-to-end row already includes it.
          </p>
        </div>
        {#if latVerdict}
          <div class="rw-nv"><span class="rw-nvk">Verdict</span><span class="rw-nvv">{latVerdict.verdict}</span></div>
          <div class="s-prose"><p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">{latVerdict.detail}</p></div>
        {/if}
        {#if latRows.length}
          <div class="rw-nv"><span class="rw-nvk">measurement</span><span class="rw-nvv">n · median · P95 · P99 · worst</span></div>
          {#each latRows as m}
            <div class="rw-nv">
              <span class="rw-nvk">{m.metric.replace(/_/g, ' ')}</span>
              <!-- `?? 0` used to be here, and it rendered a stage that was never
                   reached as `0ms` — the fastest thing on the screen. That is the
                   absence-is-not-a-zero rule (DECISIONS §38, §44) failing at the
                   last hop, on the one screen a field tester reads. -->
              <span class="rw-nvv">{m.samples} · {msOrDash(m.p50_ms)} · {msOrDash(m.p95_ms)} · {msOrDash(m.p99_ms)} · {msOrDash(m.worst_ms)}</span>
            </div>
          {/each}
          <div class="rw-nv"><span class="rw-nvk">transcript updates / second</span><span class="rw-nvv">{(lat?.transcript_updates_per_s ?? 0).toFixed(2)}</span></div>
          <div class="rw-nv"><span class="rw-nvk">partials dropped (queue full)</span><span class="rw-nvv">{lat?.dropped_partials ?? 0}</span></div>
          <!-- RG-84. A shed PARTIAL is re-decoded a moment later; shed AUDIO is a
               piece of the sermon Relay never heard. Both queues in front of the
               decoder were unbounded — a stall became memory and a transcript
               minutes behind, rather than a number. Non-zero here is worse news
               than the row above it, so it is coloured and the row above is not. -->
          <div class="rw-nv"><span class="rw-nvk">audio dropped (never heard)</span><span class="rw-nvv" class:s-netbad={(lat?.dropped_audio ?? 0) > 0}>{lat?.dropped_audio ?? 0}</span></div>
        {:else}
          <div class="rw-nv"><span class="rw-nvk">Measured so far</span><span class="rw-nvv">{settingValue(null, { missing: 'nothing yet' })}</span></div>
        {/if}
        <div class="s-prose">
          {#if latDrift}
            <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
              {#if latDrift.growing}
                <b>Latency is growing.</b> It averaged {Math.round(latDrift.early)}ms early in this session and {Math.round(latDrift.late)}ms recently — the pipeline is falling further behind the longer it runs.
              {:else}
                Steady: {Math.round(latDrift.early)}ms early in this session, {Math.round(latDrift.late)}ms recently.
              {/if}
            </p>
          {/if}
          <div class="s-addrow">
            <button class="r-btn" on:click={resetLatency} disabled={!$capture.available}>Start a fresh measurement</button>
            <button
              class="r-btn"
              on:click={() => toggleLatency(!(lat?.enabled ?? true))}
              disabled={!$capture.available}
            >{(lat?.enabled ?? true) ? 'Stop measuring' : 'Start measuring'}</button>
          </div>
          <p class="rw-foot">Start listening and speak for a few seconds to fill the table. Measuring is on by default and costs a handful of timestamps per decode; turning it off is here so a field test can prove the instrument is not the delay.</p>
        </div>

      {:else if section === 'privacy'}
        <!-- WHAT IS LEAVING THIS MACHINE, ANSWERED FROM THE LIVE SETTINGS.
             Relay's privacy story is the strongest thing about it and it has been
             invisible — it lives in PRIVACY.md, which nobody in a booth reads. Every
             row below is read from the actual state, never hardcoded: a screen that
             says "off" because somebody typed "off" is worth less than no screen.
             It also states the LAN exposure plainly, because a privacy page that
             lists only the reassuring half is an advert.

             It is a REPORT: there is not one handler between here and the ADVANCED
             marker below, and `privacy.test.js` slices the file on exactly those two
             strings to hold it that way. The control that changes the one row that
             can change lives under the marker, once. -->
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            Read from this machine right now — not from a promise. Nothing above the
            rule is a setting you change; it is a report on the settings you have.
          </p>
        </div>
        <div class="rw-nv">
          <span class="rw-nvk">What you say</span>
          <span class="s-nvp">Never leaves this computer. Audio is processed in memory and is not written to disk.</span>
        </div>
        <div class="rw-nv">
          <span class="rw-nvk">Transcripts &amp; history</span>
          <span class="s-nvp">Stored on this computer only, in Relay's own database.</span>
        </div>
        <div class="rw-nv">
          <span class="rw-nvk">Speech recognition</span>
          <span class="s-nvp">
            {$capture.stt.loaded
              ? 'Runs entirely on this machine. No audio is sent anywhere.'
              : 'No model loaded — nothing is being transcribed.'}
          </span>
        </div>
        <div class="rw-nv">
          <span class="rw-nvk">Crash reporting</span>
          <!-- The live value. This is the ONE thing Relay can send, and the one
               row somebody opens this page to check. -->
          <span class="s-nvp" class:on={crashOn}>
            {crashOn
              ? 'ON — a crash sends the error and where it happened. Never a transcript, verse, lyric or announcement.'
              : 'OFF — nothing is sent when Relay crashes.'}
          </span>
        </div>
        <div class="rw-nv">
          <span class="rw-nvk">Accounts &amp; cloud</span>
          <span class="s-nvp">There are none. Relay has no account, no server, and works with the network unplugged. It is a single-operator, on-device app — no user accounts, roles or logins, by design.</span>
        </div>
        <div class="rw-nv">
          <span class="rw-nvk">Your church network</span>
          <!-- The unflattering half, in the same size type. -->
          <span class="s-nvp">
            Relay serves your screens at <span class="r-mono">{lanIp || 'this computer'}:8032</span>.
            Anyone already on the same WiFi can see what is on the projector — <b>and can
            change it</b>: the preacher's remote has no password, by design. They cannot
            reach your transcripts, plans or history.
          </span>
        </div>
        <div class="rw-nv">
          <span class="rw-nvk">Diagnostic file</span>
          <span class="s-nvp">Only written when you press the button in Diagnostics, and only where you can read it first.</span>
        </div>
        <!-- LICENCE. The one row of the deleted Overview rail that had no other
             home. It belongs on the report, not in an inspector: "what is this
             and what may I do with it" is the same question as the four rows
             above it, and the footnote under this list already gives the long
             answer. Still a read-only row — this section carries no handler
             above the ADVANCED marker (`privacy.test.js` slices there). -->
        <div class="rw-nv">
          <span class="rw-nvk">Licence</span>
          <span class="s-nvp">MIT — free and open source. Nothing to sign in to, nothing to pay, and no licence key that can expire on a Sunday morning.</span>
        </div>
        <div class="s-prose">
          <p class="rw-foot">
            The full account, including what would make the network tradeoff change, is
            in <span class="r-mono">PRIVACY.md</span> and <span class="r-mono">docs/DECISIONS.md</span> §35.
            Relay is free and open source, MIT licensed — there is nothing to sign in
            to and nothing to pay.
          </p>
        </div>

        <!-- ADVANCED · crash reporting. The one control on this section, and the
             boundary `privacy.test.js` slices on: everything above it is a report
             and must contain no handler at all. -->
        <div class="rw-group">Crash reporting</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            Relay is offline software: nothing you do here leaves this computer. Crash reporting is the one exception, and it is <b>off unless you turn it on</b>.
            <br /><br />
            If you turn it on, Relay sends only the technical details of a crash — the error, where in the code it happened, and your operating system. <b>Sermon transcripts, verse text, song lyrics, announcements and service names are never sent</b>, and are stripped from every report before it leaves. Reports are queued and sent later, so a bad network can never slow down a live service.
          </p>
          <label class="r-lbl" for="crash-dsn">Sentry DSN (your own project)</label>
          <input id="crash-dsn" class="r-input" type="text" placeholder="https://…@…ingest.sentry.io/…" bind:value={crash.dsn} disabled={!$capture.available} />
          <div class="s-addrow">
            <button class="r-btn" class:danger={crash.enabled} on:click={() => toggleCrash(!crash.enabled)} disabled={!$capture.available}>
              {crash.enabled ? 'Turn crash reporting off' : 'Turn crash reporting on'}
            </button>
          </div>
          {#if crashMsg}<p class="rw-foot">{crashMsg}</p>{/if}
        </div>
      {/if}
      </div>
    </main>

    <!-- THE OVERVIEW RAIL USED TO BE HERE, and it was a THIRD column.
         docs/REBRAND.md §11 is a rail plus ONE reading column; the prototype
         centres that column at 880px and has no inspector on this workspace,
         because a settings page has no "thing in hand" for an inspector to be
         about. What the rail actually carried was four rows and four links, and
         every one of them already had a home:

           · Version and Environment — the first two rows of Updates, verbatim.
           · Uptime — "Uptime (this run)" in Diagnostics, the same `uptime`.
           · Licence — now a row on Privacy & Advanced, which is the section
             that answers "what is this and what can it do with my data".
           · Keyboard shortcuts / Service history / Check for updates — three
             links to three rail entries six inches to their left.
           · Support & guide — the Help tab, which is on the tab bar, and which
             the Shortcuts section already offers a button to.

         So the column was a second copy of the page beside the page, and it was
         not a harmless one: its "Check for updates" card had to call
         `describeChannel` itself (RG-92), which is a second surface that can
         drift from the Updates row about whether a check ever succeeded. One
         door, once. Rule 35 gets easier the fewer places say the same thing.
    -->
</WorkspaceFrame>

<style>
  /* SETTINGS — the workspace grammar (`WorkspaceFrame.svelte`, docs/REBRAND.md
     §2 · §11): section rail · panel · overview rail, as three panes.

     ── ONE TYPE SCALE, THREE ROLES, AND THEY ARE THE FRAME'S ────────────────
     §11's roles are page title / standfirst / row, with a footnote behind a
     hairline. This file used to own private copies of all of them, and the copies
     had already multiplied: `.s-lead` was a second standfirst under the frame's,
     `.s-row`/`.s-rowtitle`/`.s-rownote`/`.s-rowctl` was one row grammar and
     `.s-netrow`/`.s-netk`/`.s-netv` was a SECOND, with different padding
     (10px vs 8px), a different key colour (`--v-txt` vs `--v-dim`) and a
     different bleed; `.s-grouphead` duplicated `.rw-group` line for line; and
     there were two footnotes, `.s-note` behind the hairline §11 asks for and
     `.s-tr-note` without one. That is not one scale with three roles. It is five
     roles and two of them are the same role rendered two ways, which is exactly
     the failure the frame's own comment names: a type scale defined three times
     is three type scales.

     So every role below comes from the frame — `.rw-nv`, `.rw-nvk`, `.rw-nvv`,
     `.rw-nvnote`, `.rw-nvctl`, `.rw-group`, `.rw-foot` — and what is left here is
     only what Settings alone has: a level meter, the threshold sliders, the
     translation list, the language table, the shortcut rows and the overview
     rail. Every size is a `--v-fs-*` token (it used to be eleven hand-picked
     pixel values; a scale with eleven steps is not a scale).

     ── SEAMS, NOT GUTTERS ───────────────────────────────────────────────────
     A settings section was a column of bordered cards with 12px trenches between
     them. On a booth laptop that trench is about a third of the screen spent
     saying "these two settings are not related", which is false — they are the
     same section. Rows bleed to the pane's edges with a hairline between them,
     which is how the running order and the screens list read, and gives the width
     back to the words. The panel itself now carries NO padding: rows reach the
     seam by default and only prose (`.s-prose`) takes a gutter, which is the same
     arrangement every other pane on the desk uses and one negative margin fewer
     than the old `margin:0 -14px` trick. */

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

  /* ── THE READING COLUMN ──────────────────────────────────────────────────
     §11 is a rail plus ONE column, and the prototype caps it at 880px and
     centres it. The cap is not decoration: a settings row is a name, a sentence
     of explanation and a control, and on a 1920px booth monitor an uncapped
     column puts 700px of nothing between the sentence and the switch that
     belongs to it. The prototype's own note records the first attempt — 760px
     LEFT-aligned — being wrong for the opposite reason: it left a third of every
     card empty on the right, which reads as an unfinished section rather than as
     a measured line. Centred at 880px is what it settled on. */
  .s-read{ width:100%; max-width:880px; justify-self:center; }

  /* ── ACTIVE PANEL ── */
  .s-panel{ min-width:0; display:flex; flex-direction:column; gap:0; }
  /* ROW HEIGHT (§11: `min-height:46px`). The frame's `.rw-nv` is padding-only, so
     a row whose name has no note under it collapsed to about 35px and a column
     mixing the two stepped in and out down the page. A floor, not a fixed height:
     a row with a note, a wrapped value or a two-line control still grows.
     Deliberately scoped to this panel rather than pushed into `WorkspaceFrame`,
     which four other workspaces share and which is not this agent's file —
     recorded in the wave note so the integrator can promote it if Planner,
     Outputs, Templates and Library want the same floor. */
  .s-panel :global(.rw-nv){ min-height:46px; }
  /* Anything that is not a row: a paragraph, a picker, a button, a table. It
     keeps the gutter the rows deliberately do not. */
  .s-prose{ padding:12px; min-width:0; }
  .s-prose > *{ margin-top:10px; }
  .s-prose > :first-child{ margin-top:0; }
  /* The footnote keeps its hairline and its own top padding; the margin is the
     block's job, so the two do not add up to a double gap. */
  .s-prose :global(.rw-foot){ margin-top:12px; max-width:80ch; }
  /* Dashboard and History bring their own layout and their own card padding. */
  .s-dash{ padding:12px; min-width:0; }
  .s-history{ padding:12px; min-width:0; }

  .s-inline{ display:flex; justify-content:flex-end; }

  /* ── ROLE 3 · the halves of a row this file adds to the frame's ──
     A name cell that carries an explanatory line under it, a value that is a
     SENTENCE rather than a figure, and a control pair on the right. */
  .s-nvtext{ min-width:0; }
  .s-nvpair{ display:flex; align-items:center; gap:6px; justify-content:flex-end; flex-wrap:wrap; }
  /* `.rw-nvv` is mono, right-aligned and tabular because a value is usually a
     figure. The Privacy report's values are prose, and mono prose right-aligned
     against a ragged left edge is unreadable — so a sentence-shaped value says
     so. Same row, same seam, same scale. */
  .s-nvp{ min-width:0; max-width:46ch; justify-self:end; text-align:left;
    font-size:var(--v-fs-cap); line-height:var(--v-lh-cap); color:var(--v-dim); }
  .s-nvp b{ color:var(--v-txt); font-weight:600; }
  /* The one row on the Privacy report that can say something is leaving. Emerald
     is "confirmed/connected" in the design system; here it marks the state that
     is ACTIVE, not the state that is good — the copy carries the judgement. */
  .s-nvp.on{ color:var(--v-emerald); }
  .s-sel{ width:190px; max-width:100%; }
  .s-lenctl{ display:flex; align-items:center; gap:8px; justify-content:flex-end; }
  .s-leninput{ width:90px; text-align:right; }
  .s-lenunit{ color:var(--v-faint); font-size:var(--v-fs-cap); }
  /* Safe mode ON is not a normal state: it is the whole application disarmed.
     Rose, never amber — amber means ON AIR and is never spent on anything else. */
  .s-armed{ color:var(--v-rose); }

  /* Voice profiles. `s-vpactive` marks the profile the gate is calibrated by —
     EMERALD, never amber: amber is spent only on air (CLAUDE.md / DECISIONS §22),
     and a selected profile is configuration, not something on a screen. */
  .s-vpactive{ margin-left:8px; padding:1px 6px; border-radius:var(--v-r-sm);
    font-size:var(--v-fs-cap); letter-spacing:.08em; text-transform:uppercase;
    color:var(--v-emerald); border:1px solid color-mix(in srgb, var(--v-emerald) 40%, transparent); }

  /* `.s-toggle` / `.s-knob` / `.s-soon` / `.s-dim` USED TO BE HERE — a private
     38×21 switch, pixel for pixel the same shape as `.r-switch` in `app.css`,
     plus the "Soon" chip that sat beside it. §12 asks for ONE instrument
     everywhere, and a second switch defined in a view file is how that stops
     being true quietly. Their only two users were the dead Auto-start and Tray
     rows; both are gone (DECISIONS §69), so the rules went with them rather than
     waiting to be copied. A switch Settings needs in future comes from
     `app.css`'s `.r-switch`, which is the one every other surface uses.

  /* A VALUE THAT IS BAD NEWS, and one that is worth a look. Rose and amethyst,
     matching `.b-check.warn` in the boot ladder; never amber, which means ON AIR
     and nothing else. */
  .s-netbad{ color:var(--v-rose); }
  .s-netwarn{ color:var(--v-amethyst2); }
  /* An error the operator must read now, rather than a footnote. */
  .s-alert{ margin-top:10px; font-size:var(--v-fs-cap); line-height:var(--v-lh-cap); color:var(--v-red); }

  /* A row of controls that belong together — a field and the button that commits
     it, or two buttons that are one decision. */
  .s-addrow{ display:flex; gap:8px; align-items:center; flex-wrap:wrap; }
  .s-addrow .r-input{ flex:1 1 200px; width:auto; min-width:0; }

  /* Shortcuts. `.s-scrow`/`.s-scnote` were a THIRD row grammar in this file
     (8px padding against the frame's 9px, its own key colour, its own seam); the
     rows are `.rw-nv` now and only the key caps are local. The caps sit in the
     control column, so they share the single right edge every other value on the
     page is aligned to. */
  .s-sckeys{ display:flex; gap:5px; flex-wrap:wrap; justify-content:flex-end; }
  .s-kbd{ font-family:var(--f-mono); font-size:var(--v-fs-mono); color:var(--v-txt); background:var(--v-void);
    border:1px solid var(--v-line2); border-bottom-width:2px; border-radius:var(--v-r-sm); padding:1px 6px; }

  /* level meter */
  .s-meterwrap{ min-width:0; }
  .s-meter{ height:6px; border-radius:var(--v-r-round); background:var(--v-surf3); overflow:hidden; }
  .s-meter i{ display:block; height:100%; border-radius:var(--v-r-round);
    background:linear-gradient(90deg,var(--v-accent),var(--v-accent2)); }
  .s-meter-scale{ display:flex; justify-content:space-between; margin-top:7px;
    font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.05em; color:var(--v-faint); }
  .s-listen{ display:flex; align-items:center; gap:14px; flex-wrap:wrap; }
  .s-rms{ font-family:var(--f-mono); font-size:var(--v-fs-mono); letter-spacing:.03em; color:var(--v-faint);
    display:inline-flex; align-items:center; gap:7px; }
  .s-rms.voice{ color:var(--v-emerald); }
  .s-dot{ width:7px; height:7px; border-radius:50%; background:var(--v-faint); }
  .s-dot.on{ background:var(--v-emerald); box-shadow:0 0 7px var(--v-emerald); }
  .s-count{ font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.05em; color:var(--v-faint); }

  /* sliders */
  .s-slider{ width:100%; }
  .s-slider-top{ display:flex; align-items:baseline; justify-content:space-between; margin-bottom:10px; }
  .s-slider-name{ color:var(--v-dim); font-size:var(--v-fs-b2); }
  .s-slider-val{ font-family:var(--f-mono); font-size:var(--v-fs-h1); font-weight:500; color:var(--v-accent);
    font-variant-numeric:tabular-nums; }
  .s-slider-ends{ display:flex; justify-content:space-between; margin-top:8px;
    font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.06em; text-transform:uppercase;
    color:var(--v-faint); }

  /* bible translations */
  .s-checklist{ display:flex; flex-direction:column; gap:0; }
  .s-check-code{ font-family:var(--f-mono); font-size:var(--v-fs-mono); font-weight:600; letter-spacing:.05em;
    color:var(--v-txt); }
  .s-tr{ display:flex; align-items:center; gap:11px; width:100%; text-align:left; cursor:pointer;
    background:transparent; border:0; border-bottom:1px solid var(--v-line); padding:8px 12px;
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

  /* network / model */
  .s-status{ display:inline-flex; align-items:center; gap:7px;
    font-family:var(--f-mono); font-size:var(--v-fs-mono); letter-spacing:.04em; }
  .s-status.ok{ color:var(--v-emerald); }
  .s-sdot{ width:7px; height:7px; border-radius:50%; background:currentColor; }
  .s-status.ok .s-sdot{ box-shadow:0 0 7px var(--v-emerald); }
  .s-modelpath{ font-family:var(--f-mono); font-size:var(--v-fs-cap); line-height:1.5;
    color:var(--v-faint); word-break:break-all; }

  /* languages */
  .s-lang{ width:100%; border-collapse:collapse; font-size:var(--v-fs-b2); }
  .s-lang th{ text-align:left; font-weight:500; font-size:var(--v-fs-cap); letter-spacing:.06em;
    text-transform:uppercase; color:var(--v-faint); padding:6px 8px;
    border-bottom:1px solid var(--v-line); }
  .s-lang td{ padding:7px 8px; border-bottom:1px solid var(--v-line2); color:var(--v-dim); }
  .s-langcode{ color:var(--v-faint); font-size:var(--v-fs-cap); }
  /* An absence is dim, not red: nobody has failed here — the work has not been
     done, and saying so is the whole point of the column. */
  .s-langgap{ color:var(--v-faint); font-style:italic; }

  /* ── responsive ── */
  /* The frame hides the inspector column below 1240px and stacks below 900px;
     these are the rules that are this workspace's own. A setting row that cannot
     hold its control beside its name puts the control underneath rather than
     crushing the sentence that explains what it does. */
  @media (max-width:820px){
    .s-panel :global(.rw-nv){ grid-template-columns:minmax(0,1fr); }
    .s-panel :global(.rw-nvctl){ justify-self:stretch; }
    .s-panel .s-nvp{ justify-self:stretch; max-width:none; }
    .s-panel .s-sel{ width:100%; }
    .s-panel .s-nvpair{ justify-content:flex-start; }
  }
</style>
