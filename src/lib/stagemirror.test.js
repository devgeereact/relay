// THE PANEL THAT MIRRORS A SCREEN NOBODY IN THE ROOM CAN SEE.
//
// Requirement 6 asks Outputs for a Screen section showing what is currently on
// the preacher's device, with the QR in that same section — replacing the
// Sharing → Stage layouts journey.
//
// The hard half is that there is nothing to mirror. The kiosk hub records
// nothing about who connected (DECISIONS §35), a screen's beat says only that it
// is still painting, and whether the preacher's zones are switched on lives in
// `localStorage` on his own device. So the panel is a PROJECTION of what Relay
// told that screen, and every question it cannot answer has to come back as
// "not known" rather than as a confident row — rule 35, on the one surface an
// operator opens precisely because the screen is out of sight.
//
//   npx vitest run src/lib/stagemirror.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { stageMirrorZones, stageMirrorReading } from './stagemirror.js';
import { STAGE_ZONES } from './stagelayout.js';

const qr = vi.hoisted(() => ({ toDataURL: vi.fn() }));
vi.mock('qrcode', () => ({ default: qr }));

const stage = (over = {}) => ({ id: 2, name: 'Preacher tablet', role: 'stage', ...over });
const layoutOf = (zones, name = 'Preacher') => ({ id: 5, name, zones });
const stateOf = (r, key) => r.zones.find((z) => z.key === key)?.state;

describe('stageMirrorZones — four situations, four answers', () => {
  it('says it cannot tell while the screens have not been read', () => {
    const r = stageMirrorZones({ channel: null, layout: null, list: { read: false } });
    expect(r.kind).toBe('unknown');
    expect(r.headline).toContain('Cannot tell');
    expect(r.zones.every((z) => z.state === 'unknown')).toBe(true);
  });

  it('carries the REASON a read failed rather than reading as a quiet Sunday', () => {
    // An empty list from a failed read renders exactly like a church with no
    // stage screen. Only one of those is worth telling anybody about.
    const r = stageMirrorZones({ channel: null, list: { read: true, error: 'the engine is not answering' } });
    expect(r.kind).toBe('unknown');
    expect(r.headline).toContain('the engine is not answering');
    expect(r.headline).not.toContain('No screen is set');
  });

  it('names the missing role, and the place it is set, when nothing is a stage', () => {
    const r = stageMirrorZones({ channel: null, layout: null, list: { read: true } });
    expect(r.kind).toBe('norole');
    expect(r.headline).toContain('No screen is set as a Stage display');
    expect(r.headline).toContain('Screens');
    expect(r.zones.every((z) => z.state === 'unknown')).toBe(true);
  });

  it('claims NOTHING about a screen whose zones the device chooses', () => {
    // The Screens inspector already states this ("Relay cannot see what they
    // chose"). A second panel that painted seven confident rows over the same
    // screen would be one desk with two answers about one tablet.
    const r = stageMirrorZones({ channel: stage(), layout: null, list: { read: true } });
    expect(r.kind).toBe('device');
    expect(r.headline).toContain('Preacher tablet');
    expect(r.headline).toContain('cannot see what they chose');
    expect(r.zones).toHaveLength(STAGE_ZONES.length);
    expect(r.zones.every((z) => z.state === 'unknown')).toBe(true);
  });

  it('reads an assigned layout zone by zone, and says the reading is a projection', () => {
    const r = stageMirrorZones({
      channel: stage(),
      layout: layoutOf({ reading: true, next: false, note: true, countdown: false, clock: true, elapsed: false, programme: true }),
      list: { read: true },
    });
    expect(r.kind).toBe('assigned');
    expect(r.headline).toContain('Preacher');
    expect(stateOf(r, 'reading')).toBe('on');
    expect(stateOf(r, 'next')).toBe('off');
    expect(stateOf(r, 'programme')).toBe('on');
    expect(stateOf(r, 'countdown')).toBe('off');
    // NEVER "is showing" — the strongest honest verb is about what was sent.
    expect(r.note).toContain('cannot ask a screen what it painted');
    expect(r.headline).not.toContain('is showing');
  });

  it('treats a layout that names no zone as the device still deciding', () => {
    // `readStageZones({})` is null and `Stage.svelte` falls through to the
    // device's own zones for exactly that entry. Reading `{}` as "all defaults"
    // here would paint seven confident rows over a screen set by hand.
    const r = stageMirrorZones({ channel: stage(), layout: layoutOf({}), list: { read: true } });
    expect(r.kind).toBe('device');
    expect(r.zones.every((z) => z.state === 'unknown')).toBe(true);
  });

  it('carries a zone added in a later version on its own default, not as absent', () => {
    // One reader for both sources (`readStageZones`), so a layout saved before a
    // zone existed does not silently switch it off on the preacher's screen.
    const r = stageMirrorZones({ channel: stage(), layout: layoutOf({ reading: false }), list: { read: true } });
    expect(stateOf(r, 'reading')).toBe('off');
    expect(stateOf(r, 'programme')).toBe('on');
  });

  it('never prints an id for a screen with no name', () => {
    const r = stageMirrorZones({ channel: stage({ name: '   ' }), layout: null, list: { read: true } });
    expect(r.headline).toContain('Stage display');
    expect(r.headline).not.toContain('2');
  });

  it('every answer offers a row for every zone the stage page can draw', () => {
    const keys = STAGE_ZONES.map((z) => z.key);
    for (const r of [
      stageMirrorZones({ list: { read: false } }),
      stageMirrorZones({ channel: null, list: { read: true } }),
      stageMirrorZones({ channel: stage(), list: { read: true } }),
      stageMirrorZones({ channel: stage(), layout: layoutOf({ clock: false }), list: { read: true } }),
    ])
      expect(r.zones.map((z) => z.key)).toEqual(keys);
  });
});

