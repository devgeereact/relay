// THE SLIDE GRID — what is staged, as cells, and what a press on one MEANS.
//
// Two separable jobs live here, both pure, because both are the kind of thing
// that is only ever wrong in front of a congregation:
//
//   1. `gridSource` turns whatever is staged — an open plan, or the chapter the
//      live verse belongs to — into a flat list of cells. No fetching, no
//      stores: the caller hands in what it already has.
//
//   2. `pressArbiter` decides what a press on a cell does. Single click sends to
//      programme; double click previews (docs/REBRAND.md §2). That is a
//      deliberately dangerous default, so the arbitration is here, tested, and
//      not scattered through a component: the single press waits one beat
//      (`PRESS_MS`) and a second press inside that beat CANCELS it. A double
//      must never fire both — otherwise every preview puts the slide on the
//      wall on its way past, which is exactly the accident this surface cannot
//      afford.
//
// What this module deliberately does NOT do: decide anything is on air. It has
// no notion of success. `send` is the caller's existing fire path, which reports
// its own outcome and marks amber only after the fire resolves (CLAUDE.md rules
// 15 and 18). A rejection goes to `onError` and nothing here calls it a take.

/** The beat a single press waits, so a double can cancel it. */
export const PRESS_MS = 190;

/**
 * One cell of the grid.
 *
 * @typedef {object} Cell
 * @property {string} key    stable identity for a keyed `{#each}`
 * @property {number} n      1-based position, for the corner number
 * @property {string} label  what the cell is called
 * @property {string} text   the words, for the thumbnail
 * @property {string} tag    the section tag (Verse / Chorus / …), or ''
 * @property {'plan'|'verse'|'song'} kind  which fire path this cell takes
 * @property {string} ctype  the CONTENT kind — 'scripture', 'song', 'media',
 *                           'announce', 'countdown' — so a caller can render the
 *                           cell the way the wall would. A lyric slide projects
 *                           the lyric, never the section name.
 * @property {boolean} empty this cue has nothing to show. See `planCells`.
 * @property {string|null} cueId    plan cells only
 * @property {number} slideIdx      plan cells only
 * @property {string|null} reference verse cells only
 */

/**
 * An open plan, flattened: every slide of every cue, in running order.
 *
 * The grid is FLAT on purpose. The rail beside it already shows the plan's
 * shape; the grid answers a different question — "which of these do I want on
 * the wall right now" — and a nested grid answers it more slowly.
 *
 * A CUE WITH NO SLIDES STILL GETS A CELL, and this is the whole reason the
 * `empty` flag exists. `slidesOf` answers `[]` for a cue it cannot expand — a
 * song whose `sections` were never written into its payload, a media cue with no
 * asset, a `cue_type` this build does not know — and the grid used to drop those
 * cues on the floor. Measured on a real eight-cue plan: the plan rail counted 8,
 * the grid counted 6, and nothing anywhere said which two were missing or why.
 * A cue an operator loaded and cannot see is the "nothing may become
 * unreachable" rule failing quietly, on the run surface, mid-service.
 *
 * So the cue gets a cell that says it is empty. The caller is expected to refuse
 * the press — there is genuinely nothing to put on a wall — but the cue is
 * COUNTED and NAMED, which is the difference between a gap the operator can see
 * and one they find out about when they press `→`.
 *
 * @param {Array} items plan items, as `Live` already holds them
 * @param {(item:any)=>Array<{label?:string,text?:string,tag?:string}>} slidesOf
 */
export function planCells(items, slidesOf) {
  const cells = [];
  for (const item of items ?? []) {
    const slides = slidesOf(item) ?? [];
    const ctype = item.cue_type || 'unknown';
    if (!slides.length) {
      cells.push({
        key: `p:${item.id}:empty`,
        n: cells.length + 1,
        label: item.label || '',
        text: '',
        tag: '',
        kind: 'plan',
        ctype,
        empty: true,
        cueId: item.id,
        slideIdx: 0,
        reference: null,
      });
      continue;
    }
    slides.forEach((s, i) => {
      cells.push({
        key: `p:${item.id}:${i}`,
        n: cells.length + 1,
        label: s.label || item.label || '',
        text: s.text || '',
        tag: s.tag || '',
        kind: 'plan',
        ctype,
        empty: false,
        cueId: item.id,
        slideIdx: i,
        reference: null,
      });
    });
  }
  return cells;
}

/**
 * A chapter, as cells — one verse each.
 *
 * The text is carried for the thumbnail only. The FIRE goes by reference, so
 * what reaches the wall is resolved by the backend from the active translation
 * rather than from whatever this pane happened to load.
 *
 * @param {Array<{verse:number,text:string,reference:string}>} verses
 */
export function passageCells(verses) {
  return (verses ?? []).map((v, i) => ({
    key: `v:${v.reference ?? i}`,
    n: i + 1,
    label: v.reference ?? String(v.verse ?? i + 1),
    text: v.text ?? '',
    tag: `v${v.verse ?? i + 1}`,
    kind: 'verse',
    ctype: 'scripture',
    empty: false,
    cueId: null,
    slideIdx: i,
    reference: v.reference ?? null,
  }));
}

