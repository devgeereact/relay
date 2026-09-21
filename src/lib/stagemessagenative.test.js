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

// ── TWO VERBS, ONE FIELD (operator, 2026-09-21; DECISIONS §116) ────────────
//
// "When a message is written in the stage message section it should show on the
// stage display fixed text only; if the operator sends to stage or alert then it
// fills the screen with a flashing warning red line."
//
// There was ONE rendering: the full-bleed flashing panel. So "wrap up in five"
// arrived as the same emergency as "stop, there is a medical incident", and an
// alarm spent on ordinary business stops being an alarm. Nothing in the product
// could put a quiet word on a preacher's screen.
//
// THE FALLBACK IS THE PART THAT NEEDED CARE, and it was found by reading the
// operator's own template rather than by reasoning. `Stage · Reading` has five
// layers — Background, Reading, Reference, Clock, Elapsed — and NO
// `stage_message` layer. A quiet send that only ever painted into that layer
// would have been swallowed on the one screen this was asked for, which is the
// silence RG-156 was about, arriving by a different door. So a quiet message
// with nowhere declared to go gets a modest fixed strip instead of nothing.
describe('a quiet word and an alarm are different things — DECISIONS §116', () => {
  const RENDER = readFileSync(resolve(process.cwd(), 'src/lib/TemplateRender.svelte'), 'utf8');

  it('the renderer takes urgency as its own fact, not as the presence of words', () => {
    expect(RENDER, 'the renderer cannot tell a note from an alarm').toMatch(
      /export let stageUrgent/,
    );
  });

  it('the flashing panel is gated on urgency', () => {
    // It was `{#if stageAlert}` — any words at all. The panel is the thing that
    // must be rare, so it is the thing that must be asked for.
    expect(RENDER).toMatch(/\{#if stageAlert && stageUrgent\}/);
  });

  it('a quiet message with no layer to land in still lands, modestly', () => {
    // NEVER SWALLOWED. The operator's own stage template declares no
    // `stage_message` layer, so "renders into the layer, and otherwise nowhere"
    // would be a send that reports success and shows nothing.
    expect(RENDER, 'no quiet strip at all').toMatch(/class="lmsg"/);
    expect(RENDER, 'the strip is not conditional on the template lacking a place').toMatch(
      /\{#if stageAlert && !stageUrgent && !hasMessageLayer\}/,
    );
  });

  it('…and it does not flash, which is the whole distinction', () => {
    const strip = /\.lmsg\s*\{[\s\S]*?\}/.exec(RENDER);
    expect(strip, 'the strip has no styling').toBeTruthy();
    expect(strip[0], 'the quiet strip animates — then it is a second alarm').not.toMatch(
      /animation:/,
    );
  });

  it('the page carries urgency from both doors', () => {
    expect(PAGE, 'the socket door drops urgency').toMatch(/m\.urgent/);
    expect(PAGE, 'the Tauri door drops urgency').toMatch(/payload\?\.urgent/);
  });
});

