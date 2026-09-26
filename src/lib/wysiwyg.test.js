// WHAT IS IN THE EDITOR IS WHAT IS ON THE OUTPUT — PER BINDING (RG-218).
//
// The operator's words: *"Whats in the templete edit should be whats on the
// output not something different entirely like the positioning and screen
// spaces and all"*.
//
// The architecture is already right and that is worth saying first: the editor
// preview and the wall are the SAME component, so a whole class of divergence
// cannot happen here. What went wrong was per BINDING — a layer whose declared
// properties the renderer quietly overrode. The Stage Timer was one (RG-212's
// sibling: the editor showed a Size control and the rail computed its figures
// entirely from the container, so dragging the control moved nothing), and the
// Stage Message was another (a full-bleed panel painted over the layer the
// designer had placed, always).
//
// So this is an audit with a test rather than a feature. For every binding in
// `layers.js::BINDINGS`, does the renderer honour the four properties the
// editor offers for it — the box, the size, the colour and the typeface?
//
// **A binding that cannot honour one is not a failure to fix here; it is a
// control that must not be shown for it.** That is the rule the audit enforces,
// and the exemptions below each say which property and why. A control that does
// nothing is worse than an absent one, because the operator concludes the app
// is broken and goes looking for the setting that "really" works.
import { describe, it, expect } from 'vitest';
import { tick } from 'svelte';
import { BINDINGS } from './layers.js';

const TemplateRender = (await import('./TemplateRender.svelte')).default;

/** Distinctive values: nothing here is a default, so a fallback cannot pass. */
const BOX = { x: 11, y: 23, w: 47, h: 19 };
const SIZE = 4.7;
const COLOUR = '#c83a7b';
/** The same colour as jsdom serialises it — see the note at the assertion. */
const COLOUR_RGB = 'rgb(200, 58, 123)';
const FONT = 'Fraunces';

const layerFor = (bind) => ({
  id: 'probe',
  type: bind === 'countdown' ? 'timer' : 'text',
  name: 'Probe',
  bind,
  ...BOX,
  size: SIZE,
  color: COLOUR,
  font: FONT,
  align: 'right',
});

/** Content that populates every binding that rides on the fired content. */
const CONTENT = {
  reference: 'John 3:16',
  text: 'For God so loved the world',
  translation: 'KJV',
  next_text: 'And the Word became flesh',
  next_reference: 'John 1:14',
  stage_note: 'Wrap up in five',
  countdown_to: Date.now() + 120_000,
};

const TIMERS = [
  { id: 1, label: 'Sermon', countdown_to: Date.now() + 240_000, countdown_from: Date.now() - 60_000 },
];

let host;
let app;

async function render(bind) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({
    target: host,
    props: {
      template: { layout: { layers: [layerFor(bind)] }, style: {} },
      content: CONTENT,
      stageMessage: 'Five minutes left',
      programme: TIMERS,
    },
  });
  await tick();
  await tick();
  return host;
}

function teardown() {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
}

/**
 * THE EXEMPTIONS, each naming the property and the reason.
 *
 * `null` means the binding honours all four. Anything else is a promise that the
 * editor does not offer that control for that binding, which is the other half
 * of this rule.
 */
const EXEMPT = {
  // The rail is a SET of figures, not one. `size` is its base figure size
  // (RG-212 wired the control that had never moved anything) and the container
  // caps it, so a figure may shrink to fit its box and never grow past what the
  // designer asked for. It is measured through `--lp-sz`, not `data-base`,
  // because the element carrying the digits is not the element carrying the box.
  programme: 'a set of figures, measured through --lp-sz',
  // Its own hub frame, not content, and the renderer holds it in state. A quiet
  // message paints in the layer; an ALERT is a full-bleed panel over everything,
  // which is the point of an alert and is asserted in `stagealerttemplate.js`.
  stage_message: 'a hub frame, and an alert is deliberately full-bleed',
};

describe('every binding honours the box, the size, the colour and the typeface', () => {
  for (const b of BINDINGS) {
    const why = EXEMPT[b.key];
    it(`${b.key}${why ? ` — exempt: ${why}` : ''}`, async () => {
      try {
        const el = await render(b.key);
        if (why) {
          // An exemption still has to render SOMETHING for its layer, or the
          // exemption is hiding an absence.
          expect(el.textContent.trim().length, `${b.key} rendered nothing at all`).toBeGreaterThan(0);
          return;
        }
        const fit = el.querySelector('.lfit');
        expect(fit, `${b.key} rendered no text element`).toBeTruthy();
        // THE BOX — percentages of the frame, exactly as the editor writes them.
        const box = fit.closest('.ltext');
        const style = box.getAttribute('style');
        expect(style, `${b.key} ignores x`).toContain(`left:${BOX.x}%`);
        expect(style, `${b.key} ignores y`).toContain(`top:${BOX.y}%`);
        expect(style, `${b.key} ignores w`).toContain(`width:${BOX.w}%`);
        expect(style, `${b.key} ignores h`).toContain(`height:${BOX.h}%`);
        // THE SIZE, as DECLARED. `fitLayers` refines this same property at run
        // time, so what is asserted is the designed size the fit starts from —
        // which is the number the editor's control writes.
        expect(fit.getAttribute('data-base'), `${b.key} ignores size`).toBe(String(SIZE));
        // Read through the style OBJECT, not the attribute string: jsdom
        // normalises a hex colour to `rgb()` and adds a space after each colon,
        // so a substring test on the attribute asserts the serialiser rather
        // than the renderer.
        expect(fit.style.color, `${b.key} ignores colour`).toBe(COLOUR_RGB);
        expect(fit.style.fontFamily, `${b.key} ignores the typeface`).toContain(FONT);
        expect(fit.style.textAlign, `${b.key} ignores align`).toBe('right');
      } finally {
        teardown();
      }
    });
  }

  it('the assertions are not vacuous — an exempt binding genuinely fails them', async () => {
    // THE GUARD ON THE AUDIT. Eleven bindings passing is only news if the check
    // can fail, and this file asserts a clean result rather than fixing a bug,
    // so there is no red state on the way in to prove it. The programme rail IS
    // that red state: it renders no `.lfit` at all, which is exactly why it is
    // exempt and measured through `--lp-sz` instead.
    try {
      const el = await render('programme');
      expect(el.querySelector('.lfit'), 'the rail grew a .lfit and the exemption is stale').toBeNull();
      expect(el.querySelector('.lprog'), 'the rail rendered nothing').toBeTruthy();
      // …and it honours its own size through the variable the exemption names,
      // which is the half RG-212 closed.
      expect(el.querySelector('.lprog').getAttribute('style')).toContain('--lp-sz:');
    } finally {
      teardown();
    }
  });

  it('and the audit covers every binding there is, not a list somebody typed', () => {
    // The failure this guards against is a binding added next year with a
    // control that moves nothing, and an audit that never looked at it.
    expect(BINDINGS.length).toBeGreaterThan(10);
    for (const k of Object.keys(EXEMPT))
      expect(BINDINGS.map((b) => b.key), `${k} is exempt from nothing`).toContain(k);
  });
});
