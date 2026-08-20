<script>
  import { humanError } from '../../errors.js';
  // Pre-save import review — edit parsed songs (titles, slide text, tags, order)
  // BEFORE they land in the library, so there's no import-then-fix-then-replace
  // cycle. Save commits everything (dedupe by title). Design-system styling.
  import { createEventDispatcher } from 'svelte';
  import { saveReviewedSongs } from '../../stores/capture.js';

  export let songs = []; // [{ title, sections:[{tag,label,lyrics}] }]
  const dispatch = createEventDispatcher();

  // Local editable copy; each song carries a `skip` flag.
  let list = songs.map((s) => ({
    skip: false,
    title: s.title,
    author: '',
    song_key: '',
    sections: (s.sections || []).map((x) => ({ tag: x.tag, label: x.label, lyrics: x.lyrics })),
  }));
  let open = list.length === 1 ? 0 : -1; // expand the only song by default
  let saving = false;
  let msg = '';

  function toggle(i) {
    open = open === i ? -1 : i;
  }
  function renumber(song) {
    song.sections = song.sections.map((s, i) =>
      /^\d*$/.test((s.tag || '').trim()) ? { ...s, tag: String(i + 1), label: `Slide ${i + 1}` } : s
    );
  }
  function addSlide(song) {
    song.sections = [...song.sections, { tag: '', label: '', lyrics: '' }];
    renumber(song);
    list = list;
  }
  function removeSlide(song, i) {
    song.sections = song.sections.filter((_, j) => j !== i);
    renumber(song);
    list = list;
  }
  function moveSlide(song, i, dir) {
    const j = i + dir;
    if (j < 0 || j >= song.sections.length) return;
    const a = song.sections.slice();
    [a[i], a[j]] = [a[j], a[i]];
    song.sections = a;
    renumber(song);
    list = list;
  }

  $: keepCount = list.filter((s) => !s.skip).length;

  async function save() {
    const payload = list
      .filter((s) => !s.skip && s.title.trim() && s.sections.some((x) => x.lyrics.trim()))
      .map((s) => ({
        title: s.title.trim(),
        author: s.author,
        song_key: s.song_key,
        sections: s.sections.filter((x) => x.lyrics.trim()),
      }));
    if (!payload.length) {
      msg = 'Nothing to save.';
      return;
    }
    saving = true;
    msg = '';
    try {
      const res = await saveReviewedSongs(payload);
      dispatch('done', res);
    } catch (e) {
      msg = humanError(e); // `String(e)` on a typed error is "[object Object]".
      saving = false;
    }
  }
</script>

