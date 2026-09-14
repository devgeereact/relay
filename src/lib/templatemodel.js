/**
 * THE TEMPLATE MODEL — one property, one home.
 *
 * A template stores only what it has changed; this module fills the rest, and
 * owns the three things that used to be spread across the renderer, the editor
 * and the theme layer:
 *
 *   1. `migrateStyle` — the legacy WHOLE-TEMPLATE keys are written onto the
 *      elements that need them and then DELETED.
 *   2. `resolveStyle` — every per-element default, in one place, so the editor's
 *      preview and the wall cannot disagree about what "unset" looks like.
 *   3. `slideBG` — the background styles, as a register rather than as five
 *      branches of a template literal.
 *
 * And one estimator, `fitScale`, for the boxes the DOM fitter cannot measure.
 *
 * WHY THE MIGRATION DELETES. `style.font` was a whole-template font that
 * `verseFont` and `refFont` fell back to. Both homes were real, both were
 * written by different controls, and the editor showed one while saving the
 * other. Nothing crashed; the preview and the output simply stopped agreeing,
 * and the first person to notice was in a congregation. A fallback chain is a
 * second home wearing a helpful name.
 *
 * `TemplateRender.svelte` is still THE one renderer (CLAUDE.md) — this module
 * does not draw anything. It decides what the renderer is drawing FROM.
 */

/**
 * The whole-template keys this model no longer has, and the per-element keys
 * each one becomes. A key here is deleted by `migrateStyle` once its elements
 * carry the value.
 */
const LEGACY_MAP = {
  font: ['verseFont', 'refFont'],
  textShadow: ['verseShadow', 'refShadow'],
};

/** The legacy keys, for anything that wants to assert they are gone. */
export const LEGACY_STYLE_KEYS = Object.keys(LEGACY_MAP);

/**
 * The three transition names the old picker wrote, mapped onto the register in
 * `transitions.js`.
 *
 * A saved theme holds whichever of these its operator chose. Dropping them would
 * turn every one of those themes into a cut — losing a choice somebody made,
 * silently, while looking like the upgrade worked.
 */
const LEGACY_TRANSITIONS = { fade: 'crossfade', slide: 'slideup', zoom: 'materialise' };

/**
 * Every per-element default the renderer used to inline.
 *
 * These are the values that shipped, not an improvement on them: changing one
 * here repaints every template that never set it, which is most of them.
 */
export const STYLE_DEFAULTS = Object.freeze({
  bgStyle: 'solid',
  verseSize: 6,
  refSize: 2.6,
  verseLineHeight: 1.32,
  refGap: 1.4,
  verseTransform: 'none',
  refTransform: 'none',
  verseShadow: 0,
  refShadow: 0,
  verseFont: 'var(--f-serif)',
  refFont: 'var(--f-serif)',
  bgOpacity: 1,
  bgDim: 0,
  panelRadius: 1.4,
});

/** A number a person may have typed, or nothing at all. `''` is nothing. */
function num(v, fallback) {
  if (v === null || v === undefined || v === '') return fallback;
  const n = Number(v);
  return Number.isFinite(n) ? n : fallback;
}

/**
 * A SIZE, which is a number that may not be zero.
 *
 * The renderer used to read `parseFloat(style.verseSize) || 6`, and the `||`
 * was doing two jobs: it filled in an absent size, and it quietly rejected a
 * zero. Replacing it with an honest "is this a number" check kept the first job
 * and dropped the second — a template storing `0`, from an import or an old
 * editor, would have rendered a verse at 0cqw. That is an invisible verse on a
 * wall, with nothing on screen and no error anywhere.
 */
function size(v, fallback) {
  const n = num(v, fallback);
  return n > 0 ? n : fallback;
}

/**
 * Write the legacy whole-template keys onto their elements and remove them.
 *
 * Pure, and idempotent: running it on a migrated style returns the same style,
 * which is what makes it safe on every load rather than once in a script nobody
 * remembers to run.
 */
export function migrateStyle(style) {
  const out = { ...(style || {}) };
  for (const [legacy, targets] of Object.entries(LEGACY_MAP)) {
    if (!(legacy in out)) continue;
    const value = out[legacy];
    for (const t of targets) {
      // `??`, not `||`: a shadow of 0 and an alignment of '' are choices, and a
      // choice must not be overwritten by the value it replaced.
      if (out[t] === undefined || out[t] === null) out[t] = value;
    }
    delete out[legacy];
  }
  if (out.transition in LEGACY_TRANSITIONS) out.transition = LEGACY_TRANSITIONS[out.transition];
  return out;
}

