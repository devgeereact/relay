// THE STAGE TIMER ON THE OTHER STAGE SURFACE — `output.html`, gated on ROLE.
//
// ── The trap this closes ─────────────────────────────────────────────────────
//
// There are two ways to put a screen in front of a preacher and only one of them
// showed the programme:
//
//   stage.html?channel=N   built by `channelroles.js::stageRemoteUrl`,
//                          surfaced as Outputs → Sharing.        Rail: yes.
//   output.html?channel=N  built by `outputurl.js`, surfaced as
//                          Screens → Copy URL.                   Rail: never.
//
// `Output.svelte` had no `timer` branch at all and `TemplateRender` had no bind
// for one, so a church that wired its confidence monitor from the link the
// Screens inspector hands out watched the figure count down on the console and
// sent it to nobody. `stagetimerreach.test.js` names that trap in prose and
// still does: the stage address remains the route that needs no template at all.
//
// ── WHY THIS IS NOT SIMPLY FLIPPING `timer: false` ───────────────────────────
//
// `r6-contracts.test.js`'s verdict table recorded the refusal with a reason that
// is still true, word for word: *"Sermon · 4:12 left" behind a preacher is the
// running order in front of the whole building.* That sentence is about a
// CONGREGATION screen. A screen holding `role === 'stage'` is not one, which is
// why the gate is the role and the safety argument survives whole — and why the
// case that matters most in this file is the one that asserts a `main` screen
// wearing the very same stage template still shows nothing.
//
//   npx vitest run src/lib/progtimertemplate.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { codeOnly } from './codeonly.js';
import { resolve } from 'node:path';
import {
  programmeRows,
  programmeCh,
  programmeCapacity,
  programmeCells,
  MIN_TIMER_PX,
} from './timers.js';

// ─────────────────────────────────────────────────────────────────────────────
// PART 1 · THE PURE RULE, which is the thing both surfaces now read.
// ─────────────────────────────────────────────────────────────────────────────

const NOW = 1_700_000_000_000;
/** A timer row in the shape `channels::timer_frame_json` puts on the wire. */
const wire = (over = {}) => ({
  id: 1,
  label: 'Sermon',
  countdown_to: NOW + 240_000,
  countdown_from: NOW - 60_000,
  countdown_paused_ms: null,
  countdown_done: null,
  warn_ms: null,
  ...over,
});

