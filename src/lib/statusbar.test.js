// THE STATUS BAR'S FACTS (docs/REBRAND.md §2).
//
// Six figures and one sentence, in a 26px strip an operator glances at rather
// than reads. The whole risk in a strip like that is rule 35: a cell that prints
// something reassuring whatever is behind it. `0 ms` over a pipeline that has
// never timed a window looks exactly like a fast pipeline. `large-v3-turbo` over
// a model that failed to load looks exactly like speech recognition working.
//
// So every assertion below is about the difference between a MEASUREMENT and an
// ABSENCE, and each was watched to fail by making the module return a zero or a
// last-known value instead.
//
//   npx vitest run src/lib/statusbar.test.js
import { describe, it, expect } from 'vitest';
import {
  wallState,
  modelLabel,
  latencyP50,
  cadence,
  dropped,
  screenTally,
  elapsed,
  orNoData,
  NO_DATA,
} from './statusbar.js';

describe('the state line is one ladder, worst first', () => {
  // The chrome badge and the status bar read this same function, which is why
  // the order is here and not in either of them. Two ladders is how one strip
  // ends up saying On Air while the other says Rehearsal, on the same screen.
  it('safe mode outranks rehearsal, which outranks the wall', () => {
    // Both mean "not reaching the screens" — but safe mode also means the
    // operator cannot change that without restarting, so it has to be the thing
    // they read.
    expect(wallState({ safeMode: true, rehearsing: true, live: true }).tone).toBe('safe');
    expect(wallState({ rehearsing: true, live: true, black: true }).tone).toBe('rehearsal');
    expect(wallState({ black: true, live: true }).tone).toBe('blackout');
  });

  it('on air is named, never bare', () => {
    // "On air" with nothing after it is the state without the fact. The label is
    // what an operator checks against the wall behind them.
    expect(wallState({ live: true, label: 'Romans 8:29' }).words).toBe('On air — Romans 8:29');
    expect(wallState({ live: true, label: 'Romans 8:29' }).tone).toBe('onair');
  });

  it('rehearsal says what rehearsal MEANS, not just its name', () => {
    expect(wallState({ rehearsing: true }).words).toMatch(/nothing reaches the screens/);
  });

  it('nothing on the screens is a neutral state, not an alarm and not on air', () => {
    const s = wallState({});
    expect(s.tone).toBe('clear');
    expect(s.words).toBe('Screens clear');
  });
});

describe('the model is the same fact, shortened — never a different one', () => {
  it('names the model the way an operator says it', () => {
    expect(modelLabel('/Users/x/Library/Application Support/com.relay.app/models/ggml-large-v3-turbo.bin'))
      .toBe('large-v3-turbo');
    // Windows separators, because half the pilot machines are Windows.
    expect(modelLabel('C:\\Users\\x\\AppData\\Roaming\\com.relay.app\\models\\ggml-base.bin')).toBe('base');
  });

  it('says nothing at all when there is no model', () => {
    // "which model" and "no speech recognition at all" are different answers and
    // only one of them is a model name.
    expect(modelLabel(null)).toBe(null);
    expect(modelLabel(undefined)).toBe(null);
    expect(modelLabel('')).toBe(null);
  });
});