/** The same, for a whole template record. Everything but `style` is untouched. */
export function migrateTemplate(t) {
  if (!t) return t;
  return { ...t, style: migrateStyle(t.style) };
}

/**
 * A complete style: migrated, then every unset property filled from
 * `STYLE_DEFAULTS`. What the renderer reads.
 */
export function resolveStyle(style) {
  const s = migrateStyle(style);
  const out = { ...s };
  out.verseSize = size(s.verseSize, STYLE_DEFAULTS.verseSize);
  out.refSize = size(s.refSize, STYLE_DEFAULTS.refSize);
  out.verseLineHeight = size(s.verseLineHeight, STYLE_DEFAULTS.verseLineHeight);
  out.refGap = num(s.refGap, STYLE_DEFAULTS.refGap);
  out.bgOpacity = num(s.bgOpacity, STYLE_DEFAULTS.bgOpacity);
  out.bgDim = num(s.bgDim, STYLE_DEFAULTS.bgDim);
  out.panelRadius = num(s.panelRadius, STYLE_DEFAULTS.panelRadius);
  out.verseShadow = num(s.verseShadow, STYLE_DEFAULTS.verseShadow);
  out.refShadow = num(s.refShadow, STYLE_DEFAULTS.refShadow);
  // `background`, `verseAlign` and `refAlign` are deliberately absent from the
  // defaults: an unset background means TRANSPARENT (a lower third is keyed over
  // a camera), and alignment falls back to `layout.align` before it falls back
  // to centre. Filling either here would answer a question the renderer has a
  // better answer to.
  for (const k of ['bgStyle', 'verseTransform', 'refTransform', 'verseFont', 'refFont']) {
    if (out[k] === undefined || out[k] === null || out[k] === '') out[k] = STYLE_DEFAULTS[k];
  }
  return out;
}

/**
 * THE BACKGROUND STYLES. Five, named once, so the editor's picker and the
 * renderer cannot offer different sets.
 */
export const BG_STYLES = Object.freeze([
  { id: 'solid', label: 'Solid' },
  { id: 'vfade', label: 'Vertical fade' },
  { id: 'glow', label: 'Centre glow' },
  { id: 'diagonal', label: 'Diagonal' },
  { id: 'vignette', label: 'Vignette' },
]);

/** `#rgb` / `#rrggbb` → `r, g, b`. Anything else is left for the caller to reject. */
function rgbOf(hex) {
  const h = String(hex || '').replace('#', '');
  const n = h.length === 3 ? h.split('').map((c) => c + c).join('') : h;
  if (!/^[0-9a-fA-F]{6}$/.test(n)) return null;
  return [0, 2, 4].map((i) => parseInt(n.slice(i, i + 2), 16)).join(', ');
}

/**
 * The CSS a slide's background paints, or `null` when the template names none.
 *
 * NULL IS NOT A MISSING DEFAULT. A template with no background is transparent,
 * and transparency is what makes a lower-third channel keyable over a live
 * camera in OBS or an ATEM. A resolver that helpfully filled in a dark colour
 * here would black out a stream for every template that never picked one.
 *
 * `background` has always been allowed to hold raw CSS (a gradient an operator
 * pasted, a `var()`), so anything that is not a plain hex is passed through
 * untouched — wrapping it in another gradient produces an invalid value, and an
 * invalid background paints nothing at all.
 */
export function slideBG(style) {
  const s = style || {};
  const colour = s.background;
  if (!colour) return null;
  const rgb = rgbOf(colour);
  if (!rgb) return colour;
  const id = s.bgStyle || STYLE_DEFAULTS.bgStyle;
  switch (id) {
    case 'vfade':
      return `linear-gradient(180deg, rgba(${rgb}, 1) 0%, rgba(${rgb}, 0.72) 55%, rgba(${rgb}, 0.95) 100%)`;
    case 'glow':
      return `radial-gradient(ellipse at 50% 45%, rgba(${rgb}, 0.55) 0%, rgba(${rgb}, 1) 70%)`;
    case 'diagonal':
      return `linear-gradient(135deg, rgba(${rgb}, 1) 0%, rgba(${rgb}, 0.68) 100%)`;
    case 'vignette':
      return `radial-gradient(ellipse at 50% 50%, rgba(${rgb}, 1) 45%, rgba(0, 0, 0, 0.55) 100%), ${colour}`;
    case 'solid':
    default:
      // An unknown style is a template from a newer version, or a typo in an
      // imported file. Solid is the honest answer: the colour the template asked
      // for, with none of the treatment it named.
      return colour;
  }
}

