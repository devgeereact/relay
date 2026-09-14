// The Templates INSPECTOR is objects, not only a details table
// (docs/REBRAND.md §3.2; DECISIONS §80).
//
// What the panel used to be: a preview, three buttons, a Details|Usage switch,
// five read-only rows, and — on the Usage side — a sentence telling the operator
// that content looks are set somewhere else. Two facts in that panel were things
// an operator could read and could not change from the surface they were reading
// them on, and one of them (the look binding) had a control four clicks away.
//
// What it is now: the same preview, then **Used for** as a real control writing
// through the ONE writer, and the **object strip** naming the template's real
// objects, a press away from that object's own properties.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: TemplateGallery } = await import('./TemplateGallery.svelte');
const { templates, contentTemplates, capture } = await import('../../stores/capture.js');
const { STARTERS } = await import('../../layers.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 5) => { for (let i = 0; i < n; i++) await settle(); };

/** A real layer-model template, built the way the New menu builds one. */
const fromStarter = (key, id, name) => {
  const s = STARTERS.find((x) => x.key === key);
  return { id, name, ...s.make(), active: false };
};

// One layered template and one legacy REGION preset — the two shapes a shelf
// actually holds, because the five seeded built-ins are region templates.
const LAYERED = fromStarter('fullscreen', 1, 'Nocturne');
const REGION = {
  id: 2,
  name: 'Classic Serif',
  layout: { regions: ['verse_text', 'reference'], align: 'center', lowerThird: false },
  style: { verseColor: '#f4e4c8' },
  active: false,
};
const TPL = [LAYERED, REGION];

let host;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new TemplateGallery({ target: host });
}

const objectTabs = () => [...host.querySelectorAll('.tg-objtab')].map((b) => b.textContent.trim());
const usedChips = () =>
  [...host.querySelectorAll('.tg-usedchip')].map((b) => [
    b.textContent.replace(/[✓\s]+/g, ' ').trim(),
    b.classList.contains('on'),
  ]);
const pick = (id) => {
  const card = [...host.querySelectorAll('.tg-card')].find(
    (c) => c.querySelector('.tg-name').textContent.trim() === id,
  );
  card.click();
};

describe('the Templates inspector names the objects on the slide', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return structuredClone(TPL);
      if (cmd === 'list_output_channels') return [];
      return null;
    });
    templates.set(structuredClone(TPL));
    contentTemplates.set({});
    capture.update((c) => ({ ...c, available: true }));
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('lists the template\'s real objects, by the name the editor gives them', async () => {
    mount();
    await drain();
    pick('Nocturne');
    await drain();

    // The full-screen scripture starter is a background, a verse and a
    // reference — the §3.2 set for a full-screen look, read off the template
    // rather than written out here.
    expect(objectTabs()).toEqual(['Background', 'Verse', 'Reference']);
  });

  it('a press on an object opens the editor ON that object', async () => {
    // The point of the strip. Without the object id the press is just "open the
    // editor", and the operator has to find the thing they were already
    // pointing at.
    const cmp = mount();
    const seen = [];
    cmp.$on('edit', (e) => seen.push(e.detail));
    await drain();
    pick('Nocturne');
    await drain();

    const ref = [...host.querySelectorAll('.tg-objtab')].find((b) => b.textContent.trim() === 'Reference');
    ref.click();
    await settle();

    expect(seen.length).toBe(1);
    expect(seen[0].id).toBe(1);
    expect(seen[0].layerId, 'the object the operator pressed').toBe(
      LAYERED.layout.layers.find((L) => L.name === 'Reference').id,
    );
  });

  it('a region preset has no objects and SAYS SO, rather than showing an empty strip', async () => {
    // An empty strip and a strip whose contents have not loaded look identical,
    // and only one of those is a fact about the template (rule 35).
    mount();
    await drain();
    pick('Classic Serif');
    await drain();

    expect(objectTabs()).toEqual([]);
    expect(host.querySelector('.rw-insp').textContent).toMatch(/laid out by region/i);
  });
});

describe('Used for is a control on the surface that shows it', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return structuredClone(TPL);
      if (cmd === 'list_output_channels') return [];
      return null;
    });
    templates.set(structuredClone(TPL));
    contentTemplates.set({});
    capture.update((c) => ({ ...c, available: true }));
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('offers every content kind, and ticks the one this template is bound to', async () => {
    contentTemplates.set({ scripture: 1 });
    mount();
    await drain();
    pick('Nocturne');
    await drain();

    const chips = usedChips();
    expect(chips.length, 'one chip per content kind').toBeGreaterThan(1);
    expect(chips.filter(([, on]) => on).map(([label]) => label)).toEqual(['Scripture']);
  });

  it('binds through the ONE writer, and unbinds by pressing the same chip', async () => {
    // `set_content_template` is the single writer (DECISIONS §25) — the Outputs
    // matrix and the editor's own Used for call it too. A second path to the
    // same table is how two surfaces came to disagree about what was bound.
    mount();
    await drain();
    pick('Nocturne');
    await drain();

    const scripture = [...host.querySelectorAll('.tg-usedchip')].find((b) => /Scripture/.test(b.textContent));
    scripture.click();
    await drain();

    const writes = invoke.mock.calls.filter(([cmd]) => cmd === 'set_content_template');
    expect(writes.length).toBe(1);
    expect(writes[0][1]).toMatchObject({ kind: 'scripture', templateId: 1 });

    // And it is a toggle: pressing a bound kind clears it rather than re-binding
    // it, so there is no state an operator can reach and not leave.
    contentTemplates.set({ scripture: 1 });
    await drain();
    [...host.querySelectorAll('.tg-usedchip')].find((b) => /Scripture/.test(b.textContent)).click();
    await drain();

    const all = invoke.mock.calls.filter(([cmd]) => cmd === 'set_content_template');
    expect(all[all.length - 1][1]).toMatchObject({ kind: 'scripture', templateId: null });
  });

  it('the Usage footnote no longer sends the operator somewhere else for a control that is here', async () => {
    mount();
    await drain();
    pick('Nocturne');
    await drain();
    host.querySelector('.tg-insptabs button:last-child').click();
    await drain();

    const foot = host.querySelector('.rw-foot').textContent;
    expect(foot).toMatch(/Used for/);
    expect(foot, 'it must not still claim Outputs is the ONE place').not.toMatch(/the one place/i);
  });
});
