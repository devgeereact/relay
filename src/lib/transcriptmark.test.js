// WHICH LINES OF THE TRANSCRIPT DID RELAY HEAR SCRIPTURE IN (RG-278).
//
// The operator: *"can we introduce color coding to the transcript? when you hear
// any paraphrasing of the scripture or an actual scripture can it be colour
// coded? eg - psalm 23 verse 1, and if preacher says 'the lord is my shepherd'
// kind of thing"*.
//
// Two different claims, and the whole of rule 10 is the difference between them:
// a STATED reference is something Relay heard, a PARAPHRASE is something it
// guessed from meaning. The Live transcript card printed both, and neither, in
// one ink.
//
// The rule is tested here rather than through the card, because a substring
// match over normalised text is where this will go wrong — a needle that is
// empty, a needle that straddles a word boundary, a claim that belongs to a
// different line entirely. The card is driven in `dockcolour` at the bottom of
// this file so the ink is proved to reach the DOM rather than only the source.
//
//   npx vitest run src/lib/transcriptmark.test.js
//
// CLAUDE.md rule 18 (which KIND of claim) · rule 10 (only Direct was heard)
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { tick } from 'svelte';
import { normalise, markFor, rememberMarks, lineKey } from './transcriptmark.js';

const claim = (over = {}) => ({
  reference: 'Psalms 23:1',
  method: 'direct',
  confidence: 0.91,
  matched_text: 'psalm 23 verse 1',
  in_library: true,
  ...over,
});

describe('normalise — the two texts are compared as words, not as bytes', () => {
  it('folds case and punctuation away', () => {
    // The transcript is whisper's prose ("Psalm 23, verse 1.") and `matched_text`
    // is the parser's own slice of it. They are the same words and they are very
    // often not the same string.
    expect(normalise('Psalm 23, verse 1.')).toBe('psalm 23 verse 1');
    expect(normalise('  THE   LORD is my SHEPHERD  ')).toBe('the lord is my shepherd');
  });

  it('keeps accented letters, because three of the four tier-1 languages need them', () => {
    // A byte-class regex would strip the tone marks off a Yorùbá line and then
    // fail to match the claim that came from it. `\p{L}` is the whole point.
    expect(normalise('Jòhánù orí kẹta!')).toBe('jòhánù orí kẹta');
  });

  it('answers with an empty string for nothing at all', () => {
    expect(normalise(null)).toBe('');
    expect(normalise(undefined)).toBe('');
    expect(normalise('  ,,, ')).toBe('');
  });
});

