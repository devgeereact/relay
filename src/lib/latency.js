// The frontend half of the latency instrumentation — the ONE place a surface
// reports that it has PAINTED something.
//
// ── Why the frontend has to report at all ──────────────────────────────────────
//
// Rust can time everything up to the moment content leaves the machine and not one
// millisecond further, and the last leg is not small or obviously fast: a webview
// has to parse an event, run Svelte's scheduler, lay out a template at whatever
// size the projector is, and paint it. "The output path is probably quick" is an
// assumption, and this file is what replaces it with a number.
//
// ── Three rules, all learned from this codebase ────────────────────────────────
//
// 1. NEVER THROW. This runs from an event listener on the hot path, several times
//    a second, during a service. A rejected promise here must not become an
//    unhandled rejection in the middle of a sermon, and a diagnostic that can break
//    the thing it measures is worse than no diagnostic. Every path swallows.
//
// 2. REPORT AFTER THE DOM IS UPDATED, not when the event arrived. A stamp taken in
//    the listener measures the bridge and calls it rendering. `requestAnimationFrame`
//    fires after Svelte has flushed its DOM work and before the browser paints,
//    which is the closest honest moment available without a second frame's delay.
//
// 3. TWO TRANSPORTS, ONE MEANING. The console and the native output window have the
//    Tauri bridge; a kiosk browser source in OBS has only the WebSocket. Both report
//    the same stage with the same clock (`Date.now()`, which Rust places on its own
//    monotonic timeline — see `latency::from_epoch_ms`). A measurement that only
//    worked over the bridge would be blind to exactly the path a church uses.

/** Schedule `f` for after the DOM has been updated, never throwing. */
function afterPaint(f) {
  const run = () => {
    try {
      f();
    } catch {
      /* a diagnostic must never break a live service */
    }
  };
  if (typeof requestAnimationFrame === 'function') requestAnimationFrame(run);
  else setTimeout(run, 0);
}

/**
 * Report a render over the Tauri bridge. Fire-and-forget.
 *
 * `trace_id` of null/undefined means this content had no decode pass behind it —
 * an operator's own fire, a plan cue — and there is nothing to attribute. Silently
 * doing nothing is right: inventing a trace would put manual actions into a
 * percentile that describes the AI's path.
 */
export function mark(traceId, stage) {
  if (traceId === null || traceId === undefined) return;
  afterPaint(async () => {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('latency_mark', {
        traceId,
        stage,
        atEpochMs: Date.now(),
      });
    } catch {
      /* no backend (a plain browser), or the command is gone — either way, mute */
    }
  });
}

/** The console has painted a transcript update. */
export function markTranscript(traceId) {
  mark(traceId, 'transcript_rendered');
}

/**
 * An output surface has painted content.
 *
 * `ws` is a live kiosk WebSocket when this page is a browser source; pass it and
 * the mark goes back over the same connection the content arrived on, which is the
 * only way to measure the real LAN leg. Without one, the bridge is used.
 */
export function markOutput(traceId, ws) {
  if (traceId === null || traceId === undefined) return;
  if (!ws) {
    mark(traceId, 'output_rendered');
    return;
  }
  afterPaint(() => {
    // OPEN only. A queued send on a reconnecting socket would be delivered
    // seconds later and stamped as if the projector had taken seconds to paint —
    // a diagnostic that manufactures the fault it is looking for.
    if (ws.readyState !== 1) return;
    ws.send(JSON.stringify({ kind: 'rendered', trace_id: traceId, at: Date.now() }));
  });
}

/**
 * WHICH STAGE IS THE PROBLEM — the whole point of measuring nine of them.
 *
 * A field tester with a slow transcript needs one sentence, not a table: "the
 * decoder is the bottleneck, change the model" and "the console is painting
 * slowly" have nothing in common except the symptom, and the previous field test
 * concluded "STT is fine" from a backlog number while the operator was watching
 * text arrive a second and a half late.
 *
 * Pure, so it can be tested without a backend. Returns `{ verdict, detail }`.
 */
const TARGET_P50_MS = 300;

export function diagnose(report) {
  const by = {};
  for (const m of report?.metrics ?? []) by[m.metric] = m;
  const p50 = (k) => by[k]?.p50_ms ?? null;
  const n = (k) => by[k]?.samples ?? 0;

  if (!n('audio_to_partial_transcript')) {
    return { verdict: 'No speech measured yet', detail: 'Start listening and talk for a few seconds.' };
  }
  const partial = p50('audio_to_partial_transcript');
  const visible = p50('audio_to_visible_transcript');
  const decode = p50('stt_decode');

  // The webview's share is what the console adds on top of what Rust already had
  // ready. Only meaningful when the console has actually been reporting.
  const ui = n('audio_to_visible_transcript') && visible !== null && partial !== null
    ? Math.max(0, visible - partial)
    : null;

  if (ui !== null && ui > 250) {
    return {
      verdict: 'The console is the bottleneck',
      detail: `Rust has the text ${Math.round(partial)}ms after the audio, and it takes another ${Math.round(ui)}ms to appear on screen.`,
    };
  }
  // The acceptance target for a visible partial transcript. Below it there is no
  // bottleneck to report, only a dominant stage — and "the model is the
  // bottleneck" is a false alarm when the whole path takes 144ms. A verdict that
  // fires on a healthy pipeline trains the operator to ignore it.
  if (partial !== null && partial <= TARGET_P50_MS) {
    return {
      verdict: 'Keeping up',
      detail: `${Math.round(partial)}ms from audio to transcript, ${Math.round(decode ?? 0)}ms of it decoding.`,
    };
  }
  if (decode !== null && partial !== null && decode > partial * 0.6) {
    return {
      verdict: 'The speech model is the bottleneck',
      detail: `${Math.round(decode)}ms of the ${Math.round(partial)}ms is whisper decoding. A smaller model in Settings → Speech is the only thing that moves this.`,
    };
  }
  if (partial !== null && partial > 700) {
    return {
      verdict: 'The pipeline is running behind',
      detail: `${Math.round(partial)}ms from audio to transcript, and the decoder is not most of it — check dropped partials and the per-minute trend below.`,
    };
  }
  return {
    verdict: 'The pipeline is running behind',
    detail: `${Math.round(partial)}ms from audio to transcript.`,
  };
}

/**
 * Is the latency GROWING? A pipeline that starts fast and ends three seconds
 * behind passes every percentile test and fails the only one that matters, so
 * this compares the first third of the service against the last.
 *
 * Returns null when there is not enough of a service to say — deliberately, rather
 * than reporting "stable" about ninety seconds of data.
 */
export function drift(report, metric = 'audio_to_partial_transcript') {
  const series = (report?.metrics ?? []).find((m) => m.metric === metric)?.per_minute_mean_ms ?? [];
  const real = series.filter((v) => v > 0);
  if (real.length < 6) return null;
  const cut = Math.floor(real.length / 3);
  const mean = (xs) => xs.reduce((a, b) => a + b, 0) / xs.length;
  const early = mean(real.slice(0, cut));
  const late = mean(real.slice(-cut));
  return { early, late, growing: late > early * 1.25 };
}
