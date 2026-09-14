// The Templates gallery's rail (docs/REBRAND.md §2).
//
// The rail answers the question a booth actually asks — "what does scripture
// wear?" — which the cards answer only the other way round, one template at a
// time. That makes it a status line about the look register, and rule 35's
// standard applies: it has to read differently when nothing is bound than when
// something is.
//
// R3-13 is the specific trap. An unbound kind used to be the kind of place an em
// dash appears, and a dash cannot tell "nothing is bound" from "we have not
// asked yet" — the same defect as "up to date" printed over a dead update
// channel (RG-83). So the unbound row says **Not set**, in words, and this test
// holds both halves: the bound row names its template, the unbound one does not
// borrow its neighbour's answer or print a glyph.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: TemplateGallery } = await import('./TemplateGallery.svelte');
const { templates, contentTemplates } = await import('../../stores/capture.js');

const settle = () => new Promise((r) => setTimeout(r, 0));

const TPL = [
  { id: 1, name: 'Nocturne', layout: { layers: [] }, style: {} },
  { id: 2, name: 'Daybreak', layout: { layers: [] }, style: {} },
];

let host;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new TemplateGallery({ target: host });
}

/** The look-register rows, as [kind, what it says]. */
const lookRows = () =>
  [...host.querySelectorAll('.tg-look')].map((r) => [
    r.querySelector('.rw-itemname').textContent.trim(),
    r.querySelector('.tg-lookv').textContent.trim(),
  ]);

describe('the Templates rail states the look register', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return TPL;
      if (cmd === 'list_output_channels') return [];
      return null;
    });
    templates.set(structuredClone(TPL));
    contentTemplates.set({ scripture: 1 });
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('names the template a bound kind wears, and says Not set for one that is unbound', async () => {
    mount();
    for (let i = 0; i < 4; i++) await settle();

    const rows = lookRows();
    expect(rows.length, 'one row per content kind').toBeGreaterThan(1);

    const bound = rows.filter(([, v]) => v === 'Nocturne');
    expect(bound.length, 'exactly the one kind that is bound names a template').toBe(1);

    // Every other kind says so IN WORDS. Not a dash — an em dash over an unbound
    // kind reads the same as an em dash over a read that never happened.
    const rest = rows.filter(([, v]) => v !== 'Nocturne');
    expect(rest.every(([, v]) => v === 'Not set')).toBe(true);
    expect(host.querySelector('.tg-rail').textContent).not.toMatch(/—/);
  });

  it('a bound row is a way into the template, an unbound one is not a control', async () => {
    mount();
    for (let i = 0; i < 4; i++) await settle();

    // The bound row is a button; the unbound rows are not, because there is
    // nothing for them to select. A disabled button that looks like a row is a
    // control an operator will press and learn nothing from.
    const pressable = [...host.querySelectorAll('button.tg-look')];
    expect(pressable.length).toBe(1);

    pressable[0].click();
    await settle();
    expect(host.querySelector('.tg-selname').textContent.trim()).toBe('Nocturne');
  });
});
