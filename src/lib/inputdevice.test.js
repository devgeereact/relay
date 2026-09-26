// The microphone a room uses must survive a relaunch, and a microphone that is
// missing must be SAID rather than silently swapped.
//
// RG-121. `capture.js` held the selected input device in memory only, so every
// launch reverted to whatever the operating system calls default and nothing
// mentioned it. On 2026-09-06 both launches of a real service began on
// `MacBook Pro Microphone` with a Blackmagic Web Presenter desk feed plugged in
// and selected in the previous session.
//
// The failure is quiet in both directions, which is what makes it expensive. A
// laptop microphone at the back of a booth does not error: it produces a
// transcript, rule 12's learned gate adapts to whatever it hears, and the first
// sign of trouble is a detection rate nobody can explain afterwards. Relay also
// keeps the room's learned noise floor across sessions (RG-10) and did not keep
// which device it learned it on.
//
// So the fix is two halves and BOTH are required: remember the device, and, when
// the remembered one is not here today, fall back to the default *and say so*.
// Falling back silently is the original bug with an extra step.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

const { chooseInputDevice, setInputDevice, capture, INPUT_DEVICE_KEY } = await import(
  './stores/capture.js'
);

const devices = [
  { name: 'MacBook Pro Microphone', is_default: true },
  { name: 'Blackmagic Web Presenter 4K', is_default: false },
];

describe('choosing the input device at launch', () => {
  it('restores a remembered device that is attached', () => {
    expect(chooseInputDevice({ stored: 'Blackmagic Web Presenter 4K', devices })).toEqual({
      device: 'Blackmagic Web Presenter 4K',
      missing: null,
    });
  });

  it('falls back to the default AND reports it when the device is gone', () => {
    // The whole point of the row. Selecting nothing is correct — Relay cannot
    // capture from a device that is not there — but doing it without a word is
    // what put a service on a laptop microphone.
    expect(chooseInputDevice({ stored: 'Blackmagic Web Presenter 4K', devices: [devices[0]] })).toEqual({
      device: '',
      missing: 'Blackmagic Web Presenter 4K',
    });
  });

  it('says nothing when the operator deliberately chose the system default', () => {
    // '' is a real stored value, and it is not the same fact as never having
    // chosen. Neither is a fault, and neither may raise a row.
    expect(chooseInputDevice({ stored: '', devices })).toEqual({ device: '', missing: null });
    expect(chooseInputDevice({ stored: null, devices })).toEqual({ device: '', missing: null });
    expect(chooseInputDevice({ devices })).toEqual({ device: '', missing: null });
  });

  it('reports a remembered device when this machine has no inputs at all', () => {
    expect(chooseInputDevice({ stored: 'Shure SM58', devices: [] }).missing).toBe('Shure SM58');
    expect(chooseInputDevice({ stored: 'Shure SM58' }).missing).toBe('Shure SM58');
  });
});

describe('remembering the choice', () => {
  // The write is deliberately fire-and-forget, so it lands a dynamic import and a
  // command later. A macrotask covers both.
  const flush = () => new Promise((r) => setTimeout(r, 0));

  beforeEach(() => {
    invoke.mockReset();
  });

  it('writes the device to settings so the next launch starts on it', async () => {
    invoke.mockResolvedValue(null);
    setInputDevice('Blackmagic Web Presenter 4K');
    expect(get(capture).inputDevice).toBe('Blackmagic Web Presenter 4K');
    await flush();
    expect(invoke).toHaveBeenCalledWith('set_setting', {
      key: INPUT_DEVICE_KEY,
      value: 'Blackmagic Web Presenter 4K',
    });
  });

  it('stores the system default as a real choice, not as an absence', async () => {
    invoke.mockResolvedValue(null);
    setInputDevice('');
    await flush();
    expect(invoke).toHaveBeenCalledWith('set_setting', { key: INPUT_DEVICE_KEY, value: '' });
  });

  it('a failed write never reaches the operator changing microphone', async () => {
    // GROUP 2. Thirty seconds before a service is not the moment to throw because
    // a preference would not save. The choice still applies to this run.
    invoke.mockRejectedValue(new Error('database is locked'));
    expect(() => setInputDevice('Shure SM58')).not.toThrow();
    await flush();
    expect(get(capture).inputDevice).toBe('Shure SM58');
  });

  it('picking a device clears the missing-microphone warning', () => {
    invoke.mockResolvedValue(null);
    capture.update((s) => ({ ...s, inputDeviceMissing: 'Blackmagic Web Presenter 4K' }));
    setInputDevice('MacBook Pro Microphone');
    expect(get(capture).inputDeviceMissing).toBe(null);
  });
});


