<script>
  // Theme gallery — browse every theme (builtins + the operator's custom ones)
  // as a LIVE thumbnail, and inspect/edit one. Each card is the SAME
  // TemplateRender the wall uses, drawing a near-empty preview template with the
  // theme applied — so a card looks like the theme, not a drawing of it.
  //
  // Builtins are READ-ONLY: they can be previewed and duplicated, never edited or
  // deleted. Editing a builtin means "duplicate, then edit the copy".
  import { createEventDispatcher, onMount } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import {
    BUILTIN_THEMES,
    THEME_PREVIEW_TEMPLATE,
    THEME_SAMPLE_CONTENT,
  } from '../../themes.js';
  import { customThemes, loadThemes, saveTheme, deleteTheme, exportTheme, importThemeFromFile } from '../../stores/capture.js';
  import { humanError } from '../../errors.js';

  const dispatch = createEventDispatcher();

  // Import a theme file → a new custom theme. A bad file surfaces its plain-
  // language reason (parseImportedTheme) rather than failing silently.
  let fileInput;
  async function onImportFile(e) {
    err = '';
    const file = e.target.files?.[0];
    e.target.value = ''; // let the same file be re-picked after a fix
    if (!file) return;
    try {
      selId = await importThemeFromFile(file);
    } catch (ex) {
      err = humanError(ex);
    }
  }

  let q = '';
  let filter = 'all'; // all | builtin | custom
  let selId = BUILTIN_THEMES[0].id;
  let err = '';

  onMount(loadThemes);

  $: all = [...BUILTIN_THEMES, ...$customThemes];
  $: shown = all
    .filter((t) => filter === 'all' || (filter === 'builtin' ? t.builtin : !t.builtin))
    .filter((t) => !q.trim() || t.name.toLowerCase().includes(q.trim().toLowerCase()));
  $: sel = all.find((t) => t.id === selId) || null;

  // Two-step delete — Tauri's webview has no reliable confirm().
  let delArm = null;
  let delArmT;
  async function del(t) {
    if (delArm !== t.id) {
      delArm = t.id;
      clearTimeout(delArmT);
      delArmT = setTimeout(() => (delArm = null), 3000);
      return;
    }
    clearTimeout(delArmT);
    delArm = null;
    err = '';
    try {
      await deleteTheme(t.id);
      if (selId === t.id) selId = BUILTIN_THEMES[0].id;
    } catch (e) {
      err = humanError(e); // `String(e)` on a typed error is "[object Object]".
    }
  }

  // Duplicate → a NEW custom theme (fresh positive id), then open it in the
  // editor. This is also the ONLY way to "edit" a builtin.
  async function duplicate(t) {
    err = '';
    try {
      const id = await saveTheme({
        name: `${t.name} copy`,
        style: structuredClone(t.style ?? {}),
      });
      selId = id;
      dispatch('edit', { id });
    } catch (e) {
      err = humanError(e); // `String(e)` on a typed error is "[object Object]".
    }
  }

  async function newTheme() {
    err = '';
    try {
      // Seed a new theme from the current selection so it starts from a sensible
      // look rather than blank — an empty theme renders nothing meaningful.
      const seed = sel ?? BUILTIN_THEMES[0];
      const id = await saveTheme({ name: 'New theme', style: structuredClone(seed.style ?? {}) });
      selId = id;
      dispatch('edit', { id });
    } catch (e) {
      err = humanError(e); // `String(e)` on a typed error is "[object Object]".
    }
  }

  function edit(t) {
    if (t.builtin) return duplicate(t); // builtins are read-only → edit the copy
    dispatch('edit', { id: t.id });
  }
</script>

