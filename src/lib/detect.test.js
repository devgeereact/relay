// The operator must be able to see WHAT KIND of claim the AI is making.
//
// Relay's whole safety architecture is one distinction: a reference the parser
// actually HEARD may auto-fire; a paraphrase GUESS may never auto-fire, at any
// score, at any sensitivity (router.rs::decide, docs/DECISIONS.md). The gate is
// airtight in Rust and property-tested there.
//
// And the console threw the distinction away. `method` was in the IPC payload the
// whole time; Live.svelte rendered both kinds as "AI suggestion — 92% match". The
// human in the loop was shown nothing to be a human in the loop WITH.
import { describe, it, expect } from 'vitest';
import { heard, methodKey, methodBadgeKey, methodNoteKey, showsConfidence, inLibrary, evidenceIsASpan, orderClaims } from './detect.js';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { codeOnly } from './codeonly.js';

const direct = { method: 'direct', confidence: 0.92 };
const semantic = { method: 'semantic', confidence: 0.61 };
const ambiguous = { method: 'ambiguous', confidence: 0.7 };

describe('heard vs guessed', () => {
  it('a spoken reference is HEARD', () => {
    expect(heard(direct)).toBe(true);
  });

  it('a paraphrase is NOT heard, however high its score', () => {
    expect(heard({ method: 'semantic', confidence: 0.99 })).toBe(false);
  });

  it('an ambiguous reference is not heard either — its confidence is a placeholder', () => {
    // detection.rs hardcodes 0.70 for these. It is not a measurement of anything.
    expect(heard(ambiguous)).toBe(false);
  });

  it('the three methods get three DIFFERENT keys — the operator must be able to tell them apart', () => {
    const keys = [direct, semantic, ambiguous].map(methodKey);
    expect(new Set(keys).size).toBe(3);
    expect(methodKey(semantic)).toBe('live.paraphrase_a_guess');
    expect(methodKey(direct)).toBe('live.heard_the_reference');
  });
});

describe('confidence is only shown where it means something', () => {
  it('a heard reference shows its confidence', () => {
    expect(showsConfidence(direct)).toBe(true);
  });

  // THE POINT OF THIS FILE.
  //
  // A semantic score is a TF-IDF cosine — a distance in an arbitrary vector space.
  // Rendering "61%" next to it invites the operator to read it as "61% likely to be
  // right", which is precisely what it is not. A number that lies is worse than no
  // number at all, because it looks like information and gets acted on.
  it('a paraphrase NEVER shows a percentage — at any score', () => {
    for (const c of [0.1, 0.35, 0.61, 0.95, 0.99]) {
      expect(showsConfidence({ method: 'semantic', confidence: c })).toBe(false);
    }
  });

  it('an ambiguous match never shows one either', () => {
    expect(showsConfidence(ambiguous)).toBe(false);
  });
});

describe('a guessed BOOK NAME is a guess, and says so', () => {
  // THE 2026-08-14 P0, on the frontend side. `uncertain_book` is the one method
  // whose confidence is a genuine, well-calibrated parse confidence — of the wrong
  // question. The parse of "number three sixteen" really is 0.84 certain, about a
  // word nobody said. That makes it the most dangerous number in the product, and
  // the only defence on this side is that it is never printed and never described
  // as something Relay heard.
  const guessedBook = { method: 'uncertain_book', reference: 'Numbers 3:16', confidence: 0.84 };

  it('is NOT heard, however confident the parse was', () => {
    expect(heard(guessedBook)).toBe(false);
    for (const c of [0.3, 0.77, 0.84, 0.96, 0.99]) {
      expect(heard({ ...guessedBook, confidence: c })).toBe(false);
    }
  });

  it('never shows a percentage', () => {
    for (const c of [0.3, 0.77, 0.84, 0.96, 0.99]) {
      expect(showsConfidence({ ...guessedBook, confidence: c })).toBe(false);
    }
  });

  it('does not describe itself as a reference Relay heard', () => {
    expect(methodKey(guessedBook)).not.toBe('live.heard_the_reference');
  });

  it('is distinguishable from all three other kinds of claim', () => {
    // Four methods, four keys. The operator is the human in the loop and cannot be
    // one while being shown the same sentence for four different situations.
    const keys = [
      methodKey({ method: 'direct' }),
      methodKey({ method: 'semantic' }),
      methodKey({ method: 'ambiguous' }),
      methodKey(guessedBook),
    ];
    expect(new Set(keys).size).toBe(4);
  });
});

