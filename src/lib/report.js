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

  // ── WHAT THE OPERATOR DID WITH RELAY'S SUGGESTIONS ──────────────────────────
  //
  // These used to be `count('suggested')` and `count('dismissed')`, and they were
  // **always 0** — not because no suggestion was ever offered or rejected, but
  // because nothing wrote either row. `detections` is only ever inserted from
  // `persist_fire`, which runs for a fire that reaches a screen, so the column can
  // hold `'auto'` or `'manual'` and nothing else in a real service. Two of its
  // four documented values were structurally unreachable.
  //
  // A count of 0 for something nothing measures is precisely the failure this file
  // exists to prevent: it reads as *"Relay never offered you anything"*, which is a
  // claim, and a false one. `null` reads as "—".
  //
  // They now come from `cues` — what the operator actually pressed — written by
  // `confirm_detection` and `dismiss_detection`. Suggestions themselves are still
  // not persisted, deliberately: they are not debounced (CLAUDE.md rule 28), so one
  // spoken paraphrase produces a suggestion per decode pass and hundreds of rows a
  // minute that no person experienced as separate.
  const cues = rows.filter((r) => r.source === 'cue');
  const cueCount = (kind) => cues.filter((r) => r.kind === kind).length;
  const accepted = cueCount('suggestion_accepted');
  const rejected = cueCount('suggestion_dismissed');
  const actedOn = accepted + rejected;

  // ── AND WHAT RELAY OFFERED, WHICH IS NEW AND IS THE DENOMINATOR (RG-309) ────
  //
  // The paragraph above is still the operator's half and still comes from `cues`.
  // What it could not give is the other half: how many suggestions there WERE. Until
  // RG-309, `persist_fire` ran only inside `if fire.may_broadcast()` and a paraphrase
  // can never broadcast (rule 10), so `status = 'suggested'` was structurally
  // unreachable and this file said 0 for it — which is why these counters were moved
  // to `cues` in the first place. The rows exist now.
  //
  // **From `detail.detections`, NOT from the timeline.** `service_timeline` is the
  // one ordered record of what HAPPENED and deliberately excludes offers — a
  // 16-hour service offers thousands of them against a few hundred fires, and a
  // timeline that was 95% suggestions would be unreadable and would break the
  // replay's own index into it. The forensic list is `service_detections`, and this
  // is the only figure that needs the whole of it.
  //
  // `null`, not 0, when the service predates the change or recorded none. A service
  // from last month has no suggestion rows and never will, and printing `0 offered`
  // over it is the same false claim in a new column.
  const offered = splitDetections(detail?.detections).offered.length;
  const suggestionsOffered = offered || null;

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
    // Named for what they are. `suggested`/`dismissed` implied a count of what
    // Relay OFFERED; these are counts of what the operator DECIDED, and the two are
    // different questions. `null` when the operator acted on none, because 0
    // accepted out of 0 offered is not the same statement as 0 accepted out of 40.
    suggestionsAccepted: actedOn ? accepted : null,
    suggestionsRejected: actedOn ? rejected : null,
    // Of the suggestions the operator ACTED on, how many did they take? `null` when
    // they acted on none — 0% would read as "the operator rejected everything",
    // which is a different and much worse claim.
    suggestionUptake: actedOn ? accepted / actedOn : null,
    suggestionsOffered,
    // Of everything Relay offered, how much did the operator answer AT ALL? This is
    // the figure `suggestionUptake` deliberately is not: uptake is out of the ones
    // they answered, and this is out of everything. Measured on the author's own
    // 16-hour service of 2026-09-25 it is one acceptance against roughly 8,000
    // offers, and that number is the finding rather than a rounding error.
    suggestionsAnswered: suggestionsOffered ? actedOn / suggestionsOffered : null,
    panicFailures,
    outputsLost,
    outputsRecovered,
    lockLifted,
    latency: latencySummary(perf),
    // Said out loud in the report itself rather than left for a reader to notice.
    notMeasured: [
      'Whether any verse shown was the RIGHT one — nothing here checks that, and only a person in the room can',
      'Whether a suggestion you never answered was RIGHT — the offers are recorded now, with the words behind each one, but nothing here judges them and only a person who was in the room can',
      'Word error rate, in any language',
      'Whether the app crashed — crashes are recorded per launch, not per service, and guessing which service one belonged to would be a fabrication',
    ],
  };
}

