// RG-167 — THE STAGE TIMER BAND SAID THE SAME THING IN FOUR DIFFERENT SITUATIONS.
//
// `Start timer` succeeds when nothing holds the `stage` role, and it is right to:
// a timer is a registry fact and a screen may be opened a minute later
// (DECISIONS §91). What was wrong is that the band then read IDENTICALLY to a
// band with a preacher's tablet painting three feet away. A church that had never
// opened `Outputs → Screens → Role` started a Stage Timer every Sunday, watched
// the figure count down on the console, and sent it to nobody.
//
// That is rule 35 exactly — *a status badge that cannot detect its own failure is
// not a status badge* — and the first and last fixtures below are the proof: with
// the line removed they produce the same band, character for character.
//
// ── WHAT THIS FILE IS CAREFUL ABOUT ──────────────────────────────────────────
//
// `programmetimer.test.js` forbids the words *on stage*, *on air*, *on the
// screens* and *live* anywhere in this band, and it is KEPT rather than replaced.
// It is right: `attached` is a claim about a socket, not about a rail. Whether
// the preacher's `programme` zone is switched on is `localStorage` on his own
// device and is not a fact available on this side of the room. Every sentence
// here is checked against that list a second time, from this side, so a future
// wording cannot satisfy one file and break the other.
//
//   npx vitest run src/lib/stagetimerreach.test.js

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { describeStageReach } from './channelroles.js';

// ── THE PURE RULE ────────────────────────────────────────────────────────────

/** A channel row as `list_output_channels` returns one. */
const chan = (over = {}) => ({ id: 1, name: 'Stage tablet', role: 'stage', render_target: 'network_client', template_id: null, ...over });
/** A health row as `channel_status` returns one. */
const health = (over = {}) => ({ id: 1, name: 'Stage tablet', online: true, clients: 1, detail: '', supported: true, painting: true, last_beat_ms: 400, paint_state: 'content', down: null, ...over });
/** One `outs` entry as `Live.svelte` builds it. */
const row = (c, s, st) => ({ c, s, st });

describe('the four situations, and four different sentences', () => {
  const sentences = {};

  it('nothing holds the role — and the way to fix it is in the line', () => {
    const r = describeStageReach([row(chan({ role: 'main', name: 'Main screen' }), { kind: 'onair' }, health())], {
      read: true,
    });
    expect(r.text).toBe('No screen is set as a Stage display — Outputs → Screens → Role');
    sentences.norole = r.text;
  });

  it('a stage screen with no window open', () => {
    const r = describeStageReach([row(chan(), { kind: 'idle' }, health({ online: false, painting: false }))], {
      read: true,
    });
    expect(r.text).toBe('Stage tablet: no window');
    sentences.idle = r.text;
  });

  it('a stage screen that has stopped answering', () => {
    const r = describeStageReach(
      [row(chan(), { kind: 'down' }, health({ painting: false, last_beat_ms: 9000 }))],
      { read: true },
    );
    expect(r.text).toBe('Stage tablet: not responding');
    sentences.down = r.text;
  });

  it('a stage screen that is attached — and it claims nothing more than that', () => {
    const r = describeStageReach([row(chan(), { kind: 'onair' }, health())], { read: true });
    expect(r.text).toBe('Stage tablet attached');
    sentences.ok = r.text;
  });

  it('the list has not been read, or the read failed', () => {
    const unread = describeStageReach([], { read: false });
    expect(unread.text).toBe('Cannot tell whether a Stage display is open — reading the screens');
    const failed = describeStageReach([], { read: true, error: 'the engine is not answering' });
    expect(failed.text).toBe(
      'Cannot tell whether a Stage display is open — the engine is not answering',
    );
    sentences.unread = unread.text;
    sentences.failed = failed.text;
  });

  it('and no two of them are the same sentence', () => {
    // THE WHOLE POINT. The first and the last producing one sentence is the
    // behaviour this file was written against, and it is what rule 35 calls not a
    // status line at all.
    const all = Object.values(sentences);
    expect(all.length, 'a case above did not run — vitest ordering').toBe(6);
    expect(new Set(all).size, `two situations share a sentence:\n  ${all.join('\n  ')}`).toBe(all.length);
  });
});

describe('the Copy URL trap, which is the one an operator cannot guess', () => {
  it('names the right address when a stage screen has never once reported painting', () => {
    // `Outputs → Screens`'s `Copy URL` hands out `output.html`, and that page has
    // no `timer` branch at all — only `stage.html` renders the rail, and only
    // `channelroles.js::stageRemoteUrl` produces that address. A screen wired from
    // the wrong copied link is online, is attached, and will never show a Stage
    // Timer. "Not responding" would send the operator to look at the wifi.
    const r = describeStageReach(
      [row(chan(), { kind: 'down' }, health({ painting: false, last_beat_ms: null, paint_state: null }))],
      { read: true },
    );
    expect(r.text).toBe(
      'Stage tablet has never reported painting — a Stage Timer needs the stage address (Outputs → Sharing)',
    );
  });

  it('but a screen that has answered before and gone quiet is not sent there', () => {
    const r = describeStageReach(
      [row(chan(), { kind: 'down' }, health({ painting: false, last_beat_ms: 12_000 }))],
      { read: true },
    );
    expect(r.text).not.toMatch(/Sharing/);
  });
});

