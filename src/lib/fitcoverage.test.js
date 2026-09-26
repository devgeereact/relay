import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import TemplateRender from './TemplateRender.svelte';
import { codeOnly } from './codeonly.js';

// WHAT THE FIT LOOP CANNOT SEE, IT CANNOT SHRINK — AND CANNOT REPORT.
//
// `fitLayers` queries `.ltext`; `fitText` queries `.slide .content`. The default
// countdown block was emitted outside `{#each layerViews}` with neither class,
// so the largest type Relay ever paints (verseSize × 2) was outside the binary
// search, outside `overflowing()` and outside `onFit`. Rule 37's "a fit loop
// with no notion of failure always succeeds" one level up: this one had no
// notion of the box at all, and its CSS set no `overflow` either, so it did not
// even clip.
//
// jsdom does not lay out, so this test asserts the CONTRACT the fitter reads —
// the classes and the data attributes — not a pixel. The pixel half is Task 16's
// browser pass, against the real backend at 1920×1080.
//
// This repo has no `@testing-library/svelte` dependency — every other component
// test (`cardfit.test.js`) mounts via Svelte 4's own `new Component({ target,
// props })`, so this file follows that convention rather than the brief's import.

  // RG-166 — `type: 'bg'` IS NOT A LAYER TYPE. `TemplateRender` draws
  // `background`, `backdrop`, `media`, `shape`, `band`, `region`, `text` and
  // `timer`, and nothing else, so this template declared a black background that
  // the renderer has never painted: it was a fully TRANSPARENT screen the whole
  // time, and `layers.js::isKeyedTemplate` says so. That did not matter while the
  // countdown refusal read `layout.lowerThird`; it does now that both render
  // paths ask the keyed question, and a countdown may not paint over a camera.
  // Corrected rather than exempted — the fixture now IS the opaque screen it
  // always claimed to be, which is what these assertions are about.
const timerTemplate = {
  id: 1,
  name: 'Timer',
  layout: { layers: [{ id: 'bg', type: 'background', visible: true, fill: '#000', opacity: 1 }] },
  style: { verseSize: '6' },
};

let host;
let app;
function mount(props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props });
  return host;
}
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

describe('the default countdown block is inside the fit loop', () => {
  it('emits the .ltext / .lfit contract fitLayers reads', () => {
    const container = mount({
      template: timerTemplate,
      content: { kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() + 60000 },
    });
    const box = container.querySelector('.cd-default .ltext');
    expect(box, 'the digits must sit in a box the fitter queries').toBeTruthy();
    const fit = box.querySelector('.lfit');
    expect(fit, 'the box must hold exactly the .lfit element fitLayers sizes').toBeTruthy();
    expect(fit.dataset.base, 'the designed size must be declared, not only fitted').toBeTruthy();
    expect(fit.dataset.fit).toBe('shrink');
  });

  it('puts the label in the loop too', () => {
    const container = mount({
      template: timerTemplate,
      content: { kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() + 60000 },
    });
    expect(container.querySelectorAll('.cd-default .ltext').length).toBe(2);
  });

  it('keeps just the digits box when there is no reference to show', () => {
    // `{#if content.reference && !countdownDone}` guards the label box: no
    // reference, or a countdown that has finished, and only `.cd-digits` renders.
    const container = mount({
      template: timerTemplate,
      content: { kind: 'countdown', countdown_to: Date.now() + 60000 },
    });
    expect(container.querySelectorAll('.cd-default .ltext').length).toBe(1);
    expect(container.querySelector('.cd-default .cd-ref')).toBeNull();
    expect(container.querySelector('.cd-default .cd-digits')).toBeTruthy();
  });

  it('the same happens once the countdown is done, even with a reference', () => {
    const container = mount({
      template: timerTemplate,
      content: { kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() - 1000, countdown_done: 'Welcome' },
    });
    expect(container.querySelectorAll('.cd-default .ltext').length).toBe(1);
    expect(container.querySelector('.cd-default .cd-ref')).toBeNull();
  });
});

