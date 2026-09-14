// Phase 3 — the inspector's destructive control, on the surface it lives on.
//
// `layerops.test.js` holds the arithmetic. This holds the thing arithmetic
// cannot: that deleting an object takes TWO presses. The button is 20px wide and
// sits between Lock and Visibility in a row of five; it used to remove the
// object on the first click. Undo exists, and "your work is one keystroke away"
// is not the same as "you did not lose it" when the panel is being used against
// the clock.
//
// The no-native-dialog half of rule 41 is already held by `hardrules.test.js`
// across every file, so it is not repeated here: a second copy of a rule is one
// more place for it to be weakened when it fails for an unrelated reason.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const TEMPLATE = {
  id: 7,
  name: 'Nocturne',
  layout: {
    layers: [
      { id: 'bg1', type: 'background', name: 'Background', x: 0, y: 0, w: 100, h: 100, fill: '#101010' },
      { id: 'tx1', type: 'text', name: 'Verse', bind: 'verse', x: 10, y: 20, w: 80, h: 40, size: 6 },
    ],
  },
  style: {},
};

const { default: TemplateEditor } = await import('./views/templates/TemplateEditor.svelte');
const { templates } = await import('./stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));

let host;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new TemplateEditor({ target: host, props: { templateId: 7 } });
}

/** The delete button in the properties pane — the one with a word on it. */
const deleteButton = () =>
  [...host.querySelectorAll('.te-objacts button')].find((b) => /delete/i.test(b.textContent));

const layerNames = () => [...host.querySelectorAll('.te-objtab')].map((b) => b.textContent.trim());

describe('the object inspector', () => {
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

  it('lists the slide\'s objects as tabs', async () => {
    mount();
    await settle();
    expect(layerNames()).toEqual(['Background', 'Verse']);
  });

  it('deleting takes two presses, and the first one only arms it', async () => {
    mount();
    await settle();
    const tabs = [...host.querySelectorAll('.te-objtab')];
    tabs[1].click();
    await settle();

    const del = deleteButton();
    expect(del, 'the properties pane offers a delete').toBeTruthy();

    del.click();
    await settle();
    expect(layerNames(), 'the first press deletes nothing').toEqual(['Background', 'Verse']);
    expect(deleteButton().textContent).toMatch(/sure/i);

    deleteButton().click();
    await settle();
    expect(layerNames(), 'the second press deletes it').toEqual(['Background']);
  });

  it('duplicating gives the slide another object, named as a copy', async () => {
    mount();
    await settle();
    [...host.querySelectorAll('.te-objtab')][1].click();
    await settle();

    [...host.querySelectorAll('.te-objacts button')]
      .find((b) => /duplicate/i.test(b.textContent))
      .click();
    await settle();
    expect(layerNames()).toEqual(['Background', 'Verse', 'Verse copy']);
  });

});