describe('a figure nothing measured is an absence, not a zero', () => {
  const REPORT = {
    metrics: [
      { metric: 'audio_to_partial_transcript', p50_ms: 137.4 },
      { metric: 'end_to_end_speech_to_scripture', p50_ms: null },
    ],
    transcript_updates_per_s: 1.37,
    dropped_partials: 0,
    dropped_audio: 0,
  };

  it('reads the p50 of the span that exists during every service', () => {
    expect(latencyP50(REPORT)).toBe(137);
  });

  it('a metric with no samples is null, not 0', () => {
    // Rule 31's first rule, in the other direction: a stage never reached is an
    // ABSENCE, not a zero. `end_to_end_speech_to_scripture` has no samples at all
    // until scripture has reached a screen, which is why the strip does not ask
    // it — but the null must survive whoever does.
    expect(latencyP50(REPORT, 'end_to_end_speech_to_scripture')).toBe(null);
    expect(latencyP50(REPORT, 'a_metric_that_does_not_exist')).toBe(null);
  });

  it('no report at all is null everywhere', () => {
    expect(latencyP50(null)).toBe(null);
    expect(cadence(null)).toBe(null);
    expect(dropped(null)).toBe(null);
    expect(dropped(undefined)).toBe(null);
  });

  it('the cadence is the figure the model choice actually moves', () => {
    // Rule 32: `base` steps about 4.7/s and `large-v3-turbo` about 1.25/s on the
    // same machine, and nothing told a church that chose turbo.
    expect(cadence(REPORT)).toBe('1.4');
    expect(cadence({ transcript_updates_per_s: 4.74 })).toBe('4.7');
  });

  it('zero shed is a real answer and reads differently from never asked', () => {
    const d = dropped(REPORT);
    expect(d).toEqual({ partials: 0, audio: 0, bad: false });
    expect(dropped(null)).toBe(null);
  });

  it('audio Relay never heard makes the cell bad, even with no partials shed', () => {
    // Rule 33: a shed partial is re-decoded a moment later; shed audio is a piece
    // of the sermon that was never heard. The cell shows partials, so the second
    // counter has to be what turns it red or it is invisible.
    const d = dropped({ dropped_partials: 0, dropped_audio: 3 });
    expect(d.bad).toBe(true);
    expect(d.audio).toBe(3);
  });
});

describe('the screen tally cannot disagree with the lamps beside it', () => {
  // It is handed the already-described screens — the same `describeScreen`
  // verdicts the chrome lamps render — rather than the raw health rows, which is
  // what makes "cannot disagree" a property rather than a promise (RG-01).
  const DESCRIBED = [
    { kind: 'onair' },
    { kind: 'onair' },
    { kind: 'down' },
    { kind: 'idle' },
    { kind: 'ready' },
  ];

  it('counts only the screens actually showing the programme', () => {
    expect(screenTally(DESCRIBED)).toEqual({ live: 2, total: 5 });
  });

  it('a rehearsal is nobody looking at anything', () => {
    // Counting a rehearsing screen as live would be the colour law broken in
    // arithmetic: the shell says "nothing is reaching the screens" and the strip
    // beside it would say two of five are.
    expect(screenTally([{ kind: 'rehearsal' }, { kind: 'rehearsal' }])).toEqual({ live: 0, total: 2 });
  });

  it('no screens is zero of zero, not an error', () => {
    expect(screenTally([])).toEqual({ live: 0, total: 0 });
    expect(screenTally(null)).toEqual({ live: 0, total: 0 });
  });
});

describe('a clock that is not running prints nothing rather than zero', () => {
  it('formats as hh:mm:ss', () => {
    expect(elapsed(43_000)).toBe('00:00:43');
    expect(elapsed(3_723_000)).toBe('01:02:03');
  });

  it('an absent duration is an absence', () => {
    // `00:00:00` is a measurement of nothing and reads exactly like a clock that
    // has stopped — which is the one thing an operator would need to notice.
    expect(elapsed(null)).toBe(null);
    expect(elapsed(undefined)).toBe(null);
    expect(elapsed(NaN)).toBe(null);
    expect(elapsed(-1)).toBe(null);
  });

  it('but a genuine zero elapsed is a real reading', () => {
    expect(elapsed(0)).toBe('00:00:00');
  });
});

describe('one string for "we do not know"', () => {
  it('prints the absence marker for every shape of nothing', () => {
    expect(orNoData(null)).toBe(NO_DATA);
    expect(orNoData(undefined)).toBe(NO_DATA);
    expect(orNoData('')).toBe(NO_DATA);
  });

  it('and never swallows a real zero', () => {
    // A `||` here instead of an explicit null check would turn `DROPPED 0` — the
    // healthy reading — into "no data" for the whole of every good service.
    expect(orNoData(0)).toBe('0');
    expect(orNoData('00:00:00')).toBe('00:00:00');
  });
});
