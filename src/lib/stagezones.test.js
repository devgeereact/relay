// W5 · THE STAGE MONITOR — zones, the stacked rail clock, and the geometry that
// cannot overflow. docs/REBRAND.md §5.
//
// A stage monitor is not a congregation screen in other colours. Six zones, each
// switchable, and the figures either beside the reading or across the bottom.
//
// TWO INSTRUMENTS, and the split is deliberate. jsdom has no layout engine, so
// "nothing may leave the screen" cannot be measured here — a `getBoundingClientRect`
// in jsdom is zeroes, and a test that asserts on zeroes passes over a broken page.
// So the geometry is held as a CONTRACT ON THE STYLESHEET (the reading is the only
// `flex: 1 1 0`, the rows below are `flex-basis` and never `height`, and each is
// clipped), and the rendered measurement is a browser job recorded in the PR.
// Everything that is real in jsdom — what renders, what persists, what the figures
// say — is asserted against the mounted component.
//
//   npx vitest run src/lib/stagezones.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { tick } from 'svelte';

const Stage = (await import('../Stage.svelte')).default;
const { formatCountdown } = await import('./layers.js');

const SRC = readFileSync(path.resolve(__dirname, '../Stage.svelte'), 'utf8');
const ZONE_KEY = 'relay.stage.zones';

// The page opens a kiosk socket on mount. A stub, so the component mounts exactly
// as it does in a browser and messages can be pushed through the real `apply`.
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

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  try {
    localStorage.removeItem(ZONE_KEY);
  } catch {
    /* the shim in test-setup.js hands us a real Storage */
  }
});
afterEach(() => {
  cleanup();
});

let host;
let app;

function cleanup() {
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
}

/** Mount the stage page and deliver one hub frame through the real socket path. */
async function mount(frame) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  if (frame) {
    socket.onopen?.();
    socket.onmessage({ data: JSON.stringify(frame) });
  }
  await tick();
  await tick();
  return { container: host };
}

/** The button whose visible text is exactly this. */
function button(text) {
  const b = [...host.querySelectorAll('button')].find((n) => n.textContent.trim() === text);
  expect(b, `no control reads "${text}"`).toBeTruthy();
  return b;
}
async function click(text) {
  button(text).click();
  await tick();
}

const verse = {
  kind: 'content',
  reference: 'Romans 8:28',
  text: 'And we know that all things work together for good.',
  translation: 'KJV',
};

describe('zones — clean by default, the rest is switched on', () => {
  it('a fresh stage screen shows the reading, the countdown and the clock, and nothing else', async () => {
    const { container } = await mount({ ...verse, stage_note: 'Wrap at 11:40', service_started_at: Date.now() - 60_000 });

    expect(container.querySelector('.reading')).toBeTruthy();
    expect(container.querySelector('.reading').textContent).toContain('Romans 8:28');
    // A note the operator typed is a zone, and a zone that is off is off — even
    // when the operator has typed one.
    expect(container.querySelector('.noterow')).toBeNull();
    expect(container.querySelector('.next')).toBeNull();
    // Elapsed is off by default too, so a service that IS running shows no figure
    // for it until somebody asks.
    expect(container.textContent).not.toContain('Elapsed');
    // The clock is on, and it is a figure now rather than a fixed header item —
    // which is what makes it switchable at all.
    expect(container.querySelector('.figrow')).toBeTruthy();
    expect(container.textContent).toContain('Time');
  });

  it('switching a zone on shows it, and the choice survives a reload of this device', async () => {
    const note = 'Two minutes on the offering';
    const { container } = await mount({ ...verse, stage_note: note });

    await click('Zones');
    await click('Note');
    await tick();
    expect(container.querySelector('.noterow')?.textContent).toContain(note);

    // Persisted per DEVICE. Two stage screens in one building are allowed to want
    // different things, and the console must not have to know about either.
    expect(JSON.parse(localStorage.getItem(ZONE_KEY)).note).toBe(true);

    cleanup();
    const again = await mount({ ...verse, stage_note: note });
    expect(again.container.querySelector('.noterow')).toBeTruthy();
  });

  it('a device that cannot store anything still gets the default layout', async () => {
    const spy = vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('site data blocked');
    });
    const { container } = await mount(verse);
    expect(container.querySelector('.reading')).toBeTruthy();
    expect(container.querySelector('.figrow')).toBeTruthy();
    spy.mockRestore();
  });
});

