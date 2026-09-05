// RG-52 — the gap register has to be readable, and I kept breaking it.
//
// `docs/qa/RELAY_GAP.md` §23 is the register: one row per gap, forty-plus of them,
// and it is the document a person opens to ask "what is left?".
//
// ── Why this is a test and not a promise ──────────────────────────────────────
//
// Over one session I corrupted this table four separate times, always the same
// way and never on purpose. Each new entry was appended by splicing a string in
// front of an anchor; when a merge conflict was resolved by hand, rows lost the
// newline between them and **a row was concatenated onto the end of its
// neighbour's last cell.** Markdown then renders it *inside* that cell, so the
// entry is invisible — RG-47 was unreadable for two merges, and I only found it
// because a rebase happened to conflict on that line.
//
// A blank line between two rows does the same damage more quietly: the table
// silently becomes two tables, and the second has no header, so every column
// after the split is unlabelled.
//
// I fixed it three times by reading. Reading is what let it happen three times.
//
// ── The trap in the obvious repair ───────────────────────────────────────────
//
// My first automated repair made it worse. The **"Depends on" column** contains
// cells that read exactly like the start of a row — `| RG-02 |` — so a splitter
// keyed on the id alone cuts rows in half at their own dependency cell. A row
// start is identified by its STATUS MARKER (✅ ⏳ ⚠️ ~~), never by the id.
//
// Lives in the frontend suite because that is where the repo already keeps its
// static file assertions; it touches no product code.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const DOC = readFileSync(resolve(process.cwd(), 'docs/qa/RELAY_GAP.md'), 'utf8');

/** A register row starts with a status marker. `| RG-02 |` is a dependency cell. */
const ROW_START = /\| (?:✅|⏳|⚠️|~~)\s?RG-\d+/;

/** §23's lines, and the contiguous block of rows under its header separator. */
function register() {
  const start = DOC.indexOf('## 23. Gap register');
  const end = DOC.indexOf('## 24. GO / NO-GO');
  expect(start, '§23 is gone').toBeGreaterThan(-1);
  expect(end, '§24 is gone — the section boundary this reads is missing').toBeGreaterThan(start);

  const lines = DOC.slice(start, end).split('\n');
  const header = lines.findIndex((l) => l.startsWith('| ID |'));
  const sep = lines.findIndex((l) => /^\|[-| ]+\|$/.test(l));
  expect(header, 'the register lost its header row').toBeGreaterThan(-1);
  expect(sep, 'the register lost its header separator').toBe(header + 1);

  const block = [];
  for (let i = sep + 1; i < lines.length && lines[i].startsWith('|'); i += 1) block.push(lines[i]);
  const all = lines.filter((l) => ROW_START.test(l));
  return { lines, header, block, all };
}

