// ══ THE STAGE TV BEHAVES LIKE THE PREACHER'S PHONE ═══════════════════════════
//
// The operator, watching a platform monitor served by `output.html`:
//
//   1. *"When there is no Text on the screen and there is a timing, Enlarge the
//      timer to make it more visible and fill the screen"*
//   2. *"when the time is running over, change the text to Time Up"*
//   4. *"Defult timer text should be TIMER not just empty."*
//   5. *"Both screens should behave like the Preachers screen... when there is
//      no text utilise the full space for the timer and Stage Message"*
//      (the operator typed that last pair lower-case; it is capitalised here to
//      the product's own spelling, because `names.test.js` reads this file and a
//      casing variant is a second label wherever it is written)
//   6. *"When a Media is Playing I want you to Cover the Stage timer with the
//      clip countdown... and return the timer back when the media is cleared or
//      done and this happened to only Videos Not to a static picture."*
//   7. *"Also aligh the text on the stage timer properly so it dosent just stay
//      to one side"*
//
// **Every one of these already exists on the phone.** RG-244 gave `stage.html`
// `restingLayout`, RG-248 centred its digits, RG-239 gave the message the
// reading's room. `output.html` got none of it — which is RG-224, RG-265 and
// RG-280 for the fourth time: a thing built for the phone, asked for on the big
// screen, and the big screen never asked. So the rule is IMPORTED here rather
// than written a second time; `stagefill.js` is one function over
// `stageresting.js`, and a case below asserts the two agree on every input.
//
//   npx vitest run src/lib/stagefill.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';
import { restingLayout } from './stageresting.js';
import {
  stageFill,
  fillGeometry,
  clipCoversTimer,
  FILL_MESSAGE_PCT,
  FILL_SIZE_CQW,
} from './stagefill.js';
import { programmeRows, TIMER_LABEL, OVER_WORD } from './timers.js';

const TR = readFileSync(resolve('src/lib/TemplateRender.svelte'), 'utf8');
const PHONE = readFileSync(resolve('src/Stage.svelte'), 'utf8');

// ─────────────────────────────────────────────────────────────────────────────
// PART 1 · THE RULE, AND IT IS THE PHONE'S
// ─────────────────────────────────────────────────────────────────────────────

describe('who takes the room on a stage screen served by output.html', () => {
  it('the timer, when nothing is being read', () => {
    expect(stageFill({ timers: 1 })).toBe('timer');
  });

  it('the message, when one is up and nothing is being read', () => {
    expect(stageFill({ message: true })).toBe('message');
  });

  it('both, when both — which is the operator’s "timer AND Stage Message"', () => {
    expect(stageFill({ timers: 2, message: true })).toBe('both');
  });

  it('and NOBODY, the instant anything is on the screen', () => {
    // §116's line, kept: a note may not cover a reading. A picture is not text,
    // but a clock painted over the picture is the same loss — the screen is
    // being used.
    expect(stageFill({ timers: 1, message: true, reading: true })).toBe('none');
    expect(stageFill({ timers: 1, message: true, slide: true })).toBe('none');
  });

  it('a congregation countdown outranks the Stage Timer, exactly as on the phone', () => {
    // `restingLayout` answers `figures` for this; the room is the countdown's and
    // the rail stays furniture.
    expect(stageFill({ timers: 1, countdown: true })).toBe('none');
  });

  it('nothing at all when there is neither a clock nor a word', () => {
    expect(stageFill({})).toBe('none');
  });

  it('it IS `restingLayout` — every input, both rules, one answer', () => {
    // The guarantee that this is not a second opinion about the same question.
    // A `programme` verdict from the phone's rule is the one case where the rail
    // may take a stage screen's height, and nothing here may widen that.
    for (const reading of [false, true])
      for (const slide of [false, true])
        for (const countdown of [false, true])
          for (const timers of [0, 3]) {
            const phone = restingLayout({ reading, slide, countdown, programme: timers > 0 });
            const big = stageFill({ reading, slide, countdown, timers });
            expect(
              big === 'timer',
              `phone says ${phone}, big screen says ${big}`,
            ).toBe(phone === 'programme');
          }
  });

  it('and a clip that has taken the rail’s place takes its room with it', () => {
    // Requirement 6: the clip countdown REPLACES the timer. A rail that filled
    // the screen behind a clip clock would be the thing the operator asked to
    // have covered, at its largest.
    expect(stageFill({ timers: 1, clipCovers: true })).toBe('none');
  });
});

