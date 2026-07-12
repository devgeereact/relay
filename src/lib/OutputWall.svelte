<script>
  // The output wall: every active template, rendering the SAME live content, each
  // in its own style — the product's whole promise in one component.
  //
  // The Console and the Planner each kept their own copy of this, and they had
  // already drifted (different accent arrays, different "live" borders). Two
  // copies of "what is on the congregation's screen right now" is exactly the
  // thing you cannot afford to have two versions of.
  //
  // It renders through the same TemplateRender as the real output window, so what
  // the operator sees here IS what the congregation sees.
  import TemplateRender from './TemplateRender.svelte';
  import { monitorAccent } from './templates.js';
  import { live, screenBlack, liveContent, liveTemplateOverride, navVerse } from './stores/capture.js';

  /** Active templates (max 4), from listActiveTemplates(). */
  export let templates = [];
  /** Show the ‹ › verse-nav buttons on the first monitor. Off while a plan drives the transport. */
  export let verseNav = false;
</script>

<div class="wall">
  {#if templates.length}
    {#each templates as tpl, i (tpl.id)}
      {@const acc = monitorAccent(i)}
      <div class="mon a-{acc}" class:on={$live}>
        <div class="tpl">
          <TemplateRender template={$liveTemplateOverride ?? tpl} content={$liveContent} />
        </div>
        {#if $screenBlack}<div class="mon-black"></div>{/if}

        <span class="mon-badge b-{acc}">{$live ? 'Live' : 'Style'} · {tpl.name}</span>

        <div class="mon-foot">
          {#if $live}
            <span class="r-mono">{$live.reference}{$live.translation ? ' · ' + $live.translation : ''}</span>
          {:else}
            <span class="r-mono tiny dim">{tpl.name}</span>
          {/if}
          {#if i === 0 && verseNav && $live}
            <span class="mon-nav">
              <button class="nav-btn" title="Previous verse" aria-label="Previous verse" on:click={() => navVerse('previous')}>
                <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="15 18 9 12 15 6"/></svg>
              </button>
              <button class="nav-btn" title="Next verse" aria-label="Next verse" on:click={() => navVerse('next')}>
                <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="9 18 15 12 9 6"/></svg>
              </button>
            </span>
          {/if}
        </div>
      </div>
    {/each}
  {:else}
    <div class="wall-empty">
      No active styles — activate up to 4 templates in the <b>Templates</b> tab.
    </div>
  {/if}
</div>

<style>
  .wall {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
    gap: 10px;
  }
  .mon {
    position: relative;
    border-radius: 10px;
    overflow: hidden;
    background: #000;
    border: 1px solid var(--v-line2);
    aspect-ratio: 16 / 9;
    display: flex;
    flex-direction: column;
  }
  /* Amber = on air. The only place in the app it is allowed to mean anything else
     is nowhere. A monitor is ringed ONLY when content is genuinely live. */
  .mon.on {
    border-color: var(--v-amber);
    box-shadow: 0 0 0 1px var(--v-amber-soft);
  }
  .tpl {
    position: absolute;
    inset: 0;
  }
  .mon-black {
    position: absolute;
    inset: 0;
    background: #000;
  }
  .mon-badge {
    position: absolute;
    top: 7px;
    left: 7px;
    z-index: 2;
    font-family: var(--f-mono);
    font-size: 8.5px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    padding: 3px 6px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.62);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
    max-width: calc(100% - 14px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .b-amber { color: var(--v-amber2); border-color: var(--v-amber); }
  .b-cyan { color: var(--v-cyan); border-color: rgba(63, 182, 230, 0.5); }
  .b-amethyst { color: var(--v-amethyst); border-color: rgba(192, 139, 255, 0.5); }
  .b-emerald { color: var(--v-emerald); border-color: rgba(16, 185, 129, 0.5); }
  .mon-foot {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 5px 7px;
    background: linear-gradient(transparent, rgba(0, 0, 0, 0.78));
    font-size: 9.5px;
    color: var(--v-txt);
  }
  .mon-foot .tiny { font-size: 9px; }
  .mon-foot .dim { color: var(--v-dim); }
  .mon-nav { display: flex; gap: 3px; flex: none; }
  .nav-btn {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 5px;
    background: rgba(0, 0, 0, 0.55);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
    cursor: pointer;
  }
  .nav-btn:hover {
    color: var(--v-txt);
    border-color: var(--v-amber);
  }
  .wall-empty {
    grid-column: 1 / -1;
    padding: 22px 16px;
    text-align: center;
    border: 1px dashed var(--v-line2);
    border-radius: 10px;
    font-size: 12px;
    color: var(--v-dim);
  }
</style>
