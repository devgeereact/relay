<script>
  // ONE renderer for both the fullscreen output (Output.svelte) and the editor
  // preview (Templates.svelte) — guarantees WYSIWYG: what you save is exactly
  // what shows. Sizes are in `cqw` (container-query width units) so the same
  // template scales identically whether the container is a full screen or a
  // small preview box.
  import { fade } from 'svelte/transition';
  import { afterUpdate, onMount, onDestroy } from 'svelte';
  import { applySink, getAudioOutput, onAudioOutputChange } from './audioOutput.js';

  export let template = {};
  export let content = null; // { reference, text, translation }
  // Sound is OPT-IN per surface. This same renderer draws the Templates editor
  // preview, and editing a template must not blast video audio across the room —
  // so only a real output surface passes audio={true}.
  export let audio = false;

  $: layout = template?.layout ?? {};
  $: style = template?.style ?? {};
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
    // scrollHeight > clientHeight means content is overflowing its capped box.
    while (box.scrollHeight > box.clientHeight + 1 && guard < 40) {
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
  afterUpdate(fitText);
  let ro;
  onMount(() => {
    if (typeof ResizeObserver !== 'undefined' && stageEl) {
      ro = new ResizeObserver(() => fitText());
      ro.observe(stageEl);
    }
  });
  onDestroy(() => ro?.disconnect());

  // --- Video sound: the operator's chosen speaker, applied to the clip ---
  // The <video> lives inside {#key slideKey}, so a new clip is a NEW element:
  // routing is (re)applied on each element's loadedmetadata, not once on mount.
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

  // On a lower-third band the accent IS the background, so the reference uses
  // the verse (readable) color; elsewhere the accent tints the reference.
  $: refColor = layout.lowerThird ? style.verseColor || '#1c1224' : style.accent || 'var(--amber)';

  $: show = (r) => layout.regions?.includes(r);

  // The lower-third band is a TEMPLATE choice (configured in the editor), not a
  // content choice — like ProPresenter's "Lower 3rd Lyrics" / "Lower 3rd
  // Scripture" templates. So lyrics on a lower-third template render IN the band
  // (bottom, centered by the template's alignment), never floating mid-screen.
  $: hasRef = !!content?.reference;
  $: bandMode = !!layout.lowerThird;

  // Background can be a color/gradient (style.background) OR an uploaded image
  // (style.bgImage, a data URL) rendered cover. An image wins when present. The
  // background lives on the stage, so it persists while slides crossfade.
  $: bg = style.bgImage
    ? `url("${style.bgImage}") center / cover no-repeat`
    : style.background || 'transparent';

  // Alignment is configured per template (defaults centre). Lyrics inherit it —
  // the default lower-third template is centred, matching ProPresenter.
  $: verseAlign = style.verseAlign || layout.align || 'center';
  $: refAlign = style.refAlign || layout.align || 'center';

  // Slide transition (ProPresenter-style dissolve). Duration is template config
  // (style.transitionMs, default 250ms); reduced-motion users get an instant cut.
  const reduced =
    typeof window !== 'undefined' &&
    window.matchMedia &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches;
  $: dur = reduced ? 0 : Math.max(0, Number(style.transitionMs ?? 250));

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
</script>

<div class="stage" bind:this={stageEl} style="background:{bg}; --accent:{style.accent || 'var(--amber)'};">
  {#if content}
    {#key slideKey}
      <div class="slide" class:lower-third={bandMode} transition:fade={{ duration: dur }}>
        {#if content.media_url}
          {#if content.media_kind === 'video'}
            <!-- svelte-ignore a11y-media-has-caption -->
            <video
              class="media"
              src={content.media_url}
              bind:this={videoEl}
              autoplay
              loop
              muted={!audio}
              playsinline
              on:loadedmetadata={routeAudio}
            ></video>
          {:else}
            <img class="media" src={content.media_url} alt="" />
          {/if}
        {/if}
        <div
          class="content"
          style="text-align:{layout.align || 'center'}; font-family:{style.font || 'var(--f-serif)'};"
        >
          {#if countdownTo}
            <!-- Countdown: label + live MM:SS, styled by the template. At zero
                 only the done message shows (the "begins in" label is dropped so
                 it never reads "Service begins in Welcome"). -->
            {#if content.reference && !countdownDone}
              <div class="reference" style="font-size:{refSize}cqw; color:{refColor}; text-align:{refAlign};">{content.reference}</div>
            {/if}
            <div class="verse countdown" style="font-size:{verseSize * 2}cqw; color:{style.verseColor || '#f4e4c8'}; text-align:{verseAlign};">
              {countdownDone ? (content.countdown_done || '0:00') : countdownText}
            </div>
          {:else if refFirst}
            {#if show('reference') && content.reference}
              <div class="reference" style="font-size:{refSize}cqw; color:{refColor}; text-align:{refAlign};">{content.reference}</div>
            {/if}
            {#if show('verse_text') && content.text}
              <div class="verse" style="font-size:{verseSize}cqw; color:{style.verseColor || '#f4e4c8'}; text-align:{verseAlign};">{content.text}</div>
            {/if}
          {:else}
            {#if show('verse_text') && content.text}
              <div class="verse" style="font-size:{verseSize}cqw; color:{style.verseColor || '#f4e4c8'}; text-align:{verseAlign};">{#if hasRef}“{content.text}”{:else}{content.text}{/if}</div>
            {/if}
            {#if show('reference') && content.reference}
              <div class="reference" style="font-size:{refSize}cqw; color:{refColor}; font-style:{style.italicRef ? 'italic' : 'normal'}; text-align:{refAlign};">{content.reference}</div>
            {/if}
          {/if}
        </div>
      </div>
    {/key}
  {/if}
</div>

<style>
  .stage {
    position: absolute;
    inset: 0;
    container-type: size;
    overflow: hidden;
  }
  /* One slide layer. Absolute so an outgoing and incoming slide overlap during
     the crossfade instead of pushing each other around. */
  .slide {
    position: absolute;
    inset: 0;
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
  .slide.lower-third .content {
    max-width: 100%;
    width: 100%;
    background: var(--accent);
    padding: 2.4% 4%;
    box-sizing: border-box;
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
  }
  .reference {
    margin-top: 1.4%;
    font-weight: 600;
    overflow-wrap: anywhere;
  }
</style>
