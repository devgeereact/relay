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
  import { onMount } from 'svelte';
  import { trapFocus } from '../focus.js';
  import { songCue } from '../cues.js';
  import { setSession } from '../session.js';
  import { TYPE, payloadOf, slidesOf, slideAccent, cueSub } from '../plan.js';
  import {
    capture,
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
    searchScripture,
    searchSongs,
    getSong,
    listArrangements,
    listMedia,
    listAnnouncements,
  } from '../stores/capture.js';

  // ── plans list ──
  let plans = [];
  let showNew = false;
  let newTitle = '';

  // ── editor ──
  let openPlan = null;
  let items = [];
  let selId = null; // cue loaded in the centre slide flow
  let msg = '';
  let leftMode = 'cues'; // 'cues' | 'add'

  // one search (add mode) — scripture + songs + media together
  let addQ = '';
  let addVerses = [];
  let addSongs = [];
  let addMedia = [];
  let allMedia = []; // full media library, filtered locally by the query
  let addAnnounce = [];
  let allAnnounce = []; // full announcement list, filtered locally
  let addSearching = false;

  onMount(refresh);

  async function refresh() {
    plans = await listPlans();
  }

  async function addPlan() {
    const title = newTitle.trim();
    if (!title) return;
    const date = new Date().toISOString().slice(0, 10);
    try {
      await createPlan(title, date);
      newTitle = '';
      showNew = false;
      await refresh();
    } catch (e) {
      msg = String(e);
    }
  }
  async function removePlan(p, ev) {
    ev.stopPropagation();
    await deletePlan(p.id);
    await refresh();
  }
  async function clonePlan(p, ev) {
    ev.stopPropagation();
    await duplicatePlan(p.id, `${p.title} (copy)`);
    await refresh();
  }

  async function open(p) {
    openPlan = p;
    selId = null;
    leftMode = 'cues';
    msg = '';
    allMedia = await listMedia().catch(() => []);
    allAnnounce = await listAnnouncements().catch(() => []);
    await loadItems();
    if (items.length) selId = items[0].id;
  }
  function back() {
    openPlan = null;
    items = [];
    refresh();
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
    addSearching = true;
    const [v, s] = await Promise.all([searchScripture(q), searchSongs(q)]);
    addVerses = v;
    addSongs = s;
    const ql = q.toLowerCase();
    addMedia = allMedia.filter((m) => m.filename.toLowerCase().includes(ql));
    addAnnounce = allAnnounce.filter(
      (a) => a.title.toLowerCase().includes(ql) || a.body.toLowerCase().includes(ql),
    );
    addSearching = false;
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
    await addPlanItem(openPlan.id, 'scripture', v.reference, payload);
    await loadItems();
  }
  async function addMediaCue(m) {
    const payload = { media_id: m.id, kind: m.kind, filename: m.filename };
    await addPlanItem(openPlan.id, 'media', m.filename, payload);
    await loadItems();
  }
  async function addAnnounceCue(a) {
    const payload = { announce_id: a.id, title: a.title, body: a.body };
    await addPlanItem(openPlan.id, 'announce', a.title || 'Announcement', payload);
    await loadItems();
  }
  let cdAddMin = 5;
  async function addCountdownCue() {
    const m = Number(cdAddMin) || 5;
    const payload = { minutes: m, label: 'Service begins in', done: 'Welcome' };
    await addPlanItem(openPlan.id, 'countdown', `Countdown · ${m} min`, payload);
    await loadItems();
  }
  // Song → plan. If the song has saved arrangements, open a picker so the
  // operator chooses one (or Standard); otherwise add the Standard order.
  let arrPick = null; // { song, arrangements } while choosing
  async function addSong(summary) {
    const song = await getSong(summary.id);
    if (!song) return;
    const arrangements = await listArrangements(song.id);
    if (arrangements.length === 0) {
      await commitSong(song, null);
      return;
    }
    arrPick = { song, arrangements };
  }
  async function commitSong(song, arr) {
    const { label, payload } = songCue(song, arr);
    await addPlanItem(openPlan.id, 'song', label, payload);
    arrPick = null;
    await loadItems();
  }

  async function remove(id, ev) {
    ev.stopPropagation();
    await removePlanItem(id);
    if (selId === id) selId = items[0]?.id ?? null;
    await loadItems();
  }
  async function move(id, dir, ev) {
    ev.stopPropagation();
    await movePlanItem(id, dir);
    await loadItems();
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
    reorderPlan(openPlan.id, arr.map((i) => i.id));
  }

  /** Hand this plan to the LIVE tab and go there. The one path from build to run. */
  function runPlan() {
    setSession({ planId: openPlan.id, liveCueId: null, liveSlide: 0, activeTab: 'live' });
  }

  $: selCue = items.find((i) => i.id === selId) || null;

  // Per-cue operator stage note. Seed the draft only when the selected cue
  // changes (id guard) so a reload/typing doesn't clobber in-progress edits.
  let noteDraft = '';
  let noteFor = null;
  $: if (selCue && selCue.id !== noteFor) {
    noteDraft = payloadOf(selCue).stage_note || '';
    noteFor = selCue.id;
  }
  async function saveNote() {
    if (!selCue) return;
    const id = selCue.id;
    await setPlanNote(id, noteDraft);
    await loadItems();
  }
  $: selSlides = slidesOf(selCue);
  $: flowHeader = !selCue
    ? ''
    : selCue.cue_type === 'song'
      ? `${payloadOf(selCue).title || selCue.label} · SLIDE FLOW`
      : `${selCue.label} · ${(TYPE[selCue.cue_type] || {}).label || ''}`;
