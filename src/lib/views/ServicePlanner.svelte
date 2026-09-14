<script>
  // Service Planner — BUILD a plan. Running it is not this screen's job.
  //
  // It used to be both, and that was the mistake: the operator ran the service
  // from here, which meant they were sitting on a tab that could not show them an
  // AI suggestion. The preacher would go off-script, Relay would detect the verse,
  // and the suggestion would appear on a tab nobody was looking at.
  //
  // So the two jobs are split along the line that actually exists in the church's
  // week. Building a plan is a Tuesday task: unhurried, fiddly, lots of searching
  // and reordering. Running it is a Sunday task: one screen, big targets, no
  // typing, nothing that can be dragged by accident. They want opposite designs.
  //
  // Build here. Run in LIVE.
  //
  // LAYOUT: plans rail · running order · cue inspector. The plans list used to be
  // a separate full-page step; as a rail it stays put, so comparing last week's
  // order with this week's is one click rather than three.
  import { onMount } from 'svelte';
  // The shared workspace grammar (docs/REBRAND.md §2 · §11) — the same columns,
  // panes, type roles and name/value row the Outputs and Settings desks use.
  import WorkspaceFrame from './WorkspaceFrame.svelte';
  import EmptyState from '../ui/EmptyState.svelte';
  import ErrorState from '../ui/ErrorState.svelte';
  import Loading from '../ui/Loading.svelte';
  import TemplateRender from '../TemplateRender.svelte';
  import { humanError } from '../errors.js';
  import { trapFocus } from '../focus.js';
  import { songCue } from '../cues.js';
  import { setSession } from '../session.js';
  import {
    TYPE,
    typeOf,
    payloadOf,
    slidesOf,
    slideAccent,
    sectionsOf,
    planRuntime,
    fmtDuration,
    parseDuration,
    chipOf,
    planDateLabel,
    cueCountLabel,
    dropIndex,
    reorderTo,
  } from '../plan.js';
  import {
    capture,
    templates,
    listPlans,
    createPlan,
    deletePlan,
    duplicatePlan,
    planItems,
    addPlanItem,
    removePlanItem,
    movePlanItem,
    setPlanNote,
    reorderPlan,
    setPlanSection,
    setPlanDuration,
    setPlanTemplate,
    searchScripture,
    searchSongs,
    getSong,
    listArrangements,
    listMedia,
    listAnnouncements,
    loadTemplates,
    readErrors,
  } from '../stores/capture.js';

  // ── plans list ──
  let plans = [];
  let showNew = false;
  let newTitle = '';
  let planQ = ''; // rail filter

  // ── editor ──
  let openPlan = null;
  let items = [];
  let selId = null; // cue loaded in the inspector
  let msg = '';
  // Distinguish "still loading" from "genuinely empty": listPlans swallows errors
  // to [], so without this flag a slow cold-open renders "No plans yet" — telling
  // the operator to create a plan they may already have.
  let loading = true;
  let err = ''; // a FAILURE, humanised — never the green success slot (a failed
  //              save shown in success-green is how a lost cue reads as saved).

  // Every backend mutation routes through here. The store wrappers throw on
  // error; without a catch a rejected invoke() is a silently dead button — the
  // operator clicks Add, nothing happens, and no reason is shown. One wrapper,
  // one humanised error surface, no exceptions.
  async function act(fn) {
    err = '';
    try {
      await fn();
    } catch (e) {
      err = humanError(e);
    }
  }
  let leftMode = 'cues'; // 'cues' | 'add'
  let inspTab = 'general'; // 'general' | 'slides' | 'notes'

  // one search (add mode) — scripture + songs + media together
  let addQ = '';
  let addVerses = [];
  let addSongs = [];
  let addMedia = [];
  let allMedia = []; // full media library, filtered locally by the query
  let addAnnounce = [];
  let allAnnounce = []; // full announcement list, filtered locally
  let addSearching = false;

  // THE DESK OPENS ON A PLAN, not on an invitation to pick one.
  //
  // This workspace used to open as a page title, a paragraph and three empty
  // panes: the running order said "Pick a plan on the left to open it" and the
  // inspector said "Pick a cue to edit it", so the first thing an operator saw was
  // two sentences telling them the screen was not ready yet. A desk is a desk with
  // the work already on it — the prototype opens on the most recent plan and so
  // does this.
  //
  // It is a READ and nothing else. `open()` lists media, announcements and cues;
  // it invokes nothing that takes a screen, which is what
  // `plannerbuildonly.test.js` holds. And it defers to the operator: if they have
  // already clicked a plan while the list was still arriving, `openPlan` is set and
  // this does nothing rather than yanking them back to the newest one.
  //
  // Templates are loaded here too, not assumed. The inspector offers the template
  // picker for the selected cue; without this the store is empty on a cold open of
  // this tab and every cue reads "Template 4".
  onMount(() => {
    refresh()
      .then(() => {
        if (!openPlan && plans.length) return open(plans[0]);
      })
      .finally(() => (loading = false));
    loadTemplates();
  });

  async function refresh() {
    plans = await listPlans();
    // Keep the open plan's header in step with the rail (cue count, title).
    if (openPlan) openPlan = plans.find((p) => p.id === openPlan.id) || openPlan;
  }

  async function addPlan() {
    const title = newTitle.trim();
    if (!title) return;
    const date = new Date().toISOString().slice(0, 10);
    await act(async () => {
      await createPlan(title, date);
      newTitle = '';
      showNew = false;
      await refresh();
    });
  }
  async function removePlan(p, ev) {
    ev.stopPropagation();
    await act(async () => {
      await deletePlan(p.id);
      if (openPlan?.id === p.id) {
        openPlan = null;
        items = [];
        selId = null;
      }
      await refresh();
    });
  }
  async function clonePlan(p, ev) {
    ev.stopPropagation();
    await act(async () => {
      await duplicatePlan(p.id, `${p.title} (copy)`);
      await refresh();
    });
  }

  async function open(p) {
    openPlan = p;
    selId = null;
    leftMode = 'cues';
    inspTab = 'general';
    msg = '';
    allMedia = await listMedia().catch(() => []);
    allAnnounce = await listAnnouncements().catch(() => []);
    await loadItems();
    if (items.length) selId = items[0].id;
  }
  async function loadItems() {
    items = await planItems(openPlan.id);
  }

  // ── one search: scripture + songs ──
  let addTimer;
  function onAddInput() {
    clearTimeout(addTimer);
    addTimer = setTimeout(doAddSearch, 220);
  }
  async function doAddSearch() {
    const q = addQ.trim();
    if (!q) {
      addVerses = [];
      addSongs = [];
      addMedia = allMedia.slice(0, 8); // recent media when the box is empty
      addAnnounce = allAnnounce.slice(0, 8);
      return;
    }
    // try/finally so a failed search always releases the spinner — otherwise the
    // panel is stuck on "Searching…" forever with no results and no reason.
    addSearching = true;
    try {
      const [v, s] = await Promise.all([searchScripture(q), searchSongs(q)]);
      addVerses = v;
      addSongs = s;
      const ql = q.toLowerCase();
      addMedia = allMedia.filter((m) => m.filename.toLowerCase().includes(ql));
      addAnnounce = allAnnounce.filter(
        (a) => a.title.toLowerCase().includes(ql) || a.body.toLowerCase().includes(ql),
      );
    } catch (e) {
      err = humanError(e);
    } finally {
      addSearching = false;
    }
  }
  async function addVerse(v) {
    const payload = {
      book: v.book,
      chapter: v.chapter,
      verse: v.verse,
      reference: v.reference,
      text: v.text,
      translation: v.translation,
    };
    await act(async () => {
      await addPlanItem(openPlan.id, 'scripture', v.reference, payload);
      await loadItems();
      await refresh();
    });
  }
  async function addMediaCue(m) {
    const payload = { media_id: m.id, kind: m.kind, filename: m.filename };
    await act(async () => {
      await addPlanItem(openPlan.id, 'media', m.filename, payload);
      await loadItems();
      await refresh();
    });
  }
  async function addAnnounceCue(a) {
    const payload = { announce_id: a.id, title: a.title, body: a.body };
    await act(async () => {
      await addPlanItem(openPlan.id, 'announce', a.title || 'Announcement', payload);
      await loadItems();
      await refresh();
    });
  }
  let cdAddMin = 5;
  async function addCountdownCue() {
    const m = Number(cdAddMin) || 5;
    const payload = { minutes: m, label: 'Service begins in', done: 'Welcome' };
    await act(async () => {
      await addPlanItem(openPlan.id, 'countdown', `Countdown · ${m} min`, payload);
      // A countdown is the one cue type whose length is known at build time, so it
      // seeds its own duration instead of making the operator retype it.
      await loadItems();
      const added = items[items.length - 1];
      if (added) await setPlanDuration(added.id, m * 60);
      await loadItems();
      await refresh();
    });
  }
  // Song → plan. If the song has saved arrangements, open a picker so the
  // operator chooses one (or Standard); otherwise add the Standard order.
  let arrPick = null; // { song, arrangements } while choosing
  async function addSong(summary) {
    await act(async () => {
      const song = await getSong(summary.id);
      if (!song) return;
      const arrangements = await listArrangements(song.id);
      if (arrangements.length === 0) {
        await commitSong(song, null);
        return;
      }
      arrPick = { song, arrangements };
    });
  }
  async function commitSong(song, arr) {
    const { label, payload } = songCue(song, arr);
    await act(async () => {
      await addPlanItem(openPlan.id, 'song', label, payload);
      arrPick = null;
      await loadItems();
      await refresh();
    });
  }

  async function remove(id, ev) {
    ev.stopPropagation();
    await act(async () => {
      await removePlanItem(id);
      if (selId === id) selId = items[0]?.id ?? null;
      await loadItems();
      await refresh();
    });
  }
  async function move(id, dir, ev) {
    ev.stopPropagation();
    await act(async () => {
      await movePlanItem(id, dir);
      await loadItems();
    });
  }

  // ── Drag-reorder: pointer events, 1:1 with the finger, committed on release ──
  //
  // This was HTML5 drag-and-drop (`draggable`, `dragover`, `drop`). Three things
  // were wrong with that for a running order. The row did not move — the browser
  // drew its own translucent ghost and the list sat still, so there was no moment
  // at which the operator could see the order they were about to get. The drop
  // target was whichever row the pointer happened to be over, which on a list of
  // 34px rows is a different row from the gap you were aiming at. And
  // `dragstart`/`drop` are mouse-only in practice: a touch drag on a laptop's
  // trackpad-as-touchscreen, or a pen, never begins one.
  //
  // Pointer events are one code path for mouse, pen and touch. The row follows the
  // pointer exactly, its neighbours slide out of the way by one row height so the
  // gap is visible, and NOTHING is persisted until release — a drag abandoned
  // mid-list leaves the plan exactly as it was.
  //
  // The arithmetic is `plan.dropIndex`, not a line in this handler, because the
  // ends of the list are where a reorder goes wrong and that is worth a test.
  let drag = null; // { id, from, rows, y0, h, dy } while a row is in hand
  let dragId = null; // the row currently in hand, for the class

  function onGripDown(id, e) {
    if (e.button != null && e.button !== 0) return; // left button / touch only
    const row = e.currentTarget?.closest?.('.sp-row');
    const list = row?.parentNode;
    if (!row || !list) return;
    const rows = [...list.querySelectorAll('.sp-row')];
    const from = rows.indexOf(row);
    if (from < 0) return;
    // `offsetHeight` is 0 in a list that has not been laid out (and always in
    // jsdom); `dropIndex` treats a non-positive height as "nothing moved", and the
    // fallback keeps a real drag working on a list mid-layout.
    drag = { id, from, rows, y0: e.clientY, h: row.offsetHeight || 34, dy: 0 };
    dragId = id;
    selId = id; // what you are dragging is what the inspector is about
    try {
      e.currentTarget.setPointerCapture?.(e.pointerId);
    } catch {
      /* no capture (jsdom, or a mouse fallback) — the window handlers still fire */
    }
    e.preventDefault();
  }

  function onDragMove(e) {
    if (!drag) return;
    drag.dy = e.clientY - drag.y0;
    const shift = Math.round(drag.dy / drag.h);
    drag.rows.forEach((r, i) => {
      if (i === drag.from) {
        r.style.transform = `translateY(${drag.dy}px)`;
        return;
      }
      let t = 0;
      if (shift > 0 && i > drag.from && i <= drag.from + shift) t = -drag.h;
      if (shift < 0 && i < drag.from && i >= drag.from + shift) t = drag.h;
      r.style.transform = t ? `translateY(${t}px)` : '';
    });
  }

  function onDragEnd() {
    if (!drag) return;
    const { from, rows, dy, h } = drag;
    drag = null;
    dragId = null;
    // Clear every transform BEFORE the list re-renders: the each block is keyed, so
    // Svelte reuses these exact nodes and an inline transform left behind would
    // paint the new order shifted by a row.
    rows.forEach((r) => {
      r.style.transform = '';
    });
    const to = dropIndex(from, dy, h, items.length);
    if (to === from) return;
    const arr = reorderTo(items, from, to);
    items = arr;
    // Optimistic reorder, but NOT fire-and-forget: if the backend rejects, the
    // on-screen order and the persisted order silently diverge. Reload from the
    // source of truth on failure so the two can never disagree.
    act(async () => {
      try {
        await reorderPlan(openPlan.id, arr.map((i) => i.id));
      } catch (e) {
        await loadItems();
        throw e;
      }
    });
  }

  /**
   * Hand this plan to the LIVE tab and go there. The one path from build to run.
   *
   * It hands over a PLAN and a PLAYHEAD and says nothing about the wall.
   * `liveCueId: null` starts this plan at the top; `liveOnAir` is deliberately
   * ABSENT from the patch, because position and on-air-ness are separate facts
   * (CLAUDE.md, `liveCue`) and the second belongs to Live and to the panic
   * controls. Adding `liveOnAir: false` here would have this workspace assert
   * that a congregation's screen is clear — which is a panic control's claim, and
   * one it is not allowed to make on someone else's behalf (DECISIONS §20).
   * Nothing is invoked: loading a plan must not clear the programme (§2).
   * Pinned by `plannerbuildonly.test.js`.
   */
  function runPlan() {
    setSession({ planId: openPlan.id, liveCueId: null, liveSlide: 0, activeTab: 'live' });
  }

  $: selCue = items.find((i) => i.id === selId) || null;

  // ── sections + running time ──
  //
  // Both derived from the ordered cue list, never stored beside it, so a section
  // can never claim cues the transport does not actually walk.
  $: sections = sectionsOf(items);
  $: runtime = planRuntime(items);
  $: railPlans = planQ.trim()
    ? plans.filter((p) => p.title.toLowerCase().includes(planQ.trim().toLowerCase()))
    : plans;

  $: selTemplate = selCue?.template_id != null
    ? $templates.find((t) => t.id === selCue.template_id) || null
    : null;

  /**
   * Begin a section at the selected cue and put the cursor in its heading field.
   *
   * Deliberately not a `prompt()`: a modal browser dialog blocks the webview's
   * whole thread, cannot be focus-trapped with the rest of the app, and — as with
   * the arrangement picker below — Escape inside it would not be visible to
   * `shortcuts.js`, so dismissing it could fall straight through to the panic keys
   * and clear the congregation's screens.
   */
  let sectionInput;
  async function addSection() {
    const target = selCue || items[0];
    if (!target) return;
    await act(async () => {
      if (!target.section_title) await setPlanSection(target.id, 'New Section');
      selId = target.id;
      inspTab = 'general';
      await loadItems();
      queueMicrotask(() => sectionInput?.select());
    });
  }
  /** Commit the section heading. Blank merges the cue back into the one above. */
  async function saveSection() {
    if (!selCue) return;
    await act(async () => {
      await setPlanSection(selCue.id, secDraft);
      await loadItems();
    });
  }
  /** Commit a typed cue length. Blank/unreadable = untimed, which is legitimate. */
  async function saveDuration() {
    if (!selCue) return;
    await act(async () => {
      await setPlanDuration(selCue.id, parseDuration(durDraft));
      await loadItems();
    });
  }
  async function saveTemplate(ev) {
    if (!selCue) return;
    const v = ev.target.value;
    await act(async () => {
      await setPlanTemplate(selCue.id, v === '' ? null : Number(v));
      await loadItems();
    });
  }
  /** Copy a cue in place — the quickest way to a second cue of the same shape. */
  async function duplicateCue() {
    if (!selCue) return;
    await act(async () => {
      await addPlanItem(
        openPlan.id,
        selCue.cue_type,
        selCue.label,
        payloadOf(selCue),
        selCue.template_id,
      );
      await loadItems();
      await refresh();
    });
  }

  // Per-cue drafts. Seeded only when the selected cue changes (id guard) so a
  // reload — or another field's save — cannot clobber an edit in progress.
  let noteDraft = '';
  let secDraft = '';
  let durDraft = '';
  let noteFor = null;
  $: if (selCue && selCue.id !== noteFor) {
    noteDraft = payloadOf(selCue).stage_note || '';
    secDraft = selCue.section_title || '';
    durDraft = selCue.duration_sec ? fmtDuration(selCue.duration_sec) : '';
    noteFor = selCue.id;
  }
  async function saveNote() {
    if (!selCue) return;
    const id = selCue.id;
    await act(async () => {
      await setPlanNote(id, noteDraft);
      await loadItems();
    });
  }
  $: selSlides = slidesOf(selCue);

  // The inspector preview goes through TemplateRender — the ONE renderer used by
  // the fullscreen output and the Templates editor — so what the operator sees
  // here is what the wall will show, by construction rather than by resemblance.
  $: previewContent = !selCue
    ? null
    : selCue.cue_type === 'scripture'
      ? {
          reference: payloadOf(selCue).reference || selCue.label,
          text: payloadOf(selCue).text || '',
          translation: payloadOf(selCue).translation || '',
        }
      : { reference: selCue.label, text: selSlides[0]?.text || '', translation: '' };
