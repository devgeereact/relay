// ── RELAY HOLDING SOMETHING BACK MUST NOT LOOK LIKE RELAY GOING DEAF ────────
//
// The operator: *"I dont want suggestion to be changing when a bible verse is
// reading because it heard a phrase which is in another bible verse… verses needs
// to be guarded so when a preacher is reading a verse it stays within the
// verse/chapter until the preacher calls another verse…"*
//
// `detection::hold_for_the_passage` does that, and the passage-guard decision of
// 2026-09-25 records why. This
// file is the console's half, and it is rule 35 and nothing else: **a suggestion
// list that stopped churning and a detector that stopped detecting are the same
// picture from the operator's seat.** `detection://held` is the one door the
// backend says which of the two is happening, and if nothing on the frontend
// receives it the guard is invisible — which is the failure, not a cosmetic gap.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const listeners = new Map();
const invoke = vi.fn(() => Promise.resolve(null));
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: (name, fn) => {
    listeners.set(name, fn);
    return Promise.resolve(() => listeners.delete(name));
  },
}));

const { passageHold, HOLD_TTL_MS, startCapture, stopCapture } = await import('./stores/capture.js');

await startCapture(null);

const fire = (name, payload) => {
  const fn = listeners.get(name);
  if (!fn) throw new Error(`nothing is listening for ${name}`);
  fn({ payload });
};

/** The report the backend sends while a preacher reads Psalms 107 aloud. */
const holding = {
  passage: 'Psalms 107',
  reading: 'oh that men would praise the lord for his goodness',
  held: [
    {
      reference: 'Ephesians 5:20',
      method: 'quoted',
      matched_text: 'giving thanks always for all things',
      reason: 'outside_the_reading',
    },
  ],
};

describe('the console can tell a guarded reading from a dead detector', () => {
  beforeEach(() => {
    vi.useRealTimers();
    passageHold.set(null);
  });

  it('listens for the event at all — without this the guard is invisible', () => {
    expect(
      listeners.has('detection://held'),
      'nothing is listening for detection://held, so a held suggestion is indistinguishable from silence',
    ).toBe(true);
  });

  it('nothing is being held until Relay says so', () => {
    expect(get(passageHold)).toBe(null);
  });

  it('names the passage, the phrase that proves it, and every verse it held', () => {
    fire('detection://held', holding);
    const h = get(passageHold);
    expect(h.passage).toBe('Psalms 107');
    expect(h.reading).toBe('oh that men would praise the lord for his goodness');
    expect(h.held).toHaveLength(1);
    expect(h.held[0].reference).toBe('Ephesians 5:20');
    // WHICH RULE. "already on screen" needs nothing from the operator; "outside the
    // reading" might, and a count on its own cannot be acted on.
    expect(h.held[0].reason).toBe('outside_the_reading');
    // THE WORDS, not a number. Rule 18: a quotation's score is a run length on a
    // scale of its own and must never be rendered as a percentage.
    expect(h.held[0].matched_text).toBe('giving thanks always for all things');
  });

  it('the already-on-screen rule needs no passage and still reports', () => {
    fire('detection://held', {
      passage: null,
      reading: null,
      held: [
        {
          reference: 'Jeremiah 6:16',
          method: 'reading',
          matched_text: 'and ask for the old paths where is the good way',
          reason: 'already_on_screen',
        },
      ],
    });
    const h = get(passageHold);
    expect(h.passage).toBe(null);
    expect(h.held[0].reason).toBe('already_on_screen');
  });

  it('replaces rather than accumulates — a growing log of holds is a second churning list', () => {
    fire('detection://held', holding);
    fire('detection://held', { ...holding, held: [{ ...holding.held[0], reference: 'Romans 12:2' }] });
    const h = get(passageHold);
    expect(h.held).toHaveLength(1);
    expect(h.held[0].reference).toBe('Romans 12:2');
  });

  it('a hold that nothing refreshes expires, because a reading that ENDED emits nothing', () => {
    vi.useFakeTimers();
    fire('detection://held', holding);
    expect(get(passageHold)).not.toBe(null);
    vi.advanceTimersByTime(HOLD_TTL_MS - 1);
    expect(get(passageHold), 'a hold must not vanish while the reading is still going').not.toBe(null);
    vi.advanceTimersByTime(2);
    expect(
      get(passageHold),
      'a line reading "holding 3" over a preacher who stopped reading a minute ago is the stale-status failure in miniature',
    ).toBe(null);
  });

  it('stopping capture takes the line down — there is no window, so nothing is held', async () => {
    fire('detection://held', holding);
    expect(get(passageHold)).not.toBe(null);
    await stopCapture();
    expect(
      get(passageHold),
      'Relay cannot be guarding a reading over a microphone that is not listening',
    ).toBe(null);
  });
});
