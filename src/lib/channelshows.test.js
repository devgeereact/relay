// ── WHAT A SCREEN SHOWS IS THE OPERATOR'S FACT (DECISIONS §98) ───────────────
//
// The report was "timers still show on all screens, even the live screen".
// Measured in a live install: of 44 templates, 40 list `countdown` in
// `layout.shows` and the other four declare no `shows` key at all, which
// `templateShows` reads as showing every kind. So every template in that install
// painted the congregation countdown — and `shows` is not editable from any
// surface in the app. It lives in seed data and in `TemplateRender`, and nowhere
// an operator can reach.
//
// So reach becomes a property of the SCREEN, ANDed with the template's own
// statement and never replacing it. The three rules below are the whole of it and
// each is load-bearing:
//
//   1. it may NARROW only — a screen can never force a template to paint a kind
//      it has no regions for;
//   2. NO OPINION is not an empty set — it shows everything, and the default for
//      a new channel is no opinion, because the alternative hides a scripture
//      fire by accident;
//   3. it is NEVER consulted for `clear` or `black` — a screen an operator can
//      configure out of a panic control is rule 15's exact failure.
//
//   npx vitest run src/lib/channelshows.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { channelShowsKind, templateShows } from './layers.js';

describe('the pure rule', () => {
  const shows = { 1: ['scripture', 'song'], 2: [] };

  it('narrows to the kinds the screen names', () => {
    expect(channelShowsKind(shows, 1, 'scripture')).toBe(true);
    expect(channelShowsKind(shows, 1, 'countdown')).toBe(false);
  });

  it('treats an EMPTY list as an empty list, because somebody chose it', () => {
    expect(channelShowsKind(shows, 2, 'scripture')).toBe(false);
  });

  it('treats every kind of ABSENCE as no opinion, never as an empty list', () => {
    // A congregation screen painting nothing for the rest of a service, with
    // nothing on it able to say why, is the failure this asymmetry exists to
    // prevent. It has to survive a missing screen, a missing map, a map that came
    // back as the wrong shape, and a channel of 0 (a raw preview).
    for (const bad of [null, undefined, {}, [], 'nope', 0]) {
      expect(channelShowsKind(bad, 1, 'countdown')).toBe(true);
    }
    expect(channelShowsKind(shows, 3, 'countdown')).toBe(true);
    expect(channelShowsKind(shows, null, 'countdown')).toBe(true);
    expect(channelShowsKind({ 1: 'scripture' }, 1, 'countdown')).toBe(true);
  });

  it('ANDed with the template, it can only ever narrow', () => {
    // A lower third has no regions for a countdown. A screen that names
    // `countdown` must not be able to make it paint one.
    const band = { layout: { shows: ['scripture'] } };
    const and = (tpl, chShows, id, kind) =>
      templateShows(tpl, kind) && channelShowsKind(chShows, id, kind);
    expect(and(band, { 1: ['scripture', 'countdown'] }, 1, 'countdown')).toBe(false);
    expect(and(band, { 1: ['scripture', 'countdown'] }, 1, 'scripture')).toBe(true);
    // …and the screen narrows a template that would show everything.
    expect(and({}, { 1: ['scripture'] }, 1, 'countdown')).toBe(false);
    expect(and({}, {}, 1, 'countdown')).toBe(true);
  });
});

