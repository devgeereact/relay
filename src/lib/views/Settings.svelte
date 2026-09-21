<script>
  import { onMount, onDestroy } from 'svelte';
  import Button from '../ui/Button.svelte';
  import { whyDisabled, ENGINE_OFF, SERVICE_LOCKED, MIC_LIVE, BUSY } from '../ui/whydisabled.js';
  // ── THE KIT, NOT A CLASS SOMEBODY REMEMBERED ──────────────────────────────
  //
  // This page carried NINETEEN raw `class="r-btn"` buttons against three that
  // went through `ui/Button`, and the three were the only ones that could tell a
  // disabled operator anything: `disabledReason` renders `title` AND
  // `aria-describedby`, and neither channel reaches everybody on its own —
  // `title` is invisible to a keyboard or screen-reader operator, `aria-describedby`
  // to a mouse. Sixteen controls on a settings page grey themselves out and say
  // nothing, and two of them are inside a service (unlock, and the walk-through's
  // own guard). A class is opt-in in a way a component is not, which is the whole
  // argument in `ui/Button.svelte`'s header.
  //
  // `ListState` is the same argument about a LIST. *empty ≠ loading ≠ error* was
  // hand-rolled three times here with three different wordings, and one of the
  // three had its branches in the wrong order — the translation list asked
  // `!dataLoaded` BEFORE `$readErrors.listTranslations`, so a read that failed
  // announced itself as still loading for the rest of the session. The precedence
  // is fixed once, in the component, which is the reason it exists.
  import ListState from '../ui/ListState.svelte';
  import { rangeFill } from '../rangefill.js';
  import { get } from 'svelte/store';
  import ModelSetup from '../ModelSetup.svelte';
  // The shared workspace grammar (docs/REBRAND.md §2 · §11) — the same columns,
  // panes, type roles and name/value row the Planner and Outputs desks use.
  import WorkspaceFrame from './WorkspaceFrame.svelte';
  import Dashboard from './Dashboard.svelte';
  import { locale, setLocale, LOCALES, t } from '../i18n.js';
  import { restartSetup, setSession, session } from '../session.js';
  // THE shortcut table — one array, shared with the keydown handler, the
  // cheatsheet, Help and `sectionkeys.js::RESERVED`. See the note further down.
  import { SHORTCUTS } from '../shortcuts.js';
  import { humanError } from '../errors.js';
  import { settingValue, CHECKING } from '../settingvalue.js';
  import { safeMode } from '../boot/boot.js';
  import { checkForUpdate, updateAvailable, updateChannel, describeChannel } from '../updater.js';
  import {
    applySafeMode,
    safeModeError,
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
  import { capture, meter, initAudio, startCapture, stopCapture, setSensitivity, getSensitivity, setSttLanguage, setInputDevice, getBuildMarker, listTranslations, getActiveTranslation, setActiveTranslation, importTranslation, deleteTranslation, fileToBase64, localIp, getCrashReporting, setCrashReporting, serviceTargetMinutes, loadServiceTarget, setServiceTarget, countdownWarnMs, loadCountdownWarnMs, setCountdownWarnMs, latencyReport, latencyReset, latencySetEnabled, serviceLock, loadServiceLock, setServiceLock, rooms, loadRooms, saveRoom, useRoom, deleteRoom,
    listOutputChannels, setChannelDisplay, activeVoiceProfile, languageReport, exportDiagnostics, readErrors,
    demoStatus, loadDemoContent, removeDemoContent } from '../stores/capture.js';
  // `Loading` and `ErrorState` are no longer imported HERE and that is the point
  // of the change, not an oversight: every list on this page renders them through
  // `ListState`, which is the only thing that knows the precedence between them
  // and empty. A view that imports the three separately is a view choosing the
  // order again, and two of the five on this page chose it wrongly.
  import { captureRoom, observedNote, applyRoom, describeApply } from '../rooms.js';
  import { describeGate } from '../gate.js';
  import { snapshotPath, KEEP_SNAPSHOTS } from '../updater.js';
  import { diagnose, drift } from '../latency.js';

  // ─────────────────────────────────────────────────────────────────────────
  // SECTION NAV — EIGHT SECTIONS, ORDERED BY HOW OFTEN AN OPERATOR NEEDS THEM.
  //
  // It was eighteen, then eleven (docs/REBRAND.md §11), and the eleven were
  // organised by TAXONOMY — General, Screens, Audio, AI, Scripture, Network,
  // History, Shortcuts, Updates, Diagnostics, Privacy. That is a filing system,
  // and it is the wrong one, because an operator does not arrive with a category.
  // They arrive with a moment: it is 10:20, the service is at 11:00, and the
  // question is whether this machine is going to work.
  //
  // Measured on the eleven-section rail: the readiness screen and the path check
  // — the only every-Sunday surfaces in the workspace — were at the BOTTOM of the
  // tenth section, while ten setup-only controls (the console language, the
  // countdown warning, the demo content, the walk-through, the key table) sat in
  // the first two. The rail ran in almost exactly the wrong order.
  //
  // So the order is FREQUENCY, and each section is named for the moment rather
  // than the category:
  //
  //   1 Before the service — every Sunday, and the only section that is.
  //   2 This room          — occasionally: a new hall, a new microphone.
  //   3 Preachers          — occasionally: a new voice to calibrate for.
  //   4 Scripture          — rarely.
  //   5 This machine       — rarely, and mostly when something is wrong.
  //   6 Updates            — rarely.
  //   7 Privacy            — rarely, and read rather than changed.
  //   8 Getting started    — once, in the first week.
  //
  // THREE SECTIONS WENT AND NONE OF THEM TOOK A CONTROL WITH IT:
  //
  //   · `screens` — every row was a copy of Outputs. The content-look pickers are
  //     the same five selects as `Channels.svelte`'s editable matrix (one writer,
  //     `setContentTemplate`, DECISIONS §25 · §70) and the screens list was a
  //     read-only restatement of the `followers` that matrix computes beside them.
  //     A second surface onto one store is how two surfaces come to disagree.
  //   · `history` — History is not a setting. It is a record browser with a
  //     destructive erase in it, and it is a route of its own now (App.svelte).
  //     What it shared the section with — the walk-through, the demo content, the
  //     service lock — moved to the section that matches when they are used.
  //   · `general` — with safe mode promoted to Before the service and service
  //     length moved to This room, it held one switch and a paragraph, which is
  //     not a section.
  //
  // Two sections were deleted in the previous pass for the same reason and stay
  // deleted: `dashboard` (the readiness surface is section ONE now, not a rail
  // entry named for a shape) and `account` (three rows, all duplicates).
  //
  // The `desc` is role two of §11's three: the STANDFIRST, one sentence about
  // what the section is for. It is rendered once, by the frame, and no section
  // repeats it a size smaller at the top of its own panel.
  // ─────────────────────────────────────────────────────────────────────────
  const SECTIONS = [
    { key: 'ready',     label: 'Before the service', desc: 'Is this machine going to work, and the two settings that decide how well it hears.', icon: 'ready' },
    { key: 'room',      label: 'This room',          desc: 'The microphone, where video sound goes, how long the service runs, and the rooms you save.', icon: 'mic' },
    { key: 'preachers', label: 'Preachers',          desc: 'What the gate lets through, and the calibration Relay keeps for each voice.', icon: 'sparkle' },
    { key: 'scripture', label: 'Scripture',          desc: 'The translation Relay reads from, and what it honestly knows in each language.', icon: 'book' },
    { key: 'machine',   label: 'This machine',       desc: 'Addresses, other software, the facts a support request needs, and what the pipeline is measuring.', icon: 'terminal' },
    { key: 'updates',   label: 'Updates',            desc: 'This build, the update channel, and what an update would do to your history.', icon: 'refresh' },
    { key: 'privacy',   label: 'Privacy',            desc: 'What is on this machine, what can leave it, and the one thing that does.', icon: 'shield' },
    { key: 'start',     label: 'Getting started',    desc: 'The walk-through, sample content, the keys, and the two numbers a new install sets once.', icon: 'flag' },
  ];
  let section = 'ready';
  // ── A CONTROL THAT POINTED HERE MAY SAY WHERE ─────────────────────────────
  //
  // Eight sections behind one tab, and every control that meant one of them
  // could only say "Settings". `Change sensitivity in Settings` named a control
  // and landed on whatever section happened to be first.
  //
  // One-shot, and cleared as soon as it is used: `session.settingsSection` is an
  // instruction from the control that was just pressed, not a resume point. An
  // operator who opens Settings themselves should land where they left it.
  // Checked against `SECTIONS` rather than trusted, because a stale key persisted
  // from an older layout would otherwise render an empty pane.
  onMount(() => {
    const want = get(session)?.settingsSection;
    if (want && SECTIONS.some((s) => s.key === want)) section = want;
    if (want) setSession({ settingsSection: null });
  });
  $: activeSection = SECTIONS.find((s) => s.key === section) ?? SECTIONS[0];

  // One per section, and no more: an icon table with entries nothing renders is
  // the same dead weight as a class nothing wears. `gear`, `monitor`, `clock`,
  // `keyboard` and `nodes` went with the sections that used them.
  const ICONS = {
    // A tick inside a circle — the readiness verdict, which is what section one is.
    ready: '<circle cx="12" cy="12" r="9"/><path d="M8 12.5l2.6 2.6L16 9.5"/>',
    mic: '<path d="M12 2a3 3 0 0 0-3 3v6a3 3 0 0 0 6 0V5a3 3 0 0 0-3-3z"/><path d="M19 10v1a7 7 0 0 1-14 0v-1"/><line x1="12" y1="18" x2="12" y2="22"/>',
    sparkle: '<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8L12 3Z"/>',
    book: '<path d="M4 4.5A2.5 2.5 0 0 1 6.5 2H20v15H6.5A2.5 2.5 0 0 0 4 19.5z"/><path d="M4 19.5A2.5 2.5 0 0 0 6.5 22H20"/>',
    terminal: '<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M7 9l3 3-3 3M13 15h4"/>',
    refresh: '<path d="M23 4v6h-6M1 20v-6h6"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>',
    shield: '<path d="M12 3l7 3v6c0 4-3 7-7 9-4-2-7-5-7-9V6l7-3Z"/>',
    // A flag on a pole — the first week, not a category.
    flag: '<path d="M5 22V3"/><path d="M5 4h11l-2 3.5L16 11H5z"/>',
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
  // ── THE DSN NEEDS A COMMIT OF ITS OWN ──────────────────────────────────────
  //
  // `setCrashReporting` had exactly one caller, the switch. With the switch
  // already ON, editing the DSN wrote only this local object: the field showed
  // the new address, the engine went on reporting to the old one, and leaving
  // Settings and coming back re-read `get_crash_reporting` and silently put the
  // old address back. The one control in Relay that decides where data leaves
  // this machine could be changed and keep pointing somewhere else.
  //
  // A SAVE BUTTON, not commit-on-blur. Blur fires on any focus change, so a
  // half-typed or mis-pasted address would become the live destination with no
  // moment at which the operator said so — and crash reports that have gone to
  // the wrong endpoint cannot be recalled. The button's own failure mode is an
  // edit that is never saved, and that one can be made VISIBLE: `dsnDirty` marks
  // the field as unsaved, so "I typed it and nothing happened" is on the screen
  // rather than discovered a week later.
  let savedDsn = '';
  let dsnBusy = false;
  $: dsnDirty = (crash.dsn ?? '').trim() !== savedDsn.trim();
  // The Privacy screen reads the LIVE value, never a literal. A page that says
  // "off" because somebody typed "off" is worth less than no page at all — it is
  // the one row a person opens it to check.
  $: crashOn = !!crash.enabled;
  /** Take whatever the backend LANDED on, never what was asked for (rule 15). */
  function acceptCrash(landed) {
    // Guarded on both halves. `$: crashOn = !!crash.enabled` runs on every
    // assignment, so a null here takes the whole section down rather than showing
    // a wrong word.
    crash = landed ?? { enabled: false, dsn: '' };
    savedDsn = landed?.dsn ?? '';
  }
  // A READ THAT FAILED IS NOT AN EMPTY ADDRESS.
  //
  // `getCrashReporting` is GROUP 2 and its safe default is `{ enabled:false, dsn:'' }`
  // — so a failed read on mount put `savedDsn = ''` on this page, and flipping the
  // switch would then send `('', true)`. `set_crash_reporting` writes the string
  // unconditionally, so the stored DSN was DESTROYED by a control the operator
  // pressed to turn reporting on. Nothing leaked (`telemetry::enable` returns early
  // on an empty DSN), and losing the one address a church configured is bad enough.
  // While the reason is recorded, both writing controls stand down and say why.
  $: crashReadFailed = $readErrors.getCrashReporting ?? null;
  async function toggleCrash(enabled) {
    crashMsg = '';
    // THE DRAFT IS THE OPERATOR'S, AND A SWITCH IS NOT A DISCARD.
    // `acceptCrash` takes the whole landed object, `crash.dsn` included, so a flip
    // silently replaced a half-typed address with the saved one — and took the
    // "Not saved yet" mark with it, which is the instrument that exists so an edit
    // that went nowhere is visible rather than discovered a week later.
    const draft = crash.dsn ?? '';
    try {
      // `savedDsn`, NOT `crash.dsn`. The switch is the operator saying yes to
      // WHETHER, and it is not their yes to WHERE. Sending the bound field here
      // let a half-typed address that the page was calling "not saved yet" become
      // the live destination the moment the switch was flipped — `set_crash_reporting`
      // persists the string and calls `telemetry::enable` on it in the same breath,
      // and reports already sent cannot be recalled. Changing the address is
      // `saveDsn`'s job and has its own button.
      acceptCrash(await setCrashReporting(enabled, savedDsn));
      // Put the draft back where the operator left it. `savedDsn` is untouched, so
      // `dsnDirty` re-marks it and Save address is still the only way to commit it.
      if (draft.trim() !== savedDsn.trim()) crash = { ...crash, dsn: draft };
      crashMsg = crash.enabled
        ? 'Crash reporting on.'
        : enabled
          ? 'Add a Sentry DSN above to turn this on.'
          : 'Crash reporting off.';
    } catch (e) {
      crashMsg = humanError(e);
    }
  }
  /** Commit the address WITHOUT changing whether reporting is on. */
  async function saveDsn() {
    dsnBusy = true;
    crashMsg = '';
    try {
      acceptCrash(await setCrashReporting(crashOn, crash.dsn));
      crashMsg = savedDsn
        ? crash.enabled
          ? 'Saved. Crash reports now go to that address.'
          : 'Saved. Crash reporting is still off — the switch below turns it on.'
        : 'The address was cleared. Crash reporting cannot run without one.';
    } catch (e) {
      crashMsg = humanError(e);
    }
    dsnBusy = false;
  }

  // ── SCREENS & LOOKS WAS A SECTION HERE AND IS NOT ANY MORE ────────────────
  //
  // It carried the five content-look selects and a read-only list of which screens
  // follow them, plus `contentTypes`, `pickCt`, `screens`, `screensState`,
  // `loadScreens` and `screenLook`. Every row of it was a second surface onto
  // something the Outputs workspace already renders: `Channels.svelte` has the
  // same five selects in an EDITABLE matrix beside the screens they affect, and it
  // computes `followers` from the same `list_output_channels` this copy re-read on
  // mount. The writer never lived here at all — `setContentTemplate` is the ONE
  // writer (DECISIONS §25 · §70) and Outputs and the template editor both call it.
  //
  // Two surfaces onto one store is how two surfaces come to disagree, and this one
  // was the weaker of the two by construction: the map and the screens it reaches
  // were in different places on this page, which is exactly the question the
  // matrix answers by putting them in one grid. Deleting the section costs nothing
  // an operator could do here and removes a copy that could only ever go stale.
  //
  // The imports went with it — `CONTENT_KINDS`, `contentTemplates`,
  // `setContentTemplate`, `loadContentTemplates`, `templates`, `loadTemplates` —
  // because a store subscribed to by a surface that no longer renders it is a read
  // nobody can see the result of.

  // ── THE SENSITIVITY DIAL, AND THE SAME ONE THE DOCK HAS ───────────────────
  //
  // This section used to carry a SECOND control over the gate: two sliders,
  // `Auto-fire above` and `Suggest above`, fighting the dial on Live in four
  // separate ways (DECISIONS §96). They pointed the opposite way — right was
  // stricter here and right is more eager on the dial. They reached 0.99, which
  // the dial cannot express at all. `to_sensitivity` reads `auto_fire` alone, so
  // an independently-set `suggest` died the moment anybody nudged the dial. And
  // `Suggest above` was labelled HYPER-AWARE at the end where Relay makes the
  // FEWEST suggestions.
  //
  // None of that is fixable by syncing the two, because every sync makes one of
  // them lie about what the operator just did. So the dial is the only gate
  // control, exactly as DECISIONS §26 named it and as the self-calibration already
  // assumed — the anchor it decays toward is the dial position, not a pair of
  // hand-set bars.
  //
  // WHAT MAKES THIS THE SAME CONTROL and not a copy of it: one store
  // (`$capture.sensitivity`), one command (`setSensitivity` → `set_sensitivity` →
  // `apply_thresholds`), and `detection://thresholds` moving both surfaces the
  // moment either moves. Two doors onto one control, like the microphone picker
  // this page shares with the dock. A component-local copy of the number would be
  // the defect the event itself was added to end.
  //
  // A DIAL THAT DID NOT TAKE MUST SAY SO. `setSensitivity` THROWS (contract group
  // 1) precisely so this branch can exist: on failure the store is unchanged, so
  // the bound value is unchanged, so Svelte never rewrites the DOM property and
  // the thumb would stay exactly where it was dragged over a gate that had not
  // moved. `pending` is the thumb while the engine is being asked, and only that.
  let gateErr = '';
  let gatePending = null;
  $: gateDial = gatePending ?? $capture.sensitivity;
  // The ONE place that decides what this page may say about the gate — shared with
  // every other surface that shows it, so they cannot form separate opinions.
  $: gate = describeGate($capture);
  async function onSensitivity(v) {
    gatePending = v;
    gateErr = '';
    try {
      await setSensitivity(v);
      gatePending = null; // the store now holds what actually landed
    } catch (e) {
      gatePending = null;
      try {
        await getSensitivity();
      } catch {
        /* the dial is already disabled in this case */
      }
      gateErr = humanError(e);
    }
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
  // `asked` is the third fact an array cannot carry: a store that starts `[]` reads
  // the same before the answer as after an empty one, and `loadRooms` swallows to
  // `[]` on failure. `ListState` treats a null `items` as pending, but this is a
  // STORE and is never null, so the flag is what tells it which of the three.
  let roomsAsked = false;
  onMount(async () => {
    try {
      await loadRooms();
    } finally {
      roomsAsked = true;
    }
  });

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

  // ─────────────────────────────────────────────────────────────────────────
  // DEMO CONTENT — a sample service to press, and one action that takes it back.
  //
  // It lives on Getting started, directly under the setup walk-through, because the
  // two answer one question from opposite ends: the walk-through proves the MACHINE
  // works, and the demo set gives somebody something to run on it. Both are aimed at
  // an operator in their first week and neither is ever pressed again, which is what
  // makes that their section.
  //
  // It used to sit on History & Backup, on the argument that that section owned the
  // DATABASE as a thing an operator manages. That reading was defensible and it was
  // the wrong axis: a section is a moment, not a table. History is its own route
  // now, and "load me a sample Sunday" is not something anyone does twice.
  //
  // It is deliberately NOT in the Library or the Planner. Those are where a church's
  // own content lives, and a "load sample content" button among their songs is a
  // mis-click away from an import they did not want on a Sunday morning.
  // The plan's title, quoted so the operator knows what to look for in the Planner.
  // Rust owns it (`db::demo::PLAN_TITLE`); `demo.test.js` asserts the two agree, so
  // a rename on one side cannot leave this sentence pointing at nothing.
  const DEMO_PLAN_TITLE = 'Demo · Sunday Morning Service';
  let demo = { loaded: false, total: 0, edited: 0, groups: [] };
  let demoBusy = false;
  let demoErr = '';
  let demoNote = '';
  // Two-step removal, in-app. `confirm()` returns false in this webview without
  // ever showing a dialog, so a guard built on one guards nothing (rule 41).
  let demoArmed = false;

  async function refreshDemo() {
    demo = await demoStatus();
  }
  // Polled when the section opens, not on mount: the panel is only ever read here,
  // and a fresh install has nothing for it to say.
  $: if (section === 'start') refreshDemo();

  async function doLoadDemo() {
    demoBusy = true;
    demoErr = '';
    demoNote = '';
    try {
      demo = await loadDemoContent(new Date().toISOString().slice(0, 10));
      demoNote = `Loaded ${demo.total} demo items. Open Planner to find “${DEMO_PLAN_TITLE}”.`;
    } catch (e) {
      // GROUP 1 throws, and a bulk write that failed in silence would leave the
      // operator pressing a button and watching nothing appear.
      demoErr = humanError(e);
    } finally {
      demoBusy = false;
    }
  }

  async function doRemoveDemo() {
    demoBusy = true;
    demoErr = '';
    demoNote = '';
    try {
      const gone = await removeDemoContent();
      // WHAT WAS KEPT IS SAID OUT LOUD. A demo item the operator edited is theirs
      // now and survives the removal; if nothing says so, they find an orphan in
      // their Library weeks later and cannot account for it.
      demoNote =
        gone.kept > 0
          ? `Removed ${gone.removed} demo items. ${gone.kept} you had changed ${gone.kept === 1 ? 'was' : 'were'} kept — ${gone.kept === 1 ? 'it is' : 'they are'} yours now, and can be deleted from the Library.`
          : `Removed ${gone.removed} demo items.`;
      demoArmed = false;
      await refreshDemo();
    } catch (e) {
      demoErr = humanError(e);
    } finally {
      demoBusy = false;
    }
  }

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
  // WHAT THE BUTTON FOUND. Pressing *Detect speakers* used to re-render this
  // block pixel for pixel in three different situations — the operator declined
  // the prompt, the machine has no input device to ask about, and it worked and
  // this computer genuinely has one speaker — because `ensureDeviceAccess`
  // returned a bare `false` for the first two and the list stayed empty for the
  // third. `outMsg` is what happened, `outMsgBad` whether that is a refusal
  // (rose) or merely news (the ordinary foot colour). Rose, never amber: nothing
  // on this page is on air.
  let outMsg = '';
  let outMsgBad = false;
  /** Unlock real speaker names by tripping the media permission once. */
  async function detectSpeakers() {
    outBusy = true;
    outMsg = '';
    outMsgBad = false;
    try {
      const access = await ensureDeviceAccess();
      await refreshOutputs();
      if (access.ok) {
        // Granted. Whether anything NEW appeared is the second question, and it
        // is the one the operator actually pressed the button to settle.
        outMsg = outDevices.length
          ? `Found ${outDevices.length} speaker${outDevices.length === 1 ? '' : 's'} besides the system default.`
          : 'Microphone access granted — this computer reports no speakers besides the system default. There is nothing more to choose, and video sound is already going to the right place.';
      } else if (access.reason === 'denied') {
        outMsgBad = true;
        // The way back is the point. A refusal that only says "refused" leaves an
        // operator with a dead button and no next action.
        // BOTH PLATFORMS. Relay ships Windows from day one, and this is new copy
        // whose entire job is telling the operator where to go — a macOS-only
        // path sends half of them to a menu that does not exist.
        outMsg =
          'Microphone access was refused, so the speaker names stay hidden. Turn Relay back on — on macOS in System Settings → Privacy & Security → Microphone, on Windows in Settings → Privacy & security → Microphone — then press Detect speakers again.';
      } else if (access.reason === 'no-input') {
        outMsg =
          'This computer has no microphone to ask about, so the speaker names stay hidden. Plug an input in, or leave video sound on the system default.';
      } else if (access.reason === 'unsupported') {
        outMsg = 'This webview cannot ask for microphone access at all.';
      } else {
        outMsgBad = true;
        outMsg = 'Asking for microphone access failed, and the reason was not one Relay recognises.';
      }
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

  let profilesAsked = false;
  async function refreshProfiles() {
    try {
      profiles = await listVoiceProfiles();
    } finally {
      profilesAsked = true;
    }
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
  // ── THE EDITOR OPENS ON THE ROW AS IT IS NOW ──────────────────────────────
  //
  // `profiles` was loaded on mount and refreshed only inside `profileAction`.
  // `onSensitivity` moves the gate and, through `apply_thresholds`, rewrites the
  // active profile's row — and it does not refresh. (It was `onAuto`/`onSuggest`,
  // the two sliders §96 deleted; the trap is the dial's as much as it was theirs.) So: drag the slider at the
  // top of this page, scroll down, press Edit, press Save profile, and
  // `update_voice_profile` compares the STALE figure against the already-updated
  // row, concludes the dial moved, and re-derives the gate from it. The operator's
  // change of thirty seconds earlier, on this same screen, is silently reverted.
  //
  // One await closes it. The editor is a working copy of a row, so the only
  // question is which row — and the answer must be the current one.
  const openEditor = async (p) => {
    try {
      await refreshProfiles();
    } catch {
      /* fall back to what is already loaded rather than refusing to open */
    }
    editing = { ...(profiles.find((r) => r.id === p.id) ?? p) };
  };

  // ── Recognition language (RG-138) ───────────────────────────────────────────
  //
  // TWO TABS, ONE FACT. This control lives in Scripture & Languages; the voice
  // profile editor above lives in AI & Detection, and its Language select is the
  // same column. There is exactly one store — `voice_profiles.language` — and
  // `set_stt_language` writes to whichever profile is ACTIVE, which is why the
  // line under the select names it rather than leaving the operator to find out.
  //
  // `refreshProfiles()` afterwards is not cosmetic: `profiles` is what the editor
  // opens a copy FROM, so a stale copy saved later would quietly put the old
  // language back.
  let langErr = '';
  let langBusy = false;
  $: activeProfile = profiles.find((p) => p.is_active) ?? null;

  async function pickLanguage(code) {
    langBusy = true;
    langErr = '';
    try {
      const landed = await setSttLanguage(code);
      await refreshProfiles();
      // AND THE OPEN EDITOR FOLLOWS THE FACT — the comment above claims this and
      // `refreshProfiles()` alone does not deliver it.
      //
      // The sections of this page are `{#if}` branches of ONE component, so
      // `editing` — a working copy taken by `openEditor` — survives a walk to
      // Scripture & Languages and back. An operator who opened the editor to nudge
      // sensitivity, came here, pinned Yoruba, went back and pressed Save
      // calibration sent the STALE language: `update_voice_profile` wrote it and
      // called `apply_profile`, so the database AND the live engine silently
      // reverted, and nothing said a word. Pinning the language by hand is the
      // RG-116 mitigation; a save that quietly undoes it is rule 35.
      //
      // Only the LANGUAGE, and only on the profile the backend says it wrote to.
      // Re-reading the whole row would throw away the unsaved edits the operator
      // came back for, which is the opposite mistake.
      const wrote = landed?.id ?? profiles.find((p) => p.is_active)?.id ?? null;
      if (editing && wrote !== null && editing.id === wrote)
        editing = { ...editing, language: code ?? null };
    } catch (e) {
      // GROUP 1 — it throws, and this is the surface that has to say so. Without
      // this the select would sit on a language nothing was told about.
      langErr = humanError(e);
    } finally {
      langBusy = false;
    }
  }

  // RMS on speech sits well below 1.0; scale so normal talking fills the meter.
  $: levelPct = Math.min(100, Math.round($meter.level * 320));

  // Real translations from the corpus + which one to read from.
  let translations = [];
  /** Named so the ErrorState's "Try again" can re-ASK. A retry wired to anything
      other than the original read is a button that cannot work (rule 35). */
  const loadTranslations = async () => (translations = (await listTranslations()) ?? []);
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
  /**
   * Is the pipeline measuring itself — TRUE, FALSE, or NULL for "nobody has
   * asked yet".
   *
   * The control this replaced read `lat?.enabled ?? true`, which printed
   * *measuring* over a backend that had never answered: the same words a
   * healthy, measuring pipeline shows, which is rule 35's defect exactly. The
   * report's own `enabled` is a boolean when it has been read and the report is
   * `null` until then, so the three cases are already distinct at the source —
   * they were being collapsed on the way to the screen, not at it.
   */
  $: latMeasuring = lat ? !!lat.enabled : null;
  async function refreshLatency() {
    lat = await latencyReport(0);
  }
  // Start and stop with the section, not with the component.
  $: if (section === 'machine') startLatencyPoll();
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
  // WHICH BUILD (S13). A version is every build of a branch; this is the commit.
  let buildMarker = '';
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
      // The button used to compose its own sentence from ch.state here — the
      // exact second surface rule 35 warns about, and the one that kept printing
      // "You're on the latest version." about a refusal (mid-service) that never
      // asked the server anything. `describeChannel` is the ONE place a channel
      // state becomes words; the button goes through it too, same as the row.
      updateMsg = v ? `Relay ${v} is available.` : describeChannel(get(updateChannel));
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
  // Also loaded at the shell (App.svelte), because the dock reads the same rule
  // on every tab. Re-read here so this page shows what is in the row rather than
  // what this session happens to be holding.
  onMount(loadCountdownWarnMs);
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
      await loadTranslations();
      activeTranslation = await getActiveTranslation();
      acceptCrash(await getCrashReporting());
    } catch (e) {
      crashMsg = humanError(e);
    } finally {
      dataLoaded = true; // distinguish "loading" from a genuinely empty list
    }
    buildMarker = await getBuildMarker();
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

  // ── IMPORT A BIBLE (RG-50 option two, DECISIONS §113) ────────────────────
  // A licensed text cannot ship inside Relay, so a church brings its own file in
  // the KJV's shape. The name and abbreviation are typed here, not guessed from
  // the filename: what the list says is what the operator called it.
  const BUNDLED = ['KJV', 'BSB'];
  const isBundled = (tr) => BUNDLED.includes(tr.abbreviation);
  let trFileInput;
  let trName = '';
  let trAbbr = '';
  let trLang = 'en';
  let trBusy = false;
  let trMsg = '';
  let trDeleteArmed = null;
  async function onBibleFile(e) {
    const file = e.target?.files?.[0];
    if (trFileInput) trFileInput.value = '';
    if (!file) return;
    trMsg = '';
    if (!trName.trim() || !trAbbr.trim()) {
      trMsg = 'Type the translation\u2019s name and its short code first, then choose the file.';
      return;
    }
    trBusy = true;
    try {
      const r = await importTranslation({
        name: trName.trim(),
        abbreviation: trAbbr.trim(),
        language: trLang.trim() || 'en',
        licenseType: 'licensed',
        filename: file.name,
        dataB64: await fileToBase64(file),
      });
      trMsg = r.replaced
        ? `Replaced ${trAbbr.trim().toUpperCase()} with ${r.verses.toLocaleString()} verses.`
        : `Imported ${trAbbr.trim().toUpperCase()}: ${r.verses.toLocaleString()} verses. Pick it above to read from it.`;
      trName = ''; trAbbr = '';
      await loadTranslations();
    } catch (err) {
      trMsg = humanError(err);
    }
    trBusy = false;
  }
  async function doDeleteTranslation(tr) {
    if (trDeleteArmed !== tr.id) { trDeleteArmed = tr.id; return; }
    trDeleteArmed = null;
    trMsg = '';
    try {
      await deleteTranslation(tr.id);
      trMsg = `Removed ${tr.abbreviation}.`;
      await loadTranslations();
    } catch (err) {
      trMsg = humanError(err);
    }
  }

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
  columns="var(--v-rail) minmax(0,1fr)">
    <!-- ════ SECTION RAIL ════ -->
    <aside class="rw-pane">
      <div class="rw-panehead">
        <h2 class="rw-panettl">Sections</h2>
        <span class="rw-spring"></span>
        <span class="rw-itemn">{SECTIONS.length}</span>
      </div>
      <nav class="rw-panebody s-railnav">
        {#each SECTIONS as s}
          <!-- THE SHARED RAIL ROW, not a copy of it. This wore `.s-railbtn`, a
               hand-typed duplicate of `WorkspaceFrame`'s `.rw-item` that agreed
               with it on everything except the two numbers nobody re-reads: a
               32px row against the shared 34, and a 10px gap against 9. The two
               rails sit in the same chrome at the same place on screen, so a
               reader comparing Settings with Planner sees a 2px step and cannot
               name it. Keeping the icon's own class is the whole legitimate
               override — the row is the frame's. -->
          <button
            class="rw-item r-focus"
            class:on={section === s.key}
            aria-pressed={section === s.key}
            on:click={() => (section = s.key)}
          >
            <svg class="s-railic" viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">{@html ICONS[s.icon]}</svg>
            <span class="rw-itemname">{s.label}</span>
          </button>
        {/each}
      </nav>
    </aside>

    <!-- ════ ACTIVE PANEL ════ -->
    <main class="rw-pane s-read">
      <div class="rw-panebody s-panel">

      {#if section === 'ready'}
        <!-- SECTION ONE, AND THE ONLY ONE ANYBODY OPENS EVERY SUNDAY.
             `Dashboard.svelte` is the boot ladder's own probes re-run on demand —
             the same `freshChecks()` through the same `makeProbes()`, never a second
             health panel — and it sat at the BOTTOM of the tenth of eleven sections.
             It is the section now, not a card inside one: the verdict, the twenty-odd
             checks, and the path check that asks the question the checks cannot (do
             the parts work TOGETHER?).

             The two settings under it are here because this is the only moment they
             can still be changed. `select_stt_model` is guarded by the service lock,
             so once the microphone opens the choice is made; and RG-116 measured what
             the choice is worth — three of three auto-fires correct on `turbo` against
             five of nine on `base`, in one morning, after the model was changed
             mid-service. The readiness hero has NAMED both since RG-116 and the
             controls to change them were five sections away, in Network and in
             Scripture. Naming a fact beside no way to act on it is half a screen. -->
        <div class="s-dash"><Dashboard /></div>

        <div class="rw-group">Speech model</div>
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
        <div class="rw-group">Recognition language</div>
        <div class="s-prose">
          <!-- NOT disabled when no model is loaded. The language is a stored
               preference now, not a setting on a live engine: a church that has
               just installed Relay has no model yet, and pinning the language
               before the first service is exactly what RG-116 asks a pilot to do.
               `stt_status` reports the stored value while there is no engine, so
               the select cannot read "Auto-detect" over a profile that says
               English. -->
          <select class="r-select" value={$capture.stt.language ?? ''} on:change={(e) => pickLanguage(e.target.value || null)} disabled={langBusy} aria-label="Recognition language">
            <option value="">Auto-detect (code-switching)</option>
            <option value="en">English</option>
            <option value="yo">Yoruba</option>
            <option value="sw">Swahili</option>
            <option value="ha">Hausa</option>
          </select>
          {#if langErr}
            <p class="s-alert" role="alert">{langErr}</p>
          {/if}
          <p class="rw-foot">Auto-detect handles English mixed with a local language mid-sentence — the normal case. Tier-1: Yoruba · Swahili · Hausa.</p>
          <!-- WHOSE SETTING THIS IS. One fact, one store: this writes to the
               ACTIVE voice profile, the same field the profile editor on AI &
               Detection edits. Saying so is the difference between a setting that
               persists and a setting that silently rewrites a profile the operator
               is not looking at. -->
          <p class="rw-foot">
            Saved to the active voice profile{activeProfile ? ` — “${activeProfile.name}”` : ''}, and
            applied before the first word of the next service. The same setting is on
            <b>Preachers → Voice profiles</b>; one preacher, one language.
            {#if !$capture.stt.loaded}
              No speech model is loaded yet, so nothing is listening — the choice is stored
              now and applied the moment one is.
            {/if}
          </p>
        </div>

        <!-- SAFE MODE. Promoted from General, which was the section it was least
             likely to be found in: safe mode is not a preference, it is whether this
             copy of Relay is ARMED at all, and it is the last thing an operator sets
             before a service or the first thing they set when something is wrong.
             `degraded.js` names this section in the sentence it prints. -->
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
          <!-- §12 · ONE INSTRUMENT. This was a `Turn on` / `Turn off` text
               button, which is a sentence where every other binary setting in
               the product shows a switch — and a sentence whose width changes
               with its own state, so the right edge of the control column moved
               by about fourteen pixels depending on whether safe mode was on.
               The switch is `app.css`'s `.r-switch`, the same 38x21 body as a
               colour well and as the Detection switch on the dock; Settings
               defines none of its own (§12, and `settingssections.test.js`).

               The WORD stays beside it. A switch says which way it is thrown;
               `on` / `off` in the value column says what that means, and it is
               read from `$safeMode` — the derived store, never a local mirror —
               so the control cannot show a state the boot record does not hold
               (rule 35). -->
          <div class="rw-nvctl s-nvpair">
            <span class="rw-nvv" class:s-armed={$safeMode}>{$safeMode ? 'on' : 'off'}</span>
            <button
              class="r-switch"
              class:on={$safeMode}
              role="switch"
              aria-checked={$safeMode}
              aria-label="Safe mode"
              on:click={() => applySafeMode(!$safeMode)}></button>
          </div>
        </div>
        <!-- SAFE MODE COULD NOT KEEP ITS PROMISE. Rose, never amber — amber is
             ON AIR and this page never is. The switch throws from the boot
             record, which the door writes first, so the record can read `on`
             while a screen is still open; this line is the only thing that says
             so, and it is the same contract as a panic control (rule 15,
             DECISIONS §20 · §86). -->
        {#if $safeModeError}<p class="rw-foot s-netbad" role="alert">{$safeModeError}</p>{/if}
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

        <!-- SERVICE LOCK. It was on History & Backup, which no longer exists — and
             it was never a backup. It belongs with the model and the language,
             because it is the thing that decides whether either can still be changed:
             the guard in `servicelock.rs` covers `select_stt_model`, `download_model`
             and `install_model_file`, all of them on this same section, eight rows up.

             THE STATE IS A ROW OF ITS OWN, the way safe mode's is. The block below
             only ever spoke when the lock was engaged, and a page that says nothing
             in the ordinary case is a page that cannot be checked: "held" and
             "lifted" are both real answers and an operator glancing at this section
             before a service needs to be able to read the second one (rule 35). -->
        <div class="rw-group">Service lock</div>
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Service lock</div>
            <p class="rw-nvnote">Deletions, speech-model changes and imports are held back while a service is being recorded. Nothing on the live path ever is.</p>
          </div>
          <span class="rw-nvv" class:s-armed={$serviceLock.engaged}>{$serviceLock.engaged ? 'held' : 'lifted'}</span>
        </div>
        <div class="s-prose">
          {#if $serviceLock.engaged}
            <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
              <b class="s-netwarn">A service is being recorded.</b>
              Relay is holding back a few things that cannot be undone, or that would take
              the speech engine away mid-sermon: {$serviceLock.held_back.join(', ')}.
              Firing, the transport, clearing and blacking out are unaffected.
            </p>
            <Button variant="ghost" size="sm" on:click={unlockService}>Unlock for this service</Button>
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

      {:else if section === 'room'}
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
            <Button variant="primary" on:click={toggleCapture} disabled={!$capture.available}
              disabledReason={whyDisabled([!$capture.available, ENGINE_OFF])}>
              {#if $capture.capturing}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
                Stop listening
              {:else}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><path d="M7 5.5v13l11-6.5-11-6.5z"/></svg>
                Start Listening
              {/if}
            </Button>
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
              <Button on:click={detectSpeakers} disabled={outBusy} disabledReason={whyDisabled([outBusy, BUSY])}>
                {outBusy ? 'Detecting…' : 'Detect speakers'}
              </Button>
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
          <!-- OUTSIDE the branch chain on purpose. Three of the four outcomes
               leave `outLocked` true and redraw the identical block, which is the
               defect; the fourth flips the branch and would drop the sentence that
               explains why. One place, all four. -->
          {#if outMsg}
            <p class="rw-foot" class:s-netbad={outMsgBad} role="status">{outMsg}</p>
          {/if}
        </div>

        <!-- SERVICE LENGTH, from General. It is a fact about the room and the
             morning, not about this copy of Relay: it drives the "time remaining"
             line on a stage or confidence monitor, and it is one of the five things
             a saved room puts back. Its group is named for what it is about rather
             than repeating the row's own name one size larger. -->
        <div class="rw-group">This service</div>
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
            <Button variant="ghost" size="sm" on:click={doSaveRoom} disabled={roomBusy}
              disabledReason={whyDisabled([roomBusy, BUSY])}>Save this room</Button>
          </div>
          {#if roomMsg}<p class="rw-foot" role="status">{roomMsg}</p>{/if}
        </div>
        <!-- `none yet` and `could not be read` are DIFFERENT facts (rule 35), and
             so is `still asking` — the third one this row could not say at all,
             because it was an `{:else}` on an `{#each}` and an `{#each}` has two
             branches. `ListState` has the three, with error outranking loading and
             loading outranking empty, decided once rather than three times on this
             page in three different wordings. -->
        <ListState
          loading={!roomsAsked}
          error={$readErrors.rooms}
          items={$rooms}
          what="saved rooms"
          onRetry={loadRooms}
          empty="No rooms saved yet. Name this space above and press Save this room, and one press puts it all back next time.">
          {#each $rooms as r (r.id)}
            <div class="rw-nv">
              <div class="s-nvtext">
                <div class="rw-nvk">{r.name}</div>
                {#if r.notes}<p class="rw-nvnote">{r.notes}</p>{/if}
              </div>
              <div class="rw-nvctl s-nvpair">
                <Button variant="ghost" size="sm" on:click={() => doUseRoom(r)} disabled={roomBusy}
                  disabledReason={whyDisabled([roomBusy, BUSY])}>Use</Button>
                <Button variant="ghost" size="sm" on:click={() => doDeleteRoom(r)} disabled={roomBusy}
                  disabledReason={whyDisabled([roomBusy, BUSY])}>Remove</Button>
              </div>
            </div>
          {/each}
        </ListState>

      {:else if section === 'preachers'}
        <div class="rw-group">Detection sensitivity</div>
        <!-- A GATE THAT DID NOT MOVE SAYS SO. Until this existed, a rejected write
             left the thumb where it was dragged and printed nothing anywhere in
             this section, on the one control that governs what the AI may put on a
             wall without asking. -->
        {#if gateErr}
          <div class="r-err" role="alert">The gate did not move — {gateErr}</div>
        {/if}
        <div class="s-prose">
          <div class="s-inline"><span class="s-count">self-calibrating</span></div>
          <!-- THE SAME DIAL THAT IS ON LIVE. Same store, same command, same event
               — move either and the other moves, with no reload. See the note
               above `onSensitivity`. -->
          <div class="s-slider">
            <div class="s-slider-top">
              <span class="r-lbl s-slider-name">Sensitivity</span>
              <span class="s-slider-val">{gateDial}</span>
            </div>
            <input class="r-range" type="range" min="0" max="100" step="1"
              value={gateDial}
              on:input={(e) => onSensitivity(+e.target.value)}
              disabled={!$capture.available}
              use:rangeFill={gateDial}
              aria-label="Detection sensitivity" />
            <div class="s-slider-ends"><span>CAUTIOUS (few, sure)</span><span>EAGER (many, noisy)</span></div>
          </div>
          <!-- WHAT THE DIAL DID, AS A RESULT AND NEVER AS A SETTING. These two
               bars used to be sliders here, and that made them a second control
               over one fact — see §96. They are still worth SHOWING: an operator
               asking "what did 62 actually do" has nowhere else to look, and the
               detection inspector prints the same pair in the same words.
               `dd`, not `input`. Nothing here is draggable, and the label says
               "result" rather than letting the layout imply it. -->
          <p class="r-lbl s-gatelbl">What that sets right now</p>
          <dl class="s-gatedl">
            <dt>Auto-fire above</dt>
            <dd class="r-mono">{gate.autoPct ?? '—'}</dd>
            <dt>Suggest above</dt>
            <dd class="r-mono">{gate.suggestPct ?? '—'}</dd>
          </dl>
          <!-- THE SENTENCE THAT SEPARATES THREE STATES A NUMBER CANNOT (rule 35):
               no engine, an engine nobody has asked yet, and a gate the learning
               has walked off the dial's curve. `describeGate` decides which, once,
               for every surface. Steel and dim — never amber (on air), never cyan
               (a guess about scripture), never amethyst (rehearsal). None of those
               three promises is about a threshold. -->
          {#if gate.note}
            <p class="rw-foot s-gatenote" role="status">{gate.note}</p>
          {/if}
          <p class="rw-foot">Only a direct, high-confidence quotation can ever auto-fire. A paraphrase is always a suggestion — a cosine is not a probability.</p>
        </div>

        <!-- VOICE PROFILES, and the gate above them, are ONE section — which is
             why the section is named for the preacher rather than for the
             machinery. A profile IS a calibration of that gate: splitting the dial
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

        <!-- Three facts, decided once. This was an `{#each}`'s `{:else}`, which has
             room for two: a failed `list_voice_profiles` and a church that has never
             added a preacher shared one row, separated only by a ternary on
             `readErrors`, and "still asking" could not be said at all. -->
        <ListState
          loading={!profilesAsked}
          error={$readErrors.listVoiceProfiles}
          items={profiles}
          what="voice profiles"
          onRetry={refreshProfiles}
          empty="No voice profiles yet. Add a preacher’s name below — the first one you add becomes the active calibration.">
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
                <Button variant="ghost" size="sm" disabled={profileBusy}
                  disabledReason={whyDisabled([profileBusy, BUSY])}
                  on:click={() => useProfile(p.id)}>Use</Button>
              {/if}
              <Button variant="ghost" size="sm" disabled={profileBusy}
                disabledReason={whyDisabled([profileBusy, BUSY])}
                on:click={() => openEditor(p)}>Edit</Button>
              <!-- Deleting the profile in use would leave the gate calibrated by
                   nothing, so the backend refuses it and says why. -->
              <Button variant="ghost" size="sm" disabled={profileBusy}
                disabledReason={whyDisabled([profileBusy, BUSY])}
                on:click={() => removeProfile(p.id)}>Delete</Button>
            </div>
          </div>
          {/each}
        </ListState>

        <div class="s-prose">
          <div class="s-addrow">
            <input
              class="r-input"
              placeholder="Preacher's name"
              aria-label="New voice profile name"
              bind:value={newName}
              on:keydown={(e) => e.key === 'Enter' && addProfile()} />
            <!-- The empty-name reason is written at the CALL SITE, the same shape as
                 the crash DSN's "Nothing has changed since the last save." The
                 sentences in `whydisabled.js` are the shared GATES — the engine, safe
                 mode, the service lock, a busy press — and a rule about this one
                 field is not one of those. -->
            <Button disabled={profileBusy || !newName.trim()}
              disabledReason={whyDisabled(
                [profileBusy, BUSY],
                [!newName.trim(), 'Type the preacher’s name in the box first.'],
              )}
              on:click={addProfile}>Add</Button>
          </div>
          <!-- The sentence that used to be here — "The first profile you add becomes
               the active calibration." — is the `ListState` empty message above, which
               is where an operator is already looking when there is nothing in the
               list. Two places saying it is two places to keep in step. -->
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
            <p class="rw-foot">
              The same setting as <b>Before the service → Recognition language</b>, which
              writes to whichever profile is active. There is one store for it, not two.
            </p>

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
              <Button variant="primary" disabled={profileBusy}
                disabledReason={whyDisabled([profileBusy, BUSY])}
                on:click={saveProfile}>Save profile</Button>
              <Button variant="ghost" disabled={profileBusy}
                disabledReason={whyDisabled([profileBusy, BUSY])}
                on:click={() => (editing = null)}>Cancel</Button>
            </div>
          </div>
        {/if}

      

      {:else if section === 'scripture'}
        <div class="rw-group">Bible translations</div>
        <!-- THE BRANCHES WERE IN THE WRONG ORDER, and that is the bug `ListState`
             exists to make unrepeatable. This chain asked `!dataLoaded` BEFORE
             `$readErrors.listTranslations`, and `dataLoaded` is set in a `finally`
             that runs whether the read worked or not — so for the frames in
             between, and on any path where it had not landed, a read that FAILED
             announced itself as still loading. Error outranks loading and loading
             outranks empty; the precedence now lives in one place instead of being
             re-typed per list, which is how two of the three on this page came to
             order it differently.

             The KJV is BUNDLED (`src-tauri/data/kjv.json`, `include_str!`), so "no
             translations" cannot be true of a Relay that is working — which is why
             the empty sentence says what it actually means rather than sending an
             operator off to find a Bible to import. -->
        <div class="s-checklist">
          <ListState
            loading={!dataLoaded}
            error={$readErrors.listTranslations}
            items={translations}
            what="translations"
            onRetry={loadTranslations}
            empty="No translations in the corpus. The KJV ships inside Relay, so this means the verse table could not be read — Save a diagnostic file on This machine, and reinstalling restores it.">
            {#each translations as tr}
              <button class="s-tr" class:on={tr.id === activeTranslation} aria-pressed={tr.id === activeTranslation} on:click={() => pickTranslation(tr.id)}>
                <span class="s-tr-dot" class:on={tr.id === activeTranslation}></span>
                <span class="s-check-code">{tr.abbreviation}</span>
                <span class="s-tr-name">{tr.name}</span>
                {#if tr.id === activeTranslation}<span class="s-tr-active r-mono">active</span>{/if}
              </button>
              {#if !isBundled(tr)}
                <!-- TWO PRESSES, in-app (rule 41). The bundled two never show this:
                     `db::delete_translation` refuses them too, but a control that
                     cannot succeed is a dead button. -->
                <div class="s-tr-tools">
                  <Button variant={trDeleteArmed === tr.id ? 'danger' : 'ghost'} size="sm"
                    on:click={() => doDeleteTranslation(tr)}
                    disabled={$serviceLock.engaged || tr.id === activeTranslation}
                    disabledReason={tr.id === activeTranslation ? 'Choose another translation first.' : whyDisabled([$serviceLock.engaged && SERVICE_LOCKED])}>
                    {trDeleteArmed === tr.id ? `Delete ${tr.abbreviation}, really` : 'Delete'}
                  </Button>
                  {#if trDeleteArmed === tr.id}
                    <Button variant="ghost" size="sm" on:click={() => (trDeleteArmed = null)}>Keep it</Button>
                  {/if}
                </div>
              {/if}
            {/each}
          </ListState>
        </div>
        <div class="s-prose">
          <p class="rw-foot">The <b>KJV</b> and the <b>Berean Standard Bible</b> ship inside Relay, both public domain. A licensed version (NKJV, NIV, ESV) cannot be bundled; if your church holds a licence and has the text as a file, import it below.</p>
        </div>
        <!-- IMPORT A BIBLE (RG-50 option two). The file is JSON in the KJV's shape:
             a list of 66 books, each { "chapters": [[verse, …], …] }, Genesis to
             Revelation. Relay checks every book is present and no verse is empty
             before a single row is written; the refusal says which. -->
        <div class="rw-group">Import a Bible</div>
        <div class="s-prose s-trimport">
          <label class="rw-nv"><span class="rw-nvk">Name</span><input class="r-input" type="text" bind:value={trName} placeholder="New King James Version" disabled={trBusy} /></label>
          <label class="rw-nv"><span class="rw-nvk">Short code</span><input class="r-input" type="text" bind:value={trAbbr} placeholder="NKJV" maxlength="12" disabled={trBusy} /></label>
          <label class="rw-nv"><span class="rw-nvk">Language</span><input class="r-input" type="text" bind:value={trLang} placeholder="en" maxlength="8" disabled={trBusy} /></label>
          <input type="file" accept=".json,application/json" bind:this={trFileInput} on:change={onBibleFile} style="display:none" />
          <Button variant="ghost" size="sm" on:click={() => trFileInput?.click()}
            disabled={trBusy || $serviceLock.engaged || !$capture.available}
            disabledReason={whyDisabled([trBusy && BUSY, $serviceLock.engaged && SERVICE_LOCKED, !$capture.available && ENGINE_OFF])}>
            {trBusy ? 'Importing…' : 'Import a Bible from a file…'}
          </Button>
          {#if trMsg}<p class="rw-foot" role="status">{trMsg}</p>{/if}
          <p class="rw-foot">A JSON file: a list of the 66 books in order, each with <code>"chapters"</code> as a list of verse lists. Verse layout may differ from the KJV; every book must be present. Held back while a service is being recorded.</p>
        </div>
        <!-- LANGUAGE COVERAGE. It had its own rail entry once and now shares a
             section with the translation list, which is the other answer to "what
             does Relay actually know?".

             THE PICKER IT USED TO SIT UNDER HAS MOVED, and that is deliberate
             rather than an oversight. `Recognition language` is on Before the
             service, beside the model, because the two together are what RG-116
             measured; this table is the evidence behind that choice and is read
             once, not every Sunday. The picker's own footnote says what the
             honest answer is — Tier-1, and nothing measured — so the pairing
             survives where it matters. -->
        <div class="rw-group">Language coverage</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            Counted from the data Relay ships with. Nothing here is a claim —
            improving a number means improving the table the detector uses, which is
            a one-line change anyone who speaks the language can make.
          </p>
          <!-- THE FIFTH HAND-ROLLED LIST STATE, and it had the same wrong order as
               the translation list: `!langsAsked` before `$readErrors.languageReport`,
               so a failed `language_report` read as still loading. It also ended in a
               fourth branch — "The language tables could not be read." — which is the
               ERROR sentence standing in for the empty one, said without the reason
               the store had kept and without a way to ask again.

               An empty answer here is not a real state of a working Relay: the alias
               table is compiled in. So the empty message says that, rather than
               describing a failure it cannot diagnose. -->
          <ListState
            loading={!langsAsked}
            error={$readErrors.languageReport}
            items={langs}
            what="the language tables"
            empty="No language tables. They ship inside Relay, so this means the alias table could not be read — Save a diagnostic file on This machine.">
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
          </ListState>
        </div>

      

      {:else if section === 'machine'}
        <div class="rw-group">This machine</div>
        <div class="rw-nv"><span class="rw-nvk">Found on this computer</span><span class="rw-nvv">{settingValue(lanIp, {
            loading: lanState === 'loading',
            missing: lanState === 'failed' ? 'could not be read' : 'not on a network',
          })}</span></div>
        <div class="rw-nv"><span class="rw-nvk">Output / stage pages</span><span class="rw-nvv">:8032 · http</span></div>
        <div class="rw-nv"><span class="rw-nvk">Live update channel</span><span class="rw-nvv">:8031 · websocket</span></div>
        <!-- INTEGRATIONS. Every row on it is an address on this machine's network,
             which is what the three rows above it are. The speech model used to sit
             between the two and has gone to Before the service: it was filed here
             because it ARRIVES over the network, which is true of the download and
             of nothing else about it — an operator looking for the model is asking
             how well Relay hears, not how it reaches the building. -->
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
        <!-- "via HDMI" alone was an instruction a church cannot follow on half the
             range: an ATEM Mini has HDMI inputs, a rack-mount ATEM has SDI inputs
             and no HDMI in (the Television Studio HD8's single HDMI connector is an
             OUTPUT). The converter is the mechanism, not the switcher.
             docs/OUTPUT_ROUTING.md §2. -->
        <div class="rw-nv"><span class="rw-nvk">ATEM / SDI switcher</span><span class="rw-nvv">via HDMI, plus a converter on SDI-only models</span></div>
        <div class="s-prose">
          <p class="rw-foot">Relay sends its output to other software over your local network — no plugins to install. Add a <b>Browser Source</b> pointing at Relay; the exact per-channel URL is in <b>Outputs → Sharing</b>. Connected devices (OBS · kiosk · stage remote) pull the live output from this machine on the same Wi-Fi.</p>
          <p class="rw-foot"><b>NDI is parked</b> — it needs a proprietary SDK Relay does not bundle, so there is no NDI source to select. For a switcher, open a Relay output window on a display and feed that HDMI in. An <b>ATEM Mini</b> takes it directly. A <b>rack-mount ATEM</b> has SDI inputs only, so it needs a small HDMI-to-SDI converter first, costing about as much as a microphone cable. Relay does not speak SDI directly and will not; that is what the converter is for.</p>
        </div>

      
        <div class="rw-group">Support facts</div>
        <!-- A FILE, NOT A SCREEN. This table has shown the right facts for a while
             and been useless for the job it exists for: nobody can email a screen.
             What actually happens is somebody photographs it, losing half the table
             and all of the latency history. -->
        <div class="s-prose">
          <Button variant="ghost" size="sm" on:click={doExportDiagnostics} disabled={diagBusy}
            disabledReason={whyDisabled([diagBusy, BUSY])}>
            {diagBusy ? 'Writing…' : 'Save a diagnostic file'}
          </Button>
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
          })} · {environment}{buildMarker ? ` · build ${buildMarker}` : ''}</span></div>
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
          <!-- RG-120 (PR #58). `end_to_end_speech_to_scripture` stamped 0 samples
               against three auto-fires in one service and 7 against nine in
               another, and the report could not say why. An absence there is
               honest (rule 31: a stage never reached is an absence, not a zero)
               but it was UNATTRIBUTABLE — "the AI never fired" and "nothing was
               attached to paint it" looked identical, and they are completely
               different situations. The first service ran its three fires before
               any output window existed, so nothing could have painted them and
               zero was the correct answer.

               These two counters are what make that readable. The first is a fact
               about the church's setup; the second is a fact about this
               instrument, because a trace the recorder dropped first is not the
               same as a screen that never answered. Both were silent. -->
          <div class="rw-nv"><span class="rw-nvk">verses no screen reported painting</span><span class="rw-nvv" class:s-netbad={(lat?.fires_never_painted ?? 0) > 0}>{lat?.fires_never_painted ?? 0}</span></div>
          <div class="rw-nv"><span class="rw-nvk">render reports that arrived too late</span><span class="rw-nvv">{lat?.marks_after_close ?? 0}</span></div>
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
            <Button on:click={resetLatency} disabled={!$capture.available}
              disabledReason={whyDisabled([!$capture.available, ENGINE_OFF])}>Start a fresh measurement</Button>
          </div>
          <p class="rw-foot">Start listening and speak for a few seconds to fill the table.</p>
        </div>
        <!-- §12 · ONE INSTRUMENT, and rule 35 on the same row.
             This was a second `r-btn` sitting beside *Start a fresh
             measurement* and reading `Stop measuring` — a text button whose
             label was the OPPOSITE of the state it described, next to one whose
             label was the action it performed. Two buttons, two grammars, one
             row. Resetting is an ACTION and stays a button; measuring is a
             SETTING and is now the switch every other binary setting in the
             product wears.

             THE WORD IS NOT `lat?.enabled ?? true`. That fallback is what the
             button had, and it printed *measuring* over a backend that had never
             answered — the same reading as a healthy pipeline, which is exactly
             what rule 35 forbids. `latMeasuring` is a tri-state: true, false, or
             null for "not read", and the value column says which.

             AND IT IS NOT A SETTING, WHICH IS THE HALF THE ROW USED TO HIDE.
             `latency.rs` holds `enabled` in process memory. There is no
             `app_settings` key behind it and nothing reads one at launch, so
             turning it off, closing Relay and opening it again puts it back on —
             and the row read identically either way. A control presented as a
             preference that silently reverts is rule 35 in the second person: it
             does not lie about the engine, it lies about ITSELF.

             PERSISTING IT WAS THE OTHER OPTION AND IS THE WRONG ONE. Measuring is
             what Settings → This machine shows a church and what a diagnostic
             bundle carries; its only purpose off is to prove, inside one field
             test, that the instrument is not the delay (rule 31, where the reflex
             answer was twice wrong). A church that turned it off one Sunday and
             kept it off for a year would have paid for that with the numbers
             nobody can now recover. A run is the right lifetime; the row simply
             has to SAY so, in the note and in the value, rather than being filed
             beside safe mode and crash reporting, which both outlive a relaunch. -->
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Measuring</div>
            <p class="rw-nvnote">On by default, and <b>for this run of Relay only</b> — it is not saved. Close Relay and measuring is on again next time. It costs a handful of timestamps per decode; turning it off is here so a field test can prove the instrument is not the delay.</p>
          </div>
          <div class="rw-nvctl s-nvpair">
            <!-- The value says the LIFETIME as well as the state, because "off" on
                 its own is what an operator would reasonably read as saved. -->
            <span class="rw-nvv">{latMeasuring === null ? settingValue(null, { missing: 'not read yet' }) : latMeasuring ? 'on' : 'off · until you restart'}</span>
            {#if latMeasuring !== null}
              <button
                class="r-switch"
                class:on={latMeasuring}
                role="switch"
                aria-checked={latMeasuring}
                aria-label="Measuring latency"
                on:click={() => toggleLatency(!latMeasuring)}></button>
            {/if}
          </div>
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
        <!-- `.s-nvp` because every answer `describeChannel` gives is a SENTENCE,
             never a figure — and one of them is now 112 characters. `.rw-nvv` is
             mono and sits in the `auto` half of a `minmax(0,1fr) auto` grid, so a
             long value takes the width from the name beside it: measured in a
             browser at an 878px row, "Update status" went from 772px with "up to
             date" to 98.8px with the failure sentence. `.s-nvp` caps the value at
             46ch and hands the name back 562.3px. It REPLACES `.rw-nvv` rather
             than joining it, which is how the Privacy report already uses it —
             keeping both left the sentence in mono. `.s-netbad` is declared after
             `.s-nvp` at equal specificity, so a failure is still rose. -->
        <div class="rw-nv"><span class="rw-nvk">Update status</span><span class="s-nvp" class:s-netbad={$updateChannel.state === 'failed'}>{describeChannel($updateChannel)}</span></div>
        {#if $updateChannel.state === 'failed'}
          <!-- The same cell type as the row above, for the same reason: this is an
               arbitrary backend string, and the measured page had it reading
               "Cannot read properties of undefined (reading 'invoke')". It renders
               only when the channel has failed, directly beneath the sentence that
               was taking the name's width — so both halves of one moment were
               squeezing their own labels. -->
          <div class="rw-nv"><span class="rw-nvk">Last attempt</span><span class="s-nvp">{$updateChannel.detail || 'no reason given'}</span></div>
        {/if}
        <div class="s-prose">
          <Button variant="primary" size="sm" on:click={doCheckUpdates} disabled={checking}
            disabledReason={whyDisabled([checking, BUSY])}>
            {checking ? 'Checking…' : 'Check for Updates'}
          </Button>
          <!-- ANNOUNCED. Six other message surfaces on this page carry a live
               region and these two did not, so the two results a screen reader
               user gets nothing for were the update check and the one control
               that decides whether data leaves the machine. -->
          {#if updateMsg}<p class="rw-foot" role="status">{updateMsg}</p>{/if}
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
          <!-- The field and the button that commits it are ONE decision, so they
               are one row (`.s-addrow`). Nothing here commits on blur: see the
               block in the script for why an address that decides where data
               leaves this machine does not become live by accident. -->
          <div class="s-addrow">
            <input id="crash-dsn" class="r-input s-dsn" type="text" placeholder="https://…@…ingest.sentry.io/…" bind:value={crash.dsn} disabled={!$capture.available || !!crashReadFailed} />
            <Button variant="ghost" size="sm" on:click={saveDsn} disabled={!$capture.available || dsnBusy || !dsnDirty || !!crashReadFailed}
              disabledReason={whyDisabled(
                [!$capture.available, ENGINE_OFF],
                [dsnBusy, BUSY],
                [!!crashReadFailed, 'The crash-reporting settings could not be read, so a new address cannot be saved over them.'],
                [!dsnDirty, 'Nothing has changed since the last save.'],
              )}>
              {dsnBusy ? 'Saving…' : 'Save address'}
            </Button>
          </div>
          {#if crashReadFailed}
            <!-- ASKED AND FAILED, not "there is no address". The safe default this
                 page was handed reads exactly like a church that never configured
                 one — and saving over it would write the empty string. Rose, never
                 amber: a control that has had to stand down is a failure, and
                 Settings is never on air (rule 18). -->
            <p class="s-alert" role="alert">
              Relay could not read the crash-reporting setting, so it cannot say what
              address is in force — {humanError(crashReadFailed)} The switch and
              <b>Save address</b> are held back until it can, so nothing writes over
              an address that may still be stored.
            </p>
          {:else if dsnDirty}
            <!-- Amethyst, never amber: this is a caution, and amber means on air.
                 An unsaved edit that says nothing is the same silence the Save
                 button was added to end, one step along. -->
            <p class="rw-foot s-netwarn" role="status">
              Not saved yet. Crash reports still go to the address Relay already has —
              press <b>Save address</b> to change that.
            </p>
          {/if}
        </div>

        <!-- §12 · ONE INSTRUMENT. This was a full-width `r-btn` reading *Turn
             crash reporting on* — a text button carrying a sentence, in a
             product where the same question is a switch everywhere else, and one
             that showed the ACTION rather than the STATE: the only way to read
             whether Relay was reporting was to read the label and invert it.

             The switch is thrown from `crashOn`, which is derived from the value
             the BACKEND returned (`$: crashOn = !!crash.enabled`), never from
             what was asked for. So a request the engine refuses — no DSN, a
             poisoned lock — leaves the switch where it was and puts the reason
             in `crashMsg` underneath, rather than showing a state that is not in
             force (rule 15, rule 35). The rose word beside it carries the
             judgement the switch cannot. -->
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Send crash reports</div>
            <p class="rw-nvnote">Off unless you turn it on, and it needs a DSN of your own above. The only thing in Relay that can send anything off this computer.</p>
          </div>
          <div class="rw-nvctl s-nvpair">
            <span class="rw-nvv" class:s-armed={crashOn}>{crashOn ? 'on' : 'off'}</span>
            <button
              class="r-switch"
              class:on={crashOn}
              role="switch"
              aria-checked={crashOn}
              aria-label="Send crash reports"
              disabled={!$capture.available || !!crashReadFailed}
              on:click={() => toggleCrash(!crashOn)}></button>
          </div>
        </div>
        <!-- The `{#if}` is OUTSIDE the block, not inside it: `.s-prose` carries a
             12px gutter, so an always-rendered wrapper around an empty message
             leaves a phantom band under the row whenever there is nothing to
             say. -->
        {#if crashMsg}
          <div class="s-prose"><p class="rw-foot" role="status" style="margin-top:0;">{crashMsg}</p></div>
        {/if}

      {:else if section === 'start'}
        <div class="rw-group">Setup</div>
        <div class="s-prose">
          <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
            <b>New here?</b> The setup walk-through picks your projector, checks the microphone is actually hearing something, and ends by putting a real verse on your real screen — so you have <i>seen</i> it work before Sunday.
          </p>
          <!-- THREE facts, and the third is the one the copy was already claiming.
               `engaged` and `recording` are deliberately different (main.rs's own
               note on `servicelock`): lifting the lock is a first-class operator
               override and it does NOT end the service. So mid-service, an operator
               who unlocked to delete something and stopped the microphone between
               readings — exactly the gap `updater.js::idle` was widened for — had
               both of the old terms false, and one click mounted `FirstRun` over a
               recorded service: an opaque `.fr-scrim` at z-index 950 over the
               dock's Clear screens, leaving only Esc, whose first press dismisses
               the wizard and not the wall (rule 44). It then stops the microphone
               and fires a verse. `recording` is in the same store and the dock
               already reads it. -->
          <!-- THROUGH THE KIT, so the reason reaches both channels. This was a raw
               `.r-btn` with a bare `title=`, which is the pointer answer and is
               invisible to a keyboard or screen-reader operator — on the one
               control here that mounts a full-screen wizard over a recorded
               service. `disabledReason` renders `title` AND `aria-describedby`.
               The three-fact guard and the prose reason below it are untouched. -->
          <Button
            variant="ghost"
            size="sm"
            on:click={restartSetup}
            disabled={$serviceLock.engaged || $serviceLock.recording || $capture.capturing}
            disabledReason={whyDisabled(
              [$capture.capturing, MIC_LIVE],
              [$serviceLock.engaged || $serviceLock.recording, SERVICE_LOCKED],
            )}>Run the setup walk-through</Button>
          {#if $serviceLock.engaged || $serviceLock.recording || $capture.capturing}
            <p class="rw-foot s-netwarn">Not while the microphone is live, or while a service is being recorded — including one you have unlocked, because unlocking does not end it. The walk-through stops the microphone and puts a verse on your screens. Stop listening and end the service first.</p>
          {/if}
        </div>
             section and not in the Library. Two facts are always on screen: what is
             loaded, and how many of it the operator has since changed — because the
             changed ones are KEPT by a removal, and that has to be knowable BEFORE
             the button is pressed, not discovered from the sentence afterwards. -->
        <div class="rw-group">Demo content</div>
        <div class="s-prose">
          {#if demo.loaded}
            <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
              <b>Relay's demo content is loaded</b> — {demo.total} items:
              {demo.groups.map((g) => `${g.count} ${g.what}`).join(' · ')}.
              Everything it added is named “Demo · …”, except the saved verses, whose
              names are real Bible references and stay true ones.
              {#if demo.edited > 0}
                <!-- `.s-netwarn` (amethyst), not an inline amber. Amber means ON
                     AIR and nothing else (rule 18), and Settings is never on air.
                     This file's own comments say "Rose, never amber" three times
                     and define the two colours a value may wear instead; this was
                     the last inline exception left. Amethyst rather than rose
                     because an edited demo item is a caution, not a failure — it
                     is the thing the Remove button will KEEP. -->
                <b class="s-netwarn"
                  >{demo.edited} of them {demo.edited === 1 ? 'has' : 'have'} been changed since.</b
                >
                Removing will <b>keep</b> {demo.edited === 1 ? 'that one' : 'those'} and delete the
                rest — there is no undo in Relay, so anything you have edited is treated as yours.
              {/if}
            </p>
            {#if demoArmed}
              <Button variant="danger" size="sm" disabled={demoBusy}
                disabledReason={whyDisabled([demoBusy, BUSY])} on:click={doRemoveDemo}>
                {demoBusy ? 'Removing…' : 'Yes, remove the demo content'}
              </Button>
              <Button variant="ghost" size="sm" disabled={demoBusy}
                disabledReason={whyDisabled([demoBusy, BUSY])} on:click={() => (demoArmed = false)}>
                Cancel
              </Button>
            {:else}
              <Button variant="ghost" size="sm" disabled={demoBusy}
                disabledReason={whyDisabled([demoBusy, BUSY])} on:click={() => (demoArmed = true)}>
                Remove demo content
              </Button>
            {/if}
          {:else}
            <p class="rw-foot" style="margin-top:0; padding-top:0; border-top:0;">
              <b>Nothing to press yet?</b> Relay can add a sample Sunday — a service plan with a
              countdown, notices, three public-domain hymns, a passage and a background; plus the
              songs, notices and saved verses behind it — so every workspace has something real in
              it. It is all named “Demo · …”, it is never loaded by itself, and one action takes it
              back out. <b>It adds no service history:</b> nothing here will ever look like a
              service that happened.
            </p>
            <Button variant="ghost" size="sm" disabled={demoBusy}
              disabledReason={whyDisabled([demoBusy, BUSY])} on:click={doLoadDemo}>
              {demoBusy ? 'Loading…' : 'Load demo content'}
            </Button>
          {/if}
          {#if demoErr}<p class="s-alert" role="alert">{demoErr}</p>{/if}
          {#if demoNote}<p class="rw-foot" role="status">{demoNote}</p>{/if}
        </div>
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
            <!-- `b` WAS IN THIS LIST, AND `b` IS BLACKOUT. `SHORTCUTS` carries
                 'B', so `RESERVED` carries `b`, so `assignKeys` can never issue
                 it — a Bridge is deliberately on `r` for exactly this reason
                 (REBRAND §10). The sentence promised "never one this page lists"
                 while printing one this page lists, and a volunteer who followed
                 it would black out the congregation's screens. -->
            A song's sections take single letters
            (<kbd class="s-kbd">v</kbd> <kbd class="s-kbd">c</kbd> <kbd class="s-kbd">r</kbd> …)
            — never one this page lists, and never while a field has focus. They
            work on the Library's song pane; the run surface does not take them yet.
          </p>
          <Button variant="ghost" size="sm" on:click={() => setSession({ activeTab: 'help' })}>Open Help &amp; Shortcuts</Button>
        </div>

      

        <!-- THE TWO NUMBERS A NEW INSTALL SETS ONCE, and the note about the page
             itself. General held five rows: safe mode and "screens at launch" moved
             to Before the service, service length to This room, and these two are
             what is left — neither is touched again after the first week, which is
             what makes this their section rather than a sixth of the rail. -->
        <div class="rw-group">This copy of Relay</div>
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
        <!-- COUNTDOWN WARNING. How long before zero a countdown turns red, on
             the wall, on the preacher's page and in the dock. One rule
             (`layers.js::countdownWarning`), one number, and this is where it is
             set — the comment above that rule used to say the threshold was
             deliberately not a setting because the control belonged in a Settings
             pass. This is it.

             SECONDS in the field, milliseconds in the row: an operator says
             "ninety seconds", nobody says "ninety thousand". The floor is five
             seconds, because a window shorter than the eye takes to find the
             screen is a colour that is never seen.

             It is READ. `App.svelte` loads it at launch and `setCountdownWarnMs`
             applies it to the rule, so this is not another of the seven controls
             removed on 2026-09-10 for saving a preference nothing opened. -->
        <div class="rw-nv">
          <div class="s-nvtext">
            <div class="rw-nvk">Countdown warning</div>
            <p class="rw-nvnote">How long before zero a countdown turns red, on every screen showing it. A countdown shorter than ten times this warns for its last tenth instead, so a short one is not red for half its life.</p>
          </div>
          <div class="rw-nvctl s-lenctl">
            <input class="r-input s-leninput" type="number" min="5" max="600" step="5"
              value={Math.round($countdownWarnMs / 1000)}
              on:change={(e) => setCountdownWarnMs(Number(e.target.value) * 1000)}
              aria-label="Countdown warning in seconds" />
            <span class="s-lenunit r-mono">sec</span>
          </div>
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
  /* THE ROW IS `.rw-item`, in WorkspaceFrame — see the markup. `.s-railbtn`
     lived here: twelve declarations that restated the shared rail row, plus a
     `:last-child`, a `:hover:not(.on)` and an `.on` that restated it again,
     and it disagreed with the original on exactly two numbers (32px vs 34px,
     gap 10 vs 9). All that is left is the icon, which the frame does not know
     about — the shape a legitimate override has. `.s-raillbl` went with it:
     the label is `.rw-itemname`, which adds the `flex:1; min-width:0` its
     hand-rolled twin never had, so a long section name now actually ellipses
     instead of pushing the count off the end of the row. */
  .s-railic{ flex:0 0 auto; }

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
  /* THE READINESS SURFACE IS THE SECTION, so it takes the panel's own gutter and
     nothing else — no card, no second border, no inset inside an inset. `.s-history`
     lived beside this and is gone with the History embed; History is a route now.
     `Dashboard.svelte` stacks its own panels in one column at this width rather
     than laying out a card grid inside an 880px reading column. */
  .s-dash{ padding:12px; min-width:0; }

  .s-inline{ display:flex; justify-content:flex-end; }

  /* THE THREE SHARED STATE BOXES TAKE THE PROSE GUTTER. `EmptyState`, `Loading`
     and `ErrorState` all render `.r-empty` with a 2px side padding, which is right
     inside a card and wrong at the panel's own edge — the rows around them bleed
     to the seam deliberately (see the SEAMS note above), and a sentence that
     starts two pixels from the edge reads as a rendering fault rather than as a
     message. One rule, because `ListState` is the only thing that renders them
     here and it renders all three. */
  .s-panel :global(.r-empty){ padding-left:12px; padding-right:12px; }

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
  /* The DSN takes the row's width and the Save button keeps its own, so a long
     address does not push the commit off the end of the line. */
  .s-dsn{ flex:1 1 260px; min-width:0; }
  .s-lenctl{ display:flex; align-items:center; gap:8px; justify-content:flex-end; }
  .s-leninput{ width:90px; text-align:right; }
  .s-lenunit{ color:var(--v-faint); font-size:var(--v-fs-cap); }
  /* THE WORD BESIDE A SWITCH, when the ON state is the one worth noticing.
     Safe mode ON is not a normal state: it is the whole application disarmed.
     Crash reporting ON is the one thing in Relay that can send anything off this
     computer. Neither is a fault and neither is a warning — the colour marks the
     state that is ACTIVE, and the sentence above it carries the judgement (the
     same reasoning as `.s-nvp.on`, one colour along).
     Rose, never amber — amber means ON AIR and is never spent on anything else.

     It is the WORD that is coloured and never the switch: `.r-switch.on` is
     steel blue everywhere in the product, and a switch that changed colour by
     section would be a second instrument wearing the first one's shape (§12). */
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
  .s-netwarn{ color:var(--v-caution2); }
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
  /* ── THE GATE, AS A RESULT ────────────────────────────────────────────────
     Two figures the dial produced, not two settings. The grammar is deliberately
     NOT the slider grammar one line up: a label and a mono value on one row, the
     same shape every read-only pair on this page uses, so nothing about the
     layout invites a drag.
     COLOUR: the page's own steel. No amber (ON AIR), no cyan (a guess about
     scripture), no amethyst (rehearsal) — not one of those three promises is
     about a threshold, and spending one here would weaken it everywhere. */
  .s-gatelbl{ margin:16px 0 8px; color:var(--v-dim); }
  .s-gatedl{ margin:0; display:grid; grid-template-columns:1fr auto; gap:6px 16px; align-items:baseline; }
  .s-gatedl dt{ color:var(--v-dim); font-size:var(--v-fs-b2); }
  .s-gatedl dd{ margin:0; text-align:right; color:var(--v-txt); font-size:var(--v-fs-mono); }
  /* The caveat is a sentence, never a glyph: the three states it separates —
     no engine, an engine nobody has asked, and a gate the learning has walked off
     the dial's curve — cannot be told apart by a coloured dot. */
  .s-gatenote{ color:var(--v-dim); }

  .s-slider-ends{ display:flex; justify-content:space-between; margin-top:8px;
    font-family:var(--f-mono); font-size:var(--v-fs-cap); letter-spacing:.06em; text-transform:uppercase;
    color:var(--v-faint); }

  /* bible translations */
  .s-checklist{ display:flex; flex-direction:column; gap:0; }
  .s-check-code{ font-family:var(--f-mono); font-size:var(--v-fs-mono); font-weight:600; letter-spacing:.05em;
    color:var(--v-txt); }
  /* A RADIO, not a button. One seamed row per translation, carrying its own
     dot, and exactly one of them is chosen at a time — `aria-pressed` and the
     dot say which. It keeps its own shape deliberately: `.r-btn` is a control
     you press and let go of, and a stack of them down the page would read as
     several things to do rather than one choice to make. (The list is however
     many translations the corpus holds, so there is no fixed count here.) */
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
  .s-tr-tools{ display:flex; gap:6px; padding:0 12px 8px; }
  .s-trimport .rw-nv .r-input{ max-width:260px; }
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
