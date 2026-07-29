<script>
  // PREVIEW / PROGRAM — the broadcast pair, at the top of the Library.
  //
  // ── Why a library has a switcher in it ────────────────────────────────────
  //
  // Because the Library is where content actually gets chosen mid-service, and
  // choosing is the dangerous half. Every professional tool that puts something
  // in front of an audience — ATEM, OBS, ProPresenter, EasyWorship — separates
  // the thing you are ABOUT to show from the thing you ARE showing, and makes
  // crossing that line a deliberate act.
  //
  // Relay used to fire on a single click. One slip of a trackpad put the wrong
  // scripture on a wall in front of a congregation, instantly, with no undo. The
  // fix is not a confirmation dialog — nobody reads those under pressure — it is
  // the switcher model: stage it, look at it, take it.
  //
  // LEFT is what is coming. RIGHT is what they can see. That order is the same
  // on every video desk in every gallery, and an operator who has touched one
  // already knows how to read this.
  //
  // Colour is doing real work here (DECISIONS §22):
  //   Preview  — the accent. Staged, not live. Nothing has reached anyone.
  //   Program  — amber, and ONLY when the congregation is genuinely looking.
  //              Amethyst in rehearsal, because then they are not.

  import TemplateRender from '../../TemplateRender.svelte';
  import { live, screenBlack, rehearsing, clearScreens } from '../../stores/capture.js';
  import { safeMode } from '../../boot/boot.js';

  /** The staged slide: { reference, label, text, translation, hideReference }. */
  export let preview = null;
  export let previewTemplate = null;
  export let programTemplate = null;
  export let onTake = () => {};
  export let taking = false;
  /** Side by side (above the panes) or stacked (beside them, in a column). */
  export let stacked = false;

  let clearing = false;
  async function clear() {
    clearing = true;
    // Reports its own failure through the global panic banner — never wrap this
    // in a catch that swallows it (DECISIONS §20).
    await clearScreens();
    clearing = false;
  }

  $: onAir = !!$live && !$screenBlack;
</script>

