// ── "FOLLOW THE CONTENT LOOK" HAS TO REACH A RENDERER ────────────────────────
//
// A content look is stored as `app_settings['tpl_<kind>']` and reaches the output
// as an ID and NOTHING ELSE. That is deliberate and it is a hard performance rule
// (`main::cue_or_content_tpl`): a look carrying an embedded `data:` image has been
// 13 MB, and serialising it onto every fire made verses take seconds. So the id
// rides and the JSON does not.
//
// Nothing read the id. `Output.svelte` derived its override from
// `parseTemplateOverride(content.template_json)`, which is null BY CONSTRUCTION
// for a content look, and the kiosk door did not even copy `template_id` off the
// frame. A screen set to follow the content look therefore wore the configured
// default for the life of the product — five settings, a matrix to edit them in, a
// store, a backend command and a fire-path lookup, and no screen that could read
// the answer. That is DECISIONS §70's own finding, one layer further down: §70
// gave a screen the ability to have no look of its own, and the look it was then
// supposed to follow never arrived.
//
// The claim is made on BOTH doors, because there are two kinds of screen and only
// ever one rule: a native output window has the Tauri bridge and no socket, a
// kiosk/OBS browser source has the socket and no bridge, and a guarantee kept on
// one of the two is this repository's most-repeated bug. A projector on HDMI and
// an OBS source in the same room wearing different templates is the whole failure.
//
// The assertion surface is the RENDERED DOM, not the wire. The test that existed
// (`e2e::r4_a_following_screen_wears_a_different_look_for_each_kind`) asserted that
// the id rides and the JSON does not — both true, both still true, and neither of
// them a claim about what a congregation sees. A test's assertion surface is part
// of its claim.
//
//   npx vitest run src/lib/contentlook.test.js
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

const LOOK = withFill(9, 'Scripture look', '#ab00cd');
const CONFIGURED_DEFAULT = withFill(4, 'House default', '#112233');
const OWN = withFill(7, 'This screen', '#00ff99');

const VERSE = {
  reference: 'Romans 8:28',
  text: 'And we know that all things work together for good',
  translation: 'KJV',
};

async function settle() {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
}

let host;
let app;
/** Every background fill currently painted, in paint order. */
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
describe('a native output window following the content look', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockClear();
    handlers.clear();
    // NO `template_id` — this screen has no look of its own and follows the
    // content look (DECISIONS §70). It is the only configuration in which a
    // content look can apply to anything at all.
    window.history.replaceState({}, '', '/output.html?channel=1');
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_setting' && args?.key === 'default_template_id') return Promise.resolve('4');
      if (cmd === 'get_content_templates')
        return Promise.resolve({ scripture: 9, song: null, media: null, announce: null, countdown: null });
      if (cmd === 'get_template' && args?.id === 4) return Promise.resolve(CONFIGURED_DEFAULT);
      if (cmd === 'get_template' && args?.id === 9) return Promise.resolve(LOOK);
      if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(OWN);
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

  it('wears the look the fire names, not the configured default', async () => {
    await mount();
    fire('output://content', { kind: 'scripture', ...VERSE, template_id: 9, template_json: null });
    await tick();
    await settle();
    expect(
      fills(),
      'the projector painted the configured default over a content look the ' +
        'operator set — "Follow the content look" moved nothing on the wall',
    ).toEqual(['#ab00cd']);
  });
});

// ── THE KIOSK/OBS SOURCE: the socket, and no bridge ─────────────────────────
describe('a kiosk/OBS browser source following the content look', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockClear();
    handlers.clear();
    // This page reaches the kiosk path by failing the Tauri bridge, which is
    // exactly how a real browser source gets there. It has to be `listen` that
    // fails, not `invoke`: a FOLLOWER has no `template_id`, so `loadTemplate`
    // returns without ever calling a command and every read after it swallows its
    // own failure — so a page that only refused `invoke` would finish the desktop
    // mount and never open a socket at all. The one uncaught await on that path is
    // the event import, which is precisely what a browser has none of.
    invoke.mockRejectedValue(new Error('no tauri here'));
    listen.mockImplementation(() => Promise.reject(new Error('no tauri here')));
    window.history.replaceState({}, '', '/output.html?channel=1');
  });

  /** Mount on the kiosk path and return the hub's side of the socket. */
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

  it('wears the look the fire names, not the configured default', async () => {
    const { deliver } = await mountKiosk();
    // The hub's hello reply, in its real order: configuration first, then what is
    // on the screens. The look has to be IN HAND before the frame it dresses.
    deliver({ kind: 'template', id: 9, template: LOOK });
    deliver({ kind: 'default_template', template: CONFIGURED_DEFAULT });
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE, template_id: 9, template_json: null });
    await tick();
    await settle();
    expect(
      fills(),
      'the browser source painted the configured default over a content look — ' +
        'and the projector beside it would be doing the same',
    ).toEqual(['#ab00cd']);
  });
});

