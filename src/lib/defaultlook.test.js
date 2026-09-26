// THE DEFAULT LOOK REACHES THE SONGS AS WELL (RG-219).
//
// The operator's words: *"the defult theme selected should be whats activates
// for everysections of the application.... both song lyrics and bible slide...
// Media, announcements and Planner items carry the templetes set for them
// originally.....media stays same as it dosent need any templete"*.
//
// A church picks its look once. Scripture then wore it and a song did not: the
// lyrics content look is its own seeded row (`tpl_song`), and it is seeded on
// purpose — every other built-in is scripture-shaped, so a lyric rendered
// through one puts the song TITLE on the wall where the reference goes. The
// layout has to stay a lyric layout.
//
// **So what is inherited is the STYLE and not the layout**, which is the
// operator's ruling and also the only version of this that does not put a song
// title on a congregation screen. A song keeps its own arrangement of the words
// and takes the default template's palette, typeface and background — which is
// exactly what "theme" meant before themes were folded into templates
// (DECISIONS §87), and those keys are still the same flat `style` bag.
//
// TWO REFUSALS, both in the operator's own sentence:
//
//   · media, announcements and planner cues keep their own look entirely;
//   · a screen with a template OF ITS OWN is untouched — DECISIONS §29 says the
//     screen's own template is authoritative, and an operator who assigned one
//     per screen must keep seeing exactly it.
import { describe, it, expect } from 'vitest';
import { resolveOutputTemplate } from './layers.js';

const DEFAULT_TPL = {
  id: 1,
  name: 'House look',
  layout: { layers: [{ id: 'bg', type: 'background', fill: '#101018' }] },
  style: { background: '#101018', verseColor: '#f4efe6', font: 'Fraunces' },
};
const SONG_LOOK = {
  id: 2,
  name: 'Worship Lyrics',
  layout: { layers: [{ id: 'w', type: 'text', bind: 'verse', size: 5 }] },
  style: { background: '#000000', verseColor: '#ffffff', font: 'Inter' },
};
const MEDIA_LOOK = { id: 3, name: 'Full bleed', layout: { layers: [] }, style: { background: '#222' } };
const SCREEN_OWN = { id: 4, name: 'Lower third', layout: { layers: [] }, style: { background: '#010101' } };

describe('a screen that FOLLOWS the look takes the house style for words', () => {
  it('a song keeps its own layout and wears the default style', () => {
    const t = resolveOutputTemplate(null, SONG_LOOK, false, DEFAULT_TPL, null, 'song');
    expect(t.layout, 'the lyric layout was replaced — the title would go on the wall').toEqual(
      SONG_LOOK.layout,
    );
    expect(t.style.font).toBe('Fraunces');
    expect(t.style.background).toBe('#101018');
    expect(t.style.verseColor).toBe('#f4efe6');
    // The look's own row is not mutated: two screens resolving in the same tick
    // must not see each other's answer.
    expect(SONG_LOOK.style.font).toBe('Inter');
  });

  it('scripture does the same, which is what makes the two agree', () => {
    const t = resolveOutputTemplate(null, SONG_LOOK, false, DEFAULT_TPL, null, 'scripture');
    expect(t.style.font).toBe('Fraunces');
  });

  it('media, announcements and planner cues keep their own look entirely', () => {
    for (const kind of ['media', 'announce', 'countdown']) {
      const t = resolveOutputTemplate(null, MEDIA_LOOK, false, DEFAULT_TPL, null, kind);
      expect(t, `${kind} was restyled`).toBe(MEDIA_LOOK);
    }
  });

  it('and a kind nobody named inherits nothing — silence is not consent', () => {
    expect(resolveOutputTemplate(null, MEDIA_LOOK, false, DEFAULT_TPL, null, null)).toBe(MEDIA_LOOK);
    expect(resolveOutputTemplate(null, MEDIA_LOOK, false, DEFAULT_TPL)).toBe(MEDIA_LOOK);
  });
});

describe('what it must not touch', () => {
  it('a screen with a template of its own is untouched (DECISIONS §29)', () => {
    const t = resolveOutputTemplate(SCREEN_OWN, SONG_LOOK, false, DEFAULT_TPL, null, 'song');
    expect(t, 'the per-screen template was restyled by the house look').toBe(SCREEN_OWN);
  });

  it('a cue the operator pinned is their deliberate choice, and keeps its own style', () => {
    const t = resolveOutputTemplate(SCREEN_OWN, SONG_LOOK, true, DEFAULT_TPL, null, 'song');
    expect(t).toBe(SONG_LOOK);
  });

  it('with no default configured, nothing is inherited and nothing is copied', () => {
    expect(resolveOutputTemplate(null, SONG_LOOK, false, null, null, 'song')).toBe(SONG_LOOK);
  });

  it('a default with no style of its own changes nothing', () => {
    const bare = { id: 9, name: 'Bare', layout: { layers: [] } };
    expect(resolveOutputTemplate(null, SONG_LOOK, false, bare, null, 'song')).toBe(SONG_LOOK);
  });
});