</script>

<!-- Escape closes the arrangement picker, from anywhere — bound at the window rather
     than on the backdrop, which never holds focus. Without this, Escape inside the
     picker fell through to the global panic key: it cleared the congregation's screens
     and left the picker open. (shortcuts.js now also refuses to clear while any
     [role="dialog"] is mounted, so the two halves cannot disagree.) -->
<!-- The drag listens at the WINDOW, not on the row. A pointer that leaves the
     list mid-drag — over the inspector, off the top of the pane, out of the
     window entirely — still has to end the drag somewhere, and a handler bound to
     the row never hears that `pointerup`. Bound to the row, an abandoned drag left
     a cue stuck to the cursor with its neighbours shifted, and the next click
     committed a reorder nobody asked for. `pointercancel` is in the list for the
     same reason: a touch drag interrupted by the OS never sends `pointerup`. -->
<svelte:window
  on:keydown={(e) => arrPick && e.key === 'Escape' && (arrPick = null)}
  on:pointermove={onDragMove}
  on:pointerup={onDragEnd}
  on:pointercancel={onDragEnd} />

<!-- NO STANDFIRST, and the caveat did not go with it.
     The paragraph under the page title said "nothing on this workspace can reach
     an output screen — running it is Live's job", and it cost two rows at the top
     of a desk that should open ON a plan. The sentence now sits under the running
     order as the pane's own footer (`.sp-caveat`), where the prototype puts it and
     where it is stronger than it was as a standfirst: it is a pane FOOTER, outside
     the scroller, so it never scrolls away; it carries no media query, so unlike
     the toolbar note this replaced it cannot `display:none` itself on a narrow
     screen; and it is rendered in every mode, with or without a plan open. Do not
     restore the standfirst as well — half a caveat beside the whole one is weaker
     than the whole one alone, which is why this sentence has now moved twice. -->
