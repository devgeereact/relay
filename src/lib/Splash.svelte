<script>
  // The boot screen — the first thing a church ever sees.
  //
  // There WAS a rendered reference for this screen; it was deleted on
  // 2026-09-21, and this screen had already deliberately diverged from it — the
  // 2026-09-15 design pass took out a false status badge, three feature bullets,
  // a glowing divider and a pulse glyph that the reference drew. The references
  // were never a spec: where one and the stylesheet disagreed, the stylesheet
  // was what shipped, which is why deleting them changed nothing. The reasons
  // for each removal are below, and they are the part worth keeping.
  //
  // It is DECORATION OVER A FACT, never a fact of its own. It covers the shell
  // only while the engine is being attached, and App.svelte drops it on a hard
  // timeout as well as on success — a splash that outlives its boot is an app
  // that looks hung.
  //
  // Nothing here is amber. Amber is the tally light and means the congregation
  // is looking at something; during boot nothing is on any wall. The brand is
  // amethyst, which in this app means "not reaching the screens" — which, at
  // boot, is exactly true.
  //
  // ── What this screen is allowed to SAY ────────────────────────────────────
  // Only what it actually knows, which is very little: Relay is starting. The
  // four boot stages that know things about this machine run AFTER it and
  // report themselves (lib/boot/). So there is ONE status line here and it is
  // honest; there is no invented progress, and there are no standing claims
  // dressed as readouts.
  //
  // Two of those claims were removed in the 2026-09-15 design pass:
  //   · "SAFE MODE · Outputs disabled" was hard-coded markup, so it read the
  //     same whether or not safe mode was on — CLAUDE.md rule 35's defect
  //     exactly, on the only screen where nobody could check. It is NOT that
  //     the fact is unavailable here: `boot/boot.js` derives `safeMode` from
  //     the boot record and `App.svelte` already reads it while `booting` is
  //     still true. The fact is PARTIAL — the crash-streak path can still turn
  //     safe mode on at the gate a moment later — so this screen chooses to
  //     say nothing rather than to say a half of it, and BootShell says the
  //     settled answer through a real `safe` prop one screen on.
  //   · "No Internet · Privacy First · Local Processing" were three feature
  //     bullets restating the offline line beside them, and they were already
  //     hidden below 860px — chrome that vanishes on a small booth laptop was
  //     never load-bearing.
  //
  // ── Continuity with the launch sequence ───────────────────────────────────
  // The brand row and the footer strip are deliberately the same shape and the
  // same type as BootShell's `.b-bar` / `.b-foot`, so the handover reads as one
  // screen changing rather than two different apps. If you restyle one, look at
  // the other.

  import BrandMark from './ui/BrandMark.svelte';

  /** Shown next to the wordmark. Empty in a plain browser (no backend). */
  export let version = '';
  /** The one status line. Sentence case, no ellipsis — the spinner beside it
   *  already says "still going", and two of those is one too many. */
  export let stage = 'Starting Relay';
</script>

<!-- No `aria-busy`. It tells assistive technology to withhold announcements
     until it goes false, and this region only ever unmounts — so the screen's
     one line of information was silent to a screen-reader operator for the
     whole boot. Nothing here updates in place, so there is nothing to suppress. -->
