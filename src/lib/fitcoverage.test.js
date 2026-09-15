import { describe, it, expect, afterEach } from 'vitest';
import TemplateRender from './TemplateRender.svelte';

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

const timerTemplate = {
  id: 1,
  name: 'Timer',
  layout: { layers: [{ id: 'bg', type: 'bg', fill: '#000' }] },
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
    // jsdom lays out nothing, so `.ticker`'s `clientWidth` and the label's
    // `scrollWidth` both read 0 by default — `fitTicker` bails out at
    // `budget <= 0` before ever touching the label, same as the unfixed code,
    // and this case alone would prove nothing. Force real geometry instead, the
    // same way `cardfit.test.js`'s `clipBoxes` forces an overflow onto `.ltext`:
    // a 200px band caps the label's budget at 90px (45%), and a label stubbed to
    // 500px is well past it.
    let seen = null;
    const container = mount({
      template: noticeTemplate,
      content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic after the second service.' },
      onFit: (info) => (seen = info),
    });
    const band = container.querySelector('.ticker');
    const label = container.querySelector('.ticker-label');
    Object.defineProperty(band, 'clientWidth', { value: 200, configurable: true });
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
      'a label overflowing its 45% budget must pull the reported scale down from the untouched 1'
    ).toBeLessThan(1);
  });

  it('queries the ticker label as a fit box', () => {
    const container = mount({
      template: noticeTemplate,
      content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic.' },
    });
    expect(container.querySelector('.ticker-label')).toBeTruthy();
    expect(container.querySelector('.slide .content'), 'ticker renders instead of .content').toBeFalsy();
  });

  it('adds the label to fitBoxes(), so overflowing() can see it even when fitTicker itself is a no-op', async () => {
    // `fitTicker` bails out at `budget <= 0` when the band has no measurable
    // width (jsdom's default) and never touches the label at all — that path
    // alone would prove nothing about `fitBoxes()`. Isolate the OTHER half of
    // Step 3: `overflowing()` (which drives `verifyFit`'s `clipped` verdict and
    // the late re-look) reads `fitBoxes()`, so a label stubbed to overflow its
    // OWN box can only be seen if `fitBoxes()` actually queries `.ticker-label`.
    let seen = null;
    const container = mount({
      template: noticeTemplate,
      content: { kind: 'announce', reference: 'THIS SUNDAY', text: 'Church picnic after the second service.' },
      onFit: (info) => (seen = info),
    });
    const label = container.querySelector('.ticker-label');
    Object.defineProperty(label, 'scrollWidth', { value: 500, configurable: true });
    Object.defineProperty(label, 'clientWidth', { value: 50, configurable: true });
    await settle(600);
    expect(
      seen?.clipped,
      'the ticker label must be queried by fitBoxes(), or overflowing() cannot see it'
    ).toBe(true);
  });
});
