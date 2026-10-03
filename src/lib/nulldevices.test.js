// A RESOLVED NULL IS NOT A REJECTION (RG-267).
//
// Found by driving the built console against a bridge that answered `null` for
// `list_audio_devices` — which is what an older backend, a renamed command or a
// read that legitimately has no answer all look like from the frontend.
//
// `initAudio` guards it with `.catch(() => [])`. That catches a THROW and does
// nothing at all about a resolution, so `capture.devices` became `null`, the
// microphone picker's `{#each $capture.devices}` threw
// `{#each} only works with iterable values`, and the crash panel took the whole
// console with it: **"The console stopped responding."** over a working engine.
//
// This is CLAUDE.md's *"an empty env var is not an absent env var"* one layer
// up: the shape a value arrives in is a separate question from whether the call
// succeeded, and a guard on one is not a guard on the other.
//
// The harm is out of all proportion to the cause. A missing device list should
// cost an empty picker; it cost the operator every control in the product.
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { get } from 'svelte/store';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

beforeEach(() => {
  invoke.mockReset();
});

/** Every reply is `null` — the shape this defect arrives in. */
const allNull = () => invoke.mockResolvedValue(null);

describe('initAudio survives a backend that answers null', () => {
  it('leaves the device list ITERABLE, so the picker can render nothing', async () => {
    allNull();
    const cap = await import('./stores/capture.js');
    await cap.initAudio();
    const devices = get(cap.capture).devices;
    expect(Array.isArray(devices), 'the microphone picker was handed a non-array').toBe(true);
    expect(devices).toEqual([]);
  });

  it('and still reports the engine as attached, because it is', async () => {
    // THE DISTINCTION THAT MATTERS. A null device list says nothing about
    // whether the bridge is there, and marking the engine absent would disable
    // Clear screens over a perfectly good connection.
    allNull();
    const cap = await import('./stores/capture.js');
    await cap.initAudio();
    expect(get(cap.capture).available).toBe(true);
  });

  it('a rejection is handled the same way, which it already was', async () => {
    invoke.mockRejectedValue(new Error('no such command'));
    const cap = await import('./stores/capture.js');
    await cap.initAudio();
    expect(Array.isArray(get(cap.capture).devices)).toBe(true);
  });

  it('and the STT status is an OBJECT, so a reader can ask it a question', async () => {
    // THE SECOND ONE, found in the same run: `Live.svelte` renders
    // `$capture.stt.loaded`, and a `stt_status` that resolves `null` threw
    // `Cannot read properties of null (reading 'loaded')` while the component
    // was being constructed — so the run surface never mounted at all.
    //
    // Same shape as the device list, same guard that does not guard it, and the
    // same disproportion: an unknown model should read as "no model", not as a
    // console that will not open.
    allNull();
    const cap = await import('./stores/capture.js');
    await cap.initAudio();
    const stt = get(cap.capture).stt;
    expect(stt, 'the run surface was handed a null to read a field off').toBeTruthy();
    expect(stt.loaded).toBe(false);
  });

  it('a real list is passed through untouched', async () => {
    const list = [{ name: 'MacBook Pro Microphone', is_default: true }];
    invoke.mockImplementation(async (cmd) => (cmd === 'list_audio_devices' ? list : null));
    const cap = await import('./stores/capture.js');
    await cap.initAudio();
    expect(get(cap.capture).devices).toEqual(list);
  });
});

// ── AND THE CHOKE POINT, WHICH IS WHERE IT BELONGS (RG-267) ─────────────────
//
// Two guards written by hand fixed two symptoms. The third instance — found in
// the same run — was `ModelSetup.svelte`'s `models.some(...)` throwing
// `Cannot read properties of null (reading 'some')`, and `listModels` already
// declares `[]` as its fallback:
//
//     return guardedRead('listModels', async (call) => call('list_models'), []);
//
// That fallback is used on a THROW and never on a resolution, so every read in
// the store that declares an array fallback has the same hole. Rule 36: the
// check goes at the choke point, not at the call sites — a guard added at three
// of them is a guard missing from the fourth, and this file already found
// three.
//
// **Deliberately narrow.** Only an ARRAY fallback coerces, because only an array
// says something about shape that a caller then relies on (`.map`, `.some`,
// `{#each}`). A `null` fallback means "no answer" and a null answer IS that; an
// object fallback would need to know which keys matter, which is the call
// site's business.
describe('guardedRead hands back the shape it promised', () => {
  it('an array read that resolves null answers with the fallback', async () => {
    invoke.mockResolvedValue(null);
    const cap = await import('./stores/capture.js');
    const models = await cap.listModels();
    expect(Array.isArray(models), 'a caller was handed null to iterate').toBe(true);
    expect(models).toEqual([]);
  });

  it('and so do the other list reads, because the rule is in one place', async () => {
    invoke.mockResolvedValue(null);
    const cap = await import('./stores/capture.js');
    for (const [name, fn] of [
      ['listMedia', cap.listMedia],
      ['listPlans', cap.listPlans],
      ['listOutputChannels', cap.listOutputChannels],
    ]) {
      expect(Array.isArray(await fn()), `${name} handed back a non-array`).toBe(true);
    }
  });

  it('a real array still passes through untouched', async () => {
    const rows = [{ id: 1, installed: true }];
    invoke.mockResolvedValue(rows);
    const cap = await import('./stores/capture.js');
    expect(await cap.listModels()).toEqual(rows);
  });

  it('a read whose fallback is NOT an array is left exactly as it was', async () => {
    // `null` means "no answer" for these, and a null answer is that answer. A
    // blanket coercion here would invent a value where the call site is
    // deliberately checking for its absence.
    invoke.mockResolvedValue(null);
    const cap = await import('./stores/capture.js');
    expect(await cap.getSong(1)).toBeNull();
  });
});
