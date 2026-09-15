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
