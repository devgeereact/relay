// The operator must be able to see WHAT KIND of claim the AI is making.
//
// Relay's whole safety architecture is one distinction: a reference the parser
// actually HEARD may auto-fire; a paraphrase GUESS may never auto-fire, at any
// score, at any sensitivity (router.rs::decide, docs/DECISIONS.md). The gate is
// airtight in Rust and property-tested there.
//
// And the console threw the distinction away. `method` was in the IPC payload the
// whole time; Live.svelte rendered both kinds as "AI suggestion — 92% match". The
// human in the loop was shown nothing to be a human in the loop WITH.
import { describe, it, expect } from 'vitest';
import { heard, methodKey, showsConfidence } from './detect.js';

const direct = { method: 'direct', confidence: 0.92 };
const semantic = { method: 'semantic', confidence: 0.61 };
const ambiguous = { method: 'ambiguous', confidence: 0.7 };

describe('heard vs guessed', () => {
  it('a spoken reference is HEARD', () => {
    expect(heard(direct)).toBe(true);
  });

  it('a paraphrase is NOT heard, however high its score', () => {
    expect(heard({ method: 'semantic', confidence: 0.99 })).toBe(false);
  });

  it('an ambiguous reference is not heard either — its confidence is a placeholder', () => {
    // detection.rs hardcodes 0.70 for these. It is not a measurement of anything.
    expect(heard(ambiguous)).toBe(false);
  });

  it('the three methods get three DIFFERENT keys — the operator must be able to tell them apart', () => {
    const keys = [direct, semantic, ambiguous].map(methodKey);
    expect(new Set(keys).size).toBe(3);
    expect(methodKey(semantic)).toBe('live.paraphrase_a_guess');
    expect(methodKey(direct)).toBe('live.heard_the_reference');
  });
});

describe('confidence is only shown where it means something', () => {
  it('a heard reference shows its confidence', () => {
    expect(showsConfidence(direct)).toBe(true);
  });

  // THE POINT OF THIS FILE.
  //
  // A semantic score is a TF-IDF cosine — a distance in an arbitrary vector space.
  // Rendering "61%" next to it invites the operator to read it as "61% likely to be
  // right", which is precisely what it is not. A number that lies is worse than no
  // number at all, because it looks like information and gets acted on.
  it('a paraphrase NEVER shows a percentage — at any score', () => {
    for (const c of [0.1, 0.35, 0.61, 0.95, 0.99]) {
      expect(showsConfidence({ method: 'semantic', confidence: c })).toBe(false);
    }
  });

  it('an ambiguous match never shows one either', () => {
    expect(showsConfidence(ambiguous)).toBe(false);
  });
});

describe('nothing crashes on a malformed payload', () => {
  // A detection card that throws takes the console down MID-SERVICE. Degrade to the
  // cautious reading instead — but note that an unknown method reads as "heard",
  // matching the Rust default, so a future method must be added in BOTH places.
  it('survives undefined', () => {
    expect(() => methodKey(undefined)).not.toThrow();
    expect(heard(undefined)).toBe(false);
  });
});
