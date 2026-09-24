import { describe, it, expect } from 'vitest';
import { describeGate } from './gate.js';

// THE GATE READ-OUT — one place decides what a surface may say about the one
// control governing what the AI puts on a wall unasked.
//
// The shape of these cases is rule 35's: for every reading, ask what the surface
// says when the thing behind it is broken, absent, or has moved on its own. If
// that answer is the same as when everything is fine, it is not a reading.

const on = { available: true, sensitivityKnown: true, gateOnDial: true, sensitivity: 50,
  thresholds: { auto_fire: 0.5, suggest: 0.3 }, readiness: { auto_fire: 50, suggest: 70 } };

describe('describeGate', () => {
  it('reads the gate back as whole figures when the engine has answered', () => {
    const g = describeGate(on);
    expect(g.readable).toBe(true);
    expect(g.dial).toBe(50);
    // READINESS, not the threshold — see `figure` in gate.js and DECISIONS §117.
    expect(g.autoPct).toBe('50');
    expect(g.suggestPct).toBe('70');
    expect(g.drifted).toBe(false);
    expect(g.note).toBe('');
  });

  it('never claims a reading when no engine is attached', () => {
    const g = describeGate({ ...on, available: false });
    expect(g.readable).toBe(false);
    expect(g.autoPct).toBe(null);
    expect(g.suggestPct).toBe(null);
    expect(g.note).toMatch(/not attached/i);
  });

  // 50 is the shipped default AND an ordinary real setting, so the number alone
  // cannot separate "the gate is at 50" from "nobody has asked since launch".
  it('never claims a reading before the engine has answered once', () => {
    const g = describeGate({ ...on, sensitivityKnown: false });
    expect(g.readable).toBe(false);
    expect(g.note).toMatch(/has not answered/i);
  });

  // THE POINT OF THE WHOLE MODULE. `apply_profile` restores what a profile
  // LEARNED, and `record_feedback` moves the gate on every confirm and dismiss.
  // The dial still gets drawn at the nearest position to wherever the gate ended
  // up, and without this sentence that position reads as somebody's setting.
  it('says in words when the dial position no longer explains the gate', () => {
    const g = describeGate({ ...on, gateOnDial: false, sensitivity: 38,
      thresholds: { auto_fire: 0.596, suggest: 0.35 }, readiness: { auto_fire: 40, suggest: 65 } });
    expect(g.readable).toBe(true);
    expect(g.drifted).toBe(true);
    expect(g.autoPct).toBe('40');
    // It must name the position being shown, and say why it is being shown.
    expect(g.note).toContain('38');
    expect(g.note).toMatch(/nearest/i);
    expect(g.note).toMatch(/learn|profile|room/i);
  });

  // Drift is a claim about a gate that was read. An unread gate cannot have
  // drifted, and saying so would be inventing the worse of the two facts.
  it('cannot claim drift over a gate nobody has read', () => {
    for (const s of [{ available: false }, { sensitivityKnown: false }]) {
      const g = describeGate({ ...on, gateOnDial: false, ...s });
      expect(g.drifted).toBe(false);
    }
  });

  // A missing payload must read as absent, never as `NaN%` — which is a figure
  // on screen that is nobody's setting.
  it('prints nothing rather than NaN for a gate it was handed no numbers for', () => {
    const g = describeGate({ ...on, thresholds: null, readiness: null });
    expect(g.autoPct).toBe(null);
    expect(g.suggestPct).toBe(null);
    expect(g.readable).toBe(false);
  });

  // Called with nothing at all — the state a component is in for one frame.
  it('survives being handed nothing', () => {
    const g = describeGate();
    expect(g.readable).toBe(false);
    expect(g.drifted).toBe(false);
    expect(typeof g.note).toBe('string');
  });
});

// ── THE FIGURES RUN THE SAME WAY AS THE DIAL THEY SIT UNDER ──────────────────
//
// The operator's instruction, 2026-09-23: *"when the sensor is on Auto fire above
// 100, then it auto fires not when on 0."* The pair printed under the sensitivity
// slider were the raw confidence bars, and those fall as the dial rises — the
// cautious end printed 90% and the eager end printed 30%. Under a control, a
// figure reads as a setting, and that one said Relay was keenest where it fires
// least.
//
// `readiness` is the gate expressed as how ready Relay is. It comes from Rust
// (`Thresholds::readiness`) on the same payload as everything else about the
// gate, so there is no second opinion here about one number.
describe('describeGate · the printed figures', () => {
  const readied = { ...on, thresholds: { auto_fire: 0.5, suggest: 0.3 },
    readiness: { auto_fire: 50, suggest: 70 } };

  it('prints readiness, which rises with the dial', () => {
    const cautious = describeGate({ ...readied, sensitivity: 0,
      thresholds: { auto_fire: 0.9, suggest: 0.7 }, readiness: { auto_fire: 10, suggest: 30 } });
    const eager = describeGate({ ...readied, sensitivity: 100,
      thresholds: { auto_fire: 0.3, suggest: 0.1 }, readiness: { auto_fire: 70, suggest: 90 } });
    expect(cautious.autoPct).toBe('10');
    expect(eager.autoPct).toBe('70');
    expect(Number(eager.autoPct)).toBeGreaterThan(Number(cautious.autoPct));
  });

  // A suggestion is the easier of the two bars, so on a readiness scale it is
  // always the larger figure — and it is exactly 20 larger, which is the band the
  // operator asked for. `from_sensitivity` is what makes that true; this only
  // asserts the surface does not reverse it on the way to the screen.
  it('shows the suggest figure twenty above the auto-fire one', () => {
    const g = describeGate(readied);
    expect(Number(g.suggestPct) - Number(g.autoPct)).toBe(20);
  });

  // The absence must stay an absence. A `0` here would read as "Relay will never
  // fire", which is a setting, over a payload that carried no figure at all.
  it('prints nothing rather than a zero when the engine sent no figures', () => {
    const g = describeGate({ ...on, readiness: undefined, thresholds: undefined });
    expect(g.readable).toBe(false);
    expect(g.autoPct).toBe(null);
    expect(g.suggestPct).toBe(null);
  });
});
