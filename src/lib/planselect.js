// PICKING CUES OUT OF A RUNNING ORDER, AND WHAT HAPPENS TO THE PLAN AFTERWARDS.
//
// The Planner could only delete a cue from the INSPECTOR, so removing three cues
// was three select-then-travel round trips across the desk. The operator asked
// for the opposite of that and drew the line themselves: *"do not hide
// destructive deletion behind ambiguity, but do not add friction that slows a
// live operator either."*
//
// Both halves of that sentence are rules about this file.
//
// NOT AMBIGUOUS. A delete says how many cues it will take and names the one it
// is about when there is only one, and the arming step is a state the CONTROL
// reports rather than a dialog that appears somewhere else. Rule 41 rules out the
// obvious shortcut: the Tauri webview does not implement `confirm()`, so a
// two-step delete guarded by one deletes nothing and reports success.
//
// NOT SLOW. One click is still one selection; the modifier keys are the fast
// path and nothing requires them. A plain click behaves exactly as it did before
// multi-select existed, which is the case a hurried operator is always in.
//
// THREE MODIFIER COMBINATIONS, ONE DOOR. `selectionAfterClick` is the only place
// that decides what a click does to the picked set. Three handlers deciding it
// separately is how the fourth one gets it wrong — the shape this repository
// files under "a guarantee is only kept on the doors you checked" — and it is
// also why the RANGE case can be written once against the plan's real order
// rather than guessed at per call site.
//
// Pure, and here rather than in the component, so the case that actually costs
// something — what the inspector lands on after a run of cues is deleted — can be
// checked without mounting a desk.

/** The ordered position of `id` in `items`, or -1. */
function indexOf(items, id) {
  return (items ?? []).findIndex((i) => i?.id === id);
}

/**
 * What a click on cue `id` does to the picked set.
 *
 * `picked` is the array of ids currently picked, `anchor` the id a range extends
 * FROM, and the two modifier flags are what the event carried:
 *
 *   plain      · this cue alone, and it becomes the anchor
 *   additive   · (Cmd / Ctrl) toggle this cue, and it becomes the anchor
 *   range      · (Shift) every cue from the anchor to this one, inclusive
 *
 * A RANGE WITH NO ANCHOR IS A PLAIN CLICK, not an empty selection: Shift-clicking
 * as the first thing you do is a reasonable mistake and losing the click to it
 * teaches nothing. A range keeps its anchor so a second Shift-click grows or
 * shrinks the same run rather than starting a new one from where the last ended —
 * which is how every list in every operating system behaves, and an operator
 * correcting an overshoot expects it.
 *
 * ADDITIVE TAKES PRECEDENCE over range when both are held, deliberately: the two
 * together are ambiguous, and the narrower of the two answers is the one that
 * cannot surprise somebody into picking twenty cues.
 *
 * Returns `{ picked, anchor }` with `picked` in PLAN ORDER, always — a set the
 * operator built by clicking about the list must still read top to bottom when it
 * is counted, named or deleted.
 */
export function selectionAfterClick({ picked = [], anchor = null, items = [], id, additive = false, range = false } = {}) {
  const here = indexOf(items, id);
  if (here < 0) return { picked: inPlanOrder(picked, items), anchor };

  if (additive) {
    const has = picked.includes(id);
    const next = has ? picked.filter((n) => n !== id) : [...picked, id];
    return { picked: inPlanOrder(next, items), anchor: id };
  }

  if (range) {
    const from = indexOf(items, anchor);
    if (from < 0) return { picked: [id], anchor: id };
    const [lo, hi] = from <= here ? [from, here] : [here, from];
    return { picked: items.slice(lo, hi + 1).map((i) => i.id), anchor };
  }

  return { picked: [id], anchor: id };
}

/** A picked set, deduplicated and sorted into the order the plan runs. */
export function inPlanOrder(picked, items) {
  const want = new Set(picked ?? []);
  return (items ?? []).filter((i) => want.has(i?.id)).map((i) => i.id);
}

/**
 * WHAT THE INSPECTOR LANDS ON once `removed` is gone.
 *
 * The existing single delete answered `items[0]`, which means deleting cue 12 of
 * 14 throws the operator back to the top of the plan — a scroll and a hunt, every
 * time, on the surface whose whole job is a list you are working down. Deleting
 * something should leave you where you were.
 *
 * The rule is the one a list editor is expected to follow: land on the first cue
 * AFTER the removed run; failing that (the run reached the end) on the last cue
 * BEFORE it; failing that — the whole plan is gone — on nothing.
 *
 * `null` is a real answer and not a failure, which is why this returns it rather
 * than falling back to `items[0]`: an empty plan has no cue to select, and
 * pretending otherwise is what puts a stale id in the inspector.
 *
 * AN EMPTY REMOVAL ALSO ANSWERS `null`, and it means the same thing both times —
 * "there is nothing here for me to decide". It does NOT mean cue 1: answering the
 * top of the plan when nothing was deleted would move an operator who deleted
 * nothing, which is the bug this function exists to fix, fired on the one input
 * where there is no reason to fire at all. The caller keeps the selection it had.
 */
