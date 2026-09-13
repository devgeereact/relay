<script>
  import { BG_STYLES } from '../../templatemodel.js';
  import { TRANSITIONS, DEFAULT_TRANSITION } from '../../transitions.js';
  import { humanError } from '../../errors.js';
  import { rangeFill } from '../../rangefill.js';
  // Theme editor — edit ONE custom theme's style and see it live on the same
  // TemplateRender the wall uses. A theme owns only the whitelisted style keys
  // (themes.js: THEME_STYLE_KEYS); the controls are an INSPECTOR (a tab strip of
  // concerns, then that concern's properties) rather than a page of stacked
  // cards, which is the same grammar as the template editor next door
  // (docs/REBRAND.md §3.2). Every control is bound straight to the draft, so the
  // preview is WYSIWYG by construction.
  //
  // Builtins are read-only: if a builtin id somehow reaches here, the form is
  // locked and the operator is told to duplicate it. In normal flow the gallery
  // duplicates a builtin before opening the editor, so this is a guard, not a UX.
  import { createEventDispatcher, onMount } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import WorkspaceFrame from '../WorkspaceFrame.svelte';
  import {
    BUILTIN_THEMES,
    THEME_PREVIEW_TEMPLATE,
    THEME_SAMPLE_CONTENT,
  } from '../../themes.js';
  import { customThemes, loadThemes, saveTheme } from '../../stores/capture.js';

  export let themeId = null;
  const dispatch = createEventDispatcher();

  // The editable draft. Kept flat: { id, name, style:{…} }.
  let draft = null;
  let readonly = false;
  let saving = false;
  let err = '';
  let dirty = false;
  let tab = 'type'; // type | colour | motion

  const FONTS = [
    { value: 'var(--f-serif)', label: 'Serif' },
    { value: 'var(--f-body)', label: 'Body (sans)' },
    { value: 'var(--f-display)', label: 'Display' },
    { value: 'var(--f-head)', label: 'Heading (Inter)' },
  ];

  // ONE register of the keys a theme may pin, each with the words the rail uses
  // and the tab that owns it. The rail, the tab strip and the "clear" control all
  // read this, so a key cannot be listed in one place and missing from another —
  // and a key that gains a control gains a rail row by construction.
  const KEYS = [
    { key: 'font', label: 'Typeface', tab: 'type' },
    { key: 'verseSize', label: 'Verse size', tab: 'type' },
    { key: 'refSize', label: 'Reference size', tab: 'type' },
    { key: 'verseLineHeight', label: 'Line height', tab: 'type' },
    { key: 'italicRef', label: 'Italic reference', tab: 'type' },
    { key: 'accent', label: 'Accent', tab: 'colour' },
    { key: 'verseColor', label: 'Verse text', tab: 'colour' },
    { key: 'refColor', label: 'Reference text', tab: 'colour' },
    { key: 'background', label: 'Background', tab: 'colour' },
    { key: 'bgStyle', label: 'Background style', tab: 'colour' },
    { key: 'verseShadow', label: 'Verse shadow', tab: 'motion' },
    { key: 'refGap', label: 'Verse–reference gap', tab: 'motion' },
    { key: 'transition', label: 'Transition', tab: 'motion' },
    { key: 'transitionMs', label: 'Duration', tab: 'motion' },
  ];
  const TABS = [
    { key: 'type', label: 'Type' },
    { key: 'colour', label: 'Colour' },
    { key: 'motion', label: 'Motion' },
  ];
  const FONT_LABEL = (v) => FONTS.find((f) => f.value === v)?.label ?? String(v);

  /** What a pinned key reads as in the rail. A boolean says On, not "true". */
  function keyValue(key, value) {
    if (key === 'font') return FONT_LABEL(value);
    if (key === 'italicRef') return value ? 'On' : 'Off';
    if (key === 'bgStyle') return BG_STYLES.find((b) => b.id === value)?.label ?? String(value);
    if (key === 'transition') return TRANSITIONS.find((t) => t.id === value)?.label ?? String(value);
    if (key === 'transitionMs') return `${value}ms`;
    return String(value);
  }

  onMount(async () => {
    await loadThemes();
    const all = [...BUILTIN_THEMES, ...$customThemes];
    const t = all.find((x) => x.id === themeId) ?? all[0];
    readonly = !!t.builtin;
    draft = { id: t.id, name: t.name, style: { ...(t.style ?? {}) } };
  });

  // A single setter so every control marks the draft dirty and triggers the
  // reactive preview. Empty string clears the key (falls back to the renderer
  // default) rather than persisting '' — a theme should not pin "no value".
  function set(key, value) {
    if (readonly || !draft) return;
    const style = { ...draft.style };
    if (value === '' || value == null) delete style[key];
    else style[key] = value;
    draft = { ...draft, style };
    dirty = true;
  }

  async function save() {
    if (readonly || !draft || !draft.name.trim()) return;
    saving = true;
    err = '';
    try {
      await saveTheme({ id: draft.id, name: draft.name.trim(), style: draft.style });
      dirty = false;
    } catch (e) {
      err = humanError(e); // `String(e)` on a typed error is "[object Object]".
    } finally {
      saving = false;
    }
  }

  // The preview template with the draft theme applied — one live render.
  // A background that is not a plain hex (a pasted gradient, a var()) is used
  // exactly as written — see `slideBG`. Offering a treatment for it would be a
  // control that changes nothing, which is worse than no control.
  $: rawBackground = !!draft?.style?.background && !/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(String(draft.style.background).trim());

  $: previewTheme = draft ? { id: draft.id, name: draft.name, style: draft.style } : null;

  // What this theme has actually PINNED. Everything else falls through to the
  // renderer's own default — REBRAND §3.1's rule, made visible: a property with
  // two homes is a property you can edit in one place while it is written in
  // another, and the only defence is being able to see which keys are set.
  $: pinned = draft ? KEYS.filter((k) => draft.style[k.key] !== undefined && draft.style[k.key] !== '') : [];
