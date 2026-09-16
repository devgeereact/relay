// WHAT `Esc` AND `B` DO TO A WORD TO THE PREACHER — ratified, not inherited.
//
// ── Why this file exists ─────────────────────────────────────────────────────
//
// `Stage.svelte`'s `clear`/`black` branch resets six fields and leaves two
// standing. One of the two, `svcStart`, says at the line why it survives: a
// cleared wall is not the end of a service and the elapsed zone is the preacher's
// own clock. The other, `alert`, said nothing. It was a silent third answer to a
// question the predecessor spec left open, and it was indistinguishable from a
// field somebody forgot — which is the shape that branch's own comment warns
// about: "If Relay ever decides the stage monitor should survive a panic, it must
// survive BOTH controls, deliberately, in both branches — not by one of them
// being forgotten."
//
// DECISIONS §89 settles it: the alert survives, because it is an instruction to a
// PERSON rather than a state of the wall, and the moment the operator blanks the
// screens is exactly when the preacher most needs to be told why. The behaviour
// did not change. This is what stops it being an accident.
//
// The tests below were watched to fail by adding `alert = ''` to that branch —
// i.e. by taking the ruling the other way — and the last one was watched to fail
// by removing `note = ''`, so the file cannot pass by asserting nothing.
//
//   npx vitest run src/lib/stagealertpanic.test.js
import { describe, it, expect, beforeEach, afterEach } from 'vitest';
import { tick } from 'svelte';

const Stage = (await import('../Stage.svelte')).default;

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

const MESSAGE = 'Wrap up — 5 minutes';
const NOTE = 'hold for prayer';

async function mount() {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  await tick();
}

function send(frame) {
  socket.onmessage({ data: JSON.stringify(frame) });
}

const painted = () => host.textContent;

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

/** A verse with the operator's private note on it, then the alert beside it. */
async function aServiceInProgress() {
  await mount();
  send({
    kind: 'content',
    reference: 'John 3:16',
    text: 'For God so loved the world',
    stage_note: NOTE,
  });
  send({ kind: 'stage_alert', text: MESSAGE });
  await tick();
  expect(painted()).toContain(MESSAGE);
  expect(painted()).toContain(NOTE);
}

describe('a word to the preacher outlives a panic control, deliberately', () => {
  it('survives `clear`', async () => {
    await aServiceInProgress();
    send({ kind: 'clear' });
    await tick();
    expect(painted(), 'the message telling the preacher why the room went dark went with it')
      .toContain(MESSAGE);
  });

  it('survives `black` — the harsher control does not do MORE than the milder one here', async () => {
    // The branch handles both kinds together, so these two can only differ if
    // somebody splits it. That is precisely the split the branch's own comment
    // says must never happen by accident.
    await aServiceInProgress();
    send({ kind: 'black' });
    await tick();
    expect(painted()).toContain(MESSAGE);
  });

  it('is still cleared by its own empty frame, which is what makes surviving defensible', async () => {
    // "It survives a panic control" is only tolerable while the operator has a
    // one-action way to take it back. If this ever stops working, the ruling in
    // DECISIONS §89 has to be reopened rather than inherited.
    await aServiceInProgress();
    send({ kind: 'clear' });
    send({ kind: 'stage_alert', text: null });
    await tick();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('and the operator NOTE, which rides with the content, still goes', async () => {
    // The distinction the ruling rests on, asserted rather than described: a note
    // describes the slide and a cleared slide makes it wrong; an alert is
    // addressed to a person and arrives on its own frame. If both survived, the
    // ruling would just be "this branch stopped clearing things".
    await aServiceInProgress();
    send({ kind: 'clear' });
    await tick();
    expect(painted()).not.toContain(NOTE);
    expect(painted()).not.toContain('For God so loved the world');
  });
});