<div class="splash" role="status" aria-live="polite">
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
       of the reference title bar is reproduced here. Matches BootShell. -->
  <header class="bar">
    <BrandMark size="16px" />
    <b>RELAY</b>
    {#if version}<span class="ver r-mono">v{version}</span>{/if}
  </header>

  <div class="core">
    <!-- The hero mark, and the one focal point on the screen. SEVEN bars — this
         was hand-drawn with five, which is not the mark on the design sheet. It
         comes from the one component that also matches the app icon, sized
         fluidly with the viewport. -->
    <div class="logo">
      <BrandMark
        size="clamp(80px, 17vh, 168px)"
        fill="linear-gradient(180deg, var(--v-amethyst2), var(--v-amethyst))"
      />
    </div>

    <h1 class="word">RELAY</h1>
    <p class="tag">AI-Assisted Live Church Production</p>

    <!-- The status, beside its spinner rather than stacked above it. One row,
         one fact, at the place the eye is already looking. -->
    <p class="status">
      <span class="spin" aria-hidden="true">
        {#each Array(8) as _, i}<span style="--i:{i}"></span>{/each}
      </span>
      {stage}
    </p>
  </div>

  <!-- The one standing claim left on this screen. It is a statement about how
       Relay PROCESSES — nothing a church says leaves the device — rather than a
       claim that no socket is open: `App.svelte` runs `checkForUpdate()` in the
       same onMount, so an update check may well be in flight behind it. The
       wording is verbatim from BootShell's footer and must stay that way. It
       sits on the RIGHT because that is where BootShell puts the same words:
       the one line that survives the handover does not move. -->
  <footer class="foot">
    <span>Offline · all processing local</span>
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
  /* A very slight violet lift out of the corners, so the void is not flat.
     Centred a little above the middle, where the mark sits. */
  .splash::before {
    content: '';
    position: absolute;
    inset: 0;
    background:
      /* THE TOKEN, not a hand-typed copy of it. These were
         `rgba(169,107,245,…)`, which happens to be the CURRENT amethyst spelled
         out — so unlike the boot ladder's glow they were not the wrong purple,
         they were the right one written somewhere a hex sweep could not see it.
         That is the failure mode DESIGN_SYSTEM §1.3 describes: when the hex moves,
         every hand-written copy becomes a surface quietly painted in the old
         colour, and `workspacegrammar.test.js`'s own header says `rgba()` is not
         scanned at all. `color-mix` because these alphas are decorative washes the
         palette does not publish as tokens. */
      radial-gradient(64% 42% at 50% 38%, color-mix(in srgb, var(--v-amethyst) 8.5%, transparent), transparent 70%),
      radial-gradient(38% 58% at 2% 62%, color-mix(in srgb, var(--v-amethyst) 5%, transparent), transparent 70%),
      radial-gradient(38% 58% at 98% 44%, color-mix(in srgb, var(--v-amethyst) 5%, transparent), transparent 70%);
    pointer-events: none;
  }
  .waves {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    color: var(--v-amethyst);
    /* Quieter than it was (0.26). The line-art is the ground, not a subject —
       at 1024 the old weight ran right up to the wordmark. */
    opacity: 0.17;
    /* Two masks: fade out through the centre (so the line-art never runs behind
       the lockup) and out at the bottom (the footer strip stays clean). The
       centre window is wider than it was, because the lockup is centred and the
       art used to crowd it on a narrow window. */
    -webkit-mask-image:
      linear-gradient(90deg, #000 0%, #000 12%, transparent 40%, transparent 60%, #000 88%, #000 100%),
      linear-gradient(180deg, #000 0%, #000 80%, transparent 95%);
    -webkit-mask-composite: source-in;
    mask-image:
      linear-gradient(90deg, #000 0%, #000 12%, transparent 40%, transparent 60%, #000 88%, #000 100%),
      linear-gradient(180deg, #000 0%, #000 80%, transparent 95%);
    mask-composite: intersect;
    pointer-events: none;
  }

  /* ── Brand row ──
     60px with a hairline under it, matching BootShell's `.b-bar` exactly: this
     is the one element that survives the handover, so it must not move. */
  .bar {
    position: relative;
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: var(--v-sp-sm);
    height: 60px;
    padding: 0 28px;
    border-bottom: 1px solid var(--v-line);
  }
  .bar b {
    font-size: var(--v-fs-h2);
    font-weight: 700;
    letter-spacing: 0.1em;
    color: var(--v-txt);
  }
  .ver {
    font-size: var(--v-fs-lbl);
    color: var(--v-faint);
  }
  /* `app.css` narrows `.b-bar` to 16px below 640. The brand row is the one
     element that survives the handover, so it narrows at the same width or it
     jumps there. */
  @media (max-width: 640px) {
    .bar {
      padding: 0 16px;
    }
  }

  /* ── Centre stack ──
     Four things, not seven. The pulse glyph and the glowing divider were both
     removed: the glyph was a heartbeat line that never beat, sitting directly
     above the spinner that does the same job, and the divider put a 950px
     second light source across the screen beside the mark it was competing
     with. A boot screen gets one focal point. */
  .core {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    /* The footer is taller than the brand row, so a mathematically centred
       stack sits low. Lift it by the difference. */
    padding-bottom: clamp(16px, 3vh, 34px);
  }

  .logo {
    display: flex;
    align-items: center;
  }

  .word {
    margin: clamp(18px, 3.6vh, 40px) 0 0;
    font-family: var(--f-body);
    font-size: clamp(40px, 8vh, 84px);
    font-weight: 700;
    letter-spacing: 0.3em;
    /* The tracking pushes a phantom gap past the Y; pull it back so the
       wordmark is optically centred rather than mathematically centred. */
    text-indent: 0.3em;
    line-height: 1;
    /* A faint top-to-bottom sheen, as on the reference wordmark. */
    background: linear-gradient(180deg, #ffffff 50%, var(--v-dim) 135%);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .tag {
    margin: clamp(10px, 1.7vh, 18px) 0 0;
    font-size: clamp(10.5px, 1.3vh, 14.5px);
    font-weight: 500;
    letter-spacing: 0.24em;
    text-indent: 0.24em;
    text-transform: uppercase;
    color: var(--v-amethyst2);
  }

  /* The status and its spinner on ONE row. */
  .status {
    display: flex;
    align-items: center;
    gap: clamp(10px, 1.2vw, 16px);
    margin: clamp(30px, 5.8vh, 64px) 0 0;
    font-size: clamp(15.5px, 1.95vh, 20px);
    font-weight: 400;
    letter-spacing: 0.01em;
    /* Full text weight, not the dim ramp: everything above this line is
       identity, and this is the only INFORMATION on the screen. */
    color: var(--v-txt);
  }

  .spin {
    position: relative;
    flex: 0 0 auto;
    width: clamp(19px, 2.4vh, 24px);
    height: clamp(19px, 2.4vh, 24px);
  }
  .spin span {
    position: absolute;
    top: 0;
    left: 50%;
    width: 16%;
    height: 16%;
    margin-left: -8%;
    border-radius: var(--v-r-round);
    background: var(--v-amethyst2);
    /* The pivot is the ring's centre: half the ring's height expressed in the
       DOT's own height, so (0.5 / 0.16) = 312.5%. Recompute it whenever the
       dot size changes — 333% was a carry-over and sat the ring 0.8px high. */
    transform-origin: 50% 312.5%;
    transform: rotate(calc(var(--i) * 45deg));
    /* Motion is OPT-IN (DESIGN_SYSTEM §5): this IS the state an operator who
       asked for no animation gets, and the animation is added inside
       `no-preference`. The old shape animated by default and switched off
       under `reduce` — the same result by the riskier route, since a query
       cannot unset a declaration made outside it.
       0.55 IS NOT A ROUND NUMBER AND MUST NOT BE LOWERED. It is what the old
       `reduce` block rested at, and it is the side of the line that clears
       WCAG's 3:1 non-text floor: composited over --v-void, --v-amethyst2
       measures 3.32:1 at 0.55 and 2.06:1 at 0.35 (sRGB relative luminance,
       computed in the live DOM). These are ~3px dots in a dark booth, read by
       the one user this branch exists to serve. The animation writes opacity
       every frame, so this value reaches nobody else. */
    opacity: 0.55;
  }
  @media (prefers-reduced-motion: no-preference) {
    .spin span {
      animation: spindot 1.2s linear infinite;
      animation-delay: calc(var(--i) * 0.15s);
    }
  }
  @keyframes spindot {
    0% {
      opacity: 1;
    }
    100% {
      opacity: 0.15;
    }
  }

  /* ── Footer strip ──
     Same padding, same hairline and the same type as BootShell's `.b-foot`,
     and the text is right-aligned to land where BootShell puts the identical
     phrase. The amethyst dot and the stage slot on the left belong to the
     launch sequence, which has something to put in them; a dot here would be
     decoration standing where a real status is about to appear. */
  .foot {
    position: relative;
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 14px 28px;
    border-top: 1px solid var(--v-line);
    font-family: var(--f-mono);
    font-size: var(--v-fs-b3);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--v-faint);
  }
</style>