/**
 * How wide one character is, as a share of the font size. Measured off the three
 * faces Relay ships (docs/REBRAND.md §3.4).
 */
export const FACE_ADVANCE = Object.freeze({ mono: 0.62, serif: 0.49, sans: 0.52 });

/** Which of the three a family name belongs to. Serif is the renderer's default. */
export function faceOf(family) {
  const f = String(family || '').toLowerCase();
  if (!f) return 'serif';
  if (f.includes('mono')) return 'mono';
  if (f.includes('serif') || f.includes('fraunces') || f.includes('playfair') ||
      f.includes('georgia') || f.includes('times') || f.includes('garamond') ||
      f.includes('baskerville') || f.includes('palatino') || f.includes('didot') ||
      f.includes('cambria')) return 'serif';
  return 'sans';
}

/**
 * THE SHRINK CURVE, and the floor under it. One home, because the estimate here
 * and the measured loop in `TemplateRender.svelte` have to agree: an estimate
 * that shrinks on a different curve from the measurement disagrees with it on
 * exactly the passages that matter.
 *
 * WHY THE BOUND IS A SCALE AND NOT A ROUND COUNT. Both loops used to stop after
 * 40 rounds of ×0.95. 0.95^40 is 0.1285, so a box that needed 0.099 got 0.1285 —
 * and the loop returned it, having never fitted anything. The text is inside an
 * `overflow: hidden` box, so what a congregation saw was a verse with its top and
 * bottom lines sliced through the middle: rule 42's harm, reached by running out
 * of rounds rather than by measuring the wrong face.
 *
 * It is not a long-passage problem. A short line at a large designed size in a
 * shallow box (a band, a stage zone) needs a scale below 0.1285 to fit one line,
 * and that is an ordinary template, not a pathological one.
 *
 * A count cannot express "small enough"; a scale can. The curve is unchanged, so
 * anything that fits today lands on exactly the same value — the loop simply no
 * longer stops before it has an answer. `FIT_MIN_SCALE` is 1% of the size the
 * template's designer asked for: at 6cqw on a 1920px wall that is about one
 * pixel, the last size above nothing. It is a floor on the ARITHMETIC and not on
 * legibility — rule 37's 45% floor is a separate line, and it REPORTS rather
 * than stops.
 */
export const FIT_STEP = 0.95;
export const FIT_MIN_SCALE = 0.01;

/**
 * Should the fit shrink again? Pure, and exported, for the same reason
 * `needsRefit` is: both the estimate and the measured loop on the wall ask this
 * question, and they must not answer it differently.
 *
 * @param overflowing does the block still not fit?
 * @param scale       the scale about to be stepped down from.
 * @param min         the floor (see `FIT_MIN_SCALE`).
 */
export function keepShrinking({ overflowing, scale, min = FIT_MIN_SCALE }) {
  return !!overflowing && scale * FIT_STEP >= min;
}

/**
 * How many lines this text takes at this size, in a box `widthPct` of the
 * output's width.
 *
 * Everything is in cqw — a share of the output's WIDTH — so the frame is 100
 * wide by definition and its height follows from the aspect. That is what lets
 * one estimator answer for a full frame, a lower-third band, a stage reading and
 * a SuperSource word region without any of them being a special case.
 */
export function estimateLines({ text, size, face = 'serif', widthPct = 100 }) {
  const chars = String(text || '').length;
  if (!chars) return 0;
  const sz = num(size, STYLE_DEFAULTS.verseSize);
  const advance = FACE_ADVANCE[face] || FACE_ADVANCE.serif;
  const perLine = Math.max(1, Math.floor((100 * (widthPct / 100)) / (sz * advance)));
  return Math.ceil(chars / perLine);
}

