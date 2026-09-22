// THE PREVIEW SHOWS THE PICTURE, NOT THE FILENAME (RG-236).
//
// The operator: *"instead you can put the video count down on the preview
// screen instead of having the name of the media on the screen....but if it is
// image...just show the next image as preview"*.
//
// The preview pane builds `{ reference, text }` from a cell's LABEL, so a media
// cue previewed as its own filename typeset at 37px — `IMG_3427.mov` across a
// congregation template, which is neither what is coming nor anything a screen
// will ever show.
//
// So a media cue previews as the media: the picture itself for a still, and for
// a clip the first frame with **how long is left of the one on air** over it —
// which is the figure an operator is actually timing the next cue against, and
// the reason it belongs on the pane that answers "what is next".
import { describe, it, expect } from 'vitest';
import { previewOfCell } from './previewcell.js';

const IMAGE = { label: 'sunrise.jpg', ctype: 'media', mediaId: 7, mediaKind: 'image', text: '' };
const CLIP = { label: 'IMG_3427.mov', ctype: 'media', mediaId: 8, mediaKind: 'video', text: '' };
const VERSE = { label: 'John 3:16', ctype: 'scripture', text: 'For God so loved the world' };
const url = (c) => (c?.mediaId ? `http://host:8032/media/${c.mediaId}` : null);

describe('previewOfCell — what the next-up pane paints', () => {
  it('a picture previews as the picture', () => {
    const p = previewOfCell(IMAGE, url(IMAGE));
    expect(p.media_url).toBe('http://host:8032/media/7');
    expect(p.media_kind).toBe('image');
    // AND NOT AS ITS FILENAME. `sunrise.jpg` set at 37px across a scripture
    // template is a slide no screen will ever show.
    expect(p.reference, 'the filename is still being typeset').toBeFalsy();
    expect(p.text).toBe('');
  });

  it('a clip previews as its first frame, with no words either', () => {
    const p = previewOfCell(CLIP, url(CLIP));
    expect(p.media_url).toBe('http://host:8032/media/8');
    expect(p.media_kind).toBe('video');
    expect(p.reference).toBeFalsy();
  });

  it('a verse is untouched — this is about media and nothing else', () => {
    const p = previewOfCell(VERSE, null);
    expect(p.reference).toBe('John 3:16');
    expect(p.text).toBe('For God so loved the world');
    expect(p.media_url).toBeFalsy();
  });

  it('and a media cue whose asset has gone keeps its words rather than painting nothing', () => {
    // The three-way answer again: no id, a deleted row, a resolved one. A cue
    // with no picture to show is better described by its name than by an empty
    // frame, which reads as a cue with nothing in it.
    const p = previewOfCell(CLIP, null);
    expect(p.media_url).toBeFalsy();
    expect(p.reference).toBe('IMG_3427.mov');
  });
});
