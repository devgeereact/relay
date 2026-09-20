// THE RUNNING ORDER, COLOURED SECTION BY SECTION — AND THE THREE THINGS THAT
// KEEPS IT HONEST.
//
// The operator asked for the Planner to be colour coded "section by section so a
// volunteer can understand the running order at a glance". `plan.js` has refused
// a per-KIND ramp twice, most recently 2026-09-16, and the reason is arithmetic
// rather than taste: `--v-col-scripture` IS `--v-amber` and `--v-col-media` IS
// `--v-amethyst`, so colouring cue kinds from the existing set puts ON AIR
// colour on a row that is not on air and rehearsal colour on one that is not a
// rehearsal.
//
// Colouring SECTIONS is a different claim and this file holds what makes it a
// safe one:
//
//   1 · the two inks are the two hues nothing else owns, and they resolve to a
//       hex of their own rather than to somebody else's promise;
//   2 · the system REPEATS rather than inventing a hue at section seven;
//   3 · colour is never the only signal — a section's ordinal and its name are
//       printed, and a cue row's edge is a shape rather than a hue.
//
// ── HOW EACH WAS CHECKED ────────────────────────────────────────────────────
//
// Test the bug, not the fix. Every assertion here was watched to go red against
// a defect this feature could plausibly have shipped with:
//
//   · reaching for `--v-col-scripture` for section 1, which is what the wave-4
//     proposal asked for. `colourlaw.test.js`'s resolver now catches that, and
//     "the two inks are nobody else's promise" below catches it here.
//   · numbering the untitled leading group, so a plan that opens with two cues
//     before its first heading grows a "Section 1" the operator never made.
//     Watched red by dropping the title guard in `sectionBands`.
//   · an off-by-one at the wrap — `% SECTION_BANDS.length` on a 1-based ordinal
//     without the -1 paints sections 1 and 2 the same and 3 differently.
//     Watched red by removing the `- 1`.
//   · a heading that printed its colour and not its number, so the running
//     order said nothing in greyscale. Watched red by deleting `.sp-secn` from
//     the view.
//
//   npx vitest run src/lib/sectionband.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { SECTION_BANDS, sectionBands, bandForOrdinal, sectionsOf } from './plan.js';
import { contrastRatio, parseColor } from './legibility.js';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const cap = await import('./stores/capture.js');
const { clearSession } = await import('./session.js');

// The same self-detecting gate `plannermediapreview.test.js` uses: without
// `resolve: { conditions: ['browser'] }` in `vitest.config.js`, `onMount` is a
// literal empty function and a mounted view fetches nothing. These SKIP loudly
// rather than pass over a Planner that never loaded a plan.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const settle = (ms = 40) => new Promise((r) => setTimeout(r, ms));
async function until(predicate, what, tries = 60) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
    await tick();
  }
  throw new Error(`timed out waiting for: ${what}`);
}

const ROOT = resolve(__dirname, '../..');
const read = (f) => readFileSync(resolve(ROOT, f), 'utf8');
const TOKENS = read('src/tokens.css');
const PLANNER = read('src/lib/views/ServicePlanner.svelte');

/** Resolve a `--token` to its literal hex, following `var()` indirection. */
function hexOf(token, seen = new Set()) {
  if (seen.has(token)) return null;
  seen.add(token);
  const m = TOKENS.match(new RegExp(`--${token}\\s*:\\s*([^;]+);`));
  if (!m) return null;
  const value = m[1].trim();
  const via = value.match(/^var\(\s*--([\w-]+)\s*\)$/);
  if (via) return hexOf(via[1], seen);
  return /^#[0-9a-fA-F]{3,8}$/.test(value) ? value : null;
}

const cue = (id, section = '', duration = 0) => ({
  id,
  label: `Cue ${id}`,
  cue_type: 'scripture',
  section_title: section,
  duration_sec: duration,
  payload_json: '{}',
});

