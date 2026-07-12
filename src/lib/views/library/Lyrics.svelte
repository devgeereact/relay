<script>
  import { songCue } from '../../cues.js';
  // Library → Lyrics: the song catalog. Search, paste/draft new songs, open a
  // song's slide flow, add a song straight to a service plan, delete. File
  // import (.pro / text) is handled by the Library's shared Import button.
  import { onMount } from 'svelte';
  import { trapFocus } from '../../focus.js';
  import { capture, listSongs, searchSongs, importSong, deleteSong, getSong, listPlans, addPlanItem, listArrangements } from '../../stores/capture.js';
  import SongEditor from './SongEditor.svelte';

  export let startPaste = false;

  let openSongId = null; // when set, the slide-flow editor is shown
  let songs = [];
  let q = '';
  let searchTimer;

  // paste / draft form
  let showForm = false;
  let form = { title: '', author: '', key: '', bpm: '', ccli: '', lyrics: '' };
  let saving = false;
  let msg = '';

  // add-to-plan
  let plans = [];
  let planMenuFor = null; // song id whose plan menu is open
  let planMsg = '';

  onMount(async () => {
    await refresh();
    if (startPaste) showForm = true;
  });
  async function refresh() {
    songs = await listSongs();
  }
  function onSearch() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(async () => {
      songs = await searchSongs(q.trim());
    }, 200);
  }

  function resetForm() {
    form = { title: '', author: '', key: '', bpm: '', ccli: '', lyrics: '' };
  }
  async function saveForm() {
    if (!form.title.trim() || !form.lyrics.trim()) {
      msg = 'Title and lyrics are required.';
      return;
    }
    saving = true;
    msg = '';
    try {
      await importSong({
        title: form.title.trim(),
        author: form.author.trim(),
        ccli: form.ccli.trim(),
        key: form.key.trim(),
        bpm: form.bpm ? parseInt(form.bpm, 10) : null,
        lyrics: form.lyrics,
      });
      resetForm();
      showForm = false;
      await refresh();
    } catch (e) {
      msg = String(e);
    }
    saving = false;
  }

  async function remove(s, ev) {
    ev.stopPropagation();
    await deleteSong(s.id);
    await refresh();
  }

  // ── add to plan ──
  async function openPlanMenu(s, ev) {
    ev.stopPropagation();
    plans = await listPlans();
    if (!plans.length) {
      planMsg = 'Create a plan in the Planner first.';
      setTimeout(() => (planMsg = ''), 2600);
      return;
    }
    planMenuFor = planMenuFor === s.id ? null : s.id;
  }
  // Picking a plan for a song: if the song has saved arrangements, open a
  // picker (Standard + each) — else add the Standard order straight away.
  let arrPick = null; // { song, plan, arrangements }
  async function addToPlan(song, plan, ev) {
    ev.stopPropagation();
    const full = await getSong(song.id);
    if (!full) return;
    const arrangements = await listArrangements(full.id);
    if (arrangements.length === 0) {
      await commitToPlan(full, plan, null);
      return;
    }
    planMenuFor = null;
    arrPick = { song: full, plan, arrangements };
  }
  async function commitToPlan(full, plan, arr) {
    const { label, payload } = songCue(full, arr);
    await addPlanItem(plan.id, 'song', label, payload);
    planMenuFor = null;
    arrPick = null;
    planMsg = `Added “${label}” to ${plan.title}.`;
    setTimeout(() => (planMsg = ''), 2600);
  }

  function meta(s) {
    return [s.author, s.song_key && `Key ${s.song_key}`, s.bpm && `${s.bpm} BPM`].filter(Boolean).join(' · ');
  }
</script>

<!-- Escape closes the arrangement picker — see ServicePlanner. Bound at the window,
     because the backdrop it used to be bound to never holds focus, so Escape fell
     through to the global panic key and cleared the congregation's screens. -->
<svelte:window on:keydown={(e) => arrPick && e.key === 'Escape' && (arrPick = null)} />

