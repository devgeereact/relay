// ── THE PER-KIND LOOK HAS TO REACH A RENDERER, ON BOTH DOORS ─────────────────
//
// DECISIONS §97. The pure resolver is asserted in `channellooks.test.js`; this is
// the claim that actually matters to a congregation — that the template a screen
// was configured to wear for THIS kind is the one that paints.
//
// BOTH DOORS, because there are two kinds of screen and only ever one rule: a
// native output window has the Tauri bridge and no socket, a kiosk/OBS browser
// source has the socket and no bridge, and a guarantee kept on one of the two is
// this repository's most-repeated bug. A projector on HDMI and an OBS source in
// the same room wearing different templates for the same verse is the whole
// failure this feature exists to make impossible.
//
// The assertion surface is the RENDERED DOM, not the wire. A test that asserted
// the id rides would be true of the product before this existed.
//
//   npx vitest run src/lib/channellookspage.test.js
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

/** A background layer paints `background:<fill>` into the DOM, so which template
 *  actually reached the renderer is readable rather than inferable. */
const withFill = (id, name, fill) => ({
  id,
  name,
  layout: {
    layers: [
      { id: 'bg', type: 'background', visible: true, x: 0, y: 0, w: 100, h: 100, fill },
      { id: 'v', type: 'text', visible: true, bind: 'verse', x: 6, y: 20, w: 88, h: 50, size: 5 },
    ],
  },
  style: {},
});

const BLANKET = withFill(7, 'This screen', '#00ff99');
const SCRIPTURE_LOOK = withFill(9, 'Scripture on this screen', '#ab00cd');
const SONG_LOOK = withFill(12, 'Songs on this screen', '#ffaa00');
const CONFIGURED_DEFAULT = withFill(4, 'House default', '#112233');

const VERSE = {
  reference: 'Romans 8:28',
  text: 'And we know that all things work together for good',
  translation: 'KJV',
};
const LYRIC = { reference: 'Amazing Grace', text: 'How sweet the sound', translation: '' };

/**
 * DRAIN THE MOUNT, AND DRAIN IT FURTHER THAN THE OLDER FILES DO.
 *
 * `Output.svelte`'s desktop path performs its reads in SERIES and registers its
 * `listen` handlers after the last of them, so the number of microtask turns a
 * mount needs grows with every read added to it — and `loadChannelLooks` adds
 * three. At the 40+40 this repository's other output tests use, the last two
 * listeners (`channel_roles` and `channel_looks`) were not yet registered when
 * the test fired at them, and the symptom is a per-kind look that "does not
 * update live": a product failure's exact appearance, produced entirely by the
 * harness.
 *
 * That is a fact about this test, not about the page — a real bridge resolves
 * over IPC and no event can arrive in the same microtask turn as the mount — and
 * it is written here rather than fixed by reordering the mount, because
 * registering listeners before the initial reads would let a `channel_looks`
 * event be overwritten by the read that follows it.
 */
async function settle() {
  for (let round = 0; round < 3; round += 1) {
    for (let i = 0; i < 60; i += 1) await Promise.resolve();
    await tick();
  }
}

let host;
let app;
const fills = () =>
  [...host.querySelectorAll('.lbg')]
    .map((n) => (n.getAttribute('style') || '').match(/background:\s*([^;]+)/)?.[1]?.trim())
    .filter(Boolean);

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  vi.unstubAllGlobals();
});

// ── THE NATIVE WINDOW: the bridge, and no socket ────────────────────────────
describe('a native output window wearing a different look for each kind', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockClear();
    handlers.clear();
    window.history.replaceState({}, '', '/output.html?channel=1&template_id=7');
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_setting' && args?.key === 'default_template_id') return Promise.resolve('4');
      if (cmd === 'get_content_templates')
        return Promise.resolve({ scripture: null, song: null, media: null, announce: null, countdown: null });
      if (cmd === 'list_channel_looks')
        return Promise.resolve({ 1: { scripture: 9, song: 12 } });
      if (cmd === 'get_template' && args?.id === 4) return Promise.resolve(CONFIGURED_DEFAULT);
      if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(BLANKET);
      if (cmd === 'get_template' && args?.id === 9) return Promise.resolve(SCRIPTURE_LOOK);
      if (cmd === 'get_template' && args?.id === 12) return Promise.resolve(SONG_LOOK);
      return Promise.resolve(null);
    });
  });

  async function mount() {
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
  }
  const fire = (name, payload) => handlers.get(name)?.({ payload });

  it('wears the scripture look for a verse and the song look for a lyric', async () => {
    await mount();
    fire('output://content', { kind: 'scripture', ...VERSE });
    await tick();
    await settle();
    expect(
      fills(),
      'the projector painted its blanket template over a per-kind look the ' +
        'operator set for this screen',
    ).toEqual(['#ab00cd']);

    fire('output://content', { kind: 'song', ...LYRIC });
    await tick();
    await settle();
    expect(
      fills(),
      'both kinds painted the same template — a per-kind look that only ever ' +
        'answers for one kind is a screen with one look, which is where it started',
    ).toEqual(['#ffaa00']);
  });

  it('falls back to its own template for a kind it has no look for', async () => {
    await mount();
    fire('output://content', { kind: 'announce', reference: 'Notice', text: 'Tea after the service' });
    await tick();
    await settle();
    expect(fills()).toEqual(['#00ff99']);
  });

  it('learns a look changed mid-service without a reload', async () => {
    await mount();
    fire('output://channel_looks', { looks: { 1: { scripture: 12 } } });
    await tick();
    fire('output://content', { kind: 'scripture', ...VERSE });
    await tick();
    await settle();
    expect(
      fills(),
      'the screen went on wearing the look it was given at launch — an operator ' +
        'changing a look mid-service would see nothing move until something reloaded',
    ).toEqual(['#ffaa00']);
  });
});

