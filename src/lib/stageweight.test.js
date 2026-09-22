// THE PREACHER'S SCREEN IS READ FROM A PLATFORM, SO IT IS SET BOLD (RG-251).
//
// The operator, holding the built page: *"the alert on the mobile should be bold
// as in the artifacts... all text on the mobile should be bold and clean and
// readable"*.
//
// This page is a phone on a lectern and a monitor across a platform, read in one
// glance by somebody mid-sentence. Every FIGURE on it was already 700 — the
// clocks, the timers, the labels — and every piece of prose was not: the
// reference, the verse, the Up Next line and both halves of a search result all
// inherited a regular weight from the body. So the numbers were built for the
// room and the words were built for a desk.
//
// The alert is the same fault one step further on: 36 characters rendered at
// 6cqw, which is 23px on a 390px phone, on a panel whose entire job is to stop a
// service.
//
// ASSERTED ON THE STYLESHEET, deliberately. jsdom computes no layout and
// resolves no container unit, so a rendered assertion here could only repeat the
// declaration back. What the numbers MEAN was measured in a real browser at
// 390x844 and is recorded in RG-251.
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const FILE = readFileSync(resolve('src/Stage.svelte'), 'utf8');
// The stylesheet alone: this page's script quotes its own rules in comments, and
// a scanner reading the whole file finds the prose before the rule.
const SRC = FILE.slice(FILE.lastIndexOf('<style>'));
const decls = (sel) => {
  const from = SRC.indexOf(sel + ' {');
  expect(from, `no rule for ${sel}`).toBeGreaterThan(-1);
  return SRC.slice(from, SRC.indexOf('}', from)).replace(/\/\*[\s\S]*?\*\//g, '');
};
const weight = (sel) => {
  const m = decls(sel).match(/font-weight:\s*(\d+)/);
  return m ? Number(m[1]) : null;
};

// Every rank of PROSE on the page. The figures are not listed: they were already
// 700 and are pinned by the rules that own them.
const PROSE = ['.ref', '.verse', '.next-text', '.r-ref', '.r-text', '.bigmsg-v'];

describe('the weight the preacher’s screen is set in', () => {
  for (const sel of PROSE) {
    it(`${sel} states a weight, and it is bold`, () => {
      const w = weight(sel);
      expect(w, `${sel} inherits its weight from the body`).not.toBeNull();
      expect(w, `${sel} is set lighter than the room needs`).toBeGreaterThanOrEqual(600);
    });
  }

  it('the alert is the boldest thing on the page', () => {
    expect(weight('.alert')).toBeGreaterThanOrEqual(800);
  });

  it('every alert step is big enough to be read from a platform', () => {
    // Measured at 390x844 against the real face: the longest message the
    // backend will deliver (`ALERT_MAX` = 140) wraps to seven lines at 9cqw and
    // uses 282px of an 813px box, so the ceiling is nowhere near. The figures
    // below are what the drawing asks for, and the headroom is why they are
    // safe.
    const size = (cls) => Number(decls('.alert.' + cls).match(/font-size:\s*([\d.]+)cqw/)[1]);
    expect(size('xl'), 'a phrase').toBeGreaterThanOrEqual(13);
    expect(size('lg'), 'a sentence').toBeGreaterThanOrEqual(9);
    expect(size('md'), 'the longest a desk can send').toBeGreaterThanOrEqual(6);
    // AND THEY STILL DESCEND. A step table whose steps are not ordered is not a
    // step table, and nothing else in this file would notice.
    expect(size('xl')).toBeGreaterThan(size('lg'));
    expect(size('lg')).toBeGreaterThan(size('md'));
  });
});
