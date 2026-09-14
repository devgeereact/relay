// T3 · THE OUTPUTS RAIL MAY NOT HOLD A SECOND OPINION ABOUT A SCREEN.
//
// CLAUDE.md rule 35. Three findings, all of them one shape: a word or a number on
// this desk that reads the same when the thing behind it is broken as when it is
// fine.
//
//   1. THE TALLY. The rail's badge and its `Live n / m` fact counted
//      `status[id].online`. For a `network_client` `main.rs` sets that to `true`
//      UNCONDITIONALLY — the output is served for as long as the app runs,
//      whether or not any browser is pulling it — so a church whose OBS sources
//      had all died read **3 / 3 in green** on the rail of the tab they would
//      open to find out, while the cards two columns away correctly painted all
//      three rose. One desk, two verdicts, same second.
//
//   2. THE CLIENT COUNT. `run_kiosk_server` registers a client only inside
//      `if let Some(id) = template_id`, and `main.rs` counts with
//      `c.template_id.map(|t| clients.count(t))`. A screen that FOLLOWS THE
//      CONTENT LOOK has no template id on either side, so its count is
//      structurally zero — with OBS attached and painting, the panel built to
//      answer for one screen printed `Clients 0`, the same thing it prints when
//      nothing is connected at all. An absence is not a zero (rule 31).
//
//   3. THE ON/OFF CONTROL. A private `online ? 'Turn off' : 'Turn on'` ternary in
//      markup, where no test could reach it. Before the first poll `status[id]`
//      is undefined, so it offered **Open** for a screen that may already be
//      open, and pressing it opens a second window on the projector.
//      `screenSwitch` already owns this rule and `outputhealth.test.js` already
//      pins the case — Live used it and Outputs did not, which is the
//      "guarantee kept on one door" shape CLAUDE.md names four times.
//
// Each test below was watched to FAIL against the pre-fix source.
//
//   npx vitest run src/lib/screentally.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';

// The same self-detecting gate `screencards.test.js` uses: if
// `resolve.conditions: ['browser']` is ever tidied out of `vitest.config.js`,
// `onMount` becomes the SSR no-op and every mount below would pass by rendering
// nothing. These SKIP loudly rather than pass vacuously.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture, channelHealth, live, rehearsing, screenBlack, stopChannelHealth } =
  await import('./stores/capture.js');

/** Two networked screens: one with a look of its own, one that follows. */
const SCREENS = [
  { id: 1, name: 'Stream', render_target: 'network_client', template_id: 7, display_target: null, status: 'offline' },
  { id: 2, name: 'Lobby', render_target: 'network_client', template_id: null, display_target: null, status: 'offline' },
];
const NATIVE = { id: 3, name: 'Main screen', render_target: 'native_window', template_id: 7, display_target: null, status: 'offline' };

const TEMPLATES = [
  { id: 7, name: 'Classic Serif', layout: { layers: [{ id: 'bg', type: 'background', fill: '#101018' }] }, style: { verseSize: 6 } },
];

/**
 * A `ChannelLiveness` row as Rust serialises it, in the state that matters here:
 * SERVED but DEAD. `online: true` is what `main.rs` hardcodes for every network
 * client; `painting: false` with a stale beat is the screen's own word.
 */
const dead = (id, name) => ({
  id,
  name,
  online: true,
  clients: 0,
  detail: `Serving · NOT responding for 40s`,
  supported: true,
  painting: false,
  last_beat_ms: 40000,
  paint_state: null,
});

let host;
let app;
let screens = SCREENS;

beforeEach(() => {
  screens = SCREENS;
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_output_channels':
        return Promise.resolve(screens.map((s) => ({ ...s })));
      case 'list_templates':
        return Promise.resolve(TEMPLATES);
      case 'list_monitors':
        return Promise.resolve([{ index: 0, name: 'EPSON EB-2250U', width: 1920, height: 1080, primary: true }]);
      case 'local_ip':
        return Promise.resolve('192.168.1.42');
      case 'get_setting':
        return Promise.resolve('7');
      case 'channel_status':
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
  capture.update((c) => ({ ...c, available: true }));
  channelHealth.set({});
  live.set(null);
  rehearsing.set(false);
  screenBlack.set(false);
});

afterEach(() => {
  stopChannelHealth();
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
});

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}
async function until(predicate, what, tries = 60) {
  for (let i = 0; i < tries; i += 1) {
    if (predicate()) return;
    await settle();
  }
  throw new Error(`timed out waiting for: ${what}`);
}

