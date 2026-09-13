<script>
  // THE DOCK ROW (docs/REBRAND.md §2) — the strip under the desk that is the
  // same on every workspace, because these four things are true of the room
  // rather than of whatever you happen to be looking at:
  //
  //   Live audio       is the microphone hearing anything
  //   Live transcript  what it heard, as it hears it
  //   Quick tools      the three things that change during a service
  //   Controls         the four that change what a congregation sees
  //
  // It lives in the shell rather than inside Live, which is the point: an
  // operator editing a template still needs to see the level and still needs
  // Clear screens within one reach. Hunting for a panic control through a
  // workspace switch is the failure this prevents.
  import {
    capture,
    meter,
    transcript,
    screenBlack,
    rehearsing,
    detectionOn,
    clearScreens,
    blackScreen,
    setRehearsal,
    setDetection,
    startCountdown,
    sendStageAlert,
  } from './stores/capture.js';
  import { humanError } from './errors.js';

  const SEGS = 24;
  const SEG_ARR = Array.from({ length: SEGS });
  $: lvl = Math.max(0, Math.min(1, $meter.level ?? 0));
  $: litSegs = Math.round(lvl * SEGS);
  $: dbLabel = lvl > 0.0001 ? `${Math.round(20 * Math.log10(lvl))} dB` : '−∞ dB';

  // The last few lines, newest at the bottom, plus whatever is still being said.
  $: lines = $transcript.finals.slice(-4);

  let busy = false;
  let err = '';
  /** Every control here reports its own failure. A dock button that fails
   *  silently is worse than no dock button (CLAUDE.md rule 15). */
  async function run(fn) {
    busy = true;
    err = '';
    try {
      await fn();
    } catch (e) {
      err = humanError(e);
    }
    busy = false;
  }

  // Quick tools — the three things that change during a service.
  let cdMin = 5;
  let stageMsg = '';
  let stageSent = false;
  const beginCountdown = () => run(() => startCountdown(Number(cdMin) || 5, 'Service begins in', 'Welcome'));
  const toPreacher = () => run(async () => {
    const line = stageMsg.trim();
    if (!line) return;
    await sendStageAlert(line);
    stageSent = true;
  });
  const clearPreacher = () => run(async () => {
    await sendStageAlert(null);
    stageSent = false;
    stageMsg = '';
  });

  // The four controls. `clearScreens` and `blackScreen` return a BOOLEAN and set
  // `panicError` themselves — they are called from a global key handler as well
  // as from here, and neither caller can catch (rule 15). So they are not run
  // through `run()`: their failure is already on the shell's panic banner.
  const doClear = () => clearScreens();
  const doBlack = () => blackScreen();
</script>

