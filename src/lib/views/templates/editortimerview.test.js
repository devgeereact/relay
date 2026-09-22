// A CLOCK LAYER IS INVISIBLE IN THE EDITOR, SO PLACING ONE IS GUESSWORK (RG-232).
//
// The operator, mid-build: *"i also want to see how the timer looks when
// editing... and the alignments of the timer not working"*.
//
// **Both sentences are one defect.** The editor's preview passes `content` and
// nothing else, and a programme rail renders only `{#if progRows.length}` —
// which is empty here, because `programme` is a prop the editor never sends. So
// the layer draws NOTHING. The alignment buttons were writing x and y correctly
// the whole time; there was simply nothing on the canvas to see move, which is
// indistinguishable from a control that does not work.
//
// Same family as RG-222, where the console's own pane was missing the same prop.
// Three surfaces render through `TemplateRender` and two of them were not being
// told about the clocks.
//
// The sample is a SAMPLE and says so: it is not the church's programme, and no
// timer here is running anywhere.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const withLayers = (layers) => ({ id: 7, name: 'Probe', layout: { layers }, style: {} });
const PROGRAMME = withLayers([
  { id: 'bg', type: 'background', fill: '#101018' },
  { id: 'p', type: 'text', name: 'Programme', bind: 'programme', x: 0, y: 90, w: 100, h: 9, size: 2.2 },
]);
const CLOCK = withLayers([
  { id: 'bg', type: 'background', fill: '#101018' },
  { id: 't', type: 'timer', name: 'Countdown', bind: 'countdown', x: 20, y: 40, w: 60, h: 20, size: 8 },
]);
const PLAIN = withLayers([
  { id: 'bg', type: 'background', fill: '#101018' },
  { id: 'v', type: 'text', name: 'Verse', bind: 'verse', x: 6, y: 20, w: 88, h: 50, size: 5 },
]);

const { default: TemplateEditor } = await import('./TemplateEditor.svelte');
const { templates, capture } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => { for (let i = 0; i < n; i += 1) await settle(); };

let host, cmp;
async function open(tpl, layerId) {
  templates.set([structuredClone(tpl)]);
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateEditor({ target: host, props: { templateId: 7, layerId } });
  await drain();
  return host;
}
/** The artboard's own render, never a card thumbnail. */
const board = () => host.querySelector('.te-artboard');

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation(async () => null);
  capture.update((c) => ({ ...c, available: true }));
});
afterEach(() => { cmp?.$destroy(); cmp = null; host?.remove(); host = null; });

describe('a programme layer draws something to place', () => {
  it('paints a rail with figures in it, not an empty box', async () => {
    await open(PROGRAMME, 'p');
    const rail = board().querySelector('.lprog');
    expect(rail, 'the clocks never reach the editor, so the layer is invisible').toBeTruthy();
    expect(rail.textContent, 'the rail is there and says nothing').toMatch(/\d:\d\d/);
  });

  it('and the rail moves when the layer does — which is what "align" looked broken for', async () => {
    await open(PROGRAMME, 'p');
    const before = board().querySelector('.lprog').getAttribute('style');
    expect(before).toContain('top:90%');
    [...host.querySelectorAll('.te-abtn')].find((b) => /top/i.test(b.getAttribute('aria-label') || ''))?.click();
    await drain();
    const after = board().querySelector('.lprog').getAttribute('style');
    expect(after, 'the alignment wrote nothing the canvas shows').not.toBe(before);
    expect(after).toContain('top:0%');
  });
});

describe('a countdown layer does too', () => {
  it('paints a figure rather than an empty box', async () => {
    await open(CLOCK, 't');
    expect(board().textContent, 'a Clock layer shows nothing while it is being placed').toMatch(/\d:\d\d/);
  });
});

describe('and it is a SAMPLE, which has to stay true of the template without one', () => {
  it('a template with no clock layer gets no rail and no countdown', async () => {
    // The trap: handing every preview a countdown would trip the renderer's
    // default-countdown path, which HIDES the verse and reference on a template
    // that has no timer layer at all. The sample is given only where a layer
    // asked for it.
    await open(PLAIN, 'v');
    expect(board().querySelector('.lprog')).toBeNull();
    expect(board().querySelector('.cd-default'), 'a default countdown covered the verse').toBeNull();
    expect(board().textContent).toContain('firmament');
  });
});
