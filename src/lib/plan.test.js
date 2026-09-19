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
  chipOf,
  cueSub,
  sectionsOf,
  planRuntime,
  fmtDuration,
  parseDuration,
  planDateLabel,
  cueCountLabel,
  dropIndex,
  reorderTo,
  previewState,
  planChannelsOf,
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

  it('opens a section where the section CHANGES, not on every titled cue', () => {
    // The defect, rendered: a plan whose every cue records the section it is IN
    // — which is how a plan looks after an import, after a duplicate, and after
    // an operator types the same heading into two consecutive cues — became one
    // group per cue, and the running order drew EIGHT headings over eight cues
    // for a service with four sections. Fails against
    // `if (title || out.length === 0)`.
    const every = [
      cue(1, 'Gathering'),
      cue(2, 'Gathering'),
      cue(3, 'Word'),
      cue(4, 'Word'),
      cue(5, 'Sending'),
    ];
    const out = sectionsOf(every);
    expect(out.map((s) => s.title)).toEqual(['Gathering', 'Word', 'Sending']);
    expect(out.map((s) => s.items.length)).toEqual([2, 2, 1]);
  });

  it('reads a plan that only titles the FIRST cue of each section the same way', () => {
    // The other convention, the one `db/plans.rs` documents. Both have to land on
    // the same groups or the Planner draws a different plan depending on which
    // path wrote it.
    const first = [cue(1, 'Gathering'), cue(2, ''), cue(3, 'Word'), cue(4, ''), cue(5, 'Sending')];
    const out = sectionsOf(first);
    expect(out.map((s) => s.title)).toEqual(['Gathering', 'Word', 'Sending']);
    expect(out.map((s) => s.items.length)).toEqual([2, 2, 1]);
  });

  it('does not re-open a section across an untitled cue inside it', () => {
    // An empty title means "still in the section above", so a titled cue after
    // one of them is a continuation, not a second heading of the same name.
    // Comparing against the previous ROW rather than the open GROUP gets this
    // wrong — which is what the prototype's `c.sec !== lastSec` does.
    const out = sectionsOf([cue(1, 'Gathering'), cue(2, ''), cue(3, 'Gathering')]);
    expect(out.map((s) => s.title)).toEqual(['Gathering']);
    expect(out[0].items.length).toBe(3);
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

describe('chipOf — the kind, in a word, in the running order', () => {
  it('names every kind this build has', () => {
    expect(chipOf('scripture')).toBe('WORD');
    expect(chipOf('song')).toBe('SONG');
    expect(chipOf('announce')).toBe('NOTE');
    expect(chipOf('media')).toBe('MEDIA');
    expect(chipOf('countdown')).toBe('TIMER');
  });

  it('never truncates a kind it does not know', () => {
    // The prototype fell back to `kind.slice(0,4)` and the first kind added after
    // that read "LOWE" in every running order. A truncation is a name nobody
    // chose; quoting the row is not a guess.
    expect(chipOf('lower_third')).toBe('LOWER_THIRD');
    expect(chipOf('announcement')).toBe('ANNOUNCEMENT');
    for (const kind of ['lower_third', 'announcement', 'sermon']) {
      expect(chipOf(kind)).not.toBe(kind.slice(0, 4).toUpperCase());
    }
  });

  it('says UNKNOWN only when there is nothing to quote', () => {
    for (const empty of ['', '   ', null, undefined, 7, {}]) {
      expect(chipOf(empty)).toBe('UNKNOWN');
    }
  });
});

describe('planDateLabel — an absence, in words', () => {
  it('keeps a real date exactly as the backend sent it', () => {
    expect(planDateLabel('2026-09-14')).toBe('2026-09-14');
  });

  it('never renders the word undefined, and never an em dash', () => {
    // `{p.plan_date}` printed the literal `undefined` in the plan rail against a
    // summary that did not carry the field. An em dash would be no better: this
    // repository already spends it on "untimed cue" (`fmtDuration`), so a
    // dateless plan would read as a cue length.
    for (const absent of [undefined, null, '', '   ', 42, {}]) {
      expect(planDateLabel(absent)).toBe('No date');
    }
    expect(planDateLabel(undefined)).not.toContain('undefined');
    expect(planDateLabel(undefined)).not.toBe(fmtDuration(0));
  });
});

describe('cueCountLabel — a count, or an admission', () => {
  it('counts, and gets the plural right', () => {
    expect(cueCountLabel(0)).toBe('0 cues');
    expect(cueCountLabel(1)).toBe('1 cue');
    expect(cueCountLabel(8)).toBe('8 cues');
  });

  it('never renders "undefined cues"', () => {
    // The defect verbatim: `{p.cue_count} cue{s}` over a summary whose shape and
    // the frontend's had come apart.
    for (const absent of [undefined, null, NaN, '8', {}]) {
      expect(cueCountLabel(absent)).toBe('Cue count unknown');
      expect(cueCountLabel(absent)).not.toContain('undefined');
    }
  });

  it('says it does not know rather than saying zero', () => {
    // A zero is a claim about a plan that may be full. The two must not be the
    // same sentence — rule 35's family.
    expect(cueCountLabel(undefined)).not.toBe(cueCountLabel(0));
  });
});

describe('dropIndex — where a dragged cue lands', () => {
  const H = 34;

  it('lands on the row the drag actually covered', () => {
    expect(dropIndex(0, 0, H, 5)).toBe(0);
    expect(dropIndex(0, H, H, 5)).toBe(1);
    expect(dropIndex(3, -2 * H, H, 5)).toBe(1);
  });

  it('stops hard at both ends — a cue cannot be dragged out of the plan', () => {
    expect(dropIndex(0, -900, H, 5)).toBe(0);
    expect(dropIndex(4, 900, H, 5)).toBe(4);
  });

  it('moves nothing when the rows have no measurable height', () => {
    // `offsetHeight` is 0 in an unlaid-out list (and always in jsdom). Dividing
    // by it yields Infinity and then NaN, and `Math.max(0, Math.min(n, NaN))` is
    // NaN — an index that would splice the cue away entirely.
    expect(dropIndex(2, 120, 0, 5)).toBe(2);
    expect(Number.isNaN(dropIndex(2, 120, 0, 5))).toBe(false);
  });

  it('moves nothing in an empty plan', () => {
    expect(dropIndex(0, 120, H, 0)).toBe(0);
  });
});

describe('reorderTo', () => {
  it('moves an item without mutating the list it was given', () => {
    const items = [{ id: 1 }, { id: 2 }, { id: 3 }];
    const out = reorderTo(items, 0, 2);
    expect(out.map((i) => i.id)).toEqual([2, 3, 1]);
    expect(items.map((i) => i.id)).toEqual([1, 2, 3]);
  });

  it('is a copy, not a no-op, when the move goes nowhere', () => {
    const items = [{ id: 1 }, { id: 2 }];
    for (const [from, to] of [[0, 0], [-1, 1], [0, 9]]) {
      expect(reorderTo(items, from, to).map((i) => i.id)).toEqual([1, 2]);
    }
  });
});

describe('previewState — rule 35, on the cue inspector', () => {
  // ONE sentence, "No text to preview", stood for four different situations, and
  // three of them are not the same news. The whole value of this function is that
  // the four answers differ, so that is what is asserted: not the wording, but
  // that a cue which would put NOTHING in front of a congregation cannot be
  // mistaken for one behaving correctly.
  const c = (cue_type) => ({ cue_type });

  it('a cue with words renders, over the plate', () => {
    expect(previewState(c('scripture'), true)).toMatchObject({ state: 'render', plate: true });
    expect(previewState(c('media'), true)).toMatchObject({ state: 'render', plate: true });
  });

  it('media and countdown draw their own content, and get no plate', () => {
    for (const kind of ['media', 'countdown']) {
      const v = previewState(c(kind), false);
      expect(v.state, kind).toBe('self');
      expect(v.plate, kind).toBe(false);
      expect(v.message, kind).toBeTruthy();
    }
  });

  it('a scripture, song or notice cue with no words is a DEFECT, not a kind', () => {
    for (const kind of ['scripture', 'song', 'announce']) {
      const v = previewState(c(kind), false);
      expect(v.state, kind).toBe('empty');
      expect(v.message, kind).toMatch(/no words saved/);
      expect(v.message, kind).toMatch(/nothing on the screen/);
    }
  });

  it('an unrecognised cue_type claims nothing about what it renders', () => {
    // Same discipline as `typeOf` answering UNKNOWN rather than falling back to
    // scripture: this build cannot say, so it says it cannot say.
    const v = previewState(c('lower_third'), false);
    expect(v.state).toBe('unknown');
    expect(v.message).toMatch(/does not recognise/);
    expect(v.message).not.toMatch(/no words saved/);
  });

  it('the four verdicts are four different sentences', () => {
    // The defect was that they were one. A future edit that collapses two of them
    // back together fails here even if every branch above still returns its own
    // `state`.
    const said = [
      previewState(c('media'), false).message,
      previewState(c('countdown'), false).message,
      previewState(c('scripture'), false).message,
      previewState(c('lower_third'), false).message,
    ];
    expect(new Set(said).size).toBe(4);
  });

  it('no cue at all is not an empty cue', () => {
    expect(previewState(null, false)).toMatchObject({ state: 'none', plate: false, message: '' });
  });
});

// ── WHICH SCREENS A CUE IS FOR (RG-161) ────────────────────────────────────
//
// `plan_items.channels_json` is read here and nowhere else. The three readings
// below are not interchangeable and the difference is a screen going blank:
//
//   null  → EVERY screen. What every cue written before targeting existed
//           carries, so an old plan behaves exactly as it always did.
//   []    → NO screen. A real thing to ask for, and the one that would be lost
//           if it were folded in with null.
//   junk  → EVERY screen. Content that silently reaches nothing is worse than
//           content that reaches more than it had to.
describe('planChannelsOf', () => {
  it('reads a list of screens', () => {
    expect(planChannelsOf('[1,4]')).toEqual([1, 4]);
  });

  it('answers null for a cue that says nothing, which is every screen', () => {
    expect(planChannelsOf(null)).toBeNull();
    expect(planChannelsOf(undefined)).toBeNull();
  });

  it('keeps an empty list empty, because that is a different instruction', () => {
    expect(planChannelsOf('[]')).toEqual([]);
  });

  it('falls back to every screen on anything it cannot read', () => {
    expect(planChannelsOf('not json')).toBeNull();
    expect(planChannelsOf('{"a":1}')).toBeNull();
    expect(planChannelsOf('7')).toBeNull();
  });

  it('drops entries that are not numbers rather than passing them on', () => {
    expect(planChannelsOf('[1,"two",null,3]')).toEqual([1, 3]);
  });
});
