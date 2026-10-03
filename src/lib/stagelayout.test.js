// THE OPERATOR DECIDES WHAT THE PREACHER'S SCREEN SHOWS (plan S8).
//
// ── The defect ──────────────────────────────────────────────────────────────
//
// `Stage.svelte` has had seven zone switches since wave 4 and they lived in
// `localStorage` on the device, under `relay.stage.zones`. That is right for a
// preference and wrong for a decision: the operator could not set them, could
// not see them, and could not tell whether what they had just sent was being
// rendered — `Live.svelte` says so out loud, that whether the preacher's
// programme zone is on "is not a fact available on this side of the room". A
// tablet reset, or a second device, lost the arrangement silently.
//
// ── The model ───────────────────────────────────────────────────────────────
//
// A layout is GLOBAL and its assignment is PER SCREEN, which is ProPresenter's
// own shape. It is deliberately not a template: a stage layout has no regions,
// no style and no `TemplateRender` output, because this page draws its own
// zones.
//
// ── The absence that matters ────────────────────────────────────────────────
//
// No layout is a REAL ANSWER and the default. A church already running a tablet
// with zones set by hand keeps exactly that until somebody deliberately assigns
// one — there is no silent migration and no default layout handed out on first
// sight. That is what these tests are mostly about, because it is the half that
// could quietly destroy an arrangement a church depends on.
//
//   npx vitest run src/lib/stagelayout.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const Stage = (await import('../Stage.svelte')).default;

let socket;
class FakeSocket {
  constructor() { socket = this; this.sent = []; this.readyState = 1; }
  send(m) { this.sent.push(m); }
  close() { this.readyState = 3; this.onclose?.(); }
}

const ALL_ON = {
  reading: true, next: true, note: true,
  countdown: true, clock: true, elapsed: true, programme: true,
};
/** "Timer focus": the clocks and nothing to read. */
const TIMER_FOCUS = {
  reading: false, next: false, note: false,
  countdown: true, clock: true, elapsed: true, programme: true,
};

let host;
let app;
let zonesReply;

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  zonesReply = {};
  globalThis.fetch = vi.fn(async (url) => {
    if (String(url).includes('/api/stage_zones')) {
      return { ok: true, json: async () => ({ ok: true, zones: zonesReply }) };
    }
    return { ok: true, json: async () => ({ ok: true }) };
  });
  try { localStorage.removeItem('relay.stage.zones'); } catch { /* shimmed */ }
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null; host = null;
  try { localStorage.removeItem('relay.stage.zones'); } catch { /* shimmed */ }
  window.history.replaceState({}, '', '/');
});

