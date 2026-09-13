// The Themes workspace, in the control-room grammar (docs/REBRAND.md §2, §3.2).
//
// The editor's left rail claims something specific: THESE are the keys this
// theme pins, and everything else falls through to the renderer's own default.
// That is REBRAND §3.1's rule made visible — a property with two homes is a
// property you can edit in one place while it is written in another — so the
// rail is only worth having if it cannot lie. Two ways it could:
//
//   · list a key the theme does not set (it would read as a claim about the
//     wall that is false), or
//   · keep listing a key after it has been cleared, which is the same lie one
//     click later and the one a stale `$:` would produce.
//
// Both are asserted here against the real component and the real store.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: ThemeEditor } = await import('./ThemeEditor.svelte');

const settle = () => new Promise((r) => setTimeout(r, 0));

/** A custom theme that pins exactly two keys. */
const THEME = { id: 7, name: 'Mine', style: { accent: '#ff0000', verseSize: '7.2' } };

let host;
function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  return new ThemeEditor({ target: host, props: { themeId: 7 } });
}

const railRows = () =>
  [...host.querySelectorAll('.te-pjump')].map((b) => b.querySelector('.te-pn').textContent.trim());

describe('the theme editor rail says what this theme has actually set', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'get_setting') return JSON.stringify([THEME]);
      return null;
    });
  });

  afterEach(() => {
    host?.remove();
    host = null;
  });

  it('lists the keys the theme pins, and not the ones it leaves alone', async () => {
    mount();
    for (let i = 0; i < 4; i++) await settle();

    expect(railRows()).toEqual(['Verse size', 'Accent']);
    // The twelve it does NOT set are absent FROM THE RAIL — a rail that listed
    // every key it COULD set would be a table of contents, not a statement about
    // this theme. `Line height` is deliberately the probe: it HAS a control in
    // the inspector two columns over, so scoping the assertion to the rail is
    // the whole point rather than an accident of wording.
    expect(railRows()).not.toContain('Line height');
    expect(host.querySelector('.te-designbody').textContent).toMatch(/Line height/);
  });

  it('clearing a key takes its row away with it', async () => {
    mount();
    for (let i = 0; i < 4; i++) await settle();

    const clear = [...host.querySelectorAll('.te-pclear')].find(
      (b) => b.getAttribute('aria-label') === 'Clear Accent',
    );
    expect(clear, 'each pinned row offers a named clear').toBeTruthy();
    clear.click();
    for (let i = 0; i < 2; i++) await settle();

    expect(railRows()).toEqual(['Verse size']);
    // …and it is an unsaved edit, so Save has to be reachable to commit it.
    // A clear that silently left the draft clean would drop the change on exit.
    const save = [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Save');
    expect(save).toBeTruthy();
    expect(save.disabled).toBe(false);
  });

  it('a theme that pins nothing says so instead of showing an empty rail', async () => {
    invoke.mockImplementation(async (cmd) =>
      cmd === 'get_setting' ? JSON.stringify([{ id: 7, name: 'Bare', style: {} }]) : null,
    );
    mount();
    for (let i = 0; i < 4; i++) await settle();

    expect(railRows()).toEqual([]);
    expect(host.querySelector('.te-railempty').textContent).toMatch(/Nothing set yet/);
  });
});