describe('markFor — what kind of mark this line carries', () => {
  it('is nothing when there are no claims', () => {
    expect(markFor({ t: 'and he said unto them', at: '0:00:04' }, [])).toBe(null);
    expect(markFor({ t: 'and he said unto them', at: '0:00:04' }, null)).toBe(null);
    expect(markFor({ t: 'and he said unto them', at: '0:00:04' }, undefined)).toBe(null);
  });

  it('is HEARD when the claim is a stated reference that is in this line', () => {
    const m = markFor({ t: 'Turn with me to Psalm 23, verse 1.', at: '0:01:00' }, [claim()]);
    expect(m).toBeTruthy();
    expect(m.kind).toBe('heard');
    expect(m.reference).toBe('Psalms 23:1');
  });

  it('is a GUESS when the claim is a paraphrase', () => {
    // "the lord is my shepherd" — the operator's own example. Nobody said a
    // reference; Relay matched the meaning, and its confidence is a cosine.
    const m = markFor(
      { t: 'The Lord is my shepherd, I shall not want.', at: '0:02:00' },
      [claim({ method: 'semantic', matched_text: 'the lord is my shepherd' })],
    );
    expect(m.kind).toBe('guess');
    expect(m.reference).toBe('Psalms 23:1');
  });

  it('is nothing when the claim came from some other line', () => {
    // The commonest case by far: one claim in hand, forty lines on screen. A
    // mark on a line the claim did not come from is a lie about where Relay
    // heard it.
    expect(markFor({ t: 'good morning everybody', at: '0:00:01' }, [claim()])).toBe(null);
  });

  it('never marks on an empty or whitespace-only evidence string', () => {
    // `''` is a substring of every line, so an unguarded `includes` paints the
    // whole transcript off one malformed claim.
    expect(markFor({ t: 'good morning everybody', at: '0:00:01' }, [claim({ matched_text: '' })])).toBe(null);
    expect(markFor({ t: 'good morning everybody', at: '0:00:01' }, [claim({ matched_text: '  ,. ' })])).toBe(null);
    expect(markFor({ t: 'good morning everybody', at: '0:00:01' }, [claim({ matched_text: null })])).toBe(null);
  });

  it('never marks a claim that names no verse', () => {
    // The mark's whole job is to say WHICH verse. One with nothing to name is
    // an ink with no information under it.
    expect(markFor({ t: 'psalm 23 verse 1', at: '0:00:01' }, [claim({ reference: '' })])).toBe(null);
  });

  it('does not match across a word boundary', () => {
    // " psalm 2 " is not in " psalm 23 ", and a bare `includes` says it is.
    // Marking the wrong verse beside a line is the transcript-card version of
    // firing the wrong verse.
    expect(
      markFor({ t: 'Psalm 23 verse 1', at: '0:00:01' }, [claim({ reference: 'Psalms 2:1', matched_text: 'psalm 2' })]),
    ).toBe(null);
  });

  it('a heard reference wins over a guess on the same line, in either order', () => {
    const direct = claim({ reference: 'Psalms 23:1', matched_text: 'psalm 23 verse 1' });
    const guess = claim({ reference: 'John 10:11', method: 'semantic', matched_text: 'the lord is my shepherd' });
    const line = { t: 'Psalm 23 verse 1 — the Lord is my shepherd.', at: '0:03:00' };
    expect(markFor(line, [direct, guess]).reference).toBe('Psalms 23:1');
    expect(markFor(line, [guess, direct]).reference).toBe('Psalms 23:1');
  });

  it('every method that is not Direct is a guess, because none of them was heard', () => {
    // rule 10. `quoted` is scripture read aloud, `ambiguous` parsed but
    // undecidable, `uncertain_book` a book nobody said. All three are capped at
    // Suggest by the router at any score, so none may wear the stronger ink.
    const line = { t: 'the lord is my shepherd', at: '0:04:00' };
    // `reading` is in this list although DECISIONS §118 lets it reach a wall
    // unattended, and that is the point: it may ACT alone and its reference was
    // still never spoken. Relay inferred which verse from the words. It was
    // missing here because it was added to the product after RG-278 landed.
    for (const method of ['semantic', 'quoted', 'reading', 'ambiguous', 'uncertain_book']) {
      const m = markFor(line, [claim({ method, matched_text: 'the lord is my shepherd' })]);
      expect(m.kind, `${method} must never read as heard`).toBe('guess');
    }
    expect(markFor(line, [claim({ method: 'direct', matched_text: 'the lord is my shepherd' })]).kind).toBe('heard');
  });

  it('carries the claim, so the card can name the method without a second table', () => {
    const m = markFor({ t: 'the lord is my shepherd', at: '0:04:00' }, [
      claim({ method: 'semantic', matched_text: 'the lord is my shepherd' }),
    ]);
    expect(m.claim.method).toBe('semantic');
  });

  it('says nothing about a screen', () => {
    // The card is a record of what Relay HEARD. A claim that auto-fired and a
    // claim that was dismissed leave the same mark, because the same thing was
    // heard in both cases — and nothing in the mark may imply a verse went out.
    const fired = markFor({ t: 'psalm 23 verse 1', at: '0:05:00' }, [claim({ status: 'auto' })]);
    const pending = markFor({ t: 'psalm 23 verse 1', at: '0:05:00' }, [claim({ status: 'suggested' })]);
    // The key set is the assertion, so a field added later has to come past this
    // line and justify itself. `run` did, on 2026-09-25: it is a count of words
    // the preacher said, which is a fact about the SPEECH and cannot change with
    // the operator's decision — asserted below rather than assumed.
    expect(Object.keys(fired).sort()).toEqual(['claim', 'kind', 'reference', 'run']);
    expect(fired.kind).toBe(pending.kind);
    expect(fired.run).toBe(pending.run);
  });
});

