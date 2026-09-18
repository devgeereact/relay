import { describe, it, expect } from 'vitest';
import { describeGate } from './gate.js';

// THE GATE READ-OUT — one place decides what a surface may say about the one
// control governing what the AI puts on a wall unasked.
//
// The shape of these cases is rule 35's: for every reading, ask what the surface
// says when the thing behind it is broken, absent, or has moved on its own. If
// that answer is the same as when everything is fine, it is not a reading.

const on = { available: true, sensitivityKnown: true, gateOnDial: true, sensitivity: 50,
  thresholds: { auto_fire: 0.5, suggest: 0.35 } };

describe('describeGate', () => {
  it('reads the gate back as whole percentages when the engine has answered', () => {
    const g = describeGate(on);
    expect(g.readable).toBe(true);
    expect(g.dial).toBe(50);
    expect(g.autoPct).toBe('50%');
    expect(g.suggestPct).toBe('35%');
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
      thresholds: { auto_fire: 0.596, suggest: 0.35 } });
    expect(g.readable).toBe(true);
    expect(g.drifted).toBe(true);
    expect(g.autoPct).toBe('60%');
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
    const g = describeGate({ ...on, thresholds: null });
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
