<script>
  // The Relay waveform mark — the ONE copy.
  //
  // Traced from the BRAND block of docs/relaydesign/relay-designsystem.png, and
  // geometrically identical to the app icon (src-tauri/icons/relay-mark.svg):
  // SEVEN bars, centre tallest, rounded caps, the outer pair set back.
  //
  // It exists because the mark was hand-inlined in two places with FIVE bars
  // each, which is not the mark on the design sheet — and a brand drawn slightly
  // differently everywhere it appears is not a brand. Anything that needs the
  // mark imports this; nothing redraws it.
  //
  // Amethyst, never amber. Amber is the tally light: it means the congregation
  // is looking at something. A logo wearing it permanently is that colour
  // telling a lie before the app has finished starting.
  //
  // ── Sizing ────────────────────────────────────────────────────────────────
  // `size` is ANY CSS length, including a clamp() — the splash scales the hero
  // mark with the viewport. That works because the geometry is expressed in `em`
  // against a font-size set to `size`, so one number drives width, gaps and all
  // seven heights. Do not convert this back to px maths: it would cost the
  // splash its fluid mark, which is the largest thing on the first screen an
  // operator ever sees.

  /** Height of the TALLEST bar. Any CSS length: '18px', 'clamp(96px,22vh,230px)'. */
  export let size = '18px';
  /** Bar colour. Defaults to the brand amethyst. */
  export let color = 'var(--v-amethyst)';
  /** Optional gradient for the hero treatment; overrides `color` when set. */
  export let fill = '';

  // Ratios lifted straight from the icon: 116 / 252 / 412 / 540 of the centre bar.
  const BARS = [0.215, 0.467, 0.763, 1, 0.763, 0.467, 0.215];
</script>

<span class="mark" style="font-size:{size}; --c:{color}; --fill:{fill || color};" aria-hidden="true">
  {#each BARS as h, i}
    <i class:mute={i === 0 || i === BARS.length - 1} style="height:{h}em"></i>
  {/each}
</span>

<style>
  .mark {
    display: inline-flex;
    align-items: center;
    flex: 0 0 auto;
    height: 1em;
    /* 54 wide / 34 gap against a 540 centre bar, from the icon geometry. */
    gap: 0.063em;
    line-height: 0;
  }
  .mark i {
    width: 0.1em;
    min-width: 2px;
    border-radius: 99px;
    background: var(--fill);
  }
  /* The outer pair sit back, as on the design sheet. */
  .mark i.mute {
    opacity: 0.62;
  }
</style>