{#if openSongId}
  <SongEditor songId={openSongId} on:back={() => { openSongId = null; refresh(); }} on:saved={refresh} />
{:else}
  <div class="cat">
    <div class="cat-bar">
      <input class="r-input" placeholder="Search songs by title or author…" bind:value={q} on:input={onSearch} />
      <span class="spring"></span>
      <button class="r-btn ghost sm" on:click={() => (showForm = !showForm)} disabled={!$capture.available}>
        {showForm ? 'Close' : '＋ Paste / Draft'}
      </button>
    </div>

    {#if planMsg}<div class="ly-topmsg r-mono">{planMsg}</div>{/if}
    {#if !$capture.available}
      <div><span class="r-badge rose"><span class="bd"></span>Backend not attached — needs the desktop app</span></div>
    {/if}

    {#if showForm}
      <div class="r-tile ly-form-tile">
        <div class="ly-form">
          <div class="ly-fields">
            <label class="ly-field ly-wide"><span class="r-lbl">Title *</span><input class="r-input" bind:value={form.title} placeholder="Way Maker" /></label>
            <label class="ly-field"><span class="r-lbl">Author</span><input class="r-input" bind:value={form.author} placeholder="Sinach" /></label>
            <label class="ly-field ly-sm"><span class="r-lbl">Key</span><input class="r-input" bind:value={form.key} placeholder="E" /></label>
            <label class="ly-field ly-sm"><span class="r-lbl">BPM</span><input class="r-input" bind:value={form.bpm} placeholder="68" inputmode="numeric" /></label>
            <label class="ly-field"><span class="r-lbl">CCLI</span><input class="r-input" bind:value={form.ccli} placeholder="7115744" /></label>
          </div>
          <label class="ly-field">
            <span class="r-lbl">Lyrics</span>
            <textarea class="r-input ly-lyrics" bind:value={form.lyrics} placeholder={"[Verse 1]\nYou are here, moving in our midst\n\n[Chorus]\nWay maker, miracle worker"}></textarea>
            <span class="ly-hint r-mono">Section headers ([Verse 1], Chorus:, V1) or blank lines between parts — split automatically.</span>
          </label>
          <div class="ly-formactions">
            {#if msg}<span class="ly-msg r-mono">{msg}</span>{/if}
            <span class="spring"></span>
            <button class="r-btn ghost sm" on:click={() => { showForm = false; msg = ''; }}>Cancel</button>
            <button class="r-btn amber sm" on:click={saveForm} disabled={saving}>{saving ? 'Saving…' : 'Save to Library'}</button>
          </div>
        </div>
      </div>
    {/if}

    {#if songs.length}
      <div class="song-grid">
        {#each songs as s}
          <div class="song-card">
            <button class="song-hit r-focus" on:click={() => (openSongId = s.id)} title="Open slide flow">
              <div class="song-head">
                <span class="song-badge">SONG</span>
                <span class="song-slides r-mono">{s.section_count}<i>slide{s.section_count === 1 ? '' : 's'}</i></span>
              </div>
              <div class="song-title">{s.title}</div>
              <div class="song-meta r-mono">{meta(s) || 'No metadata'}</div>
            </button>
            <div class="song-foot">
              <button class="song-addplan r-focus" on:click={(e) => openPlanMenu(s, e)}>＋ Plan</button>
              <span class="song-foot-r">
                <span class="song-open r-mono">Slide flow ›</span>
                <button class="r-iconbtn song-del" title="Delete song" on:click={(e) => remove(s, e)}>
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/></svg>
                </button>
              </span>
            </div>
            {#if planMenuFor === s.id}
              <button class="song-planscrim" tabindex="-1" aria-label="Close" on:click={(e) => { e.stopPropagation(); planMenuFor = null; }}></button>
              <div class="song-planmenu">
                <div class="song-planlbl r-mono">Add to plan</div>
                {#each plans as p}
                  <button class="song-planitem" on:click={(e) => addToPlan(s, p, e)}>{p.title} <span class="r-mono">{p.cue_count}</span></button>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {:else}
      <div class="cat-empty"><span class="r-empty">{q.trim() ? 'No songs match.' : 'No songs yet — Import a .pro playlist or paste one with ＋ New.'}</span></div>
    {/if}
  </div>

  <!-- arrangement picker — a song with saved arrangements added to a plan -->
  {#if arrPick}
    <!-- Mouse convenience only; not focusable, not claiming to be a button. The
         keyboard path is Escape, handled at the window (top of this file). -->
    <!-- svelte-ignore a11y-click-events-have-key-events a11y-no-noninteractive-element-interactions -->
    <div class="ly-arrback" role="presentation" on:click={() => (arrPick = null)}>
      <div class="ly-arrsheet" role="dialog" aria-modal="true" aria-label="Choose arrangement" use:trapFocus
        on:click|stopPropagation on:keydown|stopPropagation>
        <div class="ly-arrtitle">Add “{arrPick.song.title}” to {arrPick.plan.title}</div>
        <div class="r-lbl ly-arrsub">Choose an arrangement</div>
        <button class="ly-arropt r-focus" on:click={() => commitToPlan(arrPick.song, arrPick.plan, null)}>
          <span class="ly-arroptname">Standard</span>
          <span class="ly-arroptseq r-mono">{arrPick.song.sections.length} sections · in order</span>
        </button>
        {#each arrPick.arrangements as a (a.id)}
          <button class="ly-arropt r-focus" on:click={() => commitToPlan(arrPick.song, arrPick.plan, a)}>
            <span class="ly-arroptname">{a.name}</span>
            <span class="ly-arroptseq r-mono">{a.sequence.map((i) => (arrPick.song.sections[i]?.tag ?? '?')).join(' · ')}</span>
          </button>
        {/each}
        <button class="r-btn ghost sm ly-arrcancel" on:click={() => (arrPick = null)}>Cancel</button>
      </div>
    </div>
  {/if}
{/if}

<style>
  .ly-topmsg{ font-size:11.5px; color:var(--v-emerald); }
  .ly-form-tile{ padding:16px; }
  .ly-form{ display:flex; flex-direction:column; gap:14px; }
  .ly-fields{ display:flex; flex-wrap:wrap; gap:12px; }
  .ly-field{ display:flex; flex-direction:column; gap:6px; flex:1 1 140px; }
  .ly-field .r-lbl{ margin-bottom:0; }
  .ly-wide{ flex:2 1 240px; }
  .ly-sm{ flex:0 1 90px; }
  .ly-lyrics{ height:170px; padding:11px 13px; line-height:1.5; resize:vertical; font-family:var(--f-body); }
  .ly-hint{ font-size:10px; color:var(--v-faint); margin-top:6px; }
  .ly-formactions{ display:flex; align-items:center; gap:10px; }
  .ly-msg{ font-size:11px; color:var(--v-rose); }
  .ly-formactions .spring{ flex:1; }

  /* arrangement picker (add-to-plan) */
  .ly-arrback{ position:fixed; inset:0; background:rgba(6,6,8,.6); backdrop-filter:blur(3px); z-index:200;
    display:flex; align-items:center; justify-content:center; padding:24px; }
  .ly-arrsheet{ width:min(400px, 92%); max-height:80%; overflow:auto; background:var(--v-surf); border:1px solid var(--v-line2);
    border-radius:16px; padding:18px; display:flex; flex-direction:column; gap:8px; box-shadow:0 24px 70px -20px rgba(0,0,0,.7); }
  .ly-arrtitle{ font-family:var(--f-head); font-weight:700; font-size:15px; color:var(--v-txt); line-height:1.3; }
  .ly-arrsub{ margin:2px 0 6px; }
  .ly-arropt{ display:flex; flex-direction:column; gap:3px; width:100%; text-align:left; padding:10px 12px;
    border-radius:10px; background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt); cursor:pointer; transition:.12s; }
  .ly-arropt:hover{ border-color:var(--v-amber); background:var(--v-amber-soft); }
  .ly-arroptname{ font-weight:600; font-size:13px; }
  .ly-arroptseq{ font-size:9.5px; letter-spacing:.03em; color:var(--v-faint); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .ly-arrcancel{ align-self:flex-end; margin-top:4px; }

  /* Song cards — image-free, dense, professional. */
  .song-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(240px, 1fr)); gap:12px; }
  .song-card{ position:relative; display:flex; flex-direction:column; background:var(--v-surf); border:1px solid var(--v-line);
    border-radius:13px; padding:14px 15px 11px; overflow:visible; transition:border-color .14s, background .14s; }
  .song-card:hover{ border-color:var(--v-line2); background:var(--v-surf2); }
  .song-card::before{ content:""; position:absolute; left:0; top:14px; bottom:14px; width:3px; border-radius:0 3px 3px 0; background:var(--v-amber); }
  .song-hit{ display:block; width:100%; text-align:left; background:none; border:0; padding:0 0 0 7px; cursor:pointer; color:inherit; }
  .song-head{ display:flex; align-items:center; justify-content:space-between; margin-bottom:11px; }
  .song-badge{ font-family:var(--f-mono); font-size:8.5px; font-weight:700; letter-spacing:.14em; color:var(--v-amber);
    background:var(--v-amber-soft); border:1px solid rgba(245,166,35,.3); padding:3px 8px; border-radius:6px; }
  .song-slides{ font-size:15px; font-weight:600; color:var(--v-txt); display:inline-flex; align-items:baseline; gap:5px; }
  .song-slides i{ font-style:normal; font-size:8.5px; font-weight:500; letter-spacing:.1em; text-transform:uppercase; color:var(--v-faint); }
  .song-title{ font-family:var(--f-head); font-size:15px; font-weight:600; line-height:1.28; color:var(--v-txt);
    display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; min-height:38px; }
  .song-meta{ font-size:9.5px; color:var(--v-faint); margin-top:6px; letter-spacing:.02em; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .song-foot{ display:flex; align-items:center; justify-content:space-between; margin:11px 0 0 7px; padding-top:10px; border-top:1px solid var(--v-line); }
  .song-addplan{ font-family:var(--f-mono); font-size:9px; font-weight:700; letter-spacing:.06em; color:var(--v-amber);
    background:var(--v-amber-soft); border:1px solid rgba(245,166,35,.3); padding:4px 9px; border-radius:7px; cursor:pointer; }
  .song-addplan:hover{ background:rgba(245,166,35,.2); }
  .song-foot-r{ display:flex; align-items:center; gap:9px; }
  .song-open{ font-size:8.5px; letter-spacing:.06em; color:var(--v-faint); transition:color .14s; }
  .song-card:hover .song-open{ color:var(--v-amber); }
  .song-del{ width:28px; height:28px; }
  .song-del:hover{ color:var(--v-rose); border-color:rgba(244,113,139,.4); }

  .song-planscrim{ position:fixed; inset:0; z-index:40; background:transparent; border:0; }
  .song-planmenu{ position:absolute; left:15px; right:15px; bottom:52px; z-index:50; padding:6px; max-height:180px; overflow-y:auto;
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:10px; box-shadow:0 18px 40px -16px #000; }
  .song-planlbl{ font-size:8px; letter-spacing:.14em; text-transform:uppercase; color:var(--v-faint); padding:4px 8px 6px; }
  .song-planitem{ display:flex; align-items:center; justify-content:space-between; gap:10px; width:100%; text-align:left;
    padding:8px 9px; border-radius:7px; border:0; background:transparent; color:var(--v-txt); font-size:12px; cursor:pointer; }
  .song-planitem:hover{ background:var(--v-surf3); color:var(--v-amber); }
  .song-planitem .r-mono{ font-size:9px; color:var(--v-faint); }
</style>
