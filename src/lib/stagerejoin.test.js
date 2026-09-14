// T3 · RULE 43 ON THE PREACHER'S SCREEN — a stage page that rejoins mid-service
// is shown what is on the screens.
//
// ── The hole this file closes ────────────────────────────────────────────────
//
// `KioskHub` retains the last `content` / `clear` / `black` frame and replays it
// to a client that joins late (CLAUDE.md rule 43, DECISIONS §68). It replays it
// in exactly ONE place: inside the `hello` handler in `run_kiosk_server`. A
// client that never says hello is never sent anything it missed.
//
// `Output.svelte` says hello. `Stage.svelte` did not — it opened the socket, set
// `connected = true`, and waited. So the preacher's phone locking its screen,
// dropping off the wifi for a moment, or simply being reloaded came back **blank
// and stayed blank until the next fire**, in the middle of the reading it exists
// to carry. That is RG-129's failure verbatim, on the one screen whose reader
// cannot glance at the console to find out what happened.
//
// This is the "guarantee kept on one door" shape CLAUDE.md names four times: the
// rule was real, the retained frame was real, the test for it was real
// (`channels::tests::a_client_that_connects_mid_service_is_sent_what_is_on_the
// _screens`) — and it was asserted at the hub, about a client that says hello,
// so no Rust test could see that one of the two pages never did.
//
// Both tests below were watched to FAIL with the `hello` send removed from
// `Stage.svelte`.
//
//   npx vitest run src/lib/stagerejoin.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const Stage = (await import('../Stage.svelte')).default;

let socket;
/** Every socket the page has opened, in order — the reconnect makes a second. */
let sockets;
class FakeSocket {
  constructor() {
    socket = this;
    sockets.push(this);
    this.sent = [];
  }
  send(m) {
    this.sent.push(m);
  }
  close() {
    this.closed = true;
  }
}

let host;
let app;

beforeEach(() => {
  socket = null;
  sockets = [];
  globalThis.WebSocket = FakeSocket;
});
afterEach(() => {
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
  vi.useRealTimers();
});

async function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
}

/** The frames this socket has sent, parsed. */
const framesOn = (s) =>
  s.sent.map((m) => {
    try {
      return JSON.parse(m);
    } catch {
      return null;
    }
  });

describe('a stage page that joins late asks for what it missed', () => {
  it('says hello as soon as its socket opens', async () => {
    await mount();
    socket.onopen?.();
    await tick();

    const hello = framesOn(socket).find((f) => f?.kind === 'hello');
    expect(
      hello,
      'the stage page opened a socket and never said hello — the hub replays the ' +
        'retained frame only in its hello handler, so this page comes back blank',
    ).toBeTruthy();
  });

  it('says hello again on every reconnect, not only the first connection', async () => {
    // The real case. A phone that locks its screen, walks out of wifi range or is
    // woken an hour later reconnects through `onclose`, and a hello sent once at
    // mount would leave every reconnection after the first one blank. The page
    // that is most likely to drop off the network is the one being carried.
    vi.useFakeTimers();
    await mount();
    socket.onopen?.();
    await tick();
    expect(sockets).toHaveLength(1);

    socket.onclose?.();
    vi.advanceTimersByTime(1600);
    await tick();
    expect(sockets.length, 'the page did not reconnect at all').toBeGreaterThan(1);

    const rejoined = sockets[sockets.length - 1];
    rejoined.onopen?.();
    await tick();

    expect(
      framesOn(rejoined).find((f) => f?.kind === 'hello'),
      'the reconnected socket never said hello, so the hub sent it nothing it missed',
    ).toBeTruthy();
  });

  it('paints the retained reading the hub replies with', async () => {
    // The other half of the contract, end to end through the real `apply`: what
    // arrives after a hello is an ordinary `content` frame, so a page that asks
    // is a page that comes back showing the verse rather than an empty screen.
    await mount();
    socket.onopen?.();
    await tick();
    socket.onmessage({
      data: JSON.stringify({
        kind: 'content',
        reference: 'Romans 8:28',
        text: 'And we know that all things work together for good…',
        translation: 'KJV',
      }),
    });
    await tick();
    await tick();

    expect(host.textContent).toContain('Romans 8:28');
  });
});
