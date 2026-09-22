<script>
  /**
   * THE CLIP'S OWN CONTROLS, IN THE SHELL — RG-254.
   *
   * The operator: *"I want to remove the Media functionality, like the play, loop
   * and all from the control and look for a suitable place to fix it... I just
   * want the Icons like the play icon, pause icon and all with the media
   * slider... also the level and all i dont think its needed"*.
   *
   * **Why it left the Controls card.** That card may never scroll, because
   * `Clear screens` may never sit behind an overflow edge (rule 15). It was
   * spending most of its room on a transport for a clip that is not playing for
   * most of a service: eight controls, a scrub, a level slider and a clock in a
   * 178px card beside the panic controls.
   *
   * **Why the shell and not the programme pane.** The pane is the obvious home —
   * it is the picture the operator is already watching, and it is where a scrub
   * bar means something. It is inside `views/Live.svelte`, and
   * `mediacontrols.test.js` holds RG-237's guarantee by asserting that file
   * carries no transport row. Moving here keeps that guarantee as written, and
   * keeps the controls reachable from Templates and Outputs, which is the half of
   * RG-237 that made this a shell control at all.
   *
   * **It costs nothing when nothing is playing.** Not a disabled bar and not an
   * empty rail: the component renders nothing. A strip that is always there is
   * the busy workspace the operator asked to avoid, and a disabled transport over
   * no clip is a control that owes a reason it cannot give.
   */
  import { onDestroy, onMount } from 'svelte';
  import {
    live,
    screenBlack,
    mediaTransport,
    setMediaTransport,
    channelHealth,
    stageMedia,
    sendStageMedia,
    listOutputChannels,
  } from './stores/capture.js';
  import { programmeScreen } from './channelroles.js';
  import { describeMediaClock, mediaIdFromUrl } from './mediaclock.js';
  import { clipPosition } from './clipposition.js';
  import { formatCountdown } from './countdown.js';
  import { humanError } from './errors.js';

  // WHICH SCREEN THE FIGURE IS ABOUT (RG-238), carried here whole rather than
  // restated. `programmeScreen` is the one rule for "the screen the operator is
  // watching", so the strip cannot quote one screen while Live's programme pane
  // shows another. Loaded once: a role reassigned mid-session is not picked up
  // until relaunch, and with no rows at all it answers nothing and the clock
  // falls back to the soonest screen, which is what it did before RG-238.
  let rows = [];
  onMount(async () => {
    try {
      rows = await listOutputChannels();
    } catch {
      rows = [];
    }
  });
  $: mainId = programmeScreen(rows).channel?.id ?? null;

  let err = '';
  let dragging = false;
  let dragMs = 0;
  let now = Date.now();
  let seenAt = Date.now();
  let lastKey = '';

  $: clipLive = !!$live?.media_url && !$screenBlack;
  $: clipMediaId = mediaIdFromUrl($live?.media_url);
  $: clipOnStage = $stageMedia != null && $stageMedia === clipMediaId;

  $: clock = describeMediaClock(
    Object.entries($channelHealth).map(([id, row]) => ({ ...row, id: Number(id) })),
    { mainId },
  );

  // WHEN THIS READING ARRIVED, which is the one fact `clipPosition` cannot
  // derive and the store does not carry. The poll replaces the whole health map
  // every two seconds whether or not anything moved, so the stamp is taken when
  // the FIGURES change rather than when the object does — otherwise every poll
  // would reset the age to zero and the bar would never extrapolate at all.
  $: {
    const key = `${clock.known}:${clock.positionMs}:${clock.durationMs}:${clock.paused}`;
    if (key !== lastKey) {
      lastKey = key;
      seenAt = Date.now();
    }
  }

  // A LOCAL TICK, not a second poll. Nothing is asked of the backend here: this
  // only moves the handle between the readings that already arrive, and it runs
  // only while a clip is up.
  let tick = null;
  $: if (clipLive && !tick) {
    tick = setInterval(() => (now = Date.now()), 200);
  } else if (!clipLive && tick) {
    clearInterval(tick);
    tick = null;
  }
  onDestroy(() => {
    if (tick) clearInterval(tick);
  });

  $: posMs = clipPosition({
    positionMs: clock.positionMs,
    durationMs: clock.durationMs,
    paused: clock.paused,
    known: clock.known,
    seenAt,
    now,
  });
  // THE HANDLE IS THE OPERATOR'S WHILE THEY ARE HOLDING IT. The old control
  // re-applied `value=` on every two-second poll, so a poll landing mid-drag
  // snapped the handle back to where the screen last said the clip was.
  $: shownMs = dragging ? dragMs : (posMs ?? 0);
  $: leftMs = clock.durationMs != null && posMs !== null ? Math.max(0, clock.durationMs - shownMs) : null;

  async function send(change) {
    err = '';
    try {
      await setMediaTransport(change);
    } catch (e) {
      err = humanError(e);
    }
  }

  const seek = (ms) => {
    dragging = false;
    send({ seekMs: Math.round(ms) });
  };

  async function toStage() {
    err = '';
    try {
      await sendStageMedia(clipOnStage ? null : clipMediaId);
    } catch (e) {
      err = humanError(e);
    }
  }
</script>

