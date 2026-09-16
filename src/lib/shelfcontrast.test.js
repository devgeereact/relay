// 7:1 ON EVERY WORD ON THE SHELF — high visibility as a FLOOR, not a family.
//
// Six of the thirty-seven templates a fresh install used to ship had "High
// Visibility" in the name, which is another way of saying that thirty-one of them
// were not. Legibility concentrated in one family is legibility an operator has to
// know to go and look for, on a Sunday, in a lit room, from the back.
//
// So the accessibility family became the accessibility standard. Every one of the
// forty clears 7:1 against its own ground, and the `Contrast` variants exist where
// a church wants the 21:1 maximum rather than as the only place the question was
// asked.
//
// ── THE INSTRUMENT IS THE PRODUCT'S OWN ─────────────────────────────────────
// `parseColor` and `contrastRatio` are the same functions `legibility.js` answers
// the operator's Legibility panel with, so a look that passes here is a look the
// panel will call readable. A second implementation of WCAG's formula in a test
// would be a number that agrees with nothing a church can see.
//
// ── WHAT "ITS OWN GROUND" MEANS, AND WHAT IT REFUSES TO MEAN ────────────────
// The ground is resolved the way the renderer paints it, in this order: the band
// that names the layer as a member, then a band or shape whose box CONTAINS the
// layer, then the background layer. Two of those are translucent in places, and
// they are measured at FULL ALPHA on purpose:
//
//   · a translucent band (`Scroll · Glass`) is held to its declared plate colour,
//     never to the camera behind it;
//   · a scrim over a picture (`Media · Wash`) likewise.
//
// Relay cannot know what a camera is pointed at or what a photograph looks like,
// and a contrast figure that pretended otherwise would be a number that lies —
// which rule 18 says is worse than no number. The alpha each of those ships with
// is bounded so the plate underneath genuinely does the work.
//
// ── WHAT IT DOES NOT ANSWER ─────────────────────────────────────────────────
// Whether the type is big enough. That needs measurement in a real layout engine
// and jsdom has none; `legibility.js::checkDistance` is the arithmetic and a
// rendered browser pass is the evidence. And a composite's words belong to a real
// inner template at a different width, so the composites are checked through the
// built-in their region names rather than guessed at from the outside.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { parseColor, contrastRatio } from './legibility.js';
import { BUILTINS, builtinById } from './templates.js';

const SHELF = JSON.parse(
  readFileSync(resolve(__dirname, '../../src-tauri/data/shelf_templates.json'), 'utf8'),
).templates;

/** Track A's floor. Higher than `CONTRAST_FLOOR` (3, WCAG AA for large text) and
 *  than `CONTRAST_GOOD` (4.5) on purpose: those are what Relay REPORTS against for
 *  a template a church made, and this is what Relay SHIPS. */
const FLOOR = 7;

const visible = (L) => L && L.visible !== false;
const contains = (outer, inner) =>
  inner.x >= outer.x - 0.001 &&
  inner.y >= outer.y - 0.001 &&
  inner.x + inner.w <= outer.x + outer.w + 0.001 &&
  inner.y + inner.h <= outer.y + outer.h + 0.001;

/** The colour the renderer actually paints behind this text layer, or null when
 *  nothing in the template paints one. */
function groundOf(template, layer) {
  const layers = template.layout.layers.filter(visible);
  const band = layers.find((L) => L.type === 'band' && (L.members ?? []).includes(layer.id));
  if (band) return band.fill;
  // A band or shape the layer sits inside. Last one wins: layers paint in order,
  // so the nearest ground is the one declared latest.
  let ground = null;
  for (const L of layers) {
    if (L.type === 'background') ground = L.fill;
    if ((L.type === 'band' || L.type === 'shape') && contains(L, layer)) ground = L.fill;
  }
  return ground;
}

/** Every text layer that carries WORDS a room reads — which is all of them. A
 *  fixed label ("Up Next", "Countdown") is read by a person on the platform and is
 *  held to the same floor. */
const wordsOf = (t) => t.layout.layers.filter((L) => visible(L) && L.type === 'text');

