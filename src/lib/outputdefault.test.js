// THE CONFIGURED DEFAULT, ON A NATIVE OUTPUT WINDOW.
//
// `set_default_template`'s own doc comment says what this is for: a screen
// already following the content look kept the old look until it was reopened,
// "which is why the default 'did not activate on all screens'". The wave fixed
// the CHANGE — the hub pushes `default_template` and native windows get
// `output://default_template` — and left the OPENING alone on one of the two
// doors. `defaultTpl` was seeded by the kiosk hub's `hello` reply and, on the
// desktop path, by nothing at all: a projector opened through
// `open_channel_output` on a channel with no template of its own resolved to the
// bundled `DEFAULT_TEMPLATE` until the operator happened to change the default
// and the event fired. Hidden precisely because the kiosk half worked.
//
// So this test MOUNTS the desktop path — no `output://default_template` is ever
// dispatched — and asks whether the initial read happened. It is the shape
// `r2livepath.test.js` uses for the same class of defect (mount the surface,
// assert the backend was actually reached), and it is the assertion that
// `loadLiveTransition`, which makes the identical argument for the identical
// asymmetry, never had written down either.
//
//   npx vitest run src/lib/outputdefault.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
const listen = vi.fn(async () => () => {});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const Output = (await import('../Output.svelte')).default;

const HOUSE = {
  id: 7,
  name: 'House look',
  layout: { regions: ['verse_text', 'reference'], align: 'center' },
  style: { background: '#101010', verseColor: '#ffffff', verseSize: '5' },
};

// The component's mount is a chain of awaited dynamic imports; jsdom's
// microtask queue needs draining before the reads have all landed.
async function settle() {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
}

let host;
let app;
async function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  return host;
}

beforeEach(() => {
  invoke.mockReset();
  listen.mockClear();
  // No `template_id` in the URL — this screen FOLLOWS the content look, which is
  // the only configuration in which the configured default is consulted at all.
  window.history.replaceState({}, '', '/output.html');
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

describe('a native output window reads the configured default when it opens', () => {
  it('asks for default_template_id and then for that template', async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_setting' && args?.key === 'default_template_id') return Promise.resolve('7');
      if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(HOUSE);
      return Promise.resolve(null);
    });

    await mount();

    const calls = invoke.mock.calls;
    expect(
      calls.some(([c, a]) => c === 'get_setting' && a?.key === 'default_template_id'),
      'the desktop path never read default_template_id — the projector is on the bundled look',
    ).toBe(true);
    expect(
      calls.some(([c, a]) => c === 'get_template' && a?.id === 7),
      'the id was read but the template behind it never fetched',
    ).toBe(true);
    // And it happened WITHOUT the event: no `output://default_template` was
    // dispatched here, which is the whole point — the operator has not changed
    // anything, they have opened a window.
    const handlers = listen.mock.calls.filter(([name]) => name === 'output://default_template');
    expect(handlers.length, 'the event listener must still be registered').toBe(1);
  });

  it('is guarded — an unset default leaves the screen on its own template', async () => {
    // `get_setting` returns an empty string when the operator has cleared the
    // default (`set_default_template` writes ''), and a live output page may
    // never throw. Nothing is fetched and nothing is painted from it.
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_setting') return Promise.resolve('');
      return Promise.resolve(null);
    });

    await mount();

    expect(invoke.mock.calls.some(([c]) => c === 'get_template')).toBe(false);
  });

  it('survives a backend that refuses the read', async () => {
    // The same guard `loadLiveTransition` carries: a missing command or a failed
    // read leaves this screen following its template rather than taking a live
    // output page down.
    invoke.mockImplementation((cmd) => {
      if (cmd === 'get_setting') return Promise.reject(new Error('no such command'));
      return Promise.resolve(null);
    });

    await expect(mount()).resolves.toBeTruthy();
    // It still got as far as registering the live listeners, i.e. the failure did
    // not abort the desktop mount into the kiosk fallback.
    expect(listen.mock.calls.some(([name]) => name === 'output://content')).toBe(true);
  });
});