describe('stageMirrorReading — what is on the reading zone, and when that is unknowable', () => {
  it('reports the programme when one is live and the zone is on', () => {
    const r = stageMirrorReading({ zone: 'on', live: { reference: 'John 3:16' } });
    expect(r.kind).toBe('content');
    expect(r.caveat).toBe('');
  });

  it('will not claim a verse is on a tablet whose zones it cannot see', () => {
    const r = stageMirrorReading({ zone: 'unknown', live: { reference: 'John 3:16' } });
    expect(r.kind).toBe('content');
    expect(r.caveat).toContain('device');
  });

  it('says the zone is off rather than showing the verse behind it', () => {
    const r = stageMirrorReading({ zone: 'off', live: { reference: 'John 3:16' } });
    expect(r.kind).toBe('off');
    expect(r.headline).toContain('switched off');
  });

  it('refuses to answer during a rehearsal, because a rehearsal reaches no screen', () => {
    // `$live` is the rehearsal's content and the tablet is holding whatever went
    // out before it — a thing the console never kept.
    const r = stageMirrorReading({ zone: 'on', live: { reference: 'John 3:16' }, rehearsing: true });
    expect(r.kind).toBe('unknown');
    expect(r.headline).toContain('rehearsal');
  });

  it('knows a screen the operator took down is blank whatever is on the programme', () => {
    for (const [down, word] of [['clear', 'took this screen down'], ['black', '(black)']]) {
      const r = stageMirrorReading({ zone: 'on', live: { reference: 'John 3:16' }, down });
      expect(r.kind).toBe('blank');
      expect(r.headline).toContain(word);
      expect(r.caveat).toBe('');
    }
  });

  it('a screen that is down outranks a zone it cannot see', () => {
    const r = stageMirrorReading({ zone: 'unknown', live: { reference: 'John 3:16' }, down: 'clear' });
    expect(r.kind).toBe('blank');
    expect(r.caveat).toBe('');
  });

  it('reads a blackout and a cleared wall as blank, not as unknown', () => {
    expect(stageMirrorReading({ zone: 'on', live: { reference: 'x' }, black: true }).kind).toBe('blank');
    expect(stageMirrorReading({ zone: 'on', live: null }).kind).toBe('blank');
  });
});