describe('rememberMarks — a line stays marked after the claim has gone', () => {
  const lines = [
    { t: 'good morning everybody', at: '0:00:01' },
    { t: 'Turn with me to Psalm 23, verse 1.', at: '0:01:00' },
  ];

  it('marks only the line the claim came from', () => {
    const marks = rememberMarks(lines, [claim()], new Map());
    expect(marks[0]).toBe(null);
    expect(marks[1].kind).toBe('heard');
  });

  it('keeps the mark once the claim has left the operator’s hands', () => {
    // THE CASE THIS EXISTS FOR. `detections` holds pending suggestions for 45
    // seconds and `resolvedDetections` keeps four receipts — and an AUTO-FIRE
    // never enters the pending list at all. So the strongest claim of the
    // service is the one that disappears soonest, and without a memo the ink
    // would blink out from under the operator while the words stayed on screen.
    const memo = new Map();
    rememberMarks(lines, [claim()], memo);
    const later = rememberMarks(lines, [], memo);
    expect(later[1].kind).toBe('heard');
    expect(later[0]).toBe(null);
  });

  it('never invents a mark for a line that was never claimed', () => {
    const memo = new Map();
    rememberMarks(lines, [claim()], memo);
    const other = rememberMarks([{ t: 'let us pray', at: '0:09:00' }], [], memo);
    expect(other[0]).toBe(null);
  });

  it('a later, stronger claim on the same line replaces a remembered guess', () => {
    // The paraphrase arrives first (the preacher quotes it), the reference a
    // window later. The line was heard, and the mark must say so.
    const memo = new Map();
    const line = [{ t: 'Psalm 23 verse 1, the Lord is my shepherd', at: '0:06:00' }];
    rememberMarks(line, [claim({ method: 'semantic', matched_text: 'the lord is my shepherd' })], memo);
    expect(rememberMarks(line, [claim()], memo)[0].kind).toBe('heard');
  });

  it('does not let a guess overwrite a remembered heard reference', () => {
    const memo = new Map();
    const line = [{ t: 'Psalm 23 verse 1, the Lord is my shepherd', at: '0:06:00' }];
    rememberMarks(line, [claim()], memo);
    const after = rememberMarks(line, [claim({ method: 'semantic', matched_text: 'the lord is my shepherd' })], memo);
    expect(after[0].kind).toBe('heard');
  });

  it('is bounded, so a three-hour service cannot grow it for ever', () => {
    const memo = new Map();
    for (let i = 0; i < 1200; i++) {
      rememberMarks([{ t: `psalm 23 verse 1 number ${i}`, at: `0:${i}` }], [claim()], memo);
    }
    expect(memo.size).toBeLessThanOrEqual(600);
  });

  it('a line is identified by its time code AND its words', () => {
    // `finalsAt` can repeat — two decodes land in the same second — so the stamp
    // alone is not an identity.
    expect(lineKey({ t: 'a', at: '0:00:01' })).not.toBe(lineKey({ t: 'b', at: '0:00:01' }));
  });
});

