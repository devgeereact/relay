<script>
  // Library → Scripture: verses the operator has SAVED (not the whole corpus).
  // Search a reference or phrase, save what you want; saved verses render here
  // and are cue sources for the Planner.
  import { onMount, tick } from 'svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import { searchScripture, listSavedScripture, saveScripture, deleteSavedScripture } from '../../stores/capture.js';

  export let startSave = false;

  let saved = [];
  let q = '';
  let results = [];
  let searching = false;
  let msg = '';
  let searchEl;
  let searchTimer;

  onMount(async () => {
    saved = await listSavedScripture();
    if (startSave) {
      await tick();
      searchEl?.focus();
    }
  });

  function onInput() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(doSearch, 220);
  }
  async function doSearch() {
    const s = q.trim();
    if (!s) {
      results = [];
      return;
    }
    searching = true;
    results = await searchScripture(s);
    searching = false;
  }

  async function save(v) {
    try {
      await saveScripture(v.book, v.chapter, v.verse);
      saved = await listSavedScripture();
      msg = `Saved ${v.reference}`;
    } catch (e) {
      msg = String(e);
    }
  }
  async function remove(item, ev) {
    ev.stopPropagation();
    await deleteSavedScripture(item.id);
    saved = await listSavedScripture();
  }
  function isSaved(v) {
    return saved.some((s) => s.reference === v.reference);
  }
</script>

<div class="scr">
  <div class="scr-search">
    <svg class="scr-ic" viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.9"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
    <input bind:this={searchEl} bind:value={q} on:input={onInput} placeholder="Search a reference or phrase to save — John 3:16, ps 23, shepherd" />
    {#if msg}<span class="scr-msg r-mono">{msg}</span>{/if}
  </div>

  {#if results.length}
    <!-- Best match sits proud; the rest are "other places" suggestions. -->
    {@const best = results[0]}
    <div class="scr-bestwrap">
      <div class="r-lbl scr-reslbl">Best match</div>
      <div class="scr-best">
        <div class="scr-bestbody">
          <div class="scr-bestref">{best.reference}{best.translation ? ` · ${best.translation}` : ''}</div>
          <div class="scr-besttext">{best.text}</div>
        </div>
        {#if isSaved(best)}
          <span class="r-badge emerald scr-savedbadge"><span class="bd"></span>Saved</span>
        {:else}
          <button class="r-btn amber sm" on:click={() => save(best)}>Save</button>
        {/if}
      </div>
    </div>
    {#if results.length > 1}
      <div class="r-lbl scr-reslbl">Other places this appears</div>
      <div class="scr-results">
        {#each results.slice(1) as v}
          <div class="scr-result">
            <div class="scr-rbody">
              <span class="scr-rref">{v.reference}</span>
              <span class="scr-rtext">{v.text}</span>
            </div>
            {#if isSaved(v)}
              <span class="r-badge emerald scr-savedbadge"><span class="bd"></span>Saved</span>
            {:else}
              <button class="r-btn ghost sm" on:click={() => save(v)}>Save</button>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  {:else if q.trim() && !searching}
    <div class="scr-hint r-mono">No verses found.</div>
  {/if}

  <div class="r-lbl scr-savedlbl">Saved scripture <span class="scr-count">({saved.length})</span></div>
  {#if saved.length}
    <div class="scr-grid">
      {#each saved as s}
        <div class="scr-card">
          <div class="scr-verse">{s.text}</div>
          <div class="scr-cardfoot">
            <span class="scr-cardref">{s.reference}{s.translation ? ` · ${s.translation}` : ''}</span>
            <button class="r-iconbtn scr-del" title="Remove" on:click={(e) => remove(s, e)}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/></svg>
            </button>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <EmptyState message="No saved verses yet — search above and hit Save." />
  {/if}
</div>

<style>
  .scr{ display:flex; flex-direction:column; gap:16px; }
  .scr-search{ display:flex; align-items:center; gap:11px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:10px; padding:0 14px; height:42px; max-width:640px; }
  .scr-search:focus-within{ border-color:rgba(245,166,35,.45); box-shadow:0 0 0 3px rgba(245,166,35,.08); }
  .scr-ic{ color:var(--v-faint); flex:0 0 auto; }
  .scr-search input{ flex:1; background:transparent; border:0; outline:none; color:var(--v-txt);
    font-family:var(--f-mono); font-size:12.5px; }
  .scr-search input::placeholder{ color:var(--v-faint); }
  .scr-msg{ font-size:10px; color:var(--v-emerald); flex:0 0 auto; }

  .scr-reslbl{ margin-bottom:9px; }
  .scr-bestwrap{ max-width:760px; }
  .scr-best{ display:flex; align-items:center; gap:16px; padding:16px 18px; border-radius:14px;
    border:1px solid rgba(245,166,35,.34); background:radial-gradient(130% 150% at 0% 0%, var(--v-amber-soft), var(--v-surf));
    box-shadow:0 10px 30px -18px var(--v-amber-glow); }
  .scr-bestbody{ flex:1; min-width:0; }
  .scr-bestref{ font-family:var(--f-head); font-weight:700; font-size:17px; color:var(--v-amber); }
  .scr-besttext{ font-family:var(--f-serif); font-size:15px; line-height:1.5; color:#f4e4c8; margin-top:5px; }
  .scr-results{ display:flex; flex-direction:column; gap:7px; max-width:760px; }
  .scr-result{ display:flex; align-items:center; gap:14px; padding:11px 14px; border:1px solid var(--v-line);
    border-radius:11px; background:var(--v-surf); }
  .scr-rbody{ flex:1; min-width:0; }
  .scr-rref{ display:block; font-family:var(--f-head); font-weight:700; font-size:14px; color:var(--v-txt); }
  .scr-rtext{ display:block; font-family:var(--f-serif); font-size:12.5px; color:var(--v-dim); line-height:1.45; margin-top:3px;
    display:-webkit-box; -webkit-line-clamp:2; -webkit-box-orient:vertical; overflow:hidden; }
  .scr-savedbadge{ padding:3px 9px; }
  .scr-hint{ font-size:11px; color:var(--v-faint); }

  .scr-savedlbl{ margin-top:4px; }
  .scr-count{ color:var(--v-faint); letter-spacing:0; }
  .scr-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(240px, 1fr)); gap:12px; }
  .scr-card{ display:flex; flex-direction:column; justify-content:space-between; min-height:132px; padding:15px 16px;
    border:1px solid var(--v-line); border-radius:13px; background:radial-gradient(120% 140% at 50% 20%,#241a10,#0d0a06);
    transition:border-color .14s; }
  .scr-card:hover{ border-color:var(--v-line2); }
  .scr-verse{ font-family:var(--f-serif); font-size:13.5px; line-height:1.45; color:#f4e4c8;
    display:-webkit-box; -webkit-line-clamp:4; -webkit-box-orient:vertical; overflow:hidden; }
  .scr-cardfoot{ display:flex; align-items:center; justify-content:space-between; margin-top:12px; }
  .scr-cardref{ font-family:var(--f-serif); font-style:italic; font-size:11.5px; color:var(--v-amber); }
  .scr-del:hover{ color:var(--v-rose); border-color:rgba(244,113,139,.4); }
</style>
