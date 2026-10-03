// THE WARNING FLASH, ON EVERY SURFACE THAT SHOWS ONE.
//
// Wave 3 track D's third task is "assert the flash, do not rebuild it": the
// `cdwarn` keyframes already exist on the wall (`TemplateRender.svelte`), the
// preacher's page and his programme rail (`Stage.svelte`), and the console's own
// countdown figure — which was `Dock.svelte`'s until 2026-09-20 and is now the
// Screen Countdown band in `views/Live.svelte`. Nothing pinned them. This file does, and the guarantee it
// pins is not "there is an animation" — it is the one an animation can break:
//
//   · a viewer who asked for no motion still learns that the countdown is
//     running out, because the COLOUR is stated unconditionally, outside every
//     motion query; and
//   · a viewer who asked for no motion is not given the pulse anyway, because
//     every `animation: cdwarn` sits inside `prefers-reduced-motion: no-preference`.
//
// One correction to the plan, recorded here because it is the kind of claim that
// gets copied forward. The plan says all three carry "a `prefers-reduced-motion`
// glow fallback each". Not all of them do. The CONSOLE's does not, deliberately:
// its own comment says "a viewer who asked for no motion still gets the RED,
// which is the information", and a figure an operator is sitting in front of is
// not a wall seen from the back of a room. So it is asserted on the colour, which
// is its actual answer, rather than on a glow it was never given.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { codeOnly } from './codeonly.js';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

/** The `<style>` block of a Svelte file, with its commentary removed. A comment
 *  that mentions a declaration is not the declaration — this repository has had a
 *  scan report a removed control as present for exactly that reason. */
function styleOf(file) {
  const src = read(file);
  const at = src.indexOf('<style>');
  return codeOnly(src.slice(at, src.lastIndexOf('</style>')));
}

/** The [start, end) extents of every `@media (...query...)` block, by brace
 *  matching. A regex cannot answer "is this declaration inside that block", and
 *  that is the only question this file asks. */
function mediaBlocks(css, query) {
  const out = [];
  const needle = `@media (${query})`;
  let at = css.indexOf(needle);
  while (at !== -1) {
    let i = css.indexOf('{', at);
    let depth = 0;
    for (; i < css.length; i++) {
      if (css[i] === '{') depth++;
      else if (css[i] === '}' && --depth === 0) break;
    }
    out.push([at, i]);
    at = css.indexOf(needle, i);
  }
  return out;
}

const inside = (blocks, idx) => blocks.some(([a, b]) => idx > a && idx < b);

/**
 * Every index at which `re` matches.
 *
 * A REGEX RATHER THAN `indexOf`, and the difference is not cosmetic. This was
 * `indexOf('animation: cdwarn')` — one literal, one space — and three of the four
 * surfaces happen to be written with that space. The fourth (Live's stylesheet,
 * which sets declarations without them) would have been reported as having NO
 * pulse at all, and the honest reading of that failure is not "the surface is
 * broken" but "the scanner cannot see it". A scanner that quietly narrows passes
 * everything, and this one would have failed loudly — which is luck, because the
 * same literal in the `inside()` check above could as easily have matched zero
 * spots and satisfied a `for` loop over nothing.
 */
function everyIndexOf(css, re) {
  return [...css.matchAll(re)].map((m) => m.index);
}

const SURFACES = [
  {
    name: 'the wall',
    file: 'src/lib/TemplateRender.svelte',
    // The verse box and the countdown digits are the same renderer, so this is
    // literally what a congregation sees.
    colour: /\.countdown\.warn \{ color: #f4515b; \}/,
    glow: /\.countdown\.warn \{ text-shadow: [^}]*\}/,
  },
  {
    name: "the preacher's page",
    file: 'src/Stage.svelte',
    colour: /\.fig\.warn \.figv \{ color: var\(--v-red\); \}/,
    glow: /\.fig\.warn \.figv, \.railrow\.warn \{ text-shadow: [^}]*\}/,
  },
  {
    // WAVE 4 TRACK A. The fourth surface, and the one the other three made it
    // obvious was missing: the preacher's own programme rail rendered a label and
    // digits and had no warning state at all, on the one screen in the building
    // whose whole job is telling somebody how long is left. It reuses `.fig.warn`'s
    // answer one row up rather than inventing a second — same red, same cut.
    name: "the preacher's programme rail",
    file: 'src/Stage.svelte',
    // THE COLOUR IS THE WARNING WINDOW AND THE GLOW IS THE BOUNDARY (2026-09-21).
    // They were both on `warn` until the operator asked the rail to flash when
    // the time actually goes. The steady red still marks the window; the pulse
    // and its reduced-motion glow moved to `over`, because a signal that runs
    // for the whole window is one nobody reads at the moment it is for. Both
    // stage surfaces keep the same split — `stagetimerover.test.js` holds that.
    colour: /\.tmr\.warn \.tval \{ color: var\(--v-red\); \}/,
    glow: /\.tmr\.over \.tval \{ text-shadow: [^}]*\}/,
  },
  // THE CONSOLE'S COUNTDOWN BAND IS NOT A SURFACE ANY MORE (2026-09-20, evening).
  //
  // It moved from `Dock.svelte` to `Live.svelte` that morning and was removed
  // from the console that evening on the operator's instruction, taking `.tfig`,
  // its red and the `cdwarn` blink with it. The entry is DELETED rather than
  // repointed, because there is no third file holding this control: repointing it
  // at the Planner would assert a warning colour on a surface that shows no
  // running countdown at all, and pointing it anywhere else would pass over a
  // stylesheet with no countdown in it, which is the failure the note it replaces
  // was written to prevent.
  //
  // The three surfaces below are the ones a congregation or a preacher can see,
  // and they are untouched.
];

