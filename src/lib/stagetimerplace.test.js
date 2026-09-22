// WHERE THE STAGE TIMER SITS, AND WHICH WAY IT READS (RG-248).
//
// The operator, watching the built page on a phone: *"make timer align properly
// in the middle fill screen with time just at the bottom"*.
//
// Two faults in one sentence. The Stage Timer was rendered ABOVE the time of
// day, so the least important figure on a preacher's screen sat closest to their
// thumb and the one they are working to sat above it. And every timer was
// left-aligned inside a row that spans the frame, so a single timer — the normal
// case — printed its digits hard against the left edge with the rest of the row
// empty.
//
// The order is asserted on the RENDERED page rather than on the source, because
// document order is the thing that decides it and a CSS `order` property would
// satisfy a source scan while painting the same wrong screen.
import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import Stage from '../Stage.svelte';

// THE STYLESHEET ALONE. `.tmr { flex: 1 1 0 }` also appears in a JS comment 1400
// lines above the rule it describes, and a scanner reading the whole file found
// that one first — it passed against a stylesheet it had never looked at, which
// is this repository's recurring instrument fault in miniature.
const FILE = readFileSync(resolve('src/Stage.svelte'), 'utf8');
const SRC = FILE.slice(FILE.lastIndexOf('<style>'));
const decls = (css) => css.replace(/\/\*[\s\S]*?\*\//g, '');
const ruleFor = (sel) => {
  const from = SRC.indexOf(sel + ' {');
  expect(from, `no rule for ${sel}`).toBeGreaterThan(-1);
  return decls(SRC.slice(from, SRC.indexOf('}', from)));
};

let app;
let host;
let socket;

class FakeSocket {
  constructor() {
    socket = this;
  }
  send() {}
  close() {}
}

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  window.history.replaceState({}, '', '/');
});

async function open() {
  window.history.replaceState({}, '', '/stage.html?channel=2');
  global.WebSocket = FakeSocket;
  global.fetch = () => Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  await tick();
}

const send = (m) => socket.onmessage({ data: JSON.stringify(m) });

const timers = () =>
  send({
    kind: 'timer',
    warn_default_ms: 60_000,
    timers: [{ id: 1, label: 'Sermon', countdown_to: Date.now() + 240_000, countdown_from: Date.now() }],
  });

describe('how much room the time of day gets', () => {
  it('a row carrying ONLY the clock is marked as one, and is smaller for it', async () => {
    // The operator, on the built page: *"the current time card is too big....
    // make it smaller as its just the current time... the main focus should be
    // the timer"*. Measured at 390x844: the figure row is a flat 15% — 127px —
    // whether it carries COUNTDOWN, TIME and ELAPSED or the wall clock alone, so
    // the one figure that makes no claim about this service had the same room as
    // three that do.
    await open();
    await tick();
    const row = host.querySelector('.figrow');
    expect(row, 'no figure row at all').toBeTruthy();
    const keys = [...row.querySelectorAll('.figk')].map((n) => n.textContent.trim());
    expect(keys, 'this case is not the clock alone any more').toEqual(['Time']);
    expect(row.className, 'the row does not know it is carrying the clock alone').toContain('clockonly');
    // AND THE CLASS IS NOT DECORATION: it has to take room away.
    const rule = ruleFor('.figrow.clockonly');
    expect(rule, 'the marked row is not given a smaller basis').toMatch(/flex-basis:/);
  });

  it('a row carrying a service figure keeps its room', async () => {
    await open();
    send({ kind: 'service', started_at: Date.now() - 60_000 });
    await tick();
    const row = host.querySelector('.figrow');
    if (![...row.querySelectorAll('.figk')].some((n) => n.textContent.trim() === 'Elapsed')) return;
    expect(row.className, 'a real service figure was treated as the clock alone').not.toContain(
      'clockonly',
    );
  });
});

describe('where the Stage Timer sits', () => {
  it('the timer comes BEFORE the clock, so the clock is at the foot', async () => {
    await open();
    timers();
    await tick();
    const rows = [...host.querySelectorAll('.progrow, .figrow')].map((n) => n.className.split(' ')[0]);
    expect(rows, 'no rows rendered at all').toContain('progrow');
    expect(rows.indexOf('progrow'), 'the time of day sits above the timer').toBeLessThan(
      rows.indexOf('figrow') === -1 ? Infinity : rows.indexOf('figrow'),
    );
  });

  it('a timer is centred in the room it has', () => {
    // BOTH AXES. A single timer in a row that spans the frame was hard against
    // the left edge; a timer that fills the page (`.progrow.owns`) was hard
    // against the top of it.
    expect(ruleFor('.progrow'), 'the row does not centre what is in it').toMatch(
      /justify-content:\s*center/,
    );
    expect(ruleFor('.tmr'), 'the timer does not centre its own digits').toMatch(
      /align-items:\s*center/,
    );
    expect(ruleFor('.tmr'), 'the digits are not centred vertically in the room').toMatch(
      /justify-content:\s*center/,
    );
  });
});
