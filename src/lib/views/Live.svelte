<script context="module">
  /**
   * `2026-09-14` → `14 Sep`, for the one place a plan's date is a HEAD and not a
   * record (L2, docs/REBRAND.md §2 — the prototype's `SLIDES · SUNDAY MORNING ·
   * 14 SEP`).
   *
   * The year is dropped because this head names the plan an operator is running
   * RIGHT NOW; a full `2026-09-14` beside the plan's own name reads as a record
   * id and eats the width the name needs. It is not a second `planDateLabel`:
   * that one guards an ABSENCE and belongs to the plan rail, where a plan from
   * another year is a real thing you might be looking at.
   *
   * ANYTHING THAT IS NOT A PLAIN ISO DATE COMES BACK VERBATIM. A hand-edited row
   * or an import can hold whatever a person typed, and a formatter that quietly
   * reinterpreted it would print a date nobody entered — worse, on a run surface,
   * than printing the odd string as it stands. A month outside 1–12 is the same
   * case and gets the same answer.
   *
   * `new Date()` IS DELIBERATELY NOT USED. It reads a bare ISO date as UTC
   * midnight and then renders it in local time, so west of Greenwich every plan
   * on this head would be dated the day before its own.
   *
   * At module scope so it can be tested as the arithmetic it is, rather than only
   * through a mounted plan.
   */
  const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
  const FULL_MONTHS = [
    'January', 'February', 'March', 'April', 'May', 'June',
    'July', 'August', 'September', 'October', 'November', 'December',
  ];
  export function shortDate(value) {
    const s = typeof value === 'string' ? value.trim() : '';
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
    if (!m) return s;
    const month = MONTHS[Number(m[2]) - 1];
    if (!month) return s;
    return `${Number(m[3])} ${month}`;
  }

  /**
   * DOES THE PLAN'S OWN NAME ALREADY SAY THIS DATE?
   *
   * Measured on a render: the head read `SLIDES · SUNDAY MORNING · 7 SEPTEMBER ·
   * 7 SEP` — the plan's title carries the date, and the head then printed it
   * again in a second form. One date said twice in two notations is worse than
   * either alone: it reads as two dates until you stop and compare them, on the
   * head an operator glances at mid-service.
   *
   * The date CAP is not dropped outright, because a plan named `Sunday morning`
   * with no date in it needs one — which is exactly the prototype's `SLIDES ·
   * SUNDAY MORNING · 14 SEP`. It is dropped only when the name has already said
   * it, and the test for that is containment of a form this app would print or a
   * person would type: `14 Sep`, `14 September`, `14th September`, `2026-09-14`.
   *
   * A NAME THAT MERELY MENTIONS A MONTH IS NOT A MATCH. `September series` beside
   * a plan dated the 14th is two different facts, and the day number is what
   * keeps them apart — so the day is always required, never the month alone.
   *
   * @param {string} title the plan's own name
   * @param {string} isoDate its `plan_date`, as the backend stores it
   */
  /**
   * WHAT A CUED PLAN SLIDE IS CALLED — a cue's name and its slide's, joined ONCE.
   *
   * This was `` `${cue} · ${slide}` ``, and a one-slide cue names its only slide
   * after itself: measured on a render, the preview head read `WELCOME & NOTICES
   * · WELCOME & NOTICES`. A separator between a thing and itself invents a second
   * fact out of one, on the pane that says what is about to go on a wall.
   *
   * Both names are kept when they are two names — `Amazing Grace · Verse 2` is
   * the one an operator needs — and the comparison is loose about case and
   * surrounding space only, never about the words themselves.
   */
  export function cueLabel(cue, slide) {
    const a = String(cue ?? '').trim();
    const b = String(slide ?? '').trim();
    if (!b || a.toLowerCase() === b.toLowerCase()) return a;
    if (!a) return b;
    return `${a} · ${b}`;
  }

  export function dateStatedIn(title, isoDate) {
    const t = typeof title === 'string' ? title.toLowerCase() : '';
    const iso = typeof isoDate === 'string' ? isoDate.trim() : '';
    if (!t || !iso) return false;
    const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
    if (!m) return t.includes(iso.toLowerCase());
    const idx = Number(m[2]) - 1;
    const short = MONTHS[idx];
    const full = FULL_MONTHS[idx];
    if (!short) return t.includes(iso.toLowerCase());
    const day = Number(m[3]);
    const days = [String(day), `${day}st`, `${day}nd`, `${day}rd`, `${day}th`, m[3]];
    for (const d of days) {
      for (const mon of [short, full]) {
        if (t.includes(`${d} ${mon}`.toLowerCase())) return true;
        if (t.includes(`${mon} ${d}`.toLowerCase())) return true;
      }
    }
    return t.includes(iso.toLowerCase());
  }
