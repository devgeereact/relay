<script>
  // THE DOCK ROW (docs/REBRAND.md §2) — the strip under the desk that is the
  // same on every workspace, because these four things are true of the room
  // rather than of whatever you happen to be looking at:
  //
  //   Live audio       what the microphone is hearing, and the two decisions
  //                    about that signal
  //   Live transcript  what it heard, as it hears it
  //   Quick tools      the three things that change during a service
  //   Controls         the ones that change what a congregation sees
  //
  // It lives in the shell rather than inside Live, which is the point: an
  // operator editing a template still needs to see the level and still needs
  // Clear screens within one reach. Hunting for a panic control through a
  // workspace switch is the failure this prevents.
  //
  // ── FOUR EQUAL CARDS ON A TROUGH ───────────────────────────────────────────
  //
  // The four are one height (178px) and one rhythm: a head with a grip, a mono
  // caption and a right-hand meta slot, then a body that scrolls inside itself.
  // They sit on a darker ground with 1px seams between them rather than flush to
  // the desk — the row reads as four instruments in a rack, which is what they
  // are, instead of four differently-sized panels that happen to be adjacent.
  import { onDestroy, onMount } from 'svelte';
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
    getSensitivity,
    setSensitivity,
    startCountdown,
    adjustCountdown,
    countdownRemaining,
    sendStageAlert,
    stageAlert,
    templates,
    loadTemplates,
    fireContent,
    pushAnnouncement,
  } from './stores/capture.js';
  import { session, setSession } from './session.js';
  import { templateKind } from './templateKind.js';
  import TemplateRender from './TemplateRender.svelte';
  import { humanError } from './errors.js';
  import { rangeFill } from './rangefill.js';
  import { formatCountdown, countdownWarning } from './layers.js';
  import {
    countdownSet,
    countdownPress,
    countdownCan,
    msFromFields,
    fieldsFromMs,
  } from './countdown.js';

  $: lvl = Math.max(0, Math.min(1, $meter.level ?? 0));
  $: dbLabel = lvl > 0.0001 ? `${Math.round(20 * Math.log10(lvl))} dB` : '−∞ dB';

  // The last few lines, newest at the bottom, plus whatever is still being said.
  $: lines = $transcript.finals.slice(-4);
  // What the transcript is being produced BY. Not a decoration: the language is
  // re-elected every window on accented speech (`stt://language_unstable`), and
  // "local" is the offline-first promise stated where an operator can see it.
  // With no model loaded it says so — the alternative reads identically to a
  // working recogniser that simply has not heard anything yet (rule 35).
  $: trMeta = $capture.stt?.loaded
    ? `local · ${$capture.detectedLang || $capture.stt?.language || 'auto'}`
    : 'no model';

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

  // ── LIVE AUDIO · THE WAVEFORM ──────────────────────────────────────────────
  //
  // A meter says how loud; a waveform says what the microphone has been hearing,
  // which is the question an operator is actually asking when the AI goes quiet.
  // It is the REAL envelope: one sample per `audio://chunk`, which arrives every
  // 200 ms hop, so the trace is about 38 seconds of the CLEANED stream — the same
  // signal the voice gate and whisper see.
  //
  // ── WHAT IS DELIBERATELY NOT DRAWN ─────────────────────────────────────────
  //
  // The prototype draws the sensitivity gate as a dashed line across the trace.
  // Relay must not: rule 12 (DECISIONS §19) says audio levels are LEARNED and
  // nothing may compare a signal to an absolute level. A line at a fixed height
  // would draw the voice gate as a threshold it is not, and the one time that
  // picture would matter — a quiet preacher on a church laptop — it would be
  // wrong in exactly the way that made Relay silently deaf. `$meter.isVoice` is
  // the gate's own answer and it is the chip in the head, in words.
  //
  // There is no animation loop: it repaints when the data moves and at no other
  // time, so reduced motion needs no special case and an idle console costs
  // nothing.
  const WN = 190;
  let wave = new Array(WN).fill(0);
  let cv = null;
  let cw = 0;
  let ch = 0;

  /** Push one measured level and repaint. Nothing here interpolates or invents. */
  function pushSample(v) {
    wave = [...wave.slice(1), v];
    draw();
  }
  // A plain reactive statement, never `tick()` — re-entering Svelte's scheduler
  // from a `$:` block infinite-loops the webview's JS thread (rule 1).
  $: pushSample(lvl);

  function sizeCanvas() {
    if (!cv) return;
    const r = cv.getBoundingClientRect();
    const d = (typeof window !== 'undefined' && window.devicePixelRatio) || 1;
    cw = Math.max(1, Math.round(r.width));
    ch = Math.max(28, Math.round(r.height));
    cv.width = Math.round(cw * d);
    cv.height = Math.round(ch * d);
    const cx = cv.getContext('2d');
    if (cx) cx.setTransform(d, 0, 0, d, 0, 0);
    draw();
  }

  function draw() {
    if (!cv || !cw) return;
    const cx = cv.getContext('2d');
    if (!cx) return;
    const mid = ch / 2;
    cx.clearRect(0, 0, cw, ch);
    // ONE mirrored envelope rather than 190 separate bars: a trace reads as a
    // signal, a picket fence reads as a chart.
    const step = cw / (WN - 1);
    cx.beginPath();
    cx.moveTo(0, mid - wave[0] * mid * 0.94);
    for (let i = 1; i < WN; i++) {
      cx.quadraticCurveTo((i - 1) * step + step / 2, mid - wave[i - 1] * mid * 0.94, i * step, mid - wave[i] * mid * 0.94);
    }
    for (let i = WN - 1; i >= 0; i--) cx.lineTo(i * step, mid + wave[i] * mid * 0.94);
    cx.closePath();
    const g = cx.createLinearGradient(0, 0, cw, 0);
    g.addColorStop(0, 'rgba(63,207,106,.10)');
    g.addColorStop(0.72, 'rgba(63,207,106,.34)');
    g.addColorStop(1, 'rgba(63,207,106,.62)');
    cx.fillStyle = g;
    cx.fill();
    cx.strokeStyle = 'rgba(63,207,106,.85)';
    cx.lineWidth = 1.1;
    cx.stroke();
    cx.strokeStyle = 'rgba(232,234,238,.09)';
    cx.lineWidth = 1;
    cx.beginPath();
    cx.moveTo(0, mid);
    cx.lineTo(cw, mid);
    cx.stroke();
  }

  // ── THE SENSITIVITY DIAL, BESIDE THE SIGNAL IT IS ABOUT ────────────────────
  //
  // The one operator dial (DECISIONS §26), read from and written to the live
  // thresholds in Rust — there is exactly one forward mapping and one inverse,
  // and neither lives here. `setSensitivity` THROWS (group 1), so a dial that did
  // not take says so and then shows what the engine actually holds, rather than
  // leaving the thumb where the operator dragged it.
  //
  // ── AND IT MUST NOT SHOW A NUMBER NOBODY SET ───────────────────────────────
  //
  // `getSensitivity` is GROUP 2 (it swallows) and returns **50** when there is no
  // backend — which is also a perfectly ordinary real setting, so the reading
  // alone cannot tell "the engine says 50" from "there is no engine". That is
  // rule 35 on the one control governing what the AI may put on a wall unasked;
  // `setSensitivity` was repaired for exactly this reason and its reader was not
  // (recorded in the review note — `capture.js` is another agent's file).
  //
  // What this card CAN do without reaching into that file is say so, in words,
  // in the ONE place each dock card already has for "I have no answer" — the
  // meta slot in its head, beside `no model` on the transcript card. Not in the
  // value column: a glyph there cannot tell "nobody answered" from "the gate is
  // at 50" (R3-13's rule, and the same defect as "up to date" over a dead update
  // channel), and an 18px column cannot hold the sentence that could. So the
  // figure stays a figure, the caveat is a sentence, and neither pretends to be
  // the other.
  //
  // The number itself is left exactly as `getSensitivity` returned it, fallback
  // included. Substituting a distinct "unknown" value would put a reading on
  // screen that is nobody's setting — a second lie to cover the first.
  //
  // `$capture.available` is the only signal available here, so it is the one the
  // card reports: it answers "is the bridge attached at all", which is the case
  // an operator actually meets, and it does not claim to answer more than that.
  let sensitivity = 50;
  let sensRead = false;
  $: sensReadable = sensRead && $capture.available;
  onMount(async () => {
    sizeCanvas();
    try {
      sensitivity = await getSensitivity();
      sensRead = true;
    } catch {
      sensRead = false;
    }
  });
  async function onSensitivity(v) {
    sensitivity = v;
    err = '';
    try {
      const landed = await setSensitivity(v);
      if (Number.isFinite(landed)) sensitivity = landed;
    } catch (e) {
      try {
        sensitivity = await getSensitivity();
      } catch {
        /* the dial is already disabled in this case */
      }
      err = `Sensitivity stayed at ${sensitivity} — ${humanError(e)}`;
    }
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
  // operator had typed, mid-service. (The waveform history above is deliberately
  // NOT module scope: 38 seconds of trace is a picture, not something an operator
  // typed, and it redraws itself within a breath.)
  $: cdFields = fieldsFromMs($countdownSet);
  let nowTick = Date.now();
  const cdTimer = setInterval(() => (nowTick = Date.now()), 500);
  onDestroy(() => clearInterval(cdTimer));
  // `$live` is read as well as the tick, so the readout moves when either does.
  $: cdRunning = $live?.countdown_to ? countdownRemaining(nowTick) : null;
  // ── THE FIGURE, AND THE TWO THINGS IT CAN BE ──────────────────────────────
  //
  // It is the largest thing in this panel because it is the one thing an
  // operator glances at from across a booth — but it is showing one of TWO
  // facts, and conflating them is how a tool's setting gets read as a wall.
  //
  //   ON THE WALL   `cdRunning` — what the screens are actually counting,
  //                 through the same field and the same formatter the wall and
  //                 the stage page use, so the three cannot drift.
  //   NOT ON AIR    the SET duration — what Start would put up. Dimmed, and the
  //                 caption beside it says which, so a number nobody can see is
  //                 never mistaken for one a congregation is watching.
  //
  // It used to render only in the first case, so the panel's biggest control had
  // no readout at all until after it had been used.
  $: cdLive = cdRunning != null;
  $: cdText = formatCountdown(cdLive ? cdRunning : $countdownSet);
  // The last minute — or the last tenth of a short countdown, because a minute's
  // warning on a two-minute countdown is a colour that is on for half of it
  // (`layers.js`). RED, not amber: amber in this room means ON AIR and is never
  // allowed to be anything else (rule 18), and "this is about to run out" is the
  // act-now colour. Only ever while it is genuinely on a wall.
  $: cdWarn = cdLive && countdownWarning(cdRunning, $countdownSet);

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

  // ── QUICK TOOLS · THE NAME BAND (docs/REBRAND.md §2 and §4) ────────────────
  //
  // Who is speaking changes more often than anything else in a service, and until
  // now the only way to put it on a wall was to build a plan cue for it on a
  // Tuesday. This is the operator's surface for the lower thirds phase 5 already
  // shipped (DECISIONS §75) — it adds no renderer, no template kind and no fire
  // path.
  //
  // WHICH FIELD IS WHICH, and why it reads backwards. A band's large line is bound
  // to `verse` and its small line to `reference` (`layers.js`), and `fire_content`
  // puts its `text` on the first and its `label` on the second. So the NAME is the
  // text and the ROLE is the label. The label is also what names the cue in
  // history, which is the one place this mapping costs something: a name band
  // shows up there as its role. Recorded rather than papered over — the fix is a
  // content kind of its own in `fire_content`, which is a backend change.
  //
  // The picker offers only templates whose SHAPE is a lower third — `templateKind`
  // derives that from the layers, so a template an operator built themselves is
  // offered the moment it has a band, and one that stops being a band stops being
  // offered. No flag to set, nothing to back-fill.
  let ltName = '';
  let ltRole = '';
  let ltId = null;
  let ltPreview = false;
  onMount(() => { loadTemplates(); });
  $: bands = $templates.filter((t) => templateKind(t) === 'lower-third');
  $: if (ltId == null && bands.length) ltId = bands[0].id;
  $: ltTemplate = bands.find((t) => t.id === ltId) ?? null;
  // What the band would paint, in exactly the shape `fireContent` will send.
  $: ltContent = { reference: ltRole.trim(), text: ltName.trim(), translation: null };
  $: ltReady = !!ltName.trim() && !!ltTemplate;
  const nameToProgramme = () =>
    run(async () => {
      if (!ltReady) return;
      await fireContent(ltRole.trim(), ltName.trim(), 'announce', null, ltTemplate.id);
    });

  // ── QUICK TOOLS · THE EMERGENCY ANNOUNCEMENT ───────────────────────────────
  //
  // It was in Live's inspector column. It paints over live scripture on EVERY
  // screen at once — the fire alarm, the blocked car park — so being reachable
  // only from the tab you happen to be on was the argument for moving it here,
  // not against.
  //
  // ARMED IN TWO STEPS, unchanged: a stray Enter in a text field must not be able
  // to interrupt a reading in front of a room. `pushAnnouncement` THROWS by
  // contract, and `run()` puts the failure in this card's own error line — saying
  // nothing would leave an operator believing the room had been warned.
  let annMsg = '';
  let annArmed = false;
  let annArmT;
  onDestroy(() => clearTimeout(annArmT));
  async function sendAnnouncement() {
    const text = annMsg.trim();
    if (!text) return;
    if (!annArmed) {
      annArmed = true;
      clearTimeout(annArmT);
      annArmT = setTimeout(() => (annArmed = false), 3000);
      return;
    }
    clearTimeout(annArmT);
    annArmed = false;
    await run(async () => {
      await pushAnnouncement(text);
      annMsg = '';
    });
  }

  // ── QUICK TOOLS · LOAD WHOLE PLAN (docs/REBRAND.md §2) ─────────────────────
  //
  // The prototype's one header control. It re-stages the plan the PLANNER handed
  // over — choosing which plan to run is the Planner's job, and `Run in Live`
  // there is what writes `session.planId`. With nothing chosen this button has
  // nothing to load, so it is disabled and says why rather than looking broken.
  //
  // The last plan CHOSEN, not the one currently open: Live's Close plan clears
  // `session.planId`, and a button that went dead the moment an operator closed a
  // plan would be useless in exactly the case it exists for — putting the running
  // order back after the preacher went off it.
  let lastPlanId = null;
  $: if ($session.planId != null) lastPlanId = $session.planId;
  $: planChosen = lastPlanId != null;
  const loadWholePlan = () => {
    if (lastPlanId == null) return;
    setSession({ activeTab: 'live', planId: lastPlanId });
  };

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

  // The controls. `clearScreens` and `blackScreen` return a BOOLEAN and set
  // `panicError` themselves — they are called from a global key handler as well
  // as from here, and neither caller can catch (rule 15). So they are not run
  // through `run()`: their failure is already on the shell's panic banner.
  const doClear = () => clearScreens();
  const doBlack = () => blackScreen();
</script>

<svelte:window on:resize={sizeCanvas} />

<section class="dock" aria-label="Dock">
  <div class="dpanel">
    <div class="dhead">
      <span class="grip" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="dk">Live audio</span>
      <span class="dspring"></span>
      <!-- THE META SLOT IS WHERE THIS CARD SAYS IT HAS NO ANSWER, in the same
           vocabulary the transcript card's `no model` uses: one place per card,
           one kind of sentence, in words.
           With no engine attached, `quiet` and `−∞ dB` are not measurements —
           they read exactly like a live microphone in a silent room, which is the
           one thing they must not be mistaken for (rule 35). So neither is shown;
           the card says what is actually true instead. -->
      {#if !$capture.available}
        <span class="dmeta">no engine</span>
      {:else}
        <span class="vad r-mono" class:on={$meter.isVoice}>{$meter.isVoice ? 'VOICE' : 'quiet'}</span>
        <span class="db r-mono">{dbLabel}</span>
      {/if}
    </div>
    <div class="dbody audbody">
      <div class="wavewrap">
        <!-- The trace itself carries no information a screen reader can use; the
             two facts it illustrates are the VOICE chip and the dB figure in the
             head, both of which are text. -->
        <canvas class="wave" bind:this={cv} aria-hidden="true"></canvas>
        <span class="wavescale" aria-hidden="true"></span>
        <!-- `INPUT`, and no sample rate. The rate IS on the bridge (`audio://chunk`
             carries `sample_rate`) but the console's meter store drops it, so
             printing "48 kHz" here would be a constant that reads the same on a
             device running at 16 — which is the state that silently switches the
             denoiser off. Named in the review note rather than guessed at. -->
        <span class="wavelbl r-mono" aria-hidden="true">INPUT</span>
      </div>
      <!-- THE TWO DECISIONS ABOUT THIS SIGNAL, beside the signal. Both used to be
           three panels away — sensitivity on Live, detection in the Controls card
           — and both are answers to "what should Relay do with what it is
           hearing", which is the question this card is asking. -->
      <div class="audrow">
        <span class="dcap">Sens</span>
        <input
          class="r-range"
          type="range"
          min="0"
          max="100"
          step="1"
          value={sensitivity}
          disabled={!sensReadable}
          aria-label="Detection sensitivity"
          use:rangeFill={sensitivity}
          on:input={(e) => onSensitivity(+e.target.value)} />
        <!-- NUMERIC ONLY. A glyph here cannot tell "nobody answered" from "the
             gate is at 50" (R3-13's rule), and an 18px column cannot hold the
             sentence that could — so the sentence is in the meta slot above and
             this stays a figure. The number shown is the one `getSensitivity`
             returned, fallback included: inventing a distinct "unknown" value
             would put a reading on screen that is nobody's setting. -->
        <span class="sensv r-mono">{sensitivity}</span>
        <button
          class="r-switch"
          class:on={$detectionOn}
          role="switch"
          aria-checked={$detectionOn}
          aria-label="Detection"
          disabled={busy || !$capture.available}
          on:click={() => run(() => setDetection(!$detectionOn))}></button>
        <span class="dcap detl">{$detectionOn ? 'armed' : 'off'}</span>
      </div>
    </div>
  </div>

  <div class="dpanel">
    <div class="dhead">
      <span class="grip" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="dk">Live transcript</span>
      <span class="dspring"></span>
      <span class="dmeta r-mono">{trMeta}</span>
    </div>
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
    <div class="dhead">
      <span class="grip" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="dk">Quick tools</span>
      <span class="dspring"></span>
      <!-- The meta slot says whether the one thing in this card that reaches the
           PREACHER'S monitor is currently on it. Nothing else here claims
           anything. -->
      {#if $stageAlert}<span class="dmeta on-stage r-mono">ON STAGE</span>{/if}
      <!-- `Load whole plan` — the prototype's one header control (§2). -->
      <button
        class="r-btn sm ghost dk-btn"
        on:click={loadWholePlan}
        disabled={!planChosen}
        title={planChosen
          ? 'Put the running order back in the slide grid on Live'
          : 'No plan chosen yet — open Planner and press Run in Live'}>Load whole plan</button>
    </div>
    <div class="dbody tools r-scroll">
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
        <span
          class="tfig r-mono"
          class:live={cdLive}
          class:warn={cdWarn}
          role="status"
          aria-live="off"
          title={cdLive ? 'What the screens are counting, right now.' : 'What Start would put on the screens. Nothing is counting.'}
        >{cdText}</span>
      </div>
      <!-- WHICH of the two facts the figure is. One word, beside it, because a
           big number with no label is the half of a status line that lies. -->
      <div class="trow cdstate">
        <span class="cdstatev" class:live={cdLive}>{cdLive ? 'on the screens' : 'not counting'}</span>
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
      <!-- ── THE NAME BAND (docs/REBRAND.md §2 · §4) ──────────────────────────
           Set once, fired from here. `To programme` goes through `fireContent`
           with the chosen band as the cue's own template, which is the ordinary
           manual-fire path — it reports its own failure and marks nothing amber
           on its own. -->
      <div class="trow ltrow">
        <span>Name band</span>
        <select class="r-select ltpick" bind:value={ltId} aria-label="Which lower third">
          {#each bands as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
        </select>
      </div>
      {#if bands.length}
        <div class="trow ltrow ltsub">
          <input class="r-input tin wide" type="text" bind:value={ltName}
            placeholder="Name" autocomplete="off" aria-label="Name for the lower third" />
          <input class="r-input tin wide" type="text" bind:value={ltRole}
            placeholder="Role or position" autocomplete="off" aria-label="Role for the lower third" />
        </div>
        <div class="trow ltrow ltsub" role="group" aria-label="Name band">
          <button class="r-btn sm ghost" on:click={() => (ltPreview = !ltPreview)}
            disabled={!ltReady}
            title="Render it here. Nothing reaches a screen.">{ltPreview ? 'Hide preview' : 'Preview'}</button>
          <button class="r-btn sm ghost" on:click={nameToProgramme} disabled={busy || !ltReady || !$capture.available}>To programme</button>
        </div>
        {#if ltPreview && ltReady}
          <!-- THE ONE RENDERER, in a 16:9 box. A second way of drawing a template
               is a second thing that can disagree with the wall. It is a preview
               and says so — amber is never used here, because nothing about this
               is on air. -->
          <div class="ltprev">
            <TemplateRender template={ltTemplate} content={ltContent} />
          </div>
          <p class="ltcap">preview only — nothing is on a screen</p>
        {/if}
      {:else}
        <p class="ltcap">No lower third yet — make one in Templates (New → Lower Third).</p>
      {/if}

      <!-- ── WORD TO THE PREACHER (§5) ────────────────────────────────────────
           The stage monitor and nothing else. `sendStageAlert` publishes a frame
           kind that exists inside the stage renderer, so no congregation channel
           can show it — the guarantee is in `channels.rs`, not in this label. -->
      <label class="trow">
        <span>To preacher</span>
        <input
          class="r-input tin wide"
          type="text"
          bind:value={stageMsg}
          placeholder="Wrap up · Five minutes left · Stand by"
          aria-label="Word to the preacher — stage monitor only"
          on:keydown={(e) => e.key === 'Enter' && toPreacher()} />
        <button class="r-btn sm ghost" on:click={toPreacher} disabled={busy || !stageMsg.trim()}>Send to stage</button>
        <button class="r-btn sm ghost" on:click={clearPreacher} disabled={busy || !$stageAlert}>Take down</button>
      </label>

      <!-- ── THE EMERGENCY ANNOUNCEMENT ───────────────────────────────────────
           Over live scripture, on every screen at once. Two steps, always. -->
      <label class="trow">
        <span>Announce</span>
        <input
          class="r-input tin wide"
          type="text"
          bind:value={annMsg}
          placeholder="Message for every screen"
          aria-label="Emergency announcement"
          on:keydown={(e) => e.key === 'Enter' && sendAnnouncement()}
          disabled={!$capture.available} />
        <button class="r-btn sm ghost ann-go" class:armed={annArmed} on:click={sendAnnouncement}
          disabled={busy || !$capture.available || !annMsg.trim()}>
          {annArmed ? 'Confirm?' : 'Send'}
        </button>
      </label>
      {#if err}<p class="derr" role="alert">{err}</p>{/if}
    </div>
  </div>

  <div class="dpanel">
    <div class="dhead">
      <span class="grip" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="dk">Controls</span>
    </div>
    <!-- THREE CONTROLS, THREE COLOURS, NONE SHARED — because the two most
         consequential buttons in the room used to look alike. Clear screens red
         and full width, Blackout black with a hairline, Rehearse amethyst.
         Detection moved one card left, to sit with the signal it is about; it was
         never a control over what a congregation sees, which is what this card
         is for.

         NOT the same set as docs/REBRAND.md §1 and the prototype, and the
         difference is deliberate rather than an oversight. Both have a fourth:
         "Go Live green → End service amber (it owns the on-air session)". Relay
         has no honest state to drive that pair. `start_service` is called from
         `startCapture`, `end_service` only from the service history, and the
         frontend has no way to ASK whether a service is recording —
         `current_service` was deliberately deleted (CLAUDE.md, "No dead-but-built
         commands"). A Go Live / End service button driven by a frontend flag
         would read "Go Live" after a console crash while the service was still
         open in the database: a status control that cannot detect its own
         failure, which is rule 35 and beats the spec.

         This card NEVER scrolls. The buttons stretch to fill whatever height the
         card has, so an operator can never have to scroll to reach Clear screens
         (rule 15's neighbourhood). -->
    <div class="dbody ctlbody">
      <div class="r-ctl">
        <button class="r-cbtn danger wide" on:click={doClear} disabled={!$capture.available}>Clear screens</button>
        <button class="r-cbtn black" data-on={$screenBlack ? '1' : '0'} on:click={doBlack} disabled={!$capture.available}>
          {$screenBlack ? 'Black — restore' : 'Blackout'}
        </button>
        <button
          class="r-cbtn rehearse"
          data-on={$rehearsing ? '1' : '0'}
          on:click={() => run(() => setRehearsal(!$rehearsing))}
          disabled={busy || !$capture.available}
        >{$rehearsing ? 'Rehearsing' : 'Rehearse'}</button>
      </div>
    </div>
  </div>
</section>

<style>
  /* FOUR EQUAL CARDS ON A TROUGH. The row is a fixed 178px (docs/REBRAND.md §2 —
     measured on the prototype, not sketched) and the `gap:1px` over a darker
     ground IS the seam: four instruments in a rack, not four adjacent panels.
     Every card scrolls inside itself; the dock may never grow and take the desk
     with it. */
  .dock {
    flex: 0 0 auto;
    height: 178px;
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(0, 1.5fr) minmax(0, 1.1fr) minmax(0, 1fr);
    gap: 1px;
    background: var(--v-rule);
    border-top: 1px solid var(--v-rule);
    min-height: 0;
  }
  .dpanel {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--v-bg);
  }
  /* The head is its own band on the darker surface, so the four captions line up
     across the row whatever is underneath them. */
  .dhead {
    flex: 0 0 auto;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 26px;
    padding: 3px 9px;
    background: var(--v-surf);
    border-bottom: 1px solid var(--v-rule);
  }
  .dspring { flex: 1; min-width: 0; }
  /* The grip. It does not drag anything and does not pretend to: it is the
     furniture that says "this is a dock panel", the same mark OBS and every desk
     in this genre uses, and it is `aria-hidden` because it is not a control. */
  .grip { display: flex; flex-direction: column; gap: 2px; opacity: .4; flex: 0 0 auto; }
  .grip i { width: 9px; height: 1px; background: var(--v-dim); display: block; }
  .dk {
    font-family: var(--f-mono);
    font-size: var(--v-fs-cap);
    font-weight: 600;
    letter-spacing: var(--v-tr-caps);
    text-transform: uppercase;
    color: var(--v-faint);
    flex: 0 0 auto;
  }
  /* The right-hand meta slot: one short fact about this card, in the head, where
     the eye already is. Truncates rather than wrapping — a wrapped head would
     change the card's height and break the row's rhythm. */
  .dmeta {
    flex: 0 1 auto; min-width: 0; font-size: var(--v-fs-cap); color: var(--v-faint);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  .dmeta.on-stage {
    color: var(--v-red); font-weight: 600; letter-spacing: var(--v-tr-caps);
  }
  .dbody { flex: 1 1 0; min-height: 0; padding: 8px 9px; }

  .audbody { display: flex; flex-direction: column; gap: 7px; }
  .wavewrap {
    position: relative; flex: 1 1 auto; min-height: 44px;
    border: 1px solid var(--v-rule); border-radius: var(--v-r-sm);
    background: linear-gradient(180deg, #0F1116, #0A0B0E); overflow: hidden;
  }
  .wave { display: block; width: 100%; height: 100%; }
  .wavescale {
    position: absolute; inset: 0; pointer-events: none;
    background: repeating-linear-gradient(90deg, rgba(190,205,235,.035) 0 1px, transparent 1px 46px);
  }
  .wavelbl {
    position: absolute; left: 8px; top: 5px; font-size: 8.5px;
    letter-spacing: .09em; color: var(--v-faint); pointer-events: none;
  }
  .audrow { display: flex; align-items: center; gap: 7px; flex: 0 0 auto; }
  .audrow :global(input[type='range']) { flex: 1 1 auto; min-width: 0; }
  .dcap {
    flex: 0 0 auto; font-family: var(--f-mono); font-size: var(--v-fs-cap);
    letter-spacing: var(--v-tr-caps); color: var(--v-faint);
  }
  .dcap.detl { min-width: 34px; }
  .sensv { flex: 0 0 auto; min-width: 18px; text-align: right; font-size: var(--v-fs-cap); color: var(--v-dim); }
  .db { flex: 0 0 auto; font-size: var(--v-fs-cap); color: var(--v-dim); }
  .vad {
    flex: 0 0 auto;
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

  .tools { display: flex; flex-direction: column; gap: 6px; justify-content: flex-start; overflow-y: auto; }
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
  /* THE BIGGEST THING IN THE PANEL. An operator reads this from across a booth,
     so it is a figure, not a chip — mono and tabular so a ticking second never
     reflows the row beside it (docs/REBRAND.md §1). */
  .tfig {
    flex: 0 0 auto; min-width: 0 !important;
    font-variant-numeric: tabular-nums;
    font-size: 21px; line-height: 1; font-weight: 600;
    letter-spacing: .01em;
    /* Dim until it is genuinely on a wall: this is the SET duration then, and a
       setting rendered as brightly as a live figure is the same number telling
       two different stories. */
    color: var(--v-faint);
  }
  .tfig.live { color: var(--v-txt); }
  /* Red = act now. Never amber: amber means ON AIR and nothing else (rule 18). */
  .tfig.warn { color: var(--v-red); }
  .cdstate { margin-top: -2px; }
  .cdstatev {
    min-width: 0 !important;
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
    letter-spacing: var(--v-tr-caps); text-transform: uppercase; color: var(--v-faint);
  }
  .cdstatev.live { color: var(--v-dim); }
  /* Steps, not a fade, and only where motion is welcome: the blink exists to
     catch an eye that is not looking at it, and a viewer who asked for no motion
     still gets the colour, which is the information. */
  @media (prefers-reduced-motion: no-preference) {
    .tfig.warn { animation: cdwarn 2s steps(1) infinite; }
  }
  @keyframes cdwarn { 50% { opacity: .38; } }
  .cdtrans { gap: 4px; flex-wrap: wrap; }
  .cdtrans > :global(button) { flex: 0 0 auto; }
  .tin { width: 62px; flex: 0 0 auto; }
  .tin.wide { flex: 1 1 auto; width: auto; min-width: 0; }
  .derr { margin: 0; font-size: var(--v-fs-cap); color: var(--v-red); }

  /* ── the name band, the announcement, and the header's one button ──────────
     No amber anywhere in this block. Amber means ON AIR and none of these
     controls is a claim that a screen changed — the fires they start report
     their own outcome through `run()` and the error line above. */
  .dk-btn { flex: 0 0 auto; margin-left: 6px; }
  .ltrow > span { min-width: 74px; }
  .ltpick { flex: 1; min-width: 0; height: 24px; }
  /* The rows of fields and buttons have no label of their own, so they line up
     under the one that does rather than starting at the card's edge. */
  .ltsub { padding-left: 80px; }
  .ltprev {
    margin-left: 80px; aspect-ratio: 16 / 9; container-type: inline-size;
    background: var(--v-void); border: 1px solid var(--v-line2);
    border-radius: var(--v-r-sm); overflow: hidden;
  }
  .ltcap {
    margin: 0 0 0 80px;
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
    letter-spacing: var(--v-tr-caps); color: var(--v-faint);
  }
  /* The armed state of a two-step control. RED, not amber: it is "this will
     interrupt the reading", which is an act-now colour, and amber in this room
     means a congregation is already looking at something. */
  .ann-go.armed { border-color: var(--v-red); color: var(--v-red); }

  /* The controls card takes the height it is given and divides it among the
     buttons. `overflow:hidden`, not `auto`: a panic control that can be scrolled
     out of reach is a panic control that will be, exactly once, on a Sunday. */
  .ctlbody { display: flex; flex-direction: column; overflow: hidden; }
  .ctlbody .r-ctl { flex: 1; align-content: stretch; grid-auto-rows: 1fr; gap: 5px; }
  .ctlbody :global(.r-cbtn) { height: auto; min-height: 32px; }
</style>
