// ── THE STANDING BACKGROUND, ON THE OUTPUT PAGE ─────────────────────────────
//
// `backdrop.test.js` asks what the renderer draws. This asks the question one
// level up, where the panic guarantee actually lives: does the PAGE let go of the
// background when the operator takes the screens down?
//
// Both doors, because there are two kinds of screen and only ever one rule. A
// native output window has the Tauri bridge and no socket; a kiosk/OBS browser
// source has the socket and no bridge. Four separate defects in this repository
// are a guarantee kept on one of two doors, and `Output.svelte`'s own comments
// count them. So every claim below is made twice.
//
//   npx vitest run src/lib/backdroppage.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const invoke = vi.fn();
const handlers = new Map();
const listen = vi.fn(async (name, fn) => {
  handlers.set(name, fn);
  return () => {};
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const Output = (await import('../Output.svelte')).default;

// A template with a Backdrop layer — the opt-in. Without one the page would be
// correct to paint nothing, and this file would be asserting an absence for the
// wrong reason.
const WITH_BACKDROP = {
  id: 7,
  name: 'House',
  layout: {
    layers: [
      { id: 'bg', type: 'background', visible: true, x: 0, y: 0, w: 100, h: 100, fill: '#101010' },
      { id: 'bd', type: 'backdrop', visible: true, x: 0, y: 0, w: 100, h: 100, fit: 'cover' },
      { id: 'v', type: 'text', visible: true, bind: 'verse', x: 6, y: 20, w: 88, h: 50, size: 5 },
    ],
  },
  style: {},
};

const PIC = 'http://10.0.0.5:8032/media/3';

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
const pictures = () => [...host.querySelectorAll('.lmediafill')].map((n) => n.getAttribute('src'));

/** The native window's door. */
const fire = (name, payload) => handlers.get(name)?.({ payload });

beforeEach(() => {
  invoke.mockReset();
  listen.mockClear();
  handlers.clear();
  window.history.replaceState({}, '', '/output.html?channel=1&template_id=7');
  invoke.mockImplementation((cmd, args) => {
    if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(WITH_BACKDROP);
    if (cmd === 'live_background') return Promise.resolve(null);
    return Promise.resolve(null);
  });
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

describe('a native output window and the standing background', () => {
  it('paints a background it is sent, with nothing fired', async () => {
    await mount();
    fire('output://background', { media_url: PIC, media_kind: 'image' });
    await tick();
    expect(
      pictures(),
      'a background that arrives before the first fire must paint — a church puts ' +
        'one up minutes before anything is read',
    ).toEqual([PIC]);
  });

  it('CLEAR takes it off, and that is the invariant', async () => {
    await mount();
    fire('output://background', { media_url: PIC, media_kind: 'image' });
    fire('output://content', { kind: 'scripture', reference: 'Romans 8:28', text: 'And we know' });
    await tick();
    expect(pictures()).toEqual([PIC]);

    fire('output://clear');
    await tick();
    expect(
      pictures(),
      'Clear screens left the church picture on the wall. The operator has pressed ' +
        'the control that means EVERYTHING and something is still up there.',
    ).toEqual([]);
    expect(host.textContent.replace(/\s+/g, ' ').trim()).toBe('');
  });

  it('BLACKOUT takes it off too', async () => {
    await mount();
    fire('output://background', { media_url: PIC, media_kind: 'image' });
    await tick();
    expect(pictures()).toEqual([PIC]);

    fire('output://black');
    await tick();
    // The opaque overlay would hide it on THIS channel; on a keyed one there is
    // no overlay at all, which is why the page drops the backdrop itself rather
    // than relying on something being painted over it.
    expect(pictures()).toEqual([]);
  });

  it('a verse fired over it leaves it exactly where it is', async () => {
    await mount();
    fire('output://background', { media_url: PIC, media_kind: 'image' });
    fire('output://content', { kind: 'scripture', reference: 'Romans 8:28', text: 'And we know' });
    await tick();
    expect(host.textContent).toMatch(/And we know/);
    expect(
      pictures(),
      'the verse took the backdrop down with it, which is the defect this whole ' +
        'payload exists to fix',
    ).toEqual([PIC]);
  });

  it('an explicit null is a take-down, not a message to ignore', async () => {
    await mount();
    fire('output://background', { media_url: PIC, media_kind: 'image' });
    await tick();
    fire('output://background', { media_url: null, media_kind: null });
    await tick();
    expect(pictures()).toEqual([]);
  });

  it('reads what is already up when the window opens', async () => {
    // The `loadLiveTransition` argument, on the second payload: the hub replays
    // the retained background on `hello` and a native window has no socket, so a
    // projector opened mid-service would be the one screen painting words on
    // black.
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(WITH_BACKDROP);
      if (cmd === 'live_background') return Promise.resolve([PIC, 'image']);
      return Promise.resolve(null);
    });
    await mount();
    expect(
      invoke.mock.calls.some(([c]) => c === 'live_background'),
      'the desktop path never asked what was already behind everything',
    ).toBe(true);
    expect(pictures()).toEqual([PIC]);
  });

  it('survives a backend that refuses that read', async () => {
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(WITH_BACKDROP);
      if (cmd === 'live_background') return Promise.reject(new Error('no such command'));
      return Promise.resolve(null);
    });
    await expect(mount()).resolves.toBeTruthy();
    // The failure did not abort the desktop mount into the kiosk fallback.
    expect(listen.mock.calls.some(([name]) => name === 'output://content')).toBe(true);
    expect(pictures()).toEqual([]);
  });
});