describe('RG-52 · the gap register is one readable table', () => {
  it('every row is inside the single table, with no blank line splitting it', () => {
    const { block, all } = register();
    // A blank line between two rows ends the table. Markdown then starts a NEW
    // one with no header, so every column after it is unlabelled.
    expect(
      block.length,
      `${all.length - block.length} row(s) sit outside the table — a blank line has ` +
        'split it, and everything after that point renders without a header',
    ).toBe(all.length);
  });

  it('no row is concatenated onto the end of another', () => {
    const { all } = register();
    const glued = all
      .filter((l) => (l.match(new RegExp(ROW_START.source, 'g')) ?? []).length > 1)
      .map((l) => l.slice(0, 60));
    // This is the failure that hid RG-47 for two merges: the second row lands
    // inside the first row's last cell and is not rendered as a row at all.
    expect(glued, `${glued.length} row(s) have another row glued onto them`).toEqual([]);
  });

  it('ids run from 1 with no gaps and no repeats', () => {
    const { all } = register();
    const ids = all.map((l) => Number(l.match(/RG-(\d+)/)[1]));
    const dupes = ids.filter((n, i) => ids.indexOf(n) !== i);
    expect(dupes, `duplicated: ${dupes.join(', ')}`).toEqual([]);

    const sorted = [...ids].sort((a, b) => a - b);
    const missing = [];
    for (let n = 1; n <= sorted[sorted.length - 1]; n += 1) if (!ids.includes(n)) missing.push(n);
    // A missing id is almost always a row that was glued or dropped, not a
    // deliberate gap — and if it ever IS deliberate, say so here.
    expect(missing, `missing: ${missing.map((n) => `RG-${n}`).join(', ')}`).toEqual([]);

    // Rows are in order, so a reader can find one.
    expect(ids, 'the rows are out of order').toEqual(sorted);
  });

  it('every row has the full set of columns', () => {
    const { lines, header, all } = register();
    // Count UNESCAPED pipes only. Several rows quote a shell command containing
    // `\|`, which is an escaped pipe and does not open a column — counting those
    // reported three perfectly good rows as malformed the first time this ran.
    const cells = (l) => (l.match(/(?<!\\)\|/g) ?? []).length;
    const want = cells(lines[header]);
    const wrong = all
      .filter((l) => cells(l) !== want)
      .map((l) => `${l.match(/RG-\d+/)[0]} (${cells(l)} of ${want})`);
    // A short row is a row that lost a cell in an edit; a long one has swallowed
    // something. Either way the columns after it are reading the wrong field.
    expect(wrong, `rows with the wrong cell count: ${wrong.join(', ')}`).toEqual([]);
  });

  it('the summary at the top counts the same table it summarises', () => {
    // This block said "54 closed" and "51 closed" in consecutive sentences for two
    // merges. A count in prose sitting directly above the table it counts is the
    // cheapest thing in this repository to check, and the most reliably wrong when
    // it is not checked: every row added has to be remembered in four places.
    const { all } = register();
    const total = all.length;
    const withdrawn = all.filter((l) => /\| ~~/.test(l)).length;
    const open = all.filter((l) => /\| ⏳/.test(l)).length;
    const flagged = all.filter((l) => /\| ⚠️/.test(l)).length;
    const closed = all.filter((l) => /\| ✅/.test(l)).length;
    expect(closed + withdrawn + open + flagged, 'a row carries no status marker').toBe(total);

    const head = DOC.slice(
      DOC.indexOf('## WHERE THIS IS UP TO'),
      DOC.indexOf('## 0. Method'),
    );
    const claim = head.match(/\*\*(\d+) entries\. (\d+) closed, (\d+) withdrawn as wrong, (\d+) not closed/);
    expect(claim, 'the summary sentence has changed shape — update this test with it').toBeTruthy();
    const [, cTotal, cClosed, cWithdrawn, cOpen] = claim.map(Number);
    expect(cTotal, 'entry count').toBe(total);
    expect(cClosed, 'closed count').toBe(closed);
    expect(cWithdrawn, 'withdrawn count').toBe(withdrawn);
    // "not closed" is the open one plus the flagged ones — the two kinds are
    // separated in the table below the sentence, never in the sentence itself.
    expect(cOpen, 'not-closed count').toBe(open + flagged);

    // And the roll-up row under it, which is where the two numbers disagreed.
    expect(
      head.includes(`✅ **${closed} closed**`),
      `the roll-up row does not say ${closed}`,
    ).toBe(true);
  });

  it('every row carries a status, and only the recorded ones are unresolved', () => {
    const { all } = register();
    const open = all.filter((l) => /\| ⏳/.test(l)).map((l) => l.match(/RG-\d+/)[0]);
    const flagged = all.filter((l) => /\| ⚠️/.test(l)).map((l) => l.match(/RG-\d+/)[0]);

    // Not a cap on how many gaps may be open — it is a nudge to WRITE DOWN why.
    // An entry left ⏳ with no reason in the row is how a register becomes a
    // to-do list nobody reads.
    for (const id of [...open, ...flagged]) {
      const row = all.find((l) => l.includes(`${id} |`));
      expect(
        row.length,
        `${id} is unresolved but says almost nothing — an open entry has to carry its reason`,
      ).toBeGreaterThan(200);
    }
  });
});
