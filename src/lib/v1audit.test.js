// THE AUDIT'S OWN ARITHMETIC, CHECKED BY SOMETHING THAT CANNOT TALK ITSELF ROUND.
//
// `docs/RELAY_V1_AUDIT.md` makes three claims about itself that a reader has to do
// sums to verify, and that nobody will:
//
//   1. every phase of the PWA brief (01–42) is dispositioned,
//   2. every section of the Relay brief (00–105) is accounted for — and §17.2
//      groups them into ranges, so "accounted for" is a claim about arithmetic
//      rather than a claim anyone can see,
//   3. each scorecard's rows add up to the total printed beside them.
//
// **Claim 3 was wrong on the first draft.** §15.1 summed to 73 of 90 and said 79;
// §15.2 summed to 159 and said 157. Both were then normalised from the wrong
// numerator. An audit whose own sums are wrong has no standing to complain about
// a count in prose beside the table it counts — which is exactly what this
// repository has corrected five times and gets wrong again each time
// (`RELAY_GAP.md` §18).
//
// So the sums are checked here, next to the other tests that check documents
// (`crossrefs.test.js`, `relaygap.test.js`), and by the only kind of reader that
// does not skim.
//
// It deliberately does NOT check the VALUES — whether accessibility is really an 8
// is a judgement, and a test that asserted it would be a test asserting an opinion.
// It checks that the document is internally consistent and complete, which are the
// two things a number can be wrong about without anybody noticing.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const root = resolve(__dirname, '../..');
const DOC = readFileSync(resolve(root, 'docs/RELAY_V1_AUDIT.md'), 'utf8');

/** The text between two headings. */
function section(from, to) {
  const a = DOC.indexOf(from);
  expect(a, `the audit no longer contains "${from}"`).toBeGreaterThan(-1);
  const b = to ? DOC.indexOf(to, a) : DOC.length;
  expect(b, `the audit no longer contains "${to}"`).toBeGreaterThan(-1);
  return DOC.slice(a, b);
}

