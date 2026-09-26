/**
 * HOW FAR OFF THE SLIDE AN OBJECT MAY SIT (RG-233).
 *
 * Every geometry path in the editor clamped to 0–100: the drag stopped dead at
 * the edge, the X/Y/W/H fields refused anything outside it, and the nudge keys
 * did the same. That made the two ordinary uses of a bleed impossible — a band
 * or a picture pushed past the edge so no seam can show at any output size, and
 * a word cropped deliberately by the frame.
 *
 * **The bound is generous rather than absent, and the reason is retrieval.** At
 * −100 an object is entirely outside the slide: there is no handle on the canvas
 * to drag it back, and the only way home is to type a number into a panel about
 * a thing nobody can see. Half a slide either way always leaves a corner to
 * grab.
 *
 * `null` for anything that is not a number, so a caller can tell "refused" from
 * "clamped to the edge" — the editor's own `geom` returns early on it rather
 * than writing a 0 nobody asked for.
 */
export const OVERFLOW_MIN = -50;
export const OVERFLOW_MAX = 150;

export function clampGeom(v) {
  const n = Number(v);
  if (v === null || v === undefined || v === '' || !Number.isFinite(n)) return null;
  return Math.max(OVERFLOW_MIN, Math.min(OVERFLOW_MAX, n));
}