<div class="th-shell">
  <section class="th-main">
    <div class="th-tabs">
      <button class="th-tab" class:on={filter === 'all'} on:click={() => (filter = 'all')}>
        All Themes<span class="th-tabn r-mono">{all.length}</span>
      </button>
      <button class="th-tab" class:on={filter === 'builtin'} on:click={() => (filter = 'builtin')}>
        Built-in<span class="th-tabn r-mono">{BUILTIN_THEMES.length}</span>
      </button>
      <button class="th-tab" class:on={filter === 'custom'} on:click={() => (filter = 'custom')}>
        Custom<span class="th-tabn r-mono">{$customThemes.length}</span>
      </button>
      <span class="th-spring"></span>
      <input type="file" accept=".json,application/json" bind:this={fileInput} on:change={onImportFile} style="display:none" />
      <button class="r-btn ghost sm" on:click={() => fileInput.click()}>Import</button>
      <button class="r-btn primary sm" on:click={newTheme}>＋ New Theme</button>
    </div>

    <div class="th-toolbar">
      <div class="th-search">
        <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
        <input placeholder="Search themes…" bind:value={q} aria-label="Search themes" />
      </div>
    </div>

    <div class="th-scroll r-scroll">
      {#if shown.length}
        <div class="th-grid">
          {#each shown as t (t.id)}
            <div class="th-card" class:sel={t.id === selId}
              on:click={() => (selId = t.id)} role="button" tabindex="0"
              on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = t.id; } }}
              on:dblclick={() => edit(t)}>
              <div class="th-thumb">
                <TemplateRender template={THEME_PREVIEW_TEMPLATE} theme={t} content={THEME_SAMPLE_CONTENT} />
                {#if t.builtin}<span class="th-badge r-mono">Built-in</span>{/if}
              </div>
              <div class="th-meta">
                <span class="th-name">{t.name}</span>
                <div class="th-swatch" style="--sw:{t.style?.accent || '#888'}" title="Accent"></div>
              </div>
            </div>
          {/each}
        </div>
      {:else}
        <EmptyState message="No theme matches this filter." />
      {/if}
    </div>

    {#if err}<div class="th-err" role="alert">{err}</div>{/if}
  </section>

  <aside class="th-insp">
    {#if !sel}
      <div class="th-insphead"><span class="th-inspttl">Theme Preview</span></div>
      <div class="th-empty r-empty">Pick a theme to preview it.</div>
    {:else}
      <div class="th-insphead">
        <span class="th-inspttl">Theme Preview</span>
        {#if sel.builtin}<span class="th-badge r-mono static">Built-in</span>{/if}
      </div>
      <div class="th-inspbody r-scroll">
        <div class="th-preview">
          <TemplateRender template={THEME_PREVIEW_TEMPLATE} theme={sel} content={THEME_SAMPLE_CONTENT} />
        </div>

        <div class="th-btns">
          {#if sel.builtin}
            <button class="r-btn primary sm" on:click={() => duplicate(sel)}>Duplicate to edit</button>
          {:else}
            <button class="r-btn primary sm" on:click={() => edit(sel)}>Edit theme</button>
            <button class="r-btn ghost sm" on:click={() => duplicate(sel)}>Duplicate</button>
          {/if}
        </div>
        <div class="th-btns">
          <button class="r-btn ghost sm" on:click={() => exportTheme(sel)} title="Save this theme as a portable .relaytheme.json file">Export theme</button>
        </div>

        <dl class="th-info">
          <dt>Name</dt><dd>{sel.name}</dd>
          <dt>Kind</dt><dd>{sel.builtin ? 'Built-in (read-only)' : 'Custom'}</dd>
          <dt>Typeface</dt><dd>{sel.style?.font || '—'}</dd>
          <dt>Accent</dt><dd><span class="th-inline-sw" style="--sw:{sel.style?.accent || '#888'}"></span>{sel.style?.accent || '—'}</dd>
        </dl>

        {#if !sel.builtin}
          <div class="r-lbl th-flbl">Actions</div>
          <div class="th-actions">
            <button class="r-btn ghost sm th-del" class:arm={delArm === sel.id} on:click={() => del(sel)}>
              {delArm === sel.id ? 'Click again to confirm' : 'Delete'}
            </button>
          </div>
        {/if}

        <p class="th-hint">A theme is applied to a template in the <b>Templates</b> editor. Templates always override the theme, key by key.</p>
      </div>
    {/if}
  </aside>
</div>

<style>
  .th-shell{ display:grid; grid-template-columns:minmax(0,1fr) 330px; gap:var(--v-sp-md); height:100%; min-height:0; }
  @media (max-width:1180px){ .th-shell{ grid-template-columns:1fr; height:auto; } }
  .th-main{ display:flex; flex-direction:column; min-height:0; gap:12px; }

  .th-tabs{ display:flex; align-items:center; gap:6px; flex:0 0 auto; flex-wrap:wrap; }
  .th-tab{ display:inline-flex; align-items:center; gap:7px; padding:7px 13px; border-radius:var(--v-r-md);
    background:var(--v-surf); border:1px solid var(--v-line); color:var(--v-dim); cursor:pointer;
    font-size:var(--v-fs-b2); font-weight:500; transition:.12s; }
  .th-tab:hover{ border-color:var(--v-line2); color:var(--v-txt); }
  .th-tab.on{ background:var(--v-accent-fill); border-color:var(--v-accent-fill); color:var(--v-accent-ink); }
  .th-tabn{ font-size:var(--v-fs-cap); padding:1px 6px; border-radius:99px; background:var(--v-surf3); color:var(--v-dim); }
  .th-tab.on .th-tabn{ background:rgba(0,0,0,.28); color:var(--v-accent-ink); }
  .th-spring{ flex:1; }

  .th-toolbar{ display:flex; align-items:center; gap:10px; flex:0 0 auto; }
  .th-search{ display:flex; align-items:center; gap:8px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-md); padding:0 11px; height:32px; flex:1 1 260px; max-width:340px; }
  .th-search:focus-within{ border-color:var(--v-accent-line); box-shadow:0 0 0 3px var(--v-accent-soft); }
  .th-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .th-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt); font-size:var(--v-fs-b2); }
  .th-search input::placeholder{ color:var(--v-faint); }

  .th-scroll{ flex:1; min-height:0; overflow-y:auto; }
  .th-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(210px, 1fr)); gap:14px; padding-bottom:8px; }
  .th-card{ display:flex; flex-direction:column; background:var(--v-surf); border:1px solid var(--v-line);
    border-radius:var(--v-r-lg); overflow:hidden; cursor:pointer; transition:border-color .12s, box-shadow .12s; }
  .th-card:hover{ border-color:var(--v-line2); }
  .th-card.sel{ border-color:var(--v-accent); box-shadow:0 0 0 1px var(--v-accent); }
  .th-thumb{ position:relative; aspect-ratio:16/9; background:var(--v-void); overflow:hidden; flex:0 0 auto; }
  .th-badge{ position:absolute; top:8px; right:8px; font-size:9px; letter-spacing:.04em; color:var(--v-txt);
    background:rgba(10,10,10,.62); padding:2px 6px; border-radius:var(--v-r-sm); }
  .th-badge.static{ position:static; background:var(--v-surf2); color:var(--v-faint); }
  .th-meta{ display:flex; align-items:center; gap:8px; padding:10px 11px; }
  .th-name{ flex:1; min-width:0; font-size:var(--v-fs-b1); font-weight:500; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .th-swatch{ width:16px; height:16px; border-radius:5px; background:var(--sw); border:1px solid var(--v-line2); flex:0 0 auto; }

  .th-err{ flex:0 0 auto; padding:9px 12px; border:1px solid var(--v-rose); border-radius:var(--v-r-md);
    background:var(--v-rose-soft); color:var(--v-rose); font-size:var(--v-fs-cap); }

  .th-insp{ display:flex; flex-direction:column; min-height:0; background:var(--v-surf);
    border:1px solid var(--v-line); border-radius:var(--v-r-lg); overflow:hidden; }
  .th-insphead{ display:flex; align-items:center; justify-content:space-between; gap:10px; padding:12px 14px;
    border-bottom:1px solid var(--v-line); flex:0 0 auto; }
  .th-inspttl{ font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .th-inspbody{ flex:1; min-height:0; overflow-y:auto; padding:14px; }
  .th-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md); border:1px solid var(--v-line2);
    overflow:hidden; background:var(--v-void); }
  .th-btns{ display:flex; gap:6px; margin-top:10px; }
  .th-btns .r-btn{ flex:1; justify-content:center; }
  .th-info{ display:grid; grid-template-columns:auto 1fr; gap:6px 12px; margin:14px 0 0; font-size:var(--v-fs-b2); }
  .th-info dt{ color:var(--v-faint); }
  .th-info dd{ margin:0; color:var(--v-txt); overflow-wrap:anywhere; display:flex; align-items:center; gap:6px; }
  .th-inline-sw{ width:13px; height:13px; border-radius:3px; background:var(--sw); border:1px solid var(--v-line2); }
  .th-flbl{ margin:15px 0 7px; }
  .th-actions{ display:flex; gap:6px; }
  .th-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  .th-del{ color:var(--v-rose); }
  .th-del:hover, .th-del.arm{ border-color:var(--v-rose); background:var(--v-rose-soft); }
  .th-hint{ margin:15px 0 0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .th-hint b{ color:var(--v-dim); }
  .th-empty{ margin:auto; padding:24px; text-align:center; }
</style>
