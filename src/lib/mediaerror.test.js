// 2026-09-21 · M-3 / O-4. Six media elements in the renderer and not one `on:error`.
// A 404, a codec the webview cannot decode and a CSP refusal were the same
// observable event — nothing — while the beat still said `content` and the console
// printed On Air in amber over a blank frame. The renderer now reports a media
// element that failed, and reports it healed, through one callback.
import { describe, it, expect, afterEach } from 'vitest';
import { tick } from 'svelte';
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

const bg = { ...makeLayer('background'), id: 'bg', fill: '#000' };
const plain = { id: 2, name: 'P', layout: { layers: [bg] }, style: {} };
const layered = { id: 1, name: 'T', layout: { layers: [bg, { ...makeLayer('media'), id: 'm', x: 0, y: 0, w: 100, h: 100 }] }, style: {} };
const PIC = { kind: 'media', media_url: 'http://10.0.0.5:8032/media/3', media_kind: 'image' };
const CLIP = { kind: 'media', media_url: 'http://10.0.0.5:8032/media/9', media_kind: 'video' };

describe('a picture or clip that fails to load says so', () => {
  for (const [name, template] of [['full-frame', plain], ['media layer', layered]]) {
    it(`${name}: an image error is reported, and a load clears it`, async () => {
      const seen = [];
      const el = mount({ template, content: PIC, onMediaError: (e) => seen.push(e) });
      const img = el.querySelector('img');
      expect(img).not.toBeNull();
      img.dispatchEvent(new Event('error'));
      await tick();
      expect(seen.at(-1)).toEqual({ kind: 'image', url: PIC.media_url });
      img.dispatchEvent(new Event('load'));
      await tick();
      expect(seen.at(-1)).toBeNull();
    });
    it(`${name}: a clip error is reported too`, async () => {
      const seen = [];
      const el = mount({ template, content: CLIP, onMediaError: (e) => seen.push(e) });
      el.querySelector('video').dispatchEvent(new Event('error'));
      await tick();
      expect(seen.at(-1)).toEqual({ kind: 'video', url: CLIP.media_url });
    });
  }
  it('a new clip forgets the last one\'s failure', async () => {
    const seen = [];
    const el = mount({ template: plain, content: CLIP, onMediaError: (e) => seen.push(e) });
    el.querySelector('video').dispatchEvent(new Event('error'));
    await tick();
    app.$set({ content: { ...CLIP, media_url: 'http://10.0.0.5:8032/media/10' } });
    await tick();
    expect(seen.at(-1)).toBeNull();
  });
});