describe('the V1 audit is internally consistent', () => {
  it('is the document this test thinks it is (the guard on everything below)', () => {
    // Every assertion here would also pass over a file that had been emptied.
    expect(DOC.length).toBeGreaterThan(20_000);
    expect(DOC).toMatch(/^# Relay — V1 Production Audit/);
    expect(DOC).toContain('## 15. The scorecards');
    expect(DOC).toContain('## 17. Brief disposition');
  });

  it('dispositions every phase of the PWA brief, 01 to 42, exactly once', () => {
    const table = section('### 17.1 The PWA master audit', '### 17.2 The Relay');
    const seen = [...table.matchAll(/^\| (\d{2}) /gm)].map((m) => Number(m[1]));
    const want = Array.from({ length: 42 }, (_, i) => i + 1);
    expect(seen, 'a phase is listed twice, or out of order').toEqual(want);
  });

  it('every phase carries a disposition, and it is one of the five words', () => {
    // A row that says nothing is a row that was skipped with a straight face.
    const table = section('### 17.1 The PWA master audit', '### 17.2 The Relay');
    const rows = [...table.matchAll(/^\| (\d{2}) ([^|]+)\|([^|]+)\|/gm)];
    expect(rows.length).toBe(42);
    const undecided = rows
      .filter(([, n, , d]) => !/\*\*(DONE|PARTIAL|N\/A|APPLIED|MISSING)/.test(d))
      .map(([, n]) => n);
    expect(undecided, `phases with no verdict: ${undecided.join(', ')}`).toEqual([]);
  });

  it('accounts for every section of the Relay brief, 00 to 105, with no gap and no overlap', () => {
    // §17.2 groups the 105 sections into ranges — which is the right call, because
    // 105 rows of "EXISTS" is not a report — but it turns "every section is
    // accounted for" into a claim about arithmetic that no reader will check.
    const table = section('### 17.2 The Relay live-service audit', '## 18. Recommended');
    const covered = new Set();
    const overlaps = [];
    for (const m of table.matchAll(/^\| \*\*(\d{2,3})(?:–(\d{2,3}))?\*\*/gm)) {
      const from = Number(m[1]);
      const to = m[2] === undefined ? from : Number(m[2]);
      expect(to, `a range runs backwards: ${m[0]}`).toBeGreaterThanOrEqual(from);
      for (let n = from; n <= to; n += 1) {
        if (covered.has(n)) overlaps.push(n);
        covered.add(n);
      }
    }
    expect(overlaps, `sections claimed by two rows: ${overlaps.join(', ')}`).toEqual([]);

    const missing = [];
    for (let n = 0; n <= 105; n += 1) if (!covered.has(n)) missing.push(n);
    expect(missing, `sections never accounted for: ${missing.join(', ')}`).toEqual([]);
    expect([...covered].filter((n) => n > 105), 'a row claims a section past 105').toEqual([]);
  });

  // A scorecard's rows, as numbers.
  //
  // Two shapes are allowed and both are deliberate: a bolded score over ten for a
  // scored axis, and "N/A → 10" for one where the ABSENCE is the correct answer
  // and scoring it zero would report a failure where there is no requirement.
  //
  // (A line comment, not a JSDoc block, on purpose: writing the first shape out
  // literally puts `**` immediately before a `/`, which ends a block comment two
  // lines early and turns the rest of the file into a syntax error. Which is a
  // small joke at this file's expense, and is left here as the reason.)
  function rows(block) {
    const scored = [...block.matchAll(/\*\*(\d+)\*\*\/10/g)].map((m) => Number(m[1]));
    const naFull = [...block.matchAll(/\*\*N\/A → (\d+)\*\*/g)].map((m) => Number(m[1]));
    return [...scored, ...naFull];
  }

  it('the PWA scorecard adds up to the total printed beside it', () => {
    const block = section('### 15.1 PWA master audit', '### 15.2 Relay production score');
    const got = rows(block);
    const sum = got.reduce((a, b) => a + b, 0);

    const claim = block.match(/\*\*(\d+) of a possible (\d+) → (\d+)\/100 normalised\.\*\*/);
    expect(claim, 'the 15.1 summary sentence changed shape — update this test with it').toBeTruthy();
    const [, cSum, cMax, cOut] = claim.map(Number);

    expect(cSum, `rows sum to ${sum}, sentence says ${cSum}`).toBe(sum);
    expect(cMax, 'the denominator is not ten per scored axis').toBe(got.length * 10);
    expect(cOut, 'the normalised score is not the sum over the denominator').toBe(
      Math.round((sum / cMax) * 100),
    );
    // And the heading must carry the same number as the sentence under it.
    expect(block).toContain(`### 15.1 PWA master audit — ${cOut}/100`);
  });

  it('the Relay scorecard adds up to the total printed beside it', () => {
    const block = section('### 15.2 Relay production score', '### 15.3 Live-service reliability');
    const got = rows(block);
    const sum = got.reduce((a, b) => a + b, 0);

    const claim = block.match(/\*\*Total: (\d+)\/(\d+) → (\d+)\/100\.\*\*/);
    expect(claim, 'the 15.2 summary sentence changed shape — update this test with it').toBeTruthy();
    const [, cSum, cMax, cOut] = claim.map(Number);

    expect(cSum, `rows sum to ${sum}, sentence says ${cSum}`).toBe(sum);
    expect(cMax, 'the denominator is not ten per row').toBe(got.length * 10);
    expect(cOut).toBe(Math.round((sum / cMax) * 100));
    expect(block).toContain(`### 15.2 Relay production score — ${cOut}/100`);
  });

  it('the live-service reliability score adds up, and is not hidden by the other two', () => {
    const block = section('### 15.3 Live-service reliability', '## 16. Remaining risks');
    const got = rows(block);
    const sum = got.reduce((a, b) => a + b, 0);

    const claim = block.match(/\*\*LIVE-SERVICE RELIABILITY: (\d+)\/(\d+)\.\*\*/);
    expect(claim, 'the 15.3 summary sentence changed shape').toBeTruthy();
    const [, cSum, cMax] = claim.map(Number);
    expect(cSum, `rows sum to ${sum}, sentence says ${cSum}`).toBe(sum);
    expect(cMax).toBe(got.length * 10);
    expect(block).toContain(`### 15.3 Live-service reliability — ${cSum}/${cMax}`);

    // The brief's own instruction: this score must not be averaged away against
    // the cosmetic ones. It is reported out of 80 and never folded into a /100.
    expect(DOC).not.toMatch(/LIVE-SERVICE RELIABILITY[^\n]*\/100/);
  });

  it('every fix in §6 says what it changed and how it was proved', () => {
    // The brief asks for problem · root cause · solution · files · test · result.
    // A fix write-up missing the last two is a claim, not a report — and this
    // document's whole argument is that it acted rather than looked.
    const block = section('## 6. The fix process', '## 7. Regression results');
    const fixes = [...block.matchAll(/^### (F-\d+) · ([^\n]+)$/gm)];
    expect(fixes.length, 'the fix list shrank').toBeGreaterThanOrEqual(10);

    const thin = [];
    for (let i = 0; i < fixes.length; i += 1) {
      const start = fixes[i].index;
      const end = i + 1 < fixes.length ? fixes[i + 1].index : block.length;
      const body = block.slice(start, end);
      const missing = ['**Problem.**', '**Root cause.**', '**Change.**', '**Files.**']
        .filter((h) => !body.includes(h));
      // Every fix must also say how it was TESTED — either a named test or an
      // explicit statement of what was not tested, which F-05 deliberately gives.
      if (!/\*\*Tests?\.\*\*/.test(body)) missing.push('**Test.**');
      if (missing.length) thin.push(`${fixes[i][1]}: missing ${missing.join(', ')}`);
    }
    expect(thin, thin.join(' · ')).toEqual([]);

    // Each fix names the register row it closes, so the two documents cannot
    // drift into describing different sets of work.
    const unfiled = fixes.filter(([, , title]) => !/RG-\d+/.test(title)).map((m) => m[1]);
    expect(unfiled, `fixes citing no RG- row: ${unfiled.join(', ')}`).toEqual([]);
  });

  it('quotes the same counts the register of counts holds', () => {
    // `qa/QA_HARNESS.md` §0 is the register for the whole repository, and this
    // document cites it rather than owning it. Two copies of a number is how
    // four documents came to disagree, which is the condition the sweep before
    // this one was called to end.
    const harness = readFileSync(resolve(root, 'docs/qa/QA_HARNESS.md'), 'utf8');
    const of = (re, where) => {
      const m = where.match(re);
      expect(m, `could not find ${re} in the register`).toBeTruthy();
      return m[1];
    };
    const rust = of(/\| Rust tests \| \*\*(\d+) passing\*\*/, harness);
    const front = of(/\| Frontend tests \| \*\*(\d+) passing\*\*/, harness);
    const cmds = of(/\| Registered `#\[tauri::command\]` \| \*\*(\d+)\*\*/, harness);

    expect(DOC, `the audit does not quote the register's Rust count (${rust})`).toContain(
      `**${rust} passed**`,
    );
    expect(DOC, `the audit does not quote the register's frontend count (${front})`).toContain(
      `**${front} passed**`,
    );
    expect(DOC, `the audit does not quote the register's command count (${cmds})`).toContain(
      `registers **${cmds}** commands`,
    );
  });
});
