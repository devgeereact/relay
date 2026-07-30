import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  AUDIO_OUTPUT_KEY,
  supportsSinkId,
  getAudioOutput,
  setAudioOutput,
  onAudioOutputChange,
  listOutputDevices,
  applySink,
  ensureDeviceAccess,
} from './audioOutput.js';

describe('audioOutput: the operator’s speaker choice', () => {
  beforeEach(() => {
    localStorage.clear();
    delete HTMLMediaElement.prototype.setSinkId;
  });

  it('defaults to the system output when nothing is stored', () => {
    expect(getAudioOutput()).toBe('');
  });

  it('round-trips a selection through storage', () => {
    setAudioOutput('spk-1');
    expect(getAudioOutput()).toBe('spk-1');
    expect(localStorage.getItem(AUDIO_OUTPUT_KEY)).toBe('spk-1');
  });

  it('notifies THIS window on change (storage event does not fire in the writer)', () => {
    const seen = [];
    const off = onAudioOutputChange((id) => seen.push(id));
    setAudioOutput('spk-2');
    off();
    setAudioOutput('spk-3'); // after unsubscribe — must not be seen
    expect(seen).toEqual(['spk-2']);
  });

  it('treats a cleared selection as the default output', () => {
    setAudioOutput('spk-1');
    setAudioOutput('');
    expect(getAudioOutput()).toBe('');
  });

  it('reports no sinkId support when the webview lacks setSinkId', () => {
    expect(supportsSinkId()).toBe(false);
  });

  it('lists only audiooutput devices, shaped for the UI', async () => {
    navigator.mediaDevices = {
      enumerateDevices: async () => [
        { kind: 'audioinput', deviceId: 'mic', label: 'Mic' },
        { kind: 'audiooutput', deviceId: 'default', label: 'Built-in' },
        { kind: 'videoinput', deviceId: 'cam', label: 'Cam' },
        { kind: 'audiooutput', deviceId: 'hdmi', label: 'Projector' },
      ],
    };
    const out = await listOutputDevices();
    expect(out).toEqual([
      { id: 'default', label: 'Built-in', is_default: true },
      { id: 'hdmi', label: 'Projector', is_default: false },
    ]);
  });

  it('returns an empty list rather than throwing when enumeration fails', async () => {
    navigator.mediaDevices = {
      enumerateDevices: async () => {
        throw new Error('denied');
      },
    };
    expect(await listOutputDevices()).toEqual([]);
  });

  // CLAUDE.md §15: a control may never report a success it did not achieve.
  it('does NOT claim success when the webview cannot route audio', async () => {
    const el = {};
    expect(await applySink(el, 'hdmi')).toBe(false);
  });

  it('does NOT claim success when setSinkId rejects (device vanished)', async () => {
    HTMLMediaElement.prototype.setSinkId = vi.fn();
    const el = { setSinkId: vi.fn().mockRejectedValue(new Error('gone')) };
    expect(await applySink(el, 'hdmi')).toBe(false);
  });

  it('reports success only when the routing actually took effect', async () => {
    HTMLMediaElement.prototype.setSinkId = vi.fn();
    const el = { setSinkId: vi.fn().mockResolvedValue(undefined) };
    expect(await applySink(el, 'hdmi')).toBe(true);
    expect(el.setSinkId).toHaveBeenCalledWith('hdmi');
  });

  it('unlocks the device list by tripping the media permission', async () => {
    const stop = vi.fn();
    navigator.mediaDevices = {
      getUserMedia: async () => ({ getTracks: () => [{ stop }] }),
    };
    expect(await ensureDeviceAccess()).toBe(true);
  });

  // cpal owns the microphone for real capture; the webview must never keep a
  // second stream open on it.
  it('releases the microphone immediately after being granted', async () => {
    const stop = vi.fn();
    navigator.mediaDevices = {
      getUserMedia: async () => ({ getTracks: () => [{ stop }] }),
    };
    await ensureDeviceAccess();
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it('releases the microphone even if enumeration later fails', async () => {
    const stop = vi.fn();
    navigator.mediaDevices = {
      getUserMedia: async () => ({
        getTracks: () => [{ stop }, { stop }],
      }),
    };
    await ensureDeviceAccess();
    expect(stop).toHaveBeenCalledTimes(2);
  });

  it('reports failure (not a crash) when the operator declines', async () => {
    navigator.mediaDevices = {
      getUserMedia: async () => {
        throw new Error('NotAllowedError');
      },
    };
    expect(await ensureDeviceAccess()).toBe(false);
  });

  it('routes to the system default when the selection is empty', async () => {
    HTMLMediaElement.prototype.setSinkId = vi.fn();
    const el = { setSinkId: vi.fn().mockResolvedValue(undefined) };
    await applySink(el, '');
    expect(el.setSinkId).toHaveBeenCalledWith('');
  });
});