async function mountOutputs() {
  const Channels = (await import('./views/Channels.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Channels({ target: host });
  await until(() => host.querySelector('.ch-card'), 'the screen cards to render');
  return host;
}

/** The rail's own facts — `Answering`, `This machine`. */
const railFact = (el, key) =>
  [...el.querySelectorAll('.ch-railfact')]
    .find((f) => f.querySelector('.ch-railk')?.textContent.trim() === key)
    ?.querySelector('.ch-railv')
    ?.textContent.trim();

const railBadge = (el) => el.querySelector('.rw-panehead .r-badge');
const cardFor = (el, name) =>
  [...el.querySelectorAll('.ch-card')].find((c) => c.querySelector('.ch-cardname')?.textContent.trim() === name);
const click = async (node) => {
  node.click();
  await settle();
};

describe('the rail tally counts screens that answer, not screens Relay is serving', () => {
  itMounted('reads 0 of 2, in rose, when both served screens have stopped painting', async () => {
    const el = await mountOutputs();
    channelHealth.set({ 1: dead(1, 'Stream'), 2: dead(2, 'Lobby') });
    await settle();

    // The finding, stated as the operator reads it. `online` is true for both of
    // these rows — that is exactly what `main.rs` sends for a network client —
    // and the old tally therefore said 2/2 in green over two dead screens.
    expect(
      railFact(el, 'Answering'),
      'the rail counted screens Relay is serving rather than screens that answered',
    ).toBe('0 / 2');

    const badge = railBadge(el);
    expect(badge.textContent).toContain('0/2');
    expect(
      badge.className,
      'a green tally over two screens that have stopped painting is rule 35 in the smallest space on the desk',
    ).toContain('rose');
    expect(badge.className).not.toContain('green');
  });

  itMounted('agrees with the cards beside it — one verdict, not two', async () => {
    // The point of the fix is not the number; it is that the rail and the cards
    // read the SAME object. If a future edit gives either one its own ladder,
    // this is what catches it.
    const el = await mountOutputs();
    channelHealth.set({
      1: dead(1, 'Stream'),
      2: { ...dead(2, 'Lobby'), painting: true, paint_state: 'clear', last_beat_ms: 300 },
    });
    await settle();

    expect(railFact(el, 'Answering')).toBe('1 / 2');
    expect(cardFor(el, 'Stream').querySelector('.ch-lamp').textContent).toContain('Not responding');
    // ONE screen down is enough to make the badge rose, and that is the intended
    // reading: the tally is an alarm about the building, not an average of it.
    // Two of three working is not a state anyone should have to notice the shade
    // of green for.
    expect(railBadge(el).className).toContain('rose');
  });

  itMounted('is green when every screen is answering', async () => {
    const el = await mountOutputs();
    const ok = (id, name) => ({ ...dead(id, name), painting: true, paint_state: 'clear', last_beat_ms: 300 });
    channelHealth.set({ 1: ok(1, 'Stream'), 2: ok(2, 'Lobby') });
    await settle();

    expect(railFact(el, 'Answering')).toBe('2 / 2');
    expect(railBadge(el).className).toContain('green');
  });
});

describe('a number Relay cannot know is not printed as a fact', () => {
  itMounted('says a following screen is not counted, rather than counting it as zero', async () => {
    const el = await mountOutputs();
    // Lobby follows the content look. The hub counts viewers per TEMPLATE id and
    // it has none, so this row's `clients` is structurally 0 whatever OBS is
    // doing — here with the screen demonstrably painting.
    channelHealth.set({
      2: { ...dead(2, 'Lobby'), painting: true, paint_state: 'content', last_beat_ms: 300, clients: 0 },
    });
    await settle();
    await click(cardFor(el, 'Lobby'));

    const info = host.querySelector('.ch-info').textContent;
    expect(
      info,
      'the inspector printed `Clients 0` for a screen whose viewers Relay does not count',
    ).toContain('not counted');
    expect(info).toContain('counted per template');
  });

  itMounted('still prints the real count for a screen that has a template of its own', async () => {
    const el = await mountOutputs();
    channelHealth.set({
      1: { ...dead(1, 'Stream'), painting: true, paint_state: 'content', last_beat_ms: 300, clients: 3 },
    });
    await settle();
    await click(cardFor(el, 'Stream'));

    const info = host.querySelector('.ch-info').textContent;
    expect(info).toContain('3');
    expect(info, 'a screen Relay CAN count must still be counted').not.toContain('not counted');
  });
});

describe('the on/off control is never a guess about a screen nobody has asked', () => {
  itMounted('does not offer Open for a native screen before the first poll', async () => {
    screens = [NATIVE];
    const el = await mountOutputs();
    // `channelHealth` is `{}` — nothing has been polled. `screenFault` calls that
    // `unknown`, which is "we have not asked", not "it is off".
    const card = cardFor(el, 'Main screen');
    const labels = [...card.querySelectorAll('button')].map((b) => b.textContent.trim());
    expect(
      labels,
      'the card offered Open for a screen it had not yet asked about — pressing it opens a second window',
    ).not.toContain('Open');
    expect(card.textContent).toContain('Checking…');
  });

  itMounted('offers Open once the poll says no window is attached', async () => {
    screens = [NATIVE];
    const el = await mountOutputs();
    channelHealth.set({
      3: { ...dead(3, 'Main screen'), online: false, detail: 'No output window open', last_beat_ms: null },
    });
    await settle();

    const card = cardFor(el, 'Main screen');
    const labels = [...card.querySelectorAll('button')].map((b) => b.textContent.trim());
    expect(labels).toContain('Open');
  });
});