// ── RUNG 0 STILL ASKS THE SCREEN'S OWN TEMPLATE, NEVER THE RESOLVED LOOK ─────
//
// DECISIONS §97 puts this first in the chain on purpose. `templateShows` is
// consulted on the SCREEN'S own template (`t`), and if it were consulted on the
// RESOLVED one instead, choosing a per-kind look for a kind could turn that kind
// off — a look picker that silently doubles as a visibility control, which is the
// exact confusion §97 and §98 exist to keep apart. It would also be circular: the
// look is resolved FROM the kind, so asking the resolved look whether it shows
// that kind is asking a question that has already been answered.
//
// Asserted on the source, because the two templates only differ on a page with a
// per-kind look set AND a `shows` list that disagrees with it, and building that
// in jsdom asserts the arrangement rather than the rule.
describe('rung 0 is asked of the screen, not of the look', () => {
  const page = readFileSync(path.resolve(__dirname, '..', 'Output.svelte'), 'utf8');

  it('the one helper reads `t`, the screen\u2019s own template', () => {
    expect(page).toMatch(/function paintsKind\(kind\)[\s\S]{0,200}templateShows\(t, kind\)/);
  });

  it('and nothing asks the RESOLVED template whether it shows a kind', () => {
    // `activeTemplate` and `renderedTemplate` are the resolution's OUTPUT. Either
    // of them here makes a per-kind look able to hide the kind it was chosen for.
    expect(
      page,
      'a visibility check was moved onto the resolved template, so choosing a ' +
        'look for a kind can now turn that kind off',
    ).not.toMatch(/templateShows\(\s*(activeTemplate|renderedTemplate)/);
  });

  it('both doors go through that one helper', () => {
    // The kiosk door reads `content_kind` off a differently-shaped message and
    // the Tauri door reads `kind` off the struct emit, so only the RULE is
    // shared — which is why it has to be a function and not two expressions.
    const calls = page.match(/!paintsKind\(/g) ?? [];
    expect(calls.length, 'a door stopped asking, or a third one appeared').toBe(2);
  });
});

// ── AND ON THE PAGE, ON BOTH DOORS ──────────────────────────────────────────
const invoke = vi.fn();
const handlers = new Map();
const listen = vi.fn(async (name, fn) => {
  handlers.set(name, fn);
  return () => {};
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const Output = (await import('../Output.svelte')).default;

const PLAIN = {
  id: 7,
  name: 'Wall',
  layout: {
    layers: [
      { id: 'bg', type: 'background', visible: true, x: 0, y: 0, w: 100, h: 100, fill: '#00ff99' },
      { id: 'v', type: 'text', visible: true, bind: 'verse', x: 6, y: 20, w: 88, h: 50, size: 5 },
    ],
  },
  style: {},
};

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

describe('a kiosk/OBS browser source obeys the set its own screen was given', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockClear();
    handlers.clear();
    invoke.mockRejectedValue(new Error('no tauri here'));
    listen.mockImplementation(() => Promise.reject(new Error('no tauri here')));
    window.history.replaceState({}, '', '/output.html?channel=1');
  });

  async function mountKiosk() {
    class FakeWS {
      constructor() {
        this.readyState = 1;
        FakeWS.last = this;
      }
      send() {}
      close() {}
    }
    vi.stubGlobal('WebSocket', FakeWS);
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    FakeWS.last.onopen?.();
    return (obj) => FakeWS.last.onmessage({ data: JSON.stringify(obj) });
  }

  const COUNTDOWN = {
    kind: 'content',
    content_kind: 'countdown',
    reference: 'Starting soon',
    text: '',
    countdown_to: Date.now() + 300000,
  };

  it('does not paint a kind its channel omits', async () => {
    const deliver = await mountKiosk();
    deliver({ kind: 'channel_template', channel: 1, template: PLAIN });
    deliver({ kind: 'channel_shows', shows: { 1: ['scripture', 'song'] } });
    await tick();
    deliver(COUNTDOWN);
    await tick();
    await settle();
    expect(
      fills(),
      'the countdown reached a screen the operator took it off — until this ' +
        'existed there was no surface in Relay that could take it off at all',
    ).toEqual([]);
  });

  it('still paints it when its channel says nothing', async () => {
    const deliver = await mountKiosk();
    deliver({ kind: 'channel_template', channel: 1, template: PLAIN });
    // `{}` is an ANSWER: no screen anywhere has an opinion. Every screen follows
    // its template, which is where this decision lived before the column existed.
    deliver({ kind: 'channel_shows', shows: {} });
    await tick();
    deliver(COUNTDOWN);
    await tick();
    await settle();
    expect(
      fills(),
      'a screen with no opinion stopped showing a kind — the default hid ' +
        'something nobody asked it to hide',
    ).toEqual(['#00ff99']);
  });

  it('reads only its OWN channel out of the set', async () => {
    const deliver = await mountKiosk();
    deliver({ kind: 'channel_template', channel: 1, template: PLAIN });
    deliver({ kind: 'channel_shows', shows: { 4: ['scripture'] } });
    await tick();
    deliver(COUNTDOWN);
    await tick();
    await settle();
    expect(fills()).toEqual(['#00ff99']);
  });

  it('a panic control is never narrowed by it', async () => {
    // `clear` and `black` address every screen and ask nothing about which. A
    // screen an operator can configure out of one is rule 15's exact failure, and
    // this is the precise shape it would take, so it is asserted on the screen
    // that is CONFIGURED OUT of the kind that is on it.
    const deliver = await mountKiosk();
    deliver({ kind: 'channel_template', channel: 1, template: PLAIN });
    deliver({ kind: 'channel_shows', shows: { 1: ['scripture'] } });
    await tick();
    deliver({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God' });
    await tick();
    await settle();
    expect(fills()).toEqual(['#00ff99']);
    deliver({ kind: 'clear' });
    await tick();
    await settle();
    expect(fills(), 'a clear did not reach a screen with a narrowed set').toEqual([]);
  });
});

describe('a native output window obeys the same set', () => {
  beforeEach(() => {
    invoke.mockReset();
    listen.mockClear();
    handlers.clear();
    window.history.replaceState({}, '', '/output.html?channel=1&template_id=7');
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(PLAIN);
      if (cmd === 'list_output_channels')
        return Promise.resolve([{ id: 1, name: 'Wall', shows_json: '["scripture","song"]' }]);
      if (cmd === 'get_content_templates')
        return Promise.resolve({ scripture: null, song: null, media: null, announce: null, countdown: null });
      if (cmd === 'list_channel_looks') return Promise.resolve({});
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

  it('reads its own set at launch, so a projector opened mid-service obeys it', async () => {
    // A browser source is SENT the set in the hello reply. A native window has no
    // socket and would be sent nothing until the operator happened to change it —
    // the "guarantee kept on one door" mistake, on the two screens most often in
    // the same room.
    await mount();
    fire('output://content', { kind: 'countdown', reference: 'Starting soon', countdown_to: Date.now() + 300000 });
    await tick();
    await settle();
    expect(fills()).toEqual([]);
    fire('output://content', { kind: 'scripture', reference: 'John 3:16', text: 'For God' });
    await tick();
    await settle();
    expect(fills()).toEqual(['#00ff99']);
  });

  it('learns a change without a reload', async () => {
    await mount();
    fire('output://channel_shows', { shows: { 1: ['scripture', 'song', 'countdown'] } });
    await tick();
    fire('output://content', { kind: 'countdown', reference: 'Starting soon', countdown_to: Date.now() + 300000 });
    await tick();
    await settle();
    expect(
      fills(),
      'the projector went on refusing a kind the operator has just put back',
    ).toEqual(['#00ff99']);
  });
});