<WorkspaceFrame title="Planner" columns="206px minmax(0,1fr) 330px">
  <svelte:fragment slot="head">
    {#if !$capture.available}
      <span class="r-badge rose"><span class="bd"></span>Backend not attached — plans need the desktop app</span>
    {/if}
    {#if err}<span class="sp-err r-mono" role="alert">{err}</span>
    {:else if msg}<span class="sp-msg r-mono">{msg}</span>{/if}
  </svelte:fragment>

  <!-- ══ RAIL: every plan, always reachable ══ -->
  <aside class="rw-pane sp-rail">
    <div class="rw-panehead">
      <h2 class="rw-panettl">Service plans</h2>
      <span class="rw-spring"></span>
      <span class="rw-itemn">{plans.length}</span>
    </div>
    <div class="sp-railsearch">
      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
      <input placeholder="Search plans…" bind:value={planQ} aria-label="Search plans" />
    </div>

    <div class="rw-panebody sp-raillist">
      {#if loading}
        <Loading compact what="plans" />
      {:else if railPlans.length}
        {#each railPlans as p (p.id)}
          <button class="sp-railcard r-focus" class:sel={openPlan?.id === p.id} on:click={() => open(p)}>
            <span class="sp-railtitle" title={p.title}>{p.title}</span>
            <!-- Both halves go through `plan.js`, and neither interpolates the
                 field raw. `{p.cue_count} cue{s}` printed "undefined cues" in this
                 rail against a plan summary that did not carry the field, and
                 `{p.plan_date || 'No date'}` is only right until somebody
                 shortens it to an em dash, which this repository already spends
                 on an untimed cue. A rail row is read, so it says words. -->
            <span class="sp-railfootline">
              <span class="sp-railmeta r-mono">{planDateLabel(p.plan_date)}</span>
              <span class="sp-railcues r-mono">{cueCountLabel(p.cue_count)}</span>
            </span>
          </button>
        {/each}
      {:else if plans.length}
        <div class="sp-hint r-mono">No plan matches “{planQ}”.</div>
      {:else if $readErrors.listPlans}
        <!-- RG-95. `listPlans` swallows to `[]`, so a database that did not answer
             read as "No plans yet" — and the answer to that sentence, on a Tuesday
             evening, is to build Sunday's service again from nothing. -->
        <ErrorState compact error={$readErrors.listPlans} onRetry={refresh} />
      {:else}
        <div class="sp-hint r-mono">No plans yet.</div>
      {/if}
    </div>

    <div class="rw-panefoot">
      {#if showNew}
        <form class="sp-newform" on:submit|preventDefault={addPlan}>
          <!-- svelte-ignore a11y-autofocus -->
          <input class="r-input" placeholder="Plan title…" bind:value={newTitle} autofocus />
          <div class="sp-newbtns">
            <button class="r-btn primary sm" type="submit">Create</button>
            <button class="r-btn ghost sm" type="button" on:click={() => (showNew = false)}>Cancel</button>
          </div>
        </form>
      {:else}
        <button class="r-btn primary sm" on:click={() => (showNew = true)}>＋ New Plan</button>
        <button class="r-btn ghost sm" disabled={!openPlan} on:click={(e) => clonePlan(openPlan, e)}>Duplicate Plan</button>
        <button class="r-btn ghost sm sp-raildel" disabled={!openPlan} on:click={(e) => removePlan(openPlan, e)}>Delete Plan</button>
      {/if}
    </div>
  </aside>

  <!-- ══ MAIN: the running order ══ -->
  <section class="rw-pane sp-main">
    <!-- The dock head: the plan's name on the left, what it costs and what you
         can do to it on the right — the prototype's shape (REBRAND §2). `Run in
         Live` moved here from the page head so the one path off this workspace
         sits beside the plan it would hand over, not above three panes. -->
    <div class="rw-panehead sp-panehead">
      <h2 class="rw-panettl sp-plantitle">{openPlan ? openPlan.title : 'Running order'}</h2>
      {#if openPlan}
        <span class="sp-hm r-mono">{planDateLabel(openPlan.plan_date)}</span>
        <!-- "est" is not decoration. Most plans contain a scripture cue, which is
             untimed by nature, so the sum is a floor and never the service length.
             Presenting a partial total as a real one is how a service runs long. -->
        <span class="sp-hm r-mono">{cueCountLabel(items.length)} · {fmtDuration(runtime.seconds, true)}{runtime.partial ? ' est' : ''}</span>
      {/if}
      <span class="rw-spring"></span>
      {#if openPlan}
        <div class="r-seg sp-toolseg">
          <button class:on={leftMode === 'cues'} on:click={() => (leftMode = 'cues')}>Running order</button>
          <button class:on={leftMode === 'add'} on:click={() => { leftMode = 'add'; if (!addQ.trim()) { addMedia = allMedia.slice(0, 8); addAnnounce = allAnnounce.slice(0, 8); } }}>＋ Add cue</button>
        </div>
        <button class="r-btn ghost sm" disabled={!items.length} on:click={addSection}>＋ Section</button>
      {/if}
      <!-- The ONLY path from build to run, and it hands over a plan — it does not
           put anything on a screen (`runPlan`, and `plannerbuildonly.test.js`).
           Steel blue, NOT the prototype's amber: amber means a congregation is
           looking at something (CLAUDE.md rule 18, DECISIONS §21), and a button on
           the one workspace that cannot reach an output is the last place allowed
           to borrow it. -->
      <button class="r-btn primary sm" on:click={runPlan} disabled={!openPlan || !items.length}>
        Run in Live
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
      </button>
    </div>

    {#if loading}
      <div class="rw-panebody pad"><Loading what="plans" /></div>
    {:else if !openPlan && !plans.length && $readErrors.listPlans}
      <div class="rw-panebody pad"><ErrorState error={$readErrors.listPlans} onRetry={refresh} /></div>
    {:else if !openPlan}
      <div class="rw-panebody pad">
        <EmptyState message={plans.length ? 'Pick a plan on the left to open it.' : 'No plans yet — create one to start building a service.'} />
      </div>
    {:else}
      {#if leftMode === 'cues'}
        <div class="rw-panebody sp-tablewrap">
          {#if items.length}
            {#each sections as sec (sec.items[0].id)}
              <!-- A section heading is a CAPTION and a hairline to the right edge,
                   not a container: `sectionsOf` derives the grouping from the same
                   ordered list the transport walks, so a heading can never claim a
                   cue the plan does not have in it. -->
              {#if sec.title}
                <div class="sp-sec">
                  <span class="sp-seccap">{sec.title}</span>
                  <span class="sp-secln"></span>
                </div>
              {/if}

              {#each sec.items as c (c.id)}
                {@const n = items.findIndex((i) => i.id === c.id)}
                <div class="sp-row" class:sel={c.id === selId} class:dragging={dragId === c.id}
                  on:click={() => (selId = c.id)} role="button" tabindex="0"
                  aria-pressed={c.id === selId}
                  on:keydown={(e) => {
                    // A role="button" must answer to Enter AND Space; this one only
                    // took Enter, so it was focusable but half-operable. preventDefault
                    // on Space, or the page scrolls under the operator instead.
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      selId = c.id;
                    }
                  }}>
                  <!-- The grip is a real control, not a decoration with a cursor.
                       It was an `aria-hidden` span, so reordering a plan was a
                       mouse-only act and a keyboard operator had no way to do it
                       from the running order at all. Pointer-drag for a mouse, pen
                       or finger; ↑/↓ for a keyboard, through the same
                       `move_plan_item` the inspector uses. -->
                  <button
                    class="sp-grip"
                    type="button"
                    aria-label="Reorder {c.label} — drag, or use arrow up and arrow down"
                    on:pointerdown={(e) => onGripDown(c.id, e)}
                    on:click|stopPropagation={() => (selId = c.id)}
                    on:keydown|stopPropagation={(e) => {
                      if (e.key === 'ArrowUp' && n > 0) { e.preventDefault(); move(c.id, -1, e); }
                      else if (e.key === 'ArrowDown' && n < items.length - 1) { e.preventDefault(); move(c.id, 1, e); }
                    }}>
                    <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor" aria-hidden="true"><circle cx="2" cy="2" r="1.1"/><circle cx="8" cy="2" r="1.1"/><circle cx="2" cy="7" r="1.1"/><circle cx="8" cy="7" r="1.1"/><circle cx="2" cy="12" r="1.1"/><circle cx="8" cy="12" r="1.1"/></svg>
                  </button>
                  <!-- The kind, in a word. One neutral ink for every kind: the
                       colours a taxonomy would want are all spoken for
                       (`TAXONOMY_INK`, CLAUDE.md rule 18), so the WORD is the
                       taxonomy and `chipOf` never truncates it. -->
                  <span class="sp-ck r-mono">{chipOf(c.cue_type)}</span>
                  <!-- One line unless the cue actually has something extra to say.
                       A subtitle under every row doubled the row height and
                       squeezed the cue name — the one thing an operator scans for. -->
                  <span class="sp-cuebody">
                    <span class="sp-cuetitle" title={c.label}>{c.label}</span>
                    {#if payloadOf(c).stage_note}
                      <span class="sp-cuenote" title={payloadOf(c).stage_note}>
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
                        {payloadOf(c).stage_note}
                      </span>
                    {/if}
                  </span>
                  <span class="sp-dur r-mono">{fmtDuration(c.duration_sec)}</span>
                </div>
              {/each}
            {/each}
          {:else if $readErrors.planItems}
            <!-- RG-95, last two surfaces. `planItems` swallowed to `[]`, so a read
                 that failed said "Empty plan" about a plan the operator spent an
                 evening building. -->
            <ErrorState error={$readErrors.planItems} onRetry={loadItems} />
          {:else}
            <div class="sp-drop r-mono">Empty plan — use ＋ Add Cue.</div>
          {/if}
        </div>
      {:else}
        <div class="rw-panebody sp-addpanel">
          <div class="sp-addsearch">
            <svg class="sp-searchic" viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.9"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
            <!-- svelte-ignore a11y-autofocus -->
            <input placeholder="Search scripture, songs &amp; media to add…" bind:value={addQ} on:input={onAddInput} autofocus />
          </div>
          <div class="sp-cdadd">
            <span class="sp-dot" style="background:{TYPE.countdown.color};"></span>
            <span class="sp-cdlbl">Countdown</span>
            <input class="sp-cdmin" type="number" min="1" max="120" bind:value={cdAddMin} aria-label="Countdown minutes" />
            <span class="sp-cdunit r-mono">min</span>
            <button class="r-btn ghost sm sp-cdgo" on:click={addCountdownCue}>＋ Add</button>
          </div>
          <div class="sp-results">
            {#if addSearching}
              <div class="sp-hint r-mono">Searching…</div>
            {:else if addVerses.length || addSongs.length || addMedia.length || addAnnounce.length}
              {#if addVerses.length}<div class="r-lbl sp-reslbl">Scripture</div>{/if}
              {#each addVerses as v}
                <button class="sp-result r-focus" on:click={() => addVerse(v)}>
                  <span class="sp-dot" style="background:{TYPE.scripture.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{v.reference}</span><span class="sp-restext">{v.text}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
              {#if addSongs.length}<div class="r-lbl sp-reslbl">Songs</div>{/if}
              {#each addSongs as s}
                <button class="sp-result r-focus" on:click={() => addSong(s)}>
                  <span class="sp-dot" style="background:{TYPE.song.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{s.title}</span><span class="sp-restext">{[s.author, s.song_key && `Key ${s.song_key}`, `${s.section_count} slides`].filter(Boolean).join(' · ')}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
              {#if addMedia.length}<div class="r-lbl sp-reslbl">{addQ.trim() ? 'Media' : 'Recent media'}</div>{/if}
              {#each addMedia as m (m.id)}
                <button class="sp-result r-focus" on:click={() => addMediaCue(m)}>
                  <span class="sp-dot" style="background:{TYPE.media.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{m.filename}</span><span class="sp-restext r-mono">{m.kind}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
              {#if addAnnounce.length}<div class="r-lbl sp-reslbl">{addQ.trim() ? 'Announcements' : 'Recent announcements'}</div>{/if}
              {#each addAnnounce as a (a.id)}
                <button class="sp-result r-focus" on:click={() => addAnnounceCue(a)}>
                  <span class="sp-dot" style="background:{TYPE.announce.color};"></span>
                  <span class="sp-resbody"><span class="sp-resref">{a.title || 'Untitled'}</span><span class="sp-restext">{a.body}</span></span>
                  <span class="sp-plus">＋</span>
                </button>
              {/each}
            {:else if addQ.trim()}
              <div class="sp-hint">Nothing found — save scripture, import songs, or add media in the Library.</div>
            {:else}
              <div class="sp-hint r-mono">Type to search scripture, songs and media.</div>
            {/if}
          </div>
        </div>
      {/if}
    {/if}

    <!-- THE CAVEAT. Outside every `{#if}` above, so it is on screen while a plan
         is loading, while none is open, in both left modes and on an empty plan —
         and outside the scroller, so a long running order cannot push it off. It
         is the sentence that says what this whole workspace is, and the two
         places it lived before could each hide it: a toolbar note with a
         `display:none` below 1240px, then a page standfirst that cost two rows of
         a desk. A caveat that can disappear is not a caveat. -->
    <div class="rw-panefoot sp-caveat">
      <p>
        {#if leftMode === 'cues'}Drag <b>⠿</b> to reorder. {/if}Build only — nothing here
        reaches an output. Run it in <b>Live</b>.
      </p>
    </div>
  </section>

  <!-- ══ INSPECTOR: the selected cue ══ -->
  <aside class="rw-pane rw-insp sp-insp">
    {#if !selCue}
      <div class="rw-panehead"><h2 class="rw-panettl">Cue details</h2></div>
      <div class="sp-empty r-empty">Select a cue.</div>
    {:else}
      <!-- `typeOf`, NEVER a bare `TYPE[…]` with a scripture fallback. A cue of a
           kind this build does not recognise was drawn as Scripture — amber dot,
           "SCRIPTURE", the scripture trigger — which is a claim about what will
           reach a screen, made from an absence. `typeOf` says UNKNOWN and claims
           nothing, and being the one door is what stopped this panel badging the
           cue UNKNOWN while `cueSub` printed SCRIPTURE · AUTO-DETECT under it. -->
      {@const ty = typeOf(selCue.cue_type)}
      <!-- One head, one title. The trigger used to be badged up here as well as
           stated in the Fires row below, and two copies of one fact is how they
           come to disagree — this panel has already had a badge say UNKNOWN while
           the line under it said SCRIPTURE · AUTO-DETECT. `Fires` is the copy. -->
      <div class="rw-panehead"><h2 class="rw-panettl">Cue details</h2></div>

      <!-- THE ORDER IS THE POINT (prototype, planner inspector): what this cue
           will LOOK like, then the three things about it you can change, then the
           three that are simply true of it, then the three things you can do to
           it. The panel used to open with a kind badge, a heading and a subtitle
           — three restatements of the row the operator had just clicked — and
           put the rendered preview five fields down, below the fold on a 900px
           window. The preview is the answer to the only question this panel is
           asked on a Tuesday: what does this put on the wall? -->
      <div class="rw-panebody pad sp-inspbody">
        <div class="r-seg sp-insptabs sp-insptabs-top">
          <button class:on={inspTab === 'general'} on:click={() => (inspTab = 'general')}>General</button>
          <button class:on={inspTab === 'slides'} on:click={() => (inspTab = 'slides')}>Slides</button>
          <button class:on={inspTab === 'notes'} on:click={() => (inspTab = 'notes')}>Notes</button>
        </div>

        {#if inspTab === 'general'}
          <div class="sp-preview">
            {#if previewContent?.text}
              <TemplateRender template={selTemplate ?? {}} content={previewContent} />
            {:else}
              <!-- A media or countdown cue has no text to typeset, so the renderer
                   would draw an empty black box that reads as "broken template"
                   rather than "nothing to show". Say which it is. -->
              <div class="sp-nopreview r-mono">
                {selCue.cue_type === 'media' ? 'Media plays full-frame' : 'No text to preview'}
              </div>
            {/if}
          </div>
          <p class="sp-fhelp">
            {#if previewContent?.text}
              Rendered by the same engine as the output screens, so this is what the
              wall will show. Nothing here is on air.
            {:else}
              This cue renders its own content at fire time. Nothing here is on air.
            {/if}
          </p>

          <!-- LABEL is a VALUE, not an input, and that is deliberate rather than
               unfinished. A cue's label is written when the cue is built — from the
               reference, the song and its arrangement, the media filename, the
               announcement's title — and there is no `set_plan_label` command on
               the bridge to write a new one back. A box an operator can type into
               that silently discards what they typed is worse than a line of text:
               it is a control that reports a success it did not achieve. Renaming
               a cue needs a backend command first; until it exists, this says what
               the cue is called and claims nothing else. -->
          <div class="r-lbl sp-flbl">Label</div>
          <div class="sp-fval" title={selCue.label}>{selCue.label}</div>

          <div class="r-lbl sp-flbl">Section</div>
          <input class="r-input sp-fin" bind:this={sectionInput} bind:value={secDraft}
            placeholder="No section — part of the one above"
            on:blur={saveSection} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />

          <div class="r-lbl sp-flbl">Duration</div>
          <input class="r-input sp-fin r-mono" bind:value={durDraft}
            placeholder={selCue.cue_type === 'scripture' ? 'Untimed — fires on cue' : 'e.g. 5 or 4:30'}
            on:blur={saveDuration} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />
          <p class="sp-fhelp">A bare number is minutes. Leave blank for a cue that fires when it is reached rather than on a clock.</p>

          <!-- Name and VALUE rows, the frame's third type role (REBRAND §11). Kind
               and Fires are read-only facts about the cue; Template is the one of
               the three an operator sets, so it keeps its control in the value
               column rather than being demoted to a sentence. -->
          <div class="sp-kv">
            <div class="rw-nv">
              <span class="rw-nvk">Kind</span>
              <span class="rw-nvv">{ty.label}</span>
            </div>
            <div class="rw-nv">
              <span class="rw-nvk">Template</span>
              <span class="rw-nvctl">
                <select class="r-select sp-tplsel" aria-label="Template for this cue"
                  value={selCue.template_id ?? ''} on:change={saveTemplate}>
                  <option value="">Channel default</option>
                  {#each $templates as t (t.id)}
                    <option value={t.id}>{t.name}</option>
                  {/each}
                </select>
              </span>
            </div>
            <div class="rw-nv">
              <span class="rw-nvk">Fires</span>
              <!-- `ty.trig`, never a guess from the kind at this call site: it is
                   the ONE door (`typeOf`), so an unrecognised cue says MANUAL here
                   rather than claiming the auto-detect only scripture has. -->
              <span class="rw-nvv">{ty.trig}</span>
            </div>
          </div>

          <!-- Move up · MOVE DOWN · Delete. "Move down" was missing, and its absence
               was load-bearing once the running order's per-row ↑↓✕ buttons went:
               with only "Move up" on the panel, a cue could be walked towards the
               top of a plan and never back down it without a drag, which is to say
               never at all without a mouse. Duplicate stays — it is the quickest
               route to a second cue of the same shape and nothing else offers it. -->
          <div class="r-lbl sp-flbl">Actions</div>
          <div class="sp-actions">
            <button class="r-btn ghost sm" on:click={duplicateCue}>Duplicate</button>
            <button class="r-btn ghost sm" disabled={items[0]?.id === selCue.id} on:click={(e) => move(selCue.id, -1, e)}>Move up</button>
            <button class="r-btn ghost sm" disabled={items[items.length - 1]?.id === selCue.id} on:click={(e) => move(selCue.id, 1, e)}>Move down</button>
            <button class="r-btn ghost sm sp-raildel" on:click={(e) => remove(selCue.id, e)}>Delete</button>
          </div>
        {:else if inspTab === 'slides'}
          <div class="sp-slidemeta r-mono">
            {#if selCue.cue_type === 'song'}
              {#if payloadOf(selCue).arrangement_stale}
                <!-- The song's sections moved after this cue was built, so the
                     slides below are the song's own order, NOT the arrangement
                     named on the cue. Saying the arrangement's name here would be
                     the badge lying about what is in the plan. -->
                <span class="sp-chip stale">
                  ARRANGEMENT: {(payloadOf(selCue).arrangement_name || 'Standard').toUpperCase()} — NEEDS
                  CHECKING, PLAYING IN THE SONG’S OWN ORDER
                </span>
              {:else}
                <span class="sp-chip">ARRANGEMENT: {(payloadOf(selCue).arrangement_name || 'Standard').toUpperCase()}</span>
              {/if}
            {/if}
            <span class="sp-chip">{selSlides.length} {selCue.cue_type === 'song' ? 'SECTIONS' : 'SLIDE' + (selSlides.length === 1 ? '' : 'S')}</span>
          </div>
          <div class="sp-slides">
            {#each selSlides as s, i}
              <div class="sp-slide">
                <span class="sp-slidetag" style="color:{slideAccent(s.tag)};border-color:{slideAccent(s.tag)}">{s.tag}</span>
                <span class="sp-slidetext">{s.text || s.label}</span>
                <span class="sp-slideidx r-mono">{String(i + 1).padStart(2, '0')}</span>
              </div>
            {/each}
          </div>
        {:else}
          <!-- Named the native way (`for`/`id`) rather than with an aria-label:
               the visible text and the accessible name are then the same string,
               and cannot drift apart. -->
          <label class="r-lbl sp-flbl" for="sp-stage-note">Stage note</label>
          <textarea id="sp-stage-note" class="r-input sp-note" rows="5" bind:value={noteDraft}
            placeholder="Shows on the confidence monitor only, never on the congregation screen."
            on:blur={saveNote}></textarea>
          <p class="sp-fhelp">
            The preacher's monitor shows this beside the cue. It never reaches an
            output screen.
          </p>
        {/if}
      </div>
    {/if}
  </aside>
</WorkspaceFrame>

<!-- arrangement picker — shown when a song with saved arrangements is added -->
{#if arrPick}
  <!-- The backdrop is a mouse convenience, not a control: it is not focusable and
       does not claim to be a button. The keyboard path is Escape, handled at the
       window (top of this file) — bound to this element it only fired when the
       backdrop held focus, which it does not, so Escape fell through to the
       global panic key and cleared the congregation's screens instead. -->
  <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
  <div class="sp-arrback" role="presentation" on:click={() => (arrPick = null)}>
    <div class="sp-arrsheet" role="dialog" aria-modal="true" aria-label="Choose arrangement" use:trapFocus
      on:click|stopPropagation on:keydown|stopPropagation>
      <div class="sp-arrtitle">Add “{arrPick.song.title}”</div>
      <div class="r-lbl sp-arrsub">Choose an arrangement</div>
      <button class="sp-arropt r-focus" on:click={() => commitSong(arrPick.song, null)}>
        <span class="sp-arroptname">Standard</span>
        <span class="sp-arroptseq r-mono">{arrPick.song.sections.length} sections · in order</span>
      </button>
      {#each arrPick.arrangements as a (a.id)}
        <!-- A stale arrangement is offered but not choosable, and it says why.
             Its indices no longer name the sections the operator picked, so
             adding it would put the wrong words in the plan; quietly hiding it
             would leave them hunting for an arrangement they know they made. -->
        <button
          class="sp-arropt r-focus"
          class:stale={a.stale}
          disabled={a.stale}
          on:click={() => commitSong(arrPick.song, a)}>
          <span class="sp-arroptname">{a.name}</span>
          <span class="sp-arroptseq r-mono">{a.sequence.map((i) => (arrPick.song.sections[i]?.tag ?? '?')).join(' · ')}</span>
          {#if a.stale}
            <span class="sp-arrstale">
              The song’s sections changed since this was built — open it in Library →
              Lyrics → Arrangements and check it.
            </span>
          {/if}
        </button>
      {/each}
      <button class="r-btn ghost sm sp-arrcancel" on:click={() => (arrPick = null)}>Cancel</button>
    </div>
  </div>
{/if}

<style>
  /* PLANNER — laid out in the shared workspace grammar (`WorkspaceFrame.svelte`,
     docs/REBRAND.md §2): plans rail · running order · cue inspector, as three
     panes with hairline seams and 8px gutters rather than three floating cards
     with 16px trenches between them. The columns, the type roles and the
     name/value row live in the frame; what is here is what is specific to a plan.

     The plans list used to be a separate full-page step; as a rail it stays put,
     so comparing last week's order with this week's is one click rather than
     three. */

  /* ── rail ── */
  .sp-railsearch{ display:flex; align-items:center; gap:8px; background:var(--v-bg);
    border-bottom:1px solid var(--v-line); padding:0 12px; height:30px; flex:0 0 auto; }
  .sp-railsearch:focus-within{ box-shadow:inset 0 0 0 1px var(--v-accent-line); }
  .sp-railsearch svg{ color:var(--v-faint); flex:0 0 auto; }
  .sp-railsearch input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-size:var(--v-fs-b2); }
  .sp-railsearch input::placeholder{ color:var(--v-faint); }

  .sp-raillist{ display:flex; flex-direction:column; }
  /* A dense row with a seam, not a card with a gutter. Selection is steel blue —
     the thing you are working on — and never amber, which means a congregation is
     looking at something. */
  .sp-railcard{ position:relative; display:flex; flex-direction:column; gap:3px; width:100%; text-align:left;
    padding:7px 12px; background:transparent; border:0; border-bottom:1px solid var(--v-line);
    color:inherit; cursor:pointer; transition:background var(--v-dur) var(--v-ease); }
  .sp-railcard:last-child{ border-bottom:0; }
  .sp-railcard:hover:not(.sel){ background:var(--v-surf2); }
  .sp-railcard.sel{ background:var(--v-sel-soft); box-shadow:inset 2px 0 0 var(--v-sel); }
  .sp-railtitle{ font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt); line-height:1.25;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-railfootline{ display:flex; align-items:center; justify-content:space-between; gap:6px; }
  .sp-railmeta, .sp-railcues{ font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-faint);
    letter-spacing:.02em; }

  .sp-raildel{ color:var(--v-rose); }
  .sp-raildel:hover:not(:disabled){ border-color:var(--v-rose); background:var(--v-rose-soft); }
  .sp-newform{ display:flex; flex-direction:column; gap:6px; }
  .sp-newbtns{ display:flex; gap:6px; }
  .sp-newbtns .r-btn{ flex:1; }

  /* ── main ── */
  /* One head, not a header plus a toolbar: the plan's name, what it costs, and
     the two things you do to it, on the seam that already divides the pane. */
  .sp-panehead{ min-height:38px; padding:5px 12px; gap:10px; flex-wrap:wrap; }
  .sp-plantitle{ flex:0 0 auto; text-transform:none; letter-spacing:var(--v-tr-h2);
    font-size:var(--v-fs-h3); line-height:var(--v-lh-h3); max-width:46ch; }
  .sp-hm{ display:inline-flex; align-items:center; gap:5px; flex:0 0 auto;
    font-size:var(--v-fs-cap); color:var(--v-dim); }
  /* Nothing open yet: the sentence sits in the middle of the space it is talking
     about, the way the Outputs inspector and the Cue Details panel already do.
     Top-left in a 600×750 void read as a stray line of text rather than an
     invitation. The three `.rw-panebody.pad` bodies under this pane are exactly
     the empty, loading and error voids — the running order and the add panel
     carry their own class — so centring them is centring that one sentence. */
  .sp-main > :global(.rw-panebody.pad){ display:flex; }
  .sp-main > :global(.rw-panebody.pad) > :global(.r-empty),
  .sp-main > :global(.rw-panebody.pad) > :global(.es){ margin:auto; text-align:center; max-width:44ch; }
  .sp-toolseg{ flex:0 0 auto; }
  .sp-msg{ font-size:var(--v-fs-lbl); color:var(--v-emerald); max-width:220px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* A FAILURE is rose, never the success-green above — the two must never share
     a colour, or a lost save reads as a completed one. */
  .sp-err{ font-size:var(--v-fs-lbl); color:var(--v-red); max-width:280px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* ── the running order ── */
  /* Rows, not a table. This was an eight-column grid — grip, number, cue, type,
     template, duration, trigger, three buttons — with three media queries
     dropping columns as the pane narrowed, which is a lot of machinery for a list
     whose job is to be read top to bottom. The prototype's row is four things:
     the grip you drag, the KIND in a word, the name, and how long it runs. What
     the columns used to carry has not been lost — the inspector states the
     template, the trigger and the kind in full for the selected cue, which is
     also the only cue any of the three is true of — and a row that fits at every
     width needs no breakpoints at all. */
  .sp-tablewrap{ overflow-y:auto; padding:6px 0; }
  .sp-row{ display:flex; align-items:center; gap:8px; padding:5px 10px; min-height:30px;
    cursor:pointer; user-select:none; background:transparent;
    border:1px solid transparent; border-radius:var(--v-r-sm);
    margin:0 6px 2px; width:calc(100% - 12px);
    transition:background var(--v-dur) var(--v-ease), border-color var(--v-dur) var(--v-ease); }
  .sp-row:hover:not(.sel){ background:var(--v-surf2); border-color:var(--v-line); }
  /* Selection is steel blue — the thing you are working on (docs/REBRAND.md §1).
     It is NOT amber: amber means a cue is live on the wall, and a cue merely
     being edited on a Tuesday is not. */
  .sp-row.sel{ background:var(--v-sel-soft); border-color:var(--v-sel); }
  /* The row in hand: lifted, and above its neighbours as they slide past it.
     `transform` is set from the drag handler, so this must not declare one. */
  .sp-row.dragging{ position:relative; z-index:5; background:var(--v-surf3);
    border-color:var(--v-sel); box-shadow:var(--v-shadow-lg); transition:none; }
  .sp-row.dragging .sp-grip{ cursor:grabbing; }

  .sp-grip{ display:grid; place-items:center; flex:0 0 auto; padding:2px; cursor:grab;
    background:transparent; border:0; border-radius:var(--v-r-sm); color:var(--v-500);
    touch-action:none; /* or the browser scrolls the pane instead of dragging the row */ }
  .sp-row:hover .sp-grip{ color:var(--v-faint); }
  .sp-grip:focus-visible{ outline:2px solid var(--v-sel); outline-offset:1px; color:var(--v-txt); }

  /* The kind chip. One neutral ink for every kind (`TAXONOMY_INK`): the colours a
     per-kind ramp would want are all spoken for by the colour law, and a chip that
     borrows one is a promise nobody made. The WORD is the taxonomy. */
  .sp-ck{ flex:0 0 auto; min-width:52px; text-align:center; font-size:var(--v-fs-cap);
    font-weight:600; letter-spacing:.08em; padding:2px 6px; border-radius:var(--v-r-sm);
    background:var(--v-surf3); border:1px solid var(--v-line);
    /* `--v-dim`, not `--v-faint`: the muted token is 3.76:1 on `--v-surf3` and
       `tokencontrast.test.js` fails the pair on sight. A kind an operator cannot
       read is a chip that is decoration after all. */
    color:var(--v-dim); }
  .sp-row.sel .sp-ck{ color:var(--v-txt); }

  .sp-cuebody{ flex:1; min-width:0; }
  .sp-cuetitle{ display:block; font-size:var(--v-fs-b2); font-weight:500; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-cuenote{ display:flex; align-items:center; gap:4px; margin-top:1px; max-width:100%;
    font-size:var(--v-fs-cap); color:var(--v-accent2); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-cuenote svg{ flex:0 0 auto; }
  .sp-dur{ flex:0 0 auto; font-family:var(--f-mono); font-size:var(--v-fs-cap); color:var(--v-faint);
    font-variant-numeric:tabular-nums; }
  .sp-drop{ padding:22px; text-align:center; font-size:var(--v-fs-b2); color:var(--v-faint); }

  /* A section heading: a caption and a hairline that runs to the right edge. It
     was a sticky bar with an amber rule down its left side — amber, on a build
     surface, for a heading. The line is the furniture; the word is the heading. */
  .sp-sec{ display:flex; align-items:center; gap:8px; padding:10px 12px 4px; }
  .sp-seccap{ flex:0 0 auto; font-family:var(--f-mono); font-size:var(--v-fs-cap);
    line-height:var(--v-lh-cap); font-weight:600; letter-spacing:var(--v-tr-caps);
    text-transform:uppercase; color:var(--v-faint); }
  .sp-secln{ flex:1; height:1px; background:var(--v-line); }

  /* THE CAVEAT — a pane footer, never a media query. See the markup. */
  .sp-caveat{ padding:8px 12px; }
  .sp-caveat p{ margin:0; font-size:var(--v-fs-cap); line-height:var(--v-lh-cap); color:var(--v-faint); }
  .sp-caveat b{ color:var(--v-dim); font-weight:600; }

  /* ── add panel ── */
  .sp-addpanel{ overflow-y:auto; }
  .sp-addsearch{ display:flex; align-items:center; gap:9px; background:var(--v-bg);
    border-bottom:1px solid var(--v-line); padding:0 12px; height:34px; }
  .sp-addsearch:focus-within{ box-shadow:inset 0 0 0 1px var(--v-accent-line); }
  .sp-searchic{ color:var(--v-faint); flex:0 0 auto; }
  .sp-addsearch input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-size:var(--v-fs-b2); }
  .sp-addsearch input::placeholder{ color:var(--v-faint); }
  .sp-reslbl{ padding:8px 12px 5px; background:var(--v-bg); border-bottom:1px solid var(--v-line); }
  .sp-hint{ font-size:var(--v-fs-b2); color:var(--v-faint); padding:10px 12px; }

  .sp-cdadd{ display:flex; align-items:center; gap:8px; padding:7px 12px;
    border-bottom:1px solid var(--v-line); background:var(--v-surf2); }
  .sp-cdlbl{ font-size:var(--v-fs-b2); color:var(--v-txt); }
  .sp-cdmin{ width:52px; padding:3px 6px; border-radius:var(--v-r-sm); border:1px solid var(--v-line2);
    background:var(--v-surf); color:var(--v-txt); font-family:var(--f-mono); font-size:var(--v-fs-b2); text-align:center; }
  .sp-cdunit{ font-size:var(--v-fs-lbl); color:var(--v-faint); margin-left:-3px; }
  .sp-cdgo{ margin-left:auto; }

  /* The add panel's kind dot. One neutral for every kind, like the row chip: the
     heading above each group names the kind, and a dot that borrowed a colour
     would be the taxonomy painting a promise again. */
  .sp-dot{ width:6px; height:6px; border-radius:2px; flex:0 0 auto; }

  .sp-results{ display:flex; flex-direction:column; }
  .sp-result{ display:flex; align-items:flex-start; gap:9px; width:100%; padding:8px 12px;
    background:transparent; border:0; border-bottom:1px solid var(--v-line);
    color:var(--v-txt); cursor:pointer; text-align:left;
    transition:background var(--v-dur) var(--v-ease); }
  .sp-result:hover{ background:var(--v-surf2); }
  .sp-result .sp-dot{ margin-top:5px; }
  .sp-resbody{ flex:1; min-width:0; }
  .sp-resref{ display:block; font-family:var(--f-head); font-weight:600; font-size:var(--v-fs-b2); color:var(--v-txt); }
  .sp-restext{ font-size:var(--v-fs-cap); color:var(--v-dim); line-height:1.45; margin-top:2px;
    display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-plus{ color:var(--v-accent); font-family:var(--f-mono); font-weight:700; flex:0 0 auto; }

  /* ── inspector ── */
  .sp-inspbody{ padding:12px; }
  .sp-insptabs{ margin:14px 0 4px; width:100%; }
  .sp-insptabs :global(button){ flex:1; }
  /* The tab strip is the first thing in the body now that the kind badge, the
     heading and the subtitle — three restatements of the row already selected in
     the running order — are gone, so it does not need a 14px run-up. */
  .sp-insptabs-top{ margin-top:0; margin-bottom:10px; }

  .sp-flbl{ margin:14px 0 6px; }
  .sp-fin{ width:100%; }
  /* A FIELD-SHAPED VALUE, deliberately not an input. It lines up with the boxes
     above and below it and it is plainly not one of them: no border, no cursor,
     nothing to click. A disabled input in this slot would look like a box that
     had stopped working rather than a fact. */
  .sp-fval{ font-size:var(--v-fs-b2); line-height:var(--v-lh-b2); color:var(--v-txt);
    padding:2px 0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* The three name/value rows. They carry the frame's `.rw-nv` grammar, so the
     seams and the right-aligned mono value are the same ones the Outputs and
     Settings desks use — hence the surrounding box rather than a restatement of
     the row itself. */
  .sp-kv{ margin-top:16px; border:1px solid var(--v-line); border-radius:var(--v-r-sm);
    background:var(--v-surf2); overflow:hidden; }
  .sp-tplsel{ max-width:172px; }
  .sp-fhelp{ margin:6px 0 0; font-size:var(--v-fs-cap); line-height:1.45; color:var(--v-faint); }
  .sp-note{ width:100%; resize:vertical; font-family:inherit; line-height:1.45; }

  /* The preview is 16:9 because every output Relay drives is. A preview at a
     different aspect than the wall is a preview that lies about line breaks. */
  /* `position:relative` is load-bearing: TemplateRender's root is
     `position:absolute; inset:0`, so without a positioned ancestor the preview
     escapes this box and lays itself out against the page — which reads as a
     dead black rectangle here and a mystery elsewhere. It also supplies its own
     `container-type:size` for the cqw units, so this element must not. */
  .sp-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-sm);
    border:1px solid var(--v-line2); overflow:hidden; background:var(--v-void);
    display:grid; place-items:center; }
  .sp-nopreview{ font-size:var(--v-fs-cap); color:var(--v-500); letter-spacing:.04em; }

  .sp-actions{ display:flex; flex-wrap:wrap; gap:6px; }
  .sp-actions .r-btn{ flex:1 1 auto; justify-content:center; }

  .sp-slidemeta{ display:flex; flex-wrap:wrap; gap:6px; margin:12px 0 10px; }
  .sp-chip{ font-size:var(--v-fs-cap); letter-spacing:.05em; color:var(--v-dim); padding:3px 8px;
    border-radius:var(--v-r-sm); background:var(--v-surf2); border:1px solid var(--v-line2); white-space:nowrap; }
  .sp-slides{ display:flex; flex-direction:column; gap:6px; }
  .sp-slide{ position:relative; border-radius:var(--v-r-sm); border:1px solid var(--v-line);
    background:var(--v-surf2); padding:9px 12px 9px 42px; min-height:46px;
    display:flex; align-items:center; }
  .sp-slidetag{ position:absolute; left:10px; top:9px; font-family:var(--f-mono); font-size:var(--v-fs-cap); font-weight:700;
    letter-spacing:.06em; padding:2px 5px; border-radius:var(--v-r-sm); border:1px solid currentColor; }
  .sp-slidetext{ font-size:var(--v-fs-b2); line-height:1.45; color:var(--v-dim); white-space:pre-line;
    display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-slideidx{ position:absolute; right:10px; bottom:7px; font-size:var(--v-fs-cap); color:var(--v-500); }

  .sp-empty{ margin:auto; padding:24px; text-align:center; }

  /* ── arrangement picker ── */
  .sp-arrback{ position:fixed; inset:0; background:rgba(6,6,8,.6); backdrop-filter:blur(3px); z-index:200;
    display:flex; align-items:center; justify-content:center; padding:24px; }
  .sp-arrsheet{ width:min(400px, 92%); max-height:80%; overflow:auto; background:var(--v-surf);
    border:1px solid var(--v-line2); border-radius:var(--v-r-xl); padding:18px;
    display:flex; flex-direction:column; gap:8px; box-shadow:var(--v-shadow-lg); }
  .sp-arrtitle{ font-family:var(--f-head); font-weight:600; font-size:var(--v-fs-h3); color:var(--v-txt); }
  .sp-arrsub{ margin:2px 0 6px; }
  .sp-arropt{ display:flex; flex-direction:column; gap:3px; width:100%; text-align:left; padding:10px 12px;
    border-radius:var(--v-r-sm); background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt);
    cursor:pointer; transition:.12s; }
  .sp-arropt:hover{ border-color:var(--v-accent); background:var(--v-accent-soft); }
  /* Rose, never amber: this is a thing that is wrong, not a thing that is live
     (DECISIONS §22). */
  .sp-arropt.stale{ border-color:var(--v-rose,#e0526a); opacity:.75; cursor:not-allowed; }
  .sp-arropt.stale:hover{ border-color:var(--v-rose,#e0526a); background:transparent; }
  .sp-arrstale{ font-size:var(--v-fs-lbl); color:var(--v-rose,#e0526a); }
  .sp-chip.stale{ color:var(--v-rose,#e0526a); border-color:currentColor; }
  .sp-arroptname{ font-weight:600; font-size:var(--v-fs-b1); }
  .sp-arroptseq{ font-size:var(--v-fs-cap); letter-spacing:.03em; color:var(--v-faint);
    white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-arrcancel{ align-self:flex-end; margin-top:4px; }
</style>
