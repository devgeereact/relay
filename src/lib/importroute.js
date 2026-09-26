/**
 * WHICH SHELF A FILE FROM A PROPRESENTER FOLDER BELONGS ON.
 *
 * A ProPresenter library is not one flat pile. It is folders — `SONGS`,
 * `SCRIPTURES`, `HMYN`, `SPOKEN WORDS`, `ANNOUNCEMENT` — and that grouping is the
 * church's own, made deliberately over years. Measured in the real export this
 * exists for, from its own `LibraryData` index:
 *
 * ```text
 *   SONGS         ~721
 *   SCRIPTURES     ~49
 *   HMYN           ~18
 *   SPOKEN WORDS   ~11
 *   ANNOUNCEMENT    ~4
 * ```
 *
 * Throwing that away and calling all 800 of them "songs" is losing information the
 * operator will then have to put back by hand, one item at a time.
 *
 * ## Why this needs no protobuf
 *
 * The grouping lives in `LibraryData`, a protobuf file `proimport.rs` does not
 * parse — but it does not need to. A folder import already knows which directory
 * each file came from, because the browser puts it on `webkitRelativePath`. The
 * index and the directory tree say the same thing, and one of them is free.
 *
 * **And there are no service playlists to import, whatever is decoded.** The
 * export's `Library` file holds two entries and `PlaylistTemplates` is zero bytes.
 * A decoder written for playlists would find nothing, which is worth recording
 * here so the next person does not write one to find that out.
 */

/** The folder a picked file came from, or `''` when the browser did not say. */
export function sourceFolder(file) {
  const rel = String(file?.webkitRelativePath ?? '');
  if (!rel) return '';
  const parts = rel.split('/').filter(Boolean);
  // The LAST directory, not the first: the first is the folder the operator
  // picked, which is the same for every file and says nothing.
  return parts.length >= 2 ? parts[parts.length - 2] : '';
}

/**
 * Where a file from that folder should land.
 *
 * `'announcement'` or `'song'`. Deliberately only two answers, and the reason the
 * scripture case is not a third is worth stating: a ProPresenter `SCRIPTURES`
 * folder holds SLIDE DECKS of verses, and Relay's scripture collection holds
 * actual verses out of the bundled KJV with a book, a chapter and a number. A deck
 * has none of those. Filing it as saved scripture would put something in that
 * collection which nothing there can navigate, search or fire as a passage — so it
 * goes where its SHAPE belongs, which is with the songs, and the import report
 * says how many did.
 *
 * Matching is loose on purpose. This is a folder a human named: `HMYN` is in the
 * real library and is a typo for HYMN, and `ANNOUNCEMENTS` and `ANNOUNCEMENT` are
 * the same shelf.
 */
export function shelfFor(folder) {
  const f = String(folder ?? '')
    .trim()
    .toUpperCase();
  if (!f) return 'song';
  if (f.startsWith('ANNOUNCE') || f.startsWith('NOTICE')) return 'announcement';
  return 'song';
}

/**
 * What came from where, for the report.
 *
 * Counts by the folder's own name rather than by shelf, because the folder name is
 * the church's word and the shelf is Relay's. An operator recognises `HMYN`; they
 * do not recognise "18 items were classified as songs".
 */
export function foldersIn(files) {
  const out = new Map();
  for (const f of files ?? []) {
    const folder = sourceFolder(f);
    if (!folder) continue;
    out.set(folder, (out.get(folder) ?? 0) + 1);
  }
  return [...out.entries()]
    .map(([folder, count]) => ({ folder, count, shelf: shelfFor(folder) }))
    .sort((a, b) => b.count - a.count);
}
