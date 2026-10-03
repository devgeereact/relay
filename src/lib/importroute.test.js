// A PROPRESENTER LIBRARY IS NOT ONE FLAT PILE.
//
// It is folders, and the grouping is the church's own, made deliberately over
// years. Measured in the real export, from its own `LibraryData` index: SONGS
// ~721, SCRIPTURES ~49, HMYN ~18, SPOKEN WORDS ~11, ANNOUNCEMENT ~4. Calling all
// 800 of them "songs" throws that away and makes the operator put it back by hand,
// one item at a time.
//
// It needs no protobuf. The grouping lives in `LibraryData`, which `proimport.rs`
// does not parse — but a folder import already knows which directory each file
// came from, and the index and the directory tree say the same thing.
import { describe, it, expect } from 'vitest';
import { sourceFolder, shelfFor, foldersIn } from './importroute.js';

const at = (path) => ({ name: path.split('/').pop(), webkitRelativePath: path });

describe('which folder a file came from', () => {
  it('takes the LAST directory, not the one the operator picked', () => {
    // The first is the same for every file in the pick and says nothing.
    expect(sourceFolder(at('rrr/SONGS/30 Billion.pro'))).toBe('SONGS');
    expect(sourceFolder(at('ProPresenter/Libraries/ANNOUNCEMENT/Countdown.pro'))).toBe(
      'ANNOUNCEMENT',
    );
  });

  it('says nothing when the browser said nothing', () => {
    // A hand-picked file carries no relative path, and guessing one from the name
    // would file things on a shelf nobody chose.
    expect(sourceFolder({ name: 'x.pro' })).toBe('');
    expect(sourceFolder(at('x.pro'))).toBe('');
    expect(sourceFolder(null)).toBe('');
  });
});

describe('which shelf it belongs on', () => {
  it('an announcements folder becomes announcements', () => {
    expect(shelfFor('ANNOUNCEMENT')).toBe('announcement');
    expect(shelfFor('ANNOUNCEMENTS')).toBe('announcement');
    expect(shelfFor('notices')).toBe('announcement');
  });

  it('songs, hymns and spoken word all have a song’s shape', () => {
    expect(shelfFor('SONGS')).toBe('song');
    // `HMYN` is in the real library and is a typo for HYMN. Matching is loose
    // because this is a folder a human named.
    expect(shelfFor('HMYN')).toBe('song');
    expect(shelfFor('SPOKEN WORDS')).toBe('song');
  });

  it('a SCRIPTURES folder is NOT filed as saved scripture, and that is deliberate', () => {
    // ProPresenter's SCRIPTURES folder holds slide DECKS of verses. Relay's
    // scripture collection holds actual verses out of the bundled KJV, with a
    // book, a chapter and a number. A deck has none of those, so filing it there
    // would put something in that collection which nothing there can navigate,
    // search or fire as a passage. It goes where its shape belongs.
    expect(shelfFor('SCRIPTURES')).toBe('song');
  });

  it('an unknown folder is a song rather than a refusal', () => {
    // A church can name a folder anything. The conservative answer is the one that
    // still imports the file; the report says which folder it came from, so an
    // operator can see where it went.
    expect(shelfFor('WORSHIP SET 2019')).toBe('song');
    expect(shelfFor('')).toBe('song');
  });
});

describe('what came from where, for the report', () => {
  it('counts by the folder name, which is the church’s word', () => {
    // An operator recognises `HMYN`. They do not recognise "18 items were
    // classified as songs".
    const files = [
      at('L/SONGS/a.pro'),
      at('L/SONGS/b.pro'),
      at('L/HMYN/c.pro'),
      at('L/ANNOUNCEMENT/d.pro'),
    ];
    expect(foldersIn(files)).toEqual([
      { folder: 'SONGS', count: 2, shelf: 'song' },
      { folder: 'HMYN', count: 1, shelf: 'song' },
      { folder: 'ANNOUNCEMENT', count: 1, shelf: 'announcement' },
    ]);
  });

  it('a hand-picked set has no folders to report, and says so by being empty', () => {
    expect(foldersIn([{ name: 'x.pro' }])).toEqual([]);
    expect(foldersIn([])).toEqual([]);
    expect(foldersIn(null)).toEqual([]);
  });
});
