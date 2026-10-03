// THE CONSOLE ACTS ON THE INSTRUCTION IT GAVE, NOT A COPY OF IT (RG-260).
//
// The operator: *"what's on screen is different from what's on the console...
// when the replay button is clicked, only the operator's screen replays... I want
// the media to work using just one control for all screens or output"*.
//
// **The divergence was structural.** `set_media_transport` published a frame to
// every screen and returned nothing, and `setMediaTransport` then rebuilt the
// store out of the arguments it had just passed in:
//
//     mediaTransport.update((t) => ({ paused, loop, volume }))
//
// Three fields. The wire carries seven. So the console's own preview — which
// renders through the same `TemplateRender` and the same `applyMediaTransport` as
// a projector does — was handed an object with **no `replayEpoch` and no
// `seekEpoch`**, and that rule acts on a replay or a scrub only when it sees an
// epoch it has not seen. Pause and Loop worked on both; Replay and Scrub worked
// on the screens and did nothing on the console.
//
// The fix is that there is now ONE shape: Rust hands the frame back and the store
// takes it verbatim.
import { describe, it, expect, vi, beforeEach } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn(async () => () => {}) }));

const FRAME = {
  paused: false,
  looping: true,
  replayEpoch: 3,
  seekEpoch: 1,
  seekMs: 42_000,
  volume: 0.8,
  startedAt: 1_700_000_000_000,
};

beforeEach(() => {
  invoke.mockReset();
  invoke.mockResolvedValue(FRAME);
});

describe('the store is the frame the screens were sent', () => {
  it('takes every field the wire carries, epochs included', async () => {
    const { setMediaTransport, mediaTransport } = await import('./stores/capture.js');
    await setMediaTransport({ replay: true });
    const t = get(mediaTransport);
    // THE TWO THAT WERE MISSING, and they are the whole defect: without an
    // epoch the console's own player is told to do nothing at all.
    expect(t.replayEpoch, 'the console cannot act on a replay').toBe(3);
    expect(t.seekEpoch, 'the console cannot act on a scrub').toBe(1);
    expect(t.seekMs).toBe(42_000);
    // AND THE BASELINE A SCRUB IMPLIES. Without it the console's corrector drags
    // its preview back to where the fire implied, within two seconds, while
    // every screen sits where the operator dropped the handle.
    expect(t.startedAt).toBe(1_700_000_000_000);
  });

  it('says `loop`, because that is what the player rule reads', async () => {
    // Rust cannot name a field `loop`, so the frame says `looping`. The one
    // place that difference is resolved is here — `applyMediaTransport` reads
    // `t.loop`, and a store carrying `looping` would silently stop looping.
    const { setMediaTransport, mediaTransport } = await import('./stores/capture.js');
    await setMediaTransport({ loop: true });
    expect(get(mediaTransport).loop, 'the console lost the loop flag in translation').toBe(true);
  });

  it('and the shape it hands the player is the shape the wire hands a screen', async () => {
    // THE POINT OF THE WHOLE CHANGE. Whatever the console holds must be
    // something `applyMediaTransport` can act on identically to a projector —
    // so the assertion is that the same function, given the console's object and
    // given the wire's, does the same thing to a player.
    const { setMediaTransport, mediaTransport } = await import('./stores/capture.js');
    const { applyMediaTransport } = await import('./mediatransport.js');
    await setMediaTransport({ replay: true });

    const el = () => ({ loop: false, volume: 1, currentTime: 99, paused: true, play: () => {}, pause() {} });
    const a = el();
    const b = el();
    applyMediaTransport(a, get(mediaTransport), null);
    applyMediaTransport(b, { ...FRAME, loop: FRAME.looping }, null);
    // 42, NOT 0, and that is the documented order rather than a surprise: this
    // frame carries BOTH epochs, replay is applied first and the scrub second,
    // because "replay then scrub" means *start again, at this point*. Written
    // as `toBe(0)` first, this case was asserting a rule that does not exist.
    expect(a.currentTime, 'the console acted on neither epoch').toBe(42);
    expect(a.currentTime).toBe(b.currentTime);
    expect(a.loop).toBe(b.loop);
    expect(a.volume).toBe(b.volume);
  });

  it('acts on a replay ALONE, which is the press the operator reported', async () => {
    // The narrowest form of the bug: one press of Replay, nothing else on the
    // frame. Before RG-260 the console was handed `{paused:false, loop, volume}`
    // and moved nothing.
    const { applyMediaTransport } = await import('./mediatransport.js');
    const { setMediaTransport, mediaTransport } = await import('./stores/capture.js');
    invoke.mockResolvedValueOnce({ ...FRAME, seekEpoch: 0, seekMs: 0, replayEpoch: 9 });
    await setMediaTransport({ replay: true });
    const el = { loop: false, volume: 1, currentTime: 77, paused: true, play: () => {}, pause() {} };
    applyMediaTransport(el, get(mediaTransport), { replay: 8, seek: 0 });
    expect(el.currentTime, 'Replay moved nothing on the console').toBe(0);
  });

  it('a command that fails leaves the store alone', async () => {
    // GROUP 1 (throws). A store updated before the call would claim a hold that
    // no screen was ever told about.
    const { setMediaTransport, mediaTransport } = await import('./stores/capture.js');
    await setMediaTransport({ loop: true });
    const before = get(mediaTransport);
    invoke.mockRejectedValueOnce(new Error('no'));
    await expect(setMediaTransport({ paused: true })).rejects.toBeTruthy();
    expect(get(mediaTransport)).toEqual(before);
  });
});

/** Read a store's value without a subscription ceremony. */
function get(store) {
  let v;
  store.subscribe((x) => (v = x))();
  return v;
}

// ── AND THE CONSOLE'S PREVIEW IS GIVEN THE SCRUB'S BASELINE (RG-260) ────────
//
// The second half of the same divergence, and it bites even once the epochs
// arrive. `TemplateRender`'s corrector pulls a player back to where Relay's
// clock says the clip should be, measured from `content.media_started_at`. A
// scrub moves that baseline — Rust sets `started_at = now - seek_ms` precisely so
// the corrector does not undo the operator's own drag — and `Output.svelte`
// restates it on every screen.
//
// Live did not. So after a scrub the console's preview kept the baseline the FIRE
// implied, the corrector found it two seconds' worth of drift away from where the
// operator had just put it, and pulled the picture back while every projector in
// the building stayed put. The operator watches that pane to decide what the room
// is seeing.
describe('the console preview is told where the scrub put the clip', () => {
  const read = (p) => require('node:fs').readFileSync(require('node:path').resolve(p), 'utf8');
  const LIVE = read('src/lib/views/Live.svelte');
  const OUTPUT = read('src/Output.svelte');

  it('Live restates media_started_at, exactly as an output page does', () => {
    expect(OUTPUT, 'the screens stopped restating it').toMatch(/media_started_at: m\.started_at/);
    expect(LIVE, 'the console preview still runs on the baseline the fire implied').toMatch(
      /media_started_at/,
    );
  });

  it('and it takes it from the transport store, through the one rule (RG-271)', () => {
    // THE STORE IS STILL THE SOURCE — that is RG-260 and it stands. What changed
    // is that the figure is no longer applied unconditionally: `startedAt` is
    // only ever set by a SCRUB and nothing clears it, so after one scrub every
    // clip fired for the rest of a service was previewed against that scrub's
    // baseline. `baselineFor` decides which instant belongs to the clip that is
    // actually up.
    expect(LIVE).toMatch(/\$mediaTransport\?\.startedAt/);
    expect(LIVE).toMatch(/baselineFor\(/);
  });
});
