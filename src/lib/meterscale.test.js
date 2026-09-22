// THE LEVEL METER'S ARITHMETIC (RG-257).
//
// The operator: *"Can we have the audio wave like that of OBS with horizontal
// wave and all"*. OBS's mixer draws a horizontal bar on a dBFS scale with a held
// peak, which is a different instrument from the scrolling trace this card has
// had: the trace answers *what has the room been doing for twenty seconds*, and
// a meter answers *how loud is it right now, and did it clip*.
//
// **Rule 12 was checked before any of this was drawn, because it decides whether
// the instrument is allowed at all.** The rule is *"nothing may compare a signal
// to an absolute level"*, and DECISIONS §19 spells out what it governs: the
// VOICE GATE and the AUTO-GAIN, which must track the room rather than a number.
// A meter does not decide anything — it draws — and §51 says so in its own
// words: *"a level meter moving is not 'Relay heard a voice'"*. The product
// already ships two horizontal meters on absolute scales (`FirstRun.svelte`,
// `views/Settings.svelte`), and the existing trace already plots linear
// amplitude against an absolute box height, so a dB axis is a change of units
// rather than a change of kind.
//
// **What stays forbidden, and these cases hold the line:** no band that calls a
// level good, quiet or hot — that is *"this many dB = speech"* in a costume —
// and the only absolute mark permitted is full scale, which is the one
// unambiguous fault in audio.
import { describe, it, expect } from 'vitest';
import { dbOf, meterFill, METER_FLOOR_DB, peakOf, CLIP_DB } from './meterscale.js';

describe('dbOf — linear amplitude as the unit a meter is read in', () => {
  it('full scale is nought', () => {
    expect(dbOf(1)).toBe(0);
  });

  it('half amplitude is about six decibels down', () => {
    expect(dbOf(0.5)).toBeCloseTo(-6.02, 1);
  });

  it('silence is the floor, not minus infinity', () => {
    // An infinity cannot be drawn and cannot be compared. The floor is the
    // bottom of the scale, which is what a meter shows for silence anyway.
    expect(dbOf(0)).toBe(METER_FLOOR_DB);
    expect(dbOf(-1)).toBe(METER_FLOOR_DB);
  });

  it('nothing below the floor is reported as below the floor', () => {
    expect(dbOf(0.0000001)).toBe(METER_FLOOR_DB);
  });
});

describe('meterFill — how much of the bar is lit', () => {
  it('full scale fills it', () => {
    expect(meterFill(0)).toBe(1);
  });

  it('the floor lights none of it', () => {
    expect(meterFill(METER_FLOOR_DB)).toBe(0);
  });

  it('is linear in decibels, which is what makes the scale readable', () => {
    // A bar linear in AMPLITUDE spends four fifths of its length on the top
    // 12 dB and leaves a quiet preacher pinned to the left edge. This is the
    // whole reason the meter is drawn in dB rather than in the figure the
    // event carries.
    expect(meterFill(METER_FLOOR_DB / 2)).toBeCloseTo(0.5, 5);
  });

  it('never runs past either end', () => {
    expect(meterFill(12)).toBe(1);
    expect(meterFill(-200)).toBe(0);
  });
});

describe('peakOf — the held peak, over readings the card already keeps', () => {
  const buf = [
    { t: 1000, v: 0.2 },
    { t: 1500, v: 0.9 },
    { t: 2500, v: 0.3 },
    { t: 2900, v: 0.25 },
  ];

  it('is the loudest thing inside the window', () => {
    expect(peakOf(buf, 3000, 2000)).toBeCloseTo(0.9, 5);
  });

  it('forgets a peak once it falls out of the window, so the mark decays', () => {
    // A peak held for ever is a peak about a service rather than about now.
    // At 3400 the 0.9 at t=1500 is 1900ms old and outside a 1s window; the two
    // readings still inside it top out at 0.3.
    expect(peakOf(buf, 3400, 1000)).toBeCloseTo(0.3, 5);
  });

  it('is null when there is nothing in the window at all', () => {
    // NOT a zero. A meter with no readings is a meter that cannot say, and a
    // zero draws a mark at the floor as though it had measured silence.
    expect(peakOf(buf, 9000, 1000)).toBeNull();
    expect(peakOf([], 1000, 1000)).toBeNull();
  });
});

describe('the one absolute mark this meter is allowed', () => {
  it('is full scale, and nothing else', () => {
    // DECISIONS §19 grants exactly this: the sample had nowhere left to go, so
    // saying it is a measurement rather than an assumption. There is
    // deliberately NO "too quiet", NO "good level" and NO "hot but not
    // clipping" — the last of those is named and refused in `readingKind`'s own
    // comment, because inventing a warning level is inventing the absolute
    // threshold rule 12 removed.
    expect(CLIP_DB).toBe(0);
  });

  it('and the module names no other threshold', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const src = readFileSync(resolve('src/lib/meterscale.js'), 'utf8');
    for (const banned of ['GOOD_', 'HOT_', 'QUIET_DB', 'TARGET_DB', 'SPEECH_DB']) {
      expect(src, `the meter invented a ${banned} threshold`).not.toContain(banned);
    }
  });
});

// ── AND THE CARD ACTUALLY DRAWS IT ──────────────────────────────────────────
//
// The arithmetic above is pure and the rules are the interesting half, but a
// module nothing renders is not covered however green its tests — which this
// session has already paid for once, with a `formatCountdown` imported from the
// file that does not export it.
describe('the Live audio card wears the meter', () => {
  const DOCK = (() => {
    const { readFileSync } = require('node:fs');
    const { resolve } = require('node:path');
    return readFileSync(resolve('src/lib/Dock.svelte'), 'utf8');
  })();

  it('renders a bar, a held peak and a scale', () => {
    expect(DOCK).toMatch(/class="meter"/);
    expect(DOCK).toMatch(/class="mfill"/);
    expect(DOCK).toMatch(/class="mpeak"/);
    expect(DOCK).toMatch(/class="mticks/);
  });

  it('feeds it from this module rather than doing the arithmetic twice', () => {
    expect(DOCK).toContain("from './meterscale.js'");
    expect(DOCK).toMatch(/meterFill\(dbOf\(/);
    expect(DOCK).toMatch(/peakOf\(waveBuf/);
  });

  it('colours the bar from the VOICE GATE, never from a level', () => {
    // The same rule the trace keeps. A bar that turned green at some number of
    // decibels would be drawing the gate as a threshold it is not.
    expect(DOCK).toMatch(/class:voice=\{\$meter\.isVoice\}/);
    const css = DOCK.slice(DOCK.indexOf('.mfill {'), DOCK.indexOf('.mticks {'));
    expect(css, 'the meter invented a colour of its own').not.toMatch(/--v-amber|--v-cyan|--v-amethyst/);
  });

  it('keeps the trace as well, because they answer different questions', () => {
    // The meter says how loud it is now; the trace says what the room has been
    // doing for twenty seconds, which is what shows a preacher stepping away
    // from a microphone. Replacing one with the other would lose a fact.
    expect(DOCK).toMatch(/<canvas class="wave"/);
  });

  it('the peak decays on the frame the trace already repaints on', () => {
    // NOT a timer of its own. `dockloop.test.js` exists because a second thing
    // that can be left running has happened on this card before.
    const loop = DOCK.slice(DOCK.indexOf('function frame('), DOCK.indexOf('function startLoop('));
    expect(loop).toMatch(/waveNow = Date\.now\(\)/);
    expect(DOCK, 'the meter started a timer of its own').not.toMatch(/setInterval\([^)]*waveNow/);
  });
});