export function selectionAfterRemoval(items, removed) {
  const gone = new Set(removed ?? []);
  if (!gone.size) return null;
  const list = items ?? [];
  const survivors = list.filter((i) => !gone.has(i?.id));
  if (!survivors.length) return null;
  const last = list.reduce((acc, it, i) => (gone.has(it?.id) ? i : acc), -1);
  for (let i = last + 1; i < list.length; i += 1) {
    if (!gone.has(list[i]?.id)) return list[i].id;
  }
  return survivors[survivors.length - 1].id;
}

/**
 * The WORDS on a delete control, and they are the whole "not ambiguous" half.
 *
 * A destructive control has to say what it will take before it takes it, and a
 * count is not enough on its own when the count is one — "Delete" over a list of
 * fourteen cues does not say which. So one cue is NAMED and several are COUNTED,
 * and the armed step restates the same fact rather than replacing it with a bare
 * "Click again", because the second press is the one that actually does it.
 *
 * A label is truncated rather than allowed to run: a cue name is operator-typed
 * or file-derived and can be arbitrarily long, and a button that reflows the
 * running order's footer while somebody is reading it is its own small defect.
 */
export function deleteLabel(count, label = '', armed = false) {
  const n = Math.max(0, Math.trunc(Number(count) || 0));
  if (n === 0) return 'Delete';
  if (n === 1) {
    const name = trimTo(label, 28);
    const what = name ? `“${name}”` : 'this cue';
    return armed ? `Delete ${what} — click again` : `Delete ${what}`;
  }
  return armed ? `Delete ${n} cues — click again` : `Delete ${n} cues`;
}

/** The sentence the pane footer reports once a delete has happened. */
export function deletedMessage(count, label = '') {
  const n = Math.max(0, Math.trunc(Number(count) || 0));
  if (n === 1) {
    const name = trimTo(label, 40);
    return name ? `Deleted “${name}”.` : 'Deleted 1 cue.';
  }
  return `Deleted ${n} cues.`;
}

/** The sentence the ADD panel reports, so one click per row is visibly one cue. */
export function addedMessage(label = '') {
  const name = trimTo(label, 40);
  return name ? `Added “${name}”.` : 'Added 1 cue.';
}

/** `n` characters, then an ellipsis. Never a hard cut mid-word if it can help it. */
function trimTo(text, n) {
  const s = String(text ?? '').trim();
  if (s.length <= n) return s;
  const cut = s.slice(0, n);
  const space = cut.lastIndexOf(' ');
  return `${(space > n * 0.6 ? cut.slice(0, space) : cut).trimEnd()}…`;
}

/**
 * WHERE A DUPLICATE GOES — beside the cue it was copied from.
 *
 * `duplicateCue` has always said it copies "a cue in place" and has always
 * appended to the end of the plan. On a fourteen-cue running order that is a cue
 * that appears somewhere the operator is not looking, in whatever section
 * happens to be last, and then has to be dragged back up past everything.
 *
 * `add_plan_item` appends and there is no command that inserts, so the fix is the
 * reorder that already exists: add, then hand `reorder_plan` the order with the
 * new id lifted out of the tail and dropped in after its original.
 *
 * SECTION MEMBERSHIP COMES OUT RIGHT BY CONSTRUCTION, which is the part worth
 * noticing. `add_plan_item` takes no `section_title`, so a duplicate has none —
 * and `sectionsOf` reads an empty title as "still in the section above". Landing
 * it after its original therefore puts it in its original's section; landing it at
 * the end put it in whatever section the plan happened to finish in.
 *
 * `ids` is the plan's order AFTER the add (so `newId` is in it, at the end).
 * Returns the order to persist, or the list unchanged when either id is missing —
 * a reorder built from a guess is worse than no reorder.
 */
export function orderWithDuplicateInPlace(ids, sourceId, newId) {
  const list = (ids ?? []).slice();
  const from = list.indexOf(newId);
  const at = list.indexOf(sourceId);
  if (from < 0 || at < 0 || sourceId === newId) return list;
  list.splice(from, 1);
  const to = list.indexOf(sourceId);
  list.splice(to + 1, 0, newId);
  return list;
}