describe('several screens may hold the role (DECISIONS §89)', () => {
  it('reports the best of them, and names which one it means', () => {
    const rows = [
      row(chan({ id: 1, name: 'Confidence monitor' }), { kind: 'down' }, health({ id: 1, last_beat_ms: 9000 })),
      row(chan({ id: 2, name: 'Preacher tablet' }), { kind: 'onair' }, health({ id: 2 })),
    ];
    expect(describeStageReach(rows, { read: true }).text).toBe('Preacher tablet attached');
  });

  it('a screen with no name still gets words rather than an id', () => {
    const r = describeStageReach([row(chan({ name: '  ' }), { kind: 'idle' }, health({ online: false }))], {
      read: true,
    });
    expect(r.text).toBe('Stage display: no window');
  });
});

describe('the words this band may never use', () => {
  it('no sentence claims a screen is showing anything', () => {
    // The same four `programmetimer.test.js` forbids, asserted from this side so
    // a new wording cannot satisfy one file and break the other. `attached` says
    // a socket is open; the `programme` zone is `localStorage` on the preacher's
    // own device and nothing here can see it.
    const every = [
      describeStageReach([], { read: false }),
      describeStageReach([], { read: true, error: 'x' }),
      describeStageReach([row(chan({ role: 'main' }), { kind: 'onair' }, health())], { read: true }),
      describeStageReach([row(chan(), { kind: 'idle' }, health({ online: false }))], { read: true }),
      describeStageReach([row(chan(), { kind: 'down' }, health({ last_beat_ms: 9000 }))], { read: true }),
      describeStageReach([row(chan(), { kind: 'down' }, health({ last_beat_ms: null }))], { read: true }),
      describeStageReach([row(chan(), { kind: 'onair' }, health())], { read: true }),
    ];
    for (const r of every) {
      const t = r.text.toLowerCase();
      for (const claim of ['on stage', 'on air', 'on the screens', 'live']) {
        expect(t, `"${r.text}" claims "${claim}"`).not.toContain(claim);
      }
    }
  });
});

// ── AND THE SAME THING ON THE RENDERED BAND ─────────────────────────────────
//
// The rule above is pure so that Live and anything added later cannot disagree
// about the same screens. That is worth nothing if the band renders none of it,
// which is the shape `qa-inventory` exists to police and the shape a suite of
// fourteen tests against an unrendered component once had.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const cap = await import('./stores/capture.js');
const { setSession } = await import('./session.js');
const Live = (await import('./views/Live.svelte')).default;

let host;
let app;

function bridge({ channels = [], fail = false } = {}) {
  invoke.mockImplementation(async (cmd) => {
    if (cmd === 'list_output_channels') {
      if (fail) throw { kind: 'internal', message: 'the engine is not answering' };
      return channels;
    }
    if (cmd === 'list_timers') return [];
    if (cmd === 'list_templates') return [];
    if (cmd === 'list_plans') return [];
    if (cmd === 'list_books') return [{ book: 'Psalms', chapters: 150 }];
    if (cmd === 'rehearsal') return false;
    if (cmd === 'get_sensitivity') return 50;
    return null;
  });
}

function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Live({ target: host, props: {} });
  return host;
}
async function settle(ms = 60) {
  await new Promise((r) => setTimeout(r, ms));
  await tick();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}
const band = () => host.querySelector('.pt-band');

beforeEach(() => {
  invoke.mockReset();
  bridge();
  cap.live.set(null);
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.liveCue.set({ cueId: null, slide: 0, onAir: false });
  cap.channelHealth.set({});
  cap.capture.update((s) => ({ ...s, available: true, stt: { ...s.stt, loaded: true } }));
  cap.templates.set([]);
  cap.readErrors.set({});
  setSession({ planId: null });
  // Warm the bridge before mounting, for the reason `programmetimer.test.js`
  // records at the same point: the very first mocked import resolves undefined.
  return cap.loadTemplates();
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = host = null;
  cap.detections.set([]);
  cap.resolvedDetections.set([]);
  cap.readErrors.set({});
});

describe('the band renders the answer', () => {
  it('says so when no screen holds the role', async () => {
    bridge({ channels: [{ id: 1, name: 'Main screen', role: 'main', render_target: 'native_window' }] });
    mount();
    await settle();
    expect(band().textContent).toContain('No screen is set as a Stage display');
  });

  it('and says something different when one does and is attached', async () => {
    bridge({
      channels: [{ id: 2, name: 'Preacher tablet', role: 'stage', render_target: 'network_client' }],
    });
    cap.channelHealth.set({
      2: health({ id: 2, name: 'Preacher tablet', paint_state: 'clear', painting: true }),
    });
    mount();
    await settle();
    expect(band().textContent).toContain('Preacher tablet attached');
    expect(band().textContent).not.toContain('No screen is set');
  });

  it('wears no law colour, whatever it says', async () => {
    // Amber is ON AIR, cyan is a guess, amethyst is rehearsal (rule 18). The
    // facts this line carries could honestly wear none of them.
    bridge({ channels: [] });
    mount();
    await settle();
    for (const cls of ['onair', 'inreh', 'guess', 'amber']) {
      expect(band().querySelector(`.${cls}`), `the band wears .${cls}`).toBeNull();
    }
  });
});