// CHANGING THE MICROPHONE WHILE RELAY IS LISTENING HAS TO REACH THE ENGINE.
//
// RG-291. The operator: *"when audio input switch, continue transcript once audio
// is dected"*. `setInputDevice` updated the store and persisted the setting and
// did nothing else, so the choice took effect at the NEXT `start_capture` and not
// before — and if the reason for the switch was that the old device had just died,
// rule 5 had already stopped the loop and nothing restarted it. The transcript was
// then off for the rest of the service.
//
// The two rendered pickers (`Dock.svelte`, `Settings.svelte`) are DISABLED while
// capturing and say why, so this was never reachable from them. `rooms.js` reaches
// it by another door, and the operator's ask is for the capability itself.
describe('changing the microphone while Relay is listening', () => {
  const flush = () => new Promise((r) => setTimeout(r, 0));
  beforeEach(() => {
    invoke.mockReset();
    capture.update((s) => ({ ...s, capturing: false, audioError: null, inputDevice: '' }));
  });

  it('moves the running capture onto the new device', async () => {
    invoke.mockResolvedValue(1);
    capture.update((s) => ({ ...s, capturing: true }));

    await setInputDevice('Blackmagic Web Presenter 4K');

    const cmds = invoke.mock.calls.map((c) => c[0]);
    expect(cmds).toContain('stop_capture');
    expect(cmds).toContain('start_capture');
    // Stopped BEFORE started — two engines on one microphone is the bug
    // `startCapture`'s own detach comment records, one level down.
    expect(cmds.indexOf('stop_capture')).toBeLessThan(cmds.indexOf('start_capture'));
    // And on the device the operator actually picked, not on whatever was stored.
    const start = invoke.mock.calls.find((c) => c[0] === 'start_capture');
    expect(start[1]).toEqual({ device: 'Blackmagic Web Presenter 4K' });
  });

  it('touches no engine when Relay is not listening', async () => {
    invoke.mockResolvedValue(null);
    await setInputDevice('Blackmagic Web Presenter 4K');
    await flush();
    const cmds = invoke.mock.calls.map((c) => c[0]);
    expect(cmds).not.toContain('stop_capture');
    expect(cmds).not.toContain('start_capture');
    expect(cmds).toContain('set_setting');
  });

  // A FAILED STOP MAY NEVER OPEN A SECOND CAPTURE ON TOP OF A LIVE ONE.
  //
  // `stop_capture` takes a lock, and an audio thread that panicked while holding it
  // leaves the mutex poisoned and the engine RUNNING (`micstop.test.js`,
  // `firstrunmic.test.js`). Starting anyway would put two capture threads on one
  // device and duplicate every event, which is exactly the failure `startCapture`
  // detaches its listeners to avoid.
  it('does not start a second capture when the first would not stop', async () => {
    capture.update((s) => ({ ...s, capturing: true }));
    invoke.mockImplementation(async (cmd) => {
      if (cmd === 'stop_capture') throw new Error('audio lock poisoned');
      return null;
    });

    await setInputDevice('Blackmagic Web Presenter 4K');

    const cmds = invoke.mock.calls.map((c) => c[0]);
    expect(cmds).toContain('stop_capture');
    expect(cmds).not.toContain('start_capture');
    // And it is SAID. A microphone that did not move is not a microphone that did.
    expect(get(capture).audioError).toBeTruthy();
  });

  // GROUP 2: it never throws at the caller. An operator changing microphone thirty
  // seconds before a service must not be handed an exception by the picker.
  it('never rejects, whatever the backend does', async () => {
    capture.update((s) => ({ ...s, capturing: true }));
    invoke.mockRejectedValue(new Error('no bridge'));
    await expect(setInputDevice('Anything')).resolves.toBeUndefined();
  });
});
