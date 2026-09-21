<script>
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { trapFocus } from './lib/focus.js';
  import { t } from './lib/i18n.js';
  import { capture, capturing, live, screenBlack, rehearsing, initAudio, autoOpenOutputs, applySafeMode, safeModeError, dismissSafeModeError, clearScreens, blackScreen, panicError, dismissPanicError, dismissAudioError, loadServiceLock, channelHealth, channelWaiting, startChannelHealth, latencyReport, ping, onOperatorAction, noteOperatorAction, loadLiveTransition, loadCountdownWarnMs } from './lib/stores/capture.js';
  import * as training from './lib/training.js';
  import { practice, stopPractice } from './lib/practice.js';
  import { degradations, worstLevel, summarise } from './lib/degraded.js';
  import { describeScreen } from './lib/outputHealth.js';
  import { wallState, modelLabel, latencyP50, cadence, dropped, screenTally, elapsed, orNoData } from './lib/statusbar.js';
  import { installShortcuts, cheatsheet, liveShortcuts } from './lib/shortcuts.js';
  import { installLeaveGuard } from './lib/crash.js';
  import { session, setSession, resolveActiveTab } from './lib/session.js';
  import FirstRun from './lib/FirstRun.svelte';
  import Dock from './lib/Dock.svelte';
  import Splash from './lib/Splash.svelte';
  import BootSequence from './lib/boot/BootSequence.svelte';
  import BrandMark from './lib/ui/BrandMark.svelte';
  import { safeMode } from './lib/boot/boot.js';
  import { humanError } from './lib/errors.js';
  import {
    checkForUpdate,
    installUpdate,
    dismissUpdate,
    updateAvailable,
    updateProgress,
    updateError,
    updateVerdict,
    verifyLastUpdate,
    acceptUpdate,
    restoreSnapshot,
  } from './lib/updater.js';

  // The two answers to the post-update banner. Both GROUP 1 (they throw), because a
  // restore that silently failed would leave an operator waiting for a history that
  // is never coming back.
  // ── DEGRADED — the fallbacks that already existed, made visible ────────────
  //
  // Relay degrades gracefully in half a dozen places and every one of them was
  // invisible: the denoiser switching itself off on a microphone that will not run
  // at 48 kHz, a CPU-only build decoding three times slower, no speech model at
  // all. In each case Relay knew and the operator did not, so the symptom ("it
  // isn't hearing anything") got attributed to the AI being bad — the most
  // expensive possible misdiagnosis for this product.
  //
  // In the SHELL, not on Live, because a volunteer may be in Settings when the
  // model fails to load. Collapsed to one line until opened: a permanent list of
  // caveats across the top of a live console is a list an operator stops reading.
  let gpuBackends = null; // null = not asked yet; a BUILD fact, not a hardware one
  let droppedPartials = 0;

  // The NAME, not the id. This line used to map `st.id`, so the banner an
  // operator reads mid-service said "3 is not responding" — a number nothing on
  // any screen relates back to "Streaming". `degraded.js` has documented these as
  // names since it was written; only the producer disagreed.
  $: screensDown = Object.values($channelHealth)
    .filter((st) => describeScreen(st, {}, Number.MAX_SAFE_INTEGER).kind === 'down')
    .map((st) => st.name || `Screen ${st.id}`);

  $: degraded = degradations({
    sttLoaded: $capture.stt?.loaded,
    detectionOn: $capture.detectionOn,
    capturing: $capturing,
    safeMode: $safeMode,
    // …and whether it actually took. The record is written before the enforcement
    // runs, so this row asserted the promise over a screen that refused to close.
    safeModeError: $safeModeError,
    // `undefined` until the first quality frame — no row until Relay has looked.
    denoise: $capture.quality?.denoise,
    gpuBackends,
    macos: typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform || ''),
    droppedPartials,
    screensDown,
    // A dead capture, so it reaches the strip on EVERY workspace rather than only
    // the foot of Live. An operator editing a template when the lead unplugs the
    // interface had no signal at all before this.
    audioError: $capture.audioError,
    // RG-191: the LAN output server failed to bind, so every network screen is dead.
    outputError: $capture.outputError,
    // RG-121: the microphone this machine used last time is not here today, and
    // Relay has quietly fallen back to the system default.
    micMissing: $capture.inputDeviceMissing,
  });
  $: degLevel = worstLevel(degraded);

  // ── PRACTICE ──────────────────────────────────────────────────────────────
  //
  // Drills with the REAL controls, so the strip has to be where the controls are —
  // which is every tab. It is started from Help and lives here for the same reason
  // the panic bar does: an operator part-way through a drill must not have to
  // navigate away from the thing they are practising in order to read what to do.
  //
  // Rehearsal is forced on for the duration and restored after (RG-15's rule): the
  // controls are real, so the sandbox has to be too.
  let unlistenPractice = null;
  $: drill = training.current($practice.session);

  $: if ($practice.session.active && !unlistenPractice) {
    unlistenPractice = onOperatorAction((e) => {
      practice.update((p) => ({ ...p, session: training.observe(p.session, e) }));
    });
  }
  $: if (!$practice.session.active && unlistenPractice) {
    unlistenPractice();
    unlistenPractice = null;
  }

  let updMsg = '';
  async function doAccept() {
    updMsg = '';
    try {
      await acceptUpdate();
    } catch (e) {
      updMsg = humanError(e);
    }
  }
  async function doRestore() {
    updMsg = '';
    try {
      await restoreSnapshot($updateVerdict.snapshot);
      // A REQUEST, and the operator has to be told so. Copying a file over an open
      // database corrupts both, so it happens before the database is opened — and
      // somebody who believes it has already happened will not restart, and will
      // conclude Relay ignored them.
      updMsg = 'Restored on the next launch. Close Relay and open it again.';
    } catch (e) {
      updMsg = humanError(e);
    }
  }
  // Views are CODE-SPLIT. Statically importing all eight put every tab — Live
  // (the heaviest), the Planner, Settings, all of them — into one 637 KB bundle
  // the webview had to parse before the first frame, on hardware that is often a
  // borrowed church laptop. Now the boot bundle is the shell plus whichever tab
  // the operator was last on; each other view's chunk loads on first visit (all
  // from local disk — no network, offline-safe) and an idle prefetch warms the
  // rest so switching stays instant after boot.
  const viewLoaders = {
    live:      () => import('./lib/views/Live.svelte'),
    library:   () => import('./lib/views/Library.svelte'),
    planner:   () => import('./lib/views/ServicePlanner.svelte'),
    // THE TEMPLATES WORKSPACE IS ONE DESK: browse, then make. Themes was a tab,
    // then a second desk here, and is now neither — it was folded into the
    // template model (DECISIONS §87), because a theme's every field was already
    // a template `style` key. `themes` stays in `MOVED_TABS` so an operator whose
    // session still remembers the old tab lands on the workspace that now holds
    // what they were editing rather than being dumped on Live.
    templates: () => import('./lib/views/Templates.svelte'),
    channels:  () => import('./lib/views/Channels.svelte'),
    settings:  () => import('./lib/views/Settings.svelte'),
    // HELP IS ROUTABLE BUT NOT ON THE STRIP. Six workspaces is the grammar; Help
    // is not one of them (you do not run a service from it). It stays a real
    // route because two controls in Settings navigate to it and `?` opens the
    // cheatsheet that points at it — a surface nothing can reach is an orphan,
    // and `scripts/qa-inventory.mjs` is the instrument that says so.
    help:      () => import('./lib/views/Help.svelte'),
    // HISTORY IS ROUTABLE AND NOT ON THE STRIP EITHER, for the same reason and
    // by the opposite journey. It was a Settings SECTION, and it is not a
    // setting: nothing on it configures anything. It is a 900-line record
    // browser with its own list, detail, search, export and a two-step erase,
    // and burying a destructive action three levels inside a preferences page is
    // how a volunteer finds it by accident. Reading back what happened is not a
    // job you run a service from, so it does not take a seventh workspace slot —
    // it is reached from the readiness screen's own Recent services card, which
    // is where somebody is already looking at the same rows.
    history:   () => import('./lib/views/library/History.svelte'),
  };
  const viewCache = {}; // key → resolved component, loaded once then kept
  let current = null;   // the component for the active tab (null while its chunk loads)
  let viewLoadError = null;
  let viewLoadToken = 0;

  // ── THE SIX WORKSPACES (docs/REBRAND.md §2) ────────────────────────────────
  //
  // Live · Library · Planner · Templates · Outputs · Settings, in that order,
  // and that is the whole strip. It carried EIGHT — the six plus Themes and Help
  // — which is two more surfaces than the desk has jobs, and both of the extra
  // two are places you go from somewhere else rather than places you run a
  // service from:
  //
  //   THEMES was the style layer BENEATH templates, and a theme never reached a
  //   wall on its own. It became a DESK inside the Templates workspace, and then
  //   stopped existing: a theme had no field a template does not have, so it was
  //   inlined into every template that pinned it and deleted (DECISIONS §87).
  //   One pipeline, one workspace, one model.
  //
  //   HELP is not a workspace at all. It is still a real route (see
  //   `viewLoaders`), reached from Settings → Support & guide and from the
  //   cheatsheet `?` opens; it simply stops taking a slot in the strip beside
  //   the surfaces a service is actually run from.
  //
  // The ORDER is the prototype's and it is not alphabetical: it is the Sunday
  // path (run) then the week's path (build) then the room (outputs) then the
  // machine. `label` is an i18n KEY — the strip is the first thing a volunteer
  // looks at and the last thing they should have to read in a second language.
  const tabs = [
    { key: 'live',      label: 'tab.live',      title: 'Live Service' },
    { key: 'library',   label: 'tab.library',   title: 'Content Library' },
    { key: 'planner',   label: 'tab.planner',   title: 'Service Planner' },
    // `title` is the tooltip an operator reads, so it says what the workspace is
    // rather than what it used to hold: it was 'Templates & Themes' until themes
    // were folded into templates (DECISIONS §87).
    { key: 'templates', label: 'tab.templates', title: 'Templates' },
    // Outputs — the ONE surface for every render target: the congregation wall,
    // stage/confidence/preacher monitors, streaming and lobby screens. Each is a
    // real backend channel (native window or LAN/OBS URL over :8032) with its own
    // template. Its internal key is still `channels`; the label is what an
    // operator reads.
    { key: 'channels',  label: 'tab.channels',  title: 'Outputs' },
    // The Dashboard lives INSIDE Settings — it is the FIRST section of it now,
    // because "is this machine going to work?" is the question a Sunday actually
    // opens Settings to ask. Service History used to live there too and no
    // longer does: it is its own route (see `viewLoaders`), off the strip.
    { key: 'settings',  label: 'tab.settings',  title: 'System Settings' },
  ];
  // Every key the shell can RENDER, which is the strip plus the routes that are
  // reachable from inside a workspace. `resolveActiveTab` is given this rather
  // than the strip: hand it the strip alone and Settings' two "Open Help" buttons
  // would set a tab the resolver immediately bounces back to Live — a control
  // that looks like it worked and did nothing.
  const routes = [...tabs.map((x) => x.key), 'help', 'history'];
  // The active tab IS the session — not a local copy of it that happens to be
  // written back. One direction, one source of truth, so anything can navigate:
  // the Planner's "Run this plan" hands the operator to LIVE by setting it, and a
  // reload (or a crash + Recover) puts them back on the tab they were on.
  // A surface that MOVED sends the operator where it went, rather than dumping
  // them on Live — see `MOVED_TABS` in session.js, which is where the mapping and
  // its test live. An unknown key still falls through to the run surface.
  $: active = resolveActiveTab($session.activeTab, routes);
  const go = (key) => setSession({ activeTab: key });
  // (`currentTab` used to live here and nothing read it — a derivation with no
  //  consumer is the same dead weight as a component nothing renders.)

  // Resolve the active tab's component, loading its chunk on first visit and
  // caching it after. A token guards against a fast tab switch resolving out of
  // order and flashing the wrong view. A load failure (a missing chunk) is
  // surfaced calmly rather than white-screening — the outputs are separate
  // webviews and stay live regardless.
  $: resolveView(active);
  async function resolveView(key) {
    if (viewCache[key]) { current = viewCache[key]; viewLoadError = null; return; }
    const token = ++viewLoadToken;
    viewLoadError = null;
    try {
      const mod = await viewLoaders[key]();
      viewCache[key] = mod.default;
      if (token === viewLoadToken) current = mod.default;
    } catch (e) {
      if (token === viewLoadToken) { viewLoadError = e; current = null; }
    }
  }

  // After boot, warm the other chunks while the machine is idle so the FIRST
  // switch to any tab is instant. Never blocks; failures are ignored (the chunk
  // will just load on demand instead).
  function prefetchViews() {
    const idle = typeof requestIdleCallback !== 'undefined'
      ? requestIdleCallback
      : (cb) => setTimeout(cb, 300);
    for (const key of Object.keys(viewLoaders)) {
      if (viewCache[key]) continue;
      idle(() => viewLoaders[key]().then((m) => (viewCache[key] = m.default)).catch(() => {}));
    }
  }

  // FULL SCREEN LIVE CONTROL (§4). Hides the sidebar, top bar and footer so the
  // run surface owns the whole screen — a dark booth on a 13" laptop.
  //
  // It applies ONLY on Live. Leaving it on while the operator wanders to Settings
  // would strand them on a tab with no navigation, mid-service, with the chrome
  // they need to get back deliberately hidden.
  //
  // NOTE: Escape does NOT exit it. Everywhere else in computing Escape leaves
  // full screen; here Escape CLEARS THE CONGREGATION'S SCREENS and that meaning
  // is not negotiable (shortcuts.js). So the way out is a visible button that is
  // always on screen — never a key a muscle-memory reflex would reach for.
  $: liveFullscreen = active === 'live' && !!$session.liveFullscreen;
  // A fired picture carries no reference (`fire_media` sends an empty one), so
  // every image read "content" here. Name what it actually is.
  const liveLabel = (l) =>
    l?.reference || (l?.media_kind === 'video' ? 'video' : l?.media_url ? 'picture' : 'content');

  // Inline icons keyed by tab (SVG so they stay crisp on retina, themeable).
  const icons = {
    live: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7"><rect x="2.5" y="4.5" width="19" height="13" rx="2"/><path d="M8 21h8M12 17.5V21" stroke-linecap="round"/><circle cx="12" cy="11" r="2.6" fill="currentColor" stroke="none"/></svg>',
    channels: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3"/><circle cx="4" cy="12" r="2"/><circle cx="12" cy="6" r="2"/><circle cx="20" cy="14" r="2"/></svg>',
    templates: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="m12 2 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5M3 17l9 5 9-5"/></svg>',
    library: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2V5Z"/><path d="M9 3v14"/></svg>',
    planner: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4"/><path d="M7 13h4M7 17h7"/></svg>',
    settings: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7"><circle cx="12" cy="12" r="3.2"/><path d="M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-2.7 1.1V21a2 2 0 1 1-4 0v-.1A1.6 1.6 0 0 0 7 19.4a1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0-1.1-2.7H1a2 2 0 1 1 0-4h.1A1.6 1.6 0 0 0 2.6 7a1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.6 1.6 0 0 0 7 2.6h.1A1.6 1.6 0 0 0 8 1.1V1a2 2 0 1 1 4 0v.1A1.6 1.6 0 0 0 15 2.6a1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0 1.1 2.7h.1a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1Z"/></svg>',
  };

  let clock = '';
  let timer;
  // One second-hand for the whole shell: the wall clock, the on-air stopwatch and
  // the grace window the screen lamps are judged against all move from here. A
  // second interval would be a second opinion about what time it is.
  let nowMs = 0;
  function tick() {
    nowMs = Date.now();
    clock = new Date(nowMs).toLocaleTimeString('en-GB');
  }

  // ── THE ON-AIR STOPWATCH ───────────────────────────────────────────────────
  //
  // How long the congregation has been looking at something, counted from when
  // THIS console last saw the screens go live. It is a console-local observation
  // and it says so: nothing on the bridge records when a service went on air
  // (`service_lock` carries no start time, and `current_service` was deliberately
  // deleted), so a console restarted mid-service starts this figure again from
  // zero. Labelled and titled accordingly rather than dressed up as a service
  // clock it is not.
  //
  // It is cleared — not zeroed — when the screens are cleared, because
  // `statusbar.elapsed(null)` prints an absence and `00:00:00` prints a
  // measurement of nothing that reads exactly like a stopped clock (rule 35).
  // Blackout does NOT stop it: the wall is black, the session is still on air,
  // and that is precisely the state an operator is counting.
  let onAirFrom = null;
  $: if ($live) onAirFrom ??= Date.now();
  $: if (!$live) onAirFrom = null;
  $: onAirFor = onAirFrom === null ? null : elapsed(nowMs - onAirFrom);

  // ── THE TRANSITION CONTROL MOVED TO THE RUN SURFACE (L4) ───────────────────
  //
  // The picker and its duration used to sit at the right of this 34px bar. They
  // are now in Live's take rack, beside the transport they are about to change
  // the look of (`views/Live.svelte`, and `transitionoverride.test.js` follows
  // them there). Nothing about how a transition reaches a wall moved with them:
  // `resolveTransition` is still the one ranking, `TemplateRender` still keys the
  // replay on the override, and both panic controls are still outside it.
  //
  // `loadLiveTransition()` stays HERE, in the shell's mount, and deliberately:
  // the override is a BACKEND fact that every surface reads through the store,
  // and a console reopened mid-service must not draw a picker that disagrees with
  // screens that are already crossfading (rule 35) — including on a workspace
  // that draws no picker at all.

  // ── WHAT EVERY SCREEN IS DOING ─────────────────────────────────────────────
  //
  // One row per screen, each carrying that screen's REAL state from the same
  // `describeScreen` verdict Live's Output Status pane and the Outputs table
  // read. Never a second opinion about a screen (rule 35; RG-01 is the instance
  // that rule was written from).
  //
  // The chrome used to draw a LAMP per row. It does not any more (see the header
  // below), and these rows stayed, because two things downstream are made of
  // them: the status bar's `SCREENS n of m` tally, and — through `screensDown`
  // at the top of this file — the `Reduced` cell that names a screen which has
  // stopped answering. Both are still single-sourced from here, so they cannot
  // disagree with each other any more than they could with the lamps.
  //
  // The grace window is the real one (`$channelWaiting`), matching Live: a screen
  // that has only just been opened reads "Waiting…" on both surfaces rather than
  // red on one and grey on the other.
  $: screenLamps = Object.values($channelHealth).map((st) => ({
    id: st.id,
    name: st.name || `Screen ${st.id}`,
    ...describeScreen(
      st,
      { rehearsing: $rehearsing, live: !!$live, black: $screenBlack },
      $channelWaiting[st.id] ? nowMs - $channelWaiting[st.id] : 0,
    ),
  }));
  $: screens = screenTally(screenLamps);
  // (`LAMP_TONE` and `lampWord` lived here, and drew the chrome lamps alone. A
  //  map nothing reads is the same dead weight as a stylesheet rule nothing
  //  renders, so they went out with the markup rather than staying behind as a
  //  colour law with no colours on it. The law itself is unmoved: the status
  //  bar's one lamp reads `wall.tone`, `describeScreen` is still the only thing
  //  that decides a screen's state, and no fifth state was invented on the way.)

  // ── THE STATUS BAR'S FIGURES ───────────────────────────────────────────────
  //
  // One poll, feeding every cell. `latency_report(0)` asks for no traces, so this
  // is a handful of counters rather than the diagnostic payload Settings reads.
  let perf = null;
  $: wall = wallState({
    safeMode: $safeMode,
    safeModeFailed: !!$safeModeError,
    rehearsing: $rehearsing,
    black: $screenBlack,
    live: !!$live,
    label: $live ? liveLabel($live) : '',
  });
  $: lat = latencyP50(perf);
  $: cad = cadence(perf);
  $: shed = dropped(perf);
  $: model = $capture.stt?.loaded ? modelLabel($capture.stt?.model) : null;

  let engineOnline = false;
  let teardownKeys;
  let teardownLeave;
  let shedTimer;

  // ── Boot splash ────────────────────────────────────────────────────────────
  // Decoration over a fact, never a fact of its own. Two rules:
  //   1. It is HELD briefly so a fast boot doesn't strobe the brand for 80ms.
  //   2. It is CAPPED hard. If attaching the engine hangs — no backend, a wedged
  //      IPC, a plain browser — the splash comes down anyway. A boot screen that
  //      outlives its boot is indistinguishable from a hung app, and it would be
  //      covering the console an operator may need in the next thirty seconds.
  let booting = true;
  let appVersion = '';
  // The LAUNCH & STARTUP sequence (lib/boot/) runs AFTER the splash comes down
  // and BEFORE the console is usable. It is skippable by construction — Esc, a
  // per-stage cap, and a clean boot that collapses straight through — so this
  // flag can only ever delay the console briefly, never withhold it.
  let launched = false;
  /** Measured height of the panic bar, so the shell can move out from under it. */
  let panicH = 0;
  let holdTimer;
  let capTimer;
  const BOOT_HOLD_MS = 900;
  const BOOT_CAP_MS = 4000;

  // Read through a closure, so the beforeunload guard always sees the CURRENT
  // value rather than the one captured at mount time.
  let isCapturing = false;
  $: isCapturing = $capturing;

  onMount(async () => {
    const bootAt = Date.now();
    capTimer = setTimeout(() => (booting = false), BOOT_CAP_MS);
    tick();
    timer = setInterval(tick, 1000);
    // The panic keys are installed at the shell — never per-view — so Escape and
    // B work on every tab, including one whose view is broken.
    teardownKeys = installShortcuts({ clearScreens, blackScreen });
    teardownLeave = installLeaveGuard(() => isCapturing);
    await initAudio();
    // Restore the physical output screens (HDMI/projector) the operator set up, so
    // they come back on their own after a launch/update/rebuild. Safe: the backend
    // only opens onto connected, non-primary displays.
    //
    // NOT in safe mode. Safe mode promises nothing reaches a congregation, and
    // that promise is void the instant the projector windows re-open themselves
    // on boot. This used to run unconditionally, one step ahead of the safe-mode
    // guard below — so "outputs disabled" showed over screens that had just opened.
    if (!$safeMode) autoOpenOutputs();
    // A console reopened mid-service must not show an unprotected app.
    loadServiceLock();
    // …nor a transition picker that disagrees with every screen in the building.
    // The override lives in the backend; a reloaded console has no memory of it,
    // and a picker reading "Follow template" over screens that are crossfading is
    // a control that says the same thing whether or not it is in force (rule 35).
    loadLiveTransition();
    // …nor a countdown that turns red at a figure the operator did not choose.
    // The window is a setting (`Settings → Getting started → Countdown warning`), and the
    // dock and the programme pane ask the rule long before anybody opens that
    // page — so it is loaded HERE, at the shell, not on the page that writes it.
    loadCountdownWarnMs();
    // One poller for the whole app: Live, the Outputs table and the degraded
    // banner all read the same store (see `startChannelHealth`).
    startChannelHealth();
    // The GPU backends whisper.cpp was COMPILED with. Read once — it cannot change
    // while the app runs, and naming the GPU in this machine next to a CPU-only
    // build would be the most convincing lie on the screen.
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      gpuBackends = (await invoke('system_hardware'))?.gpu_backends ?? [];
    } catch {
      gpuBackends = null; // not asked, so nothing is claimed
    }
    // Shed work, sampled rather than pushed. Cheap (an in-memory counter), and slow
    // enough that it costs nothing over a service.
    //
    // It also feeds the status bar (latency p50, cadence, what has been shed), so
    // the interval came down from fifteen seconds to five: a figure in front of an
    // operator that is a quarter of a minute stale is a figure they will stop
    // trusting. `latency_report(0)` asks for no traces, so the payload is a
    // handful of counters either way.
    shedTimer = setInterval(async () => {
      perf = await latencyReport(0);
      droppedPartials = perf?.dropped_partials ?? droppedPartials;
      // AND ASK AGAIN WHETHER THE ENGINE IS THERE. `engineOnline` was set ONCE, at
      // mount, so a `greet` that failed on a slow or locked start left the sidebar
      // reading "Engine offline" for the rest of the session while everything
      // worked — a status line that cannot detect its own recovery (rule 35, the
      // mirror image). `ping`, never `greet`: `greet` is a COUNTER whose whole
      // value is one line per console mount (rule 26), and a poller calling it
      // would print the heartbeat every five seconds forever.
      engineOnline = await ping();
      // …AND WHETHER A SERVICE IS STILL RECORDING. The Controls dock's End
      // service button reads `$serviceLock.recording`, and until this line that
      // store was only refreshed on mount and by the two commands that change it
      // — so a service started from Settings, or ended in Library → History,
      // left the dock offering to end a service that was already closed. A
      // control that cannot see the state it acts on is rule 35 with a button on
      // it. It is the same poll, and `service_lock` is two atomics and a lock.
      loadServiceLock();
    }, 5000);
    // Did the LAST update work? Asked once, here, because the answer is only
    // knowable on the launch after one — and the person who pressed the button may
    // well have gone home before this launch happens.
    verifyLastUpdate();
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('greet', { name: 'operator' });
      engineOnline = true;
    } catch {
      engineOnline = false;
    }
    // …and it is re-asked on the shed timer above, with `ping`.
    // The version the splash shows is the one the UPDATER compares against, so
    // read it from Tauri rather than a second copy in the frontend bundle
    // (CLAUDE.md §19 — the version lives in three files and no more).
    try {
      const { getVersion } = await import('@tauri-apps/api/app');
      appVersion = await getVersion();
    } catch {
      appVersion = '';
    }
    // SAFE MODE IS A PROMISE, NOT A LABEL (lib/boot/SafeModeStartup.svelte):
    // nothing this app does may reach a congregation. Honour it the moment the
    // engine is attached — before any view has had a chance to arm anything. A
    // screen that says "outputs disabled" over a live detector is worse than no
    // safe mode at all.
    // One door, so a console reopened in safe mode and a switch flipped in
    // Settings enforce exactly the same thing. `applySafeMode` disarms detection
    // AND closes any screen that is already open, and reports rather than throws
    // — there is nothing here in a position to catch. DECISIONS §86.
    if ($safeMode) {
      await applySafeMode(true);
    }
    clearTimeout(capTimer);
    holdTimer = setTimeout(
      () => (booting = false),
      Math.max(0, BOOT_HOLD_MS - (Date.now() - bootAt)),
    );
    // Check once, on launch, while nothing is live. Never during a service.
    checkForUpdate();
    // Warm the other tab chunks while idle so the first switch is instant.
    prefetchViews();
  });
  onDestroy(() => {
    clearInterval(timer);
    clearTimeout(holdTimer);
    clearTimeout(capTimer);
    teardownKeys?.();
    teardownLeave?.();
    clearInterval(shedTimer);
  });