// ── AND THE CARD PAINTS IT ──────────────────────────────────────────────────
//
// Driven, not read. This repository has twice shipped a defect behind a test
// that only grepped its own source, so the Dock is mounted against real stores
// and the class is read off the real DOM.
describe('the Live transcript card wears the mark', () => {
  vi.mock('@tauri-apps/api/core', () => ({ invoke: async () => null }));
  vi.mock('@tauri-apps/api/event', () => ({ listen: async () => () => {} }));

  let cap;
  let Dock;
  let host;
  let app;

  beforeEach(async () => {
    HTMLCanvasElement.prototype.getContext = () => null;
    cap = await import('./stores/capture.js');
    Dock = (await import('./Dock.svelte')).default;
    cap.transcript.set({ partial: '', finals: [], finalsAt: [] });
    cap.detections.set([]);
    cap.resolvedDetections.set([]);
  });

  const mount = async () => {
    host = document.createElement('div');
    document.body.appendChild(host);
    app = new Dock({ target: host, props: {} });
    await tick();
    await tick();
    return host;
  };

  const rows = () => [...host.querySelectorAll('.tbody .trl')];

  it('paints a paraphrase in the guess ink and names the verse', async () => {
    cap.transcript.set({
      partial: '',
      finals: ['good morning everybody', 'The Lord is my shepherd, I shall not want.'],
      finalsAt: ['0:00:01', '0:02:00'],
    });
    cap.detections.set([
      claim({ method: 'semantic', matched_text: 'the lord is my shepherd', confidence: 0.61 }),
    ]);
    await mount();
    const [plain, guessed] = rows();
    expect(plain.className).not.toMatch(/mk-/);
    expect(guessed.className).toMatch(/mk-guess/);
    expect(guessed.querySelector('.tref')?.textContent).toContain('Psalms 23:1');
    // A cosine is not a probability, and 61% beside a paraphrase is a number
    // that lies (rule 18). Nothing on this line may carry one.
    expect(guessed.textContent).not.toMatch(/%/);
  });

  it('paints a stated reference differently from a paraphrase', async () => {
    cap.transcript.set({
      partial: '',
      finals: ['Turn with me to Psalm 23, verse 1.'],
      finalsAt: ['0:01:00'],
    });
    cap.detections.set([claim()]);
    await mount();
    const [heardRow] = rows();
    expect(heardRow.className).toMatch(/mk-heard/);
    expect(heardRow.className).not.toMatch(/mk-guess/);
    expect(heardRow.querySelector('.tref')?.textContent).toContain('Psalms 23:1');
  });

  it('marks nothing when nothing was claimed', async () => {
    cap.transcript.set({ partial: '', finals: ['good morning everybody'], finalsAt: ['0:00:01'] });
    await mount();
    expect(rows()[0].className).not.toMatch(/mk-/);
    expect(rows()[0].querySelector('.tref')).toBe(null);
  });

  // ── AND THE STRENGTH REACHES THE DOM ──────────────────────────────────────
  //
  // rule 35, stated as a test rather than as an intention: no claim, a weak claim
  // and a strong claim must READ DIFFERENTLY. If two of the three look the same
  // it is not a ranking, it is a decoration.
  const READ_ALOUD = 'the lord is my shepherd i shall not want he maketh me lie down';

  it('prints a quotation’s run as a count of words, with its unit and no percentage', async () => {
    cap.transcript.set({ partial: '', finals: ['The Lord is my shepherd.'], finalsAt: ['0:02:00'] });
    cap.detections.set([claim({ method: 'quoted', matched_text: 'the lord is my shepherd', confidence: 0.77 })]);
    await mount();
    const chip = rows()[0].querySelector('.tref');
    expect(chip.textContent).toContain('Psalms 23:1');
    expect(chip.textContent, 'the run, in words').toMatch(/5 words/);
    // rule 18. The unit is the whole point: "5 words" cannot be read as a
    // likelihood and "77%" beside a run length would be exactly that.
    expect(rows()[0].textContent).not.toMatch(/%/);
  });

  it('marks a reading as the strong tier, and a shorter quotation as not', async () => {
    cap.transcript.set({ partial: '', finals: ['The Lord is my shepherd.', READ_ALOUD], finalsAt: ['0:02:00', '0:03:00'] });
    cap.detections.set([
      claim({ reference: 'Psalms 23:2', method: 'quoted', matched_text: 'the lord is my shepherd' }),
      claim({ reference: 'Psalms 23:1', method: 'reading', matched_text: READ_ALOUD }),
    ]);
    await mount();
    const [quoted, reading] = rows();
    expect(reading.className, 'a reading is the strong tier').toMatch(/mk-read/);
    expect(quoted.className, 'a five-word run is not').not.toMatch(/mk-read/);
    expect(reading.querySelector('.tref').textContent).toMatch(/14 words/);
    expect(reading.textContent).not.toMatch(/%/);
  });

  it('shows NO measure on a paraphrase, an ambiguous reference or an assumed book', async () => {
    // The refused half, and the one worth being loudest about. A cosine has no
    // length, `ambiguous` carries a placeholder, and `uncertain_book` carries a
    // REAL number about a word nobody said — the shape that put Numbers 3:16 in
    // front of a congregation (rule 10). A strength badge on any of the three is
    // a promotion of the claim an operator most needs to look at.
    cap.transcript.set({
      partial: '',
      finals: ['The Lord is my shepherd.', 'Revelation 22.', 'Hymn number three sixteen.'],
      finalsAt: ['0:02:00', '0:04:00', '0:05:00'],
    });
    cap.detections.set([
      claim({ method: 'semantic', matched_text: 'the lord is my shepherd' }),
      claim({ reference: 'Revelation 22:1', method: 'ambiguous', matched_text: 'revelation 22' }),
      claim({ reference: 'Numbers 3:16', method: 'uncertain_book', matched_text: 'hymn number three sixteen' }),
    ]);
    await mount();
    for (const row of rows()) {
      expect(row.querySelector('.tref'), 'every line is still marked').toBeTruthy();
      expect(row.className, 'and none of them is the strong tier').not.toMatch(/mk-read/);
      expect(row.querySelector('.trun'), `${row.textContent} carries a measure it has not earned`).toBe(null);
      expect(row.textContent).not.toMatch(/%/);
    }
  });

  it('shows no measure on a heard reference either, because its evidence is the reference', async () => {
    cap.transcript.set({ partial: '', finals: ['Turn with me to Psalm 23, verse 1.'], finalsAt: ['0:01:00'] });
    cap.detections.set([claim()]);
    await mount();
    expect(rows()[0].className).toMatch(/mk-heard/);
    expect(rows()[0].querySelector('.trun')).toBe(null);
    expect(rows()[0].textContent).not.toMatch(/%/);
  });

  it('reads differently with no claim, a weak claim and a strong one — rule 35', async () => {
    cap.transcript.set({
      partial: '',
      finals: ['good morning everybody', 'The Lord is my shepherd.', READ_ALOUD],
      finalsAt: ['0:00:01', '0:02:00', '0:03:00'],
    });
    cap.detections.set([
      claim({ reference: 'Psalms 23:2', method: 'quoted', matched_text: 'the lord is my shepherd' }),
      claim({ reference: 'Psalms 23:1', method: 'reading', matched_text: READ_ALOUD }),
    ]);
    await mount();
    const [none, weak, strong] = rows();
    // The svelte scope class is stripped along with `trl`: it is on every row and
    // says nothing, and leaving it in would make all three "differ".
    const reading = (r) => [
      r.className.replace(/\btrl\b|\bsvelte-\S+/g, '').trim(),
      r.querySelector('.tref')?.textContent ?? '',
    ];
    const three = [reading(none), reading(weak), reading(strong)].map((x) => x.join(' | '));
    expect(new Set(three).size, `two of the three read the same: ${three.join(' / ')}`).toBe(3);
    expect(three[0]).toBe(' | ');
  });

  it('the chip takes its own line before it starves the words (RG-304)', async () => {
    // A SOURCE GUARD, AND IT SAYS SO. jsdom lays nothing out, so this cannot
    // prove the row's height — the measurement is in the fix's own comment,
    // taken in a real engine at the card's widths. What it CAN hold is the two
    // properties the behaviour rests on, either of which reads like tidying:
    // `min-width: 0` on `.tx` is the standard flex incantation and restoring it
    // brings the tower back, and `flex-wrap` on a row of three items looks
    // gratuitous until the third one is 194px wide and refuses to shrink.
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const { codeOnly } = await import('./codeonly.js');
    const css = codeOnly(readFileSync(resolve('src/lib/Dock.svelte'), 'utf8'));
    const rule = (sel) => (css.match(new RegExp(`${sel.replace(/[.\-\s]/g, '\\$&')}\\s*\\{([^}]*)\\}`)) || [])[1] || '';
    const row = rule('.trl');
    const text = rule('.trl .tx');
    expect(row, '.trl has no rule').toBeTruthy();
    expect(row, 'the chip must be able to wrap to its own line').toMatch(/flex-wrap:\s*wrap/);
    expect(text, 'the words need a floor, or the chip takes the row').toMatch(/min-width:\s*11em/);
    expect(text, 'a floor of zero is no floor').not.toMatch(/min-width:\s*0\b/);
    // The basis must be 0, not auto: with `auto` the sentence's own content width
    // is the hypothetical size and the chip wraps on every row at every width.
    expect(text, 'flex-basis must be 0, not auto').toMatch(/flex:\s*1 1 0/);
  });

  it('the strong tier adds no ink that is promised elsewhere', async () => {
    // Nothing here may borrow a promise: amber is ON AIR, amethyst is rehearsal,
    // ochre is a caution and a reading is not one, grey is CUED. There was no free
    // ink for a third tier, so it is a WEIGHT step inside the cyan it already
    // wears — an unheard claim is what it still is.
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const { codeOnly } = await import('./codeonly.js');
    const css = codeOnly(readFileSync(resolve('src/lib/Dock.svelte'), 'utf8'));
    const rules = [...css.matchAll(/\.trl\.mk-read[^{]*\{([^}]*)\}|\.trun[^{]*\{([^}]*)\}/g)]
      .map((m) => m[1] ?? m[2] ?? '')
      .join(' ');
    expect(rules, 'the strong tier and the run count have no rule at all').toBeTruthy();
    for (const promise of ['--v-amber', '--v-amethyst', '--v-caution', '--v-grey', '--v-emerald']) {
      expect(rules, `the strength tier must not wear ${promise}`).not.toContain(promise);
    }
  });

  it('the guess ink is cyan and the heard ink is neither promise colour', async () => {
    const { readFileSync } = await import('node:fs');
    const { resolve } = await import('node:path');
    const { codeOnly } = await import('./codeonly.js');
    const css = codeOnly(readFileSync(resolve('src/lib/Dock.svelte'), 'utf8'));
    const rule = (sel) => (css.match(new RegExp(`${sel.replace(/[.\-]/g, '\\$&')}\\s*\\{([^}]*)\\}`)) || [])[1] || '';
    expect(rule('.trl.mk-guess .tref'), 'a guess is cyan, rule 18').toContain('--v-cyan');
    const heardInk = rule('.trl.mk-heard .tref');
    expect(heardInk, '.trl.mk-heard .tref has no rule').toBeTruthy();
    for (const promise of ['--v-amber', '--v-amethyst', '--v-cyan', '--v-caution', '--v-grey']) {
      expect(heardInk, `a heard reference must not wear ${promise}`).not.toContain(promise);
    }
  });
});

