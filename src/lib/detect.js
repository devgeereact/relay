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

/** What KIND of claim the machine is making, in words a volunteer can act on. */
export function methodLabel(d) {
  if (d?.method === 'semantic') return 'Paraphrase — a guess';
  if (d?.method === 'ambiguous') return 'Ambiguous reference';
  return 'Heard the reference';
}

/**
 * May this detection's confidence be shown as a percentage?
 *
 * ONLY for a heard reference. Printing "61%" beside a cosine invites the operator
 * to read it as "61% likely to be right", which is exactly what it is not — and
 * a number that lies is worse than no number, because it looks like information.
 */
export const showsConfidence = (d) => heard(d);
