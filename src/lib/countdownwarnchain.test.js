// RG-149 · THE WARNING THRESHOLD, ALL THE WAY TO THE TWO SCREENS A PERSON LOOKS AT.
//
// Track D landed the rule and left the chain broken in three places, measured in
// `audits/DESIGN-2026-09-16-WAVE3.md` §5d:
//
//   (a) `countdownWarning(remainingMs, totalMs, warnMs)` gained a third argument
//       and all three readers called it with two, so a threshold chosen for one
//       timer could not change anything anywhere.
//   (b) a `Both` timer had no wire field to carry a chosen threshold at all.
//   (c) the Settings default lived in `layers.js` module state whose only writer
//       is `stores/capture.js`, which the output and stage bundles do not and
//       cannot import — so `Settings → General → Countdown warning` moved the
//       console and moved neither the wall nor the preacher's page.
//
// WHAT THIS FILE CAN AND CANNOT PROVE. jsdom computes no layout and there is no
// browser driver here, so nothing below is a claim about a painted colour. What
// it does assert is the two things that were actually wrong: WHICH ARGUMENTS a
// reader passes to the one rule, and WHICH FIGURE that rule then resolves — read
// off the rendered class the stylesheet keys the colour to, and off
// `countdownWarning` itself after a frame has been delivered to a page.
//
// The rule stays `layers.js::countdownWarning` with its existing precedence — a
// figure chosen for one timer, else the configured default, else the tenth-of-span
// rule. Nothing here adds a fourth reading of when to worry.
//
//   npx vitest run src/lib/countdownwarnchain.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import fs from 'node:fs';
import path from 'node:path';

import TemplateRender from './TemplateRender.svelte';
import { countdownWarning, COUNTDOWN_WARN_MS, setCountdownWarnDefault } from './layers.js';

const invoke = vi.fn(async () => null);
const listen = vi.fn(async () => () => {});
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a) => listen(...a) }));

const Stage = (await import('../Stage.svelte')).default;
const Output = (await import('../Output.svelte')).default;

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

let host;
let app;
function mountComponent(Component, props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Component({ target: host, props });
  return host;
}
/** The page mounts through a chain of awaited dynamic imports; drain it. */
async function settle() {
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
  for (let i = 0; i < 40; i += 1) await Promise.resolve();
  await tick();
}

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
  invoke.mockReset();
  invoke.mockResolvedValue(null);
  listen.mockClear();
  // Every test starts from the SHIPPED minute, or one test's delivery would be
  // the next one's precondition and a broken chain could read as a fixed one.
  setCountdownWarnDefault(COUNTDOWN_WARN_MS);
  window.history.replaceState({}, '', '/output.html');
});
afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  setCountdownWarnDefault(COUNTDOWN_WARN_MS);
});

  // RG-166 — `type: 'bg'` IS NOT A LAYER TYPE. `TemplateRender` draws
  // `background`, `backdrop`, `media`, `shape`, `band`, `region`, `text` and
  // `timer`, and nothing else, so this template declared a black background that
  // the renderer has never painted: it was a fully TRANSPARENT screen the whole
  // time, and `layers.js::isKeyedTemplate` says so. That did not matter while the
  // countdown refusal read `layout.lowerThird`; it does now that both render
  // paths ask the keyed question, and a countdown may not paint over a camera.
  // Corrected rather than exempted — the fixture now IS the opaque screen it
  // always claimed to be, which is what these assertions are about.
const TIMER_TEMPLATE = {
  id: 1,
  name: 'Timer',
  layout: { layers: [{ id: 'bg', type: 'background', visible: true, fill: '#000', opacity: 1 }] },
  style: { verseSize: '6' },
};

// ── (a) A THRESHOLD CHOSEN FOR ONE TIMER REACHES THE SURFACES ────────────────
//
// Fifteen minutes aimed, ninety seconds left. Under the shared rule that is not a
// warning (the window is the last minute); with two minutes chosen for this timer
// it is. So the assertion cannot pass by accident on a reader that drops the
// figure — which is exactly what all three did.
const AIMED_MS = 15 * 60_000;
const chosenCountdown = (warnMs) => {
  const to = Date.now() + 90_000;
  return {
    kind: 'countdown',
    reference: 'Service begins in',
    countdown_to: to,
    countdown_from: to - AIMED_MS,
    countdown_warn_ms: warnMs,
  };
};

