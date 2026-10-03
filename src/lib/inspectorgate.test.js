// THE INSPECTOR'S GATE READ-OUT, THROUGH THE ONE DESCRIBER.
//
// `DetectionInspector` is the panel an operator opens to ask why a verse did or
// did not fire, and it printed the two thresholds straight off the store with a
// `?? 0` fallback. That reads identically in three situations which are not the
// same fact:
//
//   * the engine answered and the gate is where the dial put it;
//   * the engine answered and Relay has MOVED the gate since, from what has been
//     confirmed and dismissed during services or from a profile or room being
//     applied;
//   * the engine has not answered at all, where `?? 0` painted a confident `0%`
//     over a question nobody had asked.
//
// Rule 35, on the one panel whose entire job is explaining the gate's decision.
import { describe, it, expect } from 'vitest';
import { describeGate } from './gate.js';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const src = readFileSync(resolve(__dirname, 'DetectionInspector.svelte'), 'utf8');

describe('the detection inspector reads the gate through describeGate', () => {
  it('does not reach into the thresholds itself', () => {
    // The defect, as a shape: a second reader of the same numbers is a second
    // chance to describe them differently from Settings.
    expect(src, 'the inspector prints a threshold straight off the store').not.toMatch(
      /\$capture\.thresholds\?\.\w+\s*\?\?\s*0/,
    );
    expect(src).toMatch(/describeGate\(\$capture\)/);
  });

  it('says so when the engine has not answered, rather than printing 0%', () => {
    const g = describeGate({ available: true, sensitivityKnown: false });
    expect(g.readable).toBe(false);
    expect(g.autoPct).toBeNull();
    expect(g.note).toMatch(/has not answered/);
  });

  it('says so when Relay has moved the gate since the dial was set', () => {
    const g = describeGate({
      available: true,
      sensitivityKnown: true,
      sensitivity: 40,
      gateOnDial: false,
      thresholds: { auto_fire: 0.83, suggest: 0.7 },
      readiness: { auto_fire: 17, suggest: 30 },
    });
    expect(g.readable).toBe(true);
    expect(g.drifted).toBe(true);
    // It must name the dial position as a consequence, not as somebody's choice.
    expect(g.note).toMatch(/not because anybody set it there/);
  });

  it('is silent when the gate is exactly where the dial put it', () => {
    const g = describeGate({
      available: true,
      sensitivityKnown: true,
      sensitivity: 50,
      gateOnDial: true,
      thresholds: { auto_fire: 0.5, suggest: 0.3 },
      readiness: { auto_fire: 50, suggest: 70 },
    });
    expect(g.readable).toBe(true);
    expect(g.drifted).toBe(false);
    expect(g.note).toBe('');
  });

  it('renders the caveat in both unreadable and drifted cases', () => {
    // Two branches, one class. A drift note that existed only on the readable
    // branch would leave the `0%` case silent, which is the worse of the two.
    expect(src).toMatch(/\{#if !gate\.readable \|\| gate\.drifted\}[\s\S]{0,120}ins-gatenote/);
    // …and the paraphrase rule is NOT behind the readability branch. It is rule
    // 10's invariant, true whether or not a socket answered, and hiding it was a
    // regression the existing `inspector.test.js` caught.
    const dl = src.slice(src.indexOf('<dl class="ins-dl">'), src.indexOf('</dl>'));
    expect(dl).toMatch(/\{#if gate\.readable\}/);
    expect(dl.slice(dl.indexOf('{/if}'))).toMatch(/Paraphrases/);
  });
});
