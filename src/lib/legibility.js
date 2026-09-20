// LEGIBILITY — can the back row read this?
//
// Single responsibility: given a template's colours and sizes, plus two numbers
// about the room, say whether the words will be readable — and say plainly when it
// cannot tell.
//
// ── Two questions, and only one of them is about colour ──────────────────────
//
// 1. **Contrast.** Computable, exactly, for a solid background: WCAG's relative
//    luminance is arithmetic. Not computable at all over an image or a video,
//    because the luminance under the words depends on the pixel, the frame and the
//    scene. Relay says "cannot be checked" there rather than guessing, because a
//    green tick over an unreadable verse is worse than no tick — it is the same
//    class of harm as a status badge that cannot detect its own failure.
//
// 2. **Size at distance.** Nothing in software can know how big a projected image
//    is or how far away the back row sits. So Relay asks for those two numbers,
//    remembers them with the room (RG-10), and does the arithmetic. It never
//    invents them: with no numbers there is no verdict.
//
// ── The thresholds are not verified, and this file says so ───────────────────
//
// The ratios below come from WCAG, which is a specification for SCREENS AT ARM'S
// LENGTH, and the character-height rule comes from broadcast safe-title practice.
// **Neither has been checked against a projector in a church**, because that is
// Stage B of the human test script and it has not been run. They are the best
// available reference points and they are reported as guidance, never as a pass
// mark — and `verdict` carries that caveat rather than leaving it in a doc.

import { templateShows } from './layers.js';

/** Parse `#rgb`, `#rrggbb`, or `rgb()/rgba()`. Returns null for anything else. */
export function parseColor(c) {
  if (typeof c !== 'string') return null;
  const s = c.trim().toLowerCase();

  const hex = s.match(/^#([0-9a-f]{3}|[0-9a-f]{6})$/);
  if (hex) {
    const h = hex[1];
    const n =
      h.length === 3
        ? h.split('').map((ch) => parseInt(ch + ch, 16))
        : [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16));
    return { r: n[0], g: n[1], b: n[2], a: 1 };
  }

  const rgb = s.match(/^rgba?\(([^)]+)\)$/);
  if (rgb) {
    const parts = rgb[1].split(/[,\s/]+/).filter(Boolean).map(Number);
    if (parts.length < 3 || parts.slice(0, 3).some(Number.isNaN)) return null;
    return { r: parts[0], g: parts[1], b: parts[2], a: parts[3] ?? 1 };
  }
  // A gradient, a CSS variable, a named colour, `transparent`. Each is a real
  // answer — "I cannot compute this" — and is returned as one.
  return null;
}

