// RG-11 — the moat, measured rather than asserted.
//
// `docs/LANGUAGES.md` states the truth in prose: 66 of 66 books in all three tier-1
// languages, NONE reviewed by anyone who speaks them, word error rate NEVER measured
// in any language, and Yorùbá numerals not parsed at all.
//
// Prose cannot be tracked. A contributor who fixes eleven Yorùbá book names has no
// way to see they moved anything, and a reader has no way to tell whether the
// document is current. These tests hold the two rules that make the instrument worth
// having:
//
//   1. Every number is derived from the data the binary SHIPS, so the report cannot
//      flatter the product.
//   2. Nothing unmeasured is ever rendered as a number.
import { describe, it, expect } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

const ROOT = path.resolve(__dirname, '../..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');
const settings = read('src/lib/views/Settings.svelte');
const rs = read('src-tauri/src/detection.rs');

describe('the report is derived, never asserted', () => {
  it('reads the same file the detector loads', () => {
    // A report derived from a different reading of the same data would be a second
    // answer to one question — and this exists precisely to be trusted about the
    // state of that data.
    const fn = rs.slice(rs.indexOf('pub fn language_report()'));
    expect(fn.slice(0, 600)).toMatch(/include_str!\("\.\.\/data\/book_aliases\.json"\)/);
    expect(fn.slice(0, 600)).toMatch(/include_str!\("\.\.\/data\/numerals\.json"\)/);
  });

  it('counts only books the detector can actually key on', () => {
    // A typo in the data file is a name no transcript will ever match. Counting it
    // would make the report improve as the data got worse.
    const fn = rs.slice(rs.indexOf('pub fn language_report()'));
    expect(fn.slice(0, 1600)).toMatch(/CANONICAL_BOOKS\.iter\(\)\.any/);
  });

  it('the shipped data still has all three tier-1 languages', () => {
    // A sanity check on the fixture the report describes. If a language disappears
    // from the file, the Settings table would silently show two rows.
    const aliases = JSON.parse(read('src-tauri/data/book_aliases.json'));
    expect(Object.keys(aliases).filter((k) => !k.startsWith('_')).sort()).toEqual(['ha', 'sw', 'yo']);
  });

  it('Yorùbá numerals are still absent from the data — the largest known gap', () => {
    // Yorùbá is subtractive (16 = ẹrìndínlógún) and the largest addressable market
    // of the three. If somebody adds them, this fails and LANGUAGES.md gets updated,
    // which is the correct direction for a test like this to break.
    const numerals = JSON.parse(read('src-tauri/data/numerals.json'));
    expect(Object.keys(numerals).filter((k) => !k.startsWith('_')).sort()).toEqual(['ha', 'sw']);
  });
});

describe('nothing unmeasured is rendered as a number', () => {
  it('accuracy renders as "not measured", and there is no number to render', () => {
    expect(settings).toMatch(/not measured/);
    // The field is null in Rust, so there is nothing the view could print even if
    // it tried. Pinned on the Rust side too.
    expect(rs).toMatch(/pub wer: Option<f32>/);
    expect(rs).toMatch(/\*\*Always `None`\.\*\*/);
  });

  it('native review renders as "not yet", never as a percentage', () => {
    expect(settings).toMatch(/not yet/);
    expect(settings).not.toMatch(/reviewed.*\{.*%/);
    expect(rs).toMatch(/pub native_reviewed: bool/);
    expect(rs).toMatch(/\*\*Always false/);
  });

  it('says WHY accuracy is empty, and what it would take', () => {
    // An empty column with no explanation reads as a bug. This one is the honest
    // state of the moat, and the reader is told what closing it costs.
    expect(settings).toMatch(/has never been measured/);
    expect(settings).toMatch(/thirty minutes of\s*\n?\s*real preaching/);
  });

  it('names the gap that matters most, and how a non-programmer closes it', () => {
    // A wrong alias does not fail safely — it puts the wrong scripture on a wall.
    expect(settings).toMatch(/none has been\s*\n?\s*checked by somebody who speaks the language/i);
    expect(settings).toMatch(/book_aliases\.json/);
    expect(settings).toMatch(/no code required/);
  });

  it('an absence is dim, never red — nobody has failed here', () => {
    const css = settings.slice(settings.indexOf('.s-langgap{'));
    expect(css.slice(0, 120)).toMatch(/--v-faint/);
    expect(css.slice(0, 120)).not.toMatch(/--v-rose|--v-amber/);
  });
});

describe('it is reachable', () => {
  it('has its own Settings section', () => {
    expect(settings).toMatch(/key: 'languages'/);
    expect(settings).toMatch(/section === 'languages'/);
  });

  it('shows console-text coverage from the catalogues, not from a claim', () => {
    // The locale files ship near-empty on purpose. 0% is an invitation, not a
    // failure to hide — and it is computed, so it cannot be stale.
    expect(settings).toMatch(/coverage\(l\.code\)/);
  });
});
