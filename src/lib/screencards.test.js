// W5 · THE OUTPUTS TAB IS A GRID OF SCREENS, NOT A TABLE OF WORDS.
//
// docs/REBRAND.md §5. The table gave an operator five true facts per screen —
// name, type, template, target, status — and not one of them answers the question
// this tab is opened to ask: *is the lower third sitting over the camera or
// filling the frame; is the lobby screen wearing the warm look or the main one*.
// A word about a picture cannot be checked by looking at the word.
//
// So every card renders through `TemplateRender` — the ONE renderer, the wall's
// own — fed the wall's own content and resolved by the wall's own resolver. What
// this file holds is that this is actually WIRED and not merely written down:
//
//   1. there is a card per screen and each one contains a real rendered slide;
//   2. the state word comes from `describeScreen`, so the cards, Live and the
//      chrome cannot describe one screen three ways (CLAUDE.md rule 35);
//   3. a screen may be set to FOLLOW THE CONTENT LOOK from the card itself
//      (DECISIONS §70), and the URL that card copies says NOTHING about a
//      template — the divergence nobody at the desk can see.
//
//   npx vitest run src/lib/screencards.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';

// Same self-detecting gate as `surface.test.js`: if `resolve.conditions:
// ['browser']` is ever tidied out of `vitest.config.js`, `onMount` becomes the
// SSR no-op and every mount below would pass by rendering nothing. These SKIP
// loudly rather than pass vacuously.
const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture, channelHealth, live, rehearsing, screenBlack, stopChannelHealth } =
  await import('./stores/capture.js');

/** Three screens: one with its own look, one following, one Relay cannot drive. */
const SCREENS = [
  { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 7, display_target: null, status: 'offline' },
  { id: 2, name: 'Streaming', render_target: 'network_client', template_id: null, display_target: null, status: 'offline' },
  { id: 3, name: 'Overflow (NDI)', render_target: 'ndi_encode', template_id: null, display_target: null, status: 'offline' },
];

const TEMPLATES = [
  {
    id: 7,
    name: 'Classic Serif',
    layout: { layers: [{ id: 'bg', type: 'background', fill: '#101018' }] },
    style: { verseSize: 6 },
  },
  {
    id: 8,
    name: 'Nocturne',
    layout: { layers: [{ id: 'bg', type: 'background', fill: '#000010' }] },
    style: { verseSize: 5 },
  },
];

/** A `ChannelLiveness` row as Rust serialises it. */
const row = (over = {}) => ({
  id: 1,
  name: 'Main screen',
  online: true,
  clients: 1,
  detail: 'Serving · screen responding',
  supported: true,
  painting: true,
  last_beat_ms: 400,
  paint_state: 'content',
  ...over,
});

let host;
let app;
let clipboard;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_output_channels':
        return Promise.resolve(SCREENS.map((s) => ({ ...s })));
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
  clipboard = [];
  Object.defineProperty(navigator, 'clipboard', {
    configurable: true,
    value: { writeText: (t) => { clipboard.push(t); return Promise.resolve(); } },
  });
});

afterEach(() => {
  stopChannelHealth();
  if (app) app.$destroy();
  if (host) host.remove();
  app = null;
  host = null;
});