describe('(a) the wall reads the threshold the timer carries', () => {
  it('turns the digits red at a figure chosen for this countdown', () => {
    const c = mountComponent(TemplateRender, {
      template: TIMER_TEMPLATE,
      content: chosenCountdown(120_000),
    });
    const digits = c.querySelector('.cd-default .cd-digits .lfit');
    expect(
      digits.classList.contains('warn'),
      'the wall ignored the threshold the timer carries — `countdownWarning` was ' +
        'called with two arguments and the third is the whole of RG-149(a)',
    ).toBe(true);
  });

  it('and does not, at ninety seconds, when nobody chose one', () => {
    // The control case. Without it the test above passes on a reader that always
    // warns, which is a different broken chain with the same green tick.
    const c = mountComponent(TemplateRender, {
      template: TIMER_TEMPLATE,
      content: chosenCountdown(null),
    });
    expect(c.querySelector('.cd-default .cd-digits .lfit').classList.contains('warn')).toBe(false);
  });

  it('a figure chosen SHORTER than the shared rule holds the colour back', () => {
    // The other direction, and the one a church actually asks for: a short
    // pre-service countdown where a minute of red says nothing. Thirty seconds
    // chosen, forty left — inside the shared minute, outside the chosen window.
    const to = Date.now() + 40_000;
    const c = mountComponent(TemplateRender, {
      template: TIMER_TEMPLATE,
      content: {
        kind: 'countdown',
        reference: 'x',
        countdown_to: to,
        countdown_from: to - AIMED_MS,
        countdown_warn_ms: 30_000,
      },
    });
    expect(c.querySelector('.cd-default .cd-digits .lfit').classList.contains('warn')).toBe(false);
  });
});

describe("(a) the preacher's page reads it too", () => {
  it('the countdown figure carries .warn at the chosen threshold', async () => {
    mountComponent(Stage, {});
    await tick();
    socket.onopen?.();
    await tick();
    socket.onmessage({ data: JSON.stringify({ ...chosenCountdown(120_000), kind: 'content' }) });
    await tick();
    await tick();
    const fig = host.querySelector('.figrow .fig');
    expect(fig, 'the countdown figure did not render at all').toBeTruthy();
    expect(
      fig.classList.contains('warn'),
      "the preacher's page dropped the chosen threshold — RG-149(a), the same " +
        'reader with two arguments',
    ).toBe(true);
  });

  it('and stays calm at ninety seconds when nobody chose one', async () => {
    mountComponent(Stage, {});
    await tick();
    socket.onopen?.();
    await tick();
    socket.onmessage({ data: JSON.stringify({ ...chosenCountdown(null), kind: 'content' }) });
    await tick();
    await tick();
    expect(host.querySelector('.figrow .fig').classList.contains('warn')).toBe(false);
  });
});

describe('(a) the dock reads it', () => {
  it('passes the live content’s own threshold to the one rule', () => {
    // A source assertion, deliberately: the dock is the console chrome and this
    // file has no console to mount. What it pins is the argument list, which is
    // the whole of the defect on this surface.
    const src = read('src/lib/Dock.svelte');
    expect(
      src,
      'the dock called `countdownWarning` with two arguments, so a threshold ' +
        'chosen for the countdown on the wall never reached the figure beside it',
    ).toMatch(/cdWarn = cdLive && countdownWarning\(\s*cdRunning\s*,\s*cdTotal\s*,\s*\$live\?\.countdown_warn_ms\s*\)/);
  });
});

// ── (c) THE CONFIGURED DEFAULT REACHES THE TWO PAGES THAT CANNOT READ IT ─────
//
// These assert the FIGURE THE RULE RESOLVES rather than a class, because that is
// the claim: after a frame carrying the machine's configured default has been
// delivered, `countdownWarning` — the same module instance the page is using —
// answers at that figure instead of at the shipped minute.
const NINETY_SECONDS = 90_000;

