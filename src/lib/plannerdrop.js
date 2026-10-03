// DROPPING A FILE STRAIGHT ONTO A RUNNING ORDER.
//
// There was no file drop anywhere in `src/` — zero `dataTransfer.files`, and no
// `tauri://drag-drop` / `dragDropEnabled` / `onDragDropEvent` on the Rust side
// either. Adding a background meant: Library → Import → file dialog → the media
// review sheet → back to Planner → Add cue → search for the filename → click.
// Seven steps to put a picture in a plan.
//
// ── THE WEBVIEW PATH, NOT A NEW CAPABILITY ─────────────────────────────────
//
// This is a plain HTML5 `drop` reading `dataTransfer.files`, into the import path
// that already exists: `fileToBase64` → `importMedia`. No new Rust command, no
// new Tauri capability, and the file goes through the same `MAX_IMPORT_BYTES`
// guard and the same `media_assets` row as a file chosen in the Library.
//
// IT MUST NOT COLLIDE WITH THE REORDER. The running order's own drag was
// deliberately migrated OFF HTML5 drag-and-drop to pointer events, for three
// recorded reasons (the row did not move, the drop target was the wrong row, and
// touch never started one). The two do not overlap by construction — a file
// dragged from the operating system fires `dragenter`/`dragover`/`drop` and never
// `pointerdown` — but the caller still holds the reorder off while a file is over
// the list, because a half-finished reorder and an insertion marker on screen at
// once is two answers to "where will this land".
//
// ── WHAT IS REFUSED, AND WHY IT IS REFUSED HERE ─────────────────────────────
//
// A DOCUMENT is refused at the drop rather than at the fire. `main.rs` answers
// "documents can't be shown as an output background yet" from `fire_media` and
// `show_background`, which means a dropped PDF would become a cue that looks
// exactly like a working one and fails on a Sunday, in front of people, at the
// moment it is reached. A cue that cannot be fired must not be built.
//
// The Library takes documents and should keep taking them — it is a library, and
// a PDF in it is a file somebody wanted stored. This refusal is about a PLAN CUE,
// and the sentence says which so the operator knows where the file does belong.
//
// A file of an unknown kind is refused with its extension named. "Unsupported
// file" tells a volunteer nothing they can act on.
//
// Pure, and here, so the triage can be checked against `Library.svelte`'s own
// extension lists without a browser and without a drag.

/**
 * The extensions Relay imports, by kind. THE SAME THREE LISTS `Library.svelte`
 * routes on — `plannerdrop.test.js` reads them out of that file and asserts they
 * have not drifted, because two copies of "what Relay accepts" is how a file
 * becomes importable in one place and invisible in the other. `Library.svelte`
 * records that exact defect against its own file picker, whose accept list was
 * once shorter than its router's.
 */
export const IMAGE_EXT = ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'avif', 'svg'];
export const VIDEO_EXT = ['mp4', 'mov', 'webm', 'mkv', 'm4v'];
export const DOC_EXT = ['pdf', 'pptx', 'ppt', 'key'];

/** The lowercased extension of a filename, without the dot. '' when there is none. */
export function extOf(name) {
  const s = String(name ?? '');
  const dot = s.lastIndexOf('.');
  return dot > 0 ? s.slice(dot + 1).toLowerCase() : '';
}

/**
 * What Relay would call this file: `image`, `video`, `document`, or null.
 *
 * `document` is a real answer rather than null on purpose — the refusal it earns
 * is a different sentence from "Relay does not know this kind of file", and the
 * operator can act on only one of the two.
 */
export function kindOfFile(name) {
  const ext = extOf(name);
  if (IMAGE_EXT.includes(ext)) return 'image';
  if (VIDEO_EXT.includes(ext)) return 'video';
  if (DOC_EXT.includes(ext)) return 'document';
  return null;
}

/**
 * Sort a dropped batch into what becomes a cue and what does not.
 *
 * Returns `{ accept: [{ file, kind }], refused: [{ name, reason }] }`. A mixed
 * drop is NOT all-or-nothing: the pictures go in and the PDF is named. Refusing
 * the whole batch because one file was wrong would make an operator drag the rest
 * again, and dropping the PDF silently would be worse still.
 *
 * `reason` is a whole sentence about THAT file, because the operator is going to
 * read it in a bar at the bottom of a pane and not next to the file.
 */
