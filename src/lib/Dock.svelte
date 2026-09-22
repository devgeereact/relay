<script context="module">
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
  export function pushReading(buf, t, v, spanMs = WAVE_SPAN_MS, kind = 'quiet') {
    const last = buf.length ? buf[buf.length - 1] : null;
    // A clock that went backwards (a resumed laptop, a corrected system time)
    // would otherwise place new readings to the LEFT of old ones and draw the
    // envelope inside out. Start again rather than draw a lie.
    const base = last && last.t > t ? [] : buf;
    const out = [...base, { t, v: Math.max(0, Math.min(1, v)), kind }];
    const cut = t - spanMs;
    let i = 0;
    while (i + 1 < out.length && out[i + 1].t <= cut) i++;
    return out.slice(i);
  }

  /**
   * WHAT COLOUR IS THIS READING?
   *
   * Three answers and no fourth, and every one of them is a measurement rather
   * than a comparison against a level Relay chose:
   *
   *   `clip`  a sample at or past full scale. Clipping is the one absolutely
   *           defined fault in audio — the sample had nowhere left to go — so
   *           saying it is not a violation of rule 12 (DECISIONS §19). It is red
   *           because red means act now: move the gain, nothing else will fix it.
   *   `voice` the VOICE GATE'S OWN ANSWER, `is_voice`, carried on the same event.
   *           Not a threshold applied here. Rule 12 forbids the console deciding
   *           what counts as speech, and this asks the gate instead of guessing.
   *   `quiet` everything else, in the card's steel. A quiet room is not a fault.
   *
   * AMBER IS ABSENT ON PURPOSE. Amber means ON AIR (rule 18) and is never spent
   * on a microphone, so there is no "hot but not clipping" band: the honest line
   * between "fine" and "broken" here is full scale, and inventing a warning
   * level would be inventing exactly the absolute threshold rule 12 removed.
   */
  export const CLIP_AT = 0.999;
  export function readingKind(v, isVoice) {
    if (v >= CLIP_AT) return 'clip';
    return isVoice ? 'voice' : 'quiet';
  }

  /**
   * Append ONE CHUNK'S WHOLE ENVELOPE, spread across the time it covers.
   *
   * The chunk arrived at `endT` and is `chunkMs` long, so its first peak is
   * `chunkMs` older than its last. Stamping all sixteen at the arrival time
   * would pile them on one pixel and draw a vertical spike per delivery, which
   * is a different wrong picture from the one this replaces.
   *
   * Falls back to a single reading when the backend sent no peaks, so a console
   * running against an older engine draws what it always drew instead of a flat
   * line.
   */
  export function pushEnvelope(buf, endT, peaks, isVoice, level, chunkMs = 400, spanMs = WAVE_SPAN_MS) {
    if (!Array.isArray(peaks) || peaks.length === 0) {
      return pushReading(buf, endT, level ?? 0, spanMs, readingKind(level ?? 0, isVoice));
    }
    let out = buf;
    const step = chunkMs / peaks.length;
    for (let i = 0; i < peaks.length; i++) {
      const v = peaks[i];
      // The slice's own END, so the newest reading lands at `endT` exactly and
      // the trace's right-hand edge is the present rather than 25 ms ago.
      const t = endT - (peaks.length - 1 - i) * step;
      out = pushReading(out, t, v, spanMs, readingKind(v, isVoice));
    }
    return out;
  }

  /**
   * One segment split into RUNS OF ONE COLOUR, so a chunk that clipped halfway
   * through is red only where it clipped.
   *
   * Each run repeats the previous run's last point, so the filled shapes meet
   * rather than leaving a hairline of background between them.
   */
  export function waveRuns(seg) {
    const runs = [];
    for (const p of seg) {
      const last = runs[runs.length - 1];
      if (!last || last.kind !== (p.kind ?? 'quiet')) {
        const run = { kind: p.kind ?? 'quiet', pts: [] };
        if (last) run.pts.push(last.pts[last.pts.length - 1]);
        run.pts.push(p);
        runs.push(run);
      } else {
        last.pts.push(p);
      }
    }
    return runs.filter((r) => r.pts.length > 1);
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
      cur.push({ x: 1 - (now - r.t) / spanMs, v: Math.max(0, Math.min(1, r.v)), kind: r.kind ?? 'quiet' });
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
    sendStageAlert,
    stageAlert,
    templates,
    loadTemplates,
    fireContent,
    setInputDevice,
    startCapture,
    stopCapture,
    mediaTransport,
    setMediaTransport,
    channelHealth,
    sendStageMedia,
    stageMedia,
  } from './stores/capture.js';
  import { describeMediaClock, mediaIdFromUrl } from './mediaclock.js';
  import { session, setSession } from './session.js';

  // ── THE CLIP, AND THE ONE SET OF CONTROLS OVER IT (RG-237) ─────────────────
  //
  // The transport lived on Live's programme monitor, which is a WORKSPACE. A
  // clip is playing on every screen in the building whatever tab the operator
  // happens to be on, so its controls belong in the shell — mounted once,
  // surviving a crashed view, beside the other three controls over what a
  // congregation sees.
  //
  // The clock reads what the SCREENS report (`describeMediaClock`), never this
  // process's own player: a readout in the shell that timed the console's
  // preview would be a number about nothing. `channelHealth` is the same map
  // Live read it from, so the two cannot disagree — there is only one of them
  // now in any case.
  $: mediaClock = describeMediaClock(
    Object.entries($channelHealth).map(([id, row]) => ({ ...row, id: Number(id) })),
  );
  $: clipLive = !!$live?.media_url && !$screenBlack;
  /**
   * THE CLIP ON THE WALL, AS AN ID — or `null` for one Relay ships.
   *
   * A bundled picture has no row under `/media/<id>` (DECISIONS §90), so there
   * is no id to send and the control says so instead of guessing at one.
   */
  $: clipMediaId = mediaIdFromUrl($live?.media_url);
  $: clipOnStage = $stageMedia != null && $stageMedia === clipMediaId;
  async function toStage() {
    clipErr = '';
    try {
      await sendStageMedia(clipOnStage ? null : clipMediaId);
    } catch (e) {
      clipErr = humanError(e);
    }
  }
  let clipErr = '';
  async function clip(change) {
    clipErr = '';
    try {
      await setMediaTransport(change);
    } catch (e) {
      // GROUP 1 (throws), and it is shown rather than swallowed: a Pause that
      // failed silently leaves a clip running under an operator who believes
      // they stopped it, and the next cue goes out over the top of it.
      clipErr = humanError(e);
    }
  }
  import { templateKind } from './templateKind.js';
  import TemplateRender from './TemplateRender.svelte';
  import { humanError } from './errors.js';
  import { rangeFill } from './rangefill.js';
  // The dock lives in the SHELL, on every workspace, so its Detection switch was
  // the one door out of safe mode that nothing asked about. See the switch itself.
  import { safeMode } from './boot/boot.js';

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
  // HOW FAR BACK THE CARD LETS AN OPERATOR SCROLL.
  //
  // It was 40 and it never once bit: `capture.js` kept twelve, so the card could
  // show at most twelve however many it was willing to draw. Both numbers moved
  // on the operator's instruction of 2026-09-20 — *"use all the space it has
  // first, and the operator can scroll to read what was said in the past few
  // minutes for quick recall"* — and this one is the smaller of the two on
  // purpose: the store is the history, this is what one card will paint at once,
  // and painting a whole service into a 152px box costs an operator nothing but
  // costs the webview a layout pass per line, several times a minute.
  //
  // 120 is about 75 minutes at the rate a real service produced (one closed line
  // every ~38 s), which is further back than "the past few minutes" ever means.
  // ── HOW MUCH OF THE TRANSCRIPT IS PAINTED, AND WHY IT IS NOT ALL OF IT ────
  //
  // `capture.js` keeps EVERY closed line of the session now, with no cap, on the
  // operator's instruction of 2026-09-20. This is the other half of that: a keyed
  // `{#each}` lays out every row it is handed, and handing it a whole service
  // costs a layout pass over ~800 rows several times a minute for a 152px box
  // that can show about eight.
  //
  // So the card paints a WINDOW of the newest lines and grows it when the
  // operator scrolls near the top — which is the only moment more of them can be
  // seen. `TR_PAGE` more each time, never fewer, and it is reset when capture
  // starts a new service rather than creeping upward for the life of the install.
  //
  // The window is a render budget and NOT a limit on the history: the store has
  // the whole session and `transcripts` has the record. Nothing is lost by
  // scrolling slightly further than the window, because reaching the top is what
  // extends it.
  const TR_PAGE = 120;
  let trShown = TR_PAGE;
  $: allLines = $transcript.finals.map((t, i) => ({
    t,
    at: $transcript.finalsAt?.[i] ?? '',
  }));
  $: tlines = allLines.slice(-trShown);
  // A NEW SERVICE STARTS THE WINDOW AGAIN. `finals` is emptied when a session is
  // reset, and a window left at several thousand would then be a budget nobody
  // set for a card with four lines in it.
  $: if (allLines.length < trShown - TR_PAGE) trShown = TR_PAGE;

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
    // READING BACK PAST THE WINDOW EXTENDS IT. The operator is at the top of what
    // has been painted and there is more session behind it, so paint more. It
    // only ever grows, and only while there is something to grow into, so this
    // cannot loop against its own `afterUpdate`.
    if (trBody.scrollTop <= TR_SLACK && trShown < allLines.length) {
      trShown += TR_PAGE;
    }
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
  // It reports its own outcome TWICE, and it used to do only the second. It sits
  // in `run()`, which catches and prints `humanError(e)` under the dock — and
  // `endService` now throws rather than swallowing (`endservice.test.js`), so a
  // refusal says so in words. The next poll is the corroboration, not the report:
  // if the service did not end, the button still says End service and still burns
  // amber.
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
  // A plain `let` driven by a 500 ms interval, never `tick()` from a `$:` block —
  // re-entering Svelte's scheduler there hard-freezes the webview (rule 1).
  //
  // IT USED TO BELONG TO THE COUNTDOWN and is declared here now (2026-09-20).
  // The Screen Countdown's figure and this card's `no signal` line shared one
  // tick, and when the countdown moved to Live's run surface the tick went with
  // it — leaving `noSignal` reading a variable that no longer existed, which
  // takes the whole dock down rather than only the line that wanted it. The
  // shared tick was never the countdown's to own: this card needs a clock
  // whether or not anything else on the surface does.
  let nowTick = Date.now();
  const tickTimer = setInterval(() => (nowTick = Date.now()), 500);
  onDestroy(() => clearInterval(tickTimer));
  $: micLive = $capture.capturing && $capture.available;
  $: noSignal = micLive && waveStale(signalSince, nowTick);

  function onReading(m) {
    const now = Date.now();
    waveBuf = pushEnvelope(waveBuf, now, m?.peaks, !!m?.isVoice, m?.level ?? 0);
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

    // THE THREE COLOURS, and `readingKind` is the one place that chooses between
    // them. Steel is the card's own ink; emerald is the same emerald the VOICE
    // chip in this head uses, so the trace and the chip cannot disagree; red is
    // clipping. No amber anywhere: amber is ON AIR (rule 18).
    const INK = {
      quiet: { fill: 'rgba(148,158,176,.22)', line: 'rgba(148,158,176,.62)' },
      voice: { fill: 'rgba(63,207,106,.30)', line: 'rgba(63,207,106,.88)' },
      clip: { fill: 'rgba(233,84,84,.38)', line: 'rgba(233,84,84,.95)' },
    };

    // ONE mirrored envelope per COLOUR RUN rather than a bar per reading: a trace
    // reads as a signal, a picket fence reads as a chart. A segment BREAK is
    // audio nobody measured, and it is drawn as a break — see `waveSegments`.
    // Within a segment the shape is split where the colour changes, so a chunk
    // that clipped for 25 ms is red for 25 ms and not for its whole 400.
    for (const seg of waveSegments(waveBuf, Date.now())) {
      for (const run of waveRuns(seg)) {
        const pts = run.pts;
        const px = (p) => p.x * cw;
        const up = (p) => mid - p.v * mid * 0.94;
        const dn = (p) => mid + p.v * mid * 0.94;
        cx.beginPath();
        cx.moveTo(px(pts[0]), up(pts[0]));
        for (let i = 1; i < pts.length; i++) {
          const a = pts[i - 1];
          const b = pts[i];
          cx.quadraticCurveTo((px(a) + px(b)) / 2, up(a), px(b), up(b));
        }
        for (let i = pts.length - 1; i >= 0; i--) cx.lineTo(px(pts[i]), dn(pts[i]));
        cx.closePath();
        const ink = INK[run.kind] ?? INK.quiet;
        cx.fillStyle = ink.fill;
        cx.fill();
        cx.strokeStyle = ink.line;
        cx.lineWidth = 1.1;
        cx.stroke();
      }
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

  // ── THE DIAL IS DERIVED, NEVER HELD ───────────────────────────────────────
  //
  // This was `let sensitivity = 50`, read once in `onMount`, and it was the only
  // setting in the whole shell kept in a component-local variable. Every workspace
  // view is destroyed and rebuilt on a tab switch, so a stale copy there heals
  // itself; the dock is mounted OUTSIDE that router and is never rebuilt, so this
  // one never did. Three failures came out of that, and all three were measured:
  // Settings moved the gate and this card went on showing the old figure for the
  // rest of the session; this card moved the gate and Settings, still holding what
  // IT loaded, silently reverted the change on the next profile save; and the
  // router self-calibrates on every confirm and dismiss, so the number drifted
  // stale with the operator touching nothing at all.
  //
  // It now derives from the one store, which `detection://thresholds` keeps in
  // step with the engine. `sensitivityKnown` is a separate fact from the number
  // and stays that way: 50 is both the shipped default and an ordinary real
  // setting, so a reading alone cannot tell "the gate is at 50" from "nobody has
  // asked since launch", and the caveat is a sentence in the card's meta slot
  // rather than a glyph in an 18px value column.
  //
  // `pending` is the thumb the operator is currently dragging, and only that. It
  // is cleared the moment the engine answers, so the store wins every time except
  // the few hundred milliseconds where the operator is more current than it is.
  let pending = null;
  $: sensitivity = pending ?? $capture.sensitivity;
  $: sensReadable = $capture.sensitivityKnown && $capture.available;
  onMount(async () => {
    sizeCanvas();
    // The answer lands in the store, not here — see `getSensitivity`.
    try {
      await getSensitivity();
    } catch {
      /* `sensitivityKnown` stays false, and the card says so in words */
    }
  });
  // ── THE MICROPHONE, ON THE RUN SURFACE (L3, operator instruction) ──────────
  //
  // The device list and the on/off already existed — in Settings → This room, three
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
    pending = v; // the thumb, while the engine is being asked
    err = '';
    try {
      await setSensitivity(v);
      pending = null; // the store now holds what actually landed
    } catch (e) {
      // A dial that did not take must not leave the thumb where the operator
      // dragged it. Drop back to the engine's own answer and say what happened —
      // `setSensitivity` THROWS precisely so this branch can exist.
      pending = null;
      try {
        await getSensitivity();
      } catch {
        /* the dial is already disabled in this case */
      }
      err = `Sensitivity stayed at ${$capture.sensitivity} — ${humanError(e)}`;
    }
  }

  // ── WHERE THE SCREEN COUNTDOWN WENT (operator instruction, 2026-09-20) ────
  //
  // It was the first of Quick tools' blocks and it is not here any more. The
  // operator's words: *"Remove SCREEN COUNTDOWN from Quick Tools in the Live
  // workspace. The countdown that goes to the live screen must remain available,
  // but only where it is actually needed."*
  //
  // The whole instrument — the hh:mm:ss fields, the clock time, the format
  // picker, Start · Pause/Resume · Reset · ±1 · Clear, the reach line and `Put
  // back on screens` — is now the `Screen Countdown` band on Live's run surface,
  // directly above the Stage Timer band. Nothing of it stayed behind: the poll,
  // the tick and the two reads that fed it went with it, because a reactive half
  // left running on every workspace for a card that no longer exists is how the
  // card comes back.
  //
  // THIS REVISES THE 2026-09-17 DECISION recorded above `putBack`, which put the
  // way back INSIDE the countdown block specifically so that Quick tools could
  // stay at three. The argument it was made on — that the dock is on every
  // workspace and the run surface is not — is real and is the price of this
  // change; it is stated in the band's own comment on Live rather than restated
  // here. `screencountdown.test.js` holds both halves.

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
  // `push_announcement` command and `capture.js::pushAnnouncement` were both
  // DELETED — not left registered with nothing rendering them, which by this
  // repository's own standard would have been attack surface nobody is
  // watching. That follows the precedent for the five commands deleted on
  // 2026-08-30: delete it rather than hide it. It was a change to
  // `src-tauri/` and to another agent's file, and it has since been made.
  //
  // `announce.test.js`, `r2livepath.test.js` and `qa-r5-groups.test.js` record
  // WHY the wrapper had to throw while it existed. None of them still holds
  // that contract — there is no wrapper left to hold it.

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
  // TWO VERBS, ONE FIELD (operator, 2026-09-21; DECISIONS §116). The words are
  // the same; what differs is whether the preacher's screen is interrupted. A
  // note lands as fixed text where the template puts it; an alert takes the
  // screen and flashes. One control for each, because a modifier on a single
  // button is a thing an operator gets wrong in front of a congregation.
  const toPreacher = (urgent = false) => run(async () => {
    const line = stageMsg.trim();
    if (!line) return;
    await sendStageAlert(line, urgent);
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

  // The one sentence both panic controls show when the bridge is not attached.
  // Written once because two controls saying two different things about one fact
  // is the beginning of the drift this pass exists to stop.
  const PANIC_OFF =
    'Relay’s engine is not answering, so this cannot reach your screens from here. ' +
    'Anything already on a screen is still there. Restart Relay if this does not clear.';
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
           Settings → This room (see the note above `toggleMic`).
           The device is fixed for the life of a capture, because that is what
           `start_capture` takes — so the picker is disabled while listening
           rather than silently doing nothing. -->
      <!-- ONE GRID FOR BOTH ROWS (operator, 2026-09-21). They were two flex rows,
           and the SENS row carries one cell the MIC row does not — `.sensv`, the
           figure `50` — so with a 7px gap the ARMED toggle sat 25px right of the
           LISTEN toggle. Two instances of one instrument, on two lines of one
           card, not in one column.

           A grid is what makes a column a column. The rows keep their names and
           become `display: contents`, so their children are the grid's own items
           and the switch lands in one column by construction rather than by two
           rows happening to add up the same. Nothing was measured or reserved:
           an arithmetic fix would need re-deriving the moment the figure reaches
           three digits. -->
      <div class="audgrid">
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
             arming detection is not a claim about a screen (rule 18).

             DISABLED UNDER SAFE MODE, and this is the load-bearing half.
             `applySafeMode` disarms detection ONCE, at the transition (DECISIONS
             §86). This switch is in the SHELL — it is on every workspace,
             including the Settings page where safe mode itself lives, right
             beside the sensitivity dial a volunteer came to look at. Nothing here
             asked about safe mode, so one press re-armed the detector while three
             surfaces went on saying it was disarmed: the status bar's first
             branch ("Safe mode — outputs disabled", outranking On air),
             `degraded.js` ("nothing Relay does can reach a screen") and the
             Settings row itself. And it is not a harmless label problem — the
             kiosk hub and an OBS source keep their connection through safe mode,
             so the next AutoFire paints a verse on them. An auto-fire is Relay's
             own initiative, which is the half §86 says IS covered; the manual-fire
             carve-out does not reach it.

             The other eight library surfaces already do this
             (`VerseDeck.svelte`, `Browse.svelte`, `LiveOutputRail.svelte`,
             `Dashboard.svelte`), in the same words. -->
        <button
          class="r-iconbtn audtog"
          class:on={$detectionOn}
          role="switch"
          aria-checked={$detectionOn}
          aria-label="Detection"
          title={$safeMode
            ? 'Safe mode is on — detection stays disarmed. Turn safe mode off in Settings → Before the service first.'
            : $detectionOn
              ? 'Detection armed — Relay is matching what it hears against scripture. Press to turn it off.'
              : 'Detection off — nothing is being matched against scripture. Press to arm it.'}
          disabled={busy || !$capture.available || $safeMode}
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
      <!-- TWO BLOCKS, AND IT USED TO BE THREE (operator instruction, 2026-09-20).
           The congregation countdown was the first of them and is now a band of
           its own directly above the Stage Timer on Live's run surface. §2 named three things that
           change during a service; two of them change on every workspace and the
           third only ever changed a congregation screen, which is a thing an
           operator is on Live to do. The reasoning the card was built on is kept
           in `quicktools.test.js` rather than deleted, with the revision beside
           it — see the comment above `press` in `views/Live.svelte` for the price
           this change pays, which is real. -->
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

      <!-- ── The Stage Message (§5) ──────────────────────────────────────────
           The stage monitor and nothing else. `sendStageAlert` publishes a frame
           kind that exists inside the stage renderer, so no congregation channel
           can show it — the guarantee is in `channels.rs`, not in this label. -->
      <div class="qblock" class:onstage={$stageAlert}>
        <div class="qhead">
          <span class="r-lbl">Stage Message</span>
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
          aria-label="Stage Message — stage monitor only"
          on:keydown={(e) => e.key === 'Enter' && toPreacher(false)} />
        <!-- `primary`, not `pri`. `pri` is not a class this stylesheet defines,
             so the one button in Quick tools that is meant to read as the
             primary action had been rendering as a plain `.r-btn` — invisible,
             because a plain button is a perfectly ordinary thing to look at. -->
        <div class="qbtns">
          <button class="r-btn sm primary" on:click={() => toPreacher(false)} disabled={busy || !stageMsg.trim()}
            title="Put these words on the preacher's screen as an ordinary note. It does not flash and does not cover the reading.">Send to stage</button>
          <!-- THE ALARM, AND IT SAYS SO. Rose, because this is the one control
               here that interrupts a person mid-sentence — and never amber, which
               means a congregation is looking at something (rule 18). -->
          <button class="r-btn sm danger" on:click={() => toPreacher(true)} disabled={busy || !stageMsg.trim()}
            title="Take the whole stage screen with a flashing red alert. For something that must stop the service.">Alert</button>
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
        <!-- ══ THE CLIP ══ ONE SET OF CONTROLS, FOR EVERY SCREEN (RG-237).
             This slot held `End service`, which is inert whenever no service is
             recording — most of the life of the card — and a control that is
             dark most of the time is the wrong use of the one card that may
             never scroll. Ending a service is still reachable from the readiness
             screen, where it is READ rather than reached for under pressure.

             While a clip is on the screens this is the transport; the rest of
             the time it is the End service control it replaced, so the row is
             never empty and never inert for no reason. -->
        {#if clipLive}
          <div class="clipbar">
            <button class="r-cbtn clipbtn" on:click={() => clip({ paused: !$mediaTransport.paused })}
              aria-pressed={$mediaTransport.paused}
              title={$mediaTransport.paused ? 'Let the clip run on every screen' : 'Hold the clip on every screen'}
            >{$mediaTransport.paused ? 'Play' : 'Pause'}</button>
            <button class="r-cbtn clipbtn" on:click={() => clip({ replay: true })}
              title="Start the clip again from the beginning, on every screen">Replay</button>
            <button class="r-cbtn clipbtn" class:on={$mediaTransport.loop} aria-pressed={$mediaTransport.loop}
              on:click={() => clip({ loop: !$mediaTransport.loop })}
              title={$mediaTransport.loop ? 'Stop repeating at the end' : 'Repeat the clip when it ends'}>Loop</button>
            <!-- AND ONTO THE PREACHER'S SCREEN TOO. Scripture overrides it
                 there, so this is additive rather than a second wall. -->
            <button class="r-cbtn clipbtn" class:on={clipOnStage} aria-pressed={clipOnStage}
              disabled={clipMediaId == null} on:click={toStage}
              title={clipMediaId == null
                ? 'A picture Relay ships cannot be sent on its own'
                : clipOnStage
                  ? "Take it off the preacher's screen"
                  : "Put this on the preacher's screen as well"}
            >{clipOnStage ? 'On stage' : 'To stage'}</button>
            <!-- SCRUB AND LEVEL CAME WITH THE REST (RG-221 lives here now). On
                 the drop, never on every pixel of the drag: each frame reaches
                 every screen in the building, and a drag across a two-minute
                 clip would be hundreds of broadcasts and a wall that stutters
                 while the handle moves. -->
            {#if mediaClock.known && mediaClock.durationMs}
              <input class="r-range clipscrub" type="range" min="0" max={mediaClock.durationMs} step="250"
                value={mediaClock.positionMs ?? 0} aria-label="Scrub the clip"
                title="Drag to move the clip. Every screen follows."
                on:change={(e) => clip({ seekMs: Number(e.target.value) })} />
            {/if}
            <span class="cliplevel">
              <span class="r-lbl">Level</span>
              <input class="r-range" type="range" min="0" max="1" step="0.05"
                value={$mediaTransport.volume ?? 1} aria-label="Clip volume on the screens"
                title="How loud the clip is on the output screens"
                on:change={(e) => clip({ volume: Number(e.target.value) })} />
              <span class="r-mono clipvol">{Math.round(($mediaTransport.volume ?? 1) * 100)}%</span>
            </span>
            <!-- HOW LONG IS LEFT, from the screens rather than from any player in
                 this process. `unknown` says so in words: a dash reads as "this
                 clip has no clock" and a zero as "it has finished". -->
            <span class="cliptime r-mono" class:unknown={!mediaClock.known}>{mediaClock.known ? mediaClock.text : 'no screen reporting'}</span>
          </div>
          {#if clipErr}<span class="cliperr" role="alert">{clipErr}</span>{/if}
        {:else}
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
              ? 'Stop recording this service. The transcript, the fires and the timeline are kept — History reads them back, from All history on the readiness screen.'
              : 'Nothing is being recorded. A service starts when you start listening.'}
          >{recording ? 'End service' : 'Not recording'}</button>
        {/if}
        <button
          class="r-cbtn rehearse"
          data-on={$rehearsing ? '1' : '0'}
          on:click={() => run(() => setRehearsal(!$rehearsing))}
          disabled={busy || !$capture.available}
        >{$rehearsing ? 'Rehearsing' : 'Rehearse'}</button>
        <!-- A DISABLED PANIC CONTROL OWES A REASON, and these two were the worst
             case of the sixteen that gave none. A volunteer mid-service sees the
             one control this whole product is arranged around sitting at 45%
             opacity with nothing saying why, and the reasonable conclusion from
             that is that Relay has crashed. It has not: `capture.available` is
             only "is the Tauri bridge attached", so the honest sentence is that
             the engine is not answering and the screens cannot be reached FROM
             HERE, which is a different and much less alarming fact.

             The reason goes to both channels for the reason `ui/Button.svelte`
             gives at length: `title` is invisible to a keyboard or screen-reader
             operator and `aria-describedby` is invisible to a mouse.

             THESE TWO ARE DELIBERATELY NOT `ui/Button.svelte`. `panic.test.js`
             asserts this exact markup, by string, because these are the controls
             rule 15 and DECISIONS section 20 are about, and a shared component
             between the operator and the wall is one more thing that can be got
             wrong in a file nobody opens during a service. The reason is added in
             place; the control is untouched. -->
        <button class="r-cbtn black" data-on={$screenBlack ? '1' : '0'} on:click={doBlack} disabled={!$capture.available}
          title={$capture.available ? undefined : PANIC_OFF} aria-describedby={$capture.available ? undefined : 'dock-panic-why'}>
          {$screenBlack ? 'Black — restore' : 'Blackout'}
        </button>
        <button class="r-cbtn danger wide" on:click={doClear} disabled={!$capture.available}
          title={$capture.available ? undefined : PANIC_OFF} aria-describedby={$capture.available ? undefined : 'dock-panic-why'}>Clear screens</button>
        {#if !$capture.available}<span class="sr-only" id="dock-panic-why">{PANIC_OFF}</span>{/if}
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
  /* FIVE COLUMNS, AND THE SWITCH IS THE FOURTH.
     caption · the flexible control · the figure · the toggle · the word.
     `minmax(0, 1fr)` rather than `1fr` so a long device name truncates inside
     the column instead of widening it — the same reason `.micpick` carries
     `min-width: 0` below. */
  .audgrid {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto auto;
    align-items: center;
    gap: 7px;
    flex: 0 0 auto;
  }
  /* The rows keep their NAME — the markup still reads as two rows and
     `dockaudio.test.js` still slices them by it — and give up laying their own
     children out. Without this the grid has two items (the rows) and the columns
     inside each are independent again, which is the misalignment wearing a
     wrapper. */
  .audrow { display: contents; }
  /* EXPLICIT, not left to `justify-self: stretch`. A grid item with `width: auto`
     does stretch, but a form control's intrinsic width has been the exception in
     more than one engine, and this row is the one where a slider that sized
     itself to ~170px instead of the column would move the toggle beside it —
     which is the whole defect this grid exists to end. Nothing in jsdom can see
     that, so it is stated rather than assumed. */
  .audgrid :global(input[type='range']) { width: 100%; min-width: 0; }
  /* The device names a church actually has are long ("MacBook Pro Microphone",
     "Scarlett 2i2 USB"). It takes the row's spare width and truncates rather than
     pushing the switch off the card's edge. Width only — the height is the
     shared control's, which is the whole point of the four-row column here:
     select · slider · switch all land on one line. */
  /* FOUR CELLS IN A FIVE-COLUMN ROW. The MIC row has no figure, so the picker
     spans the flexible column AND the figure's — which is the honest way to make
     four occupy five. The alternative is an empty span that exists only to be
     empty, and a spacer is furniture the next person has to work out. */
  .micpick { grid-column: 2 / span 2; width: 100%; min-width: 0; }
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
  /* The grid sizes this column to the widest word in it (`LISTEN`), so both
     captions already start on one edge and the hand-set width that used to do
     that job would now only push the column wider than its content. */
  .dcap.detl { text-align: left; }
  /* Column three. `min-width` keeps a single digit from collapsing the column and
     moving the toggle beside it; the column grows on its own for three. */
  .sensv { min-width: 18px; text-align: right; font-size: var(--v-fs-cap); color: var(--v-dim); }
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
  /* THE LINES SIT AT THE BOTTOM AND GROW UPWARD, the way a transcript is read and
     the way every terminal in this genre behaves. Without it a service that has
     produced four closed lines strands them at the top of the card under 100px of
     nothing, which is what the operator meant on 2026-09-20 by the card not using
     the space it has.

     `margin-top:auto` ON THE FIRST ROW, NOT `justify-content:flex-end` ON THE
     CONTAINER. They look equivalent and are not: a scrolling flex column that is
     justified to the end pushes its first children ABOVE the scroll origin, where
     no engine will let you scroll back to them — so the fix for a short pane
     would silently take the history off a full one, which is the other half of
     the same instruction. An auto margin absorbs the free space instead, and when
     there is none left it contributes nothing and the pane scrolls normally. */
  .tbody > :first-child { margin-top: auto; }
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
  /* THE CLIP'S OWN ROW (RG-237). Three equal buttons with the clock under them,
     in the slot `End service` had — the same footprint, so the card's height and
     the order rule 15 depends on are untouched. */
  .clipbar { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 4px; }
  .clipbtn { min-width: 0; }
  .clipscrub { grid-column: 1 / -1; width: 100%; }
  .cliplevel { grid-column: 1 / -1; display: flex; align-items: center; gap: 6px; }
  .cliplevel .r-range { flex: 1; min-width: 0; }
  .clipvol { flex: 0 0 auto; min-width: 34px; text-align: right;
    font-size: var(--v-fs-lbl); color: var(--v-dim); font-variant-numeric: tabular-nums; }
  .cliptime { grid-column: 1 / -1; text-align: center; padding-top: 2px;
    font-size: var(--v-fs-lbl); color: var(--v-dim); font-variant-numeric: tabular-nums; }
  /* NOT a dash and not a zero — see the markup. Faint, because a screen that is
     not reporting is a fact about the screens, not an alarm about the clip. */
  .cliptime.unknown { color: var(--v-faint); font-variant-numeric: normal; }
  .cliperr { display: block; padding-top: 3px; font-size: var(--v-fs-lbl); color: var(--v-rose); }
  .ctlbody { display: flex; flex-direction: column; overflow: hidden; }
  .ctlbody .r-ctl { flex: 1; align-content: stretch; grid-auto-rows: 1fr; gap: 5px; }
  .ctlbody :global(.r-cbtn) { height: auto; min-height: 32px; }
</style>
