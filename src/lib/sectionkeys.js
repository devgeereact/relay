// SECTION KEYS — the letter that puts a section of a song on the screens.
//
// `docs/REBRAND.md` §10: "Section keys: `v c b t i o`, numbered only when a kind
// repeats. On Live the key sends that section to the programme. A half-typed key
// wins the next keystroke so `v2` can be typed; panic keys are never shadowed;
// typing in a field fires nothing."
//
// ── Why this is a pure module ─────────────────────────────────────────────
//
// It is the rule that decides which words a congregation reads when an operator
// hits one key in a dark booth. It has to be testable without a window, a
// database or a projector — exactly like `reflow.js` and `detection.rs` — so it
// takes sections and keystrokes and returns keys and decisions, and knows
// nothing about Svelte, Tauri or the DOM.
//
// ── WHERE THE SPEC AND `CLAUDE.md` DISAGREE, AND WHO WINS ─────────────────
//
// The spec's alphabet contains `b`, for Bridge. On this console `B` is
// BLACKOUT — a panic control, and the preamble says panic keys are never
// shadowed (CLAUDE.md rule 15, DECISIONS §20). `CLAUDE.md` wins. So `b` is
// RESERVED and a Bridge does not get it.
//
// The wrong fix would be to hand the Bridge `b` anyway and let `shortcuts.js`
// win the race: the operator then sees a key printed on a slide that blacks the
// wall out instead of firing it — a cheatsheet that lies, on the one surface
// where a lie costs a congregation. The other wrong fix is to give the Bridge no
// key at all, which is honest and useless.
//
// So a section whose natural letter is reserved (or already claimed) takes the
// next free letter OF ITS OWN NAME — `bridge` → `b` is reserved → `r`. It is
// derived, not invented, it is printed on the slide, and `why` says how it was
// reached so the surface can explain it.
//
// `RESERVED` is read OUT OF `shortcuts.js`'s own table rather than restated
// here. A future global key is therefore reserved the moment it is bound, with
// no edit to this file — the failure this guards against is exactly the one
// CLAUDE.md calls "a guarantee is only kept on the doors you checked".

import { SHORTCUTS } from './shortcuts.js';

/** Single-character keys the global handler already owns. Never assignable. */
export const RESERVED = new Set(
  SHORTCUTS.flatMap((s) => s.keys)
    .filter((k) => k.length === 1)
    .map((k) => k.toLowerCase()),
);

/**
 * The six kinds the spec names, and the word each letter is derived from. The
 * WORD matters: it is where a fallback letter comes from when the first is
 * taken, so the answer stays derived rather than invented.
 */
const KINDS = [
  { letter: 'v', word: 'verse', match: /^verse\b/i },
  { letter: 'c', word: 'chorus', match: /^chorus\b/i },
  { letter: 'b', word: 'bridge', match: /^bridge\b/i },
  { letter: 't', word: 'tag', match: /^tag\b/i },
  { letter: 'i', word: 'intro', match: /^intro\b/i },
  { letter: 'o', word: 'outro', match: /^outro\b/i },
];

/** An explicit key is ONE letter — `[Bridge:g]`. Numbering is always derived. */
const KEY_RE = /^[a-z]$/;

/** Which of the six kinds a label names, or `null` for anything else. */
export function kindOf(label) {
  const t = String(label ?? '').trim();
  return KINDS.find((k) => k.match.test(t)) ?? null;
}

/**
 * The key a section ASKED for, or `''`.
 *
 * It rides in `tag`, because that is the only per-section field that survives a
 * save (`song_sections` is id · position · tag · label · lyrics). A tag that is
 * simply what the label derives — `Chorus` → `C` — is not a request; a tag that
 * differs is, and `reflow.js` writes it back as `[Bridge:g]`.
 */
export function explicitKey(section) {
  const tag = String(section?.tag ?? '').trim().toLowerCase();
  if (!KEY_RE.test(tag)) return '';
  return tag === naturalTag(section?.label) ? '' : tag;
}

function naturalTag(label) {
  const m = /^([a-z])[a-z-]*\s*(\d+)?/i.exec(String(label ?? '').trim());
  return m ? (m[1] + (m[2] ?? '')).toLowerCase() : '';
}

/** The letters of a word, in order, deduplicated — the fallback ladder. */
function ladder(word) {
  const out = [];
  for (const ch of String(word ?? '').toLowerCase()) {
    if (/[a-z]/.test(ch) && !out.includes(ch)) out.push(ch);
  }
  return out;
}

/**
 * Assign a key to every section of ONE song.
 *
 * Returns one entry per section: `{ key, why, problem }`.
 *   key      the letter (or letter+number) to press, or `null`
 *   why      'explicit' · 'kind' · 'name' · 'fallback' · 'none'
 *   problem  an operator-facing sentence, or `null`
 *
 * Numbering is per LETTER and only when that letter is wanted more than once —
 * one verse is `v`, two verses are `v1` and `v2`. That is the spec's "numbered
 * only when a kind repeats", said in the one place it can be true.
 *
 * **Two sections cannot share a key.** Two sections that merely share a KIND are
 * numbered apart, which is not sharing. Two sections that both ASK for the same
 * letter are a conflict: the first keeps it, the rest get none and say why.
 * Guessing which one the operator meant is the rule-39 mistake in miniature.
 */
