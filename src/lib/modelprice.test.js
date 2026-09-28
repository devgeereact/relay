// ── RG-116 (P0) · THE MODEL IS AN ACCURACY SETTING WEARING A SPEED SETTING'S
//    CLOTHES, AND THE PANEL WHERE IT IS CHOSEN SAID NEITHER ──────────────────
//
// FIELD 2026-09-06, one morning, one machine: `ggml-large-v3-turbo` for the first
// service — 3 of 3 auto-fires correct, no wrong verse — then the model was switched
// mid-service to `ggml-base`, and the second service went 5 of 9. Four wrong verses,
// a 44% wrong-verse rate against the 5% this product sets as its bar. The switch was
// made on Relay's OWN printed advice, which named three millisecond figures and said
// nothing at all about accuracy.
//
// And the sensitivity dial is measurably not the lever: at its most cautious setting
// a wrong verse still sat at 0.95 while a correct one was discarded. So the choice of
// model is the lever, it is made on one panel, and that panel's summary line read
// "A bigger model hears more accurately but needs a faster computer" — two claims,
// neither measured, on the screen that decides how well Relay hears a preacher.
//
// These tests hold the two things that ARE measured, and — more importantly — the
// three models where the honest answer is that nothing is.
import { describe, it, expect } from 'vitest';
import { updatesPerSecond, describeSpeed, describeAccuracy } from './modelprice.js';

// The catalogue as `models.rs` serialises it. Figures from `docs/qa/audits/PERF.md`
// §1 and §2, which the Rust tests hold against `audio::HOP_MS` itself.
const BASE = {
  id: 'base',
  recommended: true,
  cadence_ms: 200,
  decode_ms: 59,
  measured_on: 'an Apple Silicon Mac with graphics acceleration',
  accuracy: 'it heard 5 of 9 spoken references correctly — four wrong verses',
};
const SMALL = { id: 'small', cadence_ms: 200, decode_ms: 152, measured_on: 'a Mac', accuracy: null };
const TURBO = { id: 'large-v3-turbo', cadence_ms: 800, decode_ms: 597, measured_on: 'a Mac', accuracy: '6 of 8' };
const Q5 = { id: 'large-v3-turbo-q5_0', cadence_ms: null, decode_ms: null, measured_on: null, accuracy: null };
const ALL = [BASE, SMALL, TURBO, Q5];

describe('the update rate — the half an operator feels', () => {
  it('turns a cadence into the number a volunteer can hold', () => {
    // 200 ms is one chunker hop; 800 ms is four. These are the only two answers the
    // shipped catalogue has, and they are the whole of rule 32's arithmetic.
    expect(updatesPerSecond(200)).toBe(5);
    expect(updatesPerSecond(800)).toBe(1.25);
  });

  it('never invents a rate for a model nobody has timed', () => {
    // Two of the five catalogue entries have never been run through the bench.
    expect(updatesPerSecond(null)).toBe(null);
    expect(updatesPerSecond(0)).toBe(null);
    expect(updatesPerSecond('fast')).toBe(null);
    expect(describeSpeed(Q5, ALL)).toBe(null);
  });

  it('names the machine, because the machine is part of the figure', () => {
    // PERF §5: the same `turbo` took ~1710 ms on CPU — slower than real time, unable
    // to keep up with a sermon at all. "1.25 updates a second" on a church laptop is
    // a sentence about somebody else's computer.
    expect(describeSpeed(TURBO, ALL)).toContain('a Mac');
  });

  it('SMALL IS FREE, and says so — the most useful sentence PERF has', () => {
    // 152 ms and 59 ms both round up to the same single 200 ms hop, so the larger
    // model costs nothing an operator can feel. This is the sentence the in-product
    // lag warning rests on, and until now it existed nowhere a church could read it.
    const said = describeSpeed(SMALL, ALL);
    expect(said).toContain('same rate as the recommended model');
    expect(said).toContain('costs no speed at all');
  });

  it('the largest model states the quarter of the cadence it takes', () => {
    // "A church that chose `turbo` chose a quarter of the cadence and nothing told
    // them" — CLAUDE.md rule 32, verbatim, and this is the line that tells them.
    const said = describeSpeed(TURBO, ALL);
    expect(said).toContain('1.25 transcript updates a second');
    expect(said).toContain('4× slower to update than the recommended model');
    // And the second cost, which is not obvious: corroboration is one cadence step
    // (rule 28), so a four-times cadence is a four-times wait before anything fires.
    expect(said).toContain('second pass');
  });

  it('the recommended model does not compare itself to itself', () => {
    const said = describeSpeed(BASE, ALL);
    expect(said).toContain('5 transcript updates a second');
    expect(said).not.toContain('recommended model');
  });
});

describe('wrong verses — the half nobody had measured', () => {
  it('a measured model reports what was actually counted', () => {
    const acc = describeAccuracy(BASE);
    expect(acc.measured).toBe(true);
    expect(acc.text).toContain('5 of 9');
  });

  it('AN UNMEASURED MODEL SAYS SO, in words, rather than showing nothing', () => {
    // This is the important one. A row with no line reads as "nothing to worry
    // about", which is exactly the reading the finding is about — and `small` is the
    // model Relay's own lag warning now points at, whose single accuracy attempt
    // returned 1 of 8 and was WITHDRAWN as invalid because it had measured whisper's
    // language election wandering rather than the model.
    for (const m of [SMALL, Q5]) {
      const acc = describeAccuracy(m);
      expect(acc.measured).toBe(false);
      expect(acc.text).toContain('Never measured');
      expect(acc.text).toContain('in any language');
    }
  });

  it('an empty string is an absence, not a measurement', () => {
    expect(describeAccuracy({ accuracy: '   ' }).measured).toBe(false);
    expect(describeAccuracy({}).measured).toBe(false);
    expect(describeAccuracy(null).measured).toBe(false);
  });
});
