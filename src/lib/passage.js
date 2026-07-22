// PASSAGE PARSING — "Ps 23 1-5" → { book: 'Ps', chapter: 23, from: 1, to: 5 }.
//
// This is the SHAPE of a query only. It does NOT resolve the book: "Ps",
// "psalm", "Sáàmù" and "Zaburi" all have to become "Psalms", and that table
// lives in Rust (detection.rs, data/book_aliases.json) where live detection
// uses it. A second alias table in the frontend would drift from the first, and
// the operator would get one answer typing a reference and a different one
// saying it out loud.
//
// So the split is: this file decides "the operator typed a range, and it ends
// at verse 5"; the backend decides which book they meant.

const RANGE = /^(.*?)[\s.:]*(\d+)\s*[:.\s]\s*(\d+)\s*(?:[-–—]|\s+to\s+)\s*(\d+)\s*$/i;
const SINGLE = /^(.*?)[\s.:]*(\d+)\s*[:.\s]\s*(\d+)\s*$/i;
const CHAPTER = /^(.*?)[\s.:]*(\d+)\s*$/i;

/**
 * Parse a typed reference.
 *
 * Returns null when the query is not reference-shaped at all (a phrase like
 * "there is therefore no condemnation") — the caller falls back to search.
 *
 * @returns {null | { book: string, chapter: number, from: number|null, to: number|null }}
 */
export function parsePassage(query) {
  const q = (query ?? '').trim().replace(/\s+/g, ' ');
  if (!q) return null;

  let m = RANGE.exec(q);
  if (m) {
    const from = Number(m[3]);
    const to = Number(m[4]);
    // "Ps 23 5-1" is a typo, not a passage. Reading it backwards would show an
    // empty pane with no explanation; reading it forwards shows the passage.
    return book(m[1], m[2], Math.min(from, to), Math.max(from, to));
  }

  m = SINGLE.exec(q);
  if (m) return book(m[1], m[2], Number(m[3]), Number(m[3]));

  m = CHAPTER.exec(q);
  // A bare number ("23") names no book, so it is not a reference.
  if (m && m[1].trim()) return book(m[1], m[2], null, null);

  return null;
}

function book(name, chapter, from, to) {
  const b = name.trim().replace(/[.,]$/, '');
  if (!b) return null;
  // A book name is words and (for 1/2/3 John etc.) a leading digit. Anything
  // else — punctuation, a stray number in the middle — is a phrase.
  if (!/^[\p{L}\p{M}0-9][\p{L}\p{M}0-9'’\- ]*$/u.test(b)) return null;
  return { book: b, chapter: Number(chapter), from, to };
}

/** The reference to ask the backend to resolve — always a single verse. */
export function probeReference(p) {
  return `${p.book} ${p.chapter}:${p.from ?? 1}`;
}

/** Keep only the verses a parsed range asked for. No range = the whole chapter. */
export function inRange(verses, p) {
  if (!p || p.from == null) return verses;
  return verses.filter((v) => v.verse >= p.from && v.verse <= p.to);
}
