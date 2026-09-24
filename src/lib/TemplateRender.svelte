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

  // ── THE THREE NUMBERS THE LAYER FIT IS BOUNDED BY ──────────────────────────
  //
  // Exported so the reasoning is checkable without a browser, and so nobody has
  // to guess what a magic 16 meant.
  //
  // The smallest size the search will settle on. Below this the words are gone
  // rather than small, and rule 37 says a verse that cannot fit is SHRUNK and
  // REPORTED, never blanked — so this is a floor on the answer, not a refusal.
  export const FIT_FLOOR_CQW = 0.4;
  // How close the bracket has to get before the search stops asking. Sizes here
  // are cqw — a share of the output's WIDTH — so this is resolution-independent:
  // 0.02cqw is 0.38px of font-size on a 1920-wide wall, 0.77px on a 4K one, and
  // 0.05px on a slide-grid thumbnail. The old loop ran a flat sixteen rounds,
  // which resolves a 21.6cqw bracket to 0.0003cqw — four decimal places of a
  // quantity that is not visible at two, bought with five forced layouts.
  export const FIT_EPS_CQW = 0.02;
  // The hard bound, unchanged. A bisection halves its bracket every round, so
  // sixteen rounds is far more than `FIT_EPS_CQW` ever needs; it stays as the
  // thing that guarantees termination whatever the bracket.
  export const FIT_MAX_ROUNDS = 16;

  /**
   * How many rounds a bracket of this width needs to reach `FIT_EPS_CQW`.
   *
   * Pure, and exported, because "is eleven enough?" is arithmetic and should not
   * need a rendered page to answer.
   */
  export function fitRoundsFor(span, eps = FIT_EPS_CQW, max = FIT_MAX_ROUNDS) {
    if (!(span > 0) || !(eps > 0)) return 0;
    return Math.min(max, Math.ceil(Math.log2(span / eps)));
  }
</script>

