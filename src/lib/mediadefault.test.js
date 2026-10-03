// 2026-09-21 · M-1. A freshly fired clip looped (`loop={mediaTransport ? … : true}`)
// while the engine's clean state, the console's store and `Output.svelte`'s own
// comment all said "playing, not looping". So the Loop button read OFF while the
// wall looped, and the first Pause — which sends `looping: null`, leaving the
// engine's `false` in charge — silently un-looped a worship background in front
// of a congregation. One default, everywhere: a clip does not loop until Loop is
// pressed.
import { describe, it, expect, afterEach } from 'vitest';
import TemplateRender from './TemplateRender.svelte';
import { makeLayer } from './layers.js';

// jsdom implements no media playback: `play()` returns undefined and the renderer
// chains `.catch` on it, so give the element the promise a browser would.
HTMLMediaElement.prototype.play = () => Promise.resolve();
HTMLMediaElement.prototype.pause = () => {};

let host, app;
function mount(props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props });
  return host;
}
afterEach(() => { app?.$destroy(); host?.remove(); });

const layered = { id: 1, name: 'T', layout: { layers: [{ ...makeLayer('background'), id: 'bg', fill: '#000' }, { ...makeLayer('media'), id: 'm', x: 0, y: 0, w: 100, h: 100 }] }, style: {} };
const plain = { id: 2, name: 'P', layout: { layers: [{ ...makeLayer('background'), id: 'bg', fill: '#000' }] }, style: {} };
const CLIP = { kind: 'media', media_url: 'http://10.0.0.5:8032/media/9', media_kind: 'video' };

describe('a fired clip does not loop until somebody asks', () => {
  it('in a media layer', () => {
    const el = mount({ template: layered, content: CLIP });
    const v = el.querySelector('video');
    expect(v, 'the clip rendered').not.toBeNull();
    expect(v.loop).toBe(false);
  });
  it('full-frame, on a template with no media layer', () => {
    const el = mount({ template: plain, content: CLIP });
    expect(el.querySelector('video').loop).toBe(false);
  });
  it('and loops once the transport says so', () => {
    const el = mount({ template: plain, content: CLIP, mediaTransport: { paused: false, loop: true } });
    expect(el.querySelector('video').loop).toBe(true);
  });
});