// ── HOW STRONG WAS THE MATCH — AND THE HALF THAT MUST NOT BE BUILT ──────────
//
// The operator, 2026-09-25: *"let the transcript be colour coded with the highest
// match so its easy to filter through what's closest to what was heard"*.
//
// A ranking, then. The trap is that "how strong" has no single answer across the
// six methods, and the number that looks most like one is the one that put a
// wrong verse on a wall: `uncertain_book` carries a REAL parse confidence about a
// word nobody said, and "hymn number three sixteen" scored 0.840 (rule 10). A
// single strength scale sorts that to the top.
//
// So the measure is a RUN LENGTH, in words, and it exists for exactly the two
// methods whose evidence is a contiguous span of one verse the speaker actually
// said — `quoted` and `reading` (`detection.rs::PhraseHit`, whose own doc calls
// `run` "the operator-facing measure of how much of the verse was actually
// said"). A count of words a person said is not a probability: it has a unit, it
// cannot be read as a likelihood, and two of them are comparable to each other.
//
// Every other method gets NOTHING, deliberately:
//   * `semantic` — a TF-IDF cosine, and its `matched_text` is not even a span but
//     a weight-ordered term list (`detect.js::evidenceIsASpan`). Counting it
//     would count terms the index liked, not words anybody said.
//   * `ambiguous` — its confidence is "a hardcoded placeholder, not a
//     measurement".
//   * `uncertain_book` — rule 10, above.
//   * `direct` — its evidence is the reference, not the verse. There is nothing
//     to count, and the reference is already printed in full.
//
// The absence IS part of the ranking: a claim with no honest measure shows no
// measure, and sorts below one that has it.
import { runWords, readAloud } from './transcriptmark.js';

