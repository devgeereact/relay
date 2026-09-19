// THE LINK THE OPERATOR HANDS THE PREACHER NAMES A SCREEN.
//
// `stage.html` now refuses a Stage Message unless its own channel holds the
// `stage` role (`stagepageidentity.test.js`). That closes an accident on the
// receiving page and opens a question on the sending desk: the address Outputs →
// Sharing prints is the one a church actually types into a phone, and a bare
// `stage.html` is now a page that will never be handed a Stage Message.
//
// So the desk hands out the screen, not the page. `stageRemoteUrl` is pure and
// lives beside the only other reader of `role`, and the panel says plainly when
// no screen holds the role rather than printing an address that half works —
// which is rule 35 on the one surface an operator uses once a week and never
// looks at again.
//
//   npx vitest run src/lib/stageremote.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';
import { stageRemoteUrl } from './channelroles.js';
const qr = vi.hoisted(() => ({ toDataURL: vi.fn() }));
vi.mock('qrcode', () => ({ default: qr }));

describe('stageRemoteUrl — the address, and what it does when nothing is a stage', () => {
  const SCREENS = [
    { id: 1, name: 'Main screen', role: 'main' },
    { id: 2, name: 'Stage display', role: 'stage' },
    { id: 4, name: 'Lobby screen', role: null },
  ];

  it('names the screen that holds the stage role', () => {
    const r = stageRemoteUrl('192.168.1.42', SCREENS);
    expect(r.url).toBe('http://192.168.1.42:8032/stage.html?channel=2');
    expect(r.channel.name).toBe('Stage display');
  });

  it('takes the FIRST stage when a church has several', () => {
    // A confidence monitor and a preacher's tablet may both be stages
    // (DECISIONS §89). The panel hands out one address and says which screen it
    // is, so an operator setting up the second one knows to change the number
    // rather than assuming the link is wrong.
    const two = [...SCREENS, { id: 9, name: 'Confidence monitor', role: 'stage' }];
    expect(stageRemoteUrl('10.0.0.5', two).url).toContain('channel=2');
    expect(stageRemoteUrl('10.0.0.5', two).others).toEqual(['Confidence monitor']);
  });

  it('addresses the chosen stage and refuses a removed selection rather than choosing another', () => {
    const two = [...SCREENS, { id: 9, name: 'Second tablet', role: 'stage' }];
    expect(stageRemoteUrl('10.0.0.5', two, 9).url).toBe('http://10.0.0.5:8032/stage.html?channel=9');
    expect(stageRemoteUrl('10.0.0.5', SCREENS, 9).url).toBeNull();
  });

  it('answers with NO url when no screen is a stage, rather than a bare page', () => {
    // The refusal that matters. A bare `stage.html` still renders the reading, so
    // an address that "works" is exactly the shape that would be handed out and
    // then quietly never receive a Stage Message all service.
    const none = SCREENS.filter((c) => c.role !== 'stage');
    const r = stageRemoteUrl('192.168.1.42', none);
    expect(r.url).toBeNull();
    expect(r.channel).toBeNull();
  });

  it('answers with no url before the screens have loaded', () => {
    // A list that has not arrived is not a church with no stage screen. Both
    // produce no address; only the second is worth telling anybody about, and
    // the caller has the list to tell them apart.
    expect(stageRemoteUrl('192.168.1.42', []).url).toBeNull();
    expect(stageRemoteUrl('192.168.1.42', null).url).toBeNull();
  });

  it.each([null, undefined, '', 'localhost', '127.0.0.1', '0.0.0.0', '::1'])('never hands a phone an unavailable or loopback address: %s', (address) => {
    const remote = stageRemoteUrl(address, SCREENS);
    expect(remote.url).toBeNull();
    expect(remote.channel.id).toBe(2);
  });
});

// ── AND THE DESK ACTUALLY RENDERS IT ────────────────────────────────────────
//
// The helper being right proves nothing about the address on the screen; this
// file's own subject is a string an operator copies. Mounted, because the URL is
// reactive over `local_ip` and a previous version of this very expression
// rendered once, before the IP resolved, and never again.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture, channelHealth, live, rehearsing, screenBlack, stopChannelHealth } =
  await import('./stores/capture.js');

const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

let host;
let app;
let screens;
let lanAddress;
let interfaceAddresses;
let networkFailure;