// ── THE FIT-LOOP CONTRACT IS NOT THE ONLY THING THE DIGITS/LABEL BOXES CARRY —
// AND THAT SECOND CONTRACT NEEDS ITS OWN PIN.
//
// `d4fccb4` joined the fit loop; `59d591a` (self-review, same task) found that the
// brief's own reasoning for dropping `.verse`/`.countdown`/`.reference` did not
// hold against the real stylesheet — `.lfit`'s own CSS restates none of
// `.countdown`'s tabular-nums/weight/tight-leading/`white-space: nowrap`, none of
// `.countdown.warn`'s last-minute red pulse, and none of `.reference`'s
// font-weight:600 — and put the classes back. That fix was verified once with a
// throwaway test, deleted before committing, so nothing in the repo would catch a
// future edit quietly dropping the class again. These assertions are that pin —
// on `classList`, not on source text, so they follow the rendered DOM rather than
// how the class happens to be spelled in the template literal.
describe('the retained classes the self-review restored are pinned, not just remembered', () => {
  it('the digits element keeps .countdown alongside .lfit', () => {
    const container = mount({
      template: timerTemplate,
      content: { kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() + 60000 },
    });
    const digits = container.querySelector('.cd-default .cd-digits .lfit');
    expect(digits.classList.contains('lfit')).toBe(true);
    expect(digits.classList.contains('countdown'), 'tabular-nums, weight, tight leading and single-line nowrap live on .countdown and are not restated inline').toBe(true);
  });

  it('the label element keeps .reference alongside .lfit', () => {
    const container = mount({
      template: timerTemplate,
      content: { kind: 'countdown', reference: 'Service begins in', countdown_to: Date.now() + 60000 },
    });
    const label = container.querySelector('.cd-default .cd-ref .lfit');
    expect(label.classList.contains('lfit')).toBe(true);
    expect(label.classList.contains('reference'), 'font-weight:600 lives on .reference and is not restated inline').toBe(true);
  });

  it('carries .warn on the digits element inside the last-minute window, alongside .countdown', () => {
    // COUNTDOWN_WARN_MS is 60s with no explicit total (layers.js::countdownWarning);
    // 30s left is inside the window, 5 minutes left is not.
    const warn = mount({
      template: timerTemplate,
      content: { kind: 'countdown', reference: 'x', countdown_to: Date.now() + 30_000 },
    });
    const warnDigits = warn.querySelector('.cd-default .cd-digits .lfit');
    expect(warnDigits.classList.contains('warn')).toBe(true);
    expect(warnDigits.classList.contains('countdown')).toBe(true);
    app.$destroy();
    host.remove();

    const calm = mount({
      template: timerTemplate,
      content: { kind: 'countdown', reference: 'x', countdown_to: Date.now() + 300_000 },
    });
    const calmDigits = calm.querySelector('.cd-default .cd-digits .lfit');
    expect(calmDigits.classList.contains('warn')).toBe(false);
  });
});

// A TICKER IS NOT EXEMPT FROM BEING MEASURED.
//
// The crawl renders INSTEAD OF `.content`, and the region fitter queries
// `.slide .content`. So an announcement in ticker mode found zero boxes, the
// loop ran zero times, and `lastFitScale` stayed at 1 — a template reporting a
// perfect fit with its fixed label shoving the body clean off the band. The
// label is `white-space: nowrap` at raw `refSize`, so it never wraps and never
// clips; it just takes the room.
//
// This file has no `@testing-library/svelte` dependency (see the note at the
// top), so `render(...)` is `mount(...)` here, and `onFit`'s report is async
// (`scheduleFit` → `requestAnimationFrame` → `verifyFit`'s own `nextFrame`), so
// the first case awaits a real settle rather than reading `scales` synchronously.
//
// THE BRIEF'S FIRST CASE DOES NOT ACTUALLY CATCH THE DEFECT IT NAMES, AND THE
// SECOND TEST BELOW IS WHAT DOES. `verifyFit` runs unconditionally after
// `fitText` — it is not gated on how many boxes were found — so with ZERO
// `.content` boxes, `overflowing()` over an empty `fitBoxes()` array is `false`
// (`Array.prototype.some` on `[]`), `fittedWithTheRealFont()` returns `true`
// when `fitBoxes()[0]` is `undefined`, and `needsRefit` then answers `false`.
// `report()` still runs and `onFit` still fires — with the untouched
// `lastFitScale: 1`, `legible: true`. So "reports a fit" already PASSED before
// this task's fix, on the code exactly as the previous commit left it: the
// silence the brief describes is really a false "perfect fit", not an absent
// report, and a test asserting only that some report arrived cannot tell those
// apart. Verified directly: reverting just the `fitTicker()` line inside
// `fitText` (Step 5) leaves this first case green. Kept anyway, because "the
// ticker reports through the same reporter" is still a real and worthwhile
// contract — it is just not, on its own, proof of the fix.
const noticeTemplate = {
  id: 2,
  name: 'Notice',
  layout: { regions: ['verse_text', 'reference'], align: 'center' },
  style: { scroll: true, refSize: '2', verseSize: '3' },
};
const settle = (ms = 200) => new Promise((r) => setTimeout(r, ms));

