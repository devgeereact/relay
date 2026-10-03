// REQUIREMENT 12 — THE CONGREGATION-FACING COUNTDOWN, AS TYPE.
//
// Scope, from the operator: the congregation countdown only. "The stage timer is
// the exception: it stays inline, because readability for the preacher wins over
// styling." `src/Stage.svelte` is therefore untouched and is not read here.
//
// It is also only the TWO branches Relay itself styles — the classic region
// countdown (`.verse.countdown`) and the default overlay a layered template with
// no timer layer falls back to (`.lfit.countdown`). A template that carries its
// own TIMER LAYER is not in scope and must not be: that layer's weight, tracking,
// colour and size are the template designer's declared choices, and a renderer
// that overrode them would be answering a design question nobody asked it.
// The classes are the boundary — `.countdown` exists on exactly the two defaults.
//
// ── WHAT THIS FILE CAN AND CANNOT SEE ──────────────────────────────────────
//
// jsdom lays nothing out and paints nothing, so NOTHING here claims the digits
// look good, are readable at 20m, or settle smoothly. What it can hold is the
// four things that are facts rather than impressions:
//
//   1. the separator is a distinct element, so it CAN be set back from the
//      figures, and the figures are not dragged down with it;
//   2. the element's text is still exactly the formatted figure — the transport
//      and `countdownwiring.test.js` read `.countdown`'s `textContent` and a
//      stray newline from the markup would break a control, not a look;
//   3. a group whose digits changed is a NEW ELEMENT and one whose digits did not
//      is the SAME ELEMENT — which is the whole mechanism behind a per-second
//      settle, asserted as identity rather than as animation;
//   4. the motion is composite-only and cannot reach the fitter. Rule 37, rule 42
//      and RG-141 all rest on `fitOne` measuring a box whose geometry the digits
//      do not move, and the tick gate (`fitSig`'s `countdownTo ? 1 : 0`) rests on
//      a tick never changing the signature. Both are asserted against the source.
//
//   npx vitest run src/lib/countdowntype.test.js
//
// CLAUDE.md rule 37 · rule 42 · RG-128 · RG-141
import { describe, it, expect, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import TemplateRender from './TemplateRender.svelte';
import { countdownParts, formatCountdown } from './layers.js';
import { review, reviewTemplate, COUNTDOWN_WARN_COLOR } from './legibility.js';

const read = (p) => readFileSync(resolve(process.cwd(), p), 'utf8');
const SRC = read('src/lib/TemplateRender.svelte');
/** The component's `<style>` block, which is the only place these rules may live
 *  — an inline style would be per-element and could not be reasoned about. */
const CSS = SRC.slice(SRC.lastIndexOf('<style>'), SRC.lastIndexOf('</style>'));

const SHELF = JSON.parse(read('src-tauri/data/shelf_templates.json')).templates;

// A plain region template — no layers at all, so the CLASSIC branch renders and
// `.content.cdbox` (RG-141) is the box the digits are fitted against.
const REGION = {
  name: 'test region',
  layout: { regions: ['verse'], shows: ['scripture', 'countdown'] },
  style: { verseSize: 5, refSize: 2, background: '#000', verseColor: '#fff' },
};

/** A layered look with NO timer layer → the default overlay branch. */
const LAYERED_NO_TIMER = {
  name: 'test layered',
  layout: {
    shows: ['scripture', 'countdown'],
    layers: [
      { id: 'bg', type: 'background', w: 100, h: 100, fill: '#000' },
      { id: 'v', type: 'text', bind: 'verse', x: 6, y: 20, w: 88, h: 40, size: 5, color: '#fff' },
    ],
  },
  style: {},
};

const countdown = (msLeft = 5 * 60_000) => ({
  kind: 'countdown',
  content_kind: 'countdown',
  reference: 'Service begins in',
  text: '',
  countdown_to: Date.now() + msLeft,
  countdown_from: Date.now(),
});

let host;
let app;
function mount(template, content, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template, content, ...props } });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

const settle = () => new Promise((r) => setTimeout(r, 250));
/** Long enough for the FORMATTED figure to move. `TemplateRender` reads the clock
 *  four times a second and `m:ss` changes once a second, so anything shorter
 *  would pass without the thing under test ever happening. */
