// PER-SCREEN CLEAR AND BLACKOUT, AT THE SCREEN.
//
// `clear_screens` and `blackout` take no channel argument and never will: a panic
// control that has to work out which screen it is addressing is a panic control
// that can fail to answer (rule 15, DECISIONS §20). So the split is in the CALL —
// three separate commands, reached from the Outputs desk, that publish a
// `screen_state` frame naming the whole set of screens the operator has taken out
// of the wall.
//
// The refusal is at the RECEIVER, for the third time in this codebase and for the
// same reason each time: the hub broadcasts to everybody and records nothing about
// who connected (DECISIONS §35), so the only party that knows which screen it is
// is the screen. `channel_template` does this, `stage_alert` does this, and this
// does this.
//
// What these hold, on both pages that render a screen:
//
//   1. a screen named in the set goes down and NO other screen does;
//   2. it STAYS down across a fire — a one-shot would be undone within a minute
//      of being used, which is to say useless for the thing it is for;
//   3. the way back is the set arriving without it, so a page cannot be left
//      down by a frame it missed;
//   4. blackout on one screen is opaque, exactly as the whole-wall one is.
//
//   npx vitest run src/lib/perscreen.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

vi.mock('@tauri-apps/api/core', () => {
  throw new Error('no tauri bridge in a browser source');
});
vi.mock('@tauri-apps/api/event', () => {
  throw new Error('no tauri bridge in a browser source');
});

const Output = (await import('../Output.svelte')).default;
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