</script>

<!-- Escape closes the arrangement picker, from anywhere — bound at the window rather
     than on the backdrop, which never holds focus. Without this, Escape inside the
     picker fell through to the global panic key: it cleared the congregation's screens
     and left the picker open. (shortcuts.js now also refuses to clear while any
     [role="dialog"] is mounted, so the two halves cannot disagree.) -->
<svelte:window on:keydown={(e) => arrPick && e.key === 'Escape' && (arrPick = null)} />

{#if !openPlan}
  <!-- ══ PLANS LIST ══ -->
  <div class="sp-view">
    <div class="sp-actionbar">
      <p class="r-lead">Build a service plan from Library content — scripture, songs, media, notices — into one ordered flow, then run it live. Firing a slide shows on every output screen through the shared template engine.</p>
      {#if showNew}
        <form class="sp-newform" on:submit|preventDefault={addPlan}>
          <input class="r-input" placeholder="Plan title…" bind:value={newTitle} autofocus />
          <button class="r-btn amber sm" type="submit">Create</button>
          <button class="r-btn ghost sm" type="button" on:click={() => (showNew = false)}>Cancel</button>
        </form>
      {:else}
        <button class="r-btn amber" on:click={() => (showNew = true)}>＋ New Plan</button>
      {/if}
    </div>

    {#if !$capture.available}
      <div><span class="r-badge rose"><span class="bd"></span>Backend not attached — plans need the desktop app</span></div>
    {/if}

    {#if plans.length}
      <div class="plan-grid">
        {#each plans as p}
          <div class="plan-card">
            <button class="plan-hit r-focus" on:click={() => open(p)} title="Open plan">
              <div class="plan-head">
                <span class="plan-badge">PLAN</span>
                <span class="plan-cues r-mono">{p.cue_count}<i>cue{p.cue_count === 1 ? '' : 's'}</i></span>
              </div>
              <div class="plan-title">{p.title}</div>
              <div class="plan-meta r-mono">{p.plan_date || 'No date'}</div>
            </button>
            <div class="plan-foot">
              <span class="plan-open r-mono">Open plan ›</span>
              <span class="plan-foot-btns">
                <button class="r-iconbtn" title="Duplicate plan" on:click={(e) => clonePlan(p, e)}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="12" height="12" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                </button>
                <button class="r-iconbtn plan-del" title="Delete plan" on:click={(e) => removePlan(p, e)}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/></svg>
                </button>
              </span>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="cat-empty"><span class="r-empty">No plans yet — create one to start building a service.</span></div>
    {/if}
  </div>
{:else}
  <!-- ══ RUN EDITOR (Mission-Control layout) ══ -->
  <div class="sp-run">
    <!-- build bar -->
    <div class="sp-bldbar">
      <button class="r-btn ghost sm" on:click={back}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
        Plans
      </button>
      <span class="sp-plantitle">{openPlan.title}</span>
      <span class="r-mono sp-cuecount">{items.length} cues</span>
      {#if msg}<span class="sp-msg r-mono">{msg}</span>{/if}
      <span class="sp-spring"></span>
      <!-- The only path from build to run. Nothing on this screen goes to the
           congregation's wall — that is deliberate. An operator arranging next
           Sunday's songs on a Tuesday must not be able to fire one onto a screen
           by clicking the wrong thing. -->
      <button class="r-btn amber sm" on:click={runPlan} disabled={!items.length}>
        Run this plan
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z"/></svg>
      </button>
    </div>

    <div class="sp-grid2">
      <!-- LEFT: service plan / add -->
      <div class="sp-col sp-plan">
        <div class="sp-colhead">
          <div class="sp-seg">
            <button class="sp-segbtn" class:on={leftMode === 'cues'} on:click={() => (leftMode = 'cues')}>Service Plan</button>
            <button class="sp-segbtn" class:on={leftMode === 'add'} on:click={() => { leftMode = 'add'; if (!addQ.trim()) { addMedia = allMedia.slice(0, 8); addAnnounce = allAnnounce.slice(0, 8); } }}>＋ Add</button>
          </div>
          {#if leftMode === 'cues'}<span class="r-mono sp-colcount">{items.length}</span>{/if}
        </div>

        {#if leftMode === 'cues'}
          <div class="sp-cuelist">
            {#if items.length}
              {#each items as c, i (c.id)}
                {@const ty = TYPE[c.cue_type] || TYPE.scripture}
                <div class="sp-cue" class:sel={c.id === selId} class:dragover={dragOverId === c.id}
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
                  <span class="sp-bar" style="background:{ty.color};"></span>
                  <span class="sp-num r-mono">{String(i + 1).padStart(2, '0')}</span>
                  <span class="sp-cuebody">
                    <span class="sp-cuetitle">{c.label}</span>
                    <span class="sp-cuemeta r-mono">{cueSub(c)}</span>
                    {#if payloadOf(c).stage_note}
                      <span class="sp-cuenote" title={payloadOf(c).stage_note}>
                        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
                        {payloadOf(c).stage_note}
                      </span>
                    {/if}
                  </span>
                  <span class="sp-cuebtns">
                    <button class="sp-mini" title="Up" disabled={i === 0} on:click={(e) => move(c.id, -1, e)}>↑</button>
                    <button class="sp-mini" title="Down" disabled={i === items.length - 1} on:click={(e) => move(c.id, 1, e)}>↓</button>
                    <button class="sp-mini danger" title="Remove" on:click={(e) => remove(c.id, e)}>✕</button>
                  </span>
                </div>
              {/each}
            {:else}
              <div class="sp-drop r-mono">Empty plan — use ＋ Add.</div>
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
              <button class="sp-arropt r-focus" on:click={() => commitSong(arrPick.song, a)}>
                <span class="sp-arroptname">{a.name}</span>
                <span class="sp-arroptseq r-mono">{a.sequence.map((i) => (arrPick.song.sections[i]?.tag ?? '?')).join(' · ')}</span>
              </button>
            {/each}
            <button class="r-btn ghost sm sp-arrcancel" on:click={() => (arrPick = null)}>Cancel</button>
          </div>
        </div>
      {/if}

      <!-- CENTRE: slide flow of the selected cue -->
      <div class="sp-col sp-flowcol">
        <div class="sp-colhead sp-flowhead">
          <span class="r-mono sp-flowtitle">{flowHeader || 'SELECT A CUE'}</span>
          {#if selCue}
            <span class="sp-flowmeta">
              {#if selCue.cue_type === 'song'}<span class="sp-chip r-mono">ARRANGEMENT: {(payloadOf(selCue).arrangement_name || 'Standard').toUpperCase()}</span>{/if}
              <span class="sp-chip r-mono">{selSlides.length} {selCue.cue_type === 'song' ? 'SECTIONS' : 'SLIDE' + (selSlides.length === 1 ? '' : 'S')}</span>
            </span>
          {/if}
        </div>
        {#if selCue}
          <div class="sp-noterow">
            <svg class="sp-noteico" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
            <input class="r-input sp-noteinput" bind:value={noteDraft}
              placeholder="Stage note — shows on the confidence monitor only, never on the congregation screen"
              on:blur={saveNote} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />
          </div>
        {/if}
        <div class="sp-slidewrap r-scroll">
          {#if selCue}
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
            <div class="sp-empty r-empty">Pick a cue on the left to see its slides.</div>
          {/if}
        </div>
      </div>

    </div>
  </div>
{/if}

<style>
  .sp-view{ display:flex; flex-direction:column; gap:18px; max-width:1120px; }

  /* plans list */
  .sp-actionbar{ display:flex; align-items:flex-end; justify-content:space-between; gap:24px; flex-wrap:wrap; }
  .sp-actionbar .r-lead{ margin:0; }
  .sp-newform{ display:flex; gap:8px; align-items:center; flex-shrink:0; }
  .sp-newform .r-input{ width:200px; }
  /* Plan cards — image-free, dense, matches the Library song card. */
  .plan-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(232px, 1fr)); gap:12px; }
  .plan-card{ position:relative; display:flex; flex-direction:column; background:var(--v-surf); border:1px solid var(--v-line);
    border-radius:13px; padding:14px 15px 11px; overflow:hidden; transition:border-color .14s, background .14s; }
  .plan-card:hover{ border-color:var(--v-line2); background:var(--v-surf2); }
  .plan-card::before{ content:""; position:absolute; left:0; top:14px; bottom:14px; width:3px; border-radius:0 3px 3px 0; background:var(--v-amber); }
  .plan-hit{ display:block; width:100%; text-align:left; background:none; border:0; padding:0 0 0 7px; cursor:pointer; color:inherit; }
  .plan-head{ display:flex; align-items:center; justify-content:space-between; margin-bottom:11px; }
  .plan-badge{ font-family:var(--f-mono); font-size:8.5px; font-weight:700; letter-spacing:.14em; color:var(--v-amber);
    background:var(--v-amber-soft); border:1px solid rgba(245,166,35,.3); padding:3px 8px; border-radius:6px; }
  .plan-cues{ font-size:15px; font-weight:600; color:var(--v-txt); display:inline-flex; align-items:baseline; gap:5px; }
  .plan-cues i{ font-style:normal; font-size:8.5px; font-weight:500; letter-spacing:.1em; text-transform:uppercase; color:var(--v-faint); }
  .plan-title{ font-family:var(--f-head); font-size:15px; font-weight:600; line-height:1.28; color:var(--v-txt);
    display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; min-height:38px; }
  .plan-meta{ font-size:9.5px; color:var(--v-faint); margin-top:6px; letter-spacing:.02em; }
  .plan-foot{ display:flex; align-items:center; justify-content:space-between; margin:11px 0 0 7px; padding-top:10px; border-top:1px solid var(--v-line); }
  .plan-foot-btns{ display:flex; align-items:center; gap:6px; }
  .plan-open{ font-size:8.5px; letter-spacing:.06em; color:var(--v-faint); transition:color .14s; }
  .plan-card:hover .plan-open{ color:var(--v-amber); }
  .plan-del{ width:30px; height:30px; }
  .plan-del:hover{ color:var(--v-rose); border-color:rgba(244,113,139,.4); }

  /* run editor — fill the scroll area; each column scrolls internally */
  .sp-run{ display:flex; flex-direction:column; gap:12px; height:100%; min-height:0; }
  .sp-bldbar{ display:flex; align-items:center; gap:12px; flex:0 0 auto; }
  .sp-plantitle{ font-family:var(--f-head); font-size:20px; font-weight:700; color:var(--v-txt); }
  .sp-cuecount{ font-size:10px; letter-spacing:.06em; color:var(--v-dim); padding:5px 10px; border-radius:99px;
    background:var(--v-surf2); border:1px solid var(--v-line2); }
  .sp-spring{ flex:1; }
  .sp-msg{ font-size:11px; color:var(--v-emerald); max-width:240px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  .sp-grid2{ flex:1; min-height:0; display:grid; grid-template-columns:300px minmax(0,1fr); gap:12px; }
  @media (max-width:980px){ .sp-run{ height:auto; } .sp-grid2{ grid-template-columns:1fr; } }

  .sp-col{ background:var(--v-surf); border:1px solid var(--v-line); border-radius:13px; display:flex; flex-direction:column;
    min-height:0; overflow:hidden; }
  .sp-colhead{ display:flex; align-items:center; justify-content:space-between; gap:10px; padding:11px 13px;
    border-bottom:1px solid var(--v-line); flex:0 0 auto; }
  .sp-colcount{ font-size:9.5px; color:var(--v-faint); letter-spacing:.06em; }

  /* segmented toggle */
  .sp-seg{ display:flex; gap:3px; background:var(--v-bg); border:1px solid var(--v-line2); border-radius:9px; padding:3px; }
  .sp-segbtn{ font-family:var(--f-mono); font-size:9.5px; font-weight:600; letter-spacing:.06em; text-transform:uppercase;
    padding:5px 9px; border-radius:6px; border:0; background:transparent; color:var(--v-dim); cursor:pointer; }
  .sp-segbtn.on{ background:var(--v-surf3); color:var(--v-amber); }

  /* cue list */
  .sp-cuelist{ flex:1; overflow-y:auto; padding:11px; display:flex; flex-direction:column; gap:7px;
    scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent; }
  .sp-cue{ display:flex; align-items:center; gap:10px; padding:10px 11px; border:1px solid var(--v-line); border-radius:10px;
    background:var(--v-surf2); cursor:pointer; text-align:left; transition:border-color .12s, background .12s; }
  .sp-cue:hover{ border-color:var(--v-line2); }
  .sp-cue.sel{ border-color:rgba(245,166,35,.4); }
  .sp-cue :global(.sp-num){ cursor:grab; }
  .sp-bar{ width:3px; align-self:stretch; min-height:24px; border-radius:3px; flex:0 0 auto; }
  .sp-num{ font-size:9.5px; color:var(--v-faint); width:16px; flex:0 0 auto; }
  .sp-cuebody{ flex:1; min-width:0; }
  .sp-cuetitle{ display:block; font-weight:600; font-size:12.5px; color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-cuemeta{ display:block; font-size:8px; color:var(--v-faint); margin-top:2px; letter-spacing:.03em; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-mini{ width:22px; height:22px; border-radius:6px; display:grid; place-items:center; cursor:pointer; font-size:11px;
    background:var(--v-surf3); border:1px solid var(--v-line); color:var(--v-dim); }
  .sp-mini:hover:not(:disabled){ color:var(--v-amber); border-color:var(--v-line2); }
  .sp-mini.danger:hover:not(:disabled){ color:var(--v-rose); border-color:rgba(244,113,139,.4); }
  .sp-mini:disabled{ opacity:.3; cursor:not-allowed; }
  .sp-drop{ padding:16px; text-align:center; font-size:11px; color:var(--v-faint); }

  /* add panel */
  .sp-addpanel{ flex:1; overflow-y:auto; padding:12px; }
  .sp-addsearch{ display:flex; align-items:center; gap:9px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:9px; padding:0 11px; height:38px; }
  .sp-addsearch:focus-within{ border-color:rgba(245,166,35,.45); box-shadow:0 0 0 3px rgba(245,166,35,.08); }
  .sp-searchic{ color:var(--v-faint); flex:0 0 auto; }
  .sp-addsearch input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-family:var(--f-body); font-size:12.5px; }
  .sp-addsearch input::placeholder{ color:var(--v-faint); }
  .sp-reslbl{ margin:10px 0 2px; }
  .sp-hint{ font-size:11px; color:var(--v-faint); padding:4px 0; }

  /* countdown quick-add */
  .sp-cdadd{ display:flex; align-items:center; gap:8px; margin-top:8px; padding:8px 10px;
    border:1px solid var(--v-line); border-radius:9px; background:var(--v-surf2); }
  .sp-cdlbl{ font-size:12px; color:var(--v-txt); }
  .sp-cdmin{ width:52px; padding:4px 6px; border-radius:6px; border:1px solid var(--v-line2);
    background:var(--v-surf); color:var(--v-txt); font-family:var(--f-mono); font-size:12px; text-align:center; }
  .sp-cdunit{ font-size:10px; color:var(--v-faint); margin-left:-3px; }
  .sp-cdgo{ margin-left:auto; }

  /* per-cue stage note editor */
  .sp-noterow{ display:flex; align-items:center; gap:8px; padding:8px 10px; margin:0 0 8px;
    border:1px solid var(--v-line); border-radius:10px; background:var(--v-surf2); }
  .sp-noteico{ color:var(--v-amber); flex:0 0 auto; }
  .sp-noteinput{ flex:1; min-width:0; border:none; background:transparent; padding:2px 0; font-size:12px; }
  .sp-noteinput:focus{ outline:none; }
  .sp-cuenote{ display:inline-flex; align-items:center; gap:4px; margin-top:3px; max-width:100%;
    font-size:10px; color:var(--v-amber); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-cuenote svg{ flex:0 0 auto; }

  /* arrangement picker */
  .sp-arrback{ position:fixed; inset:0; background:rgba(6,6,8,.6); backdrop-filter:blur(3px); z-index:200;
    display:flex; align-items:center; justify-content:center; padding:24px; }
  .sp-arrsheet{ width:min(400px, 92%); max-height:80%; overflow:auto; background:var(--v-surf); border:1px solid var(--v-line2);
    border-radius:16px; padding:18px; display:flex; flex-direction:column; gap:8px;
    box-shadow:0 24px 70px -20px rgba(0,0,0,.7); }
  .sp-arrtitle{ font-family:var(--f-head); font-weight:700; font-size:16px; color:var(--v-txt); }
  .sp-arrsub{ margin:2px 0 6px; }
  .sp-arropt{ display:flex; flex-direction:column; gap:3px; width:100%; text-align:left; padding:10px 12px;
    border-radius:10px; background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt); cursor:pointer;
    transition:.12s; }
  .sp-arropt:hover{ border-color:var(--v-amber); background:var(--v-amber-soft); }
  .sp-arroptname{ font-weight:600; font-size:13px; }
  .sp-arroptseq{ font-size:9.5px; letter-spacing:.03em; color:var(--v-faint);
    white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .sp-arrcancel{ align-self:flex-end; margin-top:4px; }

  .sp-results{ display:flex; flex-direction:column; gap:6px; margin-top:8px; }
  .sp-result{ display:flex; align-items:flex-start; gap:9px; width:100%; padding:9px 10px; border-radius:9px;
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt); cursor:pointer; text-align:left; }
  .sp-result:hover{ border-color:var(--v-line2); }
  .sp-dot{ width:7px; height:7px; border-radius:2px; flex:0 0 auto; margin-top:5px; }
  .sp-resbody{ flex:1; min-width:0; }
  .sp-resref{ display:block; font-family:var(--f-head); font-weight:700; font-size:12.5px; color:var(--v-txt); }
  .sp-restext{ display:block; font-size:10.5px; color:var(--v-dim); line-height:1.4; margin-top:2px;
    display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-plus{ color:var(--v-amber); font-family:var(--f-mono); font-weight:700; flex:0 0 auto; }

  /* centre slide flow */
  .sp-flowhead{ gap:12px; }
  .sp-flowtitle{ font-size:11px; letter-spacing:.13em; text-transform:uppercase; color:var(--v-dim);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-flowmeta{ display:flex; gap:8px; flex:0 0 auto; }
  .sp-chip{ font-size:9px; letter-spacing:.06em; color:var(--v-dim); padding:4px 9px; border-radius:7px;
    background:var(--v-surf2); border:1px solid var(--v-line2); white-space:nowrap; }
  .sp-slidewrap{ flex:1; overflow-y:auto; padding:16px; scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent; }
  .sp-slides{ display:grid; grid-template-columns:repeat(3, 1fr); gap:14px; }
  @media (max-width:1360px){ .sp-slides{ grid-template-columns:repeat(2, 1fr); } }
  @media (max-width:980px){ .sp-slides{ grid-template-columns:repeat(3, 1fr); } }
  @media (max-width:640px){ .sp-slides{ grid-template-columns:repeat(2, 1fr); } }
  .sp-slide{ position:relative; aspect-ratio:16/9; border-radius:12px; border:1px solid var(--v-line); background:var(--v-surf2);
    padding:16px; display:flex; align-items:center; justify-content:center; text-align:center; overflow:hidden; }
  .sp-slidetext{ font-family:var(--f-serif); font-size:14px; line-height:1.4; color:#f4e4c8; white-space:pre-line;
    display:-webkit-box; -webkit-line-clamp:4; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-slideidx{ position:absolute; left:12px; bottom:9px; font-size:10px; color:var(--v-faint); }

</style>
