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

// ── A REMEMBERED DISPLAY THAT IS GONE ───────────────────────────────────────
//
// `display_target` is an INDEX into the OS monitor list, so unplugging a dock
// renumbers it and a screen configured for the projector becomes a screen
// configured for whatever is now in that slot — or for nothing.
//
// A STABLE IDENTITY IS NOT AVAILABLE and is deliberately not faked: Tauri 2.11
// hands out `Monitor { name, size, position, work_area, scale_factor }` and no
// native id, `tao` names a Windows monitor by the `\\.\DISPLAY1` device path the
// OS renumbers, and a macOS one by its EDID MODEL number, which two identical
// projectors share. The reasoning is in `docs/DECISIONS.md` and in
// `main.rs::resolve_display`.
//
// What IS fixed is the fallback. The desk said **Primary display** for a screen
// whose assigned display is missing AND for a screen with none assigned — one
// reassuring sentence over two situations (rule 35), on the control that decides
// which physical screen a congregation sees.
describe('a screen configured for a display that is not connected says so', () => {
  itMounted('does not read the same as a screen with no display assigned', async () => {
    const Channels = (await import('./views/Channels.svelte')).default;
    invoke.mockImplementation((cmd) => {
      switch (cmd) {
        case 'list_output_channels':
          return Promise.resolve([
            { id: 1, name: 'Projector', render_target: 'native_window', template_id: 7, display_target: '2', status: 'offline', role: 'main' },
            { id: 2, name: 'Spare', render_target: 'native_window', template_id: 7, display_target: null, status: 'offline', role: null },
          ]);
        case 'list_templates':
          return Promise.resolve([{ id: 7, name: 'Classic Serif', layout: { layers: [] }, style: {} }]);
        case 'list_monitors':
          // Only the built-in screen is here. The projector was unplugged with
          // the dock, which is the whole scenario.
          return Promise.resolve([
            { index: 0, name: 'Built-in display', width: 1512, height: 982, primary: true },
          ]);
        case 'local_ip':
          return Promise.resolve('192.168.1.42');
        case 'channel_status':
          return Promise.resolve([]);
        default:
          return Promise.resolve(null);
      }
    });
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Channels({ target: host });
    for (let i = 0; i < 60; i += 1) await settle();

    const card = (name) =>
      [...host.querySelectorAll('.ch-card')].find((c) => c.textContent.includes(name));
    expect(card('Projector').textContent).toContain('Display 3 — not connected');
    // And the control case, which is what stops this passing by the label being
    // wrong in both directions: a screen that never named a display still reads
    // as the primary, because that is genuinely what it will open on.
    // Since RG-188 the empty option names the STATE, not a destination: with one
    // monitor it reads "This display"; with two, "Choose a display…" (and Turn on
    // is withheld until one is chosen). Either way it is not the missing-display line.
    expect(card('Spare').textContent).toContain('This display');
    expect(card('Spare').textContent).not.toContain('not connected');
  });
});

// 2026-09-21 · OU-2 (RG-191). With the LAN server dead, Outputs still printed
// `:8032 · http` as a standing fact and offered Copy URL. The fact is in the
// store; this desk now reads it.
describe('Outputs says when the LAN server is not running', () => {
  itMounted('names the failure at the head of the screens list', async () => {
    const Channels = (await import('./views/Channels.svelte')).default;
    capture.update((s) => ({ ...s, outputError: 'Address already in use (os error 48)' }));
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Channels({ target: host });
    for (let i = 0; i < 60; i += 1) await settle();
    const txt = host.textContent.replace(/\s+/g, ' ');
    expect(txt).toMatch(/not running|cannot connect|failed to start/i);
    expect(txt).toMatch(/8032/);
    capture.update((s) => ({ ...s, outputError: null }));
  });
});
