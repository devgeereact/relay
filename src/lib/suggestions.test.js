import { describe, it, expect } from 'vitest';
import { pruneStaleSuggestions, SUGGESTION_TTL_MS } from './stores/capture.js';

// A pending suggestion is a claim about what the preacher is saying RIGHT NOW.
// Once they have moved on, accepting it puts the wrong verse on the wall — so a
// stale card is not clutter, it is a trap sitting under the `A` key. One live
// service held six at once, all stale, while the one that mattered scrolled away.
describe('pending suggestions expire', () => {
  const at = (ms) => ({ reference: `R${ms}`, at: ms });

  it('keeps suggestions inside the window', () => {
    const now = 100_000;
    const list = [at(now), at(now - 1_000), at(now - SUGGESTION_TTL_MS + 1)];
    expect(pruneStaleSuggestions(list, now)).toHaveLength(3);
  });

  it('drops suggestions at or past the window', () => {
    const now = 100_000;
    const list = [at(now), at(now - SUGGESTION_TTL_MS), at(now - 120_000)];
    const left = pruneStaleSuggestions(list, now);
    expect(left).toHaveLength(1);
    expect(left[0].at).toBe(now);
  });

  it('outlives the router repeat cooldown, so the operator can actually read it', () => {
    // router.rs: DEFAULT_DEBOUNCE_MS = (WINDOW_SECS + 2) * 1000, WINDOW_SECS = 8.
    expect(SUGGESTION_TTL_MS).toBeGreaterThan((8 + 2) * 1000);
  });

  it('treats a suggestion with no timestamp as stale rather than immortal', () => {
    // Defensive: an `at`-less entry from an older payload shape must not pin the
    // queue open forever.
    expect(pruneStaleSuggestions([{ reference: 'X' }], 100_000)).toHaveLength(0);
  });
});
