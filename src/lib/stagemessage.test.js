// A WORD TO THE PREACHER, ON THE PAGE THAT MAY REFUSE IT — `output.html`.
//
// ── What changed, and why the old guarantee stopped holding ─────────────────
//
// `channels::stage_alert` publishes to EVERY kiosk client, and it has to: the hub
// records nothing about who connected, and DECISIONS §35 is not being reversed to
// let it address one screen. docs/REBRAND.md §5's guarantee — "it exists inside
// the stage renderer, so no congregation screen can show it" — therefore rested
// on an OMISSION: `Output.svelte` had no `stage_alert` branch, so the frame went
// past it. `r6-contracts.test.js` held exactly that, as a `false`.
//
// A `stage_message` layer binding ends the omission. A renderer now reads the
// value, so the absence protects nothing, and the refusal has to be a decision
// this page takes out loud. These tests drive the real page through the real
// socket and assert on what it PAINTS — the surface `r6-contracts.test.js`, which
// reads source text, can never reach.
//
// Every test below was watched to fail with the role check removed from
// `Output.svelte`'s `stage_alert` branch (see the notes on each).
//
//   npx vitest run src/lib/stagemessage.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

// The kiosk path: the Tauri bridge must be absent, exactly as it is in a browser
// source, so `onMount`'s import throws and `startKiosk()` runs.
vi.mock('@tauri-apps/api/core', () => {
  throw new Error('no tauri bridge in a browser source');
});
vi.mock('@tauri-apps/api/event', () => {
  throw new Error('no tauri bridge in a browser source');
});

const Output = (await import('../Output.svelte')).default;

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

/** A stage-shaped template: the verse, and a layer bound to Stage Message. */
const STAGE_TPL = {
  id: 7,
  name: 'Stage',
  layout: {
    layers: [
      { id: 'bg', type: 'background', fill: '#000', x: 0, y: 0, w: 100, h: 100 },
      { id: 'v', type: 'text', bind: 'verse', x: 5, y: 10, w: 90, h: 40, size: 4, color: '#fff' },
      {
        id: 'msg',
        type: 'text',
        bind: 'stage_message',
        x: 5,
        y: 60,
        w: 90,
        h: 20,
        size: 5,
        color: '#ff5555',
      },
    ],
  },
  style: { background: '#000' },
};

const VERSE = {
  kind: 'content',
  content_kind: 'scripture',
  reference: 'John 3:16',
  text: 'For God so loved the world',
};

const MESSAGE = 'Wrap up — 5 minutes';

let host;
let app;

async function settle() {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
}

/** Mount `output.html` on `channel`, then hand it the frames the hub sends. */
async function open(channel, roles) {
  window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=7`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  send({ kind: 'template', id: 7, template: STAGE_TPL });
  send({ kind: 'channel_roles', roles });
  await settle();
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

const SEEDED_ROLES = { 1: 'main', 2: 'stage' };

describe('only a screen whose role is `stage` paints a stage message', () => {
  it('paints it on the stage display', async () => {
    // Asserted FIRST, so nothing below can pass by the whole path being broken.
    await open(2, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await settle();
    expect(painted()).toContain(MESSAGE);
  });

  it('paints NOTHING on a lobby screen, which has no role at all', async () => {
    // The congregation screens of a fresh install arrive with `role` null. This
    // is the surface the repository's own lesson names: a guarantee is only kept
    // on the doors you checked, and this is the door that was never a door
    // because `Output.svelte` had no branch to get through.
    //
    // Watched to fail with the `acceptsStageMessage(myRole)` check removed: the
    // message is painted on the lobby TV.
    await open(4, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await settle();
    expect(painted()).toContain('For God so loved the world');
    expect(painted()).not.toContain(MESSAGE);
  });

  it('paints nothing on the MAIN screen either', async () => {
    // The wall the congregation is looking at. A role is not a permission ladder:
    // `main` is not a weaker `stage`.
    await open(1, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('paints nothing before the roles have arrived', async () => {
    // A page that has said hello and not yet been answered must refuse. Accepting
    // while the answer is outstanding would put the message on every screen in
    // the building for as long as the reply takes.
    window.history.replaceState({}, '', '/output.html?channel=2&template_id=7');
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    send({ kind: 'template', id: 7, template: STAGE_TPL });
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('paints nothing on a raw template preview, which is on no channel at all', async () => {
    // `?channel=` absent parses to 0. A preview that belongs to no screen must not
    // inherit a screen's job.
    window.history.replaceState({}, '', '/output.html?template_id=7');
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    send({ kind: 'template', id: 7, template: STAGE_TPL });
    send({ kind: 'channel_roles', roles: { 0: 'stage' } });
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });
});

describe('the message follows the role, in both directions', () => {
  it('goes the instant the screen stops being a stage', async () => {
    // An operator who moves the stage role off a tablet has said that tablet is a
    // congregation screen now. A refusal that only applied to the NEXT message
    // would leave the last one painted on it for the rest of the service.
    await open(2, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await settle();
    expect(painted()).toContain(MESSAGE);

    send({ kind: 'channel_roles', roles: { 1: 'main' } });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('is cleared by a blank message, not left on the glass', async () => {
    await open(2, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await settle();
    expect(painted()).toContain(MESSAGE);
    send({ kind: 'stage_alert', text: null });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('never rides on a content frame', async () => {
    // THE SHAPE THIS DESIGN EXISTS TO FORBID. If the value were a field on
    // `OutputContent` it would be broadcast to every screen with the verse, and
    // the only thing between it and a lobby TV would be which layers that TV's
    // template happens to have. A content frame carrying one must paint nothing,
    // even on the stage.
    await open(2, SEEDED_ROLES);
    send({ ...VERSE, stage_message: MESSAGE });
    await settle();
    expect(painted()).toContain('For God so loved the world');
    expect(painted()).not.toContain(MESSAGE);
  });
});
