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
</script>

<div class="tpl-editor">
  <!-- Saved layouts -->
  <aside class="tile pane list">
    <div class="pane-head">
      <span class="r-lbl">Templates · {activeCount}/4 active</span>
      <button class="r-iconbtn" title="New template" aria-label="New template" on:click={newTemplate} disabled={!$capture.available}>
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="M12 5v14M5 12h14"/></svg>
      </button>
    </div>
    <div class="list-body">
      {#each $templates as t (t.id)}
        <div class="tpl-row" class:sel={edit && t.id === edit.id}>
          <button class="star" class:on={t.active} title={t.active ? 'Active on console' : 'Activate on console'}
            aria-label="Toggle active" on:click|stopPropagation={() => toggleActive(t)}
            disabled={!$capture.available || (!t.active && activeCount >= 4)}>
            <svg viewBox="0 0 24 24" width="14" height="14" fill={t.active ? 'currentColor' : 'none'} stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"><path d="m12 3 2.6 5.3 5.9.9-4.3 4.1 1 5.8-5.2-2.7-5.2 2.7 1-5.8-4.3-4.1 5.9-.9L12 3Z"/></svg>
          </button>
          <button class="tpl-name" on:click={() => select(t)}>{t.name}</button>
          <button class="del" class:arm={delArm === t.id} title={delArm === t.id ? 'Click again to confirm' : 'Delete'} aria-label="Delete template" on:click|stopPropagation={() => del(t)} disabled={!$capture.available}>
            {#if delArm === t.id}
              <span class="delconf">Sure?</span>
            {:else}
              <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"><path d="M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13a1 1 0 0 0 1 1h8a1 1 0 0 0 1-1l1-13"/></svg>
            {/if}
          </button>
        </div>
      {/each}
      {#if !$capture.available}<div class="r-empty" style="padding:8px 4px;">backend not attached</div>{/if}
    </div>
  </aside>

  {#if edit}
    <!-- Live preview -->
    <section class="tile pane stage">
      <div class="pane-head">
        <span class="r-lbl">Preview · {edit.name}</span>
        {#if savedId}<span class="r-badge amber"><span class="bd"></span>saved · live</span>{/if}
      </div>
      <div class="stage-body">
        <div class="wysiwyg">
          <div class="checker"></div>
          <TemplateRender template={edit} content={SAMPLE} />
        </div>
        <div class="stage-note r-mono">Long verses wrap and auto-shrink. A <b>Transparent</b> background keys out in OBS/ATEM.</div>
      </div>
    </section>

    <!-- Inspector -->
    <aside class="tile pane insp">
      <div class="insp-body r-scroll">
        <div class="grp">
          <span class="r-lbl">Name</span>
          <input class="r-input" bind:value={edit.name} />
        </div>

        <div class="grp">
          <span class="r-lbl">Show regions</span>
          {#each REGIONS as r}
            <button class="sw-row" on:click={() => toggleRegion(r.key)}>
              <span>{r.label}</span>
              <span class="r-switch" class:on={edit.layout.regions.includes(r.key)}></span>
            </button>
          {/each}
        </div>

        <div class="grp">
          <span class="r-lbl">Typeface <button class="mini-link" on:click={detectFonts}>detect installed {fontMsg}</button></span>
          <select class="r-select" bind:value={edit.style.font}>
            {#if edit.style.font && !fonts.includes(edit.style.font)}<option value={edit.style.font}>{edit.style.font}</option>{/if}
            {#each fonts as f}<option value={f}>{f}</option>{/each}
          </select>
        </div>

        <div class="grp row2">
          <div>
            <span class="r-lbl">Verse size · {(+edit.style.verseSize || 6).toFixed(1)}</span>
            <input class="r-range" type="range" min="2" max="12" step="0.1" bind:value={edit.style.verseSize} />
          </div>
          <div>
            <span class="r-lbl">Ref size · {(+edit.style.refSize || 2.6).toFixed(1)}</span>
            <input class="r-range" type="range" min="1" max="6" step="0.1" bind:value={edit.style.refSize} />
          </div>
        </div>

        <div class="grp row3">
          <label class="clr"><span class="r-lbl">Text</span><input type="color" value={isColor(edit.style.verseColor) ? edit.style.verseColor : '#f4e4c8'} on:input={(e) => setStyle('verseColor', e.target.value)} /></label>
          <label class="clr"><span class="r-lbl">Accent</span><input type="color" value={isColor(edit.style.accent) ? edit.style.accent : '#f5a623'} on:input={(e) => setStyle('accent', e.target.value)} /></label>
          <label class="clr"><span class="r-lbl">Fill</span><input type="color" value={isColor(edit.style.background) ? edit.style.background : '#0b0906'} on:input={onBgColor} /></label>
        </div>

        <div class="grp">
          <span class="r-lbl">Background</span>
          <div class="bg-row">
            <label class="r-btn ghost sm file">
              <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round"><rect x="3" y="4" width="18" height="16" rx="2"/><path d="m3 16 5-5 4 4 3-3 6 6"/><circle cx="9" cy="9" r="1.6"/></svg>
              Image<input type="file" accept="image/*" on:change={onBgImage} hidden />
            </label>
            <button class="r-btn ghost sm" on:click={setTransparent}>Transparent</button>
            {#if edit.style.bgImage}<button class="r-btn danger sm" on:click={clearImage}>Clear image</button>{/if}
          </div>
          {#if edit.style.bgImage}<div class="bg-thumb" style="background-image:url({edit.style.bgImage});"></div>{/if}
        </div>

        <div class="grp">
          <span class="r-lbl">Verse alignment</span>
          <div class="seg-align">
            {#each ['left', 'center', 'right'] as a}
              <button class:on={(edit.style.verseAlign || edit.layout.align || 'center') === a} on:click={() => setStyle('verseAlign', a)}>{a}</button>
            {/each}
          </div>
          <span class="r-lbl" style="margin-top:2px;">Reference alignment</span>
          <div class="seg-align">
            {#each ['left', 'center', 'right'] as a}
              <button class:on={(edit.style.refAlign || edit.layout.align || 'center') === a} on:click={() => setStyle('refAlign', a)}>{a}</button>
            {/each}
          </div>
          <button class="sw-row" on:click={() => setLayout('refFirst', !edit.layout.refFirst)}><span>Reference above verse</span><span class="r-switch" class:on={edit.layout.refFirst}></span></button>
          <button class="sw-row" on:click={() => setLayout('lowerThird', !edit.layout.lowerThird)}><span>Lower third band</span><span class="r-switch" class:on={edit.layout.lowerThird}></span></button>
          <button class="sw-row" on:click={() => setStyle('italicRef', !edit.style.italicRef)}><span>Italic reference</span><span class="r-switch" class:on={edit.style.italicRef}></span></button>
        </div>

        <div class="grp">
          <span class="r-lbl">Transition · {edit.style.transitionMs ?? 250}ms {(edit.style.transitionMs ?? 250) === 0 ? '(cut)' : '(dissolve)'}</span>
          <input class="r-range" type="range" min="0" max="800" step="50" value={edit.style.transitionMs ?? 250} on:input={(e) => setStyle('transitionMs', +e.target.value)} />
        </div>
      </div>

      <div class="insp-foot">
        {#if err}<div class="err">{err}</div>{/if}
        <button class="r-btn amber" style="width:100%;" on:click={save} disabled={saving || !$capture.available}>
          {saving ? 'Saving…' : 'Save template'}
        </button>
      </div>
    </aside>
  {/if}
</div>

<style>
  .tpl-editor{display:grid;grid-template-columns:236px minmax(0,1fr) 328px;gap:14px;height:calc(100dvh - 150px)}
  .pane{display:flex;flex-direction:column;min-height:0;overflow:hidden;
    background:var(--v-surf);border:1px solid var(--v-line);border-radius:14px}
  .pane-head{display:flex;align-items:center;justify-content:space-between;gap:8px;padding:14px 16px;border-bottom:1px solid var(--v-line);flex:0 0 auto}
  .mini-link{background:none;border:0;color:var(--v-cyan);font-family:var(--f-mono);font-size:9px;cursor:pointer;letter-spacing:.04em;text-transform:none}

  /* list */
  .list-body{flex:1;overflow-y:auto;padding:8px}
  .tpl-row{position:relative;display:flex;align-items:center;gap:6px;border-radius:9px;padding:2px 2px 2px 4px;margin-bottom:2px;transition:background .12s}
  .tpl-row.sel{background:var(--v-surf2)}
  .tpl-row.sel::before{content:"";position:absolute;left:0;top:7px;bottom:7px;width:3px;border-radius:0 3px 3px 0;background:var(--v-amber)}
  .tpl-row:hover:not(.sel){background:var(--v-surf3)}
  .star{width:26px;height:26px;flex:0 0 auto;display:grid;place-items:center;border:0;background:none;cursor:pointer;color:var(--v-faint);border-radius:6px}
  .star.on{color:var(--v-amber)}
  .star:disabled{opacity:.35;cursor:not-allowed}
  .tpl-name{flex:1;min-width:0;text-align:left;background:none;border:0;color:var(--v-txt);font-family:var(--f-body);font-size:13px;cursor:pointer;padding:7px 2px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}
  .del{width:24px;height:24px;flex:0 0 auto;display:grid;place-items:center;border:0;background:none;cursor:pointer;color:var(--v-faint);border-radius:6px}
  .del:hover{color:var(--v-rose)}
  .del.arm{width:auto;padding:0 8px;color:var(--v-rose);background:var(--v-rose-soft)}
  .delconf{font-family:var(--f-mono);font-size:9px;font-weight:700;letter-spacing:.04em}

  /* stage */
  .stage-body{flex:1;min-height:0;display:flex;flex-direction:column;justify-content:center;gap:10px;padding:16px;
    background:radial-gradient(120% 120% at 50% 0%,#151517,#0d0d0e)}
  .wysiwyg{position:relative;width:100%;aspect-ratio:16/9;border-radius:12px;overflow:hidden;border:1px solid var(--v-line2);box-shadow:0 12px 40px rgba(0,0,0,.4)}
  .checker{position:absolute;inset:0;background:repeating-conic-gradient(#26262a 0% 25%,#161618 0% 50%) 50% / 22px 22px}
  .stage-note{font-size:10px;color:var(--v-faint);line-height:1.6;text-align:center}
  .stage-note b{color:var(--v-dim)}

  /* inspector */
  .insp{min-height:0}
  .insp-body{flex:1;overflow-y:auto;padding:14px;display:flex;flex-direction:column;gap:15px}
  .insp-foot{flex:0 0 auto;padding:12px 14px;border-top:1px solid var(--v-line)}
  .grp{display:flex;flex-direction:column;gap:8px}
  .grp .r-lbl{display:flex;align-items:center;justify-content:space-between}
  .row2{flex-direction:row;gap:12px}
  .row2>div{flex:1}
  .row3{flex-direction:row;gap:10px}
  .clr{flex:1;display:flex;flex-direction:column;gap:6px}
  .clr input[type=color]{width:100%;height:34px;border:1px solid var(--v-line2);border-radius:8px;background:var(--v-bg);cursor:pointer;padding:3px}
  .sw-row{display:flex;align-items:center;justify-content:space-between;width:100%;background:var(--v-surf2);border:1px solid var(--v-line);border-radius:9px;padding:9px 12px;color:var(--v-txt);font-size:12.5px;cursor:pointer;font-family:var(--f-body)}
  .sw-row:hover{border-color:var(--v-line2)}
  .bg-row{display:flex;flex-wrap:wrap;gap:8px}
  .file{cursor:pointer}
  .bg-thumb{width:100%;height:64px;border-radius:8px;background-size:cover;background-position:center;border:1px solid var(--v-line2);margin-top:8px}
  .seg-align{display:flex;gap:2px;background:var(--v-bg);border:1px solid var(--v-line);border-radius:9px;padding:3px}
  .seg-align button{flex:1;padding:7px;border:0;border-radius:6px;background:none;color:var(--v-dim);font-size:11.5px;text-transform:capitalize;cursor:pointer;font-family:var(--f-body)}
  .seg-align button.on{background:var(--v-amber);color:var(--v-amber-ink);font-weight:600}
  .err{color:var(--v-rose);font-size:11.5px;margin-bottom:9px;line-height:1.4}

  @media (max-width:980px){
    .tpl-editor{grid-template-columns:1fr;height:auto;gap:12px}
    .pane{max-height:none}
    .wysiwyg{max-width:640px}
  }
</style>