const oneSecond = () => new Promise((r) => setTimeout(r, 1300));

// ───────────────────────────────────────────────────────────────────────────
describe('the figure is split so the separator can be set back from the digits', () => {
  it('splits a formatted countdown into digit groups and separators, and nothing else', () => {
    // Pure, and it is a SPLITTER of `formatCountdown`'s answer rather than a
    // second formatter — there is exactly one countdown formatter (`layers.js`)
    // and a second one is how the wall and the stage came to disagree.
    expect(countdownParts('5:00').map((p) => p.t)).toEqual(['5', ':', '00']);
    expect(countdownParts('1:04:09').map((p) => p.t)).toEqual(['1', ':', '04', ':', '09']);
    expect(countdownParts('5:00').map((p) => p.sep)).toEqual([false, true, false]);
    expect(countdownParts(''), 'nothing to show is no parts, not one empty part').toEqual([]);
    // Whatever it splits, it must put back together exactly — this is what keeps
    // `.countdown`'s textContent equal to the figure the transport reads.
    for (const ms of [0, 999, 1000, 59_000, 60_000, 599_000, 3_600_000, 43_200_000]) {
      const t = formatCountdown(ms);
      expect(countdownParts(t).map((p) => p.t).join('')).toBe(t);
    }
  });

  it('keys each group by its own value, so an unchanged group is the same element', () => {
    // The mechanism behind the settle, stated where it can be tested without a
    // browser: the minutes group only gets a new key when the minutes change.
    const a = countdownParts('5:00');
    const b = countdownParts('4:59');
    const c = countdownParts('4:58');
    expect(new Set(a.map((p) => p.k)).size, 'keys must be unique within a figure').toBe(a.length);
    expect(b[0].k).not.toBe(a[0].k); // minutes moved
    expect(c[0].k).toBe(b[0].k); // minutes held
    expect(c[2].k).not.toBe(b[2].k); // seconds moved
    expect(c[1].k).toBe(b[1].k); // the separator never moves
  });

  it.each([
    ['the classic region branch', REGION, '.verse.countdown'],
    ['the default overlay branch', LAYERED_NO_TIMER, '.lfit.countdown'],
  ])('%s paints the separator as its own element', async (_name, template, sel) => {
    const el = mount(template, countdown());
    await settle();
    const cd = el.querySelector(sel);
    expect(cd, 'no countdown was painted at all').toBeTruthy();
    const sep = cd.querySelectorAll('.cd-sep');
    const nums = cd.querySelectorAll('.cd-num');
    expect(sep.length, 'the separator is not addressable, so it cannot be set back').toBe(1);
    expect(sep[0].textContent).toBe(':');
    expect(nums.length, 'the digits are not addressable, so they cannot settle').toBe(2);
  });

  it.each([
    ['the classic region branch', REGION, '.verse.countdown'],
    ['the default overlay branch', LAYERED_NO_TIMER, '.lfit.countdown'],
  ])('%s still reads as exactly the figure, with no markup whitespace in it', async (_name, template, sel) => {
    // `countdownwiring.test.js` reads `.countdown`'s textContent and trims it.
    // A newline BETWEEN the spans would survive that trim and break a control on
    // the strength of a styling change, which is the worst possible trade.
    const el = mount(template, countdown(5 * 60_000 + 500));
    await settle();
    const cd = el.querySelector(sel);
    expect(cd.textContent).toMatch(/^\d{1,2}:\d{2}$/);
    expect(cd.textContent, 'markup whitespace leaked into the figure').toBe(cd.textContent.trim());
  });

  it('sets the separator back without touching the weight or colour of the digits', () => {
    // A colon at 110-192px carries the same visual mass as a pair of digit stems
    // and pulls the eye to the middle of the figure; setting it back is what makes
    // the minutes and the seconds read as two groups instead of one block. It is
    // a SEPARATOR, not information — the figure reads identically without it —
    // which is why reducing its prominence is a legibility gain and not a
    // contrast loss the way it would be on a digit.
    expect(CSS, 'the separator is not set back from the digits').toMatch(
      /\.countdown \.cd-sep \{[^}]*opacity:\s*0?\.[0-9]+/,
    );
    // The digits keep every property that decides whether the back row can read
    // them. `tabular-nums` above all: without it the figure jitters every second.
    const cd = CSS.slice(CSS.indexOf('\n  .countdown {'));
    const block = cd.slice(0, cd.indexOf('}'));
    expect(block).toMatch(/font-variant-numeric:\s*tabular-nums/);
    expect(block).toMatch(/font-weight:\s*700/);
    expect(block).toMatch(/white-space:\s*nowrap/);
    expect(block).toMatch(/line-height:\s*1\.05/);
  });

  it('does not track a tabular figure that is already evenly spaced', () => {
    // `tabular-nums` gives every figure the SAME advance, already sized to the
    // widest digit — tracking on top of that is spacing applied twice. It also
    // costs centring: CSS puts letter-spacing after the LAST glyph too, so a
    // centred figure sits half a track left of centre, which at 192px is a visible
    // offset on an otherwise empty screen. Zero, stated rather than omitted, so
    // the reasoning has somewhere to live.
    const cd = CSS.slice(CSS.indexOf('\n  .countdown {'));
    const block = cd.slice(0, cd.indexOf('}'));
    expect(block).toMatch(/letter-spacing:\s*0;/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
describe('a digit that changes settles, and the settle cannot reach the fitter', () => {
  it('rebuilds only the group whose digits moved', async () => {
    const el = mount(REGION, countdown(5 * 60_000 + 500));
    await settle();
    const cd = el.querySelector('.verse.countdown');
    const before = [...cd.querySelectorAll('.cd-num, .cd-sep')];
    const firstText = cd.textContent;

    await oneSecond();

    const after = [...cd.querySelectorAll('.cd-num, .cd-sep')];
    expect(cd.textContent, 'the clock did not move, so nothing below was tested').not.toBe(firstText);
    expect(after.length).toBe(before.length);
    // The seconds group changed → a NEW element, which is what restarts the CSS
    // animation without any JS timing loop and without a `{#key}` around the
    // fitted element itself.
    expect(after[2], 'the seconds group was reused, so it can never settle').not.toBe(before[2]);
    // The separator did not → the SAME element, so it does not flicker every
    // second alongside the digits.
    expect(after[1], 'the separator was rebuilt with the digits').toBe(before[1]);
  });

  it('animates a composite-only property, under a motion preference', () => {
    const at = CSS.indexOf('@keyframes cdsettle');
    expect(at, 'no settle keyframes').toBeGreaterThan(-1);
    const frames = CSS.slice(at, CSS.indexOf('}', CSS.indexOf('}', at) + 1) + 1);
    // OPACITY ONLY, and this is the load-bearing half. `font-size` is the
    // fitter's own output; width, height, margin and padding move the box the fit
    // was measured against; and a TRANSFORM contributes to a parent's scrollable
    // overflow, which is exactly the `scrollWidth > clientWidth` test `fitOne`
    // stops on (rule 37 · RG-141). Opacity can do none of those things — the
    // guarantee is by construction rather than by argument.
    expect(frames).toMatch(/opacity/);
    expect(frames, 'the settle moves layout, which the fitter measures').not.toMatch(
      /font-size|width|height|margin|padding|transform|letter-spacing|top|left|inset/,
    );
    // And it is asked for only where motion was welcome, like `cdwarn` beside it.
    const noPref = CSS.indexOf('prefers-reduced-motion: no-preference');
    const use = CSS.indexOf('animation: cdsettle');
    expect(use, 'nothing uses the settle keyframes').toBeGreaterThan(-1);
    expect(use, 'the settle runs regardless of a reduced-motion preference').toBeGreaterThan(noPref);
  });

  it('leaves the tick gate exactly as it was — a tick still does not re-fit', () => {
    // The 4Hz reflow storm. `fitSig` folds a countdown in as PRESENCE, never as
    // its text, so `4:59` -> `4:58` does not move the signature and no binary
    // search runs. Nothing about a look may be bought with that.
    const sig = SRC.slice(SRC.indexOf('function fitSig()'), SRC.indexOf('function sizeIs('));
    expect(sig).toMatch(/\$\{countdownTo \? 1 : 0\}/);
    expect(sig, 'the countdown text reached the fit signature').not.toMatch(/countdownText/);
  });

  it('and the two rules the countdown box rests on are still stated', () => {
    // Not this change's work — its job is to have left them alone, and a styling
    // pass near a fit is precisely where they get quietly undone.
    expect(SRC).toMatch(/const MIN_LEGIBLE_SCALE = 0\.45;/); // rule 37
    expect(SRC).toMatch(/export function needsRefit\(/); // rule 42
    expect(CSS).toMatch(/\.content\.cdbox \{[^}]*width:\s*90%;[^}]*height:\s*92%;/); // RG-141
    // RG-166: a keyed template refuses the clock rather than painting it over a
    // live camera. A separator span must not have become a second way in.
    expect(SRC).toMatch(/\$: countdownAllowed = !!countdownTo && !keyedOut;/);
  });
});

// ───────────────────────────────────────────────────────────────────────────
describe('the readability panel knows about the largest thing on the screen', () => {
  // THE GAP: `legibility.js` reviews the colours the TEMPLATE declares. The
  // countdown's last minute is painted in a red the template never declared and
  // `TemplateRender` hard-codes (`CD_WARN`), so the one tool that answers "can the
  // back row read this" was silent about the most time-critical thing Relay ever
  // puts on a wall. Distance needs nothing — the countdown paints at TWICE the
  // verse size, so the verse is always the binding constraint and the existing
  // verdict is already conservative for it. Contrast is the real absence.
  it('is one red, not two opinions about red', () => {
    expect(COUNTDOWN_WARN_COLOR).toBe('#f4515b');
    expect(SRC, 'the renderer and the reviewer disagree about the warning red').toMatch(
      new RegExp(`const CD_WARN = '${COUNTDOWN_WARN_COLOR}';`),
    );
    expect(CSS).toMatch(new RegExp(`\\.countdown\\.warn \\{ color: ${COUNTDOWN_WARN_COLOR}; \\}`));
  });

  it('answers for the warning red against the ground it lands on', () => {
    // #8c2a33 against the warning red is 2.48:1 — under the 3.0 floor these
    // checks already use for large text, and a background a church could
    // plausibly choose for an Advent or a Passion look.
    const bad = review({ background: '#8c2a33', verseColor: '#ffffff' }, null, {}, { countdown: true });
    expect(bad.countdownWarn.state, 'red on a dark red read as fine').toBe('low');
    const good = review({ background: '#000000', verseColor: '#ffffff' }, null, {}, { countdown: true });
    expect(good.countdownWarn.state).toBe('ok');
    expect(bad.problems).toBeGreaterThan(good.problems);
  });

  it('says nothing about a countdown on a template that never shows one', () => {
    // A row that is always there is a row nobody reads. It is also how a
    // scripture-only look acquires a warning about something it cannot display,
    // which is the fastest way to teach an operator to ignore the panel.
    const style = { background: '#8c2a33', verseColor: '#fff' };
    const quiet = review(style, null, {});
    const asked = review(style, null, {}, { countdown: true });
    expect(quiet.countdownWarn).toBeNull();
    // The other three answers are byte-for-byte what they were before this
    // existed — the opt-in adds a row, it does not re-weigh the ones beside it.
    expect(quiet.verse).toEqual(asked.verse);
    expect(quiet.reference).toEqual(asked.reference);
    expect(quiet.distance).toEqual(asked.distance);
    expect(quiet.problems).toBe(asked.problems - 1);
    expect(quiet.unknowns).toBe(asked.unknowns);
  });

  it('asks the question for every shipped look that shows a countdown, and only those', () => {
    const shown = SHELF.filter((t) => (t.layout?.shows ?? []).includes('countdown'));
    expect(shown.length, 'no shipped look shows a countdown, so nothing above matters').toBeGreaterThan(0);
    for (const t of SHELF) {
      const r = reviewTemplate(t, null, { screenWidthM: 4, backRowM: 18 });
      const asked = r.countdownWarn !== null;
      expect(asked, `${t.name}: the countdown row does not follow what the look shows`).toBe(
        (t.layout?.shows ?? []).includes('countdown'),
      );
    }
  });

  it('and the editor renders the answer rather than computing one nobody sees', () => {
    const ed = read('src/lib/views/templates/TemplateEditor.svelte');
    expect(ed, 'the countdown verdict is computed and never rendered').toMatch(/legible\.countdownWarn/);
  });
});
