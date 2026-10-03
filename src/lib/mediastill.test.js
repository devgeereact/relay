// A THUMBNAIL IS A STILL, NOT A PLAYING CLIP (RG-235).
//
// The operator: *"media on the slide should just show the first frame of the
// video not playing the video...as this keeps the app distracting when
// playing"*. Four video clips in a plan meant four clips playing at once in the
// slide grid, under the one clip that is actually on air — on the surface an
// operator works from for the whole service.
//
// **`still` is a property of the RENDER, not of the content.** The same clip is
// live on a wall and a thumbnail in a deck at the same instant, and only one of
// them is playing. So it is a prop on the renderer, defaulting to false: every
// surface that paints a congregation screen is unchanged by construction, and a
// surface has to ASK to be still.
//
// A still is not a paused clip. `preload="metadata"` fetches enough for the
// first frame and never the file, so twenty cues are twenty small requests
// rather than twenty downloads over a church's network.
import { describe, it, expect } from 'vitest';
import { tick } from 'svelte';

const TemplateRender = (await import('./TemplateRender.svelte')).default;

const TPL = { layout: { layers: [{ id: 'bg', type: 'background', fill: '#000' }] }, style: {} };
const CLIP = { kind: 'media', media_url: 'http://x/media/4', media_kind: 'video' };

let host;
let app;
async function render(props) {
  host = document.createElement('div');
  document.body.appendChild(host);
  app = new TemplateRender({ target: host, props: { template: TPL, ...props } });
  await tick();
  await tick();
  return host.querySelector('video');
}
const teardown = () => { app?.$destroy(); host?.remove(); app = null; host = null; };

describe('still — what a deck cell asks for', () => {
  it('a still clip does not autoplay, and never loops', async () => {
    try {
      const v = await render({ content: CLIP, still: true });
      expect(v, 'the clip did not render at all').toBeTruthy();
      expect(v.hasAttribute('autoplay'), 'a thumbnail is playing').toBe(false);
      expect(v.loop, 'a thumbnail is looping').toBe(false);
    } finally {
      teardown();
    }
  });

  it('and fetches a frame rather than the file', async () => {
    try {
      const v = await render({ content: CLIP, still: true });
      expect(v.getAttribute('preload')).toBe('metadata');
    } finally {
      teardown();
    }
  });

  it('the wall is untouched — still is opt-in, so every screen plays as before', async () => {
    try {
      const v = await render({ content: CLIP });
      expect(v.hasAttribute('autoplay'), 'the wall stopped playing its clip').toBe(true);
    } finally {
      teardown();
    }
  });

  it('a still clip reports nothing, because it is not what any screen is showing', async () => {
    // `onMedia` is how a screen says where its clip is. A thumbnail answering
    // would put a deck cell's position into the readout an operator times the
    // next cue against — RG-222's failure, from the other side.
    const seen = [];
    try {
      await render({ content: CLIP, still: true, onMedia: (m) => seen.push(m) });
      expect(seen.filter(Boolean), 'a thumbnail reported itself as a screen').toEqual([]);
    } finally {
      teardown();
    }
  });
});