async function open(channel = 2) {
  window.history.replaceState({}, '', `/stage.html?channel=${channel}`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  // The HTTP read happens on connect; let it land.
  await new Promise((r) => setTimeout(r, 0));
  await tick();
  await tick();
  return host;
}

const send = (frame) => socket.onmessage({ data: JSON.stringify(frame) });
/**
 * WHAT THE SCREEN IS SHOWING, read off the screen (RG-241).
 *
 * These cases used to read the zone state off the Zones panel's own buttons.
 * The panel left the phone — it was one tap from the screen a preacher reads
 * mid-sermon — so the state is read where it now shows: in what the page paints.
 * That is the better instrument in any case. A pressed toggle was a claim about
 * the layout; a rendered region is the layout.
 */
const showing = (sel) => !!host.querySelector(sel);
/**
 * A verse and a clock, so every zone this file asks about has something to draw.
 *
 * A zone with nothing in it renders nothing whether it is switched on or off, so
 * without this the assertions below would pass for the wrong reason — which is
 * the one risk of reading state off the screen instead of off a toggle.
 */
const CONTENT_FRAME = {
  kind: 'content',
  content_kind: 'scripture',
  reference: 'Romans 8:28',
  text: 'And we know that all things work together for good.',
};
const TIMER_FRAME = {
  kind: 'timer',
  warn_default_ms: 60_000,
  timers: [{ id: 1, label: 'Sermon', countdown_to: Date.now() + 240_000, countdown_from: Date.now() }],
};
const ZONE_MARK = {
  Reading: '.reading',
  'Stage Timer': '.progrow',
  'Stage Note': '.noterow',
};

describe('a screen the operator has not given a layout', () => {
  it('uses the zones the device itself has, and says the device decides', async () => {
    // The arrangement a church may already be running. Nothing may erase it.
    localStorage.setItem('relay.stage.zones', JSON.stringify({ ...ALL_ON, programme: false }));
    await open();
    send(CONTENT_FRAME);
    send(TIMER_FRAME);
    await tick();
    expect(showing(ZONE_MARK['Stage Timer']), 'a stored arrangement was erased').toBe(false);
  });

  it('and the device can no longer change them — that is the point (RG-241)', async () => {
    // The assertion turned over. The picker was one tap from the screen a
    // preacher is reading, and on an unassigned screen it wrote `localStorage`:
    // the person the screen exists for could switch off the clock they were
    // relying on and nobody at the desk would know.
    await open();
    expect(
      [...host.querySelectorAll('button')].map((b) => b.textContent.trim()),
      'the zone picker is still on the phone',
    ).not.toContain('Zones');
  });
});

describe('a screen the operator HAS given a layout', () => {
  it('wears the layout rather than the device preference', async () => {
    // The device says show everything; the desk says Timer focus. The desk wins.
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    zonesReply = { 2: TIMER_FOCUS };
    await open();
    send(CONTENT_FRAME);
    send(TIMER_FRAME);
    await tick();
    expect(showing('.reading'), 'the device preference beat the desk').toBe(false);
    expect(showing('.progrow'), 'the desk asked for the rail and did not get it').toBe(true);
  });

  it('and there is nothing on the phone left to take away', async () => {
    // This used to assert that an assigned layout DISABLED the device's toggles
    // and said why. There are no toggles now, on any screen, so the guarantee is
    // kept by absence — which is stronger than a disabled control, and is why
    // the sentence explaining the disabling went with them.
    zonesReply = { 2: TIMER_FOCUS };
    await open();
    expect(host.querySelector('.zonepanel')).toBeNull();
    expect(host.textContent).not.toMatch(/desk has given this screen a layout/i);
  });

  it('is only about ITS OWN screen', async () => {
    // The whole map is published; a page applies the entry for its own channel
    // and nothing else. A screen wearing another screen's layout is the same
    // class of accident as a Stage Message on a lobby TV.
    zonesReply = { 9: TIMER_FOCUS };
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    await open(2);
    send(CONTENT_FRAME);
    send(TIMER_FRAME);
    await tick();
    expect(showing('.reading'), 'this screen wore another screen’s layout').toBe(true);
  });

  it('follows a change made while the screen is already open', async () => {
    await open();
    send(CONTENT_FRAME);
    send(TIMER_FRAME);
    send({ kind: 'stage_zones', zones: { 2: TIMER_FOCUS } });
    await tick();
    expect(showing('.reading')).toBe(false);
  });

  it('hands the screen back to the device when the assignment is cleared', async () => {
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    zonesReply = { 2: TIMER_FOCUS };
    await open();
    send(CONTENT_FRAME);
    send(TIMER_FRAME);
    send({ kind: 'stage_zones', zones: {} });
    await tick();
    expect(showing('.reading'), 'the screen was left wearing a layout nobody assigned').toBe(true);
  });
});

describe('what a broken read must not do', () => {
  it('falls back to the device rather than blanking the screen', async () => {
    // The cost of reading this over HTTP instead of replaying it on hello. The
    // safe direction is a working screen, and this pins which direction that is.
    globalThis.fetch = vi.fn(async () => { throw new TypeError('Failed to fetch'); });
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    await open();
    send(CONTENT_FRAME);
    send(TIMER_FRAME);
    await tick();
    expect(showing('.reading')).toBe(true);
  });

  it('treats an empty layout as no layout, not as show-nothing', async () => {
    // `{}` for a channel would otherwise turn every zone off by way of the
    // key-by-key merge. Absent and empty are a working screen and a blank one.
    zonesReply = { 2: {} };
    localStorage.setItem('relay.stage.zones', JSON.stringify({ ...ALL_ON, reading: false }));
    await open();
    send(CONTENT_FRAME);
    send(TIMER_FRAME);
    await tick();
    // The device said reading OFF. If `{}` were treated as a layout, the
    // key-by-key merge would hand back the DEFAULT (on) — which on a default
    // install looks exactly like the fallback it replaced. So this asserts the
    // device's own value survived, not merely that something is on.
    expect(showing('.reading'), 'an empty layout was treated as a layout').toBe(false);
    // There are no toggles to take away since RG-241; what an empty entry must
    // not do is start LOOKING like a layout, and the line above is the whole of
    // that claim now.
    expect(showing('.progrow'), 'an empty entry blanked the screen').toBe(true);
    expect(host.textContent).not.toMatch(/desk has given this screen a layout/i);
  });
});
