// RG-07 · RG-08 — the replay, and the Sunday report derived from it.
//
// The arithmetic is pure and lives in `report.js`, so these run without a database,
// a backend or a mounted component. What they are really guarding is one rule:
//
//   ONLY WHAT WAS MEASURED APPEARS, and an absence never renders as a zero.
//
// A report that shows 0 for something nobody measured is a report that improves as
// the pipeline gets worse — the exact shape of the mistake that let a field test
// conclude "STT is fine" from a backlog number while the operator watched text land
// a second and a half late.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { sundayReport, latencySummary, replayAt, fmtMs } from './report.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

const ev = (at, kind, detail = null) => ({ at_ms: at, source: 'event', kind, detail });
const det = (at, kind, detail = null) => ({ at_ms: at, source: 'detection', kind, detail });
const cue = (at, kind) => ({ at_ms: at, source: 'cue', kind, detail: null });

const SERVICE = [
  ev(0, 'service_started', 'Sunday Service'),
  det(30_000, 'auto', 'John 3:16'),
  det(60_000, 'suggested', 'Romans 8:28'),
  det(65_000, 'manual', 'Romans 8:28'),
  det(90_000, 'dismissed', 'Psalms 23:1'),
  ev(100_000, 'output_lost', 'Main screen'),
  ev(112_000, 'output_recovered', 'Main screen'),
  cue(120_000, 'clear_screens'),
  ev(130_000, 'panic_failed', 'clear'),
  ev(3_600_000, 'service_ended'),
];

describe('the Sunday report', () => {
  const r = sundayReport(SERVICE, [], { transcripts: [{}, {}, {}], detections: [] });

  it('separates who decided — the AI and the operator are never added together', () => {
    // `detections.status` is the column the self-calibrating router learns from.
    // A report that merged them would describe a service that did not happen.
    expect(r.autoFired).toBe(1);
    expect(r.manualFired).toBe(1);
    expect(r.suggested).toBe(1);
    expect(r.dismissed).toBe(1);
  });

  it('counts the things that had no other home', () => {
    expect(r.panicFailures).toBe(1);
    expect(r.outputsLost).toBe(1);
    expect(r.outputsRecovered).toBe(1);
  });

  it('takes the length from the recorded end, not the last thing that happened', () => {
    expect(r.durationMs).toBe(3_600_000);
    expect(fmtMs(r.durationMs)).toBe('60:00');
  });

  it('falls back to the last event when a service never recorded an end', () => {
    // A crashed or force-quit service has no `service_ended` row. Reporting 0
    // would say it lasted no time at all.
    const crashed = SERVICE.filter((x) => x.kind !== 'service_ended');
    expect(sundayReport(crashed).durationMs).toBe(130_000);
  });

  it('reports NOTHING rather than zero for a service with no record', () => {
    const empty = sundayReport([], [], null);
    expect(empty.durationMs).toBeNull();
    expect(empty.transcriptLines).toBeNull();
  });

  it('uptake is null when nothing was suggested — not 0%', () => {
    // 0% reads as "the operator rejected everything", which is a different and
    // much worse claim than "Relay offered nothing".
    const quiet = sundayReport([ev(0, 'service_started'), det(10, 'auto', 'John 1:1')]);
    expect(quiet.suggestionUptake).toBeNull();
    expect(quiet.autoFired).toBe(1);
  });

  it('names what it does not measure, in the report itself', () => {
    // A report that lists only what it measured, without naming what it did not,
    // invites somebody to read the absence as a pass.
    const said = r.notMeasured.join(' ').toLowerCase();
    expect(said).toMatch(/right one/);
    expect(said).toMatch(/word error rate/);
    expect(said).toMatch(/crash/);
  });

  it('says nothing about crashes, because nothing can attribute one to a service', () => {
    // `boot.js` records crashes per LAUNCH in localStorage. There is no honest way
    // to say which service one belonged to, so there is no crash-free line.
    expect(Object.keys(r)).not.toContain('crashFree');
    expect(read('src/lib/report.js')).toMatch(/no "crash-free" line/i);
  });
});

