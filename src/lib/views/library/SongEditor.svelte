<script>
  // Song Editor — the ProPresenter-style slide-flow view. Opens when a song is
  // clicked in the Lyrics list. Shows every slide as a card in a grid, grouped
  // and colour-coded by section type (Verse / Chorus / Bridge…), with the flow
  // (order) that a plan cue follows live. Edit slide text, tag, and order here;
  // Save replaces the song's sections wholesale. This is the "TITLE · SLIDE FLOW"
  // surface — the same grid the Console program pane and Planner reference.
  import { onMount, tick, createEventDispatcher } from 'svelte';
  import Loading from '../../ui/Loading.svelte';
  import {
    getSong,
    saveSong,
    listArrangements,
    saveArrangement,
    deleteArrangement,
  } from '../../stores/capture.js';

  export let songId;
  const dispatch = createEventDispatcher();
  let lyricsEl; // the selected slide's lyric textarea (for cursor-aware split)

  let song = null; // { id, title, author, ccli, song_key, bpm, sections:[{tag,label,lyrics}] }
  let sel = 0;
  let loading = true;
  let dirty = false;
  let saveMsg = '';

  // Arrangements — named play-orders of the song's sections (ProPresenter-style).
  // `arrangements` are the saved ones; "Standard" (id null) is implicit = every
  // section once, in order. `draft` is the arrangement being built/edited.
  let arrangements = [];
  let draft = null; // { id, name, sequence:[sectionIndex] } | null

  onMount(load);
  async function load() {
    loading = true;
    const s = await getSong(songId);
    // Normalize sections to the editable shape.
    song = s
      ? { ...s, sections: s.sections.map((x) => ({ tag: x.tag, label: x.label, lyrics: x.lyrics })) }
      : null;
    sel = 0;
    draft = null;
    arrangements = song ? await listArrangements(song.id) : [];
    loading = false;
  }

  // --- Arrangement editing ---------------------------------------------------
  function newArrangement() {
    draft = { id: null, name: '', sequence: [] };
  }
  function editArrangement(a) {
    draft = { id: a.id, name: a.name, sequence: [...a.sequence] };
  }
  function appendToDraft(i) {
    if (!draft) return;
    draft.sequence = [...draft.sequence, i];
  }
  function removeFromDraft(pos) {
    if (!draft) return;
    draft.sequence = draft.sequence.filter((_, j) => j !== pos);
  }
  function moveInDraft(pos, dir) {
    if (!draft) return;
    const j = pos + dir;
    if (j < 0 || j >= draft.sequence.length) return;
    const seq = draft.sequence.slice();
    [seq[pos], seq[j]] = [seq[j], seq[pos]];
    draft.sequence = seq;
  }
  async function saveDraft() {
    if (!draft || !song) return;
    const name = (draft.name || '').trim();
    if (!name) {
      saveMsg = 'Arrangement needs a name';
      return;
    }
    try {
      await saveArrangement(song.id, draft.id, name, draft.sequence);
      arrangements = await listArrangements(song.id);
      draft = null;
      saveMsg = '';
    } catch (e) {
      saveMsg = String(e);
    }
  }
  async function removeArrangement(a) {
    await deleteArrangement(a.id);
    arrangements = await listArrangements(song.id);
    if (draft && draft.id === a.id) draft = null;
  }
  // A section index shown in a sequence chip — label + colour. Out-of-range
  // indices (a section was deleted since the arrangement was made) show as "?".
  function seqTag(i) {
    return song && song.sections[i] ? song.sections[i].tag || String(i + 1) : '?';
  }

  // Group colour — ProPresenter colour-codes groups; we map the tag to a design
  // accent (Verse→cyan, Chorus→gold, Bridge→amethyst, Pre-Chorus/Intro→emerald,
  // Outro/Tag/Ending→rose, else neutral). Order of checks matters (PC before C).
  function tagClass(tag) {
    const t = (tag || '').toUpperCase();
    if (t.startsWith('V')) return 'tg-cyan';
    if (t.startsWith('PC')) return 'tg-emerald';
    if (t.startsWith('C')) return 'tg-amber';
    if (t.startsWith('BR') || t === 'B' || /^B\d/.test(t)) return 'tg-amethyst';
    if (t.startsWith('INT') || t.startsWith('IL')) return 'tg-emerald';
    if (t.startsWith('OUT') || t.startsWith('END') || t.startsWith('TAG') || t.startsWith('REF')) return 'tg-rose';
    return 'tg-dim';
  }

  function mark() {
    dirty = true;
    saveMsg = '';
  }

  function selectSlide(i) {
    sel = i;
  }

  // Renumber "auto" slides so the sequence stays correct after insert/delete/
  // reorder. A slide is auto-numbered when its tag is digits (or blank) — those
  // become "N" / "Slide N" by position. Custom groups (V1, Chorus…) are left
  // untouched so relabelled slides keep their names.
  function renumber() {
    song.sections = song.sections.map((s, i) => {
      if (/^\d*$/.test((s.tag || '').trim())) {
        return { ...s, tag: String(i + 1), label: `Slide ${i + 1}` };
      }
      return s;
    });
  }

  function addSlide() {
    song.sections = [...song.sections, { tag: '', label: '', lyrics: '' }];
    renumber();
    sel = song.sections.length - 1;
    mark();
  }
  function duplicateSlide(i, ev) {
    ev?.stopPropagation();
    const copy = { ...song.sections[i] };
    song.sections = [...song.sections.slice(0, i + 1), copy, ...song.sections.slice(i + 1)];
    renumber();
    sel = i + 1;
    mark();
  }
  function removeSlide(i, ev) {
    ev?.stopPropagation();
    song.sections = song.sections.filter((_, j) => j !== i);
    renumber();
    if (sel >= song.sections.length) sel = Math.max(0, song.sections.length - 1);
    mark();
  }
  function move(i, dir, ev) {
    ev?.stopPropagation();
    const j = i + dir;
    if (j < 0 || j >= song.sections.length) return;
    const a = song.sections.slice();
    [a[i], a[j]] = [a[j], a[i]];
    song.sections = a;
    renumber();
    sel = j;
    mark();
  }

  // Drag-reorder slides in the grid. Drop moves the dragged slide before the
  // target; selection follows the moved slide.
  let dragIdx = null;
  let dragOverIdx = null;
  function onSlideDragStart(i, e) {
    dragIdx = i;
    if (e.dataTransfer) e.dataTransfer.effectAllowed = 'move';
  }
  function dropSlide(to) {
    const from = dragIdx;
    dragIdx = null;
    dragOverIdx = null;
    if (from == null || from === to) return;
    const a = song.sections.slice();
    const [moved] = a.splice(from, 1);
    a.splice(to, 0, moved);
    song.sections = a;
    renumber();
    sel = to;
    mark();
  }

  // Split the selected slide at `pos` in its lyrics: text before stays, text
  // after becomes a new slide right below (inheriting the same group). The new
  // slide is selected with the caret at its start, so you can keep splitting.
  async function splitAt(pos) {
    const cur = song.sections[sel];
    if (!cur) return;
    const text = cur.lyrics || '';
    const at = Math.max(0, Math.min(pos ?? text.length, text.length));
    const before = text.slice(0, at);
    const after = text.slice(at);
    const nu = { tag: cur.tag, label: cur.label, lyrics: after.replace(/^\n/, '') };
    const list = song.sections.slice();
    list[sel] = { ...cur, lyrics: before.replace(/\n$/, '') };
    list.splice(sel + 1, 0, nu);
    song.sections = list;
    renumber();
    sel = sel + 1;
    mark();
    await tick();
    if (lyricsEl) {
      lyricsEl.focus();
      lyricsEl.setSelectionRange(0, 0);
    }
  }

  // Shift+Enter in the lyric box splits at the cursor.
  function onLyricsKey(e) {
    if (e.key === 'Enter' && e.shiftKey) {
      e.preventDefault();
      splitAt(e.target.selectionStart);
    }
  }
  // Split button — uses the live caret position, or end of text if unfocused.
  function splitButton() {
    const pos = lyricsEl ? lyricsEl.selectionStart : (current?.lyrics?.length ?? 0);
    splitAt(pos);
  }

  async function save() {
    try {
      await saveSong(song);
      dirty = false;
      saveMsg = 'Saved';
      dispatch('saved');
    } catch (e) {
      saveMsg = String(e);
    }
  }
  function back() {
    dispatch('back');
  }

  $: current = song && song.sections[sel] ? song.sections[sel] : null;