<section class="pp" class:stack={stacked}>
  <!-- ── PREVIEW ─────────────────────────────────────────────────────── -->
  <div class="pp-pane pp-preview" class:staged={!!preview}>
    <header>
      <span class="pp-tag">Preview</span>
      <span class="pp-name">{preview ? (preview.label ?? preview.reference) : 'Nothing staged'}</span>
    </header>
    <div class="pp-screen">
      {#if preview && previewTemplate}
        <TemplateRender
          template={previewTemplate}
          content={{
            reference: preview.hideReference ? null : preview.reference,
            text: preview.text,
            translation: preview.translation,
            media_url: preview.media_url,
            media_kind: preview.media_kind,
          }}
        />
      {:else if preview?.media_url}
        {#if preview.media_kind === 'video'}
          <!-- svelte-ignore a11y-media-has-caption -->
          <video class="pp-raw" src={preview.media_url} muted playsinline></video>
        {:else}
          <img class="pp-raw" src={preview.media_url} alt="" />
        {/if}
      {:else}
        <span class="pp-empty">Click a slide to stage it here</span>
      {/if}
    </div>
    <!-- The one control that crosses the line. Amber, because pressing it is
         what puts something in front of people. -->
    <button
      class="pp-take"
      disabled={!preview || taking || $safeMode}
      title={$safeMode ? 'Safe mode — nothing can reach a screen' : 'Send this to the output screens'}
      on:click={() => onTake(preview)}
    >
      {taking ? 'Taking…' : $safeMode ? 'Safe mode' : 'Take to screen →'}
    </button>
  </div>

  <!-- ── PROGRAM ─────────────────────────────────────────────────────── -->
  <div class="pp-pane pp-program" class:air={onAir && !$rehearsing} class:reh={onAir && $rehearsing}>
    <header>
      <span class="pp-tag prog" class:air={onAir && !$rehearsing} class:reh={$rehearsing}>
        {$rehearsing ? 'Rehearsal' : 'Program'}
      </span>
      <span class="pp-name">
        {$screenBlack
          ? 'Blacked out'
          : $live
            ? $live.reference || (($live.media_kind ?? '') === 'video' ? 'Video' : $live.media_url ? 'Picture' : 'Content')
            : 'Screens clear'}
      </span>
    </header>
    <div class="pp-screen">
      {#if $screenBlack}
        <span class="pp-empty">Blacked out</span>
      {:else if $live && programTemplate}
        <!-- media_url / media_kind HAVE to cross into the monitor. Without them
             a fired picture rendered as an empty template: the wall showed a
             photo, the topbar said ON AIR, and the one control that answers
             "what are they looking at" showed black. -->
        <TemplateRender
          template={programTemplate}
          content={{
            reference: $live.reference,
            text: $live.text,
            translation: $live.translation,
            media_url: $live.media_url,
            media_kind: $live.media_kind,
          }}
        />
      {:else if $live?.media_url}
        <!-- No template loaded yet. A picture is still a picture — the monitor
             must never go black while something is on the wall. -->
        {#if $live.media_kind === 'video'}
          <!-- svelte-ignore a11y-media-has-caption -->
          <video class="pp-raw" src={$live.media_url} muted playsinline autoplay loop></video>
        {:else}
          <img class="pp-raw" src={$live.media_url} alt="" />
        {/if}
      {:else if $live}
        <span class="pp-plain">{$live.text}</span>
      {:else}
        <span class="pp-empty">Nothing is on the screens</span>
      {/if}
    </div>
    <button class="pp-clear" disabled={clearing || (!$live && !$screenBlack)} on:click={clear}>
      {clearing ? 'Clearing…' : 'Clear screens'}
    </button>
  </div>
</section>

<style>
  .pp {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  /* STACKED — the column beside the verse list. Preview above Program, because
     top-to-bottom is the same "what is coming, then what they can see" order
     that left-to-right carries on a video desk. */
  .pp.stack {
    grid-template-columns: 1fr;
  }
  .pp.stack .pp-take,
  .pp.stack .pp-clear {
    margin: 8px 10px;
    height: 34px;
    flex: 0 0 auto;
  }
  /* NO max-height here. `aspect-ratio` plus a height cap makes the box shrink
     its WIDTH to keep the ratio — which drew both monitors short of the right
     edge of the column, with a dead strip beside them. The screen is 16:9 and
     full width, exactly like the wall it stands for; the column below it is
     what gives way instead. */
  .pp.stack .pp-screen {
    width: 100%;
  }
  .pp-pane {
    display: flex;
    flex-direction: column;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    overflow: hidden;
  }
  .pp-preview.staged {
    border-color: var(--v-accent-line);
  }
  /* ONLY when they are genuinely looking. */
  .pp-program.air {
    border-color: rgba(255, 176, 0, 0.5);
  }
  .pp-program.reh {
    border-color: rgba(139, 92, 246, 0.5);
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-bottom: 1px solid var(--v-line);
  }
  .pp-tag {
    flex: 0 0 auto;
    padding: 3px 9px;
    border-radius: 99px;
    background: var(--v-accent-soft);
    border: 1px solid var(--v-accent-line);
    color: var(--v-accent2);
    font-family: var(--f-mono);
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .pp-tag.prog {
    background: var(--v-grey-soft);
    border-color: var(--v-line2);
    color: var(--v-dim);
  }
  .pp-tag.prog.air {
    background: var(--v-amber-soft);
    border-color: rgba(255, 176, 0, 0.4);
    color: var(--v-amber);
  }
  .pp-tag.prog.reh {
    background: var(--v-amethyst-soft);
    border-color: rgba(139, 92, 246, 0.42);
    color: var(--v-amethyst);
  }
  .pp-name {
    flex: 1;
    min-width: 0;
    font-size: 12.5px;
    color: var(--v-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pp-screen {
    aspect-ratio: 16 / 9;
    container-type: inline-size;
    background: #000;
    display: grid;
    place-items: center;
    position: relative;
    overflow: hidden;
  }
  .pp-empty {
    font-family: var(--f-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--v-faint);
  }
  .pp-raw {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }
  .pp-plain {
    padding: 14px;
    font-family: var(--f-serif);
    font-size: 13px;
    line-height: 1.5;
    color: var(--v-dim);
  }

  .pp-take,
  .pp-clear {
    margin: 10px;
    height: 38px;
    border-radius: var(--v-r-md);
    cursor: pointer;
    font-family: var(--f-body);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.02em;
  }
  /* AMBER. Pressing this is what puts something in front of people — it is an
     ON AIR ACTION in the design system's exact sense. */
  .pp-take {
    background: var(--v-amber);
    border: 0;
    color: var(--v-amber-ink);
    box-shadow: 0 6px 18px -8px var(--v-amber-glow);
  }
  .pp-take:hover:not(:disabled) {
    filter: brightness(1.06);
  }
  .pp-take:disabled {
    background: var(--v-surf2);
    color: var(--v-faint);
    box-shadow: none;
    cursor: not-allowed;
  }
  .pp-clear {
    background: transparent;
    border: 1px solid rgba(239, 68, 68, 0.5);
    color: var(--v-rose);
  }
  .pp-clear:hover:not(:disabled) {
    background: var(--v-rose-soft);
  }
  .pp-clear:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  @media (max-width: 900px) {
    .pp {
      grid-template-columns: 1fr;
    }
  }
</style>
