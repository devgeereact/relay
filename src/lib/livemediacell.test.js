// A MEDIA CUE IN THE RUN SURFACE'S DECK IS A PICTURE (RG-225).
//
// The operator, running the packaged build: *"Also Media still not showing the
// Preview..."*.
//
// The Planner's inspector and the Library's media pane both paint the file;
// RG-215 put a thumbnail on the Planner's two lists. Live's deck — the grid an
// operator steps through on a Sunday — did not, and the reason is one line:
// `cellContent` builds `{ reference, text, translation }` and has never carried
// `media_url` or `media_kind`. `TemplateRender` can paint a picture and was
// never handed one, so a media cue rendered as an empty slide, which on a dark
// template is indistinguishable from a cue with nothing in it.
//
// TWO HALVES, and they are tested apart because they fail apart:
//
//   · `slidegrid.js` has to carry the cue's media id onto the cell. It reads the
//     payload already for everything else on that cell;
//   · Live has to turn that id into a URL through the SHARED builder
//     (`bundledbackgrounds.js::mediaUrl`), which is the same rule the wall, the
//     Library and the Planner use — including the `bundled:` case, where a
//     picture Relay ships has no file under `/media/<id>` at all (DECISIONS §90).
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';
import { planCells } from './slidegrid.js';

const LIVE = readFileSync(resolve(__dirname, 'views/Live.svelte'), 'utf8');

const mediaItem = (over = {}) => ({
  id: 31,
  label: 'sunrise.jpg',
  cue_type: 'media',
  payload_json: JSON.stringify({ media_id: 7, kind: 'image', filename: 'sunrise.jpg' }),
  ...over,
});
/** What `slidesOf` returns for a media cue: a placeholder with no words. */
const slidesOf = () => [{ tag: 'BG', label: 'Background', text: '' }];

describe('the cell carries what the cue actually puts on a screen', () => {
  it('a media cue names its asset and its kind', () => {
    const [cell] = planCells([mediaItem()], slidesOf);
    expect(cell.mediaId, 'the cell cannot say which picture this is').toBe(7);
    expect(cell.mediaKind).toBe('image');
  });

  it('a video cue says video, because the two are painted by different elements', () => {
    const item = mediaItem({ payload_json: JSON.stringify({ media_id: 8, kind: 'video' }) });
    expect(planCells([item], slidesOf)[0].mediaKind).toBe('video');
  });

  it('a cue with no asset claims none — an id guessed from a label is a broken picture', () => {
    const item = mediaItem({ payload_json: JSON.stringify({ filename: 'gone.jpg' }) });
    expect(planCells([item], slidesOf)[0].mediaId).toBeNull();
  });

  it('and a cue that is not media never carries one', () => {
    const verse = mediaItem({
      cue_type: 'scripture',
      payload_json: JSON.stringify({ reference: 'John 3:16', media_id: 7 }),
    });
    // Belt and braces: even a payload that happens to carry the key is ignored
    // for a kind that does not paint a picture, so a hand-edited plan cannot
    // put a background behind a verse without saying so.
    expect(planCells([verse], slidesOf)[0].mediaId).toBeNull();
  });

  it('an empty cue keeps the same shape, so a reader never has to ask which kind it got', () => {
    const [cell] = planCells([mediaItem()], () => []);
    expect(cell.empty).toBe(true);
    expect(cell).toHaveProperty('mediaId');
    expect(cell).toHaveProperty('mediaKind');
  });
});

describe('and the run surface turns that id into the picture', () => {
  const src = codeOnly(LIVE);

  it('cellContent hands the renderer the two fields it paints from', () => {
    // A BLOCK body now, not an expression one: it has a lookup to do first.
    const at = src.indexOf('cellContent = (c) => {');
    expect(at, 'cellContent is no longer identifiable').toBeGreaterThan(-1);
    const body = src.slice(at, src.indexOf('\n  };', at));
    expect(body, 'a media cue still renders as an empty slide').toMatch(/media_url:/);
    expect(body).toMatch(/media_kind:/);
  });

  it('through the SHARED builder, not a URL typed a fourth time', () => {
    // `main.rs::media_url` builds it for the wall; the Library, the Planner and
    // now this surface all call one function instead. A copy here would be the
    // door that forgets `bundled:` and shows a broken picture for every seeded
    // background — which is exactly what the Library's own note records.
    expect(src, 'Live builds its own media URL').toMatch(/mediaUrl\(/);
    expect(src).toMatch(/from '\.\.\/bundledbackgrounds\.js'/);
  });
});
