// THE STAGE PAGE HAS AN IDENTITY, AND A STAGE MESSAGE IS REFUSED WITHOUT ONE.
//
// ── The defect ──────────────────────────────────────────────────────────────
//
// `channels::stage_alert` publishes to EVERY kiosk client, because the hub cannot
// address one: it records nothing about who connected and DECISIONS §35 is not
// being reversed. `Output.svelte` therefore refuses the frame at the RECEIVER,
// against the role its own channel holds (DECISIONS §89). `Stage.svelte` accepted
// it unconditionally and said hello with no identity at all, so every copy of
// `stage.html` open anywhere on the network painted the message — a lobby TV a
// volunteer had pointed at the stage URL, a spare tablet in a back room, a
// visitor's phone. One of two doors had the guarantee, which is the shape
// CLAUDE.md names under "a guarantee is only kept on the doors you checked".
//
// ── What this is NOT ────────────────────────────────────────────────────────
//
// It is not a security boundary and must never be described as one. The LAN is
// trusted by decision (DECISIONS §35, docs/SECURITY.md T4): anyone who can reach
// `:8032` can equally open `stage.html?channel=2` and be handed the message, and
// they can already read the reading, the Stage Note and the programme. What this
// closes is the ACCIDENT — a screen that is not the preacher's showing a word
// addressed to the preacher — and it closes it by the same mechanic, in the same
// place, as the page beside it.
//
// Every test below was watched to fail against the tree before the fix: with no
// channel parsed and no `channel_roles` branch, the alert paints on every one of
// these pages.
//
//   npx vitest run src/lib/stagepageidentity.test.js
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

const MESSAGE = 'Wrap up — 5 minutes';
const VERSE = {
  kind: 'content',
  reference: 'John 3:16',
  text: 'For God so loved the world',
};
/** The roles a fresh install is seeded with (`db::channels::seeded_role`). */
const SEEDED_ROLES = { 1: 'main', 2: 'stage' };

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
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  window.history.replaceState({}, '', '/');
});

/** Mount `stage.html` on `channel` (omit for a URL with no channel at all). */
async function open(channel, roles) {
  window.history.replaceState(
    {},
    '',
    channel == null ? '/stage.html' : `/stage.html?channel=${channel}`,
  );
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  await tick();
  if (roles) send({ kind: 'channel_roles', roles });
  await tick();
  await tick();
}

function send(frame) {
  socket.onmessage({ data: JSON.stringify(frame) });
}

const painted = () => host.textContent;

describe('only a stage screen paints a Stage Message', () => {
  it('paints it on the seeded stage display', async () => {
    // Asserted FIRST, so nothing below can pass by the whole path being broken.
    await open(2, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await tick();
    expect(painted()).toContain(MESSAGE);
  });

  it('paints NOTHING on a screen that holds no role', async () => {
    // A lobby TV or a spare tablet somebody pointed at the stage URL. No role is
    // not a stage — a filter whose default is yes is not a filter.
    await open(4, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await tick();
    expect(painted()).toContain('For God so loved the world');
    expect(painted()).not.toContain(MESSAGE);
  });

  it('paints nothing on the MAIN screen either', async () => {
    // `main` is not a weaker `stage`. A role is a job, not a permission ladder.
    await open(1, SEEDED_ROLES);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await tick();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('paints nothing before the roles have arrived', async () => {
    // A page that has said hello and not been answered yet must refuse. Accepting
    // while the answer is outstanding puts the message on every screen in the
    // building for as long as the reply takes.
    await open(2, null);
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await tick();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('paints nothing on a stage URL that carries no channel at all', async () => {
    // THE DECISION THIS PAGE HAD TO TAKE, and it is a behaviour change: the bare
    // `http://<host>:8032/stage.html` that Outputs used to hand out no longer
    // receives a Stage Message. Refusing is the only answer consistent
    // with the four cases above — an unidentified page is exactly the page that
    // might be anything — and `Output.svelte` already answers a channel-less URL
    // the same way (`?channel=` absent parses to 0, `roleOf` refuses 0).
    await open(null, { ...SEEDED_ROLES, 0: 'stage' });
    send(VERSE);
    send({ kind: 'stage_alert', text: MESSAGE });
    await tick();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('says so, rather than refusing in silence', async () => {
    // Rule 35, on the one screen whose reader cannot glance at the console to
    // find out what happened. A page that silently drops the message reads
    // exactly like a page nobody has sent one to, and the preacher has no way to
    // tell the two apart — so the page states that it has no identity and names
    // the fix. The four identified pages above must NOT carry this line.
    await open(null, SEEDED_ROLES);
    await tick();
    expect(host.querySelector('.noident')).toBeTruthy();
    expect(painted()).toContain('no screen');

    app.$destroy();
    host.remove();
    await open(2, SEEDED_ROLES);
    expect(host.querySelector('.noident')).toBeNull();
  });

  it('loses the message the moment the operator moves the stage role away', async () => {
    // The same guarantee `Output.svelte` gives, for the same reason: an operator
    // who takes the stage role off a tablet has said that tablet is an ordinary
    // screen now, and a refusal that applied only to the NEXT message would leave
    // the last one painted on it for the rest of the service.
    await open(2, SEEDED_ROLES);
    send({ kind: 'stage_alert', text: MESSAGE });
    await tick();
    expect(painted()).toContain(MESSAGE);

    send({ kind: 'channel_roles', roles: { 1: 'main' } });
    await tick();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('tells the hub which channel it is when it says hello', async () => {
    // The identity has to be on the wire as well as in the URL: rule 43's replay
    // is answered inside the hub's `hello` handler, and a per-screen state the
    // hub retains for one channel can only be replayed to a client that has said
    // which channel it is.
    await open(2, SEEDED_ROLES);
    const hello = socket.sent.map((s) => JSON.parse(s)).find((m) => m.kind === 'hello');
    expect(hello, 'the stage page stopped saying hello — rule 43 depends on it').toBeTruthy();
    expect(hello.channel).toBe(2);
  });
});
