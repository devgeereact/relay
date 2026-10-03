// 2026-09-21 · P-1. An UNATTENDED auto-fire reaches the console only through the
// `output://content` event — no wrapper runs, so nothing called `leavePlan()`.
// With a song or notice cue on air, a detected verse painted the wall, the
// transport bar went on saying SLIDE, and the next `→` fired the next plan slide
// over the reading. Amber was honest (it compares the words); the bar was not.
//
// The event cannot simply always leave the plan: a PLAN fire produces the same
// event, and Tauri may deliver it after the command's promise resolves — after
// `setLive` has marked the cue on air. So a plan fire announces what it is about
// to put up, and the listener leaves the plan only for content it was not told
// to expect.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const listeners = new Map();
let onInvoke = async () => null;
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => onInvoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name, cb) => {
    listeners.set(name, cb);
    return () => listeners.delete(name);
  }),
}));

const fire = (name, payload) => listeners.get(name)?.({ payload });

describe('P-1 · an auto-fire takes the plan off air, a plan fire does not', () => {
  let cap;
  beforeEach(async () => {
    listeners.clear();
    onInvoke = async () => null;
    vi.resetModules();
    cap = await import('./stores/capture.js');
    await cap.initAudio();
    cap.liveCue.set({ cueId: 7, slide: 2, onAir: true });
  });

  it('a detected verse the console never asked for leaves the plan, keeping the position', () => {
    fire('output://content', { kind: 'scripture', reference: 'Hosea 6:1', text: 'Come, and let us return' });
    expect(get(cap.liveCue)).toEqual({ cueId: 7, slide: 2, onAir: false });
  });

  it('a plan fire whose event arrives AFTER the command resolves stays on air', async () => {
    // The order Tauri does not promise: the promise resolves, then the event lands.
    await cap.manualFire('Psalms 23:5', null, null, true, null);
    cap.liveCue.set({ cueId: 8, slide: 0, onAir: true }); // what fireSlide does after the await
    fire('output://content', { kind: 'scripture', reference: 'Psalms 23:5', text: 'Thou preparest a table' });
    expect(get(cap.liveCue)).toEqual({ cueId: 8, slide: 0, onAir: true });
  });

  it('a plan fire whose event arrives DURING the command stays on air too', async () => {
    onInvoke = async (cmd) => {
      if (cmd === 'manual_fire') fire('output://content', { kind: 'scripture', reference: 'Psalms 23:5', text: 'x' });
      return null;
    };
    await cap.manualFire('Psalms 23:5', null, null, true, null);
    cap.liveCue.set({ cueId: 8, slide: 0, onAir: true });
    expect(get(cap.liveCue).onAir).toBe(true);
  });

  it('a song slide from the plan is expected by its words, a countdown by its kind, a picture by its id', async () => {
    await cap.fireContent('Amazing Grace', 'Amazing grace how sweet the sound', 'song', null, null, true, null);
    cap.liveCue.set({ cueId: 9, slide: 1, onAir: true });
    fire('output://content', { kind: 'song', reference: '', text: 'Amazing grace how sweet the sound' });
    expect(get(cap.liveCue).onAir, 'the song slide the plan fired').toBe(true);

    await cap.fireMedia(4, null, true, null);
    cap.liveCue.set({ cueId: 10, slide: 0, onAir: true });
    fire('output://content', { kind: 'media', reference: '', media_url: 'http://192.168.1.42:8032/media/4' });
    expect(get(cap.liveCue).onAir, 'the picture the plan fired').toBe(true);

    // …and after that, a verse the AI heard still takes it off.
    fire('output://content', { kind: 'scripture', reference: 'John 3:16', text: 'For God so loved' });
    expect(get(cap.liveCue).onAir).toBe(false);
  });
});
