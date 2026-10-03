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
const zoneButton = (label) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === label);
const openPanel = async () => {
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Zones').click();
  await tick();
};

describe('a screen the operator has not given a layout', () => {
  it('uses the zones the device itself has, and says the device decides', async () => {
    // The arrangement a church may already be running. Nothing may erase it.
    localStorage.setItem('relay.stage.zones', JSON.stringify({ ...ALL_ON, programme: false }));
    await open();
    await openPanel();
    const btn = zoneButton('Stage Timer');
    expect(btn.getAttribute('aria-pressed')).toBe('false');
    expect(btn.disabled, 'the device lost control of its own zones').toBe(false);
  });

  it('lets the device change them, as it always could', async () => {
    await open();
    await openPanel();
    const btn = zoneButton('Stage Timer');
    const before = btn.getAttribute('aria-pressed');
    btn.click();
    await tick();
    expect(zoneButton('Stage Timer').getAttribute('aria-pressed')).not.toBe(before);
  });
});

describe('a screen the operator HAS given a layout', () => {
  it('wears the layout rather than the device preference', async () => {
    // The device says show everything; the desk says Timer focus. The desk wins.
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    zonesReply = { 2: TIMER_FOCUS };
    await open();
    await openPanel();
    expect(zoneButton('Reading').getAttribute('aria-pressed')).toBe('false');
    expect(zoneButton('Stage Timer').getAttribute('aria-pressed')).toBe('true');
  });

  it('takes the toggles away from the device, and says why', async () => {
    // A disabled control that says nothing is a broken control.
    zonesReply = { 2: TIMER_FOCUS };
    await open();
    await openPanel();
    expect(zoneButton('Reading').disabled).toBe(true);
    expect(host.textContent).toMatch(/desk has given this screen a layout/i);
  });

  it('is only about ITS OWN screen', async () => {
    // The whole map is published; a page applies the entry for its own channel
    // and nothing else. A screen wearing another screen's layout is the same
    // class of accident as a Stage Message on a lobby TV.
    zonesReply = { 9: TIMER_FOCUS };
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    await open(2);
    await openPanel();
    expect(zoneButton('Reading').getAttribute('aria-pressed')).toBe('true');
    expect(zoneButton('Reading').disabled).toBe(false);
  });

  it('follows a change made while the screen is already open', async () => {
    await open();
    send({ kind: 'stage_zones', zones: { 2: TIMER_FOCUS } });
    await tick();
    await openPanel();
    expect(zoneButton('Reading').getAttribute('aria-pressed')).toBe('false');
  });

  it('hands the screen back to the device when the assignment is cleared', async () => {
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    zonesReply = { 2: TIMER_FOCUS };
    await open();
    send({ kind: 'stage_zones', zones: {} });
    await tick();
    await openPanel();
    expect(zoneButton('Reading').getAttribute('aria-pressed')).toBe('true');
    expect(zoneButton('Reading').disabled, 'the device never got its toggles back').toBe(false);
  });
});

describe('what a broken read must not do', () => {
  it('falls back to the device rather than blanking the screen', async () => {
    // The cost of reading this over HTTP instead of replaying it on hello. The
    // safe direction is a working screen, and this pins which direction that is.
    globalThis.fetch = vi.fn(async () => { throw new TypeError('Failed to fetch'); });
    localStorage.setItem('relay.stage.zones', JSON.stringify(ALL_ON));
    await open();
    await openPanel();
    expect(zoneButton('Reading').getAttribute('aria-pressed')).toBe('true');
  });

  it('treats an empty layout as no layout, not as show-nothing', async () => {
    // `{}` for a channel would otherwise turn every zone off by way of the
    // key-by-key merge. Absent and empty are a working screen and a blank one.
    zonesReply = { 2: {} };
    localStorage.setItem('relay.stage.zones', JSON.stringify({ ...ALL_ON, reading: false }));
    await open();
    await openPanel();
    // The device said reading OFF. If `{}` were treated as a layout, the
    // key-by-key merge would hand back the DEFAULT (on) and lock the toggles —
    // which on a default install looks exactly like the fallback it replaced.
    // So this asserts the device's own value survived, not merely that
    // something is on.
    expect(zoneButton('Reading').getAttribute('aria-pressed')).toBe('false');
    expect(zoneButton('Reading').disabled, 'an empty entry took the toggles away').toBe(false);
    expect(host.textContent).not.toMatch(/desk has given this screen a layout/i);
  });
});
