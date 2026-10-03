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
    // A REGEX LITERAL IS SKIPPED, exactly like a quoted run above and for the
    // same reason: `/<!--[\s\S]*?-->/` is a PATTERN, not a comment. Without
    // this, the scan walked into the literal, found the `<!--` inside it, and
    // blanked to the `-->` — so `.replace(/<!--…-->/g, '')` came out as
    // `.replace(/               /g, '')` and THE RG-169 SWEEP COULD NOT SEE THE
    // ONE THING IT FORBIDS. `viewrouters.test.js` hand-rolled that chain and the
    // sweep reported clean; CodeQL was the only instrument that could see it.
    // That is this repository's most-repeated fault inside the instrument built
    // to stop it, so the fix is here and not in the sweep.
    //
    // Conservative on purpose: only a `/` whose previous non-space character
    // makes a literal unambiguous. Anything else is left to be division, so the
    // cost of a miss is a comment kept — a false alarm, never a blind spot.
    // `//` and `/*` are comment markers and are handled below; a regex literal
    // can begin with neither (an empty `//` is a comment, and a literal `*` must
    // be escaped). Without this, `foo(  // note` read its comment as a pattern
    // and the comment survived the strip.
    if (c === '/' && text[i + 1] !== '/' && text[i + 1] !== '*') {
      // Newlines are skipped too, because the sweeps in `codeonly.test.js` are
      // written `.filter((f) =>\n  /replace\\(\\/<!--/.test(…))` — the literal
      // begins a line and its `=>` is on the one before. Missing that case let
      // the scan walk into THAT pattern, find the `<!--` in it and blank to the
      // next `-->`: the 7 KB bug, in the file that exists to prevent it.
      //
      // The set is every character after which division is IMPOSSIBLE. An
      // identifier, a `)`, a `]` or a digit is deliberately absent, so `a / b`
      // and `foo()\n/ 2` stay division and the ASI ambiguity is never guessed at.
      let k = i - 1;
      while (k >= 0 && (text[k] === ' ' || text[k] === '\t' || text[k] === '\r' || text[k] === '\n'))
        k -= 1;
      if (k >= 0 && '(,=:[!&|>;{'.includes(text[k])) {
        let j = i + 1;
        let inClass = false;
        while (j < text.length) {
          const d = text[j];
          if (d === '\\') {
            j += 2;
            continue;
          }
          if (d === '\n') break; // an unterminated literal ends at the line
          if (inClass) {
            if (d === ']') inClass = false;
          } else if (d === '[') {
            inClass = true;
          } else if (d === '/') {
            break;
          }
          j += 1;
        }
        i = j + 1;
        continue;
      }
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

/**
 * The same text with every `<tag …> … </tag>` block blanked, its own tags
 * included — so a scanner asking "what MARKUP does this file paint" does not
 * read a `<button>` named in a `<script>` JSDoc or a `.lmsg` in a stylesheet.
 *
 * ── WHY THIS IS NOT `.replace(/<style[\s\S]*?<\/style>/g, '')` ───────────────
 *
 * Four scanners hand-rolled that regex and CodeQL raised `js/bad-tag-filter` on
 * three of them. The complaint is narrow and correct: `<\/style>` does not match
 * `</style >` or `</style\n>`, both of which a browser closes, so one stray
 * space leaves the whole block in the text and every heading, button and class
 * inside it is read as markup. That direction cries wolf rather than going
 * blind, which is the cheap one — and `codeonly.js` already says at the top that
 * cheap is still wrong.
 *
 * It also blanks rather than deletes, like `codeOnly`, so offsets and line
 * numbers in the result still match the real file.
 */
export function withoutBlock(text, tag) {
  const out = text.split('');
  const lower = text.toLowerCase();
  const open = `<${tag}`;
  const close = `</${tag}`;
  let i = 0;
  while (i < text.length) {
    const a = lower.indexOf(open, i);
    if (a === -1) break;
    // The whole tag NAME has to match: `<styles>` is a different element, and a
    // prefix match would blank from it to the next `</style>` — the 7 KB bug's
    // shape in another costume.
    const after = lower[a + open.length];
    if (after !== undefined && after !== '>' && after !== '/' && !/\s/.test(after)) {
      i = a + open.length;
      continue;
    }
    const c = lower.indexOf(close, a + open.length);
    // An unterminated block runs to the end of the text, which is what a browser
    // does with one as well.
    let stop = text.length;
    if (c !== -1) {
      const gt = lower.indexOf('>', c + close.length);
      stop = gt === -1 ? text.length : gt + 1;
    }
    for (let j = a; j < stop; j += 1) if (out[j] !== '\n') out[j] = ' ';
    i = stop;
  }
  return out.join('');
}
