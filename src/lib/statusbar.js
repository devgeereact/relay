/**
 * THE STATUS BAR'S FACTS — one pure module, so the strip cannot invent one.
 *
 * The shell's 26px status bar (docs/REBRAND.md §2) carries six figures about the
 * machine and one sentence about the room. Every one of them is derived here,
 * from a value that is already on the bridge, and every one of them can say **it
 * does not know**.
 *
 * ── Why absence is a return value ──────────────────────────────────────────
 *
 * CLAUDE.md rule 35: a status line that reads the same when the thing behind it
 * is broken is not a status line. `Settings → Updates` printed "up to date" over
 * a channel that had been 404 for months; the Live pane read "On Air" over a
 * kiosk source that had gone away. Both were one reassuring string covering
 * several different situations.
 *
 * So nothing here returns `0` for "nothing measured". `latencyP50` returns
 * `null` when the pipeline has never timed a window, and the strip renders
 * `no data` — which is a different sentence from `0 ms`, and the difference is
 * the whole point. A zero is a measurement; an absence is not (rule 31's first
 * rule, in the other direction).
 *
 * Pure and exported so it can be tested without mounting the shell — App.svelte
 * is not unit-testable, which is exactly how the tab-redirect map went stale.
 */

/** The five states the room can be in, worst-first. One ladder, one order. */
export const WALL_STATES = Object.freeze(['safe', 'rehearsal', 'blackout', 'onair', 'clear']);

/**
 * What the room is doing, as one word of tone and one sentence of words.
 *
 * The ladder is the shell's, and this is its ONE definition — the chrome badge
 * and the status bar both read it, so they cannot end up disagreeing about the
 * same wall on the same screen. The order is not arbitrary:
 *
 *   safe mode   outranks everything, including rehearsal. Both mean "not
 *               reaching the screens", but rehearsal is a mode somebody chose for
 *               the next few minutes while safe mode is a whole copy of Relay
 *               disarmed — so it is the thing they must read, or they will spend
 *               the service wondering why nothing fires.
 *               This used to say safe mode "cannot be changed without
 *               restarting", and DECISIONS §86 made that false in the same wave
 *               it was written: `applySafeMode` is a live transition, and
 *               Settings → General turns it off without a relaunch. That was the
 *               defect §86 fixed — a label that had stopped describing the thing
 *               behind it — so the comment describing it had to stop saying so
 *               too.
 *               `safeModeFailed` is the honest half of that. Safe mode's record
 *               is written before it is enforced (DECISIONS §86), so this cell
 *               used to print "outputs disabled" over a screen that had refused
 *               to close and was still painting a verse — the FIRST branch,
 *               outranking `live`, on the one strip an operator reads all
 *               service. A cell that says the same thing whether the thing
 *               behind it worked or not is not a status cell (rule 35).
 *   rehearsal   outranks the wall: nothing is reaching a congregation, so the
 *               strip must not say On air on any tab.
 *   blackout    outranks live: something is loaded, and the wall is black.
 *   live        amber, and named — "On air" with nothing after it is the state
 *               without the fact.
 *
 * `label` is what is on the screens, already humanised by the caller (a verse
 * reference, "picture", "video"). It is never a raw payload.
 */
export function wallState({ safeMode = false, safeModeFailed = false, rehearsing = false, black = false, live = false, label = '' } = {}) {
  if (safeMode)
    return safeModeFailed
      ? { tone: 'safe', words: 'Safe mode — NOT enforced, check the screens' }
      : { tone: 'safe', words: 'Safe mode — outputs disabled' };
  if (rehearsing) return { tone: 'rehearsal', words: 'Rehearsal — nothing reaches the screens' };
  if (black) return { tone: 'blackout', words: 'Blackout' };
  if (live) return { tone: 'onair', words: label ? `On air — ${label}` : 'On air' };
  return { tone: 'clear', words: 'Screens clear' };
}

/**
 * The speech model, named the way an operator says it.
 *
 * `stt_status` returns the model's absolute PATH, because that is what Settings
 * needs to show when somebody is chasing a missing file. In a 26px strip it is
 * 80 characters of directory and six of answer, so the basename is taken and
 * whisper.cpp's `ggml-` prefix and `.bin` suffix are dropped:
 * `…/models/ggml-large-v3-turbo.bin` → `large-v3-turbo`.
 *
 * The same fact, shortened — not a second one. `null` when no model is loaded,
 * because "which model" and "no speech recognition at all" are different
 * answers and only one of them is a model name. Rule 32: the model IS part of
 * the cadence figure beside it, which is why this cell exists at all.
 */