describe('the geometry a fill hands out', () => {
  it('the timer alone takes the whole frame', () => {
    expect(fillGeometry('timer')).toEqual({
      message: null,
      timer: { top: 0, height: 100 },
    });
  });

  it('the message alone takes the whole frame', () => {
    expect(fillGeometry('message')).toEqual({
      message: { top: 0, height: 100 },
      timer: null,
    });
  });

  it('and together they STACK — words above, digits below, no overlap', () => {
    // The failure to avoid is the one RG-247 measured on the phone: two things
    // in one box with a stacking rule deciding which the preacher reads.
    const g = fillGeometry('both');
    expect(g.message).toEqual({ top: 0, height: FILL_MESSAGE_PCT });
    expect(g.timer).toEqual({ top: FILL_MESSAGE_PCT, height: 100 - FILL_MESSAGE_PCT });
    expect(g.message.top + g.message.height).toBe(g.timer.top);
    expect(g.timer.top + g.timer.height).toBe(100);
  });

  it('nothing fills when nothing was asked to', () => {
    expect(fillGeometry('none')).toEqual({ message: null, timer: null });
    expect(fillGeometry(undefined)).toEqual({ message: null, timer: null });
  });
});

describe('when a clip stands in for the timer (requirement 6)', () => {
  it('a video with time left covers it', () => {
    expect(clipCoversTimer({ stage: true, remainingMs: 34_000 })).toBe(true);
  });

  it('a STILL PICTURE never does — the operator said so in the same sentence', () => {
    expect(clipCoversTimer({ stage: true, still: true, remainingMs: 34_000 })).toBe(false);
  });

  it('a clip that has ENDED gives the room back, rather than leaving a dead 0:00', () => {
    // The trap: `clipRemainingMs` answers 0 for a finished clip that is still
    // mounted, and 0 is not null. A cover keyed on "is there a figure" would
    // hold the timer off the screen for the rest of the service.
    expect(clipCoversTimer({ stage: true, remainingMs: 0 })).toBe(false);
  });

  it('and a player that does not know yet covers nothing', () => {
    expect(clipCoversTimer({ stage: true, remainingMs: null })).toBe(false);
    expect(clipCoversTimer({ stage: true })).toBe(false);
  });

  it('a screen that is not a stage is never asked the question', () => {
    // A congregation is not shown a clip countdown at all (RG-280), so there is
    // nothing there to cover with.
    expect(clipCoversTimer({ stage: false, remainingMs: 34_000 })).toBe(false);
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// PART 2 · THE WORDS ON THE RAIL (requirements 2 and 4)
// ─────────────────────────────────────────────────────────────────────────────

const NOW = 1_700_000_000_000;
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

describe('a Stage Timer says what it is and what it has done', () => {
  it('a timer with no name is called TIMER, not nothing', () => {
    // Requirement 4. `programmeRows` is the ONE place a rail's rows are made —
    // the phone's and the big screen's — so the default is applied once and both
    // surfaces have it or neither does.
    expect(TIMER_LABEL).toBe('TIMER');
    const [r] = programmeRows([wire({ label: '' })], NOW);
    expect(r.label).toBe('TIMER');
    expect(programmeRows([wire({ label: '   ' })], NOW)[0].label).toBe('TIMER');
  });

  it('and a timer that HAS a name keeps it', () => {
    expect(programmeRows([wire()], NOW)[0].label).toBe('Sermon');
  });

  it('past zero it says TIME UP, and still says how far over', () => {
    // Requirement 2. The figure is NOT replaced: a preacher needs to know how
    // far over he is, which is the whole of RG-153.
    expect(OVER_WORD).toBe('TIME UP');
    const [r] = programmeRows([wire({ countdown_to: NOW - 277_000 })], NOW);
    expect(r.v).toBe('+4:37');
    expect(r.over).toBe(true);
    expect(r.state).toBe('TIME UP');
  });

  it('a running timer says neither', () => {
    expect(programmeRows([wire()], NOW)[0].state).toBe('');
  });

  it('a HELD timer says Held and never TIME UP', () => {
    // `over` already stands down while held (a figure somebody chose to freeze
    // is not an alarm), and the word has to follow it or the rail says two
    // things about one clock.
    const [r] = programmeRows([wire({ countdown_paused_ms: -120_000 })], NOW);
    expect(r.held).toBe(true);
    expect(r.state).toBe('Held');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// PART 3 · THE STYLESHEET, PRICED RATHER THAN NUDGED (requirement 7)
// ─────────────────────────────────────────────────────────────────────────────
//
// jsdom lays nothing out, so a `clientWidth` assertion here would pass on two
// zeroes. `docklayout.test.js` sets the precedent: read the rule and price it.
// The measured figures are in the RG-285 row.

const STYLE = TR.slice(TR.indexOf('<style>'));
const rule = (sel) => {
  const m = STYLE.match(new RegExp(`${sel.replace(/[.]/g, '\\.')}\\s*\\{([^}]*)\\}`));
  return m ? m[1] : '';
};

describe('the digits sit in the middle of the room the timer has', () => {
  it('the cell centres them both ways, which is the phone’s own rule', () => {
    // `.lp-cell` was `flex-direction: column` with the defaults, so a single
    // timer printed its digits hard against the left edge of a cell that spans
    // the rail. The phone fixed exactly this in RG-248 and says at the line that
    // "left and top was an accident of the defaults rather than a decision".
    const r = rule('.lp-cell');
    expect(r, '.lp-cell is gone').not.toBe('');
    expect(r, 'the digits are still left-aligned in their cell').toMatch(/align-items:\s*center/);
    expect(r).toMatch(/text-align:\s*center/);
  });

  it('and the rail centres the SET when it does not fill the row', () => {
    // The other half of RG-248: with one timer in a full-width rail, `flex:1 1 0`
    // fills the row and centring the cell is enough — but the `+N more` cell is
    // `flex: 0 1 auto`, so a rail that does not fill needs this too.
    expect(rule('.lprog')).toMatch(/justify-content:\s*center/);
  });

  it('the phone still has both, so neither surface can lose it alone', () => {
    // ANCHORED AT THE RULE, not at the first mention. `PHONE.indexOf('.tmr {')`
    // lands inside the comment that quotes `.tmr { flex: 1 1 0 }` as the DEFECT
    // RG-248 fixed, so a scanner written that way reads the bug and asserts
    // against it — which is this file's own subject in miniature.
    const at = PHONE.indexOf('\n  .tmr { flex');
    expect(at, 'no rule for .tmr at all').toBeGreaterThan(0);
    const tmr = PHONE.slice(at, PHONE.indexOf('}', at));
    expect(tmr).toMatch(/align-items:\s*center/);
    expect(tmr).toMatch(/text-align:\s*center/);
  });
});

describe('a filling rail is sized by its box, not by a designer’s box', () => {
  it('the Size cap is lifted while the rail owns the screen', () => {
    // `--lp-sz` is the Size field converted into the rail's own container
    // (`timers.js::railSize`), and it is a cap. While the rail IS the screen
    // there is no designed box for that figure to be a share of, so the two real
    // caps — the per-column budget and the rail's own height — are what size it.
    expect(FILL_SIZE_CQW).toBeGreaterThanOrEqual(100);
    expect(TR, 'a filling rail still wears the designer’s size cap').toMatch(/FILL_SIZE_CQW/);
  });

  it('and the room it measures is the room a cell actually has', () => {
    // ── FOUND BY LOOKING, AFTER EVERY TEST IN THIS FILE WAS GREEN ────────────
    //
    // `.lprog.fills` carries `padding: 2cqw`, which is 77px on a 1080p screen —
    // and `clientHeight` is the PADDING box. `programmeRoom` was therefore told
    // a 486px rail had 486px to give, capped the figure at 462px, and Chromium
    // put it in a 409px cell: `2:25` cut through the middle along the bottom of
    // a projector. It is the arithmetic error `measureProgramme`'s own comment
    // records (`head + gap + figure` passing the rail) made one box further out,
    // and no test in this repository could see it because jsdom lays nothing
    // out. Measured at 1920x1080 before and after: figure 462px -> 376px,
    // bottom 1px inside the cell, `scrollHeight === clientHeight` everywhere.
    //
    // So this is a SOURCE guard and says so. The number lives in the RG-285 row
    // beside the command that produced it.
    const body = codeOnly(TR);
    expect(body, 'no content-box helper at all').toMatch(/const innerHeightOf = /);
    expect(body, 'a designed rail still hands its padding box to programmeRoom').toMatch(
      /innerHeightOf\(progEls\[i\]\)/,
    );
    expect(body, 'the fallback rail still hands its padding box over').toMatch(
      /innerHeightOf\(defaultProgEl\)/,
    );
    expect(body, 'clientHeight is still going straight into the height slot').not.toMatch(
      /const h = (progEls\[i\] \? progEls\[i\]|defaultProgEl)\.clientHeight/,
    );
    // AND THE GAP UNDER THE LABEL IS THE LABEL'S. 8px at 1920 — small, and it
    // is what tipped the filling rail over its box.
    expect(body, 'the gap between the head and the figure is reserved by nobody').toMatch(
      /gapUnder\(head\.parentElement\)/,
    );
  });

  it('and there is a rule for the class the markup names', () => {
    // A `class:` directive naming a class no stylesheet defines has been shipped
    // in this repository before, and it renders nothing while looking correct.
    expect(rule('.lprog.fills'), 'no rule for .lprog.fills').not.toBe('');
    expect(rule('.lmsg.fills'), 'no rule for .lmsg.fills').not.toBe('');
  });
});

// ─────────────────────────────────────────────────────────────────────────────
// PART 4 · AND ALL OF IT, RENDERED, ON THE PAGE THE OPERATOR IS LOOKING AT
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
  }
  send() {}
  close() {}
}

/** A stage-shaped template with a designed programme box in the corner. */
const STAGE_TPL = {
  id: 7,
  name: 'Stage',
  layout: {
    layers: [
      { id: 'bg', type: 'background', fill: '#000', x: 0, y: 0, w: 100, h: 100 },
      { id: 'v', type: 'text', bind: 'verse', x: 5, y: 10, w: 90, h: 40, size: 4, color: '#fff' },
      { id: 'p', type: 'text', bind: 'programme', x: 60, y: 80, w: 38, h: 16, size: 3, color: '#fff' },
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
const msg = () => host.querySelector('.lmsg');

async function outputPage(channel, roles, tpl = STAGE_TPL) {
  window.history.replaceState({}, '', `/output.html?channel=${channel}&template_id=7`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Output({ target: host });
  await settle();
  send({ kind: 'template', id: 7, template: tpl });
  send({ kind: 'channel_roles', roles });
  await settle();
}

const timerFrame = (timers) => ({ kind: 'timer', timers, warn_default_ms: 60_000 });
const running = (over = {}) => ({
  id: 1,
  label: 'Covenant Hour',
  countdown_to: Date.now() + 240_000,
  countdown_from: Date.now() - 60_000,
  countdown_paused_ms: null,
  countdown_done: null,
  warn_ms: null,
  ...over,
});

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  window.history.replaceState({}, '', '/');
});

describe('the platform monitor, with nothing fired', () => {
  it('gives the whole screen to the clock', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    await settle();
    const el = rail();
    expect(el, 'no rail at all').toBeTruthy();
    expect(el.className, 'the clock is still in the corner its designer drew').toContain('fills');
    // The BOX, not the class: a class with no geometry behind it is the defect
    // this file already guards one line up.
    expect(el.getAttribute('style')).toContain('height:100%');
    expect(el.getAttribute('style')).toContain('top:0%');
  });

  it('and hands it straight back the instant a verse is fired', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    await settle();
    expect(rail().className).toContain('fills');
    send({
      kind: 'content',
      content_kind: 'scripture',
      reference: 'John 3:16',
      text: 'For God so loved the world',
    });
    await settle();
    expect(rail(), 'the preacher lost his clock to a reading').toBeTruthy();
    expect(rail().className, 'the clock covered the reading').not.toContain('fills');
  });

  it('and takes it again the moment the screens are cleared', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'x' });
    await settle();
    expect(rail().className).not.toContain('fills');
    send({ kind: 'clear' });
    await settle();
    expect(rail().className).toContain('fills');
  });

  it('a CONGREGATION screen fills nothing, because it has no clock to fill with', async () => {
    // The safety argument is unchanged and is the role's: `Output.svelte` hands
    // no programme to a screen that is not a stage, so there is nothing here for
    // a fill rule to enlarge.
    await outputPage(1, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    await settle();
    expect(rail(), '"Covenant Hour 4:00" is in front of the whole building').toBeNull();
  });
});

describe('a word from the desk, on the big screen', () => {
  it('takes the screen when nothing is being read', async () => {
    // Requirement 5, and the phone's `messagePlacement` answer of `large`.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'stage_alert', text: 'Wrap up in five', urgent: false });
    await settle();
    expect(msg(), 'no strip and no panel — the word went nowhere').toBeTruthy();
    expect(msg().className).toContain('fills');
    expect(msg().textContent).toContain('Wrap up in five');
  });

  it('and shrinks back to the foot strip over a reading, which is §116’s line', async () => {
    // A NOTE MAY NOT COVER THE READING. That is the whole distinction the
    // operator drew two days earlier, and requirement 5 is about an empty screen.
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send({ kind: 'content', content_kind: 'scripture', reference: 'John 3:16', text: 'For God' });
    send({ kind: 'stage_alert', text: 'Wrap up in five', urgent: false });
    await settle();
    expect(msg()).toBeTruthy();
    expect(msg().className).not.toContain('fills');
  });

  it('and shares an empty screen with the clock rather than covering it', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    send({ kind: 'stage_alert', text: 'Wrap up in five', urgent: false });
    await settle();
    expect(msg().className).toContain('fills');
    expect(rail().className).toContain('fills');
    // Stacked, never overlapped.
    expect(msg().getAttribute('style')).toContain(`height:${FILL_MESSAGE_PCT}%`);
    expect(rail().getAttribute('style')).toContain(`top:${FILL_MESSAGE_PCT}%`);
  });

  it('an ALERT is untouched — it was already the whole screen and it flashes', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    send({ kind: 'stage_alert', text: 'STOP — medical', urgent: true });
    await settle();
    expect(host.querySelector('.lalert'), 'the alarm stopped being an alarm').toBeTruthy();
    expect(msg(), 'the alarm and the note painted at once').toBeNull();
  });
});

describe('a clip standing in for the clock', () => {
  /** Put a video on this screen and tell its player how long it is. */
  async function playing(durationS, atS) {
    send({
      kind: 'content',
      content_kind: 'media',
      media_url: '/media/7',
      media_kind: 'video',
    });
    await settle();
    const el = host.querySelector('video');
    if (!el) return null;
    Object.defineProperty(el, 'duration', { value: durationS, configurable: true });
    Object.defineProperty(el, 'currentTime', { value: atS, configurable: true, writable: true });
    el.dispatchEvent(new Event('loadedmetadata'));
    await settle();
    return el;
  }

  it('the clip clock replaces the rail while the video is running', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    await settle();
    expect(rail()).toBeTruthy();
    await playing(120, 86);
    expect(host.querySelector('.lclip'), 'no clip clock').toBeTruthy();
    expect(host.querySelector('.lclip').textContent).toContain('0:34');
    expect(rail(), 'the clip clock and the rail are both on the picture').toBeNull();
  });

  it('and the rail comes back when the clip ENDS, rather than a dead 0:00', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    const el = await playing(120, 86);
    expect(rail()).toBeNull();
    Object.defineProperty(el, 'currentTime', { value: 120, configurable: true, writable: true });
    el.dispatchEvent(new Event('ended'));
    await settle();
    expect(rail(), 'the clock never came back from a clip that finished').toBeTruthy();
  });

  it('and when the media is CLEARED', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    await playing(120, 86);
    expect(rail()).toBeNull();
    send({ kind: 'clear' });
    await settle();
    expect(rail()).toBeTruthy();
  });

  it('a STATIC PICTURE never covers the clock — the operator said so outright', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running()]));
    send({
      kind: 'content',
      content_kind: 'media',
      media_url: '/media/9',
      media_kind: 'image',
    });
    await settle();
    expect(rail(), 'a photograph took the preacher’s clock away').toBeTruthy();
  });
});

describe('the rail says TIME UP where it says Held, on both surfaces', () => {
  it('on the big screen', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running({ countdown_to: Date.now() - 811_000 })]));
    await settle();
    expect(rail().textContent).toContain('TIME UP');
    expect(rail().textContent, 'how far over went with it').toMatch(/\+13:3\d/);
    expect(rail().textContent, 'and the preacher lost which clock it was').toContain(
      'Covenant Hour',
    );
  });

  it('and the phone renders the same field rather than its own word', () => {
    // One rule, two surfaces. The phone hard-coded `Held`; the moment a second
    // state word exists that is a rail with its own opinion.
    expect(PHONE, 'Stage.svelte still writes its own state word').toMatch(
      /\{#if t\.state\}<span class="tstate">\{t\.state\}<\/span>\{\/if\}/,
    );
  });

  it('a timer with no name shows TIMER on both', async () => {
    await outputPage(2, { 1: 'main', 2: 'stage' });
    send(timerFrame([running({ label: '' })]));
    await settle();
    expect(rail().textContent).toContain('TIMER');
  });
});
