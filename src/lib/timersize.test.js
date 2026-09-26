// HOW BIG THE PREACHER'S CLOCK IS, CHOSEN BY THE OPERATOR (RG-240).
//
// The operator: *"I want to be able to make the stage timer Big on the
// screen"*, and in the same breath *"Allow only operators to select zones for
// preacher"*. The two are one change: the size travels the way the zones do, so
// a church can make the figure large on a platform monitor across a room
// without anybody touching the phone.
//
// **It rides in the zone blob rather than in a second column or a second
// frame.** `readStageZones` reads only the keys it knows and ignores the rest,
// so an extra key is already safe on the wire — and a layout saved before this
// existed simply has no opinion, which reads as `normal`.
import { describe, it, expect } from 'vitest';
import { TIMER_SIZES, readTimerSize, timerScale } from './stagelayout.js';
import { readStageZones, DEFAULT_STAGE_ZONES } from './stagelayout.js';

describe('readTimerSize — what the operator chose', () => {
  it('reads the three sizes', () => {
    for (const s of TIMER_SIZES) expect(readTimerSize({ timer_size: s.key })).toBe(s.key);
  });

  it('a layout with no opinion is normal, not absent', () => {
    // A size is always needed to render, so this answers a size — unlike
    // `readStageZones`, where "no opinion" and "shows nothing" are genuinely
    // different states and `null` separates them.
    expect(readTimerSize({})).toBe('normal');
    expect(readTimerSize(null)).toBe('normal');
    expect(readTimerSize({ reading: true })).toBe('normal');
  });

  it('and nonsense is normal rather than a broken screen', () => {
    for (const v of ['enormous', 42, true, null, '']) expect(readTimerSize({ timer_size: v })).toBe('normal');
  });

  it('every size has a scale, and they go up', () => {
    const scales = TIMER_SIZES.map((s) => timerScale(s.key));
    expect(scales).toEqual([...scales].sort((a, b) => a - b));
    expect(new Set(scales).size, 'two sizes render the same').toBe(scales.length);
    expect(timerScale('normal')).toBe(1);
  });
});

describe('and it does not disturb the zones it travels with', () => {
  it('a blob carrying a size still reads its zones', () => {
    const z = readStageZones({ reading: false, clock: true, timer_size: 'huge' });
    expect(z.reading).toBe(false);
    expect(z.clock).toBe(true);
  });

  it('a blob carrying ONLY a size names no zone, so it is not a layout', () => {
    // The distinction `readStageZones` exists to keep: "no layout" and "a layout
    // that shows nothing" are different, and a size is neither.
    expect(readStageZones({ timer_size: 'huge' })).toBeNull();
  });

  it('and the zone defaults are untouched by any of this', () => {
    expect(Object.keys(DEFAULT_STAGE_ZONES)).not.toContain('timer_size');
  });
});