// ── AND THE DESK ACTUALLY RENDERS IT ────────────────────────────────────────
//
// The helpers being right proves nothing about the panel. Mounted, because the
// address is reactive over `local_ip` and this file's own subject is a section
// an operator stands in front of while wiring a screen.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture, channelHealth, channelWaiting, live, rehearsing, screenBlack, stopChannelHealth } =
  await import('./stores/capture.js');

const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

let host;
let app;
let screens;
let layouts;
let timers;
let lanAddress;

beforeEach(() => {
  lanAddress = '192.168.1.42';
  qr.toDataURL.mockReset().mockResolvedValue('data:image/png;base64,cXI=');
  screens = [
    { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 7, display_target: null, status: 'offline', role: 'main' },
    { id: 2, name: 'Preacher tablet', render_target: 'network_client', template_id: null, display_target: null, status: 'offline', role: 'stage', stage_layout_id: null },
  ];
  layouts = [{ id: 5, name: 'Preacher', zones: { reading: true, next: false, note: true, countdown: false, clock: true, elapsed: true, programme: true } }];
  timers = [];
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_output_channels':
        return Promise.resolve(screens.map((s) => ({ ...s })));
      case 'list_templates':
        return Promise.resolve([{ id: 7, name: 'Classic Serif', layout: { layers: [] }, style: {} }]);
      case 'list_monitors':
        return Promise.resolve([]);
      case 'list_stage_layouts':
        return Promise.resolve(layouts.map((l) => ({ ...l })));
      case 'list_timers':
        return Promise.resolve(timers.map((t) => ({ ...t })));
      case 'local_ip':
        return Promise.resolve(lanAddress);
      case 'network_addresses':
        return Promise.resolve([]);
      case 'channel_status':
        return Promise.resolve([]);
      default:
        return Promise.resolve(null);
    }
  });
  capture.update((c) => ({ ...c, available: true }));
  channelHealth.set({});
  channelWaiting.set({});
  live.set(null);
  rehearsing.set(false);
  screenBlack.set(false);
});

