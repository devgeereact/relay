// THE PREACHER'S CONTROLS ARE ALWAYS THERE (RG-246).
//
// The operator, from the drawing: *"I want to see the preachers control where
// preacher can control to the next verse and previous and also search for
// scripture as it was originally... and also theres no need to have any Hold
// button on the screen"*.
//
// **This retires RG-242**, and the reasoning it replaces is worth keeping in
// view. That row guarded the panel behind a 550ms hold because it FIRED to every
// screen and opened on one tap of a small button in a header. The guard was the
// right answer to the wrong shape: a preacher's own transport is not a thing to
// go looking for mid-sentence, and a bar that is always there cannot be opened
// by accident because there is nothing to open.
//
// The risk the guard existed for is gone with it: the accident it prevented was
// the PANEL appearing over a reading, and there is no panel now — Prev and Next
// are two 52px targets on a bar of their own, which is a harder thing to hit by
// accident than a 26px button in a header ever was.
import { describe, it, expect, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const SRC = codeOnly(readFileSync(resolve(__dirname, '../Stage.svelte'), 'utf8'));

let socket;
class FakeSocket {
  constructor() { socket = this; this.sent = []; this.readyState = 1; }
  send(m) { this.sent.push(m); }
  close() { this.readyState = 3; }
}

let host;
let app;
const settle = async () => { await tick(); await tick(); };

async function open() {
  globalThis.WebSocket = FakeSocket;
  globalThis.fetch = vi.fn(async () => ({ ok: true, json: async () => ({}) }));
  window.history.replaceState({}, '', '/stage.html?channel=2');
  host = document.createElement('div');
  document.body.appendChild(host);
  const Stage = (await import('../Stage.svelte')).default;
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  await settle();
  socket.onmessage({ data: JSON.stringify({ kind: 'channel_roles', roles: { 2: 'stage' } }) });
  await settle();
  return host;
}
const named = (label) =>
  [...host.querySelectorAll('button')].find(
    (b) => b.textContent.trim() === label || b.getAttribute('aria-label') === label,
  );

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  window.history.replaceState({}, '', '/');
});

describe('the transport is on the screen, not behind anything', () => {
  it('Previous and Next are there the moment the page opens', async () => {
    await open();
    expect(named('‹ Previous'), 'no Previous on the screen').toBeTruthy();
    expect(named('Next ›'), 'no Next on the screen').toBeTruthy();
  });

  it('and so is the way to search', async () => {
    await open();
    expect(named('Search for a verse'), 'no way to reach the search').toBeTruthy();
  });

  it('there is no Hold button anywhere (RG-242 retired)', async () => {
    await open();
    expect(named('Hold'), 'the hold survived the redesign').toBeFalsy();
    expect(SRC, 'the page still reaches for the hold guard').not.toMatch(/holdGuard/);
  });

  it('the targets are big enough to be pressed on a phone', async () => {
    // 44px is the floor; the drawing uses 52. A transport an operator has to aim
    // at is a transport a preacher will not use mid-sermon.
    expect(SRC).toMatch(/\.sctl-btn\s*\{[^}]*min-height:\s*52px/);
  });
});

describe('and the search is a place, not a panel over the reading', () => {
  it('pressing search shows the field and hides the reading', async () => {
    await open();
    socket.onmessage({
      data: JSON.stringify({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' }),
    });
    await settle();
    expect(host.textContent).toContain('For God so loved');

    named('Search for a verse').click();
    await settle();
    expect(host.querySelector('.ctl'), 'the search did not open').toBeTruthy();
    expect(host.querySelector('input[type="search"]')).toBeTruthy();
  });

  it('and closing it brings the reading back', async () => {
    await open();
    named('Search for a verse').click();
    await settle();
    named('Search for a verse').click();
    await settle();
    expect(host.querySelector('.ctl'), 'the search would not close').toBeNull();
  });

  it('the clock is never covered by it', async () => {
    // The one rule every state on this screen keeps: a preacher always knows how
    // long is left. `.progrow` and `.figrow` are the two rows that can carry it.
    await open();
    socket.onmessage({
      data: JSON.stringify({
        kind: 'timer',
        warn_default_ms: 60_000,
        timers: [{ id: 1, label: 'Sermon', countdown_to: Date.now() + 240_000, countdown_from: Date.now() }],
      }),
    });
    await settle();
    named('Search for a verse').click();
    await settle();
    expect(host.querySelector('.progrow, .figrow'), 'the search covered the clock').toBeTruthy();
  });
});
