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
  import { heard, methodLabel } from '../detect.js';
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
    rehearsing,
    loadRehearsal,
    setRehearsal,
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

  // ── rehearsal ────────────────────────────────────────────────────────────
  // Rust owns the flag (channels.rs gates the one function content leaves through);
  // this only drives the UI. It THROWS on refusal — Rust will not let you rehearse
  // while a service is recording — and the operator is told why.
  let rehBusy = false;
  async function toggleRehearsal() {
    rehBusy = true;
    try {
      await setRehearsal(!$rehearsing);
      flash($rehearsing ? 'Rehearsal — nothing reaches the screens' : 'Live. Screens cleared.');
      errMsg = '';
    } catch (e) {
      errMsg = humanError(e);
    }
    rehBusy = false;
  }

  onMount(async () => {
    await loadRehearsal();
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
    //
    // Flash ONLY if it actually worked. This used to say "Screens cleared"
    // unconditionally, over a `catch {}` that could never even fire (clearScreens
    // swallowed its own errors) — so a failed clear told the operator the wall was
    // clean while the verse was still on it. On failure the panic banner in the app
    // shell says so; adding a second, softer message here would only dilute it.
    const ok = await clearScreens();
    setStageNext(null, null);
    if (ok) flash('Screens cleared');
  }

  async function blackAll() {
    const ok = await blackScreen();
    if (ok) flash('Blackout');
  }

  // ── AI suggestions ───────────────────────────────────────────────────────
  $: dets = $detections;

  // heard() / methodLabel() live in lib/detect.js — pure, and unit-tested there,
  // because they are the frontend half of the auto-fire safety rule (see that file).
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

<div class="stx">
  <!-- ══ REHEARSAL ══
       Unmissable, or it is worse than useless. Both ways of being wrong about this
       are bad, in opposite directions: rehearsing when you think you are live means
       the projector stays dark through the whole sermon; live when you think you are
       rehearsing means your practice run is on the wall in front of everyone. So it
       is stated in a full-width band, at the top, permanently, in a colour that is
       not amber — because amber means ON AIR and is never allowed to lie. -->
  {#if $rehearsing}
    <div class="reh" role="status">
      <span class="reh-dot"></span>
      <b>REHEARSAL</b>
      <span>Nothing is reaching the congregation's screens. Practise freely — the AI, the plan and the arrow keys all behave exactly as they will on Sunday.</span>
      <button class="reh-end" on:click={toggleRehearsal} disabled={rehBusy}>End rehearsal</button>
    </div>
  {/if}

  <!-- ══ TRANSPORT ══ -->
  <div class="tbar">
    <span class="sys" class:live={$live && !$rehearsing} class:reh={$rehearsing} class:blk={$screenBlack}>
      <span class="sys-dot"></span>
      {#if $rehearsing}REHEARSAL{:else if $screenBlack}BLACKOUT{:else if $live}ON AIR{:else}STANDBY{/if}
    </span>

    {#if openPlan}
      <button class="t-plan" on:click={leave} title="Close this plan">
        {openPlan.title}
        <span class="mono t-pos">
          {liveIndex >= 0 ? `${liveIndex + 1}/${items.length}` : `· ${items.length} cues`}
        </span>
      </button>
    {/if}

    <span class="spring"></span>
    {#if liveMsg}<span class="livemsg"><span class="lm-dot"></span>{liveMsg}</span>{/if}

    <!-- `→` is the most-pressed key in the product and it used to do two different
         things with no way to tell which. Now it says which. -->
    <span
      class="t-mode mono"
      class:slide={mode === 'slide'}
      title={mode === 'slide'
        ? 'Arrow keys step through the service plan'
        : 'Arrow keys walk through the passage on screen'}>
      → steps <b>{mode === 'slide' ? 'SLIDE' : 'VERSE'}</b>
    </span>

    <div class="t-nav">
      <button class="nav-sq" title="Previous (←)" aria-label="Previous" on:click={() => step(-1)}>‹</button>
      <button class="btn-gold" on:click={() => step(1)}>Next ›</button>
      <button class="nav-sq" title="Clear all screens (Esc)" aria-label="Clear all screens" on:click={clearAll}>◼</button>
    </div>
  </div>

  <div class="stx-top">
    <!-- ══ INTELLIGENCE FEED ══ Visible while the plan is running. That is the
         entire reason the Console and the Planner became one screen. -->
    <section class="tile feed">
      <div class="tile-head">
        <h3>Intelligence Feed</h3>
        <svg class="ic dim" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M3 3v18h18"/><path d="M18 17V9M13 17V5M8 17v-3"/></svg>
      </div>

      <div class="tx-box">
        <div class="seg-top">
          <span class="lbl-gold">Live transcript</span>
          <span class="mono dim">{$capture.capturing ? ($capture.detectedLang ?? 'listening') : 'standby'}</span>
        </div>
        <div class="tx-stream" bind:this={transcriptEl}>
          {#if hasTranscript}
            {$transcript.finals.join(' ')}
            {#if $transcript.partial}<mark>{$transcript.partial}</mark><i class="caret"></i>{/if}
          {:else if $capture.capturing}
            <span class="dim">Waiting for speech…</span>
          {:else if !$capture.stt.loaded}
            <span class="dim">No speech model loaded — see Settings. Manual override still works.</span>
          {:else}
            <span class="dim">Start listening to transcribe live.</span>
          {/if}
        </div>
      </div>

      <div class="feed-body">
        {#if dets.length}
          {@const d = dets[0]}
          <!-- HEARD vs GUESSED. These are not two flavours of the same thing, and
               they must not look like it. A direct hit is a reference the parser
               actually heard, and its confidence is a real parse confidence. A
               paraphrase is a TF-IDF cosine — a distance in an arbitrary vector
               space, NOT a probability (docs/DECISIONS.md; router.rs forbids it from
               ever auto-firing at ANY score, for exactly this reason).

               Both used to render as "AI suggestion — 92% match". The operator was
               shown a number that means one thing for one kind of match and nothing
               at all for the other, with no way to tell which they were looking at —
               while being asked to be the human in the loop. -->
          <div class="ai-card" class:guess={!heard(d)}>
            <div class="ai-top">
              <span class="lbl-method" class:guess={!heard(d)}>{methodLabel(d)}</span>
              {#if heard(d)}
                <span class="mono gold">{Math.round(d.confidence * 100)}% match</span>
              {:else}
                <!-- Deliberately NO percentage. Printing "61%" next to a cosine
                     invites the operator to read it as "61% likely to be right",
                     which is precisely what it is not. -->
                <span class="mono guess-note">not a spoken reference</span>
              {/if}
            </div>
            {#if heard(d)}
              <!-- A bar, not just a number: "0.92" means nothing to a volunteer. -->
              <div
                class="conf"
                role="meter"
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow={Math.round(d.confidence * 100)}
                aria-label="Detection confidence"
              >
                <i style="width:{Math.round(d.confidence * 100)}%"></i>
              </div>
            {/if}
            <div class="ai-ref">{d.reference}</div>
            {#if d.matched_text}
              <!-- THE EVIDENCE. Captured in Rust for months and dropped at the IPC
                   boundary. Showing the words that triggered a match is the clearest
                   possible explanation of an AI decision: an operator can tell at a
                   glance whether Relay heard "john three sixteen" or misheard
                   "gone free sixty" — and can judge a paraphrase by the words it
                   actually keyed on, which is something a human can agree with. -->
              <div class="why">
                <span class="why-lbl">{heard(d) ? 'Heard' : 'Matched on'}</span>
                <span class="why-txt">{d.matched_text}</span>
              </div>
            {/if}
            {#if d.text}<div class="ai-verse">“{d.text}”</div>{/if}
            <div class="ai-acts">
              <button class="btn-gold" on:click={acceptTop}>Push to stage</button>
              <button class="btn-x" on:click={dismissTop}>Dismiss</button>
              <span class="hint"><kbd>A</kbd> accept · <kbd>D</kbd> dismiss</span>
            </div>
          </div>

          {#each dets.slice(1) as x (x.reference + x.at)}
            <div class="xref" class:guess={!heard(x)}>
              <div class="xref-top">
                <span class="lbl-method sm" class:guess={!heard(x)}>{methodLabel(x)}</span>
                {#if heard(x)}
                  <span class="mono dim">{Math.round(x.confidence * 100)}%</span>
                {/if}
              </div>
              <div class="xref-ref">{x.reference}</div>
              {#if x.matched_text}
                <div class="why sm">
                  <span class="why-lbl">{heard(x) ? 'Heard' : 'Matched on'}</span>
                  <span class="why-txt">{x.matched_text}</span>
                </div>
              {/if}
              {#if x.text}<div class="xref-verse">“{x.text}”</div>{/if}
              <div class="xref-acts">
                <button class="btn-mini" on:click={() => confirmDetection(x.reference)}>Push</button>
                <button class="btn-mini ghost" on:click={() => dismissDetection(x.reference)}>Dismiss</button>
              </div>
            </div>
          {/each}
        {:else}
          <div class="empty">
            {#if !$capture.detectionOn}Detection is off — manual override still fires.{:else}No suggestions yet.{/if}
          </div>
        {/if}
      </div>
    </section>

    <!-- ══ RIGHT: the wall, then the plan being run ══ -->
    <div class="rightcol">
      <section class="tile channels">
        <div class="tile-head">
          <h2>Output</h2>
          <span class="mono dim">
            {activeTpls.length}/4 styles{$rehearsing ? ' · rehearsal' : $live ? ' · live' : ''}
          </span>
        </div>
        <div class="chan-wrap">
          <OutputWall templates={activeTpls} verseNav={mode === 'verse'} />
        </div>
      </section>

      <div class="runrow">
        <!-- PLAN -->
        <section class="tile">
          <div class="tile-head">
            <h3>Service Plan</h3>
            <span class="mono dim">{openPlan ? items.length : plans.length}</span>
          </div>
          <div class="listbody">
            {#if openPlan}
              {#each items as c, i (c.id)}
                {@const ty = TYPE[c.cue_type] || TYPE.scripture}
                <button
                  class="cue"
                  class:sel={c.id === selId}
                  class:islive={planOnAir && c.id === liveCueId}
                  class:cued={!planOnAir && c.id === liveCueId}
                  on:click={() => (selId = c.id)}>
                  <span class="cue-stripe" style="background:{ty.color}"></span>
                  <span class="cue-num mono">{String(i + 1).padStart(2, '0')}</span>
                  <span class="cue-body">
                    <span class="cue-title">{c.label}</span>
                    <span class="cue-meta mono">{cueSub(c)}</span>
                  </span>
                  {#if c.id === liveCueId}
                    <span class="cue-tag mono" class:cued={!planOnAir}>{planOnAir ? 'LIVE' : 'CUED'}</span>
                  {/if}
                </button>
              {/each}
              {#if !items.length}
                <div class="empty">This plan has no cues. Add them in <b>Planner</b>.</div>
              {/if}
            {:else}
              <!-- No plan loaded. Not an error — plenty of services run entirely on
                   the AI and the manual box. Offer the plans, don't demand one. -->
              {#each plans as p (p.id)}
                <button class="cue pick" on:click={() => loadPlan(p)}>
                  <span class="cue-body">
                    <span class="cue-title">{p.title}</span>
                    <span class="cue-meta mono">{p.plan_date} · {p.cue_count} cues</span>
                  </span>
                  <span class="cue-tag mono go">RUN</span>
                </button>
              {/each}
              {#if !plans.length}
                <div class="empty">
                  No service plans yet. Build one in <b>Planner</b> — or run the service
                  from the AI and the manual box below.
                </div>
              {/if}
            {/if}
          </div>
        </section>

        <!-- SLIDES -->
        <section class="tile">
          <div class="tile-head">
            <h3>{selCue ? selCue.label : 'Slides'}</h3>
            {#if selSlides.length}<span class="mono dim">{selSlides.length}</span>{/if}
          </div>
          <div class="listbody">
            {#if selNote}
              <!-- The preacher's stage note. Confidence monitor only — never on the
                   main output. -->
              <div class="note">
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
                {selNote}
              </div>
            {/if}
            {#if selCue}
              {#each selSlides as s, i}
                <button
                  class="slide"
                  class:islive={planOnAir && selCue.id === liveCueId && i === liveSlide}
                  class:cued={!planOnAir && selCue.id === liveCueId && i === liveSlide}
                  style="--acc:{slideAccent(s.tag)}"
                  on:click={() => fireSlide(selCue, i)}>
                  <span class="slide-stripe"></span>
                  <span class="slide-tag mono">{s.tag}</span>
                  <span class="slide-text">{s.text || s.label}</span>
                  {#if selCue.id === liveCueId && i === liveSlide}
                    <span class="cue-tag mono" class:cued={!planOnAir}>{planOnAir ? 'LIVE' : 'CUED'}</span>
                  {/if}
                </button>
              {/each}
            {:else}
              <div class="empty">Pick a cue to see its slides. Click a slide to put it on screen.</div>
            {/if}
          </div>
        </section>
      </div>
    </div>
  </div>

  <!-- ══ COMMAND BAR ══ -->
  <section class="tile entry">
    <div class="entry-row">
      <div class="search">
        <svg class="ic dim" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3"/></svg>
        <input
          bind:this={searchEl}
          bind:value={manualRef}
          on:keydown={(e) => e.key === 'Enter' && fireManual()}
          placeholder="Search scripture or commands — ps 23, John 3:16-18"
          aria-label="Manual scripture reference"
          disabled={!$capture.available} />
      </div>
      <button class="btn-gold lg" on:click={fireManual} disabled={!$capture.available}>Push to stage</button>
    </div>
    {#if errMsg}<div class="err">{errMsg}</div>{/if}

    <div class="entry-controls">
      <button class="ctl" class:rec={$capture.capturing} on:click={toggleListen}
        disabled={!$capture.available || !$capture.stt.loaded || listenBusy}>
        <span class="dot" style="background:{$capture.capturing ? 'var(--v-rose)' : 'var(--v-amber)'}"></span>
        {$capture.capturing ? 'Listening — Stop' : listenBusy ? 'Starting…' : 'Start listening'}
      </button>
      <button class="ctl" on:click={() => setDetection(!$capture.detectionOn)} disabled={!$capture.available}>
        <span class="dot" style="background:{$capture.detectionOn ? 'var(--v-emerald)' : '#47464a'}"></span>
        Detection {$capture.detectionOn ? 'active' : 'off'}
      </button>
      <button class="ctl" on:click={clearAll} disabled={!$capture.available}>
        <span class="dot" style="background:#47464a"></span>Clear all<span class="ctl-k">Esc</span>
      </button>
      <button class="ctl" class:rec={$screenBlack} on:click={blackAll} disabled={!$capture.available}>
        <span class="dot" style="background:#000;border:1px solid #47464a"></span>Black<span class="ctl-k">B</span>
      </button>
      <div class="ctl cd-ctl">
        <span class="dot" style="background:var(--v-cyan)"></span>Countdown
        <input class="cd-min" type="number" min="1" max="120" bind:value={cdMin} aria-label="Countdown minutes" disabled={!$capture.available} />
        <span class="cd-unit mono">min</span>
        <button class="cd-go" class:armed={cdArmed} on:click={beginCountdown} disabled={!$capture.available}>
          {cdArmed ? 'Confirm?' : 'Start'}
        </button>
      </div>
      <button class="ctl" on:click={openMainOutput} disabled={!$capture.available}>
        <span class="dot" style="background:var(--v-amber)"></span>Open output
      </button>
      <!-- Rehearsal sits with the other transport controls, not buried in Settings.
           Practising is part of running the service, not configuring the app. -->
      <button class="ctl reh-ctl" class:on={$rehearsing} on:click={toggleRehearsal}
        disabled={!$capture.available || rehBusy}>
        <span class="dot" style="background:{$rehearsing ? 'var(--v-amethyst)' : '#47464a'}"></span>
        {$rehearsing ? 'Rehearsing — go live' : 'Rehearse'}
      </button>
      <div class="hints">
        <span class="hint"><kbd>?</kbd> keys</span>
      </div>
    </div>
  </section>

  {#if $capture.audioError}<div class="audioerr">Audio: {$capture.audioError}</div>{/if}
  {#if $capture.outputError}<div class="audioerr">Output: {$capture.outputError}</div>{/if}

  <!-- Only while listening, and only when something is genuinely wrong. A warning
       that is always on screen is wallpaper. -->
  {#if $capture.capturing && qualityWarning}
    <div class="sttwarn"><b>{qualityWarning.title}</b>{qualityWarning.fix}</div>
  {/if}

  <!-- No STT model = the AI cannot listen. Relay degrades to a fully working MANUAL
       tool, never a dead one — and it can fix itself in one click. -->
  {#if $capture.available && !$capture.stt.loaded}
    <ModelSetup compact />
  {/if}
</div>

<style>
  /* LIVE — the Console's original "Spiritual High-Tech" language, now covering the
     merged surface. The --s-* names alias the global --v-* design tokens (app.css)
     wherever they match; a few Console-specific tones (elevation steps above surf3,
     glows with no --v- equivalent) stay as literals. */
  .stx{
    --s-bg:var(--v-surf); --s-lowest:var(--v-bg); --s-low:var(--v-surf2); --s-cont:var(--v-surf3);
    --s-high:#2a2a2b; --s-on:var(--v-txt); --s-onvar:#c8c6ca; --s-outline:#8b8a8e; --s-outvar:#47464a;
    --s-gold:var(--v-amber); --s-ongold:var(--v-amber-ink); --s-gold-glow:var(--v-amber-glow);
    --s-cyan:var(--v-cyan); --s-amethyst:var(--v-amethyst); --s-rose:var(--v-rose);
    --hair:rgba(255,255,255,.08); --hair2:rgba(255,255,255,.12);
    color:var(--s-on);font-family:var(--f-body);
    /* Fill the scroll area exactly — the console stays fixed on screen; only the
       feed and the two run columns scroll internally. */
    height:100%;display:flex;flex-direction:column;gap:14px;min-height:0;
  }
  .stx .mono{font-family:var(--f-mono);font-variant-numeric:tabular-nums;letter-spacing:.04em}
  .stx .dim{color:var(--s-onvar)}
  .stx .gold{color:var(--s-gold)}
  .ic{display:block}

  /* ── rehearsal band ── amethyst, never amber. Amber means ON AIR. */
  .reh{flex:0 0 auto;display:flex;align-items:center;gap:11px;padding:11px 15px;border-radius:11px;
    background:rgba(192,139,255,.12);border:1px solid rgba(192,139,255,.42);
    font-size:12.5px;line-height:1.5;color:var(--s-onvar)}
  .reh b{font-family:var(--f-mono);font-size:11px;font-weight:700;letter-spacing:.14em;color:var(--s-amethyst);flex:0 0 auto}
  .reh span:not(.reh-dot){flex:1}
  .reh-dot{width:8px;height:8px;border-radius:50%;flex:0 0 auto;background:var(--s-amethyst);
    box-shadow:0 0 9px var(--s-amethyst);animation:pulse 1.7s ease-in-out infinite}
  .reh-end{flex:0 0 auto;padding:7px 14px;border-radius:8px;cursor:pointer;font-family:var(--f-body);
    font-size:11px;font-weight:700;letter-spacing:.06em;text-transform:uppercase;
    background:var(--s-amethyst);border:0;color:#2a0d45}
  .reh-end:disabled{opacity:.5;cursor:not-allowed}

  /* ── transport ── */
  .tbar{flex:0 0 auto;display:flex;align-items:center;gap:12px;padding:10px 14px;
    background:var(--s-low);border:1px solid var(--hair2);border-radius:12px}
  .spring{flex:1}
  .sys{display:flex;align-items:center;gap:8px;flex:0 0 auto;font-family:var(--f-mono);font-size:10px;
    font-weight:700;letter-spacing:.12em;color:var(--s-outline);padding:6px 12px;border-radius:99px;
    background:var(--s-cont);border:1px solid var(--hair)}
  .sys-dot{width:7px;height:7px;border-radius:50%;background:currentColor}
  .sys.live{color:var(--s-gold);border-color:rgba(255,185,95,.4);background:rgba(245,166,35,.13)}
  .sys.live .sys-dot{box-shadow:0 0 9px currentColor;animation:pulse 1.7s ease-in-out infinite}
  .sys.reh{color:var(--s-amethyst);border-color:rgba(192,139,255,.42);background:rgba(192,139,255,.12)}
  .sys.blk{color:var(--s-rose);border-color:rgba(244,113,139,.42);background:rgba(244,113,139,.12)}
  @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}}
  .t-plan{display:flex;align-items:baseline;gap:8px;background:none;border:0;padding:5px 8px;border-radius:8px;
    color:var(--s-on);font-family:var(--f-body);font-size:13px;font-weight:600;cursor:pointer}
  .t-plan:hover{background:var(--s-cont)}
  .t-pos{font-size:10px;color:var(--s-outline);font-weight:400}
  .t-mode{flex:0 0 auto;font-size:10px;letter-spacing:.06em;color:var(--s-outline);padding:6px 10px;
    border-radius:7px;background:var(--s-cont);border:1px solid var(--hair)}
  .t-mode b{color:var(--s-cyan);font-weight:700}
  .t-mode.slide b{color:var(--s-gold)}
  @media (max-width:1240px){.t-mode{display:none}}
  .t-nav{display:flex;align-items:center;gap:7px;flex:0 0 auto}
  .nav-sq{width:32px;height:32px;border-radius:8px;display:grid;place-items:center;cursor:pointer;
    background:var(--s-cont);border:1px solid var(--hair);color:var(--s-onvar);font-size:13px;transition:.14s}
  .nav-sq:hover{background:var(--s-high);color:var(--s-on)}

  /* ── layout ── feed on the left, wall + run columns on the right */
  .stx-top{flex:1;min-height:0;display:grid;grid-template-columns:360px minmax(0,1fr);gap:14px}
  .rightcol{min-height:0;display:flex;flex-direction:column;gap:14px}
  .runrow{flex:1;min-height:0;display:grid;grid-template-columns:1fr 1fr;gap:14px}
  @media (max-width:1180px){
    .stx{height:auto}
    .stx-top{grid-template-columns:1fr}
    .runrow{grid-template-columns:1fr}
  }

  .tile{background:var(--s-low);border:1px solid var(--hair2);border-radius:12px;overflow:hidden;
    display:flex;flex-direction:column;min-height:0}
  .tile.feed{background:var(--s-lowest)}
  .tile.entry{flex:0 0 auto}
  .tile.channels{flex:0 0 auto}
  .chan-wrap{padding:18px}
  .tile-head{display:flex;align-items:center;justify-content:space-between;padding:13px 16px;
    border-bottom:1px solid var(--hair);flex:0 0 auto;gap:10px}
  .tile-head h2,.tile-head h3{margin:0;font-family:var(--f-body);font-size:11px;font-weight:700;
    letter-spacing:.16em;text-transform:uppercase;color:var(--s-on);
    overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .tile-head h3{color:var(--s-onvar)}
  .tile-head .mono{font-size:10px;flex:0 0 auto}

  /* ── feed ── */
  .tx-box{flex:0 0 auto;padding:14px 16px 12px;border-bottom:1px solid var(--hair);background:var(--s-lowest)}
  .tx-stream{height:92px;overflow-y:auto;margin-top:9px;font-size:14px;line-height:1.6;color:var(--s-on);
    font-weight:500;scrollbar-width:thin;scrollbar-color:var(--s-high) transparent}
  .tx-stream::-webkit-scrollbar{width:6px}
  .tx-stream::-webkit-scrollbar-thumb{background:var(--s-high);border-radius:99px}
  .tx-stream mark{background:rgba(255,185,95,.16);color:var(--s-gold);border-radius:3px;padding:0 2px}
  .caret{display:inline-block;width:2px;height:14px;background:var(--s-gold);vertical-align:-2px;
    margin-left:1px;animation:blink 1.05s steps(1) infinite}
  @keyframes blink{50%{opacity:0}}
  .feed-body{flex:1;min-height:0;overflow-y:auto;padding:16px;display:flex;flex-direction:column;gap:16px}
  .seg-top{display:flex;align-items:center;justify-content:space-between}
  .lbl-gold{font-family:var(--f-mono);font-size:10px;font-weight:600;letter-spacing:.16em;
    text-transform:uppercase;color:var(--s-gold)}
  .lbl-dim{font-family:var(--f-mono);font-size:9px;font-weight:700;letter-spacing:.16em;
    text-transform:uppercase;color:var(--s-onvar)}
  .ai-card{background:var(--s-low);border:1px solid rgba(255,185,95,.28);border-radius:10px;padding:14px;
    box-shadow:0 0 20px -5px var(--s-gold-glow)}
  .ai-top{display:flex;align-items:center;justify-content:space-between;margin-bottom:10px}

  /* A GUESS MUST LOOK LIKE A GUESS.
     Gold in this app reads as "Relay is confident" — it is the colour of the accept
     button and of a heard reference. A paraphrase has not earned it: its score is a
     cosine, not a probability, and router.rs will not let it auto-fire at ANY value.
     So it loses the gold border, the glow, and the number.

     Cyan, NOT amethyst — even though amethyst is the obvious "uncertain" colour and
     was the original suggestion. Amethyst already means REHEARSAL (docs/DECISIONS.md
     §18: amber means ON AIR and a tally light that lies is worse than none). A colour
     that means "nothing is reaching the congregation" cannot also mean "this guess is
     shaky", or on the day both are true the operator reads the wrong one. */
  .ai-card.guess{border-color:rgba(63,182,230,.30);box-shadow:none}
  .lbl-method{font-family:var(--f-mono);font-size:10px;font-weight:600;letter-spacing:.16em;
    text-transform:uppercase;color:var(--s-gold)}
  .lbl-method.guess{color:var(--v-cyan)}
  .lbl-method.sm{font-size:9px;font-weight:700}
  .guess-note{font-size:10.5px;color:var(--v-cyan);opacity:.85}
  .xref.guess{border-color:rgba(63,182,230,.22)}

  /* Confidence as a BAR, not a bare number — "0.92" means nothing to a volunteer.
     Only ever drawn for a heard reference, because it is the only one whose number
     means what it appears to mean. */
  .conf{height:3px;border-radius:2px;background:rgba(255,255,255,.07);margin:0 0 11px;overflow:hidden}
  .conf i{display:block;height:100%;background:var(--s-gold);border-radius:2px}

  /* THE EVIDENCE — the words that actually triggered the match. */
  .why{display:flex;align-items:baseline;gap:8px;margin-top:7px;flex-wrap:wrap}
  .why-lbl{font-family:var(--f-mono);font-size:9px;font-weight:700;letter-spacing:.14em;
    text-transform:uppercase;color:var(--s-outline);flex:none}
  .why-txt{font-family:var(--f-mono);font-size:11.5px;color:var(--s-onvar);
    background:rgba(255,255,255,.04);border-radius:5px;padding:2px 7px;
    overflow-wrap:anywhere}
  .why.sm .why-txt{font-size:10.5px}
  .ai-ref{font-family:var(--f-serif);font-size:19px;font-weight:600;letter-spacing:-.01em;color:var(--s-on)}
  .ai-verse{font-family:var(--f-serif);font-style:italic;font-size:13.5px;line-height:1.5;
    color:var(--s-onvar);margin:6px 0 13px}
  .ai-acts{display:flex;gap:8px;align-items:center}
  .xref{background:rgba(28,27,28,.6);border:1px solid var(--hair);border-radius:10px;padding:13px}
  .xref-top{display:flex;align-items:center;justify-content:space-between;margin-bottom:7px}
  .xref-ref{font-family:var(--f-serif);font-size:15px;font-weight:600;color:var(--s-on)}
  .xref-verse{font-family:var(--f-serif);font-style:italic;font-size:12.5px;color:var(--s-onvar);
    margin-top:4px;line-height:1.5}
  .xref-acts{display:flex;gap:7px;margin-top:11px}
  .empty{color:var(--s-outline);font-size:12.5px;line-height:1.6;padding:10px 2px}
  .empty b{color:var(--s-onvar)}

  /* ── the run columns ── */
  .listbody{flex:1;min-height:0;overflow-y:auto;padding:12px;display:flex;flex-direction:column;gap:7px;
    scrollbar-width:thin;scrollbar-color:var(--s-high) transparent}
  .cue,.slide{display:flex;align-items:center;gap:9px;width:100%;flex:0 0 auto;text-align:left;cursor:pointer;
    padding:9px 10px;border-radius:9px;background:var(--s-cont);border:1px solid var(--hair);
    color:var(--s-on);font-family:var(--f-body);transition:.14s}
  .cue:hover,.slide:hover{border-color:var(--hair2);background:var(--s-high)}
  .cue.sel{border-color:rgba(63,182,230,.5)}
  /* Amber = it is in front of the congregation. Nothing else in this app may use it. */
  .cue.islive,.slide.islive{border-color:var(--s-gold);background:rgba(245,166,35,.12)}
  /* CUED = where → will resume from, but NOT on screen. Deliberately not amber. */
  .cue.cued,.slide.cued{border-style:dashed;border-color:var(--s-outline)}
  .cue-stripe,.slide-stripe{width:3px;align-self:stretch;border-radius:99px;flex:0 0 auto}
  .slide-stripe{background:var(--acc)}
  .cue-num{font-size:10px;color:var(--s-outline);flex:0 0 auto}
  .cue-body{flex:1;min-width:0;display:flex;flex-direction:column;gap:2px}
  .cue-title{font-size:12.5px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .cue-meta{font-size:9px;letter-spacing:.05em;color:var(--s-outline)}
  .cue-tag{flex:0 0 auto;font-size:8.5px;font-weight:700;letter-spacing:.09em;color:var(--s-gold)}
  .cue-tag.cued,.cue-tag.go{color:var(--s-outline)}
  .cue.pick:hover .cue-tag.go{color:var(--s-gold)}
  .slide{align-items:flex-start}
  .slide-tag{flex:0 0 auto;min-width:26px;padding-top:2px;font-size:9px;font-weight:700;
    letter-spacing:.05em;color:var(--acc)}
  .slide-text{flex:1;min-width:0;font-family:var(--f-serif);font-size:12.5px;line-height:1.5;
    color:var(--s-onvar);white-space:pre-wrap;
    display:-webkit-box;-webkit-line-clamp:4;-webkit-box-orient:vertical;overflow:hidden}
  .slide.islive .slide-text{color:var(--s-on)}
  .note{flex:0 0 auto;display:flex;align-items:flex-start;gap:7px;padding:8px 10px;border-radius:8px;
    background:rgba(192,139,255,.1);border:1px solid rgba(192,139,255,.3);
    color:var(--s-amethyst);font-size:11.5px;line-height:1.5}
  .note svg{flex:0 0 auto;margin-top:2px}

  /* ── buttons ── */
  .btn-gold{padding:9px 16px;border-radius:8px;border:0;cursor:pointer;font-family:var(--f-body);
    font-size:11px;font-weight:700;letter-spacing:.08em;text-transform:uppercase;color:var(--s-ongold);
    background:var(--s-gold);transition:.14s}
  .btn-gold:hover:not(:disabled){filter:brightness(1.06)}
  .btn-gold.lg{padding:0 22px;height:42px;font-size:12px;flex:0 0 auto}
  .btn-gold:disabled{opacity:.45;cursor:not-allowed}
  .btn-x{padding:8px 13px;border-radius:8px;background:transparent;border:1px solid var(--hair2);
    color:var(--s-onvar);font-family:var(--f-body);font-size:11px;cursor:pointer;transition:.14s}
  .btn-x:hover{border-color:var(--s-rose);color:var(--s-rose)}
  .btn-mini{padding:5px 11px;border-radius:6px;border:0;cursor:pointer;font-family:var(--f-body);
    font-size:11px;font-weight:600;color:var(--s-ongold);background:var(--s-gold);transition:.14s}
  .btn-mini:hover{filter:brightness(1.06)}
  .btn-mini.ghost{background:transparent;border:1px solid var(--hair2);color:var(--s-onvar)}
  .btn-mini.ghost:hover{border-color:var(--s-outline);color:var(--s-on)}

  /* ── command bar ── */
  .entry-row{display:flex;gap:12px;padding:14px 16px;border-bottom:1px solid var(--hair)}
  .search{flex:1;display:flex;align-items:center;gap:11px;background:var(--s-lowest);
    border:1px solid var(--hair);border-radius:10px;padding:0 14px;height:42px}
  .search input{flex:1;min-width:0;background:transparent;border:0;outline:none;color:var(--s-on);
    font-family:var(--f-mono);font-size:12.5px}
  .search input::placeholder{color:var(--s-outline)}
  .search:focus-within{border-color:rgba(255,185,95,.4);box-shadow:0 0 0 3px rgba(255,185,95,.08)}
  .err{color:var(--s-rose);font-size:11.5px;padding:0 16px;margin-top:8px}
  .livemsg{display:flex;align-items:center;gap:8px;color:var(--v-emerald);font-size:11.5px;font-weight:500}
  .lm-dot{width:7px;height:7px;border-radius:50%;background:var(--v-emerald);box-shadow:0 0 8px var(--v-emerald)}
  .entry-controls{display:flex;flex-wrap:wrap;align-items:center;gap:10px;padding:12px 16px}
  .ctl{display:flex;align-items:center;gap:9px;padding:9px 13px;border-radius:9px;background:var(--s-cont);
    border:1px solid var(--hair);color:var(--s-on);font-family:var(--f-body);font-size:12px;
    cursor:pointer;transition:.14s}
  .ctl:hover:not(:disabled){background:var(--s-high);border-color:var(--hair2)}
  .ctl:disabled{opacity:.45;cursor:not-allowed}
  .ctl.rec{background:rgba(255,120,110,.12);border-color:rgba(255,120,110,.35);color:var(--s-rose)}
  .reh-ctl.on{background:rgba(192,139,255,.14);border-color:rgba(192,139,255,.42);color:var(--s-amethyst)}
  .dot{width:8px;height:8px;border-radius:50%;flex:0 0 auto}
  .cd-ctl{cursor:default;gap:7px}
  .cd-min{width:46px;padding:3px 6px;border-radius:6px;border:1px solid var(--hair2);background:var(--s-bg);
    color:var(--s-on);font-family:var(--f-mono);font-size:12px;text-align:center}
  .cd-unit{font-size:9px;color:var(--s-outline);margin-left:-3px}
  .cd-go{padding:4px 11px;border-radius:6px;border:1px solid rgba(63,182,230,.4);
    background:rgba(63,182,230,.14);color:var(--s-cyan);font-family:var(--f-mono);font-size:10px;
    font-weight:700;letter-spacing:.04em;cursor:pointer;transition:.12s}
  .cd-go:hover:not(:disabled){background:rgba(63,182,230,.26)}
  .cd-go:disabled{opacity:.45;cursor:not-allowed}
  .cd-go.armed{background:rgba(245,166,35,.2);border-color:rgba(245,166,35,.5);color:var(--s-gold)}
  .ctl-k{font-family:var(--f-mono);font-size:9px;color:var(--s-outline);margin-left:2px}
  .hints{display:flex;gap:14px;margin-left:auto}
  .hint{display:flex;align-items:center;gap:6px;font-size:11px;color:var(--s-outline)}
  .hint kbd{font-family:var(--f-mono);font-size:9px;color:var(--s-onvar);background:var(--s-high);
    border:1px solid var(--hair2);border-bottom-width:2px;border-radius:4px;padding:2px 6px}

  /* ── banners ── */
  .audioerr{flex:0 0 auto;background:rgba(147,0,10,.18);color:var(--s-rose);
    border:1px solid rgba(255,157,148,.3);border-radius:9px;padding:9px 12px;font-size:12px}
  /* Degraded, not broken: amber (a warning), never rose (an error) — the app is
     still fully usable by hand, and the banner should read that way. */
  .sttwarn{flex:0 0 auto;background:var(--v-amber-soft);color:var(--v-txt);
    border:1px solid rgba(245,166,35,.34);border-radius:9px;padding:10px 12px;font-size:12px;line-height:1.6}
  .sttwarn b{display:block;margin-bottom:2px;color:var(--v-amber2)}

  /* ── accessibility ── */
  .btn-gold:focus-visible,.btn-x:focus-visible,.btn-mini:focus-visible,.ctl:focus-visible,
  .nav-sq:focus-visible,.cue:focus-visible,.slide:focus-visible,.reh-end:focus-visible,
  .t-plan:focus-visible{outline:2px solid var(--s-gold);outline-offset:2px}
  @media (prefers-reduced-motion:reduce){
    .caret,.sys.live .sys-dot,.reh-dot{animation:none}
  }
</style>