describe('every look on the shelf clears 7:1 against its own ground', () => {
  it.each(SHELF.map((t) => t.name))('%s', (name) => {
    const t = SHELF.find((x) => x.name === name);
    const words = wordsOf(t);
    const composite = t.layout.layers.some((L) => visible(L) && L.type === 'region');

    if (!words.length) {
      // A look with no words of its own is not a pass by omission, and saying so
      // is the whole of rule 35 in miniature: it has to be a look where words
      // WOULD be wrong. A composite's words are its inner template's, and
      // `Media · Full` and `Timer · Monolith` are designs whose entire point is
      // that nothing else is on the screen.
      expect(
        composite || /^(Media · Full)$/.test(name),
        `${name} carries no text at all — if that is not deliberate, the contrast ` +
          'floor is being cleared by having nothing to measure',
      ).toBe(true);
      return;
    }

    for (const L of words) {
      const ground = groundOf(t, L);
      const fg = parseColor(L.color);
      const bg = parseColor(ground);
      expect(fg, `${name} / ${L.name}: the text colour is not a flat colour`).toBeTruthy();
      expect(
        bg,
        `${name} / ${L.name}: nothing paints a measurable ground behind these words ` +
          `(got ${ground ?? 'nothing'}) — a gradient or a variable cannot be checked, ` +
          'and "Relay cannot check this" is not a pass',
      ).toBeTruthy();
      const ratio = contrastRatio(fg, bg);
      expect(
        ratio,
        `${name} / ${L.name}: ${ratio.toFixed(2)}:1 against ${ground}`,
      ).toBeGreaterThanOrEqual(FLOOR);
    }
  });

  it('and a composite clears it through the built-in its region names', () => {
    // DECISIONS §74: a region names a BUILT-IN, because a kiosk or OBS page has no
    // database and resolves the id against the bundled list. So the words in a
    // composite are that built-in's, at the region's width, and THAT is where the
    // question has to be asked. Answering it from the outer template would be a
    // guess with a number on it.
    const regions = SHELF.flatMap((t) =>
      t.layout.layers.filter((L) => visible(L) && L.type === 'region').map((L) => [t.name, L]),
    );
    expect(regions.length, 'the shelf lost its composites').toBe(5);
    for (const [name, L] of regions) {
      const inner = builtinById(L.templateRef);
      expect(inner, `${name}: templateRef ${L.templateRef} is not a built-in`).toBeTruthy();
      const fg = parseColor(inner.style.verseColor);
      const bg = parseColor(inner.style.background);
      expect(fg, `${name}: the inner template's words are not a flat colour`).toBeTruthy();
      expect(
        bg,
        `${name}: the inner template's ground is a gradient, so the words in this ` +
          'composite cannot be checked by anything',
      ).toBeTruthy();
      expect(contrastRatio(fg, bg)).toBeGreaterThanOrEqual(FLOOR);
    }
  });

  it('holds a translucent plate to the colour it declares, not to the camera', () => {
    // The two looks this rule exists for, named so the convention cannot be
    // quietly widened to "anything translucent passes". Both ship an alpha, both
    // are measured at 1, and both would clear the floor at any alpha down to
    // fully transparent ONLY if the thing behind them happened to be dark — which
    // is exactly the assumption Relay is not allowed to make.
    const glass = SHELF.find((t) => t.name === 'Scroll · Glass');
    const band = glass.layout.layers.find((L) => L.type === 'band');
    expect(band.opacity, 'the glass band stopped being translucent').toBeLessThan(1);
    expect(band.opacity, 'the plate is too faint to be doing the work').toBeGreaterThanOrEqual(0.6);

    const wash = SHELF.find((t) => t.name === 'Media · Wash');
    const scrim = wash.layout.layers.find((L) => L.type === 'shape');
    expect(scrim.opacity).toBeLessThan(1);
    expect(scrim.opacity, 'the scrim is too faint to be doing the work').toBeGreaterThanOrEqual(0.6);
  });

  it('and no look wears a law colour as a band or a ground', () => {
    // Rule 18 and DECISIONS §21: amber means ON AIR, amethyst means rehearsal and
    // cyan means a guess. A seeded band shipped in amethyst once, and the repair
    // that made bands visible for the first time would have put a lilac bar on
    // every stream. Checked by HUE rather than by a list of hexes, because the
    // list is what let two different purples ship for one template.
    const hue = ({ r, g, b }) => {
      const max = Math.max(r, g, b);
      const min = Math.min(r, g, b);
      if (max === min) return null; // grey: no hue to clash with
      const d = max - min;
      if (d / max < 0.35) return null; // barely saturated: a tint, not a signal
      let h;
      if (max === r) h = ((g - b) / d) % 6;
      else if (max === g) h = (b - r) / d + 2;
      else h = (r - g) / d + 4;
      return ((h * 60) + 360) % 360;
    };
    const LAW = [
      ['ON AIR amber', 25, 55],
      ['rehearsal amethyst', 260, 300],
      ['a guess cyan', 175, 200],
    ];
    for (const t of SHELF) {
      for (const L of t.layout.layers.filter(visible)) {
        if (!['band', 'background', 'shape'].includes(L.type)) continue;
        const c = parseColor(L.fill);
        if (!c) continue;
        const h = hue(c);
        if (h == null) continue;
        for (const [what, lo, hi] of LAW) {
          expect(
            h > lo && h < hi,
            `${t.name} / ${L.name}: ${L.fill} is ${what}, which already means something else`,
          ).toBe(false);
        }
      }
    }
    expect(BUILTINS.length, 'the frozen built-in mirror went missing').toBeGreaterThan(0);
  });
});
