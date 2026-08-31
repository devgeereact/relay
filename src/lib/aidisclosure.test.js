// RG-59 — the AI disclosure has to reach the operator, not just the repository.
//
// `docs/AI_DISCLOSURE.md` is the honest account of what the AI does, what it
// refuses to do, and where it is weak. It is one of the better documents in this
// project and it was, until now, entirely unreachable from the running app: a
// church that never opens GitHub never sees a word of it.
//
// The half that was already in the app was the reassuring half — Help's
// "What the AI will and will not do on its own" topic states the never-guess rule
// correctly. The half that was missing was the part a church actually needs before
// it trusts the thing: **African-language listening is the weakest part of the
// product, and word error rate has never been measured in any language.**
//
// Publishing only the reassuring half is worse than publishing neither. So both
// documents have to carry both facts, and this test is what stops one of them from
// quietly losing its copy — the drift is invisible otherwise, because each file
// still reads perfectly well on its own.
//
// It asserts SUBSTANCE, not wording: each claim is matched by a small set of
// alternatives, so the prose can be improved without breaking the test, and only
// deleting the claim breaks it.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

// Both files hard-wrap their prose, so a claim is routinely split across two
// lines ("Transcription is not\n  measured at all"). Matching the raw file misses
// it and reports a document that says the right thing as one that does not.
const read = (p) => readFileSync(resolve(process.cwd(), p), 'utf8').replace(/\s+/g, ' ');
const HELP = read('src/lib/views/Help.svelte');
const DISCLOSURE = read('docs/AI_DISCLOSURE.md');

/** Both files must make this claim, in whatever words they choose. */
const CLAIMS = [
  {
    what: 'Relay never generates verse text — it reads a bundled translation verbatim',
    help: /never writes scripture|read verbatim|cannot invent/i,
    doc: /never writes scripture|read verbatim/i,
  },
  {
    what: 'African-language recognition is the weakest part, and it is the headline claim',
    help: /weakest part/i,
    doc: /weakest part/i,
  },
  {
    what: 'transcription accuracy has never been measured',
    help: /never been measured|not measured/i,
    doc: /not measured at all|never been measured/i,
  },
  {
    what: 'a paraphrase is offered and never auto-fired',
    help: /NEVER put a guess|offers it to you/i,
    doc: /never reaches a congregation without a human|\*\*Never\.\*\*/i,
  },
];

describe('RG-59 · the AI disclosure and the Help tab say the same things', () => {
  for (const c of CLAIMS) {
    it(`Help states: ${c.what}`, () => {
      expect(c.help.test(HELP), `Help.svelte no longer says: ${c.what}`).toBe(true);
    });
    it(`AI_DISCLOSURE states: ${c.what}`, () => {
      expect(c.doc.test(DISCLOSURE), `AI_DISCLOSURE.md no longer says: ${c.what}`).toBe(true);
    });
  }

  it('the honest half is a topic an operator can find, not a footnote', () => {
    // Searchable: Help filters TOPICS by title + body, so the claim has to live
    // inside a topic to be reachable from the search box at all.
    expect(/id: 'weakness'/.test(HELP)).toBe(true);
    expect(/title: 'What the AI is bad at'/.test(HELP)).toBe(true);
  });
});
