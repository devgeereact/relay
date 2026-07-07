<script>
  // Generic content-catalog view — one component drives every Library content
  // type (Scripture / Lyrics / Media / Announcements). Data is placeholder for
  // this layout-sketch pass; each type gets its real store wiring when we go
  // deeper. Kept deliberately dumb (props in, no side effects) so the "going
  // deeper" step only swaps `items` for a real query.
  export let kind = 'ITEM'; // mono badge shown on each thumbnail
  export let accent = 'var(--v-amber)'; // content-type accent (see architecture doc)
  export let addLabel = 'Add to Plan';
  export let scripture = false; // scripture uses a serif verse thumbnail
  export let blurb = '';
  export let items = []; // [{ title, meta, quote? }]
</script>

<div class="cat">
  <div class="cat-bar">
    <input class="r-input" placeholder="Search…" aria-label="Search content" />
    <span class="spring"></span>
    <span class="cat-sketch">Layout sketch · wiring next</span>
  </div>

  {#if blurb}<p class="r-lead" style="margin:0;">{blurb}</p>{/if}

  {#if items.length}
    <div class="cat-grid">
      {#each items as it}
        <div class="cat-card">
          <div class="cat-thumb" class:scripture>
            <span class="cat-kind">{kind}</span>
            {#if scripture && it.quote}<span class="q">{it.quote}</span>{/if}
          </div>
          <div class="cat-t">{it.title}</div>
          <div class="cat-m">{it.meta}</div>
          <button class="r-btn ghost sm cat-add">
            <span style="color:{accent};font-weight:700;">＋</span> {addLabel}
          </button>
        </div>
      {/each}
    </div>
  {:else}
    <div class="cat-empty"><span class="r-empty">Nothing here yet.</span></div>
  {/if}
</div>