describe('runWords — the only honest measure of match strength, and who may have one', () => {
  it('counts the words of a run for the two methods whose evidence IS a run', () => {
    // `matched_text` for both is `PhraseHit.phrase`, which is
    // `words[i..i + n].join(" ")` — so the word count IS `run`, losslessly, and
    // nothing is being re-derived. `main.rs` passes `Some(h.phrase)`.
    expect(runWords({ method: 'quoted', matched_text: 'the lord is my shepherd' })).toBe(5);
    expect(
      runWords({
        method: 'reading',
        matched_text: 'the lord is my shepherd i shall not want he maketh me',
      }),
    ).toBe(12);
  });

  it('reproduces PhraseHit.run exactly, because phrase_words and normalise agree', () => {
    // `phrase_words` splits on anything not ASCII-alphanumeric and joins with a
    // single space, so the phrase is `n` bare tokens; `normalise` leaves bare
    // tokens alone. A run of eight counts as eight.
    const eight = ['for', 'god', 'so', 'loved', 'the', 'world', 'that', 'he'];
    expect(runWords({ method: 'reading', matched_text: eight.join(' ') })).toBe(8);
  });

  it('is nothing for a paraphrase, because a cosine has no length', () => {
    // `semantic`'s evidence is `terms.join(" · ")` — the words that moved the
    // cosine, in weight order, from anywhere in the verse. Two of them is not "a
    // two-word match"; it is two terms out of an arbitrary vector space.
    expect(runWords({ method: 'semantic', matched_text: 'lord · shepherd' })).toBe(null);
  });

  it('is nothing for ambiguous, uncertain_book or a bare verse from memory', () => {
    // rule 10. `uncertain_book` is the claim with a real number attached to a word
    // nobody said, and a strength badge on it is how Numbers 3:16 gets promoted
    // under a scanning operator's eye.
    expect(runWords({ method: 'ambiguous', matched_text: 'revelation 22' })).toBe(null);
    expect(runWords({ method: 'uncertain_book', matched_text: 'hymn number three sixteen' })).toBe(null);
    expect(runWords({ method: 'uncertain_book', matched_text: 'verse 32' })).toBe(null);
  });

  it('is nothing for a heard reference, whose evidence is the reference itself', () => {
    // "psalm 23 verse 1" is four words and none of them is scripture. Counting it
    // would rank "first epistle of john chapter four verse eight" above "ps 23 1"
    // on how wordily the preacher spoke, which is not a fact about the match.
    expect(runWords({ method: 'direct', matched_text: 'psalm 23 verse 1' })).toBe(null);
  });

  it('is nothing when there is no evidence, and never throws', () => {
    expect(runWords({ method: 'quoted', matched_text: '' })).toBe(null);
    expect(runWords({ method: 'quoted', matched_text: '  ,. ' })).toBe(null);
    expect(runWords({ method: 'quoted' })).toBe(null);
    expect(runWords(null)).toBe(null);
    expect(runWords(undefined)).toBe(null);
  });
});

