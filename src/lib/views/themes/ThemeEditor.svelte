<script>
  // Theme editor — edit ONE custom theme's style and see it live on the same
  // TemplateRender the wall uses. A theme owns only the whitelisted style keys
  // (themes.js: THEME_STYLE_KEYS); this form is grouped by concern (type, colour,
  // rhythm) and every control is bound straight to the draft, so the preview is
  // WYSIWYG by construction.
  //
  // Builtins are read-only: if a builtin id somehow reaches here, the form is
  // locked and the operator is told to duplicate it. In normal flow the gallery
  // duplicates a builtin before opening the editor, so this is a guard, not a UX.
  import { createEventDispatcher, onMount } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
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

  const FONTS = [
    { value: 'var(--f-serif)', label: 'Serif' },
    { value: 'var(--f-body)', label: 'Body (sans)' },
    { value: 'var(--f-display)', label: 'Display' },
    { value: 'var(--f-head)', label: 'Heading (Inter)' },
  ];

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
      err = String(e);
    } finally {
      saving = false;
    }
  }

  // The preview template with the draft theme applied — one live render.
  $: previewTheme = draft ? { id: draft.id, name: draft.name, style: draft.style } : null;
</script>

<div class="te-shell">
  {#if !draft}
    <div class="te-loading">Loading theme…</div>
  {:else}
    <header class="te-head">
      <button class="r-btn ghost sm" on:click={() => dispatch('back')}>‹ Themes</button>
      <input class="r-input te-name" bind:value={draft.name} on:input={() => (dirty = true)}
        aria-label="Theme name" disabled={readonly} />
      <span class="te-spring"></span>
      {#if readonly}
        <span class="te-ro r-mono">Built-in · read-only</span>
      {:else}
        <button class="r-btn primary sm" on:click={save} disabled={saving || !dirty || !draft.name.trim()}>
          {saving ? 'Saving…' : dirty ? 'Save' : 'Saved'}
        </button>
      {/if}
    </header>

    {#if err}<div class="te-err" role="alert">{err}</div>{/if}

    <div class="te-body">
      <section class="te-preview-wrap">
        <div class="te-preview">
          <TemplateRender template={THEME_PREVIEW_TEMPLATE} theme={previewTheme} content={THEME_SAMPLE_CONTENT} />
        </div>
        <p class="te-cap r-mono">Live preview · scripture sample</p>
      </section>

      <section class="te-controls r-scroll">
        <fieldset class="te-group" disabled={readonly}>
          <legend>Typography</legend>
          <label class="te-row">
            <span>Typeface</span>
            <select class="r-select" value={draft.style.font || 'var(--f-serif)'} on:change={(e) => set('font', e.target.value)}>
              {#each FONTS as f}<option value={f.value}>{f.label}</option>{/each}
            </select>
          </label>
          <label class="te-row">
            <span>Verse size <em>{draft.style.verseSize || '6'}</em></span>
            <input type="range" min="3" max="10" step="0.1" value={draft.style.verseSize || 6} on:input={(e) => set('verseSize', e.target.value)} />
          </label>
          <label class="te-row">
            <span>Reference size <em>{draft.style.refSize || '2.6'}</em></span>
            <input type="range" min="1.4" max="4.5" step="0.1" value={draft.style.refSize || 2.6} on:input={(e) => set('refSize', e.target.value)} />
          </label>
          <label class="te-row">
            <span>Line height <em>{draft.style.verseLineHeight || '1.32'}</em></span>
            <input type="range" min="1" max="1.8" step="0.02" value={draft.style.verseLineHeight || 1.32} on:input={(e) => set('verseLineHeight', e.target.value)} />
          </label>
          <label class="te-check">
            <input type="checkbox" checked={!!draft.style.italicRef} on:change={(e) => set('italicRef', e.target.checked)} />
            <span>Italic reference</span>
          </label>
        </fieldset>

        <fieldset class="te-group" disabled={readonly}>
          <legend>Colour</legend>
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
          <label class="te-row te-bg">
            <span>Background</span>
            <input class="r-input" value={draft.style.background || ''} placeholder="#000 or a CSS gradient" on:input={(e) => set('background', e.target.value)} />
          </label>
          <p class="te-hint">Background accepts a hex colour or any CSS gradient, e.g. <code>linear-gradient(160deg,#241419,#120a0e)</code>. Leave empty for transparent.</p>
        </fieldset>

        <fieldset class="te-group" disabled={readonly}>
          <legend>Rhythm &amp; motion</legend>
          <label class="te-row">
            <span>Verse shadow <em>{Number(draft.style.verseShadow || 0).toFixed(2)}</em></span>
            <input type="range" min="0" max="1" step="0.05" value={draft.style.verseShadow || 0} on:input={(e) => set('verseShadow', e.target.value)} />
          </label>
          <label class="te-row">
            <span>Verse–reference gap <em>{draft.style.refGap || '1.4'}</em></span>
            <input type="range" min="0" max="4" step="0.1" value={draft.style.refGap || 1.4} on:input={(e) => set('refGap', e.target.value)} />
          </label>
          <label class="te-row">
            <span>Transition</span>
            <select class="r-select" value={draft.style.transition || 'fade'} on:change={(e) => set('transition', e.target.value)}>
              <option value="fade">Crossfade</option>
              <option value="slide">Slide up</option>
              <option value="zoom">Zoom</option>
            </select>
          </label>
          <label class="te-row">
            <span>Duration <em>{draft.style.transitionMs || '250'}ms</em></span>
            <input type="range" min="0" max="800" step="10" value={draft.style.transitionMs || 250} on:input={(e) => set('transitionMs', e.target.value)} />
          </label>
        </fieldset>

        {#if readonly}
          <p class="te-hint te-rohint">This is a built-in theme. Duplicate it from the gallery to make an editable copy.</p>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .te-shell{ display:flex; flex-direction:column; height:100%; min-height:0; gap:12px; }
  .te-loading{ margin:auto; color:var(--v-faint); }
  .te-head{ display:flex; align-items:center; gap:10px; flex:0 0 auto; }
  .te-name{ height:32px; max-width:280px; font-weight:600; }
  .te-spring{ flex:1; }
  .te-ro{ font-size:var(--v-fs-cap); color:var(--v-faint); }
  .te-err{ flex:0 0 auto; padding:9px 12px; border:1px solid var(--v-rose); border-radius:var(--v-r-md);
    background:var(--v-rose-soft); color:var(--v-rose); font-size:var(--v-fs-cap); }

  .te-body{ display:grid; grid-template-columns:minmax(0,1fr) 340px; gap:var(--v-sp-md); flex:1; min-height:0; }
  @media (max-width:1100px){ .te-body{ grid-template-columns:1fr; } }

  .te-preview-wrap{ display:flex; flex-direction:column; gap:8px; min-height:0; }
  .te-preview{ position:relative; aspect-ratio:16/9; width:100%; border-radius:var(--v-r-lg);
    border:1px solid var(--v-line2); overflow:hidden; background:var(--v-void); box-shadow:var(--v-shadow-lg); }
  .te-cap{ font-size:var(--v-fs-cap); color:var(--v-faint); text-align:center; }

  .te-controls{ min-height:0; overflow-y:auto; display:flex; flex-direction:column; gap:14px; padding-right:4px; }
  .te-group{ border:1px solid var(--v-line); border-radius:var(--v-r-lg); padding:12px 14px 14px;
    background:var(--v-surf); display:flex; flex-direction:column; gap:11px; margin:0; }
  .te-group[disabled]{ opacity:.6; }
  .te-group legend{ font-family:var(--f-head); font-size:var(--v-fs-b1); font-weight:600; color:var(--v-txt); padding:0 6px; }
  .te-row{ display:flex; align-items:center; justify-content:space-between; gap:12px; font-size:var(--v-fs-b2); color:var(--v-dim); }
  .te-row > span{ flex:0 0 auto; display:flex; gap:6px; align-items:baseline; }
  .te-row em{ font-style:normal; color:var(--v-faint); font-size:var(--v-fs-cap); font-variant-numeric:tabular-nums; }
  .te-row input[type=range]{ flex:1; max-width:170px; accent-color:var(--v-accent); }
  .te-row input[type=color]{ width:40px; height:26px; padding:0; border:1px solid var(--v-line2);
    border-radius:var(--v-r-sm); background:none; cursor:pointer; }
  .te-row .r-select{ height:30px; min-width:150px; }
  .te-bg{ flex-direction:column; align-items:stretch; gap:6px; }
  .te-bg > span{ align-self:flex-start; }
  .te-check{ display:flex; align-items:center; gap:9px; font-size:var(--v-fs-b2); color:var(--v-dim); }
  .te-check input{ accent-color:var(--v-accent); }
  .te-hint{ margin:0; font-size:var(--v-fs-cap); line-height:1.5; color:var(--v-faint); }
  .te-hint code{ font-family:var(--f-mono, monospace); font-size:11px; color:var(--v-dim); }
  .te-rohint{ padding:9px 11px; border:1px solid var(--v-line2); border-radius:var(--v-r-md); background:var(--v-surf2); }
</style>
