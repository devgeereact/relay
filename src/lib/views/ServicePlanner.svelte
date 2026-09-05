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
    payloadOf,
    slidesOf,
    slideAccent,
    cueSub,
    sectionsOf,
    planRuntime,
    fmtDuration,
    parseDuration,
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

  // Templates are loaded here, not assumed. The Planner names a cue's template in
  // the running order and offers the picker in the inspector; without this the
  // store is empty on a cold open of this tab and every cue reads "Template 4".
  onMount(() => {
    refresh().finally(() => (loading = false));
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

  // Drag-reorder the cue list.
  let dragId = null;
  let dragOverId = null;
  function onDragStart(id, e) {
    dragId = id;
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }
  function onDropCue(targetId) {
    const from = items.findIndex((i) => i.id === dragId);
    const to = items.findIndex((i) => i.id === targetId);
    dragId = null;
    dragOverId = null;
    if (from < 0 || to < 0 || from === to) return;
    const arr = items.slice();
    const [moved] = arr.splice(from, 1);
    arr.splice(to, 0, moved);
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

  /** Hand this plan to the LIVE tab and go there. The one path from build to run. */
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

  /** The template a cue renders with, or the honest fallback. */
  function templateName(id) {
    if (id == null) return 'Channel default';
    return $templates.find((t) => t.id === id)?.name ?? `Template ${id}`;
  }
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
<svelte:window on:keydown={(e) => arrPick && e.key === 'Escape' && (arrPick = null)} />

<div class="sp-shell">
  <!-- ══ RAIL: every plan, always reachable ══ -->
  <aside class="sp-rail">
    <div class="r-lbl sp-raillbl">Service Plans</div>
    <div class="sp-railsearch">
      <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
      <input placeholder="Search plans…" bind:value={planQ} aria-label="Search plans" />
    </div>

    <div class="sp-raillist r-scroll">
      {#if loading}
        <Loading compact what="plans" />
      {:else if railPlans.length}
        {#each railPlans as p (p.id)}
          <button class="sp-railcard r-focus" class:sel={openPlan?.id === p.id} on:click={() => open(p)}>
            <span class="sp-railtitle">{p.title}</span>
            <span class="sp-railfootline">
              <span class="sp-railmeta r-mono">{p.plan_date || 'No date'}</span>
              <span class="sp-railcues r-mono">{p.cue_count} cue{p.cue_count === 1 ? '' : 's'}</span>
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

    <div class="sp-railfoot">
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
  <section class="sp-main">
    {#if !$capture.available}
      <div class="sp-offline"><span class="r-badge rose"><span class="bd"></span>Backend not attached — plans need the desktop app</span></div>
    {/if}

    {#if loading}
      <Loading what="plans" />
    {:else if !openPlan && !plans.length && $readErrors.listPlans}
      <ErrorState error={$readErrors.listPlans} onRetry={refresh} />
    {:else if !openPlan}
      <EmptyState message={plans.length ? 'Pick a plan on the left to open it.' : 'No plans yet — create one to start building a service.'} />
    {:else}
      <header class="sp-head">
        <div class="sp-headmain">
          <h2 class="sp-plantitle">{openPlan.title}</h2>
          <div class="sp-headmeta r-mono">
            <span class="sp-hm">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M16 3v4M8 3v4M3 11h18"/></svg>
              {openPlan.plan_date || 'No date'}
            </span>
            <!-- "(est.)" is not decoration. Most plans contain a scripture cue,
                 which is untimed by nature, so the sum is a floor and never the
                 service length. Presenting a partial total as a real one is how a
                 service runs long. -->
            <span class="sp-hm">
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>
              {items.length} cue{items.length === 1 ? '' : 's'} · {fmtDuration(runtime.seconds, true)}{runtime.partial ? ' (est.)' : ''}
            </span>
          </div>
        </div>
        {#if err}<span class="sp-err r-mono" role="alert">{err}</span>
        {:else if msg}<span class="sp-msg r-mono">{msg}</span>{/if}
        <!-- The ONLY path from build to run. Nothing on this screen reaches an
             output — an operator arranging next Sunday's songs on a Tuesday must
             not be able to put one on the wall by clicking the wrong thing. -->
        <button class="r-btn primary sm" on:click={runPlan} disabled={!items.length}>
          Run in Live
          <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
        </button>
      </header>

      <div class="sp-toolbar">
        <div class="r-seg sp-toolseg">
          <button class:on={leftMode === 'cues'} on:click={() => (leftMode = 'cues')}>Running Order</button>
          <button class:on={leftMode === 'add'} on:click={() => { leftMode = 'add'; if (!addQ.trim()) { addMedia = allMedia.slice(0, 8); addAnnounce = allAnnounce.slice(0, 8); } }}>＋ Add Cue</button>
        </div>
        <button class="r-btn ghost sm" disabled={!items.length} on:click={addSection}>＋ Add Section</button>
        <span class="sp-spring"></span>
        <span class="r-lbl sp-toolnote">Build only — never reaches an output</span>
      </div>

      {#if leftMode === 'cues'}
        <div class="sp-tablewrap r-scroll">
          {#if items.length}
            <div class="sp-thead r-lbl">
              <span></span>
              <span class="sp-th-n">#</span>
              <span>Cue</span>
              <span>Type</span>
              <span class="sp-th-tpl">Template</span>
              <span class="sp-th-r">Duration</span>
              <span class="sp-th-tg">Trigger</span>
              <span></span>
            </div>

            {#each sections as sec (sec.items[0].id)}
              {#if sec.title}
                <div class="sp-section">
                  <span class="sp-secbar"></span>
                  <span class="sp-sectitle">{sec.title}</span>
                  <span class="sp-secmeta r-mono">
                    {fmtDuration(sec.seconds)}{sec.timed ? '' : '+'} · {sec.items.length} cue{sec.items.length === 1 ? '' : 's'}
                  </span>
                </div>
              {/if}

              {#each sec.items as c (c.id)}
                {@const ty = TYPE[c.cue_type] || TYPE.scripture}
                {@const n = items.findIndex((i) => i.id === c.id)}
                <div class="sp-row" class:sel={c.id === selId} class:dragover={dragOverId === c.id}
                  draggable={true}
                  on:dragstart={(e) => onDragStart(c.id, e)}
                  on:dragover|preventDefault={() => (dragOverId = c.id)}
                  on:dragleave={() => { if (dragOverId === c.id) dragOverId = null; }}
                  on:drop|preventDefault={() => onDropCue(c.id)}
                  on:click={() => (selId = c.id)} role="button" tabindex="0"
                  on:keydown={(e) => {
                    // A role="button" must answer to Enter AND Space; this one only
                    // took Enter, so it was focusable but half-operable. preventDefault
                    // on Space, or the page scrolls under the operator instead.
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.preventDefault();
                      selId = c.id;
                    }
                  }}>
                  <span class="sp-grip" aria-hidden="true">
                    <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor"><circle cx="2" cy="2" r="1.1"/><circle cx="8" cy="2" r="1.1"/><circle cx="2" cy="7" r="1.1"/><circle cx="8" cy="7" r="1.1"/><circle cx="2" cy="12" r="1.1"/><circle cx="8" cy="12" r="1.1"/></svg>
                  </span>
                  <span class="sp-num r-mono">{n + 1}</span>
                  <!-- One line unless the cue actually has something extra to say.
                       A subtitle under every row (the cue type, which the TYPE
                       column already states) doubled the row height and squeezed
                       the cue name — the one thing an operator scans for. -->
                  <span class="sp-cuebody">
                    <span class="sp-cuetitle" title={c.label}>{c.label}</span>
                    {#if payloadOf(c).stage_note}
                      <span class="sp-cuenote" title={payloadOf(c).stage_note}>
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
                        {payloadOf(c).stage_note}
                      </span>
                    {/if}
                  </span>
                  <span class="sp-ty r-mono" style="color:{ty.color};">
                    <span class="sp-dot" style="background:{ty.color};"></span>{ty.label}
                  </span>
                  <span class="sp-tpl r-mono" class:sp-inherit={c.template_id == null}>{templateName(c.template_id)}</span>
                  <span class="sp-dur r-mono">{fmtDuration(c.duration_sec)}</span>
                  <!-- The reference's STATUS column reads UP NEXT / PENDING / AUTO.
                       Those are RUN states and this screen cannot know them — on a
                       Tuesday nothing is up next. What IS true at build time is how
                       the cue will be triggered, which is what this column shows. -->
                  <span class="sp-tg r-mono">{ty.trig}</span>
                  <span class="sp-rowbtns">
                    <button class="sp-mini" title="Move up" disabled={n === 0} on:click={(e) => move(c.id, -1, e)}>↑</button>
                    <button class="sp-mini" title="Move down" disabled={n === items.length - 1} on:click={(e) => move(c.id, 1, e)}>↓</button>
                    <button class="sp-mini danger" title="Remove cue" on:click={(e) => remove(c.id, e)}>✕</button>
                  </span>
                </div>
              {/each}
            {/each}
          {:else}
            <div class="sp-drop r-mono">Empty plan — use ＋ Add Cue.</div>
          {/if}
        </div>
      {:else}
        <div class="sp-addpanel r-scroll">
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
  </section>

  <!-- ══ INSPECTOR: the selected cue ══ -->
  <aside class="sp-insp">
    {#if !selCue}
      <div class="sp-insphead"><span class="sp-inspttl">Cue Details</span></div>
      <div class="sp-empty r-empty">Pick a cue to edit it.</div>
    {:else}
      {@const ty = TYPE[selCue.cue_type] || TYPE.scripture}
      <div class="sp-insphead">
        <span class="sp-inspttl">Cue Details</span>
        <span class="sp-inspttrig r-mono">{ty.trig}</span>
      </div>

      <div class="sp-inspbody r-scroll">
        <div class="sp-insptype r-mono" style="color:{ty.color};">
          <span class="sp-dot" style="background:{ty.color};"></span>{ty.label}
        </div>
        <h3 class="sp-inspname">{selCue.label}</h3>
        <div class="sp-inspsub r-mono">{cueSub(selCue)}</div>

        <div class="r-seg sp-insptabs">
          <button class:on={inspTab === 'general'} on:click={() => (inspTab = 'general')}>General</button>
          <button class:on={inspTab === 'slides'} on:click={() => (inspTab = 'slides')}>Slides</button>
          <button class:on={inspTab === 'notes'} on:click={() => (inspTab = 'notes')}>Notes</button>
        </div>

        {#if inspTab === 'general'}
          <div class="r-lbl sp-flbl">Section</div>
          <input class="r-input sp-fin" bind:this={sectionInput} bind:value={secDraft}
            placeholder="No section — part of the one above"
            on:blur={saveSection} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />

          <div class="r-lbl sp-flbl">Template</div>
          <select class="r-select sp-fin" value={selCue.template_id ?? ''} on:change={saveTemplate}>
            <option value="">Channel default</option>
            {#each $templates as t (t.id)}
              <option value={t.id}>{t.name}</option>
            {/each}
          </select>

          <div class="r-lbl sp-flbl">Duration</div>
          <input class="r-input sp-fin r-mono" bind:value={durDraft}
            placeholder={selCue.cue_type === 'scripture' ? 'Untimed — fires on cue' : 'e.g. 5 or 4:30'}
            on:blur={saveDuration} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />
          <p class="sp-fhelp">A bare number is minutes. Leave blank for a cue that fires when it is reached rather than on a clock.</p>

          <div class="r-lbl sp-flbl">Preview</div>
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

          <div class="r-lbl sp-flbl">Actions</div>
          <div class="sp-actions">
            <button class="r-btn ghost sm" on:click={duplicateCue}>Duplicate</button>
            <button class="r-btn ghost sm" disabled={items[0]?.id === selCue.id} on:click={(e) => move(selCue.id, -1, e)}>Move up</button>
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
</div>

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
  /* Three columns: plans · running order · inspector. Each scrolls internally so
     the running order never pushes the rail or the inspector off screen. */
  .sp-shell{ display:grid; grid-template-columns:206px minmax(0,1fr) 340px; gap:var(--v-sp-md);
    height:100%; min-height:0; }
  @media (max-width:1280px){ .sp-shell{ grid-template-columns:186px minmax(0,1fr) 300px; gap:12px; } }
  @media (max-width:1020px){ .sp-shell{ grid-template-columns:1fr; height:auto; } }

  /* ── rail ── */
  .sp-rail{ display:flex; flex-direction:column; min-height:0; gap:10px;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); padding:13px 11px; }
  .sp-raillbl{ padding:0 2px; }
  .sp-railsearch{ display:flex; align-items:center; gap:8px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-md); padding:0 10px; height:32px; flex:0 0 auto; }
  .sp-railsearch:focus-within{ border-color:var(--v-accent-line); box-shadow:0 0 0 3px var(--v-accent-soft); }
  .sp-railsearch svg{ color:var(--v-faint); flex:0 0 auto; }
  .sp-railsearch input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-size:var(--v-fs-b2); }
  .sp-railsearch input::placeholder{ color:var(--v-faint); }

  .sp-raillist{ flex:1; min-height:0; overflow-y:auto; display:flex; flex-direction:column; gap:6px; }
  .sp-railcard{ position:relative; display:flex; flex-direction:column; gap:5px; width:100%; text-align:left;
    padding:10px 11px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line);
    color:inherit; cursor:pointer; transition:border-color .12s, background .12s; }
  .sp-railcard:hover{ border-color:var(--v-line2); }
  .sp-railcard.sel{ border-color:var(--v-accent-line); background:var(--v-accent-soft); }
  .sp-railtitle{ font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt); line-height:1.25;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-railfootline{ display:flex; align-items:center; justify-content:space-between; gap:6px; }
  .sp-railmeta, .sp-railcues{ font-size:var(--v-fs-cap); color:var(--v-faint); letter-spacing:.02em; }

  .sp-railfoot{ display:flex; flex-direction:column; gap:6px; flex:0 0 auto; padding-top:10px;
    border-top:1px solid var(--v-line); }
  .sp-railfoot .r-btn{ width:100%; justify-content:center; }
  .sp-raildel{ color:var(--v-rose); }
  .sp-raildel:hover:not(:disabled){ border-color:var(--v-rose); background:var(--v-rose-soft); }
  .sp-newform{ display:flex; flex-direction:column; gap:6px; }
  .sp-newbtns{ display:flex; gap:6px; }
  .sp-newbtns .r-btn{ flex:1; }

  /* ── main ── */
  .sp-main{ display:flex; flex-direction:column; min-height:0; gap:12px; }
  .sp-offline{ flex:0 0 auto; }

  .sp-head{ display:flex; align-items:flex-start; gap:14px; flex:0 0 auto; }
  .sp-headmain{ flex:1; min-width:0; }
  .sp-plantitle{ margin:0; font-family:var(--f-head); font-size:var(--v-fs-h1); line-height:var(--v-lh-h1);
    letter-spacing:var(--v-tr-tight); font-weight:600; color:var(--v-txt); }
  .sp-headmeta{ display:flex; align-items:center; gap:14px; margin-top:5px; flex-wrap:wrap; }
  .sp-hm{ display:inline-flex; align-items:center; gap:5px; font-size:var(--v-fs-lbl); color:var(--v-dim); }
  .sp-hm svg{ color:var(--v-faint); flex:0 0 auto; }
  .sp-msg{ font-size:var(--v-fs-lbl); color:var(--v-emerald); max-width:220px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* A FAILURE is rose, never the success-green above — the two must never share
     a colour, or a lost save reads as a completed one. */
  .sp-err{ font-size:var(--v-fs-lbl); color:var(--v-red); max-width:280px;
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .sp-toolbar{ display:flex; align-items:center; gap:8px; flex:0 0 auto; }
  .sp-spring{ flex:1; }
  /* Never wraps to a second line — it is a standing caveat, not a message, and a
     two-line caveat pushes the running order down the screen. */
  .sp-toolnote{ color:var(--v-faint); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  @media (max-width:1240px){ .sp-toolnote{ display:none; } }
  .sp-toolseg{ flex:0 0 auto; }

  /* ── the running order table ── */
  /* Breakpoints are derived from the TABLE's width, not the viewport's: this
     column is the viewport minus the nav sidebar (236), the plans rail (206), the
     inspector (340) and three gaps — about 814px of fixed chrome. Sized off the
     raw viewport, the table kept all eight columns at 1536px inside a ~690px box
     and overflowed, clipping the row buttons off the right edge and running the
     DURATION and TRIGGER headings together.
     (`@container` states that intent directly, but esbuild's CSS minifier cannot
     parse it and silently emitted broken rules — dev looked right, the packaged
     build would not have been.) */
  .sp-tablewrap{ flex:1; min-height:0; overflow-y:auto; background:var(--v-surf);
    border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  /* The cue name is what an operator scans; it gets the flexible column and a
     floor, and every other column is sized to its content so the name is never
     the one that collapses. */
  .sp-thead, .sp-row{ display:grid;
    grid-template-columns:18px 22px minmax(160px,1fr) 96px 122px 62px 88px 78px;
    align-items:center; gap:8px; padding:0 10px; }
  .sp-thead{ height:30px; position:sticky; top:0; z-index:2; background:var(--v-surf);
    border-bottom:1px solid var(--v-line); color:var(--v-faint); }
  .sp-th-n{ text-align:center; }
  .sp-th-r{ text-align:right; }
  /* Trigger goes first, then Template — both are stated in full in the inspector
     for the selected cue, so neither is the last copy of anything. */
  @media (max-width:1600px){
    .sp-thead, .sp-row{ grid-template-columns:18px 22px minmax(150px,1fr) 92px 172px 60px 78px; }
    .sp-tg, .sp-th-tg{ display:none; }
  }
  @media (max-width:1360px){
    .sp-thead, .sp-row{ grid-template-columns:18px 22px minmax(130px,1fr) 96px 62px 78px; }
    .sp-tpl, .sp-th-tpl{ display:none; }
  }
  /* Below the three-column break the table has the whole width back, so both
     columns return. */
  @media (max-width:1020px){
    .sp-thead, .sp-row{ grid-template-columns:18px 22px minmax(160px,1fr) 96px 122px 62px 88px 78px; }
    .sp-tg, .sp-th-tg, .sp-tpl, .sp-th-tpl{ display:block; }
  }

  .sp-row{ min-height:36px; border-bottom:1px solid var(--v-line); cursor:pointer;
    transition:background .12s, box-shadow .12s; }
  .sp-row:last-child{ border-bottom:0; }
  .sp-row:hover{ background:var(--v-surf2); }
  /* Selection is amethyst — the app's accent. It is NOT amber: amber means a cue
     is live on the wall, and a cue merely being edited on a Tuesday is not. */
  .sp-row.sel{ background:var(--v-accent-soft); box-shadow:inset 3px 0 0 var(--v-accent); }
  .sp-row.dragover{ box-shadow:inset 0 2px 0 var(--v-accent); }
  .sp-grip{ color:var(--v-500); cursor:grab; display:grid; place-items:center; }
  .sp-row:hover .sp-grip{ color:var(--v-faint); }
  .sp-num{ font-size:var(--v-fs-lbl); color:var(--v-faint); text-align:center; }
  .sp-cuebody{ min-width:0; }
  .sp-cuetitle{ display:block; font-size:var(--v-fs-b2); font-weight:500; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-cuenote{ display:flex; align-items:center; gap:4px; margin-top:1px; max-width:100%;
    font-size:var(--v-fs-cap); color:var(--v-accent2); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-cuenote svg{ flex:0 0 auto; }
  .sp-ty{ display:inline-flex; align-items:center; gap:6px; font-size:var(--v-fs-cap); letter-spacing:.06em; }
  .sp-dot{ width:6px; height:6px; border-radius:2px; flex:0 0 auto; }
  .sp-tpl{ font-size:var(--v-fs-cap); color:var(--v-dim);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-inherit{ color:var(--v-faint); font-style:italic; }
  .sp-dur{ font-size:var(--v-fs-lbl); color:var(--v-dim); text-align:right; font-variant-numeric:tabular-nums; }
  .sp-tg{ font-size:var(--v-fs-cap); color:var(--v-faint); letter-spacing:.05em; }
  .sp-rowbtns{ display:flex; gap:4px; justify-content:flex-end; opacity:0; transition:opacity .12s; }
  .sp-row:hover .sp-rowbtns, .sp-row.sel .sp-rowbtns, .sp-row:focus-within .sp-rowbtns{ opacity:1; }
  .sp-mini{ width:22px; height:22px; border-radius:var(--v-r-sm); display:grid; place-items:center; cursor:pointer;
    font-size:11px; background:var(--v-surf3); border:1px solid var(--v-line); color:var(--v-dim); }
  .sp-mini:hover:not(:disabled){ color:var(--v-accent); border-color:var(--v-line2); }
  .sp-mini.danger:hover:not(:disabled){ color:var(--v-rose); border-color:var(--v-rose); }
  .sp-mini:disabled{ opacity:.3; cursor:not-allowed; }
  .sp-drop{ padding:22px; text-align:center; font-size:var(--v-fs-b2); color:var(--v-faint); }

  /* Section header row. Amber bar = the reference's own accent for a section, and
     it is safe here: it is a heading in a build tool, not a live-state indicator
     on a cue. */
  .sp-section{ display:flex; align-items:center; gap:10px; padding:8px 10px 6px;
    background:var(--v-bg); border-bottom:1px solid var(--v-line); position:sticky; top:30px; z-index:1; }
  .sp-secbar{ width:3px; height:15px; border-radius:2px; background:var(--v-amber); flex:0 0 auto; }
  .sp-sectitle{ font-size:var(--v-fs-lbl); font-weight:600; letter-spacing:var(--v-tr-wide);
    text-transform:uppercase; color:var(--v-txt); }
  .sp-secmeta{ margin-left:auto; font-size:var(--v-fs-cap); color:var(--v-faint); }

  /* ── add panel ── */
  .sp-addpanel{ flex:1; min-height:0; overflow-y:auto; padding:12px; background:var(--v-surf);
    border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  .sp-addsearch{ display:flex; align-items:center; gap:9px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-md); padding:0 11px; height:38px; }
  .sp-addsearch:focus-within{ border-color:var(--v-accent-line); box-shadow:0 0 0 3px var(--v-accent-soft); }
  .sp-searchic{ color:var(--v-faint); flex:0 0 auto; }
  .sp-addsearch input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-size:var(--v-fs-b1); }
  .sp-addsearch input::placeholder{ color:var(--v-faint); }
  .sp-reslbl{ margin:10px 0 2px; }
  .sp-hint{ font-size:var(--v-fs-b2); color:var(--v-faint); padding:6px 2px; }

  .sp-cdadd{ display:flex; align-items:center; gap:8px; margin-top:8px; padding:8px 10px;
    border:1px solid var(--v-line); border-radius:var(--v-r-md); background:var(--v-surf2); }
  .sp-cdlbl{ font-size:var(--v-fs-b1); color:var(--v-txt); }
  .sp-cdmin{ width:52px; padding:4px 6px; border-radius:var(--v-r-sm); border:1px solid var(--v-line2);
    background:var(--v-surf); color:var(--v-txt); font-family:var(--f-mono); font-size:var(--v-fs-b2); text-align:center; }
  .sp-cdunit{ font-size:var(--v-fs-lbl); color:var(--v-faint); margin-left:-3px; }
  .sp-cdgo{ margin-left:auto; }

  .sp-results{ display:flex; flex-direction:column; gap:6px; margin-top:8px; }
  .sp-result{ display:flex; align-items:flex-start; gap:9px; width:100%; padding:9px 10px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt); cursor:pointer; text-align:left; }
  .sp-result:hover{ border-color:var(--v-line2); }
  .sp-result .sp-dot{ margin-top:5px; }
  .sp-resbody{ flex:1; min-width:0; }
  .sp-resref{ display:block; font-family:var(--f-head); font-weight:600; font-size:var(--v-fs-b1); color:var(--v-txt); }
  .sp-restext{ font-size:var(--v-fs-b2); color:var(--v-dim); line-height:1.4; margin-top:2px;
    display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-plus{ color:var(--v-accent); font-family:var(--f-mono); font-weight:700; flex:0 0 auto; }

  /* ── inspector ── */
  .sp-insp{ display:flex; flex-direction:column; min-height:0;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); overflow:hidden; }
  .sp-insphead{ display:flex; align-items:center; justify-content:space-between; gap:10px;
    padding:12px 14px; border-bottom:1px solid var(--v-line); flex:0 0 auto; }
  .sp-inspttl{ font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .sp-inspttrig{ font-size:var(--v-fs-cap); letter-spacing:.06em; color:var(--v-faint);
    padding:3px 8px; border-radius:99px; background:var(--v-surf2); border:1px solid var(--v-line2); }
  .sp-inspbody{ flex:1; min-height:0; overflow-y:auto; padding:14px; }
  .sp-insptype{ display:inline-flex; align-items:center; gap:6px; font-size:var(--v-fs-cap); letter-spacing:.06em; }
  .sp-inspname{ margin:6px 0 2px; font-family:var(--f-head); font-size:var(--v-fs-h2); line-height:var(--v-lh-h2);
    letter-spacing:var(--v-tr-h2); font-weight:600; color:var(--v-txt); }
  .sp-inspsub{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .sp-insptabs{ margin:14px 0 4px; width:100%; }
  .sp-insptabs :global(button){ flex:1; }

  .sp-flbl{ margin:14px 0 6px; }
  .sp-fin{ width:100%; }
  .sp-fhelp{ margin:6px 0 0; font-size:var(--v-fs-cap); line-height:1.45; color:var(--v-faint); }
  .sp-note{ width:100%; resize:vertical; font-family:inherit; line-height:1.45; }

  /* The preview is 16:9 because every output Relay drives is. A preview at a
     different aspect than the wall is a preview that lies about line breaks. */
  /* `position:relative` is load-bearing: TemplateRender's root is
     `position:absolute; inset:0`, so without a positioned ancestor the preview
     escapes this box and lays itself out against the page — which reads as a
     dead black rectangle here and a mystery elsewhere. It also supplies its own
     `container-type:size` for the cqw units, so this element must not. */
  .sp-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md);
    border:1px solid var(--v-line2); overflow:hidden; background:var(--v-void);
    display:grid; place-items:center; }
  .sp-nopreview{ font-size:var(--v-fs-cap); color:var(--v-500); letter-spacing:.04em; }

  .sp-actions{ display:flex; flex-wrap:wrap; gap:6px; }
  .sp-actions .r-btn{ flex:1 1 auto; justify-content:center; }

  .sp-slidemeta{ display:flex; flex-wrap:wrap; gap:6px; margin:12px 0 10px; }
  .sp-chip{ font-size:var(--v-fs-cap); letter-spacing:.05em; color:var(--v-dim); padding:4px 9px;
    border-radius:var(--v-r-sm); background:var(--v-surf2); border:1px solid var(--v-line2); white-space:nowrap; }
  .sp-slides{ display:flex; flex-direction:column; gap:8px; }
  .sp-slide{ position:relative; border-radius:var(--v-r-md); border:1px solid var(--v-line);
    background:var(--v-surf2); padding:10px 12px 10px 44px; min-height:52px;
    display:flex; align-items:center; }
  .sp-slidetag{ position:absolute; left:10px; top:10px; font-family:var(--f-mono); font-size:9px; font-weight:700;
    letter-spacing:.06em; padding:2px 5px; border-radius:var(--v-r-sm); border:1px solid currentColor; }
  .sp-slidetext{ font-size:var(--v-fs-b2); line-height:1.45; color:var(--v-dim); white-space:pre-line;
    display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-slideidx{ position:absolute; right:10px; bottom:8px; font-size:var(--v-fs-cap); color:var(--v-500); }

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
    border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt);
    cursor:pointer; transition:.12s; }
  .sp-arropt:hover{ border-color:var(--v-accent); background:var(--v-accent-soft); }
  /* Rose, never amber: this is a thing that is wrong, not a thing that is live
     (DECISIONS §22). */
  .sp-arropt.stale{ border-color:var(--v-rose,#e0526a); opacity:.75; cursor:not-allowed; }
  .sp-arropt.stale:hover{ border-color:var(--v-rose,#e0526a); background:transparent; }
  .sp-arrstale{ font-size:11px; color:var(--v-rose,#e0526a); }
  .sp-chip.stale{ color:var(--v-rose,#e0526a); border-color:currentColor; }
  .sp-arroptname{ font-weight:600; font-size:var(--v-fs-b1); }
  .sp-arroptseq{ font-size:var(--v-fs-cap); letter-spacing:.03em; color:var(--v-faint);
    white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-arrcancel{ align-self:flex-end; margin-top:4px; }
</style>
