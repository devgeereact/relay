// AN ALERT THE PREACHER CANNOT MISS — on a stage TEMPLATE, not only on the phone.
//
// ── What was wrong ───────────────────────────────────────────────────────────
//
// `Stage.svelte` paints a Stage Message as the whole screen: `position: fixed;
// inset: 0`, white on `#c8121c`, pulsing to `#7a0a11` on a 1.4s cycle, sized in
// three steps against the backend's 140-character cap. `TemplateRender` — the
// renderer behind `output.html`, which DECISIONS §89 makes the other supported
// route to a stage screen — took the same string and rendered it as ordinary
// text, at whatever size the template's `stage_message` layer happened to be,
// and rendered nothing at all if the template had no such layer.
//
// So of the two ways to put a screen in front of a preacher, one shouted and one
// whispered, and which you got depended on which link the operator copied.
//
// ── The four constraints, one case each ──────────────────────────────────────
//
//   1. it never reaches a congregation screen — gated on ROLE, at the receiver,
//      exactly as `stagemessage.test.js` already holds for the text layer;
//   2. a panic control takes it down (DECISIONS §91);
//   3. reduced motion gets a real answer rather than the pulse's absence;
//   4. `stage_alert` is not retained on the hub and stays that way.
//
// ── What a source assertion is doing in here ─────────────────────────────────
//
// jsdom applies no scoped stylesheet, so `getComputedStyle` reads `static` for an
// element a browser paints `position: fixed; inset: 0`. The same limit
// `stagealertpanic.test.js` records at its own `black` case. The element's
// PRESENCE and its size class are the falsifiable half and are asserted against
// the rendered page; the rules that make it full-bleed are read out of the
// stylesheet, named as read rather than measured, and must be confirmed on a real
// screen by a human.
//
//   npx vitest run src/lib/stagealerttemplate.test.js
// NOTE (2026-09-21, DECISIONS §116): a Stage Message is now a NOTE by default and
// an ALARM only when the operator asks. Every frame below carries `urgent: true`
// because these tests are about the alarm — the full-bleed panel, its sizing and
// what a panic control does to it. The quiet path has its own tests in
// `stagemessagenative.test.js`.

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { ALERT_MAX, ALERT_STEPS, alertStep } from './stagealert.js';

const ROOT = resolve(__dirname, '../..');
const read = (p) => readFileSync(resolve(ROOT, p), 'utf8');

// ─────────────────────────────────────────────────────────────────────────────
// PART 1 · THE SIZING, AND THE THREE FILES IT HAS TO AGREE WITH.
// ─────────────────────────────────────────────────────────────────────────────

