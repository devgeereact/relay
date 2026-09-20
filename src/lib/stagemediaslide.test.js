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
