// How a detection is PRESENTED to the operator — heard, or guessed.
//
// This is one function's worth of logic and it gets its own module, because it is
// the frontend half of Relay's central safety decision and it must be testable.
//
// The rule it serves (docs/DECISIONS.md, router.rs::decide): only a DIRECT match —
// a reference the parser actually heard — may ever auto-fire. A semantic
// (paraphrase) match may never auto-fire at ANY score, at ANY sensitivity, because
// its "confidence" is a TF-IDF cosine: a distance in an arbitrary vector space, not
// a probability. The two numbers are on incomparable scales.
//
// The console rendered them identically anyway — "AI suggestion — 92% match" for
// both — so the one distinction the entire gate is built on was invisible to the
// one person who is allowed to overrule it. The operator was being asked to be the
// human in the loop while being shown nothing to judge with.

/**
 * Did Relay HEAR this reference, or GUESS it from meaning?
 *
 * Direct = heard (a real parse confidence). Semantic = a paraphrase guess.
 * Ambiguous = parsed but genuinely undecidable ("Revelation 22" → 22:1 or 2:2),
 * and its confidence is a hardcoded placeholder, not a measurement.
 */
export const heard = (d) => d?.method === 'direct';

/**
 * What KIND of claim the machine is making — as an i18n KEY, not a sentence.
 *
 * This module stays pure and testable; it simply is not the place that decides which
 * language the operator reads. `DetectionInspector.svelte` renders `$t(methodKey(d))`.
 */
export function methodKey(d) {
  if (d?.method === 'semantic') return 'live.paraphrase_a_guess';
  // `quoted` — a contiguous run of the preacher's own words, verbatim in this
  // verse. Strong evidence about WHICH verse and none at all that anybody wants
  // it on a wall, so it is not `direct` and never will be: a preacher quotes far
  // more scripture than a congregation is shown. See detection.rs::PhraseIndex.
  if (d?.method === 'quoted') return 'live.quoted_scripture';
  // `reading` — a run of the preacher's own words long enough, and held by one
  // verse alone, that Relay is treating it as the verse being READ (DECISIONS
  // §118, the operator's instruction of 2026-09-23). It is the ONE method other
  // than `direct` that may reach a wall unattended, and the card must say so
  // rather than letting it wear the paraphrase's sentence — which promises a
  // guarantee that no longer holds here. See detection.rs::Reading.
  if (d?.method === 'reading') return 'live.read_aloud';
  if (d?.method === 'ambiguous') return 'live.ambiguous_reference';
  // `uncertain_book` — the chapter and verse were heard, the BOOK was not. Either
  // an edit-distance repair of a misheard word, or an everyday word that happens
  // to be a book name ("song two twelve"). It reads as a normal reference and it
  // is the one the operator most needs to look at, so it must not fall through to
  // "heard the reference" — that sentence would be a lie with a real parse
  // confidence standing behind it, which is how "hymn number three sixteen" put
  // Numbers 3:16 in front of a congregation. See detection.rs.
  if (d?.method === 'uncertain_book') {
    return bookFromMemory(d) ? 'live.book_from_memory' : 'live.book_name_uncertain';
  }
  return 'live.heard_the_reference';
}

/**
 * A bare "verse N" that Relay answered from the passage on the screen. It arrives
 * as `uncertain_book` — the book was not heard — with the evidence `"verse N"`,
 * because that is all the preacher said. Distinct from a misheard book word: here
 * nobody said ANY book, Relay assumed the one on the wall. FIELD 2026-09-20 put
 * Psalms 55:1 up for Hosea 6:1 this way, labelled as heard.
 */
export const bookFromMemory = (d) =>
  d?.method === 'uncertain_book' && /^verse \d+$/.test(String(d?.matched_text ?? ''));

/**
 * The SHORT form of `methodKey`, for the claim card's chip.
 *
 * Live rendered a hard-coded ternary — `heard(d) ? 'Heard' : 'Paraphrase'` — so
 * all three non-direct methods wore the same word. `uncertain_book` is the method
 * added after "please turn to hymn number three sixteen" put **Numbers 3:16** in
 * front of a congregation, and CLAUDE.md rule 10 calls it the claim an operator
 * most needs to look at; on the card they read, it was indistinguishable from a
 * TF-IDF paraphrase. `methodKey` existed for exactly this and was imported into
 * `Live.svelte` without ever being called.
 *
 * Short because it is a chip. The full sentence is `methodKey`, and the note
 * beneath the chip is `methodNoteKey` — three registers, one source of truth for
 * which method this is.
 */
