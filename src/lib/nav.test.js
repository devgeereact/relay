// The transport must never silently do nothing.
//
// `nav` was a `()` Tauri command wrapping a `()` function with THREE silent
// bail-outs inside it: a poisoned lock, stepping off the end of the passage, and
// `fire_manual`'s return `bool` being discarded outright. The frontend wrapper then
// swallowed any error into a `catch {}`.
//
// So: the operator presses → mid-sermon, the wall does not change, and there is no
// error, no toast, no log. On the key they press more than any other. It is the same
// silent-no-op class as the "Screens cleared" lie (docs/DECISIONS.md §20), and it
// survived that fix.
//
// The subtlety, and the reason a bool would have been the wrong repair: these
// outcomes are NOT all failures. Reaching the end of a passage is a correct,
// expected boundary — the operator simply needs to know that is why nothing moved.
// A verse missing from the corpus is a real fault. Flattening them is what hid this.
import { describe, it, expect, beforeEach, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { navVerse, navNotice } = await import('./stores/capture.js');

describe('nav reports what it actually did', () => {
  // Block body, NOT `beforeEach(() => invoke.mockReset())`. mockReset() returns the
  // mock, a concise arrow returns it implicitly, and vitest treats a value returned
  // from beforeEach as a TEARDOWN function — so it calls invoke() after every test,
  // with whatever rejecting implementation is still installed, and the resulting
  // unhandled rejection is reported as a failure of the test that just passed.
  beforeEach(() => {
    invoke.mockReset();
  });

  it('passes the direction through and returns the result', async () => {
    invoke.mockResolvedValue({ kind: 'fired', reference: 'John 3:17' });
    const r = await navVerse('next');
    expect(invoke).toHaveBeenCalledWith('nav', { direction: 'next' });
    expect(r.kind).toBe('fired');
  });

  // It no longer swallows. A backend failure must reach the operator, because the
  // screen did not change and they are entitled to know that was not their fault.
  it('THROWS on a backend failure instead of swallowing it', async () => {
    // mockImplementation, not mockRejectedValue: the latter builds the rejected
    // promise eagerly at the point it is configured, so it is already "unhandled"
    // before the code under test ever calls it, and vitest fails the test on that
    // rather than on anything navVerse did.
    invoke.mockImplementation(() =>
      Promise.reject(new Error('Relay lost track of the passage it was reading.')),
    );
    await expect(navVerse('next')).rejects.toThrow(/lost track of the passage/);
  });
});

describe('every outcome gets the right sentence', () => {
  it('a successful step says nothing — the wall IS the feedback', () => {
    // A toast here would be noise on the most-pressed key in the service.
    expect(navNotice({ kind: 'fired', reference: 'John 3:17' })).toBe(null);
  });

  it('the end of a passage is explained, not treated as an error', () => {
    const msg = navNotice({ kind: 'end_of_passage' });
    expect(msg).toMatch(/end of the passage/i);
    expect(msg).not.toMatch(/error|failed/i);
  });

  it('stepping with nothing on screen tells the operator what to do instead', () => {
    // The old behaviour: press →, nothing happens, no explanation. This is the case
    // a brand-new operator hits first.
    expect(navNotice({ kind: 'no_passage' })).toMatch(/fire a verse first/i);
  });

  it('a verse outside the corpus says the screen was LEFT ALONE', () => {
    // Firing it would blank the wall mid-service (Fire::may_broadcast), so we don't.
    // The operator must know the old verse is still up, not that nothing happened.
    const msg = navNotice({ kind: 'not_in_library', reference: 'Psalms 23:99' });
    expect(msg).toContain('Psalms 23:99');
    expect(msg).toMatch(/left as it is/i);
  });

  it('the four outcomes are genuinely distinguishable', () => {
    const seen = new Set(
      [
        { kind: 'fired', reference: 'x' },
        { kind: 'end_of_passage' },
        { kind: 'no_passage' },
        { kind: 'not_in_library', reference: 'x' },
      ].map(navNotice),
    );
    expect(seen.size).toBe(4); // incl. the null for `fired`
  });

  it('an unknown outcome degrades to silence, never to a crash', () => {
    // A detection card that throws takes the console down mid-service.
    expect(() => navNotice(undefined)).not.toThrow();
    expect(navNotice({ kind: 'something_new' })).toBe(null);
  });
});