export function modelLabel(path) {
  if (typeof path !== 'string') return null;
  const base = path.split(/[\\/]/).pop() ?? '';
  const name = base.replace(/\.bin$/i, '').replace(/^ggml-/i, '');
  return name.trim() || null;
}

/**
 * The p50 of one latency metric, in whole milliseconds — or `null`.
 *
 * `latency_report` carries a `metrics` array, one entry per named span
 * (`latency.rs`). The strip asks for `audio_to_partial_transcript`, which is the
 * span rule 32 quotes and the one that exists during every service rather than
 * only after a fire: `end_to_end_speech_to_scripture` has no samples at all
 * until scripture has reached a screen, so a strip built on it would read
 * "no data" for the first half of most services and look broken.
 *
 * `null` for: no report, measurement switched off, or a metric with no samples.
 * A metric present with zero samples reports `p50_ms: null` from Rust already —
 * this never turns that into a 0.
 */
export function latencyP50(report, metric = 'audio_to_partial_transcript') {
  const m = report?.metrics?.find?.((x) => x.metric === metric);
  const v = m?.p50_ms;
  if (typeof v !== 'number' || !Number.isFinite(v)) return null;
  return Math.round(v);
}

/**
 * Transcript updates per second, to one decimal — or `null`.
 *
 * Rule 32 again: `ggml-base` steps at ~4.7/s and `ggml-large-v3-turbo` at about
 * 1.25/s on the same machine, and "a church that chose turbo chose a quarter of
 * the cadence and nothing told them". This cell is the thing that tells them.
 */
export function cadence(report) {
  const v = report?.transcript_updates_per_s;
  if (typeof v !== 'number' || !Number.isFinite(v)) return null;
  return v.toFixed(1);
}

/**
 * What has been shed, as a figure and a severity.
 *
 * Two counters, deliberately not summed (rule 33). A shed PARTIAL is re-decoded
 * a moment later; shed AUDIO is a piece of the sermon Relay never heard at all,
 * which is strictly worse news. The cell shows partials — the number that moves
 * first — and turns red the moment either is non-zero, with `audio` carried
 * alongside so the caller can say which in the title.
 *
 * `null` when there is no report: zero shed and never asked are not the same.
 */
export function dropped(report) {
  if (!report) return null;
  const partials = Number(report.dropped_partials ?? 0);
  const audio = Number(report.dropped_audio ?? 0);
  if (!Number.isFinite(partials) || !Number.isFinite(audio)) return null;
  return { partials, audio, bad: partials > 0 || audio > 0 };
}

/**
 * How many screens are showing the programme, out of how many exist.
 *
 * Takes the already-described screens — the SAME `describeScreen` verdicts the
 * chrome lamps render and the Live pane and the Outputs table read — so the
 * count and the lamps beside it can never disagree about the same screen
 * (rule 35; RG-01 is the instance this rule was written from).
 *
 * A screen counts as live only at `onair`. Not `rehearsal`: during a rehearsal
 * nothing reaches a congregation, and a strip that counted two screens as live
 * while the shell said "nothing is reaching the screens" would be the colour law
 * broken in arithmetic.
 */
export function screenTally(described) {
  const rows = Array.isArray(described) ? described : [];
  return { live: rows.filter((r) => r?.kind === 'onair').length, total: rows.length };
}

/**
 * A duration as hh:mm:ss — or `null` for "not running".
 *
 * `null` rather than `00:00:00`, because a zeroed clock is a measurement of
 * nothing and reads exactly like a clock that has stopped. The strip prints the
 * absence instead.
 */
export function elapsed(ms) {
  if (typeof ms !== 'number' || !Number.isFinite(ms) || ms < 0) return null;
  const s = Math.floor(ms / 1000);
  return [Math.floor(s / 3600), Math.floor(s / 60) % 60, s % 60]
    .map((n) => String(n).padStart(2, '0'))
    .join(':');
}

/** What a cell prints when the fact behind it is absent. One string, one place. */
export const NO_DATA = 'no data';

/** `v` if it is a real answer, the absence marker if it is not. */
export const orNoData = (v) => (v === null || v === undefined || v === '' ? NO_DATA : String(v));
