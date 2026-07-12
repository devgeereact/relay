import { describe, it, expect } from 'vitest';
import { payloadOf, slidesOf, nextOf, stepFrom, cueSub } from './plan.js';

const song = (id, ...labels) => ({
  id,
  cue_type: 'song',
  label: 'Amazing Grace',
  payload_json: JSON.stringify({
    title: 'Amazing Grace',
    sections: labels.map((l, i) => ({ tag: l[0] + (i + 1), label: l, lyrics: `${l} lyrics` })),
  }),
});
const verse = (id, reference) => ({
  id,
  cue_type: 'scripture',
  label: reference,
  payload_json: JSON.stringify({ reference, text: 'For God so loved…', verse: 16 }),
});

describe('payloadOf', () => {
  it('never throws on a corrupt payload', () => {
    // A malformed row must not take the console down mid-service.
    expect(payloadOf({ payload_json: '{oh no' })).toEqual({});
    expect(payloadOf({ payload_json: 'null' })).toEqual({});
    expect(payloadOf({ payload_json: '[1,2]' })).toEqual({});
    expect(payloadOf(undefined)).toEqual({});
  });
});

describe('slidesOf', () => {
  it('reduces every cue type to the same slide shape', () => {
    expect(slidesOf(song(1, 'Verse', 'Chorus'))).toHaveLength(2);
    expect(slidesOf(verse(2, 'John 3:16'))).toHaveLength(1);
    expect(slidesOf({ id: 3, cue_type: 'countdown', label: 'c', payload_json: '{"minutes":7}' })[0].text).toBe('7:00');
    expect(slidesOf({ id: 4, cue_type: 'media', label: 'bg.jpg', payload_json: '{}' })).toHaveLength(1);
  });
});

describe('stepFrom — the transport', () => {
  const items = [song(1, 'Verse', 'Chorus'), verse(2, 'John 3:16'), song(3, 'Bridge')];

  it('advances within a cue, then rolls into the next', () => {
    expect(stepFrom(items, 1, 0, 1)).toMatchObject({ item: { id: 1 }, slide: 1 });
    expect(stepFrom(items, 1, 1, 1)).toMatchObject({ item: { id: 2 }, slide: 0 });
  });

  it('steps BACK onto the LAST slide of the previous cue, not its first', () => {
    // ← from John 3:16 must land on the song's Chorus (slide 1), not its Verse.
    // Landing on slide 0 would silently skip a whole verse of the song.
    expect(stepFrom(items, 2, 0, -1)).toMatchObject({ item: { id: 1 }, slide: 1 });
  });

  it('stops hard at both ends — never wraps', () => {
    // Wrapping from the last slide back to the top would put the opening
    // countdown on the screen at the end of the service.
    expect(stepFrom(items, 1, 0, -1)).toBeNull();
    expect(stepFrom(items, 3, 0, 1)).toBeNull();
  });

  it('starts the plan when nothing from it is live', () => {
    // After Esc, or after a detour to an AI-suggested verse, liveCue.cueId is
    // null. → must restart the plan, not do nothing.
    expect(stepFrom(items, null, 0, 1)).toMatchObject({ item: { id: 1 }, slide: 0 });
    expect(stepFrom(items, 999, 0, 1)).toMatchObject({ item: { id: 1 }, slide: 0 });
  });

  it('is a no-op on an empty plan', () => {
    expect(stepFrom([], null, 0, 1)).toBeNull();
  });
});

describe('nextOf — what the preacher is told is coming', () => {
  const items = [song(1, 'Verse', 'Chorus'), verse(2, 'John 3:16')];

  it('names the next slide inside the cue, then the next cue', () => {
    expect(nextOf(items, 1, 0).label).toBe('Amazing Grace · Chorus');
    expect(nextOf(items, 1, 1).label).toBe('John 3:16');
  });

  it('is null at the end of the plan', () => {
    expect(nextOf(items, 2, 0)).toBeNull();
  });

  it('agrees with the transport', () => {
    // These are two separate code paths and they MUST NOT disagree: the stage
    // monitor promising one thing and → doing another is worse than no monitor.
    for (const [cueId, slide] of [
      [1, 0],
      [1, 1],
    ]) {
      const step = stepFrom(items, cueId, slide, 1);
      expect(nextOf(items, cueId, slide).text).toBe(
        step.item.cue_type === 'song'
          ? slidesOf(step.item)[step.slide].text
          : slidesOf(step.item)[step.slide].text,
      );
    }
  });
});

describe('cueSub', () => {
  it('counts a song by its slides', () => {
    expect(cueSub(song(1, 'Verse', 'Chorus', 'Bridge'))).toBe('SONG · 3 SLIDES');
    expect(cueSub(verse(2, 'John 3:16'))).toBe('SCRIPTURE · AUTO-DETECT');
  });
});
