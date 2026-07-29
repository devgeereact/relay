<script>
  // Template editor — a ProPresenter-style LAYER editor.
  //
  // A template is a stack of typed, independently-styled, independently-positioned
  // layers (background · shape · text · timer). Add/remove/reorder any of them,
  // drag them on the canvas, bind a text layer to the live verse / reference /
  // translation / countdown / clock, or type fixed text. Lower third, full
  // screen, announcement ticker and freestyle are all just different stacks.
  //
  // Legacy region templates (the built-in presets/themes) still render the old
  // way; opening one offers a one-click "convert to layers" so it becomes freely
  // editable without breaking the ones left alone.
  //
  // The preview is the SAME TemplateRender as the wall — WYSIWYG by construction.
  import { createEventDispatcher, onMount, onDestroy } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import {
    capture, templates, loadTemplates, saveTemplate,
    listOutputChannels, setChannelTemplate, getContentTemplates, setContentTemplate,
  } from '../../stores/capture.js';
  import { BACKGROUNDS } from '../../backgrounds.js';
  import {
    makeLayer, isLayered, layerLabel, regionsToLayers, templateShows, CONTENT_KINDS,
    LAYER_TYPES, BINDINGS,
  } from '../../layers.js';

  export let templateId;
  const dispatch = createEventDispatcher();

  let edit = null;
  let saving = false;
  let savedTick = false;
  let err = '';
  let selId = null;
  let addOpen = false;

  onMount(async () => {
    if (!$templates.length) await loadTemplates();
    load(templateId);
    detectFonts(true);
    loadAssign();
  });
  onDestroy(() => clearTimeout(liveTimer));

  function load(id) {
    const t = $templates.find((x) => x.id === id);
    if (!t) { edit = null; return; }
    // Deep, defensive clone — a private copy detached from the store, never a
    // shared reference (editing one template must never touch another).
    edit = JSON.parse(JSON.stringify(t));
    edit.layout ??= {};
    edit.style ??= {};
    selId = layered ? edit.layout.layers[0]?.id ?? null : null;
    lastSig = sigOf(edit);
  }

  $: layered = isLayered(edit);
  $: layers = layered ? edit.layout.layers : [];
  // Panel shows front-to-back (top of list = front = last in the array).
  $: panelLayers = [...layers].reverse();
  $: sel = layers.find((l) => l.id === selId) || null;

  function convertToLayers() {
    edit.layout = regionsToLayers(edit);
    edit = edit;
    selId = edit.layout.layers[0]?.id ?? null;
  }

  // ── Layer operations ───────────────────────────────────────────────────────
  function addLayer(type) {
    addOpen = false;
    if (!edit.layout.layers) edit.layout.layers = [];
    const L = makeLayer(type);
    // A new text layer defaults to a static line so it shows something at once.
    if (type === 'text') { L.bind = 'static'; L.text = 'New text'; }
    edit.layout.layers = [...edit.layout.layers, L]; // top of the stack
    selId = L.id;
    edit = edit;
  }
  function addBoundText(bind) {
    addOpen = false;
    const L = makeLayer('text', { bind, name: BINDINGS.find((b) => b.key === bind)?.label });
    edit.layout.layers = [...(edit.layout.layers || []), L];
    selId = L.id;
    edit = edit;
  }
  function removeLayer(id) {
    edit.layout.layers = edit.layout.layers.filter((l) => l.id !== id);
    if (selId === id) selId = edit.layout.layers[edit.layout.layers.length - 1]?.id ?? null;
    edit = edit;
  }
  function moveLayer(id, dir) {
    const a = edit.layout.layers;
    const i = a.findIndex((l) => l.id === id);
    const j = i + dir; // +1 = toward front
    if (i < 0 || j < 0 || j >= a.length) return;
    [a[i], a[j]] = [a[j], a[i]];
    edit.layout.layers = [...a];
    edit = edit;
  }
  function toggleVisible(id) {
    const L = edit.layout.layers.find((l) => l.id === id);
    if (L) { L.visible = L.visible === false; edit = edit; }
  }
  // LOCK a layer against accidental movement. A locked layer can't be dragged,
  // resized or nudged, and its canvas box lets clicks pass THROUGH to whatever is
  // under it — so you can grab a layer sitting beneath a locked one. Unlock from
  // the same button to move it again.
  function toggleLock(id) {
    const L = edit.layout.layers.find((l) => l.id === id);
    if (L) { L.locked = !L.locked; edit = edit; }
  }
  // Per-screen content visibility. `layout.shows` is the allow-list of kinds this
  // screen displays; absent = shows everything. Toggling a kind materialises the
  // list (folding in any legacy noMedia opt-out) so the choice is explicit.
  function toggleShows(kind) {
    const all = CONTENT_KINDS.map((k) => k.key);
    let shows;
    if (Array.isArray(edit.layout.shows)) {
      shows = [...edit.layout.shows];
    } else {
      shows = edit.layout.noMedia ? all.filter((k) => k !== 'media') : [...all];
    }
    shows = shows.includes(kind) ? shows.filter((k) => k !== kind) : [...shows, kind];
    edit.layout.shows = shows;
    delete edit.layout.noMedia; // superseded by the explicit list
    edit = edit;
  }
  function set(k, v) { if (sel) { sel[k] = v; edit = edit; } }
  function num(k, v) { set(k, +v); }

  // ── Canvas drag / resize ───────────────────────────────────────────────────
  // `mode` is 'move' or a compass direction (n/s/e/w/ne/nw/se/sw) — a handle on
  // any of the box's eight sides, so a layer can be resized from every edge.
  const HANDLES = ['nw', 'n', 'ne', 'e', 'se', 's', 'sw', 'w'];
  let boardEl;
  let drag = null;
  // Live alignment guides — the vertical/horizontal lines that flash while a
  // layer's centre or edge snaps to the canvas centre or edges. `null` = hidden.
  let guides = { v: null, h: null };
  const r1 = (v) => Math.round(v * 10) / 10;
  function startDrag(e, L, mode) {
    if (L.type === 'background') return; // background is full-frame
    if (L.locked) return; // locked — no accidental movement
    e.preventDefault();
    e.stopPropagation();
    selId = L.id;
    const r = boardEl.getBoundingClientRect();
    drag = { id: L.id, mode, sx: e.clientX, sy: e.clientY, lx: L.x, ly: L.y, lw: L.w, lh: L.h, bw: r.width, bh: r.height };
    window.addEventListener('pointermove', onDrag);
    window.addEventListener('pointerup', endDrag);
  }
  function onDrag(e) {
    if (!drag) return;
    const L = edit.layout.layers.find((l) => l.id === drag.id);
    if (!L) return;
    const dx = ((e.clientX - drag.sx) / drag.bw) * 100;
    const dy = ((e.clientY - drag.sy) / drag.bh) * 100;
    const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
    const m = drag.mode;
    if (m === 'move') {
      let nx = clamp(r1(drag.lx + dx), 0, 100 - L.w);
      let ny = clamp(r1(drag.ly + dy), 0, 100 - L.h);
      // Snap the layer's centre/edges to the canvas centre (50%) and edges
      // (0/100%), and show the guide line while snapped. Holding a key isn't
      // required — the threshold is small enough to snap only when close, which
      // is how "centre items" is expected to feel. Shift disables snapping for
      // fine placement.
      const SNAP = e.shiftKey ? 0 : 1.2;
      let gv = null;
      let gh = null;
      if (SNAP) {
        const cx = nx + L.w / 2;
        if (Math.abs(cx - 50) <= SNAP) { nx = 50 - L.w / 2; gv = 50; }
        else if (Math.abs(nx) <= SNAP) { nx = 0; gv = 0; }
        else if (Math.abs(nx + L.w - 100) <= SNAP) { nx = 100 - L.w; gv = 100; }
        const cy = ny + L.h / 2;
        if (Math.abs(cy - 50) <= SNAP) { ny = 50 - L.h / 2; gh = 50; }
        else if (Math.abs(ny) <= SNAP) { ny = 0; gh = 0; }
        else if (Math.abs(ny + L.h - 100) <= SNAP) { ny = 100 - L.h; gh = 100; }
      }
      L.x = r1(clamp(nx, 0, 100 - L.w));
      L.y = r1(clamp(ny, 0, 100 - L.h));
      guides = { v: gv, h: gh };
    } else {
      // Resize from any edge. Left/top edges move the origin and shrink; right/
      // bottom edges only grow the size. Min size 4%, clamped to the frame.
      let { lx: x, ly: y, lw: w, lh: h } = drag;
      if (m.includes('e')) w = clamp(r1(drag.lw + dx), 4, 100 - x);
      if (m.includes('s')) h = clamp(r1(drag.lh + dy), 4, 100 - y);
      if (m.includes('w')) { const nx = clamp(r1(drag.lx + dx), 0, x + w - 4); w = r1(x + w - nx); x = nx; }
      if (m.includes('n')) { const ny = clamp(r1(drag.ly + dy), 0, y + h - 4); h = r1(y + h - ny); y = ny; }
      L.x = x; L.y = y; L.w = w; L.h = h;
    }
    edit = edit;
  }
  function endDrag() {
    drag = null;
    guides = { v: null, h: null };
    window.removeEventListener('pointermove', onDrag);
    window.removeEventListener('pointerup', endDrag);
  }
  // One-click centring — the "center items" ask. Centres the selected layer on
  // the canvas, horizontally, vertically, or both.
  function center(axis) {
    if (!sel || sel.type === 'background') return;
    if (axis === 'x' || axis === 'both') set('x', r1(50 - sel.w / 2));
    if (axis === 'y' || axis === 'both') set('y', r1(50 - sel.h / 2));
  }
  // Nudge with arrow keys when a layer is selected (1% steps, 5% with Shift).
  function onCanvasKey(e) {
    if (!sel || sel.type === 'background' || sel.locked) return;
    const step = e.shiftKey ? 5 : 1;
    const map = { ArrowLeft: ['x', -step], ArrowRight: ['x', step], ArrowUp: ['y', -step], ArrowDown: ['y', step] };
    const m = map[e.key];
    if (!m) return;
    e.preventDefault();
    const [k, d] = m;
    const max = k === 'x' ? 100 - sel.w : 100 - sel.h;
    set(k, Math.max(0, Math.min(max, Math.round(sel[k] + d))));
  }

  // ── Preview content + canvas chrome ────────────────────────────────────────
  const SAMPLE = {
    text: 'And God called the firmament Heaven. And the evening and the morning were the second day.',
    reference: 'Genesis 1:8 · KJV',
    translation: 'KJV',
  };
  // Preview content for the artboard. The sample picture is added ONLY when the
  // template actually has a Media layer, so a media template can be placed against
  // a real image — but a template with NO media layer stays TRANSPARENT (the
  // checker shows through), instead of the sample picture filling the frame
  // (media renders full-frame by default, so injecting it unconditionally painted
  // every empty template with a background it never had).
  $: hasMediaLayer = layered && layers.some((l) => l.type === 'media');
  $: previewContent = hasMediaLayer && BACKGROUNDS.length
    ? { ...SAMPLE, media_url: BACKGROUNDS[0].url, media_kind: 'image' }
    : SAMPLE;
  const ZOOMS = [40, 55, 70, 85, 100];
  let zoomIdx = ZOOMS.length - 1;
  $: zoom = ZOOMS[zoomIdx];
  let previewMode = false;

  $: transparentBg = !layers.some((l) => l.type === 'background' && l.visible !== false);

  // ── Fonts ──────────────────────────────────────────────────────────────────
  let fonts = [
    'Fraunces', 'Playfair Display', 'Space Grotesk', 'Inter', 'JetBrains Mono',
    'Georgia', 'Times New Roman', 'Palatino', 'Baskerville', 'Garamond',
    'Helvetica Neue', 'Arial', 'Futura', 'Gill Sans', 'Optima', 'Didot',
    'Menlo', 'Courier New', 'Verdana', 'Trebuchet MS', 'Cambria',
  ];
  const FONT_LABEL = {
    'var(--f-serif)': 'Fraunces (serif)', 'var(--f-display)': 'Inter (display)',
    'var(--f-body)': 'Inter (body)', 'var(--f-mono)': 'JetBrains Mono', 'var(--f-head)': 'Inter (heading)',
  };
  const fontLabel = (f) => FONT_LABEL[f] ?? f;
  const BUNDLED_FONTS = new Set([
    'var(--f-serif)', 'var(--f-display)', 'var(--f-body)', 'var(--f-mono)', 'var(--f-head)',
    'Fraunces', 'Inter', 'Playfair Display', 'Space Grotesk', 'JetBrains Mono',
  ]);
  let fontMsg = '';
  let detected = new Set();
  async function detectFonts(auto = false) {
    if (!window.queryLocalFonts) { if (!auto) fontMsg = 'not supported here'; return; }
    try {
      const avail = await window.queryLocalFonts();
      const fams = [...new Set(avail.map((f) => f.family))].sort();
      if (fams.length) { detected = new Set(fams); fonts = [...new Set([...fams, ...fonts])]; fontMsg = `${fams.length} fonts`; }
    } catch { if (!auto) fontMsg = 'permission needed — click to allow'; }
  }
  function fontInstalled(f) {
    if (!f || BUNDLED_FONTS.has(f)) return true;
    if (detected.has(f)) return true;
    try { return document?.fonts?.check(`16px "${f}"`); } catch { return true; }
  }
  $: missingFont = sel && sel.font && !fontInstalled(sel.font) ? sel.font : null;

  const isColor = (v) => typeof v === 'string' && v.startsWith('#');

  // ── Where this template shows (WYSIWYG assignment) ─────────────────────────
  // The single most confusing thing about the old editor: you could restyle a
  // template all day and the wall never changed, because the output resolves its
  // look from the CHANNEL's assigned template or the content-type default — not
  // from "whatever template happens to be open in the editor". This panel closes
  // that loop: it shows which screens and which content roles currently render
  // THIS template, and lets the operator point them here in one click. Editing
  // the template that a screen/role actually uses is then obvious, so the editor
  // preview and the live output are the same thing by construction.
  const ROLES = [
    ['scripture', 'Scripture'],
    ['song', 'Song / Lyrics'],
    ['media', 'Media'],
    ['announce', 'Announcement'],
  ];
  let assignChannels = [];
  let contentMap = { scripture: null, song: null, media: null, announce: null };
  let assignBusy = false;
  $: tid = edit?.id ?? null;
  async function loadAssign() {
    try { assignChannels = (await listOutputChannels()) || []; } catch { assignChannels = []; }
    try { contentMap = (await getContentTemplates()) || contentMap; } catch { /* keep */ }
  }
  async function toggleRole(kind) {
    if (!tid || assignBusy) return;
    assignBusy = true;
    try {
      await setContentTemplate(kind, contentMap[kind] === tid ? null : tid);
      contentMap = (await getContentTemplates()) || contentMap;
    } catch { /* surfaced elsewhere */ }
    assignBusy = false;
  }
  async function assignToChannel(cid) {
    if (!tid || assignBusy) return;
    assignBusy = true;
    try {
      await setChannelTemplate(cid, tid);
      assignChannels = (await listOutputChannels()) || assignChannels;
    } catch { /* surfaced elsewhere */ }
    assignBusy = false;
  }

  // ── Live apply (debounced save + push) ─────────────────────────────────────
  let lastSig = '';
  let liveTimer;
  const sigOf = (t) => JSON.stringify({ name: t?.name, layout: t?.layout, style: t?.style });
  $: if (edit) scheduleLive(sigOf(edit));
  function scheduleLive(sig) {
    if (sig === lastSig) return;
    lastSig = sig;
    clearTimeout(liveTimer);
    liveTimer = setTimeout(applyLive, 400);
  }
  async function applyLive() {
    if (!edit || !$capture.available) return;
    saving = true;
    try {
      const id = await saveTemplate(edit);
      if (edit && !edit.id && id) edit.id = id;
      savedTick = true;
      setTimeout(() => (savedTick = false), 1400);
      err = '';
    } catch (e) { err = 'Live update failed: ' + e; }
    saving = false;
  }
  function saveNow() { clearTimeout(liveTimer); applyLive(); }
