<script>
  // LIVE — the one screen the operator runs a whole service from.
  //
  // This is the Console and the Planner's run mode, merged. They should never
  // have been two tabs.
  //
  // The failure it removes: the operator is on the Planner, running the plan.
  // The preacher goes off-script and quotes a verse. Relay detects it — and puts
  // the suggestion on a DIFFERENT TAB, which the operator is not looking at. The
  // single thing this product exists to do was happening somewhere the operator
  // could not see it. They would find out when they next clicked Console, long
  // after the moment had passed.
  //
  // The second failure it removes: `→` meant two different things depending on
  // which tab happened to be mounted — "next slide of the plan" in the Planner,
  // "next verse of the passage" on the Console. Same key, same finger, two
  // outcomes, no indication which one you were about to get. The transport now
  // has an explicit MODE, shown in the bar, and it follows what is actually live:
  //
  //   something from the plan is live  → SLIDE   (→ steps the plan)
  //   a detected/manual verse is live  → VERSE   (→ walks the passage)
  //
  // which means accepting an AI suggestion mid-plan silently switches the
  // transport to the verse, and clearing (Esc) hands it back to the plan. That
  // falls out of `liveCue` in the store — the same reset the panic keys do.
  //
  // BUILDING a plan is not this screen's job. That is the Planner: a different
  // task, done on a Tuesday, not with a congregation waiting.
  import { onMount, onDestroy, afterUpdate } from 'svelte';
  import OutputWall from '../OutputWall.svelte';
  import ModelSetup from '../ModelSetup.svelte';
  import { registerContext } from '../shortcuts.js';
  import { TYPE, payloadOf, slidesOf, slideAccent, cueSub, nextOf, stepFrom } from '../plan.js';
  import { session, setSession } from '../session.js';
  import { get } from 'svelte/store';
  import {
    capture,
    transcript,
    detections,
    live,
    screenBlack,
    liveCue,
    listActiveTemplates,
    confirmDetection,
    dismissDetection,
    manualFire,
    fireContent,
    fireMedia,
    listOutputChannels,
    openChannelOutput,
    listMonitors,
    setChannelDisplay,
    clearScreens,
    blackScreen,
    startCountdown,
    countdownRunning,
    setDetection,
    startCapture,
    stopCapture,
    navVerse,
    listPlans,
    planItems,
    setStageNext,
  } from '../stores/capture.js';

  // ── the plan being RUN (not edited) ──────────────────────────────────────
  let plans = [];
  let openPlan = null;
  let items = [];
  let selId = null;
  let activeTpls = [];

  // The playhead lives in the store (see capture.js). `onAir` — is plan content
  // what the congregation is looking at right now — is a separate fact from the
  // position, and the panic keys clear only the former.
  $: liveCueId = $liveCue.cueId;
  $: liveSlide = $liveCue.slide;
  $: planOnAir = $liveCue.onAir;
  const setLive = (cueId, slide) => liveCue.set({ cueId, slide, onAir: true });

  $: if (openPlan) setSession({ planId: openPlan.id, liveCueId, liveSlide, liveOnAir: planOnAir });

  // THE MODE. Not a toggle the operator has to remember to set — derived from
  // what is on the congregation's screen, which is the only thing they are
  // actually looking at.
  //
  // Verse mode means, and only means: something that did NOT come from the plan is
  // on air. That is the preacher going off-script — the operator accepts the AI's
  // suggested verse and → now walks that passage. Clear the screen and → is back
  // to stepping the plan, from the cue it was already on.
  $: mode = openPlan && items.length && !($live && !planOnAir) ? 'slide' : 'verse';

  async function loadPlan(p) {
    openPlan = p;
    items = await planItems(p.id);
    selId = items[0]?.id ?? null;
    // A playhead from a DIFFERENT plan is meaningless here, and worse than
    // meaningless: its cue id could collide with one in this plan and light up an
    // unrelated cue as CUED.
    liveCue.set({ cueId: null, slide: 0, onAir: false });
  }
  function leave() {
    openPlan = null;
    items = [];
    liveCue.set({ cueId: null, slide: 0, onAir: false });
    setSession({ planId: null, liveCueId: null, liveSlide: 0, liveOnAir: false });
  }

  onMount(async () => {
    activeTpls = await listActiveTemplates().catch(() => []);
    plans = await listPlans().catch(() => []);

    // Resume where the operator actually was. The output windows are separate
    // webviews and survive a console crash, so the verse is still on the wall —
    // restoring the cursor WITHOUT re-firing makes the transport agree with what
    // the congregation is looking at.
    const saved = get(session);
    if (saved.planId) {
      const p = plans.find((x) => x.id === saved.planId);
      if (p) {
        await loadPlan(p);
        if (saved.liveCueId && items.some((i) => i.id === saved.liveCueId)) {
          // Restore the playhead AND whether it was genuinely on air — never
          // assume on air. This runs on every return to the Live tab, not only
          // after a crash, and the operator may simply have cleared the screens.
          liveCue.set({
            cueId: saved.liveCueId,
            slide: saved.liveSlide ?? 0,
            onAir: saved.liveOnAir === true,
          });
          selId = saved.liveCueId;
        }
      }
    }

    // ONE registration for the whole live surface. Previously the Console
    // registered accept/dismiss/search and the Planner registered next/prev, so
    // half the keys were dead on whichever tab you were on.
    unregisterKeys = registerContext({
      accept: acceptTop,
      dismiss: dismissTop,
      next: () => step(1),
      prev: () => step(-1),
      search: () => searchEl?.focus(),
    });
  });
  let unregisterKeys;
  onDestroy(() => {
    unregisterKeys?.();
    clearTimeout(cdArmT);
    clearTimeout(liveMsgT);
  });

  // ── the transport ────────────────────────────────────────────────────────
  function step(dir) {
    if (mode === 'slide') return stepLive(dir);
    navVerse(dir > 0 ? 'next' : 'back');
  }

  async function stepLive(dir) {
    const to = stepFrom(items, liveCueId, liveSlide, dir);
    if (!to) return; // ends of the plan are hard stops — never wrap
    await fireSlide(to.item, to.slide);
  }

  /** Fire slide `i` of `item` to every screen. This is the take. */
  async function fireSlide(item, i) {
    const p = payloadOf(item);
    const s = slidesOf(item)[i];
    if (!s) return;
    setLive(item.id, i);
    selId = item.id;
    const stageNote = p.stage_note || null;
    try {
      if (item.cue_type === 'scripture') {
        await manualFire(p.reference || item.label, stageNote);
      } else if (item.cue_type === 'media') {
        if (!p.media_id) {
          flash('Media asset missing — re-add it from the Library.');
          return;
        }
        await fireMedia(p.media_id);
      } else if (item.cue_type === 'countdown') {
        await startCountdown(Number(p.minutes) || 5, p.label || 'Service begins in', p.done || 'Welcome');
      } else if (item.cue_type === 'song') {
        // Lyrics carry NO title/section on the live screen — that stays in the
        // operator UI. Only the lyric lines go out.
        await fireContent('', s.text, 'song', stageNote);
      } else {
        await fireContent(item.label, s.text, 'announce', stageNote);
      }
      flash(`Live: ${s.label}`);
      const n = nextOf(items, item.id, i);
      setStageNext(n?.label ?? null, n?.text ?? null);
    } catch (e) {
      flash(humanError(e));
    }
  }

  async function clearAll() {
    // clearScreens() resets the transport cursor at the store, so the plan does
    // not fire straight back in on the next →.
    try {
      await clearScreens();
    } catch {
      /* backend absent */
    }
    setStageNext(null, null);
    flash('Screens cleared');
  }

  // ── AI suggestions ───────────────────────────────────────────────────────
  $: dets = $detections;
  function acceptTop() {
    if (!dets[0]) return;
    confirmDetection(dets[0].reference);
    flash(`Now live: ${dets[0].reference}`);
  }
  function dismissTop() {
    if (!dets[0]) return;
    dismissDetection(dets[0].reference);
  }

  // ── transcript ───────────────────────────────────────────────────────────
  let transcriptEl;
  // afterUpdate, never a reactive block: tick() inside `$:` re-enters the Svelte
  // scheduler and hard-freezes the webview. That one cost hours.
  afterUpdate(() => {
    if (transcriptEl) transcriptEl.scrollTop = transcriptEl.scrollHeight;
  });
  $: hasTranscript = $transcript.finals.length > 0 || $transcript.partial.length > 0;

  // ── manual fire + messages ───────────────────────────────────────────────
  let searchEl;
  let manualRef = '';
  let liveMsg = '';
  let liveMsgT;
  let errMsg = '';
  function flash(msg) {
    liveMsg = msg;
    clearTimeout(liveMsgT);
    liveMsgT = setTimeout(() => (liveMsg = ''), 2600);
  }
  /** Turn a raw backend error into a plain sentence for a live operator. */
  function humanError(e) {
    const s = String(e).replace(/^Error:\s*/, '');
    if (/could not parse|parse a reference/i.test(s))
      return `Couldn't read "${manualRef.trim()}" as a scripture reference.`;
    return s;
  }
  async function fireManual() {
    const ref = manualRef.trim();
    if (!ref) return;
    try {
      await manualFire(ref);
      flash(`Now live: ${ref}`);
      manualRef = '';
      errMsg = '';
    } catch (e) {
      errMsg = humanError(e);
    }
  }

  // ── transport controls ───────────────────────────────────────────────────
  let listenBusy = false;
  async function toggleListen() {
    listenBusy = true;
    try {
      if ($capture.capturing) await stopCapture();
      else await startCapture($capture.inputDevice || null);
    } catch {
      /* surfaced via audio://error */
    }
    listenBusy = false;
  }

  // Countdown ARMS on the first click and only fires on the second (auto-disarms
  // after 3s). No native confirm() — Tauri's webview doesn't reliably implement it.
  let cdMin = 5;
  let cdArmed = false;
  let cdArmT;
  async function beginCountdown() {
    if (countdownRunning()) {
      flash('A countdown is already running — clear the screen first');
      return;
    }
    if (!cdArmed) {
      cdArmed = true;
      clearTimeout(cdArmT);
      cdArmT = setTimeout(() => (cdArmed = false), 3000);
      return;
    }
    clearTimeout(cdArmT);
    cdArmed = false;
    const m = Number(cdMin) || 5;
    try {
      await startCountdown(m);
      flash(`Countdown started — ${m} min`);
    } catch (e) {
      flash(String(e));
    }
  }

  // Open the CONGREGATION screen: the real Main-screen channel, honouring the
  // template and display the operator configured. When no display has been chosen
  // yet it picks the first non-primary monitor — a second screen plugged into a
  // church laptop is a projector essentially every time.
  async function openMainOutput() {
    try {
      const channels = await listOutputChannels();
      const main =
        channels.find((c) => c.render_target === 'native_window' && c.name === 'Main screen') ??
        channels.find((c) => c.render_target === 'native_window');
      if (!main) {
        errMsg = 'No output channel yet — add one in the Channels tab.';
        return;
      }
      if (!main.display_target) {
        const projector = (await listMonitors()).find((m) => !m.primary);
        if (projector) {
          await setChannelDisplay(main.id, String(projector.index));
          flash(`Sending to ${projector.name}`);
        }
      }
      await openChannelOutput(main.id);
      flash('Output window opened');
    } catch (e) {
      errMsg = humanError(e);
    }
  }

  // ── mic quality ──────────────────────────────────────────────────────────
  // Plain-language copy for the dsp.rs warnings. The operator is a volunteer, not
  // an audio engineer — "snr_db below 6.0" helps nobody, so every warning names
  // the problem and the physical thing to go and do about it.
  const QUALITY = {
    clipping: {
      title: 'The microphone is too loud — it’s distorting.',
      fix: 'Turn the input gain down on the mixer. Detection accuracy drops badly on clipped audio.',
    },
    too_quiet: {
      title: 'Almost no sound is reaching Relay.',
      fix: 'The mic is probably muted, switched off, or too far away. Check the mixer channel and the mute switch.',
    },
    noisy: {
      title: 'The room is drowning out the speech.',
      fix: 'Detection will struggle. Move the mic closer to the preacher, or cut background noise.',
    },
  };
  // Looked up defensively: an unguarded QUALITY[kind].title on an unknown warning
  // kind would throw, and an exception here takes down the console mid-service
  // over a mic warning.
  $: qualityWarning = (() => {
    const kind = $capture.quality?.warning;
    if (!kind) return null;
    return (
      QUALITY[kind] ?? {
        title: 'There is a problem with the microphone input.',
        fix: 'Detection accuracy may suffer. Check the mixer channel and the mic.',
      }
    );
  })();

  $: selCue = items.find((i) => i.id === selId) || null;
  $: selSlides = slidesOf(selCue);
  $: liveIndex = items.findIndex((i) => i.id === liveCueId);
  $: selNote = selCue ? payloadOf(selCue).stage_note || '' : '';
