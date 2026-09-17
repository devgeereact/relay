// A SCREEN CAN BE RENAMED, AND THE FIELD DOES NOT LIE ABOUT IT.
//
// There was no `rename_channel` in Relay at all, and the Outputs inspector said so
// at the line: "NAME is READ-ONLY, and the prototype's editable field is not built
// … an input here would take an operator's typing and drop it, which is precisely
// the defect DECISIONS §69 closed seven of." That reasoning was right; the answer
// was to build the command rather than to keep the field read-only. The name is
// the only handle anybody in the building has on a screen — the cards, the badge,
// the shell's degraded banner and the service timeline all say it — and a church
// that hangs the seeded `Lobby screen` in the crèche had no way to say so.
//
// Three claims, and the last two are the ones a rename field usually gets wrong:
//
//   1. the new name reaches the backend, once, on blur;
//   2. a name that did not change is not a write at all;
//   3. `Escape` abandons the edit and does NOT reach the shell — rule 44 in its
//      smallest form. Nothing on this desk mounts a dialog, so `shortcuts.js`
//      still holds `Esc`, and an operator giving up on a rename must not also
//      clear the congregation's screens.
//
//   npx vitest run src/lib/screenrename.test.js
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import * as svelteRuntime from 'svelte';
import { tick } from 'svelte';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { capture, channelHealth, live, rehearsing, screenBlack, stopChannelHealth } =
  await import('./stores/capture.js');

const LIFECYCLE_LIVE = /\{\s*\}$/.test(svelteRuntime.onMount.toString()) === false;
const itMounted = LIFECYCLE_LIVE ? it : it.skip;

const SCREENS = [
  { id: 1, name: 'Main screen', render_target: 'native_window', template_id: 7, display_target: null, status: 'offline', role: 'main' },
  { id: 4, name: 'Lobby screen', render_target: 'network_client', template_id: null, display_target: null, status: 'offline', role: null },
];

let host;
let app;

beforeEach(() => {
  invoke.mockReset();
  invoke.mockImplementation((cmd) => {
    switch (cmd) {
      case 'list_output_channels':
        return Promise.resolve(SCREENS.map((s) => ({ ...s })));
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

/** The Outputs desk with the lobby screen selected, and its name field. */
async function nameField() {
  const Channels = (await import('./views/Channels.svelte')).default;
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Channels({ target: host });
  for (let i = 0; i < 60; i += 1) await settle();

  const card = [...host.querySelectorAll('.ch-card')].find((c) =>
    c.textContent.includes('Lobby screen'),
  );
  expect(card, 'no card for the lobby screen').toBeTruthy();
  card.click();
  for (let i = 0; i < 20; i += 1) await settle();

  const field = host.querySelector('input[aria-label="Name for Lobby screen"]');
  expect(field, 'the inspector still renders the name as fixed text').toBeTruthy();
  return field;
}

const calls = (cmd) => invoke.mock.calls.filter((c) => c[0] === cmd);

describe('Outputs → Screens · the name is a control', () => {
  itMounted('sends the new name once, on blur, trimmed', async () => {
    const field = await nameField();
    field.value = '  Crèche  ';
    field.dispatchEvent(new Event('blur'));
    for (let i = 0; i < 20; i += 1) await settle();

    const sent = calls('rename_channel');
    expect(sent, 'the rename never reached the backend').toHaveLength(1);
    expect(sent[0][1]).toEqual({ id: 4, name: 'Crèche' });
  });

  itMounted('a name that did not change is not a write', async () => {
    // Clicking into the field and out again must not put a row through the
    // backend, the refresh and the error pane — and on a refusal would show a
    // failure the operator did nothing to cause.
    const field = await nameField();
    field.dispatchEvent(new Event('blur'));
    for (let i = 0; i < 20; i += 1) await settle();
    expect(calls('rename_channel')).toHaveLength(0);
  });

  itMounted('Escape abandons the edit and never reaches the shell', async () => {
    // RULE 44, in its smallest form. This desk mounts no dialog, so `shortcuts.js`
    // still has `Esc` and `Esc` still clears the screens — which is correct and is
    // exactly why this field has to consume the key it has just used.
    const field = await nameField();
    field.value = 'Something else';

    const escaped = new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
      cancelable: true,
    });
    let reachedShell = false;
    document.addEventListener('keydown', () => (reachedShell = true), { once: true });
    field.dispatchEvent(escaped);
    for (let i = 0; i < 20; i += 1) await settle();

    expect(field.value, 'Escape left the abandoned text in the field').toBe('Lobby screen');
    expect(calls('rename_channel'), 'Escape saved the abandoned name').toHaveLength(0);
    expect(
      reachedShell,
      'Escape reached the document — one abandoned rename would have cleared the wall',
    ).toBe(false);
  });
});