export function triageDrop(files) {
  const accept = [];
  const refused = [];
  for (const file of Array.from(files ?? [])) {
    const name = file?.name ?? '';
    const kind = kindOfFile(name);
    if (kind === 'image' || kind === 'video') {
      accept.push({ file, kind });
    } else if (kind === 'document') {
      refused.push({
        name,
        reason: `${name} is a document, and Relay cannot put a document on an output screen yet — so a cue built from it would fail when it was fired. Import it in the Library if you want it stored.`,
      });
    } else {
      const ext = extOf(name);
      refused.push({
        name,
        reason: ext
          ? `Relay does not import .${ext} files. Pictures (${IMAGE_EXT.join(', ')}) and video (${VIDEO_EXT.join(', ')}) can become plan cues.`
          : `${name || 'That item'} has no file extension, so Relay cannot tell what it is.`,
      });
    }
  }
  return { accept, refused };
}

/**
 * The ONE sentence a refusal shows, however many files were turned away.
 *
 * One refusal is quoted whole. Several are counted and the FIRST reason is given,
 * because a bar that grew a line per file would push the running order off the
 * pane — and the first reason is almost always the only reason, an operator
 * having dragged a folder of one kind of thing.
 */
export function refusalMessage(refused) {
  const list = refused ?? [];
  if (!list.length) return '';
  if (list.length === 1) return list[0].reason;
  return `${list.length} files were not added. ${list[0].reason}`;
}

/**
 * WHERE THE DROP WILL LAND, from the rows' own geometry.
 *
 * A gap index in `0…rows.length`: 0 is above every row, `rows.length` is below
 * them all. Each row's MIDPOINT is the boundary, which is the rule every list in
 * every operating system uses and the one an operator's hand already knows.
 *
 * This exists as a function rather than as a line in the handler because the
 * requirement is that the operator can SEE where the cue will land before they
 * let go — so the marker and the eventual insertion have to be the same number,
 * and a number computed in two places is two numbers.
 *
 * DEGENERATE GEOMETRY IS THE END OF THE LIST, deterministically. An unlaid-out
 * pane reports every rect as zero (jsdom always does); answering "after
 * everything" there is the same answer `add_plan_item` gives on its own, so a
 * drop onto a list that has not been measured behaves exactly as a plain add.
 */
export function dropGapAt(rects, y) {
  const list = rects ?? [];
  const at = Number(y);
  if (!list.length || !Number.isFinite(at)) return 0;
  for (let i = 0; i < list.length; i += 1) {
    const r = list[i] || {};
    const top = Number(r.top) || 0;
    const bottom = Number(r.bottom) || 0;
    if (at < (top + bottom) / 2) return i;
  }
  return list.length;
}

/**
 * The plan's order with `newIds` lifted out of wherever they are and dropped into
 * the gap the operator was shown.
 *
 * `ids` is the order AFTER the adds, so the new cues are sitting at the end —
 * `add_plan_item` appends and there is no insert command. `at` is a gap index in
 * the order as it was BEFORE the adds, which is the list the operator was looking
 * at when they chose it.
 *
 * Ids that are not in the list are dropped rather than invented, and an empty
 * result is returned unchanged: a reorder built from a guess persists.
 */
export function orderWithDropAt(ids, newIds, at) {
  const list = (ids ?? []).slice();
  const moving = (newIds ?? []).filter((n) => list.includes(n));
  if (!moving.length) return list;
  const kept = list.filter((n) => !moving.includes(n));
  const gap = Math.max(0, Math.min(kept.length, Math.trunc(Number(at)) || 0));
  kept.splice(gap, 0, ...moving);
  return kept;
}

/** The sentence the bar reports once a drop has landed. */
export function droppedMessage(count) {
  const n = Math.max(0, Math.trunc(Number(count) || 0));
  if (!n) return '';
  return n === 1 ? 'Added 1 media cue from the file you dropped.' : `Added ${n} media cues from the files you dropped.`;
}
