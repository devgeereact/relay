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
  import WorkspaceFrame from '../WorkspaceFrame.svelte';
  import DeskStrip from '../templates/DeskStrip.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import {
    BUILTIN_THEMES,
    THEME_PREVIEW_TEMPLATE,
    THEME_SAMPLE_CONTENT,
    fontLabel,
  } from '../../themes.js';
  import ErrorState from '../../ui/ErrorState.svelte';
  import { customThemes, loadThemes, saveTheme, deleteTheme, exportTheme, importThemeFromFile, readErrors } from '../../stores/capture.js';
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


<!-- THE THEMES WORKSPACE, in the shared workspace grammar
     (`WorkspaceFrame.svelte`, docs/REBRAND.md §2 and §11) — the same rail ·
     main · inspector as Templates and Outputs, from the one definition. The
     frame carries the page's <h1>; each pane head carries an <h2>. -->
<WorkspaceFrame
  title="Themes"
  standfirst="The style layer beneath templates. A template overrides it key by key."
  columns="var(--v-rail) minmax(0,1fr) var(--v-insp)">
  <svelte:fragment slot="head">
    <!-- THE SAME STRIP the Templates desk renders, from the one component — so
         the two desks cannot offer different sets of desks, and the way back is
         where the way here was. -->
    <DeskStrip desk="themes" on:desk />
    <input type="file" accept=".json,application/json" bind:this={fileInput} on:change={onImportFile} style="display:none" />
    <button class="r-btn ghost sm" on:click={() => fileInput.click()}>Import</button>
    <button class="r-btn primary sm" on:click={newTheme}>＋ New theme</button>
  </svelte:fragment>

  <aside class="rw-pane th-rail">
    <div class="rw-panehead"><h2 class="rw-panettl">Sets</h2></div>
    <nav class="rw-panebody" aria-label="Theme sets">
      {#each sets as s (s.key)}
        <button class="rw-item r-focus" class:on={filter === s.key} on:click={() => (filter = s.key)}>
          <span class="rw-itemname">{s.label}</span>
          <span class="rw-itemn">{s.count}</span>
        </button>
      {/each}
    </nav>
    <div class="rw-panefoot">
      <!-- Built-ins are read-only and there is no control that changes that, so
           the rail says it once rather than every card carrying a lock. -->
      <p class="th-railnote">Built-in themes cannot be edited or deleted. <b>Duplicate</b> one to get an editable copy — that is what "edit a built-in" means here.</p>
    </div>
  </aside>

  <section class="rw-pane">
    <div class="rw-panehead">
      <h2 class="rw-panettl">Themes</h2>
      <span class="rw-spring"></span>
      <div class="th-search">
        <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
        <input placeholder="Search themes…" bind:value={q} aria-label="Search themes" />
      </div>
    </div>

    <div class="rw-panebody pad">
      <!-- RG-95 again, and this pane hid it better than the others because the
           BUILT-INS always render. `loadThemes` swallows to `[]` and records the
           reason; nothing here read it. So a read that failed looked like a church
           that had simply never made a theme — the rail said `Custom 0`, the grid
           said "No theme matches this filter", and the operator's answer to both is
           to build the themes they already have. It sits ABOVE the grid rather than
           inside the else, because the customs are missing whether or not the
           built-ins happen to be on screen. -->
      {#if $readErrors.loadThemes}
        <ErrorState error={$readErrors.loadThemes} onRetry={loadThemes} />
      {/if}
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

    {#if err}<div class="rw-panefoot th-err" role="alert">{err}</div>{/if}
  </section>

  <aside class="rw-pane rw-insp">
    {#if !sel}
      <div class="rw-panehead"><h2 class="rw-panettl">Theme</h2></div>
      <div class="th-empty r-empty">Pick a theme to preview it.</div>
    {:else}
      <div class="rw-panehead">
        <h2 class="rw-panettl">Theme</h2>
        <span class="rw-spring"></span>
        {#if sel.builtin}<span class="th-badge r-mono">Built-in</span>{/if}
      </div>
      <div class="rw-panebody pad">
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

        <!-- A row is a NAME and a VALUE (§11), full-bleed against the pane's own
             12px gutter so the seams reach both edges. A key the theme does not
             pin says "Renderer default" — never a dash, which cannot tell that
             apart from a read that never happened (R3-13).

             Typeface goes through `fontLabel`, NOT through the raw value. The
             stored value is a CSS custom property, so this row printed
             `var(--f-display)` at a volunteer who had opened it to find out what
             the theme looks like. `fontLabel` answers with the same word the
             editor's dropdown uses; the "not pinned" case is answered here,
             because `fontLabel` returns a dash and this row may not. -->
        <!-- TWO ROWS LEFT THIS TABLE, on DECISIONS §69's precedent. **Name**
             printed the heading one element above it, and **Kind** printed the
             `Built-in` badge in this pane's own head — three lines apart, on a
             panel 312px wide, neither of them answerable from anywhere else and
             both of them already answered. What is left is what an operator
             cannot see by looking at the card: the typeface (through
             `fontLabel`, never the raw `var(--f-display)`), what the ground is,
             and the accent as a value rather than as a swatch. -->
        <div class="th-rows">
          <div class="rw-nv"><span class="rw-nvk">Typeface</span><span class="rw-nvv">{sel.style?.font ? fontLabel(sel.style.font) : 'Renderer default'}</span></div>
          <div class="rw-nv"><span class="rw-nvk">Background</span><span class="rw-nvv">{bgLabel(sel)}</span></div>
          <div class="rw-nv">
            <span class="rw-nvk">Accent</span>
            <span class="rw-nvv th-accval"><span class="th-inline-sw" style="--sw:{sel.style?.accent || '#888'}"></span>{sel.style?.accent || 'Renderer default'}</span>
          </div>
        </div>

        {#if !sel.builtin}
          <div class="r-lbl th-flbl">Actions</div>
          <div class="th-actions">
            <!-- Two-step, because Tauri's webview has no working confirm() and a
                 delete that reports success without ever showing a dialog is
                 exactly the defect rule 41 exists for. -->
            <button class="r-btn danger sm th-del" class:arm={delArm === sel.id} on:click={() => del(sel)}>
              {delArm === sel.id ? 'Delete — sure?' : 'Delete'}
            </button>
          </div>
        {/if}
        <p class="rw-foot">A theme is applied to a template in the <b>Templates</b> editor. Templates always override it, key by key.</p>
      </div>
    {/if}
  </aside>
</WorkspaceFrame>

<style>
  /* The pane, the pane head, the rail row and the name/value row all come from
     `WorkspaceFrame.svelte`. What is left is this workspace's own: a grid of
     live theme thumbnails, and the controls around it. */
  .th-railnote{ margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .th-railnote b{ color:var(--v-dim); }

  .th-search{ display:flex; align-items:center; gap:7px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-sm); padding:0 9px; height:24px; flex:1 1 160px; max-width:260px; }
  .th-search:focus-within{ border-color:var(--v-sel-line); }
  .th-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .th-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt); font-size:var(--v-fs-b2); }
  .th-search input::placeholder{ color:var(--v-faint); }

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
    font-size:var(--v-fs-cap); letter-spacing:var(--v-tr-caps); text-transform:uppercase; color:var(--v-faint); }
  .th-swatch{ width:13px; height:13px; border-radius:2px; background:var(--sw); border:1px solid var(--v-line2); flex:0 0 auto; }

  /* The error sits in the pane's own foot, behind the same hairline every other
     footnote uses, rather than floating as a bordered card of its own. */
  .th-err{ color:var(--v-rose); font-size:var(--v-fs-cap); line-height:1.45; }

  .th-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md); border:1px solid var(--v-line2);
    overflow:hidden; background:var(--v-void); }
  .th-selname{ margin:8px 0 0; font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600;
    color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .th-btns{ display:flex; flex-wrap:wrap; gap:5px; margin-top:8px; }
  .th-btns .r-btn{ flex:1 1 auto; justify-content:center; }

  /* Full-bleed rows against the pane body's own 12px gutter, so a seam reaches
     the pane edge while the prose around it keeps the gutter. The border box
     lands exactly on the padding edge — no horizontal overflow. */
  .th-rows{ display:flex; flex-direction:column; margin:12px -12px 0; border-top:1px solid var(--v-line); }
  .th-accval{ display:flex; align-items:center; gap:6px; }
  .th-inline-sw{ width:12px; height:12px; border-radius:2px; background:var(--sw); border:1px solid var(--v-line2); flex:0 0 auto; }
  .th-flbl{ margin:14px 0 6px; }
  .th-actions{ display:flex; gap:5px; }
  .th-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  /* CONVERTED — B2. A third copy of the same hand-rolled destructive button:
     `.r-btn ghost sm` with rose text over a `--v-500` hairline, which is what
     `.r-btn.danger` already is except for the edge. Templates' Delete, Themes'
     Delete and the editor's Delete were three shapes for one word. Only the
     ARMED state is local — the variant has no opinion about rule 41's second
     press, and that is the half worth keeping. */
  .th-del.arm{ border-color:var(--v-red); background:var(--v-red-soft); }
  .th-empty{ margin:auto; padding:24px; text-align:center; }
</style>
