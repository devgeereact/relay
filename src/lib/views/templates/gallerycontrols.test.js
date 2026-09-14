// THE TEMPLATES GALLERY, AS A NEW OPERATOR MEETS IT.
//
// Every control on this surface does what its label says — `qa-inventory`
// reports 0 handlerless buttons and 0 unnamed controls, and has throughout.
// What it cannot see is a control that is WIRED and still wrong: a name that
// does not identify its object, a role that replaces the one the element needs,
// a filter that outlives the row that expresses it, a sort that groups a
// register in an order the rail beside it contradicts.
//
// Each test below was watched to fail with its own change reverted.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: TemplateGallery } = await import('./TemplateGallery.svelte');
const { default: DeskStrip } = await import('./DeskStrip.svelte');
const { templates, contentTemplates, capture } = await import('../../stores/capture.js');
const { STARTERS } = await import('../../layers.js');
const { templateKind, KIND_ORDER } = await import('../../templateKind.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 5) => { for (let i = 0; i < n; i++) await settle(); };

/** A real layer-model template, built the way the New menu builds one — so the
 *  kind each one lands on is `templateKind`'s answer rather than this file's. */
const fromStarter = (key, id, name) => {
  const s = STARTERS.find((x) => x.key === key);
  return { ...s.make(), id, name, active: false };
};

const SCRIPTURE = fromStarter('fullscreen', 1, 'Nocturne');
const MEDIA = fromStarter('media', 2, 'Full Bleed');
const TPL = [SCRIPTURE, MEDIA];

let host;
function mount(Cmp = TemplateGallery, props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new Cmp({ target: host, props });
}

const cards = () => [...host.querySelectorAll('.tg-card')];
const names = () => cards().map((c) => c.querySelector('.tg-name').textContent.trim());
// THE KIND ROWS ONLY. The rail's second half is the look register (`.tg-look`),
// which answers a different question and is read by `templatedesk.test.js`.
const railRows = () =>
  [...host.querySelectorAll('.tg-rail .rw-item:not(.tg-look)')].map((b) => ({
    name: b.querySelector('.rw-itemname')?.textContent.trim(),
    n: b.querySelector('.rw-itemn')?.textContent.trim(),
    on: b.classList.contains('on'),
    el: b,
  }));
const railKind = (label) => railRows().find((r) => r.name === label);

function baseMocks(list = TPL) {
  invoke.mockReset();
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_templates') return structuredClone(list);
    if (cmd === 'list_output_channels') return [];
    return null;
  });
  templates.set(structuredClone(list));
  contentTemplates.set({});
  capture.update((c) => ({ ...c, available: true }));
}

afterEach(() => {
  host?.remove();
  host = null;
});

// ── the card's own two controls ────────────────────────────────────────────
describe('a card control names the template it acts on', () => {
  beforeEach(() => baseMocks());

  it('the star and the kebab are not called the same thing on every card', async () => {
    mount();
    await drain();

    const stars = cards().map((c) => c.querySelector('.tg-star').getAttribute('aria-label'));
    const kebabs = cards().map((c) => c.querySelector('.tg-more').getAttribute('aria-label'));

    expect(stars.length).toBe(2);
    // The defect: twelve cards, twelve buttons, one name. An operator hearing
    // "Toggle default template" cannot tell which template is about to change.
    expect(new Set(stars).size, 'every star had one name').toBe(stars.length);
    expect(new Set(kebabs).size, 'every kebab had one name').toBe(kebabs.length);
    for (const n of names()) {
      expect(stars.some((s) => s.includes(n)), `a star naming ${n}`).toBe(true);
      expect(kebabs.some((k) => k.includes(n)), `a kebab naming ${n}`).toBe(true);
    }
  });

  it('the star says whether it is currently the default, not only what pressing it does', async () => {
    mount();
    await drain();
    const star = cards()[0].querySelector('.tg-star');
    // `class:on` is a colour. `aria-pressed` is the same fact for somebody who
    // cannot see the fill.
    expect(star.getAttribute('aria-pressed')).toBe('false');
  });

  it('a menu button says it opens a menu', async () => {
    mount();
    await drain();
    const kebab = cards()[0].querySelector('.tg-more');
    expect(kebab.getAttribute('aria-haspopup')).toBe('menu');
    expect(kebab.getAttribute('aria-expanded')).toBe('false');
    kebab.click();
    await settle();
    expect(kebab.getAttribute('aria-expanded'), 'the menu is open and the button says so').toBe('true');
  });
});