</script>

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
  import { describeScreen } from '../outputHealth.js';
  import TemplateRender from '../TemplateRender.svelte';
  import { resolveOutputTemplate, isKeyedTemplate } from '../layers.js';
  import ModelSetup from '../ModelSetup.svelte';
  import { registerContext } from '../shortcuts.js';
  import { t } from '../i18n.js';
  import CameraPlate from '../ui/CameraPlate.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import ErrorState from '../ui/ErrorState.svelte';
  import Loading from '../ui/Loading.svelte';
  import { heard, methodBadgeKey, methodNoteKey, inLibrary } from '../detect.js';
  import DetectionInspector from '../DetectionInspector.svelte';
  import { humanError as humanErrorBase } from '../errors.js';
  import { typeOf, payloadOf, slidesOf, slideAccent, cueSub, nextOf, stepFrom } from '../plan.js';
  import { gridSource, pressArbiter } from '../slidegrid.js';
  import { reflow } from '../reflow.js';
  import LiveRail from '../LiveRail.svelte';
  import { parsePassage } from '../passage.js';
  import { session, setSession } from '../session.js';
  // X1 · the transition register and the override's store (docs/REBRAND.md §8).
  // The picker moved here from the chrome bar (L4); the ranking did not move —
  // `resolveTransition` is still the ONE place the two authorities are ordered,
  // and it is still called from `TemplateRender` and nowhere else.
  import { TRANSITIONS, TRANSITION_MS, DEFAULT_TRANSITION_MS, liveTransition } from '../transitions.js';
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
    startCountdown,
    setLiveTransition,
    startCapture,
    stopCapture,
    relatedScripture,
    navVerse,
    navNotice,
    navBlocked,
    listPlans,
    planItems,
    getSong,
    setStageNext,
    rehearsing,
    loadRehearsal,
    setRehearsal,
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
  // ONE resolution, read by the render AND by the branch that decides whether
  // there is anything to render with — two calls could disagree.
  $: progTpl = resolveOutputTemplate(previewTpl, $liveTemplateOverride, $liveTemplatePinned);
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
    railSong = null;
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
    railSong = null;
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
  }));

  // ── SWITCHING A SCREEN ON OR OFF IS NOT THIS SURFACE'S JOB ──────────────────
  //
  // It was, while Live carried an Output Status pane. The pane has gone (§2 gives
  // this column to the AI's claims alone) and with it the per-screen on/off
  // repair, which now lives where the rest of a screen's settings do: Outputs →
  // Screens, one row per screen, Open and Close. Nothing became unreachable —
  // that is the distinction `scripts/qa-inventory.mjs` exists to police, and it
  // is the reason the repair MOVED rather than being deleted.
  //
  // What did not move is the pair of whole-room facts below. A lamp answers for
  // one screen; neither of these is about one screen.

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
  // TWO DIFFERENT FAILURES, AND THE WORSE ONE USED TO BE SILENT.
  //
  //  · SHRUNK — it all fits, but small. "You may not be able to read this from
  //    the back." Rule 37's original case, and `scale` is the honest measure.
  //  · CLIPPED — it does NOT fit, at the size it settled on, inside a box that is
  //    `overflow:hidden`. Words the congregation will simply never see. This
  //    could happen at `scale: 1.0` — nothing had to shrink — so `legible` was
  //    true and this pane said nothing at all (rule 35). It is checked first,
  //    because a template that is cutting words off is not improved by being told
  //    how big its type is.
  function noteFit(f) {
    if (f.clipped) {
      fitWarning =
        'This template is cutting words off — they do not fit the box, ' +
        (f.legible ? '' : `even shrunk to ${Math.round(f.scale * 100)}% of the template's size, `) +
        'so part of the text is not on the screen. Try a shorter passage or a template with more room.';
      return;
    }
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
      search: () => railEl?.focus(),
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
    // Only now: everything above IS the restore, and a watcher armed before it
    // would race the mount for the same plan.
    if (!dead) watchChosenPlan = true;
  });

  // ── LOAD WHOLE PLAN, FROM QUICK TOOLS ──────────────────────────────────────
  //
  // The dock's header button re-stages the plan the Planner handed over, and the
  // dock is in the shell — so when Live is ALREADY mounted the tab does not
  // change and nothing remounts. This is the half that answers it: a plan the
  // session names and this view does not have open gets loaded, once.
  //
  // It cannot loop against the reactive `setSession` above, because that writes
  // `openPlan.id` — the very value this refuses to act on. And it is armed only
  // after the mount's own restore, so the two never race for the same plan.
  //
  // IT CHANGES NO PROGRAMME. `loadPlan` moves the playhead and the grid; what a
  // congregation is looking at is `$live`, which nothing here touches. That is
  // the rule `plannerbuildonly.test.js` and `liveacceptance.test.js` hold.
  let watchChosenPlan = false;
  $: if (watchChosenPlan && $session.planId != null && $session.planId !== openPlan?.id) {
    const chosen = plans.find((p) => p.id === $session.planId);
    if (chosen) loadPlan(chosen);
  }
  let unregisterKeys;
  let unsubNav;
  let unsubLive;
  onDestroy(() => {
    // FIRST, so anything the async mount is still holding stops before it writes.
    dead = true;
    unregisterKeys?.();
    unsubNav?.();
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

  // THE EMERGENCY ANNOUNCEMENT moved to Quick tools (docs/REBRAND.md §2 — the
  // things that change during a service), which is in the dock row and therefore
  // one reach away on EVERY workspace rather than on this one. It paints over
  // live scripture on every screen at once; a control like that being reachable
  // only from the tab you happen to be on was the argument for moving it, not
  // against. Its two-step arm, and the `pushAnnouncement` contract that makes a
  // failure loud, moved with it unchanged.

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

  // ARMING THE AI IS THE DOCK'S CONTROL, AND ONLY THE DOCK'S (L4).
  //
  // This panel carried an `Armed` chip that called `setDetection`, one row below
  // the dock's ARMED switch, which calls the same command about the same gate.
  // Two controls for one setting is two places for them to disagree — the same
  // shape as the two status badges rule 35 was written for, and the dock already
  // holds the ruling (`shellchrome.test.js`: "detection is ONE switch, in the
  // card about the signal"). The dock's survives because it is reachable from
  // Templates and Settings too, which is where an operator who has stopped
  // trusting the AI actually is.
  //
  // THE ARM STATE IS STILL ON THIS SURFACE, as `gateState` on the panel head —
  // and it says more than a chip could, because it distinguishes disarmed from a
  // dead model from a stopped microphone. A state line is not a lost control.

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
  //
  // THE BOX MOVED; THE PATH DID NOT. §9 asks for one search box and puts it in
  // the rail, so the inspector's second scripture field went there rather than
  // being deleted — it is still the floor under the AI (the model is missing,
  // the plan has run out, the reference is a RANGE the corpus cannot offer as a
  // single hit). `fireReference` is what the rail's `Fire` calls, and it is the
  // same `manualFire` a grid verse cell takes: recorded `'manual'` (rule 14),
  // through the pre-air validator (rule 36), reporting its own outcome (rule 15).
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
  async function fireReference(ref) {
    const text = String(ref ?? '').trim();
    if (!text) return;
    // Held only so `humanError` can quote back what the operator typed.
    manualRef = text;
    try {
      await manualFire(text);
      flash($t('live.now_live', { reference: text }));
      errMsg = '';
      manualRef = '';
    } catch (e) {
      errMsg = humanError(e);
    }
  }

  // ── THE TRANSITION OVERRIDE (docs/REBRAND.md §8 · DECISIONS §84) ──────────
  //
  // MOVED HERE FROM THE CHROME BAR (L4), at the operator's instruction, and the
  // move is the whole change: `resolveTransition` still ranks the two
  // authorities, `TemplateRender` still keys the replay on the override, both
  // panic controls are still outside it, and `loadLiveTransition()` is still
  // called once in the shell's mount so a console reopened mid-service does not
  // draw a picker that disagrees with screens that are already crossfading.
  //
  // WHY THE RACK IS THE RIGHT HOME. The chrome bar carries facts about the room
  // and one panic control; how a slide replaces the last is neither. It is a
  // property of the TAKE, so it belongs beside the take — an operator deciding
  // "make everything cut, now" is already looking at this column.
  //
  // `FOLLOW` is a sentinel for "no override", not a transition — an eighth entry
  // in `TRANSITIONS` would have been a second register, and one of the two would
  // eventually have been the one somebody read.
  const FOLLOW = '';

  // A CHANGE THAT DID NOT REACH THE SCREENS PUTS THE CONTROL BACK.
  //
  // `setLiveTransition` throws (group 1 in capture.js): a congregation can see the
  // difference between a cut and an 800 ms crossfade. The honest report is the
  // picker refusing to move — the store is only written after the backend has
  // taken the change, and the `value=` binding then redraws the select from the
  // store. A picker that stayed on "Crossfade" over screens that were cutting
  // would be rule 35 with a dropdown. The rack has room for the reason, so the
  // reason is printed rather than left to a hover.
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

  // OPENING THE CONGREGATION SCREEN is an Outputs job now, and the convenience
  // this surface used to add — pick the first non-primary monitor when nobody has
  // chosen one — is `FirstRun.svelte`'s, at the moment it actually matters. Live
  // held a second, differently-worded path to the same three commands; two
  // surfaces answering "which screen is the wall" is how they come to disagree.

  // ── THE MIC-QUALITY AND LANGUAGE COPY WENT WITH THE BANNERS ──────────────
  // C2, operator instruction 2026-09-14. `QUALITY` (clipping · too_quiet ·
  // noisy) and `langWarning` composed the sentences for two amber boxes at the
  // foot of this surface; the boxes are gone, so the sentences are gone with
  // them rather than left as a string table nothing renders. The reasoning and
  // what an operator loses by it are at the removal site in the markup below.
  //
  // The EVENTS are untouched: `audio://quality` and `stt://language_unstable`
  // still cross the bridge and `capture.js` still stores both, so nothing in
  // `ipc.test.js`'s contract moves and a future surface has the facts to hand.

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
  // PREVIEW RUNS ONE AHEAD, whatever the grid is showing. The prototype states
  // the rule in its own comment — "Preview runs one ahead, so what is coming is
  // always visible" — and implements it in two places: `take()` sets the cue to
  // the slide after the one it just put up, and `stepLive()` does the same as it
  // walks. Relay had it for a PLAN only (`previewNext` below), so a chapter of
  // scripture or a song's sections left the preview empty while a verse was on
  // air, and the operator had to go and find the next one by hand.
  //
  // It is derived from the rendered cells rather than tracked, so it cannot drift
  // from what the grid is showing, and it is never a fire of its own: it only
  // decides what the PREVIEW pane and TAKE are pointed at.
  $: liveCellIdx = grid.cells.findIndex((c) => cellLive(c));
  $: gridNextCell =
    liveCellIdx >= 0 && liveCellIdx + 1 < grid.cells.length ? grid.cells[liveCellIdx + 1] : null;
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
        : gridNextCell
          ? {
              reference: gridNextCell.reference ?? gridNextCell.label,
              text: gridNextCell.text || gridNextCell.label,
              translation: null,
            }
          : null;
  $: previewLabel = gridPreview
    ? gridPreview.label
    : dets[0]
      ? dets[0].reference
      : previewCue
        ? cueLabel(previewCue.item.label, previewSlide?.label)
        : (gridNextCell?.label ?? '');
  /** The take. Never a new code path — the same accept/fire the keys already run. */
  async function take() {
    if (gridPreview) return fireCell(gridPreview);
    if (dets[0]) return acceptTop();
    if (previewCue) return fireSlide(previewCue.item, previewCue.slide);
    // The one-ahead cell, so TAKE fires what the pane is actually showing. A
    // preview that cannot be taken is a preview of nothing.
    if (gridNextCell) return fireCell(gridNextCell);
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
  let railSong = null; // { title, slides } staged from the Songs half, or null
  let railEl; // the rail component, so `/` can still reach its box

  /** Stage a chapter from the rail. It does NOT fire — the grid does that. */
  function stageChapter(book, chapter) {
    railSong = null;
    railChapter = { book, chapter };
  }

  /**
   * Stage a SONG from the rail (docs/REBRAND.md §2). It does NOT fire.
   *
   * The deck is built by `reflow` — the SAME function the Library's editor uses,
   * so the slides an operator picks here are the slides they saw there. A second
   * splitter would be a second answer to "where does this verse end", and the
   * two would drift on exactly the long sections that need splitting.
   *
   * A chapter and a song are never staged at once: whichever the operator asked
   * for last is what the grid shows, which is what `handPicked` has always meant.
   */
  async function stageSong(id, title) {
    railChapter = null;
    railSong = { title: title ?? '', slides: [] };
    try {
      const full = await getSong(id);
      if (dead) return;
      const name = full?.title || title || '';
      railSong = { title: name, slides: reflow(full?.sections ?? []) };
    } catch (e) {
      railSong = null;
      flash(humanError(e));
    }
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
    songSlides: railSong?.slides ?? [],
    songTitle: railSong?.title ?? '',
    handPicked: !!railChapter || !!railSong,
  });

  // `SLIDES · <plan name> · <date>` (docs/REBRAND.md §2). The date is only a fact
  // about a PLAN — a chapter and a song do not have one — so it is empty for
  // every other source rather than being invented.
  //
  // …AND NOT WHEN THE NAME HAS ALREADY SAID IT (`dateStatedIn`). A plan called
  // `Sunday morning · 7 September` printed the date twice, in two notations, on
  // one line.
  $: gridSubtitle =
    grid.source === 'plan' && !dateStatedIn(grid.title, openPlan?.plan_date)
      ? shortDate(openPlan?.plan_date)
      : '';

  /**
   * What a cell's kind chip says — the content kind, in the word `plan.js`'s one
   * taxonomy already holds for it.
   *
   * `ctype` is the CONTENT kind `slidegrid.js` puts on every cell, which is what
   * a chip over a rendered slide is answering. An empty or unrecognised one goes
   * through `typeOf`'s `unknown` row, which says UNKNOWN and claims nothing —
   * never a fallback to scripture, the one kind the AI may fire by itself.
   */
  const kindOf = (c) => typeOf(c?.ctype).label;

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
    // A SONG SECTION staged from the rail. `fireContent` is the path
    // `LyricsPane` has always fired songs down — not a new one — and it is the
    // only one that can take words with no reference to resolve. The label names
    // the cue in history; `fire_content` suppresses it on the way to the glass,
    // so the congregation reads the words and nothing else (rule 36).
    if (cell.kind === 'song') {
      if (!cell.text.trim()) return;
      try {
        await fireContent(cell.label, cell.text, 'song');
        flash(`${cell.label} is on the screens`);
      } catch (e) {
        flash(humanError(e));
      }
      return;
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

  /** Is this cell what is on the congregation's screen right now?
   *
   *  A song section has no reference to compare — `fire_content` deliberately
   *  sends none — so it is matched on the WORDS, which are what went out. Amber
   *  is never allowed to lie, so the comparison is against `$liveContent`, the
   *  store's record of what actually left the machine, not against the press. */
  $: cellLive = (c) =>
    c.kind === 'plan'
      ? /* AMBER IS NEVER ALLOWED TO LIE (rule 18, DECISIONS §22), and this branch
           was the one place in the grid that took the console's WORD for it. The
           other two compare against `$liveContent` — what actually left the
           machine — while a plan cell asked only the playhead, which is restored
           from the saved session on every mount. So a relaunch with a remembered
           `liveOnAir` painted a cell amber and said `Live` over a wall that had
           nothing on it at all, which is exactly what the resume dialog promises
           cannot happen ("nothing is put back on any screen").

           A plan cue carries no reference and a song section deliberately sends
           no label, so the strongest honest comparison is the WORDS, with an
           empty cue (a media or countdown slide, which has none) falling back to
           asking whether anything is on the wall at all. */
        planOnAir &&
          c.cueId === liveCueId &&
          c.slideIdx === liveSlide &&
          !$screenBlack &&
          !!$liveContent &&
          (c.text.trim() ? $liveContent.text === c.text : true)
      : c.kind === 'song'
        ? !!c.text.trim() && !$screenBlack && $liveContent?.text === c.text
        : !!c.reference && !$screenBlack && $liveContent?.reference === c.reference;

  // How many times the previewed verse has ALREADY gone out this service.
  //
  // Recomputed only when the previewed reference changes — not on every store
  // tick — because this is a per-verse DB read on the run surface. 0 also means
  // "no service is being recorded", which correctly shows nothing.
  //
  // `verseRepeatCount` swallows by contract: a badge that fails to load costs the
  // operator nothing they cannot see for themselves.
  //
  // IT IS ASKED ABOUT A REFERENCE, NEVER ABOUT A DISPLAY LABEL. It used to be
  // handed `previewLabel`, which for a plan cue is a composed name — so a NOTICE
  // called `Welcome & Notices` was put to `verseRepeatCount` and came back with a
  // count, and the head read `shown 2×` over a cue that is not a verse and has no
  // reference to have repeated. The badge's own sentence is "a preacher circling
  // back to a verse is normal"; a claim about a notice is a different claim, made
  // by a question nobody asked.
  //
  // `previewRef` is the REFERENCE the take would fire, or null:
  //   · a grid cell   `reference`, which `slidegrid.js` sets for verse cells and
  //                   leaves null for plan and song cells — the distinction is
  //                   already made there and is not re-derived here
  //   · a detection   always a reference
  //   · a plan cue    only when the cue is scripture, where `slidesOf` puts the
  //                   reference in the slide's own label
  $: previewRef = gridPreview
    ? (gridPreview.reference ?? null)
    : dets[0]
      ? (dets[0].reference ?? null)
      : previewCue && previewCue.item?.cue_type === 'scripture'
        ? (previewSlide?.label ?? null)
        : null;
  let previewRepeats = 0;
  let repeatsFor = null;
  $: if (previewRef !== repeatsFor) {
    repeatsFor = previewRef;
    previewRepeats = 0;
    if (previewRef) {
      const asked = previewRef;
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
  // FULL SCREEN is the one that is left. A `Normal | Compact` density switch sat
  // beside it and was removed on the operator's instruction: it changed spacing
  // and type only, never which controls existed, so it was a preference about
  // padding sitting on the surface a service is run from. Full screen earns its
  // place there because it reclaims the whole chrome, which is a different
  // amount of room entirely.
  //
  // WHAT WENT WITH IT, stated rather than glossed: compact shrank `.con-top`'s
  // clamp, and at a 1366x768 booth laptop that gave the slides grid 298px where
  // it now gets 226px. Nothing became unreachable — `.pane-body` scrolls, the
  // panic controls are in the dock, and the take rack is `align-self:start`, so
  // it sizes to its own content either way — but a booth on a small screen now
  // scrolls the grid a little sooner. The clamp was deliberately NOT retuned to
  // compensate: that would shrink Preview and Program for every operator to pay
  // for a setting one of them used to choose, and it is a design change nobody
  // asked for. Measured in the T2 review note.
  $: fullscreen = !!$session.liveFullscreen;
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
  // ACT ON THE CLAIM THAT WAS OPENED, not on `dets[0]`.
  //
  // `inspect()` above already opens the card that was pressed, and the comment on
  // it says why. The two buttons in the panel's footer did not follow: they called
  // `acceptTop`/`dismissTop`, which are `dets[0]` — the `A` and `D` keys' claim,
  // which is the right meaning for a key pressed at the column and the wrong one
  // for a button pressed inside a panel about a different verse.
  //
  // So opening "why this match?" on the third card and pressing the amber
  // "Accept & fire" put the FIRST card's verse on the congregation's screens. The
  // inspector is the surface an operator opens BECAUSE they are being careful,
  // which is what makes this the worst possible place for it. The dismiss half was
  // quieter and worse: the claim they rejected stayed, and the one they wanted went.
  //
  // Capture before clearing — `inspecting` is null by the time the await resolves.
  async function inspectAccept() {
    const d = inspecting;
    inspecting = null;
    await accept(d);
  }
  async function inspectDismiss() {
    const d = inspecting;
    inspecting = null;
    if (d) dismissDetection(d.reference);
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
<div class="con" class:fullscreen>
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
        bind:this={railEl}
        disabled={!$capture.available}
        onChapter={stageChapter}
        onSong={stageSong}
        onReference={fireReference}
        onVerse={fireSearchHit} />
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
             two places at once reads as two facts.
             L2 RE-EXAMINED THIS AND LEFT IT. The prototype fills its right-hand
             slot with `nothing cued` while its frame says the same thing, and the
             argument above is the better one: this repository already refuses a
             glyph in a value slot for exactly this reason, and an absent slot
             makes no claim at all. What DID change is the face — the frame now
             speaks in the machine's own mono, which was the real difference. -->
        {#if previewLabel}<span class="mon-name r-mono">{previewLabel}</span>{/if}
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


    <!-- ══════ THE TAKE COLUMN — ONE BLOCK, NOT FIVE FRAGMENTS (L4) ══════
         It was `Take` · TAKE · `‹ Prev` · `Next ›` · `walks the programme` ·
         `VERSE`, six things stacked at an even 6px gap with no internal seam, so
         the eye had no way to tell that the caption belonged to the arrows and
         the arrows belonged to the button above them. Reading it took as long as
         reading six unrelated controls, on the column an operator uses fastest.

         It is now three BANDS inside one bordered rack, divided by the rack's own
         hairline: the take, the step, and what the step walks. The transition
         moved in as a fourth band, at the operator's instruction and below all
         three, because it changes how a take LOOKS and never what a take does —
         so it may never be the thing a hurried hand lands on.

         THE MODE BADGE STAYS, and is now inside the band it is about rather than
         a loose line under it. Required by CLAUDE.md: the transport is MODE-AWARE
         and says so, because the same key silently meaning two things is how the
         wrong thing reaches a congregation. The prototype has no equivalent and
         that is the prototype being wrong, not Relay being noisy. -->
    <aside class="rack">
      <div class="rk-band rk-take">
        <span class="rack-lbl">Take</span>
        <button
          class="take"
          on:click={take}
          disabled={!previewContent || !$capture.available}
          title="Put the previewed content on the outputs">TAKE</button>
      </div>

      <!-- THE STEP, AND WHAT IT WALKS, IN ONE BAND. The two arrows and the
           caption that explains them used to be separated by the same gap that
           separated everything else. `role="group"` gives the pair one
           accessible name, so a screen reader reads them as a transport rather
           than as two unrelated buttons.
           FULL WIDTH AND NAMED. Two 30px arrow glyphs side by side was the
           smallest pair of targets on the surface an operator uses fastest, and
           `‹` and `›` name nothing: they are the same two shapes whichever of
           the two things the transport is about to do. -->
      <div class="rk-band rk-step" role="group" aria-label="Transport">
        <button class="r-btn rk wide" title="Previous (←)" on:click={() => step(-1)}>‹ Prev</button>
        <button class="r-btn rk wide" title="Next (→)" on:click={() => step(1)}>Next ›</button>
        <span
          class="rack-cap"
          title={mode === 'slide'
            ? 'Arrow keys step through the service plan'
            : 'Arrow keys walk through the passage on screen'}>
          <!-- TWO LINES, DELIBERATELY (L2). The prototype breaks this caption by
               hand rather than leaving it to a 118px column: at mono capitals the
               phrase is within a pixel or two of the rack's width, so whether it
               wraps at all depends on the font that happened to load. A break that
               is decided here is the same on every machine, and it reads as a
               screen reader's single phrase either way. -->
          walks the<br />programme
          <b class="rack-mode r-mono" class:slide={mode === 'slide'}>{mode === 'slide' ? 'SLIDE' : 'VERSE'}</b>
        </span>
      </div>

      <!-- THE TRANSITION (docs/REBRAND.md §8 · DECISIONS §84), moved out of the
           chrome bar (L4).

           TWO AUTHORITIES OVER ONE PROPERTY, AND THE PICKER IS WHAT MAKES THAT
           HONEST. §71 says a transition is a template's choice; this says an
           operator may overrule every template at once, which is a second home
           for one property unless somebody can see which home is answering.
           Hence the first option: **Follow template**. It is the default, it is
           what a fresh Relay does, and choosing anything else is a visible act
           with a visible state — not a preference silently sitting on top of a
           saved one.

           The duration is disabled while the template is being followed, because
           there is nothing for it to be the duration OF: a number an operator can
           set that changes nothing is the §69 defect, and it is what the old theme
           editor's transition control was for the whole of its life.

           IT IS LAST IN THE RACK, always. TAKE and the arrows are what a hand
           reaches for without looking; a picker may never be above either of
           them, and nothing here may grow tall enough to push them. -->
      <div class="rk-band rk-x" class:on={$liveTransition}>
        <span class="rack-lbl xcap">Transition</span>
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
      </div>
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
        <!-- ONE LINE, ONE FACE (L2). The prototype's programme head reads
             `PROGRAM · ON AIR · AS MAIN SCREEN` as a single run of mono capitals,
             and ours printed a chip in the body face followed by a sentence-case
             tail, so the two halves read as two different kinds of statement. The
             separator is drawn here rather than as a glyph in a value slot: it
             joins two facts that are both present, and disappears with the tail. -->
        {#if mainChannel}
          <span class="mon-sep" aria-hidden="true">·</span>
          <span class="mon-as r-mono" title="This pane renders through {mainChannel.name}'s template">as {mainChannel.name}</span>
        {/if}
        <span class="spring"></span>
        <!-- The REFERENCE, amber only when a congregation is genuinely looking
             at it. Amber is ON AIR and is never allowed to lie.
             NOTHING, not a dash, when nothing is live. A glyph in a value slot
             cannot tell "the wall is clear" from "nobody has asked yet", and the
             tag to the left already says `Program · Clear` in words while the
             pane below says `Screens clear`. An empty slot makes no claim at all,
             which is the honest thing for a slot with nothing to report; the
             integrator's ruling on the dock's dial, one column over, is the same
             rule and this half of the tree must not grow a second answer. -->
        {#if $live}
          <span class="mon-name r-mono" class:live={!$rehearsing && !$screenBlack}>
            {$live.reference || 'content'}
          </span>
        {/if}
      </header>
      <div class="screen">
        {#if $live && progTpl}
          <TemplateRender
            template={progTpl}
            content={$liveContent}
            onFit={noteFit}
          />
        {:else if $live}
          <!-- CONTENT, AND NOTHING TO RENDER IT WITH. Measured on 2026-09-14: with
               no template resolved this pane drew an amber ON AIR frame over a
               black rectangle and said nothing, while the PREVIEW pane one column
               left said `No active template — activate one in Templates` in the
               same situation. One pane explained itself and the other did not —
               and the silent one was the one claiming to be on air. A blank frame
               and a blackout look identical and are not the same fact (rule 35). -->
          <div class="screen-empty">
            {$live.reference || 'Content'} is on air, but no template is active — activate one in Templates
          </div>
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
      <!-- THE HEADER SAYS WHAT THIS IS AND WHAT A PRESS DOES (docs/REBRAND.md §2).
           The hint used to be small print in the pane's FOOTER, under a grid that
           scrolls — so the one sentence that tells an operator a single click
           goes straight to the congregation was below the thing it is about, and
           an operator learning this surface never read it. It is on the head now,
           beside the count, where the eye lands before the first press. -->
      <header class="pane-head sg-head">
        <h2>Slides</h2>
        <!-- WHAT IS STAGED. Nothing, not a dash, when nothing is: the body
             already says which empty it is, in a sentence. -->
        {#if grid.title}<span class="sg-cap">· {grid.title}</span>{/if}
        {#if gridSubtitle}<span class="sg-cap">· {gridSubtitle}</span>{/if}
        <span class="spring"></span>
        <!-- ONE LINE, WITH THE COUNT LEADING IT (L2). The count and the sentence
             were two spans in two faces, so the right of this head read as two
             separate remarks about the grid; the prototype sets them as a single
             mono run — `14 · SINGLE CLICK GOES TO AIR · DOUBLE CLICK PREVIEWS` —
             and the count is the first thing in it because "how many" is what an
             operator is looking for when they glance here mid-service. -->
        <span class="sg-hint r-mono" title="A single click sends the slide to the programme; a double click only previews it."><b class="cnt">{grid.cells.length}</b><span class="sg-say">{' · single click goes to air · double click previews'}</span></span>
        {#if openPlan}
          <button class="mini ghost" on:click={leave} title="Stop running {openPlan.title}">Close plan</button>
        {/if}
        <!-- THE VIEW CONTROL LIVES HERE NOW (L2), not in the browsing rail.
             It changes how the console LOOKS and never what reaches a screen,
             and in the rail it was among the loudest things in a column whose
             whole job is finding a verse — the prototype's rail carries nothing
             of the kind. This head is where it belongs: the grid is the surface
             it actually reclaims space for.
             THERE WAS A SECOND ONE HERE, a `Normal | Compact` density segment,
             removed on the operator's instruction (T2). The wrapper stays: it
             is the named slot for the view controls, and `Full screen` is one.
             This is a DELETION, not a hidden control — the handler, the session
             key and every `.con.compact` rule went with it, so there is nothing
             left for `qa-inventory` to find and nothing for a future reader to
             mistake for a preference somebody forgot to wire up. -->
        <div class="view-ctl">
          <button class="view-fs" on:click={() => setFullscreen(!fullscreen)}>
            {fullscreen ? 'Show tabs' : 'Full screen'}
          </button>
        </div>
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
                    <!-- A KEYED template is a band over a camera Relay never
                         takes, so previewed against nothing it is an empty dark
                         rectangle — right on the wall, useless on a cell. The
                         Outputs cards and the Templates gallery already answer
                         this; a third answer here would be a third surface with
                         its own opinion about the same template. One rule
                         (`isKeyedTemplate`), one picture (`CameraPlate`), and
                         it never reaches an output — the plate is a preview
                         affordance and `cameraplate.test.js` holds that. -->
                    <!-- `isKeyedTemplate({})` is TRUE — no layers and no
                         background is, structurally, a template that keys out.
                         That is right for the question it answers (may a
                         blackout black this channel) and wrong as a reason to
                         draw a camera: with no active template every cell would
                         wear a plate and the grid would claim a camera behind a
                         wall that has no look at all. The plate needs a template
                         that EXISTS and keys; the absence gets nothing. -->
                    {#if cellTemplate(c) && isKeyedTemplate(cellTemplate(c))}<CameraPlate />{/if}
                    <TemplateRender template={cellTemplate(c) ?? {}} content={cellContent(c)} />
                  {/if}
                  <!-- THE KIND, TOP-LEFT, IN WHOLE WORDS — as the prototype
                       draws it: `NOTICE`, `SCRIPTURE`, `SONG`, `MEDIA`.
                       It used to print the cell's `tag`, which for everything
                       but a song section is an abbreviation `plan.js::slidesOf`
                       invents for the plan rail's narrow chip — `SCR`, `NOTE`
                       and, worst of the three, `BG`, which names nothing an
                       operator would recognise. `plan.js`'s own table already
                       records that a truncation is "a name nobody chose"; the
                       rule applies here too, and the table has the words to fix
                       it. `typeOf` is the ONE door onto it (rule 36), so a cell
                       and a running-order row cannot come to disagree about what
                       a cue is, and an unknown `cue_type` reads UNKNOWN rather
                       than being presented as scripture. Nothing is lost: the
                       section or the verse an operator wants is in the cell's
                       own label, one line below. -->
                  <span class="sg-tag r-mono">{kindOf(c)}</span>
                  <!-- The word, not only the colour — amber alone is not a label.
                       `Live`, as the prototype plates it (L2): the chip is 8px in
                       a 158px cell, and at that size the one-word form is read
                       rather than deciphered. The COLOUR LAW is untouched — the
                       plate is still `--v-amber` on `--v-amber-ink` and still
                       derived from `cellLive`, which reads the store rather than
                       the press. The console's own head one row up still says
                       `Program · On Air` in full, which is where the long form
                       earns its width. -->
                  {#if cellLive(c)}<span class="sg-air">Live</span>
                  {:else if gridPreview?.key === c.key}<span class="sg-prev">Preview</span>{/if}
                </span>
                <span class="sg-meta">
                  <span class="r-mono sg-n">{String(c.n).padStart(2, '0')}</span>
                  <span class="sg-ttl">{c.label}</span>
                </span>
              </button>
            {/each}
          </div>
        <!-- LOADING IS NOT EMPTY, AND A FAILED READ IS NEITHER (RG-95). The plan
             pane used to carry these three states and it has gone; the grid is
             the only surface left that can tell an operator why it has nothing
             to show, so it says so here rather than printing "this plan has no
             slides yet" over a read that failed — about a plan they built. -->
        {:else if openPlan && !itemsLoaded}
          <Loading what="cues" compact />
        {:else if openPlan && $readErrors.planItems}
          <ErrorState compact error={$readErrors.planItems} onRetry={() => loadPlan(openPlan)} />
        {:else if grid.source === 'plan' || openPlan}
          <EmptyState message={$t('live.plan_no_cues')} />
        {:else if $readErrors.chapterVerses}
          <ErrorState compact error={$readErrors.chapterVerses} onRetry={() => { gridChapter = null; loadChapterFor(stagedRef); }} />
        {:else}
          <EmptyState message={$t('live.nothing_staged')} />
        {/if}
      </div>

      <!-- THE TRANSPORT'S OWN VOICE. These sentences are the feedback for `→`,
           `←`, a grid press and the rail's Fire — "Now live: John 3:16", "End of
           the passage", the humanised reason a fire was refused. They were in the
           plan pane's footer; the plan pane has gone (the plan IS the grid now)
           and the messages had to land somewhere an operator is already looking.
           `role="status"` so every one of them is announced. -->
      <footer class="pane-foot sg-foot">
        <span class="sr-only" role="status" aria-live="polite" aria-atomic="true">{liveMsg}</span>
        {#if liveMsg}
          <span class="flash" aria-hidden="true"><i class="fd"></i>{liveMsg}</span>
        {:else}
          <span class="flash idle" aria-hidden="true">{openPlan ? openPlan.title : 'No plan loaded'}</span>
        {/if}
      </footer>
    </section>

  </div>

  <!-- ══════ THE PLAN IS THE GRID ══════
       A SERVICE PLAN pane used to sit under the slides, and it was two surfaces
       in one: a chooser when no plan was open, and a second copy of the running
       order when one was. Both have gone (docs/REBRAND.md §2 — "the plan IS the
       grid"), and neither took a control with it.

       CHOOSING WHICH PLAN TO RUN belongs to the Planner. That is where a plan is
       built, named and dated, and `Run in Live` there already loads it and brings
       the operator here; a second chooser on the run surface was a second answer
       to "which plan am I running". `Load whole plan` in the Quick tools header
       re-stages the one the Planner handed over, which is the prototype's own
       control and the reason Quick tools has a header button at all.

       THE RUNNING ORDER is the grid: every slide of every cue, in order, each
       rendered as the wall would render it, with the playhead on the cell that is
       actually on air. What the removed rail added on top of that was a cue's
       stage note — an operator-only line for the PREACHER'S monitor, authored in
       the Planner and delivered by the fire itself. It is not lost from the
       service; it is no longer previewed here. Recorded in the review note. -->

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
               status line that lies.

               THIS LINE IS ALSO WHERE THE ARM STATE LIVES NOW (L4). The column
               used to carry an `Armed` chip beside it that duplicated the dock's
               ARMED switch — two controls for one gate, one row apart. The chip
               has gone and the dock keeps the control; the state did not go
               anywhere, because it was already here, saying more than the chip
               could. -->
          <span class="det-meta r-mono" class:on={gateState.armed}>{gateState.label}</span>
          <!-- THE MICROPHONE IS THE ONE CONTROL THIS COLUMN KEPT, and it is on the
               head rather than in a row of its own, because a row holding one
               button is the loose fragment the chip left behind.

               It is NOT a duplicate of anything in the dock: the dock's Live audio
               card holds the two decisions about a signal (how readily, and armed
               or not) and nothing that starts or stops the signal itself. With
               this gone, the only way to open a microphone would be Settings →
               Dashboard, which is not a Sunday-morning path. Recorded in the
               review note as a thing that should move to the dock's audio card
               (agent L3's file), beside the ARMED switch it belongs with. -->
          <button class="ibtn" on:click={toggleListen} title={$capture.capturing ? 'Stop listening' : 'Start listening'}
            aria-label={$capture.capturing ? 'Stop listening' : 'Start listening'}
            disabled={!$capture.available || !$capture.stt.loaded || listenBusy}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><rect x="9" y="2" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0M12 18v4"/></svg>
          </button>
        </header>

        <!-- NO SENSITIVITY DIAL HERE either. It lives in the dock, one row below,
             on EVERY workspace (docs/REBRAND.md §2 puts it beside the signal it is
             about). This surface carried a second copy of it, and two controls for
             one dial is two places to disagree about the same gate. -->

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
                <!-- THE CHIP NAMES THE METHOD, not merely heard-vs-guessed.
                     This was `heard(d) ? 'Heard' : 'Paraphrase'`, so `semantic`,
                     `ambiguous` and `uncertain_book` all wore one word — and
                     `uncertain_book` is the method added after "hymn number three
                     sixteen" put Numbers 3:16 on a wall, which rule 10 calls the
                     claim an operator most needs to look at. `methodKey` was
                     imported into this file and never called. Colour is unchanged:
                     every non-direct method stays cyan and stays without a
                     percentage (rule 18). -->
                <span class="cbadge" class:p={!heard(d)}>{$t(methodBadgeKey(d))}</span>
              </div>

              {#if heard(d)}
                <!-- A bar, not just a number: "0.92" means nothing to a volunteer. -->
                <div class="conf" role="meter" aria-valuemin="0" aria-valuemax="100"
                  aria-valuenow={Math.round(d.confidence * 100)} aria-label="Detection confidence">
                  <i style="width:{Math.round(d.confidence * 100)}%"></i>
                </div>
              {:else}
                <!-- …and the note says what is actually true of THIS method.
                     "not a spoken reference" is right for a paraphrase and wrong
                     for the other two: an ambiguous reference WAS spoken, and for
                     `uncertain_book` the chapter and verse were heard and only the
                     book was repaired. One sentence for all three said the
                     opposite of what happened on the two that matter most. -->
                <p class="guess-note">{$t(methodNoteKey(d))}</p>
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

        <!-- NOTHING BELOW THE CLAIMS. The manual reference box that used to sit
             here is the rail's `Fire` now (§9's one box); the screens answer for
             themselves in the chrome lamps and in Outputs; the emergency
             announcement is a Quick tools job. What is left is the column the
             prototype draws: the AI's claims, and nothing competing with them. -->
        {#if errMsg}<div class="err" role="alert">{errMsg}</div>{/if}
      </section>

    </div>
  </div>

  {#if $capture.audioError}<div class="audioerr">Audio: {$capture.audioError}</div>{/if}
  {#if $capture.outputError}<div class="audioerr">Output: {$capture.outputError}</div>{/if}

  <!-- ══ WHAT THE LAMPS CANNOT SAY ══
       The Output Status pane left this column: a screen's state is one lamp per
       screen in the chrome and a full row in Outputs, and a third opinion about
       the same screen is exactly what rule 35 was written against. These two
       lines are NOT that opinion — neither is about one screen:

         · nothing is showing what is on air. A whole-room fact, and the one
           sentence a lamp cannot compose because no single lamp knows it;
         · the verse on the wall is smaller than its template asked for. Rule
           37's measurement, taken by the program pane through the SAME renderer
           the wall uses, so it is the wall's number rather than a guess.

       Both are reported, never enforced (a service runs on the console preview
       all the time), and both appear only when they are true. -->
  {#if nowhereToShow}
    <div class="out-warn" role="status">
      <b>No screen is showing this.</b> Something is on air and every screen you have
      is down, idle or has not answered. Relay is still sending — check them in Outputs.
    </div>
  {/if}
  {#if fitWarning}<div class="out-warn" role="status"><b>Small on the wall.</b> {fitWarning}</div>{/if}
  <!-- Announced once, on the transition, through the same polite region the AI's
       suggestions use. A live region that repeats is one an operator learns to
       tune out. -->
  <p class="sr-only" aria-live="polite">{downAnnounce}</p>

  <!-- ── THE MICROPHONE-QUALITY AND LANGUAGE BANNERS ARE GONE ────────────────
       C2, operator instruction 2026-09-14: "take out this notification section
       completely… and nothing should go there." Two amber boxes rendered here —
       the `dsp.rs` mic warnings (clipping · too quiet · noisy) and the
       language-instability note — and this is deliberately an EMPTY space now,
       not a space with something quieter in it.

       WHAT WENT WITH THEM, so nobody has to find this out on a Sunday: the
       `too_quiet` warning was the ONLY place in the console that named a muted
       microphone as a muted microphone. `audio://quality` still arrives and
       `$capture.quality` is still read (App.svelte for the denoise lamp,
       Settings for a room's observed note), but nothing on a run surface turns
       it into words any more. The Live audio card in the dock shows the LEVEL —
       `−∞ dB`, a `quiet` chip and a flat trace — which is the same picture a
       silent prayer draws, and its red `no signal` fires on readings that stop
       ARRIVING, not on readings that arrive at zero. So a muted channel is
       visible as a level and is not announced as a fault. That is the
       operator's call, made with the consequence stated; it is written up in
       C2's review note rather than softened into a quieter banner here. -->

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
    font-family:var(--f-body); font-size:var(--v-fs-b1); color:var(--v-cyan); cursor:pointer;
    text-decoration:underline; }
  .inspect-link:hover{ filter:brightness(1.15); }

  /* ── §4 the view control ──
     Sized DOWN when it moved into the slides head (L2): in the rail it was among
     the column's largest controls, and on a pane head it has to sit beside a
     caption without out-shouting it.

     A `.seg` segmented control sat beside it and is GONE with the compact
     density it drove (T2). Its rules went too rather than being left as dead
     weight — `.seg` was declared here and matched nothing else: `LiveRail`'s
     collection switch is `.lr-seg`, its own class with its own rules, because
     Svelte scopes a component's styles and this `.seg` never reached it. */
  .view-ctl{ flex:0 0 auto; display:flex; align-items:center; gap:5px; }
  .view-fs{ height:22px; padding:0 8px; border-radius:var(--v-r-sm); cursor:pointer;
    background:var(--v-surf); border:1px solid var(--v-line2); color:var(--v-faint);
    font-family:var(--f-body); font-size:var(--v-fs-b3); font-weight:600; }
  .view-fs:hover{ color:var(--v-txt); border-color:var(--v-accent-line); }

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
  /* THIS CLAMP IS NOW THE ONLY ONE. A compact density used to override it to
     clamp(196px,24vh,268px); with that control deleted (T2) every operator gets
     the figures below, which at 1366x768 is 268px here and 226px for the grid
     under it. Nothing is unreachable at that size — `.pane-body` scrolls, the
     panic controls are in the dock, and `.rack` is `align-self:start` so it
     sizes to its own content — but a small booth screen reaches the grid's
     scrollbar sooner than it used to. Retuning this to compensate would shrink
     Preview and Program for everybody, so it was left alone deliberately. */
  .con-top{flex:0 0 auto; height:clamp(268px,33vh,364px);
    display:grid; grid-template-columns:1fr 118px 1fr; gap:var(--v-sp-sm); min-height:0}

  /* ── the desk: rail, stage, inspector ──────────────────────────────────── */
  .desk{flex:1; min-height:0; display:grid;
    grid-template-columns:206px minmax(0,1fr) 286px; gap:var(--v-sp-sm)}
  .rail-col{display:flex; flex-direction:column; gap:var(--v-sp-sm); min-height:0; min-width:0}
  .rail-col :global(.lrail){flex:1 1 auto; min-height:0}
  .stage{display:flex; flex-direction:column; gap:var(--v-sp-sm); min-height:0; min-width:0}
  /* The inspector runs the FULL HEIGHT of the workspace and holds ONE pane: the
     AI's claims (docs/REBRAND.md §2). It used to hold two, and the height was
     split by `:first-child` / `:last-child` — a pair that BOTH match when there
     is only one child, so the later rule won and capped the claims at 46% of a
     column with nothing under it. One child, one rule. */
  .insp-col{display:flex; flex-direction:column; gap:var(--v-sp-sm); min-height:0; min-width:0}
  .insp-col > .pane{flex:1 1 auto; min-height:0}

  /* THE GRID TAKES WHAT IS LEFT. It shared the stage with a SERVICE PLAN pane
     and the two split the remaining height 1.15 : 1; the plan pane has gone —
     the plan IS the grid (docs/REBRAND.md §2) — so there is nothing to share
     with, and the grid stretches under the monitors instead of stopping short
     and leaving a band of nothing where the second pane used to be. */
  .con-grid{flex:1 1 0; min-height:0; display:flex}
  .con-grid :global(.pane){flex:1; min-width:0}

  /* THE VIEW CONTROLS LEFT THIS COLUMN (L2). Four rules used to reshape them to
     survive a 206px rail — a wrapping row, a segment forced to its own line, a
     full-screen button forced to a second one — all of which existed because
     three controls do not fit across a browsing rail. On a pane head they do,
     so the rules went with them rather than being carried as dead weight. The
     measurement they recorded is kept here because it is the reason nothing of
     this kind goes back into a column that narrow: the row wanted 135px inside
     114px and the (since-deleted) `Compact` button was clipped to `Compa`. */

  .pane{display:flex; flex-direction:column; min-height:0; overflow:hidden;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg);
    box-shadow:var(--v-shadow-sm)}

  /* WRAPS. A panel header is a heading plus its controls, and on one unwrapped
     line the controls always won: at 1366×768 — the commonest church laptop —
     "AI Detection — Current Claim" was rendered 5px wide and "Live Transcript"
     74px, because the gate's own controls are sized to content and the heading
     was the only thing allowed to shrink. Now the controls drop to a second line
     instead of crushing the name of the panel they belong to. */
  .pane-head{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    flex-wrap:wrap; row-gap:6px;
    padding:10px 12px; border-bottom:1px solid var(--v-line)}
  /* The reference console has no sidebar, so its panels are ~25% wider than they can
     be here. The heading is therefore set a touch tighter than the design sheet's
     Label spec so the full panel name still fits rather than truncating. */
  .pane-head h2{margin:0; min-width:0; font-family:var(--f-head); font-size:var(--v-fs-cap);
    line-height:var(--v-lh-lbl); font-weight:600; letter-spacing:.06em; text-transform:uppercase;
    color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap}
  .pane-body{flex:1; min-height:0; overflow-y:auto; padding:var(--v-sp-sm) 12px;
    display:flex; flex-direction:column; gap:var(--v-sp-sm);
    scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent}
  .pane-body::-webkit-scrollbar{width:6px}
  .pane-body::-webkit-scrollbar-thumb{background:var(--v-surf3); border-radius:var(--v-r-round)}
  .pane-foot{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    padding:var(--v-sp-sm) 12px; border-top:1px solid var(--v-line)}

  /* ── PREVIEW / PROGRAM ─────────────────────────────────────────────────── */
  .mon-bar{flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm);
    padding:8px 10px; border-bottom:1px solid var(--v-line)}
  /* THE MONO FACE, on the whole head and not on half of it (L2). The prototype
     sets this line in the mono face at 9.5px with .11em of tracking, which is
     what makes `PROGRAM · ON AIR · AS MAIN SCREEN` read as ONE statement rather
     than a chip with a sentence after it. The WORDS are unchanged — the capitals
     come from `text-transform`, so `.mon-as` still says "as Main screen" to a
     screen reader and to `livedesk.test.js`, which is the right place for the
     repository's plain voice to live. */
  .tag{flex:0 0 auto; padding:4px 10px; border-radius:var(--v-r-sm);
    font-family:var(--f-mono);
    font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.11em; text-transform:uppercase}
  /* STEEL BLUE = the thing you are working on, which is what a preview is.
     Amethyst is REHEARSAL and nothing else (rule 18, DECISIONS §22) — this chip
     wore it, so on the one morning both were true the operator read the wrong
     one. The grid's cued cell has always been steel blue; these two now agree. */
  .tag.preview{background:var(--v-sel); color:var(--v-sel-ink)}
  /* Amber, and only when the congregation is genuinely looking at it. */
  .tag.onair{background:var(--v-amber); color:var(--v-amber-ink)}
  .tag.reh{background:var(--v-amethyst-soft); border:1px solid var(--v-amethyst-line); color:var(--v-amethyst)}
  .tag.off{background:var(--v-grey-soft); border:1px solid var(--v-line2); color:var(--v-dim)}
  /* HARD RIGHT, MONO, UPPERCASE. The reference is the one figure on this head an
     operator reads from across a booth, and in the body face it sat at a
     different weight and rhythm from everything beside it. */
  .mon-name{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:var(--v-fs-cap); letter-spacing:.09em; text-transform:uppercase;
    color:var(--v-faint)}
  /* Amber ONLY when a congregation is genuinely looking at it. */
  .mon-name.live{color:var(--v-amber)}
  .mon-as{flex:0 0 auto; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:var(--v-fs-fig); letter-spacing:.11em; text-transform:uppercase; color:var(--v-faint)}
  /* The join between two facts that are both present. It carries no value of its
     own, so it is hidden from the accessibility tree rather than read aloud. */
  .mon-sep{flex:0 0 auto; margin-left:-2px; font-family:var(--f-mono);
    font-size:var(--v-fs-fig); color:var(--v-faint)}
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
    font-size:var(--v-fs-b3); letter-spacing:.06em; color:var(--v-faint);
    border:1px solid var(--v-line2)}
  .screen{flex:1; min-height:0; position:relative; overflow:hidden; background:#000;
    border-top:1px solid var(--v-line)}
  /* THE PROTOTYPE'S `.blank` FACE (L2): mono, tracked, faint, on black. What a
     monitor says when it has nothing to show is a machine's statement about
     itself, not prose, and it reads as one in the mono face. The WORDS are
     Relay's and stay Relay's — the prototype wraps them in em dashes and this
     repository does not use them (`livedesk.test.js` holds the neighbouring rule
     that no glyph may stand in for a value, and the house style forbids the
     dash outright), so the sentence carries itself. */
  .screen-empty{position:absolute; inset:0; display:grid; place-items:center; padding:var(--v-sp-md);
    text-align:center; font-family:var(--f-mono); font-size:var(--v-fs-cap);
    letter-spacing:.06em; color:var(--v-faint)}
  .blk{position:absolute; inset:0; background:#000}

  /* ── the take rack ─────────────────────────────────────────────────────── */
  /* align-self:start — the rack is only as tall as its controls. Left to stretch
     to the full height of the 16:9 preview/program panes beside it, the leftover
     vertical space had to go SOMEWHERE, and it ballooned the MODE chip into a huge
     empty box. A transport rack is compact by nature; keep it that way. */
  /* THREE BANDS AND A SEAM, NOT SIX EVENLY-SPACED THINGS (L4). The rack's own
     children are the bands; the gap BETWEEN bands is a hairline and the gap
     INSIDE one is 6px, which is the whole reason the column now reads as
     `take · step · look` instead of as a list. `padding:0` on the rack so a band
     can carry the full-width rule; each band pads itself. */
  .rack{display:flex; flex-direction:column; min-height:0; align-self:start; padding:0;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg);
    overflow:hidden}
  .rk-band{display:flex; flex-direction:column; gap:6px; padding:10px 8px;
    border-top:1px solid var(--v-line)}
  .rk-band:first-child{border-top:0}
  .rack-lbl{font-family:var(--f-mono); font-size:var(--v-fs-fig); font-weight:700; letter-spacing:.14em;
    text-transform:uppercase; color:var(--v-faint); text-align:center}
  /* 64px, as measured in the prototype. The one control on this surface that is
     always the same press, in the same place, however tired the operator is. */
  .take{height:64px; border-radius:var(--v-r-md); border:0; cursor:pointer;
    background:var(--v-amber); color:var(--v-amber-ink); font-family:var(--f-body);
    font-size:var(--v-fs-lbl); font-weight:700; letter-spacing:.1em;
    box-shadow:0 6px 18px -6px var(--v-amber-glow);
    transition:transform 90ms var(--v-ease), filter .14s}
  .take:hover:not(:disabled){filter:brightness(1.06)}
  .take:disabled{opacity:.4; cursor:not-allowed; box-shadow:none}
  /* ── FEEDBACK ON POINTER-DOWN, NOT ON CLICK (L2) ──────────────────────────
     CSS `:active` begins at pointerdown and ends at release, which is the beat
     the prototype's `.press` class reproduces in JavaScript — so the rule is
     already the right one and only these three controls were missing it. The
     scales are the prototype's measured values (.955 on TAKE, .96 on the arrows,
     .985 on a cell, which is a large target and needs less).
     `transform` only, so the compositor does the work and nothing reflows.
     REDUCED MOTION STILL GETS FEEDBACK — as a cut, not as movement: the same
     press reads as a brightness step instead, because an operator who asked for
     no animation still has to be able to tell a press from a dead button. */
  @media (prefers-reduced-motion: no-preference){
    .take:active:not(:disabled){transform:scale(.955)}
    .rk:active:not(:disabled){transform:scale(.96)}
    .sg-cell:active:not(:disabled) .sg-thumb{transform:scale(.985)}
  }
  @media (prefers-reduced-motion: reduce){
    .take:active:not(:disabled),
    .rk:active:not(:disabled){filter:brightness(.88)}
    .sg-cell:active:not(:disabled) .sg-thumb{filter:brightness(.88)}
  }
  /* THE TRANSPORT IS AN ORDINARY BUTTON (B1). It drew its own box and was three
     steps off the shared control at once — 28px against 26, a `--v-line2`
     hairline against the `--v-500` every other control draws, and `--v-fs-cap`
     against the button's `--v-fs-b2` — on the pair an operator reaches for more
     often than anything else on this surface. It is `.r-btn` now, and this rule
     keeps only the two things a transport needs that a button does not.
     `flex:0 0 auto` is load-bearing rather than tidy: the rack is a flex COLUMN,
     where a fixed height is still shrinkable, so without it a short dock quietly
     squeezes Prev and Next below the shared 26px and nothing says so. */
  .rk.wide{width:100%; flex:0 0 auto}
  /* The caption the prototype puts under the transport, with the MODE inside it
     rather than beside it. "walks the programme" answers WHAT the two buttons
     do; the mode answers WHICH walk — and they are one sentence, so an operator
     cannot read the first and miss the second. */
  /* Mono capitals, as the prototype sets it. This caption labels a control; it is
     not prose, and beside a mono TAKE and a mono mode badge the body face was the
     only thing in the rack speaking a different language. */
  /* NO MARGIN OF ITS OWN — it is a child of the step band now, and the band's
     6px gap is what separates it from the arrows it explains. A margin here on
     top of that gap is how the caption came to read as a fifth loose line. */
  .rack-cap{display:block; text-align:center;
    font-family:var(--f-mono);
    font-size:var(--v-fs-fig); line-height:1.35; letter-spacing:.09em;
    text-transform:uppercase; color:var(--v-faint)}
  .rack-mode{display:block; margin-top:3px;
    font-size:var(--v-fs-cap); font-weight:700; letter-spacing:.1em; color:var(--v-cyan)}
  /* Amber here is NOT "on air": it is the plan's own colour on the plan rail
     beside it, and SLIDE mode means the arrows walk the plan. It sits on a
     caption, not on a claim about a screen. */
  .rack-mode.slide{color:var(--v-amber)}

  /* ── THE TRANSITION BAND (L4 · docs/REBRAND.md §8 · DECISIONS §84) ────────
     These rules came out of `app.css`'s X1 block when the control left the
     chrome. They are scoped here now, beside the only markup that wears them —
     a shared stylesheet carrying rules for an element in one component is how a
     dead rule survives a move. */
  .rk-x .xcap{text-align:center}
  /* STACKED, because the rack is 118px wide and two selects side by side in it
     would each be 48px: a picker whose own text ("Fade through black") cannot be
     read is a control that has to be opened to be understood. */
  .rk-x .xpick{width:100%; height:22px; padding:0 20px 0 7px; font-size:10.5px;
    background-position:calc(100% - 11px) 10px,calc(100% - 8px) 10px}
  /* AN OVERRIDE IS IN FORCE, AND THAT IS VISIBLE WITHOUT OPENING THE DROPDOWN.
     Deliberately NOT amber (on air), amethyst (rehearsal) or cyan (a guess) —
     the colour law is fixed and this is none of those three. A brighter border
     and a brighter text colour say "somebody chose this" without claiming a
     state. */
  .rk-x.on .xpick{border-color:var(--v-accent-line); color:var(--v-txt)}
  /* THE RACK HAS ROOM FOR THE REASON, so the reason is a line and not a hover.
     Rose, and only ever about a change that did NOT reach the screens. */
  .xerr{text-align:center; font-size:var(--v-fs-fig); letter-spacing:.06em;
    text-transform:uppercase; color:var(--v-rose); border:1px solid var(--v-rose);
    border-radius:var(--v-r-sm); padding:1px 5px}

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
  /* A 2px BORDER OVER A 1px OUTLINE, both of which take the state's colour —
     the prototype's treatment, and the reason it exists (L4). A single 1px
     hairline plus a soft fill is legible in isolation and invisible at a glance
     in a grid of twenty 158px cells: the eye is scanning twenty rendered slides
     and one hairline is the smallest thing on the surface. Three pixels of
     colour on the outside of the picture is not. The `outline` is drawn INSIDE
     the box (`outline-offset:-1px`) so a coloured cell takes no more space than
     an uncoloured one and the grid does not re-flow as the playhead moves.
     No new colour: amber is ON AIR and steel blue is the selection, exactly as
     before, and `slidegridwiring.test.js` still reads that off these rules. */
  .sg-thumb{position:relative; display:block; aspect-ratio:16/9; overflow:hidden;
    container-type:inline-size;
    border-radius:var(--v-r-md);
    background:var(--v-void); border:2px solid transparent;
    outline:1px solid var(--v-line2); outline-offset:-1px;
    transition:border-color var(--v-dur) var(--v-ease), outline-color var(--v-dur) var(--v-ease),
      background var(--v-dur) var(--v-ease), transform 90ms var(--v-ease)}
  /* A cue the grid could not expand. It is DRAWN rather than dropped (see
     `planCells`) so the count under the grid agrees with the plan, and it is
     disabled rather than firing nothing. */
  .sg-void{position:absolute; inset:0; display:grid; place-items:center; padding:8px;
    text-align:center; font-size:var(--v-fs-b3); letter-spacing:.05em; color:var(--v-faint)}
  .sg-cell:hover .sg-thumb{border-color:var(--v-sel-line)}
  /* Steel blue is SELECTION — the thing you are working on. It is what a preview
     is, and it is deliberately not grey: grey means CUED, a plan position. */
  .sg-cell.cued .sg-thumb{border-color:var(--v-sel); outline-color:var(--v-sel);
    background:var(--v-sel-soft)}
  /* Amber is ON AIR and nothing else. `cellLive` derives it from what the store
     says is on the screen, never from "we pressed the button". */
  .sg-cell.islive .sg-thumb{border-color:var(--v-amber); outline-color:var(--v-amber);
    background:var(--v-amber-soft)}
  /* FOCUS IS STILL ITS OWN RING, outside the box, so keyboard focus on a cell
     that is already live is still distinguishable from the live state itself. */
  .sg-cell:focus-visible .sg-thumb{outline:2px solid var(--v-sel); outline-offset:2px}
  /* TOP-LEFT, over the rendered slide, on its own scrim. It used to sit
     bottom-left on a grey card; over a real slide it needs its own ground or it
     lands on whatever the template happens to be painting there. `--v-surf3` is
     not a legible ground for dim text (tokencontrast.test.js), so the chip
     carries its own black and near-white. */
  /* A WHOLE WORD, AND AN ELLIPSIS IF IT EVER WILL NOT FIT. `max-width` is a share
     of the thumbnail rather than a fixed figure, so the rule holds at whatever
     width the auto-fill grid gives a cell. A truncated word is honest and is
     recoverable from the label below; an invented abbreviation is neither, which
     is what `SCR`, `NOTE` and `BG` were. */
  .sg-tag{position:absolute; left:5px; top:5px; padding:2px 5px; z-index:2;
    max-width:calc(100% - 10px); overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
    border-radius:2px; background:rgba(0,0,0,.62); color:#cfd6e2;
    font-size:8px; font-weight:600; letter-spacing:.08em; text-transform:uppercase}
  .sg-air,.sg-prev{position:absolute; right:5px; top:5px; padding:2px 5px; z-index:2;
    border-radius:2px; font-size:8px; font-weight:700; letter-spacing:.1em;
    text-transform:uppercase}
  .sg-air{background:var(--v-amber); color:var(--v-amber-ink)}
  .sg-prev{background:var(--v-sel); color:var(--v-sel-ink)}
  .sg-meta{display:flex; align-items:baseline; gap:6px; min-width:0; padding:0 2px}
  .sg-n{flex:0 0 auto; font-size:var(--v-fs-b3); color:var(--v-dim)}
  .sg-ttl{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:var(--v-fs-cap); color:var(--v-txt)}
  .sg-cell.islive .sg-ttl{color:var(--v-amber)}
  /* AND THE LABEL, as the prototype colours it. The border says which cell; the
     label is what an operator is already reading, and colouring it means the two
     states are legible from the text alone if the picture is a dark slide. */
  .sg-cell.cued .sg-ttl{color:var(--v-sel)}
  .sg-foot{display:flex; align-items:center; gap:var(--v-sp-sm)}
  /* `· SUNDAY MORNING · 14 SEP` — the plan's own name and date, in the head's
     own face (L2). These used to be set in the body face beside an uppercase
     `SLIDES`, so the three parts of one title read as three different things. */
  .sg-cap{min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-family:var(--f-mono); font-size:var(--v-fs-b3); letter-spacing:.09em;
    text-transform:uppercase; color:var(--v-dim)}
  /* WHAT A PRESS DOES, as ONE run (L2). `min-width:0` and the ellipsis matter:
     this is the first thing allowed to give way when the head runs out of room,
     because the pane's NAME, `Close plan` and the view controls all have to
     survive a narrow window and this sentence is carried verbatim in `title`
     besides — and on every cell's own `title` under the pointer. */
  .sg-hint{flex:0 1 auto; min-width:0; overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap; font-size:var(--v-fs-b3); letter-spacing:.08em;
    text-transform:uppercase; color:var(--v-faint)}
  /* The count LEADS the line and is the one part of it that is a figure. It is
     also the part that never goes: a count is a FACT about the grid, and the
     sentence beside it is teaching copy read once. They are separate spans for
     exactly that reason — the ladder below hides the sentence and keeps the
     figure, which a single span could not do. */
  .sg-hint .cnt{flex:0 0 auto; font-weight:700; color:var(--v-dim)}
  /* NOTHING IN THIS HEAD MAY BE SQUEEZED OUT BY THE SENTENCE. `Close plan` stops
     a plan from running and the view controls are real features; measured at
     2000px the head carries five things and fits, and the two narrower rungs the
     integrator asked about are answered by the ladder at the foot of this file
     rather than by hoping flexbox picks the right victim. */
  .sg-head .mini{flex:0 0 auto}

  /* ── 3 · detection ─────────────────────────────────────────────────────── */
  /* THE `Armed` CHIP AND ITS ROW ARE GONE (L4), and so are `.chip` / `.btnchip`
     with them — the only two elements wearing either were that chip. A duplicate
     control for a gate the dock already switches is two places to disagree about
     one setting (rule 35's family), and the honest replacement for a control is
     not a quieter control: it is the state line that was always beside it.
     `.det-ctl` went the same way. A row that held two things and now holds one is
     a loose fragment, so the microphone sits on the panel head instead. */
  /* WHAT THE GATE IS DOING — and it reads differently when it is broken. Grey
     until Relay is genuinely armed and listening; emerald when it is. Never
     amber: nothing about a gate's readiness is on air. It is the LAST thing
     allowed to shrink on this head — `min-width:0` and an ellipsis rather than
     `flex:0 0 auto`, because in a 286px column the head now carries a name, this
     line and a button, and the one that must survive is the one that says
     whether the AI is listening at all. */
  .det-meta{flex:0 1 auto; min-width:0; overflow:hidden; text-overflow:ellipsis;
    white-space:nowrap;
    font-size:var(--v-fs-fig); letter-spacing:.08em; text-transform:uppercase;
    color:var(--v-faint)}
  .det-meta.on{color:var(--v-emerald)}

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
    font-size:var(--v-fs-kind); font-weight:600; letter-spacing:.08em; text-transform:uppercase;
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
  .clm-done{margin:0; font-family:var(--f-mono); font-size:var(--v-fs-fig); letter-spacing:.08em;
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
  .act span{font-size:var(--v-fs-fig); opacity:.85; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; max-width:100%}
  .act:hover:not(:disabled){filter:brightness(1.08)}
  /* DISABLED, NOT HIDDEN. A reference that parsed against no verse still has to
     be shown — it is the operator's evidence that a number was misheard — and
     the control that cannot take it says why rather than failing after a press. */
  .act:disabled{cursor:not-allowed; opacity:.45}
  .act.go{background:var(--v-emerald); color:var(--v-void)}
  .act.no{background:var(--v-red); color:#fff}
  .khint{margin:0; text-align:center; font-size:var(--v-fs-b3); color:var(--v-faint)}
  .khint kbd{font-family:var(--f-mono); font-size:var(--v-fs-fig); color:var(--v-dim);
    background:var(--v-surf3); border:1px solid var(--v-line2); border-radius:var(--v-r-sm); padding:2px 5px}

  .klbl{font-family:var(--f-mono); font-size:var(--v-fs-fig); font-weight:700; letter-spacing:.14em;
    text-transform:uppercase; color:var(--v-faint)}
  .sub{display:flex; align-items:center; gap:var(--v-sp-sm); margin-top:var(--v-sp-sm);
    padding-top:var(--v-sp-sm); border-top:1px solid var(--v-line)}
  .mini{padding:4px 10px; border-radius:var(--v-r-sm); border:0; cursor:pointer;
    font-family:var(--f-body); font-size:var(--v-fs-cap); font-weight:600;
    background:var(--v-amber); color:var(--v-amber-ink)}
  .mini.ghost{background:transparent; border:1px solid var(--v-line2); color:var(--v-dim)}
  .mini:hover{filter:brightness(1.08)}
  .rel-note{margin:0; font-size:var(--v-fs-b3); color:var(--v-faint)}
  .rel-chips{display:flex; flex-wrap:wrap; gap:6px}
  .rel-chip{font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-dim);
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:var(--v-r-sm);
    padding:5px 11px; cursor:pointer}
  .rel-chip:hover:not(:disabled){color:var(--v-txt); background:var(--v-surf3)}
  .rel-chip:disabled{opacity:.45; cursor:default}

  .err{padding:0 12px 10px; color:var(--v-red); font-size:var(--v-fs-cap)}

  /* `.wide` WAS A WHOLE SECOND BUTTON, AND IT WAS THE ONE THAT WON (B1).
     It declared a complete skin — 32px, `--v-surf2`, a `--v-line2` hairline,
     `--v-fs-lbl` — under a name that reads like a layout utility, and the only
     element in this file wearing it is the transport pair. `.rk` and `.wide`
     have the SAME specificity, so source order decided, and `.wide` is two
     hundred lines further down: every property the two rules shared was taken
     from here and `.rk`'s box was dead code for the whole of its life.
     That is why the transport measured 32px in the browser while this file
     plainly said 28 — and why reading either rule on its own explains nothing.
     The skin is gone; the transport is `.r-btn`, and `.rk.wide` up the file
     keeps the width. A class named for a layout may not carry a look. */

  .flash{display:flex; align-items:center; gap:8px; min-width:0; overflow:hidden;
    text-overflow:ellipsis; white-space:nowrap; font-size:var(--v-fs-cap); color:var(--v-emerald)}
  .flash.idle{color:var(--v-faint)}
  .fd{width:6px; height:6px; border-radius:50%; background:var(--v-emerald);
    box-shadow:0 0 8px var(--v-emerald); flex:0 0 auto}

  /* A WARNING, NOT A FAILURE AND NOT A LIVE STATE. Amber would mean ON AIR here
     (DECISIONS §22) and rose TEXT would overstate it — Relay IS still sending.
     Dim text, rose rule. Unchanged from when these two lines sat in the Output
     Status pane; only their place on the surface moved. */
  .out-warn{flex:0 0 auto; padding:7px 9px; font-size:var(--v-fs-cap);
    color:var(--v-dim); background:var(--v-surf2); border-radius:var(--v-r-sm);
    border-left:2px solid var(--v-rose)}
  .out-warn b{color:var(--v-txt)}

  /* ── banners ───────────────────────────────────────────────────────────── */
  .audioerr{flex:0 0 auto; background:var(--v-red-soft); color:var(--v-red);
    border:1px solid var(--v-red-line); border-radius:var(--v-r-md);
    padding:9px 12px; font-size:var(--v-fs-lbl)}
  /* `.sttwarn` IS GONE, NOT QUIETENED (C2). It dressed the two microphone /
     language banners this surface no longer renders. A rule left behind for a
     markup that no longer exists is how the next person "restores" a thing
     nobody asked for; the removal is documented at the markup site. */

  /* ── accessibility ─────────────────────────────────────────────────────── */
  .take:focus-visible,.rk:focus-visible,.slide:focus-visible,
  .act:focus-visible,
  .mini:focus-visible,.ibtn:focus-visible,
  .reh-end:focus-visible{outline:2px solid var(--v-amber); outline-offset:2px}
  @media (prefers-reduced-motion:reduce){
    .reh-dot{animation:none}
  }

  /* ── responsive ────────────────────────────────────────────────────────── */
  @media (max-width:1400px){
    .con-top{grid-template-columns:1fr 104px 1fr}
    .desk{grid-template-columns:180px minmax(0,1fr) 250px}
    /* A LADDER, NOT A SWITCH — the same shape as `app.css`'s `.xcap`, and for the
       same reason. The slides head carries five things: the pane's name, the
       count, this sentence, `Close plan` and the view controls. Four of them are
       facts or controls; the sentence is teaching copy an operator reads once,
       and it is the only one that can go without anything becoming unreachable.
       Measured at 2000px all five fit; below this the sentence yields FIRST and
       on purpose, rather than flexbox choosing a victim — which at 1366 and 1024
       would have wrapped `Close plan` onto a second row or pushed it under the
       fold. It is still on the hint's own `title`, and on every cell's `title`
       under the pointer, so nothing is lost that a hover or a screen reader
       cannot recover. The COUNT stays at every width: it is a fact about the
       grid, not an explanation of it. */
    .sg-say{display:none}
  }
  /* THE INSPECTOR GOES UNDER, NEVER AWAY. The prototype hides its right column
     below 1240px; Relay may not, because the column holds the AI's claims and
     their Accept / Dismiss — the two controls the product exists to offer. A
     booth laptop is where an operator is most cramped and least able to go
     hunting, so the column becomes a row beneath the stage instead: nothing is
     removed, and nothing needs a scroll to reach. */
  @media (max-width:1180px){
    .con{height:auto}
    .desk{grid-template-columns:1fr}
    .insp-col{flex-direction:row; align-items:stretch}
    .insp-col > .pane{flex:1 1 auto; min-height:340px}
    /* The rail becomes a strip above the stage rather than a column beside it —
       nothing is removed, because a booth laptop is where an operator is most
       cramped and least able to go hunting. */
    .rail-col{flex-direction:row; align-items:stretch; height:200px}
    .con-top{height:auto; grid-template-columns:1fr 104px 1fr; grid-auto-rows:minmax(230px,auto)}
    .con-grid{flex:0 0 auto; height:320px}
  }
  @media (max-width:760px){
    /* AUTO rows. In one column the 230px floor inherited from the rule above was
       being applied to the TAKE rack too — a short strip of buttons padded out to
       230px, leaving a dead gap between Preview and Program. The two screens keep
       their own height; the rack takes what it needs. */
    .con-top{grid-template-columns:1fr; grid-auto-rows:auto}
    .con-top>.pane{min-height:230px}
    .rack{min-height:0}
    .rail-col{flex-direction:column; height:auto}
    .insp-col{flex-direction:column}
    .insp-col > .pane{flex:0 0 auto; min-height:0}
    .rack{flex-direction:row; align-items:center; flex-wrap:wrap}
    .rack-cap{flex:1 0 100%; margin-top:0}
  }
</style>
