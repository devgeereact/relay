// TWO FACTS, TWO SHAPES.
//
// `Used for` is a GLOBAL binding written by `setContentTemplate` (DECISIONS
// §70): tick Scripture and every screen set to *Follow the content look* wears
// this template when scripture fires. `Content this template renders` is a
// PER-TEMPLATE filter on `layout.shows`, read at runtime by `Output.svelte` and
// `layers.js::templateShows`: untick Media and a stage monitor wearing this
// template ignores a picture and holds the passage.
//
// They were rendered as two identical `.te-showgrid` chip rows over the same
// five labels, one above the other, and the second starts all-ticked because an
// absent `layout.shows` means "shows everything". So the first click on the
// second row materialised the list as all-minus-one — four chips visibly going
// dark at once — which is exactly what an operator reported as "clicking a
// content look activates all". Nothing was wrong with the handler. The render
// was telling them something false about what they had just done.
//
// Asserted on the RENDERED DOM rather than the source text. The brief for this
// task gave a source-text version (`src.match(/class="te-showgrid"/g)`,
// expecting exactly one match in the whole file) — but `TemplateEditor.svelte`
// already has a THIRD, unrelated `.te-showgrid` at the "Words in this band"
// list (a band's member picker, nothing to do with `CONTENT_KINDS`), which
// reuses the class purely for its flex-wrap chip layout. A whole-file count
// would never reach exactly 1 and would fail for a reason that has nothing to
// do with this defect. Mounting the real component and reading the Template
// section sidesteps that collision for free: the band picker only renders when
// a band object is selected, which this test never does, so it never appears in
// the DOM here — and the assertion is on the actual rendered structure rather
// than a source-text proxy for it, so it cannot pass while a reformatted-but-
// still-two-facts-conflated markup would fail it.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: TemplateEditor } = await import('./views/templates/TemplateEditor.svelte');
const { templates } = await import('./stores/capture.js');

// A minimal layered template — no band, so "Words in this band" (the other
// `.te-showgrid` user) never mounts and can never be confused with either
// content-kind register.
const TEMPLATE = {
  id: 21,
  name: 'Fullscreen',
  layout: { layers: [{ id: 't1', type: 'text', name: 'Verse', bind: 'verse', x: 5, y: 5, w: 90, h: 20, size: 3 }] },
  style: {},
};

const settle = () => new Promise((r) => setTimeout(r, 0));

let host;
// Same reason `layerlist.test.js` needs this: `host.remove()` alone detaches
// the DOM node without running the component's `onDestroy`, so the 400ms
// `liveTimer` `scheduleLive()` arms on every `edit` update (mount, and every
// `toggleShows` click below) is never cleared and can fire `applyLive()` into
// a torn-down jsdom later in the same worker. Track the instance and destroy
// it for real.
let cmp;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new TemplateEditor({ target: host, props: { templateId: 21 } });
  return cmp;
}

describe('the two content-kind registers are told apart', () => {
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
    cmp?.$destroy();
    cmp = null;
    host?.remove();
    host = null;
  });

  it('keeps `Used for` as a chip grid, and does not render the filter the same way', async () => {
    mount();
    await settle();
    // `Used for` still wears the chip shape.
    expect(host.querySelector('.te-showlbl').textContent.trim()).toBe('Used for');
    expect(host.querySelectorAll('.te-showgrid').length, 'the Used-for chip grid').toBe(1);
    // The filter is not a second copy of it: no `.te-showgrid`/`.te-showchip`
    // rendered for the per-template register anywhere in the mounted tree.
    expect(host.querySelectorAll('.te-showchip[role="switch"]').length).toBe(0);
  });

  it('renders the per-template filter as switches, one per content kind', async () => {
    mount();
    await settle();
    const switches = [...host.querySelectorAll('.te-showlist [role="switch"]')];
    expect(switches.length, 'one switch per CONTENT_KINDS entry').toBe(5);
    for (const sw of switches) {
      expect(sw.hasAttribute('aria-checked'), `${sw.textContent} has no aria-checked`).toBe(true);
      expect(['true', 'false']).toContain(sw.getAttribute('aria-checked'));
    }
  });

  it('gives the filter its own section heading, distinct from the Used-for label', async () => {
    mount();
    await settle();
    const heading = host.querySelector('.te-showsec');
    expect(heading, 'no heading element for the per-template filter').toBeTruthy();
    expect(heading.tagName).toBe('H3');
    expect(heading.classList.contains('te-sec')).toBe(true);
    expect(heading.textContent.trim()).toBe('Content this template renders');
  });

  it('a click flips exactly the kind pressed, never the whole row at once', async () => {
    // This is the actual bug: the first click on the old second chip grid
    // materialised `layout.shows` as all-minus-one, so four other chips went
    // dark in the same click. A switch must go from unset (Shows, because an
    // absent list means everything) to Ignores on ITS OWN row only.
    mount();
    await settle();
    const rowFor = (label) =>
      [...host.querySelectorAll('.te-showlist [role="switch"]')].find((b) => b.textContent.includes(label));
    const before = [...host.querySelectorAll('.te-showlist [role="switch"]')].map((b) => b.getAttribute('aria-checked'));
    expect(before.every((v) => v === 'true'), 'nothing ticked yet reads as showing everything').toBe(true);

    rowFor('Media').click();
    await settle();

    const after = [...host.querySelectorAll('.te-showlist [role="switch"]')];
    expect(rowFor('Media').getAttribute('aria-checked')).toBe('false');
    const others = after.filter((b) => !b.textContent.includes('Media'));
    expect(others.every((b) => b.getAttribute('aria-checked') === 'true'), 'only Media should have moved').toBe(true);
  });
});