/**
 * The scale this text needs to fit its box, 0–1.
 *
 * It SHRINKS and it never returns zero. Refusing to render is not this
 * function's call and an invisible verse is worse than a small one — the floor
 * is reported, not enforced (CLAUDE.md rule 37). The caller decides what to say
 * about a scale that went too far.
 *
 * The ×0.95 loop mirrors the DOM fitter in `TemplateRender.svelte` step for
 * step, deliberately: an estimate that shrinks on a different curve from the
 * measurement would disagree with it on exactly the passages that matter. Both
 * use `FIT_STEP` and `keepShrinking`, so there is one curve and one floor.
 *
 * THE THREE BOX ARGUMENTS ARE ONE DESCRIPTION, AND MIXING THEM IS A BUG.
 * `aspect` is the CONTAINER's aspect — the thing `cqw` is a share of. `widthPct`
 * and `heightPct` are the text box's share of that container, in each dimension.
 * Together they say: the container is 100 wide and `100/aspect` tall (a 16:9
 * frame is 56.25cqw tall), and the box is `widthPct`% and `heightPct`% of that.
 * Passing the BOX's own aspect while leaving the two shares at 100 describes a
 * container the size of the box, which over-states the room available by exactly
 * the ratio between them.
 */
export function fitScale({ text, size, face = 'serif', aspect = 16 / 9, widthPct = 100, heightPct = 100, lineHeight = STYLE_DEFAULTS.verseLineHeight }) {
  const ratio = num(aspect, 16 / 9);
  const a = ratio > 0 ? ratio : 16 / 9;
  const boxWidth = num(widthPct, 100);
  const boxHeight = (100 / a) * (num(heightPct, 100) / 100);
  const lh = num(lineHeight, STYLE_DEFAULTS.verseLineHeight);
  const base = num(size, STYLE_DEFAULTS.verseSize);
  const overflowsAt = (s) => {
    const sz = base * s;
    return estimateLines({ text, size: sz, face, widthPct: boxWidth }) * sz * lh > boxHeight;
  };
  let scale = 1;
  while (keepShrinking({ overflowing: overflowsAt(scale), scale })) scale *= FIT_STEP;
  return scale;
}

/**
 * ══ THE BAND, AND THE GROUND IT GIVES ══ (docs/REBRAND.md §4)
 *
 * A lower-third band is a real element that runs from `top%` to the BOTTOM of
 * the frame, inset by the side safe area, with the baseline lift as its bottom
 * padding and its content centred in what is left. Its children are named by
 * the band itself (`members`), which is the part that matters:
 *
 * WHY MEMBERSHIP IS DECLARED AND NOT INFERRED. "The band gives ground before
 * the words do" needs a band to know which words are its own. The cheap way to
 * get that is to look at the objects near it, or to fire the rule when a shape
 * is called `Band` — a coupling that is invisible in the data, applies to some
 * templates and not others, and breaks the moment somebody renames an object.
 * `members` is an explicit list of ids on the band: it is in the file, it
 * survives a rename, an object that is in no band is in no band, and the rule
 * is off for every template that never opted in. That is the concept phase 5
 * was waiting for, and it costs one array.
 *
 * THE RULE, exactly as §4 states it: the band grows UPWARD by at most
 * `BAND_GROW_MAX` points, never past `BAND_MAX_SHARE` of the frame, and only
 * for as long as the type would otherwise be smaller than `BAND_TYPE_FLOOR` of
 * the size its designer asked for. A short name needs no shrinking at all, so
 * the loop never runs and the band does not move — which is the half of the
 * rule that is easy to lose, because a band that grows for everything is a band
 * that has simply been redesigned taller.
 */
export const BAND_GROW_MAX = 16;
export const BAND_MAX_SHARE = 1 / 3;
export const BAND_TYPE_FLOOR = 0.78;

/**
 * A band's geometry, in percent of the frame. `top` runs to the bottom edge, so
 * the height follows from it; `side` is taken off BOTH edges and `pad` is the
 * inner gutter between the band and its words.
 *
 * Exported because the renderer draws from it and the estimator measures from
 * it, and a band whose drawn width and measured width disagree is rule 42 in a
 * different costume — a fit against a box that is not the box.
 */
export function bandBox(band) {
  const b = band || {};
  const top = Math.max(0, Math.min(100, num(b.top, 74)));
  const side = Math.max(0, Math.min(49, num(b.side, 6)));
  const pad = Math.max(0, num(b.pad, 3));
  const lift = Math.max(0, num(b.lift, 3));
  return { top, side, pad, lift, height: 100 - top, textWidth: 100 - 2 * side - 2 * pad };
}

/**
 * How far up this band has to climb for its words, and what that does to the
 * room each of them gets.
 *
 * `members` are `{ text, size, face, h }` — `h` being the share of the FRAME's
 * height that member's box was designed to take, which is how the three
 * starters are written and what keeps a band that does not grow pixel-identical
 * to the one that shipped.
 *
 * Returns `{ top, scale }`: where the band now starts, and the factor its
 * members' boxes grew by. `scale` is 1 whenever the band did not move, so the
 * no-growth path is arithmetically a no-op rather than a rounded-off
 * approximation of one.
 *
 * The estimate seeds the DOM, it does not replace it: `TemplateRender` still
 * MEASURES the type inside whatever box this produces (CLAUDE.md rule 37 and
 * rule 42 are both untouched). What this decides is the size of the box.
 */