describe('the stacked rail clock', () => {
  it('beside the reading, the countdown is three stacked pairs and the rail counts its own rows', async () => {
    const remaining = 3_725_000; // 1:02:05
    const { container } = await mount({
      ...verse,
      text: null,
      countdown_to: Date.now() + remaining,
    });

    await click('Zones');
    await click('Figures beside the reading');
    await tick();

    const rail = container.querySelector('.rail');
    expect(rail, 'the figures did not move beside the reading').toBeTruthy();

    const rows = [...rail.querySelectorAll('.railrow')].map((n) => n.textContent.trim());
    // HH / MM / SS, each its own row, plus the clock row (on by default).
    const [hh, mm, ss] = formatCountdown(remaining, 'hms').split(':');
    expect(rows.slice(0, 3)).toEqual([hh.padStart(2, '0'), mm, ss]);

    // `--rows` is what keeps the stack inside the rail's own height. It must count
    // the rows that are actually there, or the figure is sized for a layout nobody
    // is looking at.
    expect(rows).toHaveLength(4); // HH · MM · SS · the clock
    expect(rail.style.getPropertyValue('--rows').trim()).toBe(String(rows.length));
  });

  it('the pairs are the ONE formatter split, not a second piece of arithmetic', () => {
    // The rail and the figure beneath the reading must not be able to disagree, and
    // the way that is guaranteed is that one is literally the other's output.
    expect(SRC).toMatch(/formatCountdown\(cdRemain, 'hms'\)\s*\n?\s*\.split\(':'\)/);
    expect(SRC).not.toMatch(/Math\.floor\([^)]*3600/);
  });
});

describe('nothing may leave the screen', () => {
  // The stylesheet IS the guarantee here, so the stylesheet is what is asserted.
  // A row given a `height` is a floor a long passage pushes past — which is how a
  // clock leaves the top of a monitor nobody is standing next to.
  const style = SRC.slice(SRC.indexOf('<style>'));
  const rule = (sel) => {
    const i = style.indexOf(`${sel} {`);
    expect(i, `no rule for ${sel}`).toBeGreaterThan(-1);
    return style.slice(i, style.indexOf('}', i));
  };

  it('the reading takes what is left', () => {
    expect(rule('.reading')).toContain('flex: 1 1 0');
    expect(rule('.reading')).toContain('min-height: 0');
  });

  it('the rows below are the only fixed sizes, and they are a BASIS', () => {
    for (const sel of ['.rail', '.figrow']) {
      const r = rule(sel);
      expect(r, `${sel} must be a flex basis`).toMatch(/flex: 0 0 /);
      expect(r, `${sel} must not be given a height`).not.toMatch(/[^-]height:\s*\d/);
      expect(r, `${sel} must clip`).toContain('overflow: hidden');
    }
    expect(rule('.noterow')).toContain('overflow: hidden');
  });

  it('the rail is its own container, so the figure is a share of the rail', () => {
    expect(rule('.rail')).toContain('container-type: size');
    // …and the figure is sized in CONTAINER units, never viewport ones — a figure
    // sized against the frame looked right at one rail width and was clipped at
    // every other.
    const row = rule('.railrow');
    expect(row).toMatch(/cqw/);
    expect(row).toMatch(/cqh/);
    expect(row).not.toMatch(/\d(vw|vh)/);
    // AND IT COUNTS ITS CHARACTERS. `62cqw` fills a rail with a two-character pair
    // and put an eight-character clock at 205px in a 333px rail — clipped, silently,
    // because the row is `overflow: hidden`. Measured in a browser at 1280×800.
    expect(row, 'the rail figure must be sized by how many characters it has').toMatch(
      /var\(--ch/,
    );
    expect(rule('.fig .figv'), 'the bottom row needs the same rule').toMatch(/var\(--ch/);
  });
});

describe('a word to the preacher', () => {
  it('takes the whole screen even when every zone is switched off', async () => {
    const { container } = await mount(verse);

    await click('Zones');
    for (const z of ['Reading', 'Countdown', 'Clock']) await click(z);
    await tick();
    expect(container.querySelector('.reading')).toBeNull();

    socket.onmessage({ data: JSON.stringify({ kind: 'stage_alert', text: 'Wrap up — 5 minutes' }) });
    await tick();
    const alert = container.querySelector('.alert');
    expect(alert, 'an instruction a switched-off zone could hide is not an instruction').toBeTruthy();
    expect(alert.textContent).toContain('Wrap up — 5 minutes');
  });
});