describe('nothing crashes on a malformed payload', () => {
  // A detection card that throws takes the console down MID-SERVICE. Degrade to the
  // cautious reading instead — but note that an unknown method reads as "heard",
  // matching the Rust default, so a future method must be added in BOTH places.
  it('survives undefined', () => {
    expect(() => methodKey(undefined)).not.toThrow();
    expect(heard(undefined)).toBe(false);
  });
});

describe('a suggestion whose verse does not exist', () => {
  it('is recognised from the flag the backend already sends', () => {
    expect(inLibrary({ reference: 'Psalms 23:99', in_library: false })).toBe(false);
    expect(inLibrary({ reference: 'Psalms 23:1', in_library: true })).toBe(true);
  });

  it('treats an ABSENT flag as present — the warning may only ever be added on evidence', () => {
    // The LAN remote and any older payload do not set it. Greying out a real
    // suggestion because a field was missing would be a fault invented from an
    // absence, which is the same mistake in the other direction.
    expect(inLibrary({ reference: 'John 3:16' })).toBe(true);
    expect(inLibrary(undefined)).toBe(true);
    expect(inLibrary(null)).toBe(true);
  });

  it('is independent of HOW it was found — a guess can resolve, a heard one can fail', () => {
    // "Psalms 23:99" is heard, confidently, and does not exist. The two questions
    // are orthogonal and the operator needs both answered.
    const heardButAbsent = { method: 'direct', confidence: 0.94, in_library: false };
    expect(heard(heardButAbsent)).toBe(true);
    expect(inLibrary(heardButAbsent)).toBe(false);
  });
});

// ── THE CARD NAMES THE METHOD, AND AN IMPORT THAT IS NEVER CALLED FAILS ──────
//
// `methodKey` was imported into `Live.svelte` and called nowhere. The claim card
// rendered `heard(d) ? 'Heard' : 'Paraphrase'`, so `semantic`, `ambiguous` and
// `uncertain_book` all wore one word — and `uncertain_book` is the method added
// after "please turn to hymn number three sixteen" put Numbers 3:16 in front of a
// congregation, which CLAUDE.md rule 10 calls the claim an operator most needs to
// look at. On the surface they read, it was a paraphrase.
//
// The note beneath was worse: "not a spoken reference" is TRUE of a paraphrase and
// FALSE of the other two. An ambiguous reference WAS spoken; for `uncertain_book`
// the chapter and the verse were both heard and only the book was repaired. One
// sentence for three methods told the operator the opposite of what happened on
// the two where it mattered.
describe('a claim card says which KIND of claim it is', () => {
  const liveRaw = readFileSync(resolve(process.cwd(), 'src/lib/views/Live.svelte'), 'utf8');
  // Comments stripped for the `not.toMatch` below. The fix's own comment QUOTES the
  // defect it replaced, and an assertion over the prose would fail on an honest
  // note about a deletion — the lesson `panic.test.js` records in the same words.
  const live = codeOnly(liveRaw);

  it('every method gets its own chip', () => {
    const seen = new Set(
      ['direct', 'semantic', 'ambiguous', 'uncertain_book'].map((m) => methodBadgeKey({ method: m })),
    );
    expect(seen.size, 'four methods must not share a chip').toBe(4);
  });

  it('and only a paraphrase is called "not a spoken reference"', () => {
    expect(methodNoteKey({ method: 'semantic' })).toBe('live.not_a_spoken_reference');
    expect(methodNoteKey({ method: 'ambiguous' })).not.toBe('live.not_a_spoken_reference');
    expect(methodNoteKey({ method: 'uncertain_book' })).not.toBe('live.not_a_spoken_reference');
    // A heard reference shows a bar instead, so it has no note at all.
    expect(methodNoteKey({ method: 'direct' })).toBeNull();
  });

  it('the run surface renders them rather than a hard-coded pair of words', () => {
    expect(live, 'the chip must come from the register').toContain('$t(methodBadgeKey(d))');
    expect(live, 'and so must the note').toContain('$t(methodNoteKey(d))');
    // The defect, in its original form. Watched to fail by restoring it.
    expect(live).not.toMatch(/heard\(d\)\s*\?\s*'Heard'\s*:\s*'Paraphrase'/);
  });

  // THE PATTERN, not just this instance. Three instruments in this repository were
  // built well and never wired: `methodKey` imported and never called, rule 37's
  // `onFit` reaching one surface out of six, and `legibility.review` reading a
  // model no shipped template uses. The first is the cheapest to make impossible.
  it('nothing is imported from detect.js and then never used', () => {
    for (const file of ['src/lib/views/Live.svelte', 'src/lib/DetectionInspector.svelte']) {
      const src = readFileSync(resolve(process.cwd(), file), 'utf8');
      const m = src.match(/import \{([^}]+)\} from ['"][^'"]*detect\.js['"]/);
      if (!m) continue;
      const body = codeOnly(src).replace(m[0], '');
      for (const name of m[1].split(',').map((x) => x.trim()).filter(Boolean)) {
        expect(
          new RegExp(`\\b${name}\\b`).test(body),
          `${file} imports \`${name}\` from detect.js and never calls it — that is how the claim card came to render two hard-coded words for four methods`,
        ).toBe(true);
      }
    }
  });
});