// ─────────────────────────────────────────────────────────────────────────────
// 1 · THE PALETTE IS THE FREE SET, AND NOBODY ELSE'S PROMISE
// ─────────────────────────────────────────────────────────────────────────────
describe('the section palette is the two hues the colour law leaves free', () => {
  it('there are exactly two, and adding a third is a decision this test refuses', () => {
    // The count IS the assertion. A third entry here means somebody found a hue
    // on a wheel where `plan.js` has twice recorded that there is none.
    expect(SECTION_BANDS).toHaveLength(2);
  });

  it('each ink resolves to a hex of its OWN, not to a promise colour’s', () => {
    // THE DEFECT THIS EXISTS FOR. `--v-col-scripture` looks like a taxonomy token
    // and IS `var(--v-amber)`. A section ink defined the same way would paint ON
    // AIR on a build surface and read as a new colour in every grep.
    const promises = ['v-amber', 'v-cyan', 'v-amethyst', 'v-rose', 'v-red', 'v-grey', 'v-sel', 'v-emerald'];
    const spoken = new Set(promises.map((t) => hexOf(t)?.toLowerCase()).filter(Boolean));
    expect(spoken.size, 'the resolver found no promise hexes at all').toBeGreaterThan(4);

    for (const band of SECTION_BANDS) {
      const token = band.ink.match(/^var\(--([\w-]+)\)$/)[1];
      const hex = hexOf(token);
      expect(hex, `${band.ink} does not resolve to a hex in tokens.css`).toBeTruthy();
      expect(
        spoken.has(hex.toLowerCase()),
        `${band.ink} is ${hex}, which is a colour the law has already spoken for`,
      ).toBe(false);
    }
  });

  it('the two hues are the ones plan.js names, and are clear of every promise hue', () => {
    // ~310° and ~80° are what `plan.js:35` records as free. A "magenta" at 340°
    // is rose and a "lime" at 50° is amber — a near-miss reads as the promise
    // colour at a glance, which is worse than repeating a palette.
    const hue = (hex) => {
      const { r, g, b } = parseColor(hex);
      const [mx, mn] = [Math.max(r, g, b), Math.min(r, g, b)];
      if (mx === mn) return 0;
      const d = mx - mn;
      let h;
      if (mx === r) h = ((g - b) / d) % 6;
      else if (mx === g) h = (b - r) / d + 2;
      else h = (r - g) / d + 4;
      return ((h * 60) % 360 + 360) % 360;
    };
    const gap = (a, b) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b));

    const inks = SECTION_BANDS.map((b) => hue(hexOf(b.ink.match(/^var\(--([\w-]+)\)$/)[1])));
    expect(gap(inks[0], 314), 'section ink A is not the magenta plan.js names').toBeLessThan(15);
    expect(gap(inks[1], 81), 'section ink B is not the lime plan.js names').toBeLessThan(15);

    const SPOKEN_FOR = {
      'amber (ON AIR)': 36,
      'cyan (a guess)': 194,
      'amethyst (rehearsal)': 267,
      'rose (destructive)': 356,
      'emerald (healthy)': 138,
      'steel (selection)': 215,
    };
    const tooClose = [];
    for (const h of inks) {
      for (const [name, theirs] of Object.entries(SPOKEN_FOR)) {
        if (gap(h, theirs) < 35) tooClose.push(`${Math.round(h)}° is ${Math.round(gap(h, theirs))}° from ${name}`);
      }
    }
    expect(tooClose, 'a near-miss reads as the promise colour under a projector').toEqual([]);
  });

  it('both inks clear AA on every surface the Planner puts them on', () => {
    // The ordinal is TEXT in this ink, so it is body copy and gets the body bar.
    // --v-surf3 is deliberately not in the list: it is the surface --v-faint is
    // already kept off, and the Planner never paints a band on it.
    const SURFACES = ['v-void', 'v-bg', 'v-surf', 'v-surf2'];
    const failures = [];
    for (const band of SECTION_BANDS) {
      const ink = parseColor(hexOf(band.ink.match(/^var\(--([\w-]+)\)$/)[1]));
      for (const s of SURFACES) {
        const ratio = contrastRatio(ink, parseColor(hexOf(s)));
        expect(Number.isFinite(ratio), `${band.ink} on --${s} did not compute`).toBe(true);
        if (ratio < 4.5) failures.push(`${band.ink} on --${s} = ${ratio.toFixed(2)}:1`);
      }
    }
    expect(failures, 'WCAG AA is 4.5:1 for body text').toEqual([]);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 2 · WHAT HAPPENS AT SECTION SEVEN
// ─────────────────────────────────────────────────────────────────────────────
describe('a plan with more sections than hues repeats, in order', () => {
  it('sections 1…8 alternate, and 7 is 1’s colour', () => {
    const inks = [1, 2, 3, 4, 5, 6, 7, 8].map((n) => bandForOrdinal(n).ink);
    expect(inks[0]).toBe(SECTION_BANDS[0].ink);
    expect(inks[1]).toBe(SECTION_BANDS[1].ink);
    // The question the brief asks by name. Seven wears one's colour, deliberately.
    expect(inks[6]).toBe(inks[0]);
    expect(inks[7]).toBe(inks[1]);
    // …and no two ADJACENT sections ever share one, which is the whole job.
    for (let i = 1; i < inks.length; i += 1) expect(inks[i]).not.toBe(inks[i - 1]);
  });

  it('the ordinal is carried through, so a repeated colour is still a distinct section', () => {
    // A repeat is only honest if the number survives it. Section 7 and section 1
    // are the same ink and must never be the same label.
    expect(bandForOrdinal(7).ordinal).toBe(7);
    expect(bandForOrdinal(1).ordinal).toBe(1);
  });

  it('an ordinal that is not a section answers nothing, rather than the first colour', () => {
    // An off-by-one that painted every section magenta would look like a working
    // feature; answering null makes it visible instead.
    for (const bad of [0, -1, NaN, null, undefined, 'x']) {
      expect(bandForOrdinal(bad), String(bad)).toBeNull();
    }
  });

  it('every band carries an ink, a soft fill and a line, and all three are var()', () => {
    for (const b of SECTION_BANDS) {
      for (const k of ['ink', 'soft', 'line']) {
        expect(b[k], k).toMatch(/^var\(--v-sec-[ab](-soft|-line)?\)$/);
      }
    }
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 3 · SECTIONS, NOT KINDS — AND AN UNNAMED GROUP IS NOT A SECTION
// ─────────────────────────────────────────────────────────────────────────────
describe('bands are assigned by position in the plan, never by what a cue is', () => {
  it('a real plan bands its titled sections in order', () => {
    const secs = sectionsOf([
      cue(1, 'Gathering'),
      cue(2),
      cue(3, 'The Word'),
      cue(4),
      cue(5, 'Sending'),
    ]);
    const bands = sectionBands(secs);
    expect(bands.map((b) => b.ordinal)).toEqual([1, 2, 3]);
    expect(bands[0].ink).toBe(SECTION_BANDS[0].ink);
    expect(bands[1].ink).toBe(SECTION_BANDS[1].ink);
    expect(bands[2].ink).toBe(SECTION_BANDS[0].ink);
  });

  it('the cue KIND changes nothing — the same position is the same band', () => {
    // The refusal this feature is built around. Two plans identical in shape but
    // made of different content must band identically.
    const shape = (kind) =>
      sectionBands(
        sectionsOf([
          { ...cue(1, 'Gathering'), cue_type: kind },
          { ...cue(2, 'The Word'), cue_type: kind },
        ]),
      ).map((b) => b.ink);
    expect(shape('scripture')).toEqual(shape('media'));
    expect(shape('song')).toEqual(shape('countdown'));
    expect(shape('announce')).toEqual(shape('scripture'));
  });

  it('the untitled leading group gets no number and no colour', () => {
    // `sectionsOf` opens it so those cues are not dropped on the floor. Numbering
    // a group the operator never named is inventing a section and colouring the
    // invention — a claim from an absence.
    const secs = sectionsOf([cue(1), cue(2), cue(3, 'Sermon')]);
    const bands = sectionBands(secs);
    expect(secs[0].title).toBe('');
    expect(bands[0]).toBeNull();
    // …and the numbering does not skip: the FIRST named section is section 1.
    expect(bands[1].ordinal).toBe(1);
    expect(bands[1].ink).toBe(SECTION_BANDS[0].ink);
  });

  it('a whitespace-only title is not a name either', () => {
    expect(sectionBands([{ title: '   ', items: [] }])[0]).toBeNull();
  });

  it('a plan with no sections at all bands nothing, and does not throw', () => {
    expect(sectionBands(sectionsOf([cue(1), cue(2)]))).toEqual([null]);
    expect(sectionBands([])).toEqual([]);
    expect(sectionBands(null)).toEqual([]);
    expect(sectionBands(undefined)).toEqual([]);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 4 · COLOUR IS NEVER THE ONLY SIGNAL
// ─────────────────────────────────────────────────────────────────────────────
describe('the running order reads correctly with no colour at all', () => {
  it('the heading prints the section’s NUMBER beside its name', () => {
    // The greyscale channel. A heading that carried only a coloured dot would say
    // nothing to a printout, a colour-blind operator, or a projector's washed-out
    // rendering of a laptop screen.
    expect(PLANNER).toMatch(/class="sp-secn /);
    expect(PLANNER).toMatch(/band\.ordinal/);
    expect(PLANNER).toMatch(/class="sp-seccap"/);
  });

  it('the ordinal has a text ACCESSIBLE name, not a bare digit', () => {
    // "3" alone beside a heading is a number a screen reader announces with no
    // idea what it counts.
    expect(PLANNER).toMatch(/class="sr-only">Section&nbsp;<\/span>/);
  });

  it('a cue row’s band is an EDGE — a shape, present or absent, not a hue', () => {
    // `--sec-edge` is a width the row sets from the band; the colour rides on it.
    // A row inside a numbered section has an edge and a row outside one has none,
    // which is legible with every colour removed.
    expect(PLANNER).toMatch(/--sec-ink/);
    expect(PLANNER).toMatch(/\.sp-row\.inband\{/);
  });

  it('the section ink never lands on a promise colour in the view', () => {
    // The view could always undo the module by hard-coding one. Belt and braces
    // with `colourlaw.test.js`'s sweep, which reads every component.
    // The rules that draw a band, read as themselves rather than as "everything
    // after the first one" — the Planner's stylesheet legitimately spends rose
    // further down, on the delete control and the stale-arrangement warning.
    const rules = [...PLANNER.matchAll(/\.sp-(sec|secn|secln|row\.inband)[^{]*\{[^}]*\}/g)].map((m) => m[0]);
    expect(rules.length, 'the band rules were not found at all').toBeGreaterThanOrEqual(4);
    for (const t of ['--v-amber', '--v-cyan', '--v-amethyst', '--v-rose', '--v-red', '--v-grey']) {
      const hit = rules.filter((r) => r.includes(`var(${t}`));
      expect(hit, `the section band paints ${t}`).toEqual([]);
    }
    // …and the ink it DOES paint is the module's, read off the element rather
    // than restated in CSS, so the stylesheet cannot disagree with `plan.js`.
    expect(rules.join('\n')).toMatch(/var\(--sec-ink/);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// 5 · DRIVEN, NOT READ
// ─────────────────────────────────────────────────────────────────────────────
//
// Everything above this line reads the view off disk, which is the right
// instrument for "is the rule in one place" and the wrong one for "does the
// running order actually draw it". This block mounts the Planner against a mock
// bridge and reads the DOM. The machine cannot screenshot the Tauri window, but
// there is a real layout engine here, and both browser-driven audits this
// repository has run found things every source scan was blind to.
const PLAN = { id: 1, title: 'Sunday Morning', plan_date: '2026-09-20', cue_count: 6 };

const planCue = (id, section = '') => ({
  id,
  plan_id: 1,
  position: id,
  cue_type: 'scripture',
  label: `Cue ${id}`,
  payload_json: JSON.stringify({ reference: `Romans 8:${id}`, text: 'words' }),
  template_id: null,
  section_title: section,
  duration_sec: 0,
  timer_minutes: null,
  channels_json: null,
});

let cues = [];
let host;
let app;

beforeEach(() => {
  cues = [
    planCue(1), // before any heading — NOT a section
    planCue(2, 'Gathering'),
    planCue(3),
    planCue(4, 'The Word'),
    planCue(5, 'Sending'),
    planCue(6, 'After'),
  ];
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_plans':
        return Promise.resolve([PLAN]);
      case 'plan_items':
        return Promise.resolve(cues);
      case 'list_templates':
        return Promise.resolve([{ id: 1, name: 'Classic Serif' }]);
      case 'get_rehearsal':
        return Promise.resolve(false);
      case 'get_sensitivity':
        return Promise.resolve(50);
      default:
        return Promise.resolve([]);
    }
  });
  cap.capture.update((c) => ({ ...c, available: true }));
  clearSession();
  host = document.createElement('div');
  document.body.appendChild(host);
});

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  clearSession();
  document.body.innerHTML = '';
});

async function openPlan() {
  const ServicePlanner = (await import('./views/ServicePlanner.svelte')).default;
  app = new ServicePlanner({ target: host });
  await until(() => host.querySelector('.sp-railcard'), 'the plan rail');
  host.querySelector('.sp-railcard').click();
  await until(() => host.querySelectorAll('.sp-row').length === cues.length, 'the running order');
}

describe('the running order, driven', () => {
  itMounted('every named section prints its number, in order, from one', async () => {
    await openPlan();
    const numbers = [...host.querySelectorAll('.sp-secn')].map((n) => n.textContent.trim());
    // Four headings, numbered 1–4. The untitled cue above them is not numbered.
    expect(numbers).toEqual([
      'Section 1',
      'Section 2',
      'Section 3',
      'Section 4',
    ]);
    expect(host.querySelectorAll('.sp-sec').length).toBe(4);
  });

  itMounted('the fourth section wears the SECOND ink — the palette repeats', async () => {
    await openPlan();
    const inks = [...host.querySelectorAll('.sp-sec')].map((el) =>
      el.style.getPropertyValue('--sec-ink'),
    );
    expect(inks[0]).toBe(SECTION_BANDS[0].ink);
    expect(inks[1]).toBe(SECTION_BANDS[1].ink);
    expect(inks[2]).toBe(SECTION_BANDS[0].ink);
    expect(inks[3]).toBe(SECTION_BANDS[1].ink);
  });

  itMounted('a cue inside a section wears an edge; the one above the first heading does not', async () => {
    await openPlan();
    const rows = [...host.querySelectorAll('.sp-row')];
    expect(rows).toHaveLength(6);
    // Cue 1 is before any heading — no band, no edge, no colour.
    expect(rows[0].classList.contains('inband')).toBe(false);
    expect(rows[0].style.getPropertyValue('--sec-ink')).toBe('');
    // Cues 2 and 3 are both inside "Gathering" and share its ink.
    for (const i of [1, 2]) {
      expect(rows[i].classList.contains('inband'), `row ${i}`).toBe(true);
      expect(rows[i].style.getPropertyValue('--sec-ink')).toBe(SECTION_BANDS[0].ink);
    }
    // "The Word" is the next section, and the next ink.
    expect(rows[3].style.getPropertyValue('--sec-ink')).toBe(SECTION_BANDS[1].ink);
  });

  itMounted('no row and no heading carries a promise colour', async () => {
    // The law, read off the rendered tree rather than off the stylesheet. Amber
    // means ON AIR; nothing on a workspace that cannot reach an output wears it.
    await openPlan();
    const painted = [...host.querySelectorAll('.sp-row, .sp-sec, .sp-secn')]
      .map((el) => el.getAttribute('style') || '')
      .join(' ');
    for (const t of ['--v-amber', '--v-cyan', '--v-amethyst', '--v-rose', '--v-red', '--v-grey']) {
      expect(painted.includes(t), `a rendered element paints ${t}`).toBe(false);
    }
  });

  itMounted('a plan with no headings at all bands nothing and still lists every cue', async () => {
    // The opposite mistake: a change that forced a band on, so an unnamed plan
    // grew sections nobody made.
    cues = [planCue(1), planCue(2), planCue(3)];
    await openPlan();
    expect(host.querySelectorAll('.sp-sec')).toHaveLength(0);
    expect(host.querySelectorAll('.sp-row')).toHaveLength(3);
    expect(host.querySelectorAll('.sp-row.inband')).toHaveLength(0);
  });
});