describe('a kiosk/OBS browser source and the standing background', () => {
  // THE SECOND DOOR. This page reaches it by failing the Tauri import, which is
  // exactly how a real browser source gets there.
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockRejectedValue(new Error('no tauri here'));
  });

  /**
   * Mount on the kiosk path and return a function that feeds it hub frames.
   *
   * The socket is stubbed rather than opened: the claim is about what the page
   * DOES with a frame, and a real WebSocket in jsdom would be a second thing that
   * can fail for reasons that are not this test's.
   */
  async function mountKiosk() {
    const sent = [];
    class FakeWS {
      constructor() {
        this.readyState = 1;
        FakeWS.last = this;
      }
      send(m) {
        sent.push(m);
      }
      close() {}
    }
    vi.stubGlobal('WebSocket', FakeWS);
    // A built-in id, so the kiosk path resolves a real template. `template_id=1`
    // is the bundled Classic Serif; the backdrop layer has to be injected after
    // mount, which `$set` cannot reach — so this half asserts on the page's own
    // state through the ONE observable it shares with the desktop half: what it
    // hands the renderer.
    window.history.replaceState({}, '', '/output.html?channel=1&template_id=7');
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    const deliver = (obj) => FakeWS.last.onmessage({ data: JSON.stringify(obj) });
    // The hub's `hello` reply sends the real saved template first, exactly as
    // `run_kiosk_server` does.
    deliver({ kind: 'template', id: 7, template: WITH_BACKDROP });
    await tick();
    return deliver;
  }

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('paints a background frame, and a CLEAR frame takes it off', async () => {
    const deliver = await mountKiosk();
    deliver({ kind: 'background', media_url: PIC, media_kind: 'image' });
    await tick();
    expect(pictures(), 'the kiosk door never painted the background at all').toEqual([PIC]);

    deliver({ kind: 'clear' });
    await tick();
    expect(
      pictures(),
      'a clear left the church picture on an OBS source — the guarantee kept on ' +
        'one of two doors, on the two screens most often in the same room',
    ).toEqual([]);
  });

  it('a BLACK frame takes it off too', async () => {
    const deliver = await mountKiosk();
    deliver({ kind: 'background', media_url: PIC, media_kind: 'image' });
    await tick();
    deliver({ kind: 'black' });
    await tick();
    expect(pictures()).toEqual([]);
  });

  it('a verse frame leaves it alone', async () => {
    const deliver = await mountKiosk();
    deliver({ kind: 'background', media_url: PIC, media_kind: 'image' });
    deliver({
      kind: 'content',
      content_kind: 'scripture',
      reference: 'Romans 8:28',
      text: 'And we know',
    });
    await tick();
    expect(host.textContent).toMatch(/And we know/);
    expect(pictures()).toEqual([PIC]);
  });
});