describe('the programme set is derived in ONE place', () => {
  it('a running timer becomes a labelled figure', () => {
    const [r] = programmeRows([wire()], NOW);
    expect(r.label).toBe('Sermon');
    expect(r.v).toBe('4:00');
    expect(r.held).toBe(false);
    expect(r.warn).toBe(false);
  });

  it('a timer past zero counts UP, with the sign, and is warned', () => {
    // RG-153: the rail answers the only question a preacher asks it past zero,
    // which is how far over. The `+` is part of the value, which is why the
    // column budget below is taken from the rendered string.
    const [r] = programmeRows([wire({ countdown_to: NOW - 277_000 })], NOW);
    expect(r.v).toBe('+4:37');
    expect(r.warn).toBe(true);
  });

  it('a HELD timer is frozen, is said to be held, and is never warned', () => {
    // RG-175: the stored figure is SIGNED, so a sermon held two minutes over
    // holds at -120000 and reads `+2:00`. A frozen figure pulsing red says the
    // opposite of what is true.
    const [r] = programmeRows([wire({ countdown_paused_ms: -120_000 })], NOW);
    expect(r.held).toBe(true);
    expect(r.v).toBe('+2:00');
    expect(r.warn).toBe(false);
  });

  it('a timer that names no deadline is not a row at all', () => {
    expect(programmeRows([wire({ countdown_to: null })], NOW)).toEqual([]);
  });

  it('anything that is not a list answers empty rather than throwing', () => {
    // The same discipline `stageTimers` already keeps one function up: a reactive
    // block that throws takes the surface down with it, and one of the two
    // surfaces this now feeds is the preacher's.
    expect(programmeRows(null, NOW)).toEqual([]);
    expect(programmeRows(undefined, NOW)).toEqual([]);
    expect(programmeCells(null, 4)).toEqual([]);
    expect(programmeCh(null)).toBe(4);
  });

  it('the column budget is the widest RENDERED string, floored at four', () => {
    const rows = programmeRows(
      [wire(), wire({ id: 2, countdown_to: NOW + 5_400_000 })],
      NOW,
    );
    expect(rows[1].v).toBe('1:30:00');
    expect(programmeCh(rows)).toBe(7);
    expect(programmeCh([{ v: '0:09' }])).toBe(4);
  });

  it('the overflow cell is the LAST slot, and one clock always survives', () => {
    const rows = [1, 2, 3, 4, 5, 6].map((id) => ({ id, v: '4:00' }));
    expect(programmeCells(rows, 6)).toHaveLength(6);
    const squeezed = programmeCells(rows, 3);
    expect(squeezed).toHaveLength(3);
    expect(squeezed[2]).toEqual({ more: 4 });
    // A rail that says "6 more" and shows nothing has told the preacher he
    // cannot have the thing he is looking at.
    expect(programmeCells(rows, 1)).toEqual([rows[0], { more: 5 }]);
  });

  it('capacity is arithmetic on a WIDTH, and an unmeasured box is not a width of zero', () => {
    expect(MIN_TIMER_PX).toBe(132);
    expect(programmeCapacity(1920)).toBe(14);
    expect(programmeCapacity(1280)).toBe(9);
    expect(programmeCapacity(400)).toBe(3);
    // Zero is what a box reports before it has been laid out, and answering
    // "one cell" to that would collapse the rail on its first frame.
    expect(programmeCapacity(0)).toBe(programmeCapacity(1024));
    expect(programmeCapacity(NaN)).toBe(programmeCapacity(1024));
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// PART 2 · AND THE SAME RULE, RENDERED, ON THE SURFACE THAT COULD NOT SHOW IT.
// ─────────────────────────────────────────────────────────────────────────────

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

/** A stage-shaped template: a verse, and the programme rail beneath it. */
const STAGE_TPL = {
  id: 7,
  name: 'Stage',
  layout: {
    layers: [
      { id: 'bg', type: 'background', fill: '#000', x: 0, y: 0, w: 100, h: 100 },
      { id: 'v', type: 'text', bind: 'verse', x: 5, y: 10, w: 90, h: 40, size: 4, color: '#fff' },
      { id: 'p', type: 'text', bind: 'programme', x: 0, y: 80, w: 100, h: 18, size: 3, color: '#fff' },
    ],
  },
  style: { background: '#000' },
};

let host;
let app;

async function settle() {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
}
const send = (frame) => socket.onmessage({ data: JSON.stringify(frame) });
const rail = () => host.querySelector('.lprog');

/** `output.html` on `channel`, told what every screen is for. */
async function outputPage(channel, roles) {
  window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=7`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  send({ kind: 'template', id: 7, template: STAGE_TPL });
  send({ kind: 'channel_roles', roles });
  await settle();
}

/** The frame the hub already sends every client, `output.html` included. */
function timerFrame(timers) {
  return { kind: 'timer', timers, warn_default_ms: 60_000 };
}

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

describe('a stage-role screen served by output.html shows the programme', () => {
  it('paints the rail from the frame it was already being sent', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(rail(), 'output.html still ignores the timer frame').toBeTruthy();
    expect(rail().textContent).toContain('Sermon');
    expect(rail().textContent).toMatch(/\d:\d\d/);
  });

  it('before anything has been fired, which is when a Stage Timer is started', async () => {
    // The rail is NOT inside the content gate, deliberately, and this is the case
    // that makes the difference visible: a Stage Timer runs during the notices,
    // before the first verse of the service exists.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ label: 'Offering', countdown_to: Date.now() + 90_000 })]));
    await settle();
    expect(rail()).toBeTruthy();
    expect(rail().textContent).toContain('Offering');
  });

  it('and a panic control stops none of the clocks (DECISIONS §91)', async () => {
    // The two stage surfaces must agree. `Stage.svelte` renders its rail outside
    // the `visible` gate and says at the line why: a panic control takes back
    // every sentence anybody put on a screen, and stops none of the clocks.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(rail()).toBeTruthy();
    send({ kind: 'clear' });
    await settle();
    expect(host.textContent, 'the reading survived a clear').not.toContain('For God so loved');
    expect(rail(), 'the preacher lost his programme to a control aimed at the wall').toBeTruthy();
  });

  it('the whole set arrives every time, so stopping the last one empties the rail', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(rail()).toBeTruthy();
    send(timerFrame([]));
    await settle();
    expect(rail(), 'an empty set is how a clock comes OFF a screen').toBeNull();
  });
});

describe('THE CONGREGATION REFUSAL, which matters more than the feature', () => {
  it('a `main` screen wearing the very same stage template shows no rail', async () => {
    await outputPage(1, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(rail(), 'the running order is in front of the whole building').toBeNull();
    expect(host.textContent).not.toContain('Sermon');
  });

  it('and so does a screen with NO role — a lobby TV is not a default yes', async () => {
    await outputPage(3, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(rail()).toBeNull();
  });

  it('a screen that STOPS being the stage loses the rail at once', async () => {
    // The same requirement `applyRoles` keeps for a Stage Message: an operator who
    // moves the role off a tablet has said that tablet is a congregation screen
    // now, and a refusal that only applied to the NEXT frame would leave the
    // programme painted on it for the rest of the service.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(rail()).toBeTruthy();
    send({ kind: 'channel_roles', roles: { 1: 'main', 2: 'main' } });
    await settle();
    expect(rail()).toBeNull();
  });
});

describe('the overflow rule comes with it — and it measures the BOX', () => {
  it('collapses to a "+N more" cell rather than shrinking every clock to nothing', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    const many = Array.from({ length: 9 }, (_, i) =>
      wire({ id: i + 1, label: `T${i + 1}`, countdown_to: Date.now() + 240_000 }),
    );
    send(timerFrame(many));
    await settle();
    // An unmeasured box falls back to 1024px → seven cells → six clocks + the count.
    expect(rail().textContent).toContain('+3 more');
    expect(rail().querySelectorAll('[data-timer-id]')).toHaveLength(6);
  });

  it('a NARROW box shows fewer, which a window measurement could never tell you', async () => {
    // THE POINT OF THIS CASE. `Stage.svelte` measures `innerWidth` because its rail
    // spans the frame. A template renders inside a REGION, and a region is not the
    // viewport: the same rail in a 300px box on a 1920px screen may show two cells.
    // Measured here by giving the real element a width and letting the component
    // re-measure when the component updates.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    const many = Array.from({ length: 9 }, (_, i) =>
      wire({ id: i + 1, label: `T${i + 1}`, countdown_to: Date.now() + 240_000 }),
    );
    send(timerFrame(many));
    await settle();
    Object.defineProperty(rail(), 'clientWidth', { value: 300, configurable: true });
    // Any update re-measures; a fresh set is the honest way to provoke one.
    send(timerFrame(many.slice(0, 8)));
    await settle();
    expect(rail().querySelectorAll('[data-timer-id]')).toHaveLength(1);
    expect(rail().textContent).toContain('+7 more');
  });

  it('and the renderer never asks the WINDOW how wide the rail is', () => {
    // The source claim behind the case above, because a component that fell back
    // to `innerWidth` would pass every assertion in this file at 1024px.
    //
    // COMMENTS STRIPPED, for the reason `colourlaw.test.js` records at the same
    // shape: `TemplateRender.svelte` mentions `innerWidth` once, in prose saying
    // why it does NOT use it. A scanner that counted that would fail on a correct
    // file, and the cheapest way to make it green would be to delete the
    // explanation.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    // Guards the guard: the prose IS there, so a stripper that silently blanked
    // the whole file would make this assertion pass vacuously.
    expect(src, 'the premise of this case').toMatch(/innerWidth/);
    expect(codeOnly(src)).not.toMatch(/innerWidth/);
  });
});

// ── A CLOCK OVER A PICTURE HAS TO BE READABLE ON THE PICTURE (RG-212) ────────
//
// The operator's words: *"when Media is on stage screen can the timer
// automatically have its own background that will make it visible to the
// viewers"*. A programme rail is white digits with no backing of its own,
// which is exactly right over a template's own dark background and unreadable
// the moment a clip is painting behind it — and a clip's brightness changes
// frame by frame, so this is not a template a designer can pre-solve.
//
// AUTOMATICALLY is the load-bearing word. The backing appears while a picture
// is behind the rail and goes away when it stops, because a permanent plate
// would be a box sitting over every template that has no picture.
describe('the programme rail earns a backing while a picture is behind it', () => {
  it('has none over a template with no picture', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(rail().className, 'a plate over a template that needs none').not.toMatch(/overmedia/);
  });

  it('gains one the moment a clip is fired, and loses it when the clip goes', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    send({
      kind: 'content',
      content_kind: 'media',
      media_url: 'http://10.0.0.5:8032/media/4',
      media_kind: 'video',
    });
    await settle();
    expect(rail(), 'the rail went with the clip').toBeTruthy();
    expect(rail().className, 'white digits on an unknown picture').toMatch(/overmedia/);

    send({ kind: 'clear' });
    await settle();
    expect(rail().className, 'the plate outlived the picture').not.toMatch(/overmedia/);
  });

  it('a still picture counts too — a bright slide hides a clock as well as a clip does', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    send({
      kind: 'content',
      content_kind: 'media',
      media_url: 'http://10.0.0.5:8032/media/9',
      media_kind: 'image',
    });
    await settle();
    expect(rail().className).toMatch(/overmedia/);
  });

  it('and the plate is a real rule in the stylesheet, not a class nothing paints', async () => {
    // This file has been caught by a `class:` directive naming a class no
    // stylesheet defines, which is a test asserting a spelling.
    const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
    expect(codeOnly(src)).toMatch(/\.lprog\.overmedia\s*\{/);
  });
});

// ── A CLOCK REACHES THE PREACHER WHATEVER TEMPLATE THE SCREEN WEARS (RG-224) ─
//
// The operator, running the packaged build: *"Timer should be able to show
// regardless of whats on the stage screen"*, and *"My stage display not showing
// what i have on the templete"*.
//
// What they were looking at was `output.html` on a stage-role screen wearing a
// SCRIPTURE template — Classic Serif, which has no programme layer, because a
// congregation template has no business carrying the running order. The rail is
// already outside the content gate, so a panic control cannot take it; but with
// no layer to place it in, there was nothing to place, and the clocks arrived
// and painted nowhere.
//
// **The precedent is in this same component and is exactly this shape.**
// `showDefaultCountdown`: no timer layer, but a countdown is on screen, so a
// default is drawn and the comment says *"Add a Timer layer to the template to
// place it instead"*. A designed layer is better and is not required for the
// thing to work at all.
//
// The gate is the ROLE, not the template, and that is the half worth testing:
// *"Sermon · 4:12 left"* behind a preacher is the running order in front of the
// whole building, so a `main` screen with no programme layer must still show
// nothing.
describe('a stage screen with no programme layer still gets its clocks', () => {
  /** A template with NO programme layer — a congregation look, on a stage screen. */
  const PLAIN_TPL = {
    id: 9,
    name: 'Classic Serif',
    layout: {
      layers: [
        { id: 'bg', type: 'background', fill: '#101820' },
        { id: 'v', type: 'text', bind: 'verse', x: 6, y: 20, w: 88, h: 50, size: 5 },
      ],
    },
    style: {},
  };

  async function plainStage(channel, roles) {
    window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=9`);
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Output({ target: host });
    await settle();
    send({ kind: 'template', id: 9, template: PLAIN_TPL });
    send({ kind: 'channel_roles', roles });
    await settle();
  }

  it('paints a default rail, and says the clocks it was given', async () => {
    await plainStage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ label: 'Sermon', countdown_to: Date.now() + 240_000 })]));
    await settle();
    const rail = host.querySelector('.lprog');
    expect(rail, 'the clocks reached a stage screen and painted nowhere').toBeTruthy();
    expect(rail.textContent).toContain('Sermon');
    expect(rail.textContent).toMatch(/\d:\d\d/);
  });

  it('and a designed layer still wins — the default is a fallback, not a second rail', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(host.querySelectorAll('.lprog'), 'two rails on one screen').toHaveLength(1);
  });

  it('THE REFUSAL: a congregation screen gets no default rail either', async () => {
    // The whole safety argument, and the case that matters most in this file.
    // A default that ignored the role would put the running order in front of
    // the building on every screen wearing an ordinary template — which is every
    // screen a church owns.
    await plainStage(1, { 1: 'main', 2: 'stage' });
    send(timerFrame([wire({ countdown_to: Date.now() + 240_000 })]));
    await settle();
    expect(host.querySelector('.lprog')).toBeNull();
    expect(host.textContent).not.toContain('Sermon');
  });

  it('and nothing is drawn when no clock is running', async () => {
    await plainStage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([]));
    await settle();
    expect(host.querySelector('.lprog'), 'an empty rail on a screen with no clocks').toBeNull();
  });
});
