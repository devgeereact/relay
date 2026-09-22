// POSITION IS ITS OWN PANEL (RG-230).
//
// Stage four, the last of the approved canvas-first editor. The operator's
// reference put Arrange and the exact numbers behind a Position tab, and Relay
// had them as a collapsed group at the foot of the object's property column —
// reachable, but under everything else and shut by default.
//
// **What does NOT move here is as important as what does.** Align-to-the-frame
// already exists, on the canvas strip (`.te-alignbar`), and putting a second set
// in this panel would be two doors onto one job. The tab carries the two things
// that had no home: the stacking order, and the numbers.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const TEMPLATE = {
  id: 7,
  name: 'Probe',
  layout: {
    layers: [
      { id: 'a', type: 'text', name: 'Verse', bind: 'verse', x: 6, y: 20, w: 88, h: 50, size: 5 },
      { id: 'b', type: 'shape', name: 'Plate', x: 0, y: 0, w: 100, h: 8 },
      { id: 'c', type: 'shape', name: 'Rule', x: 0, y: 90, w: 100, h: 2 },
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
const tab = (name) =>
  [...host.querySelectorAll('.te-scope button')].find((b) => b.textContent.trim() === name);
const toPosition = async () => { tab('Position').click(); await drain(); };
const names = () => [...host.querySelectorAll('.te-lname')].map((n) => n.textContent.trim());
const posBtn = (label) =>
  [...host.querySelectorAll('.te-designbody button')].find((b) => b.textContent.trim() === label);

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async () => null);
  templates.set([structuredClone(TEMPLATE)]);
  capture.update((c) => ({ ...c, available: true }));
});
afterEach(() => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; });

describe('the third tab', () => {
  it('sits beside Object and Template', async () => {
    await open();
    for (const name of ['Object', 'Position', 'Template'])
      expect(tab(name), `no ${name} tab`).toBeTruthy();
  });

  it('carries the numbers, and the object panel no longer hides them in a shut group', async () => {
    await open();
    expect(host.querySelector('.te-geom'), 'the geometry is still in the object column').toBeNull();
    await toPosition();
    expect(host.querySelector('.te-geom'), 'the Position tab has no numbers').toBeTruthy();
  });

  it('and the stacking order, which had no home but the layer list', async () => {
    await open();
    await toPosition();
    for (const label of ['Forward', 'Backward', 'To front', 'To back'])
      expect(posBtn(label), `no ${label}`).toBeTruthy();
  });
});

describe('arrange moves the object in the stack', () => {
  it('Backward puts it behind the one under it', async () => {
    await open('b');
    expect(names()).toEqual(['Rule', 'Plate', 'Verse']);
    await toPosition();
    posBtn('Backward').click();
    await drain();
    expect(names()).toEqual(['Rule', 'Verse', 'Plate']);
  });

  it('To front takes it the whole way in one press', async () => {
    await open('a');
    await toPosition();
    posBtn('To front').click();
    await drain();
    expect(names()[0]).toBe('Verse');
  });

  it('and a press at the end of the stack changes nothing', async () => {
    await open('c');
    await toPosition();
    const before = names();
    posBtn('Forward').click();
    await drain();
    expect(names()).toEqual(before);
  });
});

describe('what it deliberately does not carry', () => {
  it('no second align-to-the-frame — the canvas strip already has one', async () => {
    await open();
    await toPosition();
    expect(host.querySelector('.te-alignbar'), 'the canvas strip lost its align').toBeTruthy();
    for (const label of ['Top', 'Bottom', 'Left', 'Right'])
      expect(posBtn(label), `${label} is a second door onto the canvas strip`).toBeUndefined();
  });

  it('and no Rotate, because nothing downstream renders one', async () => {
    // A field that moves a number no screen reads is the defect DECISIONS §69
    // closed seven controls of. When the renderer grows rotation, it belongs
    // here — and the panel says so rather than leaving a gap to be wondered at.
    await open();
    await toPosition();
    const panel = host.querySelector('.te-designbody').textContent;
    expect(panel.toLowerCase()).toContain('rotate');
    expect([...host.querySelectorAll('.te-designbody input')].some((i) => /rotate/i.test(i.id))).toBe(false);
  });
});
