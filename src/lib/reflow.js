// LYRIC REFLOW — text in, slides out.
//
// ── Why this is a pure module ─────────────────────────────────────────────
//
// This is the rule that decides what a congregation reads. It has to be
// testable without a window, a database or a projector, exactly like
// detection.rs — so it takes strings and returns objects, and knows nothing
// about Svelte, Tauri or the DB.
//
// ── The model ─────────────────────────────────────────────────────────────
//
//   TEXT      what the operator types. One blank line ends a section.
//   SECTION   a named block: Verse 1, Chorus, Bridge. What the DB stores.
//   SLIDE     what actually goes on the wall. A section too long to read at
//             once is broken across several, and THAT is the reflow.
//
// ProPresenter, EasyWorship and OpenLP all separate these three. Relay stored
// sections and projected them verbatim, which meant a nine-line verse went up
// as nine lines of six-point text, or was hand-split into fake "sections" that
// then lied about the song's structure in the plan and in the arrangement.

/** Section headers an operator actually types. `[Chorus]`, `Chorus:`, `V1`. */
const BRACKET = /^\[([^\]]{1,40})\]\s*$/;
const NAMED =
  /^((?:pre-?)?chorus|verse|bridge|tag|intro|outro|refrain|ending|interlude|vamp|instrumental|coda)\s*(\d+)?\s*:?\s*$/i;
const SHORT = /^([vcbpt])\s*(\d{1,2})\s*:?\s*$/i;
const SHORT_TAGS = { v: 'Verse', c: 'Chorus', b: 'Bridge', p: 'Pre-Chorus', t: 'Tag' };

/** A short tag for the slide corner: "Verse 1" → V1, "Chorus" → C. */
export function tagFor(label) {
  const m = /^([a-z])[a-z-]*\s*(\d+)?/i.exec((label ?? '').trim());
  if (!m) return '';
  return (m[1].toUpperCase() + (m[2] ?? '')).slice(0, 3);
}

/**
 * Parse typed lyrics into sections.
 *
 * A blank line ends a section. A leading header names it; without one the
 * section is numbered as a verse, because that is what an unnamed block of a
 * song is nearly always is — and a wrong-but-editable name beats "Section 3".
 *
 * @returns {{tag: string, label: string, lyrics: string}[]}
 */
export function parseLyrics(text) {
  const src = String(text ?? '').replace(/\r\n?/g, '\n');
  const blocks = src
    .split(/\n\s*\n+/)
    .map((b) => b.replace(/\s+$/, ''))
    .filter((b) => b.trim());

  let verseNo = 0;
  return blocks.map((block) => {
    const lines = block.split('\n');
    const head = lines[0].trim();
    let label = '';

    const bracket = BRACKET.exec(head);
    const named = NAMED.exec(head);
    const short = SHORT.exec(head);
    if (bracket) label = bracket[1].trim();
    else if (named) label = title(named[1]) + (named[2] ? ` ${named[2]}` : '');
    else if (short) label = `${SHORT_TAGS[short[1].toLowerCase()]} ${short[2]}`;

    const body = label ? lines.slice(1).join('\n').replace(/^\s+/, '') : block;
    if (!label) label = `Verse ${++verseNo}`;
    // A header with nothing under it is a label the operator has not filled in
    // yet, not an empty slide. Keep it — deleting what someone just typed is
    // the one thing an editor may never do.
    return { tag: tagFor(label), label, lyrics: body };
  });
}

/** Sections back to editable text. `parseLyrics(toText(s))` is stable. */
export function toText(sections) {
  return (sections ?? [])
    .map((s) => {
      const label = (s.label || s.tag || '').trim();
      const body = (s.lyrics ?? '').replace(/\s+$/, '');
      return label ? `[${label}]\n${body}` : body;
    })
    .join('\n\n')
    .trim();
}

/**
 * Break sections into the slides that actually go on the wall.
 *
 * `linesPerSlide` is the only rule that matters in a room: a slide nobody at
 * the back can read is not a slide. `maxChars` is the safety net for one very
 * long line — it splits at a line boundary, never mid-word, because a lyric cut
 * in half mid-word is worse than a slightly full slide.
 *
 * @returns {{key,tag,label,lyrics,section,part,parts}[]}
 */
export function reflow(sections, opts = {}) {
  const perSlide = Math.max(1, Number(opts.linesPerSlide) || 4);
  const maxChars = Math.max(0, Number(opts.maxChars) || 0);
  const out = [];

  (sections ?? []).forEach((sec, si) => {
    const lines = String(sec.lyrics ?? '')
      .split('\n')
      .map((l) => l.trimEnd())
      .filter((l) => l.trim());

    // A section with a name and no words is a placeholder the operator is still
    // filling in. It gets a slide so it stays visible in the deck.
    const chunks = lines.length ? chunk(lines, perSlide, maxChars) : [[]];

    chunks.forEach((body, pi) => {
      out.push({
        key: `${si}-${pi}`,
        section: si,
        part: pi + 1,
        parts: chunks.length,
        tag: sec.tag || tagFor(sec.label),
        label:
          chunks.length > 1
            ? `${sec.label || sec.tag || `Section ${si + 1}`} (${pi + 1}/${chunks.length})`
            : sec.label || sec.tag || `Section ${si + 1}`,
        lyrics: body.join('\n'),
      });
    });
  });

  return out;
}

function chunk(lines, perSlide, maxChars) {
  const out = [];
  let cur = [];
  let chars = 0;
  for (const line of lines) {
    const wouldOverflow = maxChars && cur.length && chars + line.length + 1 > maxChars;
    if (cur.length >= perSlide || wouldOverflow) {
      out.push(cur);
      cur = [];
      chars = 0;
    }
    cur.push(line);
    chars += line.length + 1;
  }
  if (cur.length) out.push(cur);
  return out;
}

const title = (w) =>
  w
    .toLowerCase()
    .replace(/^pre-?chorus$/, 'pre-chorus')
    .replace(/(^|-)([a-z])/g, (_, sep, c) => sep + c.toUpperCase());