export function bandFit({ band, members = [], aspect = 16 / 9, grow } = {}) {
  const box = bandBox(band);
  // HOW FAR IT MAY CLIMB IS THE BAND'S OWN PROPERTY, read here and nowhere else.
  // It was a parameter with a constant default, which meant a caller that did not
  // pass it silently overrode the band — a second home for `grow`, and the shape
  // of the defect this whole model exists to prevent (one property, one home).
  const climb = Math.max(0, num(grow, num(band?.grow, BAND_GROW_MAX)));
  const designed = box.height - box.lift;
  const list = (Array.isArray(members) ? members : []).filter(Boolean);
  if (designed <= 0 || !list.length) return { top: box.top, scale: 1 };

  // The worst member decides, because the band is one box: giving ground for the
  // line that fits and not for the one that does not would shrink exactly the
  // words the rule exists to protect.
  const worstAt = (scale) =>
    list.reduce((worst, m) => {
      const s = fitScale({
        text: m.text,
        size: m.size,
        face: m.face,
        aspect,
        widthPct: box.textWidth,
        heightPct: Math.max(0.01, num(m.h, 0) * scale),
      });
      return Math.min(worst, s);
    }, 1);

  // A band may not climb past a third of the frame, and may not climb past the
  // points it was allowed. Both are floors on `top`, so the tighter one wins.
  const byGrow = box.top - climb;
  const byShare = 100 - 100 * BAND_MAX_SHARE;
  const limit = Math.max(byGrow, byShare, 0);

  let top = box.top;
  // A short name fits at full size, so this is already true and the band stays
  // exactly where its designer put it.
  while (worstAt((100 - top - box.lift) / designed) < BAND_TYPE_FLOOR && top - 1 >= limit) top -= 1;
  return { top, scale: (100 - top - box.lift) / designed };
}

/**
 * THE BAND AND ITS WORDS, AS BOXES.
 *
 * Everything the renderer needs to draw a lower third: the rectangle the band
 * paints, and one box per member, all in percent of the frame, all derived — so
 * there is no second copy of the band's geometry to disagree with the first.
 *
 * WHY THE MEMBERS ARE DERIVED AND NOT NESTED. Drawing a member inside the band
 * element would mean a second text path in `TemplateRender.svelte`: one for a
 * layer that is positioned and one for a layer that flows. Two text paths is how
 * a shadow, a transform or a fit fix lands on one kind of layer and not the
 * other, which is the shape of four bugs in this repository. A member is drawn by
 * exactly the same code as every other text layer; the band decides WHERE.
 *
 * CENTRED MEANS EQUAL. The stack of members is placed so the gap above it equals
 * the gap below it, inside the band's content box (the band, less its baseline
 * lift). That is the property §4 measured as 7px and 7px — the pixels depend on
 * the output's size, the equality does not.
 *
 * @param band    the band layer.
 * @param members `{ text, size, face, h }` in the order the band names them; `h`
 *                is the share of the FRAME's height that member was designed to
 *                take.
 * @returns `{ box, top, scale, members: [{ x, y, w, h }] }`.
 */
export function bandLayout({ band, members = [], aspect = 16 / 9 } = {}) {
  const box = bandBox(band);
  const list = (Array.isArray(members) ? members : []).filter(Boolean);
  const { top, scale } = bandFit({ band, members: list, aspect });

  const contentTop = top;
  const contentHeight = Math.max(0, 100 - top - box.lift);
  const heights = list.map((m) => Math.max(0, num(m.h, 0)) * scale);
  const stack = heights.reduce((a, b) => a + b, 0);
  // Equal above, equal below. A stack taller than the box (which the fit tries
  // hard to prevent, and rule 37 reports when it cannot) starts at the top rather
  // than hanging off both ends.
  let y = contentTop + Math.max(0, contentHeight - stack) / 2;

  const boxes = heights.map((h) => {
    const b = { x: box.side + box.pad, y, w: box.textWidth, h };
    y += h;
    return b;
  });
  return {
    box: { x: box.side, y: top, w: 100 - 2 * box.side, h: 100 - top },
    top,
    scale,
    members: boxes,
  };
}