describe('(c) the configured default is delivered to the wall', () => {
  /** Mount the page on the KIOSK path — the browser source in OBS, the Pi in the
   *  foyer: no Tauri bridge at all, which is why the figure has to be delivered.
   *  The page takes that path when the event import throws, so that is arranged
   *  rather than pretended at. */
  async function mountKiosk() {
    listen.mockRejectedValue(new Error('no bridge here'));
    mountComponent(Output, {});
    await settle();
    socket.onopen?.();
    await tick();
  }

  it('a content frame carrying it moves the rule this page is using', async () => {
    expect(countdownWarning(NINETY_SECONDS), 'precondition: the shipped minute').toBe(false);

    await mountKiosk();
    socket.onmessage({
      data: JSON.stringify({
        kind: 'content',
        content_kind: 'countdown',
        reference: 'Service begins in',
        countdown_to: Date.now() + NINETY_SECONDS,
        countdown_warn_default_ms: 150_000,
      }),
    });
    await tick();

    expect(
      countdownWarning(NINETY_SECONDS),
      'the operator set the warning window in Settings and the wall never heard ' +
        'about it — RG-149(c): `setCountdownWarnDefault` has one writer and it is ' +
        'a module this bundle cannot import',
    ).toBe(true);
  });

  it('an unset default leaves the shipped minute standing', async () => {
    // A null is an ABSENT figure, not a window of zero, and not a reason to
    // invent one. The page must end up exactly where it started.
    await mountKiosk();
    socket.onmessage({
      data: JSON.stringify({
        kind: 'content',
        content_kind: 'countdown',
        countdown_to: Date.now() + NINETY_SECONDS,
        countdown_warn_default_ms: null,
      }),
    });
    await tick();

    expect(countdownWarning(NINETY_SECONDS)).toBe(false);
    expect(countdownWarning(30_000)).toBe(true);
  });

  it('and the NATIVE window hears it on its own door', async () => {
    // BOTH DOORS. The projector on HDMI has the Tauri bridge and no socket; the
    // browser source in OBS has the socket and no bridge, and they are usually in
    // the same room. A figure delivered on one of the two is this repository's
    // signature bug, which CLAUDE.md counts four times.
    mountComponent(Output, {});
    await settle();
    const onContent = listen.mock.calls.find(([name]) => name === 'output://content')?.[1];
    expect(onContent, 'the native content listener was never registered').toBeTypeOf('function');

    onContent({
      payload: {
        kind: 'countdown',
        reference: 'Service begins in',
        countdown_to: Date.now() + NINETY_SECONDS,
        countdown_warn_default_ms: 150_000,
      },
    });
    await tick();

    expect(countdownWarning(NINETY_SECONDS)).toBe(true);
  });
});

describe("(c) and to the preacher's page", () => {
  it('the timer frame carries it, which is the frame that reaches this page first', async () => {
    // The stage's own carrier. A programme timer can be running before anything
    // has been fired, so waiting for a content frame would leave the one surface
    // whose entire purpose is the clock on the shipped minute for that whole time.
    expect(countdownWarning(NINETY_SECONDS)).toBe(false);

    mountComponent(Stage, {});
    await tick();
    socket.onopen?.();
    await tick();
    socket.onmessage({
      data: JSON.stringify({ kind: 'timer', timers: [], warn_default_ms: 150_000 }),
    });
    await tick();

    expect(
      countdownWarning(NINETY_SECONDS),
      "the preacher's page never learned the configured window — RG-149(c)",
    ).toBe(true);
  });

  it('a content frame carries it here too', async () => {
    mountComponent(Stage, {});
    await tick();
    socket.onopen?.();
    await tick();
    socket.onmessage({
      data: JSON.stringify({
        kind: 'content',
        reference: 'x',
        countdown_to: Date.now() + NINETY_SECONDS,
        countdown_warn_default_ms: 150_000,
      }),
    });
    await tick();

    expect(countdownWarning(NINETY_SECONDS)).toBe(true);
  });
});
