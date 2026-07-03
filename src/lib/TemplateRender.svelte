<script>
  // ONE renderer for both the fullscreen output (Output.svelte) and the editor
  // preview (Templates.svelte) — guarantees WYSIWYG: what you save is exactly
  // what shows. Sizes are in `cqw` (container-query width units) so the same
  // template scales identically whether the container is a full screen or a
  // small preview box.
  export let template = {};
  export let content = null; // { reference, text, translation }

  $: layout = template?.layout ?? {};
  $: style = template?.style ?? {};
  $: refFirst =
    layout.refFirst || (layout.regions?.[0] === 'reference' && !layout.lowerThird);

  // Auto-shrink long verses so they never overflow the screen; short ones stay
  // large. Combined with wrapping, text always fits.
  $: len = content?.text?.length ?? 0;
  $: shrink = len > 260 ? 0.6 : len > 180 ? 0.72 : len > 110 ? 0.85 : 1;
  $: verseSize = (parseFloat(style.verseSize) || 6) * shrink;
  $: refSize = parseFloat(style.refSize) || 2.6;

  // On a lower-third band the accent IS the background, so the reference uses
  // the verse (readable) color; elsewhere the accent tints the reference.
  $: refColor = layout.lowerThird ? style.verseColor || '#1c1224' : style.accent || 'var(--amber)';

  $: show = (r) => layout.regions?.includes(r);
</script>

<div class="stage" style="background:{style.background}; --accent:{style.accent || 'var(--amber)'};">
  {#if content}
    <div
      class="content"
      class:lower-third={layout.lowerThird}
      style="text-align:{layout.align || 'center'}; font-family:{style.font || 'var(--f-serif)'};"
    >
      {#if refFirst}
        {#if show('reference') && content.reference}
          <div class="reference" style="font-size:{refSize}cqw; color:{refColor};">{content.reference}</div>
        {/if}
        {#if show('verse_text') && content.text}
          <div class="verse" style="font-size:{verseSize}cqw; color:{style.verseColor || '#f4e4c8'};">{content.text}</div>
        {/if}
      {:else}
        {#if show('verse_text') && content.text}
          <div class="verse" style="font-size:{verseSize}cqw; color:{style.verseColor || '#f4e4c8'};">"{content.text}"</div>
        {/if}
        {#if show('reference') && content.reference}
          <div class="reference" style="font-size:{refSize}cqw; color:{refColor}; font-style:{style.italicRef ? 'italic' : 'normal'};">{content.reference}</div>
        {/if}
      {/if}
    </div>
  {/if}
</div>

<style>
  .stage {
    position: absolute;
    inset: 0;
    container-type: size;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6% 7%;
    box-sizing: border-box;
    overflow: hidden;
  }
  .content {
    max-width: 90%;
    max-height: 92%;
    overflow: hidden;
  }
  /* Lower third: a band pinned to the bottom, rest transparent so a camera /
     ATEM / OBS source shows through the top. */
  .content.lower-third {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 6%;
    max-width: 100%;
    background: var(--accent);
    padding: 2.4% 4%;
  }
  .verse {
    line-height: 1.32;
    /* wrap long scripture; never run off the edge */
    overflow-wrap: anywhere;
    word-break: break-word;
    white-space: pre-wrap;
  }
  .reference {
    margin-top: 1.4%;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
</style>
