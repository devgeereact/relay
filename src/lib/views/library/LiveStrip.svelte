<script>
  // WHAT IS ON THE WALL, RIGHT NOW — shown inside the Library.
  //
  // Asked for directly: *"make it work like ProPresenter where you can see what
  // was pushed to the screen from the library."*
  //
  // The console already has a program monitor, but it is on the Live tab. An
  // operator browsing the Bible mid-service had to leave the thing they were
  // doing to answer "what is up there?" — and the honest answer to that question
  // is the single most important fact in the product.
  //
  // It renders through `TemplateRender`, the same component the projector uses,
  // so this is not a description of the output. It is the output, smaller.
  //
  // The colour law is the whole design here (DECISIONS §22):
  //   amber      — the congregation is looking at this
  //   amethyst   — rehearsal: nothing is reaching them
  //   grey       — the screens are clear
  //   red        — blacked out

  import TemplateRender from '../../TemplateRender.svelte';
  import { live, screenBlack, rehearsing, clearScreens } from '../../stores/capture.js';

  export let template = null;

  let clearing = false;
  async function clear() {
    clearing = true;
    // clearScreens() reports its own failure through the global panic banner —
    // it must never be wrapped in a catch that swallows it (DECISIONS §20).
    await clearScreens();
    clearing = false;
  }

  $: state = $rehearsing
    ? 'rehearsal'
    : $screenBlack
      ? 'black'
      : $live
        ? 'air'
        : 'clear';
</script>

<section class="ls" class:air={state === 'air'} class:reh={state === 'rehearsal'}>
  <div class="ls-thumb">
    {#if $screenBlack}
      <span class="ls-empty">Blacked out</span>
    {:else if $live && template}
      <TemplateRender
        {template}
        content={{
          reference: $live.reference,
          text: $live.text,
          translation: $live.translation,
        }}
      />
    {:else if $live}
      <span class="ls-plain">{$live.text}</span>
    {:else}
      <span class="ls-empty">Screens clear</span>
    {/if}
  </div>

  <div class="ls-meta">
    <p class="r-lbl">On the screens now</p>
    {#if state === 'air'}
      <b class="ls-ref">{$live.reference || 'Content'}</b>
      <span class="ls-state air">● On air{$live.translation ? ` · ${$live.translation}` : ''}</span>
    {:else if state === 'rehearsal'}
      <b class="ls-ref">{$live?.reference || 'Rehearsing'}</b>
      <!-- The one case where showing content must NOT read as on air. -->
      <span class="ls-state reh">● Rehearsal — nothing is reaching the congregation</span>
    {:else if state === 'black'}
      <b class="ls-ref">Blackout</b>
      <span class="ls-state black">● Every output is dark</span>
    {:else}
      <b class="ls-ref">Nothing</b>
      <span class="ls-state">● The screens are clear</span>
    {/if}
  </div>

  {#if $live || $screenBlack}
    <button class="r-btn danger sm" on:click={clear} disabled={clearing}>
      {clearing ? 'Clearing…' : 'Clear screens'}
    </button>
  {/if}
</section>

<style>
  .ls {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: var(--v-r-lg);
    background: var(--v-surf);
    border: 1px solid var(--v-line);
  }
  /* Only when the congregation is actually looking at something. */
  .ls.air {
    border-color: rgba(255, 176, 0, 0.45);
  }
  .ls.reh {
    border-color: rgba(139, 92, 246, 0.45);
  }

  .ls-thumb {
    flex: 0 0 auto;
/* The single most important readout in the app while a service is running.
       It was 132px — big enough to see that SOMETHING was up, too small to read
       WHAT. */
    width: 264px;
    aspect-ratio: 16 / 9;
    container-type: inline-size;
    border-radius: var(--v-r-md);
    overflow: hidden;
    background: #000;
    border: 1px solid var(--v-line2);
    display: grid;
    place-items: center;
    position: relative;
  }
  .ls-empty {
    font-family: var(--f-mono);
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--v-faint);
  }
  .ls-plain {
    padding: 8px;
    font-family: var(--f-serif);
    font-size: 12px;
    line-height: 1.45;
    color: var(--v-dim);
  }

  .ls-meta {
    flex: 1;
    min-width: 0;
  }
  .ls-ref {
    display: block;
    margin: 5px 0 3px;
    font-size: 15px;
    font-weight: 600;
    color: var(--v-txt);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ls-state {
    font-size: 12px;
    color: var(--v-faint);
  }
  .ls-state.air {
    color: var(--v-amber);
  }
  .ls-state.reh {
    color: var(--v-amethyst);
  }
  .ls-state.black {
    color: var(--v-red);
  }

  @media (max-width: 640px) {
    .ls-thumb {
      width: 168px;
    }
  }
</style>
