<script>
  import { onMount } from 'svelte';
  import { capture, templates, loadTemplates, saveTemplate } from '../stores/capture.js';

  // Phase 8: templates are DB-backed and edited here. Saving broadcasts
  // `template://updated`, so any open output window on this template re-renders
  // live. ONE shared renderer (Output.svelte) interprets these configs — this
  // editor just edits the config, never per-channel rendering code.
  let edit = null; // working copy of the selected template
  let saving = false;
  let savedId = null;

  onMount(async () => {
    const list = await loadTemplates();
    if (list.length) select(list[0]);
  });

  function select(t) {
    edit = structuredClone(t);
    // Ensure expected fields exist for older/custom templates.
    edit.layout ??= { regions: [], align: 'center' };
    edit.layout.regions ??= [];
    edit.style ??= {};
    savedId = null;
  }

  const SAMPLE = {
    text: 'For God so loved the world, that he gave his only begotten Son, that whosoever believeth in him should not perish.',
    reference: 'John 3:16 · KJV',
  };
  const FONTS = [
    { label: 'Fraunces (serif)', value: 'var(--f-serif)' },
    { label: 'Space Grotesk', value: 'var(--f-display)' },
    { label: 'Inter', value: 'var(--f-body)' },
  ];
  const BACKGROUNDS = [
    { label: 'Radial warm dark', value: 'radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)' },
    { label: 'Solid black', value: '#000000' },
    { label: 'Lobby warm', value: 'linear-gradient(160deg, #241419, #120a0e)' },
    { label: 'Transparent (keyed)', value: 'transparent' },
  ];
  const ACCENTS = ['var(--amber)', 'var(--teal)', 'var(--violet)', 'var(--rose)'];
  const REGIONS = ['verse_text', 'reference', 'timer', 'next_event'];

  function toggleRegion(r) {
    const set = new Set(edit.layout.regions);
    set.has(r) ? set.delete(r) : set.add(r);
    edit.layout.regions = [...set];
  }

  $: refFirst =
    edit &&
    (edit.layout.refFirst || (edit.layout.regions[0] === 'reference' && !edit.layout.lowerThird));

  async function save() {
    saving = true;
    try {
      const id = await saveTemplate(edit);
      savedId = id;
      // keep editing the reloaded row
      const fresh = $templates.find((t) => t.id === id);
      if (fresh) edit = structuredClone(fresh);
    } catch (e) {
      savedId = null;
      alert('Save failed: ' + e);
    }
    saving = false;
  }
</script>

<div class="templates-layout">
  <div class="tmpl-list">
    {#each $templates as t}
      <div class="tmpl-row" class:active={edit && t.id === edit.id} on:click={() => select(t)} role="button" tabindex="0">
        {t.name} <span class="tag">#{t.id}</span>
      </div>
    {/each}
    {#if !$capture.available}
      <div style="font-family:var(--f-mono); font-size:11px; color:var(--text-faint); margin-top:6px;">backend not attached</div>
    {/if}
  </div>

  {#if edit}
    <div class="panel editor">
      <div class="panel-title">
        Editing — {edit.name}
        {#if savedId}<span class="count" style="color:var(--green);">saved ✓ live</span>{/if}
      </div>
      <div style="display:grid; grid-template-columns:1fr 1fr; gap:20px;">
        <div>
          <div class="field-group">
            <div class="field-label">Name</div>
            <input class="search-input" bind:value={edit.name} />
          </div>
          <div class="field-group">
            <div class="field-label">Regions</div>
            {#each REGIONS as r}
              <label class="check-row">
                <input type="checkbox" checked={edit.layout.regions.includes(r)} on:change={() => toggleRegion(r)} /> {r}
              </label>
            {/each}
          </div>
          <div class="field-group">
            <div class="field-label">Typeface</div>
            <select class="select-mock" bind:value={edit.style.font}>
              {#each FONTS as f}<option value={f.value}>{f.label}</option>{/each}
            </select>
          </div>
          <div class="field-group">
            <div class="field-label">Background</div>
            <select class="select-mock" bind:value={edit.style.background}>
              {#each BACKGROUNDS as b}<option value={b.value}>{b.label}</option>{/each}
            </select>
          </div>
          <div class="field-group">
            <div class="field-label">Accent color</div>
            <div class="swatches">
              {#each ACCENTS as a}
                <div class="swatch" class:sel={edit.style.accent === a} style="background:{a};" on:click={() => (edit.style.accent = a)} role="button" tabindex="0"></div>
              {/each}
            </div>
          </div>
          <div class="field-group">
            <div class="field-label">Layout</div>
            <label class="check-row"><input type="radio" bind:group={edit.layout.align} value="center" /> Center</label>
            <label class="check-row"><input type="radio" bind:group={edit.layout.align} value="left" /> Left</label>
            <label class="check-row"><input type="checkbox" bind:checked={edit.layout.refFirst} /> Reference first</label>
            <label class="check-row"><input type="checkbox" bind:checked={edit.layout.lowerThird} /> Lower third</label>
            <label class="check-row"><input type="checkbox" bind:checked={edit.style.italicRef} /> Italic reference</label>
          </div>
          <button class="ctrl-btn primary" on:click={save} disabled={saving || !$capture.available}>
            {saving ? 'Saving…' : 'Save template'}
          </button>
        </div>

        <div>
          <div class="field-label" style="margin-bottom:9px;">Live preview</div>
          <div
            class="tmpl-preview"
            class:lt={edit.layout.lowerThird}
            style="background:{edit.style.background}; text-align:{edit.layout.align}; font-family:{edit.style.font};"
          >
            <div class="inner">
              {#if refFirst}
                {#if edit.layout.regions.includes('reference')}
                  <div class="pv-ref" style="color:{edit.style.accent};">{SAMPLE.reference}</div>
                {/if}
                {#if edit.layout.regions.includes('verse_text')}
                  <div class="pv-verse" style="color:{edit.style.verseColor || '#f0e9dd'};">{SAMPLE.text}</div>
                {/if}
              {:else}
                {#if edit.layout.regions.includes('verse_text')}
                  <div class="pv-verse" style="color:{edit.style.verseColor || '#f0e9dd'};">"{SAMPLE.text}"</div>
                {/if}
                {#if edit.layout.regions.includes('reference')}
                  <div class="pv-ref" style="color:{edit.style.accent}; font-style:{edit.style.italicRef ? 'italic' : 'normal'};">{SAMPLE.reference}</div>
                {/if}
              {/if}
            </div>
          </div>
          <div style="font-family:var(--f-mono); font-size:10.5px; color:var(--text-faint); margin-top:8px;">
            Open this template in Channels → then Save here to see it update live on the output screen.
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .tmpl-preview {
    height: 240px;
    border-radius: 9px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 18px;
    overflow: hidden;
    position: relative;
  }
  .tmpl-preview.lt { align-items: flex-end; padding: 0; }
  .tmpl-preview.lt .inner {
    width: 100%;
    background: linear-gradient(90deg, rgba(176, 128, 224, 0.95), rgba(176, 128, 224, 0.75));
    padding: 12px 16px;
  }
  .inner { max-width: 92%; }
  .pv-verse { font-size: 15px; line-height: 1.4; }
  .pv-ref { font-size: 12px; font-weight: 600; margin-top: 8px; }
</style>
