// HOW LONG IS LEFT OF THE CLIP, ON THE CLIP (RG-256).
//
// The operator: *"time left on the media should be on the stage display with a
// blur as earlier"*.
//
// **The "as earlier" is real, and it is not this page.** Checked rather than
// assumed: `git log -S 'blur' -- src/Stage.svelte` returns nothing, and
// `stage.html` has never had one. The blur being remembered is
// `.lprog.overmedia` in `TemplateRender.svelte` (RG-212) — a different page,
// backing the Stage Timer rail over a picture, carrying no clip clock at all.
//
// So the two halves have existed side by side and never met: this page has had a
// `Clip` figure since RG-213, in the rail or the figure row, with a flat wash,
// BESIDE the picture and never over it. What the operator is asking for is the
// figure moved onto the picture and given the plate the other page's rail
// already earns.
//
// **Why a plate at all.** It sits over a moving picture, so it cannot be a flat
// colour: an opaque bar punched through a clip is a worse answer than a clock
// nobody can read, which is the reasoning RG-212 recorded for the other rail.
// `backdrop-filter` is a progressive enhancement — where it is unsupported the
// wash alone still carries the figure.
import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import Stage from '../Stage.svelte';

const FILE = readFileSync(resolve('src/Stage.svelte'), 'utf8');
const CSS = FILE.slice(FILE.lastIndexOf('<style>'));

let app;
let host;
let socket;

class FakeSocket {
  constructor() {
    socket = this;
  }
  send() {}
  close() {}
}

afterEach(() => {
  app?.$destroy();
  host?.remove();
  app = null;
  host = null;
  window.history.replaceState({}, '', '/');
});

async function open() {
  window.history.replaceState({}, '', '/stage.html?channel=2');
  global.WebSocket = FakeSocket;
  global.fetch = () => Promise.resolve({ ok: true, json: () => Promise.resolve({}) });
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new Stage({ target: host });
  await tick();
  socket.onopen?.();
  // THIS SCREEN HAS TO BE A STAGE. `stage_media` is refused by any page that is
  // not one (`acceptsStageMessage`), so without this the clip never arrives and
  // every case below would pass or fail for the wrong reason.
  socket.onmessage({ data: JSON.stringify({ kind: 'channel_roles', roles: { 2: 'stage' } }) });
  await tick();
}

const send = (m) => socket.onmessage({ data: JSON.stringify(m) });

/** A clip on this screen, with a player that knows how long it is. */
async function playing(durationS, atS) {
  // `started_at` AGREES WITH `currentTime`. Written as `Date.now()` first, the
  // fixture claimed a clip that began this instant and was already five seconds
  // in — so `syncSeek` correctly dragged it back to zero and the case read
  // `2:05`. The sync was right and the fixture was wrong, which is worth leaving
  // written down: a test that fights a real rule usually loses for a reason.
  send({
    kind: 'stage_media',
    media_url: '/media/7',
    media_kind: 'video',
    started_at: Date.now() - atS * 1000,
  });
  await tick();
  const el = host.querySelector('video.slide');
  if (!el) return null;
  Object.defineProperty(el, 'duration', { value: durationS, configurable: true });
  Object.defineProperty(el, 'currentTime', { value: atS, configurable: true, writable: true });
  el.dispatchEvent(new Event('loadedmetadata'));
  await tick();
  return el;
}

describe('the clip clock on the preacher’s screen', () => {
  it('is painted OVER the clip, not beside it', async () => {
    await open();
    await playing(125, 5);
    const plate = host.querySelector('.clipplate');
    expect(plate, 'the clip clock is not on the picture').toBeTruthy();
    expect(plate.textContent).toContain('2:00');
  });

  it('and it is frosted, because a picture is moving underneath it', () => {
    // The rule, not the class: `class:` directives naming a class no stylesheet
    // defines have been caught in this repository before, and a test that only
    // asserted the spelling would pass over a plate that paints nothing.
    const rule = CSS.slice(CSS.indexOf('.clipplate {'), CSS.indexOf('}', CSS.indexOf('.clipplate {')));
    expect(rule, 'no rule for .clipplate').toBeTruthy();
    expect(rule).toMatch(/backdrop-filter:\s*blur\(/);
    // A WASH UNDERNEATH THE BLUR, so the figure is still readable where
    // `backdrop-filter` is not supported. A blur alone over a bright frame is a
    // white clock on a white picture.
    expect(rule, 'the plate has nothing behind the blur').toMatch(/background:/);
  });

  it('goes away with the clip, so the last clip cannot stand as a fact about now', async () => {
    await open();
    await playing(125, 5);
    expect(host.querySelector('.clipplate')).toBeTruthy();
    send({ kind: 'stage_media', media_url: null });
    await tick();
    expect(host.querySelector('.clipplate'), 'a clock for a clip that is gone').toBeNull();
  });

  it('a still picture gets no clock, because it does not end', async () => {
    await open();
    send({ kind: 'stage_media', media_url: '/media/9', media_kind: 'image' });
    await tick();
    expect(host.querySelector('.clipplate')).toBeNull();
  });

  it('says nothing until the player knows how long the clip is', async () => {
    await open();
    send({ kind: 'stage_media', media_url: '/media/7', media_kind: 'video', started_at: Date.now() });
    await tick();
    // NOT a zero and not a dash: a clip whose length is unknown has no clock
    // yet, and `0:00` would read as one that has finished.
    expect(host.querySelector('.clipplate')).toBeNull();
  });

  it('the figure row keeps the clock and the timer, and stops carrying the clip', async () => {
    // ONE ANSWER IN ONE PLACE. The `Clip` figure shared the rail with the time
    // of day and the Stage Timer, and the same number in two places is two
    // places that can disagree the first time either moves.
    await open();
    await playing(125, 5);
    const rail = host.querySelector('.figrow, .rail');
    expect(rail?.textContent ?? '', 'the clip is still in the rail as well').not.toMatch(/Clip/i);
  });
});