describe('ticker mode is measured', () => {
  it('reports a fit for the ticker, not the silence of an empty query', async () => {
    const scales = [];
    mount({
      template: noticeTemplate,
      content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic after the second service.' },
      onFit: (info) => scales.push(info),
    });
    await settle();
    expect(scales.length, 'ticker mode must report through the same reporter as every other mode').toBeGreaterThan(0);
  });

  it('shrinks the label and reports the real scale, instead of the untouched 1', async () => {
    // jsdom lays out nothing, so the label's `clientWidth` and `scrollWidth`
    // both read 0 by default — `fitTicker` bails out at `budget <= 0` before
    // ever touching the label, same as the unfixed code, and this case alone
    // would prove nothing. Force real geometry instead, the same way
    // `cardfit.test.js`'s `clipBoxes` forces an overflow onto `.ltext`.
    //
    // RULING 2 covered: `fitTicker` now reads the budget straight off the
    // label's OWN `clientWidth` (the box `.ticker-label { max-width: 45% }`
    // genuinely constrains in a real browser), not a JS-recomputed
    // `band.clientWidth * 0.45` — so the stub sits directly on the element
    // whose box the stylesheet actually caps.
    //
    // RULING 1 also touches this case: the base is now `refSize / 100 *
    // stageEl.clientWidth` (the template's declared size), not a computed
    // style read-back, so `.stage`'s own `clientWidth` needs a real value too
    // or the base is 0 and `fitTicker` never gets past its own `!base` guard.
    let seen = null;
    const container = mount({
      template: noticeTemplate,
      content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic after the second service.' },
      onFit: (info) => (seen = info),
    });
    const stage = container.querySelector('.stage');
    const label = container.querySelector('.ticker-label');
    Object.defineProperty(stage, 'clientWidth', { value: 1000, configurable: true });
    Object.defineProperty(label, 'clientWidth', { value: 90, configurable: true });
    Object.defineProperty(label, 'scrollWidth', { value: 500, configurable: true });
    // A label that never satisfies `overflowing` keeps re-fitting up to
    // `MAX_REFIT` (font-readiness retries included), each retry adding its own
    // `setTimeout`, so this needs more than the default settle.
    await settle(600);
    expect(
      label.style.fontSize.endsWith('px'),
      'the label must have been measured and resized in px, not left at its declared cqw'
    ).toBe(true);
    expect(
      seen?.scale,
      'a label overflowing its CSS-capped budget must pull the reported scale down from the untouched 1'
    ).toBeLessThan(1);
  });

  it('regrows toward the declared size on a later, wider pass instead of ratcheting down from its own last write', async () => {
    // RULING 1's reproduction. `fitSig()` folds the stage's rounded w×h into
    // every region-mode signature (`:469` in the brief's line numbering), and
    // the label is NOT rebuilt for a geometry-only change — `{#key slideKey}`
    // keys on content, not size — so an ordinary window RESIZE with the same
    // announcement on screen still calls `fitText()` → `fitTicker()` again for
    // the very same `<span>`.
    //
    // A STATIC `scrollWidth` stub (as used above) cannot distinguish a
    // growback fix from a ratchet: it never changes when the applied
    // font-size does, so a second pass has nothing real to react to either
    // way. This one uses a GETTER that recomputes from the label's OWN
    // current inline font-size — real text does exactly this — so each pass
    // is a genuine shrink-to-fit convergence, and only the source of "the
    // declared size" (the template's `refSize`, vs. the DOM's last write)
    // can tell the two implementations apart.
    const CHARS_WIDTH_PER_PX = 40; // an arbitrary but consistent glyphs-per-px stand-in
    let seen = null;
    const content = { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic after the second service.' };
    const container = mount({
      template: noticeTemplate,
      content,
      onFit: (info) => (seen = info),
    });
    const stage = container.querySelector('.stage');
    const label = container.querySelector('.ticker-label');
    Object.defineProperty(label, 'scrollWidth', {
      configurable: true,
      get() {
        return CHARS_WIDTH_PER_PX * (parseFloat(label.style.fontSize) || 0);
      },
    });

    // PASS 1 — a narrow stage. `noticeTemplate`'s `refSize` is `'2'`, so the
    // declared base is 2px there; the label's CSS-capped budget (45% of a
    // narrow band) is stubbed to 45px, well under the label's natural 80px
    // (`40 × 2`) at that base, so a real shrink is forced.
    Object.defineProperty(stage, 'clientWidth', { value: 100, configurable: true });
    Object.defineProperty(stage, 'clientHeight', { value: 60, configurable: true });
    Object.defineProperty(label, 'clientWidth', { value: 45, configurable: true });
    await settle(600);
    const firstPx = parseFloat(label.style.fontSize);
    expect(seen?.scale, 'the first, narrow pass must have shrunk the label').toBeLessThan(1);
    expect(
      firstPx,
      'a real shrink-to-fit convergence must land at or under the 45px budget'
    ).toBeLessThanOrEqual(45);

    // PASS 2 — the SAME announcement, a much wider stage (an operator
    // widening an OBS source back out mid-service). Re-setting `content` to a
    // NEW object carrying the SAME field values changes nothing `slideKey`
    // reads — the `<span>` stubbed above is not rebuilt, which is the whole
    // point — but it does trigger Svelte's own update cycle, which is what
    // schedules the next `runFit`. The stage's new, much larger w×h is what
    // actually moves `fitSig()` and forces that next fit to happen at all.
    Object.defineProperty(stage, 'clientWidth', { value: 1000, configurable: true });
    Object.defineProperty(stage, 'clientHeight', { value: 600, configurable: true });
    Object.defineProperty(label, 'clientWidth', { value: 450, configurable: true });
    app.$set({ content: { ...content } });
    await tick();
    await settle(600);
    const secondPx = parseFloat(label.style.fontSize);
    expect(
      secondPx,
      'a widened band must grow the label back toward its declared size, not stay pinned to its previous px write'
    ).toBeGreaterThan(firstPx * 2);
  });

  it('queries the ticker label as a fit box', () => {
    const container = mount({
      template: noticeTemplate,
      content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic.' },
    });
    expect(container.querySelector('.ticker-label')).toBeTruthy();
    expect(container.querySelector('.slide .content'), 'ticker renders instead of .content').toBeFalsy();
  });

  it('adds the label to fitBoxes(), so overflowing() can see a genuinely CSS-capped label even when fitTicker itself never touches it', async () => {
    // RULING 2 covered, the `fitBoxes()` half specifically. `fitTicker`'s own
    // budget check is `label.clientWidth` (Ruling 2), and jsdom leaves that at
    // its default 0 with nothing stubbed — so `budget <= 0` bails before
    // `fitTicker` ever writes to the label, isolating the OTHER half of
    // Step 3 cleanly: `overflowing()` (which `verifyFit` reads to decide the
    // `clipped` verdict) is built from `fitBoxes()`, so a label whose own box
    // overflows can only be SEEN there if `fitBoxes()` actually queries
    // `.slide .ticker-label`.
    //
    // The stubbed `scrollWidth` against an unstubbed (0) `clientWidth` is
    // exactly the shape `.ticker-label`'s own `max-width: 45%; overflow:
    // hidden` (Ruling 2's CSS) makes a real browser produce for a label wider
    // than its cap — not a state the CSS cannot reach, the way an uncapped
    // `flex: 0 0 auto` label (where `clientWidth` always equals `scrollWidth`)
    // used to make this unreachable in production, which was this test's
    // original defect.
    let seen = null;
    const container = mount({
      template: noticeTemplate,
      content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic after the second service.' },
      onFit: (info) => (seen = info),
    });
    const label = container.querySelector('.ticker-label');
    Object.defineProperty(label, 'scrollWidth', { value: 500, configurable: true });
    await settle(600);
    expect(
      seen?.clipped,
      'the ticker label must be queried by fitBoxes(), or overflowing() cannot see it'
    ).toBe(true);
  });
});

// THE PLATE IS A BOX, NOT A SUGGESTION.
//
// `.content` clips at `max-height: 92%`; `.content.panel` set `overflow:
// visible` and took the clip away. `overflowing()` reads `scrollHeight >
// clientHeight`, which an unclipped box does not report — so the one mode
// chosen for a hard-to-read background was also the one mode whose overflow the
// fitter could not detect. Asserted on the stylesheet the component ships,
// because jsdom does not lay out.
//
// Comments are stripped from the extracted rule before matching. The
// declaration is the only thing under test — a `/* … */` comment describing
// the history of this rule is free to say "overflow: visible" (which is
// exactly what the clearest explanation of this fix needs to say) without
// that prose deciding CI's verdict on its own.
describe('the contrast panel clips', () => {
  const stripComments = (css) => codeOnly(css);

  it('does not set overflow: visible', async () => {
    const src = await import('node:fs').then((fs) =>
      fs.readFileSync('src/lib/TemplateRender.svelte', 'utf8'),
    );
    const block = src.slice(src.indexOf('.content.panel {'));
    const rule = stripComments(block.slice(0, block.indexOf('}')));
    expect(rule, '.content.panel must not remove the clip .content provides').not.toMatch(
      /overflow:\s*visible/,
    );
  });

  it('ignores the phrase when it only appears inside a comment', () => {
    const rule = stripComments(`
      /* This was \`overflow: visible\`, which took away the clip. */
      overflow: hidden;
    `);
    expect(rule).not.toMatch(/overflow:\s*visible/);
  });
});
