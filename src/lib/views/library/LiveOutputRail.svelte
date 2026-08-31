<script>
  // THE RUN COLUMN — what is on the wall, what the AI heard, what is next, and
  // the controls for all three.
  //
  //   LIVE OUTPUT   pinned to the top. It is the answer to the only question
  //                 that matters mid-service, so it never scrolls away.
  //   HEARD         AI suggestions, approved or dismissed in one press.
  //   TRANSCRIPT    what the microphone is actually getting. When detection
  //                 goes quiet this is the difference between "the preacher has
  //                 not said a reference" and "Relay has gone deaf", and those
  //                 need opposite responses.
  //   UP NEXT       the queue, whatever kind of content it holds.
  //   ACTIONS       pinned to the bottom. Reached under pressure, always in the
  //                 same place.
  //
  // ── What was removed, and why ─────────────────────────────────────────────
  //
  // OUTPUT HEALTH is gone. It reported ports and a window count — facts that do
  // not change during a service and that nobody watches. The reference it came
  // from filled it with frame rate and bitrate, which Relay cannot measure at
  // all. A panel that is either static or fictional is worse than no panel: it
  // occupies the space where the queue and the transcript belong. Channel state
  // lives on the Channels tab, which is where an operator goes to fix it.
  import { onMount, onDestroy } from 'svelte';
  import TemplateRender from '../../TemplateRender.svelte';
  import { humanError } from '../../errors.js';
  import { safeMode } from '../../boot/boot.js';
  import { move, dequeue, take, clear as clearAll } from '../../queue.js';
  import {
    live,
    screenBlack,
    rehearsing,
    capturing,
    transcript,
    detections,
    confirmDetection,
    dismissDetection,
    setRehearsal,
    clearScreens,
    blackScreen,
    startCountdown,
    startCapture,
    stopCapture,
    listOutputChannels,
  } from '../../stores/capture.js';
  import { inLibrary } from '../../detect.js';

  export let template = null;
  export let allTemplates = [];
  export let queue = [];
  export let onQueueChange = () => {};
  export let onFireQueued = () => {};

  /** Escape closes the popup and goes NO FURTHER — it must not reach the panic key. */
  function menuEsc(e) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      e.preventDefault();
      showMins = false;
    }
  }

  let channels = [];
  let watching = null;
  let error = '';
  let msg = '';
  let busyRef = '';
  let showMins = false;
  let poll;

  onMount(async () => {
    channels = (await listOutputChannels()) ?? [];
    poll = setInterval(async () => (channels = (await listOutputChannels()) ?? channels), 8000);
  });
  onDestroy(() => clearInterval(poll));

  $: onAir = !!$live && !$screenBlack;
  // Which output this monitor draws with: a channel's own template, or whatever
  // the fired content carried. Channels are render targets of ONE engine, so
  // the same cue looks different on each — and an operator could not check the
  // stage display without walking to it.
  $: watched = channels.find((c) => c.id === watching) ?? null;
  $: monitorTemplate =
    (watched && allTemplates.find((t) => t.id === watched.template_id)) || template;
  $: lines = [...($transcript.finals ?? [])].slice(-4).reverse();

  const moveInQueue = (ref, d) => onQueueChange(move(queue, ref, d));
  const dropFromQueue = (ref) => onQueueChange(dequeue(queue, ref));

  async function goLive() {
    error = '';
    msg = '';
    // The QUEUE is the staging area — "Up Next" is a switcher that holds N items
    // rather than one. The `preview` prop this used to check first had no producer
    // in the shipping app and is gone (audit P1-2); see Library.svelte for why the
    // AI path stayed at one press.
    const { item, rest } = take(queue);
    if (!item) {
      msg = 'Nothing staged and nothing queued.';
      return;
    }
    try {
      await onFireQueued(item);
      onQueueChange(rest);
    } catch (e) {
      error = humanError(e);
    }
  }

  async function accept(d) {
    // Parsed cleanly, resolves to no verse — the backend says so with
    // `in_library: false`. The button below is disabled for it; this is the second
    // door, because a guard rendered only in markup is a guard the next caller
    // walks past.
    if (!inLibrary(d)) {
      error = `${d.reference} is not in your Bible — Relay misheard a number. Nothing was sent.`;
      return;
    }
    busyRef = d.reference;
    error = '';
    try {
      await confirmDetection(d.reference);
    } catch (e) {
      error = humanError(e);
    }
    busyRef = '';
  }

  async function mic() {
    error = '';
    try {
      if ($capturing) {
        await stopCapture();
        msg = 'Listening stopped.';
      } else {
        await startCapture();
        msg = 'Listening.';
      }
    } catch (e) {
      error = humanError(e);
    }
  }

  // Panic controls report through the global banner; they return a boolean and
  // never throw (CLAUDE.md §15).
  async function panic(fn, done) {
    msg = '';
    if (await fn()) msg = done;
  }

  /** Restarting IS the common case — the service slipped five minutes. */
  async function countdown(mins) {
    error = '';
    msg = '';
    showMins = false;
    try {
      await startCountdown(mins);
      msg = `Countdown started — ${mins} minutes.`;
    } catch {
      if (!(await clearScreens())) return;
      try {
        await startCountdown(mins);
        msg = `Countdown restarted — ${mins} minutes.`;
      } catch (e) {
        error = humanError(e);
      }
    }
  }
