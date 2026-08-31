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
 * language the operator reads. `Live.svelte` renders `$t(methodKey(d))`.
 */
export function methodKey(d) {
  if (d?.method === 'semantic') return 'live.paraphrase_a_guess';
  if (d?.method === 'ambiguous') return 'live.ambiguous_reference';
  // `uncertain_book` — the chapter and verse were heard, the BOOK was not. Either
  // an edit-distance repair of a misheard word, or an everyday word that happens
  // to be a book name ("song two twelve"). It reads as a normal reference and it
  // is the one the operator most needs to look at, so it must not fall through to
  // "heard the reference" — that sentence would be a lie with a real parse
  // confidence standing behind it, which is how "hymn number three sixteen" put
  // Numbers 3:16 in front of a congregation. See detection.rs.
  if (d?.method === 'uncertain_book') return 'live.book_name_uncertain';
  return 'live.heard_the_reference';
}

/**
 * May this detection's confidence be shown as a percentage?
 *
 * ONLY for a heard reference. Printing "61%" beside a cosine invites the operator
 * to read it as "61% likely to be right", which is exactly what it is not — and
 * a number that lies is worse than no number, because it looks like information.
 */
export const showsConfidence = (d) => heard(d);

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
