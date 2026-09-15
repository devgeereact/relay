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
});
