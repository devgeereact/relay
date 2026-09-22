// THE CANVAS SAYS WHERE YOU ARE AND HOW CLOSE (RG-228).
//
// Stage two of the approved redesign. Two absences, both of them ordinary in
// every editor of this kind and neither present here:
//
//   · A ZOOM THAT COULD NOT GET CLOSER. There IS a zoom, and reading it first
//     is what stopped this becoming a second one: `zoom` is the board's width as
//     a share of the pane and its scale ended at 100 — fit. So it could only
//     ever make the slide smaller, and exact work on a small layer meant
//     squinting at a 30px box. Worse, `max-width:100%` on the wrapper would have
//     held the board AT fit however far a wider scale was pushed, so the new
//     steps would have been a dead control — RG-223 in another place.
//   · A READOUT THAT SAYS FOUR NUMBERS AND NOT WHICH OBJECT. `6,20 · 88×50`
//     with nothing naming the layer, and nothing saying the figure size, which
//     is the number the operator was arguing with in RG-223.
//
// The zoom is a VIEW state and nothing else: it never touches the template.
// That is worth a test of its own, because a zoom that wrote into the model
// would resize a layer on a congregation screen for the rest of the service.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const TEMPLATE = {
  id: 7,
  name: 'Probe',
  layout: {
    layers: [
      { id: 'a', type: 'text', name: 'Verse', bind: 'verse', x: 6, y: 20, w: 88, h: 50, size: 5.2 },
      { id: 'b', type: 'shape', name: 'Plate', x: 0, y: 0, w: 100, h: 8 },
    ],
  },
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
  cmp = new TemplateEditor({ target: host, props: { templateId: 7, layerId: 'a' } });
  await drain();
  return host;
}
const foot = () => host.querySelector('.te-botbar').textContent.replace(/\s+/g, ' ');
// The zoom lives in the top toolbar, where it already was. It is NOT moved to
// the foot: two controls over one piece of view state is the twin door this
// repository keeps deleting, and the foot's job is to say what is selected.
const zoomBtn = (label) =>
  [...host.querySelectorAll('button')].find((b) => (b.getAttribute('aria-label') || '') === label);
const pct = () => host.querySelector('.te-pct').textContent.trim();

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async () => null);
  templates.set([structuredClone(TEMPLATE)]);
  capture.update((c) => ({ ...c, available: true }));
});
afterEach(() => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; });

describe('the readout names the object, not just its numbers', () => {
  it('says which layer, where it is and how big its type is', async () => {
    await open();
    expect(foot()).toContain('Verse');
    expect(foot(), 'the box is gone from the readout').toMatch(/6.*20.*88.*50/);
    // The figure size — the number RG-223 was about. An operator dragging Size
    // should not have to open a group to see what they are moving.
    expect(foot(), 'the size is not on the canvas foot').toMatch(/5\.2\s*cqw/);
  });

  it('and says so of the object actually selected', async () => {
    await open();
    [...host.querySelectorAll('.te-layer')].find((r) => r.textContent.includes('Plate')).click();
    await drain();
    expect(foot()).toContain('Plate');
    expect(foot()).not.toContain('cqw');
  });
});

describe('zoom is a view, never the template', () => {
  it('opens at fit, and can now go past it', async () => {
    await open();
    expect(pct()).toBe('100%');
    zoomBtn('Zoom in').click();
    await drain();
    expect(pct(), 'the zoom still stops at fit').not.toBe('100%');
    expect(Number(pct().replace('%', ''))).toBeGreaterThan(100);
  });

  it('and the wrapper stops capping the board, or the new steps do nothing', async () => {
    // The half that would have made this a dead control: `max-width:100%` held
    // the board at fit whatever the width said.
    await open();
    expect(host.querySelector('.te-board-wrap').classList.contains('over')).toBe(false);
    zoomBtn('Zoom in').click();
    await drain();
    const wrap = host.querySelector('.te-board-wrap');
    expect(wrap.classList.contains('over'), 'the board is still capped at fit').toBe(true);
    expect(wrap.getAttribute('style')).toMatch(/width:\s*150%/);
  });

  it('a zoom moves NO layer — it is a view and the template is untouched', async () => {
    await open();
    // The SELECTION's own numbers, not the whole foot: the zoom control lives on
    // the foot now (RG-231), so its own readout changes there by design.
    const shown = () => [...host.querySelectorAll('.te-botchip')].map((c) => c.textContent.trim());
    const before = shown();
    zoomBtn('Zoom in').click();
    zoomBtn('Zoom in').click();
    await drain();
    expect(shown(), 'the zoom resized a layer').toEqual(before);
  });

  it('and it stops at both ends rather than running away', async () => {
    await open();
    for (let i = 0; i < 12; i += 1) {
      zoomBtn('Zoom in').click();
      await settle();
    }
    await drain();
    expect(pct()).toBe('400%');
    expect(zoomBtn('Zoom in').disabled, 'the control does not say it has stopped').toBe(true);

    for (let i = 0; i < 20; i += 1) {
      zoomBtn('Zoom out').click();
      await settle();
    }
    await drain();
    expect(pct()).toBe('40%');
    expect(zoomBtn('Zoom out').disabled).toBe(true);
  });
});
