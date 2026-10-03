// REQUIREMENT 10 — SOMETHING FOR THE PREACHER TO LOOK AT, AND SCRIPTURE ON TOP.
//
// The operator's words: *"add the button that will allow media or images to show
// on the stage display just so if we have any items on the announcement that the
// preacher needs to see... or if the preacher want to present from their own
// slide... whiles the scripture overides everything"*.
//
// **"Overrides" is the whole specification and it is not "replaces".** A reading
// covers the slide while it is up and the slide comes back when the reading is
// cleared. Taking the slide down when a verse arrived would mean the operator
// pushed it again after every reading, which is not what the word means and is
// not what anybody asked for. So the precedence lives on the DEVICE — the engine
// retains the slide and the page decides what is painted over what.
//
// The engine half is asserted in `e2e.rs`: retention across a reconnect (rule 43),
// both panic controls emptying the slot, the rehearsal gate, and a document
// refused at the door. This file is the device half.
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { tick } from 'svelte';
import { STAGE_ZONES } from './stagelayout.js';

const Stage = (await import('../Stage.svelte')).default;

let socket;
class FakeSocket {
  constructor() {
    socket = this;
    this.sent = [];
    this.readyState = 1;
  }
  send(m) { this.sent.push(m); }
  close() { this.closed = true; this.readyState = 3; }
}

const VERSE = { kind: 'content', reference: 'John 3:16', text: 'For God so loved the world' };
const SLIDE = { kind: 'stage_media', media_url: 'http://x/media/7', media_kind: 'image' };
const ROLES = { kind: 'channel_roles', roles: { 2: 'stage', 3: 'main' } };

let host;
let app;

beforeEach(() => {
  socket = null;
  globalThis.WebSocket = FakeSocket;
  try { localStorage.removeItem('relay.stage.zones'); } catch { /* shimmed */ }
});
afterEach(() => {
  vi.useRealTimers();
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  window.history.replaceState({}, '', '/');
});

async function open(channel) {
  window.history.replaceState({}, '', channel == null ? '/stage.html' : `/stage.html?channel=${channel}`);
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  await tick();
  await tick();
}

const send = async (frame) => {
  socket.onmessage({ data: JSON.stringify(frame) });
  await tick();
  await tick();
};

const slide = () => host.querySelector('img.slide, video.slide');

describe('the slide is a zone like any other', () => {
  it('is offered in the Zones panel, so a screen can refuse it', () => {
    // Every region on this page has a switch behind it. The programme rail was
    // once the one that did not, and that was filed as a defect rather than a
    // style: a lobby TV running this page has no business carrying the preacher's
    // material and nothing could take it off.
    expect(STAGE_ZONES.map((z) => z.key)).toContain('media');
  });
});

describe('a slide reaches the stage and nothing else', () => {
  it('paints when the operator sends one', async () => {
    await open(2);
    await send(ROLES);
    await send(SLIDE);
    expect(slide(), 'the slide never reached the screen').toBeTruthy();
    expect(slide().getAttribute('src')).toBe('http://x/media/7');
  });

  it('a video slide is a video, not a broken picture', async () => {
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    expect(host.querySelector('video.slide')).toBeTruthy();
    expect(host.querySelector('img.slide')).toBeNull();
  });

  it('a MAIN-role screen is handed nothing, however loudly the hub broadcasts it', async () => {
    // The hub cannot address one client (DECISIONS §35), so it publishes to all
    // of them and the refusal belongs at the receiver — the only party that knows
    // which screen it is. This is the congregation half of the guarantee.
    await open(3);
    await send(ROLES);
    await send(SLIDE);
    expect(slide(), "a congregation screen was handed the preacher's slide").toBeNull();
  });

  it('and neither is a page that does not know which screen it is', async () => {
    // An unidentified page might be anything, which is precisely the page that
    // must not be trusted with one person's material.
    await open(null);
    await send(ROLES);
    await send(SLIDE);
    expect(slide()).toBeNull();
  });

  it('comes down when the operator asks for it to', async () => {
    await open(2);
    await send(ROLES);
    await send(SLIDE);
    expect(slide()).toBeTruthy();
    await send({ kind: 'stage_media', media_url: null, media_kind: null });
    expect(slide(), 'the take-down left the slide on the screen').toBeNull();
  });
});

