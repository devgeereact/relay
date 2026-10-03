// THE REST OF "ALL REQUIRED MEDIA FUNCTIONALITIES" (RG-221).
//
// The operator's words, in full: *"Controlls on the media shouldnt work just
// for only stage so as to be consistent and it should have all required media
// functionalities"*. RG-214 closed the first half — the transport reached every
// screen except the preacher's. This is the second half, and it was left
// explicitly open in that row rather than quietly counted as done: **scrub and
// volume did not exist on any surface.**
//
// A church cannot start a clip thirty seconds in, cannot go back to a line the
// preacher wants again, and cannot turn a clip's sound down under a spoken
// introduction. Play, Pause, Replay and Loop are not the whole of a transport.
//
// ── TWO THINGS THAT NEEDED DECIDING ─────────────────────────────────────────
//
// **A scrub is an EVENT, not a state**, exactly as Replay is, and for the same
// reason the wire carries Replay as a counter: a retained frame replayed to a
// screen joining an hour later must not drag its clip back to where somebody
// once dropped the handle. So it is an epoch and a position, and a screen acts
// on an epoch it has not seen.
//
// **A scrub moves the sync baseline.** RG-220 corrects every screen onto the
// instant Relay sent the clip, so a scrub without a new baseline would be undone
// by the corrector within two seconds — the operator drags the handle, the
// picture jumps back, and the app looks broken. The frame therefore carries the
// baseline the scrub implies, and the pages take it.
//
// **Volume is a room setting, not a property of a clip.** It survives the next
// fire, where `paused` and `loop` are deliberately reset: an operator who turned
// the sound down for a quiet room did not mean "for this clip only".
import { describe, it, expect, vi } from 'vitest';
import { applyMediaTransport } from './mediatransport.js';

/** A media element as jsdom gives one, with the bits the rule touches. */
const player = (over = {}) => {
  const el = {
    loop: false,
    paused: false,
    currentTime: 12,
    volume: 1,
    pause: vi.fn(function () { el.paused = true; }),
    play: vi.fn(() => Promise.resolve()),
    ...over,
  };
  return el;
};

const T = (over = {}) => ({ paused: false, loop: false, replayEpoch: null, ...over });

describe('scrub — an event, acted on once', () => {
  it('moves the clip to where the handle was dropped', () => {
    const el = player();
    applyMediaTransport(el, T({ seekEpoch: 1, seekMs: 45_000 }), null);
    expect(el.currentTime).toBe(45);
  });

  it('is acted on ONCE, so a retained frame cannot drag a late screen backwards', () => {
    // The exact reason Replay is a counter, stated again because this is the
    // second event on the same frame and the trap is identical: a screen that
    // joins an hour later is handed the last transport, which still names a
    // scrub somebody did at the start of the service.
    const el = player();
    const state = applyMediaTransport(el, T({ seekEpoch: 4, seekMs: 30_000 }), null);
    expect(el.currentTime).toBe(30);
    el.currentTime = 90;
    applyMediaTransport(el, T({ seekEpoch: 4, seekMs: 30_000 }), state);
    expect(el.currentTime, 'the same scrub was acted on twice').toBe(90);
  });

  it('and a later scrub still moves it', () => {
    const el = player();
    const state = applyMediaTransport(el, T({ seekEpoch: 1, seekMs: 10_000 }), null);
    applyMediaTransport(el, T({ seekEpoch: 2, seekMs: 70_000 }), state);
    expect(el.currentTime).toBe(70);
  });

  it('a replay and a scrub are counted apart — one cannot mask the other', () => {
    // They arrive on the same frame. A single epoch for both would mean a scrub
    // immediately after a replay was swallowed, which is precisely the pair of
    // presses an operator makes when a clip started in the wrong place.
    const el = player({ currentTime: 50 });
    const state = applyMediaTransport(el, T({ replayEpoch: 1 }), null);
    expect(el.currentTime).toBe(0);
    applyMediaTransport(el, T({ replayEpoch: 1, seekEpoch: 1, seekMs: 25_000 }), state);
    expect(el.currentTime, 'the scrub was eaten by the replay it followed').toBe(25);
  });
});

describe('volume — the room, not the clip', () => {
  it('is applied to the player', () => {
    const el = player();
    applyMediaTransport(el, T({ volume: 0.25 }), null);
    expect(el.volume).toBe(0.25);
  });

  it('is clamped, because a value out of range throws on a real element', () => {
    const el = player();
    applyMediaTransport(el, T({ volume: 4 }), null);
    expect(el.volume).toBe(1);
    applyMediaTransport(el, T({ volume: -2 }), null);
    expect(el.volume).toBe(0);
  });

  it('is left alone when the frame says nothing about it', () => {
    // An older engine, and the ordinary case: a Pause must not also reset the
    // sound to full, which is the shape of every "one control moved another"
    // bug this transport already documents.
    const el = player({ volume: 0.4 });
    applyMediaTransport(el, T({ paused: true }), null);
    expect(el.volume).toBe(0.4);
  });
});

describe('what the rest of the transport still does', () => {
  it('nothing here disturbed pause, loop or replay', () => {
    const el = player({ paused: true });
    const state = applyMediaTransport(el, T({ loop: true, replayEpoch: 7, volume: 0.5 }), null);
    expect(el.loop).toBe(true);
    expect(el.currentTime).toBe(0);
    expect(el.play).toHaveBeenCalled();
    // The bookkeeping is a PAIR now — see the case above about a replay and a
    // scrub being counted apart. One number could not hold two events.
    expect(state).toEqual({ replay: 7, seek: null });
  });
});