// ── QUOTED SCRIPTURE, AND THE PRESENTATION THAT WAS LYING ──────────────────
//
// The operator, 2026-09-20: *"It has to be exactly how it is in the scripture,
// not scattered. THE LORD IS MY SHEPHERD, not THE . SHEPHERD . LORD."*
//
// Two defects sat behind that sentence. The matcher was a TF-IDF cosine, which
// discards word order by construction, and the console rendered its evidence
// (`terms.join(" · ")`) INSIDE QUOTATION MARKS, as `“lord · shepherd”`. The
// second is the one these tests hold: whatever the matcher does, a word list
// must never be dressed as a quotation.
describe('quoted scripture is a different KIND of claim', () => {
  const quoted = { method: 'quoted', matched_text: 'the lord is my shepherd', confidence: 0.6 };
  const para = { method: 'semantic', matched_text: 'lord · shepherd', confidence: 0.6 };
  const direct = { method: 'direct', matched_text: 'psalm twenty three', confidence: 0.95 };

  it('is never counted as heard, so it can never auto-fire', () => {
    // The whole of rule 10. A preacher quotes far more scripture than a
    // congregation is shown, so reading a verse aloud is not asking for it.
    expect(heard(quoted)).toBe(false);
  });

  it('shows no percentage, because a run length is not a probability', () => {
    expect(showsConfidence(quoted)).toBe(false);
  });

  it('gets its own words, not the paraphrase ones', () => {
    expect(methodKey(quoted)).toBe('live.quoted_scripture');
    expect(methodBadgeKey(quoted)).toBe('live.badge_quoted');
    expect(methodNoteKey(quoted)).toBe('live.note_quoted');
    // And it must not have quietly taken the paraphrase's.
    expect(methodKey(quoted)).not.toBe(methodKey(para));
    expect(methodBadgeKey(quoted)).not.toBe(methodBadgeKey(para));
  });

  it('may be shown in quotation marks; a paraphrase may NOT', () => {
    expect(evidenceIsASpan(quoted)).toBe(true);
    expect(evidenceIsASpan(direct)).toBe(true);
    // THE DEFECT. `“lord · shepherd”` is a quotation of something nobody said.
    expect(evidenceIsASpan(para)).toBe(false);
    expect(evidenceIsASpan({ method: 'ambiguous' })).toBe(false);
    expect(evidenceIsASpan({ method: 'uncertain_book' })).toBe(false);
    expect(evidenceIsASpan(null)).toBe(false);
  });
});

