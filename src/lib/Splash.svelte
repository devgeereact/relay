<script>
  // The boot screen — docs/design/relay-splash-screen.png.
  //
  // It is DECORATION OVER A FACT, never a fact of its own. It covers the shell
  // only while the engine is being attached, and App.svelte drops it on a hard
  // timeout as well as on success — a splash that outlives its boot is an app
  // that looks hung, and this one is the first thing an operator ever sees.
  //
  // Nothing here is amber. Amber is the tally light and means the congregation
  // is looking at something; during boot nothing is on any wall. The brand is
  // amethyst, which in this app means "not reaching the screens" — which, at
  // boot, is exactly true.

  import BrandMark from './ui/BrandMark.svelte';

  /** Shown next to the wordmark. Empty in a plain browser (no backend). */
  export let version = '';
  /** The line that changes as boot progresses. */
  export let stage = 'starting engine…';
  /** The quieter line under the spinner. */
  export let detail = 'Initializing offline systems';
</script>

<div class="splash" role="status" aria-live="polite" aria-busy="true">
  <!-- Edge line-art. Pure decoration, so it is hidden from assistive tech. -->
  <svg class="waves" viewBox="0 0 1536 1024" preserveAspectRatio="none" aria-hidden="true">
    <g fill="none" stroke="currentColor" stroke-width="1">
      {#each Array(13) as _, i}
        <path d="M-40 {395 + i * 27} C 100 {325 + i * 30}, 230 {715 + i * 22}, {360 + i * 26} {690 + i * 26}" />
        <path d="M1576 {325 + i * 27} C 1440 {255 + i * 30}, 1330 {675 + i * 22}, {1200 - i * 26} {630 + i * 26}" />
      {/each}
    </g>
  </svg>

  <!-- Brand row. The native window keeps its own minimise/maximise/close —
       Relay does not draw its own window controls, so only the identity half
       of the reference title bar is reproduced here. -->
  <header class="bar">
    <BrandMark size="16px" />
    <b>RELAY</b>
    {#if version}<span class="ver r-mono">v{version}</span>{/if}
  </header>

  <div class="core">
    <!-- The hero mark. SEVEN bars — this was hand-drawn with five, which is not
         the mark on the design sheet. It now comes from the one component that
         also matches the app icon, sized fluidly with the viewport. -->
    <div class="logo">
      <BrandMark
        size="clamp(96px, 22.5vh, 230px)"
        fill="linear-gradient(180deg, var(--v-amethyst2), var(--v-amethyst))"
      />
    </div>

    <h1 class="word">RELAY</h1>
    <p class="tag">AI-Assisted Live Church Production</p>
    <div class="rule" aria-hidden="true"></div>

    <svg class="pulse" viewBox="0 0 120 24" fill="none" aria-hidden="true">
      <path
        d="M0 12h44l6-9 6 18 5-13 5 8 4-4h50"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>

    <p class="stage">{stage}</p>

    <div class="spin" aria-hidden="true">
      {#each Array(8) as _, i}<span style="--i:{i}"></span>{/each}
    </div>

    <p class="detail">{detail}</p>
  </div>

  <footer class="foot">
    <div class="f-lead">
      <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor"
        stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M12 3 4.5 6v5.5c0 4.4 3.1 8.3 7.5 9.5 4.4-1.2 7.5-5.1 7.5-9.5V6L12 3Z" />
        <path d="m9 12 2 2 4-4" />
      </svg>
      <span class="f-t"><b>Offline Mode</b><i>All systems local</i></span>
    </div>

    <ul class="f-mid">
      <li>
        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M6.5 18.5A4.5 4.5 0 0 1 6 9.6a6 6 0 0 1 9.3-3.7" />
          <path d="M17.3 10a4.5 4.5 0 0 1 .7 8.5H9" /><path d="m3 3 18 18" />
        </svg>No Internet
      </li>
      <li>
        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="4" y="10.5" width="16" height="10" rx="2" />
          <path d="M8 10.5V7.5a4 4 0 0 1 8 0v3" />
        </svg>Privacy First
      </li>
      <li>
        <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect x="3.5" y="4.5" width="17" height="6" rx="1.6" />
          <rect x="3.5" y="13.5" width="17" height="6" rx="1.6" />
          <path d="M7 7.5h.01M7 16.5h.01" />
        </svg>Local Processing
      </li>
    </ul>

    <div class="f-trail">
      <span class="f-dot"></span>
      <span class="f-t"><b>Safe Mode</b><i>Outputs disabled</i></span>
    </div>
  </footer>
</div>

<style>
  /* Sits above everything except the panic bar (z 1200) — a boot screen must
     never be able to hide the one message that says the wall may still be live. */
  .splash {
    position: fixed;
    inset: 0;
    z-index: 1100;
    display: flex;
    flex-direction: column;
    background: var(--v-void);
    color: var(--v-txt);
    font-family: var(--f-body);
    overflow: hidden;
  }
  /* A very slight violet lift out of the corners, as on the reference. */
  .splash::before {
    content: '';
    position: absolute;
    inset: 0;
    background:
      radial-gradient(70% 45% at 50% 42%, rgba(139, 92, 246, 0.09), transparent 70%),
      radial-gradient(40% 60% at 3% 60%, rgba(139, 92, 246, 0.06), transparent 70%),
      radial-gradient(40% 60% at 97% 45%, rgba(139, 92, 246, 0.06), transparent 70%);
    pointer-events: none;
  }
  .waves {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    color: var(--v-amethyst);
    opacity: 0.26;
    /* Two masks: fade out through the centre (so the line-art never runs behind
       the wordmark) and out at the bottom (the reference's footer is clean). */
    -webkit-mask-image:
      linear-gradient(90deg, #000 0%, #000 18%, transparent 44%, transparent 56%, #000 82%, #000 100%),
      linear-gradient(180deg, #000 0%, #000 84%, transparent 96%);
    -webkit-mask-composite: source-in;
    mask-image:
      linear-gradient(90deg, #000 0%, #000 18%, transparent 44%, transparent 56%, #000 82%, #000 100%),
      linear-gradient(180deg, #000 0%, #000 84%, transparent 96%);
    mask-composite: intersect;
    pointer-events: none;
  }

  /* ── Brand row ── */
  .bar {
    position: relative;
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: var(--v-sp-sm);
    height: 68px;
    padding: 0 28px;
  }
  .bar b {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--v-txt);
  }
  .ver {
    font-size: 12px;
    color: var(--v-faint);
  }

  /* ── Centre stack ── */
  .core {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding-top: clamp(8px, 2.2vh, 22px);
  }

  .logo {
    display: flex;
    align-items: center;
  }

  .word {
    margin: clamp(14px, 3.5vh, 36px) 0 0;
    font-family: var(--f-body);
    font-size: clamp(52px, 12.8vh, 132px);
    font-weight: 700;
    letter-spacing: 0.355em;
    /* The tracking pushes a phantom gap past the Y; pull it back so the
       wordmark is optically centred rather than mathematically centred. */
    text-indent: 0.355em;
    line-height: 1;
    /* A faint top-to-bottom sheen, as on the reference wordmark. */
    background: linear-gradient(180deg, #ffffff 45%, var(--v-dim) 130%);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .tag {
    margin: clamp(6px, 1.35vh, 14px) 0 0;
    font-size: clamp(11px, 1.95vh, 20px);
    font-weight: 500;
    letter-spacing: 0.29em;
    text-indent: 0.29em;
    text-transform: uppercase;
    color: var(--v-amethyst2);
  }
  .rule {
    position: relative;
    margin-top: clamp(12px, 3vh, 31px);
    width: min(62%, 950px);
    height: 1px;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(139, 92, 246, 0.28) 22%,
      var(--v-amethyst2) 50%,
      rgba(139, 92, 246, 0.28) 78%,
      transparent
    );
    box-shadow: 0 0 12px 0 rgba(139, 92, 246, 0.3);
  }
  /* The reference lights the divider from a hotspot at its centre — a soft
     violet bloom that reads as a glow, not a second rule. */
  .rule::after {
    content: '';
    position: absolute;
    left: 50%;
    top: 50%;
    width: 34%;
    height: 46px;
    transform: translate(-50%, -50%);
    background: radial-gradient(
      50% 50% at 50% 50%,
      rgba(196, 176, 255, 0.5),
      rgba(139, 92, 246, 0.18) 38%,
      transparent 72%
    );
    pointer-events: none;
  }

  .pulse {
    margin-top: clamp(20px, 5.5vh, 56px);
    width: clamp(66px, 8vw, 120px);
    color: var(--v-amethyst2);
  }
  .stage {
    margin: clamp(8px, 2.05vh, 21px) 0 0;
    font-size: clamp(18px, 3.3vh, 34px);
    font-weight: 400;
    color: var(--v-txt);
  }

  .spin {
    position: relative;
    margin-top: clamp(14px, 2.9vh, 30px);
    width: clamp(30px, 5.4vh, 55px);
    height: clamp(30px, 5.4vh, 55px);
  }
  .spin span {
    position: absolute;
    top: 0;
    left: 50%;
    width: 13%;
    height: 13%;
    margin-left: -6.5%;
    border-radius: 50%;
    background: var(--v-amethyst2);
    transform-origin: 50% 385%;
    transform: rotate(calc(var(--i) * 45deg));
    opacity: 0.2;
    animation: spindot 1.2s linear infinite;
    animation-delay: calc(var(--i) * 0.15s);
  }
  @keyframes spindot {
    0% {
      opacity: 1;
    }
    100% {
      opacity: 0.15;
    }
  }
  .detail {
    margin: clamp(10px, 2.35vh, 24px) 0 0;
    font-size: clamp(13px, 2vh, 21px);
    color: var(--v-faint);
  }

  /* ── Footer strip ── */
  .foot {
    position: relative;
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: var(--v-sp-lg);
    padding: clamp(14px, 2.35vh, 24px) clamp(20px, 3.6vw, 56px) clamp(22px, 3.8vh, 39px);
    border-top: 1px solid var(--v-line);
  }
  .f-lead,
  .f-trail {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1 1 0;
  }
  .f-trail {
    justify-content: flex-end;
  }
  .f-lead > svg {
    color: var(--v-emerald);
    flex: 0 0 auto;
  }
  .f-dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    flex: 0 0 auto;
    background: var(--v-amethyst);
  }
  .f-t {
    display: flex;
    flex-direction: column;
    gap: 2px;
    line-height: 1.2;
  }
  .f-t b {
    font-size: 15px;
    font-weight: 700;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    color: var(--v-txt);
  }
  .f-t i {
    font-size: 13px;
    font-style: normal;
    color: var(--v-faint);
  }
  .f-trail .f-t {
    align-items: flex-end;
  }

  .f-mid {
    display: flex;
    align-items: center;
    gap: clamp(14px, 2vw, 26px);
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .f-mid li {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 15px;
    color: var(--v-dim);
  }
  .f-mid li + li {
    padding-left: clamp(14px, 2vw, 26px);
    border-left: 1px solid var(--v-line2);
  }
  .f-mid svg {
    color: var(--v-faint);
    flex: 0 0 auto;
  }

  /* Tight windows: the footer's middle rail is the first thing to go. */
  @media (max-width: 860px) {
    .f-mid {
      display: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .spin span {
      animation: none;
      opacity: 0.55;
    }
  }
</style>