/** WCAG relative luminance. */
export function luminance({ r, g, b }) {
  const f = (v) => {
    const x = v / 255;
    return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * f(r) + 0.7152 * f(g) + 0.0722 * f(b);
}

/** WCAG contrast ratio, 1–21. */
export function contrastRatio(fg, bg) {
  const a = luminance(fg);
  const b = luminance(bg);
  const [hi, lo] = a > b ? [a, b] : [b, a];
  return (hi + 0.05) / (lo + 0.05);
}

/** Composite `over` (with alpha) onto `under`. */
export function over(front, back) {
  const a = front.a ?? 1;
  return {
    r: front.r * a + back.r * (1 - a),
    g: front.g * a + back.g * (1 - a),
    b: front.b * a + back.b * (1 - a),
    a: 1,
  };
}

/**
 * What is actually behind the words, as a colour — or null when it cannot be known.
 *
 * The layers, in order: the template background, then the dim scrim (`bgDim`, black
 * at that opacity), then a contrast plate if the template has one. A media URL wins
 * over all of it and makes the answer unknowable.
 */
export function effectiveBackground(style = {}, content = null) {
  if (content?.media_url) return null; // a photograph or a video: unknowable
  const base = parseColor(style.background);
  if (!base) return null; // a gradient or a variable: unknowable
  let bg = base;

  const dim = Math.min(1, Math.max(0, Number(style.bgDim) || 0));
  if (dim > 0) bg = over({ r: 0, g: 0, b: 0, a: dim }, bg);

  const plate = parseColor(style.plateColor);
  if (plate) {
    const opacity = Number(style.plateOpacity);
    bg = over({ ...plate, a: Number.isFinite(opacity) ? opacity : (plate.a ?? 1) }, bg);
  }
  return bg;
}

/**
 * WCAG's thresholds, for reference.
 *
 * 4.5 is AA for body text; 3.0 is AA for LARGE text, which every congregation
 * template is by definition. So 3.0 is the floor Relay reports against and 4.5 is
 * the comfortable mark — a verse on a wall is enormous text, and holding it to the
 * body-text ratio would flag designs that read perfectly well.
 */
export const CONTRAST_FLOOR = 3;
export const CONTRAST_GOOD = 4.5;

/**
 * Check one text colour against its background.
 *
 * Returns `{ ratio, state, note }` where `state` is `ok` · `low` · `unknown`.
 * **`unknown` is never quietly treated as a pass**, and it is the honest answer
 * over an image, a gradient or a CSS variable.
 */
export function checkContrast(style = {}, content = null, which = 'verse') {
  const fg = parseColor(which === 'verse' ? style.verseColor : style.refColor);
  const bg = effectiveBackground(style, content);
  if (!fg || !bg) {
    return {
      ratio: null,
      state: 'unknown',
      note: content?.media_url
        ? 'There is a picture or a video behind the words, so Relay cannot work out the contrast — only your eyes can.'
        : 'Relay cannot read these colours (a gradient, or a colour set elsewhere), so it cannot check the contrast.',
    };
  }
  const ratio = contrastRatio(fg, bg);
  if (ratio < CONTRAST_FLOOR)
    return {
      ratio,
      state: 'low',
      note: `The text is close in brightness to what is behind it (${ratio.toFixed(1)}:1). On a projector in a lit room this is likely to be hard to read.`,
    };
  return {
    ratio,
    state: 'ok',
    note:
      ratio < CONTRAST_GOOD
        ? `Readable at this size (${ratio.toFixed(1)}:1), though not by much.`
        : `Good contrast (${ratio.toFixed(1)}:1).`,
  };
}

/**
 * ══ THE COUNTDOWN'S LAST MINUTE, WHICH NO VERDICT HERE COULD SEE ════════════
 *
 * These checks review the colours a TEMPLATE declares. A countdown in its last
 * minute is painted in a red the template never declared and never chose:
 * `TemplateRender`'s `CD_WARN`, hard-coded, applied over whatever ground the
 * template happens to have. So the one tool in the product that answers "can the
 * back row read this" was silent about the most time-critical thing Relay puts on
 * a wall — and silent in the reassuring direction, which is rule 35's own shape.
 *
 * A red background is not hypothetical: the warning red on a deep red or a mid
 * grey is a countdown that simply stops being readable at the moment it matters
 * most, and nothing anywhere would have said so.
 *
 * DISTANCE needs nothing, and that is worth stating rather than leaving as an
 * omission. The countdown paints at TWICE the declared verse size
 * (`TemplateRender` renders `.countdown` at `verseSize * 2`), so the verse is
 * always the binding constraint and `checkDistance`'s verdict is already
 * conservative for the digits. Adding a second, easier distance row would only
 * teach an operator that this panel says yes a lot.
 *
 * The hex is stated here because a stylesheet cannot import and this module
 * cannot read one; `countdowntype.test.js` holds it equal to the renderer's own
 * `CD_WARN` and to the CSS rule, in the same way `countdownwarnmotion.test.js`
 * already holds the four surfaces to one red.
 */
export const COUNTDOWN_WARN_COLOR = '#f4515b';

/** The countdown paints at twice the declared verse size. One home for the 2. */
export const COUNTDOWN_SIZE_MULTIPLIER = 2;

/**
 * Can the last minute of a countdown be read on this template's ground?
 *
 * Deliberately NOT a second set of rules: it is `checkContrast` with the warning
 * red substituted for the verse colour, so the thresholds, the three states and
 * the `unknown` honesty are the same ones everything else here uses. Only the
 * wording says which question was asked.
 */
export function checkCountdownWarn(style = {}, content = null) {
  const c = checkContrast({ ...style, verseColor: COUNTDOWN_WARN_COLOR }, content, 'verse');
  if (c.state === 'unknown') return c;
  return {
    ...c,
    note:
      c.state === 'low'
        ? `In its last minute the countdown turns red, and that red is close in brightness to what is behind it (${c.ratio.toFixed(1)}:1). That is the moment somebody most needs to read it.`
        : `The last-minute red reads against this background (${c.ratio.toFixed(1)}:1).`,
  };
}

/**
 * How tall the text actually is, and whether the back row can read it.
 *
 * Sizes in Relay are `cqw` — a percentage of the OUTPUT's width — which is what
 * makes a template render identically at any resolution. That also makes this
 * arithmetic possible without knowing the pixel size of anything:
 *
 *     character height (metres) = screenWidthM × (cqw / 100) × capHeightRatio
 *
 * `capHeightRatio` is the fraction of a font's em box its capitals occupy — about
 * 0.7 for the faces Relay ships. It is an approximation and is named as one.
 *
 * The rule of thumb is the broadcast one: text should be at least **1/200th of the
 * viewing distance** to be comfortably readable, which is roughly a 10-minute-of-arc
 * character. It has NOT been checked against a projector in a church.
 */
export const CAP_HEIGHT_RATIO = 0.7;
export const MIN_HEIGHT_PER_DISTANCE = 1 / 200;

export function textHeightMetres(cqw, screenWidthM) {
  const size = Number(cqw);
  const w = Number(screenWidthM);
  if (!Number.isFinite(size) || !Number.isFinite(w) || size <= 0 || w <= 0) return null;
  return w * (size / 100) * CAP_HEIGHT_RATIO;
}

/**
 * @param room `{ screenWidthM, backRowM }` — the two numbers only a person can know.
 * Returns null when either is missing: **no numbers, no verdict.**
 */
export function checkDistance(style = {}, room = {}) {
  const h = textHeightMetres(style.verseSize, room.screenWidthM);
  const d = Number(room.backRowM);
  if (h === null || !Number.isFinite(d) || d <= 0) {
    return {
      state: 'unknown',
      heightM: h,
      note: 'Tell Relay how wide your screen is and how far back the last row sits, and it will work out whether the text is big enough.',
    };
  }
  const needed = d * MIN_HEIGHT_PER_DISTANCE;
  const ok = h >= needed;
  return {
    state: ok ? 'ok' : 'small',
    heightM: h,
    neededM: needed,
    // Centimetres, because a person can picture those. Metres of text height is a
    // number nobody has an instinct for.
    note: ok
      ? `About ${(h * 100).toFixed(0)}cm tall on a ${room.screenWidthM}m screen — readable from ${d}m back.`
      : `About ${(h * 100).toFixed(0)}cm tall, and the back row at ${d}m wants nearer ${(needed * 100).toFixed(0)}cm. Try a larger verse size, or fewer verses at once.`,
  };
}

/**
 * The distances to preview, and the scale each one implies.
 *
 * A preview pane is a fixed number of pixels wide. Showing "how it looks from 15m"
 * means shrinking the render by the ratio of that distance to a reference — which
 * is exactly what a person does when they step back. The reference is the near
 * distance, so the first thumbnail is full size and the rest shrink honestly.
 */
export const PREVIEW_DISTANCES_M = [5, 10, 15, 20];

export function previewScale(distanceM, referenceM = PREVIEW_DISTANCES_M[0]) {
  const d = Number(distanceM);
  const r = Number(referenceM);
  if (!Number.isFinite(d) || !Number.isFinite(r) || d <= 0 || r <= 0) return 1;
  return Math.min(1, r / d);
}

/**
 * Everything, as one verdict for the editor.
 *
 * The caveat rides WITH the verdict rather than living in a document, because the
 * person reading it is deciding whether to trust it right now.
 */
export const CAVEAT =
  'These are reference figures — WCAG contrast, and the broadcast rule of thumb for character height. Neither has been checked against a projector in a real church, so treat a warning as worth looking at rather than as a verdict.';

/**
 * THE LAYER MODEL, AS THE FLAT STYLE THESE CHECKS WERE WRITTEN FOR.
 *
 * `checkContrast` and `checkDistance` read `style.verseColor`, `style.refColor`,
 * `style.background` and `style.verseSize`. In the layer model those live on
 * layers, and every shipped template leaves `style` EMPTY — all eight shelf
 * entries carry `style: {}`.
 *
 * So the one tool in the product that answers "can the back row read this"
 * returned `unknown` for ELEVEN OF THIRTEEN shipped templates, including all
 * three lower thirds and both composites. Worse than inert: `unknown` renders in
 * the panel's reassuring branch, so a template with a real contrast failure sat
 * in the same state as one nobody had checked.
 *
 * This derives the five values from the layers, by `bind` and `type`, so `review`
 * can answer. It is deliberately NOT a second set of rules — the thresholds, the
 * wording and the three-state answer are untouched; only where the numbers come
 * from has changed.
 *
 * Two shapes worth naming:
 *   · a BAND template (a lower third) has no background layer, because the rest of
 *     the frame is a camera Relay does not control. The band's own fill IS the
 *     ground its words sit on, which is exactly what a caption bar is for.
 *   · the reference resolves the way `TemplateRender` resolves it — its own
 *     colour, then the band's verse colour on a keyed template, then the accent.
 *     Reading `style.refColor` alone is why the reference was `unknown` on all
 *     thirteen, including the two the panel could otherwise answer for.
 *
 * A region/composite template is left alone: its words are a real inner template
 * at a different width, so answering from the outer one would be a guess with a
 * number on it.
 */
export function styleOfTemplate(template = {}) {
  const layout = template.layout ?? {};
  const layers = Array.isArray(layout.layers) ? layers_(layout.layers) : null;
  if (!layers) return template.style ?? {};

  const byBind = (b) => layers.find((L) => L.bind === b && L.type === 'text');
  const byType = (t) => layers.find((L) => L.type === t);

  const verse = byBind('verse');
  const ref = byBind('reference');
  const band = byType('band');
  const bg = byType('background');

  // A composite's words belong to an inner template; refuse rather than guess.
  if (byType('region')) return template.style ?? {};

  const ground = band?.fill ?? bg?.fill ?? (template.style ?? {}).background;

  return {
    ...(template.style ?? {}),
    verseColor: verse?.color ?? (template.style ?? {}).verseColor,
    refColor: ref?.color ?? verse?.color ?? (template.style ?? {}).refColor,
    background: ground,
    verseSize: verse?.size ?? (template.style ?? {}).verseSize,
    refSize: ref?.size ?? (template.style ?? {}).refSize,
  };
}

/** Visible layers only — a hidden layer is not what a congregation sees. */
const layers_ = (ls) => ls.filter((L) => L && L.visible !== false);

/**
 * `review`, for a whole template rather than a flat style blob.
 *
 * The one call a surface should make. It adapts the layer model first, so a
 * caller cannot forget to and quietly get three `unknown`s.
 */
export function reviewTemplate(template = {}, content = null, room = {}) {
  // ASKED ONLY OF A LOOK THAT CAN SHOW ONE. A row that is always there is a row
  // nobody reads, and a scripture-only template warned about a countdown it
  // cannot display is the fastest way to teach an operator to ignore this panel.
  // `templateShows` is the same reader `TemplateRender` uses to decide whether
  // the digits reach the screen at all, so the row and the render cannot disagree.
  return review(styleOfTemplate(template), content, room, {
    countdown: templateShows(template, 'countdown'),
  });
}

export function review(style = {}, content = null, room = {}, { countdown = false } = {}) {
  const verse = checkContrast(style, content, 'verse');
  const reference = checkContrast(style, content, 'ref');
  const distance = checkDistance(style, room);
  // Null, not a fourth `unknown`: "nothing here shows a countdown" and "Relay
  // cannot work out the contrast" are different answers and the caller renders
  // them differently — one is an absent row, the other is a row that says so.
  const countdownWarn = countdown ? checkCountdownWarn(style, content) : null;
  const checks = [verse, reference, distance, ...(countdownWarn ? [countdownWarn] : [])];
  const problems = checks.filter((c) => c.state === 'low' || c.state === 'small');
  return {
    verse,
    reference,
    distance,
    countdownWarn,
    // `unknown` is not a problem and it is not a pass — it is the third answer, and
    // the caller has to render it as its own thing.
    unknowns: checks.filter((c) => c.state === 'unknown').length,
    problems: problems.length,
    caveat: CAVEAT,
  };
}