{#if clipLive}
  <!-- ══ THE CLIP STRIP ══ between the desk and the dock, only while a clip is
       on the screens. Icons and a bar: no words on the controls, because the
       four shapes are the ones every player in the world uses, and every one of
       them carries an `aria-label` for the operator who cannot see shapes. -->
  <section class="clipbar" aria-label="The clip on the screens">
    <span class="cb-name" title={$live?.media_url ?? ''}>{($live?.media_url ?? '').split('/').pop()}</span>

    <button
      class="cb-btn"
      aria-label={$mediaTransport.paused ? 'Let the clip run on every screen' : 'Hold the clip on every screen'}
      aria-pressed={$mediaTransport.paused}
      on:click={() => send({ paused: !$mediaTransport.paused })}>
      {#if $mediaTransport.paused}
        <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M7 4.5v15l13-7.5z" /></svg>
      {:else}
        <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><rect x="6" y="5" width="4" height="14" rx="1" /><rect x="14" y="5" width="4" height="14" rx="1" /></svg>
      {/if}
    </button>

    <button class="cb-btn" aria-label="Start the clip again from the beginning" on:click={() => send({ replay: true })}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 12a9 9 0 1 0 3-6.7" /><path d="M3 4v5h5" /></svg>
    </button>

    <button
      class="cb-btn"
      class:on={$mediaTransport.loop}
      aria-label="Repeat the clip when it ends"
      aria-pressed={$mediaTransport.loop}
      on:click={() => send({ loop: !$mediaTransport.loop })}>
      <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M17 2l4 4-4 4" /><path d="M3 11V9a4 4 0 0 1 4-4h14" /><path d="M7 22l-4-4 4-4" /><path d="M21 13v2a4 4 0 0 1-4 4H3" /></svg>
    </button>

    {#if posMs !== null}
      <!-- ABSENT, NEVER ZERO. A zero-length scrub bar looks usable and can move
           nothing; this one is simply not here while no screen is reporting. -->
      <span class="cb-t r-mono">{formatCountdown(shownMs)}</span>
      <input
        class="cb-scrub"
        class:held={clock.paused}
        type="range"
        min="0"
        max={clock.durationMs}
        step="100"
        value={shownMs}
        aria-label="Scrub the clip"
        aria-valuetext="{formatCountdown(shownMs)} of {formatCountdown(clock.durationMs)}"
        on:pointerdown={() => (dragging = true)}
        on:input={(e) => (dragMs = Number(e.target.value))}
        on:change={(e) => seek(Number(e.target.value))} />
      <span class="cb-t r-mono">&minus;{formatCountdown(leftMs ?? 0)}</span>
    {:else}
      <span class="cb-none">No screen is reporting this clip</span>
    {/if}

    <button
      class="cb-btn wide"
      class:on={clipOnStage}
      disabled={clipMediaId == null}
      aria-label="Put this on the preacher's screen as well"
      aria-pressed={clipOnStage}
      title={clipMediaId == null
        ? 'A picture Relay ships cannot be sent on its own'
        : clipOnStage
          ? "Take it off the preacher's screen"
          : "Put this on the preacher's screen as well"}
      on:click={toStage}>
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><rect x="2" y="4" width="20" height="13" rx="2" /><path d="M8 21h8" /><path d="M12 17v4" /></svg>
      <span>{clipOnStage ? 'On stage' : 'To stage'}</span>
    </button>

    {#if err}<span class="cb-err" role="alert">{err}</span>{/if}
  </section>
{/if}

<style>
  /* A ROW, NOT A CARD. It sits between the desk and the dock and is 44px, which
     is the touch floor and also the height at which it reads as a rule across
     the shell rather than another panel to take in.

     NO LAW COLOUR. Amber means ON AIR, cyan means the AI is guessing, amethyst
     means rehearsal — and a transport is none of those. The ON AIR claim beside
     it belongs to the screens and is made by the status bar, not by this. */
  .clipbar {
    flex: 0 0 auto;
    box-sizing: border-box;
    height: 44px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 12px;
    background: var(--v-bg);
    border-top: 1px solid var(--v-500);
  }
  .cb-name {
    flex: 0 0 auto;
    max-width: 190px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--v-fs-b1);
    color: var(--v-dim);
  }
  .cb-btn {
    flex: 0 0 auto;
    min-width: 34px;
    height: 34px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0;
    cursor: pointer;
    border-radius: var(--r-sm, 8px);
    background: var(--v-surf2);
    border: 1px solid var(--v-500);
    color: var(--v-txt);
  }
  .cb-btn.wide { padding: 0 11px; font-family: var(--f-body); font-size: var(--v-fs-b1); font-weight: 600; }
  .cb-btn.on { background: var(--v-sel-soft); border-color: var(--v-sel-line); color: var(--v-sel); }
  .cb-btn:disabled { opacity: var(--s-off, .45); cursor: default; }
  .cb-btn:focus-visible { outline: 2px solid var(--v-sel); outline-offset: 2px; }
  .cb-scrub { flex: 1 1 auto; min-width: 0; accent-color: var(--v-txt); }
  /* A HELD CLIP'S BAR IS NOT A RUNNING CLIP'S BAR. The handle stops moving
     because the picture stopped; saying so in the ink as well means the operator
     does not have to watch it for two seconds to find out which it is. */
  .cb-scrub.held { accent-color: var(--v-faint); }
  .cb-t { flex: 0 0 auto; font-size: var(--v-fs-b1); color: var(--v-dim); font-variant-numeric: tabular-nums; }
  .cb-none { flex: 1 1 auto; min-width: 0; font-size: var(--v-fs-b1); color: var(--v-caution); }
  .cb-err { flex: 0 0 auto; font-size: var(--v-fs-b1); color: var(--v-red); }
</style>
