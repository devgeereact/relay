<script>
  import { onMount, onDestroy } from 'svelte';
  import { DEFAULT_TEMPLATE } from './lib/templates.js';

  // The output window's template id + display name come from the URL query
  // (channels.rs builds output.html?template_id=…&name=…). The template config
  // is fetched from the DB — ONE shared renderer interpreting a config, never a
  // per-channel-type branch (CLAUDE.md). It re-fetches on `template://updated`
  // so edits in the Templates tab reflect live.
  const params = new URLSearchParams(location.search);
  const templateId = parseInt(params.get('template_id') || '0', 10);

  let t = DEFAULT_TEMPLATE;
  let content = null; // { reference, text, translation }
  let visible = false;
  let unlisten = [];

  async function invoke() {
    const core = await import('@tauri-apps/api/core');
    return core.invoke;
  }
  async function loadTemplate() {
    try {
      const call = await invoke();
      const tpl = await call('get_template', { id: templateId });
      if (tpl) t = tpl;
    } catch {
      t = DEFAULT_TEMPLATE; // browser preview
    }
  }

  // reference-first vs verse-first, driven by template config (not channel type)
  $: refFirst = t.layout?.refFirst || (t.layout?.regions?.[0] === 'reference' && !t.layout?.lowerThird);

  onMount(async () => {
    await loadTemplate();
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(
        await listen('output://content', (e) => {
          content = e.payload;
          visible = true;
        })
      );
      unlisten.push(await listen('output://clear', () => (visible = false)));
      unlisten.push(
        await listen('template://updated', (e) => {
          if (e.payload === templateId) loadTemplate();
        })
      );
    } catch {
      // Plain browser: show a placeholder so the template is previewable.
      content = {
        reference: 'John 3:16 · KJV',
        text: 'For God so loved the world, that he gave his only begotten Son…',
      };
      visible = true;
    }
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<div class="stage" style="background:{t.style?.background}; --accent:{t.style?.accent};">
  {#if visible && content}
    <div
      class="content"
      class:lower-third={t.layout?.lowerThird}
      style="text-align:{t.layout?.align}; font-family:{t.style?.font};"
    >
      {#if refFirst}
        {#if content.reference}
          <div class="reference" style="font-size:{t.style?.refSize};">{content.reference}</div>
        {/if}
        {#if content.text}
          <div class="verse" style="font-size:{t.style?.verseSize}; color:{t.style?.verseColor};">{content.text}</div>
        {/if}
      {:else}
        {#if content.text}
          <div class="verse" style="font-size:{t.style?.verseSize}; color:{t.style?.verseColor};">"{content.text}"</div>
        {/if}
        {#if content.reference}
          <div
            class="reference"
            style="font-size:{t.style?.refSize}; font-style:{t.style?.italicRef ? 'italic' : 'normal'};"
          >{content.reference}</div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  :global(html, body) { margin: 0; height: 100%; background: #000; overflow: hidden; }
  .stage {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6vh 7vw;
  }
  .content { max-width: 90%; transition: opacity 400ms ease; }
  .content.lower-third {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 6vh;
    max-width: 100%;
    background: linear-gradient(90deg, rgba(176, 128, 224, 0.95), rgba(176, 128, 224, 0.75));
    padding: 2.4vh 4vw;
  }
  .verse { line-height: 1.35; }
  .reference { margin-top: 1.4vh; color: var(--accent); font-weight: 600; }
</style>
