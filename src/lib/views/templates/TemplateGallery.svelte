<script>
  // Templates gallery — browse every template as a live thumbnail, filter by the
  // kind it actually is, and inspect one. Editing opens the editor (the parent
  // switches mode). Nothing here puts content on a wall.
  //
  // Thumbnails are the SAME TemplateRender the output window uses, so a card is
  // what the wall shows, not a drawing of it.
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import ErrorState from '../../ui/ErrorState.svelte';
  import { templateKind, kindsPresent, KIND_META } from '../../templateKind.js';
  import { STARTERS, isLayered, regionsToLayers, CONTENT_KINDS } from '../../layers.js';
  import { testTemplateOnOutputs } from '../../templateTest.js';
  import TemplatePreviewOverlay from '../../TemplatePreviewOverlay.svelte';
  import { humanError } from '../../errors.js';
  import {
    capture,
    templates,
    contentTemplates,
    loadTemplates,
    readErrors,
    saveTemplate,
    saveTemplateQuiet,
    deleteTemplate,
    listOutputChannels,
    exportTemplate,
    importTemplateFromFile,
    defaultTemplateId,
    loadDefaultTemplate,
    setDefaultTemplate,
  } from '../../stores/capture.js';

  const dispatch = createEventDispatcher();

  // Import a template file → a new template. A bad file surfaces its plain-
  // language reason (parseImportedTemplate) via the ONE humaniser.
  let fileInput;
  async function onImportFile(e) {
    err = '';
    const file = e.target.files?.[0];
    e.target.value = ''; // allow re-picking the same file after a fix
    if (!file) return;
    try {
      selId = await importTemplateFromFile(file);
      await loadTemplates();
    } catch (ex) {
      err = humanError(ex);
    }
  }

  let channels = [];
  let filter = 'all';
  let q = '';
  let view = 'grid'; // grid | list
  let sort = 'default'; // default | name | kind
  let selId = null;
  let inspTab = 'details';
  let err = '';

  const SAMPLE = {
    text: 'The LORD is my shepherd; I shall not want. He maketh me to lie down in green pastures: he leadeth me beside the still waters.',
    reference: 'Psalms 23:1-2 · KJV',
  };

  // Have we ASKED yet? A fresh install ships five built-in templates, so an empty
  // list before the first answer is not an empty list — it is a list nobody has
  // read. Without this the first thing a new operator saw on the Templates tab was
  // "No templates yet", which is the one sentence that makes somebody go and build
  // five more. EmptyState's own doc says it: if you do not KNOW the list is empty,
  // you are Loading.
  let asked = false;

  onMount(async () => {
    await loadTemplates();
    asked = true;
    await loadDefaultTemplate();
    await upgradeLegacyToLayers();
    channels = await listOutputChannels().catch(() => []);
    if ($templates.length && selId == null) selId = $templates[0].id;
  });

  // One-time upgrade: convert every legacy region template to editable layers, in
  // place, faithfully (the conversion reproduces the region look as a layer stack
  // — see regionsToLayers). Idempotent: once converted a template is `isLayered`,
  // so a later mount finds nothing to do. Saves quietly, then reloads once.
  let upgrading = false;
  async function upgradeLegacyToLayers() {
    if (upgrading) return;
    const legacy = $templates.filter((t) => !isLayered(t) && Array.isArray(t.layout?.regions));
    if (!legacy.length) return;
    upgrading = true;
    try {
      for (const t of legacy) {
        await saveTemplateQuiet({ ...t, layout: regionsToLayers(t) }).catch(() => {});
      }
      await loadTemplates();
    } finally {
      upgrading = false;
    }
  }

  // The ⋮ row menu.
  //
  // It is positioned FIXED and anchored to the button's screen rect, not nested
  // in the card. The card is `overflow:hidden` (for the rounded thumbnail) and
  // the grid scrolls, so a menu drawn inside a card was clipped — on the bottom
  // row it was cut off entirely and its items could not be clicked. Fixed
  // positioning escapes every overflow context.
  let menuFor = null;
  let menuTpl = null;
  let menuPos = { x: 0, y: 0 };
  function openMenu(e, t) {
    if (menuFor === t.id) { menuFor = null; return; }
    const r = e.currentTarget.getBoundingClientRect();
    const W = 150;
    const H = 152; // four items + padding
    let y = r.bottom + 4;
    if (y + H > window.innerHeight) y = r.top - H - 4; // flip up near the bottom edge
    menuPos = { x: Math.max(8, r.right - W), y };
    menuTpl = t;
    menuFor = t.id;
  }
  const closeMenu = () => { menuFor = null; newOpen = false; };
  onMount(() => {
    window.addEventListener('click', closeMenu);
    // A fixed menu detaches from a scrolled card, so close it on scroll/resize.
    window.addEventListener('resize', closeMenu);
  });
  onDestroy(() => {
    window.removeEventListener('click', closeMenu);
    window.removeEventListener('resize', closeMenu);
  });

  $: kinds = kindsPresent($templates);
  $: shown = sortList(
    $templates
      .filter((t) => filter === 'all' || templateKind(t) === filter)
      .filter((t) => !q.trim() || t.name.toLowerCase().includes(q.trim().toLowerCase())),
  );
  $: sel = $templates.find((t) => t.id === selId) || null;

  function sortList(list) {
    const a = [...list];
    if (sort === 'name') a.sort((x, y) => x.name.localeCompare(y.name));
    else if (sort === 'kind') a.sort((x, y) => templateKind(x).localeCompare(templateKind(y)) || x.name.localeCompare(y.name));
    // The DEFAULT template floats to the top — it is the fallback look every slide
    // wears, so it is what the operator reaches for first. A stable sort keeps the
    // chosen order within each group.
    a.sort((x, y) => (y.id === $defaultTemplateId ? 1 : 0) - (x.id === $defaultTemplateId ? 1 : 0));
    return a;
  }

  const kindLabel = (t) => KIND_META[templateKind(t)].one;

  // A one-word description of the background, for the Details panel — a real read
  // of style.background, not an invented "resolution / fps".
  function bgLabel(t) {
    const bg = t?.style?.bgImage ? 'image' : t?.style?.background;
    if (!bg) return 'None';
    if (bg === 'image') return 'Image';
    if (bg === 'transparent') return 'Transparent (keys out)';
    if (typeof bg === 'string' && bg.includes('gradient')) return 'Gradient';
    return 'Solid colour';
  }

  // The outputs a template is assigned to — REAL: a channel stores template_id.
  $: assignedChannels = sel ? channels.filter((c) => c.template_id === sel.id) : [];
  // READ-ONLY: the content types this template is the default look for. The ONE
  // writer of the content-type→template map is the Outputs hub matrix (Decision
  // §25); every other surface, this gallery included, only subscribes.
  $: defaultForKinds = sel
    ? CONTENT_KINDS.filter((k) => $contentTemplates[k.key] === sel.id)
    : [];

  // New template = pick a starting point (a layer stack), save it, open the editor.
  let newOpen = false;
  async function newFrom(starter) {
    newOpen = false;
    err = '';
    try {
      const t = starter.make();
      const id = await saveTemplate({ name: starter.label, layout: t.layout, style: t.style });
      selId = id;
      dispatch('edit', { id });
    } catch (e) { err = humanError(e); }
  }
  async function duplicate(t) {
    menuFor = null;
    err = '';
    try {
      // Omit the id so upsert INSERTs a fresh row; copy the real shape.
      const id = await saveTemplate({
        name: `${t.name} copy`,
        layout: structuredClone(t.layout ?? {}),
        style: structuredClone(t.style ?? {}),
      });
      selId = id;
    } catch (e) { err = humanError(e); }
  }
  // Make this template THE default (or clear it if it already is). One default,
  // not a set of four — any template can be it, and any template can still be a
  // screen's own output regardless.
  async function makeDefault(t) {
    err = '';
    try { await setDefaultTemplate($defaultTemplateId === t.id ? null : t.id); }
    catch (e) { err = humanError(e); }
  }

  // Two-step delete (Tauri's webview has no reliable confirm()).
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
    menuFor = null;
    err = '';
    try {
      await deleteTemplate(t.id);
      if (selId === t.id) selId = $templates[0]?.id ?? null;
    } catch (e) { err = humanError(e); }
  }

  // Preview the selected template fullscreen, in-console (Decision §26). The overlay
  // handles its own Esc; this only holds which template it is showing.
  let previewing = null;
  // Fire a sample verse to the LIVE screens using the selected template (Decision
  // §26). A real fire — the operator clears it with Esc. testErr shows a failure.
  let testErr = '';
  async function testOnScreens() {
    testErr = '';
    try {
      await testTemplateOnOutputs(sel.id);
    } catch (e) {
      testErr = humanError(e);
    }
  }

  // Inline rename in the inspector.
  let renaming = false;
  let renameDraft = '';
  function startRename() { renameDraft = sel.name; renaming = true; }
  async function commitRename() {
    renaming = false;
    if (!sel || !renameDraft.trim() || renameDraft === sel.name) return;
    try { await saveTemplate({ ...sel, name: renameDraft.trim() }); }
    catch (e) { err = humanError(e); }
  }