export function assignKeys(sections) {
  const list = Array.isArray(sections) ? sections : [];
  const out = list.map(() => ({ key: null, why: 'none', problem: null }));
  const taken = new Set();

  // ── Pass 1 · explicit requests, in order. First asker wins. ──────────────
  const wanted = list.map((s) => explicitKey(s));
  wanted.forEach((letter, i) => {
    if (!letter) return;
    if (RESERVED.has(letter)) {
      out[i].problem = `“${letter}” is a Relay key (${reservedFor(letter)}) — pick another letter.`;
      return;
    }
    if (taken.has(letter)) {
      out[i].problem = `Two sections both ask for “${letter}”. This one has no key until one of them changes.`;
      return;
    }
    taken.add(letter);
    out[i] = { key: letter, why: 'explicit', problem: null };
  });

  // ── Pass 2 · group the rest by WHAT KIND OF SECTION THEY ARE. ────────────
  //
  // The group is the kind, not the letter. Two verses are one group and number
  // apart — that is the spec's "numbered only when a kind repeats". An Intro and
  // an Interlude both start with `i` and are NOT the same thing, so they are two
  // groups, and the second takes a fallback letter rather than becoming `i2`.
  /** @type {Map<string, {first:string, source:string, members:number[]}>} */
  const groups = new Map();
  list.forEach((s, i) => {
    if (out[i].key || out[i].problem) return;
    const kind = kindOf(s?.label);
    const word = kind ? kind.word : String(s?.label ?? '').trim();
    const first = kind ? kind.letter : ladder(word)[0];
    if (!first) {
      out[i].problem = 'This section has no name, so there is no letter to press.';
      return;
    }
    const id = kind ? `kind:${kind.letter}` : `name:${word.toLowerCase()}`;
    const g = groups.get(id) ?? { first, source: word, members: [] };
    g.members.push(i);
    groups.set(id, g);
  });

  // First-appearance order, so the letters an operator expects are handed out
  // before a later section can take one of them by fallback.
  const ordered = [...groups.values()].sort((a, b) => a.members[0] - b.members[0]);
  for (const g of ordered) {
    const first = g.first;
    const letter = freeLetter(first, g.source, taken);
    if (!letter) {
      for (const i of g.members) {
        out[i].problem = 'Every letter in this section’s name is already spoken for.';
      }
      continue;
    }
    taken.add(letter);
    const why = letter !== first ? 'fallback' : kindOf(list[g.members[0]]?.label) ? 'kind' : 'name';
    const problem =
      letter === first
        ? null
        : RESERVED.has(first)
          ? `“${first}” is a Relay key (${reservedFor(first)}), so this section is on “${letter}”.`
          : `“${first}” is already taken, so this section is on “${letter}”.`;
    g.members.forEach((i, n) => {
      out[i] = {
        key: g.members.length > 1 ? `${letter}${n + 1}` : letter,
        why,
        problem,
      };
    });
  }

  return out;
}

/** The first letter of the ladder that is neither reserved nor already taken. */
function freeLetter(first, word, taken) {
  for (const ch of [first, ...ladder(word)]) {
    if (!RESERVED.has(ch) && !taken.has(ch)) return ch;
  }
  return null;
}

/** What a reserved letter already does, said in words the operator knows. */
function reservedFor(letter) {
  const hit = SHORTCUTS.find((s) => s.keys.some((k) => k.toLowerCase() === letter));
  return hit ? hit.label.toLowerCase() : 'already bound';
}

/** `{ key → section index }` for the keys this song actually handed out. */
export function keyIndex(sections) {
  const map = new Map();
  assignKeys(sections).forEach((k, i) => {
    if (k.key) map.set(k.key, i);
  });
  return map;
}

/**
 * One keystroke against the keys a song has.
 *
 * Returns `{ buffer, fire }` — the new buffer, and the key that just completed
 * (or `null`). **A half-typed key wins the next keystroke**: with `v1` and `v2`
 * on the song, `v` is not a key but it is a prefix, so it is held and `2`
 * completes it. With one verse there is no `v2`, `v` is itself the key, and it
 * fires on the first press with nothing held.
 *
 * A character that continues nothing is tried again as a FRESH start, so a
 * mistyped prefix does not swallow the key that follows it. A character that
 * starts nothing clears the buffer and fires nothing — silence, not a guess.
 *
 * This function never sees `Escape`, `b`, `a`, `d`, `?` or `/`: they are
 * claimed by `shortcuts.js` before the context switch is reached, and `RESERVED`
 * means they were never handed out in the first place. The guarantee is kept on
 * both doors on purpose.
 */
export function resolveKeystroke(buffer, char, keys) {
  const set = keys instanceof Set ? keys : new Set(keys ?? []);
  const ch = String(char ?? '').toLowerCase();
  if (!/^[a-z0-9]$/.test(ch)) return { buffer: '', fire: null };

  const step = (prefix) => {
    const candidate = prefix + ch;
    if (set.has(candidate)) return { buffer: '', fire: candidate };
    for (const k of set) {
      if (k.startsWith(candidate)) return { buffer: candidate, fire: null };
    }
    return null;
  };

  return step(String(buffer ?? '')) ?? step('') ?? { buffer: '', fire: null };
}
