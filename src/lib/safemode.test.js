import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

// SAFE MODE IS A PROMISE, NOT A LABEL.
//
// The switch's own row says "detection is disarmed — nothing Relay does can
// reach a screen". Before this test, setSafeMode patched a localStorage record
// and nothing else: App.svelte honoured it inside onMount only, Live.svelte
// never mentioned it, and Rust had no notion of it at all. The switch flipped,
// aria-checked flipped, and a live detector stayed armed over open projector
// windows until the next launch.
//
// A control that reports a success it did not achieve is rule 15's failure in
// a different costume, and this one makes a bigger promise than a panic key.

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

describe('safe mode keeps its own promise', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockResolvedValue(true);
  });

  it('disarms detection and closes every open screen when turned ON', async () => {
    const { applySafeMode } = await import('./stores/capture.js');
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') {
        return Promise.resolve([
          { id: 1, name: 'Main screen', render_target: 'native_window' },
          { id: 2, name: 'Lobby', render_target: 'native_window' },
        ]);
      }
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(true);
    const cmds = invoke.mock.calls.map((c) => c[0]);
    // `set_detection_enabled` is the command `setDetection` actually invokes
    // (capture.js). The brief said `set_detection`, which is the name of the
    // WRAPPER — and a contract test written against a command string that does
    // not exist passes whatever the wrapper does, which is the class of bug
    // `ipc.test.js` exists for.
    expect(cmds).toContain('set_detection_enabled');
    expect(
      invoke.mock.calls.find((c) => c[0] === 'set_detection_enabled')[1],
    ).toMatchObject({ enabled: false });
    expect(cmds.filter((c) => c === 'close_channel_output')).toHaveLength(2);
  });

  it('reports failure and does not claim a success it did not achieve', async () => {
    const { applySafeMode, safeModeError } = await import('./stores/capture.js');
    safeModeError.set(null);
    invoke.mockImplementation((cmd) => {
      if (cmd === 'set_detection_enabled')
        return Promise.reject(new Error('audio lock poisoned'));
      if (cmd === 'list_output_channels') return Promise.resolve([]);
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(false);
    expect(get(safeModeError)).toBeTruthy();
  });

  it('attempts every screen rather than stopping at the first that fails', async () => {
    // A half-closed set of screens is worse than a fully-reported one: the
    // operator reads one failure, fixes that screen by hand, and the two behind
    // it are still showing scripture that nothing ever tried to take down. So
    // the loop keeps going and the message names each one.
    const { applySafeMode, safeModeError } = await import('./stores/capture.js');
    safeModeError.set(null);
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'list_output_channels') {
        return Promise.resolve([
          { id: 1, name: 'Main screen' },
          { id: 2, name: 'Lobby' },
          { id: 3, name: 'Crèche' },
        ]);
      }
      if (cmd === 'close_channel_output' && args?.channelId === 1) {
        return Promise.reject(new Error('window handle gone'));
      }
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(false);
    // All three were tried, not just the one before the failure.
    expect(
      invoke.mock.calls.filter((c) => c[0] === 'close_channel_output'),
    ).toHaveLength(3);
    expect(get(safeModeError)).toContain('Main screen');
  });

  it('says so when it could not even ask which screens are open', async () => {
    // `listOutputChannels` is a GROUP 2 read: it swallows and returns `[]`. A
    // door that looped over that empty list and returned true would report
    // "every screen closed" on the strength of never having been told about
    // one. `readErrors` is where the swallowed reason goes, so that is what is
    // consulted here.
    const { applySafeMode, safeModeError } = await import('./stores/capture.js');
    safeModeError.set(null);
    invoke.mockImplementation((cmd) => {
      if (cmd === 'list_output_channels') return Promise.reject(new Error('db locked'));
      return Promise.resolve(true);
    });

    const ok = await applySafeMode(true);

    expect(ok).toBe(false);
    expect(get(safeModeError)).toBeTruthy();
  });

  it('turning safe mode OFF does not re-arm anything by itself', async () => {
    // Coming out of safe mode restores the operator's freedom to arm things; it
    // must not arm them FOR them. A detector that switches itself back on is a
    // different surprise from the one this control exists to prevent.
    const { applySafeMode } = await import('./stores/capture.js');
    invoke.mockImplementation((cmd) =>
      cmd === 'list_output_channels' ? Promise.resolve([]) : Promise.resolve(true),
    );

    await applySafeMode(false);

    const armed = invoke.mock.calls.filter(
      (c) => c[0] === 'set_detection_enabled' && c[1]?.enabled === true,
    );
    expect(armed).toHaveLength(0);
  });

  it('setSafeMode has exactly one caller, and it is applySafeMode', async () => {
    // THE CHOKE POINT IS WHERE THE CHECK GOES, NOT THE CALL SITES (rule 36).
    // The record write must not be reachable without the enforcement beside it,
    // or the next surface to flip safe mode reproduces the original defect.
    const { readFileSync, readdirSync, statSync } = await import('node:fs');
    const { join, resolve, relative } = await import('node:path');
    // `new URL('..', import.meta.url).pathname` is the obvious spelling and it
    // does not survive vite-node here — Wave 0 hit it, and `transport.test.js`
    // already carries this one with the reason beside it. The suite runs from
    // the repo root, so the root is resolved from cwd.
    const root = resolve(process.cwd(), 'src');

    const files = [];
    const walk = (d) => {
      for (const e of readdirSync(d)) {
        const p = join(d, e);
        if (statSync(p).isDirectory()) walk(p);
        else if (/\.(js|svelte)$/.test(e) && !e.endsWith('.test.js')) files.push(p);
      }
    };
    walk(root);
    // The scanner must actually be able to see the tree it claims to cover. A
    // walk that quietly found nothing would pass this test for ever.
    expect(files.length).toBeGreaterThan(50);

    const callers = files.filter((f) => {
      if (f.endsWith('boot/boot.js')) return false; // the definition
      return /\bsetSafeMode\s*\(/.test(readFileSync(f, 'utf8'));
    });

    expect(
      callers.map((f) => relative(root, f)),
      'setSafeMode must be reached only through applySafeMode',
    ).toEqual(['lib/stores/capture.js']);
  });
});
