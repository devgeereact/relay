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

beforeEach(() => {
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
        return Promise.resolve('192.168.1.42');
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
