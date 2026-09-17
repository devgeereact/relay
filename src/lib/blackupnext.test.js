// RG-151 — A BLACKOUT MUST TAKE THE PREACHER'S "UP NEXT" DOWN, LIKE A CLEAR DOES.
//
// `Live.svelte` takes the stage monitor's Up Next panel down by watching the
// truthy→falsy edge of the `live` store, and reports its own failure if it
// cannot — because until 2026-08-14 nothing did, and a preacher read a stale
// hint for a whole service.
//
// `output://clear` sets `live` to null. `output://black` did not. One line
// apart, under a comment documenting the same fix for the clear. So `Esc` took
// the preacher's Up Next down and `B` left it standing, and the two panic
// controls disagreed about a screen the congregation cannot see and the preacher
// is reading from.
//
// This is watched at the STORE rather than at a button, deliberately and for the
// same reason the clear is: `/api/clear` from the preacher's phone, the spoken
// command and the exit from a rehearsal all reach `channels::clear` directly and
// never pass through any control this console owns.
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

const listeners = new Map();
// A plain async function, not a `vi.fn`: this suite calls `vi.resetModules()`
// between cases and a mock whose implementation is reset returns `undefined`,
// which `initAudio` then calls `.catch` on. The distinction cost twenty minutes.
vi.mock('@tauri-apps/api/core', () => ({ invoke: async () => null }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name, cb) => {
    listeners.set(name, cb);
    return () => listeners.delete(name);
  }),
}));

describe('RG-151 · a blackout answers the same way as a clear', () => {
  let cap;
  beforeEach(async () => {
    listeners.clear();
    vi.resetModules();
    cap = await import('./stores/capture.js');
    await cap.initAudio();
  });

  const fire = (name, payload) => listeners.get(name)?.({ payload });

  it('takes `live` down on a blackout, so the stage-hint watcher fires', async () => {
    cap.live.set({ reference: 'John 3:16', text: 'For God so loved' });
    expect(get(cap.live), 'the fixture never armed').not.toBeNull();

    fire('output://black');

    // The watcher in `Live.svelte` is `live` truthy→falsy. If this stays set, the
    // preacher's monitor keeps a hint about content that is no longer on any
    // screen, for the rest of the service.
    expect(
      get(cap.live),
      'a blackout left the preacher reading a hint about a wall that is now black',
    ).toBeNull();
    expect(get(cap.screenBlack), 'a blackout must still be distinguishable from a clear').toBe(
      true,
    );
  });

  it('and a clear still answers the same way, with the other flag', async () => {
    // The pair is the point. `screenBlack` is what separates the two, and it must
    // keep separating them — a blackout that becomes indistinguishable from a
    // clear is a different bug in the same place.
    cap.live.set({ reference: 'Psalm 23:1', text: 'The Lord is my shepherd' });
    fire('output://clear');
    expect(get(cap.live)).toBeNull();
    expect(get(cap.screenBlack)).toBe(false);
  });
});
