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
    for (const method of ['semantic', 'quoted', 'ambiguous', 'uncertain_book']) {
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
    expect(Object.keys(fired).sort()).toEqual(['claim', 'kind', 'reference']);
    expect(fired.kind).toBe(pending.kind);
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
