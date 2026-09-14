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

/**
 * Move one object one step through the order that DECIDES WHAT IS DRAWN ON TOP.
 *
 * `dir` is +1 toward the front and -1 toward the back, which is the direction
 * the layer list's two arrows mean.
 *
 * ── WHY THIS IS NOT A SWAP OF TWO ADJACENT ARRAY ENTRIES ──────────────────
 *
 * It was, in the editor, and a band made that a control that changes nothing.
 * A band's words live in the same flat array as everything else and are NOT
 * drawn from it: `topLevelLayers` skips them and the band draws them itself, in
 * the order it names them (`members`). So swapping a shape with a word moved a
 * word that nothing reads past, and the wall was identical. On a `lowerBible`
 * template — a band and its two words, which is the whole template — the arrows
 * did nothing at all, in either direction, for ever. Add a shape to it and one
 * step back took three presses, two of which looked broken.
 *
 * Two orders, so two moves, decided here rather than at the button:
 *   · a TOP-LEVEL object steps past the next top-level object, skipping any
 *     words parked between them in the array;
 *   · a WORD steps within its own band, which is the order the band draws it
 *     in — and it may not step OUT of the band, because leaving a band is what
 *     the "In band" control is for and it is not something an arrow may do by
 *     accident.
 *
 * Returns the SAME list (by reference) when nothing can move, so a caller can
 * tell "already at the end" from "moved" without comparing contents.
 */
export function moveLayer(layers, id, dir) {
  const list = Array.isArray(layers) ? layers : [];
  const step = Number(dir) > 0 ? 1 : -1;
  if (!list.some((l) => l && l.id === id)) return list;

  const band = bandOf(list, id);
  if (band) {
    const members = Array.isArray(band.members) ? band.members : [];
    const at = members.indexOf(id);
    const to = at + step;
    if (at < 0 || to < 0 || to >= members.length) return list;
    const next = [...members];
    [next[at], next[to]] = [next[to], next[at]];
    return list.map((l) => (l && l.id === band.id ? { ...l, members: next } : l));
  }

  // THE STACK, which is the list with every band's words taken out of it.
  const stack = list.filter((l) => l && !bandOf(list, l.id));
  const p = stack.findIndex((l) => l.id === id);
  const q = p + step;
  if (p < 0 || q < 0 || q >= stack.length) return list;
  const moved = [...stack];
  [moved[p], moved[q]] = [moved[q], moved[p]];

  // Rebuilt as BLOCKS — each object, then (for a band) the words it names, in
  // the order it names them. A band that moved would otherwise leave its words
  // behind at the index they happened to hold: nothing would render wrongly,
  // because a band draws its words wherever they sit, but the saved JSON would
  // stop being readable by the next person who opens it. `seen` is belt and
  // braces against a stale id in `members` — the one thing that list is allowed
  // to hold (`bandMembers`).
  const seen = new Set();
  const out = [];
  const push = (l) => { if (l && !seen.has(l.id)) { seen.add(l.id); out.push(l); } };
  for (const L of moved) {
    push(L);
    if (L.type !== 'band') continue;
    for (const mid of Array.isArray(L.members) ? L.members : []) {
      push(list.find((x) => x && x.id === mid));
    }
  }
  return out;
}
