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
  // the services link into History, the plan into Planner. Nothing is editable
  // from this screen.
  //
  // NOTHING HERE CAN PUT ANYTHING ON A SCREEN, and since the quick actions were
  // deleted nothing here can change the machine's state at all. It reports and it
  // points; the controls live where they live. Firing content is the Live tab's
  // job — a "go live" button on a summary screen is how the wrong thing reaches a
  // congregation — and arming the microphone or entering rehearsal is the dock's,
  // which is in the shell and therefore already on this screen.
  //
  // THIS IS SECTION ONE OF SETTINGS and it is the section, not a card in it. It
  // stacks in one column because the reading column it renders into is capped at
  // 880px (docs/REBRAND.md §11): a two-up card grid inside that is two ~420px
  // columns, and the health list's own rows already wrap at 325px.

  import { onMount } from 'svelte';
  import { freshChecks, runChecks, rollUp } from '../boot/boot.js';
  import { makeProbes } from '../boot/probes.js';
  import CheckList from '../boot/CheckList.svelte';
  import { humanError } from '../errors.js';
  import { setSession } from '../session.js';
  import { modelLabel } from '../statusbar.js';
  import { safeMode } from '../boot/boot.js';
  // ── WHAT A SERVICE IS ACTUALLY DECIDED BY (RG-116) ────────────────────────
  //
  // This screen said "Ready for a service." and "Engine, scripture, microphone and
  // speech model all answered." while naming NEITHER the model NOR the recognition
  // language, and those are the two settings the 2026-09-06 service showed to be
  // worth more than every threshold in the product put together: five of nine
  // auto-fires correct on `ggml-base` against three of three on `turbo`, in the same
  // morning, after the model was changed mid-service. On one 70-second slice,
  // `ggml-small` on auto-detect produced 17 incoherent transcripts and found no
  // verse at all, against 161 coherent ones with the language fixed.
  //
  // The status bar has carried a `Model` cell since the rebrand. That is the wrong
  // place for this: the status bar is read DURING a service, and this is the screen
  // somebody opens BEFORE one, which is the only moment either setting can still be
  // changed without the service lock in the way.
  //
  // Both derive from the same store the rest of the console reads, and both can say
  // they do not know — `no model` and `auto-detect` are real answers and are not the
  // same as a blank. `modelLabel` is `statusbar.js`'s, not a second copy: two
  // shortenings of one path is how they come to disagree.
  $: readyModel = modelLabel($capture.stt?.model);
  $: readyLanguage = $capture.stt?.language ?? null;
  // WHICH BIBLE THE WALL READS FROM (RG-50, 2026-09-21). On 2026-09-20 the
  // preacher named the NKJV; Relay carried the KJV alone and said nothing
  // (RG-135). Now that two are bundled and one is chosen in Settings →
  // Scripture, the pre-service screen says which, beside the model and the
  // language. `null` is "the list could not be read" and is printed as that,
  // never as a blank that reads like a Bible with no name.
  let readyBible = undefined;
  async function loadBible() {
    try {
      const [list, active] = await Promise.all([listTranslations(), getActiveTranslation()]);
      if (!Array.isArray(list) || !list.length) {
        readyBible = null;
        return;
      }
      const t = list.find((x) => x.id === active) ?? list[0];
      readyBible = `${t.abbreviation} · ${t.name}`;
    } catch {
      readyBible = null;
    }
  }

  import {
    capture,
    capturing,
    detectionOn,
    rehearsing,
    live,
    listServices,
    listPlans,
    listOutputChannels,
    startCapture,
    stopCapture,
    setRehearsal,
    serviceLock,
    readErrors,
    listTranslations,
    getActiveTranslation,
  } from '../stores/capture.js';
  import * as walk from '../pathcheck.js';
  import Loading from '../ui/Loading.svelte';
  import ErrorState from '../ui/ErrorState.svelte';

  // EVERY STAGE, NOT THE FIRST (RG-190, 2026-09-21). This ran `diagnostics`
  // alone — six of twenty-three probes — so nothing on :8031, nothing on :8032,
  // a missing table and rule 25's boot-bricking scratch table all rendered
  // "Ready for a service." The ladder the launch sequence climbs is the one the
  // pre-service screen must climb.
  const allChecks = () => {
    const f = freshChecks();
    return [...f.diagnostics, ...f.hardware, ...f.plugins, ...f.migration];
  };
  let health = allChecks();
  let checking = true;

  // ── THE PATH CHECK ────────────────────────────────────────────────────────
  //
  // The twenty-one launch checks all ask about a PART, and every one of them can
  // pass on a machine where nothing works end to end: a microphone the OS has
  // muted, a model that mishears everything, an output window on a display that is
  // asleep. A church finds that out at 10:31. This finds it at 10:05.
  //
  // It runs in REHEARSAL or it does not run at all — the point is to fire a real
  // verse through the real pipeline, and the danger is doing that twenty minutes
  // before a service. If rehearsal cannot be turned on, the walk is abandoned
  // rather than run live.
  let w = walk.newWalk();
  let walking = false;
  let walkTimedOut = false;
  let unlisten = [];
  let walkT0 = 0;
  let walkTimer = null;
  let restoreRehearsal = false;
  let restoreCapture = false;

  $: walkRows = walk.progress(w).rows;
  $: walkVerdict = walk.verdict(w, walkTimedOut);

  async function stopWalk() {
    clearTimeout(walkTimer);
    unlisten.forEach((u) => u());
    unlisten = [];
    walking = false;
    // Put the machine back exactly as it was found, in the reverse order it was
    // changed. A check that leaves the microphone live, or the app in rehearsal,
    // has created the fault it was looking for.
    try {
      if (!restoreCapture && $capturing) await stopCapture();
    } catch {
      /* reported below by the restore of rehearsal, which matters more */
    }
    try {
      if (!restoreRehearsal) await setRehearsal(false);
    } catch (e) {
      w = walk.onError(w, `Relay could not leave rehearsal: ${humanError(e)}`);
    }
  }

  async function startWalk() {
    if (walking) return stopWalk();
    w = walk.newWalk();
    walkTimedOut = false;
    walking = true;
    restoreRehearsal = $rehearsing;
    restoreCapture = $capturing;

    try {
      // SANDBOX FIRST, and abandon if it will not take. Everything after this line
      // puts a real verse through the real pipeline.
      if (!$rehearsing) await setRehearsal(true);
      if (!$rehearsing) throw new Error('rehearsal did not turn on');
    } catch (e) {
      w = walk.onError(
        w,
        `Relay would not switch to rehearsal, so the check was not run — it will not fire a verse at your screens to test itself. ${humanError(e)}`,
      );
      walking = false;
      return;
    }

    const { listen } = await import('@tauri-apps/api/event');
    walkT0 = Date.now();
    const since = () => Date.now() - walkT0;
    try {
      unlisten = [
        await listen('audio://chunk', (e) => (w = walk.onAudio(w, e.payload, since()))),
        await listen('stt://transcript', (e) => (w = walk.onTranscript(w, e.payload, since()))),
        await listen('detection://match', (e) => (w = walk.onDetection(w, e.payload, since()))),
        await listen('output://content', (e) => (w = walk.onOutput(w, e.payload, since()))),
      ];
      if (!$capturing) await startCapture($capture.inputDevice || null);
      w = walk.onStarted(w, since());
    } catch (e) {
      w = walk.onError(w, humanError(e));
      await stopWalk();
      return;
    }

    walkTimer = setTimeout(async () => {
      walkTimedOut = true;
      await stopWalk();
    }, walk.WALK_TIMEOUT_MS);
  }

  $: if (walking && walk.isComplete(w)) stopWalk();
  // THREE FACTS, NOT ONE. Both lists below start empty and are filled by GROUP 2
  // reads that swallow to `[]`, so "No plans yet. Build one in Planner" was also
  // what this pane said for the few frames before the database answered, and what
  // it said for the rest of the session when the read had FAILED. That is the
  // sentence `Loading.svelte` was written for, verbatim, standing on a second
  // surface — and it is the one message that makes an operator think they have
  // lost their work. `asked` flips once the first answer (or failure) is in.
  let services = [];
  let plans = [];
  let channels = [];
  let askedServices = false;
  let askedPlans = false;
  // Named, because the retry button has to re-ASK. A "Try again" wired to anything
  // other than the original read is a button that cannot work (rule 35 again).
  const loadServices = () =>
    listServices().then((s) => ((services = s ?? []), (askedServices = true)));
  const loadPlans = () => listPlans().then((p) => ((plans = p ?? []), (askedPlans = true)));
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
      health = await runChecks(allChecks(), makeProbes(), (partial) => {
        health = partial;
      });
    } catch (e) {
      error = humanError(e);
    }
    checking = false;
  }

  onMount(async () => {
    loadBible();
    // Deliberately not awaited together with the checks: the lists are cheap and
    // should paint immediately, while the probes land one at a time.
    loadServices();
    loadPlans();
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

  // `openMain`, `toggleMic` and `toggleRehearsal` USED TO BE HERE, behind the four
  // quick actions. They are gone with them: `openChannelOutput` is the Outputs
  // workspace's, and starting the microphone or entering rehearsal is the dock's,
  // which is in the shell and so is on this screen already. `listOutputChannels` is
  // still read — the path check and the plan pointers need to know a screen exists —
  // but nothing on this surface opens one.
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
      <!-- NAMED, NOT COUNTED. "speech model all answered" says a model loaded; it
           does not say WHICH, and which is the setting that decided the accuracy of
           two of the three field services on record. An operator who reads this line
           before the room fills up can still change both; ten minutes later the
           service lock is in the way and changing the model mid-service is what
           produced the 44% wrong-verse rate this line exists because of. -->
      <!-- SHOWN WHATEVER THE VERDICT IS, once the checks have answered. These two
           are facts about how this machine will listen, and they are equally true
           when something else is broken -- an operator whose microphone has failed
           still needs to know they are on `base` and auto-detect before they fix it
           and start. Hidden only while the checks are still running, because until
           then the screen is not making any claim yet. -->
      {#if !checking}
        <p class="d-hero-set">
          <span class="d-set">
            <span class="d-setk">Speech model</span>
            <span class="d-setv r-mono">{readyModel ?? 'none — nothing will be transcribed'}</span>
          </span>
          <span class="d-set">
            <span class="d-setk">Recognition language</span>
            <!-- `auto-detect` is a REAL answer and not an absence. It is also the
                 one this row exists to make visible: a fresh install's seeded
                 profile carries no language, and automatic election is what
                 RG-116 measured the cost of. -->
            <span class="d-setv r-mono">{readyLanguage ?? 'auto-detect'}</span>
          </span>
          <span class="d-set">
            <span class="d-setk">Bible</span>
            <span class="d-setv r-mono">{readyBible === undefined ? '…' : readyBible ?? 'could not be read — see Settings → Scripture'}</span>
          </span>
        </p>
      {/if}
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

    <!-- THE PATH CHECK. Below the part-by-part list, because it answers the
         question that list cannot: do the parts work TOGETHER? -->
    <section class="d-card d-walk">
      <header>
        <h3>Test the whole path</h3>
        <span class="r-lbl">one sentence, end to end</span>
      </header>
      <p class="d-walknote">
        Press start and say “<b>{walk.PHRASE}</b>”. Relay switches itself to
        rehearsal first, so this cannot reach your screens.
      </p>
      {#if $serviceLock.engaged}
        <p class="d-walknote">A service is being recorded — end it before running this.</p>
      {/if}
      <button
        class="r-btn sm"
        on:click={startWalk}
        disabled={$safeMode || $serviceLock.engaged || !$capture.available}
      >
        {walking ? 'Listening… press to stop' : 'Start the check'}
      </button>
      {#if walking || walkTimedOut || w.error}
        <ol class="d-walklist">
          {#each walkRows as r (r.id)}
            <li class:ok={r.state === 'ok'} class:miss={walkTimedOut && r.state !== 'ok'}>
              <span class="d-walkdot"></span>
              <span class="d-walklabel">{r.label}</span>
              <!-- Reached at, or nothing. A stage never reached shows no time,
                   because "0ms" would read as instant rather than absent. -->
              <span class="r-mono d-walkat">{r.at === undefined ? '' : `${(r.at / 1000).toFixed(1)}s`}</span>
            </li>
          {/each}
        </ol>
      {/if}
      {#if w.heard}
        <p class="d-walknote">It heard: “{w.heard}”</p>
      {/if}
      {#if walkVerdict.sentence}
        <p class="d-walkverdict" class:bad={walkVerdict.ok === false} role="status">
          {walkVerdict.sentence}
        </p>
      {/if}
    </section>

    <div class="d-side">
      <!-- QUICK ACTIONS WERE HERE, AND ALL FOUR WERE SOMETHING ELSE'S CONTROL.
           *Open the congregation screen* is the Outputs workspace's Open button
           and the first-run wizard's; *Start listening* and *Rehearse* are the
           dock's, which is in the SHELL and therefore on this very screen, three
           inches below, at every moment (rule 15's neighbourhood — the Controls
           card never scrolls); and *Go to the run surface* is the first item in
           the workspace strip at the top of the window.

           A duplicate is not free even when it works. Two controls for one action
           is the shape four separate bugs in this repository have had, and these
           two in particular had already diverged from the dock's: the dock's mic
           and rehearsal buttons read `busy`, `$capture.available` and — since
           DECISIONS §86 — `$safeMode` on the detection switch beside them, while
           these read a local `busy` string and `$safeMode` only. Keeping two
           copies of a control in step is work that buys nothing; the dock owns
           them. -->

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
        {:else if !askedPlans}
          <Loading what="plans" compact />
        {:else if $readErrors.listPlans}
          <ErrorState compact error={$readErrors.listPlans} onRetry={loadPlans} />
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
      <!-- HISTORY IS ITS OWN ROUTE AGAIN. It was in the Library, then a section of
           Settings, and is now neither: a record browser with a destructive erase
           in it is not a setting, and it was three levels deep in a preferences
           page. This is the ONE door into it, which is why it is no longer behind
           `{#if services.length}` — a church with nothing recorded yet could not
           reach the screen at all, and `scripts/qa-inventory.mjs` counts a route
           nothing renders a control for as an orphan. The empty state on the other
           side is honest and is the whole answer to "have we recorded anything?". -->
      <button class="d-link" on:click={() => setSession({ activeTab: 'history' })}
        >All history</button>
    </header>
    {#if services.length}
      <table class="d-table">
        <thead>
          <tr><th>Service</th><th>When</th><th>Length</th><th>Verses</th><th>Overrides</th></tr>
        </thead>
        <tbody>
          {#each services.slice(0, 5) as s}
            <tr>
              <td class="d-t" title={s.build ? `Ran on build ${s.build}` : 'Build not recorded (before 2026-09-21)'}>{s.title || 'Untitled service'}</td>
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
    {:else if !askedServices}
      <Loading what="services" compact />
    {:else if $readErrors.listServices}
      <ErrorState compact error={$readErrors.listServices} onRetry={loadServices} />
    {:else}
      <p class="d-empty">
        No services recorded yet. Relay writes one automatically the first time you start
        listening.
      </p>
    {/if}
  </section>

  {#if error}<div class="d-err" role="alert">{error}</div>{/if}
</div>

<style>
  /* ── ONE TYPE SCALE (docs/REBRAND.md §11 · §12) ────────────────────────────
     This file had SIX literal font sizes: 13.5px, 13px, 12.5px, 11px, 10px and
     9.5px, mixed in among the `--v-fs-*` tokens the rest of the readiness
     surface uses. Three of them were a token's value typed out by hand (9.5 is
     `cap`, 11 is `lbl`, 12.5 is `h3`), and three were steps the scale does not
     have — so a Dashboard row heading was a pixel and a half larger than a
     Settings row heading for no reason anybody chose, and a change to the scale
     would have moved one and not the other.

     §11's whole claim is *one* type scale with three roles. A view that types
     its own numbers is not in that scale; it is beside it, agreeing by
     coincidence until somebody edits the tokens. Every size here is now a
     token, and `settingssections.test.js` holds both files to it. */
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
    /* The token, not the retired literal — one red across every failure edge. */
    border-color: var(--v-red-line);
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
  /* THE TWO SETTINGS, ON THE SCREEN THAT IS READ BEFORE A SERVICE. Quiet, and
     wearing no law colour: amber, cyan and amethyst each promise something about a
     screen and none of them promises anything about this. A key/value pair rather
     than a sentence, because an operator is checking two facts rather than reading
     prose, and they wrap to their own lines on a narrow window instead of
     truncating -- a model name cut in half is the failure this row is about. */
  .d-hero-set {
    display: flex;
    flex-wrap: wrap;
    gap: var(--v-sp-xs) var(--v-sp-md);
    margin: var(--v-sp-xs) 0 0;
  }
  .d-set { display: inline-flex; align-items: baseline; gap: 6px; min-width: 0; }
  .d-setk { font-size: var(--v-fs-lbl); color: var(--v-faint); }
  .d-setv { font-size: var(--v-fs-cap); color: var(--v-txt); }
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

  /* ONE COLUMN. This was `minmax(0,1.35fr) minmax(0,1fr)`, which was right when
     the Dashboard was a full-width tab and is wrong now that it is section one of
     Settings: the reading column §11 caps at 880px would make those two tracks
     about 490px and 360px, and the health card's own rules already stack every
     check at 325px because its value was ellipsising the sentence beside it. A
     card grid inside a capped column is a grid fighting the cap. */
  .d-grid {
    display: flex;
    flex-direction: column;
    gap: 16px;
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
  /* STACKED, always. The boot screen shows these rows across a full window; this
     card is about 325px wide at every viewport, and the value on the right is
     capped at 46% of it — so the check's own sentence was ellipsised mid-word on
     every row. This is the same shape the narrow-viewport rule in app.css already
     switches to, applied by CONTAINER rather than by window. */
  .d-health :global(.b-check) {
    flex-wrap: wrap;
  }
  /* Its OWN line, always — not merely when it fails to fit. A short value like
     "31,102 verses" still reserves enough of a 325px row to ellipsise the check's
     sentence beside it, so the two never share a line in this card. */
  .d-health :global(.b-check .note) {
    flex: 1 0 100%;
    max-width: 100%;
    text-align: left;
    padding-left: 34px;
  }

  /* THE PATH CHECK. Emerald for a stage reached (the design system's
     "confirmed"), rose for one that was not — and a stage that has not been
     reached YET, while the walk is still running, is neither: it is grey, because
     "not yet" and "never" are different claims and the operator is watching. */
  .d-walknote {
    font-size: var(--v-fs-cap);
    color: var(--v-dim);
    margin: 0 0 10px;
    line-height: 1.5;
  }
  .d-walklist {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .d-walklist li {
    display: flex;
    align-items: center;
    gap: 9px;
    font-size: var(--v-fs-b2);
    color: var(--v-faint);
  }
  .d-walkdot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--v-line2);
    flex: 0 0 auto;
  }
  .d-walklist li.ok { color: var(--v-txt); }
  .d-walklist li.ok .d-walkdot { background: var(--v-emerald); }
  .d-walklist li.miss { color: var(--v-rose); }
  .d-walklist li.miss .d-walkdot { background: var(--v-rose); }
  .d-walklabel { flex: 1; min-width: 0; }
  .d-walkat { font-size: var(--v-fs-cap); line-height: var(--v-lh-cap); color: var(--v-faint); }
  .d-walkverdict {
    margin: 12px 0 0;
    font-size: var(--v-fs-b2);
    color: var(--v-dim);
    line-height: 1.5;
  }
  .d-walkverdict.bad { color: var(--v-rose); }

  /* `.d-acts`, `.d-act` AND ITS FIVE STATES USED TO BE HERE — the action-tile
     shape the four quick actions wore. The controls are gone (see the markup),
     and a rule whose only elements have gone is dead weight one refactor away
     from being copied onto something that is not an action tile. `.r-btn` is the
     shape a button in this product has; anything that needs a second line of
     explanation is a row, which is what `.rw-nv` is for. */

  .d-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  /* A LIST ROW, not a button. One saved plan: its title at one end, its id at
     the other, inside an `<li>`. Pressing it navigates to Planner — it does
     not act on the plan — so it reads as the plan rather than as a control
     about the plan. */
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
    font-size: var(--v-fs-h3);
    line-height: var(--v-lh-h3);
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .d-row span {
    font-size: var(--v-fs-lbl);
    line-height: var(--v-lh-lbl);
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
    font-size: var(--v-fs-cap);
    line-height: var(--v-lh-cap);
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
  /* A TEXT LINK, not a button — and a button ELEMENT only because it navigates
     inside the app rather than to a URL (there is nowhere for an anchor's href
     to point). Spelled out rather than written as a tag, because
     `qa-inventory.mjs` scans the whole file for an opening button tag and a
     mention of one inside a comment is reported as a real control with no
     handler and no accessible name. One of the two sits INSIDE a sentence —
     "Build one in Planner — or run the service straight from…" — where a
     26px lozenge with a fill and an edge would break the line it is part of.
     The other names the same affordance beside a heading, and the two must
     keep looking alike. */
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
    /* The token, not the literal. This drew `rgba(239,68,68,.3)` — the same
       retired red B1 took out of `.r-btn.danger`, which is in no token in this
       repository; `--v-red-line` is `rgba(244,81,91,.5)`. Two reds in one
       product is how one of them stops meaning anything. `rgba()` is the one
       thing the token sweep in `workspacegrammar.test.js` says out loud that
       it does not scan, which is why this survived it. */
    border: 1px solid var(--v-red-line);
    color: var(--v-txt);
    font-size: var(--v-fs-h3);
    line-height: 1.55;
  }

  @media (max-width: 980px) {
    .d-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