describe('readAloud — the strong tier is the backend’s decision, never a bar re-derived here', () => {
  it('is true for a reading and false for every other method', () => {
    // `DetectionMethod::for_quotation` promotes a run to `Reading` on TWO
    // conditions — eight words AND sole. `sole` is not on the wire, so the
    // frontend cannot reproduce that decision and must not pretend to: the
    // method name IS the decision. Comparing a count against a local copy of
    // `READING_RUN_WORDS` would be rule 35's third instance, a component keeping
    // its own copy of a gate that lives in Rust.
    expect(readAloud({ claim: { method: 'reading' } })).toBe(true);
    for (const method of ['direct', 'quoted', 'semantic', 'ambiguous', 'uncertain_book']) {
      expect(readAloud({ claim: { method } }), `${method} is not a reading`).toBe(false);
    }
    expect(readAloud(null)).toBe(false);
    expect(readAloud({})).toBe(false);
  });
});

describe('markFor — the strongest match on a line is the one the card shows', () => {
  const line = { t: 'the lord is my shepherd i shall not want he maketh me lie down', at: '0:07:00' };

  it('carries the run, so the card needs no second table', () => {
    const m = markFor(line, [claim({ method: 'quoted', matched_text: 'the lord is my shepherd' })]);
    expect(m.run).toBe(5);
  });

  it('carries a null run where no honest measure exists', () => {
    const m = markFor(line, [claim({ method: 'semantic', matched_text: 'the lord is my shepherd' })]);
    expect(m.run).toBe(null);
  });

  it('prefers the LONGER run when one line holds two quotations — the operator’s ask', () => {
    // *"colour coded with the highest match"*. A preacher reading a verse aloud
    // produces a short run in a neighbouring verse and a long one in the verse
    // being read (`PhraseHit::sole`, the John 3:15/3:16 case). Before this the
    // mark was whichever claim the store handed over first.
    const short = claim({ reference: 'Psalms 23:2', method: 'quoted', matched_text: 'the lord is my shepherd' });
    const long = claim({
      reference: 'Psalms 23:1',
      method: 'reading',
      matched_text: 'the lord is my shepherd i shall not want',
    });
    expect(markFor(line, [short, long]).reference).toBe('Psalms 23:1');
    expect(markFor(line, [long, short]).reference).toBe('Psalms 23:1');
    expect(markFor(line, [long, short]).run).toBe(9);
  });

  it('never lets a measured run outrank a heard reference', () => {
    // A 25-word reading is a strong claim about WHICH verse and still not a
    // reference anybody said. `heard` outranks every run, at every length —
    // otherwise the strength scale has quietly become the trust ranking, which
    // is the one thing it may not be.
    const heardClaim = claim({ reference: 'Psalms 23:1', matched_text: 'the lord is my shepherd' });
    const longRun = claim({
      reference: 'John 10:11',
      method: 'reading',
      matched_text: 'the lord is my shepherd i shall not want he maketh me lie down',
    });
    expect(markFor(line, [longRun, heardClaim]).kind).toBe('heard');
    expect(markFor(line, [heardClaim, longRun]).kind).toBe('heard');
  });

  it('a measured run outranks a claim with no measure at all', () => {
    // The absence is part of the ranking. A paraphrase and a five-word quotation
    // are both `guess`, and only one of them has a number a person can check.
    const cosine = claim({ reference: 'John 10:11', method: 'semantic', matched_text: 'the lord is my shepherd' });
    const quoted = claim({ reference: 'Psalms 23:1', method: 'quoted', matched_text: 'the lord is my shepherd' });
    expect(markFor(line, [cosine, quoted]).reference).toBe('Psalms 23:1');
    expect(markFor(line, [quoted, cosine]).reference).toBe('Psalms 23:1');
  });
});