// ── THE KIOSK/OBS SOURCE: the socket, and no bridge ─────────────────────────
describe('a kiosk/OBS browser source wearing a different look for each kind', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockClear();
    handlers.clear();
    invoke.mockRejectedValue(new Error('no tauri here'));
    listen.mockImplementation(() => Promise.reject(new Error('no tauri here')));
    window.history.replaceState({}, '', '/output.html?channel=1');
  });

  /** Mount on the kiosk path and hand back the hub's side of the socket. */
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
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    FakeWS.last.onopen?.();
    const deliver = (obj) => FakeWS.last.onmessage({ data: JSON.stringify(obj) });
    return { deliver, sent };
  }

  /** The hub's hello reply, in its real order: the bytes, then the screen's own
   *  template, then the configuration. The look has to be in hand before the
   *  frame it dresses (rule 43). */
  const hello = (deliver) => {
    deliver({ kind: 'template', id: 9, template: SCRIPTURE_LOOK });
    deliver({ kind: 'template', id: 12, template: SONG_LOOK });
    deliver({ kind: 'channel_template', channel: 1, template: BLANKET });
    deliver({ kind: 'default_template', template: CONFIGURED_DEFAULT });
    deliver({ kind: 'channel_looks', looks: { 1: { scripture: 9, song: 12 } } });
  };

  it('wears the scripture look for a verse and the song look for a lyric', async () => {
    const { deliver } = await mountKiosk();
    hello(deliver);
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE });
    await tick();
    await settle();
    expect(
      fills(),
      'the browser source painted its blanket template over a per-kind look — ' +
        'and the projector beside it would be painting the right one',
    ).toEqual(['#ab00cd']);

    deliver({ kind: 'content', content_kind: 'song', ...LYRIC });
    await tick();
    await settle();
    expect(fills()).toEqual(['#ffaa00']);
  });

  it('is told `{}` and treats it as an answer rather than as silence', async () => {
    const { deliver } = await mountKiosk();
    deliver({ kind: 'template', id: 9, template: SCRIPTURE_LOOK });
    deliver({ kind: 'channel_template', channel: 1, template: BLANKET });
    deliver({ kind: 'channel_looks', looks: {} });
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE });
    await tick();
    await settle();
    expect(fills()).toEqual(['#00ff99']);
  });

  it('only ever reads its OWN channel out of the map', async () => {
    // The hub broadcasts to everybody and records nothing about who connected
    // (DECISIONS §35), so the filter lives at the receiver — and a receiver that
    // read the first entry it found would wear the lobby TV's look on the wall.
    const { deliver } = await mountKiosk();
    deliver({ kind: 'template', id: 12, template: SONG_LOOK });
    deliver({ kind: 'channel_template', channel: 1, template: BLANKET });
    deliver({ kind: 'channel_looks', looks: { 4: { scripture: 12 } } });
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE });
    await tick();
    await settle();
    expect(fills()).toEqual(['#00ff99']);
  });

  it('a panic control takes this screen whatever look it was wearing', async () => {
    // `clear` and `black` address every screen and ask nothing about which — that
    // is what makes them panic controls (rule 15, DECISIONS §20). There is no
    // channel and no kind anywhere on that path, and there must never be.
    const { deliver } = await mountKiosk();
    hello(deliver);
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE });
    await tick();
    await settle();
    expect(fills()).toEqual(['#ab00cd']);
    deliver({ kind: 'clear' });
    await tick();
    await settle();
    expect(fills(), 'a per-kind look survived a clear').toEqual([]);
  });
});
