<script context="module">
  // ── HOW THE COUNTDOWN FIGURE READS (docs/REBRAND.md §7, L2) ────────────────
  //
  // `auto` | `ms` | `hms`, the three `layers.js::formatCountdown` already takes.
  // The picker feeds THAT function; it does not carry a second copy of the
  // arithmetic, which is the whole of §7's "one formatter".
  //
  // AT MODULE SCOPE FOR THE SAME REASON THE SET DURATION IS (`countdown.js`):
  // the shell renders this component as `{#if !liveFullscreen}<Dock />{/if}`, so
  // pressing Full screen DESTROYS it. A component-local `let` would silently
  // drop an operator's choice mid-service.
  import { writable } from 'svelte/store';
  export const countdownFormat = writable('auto');

  // ── THE WAVEFORM'S TIME BASE (L3) ──────────────────────────────────────────
  //
  // The trace used to be drawn on an EVENT axis: one column per `audio://chunk`,
  // pushed from a `$:` block. Two things were wrong with that, and the operator
  // could see both ("running in bits"):
  //
  //   1. A `$:` block re-runs only when the value it reads is INVALIDATED, and
  //      Svelte's `safe_not_equal` does not invalidate a primitive that has not
  //      changed. Two identical readings in a row — a silent room, a muted
  //      channel — advanced the trace by nothing at all, so it froze.
  //   2. An event axis is not a time axis. `main.rs` emits every THIRD chunk and
  //      `audio::Chunker`'s hop is 200 ms, so a reading arrives about every
  //      600 ms — and only while the path is healthy. A column therefore meant
  //      "one delivery", and a pipeline that had stalled drew a frozen picture
  //      that read exactly like a quiet room.
  //
  // So the axis is now WALL-CLOCK TIME over `WAVE_SPAN_MS`, and these two are
  // pure so they can be asserted without a canvas (jsdom has no 2d context).
  //
  // NOTHING HERE IS A LEVEL THRESHOLD (CLAUDE.md rule 12, DECISIONS §19).
  // `WAVE_GAP_MS` is a timeout on the ARRIVAL of a reading; it compares no
  // signal to any absolute loudness and never decides whether something is
  // speech. `$meter.isVoice` remains the gate's own answer and the only one
  // drawn, in words, in the head.
  export const WAVE_SPAN_MS = 20_000;
  // Longer than three expected deliveries, so ordinary jitter never trips it.
  export const WAVE_GAP_MS = 2_000;

  /**
   * Append one MEASURED reading and drop what has scrolled off the left edge.
   *
   * One reading older than the span is KEPT, deliberately: without it the trace
   * would begin at the first reading inside the window and the left-hand edge
   * would be a made-up zero rather than the real signal running off the side.
   */
  export function pushReading(buf, t, v, spanMs = WAVE_SPAN_MS) {
    const last = buf.length ? buf[buf.length - 1] : null;
    // A clock that went backwards (a resumed laptop, a corrected system time)
    // would otherwise place new readings to the LEFT of old ones and draw the
    // envelope inside out. Start again rather than draw a lie.
    const base = last && last.t > t ? [] : buf;
    const out = [...base, { t, v: Math.max(0, Math.min(1, v)) }];
    const cut = t - spanMs;
    let i = 0;
    while (i + 1 < out.length && out[i + 1].t <= cut) i++;
    return out.slice(i);
  }

  /**
   * The readings, positioned in time and BROKEN WHERE NOTHING WAS MEASURED.
   *
   * `x` is 1 at `now` and 0 one span ago. A pause longer than `gapMs` starts a
   * new segment instead of joining the two: the audio in that gap was never
   * measured, and a line drawn across it would claim a quiet room where the
   * honest picture is an absence. Same rule as `latency.rs` — a stage never
   * reached is an ABSENCE, not a zero.
   */
  export function waveSegments(buf, now, spanMs = WAVE_SPAN_MS, gapMs = WAVE_GAP_MS) {
    const segs = [];
    let cur = null;
    let prev = null;
    for (const r of buf) {
      if (!r || !Number.isFinite(r.t) || !Number.isFinite(r.v)) continue;
      if (prev && r.t - prev.t > gapMs) cur = null;
      if (!cur) {
        cur = [];
        segs.push(cur);
      }
      cur.push({ x: 1 - (now - r.t) / spanMs, v: Math.max(0, Math.min(1, r.v)) });
      prev = r;
    }
    return segs.filter((s) => s.length);
  }

  /**
   * Has the microphone stopped reporting?
   *
   * Only ever asked while capturing. `since` is the moment capture started or
   * the last reading landed, whichever is later — so the answer is "nothing has
   * arrived for longer than three deliveries", never "the room is quiet".
   */
  export function waveStale(since, now, gapMs = WAVE_GAP_MS) {
    return Number.isFinite(since) && now - since > gapMs;
  }
