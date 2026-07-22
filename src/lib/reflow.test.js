import { describe, it, expect } from 'vitest';
import { parseLyrics, toText, reflow, tagFor } from './reflow.js';

describe('parseLyrics', () => {
  it('ends a section at a blank line', () => {
    const s = parseLyrics('line one\nline two\n\nline three');
    expect(s).toHaveLength(2);
    expect(s[0].lyrics).toBe('line one\nline two');
    expect(s[1].lyrics).toBe('line three');
  });

  it('reads the header styles an operator actually types', () => {
    for (const head of ['[Chorus]', 'Chorus', 'chorus:', 'CHORUS']) {
      expect(parseLyrics(`${head}\nsing it`)[0]).toMatchObject({
        label: 'Chorus',
        lyrics: 'sing it',
      });
    }
    expect(parseLyrics('Verse 2\nwords')[0].label).toBe('Verse 2');
    expect(parseLyrics('V2\nwords')[0].label).toBe('Verse 2');
    expect(parseLyrics('[Bridge x2]\nwords')[0].label).toBe('Bridge x2');
  });

  it('numbers unnamed blocks as verses, counting only the unnamed ones', () => {
    const s = parseLyrics('one\n\n[Chorus]\nsing\n\ntwo');
    expect(s.map((x) => x.label)).toEqual(['Verse 1', 'Chorus', 'Verse 2']);
  });

  it('keeps a header whose body is still empty', () => {
    // Deleting what someone just typed is the one thing an editor may not do.
    const s = parseLyrics('[Bridge]');
    expect(s).toHaveLength(1);
    expect(s[0]).toMatchObject({ label: 'Bridge', lyrics: '' });
  });

  it('survives CRLF, trailing space and runs of blank lines', () => {
    const s = parseLyrics('a\r\nb  \r\n\r\n\r\n\r\nc\n\n\n');
    expect(s.map((x) => x.lyrics)).toEqual(['a\nb', 'c']);
  });

  it('is empty for empty input', () => {
    for (const v of ['', '   \n\n  ', null, undefined]) expect(parseLyrics(v)).toEqual([]);
  });
});

describe('toText round trip', () => {
  it('survives text → sections → text → sections', () => {
    const src = '[Verse 1]\nline a\nline b\n\n[Chorus]\nsing it loud';
    const once = parseLyrics(src);
    const twice = parseLyrics(toText(once));
    expect(twice).toEqual(once);
    expect(toText(once)).toBe(src);
  });
});

describe('reflow', () => {
  const six = [{ tag: 'V1', label: 'Verse 1', lyrics: '1\n2\n3\n4\n5\n6' }];

  it('breaks a long section into readable slides', () => {
    const s = reflow(six, { linesPerSlide: 4 });
    expect(s).toHaveLength(2);
    expect(s[0].lyrics).toBe('1\n2\n3\n4');
    expect(s[1].lyrics).toBe('5\n6');
  });

  it('says WHICH part a broken section is on', () => {
    // The operator has to know a chorus is 1 of 2, or they will cut it short.
    expect(reflow(six, { linesPerSlide: 4 }).map((s) => s.label)).toEqual([
      'Verse 1 (1/2)',
      'Verse 1 (2/2)',
    ]);
  });

  it('does not label a section that fits on one slide', () => {
    expect(reflow(six, { linesPerSlide: 8 })[0].label).toBe('Verse 1');
  });

  it('REFLOWS when the rule changes — the whole point', () => {
    expect(reflow(six, { linesPerSlide: 2 })).toHaveLength(3);
    expect(reflow(six, { linesPerSlide: 6 })).toHaveLength(1);
    expect(reflow(six, { linesPerSlide: 1 })).toHaveLength(6);
  });

  it('splits an over-long slide at a LINE boundary, never mid-word', () => {
    const long = [{ label: 'V', lyrics: 'aaaaaaaaaa\nbbbbbbbbbb\ncccccccccc' }];
    const s = reflow(long, { linesPerSlide: 4, maxChars: 22 });
    expect(s.length).toBeGreaterThan(1);
    for (const sl of s) for (const line of sl.lyrics.split('\n')) expect(line.length).toBe(10);
  });

  it('drops blank lines inside a section rather than projecting a gap', () => {
    expect(reflow([{ label: 'V', lyrics: 'a\n\n   \nb' }], { linesPerSlide: 4 })[0].lyrics).toBe(
      'a\nb',
    );
  });

  it('keeps a named but empty section visible in the deck', () => {
    const s = reflow([{ label: 'Bridge', lyrics: '' }], { linesPerSlide: 4 });
    expect(s).toHaveLength(1);
    expect(s[0]).toMatchObject({ label: 'Bridge', lyrics: '' });
  });

  it('guards against a nonsense rule instead of looping or emptying the deck', () => {
    for (const n of [0, -3, NaN, undefined, 'x']) {
      expect(reflow(six, { linesPerSlide: n }).length).toBeGreaterThan(0);
    }
  });

  it('is empty for no sections', () => {
    expect(reflow([], {})).toEqual([]);
    expect(reflow(null, {})).toEqual([]);
  });
});

describe('tagFor', () => {
  it('makes a short corner tag', () => {
    expect(tagFor('Verse 1')).toBe('V1');
    expect(tagFor('Chorus')).toBe('C');
    expect(tagFor('Pre-Chorus 2')).toBe('P2');
    expect(tagFor('')).toBe('');
  });
});
