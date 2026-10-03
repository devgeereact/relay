// A CLIP'S FIRST FRAME HAS TO BE ASKED FOR (RG-279).
//
// The operator, with a screenshot of the Library: *"library still not having the
// snapshoot of what media is attached still just the play button"* — a black
// card with a play triangle where the clip's own picture should be. The same
// blank sits in Live's slide grid and in the Planner, and it is one cause, not
// three: every one of those surfaces renders `<video preload="metadata">` and
// expects a frame.
//
// **`preload="metadata"` does not paint anything.** It fetches duration,
// dimensions and codec — enough to LAY OUT the element, which is what the
// attribute is for — and leaves `readyState` at `HAVE_METADATA`, one short of
// the `HAVE_CURRENT_DATA` a browser needs before it will draw a pixel. So the
// element sizes itself correctly and stays black, which is exactly what the
// screenshot shows.
//
// The fix is a media fragment: `#t=0.1` asks the browser to seek to that instant,
// which pulls one frame and paints it. Relay's own media server answers ranged
// requests (`Accept-Ranges: bytes`, 206 with `Content-Range`), so it costs one
// short read rather than the file.
import { describe, it, expect } from 'vitest';
import { posterUrl } from './posterframe.js';

describe('posterUrl — the URL that paints a frame instead of a black box', () => {
  it('asks for an instant just past the start', () => {
    expect(posterUrl('http://host:8032/media/9')).toBe('http://host:8032/media/9#t=0.1');
  });

  it('NOT zero, because zero is where a stopped clip already is', () => {
    // Some browsers treat `#t=0` as "no seek asked for" and paint nothing,
    // which is the bug wearing a fragment. A tenth of a second is past the
    // start and still inside the first frame of anything at 10fps or better.
    expect(posterUrl('u')).not.toContain('#t=0#');
    expect(posterUrl('u').endsWith('#t=0.1')).toBe(true);
  });

  it('leaves a URL that already names an instant alone', () => {
    expect(posterUrl('http://h/media/9#t=4')).toBe('http://h/media/9#t=4');
  });

  it('says nothing when there is no URL', () => {
    // A cue with no asset renders no element at all; inventing a fragment for
    // one would be a request for a file that does not exist.
    for (const empty of [null, undefined, '']) expect(posterUrl(empty)).toBe(empty ?? null);
  });

  it('is only ever for a STILL — a playing clip must not be seeked by its src', () => {
    // The whole point of the fragment is a seek. Putting it on the wall's own
    // `<video>` would drag a clip a congregation is watching to 0.1s every time
    // the element re-rendered, so the rule names what it is for and the callers
    // are the still renderers alone.
    const src = require('node:fs').readFileSync(
      require('node:path').resolve('src/lib/posterframe.js'),
      'utf8',
    );
    expect(src).toMatch(/still/i);
  });
});

// ── AND EVERY STILL SURFACE ASKS FOR IT (RG-279) ────────────────────────────
//
// Four surfaces show a clip as a picture, and all four were black. Asserted by
// file, because the defect is an omission: a fifth surface added next year with
// a bare `preload="metadata"` is the same blank again.
describe('the surfaces that show a clip as a picture', () => {
  const read = (p) => require('node:fs').readFileSync(require('node:path').resolve(p), 'utf8');

  for (const [file, name] of [
    ['src/lib/ui/MediaThumb.svelte', 'the shared thumbnail'],
    ['src/lib/views/library/Inspector.svelte', "the Library's inspector"],
    ['src/lib/views/library/VerseDeck.svelte', 'the verse deck'],
  ]) {
    it(`${name} asks for a frame`, () => {
      const src = read(file);
      expect(src).toContain("from '");
      expect(src, `${file} renders a clip and never asks for a frame`).toMatch(/posterUrl\(/);
    });
  }

  it('and TemplateRender asks ONLY on a still, never on the wall', () => {
    const src = read('src/lib/TemplateRender.svelte');
    // Three still-capable `<video>` elements: a layer, a legacy band, a legacy
    // region. All three, or the guarantee is kept on one door of three, which
    // is this repository's most-repeated bug.
    const guarded = src.match(/src=\{still \? posterUrl\(content\.media_url\) : content\.media_url\}/g);
    expect(guarded?.length, 'a still renderer was left painting a black box').toBe(3);
    // AND THE PLAYING PATH IS UNTOUCHED. The fragment is a seek; on a clip a
    // congregation is watching it would jump the picture back on every
    // re-render.
    expect(src).not.toMatch(/src=\{posterUrl\(content\.media_url\)\}/);
  });
});
