// THE LAYER LIST — the one surface that drives paint order.
//
// `layerops.test.js` holds the arithmetic. This holds what the arithmetic cannot:
// that the list an operator reads is the stack the wall paints, that a band's
// words are shown as being INSIDE their band rather than beside it, and that the
// two arrows on a row move the thing the row names.
//
// The defect this was written against: the list was the raw array, reversed, and
// the arrows swapped two adjacent entries of it. A band's words live in that
// array and are not drawn from it, so on `lowerBible` — a band and its two
// words, which is the whole template — pressing either arrow, on any row, did
// nothing at all. Every lower third the product ships has that shape.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

// The shape `layers.js::lowerBible` builds, plus one shape on top of it — a band,
// the two words it names, and something in the stack to move past them.
const TEMPLATE = {
  id: 11,
  name: 'Lower Third',
  layout: {
    layers: [
      { id: 'b1', type: 'band', name: 'Band', members: ['w1', 'w2'], x: 6, y: 74, w: 88, h: 26, top: 74, side: 6, pad: 3, lift: 3 },
      { id: 'w1', type: 'text', name: 'Verse', bind: 'verse', x: 9, y: 76, w: 82, h: 10, size: 2.6 },
      { id: 'w2', type: 'text', name: 'Reference', bind: 'reference', x: 9, y: 86, w: 82, h: 5, size: 1.5 },
      { id: 's1', type: 'shape', name: 'Plate', x: 0, y: 0, w: 100, h: 8 },
    ],
  },
  style: {},
};

const { default: TemplateEditor } = await import('./TemplateEditor.svelte');
const { templates } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));

let host;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new TemplateEditor({ target: host, props: { templateId: 11 } });
}

const rows = () => [...host.querySelectorAll('.te-layer')];
const names = () => rows().map((r) => r.querySelector('.te-lname').textContent.trim());
const rowFor = (name) => rows().find((r) => r.querySelector('.te-lname').textContent.trim() === name);
const arrow = (name, which) => rowFor(name).querySelectorAll('.te-lmini')[which === 'up' ? 0 : 1];

describe('the layer list', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('is the stack front-first, with a band\'s words beneath the band that owns them', async () => {
    mount();
    await settle();
    // Front first: the shape is drawn last, so it is at the top. The band's two
    // words follow the band, in the order the band names them — reading order,
    // which is not reversed, because a band lays its words out rather than
    // stacking them.
    expect(names()).toEqual(['Plate', 'Band', 'Verse', 'Reference']);
  });

  it('marks a word as being inside its band rather than beside it', async () => {
    mount();
    await settle();
    expect(rowFor('Verse').classList.contains('inband')).toBe(true);
    expect(rowFor('Band').classList.contains('inband')).toBe(false);
    expect(rowFor('Plate').classList.contains('inband')).toBe(false);
  });

  it('moves a stack object PAST the whole band in one press', async () => {
    mount();
    await settle();
    arrow('Plate', 'down').click();
    await settle();
    // One press, one visible step: the plate is now behind the band. It used to
    // swap with `Reference` — a layer nothing draws from the stack — so the
    // wall was identical and the next two presses looked broken too.
    expect(names()).toEqual(['Band', 'Verse', 'Reference', 'Plate']);
  });

  it('moves a word within its band, and never out of it', async () => {
    mount();
    await settle();
    arrow('Reference', 'up').click();
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Reference', 'Verse']);

    // Already first in the band: the arrow has nowhere to go, and it does not
    // get there by leaving the band.
    arrow('Reference', 'up').click();
    await settle();
    expect(names()).toEqual(['Plate', 'Band', 'Reference', 'Verse']);
  });

  it('names each kind with its own icon, from the one register', async () => {
    mount();
    await settle();
    const icon = (n) => rowFor(n).querySelector('.te-ltype').textContent.trim();
    // A band drew `T`, the text icon, in the one place an operator goes to tell
    // one kind from another.
    expect(icon('Band')).toBe('▬');
    expect(icon('Plate')).toBe('▢');
    expect(icon('Verse')).toBe('T');
  });

  it('lets an operator name an object, which the list then shows', async () => {
    mount();
    await settle();
    rowFor('Verse').click();
    await settle();
    const box = host.querySelector('#te-oname');
    expect(box, 'the selected object has a name field').toBeTruthy();
    box.value = 'Speaker';
    box.dispatchEvent(new Event('input', { bubbles: true }));
    await settle();
    expect(names()).toContain('Speaker');
  });
});

// ── THE DRAG ───────────────────────────────────────────────────────────────
//
// The move/up pair is listened for on `window`, which hears nothing that happens
// outside the window. Release the button over the desktop and `pointerup` is
// delivered somewhere else: the drag is still armed, and the object follows the
// pointer the next time it crosses the canvas with no button held. Capturing the
// pointer is what makes the gesture end where the operator ended it.
describe('direct manipulation on the canvas', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return [TEMPLATE];
      if (cmd === 'get_setting') return '';
      return null;
    });
    templates.set([structuredClone(TEMPLATE)]);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  const down = (el) => {
    const e = new MouseEvent('pointerdown', { bubbles: true, cancelable: true, clientX: 100, clientY: 100 });
    Object.defineProperty(e, 'pointerId', { value: 7 });
    el.dispatchEvent(e);
    return e;
  };

  const plateBox = () =>
    [...host.querySelectorAll('.te-hbox')].find((b) => b.getAttribute('aria-label') === 'Plate');

  it('captures the pointer, so a release outside the window still ends the drag', async () => {
    mount();
    await settle();
    // The shape's handle box. A band and its words are PLACED, not dragged —
    // dragging would write x/y/w/h and nothing draws them from x/y/w/h — so they
    // are deliberately not draggable and would prove nothing here.
    const box = plateBox();
    expect(box, 'the stack object has a handle box on the canvas').toBeTruthy();
    const captured = [];
    box.setPointerCapture = (id) => captured.push(id);
    down(box);
    expect(captured, 'the gesture is captured for the pointer that started it').toEqual([7]);
  });

  it('reads back a share of the frame, never a pixel', async () => {
    mount();
    await settle();
    const box = plateBox();
    box.setPointerCapture = () => {};
    down(box);
    await settle();
    // The board has no layout under jsdom, so a real move cannot be measured
    // here. What CAN be measured is the unit the editor stores and shows: the
    // layer's own x/y/w/h, which the renderer sizes in cqw.
    expect(host.querySelector('.te-botchip').textContent.trim()).toBe('0,0 · 100×8');
  });
});
