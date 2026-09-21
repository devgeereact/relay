// DECISIONS §91 — A PANIC CONTROL SILENCES A ROOM; IT DOES NOT STOP THE CLOCKS.
//
// The stage page's clear/black branch reset five fields and deliberately kept a
// sixth, and for the whole life of the page it also kept a seventh by accident.
// `alert` was never reset and nothing in the tree recorded why — a third answer,
// given by nobody, to the question §91 exists to settle.
//
// ── Why this is its own file ──────────────────────────────────────────────────
//
// `stagezones.test.js` owns what the six zones show; this owns what a PANIC
// CONTROL takes. The two halves of §91 have to be asserted together or either one
// can be tidied into the other later: a future edit that drops `alert = ''` back
// out, and a future edit that "consistently" resets the service clock beside it,
// are opposite mistakes and only a test holding both catches both.
//
// The alert matters more than its size suggests. `.alert` is `position: fixed;
// inset: 0` — the page's own comment says "this panel IS the screen" — so an
// operator pressing `B`, whose entire meaning is *every output goes opaque
// black*, left the preacher's tablet as a full-bleed pulsing red panel while the
// console correctly reported the control had succeeded.
//
//   npx vitest run src/lib/stagepanic.test.js
// NOTE (2026-09-21, DECISIONS §116): a Stage Message is now a NOTE by default and
// an ALARM only when the operator asks. Every frame below carries `urgent: true`
// because these tests are about the alarm — the full-bleed panel, its sizing and
// what a panic control does to it. The quiet path has its own tests in
// `stagemessagenative.test.js`.

import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';

const Stage = (await import('../Stage.svelte')).default;

// The page opens a kiosk socket on mount. A stub, so the component mounts exactly
// as it does in a browser and frames go through the real `apply`.
let socket;
class FakeSocket {
  constructor() {
    socket = this;
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
  globalThis.WebSocket = FakeSocket;
  try {
    localStorage.removeItem('relay.stage.zones');
  } catch {
    /* the shim in test-setup.js hands us a real Storage */
  }
});
afterEach(() => {
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
});

/**
 * Mount the stage page with a socket open and nothing delivered yet.
 *
 * ON CHANNEL 2, with the role map delivered first. `stage.html` now refuses a
 * Stage Message unless its own channel holds the `stage` role
 * (`stagepageidentity.test.js`), and channel 2 is the screen a fresh install
 * seeds as `Stage display` — so this is the preacher's real monitor rather than
 * an anonymous page. Without the identity the alert never renders and every
 * assertion below about taking it DOWN would pass by it never having been up.
 */
async function mount() {
  window.history.replaceState({}, '', '/stage.html?channel=2');
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  socket.onmessage({ data: JSON.stringify({ kind: 'channel_roles', roles: { 1: 'main', 2: 'stage' } }) });
  await tick();
  return host;
}

async function send(frame) {
  socket.onmessage({ data: JSON.stringify(frame) });
  await tick();
  await tick();
}

const VERSE = {
  kind: 'content',
  reference: 'Romans 8:28',
  text: 'And we know that all things work together for good.',
  translation: 'KJV',
  stage_note: 'Wrap at 11:40',
  // A service that started a minute ago. This is the figure §27 already made an
  // exception for, on a stated ground, long before anybody wrote §91 down.
  service_started_at: Date.now() - 60_000,
};

const ALERT = { kind: 'stage_alert', text: 'Wrap up — 5 minutes', urgent: true };

/** Every panic control, by the name the hub publishes it under. */
const PANIC = ['clear', 'black'];

describe('DECISIONS §91 · a Stage Message comes down with the screens', () => {
  // Both controls, deliberately, in both branches. The branch's own comment says
  // that if Relay ever lets the stage survive a panic it must survive BOTH, "not
  // by one of them being forgotten" — and `black` is named here because `black`
  // is the one that WAS forgotten, once, in this exact branch.
  for (const kind of PANIC) {
    it(`\`${kind}\` takes the alert off the preacher's screen`, async () => {
      const container = await mount();
      await send(VERSE);
      await send(ALERT);

      const panel = container.querySelector('.alert');
      expect(panel, 'the alert never rendered — the test is asserting nothing').toBeTruthy();
      expect(panel.textContent).toContain('Wrap up');

      await send({ kind });

      // `.alert` is the whole screen. A panic control that leaves it up leaves one
      // screen in the room brighter than every other, under a console that has
      // just reported the control succeeding.
      expect(
        container.querySelector('.alert'),
        `\`${kind}\` left a full-screen alert on the stage monitor`,
      ).toBeNull();
    });

    it(`\`${kind}\` takes the reading and the note with it`, async () => {
      const container = await mount();
      await send(VERSE);
      expect(container.querySelector('.reading').textContent).toContain('Romans 8:28');
      expect(container.querySelector('.noterow')?.textContent).toContain('Wrap at 11:40');

      await send({ kind });

      // The half that was already true, asserted here so the alert's answer reads
      // as the same answer rather than a special case of its own.
      expect(container.querySelector('.reading')?.textContent ?? '').not.toContain('Romans 8:28');
      expect(container.textContent).not.toContain('Wrap at 11:40');
    });

    it(`\`${kind}\` does not stop the service clock`, async () => {
      const container = await mount();
      await send(VERSE);
      await send(ALERT);
      await send({ kind });

      // THE OTHER HALF OF §91, and the reason the alert's answer is a decision
      // rather than a tidy-up. A cleared or blacked wall is not the end of a
      // service. The line is between a thing that COUNTS and a thing that SAYS
      // something — and a test that only held the first half would let somebody
      // "consistently" reset this one next.
      expect(
        container.textContent,
        `\`${kind}\` took the preacher's own clock as well`,
      ).toContain('Elapsed');
    });
  }

  it('an alert sent AFTER a panic still reaches the screen', async () => {
    // Clearing the field must not disarm the channel. A panic control takes back
    // what was said; it does not mute the operator for the rest of the service.
    const container = await mount();
    await send(VERSE);
    await send(ALERT);
    await send({ kind: 'clear' });
    await send({ kind: 'stage_alert', text: 'Two minutes', urgent: true });

    expect(container.querySelector('.alert')?.textContent).toContain('Two minutes');
  });
});
