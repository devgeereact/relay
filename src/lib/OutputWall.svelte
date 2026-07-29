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
  import { live, screenBlack, liveContent, liveTemplateOverride, navVerse, rehearsing } from './stores/capture.js';

  /** Active templates (max 4), from listActiveTemplates(). */
  export let templates = [];
  /** Show the ‹ › verse-nav buttons on the first monitor. Off while a plan drives the transport. */
  export let verseNav = false;
</script>

<div class="chan-grid">
  {#if templates.length}
    {#each templates as tpl, i (tpl.id)}
      {@const acc = monitorAccent(i)}
      <!-- In rehearsal the tally does NOT light. Amber means it is in front of the
           congregation; during a rehearsal it isn't, and a tally light that lies is
           worse than no tally light. -->
      <div class="mon a-{acc}" class:on={$live && !$rehearsing} class:reh={$live && $rehearsing}>
        <div class="tpl">
          <TemplateRender template={$liveTemplateOverride ?? tpl} content={$liveContent} />
        </div>
        {#if $screenBlack}<div class="mon-black"></div>{/if}

        <span class="mon-badge b-{acc}" class:b-reh={$rehearsing}>
          {#if $rehearsing}Rehearsal{:else if $live}Live{:else}Style{/if} · {tpl.name}
        </span>

        <div class="mon-foot">
          {#if $live}
            <span class="mono">{$live.reference}{$live.translation ? ' · ' + $live.translation : ''}</span>
          {:else}
            <span class="mono tiny off">{tpl.name}</span>
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
    <div class="chan-empty">
      No active styles — activate up to 4 templates in the <b>Templates</b> tab.
    </div>
  {/if}
</div>

<style>
  /* The Console's original "Spiritual High-Tech" monitor. Kept verbatim: it reads
     as broadcast equipment, which is the point — a tally border and a hard badge,
     glanceable across a dark booth. */
  .chan-grid{display:grid;grid-template-columns:1fr 1fr;gap:16px;align-content:start}
  .mon{position:relative;aspect-ratio:16/9;border-radius:14px;overflow:hidden;background:#000;
    border:1px solid rgba(255,255,255,.12);transition:.18s}
  .mon.on.a-amber{border:2px solid var(--v-amber);box-shadow:0 0 26px -6px var(--v-amber-glow)}
  .mon.on.a-cyan{border:1px solid rgba(34,211,238,.5);box-shadow:0 0 26px -8px rgba(34,211,238,.42)}
  .mon.on.a-amethyst{border:1px solid rgba(139,92,246,.45);box-shadow:0 0 26px -8px rgba(139,92,246,.38)}
  .mon.on.a-emerald{border:1px solid rgba(34,197,94,.45);box-shadow:0 0 26px -8px rgba(34,197,94,.4)}
  /* Rehearsal: a dashed ring, never a lit one. */
  .mon.reh{border:1px dashed var(--v-amethyst);box-shadow:none}
  .mon-badge{position:absolute;top:11px;left:11px;z-index:2;padding:3px 9px;border-radius:6px;
    font-family:var(--f-body);font-size:9px;font-weight:700;letter-spacing:.09em;text-transform:uppercase;
    max-width:calc(100% - 22px);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
  .b-amber{background:var(--v-amber);color:var(--v-amber-ink)}
  .b-cyan{background:var(--v-cyan);color:#06222e}
  .b-amethyst{background:var(--v-amethyst);color:#2a0d45}
  .b-emerald{background:var(--v-emerald);color:#04291d}
  .mon:not(.on):not(.reh) .mon-badge{background:#2a2a2b;color:#c8c6ca}
  .mon-badge.b-reh{background:var(--v-amethyst);color:#2a0d45}
  .tpl{position:absolute;inset:0;overflow:hidden;background:var(--v-void);border-radius:inherit}
  .mon-black{position:absolute;inset:0;z-index:3;background:#000;border-radius:inherit}
  .mon-foot{position:absolute;left:0;right:0;bottom:0;z-index:2;display:flex;align-items:center;
    justify-content:space-between;gap:8px;padding:9px 11px;
    background:linear-gradient(to top,rgba(0,0,0,.72),transparent)}
  .mon-foot .mono{font-family:var(--f-mono);font-variant-numeric:tabular-nums;letter-spacing:.04em;
    font-size:10px;color:#fff;text-shadow:0 1px 3px rgba(0,0,0,.7)}
  .mon-foot .tiny{font-size:9px}
  .mon-foot .off{color:#8b8a8e;text-shadow:none}
  .mon-nav{display:flex;gap:6px}
  .nav-btn{width:24px;height:24px;border-radius:6px;display:grid;place-items:center;cursor:pointer;
    background:rgba(0,0,0,.5);border:1px solid rgba(255,255,255,.12);color:#fff;transition:.14s}
  .nav-btn:hover{background:rgba(0,0,0,.7);border-color:var(--v-amber);color:var(--v-amber)}
  .nav-btn:focus-visible{outline:2px solid var(--v-amber);outline-offset:2px}
  .chan-empty{grid-column:1 / -1;color:#8b8a8e;font-size:13px;line-height:1.6;padding:22px;text-align:center;
    border:1px dashed rgba(255,255,255,.12);border-radius:12px}
  .chan-empty b{color:#c8c6ca}
</style>
