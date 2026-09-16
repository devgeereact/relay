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
  import { rangeFill } from '../../rangefill.js';
  import {
    duplicateLayer, resetLayer, removeLayer as dropLayer, moveLayer as moveInOrder,
    alignLayer, spaceEvenly, isMovable, movableLayers,
  } from '../../layerops.js';
  import { BUILTINS } from '../../templates.js';
  import { contentTemplates, setContentTemplate, loadContentTemplates } from '../../stores/capture.js';
  import TemplateRender from '../../TemplateRender.svelte';
  import { reviewTemplate, PREVIEW_DISTANCES_M, previewScale } from '../../legibility.js';
  import TemplatePreviewOverlay from '../../TemplatePreviewOverlay.svelte';
  import { testTemplateOnOutputs } from '../../templateTest.js';
  import { humanError } from '../../errors.js';
  import {
    capture, templates, loadTemplates, saveTemplate,
    snapshotTemplateVersion, listTemplateVersions, restoreTemplateVersion,
  } from '../../stores/capture.js';
  import { isThemeToken, THEME_TOKENS, resolveTokens } from '../../styletokens.js';
  import { BACKGROUNDS } from '../../backgrounds.js';
  import {
    makeLayer, isLayered, layerLabel, regionsToLayers, templateShows, CONTENT_KINDS,
    LAYER_TYPES, BINDINGS, bandOf, bandMembers, topLevelLayers, drawBoxes, boundValue,
  } from '../../layers.js';
  import { BAND_GROW_MAX, BAND_TYPE_FLOOR } from '../../templatemodel.js';

  export let templateId;
  /** Which object to open on, if the caller knew. The gallery inspector's object
   *  strip (docs/REBRAND.md §3.2) names a template's real objects and a press
   *  lands here — so "change the reference on Nocturne" is one action rather
   *  than open-the-editor-then-find-it. `null` opens with nothing selected,
   *  exactly as before; an id that is not on this template is ignored rather
   *  than selecting the wrong object. */
  export let layerId = null;
  const dispatch = createEventDispatcher();

  let edit = null;
  let saving = false;
  let savedTick = false;
  let err = '';
  let selId = null;
  let addOpen = false;

  onMount(async () => {
    loadContentTemplates();
    if (!$templates.length) await loadTemplates();
    load(templateId);
    // Land on the object the caller named — but only if this template really has
    // it. An id from somewhere else would select nothing and leave the panel
    // showing another object's properties under that object's name.
    if (layerId && (edit?.layout?.layers ?? []).some((L) => L.id === layerId)) selId = layerId;
    detectFonts(true);
  });

  // The template WITH its layer style TOKENS resolved — what the wall will show,
  // so the editor preview is WYSIWYG. A token (`theme:accent`) resolves against
  // this template's OWN style, so a starter dropped in here wears this template's
  // colours; `styletokens.js` has the whole of it. Layers that carry no token are
  // shared by reference, so drag/selection still target `edit`.
  $: themedEdit = edit ? resolveTokens(edit) : edit;

  // ── CAN THE BACK ROW READ THIS? (RG-18) ───────────────────────────────────
  //
  // Here, in the editor, because this is where a designer works — on a Tuesday,
  // with time to change it. A warning on Sunday morning is a warning about
  // something nobody is going to fix at 10:29.
  //
  // Two of the three answers are computable and the third is not, and the third is
  // said as loudly as the other two: over a photograph or a video, Relay cannot
  // know what is behind the words, and a green tick there would be worse than no
  // tick at all.
  //
  // THE ROOM NUMBERS ARE TYPED, and they are typed every time. This block used
  // to say they "come from the active room (RG-10) when there is one, so a
  // church that told Relay about its hall once gets the distance answer for
  // free" — and nothing here reads a room, in this file or any other:
  // `rooms.js` captures the input device, the language, the target length and
  // the channels, and has never held a screen width or a back row. Both fields
  // start empty, `Number('')` is 0, and `review` correctly answers "unknown"
  // for the distance until somebody fills them in. The sentence described a
  // wiring that does not exist, which is the same defect as a control that does
  // nothing, one layer down: a reader checking whether the answer is trustworthy
  // was told where it came from, and it came from nowhere.
  let screenWidthM = '';
  let backRowM = '';
  $: room = { screenWidthM: Number(screenWidthM), backRowM: Number(backRowM) };
  /* THE WHOLE TEMPLATE, not `style`. `review` reads `style.verseColor`,
     `style.refColor`, `style.background` and `style.verseSize`; in the layer model
     those live on layers, and every shipped template leaves `style` EMPTY — all
     eight shelf entries carry `style: {}`.

     So this panel — the only tool in the product that answers "can the back row
     read this" — returned `unknown` for eleven of thirteen shipped templates,
     including all three lower thirds and both composites. Worse than inert:
     `unknown` renders in the reassuring branch, so a template with a real
     contrast failure sat in the same state as one nobody had checked.

     `reviewTemplate` adapts the layers first and is the one call a surface should
     make, so a caller cannot forget to and quietly get three `unknown`s. Nothing
     about the thresholds or the wording moved. */
  $: legible = themedEdit ? reviewTemplate(themedEdit, previewContent, room) : null;
  let showDistances = false;

  // ── AND IT IS FOLDED AWAY UNTIL IT IS ASKED FOR ────────────────────────────
  //
  // Seven paragraphs of small italic prose, three verdict rows, two number
  // fields and a button sat permanently under the layer list — the largest thing
  // in the column an operator opens to find their layers, and the least used.
  // Nothing is deleted: opening it gives back exactly what was there.
  //
  // THE ONE LINE THAT STAYS VISIBLE HAS TO BE A REAL STATUS LINE (rule 35). A
  // collapsed panel that always read "Readability" would say the same thing over
  // a template with two contrast failures as over one with none, which is the
  // defect the rule names, and would be worse than the essay it replaced. So the
  // summary is derived from the same `review` the rows are, and the three answers
  // are distinct: something to look at, something that CANNOT be checked from
  // here (a photograph behind the words — `review` counts those separately and
  // refuses to call them a pass), and a clean result.
  let legOpen = false;

  /**
   * WHAT THE RENDERER MEASURED, as opposed to what the stylesheet declared.
   *
   * Rule 37: a fit loop with no notion of failure always succeeds, so forty rounds
   * of x0.95 squeeze any verse into any box and a template that has stopped
   * working looks exactly like one that is working. `TemplateRender` reports the
   * ratio it settled at and whether it still clipped; below 45% of the size the
   * designer asked for, that is worth saying out loud.
   *
   * It belongs beside the Readability verdict rather than in a fourth panel — the
   * question is the same one ("will the back row read this?") and an operator
   * should not have to collate two answers.
   */
  let fitNote = '';
  let fitBad = false;
  function noteFit(f) {
    if (!f) return;
    if (f.clipped) {
      fitBad = true;
      fitNote = `words are being cut off at ${Math.round(f.scale * 100)}% of the designed size`;
      return;
    }
    fitBad = !f.legible;
    fitNote = f.legible ? '' : `shrunk to ${Math.round(f.scale * 100)}% to fit`;
  }

  $: legSummary = !legible
    ? ''
    : legible.problems
      ? `${legible.problems} to look at`
      : legible.unknowns
        ? `${legible.unknowns} it cannot check`
        : 'nothing flagged';
  $: legState = !legible ? '' : legible.problems ? 'bad' : legible.unknowns ? 'unknown' : 'ok';
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
    past = [];
    future = [];
  }

  $: layered = isLayered(edit);
  $: layers = layered ? edit.layout.layers : [];
  // ── THE LAYER LIST ────────────────────────────────────────────────────────
  //
  // It used to be `[...layers].reverse()` — the raw array, flattened, with a
  // band's words rendered as its SIBLINGS. That is not the shape of the model
  // and it is not what the wall draws: `topLevelLayers` is the stack, and a
  // band's words are drawn BY the band, in the order it names them, wherever
  // they happen to sit in the array. A list that shows a word beside the band
  // that owns it offers an arrow that cannot move it and hides the one fact
  // about it that matters — which band it is in.
  //
  // So: the stack, FRONT FIRST (the top of a list is the front of a stack
  // everywhere else in this product), each band followed by its own words.
  //
  // The words are NOT reversed under their band. A stack's order is what paints
  // over what; a band's order is reading order — the name above the role — and
  // reversing it would print the list backwards against the wall it describes.
  //
  // A FUNCTION, not only a reactive value, because the drag needs to ask this
  // question of a list it has not committed yet. `reorderTo` steps `moveLayer`
  // one place at a time and has to re-read the resulting order after each step;
  // asking `panelRows` would be asking about the list as it was before the
  // gesture started.
  function panelOrderOf(list) {
    const rows = [];
    for (const L of [...topLevelLayers(list)].reverse()) {
      rows.push({ L, member: false });
      if (L.type === 'band') {
        for (const m of bandMembers(list, L)) rows.push({ L: m, member: true });
      }
    }
    return rows;
  }
  $: panelRows = panelOrderOf(layers);
  $: sel = layers.find((l) => l.id === selId) || null;
  /** The icon for a kind, from the ONE register (`LAYER_TYPES`). The list drew
   *  its own four-way guess and answered `T` for a band, a region AND a timer —
   *  three different kinds wearing the text icon in the one place an operator
   *  goes to tell them apart, while the ＋ menu two rows up drew all three
   *  correctly from this table. */
  const typeIcon = (t) => LAYER_TYPES.find((x) => x.type === t)?.icon ?? 'T';

  // ── BANDS (docs/REBRAND.md §4) ──────────────────────────────────────────────
  // A band names the objects inside it, so membership is a fact about the band,
  // not about the word. `bandOf` is the one reader of that list (`layers.js`).
  $: selBand = sel ? bandOf(layers, sel.id) : null;
  $: bandWords = sel && sel.type === 'band'
    ? (Array.isArray(sel.members) ? sel.members : []).map((id) => layers.find((l) => l.id === id)).filter(Boolean)
    : [];
  $: bands = layers.filter((l) => l.type === 'band');
  // WHERE THINGS ARE ACTUALLY DRAWN. The canvas's handle boxes have to land on
  // the words, and a band's words are placed by the band — so the overlay reads
  // the same derived geometry the renderer does, rather than a second guess at
  // it (`drawBoxes`, one home).
  $: drawn = drawBoxes(layers, (L) => boundValue(L, previewContent));

  // ── THE ROW'S THUMBNAIL ────────────────────────────────────────────────────
  //
  // WHAT IT IS, said plainly because the alternative is a reader trusting it for
  // something it cannot do: a **placement proxy**, not a render. It shows where
  // this one object sits in the frame and what colour it is, live, as the drag
  // moves it. It does NOT show the words, the typeface, the fit or anything the
  // renderer decides — `TemplateRender` is the one renderer (CLAUDE.md) and a
  // 34px-wide second one drawing a rough likeness of a slide would be exactly the
  // fork that rule exists to prevent, and would be wrong in a way nobody could
  // see at that size.
  //
  // It reads the SAME two sources the canvas overlay does: `drawn` for the
  // geometry (a band and its words are placed by the band, not by x/y/w/h) and
  // the TOKEN-RESOLVED copy of the template for the colour, so a layer bound to
  // `theme:accent` shows the accent the wall would use rather than the token
  // string. Both are derived, so nothing here can disagree with the artboard.
  $: themedLayers = new Map((themedEdit?.layout?.layers ?? []).map((L) => [L.id, L]));

  // A colour Relay itself put in a template, or a colour an operator typed into
  // one. It reaches an inline `style`, so it is matched rather than trusted: a
  // hex, an rgb/hsl call, a gradient, or a bare CSS keyword. Anything else falls
  // back to a neutral, which is the honest answer for "I cannot draw this".
  const PAINTABLE = /^(#[0-9a-f]{3,8}|(?:rgb|rgba|hsl|hsla)\([^;{}]*\)|(?:linear|radial|conic)-gradient\([^;{}]*\)|[a-z]{3,20})$/i;
  const paint = (v, fallback) =>
    typeof v === 'string' && PAINTABLE.test(v.trim()) ? v.trim() : fallback;

  function thumbOf(L) {
    const b = drawn.get(L.id) || L;
    const t = themedLayers.get(L.id) || L;
    const full = L.type === 'background';
    const box = {
      x: full ? 0 : Math.max(0, Math.min(100, Number(b.x) || 0)),
      y: full ? 0 : Math.max(0, Math.min(100, Number(b.y) || 0)),
      w: full ? 100 : Math.max(2, Math.min(100, Number(b.w) || 0)),
      h: full ? 100 : Math.max(2, Math.min(100, Number(b.h) || 0)),
    };
    const words = L.type === 'text' || L.type === 'timer';
    return {
      ...box,
      words,
      // A text object's box is drawn as an outline with two bars in the ink; a
      // shape, band or background is drawn as the fill itself.
      fill: words ? 'transparent' : paint(t.fill, 'var(--v-line2)'),
      ink: paint(t.color, 'var(--v-dim)'),
      // Media and a slide region show something Relay does not have here, so
      // they read as an empty frame rather than as a solid nobody chose.
      hollow: L.type === 'media' || L.type === 'region',
      opacity: L.visible === false ? 0.25 : Math.max(0.15, Math.min(1, Number(t.opacity ?? 1))),
    };
  }

  /** Put a text object into a band, or take it out again. */
  function setBandMembership(id, bandId) {
    edit.layout.layers = layers.map((l) => {
      if (l.type !== 'band') return l;
      const members = (Array.isArray(l.members) ? l.members : []).filter((m) => m !== id);
      if (l.id === bandId) members.push(id);
      return { ...l, members };
    });
    edit = edit;
  }

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
    // A freshly-added bound line follows THIS TEMPLATE'S OWN style out of the
    // box: verse text takes its verse colour, a reference its reference colour,
    // anything else the accent — and all take its typeface. The operator can
    // unbind any of them to a literal in the properties panel.
    const colorToken =
      bind === 'verse' ? 'theme:verse' : bind === 'reference' ? 'theme:reference' : 'theme:accent';
    const L = makeLayer('text', {
      bind,
      name: BINDINGS.find((b) => b.key === bind)?.label,
      color: colorToken,
      font: 'theme:font',
    });
    edit.layout.layers = [...(edit.layout.layers || []), L];
    selId = L.id;
    edit = edit;
  }
  // DELETE IS TWO STEPS, and the first one lapses.
  //
  // It used to remove the object on a single click of a 20px button sitting
  // between Lock and Visibility in a row of five. Undo exists, and "your work is
  // one keystroke away" is not the same as "you did not lose it" when the panel
  // is being used against the clock. Never a native confirm(): Tauri's webview
  // returns false without showing a dialog, so a delete guarded by one deletes
  // nothing and reports success (CLAUDE.md rule 41).
  let armedDelete = null;
  let armedTimer = 0;
  function disarmDelete() {
    clearTimeout(armedTimer);
    armedTimer = 0;
    armedDelete = null;
  }
  function removeLayer(id) {
    if (armedDelete !== id) {
      clearTimeout(armedTimer);
      armedDelete = id;
      // Long enough to read the word "Delete?" and decide; short enough that an
      // armed button never sits waiting through the next thing you do.
      armedTimer = setTimeout(() => { armedDelete = null; }, 4000);
      return;
    }
    disarmDelete();
    // Through `layerops`, not through a filter: a band names its members by id,
    // and a filter here would leave the band pointing at an object that is gone.
    // Nothing would break — which is exactly why it would have survived.
    edit.layout.layers = dropLayer(edit.layout.layers, id);
    if (selId === id) selId = edit.layout.layers[edit.layout.layers.length - 1]?.id ?? null;
    edit = edit;
  }
  function duplicate(id) {
    const before = edit.layout.layers || [];
    const after = duplicateLayer(before, id);
    if (after === before) return;
    edit.layout.layers = after;
    selId = after[after.findIndex((l) => l.id === id) + 1].id;
    edit = edit;
  }
  function resetObject(id) {
    const after = resetLayer(edit.layout.layers || [], id);
    edit.layout.layers = after;
    edit = edit;
  }
  // Through `layerops`, for the same reason delete is: the arithmetic of this
  // one is where the mistake was. A raw adjacent swap of the array moved a band
  // member — an object NOTHING draws from that array — so the arrows did nothing
  // visible on every template that has a band, which is every lower third the
  // product ships. `moveLayer` returns the same list when nothing can move.
  function moveLayer(id, dir) {
    const before = edit.layout.layers || [];
    const after = moveInOrder(before, id, dir);
    if (after === before) return;
    edit.layout.layers = after;
    edit = edit;
  }
  /** Which way the list's ↑ / ↓ mean for this row. Up the LIST is toward the
   *  front of the stack (+1), and for a word inside a band it is one place
   *  EARLIER in the band (−1) — because the list draws a band's words in the
   *  order the band reads them, not in the order they paint. */
  const moveDir = (row, up) => (row.member ? (up ? -1 : 1) : up ? 1 : -1);

  // ── DRAG TO REORDER ────────────────────────────────────────────────────────
  //
  // The arrows are still there and still the keyboard's way through this. What
  // this adds is the gesture everybody arrives expecting, and it is built ON the
  // arrows rather than beside them: `reorderTo` steps `moveLayer` one place at a
  // time until the dragged row reaches the index the drop named. That matters
  // more than it looks. `moveLayer` is the module that knows a band's words are
  // not in the stack, that a word steps within its band and may never step out
  // of it, and that a band carries its words with it; a drop handler that spliced
  // the array itself would be a SECOND answer to all three, and the first one
  // took a bug in every lower third the product ships to find.
  //
  // A drag never crosses a context. The stack and each band are separate orders,
  // so dropping a word onto a shape is not a move that exists — and the loop
  // below would half-do it, shuffling the word inside its band and stopping. The
  // row refuses the drop instead, and shows no line where it cannot land.
  let dragId = null;   // the row being carried
  let overId = null;   // the row it is currently over, if the drop is legal

  /** Which ORDER a row belongs to: the stack, or one particular band. */
  function rowContext(list, id) {
    const band = bandOf(list, id);
    return band ? band.id : '__stack__';
  }

  /**
   * The row a drop on `id` actually MEANS for the row being carried, or null if
   * the drop has no meaning at all.
   *
   * Same context: itself. A top-level object dropped on a band's WORD: the BAND
   * — a band's words are indented under it and the only position in the stack
   * that row stands for is the band's own, so refusing would leave a dead zone
   * across most of the list on every lower third the product ships. A word
   * dropped on a top-level row: NOTHING. Leaving a band is what the "In band"
   * control is for, and it is not something a gesture may do by accident (the
   * same sentence `moveLayer` makes about the arrows).
   */
  function dropTarget(list, dragged, id) {
    if (!dragged || dragged === id) return null;
    const from = rowContext(list, dragged);
    const to = rowContext(list, id);
    if (from === to) return id;
    if (from !== '__stack__') return null;
    return to === dragged ? null : to; // `to` is the band's own id
  }
  const canDropOn = (id) => !!dropTarget(layers, dragId, id);

  function reorderTo(id, dropOn) {
    const before = edit?.layout?.layers || [];
    const targetId = dropTarget(before, id, dropOn);
    if (!targetId) return;
    const ids = (l) => panelOrderOf(l).map((r) => r.L.id);
    const start = ids(before).indexOf(id);
    const want = ids(before).indexOf(targetId);
    if (start < 0 || want < 0 || start === want) return;
    const up = want < start; // up the LIST is toward the front of the stack
    let list = before;
    // Bounded by construction: every iteration either moves the row one place
    // (so the distance to `want` shrinks) or returns the same list and stops.
    // The guard is belt and braces against a future `moveLayer` that cycles.
    for (let guard = 0; guard <= before.length * 2; guard++) {
      const order = panelOrderOf(list);
      const at = order.findIndex((r) => r.L.id === id);
      if (at < 0 || at === want) break;
      const next = moveInOrder(list, id, moveDir(order[at], up));
      if (next === list) break; // nowhere further to go — already at the end
      list = next;
    }
    if (list === before) return;
    edit.layout.layers = list;
    selId = id;
    edit = edit;
  }

  function onRowDragStart(e, L) {
    // A LOCKED ROW DOES NOT MOVE. The lock already refuses the canvas drag and
    // the arrow-key nudge; a rail that reordered it anyway would mean the same
    // padlock answered yes on one surface and no on another.
    if (L.locked) { e.preventDefault(); return; }
    dragId = L.id;
    selId = L.id;
    try {
      e.dataTransfer.effectAllowed = 'move';
      // Firefox starts no drag at all without some data on the transfer.
      e.dataTransfer.setData('text/plain', L.id);
    } catch { /* jsdom has no dataTransfer; the reorder is driven directly */ }
  }
  function onRowDragOver(e, L) {
    if (!canDropOn(L.id)) { overId = null; return; }
    e.preventDefault();
    try { e.dataTransfer.dropEffect = 'move'; } catch { /* see above */ }
    overId = L.id;
  }
  function onRowDrop(e, L) {
    e.preventDefault();
    if (canDropOn(L.id)) reorderTo(dragId, L.id);
    dragId = null;
    overId = null;
  }
  const onRowDragEnd = () => { dragId = null; overId = null; };

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
  // USED FOR. Which kinds of content wear this template on any screen set to
  // follow the content look (DECISIONS §70). Toggling writes through
  // `setContentTemplate`, which is the ONE writer of that map — three surfaces
  // used to each hold their own copy and overwrite one another.
  let lookErr = '';
  async function toggleUsedFor(kind) {
    const mine = $contentTemplates[kind] === edit?.id;
    lookErr = '';
    try {
      await setContentTemplate(kind, mine ? null : edit.id);
    } catch (e) {
      lookErr = humanError(e);
    }
  }
  function set(k, v) { if (sel) { sel[k] = v; edit = edit; } }
  /** A geometry number, clamped to the canvas so an object cannot be typed off it. */
  function geom(k, v) {
    const n = Number(v);
    if (!Number.isFinite(n)) return;
    set(k, Math.max(0, Math.min(100, n)));
  }
  function num(k, v) { set(k, +v); }
  // Layer colour/fill/font can bind to a STYLE TOKEN (`theme:accent`) that follows
  // this template's own style, or be a literal.
  //
  // THE WORD AN OPERATOR READS IS `linked`, AND THE PICKER IS `Style link`.
  // Both used to say "theme", which named a surface that no longer exists
  // (DECISIONS §87). The stored VALUE is untouched and still spells
  // `theme:accent` — it is written into every saved layer in every install, and
  // renaming it would need a migration to buy a nicer word — so this is a label
  // change only and nothing an operator has saved moves. Five pickers and five
  // swatch readouts, renamed together: a label saying one thing beside a readout
  // saying another is how a half-rename gets left behind.
  // Colours offer every token except the
  // typeface; unbinding ('custom') restores an editable literal.
  const COLOUR_TOKENS = THEME_TOKENS.filter((t) => t.token !== 'theme:font');
  function bindToken(field, value, fallback) {
    set(field, value === 'custom' ? fallback : value);
  }

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
    // CAPTURE THE POINTER, or the drag can outlive the gesture.
    //
    // The move/up pair is listened for on `window`, which hears everything that
    // happens INSIDE the window and nothing that happens outside it. The canvas
    // sits against the edge of a 1600-wide console: drag an object to the left
    // edge, keep going, release over the desktop or the title bar, and the
    // `pointerup` is delivered to something else. `drag` is then still set, and
    // the next time the pointer crosses the canvas the object follows it with no
    // button held — a layer that moves when you are not moving it, on the one
    // surface whose whole job is direct manipulation.
    //
    // Capturing retargets every later event for this pointer to this element,
    // wherever it goes, and the browser releases it implicitly on `pointerup` —
    // which still bubbles to `window`, so the handlers below are untouched.
    // Optional-called: jsdom does not implement it, and a missing capture is a
    // worse drag, never a broken one.
    e.currentTarget?.setPointerCapture?.(e.pointerId);
    window.addEventListener('pointermove', onDrag);
    window.addEventListener('pointerup', endDrag);
    // A cancelled pointer (the OS taking over, a touch turning into a scroll)
    // never sends `pointerup`, and it is the same sticky drag by another route.
    window.addEventListener('pointercancel', endDrag);
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
    window.removeEventListener('pointercancel', endDrag);
  }
  // One-click centring — the "center items" ask. Centres the selected layer on
  // the canvas, horizontally, vertically, or both.
  // THE SAME ARITHMETIC AS THE STRIP, not a second copy of it. These three
  // buttons predate the alignment strip and write the same two numbers; leaving
  // them with their own `50 - w/2` would be two answers to one question, which is
  // exactly how this panel ended up with two Position groups that disagreed.
  // They stay because Preview mode hides the canvas chrome, strip included, and a
  // control that vanishes with the rulers is not a control.
  function center(axis) {
    if (axis === 'x' || axis === 'both') alignSel('hcenter');
    if (axis === 'y' || axis === 'both') alignSel('vmiddle');
  }

  // ── THE ALIGNMENT STRIP ────────────────────────────────────────────────────
  //
  // Six edges and two spacings, over the canvas, where the object is. They write
  // the same four numbers the drag writes — a SHARE OF THE FRAME, rounded to a
  // tenth, clamped so nothing leaves the canvas — through `layerops`, which is
  // where the arithmetic lives for the same reason `moveLayer` does: an "align
  // right" that stores `x = 100` looks, in a 240px preview, like a centred object
  // with a wide margin.
  //
  // NOTHING HERE WRITES A PIXEL. The renderer sizes in cqw, so a pixel an editor
  // stored on one machine would land somewhere else on every screen it reached.
  const ALIGN_BUTTONS = [
    { edge: 'left', label: 'Align left', d: 'M4 3v18M8 7h11M8 14h7' },
    { edge: 'hcenter', label: 'Align centre', d: 'M12 3v18M6 7h12M8 14h8' },
    { edge: 'right', label: 'Align right', d: 'M20 3v18M5 7h11M9 14h7' },
    { edge: 'top', label: 'Align top', d: 'M3 4h18M7 8v11M14 8v7' },
    { edge: 'vmiddle', label: 'Align middle', d: 'M3 12h18M7 6v12M14 8v8' },
    { edge: 'bottom', label: 'Align bottom', d: 'M3 20h18M7 5v11M14 9v7' },
  ];
  // The names for the inspector's two text-alignment rows. They are SEPARATE from
  // the six above on purpose: the strip moves the object inside the canvas, these
  // set how the words sit inside the object, and for a while both rows answered to
  // "Align left". British spelling, like the strip's `Align centre` — the value
  // stored in the template is still the CSS `center`, which is not a word anybody
  // reads out loud.
  const TEXT_ALIGN_LABEL = {
    left: 'Text aligned left',
    center: 'Text centred',
    right: 'Text aligned right',
  };
  const VALIGN_LABEL = {
    top: 'Text at the top of the box',
    middle: 'Text in the middle of the box',
    bottom: 'Text at the bottom of the box',
  };
  // Only an object these four numbers actually place may be aligned — the same
  // question the Position group asks, from the same module, so the strip and the
  // number grid can never disagree about whether an object can move.
  $: canAlign = !!sel && isMovable(layers, sel);
  // Spacing needs three: with two, every arrangement is already evenly spaced.
  $: spaceable = movableLayers(layers).length;
  function alignSel(edge) {
    const before = edit?.layout?.layers || [];
    const after = alignLayer(before, selId, edge);
    if (after === before) return;
    edit.layout.layers = after;
    edit = edit;
  }
  function spaceOut(axis) {
    const before = edit?.layout?.layers || [];
    const after = spaceEvenly(before, axis);
    if (after === before) return;
    edit.layout.layers = after;
    edit = edit;
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
    'Fraunces', 'Playfair Display', 'Space Grotesk', 'Inter', 'IBM Plex Mono', 'JetBrains Mono',
    'Georgia', 'Times New Roman', 'Palatino', 'Baskerville', 'Garamond',
    'Helvetica Neue', 'Arial', 'Futura', 'Gill Sans', 'Optima', 'Didot',
    'Menlo', 'Courier New', 'Verdana', 'Trebuchet MS', 'Cambria',
  ];
  const FONT_LABEL = {
    'var(--f-serif)': 'Fraunces (serif)', 'var(--f-display)': 'Inter (display)',
    'var(--f-body)': 'Inter (body)', 'var(--f-mono)': 'IBM Plex Mono', 'var(--f-head)': 'Inter (heading)',
  };
  const fontLabel = (f) => FONT_LABEL[f] ?? f;
  const BUNDLED_FONTS = new Set([
    'var(--f-serif)', 'var(--f-display)', 'var(--f-body)', 'var(--f-mono)', 'var(--f-head)',
    'Fraunces', 'Inter', 'Playfair Display', 'Space Grotesk', 'IBM Plex Mono', 'JetBrains Mono',
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

  // ── Live apply (debounced save + push) ─────────────────────────────────────
  let lastSig = '';
  let liveTimer;
  const sigOf = (t) => JSON.stringify({ name: t?.name, layout: t?.layout, style: t?.style });
  // Coalesce, and DON'T stringify per frame. This block re-runs on every
  // `edit = edit` — which fires on every pointermove of a drag/resize. It used to
  // call `sigOf(edit)` (a full JSON.stringify of layout+style) on each of those
  // frames just to compare. Now the frame only re-arms a 400ms timer; the whole
  // template is serialised ONCE, when the drag settles, inside the timer.
  $: if (edit) scheduleLive();
  function scheduleLive() {
    clearTimeout(liveTimer);
    liveTimer = setTimeout(() => {
      const sig = sigOf(edit);
      if (sig === lastSig) return;
      // A settled, user-made change: bank the previous state for undo, and a new
      // edit invalidates any redo tail. (undo/redo set `lastSig` before mutating
      // `edit`, so their restores land here as sig === lastSig and are skipped.)
      past = [...past, lastSig].slice(-HIST_MAX);
      future = [];
      lastSig = sig;
      applyLive();
    }, 400);
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
  async function saveNow() {
    clearTimeout(liveTimer);
    await applyLive();
    // An explicit Save banks a restore point (deduped) — distinct from the live
    // autosave, which must NOT spam the history on every drag.
    if (edit?.id) {
      await snapshotTemplateVersion(edit);
      if (histOpen) await loadHistory();
    }
  }

  // ── Version history (persisted restore points) ─────────────────────────────
  let histOpen = false;
  let versions = [];
  async function loadHistory() {
    if (edit?.id) versions = await listTemplateVersions(edit.id);
  }
  async function toggleHistory() {
    histOpen = !histOpen;
    if (histOpen) await loadHistory();
  }
  onMount(() => {
    const close = () => (histOpen = false);
    window.addEventListener('click', close);
    return () => window.removeEventListener('click', close);
  });
  async function restoreVersion(v) {
    if (!edit) return;
    try {
      await restoreTemplateVersion(edit, v);
      edit = { ...edit, layout: v.layout, style: v.style };
      lastSig = sigOf(edit); // this IS the committed state now — don't re-autosave it
      histOpen = false;
    } catch (e) { err = humanError(e); }
  }
  // A short relative-time label for a version's timestamp.
  function agoLabel(ts) {
    const s = Math.max(0, Math.round((Date.now() - (Number(ts) || 0)) / 1000));
    if (s < 60) return 'just now';
    const m = Math.round(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.round(m / 60);
    if (h < 24) return `${h}h ago`;
    return `${Math.round(h / 24)}d ago`;
  }

  // ── Undo / redo ────────────────────────────────────────────────────────────
  //
  // A direct-manipulation editor with autosave-to-live but no undo meant one bad
  // drag was unrecoverable (Decision §26). History is a stack of `sigOf`
  // snapshots — the same {name,layout,style} shape the autosave already
  // serialises. `lastSig` doubles as the CURRENT node: it always equals the
  // committed state, so it is both the change-detector and the history cursor.
  const HIST_MAX = 60;
  let past = [];
  let future = [];
  $: canUndo = past.length > 0;
  $: canRedo = future.length > 0;

  function restoreFrom(sig) {
    const s = JSON.parse(sig);
    edit = { ...edit, name: s.name, layout: s.layout, style: s.style };
    // Keep the selected layer valid across the restore.
    if (isLayered(edit) && !edit.layout.layers?.some((l) => l.id === selId)) {
      selId = edit.layout.layers?.[0]?.id ?? null;
    }
  }
  function undo() {
    if (!canUndo || fsPreview) return;
    future = [lastSig, ...future].slice(0, HIST_MAX);
    lastSig = past[past.length - 1];
    past = past.slice(0, -1);
    restoreFrom(lastSig);
    applyLive();
  }
  function redo() {
    if (!canRedo || fsPreview) return;
    past = [...past, lastSig].slice(-HIST_MAX);
    lastSig = future[0];
    future = future.slice(1);
    restoreFrom(lastSig);
    applyLive();
  }
  function onKey(e) {
    if (!edit) return;

    // RULE 44 · a mounted [role="menu"] makes `shortcuts.js` stand down, so while
    // one of these is open the operator's panic key belongs to it. Both menus here
    // used to bind nothing, under a comment claiming "Escape is handled globally —
    // shortcuts.js gives Escape to any mounted [role=menu]". It does not: it
    // RETURNS. The key was withheld from the shell and delivered to nobody, and
    // pressing it did neither thing the operator might have wanted.
    //
    // The comment's stated reason for binding nothing was that a handler "would
    // have to stopPropagation too, which would swallow Space". That does not
    // follow. This branch returns for every key that is not Escape, so Space, the
    // arrows and everything else reach the global handler exactly as before — only
    // the key that was already being withheld is consumed. Closing the menu is all
    // it does; the second press reaches the screens through `shortcuts.js`.
    if (e.key === 'Escape' && (histOpen || addOpen)) {
      e.preventDefault();
      e.stopPropagation();
      histOpen = false;
      addOpen = false;
      return;
    }

    const tgt = e.target;
    // Let text fields keep their own native undo.
    if (tgt && (tgt.tagName === 'INPUT' || tgt.tagName === 'TEXTAREA' || tgt.isContentEditable)) return;
    if (!(e.metaKey || e.ctrlKey)) return;
    if (e.key === 'z' && !e.shiftKey) { e.preventDefault(); undo(); }
    else if ((e.key === 'z' && e.shiftKey) || e.key === 'y') { e.preventDefault(); redo(); }
  }

  // ── Preview / test on the real screens (Decision §26) ──────────────────────
  let fsPreview = false;   // in-console fullscreen preview overlay (reaches no output)
  let testing = false;
  async function testOnScreens() {
    if (!edit) return;
    testing = true;
    err = '';
    try {
      if (!edit.id) await applyLive(); // a never-saved template has no id yet
      await testTemplateOnOutputs(edit.id);
    } catch (e) {
      err = 'Test failed: ' + humanError(e);
    }
    testing = false;
  }
</script>

<svelte:window on:keydown={onKey} />

{#if fsPreview && edit}
  <TemplatePreviewOverlay template={themedEdit} onClose={() => (fsPreview = false)} />
{/if}

<div class="te-shell">
  <header class="te-top">
    <button class="r-btn ghost sm" on:click={() => dispatch('back')}>
      <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M19 12H5M12 19l-7-7 7-7"/></svg>
      Back to Templates
    </button>
    {#if edit}<span class="te-name">{edit.name}</span><span class="te-sub r-mono">{layered ? layers.length + ' layers' : 'legacy'} · 1920×1080</span>{/if}
    {#if edit && layered}
      <div class="te-undo">
        <button class="r-iconbtn te-zbtn" on:click={undo} disabled={!canUndo} title="Undo (Ctrl/⌘+Z)" aria-label="Undo">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M9 14 4 9l5-5"/><path d="M4 9h11a5 5 0 0 1 0 10h-1"/></svg>
        </button>
        <button class="r-iconbtn te-zbtn" on:click={redo} disabled={!canRedo} title="Redo (Ctrl/⌘+Shift+Z)" aria-label="Redo">
          <svg viewBox="0 0 24 24" width="15" height="15" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m15 14 5-5-5-5"/><path d="M20 9H9a5 5 0 0 0 0 10h1"/></svg>
        </button>
      </div>
    {/if}
    <span class="te-spring"></span>
    <div class="te-zoom">
      <button class="r-iconbtn te-zbtn" on:click={() => (zoomIdx = Math.max(0, zoomIdx - 1))} disabled={zoomIdx === 0} aria-label="Zoom out">−</button>
      <span class="te-pct r-mono">{zoom}%</span>
      <button class="r-iconbtn te-zbtn" on:click={() => (zoomIdx = Math.min(ZOOMS.length - 1, zoomIdx + 1))} disabled={zoomIdx === ZOOMS.length - 1} aria-label="Zoom in">+</button>
    </div>
    <button class="r-btn ghost sm" class:on={previewMode} on:click={() => (previewMode = !previewMode)}>{previewMode ? 'Editing' : 'Preview'}</button>
    <button class="r-btn ghost sm" on:click={() => (fsPreview = true)} disabled={!edit} title="Preview this template fullscreen in the console — reaches no output">Fullscreen</button>
    <button class="r-btn ghost sm" on:click={testOnScreens} disabled={testing || !$capture.available || !edit} title="Put a sample verse on the live screens with this template — clear it with Esc">
      {testing ? 'Testing…' : 'Test on screens'}
    </button>
    <span class="te-histwrap">
      <button class="r-btn ghost sm" class:on={histOpen} on:click|stopPropagation={toggleHistory} disabled={!edit?.id} title="Restore an earlier saved version of this template">History</button>
      {#if histOpen}
        <!-- The click handler is not an interaction: it stops the document-level
             outside-click closer from seeing a click on the menu itself. Every real
             control inside is a <button>, so the keyboard already reaches all of them,
             and Escape is consumed by this component's own `onKey` (rule 44). It used to say
             Escape was "handled globally"; `shortcuts.js` stands DOWN for a mounted
             [role="menu"] and hands it to nobody, so the key did nothing at all. `onKey`
             returns for every key that is not Escape, so Space still means advance
             (rule 11) for as long as a menu is open. -->
        <!-- svelte-ignore a11y-click-events-have-key-events -->
        <div class="te-histmenu" on:click|stopPropagation role="menu" tabindex="-1">
          <div class="te-histhead r-lbl">Saved versions</div>
          {#if versions.length}
            {#each versions as v, i (v.ts)}
              <button class="te-histitem" on:click={() => restoreVersion(v)}>
                <span class="te-histwhen">{i === 0 ? 'Latest' : agoLabel(v.ts)}</span>
                <span class="te-histrestore r-mono">Restore</span>
              </button>
            {/each}
          {:else}
            <div class="te-histempty">No saved versions yet. Press <b>Save Template</b> to bank one.</div>
          {/if}
        </div>
      {/if}
    </span>
    <button class="r-btn primary sm" on:click={saveNow} disabled={saving || !$capture.available || !edit} title="Edits apply to live outputs automatically; an explicit Save also banks a restore point">
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
        <div class="te-legacyprev"><TemplateRender template={themedEdit} content={SAMPLE} /></div>
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
            <button class="r-iconbtn te-addbtn" on:click|stopPropagation={() => (addOpen = !addOpen)} aria-label="Add layer">＋</button>
            {#if addOpen}
              <!-- The click handler is not an interaction: it stops the document-level
                   outside-click closer from seeing a click on the menu itself. Every real
                   control inside is a <button>, so the keyboard already reaches all of them,
                   and Escape is consumed by this component's own `onKey` (rule 44). It used to say
                   Escape was "handled globally"; `shortcuts.js` stands DOWN for a mounted
                   [role="menu"] and hands it to nobody, so the key did nothing at all. `onKey`
                   returns for every key that is not Escape, so Space still means advance
                   (rule 11) for as long as a menu is open. -->
              <!-- svelte-ignore a11y-click-events-have-key-events -->
              <div class="te-addmenu" on:click|stopPropagation role="menu" tabindex="-1">
                <div class="te-addsec r-lbl">Add layer</div>
                {#each LAYER_TYPES as t}
                  <button class="te-addmi" on:click={() => addLayer(t.type)}><span class="te-addico">{t.icon}</span>{t.label}</button>
                {/each}
                <div class="te-addsec r-lbl">Bound text</div>
                {#each BINDINGS.filter((b) => b.key !== 'static') as b}
                  <button class="te-addmi" on:click={() => addBoundText(b.key)}><span class="te-addico">T</span>{b.label}</button>
                {/each}
              </div>
            {/if}
          </div>
        </div>
        <div class="te-layerlist r-scroll">
          {#each panelRows as row (row.L.id)}
            {@const L = row.L}
            {@const th = thumbOf(L)}
            <!-- A LIST ROW, carried by its own grip. The whole row is the drop
                 target; `dragover` has to preventDefault or the browser refuses
                 the drop, and it only does so where the drop is legal, so a row
                 in another order shows no line and takes nothing. -->
            <div class="te-layer" class:sel={selId === L.id} class:off={L.visible === false} class:inband={row.member}
              class:dragging={dragId === L.id} class:dropto={overId === L.id}
              draggable={!L.locked}
              on:dragstart={(e) => onRowDragStart(e, L)}
              on:dragover={(e) => onRowDragOver(e, L)}
              on:dragleave={() => { if (overId === L.id) overId = null; }}
              on:drop={(e) => onRowDrop(e, L)}
              on:dragend={onRowDragEnd}
              on:click={() => (selId = L.id)} role="button" tabindex="0"
              on:keydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); selId = L.id; } }}>
              <span class="te-lgrip" aria-hidden="true" title={L.locked ? 'Locked — unlock it to reorder by dragging' : 'Drag to reorder'}>⠿</span>
              <!-- THE PLACEMENT PROXY (`thumbOf`). Not a render and not a claim
                   to be one: one box, where this object sits, in its own colour.
                   aria-hidden, because the row already says the object's name and
                   a screen reader has nothing to gain from a coloured rectangle. -->
              <span class="te-lthumb" aria-hidden="true">
                <span
                  class="te-lthumbbox"
                  class:words={th.words}
                  class:hollow={th.hollow}
                  style="left:{th.x}%; top:{th.y}%; width:{th.w}%; height:{th.h}%; background:{th.fill}; opacity:{th.opacity};"
                >{#if th.words}<i style="background:{th.ink}"></i><i style="background:{th.ink}"></i>{/if}</span>
              </span>
              <span class="te-lname" title={layerLabel(L)}>{layerLabel(L)}</span>
              <span class="te-lstate">
                <button class="te-lmini" title={L.visible === false ? 'Show' : 'Hide'} class:dim={L.visible === false}
                  aria-label={L.visible === false ? `Show ${layerLabel(L)}` : `Hide ${layerLabel(L)}`}
                  on:click|stopPropagation={() => toggleVisible(L.id)}>{L.visible === false ? '◌' : '●'}</button>
                <button class="te-lmini" title={L.locked ? 'Unlock' : 'Lock'} class:on={L.locked}
                  aria-label={L.locked ? `Unlock ${layerLabel(L)}` : `Lock ${layerLabel(L)}`}
                  on:click|stopPropagation={() => toggleLock(L.id)}>{L.locked ? '🔒' : '🔓'}</button>
              </span>
              <!-- WHAT KIND IT IS, in the row that has room for it now. The
                   glyph still comes from the ONE register (`typeIcon` →
                   `LAYER_TYPES`) — it drew its own four-way guess once and
                   answered `T` for a band, a region AND a timer. -->
              <span class="te-lkind">
                <span class="te-ltype" aria-hidden="true">{typeIcon(L.type)}</span>
                <span class="te-lkindtxt">{row.member ? 'in band' : LAYER_TYPES.find((t) => t.type === L.type)?.label ?? L.type}</span>
              </span>
              <span class="te-lbtns">
                <!-- UP AND DOWN THE LIST, which is not the same axis for both
                     kinds of row — `moveDir` holds the one rule, and the label
                     says which one this row is under. They stay: a drag is not
                     reachable from a keyboard, and reordering a template is not
                     something only a mouse may do. -->
                <button class="te-lmini" title={row.member ? 'Earlier in the band' : 'Forward'}
                  aria-label={row.member ? `Move ${layerLabel(L)} earlier in the band` : `Bring ${layerLabel(L)} forward`}
                  on:click|stopPropagation={() => moveLayer(L.id, moveDir(row, true))}>↑</button>
                <button class="te-lmini" title={row.member ? 'Later in the band' : 'Back'}
                  aria-label={row.member ? `Move ${layerLabel(L)} later in the band` : `Send ${layerLabel(L)} back`}
                  on:click|stopPropagation={() => moveLayer(L.id, moveDir(row, false))}>↓</button>
                <button
                  class="te-lmini danger"
                  class:armed={armedDelete === L.id}
                  title={armedDelete === L.id ? 'Click again to delete' : 'Delete'}
                  aria-label={armedDelete === L.id ? `Delete ${layerLabel(L)} — click again to confirm` : `Delete ${layerLabel(L)}`}
                  on:click|stopPropagation={() => removeLayer(L.id)}
                  on:blur={() => armedDelete === L.id && disarmDelete()}
                >{armedDelete === L.id ? 'Sure?' : '✕'}</button>
              </span>
            </div>
          {/each}
          {#if !layers.length}<div class="te-hint r-mono">No layers — use ＋ to add one.</div>{/if}
        </div>
        <p class="te-panenote">Top of the list is the front. Drag a row by its grip to reorder, or use ↑ ↓. An indented row is a word inside the band above it — the band decides where it sits, so it moves within the band and not out of it. A locked row will not move, here or on the canvas.</p>

        <!-- READABILITY. Still under the layer list, in the panel a designer
             already has open — but FOLDED, because what was here was seven
             paragraphs of small italic prose, three verdict rows, two number
             fields and a button: the biggest thing in the column and the least
             used. It is one row now, and that row carries the verdict, so it is
             not "behind a button you have to know about" either. -->
        {#if legible}
          <!-- A DISCLOSURE ROW, not a button — the panel's own heading with its
               current verdict on it and a chevron at the end. It is a <button>
               so the whole row is the target and the keyboard reaches it; it
               carries `aria-expanded`, so what it does is stated rather than
               drawn. -->
          <!-- Folding the panel also puts the distance strip away. Its only
               switch lives INSIDE the panel, so leaving it on over a closed
               panel would paint four extra renders across the canvas with
               nothing on screen able to turn them off. -->
          <button class="te-legtoggle" aria-expanded={legOpen} on:click={() => { legOpen = !legOpen; if (!legOpen) showDistances = false; }}>
            <span class="r-lbl">Readability</span>
            <span class="te-legsum" class:bad={legState === 'bad'} class:unknown={legState === 'unknown'}>{legSummary}</span>
            <span class="te-legchev" aria-hidden="true">{legOpen ? '▴' : '▾'}</span>
          </button>
        {/if}
        {#if legible && legOpen}
          <ul class="te-leg">
            {#each [['Verse', legible.verse], ['Reference', legible.reference], ['From the back', legible.distance]] as [label, c] (label)}
              <li class="te-legrow" class:bad={c.state === 'low' || c.state === 'small'} class:unknown={c.state === 'unknown'}>
                <b>{label}</b>
                <span>{c.note}</span>
              </li>
            {/each}
            <!-- THE MEASURED ROW. The three above are computed from the template's
                 declared colours and sizes; this one is what the renderer actually
                 settled at on this artboard, which is the only one of the four that
                 can catch rule 37's failure. Absent when there is nothing to say,
                 like every other honest row in this product. -->
            {#if fitNote}
              <li class="te-legrow" class:bad={fitBad}>
                <b>Measured</b>
                <span>{fitNote}</span>
              </li>
            {/if}
          </ul>
          <div class="te-legroom">
            <label class="te-leglab" for="te-scr">Screen width (m)</label>
            <input id="te-scr" class="r-input te-legin" inputmode="decimal" bind:value={screenWidthM} placeholder="4" />
            <label class="te-leglab" for="te-back">Back row (m)</label>
            <input id="te-back" class="r-input te-legin" inputmode="decimal" bind:value={backRowM} placeholder="18" />
          </div>
          <button class="r-btn ghost sm te-legbtn" on:click={() => (showDistances = !showDistances)}>
            {showDistances ? 'Hide' : 'Show'} how it looks from further back
          </button>
          <!-- THE CAVEAT RIDES WITH THE VERDICT. The person reading it is deciding
               whether to trust it right now, and it has never been checked against
               a projector in a church. -->
          <p class="te-panenote">{legible.caveat}</p>
        {/if}
      </aside>

      <!-- ══ CANVAS ══ -->
      <section class="te-canvas">
        {#if !previewMode}
          <!-- THE ALIGNMENT STRIP, over the canvas, where the object is. It
               writes the same percentages the drag writes, through `layerops`.
               Hidden in Preview, which is the mode that takes the editor's
               chrome away — the rulers and the handle boxes go with it. -->
          <div class="te-alignbar" role="toolbar" aria-label="Align the selected object">
            <span class="r-lbl te-alignlbl">Align</span>
            {#each ALIGN_BUTTONS as a (a.edge)}
              <button
                class="r-iconbtn te-abtn"
                disabled={!canAlign}
                title={canAlign ? a.label : 'Select an object the canvas can move'}
                aria-label={a.label}
                on:click={() => alignSel(a.edge)}
              >
                <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"><path d={a.d}/></svg>
              </button>
            {/each}
            <span class="te-adiv" aria-hidden="true"></span>
            <!-- SPACE EVENLY says what it does to WHAT. There is one selection in
                 this editor, and "distribute the selection" over one object is a
                 control that cannot mean anything — so these act on every object
                 the canvas can move, and the label and the tooltip both say so
                 rather than leaving an operator to find out by pressing it. -->
            <button class="r-btn ghost sm te-abtn2" disabled={spaceable < 3}
              title={spaceable < 3 ? 'Needs three objects the canvas can move' : `Space all ${spaceable} movable objects evenly across the frame`}
              on:click={() => spaceOut('x')}>Space across</button>
            <button class="r-btn ghost sm te-abtn2" disabled={spaceable < 3}
              title={spaceable < 3 ? 'Needs three objects the canvas can move' : `Space all ${spaceable} movable objects evenly down the frame`}
              on:click={() => spaceOut('y')}>Space down</button>
          </div>
        {/if}
        {#if showDistances}
          <!-- STEPPING BACK, simulated. Shrinking the render by the ratio of the
               distances is exactly what a person does when they walk away from a
               screen. It is a demonstration, not a measurement — the numbers above
               are the measurement. -->
          <div class="te-dists">
            {#each PREVIEW_DISTANCES_M as d (d)}
              <figure class="te-dist">
                <div class="te-distbox">
                  <div class="te-distinner" style="transform:scale({previewScale(d)})">
                    <TemplateRender template={themedEdit} content={previewContent} />
                  </div>
                </div>
                <figcaption class="r-mono">{d}m</figcaption>
              </figure>
            {/each}
          </div>
        {/if}
        <div class="te-stage">
          <div class="te-board-wrap" style="width:{zoom}%">
            {#if !previewMode}
              <div class="te-ruler te-ruler-x">{#each Array(11) as _, i}<span style="left:{i * 10}%">{i * 10}</span>{/each}</div>
              <div class="te-ruler te-ruler-y">{#each Array(11) as _, i}<span style="top:{i * 10}%">{i * 10}</span>{/each}</div>
            {/if}
          <div class="te-artboard" bind:this={boardEl}>
            {#if !previewMode}<div class="te-checker"></div>{/if}
            <!-- RULE 37's REPORT, ON THE SURFACE THE RULE NAMES.
                 `TemplateRender`'s own comment says the fit "SAYS SO, once, to
                 whoever is rendering it. The console and the Templates editor can
                 then tell the operator while there is still time to pick a
                 different look." `onFit` had exactly ONE caller in the whole tree
                 and it was Live — the Sunday surface, where nobody is going to
                 re-typeset a template at 10:29. This is the Tuesday one.
                 The card thumbnails deliberately still pass nothing: they are
                 194px wide, so their fit is not the wall's fit and a warning there
                 would be noise. -->
            <TemplateRender template={themedEdit} content={previewContent} onFit={noteFit} />
            {#if !previewMode}
              <!-- Selection / drag overlay: one handle box per positioned layer. -->
              <div class="te-overlay">
                <!-- Alignment guides — flash while a layer snaps to centre / edge. -->
                {#if guides.v != null}<div class="te-guide te-guide-v" style="left:{guides.v}%"></div>{/if}
                {#if guides.h != null}<div class="te-guide te-guide-h" style="top:{guides.h}%"></div>{/if}
                {#each layers as L (L.id)}
                  {#if L.visible !== false && L.type !== 'background'}
                    <!-- THE HANDLE GOES WHERE THE OBJECT IS DRAWN, which for a band
                         and its words is not the box they store (`drawBoxes`). A
                         handle sitting somewhere the words are not reads as a broken
                         editor, and it is the WYSIWYG guarantee failing one layer
                         above the renderer.

                         A band and its words are PLACED, not dragged: dragging writes
                         x/y/w/h, and nothing draws them from x/y/w/h, so the drag
                         would move the outline and leave the type where it was — a
                         control that changes nothing (DECISIONS §69). Their real
                         controls are in the panel. -->
                    {@const b = drawn.get(L.id) || L}
                    {@const placed = L.type === 'band' || !!bandOf(layers, L.id)}
                    <div class="te-hbox" class:sel={selId === L.id} class:locked={L.locked || placed}
                      style="left:{b.x}%; top:{b.y}%; width:{b.w}%; height:{b.h}%;"
                      on:pointerdown={(e) => (placed ? (selId = L.id) : startDrag(e, L, 'move'))} role="button" tabindex="0"
                      on:keydown={onCanvasKey} aria-label={layerLabel(L)}>
                      {#if selId === L.id}
                        <span class="te-htag">{layerLabel(L)}{#if L.locked} 🔒{/if}</span>
                        {#if !L.locked && !placed}
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
        <!-- THE OBJECTS ON THIS SLIDE. A wrapping strip, never a scrolling one:
             a tab that has scrolled behind a hidden scrollbar is a tab nobody
             knows is there. -->
        {#if layers.length}
          <div class="te-objtabs" role="tablist" aria-label="Objects on this slide">
            {#each layers as L (L.id)}
              <button
                class="te-objtab"
                class:on={selId === L.id}
                class:off={L.visible === false}
                role="tab"
                aria-selected={selId === L.id}
                on:click={() => (selId = L.id)}
              >{layerLabel(L)}</button>
            {/each}
          </div>
          {#if sel}
            <!-- WHAT THIS OBJECT IS CALLED. The model has carried a `name` since
                 the layer editor was written — `layerLabel` prefers it, a
                 duplicate appends " copy" to it — and NOTHING could set one, so
                 the layer list showed a name an operator could read and never
                 change, and every text object on a slide was called "Verse
                 text" after its binding. Empty falls back to the binding's own
                 label, which is what a fresh object already shows. -->
            <div class="te-frow te-objname">
              <label class="te-fk" for="te-oname">Object name</label>
              <input id="te-oname" class="r-input te-fv" value={sel.name ?? ''} placeholder={layerLabel(sel)}
                on:input={(e) => set('name', e.target.value)} />
            </div>
            <div class="te-objacts">
              <button class="r-btn sm ghost" on:click={() => duplicate(sel.id)}>Duplicate</button>
              <button class="r-btn sm ghost" on:click={() => resetObject(sel.id)}>Reset this object</button>
              <button
                class="r-btn sm danger"
                class:armed={armedDelete === sel.id}
                on:click={() => removeLayer(sel.id)}
                on:blur={() => armedDelete === sel.id && disarmDelete()}
              >{armedDelete === sel.id ? 'Delete — sure?' : 'Delete'}</button>
            </div>
          {/if}
        {/if}
        <div class="te-designbody r-scroll">
          <!-- ══ THE SELECTED OBJECT COMES FIRST (§3.2) ══════════════════════
               This panel used to open on the TEMPLATE — its name, the kinds of
               content it is used for, the kinds it shows — and an operator who
               had just clicked an object on the canvas then had to scroll past
               all of it to reach that object's properties. The tab strip above
               says which object is selected; what follows it is that object,
               and the template's own facts are one section at the foot.

               ONE DEVIATION FROM §3.2, STATED RATHER THAN HIDDEN: the reference
               groups an object as Text / Position / Effects and this renders
               Position FIRST, because Position is the one group every movable
               kind has and there is exactly ONE of it. Putting it between Text
               and Effects would mean emitting the group inside each kind's
               branch — and this file already carries the scar of a duplicated
               Position group that did not agree with its twin and moved a
               locked object. One group in a slightly different place is a much
               smaller problem than two groups. -->
          {#if sel && sel.type !== 'band' && !bandOf(layers, sel.id)}
            <!-- POSITION — ONE GROUP, §3.2. These were reachable only by dragging
                 on the canvas, so a keyboard-only operator could not place an
                 object at all and nobody could place one exactly. Percentages of
                 the frame, like everything else in a template.

                 THERE USED TO BE TWO. A second Position group sat at the bottom of
                 this panel writing the same four keys, and both rendered for every
                 selected object — the same heading twice, over two different number
                 grids. They did not agree: this one clamps to 0–100 and refuses a
                 locked object, that one did neither, so typing into the lower grid
                 moved a layer the operator had locked. The three Centre buttons were
                 the only thing it had that this did not, and they are here now.

                 A BAND AND ITS WORDS ARE NOT HERE, because x/y/w/h is not where any
                 of them sits: a band is placed by `top`/`side` and its words by the
                 band. Four numbers that change nothing is the defect DECISIONS §69
                 closed, so they get the controls that do move them instead. -->
            <h3 class="te-sec">Position</h3>
            <div class="te-geom">
              {#each [['x', 'X'], ['y', 'Y'], ['w', 'W'], ['h', 'H']] as [k, label]}
                <label class="te-geomcell">
                  <span class="r-lbl">{label}</span>
                  <input
                    class="te-num r-mono"
                    type="number"
                    min="0"
                    max="100"
                    step="0.5"
                    value={Math.round((sel[k] ?? 0) * 10) / 10}
                    disabled={sel.locked}
                    on:input={(e) => geom(k, e.target.value)}
                  />
                </label>
              {/each}
            </div>
            <div class="te-alignrow">
              <button class="r-btn ghost te-alignbtn" on:click={() => center('x')} title="Centre horizontally">Centre H</button>
              <button class="r-btn ghost te-alignbtn" on:click={() => center('y')} title="Centre vertically">Centre V</button>
              <button class="r-btn ghost te-alignbtn" on:click={() => center('both')} title="Centre on canvas">Centre</button>
            </div>
            {#if sel.locked}<p class="te-fnote">This object is locked. Unlock it in the layer list to move it.</p>{/if}
            <p class="te-fnote">Percent of the screen. Drag on the canvas — layers snap to centre and edges (hold Shift to place freely) — or type exact values.</p>
          {/if}

          {#if !sel}
            <!-- NOT `te-guide`. That class is the canvas's 1px alignment hairline
                 (position:absolute, background:var(--v-accent)), and this paragraph
                 was silently inheriting all of it: grey text on a solid amethyst bar
                 at about 1.9:1, pulled out of flow across the top of the panel. Two
                 unrelated things, one class name. -->
            <p class="te-fnote te-emptyhint">Select a layer to edit it, or add one with ＋.</p>
          {:else if sel.type === 'background'}
            <h3 class="te-sec">Background</h3>
            <div class="te-frow">
              <label class="te-fk" for="te-fill">Fill</label>
              <span class="te-fv te-swatch"><input id="te-fill" type="color" value={isColor(sel.fill) ? sel.fill : '#0b0906'} on:input={(e) => { set('fill', e.target.value); set('image', null); }} disabled={isThemeToken(sel.fill)} /><span class="te-hex r-mono">{isThemeToken(sel.fill) ? 'linked' : isColor(sel.fill) ? sel.fill.toUpperCase() : 'gradient'}</span></span>
            </div>
            <div class="te-frow"><label class="te-fk" for="te-bgbind">Style link</label><select id="te-bgbind" class="r-select te-fv" value={isThemeToken(sel.fill) ? sel.fill : 'custom'} on:change={(e) => { bindToken('fill', e.target.value, '#0b0906'); if (e.target.value !== 'custom') set('image', null); }}><option value="custom">Custom fill</option>{#each COLOUR_TOKENS as t}<option value={t.token}>{t.label}</option>{/each}</select></div>
            <div class="te-frow">
              <label class="te-fk" for="te-op">Opacity</label>
              <span class="te-fv te-rangerow"><input id="te-op" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} use:rangeFill={sel.opacity ?? 1} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span>
            </div>
            <div class="te-frow">
              <label class="te-fk" for="te-dim">Dim</label>
              <span class="te-fv te-rangerow"><input id="te-dim" class="r-range" type="range" min="0" max="0.9" step="0.05" value={sel.dim || 0} on:input={(e) => num('dim', e.target.value)} use:rangeFill={sel.dim || 0} /><span class="te-rnum r-mono">{Math.round((sel.dim || 0) * 100)}%</span></span>
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
                <button class="te-segbtn" class:on={(sel.fit || 'cover') === 'cover'} on:click={() => set('fit', 'cover')}>Cover</button>
                <button class="te-segbtn" class:on={sel.fit === 'contain'} on:click={() => set('fit', 'contain')}>Contain</button>
              </span>
            </div>
            <div class="te-frow"><label class="te-fk" for="te-mop">Opacity</label><span class="te-fv te-rangerow"><input id="te-mop" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} use:rangeFill={sel.opacity ?? 1} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-mrad">Radius</label><span class="te-fv te-rangerow"><input id="te-mrad" class="r-range" type="range" min="0" max="8" step="0.2" value={sel.radius || 0} on:input={(e) => num('radius', e.target.value)} use:rangeFill={sel.radius || 0} /><span class="te-rnum r-mono">{(sel.radius || 0).toFixed(1)}</span></span></div>
            <p class="te-fnote">Shows the fired picture or video. Empty until media is on screen — a template without a Media layer never shows media on that screen. Put it high in the layer list to cover everything, or low to sit behind the text.</p>
          {:else if sel.type === 'region'}
            <h3 class="te-sec">Slide region</h3>
            <p class="te-fnote">A real rendered slide, in its own container — the template below scales to this box exactly as it would to a screen of that width. Leave the rest of the frame unpainted and the switcher puts the camera behind it.</p>
            <div class="te-frow">
              <label class="te-fk" for="te-rtpl">Shows</label>
              <select id="te-rtpl" class="r-select te-fv" value={sel.templateRef ?? 1} on:change={(e) => num('templateRef', e.target.value)}>
                {#each BUILTINS as b (b.id)}
                  <option value={b.id}>{b.name}</option>
                {/each}
              </select>
            </div>
            <p class="te-fnote">Built-in looks only: a kiosk or OBS page has no database, so a custom template here would render on this wall and not in the stream.</p>
            <div class="te-frow"><label class="te-fk" for="te-rrad">Corner</label><span class="te-fv te-rangerow"><input id="te-rrad" class="r-range" type="range" min="0" max="8" step="0.2" value={sel.radius || 0} on:input={(e) => num('radius', e.target.value)} use:rangeFill={sel.radius || 0} /><span class="te-rnum r-mono">{(sel.radius || 0).toFixed(1)}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-rout">Outline</label><span class="te-fv te-rangerow"><input id="te-rout" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.outline || 0} on:input={(e) => num('outline', e.target.value)} use:rangeFill={sel.outline || 0} /><span class="te-rnum r-mono">{(sel.outline || 0).toFixed(2)}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-routc">Outline colour</label><span class="te-fv te-swatch"><input id="te-routc" type="color" value={isColor(sel.outlineColor) ? sel.outlineColor : '#5b9cf8'} on:input={(e) => set('outlineColor', e.target.value)} disabled={isThemeToken(sel.outlineColor)} /><span class="te-hex r-mono">{isThemeToken(sel.outlineColor) ? 'linked' : isColor(sel.outlineColor) ? sel.outlineColor.toUpperCase() : '#5B9CF8'}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-routb">Style link</label><select id="te-routb" class="r-select te-fv" value={isThemeToken(sel.outlineColor) ? sel.outlineColor : 'custom'} on:change={(e) => bindToken('outlineColor', e.target.value, '#5b9cf8')}><option value="custom">Custom colour</option>{#each COLOUR_TOKENS as t}<option value={t.token}>{t.label}</option>{/each}</select></div>
            <div class="te-frow">
              <label class="te-fk" for="te-rop">Opacity</label>
              <span class="te-fv te-rangerow"><input id="te-rop" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} use:rangeFill={sel.opacity ?? 1} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span>
            </div>
          {:else if sel.type === 'band'}
            <!-- THE BAND (docs/REBRAND.md §4). Its geometry is NOT x/y/w/h: it runs
                 from Top to the bottom edge, inset by Side, and its words sit inside
                 it. These four are the numbers that actually move it. -->
            <h3 class="te-sec">Position</h3>
            <div class="te-frow"><label class="te-fk" for="te-btop">Top</label><span class="te-fv te-rangerow"><input id="te-btop" class="r-range" type="range" min="50" max="95" step="1" value={sel.top ?? 74} on:input={(e) => num('top', e.target.value)} use:rangeFill={((sel.top ?? 74) - 50) / 45} /><span class="te-rnum r-mono">{Math.round(sel.top ?? 74)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-bside">Side</label><span class="te-fv te-rangerow"><input id="te-bside" class="r-range" type="range" min="0" max="20" step="0.5" value={sel.side ?? 6} on:input={(e) => num('side', e.target.value)} use:rangeFill={(sel.side ?? 6) / 20} /><span class="te-rnum r-mono">{(sel.side ?? 6).toFixed(1)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-bpad">Inner</label><span class="te-fv te-rangerow"><input id="te-bpad" class="r-range" type="range" min="0" max="12" step="0.5" value={sel.pad ?? 3} on:input={(e) => num('pad', e.target.value)} use:rangeFill={(sel.pad ?? 3) / 12} /><span class="te-rnum r-mono">{(sel.pad ?? 3).toFixed(1)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-blift">Lift</label><span class="te-fv te-rangerow"><input id="te-blift" class="r-range" type="range" min="0" max="15" step="0.5" value={sel.lift ?? 3} on:input={(e) => num('lift', e.target.value)} use:rangeFill={(sel.lift ?? 3) / 15} /><span class="te-rnum r-mono">{(sel.lift ?? 3).toFixed(1)}%</span></span></div>
            <p class="te-fnote">The band runs from <b>Top</b> to the bottom edge. <b>Lift</b> holds its words off the baseline; the words are centred in what is left.</p>

            <h3 class="te-sec">Effects</h3>
            <div class="te-frow"><label class="te-fk" for="te-bfill">Fill</label><span class="te-fv te-swatch"><input id="te-bfill" type="color" value={isColor(sel.fill) ? sel.fill : '#101319'} on:input={(e) => set('fill', e.target.value)} disabled={isThemeToken(sel.fill)} /><span class="te-hex r-mono">{isThemeToken(sel.fill) ? 'linked' : isColor(sel.fill) ? sel.fill.toUpperCase() : 'gradient'}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-bfillbind">Style link</label><select id="te-bfillbind" class="r-select te-fv" value={isThemeToken(sel.fill) ? sel.fill : 'custom'} on:change={(e) => bindToken('fill', e.target.value, '#101319')}><option value="custom">Custom fill</option>{#each COLOUR_TOKENS as t}<option value={t.token}>{t.label}</option>{/each}</select></div>
            <!-- OPACITY MEANS WHAT IT SAYS (§4). The band's body sits at exactly the
                 alpha set here — nothing multiplies it down on the way to the wall. -->
            <div class="te-frow"><label class="te-fk" for="te-bop">Opacity</label><span class="te-fv te-rangerow"><input id="te-bop" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} use:rangeFill={sel.opacity ?? 1} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-brad">Radius</label><span class="te-fv te-rangerow"><input id="te-brad" class="r-range" type="range" min="0" max="8" step="0.2" value={sel.radius || 0} on:input={(e) => num('radius', e.target.value)} use:rangeFill={(sel.radius || 0) / 8} /><span class="te-rnum r-mono">{(sel.radius || 0).toFixed(1)}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-bgrow">Gives ground</label><span class="te-fv te-rangerow"><input id="te-bgrow" class="r-range" type="range" min="0" max="16" step="1" value={sel.grow ?? BAND_GROW_MAX} on:input={(e) => num('grow', e.target.value)} use:rangeFill={(sel.grow ?? BAND_GROW_MAX) / 16} /><span class="te-rnum r-mono">{Math.round(sel.grow ?? BAND_GROW_MAX)} pts</span></span></div>
            <p class="te-fnote">A long line makes the band climb — up to this many points, never past a third of the screen — before the type shrinks below {Math.round(BAND_TYPE_FLOOR * 100)}% of its set size. A short one does not move it. Set 0 to keep the band still and let the words shrink.</p>

            <span class="r-lbl te-showlbl">Words in this band</span>
            {#if bandWords.length}
              <div class="te-showgrid">
                {#each bandWords as m (m.id)}
                  <button class="te-showchip" on:click={() => (selId = m.id)}>{layerLabel(m)}</button>
                {/each}
              </div>
            {:else}
              <p class="te-fnote">No words yet. Add a text object, then <b>Put in band</b> from its own panel.</p>
            {/if}
          {:else if sel.type === 'shape'}
            <h3 class="te-sec">Shape</h3>
            <div class="te-frow"><label class="te-fk" for="te-sfill">Fill</label><span class="te-fv te-swatch"><input id="te-sfill" type="color" value={isColor(sel.fill) ? sel.fill : '#101319'} on:input={(e) => set('fill', e.target.value)} disabled={isThemeToken(sel.fill)} /><span class="te-hex r-mono">{isThemeToken(sel.fill) ? 'linked' : isColor(sel.fill) ? sel.fill.toUpperCase() : '#101319'}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-sfillbind">Style link</label><select id="te-sfillbind" class="r-select te-fv" value={isThemeToken(sel.fill) ? sel.fill : 'custom'} on:change={(e) => bindToken('fill', e.target.value, '#101319')}><option value="custom">Custom fill</option>{#each COLOUR_TOKENS as t}<option value={t.token}>{t.label}</option>{/each}</select></div>
            <div class="te-frow"><label class="te-fk" for="te-sop">Opacity</label><span class="te-fv te-rangerow"><input id="te-sop" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.opacity ?? 1} on:input={(e) => num('opacity', e.target.value)} use:rangeFill={sel.opacity ?? 1} /><span class="te-rnum r-mono">{Math.round((sel.opacity ?? 1) * 100)}%</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-srad">Radius</label><span class="te-fv te-rangerow"><input id="te-srad" class="r-range" type="range" min="0" max="8" step="0.2" value={sel.radius || 0} on:input={(e) => num('radius', e.target.value)} use:rangeFill={sel.radius || 0} /><span class="te-rnum r-mono">{(sel.radius || 0).toFixed(1)}</span></span></div>
          {:else}
            <!-- text / timer -->
            {#if bands.length}
              <!-- WHICH BAND THIS BELONGS TO, said in one place. Membership lives on
                   the band as a list of ids, so this row writes the band, not the
                   word — a word cannot be in two bands, and taking it out of one is
                   the same operation as putting it in another. -->
              <h3 class="te-sec">Band</h3>
              <div class="te-frow">
                <label class="te-fk" for="te-inband">In band</label>
                <select id="te-inband" class="r-select te-fv" value={selBand ? selBand.id : ''} on:change={(e) => setBandMembership(sel.id, e.target.value)}>
                  <option value="">Not in a band</option>
                  {#each bands as b (b.id)}<option value={b.id}>{layerLabel(b)}</option>{/each}
                </select>
              </div>
              {#if selBand}
                <div class="te-frow"><label class="te-fk" for="te-bh">Height</label><span class="te-fv te-rangerow"><input id="te-bh" class="r-range" type="range" min="2" max="24" step="0.5" value={sel.h ?? 10} on:input={(e) => num('h', e.target.value)} use:rangeFill={((sel.h ?? 10) - 2) / 22} /><span class="te-rnum r-mono">{(sel.h ?? 10).toFixed(1)}%</span></span></div>
                <p class="te-fnote">Placed by <b>{layerLabel(selBand)}</b>: the band sets where this line sits and how wide it is. Height is its share of the band, and the band grows it when it gives ground.</p>
              {/if}
            {/if}
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
                <option value="theme:font">Template typeface</option>
                {#if sel.font && sel.font !== 'theme:font' && !fonts.includes(sel.font)}<option value={sel.font}>{fontLabel(sel.font)}</option>{/if}
                {#each fonts as f}<option value={f}>{f}</option>{/each}
              </select>
            </div>
            <button class="r-btn quiet sm te-minilink" on:click={() => detectFonts(false)}>Use all computer fonts {fontMsg}</button>
            {#if missingFont}<p class="te-fwarn">“{fontLabel(missingFont)}” isn't installed here — outputs use a default. Install it to use it.</p>{/if}
            <div class="te-frow"><label class="te-fk" for="te-size">Size</label><span class="te-fv te-stepper"><input id="te-size" class="te-num r-mono" type="number" min="1" max="16" step="0.1" value={sel.size} on:input={(e) => num('size', e.target.value)} /><span class="te-unit r-mono">cqw</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-col">Colour</label><span class="te-fv te-swatch"><input id="te-col" type="color" value={isColor(sel.color) ? sel.color : '#ffffff'} on:input={(e) => set('color', e.target.value)} disabled={isThemeToken(sel.color)} /><span class="te-hex r-mono">{isThemeToken(sel.color) ? 'linked' : isColor(sel.color) ? sel.color.toUpperCase() : '#FFFFFF'}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-colbind">Style link</label><select id="te-colbind" class="r-select te-fv" value={isThemeToken(sel.color) ? sel.color : 'custom'} on:change={(e) => bindToken('color', e.target.value, '#ffffff')}><option value="custom">Custom colour</option>{#each COLOUR_TOKENS as t}<option value={t.token}>{t.label}</option>{/each}</select></div>
            <div class="te-frow">
              <span class="te-fk">Align</span>
              <span class="te-fv te-seg">
                <!-- "Align left" was this button's name AND the canvas strip's, and the two
                     do different things: the strip moves the OBJECT, this sets how the WORDS
                     sit inside it. Two identical commands in one view is a defect you can only
                     hear, and `qa-inventory` counts both as named. Say which one this is. -->
                {#each ['left', 'center', 'right'] as a}
                  <button class="te-segbtn" class:on={sel.align === a} aria-label={TEXT_ALIGN_LABEL[a]} on:click={() => set('align', a)}>
                    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M4 6h16"/><path d={a === 'left' ? 'M4 12h10' : a === 'right' ? 'M10 12h10' : 'M7 12h10'}/><path d="M4 18h16"/></svg>
                  </button>
                {/each}
              </span>
            </div>
            <div class="te-frow">
              <span class="te-fk">V-align</span>
              <span class="te-fv te-seg">
                <!-- These three were named `T`, `M` and `B` — the letter on the face, which is
                     a label to a reader with the row's `V-align` beside it and nothing at all
                     to a screen reader arriving at the button. -->
                {#each ['top', 'middle', 'bottom'] as v}
                  <button class="te-segbtn" class:on={(sel.valign || 'middle') === v} aria-label={VALIGN_LABEL[v]} on:click={() => set('valign', v)}>{v[0].toUpperCase()}</button>
                {/each}
              </span>
            </div>
            <div class="te-frow">
              <label class="te-fk" for="te-case">Caps</label>
              <select id="te-case" class="r-select te-fv" value={sel.transform || 'none'} on:change={(e) => set('transform', e.target.value)}>
                <option value="none">As typed</option><option value="uppercase">UPPERCASE</option><option value="lowercase">lowercase</option><option value="capitalize">Capitalize</option>
              </select>
            </div>
            <div class="te-frow"><label class="te-fk" for="te-lh">Line height</label><span class="te-fv te-rangerow"><input id="te-lh" class="r-range" type="range" min="0.9" max="2" step="0.05" value={sel.lineHeight || 1.32} on:input={(e) => num('lineHeight', e.target.value)} use:rangeFill={sel.lineHeight || 1.32} /><span class="te-rnum r-mono">{(sel.lineHeight || 1.32).toFixed(2)}</span></span></div>
            <div class="te-frow"><label class="te-fk" for="te-ls">Spacing</label><span class="te-fv te-rangerow"><input id="te-ls" class="r-range" type="range" min="-0.05" max="0.4" step="0.01" value={sel.letterSpacing || 0} on:input={(e) => num('letterSpacing', e.target.value)} use:rangeFill={sel.letterSpacing || 0} /><span class="te-rnum r-mono">{(sel.letterSpacing || 0).toFixed(2)}em</span></span></div>
            <h3 class="te-sec">Effects</h3>
            <div class="te-frow"><label class="te-fk" for="te-sh">Shadow</label><span class="te-fv te-rangerow"><input id="te-sh" class="r-range" type="range" min="0" max="1" step="0.05" value={sel.shadow || 0} on:input={(e) => num('shadow', e.target.value)} use:rangeFill={sel.shadow || 0} /><span class="te-rnum r-mono">{Math.round((sel.shadow || 0) * 100)}%</span></span></div>
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

          <!-- ══ AND THE TEMPLATE ITSELF, UNDERNEATH ═════════════════════════
               Unchanged controls; what moved is where they sit. The rule above
               the heading is a real boundary: everything above it is about one
               object on this slide, everything below it is about the template
               all of them belong to. -->
          <h3 class="te-sec te-templatesec">Template</h3>
          <div class="te-frow"><label class="te-fk" for="te-name">Name</label><input id="te-name" class="r-input te-fv" bind:value={edit.name} /></div>
          <!-- ── TWO REGISTERS OF FIVE CHIPS, AND THEY ARE NOT THE SAME FACT ──
               Found by agent S2 while auditing the gallery, and reported here
               because both live in this file. They read as one fact printed
               twice because they were two identical neutral chip rows over the
               same five `CONTENT_KINDS` labels — and because the paragraph
               explaining the SECOND one was attached to the FIRST, so the next
               reader inherited the same confusion the render produced. The
               comment is now on the register it describes.

                 · USED FOR is a GLOBAL BINDING, written by `setContentTemplate`
                   (DECISIONS §70): when scripture fires, every screen set to
                   *Follow the content look* wears THIS template. It says nothing
                   about how this template renders.
                 · The one below is a PER-TEMPLATE FILTER on `layout.shows`, read
                   at runtime by `Output.svelte` and `layers.js::templateShows`.

               The label below was **"Shows on this screen"**, and the word
               *screen* was the damage: the thing in hand is a TEMPLATE, and
               several screens can wear it. `Used for` keeps its name — it is a
               term of art carried by `docs/REBRAND.md` §3.3, DECISIONS §70, the
               gallery card and `inspectorobjects.test.js`, and renaming it here
               alone would make two surfaces call one binding two things. -->
          <span class="r-lbl te-showlbl">Used for</span>
          <div class="te-showgrid">
            {#each CONTENT_KINDS as k}
              <button
                class="te-showchip"
                class:on={$contentTemplates[k.key] === edit.id}
                on:click={() => toggleUsedFor(k.key)}
              >
                <span class="te-showtick" aria-hidden="true">{$contentTemplates[k.key] === edit.id ? '✓' : ''}</span>{k.label}
              </button>
            {/each}
          </div>
          <p class="te-fnote">A kind ticked here wears this template on every screen set to <b>Follow the content look</b>. A screen with a look of its own keeps it.</p>
          {#if lookErr}<p class="te-fwarn" role="alert">{lookErr}</p>{/if}

          <!-- WHAT A SCREEN WEARING THIS TEMPLATE WILL RENDER. An online wall
               shows everything; a stage / confidence monitor might show only
               scripture, songs and the timer — when a picture or an announcement
               fires, a screen wearing this template ignores it and holds what it
               had. This is the paragraph that used to sit over `Used for`. -->
          <span class="r-lbl te-showlbl">Content this template renders</span>
          <div class="te-showgrid">
            {#each CONTENT_KINDS as k}
              <button class="te-showchip" class:on={templateShows(edit, k.key)} on:click={() => toggleShows(k.key)}>
                <span class="te-showtick" aria-hidden="true">{templateShows(edit, k.key) ? '✓' : ''}</span>{k.label}
              </button>
            {/each}
          </div>
          <p class="te-fnote">An unticked kind is not blanked — a screen wearing this template holds what it already had.</p>
        </div>
        {#if err}<div class="te-err" role="alert">{err}</div>{/if}
      </aside>
    </div>
  {/if}
</div>

<style>
  /* READABILITY. A problem is rose; something Relay CANNOT check is dim and
     italic — the same treatment the language table gives an absence, because it is
     the same kind of answer: nobody has failed, the question cannot be answered
     from here. */
  /* A DISCLOSURE ROW, not a button — the folded readability panel's own heading
     with its verdict on the right and a chevron at the end. Full width, no fill
     and no edge at rest, because it is the heading of the block below it rather
     than an action beside it; hovering gives it the panel's hover ground so it
     reads as pressable. Named so a shape census can tell it from a button that
     lost its class. */
  .te-legtoggle{ display:flex; align-items:center; gap:8px; width:100%; margin-top:2px;
    padding:8px 12px; border:0; border-top:1px solid var(--v-line); background:none;
    color:var(--v-dim); cursor:pointer; text-align:left; }
  .te-legtoggle:hover{ background:var(--v-surf2); }
  .te-legtoggle .r-lbl{ flex:0 0 auto; }
  /* THE VERDICT, in the one line that is always on screen. Rose when there is
     something to look at, dim italic when Relay CANNOT answer (a photograph
     behind the words), neither when it is clean — three different sentences for
     three different situations, which is what makes it a status line and not a
     label (rule 35). */
  .te-legsum{ flex:1; min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    text-align:right; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-legsum.bad{ color:var(--v-rose); }
  .te-legsum.unknown{ font-style:italic; }
  .te-legchev{ flex:0 0 auto; font-size:var(--v-fs-fig); color:var(--v-faint); }
  .te-leg{ list-style:none; margin:6px 0 0; padding:0 12px; display:flex;
    flex-direction:column; gap:6px; }
  .te-legrow{ display:flex; flex-direction:column; gap:1px; }
  .te-legrow b{ font-size:var(--v-fs-cap); color:var(--v-txt); font-weight:600; }
  .te-legrow span{ font-size:var(--v-fs-cap); color:var(--v-dim); line-height:1.4; }
  .te-legrow.bad span{ color:var(--v-rose); }
  .te-legrow.unknown span{ color:var(--v-faint); font-style:italic; }
  /* The opened panel indents to the same 12px gutter the pane's note uses. It
     had none, so the two number fields sat flush against the pane's border. */
  .te-legroom{ display:grid; grid-template-columns:1fr auto; gap:6px 8px;
    align-items:center; margin-top:10px; padding:0 12px; }
  .te-leglab{ font-size:var(--v-fs-cap); color:var(--v-dim); }
  .te-legin{ width:70px; text-align:right; }
  .te-legbtn{ margin:10px 12px 0; width:calc(100% - 24px); }
  .te-dists{ display:flex; gap:12px; margin-bottom:12px; flex-wrap:wrap; }
  .te-dist{ margin:0; }
  .te-distbox{ width:190px; aspect-ratio:16/9; overflow:hidden; position:relative;
    background:#000; border:1px solid var(--v-line); border-radius:var(--v-r-sm); }
  .te-distinner{ position:absolute; inset:0; transform-origin:center; }
  .te-dist figcaption{ font-size:var(--v-fs-b3); color:var(--v-faint); text-align:center;
    margin-top:4px; }

  .te-shell{ display:flex; flex-direction:column; height:100%; min-height:0; gap:12px; }
  .te-layers{ overflow-y:auto; }
  .te-spring{ flex:1; }
  /* WRAPS. Unwrapped, this row was 1184px of controls in 1158px at a 1440-wide
     window, and the one it pushed off the right edge was **Save Template** — the
     primary action of the screen, unreachable with a mouse. */
  .te-top{ display:flex; align-items:center; gap:10px; flex:0 0 auto; flex-wrap:wrap; row-gap:8px; }
  .te-name{ font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .te-histwrap{ position:relative; }
  .te-histmenu{ position:absolute; top:34px; right:0; z-index:60; width:230px; max-height:320px; overflow-y:auto;
    background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:var(--v-r-md);
    box-shadow:var(--v-shadow-lg); padding:5px; }
  .te-histhead{ padding:6px 8px 4px; }
  /* A MENU ROW, not a button — one saved version per row inside the History
     [role="menu"], full width, "Latest" at one end and "Restore" at the other. */
  .te-histitem{ display:flex; align-items:center; justify-content:space-between; gap:10px; width:100%;
    text-align:left; padding:8px 10px; border:0; background:none; color:var(--v-txt); border-radius:var(--v-r-sm); cursor:pointer; }
  .te-histitem:hover{ background:var(--v-surf3); }
  .te-histwhen{ font-size:var(--v-fs-b2); }
  .te-histrestore{ font-size:var(--v-fs-cap); color:var(--v-accent); }
  .te-histempty{ padding:9px 10px; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .te-histempty b{ color:var(--v-dim); }
  .te-sub{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-undo{ display:inline-flex; align-items:center; gap:2px; margin-left:10px; }
  .te-zoom{ display:flex; align-items:center; gap:4px; }
  /* CONVERTED — B2. Undo · Redo · zoom out · zoom in were a hand-rolled 26px
     square drawing a `--v-line2` DIVIDER hairline where every other icon button
     in the product draws `.r-iconbtn`'s. Same shape, different edge, in a bar
     that also carries six `.r-btn`s. The `--v-fs-ttl` the zoom glyphs carried
     went with it: `−` and `+` are type where Undo and Redo are svg, and a
     control that is one size when its icon is a character and another when it
     is a path is the same drift one level down. What is left is the line box,
     which is not a size. */
  .te-zbtn{ line-height:1; }
  .te-zbtn:disabled{ opacity:.4; cursor:not-allowed; }
  .te-pct{ min-width:42px; text-align:center; font-size:var(--v-fs-cap); color:var(--v-dim); }
  .r-btn.confirm{ background:var(--v-emerald); color:var(--v-void); border-color:transparent; }
  .r-btn.confirm:hover:not(:disabled){ filter:brightness(1.08); }
  .r-btn.ghost.on{ background:var(--v-surf3); color:var(--v-txt); border-color:var(--v-line2); }

  /* THE LAYER RAIL IS WIDER, and the canvas pays for it. At 222px the name cell
     was ~72px: `Refere…`, `Backgr…`, on a three-layer template, which is the
     simplest one that exists. The rail is the column an operator opens this
     screen to use; the artboard loses 42px out of about a thousand and is still
     the largest thing on the desk. */
  .te-body{ flex:1; min-height:0; display:grid; grid-template-columns:264px minmax(0,1fr) 300px; gap:12px; }
  @media (max-width:1180px){ .te-body{ grid-template-columns:216px minmax(0,1fr) 268px; } }
  @media (max-width:980px){ .te-shell{ height:auto; } .te-body{ grid-template-columns:1fr; } }

  .te-pane{ display:flex; flex-direction:column; min-height:0; overflow:hidden; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  .te-panehead{ display:flex; align-items:center; justify-content:space-between; gap:8px; padding:11px 13px; border-bottom:1px solid var(--v-line); flex:0 0 auto; }
  /* The object strip WRAPS. A tab that has scrolled out of sight behind a
     hidden scrollbar is a tab nobody knows is there. */
  .te-objtabs{ display:flex; flex-wrap:wrap; gap:3px; padding:7px 9px 0; }
  /* A TAB, not a button. `role="tab"` inside a `role="tablist"`, and the strip
     WRAPS rather than scrolls (see the markup). A tab is sized by its label and
     carries a selected state that a button variant does not have. */
  .te-objtab{ padding:3px 8px; border-radius:var(--v-r-sm); border:1px solid var(--v-line2);
    background:var(--v-surf2); color:var(--v-dim); font-size:var(--v-fs-lbl); font-weight:600;
    cursor:pointer; max-width:100%; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .te-objtab:hover:not(.on){ color:var(--v-txt); background:var(--v-surf3); }
  .te-objtab.on{ background:var(--v-sel-fill); border-color:transparent; color:var(--v-sel-ink); }
  .te-objtab.off{ text-decoration:line-through; opacity:.6; }
  .te-objname{ padding:7px 9px 0; }
  .te-objacts{ display:flex; flex-wrap:wrap; gap:5px; padding:7px 9px 0; }
  .te-objacts .armed{ background:var(--v-red); border-color:transparent; color:#fff; }
  .te-geom{ display:grid; grid-template-columns:repeat(4, 1fr); gap:5px; padding:0 0 4px; }
  .te-geomcell{ display:flex; flex-direction:column; gap:2px; min-width:0; }
  .te-geomcell input{ width:100%; }
  .te-designfor{ font-size:var(--v-fs-cap); color:var(--v-accent2); }

  /* layers panel */
  .te-addwrap{ position:relative; }
  /* CONVERTED — B2. The ＋ that adds a layer was a 24px accent-filled square
     against `.te-zbtn`'s 26px: two icon-button shapes in one file, one of them
     also the only accent-filled square in the product. Keeping the fill was
     tried and dropped — a component that repaints a shared control has invented
     a variant nobody else can use, and "add a layer" is not more consequential
     than the Undo two rows away. It is now exactly `.r-iconbtn`, and nothing is
     left but the line box the `＋` glyph needs. */
  .te-addbtn{ line-height:1; }
  /* A DROPDOWN MENU, not a row of buttons. Full-bleed rows inside a floating
     [role="menu"] — they have no edge and no fill of their own because the menu
     is the surface. Named so the next shape census can tell this from drift. */
  .te-addmi{ display:flex; align-items:center; gap:9px; text-align:left; padding:7px 9px; border:0; background:none; color:var(--v-txt); font-size:var(--v-fs-b2); border-radius:var(--v-r-sm); cursor:pointer; }
  .te-addmi:hover{ background:var(--v-surf3); }
  .te-addmenu{ position:absolute; top:28px; right:0; z-index:30; width:186px; background:var(--v-surf2); border:1px solid var(--v-line2); border-radius:var(--v-r-md); box-shadow:var(--v-shadow-lg); padding:5px; display:flex; flex-direction:column; }
  .te-addico{ width:16px; text-align:center; color:var(--v-faint); font-family:var(--f-mono); }
  .te-addsec{ padding:6px 8px 3px; }

  /* The layer list is what this panel is FOR. As `flex:1` it took whatever the
     readability block below it left over — 75px for 138px of layers, so a
     three-layer template showed one and a half rows. It keeps its own scroll and a
     floor of four rows; the panel scrolls for the rest. */
  /* The list now takes what is left of the panel rather than a fixed slice of
     the viewport: the readability block underneath it folds to one row, so the
     thing that used to crowd it is 30px tall until it is asked for. Four rows'
     worth of floor, and the panel scrolls past that. */
  .te-layerlist{ flex:1 1 auto; min-height:152px; overflow-y:auto; padding:8px; display:flex; flex-direction:column; gap:4px; }
  /* ── A LAYER ROW, as ProPresenter draws one ───────────────────────────────
     A LIST ROW, not a button. Two lines: the object's NAME on the first, with
     the two state toggles that are always true of it (shown / locked); what
     KIND it is on the second, with the reorder and delete affordances revealed
     over it on hover. The name gets a whole line because the previous row put
     five 20px buttons beside it and left ~72px — `Refere…` on a template with
     three layers. Ellipsis is still there for a name that is genuinely too long;
     it is no longer there for `Reference`. */
  .te-layer{ display:grid; grid-template-columns:12px 38px minmax(0,1fr); grid-template-rows:auto auto;
    align-items:center; column-gap:7px; row-gap:2px; padding:7px 9px; border-radius:var(--v-r-md);
    background:var(--v-surf2); border:1px solid var(--v-line); cursor:pointer; transition:.12s; }
  .te-layer:hover{ border-color:var(--v-line2); }
  .te-layer.sel{ border-color:var(--v-accent-line); background:var(--v-accent-soft); }
  .te-layer.off{ opacity:.5; }
  /* THE ROW BEING CARRIED, and the row it would land on. The drop line is drawn
     on the target rather than between rows: a 1px gap between cards is not a
     target anybody can hit, and an indicator that needs its own hit area is an
     indicator that changes where the drop goes. */
  .te-layer.dragging{ opacity:.45; }
  .te-layer.dropto{ border-color:var(--v-accent); box-shadow:0 0 0 1px var(--v-accent); }
  /* A WORD INSIDE A BAND. Indented under the band that owns it, on the band's
     own surface rather than a card of its own, because it is not a peer of the
     things in the stack — nothing draws it from there. The rail keeps the
     nesting legible when the list is long. */
  .te-layer.inband{ margin-left:14px; background:var(--v-surf); border-left:2px solid var(--v-line2); }
  .te-layer.inband.sel{ border-left-color:var(--v-accent-line); }
  /* THE GRIP, spanning both lines — the part of the row that says it can be
     picked up. `draggable` is on the row, not on this, because a 12px drag
     target is a drag nobody starts by accident and also one nobody starts on
     purpose; this marks where to take hold of it. */
  .te-lgrip{ grid-column:1; grid-row:1 / span 2; align-self:stretch; display:grid; place-items:center;
    color:var(--v-faint); opacity:.45; font-size:var(--v-fs-lbl); cursor:grab; line-height:1; }
  .te-layer:hover .te-lgrip{ opacity:1; }
  .te-layer.dragging .te-lgrip{ cursor:grabbing; }
  /* THE PLACEMENT PROXY, spanning both lines. A 16:9 frame the colour of the
     stage with ONE box in it — where this object sits, in its own colour. It is
     deliberately not a slide: `TemplateRender` is the one renderer and a 38px
     imitation of it would be a fork that is wrong where nobody can see. */
  .te-lthumb{ grid-column:2; grid-row:1 / span 2; position:relative; width:38px; aspect-ratio:16/9;
    border-radius:var(--v-r-sm); overflow:hidden; background:var(--v-void);
    box-shadow:inset 0 0 0 1px var(--v-line); }
  .te-lthumbbox{ position:absolute; box-sizing:border-box; border-radius:1px;
    display:flex; flex-direction:column; justify-content:center; gap:1px; padding:0 1px; }
  .te-lthumbbox.hollow{ box-shadow:inset 0 0 0 1px var(--v-line2); }
  /* A TEXT object shows as two bars in its own ink rather than as a filled
     rectangle — at this size a solid block and a paragraph look identical, and
     only one of them is what a text layer paints. */
  .te-lthumbbox.words{ box-shadow:inset 0 0 0 1px var(--v-line); }
  .te-lthumbbox i{ display:block; height:1px; border-radius:1px; opacity:.85; }
  .te-lthumbbox i:last-child{ width:60%; }
  /* The two toggles sit in the SAME grid cell as the name, pinned to its right
     end, and the name reserves exactly their width. They are not flex siblings
     because a flexed name would still be measured against a 1fr track and would
     truncate before it needed to; a reserved gutter truncates at the real edge.
     The two numbers below are the widths of what is in those gutters (2 and 3
     `.te-lmini` at 20px, plus the gaps) and nothing else. */
  .te-lname{ grid-column:3; grid-row:1; min-width:0; padding-right:46px; font-size:var(--v-fs-b2); color:var(--v-txt);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  /* SHOWN and LOCKED are states, not actions, so they sit on the name line and
     are always visible. A hidden layer whose eye only appears on hover is a
     layer an operator has to go looking for. */
  .te-lstate{ grid-column:3; grid-row:1; justify-self:end; display:flex; gap:1px; }
  .te-lkind{ grid-column:3; grid-row:2; display:flex; align-items:center; gap:5px; min-width:0; padding-right:68px; }
  .te-ltype{ color:var(--v-faint); font-family:var(--f-mono); font-size:var(--v-fs-fig); flex:0 0 auto; }
  .te-lkindtxt{ min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    font-size:var(--v-fs-kind); letter-spacing:.06em; text-transform:uppercase; color:var(--v-faint); }
  /* The actions share the second line with the kind label and cover it on hover.
     They are revealed rather than resident because five of them resident is a
     toolbar per row, and because what they do is undoable while the two states
     above are what the object IS. */
  .te-lbtns{ grid-column:3; grid-row:2; justify-self:end; display:flex; gap:1px;
    opacity:0; transition:opacity .12s; }
  .te-layer:hover .te-lbtns, .te-layer.sel .te-lbtns{ opacity:1; }
  /* A ROW AFFORDANCE, not a button — shown · locked · forward · back · delete,
     20px, inside a two-line list row. The shared button would not fit, and
     giving each one a fill and an edge would turn every layer row into a
     toolbar. The armed `Sure?` state is the two-step delete (rule 41). */
  .te-lmini{ width:20px; height:20px; display:grid; place-items:center; border:0; background:none; color:var(--v-faint); cursor:pointer; border-radius:var(--v-r-sm); font-size:var(--v-fs-lbl); }
  .te-lmini:hover{ color:var(--v-txt); background:var(--v-surf3); }
  .te-lmini.danger:hover{ color:var(--v-rose); }
  /* A hidden layer's eye is dimmer than the rest of the row is, so "hidden"
     reads from the control as well as from the row's opacity. */
  .te-lmini.dim{ color:var(--v-faint); opacity:.6; }
  /* A locked layer's button stays lit even at rest, so the lock state reads at a
     glance without hovering the row.
     NOT AMBER (rule 18, DECISIONS §21). Amber means ON AIR and nothing else, and a
     locked layer is inert — the opposite of a claim about a congregation's screen.
     Full-strength text against the `.dim` siblings is what makes it read as lit;
     the contrast is the signal, and it does not spend a law colour to get it. */
  .te-lmini.on{ color:var(--v-txt); opacity:1; }
  .te-hint{ padding:14px 8px; text-align:center; font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-panenote{ margin:0; padding:10px 12px; border-top:1px solid var(--v-line); flex:0 0 auto; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }

  /* canvas */
  .te-canvas{ display:flex; flex-direction:column; min-height:0; overflow:hidden; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); }
  /* THE ALIGNMENT STRIP. A toolbar across the head of the canvas — the six edges
     and the two spacings — wrapping rather than clipping, because it sits above
     an artboard that is already the widest thing on the desk. Every control in it
     is a shared instrument (`.r-iconbtn`, `.r-btn ghost sm`); the only local
     rules are position and the hairline between the two halves. */
  .te-alignbar{ flex:0 0 auto; display:flex; align-items:center; flex-wrap:wrap; gap:4px;
    padding:8px 12px; border-bottom:1px solid var(--v-line); }
  .te-alignlbl{ margin-right:4px; }
  .te-abtn:disabled, .te-abtn2:disabled{ opacity:.4; cursor:not-allowed; }
  .te-abtn2{ margin-left:2px; }
  .te-adiv{ width:1px; align-self:stretch; margin:0 6px; background:var(--v-line2); }
  /* ProPresenter-clean canvas: a flat, calm dark stage with a soft vignette for
     depth — no busy grid competing with the artboard. */
  /* `--v-void`, not a hand-picked hex. This was `#141417` — one step off the
     token, imperceptibly. `--v-void` is already documented as "shell + main +
     the output-window canvas", which is exactly what a stage is: the dark a
     slide is judged against. NOTE: this file is an editor, not a desk, so it is not in
     that test's DESKS array and nothing catches a literal here — the array
     means "is in the workspace grammar", and stretching it to cover one hex
     would weaken what it says. */
  .te-stage{ flex:1; min-height:0; display:flex; align-items:center; justify-content:center; padding:var(--v-sp-lg); overflow:auto; position:relative; background:var(--v-void); }
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
  /* Same rule 18 correction, and this one was a THIRD amber: a raw rgba(255,196,0)
     beside the token's #ffa31a and the #ffb000 elsewhere. A locked box is not
     alarming, it is inert, so it takes a neutral rule rather than a law colour. */
  .te-hbox.locked{ pointer-events:none; border-style:dotted; border-color:var(--v-line2); cursor:default; }
  .te-htag{ position:absolute; top:-16px; left:0; font-family:var(--f-mono); font-size:8px; letter-spacing:.04em; color:#fff; background:var(--v-accent-fill); padding:1px 5px; border-radius:var(--v-r-sm); white-space:nowrap; }
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
  /* THE BOUNDARY between one object and the template it belongs to. A rule, not
     a gap: everything above it is about the selected object and everything below
     it is about the whole template, and that distinction is the point of having
     moved this section down here at all. */
  .te-templatesec{ margin-top:18px; padding-top:14px; border-top:1px solid var(--v-line); }
  .te-frow{ display:grid; grid-template-columns:64px minmax(0,1fr); align-items:center; gap:10px; }
  .te-fk{ font-size:var(--v-fs-b2); color:var(--v-dim); }
  .te-fv{ min-width:0; }
  .te-fnote{ font-size:var(--v-fs-cap); color:var(--v-faint); margin:0; line-height:1.5; }
  .te-emptyhint{ color:var(--v-dim); }
  /* CONVERTED — B2. "Use all computer fonts" was a mono, letter-spaced text
     link sitting directly under a `.r-select` in a column of `.te-frow`s. That
     is exactly what `.r-btn.quiet` is: no fill, no edge, but the button's
     metrics, so it lines up with the column instead of floating in it. Left
     here: the alignment, because a quiet button centres its label by default
     and this one begins under the select's left edge. */
  .te-minilink{ justify-content:flex-start; padding-left:0; align-self:flex-start; }
  .te-fwarn{ margin:0; padding:8px 10px; border:1px solid var(--v-amber-soft); border-radius:var(--v-r-sm); background:var(--v-amber-soft); color:var(--v-amber2); font-size:var(--v-fs-cap); line-height:1.45; }
  .te-stepper{ display:flex; align-items:center; }
  .te-num{ height:32px; padding:0 8px; border-radius:var(--v-r-md); background:var(--v-bg); border:1px solid var(--v-line2); color:var(--v-txt); font-size:var(--v-fs-b2); outline:none; width:100%; box-sizing:border-box; }
  .te-num:focus{ border-color:var(--v-accent-line); }
  .te-stepper .te-num{ border-radius:var(--v-r-md) 0 0 var(--v-r-md); border-right:0; }
  .te-unit{ flex:0 0 auto; height:32px; display:grid; place-items:center; padding:0 9px; border-radius:0 var(--v-r-md) var(--v-r-md) 0; background:var(--v-surf2); border:1px solid var(--v-line2); font-size:var(--v-fs-fig); color:var(--v-faint); }
  .te-rangerow{ display:flex; align-items:center; gap:9px; }
  .te-rangerow .r-range{ flex:1; min-width:0; }
  .te-rnum{ flex:0 0 auto; min-width:40px; text-align:right; font-size:var(--v-fs-cap); color:var(--v-dim); }
  /* A NAME AND A VALUE, and the value on the right edge — §11 and §12, the shape
     `.te-rangerow` and `.te-swrow` already hold. The colour well is a fixed 38px
     (app.css) and the hex readout is auto-width text, so with nothing flexible
     between them the pair packed left and the row's content stopped 158px short of
     its right edge at 1280, and 730px short at 900. The box always spanned the
     column; what was short was everything in it, which is why it reads as a
     ragged column rather than as a broken control. */
  .te-swatch{ display:flex; align-items:center; gap:9px; justify-content:space-between; }
  .te-hex{ font-size:var(--v-fs-cap); color:var(--v-dim); text-transform:uppercase; }
  /* The TROUGH of a segmented control. It stays local rather than becoming
     app.css's `.r-seg` because these sit in the RIGHT half of a `.te-frow`,
     flexed to fill it, while `.r-seg` is an inline strip that sizes to its
     labels. */
  .te-seg{ display:flex; gap:2px; background:var(--v-bg); border:1px solid var(--v-line); border-radius:var(--v-r-md); padding:3px; }
  /* A SEGMENT, not a button — Cover/Contain, three alignments, three
     v-alignments. One choice out of N inside one trough that carries the edge
     for all of them; a member has no edge of its own because an edge per member
     would draw N boxes where the control is one. Named rather than styled
     through `.te-seg button`, so a census that groups by class can tell this
     from a button that lost its class. */
  .te-segbtn{ flex:1; height:26px; display:grid; place-items:center; border:0; border-radius:var(--v-r-sm); background:none; color:var(--v-dim); cursor:pointer; font-size:var(--v-fs-cap); }
  .te-segbtn:hover{ color:var(--v-txt); }
  .te-segbtn.on{ background:var(--v-surf3); color:var(--v-txt); }
  /* A SWITCH ROW, not a button — Italic, Scroll. A full-width row whose right
     end is an `.r-switch`; it is a button element so the whole row is the target. */
  .te-swrow{ display:flex; align-items:center; justify-content:space-between; width:100%; background:var(--v-surf2); border:1px solid var(--v-line); border-radius:var(--v-r-md); padding:9px 12px; color:var(--v-txt); font-size:var(--v-fs-b2); cursor:pointer; }
  .te-swrow:hover{ border-color:var(--v-line2); }
  .te-sublbl{ margin:10px 0 6px; }
  .te-bglib{ display:grid; grid-template-columns:repeat(3, 1fr); gap:6px; max-height:196px; overflow-y:auto; padding-right:4px; scrollbar-width:thin; scrollbar-color:var(--v-surf3) transparent; }
  /* A GRID CELL, not a button. The background image IS the control — a 16:9
     tile in a picker grid, selected by a ring rather than by a fill. */
  .te-bgtile{ aspect-ratio:16/9; border-radius:var(--v-r-sm); border:1px solid var(--v-line2); background-size:cover; background-position:center; cursor:pointer; padding:0; }
  .te-bgtile.on{ border-color:var(--v-accent); box-shadow:0 0 0 1px var(--v-accent); }
  /* THE EMPTY GRID CELL, not a button — B2. The "no image" tile: a `.te-bgtile`
     with a ✕ where the picture would be, so clearing the background is the same
     gesture in the same grid as choosing one. Only the FILL is local, because
     the cell has no image to be its own ground. */
  .te-bgnone{ display:grid; place-items:center; background:var(--v-surf2); color:var(--v-faint); font-size:var(--v-fs-pr); }
  .te-bgnone:hover{ color:var(--v-rose); }
  /* Per-screen content visibility chips */
  .te-showlbl{ margin-top:4px; }
  .te-showgrid{ display:flex; flex-wrap:wrap; gap:6px; }
  /* A CHIP, not a button — a tick plus a label, wrapping in a grid, each one an
     independent on/off. A chip states a fact about the thing you are editing;
     a button does something when pressed, and the row of them here would read
     as a row of actions if it wore the button shape. */
  .te-showchip{ display:inline-flex; align-items:center; gap:5px; padding:6px 10px; border-radius:var(--v-r-md); background:var(--v-surf2); border:1px solid var(--v-line2); color:var(--v-faint); font-size:var(--v-fs-cap); cursor:pointer; }
  .te-showchip:hover{ color:var(--v-txt); border-color:var(--v-accent-line); }
  .te-showchip.on{ background:var(--v-accent-soft); border-color:var(--v-accent-line); color:var(--v-txt); }
  .te-showtick{ width:9px; text-align:center; color:var(--v-emerald); font-weight:700; }
  .te-alignrow{ display:flex; gap:6px; }
  /* CONVERTED — B2. Centre H · Centre V · Centre is three equal buttons in one
     row, which is the definition of the shared control, and it was drawing a
     30px box with an `--v-r-md` corner and a divider hairline — three steps off
     `.r-btn.ghost` at once, four rows under a `.r-btn sm ghost` Duplicate. All
     that is left is the only thing about it that is not a button: it shares the
     row equally with its two neighbours. */
  .te-alignbtn{ flex:1; }
  /* `.te-geo` — the SECOND Position group's grid — went out with the group it
     styled (the one that wrote x/y/w/h without clamping them and moved a locked
     object). Two of its three rules were still here, and the compiler was
     printing "Unused CSS selector" for both on every build. */
  .te-err{ flex:0 0 auto; margin:0; padding:10px 14px; border-top:1px solid var(--v-line); color:var(--v-red); font-size:var(--v-fs-cap); line-height:1.5; }
  .te-missing{ margin:auto; padding:40px; }

  /* legacy convert */
  .te-legacy{ flex:1; min-height:0; display:grid; place-items:center; padding:20px; }
  .te-legacycard{ max-width:520px; text-align:center; background:var(--v-surf); border:1px solid var(--v-line); border-radius:var(--v-r-lg); padding:24px; }
  .te-legacycard h2{ margin:0 0 8px; font-family:var(--f-head); font-size:var(--v-fs-h2); color:var(--v-txt); }
  .te-legacycard p{ margin:0 0 16px; font-size:var(--v-fs-b2); color:var(--v-dim); line-height:1.5; }
  .te-legacyprev{ position:relative; aspect-ratio:16/9; border-radius:var(--v-r-md); overflow:hidden; border:1px solid var(--v-line2); background:var(--v-void); margin-bottom:16px; }
</style>
