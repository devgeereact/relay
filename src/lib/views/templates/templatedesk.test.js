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
// channel (RG-83).
//
// An unbound kind used to say **Not set**, which was true until the Rust half
// of Task 4 (`cue_or_content_tpl`) made a fire for that kind fall back to the
// CONFIGURED DEFAULT rather than answering `None` — so a screen following that
// content look does not go blank, it wears the default. "Not set" then read
// exactly like the R3-13 trap it was written to avoid: a fixed sentence over
// two different situations (the wall goes blank vs. the wall wears something).
// The row now names what will actually paint — the default's own name, marked
// as inherited rather than bound (`Default · <name>`) — or the bundled floor
// when no default is configured either. This test holds both halves: the bound
// row names its template outright, the unbound one names what it inherits
// rather than borrowing its neighbour's answer or printing a glyph.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: TemplateGallery } = await import('./TemplateGallery.svelte');
const { templates, contentTemplates, defaultTemplateId } = await import('../../stores/capture.js');
const { DEFAULT_TEMPLATE } = await import('../../templates.js');

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
    defaultTemplateId.set(null);
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('names the template a bound kind wears, and the bundled floor for one that is unbound with no default configured', async () => {
    mount();
    for (let i = 0; i < 4; i++) await settle();

    const rows = lookRows();
    expect(rows.length, 'one row per content kind').toBeGreaterThan(1);

    const bound = rows.filter(([, v]) => v === 'Nocturne');
    expect(bound.length, 'exactly the one kind that is bound names a template').toBe(1);

    // Every other kind says so IN WORDS, and now names what it will actually
    // wear rather than claiming nothing will paint — the bundled floor, since
    // no operator default is configured in this test.
    const rest = rows.filter(([, v]) => v !== 'Nocturne');
    expect(rest.every(([, v]) => v === `Default · ${DEFAULT_TEMPLATE.name}`)).toBe(true);
    expect(host.querySelector('.tg-rail').textContent).not.toMatch(/—/);
    expect(host.querySelector('.tg-rail').textContent).not.toMatch(/Not set/);
  });

  it('names the CONFIGURED DEFAULT for an unbound kind, once one is set — not the bundled floor', async () => {
    // This is the regression Task 4's Rust half exists to close: a kind with
    // nothing bound wears `default_template_id` when a fire actually happens
    // (`cue_or_content_tpl`), and the rail must name THAT template, not the
    // bundled Classic Serif, once an operator has configured a different one.
    //
    // Answered through `get_setting`, not a direct store `.set` — `onMount`
    // calls `loadDefaultTemplate`, which reads the setting over `invoke` and
    // would otherwise overwrite a store value set ahead of mount.
    invoke.mockImplementation(async (cmd, args) => {
      if (cmd === 'list_templates') return TPL;
      if (cmd === 'list_output_channels') return [];
      if (cmd === 'get_setting' && args?.key === 'default_template_id') return '2'; // Daybreak
      return null;
    });
    mount();
    for (let i = 0; i < 4; i++) await settle();

    const rows = lookRows();
    const rest = rows.filter(([, v]) => !v.includes('Nocturne'));
    expect(rest.length, 'every other kind is unbound').toBeGreaterThan(0);
    expect(rest.every(([, v]) => v === 'Default · Daybreak')).toBe(true);
    // Watched to fail against the pre-fix behaviour: reverting the Svelte
    // change makes every one of these rows read "Not set" instead, which does
    // not mention Daybreak at all.
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