</script>

<div class="tg-shell">
  <!-- A screen-reader operator navigates by heading; this view had none.
       Visually hidden because the tab bar already carries the visible title —
       the heading exists for the reader that cannot see it. -->
  <h1 class="sr-only">Templates</h1>
  <section class="tg-main">
    <!-- Type tabs — DERIVED from each template's shape, so a tab can never claim a
         template it isn't. Only kinds that actually occur are shown; there is no
         empty "Announcements" tab because nothing distinguishes one. -->
    <div class="tg-tabs">
      <button class="tg-tab" class:on={filter === 'all'} on:click={() => (filter = 'all')}>
        All Templates<span class="tg-tabn r-mono">{$templates.length}</span>
      </button>
      {#each kinds as k (k.key)}
        <button class="tg-tab" class:on={filter === k.key} on:click={() => (filter = k.key)}>
          {k.many}<span class="tg-tabn r-mono">{k.count}</span>
        </button>
      {/each}
      <span class="tg-spring"></span>
      <input type="file" accept=".json,application/json" bind:this={fileInput} on:change={onImportFile} style="display:none" />
      <button class="r-btn ghost sm" on:click|stopPropagation={() => fileInput.click()}>Import</button>
      <span class="tg-newwrap">
        <button class="r-btn primary sm" on:click|stopPropagation={() => (newOpen = !newOpen)} disabled={!$capture.available}>＋ New Template</button>
        {#if newOpen}
          <div class="tg-newmenu" on:click|stopPropagation role="menu" tabindex="-1">
            <div class="tg-newsec r-lbl">Start from</div>
            {#each STARTERS as s}
              <button on:click={() => newFrom(s)}>
                <span class="tg-newname">{s.label}</span>
                <span class="tg-newhint">{s.hint}</span>
              </button>
            {/each}
          </div>
        {/if}
      </span>
    </div>

    <div class="tg-toolbar">
      <div class="tg-search">
        <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" aria-hidden="true"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3" stroke-linecap="round"/></svg>
        <input placeholder="Search templates…" bind:value={q} aria-label="Search templates" />
      </div>
      <label class="tg-sort">
        <span class="r-lbl">Sort</span>
        <select class="r-select" bind:value={sort}>
          <option value="default">Default</option>
          <option value="name">Name (A–Z)</option>
          <option value="kind">Type</option>
        </select>
      </label>
      <div class="tg-viewtog">
        <button class:on={view === 'grid'} on:click={() => (view = 'grid')} aria-label="Grid view" title="Grid">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
        </button>
        <button class:on={view === 'list'} on:click={() => (view = 'list')} aria-label="List view" title="List">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/></svg>
        </button>
      </div>
    </div>

    <div class="tg-scroll r-scroll" on:scroll={closeMenu}>
      {#if shown.length}
        <div class="tg-grid" class:list={view === 'list'}>
          {#each shown as t (t.id)}
            <div class="tg-card" class:sel={t.id === selId} class:row={view === 'list'}
              on:click={() => (selId = t.id)} role="button" tabindex="0"
              on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = t.id; } }}>
              <div class="tg-thumb">
                <TemplateRender template={t} content={SAMPLE} />
                <span class="tg-aspect r-mono">16:9</span>
                {#if t.id === selId}<span class="tg-check" aria-hidden="true"><svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><path d="M20 6 9 17l-5-5"/></svg></span>{/if}
              </div>
              <div class="tg-meta">
                <div class="tg-metatext">
                  <span class="tg-name">{t.name}</span>
                  <span class="tg-sub r-mono">{kindLabel(t)} · 16:9</span>
                </div>
                <div class="tg-cardbtns">
                  <!-- Star = THE default template (the fallback look every slide
                       wears). One default, not a set of four; accent when set. Not
                       amber — amber means live on the wall, this only marks a fallback. -->
                  <button class="tg-star" class:on={t.id === $defaultTemplateId}
                    title={t.id === $defaultTemplateId ? 'Default template — click to clear' : 'Make this the default template'}
                    aria-label="Toggle default template" on:click|stopPropagation={() => makeDefault(t)}
                    disabled={!$capture.available}>
                    <svg viewBox="0 0 24 24" width="14" height="14" fill={t.id === $defaultTemplateId ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="m12 3 2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9L12 3Z"/></svg>
                  </button>
                  <button class="tg-more" aria-label="More actions" on:click|stopPropagation={(e) => openMenu(e, t)}>
                    <svg viewBox="0 0 24 24" width="15" height="15" fill="currentColor" aria-hidden="true"><circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/></svg>
                  </button>
                </div>
              </div>
            </div>
          {/each}
        </div>
      {:else if !asked && !$readErrors.loadTemplates}
        <Loading what="templates" />
      {:else if $readErrors.loadTemplates}
        <!-- THREE FACTS, NOT TWO. A fresh install ships five built-in templates, so
             "No templates yet — create one to start" was never a thing this screen
             could truthfully say about an empty list; it could only ever mean the
             read failed. An operator told their five templates do not exist is about
             to make five more. `readErrors` carries the reason the GROUP 2 wrapper
             used to discard. -->
        <ErrorState error={$readErrors.loadTemplates} onRetry={() => loadTemplates()} />
      {:else}
        <!-- …and the empty state OFFERS the thing it is telling them to do.
             `EmptyState` styles a button in its slot — it was built expecting an
             action — and telling a volunteer to create one and then making them
             find the control is the version of help that costs them a minute in a
             dark booth. The filter case gets no button: the templates exist, the
             filter is the problem, and a New button there would be an answer to a
             question nobody asked. -->
        <EmptyState message={$templates.length ? 'No template matches this filter.' : 'No templates yet — create one to start.'}>
          {#if !$templates.length}
            <button class="r-btn primary sm" on:click={() => (newOpen = true)}>New template</button>
          {/if}
        </EmptyState>
      {/if}
    </div>

    {#if err}<div class="tg-err" role="alert">{err}</div>{/if}
  </section>

  <!-- ══ PREVIEW / INSPECTOR ══ -->
  <aside class="tg-insp">
    {#if !sel}
      <div class="tg-insphead"><span class="tg-inspttl">Template Preview</span></div>
      <div class="tg-empty r-empty">Pick a template to preview it.</div>
    {:else}
      <div class="tg-insphead">
        <span class="tg-inspttl">Template Preview</span>
        <span class="tg-aspect r-mono static">16:9</span>
      </div>

      <div class="tg-inspbody r-scroll">
        <div class="tg-preview">
          <TemplateRender template={sel} content={SAMPLE} />
        </div>

        <div class="tg-previewbtns">
          <button class="r-btn primary sm" on:click={() => (previewing = sel)}>
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>
            Preview fullscreen
          </button>
          <button class="r-btn ghost sm" on:click={() => dispatch('edit', { id: sel.id })}>
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
            Edit template
          </button>
        </div>
        <div class="tg-previewbtns">
          <button class="r-btn ghost sm" on:click={testOnScreens} disabled={!$capture.available}
            title="Fires sample scripture to the live screens using this template — clear it with Esc.">
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M5 3v18l15-9L5 3Z"/></svg>
            Test on screens
          </button>
        </div>
        {#if testErr}<p class="tg-testerr" role="alert">{testErr}</p>{/if}

        <div class="r-seg tg-insptabs">
          <button class:on={inspTab === 'details'} on:click={() => (inspTab = 'details')}>Details</button>
          <button class:on={inspTab === 'usage'} on:click={() => (inspTab = 'usage')}>Usage</button>
        </div>

        {#if inspTab === 'details'}
          <dl class="tg-info">
            <dt>Name</dt>
            <dd>
              {#if renaming}
                <!-- svelte-ignore a11y-autofocus -->
                <input class="r-input tg-rename" bind:value={renameDraft} autofocus
                  on:blur={commitRename} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />
              {:else}
                {sel.name}
              {/if}
            </dd>
            <dt>Content type</dt><dd>{kindLabel(sel)}</dd>
            <!-- A READOUT, not a picker: every template is 16:9 by construction
                 (TemplateRender sizes in cqw), so there is no orientation to set. -->
            <dt>Orientation</dt><dd>16:9 · 1920×1080</dd>
            <dt>Background</dt><dd>{bgLabel(sel)}</dd>
            <dt>Default</dt><dd>{sel.id === $defaultTemplateId ? 'Yes — the fallback for every slide' : 'No'}</dd>
          </dl>
          <!-- Created / Last modified / "used 26 times" are in the reference and
               omitted here on purpose: templates carry no timestamps and Relay
               keeps no per-template usage count, so any figure would be invented. -->

          <div class="r-lbl tg-flbl">Actions</div>
          <div class="tg-actions">
            <button class="r-btn ghost sm" class:on={sel.id === $defaultTemplateId} on:click={() => makeDefault(sel)} disabled={!$capture.available}
              title="The default template is the fallback look every slide wears when a screen or content type has no template of its own">
              {sel.id === $defaultTemplateId ? 'Default ✓' : 'Set as default'}
            </button>
            <button class="r-btn ghost sm" on:click={() => duplicate(sel)}>Duplicate</button>
            <button class="r-btn ghost sm" on:click={() => exportTemplate(sel)}>Export</button>
            <button class="r-btn ghost sm" on:click={startRename}>Rename</button>
            <button class="r-btn ghost sm tg-del" class:arm={delArm === sel.id} on:click={() => del(sel)} disabled={!$capture.available}>
              {delArm === sel.id ? 'Click again to confirm' : 'Delete'}
            </button>
          </div>
        {:else}
          <div class="r-lbl tg-flbl">Assigned to outputs</div>
          {#if assignedChannels.length}
            <div class="tg-assigned">
              {#each assignedChannels as c (c.id)}
                <div class="tg-arow">
                  <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/></svg>
                  <span class="tg-aname">{c.name}</span>
                  <span class="tg-atype r-mono">{c.render_target === 'native_window' ? 'display' : c.render_target === 'ndi_encode' ? 'NDI' : 'network'}</span>
                </div>
              {/each}
            </div>
          {:else}
            <p class="tg-fhelp">Not assigned to any screen. Assign it in <b>Outputs → Screens</b>.</p>
          {/if}

          <div class="r-lbl tg-flbl">Default content look</div>
          {#if defaultForKinds.length}
            <div class="tg-chips">
              {#each defaultForKinds as ck (ck.key)}
                <span class="tg-defchip">Default for: {ck.label}</span>
              {/each}
            </div>
          {:else}
            <p class="tg-fhelp">Not set as a default content look.</p>
          {/if}
          <p class="tg-fhelp">Content looks are set in <b>Outputs → Content looks</b> — the one place a content type is bound to a template.</p>
        {/if}
      </div>
    {/if}
  </aside>
</div>

<!-- Fixed-position row menu — anchored to the ⋮ button's screen rect so it is
     never clipped by the card or the scroll area. -->
{#if menuFor && menuTpl}
  <div class="tg-menu" style="left:{menuPos.x}px; top:{menuPos.y}px" on:click|stopPropagation role="menu" tabindex="-1">
    <button on:click={() => { menuFor = null; dispatch('edit', { id: menuTpl.id }); }}>Edit</button>
    <button on:click={() => duplicate(menuTpl)}>Duplicate</button>
    <button on:click={() => { const t = menuTpl; menuFor = null; exportTemplate(t); }}>Export</button>
    <button class="danger" class:arm={delArm === menuTpl.id} on:click={() => del(menuTpl)}>{delArm === menuTpl.id ? 'Click again' : 'Delete'}</button>
  </div>
{/if}

<!-- Fullscreen in-console preview (Decision §26). Handles its own Esc. -->
{#if previewing}
  <TemplatePreviewOverlay template={previewing} onClose={() => (previewing = null)} />
{/if}

<style>
  .tg-shell{ display:grid; grid-template-columns:minmax(0,1fr) 330px; gap:var(--v-sp-md);
    height:100%; min-height:0; }
  @media (max-width:1180px){ .tg-shell{ grid-template-columns:1fr; height:auto; } }

  .tg-main{ display:flex; flex-direction:column; min-height:0; gap:12px; }

  /* tabs */
  .tg-tabs{ display:flex; align-items:center; gap:6px; flex:0 0 auto; flex-wrap:wrap; }
  .tg-tab{ display:inline-flex; align-items:center; gap:7px; padding:7px 13px; border-radius:var(--v-r-md);
    background:var(--v-surf); border:1px solid var(--v-line); color:var(--v-dim); cursor:pointer;
    font-size:var(--v-fs-b2); font-weight:500; transition:.12s; }
  .tg-tab:hover{ border-color:var(--v-line2); color:var(--v-txt); }
  .tg-tab.on{ background:var(--v-accent-fill); border-color:var(--v-accent-fill); color:var(--v-accent-ink); }
  .tg-tabn{ font-size:var(--v-fs-cap); padding:1px 6px; border-radius:99px; background:var(--v-surf3); color:var(--v-dim); }
  .tg-tab.on .tg-tabn{ background:rgba(0,0,0,.28); color:var(--v-accent-ink); }
  .tg-spring{ flex:1; }
  .tg-newwrap{ position:relative; }
  .tg-newmenu{ position:absolute; top:34px; right:0; z-index:40; width:250px; background:var(--v-surf2);
    border:1px solid var(--v-line2); border-radius:var(--v-r-md); box-shadow:var(--v-shadow-lg); padding:5px; }
  .tg-newsec{ padding:6px 8px 4px; }
  .tg-newmenu button{ display:flex; flex-direction:column; gap:2px; width:100%; text-align:left; padding:9px 10px;
    border:0; background:none; color:var(--v-txt); border-radius:var(--v-r-sm); cursor:pointer; }
  .tg-newmenu button:hover{ background:var(--v-surf3); }
  .tg-newname{ font-size:var(--v-fs-b2); font-weight:600; }
  .tg-newhint{ font-size:var(--v-fs-cap); color:var(--v-faint); line-height:1.35; }

  .tg-toolbar{ display:flex; align-items:center; gap:10px; flex:0 0 auto; }
  .tg-search{ display:flex; align-items:center; gap:8px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-md); padding:0 11px; height:32px; flex:1 1 260px; max-width:340px; }
  .tg-search:focus-within{ border-color:var(--v-accent-line); box-shadow:0 0 0 3px var(--v-accent-soft); }
  .tg-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .tg-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt); font-size:var(--v-fs-b2); }
  .tg-search input::placeholder{ color:var(--v-faint); }
  .tg-sort{ display:flex; align-items:center; gap:7px; margin-left:auto; }
  .tg-sort .r-select{ height:32px; }
  .tg-viewtog{ display:flex; gap:2px; background:var(--v-bg); border:1px solid var(--v-line2); border-radius:var(--v-r-md); padding:3px; }
  .tg-viewtog button{ width:30px; height:26px; display:grid; place-items:center; border:0; border-radius:var(--v-r-sm);
    background:none; color:var(--v-faint); cursor:pointer; }
  .tg-viewtog button:hover{ color:var(--v-txt); }
  .tg-viewtog button.on{ background:var(--v-surf3); color:var(--v-accent); }

  .tg-scroll{ flex:1; min-height:0; overflow-y:auto; }
  .tg-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(232px, 1fr)); gap:14px; padding-bottom:8px; }
  .tg-grid.list{ grid-template-columns:1fr; gap:8px; }

  .tg-card{ display:flex; flex-direction:column; background:var(--v-surf); border:1px solid var(--v-line);
    border-radius:var(--v-r-lg); overflow:hidden; cursor:pointer; transition:border-color .12s, box-shadow .12s; }
  .tg-card:hover{ border-color:var(--v-line2); }
  .tg-card.sel{ border-color:var(--v-accent); box-shadow:0 0 0 1px var(--v-accent); }
  .tg-card.row{ flex-direction:row; align-items:stretch; }

  /* position:relative is load-bearing — TemplateRender's root is
     position:absolute; inset:0 and supplies its own container-type. */
  .tg-thumb{ position:relative; aspect-ratio:16/9; background:var(--v-void); overflow:hidden; flex:0 0 auto; }
  .tg-card.row .tg-thumb{ width:200px; aspect-ratio:16/9; }
  .tg-aspect{ position:absolute; top:8px; right:8px; font-size:9px; letter-spacing:.04em; color:var(--v-txt);
    background:rgba(10,10,10,.62); padding:2px 6px; border-radius:var(--v-r-sm); }
  .tg-aspect.static{ position:static; background:var(--v-surf2); color:var(--v-faint); }
  .tg-check{ position:absolute; top:8px; left:8px; width:20px; height:20px; border-radius:50%;
    background:var(--v-accent-fill); color:#fff; display:grid; place-items:center; }

  .tg-meta{ display:flex; align-items:center; gap:8px; padding:10px 11px; flex:1; min-width:0; }
  .tg-metatext{ flex:1; min-width:0; }
  .tg-name{ display:block; font-size:var(--v-fs-b1); font-weight:500; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .tg-sub{ display:block; font-size:var(--v-fs-cap); color:var(--v-faint); margin-top:1px; }
  .tg-cardbtns{ display:flex; align-items:center; gap:2px; flex:0 0 auto; position:relative; }
  .tg-star{ width:28px; height:28px; display:grid; place-items:center; border:0; background:none;
    color:var(--v-faint); cursor:pointer; border-radius:var(--v-r-sm); }
  .tg-star.on{ color:var(--v-accent); }
  .tg-star:disabled{ opacity:.3; cursor:not-allowed; }
  .tg-more{ width:28px; height:28px; display:grid; place-items:center; border:0; background:none;
    color:var(--v-faint); cursor:pointer; border-radius:var(--v-r-sm); }
  .tg-more:hover{ color:var(--v-txt); background:var(--v-surf3); }
  .tg-menu{ position:fixed; z-index:200; width:150px; display:flex; flex-direction:column;
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:var(--v-r-md);
    box-shadow:var(--v-shadow-lg); padding:4px; }
  .tg-menu button{ text-align:left; padding:7px 10px; border:0; background:none; color:var(--v-txt);
    font-size:var(--v-fs-b2); border-radius:var(--v-r-sm); cursor:pointer; }
  .tg-menu button:hover{ background:var(--v-surf3); }
  .tg-menu .danger{ color:var(--v-rose); }
  .tg-menu .danger.arm{ background:var(--v-rose-soft); }

  .tg-err{ flex:0 0 auto; padding:9px 12px; border:1px solid var(--v-rose); border-radius:var(--v-r-md);
    background:var(--v-rose-soft); color:var(--v-rose); font-size:var(--v-fs-cap); }

  /* inspector */
  .tg-insp{ display:flex; flex-direction:column; min-height:0; background:var(--v-surf);
    border:1px solid var(--v-line); border-radius:var(--v-r-lg); overflow:hidden; }
  .tg-insphead{ display:flex; align-items:center; justify-content:space-between; gap:10px; padding:12px 14px;
    border-bottom:1px solid var(--v-line); flex:0 0 auto; }
  .tg-inspttl{ font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .tg-inspbody{ flex:1; min-height:0; overflow-y:auto; padding:14px; }
  .tg-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md); border:1px solid var(--v-line2);
    overflow:hidden; background:var(--v-void); }
  .tg-previewbtns{ display:flex; gap:6px; margin-top:10px; }
  .tg-previewbtns .r-btn{ flex:1; justify-content:center; }
  .tg-testerr{ margin:8px 0 0; padding:8px 10px; border:1px solid var(--v-rose); border-radius:var(--v-r-md);
    background:var(--v-rose-soft); color:var(--v-rose); font-size:var(--v-fs-cap); line-height:1.45; }
  .tg-insptabs{ margin:14px 0 4px; width:100%; }
  .tg-insptabs :global(button){ flex:1; }

  .tg-info{ display:grid; grid-template-columns:auto 1fr; gap:6px 12px; margin:12px 0 0; font-size:var(--v-fs-b2); }
  .tg-info dt{ color:var(--v-faint); }
  .tg-info dd{ margin:0; color:var(--v-txt); overflow-wrap:anywhere; }
  .tg-rename{ height:28px; padding:2px 8px; }

  .tg-flbl{ margin:15px 0 7px; }
  .tg-fhelp{ margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .tg-fhelp b{ color:var(--v-dim); }
  .tg-actions{ display:flex; flex-wrap:wrap; gap:6px; }
  .tg-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  .tg-del{ color:var(--v-rose); }
  .tg-del:hover:not(:disabled), .tg-del.arm{ border-color:var(--v-rose); background:var(--v-rose-soft); }

  .tg-assigned{ display:flex; flex-direction:column; gap:6px; }
  .tg-arow{ display:flex; align-items:center; gap:9px; padding:8px 10px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); }
  .tg-arow svg{ color:var(--v-faint); flex:0 0 auto; }
  .tg-aname{ flex:1; min-width:0; font-size:var(--v-fs-b2); color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .tg-atype{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .tg-chips{ display:flex; flex-wrap:wrap; gap:6px; }
  .tg-defchip{ display:inline-flex; align-items:center; padding:4px 10px; border-radius:99px;
    background:var(--v-accent-soft); border:1px solid var(--v-accent-line); color:var(--v-txt);
    font-size:var(--v-fs-cap); font-weight:500; }

  .tg-empty{ margin:auto; padding:24px; text-align:center; }
</style>
