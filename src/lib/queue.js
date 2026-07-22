// THE VERSE QUEUE — what the operator lined up, in order.
//
// The reference calls it "VERSES IN QUEUE". It did not exist: Relay had a
// PASSAGE cursor (walk forward from the verse you fired) and a service PLAN
// (built on Tuesday), and nothing in between for "these four, in this order, in
// a minute". That gap is exactly where an operator is under the most pressure —
// the preacher has just named four references in one sentence.
//
// Pure operations on an array, so the ordering rules are testable without a
// window. The store that holds it is at the bottom; the logic above it never
// touches Svelte.

/** Add to the end. Adding something already queued is a no-op, not a duplicate. */
export function enqueue(list, item) {
  if (!item?.reference) return list;
  if (list.some((q) => q.reference === item.reference)) return list;
  return [...list, item];
}

export function dequeue(list, reference) {
  return list.filter((q) => q.reference !== reference);
}

/**
 * Move an item by one place. Out-of-range moves return the list UNCHANGED —
 * an operator dragging the top item up must not see it silently vanish or wrap
 * round to the bottom.
 */
export function move(list, reference, delta) {
  const i = list.findIndex((q) => q.reference === reference);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= list.length) return list;
  const out = [...list];
  [out[i], out[j]] = [out[j], out[i]];
  return out;
}

/** Reorder to an explicit position (drag and drop). Clamped, never wrapping. */
export function reorder(list, from, to) {
  if (from < 0 || from >= list.length) return list;
  const target = Math.max(0, Math.min(list.length - 1, to));
  const out = [...list];
  const [row] = out.splice(from, 1);
  out.splice(target, 0, row);
  return out;
}

/** The next thing to fire, and the queue with it removed. */
export function take(list) {
  if (!list.length) return { item: null, rest: list };
  return { item: list[0], rest: list.slice(1) };
}

export const clear = () => [];
