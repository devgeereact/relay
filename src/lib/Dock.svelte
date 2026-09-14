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
  import { onDestroy } from 'svelte';
  import {
    capture,
    meter,
    transcript,
    live,
    screenBlack,
    rehearsing,
    detectionOn,
    clearScreens,
    blackScreen,
    setRehearsal,
    setDetection,
    startCountdown,
    adjustCountdown,
    countdownRemaining,
    sendStageAlert,
    stageAlert,
  } from './stores/capture.js';
  import { humanError } from './errors.js';
  import { formatCountdown } from './layers.js';
  import {
    countdownSet,
    countdownPress,
    countdownCan,
    msFromFields,
    fieldsFromMs,
  } from './countdown.js';

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

  // ── QUICK TOOLS · THE COUNTDOWN (docs/REBRAND.md §7) ─────────────────────
  //
  // One timer, one formatter. The figure below is `formatCountdown` reading the
  // live content's own `countdown_to` — the same field the wall and the stage page
  // read, through the same function — so the three cannot drift. The transport
  // never runs a clock of its own; it only re-aims that target.
  //
  // The SET duration lives in `countdown.js`, at module scope, because this
  // component is `{#if !liveFullscreen}<Dock />{/if}` in the shell: pressing Full
  // screen destroys it. A component-local `let` would silently lose whatever the
  // operator had typed, mid-service.
  $: cdFields = fieldsFromMs($countdownSet);
  let nowTick = Date.now();
  const cdTimer = setInterval(() => (nowTick = Date.now()), 500);
  onDestroy(() => clearInterval(cdTimer));
  // `$live` is read as well as the tick, so the readout moves when either does.
  $: cdRunning = $live?.countdown_to ? countdownRemaining(nowTick) : null;
  $: cdText = cdRunning == null ? '' : formatCountdown(cdRunning);

  /** Type into hh : mm : ss. Only ever changes the tool, never a screen. */
  function setField(which, value) {
    const f = { ...cdFields, [which]: value };
    countdownSet.set(msFromFields(f.h, f.m, f.s));
  }

  /**
   * One press of the transport. The decision is `countdownPress` — pure, tested —
   * and this half only performs it. `broadcastMs === null` means "touch no
   * screen", which is what Clear and an off-air ±1 both are.
   */
  function press(action) {
    const r = countdownPress(action, $countdownSet, cdRunning);
    countdownSet.set(r.setMs);
    if (r.refused) {
      err = r.refused;
      return;
    }
    if (r.broadcastMs == null) {
      err = '';
      return;
    }
    // Start puts a new countdown up; Reset and ± re-aim the one already there,
    // which `startCountdown` deliberately refuses to do.
    run(() =>
      action === 'start'
        ? startCountdown(r.broadcastMs / 60_000, 'Service begins in', 'Welcome')
        : adjustCountdown(r.broadcastMs),
    );
  }

  let stageMsg = '';
  const toPreacher = () => run(async () => {
    const line = stageMsg.trim();
    if (!line) return;
    await sendStageAlert(line);
  });
  const clearPreacher = () => run(async () => {
    await sendStageAlert(null);
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
      <!-- THE COUNTDOWN, WITH ITS TRANSPORT (docs/REBRAND.md §7). hh : mm : ss,
           then Start · Reset · ±1 · Clear. The figure on the right is the one on
           the wall — same field, same formatter — not a second clock. -->
      <div class="trow">
        <span>Countdown</span>
        <span class="cdfields">
          <input class="r-input cdf" type="number" min="0" max="12" value={cdFields.h}
            on:input={(e) => setField('h', e.target.value)} aria-label="Countdown hours" />
          <i class="cdsep">:</i>
          <input class="r-input cdf" type="number" min="0" max="59" value={cdFields.m}
            on:input={(e) => setField('m', e.target.value)} aria-label="Countdown minutes" />
          <i class="cdsep">:</i>
          <input class="r-input cdf" type="number" min="0" max="59" value={cdFields.s}
            on:input={(e) => setField('s', e.target.value)} aria-label="Countdown seconds" />
        </span>
        <span class="spring"></span>
        {#if cdText}
          <span class="cdlive r-mono" role="status" aria-live="off" title="What the screens are showing">{cdText}</span>
        {/if}
      </div>
      <!-- Clear is NOT Clear screens. It returns this tool to its default length
           and touches nothing a congregation can see; the red control one panel
           along is the one that blanks a wall. -->
      <div class="trow cdtrans" role="group" aria-label="Countdown transport">
        <button class="r-btn sm ghost" on:click={() => press('start')}
          disabled={busy || !$capture.available || !countdownCan('start', $countdownSet, cdRunning)}>Start</button>
        <button class="r-btn sm ghost" on:click={() => press('reset')}
          disabled={busy || !$capture.available || !countdownCan('reset', $countdownSet, cdRunning)}>Reset</button>
        <button class="r-btn sm ghost" on:click={() => press('minus')} aria-label="One minute less"
          disabled={busy || !$capture.available || !countdownCan('minus', $countdownSet, cdRunning)}>−1</button>
        <button class="r-btn sm ghost" on:click={() => press('plus')} aria-label="One minute more"
          disabled={busy || !$capture.available || !countdownCan('plus', $countdownSet, cdRunning)}>+1</button>
        <button class="r-btn sm ghost" on:click={() => press('clear')}
          title="Reset this tool to five minutes. It does not clear the screens.">Clear</button>
      </div>
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
        <button class="r-btn sm ghost" on:click={clearPreacher} disabled={busy || !$stageAlert}>Clear</button>
      </label>
      {#if err}<p class="derr" role="alert">{err}</p>{/if}
    </div>
  </div>

  <div class="dpanel">
    <span class="dk">Controls</span>
    <!-- FOUR CONTROLS, FOUR COLOURS, NONE SHARED — because the two most
         consequential buttons in the room used to look alike.
         Clear screens red, Blackout black with a hairline, Rehearse amethyst,
         Detection cyan.
         NOT the same four as docs/REBRAND.md §1, and the difference is deliberate
         rather than an oversight. §1's fourth is "Go Live green → End service
         amber (it owns the on-air session)". Relay has no honest state to drive
         that pair: `start_service` is called from `startCapture`, `end_service`
         only from Library → History, and the frontend has no way to ASK whether a
         service is recording — `current_service` was deliberately deleted
         (CLAUDE.md, "No dead-but-built commands"). A Go Live / End service button
         driven by a frontend flag would read "Go Live" after a console crash while
         the service was still open in the database: a status control that cannot
         detect its own failure, which is rule 35. Detection on/off is what is here
         instead, and it answers for itself. -->
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
  .spring { flex: 1 1 auto; min-width: 0 !important; }

  /* hh : mm : ss. Mono figures so a changing number never reflows the row beside
     it (docs/REBRAND.md §1). */
  .cdfields { display: flex; align-items: center; gap: 2px; min-width: 0 !important; }
  .cdf {
    width: 34px; flex: 0 0 auto; text-align: center;
    font-family: var(--f-mono); font-variant-numeric: tabular-nums;
    /* The spinner arrows steal a third of a 34px field and are unusable in a dark
       booth; the ±1 buttons below are the control that adjusts this. */
    -moz-appearance: textfield;
    appearance: textfield;
  }
  .cdf::-webkit-outer-spin-button,
  .cdf::-webkit-inner-spin-button { -webkit-appearance: none; margin: 0; }
  .cdsep { font-style: normal; color: var(--v-faint); flex: 0 0 auto; }
  /* The figure on the wall. NOT amber: a countdown is content on a screen, but
     this is a readout of it, and amber in this room means ON AIR and is never
     allowed to be anything else (CLAUDE.md rule 18). */
  .cdlive {
    flex: 0 0 auto; min-width: 0 !important;
    font-variant-numeric: tabular-nums;
    font-size: var(--v-fs-b2); color: var(--v-txt);
    padding: 1px 5px; border-radius: var(--v-r-sm);
    background: var(--v-surf3); border: 1px solid var(--v-500);
  }
  .cdtrans { gap: 4px; flex-wrap: wrap; }
  .cdtrans > :global(button) { flex: 0 0 auto; }
  .tin { width: 62px; flex: 0 0 auto; }
  .tin.wide { flex: 1 1 auto; width: auto; min-width: 0; }
  .derr { margin: 0; font-size: var(--v-fs-cap); color: var(--v-red); }

  .r-ctl { align-content: start; gap: 5px; }
</style>
