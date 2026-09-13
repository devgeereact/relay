/**
 * What the object inspector's buttons do to a slide's list of objects.
 *
 * These are pure list operations, kept out of the editor component for one
 * reason: the mistakes they can make are invisible on screen. A "duplicate" that
 * shares a nested object with its original renders perfectly and then moves both
 * layers whenever one is edited; a "reset" that also resets geometry throws away
 * the placement somebody did by hand and looks like a bug in the canvas.
 *
 * `makeLayer` is the one starter register (`layers.js`), so a reset restores the
 * same defaults a new object is created with — there is no second table of "what
 * this kind should look like" to drift from it.
 */
import { makeLayer } from './layers.js';

/** A copy that shares nothing with its original. */
function deepCopy(v) {
  // structuredClone is available in every runtime this ships on (a Tauri
  // webview, Node 18+ under vitest) and, unlike a JSON round trip, it does not
  // quietly turn a Date or an undefined into something else.
  return typeof structuredClone === 'function'
    ? structuredClone(v)
    : JSON.parse(JSON.stringify(v));
}

/** Nudge a duplicate off its original without pushing it off the canvas. */
const OFFSET = 2;
const nudge = (pos, extent) => Math.max(0, Math.min(pos + OFFSET, 100 - extent));

/**
 * Duplicate one object, placing the copy directly in front of it.
 *
 * Returns a new list; the original list and its layers are untouched. An id that
 * is not in the list is not an error — a stale selection is an ordinary thing
 * for a panel to hold — so the list comes back unchanged.
 */
export function duplicateLayer(layers, id) {
  const list = Array.isArray(layers) ? layers : [];
  const i = list.findIndex((l) => l && l.id === id);
  if (i < 0) return list;
  const src = list[i];
  const copy = deepCopy(src);
  copy.id = makeLayer(src.type).id;
  copy.x = nudge(Number(src.x) || 0, Number(src.w) || 0);
  copy.y = nudge(Number(src.y) || 0, Number(src.h) || 0);
  if (src.name) copy.name = `${src.name} copy`;
  return [...list.slice(0, i + 1), copy, ...list.slice(i + 1)];
}

/**
 * Put one object's LOOK back to the starter for its kind.
 *
 * Four things survive, because none of them is a look:
 *   - where it sits (x/y/w/h) — somebody placed that by hand;
 *   - what it is (id, type) and what it shows (bind, name);
 *   - whether it is hidden or locked. Resetting the look of a hidden object must
 *     not put it back on a wall.
 */
export function resetLayer(layers, id) {
  const list = Array.isArray(layers) ? layers : [];
  const i = list.findIndex((l) => l && l.id === id);
  if (i < 0) return list;
  const src = list[i];
  const fresh = makeLayer(src.type);
  const out = {
    ...fresh,
    id: src.id,
    type: src.type,
    x: src.x,
    y: src.y,
    w: src.w,
    h: src.h,
    visible: src.visible,
    locked: src.locked,
  };
  if (src.bind !== undefined) out.bind = src.bind;
  if (src.name !== undefined) out.name = src.name;
  return [...list.slice(0, i), out, ...list.slice(i + 1)];
}