<script>
  // ONE renderer for both the fullscreen output (Output.svelte) and the editor
  // preview (Templates.svelte) — guarantees WYSIWYG: what you save is exactly
  // what shows. Sizes are in `cqw` (container-query width units) so the same
  // template scales identically whether the container is a full screen or a
  // small preview box.
  import { afterUpdate, onMount, onDestroy } from 'svelte';
  import { isLayered, isKeyedTemplate, boundValue, templateShows, formatElapsed, formatRemaining, formatCountdown, countdownParts, countdownWarning, topLevelLayers, drawBoxes } from './layers.js';
  // ONE timer, ONE formatter (docs/REBRAND.md §7). `layers.js` owns the formatter;
  // `countdown.js` owns the arithmetic in front of it — including the one exception,
  // a countdown that is being HELD.
  import { countdownRemainingMs, countdownIsPaused, countdownTotalMs } from './countdown.js';
  import { applySink, getAudioOutput, onAudioOutputChange } from './audioOutput.js';
  // THE PROGRAMME RAIL AND THE ALERT'S SIZING — both shared, neither re-derived.
  // `timers.js` is the one place the Stage Timer set becomes rows, so this
  // renderer and the preacher's phone cannot disagree about the same clock;
  // `stagealert.js` is the one table that says how big a Stage Message is.
  import {
    programmeRows,
    railSize,
    programmeCh,
    programmeCapacity,
    programmeCells,
    programmeRoom,
  } from './timers.js';
  import { alertStep } from './stagealert.js';
  // A STILL ASKS FOR ITS FRAME (RG-279): `preload="metadata"` sizes the element
  // and paints nothing. NEVER on a playing clip — the fragment is a seek, and it
  // would drag the wall back to 0.1s on every re-render.
  import { posterUrl } from './posterframe.js';
  // ONE threshold and one formatter, shared with the phone (RG-280): two
  // surfaces showing the same figure from two constants is a figure that will
  // one day differ between the stage and the desk.
  import { clipRemainingMs, CLIP_WARN_MS } from './mediaclock.js';
  import { RAIL_BASE_PCT } from './bigstagetimer.js';
  // WHO TAKES A STAGE SCREEN NOBODY HAS FIRED TO (RG-285). One function over
  // `stageresting.js`, which is the preacher's phone's own rule — imported and
  // never restated, so the two surfaces in one room cannot hold two opinions
  // about when a clock may take a screen.
  import {
    stageFill,
    fillGeometry,
    clipCoversTimer,
    FILL_SIZE_CQW,
  } from './stagefill.js';
  import { applyMediaTransport } from './mediatransport.js';
  import { syncSeek } from './mediasync.js';

  export let template = {};
  export let content = null; // { reference, text, translation }
  import { resolveTokens } from './styletokens.js';
  import { resolveStyle, slideBG, faceOf, fitScale, keepShrinking, FIT_STEP } from './templatemodel.js';
  import { transitionCss, transitionDuration, resolveTransition, isOverride, liveTransition } from './transitions.js';
  import { builtinById } from './templates.js';
  // Sound is OPT-IN per surface. This same renderer draws the Templates editor
  // preview, and editing a template must not blast video audio across the room —
  // so only a real output surface passes audio={true}.
  export let audio = false;
  /**
   * THE STAGE MESSAGE — supplied by the page, never by the content.
   *
   * It arrives on its own hub frame (`stage_alert`) and the page has already
   * decided whether this screen may be shown one: only a channel whose role is
   * `stage` accepts it. Passing it as a prop rather than putting it on
   * `OutputContent` is what keeps it off every other screen — a field on the
   * content would be broadcast to all of them, and the only thing between it and
   * a lobby TV would be which layers that TV's template happens to have.
   *
   * Empty on the console previews and in the Templates editor, deliberately:
   * neither is a stage screen, and a preview that painted a private message
   * would put it on the one surface an operator shows people.
   */
  export let stageMessage = '';
  /**
   * IS THIS A NOTE OR AN ALARM (operator, 2026-09-21; DECISIONS §116).
   *
   * There was one rendering — the full-bleed flashing panel — so every word sent
   * to a preacher arrived as an emergency. `false` is the safe default for the
   * same reason it is on the command: a caller that did not ask for an alarm must
   * not get one.
   */
  export let stageUrgent = false;
  /**
   * THE STAGE TIMERS — supplied by the page, never by the content, exactly like
   * `stageMessage` above and for the same reason.
   *
   * Rows as the `timer` hub frame carries them. The page has already decided
   * whether this screen may be shown them: only a channel whose role is `stage`.
   * A field on `OutputContent` would be broadcast to every screen in the
   * building, and "Sermon · 4:12 left" behind a preacher is the running order in
   * front of the whole congregation — which is the reason `r6-contracts.test.js`
   * recorded `timer: false` for this page in the first place, and the reason the
   * reversal is gated on the role rather than on the frame.
   *
   * It renders a SET, which is what distinguishes it from a `countdown` layer:
   * that is one congregation clock riding on the fired content, this is however
   * many Stage Timers are running, each with its own label and its own warning.
   *
   * Empty on the console previews and in the Templates editor, deliberately —
   * neither is a stage screen.
   */
  export let programme = [];
  /**
   * HOW MUCH ROOM THE FALLBACK PROGRAMME RAIL TAKES — RG-265.
   *
   * A MULTIPLIER, never a setting. `bigstagetimer.js::railScale` is the one
   * place that turns Normal / Large / Huge into a number, and it re-exports the
   * phone's own steps — a renderer that knew what `huge` meant would be a second
   * opinion about a control the operator set once, and this component is shared
   * with the console's panes and the Templates editor besides.
   *
   * 1 for every caller that does not pass one, which is all of them but
   * `Output.svelte`: a designed rail states its own box and is untouched by
   * this.
   */
  export let timerScale = 1;
  /**
   * IS THIS SCREEN A STAGE? — RG-280.
   *
   * Decides one thing: whether the clip playing here is given a countdown. A
   * congregation must not be shown one — it is the preacher's cue to get ready
   * and not theirs — so this defaults to false and `Output.svelte` passes its
   * own role.
   *
   * RG-256 put this clock on `stage.html`, the phone. This page had none of any
   * kind, which is why the operator asked three times about a screen they had
   * labelled STAGE MONITOR.
   */
  export let stageClip = false;
  /** How long is left of the clip on this screen, or `null` (RG-280). */
  let clipLeft = null;
  // A CLOCK MAY NOT OUTLIVE THE CLIP IT COUNTS (RG-280). `videoEl` is
  // `bind:this`, so Svelte nulls it the moment the clip unmounts - a verse
  // fired over a video would otherwise leave the last reading frozen on the
  // platform monitor, which is rule 35 in a smaller costume.
  $: if (!videoEl || still) clipLeft = null;
  $: clipWarn = clipLeft != null && clipLeft <= CLIP_WARN_MS;
  /**
   * How deep this render is inside a composite. 0 is the screen itself.
   *
   * A REGION LAYER ONLY RENDERS AT DEPTH 0 — "a composite may not be another
   * composite's fill" (docs/REBRAND.md §6). Without it a template that names
   * itself would recurse until the webview died, on a wall, mid-service.
   */
  export let depth = 0;

  // Resolve any layer style TOKENS (`theme:accent` and friends) against this
  // template's own style, so a starter dropped onto a template wears that
  // template's colours and typeface. A template with no tokenised layer hits the
  // fast path and comes back unchanged.
  $: resolved = resolveTokens(template);
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
  // presets are untouched). Layers are drawn back-to-front.
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

  // ── THE STANDING BACKGROUND ────────────────────────────────────────────────
  //
  // A SECOND PAYLOAD, not a field on the content, and the difference is lifetime.
  // `content.media_url` is the fired picture: it IS the slide, and the next verse
  // replaces it. `backdrop` is what the church put up behind everything at the top
  // of the service, and a verse painted over it must leave it exactly where it is.
  // Until this prop existed the two were the same field, so scripture over a
  // church's own background could not be expressed at all.
  //
  // `{ media_url, media_kind }` or null. Null is the answer a cleared wall gives,
  // and the panic controls are what produce it — see `Output.svelte`, which drops
  // this on `clear` and `black` at both doors.
  export let backdrop = null;
  // OPT-IN BY TEMPLATE DESIGN, WHICH IS THE WHOLE OF THE OPT-IN. There is no
  // setting and no flag: a template with no `backdrop` layer renders byte for byte
  // what it rendered before this prop existed, because `backdropUp` is false and
  // every expression below collapses to its old form. A stored preference nothing
  // reads is the defect the 2026-09-10 pass closed seven Settings controls of.
  $: hasBackdropLayer = layered && layers.some((L) => L.type === 'backdrop' && L.visible !== false);
  $: backdropUp = !!backdrop?.media_url && hasBackdropLayer;
  // ── IS THERE A PICTURE BEHIND THE CLOCK RIGHT NOW? (RG-212) ────────────────
  //
  // The programme rail is digits with no backing, which is right over a
  // template's own background and unreadable over a clip — and a clip's
  // brightness changes frame by frame, so no designer can pre-solve it in the
  // template. The rail earns a plate while a picture is painting and loses it
  // when the picture goes; a permanent one would be a box sitting over every
  // template that never shows a picture at all.
  //
  // Both kinds count. A fired clip or slide (`content.media_url`, gated on
  // `allowMedia` because a screen told not to show fired media has no picture on
  // it) and the standing backdrop, which is behind everything for the whole
  // service and is the likelier of the two to be bright.
  $: pictureBehind = (!!content?.media_url && allowMedia) || backdropUp;
  // DELIBERATELY NOT GATED ON `allowMedia`. That answers "does this SCREEN show
  // fired media" — a lower third keeping a camera clean while a picture fills the
  // main wall — and a backdrop is not fired media. Having the layer at all is the
  // consent, per screen, which is the same answer `templateShows` gives by a
  // longer route and one authority instead of two.

  // ── Countdown policy ─────────────────────────────────────────────────────────
  // Same idea as media: a fired countdown shows its MM:SS BY DEFAULT — the wall
  // needs no timer layer. A layered template's scripture text layers can't render
  // a countdown (they bind to the verse/reference, which a countdown doesn't
  // carry), so without this a countdown showed only its label and no digits. A
  // TIMER layer (or any layer bound to 'countdown') is opt-in placement; when one
  // exists it renders the MM:SS itself and this default steps aside.
  $: hasTimerLayer = layered && layers.some((L) => L.visible !== false && (L.type === 'timer' || L.bind === 'countdown'));
  // AND IT ASKS THE KEYED QUESTION TOO. `countdownAllowed` is declared with the
  // band-layout block far below; Svelte's reactive statements are ordered by
  // dependency rather than by position, so reading it here is the same one
  // predicate both models ask. Before this it was asked by the region markup
  // alone, and `.cd-default` — `position:absolute; inset:0` — painted a
  // full-frame clock straight over the camera on every layer-model lower third.
  $: showDefaultCountdown =
    layered && content?.countdown_to != null && !hasTimerLayer && countdownAllowed;
  // ── THE SAME FALLBACK, FOR THE PREACHER'S CLOCKS (RG-224) ──────────────────
  //
  // A stage screen wearing a CONGREGATION template has no programme layer — and
  // it should not: the running order has no business on a template designed for
  // a wall. So the clocks arrived, found nowhere to paint, and the preacher had
  // no clock at all, which is what the operator reported.
  //
  // Exactly the shape of `showDefaultCountdown` one line above, and the same
  // trade: a designed layer is better, is what the template editor is for, and
  // is NOT required for the thing to work. `programmeLayers` wins when there is
  // one, so this can never be a second rail.
  //
  // **The gate is the ROLE and the role alone**, because `progSet` is already
  // empty on any screen that is not a stage (`Output.svelte::shownProgramme`).
  // That is the whole safety argument: a default keyed on the template rather
  // than the role would put "Sermon · 4:12 left" in front of the building on
  // every screen wearing an ordinary look, which is every screen a church owns.
  $: hasProgrammeLayer = layered && layers.some((L) => L.bind === 'programme' && L.visible !== false);
  $: showDefaultProgramme = layered && progRows.length > 0 && !hasProgrammeLayer;
  // ── THE TEMPLATE'S OWN DEFAULTS, WHICH ARE NOT THE APP'S ──────────────────
  // This component renders BOTH the console's preview and the congregation's
  // wall, so every fallback it reaches for is a fallback a church sees. Two of
  // them used to be operator-console tokens: `var(--f-serif)` for an unset face
  // and `var(--v-amber)` for an unset accent. Both are declared in `app.css`,
  // which is the app's chrome — `--f-display` has already been re-aliased once,
  // from Space Grotesk to Inter, silently changing the typeface of every template
  // naming it, and `--v-amber` is the console's ON AIR colour, which rule 18
  // reserves for a meaning a wall does not carry.
  //
  // A default here belongs to the TEMPLATE. Fraunces is the family `--f-serif`
  // resolves to today, so nothing on a wall moves; white is the neutral accent —
  // it is what the High Visibility and Worship Lyrics templates already ask for,
  // and it is none of the three colours rule 18 has spoken for (amber = ON AIR,
  // cyan = a guess, amethyst = rehearsal). Wave 5, Track E.
  const DEFAULT_FAMILY = "'Fraunces', Georgia, serif";
  const DEFAULT_ACCENT = '#ffffff';
  // A legacy row may still hold `var(--f-serif)`; it is passed through unchanged,
  // because the token block is still shared with output.html and stage.html.
  const fontFamOf = (f) => {
    if (!f) return DEFAULT_FAMILY;
    return f.startsWith('var(') ? f : `${f}, system-ui, sans-serif`;
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
    // From the PAGE, not from the content — see the prop above and
    // `layers.js::boundValue`, which returns nothing for this bind on purpose.
    else if (L.bind === 'stage_message') v = stageMessage || '';
    else v = boundValue(L, content);
    return lineTransform(v, L.lineTransform);
  }
  // vertical alignment → flex
  const vAlign = (v) => (v === 'top' ? 'flex-start' : v === 'bottom' ? 'flex-end' : 'center');
  /** Does this layer's text change on a CLOCK rather than on content? The four
   *  binds `layerText` reads from a ticking source — the ones whose words move
   *  several times a second and whose size deliberately must not be re-measured
   *  when they do. */
  const TICKING = ['countdown', 'clock', 'elapsed', 'remaining'];
  const isTicking = (L) => TICKING.includes(L?.bind);
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
  /**
   * WHERE THE CLIP IS, reported by the element that is actually playing it.
   *
   * Called with `{ pos_ms, dur_ms, paused }` while a video is on this screen, and
   * with `null` the moment one is not. `null` matters as much as the numbers: a
   * last-known position left behind after the clip came off would have the console
   * counting down a clip nobody is watching.
   *
   * The consumer is the beat (`outputHealth.js`), so the figure an operator reads
   * comes from the screen that is painting rather than from the console's own
   * preview of the same file. See `channels::MediaBeat` for why that distinction
   * is the whole design.
   */
  export let onMedia = null;
  /**
   * WHAT THE OPERATOR HAS ASKED THE CLIP TO DO.
   *
   * `{ paused, loop, replayEpoch }`, or `null` for the default a clip is fired
   * with: playing, not looping.
   *
   * **Applied to the element, never by re-mounting it.** `{#key slideKey}` rebuilds
   * the video whenever the content changes, and rebuilding it to pause it would
   * restart the clip from zero — the opposite of what Pause means. So these are
   * read in a reactive block that touches the existing element and nothing else.
   */
  export let mediaTransport = null;
  /**
   * PAINT THE FIRST FRAME AND STOP THERE (RG-235).
   *
   * A deck cell is a thumbnail of a cue, and four video cues in a plan meant
   * four clips playing at once under the one that is actually on air — on the
   * surface an operator works from for a whole service.
   *
   * It is a property of the RENDER and not of the content, because the same clip
   * is live on a wall and a thumbnail in a deck at the same instant and only one
   * of them is playing. Default `false`, so every surface that paints a
   * congregation screen is unchanged by construction: a surface has to ask.
   *
   * A still is not a paused clip. `preload="metadata"` fetches enough for a
   * frame and never the file, so twenty cues are twenty small requests rather
   * than twenty downloads over a church's network. It reports nothing through
   * `onMedia` either — a thumbnail answering "where is the clip" would put a
   * deck cell's position into the readout an operator times the next cue
   * against.
   */
  export let still = false;
  /**
   * A PICTURE OR CLIP THAT DID NOT LOAD, or `null` once one has (2026-09-21, O-4).
   *
   * Six media elements and not one `on:error`: a 404, a codec the webview cannot
   * decode and a CSP refusal were the same observable event — nothing — while the
   * beat still said `content` and the desk printed On Air in amber over a blank
   * frame. Every fired-media element now reports through this one callback, and
   * reports the failure healed on `load`/`loadeddata`, so the page can carry it on
   * the beat and the desk can stop calling the screen On Air.
   */
  export let onMediaError = null;
  /**
   * RELAY'S CLOCK MINUS THIS SCREEN'S, in ms (RG-194, 2026-09-21). `countdown_to`
   * is an absolute epoch produced on the Relay machine; a browser screen with a
   * clock a minute out showed a minute of error to the room while the projector
   * beside it was right. The page measures the offset from `beat_ack` and hands
   * it here; the console and the native window pass nothing and get zero.
   */
  export let hostOffsetMs = 0;
  $: if (cdTimer && Number.isFinite(hostOffsetMs)) now = Date.now() + hostOffsetMs;
  let mediaFailedUrl = null;
  const mediaFailed = (kind, url) => {
    mediaFailedUrl = url;
    onMediaError?.({ kind, url });
  };
  const mediaLoaded = () => {
    if (mediaFailedUrl === null) return;
    mediaFailedUrl = null;
    onMediaError?.(null);
  };
  // A new clip or picture is a new question; the last one's failure must not
  // stand for it. Fires on the URL, so a re-render of the same content is silent.
  $: if (content?.media_url !== mediaFailedUrl && mediaFailedUrl !== null) mediaLoaded();

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
  /**
   * THE CRAWL HAS A BUDGET, AND THE LABEL SPENDS IT FIRST.
   *
   * A ticker is a band: a fixed label on the left, then the body scrolling
   * through whatever room is left. Neither element is `.content`, so the region
   * fitter never saw either — the loop found zero boxes and reported a scale of
   * 1, which is the "fit loop with no notion of failure" of rule 37 with the
   * loop removed entirely.
   *
   * THE BUDGET IS A CSS FACT. `.ticker-label`'s own `max-width: 45%` and
   * `overflow: hidden` are the one source for the label's share of the band
   * (see the stylesheet) — `label.clientWidth` is therefore already the real,
   * constrained box a browser paints, so it is what gets measured, not a
   * second JS computation (`band.clientWidth * 0.45`) that could drift from
   * the rule that actually clips.
   *
   * THE BASE IS THE TEMPLATE'S DECLARED SIZE, READ FRESH EVERY PASS — never
   * the DOM's last write. `refSize` is cqw, 1% of `stageEl`'s inline size
   * (`container-type: size` — the same fact `fitOne`'s own comment relies on
   * for the box share above), so converting it to the px this loop measures
   * in is `refSize / 100 * stageEl.clientWidth`. An earlier version of this
   * function read `getComputedStyle(label).fontSize` instead, which reads
   * back fitTicker's OWN previous px write on any later pass: `fitSig()`
   * folds the stage's rounded w×h into every region-mode signature, so an
   * ordinary window RESIZE with no content change — the label is not
   * rebuilt; `{#key slideKey}` keys on content, not geometry — still calls
   * `fitText` → `fitTicker` again for the very same `<span>`. A label shrunk
   * once during a narrow moment stayed shrunk for the rest of that
   * announcement even after the window widened back out, because what it
   * thought was "the declared size" was actually its own last answer.
   * Recomputing from `refSize` every pass is what lets a widened band grow
   * the label back — the same ceiling `fitOne` holds via its prop-derived
   * `vBase`, reached a different way: this loop always starts its search AT
   * that ceiling rather than seeding a guess and growing up to it.
   *
   * The label is what is worth measuring: it is `nowrap`, so it never wraps,
   * it simply takes the width its cap allows. The body is deliberately NOT
   * shrunk: it scrolls, so its length is time, not overflow.
   */
  function fitTicker() {
    if (!stageEl) return 1;
    const band = stageEl.querySelector('.ticker');
    const label = stageEl.querySelector('.ticker-label');
    if (!band || !label) return 1;
    const budget = label.clientWidth || 0;
    if (budget <= 0) return 1;
    const base = (refSize / 100) * stageEl.clientWidth;
    if (!base) return 1;
    let scale = 1;
    label.style.fontSize = `${base}px`;
    while (keepShrinking({ overflowing: label.scrollWidth > budget, scale })) {
      scale *= FIT_STEP;
      label.style.fontSize = `${base * scale}px`;
    }
    return Math.min(scale, fitStillBody(band));
  }
  /**
   * THE BODY IS ONLY EXEMPT WHILE IT IS MOVING.
   *
   * `fitTicker` above says the body is deliberately not shrunk, *"it scrolls, so
   * its length is time, not overflow"*, and that is exactly right of a crawl. It
   * is exactly wrong of a stopped one: under `prefers-reduced-motion` there is no
   * later, so the length IS overflow again and the track's `overflow: hidden`
   * silently takes whatever does not fit.
   *
   * THE TRACK IS WHAT IS MEASURED, not the run. `.ticker-run` is an
   * `inline-block` under `white-space: nowrap`, so it sizes its own box to its
   * content and `scrollWidth` can never exceed `clientWidth` on it — the same
   * trap `.ticker-label`'s own comment records about the label's 45% cap, and the
   * same one `.content` fell into in RG-141. `.ticker-track` is the flex item
   * with `overflow: hidden`, so it is the box that can report.
   *
   * The answer is returned rather than reported here, so it folds into
   * `fitText`'s `worst` and reaches `onFit` through the one reporter — the same
   * route every other box on this page takes (`report()`, `verifyFit`). A notice
   * that has to go below rule 37's floor to fit its band is then shown small and
   * SAID, which is the whole of the rule: small and reported is a thing an
   * operator can act on before next Sunday, and 62% of a notice that was never
   * painted is not.
   */
  function fitStillBody(band) {
    if (!reduceMotion) return 1;
    const track = band.querySelector('.ticker-track');
    const run = band.querySelector('.ticker-run');
    if (!track || !run) return 1;
    // Same reasoning as the label's base above: read the template's declared size
    // fresh every pass, never this function's own previous write, or a band that
    // was narrow for one frame stays shrunk for the rest of the notice.
    const base = (verseSize / 100) * stageEl.clientWidth;
    if (!base) return 1;
    let scale = 1;
    run.style.fontSize = `${base}px`;
    while (keepShrinking({ overflowing: track.scrollWidth > track.clientWidth + 1, scale })) {
      scale *= FIT_STEP;
      run.style.fontSize = `${base * scale}px`;
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
    // Ticker mode renders instead of `.content`, so the query above finds
    // nothing at all. Its own pass is the only measurement this mode gets.
    worst = Math.min(worst, fitTicker());
    // The WORST of the slides on screen. It is HANDED ON rather than reported
    // here: how far this had to shrink is only half the verdict, and the other
    // half — whether it actually fits — cannot be read until a later frame. One
    // reporter, at the point where the answer is complete (`verifyFit`).
    lastFitScale = worst;
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
          // THE WORDS, NOT THEIR LENGTH — because `{#key text}` rebuilds this
          // layer's `.lfit` on ANY text change, and a rebuilt element carries the
          // declared base and no fitted size. A length was close enough to look
          // right and let two passages of equal length share a signature: the fit
          // was skipped, the new element kept the base, it clipped, and
          // `verifyFit` never ran so nothing was reported either. Measured on the
          // console after a fire — `.lfit` at 5.2cqw, the untouched default verse
          // size, 207px of content in a 174px box, no warning, corrected only by a
          // window resize (which moves `clientWidth`, which IS in the signature).
          //
          // A TICKING layer is the exception it has always been, and it is why
          // this cannot simply be the text everywhere: a countdown or clock
          // changes its text four times a second, so its length is what is folded
          // in, exactly as before, and the reflow storm that gating exists to
          // prevent stays prevented. Its rebuilt element keeps its size through
          // `reapplyFitted` instead — the digits hold width as they count.
          const t = layerText(L) || '';
          s += `|${L.w},${L.h},${L.size},${isTicking(L) ? t.length : t}`;
        }
      }
      return s;
    }
    return `R${w}x${h}|${verseSize}|${refSize}|${content?.reference ?? ''}|${(content?.text ?? '').length}|${bandMode ? 1 : 0}|${countdownTo ? 1 : 0}`;
  }
  /**
   * IS THE FITTER'S ANSWER THE SIZE THIS ELEMENT IS ACTUALLY WEARING?
   *
   * `data-sized` used to be the whole question, and it is a ONE-WAY LATCH: it
   * records that a fit once happened, never that its answer is still on the
   * element. Both recovery paths below asked it, so both declined over an element
   * whose fitted size had been wiped while the latch stayed set — which is rule
   * 37's shape at the recovery layer rather than inside the loop. A flag that
   * cannot report that the thing it stands for has been undone is not a flag.
   *
   * Attribute reads only — `style.fontSize` is the inline declaration, not a
   * computed value — so this is free on the frames where it says yes. A box with
   * no recorded answer is left to the latch: there is nothing to compare against,
   * and claiming a disagreement from an absence is the same lie in the other
   * direction (rule 39's `built_shape`).
   */
  function fitInForce(box, el) {
    if (!el.dataset.sized) return false;
    const px = box?.dataset?.fitted;
    if (!px) return true;
    return sizeIs(el, px);
  }
  /**
   * IS THIS ELEMENT WEARING THIS SIZE? — and the comparison may not be a string
   * one, which cost a measured regression on the way to the fix above.
   *
   * The fitter's answers are full-precision JS numbers (`21.166796875`), and the
   * CSSOM does not store the string it was handed: writing `21.166796875cqw` and
   * reading `style.fontSize` straight back returns `21.1668cqw` in Chrome. A
   * string compare therefore says "different" forever, on an element that is
   * wearing exactly the right size — which sent `anythingUnfitted` true on every
   * frame and drove the full binary search four times a second. Measured with
   * that compare in place: 118 fit passes and 2682 style writes in 29 seconds of
   * one countdown, the precise reflow storm the fit gating exists to prevent.
   *
   * `FIT_EPS_CQW` is the fitter's own idea of a difference nobody could see, so
   * it is the right tolerance: anything inside it IS this size, and a clobber
   * back to a declared base that happens to land inside it needed no repair
   * anyway. A missing or unparseable size is NaN and fails, which is the honest
   * answer for an element nothing has sized.
   */
  function sizeIs(el, px) {
    return Math.abs(parseFloat(el.style.fontSize) - Number(px)) <= FIT_EPS_CQW;
  }
  /**
   * Hand an element back the size its layer was already fitted at.
   *
   * TWO THINGS TAKE IT AWAY, and for a long time this function knew about one.
   *
   *   1 · `{#key text}` destroys and rebuilds a layer's `.lfit` whenever its words
   *       change, and the new one carries only the DECLARED base.
   *   2 · SVELTE RE-WRITES THE SIZE ON AN ELEMENT IT DOES NOT REBUILD. The `.lfit`
   *       markup declares `font-size:{baseSize(L)}cqw` inline, deliberately (rule
   *       42 · `cardfit.test.js`), and Svelte 4 compiles that attribute into one
   *       `set_style(div, 'font-size', …)` per interpolation whose update is
   *       guarded on the DIRTY BIT ALONE — there is no value comparison, unlike
   *       the plain `data-base` / `data-fit` attributes beside it. So every update
   *       that marks `stackLayers` dirty re-writes the declared base over the
   *       imperative fit, in place, on every visible text layer.
   *
   *       A countdown tick is exactly that update (`countdownText` → `layerViews`
   *       → `stackLayers`) and it is the case where nothing repairs it: `fitSig`
   *       folds a ticking layer in by text LENGTH, so `4:59` → `4:58` does not move
   *       the signature and no re-fit runs. Measured at 1920×1080 on the shipped
   *       `Timer · Titled` with a 118-character label: fitted `1.99375cqw`, painted
   *       `3.4cqw` from the first tick onward, 176px of words in a 97px
   *       `overflow:hidden` box — and `onFit` had already reported the fit a
   *       success, because it was, half a second earlier (RG-139).
   *
   * Both are now the same question, asked of the DOM rather than of the trigger:
   * is the size on the element the size we answered for its box? Pure style
   * writes: no `scrollHeight`, no reflow, so it stays free at 4 Hz.
   */
  function reapplyFitted() {
    if (!stageEl || !layered) return;
    stageEl.querySelectorAll('.ltext').forEach((box) => {
      const px = box.dataset.fitted;
      if (!px) return;
      const el = box.querySelector('.lfit');
      if (!el) return;
      if (!sizeIs(el, px)) el.style.fontSize = `${px}cqw`;
      el.dataset.sized = '1';
    });
  }
  /**
   * IS ANYTHING ON SCREEN WEARING ONLY ITS DECLARED BASE?
   *
   * An element the fitter has never sized is a fit that has NOT HAPPENED — not
   * one that succeeded. That is rule 37's shape ("a fit loop with no notion of
   * failure always succeeds") one level above the loop, and it is the hole every
   * round of this audit has fallen through in a different costume.
   *
   * The instance that found it: `fitSig` is VALUE-based — each layer's `w,h,size`
   * and its words — while the DOM is IDENTITY-keyed (`{#each layerViews as …
   * (L.id)}`). A template arriving with the same geometry and the same words but
   * different layer ids therefore rebuilds every `.ltext` and `.lfit`, leaving
   * them at the declared base with nothing remembered to re-apply, while the
   * signature does not move a character. Measured on the console: 5.2cqw
   * untouched, 207px of content in a 174px box, no warning, still wrong at six
   * seconds — and the next content change fitted fine, because that moves the
   * signature. That is what `loadTemplates()` resolving after the first render
   * hands over, and what an operator swapping a screen's template mid-service does.
   *
   * Asked as a question about the DOM rather than about the trigger, because the
   * trigger has been something different every round. Attribute reads only: no
   * `scrollHeight`, no reflow, so this is free on the frames where it says no.
   */
  function anythingUnfitted() {
    if (!stageEl || !layered) return false;
    for (const el of stageEl.querySelectorAll('.lfit')) {
      // The SAME question `reapplyFitted` asks, so the two cannot disagree about
      // what "fitted" means. `reapplyFitted` runs first and repairs anything with
      // a recorded answer, so what survives to here is a box that has none — a
      // layer the fitter has genuinely never measured — and that is a real fit,
      // with a real verdict, not a style write.
      if (!fitInForce(el.closest('.ltext'), el)) return true;
    }
    return false;
  }
  function runFit() {
    fitRaf = 0;
    if (!stageEl || !visible) return; // don't reflow an offscreen render
    const sig = fitSig();
    if (sig === lastFitSig) {
      // A ticking layer's element was just rebuilt and is wearing the declared
      // base. Give it back the size this layer was fitted at — a style write, no
      // layout read, so it stays free at 4 Hz.
      reapplyFitted();
      // ANYTHING STILL WEARING ITS BASE HAS NOT BEEN FITTED, so fall through and
      // fit it for real — which also produces a verdict, and therefore a report.
      // The signature describes the SHAPE and the DOM is keyed on IDENTITY, so
      // the two can disagree; asking the DOM is what makes this independent of
      // whichever trigger caused the disagreement.
      if (!anythingUnfitted()) {
        // The FIT is still the right fit. The VERDICT may not be: something told
        // us the geometry moved, and the verdict is about the geometry. Re-take
        // it with a fresh budget — a resize is a new situation, not a
        // continuation of the last one's retries.
        if (recheck) {
          recheck = false;
          refitSig = '';
          verifyFit(sig);
        }
        return;
      }
    }
    recheck = false;
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
  /** How far the last fit had to shrink, 0–1. Half of the verdict; `verifyFit`
   *  adds the other half (does it actually fit) and reports both, once. */
  let lastFitScale = 1;
  // ── A VERDICT IS ONLY AS GOOD AS THE MOMENT IT WAS TAKEN ───────────────────
  //
  // `verifyFit` — and therefore `report` — is reachable ONLY from `runFit`, and
  // `runFit` early-returns whenever `fitSig()` is unchanged. So the question "is
  // what I painted actually fitting?" was gated behind the same signature that
  // decides whether to re-FIT, and that signature cannot see this defect: it is
  // built from the stage's integer clientWidth/clientHeight and each layer's
  // stored `w,h,size` plus its text length, and not one of those moves when the
  // painted content outgrows its box.
  //
  // So a render that fitted cleanly while its pane was still settling reported
  // `clipped: false`, the box then went over, and nothing ever looked again. That
  // is the console the lead measured at 2000x1175 with `Romans 8:28` on air: 207px
  // of content in a 174px box inside `.mon.prog`, the first line sliced through
  // the middle, and neither warning anywhere in `document.body.innerText`. The
  // instrument was not computing the wrong answer — it had stopped being asked.
  //
  // Two triggers re-take a settled verdict, and neither costs anything per frame:
  //   · a RESIZE, which is the event that says the geometry moved underneath it.
  //     It already called `scheduleFit`, and `runFit` already threw it away when
  //     the rounded signature had not changed — which is exactly a pane settling
  //     by less than a pixel;
  //   · ONE late re-look after a verdict settles, for a layout that resolves a
  //     beat after the frame `verifyFit` samples. One per signature, never a
  //     loop: polling would mean a forced reflow every frame, which is the
  //     regression the fit gating exists to prevent (a countdown at 4 Hz, a
  //     Library grid of a dozen renders).
  const LATE_CHECK_MS = 250;
  let recheck = false;
  let lateDoneFor = '';
  let lateTimer = 0;
  function fitBoxes() {
    if (!stageEl) return [];
    return [
      ...(layered
        ? stageEl.querySelectorAll('.ltext')
        : stageEl.querySelectorAll('.slide .content, .slide .ticker-label')),
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
  /**
   * THE ONE REPORT, at the point where the verdict is complete.
   *
   * `scale` is how far the fit had to shrink; `clipped` is whether the words
   * actually fit afterwards. They are different failures and only the first was
   * ever reported — `legible` answered "did we shrink past 45%?" and nothing
   * answered "does it fit". `Nocturne · Lyrics` settled at 8.5 of a designed 8.5
   * — `scale: 1.0`, `legible: true` — over a 104px box holding 174px of words.
   * The most reassuring possible report over the worst possible outcome, which
   * is rule 35 in the place rule 37 was supposed to be watching.
   */
  function report(clipped) {
    // Never throw: this runs on the page that is ON THE WALL, and a listener that
    // breaks must not take the render with it.
    if (onFit) {
      try {
        onFit({ scale: lastFitScale, legible: lastFitScale >= MIN_LEGIBLE_SCALE, clipped });
      } catch {
        /* a report about legibility may not cost legibility */
      }
    }
  }
  /** A frame for the browser to lay out, then one to look at what it did. */
  function nextFrame(fn) {
    if (typeof requestAnimationFrame === 'undefined') return void setTimeout(fn, 32);
    requestAnimationFrame(() => requestAnimationFrame(fn));
  }
  /**
   * DID THE FIT ACTUALLY FIT? — and the check has to be taken where it can tell.
   *
   * `overflowing()` used to be sampled in the SAME synchronous frame as the
   * binary search, which is the one moment it cannot see a discrepancy that
   * materialises a frame later. Rule 42's comment above says exactly this about
   * the font case; the blindness is general, and the font half is INERT in the
   * shipped product anyway — Relay bundles no webfont at all (`app.css` line 11:
   * zero network, so no `fonts.googleapis` links; there is no `@font-face` and no
   * `.woff` in `src/`), and an empty FontFaceSet makes `document.fonts.check()`
   * answer true for every family. So `overflowing()` is the only real signal
   * there is, and it was being read too early to be one.
   *
   * Measured by the lead in the Templates gallery: `Nocturne · Lyrics` at
   * `data-base="8.5"` computing to exactly 8.5cqw — a size the search genuinely
   * reached and genuinely measured as fitting (from `lo=0.4, hi=22` the mids are
   * 11.2, 5.8, then exactly 8.5) — painting 174px inside a 104px `overflow:hidden`
   * box, and staying there. The measurement and the paint disagreed, and nothing
   * looked again. Under `container-type: size` a fit writes `font-size` in `cqw`
   * and reads `scrollHeight` back inside one loop; a container still settling, a
   * container-query length resolved in a later pass and a system-font
   * substitution all leave that same signature. This component cannot tell them
   * apart and does not need to: every one of them is invisible to a same-frame
   * sample and visible to a next-frame one.
   *
   * The bound is UNCHANGED (`MAX_REFIT`), so rule 37's genuinely-unfittable
   * passage is still shrunk, still shown, and now actually reported.
   */
  function verifyFit(sig) {
    if (sig !== refitSig) {
      refitSig = sig;
      refitTries = 0;
    }
    nextFrame(() => {
      // Something has re-fitted since; that pass owns the verdict, not this one.
      if (sig !== lastFitSig) return;
      const stale = !fittedWithTheRealFont();
      const over = overflowing();
      if (!needsRefit({ fontReady: !stale, overflowing: over, tries: refitTries, max: MAX_REFIT })) {
        report(over);
        // One late re-look per settled verdict, for a layout that resolves a beat
        // after this frame. The guard is what stops it becoming a poll: the
        // second time this signature settles, `lateDoneFor` already names it and
        // nothing more is scheduled.
        if (lateDoneFor !== sig) {
          lateDoneFor = sig;
          clearTimeout(lateTimer);
          lateTimer = setTimeout(() => {
            refitSig = '';
            verifyFit(sig);
          }, LATE_CHECK_MS);
        }
        return;
      }
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
    });
  }
  function scheduleFit() {
    // ── THE REPAIR CANNOT WAIT FOR A FRAME, AND THE MEASUREMENT MUST ─────────
    //
    // Two different costs, so two different schedules. The fit READS layout in a
    // loop, which forces synchronous reflow, so it is deferred to one animation
    // frame — that is what the whole gate below exists for. Putting back a size
    // Svelte has just overwritten is pure style writes, and deferring THAT by a
    // frame is what makes it visible.
    //
    // `afterUpdate` runs in the same task as the DOM update that clobbered the
    // size (see `reapplyFitted`), before the browser paints; the next animation
    // frame is one paint later. Measured at 1920×1080 on `Timer · Titled` with
    // the repair left in the frame: sampling `getComputedStyle().fontSize` every
    // animation frame for four seconds caught the DECLARED base on 4 of 482
    // frames — one frame per countdown tick, a 65px flash on a 38px label four
    // times a second, for the whole pre-service countdown. Repairing here instead
    // closes that window by construction rather than by winning a race.
    //
    // It touches no Svelte state and reads no layout, so it cannot re-enter the
    // scheduler (rule 1) and cannot cost a reflow.
    reapplyFitted();
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
      // A resize genuinely changes the fit — and when it does NOT change the
      // (integer, rounded) signature, it still changes the verdict, so it also
      // asks for that to be re-taken. See `recheck`.
      ro = new ResizeObserver(() => {
        recheck = true;
        // THE RAIL IS RE-MEASURED TOO. A resize moves the box a programme layer
        // sits in, and nothing else on this path would notice: `afterUpdate`
        // only runs when Svelte has something to update, and a window that got
        // narrower is not a state change. Without this the rail keeps the
        // capacity it was born with for the life of the page.
        measureProgramme();
        scheduleFit();
      });
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
    clearTimeout(lateTimer);
    if (fitRaf && typeof cancelAnimationFrame !== 'undefined') cancelAnimationFrame(fitRaf);
  });

  // --- Video sound: the operator's chosen speaker, applied to the clip ---
  // The <video> lives inside a {#key} block, so a new clip is a NEW element:
  // routing is (re)applied on each element's loadedmetadata, not once on mount.
  // Layer mode, full-frame media and the legacy band are mutually exclusive
  // branches, so at most one <video> is ever mounted — one binding covers all.
  let videoEl;
  /**
   * ONE REPORTER FOR ALL THREE `<video>` BRANCHES.
   *
   * Layer, legacy band and legacy region each mount their own element and each
   * binds the same `videoEl`, so the handlers go on all three or the report is
   * silently missing from two thirds of the templates in the product — the twin
   * -door failure this repository has recorded four times.
   */
  /**
   * THE REPLAY THIS ELEMENT HAS ALREADY ACTED ON.
   *
   * Replay is an event, not a state, and the wire carries it as a counter for that
   * reason. This remembers the last number acted on so a re-render, a re-connect,
   * or a retained frame replayed on hello cannot start the clip again — a screen
   * that rejoined mid-clip would otherwise jump to the beginning because the frame
   * it was handed still names a replay from ten minutes ago.
   */
  /** The replay and scrub epochs this element has already acted on (RG-221). */
  let actedTransport = null;
  // THE BACKDROP IS NOT THE CLIP, and its `loop` is deliberately left alone above.
  // A standing background is room furniture that plays behind whatever is fired
  // over it; the transport belongs to the video the operator put up, and a Pause
  // that stopped the church's backdrop would be a control reaching past what it
  // says it does.
  // Ask, then report. The asking is `applyMediaTransport`, which is shared with
  // the preacher's own page — see RG-214, where the two surfaces disagreed about
  // what Pause means. The reporting stays here, because the BEAT is what says
  // whether the clip is actually moving and only this page sends one.
  $: if (videoEl && mediaTransport) {
    actedTransport = applyMediaTransport(videoEl, mediaTransport, actedTransport);
    reportMedia();
  }

  /**
   * PULL THIS PLAYER BACK TO WHERE THE CLIP SHOULD BE (RG-220).
   *
   * Called on the events the player already fires, so there is no timer of its
   * own: the clip reports its position several times a second while it plays and
   * that is exactly when the question is worth asking.
   *
   * The baseline is Relay's clock (`content.media_started_at` against
   * `Date.now() + hostOffsetMs`), never a leader screen — a leader would make
   * the whole wall follow whichever browser buffered worst. `syncSeek` owns
   * every refusal, including the held clip and the one past its end.
   */
  const syncMedia = () => {
    const el = videoEl;
    if (!el) return;
    const want = syncSeek({
      startedAt: content?.media_started_at,
      now: Date.now() + (hostOffsetMs || 0),
      duration: el.duration,
      position: el.currentTime,
      paused: !!mediaTransport?.paused,
      looping: !!el.loop,
    });
    if (want == null) return;
    try {
      el.currentTime = want;
    } catch {
      /* a video with no metadata cannot be seeked; the next event will. */
    }
  };

  const reportMedia = () => {
    if (still) return;
    // HOW LONG IS LEFT (RG-280), read on the events the player already fires so
    // there is no timer of its own. `clipRemainingMs` answers null for every
    // pre-knowledge state — a zero reads as "it has finished" about a clip that
    // has not started.
    clipLeft = stageClip ? clipRemainingMs(videoEl) : null;
    syncMedia();
    if (!onMedia) return;
    const el = videoEl;
    const dur = el ? el.duration * 1000 : NaN;
    if (!el || !Number.isFinite(dur) || dur <= 0) {
      onMedia(null);
      return;
    }
    onMedia({
      pos_ms: Math.max(0, Math.round(el.currentTime * 1000)),
      dur_ms: Math.round(dur),
      paused: !!el.paused,
    });
  };
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
    style.refColor || (layout.lowerThird ? style.verseColor || '#1c1224' : style.accent || DEFAULT_ACCENT);

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
    if (!f) return DEFAULT_FAMILY;
    return f.startsWith('var(') ? f : `${f}, system-ui, sans-serif`;
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
  // THE BAND IS THE ONE BACKGROUND THAT MAY NOT FALL THROUGH TO `transparent`.
  //
  // `panelOn` is false for a lower third BY CONSTRUCTION (`&& !layout.lowerThird`),
  // so this expression resolved to the string `transparent` for every keyed
  // template — and it is written as an INLINE style on `.content`, which always
  // beats the `.slide.lower-third .content { background: var(--accent) }` rule
  // that is supposed to paint the band. Measured on a rendered
  // `output.html?template_id=3`: computed background `rgba(0, 0, 0, 0)` with
  // `--accent` resolved to a real colour and ignored.
  //
  // So every lower third — the template family whose entire purpose is to be
  // readable over a picture Relay does not control — shipped with nothing behind
  // the words, over a live camera, on the stream and the ATEM where nobody at the
  // desk is watching. Three of the six seeded ones are dark type on a light band
  // and were effectively invisible.
  //
  // It was silent to every instrument: `legibility.js` answers `unknown` for a
  // transparent ground rather than failing it, and the fit reports a healthy
  // scale because the type fits its box perfectly well.
  //
  // The band answers for itself here rather than relying on a stylesheet rule an
  // inline style outranks — one home for the value, which is what `tickerBg`
  // twenty lines below already does for the same reason.
  $: panelBg = bandMode
    ? style.accent || 'rgba(0,0,0,0.82)'
    : panelOn
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
  /**
   * WHAT A LAYER'S FIT DEFAULTS TO WHEN ITS TEMPLATE DOES NOT SAY.
   *
   * It was `'both'` for everything, and `'both'` GROWS a short string until it
   * fills its box. So every declared `size` in the shipped shelf was advisory and
   * the shortest string in the template always won the most room — which on a
   * scripture slide is always the citation.
   *
   * Measured at 1920x1080 before this: `High Visibility` put the verse at 108.3px
   * against a designed 161.3px, and `Romans 8:28` at 126.5px against a designed
   * 88.3px. The reference rendered 17% LARGER than the scripture, in the same
   * white, on the template the shelf file itself describes as "the answer to a lit
   * room". Five of the eight shelf templates inverted the hierarchy this way, and
   * it directly contradicts REBRAND §4's "reference beneath, right-aligned,
   * tracked, small".
   *
   * A LABEL NEVER GROWS. A reference, a translation and a static caption are
   * subordinate by definition, so they shrink to fit and no further. The verse and
   * the lyric still take the room they can — that part was right.
   *
   * Fixed here rather than by writing `"fit":"shrink"` into eight JSON entries and
   * five starters, so a template authored next year inherits it.
   */
  const LABEL_BINDS = new Set(['reference', 'translation']);
  const defaultFit = (L) => (LABEL_BINDS.has(L?.bind) || L?.type === 'static' ? 'shrink' : 'both');

  $: bandMode = !!layout.lowerThird;
  // THE BAND ONLY EXISTS WHERE THERE ARE WORDS. Drawn unconditionally it painted
  // a coloured strip across the bottom of a full-frame photo that had nothing
  // written in it — a bar over someone's picture for no reason.
  $: bandHasWords = !!(content?.text || (hasRef && !bandMode));
  // ── A COUNTDOWN NEVER REACHES A KEYED SCREEN ───────────────────────────────
  //
  // The reason is unchanged and is the transparency law's: a keyed channel is
  // composited over a live camera in OBS or on an ATEM, so every pixel it fills
  // that it did not have to fill takes the preacher off the stream — and nobody
  // in the building can see it happen. A clock ticking across the shot belongs
  // on the lobby screen and the main screen, not over the person talking.
  //
  // WHAT MOVED IS THE PREDICATE, AND IT IS THE ONE THIS REPOSITORY HAD ALREADY
  // CORRECTED ONCE. This asked `bandMode` — `layout.lowerThird` — which is the
  // check `layers.js::isKeyedTemplate` exists because of: a LAYER-MODEL lower
  // third carries no such flag (its band is a shape layer), so the flag answers
  // `false` for exactly the templates the rule is about. Blackout and the
  // transparency law were migrated to `isKeyedTemplate` when that was found; the
  // countdown was left behind, and ten of the forty shelf looks are keyed,
  // declare `countdown` in `shows`, carry no timer layer, and painted
  // `Service begins in 4:43` full-frame over the camera.
  //
  // AN ABSENT TEMPLATE IS NOT A KEYED ONE. `isKeyedTemplate(null)` and
  // `isKeyedTemplate({})` both answer `true` — correctly, for the question
  // blackout asks, since nothing paints a whole frame when there is no template
  // at all. Answering the COUNTDOWN question from that would make a preview
  // surface that has not resolved a template yet (`Live.svelte`'s slide cells
  // and the Planner inspector both pass `?? {}`) silently refuse to draw the
  // clock. So the refusal needs a template to have been given first, and only
  // then asks the shared question. The §82 camera plate is unaffected: it is
  // drawn by the previewing SURFACE, never by this component, and it already
  // decides from `isKeyedTemplate` itself.
  //
  // `bandMode` goes back to being about band LAYOUT and nothing else.
  $: templateGiven = !!(template && (template.layout || template.style));
  $: keyedOut = templateGiven && isKeyedTemplate(template);
  $: countdownAllowed = !!countdownTo && !keyedOut;

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
    const f = style.verseFont;
    if (!f) return DEFAULT_FAMILY;
    if (f.startsWith('var(')) return f; // a legacy token supplies its own fallback
    return `${f}, system-ui, sans-serif`;
  })();

  // THE SLIDE TRANSITION (docs/REBRAND.md §8). A CUT unless the template or its
  // style asks for something else, because that is what an operator asked for
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
  // console surface the picker with no per-surface wiring. `Output.svelte` passes
  // the prop explicitly instead, because a congregation screen must apply an
  // override only when CONTENT arrives: see the snapshot comment there.
  export let transitionOverride = undefined;
  $: activeOverride = transitionOverride === undefined ? $liveTransition : transitionOverride;
  $: resolvedTransition = resolveTransition(style, activeOverride);
  $: transitionMode = resolvedTransition.mode;
  $: transitionMs = transitionDuration(resolvedTransition.mode, resolvedTransition.ms, reduceMotion);
  const reduceMotion =
    typeof window !== 'undefined' && typeof window.matchMedia === 'function'
      ? window.matchMedia('(prefers-reduced-motion: reduce)').matches
      : false;
  /**
   * DOES THIS LAYER ACTUALLY CRAWL? — and the answer is not `L.scroll` alone.
   *
   * A crawl shows a notice longer than its band by moving it past a window, so
   * the words that do not fit are shown LATER. Under `prefers-reduced-motion` the
   * animation is off and later never comes, and the box is still `white-space:
   * nowrap; overflow: hidden` — so the notice is simply cut where the band ends,
   * with nothing shown and nothing said. Measured at 1920×1080 on the shipped
   * `Scroll · Banner` with a 185-character notice and reduced motion on:
   * `clientWidth 1651` against `scrollWidth 4299`, so **2648px — 62% of the
   * notice — was never on the screen at all**, while `onFit` reported
   * `{ scale: 1, legible: true, clipped: false }`. A fit that reports a success
   * over a notice with two thirds of it missing is rule 37 and rule 35 at once.
   *
   * DECISIONS §88 already had to distort the shelf around this: `High Visibility ·
   * Announcement` deliberately does not scroll, *"because a crawl that stops under
   * `prefers-reduced-motion` silently truncates a notice, and a family built for a
   * low-vision or vestibular reader cannot ship the one member most likely to cut
   * text off screen."* That reasoning is right and it should not have needed a
   * carve-out: what a crawl that cannot move should become is an ordinary text
   * layer. It wraps, the fitter measures it like every other layer, it shrinks if
   * it must, and it REPORTS — which is what rule 37 asks for and is strictly
   * better for a congregation than two thirds of a notice that never arrives.
   *
   * ONE FACT, READ IN BOTH PLACES. The class, the markup and `fitLayers`'
   * `lscroll` skip all follow from this single answer, so the renderer cannot
   * paint a still crawl while the fitter still believes it is moving — which is
   * the shape of every "guarantee kept on one of two doors" bug in this file.
   */
  const crawls = (L) => !!L?.scroll && !reduceMotion;
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
  // A RUNNING STAGE TIMER TICKS THIS CLOCK TOO — and a HELD one does not.
  //
  // The rail exists with no content on screen, so the countdown's own condition
  // could not reach it. The exclusion of held rows is the same discipline the
  // line above keeps: a timer firing four times a second to recompute a number
  // that cannot change is the wrong thing on a machine that is also decoding
  // speech.
  $: programmeTicking = progSet.some((t) => !countdownIsPaused(t));
  $: if ((countdownTo && !countdownHeld) || programmeTicking) startClock();
  else stopClock();
  function startClock() {
    if (cdTimer) return;
    now = typeof Date !== 'undefined' ? Date.now() + (hostOffsetMs || 0) : 0;
    cdTimer = setInterval(() => (now = Date.now() + (hostOffsetMs || 0)), 250);
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
  // ── THE PROGRAMME RAIL ──────────────────────────────────────────────────────
  //
  // `now` is the same 250ms clock the countdown ticks on, so the two figures on
  // one screen can never be a quarter-second apart from each other. Before the
  // clock has started it is 0, and a rail dated to the epoch would read as every
  // timer having run out decades ago — hence the fallback.
  // THE ALERT, trimmed once. A message of spaces is not a message, and both
  // surfaces that can hand one over already trim - this is the renderer refusing
  // to paint a full-bleed red panel over a string nobody typed.
  $: stageAlert = (stageMessage || '').trim();
  // Clamped, because a rail that took half a projector would be a clock
  // standing in front of the words it is there to time.
  $: railPct = Math.min(30, RAIL_BASE_PCT * (Number(timerScale) || 1));
  /**
   * DOES THIS TEMPLATE DECLARE ANYWHERE FOR A QUIET WORD TO GO?
   *
   * Found by reading the operator's own template rather than by reasoning:
   * `Stage · Reading` has Background, Reading, Reference, Clock and Elapsed, and
   * no `stage_message` layer. A quiet send that painted only into that layer
   * would be swallowed on the one screen this was asked for — the silence RG-156
   * was about, arriving by a different door. So the strip below renders exactly
   * when there is nowhere declared, and a template that DOES declare a place
   * keeps its designer's placement.
   */
  $: hasMessageLayer = stackLayers.some(
    ({ L }) => L?.bind === 'stage_message' && L?.visible !== false,
  );
  $: progSet = Array.isArray(programme) ? programme : [];
  $: progNow = now || (typeof Date !== 'undefined' ? Date.now() : 0);
  $: progRows = programmeRows(progSet, progNow);
  $: progCh = programmeCh(progRows);
  // ── THE CLIP STANDS IN FOR THE CLOCK (RG-288) ──────────────────────────────
  //
  // *"When a Media is Playing I want you to Cover the Stage timer with the clip
  // countdown... and return the timer back when the media is cleared or done."*
  //
  // `clipCoversTimer` owns the rule and says at the line why it asks for time
  // REMAINING rather than for a figure: `clipRemainingMs` answers 0 for a clip
  // that has finished and is still mounted, and a cover keyed on "is there a
  // figure" would hold the rail off a preacher's screen for the rest of the
  // service behind a dead 0:00.
  $: clipCovers = clipCoversTimer({ stage: stageClip, still, remainingMs: clipLeft });
  // ── AND WHO TAKES THE SCREEN WHEN NOTHING HAS BEEN FIRED TO IT (RG-285) ────
  //
  // The three content facts go straight to `stageresting.js` through
  // `stageFill`, so a `timer` verdict here is exactly the phone's `programme`
  // verdict. `progRows` is empty on any screen that is not a stage
  // (`Output.svelte::shownProgramme`), and `stageMessage` is refused by any
  // channel whose role is not `stage` — so the gate is the ROLE, as it already
  // was for the rail itself, and no new surface can be enlarged by this.
  $: fillWhat = stageFill({
    reading: !!(content?.text || content?.reference),
    slide: !!content?.media_url,
    countdown: content?.countdown_to != null,
    timers: progRows.length,
    // A note only. An ALERT is already the whole screen and flashes
    // (DECISIONS §116); giving it a share of a layout would make it smaller.
    message: !!stageAlert && !stageUrgent && !hasMessageLayer,
    clipCovers,
  });
  $: fillGeo = fillGeometry(fillWhat);
  // ONE rail fills, never all of them. A template with two `programme` layers
  // draws two rails, and two rails each told to take the whole frame is one rail
  // painted over another — so the first VISIBLE one takes it and the rest are
  // not drawn while it does.
  $: fillRailId = fillGeo.timer
    ? (programmeLayers.find(({ L }) => L.visible !== false)?.L?.id ?? null)
    : null;
  /** The box a filling part is given, in the same percent `boxStyle` draws in. */
  const fillStyle = (box) =>
    `left:0%; top:${box.top}%; width:100%; height:${box.height}%;`;
  // ── AND THE OVERFLOW RULE MEASURES THE BOX, NOT THE WINDOW ──────────────────
  //
  // `Stage.svelte` reads `innerWidth` and says at the line why it is allowed to:
  // its rail spans the frame, and measuring the box there would mean a forced
  // layout on the one page whose job is to be still. Neither half of that holds
  // here. A template renders inside a REGION and a region is not the viewport —
  // the same rail may be 1600px across on a wall-sized stage display and 300px
  // across in a corner of a composite, and a window measurement would give both
  // the same fourteen cells. So each rail is measured, once per update, in
  // `afterUpdate` (never in a reactive block — rule 1).
  //
  // A width of 0 is what an element reports before it has been laid out, and
  // `programmeCapacity` reads that as unmeasured rather than as a box one cell
  // wide. That is the honest reading of an absence and it is stated there, once.
  //
  // AND ITS HEIGHT, for a reason that cost a sliced clock to find. `.lprog` is
  // `container-type: inline-size`, which contains the INLINE axis and nothing
  // else — so a `cqh` inside it does not measure the rail, it measures the small
  // viewport. The cap that was meant to stop a figure growing taller than the box
  // it sits in resolved to 670px on a 76px rail and did nothing at all, and the
  // digits took the clamp ceiling and were cut off along the bottom by the
  // `overflow: hidden` that was supposed to be the last resort.
  //
  // `Stage.svelte` does not have this bug and the difference is instructive: its
  // rail caps against `--progmax`, a `dvh` figure it sets itself, precisely
  // because its own container is `inline-size` too (`:1719-1722`). A measured
  // pixel height is the same answer without a second unit to reason about.
  //
  // An unmeasured height is 0 and reads as UNMEASURED, never as a rail of no
  // height — the same honest reading of an absence that `programmeCapacity`
  // gives a width of 0, stated there once and followed here.
  let progEls = [];
  let progW = [];
  let progH = [];
  let progHeadH = [];
  // The default rail's own three (RG-224). Separate rather than an extra slot in
  // the arrays above, because those are indexed by LAYER and this rail has no
  // layer — an index into a list of layers for a thing that is not one is how a
  // measurement ends up describing the wrong box.
  let defaultProgEl = null;
  let defaultProgW = 0;
  let defaultProgH = 0;
  /**
   * A rail's CONTENT height — what a cell inside it actually has to work with.
   *
   * `clientHeight` is the padding box, and `programmeRoom` is asked about the
   * content box. The two were the same number for as long as no rail had any
   * padding, which is exactly how this kind of thing hides.
   */
  const innerHeightOf = (el) => {
    if (!el) return 0;
    const cs = typeof getComputedStyle === 'function' ? getComputedStyle(el) : null;
    const pad = cs ? (parseFloat(cs.paddingTop) || 0) + (parseFloat(cs.paddingBottom) || 0) : 0;
    return Math.max(0, (el.clientHeight | 0) - Math.round(pad));
  };
  /** The row gap a cell puts between its head and its figure, in px. */
  const gapUnder = (cell) => {
    if (!cell || typeof getComputedStyle !== 'function') return 0;
    const g = parseFloat(getComputedStyle(cell).rowGap);
    return Number.isFinite(g) ? Math.ceil(g) : 0;
  };
  function measureProgramme() {
    let moved = false;
    for (let i = 0; i < progEls.length; i += 1) {
      const w = progEls[i] ? progEls[i].clientWidth | 0 : 0;
      // THE ROOM A CELL HAS, WHICH IS NOT THE RAIL'S `clientHeight` (RG-285).
      // `clientHeight` includes PADDING, and a filling rail has 2cqw of it —
      // 77px on a 1080p screen. Handing that to `programmeRoom` as the rail's
      // height is the same arithmetic error the comment below records, made one
      // box further out: the figure was capped at 462px inside a 409px cell and
      // `2:25` was cut through the middle along the bottom of a projector,
      // measured in a real engine at 1920x1080. An unpadded rail is unchanged,
      // because its padding is zero.
      const h = progEls[i] ? innerHeightOf(progEls[i]) : 0;
      // AND WHAT THE LABEL ABOVE THE FIGURE IS ACTUALLY TAKING. `--lp-h` alone
      // was not enough: capping the figure at a SHARE of the rail reserves
      // nothing for the head, and the two only fit at 16:9 by arithmetic
      // accident. `.lp-lbl` is `clamp(9px, …)`, so below a certain rail the
      // label stops shrinking while the figure keeps going, their sum passes
      // the rail height, and `overflow: hidden` cuts the digits through the
      // middle. Measured at 900x300: head 9.90 + gap 3.60 + figure 13.02 into
      // a 21.00 rail, and 42% of every digit gone.
      // AND THE GAP UNDER IT GOES WITH IT. `.lp-cell` puts `0.4cqw` between the
      // head and the figure, so the head's own share of the cell is the word
      // plus that gap — 8px at 1920, which is what tipped the filling rail over
      // its box. The comment above already names the gap in its measurement
      // (`head 9.90 + gap 3.60 + figure 13.02`) and then did not subtract it.
      const head = progEls[i] ? progEls[i].querySelector('.lp-head') : null;
      const hh = head
        ? Math.ceil(head.getBoundingClientRect().height) + gapUnder(head.parentElement)
        : 0;
      if (progW[i] !== w) {
        progW[i] = w;
        moved = true;
      }
      if (progH[i] !== h) {
        progH[i] = h;
        moved = true;
      }
      if (progHeadH[i] !== hh) {
        progHeadH[i] = hh;
        moved = true;
      }
    }
    // THE DEFAULT RAIL MEASURES ITSELF THE SAME WAY. It has no layer, so it is
    // not in the loop above; leaving it unmeasured would give it the `100px`
    // fallback for ever and size its digits against a box it is not in.
    if (defaultProgEl) {
      const w = defaultProgEl.clientWidth | 0;
      const h = innerHeightOf(defaultProgEl);
      if (defaultProgW !== w) {
        defaultProgW = w;
        moved = true;
      }
      if (defaultProgH !== h) {
        defaultProgH = h;
        moved = true;
      }
    }
    if (moved) {
      progW = progW;
      progH = progH;
      progHeadH = progHeadH;
    }
  }
  afterUpdate(measureProgramme);
  /**
   * The measured rail height, as a CSS declaration, or NOTHING when it has not
   * been measured yet.
   *
   * Nothing, and not `0px`, and the difference is the whole point. A custom
   * property set to zero IS set, so `var(--lp-h, 100px)` would never reach its
   * fallback and `min(clamp(12px, …), calc(0px * 0.62))` is `0px` — the figure
   * would be invisible on the first paint, before `afterUpdate` has run, and
   * appear a frame later. Measured in a real engine: setting `--lp-h: 0px` on a
   * painted rail takes the digits from 47.12px to 0px.
   *
   * Omitting the declaration lets the fallback do its job, which is the same
   * honest reading of an absence that `programmeCapacity` gives a width of 0.
   */
  const railHeightVar = (px) => (Number(px) > 0 ? `--lp-h:${Math.round(px)}px;` : '');
  /**
   * WHAT IS LEFT FOR THE FIGURE once the label above it has taken its share, and
   * whether the label may have a share at all.
   *
   * The cap used to be a fraction of the rail, and a fraction reserves nothing.
   * It happened to hold at 16:9 because the label was still scaling there; it
   * stopped holding the moment `.lp-lbl`'s `clamp(9px, …)` floor bit, which is
   * every rail shorter than about 36px — a small composite region, or any output
   * below 1024x576. The digits were then cut through the middle by the
   * `overflow: hidden` that is supposed to be the last resort, and what is left
   * of a sliced clock still reads as a valid time (RG-147).
   *
   * **Below the floor the LABEL stands down, not the figure**, and that follows
   * the rail's own law one rule up: the label takes what it can and ellipses,
   * the figure is never the thing that gets cut. A name with an unreadable clock
   * under it tells a preacher nothing; a clock with no name still tells him how
   * long is left, and the rail is only ever showing what the operator put on it.
   *
   * `MIN_FIGURE_PX` is the reading floor, not a taste: `.lp-val`'s own clamp
   * bottoms out at 12px, and a figure that has been squeezed under it has
   * already stopped being an instrument.
   */
  /**
   * The rail's own height, and what is left for the figure once the label above
   * it has taken its share. `programmeRoom` owns the arithmetic — it is a
   * decision, and decisions live in the pure module where they can be tested
   * against numbers rather than against a regex.
   */
  const railRoomVars = (railPx, headPx) => {
    const room = programmeRoom(railPx, headPx);
    if (!room) return '';
    return room.bare
      ? `--lp-h:${room.rail}px; --lp-bare:1;`
      : `--lp-h:${room.rail}px; --lp-room:${room.figure}px;`;
  }; $: remainingMs = countdownRemainingMs(content, now);
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
  // THE THIRD ARGUMENT IS A THRESHOLD SOMEBODY CHOSE FOR THIS COUNTDOWN, and it
  // rides on the content (`countdown_warn_ms`, projected from the timer registry in
  // `timers::project_both`). It was added to the rule in wave 3 and passed by none
  // of the three readers, so a figure anybody chose changed nothing anywhere
  // (RG-149(a)). The RANKING is still the rule's own and is not restated here:
  // chosen, else the configured default, else the tenth-of-span rule.
  $: countdownWarn =
    remainingMs != null &&
    countdownWarning(remainingMs, countdownTotalMs(content), content?.countdown_warn_ms);

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
  //
  // ── SITE 10 OF THE CONTENT-KIND SWEEP. NOTHING CHANGED HERE, AND WHY ────────
  //
  // A kind that varies in none of the five keyed fields does not re-key, so it
  // gets no transition and no refit — it paints into the box the last slide was
  // measured for. The timer registry adds no such kind. A congregation timer keys
  // on `countdownTo`, which is `countdown_to`, and the registry moves that field
  // on every action that changes what the clock says: a start, a re-aim, a hold
  // and a release all re-stamp `target_ms`. So putting a timer back after a
  // reading re-keys on both `reference` and `countdownTo`, and a ±1 re-keys on
  // `countdownTo` alone. A Stage Timer never reaches this component at all.
  //
  // `countdown_paused_ms` is deliberately NOT keyed, and that is a decision rather
  // than an omission: holding a countdown changes whether the digits move, not
  // which slide is up, and re-keying there would re-animate the one slide an
  // operator deliberately froze. Same reasoning as `now` being excluded below — a
  // ticking layer must not re-animate.
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
    // …and the Stage Message, which arrives on its own frame and moves on
    // no clock at all. Without this line the message paints when something else
    // happens to change — and, worse, does not go when it is CLEARED: the exact
    // freeze this block's comment describes, on the one screen a person is
    // reading mid-sermon. Caught by `stagemessage.test.js`, not by reasoning.
    void stageMessage;
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

  // WHAT ACTUALLY DRAWS — the whole stack, or the backdrop alone.
  //
  // With content on screen this IS `layerViews`, the same array by identity, so
  // the keyed `{#each}` below sees no change at all and a template with no
  // backdrop layer renders byte for byte what it rendered before this existed.
  //
  // With no content and a backdrop up, everything else is filtered OUT, and that
  // is not tidiness. The `{#if}` above states the rule this file has always kept:
  // a cleared wall shows NOTHING, because a band left over a live camera or a
  // shape left standing with no words in it is furniture on a congregation's
  // screen. A backdrop earns its place there because a church puts one up
  // deliberately and minutes before the first fire; an empty band does not.
  // ── THE PROGRAMME LAYERS COME OUT OF THE STACK ──────────────────────────────
  //
  // Two reasons, and the second is the one that matters.
  //
  // They are not TEXT: `layerText` answers '' for this bind (deliberately —
  // `layers.js::boundValue` says why), so left in the stack a programme layer
  // would draw an empty `.ltext` box and enter the fit loop measuring nothing.
  //
  // And they must draw when there is NO CONTENT. The gate above this stack
  // states a rule about a cleared wall — furniture on a congregation's screen
  // with nothing to say — and a Stage Timer is the case that rule is not about:
  // it runs during the notices, before the first verse of the service exists,
  // and it survives a panic control because `Stage.svelte` survives one
  // (DECISIONS §91: a panic control takes back every sentence anybody put on a
  // screen, and stops none of the clocks). The two stage surfaces have to agree
  // about that or a church with one of each gets two answers.
  $: programmeLayers = layerViews.filter(({ L }) => L.bind === 'programme');
  $: stackLayers = (content ? layerViews : layerViews.filter(({ L }) => L.type === 'backdrop'))
    .filter(({ L }) => L.bind !== 'programme');

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
    // ── ONE FLUSH PER ROUND, NOT ONE PER LAYER ───────────────────────────────
    //
    // What this search costs is not the arithmetic and not the reads — it is the
    // FLUSH. Every probe writes a `font-size` and then reads `scrollHeight` back,
    // and a read taken while a write is outstanding forces the browser to lay the
    // page out synchronously before it can answer. The reads after it are free,
    // because layout is clean again until the next write.
    //
    // So the cost of this function is the number of write→read TRANSITIONS, and
    // searching each layer to completion before starting the next one makes that
    // number `rounds × layers`. Measured on the shipped `High Visibility`
    // template (two text layers): 32 forced layouts per render, and Live mounts
    // one render per slide-grid cell — 1281 forced layouts to open a forty-slide
    // plan, in a single frame, before the operator has touched anything.
    //
    // Collecting every layer's next candidate, writing them ALL, then reading
    // them ALL makes one flush serve the whole render: `rounds`, whatever the
    // layer count. Each layer still walks its own bracket, with its own `lo` /
    // `top` / `best`, and lands on the same size it always did — only the
    // interleaving changes. A template with four text layers now costs what a
    // template with one costs.
    const jobs = [];
    stageEl.querySelectorAll('.ltext').forEach((box) => {
      const el = box.querySelector('.lfit');
      if (!el) return;
      const base = parseFloat(el.dataset.base || '5') || 5;
      const mode = el.dataset.fit || 'both';
      if (el.classList.contains('lscroll') || mode === 'none') {
        el.style.fontSize = `${base}cqw`;
        // A REFUSAL IS STILL AN ANSWER, and it has to be recorded like one. These
        // two never entered the search, so they never carried `data-sized` — and
        // `anythingUnfitted` therefore said "true" about them on every single
        // frame, which sent a template carrying a crawl or a `fit:'none'` layer
        // through the whole forced-reflow search four times a second for as long
        // as a countdown was on the wall beside it. The size the fitter answers
        // for a layer it declines to measure is that layer's declared base, so
        // saying so is truthful as well as free.
        el.dataset.sized = '1';
        box.dataset.fitted = String(base);
        return;
      }
      // 'shrink' caps growth at the configured size; 'both' allows growing to a
      // generous ceiling so a single short word fills the box.
      const hi = mode === 'shrink' ? base : Math.max(base, 22);
      jobs.push({ box, el, base, hi, lo: FIT_FLOOR_CQW, top: hi, best: FIT_FLOOR_CQW, probe: 0, done: false });
    });
    const write = (j, px) => {
      j.probe = px;
      j.el.style.fontSize = `${px}cqw`;
    };
    const fits = (j) =>
      j.box.scrollHeight <= j.box.clientHeight + 1 && j.box.scrollWidth <= j.box.clientWidth + 1;
    // ROUND ONE — THE CEILING IS AN ANSWER, not merely the top of a bracket.
    // When the words already fit at the largest size this layer is allowed, the
    // bisection can only ever creep back up towards it and stop one
    // ten-thousandth short. Asking the ceiling directly is the same answer
    // (fractionally the better one — it is the true supremum of the bracket) for
    // one flush instead of the whole ladder, and a short label in a wide box —
    // a reference line, a Stage Note, a name band — is the common case.
    for (const j of jobs) write(j, j.hi);
    for (const j of jobs) {
      if (fits(j)) {
        j.best = j.hi;
        j.done = true;
      }
    }
    // …then bisect whatever is left, in lockstep, until the bracket is narrower
    // than a difference anyone could see. `FIT_EPS_CQW` is a share of the
    // OUTPUT'S WIDTH, like every other size here, so the guarantee holds at any
    // resolution: 0.02cqw is 0.4px of font-size on a 1080p wall and 0.8px on a
    // 4K one. `best` is only ever assigned a size that MEASURED AS FITTING, so
    // stopping early can only leave the text very slightly smaller — never
    // overflowing. The hard bound is unchanged, so rule 37's genuinely
    // unfittable passage still terminates and is still reported.
    for (let i = 0; i < FIT_MAX_ROUNDS; i++) {
      const live = jobs.filter((j) => !j.done && j.top - j.lo > FIT_EPS_CQW);
      if (!live.length) break;
      for (const j of live) write(j, (j.lo + j.top) / 2);
      for (const j of live) {
        if (fits(j)) {
          j.best = j.probe;
          j.lo = j.probe;
        } else {
          j.top = j.probe;
        }
      }
    }
    for (const j of jobs) {
      j.el.style.fontSize = `${j.best}cqw`;
      // THIS SIZE OUTLIVES THIS ELEMENT. `{#key text}` will throw the element
      // away on the next tick or the next verse; the answer stays on the `.ltext`,
      // which is keyed by the layer's own id and survives. `reapplyFitted` hands
      // it to whatever element takes its place.
      j.el.dataset.sized = '1';
      j.box.dataset.fitted = String(j.best);
      // A box with nothing in it was not shrunk, it is EMPTY — a reference layer
      // on a lyric fire, a `next` line with no next. Reporting its ratio would
      // make Live shout "38% of the designed size" on an ordinary song.
      if ((j.el.textContent || '').trim()) worst = Math.min(worst, j.best / j.base);
    }
    // Handed on, not reported — see `fitText` and `report()`.
    lastFitScale = worst;
  }
  // Fit is driven by the unified scheduler above (runFit → fitLayers/fitText),
  // gated to prop-change + resize so countdown/clock ticks don't force reflow.
