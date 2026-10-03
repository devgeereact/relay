// THE TWO BLOCKS THAT CAME OUT OF `ServicePlanner.svelte`, AND THE PROOF THEY
// STILL ANSWER WHAT THE COMPONENT USED TO.
//
// `ServicePlanner.svelte` is the largest view in the product. Three requirements
// grow it at once, so the two blocks inside it that were already self-contained —
// the add panel's filter and its four payload builders, and the pointer drag's
// paint arithmetic — were lifted into `planneradd.js` and `plannerdrag.js` first.
//
// AN EXTRACTION THAT IS NOT TESTED IS A MOVE, NOT A REFACTOR. The value of taking
// these out is that they become checkable without a laid-out list and without a
// mouse, on a machine that cannot screenshot the app. So this file checks them,
// and the last describe block checks that the component actually CALLS them —
// otherwise the modules are a second copy of the rules rather than the only one,
// which is how two surfaces in this repository came to disagree about what a
// fresh install contains.
//
// ── HOW EACH WAS CHECKED ────────────────────────────────────────────────────
//
// Test the bug, not the fix. Each assertion below was written against a defect
// that the pre-extraction code could actually have:
//
//   · `filterMedia('')` returning `[]` — the add panel opens empty under a box
//     nobody has typed in. Watched to fail with `if (!n) return [];`.
//   · a payload builder dropping `media_id` — the cue previews as a filename and
//     `slidesOf` has nothing to look up. Watched to fail by removing the key.
//   · `rowsMoved` on a zero row height — `Infinity`, then `translateY(NaN px)`
//     on every neighbour. Watched to fail by removing the `h <= 0` guard.
//   · `rowOffset` shifting a row the drag has not passed — the gap opens in the
//     wrong place, so the operator sees an order they will not get. Watched to
//     fail by relaxing the `i <= from + shift` bound to `i > from`.
//   · `rowsMoved` disagreeing with `plan.js::dropIndex` — what the operator SEES
//     and what they GET come apart. That one is asserted against the real
//     `dropIndex` rather than a restatement of it.
//
//   npx vitest run src/lib/plannerblocks.test.js

import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import {
  RECENT_LIMIT,
  filterMedia,
  filterAnnouncements,
  verseCue,
  mediaCuePayload,
  announceCuePayload,
  countdownCuePayload,
} from './planneradd.js';
import { rowsMoved, rowOffset, dragFrame } from './plannerdrag.js';
import { dropIndex, slidesOf, previewState } from './plan.js';

const PLANNER = resolve(__dirname, './views/ServicePlanner.svelte');

