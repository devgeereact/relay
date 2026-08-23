// The latency diagnostic's job is to name the BOTTLENECK, not to print a number.
// The previous field test had a number — a 0ms mean backlog — and drew the wrong
// conclusion from it, so these tests are about the conclusion.

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { diagnose, drift, mark, markOutput } from './latency.js';

function report(metrics, extra = {}) {
  return {
    metrics: Object.entries(metrics).map(([metric, v]) => ({
      metric,
      samples: v.n ?? 10,
      p50_ms: v.p50 ?? null,
      p95_ms: v.p95 ?? null,
      worst_ms: v.worst ?? null,
      per_minute_mean_ms: v.per_minute ?? [],
    })),
    ...extra,
  };
}

describe('diagnose', () => {
  it('says nothing has been measured rather than inventing a verdict', () => {
    expect(diagnose(report({})).verdict).toMatch(/no speech/i);
    expect(diagnose(null).verdict).toMatch(/no speech/i);
  });

  /// THE MISDIAGNOSIS THIS EXISTS TO PREVENT. When the transcript is late the
  /// reflex is to blame the speech model, and twice that reflex was right — so a
  /// panel that always says "the model" would have looked correct and taught the
  /// operator nothing. A slow WEBVIEW has to read differently.
  it('blames the console when the webview is where the time goes', () => {
    const d = diagnose(
      report({
        audio_to_partial_transcript: { p50: 150 },
        audio_to_visible_transcript: { p50: 900 },
        stt_decode: { p50: 60 },
      })
    );
    expect(d.verdict).toMatch(/console/i);
    expect(d.detail).toContain('750ms');
  });

  it('blames the model when the decode is most of the latency', () => {
    const d = diagnose(
      report({
        audio_to_partial_transcript: { p50: 1500 },
        audio_to_visible_transcript: { p50: 1510 },
        stt_decode: { p50: 1300 },
      })
    );
    expect(d.verdict).toMatch(/speech model/i);
  });

  /// Slow, but the decoder is NOT most of it — the answer is neither of the two
  /// easy ones, and saying so is the whole value of measuring nine stages.
  it('reports a pipeline running behind that the decoder does not explain', () => {
    const d = diagnose(
      report({
        audio_to_partial_transcript: { p50: 1200 },
        audio_to_visible_transcript: { p50: 1250 },
        stt_decode: { p50: 200 },
      })
    );
    expect(d.verdict).toMatch(/running behind/i);
  });

  it('says it is keeping up when it is', () => {
    const d = diagnose(
      report({
        audio_to_partial_transcript: { p50: 144 },
        audio_to_visible_transcript: { p50: 160 },
        stt_decode: { p50: 143 },
      })
    );
    expect(d.verdict).toMatch(/keeping up/i);
  });

  /// A console that never reports back leaves `audio_to_visible` with no samples.
  /// Treating that absence as a zero would read as "the webview is instant" — the
  /// most flattering possible reading of a broken measurement.
  it('does not blame or exonerate the console when it never reported', () => {
    const d = diagnose(
      report({
        audio_to_partial_transcript: { p50: 150 },
        audio_to_visible_transcript: { n: 0, p50: null },
        stt_decode: { p50: 143 },
      })
    );
    expect(d.verdict).not.toMatch(/console/i);
  });
});

describe('drift', () => {
  /// The failure mode the acceptance criteria care about most: fine for ten
  /// minutes, seconds behind by the end. Every percentile over the whole service
  /// hides it, which is why this compares the start against the end.
  it('finds a pipeline that falls further behind the longer it runs', () => {
    const d = drift(
      report({
        audio_to_partial_transcript: { per_minute: [150, 160, 300, 700, 1400, 2600] },
      })
    );
    expect(d.growing).toBe(true);
    expect(d.late).toBeGreaterThan(d.early);
  });

  it('does not call a steady pipeline a growing one', () => {
    const d = drift(
      report({ audio_to_partial_transcript: { per_minute: [150, 160, 145, 152, 149, 158] } })
    );
    expect(d.growing).toBe(false);
  });

  /// Ninety seconds of data cannot answer a question about a ninety-minute
  /// service. Saying "stable" from six data points would be a guess wearing a
  /// verdict's clothes.
  it('declines to answer before there is enough of a service to answer about', () => {
    expect(drift(report({ audio_to_partial_transcript: { per_minute: [150, 160] } }))).toBeNull();
    expect(drift(report({}))).toBeNull();
  });
});

describe('reporting a render', () => {
  beforeEach(() => {
    vi.stubGlobal('requestAnimationFrame', (f) => f());
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  /// Content a HUMAN fired has no decode pass behind it. Reporting one would put
  /// the operator's own reflexes into a percentile that is supposed to describe
  /// how fast the AI is.
  it('reports nothing for content with no trace behind it', () => {
    const ws = { readyState: 1, send: vi.fn() };
    markOutput(null, ws);
    markOutput(undefined, ws);
    expect(ws.send).not.toHaveBeenCalled();
  });

  it('reports an output render back over the socket the content arrived on', () => {
    const ws = { readyState: 1, send: vi.fn() };
    markOutput(42, ws);
    expect(ws.send).toHaveBeenCalledTimes(1);
    const sent = JSON.parse(ws.send.mock.calls[0][0]);
    expect(sent.kind).toBe('rendered');
    expect(sent.trace_id).toBe(42);
    expect(typeof sent.at).toBe('number');
  });

  /// A queued send on a reconnecting socket is delivered whenever the socket comes
  /// back — seconds later — and would be stamped as though the projector had taken
  /// seconds to paint. A diagnostic must not manufacture the fault it looks for.
  it('does not report over a socket that is not open', () => {
    const ws = { readyState: 0, send: vi.fn() };
    markOutput(42, ws);
    expect(ws.send).not.toHaveBeenCalled();
  });

  /// This runs on the hot path several times a second during a service. An
  /// exception here is an unhandled rejection in the middle of a sermon.
  it('never throws, whatever the transport does', () => {
    const ws = {
      readyState: 1,
      send: () => {
        throw new Error('socket died');
      },
    };
    expect(() => markOutput(1, ws)).not.toThrow();
    expect(() => mark(1, 'transcript_rendered')).not.toThrow();
  });
});
