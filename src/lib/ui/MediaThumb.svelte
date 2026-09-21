<script>
  // A PICTURE, SMALL, IN A LIST — the one place a media item is recognisable.
  //
  // The Library's media pane has painted the file itself since it was written,
  // and the Planner's cue inspector since RG-129's sibling. The two lists an
  // operator actually spends a Tuesday in did not: a media cue in the running
  // order was a coloured dot and a filename, and picking a background from the
  // Add-cue results meant recognising `IMG_20240714_113255.jpg`.
  //
  // It is a component rather than markup typed twice for the ordinary reason:
  // two lists, one rule, and a guarantee kept on one of two doors is this
  // repository's most-repeated bug. The rule is small and all of it matters —
  // an `<img>` for a picture, a `<video preload="metadata">` for a clip (the
  // browser paints its first frame), and NOTHING AT ALL where there is no URL.
  // A stand-in icon would be a picture of a picture, which is the thing a
  // thumbnail exists to stop somebody having to imagine.
  //
  // `url` is built by the caller through `bundledbackgrounds.js::mediaUrl`, the
  // shared builder, so a row and the wall cannot disagree about where a file
  // lives — `bundled:` included (DECISIONS §90).

  /** The file, or null when there is nothing to show (no asset, a document). */
  export let url = null;
  /** `'video'` paints a first frame; anything else is treated as a picture. */
  export let kind = 'image';
  /** The edge, in px. A list row and a search result want different sizes. */
  export let size = 30;
</script>

{#if url}
  <span class="mthumb" style="--mt:{size}px">
    {#if kind === 'video'}
      <!-- `preload="metadata"`: enough for the first frame, never the file. A
           running order with twenty clips in it must not fetch twenty videos. -->
      <video src={url} preload="metadata" muted playsinline aria-hidden="true"></video>
    {:else}
      <!-- `alt=""`: the name is already on the row beside this, and a screen
           reader announcing the filename twice is worse than not at all. -->
      <img src={url} alt="" loading="lazy" />
    {/if}
  </span>
{/if}

<style>
  .mthumb {
    flex: 0 0 auto;
    width: var(--mt);
    height: var(--mt);
    border-radius: var(--v-r-sm);
    overflow: hidden;
    background: var(--v-void);
    border: 1px solid var(--v-line2);
    display: block;
  }
  /* `cover`, not `contain`, and this is the one place the two differ from the
     wall's rule: a thumbnail is for RECOGNITION and letterboxing a landscape
     picture into a 30px square leaves eight pixels of image. The wall keeps
     `contain`, because an announcement cropped to fill is one with its edges
     cut off and nobody reading it can tell. */
  .mthumb img,
  .mthumb video {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
