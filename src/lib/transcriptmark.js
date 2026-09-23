// WHICH LINES OF THE TRANSCRIPT DID RELAY HEAR SCRIPTURE IN (RG-278).
//
// The operator, 2026-09-23: *"can we introduce color coding to the transcript?
// when you hear any paraphrasing of the scripture or an actual scripture can it
// be colour coded? eg - psalm 23 verse 1, and if preacher says 'the lord is my
// shepherd' kind of thing"*.
//
// The Live transcript card printed every closed line in one ink, so the two
// things the whole gate is built on — a reference Relay HEARD, and a meaning it
// GUESSED at — looked identical to the person watching the words go past. This
// module decides which mark a line carries and nothing else.
//
// ── WHY IT IS A MODULE AND NOT FOUR LINES IN THE CARD ───────────────────────
//
// Because the interesting part is a substring match over two strings that came
// from different places, and that is where it goes wrong: `matched_text` is the
// parser's slice, the line is whisper's prose, and an unguarded `includes` on an
// empty needle paints the entire transcript. Every input is an argument, so the
// rule can be driven without a socket, a store or a window.
//
// ── WHAT IT DELIBERATELY DOES NOT KNOW ──────────────────────────────────────
//
// Whether the verse reached a screen. The transcript card is a record of what
// Relay HEARD; an auto-fire, an accepted suggestion and a dismissed one all
// leave the same mark, because the same thing was heard in all three cases. A
// mark that changed with the operator's decision would be a second, quieter
// claim about the wall — and the wall already has amber, which nothing here may
// borrow (CLAUDE.md rule 18).
//
// Nothing here carries a confidence either. Only `Direct` has a real parse
// confidence; every other method's number is a cosine or a count, and the ink
// says which kind the claim is, which is the thing a number cannot.
import { heard } from './detect.js';

/**
 * The two texts, reduced to their words.
 *
 * `matched_text` is `detection.rs`'s slice of the tokens it parsed; the line is
 * whatever whisper wrote, with its capitals and its full stops. "Psalm 23,
 * verse 1." and "psalm 23 verse 1" are the same words and are not the same
 * string, and the card would have marked almost nothing if they were compared
 * as bytes.
 *
 * `\p{L}` rather than `a-z`: Yorùbá, Swahili and Hausa are tier-1 (CLAUDE.md,
 * "Priority languages"), and a byte-class regex strips the tone marks off a
 * Yorùbá line and then fails to match the claim that came out of it.
 */
export function normalise(s) {
  return String(s ?? '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();
}

/**
 * Is this claim's evidence present in this line, as whole words?
 *
 * Padded with spaces on both sides, because a bare `includes` matches across a
 * word boundary: " psalm 2 " is not in " psalm 23 " and `String.includes` says
 * it is. Naming the wrong verse beside a line is the transcript card's version
 * of firing the wrong one.
 *
 * An empty needle never matches. `''` is a substring of every line, so one
 * malformed claim would otherwise mark the whole service.
 */
function evidenceIsInLine(claim, lineWords) {
  const needle = normalise(claim?.matched_text);
  if (!needle) return false;
  return ` ${lineWords} `.includes(` ${needle} `);
}

/**
 * What kind of mark does this line carry — nothing, a heard reference, a guess?
 *
 * @param line   `{ t, at }`, one closed transcript line as the card holds it.
 * @param claims the claims in hand, in any order.
 * @returns `null`, or `{ kind: 'heard' | 'guess', reference, claim }`.
 *
 * TWO KINDS AND NO THIRD, and the split is `detect.js::heard` rather than a
 * second table here — one concept, one name. Only `Direct` means Relay heard the
 * reference; `semantic`, `quoted`, `ambiguous` and `uncertain_book` are all
 * capped at `Suggest` by the router at any score (rule 10), so none of them may
 * wear the stronger ink. The card names the method itself, from the claim this
 * carries, so the five stay distinguishable where there is room to say so.
 *
 * A HEARD REFERENCE WINS over a guess on the same line, whatever order they
 * arrive in — a window that both states a reference and quotes the verse is
 * ordinary preaching, and the reference is the stronger fact about it. Within
 * one kind the first match in the list given wins; the store hands them over
 * newest first.
 */
export function markFor(line, claims) {
  const list = Array.isArray(claims) ? claims : [];
  if (!list.length) return null;
  const words = normalise(line?.t);
  if (!words) return null;
  let best = null;
  for (const claim of list) {
    // A claim with nothing to name is an ink with no information under it: the
    // mark's whole job is to say WHICH verse.
    if (!claim?.reference) continue;
    if (!evidenceIsInLine(claim, words)) continue;
    const kind = heard(claim) ? 'heard' : 'guess';
    if (!best) best = { kind, reference: claim.reference, claim };
    if (kind === 'heard') return { kind, reference: claim.reference, claim };
  }
  return best;
}

/**
 * A line's identity for the memo below.
 *
 * The time code AND the words. `finalsAt` repeats — two decodes can land inside
 * the same second — so the stamp alone would fuse two different lines into one
 * remembered mark.
 */
export const lineKey = (line) => `${line?.at ?? ''}·${line?.t ?? ''}`;

/**
 * How many marked lines are remembered. Far past a service's worth: a 110-minute
 * service produced eight auto-fires, and this is a cap against a pathological
 * run rather than a budget anybody is expected to reach.
 */
export const MARK_MEMO_CAP = 600;

/**
 * The marks for a whole card of lines, with what has already been marked kept.
 *
 * ── WHY THIS REMEMBERS AT ALL ───────────────────────────────────────────────
 *
 * The claims in hand are a moving window, and a short one: `detections` holds a
 * pending suggestion for 45 seconds (`SUGGESTION_TTL_MS`) and `resolvedDetections`
 * keeps four receipts. **An auto-fire never enters the pending list at all** — it
 * goes straight to the screens and its suggestion is removed. So the strongest
 * claim of the service is the one that leaves soonest, and a mark derived from
 * the window alone would blink out from under the operator while the words it
 * belongs to were still on the card.
 *
 * Remembering claims nothing extra: Relay did hear scripture in that line, and
 * that stays true however the claim was resolved.
 *
 * `memo` is a `Map` this function READS AND WRITES. That is deliberate rather
 * than lazy: a fold returning a new memo cannot be driven from a Svelte `$:`
 * block, because assigning the memo inside the block that reads it invalidates
 * it and re-runs for ever (`safe_not_equal` always invalidates an object, even
 * an identical reference). A Map that is mutated and never reassigned is
 * invisible to the scheduler. It is still an argument, so this is still driven
 * from a test with `new Map()` and nothing else.
 */
export function rememberMarks(lines, claims, memo) {
  const rows = Array.isArray(lines) ? lines : [];
  return rows.map((line) => {
    const key = lineKey(line);
    const fresh = markFor(line, claims);
    const held = memo.get(key) ?? null;
    // A heard reference may replace a remembered guess and never the other way
    // round. The paraphrase often arrives first — the preacher quotes the verse
    // and names it a window later — and the line was heard.
    if (fresh && (!held || (held.kind === 'guess' && fresh.kind === 'heard'))) {
      memo.set(key, fresh);
      if (memo.size > MARK_MEMO_CAP) memo.delete(memo.keys().next().value);
      return fresh;
    }
    return held;
  });
}
