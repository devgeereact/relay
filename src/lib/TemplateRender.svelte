<script context="module">
  /**
   * Does the fit that just ran need doing again?
   *
   * Pure, and exported, because it is the rule that decides whether a
   * congregation reads a whole verse or one with its first and last lines cut
   * through the middle — and a rule that important should not only be testable
   * through a browser.
   *
   * @param fontReady    is the face the text will be PAINTED in available yet?
   *                     A webfont is fetched only when something first uses it,
   *                     so on an output page (which opens with nothing on it)
   *                     the very first verse is measured in the fallback, fits
   *                     in fewer lines, and then grows when the real face lands.
   * @param overflowing  does the box overflow right now? Catches the non-font
   *                     causes; it cannot catch the font one, because the reflow
   *                     the new face causes has not happened yet when we look.
   * @param tries        re-fits already spent on this content.
   * @param max          the ceiling. Text that does not fit at ANY size is rule
   *                     37's case: it is reported through `onFit`, not retried
   *                     forever.
   */
  export function needsRefit({ fontReady, overflowing, tries, max = 2 }) {
    if (tries >= max) return false;
    return !fontReady || !!overflowing;
  }
</script>

<script>
  // ONE renderer for both the fullscreen output (Output.svelte) and the editor
  // preview (Templates.svelte) — guarantees WYSIWYG: what you save is exactly
  // what shows. Sizes are in `cqw` (container-query width units) so the same
  // template scales identically whether the container is a full screen or a
  // small preview box.
  import { afterUpdate, onMount, onDestroy } from 'svelte';
  import { isLayered, boundValue, templateShows, formatElapsed, formatRemaining, formatCountdown, countdownWarning, topLevelLayers, drawBoxes } from './layers.js';
  // ONE timer, ONE formatter (docs/REBRAND.md §7). `layers.js` owns the formatter;
  // `countdown.js` owns the arithmetic in front of it — including the one exception,
  // a countdown that is being HELD.
  import { countdownRemainingMs, countdownIsPaused, countdownTotalMs } from './countdown.js';
  import { applySink, getAudioOutput, onAudioOutputChange } from './audioOutput.js';

  export let template = {};
  export let content = null; // { reference, text, translation }
  // Optional theme (the style layer BENEATH the template — see themes.js). When
  // present its whitelisted defaults fill the keys the template leaves unset;
  // the template always WINS per key, so a themed render is byte-identical to a
  // hand-styled one downstream. When ABSENT (the default) resolution is a no-op
  // and this component behaves exactly as it did before themes existed — zero
  // regression on every existing call site.
  export let theme = null;
  import { applyTheme, themeById, templateThemeRef, BUILTIN_THEMES } from './themes.js';
  import { resolveStyle, slideBG, faceOf, fitScale, keepShrinking, FIT_STEP } from './templatemodel.js';
  import { transitionCss, transitionDuration, resolveTransition, isOverride, liveTransition } from './transitions.js';
  import { builtinById } from './templates.js';
  // Sound is OPT-IN per surface. This same renderer draws the Templates editor
  // preview, and editing a template must not blast video audio across the room —
  // so only a real output surface passes audio={true}.
  export let audio = false;
  /**
   * How deep this render is inside a composite. 0 is the screen itself.
   *
   * A REGION LAYER ONLY RENDERS AT DEPTH 0 — "a composite may not be another
   * composite's fill" (docs/REBRAND.md §6). Without it a template that names
   * itself would recurse until the webview died, on a wall, mid-service.
   */
  export let depth = 0;

  // The theme to apply. An EXPLICIT `theme` prop always wins (the Themes editor
  // previewing an unsaved draft, or an output page that resolved a CUSTOM theme
  // from the DB). Otherwise the template's own pinned theme (`style.themeRef`) is
  // resolved against the BUILT-IN themes — which are bundled everywhere, so every
  // surface (console previews, gallery cards, the wall) shows a builtin-themed
  // template correctly with NO per-surface wiring. Custom themes carry no store
  // here, so they resolve only where a caller injects them via the prop.
  $: effectiveTheme = theme ?? themeById(templateThemeRef(template), BUILTIN_THEMES);
  // Always run applyTheme: with a theme it merges style + resolves layer tokens;
  // WITHOUT one it still resolves any layer theme-tokens to their literal
  // fallbacks (a literal template hits applyTheme's fast path and is unchanged).
  $: resolved = applyTheme(template, effectiveTheme);
  $: layout = resolved?.layout ?? {};
  // THE MODEL, not a bag of keys. `resolveStyle` migrates the legacy
  // whole-template properties onto their elements and fills every default in one
  // place, so the editor's preview and the wall cannot disagree about what an
  // unset property looks like (docs/REBRAND.md §3.1). It deliberately does NOT
  // answer for `background` or alignment — see the transparency law below.
  $: style = resolveStyle(resolved?.style ?? {});

  // ── LAYER MODE ─────────────────────────────────────────────────────────────
  // When a template carries `layout.layers`, render the free-form layer stack;
  // otherwise fall back to the legacy region rendering below (so the built-in
  // presets and themes are untouched). Layers are drawn back-to-front.
  $: layered = isLayered(template);
  $: layers = layered ? layout.layers : [];

  // ── Media policy ───────────────────────────────────────────────────────────
  // A fired picture/video SHOWS BY DEFAULT — a template needs no media layer for
  // the wall to display media (that's the common case: the main screen just shows
  // it). A MEDIA layer is opt-IN placement: add one and media renders at THAT
  // layer's box/z-order instead of full-frame. A screen can opt OUT entirely with
  // `layout.noMedia` (the editor's "Show media" toggle) — e.g. a lower third that
  // must keep the camera clean while a picture is on the main wall.
  $: allowMedia = templateShows(template, 'media');
  $: hasMediaLayer = layered && layers.some((L) => L.type === 'media' && L.visible !== false);
  // Media with no placement layer → it fills the frame (drawn on top).
  $: showFullMedia = !!content?.media_url && allowMedia && !hasMediaLayer;

  // ── Countdown policy ─────────────────────────────────────────────────────────
  // Same idea as media: a fired countdown shows its MM:SS BY DEFAULT — the wall
  // needs no timer layer. A layered template's scripture text layers can't render
  // a countdown (they bind to the verse/reference, which a countdown doesn't
  // carry), so without this a countdown showed only its label and no digits. A
  // TIMER layer (or any layer bound to 'countdown') is opt-in placement; when one
  // exists it renders the MM:SS itself and this default steps aside.
  $: hasTimerLayer = layered && layers.some((L) => L.visible !== false && (L.type === 'timer' || L.bind === 'countdown'));
  $: showDefaultCountdown = layered && content?.countdown_to != null && !hasTimerLayer;
  const fontFamOf = (f) => {
    const v = f || 'var(--f-serif)';
    return v.startsWith('var(') ? v : `${v}, system-ui, sans-serif`;
  };
  const shadowOf = (k) => {
    const n = Math.max(0, Math.min(1, Number(k) || 0));
    return n > 0 ? `0 ${(0.1 * n).toFixed(3)}em ${(0.35 * n).toFixed(3)}em rgba(0,0,0,${(0.9 * n).toFixed(2)})` : 'none';
  };
  const hexA = (hex, a) => {
    const h = String(hex || '#000').replace('#', '');
    const n = h.length === 3 ? h.split('').map((c) => c + c).join('') : h;
    return `rgba(${parseInt(n.slice(0, 2), 16) || 0}, ${parseInt(n.slice(2, 4), 16) || 0}, ${parseInt(n.slice(4, 6), 16) || 0}, ${Math.max(0, Math.min(1, Number(a) ?? 1))})`;
  };
  /**
   * A SHAPE'S PAINT, WHICH IS NOT ALWAYS A HEX.
   *
   * `hexA` parses two characters at a time and falls back to 0 per component, so
   * a gradient, a CSS var or a theme token that resolves to one came out BLACK at
   * the requested alpha — silently, and rendering perfectly. The layer most likely
   * to carry a gradient is a lower-third band, and that is the layer keyed over a
   * live camera.
   *
   * A hex still gets its alpha folded into the colour (so the shape can be
   * translucent without making its own children translucent). Anything else is
   * painted as written, with the alpha on the element instead.
   */
  const isHex = (v) => /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(String(v || '').trim());
  const shapePaint = (L) => {
    const a = L.opacity == null ? 1 : L.opacity;
    return isHex(L.fill)
      ? `background:${hexA(L.fill, a)};`
      : `background:${L.fill || 'transparent'}; opacity:${Math.max(0, Math.min(1, Number(a) ?? 1))};`;
  };
  // The box style for a positioned layer (percent geometry of the 16:9 stage).
  const boxStyle = (L) =>
    `left:${L.x}%; top:${L.y}%; width:${L.w}%; height:${L.h}%;`;
  // A background layer's paint (fill or image), and its dim scrim opacity.
  const bgPaint = (L) =>
    L.image ? `url("${L.image}") center / cover no-repeat` : L.fill || 'transparent';
  // Line transform (ProPresenter-style): reshape the text before it is laid out.
  function lineTransform(text, mode) {
    if (!text || !mode || mode === 'none') return text || '';
    switch (mode) {
      case 'remove-returns':
        return text.replace(/\s*\n\s*/g, ' ').trim();
      case 'replace-returns':
        return text.replace(/\s*\n\s*/g, '  •  ').trim();
      case 'one-word-per-line':
        return text.replace(/\s*\n\s*/g, ' ').trim().split(/\s+/).join('\n');
      case 'one-char-per-line':
        return text.replace(/\s+/g, '').split('').join('\n');
      default:
        return text;
    }
  }
  // The live text for a text/timer layer (with any line transform applied).
  function layerText(L) {
    let v;
    if (L.bind === 'countdown') v = countdownTo ? countdownText : '';
    else if (L.bind === 'clock') v = clockText;
    else if (L.bind === 'elapsed') v = elapsedText;
    else if (L.bind === 'remaining') v = remainingText;
    else v = boundValue(L, content);
    return lineTransform(v, L.lineTransform);
  }
  // vertical alignment → flex
  const vAlign = (v) => (v === 'top' ? 'flex-start' : v === 'bottom' ? 'flex-end' : 'center');
  /**
   * THE SIZE A TEXT LAYER ASKS FOR, in cqw — the one home for it.
   *
   * It is read twice and the two readers must not be able to disagree: the
   * markup DECLARES it (see `.lfit` below, and the comment there for why that is
   * load-bearing) and `fitLayers` measures FROM it. The `5` is the same fallback
   * the fitter used to keep to itself; a layer with no size is a layer nobody
   * designed, and a silent 0 would fit any box by vanishing.
   */
  const baseSize = (L) => (Number(L?.size) > 0 ? Number(L.size) : 5);
  $: refFirst =
    layout.refFirst || (layout.regions?.[0] === 'reference' && !layout.lowerThird);

  // Base type sizes (cqw). Real fit is measured, not guessed — see fitText().
  $: verseSize = style.verseSize;
  $: refSize = style.refSize;

  // Auto-fit: after every render (and on container resize), shrink the verse +
  // reference until the content box no longer overflows, so scripture is NEVER
  // clipped and NEVER spills off screen — at any output size. Font-size is set
  // imperatively on the element (not via a reactive var) so it can't re-enter
  // the Svelte scheduler and loop (CLAUDE.md rule #1). Idempotent: it resets to
  // the base size first, so it grows back when a shorter verse fires or the
  // container gets bigger.
  let stageEl;

  // ── THE FLOOR UNDER THE FIT ────────────────────────────────────────────────
  //
  // Shrinking until it stops overflowing always "succeeds" — there is no verse so
  // long that 40 rounds of ×0.95 cannot squeeze it in, and the result is a wall of
  // 2cqw text nobody past the third row can read. The loop was a fit algorithm
  // with no notion of failure, so a template that had stopped working looked
  // exactly like one that was working.
  //
  // It still shrinks, and it still shows the verse: blanking the screen would be
  // strictly worse for the congregation, and refusing to render is not this
  // component's call to make (`pipeline::preflight` owns refusals). What changes
  // is that when it goes below a legible size it SAYS SO, once, to whoever is
  // rendering it. The console and the Templates editor can then tell the operator
  // while there is still time to pick a different look.
  //
  // 45% of the template's own chosen size is the line. Not an absolute point size,
  // because the unit here is cqw — a share of the output's width — and a template
  // designed at 6cqw is making a different claim from one designed at 3cqw. The
  // question is "did we have to shrink this beyond what the designer intended",
  // and that is a ratio.
  const MIN_LEGIBLE_SCALE = 0.45;
  /** Called with `{ scale, legible }` when a fit has been forced below the floor. */
  export let onFit = null;

  function fitOne(box, container) {
    const verse = box.querySelector('.verse');
    const ref = box.querySelector('.reference');
    // The countdown renders at 2× the verse size — fit from THAT base, not the
    // plain verse size, or it would be shrunk to half on every tick.
    const vBase = verse && verse.classList.contains('countdown') ? verseSize * 2 : verseSize;
    const apply = (k) => {
      if (verse) verse.style.fontSize = `${vBase * k}cqw`;
      if (ref) ref.style.fontSize = `${refSize * k}cqw`;
    };
    // BOTH DIMENSIONS. It only ever checked height, which is fine for a verse —
    // prose wraps, so too much text gets taller. A COUNTDOWN does not wrap: it
    // is one wide line of tabular digits, so `2:00` at 12cqw overflows sideways
    // and runs off the edges of the screen at its own natural height, and the
    // loop never noticed. Same for a long unbroken word.
    const overflows = () =>
      box.scrollHeight > box.clientHeight + 1 || box.scrollWidth > box.clientWidth + 1;

    // WHERE THE LOOP IS LIKELY TO LAND. Each measured step forces a synchronous
    // reflow, so starting at 1 and shrinking costs one layout per 5% for a long
    // passage — on the page that is on the wall. The estimate uses the same 0.95
    // curve and the face's own advance, so it is a seed rather than an answer:
    // the measurement below still decides.
    //
    // THE BOX IS DESCRIBED IN THE UNITS THE SIZES ARE IN. `vBase` is cqw — a
    // share of the CONTAINER's width (`.stage`, which carries `container-type`),
    // not of this box. `.slide` pads by 6%/7% and `.content` caps at 90%/92% of
    // that, so the box is roughly three quarters of the container and the two
    // are never the same rectangle. Handing `fitScale` the box's own aspect with
    // the shares left at 100 described a container the size of the box, which
    // over-stated the room by that ratio and made the seed uniformly optimistic.
    // The container's aspect plus the box's real share of it in each dimension is
    // the same rectangle expressed in the same units as the size.
    let scale = 1;
    if (!(verse && verse.classList.contains('countdown'))) {
      const w = box.clientWidth || 0;
      const h = box.clientHeight || 0;
      const cw = container?.clientWidth || 0;
      const ch = container?.clientHeight || 0;
      if (w > 0 && h > 0 && cw > 0 && ch > 0) {
        scale = fitScale({
          text: verse ? verse.textContent || '' : '',
          size: vBase,
          face: faceOf(verseFontFamily),
          aspect: cw / ch,
          widthPct: (100 * w) / cw,
          heightPct: (100 * h) / ch,
          lineHeight: verseLineHeight,
        });
      }
    }
    apply(scale);

    // BOUNDED BY A SIZE, NOT BY A COUNT. This was `guard < 40`, and 0.95^40 is
    // 0.1285 — so a box needing less than that got the loop's last guess and kept
    // it, still overflowing, inside `overflow: hidden`. That is rule 42's sliced
    // verse arriving by a different road, and it does not take a long passage:
    // one short line at a large designed size in a shallow box needs a scale
    // below the old floor. `keepShrinking` stops on the answer instead
    // (templatemodel.js), so the curve is unchanged and only the cases that never
    // fitted move.
    let guard = 0;
    while (keepShrinking({ overflowing: overflows(), scale })) {
      scale *= FIT_STEP;
      apply(scale);
    }
    // The estimate can be pessimistic — a verse of short words wraps sooner in
    // arithmetic than it does in a real line-breaker. Grow back while it still
    // genuinely fits, so a seeded fit lands exactly where the plain loop would
    // have. Never above 1: the template's own size is the ceiling.
    while (scale < 1 && guard < 40) {
      const bigger = Math.min(1, scale / FIT_STEP);
      apply(bigger);
      if (overflows()) {
        apply(scale);
        break;
      }
      scale = bigger;
      guard++;
    }
    return scale;
  }
  function fitText() {
    if (!stageEl) return;
    // During a crossfade the outgoing and incoming slides coexist — fit both so
    // whichever is on top is already sized correctly.
    let worst = 1;
    // `stageEl` IS the container `cqw` resolves against (`container-type: size`),
    // so it is what the box's share is measured against.
    stageEl.querySelectorAll('.slide .content').forEach((box) => {
      worst = Math.min(worst, fitOne(box, stageEl));
    });
    // Report the WORST of the slides on screen, and never throw: this runs inside
    // a requestAnimationFrame on the page that is on the wall, and a listener that
    // breaks must not take the render with it.
    if (onFit) {
      try {
        onFit({ scale: worst, legible: worst >= MIN_LEGIBLE_SCALE });
      } catch {
        /* a report about legibility may not cost legibility */
      }
    }
  }

  // ── Fit scheduling (perf) ──────────────────────────────────────────────────
  // The fit loops force synchronous reflow (they read scrollHeight/clientHeight
  // in a shrink / binary-search loop), so they are EXPENSIVE and must run as
  // rarely as correctness allows. Two rules keep them off the hot path:
  //   1. Run at most once per animation frame — coalesce the many afterUpdate
  //      calls a single change fans out into one layout pass.
  //   2. Re-fit only when something that changes the layout changed: a new
  //      template or new content (a PROP change), or a container resize. A
  //      COUNTDOWN / CLOCK tick mutates internal state only (`now` / `clockNow`),
  //      never a prop — and the digits shrink or hold width as they count down,
  //      so the size found on the first fit stays valid. Ticks therefore must NOT
  //      re-fit. This is what kills the 4 Hz reflow storm the countdown used to
  //      drive on every mounted output at once (the app-feels-slow regression).
  //
  // IMPORTANT: this touches font-size ONLY. Content rendering (the slide, the
  // verse/reference text) is plain Svelte reactivity and is deliberately NOT
  // gated by any of this — the fitter must never be able to stall what's on the
  // wall. So there is no reactive `$:` block here that could interfere with the
  // component's update graph; afterUpdate simply schedules a frame, and the frame
  // decides — by a cheap signature — whether the expensive reflow actually runs.
  let fitRaf = 0;
  let lastFitSig = '';
  // Only fit when the render is ON SCREEN (set by an IntersectionObserver below).
  // Without IntersectionObserver (tests/jsdom) default to true so fitting still
  // runs. This is what stops a chapter grid — a dozen TemplateRenders mounting at
  // once — from running a dozen forced-reflow fit loops on load ("the Bible takes
  // seconds"): offscreen cards defer their fit until scrolled into view.
  let visible = typeof IntersectionObserver === 'undefined';
  // Everything that changes the FIT: container size, the configured sizes, and
  // the text being laid out. A countdown/clock tick does not change it, so the
  // reflow is skipped for ticks; a new verse changes it, so it always re-fits.
  function fitSig() {
    if (!stageEl) return '';
    const w = stageEl.clientWidth | 0;
    const h = stageEl.clientHeight | 0;
    if (layered) {
      let s = `L${w}x${h}`;
      for (const L of layers) {
        if (L.visible === false) continue;
        if (L.type === 'text' || L.type === 'timer') {
          s += `|${L.w},${L.h},${L.size},${(layerText(L) || '').length}`;
        }
      }
      return s;
    }
    return `R${w}x${h}|${verseSize}|${refSize}|${content?.reference ?? ''}|${(content?.text ?? '').length}|${bandMode ? 1 : 0}|${countdownTo ? 1 : 0}`;
  }
  function runFit() {
    fitRaf = 0;
    if (!stageEl || !visible) return; // don't reflow an offscreen render
    const sig = fitSig();
    if (sig === lastFitSig) return;
    lastFitSig = sig;
    if (layered) fitLayers();
    else fitText();
    verifyFit(sig);
  }

  // ── THE FIT HAS TO CHECK ITSELF ────────────────────────────────────────────
  //
  // The mount-time `document.fonts.status !== 'loaded'` guard below is correct
  // about the case it was written for and blind to the one that matters most: an
  // OUTPUT PAGE OPENS WITH NO TEXT ON IT. A webfont is only fetched when something
  // first uses it, so at mount there is nothing pending, `status` is already
  // `loaded`, and the deferred re-fit is skipped — permanently. Then the first
  // verse of the service arrives, the binary search measures it in the FALLBACK
  // face, converges, the real serif lands a moment later and the text grows past
  // the box it was fitted to. Nothing re-fits, because the signature has not
  // changed.
  //
  // Measured on a fresh `output.html` at 1920x1080 with the default Classic Serif
  // template: Romans 8:28 settled at 110.9px and painted a 732px block inside a
  // 583px box with `overflow: hidden`. The congregation reads a verse with its
  // first and last lines cut through the middle, and it stays that way until
  // something happens to resize the window.
  //
  // So the fit now looks at what it produced. If the box still overflows, the
  // metrics it measured are not the metrics being painted: wait for the fonts to
  // settle and fit exactly once more. A verse that genuinely cannot fit — the case
  // rule 37 is about — retries once, finds the same answer, and is reported
  // through `onFit` as before, so this cannot loop.
  let refitSig = '';
  let refitTries = 0;
  const MAX_REFIT = 2;
  function fitBoxes() {
    if (!stageEl) return [];
    return [
      ...(layered
        ? stageEl.querySelectorAll('.ltext')
        : stageEl.querySelectorAll('.slide .content')),
    ];
  }
  function overflowing() {
    return fitBoxes().some(
      (b) => b.scrollHeight > b.clientHeight + 1 || b.scrollWidth > b.clientWidth + 1
    );
  }
  /**
   * Did the fit that just ran measure the face that will actually be painted?
   *
   * A webfont is only fetched when something first USES it, so on an output page —
   * which opens with nothing on it — the request starts on the same frame as the
   * first verse. The binary search then measures the fallback (`serif`), finds a
   * size that fits in fewer lines, and the real face lands a moment later, taller.
   * Asking `overflowing()` straight after the fit does not catch it: the reflow the
   * new face causes has not happened yet. `fonts.check` is a fact available NOW.
   */
  function fittedWithTheRealFont() {
    if (typeof document === 'undefined' || !document.fonts || !document.fonts.check) return true;
    const box = fitBoxes()[0];
    if (!box) return true;
    const el = box.querySelector('.lfit') || box;
    try {
      return document.fonts.check(`1em ${getComputedStyle(el).fontFamily}`);
    } catch {
      return true; // an unparseable family is not a reason to keep re-fitting
    }
  }
  function verifyFit(sig) {
    if (sig !== refitSig) {
      refitSig = sig;
      refitTries = 0;
    }
    const stale = !fittedWithTheRealFont();
    if (!needsRefit({ fontReady: !stale, overflowing: overflowing(), tries: refitTries, max: MAX_REFIT }))
      return;
    refitTries += 1;
    const again = () => {
      lastFitSig = '';
      scheduleFit();
    };
    const fonts = typeof document !== 'undefined' ? document.fonts : null;
    const box = fitBoxes()[0];
    if (stale && fonts && fonts.load && box) {
      const family = getComputedStyle(box.querySelector('.lfit') || box).fontFamily;
      Promise.resolve(fonts.load(`1em ${family}`))
        .catch(() => {})
        .then(() => setTimeout(again, 32));
    } else {
      setTimeout(again, 60);
    }
  }
  function scheduleFit() {
    if (fitRaf) return;
    fitRaf =
      typeof requestAnimationFrame !== 'undefined' ? requestAnimationFrame(runFit) : setTimeout(runFit, 16);
  }
  afterUpdate(scheduleFit);
  let ro;
  let io;
  onMount(() => {
    if (typeof ResizeObserver !== 'undefined' && stageEl) {
      // A resize genuinely changes the fit — always re-fit (coalesced to a frame).
      ro = new ResizeObserver(scheduleFit);
      ro.observe(stageEl);
    }
    // Defer the (reflow-heavy) fit until this render is actually on screen. The
    // output window is always visible, so it fits at once; a grid's offscreen
    // cards fit lazily on scroll instead of all-at-once on mount.
    if (typeof IntersectionObserver !== 'undefined' && stageEl) {
      io = new IntersectionObserver((entries) => {
        const nowVisible = entries.some((e) => e.isIntersecting);
        if (nowVisible && !visible) {
          visible = true;
          lastFitSig = ''; // force a fresh fit now that we can measure it
          scheduleFit();
        } else {
          visible = nowVisible;
        }
      });
      io.observe(stageEl);
    } else {
      visible = true;
    }
    // WEB FONTS LOAD ASYNC. The FIRST fire often measures + fits with a fallback
    // font, then the real webfont arrives, the text grows, and it overflows/clips
    // — the "first verse doesn't fit" bug. When the fonts settle, force a re-fit.
    //
    // GUARD on `status !== 'loaded'`: once the fonts are in (which they are for
    // every mount after the first view), `fonts.ready` is already resolved and its
    // `.then` fires immediately — so WITHOUT this guard every TemplateRender adds a
    // redundant second fit on mount, and a Library/Scripture grid of a dozen verse
    // thumbnails becomes a reflow storm (the "slow on Scriptures/Library" regression).
    // Only the genuinely-still-loading first view needs the deferred re-fit.
    if (
      typeof document !== 'undefined' &&
      document.fonts &&
      document.fonts.ready &&
      document.fonts.status !== 'loaded'
    ) {
      document.fonts.ready.then(() => {
        lastFitSig = '';
        scheduleFit();
      });
    }
  });
  onDestroy(() => {
    ro?.disconnect();
    io?.disconnect();
    if (fitRaf && typeof cancelAnimationFrame !== 'undefined') cancelAnimationFrame(fitRaf);
  });

  // --- Video sound: the operator's chosen speaker, applied to the clip ---
  // The <video> lives inside a {#key} block, so a new clip is a NEW element:
  // routing is (re)applied on each element's loadedmetadata, not once on mount.
  // Layer mode, full-frame media and the legacy band are mutually exclusive
  // branches, so at most one <video> is ever mounted — one binding covers all.
  let videoEl;
  let sink = getAudioOutput();
  let unsubSink;
  onMount(() => {
    unsubSink = onAudioOutputChange((id) => {
      sink = id;
      routeAudio();
    });
  });
  onDestroy(() => unsubSink?.());

  // Apply the speaker choice, then play WITH sound. If the webview refuses
  // unmuted autoplay (no user gesture in this window yet), fall back to muted
  // playback rather than letting the clip not play at all — the picture is the
  // primary job in front of a congregation; sound is the bonus. Never let an
  // audio problem become a blank screen.
  async function routeAudio() {
    const el = videoEl;
    if (!el || !audio) return;
    await applySink(el, sink);
    try {
      el.muted = false;
      await el.play();
    } catch {
      el.muted = true;
      try {
        await el.play();
      } catch {
        /* autoplay blocked entirely; the element keeps its own autoplay attempt */
      }
    }
  }

  // Verse colour, with a readable default.
  $: verseColor = style.verseColor || '#f4e4c8';
  // Reference colour: an EXPLICIT `style.refColor` wins (the editor's per-region
  // "text colour"); otherwise the old behaviour — the accent, or the verse colour
  // on a band where the accent is the band fill.
  $: refColor =
    style.refColor || (layout.lowerThird ? style.verseColor || '#1c1224' : style.accent || 'var(--v-amber)');

  // ── Type styling, all template-configurable (editor "Design" controls) ─────
  // Every property below is PER REGION: the verse and the reference each carry
  // their own font, shadow, transform and spacing, so editing one never changes
  // the other. Where a per-region font/shadow is unset it falls back to the
  // template-level default (`style.font` / `style.textShadow`) for back-compat.
  const clamp01 = (v) => Math.max(0, Math.min(1, Number(v)));
  const shadowCssOf = (k) =>
    k > 0
      ? `0 ${(0.1 * k).toFixed(3)}em ${(0.35 * k).toFixed(3)}em rgba(0,0,0,${(0.9 * k).toFixed(2)})`
      : 'none';
  // A family with a fallback: a CSS var carries its own; a bare name gets a
  // generic appended so an uninstalled font degrades to the computer default.
  const fontFam = (f) => {
    const v = f || 'var(--f-serif)';
    return v.startsWith('var(') ? v : `${v}, system-ui, sans-serif`;
  };

  $: bgOpacity = clamp01(style.bgOpacity);
  // DIM SCRIM — a black overlay over the background (behind the text) to knock
  // down a bright image/background so text stays readable. 0 = none.
  $: bgDim = clamp01(style.bgDim);

  // TEXT CONTRAST PANEL (a "shape" behind the words). On a bright background a
  // coloured plate behind the text is what keeps it legible. Colour + opacity +
  // corner radius are all tweakable. Skipped on a lower-third band, which already
  // provides its own contrast bar.
  const hexToRgba = (hex, a) => {
    const h = (hex || '#000000').replace('#', '');
    const n = h.length === 3 ? h.split('').map((c) => c + c).join('') : h;
    const r = parseInt(n.slice(0, 2), 16) || 0;
    const g = parseInt(n.slice(2, 4), 16) || 0;
    const b = parseInt(n.slice(4, 6), 16) || 0;
    return `rgba(${r}, ${g}, ${b}, ${clamp01(a)})`;
  };
  $: panelOn = !!style.textPanel && !layout.lowerThird;
  $: panelBg = panelOn
    ? hexToRgba(style.panelColor || '#000000', style.panelOpacity == null ? 0.45 : style.panelOpacity)
    : 'transparent';
  $: panelRadius = style.panelRadius;

  // Heights. `bandHeight` (cqh) sizes the lower-third bar; `bgHeight` (%) lets the
  // background cover less than the full frame (anchored to the bottom, e.g. a
  // gradient that only fills the lower part of the screen). Both are optional.
  $: bandHeight = Number(style.bandHeight) > 0 ? Number(style.bandHeight) : null;
  $: bgHeight = style.bgHeight == null || style.bgHeight === '' ? 100 : Number(style.bgHeight);

  $: verseTransform = style.verseTransform; // capitalization; resolved in the model
  $: refTransform = style.refTransform;
  $: verseLineHeight = style.verseLineHeight > 0 ? style.verseLineHeight : 1.32;
  $: verseLetter = style.verseLetterSpacing ? `${Number(style.verseLetterSpacing)}em` : 'normal';
  $: refLetter = style.refLetterSpacing ? `${Number(style.refLetterSpacing)}em` : 'normal';
  // Gap between the verse and its reference (cqw, so it scales with the output).
  $: refGap = style.refGap;
  // Per-element font and shadow. There is no whole-template fallback any more:
  // `migrateStyle` writes the old `style.font` / `style.textShadow` onto both
  // elements and deletes them, so reading them here would be reading a key that
  // no longer exists — and a fallback chain is a second home wearing a helpful
  // name (docs/REBRAND.md §3.1).
  $: verseFontFamily = fontFam(style.verseFont);
  $: refFontFamily = fontFam(style.refFont);
  $: verseShadowCss = shadowCssOf(clamp01(style.verseShadow));
  $: refShadowCss = shadowCssOf(clamp01(style.refShadow));
  // Announcement/ticker scroll: renders as a bottom FOOTER band (a ProPresenter
  // ticker), not centred text. Off unless the template asks for it.
  $: scroll = !!style.scroll;

  // Assembled inline styles for the verse and reference (font-size is applied
  // separately in markup and then overridden by the fitter).
  $: verseStyle = `color:${verseColor}; text-align:${verseAlign}; text-transform:${verseTransform}; line-height:${verseLineHeight}; letter-spacing:${verseLetter}; text-shadow:${verseShadowCss}; font-family:${verseFontFamily};`;
  $: refStyle = `color:${refColor}; text-align:${refAlign}; text-transform:${refTransform}; letter-spacing:${refLetter}; text-shadow:${refShadowCss}; font-family:${refFontFamily};`;

  // Ticker footer duration: constant reading speed, so a long notice scrolls no
  // faster than a short one. ~7 characters/second, clamped to something sane.
  $: tickerSecs = Math.min(60, Math.max(10, (content?.text?.length || 0) * 0.42));
  // The footer BAR colour. The template's own background (a solid or gradient)
  // makes a cohesive bar; failing that the accent, then a dark scrim. Never
  // transparent — the bar IS the visible ticker, even on a keyed lower-third
  // channel where it becomes the crawl composited over the camera.
  $: tickerBg =
    style.tickerBg ||
    (style.background && style.background !== 'transparent'
      ? style.background
      : style.accent || 'rgba(0,0,0,0.82)');

  $: show = (r) => layout.regions?.includes(r);

  // The lower-third band is a TEMPLATE choice (configured in the editor), not a
  // content choice — like ProPresenter's "Lower 3rd Lyrics" / "Lower 3rd
  // Scripture" templates. So lyrics on a lower-third template render IN the band
  // (bottom, centered by the template's alignment), never floating mid-screen.
  $: hasRef = !!content?.reference;
  $: bandMode = !!layout.lowerThird;
  // THE BAND ONLY EXISTS WHERE THERE ARE WORDS. Drawn unconditionally it painted
  // a coloured strip across the bottom of a full-frame photo that had nothing
  // written in it — a bar over someone's picture for no reason.
  $: bandHasWords = !!(content?.text || (hasRef && !bandMode));
  // A COUNTDOWN NEVER REACHES A LOWER THIRD. The band is keyed over a live
  // camera during the service; a clock ticking across it belongs on the lobby
  // screen and the main screen, not over the preacher.
  $: countdownAllowed = !!countdownTo && !bandMode;

  // Background can be a color/gradient (style.background) OR an uploaded image
  // (style.bgImage, a data URL) rendered cover. An image wins when present. The
  // background lives on the stage, so it persists while slides crossfade.
  // ── THE TRANSPARENCY LAW ─────────────────────────────────────────────────
  //
  // A LOWER-THIRD CHANNEL IS TRANSPARENT AT ALL TIMES. That is what it is FOR:
  // it is keyed over a live camera in OBS or an ATEM, and the moment it paints
  // a background the congregation — and the stream — lose the preacher and get
  // a coloured rectangle instead. So a lower-third template's own background is
  // ignored, not merely defaulted: an operator picking a background in the
  // Templates editor must not be able to black out a stream by accident.
  //
  // MEDIA IS THE ONE EXCEPTION. When a picture or a video is fired the operator
  // has deliberately chosen a full-frame image; it becomes the background and
  // transparency yields to it.
  $: bg = bandMode
    ? 'transparent'
    : style.bgImage
      ? `url("${style.bgImage}") center / cover no-repeat`
      // `slideBG` returns null when the template names no background, which is
      // what keeps an unset template transparent rather than black.
      : slideBG(style) || 'transparent';

  // Alignment is configured per template (defaults centre). Lyrics inherit it —
  // the default lower-third template is centred, matching ProPresenter.
  $: verseAlign = style.verseAlign || layout.align || 'center';
  $: refAlign = style.refAlign || layout.align || 'center';

  // Font family, with a fallback so an UNINSTALLED named font degrades to the
  // computer's default rather than something arbitrary. A CSS var already carries
  // its own generic; a bare family name ("Didot") does not, so append one.
  $: fontFamily = (() => {
    const f = style.verseFont || 'var(--f-serif)';
    if (f.startsWith('var(')) return f; // the var supplies its own fallback
    return `${f}, system-ui, sans-serif`;
  })();

  // THE SLIDE TRANSITION (docs/REBRAND.md §8). A CUT unless the template or its
  // theme asks for something else, because that is what an operator asked for
  // ("quick as light, remove every animation") and what a wall should do when
  // nobody has said otherwise.
  //
  // Transitions were removed from this renderer once, for a real reason: a
  // crossfade made the auto-fit measure a slide that still carried a transform.
  // `transitions.js` animates ONLY opacity, transform and filter, and none of the
  // three moves `scrollHeight` or `clientHeight` — so the fitter measures the same
  // box whether or not a transition is running. A mode that animated width,
  // padding or font-size would bring the old bug straight back.
  //
  // Reduced motion is a CUT, not a faster animation: the viewer asked for none.
  //
  // TWO AUTHORITIES, ONE RANKING (DECISIONS §84). The operator's live override
  // outranks the template; `resolveTransition` is the only place that is decided,
  // so the console preview and the wall cannot disagree about it.
  //
  // The override is read from the store by DEFAULT, which is what gives every
  // console surface the picker with no per-surface wiring — the same arrangement
  // themes use. `Output.svelte` passes the prop explicitly instead, because a
  // congregation screen must apply an override only when CONTENT arrives: see the
  // snapshot comment there.
  export let transitionOverride = undefined;
  $: activeOverride = transitionOverride === undefined ? $liveTransition : transitionOverride;
  $: resolvedTransition = resolveTransition(style, activeOverride);
  $: transitionMode = resolvedTransition.mode;
  $: transitionMs = transitionDuration(resolvedTransition.mode, resolvedTransition.ms, reduceMotion);
  const reduceMotion =
    typeof window !== 'undefined' && typeof window.matchMedia === 'function'
      ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
      : false;
  /** Svelte's transition contract, driven by the one pure function. */
  function slideIn(node, { mode, duration }) {
    return { duration, css: (t) => transitionCss(mode, t) };
  }

  // Countdown: tick a local clock only while a target is set. The number updates
  // in place via its own reactive (`now`), which slideKey excludes — so ticks
  // never re-key the slide (no per-second crossfade). setInterval (not Svelte's
  // tick()) keeps this clear of the reactive-loop freeze (CLAUDE.md rule #1).
  $: countdownTo = content?.countdown_to ?? null;
  // A HELD countdown is not counting, so nothing here ticks for it — the figure is
  // whatever it was held at. The interval is stopped as well as ignored: a timer
  // firing four times a second to recompute a number that cannot change is the
  // cheapest thing on this page and still the wrong thing on an output machine that
  // is also decoding speech.
  $: countdownHeld = countdownIsPaused(content);
  let now = 0;
  let cdTimer = null;
  $: if (countdownTo && !countdownHeld) startClock();
  else stopClock();
  function startClock() {
    if (cdTimer) return;
    now = typeof Date !== 'undefined' ? Date.now() : 0;
    cdTimer = setInterval(() => (now = Date.now()), 250);
  }
  function stopClock() {
    if (cdTimer) {
      clearInterval(cdTimer);
      cdTimer = null;
    }
  }
  onDestroy(stopClock);
  // ONE READER (docs/REBRAND.md §7). This used to be its own subtraction, as did the
  // stage page and the console — survivable while the answer was one subtraction, and
  // not survivable now that it has an exception: a copy that has never heard of
  // `countdown_paused_ms` goes on counting down while the other two hold, and this
  // copy is the congregation's.
  $: remainingMs = countdownRemainingMs(content, now);
  // Only ever true when it genuinely ran out. A countdown held at 0:00 cannot exist
  // (`adjust_countdown` refuses a target under a second), but saying so here keeps
  // the done message off a screen that is merely paused.
  $: countdownDone = remainingMs === 0 && !countdownHeld;
  // ONE FORMATTER (docs/REBRAND.md §7). This used to be its own copy of the
  // arithmetic, as did the stage page — and both stopped at minutes, so a
  // 90-minute pre-service countdown read `90:00`.
  $: countdownText = remainingMs == null ? '' : formatCountdown(remainingMs);
  // The last minute, or the last tenth of a short countdown. The span comes from
  // `countdown_from`, which `start_countdown` now WRITES — for as long as this
  // rule has existed, that field was read here and written nowhere, so the
  // short-countdown half of it had never once fired in the product. Without a span
  // the rule still falls back to the last minute, which is the honest answer for a
  // countdown whose length nobody told us.
  // The warning colour is applied INLINE as well as by class: the countdown's own
  // colour is an inline style, and an inline style beats a stylesheet rule, so a
  // `.warn` class alone would have changed nothing on the wall.
  const CD_WARN = '#f4515b';
  $: countdownWarn = remainingMs != null && countdownWarning(remainingMs, countdownTotalMs(content));

  // Re-key on the actual content so a new slide crossfades but identical content
  // (a re-broadcast of the same verse) does not re-animate. Countdown ticks are
  // deliberately excluded — only a NEW countdown target re-keys.
  //
  // THE OVERRIDE IS PART OF THE KEY, and the template's own transition is NOT.
  // "Choosing one replays it on the programme at once" (docs/REBRAND.md §8) is the
  // half of this control that stops it reading as dead — the prototype repaints its
  // program frame on every pick for exactly that reason. Keying on the override
  // gives that for free on any surface that follows the store.
  //
  // The RESOLVED mode is deliberately not in the key: a live template edit pushes a
  // new `template` frame to every screen, and keying on it would make every such
  // edit re-animate a verse that is already up on the wall.
  $: overrideKey = isOverride(activeOverride) ? `${activeOverride.mode}|${activeOverride.ms ?? ''}` : '';
  $: slideKey = `${content?.reference ?? ''}|${content?.text ?? ''}|${content?.media_url ?? ''}|${countdownTo ?? ''}|${overrideKey}`;

  // A wall clock for clock-bound layers — ticks once a second only when needed.
  let clockNow = 0;
  let clockTimer = null;
  $: needClock =
    layered &&
    layers.some((L) => L.bind === 'clock' || L.bind === 'elapsed' || L.bind === 'remaining');
  // Elapsed service timer: counts UP from the service-start epoch carried on the
  // fired content, ticked by the same per-second clock. Empty when no service is
  // recording (no start epoch). Like the wall clock it lives on internal state
  // (clockNow), so a tick never re-keys the slide.
  $: serviceStartedAt = content?.service_started_at ?? null;
  $: serviceTargetMs = content?.service_target_ms ?? null;
  $: nowTick = clockNow || (typeof Date !== 'undefined' ? Date.now() : 0);
  $: elapsedText = serviceStartedAt != null ? formatElapsed(nowTick - serviceStartedAt) : '';
  // Remaining = planned length − elapsed. Needs BOTH a start and a target; goes
  // negative (shown as -M:SS) once the service runs over.
  $: remainingText =
    serviceStartedAt != null && serviceTargetMs != null
      ? formatRemaining(serviceTargetMs - (nowTick - serviceStartedAt))
      : '';
  $: if (needClock) startClockTick();
  else stopClockTick();
  function startClockTick() {
    if (clockTimer) return;
    clockNow = Date.now();
    clockTimer = setInterval(() => (clockNow = Date.now()), 1000);
  }
  function stopClockTick() {
    if (clockTimer) {
      clearInterval(clockTimer);
      clockTimer = null;
    }
  }
  onDestroy(stopClockTick);
  $: clockText = (() => {
    const d = new Date(clockNow || Date.now());
    const h = d.getHours();
    return `${((h + 11) % 12) + 1}:${String(d.getMinutes()).padStart(2, '0')} ${h < 12 ? 'AM' : 'PM'}`;
  })();

  // The live text for each layer, computed REACTIVELY. This is load-bearing:
  // `layerText(L)` reads `content` (and the ticking countdown/clock) INSIDE the
  // function, which Svelte's template dependency analysis cannot see — so calling
  // `layerText(L)` directly in the markup froze every bound layer on its FIRST
  // value. That was the "Program screen stuck on the first fired verse" bug: the
  // store moved on, but a layered template's text never re-evaluated. Referencing
  // content/countdownText/clockText here makes Svelte re-run this whenever any of
  // them change, so the markup reads already-current text.
  $: layerViews = (() => {
    void content;
    void countdownText;
    void clockText;
    void elapsedText;
    void remainingText;
    // A BAND DECIDES WHERE ITS WORDS GO; it does not draw them (docs/REBRAND.md
    // §4, `bandLayout`). Members are emitted into this same list with a derived
    // box, so every text layer on a wall — inside a band or not — goes through
    // ONE path below: one fit, one shadow rule, one transform, one `{#key}`.
    // They are skipped where they sit in the stack, because their band draws
    // them in the order it names them.
    const boxes = drawBoxes(layers, layerText);
    const out = [];
    for (const L of topLevelLayers(layers)) {
      out.push({ L, text: layerText(L), box: boxes.get(L.id) });
      if (L.type !== 'band') continue;
      // The band's words, in the order the band names them, each at the box the
      // band just computed for it — and then through the SAME text branch below
      // as every other text layer.
      for (const id of Array.isArray(L.members) ? L.members : []) {
        const m = layers.find((x) => x && x.id === id);
        if (m) out.push({ L: m, text: layerText(m), box: boxes.get(m.id) });
      }
    }
    return out;
  })();

  // Per-text-layer auto-fit. Each layer's text is sized to BEST FIT its own box —
  // it scales DOWN when there is a lot of text and UP when there is little, and it
  // is NEVER allowed to push outside the box (both dimensions are checked). A
  // binary search finds the largest font that fits, so short text fills the box
  // and long text wraps and shrinks — the ProPresenter "scale text up or down"
  // behaviour. `fit` modes: 'both' (default, up+down), 'shrink' (cap at the set
  // size, only shrink), 'none' (use the set size verbatim).
  //
  // AND IT REPORTS, like the region fit always has (rule 37). `fitText` called
  // `onFit` and this did not — so once `TemplateGallery.upgradeLegacyToLayers`
  // converted the shelf, which it does on mount, rule 37's instrument covered
  // nothing a church actually renders. Live passes `onFit` to its programme pane
  // and its "may not be readable from the back" line simply could not fire for a
  // layered look: the same sentence over a template that was working and one that
  // had stopped, which is rule 35.
  function fitLayers() {
    if (!stageEl || !layered) return;
    // The WORST layer on the screen, on the same terms as `fitText`'s worst
    // slide: a ratio against the size the designer asked for, never above 1, so
    // "scale" always means how far this had to shrink and never how far a short
    // word was allowed to grow.
    let worst = 1;
    stageEl.querySelectorAll('.ltext').forEach((box) => {
      const el = box.querySelector('.lfit');
      if (!el) return;
      const base = parseFloat(el.dataset.base || '5') || 5;
      const mode = el.dataset.fit || 'both';
      if (el.classList.contains('lscroll') || mode === 'none') {
        el.style.fontSize = `${base}cqw`;
        return;
      }
      const fits = (px) => {
        el.style.fontSize = `${px}cqw`;
        return box.scrollHeight <= box.clientHeight + 1 && box.scrollWidth <= box.clientWidth + 1;
      };
      // 'shrink' caps growth at the configured size; 'both' allows growing to a
      // generous ceiling so a single short word fills the box.
      const hi = mode === 'shrink' ? base : Math.max(base, 22);
      let lo = 0.4;
      let top = hi;
      let best = lo;
      for (let i = 0; i < 16; i++) {
        const mid = (lo + top) / 2;
        if (fits(mid)) { best = mid; lo = mid; } else { top = mid; }
      }
      el.style.fontSize = `${best}cqw`;
      // A box with nothing in it was not shrunk, it is EMPTY — a reference layer
      // on a lyric fire, a `next` line with no next. Reporting its ratio would
      // make Live shout "38% of the designed size" on an ordinary song.
      if ((el.textContent || '').trim()) worst = Math.min(worst, best / base);
    });
    // Never throw: this runs inside a requestAnimationFrame on the page that is
    // on the wall, and a listener that breaks must not take the render with it.
    if (onFit) {
      try {
        onFit({ scale: worst, legible: worst >= MIN_LEGIBLE_SCALE });
      } catch {
        /* a report about legibility may not cost legibility */
      }
    }
  }
  // Fit is driven by the unified scheduler above (runFit → fitLayers/fitText),
  // gated to prop-change + resize so countdown/clock ticks don't force reflow.
