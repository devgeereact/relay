// WHAT THE PREACHER'S PHONE IS ALLOWED TO SAY ABOUT A REQUEST THAT DID NOT COME BACK.
//
// ── Two defects, one root (plan S9) ────────────────────────────────────────
//
// `Stage.svelte`'s control panel drives the same fire/nav path the console does,
// over `:8032/api`. It had no deadline on any request and no way to tell a
// transport failure from an answer.
//
// 1. **A request that never settles disabled the panel for the rest of the
//    service.** `busy` is set before the await and cleared in `finally`; a
//    `fetch` that neither resolves nor rejects — a phone that has roamed to a
//    dead AP, a laptop asleep behind a NAT that swallows the SYN — never
//    reaches the `finally`. Every button stays disabled, silently, and the only
//    way out is reloading the page mid-sermon.
//
// 2. **A network failure wore the words of a correct boundary.** The `catch`
//    said `'No next verse.'` — which is what Relay says when the reading has
//    genuinely ended. So the preacher was told a true-sounding fact about the
//    passage when what actually happened was that the phone could not reach the
//    building's computer. Worse, it is not even a safe lie: the request may have
//    REACHED the server and executed, with only the reply lost. Reporting "no
//    next verse" over a wall that just advanced is the same class of error as a
//    panic control reporting a success it did not achieve (rule 15).
//
// The rule this file pins: **a mutating request that did not come back is an
// UNCERTAIN outcome, and the phone says so.** It never claims nothing happened,
// and it never retries on its own — an automatic retry after an ambiguous fire
// is how a verse gets fired twice, or a wall gets cleared after the operator has
// already put something back.
//
// Read-only requests are exempt, deliberately: a failed `search` moved nothing,
// so "Search failed" is the whole truth.
//
//   npx vitest run src/lib/stagecontrol.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

const Stage = (await import('../Stage.svelte')).default;

let socket;
class FakeSocket {
  constructor() { socket = this; this.sent = []; this.readyState = 1; }
  send(m) { this.sent.push(m); }
  close() { this.closed = true; this.readyState = 3; }
}

let host;
let app;
let fetchMock;

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  fetchMock = vi.fn();
  globalThis.fetch = fetchMock;
  try { localStorage.removeItem('relay.stage.zones'); } catch { /* shimmed */ }
});
afterEach(() => {
  vi.useRealTimers();
  app?.$destroy();
  host?.remove();
  app = null; host = null;
  window.history.replaceState({}, '', '/');
});

/** A JSON reply the way `remote_api` sends one. */
const reply = (body) => Promise.resolve({ ok: true, json: () => Promise.resolve(body) });