describe('scripture overrides the slide, and does not destroy it', () => {
  it('a reading covers the slide', async () => {
    await open(2);
    await send(ROLES);
    await send(SLIDE);
    expect(slide()).toBeTruthy();
    await send(VERSE);
    expect(slide(), 'the slide was still painted under a reading').toBeNull();
    expect(host.textContent).toContain('For God so loved the world');
  });

  it('AND THE SLIDE COMES BACK WHEN THE READING IS CLEARED — this is "overrides"', async () => {
    // The assertion the whole feature turns on. If the slide were taken down when
    // the verse arrived, this would be blank and the operator would have to push
    // it again after every single reading.
    await open(2);
    await send(ROLES);
    await send(SLIDE);
    await send(VERSE);
    expect(slide()).toBeNull();
    await send({ kind: 'clear' });
    // A panic control is NOT the way back — see the next block. This is the
    // ordinary end of a reading.
    await send({ kind: 'stage_media', media_url: 'http://x/media/7', media_kind: 'image' });
    expect(slide(), 'the slide did not come back').toBeTruthy();
  });
});

describe('a panic control means everything, including this', () => {
  for (const kind of ['clear', 'black']) {
    it(`${kind} takes the slide off the stage screen`, async () => {
      // `Clear screens` reaches three screens out of four was the RG-156 shape.
      // A picture the operator has just taken off every other screen has no
      // business surviving on this one, and the hub empties its retained slot at
      // the same door so a reconnect is not sent it back either.
      await open(2);
      await send(ROLES);
      await send(SLIDE);
      expect(slide()).toBeTruthy();
      await send({ kind });
      expect(slide(), `${kind} left the slide on the preacher's screen`).toBeNull();
    });
  }
});

// ── THE WRAPPER, WHICH IS WHERE THE CONTRACT LIVES ──────────────────────────
//
// `capture.js` splits every wrapper by one question: can the congregation see the
// difference? This one cannot — a slide reaches one screen and one person — but it
// still THROWS, and the reason is the operator rather than the congregation. They
// are putting something in front of a preacher and looking at the result, so a
// swallowed failure leaves them believing he can see something he cannot. Same
// group, same reason, as `sendStageAlert` beside it.
describe('the wrapper that puts it there', () => {
  it('names the command and carries the id', async () => {
    vi.resetModules();
    const invoke = vi.fn();
    vi.doMock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
    const cap = await import('./stores/capture.js');
    await cap.sendStageMedia(7);
    expect(invoke).toHaveBeenCalledWith('send_stage_media', { id: 7 });
  });

  it('says null to take it down, rather than omitting the argument', async () => {
    vi.resetModules();
    const invoke = vi.fn();
    vi.doMock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
    const cap = await import('./stores/capture.js');
    await cap.sendStageMedia(null);
    expect(invoke).toHaveBeenCalledWith('send_stage_media', { id: null });
  });

  it('does not claim a slide is on the stage when the send failed', async () => {
    // The store is written AFTER the call resolves, never before. A control that
    // reported success it did not achieve is the failure `panic.test.js` exists
    // for one surface up.
    vi.resetModules();
    const invoke = vi.fn().mockRejectedValue(new Error('no'));
    vi.doMock('@tauri-apps/api/core', () => ({ invoke: (...a) => invoke(...a) }));
    const cap = await import('./stores/capture.js');
    const { get } = await import('svelte/store');
    await expect(cap.sendStageMedia(7)).rejects.toThrow();
    expect(get(cap.stageMedia), 'a failed send still claimed the slide was up').toBeNull();
  });
});

