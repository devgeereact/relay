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
 * measurement would disagree with it on exactly the passages that matter.
 */
export function fitScale({ text, size, face = 'serif', aspect = 16 / 9, widthPct = 100, heightPct = 100, lineHeight = STYLE_DEFAULTS.verseLineHeight }) {
  const ratio = num(aspect, 16 / 9);
  const a = ratio > 0 ? ratio : 16 / 9;
  const boxHeight = (100 / a) * (num(heightPct, 100) / 100);
  const lh = num(lineHeight, STYLE_DEFAULTS.verseLineHeight);
  const base = num(size, STYLE_DEFAULTS.verseSize);
  let scale = 1;
  let guard = 0;
  while (guard < 40) {
    const sz = base * scale;
    const lines = estimateLines({ text, size: sz, face, widthPct });
    if (lines * sz * lh <= boxHeight) break;
    scale *= 0.95;
    guard++;
  }
  return scale;
}
