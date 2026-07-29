<script>
  // Edit Stage Display (relay-stagedisplayeditor-screen) — the ProPresenter-style
  // layer editor, upscaled for stage displays. It reuses the SAME layer model and
  // drag/resize machinery as the Templates editor (layers.js + TemplateRender), so
  // a stage display's layout is edited exactly like a template: add typed layers,
  // drag them on a ruled canvas, bind text to the live verse / reference /
  // countdown / clock, restyle everything, and pick a starting layout.
  //
  // A display owns its layout locally (stagedisplays.js) — Save commits the edited
  // clone back to the store; Cancel discards it. WYSIWYG by construction: the
  // canvas is the same renderer the wall uses.
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import { displays, updateDisplay } from '../../stores/stagedisplays.js';
  import { makeLayer, layerLabel, BINDINGS, STARTERS } from '../../layers.js';

  export let displayId;
  const dispatch = createEventDispatcher();

  let disp = null;
  let edit = null; // { name, layout:{layers}, style }
  let baseSig = '';
  let selId = null;
  let tab = 'layout'; // layout | content | advanced
  let savedTick = false;

  const SAMPLE = { text: 'In the beginning God created the heaven and the earth.', reference: 'Genesis 1:1 · KJV', translation: 'KJV' };

  onMount(() => {
    disp = $displays.find((d) => d.id === displayId) || null;
    if (disp) {
      edit = JSON.parse(JSON.stringify(disp.template));
      edit.layout ??= { layers: [] };
      edit.layout.layers ??= [];
      selId = edit.layout.layers[edit.layout.layers.length - 1]?.id ?? null;
      baseSig = JSON.stringify(edit);
    }
  });
  onDestroy(() => endDrag());

  $: layers = edit?.layout?.layers ?? [];
  $: panelLayers = [...layers].reverse(); // front-to-back
  $: sel = layers.find((l) => l.id === selId) || null;
  $: dirty = edit && JSON.stringify(edit) !== baseSig;

  // ── Layer ops ──────────────────────────────────────────────────────────────
  function addLayerOf(type, over = {}) {
    const L = makeLayer(type, over);
    if (type === 'text' && !over.bind) { L.bind = 'static'; L.text = 'New text'; }
    edit.layout.layers = [...layers, L];
    selId = L.id;
    edit = edit;
  }
  // The OBJECTS palette maps ProPresenter-style objects onto the layer model the
  // renderer supports (text · shape · timer/clock · background). Circle / Line /
  // Image / Icon / Logo / QR render as shape/text placeholders you then style.
  function addObject(kind) {
    switch (kind) {
      case 'text': addLayerOf('text', { name: 'Text' }); break;
      case 'rect': addLayerOf('shape', { name: 'Rectangle' }); break;
      case 'circle': addLayerOf('shape', { name: 'Circle', w: 20, h: 34, radius: 8 }); break;
      case 'line': addLayerOf('shape', { name: 'Line', y: 50, h: 1, radius: 0.4 }); break;
      case 'image': addLayerOf('shape', { name: 'Image', fill: '#1b1b1b' }); break;
      case 'icon': addLayerOf('text', { name: 'Icon', bind: 'static', text: '★', size: 6, align: 'center' }); break;
      case 'countdown': addLayerOf('timer', { name: 'Countdown', bind: 'countdown' }); break;
      case 'clock': addLayerOf('timer', { name: 'Clock', bind: 'clock' }); break;
      case 'logo': addLayerOf('shape', { name: 'Logo', x: 40, y: 8, w: 20, h: 12, fill: '#2a2a2a' }); break;
      case 'qr': addLayerOf('shape', { name: 'QR Code', x: 78, y: 74, w: 14, h: 24, fill: '#f2f2f2' }); break;
      default: addLayerOf('text');
    }
  }
  function removeLayer(id) {
    edit.layout.layers = layers.filter((l) => l.id !== id);
    if (selId === id) selId = layers[layers.length - 1]?.id ?? null;
    edit = edit;
  }
  function moveLayer(id, dir) {
    const a = [...layers];
    const i = a.findIndex((l) => l.id === id);
    const j = i + dir;
    if (i < 0 || j < 0 || j >= a.length) return;
    [a[i], a[j]] = [a[j], a[i]];
    edit.layout.layers = a;
    edit = edit;
  }
  function toggleVisible(id) {
    const L = layers.find((l) => l.id === id);
    if (L) { L.visible = L.visible === false; edit = edit; }
  }
  function set(k, v) { if (sel) { sel[k] = v; edit = edit; } }
  function num(k, v) { set(k, +v); }

  // ── Canvas drag / resize (percent geometry, same as the template editor) ─────
  let boardEl;
  let drag = null;
  function startDrag(e, L, mode) {
    if (L.type === 'background') return;
    e.preventDefault(); e.stopPropagation();
    selId = L.id;
    const r = boardEl.getBoundingClientRect();
    drag = { id: L.id, mode, sx: e.clientX, sy: e.clientY, lx: L.x, ly: L.y, lw: L.w, lh: L.h, bw: r.width, bh: r.height };
    window.addEventListener('pointermove', onDrag);
    window.addEventListener('pointerup', endDrag);
  }
  function onDrag(e) {
    if (!drag) return;
    const L = layers.find((l) => l.id === drag.id);
    if (!L) return;
    const dx = ((e.clientX - drag.sx) / drag.bw) * 100;
    const dy = ((e.clientY - drag.sy) / drag.bh) * 100;
    const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
    if (drag.mode === 'move') {
      L.x = clamp(Math.round((drag.lx + dx) * 10) / 10, 0, 100 - L.w);
      L.y = clamp(Math.round((drag.ly + dy) * 10) / 10, 0, 100 - L.h);
    } else {
      L.w = clamp(Math.round((drag.lw + dx) * 10) / 10, 2, 100 - L.x);
      L.h = clamp(Math.round((drag.lh + dy) * 10) / 10, 2, 100 - L.y);
    }
    edit = edit;
  }
  function endDrag() {
    if (!drag) return;
    drag = null;
    window.removeEventListener('pointermove', onDrag);
    window.removeEventListener('pointerup', endDrag);
  }

  // ── Layouts (starters) ───────────────────────────────────────────────────────
  function applyStarter(make) {
    const s = make();
    edit.layout = s.layout;
    edit.style = s.style;
    selId = s.layout.layers[s.layout.layers.length - 1]?.id ?? null;
    edit = edit;
  }

  // ── px ⇆ percent for the POSITION & SIZE box (canvas is 1920×1080) ────────────
  const CW = 1920, CH = 1080;
  const pxX = (v) => Math.round((v / 100) * CW);
  const pxY = (v) => Math.round((v / 100) * CH);
  const setPxX = (v) => set('x', Math.max(0, Math.min(100 - (sel?.w ?? 0), (+v / CW) * 100)));
  const setPxY = (v) => set('y', Math.max(0, Math.min(100 - (sel?.h ?? 0), (+v / CH) * 100)));
  const setPxW = (v) => set('w', Math.max(2, Math.min(100 - (sel?.x ?? 0), (+v / CW) * 100)));

  // ── Fonts + weights ──────────────────────────────────────────────────────────
  const FONTS = ['Playfair Display', 'Fraunces', 'Inter', 'Space Grotesk', 'JetBrains Mono', 'Georgia', 'Helvetica Neue', 'Arial'];
  const WEIGHTS = [{ v: 300, l: 'Light' }, { v: 400, l: 'Regular' }, { v: 500, l: 'Medium' }, { v: 600, l: 'Semibold' }, { v: 700, l: 'Bold' }];
  const isColor = (v) => typeof v === 'string' && v.startsWith('#');

  const OBJECTS = [
    { kind: 'text', label: 'Text', grp: 'text', ico: '<path d="M4 6h16M12 6v14"/>' },
    { kind: 'rect', label: 'Rectangle', grp: 'shape', ico: '<rect x="4" y="6" width="16" height="12" rx="1"/>' },
    { kind: 'circle', label: 'Circle', grp: 'shape', ico: '<circle cx="12" cy="12" r="8"/>' },
    { kind: 'line', label: 'Line', grp: 'shape', ico: '<path d="M4 12h16"/>' },
    { kind: 'image', label: 'Image', grp: 'image', ico: '<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M3 16l5-5 4 4 3-3 6 6"/>' },
    { kind: 'icon', label: 'Icon', grp: 'image', ico: '<path d="M12 3l2.5 5.5L20 9l-4 4 1 6-5-3-5 3 1-6-4-4 5.5-.5z"/>' },
    { kind: 'countdown', label: 'Countdown', grp: 'text', ico: '<circle cx="12" cy="13" r="8"/><path d="M12 9v4M9 2h6"/>' },
    { kind: 'clock', label: 'Clock', grp: 'text', ico: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>' },
    { kind: 'logo', label: 'Logo', grp: 'image', ico: '<rect x="3" y="6" width="18" height="12" rx="2"/><path d="M7 15l3-4 2 2 2-3 3 5"/>' },
    { kind: 'qr', label: 'QR Code', grp: 'image', ico: '<rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/><path d="M14 14h3v3M20 14v7M14 20h3"/>' },
  ];
  let objFilter = 'all';
  $: objList = objFilter === 'all' ? OBJECTS : OBJECTS.filter((o) => o.grp === objFilter);

  const RULER_X = [0, 200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000];
  const RULER_Y = [0, 200, 400, 600, 800, 1000, 1200, 1400, 1600, 1800, 2000];

  function save() {
    updateDisplay(displayId, { template: JSON.parse(JSON.stringify(edit)) });
    baseSig = JSON.stringify(edit);
    savedTick = true;
    setTimeout(() => (savedTick = false), 1400);
  }
  const layerGlyph = (L) => (L.type === 'background' ? '▦' : L.type === 'shape' ? '▢' : L.bind === 'clock' || L.bind === 'countdown' ? '⏱' : 'T');
</script>

<div class="ed">
  <!-- ══ TOP BAR ══ -->
  <header class="ed-top">
    <div class="ed-tabs">
      {#each ['layout', 'content', 'advanced'] as t}
        <button class="ed-tab" class:on={tab === t} on:click={() => (tab = t)}>{t[0].toUpperCase() + t.slice(1)}</button>
      {/each}
    </div>
    <span class="ed-spring"></span>
    <button class="r-btn ghost sm" on:click={() => dispatch('back')}>Cancel</button>
    <button class="r-btn ghost sm" on:click={() => dispatch('back')}>Preview</button>
    <button class="r-btn primary sm" on:click={save} disabled={!dirty && !savedTick}>{savedTick ? 'Saved ✓' : 'Save Changes'}</button>
  </header>

  {#if !edit}
    <div class="ed-missing r-empty">This display could not be loaded.</div>
  {:else if tab === 'layout'}
    <div class="ed-body">
      <!-- ══ LEFT: LAYERS + OBJECTS ══ -->
      <aside class="ed-left">
        <div class="ed-pane ed-layers">
          <div class="ed-panehead"><span class="r-lbl">Layers</span>
            <button class="ed-addlayer" on:click={() => addObject('text')}><svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>Add Layer</button>
          </div>
          <div class="ed-layerlist r-scroll">
            {#each panelLayers as L (L.id)}
              <div class="ed-layer" class:sel={selId === L.id} class:off={L.visible === false}
                on:click={() => (selId = L.id)} role="button" tabindex="0"
                on:keydown={(e) => { if (e.key === 'Enter') selId = L.id; }}>
                <span class="ed-lglyph">{layerGlyph(L)}</span>
                <span class="ed-ltext"><b>{layerLabel(L)}</b><em>{L.type === 'background' ? 'Image/Color' : L.type === 'shape' ? 'Shape' : 'Text Layer'}</em></span>
                <button class="ed-leye" title="Visibility" on:click|stopPropagation={() => toggleVisible(L.id)} aria-label="Toggle visibility">
                  {#if L.visible === false}
                    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M2 12s3.5-7 10-7c1.6 0 3 .4 4.3 1M22 12s-3.5 7-10 7c-1.6 0-3-.4-4.3-1M3 3l18 18"/></svg>
                  {:else}
                    <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7"><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>
                  {/if}
                </button>
                <span class="ed-ldrag" title="Reorder">
                  <button class="ed-lmini" on:click|stopPropagation={() => moveLayer(L.id, 1)} aria-label="Forward">↑</button>
                  <button class="ed-lmini" on:click|stopPropagation={() => moveLayer(L.id, -1)} aria-label="Back">↓</button>
                  <button class="ed-lmini danger" on:click|stopPropagation={() => removeLayer(L.id)} aria-label="Delete">✕</button>
                </span>
              </div>
            {/each}
            {#if !layers.length}<div class="ed-hint r-mono">No layers — add one below.</div>{/if}
          </div>
        </div>

        <div class="ed-pane ed-objects">
          <div class="ed-panehead"><span class="r-lbl">Objects</span></div>
          <div class="ed-objfilter">
            {#each [['all', 'All'], ['text', 'Text'], ['shape', 'Shape'], ['image', 'Media']] as [k, l]}
              <button class="ed-objf" class:on={objFilter === k} on:click={() => (objFilter = k)}>{l}</button>
            {/each}
          </div>
          <div class="ed-objgrid">
            {#each objList as o (o.kind)}
              <button class="ed-obj" on:click={() => addObject(o.kind)}>
                <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">{@html o.ico}</svg>
                {o.label}
              </button>
            {/each}
          </div>
        </div>
      </aside>

      <!-- ══ CENTER: CANVAS + LAYOUTS ══ -->
      <section class="ed-center">
        <div class="ed-canvas">
          <div class="ed-rulercorner"><button class="ed-addcanvas" on:click={() => addObject('text')} aria-label="Add object">＋</button></div>
          <div class="ed-rulerx">{#each RULER_X as n}<span class="ed-tick" style="left:{(n / 2000) * 100}%">{n}</span>{/each}</div>
          <div class="ed-rulery">{#each RULER_Y as n}<span class="ed-tick" style="top:{(n / 2000) * 100}%">{n}</span>{/each}</div>
          <div class="ed-stage">
            <div class="ed-artboard" bind:this={boardEl}>
              <TemplateRender template={edit} content={SAMPLE} />
              <div class="ed-overlay">
                {#each layers as L (L.id)}
                  {#if L.visible !== false && L.type !== 'background'}
                    <div class="ed-hbox" class:sel={selId === L.id}
                      style="left:{L.x}%; top:{L.y}%; width:{L.w}%; height:{L.h}%;"
                      on:pointerdown={(e) => startDrag(e, L, 'move')} role="button" tabindex="0"
                      on:keydown={(e) => { if (e.key === 'Enter') selId = L.id; }} aria-label={layerLabel(L)}>
                      {#if selId === L.id}
                        <span class="ed-htag">{layerLabel(L)}</span>
                        <span class="ed-hh ed-hh-nw"></span><span class="ed-hh ed-hh-ne"></span>
                        <span class="ed-hh ed-hh-sw"></span>
                        <span class="ed-hh ed-hh-se" on:pointerdown={(e) => startDrag(e, L, 'resize')} role="button" tabindex="-1" aria-label="Resize"></span>
                        <span class="ed-hh ed-hh-w"></span><span class="ed-hh ed-hh-e"></span>
                      {/if}
                    </div>
                  {/if}
                {/each}
              </div>
            </div>
          </div>
        </div>

        <div class="ed-layouts">
          <div class="r-lbl ed-layoutslbl">Layouts</div>
          <div class="ed-layoutrow">
            <button class="ed-layout on"><span class="ed-layicon"><svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4z"/></svg></span><span class="ed-layname">Custom</span></button>
            {#each STARTERS as s}
              <button class="ed-layout" on:click={() => applyStarter(s.make)} title={s.hint}>
                <span class="ed-laythumb"><TemplateRender template={{ ...s.make(), name: s.label }} content={SAMPLE} /></span>
                <span class="ed-layname">{s.label}</span>
              </button>
            {/each}
            <button class="ed-layout ed-laynew" on:click={() => addObject('text')}><span class="ed-layicon">＋</span><span class="ed-layname">New Layout</span></button>
          </div>
        </div>
      </section>

      <!-- ══ RIGHT: INSPECTOR ══ -->
      <aside class="ed-insp r-scroll">
        {#if sel}
          <div class="ed-insphead">
            <div class="ed-inspback"><svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M15 18l-6-6 6-6"/></svg><span class="r-lbl">{sel.type === 'background' ? 'Background Layer' : sel.type === 'shape' ? 'Shape Layer' : 'Text Layer'}</span><span class="ed-inspid r-mono">ID: {sel.id.slice(0, 8).toUpperCase()}</span></div>
            <div class="ed-inspname"><span class="ed-inspglyph">{layerGlyph(sel)}</span><input class="ed-inspnamein" value={layerLabel(sel)} on:input={(e) => set('name', e.target.value)} /></div>
          </div>

          {#if sel.type === 'text' || sel.type === 'timer'}
            <div class="ed-isec r-lbl">Content</div>
            {#if sel.bind === 'static'}
              <textarea class="ed-textarea" value={sel.text || ''} on:input={(e) => set('text', e.target.value)} rows="3"></textarea>
            {:else}
              <div class="ed-bound r-mono">Bound to <b>{BINDINGS.find((b) => b.key === sel.bind)?.label}</b> — follows the live {sel.bind}.</div>
            {/if}
            <div class="ed-ifield"><span class="ed-ik">Source</span>
              <select class="r-select" value={sel.bind} on:change={(e) => set('bind', e.target.value)}>
                {#each BINDINGS as b}<option value={b.key}>{b.label}</option>{/each}
              </select>
            </div>
            <button class="ed-scripted" on:click={() => (tab = 'content')}>Open in Content Tab<svg viewBox="0 0 24 24" width="12" height="12" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M7 17L17 7M9 7h8v8"/></svg></button>

            <div class="ed-isec r-lbl">Typography</div>
            <div class="ed-ifield"><span class="ed-ik">Font Family</span>
              <select class="r-select" value={sel.font} on:change={(e) => set('font', e.target.value)}>
                {#if sel.font && !FONTS.includes(sel.font)}<option value={sel.font}>{sel.font}</option>{/if}
                {#each FONTS as f}<option value={f}>{f}</option>{/each}
              </select>
            </div>
            <div class="ed-i2">
              <label class="ed-il">Weight<select class="r-select" value={sel.weight || 400} on:change={(e) => num('weight', e.target.value)}>{#each WEIGHTS as w}<option value={w.v}>{w.l}</option>{/each}</select></label>
              <label class="ed-il">Size<span class="ed-stepper"><input class="ed-num r-mono" type="number" min="1" max="20" step="0.1" value={sel.size} on:input={(e) => num('size', e.target.value)} /><span class="ed-unit">cqw</span></span></label>
            </div>
            <div class="ed-i2">
              <label class="ed-il">Line Height<input class="ed-num r-mono" type="number" min="0.8" max="2.5" step="0.05" value={sel.lineHeight || 1.32} on:input={(e) => num('lineHeight', e.target.value)} /></label>
              <label class="ed-il">Letter Spacing<input class="ed-num r-mono" type="number" min="-0.05" max="0.5" step="0.01" value={sel.letterSpacing || 0} on:input={(e) => num('letterSpacing', e.target.value)} /></label>
            </div>
            <div class="ed-ifield"><span class="ed-ik">Text Align</span>
              <span class="ed-seg">
                {#each ['left', 'center', 'right'] as a}
                  <button class:on={sel.align === a} on:click={() => set('align', a)} aria-label="Align {a}"><svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 6h16"/><path d={a === 'left' ? 'M4 12h10' : a === 'right' ? 'M10 12h10' : 'M7 12h10'}/><path d="M4 18h16"/></svg></button>
                {/each}
              </span>
            </div>
            <div class="ed-ifield"><span class="ed-ik">Text Colour</span>
              <span class="ed-swatch"><input type="color" value={isColor(sel.color) ? sel.color : '#ffffff'} on:input={(e) => set('color', e.target.value)} /><input class="ed-hexin r-mono" value={isColor(sel.color) ? sel.color.toUpperCase() : '#FFFFFF'} on:change={(e) => set('color', e.target.value)} /></span>
            </div>

            <div class="ed-isec r-lbl">Appearance</div>
            <div class="ed-ifield"><span class="ed-ik">Opacity</span><span class="ed-rangerow"><input class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} /><span class="ed-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="ed-swrow"><span>Text Shadow</span><button class="ed-toggle" class:on={(sel.shadow || 0) > 0} on:click={() => set('shadow', (sel.shadow || 0) > 0 ? 0 : 0.4)} aria-label="Text shadow"><span></span></button></div>
            {#if (sel.shadow || 0) > 0}
              <div class="ed-ifield"><span class="ed-ik">Softness</span><span class="ed-rangerow"><input class="r-range" type="range" min="0.05" max="1" step="0.05" value={sel.shadow} on:input={(e) => num('shadow', e.target.value)} /><span class="ed-rnum r-mono">{Math.round(sel.shadow * 100)}%</span></span></div>
            {/if}
            <div class="ed-swrow"><span>Italic</span><button class="ed-toggle" class:on={sel.italic} on:click={() => set('italic', !sel.italic)} aria-label="Italic"><span></span></button></div>

          {:else if sel.type === 'shape'}
            <div class="ed-isec r-lbl">Fill</div>
            <div class="ed-ifield"><span class="ed-ik">Colour</span><span class="ed-swatch"><input type="color" value={isColor(sel.fill) ? sel.fill : '#101319'} on:input={(e) => set('fill', e.target.value)} /><input class="ed-hexin r-mono" value={isColor(sel.fill) ? sel.fill.toUpperCase() : '#101319'} on:change={(e) => set('fill', e.target.value)} /></span></div>
            <div class="ed-isec r-lbl">Appearance</div>
            <div class="ed-ifield"><span class="ed-ik">Opacity</span><span class="ed-rangerow"><input class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} /><span class="ed-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="ed-ifield"><span class="ed-ik">Corner</span><span class="ed-rangerow"><input class="r-range" type="range" min="0" max="12" step="0.2" value={sel.radius || 0} on:input={(e) => num('radius', e.target.value)} /><span class="ed-rnum r-mono">{(sel.radius || 0).toFixed(1)}</span></span></div>

          {:else}
            <div class="ed-isec r-lbl">Background</div>
            <div class="ed-ifield"><span class="ed-ik">Fill</span><span class="ed-swatch"><input type="color" value={isColor(sel.fill) ? sel.fill : '#0b0906'} on:input={(e) => { set('fill', e.target.value); set('image', null); }} /><span class="ed-hexin r-mono">{isColor(sel.fill) ? sel.fill.toUpperCase() : 'gradient'}</span></span></div>
            <div class="ed-ifield"><span class="ed-ik">Opacity</span><span class="ed-rangerow"><input class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} /><span class="ed-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="ed-ifield"><span class="ed-ik">Dim</span><span class="ed-rangerow"><input class="r-range" type="range" min="0" max="0.9" step="0.05" value={sel.dim || 0} on:input={(e) => num('dim', e.target.value)} /><span class="ed-rnum r-mono">{Math.round((sel.dim || 0) * 100)}%</span></span></div>
          {/if}

          {#if sel.type !== 'background'}
            <div class="ed-isec r-lbl">Position &amp; Size</div>
            <div class="ed-i3">
              <label class="ed-il">X<input class="ed-num r-mono" type="number" value={pxX(sel.x)} on:input={(e) => setPxX(e.target.value)} /></label>
              <label class="ed-il">Y<input class="ed-num r-mono" type="number" value={pxY(sel.y)} on:input={(e) => setPxY(e.target.value)} /></label>
              <label class="ed-il">W<input class="ed-num r-mono" type="number" value={pxX(sel.w)} on:input={(e) => setPxW(e.target.value)} /></label>
            </div>
            <p class="ed-inote">Pixels on a {CW}×{CH} canvas. Drag on the canvas, or type exact values.</p>
          {/if}
        {:else}
          <div class="ed-inspempty r-empty">Select a layer to edit it, or add an object.</div>
        {/if}
      </aside>
    </div>

  {:else if tab === 'content'}
    <div class="ed-tabbody">
      <p class="ed-lead">Bound text layers follow the live service. Type fixed text for static layers here; bound layers show what they track.</p>
      <div class="ed-contentlist">
        {#each layers.filter((l) => l.type === 'text' || l.type === 'timer') as L (L.id)}
          <div class="ed-crow">
            <div class="ed-chead"><span class="ed-lglyph">{layerGlyph(L)}</span><b>{layerLabel(L)}</b><span class="ed-cbind r-mono">{BINDINGS.find((b) => b.key === L.bind)?.label}</span></div>
            {#if L.bind === 'static'}
              <input class="r-input" value={L.text || ''} on:input={(e) => { L.text = e.target.value; edit = edit; }} placeholder="Fixed text" />
            {:else}
              <div class="ed-bound r-mono">Live — {L.bind}</div>
            {/if}
          </div>
        {/each}
        {#if !layers.some((l) => l.type === 'text' || l.type === 'timer')}<div class="r-empty">No text layers yet.</div>{/if}
      </div>
    </div>

  {:else}
    <div class="ed-tabbody">
      <p class="ed-lead">Display hardware and output settings for <b>{disp?.name}</b>.</p>
      <div class="ed-advgrid">
        <div class="ed-arow"><span class="ed-ik">Resolution</span><span class="r-mono">{disp?.res[0]} × {disp?.res[1]}</span></div>
        <div class="ed-arow"><span class="ed-ik">Refresh Rate</span><span class="r-mono">{disp?.fps} FPS</span></div>
        <div class="ed-arow"><span class="ed-ik">Connection</span><span class="r-mono">{disp?.connection}</span></div>
        <div class="ed-arow"><span class="ed-ik">Colour Profile</span><span class="r-mono">{disp?.colour}</span></div>
        <div class="ed-arow"><span class="ed-ik">Layers</span><span class="r-mono">{layers.length}</span></div>
      </div>
      <p class="ed-inote">Change hardware settings on the display card's General tab. Wire the output to a real screen in the Channels tab.</p>
    </div>
  {/if}
</div>

<style>
  .ed{ display:flex; flex-direction:column; height:100%; min-height:0; gap:12px; }
  .ed-spring{ flex:1; }
  .ed-top{ display:flex; align-items:center; gap:8px; flex:0 0 auto; }
  .ed-tabs{ display:flex; gap:4px; }
  .ed-tab{ padding:8px 14px; border:0; background:none; color:var(--v-faint); font-size:13.5px; font-weight:500;
    cursor:pointer; border-bottom:2px solid transparent; }
  .ed-tab:hover{ color:var(--v-txt); }
  .ed-tab.on{ color:var(--v-accent2); border-bottom-color:var(--v-accent); }
  .ed-missing{ margin:auto; padding:40px; }

  .ed-body{ flex:1; min-height:0; display:grid; grid-template-columns:236px minmax(0,1fr) 296px; gap:12px; }
  @media (max-width:1240px){ .ed-body{ grid-template-columns:206px minmax(0,1fr) 268px; } }

  .ed-pane{ display:flex; flex-direction:column; min-height:0; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); overflow:hidden; }
  .ed-panehead{ display:flex; align-items:center; justify-content:space-between; padding:11px 13px; border-bottom:1px solid var(--v-line); flex:0 0 auto; }

  /* LEFT column */
  .ed-left{ display:flex; flex-direction:column; gap:12px; min-height:0; }
  .ed-layers{ flex:1 1 60%; min-height:0; }
  .ed-objects{ flex:0 0 auto; }
  .ed-addlayer{ display:flex; align-items:center; gap:5px; padding:5px 9px; border-radius:var(--v-r-sm); border:1px solid var(--v-accent-line);
    background:var(--v-accent-soft); color:var(--v-accent2); font-size:11px; font-weight:600; cursor:pointer; }
  .ed-layerlist{ flex:1; min-height:0; overflow-y:auto; padding:8px; display:flex; flex-direction:column; gap:4px; }
  .ed-layer{ display:flex; align-items:center; gap:9px; padding:8px 10px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); cursor:pointer; }
  .ed-layer:hover{ border-color:var(--v-line2); }
  .ed-layer.sel{ border-color:var(--v-accent-line); background:var(--v-accent-soft); }
  .ed-layer.off{ opacity:.5; }
  .ed-lglyph{ width:18px; text-align:center; flex:0 0 auto; font-family:var(--f-mono); font-size:12px; color:var(--v-faint); }
  .ed-ltext{ flex:1; min-width:0; display:flex; flex-direction:column; gap:1px; }
  .ed-ltext b{ font-size:12.5px; font-weight:600; color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .ed-ltext em{ font-style:normal; font-size:10px; color:var(--v-faint); }
  .ed-leye{ flex:0 0 auto; width:24px; height:24px; display:grid; place-items:center; border:0; background:none; color:var(--v-faint); cursor:pointer; border-radius:var(--v-r-sm); }
  .ed-leye:hover{ color:var(--v-txt); }
  .ed-ldrag{ display:flex; gap:1px; flex:0 0 auto; opacity:0; }
  .ed-layer:hover .ed-ldrag, .ed-layer.sel .ed-ldrag{ opacity:1; }
  .ed-lmini{ width:18px; height:20px; display:grid; place-items:center; border:0; background:none; color:var(--v-faint); cursor:pointer; border-radius:3px; font-size:10px; }
  .ed-lmini:hover{ color:var(--v-txt); background:var(--v-surf3); }
  .ed-lmini.danger:hover{ color:var(--v-red); }
  .ed-hint{ padding:14px 8px; text-align:center; font-size:10px; color:var(--v-faint); }

  .ed-objfilter{ display:flex; gap:3px; padding:8px 10px 0; }
  .ed-objf{ padding:4px 9px; border:0; background:none; color:var(--v-faint); font-size:11.5px; cursor:pointer; border-radius:var(--v-r-sm); }
  .ed-objf:hover{ color:var(--v-txt); }
  .ed-objf.on{ background:var(--v-surf3); color:var(--v-txt); }
  .ed-objgrid{ display:grid; grid-template-columns:1fr 1fr; gap:7px; padding:10px; }
  .ed-obj{ display:flex; align-items:center; gap:8px; padding:9px 10px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-dim); font-size:11.5px; cursor:pointer; }
  .ed-obj:hover{ color:var(--v-txt); border-color:var(--v-accent-line); }
  .ed-obj svg{ color:var(--v-faint); flex:0 0 auto; }
  .ed-obj:hover svg{ color:var(--v-accent2); }

  /* CENTER */
  .ed-center{ display:flex; flex-direction:column; min-height:0; gap:12px; }
  .ed-canvas{ position:relative; flex:1; min-height:0; background:var(--v-surf); border:1px solid var(--v-line);
    border-radius:var(--v-r-lg); padding:26px 12px 12px 34px; overflow:hidden; }
  .ed-rulercorner{ position:absolute; top:0; left:0; width:34px; height:26px; z-index:3; display:grid; place-items:center; }
  .ed-addcanvas{ width:22px; height:22px; border-radius:50%; border:1px solid var(--v-line2); background:var(--v-surf2); color:var(--v-dim); cursor:pointer; font-size:13px; line-height:1; }
  .ed-rulerx{ position:absolute; top:0; left:34px; right:12px; height:26px; }
  .ed-rulerx .ed-tick{ position:absolute; top:8px; transform:translateX(-50%); font-family:var(--f-mono); font-size:8.5px; color:var(--v-faint); }
  .ed-rulery{ position:absolute; top:26px; left:0; bottom:12px; width:34px; }
  .ed-rulery .ed-tick{ position:absolute; left:6px; transform:translateY(-50%); font-family:var(--f-mono); font-size:8.5px; color:var(--v-faint); }
  .ed-stage{ height:100%; display:flex; align-items:center; justify-content:center; }
  .ed-artboard{ position:relative; width:100%; max-height:100%; aspect-ratio:16/9; border-radius:var(--v-r-md);
    overflow:hidden; border:1px solid var(--v-line2); box-shadow:var(--v-shadow-lg); }
  .ed-overlay{ position:absolute; inset:0; z-index:5; }
  .ed-hbox{ position:absolute; box-sizing:border-box; border:1px dashed rgba(255,255,255,.22); cursor:move; }
  .ed-hbox:hover{ border-color:rgba(255,255,255,.45); }
  .ed-hbox.sel{ border:1px solid var(--v-accent); }
  .ed-htag{ position:absolute; top:-17px; left:0; font-family:var(--f-mono); font-size:8px; color:#fff; background:var(--v-accent-fill); padding:2px 6px; border-radius:3px; white-space:nowrap; }
  .ed-hh{ position:absolute; width:8px; height:8px; background:#fff; border:1px solid var(--v-accent); border-radius:1px; }
  .ed-hh-nw{ left:-4px; top:-4px; } .ed-hh-ne{ right:-4px; top:-4px; }
  .ed-hh-sw{ left:-4px; bottom:-4px; } .ed-hh-se{ right:-4px; bottom:-4px; cursor:nwse-resize; }
  .ed-hh-w{ left:-4px; top:calc(50% - 4px); } .ed-hh-e{ right:-4px; top:calc(50% - 4px); }

  /* LAYOUTS strip */
  .ed-layouts{ flex:0 0 auto; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); padding:12px 14px; }
  .ed-layoutslbl{ margin-bottom:10px; }
  .ed-layoutrow{ display:flex; gap:10px; overflow-x:auto; padding-bottom:2px; }
  .ed-layout{ flex:0 0 auto; width:110px; display:flex; flex-direction:column; gap:6px; padding:8px; cursor:pointer;
    background:var(--v-surf2); border:1px solid var(--v-line); border-radius:var(--v-r-md); }
  .ed-layout:hover{ border-color:var(--v-line2); }
  .ed-layout.on{ border-color:var(--v-accent-line); background:var(--v-accent-soft); }
  .ed-layicon{ height:52px; display:grid; place-items:center; color:var(--v-accent2); font-size:20px; border-radius:var(--v-r-sm); background:var(--v-void); }
  .ed-laythumb{ position:relative; height:52px; border-radius:var(--v-r-sm); overflow:hidden; border:1px solid var(--v-line); background:var(--v-void); }
  .ed-layname{ font-size:11px; color:var(--v-dim); text-align:center; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .ed-laynew .ed-layicon{ color:var(--v-faint); border:1px dashed var(--v-line2); background:none; }

  /* INSPECTOR */
  .ed-insp{ display:flex; flex-direction:column; min-height:0; overflow-y:auto; gap:10px; padding:14px;
    background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  .ed-insphead{ display:flex; flex-direction:column; gap:8px; padding-bottom:12px; border-bottom:1px solid var(--v-line); }
  .ed-inspback{ display:flex; align-items:center; gap:6px; color:var(--v-faint); }
  .ed-inspid{ margin-left:auto; font-size:9px; color:var(--v-faint); }
  .ed-inspname{ display:flex; align-items:center; gap:9px; }
  .ed-inspglyph{ width:24px; height:24px; display:grid; place-items:center; border-radius:var(--v-r-sm); background:var(--v-surf2); color:var(--v-accent2); font-family:var(--f-mono); font-size:13px; }
  .ed-inspnamein{ flex:1; min-width:0; background:none; border:0; outline:none; color:var(--v-txt); font-family:var(--f-head); font-size:16px; font-weight:600; }
  .ed-inspnamein:focus{ border-bottom:1px solid var(--v-accent-line); }

  .ed-isec{ margin-top:6px; padding-top:12px; border-top:1px solid var(--v-line); }
  .ed-isec:first-of-type{ border-top:0; padding-top:2px; }
  .ed-textarea{ width:100%; box-sizing:border-box; resize:vertical; min-height:66px; padding:10px 12px; border-radius:var(--v-r-md);
    background:var(--v-bg); border:1px solid var(--v-line2); color:var(--v-txt); font-family:var(--f-body); font-size:13px; line-height:1.5; outline:none; }
  .ed-textarea:focus{ border-color:var(--v-accent-line); }
  .ed-bound{ font-size:11px; color:var(--v-faint); background:var(--v-surf2); border:1px solid var(--v-line); border-radius:var(--v-r-sm); padding:8px 10px; line-height:1.5; }
  .ed-bound b{ color:var(--v-cyan); }
  .ed-scripted{ display:flex; align-items:center; justify-content:center; gap:7px; width:100%; padding:9px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-dim); font-size:12px; cursor:pointer; }
  .ed-scripted:hover{ color:var(--v-txt); border-color:var(--v-accent-line); }

  .ed-ifield{ display:flex; flex-direction:column; gap:6px; }
  .ed-ik{ font-size:11px; color:var(--v-faint); }
  .ed-i2{ display:grid; grid-template-columns:1fr 1fr; gap:10px; }
  .ed-i3{ display:grid; grid-template-columns:1fr 1fr 1fr; gap:8px; }
  .ed-il{ display:flex; flex-direction:column; gap:6px; font-size:11px; color:var(--v-faint); }
  .ed-num{ height:32px; padding:0 9px; border-radius:var(--v-r-md); background:var(--v-bg); border:1px solid var(--v-line2); color:var(--v-txt); font-size:12.5px; outline:none; width:100%; box-sizing:border-box; }
  .ed-num:focus{ border-color:var(--v-accent-line); }
  .ed-stepper{ display:flex; }
  .ed-stepper .ed-num{ border-radius:var(--v-r-md) 0 0 var(--v-r-md); border-right:0; }
  .ed-unit{ flex:0 0 auto; height:32px; display:grid; place-items:center; padding:0 9px; border-radius:0 var(--v-r-md) var(--v-r-md) 0; background:var(--v-surf2); border:1px solid var(--v-line2); font-family:var(--f-mono); font-size:9px; color:var(--v-faint); }
  .ed-seg{ display:flex; gap:2px; background:var(--v-bg); border:1px solid var(--v-line); border-radius:var(--v-r-md); padding:3px; }
  .ed-seg button{ flex:1; height:28px; display:grid; place-items:center; border:0; border-radius:var(--v-r-sm); background:none; color:var(--v-dim); cursor:pointer; }
  .ed-seg button:hover{ color:var(--v-txt); }
  .ed-seg button.on{ background:var(--v-accent-fill); color:var(--v-accent-ink); }
  .ed-swatch{ display:flex; align-items:center; gap:8px; }
  .ed-swatch input[type=color]{ width:34px; height:32px; flex:0 0 auto; border:1px solid var(--v-line2); border-radius:var(--v-r-md); background:var(--v-bg); cursor:pointer; padding:3px; }
  .ed-hexin{ flex:1; min-width:0; height:32px; padding:0 9px; border-radius:var(--v-r-md); background:var(--v-bg); border:1px solid var(--v-line2); color:var(--v-txt); font-size:12px; outline:none; }
  .ed-rangerow{ display:flex; align-items:center; gap:9px; }
  .ed-rangerow .r-range{ flex:1; min-width:0; }
  .ed-rnum{ flex:0 0 auto; min-width:42px; text-align:right; font-size:11px; color:var(--v-dim); }
  .ed-swrow{ display:flex; align-items:center; justify-content:space-between; padding:8px 0; font-size:13px; color:var(--v-txt); }
  .ed-toggle{ position:relative; width:40px; height:22px; border-radius:99px; border:1px solid var(--v-line2); background:var(--v-surf3); cursor:pointer; padding:0; }
  .ed-toggle.on{ background:var(--v-accent-fill); border-color:var(--v-accent-fill); }
  .ed-toggle span{ position:absolute; top:1px; left:1px; width:18px; height:18px; border-radius:50%; background:#fff; transition:transform .15s; }
  .ed-toggle.on span{ transform:translateX(18px); }
  .ed-inote{ margin:2px 0 0; font-size:10.5px; color:var(--v-faint); line-height:1.5; }
  .ed-inspempty{ padding:30px 10px; text-align:center; }

  /* content / advanced tabs */
  .ed-tabbody{ flex:1; min-height:0; overflow-y:auto; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); padding:20px 22px; }
  .ed-lead{ margin:0 0 16px; font-size:13px; color:var(--v-dim); line-height:1.6; }
  .ed-lead b{ color:var(--v-txt); }
  .ed-contentlist{ display:flex; flex-direction:column; gap:10px; max-width:560px; }
  .ed-crow{ display:flex; flex-direction:column; gap:8px; padding:12px 14px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line); }
  .ed-chead{ display:flex; align-items:center; gap:9px; }
  .ed-chead b{ font-size:13px; color:var(--v-txt); }
  .ed-cbind{ margin-left:auto; font-size:10px; color:var(--v-faint); }
  .ed-advgrid{ display:flex; flex-direction:column; gap:2px; max-width:420px; }
  .ed-arow{ display:flex; align-items:center; justify-content:space-between; padding:11px 2px; border-bottom:1px solid var(--v-line); font-size:13px; color:var(--v-dim); }
  .ed-arow .r-mono{ color:var(--v-txt); font-size:12px; }

  @media (max-width:980px){
    .ed{ height:auto; }
    .ed-body{ grid-template-columns:1fr; }
    .ed-canvas{ min-height:340px; }
  }
</style>