<div class="ir">
  <div class="ir-top">
    <div class="ir-title">Review import</div>
    <span class="ir-sub r-mono">{keepCount} of {list.length} song{list.length === 1 ? '' : 's'} · edit before saving</span>
    <span class="ir-spring"></span>
    {#if msg}<span class="ir-msg r-mono" role="alert">{msg}</span>{/if}
    <button class="r-btn ghost sm" on:click={() => dispatch('cancel')}>Cancel</button>
    <button class="r-btn primary sm" on:click={save} disabled={saving || !keepCount}>{saving ? 'Saving…' : `Save ${keepCount} to Library`}</button>
  </div>

  <div class="ir-list">
    {#each list as song, i}
      <div class="ir-song" class:skip={song.skip}>
        <div class="ir-songhead">
          <button class="ir-exp r-focus" on:click={() => toggle(i)} aria-label="Expand">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="transform:rotate({open === i ? 90 : 0}deg);transition:transform .15s"><path d="M9 18l6-6-6-6"/></svg>
          </button>
          <input class="ir-titleinput" bind:value={song.title} placeholder="Song title" disabled={song.skip} />
          <span class="ir-slidecount r-mono">{song.sections.length} slide{song.sections.length === 1 ? '' : 's'}</span>
          <button class="ir-skip" class:on={song.skip} on:click={() => (song.skip = !song.skip)}>
            {song.skip ? 'Skipped' : 'Skip'}
          </button>
        </div>

        {#if open === i && !song.skip}
          <div class="ir-slides">
            {#each song.sections as s, j}
              <div class="ir-slide">
                <div class="ir-slidetop">
                  <input class="ir-tag r-mono" bind:value={s.tag} placeholder="1" />
                  <span class="ir-idx r-mono">{String(j + 1).padStart(2, '0')}</span>
                  <span class="ir-spring"></span>
                  <button class="ir-mini" title="Up" disabled={j === 0} on:click={() => moveSlide(song, j, -1)}>↑</button>
                  <button class="ir-mini" title="Down" disabled={j === song.sections.length - 1} on:click={() => moveSlide(song, j, 1)}>↓</button>
                  <button class="ir-mini danger" title="Remove" on:click={() => removeSlide(song, j)}>✕</button>
                </div>
                <textarea class="ir-lyrics" bind:value={s.lyrics} placeholder="Slide text…" rows="2"></textarea>
              </div>
            {/each}
            <button class="ir-addslide" on:click={() => addSlide(song)}>＋ Add slide</button>
          </div>
        {/if}
      </div>
    {/each}
  </div>
</div>

<style>
  .ir{ display:flex; flex-direction:column; gap:16px; }
  .ir-top{ display:flex; align-items:center; gap:12px; flex-wrap:wrap; }
  .ir-title{ font-family:var(--f-head); font-size:22px; font-weight:700; color:var(--v-txt); }
  .ir-sub{ font-size:10.5px; color:var(--v-faint); }
  .ir-spring{ flex:1; }
  .ir-msg{ font-size:11px; color:var(--v-rose); }

  .ir-list{ display:flex; flex-direction:column; gap:10px; }
  .ir-song{ border:1px solid var(--v-line); border-radius:13px; background:var(--v-surf); overflow:hidden; transition:border-color .14s; }
  .ir-song:hover{ border-color:var(--v-line2); }
  .ir-song.skip{ opacity:.5; }
  .ir-songhead{ display:flex; align-items:center; gap:11px; padding:11px 14px; }
  .ir-exp{ width:26px; height:26px; flex:0 0 auto; display:grid; place-items:center; border:0; background:none; color:var(--v-dim); cursor:pointer; border-radius:6px; }
  .ir-exp:hover{ color:var(--v-accent); }
  .ir-titleinput{ flex:1; min-width:0; height:34px; padding:0 11px; border-radius:8px; background:var(--v-bg);
    border:1px solid var(--v-line2); color:var(--v-txt); font-family:var(--f-head); font-weight:600; font-size:15px; outline:none; }
  .ir-titleinput:focus{ border-color:var(--v-accent-line); }
  .ir-slidecount{ font-size:10px; color:var(--v-faint); flex:0 0 auto; }
  .ir-skip{ flex:0 0 auto; font-family:var(--f-mono); font-size:9px; font-weight:700; letter-spacing:.08em; text-transform:uppercase;
    padding:5px 10px; border-radius:7px; border:1px solid var(--v-line2); background:var(--v-surf2); color:var(--v-dim); cursor:pointer; }
  .ir-skip:hover{ color:var(--v-rose); border-color:rgba(239,68,68,.4); }
  .ir-skip.on{ color:var(--v-rose); border-color:rgba(239,68,68,.4); background:var(--v-rose-soft); }

  .ir-slides{ display:grid; grid-template-columns:repeat(auto-fill, minmax(240px, 1fr)); gap:10px; padding:0 14px 14px; }
  .ir-slide{ border:1px solid var(--v-line); border-radius:10px; background:var(--v-surf2); padding:9px; }
  .ir-slidetop{ display:flex; align-items:center; gap:6px; margin-bottom:7px; }
  .ir-tag{ width:52px; height:26px; padding:0 8px; border-radius:6px; background:var(--v-bg); border:1px solid var(--v-line2);
    color:var(--v-accent); font-size:11px; font-weight:700; outline:none; }
  .ir-idx{ font-size:9px; color:var(--v-faint); }
  .ir-mini{ width:24px; height:24px; border-radius:6px; display:grid; place-items:center; cursor:pointer; font-size:11px;
    background:var(--v-surf3); border:1px solid var(--v-line); color:var(--v-dim); }
  .ir-mini:hover:not(:disabled){ color:var(--v-accent); border-color:var(--v-line2); }
  .ir-mini.danger:hover:not(:disabled){ color:var(--v-rose); border-color:rgba(239,68,68,.4); }
  .ir-mini:disabled{ opacity:.3; cursor:not-allowed; }
  .ir-lyrics{ width:100%; padding:9px 11px; border-radius:8px; background:var(--v-bg); border:1px solid var(--v-line2);
    color:var(--v-txt); font-family:var(--f-serif); font-size:13px; line-height:1.4; resize:vertical; outline:none; }
  .ir-lyrics:focus{ border-color:var(--v-accent-line); }
  .ir-addslide{ grid-column:1 / -1; padding:11px; border:1.5px dashed var(--v-line2); border-radius:10px; background:transparent;
    color:var(--v-faint); font-family:var(--f-mono); font-size:11px; letter-spacing:.06em; cursor:pointer; }
  .ir-addslide:hover{ color:var(--v-accent); border-color:var(--v-accent-line); }
</style>