</script>

<aside class="lo">
  <!-- ── LIVE OUTPUT — pinned ─────────────────────────────────────────────── -->
  <section class="lo-panel lo-top">
    <header class="lo-head">
      <p class="r-lbl">
        Live Output <span class="lo-dot">·</span>
        {$rehearsing ? 'Rehearsal' : 'Program'}
      </p>
      {#if channels.length > 1}
        <select class="lo-pick" aria-label="Which output to watch" bind:value={watching}>
          <option value={null}>As fired</option>
          {#each channels as c}<option value={c.id}>{c.name}</option>{/each}
        </select>
      {/if}
      <!-- This monitor now shows ONE thing: the congregation's wall. It used to be
           time-multiplexed with a staged slide, and the badge read `onAir` alone —
           which knew nothing about `preview` — so a staged verse drew an amber,
           pulsing "Live" beside content nobody could see. That was fixed, and then
           the audit found the staged half had no producer at all and could never
           render. The honest repair is one pane, one fact. Amber means live and is
           never allowed to lie (CLAUDE.md §18). -->
      <span class="r-badge {onAir && !$rehearsing ? 'amber' : $rehearsing ? 'amethyst' : 'grey'}">
        {#if onAir && !$rehearsing}<span class="bd"></span>{/if}
        {onAir ? ($rehearsing ? 'Rehearsal' : 'Live') : 'Clear'}
      </span>
    </header>

    <div class="lo-screen">
      {#if $screenBlack}
        <span class="lo-empty">Blacked out</span>
      {:else if $live}
        {#if monitorTemplate}
          <TemplateRender
            template={monitorTemplate}
            content={{
              reference: $live.reference,
              text: $live.text,
              translation: $live.translation,
              media_url: $live.media_url,
              media_kind: $live.media_kind,
            }} />
        {:else if $live.media_url}
          <img class="lo-raw" src={$live.media_url} alt="" />
        {:else}
          <span class="lo-plain">{$live.text}</span>
        {/if}
      {:else}
        <span class="lo-empty">Nothing is on the screens</span>
      {/if}
    </div>

    <!-- "Take to screen →" lived here and was permanently disabled: it took the
         `preview` prop, which had no producer. Going live from the Library is the
         "Go Live" control at the bottom of this rail, which fires the top of the
         queue — the staging area that actually exists. -->
  </section>

  <div class="lo-scroll r-scroll">
    <!-- ── HEARD — approve or dismiss in one press ───────────────────────── -->
    <section class="lo-panel lo-pad lo-heardpanel">
      <header class="lo-secheadrow">
        <p class="r-lbl">Heard</p>
        {#if $detections.length}<span class="r-chip cyan">{$detections.length}</span>{/if}
      </header>
      {#if !$detections.length}
        <p class="lo-none">
          Nothing suggested. Detected references appear here — the AI never puts one on a
          screen by itself.
        </p>
      {:else}
        {#each $detections as d (d.reference)}
          <article class="lo-sug">
            <div class="lo-sugtop">
              <b>{d.reference}</b>
              <!-- WHICH KIND OF CLAIM (CLAUDE.md §18). A paraphrase carries no
                   percentage: a cosine is not a probability. -->
              <span class="r-chip {d.method === 'direct' ? 'green' : 'cyan'}">
                {d.method === 'direct' ? 'Heard' : 'Guess'}
              </span>
            </div>
            {#if d.matched_text}<p class="lo-heard">“{d.matched_text}”</p>{/if}
            {#if !inLibrary(d)}
              <p class="lo-absent">Not in your Bible — Relay misheard a number.</p>
            {/if}
            <div class="lo-sugacts">
              <button
                class="r-btn amber sm"
                disabled={busyRef === d.reference || $safeMode || !inLibrary(d)}
                on:click={() => accept(d)}>
                {#if !inLibrary(d)}Nothing to send{:else if busyRef === d.reference}Sending…{:else}Approve{/if}
              </button>
              <button class="r-btn ghost sm" on:click={() => dismissDetection(d.reference)}>
                Dismiss
              </button>
            </div>
          </article>
        {/each}
      {/if}
    </section>

    <!-- ── UP NEXT ─────────────────────────────────────────────────────────
         ONLY WHEN THERE IS SOMETHING IN IT. An empty panel reading "(0)" and a
         sentence explaining that it is empty was taking a third of the column
         away from the two panels an operator actually reads mid-service — and
         it pushed the Approve button of a live suggestion below the fold. An
         empty queue needs no words; `Go Live` names the next item when there
         is one. -->
    {#if queue.length}
    <section class="lo-panel lo-pad">
      <header class="lo-secheadrow">
        <p class="r-lbl">Up next ({queue.length})</p>
        <button class="lo-link" on:click={() => onQueueChange(clearAll())}>Clear</button>
      </header>
      {#if true}
        <ol class="lo-queue">
          {#each queue as item, i (item.reference)}
            <li class="lo-q" class:next={i === 0}>
              <span class="lo-qn r-mono">{i + 1}</span>
              <span class="lo-qc">
                <b>{item.reference}</b>
                {#if item.text}<span>{item.text}</span>{/if}
              </span>
              <span class="lo-qacts">
                <button class="lo-ic r-focus" aria-label="Move up" disabled={i === 0} on:click={() => moveInQueue(item.reference, -1)}>↑</button>
                <button class="lo-ic r-focus" aria-label="Move down" disabled={i === queue.length - 1} on:click={() => moveInQueue(item.reference, 1)}>↓</button>
                <button class="lo-ic r-focus" aria-label="Remove" on:click={() => dropFromQueue(item.reference)}>×</button>
              </span>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
    {/if}

    <!-- ── TRANSCRIPT ────────────────────────────────────────────────────── -->
    <section class="lo-panel lo-pad">
      <header class="lo-secheadrow">
        <p class="r-lbl">Transcript</p>
        <span class="r-chip {$capturing ? 'green' : ''}">{$capturing ? 'Listening' : 'Idle'}</span>
      </header>
      {#if $transcript.partial}
        <p class="lo-partial">{$transcript.partial}</p>
      {/if}
      {#if lines.length}
        <ol class="lo-lines">
          {#each lines as line, i}<li class:latest={i === 0}>{line}</li>{/each}
        </ol>
      {:else if !$transcript.partial}
        <p class="lo-none">
          {$capturing
            ? 'Listening — nothing transcribed yet.'
            : 'Not listening. Start the microphone to see what Relay hears.'}
        </p>
      {/if}
    </section>
  </div>

  <!-- ── ACTIONS — pinned ─────────────────────────────────────────────────── -->
  <section class="lo-panel lo-pad lo-quick">
    <!-- AMBER: this is what puts something in front of people. "Send to Preview"
         used to sit beside it and did the same job — two buttons, one action, and a
         row of height the transcript needed more. That removal was right, and the
         2026-08-15 audit finished it: the staged half it left behind had no producer
         at all. This fires the top of the QUEUE, which is the staging area that
         actually exists. -->
    <button
      class="r-btn amber lo-golive"
      disabled={$safeMode || !queue.length}
      on:click={goLive}>
      {queue.length ? `Go Live — ${queue[0].reference}` : 'Go Live'}
    </button>

    <div class="lo-tiles">
      <button class="lo-tile r-focus" class:on={$capturing} on:click={mic}>
        <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><rect x="9" y="3" width="6" height="11" rx="3" /><path d="M5 11a7 7 0 0 0 14 0M12 18v3" /></svg>
        {$capturing ? 'Listening' : 'Listen'}
      </button>
      <button class="lo-tile r-focus" on:click={() => panic(clearScreens, 'Screens cleared.')}>
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><path d="M4 7h16M9 7V5h6v2M7 7l1 13h8l1-13" /></svg>
        Clear Screens
      </button>
      <button class="lo-tile r-focus" on:click={() => panic(blackScreen, 'Blacked out.')}>
        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7"><rect x="3" y="5" width="18" height="14" rx="2" /></svg>
        Blank Screen
      </button>
      <div class="lo-cdwrap">
        <button class="lo-tile r-focus" on:click={() => (showMins = !showMins)}>
          <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><circle cx="12" cy="13" r="8" /><path d="M12 9v4l2.5 2.5M9 2h6" /></svg>
          Countdown
        </button>
        {#if showMins}
          <button class="lo-scrim" tabindex="-1" aria-label="Close" on:click={() => (showMins = false)}></button>
          <!-- `role="menu"` is load-bearing, not decoration. `shortcuts.js` decides
               whether Escape belongs to an overlay or to the panic key by probing
               the DOM, deliberately, so the rule cannot depend on anyone
               remembering to register a new popup. This menu carried no role at
               all, so Escape here used to blank the congregation's screens and
               leave the menu open — on the RUN RAIL, mid-service. -->
          <div class="lo-menu" role="menu" tabindex="-1" on:keydown={menuEsc}>
            {#each [5, 10, 15, 30] as m}
              <button class="lo-mi" role="menuitem" on:click={() => countdown(m)}>{m} minutes</button>
            {/each}
          </div>
        {/if}
      </div>
    </div>

    <button class="lo-reh r-focus" class:on={$rehearsing} on:click={() => setRehearsal(!$rehearsing)}>
      {$rehearsing ? 'End rehearsal' : 'Rehearsal mode'}
    </button>

    {#if msg}<p class="lo-msg">{msg}</p>{/if}
    {#if error}<p class="lo-err" role="alert">{error}</p>{/if}
  </section>
</aside>

<style>
  .lo {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
    overflow: hidden;
    /* A HARD CEILING. This rail leads with a 16:9 monitor, so any width it is
       given becomes a slide 0.56x as tall; when a parent failed to constrain it
       the monitor filled the console. Width, never height — capping the height
       of an `aspect-ratio` box shrinks its WIDTH and leaves a dead strip. */
    max-width: 460px;
    width: 100%;
  }
  /* PINNED TOP. What is on the wall is the one thing that must never scroll
     away. */
  .lo-top,
  .lo-quick {
    flex: 0 0 auto;
  }
  .lo-scroll {
    flex: 1 1 auto;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .lo-panel {
    background: var(--v-bg);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-lg);
    overflow: hidden;
  }
  /* NOT clipped: the countdown menu opens upward out of this panel, and
     `overflow: hidden` cut it in half — the same bug the card kebab had. */
  .lo-quick {
    overflow: visible;
  }
  .lo-pad {
    padding: 13px 14px;
  }
  .lo-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 11px 13px;
    border-bottom: 1px solid var(--v-line);
  }
  .lo-head .r-lbl {
    flex: 1;
    margin: 0;
  }
  /* The "wall live" chip sits BESIDE the pane's own badge, and is deliberately the
     smaller of the two: it is a second fact, not a competing headline. It must not
     wrap the header onto a second line on a narrow rail. */
  .lo-behind {
    flex: 0 0 auto;
    padding: 4px 9px;
    font-size: 9px;
  }
  .lo-pad .r-lbl {
    margin: 0;
  }
  .lo-dot {
    color: var(--v-500);
  }
  .lo-pick {
    flex: 0 0 auto;
    max-width: 128px;
    height: 24px;
    padding: 0 6px;
    border-radius: var(--v-r-sm);
    background: var(--v-surf2);
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
    font-family: var(--f-body);
    font-size: 11px;
    cursor: pointer;
  }
  .lo-secheadrow {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-bottom: 9px;
  }
  .lo-link {
    border: 0;
    background: transparent;
    color: var(--v-accent2);
    font-family: var(--f-body);
    font-size: 11.5px;
    cursor: pointer;
  }

  .lo-screen {
    /* POSITION: RELATIVE IS LOAD-BEARING. `TemplateRender`'s root is
       `position: absolute; inset: 0`; without a positioned ancestor the slide
       resolves against the shell and paints across the whole console. */
    position: relative;
    aspect-ratio: 16 / 9;
    width: 100%;
    container-type: inline-size;
    background: #000;
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .lo-raw {
    width: 100%;
    height: 100%;
    object-fit: contain;
    display: block;
  }
  .lo-empty {
    font-family: var(--f-mono);
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--v-faint);
  }
  .lo-plain {
    padding: 14px;
    font-family: var(--f-serif);
    font-size: 13px;
    line-height: 1.5;
    color: var(--v-dim);
    text-align: center;
  }
  .lo-take {
    display: block;
    width: calc(100% - 20px);
    margin: 10px;
    height: 36px;
    border-radius: var(--v-r-md);
    background: var(--v-surf2);
    border: 1px solid var(--v-line2);
    color: var(--v-accent2);
    font-family: var(--f-body);
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
  }
  .lo-take:hover:not(:disabled) {
    background: var(--v-surf3);
  }
  .lo-take:disabled {
    color: var(--v-faint);
    cursor: not-allowed;
  }

  .lo-none {
    margin: 0;
    font-size: var(--v-fs-cap);
    line-height: 1.6;
    color: var(--v-faint);
  }

  /* The suggestion panel keeps its full height; the transcript below it is what
     gives way. Approve must never be the thing that scrolled off. */
  .lo-heardpanel {
    flex: 0 0 auto;
  }
  .lo-sug {
    padding: 10px 0;
    border-top: 1px solid var(--v-line);
  }
  .lo-sug:first-of-type {
    border-top: 0;
    padding-top: 0;
  }
  .lo-sugtop {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .lo-sugtop b {
    flex: 1;
    min-width: 0;
    font-size: var(--v-fs-b2);
    font-weight: 600;
    color: var(--v-txt);
  }
  .lo-heard {
    margin: 6px 0 0;
    font-size: var(--v-fs-cap);
    line-height: 1.5;
    color: var(--v-faint);
    font-style: italic;
  }
  /* Rose, never amber: nothing about a reference with no verse behind it is live. */
  .lo-absent {
    margin: 6px 0 0;
    font-size: var(--v-fs-cap);
    line-height: 1.5;
    color: var(--v-rose);
  }
  .lo-sugacts {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    margin-top: 9px;
  }
  .lo-sugacts .r-btn {
    width: 100%;
  }

  .lo-queue {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .lo-q {
    display: grid;
    grid-template-columns: 18px minmax(0, 1fr) auto;
    gap: 9px;
    align-items: start;
    padding: 8px 9px;
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    border-radius: var(--v-r-md);
  }
  /* The one that Go Live will send. Grey — it is a position, not a tally. */
  .lo-q.next {
    border-color: var(--v-line2);
    background: var(--v-surf2);
  }
  .lo-qn {
    font-size: 11px;
    color: var(--v-faint);
    padding-top: 2px;
  }
  .lo-qc {
    min-width: 0;
  }
  .lo-qc b {
    display: block;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--v-txt);
  }
  .lo-qc span {
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    margin-top: 2px;
    font-size: var(--v-fs-cap);
    line-height: 1.45;
    color: var(--v-faint);
  }
  .lo-qacts {
    display: flex;
    gap: 2px;
  }
  .lo-ic {
    width: 20px;
    height: 20px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: var(--v-r-sm);
    background: transparent;
    color: var(--v-faint);
    font-size: 12px;
    cursor: pointer;
  }
  .lo-ic:hover:not(:disabled) {
    background: var(--v-surf3);
    color: var(--v-txt);
  }
  .lo-ic:disabled {
    opacity: 0.3;
    cursor: not-allowed;
  }

  .lo-partial {
    margin: 0 0 8px;
    font-size: var(--v-fs-b2);
    line-height: 1.5;
    color: var(--v-txt);
  }
  .lo-lines {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .lo-lines li {
    font-size: var(--v-fs-cap);
    line-height: 1.5;
    color: var(--v-faint);
  }
  .lo-lines li.latest {
    color: var(--v-dim);
  }

  .lo-golive {
    width: 100%;
    height: 38px;
    margin-bottom: 7px;
  }
  /* ONE ROW of four. Two rows of two cost ~90px of a column the transcript and
     the queue were being squeezed out of. */
  .lo-tiles {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }
  .lo-tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 8px 4px;
    border-radius: var(--v-r-md);
    background: var(--v-surf);
    border: 1px solid var(--v-line);
    color: var(--v-dim);
    font-family: var(--f-body);
    font-size: 10px;
    line-height: 1.2;
    text-align: center;
    cursor: pointer;
    transition: border-color 0.14s, color 0.14s;
  }
  .lo-tile:hover {
    border-color: var(--v-line2);
    color: var(--v-txt);
  }
  /* Listening is ON — amethyst, never amber: amber means the congregation is
     looking at something, and a live microphone is not that. */
  .lo-tile.on {
    border-color: var(--v-accent-line);
    color: var(--v-accent2);
    background: var(--v-accent-soft);
  }
  .lo-reh {
    width: 100%;
    height: 28px;
    margin-top: 6px;
    border-radius: var(--v-r-md);
    background: transparent;
    border: 1px solid var(--v-line2);
    color: var(--v-dim);
    font-family: var(--f-body);
    font-size: 11.5px;
    cursor: pointer;
  }
  .lo-reh.on {
    border-color: var(--v-amethyst);
    background: var(--v-amethyst-soft);
    color: var(--v-amethyst);
  }

  .lo-cdwrap {
    position: relative;
  }
  .lo-scrim {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: transparent;
    border: 0;
    cursor: default;
  }
  .lo-menu {
    position: absolute;
    left: 0;
    bottom: calc(100% + 6px);
    z-index: 50;
    min-width: 140px;
    padding: 6px;
    background: var(--v-surf2);
    border: 1px solid var(--v-line2);
    border-radius: var(--v-r-lg);
    box-shadow: var(--v-shadow-lg);
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .lo-mi {
    text-align: left;
    padding: 8px 10px;
    border: 0;
    border-radius: var(--v-r-md);
    background: transparent;
    color: var(--v-txt);
    font-family: var(--f-body);
    font-size: 12.5px;
    cursor: pointer;
  }
  .lo-mi:hover {
    background: var(--v-surf3);
    color: var(--v-accent2);
  }

  .lo-msg,
  .lo-err {
    margin: 9px 0 0;
    font-size: var(--v-fs-cap);
  }
  .lo-msg {
    color: var(--v-emerald);
  }
  .lo-err {
    color: var(--v-red);
  }

  @media (max-width: 860px) {
    .lo {
      max-height: 78vh;
    }
  }
</style>
