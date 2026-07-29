<script>
  // Library → Announcements: text notice slides (title + body) the operator
  // drafts ahead of a service and fires like any other cue. A plain content
  // type — the template engine renders it; nothing is special-cased downstream.
  import { onMount } from 'svelte';
  import {
    listAnnouncements,
    saveAnnouncement,
    deleteAnnouncement,
    fireContent,
  } from '../../stores/capture.js';

  export let startDraft = false; // New → "Draft announcement" opens the editor

  let items = [];
  let msg = '';
  let msgT;
  // Editor state: null = list view; else { id, title, body } being edited.
  let edit = null;

  onMount(async () => {
    await refresh();
    if (startDraft) draft();
  });
  async function refresh() {
    items = await listAnnouncements();
  }
  function flash(t) {
    msg = t;
    clearTimeout(msgT);
    msgT = setTimeout(() => (msg = ''), 2600);
  }

  function draft() {
    edit = { id: null, title: '', body: '' };
  }
  function open(a) {
    edit = { id: a.id, title: a.title, body: a.body };
  }
  async function save() {
    const title = edit.title.trim();
    const body = edit.body.trim();
    if (!title && !body) {
      flash('Add a title or body first.');
      return;
    }
    try {
      await saveAnnouncement(edit.id, title, body);
      edit = null;
      await refresh();
      flash('Saved.');
    } catch (e) {
      flash(String(e));
    }
  }
  // Two-step delete (no native confirm — Tauri's webview doesn't implement it).
  let delArm = null;
  let delArmT;
  async function remove(a, ev) {
    ev.stopPropagation();
    if (delArm !== a.id) {
      delArm = a.id;
      clearTimeout(delArmT);
      delArmT = setTimeout(() => (delArm = null), 3000);
      return;
    }
    clearTimeout(delArmT);
    delArm = null;
    await deleteAnnouncement(a.id);
    if (edit && edit.id === a.id) edit = null;
    await refresh();
  }
  async function send(a, ev) {
    ev?.stopPropagation();
    try {
      await fireContent(a.title || '', a.body || a.title, 'announce');
      flash(`Live: ${a.title || 'announcement'}`);
    } catch (e) {
      flash(String(e));
    }
  }
</script>

<div class="ann">
  {#if msg}<div class="ann-msg r-mono">{msg}</div>{/if}

  {#if edit}
    <!-- editor -->
    <div class="ann-editor">
      <div class="ann-ehead">
        <span class="r-lbl">{edit.id ? 'Edit announcement' : 'New announcement'}</span>
        <span class="ann-spring"></span>
        <button class="r-btn ghost sm" on:click={() => (edit = null)}>Cancel</button>
        <button class="r-btn primary sm" on:click={save}>Save</button>
      </div>
      <label class="ann-field">
        <span class="r-lbl">Title</span>
        <input class="r-input" bind:value={edit.title} placeholder="e.g. Midweek service — Wednesday 7pm" />
      </label>
      <label class="ann-field">
        <span class="r-lbl">Body</span>
        <textarea class="r-input ann-body" bind:value={edit.body} placeholder="The notice text shown on screen…"></textarea>
      </label>
    </div>
  {/if}

  {#if items.length}
    <div class="ann-grid">
      {#each items as a (a.id)}
        <button class="ann-card r-focus" class:sel={edit && edit.id === a.id} on:click={() => open(a)}>
          <span class="ann-bar"></span>
          <span class="ann-body-wrap">
            <span class="ann-title">{a.title || 'Untitled'}</span>
            {#if a.body}<span class="ann-preview">{a.body}</span>{/if}
          </span>
          <span class="ann-foot">
            <button class="ann-send" title="Send to output" on:click={(e) => send(a, e)}>▶ To output</button>
            <button class="r-iconbtn ann-del" class:arm={delArm === a.id} title={delArm === a.id ? 'Click again to confirm' : 'Delete'} on:click={(e) => remove(a, e)}>
              {#if delArm === a.id}
                <span class="ann-delconf r-mono">Sure?</span>
              {:else}
                <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18M8 6V4h8v2M6 6l1 14h10l1-14"/></svg>
              {/if}
            </button>
          </span>
        </button>
      {/each}
    </div>
  {:else if !edit}
    <div class="cat-empty">
      <span class="r-empty">No announcements yet — use <b>＋ New → Draft announcement</b> above, or the button below.</span>
      <button class="r-btn primary sm ann-new" on:click={draft}>＋ Draft announcement</button>
    </div>
  {/if}
</div>

<style>
  .ann{ display:flex; flex-direction:column; gap:16px; }
  .ann-msg{ font-size:11.5px; color:var(--v-emerald); }

  .ann-editor{ display:flex; flex-direction:column; gap:12px; padding:16px; border:1px solid var(--v-line2);
    border-radius:14px; background:var(--v-surf); }
  .ann-ehead{ display:flex; align-items:center; gap:8px; }
  .ann-spring{ flex:1; }
  .ann-field{ display:flex; flex-direction:column; gap:5px; }
  .ann-body{ min-height:110px; line-height:1.5; resize:vertical; font-family:var(--f-body); }

  .ann-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(240px, 1fr)); gap:12px; }
  .ann-card{ position:relative; display:flex; flex-direction:column; text-align:left; gap:9px; overflow:hidden;
    border:1px solid var(--v-line); border-radius:13px; background:var(--v-surf); padding:14px 14px 12px 18px;
    cursor:pointer; transition:border-color .14s; }
  .ann-card:hover{ border-color:var(--v-line2); }
  .ann-card.sel{ border-color:var(--v-rose); box-shadow:0 0 0 1px var(--v-rose); }
  .ann-bar{ position:absolute; left:0; top:0; bottom:0; width:4px; background:var(--v-rose); }
  .ann-title{ font-family:var(--f-head); font-weight:700; font-size:14px; color:var(--v-txt); }
  .ann-preview{ display:-webkit-box; -webkit-line-clamp:3; -webkit-box-orient:vertical; overflow:hidden;
    font-size:12px; line-height:1.5; color:var(--v-dim); margin-top:3px; white-space:pre-wrap; }
  .ann-foot{ display:flex; align-items:center; justify-content:space-between; margin-top:auto; }
  .ann-send{ font-family:var(--f-mono); font-size:9px; font-weight:700; letter-spacing:.06em; color:var(--v-accent);
    background:var(--v-accent-soft); border:1px solid var(--v-accent-line); padding:5px 10px; border-radius:7px; cursor:pointer; }
  .ann-send:hover{ background:var(--v-accent-line); }
  .ann-del:hover{ color:var(--v-rose); border-color:rgba(239,68,68,.4); }
  .ann-del.arm{ width:auto; padding:0 8px; color:var(--v-rose); border-color:var(--v-rose); background:var(--v-rose-soft); }
  .ann-delconf{ font-size:9px; font-weight:700; letter-spacing:.04em; }
  .cat-empty{ display:flex; flex-direction:column; align-items:flex-start; gap:12px; }
  .ann-new{ align-self:flex-start; }
</style>