describe('latency over a whole service', () => {
  const perf = [
    { at_ms: 60_000, metric: 'audio_to_partial_transcript', samples: 100, p50_ms: 140, p95_ms: 340, worst_ms: 500 },
    { at_ms: 600_000, metric: 'audio_to_partial_transcript', samples: 900, p50_ms: 152, p95_ms: 360, worst_ms: 740 },
    { at_ms: 600_000, metric: 'reference_detection_to_fire', samples: 0, p50_ms: null, p95_ms: null, worst_ms: null },
  ];

  it('takes the LAST snapshot per metric — the percentiles are cumulative', () => {
    const l = latencySummary(perf);
    const partial = l.find((x) => x.metric === 'audio_to_partial_transcript');
    expect(partial.samples).toBe(900);
    expect(partial.p50_ms).toBe(152);
  });

  it('drops a metric whose stages were never reached, rather than printing zeros', () => {
    expect(latencySummary(perf).map((x) => x.metric)).not.toContain('reference_detection_to_fire');
  });

  it('answers "did it get worse" — and says null when it could not look', () => {
    // A rising line is the finding whatever the median says (DECISIONS §38). One
    // sample cannot answer it, and `false` would be a claim nobody checked.
    expect(latencySummary(perf).find((x) => x.metric === 'audio_to_partial_transcript').grew).toBe(false);

    const grew = [
      { at_ms: 60_000, metric: 'm', samples: 10, p50_ms: 140, p95_ms: 300, worst_ms: 400 },
      { at_ms: 900_000, metric: 'm', samples: 90, p50_ms: 400, p95_ms: 900, worst_ms: 1200 },
    ];
    expect(latencySummary(grew)[0].grew).toBe(true);

    const once = [{ at_ms: 60_000, metric: 'm', samples: 10, p50_ms: 140, p95_ms: 300, worst_ms: 400 }];
    expect(latencySummary(once)[0].grew).toBeNull();
  });

  it('survives an empty or missing set', () => {
    expect(latencySummary()).toEqual([]);
    expect(latencySummary([])).toEqual([]);
  });
});

describe('the replay', () => {
  const detail = {
    transcripts: [
      { timestamp: 10, text: 'good morning everyone', language: 'en' },
      { timestamp: 28, text: 'let us turn to john chapter three', language: 'en' },
      { timestamp: 33, text: 'verse sixteen', language: 'en' },
      { timestamp: 300, text: 'much later in the sermon', language: 'en' },
    ],
    detections: [
      { reference: 'John 3:16', method: 'direct', confidence: 0.91, status: 'auto', fired_at: 30 },
    ],
  };

  it('gathers the words either side — WHAT went up plus WHY', () => {
    const r = replayAt(det(30_000, 'auto', 'John 3:16'), detail, []);
    expect(r.lines.map((l) => l.text)).toEqual([
      'good morning everyone',
      'let us turn to john chapter three',
      'verse sixteen',
    ]);
    expect(r.lines.map((l) => l.text)).not.toContain('much later in the sermon');
  });

  it('finds the detection record, which carries the method the timeline row does not', () => {
    const r = replayAt(det(30_000, 'auto', 'John 3:16'), detail, []);
    expect(r.detection.method).toBe('direct');
    expect(r.detection.confidence).toBeCloseTo(0.91);
  });

  it('returns no detection for an event row, rather than the nearest one', () => {
    // An output going quiet is not a verse. Attaching the nearest detection to it
    // would invent a causal link nobody recorded.
    const r = replayAt(ev(30_000, 'output_lost', 'Main screen'), detail, []);
    expect(r.detection).toBeNull();
  });

  it('reports an empty transcript window as empty, not as the whole service', () => {
    const r = replayAt(ev(3_000_000, 'service_ended'), detail, []);
    expect(r.lines).toEqual([]);
  });

  it('uses the latency snapshot from BEFORE that moment, never a later one', () => {
    // Borrowing a later sample would describe the wrong part of the service — and
    // a service's first minute genuinely has no history behind it.
    const perf = [
      { at_ms: 60_000, metric: 'm', samples: 10, p50_ms: 140 },
      { at_ms: 600_000, metric: 'm', samples: 90, p50_ms: 400 },
    ];
    expect(replayAt(det(30_000, 'auto'), detail, perf).latency).toBeNull();
    expect(replayAt(det(120_000, 'auto'), detail, perf).latency.p50_ms).toBe(140);
    expect(replayAt(det(900_000, 'auto'), detail, perf).latency.p50_ms).toBe(400);
  });

  it('is null for no row at all', () => {
    expect(replayAt(null)).toBeNull();
  });
});

describe('what the History screen may claim', () => {
  const view = read('src/lib/views/library/History.svelte');

  it('renders a null metric as "—", never as a number', () => {
    expect(view).toMatch(/const num = \(v\) =>[\s\S]{0,80}'—'/);
    expect(view).toMatch(/const pct = \(v\) =>[\s\S]{0,60}'—'/);
  });

  it('shows what the report does not measure', () => {
    expect(view).toMatch(/What this does not tell you/);
  });

  it('a panic control that failed is not styled as a statistic', () => {
    expect(view).toMatch(/class:bad=\{report\.panicFailures > 0\}/);
    const styles = view.slice(view.indexOf('.lib-rep-cell.bad'));
    expect(styles.slice(0, 200)).toMatch(/--v-rose/);
    expect(styles.slice(0, 200)).not.toMatch(/--v-amber/);
  });

  it('the archive obeys the number rule — a guess never shows a score', () => {
    // A TF-IDF cosine does not become a probability by being a week old.
    const replay = view.slice(view.indexOf('What Relay decided'));
    expect(replay.slice(0, 900)).toMatch(/showsConfidence\(replay\.detection\)/);
    expect(replay.slice(0, 900)).toMatch(/no score — a guess/);
  });

  it('a timeline row is a real button, so it is reachable by keyboard', () => {
    expect(view).toMatch(/<button[\s\S]{0,200}class="lib-tl-row"/);
    expect(view).toMatch(/aria-expanded=\{replayIdx === i\}/);
  });
});