</script>

<!-- The live region.
     The `aria-live` count in this entire app was ZERO. A screen-reader operator
     was told NOTHING when scripture went onto the wall, or when the screens were
     cleared — the single thing they most need to know was the one thing the app
     never said out loud.

     "polite", not "assertive": it announces after whatever the operator is
     already reading, never over the top of it. -->
<div class="sr-only" role="status" aria-live="polite" aria-atomic="true">
  {#if $rehearsing}
    Rehearsal mode. Nothing is reaching the congregation's screens.
  {:else if $screenBlack}
    Screens blacked out.
  {:else if $live}
    Now on screen: {liveLabel($live)}{$live.translation
      ? `, ${$live.translation}`
      : ''}
  {:else}
    Screens cleared.
  {/if}
</div>

<!-- Boot splash. Covers the shell only while the engine is being attached, and
     comes down on a hard cap as well as on success (see BOOT_CAP_MS). It sits
     BELOW the panic bar deliberately — nothing may hide "the screens may still
     be live", not even the brand. -->
{#if booting}
  <Splash version={appVersion} />
{/if}

<!-- LAUNCH & STARTUP — Boot Diagnostics · Hardware Check · Plugin Loading ·
     Database Migration, plus the four gates (crash report, safe mode, update,
     recover session). Takes over from the splash and hands off to the console.

     Like the splash, it sits BELOW the panic bar: nothing may hide "the screens
     may still be live", not even a boot screen that is asking a question. -->
{#if !booting && !launched}
  <BootSequence version={appVersion} onDone={() => (launched = true)} />
{/if}

<!-- First run. Only ever on a genuinely fresh install, once the backend is
     attached (in a plain browser there is nothing to configure) — and never on
     top of the launch sequence, which is still asking about crashes and
     updates. One question at a time. -->
{#if launched && $capture.available && !$session.setupDone}
  <FirstRun />
{/if}

<!-- `padding-top` is driven by the MEASURED height of the panic bar, not a
     guessed constant: the message wraps to two or three lines on a narrow window,
     and a hard-coded offset would either clip it or leave a gap.

     Why it matters that the shell moves at all: the bar is `position:fixed` at
     the top, so it was COVERING the first ~56px of the app — the sidebar's brand
     lockup, and part of the top bar including the On Air badge. The one moment
     the operator most needs to see what is on the wall is the moment this bar is
     up, and it was sitting on top of that exact readout. -->
<div class="shell" class:has-panic={$panicError} class:chromeless={liveFullscreen} style="--panic-h:{panicH}px">

  <!-- THE TWO IN-FLOW BANNERS COME FIRST, ABOVE THE DESK.
       `.shell` is a flex COLUMN (`app.css`), so these stack across the top under
       the fixed panic bar — which is what `.audiobar`'s own comment has always
       said they do, and what a full-width bar with a `border-bottom` is drawn to
       be. They were below `.main-v` in the markup and beside it on the screen;
       see the measurement in `app.css`'s `.shell` note. -->
  <!-- THE MICROPHONE DIED (RG-117, PR #56). In the SHELL, for the reason the panic
       bar is: a volunteer may well be in Settings or Templates when the desk feed
       is knocked out, and a message on a tab they are not looking at is not a
       message. `role="alert"`, because it interrupts the service; it does not
       auto-dismiss, because "the microphone stopped" stays true until somebody
       does something about it.

       IT IS NOT A DUPLICATE OF `degraded.js`'s `audio` row, and the two cannot
       disagree because both read this one store field. They do different jobs:
       this INTERRUPTS, and the register INVENTORIES — the status bar's Reduced
       cell and the Dashboard's readiness rollup both walk `degradations()`, so a
       dead microphone that appeared only as a banner would be missing from the
       one list that answers "what is wrong right now". Severity earns the banner;
       completeness earns the row. -->
  {#if $capture.audioError}
    <div class="audiobar" role="alert" aria-live="assertive">
      <div class="panic-t">
        <b>Relay has stopped hearing the microphone.</b>
        <span>{$capture.audioError}</span>
      </div>
      <button class="r-btn ghost sm" on:click={dismissAudioError}>Dismiss</button>
    </div>
  {/if}

  <!-- SAFE MODE COULD NOT KEEP ITS PROMISE (DECISIONS §86). In the SHELL, for the
       reason the other two rose bars are: the switch is in Settings, the failure
       is about the OUTPUT SCREENS, and an operator who flips safe mode and then
       walks to Live to see what is still lit must not be told there that outputs
       are disabled. The rose line on the Settings row is the only other place
       this exists, and it is on the one page they have just left.

       `.audiobar`, not `.panicbar`: in flow, paints over nothing, and takes no
       part in the `--panic-h` offset that a second fixed bar would have to
       share. Rule 44 — an overlay may never cover `Clear screens`, and the
       cheapest way to keep that true is not to overlay anything.

       AFTER the microphone bar on purpose: `audioerror.test.js` slices the shell
       from the FIRST `class="audiobar"` to find that banner, so a second one
       above it shadows the assertion. A dead microphone is also the more urgent
       of the two — this one is about screens nobody is firing to.

       Rose, never amber. It does not auto-dismiss: "a screen may still be live"
       stays true until somebody has looked at the screen. -->
  {#if $safeModeError}
    <div class="audiobar" role="alert" aria-live="assertive">
      <div class="panic-t">
        <b>Safe mode is NOT enforced.</b>
        <span>{$safeModeError}</span>
      </div>
      <button class="r-btn ghost sm" on:click={dismissSafeModeError}>Dismiss</button>
    </div>
  {/if}

  <!-- Main -->
  <div class="main-v">
    <header class="topbar-v">
      <!-- THE LOCKUP, then the workspaces. A control room puts them across the
           top: the desk is wide, not tall, and a column of nav is height the
           slide grid does not get (docs/REBRAND.md §2). -->
      <!-- THE LOCKUP IS TWO WORDS (docs/REBRAND.md §1). `RELAY` is the product and
           `studio` is the room it is: a mono tag, a third the weight, on the
           baseline of the wordmark rather than beside it as a second name. The
           prototype's `.wordmark` is exactly this and the app carried only the
           first half.

           It is NOT a rename — the product, the bundle id, the window title and
           every document still say Relay. `aria-hidden` on the tag for the same
           reason: a screen reader announcing "relay studio" for the thing every
           other surface calls Relay is a second name where there is one. -->
      <span class="chrome-brand"><BrandMark size="17px" /><b>RELAY</b><span class="chrome-tag" aria-hidden="true">studio</span></span>
      <nav class="ws-menu" aria-label="Workspaces">
        {#each tabs as tab}
          <button
            class="ws-tab r-focus"
            class:on={tab.key === active}
            aria-current={tab.key === active}
            on:click={() => go(tab.key)}
          >{$t(tab.label)}</button>
        {/each}
      </nav>
      <!-- ── AND NOTHING ELSE (2026-09-14, on the operator's instruction) ──────
           This bar is NAVIGATION. Six things used to share it with the six
           workspaces — the ON AIR ladder, the LISTENING chip, the PROTECTED
           chip, one lamp per screen, the keys legend and an Emergency Stop
           button — and the operator's instruction was to clear all of it out.

           TWO OF THEM CARRIED GUARANTEES, so how they left matters more than
           that they left.

           EMERGENCY STOP was a panic control (rule 15, DECISIONS §20), and
           removing it is only safe because the two paths it duplicated are both
           still here and were both re-read before it went:
             · `Esc` clears from every tab, even mid-typing — `installShortcuts`
               is mounted ONCE in this file's `onMount` and binds `Escape`
               straight to `clearScreens`, never through a view's context, so it
               survives a crashed workspace (`shortcuts.js`; `panic.test.js`);
             · `Clear screens` is the full-width control along the bottom edge of
               the Controls card in the dock, which is in the SHELL and on every
               workspace, whose body is `overflow:hidden` so it can never scroll
               a panic control out of reach, and which is ordered FIRST when the
               dock stacks to one column (`Dock.svelte`; `panic.test.js`).
           Neither the button nor the dock was reachable inside full-screen Live
           in the first place: `.chromeless` hides this whole header and the
           shell withholds the dock, so on that one surface `Esc` was already the
           only way and nothing about it changed today.

           THE ON AIR LADDER AND THE LAMPS were the chrome's two true statements
           about what a congregation can see, and after this the STATUS BAR is
           the only surface that makes them. It does still make them, which is
           why this was allowed: `wallState` is the SAME ladder this badge read
           (safe ▸ rehearsal ▸ blackout ▸ on air ▸ clear) and it carries the
           label too, so "Service begins in" is still named; `screenTally` counts
           the same `screenLamps` rows the lamps were drawn from; and a screen
           that has stopped answering is named, by name, in the `Reduced` cell
           (`degraded.js`, fed by `screensDown` above). What is genuinely lost is
           the per-screen colour at a glance — the tally says two of three, the
           Reduced cell says which one, and no cell says the same thing whether
           the wall is live or dark (rule 35).

           THE LISTENING CHIP is restated by the dock's Live audio card (its
           switch reads `live`/`off`) and its transcript card (`listening…` /
           `not listening`), on every workspace.

           THE PROTECTED CHIP is NOT restated on every workspace. Service lock
           can be lifted while a service is still recording, so the dock's
           `End service` state is not the same fact; it is stated in Settings →
           History & Backup and on the Dashboard, and nowhere else. Recorded in
           the review note rather than quietly compensated for here.

           THE KEYS LEGEND is gone and `?` remains the one place the keys are
           documented. Its `short` glosses went with it, out of `SHORTCUTS`. -->
    </header>

    <div class="mainscroll r-scroll">
      {#if liveFullscreen}
        <button
          class="exit-fs"
          on:click={() => setSession({ liveFullscreen: false })}
          title="Escape clears the screens — it does not leave full screen">
          Exit full screen
        </button>
      {/if}
      {#if current}
        <svelte:component this={current} />
      {:else if viewLoadError}
        <div class="view-loaderr">
          <p>This section could not load.</p>
          <button class="r-btn sm" on:click={() => resolveView(active)}>Try again</button>
        </div>
      {:else}
        <div class="view-loading" aria-live="polite" aria-busy="true"><span class="view-spinner"></span></div>
      {/if}
    </div>

    <!-- THE DOCK ROW (docs/REBRAND.md §2). Under the desk, the same on every
         workspace: the level, the transcript, the three tools that change during
         a service, and the four controls that change what a congregation sees.
         In the SHELL, not inside Live — an operator editing a template still
         needs Clear screens within one reach. -->
    {#if !liveFullscreen}<Dock />{/if}

    <!-- THE STATUS BAR (docs/REBRAND.md §2). 26px of facts, left to right in the
         order an operator asks for them: what the ROOM is doing, then what the
         MACHINE is doing, then — pushed right — the screens and the time.

         Every figure comes from something already on the bridge, through the ONE
         pure module that derives it (`lib/statusbar.js`): `latency_report` for
         the p50, the cadence and what has been shed, `stt_status` for the model,
         the `channel_status` poll for the screens. Nothing here is computed twice
         and nothing here is invented.

         And every cell can say it does not know. A figure that prints `0 ms` when
         nothing has ever been measured reads exactly like a fast pipeline, which
         is rule 35 with a number instead of a word — so an absent fact prints
         `no data` and an absent clock prints nothing at all. -->
    <footer class="footer-v" aria-label="Status">
      <!-- THE ROOM. One ladder, shared with the chrome badge above through
           `wallState`, so the two strips on the same screen cannot disagree about
           the same wall. -->
      <span class="st st-state">
        <span class="lamp {wall.tone === 'onair' ? 'amber' : wall.tone === 'rehearsal' || wall.tone === 'safe' ? 'amethyst' : 'grey'}"></span>
        <span class="v vw">{wall.words}</span>
      </span>
      <!-- Counted by this console, from when it last saw the screens go live. -->
      <span class="st" title="How long something has been on the screens, counted by this console. A console restarted mid-service counts from its own start.">
        <span class="k">On air</span><span class="v">{orNoData(onAirFor)}</span>
      </span>
      <!-- Microphone to a transcript on this screen, median. The span that exists
           during every service — `end_to_end_speech_to_scripture` has no samples
           at all until scripture has reached a screen. -->
      <span class="st" title="Median time from the audio to a transcript update on this screen (audio_to_partial_transcript). Settings → This machine has the whole report.">
        <span class="k">Latency p50</span><span class="v">{lat === null ? orNoData(null) : `${lat} ms`}</span>
      </span>
      <!-- Shed partials, and RED the moment either counter moves. Audio that was
           never heard is worse news than a partial that gets re-decoded (rule 33),
           so the title says which. -->
      <span
        class="st"
        title={shed
          ? `${shed.partials} partial transcript${shed.partials === 1 ? '' : 's'} shed because detection could not keep up; ${shed.audio} chunk${shed.audio === 1 ? '' : 's'} of audio never heard at all.`
          : 'Nothing measured yet.'}>
        <span class="k">Dropped</span>
        <span class="v" class:bad={shed?.bad}>{shed ? shed.partials : orNoData(null)}</span>
      </span>
      <!-- The model is part of the cadence figure beside it (rule 32): `base`
           steps about four times a second and `large-v3-turbo` about once. -->
      <span class="st" title={$capture.stt?.model || 'No speech model is loaded.'}>
        <span class="k">Model</span><span class="v">{orNoData(model)}</span>
      </span>
      <span class="st" title="Transcript updates per second, this session.">
        <span class="k">Cadence</span><span class="v">{cad === null ? orNoData(null) : `${cad} /s`}</span>
      </span>
      <span class="push"></span>
      <!-- WHAT IS REDUCED RIGHT NOW (DECISIONS §45). The same `degradations()`
           verdict the floating strip used to carry, in the strip an operator is
           already reading. It is ABSENT when nothing is reduced rather than
           printing a reassuring "all well": a cell that says the same thing when
           it has nothing to report is rule 35's defect. Rose only when something
           is BLOCKED; a merely reduced capability is grey, because Relay is still
           doing the thing. -->
      {#if degLevel}
        <span
          class="st st-deg"
          title={degraded.map((d) => `${d.title} — ${d.what} ${d.fix}`).join('\n')}>
          <span class="k">Reduced</span>
          <span class="v" class:bad={degLevel === 'blocked'}>{summarise(degraded)}</span>
        </span>
      {/if}
      <!-- The same rows the chrome lamps are drawn from, counted. A screen counts
           as live only at On Air: in a rehearsal nothing reaches a congregation,
           and a tally that said otherwise would be the colour law broken in
           arithmetic. -->
      <span class="st" title="Screens showing the programme, out of the screens Relay knows about.">
        <span class="k">Screens</span><span class="v">{screens.live} of {screens.total}</span>
      </span>
      <!-- IS THE BACKEND THERE AT ALL. The prototype's `LOCAL offline` is a
           constant, and Relay has no honest equivalent (see the review note);
           this is the fact that slot is actually worth. It is re-asked on every
           poll with `ping`, so it can detect its own recovery as well as its own
           failure — the mirror half of rule 35, and a real bug this shell had. -->
      <span class="st" title="Whether the console can still reach the Relay engine.">
        <span class="k">Engine</span>
        <span class="v" class:bad={!engineOnline}>{engineOnline ? 'attached' : 'not answering'}</span>
      </span>
      <!-- The prototype has no clock; an operator has a service to start on time.
           Last, because it is the one figure here that is not about Relay. -->
      <span class="st"><span class="k">Clock</span><span class="v">{clock}</span></span>
    </footer>
  </div>

  <!-- A panic control FAILED. Clear or blackout did not reach the outputs, so the
       congregation may still be seeing the last thing that went up.

       `assertive`, not `polite` — this is the one message in Relay that is allowed
       to interrupt whatever a screen reader is currently saying. And it does not
       auto-dismiss: the operator closes it, having looked at the actual screen. -->
  {#if $panicError}
    <div class="panicbar" role="alert" aria-live="assertive" bind:clientHeight={panicH}>
      <div class="panic-t">
        <b>The screens may still be live.</b>
        <span>{$panicError}</span>
      </div>
      <button class="r-btn ghost sm" on:click={dismissPanicError}>Dismiss</button>
    </div>
  {/if}

  <!-- PRACTICE. Above the degraded strip: while a volunteer is being taught, the
       instruction is the most important thing on the screen. Amethyst, because it
       is rehearsal — the same colour the top bar is already showing. -->
  {#if $practice.session.active && drill}
    <div class="prac" role="status">
      <div class="prac-t">
        <span class="r-mono prac-n">{$practice.session.index + 1} / {training.DRILLS.length}</span>
        <b>{drill.title}</b>
        <span>{drill.hint}</span>
      </div>
      {#if drill.id === 'rehearsal'}
        <button class="r-btn sm" on:click={() => noteOperatorAction('acknowledge')}>I can see it</button>
      {/if}
      <button class="r-btn ghost sm" on:click={() => practice.update((p) => ({ ...p, session: training.skip(p.session) }))}>Skip</button>
      <button class="r-btn ghost sm" on:click={stopPractice}>Stop</button>
    </div>
  {/if}

  <!-- DEGRADED used to be a floating strip above the dock. It is now a STATUS BAR
       CELL (see `.st-deg` in the footer above): the verdict is unchanged and
       `degraded.js` is untouched, but it no longer sits over the console.

       What this costs, stated rather than hidden: the strip carried each row's
       `fix` line in words, and a status cell carries it in a `title`. Nothing is
       silent — a reduced capability is still named on every tab, all the time,
       which is what DECISIONS §45 asks for — but the detail is now a hover away
       rather than a press away. -->

  <!-- THE LAUNCH AFTER AN UPDATE.
       Sits ABOVE the "an update is available" banner, because "the last one broke
       your history" outranks "here is another one". Only the Broken case is loud:
       an update that landed cleanly is confirmed once, quietly, and forgotten. -->
  {#if $updateVerdict?.verdict === 'broken'}
    <div class="upd upd-bad" role="alert">
      <div class="upd-t">
        <b>Relay updated from {$updateVerdict.from_version}, and its database is not right.</b>
        <span>{$updateVerdict.reason} A copy of your history from before the update is still on this machine.</span>
        {#if updMsg}<span class="upd-msg">{updMsg}</span>{/if}
      </div>
      <button class="r-btn sm" on:click={doRestore}>Restore my history</button>
      <button class="r-btn ghost sm" on:click={doAccept}>Keep this and continue</button>
    </div>
  {:else if $updateVerdict?.verdict === 'landed'}
    <div class="upd">
      <div class="upd-t">
        <b>Relay updated from {$updateVerdict.from_version}.</b>
        <span>Your history came through intact.</span>
      </div>
      <button class="r-btn ghost sm" on:click={doAccept}>Dismiss</button>
    </div>
  {/if}

  <!-- Update banner. Only ever appears at rest — updater.js refuses to even look
       while the microphone is live, and refuses to install if it becomes live. -->
  {#if $updateAvailable && !$capturing}
    <div class="upd">
      <div class="upd-t">
        <b>Relay {$updateAvailable.version} is available.</b>
        <span>Installing restarts the app, so do it before the service — not during.</span>
      </div>
      {#if $updateProgress !== null}
        <span class="r-mono upd-pct">{$updateProgress}%</span>
      {:else}
        <button class="r-btn primary sm" on:click={installUpdate}>Update now</button>
        <button class="r-btn ghost sm" on:click={dismissUpdate}>Not now</button>
      {/if}
    </div>
  {/if}
  {#if $updateError}
    <div class="upd err">{$updateError}</div>
  {/if}

  <!-- Shortcut cheatsheet (?) — the bindings are read from the same table the
       handler uses, so help can never drift out of sync with reality. -->
  {#if $cheatsheet}
    <!-- Clicking the scrim closes it. That's a mouse convenience only — the
         keyboard path is Escape, handled globally in lib/shortcuts.js — so there
         is no keyboard trap here and no keyboard-only user is stranded. -->
    <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
    <div class="cheat-scrim" role="presentation" on:click={() => cheatsheet.set(false)}>
      <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
      <div class="cheat" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts" use:trapFocus on:click|stopPropagation>
        <h2>Keyboard shortcuts</h2>
        <table>
          {#each $liveShortcuts as s}
            <tr>
              <td class="keys">
                {#each s.keys as k}<kbd>{k}</kbd>{/each}
              </td>
              <td class="lbl">{s.label}</td>
              <td class="scope">{s.always ? 'Always' : 'Here'}</td>
            </tr>
          {/each}
        </table>
        <!-- This used to read "Esc and B work on every tab, even while typing."
             The B half was false: shortcuts.js yields to text entry before it
             reaches B, because an operator typing "Habakkuk" into the reference
             box must not black out the congregation on the second keystroke. A
             help screen that teaches a false fact about a PANIC key, to someone
             who will only read it under pressure, is the worst line in the app. -->
        <p class="cheat-foot">
          <kbd>Esc</kbd> works on every tab, even while typing — it clears the screens
          and leaves the box you were in. <kbd>B</kbd> works on every tab too, but not
          while your cursor is in a text field.
        </p>
      </div>
    </div>
  {/if}

  <!-- Mobile bottom nav -->
  <nav class="botnav">
    {#each tabs as tab}
      <!-- go(), NOT `active = tab.key`. `active` is a DERIVATION of $session (see the
           reactive statement above), so assigning to it writes to a value that is
           immediately recomputed. The tab change was never persisted, and the next
           setSession() from anywhere — Live writes one on every slide — recomputed
           `active` from the store and yanked the operator back to the previous tab
           mid-service. The desktop sidebar always called go(); the bottom nav didn't. -->
      <button class="bn r-focus" class:active={tab.key === active} on:click={() => go(tab.key)}>
        <span class="ic">{@html icons[tab.key]}</span>
        {$t(tab.label)}
      </button>
    {/each}
  </nav>
</div>