describe('the countdown warning survives a viewer who asked for no motion', () => {
  for (const s of SURFACES) {
    describe(s.name, () => {
      const css = styleOf(s.file);
      const noPref = mediaBlocks(css, 'prefers-reduced-motion: no-preference');
      const reduce = mediaBlocks(css, 'prefers-reduced-motion: reduce');

      it('states the warning colour unconditionally, outside every motion query', () => {
        // The one that matters. If the colour only exists inside the animation
        // block, a reduced-motion viewer gets no warning at all — the countdown
        // simply runs out in the ordinary face while somebody watches it.
        const m = s.colour.exec(css);
        expect(m, `no unconditional warn colour in ${s.file}`).toBeTruthy();
        expect(inside(noPref, m.index), 'the colour is inside the no-preference block').toBe(false);
        expect(inside(reduce, m.index), 'the colour is inside the reduce block').toBe(false);
      });

      it('gives the pulse only where motion was welcome', () => {
        const spots = everyIndexOf(css, /animation:\s*cdwarn/g);
        expect(spots.length, `no cdwarn animation in ${s.file}`).toBeGreaterThan(0);
        for (const at of spots) {
          expect(
            inside(noPref, at),
            `an "animation: cdwarn" in ${s.file} is outside prefers-reduced-motion: no-preference`,
          ).toBe(true);
        }
      });

      it('defines the keyframes it animates', () => {
        // A `cdwarn` animation with no `@keyframes cdwarn` in the same component
        // is silently inert: Svelte scopes keyframes per component, so a surface
        // that inherited the name and not the definition would simply not pulse.
        expect(css).toMatch(/@keyframes cdwarn/);
      });

      if (s.glow) {
        it('reaches for a glow when the pulse is refused, and keeps the same red', () => {
          const m = s.glow.exec(css);
          expect(m, `no reduced-motion fallback in ${s.file}`).toBeTruthy();
          expect(inside(reduce, m.index), 'the glow is not inside a reduce block').toBe(true);
          expect(m[0]).toMatch(/244, ?81, ?91/);
        });
      } else {
        it('answers with the colour alone, which is the information', () => {
          // Recorded rather than asserted as an absence: if a glow is ever added
          // here, that is an improvement and this test should be updated, not a
          // regression. What must stay true is the line above it — the colour is
          // unconditional — and that is asserted, not this.
          expect(reduce.every(([a, b]) => !css.slice(a, b).includes('.tfig.warn'))).toBe(true);
        });
      }
    });
  }

  it('is one colour, not three opinions about red', () => {
    // The wall writes the hex inline as well (an inline style beats a stylesheet
    // rule, so a class alone would change nothing on a screen), and the other two
    // use the token. They have to be the same red or the same countdown reads as
    // two different states depending on which screen you are looking at.
    const tr = styleOf('src/lib/TemplateRender.svelte');
    expect(tr).toMatch(/\.countdown\.warn \{ color: #f4515b; \}/);
    expect(read('src/lib/TemplateRender.svelte')).toMatch(/const CD_WARN = '#f4515b';/);
    // THE TOKEN MOVED, THE CLAIM DID NOT. This read `src/app.css` until wave 5
    // lifted the whole palette into `src/tokens.css` so that `output.js` and
    // `stage.js` could import the colours without dragging the operator console's
    // stylesheet onto a congregation screen. The two waves met in a merge and this
    // assertion went red on a file, not on a red. Both halves are asserted now —
    // where the token is DECLARED, and that the console's stylesheet still pulls
    // that file in — because a token declared in a file nothing imports is the same
    // failure as a token that does not exist.
    expect(read('src/tokens.css')).toMatch(/--v-red:\s*#f4515b/i);
    expect(read('src/app.css')).toMatch(/@import '\.\/tokens\.css';/);
  });
});