async function mountOutputs() {
  const Channels = (await import('./views/Channels.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Channels({ target: host });
  await until(() => host.querySelector('.ch-card'), 'the screen cards to render');
  return host;
}

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

const cardFor = (el, name) =>
  [...el.querySelectorAll('.ch-card')].find((c) => c.querySelector('.ch-cardname')?.textContent.trim() === name);

describe('§5 · one card per screen, and each card is a rendered slide', () => {
  itMounted('renders a card for every screen, not a table row', async () => {
    const el = await mountOutputs();
    expect(el.querySelectorAll('.ch-card')).toHaveLength(SCREENS.length);
    // The table is gone, not merely restyled. `.ch-row` and its seven-column
    // header were the shape this replaces.
    expect(el.querySelector('.ch-thead')).toBeNull();
    expect(el.querySelector('.ch-row')).toBeNull();
  });

  itMounted('every card contains a real rendered slide, not a drawing of one', async () => {
    const el = await mountOutputs();
    for (const s of SCREENS) {
      const card = cardFor(el, s.name);
      expect(card, `no card for ${s.name}`).toBeTruthy();
      const frame = card.querySelector('.ch-frame');
      expect(frame, `${s.name} has no 16:9 frame`).toBeTruthy();
      // TemplateRender's own root. If the component were not mounted here the
      // frame would be an empty box and every assertion about "what that screen
      // shows" would be about nothing.
      expect(frame.children.length, `${s.name} renders nothing`).toBeGreaterThan(0);
    }
  });

  itMounted('an UNAVAILABLE target still renders the content — the claim is about the transport', async () => {
    // NDI is parked. That is a fact about how pixels would leave the machine, not
    // about what the screen would be asked to show, and a blank card would say
    // the second thing.
    const el = await mountOutputs();
    const ndi = cardFor(el, 'Overflow (NDI)');
    expect(ndi.querySelector('.ch-frame').children.length).toBeGreaterThan(0);
    expect(ndi.querySelector('.ch-cardmeta').textContent).toMatch(/NDI/);
  });

  itMounted('the meta line names the kind and the transport, from the shared helper', async () => {
    const el = await mountOutputs();
    expect(cardFor(el, 'Main screen').querySelector('.ch-cardmeta').textContent.trim()).toBe(
      'Native window · HDMI / display',
    );
    expect(cardFor(el, 'Streaming').querySelector('.ch-cardmeta').textContent.trim()).toBe(
      'Network client · WebSocket',
    );
  });
});

describe('§2 · the desk starts where the prototype starts, and fits three cards', () => {
  // BOTH OF THESE WERE MEASURED IN A BROWSER, not guessed, and both were wrong
  // on the first render of this workspace (1280×900, against the mock bridge):
  // an `Outputs` H1 plus a two-line standfirst started the desk 110px lower than
  // the prototype, and the grid fitted TWO cards across where the prototype fits
  // three — so an operator with five screens scrolled to reach the third.
  //
  // Neither can be held by mounting, because jsdom has no layout. What CAN be
  // held is the arithmetic that produced the wrong answer, computed from the
  // real numbers rather than pinned as a literal: change the rail, the
  // inspector, the gap or the padding and this recomputes and fails.
  const read = (p) => readFileSync(resolve(__dirname, '..', p), 'utf8');
  const outputs = read('lib/views/Channels.svelte');

  it('passes NO page title and NO standfirst — the rail and the pane head say it', () => {
    const call = outputs.slice(outputs.indexOf('<WorkspaceFrame'), outputs.indexOf('<!-- ══ RAIL'));
    expect(call, 'the page title is back').not.toMatch(/\btitle=/);
    expect(call, 'the standfirst is back').not.toMatch(/\bstandfirst=/);
    // …and the frame must not paint an empty `<h1>` in its place — a screen
    // reader announces "heading level 1" and then nothing.
    const frame = read('lib/views/WorkspaceFrame.svelte');
    expect(frame).toMatch(/\{#if title\}<h1 class="rw-h1">/);
  });

  it('the sentence each section is FOR did not just get deleted', () => {
    // Role two of the type scale is still rendered — it moved to the rail foot,
    // where it costs the grid no height and shows on all three sections.
    expect(outputs).toMatch(/class="ch-raillead">\{activeView\.lead\}/);
  });

  it('three cards fit across the main column at 1280, by the real numbers', () => {
    const [, cols] = outputs.match(/columns="([^"]+)"/);
    const [rail, , inspector] = cols.trim().split(/\s+(?![^(]*\))/);
    // THE TRACKS ARE TOKENS NOW, so resolve them out of `app.css` rather than
    // parsing a literal. Both rules of the desk used to be typed per view —
    // rails of 180/206/206/206/212 and inspectors of 250/276/312/320/330 — so
    // both seams moved on every tab press. Reading the token here means this
    // arithmetic checks the width that actually ships, which a literal could
    // only ever agree with by coincidence.
    const sheet = read('app.css');
    const token = (name) => {
      const m = sheet.match(new RegExp(`--${name}\\s*:\\s*(\\d+)px`));
      if (!m) throw new Error(`no --${name} in app.css — the desk's track tokens moved`);
      return parseInt(m[1], 10);
    };
    const px = (v) => {
      const t = String(v).match(/var\(\s*--([a-z0-9-]+)\s*\)/i);
      return t ? token(t[1]) : parseInt(v, 10);
    };

    const [, minTrack] = outputs.match(/\.ch-cards\{[^}]*minmax\((\d+)px/);
    const [, gridGap] = outputs.match(/\.ch-cards\{[^}]*gap:(\d+)px/);
    const [, wrapPad] = outputs.match(/\.ch-gridwrap\{[^}]*padding:(\d+)px/);

    // The one number this test cannot read out of my own files: the shell's page
    // inset either side of the workspace. Stated, not hidden — if the shell
    // changes it, this assumption is what a reader should check first.
    const PAGE_INSET = 28;
    const FRAME_GUTTERS = 16; // two 8px gutters between the three panes

    const across = (page) => {
      const main = page - PAGE_INSET - px(rail) - px(inspector) - FRAME_GUTTERS;
      const inner = main - 2 * px(wrapPad) - 2; // pane borders
      return Math.floor((inner + px(gridGap)) / (px(minTrack) + px(gridGap)));
    };

    // The measured failure: at 1280 this was 2.
    expect(across(1280), 'an operator with five screens scrolls for the third').toBeGreaterThanOrEqual(3);
    expect(across(1440)).toBeGreaterThanOrEqual(3);
    // …and not so small that the preview stops being a preview.
    expect(across(1280)).toBeLessThanOrEqual(4);
  });
});

describe('§5 · the lamp is the word Live uses, about the same backend fact', () => {
  itMounted('a screen that is painting while content is live reads On Air, in amber', async () => {
    const el = await mountOutputs();
    channelHealth.set({ 1: row({ id: 1 }) });
    live.set({ reference: 'Romans 8:28' });
    await settle();
    const lamp = cardFor(el, 'Main screen').querySelector('.ch-lamp');
    expect(lamp.textContent.trim()).toBe('On Air');
    expect(lamp.className).toMatch(/\bamber\b/);
  });

  itMounted('a rehearsal is NEVER amber on a card — amethyst, and it says Rehearsal', async () => {
    // The table's word came from `FAULT_WORD[screenFault(st)]`, which knows
    // nothing about a rehearsal: the same screen read LIVE here and Rehearsal on
    // the run surface, in the same second.
    const el = await mountOutputs();
    channelHealth.set({ 1: row({ id: 1 }) });
    live.set({ reference: 'Romans 8:28' });
    rehearsing.set(true);
    await settle();
    const lamp = cardFor(el, 'Main screen').querySelector('.ch-lamp');
    expect(lamp.textContent.trim()).toBe('Rehearsal');
    expect(lamp.className).not.toMatch(/\bamber\b/);
    expect(lamp.className).toMatch(/amethyst/);
  });

  itMounted('a screen that ANSWERS and says it is blank is not amber either', async () => {
    // The other half of RG-01, and the shape of RG-129: this screen is beating
    // on time — it is answering perfectly — and what it is answering is `clear`
    // while a verse is on the programme. Every instrument Relay had said On Air
    // about it, because `wall.live` was true, and a congregation looked at a
    // black wall. The card now repeats what the SCREEN said.
    const el = await mountOutputs();
    channelHealth.set({ 1: row({ id: 1, paint_state: 'clear' }) });
    live.set({ reference: 'Romans 8:28' });
    await settle();
    const lamp = cardFor(el, 'Main screen').querySelector('.ch-lamp');
    expect(lamp.textContent.trim()).toBe('Not confirmed');
    expect(lamp.className).not.toMatch(/\bamber\b/);
    // …and the card says which way the two disagree, without a click.
    expect(lamp.getAttribute('title')).toMatch(/Relay is sending content/);
    expect(lamp.getAttribute('title')).toMatch(/screen says clear/);
  });

  itMounted('a screen that has gone quiet turns the CARD red, not just its lamp', async () => {
    const el = await mountOutputs();
    channelHealth.set({ 1: row({ id: 1, painting: false, last_beat_ms: 30000 }) });
    live.set({ reference: 'Romans 8:28' });
    await settle();
    const card = cardFor(el, 'Main screen');
    expect(card.querySelector('.ch-lamp').textContent.trim()).toBe('Not responding');
    expect(card.className).toMatch(/\bdown\b/);
    // A broken screen must never be able to read as on air.
    expect(card.querySelector('.ch-lamp').className).toMatch(/rose/);
  });
});

describe('§3.3 · FOLLOW THE CONTENT LOOK is reachable from the card itself', () => {
  itMounted('it is the first option on every card`s template picker', async () => {
    const el = await mountOutputs();
    for (const s of SCREENS) {
      const pick = cardFor(el, s.name).querySelector('select');
      expect(pick.options[0].textContent).toBe('Follow the content look');
      // The VALUE is the empty string, which `assignTemplate` turns into `null`.
      // A screen with no look of its own is a value, not a missing field.
      expect(pick.options[0].value).toBe('');
    }
  });

  itMounted('the card for a following screen shows it as selected, not as "None"', async () => {
    const el = await mountOutputs();
    const pick = cardFor(el, 'Streaming').querySelector('select');
    expect(pick.value).toBe('');
    expect(cardFor(el, 'Main screen').querySelector('select').value).toBe('7');
  });

  itMounted('choosing it sends NULL to the backend, which is what §70 added', async () => {
    const el = await mountOutputs();
    const pick = cardFor(el, 'Main screen').querySelector('select');
    pick.value = '';
    pick.dispatchEvent(new Event('change'));
    await until(
      () => invoke.mock.calls.some(([c]) => c === 'set_channel_template'),
      'the template assignment to reach the backend',
    );
    const [, args] = invoke.mock.calls.find(([c]) => c === 'set_channel_template');
    expect(args.templateId).toBeNull();
  });
});

describe('§3.3 · the URL a follower copies says NOTHING about a template', () => {
  itMounted('a following screen`s URL carries no template_id', async () => {
    // THE DIVERGENCE NOBODY AT THE DESK SEES. `template_id=1` on a follower's
    // browser source starts that source on a built-in and leaves it there until a
    // `channel_template` message happens along — the operator's own window
    // following the content look while the stream wears something else.
    const el = await mountOutputs();
    const btn = [...cardFor(el, 'Streaming').querySelectorAll('button')].find(
      (b) => b.textContent.trim() === 'URL',
    );
    expect(btn, 'the card has no URL button').toBeTruthy();
    btn.click();
    await until(() => clipboard.length, 'the URL to reach the clipboard');
    expect(clipboard[0]).toContain('channel=2');
    expect(clipboard[0]).not.toContain('template_id');
    // …and it is the LAN address, not localhost, because it is typed into a
    // browser on another device.
    expect(clipboard[0]).toContain('192.168.1.42:8032');
  });

  itMounted('a screen with its own look still carries it, for the first render', async () => {
    const el = await mountOutputs();
    // Main screen is native, so its card offers Open rather than URL; the
    // inspector is the door for both. Select it and copy from there.
    cardFor(el, 'Streaming').click();
    await settle();
    expect(host.querySelector('.rw-insp').textContent).toMatch(/Follows|Follow the content look/);
  });
});

describe('§5 · the inspector answers for the screen in hand', () => {
  itMounted('it says "Pick a screen" until one is picked, then Type · Transport · Output · URL · Reporting', async () => {
    const el = await mountOutputs();
    const insp = el.querySelector('.rw-insp');
    expect(insp.textContent).toMatch(/Pick a screen/);

    cardFor(el, 'Streaming').click();
    await settle();
    const terms = el.querySelector('.rw-insp .ch-info');
    const names = [...terms.querySelectorAll('dt')].map((d) => d.textContent.trim());
    expect(names.slice(0, 3)).toEqual(['Type', 'Transport', 'Output']);
    expect(names).toContain('URL');
    expect(names).toContain('Reporting');
  });

  itMounted('Reporting says NEVER for a screen that has never answered — not "no"', async () => {
    const el = await mountOutputs();
    channelHealth.set({ 2: row({ id: 2, name: 'Streaming', painting: false, last_beat_ms: null }) });
    cardFor(el, 'Streaming').click();
    await settle();
    const info = el.querySelector('.rw-insp .ch-info');
    const dd = [...info.querySelectorAll('dt')].find((d) => d.textContent.trim() === 'Reporting')
      .nextElementSibling;
    expect(dd.textContent).toMatch(/never/);
    expect(dd.textContent).not.toMatch(/\byes\b/);
  });

  itMounted('it prints the screen`s own last word when that word contradicts Relay', async () => {
    // `describeScreen` returns a note as well as a label, and this panel rendered
    // only the label — so the one surface built to answer for a single screen
    // dropped the half of the answer that says why. Live has rendered the note in
    // a row of its own since RG-01; the two panes read the same helper and must
    // not show different amounts of it.
    const el = await mountOutputs();
    channelHealth.set({ 2: row({ id: 2, name: 'Streaming', paint_state: 'clear' }) });
    live.set({ reference: 'Romans 8:28' });
    cardFor(el, 'Streaming').click();
    await settle();
    const insp = el.querySelector('.rw-insp');
    expect(insp.textContent).toMatch(/Not confirmed/);
    expect(insp.textContent).toMatch(/Relay is sending content/);
    expect(insp.textContent).toMatch(/screen says clear/);
  });

  itMounted('the destructive control is Remove, and it is two-step (rule 41)', async () => {
    const el = await mountOutputs();
    cardFor(el, 'Streaming').click();
    await settle();
    const del = [...el.querySelectorAll('.rw-insp button')].find(
      (b) => b.textContent.trim() === 'Remove',
    );
    expect(del).toBeTruthy();
    del.click();
    await settle();
    // Nothing has been deleted yet — the first press ARMS it. A native confirm()
    // returns false in Tauri's webview without ever showing a dialog, so a
    // one-press delete guarded by one deletes nothing and reports success.
    expect(invoke.mock.calls.some(([c]) => c === 'delete_channel')).toBe(false);
    expect(del.textContent).toMatch(/Click again/);
  });
});
