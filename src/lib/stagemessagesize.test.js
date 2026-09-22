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

const SRC = readFileSync(resolve('src/Stage.svelte'), 'utf8');

const at = (over = {}) =>
  messagePlacement({ message: 'Wrap up in five', urgent: false, reading: false, slide: false, ...over });

describe('messagePlacement — which of the two a message gets', () => {
  it('nothing else on screen: the message takes the reading’s room', () => {
    expect(at()).toBe('large');
  });

  it('a reading is up: it takes that room too (RG-245)', () => {
    // THE DECISION REVERSED. This shipped as a strip along the foot, on the
    // argument that a reading is why the preacher is looking at the screen. The
    // operator watched it on a phone and overruled it: a Stage Message is the
    // desk speaking to ONE person mid-sermon and is never ambient, and the
    // reading they are holding is the thing it is most often about.
    expect(at({ reading: true })).toBe('large');
  });

  it('a slide is up: the same', () => {
    expect(at({ slide: true })).toBe('large');
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

  it('with a reading up it covers it, and the clock survives (RG-245)', async () => {
    await open();
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    msg('Wrap up in five');
    await settle();
    expect(host.querySelector('.bigmsg'), 'the message is still a strip beside a reading').toBeTruthy();
    expect(host.querySelector('.quietmsg'), 'the strip survived the reversal').toBeNull();
    // THE ONE THING A MESSAGE MAY NOT TAKE. The preacher still has to know how
    // long is left while they read it.
    expect(host.querySelector('.progrow, .figrow'), 'the message covered the clock').toBeTruthy();
  });

  it('it takes the reading’s ROOM, not the whole page (RG-247)', async () => {
    // MEASURED ON A REAL PHONE-SIZED VIEWPORT, not argued. `.bigmsg` was
    // `position: absolute; inset: 0` against the page, while the comment beside
    // it said "it sits in the reading's own box". The rule and the sentence
    // disagreed, and the rule won: at 390×844 the message spanned all 844px and
    // centred its text at y=420 — under 281px of clock rows that paint over it,
    // so the words a preacher is meant to read sat 140px below the middle of the
    // room they actually had, with 63px of clearance above the clock.
    //
    // A message that is IN the column cannot make that mistake: the rows follow
    // it, the browser does the arithmetic, and nothing has to know how tall the
    // clocks happen to be today.
    await open();
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    msg('Wrap up in five');
    await settle();
    // COMMENTS STRIPPED FIRST. The rule below is explained by a comment that
    // QUOTES the declaration it replaced, so a scanner reading the raw text
    // fails on the prose describing the fix — the opposite of this repository's
    // usual scanner fault, and the same lesson: read what paints, not what is
    // written beside it.
    const decls = (css) => css.replace(/\/\*[\s\S]*?\*\//g, '');
    const from = SRC.indexOf('.bigmsg {');
    const rule = decls(SRC.slice(from, SRC.indexOf('}', from)));
    expect(rule, 'the message is positioned against the PAGE').not.toMatch(/position:\s*absolute/);
    expect(rule, 'the message is positioned against the PAGE').not.toMatch(/inset:\s*0/);
    // AND IT TAKES THE ROOM RATHER THAN SITTING OVER IT: the verse is gone from
    // the page while a message is up, so there is no second thing competing for
    // the same box and no z-index deciding which one a preacher reads.
    expect(host.querySelector('.verse'), 'the reading is still under the message').toBeNull();
    // The clock rows still come AFTER it, in that order, which is what makes
    // them visible without a stacking rule.
    const order = [...host.querySelectorAll('.bigmsg, .figrow, .progrow')].map((n) => n.className.split(' ')[0]);
    expect(order[0]).toBe('bigmsg');
    expect(order.length, 'a clock row went with the reading').toBeGreaterThan(1);
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
    expect(host.querySelector('.bigmsg .pulse'), 'it stopped moving over a reading').toBeTruthy();
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
