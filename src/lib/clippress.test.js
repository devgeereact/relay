// ── DOES PRESSING A CLIP CONTROL REACH THE ENGINE? (RG-281) ─────────────────
//
// The operator, for the third time: *"media still not working... controlls not
// functioning still"*. Every instrument Relay had answered a different question.
// `clipbar.test.js` mounts the strip and asserts the buttons, the scrub and the
// figures are THERE; `mediacontrols.test.js` asserts there is exactly one set of
// them; `mediatransport.test.js` asserts the rule a player applies once a frame
// arrives. Not one of them presses a button and watches for the command.
//
// That gap is the shape this repository keeps finding: a guarantee kept on the
// doors somebody checked. A control that renders, is named, is unique and is
// correctly styled is still a control that does nothing if its handler never
// reaches `set_media_transport` — and "it does nothing" is exactly the report.
//
// So this file is the missing half: mount the real component, dispatch a real
// click, and assert on the ARGUMENTS that crossed the bridge.
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';

/**
 * LET THE BRIDGE FINISH ARRIVING.
 *
 * `capture.js` reaches the engine through a DYNAMIC import of the Tauri core,
 * resolved once and cached. So the first press of a run pays for a module load
 * that every later press does not, and a fixed pair of `tick()`s asserts on a
 * command that has not been sent yet - a test that passes or fails by its
 * position in the file. Waiting on the condition rather than on a count of
 * turns is the only form of this that cannot drift.
 */
const flush = async () => {
  for (let i = 0; i < 20; i += 1) {
    await Promise.resolve();
    await tick();
  }
};

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const ClipBar = (await import('./ClipBar.svelte')).default;
const { live, channelHealth, mediaTransport, screenBlack } = await import('./stores/capture.js');

// The frame `set_media_transport` really returns, because `setMediaTransport`
// takes it whole (RG-260) and a test that returns `null` would assert nothing
// about the store the controls read their own state back from.
const frameFor = (args) => ({
  paused: args.paused ?? false,
  looping: args.looping ?? false,
  replayEpoch: args.replay ? 1 : 0,
  seekEpoch: args.seekMs == null ? 0 : 1,
  seekMs: args.seekMs ?? 0,
  volume: args.volume ?? 1,
  startedAt: null,
});

let app;
let host;

async function mount() {
  live.set({ media_url: 'http://host:8032/media/7', kind: 'media' });
  screenBlack.set(false);
  mediaTransport.set({ paused: false, loop: false, volume: 1 });
  channelHealth.set({
    1: {
      online: true,
      painting: true,
      supported: true,
      last_beat_ms: 0,
      name: 'Main screen',
      media: { pos_ms: 60_000, dur_ms: 120_000, paused: false },
    },
  });
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new ClipBar({ target: host });
  await tick();
  await tick();
  return host;
}

/** The transport calls only — `listOutputChannels` runs on mount and is noise here. */
const transportCalls = () => invoke.mock.calls.filter((c) => c[0] === 'set_media_transport');

describe('a press on a clip control reaches the engine', () => {
  beforeEach(() => {
    invoke.mockReset();
    invoke.mockImplementation((cmd, args) => {
      if (cmd === 'set_media_transport') return Promise.resolve(frameFor(args ?? {}));
      if (cmd === 'list_output_channels') return Promise.resolve([]);
      return Promise.resolve(null);
    });
  });

  afterEach(() => {
    app?.$destroy();
    host?.remove();
    app = null;
    host = null;
    live.set(null);
  });

  it('Hold sends a pause, and nothing else with it', async () => {
    await mount();
    const btn = host.querySelector('button[aria-label*="Hold the clip"]');
    expect(btn, 'no Hold control to press').toBeTruthy();
    btn.click();
    await flush();
    const calls = transportCalls();
    expect(calls.length, 'the press never reached the engine').toBe(1);
    // RE-AIM, NOT REPLACE. Each field omitted is left alone, so Pause may not
    // un-loop and may not reset the room's level — the bug this transport was
    // shaped to avoid, and the one an operator would read as "it does nothing"
    // about the OTHER control.
    expect(calls[0][1].paused).toBe(true);
    expect(calls[0][1].looping).toBeNull();
    expect(calls[0][1].volume).toBeNull();
    expect(calls[0][1].seekMs).toBeNull();
  });

  it('and the icon then reads back from the frame the engine returned', async () => {
    await mount();
    host.querySelector('button[aria-label*="Hold the clip"]').click();
    await flush();
    // The control's own state comes from the store the command wrote, never from
    // an optimistic local flag: a Pause that failed must leave the button saying
    // the clip is running, because it is (rule 35).
    const btn = host.querySelector('button[aria-label*="Let the clip run"]');
    expect(btn, 'the control never took the engine’s answer').toBeTruthy();
    expect(btn.getAttribute('aria-pressed')).toBe('true');
  });

  it('Start again sends a replay', async () => {
    await mount();
    host.querySelector('button[aria-label*="Start the clip again"]').click();
    await flush();
    const calls = transportCalls();
    expect(calls.length, 'Replay never reached the engine').toBe(1);
    expect(calls[0][1].replay).toBe(true);
    expect(calls[0][1].paused).toBeNull();
  });

  it('Repeat sends a loop', async () => {
    await mount();
    host.querySelector('button[aria-label*="Repeat the clip"]').click();
    await flush();
    const calls = transportCalls();
    expect(calls.length, 'Loop never reached the engine').toBe(1);
    expect(calls[0][1].looping).toBe(true);
    expect(calls[0][1].paused).toBeNull();
  });

  it('and a drop on the scrub sends where the handle was let go, in whole ms', async () => {
    await mount();
    const scrub = host.querySelector('input[type="range"]');
    expect(scrub).toBeTruthy();
    scrub.value = '42500.6';
    scrub.dispatchEvent(new Event('change', { bubbles: true }));
    await flush();
    const calls = transportCalls();
    expect(calls.length, 'the drop never reached the engine').toBe(1);
    expect(calls[0][1].seekMs).toBe(42_501);
    expect(calls[0][1].paused).toBeNull();
  });

  it('a control that could not be obeyed says so, and claims nothing', async () => {
    await mount();
    invoke.mockImplementation((cmd) =>
      cmd === 'set_media_transport'
        ? Promise.reject('the clip is gone')
        : Promise.resolve([]),
    );
    host.querySelector('button[aria-label*="Hold the clip"]').click();
    await flush();
    // The strip prints the reason. And the button must NOT have flipped: a
    // transport that draws itself paused over a running clip is the same lie
    // rule 15 forbids of a panic control, one surface along.
    expect(host.textContent.toLowerCase()).toContain('clip is gone');
    expect(host.querySelector('button[aria-label*="Hold the clip"]')).toBeTruthy();
  });
});