</script>

<div class="stage" bind:this={stageEl} style="--accent:{style.accent || DEFAULT_ACCENT};">
  <!-- NOTHING renders without content. "Clear all screens" (content → null) must
       remove EVERYTHING — the background, the lower-third band, every layer — not
       just the text. A background left painted after a clear, or a band left over
       a live camera after a blackout, is furniture on the congregation's wall (or
       the stream) with nothing to say. This persists across a content→content
       CROSSFADE (content stays non-null throughout); it only leaves on a real
       clear, or a blackout of a keyed channel.

       THE STANDING BACKGROUND IS THE ONE EXCEPTION, AND IT DOES NOT WEAKEN THE
       RULE — it restates it. A backdrop that only painted while something was
       fired would be a background you could not put up: a church sets one at the
       top of a service and the words arrive minutes later. So `backdropUp` opens
       this block too. What is NOT relaxed is the clear: `Output.svelte` drops
       `backdrop` on `output://clear` and `output://black` at BOTH doors, and the
       hub empties its retained slot at the same instant, so both halves of this
       condition go to nothing together and a cleared screen is as empty as it
       ever was. Pinned from the Rust side by
       `channels::tests::a_panic_control_takes_the_retained_background_with_it`
       and from here by `backdrop.test.js`.

       WITH NO CONTENT, ONLY THE BACKDROP DRAWS. `stackLayers` filters the rest
       out, and that is the half of this that would otherwise be a new bug of the
       exact kind the paragraph above is about: rendering the whole stack over a
       cleared wall would paint an empty band and an empty shape — furniture on a
       congregation's screen with nothing to say. -->
  {#if content || backdropUp}
  {#if layered}
    <!-- ══ LAYER MODE ══ free-form stack, drawn back-to-front. Media shows ONLY
         where a template includes a MEDIA layer (below), at that layer's z-order —
         so each screen opts into (or out of) media and controls what sits over or
         under it. A lower third with no media layer never shows the picture; a
         full-screen template with a media layer on top lets the picture fill it. -->
    <!-- ── THE SLIDE TRANSITION, ON THIS PATH TOO ─────────────────────────────
         `{#key slideKey}` + `in:slideIn` lived in the REGION branch only, so every
         layered template cut regardless of what its style or the
         operator's live override said — and layered is what everything new is.
         Same key, same `slideIn`, and the same already-resolved `transitionMode` /
         `transitionMs` pair the region branch reads — so the ranking of override
         over template (DECISIONS §84) is still done in exactly one place, above.
         ONE mechanism, not a second one for the other half of the renderer, which
         is how the console preview and the wall stay agreed.

         WHAT IS INSIDE THE KEY, and why it is not simply the whole stack. The
         region path keeps `bglayer` and the full-frame media element OUTSIDE its
         key and animates only `.slide` — the words and the band they sit in. This
         is that same division:

           · `background`, `media` and `backdrop` are FURNITURE and stay out. A
             wrapper around the whole `{#each}` would rebuild them on every fire,
             and rebuilding a `media` layer tears down its <video> and restarts the
             loop, mid-fire, on a congregation screen. `backdrop` is the strongest
             case of the three: it is meant to be the ONE thing on the wall that a
             fire does not touch, so a re-key on it would refetch the church's
             picture and flash it behind every verse of the service.
           · `region` stays out because it does not need help: the composite is a
             nested `<svelte:self>` with the same content and its own style, so it
             resolves and runs its own transition. Keying it here would remount a
             whole renderer per fire to duplicate an animation it already does.
           · `shape`, `band` and `text` are the slide, and they animate.

         A TICKING LAYER MUST NOT RE-ANIMATE. `slideKey` excludes `now` and
         `clockNow` deliberately, so a countdown redrawing four times a second sits
         still inside this key. The transition is hung here and NOT on the `{#key
         text}` below, which a clock rebuilds every second — that would fade the
         figure once a quarter second for the whole pre-service countdown.

         The panic controls do not pass through any of this. A clear drops `content`
         to null and the `{#if content}` above takes the whole stack away; a blackout
         is decided by the output page, not here. There is no `out:` transition
         anywhere in this file — `transitionoverride.test.js` asserts exactly that —
         so a clear and a blackout are instant at every duration the picker offers
         (rule 15, DECISIONS §20). An intro cannot delay a removal. -->
    {#each stackLayers as { L, text, box } (L.id)}
      {#if L.visible !== false}
        {#if L.type === 'background'}
          <div class="lbg" style="{boxStyle(L)} background:{bgPaint(L)}; opacity:{L.opacity == null ? 1 : L.opacity};"></div>
          {#if L.dim > 0}<div class="lbg ldim" style="{boxStyle(L)} opacity:{L.dim};"></div>{/if}
        {:else if L.type === 'backdrop'}
          <!-- THE STANDING BACKGROUND. The same elements a `media` layer paints
               with, deliberately, so a picture looks identical whichever of the two
               put it there — one set of CSS, one `object-fit` rule, one <video>
               configuration. What differs is the SOURCE and therefore the lifetime:
               this reads `backdrop`, which no content can touch, so it survives
               every fire until the operator or a panic control takes it away.

               At its own z-order, like every other layer: it is not pinned to the
               bottom, because every built-in ships an opaque `background` fill and
               a backdrop underneath one would never be seen. -->
          {#if backdrop?.media_url}
            <div class="lmediabox" style="{boxStyle(L)} border-radius:{L.radius || 0}cqw; opacity:{L.opacity == null ? 1 : L.opacity};">
              {#if backdrop.media_kind === 'video'}
                <!-- MUTED ALWAYS, unlike a fired video. A backdrop runs for the
                     whole service under everything else; sound from it would play
                     under the sermon, and `routeAudio` exists to give the ONE
                     fired video the house speakers. -->
                <!-- svelte-ignore a11y-media-has-caption -->
                <video class="lmediafill" src={backdrop.media_url} style="object-fit:{L.fit === 'contain' ? 'contain' : 'cover'};" autoplay loop muted playsinline></video>
              {:else}
                <img class="lmediafill" src={backdrop.media_url} style="object-fit:{L.fit === 'contain' ? 'contain' : 'cover'};" alt="" />
              {/if}
            </div>
            <!-- A WASH OVER THE PICTURE, and only over the picture: it is emitted
                 here, at this layer's box and this layer's place in the stack, so
                 everything drawn ABOVE the backdrop is unaffected. Same shape as
                 the `background` layer's dim above. -->
            {#if L.dim > 0}<div class="lbg ldim" style="{boxStyle(L)} opacity:{L.dim};"></div>{/if}
          {/if}
        {:else if L.type === 'media'}
          <!-- Paints only when a picture/video is on screen and this screen shows
               media; empty otherwise, so the layer is invisible on a text-only
               cue (or when the screen opts out of media). -->
          {#if content?.media_url && allowMedia}
            <div class="lmediabox" style="{boxStyle(L)} border-radius:{L.radius || 0}cqw; opacity:{L.opacity == null ? 1 : L.opacity};">
              {#if content.media_kind === 'video'}
                <!-- svelte-ignore a11y-media-has-caption -->
                <video class="lmediafill" src={still ? posterUrl(content.media_url) : content.media_url} style="object-fit:{L.fit === 'contain' ? 'contain' : 'cover'};" bind:this={videoEl} on:error={() => mediaFailed('video', content.media_url)} on:loadeddata={mediaLoaded} autoplay={!still} loop={!still && !!mediaTransport?.loop} preload={still ? "metadata" : "auto"} muted={!audio} playsinline on:loadedmetadata={() => { routeAudio(); reportMedia(); }} on:timeupdate={reportMedia} on:pause={reportMedia} on:play={reportMedia} on:ended={reportMedia}></video>
              {:else}
                <img class="lmediafill" src={content.media_url} style="object-fit:{L.fit === 'contain' ? 'contain' : 'cover'};" alt="" on:error={() => mediaFailed('image', content.media_url)} on:load={mediaLoaded} />
              {/if}
            </div>
          {/if}
        {:else if L.type === 'shape'}
          {#key slideKey}
            <div class="lshape" style="{boxStyle(L)} {shapePaint(L)} border-radius:{L.radius || 0}cqw;"
              in:slideIn={{ mode: transitionMode, duration: transitionMs }}></div>
          {/key}
        {:else if L.type === 'band'}
          <!-- THE BAND (docs/REBRAND.md §4): a real element running from its own
               `top` to the bottom edge, inset by the side safe area. Its words are
               NOT its children — they are emitted beside it with boxes this band
               computed, so they take the one text path below. Its alpha is applied
               exactly (`shapePaint`); it has never been scaled by 0.9 here. -->
          {#key slideKey}
            <div class="lband" style="{boxStyle(box || L)} {shapePaint(L)} border-radius:{L.radius || 0}cqw;"
              in:slideIn={{ mode: transitionMode, duration: transitionMs }}></div>
          {/key}
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
                depth={depth + 1}
              />
            </div>
          {/if}
        {:else if !(showDefaultCountdown && (L.bind === 'verse' || L.bind === 'reference' || L.bind === 'translation'))}
          <!-- Verse/reference/translation layers are hidden during a default
               countdown (they carry no content then); a static or clock layer
               still shows. -->
          {#key slideKey}
          <div class="ltext" style="{boxStyle(box || L)} align-items:{vAlign(L.valign)};"
            in:slideIn={{ mode: transitionMode, duration: transitionMs }}>
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
                class:lscroll={crawls(L)}
                data-base={baseSize(L)}
                data-fit={L.fit || defaultFit(L)}
                style="font-size:{baseSize(L)}cqw; color:{L.color}; font-family:{fontFamOf(L.font)}; font-weight:{L.weight || 400}; text-align:{L.align}; text-transform:{L.transform || 'none'}; line-height:{L.lineHeight || 1.3}; letter-spacing:{(L.letterSpacing || 0)}em; text-shadow:{shadowOf(L.shadow)}; font-style:{L.italic ? 'italic' : 'normal'};">
                {#if crawls(L)}
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
          {/key}
        {/if}
      {/if}
    {/each}
    {#if showFullMedia}
      <!-- This screen shows media but has no media layer to place it → the picture
           fills the frame, drawn on TOP of the layers (foreground). Add a media
           layer to the template to position it instead. -->
      {#if content.media_kind === 'video'}
        <!-- svelte-ignore a11y-media-has-caption -->
        <video class="media" src={still ? posterUrl(content.media_url) : content.media_url} bind:this={videoEl} on:error={() => mediaFailed('video', content.media_url)} on:loadeddata={mediaLoaded} autoplay={!still} loop={!still && !!mediaTransport?.loop} preload={still ? "metadata" : "auto"} muted={!audio} playsinline on:loadedmetadata={() => { routeAudio(); reportMedia(); }} on:timeupdate={reportMedia} on:pause={reportMedia} on:play={reportMedia} on:ended={reportMedia}></video>
      {:else}
        <img class="media" src={content.media_url} alt="" on:error={() => mediaFailed('image', content.media_url)} on:load={mediaLoaded} />
      {/if}
    {/if}
    {#if showDefaultCountdown}
      <!-- No timer layer, but a countdown is on screen → a default centred label +
           MM:SS over the template's background/shape layers. Add a Timer layer to
           the template to place it instead. -->
      <div class="cd-default">
        {#if content.reference && !countdownDone}
          <div class="ltext cd-line cd-ref" style="align-items:center;">
            <!-- `.reference` is kept alongside `.lfit` (not dropped): it is what
                 carries font-weight:600 on this label, and `.lfit`'s own CSS does
                 not restate it. -->
            <div class="lfit reference" data-base={refSize} data-fit="shrink" style="font-size:{refSize}cqw; {refStyle} text-align:center;">{content.reference}</div>
          </div>
        {/if}
        <div class="ltext cd-line cd-digits" style="align-items:center;">
          <!-- THE SIZE IS DECLARED AND THE BOX IS MEASURABLE. `data-base` is the
               designed size in cqw, so a countdown that has not been fitted yet
               paints at the size the template asked for rather than at the app's
               UI type (the same reasoning as the layer text path above). `shrink`
               caps growth at that size: a countdown must never grow to fill a
               box, because the digits change width every second and a growing
               clock jitters. -->
          <!-- `.countdown` is kept alongside `.lfit` (not dropped): it is what
               carries tabular-nums, weight, tight leading and single-line
               `white-space: nowrap` for the ticking digits, and what
               `.countdown.warn` needs below to paint the last-minute red pulse.
               None of that is restated inline. -->
          <!-- THE DIGITS ARE GROUPED, AND THE MARKUP IS ONE LINE ON PURPOSE.
               `countdownParts` splits the figure so the separator can be set back
               and each group can settle on its own (see `layers.js`). Svelte
               keeps the whitespace between sibling elements, so a newline in
               here would land INSIDE `.countdown` and survive the `.trim()` the
               transport's own test does on `textContent` — a control broken by a
               styling change. Hence one line, and a test that reads the text
               back. -->
          <div class="lfit countdown" data-base={verseSize * 2} data-fit="shrink" class:warn={countdownWarn}
            style="font-size:{verseSize * 2}cqw; margin-top:{refGap}cqw; color:{countdownWarn ? CD_WARN : verseColor}; text-align:center; text-shadow:{verseShadowCss};"
          >{#if countdownDone}{content.countdown_done || '0:00'}{:else}{#each countdownParts(countdownText) as p (p.k)}<span class:cd-sep={p.sep} class:cd-num={!p.sep}>{p.t}</span>{/each}{/if}</div>
        </div>
      </div>
    {/if}
  {:else if content.media_url && allowMedia}
    <!-- REGION (legacy, no layers): a fired picture/video fills the frame alone —
         these templates have no media layer to place it, so full-frame is the only
         sensible behaviour and keeps old templates working. -->
    {#if content.media_kind === 'video'}
      <!-- svelte-ignore a11y-media-has-caption -->
      <video class="media" src={still ? posterUrl(content.media_url) : content.media_url} bind:this={videoEl} on:error={() => mediaFailed('video', content.media_url)} on:loadeddata={mediaLoaded} autoplay={!still} loop={!still && !!mediaTransport?.loop} preload={still ? "metadata" : "auto"} muted={!audio} playsinline on:loadedmetadata={() => { routeAudio(); reportMedia(); }} on:timeupdate={reportMedia} on:pause={reportMedia} on:play={reportMedia} on:ended={reportMedia}></video>
    {:else}
      <img class="media" src={content.media_url} alt="" on:error={() => mediaFailed('image', content.media_url)} on:load={mediaLoaded} />
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
          <div class="ticker" class:still={reduceMotion} style="background:{tickerBg}; --tickdur:{tickerSecs}s;">
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
            class:cdbox={countdownTo && countdownAllowed}
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
              <!-- Grouped digits + a separable separator, one line of markup, for
                   the reasons written at the default-overlay branch above. -->
              <div class="verse countdown" class:warn={countdownWarn} style="font-size:{verseSize * 2}cqw; color:{countdownWarn ? CD_WARN : verseColor}; text-align:{verseAlign}; text-shadow:{verseShadowCss};"
              >{#if countdownDone}{content.countdown_done || '0:00'}{:else}{#each countdownParts(countdownText) as p (p.k)}<span class:cd-sep={p.sep} class:cd-num={!p.sep}>{p.t}</span>{/each}{/if}</div>
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

  <!-- == THE PROGRAMME RAIL == OUTSIDE THE CONTENT GATE, ON PURPOSE.
       A Stage Timer runs during the notices and survives a panic control
       (DECISIONS §91), so it cannot live inside a block whose rule is "a cleared
       wall shows NOTHING". It draws only where a template asks for it and only
       where the PAGE has handed rows over, which it does for a `stage`-role
       screen and for nothing else. -->
  <!-- AND THE CLIP MAY TAKE ITS PLACE (RG-288). Only ever on a stage screen,
       only for a VIDEO, and only while that video has time left to run — the
       rule and the reason are in `stagefill.js::clipCoversTimer`. -->
  {#if progRows.length && !clipCovers}
    {#each programmeLayers as { L }, i (L.id)}
      {#if L.visible !== false && (!fillGeo.timer || L.id === fillRailId)}
        {@const fills = fillGeo.timer && L.id === fillRailId}
        {@const cells = programmeCells(progRows, programmeCapacity(progW[i]))}
        <div
          class="lprog"
          class:overmedia={pictureBehind}
          class:fills
          bind:this={progEls[i]}
          style="{fills ? fillStyle(fillGeo.timer) : boxStyle(L)} --tmrs:{cells.length}; --tch:{progCh}; --lp-sz:{fills ? FILL_SIZE_CQW : railSize(L)}; {railRoomVars(progH[i], progHeadH[i])} color:{L.color || '#fff'}; font-family:{fontFamOf(L.font)}; opacity:{L.opacity == null ? 1 : L.opacity};"
          aria-label="Programme">
          {#each cells as t, j (j)}
            {#if t.more}
              <!-- THE RAIL SAYING WHAT IT COULD NOT SHOW. Not a timer, so it
                   carries no `data-timer-id` and nothing counts it as one. -->
              <div class="lp-cell lp-more"><span class="lp-val lp-msg">+{t.more} more</span></div>
            {:else}
              <div class="lp-cell" class:warn={t.warn} class:over={t.over} class:held={t.held} data-timer-id={t.id}>
                {#if t.label || t.state}
                  <span class="lp-head">
                    {#if t.label}<span class="lp-lbl">{t.label}</span>{/if}
                    <!-- `Held`, or `TIME UP` past zero (RG-286). One field, from
                         `timers.js::programmeRows`, so this rail and the phone's
                         cannot spell the same clock's state two ways. -->
                    {#if t.state}<span class="lp-state">{t.state}</span>{/if}
                  </span>
                {/if}
                <!-- ALWAYS A FIGURE, NEVER PROSE - `--tch` budgets this column
                     from the widest rendered string on the rail, so an
                     operator's done message here would size every column to its
                     own length. Those words land on the congregation countdown
                     instead. -->
                <span class="lp-val">{t.v}</span>
              </div>
            {/if}
          {/each}
        </div>
      {/if}
    {/each}
  {/if}

  {#if showDefaultProgramme && !clipCovers}
    <!-- NO PROGRAMME LAYER, BUT CLOCKS ARE RUNNING (RG-224). A rail along the
         foot, in the same shape a designed one takes, so an operator who adds
         the layer later gets the same thing in a place they chose. Add a
         Programme layer to the template to place it instead.

         The box is written out rather than taken from a layer, because there is
         no layer: full width, the bottom 8%, which is where every seeded stage
         template puts it. `--lp-room` is deliberately absent — nothing measured
         this rail — so `.lp-val` falls back to its `--lp-h` share, exactly as it
         does for a designed rail before the first measurement lands. -->
    {@const cells = programmeCells(progRows, programmeCapacity(defaultProgW))}
    <!-- ITS HEIGHT IS THE OPERATOR'S (RG-265). `RAIL_BASE_PCT` was a flat 8,
         which is 86 pixels on a 1080p projector read from ten metres — and the
         Normal / Large / Huge control the operator set for this screen reached
         the phone and stopped there. -->
    <!-- AND WITH NOTHING FIRED IT TAKES THE WHOLE SCREEN (RG-285), which is what
         `stage.html` has done since RG-244. `--lp-sz` is lifted with it: the
         Size field is a share of a DESIGNED box, and there is no designed box
         while the rail IS the screen. -->
    <div
      class="lprog lprog-default"
      class:overmedia={pictureBehind}
      class:fills={!!fillGeo.timer}
      bind:this={defaultProgEl}
      style="{fillGeo.timer ? `${fillStyle(fillGeo.timer)} --lp-sz:${FILL_SIZE_CQW};` : `left:0%; top:${100 - railPct}%; width:100%; height:${railPct}%;`} --tmrs:{cells.length}; --tch:{progCh}; {railHeightVar(defaultProgH)}"
      aria-label="Programme">
      {#each cells as t, j (j)}
        {#if t.more}
          <div class="lp-cell lp-more"><span class="lp-val lp-msg">+{t.more} more</span></div>
        {:else}
          <div class="lp-cell" class:warn={t.warn} class:over={t.over} class:held={t.held} data-timer-id={t.id}>
            {#if t.label || t.state}
              <span class="lp-head">
                {#if t.label}<span class="lp-lbl">{t.label}</span>{/if}
                {#if t.state}<span class="lp-state">{t.state}</span>{/if}
              </span>
            {/if}
            <span class="lp-val r-mono">{t.v}</span>
          </div>
        {/if}
      {/each}
    </div>
  {/if}

  <!-- == THE STAGE MESSAGE == THE WHOLE SCREEN, NOT A LAYER.
       Requirement 5: *an unmistakable flashing state: full-bleed or near-full-
       bleed, high contrast, sustained for the alert duration, visible to someone
       glancing up from a distance in bright light. A subtle tint or a small badge
       is not acceptable.*

       Rendered here rather than through a `stage_message` layer because a
       template designed before Stage Messages existed is exactly the screen this
       has to reach - an alert the designer had to opt into is an alert that is
       missing from the one screen nobody remembered to update. A template MAY
       still carry the layer; the panel covers it, which is the right order.

       It is outside the content gate for the same reason the rail is: a Stage
       Message over a cleared screen is still a Stage Message. It comes down when
       the page stops handing one over, which `Output.svelte` does on a role
       change and on both panic controls (DECISIONS §91). -->
  <!-- ══ HOW LONG IS LEFT OF THE CLIP (RG-280) ══ on a stage screen only.
       Frosted rather than filled, for the reason RG-212 and RG-256 both
       recorded: the clip is what the room is watching, and an opaque bar
       punched through it is a worse answer than a clock nobody can read. -->
  {#if stageClip && clipLeft != null}
    <div class="lclip" class:warn={clipWarn} role="status" aria-live="off">
      <span class="lclip-k">Clip</span>
      <span class="lclip-v">{formatCountdown(clipLeft)}</span>
    </div>
  {/if}
  {#if stageAlert && stageUrgent}
    <div class="lalert {alertStep(stageAlert)}" role="status" aria-live="assertive">
      <!-- SHOWN ONLY UNDER REDUCED MOTION (see the stylesheet). With the pulse
           running, the panel identifies itself by behaving like nothing else on a
           platform; without it, the label and the frame are what stop a flat red
           rectangle reading as part of the set. -->
      <span class="lalert-lbl">Stage Message</span>
      <span class="lalert-txt">{stageAlert}</span>
    </div>
  {/if}

  <!-- == A QUIET WORD WITH NOWHERE DECLARED TO GO ==
       Not an alarm and not nothing. A template that declares a `stage_message`
       layer paints the words where its designer put them; one that does not gets
       this strip, because a send that reports success and shows nothing is the
       failure RG-156 filed. It does not flash, does not fill the screen and does
       not cover the reading — that is the whole distinction the operator asked
       for (DECISIONS §116). -->
  <!-- AND WITH NOTHING ON THE SCREEN IT TAKES THE ROOM (RG-285), which is the
       phone's own answer: `stagemessage.js::messagePlacement` gives a note the
       reading's whole box, because *"a Stage Message is the desk speaking to ONE
       person in the middle of a sermon — it is never ambient"* (RG-239/RG-245).
       §116's line is untouched and is what the `reading`/`slide` half of
       `stageFill` keeps: over a reading this is still the modest foot strip,
       because a NOTE MAY NOT COVER THE READING. -->
  {#if stageAlert && !stageUrgent && !hasMessageLayer}
    <div
      class="lmsg"
      class:fills={!!fillGeo.message}
      style={fillGeo.message ? fillStyle(fillGeo.message) : ''}
      role="status"
      aria-live="polite">
      <!-- THE WORDS IN AN ELEMENT OF THEIR OWN (RG-268). A bare text node cannot
           be coloured or animated apart from the plate it sits on, and the
           distinction this whole path rests on is that a MESSAGE pulses its text
           while an ALARM flashes its panel. `Stage.svelte`'s `.bigmsg-v pulse`,
           on the surface that never had it. -->
      <span class="lmsg-v pulse">{stageAlert}</span>
    </div>
  {/if}
</div>

<style>
  /* The quiet strip's rules live beside the alarm's, under THE STAGE MESSAGE
     below, because the two are one decision (DECISIONS §116) and reading either
     of them alone is how they came to disagree. */
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
  /* == THE PROGRAMME RAIL ==================================================
     The Stage Timer set, inside whatever box the template's programme layer
     names. The rules are `Stage.svelte`'s `.progrow` family, restated for a box
     that is NOT the frame - which is the one real difference between the two
     surfaces and the reason the sizing reads `cqw` against this element rather
     than against the page.

     ITS OWN CONTAINER, so `cqw` below is a share of THE RAIL and not of the
     screen. A rail in a 300px corner of a composite and a rail across a
     wall-sized stage display then set their digits from their own width, which
     is the whole of requirement 2b's "measure the container, not the window". */
  .lprog {
    position: absolute;
    box-sizing: border-box;
    container-type: inline-size;
    display: flex;
    gap: 1cqw;
    align-items: stretch;
    /* CENTRED, the way the phone's `.progrow` has been since RG-248. The cells
       are `flex: 1 1 0` so they fill the rail and this decides nothing for them
       — it decides where the set sits when it does NOT fill, which is every rail
       carrying a `+N more` cell (`flex: 0 1 auto`). Left was the default's
       answer rather than anybody's. */
    justify-content: center;
    overflow: hidden;
    z-index: 3;
  }
  /* ══ THE RAIL OWNS THE SCREEN (RG-285) ══
     Only ever on a stage-role screen with nothing fired to it — the rule is
     `stagefill.js`, which is `stageresting.js` with two extra facts, so this and
     the preacher's phone answer the same question the same way.
     It is only the BOX that changes: the cells, the caps, the warning colour and
     the TIME UP flash are the rail's own and are untouched, which is what keeps
     a filled rail and a corner rail the same instrument at two sizes. */
  .lprog.fills {
    /* The gap is a share of the container, and the container is now the screen —
       1cqw of 1920 is 19px between two clocks, which reads as a seam. */
    gap: 3cqw;
    padding: 2cqw;
  }
  /* THE PLATE THE RAIL EARNS OVER A PICTURE (RG-212).
     A wash and a blur rather than a solid block: the clip is the thing the
     congregation is watching, and a rail that punches an opaque bar through it
     is a worse answer than a clock nobody can read. `backdrop-filter` is a
     progressive enhancement - where it is not supported the wash alone still
     lifts the digits off the picture, which is why the colour carries most of
     the contrast rather than the blur.
     NO COLOUR CHANGE: the digits keep the template's own ink, and a warning or
     an over-run keeps its own (rule 18). This adds a surface, never a meaning. */
  /* THE FALLBACK RAIL (RG-224). It borrows every rule `.lprog` has — it IS a
     `.lprog` — and adds only the two things a designed layer carries on itself:
     an ink, and enough of a backing that white digits are readable over whatever
     the template happens to paint at its foot. A designed layer states its own
     colour; this one has no designer to ask. */
  .lprog-default {
    color: #fff;
    background: color-mix(in srgb, #000 55%, transparent);
  }
  .lprog.overmedia {
    background: color-mix(in srgb, #000 62%, transparent);
    backdrop-filter: blur(6px);
    border-radius: 0.8cqw;
  }
  /* `min-width: 0` on the item, or a long label refuses to shrink and pushes the
     last timer off the end of a screen nobody is standing next to. */
  /* AND THE DIGITS SIT IN THE MIDDLE OF WHATEVER ROOM THE TIMER HAS (RG-289).
     The operator: *"aligh the text on the stage timer properly so it dosent just
     stay to one side"*. A column flex box leaves `align-items: stretch` and text
     at its default alignment, so a single timer printed its figure hard against
     the left edge of a cell that spans the rail — measured at 1920x1080 with the
     seeded Programme layer: the digits started 12px from the left of a 1896px
     cell and ended 1631px short of its right edge.
     `Stage.svelte`'s `.tmr` says the same thing at the same line and has since
     RG-248: *"left and top was an accident of the defaults rather than a
     decision"*. This is the other rail, two days behind. */
  .lp-cell {
    flex: 1 1 0;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    gap: 0.4cqw;
    overflow: hidden;
  }
  /* A LABEL-LESS TIMER IS DIGITS ALONE - the head is not rendered at all rather
     than rendered empty, so the cell closes up instead of leaving a gap the
     height of a word. The label takes what it can and ellipses; the state word is
     never the thing that gets cut. */
  .lp-head {
    display: flex;
    align-items: baseline;
    gap: 0.6cqw;
    min-width: 0;
  }
  .lp-lbl,
  .lp-state {
    font-weight: 700;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    line-height: 1.1;
    font-size: clamp(9px, calc(30cqw / var(--tmrs) / 10), 22px);
  }
  .lp-lbl {
    opacity: 0.62;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  /* HELD IS A REAL THIRD STATE AND IT READS AS ONE. It is not a colour from the
     law: amber means ON AIR, cyan means a guess, amethyst means rehearsal, and a
     clock somebody paused is none of those. It is the layer's own ink at full
     strength - the operator did this deliberately, and the preacher is entitled
     to know a clock has stopped rather than broken. */
  .lp-state {
    flex: 0 0 auto;
    opacity: 1;
  }
  /* THE RAIL IS TOO SHORT FOR BOTH, SO THE NAME GOES AND THE CLOCK STAYS.
     Set from `railRoomVars` once the measured head would leave the figure under
     its reading floor. `display: none` and not `visibility: hidden`, because the
     point is to give the height back. */
  .lprog[style*='--lp-bare'] .lp-head {
    display: none;
  }
  .lp-val {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    line-height: 1;
    /* The width a figure may take is its share of the rail divided by the
       characters IT ACTUALLY HAS. `--tch` is the row's own longest rendered
       figure (`programmeCh`), which is why `1:30:13` cannot be sliced into a
       shorter time that still reads as a valid one (RG-147), and `0.62` is the
       mono advance. Capped against the rail's own height so one timer on a wide
       box does not become taller than the box it is in.

       AND IT ELLIPSISES RATHER THAN SLICING. A fit that cannot report is rule
       37's defect; a sliced clock is a lie the one person reading it cannot
       detect, and an ellipsis says the figure did not fit instead of showing a
       shorter one that looks correct. */
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    /* WHAT THE DESIGNER ASKED FOR, CAPPED BY WHAT FITS (operator, 2026-09-21).
       `--lp-sz` is `L.size` converted into this container by `timers.js::railSize`
       — the Size field in the editor, which reached this layer and nothing else
       in the product until now.

       It is a `min()`, so the two caps that were the whole value before are still
       caps: the per-column budget (`--tch`, which is what stops `1:30:13` being
       sliced into a shorter time that still reads as valid — RG-147) and the
       rail's own height. A figure may SHRINK to fit its box; it may not grow past
       what was asked for. The 12px floor stays: below it nothing is readable from
       a platform anyway, and a figure that small is a box that is too small.

       THE 64px CEILING IS GONE, and it is why the operator reported the Size
       control as dead (RG-223). It was a literal, not a fit: on a 1920×1080
       screen with the seeded Programme layer and one timer, the per-column budget
       is 294px and the rail's room is 72px, so `64px` was the smallest of the
       three from about 3.3cqw upward — the dial moved for a turn and a half and
       then nothing happened, however far it went. `max()` keeps the floor and
       drops the ceiling; past that the rail's own HEIGHT binds, which is honest
       and visible, because the box is drawn in the editor. */
    font-size: min(
      calc(var(--lp-sz, 2.2) * 1cqw),
      max(12px, calc(92cqw / var(--tmrs) / var(--tch, 6) / 0.62)),
      var(--lp-room, calc(var(--lp-h, 100px) * 0.62))
    );
  }
  /* THE ONE PROSE CELL ON THIS RAIL, and it is the rail talking about itself.
     Digits sized for `MM:SS` would set `+3 more` at the size of a clock and clip
     it. Quiet, because it is bookkeeping about bookkeeping - but present,
     because a rail that silently drops half the programme is a rail nobody can
     tell from a complete one. */
  .lp-more {
    flex: 0 1 auto;
    justify-content: center;
  }
  .lp-msg {
    font-variant-numeric: normal;
    letter-spacing: 0;
    line-height: 1.15;
    opacity: 0.62;
    font-size: min(clamp(10px, calc(92cqw / var(--tmrs) / 11 / 0.5), 30px), var(--lp-room, calc(var(--lp-h, 100px) * 0.4)));
  }
  /* THE LAST MINUTE, ON THE PREACHER'S OWN PROGRAMME. The same red and the same
     rule as the stage page (`.tmr.warn .tval`), stated UNCONDITIONALLY and
     outside every motion query: a viewer who asked for no motion must still
     learn that the clock is running out. It is a claim about TIME and none of
     the three law colours - amber is ON AIR, cyan is a guess, amethyst is
     rehearsal. A timer that has run OUT wears the same red and counts upward;
     being over is the far end of the same claim, not a second colour. A HELD row
     wears neither: it is not running out, it is where the operator left it. */
  .lp-cell.warn .lp-val {
    color: #f4515b;
  }
  /* ── TIME UP, AND IT ASKS FOR ATTENTION (operator, 2026-09-21) ─────────────
     The congregation countdown has flashed on its warning since §7
     (`.countdown.warn`), and the preacher's own rail — the one clock a person is
     meant to ACT on — only ever changed colour. A red figure among red figures,
     on a dark stage screen at arm's length under stage lighting, was the weakest
     signal in the product pointed at the person with the least attention spare.

     THE SAME 2s CADENCE as `cdwarn`, deliberately: two clocks flashing at two
     rhythms in one room is two alarms, and an operator glancing between a wall
     and a monitor should not have to tell them apart by tempo.

     It marks `over`, not `warn` — see `timers.js::programmeRows` for why a flash
     that runs for the whole warning window is a flash nobody sees. */
  .lp-cell.over .lp-val {
    animation: lpover 2s ease-in-out infinite;
  }
  @keyframes lpover {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.45;
    }
  }

  /* == THE STAGE MESSAGE ===================================================
     Requirement 5. The presentation `Stage.svelte` gives the preacher's phone,
     on the template path, so a stage TV and a stage tablet in the same room say
     the same thing in the same way.

     NEAR-FULL-BLEED AND ABOVE EVERYTHING. `absolute` rather than the phone's
     `fixed`, because this renderer also draws the Templates editor preview and a
     console pane: `fixed` would escape the box and paint over the operator's
     own chrome. `.stage` is `position: absolute; inset: 0` on an output page, so
     on the screen this IS the screen. */
  /* ══ THE CLIP CLOCK ON A BIG STAGE SCREEN (RG-280) ══
     Same plate, same ink and the same warning threshold as the phone's
     `.clipplate` in `Stage.svelte`: a preacher who checks the tablet and then
     looks up at the platform monitor must not be told two different things.
     FROSTED, not filled - the clip is what the room is watching, and a solid
     bar punched through it is a worse answer than a clock nobody reads.
     Sized in cqw so it holds its proportion on a 24-inch monitor and a wall. */
  .lclip {
    position: absolute;
    left: 50%;
    bottom: 4cqh;
    transform: translateX(-50%);
    z-index: 55;
    display: inline-flex;
    align-items: baseline;
    gap: 2.2cqw;
    padding: 1.4cqh 2.6cqw;
    border-radius: 1.4cqw;
    background: rgba(8, 10, 14, 0.42);
    border: 1px solid rgba(255, 255, 255, 0.14);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    pointer-events: none;
  }
  .lclip-k {
    /* LITERALS, NOT CONSOLE TOKENS. `seal.test.js` holds the rule and it is the
       right one: this component also paints a congregation screen, where the
       operator's stylesheet does not exist and a `var(--f-mono)` resolves to
       nothing. The phone may name tokens because it is only ever the phone. */
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    line-height: 1.1;
    font-size: min(1.6cqw, 2.4cqh);
    color: rgba(242, 244, 248, 0.66);
  }
  .lclip-v {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    line-height: 1;
    font-size: min(4.4cqw, 7cqh);
    color: #f2f4f8;
  }
  /* The last 30 seconds. Red, and the same red the phone and the wall use -
     this is the only figure on a stage screen that is about to run out. */
  .lclip.warn { border-color: rgba(244, 81, 91, 0.5); }
  .lclip.warn .lclip-v { color: #f4515b; }
  .lalert {
    position: absolute;
    inset: 0;
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2cqh;
    padding: 4cqw;
    text-align: center;
    font-weight: 700;
    line-height: 1.15;
    color: #fff;
    text-shadow: 0 0.02em 0.06em rgba(0, 0, 0, 0.75);
    background: #c8121c;
    overflow: hidden;
  }
  /* THREE STEPS, AND THE FIRST IS docs/REBRAND.md §5's FIGURE UNCHANGED. A
     message an operator types in a hurry is longer than a phrase, and at 8.5cqw
     a three-sentence one runs off the bottom of a box that clips. The table is
     `stagealert.js`; there is no step here that it cannot answer, and
     `stagealerttemplate.test.js` holds both halves of that. */
  .lalert.xl .lalert-txt { font-size: 8.5cqw; }
  .lalert.lg .lalert-txt { font-size: 6cqw; }
  .lalert.md .lalert-txt { font-size: 4.2cqw; }
  .lalert-txt {
    max-width: 100%;
    overflow: hidden;
  }
  /* THE LABEL IS THE REDUCED-MOTION ANSWER'S FIRST HALF, so it is hidden while
     the pulse is running - the panel then looks exactly like the phone's, which
     is the point of building the same presentation twice. */
  .lalert-lbl {
    display: none;
    font-size: min(2.2cqw, 3cqh);
    letter-spacing: 0.3em;
    text-transform: uppercase;
    opacity: 0.9;
  }
  /* THE PULSE IS THE POINT: a platform is a bright place and a flat red panel
     reads as part of the set. Same colours and same cycle as the phone. */
  @media (prefers-reduced-motion: no-preference) {
    .lalert {
      animation: stagealert 1.4s ease-in-out infinite;
    }
  }
  @keyframes stagealert {
    0%, 100% { background: #c8121c; }
    50% { background: #7a0a11; }
  }
  /* == AND REDUCED MOTION GETS AN ANSWER, NOT THE PULSE'S ABSENCE ==========
     The phone has a `no-preference` branch and NO `reduce` counterpart, so under
     reduced motion its alert is a flat red panel - the exact thing its own
     stylesheet says the pulse exists to avoid. Falling silent is not a decision,
     it is the absence of one.

     So this branch answers with the two things a still image can carry that a
     red rectangle cannot: a hard-edged high-contrast hazard frame, which no
     stage set has, and the panel NAMING ITSELF. Sustained for as long as the
     message is up, unmistakable from across a platform, and not a brightness
     pulse under another word - nothing here animates, filters or fades. */
  @media (prefers-reduced-motion: reduce) {
    .lalert {
      border: 2.4cqh solid transparent;
      border-image: repeating-linear-gradient(
          135deg,
          #fff 0,
          #fff 2.2cqw,
          #1a0205 2.2cqw,
          #1a0205 4.4cqw
        )
        24;
    }
    .lalert-lbl {
      display: block;
    }
  }

  /* == THE QUIET STRIP — A WORD THAT IS SEEN, NOT SHOUTED (DECISIONS §116) ===
     The other half of the Stage Message, and it sits here rather than at the top
     of the stylesheet so that nobody can change one of the two renderings while
     reading only the other. That is how this one came to be the thing it is.

     RG-268, the operator with a screenshot of a stage TV: *"Stage message sent
     still not flashing catching attention ... its just showing a gray/white text
     which can easily be missed."* It was white on a black plate behind a
     hairline white border, which is the presentation of a caption, and a caption
     is the one thing a message from the desk must not read as.

     `Stage.svelte` had already answered this for the phone (RG-239): the caution
     ink, a rule down the edge, and a gentle pulse of the TEXT. This is that
     answer, on the surface that never got it — so the tablet and the TV in one
     room now say the same thing in the same way, which is the whole point of
     building the presentation twice.

     WHAT IT IS STILL NOT. It does not flash its PANEL and it does not take the
     screen; `.lalert` above does both, and that distinction is the operator's
     own (§116). A message pulses its text; an alarm flashes its panel.

     OCHRE, because rule 18 leaves it the only free ink: amber means ON AIR, cyan
     means the AI guessed, amethyst means a rehearsal, and red is the alarm's.
     None of those promises is true of a quiet word to a preacher.
     `colourlaw.test.js` carries `.lmsg` in its caution sweep. */
  .lmsg {
    position: absolute;
    left: 4cqw;
    right: 4cqw;
    bottom: 3cqh;
    z-index: 3;
    padding: 0.9cqh 1.4cqw;
    border-radius: 0.6cqw;
    background: rgba(0, 0, 0, 0.62);
    /* A RULE, NOT A HAIRLINE. Sized in `cqw` like everything else here, so it is
       the same share of the screen on a 24" monitor and on a projector. */
    border-left: 0.9cqw solid var(--v-caution, #c9a24a);
    box-shadow: 0 0 0 0.12cqw var(--v-caution-line, rgba(201, 162, 74, 0.42));
    font-size: 3.2cqw;
    line-height: 1.25;
    text-align: center;
    overflow: hidden;
  }
  /* THE RESTING COLOUR, and the answer when neither media query applies. A
     browser that reports no motion preference at all gets the coloured message
     rather than the grey one, which is the failure direction that matters. */
  .lmsg-v {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    font-weight: 700;
    color: var(--v-caution, #c9a24a);
  }
  /* THE PULSE, and the phone's cycle unchanged. Gentle on purpose: it has to be
     unmistakably alive from a platform without competing with the alarm, which
     is twice as fast and moves a whole red panel. */
  @media (prefers-reduced-motion: no-preference) {
    .lmsg-v.pulse {
      animation: stagemsg 2s ease-in-out infinite;
    }
  }
  @keyframes stagemsg {
    0%, 100% { color: #fff; }
    50% { color: var(--v-caution, #c9a24a); }
  }
  /* AND REDUCED MOTION GETS AN EQUIVALENT, NOT A QUIETER STATE: the words rest
     AT the caution ink rather than pulsing to it, so a viewer who asked for no
     animation still reads a coloured message rather than a plain one. Nothing
     here animates, filters or fades — a brightness pulse under another word
     would be the setting ignored. */
  @media (prefers-reduced-motion: reduce) {
    .lmsg-v {
      color: var(--v-caution, #c9a24a);
    }
  }
  /* ══ AND WITH NOTHING ON THE SCREEN, THE WORDS TAKE IT (RG-285) ══
     The phone's `large` form (`stagemessage.js`), on the surface that only ever
     had the strip. The box comes from `stagefill.js::fillGeometry` as an inline
     style, in the same percent every other box on this page is drawn in, so
     `right` and `bottom` are released here rather than left to the browser's
     over-constraint rule.

     THE INK IS UNCHANGED AND THAT IS DELIBERATE. `--v-caution` is what rule 18
     leaves a message that warns and promises nothing about a screen, and RG-268
     enumerated this exact selector in `colourlaw.test.js` beside the phone's.
     What the operator reported — that the word did not catch an eye — is
     answered by the ROOM and the SIZE, which is what the phone's own answer to
     the same complaint was. The ink is the one part of this request left open;
     the RG-285 row records why. */
  .lmsg.fills {
    right: auto;
    bottom: auto;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 1.6cqh;
    padding: 3cqh 5cqw;
    border-radius: 0;
    /* A rule down the whole side rather than beside a line of text: at this size
       the strip's 0.9cqw edge reads as a border on a panel, which is furniture. */
    border-left: 1.4cqw solid var(--v-caution, #c9a24a);
    box-shadow: none;
    background: rgba(0, 0, 0, 0.72);
    /* 8cqw against the strip's 3.2. The strip shares the foot with a reading and
       is capped by it; this has the screen. */
    font-size: 8cqw;
  }
  /* FOUR LINES, NOT TWO. The clamp is what stops a long message pushing a clock
     off the screen, and with the room to spare it may have more of them — the
     cap on what can arrive is `stagealert.js::ALERT_MAX`, which is 140
     characters, so four lines at this size cannot be reached by a message the
     backend will deliver and nothing is silently cut. */
  .lmsg.fills .lmsg-v {
    -webkit-line-clamp: 4;
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
    /* IT CLIPS. This set no `overflow` at all, so a countdown too big for its
       box did not slice — it painted straight over the template's own layers and
       off the edge of the screen. Clipping is what makes the fitter's verdict
       honest: `overflowing()` reads `scrollHeight > clientHeight`, which an
       unclipped box never reports. */
    overflow: hidden;
  }
  /* The two fit boxes inside it are flex children, not absolutely-positioned
     layers, so they override `.ltext`'s `position: absolute`. `min-height: 0`
     is what lets a flex child actually be shorter than its content — without it
     the box reports that everything fits, at any size. */
  .cd-default .cd-line {
    position: relative;
    display: flex;
    width: 100%;
    min-height: 0;
    overflow: hidden;
    justify-content: center;
  }
  .cd-default .cd-digits {
    flex: 1 1 auto;
  }
  .cd-default .cd-ref {
    flex: 0 0 auto;
    max-height: 25%;
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
    /* A GLOW RATHER THAN NOTHING. Somebody who has asked for no animation has not
       asked to be left out of the one clock they are meant to act on — the same
       trade the countdown above makes, on the screen where it matters more. */
    .lp-cell.over .lp-val {
      animation: none;
      text-shadow: 0 0 0.3em rgba(244, 81, 91, 0.9);
    }
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
  /* ── THE BOX A COUNTDOWN IS FITTED AGAINST MAY NOT BE SIZED BY THE COUNTDOWN ──
     RG-141. `.content` above declares no width and no height: it is a flex item
     with caps, so it is shrink-to-fit and its box IS its text. That is right for
     a verse — prose wraps, so a long passage grows to the 90%/92% caps and then
     genuinely overflows them, which is a real signal the fitter can act on — and
     it is wrong for a countdown, which is one `nowrap` line with `line-height:
     1.05`, a leading deliberately TIGHTER than the face's own line box. The glyph
     box is therefore a fixed FRACTION taller than the box measured around it, at
     every size, and `fitOne`'s stop condition is an absolute one pixel.

     A loop whose overflow scales with the thing it is adjusting, against a
     tolerance that does not, can only terminate by shrinking until the residue
     rounds under a pixel. Measured at 1920x1080 on a legacy region row declaring
     `verseSize 5` — so a countdown designed at 10cqw, 192px — the residue went
     12px, 10, 8, 5, 3, 2 as the scale went 1, 0.8, 0.6, 0.4, 0.3, 0.2, and the
     loop stopped at 0.135: a 59x35px blob in the middle of an otherwise empty
     1920x1080 screen, the digits at 26px and the label at 6px. `onFit` correctly
     reported `legible: false`, which the audit could not confirm and which is the
     one part of this that was already working.

     It is rule 37 in its purest form — the loop had no notion of failure because
     it had no notion of the BOX — and the repair is the box, not the tolerance.
     Giving the countdown a definite rectangle makes `clientHeight` independent of
     the type, so the search terminates on the true fit: at 1920x1080 the declared
     192px now fits at scale 1 and paints at 192px. A template whose designer
     genuinely asked for more than the frame still shrinks, still shows, and still
     reports, exactly as rule 37 requires.

     90%/92% are `.content`'s own caps, restated as sizes rather than limits, so
     the countdown occupies the same budget every other kind already had and
     nothing about the layout moves. This is also what the LAYER branch has always
     had for free: `.ltext` and `.cd-default`'s lines are percentage boxes, which
     is why a layered countdown never showed this. Same guarantee, reached the same
     way, on the second door. */
  .content.cdbox {
    width: 90%;
    height: 92%;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
  /* Text contrast panel — a plate behind the words for a bright background. The
     padding gives the plate room around the text; it collapses to nothing when
     the panel is off (background transparent, radius still set but invisible). */
  .content.panel {
    padding: 3.5cqw 4.5cqw;
    max-width: 82%;
    /* IT CLIPS, LIKE `.content` DOES. This was `overflow: visible`, which took
       away the clip the unpanelled box has — and with it the only signal
       `overflowing()` reads. The panel is the mode an operator picks for a
       bright background, which is where legibility is already hardest, so it
       was the worst box in the component to have left unmeasured. The padding
       still gives the plate room; what it no longer does is let the words leave
       the plate. Kept explicit rather than relying on `.content`'s own
       `overflow: hidden` falling through: belt-and-braces against this exact
       rule regressing to `visible` again, right next to the history explaining
       why that would be wrong. */
    overflow: hidden;
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
    /* NO TRACKING ON A TABULAR FIGURE, and the zero is stated rather than left
       out so the reasoning has somewhere to live. This was 0.01em, a prose-scale
       value, on text that is never prose: `tabular-nums` already gives every
       figure the same advance, sized to the widest digit, so tracking on top of
       it is spacing applied twice. It also costs CENTRING — CSS adds the track
       after the LAST glyph as well, so a centred figure sits half a track left
       of centre, which at 192px on an otherwise empty screen is a visible
       offset. And on a `nowrap` line that is fitted in BOTH dimensions, every
       pixel of width is paid back by the fitter as a smaller figure. */
    letter-spacing: 0;
    /* The digits are one unbreakable line; keep them on one line so the fitter
       scales them down instead of letting them wrap mid-number. */
    white-space: nowrap;
  }
  /* THE SEPARATOR IS SET BACK, THE DIGITS ARE NOT.
     At 110-192px a colon is two solid dots carrying the mass of a pair of digit
     stems, parked in the middle of the figure; at full weight it reads as a
     third glyph and the four digits read as one block. Setting it back groups
     the figure into minutes and seconds, which is the reading somebody makes at
     a glance from the back of a room. It is safe to reduce in a way a digit
     never would be: the separator carries no information — `4 59` reads as
     4:59 — so the contrast that matters is untouched. */
  .countdown .cd-sep {
    opacity: 0.7;
    font-weight: 600;
  }
  /* A CHANGED GROUP SETTLES RATHER THAN SNAPPING.
     The keyed `{#each}` in the markup rebuilds only the group whose digits moved
     (`layers.js::countdownParts`), and a fresh element restarts this animation —
     no JS timing loop, and no `{#key}` around the element the fitter has sized.

     OPACITY ONLY, and that is the guarantee rather than a preference. Rule 37 and
     RG-141 both rest on `fitOne` stopping on `scrollHeight`/`scrollWidth`, and a
     TRANSFORM on a descendant contributes to a parent's scrollable overflow — so
     even a purely decorative scale could push a fitted countdown into another
     shrink round, four times a second, on the page that is on the wall. Opacity
     cannot move a box, cannot change a measured dimension, and is composited off
     the main thread. The fit gating (`fitSig`'s `countdownTo ? 1 : 0`) is
     untouched: this animates an element the fitter never measures. */
  @media (prefers-reduced-motion: no-preference) {
    .countdown .cd-num {
      animation: cdsettle 200ms ease-out;
    }
  }
  @keyframes cdsettle {
    from {
      opacity: 0.3;
    }
    to {
      opacity: 1;
    }
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
    /* THE SHARE LIVES HERE, ONLY HERE. A `flex: 0 0 auto; white-space: nowrap`
       item sizes its own box to its content and clips nothing on its own —
       only the parent `.ticker`'s `overflow: hidden` did, which clips the
       whole band, not the label. Without a cap of its own, `clientWidth`
       always equals `scrollWidth` and `fitTicker`'s shrink loop is measuring
       a box that can never report itself as overflowing. `max-width: 45%`
       (against the flex container, i.e. the band) makes the budget a fact a
       real browser enforces; `fitTicker` reads `clientWidth` off THIS rule
       rather than recomputing the share in JS, so the two cannot drift apart. */
    flex: 0 0 auto;
    max-width: 45%;
    overflow: hidden;
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
  /* ── A CRAWL THAT CANNOT CRAWL IS NOT A CRAWL ─────────────────────────────
     This block used to read *"the crawl stops and the notice sits static, still
     readable"*, and the second half of that was not true. Stopping the animation
     inside a `white-space: nowrap; overflow: hidden` track shows the notice's
     first bandful and discards the rest — and `text-overflow: ellipsis` did not
     even mark the cut, because the track's inline content is a single
     `inline-block` child rather than the text itself, so there was no truncated
     line for it to apply to. See `crawls()` for the measured layer-mode figure and
     for DECISIONS §88, which had to carve a family member out of the shelf over
     exactly this.

     A still notice is SHRUNK TO THE BAND IT WAS GIVEN and the result reaches
     `onFit` through `fitText`'s `worst`, so a notice too long for its band is
     shown small and REPORTED, never cut in silence. The band keeps its geometry
     and the notice gives way, not the other way round: letting the text wrap was
     tried first and measured, and it grew this band from 115px to 236px and
     lifted its top edge from y965 to y844 — a legacy footer band silently
     becoming a fifth of the wall, over a camera, because somebody's operating
     system prefers less motion. A notice at 32% of its designed size is small;
     one that has redrawn the screen behind it is a different template. The
     layer-mode crawl has no such conflict, because `.ltext` is a real percentage
     box: it wraps INSIDE the box its designer drew, which is the same lesson
     `.content.cdbox` above records.

     The class is set from the same `reduceMotion` flag the fitter reads, so the
     stylesheet and the measurement cannot disagree about whether this notice is
     moving. */
  @media (prefers-reduced-motion: reduce) {
    .ticker-run {
      animation: none;
      padding-left: 0;
    }
  }
  .ticker.still .ticker-run {
    animation: none;
    padding-left: 0;
  }
</style>
