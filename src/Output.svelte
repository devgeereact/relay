<script>
  import { onMount, onDestroy } from 'svelte';
  import { templateById } from './lib/templates.js';

  // The output window's template + display name come from the URL query
  // (channels.rs builds output.html?template=…&name=…). One shared renderer:
  // it interprets the template config — never branches on channel type.
  const params = new URLSearchParams(location.search);
  const t = templateById(params.get('template'));

  let content = null; // { reference, text, translation }
  let visible = false;
  let unlisten = [];

  onMount(async () => {
    try {
      const { listen } = await import('@tauri-apps/api/event');
      unlisten.push(
        await listen('output://content', (e) => {
          content = e.payload;
          visible = true;
        })
      );
      unlisten.push(
        await listen('output://clear', () => {
          visible = false;
        })
      );
    } catch {
      // Opened in a plain browser (no Tauri) — show a placeholder so the
      // template is still previewable.
      content = {
        reference: 'John 3:16 · KJV',
        text: 'For God so loved the world, that he gave his only begotten Son…',
      };
      visible = true;
    }
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<div class="stage" style="background:{t.background}; --accent:{t.accent};">
  {#if visible && content}
    <div
      class="content"
      class:lower-third={t.lowerThird}
      style="text-align:{t.align}; font-family:{t.font};"
    >
      {#if t.refFirst || (t.regions[0] === 'reference' && !t.lowerThird)}
        {#if content.reference}
          <div class="reference" style="font-size:{t.refSize};">{content.reference}</div>
        {/if}
        {#if content.text}
          <div class="verse" style="font-size:{t.verseSize}; color:{t.verseColor};">{content.text}</div>
        {/if}
      {:else}
        {#if content.text}
          <div class="verse" style="font-size:{t.verseSize}; color:{t.verseColor};">"{content.text}"</div>
        {/if}
        {#if content.reference}
          <div
            class="reference"
            style="font-size:{t.refSize}; font-style:{t.italicRef ? 'italic' : 'normal'};"
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
  .content {
    max-width: 90%;
    transition: opacity 400ms ease;
  }
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