/**
 * A song, as cells — one reflowed slide each.
 *
 * The rail's Songs half stages a song here (docs/REBRAND.md §2: the run surface
 * offers both collections). A song slide carries its WORDS, because that is what
 * the fire sends and what the congregation reads; the section name rides along
 * as the cell's tag for the operator and is suppressed on the way to the glass
 * by `fire_content` itself (§10, and rule 36 — one place decides).
 *
 * `reference` stays null: a song section has no canonical reference to resolve,
 * which is exactly why its fire is `fireContent` and not `manualFire`. The
 * label is what the cue is CALLED in history, the same string `LyricsPane`
 * already sends, so the two surfaces cannot name the same slide differently.
 *
 * @param {Array<{key?:string,label?:string,lyrics?:string}>} slides a reflowed deck
 * @param {string} title the song's title
 */
export function songCells(slides, title) {
  return (slides ?? []).map((s, i) => ({
    key: `s:${title}:${s.key ?? i}`,
    n: i + 1,
    label: `${title}${s.label ? ` · ${s.label}` : ''}`,
    text: s.lyrics ?? '',
    tag: s.label ?? '',
    kind: 'song',
    ctype: 'song',
    empty: !(s.lyrics ?? '').trim(),
    cueId: null,
    slideIdx: i,
    reference: null,
  }));
}

/**
 * What is staged right now.
 *
 * A plan wins when one is open — it is the thing the operator deliberately
 * loaded. Otherwise the chapter around the live verse, which is how a preacher
 * who has gone off the plan is followed.
 *
 * Returns a title as well as the cells because an empty grid has to say WHICH
 * empty it is: no plan and no verse is "nothing staged", a plan whose cues are
 * all empty is a different problem.
 *
 * A HAND PICK OUTRANKS THE PLAN, and only a hand pick. `handPicked` is set when
 * the operator chose this chapter — or this song — in the Live rail: the most
 * recent deliberate act, so the grid shows it.
 * What must never displace a plan is a DETECTION: the preacher quoting something
 * is not the operator asking for it, and `handPicked` stays false on that path.
 *
 * A hand-picked SONG and a hand-picked CHAPTER cannot both be staged: the rail
 * clears one when it stages the other, so the song is simply checked first and
 * there is no third rule to get wrong.
 *
 * @returns {{ title: string, cells: Cell[], source: 'plan'|'passage'|'song'|'none' }}
 */
export function gridSource({
  planOpen,
  planTitle,
  items,
  slidesOf,
  verses,
  passageTitle,
  songSlides,
  songTitle,
  handPicked = false,
}) {
  if (handPicked && (songSlides ?? []).length) {
    return { title: songTitle || '', cells: songCells(songSlides, songTitle || ''), source: 'song' };
  }
  if (planOpen && !handPicked) {
    return { title: planTitle || 'Plan', cells: planCells(items, slidesOf), source: 'plan' };
  }
  const cells = passageCells(verses);
  if (cells.length) return { title: passageTitle || '', cells, source: 'passage' };
  // A hand-picked chapter that has not arrived (or would not load) must not blank
  // an open plan and tell the operator nothing is staged. The plan is still the
  // truthful answer until the verses are actually here.
  if (planOpen) return { title: planTitle || 'Plan', cells: planCells(items, slidesOf), source: 'plan' };
  return { title: '', cells: [], source: 'none' };
}

/**
 * Single click sends to programme; double click previews.
 *
 * @param {object} o
 * @param {(cell:Cell)=>any} o.send      the caller's real fire path
 * @param {(cell:Cell)=>any} o.preview   stage it in the preview pane only
 * @param {(err:any)=>void} [o.onError]  where a rejected `send` goes
 * @param {number} [o.delay]             the beat, in ms
 * @param {Function} [o.setTimer]        injectable for tests
 * @param {Function} [o.clearTimer]
 */
export function pressArbiter({
  send,
  preview,
  onError = () => {},
  delay = PRESS_MS,
  setTimer = setTimeout,
  clearTimer = clearTimeout,
}) {
  let pending = null;

  /** Drop any press that has not yet happened. */
  function cancel() {
    if (pending !== null) {
      clearTimer(pending);
      pending = null;
    }
  }

  return {
    /**
     * A single press. Arms the send; a `double` inside `delay` disarms it.
     *
     * The press is armed even while one is already pending, and the older one is
     * dropped — two quick presses on DIFFERENT cells must send the second, not
     * both. (Two on the SAME cell is a double click, and the browser delivers
     * `dblclick` for it, which cancels.)
     */
    press(cell) {
      cancel();
      pending = setTimer(() => {
        pending = null;
        try {
          const r = send(cell);
          // A fire is async and can fail. Route the failure out; never treat
          // "the press happened" as "the screen changed".
          if (r && typeof r.then === 'function') r.then(undefined, onError);
        } catch (e) {
          onError(e);
        }
      }, delay);
    },

    /** The second press of a double. Cancels the pending send, then previews. */
    double(cell) {
      cancel();
      preview(cell);
    },

    /**
     * Drop a pending send without doing anything else.
     *
     * Called on destroy, and whenever the transport is taken back by hand: a
     * view that has gone away must not put scripture on a wall a beat later.
     */
    cancel,

    /** Test seam: is a send still armed? */
    get armed() {
      return pending !== null;
    },
  };
}
