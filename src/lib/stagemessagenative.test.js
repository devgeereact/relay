// THE STAGE MESSAGE ON THE OTHER DOOR — a native output window (RG-156).
//
// `stagemessage.test.js` beside this file drives the KIOSK door: a browser source
// with no Tauri bridge, reading frames off the socket. Every guarantee it holds
// was true and none of it reached the second door, because there was no second
// door: `channels::stage_alert` called `publish_kiosk` and nothing else, so a
// screen wired as a `native_window` and given the `stage` role received nothing
// at all.
//
// THE FAILURE DIRECTION IS SILENCE, which is the worse of the two. The console
// reports a Stage Message sent, the operator believes the preacher has been told,
// and one whole class of stage screen never heard it. That is rule 35 seen from
// the engine end rather than the badge end — a control that cannot detect its own
// failure, where the thing that cannot detect it is the sender.
//
// WHY THE ROLE CHECK IS THE SAME CHECK AND NOT A SECOND ONE. The kiosk hub cannot
// address one client (DECISIONS §35, not being reversed), so the filter has always
// lived on the receiving page. A Tauri emit reaches every webview, so it needs the
// identical filter — and `acceptsStageMessage(myRole)` is that filter, asked at
// both doors from one function. Two expressions would be two rules, and this
// repository's most-repeated bug is a guarantee kept on one of two doors.
//
// Watched to fail: without the `output://stage_alert` listener the first test
// paints nothing; with the listener but no role check the second and third paint
// the message on a congregation screen.
//
//   npx vitest run src/lib/stagemessagenative.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const invoke = vi.fn();
const handlers = new Map();
const listen = vi.fn(async (name, fn) => {
  handlers.set(name, fn);
  return () => {};
});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const Output = (await import('../Output.svelte')).default;
const PAGE = readFileSync(resolve(process.cwd(), 'src/Output.svelte'), 'utf8');

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

const MESSAGE = 'Wrap up — 5 minutes';
/** A fresh install: one main screen, one stage, and congregation screens with no role. */
const SEEDED_ROLES = { 1: 'main', 2: 'stage' };

let host;
let app;

async function settle() {
  for (let round = 0; round < 3; round += 1) {
    for (let i = 0; i < 60; i += 1) await Promise.resolve();
    await tick();
  }
}

/** Mount `output.html` as a NATIVE window on `channel`, with the roles known. */
async function open(channel) {
  window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=7`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  fire('output://channel_roles', { roles: SEEDED_ROLES });
  await settle();
}

const fire = (name, payload) => handlers.get(name)?.({ payload });
const painted = () => host.textContent;

beforeEach(() => {
  invoke.mockReset();
  listen.mockClear();
  handlers.clear();
  invoke.mockImplementation((cmd, args) => {
    if (cmd === 'get_template' && args?.id === 7) return Promise.resolve(STAGE_TPL);
    if (cmd === 'list_output_channels')
      return Promise.resolve([{ id: 1, name: 'Main screen' }, { id: 2, name: 'Stage display' }]);
    if (cmd === 'get_content_templates')
      return Promise.resolve({ scripture: null, song: null, media: null, announce: null, countdown: null });
    if (cmd === 'list_channel_looks') return Promise.resolve({});
    if (cmd === 'channel_roles') return Promise.resolve(SEEDED_ROLES);
    return Promise.resolve(null);
  });
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

describe('a native stage window is reached by a Stage Message — RG-156', () => {
  it('the page listens on the Tauri door at all', async () => {
    await open(2);
    expect(
      [...handlers.keys()],
      'no output://stage_alert listener — a native stage screen hears nothing',
    ).toContain('output://stage_alert');
  });

  it('paints it on a native window whose role is stage', async () => {
    await open(2);
    fire('output://stage_alert', { text: MESSAGE });
    await settle();
    expect(painted()).toContain(MESSAGE);
  });

  it('paints NOTHING on a native window that is the main screen', async () => {
    // The congregation case, and the one the whole guarantee is about. `main` is
    // a real role and it is not `stage`; a filter that answered yes here would put
    // a word meant for one person on the wall in front of everybody.
    await open(1);
    fire('output://stage_alert', { text: MESSAGE });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('a blank message takes it down again rather than leaving it on the glass', async () => {
    await open(2);
    fire('output://stage_alert', { text: MESSAGE });
    await settle();
    expect(painted()).toContain(MESSAGE);
    fire('output://stage_alert', { text: null });
    await settle();
    expect(painted()).not.toContain(MESSAGE);
  });

  it('and BOTH doors ask the one predicate, rather than two expressions of it', () => {
    // The rule, not the instance. A second door that re-derived "is this a stage"
    // is two rules about one fact, and the second one is the one that drifts.
    // FOUR CALL SITES, exactly, and the regex requires the `(` so the import line
    // is not one of them: the programme-rail gate (§104), the reset when a screen
    // stops being a stage, the socket door, and the Tauri door added here.
    //
    // It is `toBe` rather than a range on purpose. This assertion was first written
    // as `>= 3` and PASSED against the broken tree — three call sites existed and
    // the missing door was the whole defect — which is a guard on a guarantee that
    // is really a guard on nothing. At four it fails before the fix and passes
    // after it, which is the only shape that means anything.
    const calls = PAGE.match(/acceptsStageMessage\(/g) ?? [];
    expect(
      calls.length,
      'a door stopped asking, or a third one appeared without the check',
    ).toBe(4);
  });
});
