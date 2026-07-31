<script>
  import { onMount, onDestroy } from 'svelte';
  import { trapFocus } from './lib/focus.js';
  import { t } from './lib/i18n.js';
  import { capture, capturing, detectionOn, live, screenBlack, rehearsing, initAudio, autoOpenOutputs, setDetection, clearScreens, blackScreen, panicError, dismissPanicError } from './lib/stores/capture.js';
  import { installShortcuts, cheatsheet, liveShortcuts } from './lib/shortcuts.js';
  import { installLeaveGuard } from './lib/crash.js';
  import { session, setSession } from './lib/session.js';
  import FirstRun from './lib/FirstRun.svelte';
  import Splash from './lib/Splash.svelte';
  import BootSequence from './lib/boot/BootSequence.svelte';
  import BrandMark from './lib/ui/BrandMark.svelte';
  import { safeMode } from './lib/boot/boot.js';
  import {
    checkForUpdate,
    installUpdate,
    dismissUpdate,
    updateAvailable,
    updateProgress,
    updateError,
  } from './lib/updater.js';
  // Views are CODE-SPLIT. Statically importing all eight put every tab — Live
  // (the heaviest), the Planner, Settings, all of them — into one 637 KB bundle
  // the webview had to parse before the first frame, on hardware that is often a
  // borrowed church laptop. Now the boot bundle is the shell plus whichever tab
  // the operator was last on; each other view's chunk loads on first visit (all
  // from local disk — no network, offline-safe) and an idle prefetch warms the
  // rest so switching stays instant after boot.
  const viewLoaders = {
    live:      () => import('./lib/views/Live.svelte'),
    channels:  () => import('./lib/views/Channels.svelte'),
    templates: () => import('./lib/views/Templates.svelte'),
    themes:    () => import('./lib/views/Themes.svelte'),
    library:   () => import('./lib/views/Library.svelte'),
    planner:   () => import('./lib/views/ServicePlanner.svelte'),
    settings:  () => import('./lib/views/Settings.svelte'),
    help:      () => import('./lib/views/Help.svelte'),
  };
  const viewCache = {}; // key → resolved component, loaded once then kept
  let current = null;   // the component for the active tab (null while its chunk loads)
  let viewLoadError = null;
  let viewLoadToken = 0;

  // `label` is an i18n KEY. The tab strip is the first thing a volunteer looks at and
  // the last thing they should have to read in a second language.
  const tabs = [
    // Dashboard is FIRST but is not where a returning operator lands — the active
    // tab is persisted (session.js), so only a genuinely fresh install starts
    // here. Someone who was on Live yesterday is on Live today.
    // Dashboard is no longer a top-level tab — it lives inside Settings (a
    // records/overview surface, not a run surface). The sidebar is the surfaces an
    // operator actually runs during a service.
    { key: 'live',      label: 'tab.live',      title: 'Live Service' },
    // Outputs — the ONE surface for every render target: the congregation wall,
    // stage/confidence/preacher monitors, streaming and lobby screens. Each is a
    // real backend channel (native window or LAN/OBS URL over :8032) with its own
    // template. This absorbed the old localStorage-only "Stage Displays" gallery,
    // which looked lovely but never actually reached a screen — one real surface
    // instead of two, one of them a phantom.
    { key: 'channels',  label: 'tab.channels',  title: 'Outputs' },
    { key: 'templates', label: 'tab.templates', title: 'Templates' },
    // Themes — the style layer BENEATH templates (typography, colour, rhythm). A
    // template inherits a theme and overrides it per key; a theme never reaches a
    // wall on its own. Sits next to Templates because they are one pipeline: pick
    // a look here, apply it to a template, fire the template.
    { key: 'themes',    label: 'tab.themes',    title: 'Themes' },
    { key: 'library',   label: 'tab.library',   title: 'Content Library' },
    { key: 'planner',   label: 'tab.planner',   title: 'Service Planner' },
    // Service History now lives INSIDE Settings (its own section) — a record of
    // past services is a config/records surface, not a top-level run tab, and
    // folding it in keeps the sidebar to the seven surfaces an operator runs.
    { key: 'settings',  label: 'tab.settings',  title: 'System Settings' },
    // In-app help. There was NONE — the operator guide was a markdown file on
    // GitHub, which is exactly no use to a volunteer in a dark booth on a Sunday
    // with no internet. Help that needs a network is missing when Relay is most
    // useful: offline.
    { key: 'help',      label: 'tab.help',      title: 'Help / Shortcuts' },
  ];
  // The active tab IS the session — not a local copy of it that happens to be
  // written back. One direction, one source of truth, so anything can navigate:
  // the Planner's "Run this plan" hands the operator to LIVE by setting it, and a
  // reload (or a crash + Recover) puts them back on the tab they were on.
  // `stagedisplays` was merged into Outputs (the `channels` tab); an operator
  // whose persisted session still points at it is sent to Outputs, not dumped
  // back on Live.
  $: requestedTab = $session.activeTab === 'stagedisplays' ? 'channels' : $session.activeTab;
  $: active = tabs.some((x) => x.key === requestedTab) ? requestedTab : 'live';
  const go = (key) => setSession({ activeTab: key });
  $: currentTab = tabs.find((x) => x.key === active) ?? tabs[0];

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
    dashboard: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><rect x="3" y="3" width="7.5" height="8.5" rx="1.6"/><rect x="13.5" y="3" width="7.5" height="5.5" rx="1.6"/><rect x="3" y="14.5" width="7.5" height="6.5" rx="1.6"/><rect x="13.5" y="11.5" width="7.5" height="9.5" rx="1.6"/></svg>',
    live: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7"><rect x="2.5" y="4.5" width="19" height="13" rx="2"/><path d="M8 21h8M12 17.5V21" stroke-linecap="round"/><circle cx="12" cy="11" r="2.6" fill="currentColor" stroke="none"/></svg>',
    channels: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3"/><circle cx="4" cy="12" r="2"/><circle cx="12" cy="6" r="2"/><circle cx="20" cy="14" r="2"/></svg>',
    templates: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="m12 2 9 5-9 5-9-5 9-5Z"/><path d="m3 12 9 5 9-5M3 17l9 5 9-5"/></svg>',
    themes: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><circle cx="13.5" cy="6.5" r="1.3"/><circle cx="17.5" cy="10.5" r="1.3"/><circle cx="8.5" cy="7.5" r="1.3"/><circle cx="6.5" cy="12.5" r="1.3"/><path d="M12 2a10 10 0 1 0 0 20 2.5 2.5 0 0 0 2-4 2.5 2.5 0 0 1 2-4h1a5 5 0 0 0 5-5 9 9 0 0 0-9-7Z"/></svg>',
    library: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linejoin="round"><path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2V5Z"/><path d="M9 3v14"/></svg>',
    history: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 3-6.7"/><path d="M3 4v4h4"/><path d="M12 7v5l3 2"/></svg>',
    planner: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4"/><path d="M7 13h4M7 17h7"/></svg>',
    stagedisplays: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="2.5" y="4" width="19" height="13" rx="2"/><path d="M8 21h8M12 17v4"/></svg>',
    help: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><circle cx="12" cy="12" r="9"/><path d="M9.6 9a2.5 2.5 0 0 1 4.8.9c0 1.7-2.4 2.1-2.4 3.6"/><circle cx="12" cy="17" r=".6" fill="currentColor"/></svg>',
    settings: '<svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" stroke-width="1.7"><circle cx="12" cy="12" r="3.2"/><path d="M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-2.7 1.1V21a2 2 0 1 1-4 0v-.1A1.6 1.6 0 0 0 7 19.4a1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0-1.1-2.7H1a2 2 0 1 1 0-4h.1A1.6 1.6 0 0 0 2.6 7a1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1A1.6 1.6 0 0 0 7 2.6h.1A1.6 1.6 0 0 0 8 1.1V1a2 2 0 1 1 4 0v.1A1.6 1.6 0 0 0 15 2.6a1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0 1.1 2.7h.1a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1Z"/></svg>',
  };

  let clock = '';
  let timer;
  function tick() {
    clock = new Date().toLocaleTimeString('en-GB');
  }

  let engineOnline = false;
  let teardownKeys;
  let teardownLeave;

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
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('greet', { name: 'operator' });
      engineOnline = true;
    } catch {
      engineOnline = false;
    }
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
  <!-- Sidebar -->
  <aside class="side">
    <!-- The lockup from the design sheet's BRAND block: the waveform mark, then
         the wordmark. The sidebar carried the word alone — the app's own logo
         appeared nowhere in the app. -->
    <div class="side-brand">
      <BrandMark size="22px" />
      <span>RELAY</span>
    </div>
    <div class="side-profile">
      <div class="side-avatar">
        <svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><path d="M12 2 4 7v13h16V7l-8-5Z"/><path d="M12 6v5M9.5 8.5h5M9 20v-4a3 3 0 0 1 6 0v4"/></svg>
      </div>
      <div class="who"><b>Relay Console</b><span>Live Service</span></div>
    </div>

    <nav class="nav">
      {#each tabs as tab}
        <button class="nav-item r-focus" class:active={tab.key === active} on:click={() => go(tab.key)}>
          <span class="ic">{@html icons[tab.key]}</span>
          <span class="nav-label">{$t(tab.label)}</span>
        </button>
      {/each}
    </nav>

    <div class="side-foot">
      <div class="row">
        <span class="k">AI Signal</span>
        <!-- Emerald, not amber. `engineOnline` is true whenever the backend is
             attached (essentially always), so amber here would burn permanently —
             and amber is the tally light: it may only ever mean the congregation
             is looking at something on air (CLAUDE.md rule 18). -->
        <span class="dot" style="background:{engineOnline ? 'var(--v-emerald)' : 'var(--v-faint)'};"></span>
      </div>
      <div class="m">Engine {engineOnline ? 'online' : 'offline'}</div>
      <div class="m">Detection {$detectionOn ? 'active' : 'off'}</div>
    </div>
  </aside>

  <!-- Main -->
  <div class="main-v">
    <header class="topbar-v">
      <h1 class="topbar-title">{currentTab.title}</h1>
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
      <span class="topbar-spring"></span>
      <div class="topbar-icons">
        <span class="ib" title="Signal"><svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4.9 16.1a10 10 0 0 1 14.2 0M8 13a5.5 5.5 0 0 1 8 0"/><circle cx="12" cy="19" r="1.4" fill="currentColor" stroke="none"/></svg></span>
        <span class="ib" title="Clock"><svg viewBox="0 0 24 24" width="19" height="19" fill="none" stroke="currentColor" stroke-width="1.7"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2" stroke-linecap="round"/></svg></span>
        <span class="clock r-mono" style="font-size:13px;color:var(--v-dim);">{clock}</span>
      </div>
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

    <footer class="footer-v">
      <div class="fl">
        <b>Relay AI</b>
        <span>Detection {$detectionOn ? 'ACTIVE' : 'OFF'}</span>
      </div>
      <div style="display:flex;align-items:center;gap:8px;">
        <span class="dot" style="width:6px;height:6px;border-radius:50%;background:{$safeMode || $rehearsing ? 'var(--v-amethyst)' : $live && !$screenBlack ? 'var(--v-amber)' : 'var(--v-faint)'};"></span>
        {$safeMode ? 'SAFE MODE' : $rehearsing ? 'REHEARSAL' : $screenBlack ? 'BLACKOUT' : $live ? 'ON AIR' : 'SCREENS CLEAR'}
      </div>
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