</script>

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
  import { afterUpdate, onDestroy, onMount } from 'svelte';
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
    serviceLock,
    endService,
    setDetection,
    getSensitivity,
    setSensitivity,
    startCountdown,
    adjustCountdown,
    countdownRemaining,
    countdownHeld,
    pauseCountdown,
    sendStageAlert,
    stageAlert,
    templates,
    loadTemplates,
    fireContent,
    setInputDevice,
    startCapture,
    stopCapture,
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
    countdownTotalMs,
    msFromFields,
    fieldsFromMs,
  } from './countdown.js';

  $: lvl = Math.max(0, Math.min(1, $meter.level ?? 0));
  $: dbLabel = lvl > 0.0001 ? `${Math.round(20 * Math.log10(lvl))} dB` : '−∞ dB';

  // ── LIVE TRANSCRIPT · WHAT IT HEARD, WHEN IT HEARD IT (L3) ─────────────────
  //
  // Timestamped lines, newest at the bottom, with whatever is still being said
  // marked `now` and carrying a caret — the prototype's `.trl` / `.trl.cur`.
  // Four lines was a keyhole: a preacher's sentence closes every second or two,
  // so anything an operator looked away from was gone. The pane scrolls itself
  // instead.
  //
  // ZIPPED BEFORE IT IS SLICED, and this is the whole reason it is written this
  // way. `finals` and `finalsAt` are stamped in lockstep at the source
  // (`capture.js`), and the ONE previous consumer that aligned them by length
  // drifted them the moment the rolling cap froze `finals.length` — every line
  // then carried the timestamp of a different line. Pairing by index across the
  // FULL arrays and slicing the pairs cannot reproduce that.
  const TR_LINES = 40;
  $: tlines = $transcript.finals
    .map((t, i) => ({ t, at: $transcript.finalsAt?.[i] ?? '' }))
    .slice(-TR_LINES);

  // ── THE PANE FOLLOWS THE PREACHER, UNLESS THE OPERATOR IS READING BACK ─────
  //
  // `afterUpdate`, never `tick()` inside a `$:` block — that re-enters Svelte's
  // scheduler and hard-freezes the webview with no error (rule 1), and this is
  // the one surface that updates several times a second.
  //
  // It sticks to the bottom only while it was ALREADY at the bottom. An operator
  // who has scrolled up is looking for something; yanking them back to the live
  // line every 200 ms would make the history unreadable in exactly the moment
  // they needed it.
  let trBody = null;
  let trStuck = true;
  const TR_SLACK = 24;
  function onTrScroll() {
    if (!trBody) return;
    trStuck = trBody.scrollHeight - trBody.scrollTop - trBody.clientHeight <= TR_SLACK;
  }
  afterUpdate(() => {
    if (trStuck && trBody) trBody.scrollTop = trBody.scrollHeight;
  });
  // What the transcript is being produced BY. Not a decoration: the language is
  // re-elected every window on accented speech (`stt://language_unstable`), and
  // "local" is the offline-first promise stated where an operator can see it.
  // With no model loaded it says so — the alternative reads identically to a
  // working recogniser that simply has not heard anything yet (rule 35).
  $: trMeta = $capture.stt?.loaded
    ? `local · ${$capture.detectedLang || $capture.stt?.language || 'auto'}`
    : 'no model';

  // ── THE FOURTH CONTROL · END SERVICE (docs/REBRAND.md §1) ──────────────────
  //
  // "Go Live green → End service amber (it owns the on-air session)". Relay has
  // no Go Live: a service row is opened by `startCapture`, so the pair is one
  // button, and the half that exists is the half that closes the record.
  //
  // This card declined it for a whole wave, and the reason it gave was right at
  // the time: "Relay has no honest state to drive that pair … a Go Live / End
  // service button driven by a frontend flag would read Go Live after a console
  // crash while the service was still open in the database". That is rule 35.
  //
  // It now has one. `service_lock` carries `recording`, read from the session
  // Rust itself clears in `end_service` — not from `engaged`, which the operator
  // can lift mid-service and which would then say there is nothing to end while
  // the record is open (`e2e::r3_the_service_is_still_recording_after_the_operator_lifts_the_lock`).
  // The shell re-polls it every five seconds, so a service started or ended
  // anywhere else in the app reaches this button on its own.
  //
  // AMBER ONLY WHILE IT IS TRUE. §1 calls the button amber; CLAUDE.md rule 18
  // says amber IS on air and is never allowed to mean anything else. Both are
  // satisfied by making the colour the STATE rather than the button: amber while
  // a service is recording, an ordinary dock button when there is nothing to
  // end. Green — the prototype's Go Live — is not in Relay's colour law and does
  // not appear.
  //
  // It reports its own outcome by not changing: `endService` swallows a backend
  // failure (it is a history control, not a panic control, and it must never take
  // the console down), so the proof is the next poll. If the service did not end,
  // the button still says End service and still burns amber.
  $: recording = !!$serviceLock.recording;

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
  // Every point is a REAL measurement — `chunk.rms` over one 200 ms window of the
  // CLEANED stream, the same signal the voice gate and whisper see. Nothing is
  // decayed, smoothed or held.
  //
  // ── WHAT IT CANNOT SHOW, AND WHY THAT IS NOT A FRONTEND BUG ────────────────
  //
  // `main.rs::start_capture` emits `audio://chunk` on every THIRD chunk, so two
  // thirds of the stream never crosses the bridge at all. The trace is therefore
  // a reading every ~600 ms, not a continuous envelope, and no amount of drawing
  // can recover what was not sent. What the drawing CAN do — and now does — is
  // put each reading at the time it was taken, scroll at wall-clock rate, and
  // decline to join two readings across a silence nobody measured. The comment
  // this replaced claimed "every 200 ms hop" and "about 38 seconds"; both were
  // wrong (it is ~600 ms, and 190 of those is nearly two minutes).
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
  // ── THE ONE ANIMATION LOOP, AND WHAT IT COSTS ──────────────────────────────
  //
  // A time axis has to be repainted on a clock rather than on an event, or the
  // trace stands still between deliveries and then jumps 600 ms to the left —
  // which is the "running in bits" the operator reported. So there IS a frame
  // loop now, and the honest accounting is: it runs ONLY while the microphone is
  // live, it is throttled to ~20 fps, and it is cancelled on stop and on
  // destroy. An idle console still costs nothing.
  //
  // It is not decorative motion — it is the reading — so it is not suppressed
  // under `prefers-reduced-motion`; a still picture of a live signal would be
  // the defect, not the courtesy.
  const FRAME_MS = 50;
  let waveBuf = [];
  let cv = null;
  let cw = 0;
  let ch = 0;
  let raf = 0;
  let lastFrame = 0;
  // The moment capture started, or the last reading, whichever is later. Capture
  // start seeds it so the first ~2 s of a service is not reported as a fault.
  let signalSince = 0;
  // A plain `let` driven by the 500 ms tick below, never `tick()` from a `$:`
  // block — re-entering Svelte's scheduler there hard-freezes the webview
  // (rule 1).
  $: micLive = $capture.capturing && $capture.available;
  $: noSignal = micLive && waveStale(signalSince, nowTick);

  function onReading(m) {
    const now = Date.now();
    waveBuf = pushReading(waveBuf, now, m?.level ?? 0);
    signalSince = now;
    // Repaint immediately when the loop is not running, so a reading taken with
    // the microphone stopped (the reset `stopCapture` performs) still lands.
    if (!raf) draw();
  }

  function frame(ts) {
    raf = requestAnimationFrame(frame);
    if (ts - lastFrame < FRAME_MS) return;
    lastFrame = ts;
    draw();
  }
  function startLoop() {
    if (!raf && typeof requestAnimationFrame === 'function') raf = requestAnimationFrame(frame);
  }
  function stopLoop() {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    draw();
  }
  // Only on the TRANSITION, so starting the microphone re-seeds the grace window
  // rather than inheriting a `signalSince` from the previous service — which
  // would report `no signal` on the first frame of the next one.
  let wasLive = false;
  $: if (micLive !== wasLive) {
    wasLive = micLive;
    if (micLive) {
      signalSince = Date.now();
      startLoop();
    } else {
      stopLoop();
    }
  }
  onDestroy(stopLoop);

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
    // jsdom has no 2d context, so the pure half of this — `pushReading`,
    // `waveSegments`, `waveStale` — is what the tests assert.
    if (!cx) return;
    const mid = ch / 2;
    cx.clearRect(0, 0, cw, ch);

    const g = cx.createLinearGradient(0, 0, cw, 0);
    g.addColorStop(0, 'rgba(63,207,106,.10)');
    g.addColorStop(0.72, 'rgba(63,207,106,.34)');
    g.addColorStop(1, 'rgba(63,207,106,.62)');

    // ONE mirrored envelope per segment rather than a bar per reading: a trace
    // reads as a signal, a picket fence reads as a chart. A segment BREAK is
    // audio nobody measured, and it is drawn as a break — see `waveSegments`.
    for (const seg of waveSegments(waveBuf, Date.now())) {
      const px = (p) => p.x * cw;
      const up = (p) => mid - p.v * mid * 0.94;
      const dn = (p) => mid + p.v * mid * 0.94;
      cx.beginPath();
      cx.moveTo(px(seg[0]), up(seg[0]));
      for (let i = 1; i < seg.length; i++) {
        const a = seg[i - 1];
        const b = seg[i];
        cx.quadraticCurveTo((px(a) + px(b)) / 2, up(a), px(b), up(b));
      }
      for (let i = seg.length - 1; i >= 0; i--) cx.lineTo(px(seg[i]), dn(seg[i]));
      cx.closePath();
      cx.fillStyle = g;
      cx.fill();
      cx.strokeStyle = 'rgba(63,207,106,.85)';
      cx.lineWidth = 1.1;
      cx.stroke();
    }

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
  // The readings, taken from the store rather than from a `$:` on the derived
  // level — see the module block: an identical consecutive reading does not
  // invalidate a primitive, so the trace used to stand still through exactly the
  // stretch an operator is asking about. A `writable` set with a fresh object
  // notifies every time, which is the property this relies on.
  onMount(() => meter.subscribe(onReading));

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
  // ── THE MICROPHONE, ON THE RUN SURFACE (L3, operator instruction) ──────────
  //
  // The device list and the on/off already existed — in Settings → Audio, three
  // workspaces away from the person watching the level. A volunteer who picks
  // the wrong input at 10:29 had to leave the run surface to fix it. Same store,
  // same wrappers, same contracts; this is a second door onto one control, not a
  // second control (rule 35's cousin — two opinions about one fact is the bug).
  //
  // THE DEVICE IS CHOSEN BEFORE CAPTURE, NOT DURING IT. `start_capture` takes
  // the device name as an argument, so changing it mid-capture would change a
  // label and nothing else. Disabled while live, exactly as Settings has it.
  //
  // COLOUR: the toggle is emerald when open and the card's steel when closed,
  // both already in this card (the voice chip is the same emerald). AMBER IS ON
  // AIR and is not spent on a microphone (rule 18); a live mic is not a
  // congregation looking at something.
  //
  // It cannot claim a success it did not have: `startCapture`/`stopCapture` both
  // THROW (contract group 1), `run()` puts the failure in this card's error
  // line, and the LABEL is driven by `$capture.capturing` — which a failed stop
  // deliberately leaves set (see `stopCapture`'s own note). So a stop that did
  // not stop still reads Listening over a live microphone, which is the truth.
  const toggleMic = () =>
    run(async () => {
      if ($capture.capturing) await stopCapture();
      else await startCapture($capture.inputDevice || null);
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
  // HELD, read from the content on the wall rather than from a flag this panel
  // keeps. A transport that remembered its own hold would go on saying "Resume"
  // over a countdown some other surface released — rule 35, on the one control
  // row an operator watches a service from.
  $: cdPaused = !!$live && countdownHeld();
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
  // ONE FORMATTER, ASKED A QUESTION (§7). `$countdownFormat` is the third
  // argument `formatCountdown` has always taken; nothing here re-derives hours,
  // minutes or seconds.
  //
  // WHAT THIS PICKER DOES **NOT** REACH, said plainly: the screens. A wall's
  // countdown is rendered by `TemplateRender` from `OutputContent`, which carries
  // no format field, so making the choice follow the content would take a column
  // on the broadcast and an edit to the one renderer — neither of which is this
  // agent's to make. It changes the notation of the CONSOLE'S readout of the same
  // number, and the control says so where an operator can read it. Recorded in
  // the review note as the backend half that is still owed.
  $: cdText = formatCountdown(cdLive ? cdRunning : $countdownSet, $countdownFormat);
  // What Start would put up, in the caption beside the name. Only while something
  // IS counting: off air the big figure below already IS the set duration, and the
  // same number twice in one block reads as two facts.
  $: cdSetLabel = formatCountdown($countdownSet, $countdownFormat);
  // The last minute — or the last tenth of a short countdown, because a minute's
  // warning on a two-minute countdown is a colour that is on for half of it
  // (`layers.js`). RED, not amber: amber in this room means ON AIR and is never
  // allowed to be anything else (rule 18), and "this is about to run out" is the
  // act-now colour. Only ever while it is genuinely on a wall.
  // THE TOTAL IS THE CONTENT'S, NOT THE TOOL'S. `countdownWarning` scales the
  // last-minute threshold to the countdown's own span, and `$countdownSet` is
  // what Start WOULD put up — a different number the moment an operator types in
  // the fields while one is running, or ±1s one that started somewhere else. The
  // engine now carries the real span, so the warning is read from there and the
  // figure in the dock and the figure on the wall turn red together.
  $: cdTotal = countdownTotalMs($live) ?? $countdownSet;
  $: cdWarn = cdLive && countdownWarning(cdRunning, cdTotal);

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
    const r = countdownPress(action, $countdownSet, cdRunning, cdPaused);
    countdownSet.set(r.setMs);
    if (r.refused) {
      err = r.refused;
      return;
    }
    // HOLD AND RELEASE. The one press here that is not a re-aim: it changes no
    // number, it asks the engine to set `countdown_paused_ms`, and it is TWO
    // actions rather than a toggle — a toggle computed from state this panel
    // might hold stale is how a press does the opposite of what it says.
    if (r.pause !== null) {
      run(() => pauseCountdown(r.pause));
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

  // ── THE EMERGENCY ANNOUNCEMENT IS NOT HERE, AND THAT IS DELIBERATE ─────────
  //
  // Removed from Quick tools on the operator's instruction (2026-09-14, L3). It
  // was the fourth thing in a card §2 defines as "the three things that change
  // during a service", and it is the only one of the four that paints over live
  // scripture on every screen at once.
  //
  // WHAT THAT LEAVES, said plainly rather than left to be discovered: the
  // `push_announcement` command is still registered in Rust and
  // `capture.js::pushAnnouncement` still wraps it, so `ipc.test.js` is satisfied
  // and `scripts/qa-inventory.mjs` still counts it addressed — but NOTHING
  // RENDERED REACHES IT any more. By this repository's own standard that is
  // attack surface nobody is watching, and the precedent for the five commands
  // deleted on 2026-08-30 is to delete it rather than to hide it. That is a
  // change to `src-tauri/` and to another agent's file, so it is recorded for
  // the lead to decide rather than taken here.
  //
  // `announce.test.js`, `r2livepath.test.js` and `qa-r5-groups.test.js` still
  // hold the WRAPPER's throw contract, which is correct: the contract is about
  // the wrapper, not about this card.

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
      {:else if noSignal}
        <!-- THE MICROPHONE IS LIVE AND NOTHING IS ARRIVING. `quiet` and `−∞ dB`
             would be the same two words a silent room gets, over a path that has
             stopped delivering — one sentence for two opposite situations is
             exactly what rule 35 forbids. It says nothing about loudness: it is a
             timeout on the ARRIVAL of a reading (rule 12 is untouched). -->
        <span class="dmeta nosig r-mono">no signal</span>
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
        <!-- `INPUT`, THE SPAN, and no sample rate. The span is printed because
             the axis is now time and a picture that does not state its own scale
             cannot be read: the same trace at 20s and at 2 minutes tells two
             different stories about the same signal.
             The RATE is still absent. It IS on the bridge (`audio://chunk`
             carries `sample_rate`) but the console's meter store drops it, so
             printing "48 kHz" here would be a constant that reads the same on a
             device running at 16 — which is the state that silently switches the
             denoiser off. Named in the review note rather than guessed at. -->
        <span class="wavelbl r-mono" aria-hidden="true">INPUT · {WAVE_SPAN_MS / 1000}s</span>
      </div>
      <!-- THE TWO DECISIONS ABOUT THIS SIGNAL, beside the signal. Both used to be
           three panels away — sensitivity on Live, detection in the Controls card
           — and both are answers to "what should Relay do with what it is
           hearing", which is the question this card is asking. -->
      <!-- THE MICROPHONE, beside the signal it produces. Which input, and
           whether it is open: the two facts a volunteer needs at 10:29 and had
           to leave the run surface to reach. Same store and same wrappers as
           Settings → Audio (see the note above `toggleMic`).
           The device is fixed for the life of a capture, because that is what
           `start_capture` takes — so the picker is disabled while listening
           rather than silently doing nothing. -->
      <div class="audrow">
        <span class="dcap">Mic</span>
        <select
          class="r-select micpick"
          value={$capture.inputDevice}
          on:change={(e) => setInputDevice(e.target.value)}
          disabled={!$capture.available || $capture.capturing}
          title={$capture.capturing
            ? 'Stop listening to change the microphone — the device is chosen when capture opens.'
            : 'Which microphone Relay opens when you start listening.'}
          aria-label="Microphone input device">
          <option value="">System default</option>
          {#each $capture.devices as d (d.name)}<option value={d.name}>{d.name}</option>{/each}
        </select>
        <!-- AN ICON TOGGLE, NOT A PILL (C2, operator instruction 2026-09-14).
             What it DRAWS is the state: a struck-through microphone is closed, a
             plain one is open. The word beside it and the `title` say the same
             thing in Relay's own voice, so the control is still readable with no
             colour at all — which a bare tinted square would not be.
             Emerald, never amber. Amber is ON AIR and a live microphone is not a
             congregation looking at something (rule 18). -->
        <button
          class="r-iconbtn audtog"
          class:on={$capture.capturing}
          role="switch"
          aria-checked={$capture.capturing}
          aria-label="Microphone"
          title={$capture.capturing
            ? 'Microphone open — Relay is listening. Press to stop.'
            : 'Microphone closed — Relay is not listening. Press to start.'}
          disabled={busy || !$capture.available}
          on:click={toggleMic}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <rect x="9" y="2" width="6" height="11" rx="3" />
            <path d="M5 11a7 7 0 0 0 14 0" />
            <path d="M12 18v3" />
            {#if !$capture.capturing}<path d="M4 3.5l16 17" />{/if}
          </svg>
        </button>
        <!-- THE WORDS THE INSTRUCTIONS NAME. The guide, the Help screen, the
             latency panel and the empty History pane all say "press Start
             listening", and the only control whose VISIBLE text said it lives
             inside Settings — on Live it was an unlabelled glyph whose caption
             read `off`, which reads as a status, not a button. This is the first
             action of every service. `Listen` / `Listening` is the same two
             states in the words the volunteer was told to look for. -->
        <span class="dcap detl">{$capture.capturing ? 'Listening' : 'Listen'}</span>
      </div>
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
        <!-- THE SAME INSTRUMENT, ONE ROW DOWN. A struck-through target is a
             detector that is not looking; an open one is armed. Emerald and
             steel, the two colours already in this card — amber is ON AIR and
             arming detection is not a claim about a screen (rule 18). -->
        <button
          class="r-iconbtn audtog"
          class:on={$detectionOn}
          role="switch"
          aria-checked={$detectionOn}
          aria-label="Detection"
          title={$detectionOn
            ? 'Detection armed — Relay is matching what it hears against scripture. Press to turn it off.'
            : 'Detection off — nothing is being matched against scripture. Press to arm it.'}
          disabled={busy || !$capture.available}
          on:click={() => run(() => setDetection(!$detectionOn))}>
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
            stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="12" cy="12" r="8.5" />
            <circle cx="12" cy="12" r="2.5" />
            {#if !$detectionOn}<path d="M4 3.5l16 17" />{/if}
          </svg>
        </button>
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
    <div class="dbody tbody r-scroll" bind:this={trBody} on:scroll={onTrScroll}>
      {#each tlines as l, i (i + '·' + l.at)}
        <p class="trl"><span class="tt r-mono">{l.at}</span><span class="tx">{l.t}</span></p>
      {/each}
      {#if $transcript.partial}
        <!-- WHAT IS BEING SAID RIGHT NOW. `now` rather than a clock time,
             because it has not closed yet and stamping it would date a line that
             is still being revised. Steel — the thing being worked on — never
             cyan (a guess about scripture) and never amber. -->
        <p class="trl cur"><span class="tt r-mono">now</span><span class="tx">{$transcript.partial}<i class="caret" aria-hidden="true"></i></span></p>
      {/if}
      {#if !tlines.length && !$transcript.partial}
        <p class="trl empty">{$capture.capturing ? 'listening…' : 'not listening'}</p>
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
           then Start · Pause · Reset · ±1 · Clear. The figure on the right is the
           one on the wall — same field, same formatter — not a second clock.

           ── ONE INSTRUMENT, THREE TIMES (L3, operator instruction 2026-09-14) ──
           L2 gave each tool an edge of its own and stopped there, so the three
           still read as three degrees of finish: the countdown sat on a different
           ground with a different hairline and a different corner from the other
           two, its caption was a different class, its fields were a bare row
           while the name band's were indented 80px under a label that was not
           there, and the three button rows were a wrapping flex, a plain flex and
           a two-column grid.

           The prototype's `.tmr` / `.lt3` / `.alrt` are the SAME CARD three
           times — one ground, one 7px padding, one head (mono caption left, its
           own control or badge right), one 26px full-width field, one grid button
           row at 5px. That is what `.qblock` now is, and all three use it. The
           per-tool classes that remain (`.tmr`, `.onstage`) carry only what is
           genuinely that tool's: the countdown's figure, the alert's red. -->
      <div class="qblock tmr">
        <div class="qhead">
          <span class="r-lbl">Countdown</span>
          <!-- WHAT IS LOADED, while the figure beside it shows what is LEFT. -->
          {#if cdLive}<span class="cdset r-mono">· {cdSetLabel}</span>{/if}
          <span class="qspring"></span>
          <!-- THE FIGURE AND THE WORD THAT SAYS WHICH FIGURE IT IS, TOGETHER
               (C2, operator instruction 2026-09-14). The state line used to be a
               row of its own BETWEEN the fields and the transport, where it read
               as a caption for neither: an orphaned `NOT COUNTING` under a set of
               number boxes it says nothing about. It is a label for the figure,
               so it lives under the figure. -->
          <span class="cdfig">
            <span
              class="tfig r-mono"
              class:live={cdLive}
              class:warn={cdWarn}
              role="status"
              aria-live="off"
              title={cdLive ? 'What the screens are counting, right now.' : 'What Start would put on the screens. Nothing is counting.'}
            >{cdText}</span>
            <!-- WHICH of the two facts the figure is. One word, beside it, because a
                 big number with no label is the half of a status line that lies.
                 THREE states, not two. A held countdown IS on the screens — it simply
                 is not moving — and reading "on the screens" over a stopped figure is
                 the half of a status line that lies (rule 35). -->
            <span class="cdstatev" class:live={cdLive} class:held={cdPaused}
              >{!cdLive ? 'not counting' : cdPaused ? 'on the screens · held' : 'on the screens'}</span>
          </span>
        </div>
        <div class="qrow">
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
          <!-- SET IT, DO NOT ONLY NUDGE IT (§7). A pre-service countdown and a
               90-minute service are both timers, and `5:00` and `0:05:00` are the
               same number read two ways. The picker is the third argument
               `formatCountdown` already takes — there is no second formatter here
               and there must never be one.
               THE TITLE SAYS WHAT IT GOVERNS. It changes this readout, not a
               screen: the wall renders its countdown from `OutputContent`, which
               carries no format, so a control that implied otherwise would be
               claiming a reach it has not got (rule 35's family). -->
          <select
            class="r-select cdfmt"
            bind:value={$countdownFormat}
            aria-label="Countdown format"
            title="How this readout reads. The screens read the countdown through their own template.">
            <option value="auto">auto</option>
            <option value="ms">m:ss</option>
            <option value="hms">h:mm:ss</option>
          </select>
        </div>
        <!-- Clear is NOT Clear screens. It returns this tool to its default length
             and touches nothing a congregation can see; the red control one panel
             along is the one that blanks a wall. -->
        <div class="qbtns cdtrans" role="group" aria-label="Countdown transport">
          <button class="r-btn sm ghost" on:click={() => press('start')}
            disabled={busy || !$capture.available || !countdownCan('start', $countdownSet, cdRunning, cdPaused)}>Start</button>
          <!-- PAUSE AND RESUME ARE TWO ACTIONS, NOT A TOGGLE (§7, and the engine
               field that finally made it possible). Which one is offered is read
               from the CONTENT on the wall, so a press can never do the opposite of
               what its label says; with nothing counting, neither is available and
               `countdownCan` says so through the same refusal the press would give.
               Nothing here is amber: holding a countdown does not change what is on
               air, it changes whether it is moving. -->
          {#if cdPaused}
            <button class="r-btn sm ghost" on:click={() => press('resume')}
              title="Let the countdown on the screens carry on from where it was held"
              disabled={busy || !$capture.available || !countdownCan('resume', $countdownSet, cdRunning, cdPaused)}>Resume</button>
          {:else}
            <button class="r-btn sm ghost" on:click={() => press('pause')}
              title="Hold the countdown on the screens at exactly what it says"
              disabled={busy || !$capture.available || !countdownCan('pause', $countdownSet, cdRunning, cdPaused)}>Pause</button>
          {/if}
          <button class="r-btn sm ghost" on:click={() => press('reset')}
            disabled={busy || !$capture.available || !countdownCan('reset', $countdownSet, cdRunning, cdPaused)}>Reset</button>
          <button class="r-btn sm ghost" on:click={() => press('minus')} aria-label="One minute less"
            disabled={busy || !$capture.available || !countdownCan('minus', $countdownSet, cdRunning, cdPaused)}>−1</button>
          <button class="r-btn sm ghost" on:click={() => press('plus')} aria-label="One minute more"
            disabled={busy || !$capture.available || !countdownCan('plus', $countdownSet, cdRunning, cdPaused)}>+1</button>
          <button class="r-btn sm ghost" on:click={() => press('clear')}
            title="Reset this tool to five minutes. It does not clear the screens.">Clear</button>
        </div>
      </div>
      <!-- ── THE NAME BAND (docs/REBRAND.md §2 · §4) ──────────────────────────
           Set once, fired from here. `To programme` goes through `fireContent`
           with the chosen band as the cue's own template, which is the ordinary
           manual-fire path — it reports its own failure and marks nothing amber
           on its own. -->
      <!-- THE SAME CARD AS THE COUNTDOWN ABOVE AND THE ALERT BELOW (L3). The
           picker is the head's right-hand slot, exactly where the countdown puts
           its figure and the alert puts its badge; the two fields are the
           prototype's stacked full-width `.tin`s rather than a pair squeezed
           into a ~200px card under an 80px indent that had no label above it. -->
      <div class="qblock">
        <div class="qhead">
          <span class="r-lbl">Name band</span>
          <span class="qspring"></span>
          <select class="r-select ltpick" bind:value={ltId} aria-label="Which lower third">
            {#each bands as t (t.id)}<option value={t.id}>{t.name}</option>{/each}
          </select>
        </div>
      {#if bands.length}
        <input class="r-input tin wide" type="text" bind:value={ltName}
          placeholder="Name" autocomplete="off" aria-label="Name for the lower third" />
        <input class="r-input tin wide" type="text" bind:value={ltRole}
          placeholder="Role or position" autocomplete="off" aria-label="Role for the lower third" />
        <div class="qbtns" role="group" aria-label="Name band">
          <button class="r-btn sm ghost" on:click={() => (ltPreview = !ltPreview)}
            disabled={!ltReady}
            title="Render it here. Nothing reaches a screen.">{ltPreview ? 'Hide preview' : 'Preview'}</button>
          <button class="r-btn sm primary" on:click={nameToProgramme} disabled={busy || !ltReady || !$capture.available}>To programme</button>
        </div>
        {#if ltPreview && ltReady}
          <!-- THE ONE RENDERER, in a 16:9 box. A second way of drawing a template
               is a second thing that can disagree with the wall. It is a preview
               and says so — amber is never used here, because nothing about this
               is on air. -->
          <div class="ltprev">
            <TemplateRender template={ltTemplate} content={ltContent} />
          </div>
          <p class="qcap">preview only — nothing is on a screen</p>
        {/if}
      {:else}
        <p class="qcap">No lower third yet — make one in Templates (New → Lower Third).</p>
      {/if}
      </div>

      <!-- ── WORD TO THE PREACHER (§5) ────────────────────────────────────────
           The stage monitor and nothing else. `sendStageAlert` publishes a frame
           kind that exists inside the stage renderer, so no congregation channel
           can show it — the guarantee is in `channels.rs`, not in this label. -->
      <div class="qblock" class:onstage={$stageAlert}>
        <div class="qhead">
          <span class="r-lbl">Word to the preacher</span>
          <span class="qspring"></span>
          <!-- The badge says WHERE it is, and only while it is there. Amethyst and
               amber are both spoken for; this is the stage's own red, which is what
               the preacher's monitor is painting at that moment. -->
          {#if $stageAlert}<span class="qbadge">on stage</span>{/if}
        </div>
        <input
          class="r-input tin wide"
          type="text"
          bind:value={stageMsg}
          placeholder="Wrap up · Five minutes left · Stand by"
          aria-label="Word to the preacher — stage monitor only"
          on:keydown={(e) => e.key === 'Enter' && toPreacher()} />
        <!-- `primary`, not `pri`. `pri` is not a class this stylesheet defines,
             so the one button in Quick tools that is meant to read as the
             primary action had been rendering as a plain `.r-btn` — invisible,
             because a plain button is a perfectly ordinary thing to look at. -->
        <div class="qbtns">
          <button class="r-btn sm primary" on:click={toPreacher} disabled={busy || !stageMsg.trim()}>Send to stage</button>
          <button class="r-btn sm ghost" on:click={clearPreacher} disabled={busy || !$stageAlert}>Take down</button>
        </div>
      </div>
      {#if err}<p class="derr" role="alert">{err}</p>{/if}
    </div>
  </div>

  <div class="dpanel ctl">
    <div class="dhead">
      <span class="grip" aria-hidden="true"><i></i><i></i><i></i></span>
      <span class="dk">Controls</span>
    </div>
    <!-- FOUR CONTROLS, FOUR COLOURS, NONE SHARED (docs/REBRAND.md §1) — because
         the two most consequential buttons in the room used to look alike. Clear
         screens red and full width, Blackout black with a hairline, Rehearse
         amethyst, End service amber while a service is recording. Detection moved
         one card left, to sit with the signal it is about; it was never a control
         over what a congregation sees, which is what this card is for.

         THE ORDER IS THE PROTOTYPE'S, on the operator's instruction (2026-09-14):
         the session control on top, the two state controls as a pair, and Clear
         screens full width along the bottom edge. The earlier order put the red
         one first on the argument that the control reached for without reading
         belongs under the thumb; the bottom edge of a card that never scrolls is
         the same distance from a thumb and is where the prototype puts it.

         What rule 15 actually requires is unchanged and still holds: this card
         never scrolls, Clear screens is never behind an overflow edge, and at one
         column the whole card is ordered first (see the 640px rule below, pinned
         by `panic.test.js`).

         This card NEVER scrolls. The buttons stretch to fill whatever height the
         card has, so an operator can never have to scroll to reach Clear
         screens. -->
    <div class="dbody ctlbody">
      <div class="r-ctl">
        <!-- The label is the STATE, like Blackout and Rehearse below it. With no
             service open it says so and is inert: "End service" over nothing to
             end reads exactly like "End service" over a recording church, which
             is the one thing this button may not do. -->
        <button
          class="r-cbtn endsvc wide"
          data-on={recording ? '1' : '0'}
          on:click={() => run(endService)}
          disabled={busy || !recording || !$capture.available}
          title={recording
            ? 'Stop recording this service. The transcript, the fires and the timeline are kept — Library → History reads them back.'
            : 'No service is being recorded. One starts when you start listening.'}
        >{recording ? 'End service' : 'No service'}</button>
        <button
          class="r-cbtn rehearse"
          data-on={$rehearsing ? '1' : '0'}
          on:click={() => run(() => setRehearsal(!$rehearsing))}
          disabled={busy || !$capture.available}
        >{$rehearsing ? 'Rehearsing' : 'Rehearse'}</button>
        <button class="r-cbtn black" data-on={$screenBlack ? '1' : '0'} on:click={doBlack} disabled={!$capture.available}>
          {$screenBlack ? 'Black — restore' : 'Blackout'}
        </button>
        <button class="r-cbtn danger wide" on:click={doClear} disabled={!$capture.available}>Clear screens</button>
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
  /* NARROW. Four instruments in a rack need a rack: below 900px the row's own
     columns are too thin for their heads (measured at 768: `Load whole plan`
     overlapped the next card's CONTROLS caption), and at phone width `Clear
     screens` was sliced by the card's edge — a panic control an operator cannot
     read is the defect rule 15 exists to prevent. Two columns at 900, one at
     640, and the dock scrolls rather than clipping.

     `height:auto` with a max is deliberate: the fixed 178px is a rack height for
     a desk, and stacked cards on a phone need their own. */
  @media (max-width: 900px) {
    .dock {
      grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
      height: auto;
      max-height: 46vh;
      overflow-y: auto;
    }
    /* A stacked card sizes to its CONTENT. `.dbody` is `flex:1 1 0` for the rack
       layout, where the card has a known 178px to divide; in a grid row of auto
       height that basis collapses the body to nothing — measured at 768 as a
       **19px body over 122px of content**, which put the whole Controls card,
       Clear screens included, behind an `overflow:hidden` edge. That is the
       failure rule 15 exists to prevent, and it was introduced by the stacking
       rule above, so it is fixed here rather than anywhere else. */
    .dpanel { min-height: 150px; }
    .dbody { flex: 1 1 auto; }
  }
  @media (max-width: 640px) {
    .dock { grid-template-columns: minmax(0, 1fr); }
    /* ONE COLUMN PUTS THE PANIC CONTROL LAST, and a panic control an operator
       has to scroll to is not a panic control (rule 15, DECISIONS §20).
       Measured at 430: `Clear screens` sat inside its card and BELOW the
       viewport. In a stack the Controls card goes first; the meters and the
       transcript are things you read, and reading can scroll. */
    .dpanel.ctl { order: -1; }
  }
  /* A head that has to choose between its caption and its button wraps rather
     than overlapping the card beside it. */
  .dhead { flex-wrap: wrap; row-gap: 2px; }
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
  /* A live microphone delivering nothing. RED because it is act-now — a service
     is being recorded and none of it is arriving — and never amber, which means
     a congregation is looking at something (rule 18). */
  .dmeta.nosig {
    color: var(--v-red); font-weight: 600; letter-spacing: var(--v-tr-caps);
    text-transform: uppercase;
  }
  .dbody { flex: 1 1 0; min-height: 0; padding: 8px 9px; }

  .audbody { display: flex; flex-direction: column; gap: 7px; }
  .wavewrap {
    position: relative; flex: 1 1 auto; min-height: 44px;
    border: 1px solid var(--v-rule); border-radius: var(--v-r-sm);
    background: linear-gradient(180deg, var(--v-void), var(--v-rule)); overflow: hidden;
  }
  .wave { display: block; width: 100%; height: 100%; }
  .wavescale {
    position: absolute; inset: 0; pointer-events: none;
    background: repeating-linear-gradient(90deg, rgba(190,205,235,.035) 0 1px, transparent 1px 46px);
  }
  .wavelbl {
    position: absolute; left: 8px; top: 5px; font-size: var(--v-fs-kind);
    letter-spacing: .09em; color: var(--v-faint); pointer-events: none;
  }
  .audrow { display: flex; align-items: center; gap: 7px; flex: 0 0 auto; }
  .audrow :global(input[type='range']) { flex: 1 1 auto; min-width: 0; }
  /* The device names a church actually has are long ("MacBook Pro Microphone",
     "Scarlett 2i2 USB"). It takes the row's spare width and truncates rather than
     pushing the switch off the card's edge. Width only — the height is the
     shared control's, which is the whole point of the four-row column here:
     select · slider · switch all land on one line. */
  .micpick { flex: 1 1 auto; min-width: 0; }
  /* ── THE ICON TOGGLE (C2, operator instruction 2026-09-14) ────────────────
     "I will prefer to have an icon toggle button rather than having this big
     switch on the audio section." The 38x21 pill is a good instrument in a
     Settings column, where a stack of them lines up on one right edge; on a
     dock row beside a device picker it is the widest thing on the line, and it
     spends that width saying in a sliding knob what the word next to it already
     says in Relay's own voice.

     THE SHAPE IS THE SHARED ICON BUTTON, NOT A NEW ONE. `.r-iconbtn` is the
     26px square `app.css` already publishes, already in its focus-ring list,
     and already worn by the Library's Favourites toggle for exactly this job —
     an icon-only control whose STATE is the whole message. What is declared
     here is position and paint. No height, no radius, no border shorthand, no
     font: the shared control owns its box (B1).

     WHY IT IS A TOGGLE AND NOT A BUTTON THAT DOES A THING. `role="switch"` +
     `aria-checked` is what a screen reader needs to hear "microphone, switch,
     on" rather than "microphone, button" — the pill had that and losing it
     would be a silent accessibility regression for a visual preference.

     EMERALD, NEVER AMBER (rule 18). Neither a live microphone nor an armed
     detector is a congregation looking at something. Emerald is this card's own
     "the signal is real" colour four rows up, and the same green `.r-cbtn.golive`
     uses for ready-but-not-yet-live. */
  .audtog { flex: 0 0 auto; }
  .audtog.on {
    background: var(--v-emerald-soft); border-color: var(--v-emerald-line); color: var(--v-emerald);
  }
  /* The hover of the shared control tints toward the accent, which would move an
     ARMED toggle off emerald under the pointer and read as a state change. */
  .audtog.on:hover { color: var(--v-emerald); border-color: var(--v-emerald-line); }
  .audtog:disabled { opacity: .45; cursor: not-allowed; }
  /* `SENS`, `ARMED`, `COUNTDOWN` (L2). This class already carried the tracking a
     line of capitals needs and then set the words in lower case, which is the one
     combination that reads as neither: the prototype's equivalent (`.cap`) is
     uppercase and the letter-spacing is there because of it. The WORDS in the
     markup are unchanged, so a screen reader still hears Relay's own voice. */
  .dcap {
    flex: 0 0 auto; font-family: var(--f-mono); font-size: var(--v-fs-cap);
    letter-spacing: var(--v-tr-caps); text-transform: uppercase; color: var(--v-faint);
  }
  .dcap.detl { min-width: 34px; }
  .sensv { flex: 0 0 auto; min-width: 18px; text-align: right; font-size: var(--v-fs-cap); color: var(--v-dim); }
  .db { flex: 0 0 auto; font-size: var(--v-fs-cap); color: var(--v-dim); }
  .vad {
    flex: 0 0 auto;
    font-size: var(--v-fs-kind); font-weight: 600; letter-spacing: var(--v-tr-caps);
    text-transform: uppercase; padding: 2px 6px; border-radius: var(--v-r-sm);
    /* --v-dim, not --v-faint: muted text on --v-surf3 is 3.79:1, below AA, and
       `tokencontrast.test.js` fails the build for exactly this pairing. */
    background: var(--v-surf3); color: var(--v-dim); border: 1px solid var(--v-500);
  }
  .vad.on { background: var(--v-emerald-soft); color: var(--v-emerald); border-color: var(--v-emerald-line); }

  /* ── LIVE TRANSCRIPT · THE PROTOTYPE'S `.trl` (L3) ────────────────────────
     A timestamp column and a text column, with what is still being said set
     apart by a steel left edge rather than only by a colour — an operator
     glancing across the dock has to find the live line without reading it. */
  .tbody { overflow-y: auto; display: flex; flex-direction: column; gap: 2px; }
  .trl {
    margin: 0; display: flex; gap: 8px; padding: 3px 5px;
    border-radius: var(--v-r-sm); border-left: 2px solid transparent;
  }
  .trl .tt {
    flex: 0 0 auto; padding-top: 1px;
    font-size: var(--v-fs-fig); color: var(--v-faint);
  }
  .trl .tx { font-size: var(--v-fs-b2); line-height: 1.45; color: var(--v-dim); min-width: 0; }
  /* What is still being said is the SELECTION colour — it is the thing being
     worked on, not a claim about a screen. Never cyan (that means the AI has
     guessed at a verse) and never amber. */
  .trl.cur { background: var(--v-sel-soft); border-left-color: var(--v-sel); }
  .trl.cur .tx { color: var(--v-txt); }
  .caret {
    display: inline-block; width: 6px; height: 11px; margin-left: 2px;
    vertical-align: -1px; background: var(--v-sel);
  }
  /* Steps, not a fade. A viewer who asked for no motion still gets the caret,
     which is the information — it just stops blinking. */
  @media (prefers-reduced-motion: no-preference) {
    .caret { animation: trcaret 1s steps(1) infinite; }
  }
  @keyframes trcaret { 50% { opacity: 0; } }
  .trl.empty { color: var(--v-faint); font-family: var(--f-mono); font-size: var(--v-fs-cap); }

  .tools { display: flex; flex-direction: column; gap: 6px; justify-content: flex-start; overflow-y: auto; }
  /* ── ONE INSTRUMENT, THREE TIMES (L3, docs/REBRAND.md §2) ─────────────────
     The prototype's `.tmr`, `.lt3` and `.alrt` are one card repeated: the same
     ground, the same 7px padding, the same 5px inner gap, a head whose left is a
     mono caption and whose right is that tool's one control or badge, a 26px
     full-width field, and a grid button row at 5px. All three tools use this.

     The countdown used to sit on `--v-surf2` behind `--v-line2` at `--v-r-md`
     while the other two used `--v-surf` / `--v-line` / `--v-r-lg`; that is three
     differences between two neighbours in one 200px column, which is why they
     read as three degrees of finish. `.r-tile` is the house card and these are
     its tokens. */
  .qblock {
    display: flex; flex-direction: column; gap: 5px; padding: 7px; margin-bottom: 6px;
    background: var(--v-surf); border: 1px solid var(--v-line); border-radius: var(--v-r-lg);
  }
  .qhead { display: flex; align-items: center; gap: 6px; flex-wrap: wrap; row-gap: 4px; }
  .qspring { flex: 1 1 auto; min-width: 0; }
  /* ONE EVEN BUTTON ROW (C2, operator instruction 2026-09-14). Every button in
     a row is the same width, at every width of the card, by construction:
     `grid-auto-flow:column` gives each child its own implicit track and
     `grid-auto-columns:minmax(0,1fr)` makes every track an equal share.

     IT WAS `repeat(auto-fit, minmax(62px, 1fr))`, AND THAT IS WHY IT WAS
     RAGGED. `auto-fit` lays as many tracks as FIT, which is a function of the
     card's width and not of the number of buttons — so the countdown's six ran
     five-and-one at the desk width and four-and-two on a 1280 booth laptop, a
     full-width row above an orphan. Two buttons happened to look right because
     two is the one count `auto-fit` cannot get wrong.

     Never a wrapping flex of fixed-width controls, which is what came before
     that: at 1024 it ran the countdown figure and `Take down` 37px and 56px past
     this card's right edge, over the Controls card beside it. */
  .qbtns { display: grid; grid-auto-flow: column; grid-auto-columns: minmax(0, 1fr); gap: 5px; }
  .qbtns > :global(button) { min-width: 0; padding: 0 6px; }
  /* A row inside a block: fields and the one-word states. Same 5px rhythm. */
  .qrow { display: flex; align-items: center; gap: 5px; flex-wrap: wrap; row-gap: 4px;
    font-size: var(--v-fs-b2); color: var(--v-dim); }
  /* The caption under a preview or in place of a missing one. */
  .qcap {
    margin: 0;
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
    letter-spacing: var(--v-tr-caps); color: var(--v-faint);
  }
  /* The stage's own red, and only while the message is actually on the monitor.
     Amber is ON AIR and amethyst is rehearsal; neither is what this is. */
  .qblock.onstage { border-color: var(--v-red-line); background: var(--v-red-soft); }
  .qbadge {
    flex: 0 0 auto; padding: 2px 6px; border-radius: var(--v-r-sm);
    background: var(--v-red-soft); color: var(--v-red);
    font-family: var(--f-mono); font-size: var(--v-fs-kind);
    letter-spacing: .08em; text-transform: uppercase;
  }

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
    font-size:var(--v-fs-d2); line-height: 1; font-weight: 600;
    letter-spacing: .01em;
    /* Dim until it is genuinely on a wall: this is the SET duration then, and a
       setting rendered as brightly as a live figure is the same number telling
       two different stories. */
    color: var(--v-faint);
  }
  .tfig.live { color: var(--v-txt); }
  /* Red = act now. Never amber: amber means ON AIR and nothing else (rule 18). */
  .tfig.warn { color: var(--v-red); }
  /* ── WHAT IS LEFT OF `.tmr` (L3) ──────────────────────────────────────────
     The ground, the hairline, the corner, the padding and the inner gap are
     `.qblock`'s now — this tool is not a different kind of card from the two
     below it. `flex:0 0 auto` is all that remains, so a scrolling column of
     three does not squash the one with the most rows in it. */
  .tmr { flex: 0 0 auto; }
  /* What Start would load, beside the name — small, and never the size of the
     figure it sits next to, which is the number that is actually on a screen. */
  .cdset { flex: 0 0 auto; font-size: var(--v-fs-cap); color: var(--v-faint); }
  /* auto / m:ss / h:mm:ss.
     WIDTH AND PADDING ONLY — the height is the shared control's (§1's reference
     table): a mixed column of a select, three fields and six buttons is exactly
     the column that table exists to keep on one line. It used to be 22px here
     and 26px everywhere else in the same card.

     FIELDS THAT LINE UP (C2). It was `flex: 0 0 auto` with a `.qspring` after
     it, so the field row stopped somewhere in the middle of a card whose head,
     button row and neighbouring tools all run to the edge — the one row in the
     three blocks that did not. It now takes the rest of the line, exactly as
     the prototype's `.tset .pick.sm { flex: 1 }` does, and the spring is gone
     because there is nothing left to push. */
  .cdfmt { width: auto; flex: 1 1 auto; min-width: 0; padding: 0 20px 0 6px;
    font-size: var(--v-fs-lbl); background-position: calc(100% - 7px) center; }
  /* The figure and the word under it, right-aligned as one thing in the head's
     right-hand slot — where the name band puts its picker and the alert puts its
     badge. `min-width:0` so a long state word ellipses rather than wrapping the
     head and changing the card's height. */
  .cdfig { display: flex; flex-direction: column; align-items: flex-end; gap: 1px;
    flex: 0 1 auto; min-width: 0; }
  .cdstatev {
    max-width: 100%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    font-family: var(--f-mono); font-size: var(--v-fs-cap);
    letter-spacing: var(--v-tr-caps); text-transform: uppercase; color: var(--v-faint);
  }
  .cdstatev.live { color: var(--v-dim); }
  /* Held is a real third state and it reads as one. Cyan is a GUESS and amber is
     ON AIR, so neither is available; the text colour is the one that means "the
     operator did this deliberately". */
  .cdstatev.held { color: var(--v-txt); }
  /* Steps, not a fade, and only where motion is welcome: the blink exists to
     catch an eye that is not looking at it, and a viewer who asked for no motion
     still gets the colour, which is the information. */
  @media (prefers-reduced-motion: no-preference) {
    .tfig.warn { animation: cdwarn 2s steps(1) infinite; }
  }
  @keyframes cdwarn { 50% { opacity: .38; } }
  /* THE TRANSPORT IS THE ONE ROW THAT CANNOT BE SIX ACROSS, so it is three and
     three — still every cell the same width, still no orphan, and the split
     falls where the meaning does: run it, then re-aim it.

     THE ARITHMETIC, because "one row" was the instruction and this is not it.
     Quick tools is `1.1fr` of the dock's `1.25 + 1.5 + 1.1 + 1fr`, so at a
     1600px desk the card is ~363px and this row has ~323px after the body's 9px
     and the block's 7px. Six cells at 5px gaps is 49.7px each, and `.r-btn.sm`
     spends 12px of that on padding: `Resume` does not fit in 37px of type at
     1600 and has 25px at 1280. A row that clips its own labels is not clean, so
     the honest shape is two rows that are each even. `grid-auto-flow` goes back
     to `row` because `.qbtns` sets it to `column` for the two-button rows. */
  .cdtrans { grid-auto-flow: row; grid-template-columns: repeat(3, minmax(0, 1fr)); }
  .tin { width: 62px; flex: 0 0 auto; }
  .tin.wide { flex: 1 1 auto; width: auto; min-width: 0; }
  .derr { margin: 0; font-size: var(--v-fs-cap); color: var(--v-red); }

  /* ── the name band and the header's one button ────────────────────────────
     No amber anywhere in this block. Amber means ON AIR and none of these
     controls is a claim that a screen changed — the fires they start report
     their own outcome through `run()` and the error line above. */
  .dk-btn { flex: 0 0 auto; margin-left: 6px; }
  /* The head's right-hand slot, like the countdown's figure and the alert's
     badge. Width only — the height is the shared control's. */
  .ltpick { flex: 1 1 auto; min-width: 0; }
  /* NO 80px INDENT (L3). It hung the fields, the buttons, the preview and the
     caption under a label that is in the HEAD, not in the column — so a third of
     a 200px card was empty and the name band was the one tool whose contents did
     not start at the card's edge. */
  .ltprev {
    aspect-ratio: 16 / 9; container-type: inline-size;
    background: var(--v-void); border: 1px solid var(--v-line2);
    border-radius: var(--v-r-sm); overflow: hidden;
  }

  /* The controls card takes the height it is given and divides it among the
     buttons. `overflow:hidden`, not `auto`: a panic control that can be scrolled
     out of reach is a panic control that will be, exactly once, on a Sunday. */
  .ctlbody { display: flex; flex-direction: column; overflow: hidden; }
  .ctlbody .r-ctl { flex: 1; align-content: stretch; grid-auto-rows: 1fr; gap: 5px; }
  .ctlbody :global(.r-cbtn) { height: auto; min-height: 32px; }
</style>
