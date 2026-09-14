<script>
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { trapFocus } from './lib/focus.js';
  import { t } from './lib/i18n.js';
  import { capture, capturing, live, screenBlack, rehearsing, initAudio, autoOpenOutputs, setDetection, clearScreens, blackScreen, panicError, dismissPanicError, serviceLock, loadServiceLock, channelHealth, channelWaiting, startChannelHealth, latencyReport, ping, onOperatorAction, noteOperatorAction, setLiveTransition, loadLiveTransition } from './lib/stores/capture.js';
  // X1 · the transition register and the override's store (docs/REBRAND.md §8).
  import { TRANSITIONS, TRANSITION_MS, DEFAULT_TRANSITION_MS, liveTransition } from './lib/transitions.js';
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
  let degOpen = false;

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
    // `undefined` until the first quality frame — no row until Relay has looked.
    denoise: $capture.quality?.denoise,
    gpuBackends,
    macos: typeof navigator !== 'undefined' && /Mac/i.test(navigator.platform || ''),
    droppedPartials,
    screensDown,
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
    // THE TEMPLATES WORKSPACE HOLDS TWO DESKS (docs/REBRAND.md §2). Themes is the
    // style layer beneath templates and was never a seventh thing an operator
    // runs a service from — it is where you go while you are already editing a
    // look. `Templates.svelte` is the router that picks the desk; `themes` is in
    // `MOVED_TABS` so an operator whose session still remembers the old tab lands
    // on the desk it went to rather than being dumped on Live.
    templates: () => import('./lib/views/Templates.svelte'),
    channels:  () => import('./lib/views/Channels.svelte'),
    settings:  () => import('./lib/views/Settings.svelte'),
    // HELP IS ROUTABLE BUT NOT ON THE STRIP. Six workspaces is the grammar; Help
    // is not one of them (you do not run a service from it). It stays a real
    // route because two controls in Settings navigate to it and `?` opens the
    // cheatsheet that points at it — a surface nothing can reach is an orphan,
    // and `scripts/qa-inventory.mjs` is the instrument that says so.
    help:      () => import('./lib/views/Help.svelte'),
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
  //   THEMES is the style layer BENEATH templates. A theme never reaches a wall
  //   on its own: it is applied to a template, and the template is what fires.
  //   It is now a DESK inside the Templates workspace (`views/Templates.svelte`
  //   is the router, and the two-way switch is in its header) — one pipeline,
  //   one workspace, instead of two tabs an operator has to know are related.
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
    { key: 'templates', label: 'tab.templates', title: 'Templates & Themes' },
    // Outputs — the ONE surface for every render target: the congregation wall,
    // stage/confidence/preacher monitors, streaming and lobby screens. Each is a
    // real backend channel (native window or LAN/OBS URL over :8032) with its own
    // template. Its internal key is still `channels`; the label is what an
    // operator reads.
    { key: 'channels',  label: 'tab.channels',  title: 'Outputs' },
    // Service History and the Dashboard live INSIDE Settings — records and
    // overview surfaces, not surfaces a service is run from.
    { key: 'settings',  label: 'tab.settings',  title: 'System Settings' },
  ];
  // Every key the shell can RENDER, which is the strip plus the routes that are
  // reachable from inside a workspace. `resolveActiveTab` is given this rather
  // than the strip: hand it the strip alone and Settings' two "Open Help" buttons
  // would set a tab the resolver immediately bounces back to Live — a control
  // that looks like it worked and did nothing.
  const routes = [...tabs.map((x) => x.key), 'help'];
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

  // ── THE TRANSITION CONTROL (docs/REBRAND.md §8 · DECISIONS §83) ────────────
  //
  // The one operator-level control over how a slide replaces the last, on every
  // screen at once. `FOLLOW` is a sentinel for "no override", not a transition —
  // an eighth entry in `TRANSITIONS` would have been a second register, and one of
  // the two would eventually have been the one somebody read.
  const FOLLOW = '';

  // A CHANGE THAT DID NOT REACH THE SCREENS PUTS THE CONTROL BACK.
  //
  // `setLiveTransition` throws (group 1 in capture.js): a congregation can see the
  // difference between a cut and an 800 ms crossfade. There is no room in a 34px
  // bar for an error chip and no need for one — the honest report is the picker
  // refusing to move, because the store is only written after the backend has
  // taken the change, and the `value=` binding then redraws the select from the
  // store. A picker that stayed on "Crossfade" over screens that were cutting
  // would be rule 35 with a dropdown.
  let xError = '';
  async function applyTransition(mode, ms) {
    try {
      xError = '';
      await setLiveTransition(mode, ms);
    } catch (e) {
      xError = humanError(e);
      // Force the selects to redraw from the store, which did NOT move.
      liveTransition.set(get(liveTransition));
    }
  }
  function pickTransition(e) {
    const mode = e.currentTarget.value;
    if (mode === FOLLOW) return applyTransition(null, null);
    return applyTransition(mode, get(liveTransition)?.ms ?? DEFAULT_TRANSITION_MS);
  }
  function pickDuration(e) {
    const cur = get(liveTransition);
    if (!cur) return; // disabled; nothing to be the duration of
    return applyTransition(cur.mode, Number(e.currentTarget.value));
  }

  // ── THE SCREEN LAMPS (docs/REBRAND.md §2) ──────────────────────────────────
  //
  // One lamp per screen in the chrome, coloured by that screen's REAL state, from
  // the same `describeScreen` verdict Live's Output Status pane and the Outputs
  // table read. Never a second opinion about a screen (rule 35; RG-01 is the
  // instance that rule was written from) — and the `SCREENS n of m` count in the
  // status bar is derived from these same rows, so the lamps and the tally cannot
  // disagree either.
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
  /** Colour law: amber = on air, amethyst = rehearsal, red = not responding,
   *  grey = everything else. Four words, and none of them invents a fifth state. */
  const LAMP_TONE = { onair: 'amber', rehearsal: 'amethyst', down: 'red' };
  // A screen name is one word wide in a 34px bar, so the lamp carries the FIRST
  // word and the title carries all of it plus what the lamp means. Truncated,
  // never wrapped: a chrome bar that grows a second row moves the whole desk down.
  const lampWord = (n) => String(n).split(/\s+/)[0];

  // ── THE STATUS BAR'S FIGURES ───────────────────────────────────────────────
  //
  // One poll, feeding every cell. `latency_report(0)` asks for no traces, so this
  // is a handful of counters rather than the diagnostic payload Settings reads.
  let perf = null;
  $: wall = wallState({
    safeMode: $safeMode,
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
    if ($safeMode) {
      try {
        await setDetection(false);
      } catch {
        /* no backend — nothing was armed in the first place */
      }
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

  <!-- Main -->
  <div class="main-v">
    <header class="topbar-v">
      <!-- THE LOCKUP, then the workspaces. A control room puts them across the
           top: the desk is wide, not tall, and a column of nav is height the
           slide grid does not get (docs/REBRAND.md §2). -->
      <span class="chrome-brand"><BrandMark size="17px" /><b>RELAY</b></span>
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
      <span class="chrome-sep" aria-hidden="true"></span>
      <!-- ON AIR must mean "the congregation is looking at something" — NOT "the
           microphone is on". It used to key off $capturing, so Relay would sit
           there pulsing ON AIR at an operator whose screens were completely blank.
           The loudest indicator in the product was answering the wrong question.

           Now: what is on the wall, right now, named. The microphone gets its own
           quieter indicator, because it is a different fact. -->
      <!-- Rehearsal outranks everything else here. Nothing is reaching the
           congregation, so the app must not say "On Air" — on ANY tab, not just
           Live. The one indicator the operator glances at cannot be tab-specific. -->
      <!-- Safe mode OUTRANKS every other state here, including rehearsal. Both
           mean "not reaching the screens", but safe mode also means the operator
           cannot change that without restarting — so it must be the thing they
           read, or they will spend the service wondering why nothing fires. -->
      {#if $safeMode}
        <span class="r-badge amethyst"><span class="bd" style="box-shadow:none;"></span>Safe mode</span>
        <span class="topbar-live r-mono">outputs disabled — turn off Safe mode in Settings › Backup</span>
      {:else if $rehearsing}
        <span class="r-badge amethyst pulse"><span class="bd"></span>Rehearsal</span>
        <span class="topbar-live r-mono">nothing is reaching the screens</span>
      {:else if $screenBlack}
        <span class="r-badge" style="border-color:var(--v-line2);color:var(--v-dim);">
          <span class="bd" style="background:var(--v-faint);box-shadow:none;"></span>Blackout
        </span>
      {:else if $live}
        <!-- AMBER. The design system's MODE INDICATORS and CLAUDE.md say the same
             thing: amber IS on air. This badge was rose — the system's Error/Panic
             colour — which put the loudest indicator in the product on the wrong
             side of the one colour law the whole app is built around. -->
        <span class="r-badge amber pulse"><span class="bd"></span>On Air</span>
        <span class="topbar-live r-mono">{liveLabel($live)}</span>
      {:else}
        <!-- Nothing is on the wall. That is a NEUTRAL state, so it gets the grey
             chip — not amber, which now means, and only means, on air. -->
        <span class="r-badge grey"><span class="bd" style="box-shadow:none;"></span>Screens clear</span>
      {/if}
      {#if $capturing}
        <span class="topbar-mic" title="Microphone is live">
          <span class="mic-dot"></span>Listening
        </span>
      {/if}
      <!-- SERVICE LOCK. A quiet chip, deliberately AFTER the state ladder and never
           part of it: it says something about the console, not about the wall, and
           it must never displace or dilute the one indicator that says whether a
           congregation is looking at something. Grey, because nothing is wrong. -->
      {#if $serviceLock.engaged}
        <span class="lockchip r-mono" title="Deletions, model changes and imports are held back while a service is recording. Nothing on the live path is affected. Lift it in Settings → History &amp; Backup.">
          PROTECTED
        </span>
      {/if}
      <span class="topbar-spring"></span>
      <!-- THE SCREEN LAMPS (docs/REBRAND.md §2). One per screen, at the top where
           the eye starts, each coloured by that screen\'s OWN state through
           `describeScreen` — the same verdict Live\'s Output Status pane and the
           Outputs table read. Never a second opinion about a screen (rule 35).

           Two decorative icons used to sit here: a "Signal" glyph that was wired
           to nothing at all, and a clock face beside a clock. A picture of a
           signal, next to a real on-air badge, in a room where the whole point is
           that indicators mean something — that is the defect this rule is about,
           drawn rather than written. The clock is in the status bar, once.

           The name is TRUNCATED to its first word and never wrapped: this bar is
           34px and a second row would push the whole desk down. The title carries
           the full name and what the lamp means. -->
      {#if screenLamps.length}
        <span class="siglamps" aria-label="Screens">
          {#each screenLamps as sc (sc.id)}
            <span class="sig" title="{sc.name} — {sc.label}{sc.note ? ` (${sc.note})` : ''}">
              <i class="lamp {LAMP_TONE[sc.kind] ?? 'grey'}"></i><span class="signm">{lampWord(sc.name)}</span>
            </span>
          {/each}
        </span>
      {/if}
      <!-- THE TRANSITION CONTROL (docs/REBRAND.md §8 · DECISIONS §83).

           TWO AUTHORITIES OVER ONE PROPERTY, AND THE PICKER IS WHAT MAKES THAT
           HONEST. §71 says a transition is a template's choice; this says an
           operator may overrule every template at once, which is a second home for
           one property unless somebody can see which home is answering. Hence the
           first option: **Follow template**. It is the default, it is what a fresh
           Relay does, and choosing anything else is a visible act with a visible
           state — not a preference silently sitting on top of a saved one.

           The duration is disabled while the template is being followed, because
           there is nothing for it to be the duration OF: a number an operator can
           set that changes nothing is the §69 defect, and it is what the old theme
           editor's transition control was for the whole of its life.

           IT GIVES WAY, ALWAYS. It sits before Emergency Stop and after the screen
           lamps, and below 1180px it is not rendered at all — the panic control
           lives at a fixed screen corner an operator hits without reading (rule 15,
           DECISIONS §20) and a picker may never be the reason that corner moved. -->
      <span class="xfade" class:on={$liveTransition}>
        <span class="xcap">Transition</span>
        <select
          class="r-select xpick r-focus"
          aria-label="Slide transition"
          title="How one slide replaces the last, on every screen. Follow template leaves each template's own choice alone."
          value={$liveTransition?.mode ?? FOLLOW}
          on:change={pickTransition}>
          <option value={FOLLOW}>Follow template</option>
          {#each TRANSITIONS as x}<option value={x.id}>{x.label}</option>{/each}
        </select>
        <select
          class="r-select xpick xdur r-focus"
          aria-label="Transition duration"
          title="How long it runs. Only meaningful while an override is in force."
          disabled={!$liveTransition}
          value={String($liveTransition?.ms ?? DEFAULT_TRANSITION_MS)}
          on:change={pickDuration}>
          {#each TRANSITION_MS as ms}<option value={String(ms)}>{ms} ms</option>{/each}
        </select>
        <!-- A CHANGE THAT DID NOT REACH THE SCREENS SAYS SO. The selects have
             already snapped back to what the screens are actually doing; this says
             why, so the operator is not left wondering whether they mis-clicked.
             It reads differently when it is broken from when it is fine, which is
             the whole of rule 35. -->
        {#if xError}
          <span class="xerr" role="status" title={xError}>not applied</span>
        {/if}
      </span>
      <!-- EMERGENCY STOP stays at the far right of the chrome, and this is where
           the prototype puts its transition picker. A panic control lives at a
           fixed screen corner an operator can hit without reading (rule 15,
           DECISIONS §20) — that corner is the one thing in this bar that may never
           move, so the picker above gives way rather than this. -->
      <button class="r-btn danger sm" on:click={clearScreens} title="Blank every output screen">Emergency Stop</button>
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
      <span class="st" title="Median time from the audio to a transcript update on this screen (audio_to_partial_transcript). Settings → Diagnostics has the whole report.">
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

  <!-- DEGRADED. One line, always, on every tab — opened for the detail. It sits
       BELOW the panic banner (a panic control that failed outranks everything) and
       ABOVE the update banners, because "something is working less well right now"
       is more urgent than "there is a new version". -->
  {#if degLevel}
    <div class="deg" class:blocked={degLevel === 'blocked'} role="status">
      <button
        type="button"
        class="deg-head"
        aria-expanded={degOpen}
        on:click={() => (degOpen = !degOpen)}
      >
        <span class="deg-dot"></span>
        <span class="deg-sum">{summarise(degraded)}</span>
        <span class="deg-more r-mono">{degOpen ? 'hide' : `${degraded.length} detail${degraded.length === 1 ? '' : 's'}`}</span>
      </button>
      {#if degOpen}
        <ul class="deg-list">
          {#each degraded as d (d.id)}
            <li class="deg-item" class:blocked={d.level === 'blocked'}>
              <b>{d.title}</b>
              <span>{d.what}</span>
              <!-- Every row says what to do, or admits there is nothing. "Degraded"
                   on its own is a mood, not information. -->
              <i>{d.fix}</i>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}

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