async function open() {
  window.history.replaceState({}, '', '/stage.html?channel=2');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  await tick();
  // The control panel is behind a press and HOLD since RG-242: it fires to every
  // screen in the building and used to open on one tap of a small button in the
  // header of a page a preacher is holding mid-sermon.
  const toggle = [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Hold');
  expect(
    toggle,
    `no control toggle; buttons are ${JSON.stringify([...host.querySelectorAll('button')].map((b) => b.textContent.trim()))}`,
  ).toBeTruthy();
  // BOTH stamps are forced. jsdom gives a synthetic event the real time since
  // the page loaded, so an unforced `pointerdown` can be LATER than a forced
  // `pointerup` and the hold reads as negative.
  const down = new PointerEvent('pointerdown', { bubbles: true });
  Object.defineProperty(down, 'timeStamp', { value: 0 });
  toggle.dispatchEvent(down);
  // `timeStamp` is read from the event, so a synthetic pair a millisecond apart
  // is a TAP however long the test takes — the hold has to be expressed here.
  const held = new PointerEvent('pointerup', { bubbles: true });
  Object.defineProperty(held, 'timeStamp', { value: 5_000 });
  toggle.dispatchEvent(held);
  await tick();
  await tick();
}

/** Flush the promise chain behind a control, not just Svelte's queue. */
async function settle() {
  await Promise.resolve();
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const button = (label) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim().includes(label));
const said = () => host.textContent;
/** The mutating calls that actually left the phone. */
const posts = () => fetchMock.mock.calls.filter(([, init]) => init?.method === 'POST');

describe('a request that does not come back', () => {
  it('is abandoned on a deadline instead of disabling the panel for the service', async () => {
    vi.useFakeTimers();
    await open();
    // Never settles, and never rejects. This is the case a bare `fetch` cannot
    // escape from: without an abort there is no path to the `finally`.
    fetchMock.mockImplementation(() => new Promise(() => {}));
    button('Next').click();
    await tick();
    expect(button('Next').disabled, 'the panel should be busy while in flight').toBe(true);
    await vi.advanceTimersByTimeAsync(30_000);
    await tick();
    expect(button('Next').disabled, 'the panel never came back').toBe(false);
  });

  it('passes an abort signal, so the socket is actually released', async () => {
    vi.useFakeTimers();
    await open();
    fetchMock.mockImplementation(() => new Promise(() => {}));
    button('Next').click();
    await tick();
    const [, init] = fetchMock.mock.calls.at(-1);
    expect(init?.signal, 'no AbortSignal was given to fetch').toBeTruthy();
    expect(init.signal.aborted).toBe(false);
    await vi.advanceTimersByTimeAsync(30_000);
    expect(init.signal.aborted, 'the deadline did not abort the request').toBe(true);
  });
});

describe('an uncertain outcome is never reported as a fact about the passage', () => {
  it('does not say "No next verse" when the phone could not reach Relay', async () => {
    await open();
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    button('Next').click();
    await settle();
    expect(said()).not.toContain('No next verse');
    expect(said()).not.toContain('End of the reading');
    expect(said().toLowerCase()).toContain('did not answer');
  });

  it('still reports a genuine end of passage in the words for it', async () => {
    // The guard against over-correcting: a real boundary must keep its own
    // sentence, or this fix has traded one wrong message for another.
    await open();
    fetchMock.mockReturnValue(reply({ ok: true, nav: { kind: 'end_of_passage' } }));
    button('Next').click();
    await settle();
    expect(said()).toContain('End of the reading');
    expect(said().toLowerCase()).not.toContain('did not answer');
  });

  it('does not claim a failed fire changed nothing', async () => {
    await open();
    fetchMock.mockReturnValue(reply({ ok: true, results: [{ reference: 'John 3:16', text: 'For God so loved' }] }));
    const box = host.querySelector('input[type="search"], input[type="text"], input');
    box.value = 'John 3:16';
    box.dispatchEvent(new Event('input'));
    await settle();
    const hit = [...host.querySelectorAll('button')].find((b) => b.textContent.includes('John 3:16'));
    expect(hit, 'no search result to fire').toBeTruthy();
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    hit.click();
    await settle();
    expect(said().toLowerCase()).toContain('did not answer');
  });

  it('never retries a mutating request on its own', async () => {
    // One tap is one attempt. An automatic retry after an ambiguous fire is how
    // a verse reaches the wall twice, or a wall is cleared after the operator
    // has already put something back.
    vi.useFakeTimers();
    await open();
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    button('Next').click();
    await tick(); await tick();
    const after = posts().length;
    expect(after).toBe(1);
    await vi.advanceTimersByTimeAsync(60_000);
    expect(posts().length).toBe(after);
  });
});

describe('a read-only request keeps its plain words', () => {
  it('says a search failed, because a failed search moved nothing', async () => {
    await open();
    fetchMock.mockRejectedValue(new TypeError('Failed to fetch'));
    const box = host.querySelector('input');
    box.value = 'John';
    box.dispatchEvent(new Event('input'));
    await settle();
    expect(said()).toContain('Search failed');
    expect(said().toLowerCase()).not.toContain('did not answer');
  });
});
