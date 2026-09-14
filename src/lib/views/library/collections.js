/**
 * THE COLLECTION REGISTER — the one place that says what a Library collection
 * is, what colour it wears, and which panes live inside it.
 *
 * `docs/REBRAND.md` §10 asks the Library to use the same grammar as Live:
 * collections across the top, items down the left rail, slides in the big grid.
 * A collection is a KIND of content, and the kinds are already named by the
 * template engine — scripture, song, notice, media — so this register borrows
 * those names rather than inventing a fifth vocabulary for the same four things.
 *
 * **A collection is not a pane.** Two of the four hold more than one pane
 * (scripture is a Bible you read AND the verses you saved; media is the moving
 * half and the still half, which ProPresenter separates for the same reason —
 * a still you put behind words is a different job from a video you play). The
 * VIEW is what renders; the COLLECTION is what an operator is thinking about.
 * Keeping the view key as the shell's single source of truth is what lets every
 * existing pane, prop and binding stay exactly as it was.
 *
 * **The colour is a tint, never a status** (CLAUDE.md rule 18, DECISIONS §21).
 * Amber means ON AIR on every other surface in this console, so a collection
 * colour is allowed to paint a small square icon tile and nothing else — never
 * a badge, never a filled chip, never the word LIVE. Selection stays steel
 * blue, which is what steel blue means everywhere else.
 */

/** @typedef {{key:string,label:string}} LibraryView */

export const COLLECTIONS = [
  {
    key: 'scripture',
    label: 'Scripture',
    // The token is `--v-col-<colour>`; see `src/app.css`.
    colour: 'scripture',
    /** What the count is counting, said in words for a screen reader. */
    counts: 'saved verse',
    views: [
      // BIBLE is first inside the collection for the same reason it was the
      // first sub-tab: the Library could search and could list what had been
      // saved, but could not open a Bible and read it.
      { key: 'browse', label: 'Bible' },
      { key: 'scripture', label: 'Saved' },
    ],
  },
  {
    key: 'songs',
    label: 'Songs',
    colour: 'song',
    counts: 'song',
    views: [{ key: 'lyrics', label: 'Songs' }],
  },
  {
    key: 'notices',
    label: 'Announcements',
    colour: 'notice',
    counts: 'announcement',
    views: [{ key: 'announcements', label: 'Announcements' }],
  },
  {
    key: 'media',
    label: 'Media',
    colour: 'media',
    counts: 'item',
    views: [
      { key: 'media', label: 'Video & documents' },
      { key: 'graphics', label: 'Graphics' },
    ],
  },
];

/** Every view key the Library can render, in collection order. */
export const VIEW_KEYS = COLLECTIONS.flatMap((c) => c.views.map((v) => v.key));

/** The collection a pane belongs to. Returns `undefined` for a key nothing owns. */
export function collectionOf(viewKey) {
  return COLLECTIONS.find((c) => c.views.some((v) => v.key === viewKey));
}

/** The collection with this key. */
export function collectionByKey(key) {
  return COLLECTIONS.find((c) => c.key === key);
}

/**
 * What the count says out loud.
 *
 * `null` is NOT zero, and this is the whole reason the function exists (rule
 * 35): a count that has not loaded yet, and a count whose query failed, must
 * not read the same as an empty collection. An operator who sees "0 songs"
 * over a broken query goes looking for the songs they know they imported.
 */
export function countWords(collection, n) {
  const noun = collection?.counts ?? 'item';
  if (n === null || n === undefined) return 'count not loaded';
  if (n < 0) return 'count unavailable';
  return `${n} ${noun}${n === 1 ? '' : 's'}`;
}

/** The digits on the chip — or a mark that is visibly not a number. */
export function countMark(n) {
  if (n === null || n === undefined) return '·';
  if (n < 0) return '!';
  return String(n);
}
