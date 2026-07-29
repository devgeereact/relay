<script>
  // ONE renderer for both the fullscreen output (Output.svelte) and the editor
  // preview (Templates.svelte) — guarantees WYSIWYG: what you save is exactly
  // what shows. Sizes are in `cqw` (container-query width units) so the same
  // template scales identically whether the container is a full screen or a
  // small preview box.
  import { fade } from 'svelte/transition';
  import { afterUpdate, onMount, onDestroy } from 'svelte';
  import { isLayered, boundValue, templateShows } from './layers.js';
  import { applySink, getAudioOutput, onAudioOutputChange } from './audioOutput.js';

  export let template = {};
  export let content = null; // { reference, text, translation }
  // Sound is OPT-IN per surface. This same renderer draws the Templates editor
  // preview, and editing a template must not blast video audio across the room —
  // so only a real output surface passes audio={true}.
  export let audio = false;

  $: layout = template?.layout ?? {};
  $: style = template?.style ?? {};

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
    else v = boundValue(L, content);
    return lineTransform(v, L.lineTransform);
  }
  // vertical alignment → flex
  const vAlign = (v) => (v === 'top' ? 'flex-start' : v === 'bottom' ? 'flex-end' : 'center');
  $: refFirst =
    layout.refFirst || (layout.regions?.[0] === 'reference' && !layout.lowerThird);

  // Base type sizes (cqw). Real fit is measured, not guessed — see fitText().
  $: verseSize = parseFloat(style.verseSize) || 6;
  $: refSize = parseFloat(style.refSize) || 2.6;

  // Auto-fit: after every render (and on container resize), shrink the verse +
  // reference until the content box no longer overflows, so scripture is NEVER
  // clipped and NEVER spills off screen — at any output size. Font-size is set
  // imperatively on the element (not via a reactive var) so it can't re-enter
  // the Svelte scheduler and loop (CLAUDE.md rule #1). Idempotent: it resets to
  // the base size first, so it grows back when a shorter verse fires or the
  // container gets bigger.
  let stageEl;
  function fitOne(box) {
    const verse = box.querySelector('.verse');
    const ref = box.querySelector('.reference');
    // The countdown renders at 2× the verse size — fit from THAT base, not the
    // plain verse size, or it would be shrunk to half on every tick.
    const vBase = verse && verse.classList.contains('countdown') ? verseSize * 2 : verseSize;
    if (verse) verse.style.fontSize = `${vBase}cqw`;
    if (ref) ref.style.fontSize = `${refSize}cqw`;
    let scale = 1;
    let guard = 0;
    // BOTH DIMENSIONS. It only ever checked height, which is fine for a verse —
    // prose wraps, so too much text gets taller. A COUNTDOWN does not wrap: it
    // is one wide line of tabular digits, so `2:00` at 12cqw overflows sideways
    // and runs off the edges of the screen at its own natural height, and the
    // loop never noticed. Same for a long unbroken word.
    const overflows = () =>
      box.scrollHeight > box.clientHeight + 1 || box.scrollWidth > box.clientWidth + 1;
    while (overflows() && guard < 40) {
      scale *= 0.95;
      if (verse) verse.style.fontSize = `${vBase * scale}cqw`;
      if (ref) ref.style.fontSize = `${refSize * scale}cqw`;
      guard++;
    }
  }
  function fitText() {
    if (!stageEl) return;
    // During a crossfade the outgoing and incoming slides coexist — fit both so
    // whichever is on top is already sized correctly.
    stageEl.querySelectorAll('.slide .content').forEach(fitOne);
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
    if (!stageEl) return;
    const sig = fitSig();
    if (sig === lastFitSig) return;
    lastFitSig = sig;
    if (layered) fitLayers();
    else fitText();
  }
  function scheduleFit() {
    if (fitRaf) return;
    fitRaf =
      typeof requestAnimationFrame !== 'undefined' ? requestAnimationFrame(runFit) : setTimeout(runFit, 16);
  }
  afterUpdate(scheduleFit);
  let ro;
  onMount(() => {
    if (typeof ResizeObserver !== 'undefined' && stageEl) {
      // A resize genuinely changes the fit — always re-fit (coalesced to a frame).
      ro = new ResizeObserver(scheduleFit);
      ro.observe(stageEl);
    }
  });
  onDestroy(() => {
    ro?.disconnect();
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

  $: bgOpacity = style.bgOpacity == null || style.bgOpacity === '' ? 1 : clamp01(style.bgOpacity);
  // DIM SCRIM — a black overlay over the background (behind the text) to knock
  // down a bright image/background so text stays readable. 0 = none.
  $: bgDim = clamp01(style.bgDim || 0);

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
  $: panelRadius = style.panelRadius == null ? 1.4 : Number(style.panelRadius);

  // Heights. `bandHeight` (cqh) sizes the lower-third bar; `bgHeight` (%) lets the
  // background cover less than the full frame (anchored to the bottom, e.g. a
  // gradient that only fills the lower part of the screen). Both are optional.
  $: bandHeight = Number(style.bandHeight) > 0 ? Number(style.bandHeight) : null;
  $: bgHeight = style.bgHeight == null || style.bgHeight === '' ? 100 : Number(style.bgHeight);

  $: verseTransform = style.verseTransform || 'none'; // capitalization
  $: refTransform = style.refTransform || 'none';
  $: verseLineHeight = Number(style.verseLineHeight) > 0 ? Number(style.verseLineHeight) : 1.32;
  $: verseLetter = style.verseLetterSpacing ? `${Number(style.verseLetterSpacing)}em` : 'normal';
  $: refLetter = style.refLetterSpacing ? `${Number(style.refLetterSpacing)}em` : 'normal';
  // Gap between the verse and its reference (cqw, so it scales with the output).
  $: refGap = style.refGap == null ? 1.4 : Number(style.refGap);
  // Per-region font. Each layer picks its own; `style.font` is the shared default.
  $: verseFontFamily = fontFam(style.verseFont || style.font);
  $: refFontFamily = fontFam(style.refFont || style.font);
  // Per-region shadow (each falls back to the shared `textShadow`).
  $: verseShadowCss = shadowCssOf(clamp01(style.verseShadow ?? style.textShadow ?? 0));
  $: refShadowCss = shadowCssOf(clamp01(style.refShadow ?? style.textShadow ?? 0));
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
      : style.background || 'transparent';

  // Alignment is configured per template (defaults centre). Lyrics inherit it —
  // the default lower-third template is centred, matching ProPresenter.
  $: verseAlign = style.verseAlign || layout.align || 'center';
  $: refAlign = style.refAlign || layout.align || 'center';

  // Font family, with a fallback so an UNINSTALLED named font degrades to the
  // computer's default rather than something arbitrary. A CSS var already carries
  // its own generic; a bare family name ("Didot") does not, so append one.
  $: fontFamily = (() => {
    const f = style.font || 'var(--f-serif)';
    if (f.startsWith('var(')) return f; // the var supplies its own fallback
    return `${f}, system-ui, sans-serif`;
  })();

  // Slide transition (ProPresenter-style dissolve). Duration is template config
  // (style.transitionMs, default 250ms); reduced-motion users get an instant cut.
  //
  // The slide uses `in:fade` ONLY — never a bidirectional `transition:fade`. With
  // `{#key}` a bidirectional transition keeps the OUTGOING slide mounted (stacked
  // on top, since the new block mounts before it) until its outro finishes; a
  // rapid second fire interrupts that outro and Svelte can leave the stale node
  // on top forever, so the wall looks FROZEN on the first verse while the store
  // has long since moved on. `in:` destroys the old slide immediately — the new
  // verse can never be hidden behind a lingering one.
  const reduced =
    typeof window !== 'undefined' &&
    window.matchMedia &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  // Guard against a non-numeric transitionMs: `Number("x")` is NaN, and a NaN
  // duration makes the tween never complete.
  $: dur = (() => {
    if (reduced) return 0;
    const n = Number(style.transitionMs ?? 250);
    return Number.isFinite(n) ? Math.max(0, n) : 250;
  })();

  // Countdown: tick a local clock only while a target is set. The number updates
  // in place via its own reactive (`now`), which slideKey excludes — so ticks
  // never re-key the slide (no per-second crossfade). setInterval (not Svelte's
  // tick()) keeps this clear of the reactive-loop freeze (CLAUDE.md rule #1).
  $: countdownTo = content?.countdown_to ?? null;
  let now = 0;
  let cdTimer = null;
  $: if (countdownTo) startClock();
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
  $: remainingMs = countdownTo ? Math.max(0, countdownTo - now) : null;
  $: countdownDone = remainingMs === 0;
  $: countdownText = (() => {
    if (remainingMs == null) return '';
    const s = Math.round(remainingMs / 1000);
    return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
  })();

  // Re-key on the actual content so a new slide crossfades but identical content
  // (a re-broadcast of the same verse) does not re-animate. Countdown ticks are
  // deliberately excluded — only a NEW countdown target re-keys.
  $: slideKey = `${content?.reference ?? ''}|${content?.text ?? ''}|${content?.media_url ?? ''}|${countdownTo ?? ''}`;

  // A wall clock for clock-bound layers — ticks once a second only when needed.
  let clockNow = 0;
  let clockTimer = null;
  $: needClock = layered && layers.some((L) => L.bind === 'clock');
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
    return layers.map((L) => ({ L, text: layerText(L) }));
  })();

  // Per-text-layer auto-fit. Each layer's text is sized to BEST FIT its own box —
  // it scales DOWN when there is a lot of text and UP when there is little, and it
  // is NEVER allowed to push outside the box (both dimensions are checked). A
  // binary search finds the largest font that fits, so short text fills the box
  // and long text wraps and shrinks — the ProPresenter "scale text up or down"
  // behaviour. `fit` modes: 'both' (default, up+down), 'shrink' (cap at the set
  // size, only shrink), 'none' (use the set size verbatim).
  function fitLayers() {
    if (!stageEl || !layered) return;
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
    });
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
    {#each layerViews as { L, text } (L.id)}
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
          <div class="lshape" style="{boxStyle(L)} background:{hexA(L.fill, L.opacity == null ? 1 : L.opacity)}; border-radius:{L.radius || 0}cqw;"></div>
        {:else if !(showDefaultCountdown && (L.bind === 'verse' || L.bind === 'reference' || L.bind === 'translation'))}
          <!-- Verse/reference/translation layers are hidden during a default
               countdown (they carry no content then); a static or clock layer
               still shows. -->
          <div class="ltext" style="{boxStyle(L)} align-items:{vAlign(L.valign)};">
            {#key text}
              <div
                class="lfit"
                class:lscroll={L.scroll}
                data-base={L.size}
                data-fit={L.fit || 'both'}
                style="color:{L.color}; font-family:{fontFamOf(L.font)}; font-weight:{L.weight || 400}; text-align:{L.align}; text-transform:{L.transform || 'none'}; line-height:{L.lineHeight || 1.3}; letter-spacing:{(L.letterSpacing || 0)}em; text-shadow:{shadowOf(L.shadow)}; font-style:{L.italic ? 'italic' : 'normal'};"
                in:fade={{ duration: dur }}>
                {#if L.scroll}
                  <span class="lrun" style="--tickdur:{Math.min(60, Math.max(10, (text?.length || 0) * 0.42))}s">{text}</span>
                {:else if L.quote && text}
                  “{text}”
                {:else}
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
        <div class="verse countdown" style="font-size:{verseSize * 2}cqw; margin-top:{refGap}cqw; color:{verseColor}; text-align:center; text-shadow:{verseShadowCss};">
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
        in:fade={{ duration: dur }}>
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
              <div class="verse countdown" style="font-size:{verseSize * 2}cqw; color:{verseColor}; text-align:{verseAlign}; text-shadow:{verseShadowCss};">
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
                <div class="verse" style="font-size:{verseSize}cqw; {verseStyle}">{#if hasRef}“{content.text}”{:else}{content.text}{/if}</div>
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
  .ltext,
  .lmediabox {
    position: absolute;
    box-sizing: border-box;
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
