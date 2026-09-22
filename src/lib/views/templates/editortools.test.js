// A TOOL RAIL, NOT A PLUS MENU IN A LIST HEADER (RG-227).
//
// Stage one of the canvas-first editor the operator approved
// (`claude.ai/artifact/GHWdoiwZA2Y6aak1Vg4vNu`). Every other editor of this kind
// — ProPresenter, Canva, Keynote — puts CREATION on a rail down the left, and
// Relay hid it behind a `＋` inside the Layers pane's own header: an operator
// looking for "how do I add a picture" had to find a plus on a panel about
// something else.
//
// **The menu is not duplicated, it is MOVED.** A rail button that opened the
// same menu as the `＋` would be two doors onto one job, which is the shape this
// repository keeps deleting. The Text tool IS that menu's button now, and it
// keeps the class the existing tests hold it by — including rule 44's guarantee
// that an open menu consumes Escape rather than swallowing it, which is
// `editorlayout.test.js`'s subject and must not be lost in a rearrangement.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const TEMPLATE = {
  id: 7,
  name: 'Probe',
  layout: { layers: [{ id: 'a', type: 'text', name: 'One', bind: 'verse', x: 5, y: 5, w: 40, h: 10, size: 3 }] },
  style: {},
};

const { default: TemplateEditor } = await import('./TemplateEditor.svelte');
const { templates, capture } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => { for (let i = 0; i < n; i += 1) await settle(); };

let host, cmp;
async function open() {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateEditor({ target: host, props: { templateId: 7 } });
  await drain();
  return host;
}
const tool = (label) =>
  [...host.querySelectorAll('.te-tools button')].find((b) => b.getAttribute('aria-label') === label);
const names = () => [...host.querySelectorAll('.te-lname')].map((n) => n.textContent.trim());

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async () => null);
  templates.set([structuredClone(TEMPLATE)]);
  capture.update((c) => ({ ...c, available: true }));
});
afterEach(() => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; });

describe('the rail is where an object comes from', () => {
  it('offers a tool per kind, each named for somebody who cannot see the icon', async () => {
    await open();
    expect(host.querySelector('.te-tools'), 'there is no tool rail').toBeTruthy();
    for (const label of ['Select', 'Text', 'Shape', 'Picture', 'Band', 'Clock'])
      expect(tool(label), `no ${label} tool`).toBeTruthy();
  });

  it('a tool press puts that object on the slide', async () => {
    await open();
    expect(names()).toEqual(['One']);
    tool('Shape').click();
    await drain();
    expect(names().length, 'the Shape tool made nothing').toBe(2);
  });

  it('the new object is SELECTED, because the next thing anybody does is style it', async () => {
    await open();
    tool('Shape').click();
    await drain();
    // The inspector follows the selection, so this is what makes the rail a
    // flow rather than two separate acts.
    expect(host.querySelector('.te-layer.sel .te-lname')).toBeTruthy();
    expect(host.querySelector('.te-designfor').textContent.trim().length).toBeGreaterThan(0);
  });

  it('Select makes nothing, and DROPS the selection rather than being inert', async () => {
    // An arrow that looks pressed and does nothing is what `inventory.test.js`
    // counts as an inert control — and it counted this one on its first draft.
    // Dropping the selection is the one thing an operator wants from an arrow
    // they have just clicked, and it is what separates this tool from the rest
    // of the rail: it creates no layer.
    await open();
    host.querySelector('.te-layer').click();
    await drain();
    expect(host.querySelector('.te-layer.sel'), 'nothing was selected to drop').toBeTruthy();
    const before = names().length;

    tool('Select').click();
    await drain();
    expect(host.querySelector('.te-layer.sel'), 'the arrow did not drop the selection').toBeNull();
    expect(names().length, 'the arrow tool created a layer').toBe(before);
    expect(tool('Select').getAttribute('aria-pressed'), 'it does not read as the resting tool').toBe('true');
  });

  it('Text opens the binding menu — the SAME one, moved, not a second copy', async () => {
    // Its class is the one `editorlayout.test.js` holds the Escape guarantee by
    // (rule 44). If a rearrangement ever leaves two buttons opening this menu,
    // this is the assertion that has to be argued with.
    await open();
    expect(host.querySelectorAll('.te-addbtn'), 'two doors onto one menu').toHaveLength(1);
    expect(tool('Text').classList.contains('te-addbtn')).toBe(true);
    tool('Text').dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await drain();
    expect(host.querySelector('[role="menu"]')).toBeTruthy();
  });

  it('and the Layers pane head no longer carries a creator', async () => {
    await open();
    const head = host.querySelector('.te-layers .te-panehead');
    expect(head.querySelector('.te-addbtn'), 'the ＋ is still on the wrong panel').toBeNull();
  });
});
