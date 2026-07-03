<script>
  import { onMount } from 'svelte';
  import { capture, templates, loadTemplates, saveTemplate } from '../stores/capture.js';
  import TemplateRender from '../TemplateRender.svelte';

  // WYSIWYG: the preview below is the SAME renderer (TemplateRender) as the live
  // output, so what you save is exactly what shows. Saving broadcasts
  // `template://updated` → any open output/OBS source re-renders live.
  let edit = null;
  let saving = false;
  let savedId = null;

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
  }

  const SAMPLE = {
    text: 'The LORD is my shepherd; I shall not want. He maketh me to lie down in green pastures: he leadeth me beside the still waters.',
    reference: 'Psalms 23:1-2 · KJV',
  };

  const FONTS = [
    { label: 'Fraunces (serif)', value: 'var(--f-serif)' },
    { label: 'Space Grotesk', value: 'var(--f-display)' },
    { label: 'Inter', value: 'var(--f-body)' },
    { label: 'JetBrains Mono', value: 'var(--f-mono)' },
  ];
  // Background + a readable text color paired for each, plus Transparent (alpha
  // for OBS/ATEM camera keying).
  const BACKGROUNDS = [
    { label: 'Black', value: '#000000', text: '#ffffff' },
    { label: 'White', value: '#ffffff', text: '#111318' },
    { label: 'Charcoal', value: '#14161a', text: '#f2f4f7' },
    { label: 'Warm dark', value: 'radial-gradient(120% 140% at 50% 30%, #2a2013, #0b0906)', text: '#f4e4c8' },
    { label: 'Deep blue', value: 'linear-gradient(160deg, #0b1a2e, #060d18)', text: '#eaf2ff' },
    { label: 'Lobby warm', value: 'linear-gradient(160deg, #241419, #120a0e)', text: '#f0dfe3' },
    { label: 'Transparent (OBS key)', value: 'transparent', text: '#1c1224' },
  ];
  const TEXT_COLORS = ['#ffffff', '#f4e4c8', '#eaf2ff', '#111318', '#1c1224', '#f0dfe3', '#cbd3e0'];
  const ACCENTS = ['#e8a33d', '#4fa8c9', '#b080e0', '#e27d93', '#4caf7d', '#ffffff', '#111318'];
  const REGIONS = ['verse_text', 'reference'];

  function toggleRegion(r) {
    const set = new Set(edit.layout.regions);
    set.has(r) ? set.delete(r) : set.add(r);
    edit.layout.regions = [...set];
  }
  // Picking a background also sets a readable default text color.
  function pickBackground(b) {
    edit.style.background = b.value;
    edit.style.verseColor = b.text;
  }

  async function save() {
    saving = true;
    try {
      const id = await saveTemplate(edit);
      savedId = id;
      const fresh = $templates.find((t) => t.id === id);
      if (fresh) edit = structuredClone(fresh);
    } catch (e) {
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
      <div style="display:grid; grid-template-columns:300px 1fr; gap:20px; align-items:start;">
        <div>
          <div class="field-group">
            <div class="field-label">Name</div>
            <input class="search-input" bind:value={edit.name} />
          </div>
          <div class="field-group">
            <div class="field-label">Show</div>
            {#each REGIONS as r}
              <label class="check-row"><input type="checkbox" checked={edit.layout.regions.includes(r)} on:change={() => toggleRegion(r)} /> {r}</label>
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
            <div class="swatches" style="flex-wrap:wrap;">
              {#each BACKGROUNDS as b}
                <button class="bgsw" class:sel={edit.style.background === b.value} style="background:{b.value === 'transparent' ? 'repeating-conic-gradient(#444 0% 25%, #222 0% 50%) 50% / 10px 10px' : b.value};" title={b.label} on:click={() => pickBackground(b)}></button>
              {/each}
            </div>
          </div>
          <div class="field-group">
            <div class="field-label">Text color</div>
            <div class="swatches" style="flex-wrap:wrap;">
              {#each TEXT_COLORS as c}
                <div class="swatch" class:sel={edit.style.verseColor === c} style="background:{c}; border:1px solid var(--border);" on:click={() => (edit.style.verseColor = c)} role="button" tabindex="0"></div>
              {/each}
            </div>
          </div>
          <div class="field-group">
            <div class="field-label">Accent</div>
            <div class="swatches" style="flex-wrap:wrap;">
              {#each ACCENTS as a}
                <div class="swatch" class:sel={edit.style.accent === a} style="background:{a}; border:1px solid var(--border);" on:click={() => (edit.style.accent = a)} role="button" tabindex="0"></div>
              {/each}
            </div>
          </div>
          <div class="field-group">
            <div class="field-label">Verse size <span style="color:var(--text-faint);">{(+edit.style.verseSize || 5).toFixed(1)}</span></div>
            <input class="range" type="range" min="2" max="9" step="0.1" bind:value={edit.style.verseSize} />
            <div class="field-label" style="margin-top:8px;">Reference size <span style="color:var(--text-faint);">{(+edit.style.refSize || 2).toFixed(1)}</span></div>
            <input class="range" type="range" min="1" max="5" step="0.1" bind:value={edit.style.refSize} />
          </div>
          <div class="field-group">
            <div class="field-label">Layout</div>
            <label class="check-row"><input type="radio" bind:group={edit.layout.align} value="center" /> Center</label>
            <label class="check-row"><input type="radio" bind:group={edit.layout.align} value="left" /> Left</label>
            <label class="check-row"><input type="checkbox" bind:checked={edit.layout.refFirst} /> Reference first</label>
            <label class="check-row"><input type="checkbox" bind:checked={edit.layout.lowerThird} /> Lower third (band at bottom)</label>
            <label class="check-row"><input type="checkbox" bind:checked={edit.style.italicRef} /> Italic reference</label>
          </div>
          <button class="ctrl-btn primary" on:click={save} disabled={saving || !$capture.available}>
            {saving ? 'Saving…' : 'Save template'}
          </button>
        </div>

        <div>
          <div class="field-label" style="margin-bottom:9px;">Live preview (exactly what outputs)</div>
          <div class="wysiwyg">
            <div class="checker"></div>
            <TemplateRender template={edit} content={SAMPLE} />
          </div>
          <div style="font-family:var(--f-mono); font-size:10.5px; color:var(--text-faint); margin-top:8px; line-height:1.6;">
            Long verses wrap and auto-shrink to fit. A <b>Transparent</b> background keys out in OBS/ATEM so a camera shows behind (great for the lower third).
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .wysiwyg {
    position: relative;
    width: 100%;
    aspect-ratio: 16 / 9;
    border-radius: 9px;
    overflow: hidden;
    border: 1px solid var(--border);
  }
  /* Checkerboard behind the preview so transparent templates read as alpha. */
  .checker {
    position: absolute;
    inset: 0;
    background: repeating-conic-gradient(#2a2f38 0% 25%, #1b1e24 0% 50%) 50% / 20px 20px;
  }
  .bgsw {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
  }
  .bgsw.sel { border-color: var(--text); }
</style>
