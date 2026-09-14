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
import { makeLayer, bandOf } from './layers.js';

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

  // A BAND CARRIES A LIST OF OTHER OBJECTS, so `deepCopy` is not enough: it
  // copies the ids faithfully, and the copy then points at the ORIGINAL's words.
  // Both bands would lay out the same two text layers, one of them twice, and
  // editing either band would move the other's type. That is this module's
  // opening sentence — a duplicate that shares something with its original
  // renders perfectly — one level up from a nested object.
  if (src.type === 'band') {
    const copies = [];
    copy.members = (Array.isArray(src.members) ? src.members : [])
      .map((mid) => list.find((l) => l && l.id === mid))
      .filter(Boolean)
      .map((m) => {
        const c = deepCopy(m);
        c.id = makeLayer(m.type).id;
        copies.push(c);
        return c.id;
      });
    return [...list.slice(0, i + 1), copy, ...copies, ...list.slice(i + 1)];
  }

  // A MEMBER's copy joins the band its original is in. Dropping it to the top
  // level instead would leave a line of a lower third floating at whatever box
  // it last stored, outside the band that decides where its words sit.
  const band = bandOf(list, id);
  if (band) {
    const at = band.members.indexOf(id);
    const grown = { ...band, members: [...band.members.slice(0, at + 1), copy.id, ...band.members.slice(at + 1)] };
    const out = [...list.slice(0, i + 1), copy, ...list.slice(i + 1)];
    return out.map((l) => (l.id === band.id ? grown : l));
  }
  return [...list.slice(0, i + 1), copy, ...list.slice(i + 1)];
}

/**
 * Remove one object, and every reference to it.
 *
 * A band names its members by id, so deleting a member without telling its band
 * leaves the band pointing at an object that is not there. Nothing crashes —
 * `bandMembers` skips what it cannot find — which is exactly why it would have
 * survived: the template renders correctly and carries a dead id in its saved
 * JSON for ever, until something one day reads the list without skipping.
 *
 * Deleting a BAND deletes only the band. Its words are released to the boxes
 * they store, which is where their designer put them; removing two more objects
 * than the one an operator armed is not a thing a two-step confirm can mean.
 */
export function removeLayer(layers, id) {
  const list = Array.isArray(layers) ? layers : [];
  if (!list.some((l) => l && l.id === id)) return list;
  return list
    .filter((l) => l.id !== id)
    .map((l) =>
      l.type === 'band' && Array.isArray(l.members) && l.members.includes(id)
        ? { ...l, members: l.members.filter((m) => m !== id) }
        : l,
    );
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
  // A band's own geometry is `top`/`side`/`pad`/`lift`, not x/y/w/h, and its
  // members are what it SHOWS — the same reason `bind` survives above. Resetting
  // a band's look must not move it or empty it; the fill, the alpha, the corner
  // and how much ground it may give are the look, and those do go back.
  if (src.type === 'band') {
    for (const k of ['top', 'side', 'pad', 'lift']) if (src[k] !== undefined) out[k] = src[k];
    out.members = Array.isArray(src.members) ? [...src.members] : [];
  }
  return [...list.slice(0, i), out, ...list.slice(i + 1)];
}
