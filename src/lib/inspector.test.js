// THE INSPECTOR MAY NEVER PUT A NUMBER ON A GUESS.
//
// This is the same law `detect.test.js` pins for the Live panel, tested again at
// the one other surface that renders a claim. It is not duplication for its own
// sake: the Inspector is a SECOND renderer of the same fact, and the reference
// mockup it was built from (panel 8) draws a percentage on the claim chip and a
// bullet reading "Confidence computed from semantic similarity". Building to that
// reference faithfully would have shipped exactly the bug this forbids.
//
// A TF-IDF cosine is a distance in an arbitrary vector space. Printed as "61%" it
// reads as "61% likely to be right", which is not what it is — and a number that
// lies is worse than no number, because it looks like information and therefore
// gets acted on, in front of a congregation.
//
//   CLAUDE.md §18 · DECISIONS §21 · router.rs::semantic_can_never_auto_fire

import { describe, it, expect, afterEach } from 'vitest';
import DetectionInspector from './DetectionInspector.svelte';

let host;
let app;
function mount(props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new DetectionInspector({ target: host, props });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
});

const base = {
  reference: 'John 3:16',
  confidence: 0.61,
  text: 'For God so loved the world…',
  translation: 'KJV',
};

describe('the claim chip', () => {
  it('shows a percentage for a HEARD reference', () => {
    // A direct hit's number is a real parse confidence, and it is allowed.
    const el = mount({ detection: { ...base, method: 'direct', confidence: 0.92 } });
    expect(el.textContent).toMatch(/92%/);
  });

  it('shows NO percentage for a paraphrase, at any score', () => {
    const el = mount({ detection: { ...base, method: 'semantic', confidence: 0.61 } });
    expect(el.textContent).not.toMatch(/61%/);
    expect(el.textContent).not.toMatch(/\b61\b/);
  });

  it('shows no percentage for an ambiguous reference either', () => {
    // Its confidence is a hardcoded placeholder, not a measurement (detect.js).
    const el = mount({ detection: { ...base, method: 'ambiguous', confidence: 0.7 } });
    expect(el.textContent).not.toMatch(/70%/);
  });

  it('says WHY there is no number, rather than silently omitting it', () => {
    // An absent number is itself confusing unless it is explained — the operator
    // would otherwise assume the reading failed.
    const el = mount({ detection: { ...base, method: 'semantic' } });
    expect(el.textContent).toMatch(/no percentage here on purpose/i);
  });
});

describe('the evidence', () => {
  it('lists the actual matching terms for a paraphrase', () => {
    // main.rs joins `top_k_explained`'s terms with " · ". These are the real words
    // that drove the cosine — the only honest answer to "why this match?".
    const el = mount({
      detection: { ...base, method: 'semantic', matched_text: 'shepherd · want · pastures' },
    });
    expect(el.textContent).toMatch(/shepherd/);
    expect(el.textContent).toMatch(/pastures/);
  });

  it('never claims Relay compared grammar, word order or meaning', () => {
    // The reference mockup's bullets — "Order and structure align closely",
    // "Minimal words added or skipped" — describe an algorithm that does not
    // exist. Rendering them would be fabricated reasoning: checkable, and wrong.
    const el = mount({
      detection: { ...base, method: 'semantic', matched_text: 'shepherd · want' },
    });
    expect(el.textContent).not.toMatch(/order and structure/i);
    expect(el.textContent).not.toMatch(/words added or skipped/i);
    expect(el.textContent).not.toMatch(/semantic similarity/i);
    // ...and it says plainly what it does NOT do.
    expect(el.textContent).toMatch(/does not compare/i);
  });

  it('shows the transcript span a heard reference was read from', () => {
    const el = mount({
      detection: { ...base, method: 'direct', matched_text: 'john three sixteen' },
    });
    expect(el.textContent).toMatch(/john three sixteen/);
  });

  it('is suspicious, not reassuring, when there is no evidence at all', () => {
    // The failure mode to avoid: an empty evidence box reading as "nothing to
    // worry about" when it actually means "we cannot say why we suggested this".
    const el = mount({ detection: { ...base, method: 'semantic', matched_text: null } });
    expect(el.textContent).toMatch(/more suspicion/i);
  });
});

describe('the gate', () => {
  it('tells the operator that accepting or dismissing retunes it', () => {
    // `confirm_detection` and `dismiss_detection` both call `record_feedback`.
    // That has always been true and no screen ever said so — which makes it
    // invisible training: the operator changes the product without being told.
    const el = mount({ detection: { ...base, method: 'direct' } });
    expect(el.textContent).toMatch(/retunes the gate/i);
  });

  it('states that paraphrases can never auto-fire', () => {
    const el = mount({ detection: { ...base, method: 'semantic' } });
    expect(el.textContent).toMatch(/never auto-fire|never fire on its own/i);
  });
});

describe('nothing to inspect', () => {
  it('renders nothing at all when there is no detection', () => {
    const el = mount({ detection: null });
    expect(el.textContent.trim()).toBe('');
  });
});
