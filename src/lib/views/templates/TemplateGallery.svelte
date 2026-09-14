<script>
  // Templates gallery — browse every template as a live thumbnail, filter by the
  // kind it actually is, and inspect one. Editing opens the editor (the parent
  // switches mode). Nothing here puts content on a wall.
  //
  // Thumbnails are the SAME TemplateRender the output window uses, so a card is
  // what the wall shows, not a drawing of it.
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import WorkspaceFrame from '../WorkspaceFrame.svelte';
  import DeskStrip from './DeskStrip.svelte';
  import EmptyState from '../../ui/EmptyState.svelte';
  import Loading from '../../ui/Loading.svelte';
  import ErrorState from '../../ui/ErrorState.svelte';
  import { templateKind, kindsPresent, KIND_META } from '../../templateKind.js';
  import { STARTERS, isLayered, regionsToLayers, CONTENT_KINDS, layerLabel, isKeyedTemplate } from '../../layers.js';
  // THE ONE CAMERA PLATE, shared with Outputs (`ui/CameraPlate.svelte`). A KEYED
  // template — a lower third — paints a band and leaves the rest transparent,
  // because the rest is a camera the switcher supplies. Previewed against
  // nothing, `Lower Third`'s near-black type on a transparent frame is an empty
  // dark rectangle: correct on the wall, useless on a card. PREVIEW ONLY —
  // nothing may put a plate behind a real output, where the transparency is the
  // whole point.
  import CameraPlate from '../../ui/CameraPlate.svelte';
  import { testTemplateOnOutputs } from '../../templateTest.js';
  import TemplatePreviewOverlay from '../../TemplatePreviewOverlay.svelte';
  import { humanError } from '../../errors.js';
  import {
    capture,
    templates,
    contentTemplates,
    setContentTemplate,
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
  // THE ROLE TAG on a card (docs/REBRAND.md §3.3): what this template is FOR, in
  // the prototype's right-hand mono label. DERIVED, like the rail row it belongs
  // to, so a card and its row can never disagree — and deliberately NEUTRAL in
  // colour, because every colour that carries a promise is spoken for (rule 18).
  const kindTag = (t) => KIND_META[templateKind(t)].tag;

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
  // READ-ONLY HERE: the content types this template is the look for. The one
  // WRITER is `setContentTemplate` (capture.js) — called by the Outputs hub matrix
  // and by the template editor's "Used for" (Decision §25, §70). This gallery only
  // subscribes, which is what keeps every surface showing the same answer.
  /** The content kinds a given template is the look for. */
  $: usedFor = (id) => CONTENT_KINDS.filter((k) => $contentTemplates[k.key] === id);
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

  // ── USED FOR ────────────────────────────────────────────────────────────
  // THE SAME WRITER the editor's own "Used for" calls (`setContentTemplate`,
  // DECISIONS §25 and §70), not a second path to the same table. A kind ticked
  // here wears this template on every screen set to Follow the content look; a
  // screen with a look of its own keeps it (DECISIONS §29 — a screen's own
  // template wins). The read side is `$contentTemplates`, the one store, so this
  // control and the rail's look register cannot disagree about what is bound.
  let lookErr = '';
  async function toggleUsedFor(kind) {
    if (!sel) return;
    const mine = $contentTemplates[kind] === sel.id;
    lookErr = '';
    try {
      await setContentTemplate(kind, mine ? null : sel.id);
    } catch (e) {
      lookErr = humanError(e);
    }
  }

  // ── THE OBJECTS ON THIS SLIDE ───────────────────────────────────────────
  // The object tab strip (docs/REBRAND.md §3.2), reading the template's real
  // layers through `layerLabel` — the same namer the editor's strip uses, so the
  // two strips cannot disagree about what an object is called. It wraps rather
  // than scrolls: a tab behind a hidden scrollbar is a tab nobody knows is there.
  //
  // A region-model template has no objects to list; the strip says so in words
  // rather than rendering an empty row, because an empty strip and a template
  // whose objects have not loaded look identical (rule 35).
  $: selLayers = Array.isArray(sel?.layout?.layers) ? sel.layout.layers : [];

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

<!-- THE TEMPLATES WORKSPACE, laid out in the shared workspace grammar
     (`WorkspaceFrame.svelte`, docs/REBRAND.md §2 and §11): a left RAIL of what
     there is, the MAIN area holding the thing in hand, an INSPECTOR of what is
     true of it. Nothing here declares its own three-column body — a fourth copy
     of that grid is how the desks drifted apart the first time.

     The frame carries the page's <h1>; each pane head carries an <h2>. That is
     ordinary document structure rather than a workaround, and it is what keeps a
     screen-reader operator able to jump into a pane rather than only onto the
     page (surface.test.js R3-12). -->
<WorkspaceFrame
  title="Templates"
  standfirst="How a verse, a song or a notice looks on a screen. Editing one repaints every screen already wearing it."
  columns="206px minmax(0,1fr) 312px">
  <svelte:fragment slot="head">
    <!-- THE DESK STRIP. Themes moved INTO this workspace (docs/REBRAND.md §2);
         the shell's strip carries workspaces, and Themes is a desk within
         Templates rather than a workspace beside it. Nothing became
         unreachable: every control the Themes tab carried is still rendered,
         one press away. -->
    <DeskStrip desk="templates" on:desk />
    <input type="file" accept=".json,application/json" bind:this={fileInput} on:change={onImportFile} style="display:none" />
    <button class="r-btn ghost sm" on:click|stopPropagation={() => fileInput.click()}>Import</button>
    <span class="tg-newwrap">
      <button class="r-btn primary sm" on:click|stopPropagation={() => (newOpen = !newOpen)} disabled={!$capture.available}>＋ New template</button>
      {#if newOpen}
        <!-- The click handler is not an interaction: it stops the document-level
             outside-click closer from seeing a click on the menu itself. Every real
             control inside is a <button>, so the keyboard already reaches all of them,
             and Escape is handled globally — `shortcuts.js` gives Escape to any mounted
             [role="menu"] rather than clearing the screens. A keydown handler here would
             have to stopPropagation too, which would swallow Space (rule 11: Space means
             advance, app-wide) for as long as a menu is open. -->
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <div class="tg-newmenu" on:click|stopPropagation role="menu" tabindex="-1">
          <div class="tg-newsec r-lbl">Start from</div>
          {#each STARTERS as s}
            <button class="tg-newmi" on:click={() => newFrom(s)}>
              <span class="tg-newname">{s.label}</span>
              <span class="tg-newhint">{s.hint}</span>
            </button>
          {/each}
        </div>
      {/if}
    </span>
  </svelte:fragment>

  <!-- ══ RAIL ══ Kinds, then the look register read from the other direction.
       Dense rows with a hairline between them, which is the whole grammar: a
       list you scan, not a stack of cards you read. -->
  <aside class="rw-pane tg-rail">
    <div class="rw-panehead"><h2 class="rw-panettl">Kinds</h2></div>
    <nav class="rw-panebody" aria-label="Template kinds">
      <!-- Type rows — DERIVED from each template's shape, so a row can never
           claim a template it isn't. Only kinds that actually occur are shown;
           there is no empty "Announcements" row because nothing distinguishes one. -->
      <button class="rw-item r-focus" class:on={filter === 'all'} on:click={() => (filter = 'all')}>
        <span class="rw-itemname">All templates</span>
        <span class="rw-itemn">{$templates.length}</span>
      </button>
      {#each kinds as k (k.key)}
        <button class="rw-item r-focus" class:on={filter === k.key} on:click={() => (filter = k.key)}>
          <span class="rw-itemname">{k.many}</span>
          <span class="rw-itemn">{k.count}</span>
        </button>
      {/each}

      <!-- THE LOOK REGISTER, read the other way round. A card's tag answers
           "what is this template used for"; this answers "what does scripture
           wear", which is the question asked in a booth. READ-ONLY: the one
           writer is `setContentTemplate` (Outputs, and the editor's Used for) —
           a row here only selects the template so you can look at it.
           A kind with nothing bound says so IN WORDS; it never prints a dash,
           because a dash cannot tell "not set" from "we did not ask" (R3-13). -->
      <div class="rw-group">Content looks</div>
      {#each CONTENT_KINDS as ck (ck.key)}
        {@const lookTpl = $templates.find((t) => t.id === $contentTemplates[ck.key]) || null}
        {#if lookTpl}
          <button class="rw-item r-focus tg-look" class:on={lookTpl.id === selId}
            on:click={() => { filter = 'all'; selId = lookTpl.id; }}>
            <span class="rw-itemname">{ck.label}</span>
            <span class="tg-lookv">{lookTpl.name}</span>
          </button>
        {:else}
          <!-- Not a disabled button. There is nothing for this row to select, and
               a control an operator can press and learn nothing from is worse
               than a line of text that states the fact. -->
          <div class="rw-item tg-look tg-static">
            <span class="rw-itemname">{ck.label}</span>
            <span class="tg-lookv unset">Not set</span>
          </div>
        {/if}
      {/each}
    </nav>
  </aside>

  <!-- ══ THE TEMPLATES ══ -->
  <section class="rw-pane">
    <div class="rw-panehead">
      <h2 class="rw-panettl">Templates</h2>
      <span class="rw-spring"></span>
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
        <button class="tg-viewbtn" class:on={view === 'grid'} on:click={() => (view = 'grid')} aria-label="Grid view" title="Grid">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/></svg>
        </button>
        <button class="tg-viewbtn" class:on={view === 'list'} on:click={() => (view = 'list')} aria-label="Row view" title="Rows">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/></svg>
        </button>
      </div>
    </div>

    <div class="rw-panebody" class:pad={view === 'grid'} on:scroll={closeMenu}>
      {#if shown.length}
        <div class="tg-grid" class:list={view === 'list'}>
          {#each shown as t (t.id)}
            <div class="tg-card" class:sel={t.id === selId} class:row={view === 'list'}
              on:click={() => (selId = t.id)} role="button" tabindex="0"
              on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = t.id; } }}>
              <div class="tg-thumb">
                {#if isKeyedTemplate(t)}<CameraPlate />{/if}
                <TemplateRender template={t} content={SAMPLE} />
                {#if view === 'grid'}<span class="tg-aspect r-mono">16:9</span>{/if}
              </div>
              <!-- THE CAPTION — the prototype's `.cell .meta` plus `.roletag`
                   beneath it. ONE row: the name on the left, what the template
                   IS on the right, the card's own controls at the end; then, on
                   its own line, what it is USED FOR.
                   It was four things in one row (name, kind, a `Scripture · 16:9`
                   sub-line, a bound-look pill) and every one but the name was
                   `flex:0 0 auto`, so the name paid the whole shortfall: 225px of
                   claims inside a 194px row rendered "Classic Serif" as `Cla…`.
                   `templatecard.test.js` prices the row from these rules. -->
              <div class="tg-meta">
                <div class="tg-metarow">
                  <!-- THE NAME IS THE CARD'S IDENTITY, so it is the LAST thing
                       allowed to shrink: it holds a floor and the kind beside it
                       ellipses first. The kind is still readable in full in the
                       rail, the inspector and this element's own tooltip. -->
                  <span class="tg-name" title={t.name}>{t.name}</span>
                  <!-- THE ROLE TAG (the prototype's `.kd`). What this template is
                       FOR, derived from its shape — never stored, so it cannot
                       claim a role the template is not. The rail row and this tag
                       come from the one derivation, which is what stops a card
                       saying Scripture under a Lower Thirds filter. It carries
                       the kind ALONE now: the `Scripture · 16:9` sub-line said
                       the same thing a second time, and stated an aspect the
                       thumbnail's own badge already states. -->
                  <span class="tg-role r-mono" title="Content type · {kindLabel(t)}">{kindTag(t)}</span>
                  <div class="tg-cardbtns">
                  <!-- Star = THE default template (the fallback look every slide
                       wears). One default, not a set of four; steel blue when set.
                       Not amber — amber means live on the wall, this only marks a fallback. -->
                  <button class="tg-star" class:on={t.id === $defaultTemplateId}
                    title={t.id === $defaultTemplateId ? 'Default template — click to clear' : 'Make this the default template'}
                    aria-label="Toggle default template" on:click|stopPropagation={() => makeDefault(t)}
                    disabled={!$capture.available}>
                    <svg viewBox="0 0 24 24" width="13" height="13" fill={t.id === $defaultTemplateId ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="m12 3 2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9L12 3Z"/></svg>
                  </button>
                  <button class="tg-more" aria-label="More actions" on:click|stopPropagation={(e) => openMenu(e, t)}>
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="currentColor" aria-hidden="true"><circle cx="12" cy="5" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="12" cy="19" r="1.6"/></svg>
                  </button>
                  </div>
                </div>
                <!-- USED FOR — the prototype's `.roletag`, its own line beneath
                     the row, so "which template does scripture wear" is
                     answerable by looking rather than by opening each one.
                     It says **Used for** in words. The derived ROLE above and the
                     bound LOOK here are two registers (DECISIONS §78); rendered
                     as two identical uppercase pills side by side they read as
                     one fact printed twice, which is exactly what the 1600x1000
                     render showed. Words are the only way to separate them:
                     every colour that carries a promise is already spoken for
                     (CLAUDE.md rule 18), steel blue included. -->
                {#if usedFor(t.id).length}
                  <span class="tg-roletag r-mono">Used for {usedFor(t.id).map((k) => k.label).join(' · ')}</span>
                {/if}
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

    {#if err}<div class="rw-panefoot tg-err" role="alert">{err}</div>{/if}
  </section>

  <!-- ══ INSPECTOR ══ -->
  <aside class="rw-pane rw-insp">
    {#if !sel}
      <div class="rw-panehead"><h2 class="rw-panettl">Template</h2></div>
      <div class="tg-empty r-empty">Pick a template to preview it.</div>
    {:else}
      <div class="rw-panehead">
        <h2 class="rw-panettl">Template</h2>
        <span class="rw-spring"></span>
        <span class="tg-aspect r-mono static">16:9</span>
      </div>

      <div class="rw-panebody pad">
        <div class="tg-preview">
          {#if isKeyedTemplate(sel)}<CameraPlate />{/if}
          <TemplateRender template={sel} content={SAMPLE} />
        </div>
        <div class="tg-selname">{sel.name}</div>

        <div class="tg-previewbtns">
          <button class="r-btn primary sm" on:click={() => dispatch('edit', { id: sel.id })}>
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z"/></svg>
            Edit
          </button>
          <button class="r-btn ghost sm" on:click={() => (previewing = sel)}>
            <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>
            Full screen
          </button>
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
          <!-- ══ USED FOR ══ What this template is FOR, as a control rather than
               a row you can read and not change (docs/REBRAND.md §3.2). It was
               the one fact in this panel an operator could see and had to leave
               the workspace to set — the Usage tab said "Content looks are set
               in Outputs → Content looks", which is a signpost where a control
               belongs.

               ONE WRITER, still: `setContentTemplate`, the same call the
               editor's own Used for makes and the same store the rail's look
               register reads. Steel blue for a ticked kind — it is the thing you
               are working on, not a claim about any screen. -->
          <div class="r-lbl tg-flbl">Used for</div>
          <div class="tg-usedgrid">
            {#each CONTENT_KINDS as k (k.key)}
              {@const mine = $contentTemplates[k.key] === sel.id}
              <button class="tg-usedchip" class:on={mine} aria-pressed={mine}
                on:click={() => toggleUsedFor(k.key)} disabled={!$capture.available}>
                <span class="tg-usedtick" aria-hidden="true">{mine ? '✓' : ''}</span>{k.label}
              </button>
            {/each}
          </div>
          {#if lookErr}<p class="tg-testerr" role="alert">{lookErr}</p>{/if}
          <p class="tg-fhelp">A kind ticked here wears this template on every screen set to <b>Follow the content look</b>. A screen with a look of its own keeps it.</p>

          <!-- ══ THE OBJECTS ON THIS SLIDE ══ §3.2's tab strip, on the surface
               an operator browses from. It names the template's REAL objects
               through the same `layerLabel` the editor's strip uses, and a press
               opens that object in the editor with its properties already
               selected — one action from looking at a template to changing the
               part of it you meant.

               The property groups themselves stayed in the editor, deliberately;
               the reason and its cost are recorded in DECISIONS §80. -->
          <div class="r-lbl tg-flbl">Objects</div>
          {#if selLayers.length}
            <div class="tg-objtabs" role="list">
              {#each selLayers as L (L.id)}
                <button class="tg-objtab" class:off={L.visible === false} role="listitem"
                  title="Edit {layerLabel(L)}"
                  on:click={() => dispatch('edit', { id: sel.id, layerId: L.id })}>{layerLabel(L)}</button>
              {/each}
            </div>
          {:else}
            <!-- IN WORDS, not an empty strip. A template with no objects and one
                 whose objects have not been read look identical, and only one of
                 those is a fact about the template (rule 35). -->
            <p class="tg-fhelp">This is a built-in preset, laid out by region rather than as separate objects. <b>Edit</b> converts it to objects you can move.</p>
          {/if}

          <!-- A row is a NAME and a VALUE (§11), full-bleed against the pane's own
               12px gutter so the seams reach both edges. -->
          <div class="r-lbl tg-flbl">Details</div>
          <div class="tg-rows">
            <div class="rw-nv">
              <span class="rw-nvk">Name</span>
              {#if renaming}
                <!-- svelte-ignore a11y-autofocus -->
                <input class="r-input tg-rename rw-nvctl" bind:value={renameDraft} autofocus
                  aria-label="Template name"
                  on:blur={commitRename} on:keydown={(e) => e.key === 'Enter' && e.target.blur()} />
              {:else}
                <span class="rw-nvv">{sel.name}</span>
              {/if}
            </div>
            <div class="rw-nv"><span class="rw-nvk">Content type</span><span class="rw-nvv">{kindLabel(sel)}</span></div>
            <!-- A READOUT, not a picker: every template is 16:9 by construction
                 (TemplateRender sizes in cqw), so there is no orientation to set. -->
            <div class="rw-nv"><span class="rw-nvk">Orientation</span><span class="rw-nvv">16:9 · 1920×1080</span></div>
            <div class="rw-nv"><span class="rw-nvk">Background</span><span class="rw-nvv">{bgLabel(sel)}</span></div>
            <div class="rw-nv">
              <span class="rw-nvk">Default</span>
              <span class="rw-nvv">{sel.id === $defaultTemplateId ? 'Yes' : 'No'}</span>
            </div>
          </div>
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
            <!-- Two-step, because Tauri's webview has no working confirm() and a
                 delete that reports success without ever showing a dialog is
                 exactly the defect rule 41 exists for. -->
            <button class="r-btn danger sm tg-del" class:arm={delArm === sel.id} on:click={() => del(sel)} disabled={!$capture.available}>
              {delArm === sel.id ? 'Delete — sure?' : 'Delete'}
            </button>
          </div>
          <p class="rw-foot">The <b>default template</b> is the fallback look a slide wears when neither the screen nor the content type has one of its own.</p>
        {:else}
          <div class="r-lbl tg-flbl">Assigned to outputs</div>
          {#if assignedChannels.length}
            <div class="tg-rows">
              {#each assignedChannels as c (c.id)}
                <div class="rw-nv"><span class="rw-nvk">{c.name}</span><span class="rw-nvv">{c.render_target === 'native_window' ? 'display' : c.render_target === 'ndi_encode' ? 'NDI' : 'network'}</span></div>
              {/each}
            </div>
          {:else}
            <p class="tg-fhelp">Not assigned to any screen. Assign it in <b>Outputs → Screens</b>.</p>
          {/if}

          <div class="r-lbl tg-flbl">Default content look</div>
          {#if defaultForKinds.length}
            <div class="tg-rows">
              {#each defaultForKinds as ck (ck.key)}
                <div class="rw-nv"><span class="rw-nvk">{ck.label}</span><span class="rw-nvv">default</span></div>
              {/each}
            </div>
          {:else}
            <p class="tg-fhelp">Not set as a default content look.</p>
          {/if}
          <!-- IT IS NO LONGER ONE PLACE, and saying so would be wrong. This line
               read "Content looks are set in Outputs → Content looks — the one
               place a content type is bound to a template", which was a signpost
               standing where a control belonged. `Used for` on the Details tab
               now binds them here too. Two surfaces, still ONE writer
               (`setContentTemplate`) and ONE store, which is the property that
               actually matters — a second writer is how the matrix and the
               editor came to disagree in the first place (DECISIONS §25). -->
          <p class="rw-foot">A content look is bound on <b>Details → Used for</b>, or in <b>Outputs → Content looks</b>. Both write the same binding, so the two can never disagree.</p>
        {/if}
      </div>
    {/if}
  </aside>
</WorkspaceFrame>

<!-- Fixed-position row menu — anchored to the ⋮ button's screen rect so it is
     never clipped by the card or the scroll area. -->
{#if menuFor && menuTpl}
  <!-- The click handler is not an interaction: it stops the document-level
       outside-click closer from seeing a click on the menu itself. Every real
       control inside is a <button>, so the keyboard already reaches all of them,
       and Escape is handled globally — `shortcuts.js` gives Escape to any mounted
       [role="menu"] rather than clearing the screens. A keydown handler here would
       have to stopPropagation too, which would swallow Space (rule 11: Space means
       advance, app-wide) for as long as a menu is open. -->
  <!-- svelte-ignore a11y-click-events-have-key-events -->
  <div class="tg-menu" style="left:{menuPos.x}px; top:{menuPos.y}px" on:click|stopPropagation role="menu" tabindex="-1">
    <button class="tg-mi" on:click={() => { menuFor = null; dispatch('edit', { id: menuTpl.id }); }}>Edit</button>
    <button class="tg-mi" on:click={() => duplicate(menuTpl)}>Duplicate</button>
    <button class="tg-mi" on:click={() => { const t = menuTpl; menuFor = null; exportTemplate(t); }}>Export</button>
    <button class="tg-mi danger" class:arm={delArm === menuTpl.id} on:click={() => del(menuTpl)}>{delArm === menuTpl.id ? 'Sure?' : 'Delete'}</button>
  </div>
{/if}

<!-- Fullscreen in-console preview (Decision §26). Handles its own Esc. -->
{#if previewing}
  <TemplatePreviewOverlay template={previewing} onClose={() => (previewing = null)} />
{/if}

<style>
  /* The pane, the pane head, the rail row and the name/value row all come from
     `WorkspaceFrame.svelte`. What is left here is what is genuinely this
     workspace's own: a grid of live thumbnails, and the controls around it. */

  /* ── the rail's second column ─────────────────────────────────────────── */
  /* A look register row names a TEMPLATE, not a count, so it takes the width it
     needs rather than `.rw-itemn`'s figure column. */
  .tg-look .rw-itemname{ flex:0 0 auto; color:var(--v-faint); }
  .tg-lookv{ flex:1; min-width:0; text-align:right; font-size:var(--v-fs-cap);
    color:var(--v-faint); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .tg-lookv.unset{ font-style:italic; }
  .tg-static{ cursor:default; }
  .tg-static:hover{ background:transparent; color:var(--v-dim); }

  /* ── the head's controls ──────────────────────────────────────────────── */
  .tg-newwrap{ position:relative; }
  .tg-newmenu{ position:absolute; top:30px; right:0; z-index:40; width:250px; background:var(--v-surf2);
    border:1px solid var(--v-line2); border-radius:var(--v-r-md); box-shadow:var(--v-shadow-lg); padding:4px; }
  .tg-newsec{ padding:6px 8px 4px; }
  /* A MENU ROW, not a button — two lines (a starter's name over its hint) inside
     the New template [role="menu"]. Two lines is the whole reason it is not a
     `.r-btn`: the shared control is a single 26px line of centred label. Named
     rather than reached through `.tg-newmenu button`, so a shape census can tell
     a deliberate menu row from a button that lost its class. */
  .tg-newmi{ display:flex; flex-direction:column; gap:2px; width:100%; text-align:left; padding:8px 9px;
    border:0; background:none; color:var(--v-txt); border-radius:var(--v-r-sm); cursor:pointer; }
  .tg-newmi:hover{ background:var(--v-surf3); }
  .tg-newname{ font-size:var(--v-fs-b2); font-weight:600; }
  .tg-newhint{ font-size:var(--v-fs-cap); color:var(--v-faint); line-height:1.35; }
  /* --v-faint is 3.79:1 on --v-surf3 and nothing is allowed to pair them
     (app.css, RG-74). The hover background IS surf3, so the hint lifts a step
     with it rather than dropping below AA for as long as the pointer is there —
     a contrast failure that only exists on hover is still a contrast failure. */
  .tg-newmi:hover .tg-newhint{ color:var(--v-dim); }

  .tg-search{ display:flex; align-items:center; gap:7px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-sm); padding:0 9px; height:24px; flex:1 1 160px; max-width:260px; }
  .tg-search:focus-within{ border-color:var(--v-sel-line); }
  .tg-search svg{ color:var(--v-faint); flex:0 0 auto; }
  .tg-search input{ flex:1; min-width:0; background:transparent; border:0; outline:none; color:var(--v-txt); font-size:var(--v-fs-b2); }
  .tg-search input::placeholder{ color:var(--v-faint); }
  .tg-sort{ display:flex; align-items:center; gap:7px; flex:0 0 auto; }
  .tg-sort .r-select{ width:auto; }  /* height is the shared control's; was 24px */
  .tg-viewtog{ display:flex; gap:2px; background:var(--v-bg); border:1px solid var(--v-line2);
    border-radius:var(--v-r-sm); padding:2px; flex:0 0 auto; }
  /* A SEGMENTED TOGGLE, not two buttons — grid or rows, one of two, inside one
     trough that carries the edge for the pair. Named (`.tg-viewbtn`) so it is
     legible as a segment rather than as two icon buttons that lost their fill. */
  .tg-viewbtn{ width:26px; height:20px; display:grid; place-items:center; border:0; border-radius:var(--v-r-sm);
    background:none; color:var(--v-faint); cursor:pointer; }
  .tg-viewbtn:hover{ color:var(--v-txt); }
  .tg-viewbtn.on{ background:var(--v-surf3); color:var(--v-txt); }

  /* ── the grid, and the rows it becomes ────────────────────────────────── */
  .tg-grid{ display:grid; grid-template-columns:repeat(auto-fill, minmax(210px, 1fr)); gap:10px; }
  /* ROWS: no card borders, one hairline between — a list you scan down, which is
     what a desk does with thirty of anything. The pane body drops its padding in
     this mode (`pad` is opt-in), so the seams reach both edges. */
  .tg-grid.list{ display:block; }

  .tg-card{ display:flex; flex-direction:column; background:var(--v-surf2); border:1px solid var(--v-line);
    border-radius:var(--v-r-md); overflow:hidden; cursor:pointer;
    transition:border-color var(--v-dur) var(--v-ease); }
  .tg-card:hover{ border-color:var(--v-line2); }
  .tg-card.sel{ border-color:var(--v-sel); box-shadow:0 0 0 1px var(--v-sel); }
  .tg-card.row{ flex-direction:row; align-items:stretch; background:none; border:0; border-radius:0;
    border-bottom:1px solid var(--v-line); box-shadow:none; }
  .tg-card.row:hover{ background:var(--v-surf2); }
  .tg-card.row.sel{ background:var(--v-sel-soft); box-shadow:inset 2px 0 0 var(--v-sel); }

  /* position:relative is load-bearing — TemplateRender's root is
     position:absolute; inset:0 and supplies its own container-type. */
  .tg-thumb{ position:relative; aspect-ratio:16/9; background:var(--v-void); overflow:hidden; flex:0 0 auto; }
  .tg-card.row .tg-thumb{ width:84px; margin:5px 0 5px 8px; border-radius:2px; }
  .tg-aspect{ position:absolute; top:6px; right:6px; font-size:var(--v-fs-cap); letter-spacing:.04em;
    color:var(--v-txt); background:rgba(10,10,10,.62); padding:1px 5px; border-radius:var(--v-r-sm); }
  .tg-aspect.static{ position:static; background:var(--v-surf2); color:var(--v-faint); }

  /* NOT A STATUS. The used-for line, in mono, in the muted step — every colour
     that carries a promise is spoken for, steel blue (selection) included.
     It is a BLOCK on its own line (the prototype's `.roletag`), not a pill
     competing with the name: it used to claim up to 40% of the caption row. */
  .tg-roletag{ display:block; min-width:0; font-size:var(--v-fs-cap); letter-spacing:var(--v-tr-caps);
    text-transform:uppercase; color:var(--v-faint); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* ── USED FOR, and the objects on this slide (§3.2) ────────────────────── */
  /* A ticked kind is steel blue: "the thing you are working on". It is a fact
     about the template, never a claim about a screen, so it borrows neither
     amber (on air) nor amethyst (rehearsal). */
  .tg-usedgrid{ display:flex; flex-wrap:wrap; gap:4px; margin-bottom:8px; }
  /* A CHIP, not a button — `aria-pressed`, a tick and a label, wrapping in a
     grid. Each one states whether this template is what that content kind
     wears; a row of them in the button shape would read as a row of actions. */
  .tg-usedchip{ display:inline-flex; align-items:center; gap:5px; padding:4px 8px;
    border:1px solid var(--v-line2); border-radius:var(--v-r-sm); background:var(--v-surf2);
    color:var(--v-dim); font-family:var(--f-body); font-size:var(--v-fs-cap); cursor:pointer;
    transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .tg-usedchip:hover:not(.on):not(:disabled){ color:var(--v-txt); background:var(--v-surf3); }
  .tg-usedchip.on{ background:var(--v-sel-fill); border-color:transparent; color:var(--v-sel-ink); font-weight:600; }
  .tg-usedchip:disabled{ opacity:.4; cursor:not-allowed; }
  .tg-usedtick{ width:8px; display:inline-block; text-align:center; }

  /* THE OBJECT STRIP WRAPS, never scrolls — a tab that has gone behind a hidden
     scrollbar is a tab nobody knows is there. Same rule as the editor's strip. */
  .tg-objtabs{ display:flex; flex-wrap:wrap; gap:3px; margin-bottom:8px; }
  /* A TAB, not a button. Same shape as the editor's `.te-objtab` and the same
     job — it names one object in the template and a press opens that object.
     Sized by its label, wrapping rather than scrolling. */
  .tg-objtab{ padding:3px 8px; border:1px solid var(--v-line2); border-radius:var(--v-r-sm);
    background:var(--v-surf2); color:var(--v-dim); font-family:var(--f-body);
    font-size:var(--v-fs-cap); cursor:pointer; }
  .tg-objtab:hover{ color:var(--v-txt); background:var(--v-surf3); }
  /* A hidden object is struck through rather than dropped: an object that is not
     drawn is still an object, and one that has vanished from the strip is one an
     operator cannot switch back on. */
  .tg-objtab.off{ text-decoration:line-through; opacity:.6; }

  /* THE ROLE TAG — what this template is FOR. Same neutral treatment as
     `.tg-roletag` above and for the same reason: it is a fact about the
     template, never a claim about a screen, so it borrows no promised colour.
     It is `flex:0 1 auto` with `min-width:0` on purpose: under pressure the KIND
     ellipses and the name does not. It claimed `flex:0 0 auto; max-width:42%`,
     which is half of how the name came to render as three characters. */
  .tg-role{ flex:0 1 auto; min-width:0; font-size:var(--v-fs-cap); letter-spacing:var(--v-tr-caps);
    text-transform:uppercase; color:var(--v-faint); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }

  /* THE CAPTION is a column: the name's row, then the used-for line. */
  .tg-meta{ display:flex; flex-direction:column; justify-content:center; gap:2px;
    padding:6px 8px; flex:1; min-width:0; }
  .tg-card.row .tg-meta{ padding:0 12px 0 8px; }
  .tg-metarow{ display:flex; align-items:center; gap:7px; min-width:0; }
  /* THE FLOOR, and the whole point of it: 96px is wider than the longest name a
     fresh install ships ("Worship Lyrics"), so no seeded built-in is ever
     abbreviated. `templatecard.test.js` prices the row against the grid's own
     narrowest column and fails if the row cannot honour this. */
  .tg-name{ flex:1 1 auto; min-width:96px; font-size:var(--v-fs-b2); font-weight:600; color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .tg-cardbtns{ display:flex; align-items:center; gap:1px; flex:0 0 auto; position:relative; }
  /* ICON-ONLY CARD AFFORDANCES, not buttons — a star that marks the default
     template and a kebab that opens the row menu, both 22px in the card's own
     title row. A fill and an edge on either would put two chrome boxes on top
     of a thumbnail whose whole job is to show a template. */
  .tg-star, .tg-more{ width:22px; height:22px; display:grid; place-items:center; border:0; background:none;
    color:var(--v-faint); cursor:pointer; border-radius:var(--v-r-sm); }
  .tg-star.on{ color:var(--v-sel); }
  .tg-star:disabled{ opacity:.3; cursor:not-allowed; }
  .tg-more:hover{ color:var(--v-txt); background:var(--v-surf3); }

  .tg-menu{ position:fixed; z-index:200; width:150px; display:flex; flex-direction:column;
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:var(--v-r-md);
    box-shadow:var(--v-shadow-lg); padding:4px; }
  /* A MENU ROW, not a button — the ⋮ row menu's items. Full-bleed, left-aligned,
     no edge, because the floating menu is the surface they sit on. Named so the
     shape census reads them as menu rows rather than as unstyled buttons. */
  .tg-mi{ text-align:left; padding:6px 9px; border:0; background:none; color:var(--v-txt);
    font-size:var(--v-fs-b2); border-radius:var(--v-r-sm); cursor:pointer; }
  .tg-mi:hover{ background:var(--v-surf3); }
  .tg-mi.danger{ color:var(--v-rose); }
  .tg-mi.danger.arm{ background:var(--v-rose-soft); }

  /* The error sits in the pane's own foot, behind the same hairline every other
     footnote uses, rather than floating as a bordered card of its own. */
  .tg-err{ color:var(--v-rose); font-size:var(--v-fs-cap); line-height:1.45; }

  /* ── inspector ────────────────────────────────────────────────────────── */
  .tg-preview{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md); border:1px solid var(--v-line2);
    overflow:hidden; background:var(--v-void); }
  .tg-selname{ margin:8px 0 0; font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600;
    color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .tg-previewbtns{ display:flex; flex-wrap:wrap; gap:5px; margin-top:8px; }
  .tg-previewbtns .r-btn{ flex:1 1 auto; justify-content:center; }
  .tg-testerr{ margin:8px 0 0; padding:8px 10px; border:1px solid var(--v-rose); border-radius:var(--v-r-sm);
    background:var(--v-rose-soft); color:var(--v-rose); font-size:var(--v-fs-cap); line-height:1.45; }
  .tg-insptabs{ margin:12px 0 0; width:100%; }
  .tg-insptabs :global(button){ flex:1; }

  /* Full-bleed rows against the pane body's own 12px gutter, so a seam reaches
     the pane edge while the prose around it keeps the gutter. The border box
     lands exactly on the padding edge — no horizontal overflow. */
  .tg-rows{ display:flex; flex-direction:column; margin:10px -12px 0;
    border-top:1px solid var(--v-line); }
  .tg-rename{ height:24px; padding:2px 7px; max-width:150px; }

  .tg-flbl{ margin:14px 0 6px; }
  .tg-fhelp{ margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .tg-fhelp b{ color:var(--v-dim); }
  .tg-actions{ display:flex; flex-wrap:wrap; gap:5px; }
  .tg-actions .r-btn{ flex:1 1 auto; justify-content:center; }
  /* CONVERTED — B2. It was `.r-btn ghost sm` repainted rose: a ghost's `--v-500`
     hairline with red text inside it. The editor's own Delete, one press away in
     the same workspace, is `.r-btn sm danger` and draws a RED edge — so the same
     word wore two shapes depending on which surface you deleted from. `danger`
     is the variant that exists for this; what is left is the ARMED half, which
     is a state the variant has no opinion about (rule 41's two-step). */
  .tg-del.arm{ background:var(--v-red-soft); border-color:var(--v-red); }

  .tg-empty{ margin:auto; padding:24px; text-align:center; }
</style>