beforeEach(() => {
  lanAddress = '192.168.1.42';
  interfaceAddresses = [];
  networkFailure = false;
  qr.toDataURL.mockReset().mockResolvedValue('data:image/png;base64,cXI=');
  screens = [
    { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 7, display_target: null, status: 'offline', role: 'main' },
    { id: 2, name: 'Stage display', render_target: 'network_client', template_id: null, display_target: null, status: 'offline', role: 'stage' },
  ];
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_output_channels':
        return Promise.resolve(screens.map((s) => ({ ...s })));
      case 'list_templates':
        return Promise.resolve([{ id: 7, name: 'Classic Serif', layout: { layers: [] }, style: {} }]);
      case 'list_monitors':
        return Promise.resolve([]);
      case 'local_ip':
        return Promise.resolve(lanAddress);
      case 'network_addresses':
        return networkFailure ? Promise.reject(new Error('interface read failed')) : Promise.resolve(interfaceAddresses);
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
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
});

async function settle() {
  await new Promise((r) => setTimeout(r, 0));
  await tick();
}

async function sharing() {
  const Channels = (await import('./views/Channels.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Channels({ target: host });
  for (let i = 0; i < 60; i += 1) await settle();
  const tab = [...host.querySelectorAll('button')].find(
    (b) => b.textContent.trim() === 'Sharing',
  );
  expect(tab, 'no Sharing tab on the Outputs desk').toBeTruthy();
  tab.click();
  for (let i = 0; i < 20; i += 1) await settle();
  return host;
}

describe('Outputs → Sharing prints the address the preacher can actually be sent to', () => {
  itMounted('selects the second stage and replaces its QR and copied link together', async () => {
    screens.push({ ...screens[1], id: 9, name: 'Second tablet' });
    const copy = vi.fn().mockResolvedValue(undefined);
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { writeText: copy } });
    const el = await sharing();
    [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Show QR').click();
    await settle();
    const select = el.querySelector('#stage-device');
    select.value = '9'; select.dispatchEvent(new Event('change')); await settle();
    expect(el.querySelector('.ch-stage-qr')).toBeNull();
    expect(el.textContent).toContain('stage.html?channel=9');
    [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Copy link').click();
    await settle();
    expect(copy).toHaveBeenCalledWith('http://192.168.1.42:8032/stage.html?channel=9');
    [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Show QR').click();
    await settle();
    expect(qr.toDataURL).toHaveBeenLastCalledWith('http://192.168.1.42:8032/stage.html?channel=9', expect.any(Object));
  });

  itMounted('uses an interface on an offline LAN and keeps a chosen address through refresh', async () => {
    lanAddress = null;
    interfaceAddresses = [{ interface: 'Ethernet', address: '10.0.0.8' }, { interface: 'Wi-Fi', address: '192.168.1.42' }];
    const el = await sharing();
    expect(el.textContent).toContain('http://10.0.0.8:8032/stage.html?channel=2');
    const select = el.querySelector('#stage-network');
    select.value = '192.168.1.42'; select.dispatchEvent(new Event('change')); await settle();
    lanAddress = '10.0.0.8';
    const refresh = () => [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Refresh addresses').click();
    refresh(); await settle();
    expect(el.textContent).toContain('http://192.168.1.42:8032/stage.html?channel=2');
    interfaceAddresses = [interfaceAddresses[0]];
    refresh(); await settle();
    expect(el.textContent).not.toContain('http://10.0.0.8:8032/stage.html');
    expect(el.textContent).toContain('could not find a local network address');
  });

  itMounted('invalidates an old QR when refreshing the address and reports a failed refresh', async () => {
    const el = await sharing();
    [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Show QR').click(); await settle();
    lanAddress = '10.0.0.9';
    const refresh = () => [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Refresh addresses').click();
    refresh(); await settle();
    expect(el.querySelector('.ch-stage-qr')).toBeNull();
    expect(el.textContent).toContain('http://10.0.0.9:8032/stage.html?channel=2');
    networkFailure = true;
    refresh(); await settle();
    expect(el.textContent).toContain('Could not refresh local network addresses');
    expect(el.textContent).not.toContain('http://10.0.0.9:8032/stage.html');
  });

  itMounted('explains a missing network without offering a phone a localhost link', async () => {
    lanAddress = null;
    const el = await sharing();
    expect(el.textContent).toContain('could not find a local network address');
    expect(el.textContent).not.toContain('http://localhost:8032/stage.html');
    expect(el.textContent).not.toContain('No screen is set as a stage display');
    expect([...el.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Show QR')).toBe(false);
  });

  itMounted('renders the stage QR at its generated size with a four-module quiet zone', async () => {
    const el = await sharing();
    [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Show QR').click();
    await settle();
    expect(qr.toDataURL).toHaveBeenCalledWith('http://192.168.1.42:8032/stage.html?channel=2', expect.objectContaining({ margin: 4, width: 240 }));
    const img = el.querySelector('.ch-stage-qr');
    expect(img?.width).toBe(240);
    expect(img?.height).toBe(240);
  });

  itMounted('reports a QR generation failure beside the available copy-link action', async () => {
    qr.toDataURL.mockRejectedValue(new Error('Canvas unavailable'));
    const el = await sharing();
    [...el.querySelectorAll('button')].find((b) => b.textContent.trim() === 'Show QR').click();
    await settle();
    expect(el.querySelector('[role="status"]')?.textContent).toContain('Could not create the QR code');
    expect(el.querySelector('.ch-stage-qr')).toBeNull();
    expect([...el.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Copy link')).toBe(true);
  });

  itMounted('prints the channel-keyed address', async () => {
    const el = await sharing();
    expect(el.textContent).toContain('http://192.168.1.42:8032/stage.html?channel=2');
  });

  itMounted('says so when no screen holds the stage role', async () => {
    // Watched to fail against a panel that prints the bare page regardless: the
    // address renders, nothing warns, and the Stage Message silently never
    // arrives for the whole service.
    screens = screens.map((s) => ({ ...s, role: s.role === 'stage' ? null : s.role }));
    const el = await sharing();
    expect(el.textContent).not.toContain('stage.html?channel=');
    expect(el.textContent).toContain('No screen is set as a stage display');
  });
});

// ── AND THE SAME GUARANTEE ON THE OTHER DOOR ────────────────────────────────
//
// `showStageQr` refuses a loopback address because, scanned on a phone,
// `localhost` names the phone. The Screens inspector's **Show QR** is the same
// affordance for the same reason and had no such guard: `obsUrl` is built from
// the same `lanIp`, which still defaults to `'localhost'` in three branches of
// `refreshNetwork`. That is the "guarantee kept on one door" shape this
// repository has now hit five times — and it was reintroduced by the very
// change that fixed the stage door.
//
// The fix is deliberately NOT to blank the output URL. A loopback output
// address is genuinely correct for OBS running on this same computer, which is
// the common case for that link. What cannot be true is a QR CODE of it: a QR
// exists to be photographed by a second device. So the URL and Copy URL stay,
// and the QR refuses and says why.
describe('the general-output QR is a second device by definition, so it refuses a loopback host', () => {
  async function screensInspector() {
    const Channels = (await import('./views/Channels.svelte')).default;
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Channels({ target: host });
    for (let i = 0; i < 60; i += 1) await settle();
    const row = [...host.querySelectorAll('button, [role="button"]')].find((b) =>
      b.textContent.includes('Stage display'),
    );
    expect(row, 'no screen row to open the inspector with').toBeTruthy();
    row.click();
    for (let i = 0; i < 20; i += 1) await settle();
    return host;
  }

  const qrButton = (el) =>
    [...el.querySelectorAll('button')].find((b) => /^(Show|Hide) QR$/.test(b.textContent.trim()));

  itMounted('will not photograph a localhost output address onto a phone', async () => {
    lanAddress = null;              // no route to 8.8.8.8
    interfaceAddresses = [];        // and no usable interface either
    const el = await screensInspector();
    qrButton(el)?.click();
    await settle();
    expect(qr.toDataURL).not.toHaveBeenCalled();
    expect(el.querySelector('.ch-qr-img')).toBeNull();
    expect(el.textContent).toContain('local network address');
  });

  itMounted('still offers the address itself, because OBS on this computer is a real caller', async () => {
    lanAddress = null;
    interfaceAddresses = [];
    const el = await screensInspector();
    expect([...el.querySelectorAll('button')].some((b) => b.textContent.trim() === 'Copy URL')).toBe(true);
    expect(el.textContent).toContain('localhost:8032/output.html');
  });

  itMounted('photographs a real LAN address exactly as the stage door does', async () => {
    lanAddress = '192.168.1.42';
    const el = await screensInspector();
    qrButton(el)?.click();
    await settle();
    expect(qr.toDataURL).toHaveBeenCalledWith(
      expect.stringContaining('http://192.168.1.42:8032/output.html?channel=2'),
      expect.objectContaining({ margin: 4, width: 240 }),
    );
  });
});
