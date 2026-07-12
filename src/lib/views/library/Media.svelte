<script>
  // Library → Media: imported images, video, and documents (pdf/pptx). Files
  // live on disk; the catalog holds pointers (offline-first). Import happens via
  // the Library's shared Import button, which routes files here by type.
  import { onMount } from 'svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import { listMedia, deleteMedia, fireMedia } from '../../stores/capture.js';

  let items = [];
  let msg = '';
  let msgT;
  onMount(refresh);
  async function refresh() {
    items = await listMedia();
  }
  async function remove(m, ev) {
    ev.stopPropagation();
    await deleteMedia(m.id);
    await refresh();
  }
  function flash(t) {
    msg = t;
    clearTimeout(msgT);
    msgT = setTimeout(() => (msg = ''), 2600);
  }
  async function send(m, ev) {
    ev.stopPropagation();
    try {
      await fireMedia(m.id);
      flash(`Live: ${m.filename}`);
    } catch (e) {
      flash(String(e));
    }
  }

  const KIND = {
    image: { label: 'IMAGE', color: 'var(--v-amethyst)' },
    video: { label: 'VIDEO', color: 'var(--v-amethyst)' },
    document: { label: 'DOC', color: 'var(--v-cyan)' },
  };
  function icon(kind) {
    if (kind === 'image')
      return '<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="8.5" cy="9.5" r="1.6"/><path d="M21 16l-5-5L5 20"/>';
    if (kind === 'video') return '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M10 9l5 3-5 3z"/>';
    return '<path d="M6 2h9l5 5v15H6z"/><path d="M15 2v5h5"/>';
  }
</script>

<div class="med">
  {#if msg}<div class="med-msg r-mono">{msg}</div>{/if}
  {#if items.length}
    <div class="med-grid">
      {#each items as m}
        <div class="med-card">
          <div class="med-thumb" style="--c:{(KIND[m.kind] || {}).color || 'var(--v-faint)'}">
            <svg viewBox="0 0 24 24" width="26" height="26" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round">{@html icon(m.kind)}</svg>
            <span class="med-kind">{(KIND[m.kind] || {}).label || m.kind}</span>
          </div>
          <div class="med-title" title={m.filename}>{m.filename}</div>
          <div class="med-foot">
            {#if m.kind === 'document'}
              <span class="med-meta r-mono">document</span>
            {:else}
              <button class="med-send" title="Send to output" on:click={(e) => send(m, e)}>▶ To output</button>
            {/if}
            <button class="r-iconbtn med-del" title="Delete" on:click={(e) => remove(m, e)}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/></svg>
            </button>
          </div>
        </div>
      {/each}
    </div>
  {:else}
    <EmptyState>No media yet — use <b>Import</b> above. Images &amp; video go here; PDF/PPTX import as documents.</EmptyState>
  {/if}
</div>

<style>
  .med{ display:flex; flex-direction:column; gap:16px; }
  .med-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(200px, 1fr)); gap:12px; }
  .med-card{ display:flex; flex-direction:column; border:1px solid var(--v-line); border-radius:13px; background:var(--v-surf);
    padding:12px; transition:border-color .14s; }
  .med-card:hover{ border-color:var(--v-line2); }
  .med-thumb{ position:relative; aspect-ratio:16/9; border-radius:9px; margin-bottom:10px; display:grid; place-items:center;
    color:var(--c); background:linear-gradient(150deg,var(--v-surf2),var(--v-surf3)); border:1px solid var(--v-line); }
  .med-kind{ position:absolute; top:8px; left:8px; font-family:var(--f-mono); font-size:8px; font-weight:700; letter-spacing:.08em;
    padding:2px 6px; border-radius:5px; background:rgba(0,0,0,.5); color:var(--c); border:1px solid currentColor; }
  .med-title{ font-size:12.5px; font-weight:600; color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .med-foot{ display:flex; align-items:center; justify-content:space-between; margin-top:9px; }
  .med-meta{ font-size:9px; color:var(--v-faint); }
  .med-send{ font-family:var(--f-mono); font-size:9px; font-weight:700; letter-spacing:.06em; color:var(--v-amber);
    background:var(--v-amber-soft); border:1px solid rgba(245,166,35,.3); padding:5px 10px; border-radius:7px; cursor:pointer; }
  .med-send:hover{ background:rgba(245,166,35,.2); }
  .med-del:hover{ color:var(--v-rose); border-color:rgba(244,113,139,.4); }
  .med-msg{ font-size:11.5px; color:var(--v-emerald); margin-bottom:4px; }
</style>