const TPL = {
  id: 7,
  name: 'Classic',
  layout: {
    layers: [
      { id: 'bg', type: 'background', fill: '#000', x: 0, y: 0, w: 100, h: 100 },
      { id: 'v', type: 'text', bind: 'verse', x: 5, y: 10, w: 90, h: 60, size: 4, color: '#fff' },
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
const WORDS = 'For God so loved the world';

let host;
let app;

async function settle() {
  for (let i = 0; i < 30; i += 1) await Promise.resolve();
  await tick();
  for (let i = 0; i < 30; i += 1) await Promise.resolve();
  await tick();
}

function send(frame) {
  socket.onmessage({ data: JSON.stringify(frame) });
}

/** `output.html` on `channel`, with its template delivered. */
async function openOutput(channel) {
  window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=7`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  socket.onopen?.();
  send({ kind: 'template', id: 7, template: TPL });
  await settle();
}

/** `stage.html` on `channel`. */
async function openStage(channel) {
  window.history.replaceState({}, '', `/stage.html?channel=${channel}`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  send({ kind: 'channel_roles', roles: { 1: 'main', 2: 'stage' } });
  await tick();
}

const painted = () => host.textContent;
const blackedOut = () => !!host.querySelector('.blackout');

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

describe('output.html — one screen goes down and the others do not', () => {
  it('blanks the screen the set names', async () => {
    await openOutput(4);
    send(VERSE);
    await settle();
    expect(painted(), 'the fixture never got the verse up').toContain(WORDS);

    send({ kind: 'screen_state', screens: { 4: 'clear' } });
    await settle();
    expect(painted()).not.toContain(WORDS);
  });

  it('leaves every screen the set does NOT name exactly as it was', async () => {
    // The assertion that makes the first one mean anything. A frame that blanked
    // whatever received it would pass the test above and take the congregation's
    // wall down with the lobby TV.
    await openOutput(1);
    send(VERSE);
    send({ kind: 'screen_state', screens: { 4: 'clear' } });
    await settle();
    expect(painted()).toContain(WORDS);
  });

  it('STAYS down when the next verse fires', async () => {
    // The durability that makes the control worth having. A one-shot would be
    // undone by the next fire, which during a service is within a minute.
    await openOutput(4);
    send({ kind: 'screen_state', screens: { 4: 'clear' } });
    send(VERSE);
    await settle();
    expect(painted()).not.toContain(WORDS);
  });

  it('comes back up when the set arrives without it, showing what is on the wall', async () => {
    // The whole set every time, never a delta — so a page that missed one frame
    // cannot be wrong about itself for the rest of the service. Coming back means
    // showing what the WALL is showing, which is why the page keeps tracking
    // content while it is down rather than throwing it away.
    await openOutput(4);
    send({ kind: 'screen_state', screens: { 4: 'clear' } });
    send(VERSE);
    await settle();
    expect(painted()).not.toContain(WORDS);

    send({ kind: 'screen_state', screens: {} });
    await settle();
    expect(painted()).toContain(WORDS);
  });

  it('blacks out opaquely on one screen, exactly as the whole wall does', async () => {
    await openOutput(4);
    send(VERSE);
    send({ kind: 'screen_state', screens: { 4: 'black' } });
    await settle();
    expect(blackedOut(), 'a per-screen blackout painted no opaque layer').toBe(true);
  });

  it('a raw preview on no channel is never named by any set', async () => {
    // `?channel=` absent parses to 0. A preview that belongs to no screen must not
    // be taken down by a decision about a screen.
    window.history.replaceState({}, '', '/output.html?template_id=7');
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    send({ kind: 'template', id: 7, template: TPL });
    send(VERSE);
    send({ kind: 'screen_state', screens: { 0: 'clear', 4: 'clear' } });
    await settle();
    expect(painted()).toContain(WORDS);
  });

  it('names its channel when it says hello', async () => {
    // The hub replays the retained set inside its `hello` handler, and a state
    // retained for one screen can only be replayed to a client that has said which
    // screen it is.
    await openOutput(4);
    const hello = socket.sent.map((s) => JSON.parse(s)).find((m) => m.kind === 'hello');
    expect(hello).toBeTruthy();
    expect(hello.channel).toBe(4);
  });
});

describe('stage.html honours it too — both doors, not one', () => {
  it('blanks the preacher’s screen when the set names it', async () => {
    // `Stage.svelte` is the door this repository has now missed four times, most
    // recently for `black` (DECISIONS §91) — the operator hit the key, the wall
    // went dark, and the screen the preacher reads from kept the verse while the
    // console correctly reported success.
    await openStage(2);
    send({ kind: 'content', reference: 'John 3:16', text: WORDS });
    await tick();
    expect(painted(), 'the fixture never got the verse up').toContain(WORDS);

    send({ kind: 'screen_state', screens: { 2: 'clear' } });
    await tick();
    expect(painted()).not.toContain(WORDS);
  });

  it('leaves a stage screen the set does not name alone', async () => {
    await openStage(2);
    send({ kind: 'content', reference: 'John 3:16', text: WORDS });
    send({ kind: 'screen_state', screens: { 4: 'black' } });
    await tick();
    expect(painted()).toContain(WORDS);
  });
});

// ── AND A CUE THAT NAMES A SCREEN (RG-161) ─────────────────────────────────
//
// The same mechanic one field along, and the fourth time in this codebase: the
// hub broadcasts to everybody and records nothing about who connected
// (DECISIONS §35), so the only party that knows which screen it is, is the
// screen. `channel_template` does this, `stage_alert` does this,
// `screen_state` above does this, and now content does.
//
// What a plan that names a screen is FOR: stopping a notice landing on the
// preacher's tablet, a lobby TV showing a countdown after the service has
// started, and a band channel taking words it is not meant to carry.
//
// **A screen a cue does not name is UNTOUCHED, not cleared.** Targeting means
// "these screens change and the others carry on" — the alternative gives a plan
// cue the reach of a panic control, and a cue built on a Tuesday with one
// screen ticked would blank every other screen on the Sunday.
describe('a cue that names its screens', () => {
  const NOTICE = {
    ...VERSE,
    reference: 'Tea afterwards',
    text: 'In the hall',
    channels: [4],
  };

  it('paints on the screen it names', async () => {
    await openOutput(4);
    send(NOTICE);
    await settle();
    expect(painted()).toContain('In the hall');
  });

  it('does not paint on a screen it does not name', async () => {
    await openOutput(1);
    send(NOTICE);
    await settle();
    expect(painted()).not.toContain('In the hall');
  });

  it('leaves an unnamed screen showing what it already had, rather than clearing it', async () => {
    // THE DECISION, and the reason it is not "untargeted screens go blank".
    await openOutput(1);
    send(VERSE);
    await settle();
    const before = painted();
    expect(before).toContain('For God so loved');
    send(NOTICE);
    await settle();
    expect(painted()).toContain('For God so loved');
    expect(painted()).not.toContain('In the hall');
  });

  it('reaches every screen when the cue names none', async () => {
    // Every cue built before targeting existed says nothing about screens.
    await openOutput(1);
    send(VERSE);
    await settle();
    expect(painted()).toContain('For God so loved');
  });

  it('reaches no screen when the cue names an empty set', async () => {
    // Not "all of them". A cue that reaches nothing is a real thing to ask for.
    await openOutput(1);
    send({ ...VERSE, channels: [] });
    await settle();
    expect(painted()).not.toContain('For God so loved');
  });

  it('paints on a raw preview, which belongs to no screen', async () => {
    // Channel 0 is a template preview. A preview that silently dropped a
    // targeted cue would be a preview that lies about the plan.
    await openOutput(0);
    send(NOTICE);
    await settle();
    expect(painted()).toContain('In the hall');
  });

  // ── AND THE SECOND DOOR ─────────────────────────────────────────────────
  //
  // The rehearsal guarantee was green and false for the stage tablet for
  // exactly this reason: a rule kept on `output.html` and skipped on its twin.
  it('stage.html paints a cue that names it', async () => {
    await openStage(2);
    send({ ...VERSE, channels: [2] });
    await tick();
    expect(painted()).toContain('For God so loved');
  });

  it('stage.html ignores a cue aimed at the wall', async () => {
    await openStage(2);
    send({ ...VERSE, channels: [1] });
    await tick();
    expect(painted()).not.toContain('For God so loved');
  });

  it('stage.html keeps the reading when a later cue is aimed elsewhere', async () => {
    // The preacher is halfway through a reading; a notice goes to the foyer.
    await openStage(2);
    send(VERSE);
    await tick();
    expect(painted()).toContain('For God so loved');
    send({ ...VERSE, reference: 'Tea', text: 'In the hall', channels: [4] });
    await tick();
    expect(painted()).toContain('For God so loved');
    expect(painted()).not.toContain('In the hall');
  });
});