</script>

<div class="lv">
  <!-- ══ TRANSPORT ══ Always at the top, always the same shape, plan or no plan. -->
  <div class="tbar">
    <span class="t-state r-mono" class:on={$live} class:blk={$screenBlack}>
      <span class="t-dot"></span>
      {#if $screenBlack}BLACKOUT{:else if $live}ON AIR{:else}STANDBY{/if}
    </span>

    {#if openPlan}
      <button class="t-plan r-focus" on:click={leave} title="Close this plan">
        {openPlan.title}
        <span class="r-mono t-pos">
          {liveIndex >= 0 ? `${liveIndex + 1}/${items.length}` : `· ${items.length} cues`}
        </span>
      </button>
    {/if}

    <span class="t-spring"></span>

    {#if liveMsg}<span class="t-msg r-mono">{liveMsg}</span>{/if}

    <!-- The mode indicator. `→` is the most-pressed key in the product and it did
         two different things with no way to tell which. Now it says so. -->
    <span class="t-mode r-mono" class:slide={mode === 'slide'} title={mode === 'slide' ? 'Arrow keys step through the service plan' : 'Arrow keys walk through the passage on screen'}>
      → steps <b>{mode === 'slide' ? 'SLIDE' : 'VERSE'}</b>
    </span>

    <div class="t-nav">
      <button class="r-iconbtn" title="Previous (←)" aria-label="Previous" on:click={() => step(-1)}>‹</button>
      <button class="r-btn amber sm" on:click={() => step(1)}>Next ›</button>
      <button class="r-iconbtn" title="Clear all screens (Esc)" aria-label="Clear all screens" on:click={clearAll}>◼</button>
    </div>
  </div>

  <!-- ══ THE WALL ══ One copy, shared with nothing else to drift against. -->
  <OutputWall templates={activeTpls} verseNav={mode === 'verse'} />

  <div class="lv-grid">
    <!-- ══ PLAN ══ -->
    <aside class="col plan">
      <div class="col-head">
        <h3>Service plan</h3>
        {#if openPlan}<span class="r-mono dim">{items.length}</span>{/if}
      </div>

      {#if openPlan}
        <div class="cues r-scroll">
          {#each items as c, i (c.id)}
            {@const ty = TYPE[c.cue_type] || TYPE.scripture}
            <button
              class="cue r-focus"
              class:sel={c.id === selId}
              class:islive={planOnAir && c.id === liveCueId}
              class:cued={!planOnAir && c.id === liveCueId}
              on:click={() => (selId = c.id)}>
              <span class="cue-stripe" style="background:{ty.color};"></span>
              <span class="cue-num r-mono">{String(i + 1).padStart(2, '0')}</span>
              <span class="cue-body">
                <span class="cue-title">{c.label}</span>
                <span class="cue-meta r-mono">{cueSub(c)}</span>
              </span>
              {#if c.id === liveCueId}
                <span class="cue-live r-mono" class:cued={!planOnAir}>{planOnAir ? 'LIVE' : 'CUED'}</span>
              {/if}
            </button>
          {/each}
          {#if !items.length}
            <div class="col-empty">This plan has no cues. Add them in <b>Planner</b>.</div>
          {/if}
        </div>
      {:else}
        <!-- No plan loaded. Not an error — plenty of services are run entirely from
             the AI and the manual box. Offer the plans, don't demand one. -->
        <div class="cues r-scroll">
          {#each plans as p (p.id)}
            <button class="cue pick r-focus" on:click={() => loadPlan(p)}>
              <span class="cue-body">
                <span class="cue-title">{p.title}</span>
                <span class="cue-meta r-mono">{p.plan_date} · {p.cue_count} cues</span>
              </span>
              <span class="cue-go r-mono">RUN</span>
            </button>
          {/each}
          {#if !plans.length}
            <div class="col-empty">
              No service plans yet. Build one in <b>Planner</b> — or just run the
              service from the AI and the manual box below.
            </div>
          {/if}
        </div>
      {/if}
    </aside>

    <!-- ══ SLIDES ══ -->
    <section class="col flow">
      <div class="col-head">
        <h3>{selCue ? selCue.label : 'Slides'}</h3>
        {#if selSlides.length}<span class="r-mono dim">{selSlides.length}</span>{/if}
      </div>

      {#if selNote}
        <!-- The preacher's stage note. Confidence-monitor only — never on the main
             output. -->
        <div class="note">
          <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
          {selNote}
        </div>
      {/if}

      <div class="slides r-scroll">
        {#if selCue}
          {#each selSlides as s, i}
            <button
              class="slide r-focus"
              class:islive={planOnAir && selCue.id === liveCueId && i === liveSlide}
              style="--acc:{slideAccent(s.tag)}"
              on:click={() => fireSlide(selCue, i)}>
              <span class="slide-stripe"></span>
              <span class="slide-tag r-mono">{s.tag}</span>
              <span class="slide-text">{s.text || s.label}</span>
              {#if planOnAir && selCue.id === liveCueId && i === liveSlide}
                <span class="slide-live r-mono">LIVE</span>
              {/if}
              {#if !planOnAir && selCue.id === liveCueId && i === liveSlide}
                <!-- Not on air, but this is where → will resume from. The operator
                     must be able to see that without having to fire it to find out. -->
                <span class="slide-cued r-mono">CUED</span>
              {/if}
            </button>
          {/each}
        {:else}
          <div class="col-empty">Pick a cue to see its slides. Click any slide to put it on screen.</div>
        {/if}
      </div>
    </section>

    <!-- ══ INTELLIGENCE ══ The reason the merge exists: this is now VISIBLE while
         the operator is running the plan. -->
    <section class="col feed">
      <div class="col-head">
        <h3>Intelligence</h3>
        <span class="r-mono dim">
          {$capture.capturing ? ($capture.detectedLang ?? 'listening') : 'standby'}
        </span>
      </div>

      <div class="tx" bind:this={transcriptEl}>
        {#if hasTranscript}
          {$transcript.finals.join(' ')}
          {#if $transcript.partial}<mark>{$transcript.partial}</mark><i class="caret"></i>{/if}
        {:else if $capture.capturing}
          <span class="dim">Waiting for speech…</span>
        {:else if !$capture.stt.loaded}
          <span class="dim">No speech model loaded. Manual override still works.</span>
        {:else}
          <span class="dim">Start listening to transcribe live.</span>
        {/if}
      </div>

      <div class="sugs r-scroll">
        {#if dets.length}
          {@const d = dets[0]}
          <div class="sug">
            <div class="sug-top">
              <span class="r-lbl">AI suggestion</span>
              <span class="r-mono amber">{Math.round(d.confidence * 100)}%</span>
            </div>
            <div class="sug-ref">{d.reference}</div>
            {#if d.text}<div class="sug-verse">“{d.text}”</div>{/if}
            <div class="sug-acts">
              <button class="r-btn amber sm" on:click={acceptTop}>Push to stage <kbd>A</kbd></button>
              <button class="r-btn ghost sm" on:click={dismissTop}>Dismiss <kbd>D</kbd></button>
            </div>
          </div>

          {#each dets.slice(1) as x (x.reference + x.at)}
            <div class="xref">
              <div class="sug-top">
                <span class="r-lbl">Cross reference</span>
                <span class="r-mono dim">{Math.round(x.confidence * 100)}%</span>
              </div>
              <div class="xref-ref">{x.reference}</div>
              {#if x.text}<div class="sug-verse">“{x.text}”</div>{/if}
              <div class="sug-acts">
                <button class="r-btn ghost sm" on:click={() => confirmDetection(x.reference)}>Push</button>
                <button class="r-btn ghost sm" on:click={() => dismissDetection(x.reference)}>Dismiss</button>
              </div>
            </div>
          {/each}
        {:else}
          <div class="col-empty">
            {#if !$capture.detectionOn}
              Detection is off — manual override still fires.
            {:else}
              No suggestions yet.
            {/if}
          </div>
        {/if}
      </div>
    </section>
  </div>

  <!-- ══ COMMAND BAR ══ -->
  <section class="cmd">
    <div class="cmd-row">
      <div class="cmd-search">
        <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
        <input
          bind:this={searchEl}
          bind:value={manualRef}
          on:keydown={(e) => e.key === 'Enter' && fireManual()}
          placeholder="Type any reference and press Enter — ps 23, John 3:16-18"
          aria-label="Manual scripture reference"
          disabled={!$capture.available} />
      </div>
      <button class="r-btn amber" on:click={fireManual} disabled={!$capture.available}>Push to stage</button>
    </div>
    {#if errMsg}<div class="cmd-err">{errMsg}</div>{/if}

    <div class="cmd-ctls">
      <button class="ctl" class:rec={$capture.capturing} on:click={toggleListen}
        disabled={!$capture.available || !$capture.stt.loaded || listenBusy}>
        <span class="dot" style="background:{$capture.capturing ? 'var(--v-rose)' : 'var(--v-amber)'}"></span>
        {$capture.capturing ? 'Listening — Stop' : listenBusy ? 'Starting…' : 'Start listening'}
      </button>
      <button class="ctl" on:click={() => setDetection(!$capture.detectionOn)} disabled={!$capture.available}>
        <span class="dot" style="background:{$capture.detectionOn ? 'var(--v-emerald)' : 'var(--v-faint)'}"></span>
        Detection {$capture.detectionOn ? 'active' : 'off'}
      </button>
      <button class="ctl" on:click={clearAll} disabled={!$capture.available}>
        <span class="dot" style="background:var(--v-faint)"></span>Clear all<kbd>Esc</kbd>
      </button>
      <button class="ctl" class:rec={$screenBlack} on:click={() => { blackScreen(); flash('Blackout'); }} disabled={!$capture.available}>
        <span class="dot" style="background:#000;border:1px solid var(--v-line2)"></span>Black<kbd>B</kbd>
      </button>
      <div class="ctl cd">
        <span class="dot" style="background:var(--v-cyan)"></span>Countdown
        <input class="cd-min" type="number" min="1" max="120" bind:value={cdMin} aria-label="Countdown minutes" disabled={!$capture.available} />
        <span class="r-mono dim">min</span>
        <button class="cd-go" class:armed={cdArmed} on:click={beginCountdown} disabled={!$capture.available}>
          {cdArmed ? 'Confirm?' : 'Start'}
        </button>
      </div>
      <button class="ctl" on:click={openMainOutput} disabled={!$capture.available}>
        <span class="dot" style="background:var(--v-amber)"></span>Open output
      </button>
    </div>
  </section>

  {#if $capture.audioError}<div class="banner">Audio: {$capture.audioError}</div>{/if}
  {#if $capture.outputError}<div class="banner">Output: {$capture.outputError}</div>{/if}

  <!-- Only while listening, and only when something is genuinely wrong. A warning
       that is always on screen is wallpaper. -->
  {#if $capture.capturing && qualityWarning}
    <div class="banner warn"><b>{qualityWarning.title}</b> {qualityWarning.fix}</div>
  {/if}

  <!-- No STT model = the AI cannot listen. Relay degrades to a fully working
       MANUAL tool, never a dead one — and it can fix itself in one click. -->
  {#if $capture.available && !$capture.stt.loaded}
    <ModelSetup compact />
  {/if}
</div>

<style>
  .lv {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  /* ── transport ── */
  .tbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    background: var(--v-surf);
    border: 1px solid var(--v-line2);
    border-radius: 11px;
  }
  .t-spring { flex: 1; }
  .t-state {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    letter-spacing: 0.08em;
    padding: 5px 9px;
    border-radius: 99px;
    background: var(--v-surf3);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
    flex: none;
  }
  .t-dot {
    width: 6px;
    height: 6px;
    border-radius: 99px;
    background: var(--v-faint);
  }
  .t-state.on {
    color: var(--v-amber2);
    border-color: var(--v-amber);
    background: var(--v-amber-soft);
  }
  .t-state.on .t-dot {
    background: var(--v-amber);
    box-shadow: 0 0 7px var(--v-amber-glow);
  }
  .t-state.blk {
    color: var(--v-rose);
    border-color: rgba(244, 113, 139, 0.45);
    background: rgba(244, 113, 139, 0.12);
  }
  .t-state.blk .t-dot { background: var(--v-rose); }
  .t-plan {
    display: inline-flex;
    align-items: baseline;
    gap: 7px;
    background: none;
    border: 0;
    padding: 4px 6px;
    border-radius: 7px;
    color: var(--v-txt);
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
  }
  .t-plan:hover { background: var(--v-surf2); }
  .t-pos { font-size: 10px; color: var(--v-dim); font-weight: 400; }
  .t-msg { font-size: 11px; color: var(--v-emerald); }
  .t-mode {
    font-size: 10px;
    letter-spacing: 0.05em;
    color: var(--v-dim);
    padding: 5px 8px;
    border-radius: 6px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    flex: none;
  }
  .t-mode b { color: var(--v-cyan); font-weight: 600; }
  .t-mode.slide b { color: var(--v-amber2); }
  .t-nav { display: flex; gap: 6px; flex: none; }

  /* ── three columns ── */
  .lv-grid {
    display: grid;
    grid-template-columns: minmax(210px, 1fr) minmax(240px, 1.3fr) minmax(260px, 1.4fr);
    gap: 10px;
    align-items: stretch;
  }
  @media (max-width: 1100px) {
    .lv-grid { grid-template-columns: 1fr; }
  }
  .col {
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: 11px;
    padding: 11px 12px 12px;
  }
  .col-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 9px;
  }
  .col-head h3 {
    margin: 0;
    font-family: var(--f-display);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--v-txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .col-head .dim { font-size: 10px; color: var(--v-dim); }
  .col-empty {
    padding: 16px 12px;
    font-size: 12px;
    line-height: 1.6;
    color: var(--v-dim);
    text-align: center;
  }

  /* ── plan rail ── */
  .cues {
    display: flex;
    flex-direction: column;
    gap: 5px;
    overflow-y: auto;
    max-height: 300px;
  }
  .cue {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 9px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    border-radius: 8px;
    color: var(--v-txt);
    font: inherit;
    text-align: left;
    cursor: pointer;
    flex: none;
  }
  .cue:hover { border-color: var(--v-line2); }
  .cue.sel { border-color: var(--v-cyan); }
  .cue.islive {
    border-color: var(--v-amber);
    background: var(--v-amber-soft);
  }
  /* Cued, not live. A dashed ring, never amber — amber means ON AIR and nothing
     else, anywhere in this product. */
  .cue.cued { border-style: dashed; border-color: var(--v-faint); }
  .cue-stripe {
    width: 3px;
    align-self: stretch;
    border-radius: 99px;
    flex: none;
  }
  .cue-num { font-size: 10px; color: var(--v-dim); flex: none; }
  .cue-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 2px; }
  .cue-title {
    font-size: 12.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cue-meta { font-size: 9px; letter-spacing: 0.04em; color: var(--v-dim); }
  .cue-live, .cue-go {
    font-size: 9px;
    letter-spacing: 0.06em;
    color: var(--v-amber2);
    flex: none;
  }
  .cue-live.cued { color: var(--v-dim); }
  .cue-go { color: var(--v-dim); }
  .cue.pick:hover .cue-go { color: var(--v-amber2); }

  /* ── slides ── */
  .note {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 7px 9px;
    margin-bottom: 8px;
    border-radius: 7px;
    background: rgba(192, 139, 255, 0.1);
    border: 1px solid rgba(192, 139, 255, 0.3);
    color: var(--v-amethyst);
    font-size: 11.5px;
    line-height: 1.5;
  }
  .note svg { flex: none; margin-top: 2px; }
  .slides {
    display: flex;
    flex-direction: column;
    gap: 5px;
    overflow-y: auto;
    max-height: 300px;
  }
  .slide {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 9px;
    width: 100%;
    padding: 9px 10px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    border-radius: 8px;
    color: var(--v-txt);
    font: inherit;
    text-align: left;
    cursor: pointer;
    flex: none;
  }
  /* Same stripe idiom as the cue rail, so a chorus reads the same in both columns. */
  .slide-stripe {
    width: 3px;
    align-self: stretch;
    border-radius: 99px;
    background: var(--acc);
    flex: none;
  }
  .slide:hover { border-color: var(--v-line2); }
  .slide.islive {
    border-color: var(--v-amber);
    background: var(--v-amber-soft);
  }
  .slide-tag {
    font-size: 9px;
    letter-spacing: 0.05em;
    color: var(--acc);
    flex: none;
    padding-top: 2px;
    min-width: 26px;
  }
  .slide-text {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    line-height: 1.55;
    color: var(--v-dim);
    white-space: pre-wrap;
    /* Four lines is enough to recognise a slide; more turns the grid into a wall
       of lyrics you have to scroll past to find the one you want. */
    display: -webkit-box;
    -webkit-line-clamp: 4;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .slide.islive .slide-text { color: var(--v-txt); }
  .slide-live, .slide-cued {
    font-size: 8.5px;
    letter-spacing: 0.06em;
    color: var(--v-amber2);
    flex: none;
  }
  .slide-cued { color: var(--v-dim); }

  /* ── intelligence ── */
  .tx {
    height: 96px;
    overflow-y: auto;
    padding: 9px 10px;
    margin-bottom: 8px;
    border-radius: 8px;
    background: var(--v-bg);
    border: 1px solid var(--v-line);
    font-size: 12.5px;
    line-height: 1.65;
    color: var(--v-txt);
  }
  .tx .dim { color: var(--v-dim); }
  .tx mark {
    background: none;
    color: var(--v-amber2);
  }
  .caret {
    display: inline-block;
    width: 6px;
    height: 12px;
    margin-left: 2px;
    background: var(--v-amber);
    vertical-align: -1px;
    animation: blink 1.05s steps(2, start) infinite;
  }
  @keyframes blink {
    to { visibility: hidden; }
  }
  @media (prefers-reduced-motion: reduce) {
    .caret { animation: none; }
  }
  .sugs {
    display: flex;
    flex-direction: column;
    gap: 7px;
    overflow-y: auto;
    max-height: 260px;
  }
  .sug, .xref {
    padding: 10px 11px;
    border-radius: 9px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    flex: none;
  }
  .sug {
    border-color: var(--v-amber);
    background: var(--v-amber-soft);
  }
  .sug-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 5px;
  }
  .sug-top .amber { font-size: 10px; color: var(--v-amber2); }
  .sug-top .dim { font-size: 10px; color: var(--v-dim); }
  .sug-ref {
    font-family: var(--f-display);
    font-size: 15px;
    font-weight: 600;
    color: var(--v-txt);
  }
  .xref-ref { font-size: 13px; font-weight: 600; color: var(--v-txt); }
  .sug-verse {
    margin: 5px 0 9px;
    font-family: var(--f-serif);
    font-size: 12.5px;
    line-height: 1.6;
    color: var(--v-dim);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .sug-acts { display: flex; gap: 6px; }

  /* ── command bar ── */
  .cmd {
    background: var(--v-surf);
    border: 1px solid var(--v-line2);
    border-radius: 11px;
    padding: 11px 12px;
  }
  .cmd-row { display: flex; gap: 8px; }
  .cmd-search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 11px;
    border-radius: 8px;
    background: var(--v-bg);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
  }
  .cmd-search input {
    flex: 1;
    background: none;
    border: 0;
    outline: none;
    padding: 10px 0;
    color: var(--v-txt);
    font: inherit;
    font-size: 13px;
  }
  .cmd-search:focus-within { border-color: var(--v-amber); }
  .cmd-err {
    margin-top: 8px;
    padding: 7px 10px;
    border-radius: 7px;
    background: rgba(244, 113, 139, 0.12);
    border: 1px solid rgba(244, 113, 139, 0.32);
    color: var(--v-txt);
    font-size: 12px;
  }
  .cmd-ctls {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 9px;
  }
  .ctl {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 7px 10px;
    border-radius: 8px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line);
    color: var(--v-txt);
    font: inherit;
    font-size: 11.5px;
    cursor: pointer;
  }
  .ctl:hover:not(:disabled) { border-color: var(--v-line2); }
  .ctl:disabled { opacity: 0.45; cursor: not-allowed; }
  .ctl.rec { border-color: var(--v-rose); }
  .ctl .dot {
    width: 7px;
    height: 7px;
    border-radius: 99px;
    flex: none;
  }
  .ctl kbd {
    font-family: var(--f-mono);
    font-size: 9px;
    padding: 2px 4px;
    border-radius: 4px;
    background: var(--v-surf3);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
  }
  .cd { cursor: default; }
  .cd-min {
    width: 44px;
    padding: 4px 5px;
    border-radius: 5px;
    background: var(--v-bg);
    border: 1px solid var(--v-line2);
    color: var(--v-txt);
    font: inherit;
    font-size: 11px;
  }
  .cd .dim { font-size: 10px; color: var(--v-dim); }
  .cd-go {
    padding: 4px 9px;
    border-radius: 6px;
    background: var(--v-surf3);
    border: 1px solid var(--v-line2);
    color: var(--v-txt);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }
  .cd-go.armed {
    background: var(--v-amber);
    border-color: var(--v-amber);
    color: var(--v-amber-ink);
    font-weight: 600;
  }
  .sug-acts kbd {
    font-family: var(--f-mono);
    font-size: 9px;
    margin-left: 4px;
    opacity: 0.7;
  }

  /* ── banners ── */
  .banner {
    padding: 9px 12px;
    border-radius: 9px;
    background: rgba(244, 113, 139, 0.12);
    border: 1px solid rgba(244, 113, 139, 0.32);
    color: var(--v-txt);
    font-size: 12px;
    line-height: 1.55;
  }
  .banner.warn {
    background: var(--v-amber-soft);
    border-color: var(--v-amber);
  }
  .banner b { display: block; margin-bottom: 2px; }
</style>
