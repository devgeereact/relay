// Rehearsal must never fail silently.
//
// Every other wrapper in capture.js swallows backend errors into a `catch {}` and
// carries on — which is right for a device list, and catastrophic here. Rust
// REFUSES to enter rehearsal while a service is being recorded. If that refusal is
// swallowed, the operator believes they are practising, and the next thing they
// press goes onto the wall in front of the congregation.
//
// So `setRehearsal` is the one wrapper in the store that is allowed to throw, and
// this pins that down.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));

const { rehearsing, setRehearsal, loadRehearsal } = await import('./stores/capture.js');

describe('rehearsal', () => {
  beforeEach(() => {
    invoke.mockReset();
    rehearsing.set(false);
  });

  it('is off by default — a fresh install broadcasts for real', () => {
    // If it defaulted ON, an operator's first ever service would show nothing on
    // the projector, with no visible reason why.
    expect(get(rehearsing)).toBe(false);
  });

  it('turning it on flips the store', async () => {
    invoke.mockResolvedValue(null);
    await setRehearsal(true);
    expect(invoke).toHaveBeenCalledWith('set_rehearsal', { on: true });
    expect(get(rehearsing)).toBe(true);
  });

  it('THROWS when the backend refuses, and does NOT flip the store', async () => {
    // Rust refuses while a service is recording. The store must not claim a state
    // the backend is not in — that disagreement is the whole danger.
    invoke.mockRejectedValue('A service is being recorded. End it before rehearsing.');
    await expect(setRehearsal(true)).rejects.toBeTruthy();
    expect(get(rehearsing)).toBe(false);
  });

  it('reads its state from the backend, which owns it', async () => {
    invoke.mockResolvedValue(true);
    await loadRehearsal();
    expect(invoke.mock.calls[0][0]).toBe('get_rehearsal');
    expect(get(rehearsing)).toBe(true);
  });

  it('a backend that answers nonsense is treated as NOT rehearsing', async () => {
    // Fail towards "you are live". Believing you are live when you are rehearsing
    // costs a blank projector; believing you are rehearsing when you are live puts
    // your practice run in front of the church.
    invoke.mockResolvedValue('yes');
    await loadRehearsal();
    expect(get(rehearsing)).toBe(false);
  });
});
