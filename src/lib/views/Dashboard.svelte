<script>
  // DASHBOARD — "am I ready?", answered before anyone is in the room.
  //
  // ── Why this screen is allowed to exist ────────────────────────────────────
  //
  // A dashboard is the easiest screen in any product to fill with duplicates of
  // other screens. This one earns its place by answering a question no other tab
  // does: it is 10:20 on a Sunday, the service is at 11:00 — **is this machine
  // going to work?**
  //
  // The Boot Diagnostics screen answers exactly that, and then vanishes: it runs
  // once, at launch, and the operator who arrives forty minutes later never sees
  // it. So System Health here is not a second health check — it is *the same
  // check*, the same list and the same probes from `lib/boot/`, re-run on
  // demand. Two independently-written health panels would eventually disagree,
  // and then the app would be arguing with itself about whether it works.
  //
  // Everything else here is a SHORTCUT to a real surface, never a copy of one:
  // the recent services link into Library, the plan into Planner. Nothing is
  // editable from this screen.
  //
  // NOTHING HERE CAN PUT ANYTHING ON A SCREEN. The quick actions open an output
  // window (blank), arm the microphone, or toggle rehearsal. Firing content is
  // the Live tab's job and stays there — a "go live" button on a summary screen
  // is how the wrong thing reaches a congregation.

  import { onMount } from 'svelte';
  import { freshChecks, runChecks, rollUp } from '../boot/boot.js';
  import { makeProbes } from '../boot/probes.js';
  import CheckList from '../boot/CheckList.svelte';
  import { humanError } from '../errors.js';
  import { setSession } from '../session.js';
  import { safeMode } from '../boot/boot.js';
  import {
    capture,
    capturing,
    detectionOn,
    rehearsing,
    live,
    listServices,
    listPlans,
    listOutputChannels,
    openChannelOutput,
    startCapture,
    stopCapture,
    setRehearsal,
  } from '../stores/capture.js';

  let health = freshChecks().diagnostics;
  let checking = true;
  let services = [];
  let plans = [];
  let channels = [];
  let error = '';
  let busy = '';

  $: verdict = rollUp(health);
  $: failures = health.filter((c) => c.state === 'fail');
  $: warnings = health.filter((c) => c.state === 'warn');

  // ONE CAUSE IS ONE FACT. When every check fails with the same message — the
  // engine is not attached, so nothing can be probed — repeating that sentence
  // once per row turns a single problem into a wall of red the operator has to
  // read six times to discover it said the same thing six times.
  //
  // So: say it once, in the hero, and let the rows stay terse.
  $: commonCause =
    failures.length > 1 && failures.every((c) => c.note && c.note === failures[0].note)
      ? failures[0].note
      : '';
  $: healthRows = commonCause
    ? health.map((c) => (c.state === 'fail' ? { ...c, note: 'not answered' } : c))
    : health;

  async function refresh() {
    checking = true;
    error = '';
    try {
      health = await runChecks(freshChecks().diagnostics, makeProbes(), (partial) => {
        health = partial;
      });
    } catch (e) {
      error = humanError(e);
    }
    checking = false;
  }

  onMount(async () => {
    // Deliberately not awaited together with the checks: the lists are cheap and
    // should paint immediately, while the probes land one at a time.
    listServices().then((s) => (services = s ?? []));
    listPlans().then((p) => (plans = p ?? []));
    listOutputChannels().then((c) => (channels = c ?? []));
    refresh();
  });

  async function act(name, fn) {
    busy = name;
    error = '';
    try {
      await fn();
    } catch (e) {
      error = humanError(e);
    }
    busy = '';
  }

  const openMain = () =>
    act('output', async () => {
      const ch =
        channels.find((c) => c.render_target === 'native_window' && c.name === 'Main screen') ??
        channels.find((c) => c.render_target === 'native_window');
      if (!ch) throw new Error('No screen is configured yet. Add one in Outputs.');
      await openChannelOutput(ch.id);
    });

  const toggleMic = () =>
    act('mic', async () => {
      if ($capturing) await stopCapture();
      else await startCapture($capture.inputDevice || null);
    });

  const toggleRehearsal = () => act('rehearse', () => setRehearsal(!$rehearsing));

  const go = (tab) => setSession({ activeTab: tab });

  // A service row's date is an ISO string from SQLite; show it the way a person
  // running a Sunday would say it.
  const when = (iso) => {
    const d = new Date(iso);
    return Number.isNaN(d.getTime())
      ? iso
      : d.toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'short' });
  };
  const mins = (secs) => `${Math.max(1, Math.round((secs ?? 0) / 60))} min`;
</script>

