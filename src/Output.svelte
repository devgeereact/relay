<script>
  import { onMount, onDestroy } from 'svelte';
  import { DEFAULT_TEMPLATE, builtinById } from './lib/templates.js';

  // The output view runs in two modes from the SAME renderer:
  //  - Desktop (Tauri): fetches its template from the DB, receives content over
  //    Tauri events, re-fetches on `template://updated` (live template edits).
  //  - Kiosk (plain browser on the LAN, e.g. a Pi): resolves the template from
  //    the built-in fallback and receives content over the WebSocket hub.
  // ONE renderer interpreting a template config — never a per-channel branch.
  const params = new URLSearchParams(location.search);
  const templateId = parseInt(params.get('template_id') || '1', 10);

  let t = DEFAULT_TEMPLATE;
  let content = null; // { reference, text, translation }
  let visible = false;
  let unlisten = [];
  let ws = null;

  async function invoke() {
    const core = await import('@tauri-apps/api/core');
    return core.invoke;
  }
  async function loadTemplate() {
    const call = await invoke();
    const tpl = await call('get_template', { id: templateId });
    if (tpl) t = tpl;
  }

  // reference-first vs verse-first, driven by template config (not channel type)
  $: refFirst = t.layout?.refFirst || (t.layout?.regions?.[0] === 'reference' && !t.layout?.lowerThird);

  function applyMessage(m) {
    if (m.kind === 'content') {
      content = { reference: m.reference, text: m.text, translation: m.translation };
      visible = true;
    } else if (m.kind === 'clear') {
      visible = false;
    }
  }

  // Kiosk mode: no Tauri runtime → use the built-in template and stream state
  // from the WebSocket hub (channels.rs, port 8031 on the app host).
  function startKiosk() {
    t = builtinById(templateId);
    const host = location.hostname || 'localhost';
    try {
      ws = new WebSocket(`ws://${host}:8031`);
      ws.onmessage = (ev) => {
        try {
          applyMessage(JSON.parse(ev.data));
        } catch {
          /* ignore malformed */
        }
      };
    } catch {
      // No hub reachable — show a static placeholder so the screen isn't blank.
      content = {
        reference: 'John 3:16 · KJV',
        text: 'For God so loved the world, that he gave his only begotten Son…',
      };
      visible = true;
    }
  }

  onMount(async () => {
    try {
      await loadTemplate();
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
      startKiosk(); // no Tauri → kiosk/browser mode
    }
  });
  onDestroy(() => {
    unlisten.forEach((u) => u());
    if (ws) ws.close();
  });
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
