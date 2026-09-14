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
  import { onMount, onDestroy } from 'svelte';
  import { rangeFill } from '../rangefill.js';
  import { describeScreen, SCREEN_BADGE, screenSwitch, screenKind } from '../outputHealth.js';
  import TemplateRender from '../TemplateRender.svelte';
  import { resolveOutputTemplate } from '../layers.js';
  import ModelSetup from '../ModelSetup.svelte';
  import { registerContext } from '../shortcuts.js';
  import { t } from '../i18n.js';
  import EmptyState from '../ui/EmptyState.svelte';
  import ErrorState from '../ui/ErrorState.svelte';
  import Loading from '../ui/Loading.svelte';
  import { heard, methodKey, inLibrary } from '../detect.js';
  import DetectionInspector from '../DetectionInspector.svelte';
  import { humanError as humanErrorBase } from '../errors.js';
  import { typeOf, payloadOf, slidesOf, slideAccent, cueSub, nextOf, stepFrom } from '../plan.js';
  import { gridSource, pressArbiter } from '../slidegrid.js';
  import LiveRail from '../LiveRail.svelte';
  import { parsePassage } from '../passage.js';
  import { session, setSession } from '../session.js';
  import { get } from 'svelte/store';
  import {
    capture,
    liveContent,
    liveTemplateOverride,
    liveTemplatePinned,
    transcript,
    detections,
    resolvedDetections,
    MAX_RESOLVED,
    live,
    screenBlack,
    liveCue,
    templates,
    loadTemplates,
    defaultTemplateId,
    loadDefaultTemplate,
    confirmDetection,
    dismissDetection,
    manualFire,
    fireContent,
    fireMedia,
    listOutputChannels,
    channelHealth,
    channelWaiting,
    openChannelOutput,
    closeChannelOutput,
    refreshChannelHealth,
    listMonitors,
    setChannelDisplay,
    startCountdown,
    setDetection,
    startCapture,
    stopCapture,
    relatedScripture,
    navVerse,
    navNotice,
    navBlocked,
    listPlans,
    planItems,
    setStageNext,
    rehearsing,
    loadRehearsal,
    setRehearsal,
    getSensitivity,
    setSensitivity,
    pushAnnouncement,
    verseRepeatCount,
    chapterVerses,
    readErrors,
  } from '../stores/capture.js';

  // ── the plan being RUN (not edited) ──────────────────────────────────────
  let plans = [];
  let openPlan = null;
  let items = [];
  let selId = null;

  // "Have we actually asked the database yet?" — NOT the same question as "is the
  // list empty", and conflating them is a lie the operator sees every single mount.
  //
  // These lists start as `[]` and are filled by an await. The empty states rendered
  // off `.length` alone, so for the first frames of every visit to Live, an operator
  // with a full plan library was told "No service plans yet" — the one message that
  // makes a new operator think they have lost their work.
  let plansLoaded = false;
  let itemsLoaded = false;

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
  // `!$screenBlack` is load-bearing, and its absence was a live-safety bug.
  //
  // `$live` means "content is ARMED", not "a congregation is looking at it" — the
  // same confusion the amber badge on the run rail had. A blackout leaves `$live`
  // set and blanks the screens, so this read VERSE mode after `B` and SLIDE mode
  // after `Esc`: the same conceptual state, two panic keys, opposite transports,
  // and only the Esc behaviour was documented. The consequence was worse than the
  // inconsistency — after a blackout mid-plan the next `→` fired a verse from an
  // earlier passage AND cancelled the blackout, so the emergency key was undone by
  // the key an operator presses more than any other.
  //
  // Now it matches the sentence above it: verse mode means something that did not
  // come from the plan is genuinely IN FRONT OF PEOPLE.
  $: mode = openPlan && items.length && !($live && !$screenBlack && !planOnAir) ? 'slide' : 'verse';

  // Named so the plan picker's error state has something to retry with (RG-95).
  async function loadPlans() {
    plans = await listPlans().catch(() => []);
    plansLoaded = true;
  }

  async function loadPlan(p) {
    openPlan = p;
    // Loading a plan is the operator asking for the plan. A chapter or a song
    // they staged from the rail earlier must not keep outranking it.
    railChapter = null;
    itemsLoaded = false;
    items = await planItems(p.id);
    itemsLoaded = true;
    selId = items[0]?.id ?? null;
    // A playhead from a DIFFERENT plan is meaningless here, and worse than
    // meaningless: its cue id could collide with one in this plan and light up an
    // unrelated cue as CUED.
    liveCue.set({ cueId: null, slide: 0, onAir: false });
  }
  function leave() {
    openPlan = null;
    items = [];
    railChapter = null;
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
      flash($rehearsing ? $t('live.rehearsal_on') : $t('live.rehearsal_off'));
      errMsg = '';
    } catch (e) {
      errMsg = humanError(e);
    }
    rehBusy = false;
  }

  // ── OUTPUT HEALTH — the screens answer for themselves ────────────────────
  //
  // This pane used to derive every badge from GLOBAL state: if content was live
  // and we were not rehearsing or blacked out, every screen read **On Air**. That
  // is not a status; it is a restatement of what Relay believes it sent, wearing
  // the costume of a report about what happened. A projector whose window had
  // frozen, an OBS source whose tab had been killed, a display that had gone to
  // sleep — all three read On Air, in amber, forever, on the one surface an
  // operator glances at during a service to rule exactly that out.
  //
  // Now each screen reports that it is painting (`outputBeat.js` → Rust
  // `OutputHealth`), and this pane shows what the screen said. A badge that cannot
  // detect its own failure is not a badge.
  // The poll lives in the store (`startChannelHealth`, started by the shell), not
  // here: the Outputs table and the shell's degraded banner want the same answer,
  // and three timers asking one question would let three surfaces disagree about
  // the same screen for up to two seconds — the asymmetry RG-01 exists to end.

  // The badge rule itself lives in `lib/outputHealth.js` and is PURE, so it can be
  // tested without mounting this view and so the Outputs inspector cannot end up
  // saying something different about the same screen. All four inputs are named in
  // this expression on purpose — a helper that closed over the health map would not be
  // tracked by Svelte's reactivity and the pane would freeze on its first reading,
  // which is the same class of bug as the badge it replaces.
  $: outs = channels.map((c) => ({
    c,
    s: describeScreen(
      $channelHealth[c.id] ?? null,
      { rehearsing: $rehearsing, live: !!$live, black: $screenBlack },
      $channelWaiting[c.id] ? Date.now() - $channelWaiting[c.id] : 0,
    ),
    // Same health row, same pure rule, so the badge and the button can never
    // describe different screens.
    w: screenSwitch($channelHealth[c.id] ?? null, c),
  }));

  // ── SWITCHING A SCREEN ON OR OFF ────────────────────────────────────────────
  //
  // Exempt from the Service Lock on purpose (`servicelock.rs`): a screen that has
  // to be restored mid-service is exactly the case the lock must not block.
  //
  // It is NOT a panic control and must never read like one — it acts on one
  // screen, it is not on a key, and it does not touch what is on air. Blackout
  // and Clear remain the two things that do.
  let switching = null;
  let screenMsg = '';
  async function toggleScreen(c, action) {
    switching = c.id;
    screenMsg = '';
    try {
      if (action === 'on') await openChannelOutput(c.id);
      else await closeChannelOutput(c.id);
      // Ask the backend what actually happened rather than assuming the command
      // that returned did the thing. The pane's whole point is that a screen
      // answers for itself.
      await refreshChannelHealth();
      screenMsg = `${c.name} ${action === 'on' ? 'opened' : 'closed'}.`;
    } catch (e) {
      screenMsg = humanError(e);
    }
    switching = null;
  }

  // ── SAFE SCREEN — what the operator is told BEFORE the congregation notices ──
  //
  // Two things the console could never say, and both of them look to an operator
  // exactly like Relay working:
  //
  //  1 · The verse is on the wall at a size nobody past the third row can read.
  //      The fit loop always "succeeds" — 40 rounds of ×0.95 will squeeze anything
  //      in — so a template that had stopped working looked like one that was.
  //      `TemplateRender` now reports when it had to go below the size the
  //      template's designer asked for, and this is where that lands. The program
  //      pane renders through the SAME component as the wall, so the measurement
  //      is the wall's rather than a guess about it.
  //
  //  2 · Nothing is attached to show it. REPORTED, never enforced: a service runs
  //      on the console preview alone all the time — during setup, in rehearsal,
  //      while somebody re-cables a projector — and refusing to fire because no
  //      screen happens to be connected would take the operator's tool away at the
  //      exact moment they are fixing the screen.
  let fitWarning = '';
  function noteFit(f) {
    fitWarning = f.legible
      ? ''
      : `This is rendering at ${Math.round(f.scale * 100)}% of the template's size to fit — ` +
        'it may not be readable from the back. Try a shorter passage or a template with more room.';
  }
  // Only while something is actually on air: a cleared wall with no screens
  // attached is not a problem, it is a Tuesday.
  $: nowhereToShow =
    !!$live &&
    !$rehearsing &&
    outs.length > 0 &&
    outs.every((o) => o.s.kind === 'down' || o.s.kind === 'idle' || o.s.kind === 'unknown');

  // A screen falling over mid-service is exactly the thing an operator finds out
  // about too late by looking. Announce it once, on the transition, through the
  // same polite region the AI's suggestions use — never repeatedly, which is how a
  // live region becomes noise an operator learns to tune out.
  let downAnnounce = '';
  let wasDown = {};
  $: {
    const nowDown = {};
    for (const o of outs) if (o.s.kind === 'down') nowDown[o.c.id] = o.c.name;
    const fresh = Object.keys(nowDown).filter((id) => !wasDown[id]);
    if (fresh.length)
      downAnnounce = `${fresh.map((id) => nowDown[id]).join(', ')} is not responding.`;
    wasDown = nowDown;
  }

  // HAS THIS VIEW ALREADY GONE AWAY? `onMount` is async and Svelte does not wait
  // for it: `onDestroy` runs the instant the operator switches workspace, which
  // can be in the middle of the awaits below. Every step after an await has to
  // ask this before it writes anything that outlives the component — a store, a
  // subscription, the playhead.
  let dead = false;

  onMount(async () => {
    // ── EVERYTHING THAT OUTLIVES THIS VIEW IS SET UP BEFORE THE FIRST AWAIT ──
    //
    // This used to sit at the BOTTOM of the async body, after five backend round
    // trips, and that was a live-safety bug rather than untidiness.
    // `registerContext` is ONE global slot and the last writer wins, so a
    // workspace switch inside the mount window ran `onDestroy` against three
    // `undefined`s — tearing nothing down — and then let a view that no longer
    // exists take ownership of `→`, `←` and `Space` on whatever tab the operator
    // had moved to. They press the key they press more than any other and a plan
    // slide reaches the congregation from a surface they cannot see.
    //
    // None of the three needs data, so none of them waits for any.
    // Pinned by `liveunmount.test.js`.

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

    // THE TWO LOOSE ENDS OF A CLEAR, watched at the store rather than owned by a
    // button. Live used to carry its own Clear screens; the dock owns that control
    // now, and the panic key and a spoken clear never went through it anyway. What
    // must still happen when the wall goes clear, however it was cleared:
    //   · a press armed a beat ago must not paint a verse over a cleared wall
    //   · the preacher's "up next" must not outlive the content it was about
    // The second one reports its own failure, because until 2026-08-14 nothing
    // anywhere did and a preacher read a stale hint for a whole service.
    let wasLive = !!get(live);
    unsubLive = live.subscribe((v) => {
      const now = !!v;
      if (wasLive && !now) {
        gridPress.cancel();
        setStageNext(null, null).catch((e) =>
          flash(`The preacher's stage monitor may still show the old "up next" — ${humanError(e)}`),
        );
      }
      wasLive = now;
    });

    // A SPOKEN "next"/"back" that did nothing. It comes from the STT thread, which
    // has no caller to hand a result back to, so it arrives as an event. The
    // preacher says "next", the wall does not move — and now the console says why
    // instead of leaving the operator to wonder whether Relay even heard it.
    unsubNav = navBlocked.subscribe((r) => {
      const notice = navNotice(r);
      if (notice) {
        flash(notice);
        navBlocked.set(null);
      }
    });

    await loadRehearsal();
    if (dead) return;
    // ONLY WHEN THERE IS AN ENGINE TO ASK. `getSensitivity` is a GROUP 2 wrapper:
    // with no backend it answers 50 and says nothing — and 50 is a real setting,
    // so the dial cannot tell "the gate is at 50" from "nobody answered". That is
    // rule 35 on a control rather than on a badge. The dial shows `—` until a
    // real answer lands, and `sensKnown` is what the value column reads.
    if ($capture.available)
      getSensitivity().then((v) => {
        if (dead) return;
        sensitivity = v;
        sensKnown = true;
      });
    // Populate the reactive `$templates` store so the preview/program panes
    // resolve (and stay live to edits) from it, not just a one-shot snapshot.
    await loadTemplates().catch(() => {});
    await loadDefaultTemplate().catch(() => {});
    if (dead) return;
    channels = await listOutputChannels().catch(() => []);
    if (dead) return;
    await loadPlans();
    if (dead) return;

    // Resume where the operator actually was. The output windows are separate
    // webviews and survive a console crash, so the verse is still on the wall —
    // restoring the cursor WITHOUT re-firing makes the transport agree with what
    // the congregation is looking at.
    const saved = get(session);
    if (saved.planId) {
      const p = plans.find((x) => x.id === saved.planId);
      if (p) {
        // `loadPlan` RESETS the playhead. Doing that from a view the operator has
        // already left would put the next `→` back at cue 1 — the opening
        // countdown, at the end of the service.
        if (dead) return;
        await loadPlan(p);
        if (dead) return;
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
  });
  let unregisterKeys;
  let unsubNav;
  let unsubLive;
  onDestroy(() => {
    // FIRST, so anything the async mount is still holding stops before it writes.
    dead = true;
    unregisterKeys?.();
    unsubNav?.();
    // The emergency announcement's arm timer, cleared for the same reason as the
    // countdown's right above it. It was the one of the pair that was missed.
    clearTimeout(annArmT);
    clearTimeout(liveMsgT);
    unsubLive?.();
    clearTimeout(relatedT); // a pending poll must not fire into a destroyed view
    // A view that has gone away must not put scripture on a wall a beat later.
    gridPress.cancel();
  });

  // ── the transport ────────────────────────────────────────────────────────
  //
  // `→` in VERSE mode used to be fire-and-forget into a command that returned
  // nothing. Three things inside it could silently do nothing — a poisoned lock, the
  // end of the passage, a verse missing from the corpus — and the operator got no
  // error, no toast and no log. On the key they press more than any other, in the
  // middle of a sermon. It now always says what happened.
  async function step(dir) {
    if (mode === 'slide') return stepLive(dir);
    try {
      const notice = navNotice(await navVerse(dir > 0 ? 'next' : 'back'));
      if (notice) flash(notice);
    } catch (e) {
      flash(humanError(e));
    }
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
    const stageNote = p.stage_note || null;
    // The template the operator set for THIS cue in the Planner. Passed on every
    // fire so a plan item renders with its own chosen look, not just the
    // content-type default. null → the backend falls back to that default.
    const tpl = item.template_id ?? null;
    try {
      if (item.cue_type === 'scripture') {
        // keepPlan: TRUE — this is a plan slide, so the transport must stay in
        // Slide mode. Without it, manualFire's leavePlan() flipped us to Verse
        // mode the moment a scripture cue fired, and the next → walked the passage
        // instead of advancing the plan. That was the Slide-mode bug.
        await manualFire(p.reference || item.label, stageNote, tpl, true);
      } else if (item.cue_type === 'media') {
        if (!p.media_id) {
          flash('Media asset missing — re-add it from the Library.');
          return;
        }
        await fireMedia(p.media_id, tpl, true); // keepPlan — this IS the plan's slide
      } else if (item.cue_type === 'countdown') {
        await startCountdown(
          Number(p.minutes) || 5,
          p.label || 'Service begins in',
          p.done || 'Welcome',
          tpl,
          true, // keepPlan — this IS the plan's slide
        );
      } else if (item.cue_type === 'song') {
        // Lyrics carry NO title/section on the live screen — and `fire_content`
        // is the ONE place that decides that (CLAUDE.md rule 36). Passing an
        // empty label here suppressed it a second time, in the wrong place: the
        // service record then had nothing to say about which song was on screen,
        // and the Library's own fire (which passes the label) disagreed with this
        // one about the same rule.
        await fireContent(item.label, s.text, 'song', stageNote, tpl, true); // keepPlan
      } else {
        await fireContent(item.label, s.text, 'announce', stageNote, tpl, true); // keepPlan
      }
      // Mark the cue live ONLY after the fire resolves. Setting onAir before the
      // await meant a failed fire left this cue amber "On Air" — and the reactive
      // setSession persisted that lie across a reload — while the wall still
      // showed the previous content. Amber must never claim a screen it did not
      // reach (CLAUDE.md rule 18; matches manualFire, which also sets after).
      setLive(item.id, i);
      selId = item.id;
      flash(`Live: ${s.label}`);
      const n = nextOf(items, item.id, i);
      // Deliberately shrugged: a missing "up next" is an absent hint, and the
      // wall — and this catch — already report anything that matters. Contrast
      // the CLEAR below, which cannot be shrugged.
      setStageNext(n?.label ?? null, n?.text ?? null).catch(() => {});
    } catch (e) {
      flash(humanError(e));
    }
  }

  // ── EMERGENCY ANNOUNCEMENT ────────────────────────────────────────────────
  //
  // Paints a message over whatever is on the wall, on every channel at once —
  // the fire alarm, the blocked car park, the doctor needed at the back. It goes
  // through the same template engine as any slide, so it is not a special screen
  // and needs no per-channel handling.
  //
  // ARMED IN TWO STEPS, exactly like the countdown, and for a stronger reason:
  // this one interrupts live scripture on every screen in the building. A stray
  // Enter in a text field must not be able to do that.
  //
  // `pushAnnouncement` THROWS by contract (it changes what the congregation
  // sees), and this is the one place that can tell the operator — so the catch
  // reports rather than swallowing. Saying nothing here would leave them
  // believing the room had been warned.
  let annMsg = '';
  let annArmed = false;
  let annArmT;
  async function sendAnnouncement() {
    const text = annMsg.trim();
    if (!text) return;
    if (!annArmed) {
      annArmed = true;
      clearTimeout(annArmT);
      annArmT = setTimeout(() => (annArmed = false), 3000);
      return;
    }
    clearTimeout(annArmT);
    annArmed = false;
    try {
      await pushAnnouncement(text);
      flash('Announcement on all screens');
      annMsg = '';
    } catch (e) {
      flash(humanError(e));
    }
  }

  // ── AI suggestions ───────────────────────────────────────────────────────
  $: dets = $detections;

  /**
   * WHAT THE GATE IS ACTUALLY DOING, in words, on the run surface.
   *
   * Rule 35: a status line that reads the same when the thing behind it is
   * broken as when it is fine is not a status line. "Auto-fire on" is true of a
   * listening, armed, model-loaded Relay and of nothing else — a dead model, a
   * stopped microphone and a disarmed detector each get their own sentence, in
   * the order an operator would have to fix them.
   *
   * Relay deliberately has no "suggest only" mode, which the prototype offers: a
   * Direct hit above the bar fires unattended whenever detection is armed. A
   * mode nothing implements is a control that saves a preference nothing reads
   * (DECISIONS §69), so it is not drawn.
   */
  $: gateState = !$capture.detectionOn
    ? { armed: false, label: 'detection off' }
    : !$capture.stt?.loaded
      ? { armed: false, label: 'no speech model' }
      : !$capture.capturing
        ? { armed: false, label: 'not listening' }
        : { armed: true, label: 'auto-fire on' };

  /** How an ended claim is described. Every wording names WHO acted. */
  function outcomeLabel(d) {
    if (d.outcome === 'accepted') return 'Accepted — put on the screens';
    if (d.outcome === 'dismissed') return 'Dismissed by the operator';
    // An auto-fire can only be a heard reference (rule 10 — a paraphrase is
    // capped at Suggest at any score). It still says WHICH, rather than assuming
    // the rule held: if a paraphrase ever appears on this line, that is the
    // finding, and it is visible on the surface an operator is already watching.
    return `Auto-fired · ${heard(d) ? 'heard' : 'paraphrase'}`;
  }

  /**
   * THE COLUMN — pending claims, then the receipts, bounded.
   *
   * Pending first, deliberately, where the prototype is strictly newest-first: a
   * receipt records something that already happened and a pending claim is a
   * decision the operator still owes, and a record must never push a decision
   * out of the column.
   *
   * AND THE CAP FALLS ON THE RECEIPTS, NOT ON THE CLAIMS. Every pending claim is
   * drawn — the store already bounds them at six and prunes them at 45 seconds,
   * which are the two limits that belong to a suggestion — and the receipts then
   * top the column up to `MAX_RESOLVED`. A cap applied to the whole list would
   * hide a decision the operator still owes in order to keep showing a record of
   * one already made, which is the wrong way round at the one moment it matters.
   */
  $: claimCards = [
    ...dets.map((d) => ({ d, outcome: null, key: `p:${d.reference}` })),
    ...$resolvedDetections.slice(0, Math.max(0, MAX_RESOLVED - dets.length)).map((d) => ({
      d,
      outcome: outcomeLabel(d),
      key: `r:${d.reference}:${d.resolvedAt}`,
    })),
  ];

  // heard() / methodLabel() live in lib/detect.js — pure, and unit-tested there,
  // because they are the frontend half of the auto-fire safety rule (see that file).
  // Await it, and flash ONLY if the verse actually went up. This used to fire and
  // forget, then say "Now live: John 3:16" regardless — while confirmDetection
  // swallowed the failure and removed the suggestion card. The operator pressed A, the
  // card vanished, the toast said it was live, and the wall was unchanged.
  /** The `A` key, and the TAKE of a claim: the top pending card. */
  const acceptTop = () => accept(dets[0]);

  async function accept(d) {
    if (!d) return;
    // A reference that parsed but resolves to no verse cannot be fired, and the
    // backend already knows — it marks the suggestion `in_library: false`. Without
    // this the `A` key still reached `confirm_detection` and failed after the
    // press, so the keyboard bypassed the very warning the card renders.
    if (!inLibrary(d)) {
      flash($t('live.not_in_bible', { reference: d.reference }));
      return;
    }
    try {
      await confirmDetection(d.reference);
      flash($t('live.now_live', { reference: d.reference }));
    } catch (e) {
      flash(humanError(e));
    }
  }
  /** Push a cross-reference. Same contract as acceptTop: say nothing unless it worked. */
  async function pushRef(reference) {
    try {
      await confirmDetection(reference);
      flash($t('live.now_live', { reference }));
    } catch (e) {
      flash(humanError(e));
    }
  }

  /** Arming/disarming the AI must not silently fail — the dot would lie about it. */
  async function toggleDetection() {
    try {
      await setDetection(!$capture.detectionOn);
    } catch (e) {
      flash(humanError(e));
    }
  }

  // The operator's single sensitivity dial, on the run surface (Decision §26).
  // It writes the SAME thresholds the Settings sliders do (one baseline) — the
  // whole point is dialling out false fires mid-service without leaving Live.
  let sensitivity = 50;
  /** Has a real backend answer landed? See the note at the read in `onMount`. */
  let sensKnown = false;
  async function onSensitivity(v) {
    // Optimistic, then CORRECTED — never assumed. The slider used to be written
    // from the request and the result thrown away, so a refused change left the
    // dial showing a position the gate had never reached.
    sensitivity = v;
    sensKnown = true;
    try {
      const landed = await setSensitivity(v);
      // The backend owns the curve and its inverse; trust its number, not ours.
      if (Number.isFinite(landed)) sensitivity = landed;
    } catch (e) {
      // Put the dial back where the GATE actually is, read from the backend rather
      // than remembered here, and say so. A slider that silently disagrees with the
      // thing it controls is the whole finding.
      sensitivity = await getSensitivity();
      sensKnown = true;
      flash(`Sensitivity stayed at ${sensitivity} — ${humanError(e)}`);
    }
  }

  function dismissTop() {
    if (!dets[0]) return;
    dismissDetection(dets[0].reference);
  }

  // ── related scripture ────────────────────────────────────────────────────
  //
  // Topical cross-references for what is being preached. NOT a detection — nobody said
  // these references out loud. It is a keyword match against 19 themes, which is the
  // weakest evidence anywhere in this product, and it is offered on that basis: the
  // operator may find it useful, and it never touches a screen unless they choose it.
  //
  // Pull-based and debounced. The transcript updates several times a second and each
  // call does a DB lookup per reference; polling it on every keystroke of speech would
  // be a database query storm for a feature nobody asked for.
  let related = null;
  let relatedT;
  let lastRelatedFor = '';

  // The window we ask about: the tail of the sermon, not the whole thing. A theme is
  // about what is being said NOW, and an hour of transcript matches everything.
  $: relatedWindow = $transcript.finals.slice(-3).join(' ').slice(-400);

  // Depend ONLY on relatedWindow (new speech). Reading $capture.detectionOn or
  // $live here made the block re-run on every audio://quality / language update
  // too — each one cleared and re-armed the 1500ms timer, and those arrive faster
  // than 1.5s while listening, so the timeout never elapsed and Related Scripture
  // never populated. detectionOn / live are read imperatively at fire time instead,
  // so they gate the result without resetting the debounce.
  $: armRelated(relatedWindow);
  function armRelated(w) {
    clearTimeout(relatedT);
    if (w.length > 40) {
      relatedT = setTimeout(async () => {
        if (!get(capture).detectionOn) {
          related = null;
          return;
        }
        const ex = get(live)?.reference ?? null;
        const key = w + '|' + (ex ?? '');
        if (key === lastRelatedFor) return; // nothing new was said
        lastRelatedFor = key;
        related = await relatedScripture(w, ex);
      }, 1500);
    } else {
      related = null;
    }
  }

  /** Related refs are an OFFER. Putting one on screen is the operator's decision, and
   *  it goes through the same manual path — recorded as a human's fire, never the AI's
   *  (the self-calibrating router learns from that column). */
  async function pushRelated(reference) {
    try {
      await manualFire(reference);
      flash($t('live.now_live', { reference }));
    } catch (e) {
      flash(humanError(e));
    }
  }

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
  /**
   * A plain sentence for a live operator.
   *
   * Delegates to lib/errors.js — the ONE humaniser — and only adds the thing that
   * view knows and it doesn't: what the operator actually typed. It used to be
   * `String(e)`, which now that the backend sends a typed `{kind, message}` would
   * render literally as "[object Object]".
   */
  function humanError(e) {
    const s = humanErrorBase(e);
    if (/could not parse|parse a reference|isn't in the Bible/i.test(s) && manualRef.trim())
      return `Couldn't read "${manualRef.trim()}" as a scripture reference.`;
    return s;
  }
  async function fireManual() {
    const ref = manualRef.trim();
    if (!ref) return;
    try {
      await manualFire(ref);
      flash($t('live.now_live', { reference: ref }));
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
    } catch (e) {
      // Capture start is non-blocking (DECISIONS/rule 5): DEVICE errors arrive
      // asynchronously on audio://error. So a rejection HERE is a command-level
      // failure that event never carries — surface it rather than swallow, or the
      // Listen button just silently does nothing.
      flash(humanError(e));
    }
    listenBusy = false;
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
        errMsg = 'No screen yet — add one in the Outputs tab.';
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

  // ── recognition language is not settling ─────────────────────────────────
  // Same shape as the mic warnings above, and the same reasoning: name the
  // problem and the physical thing to go and do. This one is worth saying
  // because it is INVISIBLE — a wandering language label degrades the transcript
  // and reads to the operator as "the AI is bad", while the fix is one dropdown.
  $: langWarning = (() => {
    const langs = $capture.langUnstable;
    if (!langs?.length || $capture.stt?.language) return null; // already pinned
    return {
      title: 'Relay keeps changing its mind about the language.',
      fix: `It has heard ${langs.join(', ')} in the last few minutes. Pick the language in Settings → Scripture & Languages → Recognition language — auto-detect struggles with a strong accent, and a wrong guess garbles the transcript.`,
    };
  })();

  $: selCue = items.find((i) => i.id === selId) || null;
  $: selSlides = slidesOf(selCue);
  $: liveIndex = items.findIndex((i) => i.id === liveCueId);
  $: selNote = selCue ? payloadOf(selCue).stage_note || '' : '';

  // ── PREVIEW / PROGRAM ────────────────────────────────────────────────────
  //
  // The design reference draws a broadcast preview/program pair with a transition
  // rack (Cut / Fade / Wipe / Stinger) between them. Relay has no transition
  // engine and no preview bus, and inventing one is a feature, not a rebrand — so
  // the PAIR is reproduced from facts Relay already has, and the transition rack
  // is not drawn (see the loop log).
  //
  //   PROGRAM = `$liveContent` — literally what is on the congregation's screen,
  //             through the same TemplateRender the output window uses.
  //   PREVIEW = what the next TAKE would put there: the AI's top pending claim if
  //             there is one, otherwise the slide `→` would fire.
  //
  // TAKE is therefore not a new command. It is exactly acceptTop() or fireSlide(),
  // whichever the preview is showing — the same two paths the keyboard already uses.
  $: previewNext = openPlan ? stepFrom(items, liveCueId, liveSlide, 1) : null;
  $: previewCue = previewNext ?? (selCue ? { item: selCue, slide: 0 } : null);
  $: previewSlide = previewCue ? slidesOf(previewCue.item)[previewCue.slide] : null;
  // A cell the operator DOUBLE-clicked in the grid outranks both, and is the one
  // case where an unaccepted AI claim loses the preview pane. That is deliberate:
  // the operator asked for this slide by hand, and a preview that swapped under
  // them would make TAKE fire something they never chose. The claim is not lost —
  // it is still in the detection panel, where accepting it is one press. The
  // override is transient: taking it clears it, and the preview goes back to the
  // ordinary order.
  $: previewContent = gridPreview
    ? { reference: gridPreview.label, text: gridPreview.text || gridPreview.label, translation: null }
    : dets[0]
      ? { reference: dets[0].reference, text: dets[0].text ?? '', translation: null }
      : previewSlide
        ? { reference: previewCue.item.label, text: previewSlide.text || previewSlide.label, translation: null }
        : null;
  $: previewLabel = gridPreview
    ? gridPreview.label
    : dets[0]
      ? dets[0].reference
      : previewCue
        ? `${previewCue.item.label} · ${previewSlide?.label ?? ''}`.trim()
        : '';
  /** The take. Never a new code path — the same accept/fire the keys already run. */
  async function take() {
    if (gridPreview) return fireCell(gridPreview);
    if (dets[0]) return acceptTop();
    if (previewCue) return fireSlide(previewCue.item, previewCue.slide);
  }

  // ── THE SLIDE GRID ───────────────────────────────────────────────────
  //
  // What is staged, as pickable cells (docs/REBRAND.md §2). The arbitration and
  // the cell-building are in `slidegrid.js`, tested there; this half is the
  // wiring — which fire path a cell takes, and what is loaded behind it.
  let gridVerses = [];
  let gridChapter = null; // the chapter `gridVerses` holds, e.g. "Psalms 23"
  let gridPreview = null; // the cell a double click staged, or null

  // What the operator picked in the rail. A hand pick outranks the plan (see
  // `gridSource`) because it is the more recent deliberate act; a DETECTION never
  // does, which is the distinction the flag exists to keep.
  let railChapter = null; // { book, chapter } chosen in the rail, or null
  /** Stage a chapter from the rail. It does NOT fire — the grid does that. */
  function stageChapter(book, chapter) {
    railChapter = { book, chapter };
  }

  /**
   * ONE SEARCH HIT, the whole job in one press (docs/REBRAND.md §9): the verse
   * goes to the programme, its chapter loads into the grid, and that verse is
   * the active slide.
   *
   * NEVER a new fire path — it is the same `manualFire` a verse cell in the grid
   * takes, so the fire is recorded `'manual'` (rule 14), passes the pre-air
   * validator (rule 36) and reports its own outcome (rule 15). The third clause
   * of §9 needs no code: `cellLive` marks a verse cell from `$liveContent`, so
   * the verse that just went out IS the active cell once the chapter is staged.
   *
   * The chapter is staged FIRST and unconditionally. A fire that fails must still
   * leave the operator looking at the passage they asked for — that is the
   * surface they will use to try again by hand.
   */
  async function fireSearchHit(book, chapter, verse, reference) {
    stageChapter(book, chapter);
    const ref = reference || `${book} ${chapter}:${verse}`;
    try {
      await manualFire(ref);
      flash($t('live.now_live', { reference: ref }));
    } catch (e) {
      flash(humanError(e));
    }
  }

  // The chapter around the live verse — ONLY when no plan is open, and only when
  // the operator has not asked for a different one. A plan is what the operator
  // deliberately staged, and must not be pushed out of the grid by whatever the
  // preacher happened to say next.
  $: stagedRef = railChapter
    ? `${railChapter.book} ${railChapter.chapter}`
    : openPlan
      ? null
      : ($liveContent?.reference ?? null);
  $: loadChapterFor(stagedRef);

  async function loadChapterFor(ref) {
    const p = ref ? parsePassage(ref) : null;
    if (!p) {
      gridChapter = null;
      gridVerses = [];
      return;
    }
    const title = `${p.book} ${p.chapter}`;
    // Every fire re-emits the same chapter. Refetching it each time would put a
    // DB read on the fire path for no new information.
    if (title === gridChapter) return;
    gridChapter = title;
    const asked = title;
    // `chapterVerses` is a GUARDED read — it reports through `$readErrors` rather
    // than throwing, and answers [] when it could not load. An empty grid that
    // says nothing is staged is the honest reading of that.
    const rows = await chapterVerses(p.book, p.chapter);
    // A slow load must not stage the previous chapter's verses under this title.
    if (gridChapter === asked) gridVerses = rows ?? [];
  }

  $: grid = gridSource({
    planOpen: !!openPlan,
    planTitle: openPlan?.title ?? '',
    items,
    slidesOf,
    verses: gridVerses,
    passageTitle: gridChapter ?? '',
    handPicked: !!railChapter,
  });

  /**
   * Fire one grid cell.
   *
   * NEVER a new fire path: a plan cell is `fireSlide` (which marks the cue amber
   * only after the fire resolves), and a verse cell is the same `manualFire` the
   * search box uses. Both report their own failure.
   */
  async function fireCell(cell) {
    gridPreview = null;
    if (cell.kind === 'plan') {
      const item = items.find((i) => i.id === cell.cueId);
      // The plan was reloaded under the grid. Say so rather than firing a guess.
      if (!item) {
        flash('That cue is no longer in the plan.');
        return;
      }
      return fireSlide(item, cell.slideIdx);
    }
    if (!cell.reference) return;
    try {
      await manualFire(cell.reference);
      flash($t('live.now_live', { reference: cell.reference }));
    } catch (e) {
      flash(humanError(e));
    }
  }

  // Single click sends to programme, double click previews. The 190ms beat and
  // the guarantee that a double never fires both live in `slidegrid.js`.
  const gridPress = pressArbiter({
    send: fireCell,
    preview: (cell) => {
      gridPreview = cell;
    },
    onError: (e) => flash(humanError(e)),
  });

  // ── A CELL IS THE WALL IN MINIATURE ──────────────────────────────────────
  //
  // The grid drew a grey box with the slide's LABEL in it, so a cell said
  // "Romans 8:28-31" and nothing about what a congregation would see. The
  // prototype renders every cell, and it is right: the operator picking under
  // pressure is matching a shape, not reading a list.
  //
  // Through `TemplateRender` — THE one renderer — so a thumbnail cannot disagree
  // with the wall about a template that has stopped working. No second fit path,
  // no `if kind == …`; the same component, in a 16:9 `container-type` box, and
  // cqw does the rest.
  //
  // TWO THINGS THIS DELIBERATELY DOES NOT DO.
  //
  //  · It does not pass `onFit`. Rule 37's "this is rendering at 38% of its
  //    designed size" warning is the WALL's measurement and there must be one of
  //    it. Twenty thumbnails reporting their own fit would drown the one that
  //    matters, and `noteFit` has exactly one caller: the Program pane.
  //  · It never serialises or broadcasts a template. The object is handed to a
  //    local component and goes nowhere near IPC — which is the rule that exists
  //    because a content-look default carrying an embedded image was 13 MB and
  //    made every fire take seconds (CLAUDE.md, Testing).
  //
  /** The template this cell would actually be painted through. */
  $: cellTemplate = (c) => {
    if (c.kind !== 'plan') return previewTpl;
    const item = items.find((i) => i.id === c.cueId);
    // A cue's own template choice is PINNED — the operator picked that look for
    // that item, and §29 says a pinned choice overrides the screen's.
    const pinned = item?.template_id
      ? ($templates.find((t) => t.id === item.template_id) ?? null)
      : null;
    return resolveOutputTemplate(previewTpl, pinned, !!pinned);
  };

  /**
   * What the cell paints — the same shape `fireSlide` actually sends.
   *
   * A LYRIC CARRIES NO TITLE. `fire_content` suppresses it for songs (rule 36 —
   * one place decides), so a song thumbnail that printed the song's name would
   * be showing the operator something no congregation will ever see. That is the
   * whole value of a rendered thumbnail and the one way to throw it away.
   */
  $: cellContent = (c) => ({
    reference: c.ctype === 'song' ? null : c.label,
    text: c.text || '',
    translation: null,
  });

  /** Is this cell what is on the congregation's screen right now? */
  $: cellLive = (c) =>
    c.kind === 'plan'
      ? planOnAir && c.cueId === liveCueId && c.slideIdx === liveSlide
      : !!c.reference && !$screenBlack && $liveContent?.reference === c.reference;

  // How many times the previewed verse has ALREADY gone out this service.
  //
  // Recomputed only when the previewed reference changes — not on every store
  // tick — because this is a per-verse DB read on the run surface. 0 also means
  // "no service is being recorded", which correctly shows nothing.
  //
  // `verseRepeatCount` swallows by contract: a badge that fails to load costs the
  // operator nothing they cannot see for themselves.
  let previewRepeats = 0;
  let repeatsFor = null;
  $: if (previewLabel !== repeatsFor) {
    repeatsFor = previewLabel;
    previewRepeats = 0;
    if (previewLabel) {
      const asked = previewLabel;
      verseRepeatCount(asked).then((n) => {
        // A slow lookup must not label the NEXT verse with the last one's count.
        if (repeatsFor === asked) previewRepeats = n;
      });
    }
  }

  // The preview and program panes must render through the SAME template the real
  // output window uses, or the operator sees one thing here and the congregation
  // sees another. The wall's look is the MAIN output channel's assigned template
  // (Output.svelte: `contentOverride ?? channelTemplate`), NOT the first
  // console-active template — those diverge, which read as the program pane being
  // "frozen on something different" from the wall.
  //
  // Resolved from the reactive `$templates` store (not a one-shot snapshot), so a
  // template edit — which updates `$templates` app-wide via saveTemplate →
  // loadTemplates — flows straight into these panes instead of leaving them stale.
  $: mainChannel = channels.find((c) => c.render_target === 'native_window') ?? channels[0] ?? null;
  $: mainTpl =
    (mainChannel && $templates.find((t) => t.id === mainChannel.template_id)) ||
    $templates.find((t) => t.id === $defaultTemplateId) ||
    $templates[0] ||
    null;
  $: previewTpl = mainTpl;

  // ── output status ────────────────────────────────────────────────────────
  // The configured render targets, read-only here. Editing them is the Channels
  // tab's job; this panel answers "is it up?" during a service and nothing else.
  let channels = [];

  // ── §4 presentation modes ────────────────────────────────────────────────
  // COMPACT is a density change, not a different screen: the same panels, the
  // same controls, tighter. It exists because the reference console assumes a
  // wide desk monitor and a great many church booths are a 13" laptop, where the
  // bottom row scrolls out of sight — and the bottom row is where the transport
  // and the panic controls live.
  //
  // Nothing is REMOVED in compact. A run surface that hides a control at small
  // sizes hides it at exactly the moment the operator is most cramped and most
  // rushed; this only tightens spacing and type.
  $: compact = $session.liveDensity === 'compact';
  $: fullscreen = !!$session.liveFullscreen;
  const setDensity = (d) => setSession({ liveDensity: d });
  const setFullscreen = (v) => setSession({ liveFullscreen: v });

  // §5 INSPECTOR. The claim panel has room for the verdict; the reasoning needs
  // a surface of its own. Opened per-detection, never a tab: an operator does not
  // browse detections, they interrogate the one in front of them.
  let inspecting = null;
  $: inspectAlts = inspecting
    ? dets.filter((d) => d.reference !== inspecting.reference).slice(0, 4)
    : [];
  /** Interrogate ONE claim — the card that was pressed, not `dets[0]`. With a
   *  column of claims, "why this match?" on the third card must open the third
   *  card's reasoning. */
  function inspect(d) {
    inspecting = d ?? null;
  }
  async function inspectAccept() {
    inspecting = null;
    await acceptTop();
  }
  async function inspectDismiss() {
    inspecting = null;
    await dismissTop();
  }

</script>


<!-- LIVE — laid out to docs/design/relay-console-screen.png.
     Row A: PREVIEW · take rack · PROGRAM · OUTPUT STATUS
     Row B: 1 Live Transcript · 2 AI Detection · 3 Service Plan · 4 Quick Controls
     Everything below is a re-dressing of the controls that were already here — no
     command was added, removed or rewired. Where the reference draws a control
     Relay has no backend for (Fit/Safe-Area, Hold Outputs, Override Mode, ±5s
     audio scrub, a monitor bus), it is NOT drawn: a dead button in a live console
     is the exact failure this codebase keeps fixing. (The transition rack has
     since come off that list for a different reason — see the rack itself.) -->
<div class="con" class:compact class:fullscreen>
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

  <!-- THE ENGINE IS NOT ATTACHED.
       Eighteen controls on this tab — Clear screens, Blackout, Fire, Rehearse, the
       microphone — are `disabled={!$capture.available}`. Channels, ServicePlanner
       and History each say why in that state; the RUN surface, the one an operator
       is looking at when something goes wrong, said nothing at all and simply
       appeared broken.
       Rose, because it is a fault the operator must act on, and it names the one
       thing that is still true: firing by hand needs the engine too, so there is no
       reassuring half-truth to offer here. -->
  {#if !$capture.available}
    <div class="con-noengine" role="alert">
      <span class="r-badge rose"><span class="bd"></span>Backend not attached</span>
      <span>
        Relay's engine is not running, so nothing on this tab can reach a screen —
        including the manual controls. Reopen Relay from the app, not from a browser.
      </span>
    </div>
  {/if}

  <!-- THE DESK — a 206px browsing rail, then the stage. The rail is the
       prototype's, and the reason it exists is the whole product: the preacher
       goes off-script, and until now the only way to reach an unplanned verse was
       to type it blind into the manual box. BROWSING a book or a chapter only
       stages into the grid; a SEARCH HIT names one verse, and one click takes it
       the whole way (docs/REBRAND.md §9) through the same `manualFire` the grid
       uses. The rail's own header note carries the rest. -->
  <div class="desk">
    <div class="rail-col">
      <LiveRail
        disabled={!$capture.available}
        onChapter={stageChapter}
        onVerse={fireSearchHit} />
      <!-- View controls. Deliberately at the TOP-RIGHT and deliberately small: they
           change how the console looks, never what reaches a screen, and must not
           compete with the transport for an operator's attention. -->
      <div class="view-ctl">
        <div class="seg" role="group" aria-label="Console density">
          <button class:on={!compact} on:click={() => setDensity('normal')}>Normal</button>
          <button class:on={compact} on:click={() => setDensity('compact')}>Compact</button>
        </div>
        <button class="view-fs" on:click={() => setFullscreen(!fullscreen)}>
          {fullscreen ? 'Show tabs' : 'Full screen'}
        </button>
      </div>
    </div>

    <div class="stage">
  <!-- ══════ ROW A — the pair, the rack, and the outputs ══════ -->
  <div class="con-top">
    <!-- PREVIEW — what the next TAKE would put on the wall. Amethyst, because it
         is by definition NOT on air; amber is reserved for the pane on the right. -->
    <section class="pane mon prev">
      <header class="mon-bar">
        <!-- STEEL BLUE, not amethyst. Amethyst means REHEARSAL and nothing else
             (rule 18) — a Preview chip wearing it says "nothing is reaching the
             congregation" on a console where that may or may not be true, and on
             the one day both are true the operator reads the wrong one. Steel
             blue is the thing you are working on, which is exactly what a
             preview is, and it is what the grid's own cued cell already uses. -->
        <span class="tag preview">Preview</span>
        <span class="spring"></span>
        <!-- "Shown earlier" belongs HERE, on the thing about to go out, not on the
             thing already on air — by then the repeat has happened. A preacher
             circling back to a verse is normal, so this states the fact and stops:
             it is not a warning and must not read like one. -->
        {#if previewRepeats > 0}
          <span class="mon-repeat r-mono" title="Already shown in this service">
            shown {previewRepeats > 1 ? `${previewRepeats}×` : 'earlier'}
          </span>
        {/if}
        <!-- Only when there IS something. The pane below already says "Nothing
             cued" in the middle of the empty screen, and the same three words in
             two places at once reads as two facts. -->
        {#if previewLabel}<span class="mon-name">{previewLabel}</span>{/if}
      </header>
      <div class="screen">
        {#if previewTpl && previewContent}
          <TemplateRender template={previewTpl} content={previewContent} />
        {:else}
          <div class="screen-empty">
            {previewTpl ? 'Nothing cued' : 'No active template — activate one in Templates'}
          </div>
        {/if}
      </div>
    </section>


    <!-- THE RACK. The reference draws a transition list here (Cut / Fade / Wipe /
         Stinger / Duration). Relay HAS a transition engine now — seven modes in
         `transitions.js`, played by the renderer (docs/REBRAND.md §8, DECISIONS
         §71) — so the old reason for leaving them out ("no engine") has expired.
         They are still not drawn, for a different and better reason: a transition
         is a property of the TEMPLATE, resolved through the style model, so a
         desk-level picker here would either change nothing on the wall or silently
         edit a template from the run surface. It belongs where the look is chosen.
         What is here is the real take path: the same accept/fire and the same nav
         the keys already run, plus the transport MODE, which is the one thing about
         `→` an operator must never have to guess (CLAUDE.md — same key, two
         meanings, is how the wrong thing reaches a congregation). -->
    <aside class="rack">
      <span class="rack-lbl">Take</span>
      <button
        class="take"
        on:click={take}
        disabled={!previewContent || !$capture.available}
        title="Put the previewed content on the outputs">TAKE</button>
      <!-- FULL WIDTH AND NAMED. Two 30px arrow glyphs side by side was the
           smallest pair of targets on the surface an operator uses fastest, and
           `‹` and `›` name nothing: they are the same two shapes whichever of
           the two things the transport is about to do. -->
      <button class="rk wide" title="Previous (←)" on:click={() => step(-1)}>‹ Prev</button>
      <button class="rk wide" title="Next (→)" on:click={() => step(1)}>Next ›</button>
      <!-- WHAT THOSE TWO WALK. Required by CLAUDE.md — the transport is
           MODE-AWARE and says so; the same key silently meaning two things is
           how the wrong thing reaches a congregation. The prototype's caption is
           "walks the programme" and Relay's has to say WHICH walk. -->
      <span
        class="rack-cap"
        title={mode === 'slide'
          ? 'Arrow keys step through the service plan'
          : 'Arrow keys walk through the passage on screen'}>
        walks the programme
        <b class="rack-mode r-mono" class:slide={mode === 'slide'}>{mode === 'slide' ? 'SLIDE' : 'VERSE'}</b>
      </span>
    </aside>

    <!-- PROGRAM — literally what the congregation is looking at, rendered through
         the SAME TemplateRender as the real output window, so the pane cannot
         disagree with the wall. -->
    <section class="pane mon prog"
      class:onair={$live && !$rehearsing && !$screenBlack}
      class:inreh={$live && $rehearsing}>
      <header class="mon-bar">
        <!-- AS WHICH SCREEN. The pane renders through the MAIN channel's
             template, so it is a preview OF ONE SCREEN and never said which —
             and a console that shows one look while a lobby TV shows another is
             exactly the disagreement an operator has no way to spot. The
             prototype prints it and it is right to. `mainChannel` is the same
             one the template resolution above already picked, so the sentence
             and the render cannot come apart. -->
        {#if $rehearsing}
          <span class="tag reh">Rehearsal</span>
        {:else if $screenBlack}
          <span class="tag off">Blackout</span>
        {:else if $live}
          <span class="tag onair">Program · On Air</span>
        {:else}
          <span class="tag off">Program · Clear</span>
        {/if}
        {#if mainChannel}
          <span class="mon-as r-mono" title="This pane renders through {mainChannel.name}'s template">as {mainChannel.name}</span>
        {/if}
        <span class="spring"></span>
        <!-- The REFERENCE, amber only when a congregation is genuinely looking
             at it. Amber is ON AIR and is never allowed to lie. -->
        <span class="mon-name" class:live={$live && !$rehearsing && !$screenBlack}>
          {$live ? ($live.reference || 'content') : '—'}
        </span>
      </header>
      <div class="screen">
        {#if $live}
          <TemplateRender
            template={resolveOutputTemplate(previewTpl, $liveTemplateOverride, $liveTemplatePinned)}
            content={$liveContent}
            onFit={noteFit}
          />
        {:else}
          <!-- Nothing is on the wall. Say so in words — a blank rectangle and a
               black-out look identical, and they are not the same fact. -->
          <div class="screen-empty">Screens clear</div>
        {/if}
        {#if $screenBlack}<div class="blk"></div>{/if}
      </div>
    </section>

  </div>

  <!-- ══════ THE SLIDE GRID — full width, directly under the monitors ══════
       It was one fifth-width card in a row of five, which made every cell too
       small to read and forced a click just to identify a slide. It is the thing
       an operator picks from most often, so it gets the width. -->
  <div class="con-grid">
    <!-- ── THE SLIDES ── -->
    <!-- Single click sends to programme, double click previews (docs/REBRAND.md §2).
         BOTH handlers go through ONE arbiter, so a double can never do both — the
         alternative is that every preview puts the slide on the wall on its way
         past. The 190ms beat and that guarantee are tested in `slidegrid.test.js`.
         Nothing here claims a screen: `fireCell` reuses the existing fire paths,
         which mark amber only after the fire resolves (rules 15 and 18). -->
    <section class="pane">
      <header class="pane-head">
        <h2>Slides</h2>
        <span class="spring"></span>
        <span class="r-mono cnt">{grid.cells.length}</span>
      </header>

      <div class="pane-body sg-body">
        {#if grid.cells.length}
          <div class="sgrid">
            {#each grid.cells as c (c.key)}
              <button
                class="sg-cell"
                class:islive={cellLive(c)}
                class:cued={gridPreview?.key === c.key}
                class:isempty={c.empty}
                on:click={() => gridPress.press(c)}
                on:dblclick={() => gridPress.double(c)}
                disabled={!$capture.available || c.empty}
                title={c.empty
                  ? `${c.label} has nothing to show — open it in the Planner or the Library and give it some words.`
                  : `Click to send ${c.label} to the programme · double click to preview it`}>
                <span class="sg-thumb">
                  <!-- THE SLIDE, not a description of it. The same renderer the
                       wall uses, in a 16:9 container — see `cellTemplate`. -->
                  {#if c.empty}
                    <span class="sg-void">Nothing to show</span>
                  {:else}
                    <TemplateRender template={cellTemplate(c) ?? {}} content={cellContent(c)} />
                  {/if}
                  <!-- The KIND, top-left, as the prototype draws it: a cell is
                       recognised by its shape and confirmed by its tag. -->
                  {#if c.tag}<span class="sg-tag r-mono">{c.tag}</span>{/if}
                  <!-- The word, not only the colour — amber alone is not a label. -->
                  {#if cellLive(c)}<span class="sg-air">On Air</span>
                  {:else if gridPreview?.key === c.key}<span class="sg-prev">Preview</span>{/if}
                </span>
                <span class="sg-meta">
                  <span class="r-mono sg-n">{String(c.n).padStart(2, '0')}</span>
                  <span class="sg-ttl">{c.label}</span>
                </span>
              </button>
            {/each}
          </div>
        {:else if grid.source === 'plan'}
          <EmptyState message="This plan has no slides yet — add cues to it in the Planner." />
        {:else}
          <EmptyState message="Nothing staged. Load a plan, or put a verse on screen and its chapter appears here." />
        {/if}
      </div>

      <footer class="pane-foot sg-foot">
        <span class="sg-cap">{grid.title || '—'}</span>
        <span class="spring"></span>
        <span class="sg-cap">single click → programme · double click → preview</span>
      </footer>
    </section>

  </div>

  <!-- ══════ ROW B — the claim, and the plan ══════ -->
  <div class="con-bot">
    <!-- ── SERVICE PLAN — RUNNING ── -->
    <section class="pane">
      <header class="pane-head">
        <h2>{openPlan ? 'Service Plan — Running' : 'Service Plan'}</h2>
        <span class="spring"></span>
        {#if openPlan}
          <span class="r-mono cnt">{liveIndex >= 0 ? `${liveIndex + 1}/${items.length}` : `${items.length}`}</span>
          <button class="mini ghost" on:click={leave}>Close</button>
        {:else}
          <span class="r-mono cnt">{plans.length}</span>
        {/if}
      </header>

      <div class="pane-body plan">
        {#if openPlan}
          {#each items as c, i (c.id)}
            {@const ty = typeOf(c.cue_type)}
            <div class="rail">
              <span class="rail-dot" class:on={planOnAir && c.id === liveCueId} class:cued={!planOnAir && c.id === liveCueId}></span>
              <button
                class="cue"
                class:sel={c.id === selId}
                class:islive={planOnAir && c.id === liveCueId}
                class:cued={!planOnAir && c.id === liveCueId}
                on:click={() => (selId = c.id)}>
                <span class="cue-stripe" style="background:{ty.color}"></span>
                <span class="cue-num r-mono">{String(i + 1).padStart(2, '0')}</span>
                <span class="cue-body">
                  <span class="cue-title" title={c.label}>{c.label}</span>
                  <span class="cue-meta r-mono">{cueSub(c)}</span>
                </span>
                {#if c.id === liveCueId}
                  <!-- Amber = the congregation is looking at it. CUED = where `→`
                       resumes from, and NOT on screen — grey, never amber. -->
                  <span class="r-badge sm-badge" class:amber={planOnAir} class:grey={!planOnAir}>
                    <span class="bd"></span>{planOnAir ? 'On Air' : 'Cued'}
                  </span>
                {/if}
              </button>
            </div>

            <!-- The live cue opens into its slides, in place. That is the reference's
                 "Now Playing" block, and it is where the take actually happens. -->
            {#if c.id === selId}
              {#if payloadOf(c).stage_note}
                <!-- The preacher's stage note. Confidence monitor only — never on the
                     main output. -->
                <div class="note">
                  <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
                  {payloadOf(c).stage_note}
                </div>
              {/if}
              {#each slidesOf(c) as s, si}
                <button
                  class="slide"
                  class:islive={planOnAir && c.id === liveCueId && si === liveSlide}
                  class:cued={!planOnAir && c.id === liveCueId && si === liveSlide}
                  style="--acc:{slideAccent(s.tag)}"
                  on:click={() => fireSlide(c, si)}>
                  <span class="slide-stripe"></span>
                  <span class="slide-tag r-mono">{s.tag}</span>
                  <span class="slide-text">{s.text || s.label}</span>
                </button>
              {/each}
            {/if}
          {/each}
          {#if !itemsLoaded}
            <Loading what="cues" compact />
          {:else if !items.length && $readErrors.planItems}
            <!-- RG-95, last two surfaces. `planItems` swallowed to `[]`, so a read
                 that failed rendered "this plan has no cues yet" — on the RUN
                 surface, mid-service, about a plan the operator built. -->
            <ErrorState compact error={$readErrors.planItems} onRetry={() => loadPlan(openPlan)} />
          {:else if !items.length}
            <EmptyState message={$t('live.plan_no_cues')} />
          {/if}
        {:else}
          <!-- No plan loaded. Not an error — plenty of services run entirely on the
               AI and the manual box. Offer the plans, don't demand one. -->
          {#if plansLoaded && plans.length}
            <div class="pick-intro r-lbl">Service plans — pick one to run</div>
          {/if}
          {#each plans as p (p.id)}
            <button class="cue pick" on:click={() => loadPlan(p)}>
              <span class="pick-ic" aria-hidden="true">
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4"/></svg>
              </span>
              <span class="cue-body">
                <span class="cue-title" title={p.title}>{p.title}</span>
                <span class="cue-meta r-mono">{p.plan_date} · {p.cue_count} {p.cue_count === 1 ? 'cue' : 'cues'}</span>
              </span>
              <span class="pick-run"><svg viewBox="0 0 24 24" width="10" height="10" fill="currentColor" aria-hidden="true"><path d="M5 3v18l15-9L5 3Z"/></svg>Run</span>
            </button>
          {/each}
          <!-- Loading is NOT Empty. Until the query comes back, "no plans" is not a
               fact — it is the absence of one, and rendering it told an operator with
               a full library that they had lost their work. -->
          {#if !plansLoaded}
            <Loading what="plans" compact />
          {:else if !plans.length && $readErrors.listPlans}
            <ErrorState compact error={$readErrors.listPlans} onRetry={loadPlans} />
          {:else if !plans.length}
            <EmptyState message={$t('live.no_plans')} />
          {/if}
        {/if}
      </div>

      <footer class="pane-foot">
        <!-- role="status" so every flash is announced. These sentences ARE the
             feedback for the transport keys. -->
        <span class="sr-only" role="status" aria-live="polite" aria-atomic="true">{liveMsg}</span>
        {#if liveMsg}
          <span class="flash" aria-hidden="true"><i class="fd"></i>{liveMsg}</span>
        {:else}
          <span class="flash idle" aria-hidden="true">{openPlan ? openPlan.title : 'No plan loaded'}</span>
        {/if}
      </footer>
    </section>

  </div>

    </div>

    <!-- ══════ THE INSPECTOR — the AI's claims, then the screens ══════
         docs/REBRAND.md §2 gives the workspace a right-hand column and puts the
         detection panel in it. The reason is not layout: ONE CLAIM AT A TIME WAS
         NEVER THE TRUTH. A decode window can name several references, and an
         operator choosing between them needs to see them together — which is
         rule 29 read from the operator's side ("one window may inform the
         operator about several verses; it may put at most ONE on a wall"). The
         old panel showed `dets[0]` as a headline and demoted the rest to a strip
         of one-line rows with a `Fire` button, so the second candidate was
         offered at a glance and decided at a squint.

         ORDER: PENDING FIRST, then the receipts. The prototype is strictly
         newest-first; Relay is not, deliberately. A receipt is a record of
         something that already happened and a pending claim is a decision the
         operator still owes, and a record must never push a decision out of a
         bounded column. -->
    <div class="insp-col">
      <section class="pane">
        <header class="pane-head">
          <h2>AI Detection</h2>
          <span class="spring"></span>
          <!-- WHAT THE GATE IS ACTUALLY DOING, and it reads differently when the
               thing behind it is broken (rule 35). "Auto-fire on" over a dead
               model, a stopped microphone or a disarmed detector is the same
               reassuring sentence over four different situations, which is the
               defect that rule exists to name. Relay has no "suggest only" mode
               — a Direct hit above the bar fires unattended whenever detection
               is armed — so it is not offered; a mode nothing implements is a
               status line that lies. -->
          <span class="det-meta r-mono" class:on={gateState.armed}>{gateState.label}</span>
        </header>

        <div class="det-ctl">
          <label class="sens" title="How readily the AI fires. Lower = fewer, surer catches; higher = more, noisier. Same dial as Settings.">
            <span class="sens-lbl r-mono">SENS</span>
            <input type="range" min="0" max="100" step="1" value={sensitivity}
              on:input={(e) => onSensitivity(+e.target.value)} disabled={!$capture.available}
              aria-label="Detection sensitivity" use:rangeFill={sensitivity} />
            <span class="sens-val r-mono">{sensKnown ? sensitivity : '—'}</span>
          </label>
          <span class="spring"></span>
          <button class="chip btnchip" class:ok={$capture.detectionOn} on:click={toggleDetection}
            disabled={!$capture.available} title="Arm or disarm automatic detection">
            <i class="bd"></i>{$capture.detectionOn ? 'Armed' : 'Off'}
          </button>
          <button class="ibtn" on:click={toggleListen} title={$capture.capturing ? 'Stop listening' : 'Start listening'}
            aria-label={$capture.capturing ? 'Stop listening' : 'Start listening'}
            disabled={!$capture.available || !$capture.stt.loaded || listenBusy}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v4"/></svg>
          </button>
        </div>

        <!-- The AI has heard something. This is the product's whole reason to exist, and
             it arrived in total silence for a screen-reader operator. "polite", not
             "assertive": a suggestion is an offer, not an emergency. -->
        <span class="sr-only" role="status" aria-live="polite" aria-atomic="true">
          {#if dets.length}
            {heard(dets[0]) ? 'Heard' : 'Possible paraphrase'}: {dets[0].reference}.
            Press A to put it on screen, D to dismiss.
          {/if}
        </span>

        <div class="pane-body det">
          <!-- No STT model = the AI cannot listen. Relay degrades to a fully working
               MANUAL tool, never a dead one — and it can fix itself in one click. -->
          {#if $capture.available && !$capture.stt.loaded}
            <ModelSetup compact />
          {/if}

          {#each claimCards as card, i (card.key)}
            {@const d = card.d}
            <!-- HEARD vs GUESSED. Not two flavours of one thing, and they must not
                 look like it. A direct hit's number is a real parse confidence. A
                 paraphrase is a TF-IDF cosine — a distance, NOT a probability
                 (router.rs forbids it from ever auto-firing at ANY score). So the
                 guess gets cyan, and gets no number at all: a number that lies is
                 worse than no number. Cyan, NOT amethyst, which means rehearsal. -->
            <article class="clm" class:guess={!heard(d)} class:done={!!card.outcome}>
              <div class="clm-top">
                <span class="clm-ref">{d.reference}</span>
                <span class="cbadge" class:p={!heard(d)}>{heard(d) ? 'Heard' : 'Paraphrase'}</span>
              </div>

              {#if heard(d)}
                <!-- A bar, not just a number: "0.92" means nothing to a volunteer. -->
                <div class="conf" role="meter" aria-valuemin="0" aria-valuemax="100"
                  aria-valuenow={Math.round(d.confidence * 100)} aria-label="Detection confidence">
                  <i style="width:{Math.round(d.confidence * 100)}%"></i>
                </div>
              {:else}
                <p class="guess-note">{$t('live.not_a_spoken_reference')}</p>
              {/if}

              {#if d.matched_text}
                <!-- THE EVIDENCE — the words that actually triggered the match. -->
                <p class="mt-q">“{d.matched_text}”</p>
              {/if}

              {#if d.text}<p class="clm-verse">{d.text}</p>{/if}

              <!-- PARSED, BUT THERE IS NO SUCH VERSE. Relay keeps showing it — the
                   suggestion is the operator's evidence that a number was misheard,
                   and dropping it would be silence. -->
              {#if !inLibrary(d)}
                <p class="claim-absent">{$t('live.not_in_bible', { reference: d.reference })}</p>
              {/if}

              {#if card.outcome}
                <!-- WHO ACTED. Each wording names an actor, because "fired" alone
                     is exactly the fact an operator cannot reconstruct afterwards:
                     did the AI do that, or did I? -->
                <p class="clm-done">{card.outcome}</p>
              {:else}
                <div class="cacts">
                  <button class="act go" on:click={() => accept(d)} disabled={!inLibrary(d)}>
                    {#if inLibrary(d)}
                      <b>Accept &amp; fire</b><span>Send to outputs</span>
                    {:else}
                      <b>Nothing to send</b><span>That verse does not exist</span>
                    {/if}
                  </button>
                  <button class="act no" on:click={() => dismissDetection(d.reference)}>
                    <b>Dismiss</b><span>Not this verse</span>
                  </button>
                </div>
                <button class="inspect-link" on:click={() => inspect(d)}>Why this match?</button>
                {#if i === 0}<p class="khint"><kbd>A</kbd> accept · <kbd>D</kbd> dismiss</p>{/if}
              {/if}
            </article>
          {:else}
            <EmptyState
              message={$capture.detectionOn ? $t('live.no_suggestions') : $t('live.detection_off')} />
          {/each}

          <!-- RELATED SCRIPTURE. Deliberately the quietest thing on this panel.
               Nobody SAID these references — it is a keyword match against 19 themes,
               the weakest evidence in the product. So: no tally colour, no confidence,
               and it does nothing until the operator clicks it. -->
          {#if related?.refs?.length}
            <div class="sub"><span class="klbl">{$t('live.related', { theme: related.theme })}</span></div>
            <p class="rel-note">{$t('live.related_note')}</p>
            <div class="rel-chips">
              {#each related.refs as r (r.reference)}
                <button class="rel-chip r-focus" on:click={() => pushRelated(r.reference)}
                  disabled={!$capture.available || !r.text}
                  title={r.text ?? 'Not in your Bible text'}>{r.reference}</button>
              {/each}
            </div>
          {/if}
        </div>

        <!-- Manual fire. NOT in the reference mockup, and kept anyway: it is the one
             path that works when the AI is wrong, the model is missing, or the plan
             has run out — removing it to match a picture would remove the product's
             floor. The rail's search is a different job: it FINDS a verse. This
             fires a reference, ranges included, with no search in between. -->
        <footer class="pane-foot entry">
          <div class="search">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
            <input
              bind:this={searchEl}
              bind:value={manualRef}
              on:keydown={(e) => e.key === 'Enter' && fireManual()}
              placeholder="Search verses — ps 23, John 3:16-18"
              aria-label="Manual scripture reference"
              disabled={!$capture.available} />
          </div>
          <button class="wide amber" on:click={fireManual} disabled={!$capture.available}>Fire</button>
        </footer>
        {#if errMsg}<div class="err" role="alert">{errMsg}</div>{/if}
      </section>

    <!-- OUTPUT STATUS. During a service the only question is "is it up?" — and
         until now the pane could report a screen that was down and offer no way to
         bring it back, which sent the operator to another tab mid-service with a
         congregation waiting. Switching a screen on or off is the REPAIR for the
         state this pane reports, not configuration; changing a screen's display or
         its template is configuration and stays in the Outputs tab. -->
    <section class="pane">
      <header class="pane-head">
        <h2>Output Status</h2>
        <span class="spring"></span>
        <span class="r-mono cnt">{channels.length}</span>
      </header>
      <div class="pane-body outs">
        {#each outs as o (o.c.id)}
          <div class="out" class:down={o.s.kind === 'down'}>
            <span class="out-ic" aria-hidden="true">
              <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="4" width="20" height="13" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
            </span>
            <b class="out-nm" title={o.c.name}>{o.c.name}</b>
            <span class="r-badge {SCREEN_BADGE[o.s.kind]} sm-badge"><span class="bd"></span>{o.s.label}</span>
            <!-- The screen's OWN last word, not ours. When it disagrees with the
                 badge, that disagreement is the finding. With nothing to report
                 yet it names the KIND of screen in words — it used to print the
                 raw database value (`native_window`) at a volunteer mid-service. -->
            <span class="out-note r-mono">{o.s.note || screenKind(o.c.render_target)}</span>
            <!-- ONLY WHEN THERE IS SOMETHING TO PRESS. The inert half of this pair
                 ("Browser source", "No window") was a label that never did anything,
                 sitting where the eye looks for a control and taking ~85px from the
                 screen's name and the badge on a ~230px rail. The Outputs tab states
                 the type in full, in a column made for it. -->
            {#if o.w.action}
              <button
                class="out-sw"
                title={o.w.why}
                disabled={!$capture.available || switching === o.c.id}
                on:click={() => toggleScreen(o.c, o.w.action)}>
                {switching === o.c.id ? '…' : o.w.label}
              </button>
            {/if}
          </div>
        {:else}
          {#if $readErrors.listOutputChannels}
            <ErrorState compact error={$readErrors.listOutputChannels} />
          {:else}
            <EmptyState message="No screens yet — add one in the Outputs tab." />
          {/if}
        {/each}
      </div>
      {#if nowhereToShow}
        <p class="out-warn" role="status">
          Something is on air, and no screen is reporting that it is showing it.
          Relay is still sending — check the screens above.
        </p>
      {/if}
      {#if fitWarning}
        <p class="out-warn" role="status">{fitWarning}</p>
      {/if}
      {#if screenMsg}
        <p class="out-warn" role="status">{screenMsg}</p>
      {/if}
      <p class="sr-only" aria-live="polite">{downAnnounce}</p>
      <footer class="pane-foot ann">
        <!-- EMERGENCY ANNOUNCEMENT. It paints over live scripture on EVERY screen
             at once, so it belongs with the screens rather than in a drawer of
             quick tools. Armed in two steps for the reason it always was: a stray
             Enter must not be able to interrupt a reading in front of a room. -->
        <div class="sb cd">
          <span>Announce</span>
          <input
            class="cd-msg"
            type="text"
            placeholder="Message for every screen"
            bind:value={annMsg}
            aria-label="Emergency announcement"
            on:keydown={(e) => e.key === 'Enter' && sendAnnouncement()}
            disabled={!$capture.available} />
          <button class="cd-go" class:armed={annArmed} on:click={sendAnnouncement}
            disabled={!$capture.available || !annMsg.trim()}>
            {annArmed ? 'Confirm?' : 'Send'}
          </button>
        </div>
        <button class="wide" on:click={openMainOutput} disabled={!$capture.available}>Open main output</button>
      </footer>
    </section>
    </div>
  </div>

  {#if $capture.audioError}<div class="audioerr">Audio: {$capture.audioError}</div>{/if}
  {#if $capture.outputError}<div class="audioerr">Output: {$capture.outputError}</div>{/if}

  <!-- Only while listening, and only when something is genuinely wrong. A warning
       that is always on screen is wallpaper. -->
  {#if $capture.capturing && qualityWarning}
    <div class="sttwarn"><b>{qualityWarning.title}</b>{qualityWarning.fix}</div>
  {/if}

  {#if $capture.capturing && langWarning}
    <div class="sttwarn"><b>{langWarning.title}</b>{langWarning.fix}</div>
  {/if}

  <!-- §5 INSPECTOR. Mounted at the console root so it overlays the whole surface
       rather than being clipped inside a panel. It is a dialog, so shortcuts.js's
       Escape guard already refuses to clear the screens while it is open. -->
  <DetectionInspector
    detection={inspecting}
    alternatives={inspectAlts}
    onClose={() => (inspecting = null)}
    onAccept={inspectAccept}
    onDismiss={inspectDismiss}
    onTuning={() => { inspecting = null; setSession({ activeTab: 'settings' }); }}
  />
</div>

<style>
  /* LIVE — laid out to docs/design/relay-console-screen.png, styled entirely
     from the --v-* design tokens in app.css. No raw hex, no arbitrary px: every
     colour is a token and every gap comes off the 8pt scale. */
  .inspect-link{ align-self:flex-start; margin-top:9px; background:none; border:0; padding:0;
    font-family:var(--f-body); font-size:12px; color:var(--v-cyan); cursor:pointer;
    text-decoration:underline; }
  .inspect-link:hover{ filter:brightness(1.15); }

  /* ── §4 view controls + compact density ── */
  .view-ctl{ display:flex; align-items:center; justify-content:flex-end; gap:8px; margin-bottom:10px; }
  .seg{ display:flex; border:1px solid var(--v-line2); border-radius:8px; overflow:hidden; }
  .seg button{ padding:5px 11px; background:var(--v-surf); border:0; cursor:pointer;
    font-family:var(--f-body); font-size:11.5px; font-weight:600; color:var(--v-faint); }
  .seg button.on{ background:var(--v-accent-soft); color:var(--v-accent2); }
  .seg button:not(.on):hover{ color:var(--v-dim); }
  .view-fs{ height:26px; padding:0 11px; border-radius:8px; cursor:pointer;
    background:var(--v-surf); border:1px solid var(--v-line2); color:var(--v-faint);
    font-family:var(--f-body); font-size:11.5px; font-weight:600; }
  .view-fs:hover{ color:var(--v-txt); border-color:var(--v-accent-line); }

  /* COMPACT — spacing and type only. Nothing is hidden: see the note in the
     script block. Panels keep every control they have at normal density. */
  .con.compact{ gap:9px; }
  /* The actual density win: give the BOTTOM row its space back. The top row is a
     fixed clamp, so on a 13" booth laptop it eats a third of the window and the
     bottom row — transcript, detections, plan, and the transport — is squeezed
     into whatever is left. Panels scroll internally (.pane-body), so nothing was
     ever unreachable; compact just stops making the operator scroll for the
     controls they use most. */
  .con.compact :global(.con-top){ height:clamp(196px,24vh,268px); }
  .con.compact :global(.desk){ gap:6px; }
  /* Full screen has already reclaimed the chrome, so the exit affordance sits
     where the view controls would be. Keep clear of it rather than under it. */
  .con.compact :global(.pane){ border-radius:10px; }
  .con.compact :global(.pane-head){ padding:8px 11px; }
  .con.compact :global(.pane-head h2){ font-size:11px; }
  .con.compact :global(.pane-body){ padding:10px 11px; }

  .con{
    height:100%; min-height:0; display:flex; flex-direction:column;
    gap:var(--v-sp-sm); color:var(--v-txt); font-family:var(--f-body);
  }
  .spring{flex:1}
  .cnt{font-size:var(--v-fs-cap); color:var(--v-faint)}

  /* ── rehearsal band ── amethyst, never amber. Amber means ON AIR. */
  .reh{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    padding:10px var(--v-sp-md); border-radius:var(--v-r-lg);
    background:var(--v-amethyst-soft); border:1px solid var(--v-amethyst-line);
    font-size:var(--v-fs-b2); line-height:var(--v-lh-b2); color:var(--v-dim)}
  .reh b{font-family:var(--f-mono); font-size:var(--v-fs-cap); font-weight:700;
    letter-spacing:.14em; color:var(--v-amethyst); flex:0 0 auto}
  .reh span:not(.reh-dot){flex:1}
  .con-noengine{ display:flex; align-items:center; gap:10px; padding:9px 12px;
    border:1px solid var(--v-rose); border-radius:var(--v-r-md);
    background:var(--v-rose-soft); margin-bottom:10px }
  .con-noengine span:last-child{ font-size:var(--v-fs-b2); color:var(--v-txt) }

  .reh-dot{width:8px; height:8px; border-radius:50%; flex:0 0 auto; background:var(--v-amethyst);
    box-shadow:0 0 9px var(--v-amethyst); animation:pulse 1.7s ease-in-out infinite}
  .reh-end{flex:0 0 auto; padding:7px 14px; border-radius:var(--v-r-md); cursor:pointer;
    font-family:var(--f-body); font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.06em;
    text-transform:uppercase; background:var(--v-amethyst); border:0; color:var(--v-void)}
  .reh-end:disabled{opacity:.5; cursor:not-allowed}
  @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}}

  /* ── the two rows ──────────────────────────────────────────────────────── */
  /* THE STUDIO SPLIT (docs/REBRAND.md §2). Two EQUAL monitors with the take
     column between them, at the prototype's measured 118px: Preview and Program
     are the same size because they are the same question asked twice — what is
     about to go out, and what is out. The 1.19fr that made Preview wider was a
     hierarchy the room does not have. */
  /* THREE COLUMNS, and the two monitors are EQUAL. Output Status used to take a
     300px fourth column out of this row, which made the pair unequal and pushed
     both below the size at which a rendered slide is recognisable. The screens
     now live in the workspace's own inspector column (`.insp-col`), beneath the
     AI's claims — the whole column is 286px, as measured in the prototype. */
  .con-top{flex:0 0 auto; height:clamp(268px,33vh,364px);
    display:grid; grid-template-columns:1fr 118px 1fr; gap:var(--v-sp-sm); min-height:0}
  /* Five panels: transcript · slides · detection · plan · controls. The grid
     takes the widest share of the flexible columns — a cell the operator cannot
     read is a cell they have to click twice to identify. */
  /* Two panels now: the claim, and the plan. The transcript and the quick
     controls used to sit here and are the DOCK's — one row below, on every
     workspace — and a second copy of a panic control is how two surfaces come to
     disagree about the same room. */
  /* One panel now: the plan. The claim column moved to the inspector, where §2
     puts it and where several claims fit at once — see the note in the markup. */
  .con-bot{flex:1 1 0; min-height:0;
    display:grid; grid-template-columns:minmax(0,1fr); gap:var(--v-sp-sm)}

  /* ── the desk: rail, stage, inspector ──────────────────────────────────── */
  .desk{flex:1; min-height:0; display:grid;
    grid-template-columns:206px minmax(0,1fr) 286px; gap:var(--v-sp-sm)}
  .rail-col{display:flex; flex-direction:column; gap:var(--v-sp-sm); min-height:0; min-width:0}
  .rail-col :global(.lrail){flex:1 1 auto; min-height:0}
  .stage{display:flex; flex-direction:column; gap:var(--v-sp-sm); min-height:0; min-width:0}
  /* The inspector runs the FULL HEIGHT of the workspace: the claims take what is
     left after the screens, because the claims are the thing that keeps arriving
     and the screens are a fixed list. */
  .insp-col{display:flex; flex-direction:column; gap:var(--v-sp-sm); min-height:0; min-width:0}
  .insp-col > .pane:first-child{flex:1 1 auto; min-height:0}
  .insp-col > .pane:last-child{flex:0 0 auto; max-height:46%}

  /* The grid gets the full width under the monitors, and the larger share of
     what is left: it is the surface an operator picks from, and a cell too small
     to read is a cell they have to click to identify. */
  .con-grid{flex:1.35 1 0; min-height:0; display:flex}
  .con-grid :global(.pane){flex:1; min-width:0}

  /* The view controls sit UNDER the rail, not in a band of their own. They change
     how the console looks and never what reaches a screen, so they get the
     quietest corner of the desk rather than a row across it. */
  .rail-col .view-ctl{flex:0 0 auto; margin-bottom:0; justify-content:stretch}
  .rail-col .seg{flex:1}
  .rail-col .seg button{flex:1; text-align:center}

  /* The announcement row in the Output Status footer stacks; `.pane-foot` is a
     row, and an input beside a button beside another button truncates the one
     thing an operator has to read before pressing it. */
  .pane-foot.ann{flex-direction:column; align-items:stretch; gap:6px}

  .pane{display:flex; flex-direction:column; min-height:0; overflow:hidden;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg);
    box-shadow:var(--v-shadow-sm)}

  /* WRAPS. A panel header is a heading plus its controls, and on one unwrapped
     line the controls always won: at 1366×768 — the commonest church laptop —
     "AI Detection — Current Claim" was rendered 5px wide and "Live Transcript"
     74px, because the sensitivity dial and the Armed chip are sized to content and
     the heading was the only thing allowed to shrink. Now the controls drop to a
     second line instead of crushing the name of the panel they belong to. */
  .pane-head{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    flex-wrap:wrap; row-gap:6px;
    padding:10px 12px; border-bottom:1px solid var(--v-line)}
  /* The reference console has no sidebar, so its panels are ~25% wider than they can
     be here. The heading is therefore set a touch tighter than the design sheet's
     Label spec so the full panel name still fits rather than truncating. */
  .pane-head h2{margin:0; min-width:0; font-family:var(--f-head); font-size:var(--v-fs-cap);
    line-height:var(--v-lh-lbl); font-weight:600; letter-spacing:.06em; text-transform:uppercase;
    color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
  /* The numbered panel chips from the reference. Ordinals, not status — grey. */
  .pn{flex:0 0 auto; width:18px; height:18px; border-radius:var(--v-r-sm); display:grid;
    place-items:center; background:var(--v-surf3); border:1px solid var(--v-line2);
    font-family:var(--f-mono); font-size:10px; font-weight:700; color:var(--v-dim)}
  .pane-body{flex:1; min-height:0; overflow-y:auto; padding:var(--v-sp-sm) 12px;
    display:flex; flex-direction:column; gap:var(--v-sp-sm);
    scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent}
  .pane-body::-webkit-scrollbar{width:6px}
  .pane-body::-webkit-scrollbar-thumb{background:var(--v-surf3); border-radius:99px}
  .pane-foot{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    padding:var(--v-sp-sm) 12px; border-top:1px solid var(--v-line)}

  /* ── PREVIEW / PROGRAM ─────────────────────────────────────────────────── */
  .mon-bar{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    padding:8px 10px; border-bottom:1px solid var(--v-line)}
  .tag{flex:0 0 auto; padding:4px 10px; border-radius:var(--v-r-sm);
    font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.09em; text-transform:uppercase}
  /* STEEL BLUE = the thing you are working on, which is what a preview is.
     Amethyst is REHEARSAL and nothing else (rule 18, DECISIONS §22) — this chip
     wore it, so on the one morning both were true the operator read the wrong
     one. The grid's cued cell has always been steel blue; these two now agree. */
  .tag.preview{background:var(--v-sel); color:var(--v-sel-ink)}
  /* Amber, and only when the congregation is genuinely looking at it. */
  .tag.onair{background:var(--v-amber); color:var(--v-amber-ink)}
  .tag.reh{background:var(--v-amethyst-soft); border:1px solid var(--v-amethyst-line); color:var(--v-amethyst)}
  .tag.off{background:var(--v-grey-soft); border:1px solid var(--v-line2); color:var(--v-dim)}
  .mon-name{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:var(--v-fs-cap); color:var(--v-faint)}
  /* Amber ONLY when a congregation is genuinely looking at it. */
  .mon-name.live{color:var(--v-amber)}
  .mon-as{flex:0 0 auto; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:9px; letter-spacing:.05em; color:var(--v-faint)}
  /* THE FRAME CARRIES THE STATE. The prototype frames preview in steel blue and
     programme in amber (amethyst in rehearsal), and it is the right instrument:
     an operator glancing up is looking at the picture, not at a chip beside it.
     Amber is ON AIR, amethyst is REHEARSAL, and neither is ever spent elsewhere. */
  .mon .screen{transition:box-shadow var(--v-dur) var(--v-ease)}
  .mon.prev .screen{box-shadow:inset 0 0 0 2px var(--v-sel)}
  .mon.prog.onair .screen{box-shadow:inset 0 0 0 2px var(--v-amber)}
  .mon.prog.inreh .screen{box-shadow:inset 0 0 0 2px var(--v-amethyst)}
  /* Slate, not amber and not rose: a repeat is a fact, not an alarm. Amber means
     ON AIR and must never be spent on anything else (DECISIONS §22). */
  .mon-repeat{margin-right:8px; padding:1px 6px; border-radius:var(--v-r-sm);
    font-size:10px; letter-spacing:.06em; color:var(--v-faint);
    border:1px solid var(--v-line2)}
  .screen{flex:1; min-height:0; position:relative; overflow:hidden; background:#000;
    border-top:1px solid var(--v-line)}
  .screen-empty{position:absolute; inset:0; display:grid; place-items:center; padding:var(--v-sp-md);
    text-align:center; font-size:var(--v-fs-b2); color:var(--v-faint)}
  .blk{position:absolute; inset:0; background:#000}

  /* ── the take rack ─────────────────────────────────────────────────────── */
  /* align-self:start — the rack is only as tall as its controls. Left to stretch
     to the full height of the 16:9 preview/program panes beside it, the leftover
     vertical space had to go SOMEWHERE, and it ballooned the MODE chip into a huge
     empty box. A transport rack is compact by nature; keep it that way. */
  .rack{display:flex; flex-direction:column; gap:6px; min-height:0; align-self:start; padding:10px 8px;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg)}
  .rack-lbl{font-family:var(--f-mono); font-size:9px; font-weight:700; letter-spacing:.14em;
    text-transform:uppercase; color:var(--v-faint); text-align:center}
  /* 64px, as measured in the prototype. The one control on this surface that is
     always the same press, in the same place, however tired the operator is. */
  .take{height:64px; border-radius:var(--v-r-md); border:0; cursor:pointer;
    background:var(--v-amber); color:var(--v-amber-ink); font-family:var(--f-body);
    font-size:var(--v-fs-lbl); font-weight:700; letter-spacing:.1em;
    box-shadow:0 6px 18px -6px var(--v-amber-glow); transition:filter .14s}
  .take:hover:not(:disabled){filter:brightness(1.06)}
  .take:disabled{opacity:.4; cursor:not-allowed; box-shadow:none}
  .rk{height:28px; border-radius:var(--v-r-md); cursor:pointer; background:var(--v-surf2);
    border:1px solid var(--v-line2); color:var(--v-dim); font-family:var(--f-body);
    font-size:var(--v-fs-cap); transition:.14s}
  .rk:hover:not(:disabled){background:var(--v-surf3); color:var(--v-txt)}
  .rk:disabled{opacity:.4; cursor:not-allowed}
  .rk.wide{width:100%}
  /* The caption the prototype puts under the transport, with the MODE inside it
     rather than beside it. "walks the programme" answers WHAT the two buttons
     do; the mode answers WHICH walk — and they are one sentence, so an operator
     cannot read the first and miss the second. */
  .rack-cap{display:block; margin-top:2px; text-align:center;
    font-size:9px; line-height:1.35; letter-spacing:.04em; color:var(--v-faint)}
  .rack-mode{display:block; margin-top:3px;
    font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.1em; color:var(--v-cyan)}
  /* Amber here is NOT "on air": it is the plan's own colour on the plan rail
     beside it, and SLIDE mode means the arrows walk the plan. It sits on a
     caption, not on a claim about a screen. */
  .rack-mode.slide{color:var(--v-amber)}

  /* ── output status ─────────────────────────────────────────────────────── */
  /* TWO ROWS, not one. On a single flex line the name, the note, the badge and
     the switch competed for a ~275px rail, and the name lost: "Streaming" was
     rendered 7px wide and "Lobby screen" 37px, while the note wrapped one word
     per line and pushed the row to 122px. The screen with the WORST news was the
     least readable of all, because "Not responding" is the longest badge — on the
     one pane whose whole job is telling an operator which screen has died.
     Name + badge on top, the screen's own word + the switch underneath. */
  /* A SCROLLER THAT SAYS SO. Three of five screens fit at 900px, and the pane
     scrolls — but macOS overlay scrollbars are invisible at rest, so the pane
     looked like the whole list. Two `background-attachment: local` gradients: the
     shadow at an edge appears only while there is more content past it, and goes
     when you reach the end. No script, no state, nothing to keep in step. */
  .outs{
    background:
      linear-gradient(var(--v-surf) 30%, transparent) top / 100% 14px no-repeat local,
      linear-gradient(transparent, var(--v-surf) 70%) bottom / 100% 14px no-repeat local,
      radial-gradient(farthest-side at 50% 0, rgba(0,0,0,.4), transparent) top / 100% 7px no-repeat scroll,
      radial-gradient(farthest-side at 50% 100%, rgba(0,0,0,.4), transparent) bottom / 100% 7px no-repeat scroll;
  }
  .out{display:grid; grid-template-columns:28px minmax(0,1fr) auto;
    grid-template-areas:"ic nm badge" "ic note act";
    column-gap:10px; row-gap:2px; align-items:center; padding:9px 10px;
    border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line)}
  .out-ic{grid-area:ic; width:28px; height:28px; border-radius:var(--v-r-sm); display:grid;
    place-items:center; background:var(--v-surf3); color:var(--v-dim)}
  .out-nm{grid-area:nm; min-width:0}
  .out-note{grid-area:note; min-width:0}
  .out .sm-badge{grid-area:badge; justify-self:end}
  .out-sw{grid-area:act; justify-self:end}
  /* Deliberately quiet. This is a repair for one screen, not a panic control —
     Clear and Blackout are the loud pair and nothing else may look like them. */
  .out-sw{font:inherit; font-size:10px; letter-spacing:.04em;
    padding:3px 8px; border-radius:var(--v-r-sm); cursor:pointer;
    background:transparent; color:var(--v-faint); border:1px solid var(--v-line)}
  /* `--v-txt-dim` was never defined. `border-color` is not inherited, so it fell
     back to currentColor — which this rule sets to --v-txt on the same line, and
     that made the loudest border in the pane out of the control the comment above
     calls deliberately quiet. --v-line2 is the app's hover-border step (.r-row,
     .te-swrow, .r-input all use it). */
  .out-sw:hover:not(:disabled){color:var(--v-txt); border-color:var(--v-line2)}
  .out-sw:disabled{opacity:.45; cursor:not-allowed}

  /* WRAPS RATHER THAN TRUNCATES. On a ~230px rail (1366-wide laptop) the status
     badge is 60–115px wide and "Lobby screen" does not fit beside "NO WINDOW".
     A second line costs 12px; an ellipsis costs the operator the identity of the
     screen that has just failed, which is the one thing this pane is for. */
  .out-nm{font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt);
    display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:2;
    overflow:hidden; overflow-wrap:anywhere}
  /* Wraps, but never past two lines: the note is the screen's own word and it is
     worth reading, and it is not worth 120px of a pane an operator scans. */
  .out-note{font-size:9px; letter-spacing:.05em; color:var(--v-faint);
    display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:2;
    overflow:hidden}
  /* A screen that is not answering is a FAILURE, and the row says so without
     spending amber (which means on air, DECISIONS §22) or reading as decoration.
     The border is the signal; the badge carries the word. */
  /* A warning, not a failure and not a live state. Amber would mean ON AIR here
     and rose text would overstate it — Relay IS still sending. Dim text, rose rule. */
  .out-warn{margin:8px 10px 0; padding:7px 9px; font-size:var(--v-fs-cap);
    color:var(--v-dim); background:var(--v-surf2); border-radius:var(--v-r-sm);
    border-left:2px solid var(--v-rose)}
  .out.down{border-color:color-mix(in srgb, var(--v-rose) 45%, transparent);
    background:color-mix(in srgb, var(--v-rose) 7%, var(--v-surf2))}
  .out.down .out-ic{color:var(--v-rose)}
  .out.down .out-note{color:var(--v-rose)}
  .sm-badge{padding:3px 8px; font-size:9px; letter-spacing:.07em; flex:0 0 auto}
  .sm-badge .bd{width:5px; height:5px}

  /* ── 1 · transcript ────────────────────────────────────────────────────── */
  .tx{gap:2px; padding-top:10px}
  .txl{display:flex; gap:10px; padding:7px 8px; border-radius:var(--v-r-md);
    border-left:2px solid transparent}
  /* The line the AI is working on right now. Amethyst: it is being considered, not
     fired — amber would claim it is on the wall. */
  .txl.cur{background:var(--v-amethyst-soft); border-left-color:var(--v-amethyst)}
  .txl-at{flex:0 0 auto; width:56px; font-size:var(--v-fs-cap); color:var(--v-faint)}
  .txl-b{flex:1; min-width:0; font-size:var(--v-fs-b2); line-height:1.55; color:var(--v-dim)}
  .txl.cur .txl-b{color:var(--v-txt)}
  .txl-b mark{background:transparent; color:var(--v-txt)}
  .caret{display:inline-block; width:2px; height:13px; background:var(--v-amethyst);
    vertical-align:-2px; margin-left:2px; animation:blink 1.05s steps(1) infinite}
  @keyframes blink{50%{opacity:0}}

  .mic{gap:10px}
  .mic-lbl{flex:0 0 auto; font-size:var(--v-fs-cap); color:var(--v-faint)}
  .meter{flex:1; min-width:0; display:flex; gap:2px; align-items:center; height:12px}
  .sg{flex:1; height:100%; border-radius:1px; background:var(--v-surf3)}
  .sg.on{background:var(--v-emerald)}
  .sg.on.mid{background:var(--v-amber)}
  .sg.on.hot{background:var(--v-red)}
  .db{flex:0 0 auto; font-size:var(--v-fs-cap); color:var(--v-dim)}
  .ibtn{flex:0 0 auto; width:26px; height:26px; border-radius:var(--v-r-sm); display:grid;
    place-items:center; cursor:pointer; background:var(--v-surf2); border:1px solid var(--v-line2);
    color:var(--v-dim); transition:.14s}
  .ibtn:hover:not(:disabled){color:var(--v-amber)}
  .ibtn:disabled{opacity:.4; cursor:not-allowed}

  /* ── 2 · slides ───────────────────────────────────────────────── */
  .sg-body{padding:var(--v-sp-sm)}
  .sgrid{display:grid; grid-template-columns:repeat(auto-fill,minmax(158px,1fr));
    gap:var(--v-sp-sm)}
  .sg-cell{display:flex; flex-direction:column; gap:5px; padding:0; text-align:left;
    background:none; border:0; cursor:pointer; min-width:0; font-family:var(--f-body)}
  .sg-cell:disabled{opacity:.45; cursor:not-allowed}
  /* A CELL IS THE WALL IN MINIATURE. `position:relative` + `container-type`
     are both load-bearing: `TemplateRender`'s root is `position:absolute;
     inset:0` and it sizes every element in cqw, so the box has to be the
     container the query resolves against or the type comes out at the page's
     width. Black ground, like every other surface that shows what a screen
     shows — a grey card behind a rendered slide is a different slide. */
  .sg-thumb{position:relative; display:block; aspect-ratio:16/9; overflow:hidden;
    container-type:inline-size;
    border-radius:var(--v-r-md);
    background:var(--v-void); border:1px solid var(--v-line2);
    transition:border-color var(--v-dur) var(--v-ease), background var(--v-dur) var(--v-ease)}
  /* A cue the grid could not expand. It is DRAWN rather than dropped (see
     `planCells`) so the count under the grid agrees with the plan, and it is
     disabled rather than firing nothing. */
  .sg-void{position:absolute; inset:0; display:grid; place-items:center; padding:8px;
    text-align:center; font-size:10px; letter-spacing:.05em; color:var(--v-faint)}
  .sg-cell:hover .sg-thumb{border-color:var(--v-sel-line)}
  /* Steel blue is SELECTION — the thing you are working on. It is what a preview
     is, and it is deliberately not grey: grey means CUED, a plan position. */
  .sg-cell.cued .sg-thumb{border-color:var(--v-sel); background:var(--v-sel-soft)}
  /* Amber is ON AIR and nothing else. `cellLive` derives it from what the store
     says is on the screen, never from "we pressed the button". */
  .sg-cell.islive .sg-thumb{border-color:var(--v-amber); background:var(--v-amber-soft)}
  .sg-cell:focus-visible .sg-thumb{outline:2px solid var(--v-sel); outline-offset:2px}
  /* TOP-LEFT, over the rendered slide, on its own scrim. It used to sit
     bottom-left on a grey card; over a real slide it needs its own ground or it
     lands on whatever the template happens to be painting there. `--v-surf3` is
     not a legible ground for dim text (tokencontrast.test.js), so the chip
     carries its own black and near-white. */
  .sg-tag{position:absolute; left:5px; top:5px; padding:2px 5px; z-index:2;
    border-radius:2px; background:rgba(0,0,0,.62); color:#cfd6e2;
    font-size:8px; font-weight:600; letter-spacing:.08em; text-transform:uppercase}
  .sg-air,.sg-prev{position:absolute; right:5px; top:5px; padding:2px 5px; z-index:2;
    border-radius:2px; font-size:8px; font-weight:700; letter-spacing:.1em;
    text-transform:uppercase}
  .sg-air{background:var(--v-amber); color:var(--v-amber-ink)}
  .sg-prev{background:var(--v-sel); color:var(--v-sel-ink)}
  .sg-meta{display:flex; align-items:baseline; gap:6px; min-width:0; padding:0 2px}
  .sg-n{flex:0 0 auto; font-size:10px; color:var(--v-dim)}
  .sg-ttl{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:var(--v-fs-cap); color:var(--v-txt)}
  .sg-cell.islive .sg-ttl{color:var(--v-amber)}
  .sg-foot{display:flex; align-items:center; gap:var(--v-sp-sm)}
  .sg-cap{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:10px; color:var(--v-dim)}

  /* ── 3 · detection ─────────────────────────────────────────────────────── */
  .chip{display:inline-flex; align-items:center; gap:6px; flex:0 0 auto; padding:4px 9px;
    border-radius:99px; background:var(--v-surf2); border:1px solid var(--v-line2);
    font-size:var(--v-fs-cap); color:var(--v-faint)}
  .chip .bd{width:6px; height:6px; border-radius:50%; background:var(--v-faint)}
  .chip.ok{color:var(--v-emerald); border-color:var(--v-emerald-line); background:var(--v-emerald-soft)}
  .chip.ok .bd{background:var(--v-emerald); box-shadow:0 0 6px var(--v-emerald)}
  .btnchip{cursor:pointer; font-family:var(--f-body)}
  .btnchip:disabled{opacity:.5; cursor:not-allowed}

  /* The gate's own controls get their own row under the heading. On one line in
     a 286px column the dial, the Armed chip and the microphone button are sized
     to content and the panel's NAME is the only thing allowed to shrink — the
     same failure `.pane-head`'s wrap comment records at 1366px, one column
     narrower. */
  .det-ctl{flex:0 0 auto; display:flex; align-items:center; gap:8px;
    padding:8px 12px; border-bottom:1px solid var(--v-line)}
  /* WHAT THE GATE IS DOING — and it reads differently when it is broken. Grey
     until Relay is genuinely armed and listening; emerald when it is, which is
     the same green the Armed chip beside it already uses. Never amber: nothing
     about a gate's readiness is on air. */
  .det-meta{flex:0 0 auto; font-size:9px; letter-spacing:.08em; text-transform:uppercase;
    color:var(--v-faint)}
  .det-meta.on{color:var(--v-emerald)}

  /* Sensitivity dial — compact, on the run surface. Reaches the same thresholds
     as Settings, and is now the SAME INSTRUMENT: the track, the thumb and the
     filled share all come from app.css, so the dial an operator learns in
     Settings is the dial they use during a service. Width is the only thing
     that is genuinely local — this one lives in a crowded transport bar. */
  .sens{display:inline-flex; align-items:center; gap:7px; flex:0 0 auto;}
  .sens-lbl{font-size:var(--v-fs-cap); letter-spacing:var(--v-tr-caps); color:var(--v-faint);}
  .sens-val{font-size:var(--v-fs-cap); color:var(--v-dim); min-width:20px; text-align:right;}
  /* Width ONLY. THE BAR IS 3px AND THE CONTROL IS NOT — an input styled as the
     track is a 4px pointer target, and a near-miss on a live console lands on
     whatever is underneath. That box (18px) and the track that draws the bar
     inside it are app.css's `input[type=range]` block, which this dial shares
     with every other slider in the app; overriding them here is what made the
     dial a different instrument from the one in Settings. */
  .sens input[type="range"]{width:88px;}

  /* ── A CLAIM CARD ─────────────────────────────────────────────────────────
     One card per claim, in a column, because a decode window can name several
     references and an operator choosing between them needs to see them
     together. The left rule carries the KIND and is the fastest thing to read
     down a column: amber for a reference Relay HEARD, cyan for a guess, grey
     once the claim has been decided and nothing is owed. */
  .clm{background:var(--v-surf2); border:1px solid var(--v-line);
    border-left:3px solid var(--v-amber);
    border-radius:var(--v-r-md); padding:11px 12px;
    display:flex; flex-direction:column; gap:8px}
  /* A GUESS MUST LOOK LIKE A GUESS. Amber reads as "Relay is confident" and a
     paraphrase has not earned it — its score is a cosine, and router.rs will not
     let it auto-fire at ANY value. Cyan, NOT amethyst: amethyst already means
     REHEARSAL, and a colour that means "nothing is reaching the congregation"
     cannot also mean "this guess is shaky", or on the day both are true the
     operator reads the wrong one. */
  .clm.guess{border-left-color:var(--v-cyan)}
  /* DECIDED. Grey, and quieter — it is a receipt, and nothing on it is
     actionable. It must never look like a claim still waiting for a press. */
  .clm.done{opacity:.62; border-left-color:var(--v-line2); background:var(--v-surf)}
  .clm-top{display:flex; align-items:center; gap:8px}
  .clm-ref{flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-family:var(--f-head); font-size:var(--v-fs-h2); line-height:1.2;
    font-weight:700; letter-spacing:var(--v-tr-tight); color:var(--v-txt)}
  .cbadge{flex:0 0 auto; padding:2px 6px; border-radius:2px; font-family:var(--f-mono);
    font-size:8.5px; font-weight:600; letter-spacing:.08em; text-transform:uppercase;
    background:var(--v-amber-soft); color:var(--v-amber)}
  .cbadge.p{background:var(--v-cyan-soft); color:var(--v-cyan)}
  /* Confidence as a BAR — "0.92" means nothing to a volunteer. Only ever drawn
     for a heard reference, the only one whose number means what it appears to
     mean. A paraphrase gets the sentence below instead, and no number at all. */
  .conf{height:3px; border-radius:2px; background:var(--v-surf3); margin:0; overflow:hidden}
  .conf i{display:block; height:100%; background:var(--v-amber); border-radius:2px}
  .guess-note{margin:0; font-size:var(--v-fs-cap); line-height:1.45; color:var(--v-cyan)}
  /* THE EVIDENCE — the words that actually triggered the match. */
  .mt-q{margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-dim)}
  /* The verse, in the serif face the wall uses. Clamped: the whole thing is
     already rendered in its real template in the Preview pane, so a second full
     copy buys nothing and runs to ten lines on a psalm. */
  .clm-verse{margin:0; font-family:var(--f-serif);
    font-size:var(--v-fs-b2); line-height:1.5; color:var(--v-txt);
    display:-webkit-box; -webkit-box-orient:vertical; -webkit-line-clamp:4; overflow:hidden}
  .clm.guess .clm-verse{color:var(--v-dim)}
  /* WHO ACTED. Quiet, because a receipt is not an offer. */
  .clm-done{margin:0; font-family:var(--f-mono); font-size:9px; letter-spacing:.08em;
    text-transform:uppercase; color:var(--v-faint)}
  /* No verse behind the reference. Rose is the failure colour on this screen;
     amber is never spent here, because nothing about this is on air. */
  .claim-absent{margin:0; font-size:var(--v-fs-cap); line-height:1.5;
    color:var(--v-rose)}
  .cacts{display:grid; grid-template-columns:1fr 1fr; gap:6px}
  .act{display:flex; flex-direction:column; gap:1px; align-items:center; padding:7px 8px;
    border-radius:var(--v-r-sm); cursor:pointer; border:1px solid transparent;
    font-family:var(--f-body); transition:filter .14s; min-width:0}
  .act b{font-size:var(--v-fs-cap); font-weight:700}
  .act span{font-size:9px; opacity:.85; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:100%}
  .act:hover:not(:disabled){filter:brightness(1.08)}
  /* DISABLED, NOT HIDDEN. A reference that parsed against no verse still has to
     be shown — it is the operator's evidence that a number was misheard — and
     the control that cannot take it says why rather than failing after a press. */
  .act:disabled{cursor:not-allowed; opacity:.45}
  .act.go{background:var(--v-emerald); color:var(--v-void)}
  .act.no{background:var(--v-red); color:#fff}
  .khint{margin:0; text-align:center; font-size:10px; color:var(--v-faint)}
  .khint kbd{font-family:var(--f-mono); font-size:9px; color:var(--v-dim);
    background:var(--v-surf3); border:1px solid var(--v-line2); border-radius:var(--v-r-sm); padding:2px 5px}

  .klbl{font-family:var(--f-mono); font-size:9px; font-weight:700; letter-spacing:.14em;
    text-transform:uppercase; color:var(--v-faint)}
  .klbl.sec{margin-top:var(--v-sp-sm)}
  .sub{display:flex; align-items:center; gap:var(--v-sp-sm); margin-top:var(--v-sp-sm);
    padding-top:var(--v-sp-sm); border-top:1px solid var(--v-line)}
  .mini{padding:4px 10px; border-radius:var(--v-r-sm); border:0; cursor:pointer;
    font-family:var(--f-body); font-size:var(--v-fs-cap); font-weight:600;
    background:var(--v-amber); color:var(--v-amber-ink)}
  .mini.ghost{background:transparent; border:1px solid var(--v-line2); color:var(--v-dim)}
  .mini:hover{filter:brightness(1.08)}
  .rel-note{margin:0; font-size:10px; color:var(--v-faint)}
  .rel-chips{display:flex; flex-wrap:wrap; gap:6px}
  .rel-chip{font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-dim);
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:99px;
    padding:5px 11px; cursor:pointer}
  .rel-chip:hover:not(:disabled){color:var(--v-txt); background:var(--v-surf3)}
  .rel-chip:disabled{opacity:.45; cursor:default}

  .entry{gap:var(--v-sp-sm)}
  .search{flex:1; min-width:0; display:flex; align-items:center; gap:9px; height:34px;
    padding:0 11px; border-radius:var(--v-r-md); background:var(--v-bg);
    border:1px solid var(--v-line2); color:var(--v-faint)}
  .search input{flex:1; min-width:0; background:transparent; border:0; outline:none;
    color:var(--v-txt); font-family:var(--f-mono); font-size:var(--v-fs-mono)}
  .search input::placeholder{color:var(--v-faint)}
  .search:focus-within{border-color:var(--v-amber-line); box-shadow:0 0 0 3px var(--v-amber-soft)}
  .err{padding:0 12px 10px; color:var(--v-red); font-size:var(--v-fs-cap)}

  .wide{width:100%; height:32px; border-radius:var(--v-r-md); cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-txt);
    font-family:var(--f-body); font-size:var(--v-fs-lbl); font-weight:600; transition:.14s}
  .wide:hover:not(:disabled){background:var(--v-surf3)}
  .wide:disabled{opacity:.45; cursor:not-allowed}
  .wide.amber{flex:0 0 auto; width:auto; padding:0 18px; background:var(--v-amber);
    border-color:transparent; color:var(--v-amber-ink)}

  /* ── 4 · plan ──────────────────────────────────────────────────────────── */
  .plan{gap:6px}
  .rail{display:flex; align-items:stretch; gap:10px}
  .rail-dot{flex:0 0 auto; align-self:center; width:9px; height:9px; border-radius:50%;
    background:var(--v-surf3); border:1px solid var(--v-line2)}
  .rail-dot.on{background:var(--v-amber); border-color:var(--v-amber); box-shadow:0 0 8px var(--v-amber-glow)}
  .rail-dot.cued{background:var(--v-grey); border-color:var(--v-grey)}
  .cue,.slide{display:flex; align-items:center; gap:9px; width:100%; flex:1; text-align:left;
    cursor:pointer; padding:9px 10px; border-radius:var(--v-r-md); background:var(--v-surf2);
    border:1px solid var(--v-line); color:var(--v-txt); font-family:var(--f-body); transition:.14s}
  .cue:hover,.slide:hover{border-color:var(--v-line2); background:var(--v-surf3)}
  /* SELECTION is steel blue, never cyan. Cyan means "a guess" (rule 18), and this
     sat one line above the amber rule that says a promise colour may mean only
     what it says. `LyricsPane` already had it right — "Selection is chrome — the
     accent, never amber" — and the rebrand created --v-sel for exactly this. */
  .cue.sel{border-color:var(--v-sel-line)}
  /* Amber = it is in front of the congregation. Nothing else may use it. */
  .cue.islive,.slide.islive{border-color:var(--v-amber); background:var(--v-amber-soft)}
  /* CUED = where → will resume from, but NOT on screen. Deliberately not amber. */
  .cue.cued,.slide.cued{border-style:dashed; border-color:var(--v-grey)}
  .cue-stripe,.slide-stripe{width:3px; align-self:stretch; border-radius:99px; flex:0 0 auto}
  .slide-stripe{background:var(--acc)}
  .cue-num{flex:0 0 auto; font-size:10px; color:var(--v-faint)}
  .cue-body{flex:1; min-width:0; display:flex; flex-direction:column; gap:2px}
  /* Two lines, not an ellipsis. Three plans truncated to 57px on a 1366 laptop
     ("Sunday M…", "Evening P…", "Harvest T…") are three plans an operator cannot
     tell apart, on the control that RUNS one of them. */
  .cue-title{font-size:var(--v-fs-b2); display:-webkit-box; -webkit-box-orient:vertical;
    -webkit-line-clamp:2; overflow:hidden; overflow-wrap:anywhere}
  .cue-meta{font-size:9px; letter-spacing:.05em; color:var(--v-faint)}
  /* Plan picker (no plan loaded) — a plan reads as a real, inviting card. */
  .pick-intro{ padding:2px 2px 4px; flex:0 0 auto; }
  /* A pick card is NATURAL height (flex:0 0 auto) — `.cue` is flex:1, which made a
     single plan card stretch to fill the whole pane. Picks stack at the top. */
  .cue.pick{ flex:0 0 auto; padding:13px; gap:12px; align-items:center; }
  .pick-ic{ flex:0 0 auto; width:32px; height:32px; display:grid; place-items:center;
    border-radius:var(--v-r-md); background:var(--v-surf3); border:1px solid var(--v-line); color:var(--v-dim); transition:.14s; }
  .cue.pick:hover .pick-ic{ color:var(--v-accent); border-color:var(--v-accent-line); }
  .cue.pick .cue-title{ font-size:var(--v-fs-b1); font-weight:600; }
  .cue.pick .cue-meta{ font-size:var(--v-fs-cap); letter-spacing:.03em; }
  .pick-run{ flex:0 0 auto; display:inline-flex; align-items:center; gap:5px; padding:5px 11px;
    border-radius:99px; background:var(--v-accent-soft); border:1px solid var(--v-accent-line);
    color:var(--v-accent); font-size:var(--v-fs-cap); font-weight:600; transition:.14s; }
  .cue.pick:hover .pick-run{ background:var(--v-accent-fill); color:var(--v-accent-ink); border-color:var(--v-accent-fill); }
  .slide{align-items:flex-start; margin-left:19px; background:var(--v-bg)}
  .slide-tag{flex:0 0 auto; min-width:26px; padding-top:2px; font-size:9px; font-weight:700;
    letter-spacing:.05em; color:var(--acc)}
  .slide-text{flex:1; min-width:0; font-family:var(--f-serif); font-size:var(--v-fs-b2);
    line-height:1.5; color:var(--v-dim); white-space:pre-wrap;
    display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical; overflow:hidden}
  .slide.islive .slide-text{color:var(--v-txt)}
  .note{margin-left:19px; display:flex; align-items:flex-start; gap:7px; padding:8px 10px;
    border-radius:var(--v-r-md); background:var(--v-amethyst-soft);
    border:1px solid var(--v-amethyst-line); color:var(--v-amethyst);
    font-size:var(--v-fs-cap); line-height:1.5}
  .note svg{flex:0 0 auto; margin-top:2px}
  .flash{display:flex; align-items:center; gap:8px; min-width:0; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap; font-size:var(--v-fs-cap); color:var(--v-emerald)}
  .flash.idle{color:var(--v-faint)}
  .fd{width:6px; height:6px; border-radius:50%; background:var(--v-emerald);
    box-shadow:0 0 8px var(--v-emerald); flex:0 0 auto}

  /* ── 5 · quick controls ────────────────────────────────────────────────── */
  .quick{gap:5px}
  .q4{display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr); gap:var(--v-sp-sm)}
  .q4.tight{gap:6px}
  .qb{display:flex; flex-direction:column; gap:2px; align-items:flex-start; text-align:left;
    padding:10px 11px; border-radius:var(--v-r-md); cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-txt);
    font-family:var(--f-body); transition:.14s}
  .qb{min-width:0; padding:8px 10px}
  /* THE NAME OF A PANIC CONTROL IS NEVER TRUNCATED. At 1366px — the commonest
     church laptop — this read "Clear scree…" over "Stop all outputs …". A key an
     operator reaches for once a year, in front of people, must not need guessing;
     two lines cost 12px and the column already scrolls. */
  .qb b{font-size:var(--v-fs-b2); font-weight:600; max-width:100%; overflow-wrap:anywhere}
  .qb span{font-size:10px; color:var(--v-faint); max-width:100%; overflow-wrap:anywhere}
  .qb:disabled{opacity:.45; cursor:not-allowed}
  .qb.red{background:var(--v-red-soft); border-color:var(--v-red-line); color:var(--v-red)}
  /* BLACKOUT IS BLACK, not grey. Grey means CUED — where the transport resumes,
     and not on screen (CLAUDE.md, frontend shape). Wearing it here meant the one
     control that takes the wall to black shared a colour with a position marker.
     The hairline is what keeps it findable on a dark desk. */
  .qb.black{background:linear-gradient(180deg,#0b0d12,#050609); border-color:rgba(210,220,235,.34); color:#d7deea}
  .qb.black.on{background:#000; border-color:#e8edf5; color:#fff}
  .qb.amethyst.on{background:var(--v-amethyst-soft); border-color:var(--v-amethyst-line); color:var(--v-amethyst)}
  .qb.cyan.on{background:var(--v-cyan-soft); border-color:var(--v-cyan-line); color:var(--v-cyan)}
  .qb:hover:not(:disabled){filter:brightness(1.12)}

  .modes{display:flex; flex-direction:column; gap:5px}
  .md{display:flex; align-items:center; gap:9px; padding:6px 10px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line);
    font-size:var(--v-fs-b2); color:var(--v-faint)}
  .md i{width:11px; height:11px; border-radius:50%; flex:0 0 auto;
    border:1px solid var(--v-line2); background:transparent}
  .md em{font-style:normal; font-size:10px; opacity:.8}
  .md.on{background:var(--v-amethyst-soft); border-color:var(--v-amethyst-line); color:var(--v-txt)}
  .md.on i{background:var(--v-amethyst); border-color:var(--v-amethyst)}

  .sb{display:flex; align-items:center; justify-content:space-between; gap:7px; min-width:0;
    padding:7px 10px; border-radius:var(--v-r-md); cursor:pointer; background:var(--v-surf2);
    border:1px solid var(--v-line2); color:var(--v-txt); font-family:var(--f-body);
    font-size:var(--v-fs-cap); transition:.14s}
  .sb>span{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
  .sb:hover:not(:disabled){background:var(--v-surf3)}
  .sb:disabled{opacity:.45; cursor:not-allowed}
  .sb i{font-style:normal; color:var(--v-faint)}
  .sb.cd{cursor:default; gap:6px; justify-content:flex-start}
  /* The row LABEL never shrinks. "Announce" was ellipsised to "An…" beside a text
     field that had all the room it wanted — and this is the control that writes
     over live scripture on every screen at once. */
  .sb.cd>span:first-child{flex:0 0 auto; overflow:visible}
  .cd-unit{flex:0 0 auto; font-size:9px; color:var(--v-faint)}
  .cd-min{width:40px; padding:3px 5px; border-radius:var(--v-r-sm); border:1px solid var(--v-line2);
    background:var(--v-bg); color:var(--v-txt); font-size:var(--v-fs-cap); text-align:center}
  /* The announcement field takes the row's spare width — the message, not the
     label, is the part the operator is reading back before they confirm. */
  .cd-msg{flex:1 1 auto; min-width:0; padding:3px 7px; border-radius:var(--v-r-sm);
    border:1px solid var(--v-line2); background:var(--v-bg); color:var(--v-txt);
    font-size:var(--v-fs-cap)}
  .cd-go{padding:4px 9px; border-radius:var(--v-r-sm); border:1px solid var(--v-cyan-line);
    background:var(--v-cyan-soft); color:var(--v-cyan); font-family:var(--f-mono);
    font-size:10px; font-weight:700; cursor:pointer}
  .cd-go:hover:not(:disabled){filter:brightness(1.2)}
  .cd-go:disabled{opacity:.45; cursor:not-allowed}
  .cd-go.armed{background:var(--v-amber-soft); border-color:var(--v-amber-line); color:var(--v-amber)}

  .amon{display:flex; align-items:center; gap:var(--v-sp-sm); padding:7px 10px;
    border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line)}
  .amon-k{flex:0 0 auto; font-size:var(--v-fs-cap); color:var(--v-faint)}

  /* ── banners ───────────────────────────────────────────────────────────── */
  .audioerr{flex:0 0 auto; background:var(--v-red-soft); color:var(--v-red);
    border:1px solid var(--v-red-line); border-radius:var(--v-r-md);
    padding:9px 12px; font-size:var(--v-fs-lbl)}
  /* Degraded, not broken: amber (a warning), never red (an error) — the app is still
     fully usable by hand, and the banner should read that way. */
  .sttwarn{flex:0 0 auto; background:var(--v-amber-soft); color:var(--v-txt);
    border:1px solid var(--v-amber-line); border-radius:var(--v-r-md);
    padding:10px 12px; font-size:var(--v-fs-lbl); line-height:1.6}
  .sttwarn b{display:block; margin-bottom:2px; color:var(--v-amber2)}

  /* ── accessibility ─────────────────────────────────────────────────────── */
  .take:focus-visible,.rk:focus-visible,.cue:focus-visible,.slide:focus-visible,
  .act:focus-visible,.qb:focus-visible,.sb:focus-visible,.wide:focus-visible,
  .mini:focus-visible,.ibtn:focus-visible,.btnchip:focus-visible,.cd-go:focus-visible,
  .reh-end:focus-visible{outline:2px solid var(--v-amber); outline-offset:2px}
  @media (prefers-reduced-motion:reduce){
    .caret,.reh-dot{animation:none}
  }

  /* ── responsive ────────────────────────────────────────────────────────── */
  @media (max-width:1400px){
    .con-top{grid-template-columns:1fr 104px 1fr}
    .desk{grid-template-columns:180px minmax(0,1fr) 250px}
  }
  /* THE INSPECTOR GOES UNDER, NEVER AWAY. The prototype hides its right column
     below 1240px; Relay may not, because the column holds the AI's claims and
     their Accept / Dismiss — the two controls the product exists to offer — and
     the screens pane. A booth laptop is where an operator is most cramped and
     least able to go hunting, so the column becomes a row beneath the stage
     instead: nothing is removed, and nothing needs a scroll to reach. */
  @media (max-width:1180px){
    .con{height:auto}
    .desk{grid-template-columns:1fr}
    .insp-col{flex-direction:row; align-items:stretch}
    .insp-col > .pane:first-child{flex:1 1 60%; min-height:340px}
    .insp-col > .pane:last-child{flex:1 1 40%; max-height:none}
    /* The rail becomes a strip above the stage rather than a column beside it —
       nothing is removed, because a booth laptop is where an operator is most
       cramped and least able to go hunting. */
    .rail-col{flex-direction:row; align-items:stretch; height:200px}
    .con-top{height:auto; grid-template-columns:1fr 104px 1fr; grid-auto-rows:minmax(230px,auto)}
    .con-grid{flex:0 0 auto; height:320px}
    .con-bot{grid-template-columns:1fr 1fr; grid-auto-rows:minmax(320px,auto)}
  }
  @media (max-width:760px){
    /* AUTO rows. In one column the 230px floor inherited from the rule above was
       being applied to the TAKE rack too — a short strip of buttons padded out to
       230px, leaving a dead gap between Preview and Program. The two screens keep
       their own height; the rack takes what it needs. */
    .con-top{grid-template-columns:1fr; grid-auto-rows:auto}
    .con-top>.pane{min-height:230px}
    .rack{min-height:0}
    .con-bot{grid-template-columns:1fr}
    .rail-col{flex-direction:column; height:auto}
    .insp-col{flex-direction:column}
    .insp-col > .pane:first-child,
    .insp-col > .pane:last-child{flex:0 0 auto; min-height:0}
    .rack{flex-direction:row; align-items:center; flex-wrap:wrap}
    .rack-cap{flex:1 0 100%; margin-top:0}
  }
</style>