// ── the object strip ───────────────────────────────────────────────────────
describe('the objects on this slide are controls, and announce as controls', () => {
  beforeEach(() => baseMocks());

  it('no object tab carries a role that replaces its button role', async () => {
    mount();
    await drain();
    cards()[0].click();
    await drain();

    const tabs = [...host.querySelectorAll('.tg-objtab')];
    expect(tabs.length).toBeGreaterThan(0);
    for (const t of tabs) {
      // `role="listitem"` on a <button> REPLACES the implicit button role: the
      // press still works and the announcement no longer says there is one.
      expect(t.getAttribute('role'), `${t.textContent.trim()} announced as a control`).toBe(null);
    }
    // …and the group that holds them is still named, so the strip is not a bare
    // run of buttons with nothing saying what they are.
    const strip = host.querySelector('.tg-objtabs');
    expect(strip.getAttribute('role')).toBe('group');
    expect(strip.getAttribute('aria-label')).toBeTruthy();
  });
});

// ── the kinds rail ─────────────────────────────────────────────────────────
describe('the rail keeps the filter an operator is looking through', () => {
  beforeEach(() => baseMocks());

  it('a kind whose last template is deleted keeps its row, at zero, still selected', async () => {
    mount();
    await drain();

    expect(railKind('Media').n).toBe('1');
    railKind('Media').el.click();
    await drain();
    expect(names()).toEqual(['Full Bleed']);

    // The only Media template goes — by the card menu, or from anywhere else.
    templates.set([structuredClone(SCRIPTURE)]);
    await drain();

    const row = railKind('Media');
    // The defect: the row vanished, nothing in the rail was lit, and the grid
    // said "No template matches this filter" about a filter with no expression
    // anywhere on the surface.
    expect(row, 'the row an operator is filtering through').toBeTruthy();
    expect(row.n).toBe('0');
    expect(row.on, 'and it is still the selected one').toBe(true);
    expect(host.querySelector('.tg-grid')).toBe(null);
    expect(host.textContent).toMatch(/No template matches this filter/i);

    // It is still a control, so it can be pressed off.
    expect(row.el.tagName).toBe('BUTTON');
  });

  it('invents no row for a kind nobody is looking at', async () => {
    mount();
    await drain();
    // Only what occurs, plus All — never a full nine-row register of kinds this
    // shelf does not hold.
    expect(railRows().map((r) => r.name)).toEqual(['All templates', 'Scripture', 'Media']);
  });

  it('the row returns to its own place in the order rather than to the end', async () => {
    // Scripture sorts before Media in KIND_ORDER, and an emptied Media row that
    // jumped to the end would read as a different row.
    mount();
    await drain();
    railKind('Media').el.click();
    await drain();
    templates.set([structuredClone(SCRIPTURE)]);
    await drain();
    expect(railRows().map((r) => r.name)).toEqual(['All templates', 'Scripture', 'Media']);
  });
});

// ── sort ───────────────────────────────────────────────────────────────────
describe('the Sort dropdown reorders the grid when it is pressed', () => {
  beforeEach(() => baseMocks());

  it('and does not wait for an unrelated keystroke to apply', async () => {
    mount();
    await drain();
    expect(names()).toEqual(['Nocturne', 'Full Bleed']);

    const sel = host.querySelector('.tg-sort select');
    sel.value = 'name';
    sel.dispatchEvent(new Event('change'));
    await drain();

    // THE DEFECT: `shown` was reactive on $templates, filter and q. `sort` was
    // read inside `sortList`'s body, where Svelte's syntactic dependency
    // collection never looks — so the grid kept its old order until something
    // else invalidated the list.
    expect(names(), 'reordered as soon as the control was used').toEqual(['Full Bleed', 'Nocturne']);
  });

  it('the star floats the new default to the top on the press, not on the next one', async () => {
    const { defaultTemplateId } = await import('../../stores/capture.js');
    defaultTemplateId.set(null);
    mount();
    await drain();
    expect(names()).toEqual(['Nocturne', 'Full Bleed']);

    // The same bug's other half: `$defaultTemplateId` was read from scope too.
    defaultTemplateId.set(2);
    await drain();
    expect(names(), 'the default template floats to the top').toEqual(['Full Bleed', 'Nocturne']);
    defaultTemplateId.set(null);
  });
});

