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
 * HOW MUCH OF THE VERSE WAS ACTUALLY SAID — in words, or nothing at all.
 *
 * The operator, 2026-09-25: *"let the transcript be colour coded with the highest
 * match so its easy to filter through what's closest to what was heard"*. A
 * ranking, then — and the whole difficulty is that "strength" has no single
 * answer across the six methods, while the number that looks most like one is the
 * number that put a wrong verse in front of a congregation.
 *
 * ── WHY THIS IS A COUNT AND NEVER A SCORE ───────────────────────────────────
 *
 * `confidence` is on a DIFFERENT SCALE PER METHOD, and only `direct` calibrates
 * it (`DetectionMethod::confidence_is_calibrated`). Sorting the six by that field
 * puts `uncertain_book` near the top, because its confidence is real and is about
 * a word nobody said — "please turn to hymn number three sixteen" parsed
 * **Numbers 3:16** at 0.840 against a 0.50 bar (rule 10). A strength badge is a
 * promotion, and promoting that claim on a scanning operator's eye is the exact
 * failure the router's cap exists to prevent.
 *
 * A RUN IS DIFFERENT AND THAT IS THE WHOLE DESIGN. For `quoted` and `reading`,
 * `matched_text` is `detection.rs::PhraseHit::phrase` — a contiguous span of the
 * speaker's own words, held by one verse, in the order they said them
 * (`detect.js::evidenceIsASpan`). Its length is a COUNT, with a unit, of words a
 * person said; `PhraseHit::run`'s own doc calls it "the operator-facing measure of
 * how much of the verse was actually said". A count cannot be misread as a
 * likelihood the way a bare percentage can, and two counts are comparable to each
 * other, which is what "filter through what's closest" needs.
 *
 * ── AND THIS IS NOT RE-DERIVED, IT IS READ ──────────────────────────────────
 *
 * `phrase` is `words[i..i + n].join(" ")` over `phrase_words`, which splits on
 * anything not ASCII-alphanumeric — so the phrase is exactly `n` bare tokens
 * joined by single spaces, and `normalise` leaves bare tokens alone. The word
 * count IS `run`, losslessly. `run` itself is not on the wire (`DetectionEvent`
 * carries `matched_text`, `method` and `confidence` and no length), and it would
 * be the better field; until it is there, this reads the field it was joined into
 * rather than inventing a measure of its own.
 *
 * ── THE FOUR METHODS THAT GET NOTHING, AND WHY THAT IS THE HONEST ANSWER ────
 *
 *   * `semantic` — a TF-IDF cosine, and its evidence is not even a span: it is
 *     `terms.join(" · ")`, the words that moved the cosine, in weight order, from
 *     anywhere in the verse. Counting them counts terms the index liked.
 *   * `ambiguous` — its confidence is a hardcoded placeholder, not a measurement.
 *   * `uncertain_book` — rule 10, above.
 *   * `direct` — its evidence is the REFERENCE, not the verse. "first epistle of
 *     john chapter four verse eight" is not a better match than "ps 23 1"; it is
 *     a wordier way of saying one. There is nothing to count, and the reference is
 *     printed in full already.
 *
 * The absence is itself part of the ranking: a claim with no honest measure shows
 * no measure, and `markFor` sorts it below one that has it.
 *
 * @returns a positive integer, or `null` where no honest measure exists.
 */
export function runWords(claim) {
  const method = claim?.method;
  if (method !== 'quoted' && method !== 'reading') return null;
  const words = normalise(claim?.matched_text);
  if (!words) return null;
  return words.split(' ').length;
}

/**
 * Is this mark the STRONG tier of a quotation — the preacher reading the verse?
 *
 * The answer is the method's name and nothing else. `for_quotation` promotes a run
 * to `Reading` on TWO conditions, eight words AND `sole`, and **`sole` is not on
 * the wire** — so the frontend cannot reproduce that decision and must not
 * pretend to. Comparing a count here against a local copy of `READING_RUN_WORDS`
 * would be rule 35's third instance in a new costume: a component keeping its own
 * copy of a gate that lives in Rust, drifting the moment the bar moves.
 *
 * Takes a MARK rather than a claim, because that is what the card holds.
 */
export const readAloud = (mark) => mark?.claim?.method === 'reading';

/**
 * Is `a` a strictly stronger mark than `b`?
 *
 * TWO KEYS, IN THIS ORDER, AND THE ORDER IS THE SAFETY PROPERTY.
 *
 * 1. A HEARD REFERENCE OUTRANKS EVERY RUN, AT EVERY LENGTH. A 25-word reading is
 *    strong evidence about WHICH verse and no evidence that anybody said its
 *    name. If a long enough run could outrank a hearing, the strength scale would
 *    have quietly become the trust ranking — and the trust ranking is rule 10's,
 *    decided in `router.rs`, not on this card.
 * 2. WITHIN ONE KIND, THE LONGER RUN WINS, and no run at all loses to any run.
 *    That is the operator's *"highest match"*: a preacher reading aloud produces a
 *    short run in a neighbouring verse and a long one in the verse being read
 *    (`PhraseHit::sole`, the John 3:15/3:16 case), and before this the mark was
 *    whichever claim the store happened to hand over first.
 *
 * STRICTLY stronger, so an equal claim never displaces an earlier one — the list
 * order stays the tie-break, as it was.
 */
function stronger(a, b) {
  if (a.kind !== b.kind) return a.kind === 'heard';
  return (a.run ?? 0) > (b.run ?? 0);
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
 * ordinary preaching, and the reference is the stronger fact about it. Within one
 * kind the STRONGEST MATCH wins and an equal one never displaces an earlier one,
 * so the list order is still the tie-break; the store hands them over newest
 * first. See `stronger`.
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
    const mark = {
      kind: heard(claim) ? 'heard' : 'guess',
      reference: claim.reference,
      claim,
      run: runWords(claim),
    };
    // NO EARLY RETURN ON THE FIRST HEARD CLAIM any more. It was equivalent while
    // the only key was the kind; with a second key it would stop the scan before
    // the strength comparison had seen the rest of the list.
    if (!best || stronger(mark, best)) best = mark;
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
    // A STRONGER CLAIM MAY REPLACE A REMEMBERED ONE AND A WEAKER MAY NOT, on the
    // one comparison `markFor` uses within a window (`stronger`). Two cases, both
    // ordinary preaching: the paraphrase arrives first and the reference a window
    // later, so a hearing replaces a guess; and the window slides forward while
    // the preacher is still reading, so the same line is claimed again with more
    // of the verse in it. Freezing the first would hold the strongest line of the
    // service at its weakest reading of it.
    if (fresh && (!held || stronger(fresh, held))) {
      memo.set(key, fresh);
      if (memo.size > MARK_MEMO_CAP) memo.delete(memo.keys().next().value);
      return fresh;
    }
    return held;
  });
}