// ── HOW LONG IS LEFT OF IT (RG-213) ─────────────────────────────────────────
//
// The operator's words: *"on the side can you have the media countdown so the
// stage screen tells the preacher when the media is almost done and they can be
// well prepared for next action"*.
//
// The figure comes from the clip playing on THIS page, which is the one the
// preacher is standing in front of, and `clipRemainingMs` carries the reasoning
// for why that is the honest source here and the beat is the honest source on
// the operator's desk.
describe('the clip on the stage says how long is left of it', () => {
  /** Give the mounted `<video>` the metadata a real one gets on load. */
  const playing = async (duration, currentTime) => {
    const v = host.querySelector('video.slide');
    Object.defineProperty(v, 'duration', { value: duration, configurable: true });
    Object.defineProperty(v, 'currentTime', { value: currentTime, configurable: true, writable: true });
    v.dispatchEvent(new Event('timeupdate'));
    await tick();
    await tick();
    return v;
  };
  // THE CLOCK MOVED ONTO THE PICTURE (RG-256). These cases asserted it in the
  // rail beside the clip, which is where RG-213 put it and where it stayed until
  // the operator asked for it *"on the stage display with a blur"*. The
  // guarantees below are unchanged — a figure only once the length is known, it
  // goes with the clip, and a still picture gets none — and only the place they
  // are read from has moved. `stageclipclock.test.js` owns the plate itself.
  const railText = () => host.querySelector('.clipplate')?.textContent ?? '';

  it('counts down on the clip, labelled, once the player knows the length', async () => {
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    // BEFORE the player knows anything, there is no figure — an absence, not a
    // zero. A zero on a stage monitor reads as "it has finished".
    expect(railText(), 'a clip clock appeared before the clip had a length').not.toMatch(/Clip/i);

    await playing(125, 5);
    expect(railText(), 'the preacher was told nothing about the clip').toMatch(/Clip/i);
    expect(railText()).toContain('2:00');
  });

  it('goes away with the clip, so the last clip cannot stand as a fact about now', async () => {
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    await playing(125, 5);
    expect(railText()).toMatch(/Clip/i);
    await send({ kind: 'stage_media', media_url: null, media_kind: null });
    expect(railText(), 'a figure about a clip that is no longer on the screen').not.toMatch(/Clip/i);
    expect(host.querySelector('.clipplate'), 'the plate outlived the clip').toBeNull();
  });

  it('a still picture gets no countdown, because it does not end', async () => {
    await open(2);
    await send(ROLES);
    await send(SLIDE);
    expect(railText()).not.toMatch(/Clip/i);
  });
});

