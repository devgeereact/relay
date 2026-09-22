// A STAGE MESSAGE IS SIZED BY WHAT ELSE IS ON THE SCREEN (RG-239).
//
// The operator: *"When there is no text or any slide on the screen Make the
// Stage message Bigger... and make it alert text should be colored flashing.....
// Even when any stage message is sent to get attention"*.
//
// Today `.quietmsg` is a one-line strip along the foot — `bottom: 2vh`,
// `padding: 1.1vh 2.4vw` — and it is that size whether the screen is full of
// verse or completely empty. A preacher glancing at a dark phone from a platform
// reads nothing.
//
// **Three states, not two.** The alert is unchanged and still takes the whole
// screen. What is new is that an ordinary message has a LARGE form, used when
// there is nothing else to look at, and the strip it has today, used when there
// is. A reading is why the preacher is looking at the screen; a message may not
// take that room while one is up.
import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { messagePlacement } from './stagemessage.js';

const at = (over = {}) =>
  messagePlacement({ message: 'Wrap up in five', urgent: false, reading: false, slide: false, ...over });

describe('messagePlacement — which of the three a message gets', () => {
  it('nothing else on screen: the message takes the reading’s room', () => {
    expect(at()).toBe('large');
  });

  it('a reading is up: the strip, because the reading is why they are looking', () => {
    expect(at({ reading: true })).toBe('strip');
  });

  it('a slide is up: the strip too — the operator put that there to be read', () => {
    expect(at({ slide: true })).toBe('strip');
  });

  it('an alert is the whole screen whatever else is on it', () => {
    expect(at({ urgent: true })).toBe('alert');
    expect(at({ urgent: true, reading: true })).toBe('alert');
    expect(at({ urgent: true, slide: true })).toBe('alert');
  });

  it('and no message is no placement — not a small one', () => {
    // An empty string and a string of spaces are the same absence. A blank panel
    // sitting over a preacher's clock is worse than no panel.
    for (const m of ['', '   ', null, undefined])
      expect(messagePlacement({ message: m, reading: false, slide: false }), JSON.stringify(m)).toBe('none');
  });
});

// ── AND THE PAGE ACTUALLY PAINTS THEM (RG-239) ──────────────────────────────
//
// The rule above is held against arguments; this holds that `stage.html` asks it
// and acts on the answer. Both halves are needed: a correct rule nothing calls
// is the shape of most of this register.
describe('the preacher’s screen paints the three states', () => {
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
    window.history.replaceState({}, '', '/stage.html?channel=2');
    host = document.createElement('div');
    document.body.appendChild(host);
    const Stage = (await import('../Stage.svelte')).default;
    app = new Stage({ target: host });
    await tick();
    socket.onopen?.();
    await settle();
    send({ kind: 'channel_roles', roles: { 2: 'stage' } });
    await settle();
  }
  const send = (frame) => socket.onmessage({ data: JSON.stringify(frame) });
  const msg = (text, urgent = false) => send({ kind: 'stage_alert', text, urgent });

  afterEach(() => {
    app?.$destroy();
    host?.remove();
    app = null;
    host = null;
    window.history.replaceState({}, '', '/');
  });

  it('with an empty screen the message is the LARGE form', async () => {
    await open();
    msg('Wrap up in five');
    await settle();
    expect(host.querySelector('.bigmsg'), 'a message on an empty screen is still a strip').toBeTruthy();
    expect(host.querySelector('.quietmsg')).toBeNull();
    expect(host.querySelector('.bigmsg').textContent).toContain('Wrap up in five');
  });

  it('with a reading up it is the strip, and the reading keeps its room', async () => {
    await open();
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    msg('Wrap up in five');
    await settle();
    expect(host.querySelector('.quietmsg'), 'the message took the reading’s room').toBeTruthy();
    expect(host.querySelector('.bigmsg')).toBeNull();
    expect(host.textContent).toContain('For God so loved');
  });

  it('an alert is the whole screen, over a reading', async () => {
    await open();
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    msg('STOP', true);
    await settle();
    expect(host.querySelector('.alert')).toBeTruthy();
    expect(host.querySelector('.bigmsg')).toBeNull();
    expect(host.querySelector('.quietmsg')).toBeNull();
  });

  it('and every ordinary message pulses — not only the alert', async () => {
    // The operator's own words: *"Even when any stage message is sent to get
    // attention"*. The large form and the strip both carry it.
    await open();
    msg('Wrap up in five');
    await settle();
    expect(host.querySelector('.bigmsg .pulse'), 'the large message does not move').toBeTruthy();

    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    await settle();
    expect(host.querySelector('.quietmsg.pulse'), 'the strip does not move').toBeTruthy();
  });

  it('a reduced-motion viewer gets the colour without the movement', async () => {
    // Not a quieter state: the same ink, at rest. A viewer who has asked for no
    // animation still sees a coloured message rather than a plain one.
    const src = readFileSync(resolve(__dirname, '../Stage.svelte'), 'utf8');
    // THE BLOCK THAT MENTIONS `.pulse`, not the first `reduce` block in the file
    // — this page has two, and the other one is about a countdown's warning. A
    // scanner that took the first would be reading the wrong rule and would
    // pass or fail for reasons that have nothing to do with a message.
    const at = src.indexOf('.pulse { color: var(--v-caution');
    expect(at, 'a reduced-motion viewer gets no colour at all').toBeGreaterThan(-1);
    const before = src.slice(0, at);
    expect(
      before.lastIndexOf('@media (prefers-reduced-motion: reduce)'),
      'the resting colour is not inside a reduced-motion block',
    ).toBeGreaterThan(before.lastIndexOf('@media (prefers-reduced-motion: no-preference)'));
  });

  it('and the message goes down with the screen', async () => {
    // DECISIONS §91: a panic control takes back every sentence anybody put on a
    // screen, and that includes this one.
    await open();
    msg('Wrap up in five');
    await settle();
    expect(host.querySelector('.bigmsg')).toBeTruthy();
    send({ kind: 'black' });
    await settle();
    expect(host.querySelector('.bigmsg'), 'a blackout left a message up').toBeNull();
  });
});
