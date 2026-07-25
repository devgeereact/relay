<script>
  // Fullscreen in-console preview of a template (Decision §26). Pure console —
  // it renders through the SAME TemplateRender the wall uses, but reaches NO
  // output, so it is always safe (even in a live service). Shared by the gallery
  // and the editor.
  //
  // role="dialog" is load-bearing: shortcuts.js suppresses the panic keys while a
  // [role="dialog"] is mounted (rule 16), so Esc dismisses THIS preview instead
  // of clearing the congregation's screens. The keydown below closes it.
  import { onMount, onDestroy } from 'svelte';
  import TemplateRender from './TemplateRender.svelte';
  import { SAMPLE_TEST_CONTENT } from './templateTest.js';

  export let template = {};
  export let onClose = () => {};

  function onKey(e) {
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); onClose(); }
  }
  onMount(() => window.addEventListener('keydown', onKey, true));
  onDestroy(() => window.removeEventListener('keydown', onKey, true));
</script>

<div class="tpv-scrim" role="dialog" aria-modal="true" aria-label="Template preview">
  <button class="tpv-scrimbtn" aria-label="Close preview" on:click={onClose}></button>
  <div class="tpv-frame">
    <TemplateRender {template} content={SAMPLE_TEST_CONTENT} />
  </div>
  <button class="r-btn sm tpv-close" on:click={onClose}>Close preview · Esc</button>
</div>

<style>
  .tpv-scrim{ position:fixed; inset:0; z-index:1200; display:grid; place-items:center;
    background:rgba(6,6,9,.88); backdrop-filter:blur(4px); padding:4vh 4vw; }
  /* A full-bleed button behind the frame so a click anywhere off the preview
     closes it, without swallowing clicks on the frame or the labelled button. */
  .tpv-scrimbtn{ position:absolute; inset:0; border:0; background:transparent; cursor:pointer; }
  /* 16:9, sized to the viewport. position:relative is load-bearing — TemplateRender
     is position:absolute; inset:0 and supplies its own container-type. */
  .tpv-frame{ position:relative; width:min(92vw, calc(88vh * 16 / 9)); aspect-ratio:16/9;
    border-radius:var(--v-r-lg); overflow:hidden; background:var(--v-void);
    box-shadow:var(--v-shadow-lg); border:1px solid var(--v-line2); }
  .tpv-close{ position:absolute; bottom:3vh; left:50%; transform:translateX(-50%); }
</style>
