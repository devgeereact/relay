// THE TRANSCRIPT SAYS WHEN IT WAS SAID, NOT WHEN IT ARRIVED (RG-263).
//
// The operator: *"make the live transcript capture everything being said without
// hiding or dismissing any part... and with its time code/timestamp"*.
//
// The first half is RG-262, in `stt.rs`. This is the second, and it was nearly
// free: `TranscriptUpdate` has carried `timestamp_ms` — the position of the
// newest audio in the capture stream — since the module was written, and the
// console destructured it away. `grep -rn "timestamp_ms" src/` returned nothing
// at all.
//
// So the card stamped each line with `new Date()` at the moment the decode
// LANDED IN THE WEBVIEW, which is up to eight seconds plus a decode after the
// words were spoken, and at second resolution. On a 93-minute service that is a
// stamp nobody can line up against a recording.
import { describe, it, expect } from 'vitest';
import { applyTranscript, stampOf } from './stores/capture.js';

const line = (over = {}) => ({ text: 'and he said unto them', is_final: true, ...over });

describe('stampOf — the time code a line carries', () => {
  it('is the position in the capture, counted from the first sample', () => {
    // A TIME CODE, not a clock: this is what a recording of the service would
    // show at the same instant, which is the thing an operator lines a
    // transcript up against.
    expect(stampOf(0)).toBe('0:00:00');
    expect(stampOf(61_000)).toBe('0:01:01');
    expect(stampOf(3_599_000)).toBe('0:59:59');
  });

  it('carries hours, because services run past one', () => {
    // The export format `fmt_secs` prints `93:31` for a 93-minute service — a
    // figure that reads as ninety-three seconds to anybody who has not been
    // told otherwise.
    expect(stampOf(5_611_300)).toBe('1:33:31');
  });

  it('says nothing rather than zero when there is no position', () => {
    // A line with no time code is a line from an older build or a path that
    // never carried one. `0:00:00` would place it at the start of the service.
    expect(stampOf(null)).toBe('');
    expect(stampOf(undefined)).toBe('');
    expect(stampOf(Number.NaN)).toBe('');
  });
});

describe('the reducer keeps the stamp the line was given', () => {
  it('stores the time code beside the line, in lockstep', () => {
    let t = { partial: '', finals: [], finalsAt: [] };
    t = applyTranscript(t, line(), '0:00:04');
    t = applyTranscript(t, line({ text: 'come unto me' }), '0:00:12');
    expect(t.finals).toEqual(['and he said unto them', 'come unto me']);
    expect(t.finalsAt).toEqual(['0:00:04', '0:00:12']);
  });

  it('a partial is still one line being revised', () => {
    // RG-262 did not change this and must not: a partial is a revision of the
    // window that is open, not a new line.
    let t = { partial: '', finals: [], finalsAt: [] };
    t = applyTranscript(t, line({ is_final: false, text: 'and he' }), '0:00:01');
    t = applyTranscript(t, line({ is_final: false, text: 'and he said' }), '0:00:02');
    expect(t.partial).toBe('and he said');
    expect(t.finals).toEqual([]);
  });
});

describe('the console keeps what the engine sent', () => {
  it('the listener reads timestamp_ms rather than throwing it away', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const src = readFileSync(resolve('src/lib/stores/capture.js'), 'utf8');
    //
    // COMMENTS STRIPPED, AND THE DESTRUCTURE ASSERTED BY NAME. This case read
    // the raw body for the word `timestamp_ms` and passed on the COMMENT that
    // explains the change, while the destructure beside it never gained the
    // field — so `stampOf(timestamp_ms)` was a `ReferenceError` that crashed
    // the console thirteen times before an operator's screenshot found it.
    // A scanner that matches prose is a scanner that matches anything.
    const listener = src.slice(src.indexOf("listen('stt://transcript'"));
    const body = listener.slice(0, listener.indexOf('});')).replace(/\/\/[^\n]*/g, '');
    expect(body, 'the field is never taken off the payload').toMatch(
      /const \{[^}]*\btimestamp_ms\b[^}]*\} = e\.payload/,
    );
    expect(body, 'the line is stamped with something else').toMatch(/stampOf\(timestamp_ms\)/);
  });

  it('and the card renders it', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const dock = readFileSync(resolve('src/lib/Dock.svelte'), 'utf8');
    expect(dock).toMatch(/class="tt r-mono">\{l\.at\}/);
  });
});
