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

  it('takes the screens down, not only the windows that have one', async () => {
    // `close_channel_output` closes a native webview. A kiosk channel, an OBS
    // browser source or a lobby TV on the hub has no window, so that command is a
    // silent no-op for them and the RETAINED frame (rule 43) goes on showing the
    // last verse. `clear_screens` is the only thing that reaches every render
    // target, and its clear becomes the retained frame in turn.
    const { applySafeMode } = await import('./stores/capture.js');
    invoke.mockImplementation((cmd) =>
      cmd === 'list_output_channels'
        ? Promise.resolve([{ id: 1, name: 'Main screen' }])
        : Promise.resolve(true),
    );

    expect(await applySafeMode(true)).toBe(true);
    expect(invoke.mock.calls.map((c) => c[0])).toContain('clear_screens');
  });

  it('a clear that failed is a promise that was not kept', async () => {
    const { applySafeMode, safeModeError } = await import('./stores/capture.js');
    safeModeError.set(null);
    invoke.mockImplementation((cmd) => {
      if (cmd === 'clear_screens') return Promise.reject(new Error('output window is gone'));
      if (cmd === 'list_output_channels') return Promise.resolve([]);
      return Promise.resolve(true);
    });

    expect(await applySafeMode(true)).toBe(false);
    expect(get(safeModeError)).toContain('cleared');
  });

  it('refuses to open an output screen while safe mode is on, and says why', async () => {
    // "Outputs will not open" is an ONGOING clause, not part of the transition.
    // `Channels.svelte` does not import `safeMode` at all, so the Outputs
    // workspace's Open button opened a projector window with safe mode on — the
    // capability `degraded.js` reports as blocked. The gate is at the door, not a
    // tenth `$safeMode` check in a view, and it REFUSES rather than no-opping.
    const { openChannelOutput, autoOpenOutputs } = await import('./stores/capture.js');
    const { setSafeMode } = await import('./boot/boot.js');
    setSafeMode(true);
    try {
      await expect(openChannelOutput(3)).rejects.toMatchObject({ kind: 'refused' });
      expect(invoke.mock.calls.map((c) => c[0])).not.toContain('open_channel_output');
      // The other way a screen opens is a different backend command, so the
      // refusal above does not cover it.
      expect(await autoOpenOutputs()).toBe(null);
      expect(invoke.mock.calls.map((c) => c[0])).not.toContain('auto_open_outputs');
    } finally {
      setSafeMode(false);
    }
    // …and it opens perfectly well when safe mode is off.
    await openChannelOutput(3);
    expect(invoke.mock.calls.map((c) => c[0])).toContain('open_channel_output');
  });

  it('the shell says so too, not only the page the switch is on', async () => {
    // A SOURCE assertion, and saying which instrument saw it is the point: the
    // shell is not mounted here. The scenario it holds is the one the fix exists
    // for — flip safe mode in Settings, a screen refuses to close, walk to Live to
    // see what is still lit, and be told there that outputs are disabled.
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const app = readFileSync(resolve(process.cwd(), 'src/App.svelte'), 'utf8');
    expect(app).toMatch(/\{#if \$safeModeError\}/);
    expect(app).toContain('safeModeError: $safeModeError');
    expect(app).toContain('safeModeFailed: !!$safeModeError');
    // Rose, never amber — amber is ON AIR (rule 18). `.audiobar` is the shell's
    // existing rose in-flow banner; `.panicbar` is fixed and owns `--panic-h`.
    expect(app).not.toMatch(/\{#if \$safeModeError\}[\s\S]{0,200}amber/);
  });

  it('the shell stacks its banners as BARS — every in-flow child expects a column', async () => {
    // A BANNER LAID OUT AS A COLUMN IS A BANNER NOBODY READS.
    //
    // `.shell` was `display:flex` with no direction, which is a ROW, and the two
    // in-flow rose banners are direct children of it. So `.audiobar` — a full-width
    // bar with a `border-bottom`, whose own comment says it stacks under the panic
    // bar — was laid out BESIDE the desk as a narrow full-height strip. Measured in
    // a real browser against this stylesheet at 1200px: with the microphone banner
    // up, `.main-v` went 1200 → 730 and the banner took 470px down the right-hand
    // side; with two banners up `.main-v` measured ZERO and the console vanished,
    // because `flex:1` is `flex-basis:0` and the banners had taken the row.
    //
    // That has shipped since RG-117 added the first banner. `.panicbar` and `.prac`
    // escaped it only by being `position:fixed`.
    //
    // THIS TEST CANNOT MEASURE. jsdom does not lay out, so `getBoundingClientRect`
    // is all zeros here and a geometric assertion would be theatre. What it holds
    // instead is the two source facts the geometry follows from, which is the whole
    // of the bug: the direction, and that every direct child of `.shell` is either
    // the desk, a bar meant to stack, or out of flow entirely. The measurement is
    // in the report and in `app.css`'s `.shell` note.
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const css = readFileSync(resolve(process.cwd(), 'src/app.css'), 'utf8');
    const app = readFileSync(resolve(process.cwd(), 'src/App.svelte'), 'utf8');

    const shellRule = css.slice(css.indexOf('.shell{'), css.indexOf('}', css.indexOf('.shell{')));
    expect(shellRule, 'a shell with no flex-direction is a ROW').toMatch(/flex-direction:\s*column/);

    // Every direct child of `.shell`, by markup indentation: an element at two
    // spaces, or one at four directly under a two-space `{#if}` / `{:else if}`.
    const body = app.slice(app.indexOf('<div class="shell"'));
    const lines = body.split('\n');
    const kids = [];
    let underBlock = false;
    for (const ln of lines) {
      if (/^  \{[#:]/.test(ln)) { underBlock = true; continue; }
      if (/^  \{\/|^  <!--/.test(ln)) { underBlock = false; continue; }
      const two = ln.match(/^  <\w+ class="([^"{]+)"/);
      if (two) { kids.push(two[1]); underBlock = false; continue; }
      if (underBlock) {
        const four = ln.match(/^    <\w+ class="([^"{]+)"/);
        if (four) { kids.push(four[1]); underBlock = false; }
      }
    }
    // The scanner must see a tree. One that quietly found nothing would pass for ever.
    expect(kids.length, `direct children of .shell: ${kids.join(' | ')}`).toBeGreaterThan(5);
    expect(kids).toContain('main-v');
    expect(kids.filter((k) => k === 'audiobar')).toHaveLength(2);

    // `main-v` is the desk. `audiobar` is the sanctioned in-flow bar. Everything
    // else must take itself out of the flow, or it will sit beside the desk again —
    // and in a column it would eat the desk's HEIGHT instead of its width, which is
    // the same bug wearing the other axis.
    for (const cls of kids) {
      if (cls === 'main-v' || cls === 'audiobar') continue;
      const first = cls.split(/\s+/)[0];
      const rule = css.slice(css.indexOf(`.${first}{`), css.indexOf('}', css.indexOf(`.${first}{`)));
      expect(
        /position:\s*fixed|display:\s*none/.test(rule),
        `.${first} is a direct child of .shell and is neither out of flow nor a bar ` +
          `meant to stack. Its base rule: ${rule || '(no rule found)'}`,
      ).toBe(true);
    }
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

    // `setSafeMode(` catches a direct call and MISSES
    // `import { setSafeMode as flip }` followed by `flip(...)` — a scanner that
    // quietly narrows passes everything, which is this file's own rule. So the
    // import specifier is matched as well: a name cannot be rebound without
    // passing through `as` at the import. A prose mention in a comment is
    // followed by neither, and there are three of those in the tree.
    const callers = files.filter((f) => {
      if (f.endsWith('boot/boot.js')) return false; // the definition
      return /\bsetSafeMode\s*(?:\(|as\s)/.test(readFileSync(f, 'utf8'));
    });

    expect(
      callers.map((f) => relative(root, f)),
      'setSafeMode must be reached only through applySafeMode',
    ).toEqual(['lib/stores/capture.js']);
  });
});