</script>

{#if loading}
  <Loading what="song" />
{:else if !song}
  <div class="r-empty">Song not found.</div>
{:else}
  <div class="se">
    <!-- top bar -->
    <div class="se-top">
      <button class="r-btn ghost sm" on:click={back}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
        Lyrics
      </button>
      <div class="se-titlewrap">
        <input class="se-title" bind:value={song.title} on:input={mark} placeholder="Song title" />
      </div>
      <div class="se-chips">
        <span class="se-flowtag r-mono">Slide Flow</span>
        <span class="se-chip r-mono">{arrangements.length + 1} ARRANGEMENT{arrangements.length === 0 ? '' : 'S'}</span>
        <span class="se-chip r-mono">{song.sections.length} SECTION{song.sections.length === 1 ? '' : 'S'}</span>
      </div>
      <span class="se-spring"></span>
      {#if saveMsg}<span class="se-msg r-mono" class:err={saveMsg !== 'Saved'}>{saveMsg}</span>{/if}
      <button class="r-btn amber sm" on:click={save} disabled={!dirty}>Save</button>
    </div>

    <!-- meta row -->
    <div class="se-meta">
      <label><span class="r-lbl">Author</span><input class="r-input" bind:value={song.author} on:input={mark} placeholder="—" /></label>
      <label class="se-msm"><span class="r-lbl">Key</span><input class="r-input" bind:value={song.song_key} on:input={mark} placeholder="—" /></label>
      <label class="se-msm"><span class="r-lbl">BPM</span><input class="r-input" bind:value={song.bpm} on:input={mark} placeholder="—" inputmode="numeric" /></label>
      <label><span class="r-lbl">CCLI</span><input class="r-input" bind:value={song.ccli} on:input={mark} placeholder="—" /></label>
    </div>

    <!-- arrangements: named play-orders of the sections -->
    <div class="se-arr">
      <div class="se-arrhead">
        <span class="r-lbl se-arrlbl">Arrangements</span>
        <span class="se-arrpills">
          <span class="se-arrpill" class:on={!draft}>Standard</span>
          {#each arrangements as a (a.id)}
            <button class="se-arrpill act" class:on={draft && draft.id === a.id} on:click={() => editArrangement(a)}>
              {a.name}
              <span class="se-arrcount r-mono">{a.sequence.length}</span>
            </button>
          {/each}
        </span>
        <span class="se-spring"></span>
        {#if !draft}
          <button class="r-btn ghost sm" on:click={newArrangement}>＋ New arrangement</button>
        {/if}
      </div>

      {#if draft}
        <div class="se-arrbuild">
          <div class="se-arrrow1">
            <input class="r-input se-arrname" bind:value={draft.name} placeholder="Arrangement name (e.g. Sunday AM)" />
            <span class="se-spring"></span>
            {#if draft.id}
              <button class="r-iconbtn se-del" title="Delete arrangement"
                on:click={() => removeArrangement({ id: draft.id })}>✕</button>
            {/if}
            <button class="r-btn ghost sm" on:click={() => (draft = null)}>Cancel</button>
            <button class="r-btn amber sm" on:click={saveDraft}>Save arrangement</button>
          </div>

          <!-- the built sequence -->
          <div class="se-seq">
            {#if draft.sequence.length === 0}
              <span class="se-seqempty r-mono">Click sections below to build the play order — repeats allowed.</span>
            {/if}
            {#each draft.sequence as si, pos (pos)}
              <span class="se-seqchip {tagClass(seqTag(si))}">
                <button class="se-seqmv" title="Move left" disabled={pos === 0} on:click={() => moveInDraft(pos, -1)}>‹</button>
                <span class="se-seqtag">{seqTag(si)}</span>
                <button class="se-seqmv" title="Move right" disabled={pos === draft.sequence.length - 1} on:click={() => moveInDraft(pos, 1)}>›</button>
                <button class="se-seqx" title="Remove" on:click={() => removeFromDraft(pos)}>✕</button>
              </span>
            {/each}
          </div>

          <!-- palette of sections to append -->
          <div class="se-palette">
            {#each song.sections as s, i (i)}
              <button class="se-palchip {tagClass(s.tag)}" title={s.label || s.lyrics} on:click={() => appendToDraft(i)}>
                <span class="se-palnum r-mono">{String(i + 1).padStart(2, '0')}</span>{s.tag || '—'}
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>

    <div class="se-body">
      <!-- slide grid -->
      <div class="se-grid">
        {#each song.sections as s, i (i)}
          <button class="se-slide r-focus" class:on={i === sel} class:drop={dragOverIdx === i}
            draggable={true}
            on:dragstart={(e) => onSlideDragStart(i, e)}
            on:dragover|preventDefault={() => (dragOverIdx = i)}
            on:dragleave={() => dragOverIdx === i && (dragOverIdx = null)}
            on:drop|preventDefault={() => dropSlide(i)}
            on:dragend={() => { dragIdx = null; dragOverIdx = null; }}
            on:click={() => selectSlide(i)}>
            <span class="se-tag {tagClass(s.tag)}">{s.tag}</span>
            <span class="se-lyric">{s.lyrics || '—'}</span>
            <span class="se-idx r-mono">{String(i + 1).padStart(2, '0')}</span>
          </button>
        {/each}
        <button class="se-add r-focus" on:click={addSlide}>
          <span class="se-addplus">＋</span>
          <span class="r-mono">Add slide</span>
        </button>
      </div>

      <!-- editor panel -->
      <aside class="se-edit">
        {#if current}
          <div class="r-lbl se-editlbl">Slide {sel + 1} of {song.sections.length}</div>
          <div class="se-field2">
            <label><span class="r-lbl">Section</span><input class="r-input" bind:value={current.label} on:input={mark} placeholder="Verse 1" /></label>
            <label class="se-tagfield"><span class="r-lbl">Tag</span><input class="r-input mono" bind:value={current.tag} on:input={mark} placeholder="V1" /></label>
          </div>
          <label class="se-lyrfield">
            <span class="r-lbl">Lyrics</span>
            <textarea class="r-input se-lyrics" bind:this={lyricsEl} bind:value={current.lyrics}
              on:input={mark} on:keydown={onLyricsKey} placeholder="Slide text…"></textarea>
            <span class="se-lyrhint r-mono">Shift + Enter splits into a new slide at the cursor</span>
          </label>
          <button class="r-btn ghost sm se-splitbtn" on:click={splitButton} title="Split this slide at the cursor">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12h18M8 7l-5 5 5 5M16 7l5 5-5 5"/></svg>
            Split at cursor
          </button>
          <div class="se-slideacts">
            <button class="r-iconbtn" title="Move up" disabled={sel === 0} on:click={(e) => move(sel, -1, e)}>↑</button>
            <button class="r-iconbtn" title="Move down" disabled={sel === song.sections.length - 1} on:click={(e) => move(sel, 1, e)}>↓</button>
            <button class="r-iconbtn" title="Duplicate" on:click={(e) => duplicateSlide(sel, e)}>⧉</button>
            <span class="se-spring"></span>
            <button class="r-iconbtn se-del" title="Delete slide" on:click={(e) => removeSlide(sel, e)}>✕</button>
          </div>
        {:else}
          <div class="r-empty">No slides — add one.</div>
        {/if}
      </aside>
    </div>
  </div>
{/if}

<style>
  .se{ display:flex; flex-direction:column; gap:14px; }

  .se-top{ display:flex; align-items:center; gap:14px; flex-wrap:wrap; padding-bottom:2px; }
  .se-titlewrap{ flex:1 1 200px; min-width:0; }
  .se-title{ width:100%; font-family:var(--f-head); font-size:22px; font-weight:700; color:var(--v-txt);
    background:transparent; border:1px solid transparent; border-radius:8px; padding:3px 9px; outline:none;
    text-overflow:ellipsis; }
  .se-title:hover{ border-color:var(--v-line); }
  .se-title:focus{ border-color:var(--v-line2); background:var(--v-surf); }
  .se-chips{ display:flex; align-items:center; gap:8px; flex:0 0 auto; }
  .se-flowtag{ font-size:9.5px; font-weight:600; letter-spacing:.16em; text-transform:uppercase; color:var(--v-faint); }
  .se-chip{ font-size:9px; letter-spacing:.06em; color:var(--v-dim); padding:4px 9px; border-radius:7px;
    background:var(--v-surf2); border:1px solid var(--v-line2); white-space:nowrap; }
  .se-spring{ flex:1 1 0; min-width:0; }
  .se-msg{ font-size:11px; color:var(--v-emerald); }
  .se-msg.err{ color:var(--v-rose); }

  .se-meta{ display:flex; gap:12px; flex-wrap:wrap; }
  .se-meta label{ display:flex; flex-direction:column; gap:5px; flex:1 1 150px; }
  .se-meta .se-msm{ flex:0 1 90px; }

  /* arrangements */
  .se-arr{ display:flex; flex-direction:column; gap:12px; padding:14px; border:1px solid var(--v-line);
    border-radius:14px; background:var(--v-surf); }
  .se-arrhead{ display:flex; align-items:center; gap:10px; flex-wrap:wrap; }
  .se-arrlbl{ margin:0; flex:0 0 auto; }
  .se-arrpills{ display:flex; align-items:center; gap:7px; flex-wrap:wrap; }
  .se-arrpill{ display:inline-flex; align-items:center; gap:7px; font-family:var(--f-mono); font-size:10.5px;
    letter-spacing:.03em; color:var(--v-dim); padding:5px 11px; border-radius:999px; background:var(--v-surf2);
    border:1px solid var(--v-line2); }
  .se-arrpill.act{ cursor:pointer; transition:.12s; }
  .se-arrpill.act:hover{ color:var(--v-txt); border-color:var(--v-line); }
  .se-arrpill.on{ color:var(--v-amber); border-color:var(--v-amber); background:var(--v-amber-soft); }
  .se-arrcount{ font-size:9px; color:var(--v-faint); padding:1px 5px; border-radius:5px; background:var(--v-surf3); }

  .se-arrbuild{ display:flex; flex-direction:column; gap:11px; padding-top:2px; }
  .se-arrrow1{ display:flex; align-items:center; gap:8px; }
  .se-arrname{ flex:1 1 240px; max-width:340px; }
  .se-seq{ display:flex; align-items:center; gap:7px; flex-wrap:wrap; min-height:34px; padding:8px 10px;
    border:1px dashed var(--v-line2); border-radius:10px; background:var(--v-surf2); }
  .se-seqempty{ font-size:10px; letter-spacing:.03em; color:var(--v-faint); }
  .se-seqchip{ display:inline-flex; align-items:center; gap:2px; font-family:var(--f-mono); font-size:10px;
    font-weight:700; padding:2px 3px 2px 5px; border-radius:7px; }
  .se-seqtag{ padding:0 3px; }
  .se-seqmv{ background:none; border:none; color:inherit; cursor:pointer; font-size:13px; line-height:1; opacity:.6;
    padding:0 1px; }
  .se-seqmv:hover:not(:disabled){ opacity:1; }
  .se-seqmv:disabled{ opacity:.2; cursor:default; }
  .se-seqx{ background:none; border:none; color:inherit; cursor:pointer; font-size:9px; opacity:.6; padding:0 2px; }
  .se-seqx:hover{ opacity:1; }
  .se-palette{ display:flex; align-items:center; gap:7px; flex-wrap:wrap; }
  .se-palchip{ display:inline-flex; align-items:center; gap:6px; font-family:var(--f-mono); font-size:10px;
    font-weight:700; letter-spacing:.03em; padding:5px 9px; border-radius:8px; cursor:pointer; transition:.12s; }
  .se-palchip:hover{ filter:brightness(1.18); }
  .se-palnum{ font-size:8.5px; font-weight:400; opacity:.7; }

  .se-body{ display:grid; grid-template-columns:1fr 300px; gap:16px; align-items:start; }

  .se-grid{ display:grid; grid-template-columns:repeat(3, 1fr); gap:14px; }
  .se-slide{ position:relative; aspect-ratio:16/9; border-radius:12px; border:1px solid var(--v-line);
    background:var(--v-surf2); padding:16px; display:flex; align-items:center; justify-content:center; text-align:center;
    cursor:pointer; overflow:hidden; transition:border-color .12s, box-shadow .12s; }
  .se-slide:hover{ border-color:var(--v-line2); }
  .se-slide.on{ border-color:var(--v-amber); box-shadow:0 0 0 1px var(--v-amber), 0 10px 30px -12px var(--v-amber-glow); }
  .se-slide.drop{ border-color:var(--v-cyan); box-shadow:0 0 0 2px var(--v-cyan); }
  .se-slide{ cursor:grab; }
  .se-slide:active{ cursor:grabbing; }
  .se-tag{ position:absolute; top:11px; left:11px; font-family:var(--f-mono); font-size:9.5px; font-weight:700;
    letter-spacing:.04em; padding:3px 8px; border-radius:6px; }
  .se-lyric{ font-family:var(--f-serif); font-size:14px; line-height:1.4; color:#f4e4c8; white-space:pre-line;
    display:-webkit-box; -webkit-line-clamp:4; -webkit-box-orient:vertical; overflow:hidden; }
  .se-idx{ position:absolute; left:12px; bottom:9px; font-size:10px; color:var(--v-faint); }

  .se-add{ aspect-ratio:16/9; border-radius:12px; border:1.5px dashed var(--v-line2); background:transparent;
    display:flex; flex-direction:column; align-items:center; justify-content:center; gap:6px; cursor:pointer;
    color:var(--v-faint); font-size:11px; letter-spacing:.06em; transition:.14s; }
  .se-add:hover{ color:var(--v-amber); border-color:rgba(245,166,35,.4); }
  .se-addplus{ font-size:22px; line-height:1; }

  /* group colours */
  .tg-cyan{ background:var(--v-cyan-soft); color:var(--v-cyan); border:1px solid rgba(63,182,230,.4); }
  .tg-amber{ background:var(--v-amber-soft); color:var(--v-amber); border:1px solid rgba(245,166,35,.4); }
  .tg-amethyst{ background:var(--v-amethyst-soft); color:var(--v-amethyst); border:1px solid rgba(192,139,255,.4); }
  .tg-rose{ background:var(--v-rose-soft); color:var(--v-rose); border:1px solid rgba(244,113,139,.4); }
  .tg-emerald{ background:var(--v-emerald-soft); color:var(--v-emerald); border:1px solid rgba(16,185,129,.4); }
  .tg-dim{ background:var(--v-surf3); color:var(--v-dim); border:1px solid var(--v-line2); }

  /* editor panel */
  .se-edit{ position:sticky; top:0; background:var(--v-surf); border:1px solid var(--v-line); border-radius:14px; padding:16px;
    display:flex; flex-direction:column; gap:12px; }
  .se-editlbl{ margin:0; }
  .se-field2{ display:flex; gap:10px; }
  .se-field2 label{ display:flex; flex-direction:column; gap:5px; flex:1; }
  .se-tagfield{ flex:0 0 84px !important; }
  .se-lyrfield{ display:flex; flex-direction:column; gap:5px; }
  .se-lyrics{ height:150px; padding:11px 13px; line-height:1.5; resize:vertical; font-family:var(--f-serif); font-size:14px; }
  .se-lyrhint{ font-size:9px; letter-spacing:.03em; color:var(--v-faint); margin-top:2px; }
  .se-splitbtn{ width:100%; justify-content:center; }
  .se-slideacts{ display:flex; align-items:center; gap:6px; }
  .se-del:hover{ color:var(--v-rose); border-color:rgba(244,113,139,.4); }

  @media (max-width:900px){
    .se-body{ grid-template-columns:1fr; }
    .se-grid{ grid-template-columns:repeat(2, 1fr); }
    .se-edit{ position:static; }
  }
</style>