</script>

<div class="te-shell">
  <header class="te-top">
    <button class="r-btn ghost sm" on:click={() => dispatch('back')}>
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
      Back to Templates
    </button>
    {#if edit}<span class="te-name">{edit.name}</span><span class="te-sub r-mono">{layered ? layers.length + ' layers' : 'legacy'} · 1920×1080</span>{/if}
    <span class="te-spring"></span>
    <div class="te-zoom">
      <button class="te-zbtn" on:click={() => (zoomIdx = Math.max(0, zoomIdx - 1))} disabled={zoomIdx === 0} aria-label="Zoom out">−</button>
      <span class="te-pct r-mono">{zoom}%</span>
      <button class="te-zbtn" on:click={() => (zoomIdx = Math.min(ZOOMS.length - 1, zoomIdx + 1))} disabled={zoomIdx === ZOOMS.length - 1} aria-label="Zoom in">+</button>
    </div>
    <button class="r-btn ghost sm" class:on={previewMode} on:click={() => (previewMode = !previewMode)}>{previewMode ? 'Editing' : 'Preview'}</button>
    <button class="r-btn confirm sm" on:click={saveNow} disabled={saving || !$capture.available || !edit} title="Edits also apply to live outputs automatically">
      {saving ? 'Saving…' : savedTick ? 'Saved · live ✓' : 'Save Template'}
    </button>
  </header>

  {#if !edit}
    <div class="te-missing r-empty">This template could not be loaded.</div>
  {:else if !layered}
    <!-- Legacy region template — offer conversion. -->
    <div class="te-legacy">
      <div class="te-legacycard">
        <h2>This is a classic template</h2>
        <p>It renders with fixed regions. Convert it to editable layers to move, restyle and add pieces freely — the look is preserved, and other templates are untouched.</p>
        <div class="te-legacyprev"><TemplateRender template={edit} content={SAMPLE} /></div>
        <button class="r-btn primary" on:click={convertToLayers}>Convert to layers</button>
      </div>
    </div>
  {:else}
    <div class="te-body">
      <!-- ══ LAYERS ══ -->
      <aside class="te-pane te-layers">
        <div class="te-panehead">
          <span class="r-lbl">Layers</span>
          <div class="te-addwrap">
            <button class="te-addbtn" on:click|stopPropagation={() => (addOpen = !addOpen)} aria-label="Add layer">＋</button>
            {#if addOpen}
              <div class="te-addmenu" on:click|stopPropagation role="menu" tabindex="-1">
                <div class="te-addsec r-lbl">Add layer</div>
                {#each LAYER_TYPES as t}
                  <button on:click={() => addLayer(t.type)}><span class="te-addico">{t.icon}</span>{t.label}</button>
                {/each}
                <div class="te-addsec r-lbl">Bound text</div>
                {#each BINDINGS.filter((b) => b.key !== 'static') as b}
                  <button on:click={() => addBoundText(b.key)}><span class="te-addico">T</span>{b.label}</button>
                {/each}
              </div>
            {/if}
          </div>
        </div>
        <div class="te-layerlist r-scroll">
          {#each panelLayers as L (L.id)}
            <div class="te-layer" class:sel={selId === L.id} class:off={L.visible === false}
              on:click={() => (selId = L.id)} role="button" tabindex="0"
              on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = L.id; } }}>
              <span class="te-ltype" aria-hidden="true">{L.type === 'background' ? '▦' : L.type === 'shape' ? '▢' : L.type === 'media' ? '▷' : 'T'}</span>
              <span class="te-lname">{layerLabel(L)}</span>
              <span class="te-lbtns">
                <button class="te-lmini" title="Forward" on:click|stopPropagation={() => moveLayer(L.id, 1)}>↑</button>
                <button class="te-lmini" title="Back" on:click|stopPropagation={() => moveLayer(L.id, -1)}>↓</button>
                <button class="te-lmini" title={L.locked ? 'Unlock' : 'Lock'} class:on={L.locked} on:click|stopPropagation={() => toggleLock(L.id)}>{L.locked ? '🔒' : '🔓'}</button>
                <button class="te-lmini" title="Visibility" on:click|stopPropagation={() => toggleVisible(L.id)}>{L.visible === false ? '◌' : '●'}</button>
                <button class="te-lmini danger" title="Delete" on:click|stopPropagation={() => removeLayer(L.id)}>✕</button>
              </span>
            </div>
          {/each}
          {#if !layers.length}<div class="te-hint r-mono">No layers — use ＋ to add one.</div>{/if}
        </div>
        <p class="te-panenote">Top of the list is the front. Drag layers on the canvas to move them.</p>
      </aside>

      <!-- ══ CANVAS ══ -->
      <section class="te-canvas">
        <div class="te-stage">
          <div class="te-board-wrap" style="width:{zoom}%">
            {#if !previewMode}
              <div class="te-ruler te-ruler-x">{#each Array(11) as _, i}<span style="left:{i * 10}%">{i * 10}</span>{/each}</div>
              <div class="te-ruler te-ruler-y">{#each Array(11) as _, i}<span style="top:{i * 10}%">{i * 10}</span>{/each}</div>
            {/if}
          <div class="te-artboard" bind:this={boardEl}>
            {#if !previewMode}<div class="te-checker"></div>{/if}
            <TemplateRender template={edit} content={previewContent} />
            {#if !previewMode}
              <!-- Selection / drag overlay: one handle box per positioned layer. -->
              <div class="te-overlay">
                <!-- Alignment guides — flash while a layer snaps to centre / edge. -->
                {#if guides.v != null}<div class="te-guide te-guide-v" style="left:{guides.v}%"></div>{/if}
                {#if guides.h != null}<div class="te-guide te-guide-h" style="top:{guides.h}%"></div>{/if}
                {#each layers as L (L.id)}
                  {#if L.visible !== false && L.type !== 'background'}
                    <div class="te-hbox" class:sel={selId === L.id} class:locked={L.locked}
                      style="left:{L.x}%; top:{L.y}%; width:{L.w}%; height:{L.h}%;"
                      on:pointerdown={(e) => startDrag(e, L, 'move')} role="button" tabindex="0"
                      on:keydown={onCanvasKey} aria-label={layerLabel(L)}>
                      {#if selId === L.id}
                        <span class="te-htag">{layerLabel(L)}{#if L.locked} 🔒{/if}</span>
                        {#if !L.locked}
                          {#each HANDLES as h}
                            <span class="te-hh te-hh-{h}" on:pointerdown={(e) => startDrag(e, L, h)} role="button" tabindex="-1" aria-label="Resize {h}"></span>
                          {/each}
                        {/if}
                      {/if}
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}
          </div>
          </div>
        </div>
        <footer class="te-botbar">
          <span class="r-lbl">Canvas</span>
          <span class="te-botnote">{transparentBg ? 'Transparent — keys out in OBS / ATEM' : 'Opaque background'}</span>
          <span class="te-spring"></span>
          {#if sel}<span class="te-botchip r-mono">{Math.round(sel.x)},{Math.round(sel.y)} · {Math.round(sel.w)}×{Math.round(sel.h)}</span>{/if}
        </footer>
      </section>

      <!-- ══ PROPERTIES ══ -->
      <aside class="te-pane te-design">
        <div class="te-panehead"><span class="r-lbl">Design</span><span class="te-designfor r-mono">{sel ? layerLabel(sel) : ''}</span></div>
        <div class="te-designbody r-scroll">
          <h3 class="te-sec">Template</h3>
          <div class="te-frow"><label class="te-fk" for="te-name">Name</label><input id="te-name" class="r-input te-fv" bind:value={edit.name} /></div>
          <!-- PER-SCREEN CONTENT VISIBILITY. Tick the kinds this screen shows. An
               online wall shows everything; a stage / confidence monitor might show
               only scripture, songs and the timer — when a picture or announcement
               fires, this screen ignores it and holds what it had. -->
          <span class="r-lbl te-showlbl">Shows on this screen</span>
          <div class="te-showgrid">
            {#each CONTENT_KINDS as k}
              <button class="te-showchip" class:on={templateShows(edit, k.key)} on:click={() => toggleShows(k.key)}>
                <span class="te-showtick" aria-hidden="true">{templateShows(edit, k.key) ? '✓' : ''}</span>{k.label}
              </button>
            {/each}
          </div>

          <!-- ══ WHERE THIS SHOWS ══ point real screens / content roles at this
               template so editing here changes the wall. -->
          <h3 class="te-sec">Where this shows</h3>
          {#if !tid}
            <p class="te-fnote">Saving… once saved you can point screens and content types at this template.</p>
          {:else}
            <div class="te-asblock">
              <span class="r-lbl te-assublbl">Content types</span>
              <div class="te-aschips">
                {#each ROLES as [kind, label]}
                  <button class="te-aschip" class:on={contentMap[kind] === tid} disabled={assignBusy}
                    on:click={() => toggleRole(kind)}
                    title={contentMap[kind] === tid ? `${label} uses this template — click to unset` : `Use this template for ${label}`}>
                    {label}{#if contentMap[kind] === tid}<span class="te-astick">✓</span>{/if}
                  </button>
                {/each}
              </div>
              <p class="te-fnote">A content type set here overrides each screen's own template when that kind of content fires — how scripture can look like scripture and lyrics like lyrics.</p>
            </div>
            {#if assignChannels.length}
              <div class="te-asblock">
                <span class="r-lbl te-assublbl">Screens</span>
                <div class="te-aslist">
                  {#each assignChannels as c (c.id)}
                    <button class="te-asrow" class:on={c.template_id === tid} disabled={assignBusy}
                      on:click={() => assignToChannel(c.id)}
                      title={c.template_id === tid ? 'This screen already uses this template' : 'Point this screen at this template'}>
                      <span class="te-asname">{c.name}</span>
                      <span class="te-asstate r-mono">{c.template_id === tid ? 'live ✓' : 'assign'}</span>
                    </button>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}

          {#if !sel}
            <p class="te-guide">Select a layer to edit it, or add one with ＋.</p>
          {:else if sel.type === 'background'}
            <h3 class="te-sec">Background</h3>
            <div class="te-frow">
              <label class="te-fk" for="te-fill">Fill</label>
              <span class="te-fv te-swatch"><input id="te-fill" type="color" value={isColor(sel.fill) ? sel.fill : '#0b0906'} on:input={(e) => { set('fill', e.target.value); set('image', null); }} /><span class="te-hex r-mono">{isColor(sel.fill) ? sel.fill.toUpperCase() : 'gradient'}</span></span>
            </div>
            <div class="te-frow">
              <label class="te-fk" for="te-op">Opacity</label>
              <span class="te-fv te-rangerow"><input id="te-op" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span>
            </div>
            <div class="te-frow">
              <label class="te-fk" for="te-dim">Dim</label>
              <span class="te-fv te-rangerow"><input id="te-dim" class="r-range" type="range" min="0" max="0.9" step="0.05" value={sel.dim || 0} on:input={(e) => num('dim', e.target.value)} /><span class="te-rnum r-mono">{Math.round((sel.dim || 0) * 100)}%</span></span>
            </div>
            <p class="te-fnote">Dim lays black over the background so text stays readable on bright images.</p>
            <div class="r-lbl te-sublbl">Image library</div>
            {#if BACKGROUNDS.length}
              <div class="te-bglib">
                {#if sel.image}<button class="te-bgtile te-bgnone" on:click={() => set('image', null)} title="No image">✕</button>{/if}
                {#each BACKGROUNDS as b (b.file)}
                  <button class="te-bgtile" class:on={sel.image === b.url} title={b.name} style="background-image:url({b.url});" aria-label={b.name} on:click={() => set('image', b.url)}></button>
                {/each}
              </div>
            {:else}
              <p class="te-fnote">Drop images into <code>src/backgrounds/</code> to fill this.</p>
            {/if}
          {:else if sel.type === 'media'}
            <h3 class="te-sec">Media</h3>
            <div class="te-frow">
              <span class="te-fk">Fit</span>
              <span class="te-fv te-seg">
                <button class:on={(sel.fit || 'cover') === 'cover'} on:click={() => set('fit', 'cover')}>Cover</button>
                <button class:on={sel.fit === 'contain'} on:click={() => set('fit', 'contain')}>Contain</button>
              </span>
            </div>
            <div class="te-frow"><label class="te-fk" for="te-mop">Opacity</label><span class="te-fv te-rangerow"><input id="te-mop" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-mrad">Radius</label><span class="te-fv te-rangerow"><input id="te-mrad" class="r-range" type="range" min="0" max="8" step="0.2" value={sel.radius || 0} on:input={(e) => num('radius', e.target.value)} /><span class="te-rnum r-mono">{(sel.radius || 0).toFixed(1)}</span></span></div>
            <p class="te-fnote">Shows the fired picture or video. Empty until media is on screen — a template without a Media layer never shows media on that screen. Put it high in the layer list to cover everything, or low to sit behind the text.</p>
          {:else if sel.type === 'shape'}
            <h3 class="te-sec">Shape</h3>
            <div class="te-frow"><label class="te-fk" for="te-sfill">Fill</label><span class="te-fv te-swatch"><input id="te-sfill" type="color" value={isColor(sel.fill) ? sel.fill : '#101319'} on:input={(e) => set('fill', e.target.value)} /><span class="te-hex r-mono">{isColor(sel.fill) ? sel.fill.toUpperCase() : '#101319'}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-sop">Opacity</label><span class="te-fv te-rangerow"><input id="te-sop" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-srad">Radius</label><span class="te-fv te-rangerow"><input id="te-srad" class="r-range" type="range" min="0" max="8" step="0.2" value={sel.radius || 0} on:input={(e) => num('radius', e.target.value)} /><span class="te-rnum r-mono">{(sel.radius || 0).toFixed(1)}</span></span></div>
          {:else}
            <!-- text / timer -->
            <h3 class="te-sec">Text</h3>
            <div class="te-frow">
              <label class="te-fk" for="te-bind">Content</label>
              <select id="te-bind" class="r-select te-fv" value={sel.bind} on:change={(e) => set('bind', e.target.value)}>
                {#each BINDINGS as b}<option value={b.key}>{b.label}</option>{/each}
              </select>
            </div>
            {#if sel.bind === 'static'}
              <div class="te-frow"><label class="te-fk" for="te-txt">Text</label><input id="te-txt" class="r-input te-fv" value={sel.text || ''} on:input={(e) => set('text', e.target.value)} /></div>
            {/if}
            <div class="te-frow">
              <label class="te-fk" for="te-font">Font</label>
              <select id="te-font" class="r-select te-fv" value={sel.font} on:change={(e) => set('font', e.target.value)}>
                {#if sel.font && !fonts.includes(sel.font)}<option value={sel.font}>{fontLabel(sel.font)}</option>{/if}
                {#each fonts as f}<option value={f}>{f}</option>{/each}
              </select>
            </div>
            <button class="te-minilink" on:click={() => detectFonts(false)}>Use all computer fonts {fontMsg}</button>
            {#if missingFont}<p class="te-fwarn">“{fontLabel(missingFont)}” isn't installed here — outputs use a default. Install it to use it.</p>{/if}
            <div class="te-frow"><label class="te-fk" for="te-size">Size</label><span class="te-fv te-stepper"><input id="te-size" class="te-num r-mono" type="number" min="1" max="16" step="0.1" value={sel.size} on:input={(e) => num('size', e.target.value)} /><span class="te-unit r-mono">cqw</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-col">Colour</label><span class="te-fv te-swatch"><input id="te-col" type="color" value={isColor(sel.color) ? sel.color : '#ffffff'} on:input={(e) => set('color', e.target.value)} /><span class="te-hex r-mono">{isColor(sel.color) ? sel.color.toUpperCase() : '#FFFFFF'}</span></span></div>
            <div class="te-frow">
              <span class="te-fk">Align</span>
              <span class="te-fv te-seg">
                {#each ['left', 'center', 'right'] as a}
                  <button class:on={sel.align === a} aria-label="Align {a}" on:click={() => set('align', a)}>
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 6h16"/><path d={a === 'left' ? 'M4 12h10' : a === 'right' ? 'M10 12h10' : 'M7 12h10'}/><path d="M4 18h16"/></svg>
                  </button>
                {/each}
              </span>
            </div>
            <div class="te-frow">
              <span class="te-fk">V-align</span>
              <span class="te-fv te-seg">
                {#each ['top', 'middle', 'bottom'] as v}
                  <button class:on={(sel.valign || 'middle') === v} on:click={() => set('valign', v)}>{v[0].toUpperCase()}</button>
                {/each}
              </span>
            </div>
            <div class="te-frow">
              <label class="te-fk" for="te-case">Caps</label>
              <select id="te-case" class="r-select te-fv" value={sel.transform || 'none'} on:change={(e) => set('transform', e.target.value)}>
                <option value="none">As typed</option><option value="uppercase">UPPERCASE</option><option value="lowercase">lowercase</option><option value="capitalize">Capitalize</option>
              </select>
            </div>
            <div class="te-frow"><label class="te-fk" for="te-lh">Line height</label><span class="te-fv te-rangerow"><input id="te-lh" class="r-range" type="range" min="0.9" max="2" step="0.05" value={sel.lineHeight || 1.32} on:input={(e) => num('lineHeight', e.target.value)} /><span class="te-rnum r-mono">{(sel.lineHeight || 1.32).toFixed(2)}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-ls">Spacing</label><span class="te-fv te-rangerow"><input id="te-ls" class="r-range" type="range" min="-0.05" max="0.4" step="0.01" value={sel.letterSpacing || 0} on:input={(e) => num('letterSpacing', e.target.value)} /><span class="te-rnum r-mono">{(sel.letterSpacing || 0).toFixed(2)}em</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-sh">Shadow</label><span class="te-fv te-rangerow"><input id="te-sh" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.shadow || 0} on:input={(e) => num('shadow', e.target.value)} /><span class="te-rnum r-mono">{Math.round((sel.shadow || 0) * 100)}%</span></span></div>
            <div class="te-frow">
              <label class="te-fk" for="te-fit">Scale</label>
              <select id="te-fit" class="r-select te-fv" value={sel.fit || 'both'} on:change={(e) => set('fit', e.target.value)}>
                <option value="both">Up or down (fit box)</option>
                <option value="shrink">Shrink to fit only</option>
                <option value="none">Fixed size</option>
              </select>
            </div>
            <div class="te-frow">
              <label class="te-fk" for="te-lt">Line transform</label>
              <select id="te-lt" class="r-select te-fv" value={sel.lineTransform || 'none'} on:change={(e) => set('lineTransform', e.target.value)}>
                <option value="none">None</option>
                <option value="remove-returns">Remove line returns</option>
                <option value="replace-returns">Replace line returns</option>
                <option value="one-word-per-line">One word per line</option>
                <option value="one-char-per-line">One character per line</option>
              </select>
            </div>
            <button class="te-swrow" on:click={() => set('italic', !sel.italic)}><span>Italic</span><span class="r-switch" class:on={sel.italic}></span></button>
            <button class="te-swrow" on:click={() => set('scroll', !sel.scroll)}><span>Scroll (ticker)</span><span class="r-switch" class:on={sel.scroll}></span></button>
          {/if}

          {#if sel && sel.type !== 'background'}
            <h3 class="te-sec">Position</h3>
            <div class="te-geo">
              <label>X<input class="te-num r-mono" type="number" min="0" max="100" value={Math.round(sel.x)} on:input={(e) => num('x', e.target.value)} /></label>
              <label>Y<input class="te-num r-mono" type="number" min="0" max="100" value={Math.round(sel.y)} on:input={(e) => num('y', e.target.value)} /></label>
              <label>W<input class="te-num r-mono" type="number" min="2" max="100" value={Math.round(sel.w)} on:input={(e) => num('w', e.target.value)} /></label>
              <label>H<input class="te-num r-mono" type="number" min="2" max="100" value={Math.round(sel.h)} on:input={(e) => num('h', e.target.value)} /></label>
            </div>
            <div class="te-alignrow">
              <button class="te-alignbtn" on:click={() => center('x')} title="Centre horizontally">Centre H</button>
              <button class="te-alignbtn" on:click={() => center('y')} title="Centre vertically">Centre V</button>
              <button class="te-alignbtn" on:click={() => center('both')} title="Centre on canvas">Centre</button>
            </div>
            <p class="te-fnote">Percent of the screen. Drag on the canvas — layers snap to centre and edges (hold Shift to place freely) — or type exact values.</p>
          {/if}
        </div>
        {#if err}<div class="te-err" role="alert">{err}</div>{/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  .te-shell{ display:flex; flex-direction:column; height:100%; min-height:0; gap:12px; }
  .te-spring{ flex:1; }
  .te-top{ display:flex; align-items:center; gap:10px; flex:0 0 auto; }
  .te-name{ font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .te-sub{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-zoom{ display:flex; align-items:center; gap:4px; }
  .te-zbtn{ width:26px; height:26px; border-radius:var(--v-r-sm); background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-dim); cursor:pointer; font-size:15px; line-height:1; }
  .te-zbtn:disabled{ opacity:.4; cursor:not-allowed; }
  .te-pct{ min-width:42px; text-align:center; font-size:var(--v-fs-cap); color:var(--v-dim); }
  .r-btn.confirm{ background:var(--v-emerald); color:var(--v-void); border-color:transparent; }
  .r-btn.confirm:hover:not(:disabled){ filter:brightness(1.08); }
  .r-btn.ghost.on{ background:var(--v-surf3); color:var(--v-txt); border-color:var(--v-line2); }

  .te-body{ flex:1; min-height:0; display:grid; grid-template-columns:222px minmax(0,1fr) 300px; gap:12px; }
  @media (max-width:1180px){ .te-body{ grid-template-columns:186px minmax(0,1fr) 268px; } }
  @media (max-width:980px){ .te-shell{ height:auto; } .te-body{ grid-template-columns:1fr; } }

  .te-pane{ display:flex; flex-direction:column; min-height:0; overflow:hidden; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  .te-panehead{ display:flex; align-items:center; justify-content:space-between; gap:8px; padding:11px 13px; border-bottom:1px solid var(--v-line); flex:0 0 auto; }
  .te-designfor{ font-size:var(--v-fs-cap); color:var(--v-accent2); }

  /* layers panel */
  .te-addwrap{ position:relative; }
  .te-addbtn{ width:24px; height:24px; border-radius:var(--v-r-sm); background:var(--v-accent-fill); color:#fff; border:0; cursor:pointer; font-size:15px; line-height:1; }
  .te-addmenu{ position:absolute; top:28px; right:0; z-index:30; width:186px; background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:var(--v-r-md); box-shadow:var(--v-shadow-lg); padding:5px; display:flex; flex-direction:column; }
  .te-addmenu button{ display:flex; align-items:center; gap:9px; text-align:left; padding:7px 9px; border:0; background:none; color:var(--v-txt); font-size:var(--v-fs-b2); border-radius:var(--v-r-sm); cursor:pointer; }
  .te-addmenu button:hover{ background:var(--v-surf3); }
  .te-addico{ width:16px; text-align:center; color:var(--v-faint); font-family:var(--f-mono); }
  .te-addsec{ padding:6px 8px 3px; }

  .te-layerlist{ flex:1; min-height:0; overflow-y:auto; padding:8px; display:flex; flex-direction:column; gap:4px; }
  .te-layer{ display:flex; align-items:center; gap:8px; padding:8px 9px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line); cursor:pointer; transition:.12s; }
  .te-layer:hover{ border-color:var(--v-line2); }
  .te-layer.sel{ border-color:var(--v-accent-line); background:var(--v-accent-soft); }
  .te-layer.off{ opacity:.5; }
  .te-ltype{ width:16px; text-align:center; color:var(--v-faint); font-family:var(--f-mono); font-size:11px; flex:0 0 auto; }
  .te-lname{ flex:1; min-width:0; font-size:var(--v-fs-b2); color:var(--v-txt); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .te-lbtns{ display:flex; gap:1px; flex:0 0 auto; opacity:0; transition:opacity .12s; }
  .te-layer:hover .te-lbtns, .te-layer.sel .te-lbtns{ opacity:1; }
  .te-lmini{ width:20px; height:20px; display:grid; place-items:center; border:0; background:none; color:var(--v-faint); cursor:pointer; border-radius:var(--v-r-sm); font-size:11px; }
  .te-lmini:hover{ color:var(--v-txt); background:var(--v-surf3); }
  .te-lmini.danger:hover{ color:var(--v-rose); }
  /* A locked layer's button stays lit even at rest, so the lock state reads at a
     glance without hovering the row. */
  .te-lmini.on{ color:var(--v-amber); opacity:1; }
  .te-layer .te-lbtns:has(.te-lmini.on){ opacity:1; }
  .te-hint{ padding:14px 8px; text-align:center; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-panenote{ margin:0; padding:10px 12px; border-top:1px solid var(--v-line); flex:0 0 auto; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }

  /* canvas */
  .te-canvas{ display:flex; flex-direction:column; min-height:0; overflow:hidden; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  /* ProPresenter-clean canvas: a flat, calm dark stage with a soft vignette for
     depth — no busy grid competing with the artboard. */
  .te-stage{ flex:1; min-height:0; display:flex; align-items:center; justify-content:center; padding:var(--v-sp-lg); overflow:auto; position:relative; background:#141417; }
  .te-stage::before{ content:""; position:absolute; inset:0; pointer-events:none; background:radial-gradient(130% 110% at 50% 32%, transparent 45%, rgba(0,0,0,.45) 100%); }
  /* board wrapper carries the rulers; the artboard sits inside, offset for them. */
  .te-board-wrap{ position:relative; max-width:100%; padding:18px 0 0 26px; flex:0 0 auto; z-index:1; }
  .te-ruler{ position:absolute; color:var(--v-faint); font-family:var(--f-mono); font-size:7px; pointer-events:none; }
  .te-ruler-x{ top:2px; left:26px; right:0; height:14px; border-bottom:1px solid var(--v-line2); }
  .te-ruler-x span{ position:absolute; transform:translateX(1px); }
  .te-ruler-x span::before{ content:""; position:absolute; left:0; bottom:-4px; width:1px; height:4px; background:var(--v-line2); }
  .te-ruler-y{ top:18px; left:2px; bottom:0; width:20px; border-right:1px solid var(--v-line2); }
  .te-ruler-y span{ position:absolute; right:3px; transform:translateY(-3px); }
  .te-ruler-y span::before{ content:""; position:absolute; right:-3px; top:4px; height:1px; width:4px; background:var(--v-line2); }
  /* The artboard floats on the canvas with a soft drop shadow and a hairline edge
     — clean, no heavy border. */
  .te-artboard{ position:relative; aspect-ratio:16/9; width:100%; border-radius:var(--v-r-md); overflow:hidden; box-shadow:0 24px 60px -22px rgba(0,0,0,.75), 0 0 0 1px rgba(255,255,255,.07); z-index:1; }
  /* Transparency is shown as a subtle, neutral checker (like ProPresenter) so a
     keyed template reads as transparent without shouting. */
  .te-checker{ position:absolute; inset:0; z-index:0; background:repeating-conic-gradient(#26262b 0% 25%,#1d1d21 0% 50%) 50% / 16px 16px; }
  .te-overlay{ position:absolute; inset:0; z-index:5; }
  /* Alignment guides — a bright hairline where a layer snapped to centre/edge. */
  .te-guide{ position:absolute; z-index:6; pointer-events:none; background:var(--v-accent); box-shadow:0 0 5px var(--v-accent); }
  .te-guide-v{ top:0; bottom:0; width:1px; margin-left:-0.5px; }
  .te-guide-h{ left:0; right:0; height:1px; margin-top:-0.5px; }
  .te-hbox{ position:absolute; box-sizing:border-box; border:1px dashed rgba(255,255,255,.28); cursor:move; }
  .te-hbox:hover{ border-color:rgba(255,255,255,.5); }
  /* Selected box sits ABOVE the others so an overlapping layer above it can't
     intercept the drag — you can always move the selected layer, even under one. */
  .te-hbox.sel{ border:1px solid var(--v-accent); box-shadow:0 0 0 1px var(--v-accent); z-index:10; }
  /* A locked box is inert AND click-through, so it never moves and never blocks a
     layer beneath it (select that one and drag it right under the locked one). */
  .te-hbox.locked{ pointer-events:none; border-style:dotted; border-color:rgba(255,196,0,.5); cursor:default; }
  .te-htag{ position:absolute; top:-16px; left:0; font-family:var(--f-mono); font-size:8px; letter-spacing:.04em; color:#fff; background:var(--v-accent-fill); padding:1px 5px; border-radius:3px; white-space:nowrap; }
  /* Eight resize handles — one on every corner and edge. */
  .te-hh{ position:absolute; width:10px; height:10px; background:var(--v-accent); border:2px solid #fff; border-radius:2px; box-sizing:border-box; }
  .te-hh-nw{ left:-5px; top:-5px; cursor:nwse-resize; }
  .te-hh-n{ left:50%; top:-5px; margin-left:-5px; cursor:ns-resize; }
  .te-hh-ne{ right:-5px; top:-5px; cursor:nesw-resize; }
  .te-hh-e{ right:-5px; top:50%; margin-top:-5px; cursor:ew-resize; }
  .te-hh-se{ right:-5px; bottom:-5px; cursor:nwse-resize; }
  .te-hh-s{ left:50%; bottom:-5px; margin-left:-5px; cursor:ns-resize; }
  .te-hh-sw{ left:-5px; bottom:-5px; cursor:nesw-resize; }
  .te-hh-w{ left:-5px; top:50%; margin-top:-5px; cursor:ew-resize; }
  .te-botbar{ flex:0 0 auto; display:flex; align-items:center; gap:var(--v-sp-sm); padding:10px 12px; border-top:1px solid var(--v-line); }
  .te-botnote{ font-size:var(--v-fs-cap); color:var(--v-faint); white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
  .te-botchip{ padding:5px 10px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line2); font-size:var(--v-fs-cap); color:var(--v-dim); }

  /* design panel */
  .te-designbody{ flex:1; min-height:0; overflow-y:auto; padding:14px; display:flex; flex-direction:column; gap:10px; }
  .te-sec{ margin:8px 0 2px; font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .te-sec:first-child{ margin-top:0; }
  .te-frow{ display:grid; grid-template-columns:64px minmax(0,1fr); align-items:center; gap:10px; }
  .te-fk{ font-size:var(--v-fs-b2); color:var(--v-dim); }
  .te-fv{ min-width:0; }
  .te-fnote, .te-guide{ font-size:var(--v-fs-cap); color:var(--v-faint); margin:0; line-height:1.5; }
  .te-minilink{ background:none; border:0; padding:0; text-align:left; color:var(--v-cyan); font-family:var(--f-mono); font-size:9px; cursor:pointer; letter-spacing:.04em; }
  .te-fwarn{ margin:0; padding:8px 10px; border:1px solid var(--v-amber-soft); border-radius:var(--v-r-sm); background:var(--v-amber-soft); color:var(--v-amber2); font-size:var(--v-fs-cap); line-height:1.45; }
  .te-stepper{ display:flex; align-items:center; }
  .te-num{ height:32px; padding:0 8px; border-radius:var(--v-r-md); background:var(--v-bg); border:1px solid var(--v-line2); color:var(--v-txt); font-size:var(--v-fs-b2); outline:none; width:100%; box-sizing:border-box; }
  .te-num:focus{ border-color:var(--v-accent-line); }
  .te-stepper .te-num{ border-radius:var(--v-r-md) 0 0 var(--v-r-md); border-right:0; }
  .te-unit{ flex:0 0 auto; height:32px; display:grid; place-items:center; padding:0 9px; border-radius:0 var(--v-r-md) var(--v-r-md) 0; background:var(--v-surf2); border:1px solid var(--v-line2); font-size:9px; color:var(--v-faint); }
  .te-rangerow{ display:flex; align-items:center; gap:9px; }
  .te-rangerow .r-range{ flex:1; min-width:0; }
  .te-rnum{ flex:0 0 auto; min-width:40px; text-align:right; font-size:var(--v-fs-cap); color:var(--v-dim); }
  .te-swatch{ display:flex; align-items:center; gap:9px; }
  .te-swatch input[type=color]{ width:32px; height:32px; flex:0 0 auto; border:1px solid var(--v-line2); border-radius:var(--v-r-md); background:var(--v-bg); cursor:pointer; padding:3px; }
  .te-hex{ font-size:var(--v-fs-cap); color:var(--v-dim); text-transform:uppercase; }
  .te-seg{ display:flex; gap:2px; background:var(--v-bg); border:1px solid var(--v-line); border-radius:var(--v-r-md); padding:3px; }
  .te-seg button{ flex:1; height:26px; display:grid; place-items:center; border:0; border-radius:var(--v-r-sm); background:none; color:var(--v-dim); cursor:pointer; font-size:var(--v-fs-cap); }
  .te-seg button:hover{ color:var(--v-txt); }
  .te-seg button.on{ background:var(--v-surf3); color:var(--v-txt); }
  .te-swrow{ display:flex; align-items:center; justify-content:space-between; width:100%; background:var(--v-surf2); border:1px solid var(--v-line); border-radius:var(--v-r-md); padding:9px 12px; color:var(--v-txt); font-size:var(--v-fs-b2); cursor:pointer; }
  .te-swrow:hover{ border-color:var(--v-line2); }
  .te-sublbl{ margin:10px 0 6px; }
  .te-bglib{ display:grid; grid-template-columns:repeat(3, 1fr); gap:6px; max-height:196px; overflow-y:auto; padding-right:4px; scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent; }
  .te-bgtile{ aspect-ratio:16/9; border-radius:var(--v-r-sm); border:1px solid var(--v-line2); background-size:cover; background-position:center; cursor:pointer; padding:0; }
  .te-bgtile.on{ border-color:var(--v-accent); box-shadow:0 0 0 1px var(--v-accent); }
  .te-bgnone{ display:grid; place-items:center; background:var(--v-surf2); color:var(--v-faint); font-size:13px; }
  .te-bgnone:hover{ color:var(--v-rose); }
  /* Where-this-shows assignment */
  .te-asblock{ display:flex; flex-direction:column; gap:7px; }
  .te-assublbl{ margin-top:2px; }
  /* Per-screen content visibility chips */
  .te-showlbl{ margin-top:4px; }
  .te-showgrid{ display:flex; flex-wrap:wrap; gap:6px; }
  .te-showchip{ display:inline-flex; align-items:center; gap:5px; padding:6px 10px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-faint); font-size:var(--v-fs-cap); cursor:pointer; }
  .te-showchip:hover{ color:var(--v-txt); border-color:var(--v-accent-line); }
  .te-showchip.on{ background:var(--v-accent-soft); border-color:var(--v-accent-line); color:var(--v-txt); }
  .te-showtick{ width:9px; text-align:center; color:var(--v-emerald); font-weight:700; }
  .te-aschips{ display:flex; flex-wrap:wrap; gap:6px; }
  .te-aschip{ display:inline-flex; align-items:center; gap:5px; padding:6px 10px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-dim); font-size:var(--v-fs-cap); cursor:pointer; }
  .te-aschip:hover:not(:disabled){ color:var(--v-txt); border-color:var(--v-accent-line); }
  .te-aschip.on{ background:var(--v-accent-soft); border-color:var(--v-accent-line); color:var(--v-txt); }
  .te-aschip:disabled{ opacity:.5; cursor:default; }
  .te-astick{ color:var(--v-emerald); font-weight:700; }
  .te-aslist{ display:flex; flex-direction:column; gap:4px; }
  .te-asrow{ display:flex; align-items:center; justify-content:space-between; gap:8px; padding:8px 10px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line); color:var(--v-txt); font-size:var(--v-fs-b2); cursor:pointer; }
  .te-asrow:hover:not(:disabled){ border-color:var(--v-accent-line); }
  .te-asrow.on{ background:var(--v-accent-soft); border-color:var(--v-accent-line); }
  .te-asrow:disabled{ opacity:.6; cursor:default; }
  .te-asname{ overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .te-asstate{ flex:0 0 auto; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-asrow.on .te-asstate{ color:var(--v-emerald); }
  .te-alignrow{ display:flex; gap:6px; }
  .te-alignbtn{ flex:1; height:30px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-dim); font-size:var(--v-fs-cap); cursor:pointer; }
  .te-alignbtn:hover{ color:var(--v-txt); border-color:var(--v-accent-line); background:var(--v-accent-soft); }
  .te-geo{ display:grid; grid-template-columns:1fr 1fr; gap:6px; }
  .te-geo label{ display:flex; align-items:center; gap:6px; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-geo .te-num{ height:28px; }
  .te-err{ flex:0 0 auto; margin:0; padding:10px 14px; border-top:1px solid var(--v-line); color:var(--v-red); font-size:var(--v-fs-cap); line-height:1.5; }
  .te-missing{ margin:auto; padding:40px; }

  /* legacy convert */
  .te-legacy{ flex:1; min-height:0; display:grid; place-items:center; padding:20px; }
  .te-legacycard{ max-width:520px; text-align:center; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); padding:24px; }
  .te-legacycard h2{ margin:0 0 8px; font-family:var(--f-head); font-size:var(--v-fs-h2); color:var(--v-txt); }
  .te-legacycard p{ margin:0 0 16px; font-size:var(--v-fs-b2); color:var(--v-dim); line-height:1.5; }
  .te-legacyprev{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md); overflow:hidden; border:1px solid var(--v-line2); background:var(--v-void); margin-bottom:16px; }
</style>