// ─────────────────────────────────────────────────────────────────────────────
// 1 · THE ADD PANEL'S FILTER
// ─────────────────────────────────────────────────────────────────────────────
describe('the add panel searches what it already holds', () => {
  const library = Array.from({ length: 12 }, (_, i) => ({
    id: i + 1,
    filename: `shot-${i + 1}.jpg`,
    kind: 'image',
  }));

  it('an empty box is the RECENT list, not no results', () => {
    // The panel opens before anybody types. `[]` here renders an empty pane
    // under an untouched search box, which reads as a library with nothing in it.
    expect(filterMedia(library, '')).toHaveLength(RECENT_LIMIT);
    expect(filterMedia(library, '   ')).toHaveLength(RECENT_LIMIT);
    expect(filterMedia(library, '')[0]).toBe(library[0]);
  });

  it('a short library is not padded, and a missing one is not a crash', () => {
    expect(filterMedia(library.slice(0, 3), '')).toHaveLength(3);
    expect(filterMedia(null, '')).toEqual([]);
    expect(filterMedia(undefined, 'sunrise')).toEqual([]);
  });

  it('a query matches the filename, ignoring case', () => {
    expect(filterMedia(library, 'SHOT-11').map((m) => m.id)).toEqual([11]);
    // …and a query is NOT capped at the recent limit: a search that matched ten
    // files and showed eight would be hiding two with nothing saying so.
    expect(filterMedia(library, 'shot').length).toBe(12);
  });

  it('a filename that is missing does not throw the panel away', () => {
    // A hand-edited or older row. One bad row must not take the search down.
    expect(filterMedia([{ id: 1 }, library[0]], 'shot-1').map((m) => m.id)).toEqual([1]);
  });

  it('an announcement matches on its BODY as well as its title', () => {
    // An announcement's title is often "Notice"; the words the operator
    // remembers are in the text.
    const list = [
      { id: 1, title: 'Notice', body: 'Church picnic on Saturday' },
      { id: 2, title: 'Baptism', body: 'Next month' },
    ];
    expect(filterAnnouncements(list, 'picnic').map((a) => a.id)).toEqual([1]);
    expect(filterAnnouncements(list, 'baptism').map((a) => a.id)).toEqual([2]);
    expect(filterAnnouncements(list, '')).toHaveLength(2);
    expect(filterAnnouncements(null, 'x')).toEqual([]);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · THE FOUR PAYLOAD BUILDERS, READ AGAINST WHAT CONSUMES THEM
// ─────────────────────────────────────────────────────────────────────────────
describe('a result row becomes a cue whose payload the rest of the app can read', () => {
  it('a verse carries everything the preview and the wall need', () => {
    const built = verseCue({
      book: 'Romans',
      chapter: 8,
      verse: 28,
      reference: 'Romans 8:28',
      text: 'And we know that all things work together for good',
      translation: 'KJV',
    });
    expect(built.cue_type).toBe('scripture');
    expect(built.label).toBe('Romans 8:28');
    // Round-tripped through the real reader, not compared field by field: what
    // matters is that `slidesOf` finds words in it, because a scripture cue with
    // no text is a cue that would put nothing in front of a congregation.
    const slide = slidesOf({ cue_type: 'scripture', payload_json: JSON.stringify(built.payload) })[0];
    expect(slide.label).toBe('Romans 8:28');
    expect(slide.text).toContain('work together');
  });

  it('a media row carries the ID, so the cue previews as a picture and not a filename', () => {
    // The defect this key exists to prevent: a payload carrying only the filename
    // left every surface downstream with a name and nothing to look up.
    const built = mediaCuePayload({ id: 7, filename: 'sunrise.jpg', kind: 'image' });
    expect(built.cue_type).toBe('media');
    const slide = slidesOf({
      cue_type: 'media',
      label: built.label,
      payload_json: JSON.stringify(built.payload),
    })[0];
    expect(slide.media_id).toBe(7);
    expect(slide.media_kind).toBe('image');
  });

  it('a video row keeps its kind — an image cue would paint a still of nothing', () => {
    const built = mediaCuePayload({ id: 8, filename: 'loop.mp4', kind: 'video' });
    const slide = slidesOf({ cue_type: 'media', payload_json: JSON.stringify(built.payload) })[0];
    expect(slide.media_kind).toBe('video');
  });

  it('an untitled announcement still gets a name', () => {
    expect(announceCuePayload({ id: 3, title: '', body: 'Coffee after' }).label).toBe('Announcement');
    expect(announceCuePayload({ id: 3, title: 'Baptism', body: '' }).label).toBe('Baptism');
  });

  it('a countdown trims the operator’s words, and blank stays blank', () => {
    // Blank is a REAL answer — it puts the digits on the wall and nothing else.
    // Trimming here rather than at the call site is what stops a trailing space
    // reaching a congregation screen.
    const built = countdownCuePayload(5, '  Service begins in  ', '');
    expect(built.payload.label).toBe('Service begins in');
    expect(built.payload.done).toBe('');
    expect(built.label).toBe('Countdown · 5 min');
    expect(built.seconds).toBe(300);
  });

  it('an unreadable minute count falls back to five rather than storing a zero', () => {
    // `set_plan_timer` clears anything non-positive, so a 0 produces a countdown
    // cue with no clock and nothing saying why.
    for (const bad of [0, -3, NaN, null, undefined, 'x']) {
      expect(countdownCuePayload(bad, '', '').payload.minutes, String(bad)).toBe(5);
    }
    expect(countdownCuePayload(12, '', '').payload.minutes).toBe(12);
  });

  it('a countdown cue draws its own clock, so the preview must not call it broken', () => {
    // The builders are only correct if the verdicts downstream still land right.
    const built = countdownCuePayload(5, '', '');
    const item = { cue_type: built.cue_type, payload_json: JSON.stringify(built.payload) };
    expect(previewState(item, false).state).toBe('self');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · THE DRAG, AS ARITHMETIC
// ─────────────────────────────────────────────────────────────────────────────
describe('the running order paints the order the operator is about to get', () => {
  const H = 34;

  it('an unlaid-out list moves nothing, rather than moving everything by NaN', () => {
    // `offsetHeight` is 0 in a list mid-layout, and always in jsdom. Dividing by
    // it gives Infinity, and `translateY(NaN px)` on every neighbour.
    expect(rowsMoved(120, 0)).toBe(0);
    expect(rowsMoved(120, -5)).toBe(0);
    expect(Number.isFinite(rowOffset(3, 1, 2, 0, 120))).toBe(true);
    expect(dragFrame(4, 1, 0, 120).every((n) => Number.isFinite(n))).toBe(true);
  });

  it('half a row has not moved; over half a row has', () => {
    expect(rowsMoved(16, H)).toBe(0);
    expect(rowsMoved(18, H)).toBe(1);
    expect(rowsMoved(-18, H)).toBe(-1);
    expect(rowsMoved(70, H)).toBe(2);
  });

  it('it agrees with plan.js about where the cue lands — SEEN and GOT are one number', () => {
    // The two halves of a drag live in two modules on purpose, so this is the
    // seam where they could come apart. `dropIndex` decides what the operator
    // GETS; `rowsMoved` decides what they SEE while deciding it.
    for (const dy of [-120, -70, -18, -16, 0, 16, 18, 70, 120]) {
      const from = 4;
      const seen = Math.max(0, Math.min(9, from + rowsMoved(dy, H)));
      expect(dropIndex(from, dy, H, 10), `dy=${dy}`).toBe(seen);
    }
  });

  it('the dragged row follows the pointer exactly, never a whole number of rows', () => {
    // A row that snapped between slots would stop being the thing under the finger.
    expect(rowOffset(2, 2, 1, H, 41)).toBe(41);
    expect(rowOffset(2, 2, 0, H, 7)).toBe(7);
  });

  it('only the rows the drag has PASSED open the gap', () => {
    // Dragging row 1 down two rows: 2 and 3 slide up, 4 and 0 sit still.
    const frame = dragFrame(5, 1, H, 70);
    expect(frame[1]).toBe(70);
    expect(frame[2]).toBe(-H);
    expect(frame[3]).toBe(-H);
    expect(frame[4]).toBe(0);
    expect(frame[0]).toBe(0);
  });

  it('…and the same upwards', () => {
    // Dragging row 3 up two rows: 1 and 2 slide down, 0 and 4 sit still.
    const frame = dragFrame(5, 3, H, -70);
    expect(frame[3]).toBe(-70);
    expect(frame[2]).toBe(H);
    expect(frame[1]).toBe(H);
    expect(frame[0]).toBe(0);
    expect(frame[4]).toBe(0);
  });

  it('a drag that has not travelled a whole row leaves every neighbour alone', () => {
    const frame = dragFrame(5, 2, H, 9);
    expect(frame[2]).toBe(9);
    expect(frame.filter((_, i) => i !== 2)).toEqual([0, 0, 0, 0]);
  });

  it('a still row is exactly 0, so the caller can CLEAR the style instead of writing one', () => {
    // The each block is keyed, so Svelte reuses these nodes: an inline
    // `translateY(0px)` left behind paints the new order shifted by a row.
    expect(dragFrame(3, 0, H, 0)).toEqual([0, 0, 0]);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4 · AND THE VIEW ACTUALLY USES THEM
// ─────────────────────────────────────────────────────────────────────────────
describe('the extraction is the only copy, not a second one', () => {
  const src = readFileSync(PLANNER, 'utf8');

  it('the Planner imports both modules', () => {
    expect(src).toMatch(/from '\.\.\/planneradd\.js'/);
    expect(src).toMatch(/from '\.\.\/plannerdrag\.js'/);
  });

  it('the payload literals are gone from the view', () => {
    // A builder in a module and a literal left behind in the view is two rules
    // for one shape, which is how they drift. `media_id:` and `announce_id:`
    // appear in the payload builders and nowhere else in the component.
    expect(src).not.toMatch(/media_id:\s*m\.id/);
    expect(src).not.toMatch(/announce_id:\s*a\.id/);
    expect(src).not.toMatch(/minutes:\s*m,/);
  });

  it('the drag handler no longer does the arithmetic itself', () => {
    // The three lines that decided a neighbour's offset are in `plannerdrag.js`.
    // If they come back here, they are untestable again.
    expect(src).not.toMatch(/if \(shift > 0 && i > drag\.from/);
    expect(src).toMatch(/dragFrame\(/);
  });
});
