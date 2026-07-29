<script>
  import { onMount } from 'svelte';
  import {
    capture,
    templates,
    loadTemplates,
    saveTemplate,
    createTemplate,
    deleteTemplate,
    setTemplateActive,
  } from '../stores/capture.js';
  import TemplateRender from '../TemplateRender.svelte';

  // WYSIWYG: the preview is the SAME renderer (TemplateRender) as the live
  // output. Users keep many templates but activate ≤4 for the console Output.
  let edit = null;
  let saving = false;
  let savedId = null;
  let err = '';

  $: activeCount = $templates.filter((t) => t.active).length;

  onMount(async () => {
    const list = await loadTemplates();
    if (list.length) select(list[0]);
  });

  function select(t) {
    edit = structuredClone(t);
    edit.layout ??= { regions: [], align: 'center' };
    edit.layout.regions ??= [];
    edit.style ??= {};
    savedId = null;
    err = '';
  }

  const SAMPLE = {
    text: 'The LORD is my shepherd; I shall not want. He maketh me to lie down in green pastures: he leadeth me beside the still waters.',
    reference: 'Psalms 23:1-2 · KJV',
  };
  const REGIONS = [
    { key: 'verse_text', label: 'Verse text' },
    { key: 'reference', label: 'Reference' },
  ];

  // Font list: curated system + bundled families; extendable with the real
  // installed fonts via the Local Font Access API on a user gesture.
  let fonts = [
    'Fraunces', 'Playfair Display', 'Space Grotesk', 'Inter', 'JetBrains Mono',
    'Georgia', 'Times New Roman', 'Palatino', 'Baskerville', 'Garamond',
    'Helvetica Neue', 'Arial', 'Futura', 'Gill Sans', 'Optima', 'Didot',
    'Menlo', 'Courier New', 'Verdana', 'Trebuchet MS', 'Cambria',
  ];
  let fontMsg = '';
  async function detectFonts() {
    if (!window.queryLocalFonts) { fontMsg = 'not available here'; return; }
    try {
      const avail = await window.queryLocalFonts();
      const fams = [...new Set(avail.map((f) => f.family))].sort();
      if (fams.length) { fonts = [...new Set([...fonts, ...fams])]; fontMsg = `${fams.length} installed`; }
    } catch { fontMsg = 'access denied'; }
  }

  function toggleRegion(k) {
    const set = new Set(edit.layout.regions);
    set.has(k) ? set.delete(k) : set.add(k);
    edit.layout.regions = [...set];
    edit = edit;
  }
  function setStyle(k, v) { edit.style[k] = v; edit = edit; }
  function setLayout(k, v) { edit.layout[k] = v; edit = edit; }

  function onBgColor(e) { edit.style.background = e.target.value; edit.style.bgImage = null; edit = edit; }
  function setTransparent() { edit.style.background = 'transparent'; edit.style.bgImage = null; edit = edit; }
  function onBgImage(e) {
    const file = e.target.files?.[0];
    if (!file) return;
    const r = new FileReader();
    r.onload = () => { edit.style.bgImage = r.result; edit = edit; };
    r.readAsDataURL(file);
    e.target.value = '';
  }
  function clearImage() { edit.style.bgImage = null; edit = edit; }

  async function save() {
    saving = true; err = '';
    try {
      const id = await saveTemplate(edit);
      savedId = id;
      const fresh = $templates.find((t) => t.id === id);
      if (fresh) edit = structuredClone(fresh);
    } catch (e) { err = 'Save failed: ' + e; }
    saving = false;
  }

  async function newTemplate() {
    err = '';
    try {
      const id = await createTemplate('New template');
      const fresh = $templates.find((t) => t.id === id);
      if (fresh) select(fresh);
    } catch (e) { err = String(e); }
  }
  // Two-step delete (no native confirm — Tauri's webview doesn't implement it).
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
      await deleteTemplate(t.id);
      if (edit && edit.id === t.id) {
        const first = $templates[0];
        first ? select(first) : (edit = null);
      }
    } catch (e) { err = String(e); }
  }
  async function toggleActive(t) {
    err = '';
    try { await setTemplateActive(t.id, !t.active); }
    catch (e) { err = String(e).replace('Error: ', ''); }
  }

  const isColor = (v) => typeof v === 'string' && v.startsWith('#');

  // ── editor chrome (VIEW state only — none of this is saved on the template) ──
  //
  // The design reference draws a canvas toolbar with a zoom readout, an aspect
  // selector, a Preview button and a safe-area guide. Zoom, the guide and Preview
  // are genuinely view-only, so they live here and touch nothing that renders on a
  // wall. The aspect selector is NOT a control: TemplateRender is 16:9 by
  // construction (sizes are cqw so a template scales identically at any output
  // size), so it is shown as a readout rather than a dropdown that would lie.
  const ZOOMS = [40, 55, 70, 85, 100];
  let zoomIdx = ZOOMS.length - 1;
  $: zoom = ZOOMS[zoomIdx];
  const zoomOut = () => (zoomIdx = Math.max(0, zoomIdx - 1));
  const zoomIn = () => (zoomIdx = Math.min(ZOOMS.length - 1, zoomIdx + 1));

  // "Preview" hides the editor's own chrome — the safe-area guide and the
  // transparency checkerboard — so what is left is exactly what leaves the machine.
  let previewMode = false;
  let safeArea = true;

  // Is this template keyed out for OBS/ATEM? A real property of the template, not
  // editor state — it is what `background: transparent` means.
  $: transparentBg = edit?.style?.background === 'transparent';
  function toggleTransparent() {
    if (transparentBg) setStyle('background', '#0b0906');
    else setTransparent();
  }

  // The reference's "Elements" rail. Each row is a REAL toggle on the template —
  // nothing here is decorative. `Translation` and `Logo` appear in the reference but
  // Relay's template model has no such regions, so they are not drawn (see log).
  $: elements = edit
    ? [
        { key: 'bg',        label: 'Background',      on: !transparentBg,                            toggle: toggleTransparent },
        { key: 'reference', label: 'Verse Reference', on: edit.layout.regions.includes('reference'),  toggle: () => toggleRegion('reference') },
        { key: 'verse',     label: 'Verse Text',      on: edit.layout.regions.includes('verse_text'), toggle: () => toggleRegion('verse_text') },
        { key: 'bar',       label: 'Reference Bar',   on: !!edit.layout.lowerThird,                   toggle: () => setLayout('lowerThird', !edit.layout.lowerThird) },
        { key: 'safe',      label: 'Safe Area Guide', on: safeArea && !previewMode,                   toggle: () => (safeArea = !safeArea) },
      ]
    : [];