<section class="dock" aria-label="Dock">
  <div class="dpanel">
    <span class="dk">Live audio</span>
    <div class="dbody arow">
      <span class="meter" aria-hidden="true">
        {#each SEG_ARR as _, i}
          <i class="sg" class:on={i < litSegs} class:mid={i >= 15 && i < 20} class:hot={i >= 20}></i>
        {/each}
      </span>
      <span class="db r-mono">{dbLabel}</span>
      <span class="vad r-mono" class:on={$meter.isVoice}>{$meter.isVoice ? 'VOICE' : 'quiet'}</span>
    </div>
  </div>

  <div class="dpanel">
    <span class="dk">Live transcript</span>
    <div class="dbody tbody r-scroll">
      {#each lines as l}
        <p class="tl">{l}</p>
      {/each}
      {#if $transcript.partial}
        <p class="tl part">{$transcript.partial}</p>
      {/if}
      {#if !lines.length && !$transcript.partial}
        <p class="tl empty">{$capture.capturing ? 'listening…' : 'not listening'}</p>
      {/if}
    </div>
  </div>

  <div class="dpanel">
    <span class="dk">Quick tools</span>
    <div class="dbody tools">
      <label class="trow">
        <span>Countdown</span>
        <input class="r-input tin" type="number" min="1" max="120" bind:value={cdMin} aria-label="Countdown minutes" />
        <button class="r-btn sm ghost" on:click={beginCountdown} disabled={busy || !$capture.available}>Start</button>
      </label>
      <label class="trow">
        <span>To preacher</span>
        <input
          class="r-input tin wide"
          type="text"
          bind:value={stageMsg}
          placeholder="stage monitor only"
          aria-label="Word to the preacher — stage monitor only"
          on:keydown={(e) => e.key === 'Enter' && toPreacher()} />
        <button class="r-btn sm ghost" on:click={toPreacher} disabled={busy || !stageMsg.trim()}>Send</button>
        <button class="r-btn sm ghost" on:click={clearPreacher} disabled={busy || !stageSent}>Clear</button>
      </label>
      {#if err}<p class="derr" role="alert">{err}</p>{/if}
    </div>
  </div>

  <div class="dpanel">
    <span class="dk">Controls</span>
    <!-- THE FOUR COLOURS (docs/REBRAND.md §1). Clear screens red, Blackout black
         with a hairline, Rehearse amethyst, Detection cyan. None of them shares a
         colour with another, because the two most consequential buttons in the
         room used to look alike. -->
    <div class="dbody r-ctl">
      <button class="r-cbtn danger" on:click={doClear} disabled={!$capture.available}>Clear screens</button>
      <button class="r-cbtn black" data-on={$screenBlack ? '1' : '0'} on:click={doBlack} disabled={!$capture.available}>
        {$screenBlack ? 'Black — restore' : 'Blackout'}
      </button>
      <button
        class="r-cbtn rehearse"
        data-on={$rehearsing ? '1' : '0'}
        on:click={() => run(() => setRehearsal(!$rehearsing))}
        disabled={busy || !$capture.available}
      >{$rehearsing ? 'Rehearsing' : 'Rehearse'}</button>
      <button
        class="r-cbtn"
        on:click={() => run(() => setDetection(!$detectionOn))}
        disabled={busy || !$capture.available}
      >Detection {$detectionOn ? 'on' : 'off'}</button>
    </div>
  </div>
</section>

<style>
  /* Fixed height, four columns, and every panel scrolls inside itself — the dock
     may never grow and take the desk with it. */
  .dock {
    flex: 0 0 auto;
    height: 152px;
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1.4fr) minmax(0, 1.2fr) minmax(0, 1fr);
    background: var(--v-bg);
    border-top: 1px solid var(--v-rule);
    min-height: 0;
  }
  .dpanel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    border-right: 1px solid var(--v-rule);
    padding: 6px 9px 8px;
  }
  .dpanel:last-child { border-right: 0; }
  .dk {
    font-family: var(--f-mono);
    font-size: var(--v-fs-cap);
    font-weight: 600;
    letter-spacing: var(--v-tr-caps);
    text-transform: uppercase;
    color: var(--v-faint);
    margin-bottom: 6px;
    flex: 0 0 auto;
  }
  .dbody { flex: 1 1 0; min-height: 0; }

  .arow { display: flex; align-items: center; gap: 8px; }
  .meter { display: flex; gap: 2px; flex: 1; min-width: 0; }
  .sg {
    flex: 1;
    height: 16px;
    min-width: 2px;
    border-radius: 1px;
    background: var(--v-surf3);
  }
  .sg.on { background: var(--v-emerald); }
  .sg.on.mid { background: var(--v-amber); }
  .sg.on.hot { background: var(--v-red); }
  .db { font-size: var(--v-fs-cap); color: var(--v-dim); flex: 0 0 auto; }
  .vad {
    font-size: 8.5px; font-weight: 600; letter-spacing: var(--v-tr-caps);
    text-transform: uppercase; padding: 2px 6px; border-radius: var(--v-r-sm);
    /* --v-dim, not --v-faint: muted text on --v-surf3 is 3.79:1, below AA, and
       `tokencontrast.test.js` fails the build for exactly this pairing. */
    background: var(--v-surf3); color: var(--v-dim); border: 1px solid var(--v-500);
  }
  .vad.on { background: var(--v-emerald-soft); color: var(--v-emerald); border-color: var(--v-emerald-line); }

  .tbody { overflow-y: auto; display: flex; flex-direction: column; gap: 3px; }
  .tl { margin: 0; font-size: var(--v-fs-b2); line-height: 1.45; color: var(--v-dim); }
  /* What is still being said is the SELECTION colour — it is the thing being
     worked on, not a claim about a screen. */
  .tl.part { color: var(--v-sel); }
  .tl.empty { color: var(--v-faint); font-family: var(--f-mono); font-size: var(--v-fs-cap); }

  .tools { display: flex; flex-direction: column; gap: 6px; justify-content: flex-start; }
  .trow { display: flex; align-items: center; gap: 6px; font-size: var(--v-fs-b2); color: var(--v-dim); }
  .trow > span { flex: 0 0 auto; min-width: 74px; }
  .tin { width: 62px; flex: 0 0 auto; }
  .tin.wide { flex: 1 1 auto; width: auto; min-width: 0; }
  .derr { margin: 0; font-size: var(--v-fs-cap); color: var(--v-red); }

  .r-ctl { align-content: start; gap: 5px; }
</style>