/**
 * A service's detection rows split by whether they reached a screen — RG-309.
 *
 * ## Why this is a function and not a filter written twice
 *
 * Until suggestions were persisted, every row in `detections` had been on a screen,
 * so `db::service_detections` returning "the service's detections" and History
 * rendering them all under **Detected verses (N)** were the same statement. Once
 * `status = 'suggested'` became reachable they stopped being the same statement, by
 * roughly twenty to one: the author's service of 2026-09-25 put **365** verses on a
 * screen and offered in the region of **8,000**, and the column would have printed
 * the second number under the first word.
 *
 * That is rule 35 in the archive rather than on the desk — a figure whose meaning
 * changed while its label did not — and a filter inlined at the one surface that
 * renders it today is how the next surface gets it wrong. `status` is the load-bearing
 * column (the router learns from it, rule 14) and this is the one place its four
 * values are turned into the two questions a reader actually has.
 *
 * **Total over the input, deliberately.** A fifth status added to the CHECK
 * constraint one day lands in `offered` rather than vanishing: a detection the
 * history quietly stops showing is worse than one filed under the wrong heading.
 */
export function splitDetections(list) {
  const rows = Array.isArray(list) ? list : [];
  const reached = (d) => d?.status === 'auto' || d?.status === 'manual';
  return {
    fired: rows.filter(reached),
    offered: rows.filter((d) => !reached(d)),
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
      // One window in a hundred. Over a ninety-minute service that is roughly one
      // visibly late verse — the thing a congregation notices and a median cannot
      // show. `null` on a service recorded before p99 existed, which is an absence
      // and renders as one.
      p99_ms: last.p99_ms ?? null,
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

/**
 * WEEK BY WEEK — is it getting slower across services, not within one?
 *
 * `latencySummary` answers "did it grow during this service" (DECISIONS §38). This
 * answers the slower question a church actually lives with: a bigger model added in
 * March, a laptop that fills up over a winter, a room that got louder. Every
 * individual Sunday looks fine.
 *
 * Rows arrive newest first. Returns `null` — not `false` — with fewer than three
 * services, because two points are a line through anything and "we have not seen
 * enough yet" is a different statement from "it is not getting worse".
 */
export function weekOnWeek(rows = [], field = 'p50_ms') {
  const usable = (rows ?? []).filter((r) => r[field] != null);
  if (usable.length < 3) return null;

  // Compare the most recent against the median of the ones before it. A mean would
  // let one catastrophic Sunday — a service run on a laptop that was compiling
  // something — hide a real trend, or invent one.
  const [latest, ...older] = usable;
  const values = older.map((r) => r[field]).sort((a, b) => a - b);
  const mid = values.length % 2
    ? values[(values.length - 1) / 2]
    : (values[values.length / 2 - 1] + values[values.length / 2]) / 2;

  return {
    latest: latest[field],
    typical: mid,
    services: usable.length,
    // A quarter slower than usual is the point at which an operator would notice,
    // and it is the same threshold `latency.js` uses for drift within a service —
    // one number, two places, deliberately.
    slower: latest[field] > mid * 1.25,
    faster: latest[field] < mid * 0.8,
  };
}

/** One sentence about the trend, or '' when there is not enough to say. */
export function describeTrend(t, metricLabel = 'the transcript') {
  if (!t) return '';
  const pct = Math.round(Math.abs(t.latest - t.typical) / t.typical * 100);
  if (t.slower)
    return `${metricLabel} was about ${pct}% slower this time than the last ${t.services - 1} services.`;
  if (t.faster) return `${metricLabel} was about ${pct}% faster this time than usual.`;
  return `${metricLabel} was about the same as the last ${t.services - 1} services.`;
}
