// A NEW TEMPLATE IS A DRAFT UNTIL SOMEBODY SAVES IT.
//
// `newFrom` used to INSERT the starter and then open the editor on the row it
// had just written. So the only way to look at what "Screen Countdown" is was to
// create one, and the only way to change your mind was to go back and delete it
// — if you noticed. A gallery accumulates several `Screen Countdown` rows a
// church never asked for, and because the shelf seeds by name, they sit beside
// the built-in of the same name.
//
// What replaces it: the starter is built in memory and handed to the editor as
// a draft (`id: null`). The editor renders it exactly as it renders a saved row,
// with three differences — the header says it is unsaved, Save inserts, and
// Discard writes nothing. Leaving a dirty draft arms an in-app two-step, never
// a native confirm(): Tauri's webview returns false from confirm() without
// showing anything, which is CLAUDE.md rule 41.
//
// Every test here was watched to fail with its own change reverted.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

// THE ROUTER, not the gallery alone. The draft crosses a component boundary —
// the gallery dispatches it and the editor renders it — so a test that mounted
// only one of the two would be asserting about half of the path. `Templates`
// is what the workspace strip actually renders.
const { default: Templates } = await import('../Templates.svelte');
const { templates, contentTemplates, capture } = await import('../../stores/capture.js');
const { STARTERS } = await import('../../layers.js');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => { for (let i = 0; i < n; i++) await settle(); };

/** A real layer-model template, built the way the New menu builds one — so
 *  nothing here is legacy and the gallery's one-time conversion finds no work.
 *  A region-model fixture would have the upgrade write `save_template` for
 *  reasons that have nothing to do with a draft. */
const fromStarter = (key, id, name) => {
  const s = STARTERS.find((x) => x.key === key);
  return { ...s.make(), id, name, active: false };
};

const TPL = [fromStarter('fullscreen', 1, 'Nocturne')];

let host;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new Templates({ target: host });
}

// SNAPSHOT THE PAYLOAD AT THE MOMENT IT IS SENT. `invoke.mock.calls` holds the
// live object, and the editor stamps the returned id onto that same object a
// tick later — so reading `calls[0][1].template.id` reports what the template
// became, not what was sent, and an insert is indistinguishable from an update.
let sent = [];
function baseMocks(list = TPL) {
  invoke.mockReset();
  sent = [];
  invoke.mockImplementation(async (cmd, args) => {
    if (cmd === 'save_template') sent.push(structuredClone(args.template));
    if (cmd === 'list_templates') return structuredClone(list);
    if (cmd === 'list_output_channels') return [];
    if (cmd === 'save_template') return 99;
    if (cmd === 'get_content_templates') return {};
    return null;
  });
  templates.set(structuredClone(list));
  contentTemplates.set({});
  capture.update((c) => ({ ...c, available: true }));
}

const saves = () => sent;
const byText = (sel, re) => [...host.querySelectorAll(sel)].find((b) => re.test(b.textContent));

/** Open the New menu and choose the named starter. */
async function chooseStarter(label = 'Full-Screen Scripture') {
  byText('.r-btn.primary.sm', /New template/).click();
  await drain();
  const item = [...host.querySelectorAll('.tg-newmi')].find((b) => new RegExp(label, 'i').test(b.textContent));
  expect(item, `starter "${label}" is on the New menu`).toBeTruthy();
  item.click();
  await drain();
}

beforeEach(() => baseMocks());
afterEach(() => { host?.remove(); host = null; });

describe('choosing a starter creates nothing', () => {
  it('opens the editor on a draft and writes no row', async () => {
    mount();
    await drain();
    await chooseStarter();
    // The editor is up …
    expect(host.querySelector('.te-shell'), 'the editor is open').toBeTruthy();
    // … and the database has not been touched.
    expect(saves()).toHaveLength(0);
  });

  it('says out loud that the template is not saved yet', async () => {
    mount();
    await drain();
    await chooseStarter();
    expect(host.querySelector('.te-unsaved')).toBeTruthy();
    expect(host.querySelector('.te-unsaved').textContent).toMatch(/not saved|unsaved/i);
  });

  it('closes without saving when the operator backs out of a draft nobody changed', async () => {
    mount();
    await drain();
    await chooseStarter();
    byText('.te-top .r-btn', /Back to Templates/).click();
    await drain();
    expect(host.querySelector('.te-shell'), 'back at the gallery').toBeFalsy();
    expect(saves()).toHaveLength(0);
  });

  it('Discard writes nothing and returns to the gallery', async () => {
    mount();
    await drain();
    await chooseStarter();
    const discard = byText('.te-top .r-btn', /^\s*Discard\s*$/);
    expect(discard, 'a draft offers an explicit Discard').toBeTruthy();
    discard.click();
    await drain();
    expect(host.querySelector('.te-shell')).toBeFalsy();
    expect(saves()).toHaveLength(0);
  });
});