export function methodBadgeKey(d) {
  if (d?.method === 'semantic') return 'live.badge_paraphrase';
  if (d?.method === 'quoted') return 'live.badge_quoted';
  if (d?.method === 'reading') return 'live.badge_reading';
  if (d?.method === 'ambiguous') return 'live.badge_ambiguous';
  if (d?.method === 'uncertain_book') {
    return bookFromMemory(d) ? 'live.badge_from_memory' : 'live.badge_book_uncertain';
  }
  return 'live.badge_heard';
}

/**
 * The sentence under an unheard claim, per method.
 *
 * "not a spoken reference" is TRUE of a paraphrase and FALSE of the other two —
 * an ambiguous reference WAS spoken, and for `uncertain_book` the chapter and the
 * verse were both heard and only the book was repaired. Live printed that one
 * sentence for all three, so the card said the opposite of what had happened on
 * the two methods where it mattered most.
 *
 * Returns null for a heard reference, which shows a confidence bar instead.
 */
export function methodNoteKey(d) {
  if (d?.method === 'semantic') return 'live.not_a_spoken_reference';
  if (d?.method === 'quoted') return 'live.note_quoted';
  if (d?.method === 'reading') return 'live.note_reading';
  if (d?.method === 'ambiguous') return 'live.note_ambiguous';
  if (d?.method === 'uncertain_book') {
    return bookFromMemory(d) ? 'live.note_from_memory' : 'live.note_book_uncertain';
  }
  return null;
}

/**
 * May this detection's confidence be shown as a percentage?
 *
 * ONLY for a heard reference. Printing "61%" beside a cosine invites the operator
 * to read it as "61% likely to be right", which is exactly what it is not — and
 * a number that lies is worse than no number, because it looks like information.
 *
 * `reading` may AUTO-FIRE since DECISIONS §118 and still shows no number, which
 * is the point worth keeping separate: whether a claim may reach a wall and
 * whether its number means anything are two questions, and they only looked like
 * one while `direct` was the answer to both. A reading's confidence is derived
 * from how many words ran together — a count, on a scale of its own — so it is
 * exactly the kind of figure this function exists to keep off the screen.
 */
export const showsConfidence = (d) => heard(d);

/**
 * The order the claim column shows pending claims in (RG-192, 2026-09-21).
 *
 * Measured at 1440×900 with three claims pending: the third card's Accept & fire
 * was below the fold of its own column, and newest-first meant the one that fell
 * off was the oldest — the HEARD one, the only class that may auto-fire and the
 * one an operator most needs a hand on. A heard reference comes first, whatever
 * arrived last; within each group the arrival order is kept.
 */
export function orderClaims(list) {
  const arr = Array.isArray(list) ? list : [];
  return [...arr.filter((d) => heard(d)), ...arr.filter((d) => !heard(d))];
}

/**
 * Is there actually a verse behind this reference?
 *
 * `emit_detections` deliberately does NOT drop a reference that parsed cleanly but
 * resolves to nothing — "Psalms 23:99" out of garbled speech, or a book/chapter
 * pair that does not exist. Silence would be worse: the operator would never learn
 * that Relay is mishearing numbers, which is the single most useful thing that
 * suggestion can tell them. So it is demoted to a suggestion and marked
 * `in_library: false` (`pipeline.rs`).
 *
 * **Nothing on the frontend read that flag.** The suggestion rendered exactly like
 * a real one, with the same amber Accept button beside it — and accepting it
 * failed, after the click, with "…isn't in the Bible text". A control that looks
 * identical to its working neighbours and cannot work is the same defect class as
 * a status badge that cannot detect its own failure.
 *
 * **Absent means yes**, deliberately: an older payload, the LAN remote, or any
 * producer that does not set the field must not have its suggestions greyed out on
 * a guess. This can only ever add a warning where the backend explicitly said so.
 */
export const inLibrary = (d) => d?.in_library !== false;

/**
 * Is this detection's evidence a CONTIGUOUS SPAN of what was said?
 *
 * `direct` carries the words the reference was parsed from ("proverbs chapter
 * six verse sixteen") and `quoted` carries a run of scripture read aloud. Both
 * are things a person said, in the order they said them, so both may be shown
 * inside quotation marks.
 *
 * `semantic` may not. Its evidence is `terms.join(" · ")` — the words that
 * contributed most to a TF-IDF cosine, in weight order, from anywhere in the
 * verse. Rendered inside quotation marks it read as
 *
 *     “lord · shepherd”
 *
 * which is a quotation of something nobody said. The operator's instruction of
 * 2026-09-20 was that the evidence has to be the words together as they are in
 * the scripture; where it genuinely is not, it must stop dressing as though it
 * were. Same principle as `showsConfidence`: a presentation that lies about what
 * kind of thing it is showing is worse than showing nothing.
 */
export const evidenceIsASpan = (d) =>
  d?.method === 'direct' || d?.method === 'quoted' || d?.method === 'reading';