describe('Sort by Type groups in the order the rail lists', () => {
  // Two kinds whose display order and alphabetical-by-key order DISAGREE:
  // KIND_ORDER puts scripture first, `localeCompare` on the key puts media
  // first. The grid and the rail beside it grouped the same register two ways.
  const A = fromStarter('media', 10, 'Zeta Media');
  const B = fromStarter('fullscreen', 11, 'Alpha Scripture');

  beforeEach(() => baseMocks([A, B]));

  it('orders by KIND_ORDER, not alphabetically by the internal key', async () => {
    expect(templateKind(B)).toBe('scripture');
    expect(templateKind(A)).toBe('media');
    expect(KIND_ORDER.indexOf('scripture')).toBeLessThan(KIND_ORDER.indexOf('media'));
    // …and the old comparator would have disagreed, which is what makes this a
    // test rather than a restatement.
    expect('media'.localeCompare('scripture')).toBeLessThan(0);

    mount();
    await drain();
    const sel = host.querySelector('.tg-sort select');
    sel.value = 'kind';
    sel.dispatchEvent(new Event('change'));
    await drain();

    expect(names()).toEqual(['Alpha Scripture', 'Zeta Media']);
  });
});

// ── import ─────────────────────────────────────────────────────────────────
describe('an import shows the template it just imported', () => {
  beforeEach(() => baseMocks());

  it('clears a kind filter and a search that would hide the new template', async () => {
    const IMPORTED = fromStarter('media', 99, 'From A Stick');
    let imported = false;
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'list_templates') return structuredClone(imported ? [...TPL, IMPORTED] : TPL);
      if (cmd === 'list_output_channels') return [];
      // The real path is file → parseImportedTemplate → saveTemplate → a fresh
      // id, so the command the import reaches is the ordinary upsert.
      if (cmd === 'save_template') { imported = true; return 99; }
      return null;
    });

    mount();
    await drain();

    // An operator filtered to Scripture and searching, who then imports.
    railKind('Scripture').el.click();
    const q = host.querySelector('.tg-search input');
    q.value = 'noct';
    q.dispatchEvent(new Event('input'));
    await drain();
    expect(names()).toEqual(['Nocturne']);

    // A REAL exported template file — it has to survive `parseImportedTemplate`,
    // or this test would be exercising the error path and passing for it.
    const file = new File(
      [JSON.stringify({ marker: 'relay.template/v1', name: 'From A Stick', layout: IMPORTED.layout, style: IMPORTED.style })],
      'x.json',
      { type: 'application/json' },
    );
    if (typeof file.text !== 'function') file.text = async () => JSON.stringify({ marker: 'relay.template/v1', name: 'From A Stick', layout: IMPORTED.layout, style: IMPORTED.style });
    const input = host.querySelector('input[type="file"]');
    Object.defineProperty(input, 'files', { value: [file], configurable: true });
    input.dispatchEvent(new Event('change'));
    await drain(15);

    expect(host.querySelector('.tg-err'), 'the import did not take the error path').toBe(null);

    // The defect: the import succeeded, the inspector switched to a template
    // outside the filter, and the grid looked exactly as it had. A successful
    // import and one that did nothing were the same picture.
    expect(names(), 'the imported template is on the grid').toContain('From A Stick');
    expect(q.value).toBe('');
  });
});

// ── the desk strip ─────────────────────────────────────────────────────────
describe('the desk strip promises only what it keeps', () => {
  beforeEach(() => baseMocks());

  it('is a group of buttons, like every other .r-seg, not a tablist', async () => {
    mount(DeskStrip, { desk: 'templates' });
    await settle();
    const strip = host.querySelector('.ds-strip');
    // A tablist promises Left/Right arrow navigation between the tabs and a
    // `tabpanel` behind each. This strip has neither — arrows do nothing, and a
    // desk change re-renders the workspace rather than swapping a panel.
    expect(strip.getAttribute('role')).toBe('group');
    expect(strip.getAttribute('aria-label')).toBeTruthy();
    for (const b of strip.querySelectorAll('button')) {
      expect(b.getAttribute('role')).toBe(null);
    }
  });

  it('still says which desk is showing', async () => {
    mount(DeskStrip, { desk: 'themes' });
    await settle();
    const [tpl, thm] = [...host.querySelectorAll('.ds-strip button')];
    expect(tpl.getAttribute('aria-pressed')).toBe('false');
    expect(thm.getAttribute('aria-pressed')).toBe('true');
    // The selected treatment is unchanged — `.r-seg` styles on `.on`, so the
    // role swap moves nothing an operator can see.
    expect(thm.classList.contains('on')).toBe(true);
  });
});