</script>

<div class="stage" bind:this={stageEl} style="--accent:{style.accent || 'var(--v-amber)'};">
  <!-- NOTHING renders without content. "Clear all screens" (content → null) must
       remove EVERYTHING — the background, the lower-third band, every layer — not
       just the text. A background left painted after a clear, or a band left over
       a live camera after a blackout, is furniture on the congregation's wall (or
       the stream) with nothing to say. This persists across a content→content
       CROSSFADE (content stays non-null throughout); it only leaves on a real
       clear, or a blackout of a keyed channel. -->
  {#if content}
  {#if layered}
    <!-- ══ LAYER MODE ══ free-form stack, drawn back-to-front. Media shows ONLY
         where a template includes a MEDIA layer (below), at that layer's z-order —
         so each screen opts into (or out of) media and controls what sits over or
         under it. A lower third with no media layer never shows the picture; a
         full-screen template with a media layer on top lets the picture fill it. -->
    {#each layerViews as { L, text, box } (L.id)}
      {#if L.visible !== false}
        {#if L.type === 'background'}
          <div class="lbg" style="{boxStyle(L)} background:{bgPaint(L)}; opacity:{L.opacity == null ? 1 : L.opacity};"></div>
          {#if L.dim > 0}<div class="lbg ldim" style="{boxStyle(L)} opacity:{L.dim};"></div>{/if}
        {:else if L.type === 'media'}
          <!-- Paints only when a picture/video is on screen and this screen shows
               media; empty otherwise, so the layer is invisible on a text-only
               cue (or when the screen opts out of media). -->
          {#if content.media_url && allowMedia}
            <div class="lmediabox" style="{boxStyle(L)} border-radius:{L.radius || 0}cqw; opacity:{L.opacity == null ? 1 : L.opacity};">
              {#if content.media_kind === 'video'}
                <!-- svelte-ignore a11y-media-has-caption -->
                <video class="lmediafill" src={content.media_url} style="object-fit:{L.fit === 'contain' ? 'contain' : 'cover'};" bind:this={videoEl} autoplay loop muted={!audio} playsinline on:loadedmetadata={routeAudio}></video>
              {:else}
                <img class="lmediafill" src={content.media_url} style="object-fit:{L.fit === 'contain' ? 'contain' : 'cover'};" alt="" />
              {/if}
            </div>
          {/if}
        {:else if L.type === 'shape'}
          <div class="lshape" style="{boxStyle(L)} {shapePaint(L)} border-radius:{L.radius || 0}cqw;"></div>
        {:else if L.type === 'band'}
          <!-- THE BAND (docs/REBRAND.md §4): a real element running from its own
               `top` to the bottom edge, inset by the side safe area. Its words are
               NOT its children — they are emitted beside it with boxes this band
               computed, so they take the one text path below. Its alpha is applied
               exactly (`shapePaint`); it has never been scaled by 0.9 here. -->
          <div class="lband" style="{boxStyle(box || L)} {shapePaint(L)} border-radius:{L.radius || 0}cqw;"></div>
        {:else if L.type === 'region'}
          <!-- A REAL RENDERED SLIDE, inside its own container (docs/REBRAND.md §6).
               `container-type: inline-size` is the feature: cqw inside this box is
               a share of the BOX's width, so the template scales to the region
               exactly as it would to a screen of that width.

               Only at depth 0 — a composite may not be another composite's fill. -->
          {#if depth === 0}
            <div
              class="lregion"
              style="{boxStyle(L)} border-radius:{L.radius || 0}cqw; opacity:{L.opacity == null ? 1 : L.opacity}; {L.outline ? `outline:${L.outline}cqw solid ${L.outlineColor || 'var(--accent)'}; outline-offset:-${L.outline}cqw;` : ''} {L.plate ? `background:${L.plate};` : ''}">
              <svelte:self
                template={builtinById(L.templateRef)}
                {content}
                {theme}
                depth={depth + 1}
              />
            </div>
          {/if}
        {:else if !(showDefaultCountdown && (L.bind === 'verse' || L.bind === 'reference' || L.bind === 'translation'))}
          <!-- Verse/reference/translation layers are hidden during a default
               countdown (they carry no content then); a static or clock layer
               still shows. -->
          <div class="ltext" style="{boxStyle(box || L)} align-items:{vAlign(L.valign)};">
            {#key text}
              <!-- THE SIZE IS DECLARED, NOT ONLY FITTED (rule 37 · rule 42).
                   `font-size` used to be the ONE type property this element did
                   not emit — colour, family, weight, alignment, transform,
                   line-height, tracking, shadow and style were all here, and the
                   one that decides whether the words fit the box was set only by
                   `fitLayers`, imperatively, inside a requestAnimationFrame that
                   is deliberately deferred while a render is off screen. Until it
                   landed, a layered template painted in whatever `body` says —
                   `--v-fs-b1`, 12px of UI text — and 12px at line-height 1.32 is
                   15.8px for ONE line inside a band box that is 14.8px tall on a
                   gallery card. Four cards in the Templates gallery were rendered
                   with 39px of content inside a 15px `overflow:hidden` box: the
                   words sliced, on the surface an operator judges a look from.
                   The region branch below never had this, because it emits
                   `font-size:{verseSize}cqw` — two text paths, one of them
                   missing the base size, which is the shape `bandLayout`'s own
                   doc comment warns about.
                   Declaring it makes the un-fitted state the DESIGNED state,
                   which fits; `fitLayers` then overwrites this same inline
                   property to refine it, exactly as before. It also means every
                   moment that wipes the imperative value — a `{#key text}`
                   rebuild, a style attribute Svelte re-renders — lands on the
                   template's own size instead of on the app's. `cardfit.test.js`. -->
              <div
                class="lfit"
                class:lscroll={L.scroll}
                data-base={baseSize(L)}
                data-fit={L.fit || 'both'}
                style="font-size:{baseSize(L)}cqw; color:{L.color}; font-family:{fontFamOf(L.font)}; font-weight:{L.weight || 400}; text-align:{L.align}; text-transform:{L.transform || 'none'}; line-height:{L.lineHeight || 1.3}; letter-spacing:{(L.letterSpacing || 0)}em; text-shadow:{shadowOf(L.shadow)}; font-style:{L.italic ? 'italic' : 'normal'};">
                {#if L.scroll}
                  <span class="lrun" style="--tickdur:{Math.min(60, Math.max(10, (text?.length || 0) * 0.42))}s">{text}</span>
                {:else}
                  <!-- Verbatim: the text is shown exactly as imported/typed. Relay
                       does NOT wrap it in quotation marks — added quotes that were
                       never in the source are wrong (operator request). -->
                  {text}
                {/if}
              </div>
            {/key}
          </div>
        {/if}
      {/if}
    {/each}
    {#if showFullMedia}
      <!-- This screen shows media but has no media layer to place it → the picture
           fills the frame, drawn on TOP of the layers (foreground). Add a media
           layer to the template to position it instead. -->
      {#if content.media_kind === 'video'}
        <!-- svelte-ignore a11y-media-has-caption -->
        <video class="media" src={content.media_url} bind:this={videoEl} autoplay loop muted={!audio} playsinline on:loadedmetadata={routeAudio}></video>
      {:else}
        <img class="media" src={content.media_url} alt="" />
      {/if}
    {/if}
    {#if showDefaultCountdown}
      <!-- No timer layer, but a countdown is on screen → a default centred label +
           MM:SS over the template's background/shape layers. Add a Timer layer to
           the template to place it instead. -->
      <div class="cd-default">
        {#if content.reference && !countdownDone}
          <div class="reference" style="font-size:{refSize}cqw; {refStyle}">{content.reference}</div>
        {/if}
        <div class="verse countdown" class:warn={countdownWarn} style="font-size:{verseSize * 2}cqw; margin-top:{refGap}cqw; color:{countdownWarn ? CD_WARN : verseColor}; text-align:center; text-shadow:{verseShadowCss};">
          {countdownDone ? (content.countdown_done || '0:00') : countdownText}
        </div>
      </div>
    {/if}
  {:else if content.media_url && allowMedia}
    <!-- REGION (legacy, no layers): a fired picture/video fills the frame alone —
         these templates have no media layer to place it, so full-frame is the only
         sensible behaviour and keeps old templates working. -->
    {#if content.media_kind === 'video'}
      <!-- svelte-ignore a11y-media-has-caption -->
      <video class="media" src={content.media_url} bind:this={videoEl} autoplay loop muted={!audio} playsinline on:loadedmetadata={routeAudio}></video>
    {:else}
      <img class="media" src={content.media_url} alt="" />
    {/if}
  {:else}
  <!-- Background is its OWN layer so its opacity can be dimmed (for readability
       over an image) without touching the text. Band mode keeps it transparent
       — the transparency law. -->
  {#if bg !== 'transparent'}
    <div class="bglayer" style="background:{bg}; opacity:{bgOpacity}; height:{bgHeight}%; top:auto; bottom:0;"></div>
  {/if}
  <!-- Dim scrim: knocks down a bright background so text stays readable. -->
  {#if bgDim > 0}
    <div class="dimlayer" style="opacity:{bgDim};"></div>
  {/if}
  {#if content}
    {#key slideKey}
      <div
        class="slide"
        class:lower-third={bandMode}
        class:bandless={bandMode && !bandHasWords}
        in:slideIn={{ mode: transitionMode, duration: transitionMs }}>
        {#if scroll && show('verse_text') && content.text && !countdownTo}
          <!-- FOOTER TICKER (ProPresenter-style). A band pinned to the very
               bottom of the screen: an optional fixed label on the left, then the
               body scrolling right-to-left at a constant reading speed. This is
               the announcement crawl — it never occupies the centre of the wall.
               The label obeys the Reference toggle just like every other region:
               turning the reference OFF removes it from the ticker too. -->
          <div class="ticker" style="background:{tickerBg}; --tickdur:{tickerSecs}s;">
            {#if show('reference') && content.reference}
              <span class="ticker-label" style="font-size:{refSize}cqw; {refStyle}">{content.reference}</span>
            {/if}
            <div class="ticker-track">
              <span class="ticker-run" style="font-size:{verseSize}cqw; {verseStyle}">{content.text}</span>
            </div>
          </div>
        {:else}
          <div
            class="content"
            class:panel={panelOn}
            style="text-align:{layout.align || 'center'}; font-family:{fontFamily}; background:{panelBg}; border-radius:{panelRadius}cqw;{bandMode && bandHeight ? ` min-height:${bandHeight}cqh;` : ''}"
          >
            {#if countdownTo && !countdownAllowed}
              <!-- Deliberately nothing: a countdown does not go out on a lower
                   third. The other channels still show it. -->
            {:else if countdownTo}
              <!-- Countdown: label + live MM:SS, styled by the template. At zero
                   only the done message shows (the "begins in" label is dropped so
                   it never reads "Service begins in Welcome"). -->
              {#if content.reference && !countdownDone}
                <div class="reference" style="font-size:{refSize}cqw; {refStyle}">{content.reference}</div>
              {/if}
              <div class="verse countdown" class:warn={countdownWarn} style="font-size:{verseSize * 2}cqw; color:{countdownWarn ? CD_WARN : verseColor}; text-align:{verseAlign}; text-shadow:{verseShadowCss};">
                {countdownDone ? (content.countdown_done || '0:00') : countdownText}
              </div>
            {:else if refFirst}
              {#if show('reference') && content.reference}
                <div class="reference" style="font-size:{refSize}cqw; {refStyle}">{content.reference}</div>
              {/if}
              {#if show('verse_text') && content.text}
                <div class="verse" style="font-size:{verseSize}cqw; margin-top:{refGap}cqw; {verseStyle}">{content.text}</div>
              {/if}
            {:else}
              {#if show('verse_text') && content.text}
                <div class="verse" style="font-size:{verseSize}cqw; {verseStyle}">{content.text}</div>
              {/if}
              {#if show('reference') && content.reference}
                <div class="reference" style="font-size:{refSize}cqw; margin-top:{refGap}cqw; {refStyle} font-style:{style.italicRef ? 'italic' : 'normal'};">{content.reference}</div>
              {/if}
            {/if}
          </div>
        {/if}
      </div>
    {/key}
  {/if}
  {/if}
  {/if}
</div>

<style>
  .stage {
    position: absolute;
    inset: 0;
    container-type: size;
    overflow: hidden;
  }
  /* Background as its own layer, so template opacity dims the background image
     for readability without fading the text on top of it. */
  .bglayer {
    position: absolute;
    inset: 0;
    z-index: 0;
  }
  /* Dim scrim over the background (still behind the text). */
  .dimlayer {
    position: absolute;
    inset: 0;
    z-index: 0;
    background: #000;
    pointer-events: none;
  }

  /* ── Layer mode ── each layer is an absolutely-positioned box (percent
     geometry), drawn in DOM order (back-to-front). */
  .lbg,
  .lshape,
  .lband,
  .ltext,
  .lregion,
  .lmediabox {
    position: absolute;
    box-sizing: border-box;
  }
  /* THE REGION IS ITS OWN CONTAINER — the whole point of a composite. `cqw`
     inside this box is a share of the BOX's width, so the template rendered in
     it scales to the region exactly as it would to a screen of that width. */
  .lregion {
    overflow: hidden;
    container-type: inline-size;
  }
  /* A media layer: the picture/video fills the layer's box (cover/contain set
     inline per layer), clipped to its rounded corners. */
  .lmediabox {
    overflow: hidden;
  }
  .lmediafill {
    width: 100%;
    height: 100%;
    display: block;
  }
  .lbg {
    background-repeat: no-repeat;
  }
  .lbg.ldim {
    background: #000;
  }
  .ltext {
    display: flex;
    overflow: hidden;
  }
  .lfit {
    width: 100%;
    overflow-wrap: anywhere;
    white-space: pre-wrap;
  }
  /* Default countdown overlay for a layered template with no timer layer — the
     label + MM:SS, centred over the template's own background/shape layers. */
  .cd-default {
    position: absolute;
    inset: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 6% 7%;
    box-sizing: border-box;
  }
  /* A scrolling text layer runs on one line inside its (clipped) box. */
  .lfit.lscroll {
    white-space: nowrap;
    overflow: hidden;
  }
  .lscroll .lrun {
    display: inline-block;
    padding-left: 100%;
    animation: relay-ticker var(--tickdur, 18s) linear infinite;
    will-change: transform;
  }
  @media (prefers-reduced-motion: reduce) {
    .lscroll .lrun {
      animation: none;
      padding-left: 0;
    }
  }
  /* One slide layer. Absolute so an outgoing and incoming slide overlap during
     the crossfade instead of pushing each other around. */
  .slide {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 6% 7%;
    box-sizing: border-box;
  }
  /* Lower third: the content sits as a band pinned to the bottom, rest
     transparent so a camera / ATEM / OBS source shows through the top. */
  .slide.lower-third {
    align-items: flex-end;
    padding: 0 0 6% 0;
  }
  /* Full-bleed media layer behind the text (image/video background). */
  /* THE LAST MINUTE (docs/REBRAND.md §7). Red, and moving — a still colour
     change on a screen somebody glances at is easy to miss. Reduced motion gets
     the glow without the pulse: the information is the colour, the pulse only
     makes it findable. */
  .countdown.warn { color: #f4515b; }
  @media (prefers-reduced-motion: no-preference) {
    .countdown.warn { animation: cdwarn 2s ease-in-out infinite; }
  }
  @media (prefers-reduced-motion: reduce) {
    .countdown.warn { text-shadow: 0 0 0.25em rgba(244, 81, 91, 0.85); }
  }
  @keyframes cdwarn {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.55; }
  }

  .media {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .content {
    position: relative;
    max-width: 90%;
    max-height: 92%;
    overflow: hidden;
  }
  /* Text contrast panel — a plate behind the words for a bright background. The
     padding gives the plate room around the text; it collapses to nothing when
     the panel is off (background transparent, radius still set but invisible). */
  .content.panel {
    padding: 3.5cqw 4.5cqw;
    max-width: 82%;
    overflow: visible;
  }
  .slide.lower-third .content {
    max-width: 100%;
    width: 100%;
    background: var(--accent);
    padding: 2.4% 4%;
    box-sizing: border-box;
  }
  /* No words, no band. */
  .slide.lower-third.bandless .content {
    background: transparent;
    padding: 0;
  }
  .verse {
    line-height: 1.32;
    overflow-wrap: anywhere;
    word-break: break-word;
    white-space: pre-wrap;
  }
  /* Countdown: tabular figures + tight leading so the ticking digits don't
     shift the layout every second. */
  .countdown {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    line-height: 1.05;
    letter-spacing: 0.01em;
    /* The digits are one unbreakable line; keep them on one line so the fitter
       scales them down instead of letting them wrap mid-number. */
    white-space: nowrap;
  }
  .reference {
    font-weight: 600;
    overflow-wrap: anywhere;
  }
  /* Footer ticker — a band pinned to the very bottom of the screen (a
     ProPresenter-style announcement crawl). The optional label stays fixed on
     the left; the body scrolls right-to-left inside its own clipped track. */
  .ticker {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 2.5cqw;
    padding: 1.4cqw 3cqw;
    box-sizing: border-box;
    overflow: hidden;
  }
  .ticker-label {
    flex: 0 0 auto;
    font-weight: 700;
    white-space: nowrap;
  }
  .ticker-track {
    flex: 1 1 auto;
    overflow: hidden;
    white-space: nowrap;
  }
  .ticker-run {
    display: inline-block;
    padding-left: 100%;
    white-space: nowrap;
    animation: relay-ticker var(--tickdur, 18s) linear infinite;
    will-change: transform;
  }
  @keyframes relay-ticker {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(-100%);
    }
  }
  /* Reduced motion: the crawl stops and the notice sits static, still readable. */
  @media (prefers-reduced-motion: reduce) {
    .ticker-run {
      animation: none;
      padding-left: 0;
    }
    .ticker-track {
      text-overflow: ellipsis;
    }
  }
</style>
