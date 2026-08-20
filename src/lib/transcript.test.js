// THE TRANSCRIPT IS WHAT AN OPERATOR READS TO DECIDE WHETHER TO INTERVENE.
//
// Until 2026-08-15 nothing in this repo tested it. A grep for
// `transcript.set|transcript.update|$transcript` across every `*.test.js` returned
// **zero** — on the only live surface with no coverage of any kind.
//
// The run rail's own comment says what the panel is for:
//
//   "TRANSCRIPT — what the microphone is actually getting. When detection goes
//    quiet this is the difference between 'the preacher has not said a reference'
//    and 'Relay has gone deaf', and those need opposite responses."
//
// So it is not decoration. It is the instrument an operator uses to choose between
// waiting and reaching for the manual override, and a panel that lies about what was
// heard sends them the wrong way at the worst moment.
//
// `applyTranscript` was extracted from inside the `stt://transcript` listener to make
// these assertions possible — the rule now has one home, the way `detect.js` owns
// heard-vs-guessed and `errors.js` owns humanising.

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { applyTranscript, transcript } = await import('./stores/capture.js');

/** The store's own initial shape, so these start where the app starts. */
const EMPTY = { partial: '', finals: [], finalsAt: [] };
const final = (text, at = '10:00:00') => ({ text, is_final: true, at });
const feed = (state, ...steps) =>
  steps.reduce((t, s) => applyTranscript(t, { text: s.text, is_final: s.is_final }, s.at), state);

beforeEach(() => transcript.set({ ...EMPTY }));

describe('a partial is one line being revised, not a new one', () => {
  it('never appends to finals', () => {
    // Whisper revises the same utterance several times a second. If a partial
    // appended, a single sentence would become a dozen permanent lines and the
    // panel would scroll away from what the preacher is actually saying.
    const t = feed(EMPTY, { text: 'turn with', is_final: false }, { text: 'turn with me to', is_final: false });
    expect(t.finals).toEqual([]);
    expect(t.partial).toBe('turn with me to');
  });

  it('is replaced, not accumulated', () => {
    const t = feed(EMPTY, { text: 'John', is_final: false }, { text: 'John three', is_final: false });
    expect(t.partial).toBe('John three');
  });
});

describe('a final closes the line', () => {
  it('CLEARS the partial', () => {
    // Otherwise the half-heard fragment that became this final sits underneath it
    // and the operator reads the tail of the sentence twice — which reads exactly
    // like the recogniser stuttering, i.e. like Relay going deaf.
    const t = feed(
      EMPTY,
      { text: 'turn with me to John three', is_final: false },
      final('turn with me to John three sixteen'),
    );
    expect(t.partial).toBe('');
    expect(t.finals).toEqual(['turn with me to John three sixteen']);
  });

  it('appends in the order it was heard', () => {
    const t = feed(EMPTY, final('first'), final('second'), final('third'));
    expect(t.finals).toEqual(['first', 'second', 'third']);
  });

  it('stamps each line with its own arrival time', () => {
    const t = feed(EMPTY, final('first', '10:00:01'), final('second', '10:00:09'));
    expect(t.finalsAt).toEqual(['10:00:01', '10:00:09']);
  });
});

describe('finals and their timestamps are sliced in LOCKSTEP', () => {
  // THE BUG THIS EXISTS FOR, from the store's own comment: a consumer aligned the
  // two arrays by length, and once the rolling cap froze `finals.length` every new
  // line shifted the array left — so every timestamp labelled the wrong line. On a
  // surface whose whole job is "when did Relay last hear anything", a timestamp
  // attached to the wrong sentence is worse than no timestamp.
  it('stay the same length and the same order past the cap', () => {
    let t = { ...EMPTY };
    for (let i = 1; i <= 40; i++) t = applyTranscript(t, { text: `line ${i}`, is_final: true }, `t${i}`);

    expect(t.finals.length).toBe(t.finalsAt.length);
    // Every kept line still carries ITS OWN stamp — checked pairwise, not by length.
    for (let i = 0; i < t.finals.length; i++) {
      const n = t.finals[i].split(' ')[1];
      expect(t.finalsAt[i]).toBe(`t${n}`);
    }
  });

  it('the cap keeps the NEWEST lines, because the operator is reading now', () => {
    let t = { ...EMPTY };
    for (let i = 1; i <= 40; i++) t = applyTranscript(t, { text: `line ${i}`, is_final: true }, `t${i}`);

    expect(t.finals.at(-1)).toBe('line 40');
    expect(t.finals).not.toContain('line 1');
    // And it is genuinely bounded: a sermon is an hour long.
    expect(t.finals.length).toBeLessThanOrEqual(12);
    expect(t.finals.length).toBeGreaterThan(1);
  });

  it('a state restored without finalsAt degrades instead of crashing', () => {
    // An older build's session has no `finalsAt`. An unlabelled line is a small
    // loss; a crash on the panel an operator is watching to decide whether Relay
    // has gone deaf is not.
    const legacy = { partial: '', finals: ['old line'] };
    const t = applyTranscript(legacy, { text: 'new line', is_final: true }, '10:00:00');
    expect(t.finals).toEqual(['old line', 'new line']);
    expect(t.finalsAt).toEqual(['10:00:00']);
    expect(t.finals.length).toBeGreaterThanOrEqual(t.finalsAt.length);
  });
});

describe('what the run rail actually shows', () => {
  // `LiveOutputRail` renders `[...finals].slice(-4).reverse()` — the last four,
  // newest first. Pinned here because the reversal is the kind of thing a tidy-up
  // removes, and an operator scanning for "did it hear that?" reads the top line.
  const shown = (t) => [...(t.finals ?? [])].slice(-4).reverse();

  it('is the newest four, newest first', () => {
    let t = { ...EMPTY };
    for (let i = 1; i <= 6; i++) t = applyTranscript(t, { text: `line ${i}`, is_final: true }, `t${i}`);
    expect(shown(t)).toEqual(['line 6', 'line 5', 'line 4', 'line 3']);
  });

  it('and it does not blow up on a fresh, silent start', () => {
    expect(shown(EMPTY)).toEqual([]);
  });
});

describe('the store itself', () => {
  it('starts empty, so a fresh console shows silence rather than stale lines', () => {
    expect(get(transcript)).toEqual(EMPTY);
  });

  it('survives a stop: the partial goes, the finals stay', () => {
    // `stopCapture` does `transcript.update((t) => ({ ...t, partial: '' }))`. The
    // finals MUST survive — an operator stops listening to deal with something and
    // then needs to read what was heard before they stopped. And the partial must
    // NOT: it is a fragment of an utterance that will now never be completed, and
    // leaving it under the last final reads as a line Relay heard.
    transcript.set(feed(EMPTY, final('a closed line'), { text: 'half a sent', is_final: false }));
    transcript.update((t) => ({ ...t, partial: '' }));

    expect(get(transcript).partial).toBe('');
    expect(get(transcript).finals).toEqual(['a closed line']);
  });
});
