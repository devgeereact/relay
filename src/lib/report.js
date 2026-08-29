// THE SUNDAY REPORT, and the replay it is derived from.
//
// Single responsibility: turn a service's stored record into the handful of
// numbers a church would actually act on, and refuse to invent the rest.
//
// ── The rule this file exists to keep ─────────────────────────────────────────
//
// **Only metrics that were actually measured appear.** Every field here can come
// back `null`, and `null` renders as "—", never as 0. This is the same rule
// `latency.rs` enforces inside its histogram and `perf_samples` enforces in the
// schema, carried to the last hop: a report that shows 0 for something nobody
// measured is a report that gets better as the pipeline gets worse, and it is the
// exact shape of the mistake that let a field test conclude "STT is fine" from a
// backlog number while the operator watched text arrive a second and a half late.
//
// It is also why there is no "crash-free" line. Crashes are recorded per LAUNCH in
// `localStorage` (`boot.js`), not per service; there is no honest way to attribute
// one to the service that was running, so the report says nothing about it rather
// than saying something reassuring.
//
// Pure, and given plain arrays rather than a store, so the arithmetic can be tested
// without a database, a backend or a mounted component.

/** Milliseconds → `m:ss`, the same shape the rest of the History screen uses. */
export function fmtMs(ms) {
  const s = Math.round((ms || 0) / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

/**
 * The numbers, from one service's stored record.
 *
 * @param timeline rows from `service_timeline` — events, cues and detections merged
 * @param perf     rows from `service_perf` — latency snapshots taken during it
 * @param detail   `{ transcripts, detections }` from `service_detail`
 */
export function sundayReport(timeline = [], perf = [], detail = null) {
  const rows = timeline ?? [];
  const dets = rows.filter((r) => r.source === 'detection');
  const events = rows.filter((r) => r.source === 'event');

  // Duration: prefer the recorded end, fall back to the last thing that happened.
  // Null when nothing was recorded at all — an empty service is not a 0-minute one.
  const ended = events.find((r) => r.kind === 'service_ended');
  const last = rows.length ? rows[rows.length - 1].at_ms : null;
  const durationMs = ended ? ended.at_ms : last;

  const count = (kind) => dets.filter((r) => r.kind === kind).length;

  // What reached a screen, and who decided. `detections.status` is the load-bearing
  // column here (the router learns from it), so the two are never added together.
  const autoFired = count('auto');
  const manualFired = count('manual');
  const suggested = count('suggested');
  const dismissed = count('dismissed');

  // What went wrong. These have no other home — before `service_events` existed, a
  // panic control that did not reach the screens left no trace once the operator
  // dismissed the banner.
  const panicFailures = events.filter((r) => r.kind === 'panic_failed').length;
  const outputsLost = events.filter((r) => r.kind === 'output_lost').length;
  const outputsRecovered = events.filter((r) => r.kind === 'output_recovered').length;
  const lockLifted = events.filter((r) => r.kind === 'lock_lifted').length;

  return {
    durationMs,
    transcriptLines: detail?.transcripts?.length ?? null,
    autoFired,
    manualFired,
    suggested,
    dismissed,
    // Of the suggestions Relay offered, how many did the operator take? `null` when
    // it offered none — 0% would read as "the operator rejected everything", which
    // is a different and much worse claim.
    suggestionUptake: suggested + dismissed > 0 ? manualFired / (suggested + dismissed) : null,
    panicFailures,
    outputsLost,
    outputsRecovered,
    lockLifted,
    latency: latencySummary(perf),
    // Said out loud in the report itself rather than left for a reader to notice.
    notMeasured: [
      'Whether any verse shown was the RIGHT one — nothing here checks that, and only a person in the room can',
      'Word error rate, in any language',
      'Whether the app crashed — crashes are recorded per launch, not per service, and guessing which service one belonged to would be a fabrication',
    ],
  };
}

/**
 * Latency across a service, from its snapshots.
 *
 * The LAST snapshot per metric is the whole-service figure — `latency.rs`'s
 * percentiles are cumulative, so the final sample already covers everything before
 * it. The per-minute drift question ("did it get worse?") is answered by comparing
 * the first and last, which is the one thing a single median cannot say.
 */
export function latencySummary(perf = []) {
  const byMetric = new Map();
  for (const p of perf ?? []) {
    const cur = byMetric.get(p.metric);
    if (!cur || p.at_ms >= cur.last.at_ms) {
      byMetric.set(p.metric, { first: cur?.first ?? p, last: p });
    } else if (p.at_ms < cur.first.at_ms) {
      byMetric.set(p.metric, { first: p, last: cur.last });
    }
  }

  const out = [];
  for (const [metric, { first, last }] of byMetric) {
    // A metric whose stages were never reached is an ABSENCE. It is dropped rather
    // than printed as zero.
    if (!last || last.samples === 0) continue;
    out.push({
      metric,
      samples: last.samples,
      p50_ms: last.p50_ms ?? null,
      p95_ms: last.p95_ms ?? null,
      worst_ms: last.worst_ms ?? null,
      // "Did it get worse over the service?" — a rising line is the finding whatever
      // the median says (DECISIONS §38). Needs two real samples; null otherwise,
      // never `false`, because "we did not look" is not "it did not grow".
      grew:
        first && last !== first && first.p50_ms != null && last.p50_ms != null
          ? last.p50_ms > first.p50_ms * 1.25
          : null,
    });
  }
  out.sort((a, b) => a.metric.localeCompare(b.metric));
  return out;
}

/**
 * THE REPLAY — everything known about one moment in a service.
 *
 * Given a timeline row, gathers what was being said around it and what Relay did.
 * The transcript context is the point: a fire on its own says *what* went up, and
 * the words either side say *why*, which is the question somebody actually has
 * three days later.
 *
 * `window` is deliberately generous (±20 s by default). Detection runs on partial
 * hypotheses and only FINAL transcripts are stored, so the line that triggered a
 * fire may be stamped seconds away from it — narrowing this to be tidy would hide
 * the very line the operator is looking for.
 */
export function replayAt(row, detail = null, perf = [], windowMs = 20_000) {
  if (!row) return null;
  const lines = (detail?.transcripts ?? [])
    .map((t) => ({ ...t, at_ms: (t.timestamp ?? 0) * 1000 }))
    .filter((t) => Math.abs(t.at_ms - row.at_ms) <= windowMs)
    .sort((a, b) => a.at_ms - b.at_ms);

  // The detection record behind a detection row, when there is one — it carries the
  // method and confidence the timeline row does not.
  const det =
    row.source === 'detection'
      ? (detail?.detections ?? []).find(
          (d) => Math.abs((d.fired_at ?? 0) * 1000 - row.at_ms) < 1500,
        ) ?? null
      : null;

  // The most recent latency snapshot at or before this moment. `null` before the
  // first one — a service's first minute has no history behind it, and borrowing a
  // later sample would describe the wrong part of the service.
  const before = (perf ?? []).filter((p) => p.at_ms <= row.at_ms);
  const nearest = before.length
    ? before.reduce((a, b) => (b.at_ms > a.at_ms ? b : a))
    : null;

  return { row, lines, detection: det, latency: nearest };
}
