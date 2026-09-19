// THE OPERATOR'S SIDE OF A STAGE LAYOUT (DECISIONS §103).
//
// `stagelayout.test.js` covers what the preacher's screen does with a layout.
// This is the desk that makes one: Outputs → Stage layouts.
//
// The two things worth pinning here are both about a save that would be
// invisible:
//
//   * A zone toggle does NOT write through. If it did, every tap would change
//     what a preacher is looking at while the operator was still deciding — so
//     Save is the moment it reaches a screen, and until then the editor has to
//     SAY there is something unsaved rather than leave the operator to remember.
//   * The row that was just saved is re-selected by the id the engine handed
//     back, not by guessing which row in the reloaded list is new. Two layouts
//     saved in one sitting would make that guess wrong.
//
//   npx vitest run src/lib/stagelayoutdesk.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('qrcode', () => ({ default: { toDataURL: vi.fn().mockResolvedValue('data:,') } }));

const { capture, channelHealth, live, rehearsing, screenBlack, stopChannelHealth } =
  await import('./stores/capture.js');

const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const ALL_ON = {
  reading: true, next: true, note: true,
  countdown: true, clock: true, elapsed: true, programme: true,
};

let host;
let app;
let layouts;
let screens;

beforeEach(() => {
  layouts = [
    { id: 1, name: 'Preacher', zones: { ...ALL_ON } },
    { id: 2, name: 'Timer focus', zones: { ...ALL_ON, reading: false, next: false } },
  ];
  screens = [
    { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 7, display_target: null, status: 'offline', role: 'main', stage_layout_id: null },
    { id: 2, name: 'Stage display', render_target: 'network_client', template_id: null, display_target: null, status: 'offline', role: 'stage', stage_layout_id: 2 },
  ];
  invoke.mockReset();
  invoke.mockImplementation((cmd, args) => {
    switch (cmd) {
      case 'list_output_channels': return Promise.resolve(screens.map((s) => ({ ...s })));
      case 'list_stage_layouts': return Promise.resolve(layouts.map((l) => ({ ...l })));
      case 'list_templates': return Promise.resolve([{ id: 7, name: 'Classic', layout: { layers: [] }, style: {} }]);
      case 'list_monitors': return Promise.resolve([]);
      case 'local_ip': return Promise.resolve('192.168.1.5');
      case 'network_addresses': return Promise.resolve([{ interface: 'Wi-Fi', address: '192.168.1.5' }]);
      case 'channel_status': return Promise.resolve([]);
      case 'upsert_stage_layout': {
        const id = args.id ?? 99;
        const row = { id, name: args.name.trim(), zones: args.zones };
        layouts = [...layouts.filter((l) => l.id !== id), row];
        return Promise.resolve(id);
      }
      case 'delete_stage_layout':
        layouts = layouts.filter((l) => l.id !== args.id);
        return Promise.resolve(null);
      default: return Promise.resolve(null);
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
  app?.$destroy();
  host?.remove();
  app = null; host = null;
});

const settle = async () => {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
};

async function desk() {
  const Channels = (await import('./views/Channels.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Channels({ target: host });
  for (let i = 0; i < 60; i += 1) await settle();
  const tab = [...host.querySelectorAll('button')].find(
    (b) => b.textContent.trim() === 'Stage layouts',
  );
  expect(tab, 'no Stage layouts section on the Outputs desk').toBeTruthy();
  tab.click();
  for (let i = 0; i < 20; i += 1) await settle();
  return host;
}

const called = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);
const btn = (text) =>
  [...host.querySelectorAll('button')].find((b) => b.textContent.trim() === text);
const rowFor = (name) =>
  [...host.querySelectorAll('.ch-lrow')].find((r) => r.textContent.includes(name));

describe('the stage layouts desk', () => {
  itMounted('lists the layouts and says which screens wear them', async () => {
    const el = await desk();
    expect(el.textContent).toContain('Preacher');
    expect(el.textContent).toContain('Timer focus');
    // The fact the delete refusal would otherwise be the first to mention.
    expect(rowFor('Timer focus').textContent).toContain('Stage display');
    expect(rowFor('Preacher').textContent).toContain('not in use');
  });

  itMounted('does not touch a screen while zones are being chosen', async () => {
    const el = await desk();
    rowFor('Preacher').click();
    await settle();
    btn('Reading').click();
    await settle();
    expect(called('upsert_stage_layout'), 'a toggle wrote straight through').toHaveLength(0);
    expect(el.textContent).toMatch(/not saved yet/i);
  });

  itMounted('saves the name and the zones together', async () => {
    await desk();
    rowFor('Preacher').click();
    await settle();
    btn('Reading').click();
    await settle();
    btn('Save').click();
    await settle();
    const call = called('upsert_stage_layout').at(-1);
    expect(call[1].id).toBe(1);
    expect(call[1].name).toBe('Preacher');
    expect(call[1].zones.reading).toBe(false);
  });

  itMounted('creates a new layout with no id', async () => {
    await desk();
    btn('New layout').click();
    await settle();
    const name = host.querySelector('#ch-lname');
    name.value = 'Back wall';
    name.dispatchEvent(new Event('input'));
    await settle();
    btn('Save').click();
    await settle();
    const call = called('upsert_stage_layout').at(-1);
    expect(call[1].id).toBeNull();
    expect(call[1].name).toBe('Back wall');
  });

  itMounted('will not save a layout with no name', async () => {
    await desk();
    btn('New layout').click();
    await settle();
    expect(btn('Save').disabled).toBe(true);
  });

  itMounted('warns that saving changes a screen that is already wearing it', async () => {
    const el = await desk();
    rowFor('Timer focus').click();
    await settle();
    expect(el.textContent).toMatch(/worn by/i);
    expect(el.textContent).toContain('Stage display');
  });

  itMounted('arms a delete rather than doing it on one click', async () => {
    await desk();
    rowFor('Preacher').click();
    await settle();
    btn('Delete').click();
    await settle();
    expect(called('delete_stage_layout'), 'one click deleted it').toHaveLength(0);
    btn('Click again to confirm').click();
    await settle();
    expect(called('delete_stage_layout').at(-1)[1].id).toBe(1);
  });
});