<div class="dash">
  <!-- The verdict. ONE sentence, and it is the first thing on the screen —
       an operator scanning this for three seconds should get the answer without
       reading a single row below. -->
  <section class="d-hero" class:bad={verdict === 'fail'} class:warn={verdict === 'warn'}>
    <div class="d-hero-t">
      <p class="r-lbl">Readiness</p>
      <h2>
        {#if checking}
          Checking this machine…
        {:else if verdict === 'fail'}
          {commonCause
            ? 'Nothing could be checked.'
            : failures.length === 1
              ? `${failures[0].label} is not working.`
              : `${failures.length} things not working.`}
        {:else if verdict === 'warn'}
          Ready, with {warnings.length === 1 ? 'one thing' : `${warnings.length} things`} worth a look.
        {:else}
          Ready for a service.
        {/if}
      </h2>
      <p class="d-hero-p">
        {#if checking}
          Running the same checks Relay runs at startup.
        {:else if commonCause}
          {commonCause}
        {:else if verdict === 'fail'}
          {#if failures.some((c) => c.id === 'database')}
            Scripture cannot be looked up, so nothing can be put on a screen.
          {:else if failures.some((c) => c.id === 'audio')}
            There is no microphone, so nothing will be transcribed — you can still fire verses by hand.
          {:else}
            Relay will start, but the affected feature will not work.
          {/if}
        {:else if verdict === 'warn'}
          None of it stops a service. Worth sorting before people arrive, not during.
        {:else}
          Engine, scripture, microphone and speech model all answered.
        {/if}
      </p>
    </div>
    <button class="r-btn ghost sm" on:click={refresh} disabled={checking}>
      {checking ? 'Checking…' : 'Re-check'}
    </button>
  </section>

  {#if $safeMode}
    <p class="d-safe">
      <b>Safe mode is on.</b> Outputs will not open and detection is disarmed — nothing
      Relay does can reach a screen. Turn it off in Settings before you run a service.
    </p>
  {/if}

  <div class="d-grid">
    <!-- SYSTEM HEALTH — the boot check, re-run. Same list, same probes. -->
    <section class="d-card d-health">
      <header><h3>System health</h3><span class="r-lbl">the startup checks, live</span></header>
      <CheckList items={healthRows} />
    </section>

    <div class="d-side">
      <!-- QUICK ACTIONS. Nothing here fires content. -->
      <section class="d-card">
        <header><h3>Quick actions</h3></header>
        <div class="d-acts">
          <button class="d-act" on:click={openMain} disabled={busy === 'output' || $safeMode}>
            <b>Open the congregation screen</b>
            <span>Opens the output window. It starts blank.</span>
          </button>
          <button class="d-act" on:click={toggleMic} disabled={busy === 'mic' || $safeMode}>
            <b>{$capturing ? 'Stop listening' : 'Start listening'}</b>
            <span>
              {$capturing
                ? 'The microphone is live right now.'
                : 'Arms the microphone so Relay can hear the sermon.'}
            </span>
          </button>
          <button class="d-act" class:on={$rehearsing} on:click={toggleRehearsal} disabled={busy === 'rehearse'}>
            <b>{$rehearsing ? 'Leave rehearsal' : 'Rehearse'}</b>
            <span>
              {$rehearsing
                ? 'Nothing is reaching the congregation.'
                : 'Practise the whole run with nothing reaching a screen.'}
            </span>
          </button>
          <button class="d-act" on:click={() => go('live')}>
            <b>Go to the run surface</b>
            <span>Live is where a service is actually run.</span>
          </button>
        </div>
      </section>

      <!-- TODAY'S PLAN. A pointer into Planner, never an editor. -->
      <section class="d-card">
        <header><h3>Service plans</h3></header>
        {#if plans.length}
          <ul class="d-list">
            {#each plans.slice(0, 3) as p}
              <li>
                <button class="d-row" on:click={() => go('planner')}>
                  <b>{p.title ?? p.name ?? `Plan #${p.id}`}</b>
                  <span class="r-mono">#{p.id}</span>
                </button>
              </li>
            {/each}
          </ul>
        {:else}
          <p class="d-empty">
            No plans yet. Build one in <button class="d-link" on:click={() => go('planner')}>Planner</button>
            — or run the service straight from the AI and the manual box in Live.
          </p>
        {/if}
      </section>
    </div>
  </div>

  <!-- RECENT SERVICES — real rows from the service history. -->
  <section class="d-card">
    <header>
      <h3>Recent services</h3>
      {#if services.length}
        <button class="d-link" on:click={() => go('library')}>All history</button>
      {/if}
    </header>
    {#if services.length}
      <table class="d-table">
        <thead>
          <tr><th>Service</th><th>When</th><th>Length</th><th>Verses</th><th>Overrides</th></tr>
        </thead>
        <tbody>
          {#each services.slice(0, 5) as s}
            <tr>
              <td class="d-t">{s.title || 'Untitled service'}</td>
              <td>{when(s.date)}</td>
              <td class="r-mono">{mins(s.duration_secs)}</td>
              <td class="r-mono">{s.verses}</td>
              <!-- Overrides are not a failure count. A human taking control is the
                   product working, so this column is never painted as an error. -->
              <td class="r-mono">{s.overrides}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="d-empty">
        No services recorded yet. Relay writes one automatically the first time you start
        listening.
      </p>
    {/if}
  </section>

  {#if error}<div class="d-err">{error}</div>{/if}
</div>

<style>
  .dash {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1180px;
  }

  .d-hero {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    padding: 20px 22px;
    border-radius: var(--v-r-lg);
    background: var(--v-surf);
    /* A FULL coloured outline, not a thick left bar. The design sheet's CARDS
       block states the pattern: its Selected/Active card is a 1px amethyst
       outline and its Status card a 1px cyan one — no side accents anywhere.
       Amethyst for "worth a look", red for "broken", green for ready. Never
       amber: nothing on this screen is on air, and amber is the tally light
       (DECISIONS §22). */
    border: 1px solid rgba(34, 197, 94, 0.45);
  }
  .d-hero.warn {
    border-color: var(--v-accent-line);
  }
  .d-hero.bad {
    border-color: rgba(239, 68, 68, 0.5);
  }
  .d-hero-t {
    flex: 1;
    min-width: 0;
  }
  .d-hero h2 {
    margin: 7px 0 0;
    font-family: var(--f-head);
    font-size: var(--v-fs-h1);
    line-height: var(--v-lh-h1);
    letter-spacing: var(--v-tr-tight);
    font-weight: 600;
    color: var(--v-txt);
  }
  .d-hero-p {
    margin: 6px 0 0;
    font-size: var(--v-fs-b1);
    line-height: 1.6;
    color: var(--v-dim);
    max-width: 62ch;
  }

  .d-safe {
    margin: 0;
    padding: 12px 16px;
    border-radius: var(--v-r-md);
    background: var(--v-accent-soft);
    border: 1px solid var(--v-accent-line);
    font-size: var(--v-fs-b2);
    line-height: 1.6;
    color: var(--v-dim);
  }
  .d-safe b {
    color: var(--v-accent2);
  }

  .d-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
    gap: 16px;
    align-items: start;
  }
  .d-side {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  .d-card {
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    padding: 18px 20px 20px;
    min-width: 0;
  }
  .d-card header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 12px;
  }
  .d-card h3 {
    margin: 0;
    font-family: var(--f-head);
    font-size: var(--v-fs-h3);
    font-weight: 600;
    color: var(--v-txt);
  }
  /* The health card reuses the boot .b-checks list, which brings its own border
     and background — so it must not sit inside a second one. */
  .d-health :global(.b-checks) {
    background: transparent;
    border-color: var(--v-line);
  }

  .d-acts {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .d-act {
    display: block;
    width: 100%;
    text-align: left;
    padding: 12px 14px;
    border-radius: var(--v-r-md);
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    color: var(--v-txt);
    font: inherit;
    cursor: pointer;
    transition: border-color 0.14s, background 0.14s;
  }
  .d-act:hover:not(:disabled) {
    border-color: var(--v-accent-line);
    background: var(--v-surf3);
  }
  .d-act:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .d-act.on {
    border-color: var(--v-accent-line);
    background: var(--v-accent-soft);
  }
  .d-act b {
    display: block;
    font-size: 13.5px;
    font-weight: 600;
  }
  .d-act span {
    display: block;
    margin-top: 3px;
    font-size: var(--v-fs-b2);
    color: var(--v-faint);
    line-height: 1.5;
  }

  .d-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .d-row {
    display: flex;
    width: 100%;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 12px;
    border-radius: var(--v-r-md);
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    color: var(--v-txt);
    font: inherit;
    cursor: pointer;
  }
  .d-row:hover {
    border-color: var(--v-accent-line);
  }
  .d-row b {
    font-size: 13px;
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .d-row span {
    font-size: 11px;
    color: var(--v-faint);
    flex: 0 0 auto;
  }

  .d-table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--v-fs-b2);
  }
  .d-table th {
    text-align: left;
    font-family: var(--f-mono);
    font-size: 9.5px;
    font-weight: 600;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--v-faint);
    padding: 0 12px 8px 0;
    border-bottom: 1px solid var(--v-line);
  }
  .d-table td {
    padding: 11px 12px 11px 0;
    color: var(--v-dim);
    border-bottom: 1px solid var(--v-line);
  }
  .d-table tr:last-child td {
    border-bottom: 0;
  }
  .d-table .d-t {
    color: var(--v-txt);
    font-weight: 500;
  }

  .d-empty {
    margin: 0;
    font-size: var(--v-fs-b2);
    line-height: 1.65;
    color: var(--v-faint);
  }
  .d-link {
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    font-size: inherit;
    color: var(--v-accent2);
    cursor: pointer;
    text-decoration: underline;
  }

  .d-err {
    padding: 11px 13px;
    border-radius: var(--v-r-md);
    background: var(--v-red-soft);
    border: 1px solid rgba(239, 68, 68, 0.3);
    color: var(--v-txt);
    font-size: 12.5px;
    line-height: 1.55;
  }

  @media (max-width: 980px) {
    .d-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