describe('rememberMarks — a run that grows as the preacher keeps reading', () => {
  const line = [{ t: 'the lord is my shepherd i shall not want he maketh me lie down', at: '0:08:00' }];

  it('replaces a remembered run with a longer one on the same line', () => {
    // The window slides forward while the preacher is still reading, so the same
    // line is claimed twice with more of the verse each time. Keeping the first
    // would freeze the mark at the weakest reading of the strongest line.
    const memo = new Map();
    rememberMarks(line, [claim({ method: 'quoted', matched_text: 'the lord is my shepherd' })], memo);
    const after = rememberMarks(
      line,
      [claim({ method: 'reading', matched_text: 'the lord is my shepherd i shall not want' })],
      memo,
    );
    expect(after[0].run).toBe(9);
  });

  it('does not let a shorter run overwrite a longer remembered one', () => {
    const memo = new Map();
    rememberMarks(line, [claim({ method: 'reading', matched_text: 'the lord is my shepherd i shall not want' })], memo);
    const after = rememberMarks(line, [claim({ method: 'quoted', matched_text: 'the lord is my shepherd' })], memo);
    expect(after[0].run).toBe(9);
  });

  it('does not let a measured run overwrite a remembered heard reference', () => {
    const memo = new Map();
    rememberMarks(line, [claim({ matched_text: 'the lord is my shepherd' })], memo);
    const after = rememberMarks(
      line,
      [claim({ method: 'reading', matched_text: 'the lord is my shepherd i shall not want' })],
      memo,
    );
    expect(after[0].kind).toBe('heard');
    expect(after[0].run).toBe(null);
  });
});
