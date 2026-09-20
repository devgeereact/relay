// THE RUNNING ORDER'S POINTER DRAG, AS ARITHMETIC.
//
// The second of the two self-contained blocks lifted out of
// `ServicePlanner.svelte`. `plan.js` already owns where a drag LANDS
// (`dropIndex`, `reorderTo`) — deliberately, because "a drag that lands one row
// off is a Tuesday-evening reorder that silently is not the order the operator
// saw". What stayed behind in the component was the other half: where every row
// is PAINTED while the finger is still down, which is the half that decides
// whether the operator can see the order they are about to get.
//
// That arithmetic was three lines inside `onDragMove`, next to `style.transform`
// assignments, so it could only be checked by dragging a real list with a real
// mouse — on a machine that cannot screenshot the app. It is pure. It is here.
//
// THE COMPONENT STILL OWNS THE DOM. These functions return numbers; the caller
// writes the transform. That boundary is the point: a module that reached into
// the list would need a laid-out list to test, which is exactly the thing this
// was extracted to stop needing.

/**
 * How many whole rows a drag of `dy` pixels has travelled.
 *
 * A non-positive row height means nothing has moved — an unlaid-out list reports
 * `offsetHeight === 0` (and jsdom always does), and dividing by it yields
 * `Infinity` and then `NaN`, which would paint every neighbour at `NaN` pixels.
 * `plan.js::dropIndex` guards the same case for the same reason; the two must
 * agree, because one decides what the operator SEES and the other decides what
 * they GET, and a disagreement between those is a reorder nobody chose.
 */
export function rowsMoved(dy, rowHeight) {
  const h = Number(rowHeight) || 0;
  if (h <= 0) return 0;
  return Math.round((Number(dy) || 0) / h);
}

/**
 * Where row `i` is painted during a drag started on row `from`.
 *
 * The dragged row follows the pointer EXACTLY — `dy`, not a whole number of rows
 * — because a row that snapped between slots would stop being the thing under
 * the finger. Its neighbours move by exactly one row height, in the direction
 * that opens the gap, and only the ones the dragged row has actually passed:
 *
 *   dragging DOWN  · rows from+1 … from+shift slide UP by one row
 *   dragging UP    · rows from+shift … from-1 slide DOWN by one row
 *
 * Everything else sits still. Returning 0 rather than a transform for those is
 * deliberate — the caller clears the style instead of writing `translateY(0px)`,
 * so a keyed `{#each}` re-render cannot inherit a stale inline transform.
 */
export function rowOffset(i, from, shift, rowHeight, dy) {
  if (i === from) return Number(dy) || 0;
  const h = Number(rowHeight) || 0;
  if (h <= 0 || !shift) return 0;
  if (shift > 0 && i > from && i <= from + shift) return -h;
  if (shift < 0 && i < from && i >= from + shift) return h;
  return 0;
}

/**
 * Every row's offset for one frame of a drag, as an array parallel to the list.
 *
 * One call per pointer move rather than one per row, so the whole frame is
 * decided in one place and a caller cannot apply `rowOffset` to some rows and
 * not others — the shape of bug this repository files under "a guarantee is only
 * kept on the doors you checked".
 */
export function dragFrame(count, from, rowHeight, dy) {
  const n = Math.max(0, Number(count) || 0);
  const shift = rowsMoved(dy, rowHeight);
  const out = new Array(n);
  for (let i = 0; i < n; i += 1) out[i] = rowOffset(i, from, shift, rowHeight, dy);
  return out;
}
