// WHERE THE CLIP IS, ACCORDING TO THE SCREEN THAT IS PLAYING IT.
//
// Requirement 11's first half: *"the countdown for the media so as to help know
// when media is almost done or time remaining for preparation of the next plan"*.
//
// **The console must never time a clip off its own preview**, and that constraint
// is the whole design rather than a detail of it. Live's programme pane renders
// through the same component as the wall, so it holds a second `<video>` playing
// the same file. That one buffers differently, starts at a different instant, and
// carries on happily if the wall's copy stalls. An operator reading "0:12 left"
// off the console while the congregation's screen is frozen at 2:30 is rule 35
// exactly: a status line that cannot detect its own failure is not a status line.
//
// So the figure rides the BEAT — the one thing a screen already says about itself,
// and the one that already answers "am I still painting". A position without that
// answer is worthless: a countdown from a screen that stopped responding a minute
// ago is precisely what an operator would time the next cue against.
import { describe, it, expect, vi, afterEach } from 'vitest';
import { startBeat } from './outputHealth.js';

afterEach(() => {
  vi.useRealTimers();
});

const socket = () => {
  const frames = [];
  return { ws: { readyState: 1, send: (f) => frames.push(JSON.parse(f)) }, frames };
};

describe('a screen reports its clip on the beat it already sends', () => {
  it('carries position, duration and whether it is paused', () => {
    const { ws, frames } = socket();
    const stop = startBeat({
      channelId: 4,
      getState: () => 'content',
      getWs: () => ws,
      getMedia: () => ({ pos_ms: 12_000, dur_ms: 240_000, paused: false }),
    });
    stop();
    expect(frames[0]).toMatchObject({
      kind: 'beat',
      channel: 4,
      media_pos_ms: 12_000,
      media_dur_ms: 240_000,
      media_paused: false,
    });
  });

  it('says nothing at all when the screen is not playing one', () => {
    // Not zeroes. A screen showing a verse is not a screen with a clip at 0:00,
    // and the difference is what stops the console counting down something that
    // is not there.
    const { ws, frames } = socket();
    const stop = startBeat({ channelId: 4, getState: () => 'content', getWs: () => ws });
    stop();
    expect(frames[0]).not.toHaveProperty('media_pos_ms');
    expect(frames[0]).not.toHaveProperty('media_dur_ms');
  });

  it('drops a report with no duration WHOLE, rather than sending a zero', () => {
    // Zero is not a clip that takes no time; it is a player that has not loaded
    // one yet. "0:00 left" over a clip that has barely started is worse than
    // saying nothing, because the operator acts on it.
    const { ws, frames } = socket();
    const stop = startBeat({
      channelId: 4,
      getState: () => 'content',
      getWs: () => ws,
      getMedia: () => ({ pos_ms: 0, dur_ms: 0, paused: false }),
    });
    stop();
    expect(frames[0]).not.toHaveProperty('media_dur_ms');
  });

  it('never reports a position past the end of the clip', () => {
    const { ws, frames } = socket();
    const stop = startBeat({
      channelId: 4,
      getState: () => 'content',
      getWs: () => ws,
      getMedia: () => ({ pos_ms: 999_999, dur_ms: 5_000, paused: false }),
    });
    stop();
    expect(frames[0].media_pos_ms).toBe(5_000);
  });

  it('a getMedia that throws costs the beat nothing', () => {
    // The beat's own job is more important than the clip's position. A page that
    // cannot say where its clip is must still be able to say it is painting.
    const { ws, frames } = socket();
    const stop = startBeat({
      channelId: 4,
      getState: () => 'content',
      getWs: () => ws,
      getMedia: () => {
        throw new Error('no');
      },
    });
    stop();
    expect(frames[0]).toMatchObject({ kind: 'beat', channel: 4, state: 'content' });
  });

  it('and the native window says the same thing in camelCase', async () => {
    // One rule, two transports. A window and a browser source must not be able to
    // reach different conclusions about the same clip, which is why the engine
    // reads both through one `MediaBeat`.
    const sent = [];
    const stop = startBeat({
      channelId: 3,
      getState: () => 'content',
      getWs: () => null,
      invoke: async (...a) => sent.push(a),
      getMedia: () => ({ pos_ms: 1_000, dur_ms: 2_000, paused: true }),
    });
    await new Promise((r) => setTimeout(r, 0));
    stop();
    expect(sent[0][1]).toMatchObject({
      mediaPosMs: 1_000,
      mediaDurMs: 2_000,
      mediaPaused: true,
    });
  });
});