// 2026-09-20 · service 24. A bare "verse 1" answered from the passage in memory
// reached the wall as Psalms 55:1 while the preacher quoted Hosea 6:1. It now
// arrives as `uncertain_book` with `matched_text` "verse 1", and the operator must
// be able to tell THAT case from a misheard book word: nobody said any book here,
// Relay assumed the one on the screen.
describe('a verse whose book came from memory', () => {
  const fromMemory = { method: 'uncertain_book', matched_text: 'verse 1', reference: 'Psalms 55:1' };
  const misheard = { method: 'uncertain_book', matched_text: 'room two twelve', reference: 'Romans 2:12' };

  it('says so, in all three registers', () => {
    expect(methodKey(fromMemory)).toBe('live.book_from_memory');
    expect(methodBadgeKey(fromMemory)).toBe('live.badge_from_memory');
    expect(methodNoteKey(fromMemory)).toBe('live.note_from_memory');
  });

  it('and a misheard book word keeps its own words', () => {
    expect(methodKey(misheard)).toBe('live.book_name_uncertain');
    expect(methodBadgeKey(misheard)).toBe('live.badge_book_uncertain');
  });

  it('is never shown a percentage and never reads as heard', () => {
    expect(showsConfidence(fromMemory)).toBe(false);
    expect(heard(fromMemory)).toBe(false);
  });
});

// 2026-09-21 · measured in a browser (RG-192). With three claims pending at
// 1440×900 the third card's Accept & fire sat below the fold of its own column,
// and newest-first meant the one that fell off was the oldest — the HEARD one,
// the only class that can auto-fire. A heard reference outranks a guess in the
// column, whatever arrived last.
describe('the claim column puts a heard reference first', () => {
  const heardOld = { method: 'direct', reference: 'Numbers 10:29' };
  const guessNew = { method: 'semantic', reference: 'John 3:16' };
  const memNew = { method: 'uncertain_book', reference: 'Psalms 55:1', matched_text: 'verse 1' };
  it('heard before every other kind, each group keeping its arrival order', () => {
    expect(orderClaims([guessNew, memNew, heardOld]).map((d) => d.reference)).toEqual([
      'Numbers 10:29',
      'John 3:16',
      'Psalms 55:1',
    ]);
  });
  it('is stable for a list of one kind', () => {
    expect(orderClaims([guessNew, memNew]).map((d) => d.reference)).toEqual(['John 3:16', 'Psalms 55:1']);
    expect(orderClaims([])).toEqual([]);
  });
});

// ── A VERSE RELAY HEARD BEING READ (DECISIONS §118) ──────────────────────────
//
// The operator's instruction of 2026-09-23 lifted the auto-fire cap for one new
// method and one only. The console has to say which one it is looking at, for
// exactly the reason §21 exists: the gate is airtight in Rust and invisible in
// the one place a human can act on it.
describe('a reading', () => {
  const reading = { method: 'reading', confidence: 0.86, matched_text: 'in him should not perish but have everlasting life' };

  it('is not dressed as a heard reference', () => {
    // `heard` means Relay heard the REFERENCE. It did not — it heard the verse.
    // The two are different claims and the card must not merge them.
    expect(heard(reading)).toBe(false);
  });

  // A run length is a COUNT. Printing it as a percentage would be the exact
  // mistake §21 was written about, arriving through a new door.
  it('shows no percentage, at any score', () => {
    expect(showsConfidence(reading)).toBe(false);
    expect(showsConfidence({ ...reading, confidence: 0.99 })).toBe(false);
  });

  it('has its own words, not the paraphrase ones', () => {
    expect(methodKey(reading)).toBe('live.read_aloud');
    expect(methodBadgeKey(reading)).toBe('live.badge_reading');
    expect(methodNoteKey(reading)).toBe('live.note_reading');
    // And it must not fall through to the paraphrase sentence, which promises a
    // guarantee that no longer holds for this method.
    expect(methodKey(reading)).not.toBe('live.heard_the_reference');
    expect(methodNoteKey(reading)).not.toBe('live.not_a_spoken_reference');
  });

  it('carries a contiguous span of speech, so it may be quoted', () => {
    expect(evidenceIsASpan(reading)).toBe(true);
  });
});