</script>

<!-- THE THEME EDITOR, in the shared workspace grammar (`WorkspaceFrame.svelte`,
     docs/REBRAND.md §2 and §11): the rail says what this theme has CHANGED, the
     main area is one live render, the inspector is a tab strip of concerns and
     that concern's properties (§3.2) — not a page of stacked cards. -->
<WorkspaceFrame
  title="Theme editor"
  standfirst="Everything this theme does not pin falls through to the renderer's own default."
  columns="206px minmax(0,1fr) 312px">
  <svelte:fragment slot="head">
    {#if draft}
      <button class="r-btn ghost sm" on:click={() => dispatch('back')}>‹ Themes</button>
      <input class="r-input te-name" bind:value={draft.name} on:input={() => (dirty = true)}
        aria-label="Theme name" disabled={readonly} />
      {#if readonly}
        <span class="te-ro r-mono">Built-in · read-only</span>
      {:else}
        <button class="r-btn primary sm" on:click={save} disabled={saving || !dirty || !draft.name.trim()}>
          {saving ? 'Saving…' : dirty ? 'Save' : 'Saved'}
        </button>
      {/if}
    {/if}
  </svelte:fragment>

  {#if !draft}
    <section class="rw-pane te-loadpane">
      <div class="rw-panehead"><h2 class="rw-panettl">Theme</h2></div>
      <div class="te-loading">Loading theme…</div>
    </section>
  {:else}
    <!-- ══ RAIL ══ what this theme has CHANGED, and nothing else. -->
    <aside class="rw-pane te-rail">
      <div class="rw-panehead"><h2 class="rw-panettl">Set by this theme</h2></div>
      <div class="rw-panebody">
        {#each pinned as k (k.key)}
          <div class="te-prow">
            <button class="rw-item r-focus te-pjump" on:click={() => (tab = k.tab)} title="Show {k.label} in the inspector">
              <span class="rw-itemname">{k.label}</span>
              <span class="te-pv r-mono">{keyValue(k.key, draft.style[k.key])}</span>
            </button>
            {#if !readonly}
              <button class="te-pclear" aria-label="Clear {k.label}" title="Clear — let it fall back to the renderer default"
                on:click={() => set(k.key, '')}>✕</button>
            {/if}
          </div>
        {/each}
        {#if !pinned.length}
          <p class="te-railempty">Nothing set yet. Every key falls through to the renderer's own default until a control in the inspector changes it.</p>
        {/if}
      </div>
      <div class="rw-panefoot">
        <p class="te-railnote">A theme sets defaults. A template overrides it key by key, so anything listed here is what a template has <b>not</b> said for itself.</p>
      </div>
    </aside>

    <!-- ══ THE STAGE ══ one live render, the same component as the wall. -->
    <section class="rw-pane">
      <div class="rw-panehead">
        <h2 class="rw-panettl">Preview</h2>
        <span class="rw-spring"></span>
        <span class="te-cap r-mono">scripture sample · 16:9</span>
      </div>
      {#if err}<div class="te-err" role="alert">{err}</div>{/if}
      <div class="te-stage">
        <div class="te-preview">
          <TemplateRender template={THEME_PREVIEW_TEMPLATE} theme={previewTheme} content={THEME_SAMPLE_CONTENT} />
        </div>
      </div>
    </section>

    <!-- ══ INSPECTOR ══ a tab strip of concerns, then that concern's
         properties. The strip WRAPS rather than scrolls: a tab hidden behind a
         hidden scrollbar is a tab nobody knows is there. -->
    <aside class="rw-pane rw-insp">
      <div class="rw-panehead"><h2 class="rw-panettl">Design</h2></div>
      <div class="te-objtabs" role="tablist" aria-label="Theme properties">
        {#each TABS as t (t.key)}
          <button class="te-objtab" class:on={tab === t.key} role="tab" aria-selected={tab === t.key}
            on:click={() => (tab = t.key)}>{t.label}</button>
        {/each}
      </div>

      <div class="rw-panebody pad te-designbody">
        <fieldset class="te-fs" disabled={readonly}>
          {#if tab === 'type'}
            <h3 class="te-sec">Text</h3>
            <label class="te-row">
              <span>Typeface</span>
              <select class="r-select" value={draft.style.font || 'var(--f-serif)'} on:change={(e) => set('font', e.target.value)}>
                {#each FONTS as f}<option value={f.value}>{f.label}</option>{/each}
              </select>
            </label>
            <label class="te-row">
              <span>Verse size <em>{draft.style.verseSize || '6'}</em></span>
              <input type="range" min="3" max="10" step="0.1" value={draft.style.verseSize || 6} on:input={(e) => set('verseSize', e.target.value)} use:rangeFill={draft.style.verseSize || 6} />
            </label>
            <label class="te-row">
              <span>Reference size <em>{draft.style.refSize || '2.6'}</em></span>
              <input type="range" min="1.4" max="4.5" step="0.1" value={draft.style.refSize || 2.6} on:input={(e) => set('refSize', e.target.value)} use:rangeFill={draft.style.refSize || 2.6} />
            </label>
            <label class="te-row">
              <span>Line height <em>{draft.style.verseLineHeight || '1.32'}</em></span>
              <input type="range" min="1" max="1.8" step="0.02" value={draft.style.verseLineHeight || 1.32} on:input={(e) => set('verseLineHeight', e.target.value)} use:rangeFill={draft.style.verseLineHeight || 1.32} />
            </label>
            <label class="te-check">
              <input type="checkbox" checked={!!draft.style.italicRef} on:change={(e) => set('italicRef', e.target.checked)} />
              <span>Italic reference</span>
            </label>
          {:else if tab === 'colour'}
            <h3 class="te-sec">Ink</h3>
            <label class="te-row">
              <span>Accent</span>
              <input type="color" value={draft.style.accent || '#22d3ee'} on:input={(e) => set('accent', e.target.value)} />
            </label>
            <label class="te-row">
              <span>Verse text</span>
              <input type="color" value={draft.style.verseColor || '#ffffff'} on:input={(e) => set('verseColor', e.target.value)} />
            </label>
            <label class="te-row">
              <span>Reference text</span>
              <input type="color" value={draft.style.refColor || draft.style.accent || '#22d3ee'} on:input={(e) => set('refColor', e.target.value)} />
            </label>

            <h3 class="te-sec">Ground</h3>
            <label class="te-row te-bg">
              <span>Background</span>
              <input class="r-input" value={draft.style.background || ''} placeholder="#000 or a CSS gradient" on:input={(e) => set('background', e.target.value)} />
            </label>
            <label class="te-row">
              <span>Background style</span>
              <select
                class="r-select"
                value={draft.style.bgStyle || 'solid'}
                disabled={rawBackground}
                on:change={(e) => set('bgStyle', e.target.value)}
              >
                {#each BG_STYLES as b (b.id)}
                  <option value={b.id}>{b.label}</option>
                {/each}
              </select>
            </label>
            <p class="te-hint">Background accepts a hex colour or any CSS gradient, e.g. <code>linear-gradient(160deg,#241419,#120a0e)</code>. Leave empty for transparent.{#if rawBackground} A background written as CSS is used exactly as typed, so the style above does not apply to it.{/if}</p>
          {:else}
            <h3 class="te-sec">Effects</h3>
            <label class="te-row">
              <span>Verse shadow <em>{Number(draft.style.verseShadow || 0).toFixed(2)}</em></span>
              <input type="range" min="0" max="1" step="0.05" value={draft.style.verseShadow || 0} on:input={(e) => set('verseShadow', e.target.value)} use:rangeFill={draft.style.verseShadow || 0} />
            </label>
            <label class="te-row">
              <span>Verse–reference gap <em>{draft.style.refGap || '1.4'}</em></span>
              <input type="range" min="0" max="4" step="0.1" value={draft.style.refGap || 1.4} on:input={(e) => set('refGap', e.target.value)} use:rangeFill={draft.style.refGap || 1.4} />
            </label>

            <h3 class="te-sec">Transition</h3>
            <label class="te-row">
              <span>Transition</span>
              <select class="r-select" value={draft.style.transition || DEFAULT_TRANSITION} on:change={(e) => set('transition', e.target.value)}>
                {#each TRANSITIONS as t (t.id)}
                  <option value={t.id} title={t.hint}>{t.label}</option>
                {/each}
              </select>
            </label>
            <label class="te-row">
              <span>Duration <em>{draft.style.transitionMs || '250'}ms</em></span>
              <input type="range" min="0" max="800" step="10" value={draft.style.transitionMs || 250} on:input={(e) => set('transitionMs', e.target.value)} use:rangeFill={draft.style.transitionMs || 250} />
            </label>
          {/if}
        </fieldset>

        {#if readonly}
          <p class="rw-foot">This is a <b>built-in</b> theme. Duplicate it from the gallery to make an editable copy.</p>
        {/if}
      </div>
    </aside>
  {/if}
</WorkspaceFrame>

<style>
  /* The pane, the pane head, the rail row and the footnote all come from
     `WorkspaceFrame.svelte`. What is left is this editor's own: the head's
     controls, the stage, and the inspector's property rows. */
  .te-name{ height:26px; max-width:240px; font-weight:600; }
  .te-ro{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-loadpane{ grid-column:1 / -1; }
  .te-loading{ margin:auto; padding:24px; color:var(--v-faint); font-size:var(--v-fs-b2); }
  .te-err{ flex:0 0 auto; padding:8px 12px; border-bottom:1px solid var(--v-line);
    background:var(--v-rose-soft); color:var(--v-rose); font-size:var(--v-fs-cap); line-height:1.45; }
  .te-cap{ font-size:var(--v-fs-cap); color:var(--v-faint); }

  /* ── the rail ─────────────────────────────────────────────────────────── */
  /* A pinned key and its clear share one seam, so the row reads as one thing
     with two halves rather than as two rows of different heights. */
  .te-prow{ display:flex; align-items:stretch; border-bottom:1px solid var(--v-line); }
  .te-pjump{ flex:1; min-width:0; border-bottom:0; padding-right:4px; }
  .te-pjump .rw-itemname{ flex:0 0 auto; }
  .te-pv{ flex:1; min-width:0; text-align:right; font-size:var(--v-fs-cap); color:var(--v-faint);
    overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .te-pclear{ flex:0 0 auto; width:24px; border:0; background:none; color:var(--v-faint);
    cursor:pointer; font-size:var(--v-fs-cap); line-height:1; }
  .te-pclear:hover{ color:var(--v-rose); background:var(--v-rose-soft); }
  .te-railempty{ margin:0; padding:12px; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .te-railnote{ margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .te-railnote b{ color:var(--v-dim); }

  /* ── the stage ────────────────────────────────────────────────────────── */
  /* Flat, calm and dark with a soft vignette for depth, so a slide is judged
     against something close to a black wall rather than against a card.
     `--v-void` rather than a hand-picked hex: `TemplateEditor.svelte`'s
     `.te-stage` still carries a literal one step off this, which is a real (if
     imperceptible) inconsistency and is reported rather than copied. */
  .te-stage{ flex:1; min-height:0; display:flex; align-items:center; justify-content:center;
    padding:var(--v-sp-lg); overflow:auto; position:relative; background:var(--v-void); }
  .te-stage::before{ content:""; position:absolute; inset:0; pointer-events:none;
    background:radial-gradient(130% 110% at 50% 32%, transparent 45%, rgba(0,0,0,.45) 100%); }
  /* position:relative is load-bearing — TemplateRender's root is
     position:absolute; inset:0 and supplies its own container-type. */
  .te-preview{ position:relative; z-index:1; width:100%; max-width:760px; aspect-ratio:16/9;
    border-radius:var(--v-r-md); border:1px solid var(--v-line2); overflow:hidden;
    background:var(--v-void); box-shadow:var(--v-shadow-lg); }

  /* ── inspector ────────────────────────────────────────────────────────── */
  /* The strip WRAPS. A tab that has scrolled out of sight behind a hidden
     scrollbar is a tab nobody knows is there. */
  .te-objtabs{ display:flex; flex-wrap:wrap; gap:3px; padding:8px 12px 0; flex:0 0 auto; }
  .te-objtab{ padding:3px 8px; border-radius:var(--v-r-sm); border:1px solid var(--v-line2);
    background:var(--v-surf2); color:var(--v-dim); font-size:var(--v-fs-lbl); font-weight:600;
    cursor:pointer; max-width:100%; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;
    transition:background var(--v-dur) var(--v-ease), color var(--v-dur) var(--v-ease); }
  .te-objtab:hover:not(.on){ color:var(--v-txt); background:var(--v-surf3); }
  .te-objtab.on{ background:var(--v-sel-fill); border-color:transparent; color:var(--v-sel-ink); }

  .te-designbody{ padding-top:10px; }
  .te-fs{ border:0; padding:0; margin:0; min-width:0; display:flex; flex-direction:column; gap:10px; }
  .te-fs[disabled]{ opacity:.6; }
  .te-sec{ margin:6px 0 0; font-family:var(--f-head); font-size:var(--v-fs-h3); font-weight:600; color:var(--v-txt); }
  .te-sec:first-child{ margin-top:0; }
  .te-row{ display:flex; align-items:center; justify-content:space-between; gap:12px; font-size:var(--v-fs-b2); color:var(--v-dim); }
  .te-row > span{ flex:0 0 auto; display:flex; gap:6px; align-items:baseline; }
  .te-row em{ font-style:normal; color:var(--v-faint); font-size:var(--v-fs-cap); font-variant-numeric:tabular-nums; }
  /* Width only. The track, the thumb and the filled share come from app.css —
     accent-color let every platform draw its own idea of a slider. */
  .te-row input[type=range]{ flex:1; max-width:150px; }
  .te-row .r-select{ height:26px; min-width:136px; }
  .te-bg{ flex-direction:column; align-items:stretch; gap:6px; }
  .te-bg > span{ align-self:flex-start; }
  .te-check{ display:flex; align-items:center; gap:9px; font-size:var(--v-fs-b2); color:var(--v-dim); }
  .te-check input{ accent-color:var(--v-sel); }
  .te-hint{ margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .te-hint code{ font-family:var(--f-mono, monospace); font-size:var(--v-fs-cap); color:var(--v-dim); }
</style>