describe('the draft becomes a row only when somebody says so', () => {
  it('Save inserts exactly once, with no id, and the header stops saying unsaved', async () => {
    mount();
    await drain();
    await chooseStarter();
    byText('.te-top .r-btn.primary', /Save/).click();
    await drain(12);
    const written = saves();
    expect(written).toHaveLength(1);
    // An INSERT: upsert_template takes the absence of an id as "new row".
    expect(written[0].id ?? null).toBe(null);
    expect(host.querySelector('.te-unsaved')).toBeFalsy();
  });

  it('a change to a draft does not autosave it into existence', async () => {
    mount();
    await drain();
    await chooseStarter();
    const name = host.querySelector('#te-name');
    expect(name, 'the draft renders the template section like any saved row').toBeTruthy();
    name.value = 'Sunday evening';
    name.dispatchEvent(new Event('input'));
    // Long past the 400ms live-apply debounce that saves a real template.
    await new Promise((r) => setTimeout(r, 700));
    await drain();
    expect(saves()).toHaveLength(0);
  });
});

describe('leaving a dirty draft asks, in the app', () => {
  it('arms a two-step on Back rather than leaving, and never calls confirm()', async () => {
    const confirmSpy = vi.fn(() => true);
    const had = Object.prototype.hasOwnProperty.call(window, 'confirm');
    const prev = window.confirm;
    window.confirm = confirmSpy;
    try {
      mount();
      await drain();
      await chooseStarter();
      const name = host.querySelector('#te-name');
      name.value = 'Sunday evening';
      name.dispatchEvent(new Event('input'));
      await drain();

      const back = byText('.te-top .r-btn', /Back to Templates/);
      back.click();
      await drain();
      // Still here, and now the button says what a second press will cost.
      expect(host.querySelector('.te-shell'), 'the first press does not leave').toBeTruthy();
      expect(byText('.te-top .r-btn', /without saving/i)).toBeTruthy();
      expect(confirmSpy).not.toHaveBeenCalled();

      byText('.te-top .r-btn', /without saving/i).click();
      await drain();
      expect(host.querySelector('.te-shell')).toBeFalsy();
      expect(saves()).toHaveLength(0);
    } finally {
      if (had) window.confirm = prev; else delete window.confirm;
    }
  });

  it('does not arm the two-step on a draft nobody has touched', async () => {
    mount();
    await drain();
    await chooseStarter();
    byText('.te-top .r-btn', /Back to Templates/).click();
    await drain();
    expect(host.querySelector('.te-shell')).toBeFalsy();
  });
});

describe('duplicate and import are unchanged', () => {
  it('duplicate still writes a row immediately — it is an act on content that exists', async () => {
    mount();
    await drain();
    const kebab = host.querySelector('.tg-card .tg-more');
    expect(kebab, 'a card carries its row menu').toBeTruthy();
    kebab.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await drain();
    const dup = byText('.tg-mi', /Duplicate/);
    expect(dup).toBeTruthy();
    dup.click();
    await drain(10);
    expect(saves()).toHaveLength(1);
    expect(saves()[0].name).toMatch(/copy/);
  });
});

// 2026-09-21 · T-1 (RG-200). The autosave's catch rendered `'Live update failed: ' + e`
// and the bridge sends `{kind, message}`, so the workspace's most frequent write
// failed as "[object Object]". Every other catch in the file humanises.
describe('the autosave failure is humanised', () => {
  it('goes through errors.js like every other catch here', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const src = readFileSync(resolve(process.cwd(), 'src/lib/views/templates/TemplateEditor.svelte'), 'utf8');
    expect(src).not.toMatch(/'Live update failed: ' \+ e\b/);
    expect(src).toMatch(/Live update failed: ' \+ humanError\(e\)/);
  });
});