describe('the steps and the cap are one figure, in every file that states them', () => {
  it('the module sizes a phrase, a sentence and a paragraph differently', () => {
    expect(alertStep('Wrap up')).toBe('xl');
    expect(alertStep('x'.repeat(40))).toBe('lg');
    expect(alertStep('x'.repeat(120))).toBe('md');
    // Nothing longer than the cap can arrive, so this is a FLOOR and not a
    // fourth step: were the cap ever raised without the steps following it, a
    // long alert would render at `md` and be readable.
    expect(alertStep('x'.repeat(400))).toBe('md');
    expect(alertStep('')).toBe('xl');
    expect(alertStep(null)).toBe('xl');
  });

  it('agrees with the cap the backend will actually deliver', () => {
    // The shape `stagezones.test.js` uses for the same claim one file along:
    // read out of BOTH files in one assertion, so neither can move alone.
    const rust = read('src-tauri/src/main.rs');
    const body = rust.slice(rust.indexOf('fn send_stage_alert'));
    const cap = body.match(/const MAX: usize = (\d+);/);
    expect(cap, 'the cap moved or was renamed').toBeTruthy();
    expect(ALERT_MAX, 'a step above the cap is a rule no message can reach').toBe(
      Number(cap[1]),
    );
  });

  it('and agrees with the phone, which states its own table for its own test', () => {
    // `Stage.svelte` keeps `ALERT_MAX` and `ALERT_STEPS` in its own script because
    // `stagezones.test.js` reads them out of that file's TEXT — moving them would
    // either break that instrument or mean editing it. So the table genuinely
    // lives in two places, and this is the assertion that stops the two drifting:
    // a step changed on one surface and not the other fails here.
    const stage = read('src/Stage.svelte');
    expect(Number(stage.match(/const ALERT_MAX = (\d+);/)[1])).toBe(ALERT_MAX);
    const steps = [...stage.matchAll(/\{ max: ([A-Z_0-9]+), size: '(\w+)' \}/g)].map(
      ([, max, size]) => ({ max: max === 'ALERT_MAX' ? ALERT_MAX : Number(max), size }),
    );
    expect(steps.length, 'the phone’s steps table moved or was reshaped').toBe(
      ALERT_STEPS.length,
    );
    expect(steps).toEqual(ALERT_STEPS);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// PART 2 · THE PRESENTATION, ON THE PAGE THAT ONLY EVER HAD WORDS.
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

const MESSAGE = 'Wrap up — 5 minutes';

/** A stage template that has NO `stage_message` layer — the ordinary case. */
const PLAIN_STAGE_TPL = {
  id: 7,
  name: 'Stage',
  layout: {
    layers: [
      { id: 'bg', type: 'background', fill: '#000', x: 0, y: 0, w: 100, h: 100 },
      { id: 'v', type: 'text', bind: 'verse', x: 5, y: 10, w: 90, h: 60, size: 4, color: '#fff' },
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
const panel = () => host.querySelector('.lalert');

async function outputPage(channel, roles) {
  window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=7`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  send({ kind: 'template', id: 7, template: PLAIN_STAGE_TPL });
  send({ kind: 'channel_roles', roles });
  await settle();
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

describe('every stage template gets the alert, not only the ones that asked for it', () => {
  it('a template with no `stage_message` layer still paints the panel', async () => {
    // The operator's words: *a subtle tint or a small badge is not acceptable*.
    // A template designed before Stage Messages existed is exactly the screen
    // this has to reach, so the panel cannot be a layer the designer opts into.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    expect(panel(), 'a stage TV got the alert as ordinary text, or as nothing').toBeTruthy();
    expect(panel().textContent).toContain(MESSAGE);
    expect(panel().className).toContain(alertStep(MESSAGE));
  });

  it('over a blank screen, because an alert is not a decoration on a slide', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    expect(host.textContent).not.toContain('For God so loved');
    expect(panel()).toBeTruthy();
  });

  it('it is announced assertively, for the one reader who may not be looking', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    expect(panel().getAttribute('aria-live')).toBe('assertive');
  });

  it('and an empty message takes it away again', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    expect(panel()).toBeTruthy();
    send({ kind: 'stage_alert', text: null });
    await settle();
    expect(panel()).toBeNull();
  });
});

describe('CONSTRAINT 1 · it never reaches a congregation screen', () => {
  for (const [channel, what] of [
    [1, 'the main screen'],
    [3, 'a screen with no role at all'],
  ]) {
    it(`no panel on ${what}`, async () => {
      await outputPage(channel, { 1: 'main', 2: 'stage' });
      send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
      await settle();
      expect(panel()).toBeNull();
      expect(host.textContent).not.toContain(MESSAGE);
    });
  }

  it('and a screen that stops being the stage loses it mid-service', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    expect(panel()).toBeTruthy();
    send({ kind: 'channel_roles', roles: { 1: 'main', 2: 'main' } });
    await settle();
    expect(panel(), 'a full-bleed red panel stayed on a congregation screen').toBeNull();
  });
});

describe('CONSTRAINT 2 · a panic control takes it down (DECISIONS §91)', () => {
  for (const kind of ['clear', 'black']) {
    it(`\`${kind}\` removes the panel from the page, not merely from view`, async () => {
      // `black` paints `.blackout` over everything, so a covered panel would pass
      // a `.not.toContain` and prove nothing. The panel is REMOVED on both, which
      // is what makes the two stage surfaces agree — `Stage.svelte` clears its own
      // `alert` on exactly these two kinds.
      await outputPage(2, { 1: 'main', 2: 'stage' });
      send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
      await settle();
      expect(panel()).toBeTruthy();
      send({ kind });
      await settle();
      expect(panel()).toBeNull();
    });
  }

  it('and it does not come back with the next verse (RG-156)', async () => {
    // The defect `stagealertpanic.test.js` left unpinned on purpose, closed by the
    // same line: this page used to HIDE the message with the layer stack and never
    // reset it, so the next fire painted a private word nobody had re-sent.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    send({ kind: 'clear' });
    await settle();
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    await settle();
    expect(host.textContent).toContain('For God so loved');
    expect(host.textContent, 'a cleared Stage Message came back with the next fire').not.toContain(
      MESSAGE,
    );
  });

  it('and the operator can say it again in one action afterwards', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    send({ kind: 'black' });
    await settle();
    expect(panel()).toBeNull();
    send({ kind: 'stage_alert', text: MESSAGE, urgent: true });
    await settle();
    expect(panel()).toBeTruthy();
  });
});

describe('CONSTRAINT 3 · reduced motion gets an answer, not the pulse’s absence', () => {
  // READ, NOT MEASURED — jsdom applies no scoped stylesheet and honours no media
  // query. What is falsifiable here is that the rules EXIST and say what they are
  // claimed to say; whether the result is unmistakable across a platform in bright
  // light is a thing only a human in front of a real screen can report.
  const src = readFileSync(resolve(__dirname, './TemplateRender.svelte'), 'utf8');
  const style = src.slice(src.indexOf('<style>'));

  it('the pulse is behind `no-preference`, exactly as the phone has it', () => {
    expect(style).toMatch(/prefers-reduced-motion: no-preference[\s\S]{0,400}?animation: stagealert/);
    expect(style).toMatch(/@keyframes stagealert/);
  });

  it('and `reduce` gets a rule of its own — which the phone does NOT have', () => {
    // `Stage.svelte` has a `no-preference` branch and no `reduce` counterpart, so
    // under reduced motion its alert is a flat red panel — and its own stylesheet
    // says, two rules up, that a flat red panel is the thing the pulse exists to
    // avoid, because a platform is a bright place and flat red reads as part of
    // the set. This path answers instead of falling silent.
    const reduce = style.slice(style.indexOf('prefers-reduced-motion: reduce'));
    expect(style, 'no reduced-motion branch at all').toContain('prefers-reduced-motion: reduce');
    expect(reduce, 'the reduced-motion answer draws nothing of its own').toMatch(/\.lalert/);
  });

  it('and whatever it does, it is not a brightness pulse by another name', () => {
    // The one thing the constraint rules out. A reduced-motion branch that
    // animated `opacity`, `filter` or a second set of background keyframes would
    // be the pulse wearing a different word.
    const reduce = style.slice(style.indexOf('prefers-reduced-motion: reduce'));
    const block = reduce.slice(0, reduce.indexOf('\n  }\n') + 5);
    expect(block).not.toMatch(/animation|@keyframes|filter:\s*brightness/);
  });

  it('every step the module can answer has a rule that draws it', () => {
    // The other half of `stagezones.test.js`'s pairing, kept here for this path:
    // a size class with no rule renders at whatever the page's base type is.
    for (const s of ALERT_STEPS) {
      expect(style, `no rule for .lalert.${s.size}`).toContain(`.lalert.${s.size}`);
    }
  });
});

describe('CONSTRAINT 4 · the hub does not retain it, and must not start', () => {
  it('a screen that joins an hour later is not flashed a message from before the sermon', () => {
    // Held in Rust, read from here, because the frontend half of this guarantee is
    // worth nothing if the retention rule moves: rule 43 retains `content`,
    // `clear` and `black` and nothing else about what is on a screen.
    const rust = read('src-tauri/src/channels.rs');
    const fn = rust.slice(rust.indexOf('pub fn stage_alert'));
    const body = fn.slice(0, fn.indexOf('\n}\n') + 3);
    expect(body, 'stage_alert now retains something').not.toMatch(/last_screen|retain/);
    expect(rust).toMatch(/Deliberately NOT retained by the hub/);
  });
});