// ── THE RANKING IS UNCHANGED, AND THAT IS HALF THE CLAIM ────────────────────
//
// A look that reaches the wall is only correct if it reaches it in the RIGHT
// PLACE in the order. §29 was reversed once already, on operator direction,
// because a content look overriding every screen silently replaced the templates
// operators had deliberately assigned — "all my outputs have a template set but
// the output shows something else." Making the look resolvable again is exactly
// the change that could reintroduce that, so each rank is asserted on the page
// that paints, not on the pure resolver alone.
describe('the look takes its place in the order and does not jump it', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockClear();
    handlers.clear();
    invoke.mockRejectedValue(new Error('no tauri here'));
    listen.mockImplementation(() => Promise.reject(new Error('no tauri here')));
  });

  async function mountKiosk(search) {
    class FakeWS {
      constructor() {
        this.readyState = 1;
        FakeWS.last = this;
      }
      send() {}
      close() {}
    }
    vi.stubGlobal('WebSocket', FakeWS);
    window.history.replaceState({}, '', search);
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    return (obj) => FakeWS.last.onmessage({ data: JSON.stringify(obj) });
  }

  it("a screen's OWN template still wins over the look (DECISIONS §29)", async () => {
    const deliver = await mountKiosk('/output.html?channel=1&template_id=7');
    deliver({ kind: 'template', id: 7, template: OWN });
    deliver({ kind: 'template', id: 9, template: LOOK });
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE, template_id: 9 });
    await tick();
    await settle();
    expect(
      fills(),
      'the content look replaced a template the operator deliberately assigned to ' +
        'this screen — the §25 behaviour §29 reversed, reintroduced',
    ).toEqual(['#00ff99']);
  });

  it('a PINNED cue template still overrides the screen', async () => {
    const deliver = await mountKiosk('/output.html?channel=1&template_id=7');
    deliver({ kind: 'template', id: 7, template: OWN });
    await tick();
    deliver({
      kind: 'content',
      content_kind: 'scripture',
      ...VERSE,
      template_id: 9,
      template_json: JSON.stringify(LOOK),
      template_pinned: true,
    });
    await tick();
    await settle();
    expect(
      fills(),
      "a cue's deliberate per-item choice stopped overriding the screen",
    ).toEqual(['#ab00cd']);
  });

  it('the transparency law still wins: a keyed screen refuses an opaque look', async () => {
    // A keyed (lower-third) template has no background layer at all, so it keys
    // over the camera. An opaque look must never be allowed to cover that camera —
    // the verse flows into the band instead. The look is resolvable now, so this
    // is the rank that had to be re-checked rather than assumed.
    const KEYED = {
      id: 7,
      name: 'Lower third',
      layout: {
        layers: [
          { id: 'band', type: 'band', visible: true, x: 0, y: 72, w: 100, h: 20, fill: '#00000088' },
          { id: 'v', type: 'text', visible: true, bind: 'verse', x: 6, y: 74, w: 88, h: 16, size: 3 },
        ],
      },
      style: { background: 'transparent' },
    };
    const deliver = await mountKiosk('/output.html?channel=1&template_id=7');
    deliver({ kind: 'template', id: 7, template: KEYED });
    deliver({ kind: 'template', id: 9, template: LOOK });
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE, template_id: 9 });
    await tick();
    await settle();
    expect(
      fills(),
      'an opaque content look painted over a keyed channel — the lower third ' +
        'exists to caption the very camera it just blotted out',
    ).toEqual([]);
  });

  it('an id nothing sent falls through to the configured default, and never throws', async () => {
    // The honest answer to a cache miss. A screen that has not been told what
    // template 9 IS cannot paint it, and painting nothing is worse than painting
    // the default (DECISIONS §70).
    const deliver = await mountKiosk('/output.html?channel=1');
    deliver({ kind: 'default_template', template: CONFIGURED_DEFAULT });
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', ...VERSE, template_id: 9 });
    await tick();
    await settle();
    expect(fills()).toEqual(['#112233']);
  });
});
