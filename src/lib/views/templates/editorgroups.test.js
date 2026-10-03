// THE DESIGN PANEL IS FOUR JOBS, NOT ONE LIST (RG-217).
//
// The operator's words: *"Lets work on the Template Edit Workspace as it too
// busy and not user friendly... Can you restrategise on this and draw a clear
// guide"*.
//
// The problem was never the NUMBER of controls. It is that one column carried
// the object's name, its duplicate/reset/delete, X/Y/W/H, three centre buttons,
// content, font, size, colour, style link, align, v-align, caps, line height,
// spacing, shadow, scale, line transform, italic and scroll — and then, under
// the same scroll, the TEMPLATE's name and the five content kinds it renders.
// Somebody opening this screen is doing one of four jobs and was asked to hold
// all four at once, with a fifth question at the bottom that is not about the
// thing they selected at all.
//
// The guide, which is `docs/superpowers/plans/2026-09-21-stage-templates-media.md` §3:
//
//   1 · WHAT is this?      binding and name. Always visible, never collapsed.
//   2 · HOW does it read?  font, size, colour, align, caps, line height,
//                          spacing. Open by default — every layer, every time.
//   3 · WHERE does it sit? X/Y/W/H and the centres. COLLAPSED: dragging is how
//                          this is normally done, and the numbers are for the
//                          one time two layers must agree exactly.
//   4 · Anything else      shadow, scale, line transform, italic, scroll.
//                          COLLAPSED — controls touched once a year, currently
//                          at the same visual weight as Colour.
//
// And the template's own two facts leave the object panel entirely, because they
// answer a different question from everything above them.
//
// **No rendered output changes.** Every control writes exactly what it wrote
// before; this is a regrouping of one column. The cases below are about which
// group a control is in and what is open, not about what any of them do — those
// tests already exist and must stay green beside these.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const TEMPLATE = {
  id: 7,
  name: 'Probe',
  layout: {
    layers: [
      { id: 'a', type: 'text', name: 'One', bind: 'verse', x: 5, y: 5, w: 40, h: 10, size: 3 },
      { id: 'b', type: 'text', name: 'Two', bind: 'reference', x: 5, y: 20, w: 40, h: 8, size: 2 },
    ],
  },
  style: {},
};

const { default: TemplateEditor } = await import('./TemplateEditor.svelte');
const { templates, capture } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => { for (let i = 0; i < n; i++) await settle(); };

let host, cmp;
async function open() {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateEditor({ target: host, props: { templateId: 7, layerId: 'a' } });
  await drain();
  return host;
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async () => null);
  templates.set([structuredClone(TEMPLATE)]);
  capture.update((c) => ({ ...c, available: true }));
});
afterEach(() => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; });

// A group is titled by a HEAD (collapsible) or a LABEL (the one that is not).
/**
 * Put the panel on STYLE, where the look controls now live (RG-231).
 *
 * The approved canvas keeps an object's identity and its look apart: Content
 * answers *what is this*, Style answers *how does it read*. The groups below did
 * not change — only which tab they sit behind.
 */
async function styleTab(el) {
  [...el.querySelectorAll('.te-scope button')].find((b) => b.textContent.trim() === 'Style')?.click();
  await drain(2);
}

const group = (el, name) =>
  [...el.querySelectorAll('.te-group')].find((g) =>
    (g.querySelector('.te-grouphead') ?? g.querySelector('.te-grouplbl'))
      ?.textContent.trim()
      .toLowerCase()
      .includes(name),
  );
const isOpen = (g) => g?.querySelector('.te-grouphead')?.getAttribute('aria-expanded') === 'true';
const has = (el, sel) => Boolean(el?.querySelector(sel));

describe('the four groups, and what each one opens as', () => {
  it('a text object is grouped, not listed', async () => {
    const el = await open();
    // The four groups are behind TWO tabs now (RG-231): Content answers *what is
    // this*, Style answers *how does it read* and *anything else*, and *where
    // does it sit* went to its own tab in RG-230.
    expect(group(el, 'what is this'), 'the identity left the Content tab').toBeTruthy();
    await styleTab(el);
    for (const g of ['how does it read', 'anything else'])
      expect(group(el, g), `no “${g}” group`).toBeTruthy();
    expect(group(el, 'where does it sit'), 'the geometry is in two places').toBeFalsy();
  });

  it('“how does it read” is open, because it is the one used on every layer', async () => {
    const el = await open();
    await styleTab(el);
    expect(isOpen(group(el, 'how does it read'))).toBe(true);
    expect(has(group(el, 'how does it read'), '#te-font')).toBe(true);
    expect(has(group(el, 'how does it read'), '#te-size')).toBe(true);
    expect(has(group(el, 'how does it read'), '#te-col')).toBe(true);
  });

  it('“anything else” starts closed, and opens on a press', async () => {
    const el = await open();
    await styleTab(el);
    const more = group(el, 'anything else');
    expect(isOpen(more), 'the once-a-year controls are open by default').toBe(false);
    expect(has(more, '#te-sh'), 'a closed group still rendered its body').toBe(false);
    more.querySelector('.te-grouphead').click();
    await drain(2);
    expect(isOpen(group(el, 'anything else'))).toBe(true);
    expect(has(group(el, 'anything else'), '#te-sh')).toBe(true);
  });

  it('“what” is not collapsible at all — everything else is about an object whose job is chosen', async () => {
    const el = await open();
    const what = group(el, 'what is this');
    expect(what.querySelector('.te-grouphead')).toBeNull();
    expect(has(what, '#te-bind'), 'the binding is not in the first group').toBe(true);
    expect(has(what, '#te-oname'), 'the name is not in the first group').toBe(true);
  });

  it('the rare controls are in “anything else”, not beside Colour', async () => {
    const el = await open();
    await styleTab(el);
    group(el, 'anything else').querySelector('.te-grouphead').click();
    await drain(2);
    const more = group(el, 'anything else');
    for (const id of ['#te-sh', '#te-fit', '#te-lt'])
      expect(has(more, id), `${id} is still at the weight of Colour`).toBe(true);
    expect(more.textContent).toMatch(/Italic/);
    expect(more.textContent).toMatch(/Scroll/);
  });
});

describe('the template’s own facts are not a section of a layer', () => {
  it('they are behind their own tab, not under the object’s controls', async () => {
    const el = await open();
    expect(el.querySelector('#te-name'), 'the template name is still in the object panel').toBeNull();
    const tab = [...el.querySelectorAll('.te-scope button')].find((b) => /template/i.test(b.textContent));
    expect(tab, 'nothing switches the panel to the template').toBeTruthy();
    tab.click();
    await drain(2);
    expect(el.querySelector('#te-name'), 'the template tab shows no name field').toBeTruthy();
    expect(el.textContent).toMatch(/Content this template renders/);
  });

  it('and switching back returns to the object that was selected', async () => {
    const el = await open();
    const [objTab, tplTab] = [...el.querySelectorAll('.te-scope button')];
    tplTab.click();
    await drain(2);
    expect(el.querySelector('#te-bind')).toBeNull();
    objTab.click();
    await drain(2);
    expect(el.querySelector('#te-bind'), 'the object panel did not come back').toBeTruthy();
  });
});
