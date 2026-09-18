// THE ONE COMMENT STRIPPER, because four of them is how a scanner goes blind.
//
// Every static test in this repository that asks "does the CODE say this" has to
// take the comments out first, and each one grew its own regex to do it. That is
// the shape of a defect this project has now paid for twice.
//
// `names.test.js` stripped comments with three regexes, and a comment in
// `Stage.svelte` containing `:8032/api/*` opened a block comment that ran to the
// next real `*/` SEVEN THOUSAND characters later, taking the whole `ZONES` table
// with it. The register read the preacher's own screen with its labels missing
// and reported clean.
//
// CodeQL then flagged three more scanners for the same class from the other
// direction: `.replace(/<!--[\s\S]*?-->/g, '')` leaves a dangling `<!--` behind a
// nested comment, so text that IS a comment survives and is read as code. That
// direction produces a false alarm rather than a blind spot, which is the less
// expensive of the two and still wrong.
//
// So: one stripper, scanning left to right, aware that a quoted run cannot open a
// comment. Blanking rather than deleting, so every offset and line number in the
// result still matches the original file.

/**
 * The same text with its comment CONTENT removed — `//` to end of line, `/* … *\/`
 * and `<!-- … -->` — so the casing and register tests read what an operator can
 * see rather than what a maintainer wrote beside it.
 *
 * ── IT WAS THREE REGEXES, AND IT QUIETLY ATE 7 KB OF A REAL PAGE ─────────────
 *
 * The block-comment pass ran FIRST and over the whole file, so it could not know
 * that a `/*` it had found was inside a `//` line. `Stage.svelte` carries the
 * comment *"Hits the LAN HTTP API on :8031's sibling port (:8032/api/\*)"*; that
 * `/\*` opened a block comment for the scanner, which then ran to the next real
 * `*\/` seven thousand characters later and blanked every line between —
 * including the whole `ZONES` table, which is where the labels the preacher's own
 * screen renders are declared. The casing and register tests read a version of
 * that page with its zone labels missing, reported nothing, and were believed.
 * **A scanner that quietly narrows passes everything** — this file says so at the
 * top about `ipc.test.js`, twice, and was doing it itself.
 *
 * So it is one left-to-right pass with the states a reader has: a quoted run is
 * skipped (nothing inside it opens a comment), and `//`, `/* *\/` and `<!-- -->`
 * are each recognised where they actually begin. Offsets and newlines are
 * preserved, so a reported line number is still the real one.
 *
 * `//` is still only treated as a comment when it does not follow a `:`, which
 * keeps a bare `http://host` outside a string readable. The retired-label test
 * does NOT use this function at all: a stale comment reviving an old name is
 * exactly what it is for.
 */
export function codeOnly(text) {
  const out = text.split('');
  const blank = (from, to) => {
    for (let j = from; j < to; j += 1) if (out[j] !== '\n') out[j] = ' ';
  };
  let i = 0;
  while (i < text.length) {
    const c = text[i];
    // A QUOTED RUN IS SKIPPED, NOT BLANKED. Nothing inside it may open a comment
    // — `':8032/api/*'` in a string is a path, not a block comment — and the text
    // itself is code, so it stays. An unterminated quote (an apostrophe in
    // markup: `don't`) ends at the newline rather than eating the rest of the
    // file; the worst that costs is that a comment opened later on that one line
    // is kept, which errs toward reporting a label rather than missing one.
    if (c === "'" || c === '"' || c === '`') {
      let j = i + 1;
      while (j < text.length) {
        if (text[j] === '\\') {
          j += 2;
          continue;
        }
        if (text[j] === c) break;
        if (c !== '`' && text[j] === '\n') break;
        j += 1;
      }
      i = j + 1;
      continue;
    }
    if (text.startsWith('<!--', i)) {
      const end = text.indexOf('-->', i + 4);
      const stop = end === -1 ? text.length : end + 3;
      blank(i, stop);
      i = stop;
      continue;
    }
    if (text.startsWith('/*', i)) {
      const end = text.indexOf('*/', i + 2);
      const stop = end === -1 ? text.length : end + 2;
      blank(i, stop);
      i = stop;
      continue;
    }
    // `//` is only a comment when it does not follow a `:`, so a bare
    // `http://host` outside a string keeps the rest of its line.
    if (text.startsWith('//', i) && text[i - 1] !== ':') {
      const end = text.indexOf('\n', i);
      blank(i, end === -1 ? text.length : end);
      i = end === -1 ? text.length : end;
      continue;
    }
    i += 1;
  }
  return out.join('');
}
