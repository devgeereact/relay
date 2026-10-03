// 2026-09-21 · O-2 and O-1 (RG-193, RG-194). `output.html` reconnected only on
// `onclose`/`onerror`, so a half-open socket — wifi roaming, a sleeping kiosk, a
// NAT timeout — froze the last frame for the rest of a service; and its
// countdown ticked on the screen's OWN clock while the stage page had corrected
// for host skew via `beat_ack` since DECISIONS §100. Both rules the stage page
// already keeps, now kept on the door that faces the room.
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { BEAT_INTERVAL_MS } from './outputHealth.js';

const Output = (await import('../Output.svelte')).default;

let sockets;
class FakeSocket {
  constructor(url) { this.url = url; this.sent = []; this.readyState = 0; sockets.push(this); }
  open() { this.readyState = 1; this.onopen?.(); }
  send(m) { this.sent.push(m); }
  close() { if (this.readyState === 3) return; this.readyState = 3; this.onclose?.(); }
  receive(frame) { this.onmessage?.({ data: JSON.stringify(frame) }); }
}
const live = () => sockets.filter((s) => s.readyState !== 3);
const latest = () => sockets.at(-1);

let host, app;
beforeEach(() => { sockets = []; globalThis.WebSocket = FakeSocket; });
afterEach(() => { vi.useRealTimers(); app?.$destroy(); host?.remove(); app = null; host = null; window.history.replaceState({}, '', '/'); });

async function open() {
  window.history.replaceState({}, '', '/output.html?channel=2');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  for (let i = 0; i < 6; i++) await tick();
  // No Tauri bridge in jsdom: the page falls back to the kiosk socket.
  let waited = 0;
  while (!sockets.length && waited < 50) { await new Promise((r) => setTimeout(r, 0)); waited++; }
  expect(sockets.length, 'the page never opened a kiosk socket').toBeGreaterThan(0);
  latest().open();
  await tick();
}
function resume() {
  Object.defineProperty(document, 'hidden', { configurable: true, value: false });
  document.dispatchEvent(new Event('visibilitychange'));
}

describe('a congregation screen on a half-open socket', () => {
  it('replaces a socket that is open but not answering', async () => {
    await open();
    vi.useFakeTimers();
    const dead = latest();
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 4);
    await tick();
    resume();
    await tick();
    expect(dead.readyState, 'the stale socket was left open').toBe(3);
    expect(live().length).toBe(1);
    expect(latest()).not.toBe(dead);
  });

  it('does not replace a socket that is answering', async () => {
    await open();
    vi.useFakeTimers();
    const good = latest();
    for (let i = 0; i < 4; i++) {
      await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS);
      good.receive({ kind: 'beat_ack', channel: 2, at: Date.now() });
    }
    await tick();
    resume();
    await tick();
    expect(good.readyState).toBe(1);
    expect(latest()).toBe(good);
  });
});

describe('the wall countdown keeps Relay\'s time, not the screen\'s', () => {
  it('a beat_ack a minute ahead moves the figure by a minute', async () => {
    await open();
    const s = latest();
    const t0 = Date.now();
    s.receive({ kind: 'content', content_kind: 'countdown', reference: 'Service starts', text: '', countdown_to: t0 + 10 * 60_000, countdown_from: t0, template_id: null });
    await tick(); await tick();
    const digits = () => (host.textContent.match(/\d{1,2}:\d{2}/) || [''])[0];
    const before = digits();
    expect(before, 'no countdown rendered').toMatch(/\d/);
    // Relay's clock is a minute AHEAD of this screen's: five acks make a median.
    for (let i = 0; i < 5; i++) s.receive({ kind: 'beat_ack', channel: 2, at: Date.now() + 60_000 });
    await new Promise((r) => setTimeout(r, 300));
    await tick();
    const after = digits();
    const secs = (d) => { const [m, ss] = d.split(':').map(Number); return m * 60 + ss; };
    expect(secs(before) - secs(after)).toBeGreaterThanOrEqual(58);
  });
});
