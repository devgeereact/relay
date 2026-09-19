// HOW THE PREACHER'S PHONE COMES BACK.
//
// The page reconnects on `onclose` after a flat 1.5 seconds and has no other
// trigger. Three gaps follow from that, all of them on the screen a preacher is
// holding during a service (plan phase 2).
//
// 1. **A flat retry is wrong at both ends.** Through a real outage — the wifi
//    access point rebooting, the laptop's lid closed between services — it
//    reconnects forty times a minute for as long as it takes, on a battery, in
//    somebody's hand. A backoff fixes that and immediately creates the opposite
//    problem: the phone that has just woken up is now the one waiting.
//
// 2. **So waking up has to be a trigger.** A phone sleeps for twenty minutes of
//    a sermon and comes back; `visibilitychange` and `online` are the two moments
//    the platform tells us something changed, and neither was listened for. The
//    backoff is only acceptable BECAUSE these exist: the long delays are for a
//    page nobody is looking at.
//
// 3. **Two triggers means two ways to open a socket.** Every guard below is
//    about there being exactly one — a page that reconnects on resume AND on a
//    pending timer opens two sockets, both of which say hello, both of which get
//    the retained frame, and the one that loses the race goes on beating into a
//    channel that now has two clients answering for it.
//
// A stale connection is a fourth case and not the same as a closed one: the
// socket is open, so nothing will ever fire `onclose`, and only an explicit
// close can get the page a working one.
//
//   npx vitest run src/lib/stagereconnect.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { BEAT_INTERVAL_MS } from './outputHealth.js';

const Stage = (await import('../Stage.svelte')).default;

/** Every socket the page has ever opened, in order. */
let sockets;
class FakeSocket {
  constructor(url) {
    this.url = url;
    this.sent = [];
    this.readyState = 0;
    sockets.push(this);
  }
  open() { this.readyState = 1; this.onopen?.(); }
  send(m) { this.sent.push(m); }
  close() {
    if (this.readyState === 3) return;
    this.readyState = 3;
    this.onclose?.();
  }
}

const live = () => sockets.filter((s) => s.readyState !== 3);
const latest = () => sockets.at(-1);

let host;
let app;

beforeEach(() => {
  sockets = [];
  globalThis.WebSocket = FakeSocket;
  try { localStorage.removeItem('relay.stage.zones'); } catch { /* shimmed */ }
});
afterEach(() => {
  vi.useRealTimers();
  app?.$destroy();
  host?.remove();
  app = null; host = null;
  window.history.replaceState({}, '', '/');
});

async function open({ connect = true } = {}) {
  window.history.replaceState({}, '', '/stage.html?channel=2');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  if (connect) { latest().open(); await tick(); }
  await tick();
}

/** Tell the page the tab came back, the way a phone waking up does. */
function resume() {
  Object.defineProperty(document, 'hidden', { configurable: true, value: false });
  document.dispatchEvent(new Event('visibilitychange'));
}
function hide() {
  Object.defineProperty(document, 'hidden', { configurable: true, value: true });
  document.dispatchEvent(new Event('visibilitychange'));
}

describe('a phone reconnecting through a real outage', () => {
  /** Close the current socket and report how long until a new one appears. */
  async function timeToRetry() {
    const before = sockets.length;
    latest().close();
    let waited = 0;
    while (sockets.length === before && waited < 120_000) {
      await vi.advanceTimersByTimeAsync(250);
      waited += 250;
    }
    return waited;
  }

  it('waits longer after each successive failure, rather than at a fixed rate', async () => {
    vi.useFakeTimers();
    await open();
    const first = await timeToRetry();
    await timeToRetry();
    const third = await timeToRetry();
    expect(first, 'the first retry should still be prompt').toBeLessThanOrEqual(2000);
    expect(
      third,
      `the third retry waited ${third}ms and the first ${first}ms — a flat rate, not a backoff`,
    ).toBeGreaterThan(first);
  });

  it('caps the wait, so a phone left alone still comes back on its own', async () => {
    vi.useFakeTimers();
    await open();
    let last = 0;
    for (let i = 0; i < 10; i += 1) last = await timeToRetry();
    expect(last, 'the backoff grew without limit').toBeLessThanOrEqual(30_000);
    expect(last, 'it stopped retrying altogether').toBeGreaterThan(0);
  });
});

describe('waking up is a trigger, and there is only ever one socket', () => {
  it('reconnects at once when the tab comes back, instead of waiting out the backoff', async () => {
    vi.useFakeTimers();
    await open();
    hide();
    latest().close();
    // Deep into the backoff, where a sleeping phone would otherwise sit.
    await vi.advanceTimersByTimeAsync(30_000);
    const waiting = sockets.length;
    resume();
    await tick();
    expect(sockets.length, 'coming back to the page did not retry').toBeGreaterThan(waiting);
  });

  it('does not open a second socket when the tab comes back to a working one', async () => {
    vi.useFakeTimers();
    await open();
    resume();
    resume();
    await tick();
    expect(live().length, 'the page is now holding more than one socket').toBe(1);
  });

  it('does not let a resume and a pending retry both fire', async () => {
    vi.useFakeTimers();
    await open();
    latest().close();
    resume();            // reconnects now
    await tick();
    await vi.advanceTimersByTimeAsync(60_000); // the timer that was already pending
    expect(live().length, 'a pending retry opened a socket beside the resumed one').toBe(1);
  });

  it('replaces a socket that is open but not answering, which nothing else can', async () => {
    // A half-open socket never fires `onclose`, so the retry path is unreachable
    // by construction. Only an explicit close gets the page a working one.
    vi.useFakeTimers();
    await open();
    const dead = latest();
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 4); // no acks: stale
    await tick();
    resume();
    await tick();
    expect(dead.readyState, 'the stale socket was left open').toBe(3);
    expect(live().length).toBe(1);
    expect(latest()).not.toBe(dead);
  });

  it('opens nothing at all once the page is destroyed', async () => {
    vi.useFakeTimers();
    await open();
    latest().close();
    app.$destroy();
    app = null;
    const after = sockets.length;
    await vi.advanceTimersByTimeAsync(120_000);
    resume();
    expect(sockets.length, 'a destroyed page went on reconnecting').toBe(after);
  });
});

describe('what the header says while nobody is answering', () => {
  it('reports how long it has been, not just that something is wrong', async () => {
    // "not answering" reads the same at four seconds and at ten minutes, and
    // those want different actions from whoever is holding the phone.
    vi.useFakeTimers();
    await open();
    await vi.advanceTimersByTimeAsync(BEAT_INTERVAL_MS * 4);
    await tick();
    const status = () => host.querySelector('.status')?.textContent ?? '';
    expect(status()).toMatch(/not answering/);
    await vi.advanceTimersByTimeAsync(40_000);
    await tick();
    // Scoped to the status line: an unscoped match would also find the wall
    // clock and pass whatever the header said.
    expect(status(), 'the age of the silence is not shown').toMatch(/\d+\s*s/);
  });
});
