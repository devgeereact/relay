import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync } from 'node:fs';
import { resolve, join } from 'node:path';
import {
  TYPE,
  typeOf,
  payloadOf,
  slidesOf,
  nextOf,
  stepFrom,
  cueSub,
  sectionsOf,
  planRuntime,
  fmtDuration,
  parseDuration,
} from './plan.js';

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

// ── TYPE has ONE door, and it never answers "scripture" from an absence ──────
//
// The fix that added `TYPE.unknown` was applied at three call sites and missed
// the fourth. `cueSub` kept `|| TYPE.scripture`, and it is rendered on BOTH the
// Planner's cue inspector and the Live run surface — so a cue of a kind this
// build does not recognise was badged UNKNOWN / MANUAL with "SCRIPTURE ·
// AUTO-DETECT" printed two lines beneath it. Reproduced in a browser against the
// real component before this was written: the inspector read
// {type: 'UNKNOWN', trig: 'MANUAL', sub: 'SCRIPTURE · AUTO-DETECT'}.
//
// Scripture is the one kind the AI may fire by itself, so saying it about a row
// nobody can identify is a claim made from an absence. `cue_type` is plain TEXT
// with no CHECK constraint, and `docs/data/schema.sql` documents the notice type
// under a spelling the frontend has never used ('announcement' vs 'announce'),
// which is exactly how such a row arrives.
const foreign = (cue_type) => ({
  id: 9,
  cue_type,
  label: 'Legacy notice row',
  payload_json: JSON.stringify({ body: 'Imported from an older schema.' }),
});

describe('typeOf — the one door onto TYPE', () => {
  it('answers UNKNOWN for a cue_type this build does not recognise', () => {
    for (const t of ['announcement', 'sermon', '', null, undefined, 'SCRIPTURE']) {
      expect(typeOf(t)).toBe(TYPE.unknown);
    }
  });

  it('answers the real row for every kind this build does know', () => {
    for (const k of ['scripture', 'song', 'media', 'announce', 'countdown']) {
      expect(typeOf(k)).toBe(TYPE[k]);
    }
  });

  it('never lets an unrecognised cue claim the trigger only scripture has', () => {
    // The fourth door. Fails against `TYPE[item.cue_type] || TYPE.scripture`.
    expect(cueSub(foreign('announcement'))).toBe('UNKNOWN · MANUAL');
    expect(cueSub(foreign('announcement'))).not.toContain('AUTO-DETECT');
    expect(cueSub(foreign('announcement'))).not.toContain('SCRIPTURE');
  });

  it('has no fifth door — nothing in src/ falls back to TYPE.scripture', () => {
    // A scanner rather than a list, because the defect this replaces was a call
    // site nobody thought to enumerate. It covers Live.svelte too, which is not
    // this branch's file: the point is that reintroducing the fallback anywhere
    // fails here, wherever "anywhere" turns out to be next time.
    const root = resolve(__dirname, '..');
    const hits = [];
    const walk = (d) => {
      for (const e of readdirSync(d, { withFileTypes: true })) {
        const p = join(d, e.name);
        if (e.isDirectory()) walk(p);
        else if (/\.(js|svelte)$/.test(e.name) && !e.name.endsWith('.test.js')) {
          const src = readFileSync(p, 'utf8');
          // Strip comments: three files DESCRIBE this defect in prose, and a
          // scanner that greps a comment is how one entitlement test passed on
          // a broken file.
          const code = src
            .replace(/<!--[\s\S]*?-->/g, '')
            .replace(/\/\*[\s\S]*?\*\//g, '')
            .replace(/(^|[^:])\/\/[^\n]*/g, '$1');
          if (/(\|\||\?\?)\s*TYPE\.scripture\b/.test(code)) hits.push(p.slice(root.length + 1));
        }
      }
    };
    walk(root);
    expect(hits).toEqual([]);
  });
});

// A cue with a section_title begins a section; grouping is derived from the
// order, never stored, so it cannot disagree with what the transport walks.
const cue = (id, section_title = '', duration_sec = 0) => ({
  id,
  cue_type: 'song',
  label: `Cue ${id}`,
  payload_json: '{}',
  section_title,
  duration_sec,
});

describe('sectionsOf', () => {
  it('groups cues under the cue that starts each section', () => {
    const secs = sectionsOf([
      cue(1, 'Welcome & Worship'),
      cue(2),
      cue(3),
      cue(4, 'Sermon'),
      cue(5),
    ]);
    expect(secs.map((s) => s.title)).toEqual(['Welcome & Worship', 'Sermon']);
    expect(secs[0].items.map((i) => i.id)).toEqual([1, 2, 3]);
    expect(secs[1].items.map((i) => i.id)).toEqual([4, 5]);
  });

  it('keeps cues that precede the first heading', () => {
    // A plan need not open with a section. Dropping these would hide real cues
    // from the operator entirely.
    const secs = sectionsOf([cue(1), cue(2, 'Sermon')]);
    expect(secs[0].title).toBe('');
    expect(secs[0].items.map((i) => i.id)).toEqual([1]);
    expect(secs[1].title).toBe('Sermon');
  });

  it('sums only timed cues and flags a section that is not fully timed', () => {
    const secs = sectionsOf([cue(1, 'Worship', 240), cue(2, '', 120), cue(3)]);
    expect(secs[0].seconds).toBe(360);
    expect(secs[0].timed).toBe(false); // cue 3 is untimed
  });

  it('is empty for an empty plan', () => {
    expect(sectionsOf([])).toEqual([]);
    expect(sectionsOf(undefined)).toEqual([]);
  });
});

describe('planRuntime', () => {
  it('marks the total partial when any cue is untimed', () => {
    // Most real plans are partial — scripture fires when the preacher reaches
    // it. Presenting a partial sum as the service length is how a service runs
    // long, so the Planner has to be able to say "est.".
    expect(planRuntime([cue(1, '', 300), cue(2, '', 120)])).toEqual({
      seconds: 420,
      partial: false,
    });
    expect(planRuntime([cue(1, '', 300), cue(2)])).toEqual({ seconds: 300, partial: true });
    expect(planRuntime([])).toEqual({ seconds: 0, partial: false });
  });
});

describe('fmtDuration', () => {
  it('renders a cue as m:ss and a plan as Xh Ym', () => {
    expect(fmtDuration(120)).toBe('2:00');
    expect(fmtDuration(345)).toBe('5:45');
    expect(fmtDuration(5520, true)).toBe('1h 32m');
    expect(fmtDuration(1800, true)).toBe('30m');
  });

  it('renders an untimed cue as a dash, never 0:00', () => {
    // 0:00 reads as "this cue takes no time", which is a different claim from
    // "this cue is not on a clock".
    expect(fmtDuration(0)).toBe('—');
    expect(fmtDuration(null)).toBe('—');
    expect(fmtDuration(-5)).toBe('—');
  });
});
