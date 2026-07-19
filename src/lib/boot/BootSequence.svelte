<script>
  // LAUNCH & STARTUP — the sequencer.
  //
  // Owns the order of section 1 of docs/relaydesign/relayscreens.md:
  //
  //   Splash  →  [gate: crash / safe mode]  →  Diagnostics  →  Hardware
  //           →  Integrations  →  Database  →  [gate: update]  →  [gate: recover]
  //           →  the console
  //
  // Gates OUTRANK stages, and the order within them is a priority order, not a
  // taste: a crash outranks a resume, a resume outranks an update. That is the
  // order of "what is the operator most likely to be surprised by".
  //
  // ── Why this is skippable ─────────────────────────────────────────────────
  //
  // This whole sequence is DECORATION OVER FACTS, exactly like the splash. It
  // must never be the reason someone cannot reach the console. So:
  //
  //   · Every stage has a hard cap. A wedged probe does not wedge the boot.
  //   · Esc skips straight to the console at any point in a stage (never over a
  //     gate — a gate is a question that has to be answered).
  //   · A boot with nothing to say (no crash, no update, no resume, no failed
  //     check) collapses to the splash alone. An operator who launches Relay on
  //     a healthy machine should not be made to click through four screens to
  //     get to work; the full sequence is for the boot that has news.

  import { onMount } from 'svelte';
  import {
    STAGES,
    checks,
    stage,
    gate,
    booting,
    bootRecord,
    clearCrash,
    setSafeMode,
    safeMode,
    runStage,
    rollUp,
    hasResumePoint,
    resetBoot,
  } from './boot.js';
  import { makeProbes } from './probes.js';
  import { session, setSession, clearSession } from '../session.js';
  import {
    updateAvailable,
    updateProgress,
    updateError,
    installUpdate,
    dismissUpdate,
  } from '../updater.js';

  import BootDiagnostics from './BootDiagnostics.svelte';
  import HardwareCheck from './HardwareCheck.svelte';
  import PluginLoading from './PluginLoading.svelte';
  import DatabaseMigration from './DatabaseMigration.svelte';
  import RecoverSession from './RecoverSession.svelte';
  import CrashReportRecovery from './CrashReportRecovery.svelte';
  import SafeModeStartup from './SafeModeStartup.svelte';
  import UpdateAvailableGate from './UpdateAvailable.svelte';

  export let version = '';
  /** Injected in tests; the real probes hit Tauri. */
  export let probes = null;
  /** Called exactly once, when the sequence hands over to the console. */
  export let onDone = () => {};

  const SCREENS = {
    diagnostics: BootDiagnostics,
    hardware: HardwareCheck,
    plugins: PluginLoading,
    migration: DatabaseMigration,
  };

  /** True while a stage is holding for the operator's button. */
  let paused = false;
  let finished = false;

  function finish() {
    if (finished) return;
    finished = true;
    booting.set(false);
    onDone();
  }

  /**
   * The operator pressed Continue on a held stage. Resume the RUN from the next
   * stage — not merely swap the screen, or the remaining probes never fire.
   */
  function next(from) {
    paused = false;
    runFrom(STAGES.indexOf(from) + 1);
  }

  /** The gates that come AFTER the checks: update, then resume. */
  function afterStages() {
    if ($updateAvailable) return gate.set('update');
    if (hasResumePoint($session)) return gate.set('recover');
    finish();
  }

  // ── Gate handlers ─────────────────────────────────────────────────────────

  function crashContinue() {
    clearCrash();
    gate.set(null);
    runAll();
  }
  function crashToSafeMode() {
    clearCrash();
    gate.set('safemode');
  }
  function enterSafeMode() {
    setSafeMode(true);
    gate.set(null);
    runAll();
  }
  function declineSafeMode() {
    setSafeMode(false);
    gate.set(null);
    runAll();
  }
  function updateLater() {
    dismissUpdate();
    gate.set(null);
    if (hasResumePoint($session)) return gate.set('recover');
    finish();
  }
  function resume() {
    // The POSITION comes back. `liveOnAir` does not — see RecoverSession.svelte.
    // Amber means the congregation is looking at something, and it is never
    // allowed to be true because an app restarted.
    setSession({ liveOnAir: false });
    gate.set(null);
    finish();
  }
  function startFresh() {
    clearSession();
    gate.set(null);
    finish();
  }

  // ── The run ───────────────────────────────────────────────────────────────

  /** Hard cap per stage. A wedged probe must never wedge a boot. */
  const STAGE_CAP_MS = 6000;
  /** How long a CLEAN stage stays up before advancing itself. */
  const DWELL_MS = 700;
  /**
   * A stage with warnings stays up longer — but still advances itself.
   *
   * It used to HOLD for a click on anything short of clean, which was fine while
   * the checks were stubs and nothing warned. The moment the integration probes
   * became real, the normal state of a church laptop — no OBS on :4455, no ATEM
   * on :9910, no NDI SDK in the build — produced three warnings on EVERY boot,
   * and the sequence stopped dead waiting for a click each time.
   *
   * That is the "clicked through blindly" failure: a gate that fires every single
   * launch stops being read by the second week, and then the one boot that
   * mattered gets dismissed with the same reflex. Only a FAILURE holds now.
   */
  const WARN_DWELL_MS = 2600;

  const runAll = () => runFrom(0);

  /**
   * Which run is current. Every `runFrom` takes the next token and abandons
   * itself the moment another run starts.
   *
   * Needed because a warning-stage now DWELLS rather than holding, while its
   * Continue button is on screen: an operator clicking during the dwell would
   * otherwise start a second loop alongside the first, and two loops writing
   * `stage` race each other through the remaining screens.
   */
  let runToken = 0;

  async function runFrom(start) {
    const token = ++runToken;
    const p = probes ?? makeProbes();
    for (const name of STAGES.slice(start)) {
      if (token !== runToken) return; // superseded
      stage.set(name);
      await Promise.race([
        runStage(name, p),
        new Promise((r) => setTimeout(r, STAGE_CAP_MS)),
      ]);
      if (token !== runToken) return;
      const verdict = rollUp($checks[name] ?? []);
      // Only a FAILURE holds. A clean stage shows its result briefly; a stage
      // with warnings lingers so they can be read, then advances itself. See
      // WARN_DWELL_MS for why warnings must not block.
      if (verdict !== 'fail') {
        await new Promise((r) => setTimeout(r, verdict === 'warn' ? WARN_DWELL_MS : DWELL_MS));
        continue;
      }
      paused = true;
      return; // the stage screen's own button resumes via next()
    }
    if (token !== runToken) return;
    stage.set(null);
    afterStages();
  }

  function retry() {
    resetBoot();
    runAll();
  }

  // Esc skips the checks. NOT the gates — a gate is a question.
  function onKey(e) {
    if (e.key === 'Escape' && !$gate && !finished) {
      e.preventDefault();
      finish();
    }
  }

  onMount(() => {
    const r = $bootRecord;
    // A crash outranks everything. Ask about it before running any checks —
    // the checks are what may crash again.
    if (r.lastCrash) {
      gate.set('crash');
    } else if ((r.crashStreak ?? 0) >= 3 && !r.safeMode) {
      gate.set('safemode');
    } else {
      runAll();
    }
  });
</script>

<svelte:window on:keydown={onKey} />

{#if $stage && SCREENS[$stage]}
  <svelte:component
    this={SCREENS[$stage]}
    {version}
    safe={$safeMode}
    onContinue={() => next($stage)}
    onRetry={retry}
  />
{/if}

{#if $gate === 'crash'}
  <CrashReportRecovery
    crash={$bootRecord.lastCrash}
    streak={$bootRecord.crashStreak}
    onContinue={crashContinue}
    onSafeMode={crashToSafeMode}
  />
{:else if $gate === 'safemode'}
  <SafeModeStartup
    streak={$bootRecord.crashStreak}
    onEnter={enterSafeMode}
    onNormal={declineSafeMode}
  />
{:else if $gate === 'update'}
  <UpdateAvailableGate
    update={$updateAvailable}
    current={version}
    progress={$updateProgress}
    error={$updateError}
    onInstall={installUpdate}
    onLater={updateLater}
  />
{:else if $gate === 'recover'}
  <RecoverSession session={$session} onResume={resume} onFresh={startFresh} />
{/if}