afterEach(() => {
  stopChannelHealth();
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

const byText = (el, text) =>
  [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === text);

async function preacherScreen() {
  const Channels = (await import('./views/Channels.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Channels({ target: host });
  for (let i = 0; i < 60; i += 1) await settle();
  const tab = byText(host, "Preacher's screen");
  expect(tab, "no Preacher's screen section on the Outputs desk").toBeTruthy();
  tab.click();
  for (let i = 0; i < 20; i += 1) await settle();
  return host;
}

describe("Outputs → Preacher's screen", () => {
  itMounted('shows the zones an assigned layout puts on the tablet, and says it is not a readback', async () => {
    screens[1].stage_layout_id = 5;
    const el = await preacherScreen();
    const rows = [...el.querySelectorAll('.ch-mz')];
    expect(rows.length).toBe(STAGE_ZONES.length);
    const row = (label) => rows.find((r) => r.textContent.includes(label));
    expect(row('Reading').textContent).toContain('Showing');
    expect(row('Next').textContent).toContain('Hidden');
    expect(row('Stage Timer').textContent).toContain('Showing');
    expect(el.textContent).toContain('cannot ask a screen what it painted');
  });

  itMounted('says it does not know, rather than guessing, when the device chooses', async () => {
    const el = await preacherScreen();
    expect(el.textContent).toContain('cannot see what they chose');
    const rows = [...el.querySelectorAll('.ch-mz')];
    expect(rows.length).toBe(STAGE_ZONES.length);
    expect(rows.every((r) => r.textContent.includes('Not known'))).toBe(true);
    expect(el.textContent).not.toContain('Showing');
  });

  itMounted('carries the stage QR, the address and a copy control in this same section', async () => {
    const copy = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: copy } });
    const el = await preacherScreen();
    expect(el.textContent).toContain('http://192.168.1.42:8032/stage.html?channel=2');
    byText(el, 'Copy link').click();
    await settle();
    expect(copy).toHaveBeenCalledWith('http://192.168.1.42:8032/stage.html?channel=2');
    byText(el, 'Show QR').click();
    await settle();
    expect(qr.toDataURL).toHaveBeenLastCalledWith(
      'http://192.168.1.42:8032/stage.html?channel=2',
      expect.any(Object),
    );
    expect(el.querySelector('.ch-stage-qr')).toBeTruthy();
  });

  itMounted('names the screen that holds the role and reports its health here', async () => {
    // `describeStageReach` was read only by Live, and Live is not where an
    // operator stands when they wire a screen. Before the first poll it says it
    // has not been checked, which is the answer — not a green one.
    const el = await preacherScreen();
    expect(el.textContent).toContain('Preacher tablet');
    expect(el.textContent).toContain('has not been checked yet');
    channelHealth.set({ 2: { supported: true, online: false, painting: false, last_beat_ms: null } });
    for (let i = 0; i < 6; i += 1) await settle();
    expect(el.textContent).toContain('no window');
  });

  itMounted('reports a stage screen that has never once said it is painting', async () => {
    // The trap `describeStageReach` was written for: a screen wired from the
    // OBS-shaped `Copy URL` is attached, answers, and will never draw a rail.
    // Past the beat grace window, or this is correctly still "Waiting\u2026".
    const el = await preacherScreen();
    channelWaiting.set({ 2: Date.now() - 60000 });
    channelHealth.set({ 2: { supported: true, online: true, painting: false, last_beat_ms: null } });
    for (let i = 0; i < 6; i += 1) await settle();
    expect(el.textContent).toContain('has never reported painting');
    expect(el.textContent).toContain('stage address');
  });

  itMounted('reaches the stage layout for that screen without leaving the section', async () => {
    const el = await preacherScreen();
    const pick = el.querySelector('#ch-mirror-layout');
    expect(pick, 'no stage-layout picker in the section').toBeTruthy();
    expect([...pick.options].some((o) => o.textContent.includes('Preacher'))).toBe(true);
    pick.value = '5';
    pick.dispatchEvent(new Event('change'));
    for (let i = 0; i < 10; i += 1) await settle();
    expect(invoke).toHaveBeenCalledWith('set_channel_stage_layout', { channelId: 2, layoutId: 5 });
  });

  itMounted('refuses to describe a screen when no screen holds the stage role', async () => {
    screens[1].role = null;
    const el = await preacherScreen();
    expect(el.textContent).toContain('No screen is set as a Stage display');
    expect(el.textContent).not.toContain('stage.html?channel=');
    expect(byText(el, 'Show QR')).toBeFalsy();
  });

  itMounted('will not claim a reading during a rehearsal', async () => {
    screens[1].stage_layout_id = 5;
    live.set({ reference: 'John 3:16', text: 'For God so loved the world…', kind: 'scripture' });
    const el = await preacherScreen();
    expect(el.textContent).toContain('John 3:16');
    rehearsing.set(true);
    for (let i = 0; i < 6; i += 1) await settle();
    expect(el.textContent).toContain('rehearsal reaches no screen');
    expect(el.textContent).not.toContain('For God so loved the world');
  });

  itMounted('keeps the old Sharing journey working beside the new one', async () => {
    // Binding: the operator asked for the new path to be additive until the new
    // one is verified. A section that replaced Sharing or Stage layouts would be
    // this requirement's own instruction broken.
    const el = await preacherScreen();
    expect(byText(el, 'Sharing')).toBeTruthy();
    expect(byText(el, 'Stage layouts')).toBeTruthy();
    byText(el, 'Sharing').click();
    for (let i = 0; i < 10; i += 1) await settle();
    expect(el.textContent).toContain('http://192.168.1.42:8032/stage.html?channel=2');
    expect(el.textContent).toContain("Preacher's stage remote");
  });

  itMounted('shows the Stage Timers that are actually in the registry', async () => {
    timers = [{ id: 't1', label: 'Sermon', scope: 'stage', target_ms: Date.now() + 600000, paused_ms: null }];
    screens[1].stage_layout_id = 5;
    const el = await preacherScreen();
    expect(el.textContent).toContain('Sermon');
    expect(el.querySelector('.ch-mtimer')).toBeTruthy();
  });

  itMounted('tells a quiet registry from one it has not read', async () => {
    screens[1].stage_layout_id = 5;
    const el = await preacherScreen();
    expect(el.textContent).toContain('No Stage Timer');
  });
});
