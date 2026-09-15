// WHICH DESK THE TEMPLATES WORKSPACE OPENS ON (DECISIONS §79).
//
// The shell mounts a workspace with NO props, so a desk held only in a local
// `let` is a desk forgotten on every reload — and the `themes → templates` tab
// redirect would have nowhere to land an operator whose saved session still
// names the old tab. Landing them on the Templates desk reads exactly like the
// surface having been deleted, which is the failure that redirect exists to
// prevent, one level deeper.
//
// So the choice lives in the session, beside `activeTab` and `liveDensity`, and
// `initialDesk` is only an override for a caller that passes one.
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// A backend that answers, emptily. `list_templates` in particular must be an
// ARRAY: `loadTemplates` sets the store to whatever it is handed, so a `null`
// there puts a null in `$templates` and the gallery's own filter throws on
// mount — a fixture artefact, not a defect in the thing under test, but it took
// a stack trace to see that.
const invoke = vi.fn(async (cmd) => (cmd === 'list_templates' || cmd === 'list_output_channels' ? [] : null));
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { default: Templates } = await import('./Templates.svelte');
const { session, setSession } = await import('../session.js');
const { get } = await import('svelte/store');

const settle = () => new Promise((r) => setTimeout(r, 0));
const drain = async (n = 6) => { for (let i = 0; i < n; i++) await settle(); };

let host;
let cmp;
function mount(props = {}) {
  host = document.createElement('div');
  document.body.appendChild(host);
  cmp = new Templates({ target: host, props });
  return cmp;
}
/** A reload, as far as this workspace is concerned: the component really goes
 *  away. Removing the host node alone leaves it mounted and still reacting to
 *  the store, so the "it survived" assertion would be reading the FIRST mount. */
function unmount() {
  cmp?.$destroy();
  cmp = null;
  host?.remove();
  host = null;
}

/** Which desk is showing, read off the strip rather than off the component's
 *  internals — an operator can only see the strip. */
const showing = () => {
  const on = [...host.querySelectorAll('.ds-strip button')].find((b) => b.classList.contains('on'));
  return on?.textContent.trim();
};
const press = (label) =>
  [...host.querySelectorAll('.ds-strip button')].find((b) => b.textContent.trim() === label).click();

describe('the Templates workspace remembers which desk you were on', () => {
  beforeEach(() => {
    invoke.mockClear();
    setSession({ templatesDesk: undefined });
  });
  afterEach(unmount);

  it('opens on Templates when the session says nothing', async () => {
    mount();
    await drain();
    expect(showing()).toBe('Templates');
  });

  it('opens on the desk the session remembers', async () => {
    // THE REDIRECT'S LANDING PLACE. `migrateSession` turns a saved
    // `activeTab: 'themes'` into `templatesDesk: 'themes'`; without this line
    // reading it, that operator lands on Templates and the Themes surface looks
    // deleted rather than moved.
    setSession({ templatesDesk: 'themes' });
    mount();
    await drain();
    expect(showing()).toBe('Themes');
  });

  it('PERSISTS a desk press, so it survives the next mount', async () => {
    // The whole point. The shell passes no props, so a desk kept in a local
    // `let` is lost on every reload.
    mount();
    await drain();
    press('Themes');
    await drain();
    expect(showing()).toBe('Themes');
    expect(get(session).templatesDesk).toBe('themes');

    // Remount, as a reload would.
    unmount();
    mount();
    await drain();
    expect(showing(), 'the desk survived the remount').toBe('Themes');
  });

  it('an explicit initialDesk overrides the session, and an unknown one does not', async () => {
    setSession({ templatesDesk: 'templates' });
    mount({ initialDesk: 'themes' });
    await drain();
    expect(showing()).toBe('Themes');

    // A value that is not a desk this workspace has must fall through rather
    // than render nothing — a saved value outlives the layout it was written
    // under, which is the same reason `resolveActiveTab` exists.
    unmount();
    setSession({ templatesDesk: 'stagedisplays' });
    mount({ initialDesk: 'nonsense' });
    await drain();
    expect(showing()).toBe('Templates');
  });

  it('switching desk always lands on that desk\'s GALLERY, never a half-finished editor', async () => {
    mount();
    await drain();
    press('Themes');
    await drain();
    // The themes gallery renders its own pane head; the editor does not.
    expect(host.textContent).toMatch(/The style layer beneath templates/i);
  });
});
