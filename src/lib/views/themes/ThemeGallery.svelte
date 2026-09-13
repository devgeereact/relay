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

  // The rail's rows. Derived rather than written out twice, so a count and the
  // list it filters to can never disagree.
  $: sets = [
    { key: 'all', label: 'All themes', count: all.length },
    { key: 'builtin', label: 'Built-in', count: BUILTIN_THEMES.length },
    { key: 'custom', label: 'Custom', count: $customThemes.length },
  ];

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

  // A one-word read of the theme's background, so the inspector states a fact
  // rather than printing a raw CSS string an operator cannot parse at a glance.
  function bgLabel(t) {
    const bg = t?.style?.background;
    if (!bg) return 'None (transparent)';
    if (typeof bg === 'string' && bg.includes('gradient')) return 'Gradient';
    return 'Solid colour';
  }
</script>

<!-- THE THEMES WORKSPACE (docs/REBRAND.md §2), in the same three columns as
     Templates: a rail of what you can narrow by, the themes, an inspector. -->
<div class="th-shell">
  <!-- A screen-reader operator navigates by heading. This tab had none at all,
       so there was nothing to jump to and no way to tell where you had landed.
       Visually hidden because the tab bar is already the visible title — the
       heading is for the reader that cannot see it. -->
  <h1 class="sr-only">Themes</h1>

  <aside class="th-pane th-rail">
    <div class="th-panehead"><span class="r-lbl">Sets</span></div>
    <div class="th-railscroll r-scroll">
      {#each sets as s (s.key)}
        <button class="th-prow" class:on={filter === s.key} on:click={() => (filter = s.key)}>
          <span class="th-pn">{s.label}</span>
          <span class="th-pv r-mono">{s.count}</span>
        </button>
      {/each}
    </div>
    <!-- The one sentence that explains what a theme IS relative to a template.
         It belongs beside the list, not at the foot of the inspector where it
         was only read by somebody who had already selected something. -->
    <p class="th-railnote">A theme is the style layer <b>beneath</b> templates. A template overrides it key by key, so a theme sets the defaults and never the last word.</p>
  </aside>

  <section class="th-pane th-main">
    <div class="th-panehead">
      <span class="r-lbl">Themes</span>
      <span class="th-spring"></span>
      <input type="file" accept=".json,application/json" bind:this={fileInput} on:change={onImportFile} style="display:none" />
      <button class="r-btn ghost sm" on:click={() => fileInput.click()}>Import</button>
      <button class="r-btn primary sm" on:click={newTheme}>＋ New theme</button>
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
              </div>
              <div class="th-meta">
                <div class="th-swatch" style="--sw:{t.style?.accent || '#888'}" title="Accent"></div>
                <span class="th-name">{t.name}</span>
                <!-- NOT A STATUS: it says where the theme came from, so it is
                     the muted step and a hairline, never one of the four
                     colours that carry a promise. -->
                {#if t.builtin}<span class="th-badge r-mono">Built-in</span>{/if}
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

  <aside class="th-pane th-insp">
    {#if !sel}
      <div class="th-panehead"><span class="r-lbl">Theme</span></div>
      <div class="th-empty r-empty">Pick a theme to preview it.</div>
    {:else}
      <div class="th-panehead">
        <span class="r-lbl">Theme</span>
        <span class="th-spring"></span>
        {#if sel.builtin}<span class="th-badge r-mono">Built-in</span>{/if}
      </div>
      <div class="th-inspbody r-scroll">
        <div class="th-preview">
          <TemplateRender template={THEME_PREVIEW_TEMPLATE} theme={sel} content={THEME_SAMPLE_CONTENT} />
        </div>
        <div class="th-selname">{sel.name}</div>

        <div class="th-btns">
          {#if sel.builtin}
            <button class="r-btn primary sm" on:click={() => duplicate(sel)}>Duplicate to edit</button>
          {:else}
            <button class="r-btn primary sm" on:click={() => edit(sel)}>Edit theme</button>
            <button class="r-btn ghost sm" on:click={() => duplicate(sel)}>Duplicate</button>
          {/if}
          <button class="r-btn ghost sm" on:click={() => exportTheme(sel)} title="Save this theme as a portable .relaytheme.json file">Export</button>
        </div>

        <dl class="th-info">
          <dt>Name</dt><dd>{sel.name}</dd>
          <dt>Kind</dt><dd>{sel.builtin ? 'Built-in (read-only)' : 'Custom'}</dd>
          <dt>Typeface</dt><dd>{sel.style?.font || 'Renderer default'}</dd>
          <dt>Background</dt><dd>{bgLabel(sel)}</dd>
          <dt>Accent</dt><dd><span class="th-inline-sw" style="--sw:{sel.style?.accent || '#888'}"></span>{sel.style?.accent || 'Renderer default'}</dd>
        </dl>

        {#if sel.builtin}
          <p class="th-hint th-rohint">Built-in themes are read-only. Duplicate this one to get an editable copy.</p>
        {:else}
          <div class="r-lbl th-flbl">Actions</div>
          <div class="th-actions">
            <!-- Two-step, because Tauri's webview has no working confirm() and a
                 delete that reports success without ever showing a dialog is
                 exactly the defect rule 41 exists for. -->
            <button class="r-btn ghost sm th-del" class:arm={delArm === sel.id} on:click={() => del(sel)}>
              {delArm === sel.id ? 'Delete — sure?' : 'Delete'}
            </button>
          </div>
        {/if}
      </div>
    {/if}
  </aside>
</div>

<style>
  /* THREE COLUMNS, the same desk as Templates and the template editor. */
  .th-shell{ display:grid; grid-template-columns:206px minmax(0,1fr) 312px; gap:12px; height:100%; min-height:0; }
  @media (max-width:1180px){ .th-shell{ grid-template-columns:176px minmax(0,1fr) 276px; } }
  @media (max-width:980px){ .th-shell{ grid-template-columns:1fr; height:auto; } }

  .th-pane{ display:flex; flex-direction:column; min-height:0; overflow:hidden;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  .th-panehead{ display:flex; align-items:center; gap:8px; padding:0 10px; height:34px; flex:0 0 auto;
    border-bottom:1px solid var(--v-line); }
  .th-spring{ flex:1; }

  /* ── the rail: dense rows, hairline seams ─────────────────────────────── */
  .th-railscroll{ flex:1; min-height:0; overflow-y:auto; }
  .th-prow{ display:flex; align-items:center; gap:8px; width:100%; height:26px; padding:0 10px;
    border:0; border-bottom:1px solid var(--v-line); background:none; color:var(--v-dim);
    font-family:var(--f-body); font-size:var(--v-fs-b2); text-align:left; cursor:pointer;
    box-shadow:inset 2px 0 0 transparent;
    transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .th-prow:hover{ background:var(--v-surf2); color:var(--v-txt); }
  /* Selection is steel blue and nothing else is (REBRAND §1). */
  .th-prow.on{ background:var(--v-sel-soft); color:var(--v-txt); box-shadow:inset 2px 0 0 var(--v-sel); }
  .th-pn{ flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .th-pv{ flex:0 0 auto; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .th-railnote{ margin:0; padding:10px; border-top:1px solid var(--v-line); flex:0 0 auto;
    font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .th-railnote b{ color:var(--v-dim); }

  /* ── the middle column ────────────────────────────────────────────────── */
  .th-toolbar{ display:flex; align-items:center; gap:8px; flex:0 0 auto; height:34px; padding:0 10px;
    border-bottom:1px solid var(--v-line); }
  .th-search{ display:flex; align-items:center; gap:7px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-sm); padding:0 9px; height:24px; flex:1 1 200px; max-width:280px; }
  .th-search:focus-within{ border-color:var(--v-sel-line); }
  .th-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .th-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt); font-size:var(--v-fs-b2); }
  .th-search input::placeholder{ color:var(--v-faint); }

  .th-scroll{ flex:1; min-height:0; overflow-y:auto; padding:10px; }
  .th-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(196px, 1fr)); gap:10px; }
  .th-card{ display:flex; flex-direction:column; background:var(--v-surf2); border:1px solid var(--v-line);
    border-radius:var(--v-r-md); overflow:hidden; cursor:pointer;
    transition:border-color var(--v-dur) var(--v-ease); }
  .th-card:hover{ border-color:var(--v-line2); }
  .th-card.sel{ border-color:var(--v-sel); box-shadow:0 0 0 1px var(--v-sel); }
  /* position:relative is load-bearing — TemplateRender's root is
     position:absolute; inset:0 and supplies its own container-type. */
  .th-thumb{ position:relative; aspect-ratio:16/9; background:var(--v-void); overflow:hidden; flex:0 0 auto; }
  .th-meta{ display:flex; align-items:center; gap:7px; padding:6px 8px; min-width:0; }
  .th-name{ flex:1; min-width:0; font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .th-badge{ flex:0 0 auto; padding:1px 6px; border:1px solid var(--v-line2); border-radius:var(--v-r-sm);
    font-size:9px; letter-spacing:var(--v-tr-caps); text-transform:uppercase; color:var(--v-faint); }
  .th-swatch{ width:13px; height:13px; border-radius:2px; background:var(--sw); border:1px solid var(--v-line2); flex:0 0 auto; }

  .th-err{ flex:0 0 auto; margin:8px; padding:8px 10px; border:1px solid var(--v-rose); border-radius:var(--v-r-sm);
    background:var(--v-rose-soft); color:var(--v-rose); font-size:var(--v-fs-cap); }

  /* ── inspector ────────────────────────────────────────────────────────── */
  .th-inspbody{ flex:1; min-height:0; overflow-y:auto; padding:10px; }
  .th-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md); border:1px solid var(--v-line2);
    overflow:hidden; background:var(--v-void); }
  .th-selname{ margin:8px 0 0; font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600;
    color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .th-btns{ display:flex; flex-wrap:wrap; gap:5px; margin-top:8px; }
  .th-btns .r-btn{ flex:1 1 auto; justify-content:center; }
  .th-info{ display:grid; grid-template-columns:auto 1fr; gap:5px 12px; margin:12px 0 0; font-size:var(--v-fs-b2); }
  .th-info dt{ color:var(--v-faint); }
  .th-info dd{ margin:0; color:var(--v-txt); overflow-wrap:anywhere; display:flex; align-items:center; gap:6px; }
  .th-inline-sw{ width:12px; height:12px; border-radius:2px; background:var(--sw); border:1px solid var(--v-line2); flex:0 0 auto; }
  .th-flbl{ margin:14px 0 6px; }
  .th-actions{ display:flex; gap:5px; }
  .th-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  .th-del{ color:var(--v-rose); }
  .th-del:hover, .th-del.arm{ border-color:var(--v-rose); background:var(--v-rose-soft); }
  .th-hint{ margin:14px 0 0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .th-rohint{ padding:8px 10px; border:1px solid var(--v-line2); border-radius:var(--v-r-sm); background:var(--v-surf2); }
  .th-empty{ margin:auto; padding:24px; text-align:center; }
</style>