</script>


<!-- TEMPLATES — OUTPUT DESIGNER, laid out to
     docs/relaydesign/relay-templetedesigner-screen.png:
       Elements rail · canvas toolbar + stage · Text/Background inspector · bottom bar.
     Every control is one that already existed; the reference's grammar (label-left /
     control-right rows, sectioned inspector, element visibility rail) is what changed.
     Controls the reference draws but Relay has no model for — Translation and Logo
     regions, per-element Weight, background Opacity, undo/redo, an aspect-ratio
     selector, a Content Type picker — are NOT drawn. See the loop log. -->
<div class="tpl-editor">
  <!-- ══ ELEMENTS ══ Every row toggles a real property of the template. -->
  <aside class="pane rail">
    <div class="pane-head">
      <h2>Elements</h2>
      <span class="r-mono cnt">{activeCount}/4 active</span>
    </div>
    <div class="rail-body">
      {#if edit}
        {#each elements as el (el.key)}
          <button class="el" class:off={!el.on} on:click={el.toggle}>
            <span class="el-ic" aria-hidden="true">
              {#if el.key === 'bg'}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m3 16 5-5 4 4 3-3 6 6"/><circle cx="9" cy="9" r="1.5"/></svg>
              {:else if el.key === 'bar'}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 15h18M7 18.5h6"/></svg>
              {:else if el.key === 'safe'}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/></svg>
              {:else}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M5 6V4h14v2M12 4v16M9 20h6"/></svg>
              {/if}
            </span>
            <span class="el-name">{el.label}</span>
            <!-- The eye is the row's state, not a second control — the whole row is
                 the button, so there is only ever one thing to hit. -->
            <span class="el-eye" aria-hidden="true">
              {#if el.on}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M2 12s3.6-6.5 10-6.5S22 12 22 12s-3.6 6.5-10 6.5S2 12 2 12Z"/><circle cx="12" cy="12" r="2.6"/></svg>
              {:else}
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 3l18 18M10.6 6.1A9.6 9.6 0 0 1 12 6c6.4 0 10 6 10 6a17 17 0 0 1-3.3 4M6.3 8.3A17 17 0 0 0 2 12s3.6 6.5 10 6.5a9.9 9.9 0 0 0 3.6-.65"/></svg>
              {/if}
            </span>
          </button>
        {/each}
      {/if}

      <div class="rail-sep"></div>
      <div class="rail-head">
        <span class="klbl">Saved templates</span>
        <button class="r-iconbtn sm" title="New template" aria-label="New template"
          on:click={newTemplate} disabled={!$capture.available}>
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
        </button>
      </div>
      {#each $templates as t (t.id)}
        <div class="tpl-row" class:sel={edit && t.id === edit.id}>
          <!-- Amber star = this template is one of the ≤4 the console can put on a
               wall. Amber is the on-air colour and this is the closest thing the
               editor has to it: it is the list of templates that can go out. -->
          <button class="star" class:on={t.active} title={t.active ? 'Active on console' : 'Activate on console'}
            aria-label="Toggle active" on:click|stopPropagation={() => toggleActive(t)}
            disabled={!$capture.available || (!t.active && activeCount >= 4)}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill={t.active ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="m12 3 2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9L12 3Z"/></svg>
          </button>
          <button class="tpl-name" on:click={() => select(t)}>{t.name}</button>
          <!-- Two-step delete. No native confirm() — Tauri's webview does not
               reliably implement it, so the arm/confirm lives in the button. -->
          <button class="del" class:arm={delArm === t.id} title={delArm === t.id ? 'Click again to confirm' : 'Delete'}
            aria-label="Delete template" on:click|stopPropagation={() => del(t)} disabled={!$capture.available}>
            {#if delArm === t.id}
              <span class="delconf">Sure?</span>
            {:else}
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13"/></svg>
            {/if}
          </button>
        </div>
      {/each}
      {#if !$capture.available}<div class="r-empty rail-empty">backend not attached</div>{/if}
    </div>
  </aside>

  {#if edit}
    <!-- ══ CANVAS ══ -->
    <section class="canvas">
      <header class="toolbar">
        <div class="tb-zoom">
          <button class="tbtn" on:click={zoomOut} disabled={zoomIdx === 0} title="Zoom out" aria-label="Zoom out">
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3M8 11h6"/></svg>
          </button>
          <button class="tbtn" on:click={zoomIn} disabled={zoomIdx === ZOOMS.length - 1} title="Zoom in" aria-label="Zoom in">
            <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><circle cx="11" cy="11" r="7"/><path d="m21 21-4.3-4.3M8 11h6M11 8v6"/></svg>
          </button>
          <span class="tb-pct r-mono">{zoom}%</span>
        </div>

        <!-- A READOUT, not a picker. TemplateRender sizes everything in cqw, so a
             template renders identically at any output size — there is nothing here
             to choose, and a dropdown would imply otherwise. -->
        <span class="tb-aspect r-mono" title="Templates scale to any output size — sizes are container-relative">
          16:9 · 1920×1080
        </span>

        <span class="spring"></span>

        <button class="r-btn ghost sm" class:on={previewMode} on:click={() => (previewMode = !previewMode)}>
          {previewMode ? 'Editing' : 'Preview'}
        </button>
        <!-- CONFIRM, not amber. The reference paints Save violet, but amethyst means
             REHEARSAL in this app and amber means ON AIR — neither is true of saving
             a template. The design sheet's own CONFIRM ACTION colour is green. -->
        <button class="r-btn confirm sm" on:click={save} disabled={saving || !$capture.available}>
          {saving ? 'Saving…' : 'Save'}
        </button>
      </header>

      <div class="stage-body">
        <!-- WYSIWYG by construction: this is the SAME TemplateRender the real output
             window and the kiosk page use. Two drawings of "what the congregation
             sees" is exactly the thing you cannot afford two versions of. -->
        <div class="wysiwyg" style="width:{zoom}%">
          {#if !previewMode}<div class="checker"></div>{/if}
          <TemplateRender template={edit} content={SAMPLE} />
          {#if safeArea && !previewMode}<div class="safe"></div>{/if}
          {#if savedId}<span class="saved r-mono">Saved · live on the console</span>{/if}
        </div>
      </div>

      <footer class="botbar">
        <button class="sw-inline" on:click={toggleTransparent}>
          <span class="r-switch green" class:on={transparentBg}></span>
          <span>Transparent background</span>
        </button>
        <span class="bot-note">Keys out in OBS / ATEM</span>
        <span class="spring"></span>
        <span class="klbl nowrap">Content type</span>
        <!-- Scripture is the only content type this editor's sample renders. Relay
             assigns templates PER content type elsewhere (Settings), so this is a
             readout of what is on the canvas, not a second place to set it. -->
        <span class="bot-chip r-mono">Scripture</span>
      </footer>
    </section>

    <!-- ══ INSPECTOR ══ label-left / control-right, in the reference's sections. -->
    <aside class="pane insp">
      <div class="insp-body r-scroll">
        <h3 class="sec">Template</h3>
        <div class="frow">
          <label class="fk" for="tpl-name">Name</label>
          <input id="tpl-name" class="r-input fv" bind:value={edit.name} />
        </div>

        <h3 class="sec">Text</h3>
        <div class="frow">
          <label class="fk" for="tpl-font">Font</label>
          <select id="tpl-font" class="r-select fv" bind:value={edit.style.font}>
            {#if edit.style.font && !fonts.includes(edit.style.font)}<option value={edit.style.font}>{edit.style.font}</option>{/if}
            {#each fonts as f}<option value={f}>{f}</option>{/each}
          </select>
        </div>
        <button class="mini-link" on:click={detectFonts}>Detect installed fonts {fontMsg}</button>
        <div class="frow">
          <label class="fk" for="tpl-vsize">Verse size</label>
          <span class="fv stepper">
            <input id="tpl-vsize" class="num r-mono" type="number" min="2" max="12" step="0.1"
              bind:value={edit.style.verseSize} />
            <span class="unit r-mono">cqw</span>
          </span>
        </div>
        <div class="frow">
          <label class="fk" for="tpl-rsize">Ref size</label>
          <span class="fv stepper">
            <input id="tpl-rsize" class="num r-mono" type="number" min="1" max="6" step="0.1"
              bind:value={edit.style.refSize} />
            <span class="unit r-mono">cqw</span>
          </span>
        </div>
        <div class="frow">
          <label class="fk" for="tpl-vcol">Text colour</label>
          <span class="fv swatch">
            <input id="tpl-vcol" type="color" value={isColor(edit.style.verseColor) ? edit.style.verseColor : '#f4e4c8'}
              on:input={(e) => setStyle('verseColor', e.target.value)} />
            <span class="hex r-mono">{isColor(edit.style.verseColor) ? edit.style.verseColor.toUpperCase() : '#F4E4C8'}</span>
          </span>
        </div>
        <div class="frow">
          <label class="fk" for="tpl-acol">Accent</label>
          <span class="fv swatch">
            <input id="tpl-acol" type="color" value={isColor(edit.style.accent) ? edit.style.accent : '#ffb000'}
              on:input={(e) => setStyle('accent', e.target.value)} />
            <span class="hex r-mono">{isColor(edit.style.accent) ? edit.style.accent.toUpperCase() : '#FFB000'}</span>
          </span>
        </div>
        <div class="frow">
          <span class="fk">Verse align</span>
          <span class="fv seg">
            {#each ['left', 'center', 'right'] as a}
              <button class:on={(edit.style.verseAlign || edit.layout.align || 'center') === a}
                title="Align {a}" aria-label="Align verse {a}" on:click={() => setStyle('verseAlign', a)}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                  <path d="M4 6h16"/>
                  <path d={a === 'left' ? 'M4 12h10' : a === 'right' ? 'M10 12h10' : 'M7 12h10'}/>
                  <path d="M4 18h16"/>
                </svg>
              </button>
            {/each}
          </span>
        </div>
        <div class="frow">
          <span class="fk">Ref align</span>
          <span class="fv seg">
            {#each ['left', 'center', 'right'] as a}
              <button class:on={(edit.style.refAlign || edit.layout.align || 'center') === a}
                title="Align {a}" aria-label="Align reference {a}" on:click={() => setStyle('refAlign', a)}>
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                  <path d="M4 6h16"/>
                  <path d={a === 'left' ? 'M4 12h10' : a === 'right' ? 'M10 12h10' : 'M7 12h10'}/>
                  <path d="M4 18h16"/>
                </svg>
              </button>
            {/each}
          </span>
        </div>
        <button class="sw-row" on:click={() => setLayout('refFirst', !edit.layout.refFirst)}>
          <span>Reference above verse</span><span class="r-switch" class:on={edit.layout.refFirst}></span>
        </button>
        <button class="sw-row" on:click={() => setStyle('italicRef', !edit.style.italicRef)}>
          <span>Italic reference</span><span class="r-switch" class:on={edit.style.italicRef}></span>
        </button>

        <h3 class="sec">Background</h3>
        <div class="frow">
          <label class="fk" for="tpl-fill">Fill</label>
          <span class="fv swatch">
            <input id="tpl-fill" type="color" value={isColor(edit.style.background) ? edit.style.background : '#0b0906'}
              on:input={onBgColor} disabled={transparentBg} />
            <span class="hex r-mono">{transparentBg ? 'transparent' : isColor(edit.style.background) ? edit.style.background.toUpperCase() : '#0B0906'}</span>
          </span>
        </div>
        <div class="frow">
          <span class="fk">Image</span>
          <span class="fv bg-row">
            <label class="r-btn ghost sm file">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m3 16 5-5 4 4 3-3 6 6"/><circle cx="9" cy="9" r="1.6"/></svg>
              Choose<input type="file" accept="image/*" on:change={onBgImage} hidden />
            </label>
            {#if edit.style.bgImage}
              <button class="r-btn danger sm" on:click={clearImage}>Clear</button>
            {/if}
          </span>
        </div>
        {#if edit.style.bgImage}
          <div class="bg-thumb" style="background-image:url({edit.style.bgImage});"></div>
        {/if}

        <h3 class="sec">Motion</h3>
        <div class="frow">
          <label class="fk" for="tpl-trans">Transition</label>
          <span class="fv stepper">
            <input id="tpl-trans" class="num r-mono" type="number" min="0" max="800" step="50"
              value={edit.style.transitionMs ?? 250} on:input={(e) => setStyle('transitionMs', +e.target.value)} />
            <span class="unit r-mono">ms</span>
          </span>
        </div>
        <p class="fnote">{(edit.style.transitionMs ?? 250) === 0 ? 'Cut — the change is instant.' : 'Dissolve.'}</p>
      </div>

      {#if err}<div class="err" role="alert">{err}</div>{/if}
    </aside>
  {/if}
</div>

<style>
  /* TEMPLATES — OUTPUT DESIGNER. Built to the design reference's grammar and styled
     only from --v-* tokens: no raw hex, no off-scale spacing. */
  .tpl-editor{display:grid;grid-template-columns:252px minmax(0,1fr) 316px;
    gap:var(--v-sp-sm);height:calc(100dvh - 150px)}
  .spring{flex:1}
  .cnt{font-size:var(--v-fs-cap);color:var(--v-faint)}

  .pane{display:flex;flex-direction:column;min-height:0;overflow:hidden;
    background:var(--v-surf);border:1px solid var(--v-line);border-radius:var(--v-r-lg);
    box-shadow:var(--v-shadow-sm)}
  .pane-head{display:flex;align-items:center;gap:var(--v-sp-sm);padding:12px 14px;
    border-bottom:1px solid var(--v-line);flex:0 0 auto}
  .pane-head h2{margin:0;flex:1;min-width:0;font-family:var(--f-head);
    font-size:var(--v-fs-h3);line-height:var(--v-lh-h3);font-weight:600;color:var(--v-txt)}
  .klbl{font-family:var(--f-mono);font-size:9px;font-weight:700;letter-spacing:.14em;
    text-transform:uppercase;color:var(--v-faint)}

  /* ── elements rail ─────────────────────────────────────────────────────── */
  .rail-body{flex:1;min-height:0;overflow-y:auto;padding:10px;display:flex;
    flex-direction:column;gap:6px;scrollbar-width:thin;scrollbar-color:var(--v-surf3) transparent}
  .rail-body::-webkit-scrollbar{width:6px}
  .rail-body::-webkit-scrollbar-thumb{background:var(--v-surf3);border-radius:99px}
  .el{display:flex;align-items:center;gap:10px;width:100%;text-align:left;cursor:pointer;
    padding:11px 12px;border-radius:var(--v-r-md);background:var(--v-surf2);
    border:1px solid var(--v-line);color:var(--v-txt);font-family:var(--f-body);
    font-size:var(--v-fs-b2);transition:.14s}
  .el:hover{background:var(--v-surf3);border-color:var(--v-line2)}
  /* A hidden element is dimmed, never removed — the operator has to be able to see
     what they have switched off. */
  .el.off{color:var(--v-faint);background:var(--v-bg)}
  .el-ic{flex:0 0 auto;color:var(--v-dim)}
  .el.off .el-ic{color:var(--v-disabled)}
  .el-name{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .el-eye{flex:0 0 auto;color:var(--v-faint)}
  .el.off .el-eye{color:var(--v-disabled)}

  .rail-sep{height:1px;background:var(--v-line);margin:6px 0 2px}
  .rail-head{display:flex;align-items:center;justify-content:space-between;gap:8px;padding:2px 2px 4px}
  .rail-empty{padding:8px 4px}
  .tpl-row{position:relative;display:flex;align-items:center;gap:6px;
    border-radius:var(--v-r-md);padding:2px 2px 2px 4px;transition:background .12s}
  .tpl-row.sel{background:var(--v-surf2)}
  .tpl-row.sel::before{content:"";position:absolute;left:0;top:7px;bottom:7px;width:3px;
    border-radius:0 3px 3px 0;background:var(--v-accent)}
  .tpl-row:hover:not(.sel){background:var(--v-surf3)}
  .star{width:26px;height:26px;flex:0 0 auto;display:grid;place-items:center;border:0;
    background:none;cursor:pointer;color:var(--v-faint);border-radius:var(--v-r-sm)}
  .star.on{color:var(--v-accent)}
  .star:disabled{opacity:.35;cursor:not-allowed}
  .tpl-name{flex:1;min-width:0;text-align:left;background:none;border:0;color:var(--v-txt);
    font-family:var(--f-body);font-size:var(--v-fs-b2);cursor:pointer;padding:7px 2px;
    white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .del{width:24px;height:24px;flex:0 0 auto;display:grid;place-items:center;border:0;
    background:none;cursor:pointer;color:var(--v-faint);border-radius:var(--v-r-sm)}
  .del:hover{color:var(--v-red)}
  .del.arm{width:auto;padding:0 8px;color:var(--v-red);background:var(--v-red-soft)}
  .delconf{font-family:var(--f-mono);font-size:9px;font-weight:700;letter-spacing:.04em}

  /* ── canvas ────────────────────────────────────────────────────────────── */
  .canvas{display:flex;flex-direction:column;min-height:0;overflow:hidden;
    background:var(--v-surf);border:1px solid var(--v-line);border-radius:var(--v-r-lg);
    box-shadow:var(--v-shadow-sm)}
  .toolbar{flex:0 0 auto;display:flex;align-items:center;gap:var(--v-sp-sm);
    padding:10px 12px;border-bottom:1px solid var(--v-line)}
  .tb-zoom{display:flex;align-items:center;gap:6px}
  .tbtn{width:30px;height:30px;border-radius:var(--v-r-md);display:grid;place-items:center;
    cursor:pointer;background:var(--v-surf2);border:1px solid var(--v-line2);
    color:var(--v-dim);transition:.14s}
  .tbtn:hover:not(:disabled){background:var(--v-surf3);color:var(--v-txt)}
  .tbtn:disabled{opacity:.4;cursor:not-allowed}
  .tb-pct{min-width:44px;text-align:center;font-size:var(--v-fs-cap);color:var(--v-dim)}
  .tb-aspect{padding:6px 11px;border-radius:var(--v-r-md);background:var(--v-surf2);
    border:1px solid var(--v-line);font-size:var(--v-fs-cap);color:var(--v-faint)}
  .r-btn.confirm{background:var(--v-emerald);color:var(--v-void);border-color:transparent}
  .r-btn.confirm:hover:not(:disabled){filter:brightness(1.08)}
  .r-btn.ghost.on{background:var(--v-surf3);color:var(--v-txt);border-color:var(--v-line2)}

  .stage-body{flex:1;min-height:0;display:flex;align-items:center;justify-content:center;
    padding:var(--v-sp-lg);overflow:auto;position:relative;background:var(--v-void)}
  /* Graph-paper field behind the artboard, as in the reference. Decorative only —
     it never reaches an output. */
  .stage-body::before{content:"";position:absolute;inset:0;pointer-events:none;
    background-image:linear-gradient(var(--v-line) 1px,transparent 1px),
      linear-gradient(90deg,var(--v-line) 1px,transparent 1px);
    background-size:32px 32px;
    -webkit-mask-image:radial-gradient(120% 110% at 50% 40%,#000 35%,transparent 100%);
    mask-image:radial-gradient(120% 110% at 50% 40%,#000 35%,transparent 100%)}
  .wysiwyg{z-index:1}
  .wysiwyg{position:relative;aspect-ratio:16/9;max-width:100%;border-radius:var(--v-r-lg);
    overflow:hidden;border:1px solid var(--v-line2);box-shadow:var(--v-shadow-lg);flex:0 0 auto}
  .checker{position:absolute;inset:0;
    background:repeating-conic-gradient(var(--v-surf2) 0% 25%,var(--v-surf) 0% 50%) 50% / 22px 22px}
  /* Title/action-safe guide. Editor chrome only — it is never rendered by the
     output window, and `Preview` removes it so what is left is exactly what ships. */
  .safe{position:absolute;inset:5%;border:1px dashed var(--v-line2);pointer-events:none;
    border-radius:2px}
  .saved{position:absolute;left:10px;bottom:10px;padding:4px 9px;border-radius:var(--v-r-sm);
    background:var(--v-emerald-soft);border:1px solid rgba(34,197,94,.32);
    color:var(--v-emerald);font-size:9px;letter-spacing:.06em;text-transform:uppercase}

  .botbar{flex:0 0 auto;display:flex;align-items:center;gap:var(--v-sp-sm);
    padding:10px 12px;border-top:1px solid var(--v-line)}
  .sw-inline{display:flex;align-items:center;gap:10px;background:none;border:0;cursor:pointer;
    color:var(--v-txt);font-family:var(--f-body);font-size:var(--v-fs-b2);padding:0;
    flex:0 0 auto;white-space:nowrap}
  .nowrap{white-space:nowrap;flex:0 0 auto}
  /* Green = confirmed/on, per the design sheet's toggle. Not amber: nothing about a
     template's background is on air. */
  .r-switch.green.on{background:var(--v-emerald);border-color:transparent}
  .r-switch.green.on::after{background:var(--v-void)}
  .bot-note{font-size:var(--v-fs-cap);color:var(--v-faint);min-width:0;overflow:hidden;
    text-overflow:ellipsis;white-space:nowrap}
  .bot-chip{padding:6px 11px;border-radius:var(--v-r-md);background:var(--v-surf2);
    border:1px solid var(--v-line2);font-size:var(--v-fs-cap);color:var(--v-dim)}

  /* ── inspector ─────────────────────────────────────────────────────────── */
  .insp{min-height:0}
  .insp-body{flex:1;min-height:0;overflow-y:auto;padding:16px;display:flex;
    flex-direction:column;gap:11px}
  .sec{margin:10px 0 3px;font-family:var(--f-head);font-size:var(--v-fs-h2);
    line-height:var(--v-lh-h2);font-weight:600;letter-spacing:var(--v-tr-h2);color:var(--v-txt)}
  .sec:first-child{margin-top:0}
  /* label left, control right — the reference's inspector row. */
  .frow{display:grid;grid-template-columns:74px minmax(0,1fr);align-items:center;gap:10px}
  .fk{font-size:var(--v-fs-b2);color:var(--v-dim)}
  .fv{min-width:0}
  .fnote{margin:0;font-size:var(--v-fs-cap);color:var(--v-faint)}
  .mini-link{background:none;border:0;padding:0;text-align:left;color:var(--v-cyan);
    font-family:var(--f-mono);font-size:9px;cursor:pointer;letter-spacing:.04em}

  .stepper{display:flex;align-items:center;gap:0}
  .num{flex:1;min-width:0;height:34px;padding:0 10px;border-radius:var(--v-r-md) 0 0 var(--v-r-md);
    background:var(--v-bg);border:1px solid var(--v-line2);border-right:0;color:var(--v-txt);
    font-size:var(--v-fs-b2);outline:none}
  .num:focus{border-color:var(--v-accent-line)}
  .unit{flex:0 0 auto;height:34px;display:grid;place-items:center;padding:0 9px;
    border-radius:0 var(--v-r-md) var(--v-r-md) 0;background:var(--v-surf2);
    border:1px solid var(--v-line2);font-size:9px;color:var(--v-faint)}

  .swatch{display:flex;align-items:center;gap:9px}
  .swatch input[type=color]{width:34px;height:34px;flex:0 0 auto;border:1px solid var(--v-line2);
    border-radius:var(--v-r-md);background:var(--v-bg);cursor:pointer;padding:3px}
  .swatch input[type=color]:disabled{opacity:.4;cursor:not-allowed}
  .hex{font-size:var(--v-fs-cap);color:var(--v-dim);text-transform:uppercase}

  .seg{display:flex;gap:2px;background:var(--v-bg);border:1px solid var(--v-line);
    border-radius:var(--v-r-md);padding:3px}
  .seg button{flex:1;height:28px;display:grid;place-items:center;border:0;
    border-radius:var(--v-r-sm);background:none;color:var(--v-dim);cursor:pointer}
  .seg button:hover{color:var(--v-txt)}
  .seg button.on{background:var(--v-surf3);color:var(--v-txt)}

  .sw-row{display:flex;align-items:center;justify-content:space-between;width:100%;
    background:var(--v-surf2);border:1px solid var(--v-line);border-radius:var(--v-r-md);
    padding:9px 12px;color:var(--v-txt);font-size:var(--v-fs-b2);cursor:pointer;
    font-family:var(--f-body)}
  .sw-row:hover{border-color:var(--v-line2)}
  .bg-row{display:flex;flex-wrap:wrap;gap:8px}
  .file{cursor:pointer}
  .bg-thumb{width:100%;height:64px;border-radius:var(--v-r-md);background-size:cover;
    background-position:center;border:1px solid var(--v-line2)}
  .err{flex:0 0 auto;margin:0;padding:10px 14px;border-top:1px solid var(--v-line);
    color:var(--v-red);font-size:var(--v-fs-cap);line-height:1.5}

  .r-btn.confirm:focus-visible,.tbtn:focus-visible,.el:focus-visible,.seg button:focus-visible,
  .sw-inline:focus-visible,.sw-row:focus-visible,.mini-link:focus-visible,
  .tpl-name:focus-visible,.star:focus-visible,.del:focus-visible{outline:2px solid var(--v-accent);outline-offset:2px}

  @media (max-width:1180px){
    .tpl-editor{grid-template-columns:216px minmax(0,1fr) 280px}
    .frow{grid-template-columns:66px minmax(0,1fr)}
  }
  @media (max-width:980px){
    .tpl-editor{grid-template-columns:1fr;height:auto;gap:var(--v-sp-sm)}
    .pane{max-height:none}
    .wysiwyg{width:100%!important;max-width:640px}
  }
</style>
