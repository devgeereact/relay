<script>
  // Service Planner — build a plan from Library content, then RUN it. The editor
  // is a Mission-Control surface: the plan's cues (left), the selected cue's full
  // slide flow (centre), and the live OUTPUT on every styled screen (right).
  // Each cue is { cue_type, label, payload }; firing a slide broadcasts through
  // the one shared pipeline, so the output monitors here are the real thing —
  // the same TemplateRender + active templates the Console and OBS clients use.
  import { onMount, onDestroy } from 'svelte';
  import TemplateRender from '../TemplateRender.svelte';
  import { registerContext } from '../shortcuts.js';
  import { monitorAccent } from '../templates.js';
  import { songCue } from '../cues.js';
  import { setSession } from '../session.js';
  import {
    capture,
    live,
    listActiveTemplates,
    liveContent,
    liveTemplateOverride,
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
    fireMedia,
    listAnnouncements,
    startCountdown,
    manualFire,
    fireContent,
    clearScreens,
    setStageNext,
  } from '../stores/capture.js';

  const TYPE = {
    scripture: { label: 'SCRIPTURE', color: 'var(--v-cyan)', trig: 'AUTO-DETECT' },
    song: { label: 'SONG', color: 'var(--v-amber)', trig: 'SUGGEST-ONLY' },
    media: { label: 'MEDIA', color: 'var(--v-amethyst)', trig: 'MANUAL/LOOP' },
    announce: { label: 'NOTICE', color: 'var(--v-rose)', trig: 'MANUAL/TIMER' },
    countdown: { label: 'COUNTDOWN', color: 'var(--v-cyan)', trig: 'TIMER' },
  };

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
  let activeTpls = [];

  // live state (what's on the screens)
  let liveCueId = null;
  let liveSlide = 0;

  // Mirror the live position into the persisted session, so a reload — or a
  // crash followed by Recover — brings the operator back to the cue and slide
  // they were actually on, mid-service, instead of to a blank Console tab.
  $: setSession({ planId: openPlan?.id ?? null, liveCueId, liveSlide });

  // one search (add mode) — scripture + songs + media together
  let addQ = '';
  let addVerses = [];
  let addSongs = [];
  let addMedia = [];
  let allMedia = []; // full media library, filtered locally by the query
  let addAnnounce = [];
  let allAnnounce = []; // full announcement list, filtered locally
  let addSearching = false;

  // Slide transport. Registered as CONTEXT actions with the app-shell shortcut
  // registry (lib/shortcuts.js) rather than a private window listener — Escape
  // (clear) and B (blackout) are global and owned by the shell, so they keep
  // working on every tab. Only advance/back are ours, and only while a plan is
  // actually open in the run editor.
  let unregisterKeys;
  onMount(() => {
    refresh();
    unregisterKeys = registerContext({
      next: () => openPlan && stepLive(1),
      prev: () => openPlan && stepLive(-1),
    });
  });
  onDestroy(() => unregisterKeys?.());
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
    liveCueId = null;
    liveSlide = 0;
    leftMode = 'cues';
    msg = '';
    activeTpls = await safeActive();
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
  async function safeActive() {
    try {
      return await listActiveTemplates();
    } catch {
      return [];
    }
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

  function payloadOf(item) {
    try {
      return JSON.parse(item.payload_json || '{}');
    } catch {
      return {};
    }
  }

  // The slides of a cue, normalized for the centre grid. Every content type
  // reduces to { tag, label, text } — no per-type branch downstream.
  function slidesOf(item) {
    if (!item) return [];
    const p = payloadOf(item);
    if (item.cue_type === 'song') {
      return (p.sections || []).map((s) => ({ tag: s.tag, label: s.label, text: s.lyrics }));
    }
    if (item.cue_type === 'scripture') {
      return [{ tag: p.verse != null ? String(p.verse) : 'SCR', label: p.reference || item.label, text: p.text || '' }];
    }
    if (item.cue_type === 'announce') {
      return [{ tag: 'NOTE', label: item.label, text: p.body || p.text || '' }];
    }
    if (item.cue_type === 'media') {
      return [{ tag: 'BG', label: item.label, text: '' }];
    }
    if (item.cue_type === 'countdown') {
      const m = Number(p.minutes) || 5;
      return [{ tag: '⏱', label: p.label || item.label, text: `${m}:00` }];
    }
    return [];
  }

  // Group colour — matches the Song Editor (Verse→cyan, Chorus→gold, Bridge→
  // amethyst, Pre-Chorus/Intro→emerald, Outro/Tag→rose, else neutral).
  function slideAccent(tag) {
    const t = (tag || '').toUpperCase();
    if (/^\d+$/.test(t)) return 'var(--v-faint)';
    if (t.startsWith('V')) return 'var(--v-cyan)';
    if (t.startsWith('PC')) return 'var(--v-emerald)';
    if (t.startsWith('C')) return 'var(--v-amber)';
    if (t.startsWith('BR') || /^B\d?$/.test(t)) return 'var(--v-amethyst)';
    if (t.startsWith('INT') || t.startsWith('IL')) return 'var(--v-emerald)';
    if (t.startsWith('OUT') || t.startsWith('END') || t.startsWith('TAG') || t.startsWith('REF')) return 'var(--v-rose)';
    if (t === 'NOTE') return 'var(--v-rose)';
    if (t === 'BG') return 'var(--v-amethyst)';
    return 'var(--v-cyan)';
  }

  // Fire slide `i` of `item` to every screen. This is the take.
  async function fireSlide(item, i) {
    const p = payloadOf(item);
    const slides = slidesOf(item);
    const s = slides[i] || slides[0];
    if (!s) return;
    liveCueId = item.id;
    liveSlide = i;
    selId = item.id;
    const stageNote = p.stage_note || null; // operator's confidence-monitor note
    try {
      if (item.cue_type === 'scripture') {
        await manualFire(p.reference || item.label, stageNote);
      } else if (item.cue_type === 'media') {
        if (!p.media_id) {
          msg = 'Media asset missing — re-add it from the Library.';
          return;
        }
        await fireMedia(p.media_id);
      } else if (item.cue_type === 'countdown') {
        await startCountdown(Number(p.minutes) || 5, p.label || 'Service begins in', p.done || 'Welcome');
      } else if (item.cue_type === 'song') {
        // Lyrics carry NO title/section on the live screen — that stays in the
        // operator UI. Only the lyric lines go out (centered by the template).
        await fireContent('', s.text, 'song', stageNote);
      } else {
        await fireContent(item.label, s.text, 'announce', stageNote);
      }
      msg = `Live: ${s.label}`;
      pushNext(item.id, i);
    } catch (e) {
      msg = String(e);
    }
  }

  // Compute what comes after (cueId, slideIdx) and push it to the stage monitor:
  // the next slide in the same cue, else the first slide of the next cue.
  function nextOf(cueId, slideIdx) {
    const idx = items.findIndex((it) => it.id === cueId);
    if (idx < 0) return null;
    const here = items[idx];
    const slides = slidesOf(here);
    if (slideIdx + 1 < slides.length) return labelled(here, slides[slideIdx + 1]);
    const nx = items[idx + 1];
    if (nx) {
      const ns = slidesOf(nx)[0];
      return ns ? labelled(nx, ns) : { label: nx.label, text: '' };
    }
    return null;
  }
  function labelled(item, slide) {
    const p = payloadOf(item);
    const label = item.cue_type === 'song' ? `${p.title} · ${slide.label}` : slide.label || item.label;
    return { label, text: slide.text || slide.label };
  }
  function pushNext(cueId, slideIdx) {
    const n = nextOf(cueId, slideIdx);
    setStageNext(n?.label ?? null, n?.text ?? null);
  }

  async function stepLive(dir) {
    const idx = items.findIndex((i) => i.id === liveCueId);
    if (idx < 0) {
      if (items[0]) await fireSlide(items[0], 0);
      return;
    }
    const item = items[idx];
    const n = slidesOf(item).length;
    const ns = liveSlide + dir;
    if (ns >= 0 && ns < n) {
      await fireSlide(item, ns);
      return;
    }
    const ni = idx + dir;
    if (ni >= 0 && ni < items.length) {
      const next = items[ni];
      const last = dir > 0 ? 0 : Math.max(0, slidesOf(next).length - 1);
      await fireSlide(next, last);
    }
  }
  async function clearLive() {
    try {
      await clearScreens();
    } catch {
      /* backend absent */
    }
    liveCueId = null;
    liveSlide = 0;
    setStageNext(null, null);
    msg = 'Cleared';
  }

  function cueSub(c) {
    const ty = TYPE[c.cue_type] || TYPE.scripture;
    return c.cue_type === 'song' ? `SONG · ${slidesOf(c).length} SLIDES` : `${ty.label} · ${ty.trig}`;
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
    // If this cue is live, re-push so the stage monitor picks up the new note.
    if (liveCueId === id) await fireSlide(items.find((i) => i.id === id), liveSlide);
  }
  $: selSlides = slidesOf(selCue);
  $: liveIndex = items.findIndex((i) => i.id === liveCueId);
  $: flowHeader = !selCue
    ? ''
    : selCue.cue_type === 'song'
      ? `${payloadOf(selCue).title || selCue.label} · SLIDE FLOW`
      : `${selCue.label} · ${(TYPE[selCue.cue_type] || {}).label || ''}`;
</script>

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
    <!-- transport bar -->
    <div class="sp-bar">
      <button class="r-btn ghost sm" on:click={back}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
        Plans
      </button>
      <span class="sp-plantitle">{openPlan.title}</span>
      {#if msg}<span class="sp-msg r-mono">{msg}</span>{/if}
      <span class="sp-spring"></span>
      <span class="sp-livecount r-mono" class:on={liveIndex >= 0}>
        {#if liveIndex >= 0}<span class="sp-livedot"></span>LIVE {liveIndex + 1}/{items.length}{:else}STANDBY · {items.length}{/if}
      </span>
      <span class="sp-kbd r-mono" title="Arrow keys / Space advance · Esc clears">←/→ · Space</span>
      <div class="sp-transport">
        <button class="r-iconbtn" title="Previous slide (←)" on:click={() => stepLive(-1)} disabled={!items.length}>‹</button>
        <button class="r-btn amber sm" on:click={() => stepLive(1)} disabled={!items.length}>Next ›</button>
        <button class="r-iconbtn sp-del" title="Clear output (Esc)" on:click={clearLive}>◼</button>
      </div>
    </div>

    <div class="sp-grid3">
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
                <div class="sp-cue" class:sel={c.id === selId} class:islive={c.id === liveCueId} class:dragover={dragOverId === c.id}
                  draggable={true}
                  on:dragstart={(e) => onDragStart(c.id, e)}
                  on:dragover|preventDefault={() => (dragOverId = c.id)}
                  on:dragleave={() => { if (dragOverId === c.id) dragOverId = null; }}
                  on:drop|preventDefault={() => onDropCue(c.id)}
                  on:click={() => (selId = c.id)} role="button" tabindex="0"
                  on:keydown={(e) => (e.key === 'Enter' ? (selId = c.id) : null)}>
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
                  {#if c.id === liveCueId}
                    <span class="r-badge emerald sp-livebadge"><span class="bd"></span>LIVE</span>
                  {:else}
                    <span class="sp-cuebtns">
                      <button class="sp-mini" title="Up" disabled={i === 0} on:click={(e) => move(c.id, -1, e)}>↑</button>
                      <button class="sp-mini" title="Down" disabled={i === items.length - 1} on:click={(e) => move(c.id, 1, e)}>↓</button>
                      <button class="sp-mini danger" title="Remove" on:click={(e) => remove(c.id, e)}>✕</button>
                    </span>
                  {/if}
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
        <div class="sp-arrback" role="button" tabindex="0" on:click={() => (arrPick = null)}
          on:keydown={(e) => e.key === 'Escape' && (arrPick = null)}>
          <div class="sp-arrsheet" role="dialog" aria-label="Choose arrangement"
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
                <button class="sp-slide r-focus" class:islive={selCue.id === liveCueId && i === liveSlide} on:click={() => fireSlide(selCue, i)}>
                  <span class="sp-slidetag" style="color:{slideAccent(s.tag)};border-color:{slideAccent(s.tag)}">{s.tag}</span>
                  <span class="sp-slidetext">{s.text || s.label}</span>
                  <span class="sp-slideidx r-mono">{String(i + 1).padStart(2, '0')}</span>
                  {#if selCue.id === liveCueId && i === liveSlide}<span class="sp-slidelive r-mono">◉ LIVE</span>{/if}
                </button>
              {/each}
            </div>
          {:else}
            <div class="sp-empty r-empty">Pick a cue on the left to see its slides.</div>
          {/if}
        </div>
      </div>

      <!-- RIGHT: live output on every screen -->
      <div class="sp-col sp-outcol">
        <div class="sp-colhead"><span class="r-mono sp-outtitle">Output</span><span class="r-mono sp-colcount">{$live ? 'LIVE' : 'STANDBY'}</span></div>
        <div class="sp-monwrap r-scroll">
          {#if activeTpls.length}
            {#each activeTpls as tpl, i (tpl.id)}
              {@const acc = monitorAccent(i)}
              <div class="sp-mon a-{acc}" class:on={$live}>
                <div class="sp-monhead">
                  <span class="sp-monlbl">{$live ? 'LIVE' : 'IDLE'} · {tpl.name}</span>
                  {#if i === 0}
                    <span class="sp-monnav">
                      <button class="sp-navbtn" title="Previous" on:click={() => stepLive(-1)}>‹</button>
                      <button class="sp-navbtn" title="Next" on:click={() => stepLive(1)}>›</button>
                    </span>
                  {/if}
                </div>
                <div class="sp-moncanvas"><TemplateRender template={$liveTemplateOverride ?? tpl} content={$liveContent} /></div>
                <div class="sp-monfoot r-mono">{$live ? $live.reference + ($live.translation ? ' · ' + $live.translation : '') : '—'}</div>
              </div>
            {/each}
          {:else}
            <div class="sp-empty r-empty">No active output styles — activate up to 4 in the <b>Templates</b> tab.</div>
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
  .sp-bar{ display:flex; align-items:center; gap:12px; flex:0 0 auto; }
  .sp-plantitle{ font-family:var(--f-head); font-size:20px; font-weight:700; color:var(--v-txt); }
  .sp-spring{ flex:1; }
  .sp-msg{ font-size:11px; color:var(--v-emerald); max-width:240px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-livecount{ display:inline-flex; align-items:center; gap:7px; font-size:10px; letter-spacing:.08em; color:var(--v-dim);
    padding:6px 11px; border-radius:99px; background:var(--v-surf2); border:1px solid var(--v-line2); }
  .sp-livecount.on{ color:var(--v-emerald); border-color:rgba(16,185,129,.35); background:var(--v-emerald-soft); }
  .sp-livedot{ width:6px; height:6px; border-radius:50%; background:var(--v-emerald); box-shadow:0 0 7px var(--v-emerald);
    animation:sp-pulse 1.6s ease-in-out infinite; }
  @keyframes sp-pulse{ 0%,100%{opacity:1} 50%{opacity:.4} }
  .sp-transport{ display:flex; align-items:center; gap:6px; }
  .sp-kbd{ font-size:9px; letter-spacing:.06em; color:var(--v-faint); padding:5px 9px; border-radius:7px;
    background:var(--v-surf2); border:1px solid var(--v-line); }
  @media (max-width:1100px){ .sp-kbd{ display:none; } }

  .sp-grid3{ flex:1; min-height:0; display:grid; grid-template-columns:264px minmax(0,1fr) 340px; gap:12px; }
  @media (max-width:1200px){ .sp-grid3{ grid-template-columns:230px minmax(0,1fr) 280px; } }
  @media (max-width:980px){ .sp-run{ height:auto; } .sp-grid3{ grid-template-columns:1fr; } }

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
  .sp-cue.islive{ border-color:var(--v-emerald); background:var(--v-emerald-soft); }
  .sp-cue.dragover{ border-color:var(--v-amber); box-shadow:inset 0 2px 0 var(--v-amber); }
  .sp-cue :global(.sp-num){ cursor:grab; }
  .sp-bar{ width:3px; align-self:stretch; min-height:24px; border-radius:3px; flex:0 0 auto; }
  .sp-num{ font-size:9.5px; color:var(--v-faint); width:16px; flex:0 0 auto; }
  .sp-cuebody{ flex:1; min-width:0; }
  .sp-cuetitle{ display:block; font-weight:600; font-size:12.5px; color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-cuemeta{ display:block; font-size:8px; color:var(--v-faint); margin-top:2px; letter-spacing:.03em; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .sp-livebadge{ padding:3px 8px; flex:0 0 auto; }
  .sp-cuebtns{ display:flex; gap:3px; flex:0 0 auto; }
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
  .sp-collabel{ margin-bottom:8px; }
  .sp-soon{ margin-top:18px; }
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
    padding:16px; display:flex; align-items:center; justify-content:center; text-align:center; cursor:pointer; overflow:hidden;
    transition:border-color .12s, box-shadow .12s; }
  .sp-slide:hover{ border-color:var(--v-amber); }
  .sp-slide.islive{ border-color:var(--v-emerald); box-shadow:0 0 0 1px var(--v-emerald), 0 10px 28px -12px rgba(16,185,129,.5); }
  .sp-slidetag{ position:absolute; top:11px; left:11px; font-family:var(--f-mono); font-size:9.5px; font-weight:700;
    letter-spacing:.04em; padding:3px 8px; border-radius:6px; border:1px solid currentColor; }
  .sp-slidetext{ font-family:var(--f-serif); font-size:14px; line-height:1.4; color:#f4e4c8; white-space:pre-line;
    display:-webkit-box; -webkit-line-clamp:4; -webkit-box-orient:vertical; overflow:hidden; }
  .sp-slideidx{ position:absolute; left:12px; bottom:9px; font-size:10px; color:var(--v-faint); }
  .sp-slidelive{ position:absolute; right:11px; bottom:9px; font-size:8.5px; color:var(--v-emerald); }
  .sp-empty{ padding:26px 18px; text-align:center; }

  /* right output monitors */
  .sp-outtitle{ font-size:11px; letter-spacing:.13em; text-transform:uppercase; color:var(--v-dim); }
  .sp-monwrap{ flex:1; overflow-y:auto; padding:12px; display:flex; flex-direction:column; gap:12px;
    scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent; }
  .sp-mon{ border:1px solid var(--v-line); border-radius:12px; overflow:hidden; background:#050506; --c:var(--v-amber); }
  .sp-mon.a-amber{ --c:var(--v-amber); } .sp-mon.a-cyan{ --c:var(--v-cyan); }
  .sp-mon.a-amethyst{ --c:var(--v-amethyst); } .sp-mon.a-rose{ --c:var(--v-rose); }
  .sp-mon.on{ border-color:transparent; box-shadow:0 0 0 1px var(--c), 0 10px 26px -14px var(--c); }
  .sp-monhead{ display:flex; align-items:center; justify-content:space-between; padding:7px 11px; }
  .sp-monlbl{ font-family:var(--f-mono); font-size:8.5px; font-weight:700; letter-spacing:.08em; text-transform:uppercase; color:var(--c); }
  .sp-monnav{ display:flex; gap:5px; }
  .sp-navbtn{ width:22px; height:22px; border-radius:6px; display:grid; place-items:center; cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-dim); font-size:12px; }
  .sp-navbtn:hover{ color:var(--c); border-color:var(--v-line2); }
  .sp-moncanvas{ position:relative; aspect-ratio:16/9; overflow:hidden; background:#0a0a0b; }
  .sp-monfoot{ padding:6px 11px; border-top:1px solid var(--v-line); font-size:8.5px; color:var(--v-faint);
    letter-spacing:.04em; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
</style>