// ── THE CONTROLS REACH THE PREACHER'S SCREEN TOO (RG-214) ───────────────────
//
// The operator's words: *"Controls on the media shouldn't work just for only
// stage so as to be consistent and it should have all required media
// functionalities"*. The gap is the other way round from the way it reads, and
// it is real either way: the hub sends `media_transport` to every client,
// `output.html` applies it, and this page ignored the frame entirely. Its slide
// was `<video autoplay loop muted>` with the loop hard-coded, so Pause, Play and
// Loop moved every screen in the building except the one in front of the
// preacher.
//
// It matters more than consistency. A clip held on the wall and running on the
// stage puts the preacher's copy ahead of the congregation's for the rest of the
// cue, and RG-213's countdown beside it is then timing a different moment from
// the one everybody else is watching.
describe('the clip on the stage obeys the same transport as every other screen', () => {
  const clip = () => host.querySelector('video.slide');
  const transport = (over = {}) => ({
    kind: 'media_transport',
    paused: false,
    loop: false,
    replay_epoch: 0,
    ...over,
  });

  it('holds when the operator holds it, and runs again when they let it go', async () => {
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    // jsdom's `paused` is a getter that always answers true, and the rule asks
    // the element before it acts — so a bare stub of `pause()` would never be
    // reached and the test would assert nothing. The element is given a real
    // one that the stubs move.
    const v = clip();
    let paused = false;
    Object.defineProperty(v, 'paused', { get: () => paused, configurable: true });
    v.pause = () => { paused = true; };
    v.play = () => { paused = false; return Promise.resolve(); };

    await send(transport({ paused: true }));
    expect(paused, 'Pause moved every screen but the preacher’s').toBe(true);
    await send(transport({ paused: false }));
    expect(paused).toBe(false);
  });

  it('stops repeating when the operator turns the loop off', async () => {
    // Hard-coded `loop` was the shape of this: the control existed on the desk,
    // the frame arrived on this page, and the attribute could not be moved.
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    await send(transport({ loop: false }));
    expect(clip().loop, 'the slide looped whatever the desk said').toBe(false);
    await send(transport({ loop: true }));
    expect(clip().loop).toBe(true);
  });

  it('a replay starts it again from the top', async () => {
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    const v = clip();
    v.currentTime = 40;
    v.play = () => Promise.resolve();
    await send(transport({ replay_epoch: 1 }));
    expect(v.currentTime, 'Replay left the preacher’s copy where it was').toBe(0);
  });

  it('and a transport belongs to the clip it was pressed for', async () => {
    // The same decision `Output.svelte` records: a held clip must not hand its
    // Pause to whatever is sent next, or the following slide arrives frozen with
    // nothing in the product to say why.
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    await send(transport({ paused: true, loop: true }));
    await send({ kind: 'stage_media', media_url: 'http://x/media/8', media_kind: 'video' });
    expect(clip().loop, 'the next clip inherited the last one’s loop').toBe(false);
  });
});

// ── AND IT IS PLAYING THE SAME MOMENT AS EVERY OTHER SCREEN (RG-220) ────────
//
// The wiring, not the rule: `mediasync.test.js` holds what `syncSeek` decides
// and this holds that the page actually asks it and acts on the answer. Both
// halves are needed — a correct rule nothing calls is the shape of every defect
// in this repository's register.
describe('the clip on the stage is corrected onto Relay’s clock', () => {
  const clip = () => host.querySelector('video.slide');
  const at = (v, duration, currentTime) => {
    Object.defineProperty(v, 'duration', { value: duration, configurable: true });
    v.currentTime = currentTime;
  };

  it('a slide that arrives late is pulled to where the clip should be', async () => {
    await open(2);
    await send(ROLES);
    // Relay sent it ten seconds ago; this browser has just started playing.
    await send({
      kind: 'stage_media',
      media_url: 'http://x/media/7',
      media_kind: 'video',
      started_at: Date.now() - 10_000,
    });
    const v = clip();
    at(v, 120, 0.4);
    v.dispatchEvent(new Event('timeupdate'));
    await tick();
    expect(v.currentTime, 'the preacher is watching a different moment').toBeCloseTo(10, 0);
  });

  it('a slide already in step is left exactly alone — a seek is visible', async () => {
    await open(2);
    await send(ROLES);
    await send({
      kind: 'stage_media',
      media_url: 'http://x/media/7',
      media_kind: 'video',
      started_at: Date.now() - 10_000,
    });
    const v = clip();
    at(v, 120, 10.3);
    v.dispatchEvent(new Event('timeupdate'));
    await tick();
    expect(v.currentTime).toBe(10.3);
  });

  it('and an engine that sends no instant corrects nothing', async () => {
    // The older frame shape. A page that guessed a baseline would drag every
    // clip to zero on the first beat.
    await open(2);
    await send(ROLES);
    await send({ ...SLIDE, media_kind: 'video' });
    const v = clip();
    at(v, 120, 42);
    v.dispatchEvent(new Event('timeupdate'));
    await tick();
    expect(v.currentTime).toBe(42);
  });
});
