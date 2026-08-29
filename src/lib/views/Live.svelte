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
  import { describeScreen, SCREEN_BADGE } from '../outputHealth.js';
  import TemplateRender from '../TemplateRender.svelte';
  import { resolveOutputTemplate } from '../layers.js';
  import ModelSetup from '../ModelSetup.svelte';
  import { registerContext } from '../shortcuts.js';
  import { t } from '../i18n.js';
  import EmptyState from '../ui/EmptyState.svelte';
  import Loading from '../ui/Loading.svelte';
  import { heard, methodKey } from '../detect.js';
  import DetectionInspector from '../DetectionInspector.svelte';
  import { humanError as humanErrorBase } from '../errors.js';
  import { TYPE, payloadOf, slidesOf, slideAccent, cueSub, nextOf, stepFrom } from '../plan.js';
  import { session, setSession } from '../session.js';
  import { get } from 'svelte/store';
  import {
    capture,
    meter,
    liveContent,
    liveTemplateOverride,
    liveTemplatePinned,
    transcript,
    detections,
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
    channelStatus,
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

  async function loadPlan(p) {
    openPlan = p;
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
  let chStatus = {}; // channel id → ChannelLiveness from the backend
  let chPoll = null;
  // When we first saw a channel as attached-but-not-yet-answering. A window that
  // has just opened has not had time to report, and calling that a fault would
  // teach an operator to ignore the one colour that matters. After a grace period
  // silence stops being "not yet" and becomes the finding.
  let awaitingSince = {};
  const BEAT_GRACE_MS = 8000;

  async function pollChannelHealth() {
    const rows = await channelStatus();
    const next = {};
    for (const r of rows) next[r.id] = r;
    const now = Date.now();
    for (const r of rows) {
      const attached = r.supported && r.online;
      if (attached && !r.painting) {
        if (!awaitingSince[r.id]) awaitingSince[r.id] = now;
      } else {
        delete awaitingSince[r.id];
      }
    }
    chStatus = next;
  }

  // The badge rule itself lives in `lib/outputHealth.js` and is PURE, so it can be
  // tested without mounting this view and so the Outputs inspector cannot end up
  // saying something different about the same screen. All four inputs are named in
  // this expression on purpose — a helper that closes over `chStatus` would not be
  // tracked by Svelte's reactivity and the pane would freeze on its first reading,
  // which is the same class of bug as the badge it replaces.
  $: outs = channels.map((c) => ({
    c,
    s: describeScreen(
      chStatus[c.id] ?? null,
      { rehearsing: $rehearsing, live: !!$live, black: $screenBlack },
      awaitingSince[c.id] ? Date.now() - awaitingSince[c.id] : 0,
    ),
  }));

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

  onMount(async () => {
    await loadRehearsal();
    getSensitivity().then((v) => (sensitivity = v));
    // Populate the reactive `$templates` store so the preview/program panes
    // resolve (and stay live to edits) from it, not just a one-shot snapshot.
    await loadTemplates().catch(() => {});
    await loadDefaultTemplate().catch(() => {});
    channels = await listOutputChannels().catch(() => []);
    // Poll at the beat interval, so a screen that stops answering shows up within
    // about three beats. One command, no payload — cheap enough to run for the
    // length of a service on the surface that needs it most.
    await pollChannelHealth();
    chPoll = setInterval(pollChannelHealth, 2000);
    plans = await listPlans().catch(() => []);
    plansLoaded = true;

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
  });
  let unregisterKeys;
  let unsubNav;
  onDestroy(() => {
    unregisterKeys?.();
    unsubNav?.();
    clearTimeout(cdArmT);
    // The emergency announcement's arm timer, cleared for the same reason as the
    // countdown's right above it. It was the one of the pair that was missed.
    clearTimeout(annArmT);
    clearTimeout(liveMsgT);
    clearTimeout(relatedT); // a pending poll must not fire into a destroyed view
    clearInterval(chPoll);
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
        // Lyrics carry NO title/section on the live screen — that stays in the
        // operator UI. Only the lyric lines go out.
        await fireContent('', s.text, 'song', stageNote, tpl, true); // keepPlan
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
    // This one is a SCREEN going blank, not a hint disappearing. If it fails the
    // preacher keeps reading a stale "up next" all service, and until 2026-08-14
    // nothing anywhere said so — the wrapper swallowed it.
    try {
      await setStageNext(null, null);
    } catch (e) {
      flash(`The preacher's stage monitor may still show the old "up next" — ${humanError(e)}`);
      return;
    }
    if (ok) flash($t('live.screens_cleared'));
  }

  async function blackAll() {
    const ok = await blackScreen();
    if (ok) flash('Blackout');
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

  // heard() / methodLabel() live in lib/detect.js — pure, and unit-tested there,
  // because they are the frontend half of the auto-fire safety rule (see that file).
  // Await it, and flash ONLY if the verse actually went up. This used to fire and
  // forget, then say "Now live: John 3:16" regardless — while confirmDetection
  // swallowed the failure and removed the suggestion card. The operator pressed A, the
  // card vanished, the toast said it was live, and the wall was unchanged.
  async function acceptTop() {
    const d = dets[0];
    if (!d) return;
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
  async function onSensitivity(v) {
    // Optimistic, then CORRECTED — never assumed. The slider used to be written
    // from the request and the result thrown away, so a refused change left the
    // dial showing a position the gate had never reached.
    sensitivity = v;
    try {
      const landed = await setSensitivity(v);
      // The backend owns the curve and its inverse; trust its number, not ours.
      if (Number.isFinite(landed)) sensitivity = landed;
    } catch (e) {
      // Put the dial back where the GATE actually is, read from the backend rather
      // than remembered here, and say so. A slider that silently disagrees with the
      // thing it controls is the whole finding.
      sensitivity = await getSensitivity();
      flash(`Sensitivity stayed at ${sensitivity} — ${humanError(e)}`);
    }
  }

  function dismissTop() {
    if (!dets[0]) return;
    dismissDetection(dets[0].reference);
  }

  // ── transcript ───────────────────────────────────────────────────────────
  let transcriptEl;
  // afterUpdate, never a reactive block: tick() inside `$:` re-enters the Svelte
  // scheduler and hard-freezes the webview. That one cost hours.
  //
  // Reading scrollHeight forces a synchronous reflow, so do it ONLY when the
  // transcript actually changed — not on every unrelated component update (a
  // detection, a meter tick, a hover). `$transcript` updates several times a
  // second during a sermon; scrolling on every render made that a reflow-per-tick.
  let lastTxSig = '';
  afterUpdate(() => {
    if (!transcriptEl) return;
    const sig = `${$transcript.finals.length}|${$transcript.partial}`;
    if (sig === lastTxSig) return;
    lastTxSig = sig;
    transcriptEl.scrollTop = transcriptEl.scrollHeight;
  });
  $: hasTranscript = $transcript.finals.length > 0 || $transcript.partial.length > 0;

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
      flash(humanError(e));
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
      fix: `It has heard ${langs.join(', ')} in the last few minutes. Pick the language in Settings → Scripture & Bible → Recognition Language — auto-detect struggles with a strong accent, and a wrong guess garbles the transcript.`,
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
  $: previewContent = dets[0]
    ? { reference: dets[0].reference, text: dets[0].text ?? '', translation: null }
    : previewSlide
      ? { reference: previewCue.item.label, text: previewSlide.text || previewSlide.label, translation: null }
      : null;
  $: previewLabel = dets[0]
    ? dets[0].reference
    : previewCue
      ? `${previewCue.item.label} · ${previewSlide?.label ?? ''}`.trim()
      : '';
  /** The take. Never a new code path — the same accept/fire the keys already run. */
  async function take() {
    if (dets[0]) return acceptTop();
    if (previewCue) return fireSlide(previewCue.item, previewCue.slide);
  }

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

  // ── transcript arrival times ─────────────────────────────────────────────
  // Each final's arrival time is stamped in the store (`finalsAt`) and trimmed in
  // lockstep with `finals`, so line and time can never drift. The old view-local
  // length-tracking froze once the rolling cap pinned `finals.length` at
  // MAX_FINALS: every new line shifted the array left while the length stayed 12,
  // so the change was never detected and every stamp then labelled the wrong line.
  //
  // Newest last, and the LAST one is the one the AI is currently working on — that
  // is the line the reference highlights.
  $: tLines = $transcript.finals.map((text, i) => ({ text, at: $transcript.finalsAt?.[i] ?? '' }));

  // ── audio meter ──────────────────────────────────────────────────────────
  // Segment count is fixed; which segments light is the LEARNED level, never an
  // absolute threshold (DECISIONS §19 — nothing here compares a signal to a fixed
  // level; it only draws the one the engine already computed).
  const SEGS = 24;
  // Hoisted: `$meter` ticks ~15×/s during a sermon and each VU meter used to
  // rebuild a fresh `Array.from({length: SEGS})` on every one of those renders —
  // two throwaway 24-element arrays per tick, pure GC churn. One frozen array,
  // iterated read-only, does the same job with no allocation.
  const SEG_ARR = Array.from({ length: SEGS });
  $: lvl = Math.max(0, Math.min(1, $meter.level ?? 0));
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
  function inspectTop() {
    inspecting = dets[0] ?? null;
  }
  async function inspectAccept() {
    inspecting = null;
    await acceptTop();
  }
  async function inspectDismiss() {
    inspecting = null;
    await dismissTop();
  }

  $: litSegs = Math.round(lvl * SEGS);
  $: dbLabel = lvl > 0.0001 ? `${Math.round(20 * Math.log10(lvl))} dB` : '−∞ dB';
</script>


<!-- LIVE — laid out to docs/relaydesign/relay-console-screen.png.
     Row A: PREVIEW · take rack · PROGRAM · OUTPUT STATUS
     Row B: 1 Live Transcript · 2 AI Detection · 3 Service Plan · 4 Quick Controls
     Everything below is a re-dressing of the controls that were already here — no
     command was added, removed or rewired. Where the reference draws a control
     Relay has no backend for (a transition rack, Fit/Safe-Area, Hold Outputs,
     Override Mode, ±5s audio scrub, a monitor bus), it is NOT drawn: a dead
     button in a live console is the exact failure this codebase keeps fixing. -->
<div class="con" class:compact class:fullscreen>
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

  <!-- ══════ ROW A — the pair, the rack, and the outputs ══════ -->
  <div class="con-top">
    <!-- PREVIEW — what the next TAKE would put on the wall. Amethyst, because it
         is by definition NOT on air; amber is reserved for the pane on the right. -->
    <section class="pane">
      <header class="mon-bar">
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
        <span class="mon-name">{previewLabel || 'Nothing cued'}</span>
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

    <!-- THE RACK. The reference's transition list (Cut / Fade / Wipe / Stinger /
         Duration) is not drawn — Relay has no transition engine, and drawing five
         buttons that do nothing would be inventing a feature. What is here is the
         real take path: the same accept/fire and the same nav the keys already run,
         plus the transport MODE, which is the one thing about `→` an operator must
         never have to guess (CLAUDE.md — same key, two meanings, is how the wrong
         thing reaches a congregation). -->
    <aside class="rack">
      <span class="rack-lbl">Take</span>
      <button
        class="take"
        on:click={take}
        disabled={!previewContent || !$capture.available}
        title="Put the previewed content on the outputs">TAKE</button>
      <div class="rack-nav">
        <button class="rk" title="Previous (←)" aria-label="Previous" on:click={() => step(-1)}>‹</button>
        <button class="rk" title="Next (→)" aria-label="Next" on:click={() => step(1)}>›</button>
      </div>
      <button class="rk wide" on:click={dismissTop} disabled={!dets.length}>Dismiss</button>
      <span class="rack-lbl push">Mode</span>
      <span
        class="rack-mode r-mono"
        class:slide={mode === 'slide'}
        title={mode === 'slide'
          ? 'Arrow keys step through the service plan'
          : 'Arrow keys walk through the passage on screen'}>
        {mode === 'slide' ? 'SLIDE' : 'VERSE'}
      </span>
    </aside>

    <!-- PROGRAM — literally what the congregation is looking at, rendered through
         the SAME TemplateRender as the real output window, so the pane cannot
         disagree with the wall. -->
    <section class="pane">
      <header class="mon-bar">
        {#if $rehearsing}
          <span class="tag reh">Rehearsal</span>
        {:else if $screenBlack}
          <span class="tag off">Blackout</span>
        {:else if $live}
          <span class="tag onair">Program · On Air</span>
        {:else}
          <span class="tag off">Program · Clear</span>
        {/if}
        <span class="spring"></span>
        <span class="mon-name">{$live ? ($live.reference || 'content') : '—'}</span>
      </header>
      <div class="screen" class:lit={$live && !$rehearsing && !$screenBlack}>
        {#if $live}
          <TemplateRender template={resolveOutputTemplate(previewTpl, $liveTemplateOverride, $liveTemplatePinned)} content={$liveContent} />
        {:else}
          <!-- Nothing is on the wall. Say so in words — a blank rectangle and a
               black-out look identical, and they are not the same fact. -->
          <div class="screen-empty">Screens clear</div>
        {/if}
        {#if $screenBlack}<div class="blk"></div>{/if}
      </div>
    </section>

    <!-- OUTPUT STATUS. Read-only on purpose: during a service the only question is
         "is it up?". Changing a target is the Outputs tab's job. -->
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
            <span class="out-t">
              <b>{o.c.name}</b>
              <!-- The screen's OWN last word, not ours. When it disagrees with the
                   badge, that disagreement is the finding. -->
              <span class="r-mono">{o.s.note || o.c.render_target}</span>
            </span>
            <span class="r-badge {SCREEN_BADGE[o.s.kind]} sm-badge"><span class="bd"></span>{o.s.label}</span>
          </div>
        {:else}
          <EmptyState message="No screens yet — add one in the Outputs tab." />
        {/each}
      </div>
      <p class="sr-only" aria-live="polite">{downAnnounce}</p>
      <footer class="pane-foot">
        <button class="wide" on:click={openMainOutput} disabled={!$capture.available}>Open main output</button>
      </footer>
    </section>
  </div>

  <!-- ══════ ROW B — the four working panels ══════ -->
  <div class="con-bot">
    <!-- ── 1 · LIVE TRANSCRIPT ── -->
    <section class="pane">
      <header class="pane-head">
        <span class="pn">1</span>
        <h2>Live Transcript</h2>
        <span class="spring"></span>
        <span class="chip" class:ok={$capture.capturing}>
          <i class="bd"></i>{$capture.stt.loaded ? 'STT Local' : 'No model'}
        </span>
      </header>

      <div class="pane-body tx" bind:this={transcriptEl}>
        {#if hasTranscript}
          {#each tLines as l, i (i)}
            <div class="txl" class:cur={i === tLines.length - 1 && !$transcript.partial}>
              <span class="txl-at r-mono">{l.at}</span>
              <span class="txl-b">{l.text}</span>
            </div>
          {/each}
          {#if $transcript.partial}
            <div class="txl cur">
              <span class="txl-at r-mono">now</span>
              <span class="txl-b"><mark>{$transcript.partial}</mark><i class="caret"></i></span>
            </div>
          {/if}
        {:else if $capture.capturing}
          <EmptyState message={$t('live.waiting_for_speech')} />
        {:else if !$capture.stt.loaded}
          <EmptyState message={$t('live.no_model')} />
        {:else}
          <EmptyState message={$t('live.start_listening_to_transcribe')} />
        {/if}
      </div>

      <footer class="pane-foot mic">
        <!-- The DETECTED language, not a chosen one. Code-switching is the normal
             case for the priority languages, so this changes mid-sermon. -->
        <span class="mic-lbl r-mono">{$capture.capturing ? ($capture.detectedLang ?? 'listening') : 'standby'}</span>
        <span class="meter" role="meter" aria-valuemin="0" aria-valuemax="100"
          aria-valuenow={Math.round(lvl * 100)} aria-label="Microphone input level">
          {#each SEG_ARR as _, i}
            <i class="sg" class:on={i < litSegs} class:mid={i >= 15 && i < 20} class:hot={i >= 20}></i>
          {/each}
        </span>
        <span class="r-mono db">{dbLabel}</span>
        <button class="ibtn" on:click={toggleListen} title={$capture.capturing ? 'Stop listening' : 'Start listening'}
          aria-label={$capture.capturing ? 'Stop listening' : 'Start listening'}
          disabled={!$capture.available || !$capture.stt.loaded || listenBusy}>
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v4"/></svg>
        </button>
      </footer>
    </section>

    <!-- ── 2 · AI DETECTION — CURRENT CLAIM ── -->
    <section class="pane">
      <header class="pane-head">
        <span class="pn">2</span>
        <h2>AI Detection — Current Claim</h2>
        <span class="spring"></span>
        <label class="sens" title="How readily the AI fires. Lower = fewer, surer catches; higher = more, noisier. Same dial as Settings.">
          <span class="sens-lbl r-mono">SENS</span>
          <input type="range" min="0" max="100" step="1" value={sensitivity}
            on:input={(e) => onSensitivity(+e.target.value)} disabled={!$capture.available}
            aria-label="Detection sensitivity" />
          <span class="sens-val r-mono">{sensitivity}</span>
        </label>
        <button class="chip btnchip" class:ok={$capture.detectionOn} on:click={toggleDetection}
          disabled={!$capture.available} title="Arm or disarm automatic detection">
          <i class="bd"></i>{$capture.detectionOn ? 'Armed' : 'Off'}
        </button>
      </header>

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
        {#if dets.length}
          {@const d = dets[0]}
          <!-- HEARD vs GUESSED. Not two flavours of one thing, and they must not look
               like it. A direct hit's number is a real parse confidence. A paraphrase
               is a TF-IDF cosine — a distance, NOT a probability (router.rs forbids it
               from ever auto-firing at ANY score). So the guess gets cyan, and gets no
               number at all: a number that lies is worse than no number. -->
          <div class="claim" class:guess={!heard(d)}>
            <div class="claim-top">
              <span class="claim-ref">{d.reference}</span>
              {#if heard(d)}
                <span class="mchip">{$t(methodKey(d))} {Math.round(d.confidence * 100)}%</span>
              {:else}
                <span class="mchip guess">{$t(methodKey(d))}</span>
              {/if}
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
              <!-- THE EVIDENCE — the words that actually triggered the match. Captured
                   in Rust for months and dropped at the IPC boundary. -->
              <div class="mt">
                <span class="klbl">{heard(d) ? $t('live.heard') : $t('live.matched_on')} (from transcript)</span>
                <p class="mt-q">“{d.matched_text}”</p>
              </div>
            {/if}

            {#if d.text}<p class="claim-verse">“{d.text}”</p>{/if}

            <div class="meta2">
              <div><span class="klbl">Method</span><b>{$t(methodKey(d))}</b></div>
              <div><span class="klbl">Reference</span><b>{d.reference}</b></div>
            </div>

            <div class="acts">
              <button class="act go" on:click={acceptTop}>
                <b>Accept &amp; fire</b><span>Send to outputs</span>
              </button>
              <button class="act no" on:click={dismissTop}>
                <b>Dismiss</b><span>Not this verse</span>
              </button>
            </div>

            <!-- WHY did it say that? The evidence chip above is the short answer;
                 this is the long one, including what accepting or dismissing does
                 to the gate. -->
            <button class="inspect-link" on:click={inspectTop}>
              Why this match?
            </button>
            <p class="khint"><kbd>A</kbd> accept · <kbd>D</kbd> dismiss</p>
          </div>
        {:else}
          <EmptyState
            message={$capture.detectionOn ? $t('live.no_suggestions') : $t('live.detection_off')} />
        {/if}

        <!-- OTHER PENDING CLAIMS. The reference calls this strip "Recent Claims";
             Relay's store holds only what is still AWAITING A DECISION (accepted and
             dismissed claims leave it), so it is labelled for what it actually is. -->
        {#if dets.length > 1}
          <div class="sub">
            <span class="klbl">Also pending</span>
            <span class="r-mono cnt">{dets.length - 1}</span>
          </div>
          {#each dets.slice(1) as x (x.reference + x.at)}
            <div class="rc">
              <span class="rc-ref">{x.reference}</span>
              <span class="mchip sm" class:guess={!heard(x)}>
                {$t(methodKey(x))}{#if heard(x)} {Math.round(x.confidence * 100)}%{/if}
              </span>
              <span class="spring"></span>
              <button class="mini" on:click={() => pushRef(x.reference)}>Fire</button>
              <button class="mini ghost" on:click={() => dismissDetection(x.reference)}>Dismiss</button>
            </div>
          {/each}
        {/if}

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
           floor. -->
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

    <!-- ── 3 · SERVICE PLAN — RUNNING ── -->
    <section class="pane">
      <header class="pane-head">
        <span class="pn">3</span>
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
            {@const ty = TYPE[c.cue_type] || TYPE.scripture}
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
                  <span class="cue-title">{c.label}</span>
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
                <span class="cue-title">{p.title}</span>
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

    <!-- ── 4 · QUICK CONTROLS ── -->
    <section class="pane">
      <header class="pane-head">
        <span class="pn">4</span>
        <h2>Quick Controls</h2>
      </header>

      <div class="pane-body quick">
        <div class="q4">
          <button class="qb red" on:click={clearAll} disabled={!$capture.available}>
            <b>Clear screens</b><span>Stop all outputs · Esc</span>
          </button>
          <button class="qb grey" class:on={$screenBlack} on:click={blackAll} disabled={!$capture.available}>
            <b>Blackout</b><span>Go to black · B</span>
          </button>
          <button class="qb amethyst" class:on={$rehearsing} on:click={toggleRehearsal}
            disabled={!$capture.available || rehBusy}>
            <b>{$rehearsing ? 'Rehearsing' : 'Rehearse'}</b>
            <span>{$rehearsing ? 'Go live' : 'Nothing goes live'}</span>
          </button>
          <button class="qb cyan" class:on={$capture.detectionOn} on:click={toggleDetection}
            disabled={!$capture.available}>
            <b>Detection {$capture.detectionOn ? 'on' : 'off'}</b><span>AI listening</span>
          </button>
        </div>

        <!-- TRANSPORT MODE is DERIVED from what is on the wall, never a switch the
             operator has to remember to set — so these read out, they do not choose. -->
        <span class="klbl sec">Transport mode</span>
        <div class="modes" role="status" aria-label="Transport mode">
          <span class="md" class:on={mode === 'verse'}><i></i>Verse mode <em>(step verses)</em></span>
          <span class="md" class:on={mode === 'slide'}><i></i>Slide mode <em>(step plan slides)</em></span>
        </div>

        <span class="klbl sec">Step controls</span>
        <div class="q4 tight">
          <button class="sb" on:click={() => step(-1)}><span>Previous</span><i>←</i></button>
          <button class="sb" on:click={() => step(1)}><span>Next</span><i>→</i></button>
        </div>
        <!-- Countdown gets its own row: it carries an input and a two-step arm, so it
             does not fit a half-width cell without truncating its own name. -->
        <div class="sb cd">
          <span>Countdown</span>
          <input class="cd-min r-mono" type="number" min="1" max="120" bind:value={cdMin}
            aria-label="Countdown minutes" disabled={!$capture.available} />
          <span class="cd-unit r-mono">min</span>
          <button class="cd-go" class:armed={cdArmed} on:click={beginCountdown} disabled={!$capture.available}>
            {cdArmed ? 'Confirm?' : 'Start'}
          </button>
        </div>

        <!-- Emergency announcement. Same row shape as the countdown, and armed the
             same way — this one goes over live scripture on every screen at once. -->
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

        <span class="klbl sec">Audio monitor</span>
        <div class="amon">
          <span class="amon-k">Input level</span>
          <span class="meter" aria-hidden="true">
            {#each SEG_ARR as _, i}
              <i class="sg" class:on={i < litSegs} class:mid={i >= 15 && i < 20} class:hot={i >= 20}></i>
            {/each}
          </span>
          <span class="r-mono db">{dbLabel}</span>
        </div>
        <button class="wide" on:click={toggleListen}
          disabled={!$capture.available || !$capture.stt.loaded || listenBusy}>
          {$capture.capturing ? 'Stop listening' : listenBusy ? 'Starting…' : 'Start listening'}
        </button>
      </div>
    </section>
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

  <!-- No STT model = the AI cannot listen. Relay degrades to a fully working MANUAL
       tool, never a dead one — and it can fix itself in one click. -->
  {#if $capture.available && !$capture.stt.loaded}
    <ModelSetup compact />
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
  /* LIVE — laid out to docs/relaydesign/relay-console-screen.png, styled entirely
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
  /* Full screen has already reclaimed the chrome, so the exit affordance sits
     where the view controls would be. Keep clear of it rather than under it. */
  .con.fullscreen .view-ctl{ padding-right:132px; }
  .con.compact :global(.pane){ border-radius:10px; }
  .con.compact :global(.pane-head){ padding:8px 11px; }
  .con.compact :global(.pane-head h2){ font-size:11px; }
  .con.compact :global(.pane-body){ padding:10px 11px; }
  .con.compact .view-ctl{ margin-bottom:7px; }

  .con{
    height:100%; min-height:0; display:flex; flex-direction:column;
    gap:var(--v-sp-sm); color:var(--v-txt); font-family:var(--f-body);
  }
  .spring{flex:1}
  .cnt{font-size:var(--v-fs-cap); color:var(--v-faint)}

  /* ── rehearsal band ── amethyst, never amber. Amber means ON AIR. */
  .reh{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    padding:10px var(--v-sp-md); border-radius:var(--v-r-lg);
    background:var(--v-amethyst-soft); border:1px solid rgba(139,92,246,.42);
    font-size:var(--v-fs-b2); line-height:var(--v-lh-b2); color:var(--v-dim)}
  .reh b{font-family:var(--f-mono); font-size:var(--v-fs-cap); font-weight:700;
    letter-spacing:.14em; color:var(--v-amethyst); flex:0 0 auto}
  .reh span:not(.reh-dot){flex:1}
  .reh-dot{width:8px; height:8px; border-radius:50%; flex:0 0 auto; background:var(--v-amethyst);
    box-shadow:0 0 9px var(--v-amethyst); animation:pulse 1.7s ease-in-out infinite}
  .reh-end{flex:0 0 auto; padding:7px 14px; border-radius:var(--v-r-md); cursor:pointer;
    font-family:var(--f-body); font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.06em;
    text-transform:uppercase; background:var(--v-amethyst); border:0; color:var(--v-void)}
  .reh-end:disabled{opacity:.5; cursor:not-allowed}
  @keyframes pulse{0%,100%{opacity:1}50%{opacity:.4}}

  /* ── the two rows ──────────────────────────────────────────────────────── */
  .con-top{flex:0 0 auto; height:clamp(268px,33vh,364px);
    display:grid; grid-template-columns:1.19fr 92px 1fr 300px; gap:var(--v-sp-sm); min-height:0}
  .con-bot{flex:1; min-height:0;
    display:grid; grid-template-columns:1fr 1.21fr 1fr 300px; gap:var(--v-sp-sm)}

  .pane{display:flex; flex-direction:column; min-height:0; overflow:hidden;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg);
    box-shadow:var(--v-shadow-sm)}

  .pane-head{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
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
  .tag.preview{background:var(--v-amethyst); color:var(--v-void)}
  /* Amber, and only when the congregation is genuinely looking at it. */
  .tag.onair{background:var(--v-amber); color:var(--v-amber-ink)}
  .tag.reh{background:var(--v-amethyst-soft); border:1px solid rgba(139,92,246,.45); color:var(--v-amethyst)}
  .tag.off{background:var(--v-grey-soft); border:1px solid var(--v-line2); color:var(--v-dim)}
  .mon-name{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:var(--v-fs-cap); color:var(--v-faint)}
  /* Slate, not amber and not rose: a repeat is a fact, not an alarm. Amber means
     ON AIR and must never be spent on anything else (DECISIONS §22). */
  .mon-repeat{margin-right:8px; padding:1px 6px; border-radius:var(--v-r-sm);
    font-size:10px; letter-spacing:.06em; color:var(--v-faint);
    border:1px solid var(--v-line2)}
  .screen{flex:1; min-height:0; position:relative; overflow:hidden; background:#000;
    border-top:1px solid var(--v-line)}
  .screen.lit{box-shadow:inset 0 0 0 2px var(--v-amber)}
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
  .take{height:44px; border-radius:var(--v-r-md); border:0; cursor:pointer;
    background:var(--v-amber); color:var(--v-amber-ink); font-family:var(--f-body);
    font-size:var(--v-fs-lbl); font-weight:700; letter-spacing:.1em;
    box-shadow:0 6px 18px -6px var(--v-amber-glow); transition:filter .14s}
  .take:hover:not(:disabled){filter:brightness(1.06)}
  .take:disabled{opacity:.4; cursor:not-allowed; box-shadow:none}
  .rack-nav{display:grid; grid-template-columns:1fr 1fr; gap:6px}
  .rk{height:30px; border-radius:var(--v-r-md); cursor:pointer; background:var(--v-surf2);
    border:1px solid var(--v-line2); color:var(--v-dim); font-family:var(--f-body);
    font-size:var(--v-fs-cap); transition:.14s}
  .rk:hover:not(:disabled){background:var(--v-surf3); color:var(--v-txt)}
  .rk:disabled{opacity:.4; cursor:not-allowed}
  .rk.wide{width:100%}
  /* A small top gap before the MODE group; NOT margin-top:auto — with the rack no
     longer stretched there is no free space to push into, and auto once let the
     chip swallow it. */
  .rack-lbl.push{margin-top:4px}
  /* A compact, content-width pill — inline-flex + align-self:center so it hugs its
     text and centres under the MODE label, and a fixed height so it can never
     stretch no matter what the flex context does. */
  .rack-mode{flex:0 0 auto; align-self:center; height:30px; display:inline-flex; align-items:center; justify-content:center;
    padding:0 14px; border-radius:999px;
    background:var(--v-surf2); border:1px solid var(--v-line2);
    font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.1em; color:var(--v-cyan)}
  .rack-mode.slide{color:var(--v-amber)}

  /* ── output status ─────────────────────────────────────────────────────── */
  .out{display:flex; align-items:center; gap:10px; padding:9px 10px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line)}
  .out-ic{flex:0 0 auto; width:28px; height:28px; border-radius:var(--v-r-sm); display:grid;
    place-items:center; background:var(--v-surf3); color:var(--v-dim)}
  .out-t{flex:1; min-width:0; display:flex; flex-direction:column; gap:2px}
  .out-t b{font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
  .out-t span{font-size:9px; letter-spacing:.05em; color:var(--v-faint)}
  /* A screen that is not answering is a FAILURE, and the row says so without
     spending amber (which means on air, DECISIONS §22) or reading as decoration.
     The border is the signal; the badge carries the word. */
  .out.down{border-color:color-mix(in srgb, var(--v-rose) 45%, transparent);
    background:color-mix(in srgb, var(--v-rose) 7%, var(--v-surf2))}
  .out.down .out-ic{color:var(--v-rose)}
  .out.down .out-t span{color:var(--v-rose)}
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

  /* ── 2 · detection ─────────────────────────────────────────────────────── */
  .chip{display:inline-flex; align-items:center; gap:6px; flex:0 0 auto; padding:4px 9px;
    border-radius:99px; background:var(--v-surf2); border:1px solid var(--v-line2);
    font-size:var(--v-fs-cap); color:var(--v-faint)}
  .chip .bd{width:6px; height:6px; border-radius:50%; background:var(--v-faint)}
  .chip.ok{color:var(--v-emerald); border-color:rgba(34,197,94,.32); background:var(--v-emerald-soft)}
  .chip.ok .bd{background:var(--v-emerald); box-shadow:0 0 6px var(--v-emerald)}
  .btnchip{cursor:pointer; font-family:var(--f-body)}
  .btnchip:disabled{opacity:.5; cursor:not-allowed}

  /* Sensitivity dial — compact, on the run surface. Reaches the same thresholds
     as Settings; the value is amethyst (chrome), never amber. */
  .sens{display:inline-flex; align-items:center; gap:7px; flex:0 0 auto;}
  .sens-lbl{font-size:var(--v-fs-cap); letter-spacing:.08em; color:var(--v-faint);}
  .sens-val{font-size:var(--v-fs-cap); color:var(--v-dim); min-width:20px; text-align:right;}
  .sens input[type="range"]{-webkit-appearance:none; appearance:none; width:88px; height:4px;
    border-radius:99px; background:var(--v-surf3); cursor:pointer; outline:none;}
  .sens input[type="range"]:focus-visible{box-shadow:0 0 0 3px var(--v-accent-soft);}
  .sens input[type="range"]:disabled{opacity:.5; cursor:not-allowed;}
  .sens input[type="range"]::-webkit-slider-thumb{-webkit-appearance:none; appearance:none;
    width:13px; height:13px; border-radius:50%; background:var(--v-accent);
    border:2px solid var(--v-surf); box-shadow:var(--v-shadow-sm);}
  .sens input[type="range"]::-moz-range-thumb{width:13px; height:13px; border-radius:50%;
    background:var(--v-accent); border:2px solid var(--v-surf);}

  .claim{background:var(--v-surf2); border:1px solid rgba(255,176,0,.28);
    border-radius:var(--v-r-lg); padding:14px; box-shadow:0 0 20px -6px var(--v-amber-glow)}
  /* A GUESS MUST LOOK LIKE A GUESS. Amber reads as "Relay is confident" and a
     paraphrase has not earned it — its score is a cosine, and router.rs will not let
     it auto-fire at ANY value. Cyan, NOT amethyst: amethyst already means REHEARSAL,
     and a colour that means "nothing is reaching the congregation" cannot also mean
     "this guess is shaky", or on the day both are true the operator reads the wrong one. */
  .claim.guess{border-color:var(--v-cyan-soft); box-shadow:none}
  .claim-top{display:flex; align-items:center; justify-content:space-between; gap:var(--v-sp-sm)}
  .claim-ref{font-family:var(--f-head); font-size:var(--v-fs-h1); line-height:var(--v-lh-h1);
    font-weight:600; letter-spacing:var(--v-tr-tight); color:var(--v-txt)}
  .mchip{flex:0 0 auto; padding:4px 10px; border-radius:99px; font-family:var(--f-mono);
    font-size:var(--v-fs-cap); font-weight:600; background:var(--v-amber-soft);
    border:1px solid rgba(255,176,0,.32); color:var(--v-amber)}
  .mchip.guess{background:var(--v-cyan-soft); border-color:rgba(34,211,238,.32); color:var(--v-cyan)}
  .mchip.sm{padding:3px 8px; font-size:10px}
  /* Confidence as a BAR — "0.92" means nothing to a volunteer. Only ever drawn for a
     heard reference, the only one whose number means what it appears to mean. */
  .conf{height:3px; border-radius:2px; background:var(--v-surf3); margin:10px 0 0; overflow:hidden}
  .conf i{display:block; height:100%; background:var(--v-amber); border-radius:2px}
  .guess-note{margin:8px 0 0; font-size:var(--v-fs-cap); color:var(--v-cyan)}
  .klbl{font-family:var(--f-mono); font-size:9px; font-weight:700; letter-spacing:.14em;
    text-transform:uppercase; color:var(--v-faint)}
  .klbl.sec{margin-top:var(--v-sp-sm)}
  .mt{margin-top:12px}
  .mt-q{margin:5px 0 0; font-size:var(--v-fs-b1); line-height:1.55; color:var(--v-txt)}
  .claim-verse{margin:10px 0 0; font-family:var(--f-serif); font-style:italic;
    font-size:var(--v-fs-b2); line-height:1.55; color:var(--v-dim)}
  .meta2{display:grid; grid-template-columns:1fr 1fr; gap:10px; margin-top:14px;
    padding-top:12px; border-top:1px solid var(--v-line)}
  .meta2 b{display:block; margin-top:3px; font-size:var(--v-fs-b2); font-weight:500; color:var(--v-txt)}
  .acts{display:grid; grid-template-columns:1fr 1fr; gap:var(--v-sp-sm); margin-top:14px}
  .act{display:flex; flex-direction:column; gap:2px; align-items:center; padding:9px 10px;
    border-radius:var(--v-r-md); cursor:pointer; border:1px solid transparent;
    font-family:var(--f-body); transition:filter .14s}
  .act b{font-size:var(--v-fs-b2); font-weight:600}
  .act span{font-size:10px; opacity:.8}
  .act:hover{filter:brightness(1.08)}
  .act.go{background:var(--v-emerald); color:var(--v-void)}
  .act.no{background:var(--v-red); color:#fff}
  .khint{margin:10px 0 0; text-align:center; font-size:10px; color:var(--v-faint)}
  .khint kbd{font-family:var(--f-mono); font-size:9px; color:var(--v-dim);
    background:var(--v-surf3); border:1px solid var(--v-line2); border-radius:var(--v-r-sm); padding:2px 5px}

  .sub{display:flex; align-items:center; gap:var(--v-sp-sm); margin-top:var(--v-sp-sm);
    padding-top:var(--v-sp-sm); border-top:1px solid var(--v-line)}
  .rc{display:flex; align-items:center; gap:var(--v-sp-sm); padding:7px 10px;
    border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line)}
  .rc-ref{font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt)}
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
  .search:focus-within{border-color:rgba(255,176,0,.45); box-shadow:0 0 0 3px rgba(255,176,0,.1)}
  .err{padding:0 12px 10px; color:var(--v-red); font-size:var(--v-fs-cap)}

  .wide{width:100%; height:32px; border-radius:var(--v-r-md); cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-txt);
    font-family:var(--f-body); font-size:var(--v-fs-lbl); font-weight:600; transition:.14s}
  .wide:hover:not(:disabled){background:var(--v-surf3)}
  .wide:disabled{opacity:.45; cursor:not-allowed}
  .wide.amber{flex:0 0 auto; width:auto; padding:0 18px; background:var(--v-amber);
    border-color:transparent; color:var(--v-amber-ink)}

  /* ── 3 · plan ──────────────────────────────────────────────────────────── */
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
  .cue.sel{border-color:rgba(34,211,238,.45)}
  /* Amber = it is in front of the congregation. Nothing else may use it. */
  .cue.islive,.slide.islive{border-color:var(--v-amber); background:var(--v-amber-soft)}
  /* CUED = where → will resume from, but NOT on screen. Deliberately not amber. */
  .cue.cued,.slide.cued{border-style:dashed; border-color:var(--v-grey)}
  .cue-stripe,.slide-stripe{width:3px; align-self:stretch; border-radius:99px; flex:0 0 auto}
  .slide-stripe{background:var(--acc)}
  .cue-num{flex:0 0 auto; font-size:10px; color:var(--v-faint)}
  .cue-body{flex:1; min-width:0; display:flex; flex-direction:column; gap:2px}
  .cue-title{font-size:var(--v-fs-b2); overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
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
    border:1px solid rgba(139,92,246,.3); color:var(--v-amethyst);
    font-size:var(--v-fs-cap); line-height:1.5}
  .note svg{flex:0 0 auto; margin-top:2px}
  .flash{display:flex; align-items:center; gap:8px; min-width:0; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap; font-size:var(--v-fs-cap); color:var(--v-emerald)}
  .flash.idle{color:var(--v-faint)}
  .fd{width:6px; height:6px; border-radius:50%; background:var(--v-emerald);
    box-shadow:0 0 8px var(--v-emerald); flex:0 0 auto}

  /* ── 4 · quick controls ────────────────────────────────────────────────── */
  .quick{gap:5px}
  .q4{display:grid; grid-template-columns:minmax(0,1fr) minmax(0,1fr); gap:var(--v-sp-sm)}
  .q4.tight{gap:6px}
  .qb{display:flex; flex-direction:column; gap:2px; align-items:flex-start; text-align:left;
    padding:10px 11px; border-radius:var(--v-r-md); cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-txt);
    font-family:var(--f-body); transition:.14s}
  .qb{min-width:0; padding:8px 10px}
  .qb b{font-size:var(--v-fs-b2); font-weight:600; max-width:100%;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
  .qb span{font-size:10px; color:var(--v-faint); max-width:100%;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
  .qb:disabled{opacity:.45; cursor:not-allowed}
  .qb.red{background:var(--v-red-soft); border-color:rgba(239,68,68,.4); color:var(--v-red)}
  .qb.grey.on{background:var(--v-grey-soft); border-color:var(--v-grey); color:var(--v-txt)}
  .qb.amethyst.on{background:var(--v-amethyst-soft); border-color:rgba(139,92,246,.45); color:var(--v-amethyst)}
  .qb.cyan.on{background:var(--v-cyan-soft); border-color:rgba(34,211,238,.4); color:var(--v-cyan)}
  .qb:hover:not(:disabled){filter:brightness(1.12)}

  .modes{display:flex; flex-direction:column; gap:5px}
  .md{display:flex; align-items:center; gap:9px; padding:6px 10px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line);
    font-size:var(--v-fs-b2); color:var(--v-faint)}
  .md i{width:11px; height:11px; border-radius:50%; flex:0 0 auto;
    border:1px solid var(--v-line2); background:transparent}
  .md em{font-style:normal; font-size:10px; opacity:.8}
  .md.on{background:var(--v-amethyst-soft); border-color:rgba(139,92,246,.45); color:var(--v-txt)}
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
  .sb.cd>span:first-child{flex:1}
  .cd-unit{flex:0 0 auto; font-size:9px; color:var(--v-faint)}
  .cd-min{width:40px; padding:3px 5px; border-radius:var(--v-r-sm); border:1px solid var(--v-line2);
    background:var(--v-bg); color:var(--v-txt); font-size:var(--v-fs-cap); text-align:center}
  /* The announcement field takes the row's spare width — the message, not the
     label, is the part the operator is reading back before they confirm. */
  .cd-msg{flex:1 1 auto; min-width:0; padding:3px 7px; border-radius:var(--v-r-sm);
    border:1px solid var(--v-line2); background:var(--v-bg); color:var(--v-txt);
    font-size:var(--v-fs-cap)}
  .cd-go{padding:4px 9px; border-radius:var(--v-r-sm); border:1px solid rgba(34,211,238,.4);
    background:var(--v-cyan-soft); color:var(--v-cyan); font-family:var(--f-mono);
    font-size:10px; font-weight:700; cursor:pointer}
  .cd-go:hover:not(:disabled){filter:brightness(1.2)}
  .cd-go:disabled{opacity:.45; cursor:not-allowed}
  .cd-go.armed{background:var(--v-amber-soft); border-color:rgba(255,176,0,.5); color:var(--v-amber)}

  .amon{display:flex; align-items:center; gap:var(--v-sp-sm); padding:7px 10px;
    border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line)}
  .amon-k{flex:0 0 auto; font-size:var(--v-fs-cap); color:var(--v-faint)}

  /* ── banners ───────────────────────────────────────────────────────────── */
  .audioerr{flex:0 0 auto; background:var(--v-red-soft); color:var(--v-red);
    border:1px solid rgba(239,68,68,.3); border-radius:var(--v-r-md);
    padding:9px 12px; font-size:var(--v-fs-lbl)}
  /* Degraded, not broken: amber (a warning), never red (an error) — the app is still
     fully usable by hand, and the banner should read that way. */
  .sttwarn{flex:0 0 auto; background:var(--v-amber-soft); color:var(--v-txt);
    border:1px solid rgba(255,176,0,.34); border-radius:var(--v-r-md);
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
    .con-top{grid-template-columns:1.1fr 84px 1fr 250px}
    .con-bot{grid-template-columns:1fr 1.2fr 1fr 250px}
  }
  @media (max-width:1180px){
    .con{height:auto}
    .con-top{height:auto; grid-template-columns:1fr 84px 1fr; grid-auto-rows:minmax(230px,auto)}
    .con-bot{grid-template-columns:1fr 1fr; grid-auto-rows:minmax(320px,auto)}
  }
  @media (max-width:760px){
    .con-top{grid-template-columns:1fr}
    .con-bot{grid-template-columns:1fr}
    .rack{flex-direction:row; align-items:center; flex-wrap:wrap}
    .rack-mode{margin-top:0; flex:1}
  }
</style>
