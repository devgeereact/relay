// THE CONTROLS COME TO THE OBJECT (RG-229).
//
// Stage three of the approved canvas-first editor. The operator's own reference
// was a floating bar sitting above the thing you have selected — every editor of
// this kind has one, and Relay made you cross the window to a column on the
// right for the two or three properties you touch on every layer.
//
// **It is not a second set of controls.** Every button here writes through the
// same `set`/`num` the inspector writes through, so a change made on the canvas
// and a change made in the panel are the same change, and the panel shows it
// immediately. A floating bar with its own handlers would be the twin door this
// repository keeps deleting.
//
// **It carries only what the selected kind can do.** A control that does nothing
// for a shape is absent rather than greyed: an operator concludes the app is
// broken, which is RG-223 stated as a rule.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const TEMPLATE = {
  id: 7,
  name: 'Probe',
  layout: {
    layers: [
      { id: 'a', type: 'text', name: 'Verse', bind: 'verse', x: 6, y: 40, w: 88, h: 30, size: 5.2, color: '#ffffff', align: 'center' },
      { id: 'b', type: 'shape', name: 'Plate', x: 0, y: 0, w: 100, h: 8, fill: '#101319' },
      { id: 'c', type: 'text', name: 'High', bind: 'reference', x: 10, y: 2, w: 40, h: 8, size: 2 },
    ],
  },
  style: {},
};

const { default: TemplateEditor } = await import('./TemplateEditor.svelte');
const { templates, capture } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => { for (let i = 0; i < n; i += 1) await settle(); };

let host, cmp;
async function open(layerId = 'a') {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateEditor({ target: host, props: { templateId: 7, layerId } });
  await drain();
  return host;
}
const bar = () => host.querySelector('.te-float');
const btn = (label) =>
  [...(bar()?.querySelectorAll('button') ?? [])].find((b) => (b.getAttribute('aria-label') || '') === label);
const pick = async (name) => {
  [...host.querySelectorAll('.te-layer')].find((r) => r.textContent.includes(name)).click();
  await drain();
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async () => null);
  templates.set([structuredClone(TEMPLATE)]);
  capture.update((c) => ({ ...c, available: true }));
});
afterEach(() => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; });

describe('the bar sits on the selection', () => {
  it('appears when something is selected and not before', async () => {
    await open('a');
    expect(bar(), 'no floating bar').toBeTruthy();
    // The Select tool drops the selection; the bar has nothing to sit on.
    [...host.querySelectorAll('.te-tools button')].find((b) => b.getAttribute('aria-label') === 'Select').click();
    await drain();
    expect(bar(), 'a bar floating over nothing').toBeNull();
  });

  it('is anchored to the object it is about', async () => {
    await open('a');
    // Centred on the layer, above it: x + w/2 = 50.
    expect(bar().getAttribute('style')).toMatch(/left:\s*50%/);
    expect(bar().getAttribute('style')).toMatch(/top:\s*40%/);
  });

  it('and drops BELOW a layer near the top, rather than off the slide', async () => {
    // A bar rendered above a layer at y=2 is a bar outside the artboard, which
    // on a scrolling pane is a bar nobody can reach.
    await open('c');
    expect(bar().classList.contains('below'), 'the bar hung off the top of the slide').toBe(true);
  });
});

describe('what it carries is what the kind can do', () => {
  it('a text object gets type controls', async () => {
    await open('a');
    // The ALIGNMENT labels are the panel's own (`TEXT_ALIGN_LABEL`), word for
    // word, because they are the same command in a second place. The panel's
    // comment records why they are phrased as they are: "Align left" was also
    // the canvas strip's name for MOVING the object, and two identical names for
    // two different commands is a defect you can only hear.
    for (const label of ['Bigger', 'Smaller', 'Bold', 'Italic', 'Text aligned left', 'Text centred', 'Text aligned right'])
      expect(btn(label), `no ${label} on a text object`).toBeTruthy();
  });

  it('a shape gets none of them — absent, never greyed', async () => {
    await open('a');
    await pick('Plate');
    expect(bar(), 'the bar went away for a shape').toBeTruthy();
    expect(btn('Bold'), 'a shape was offered Bold').toBeUndefined();
    expect(btn('Bigger'), 'a shape was offered a type size').toBeUndefined();
  });
});

describe('it writes the same model the panel writes', () => {
  it('the size stepper moves the layer, and the panel agrees at once', async () => {
    await open('a');
    btn('Bigger').click();
    await drain();
    const inPanel = host.querySelector('#te-size');
    expect(Number(inPanel.value), 'the panel and the bar disagree about the size').toBeGreaterThan(5.2);
    // And the canvas foot, which reads the template, moved with it.
    expect(host.querySelector('.te-botbar').textContent).toContain(inPanel.value);
  });

  it('align writes the same key the panel does', async () => {
    await open('a');
    btn('Text aligned left').click();
    await drain();
    const on = [...host.querySelectorAll('.te-seg .te-segbtn')].find((b) => b.classList.contains('on'));
    expect(on.getAttribute('aria-label'), 'the panel still shows the old alignment').toMatch(/left/i);
  });

  it('and Bold is a toggle, not a one-way switch', async () => {
    await open('a');
    btn('Bold').click();
    await drain();
    expect(btn('Bold').getAttribute('aria-pressed')).toBe('true');
    btn('Bold').click();
    await drain();
    expect(btn('Bold').getAttribute('aria-pressed')).toBe('false');
  });
});
