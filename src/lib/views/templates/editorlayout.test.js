// THE EDITOR STOPS PAINTING TEXT OVER TEXT.
//
// Three defects, all of them geometry, and geometry is the one thing this suite
// cannot see: jsdom has no layout engine, so `getBoundingClientRect` is zeros
// and a box that clips its contents reports that everything fits. The measuring
// was therefore done in Chrome against the real stylesheet, at 1280×640, on a
// twenty-four-object template, and the numbers are in the commit that landed
// this file. What is held HERE is the structure those measurements depend on —
// the classes, the anchoring and the CSS rules — so that a later tidy-up cannot
// quietly restore the construction without this going red.
//
// That distinction is the honest one and it is worth stating: a green run here
// is NOT a claim that anything fits. It is a claim that the three things that
// made it not fit are still gone.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

// `__dirname`, the way `hardrules.test.js` reads source: `import.meta.url` is
// an http URL under vitest's jsdom environment, not a file one.
const SRC = readFileSync(resolve(__dirname, 'TemplateEditor.svelte'), 'utf8');

const TEMPLATE = {
  id: 7,
  name: 'Probe',
  layout: {
    layers: [
      { id: 'a', type: 'text', name: 'One', bind: 'verse', x: 5, y: 5, w: 40, h: 10, size: 3 },
      { id: 'b', type: 'text', name: 'Two', bind: 'reference', x: 5, y: 20, w: 40, h: 8, size: 2 },
      { id: 'c', type: 'shape', name: 'Three', x: 5, y: 40, w: 40, h: 8 },
    ],
  },
  style: {},
};

const { default: TemplateEditor } = await import('./TemplateEditor.svelte');
const { templates, capture } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 4) => { for (let i = 0; i < n; i++) await settle(); };

let host, cmp;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateEditor({ target: host, props: { templateId: 7 } });
  return cmp;
}

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async () => null);
  templates.set([structuredClone(TEMPLATE)]);
  capture.update((c) => ({ ...c, available: true }));
});
afterEach(() => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; });

// ── F1 · the armed layer delete ────────────────────────────────────────────
describe('the armed layer delete is a confirm an operator can see', () => {
  it('arms rather than deleting, and says so', async () => {
    mount();
    await drain();
    const row = [...host.querySelectorAll('.te-layer')].find((r) => /One/.test(r.textContent));
    const del = row.querySelector('.te-lmini.danger');
    del.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await drain();
    expect(del.textContent.trim()).toBe('Sure?');
    expect(host.querySelectorAll('.te-layer')).toHaveLength(3);
  });

  it('marks the ROW armed, so the cluster that holds the question is not hidden', async () => {
    // `.te-lbtns` is `opacity:0` until the row is hovered or selected. Measured
    // in Chrome: an armed `Sure?` on a row the pointer had left computed
    // `opacity: 0` and stayed armed for four seconds — a question asked of
    // somebody who could no longer see it, with the next click in that spot
    // deleting the object. The rule that reveals it needs the row to say so.
    mount();
    await drain();
    const row = [...host.querySelectorAll('.te-layer')].find((r) => /Two/.test(r.textContent));
    expect(row.classList.contains('armed')).toBe(false);
    row.querySelector('.te-lmini.danger').dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await drain();
    expect(row.classList.contains('armed')).toBe(true);
    expect(SRC).toMatch(/\.te-layer\.armed\s+\.te-lbtns\s*\{[^}]*opacity\s*:\s*1/);
  });

  it('has a rule of its own for the armed state, not one scoped to another row', async () => {
    // The only `.armed` rule in this file was `.te-objacts .armed`, scoped to the
    // inspector's action row, so the layer row's button carried the class and got
    // nothing from it: measured 20px wide against a word that wanted 30, in the
    // ordinary grey, over a transparent ground.
    expect(SRC).toMatch(/\.te-lmini\.armed\s*\{/);
    const rule = SRC.match(/\.te-lmini\.armed\s*\{([^}]*)\}/)[1];
    expect(rule, 'the box is sized by the word, not the other way round').toMatch(/width\s*:\s*auto/);
    expect(rule, 'and the confirm is coloured as one').toMatch(/background\s*:\s*var\(--v-red\)/);
    // Rule 18: amber means ON AIR and nothing else.
    expect(rule).not.toMatch(/amber/);
  });
});

// ── F2 · the object tab strip ──────────────────────────────────────────────
describe('the object tab strip cannot eat the properties body', () => {
  it('is bounded and scrolls itself rather than growing without limit', () => {
    // Unbounded inside `.te-pane{overflow:hidden}`, with only `.te-designbody`
    // carrying `min-height:0`, the strip took whatever it wanted. Measured in
    // Chrome at 1280×640 on a twenty-four-object template: the strip was 292px
    // of a 574px pane and the properties body was left 174px to hold 1379px.
    // With the ceiling: 88px and 378px, same page, same measurement.
    const rule = SRC.match(/\.te-objtabs\s*\{([^}]*)\}/)[1];
    expect(rule).toMatch(/max-height\s*:/);
    expect(rule).toMatch(/overflow-y\s*:\s*auto/);
    expect(rule, 'it must be allowed to shrink inside the column').toMatch(/min-height\s*:\s*0/);
    // The reason it WRAPS is untouched: a tab hidden behind a horizontal
    // scrollbar is a tab nobody knows is there.
    expect(rule).toMatch(/flex-wrap\s*:\s*wrap/);
  });
});

// ── F3 · the add-layer menu ────────────────────────────────────────────────
describe('the add-layer menu is not clipped by the pane it opens in', () => {
  it('is positioned fixed and anchored to the button, the way the gallery already does it', async () => {
    // Measured in Chrome at 1280×640: 591px of menu (four layer types, thirteen
    // bindings) inside a pane ending at y=626, 57px cut off, and
    // `elementFromPoint` over the centre of the last item returned null — the
    // item was not merely hidden, nothing could click it.
    mount();
    await drain();
    expect(host.querySelector('.te-addmenu')).toBeFalsy();
    host.querySelector('.te-addbtn').dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await drain();
    const menu = host.querySelector('.te-addmenu');
    expect(menu).toBeTruthy();
    // An anchored menu carries its screen position inline. A menu nested in its
    // own relatively-positioned wrapper carries none, which is what it had.
    expect(menu.getAttribute('style')).toMatch(/left:\s*-?\d/);
    expect(menu.getAttribute('style')).toMatch(/top:\s*-?\d/);
    const rule = SRC.match(/\.te-addmenu\s*\{([^}]*)\}/)[1];
    expect(rule).toMatch(/position\s*:\s*fixed/);
    // Taller than some windows are, so it clamps and keeps its own scroll.
    expect(rule).toMatch(/max-height\s*:/);
    expect(rule).toMatch(/overflow-y\s*:\s*auto/);
  });

  it('closes on a resize and on a scroll, because a fixed menu detaches from both', () => {
    expect(SRC).toMatch(/addEventListener\('resize',\s*closeAdd\)/);
    expect(SRC).toMatch(/addEventListener\('scroll',\s*closeAdd,\s*true\)/);
  });

  it('still gives Escape back on the second press (rule 44)', async () => {
    // A mounted [role="menu"] makes `shortcuts.js` stand down, so while this is
    // open the panic key belongs to it. One press closes the menu and nothing
    // else; by the second the shell has the key again.
    mount();
    await drain();
    host.querySelector('.te-addbtn').dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await drain();
    expect(host.querySelector('[role="menu"]')).toBeTruthy();
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    await drain();
    expect(host.querySelector('[role="menu"]')).toBeFalsy();
  });
});
