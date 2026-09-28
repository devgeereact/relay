/**
 * What a speech model COSTS, in the two currencies a church actually spends —
 * RG-116, the project's P0.
 *
 * ## The finding this exists to answer
 *
 * A model is chosen on one panel (`ModelSetup.svelte`), and what that panel showed
 * was prose: *"hears them more accurately"*, *"best choice for African languages"*,
 * *"the best speech recognition Relay can run"*. Nobody had measured any of it.
 * Meanwhile Relay's own in-product advice, when the decoder fell behind, printed
 * three millisecond figures and said nothing about accuracy — so an operator who
 * followed it landed on `ggml-base`, and on 2026-09-06 that configuration put
 * **four wrong verses** into a service: 5 of 9 auto-fires correct, a 44% wrong-verse
 * rate against the 5% this product sets as its bar. The same morning, on the same
 * machine, `large-v3-turbo` was 3 of 3.
 *
 * So the model is an accuracy setting wearing a speed setting's clothes, and both
 * halves of the trade have to be legible at the point of choice.
 *
 * ## The two currencies
 *
 * **Update rate**, which is what an operator feels and is NOT the decode time.
 * Audio reaches the decoder in whole `HOP_MS` (200 ms) lumps and in no other size,
 * so the cadence is a whole number of hops (rule 32). `base` and `small` both land
 * in one hop — 59 ms and 152 ms — so `small` is the larger model at the same rate,
 * which `docs/qa/audits/PERF.md` §2 calls the most useful sentence in that document.
 * `large-v3-turbo` lands in four, and a church that chose it chose a quarter of the
 * update rate with nothing anywhere telling them.
 *
 * **Wrong verses**, scored through the router, because that is the only question
 * SPEC sets a bar for (rule 13: not how good the transcript reads, but which verse
 * would reach a wall).
 *
 * ## What it must never do
 *
 * Invent either. Three of the five catalogued models have never been scored for
 * accuracy at all and two have never been timed, and every function here returns a
 * null rather than a plausible sentence in those cases. Word error rate is
 * unmeasured in every language (`docs/LANGUAGES.md`) and nothing on this screen may
 * suggest otherwise — a sentence that sounds like a measurement is worse than an
 * admission, because an operator cannot tell it from one.
 *
 * Pure. Every figure comes from the Rust catalogue (`models.rs`), which carries its
 * own provenance in `measured_on`; nothing here holds a second copy of a number.
 */

/**
 * Transcript updates a second, from a published cadence. `null` when unmeasured.
 *
 * Printed with at most two decimals and no trailing zeros, because the two real
 * answers are `5` and `1.25` and "5.00" reads like false precision.
 */
export function updatesPerSecond(cadenceMs) {
  if (!Number.isFinite(cadenceMs) || cadenceMs <= 0) return null;
  return Math.round((1000 / cadenceMs) * 100) / 100;
}

/**
 * How much slower to update than the model Relay recommends, as a whole-ish
 * multiple. `1` means the same rate — which is the `small` case and the whole point.
 */
function slowdown(model, recommended) {
  const mine = Number(model?.cadence_ms);
  const theirs = Number(recommended?.cadence_ms);
  if (!Number.isFinite(mine) || !Number.isFinite(theirs) || theirs <= 0) return null;
  return Math.round((mine / theirs) * 100) / 100;
}

/**
 * One sentence about speed, or `null` when this model has never been timed.
 *
 * `models` is the whole catalogue, because the useful fact is comparative: an
 * absolute "5 updates a second" means nothing to a volunteer, and "the same rate as
 * the one we recommend" is the sentence that decides. Taking it from the catalogue
 * rather than from a constant is what keeps this from becoming a second copy of
 * PERF §2's table.
 */
export function describeSpeed(model, models = []) {
  const ups = updatesPerSecond(Number(model?.cadence_ms));
  if (ups == null) return null;
  const on = model?.measured_on ? ` on ${model.measured_on}` : '';
  const head = `About ${ups} transcript updates a second${on}.`;
  const recommended = models.find((m) => m?.recommended);
  const factor = recommended && recommended.id !== model?.id ? slowdown(model, recommended) : null;
  if (factor == null) return head;
  if (factor === 1) {
    // PERF §2: "`small` is free." It costs 2.6× the decode of `base` and lands in the
    // same single hop, so the operator gives up nothing they can feel.
    return `${head} That is the same rate as the recommended model, so choosing this one costs no speed at all.`;
  }
  if (factor > 1) {
    return `${head} That is ${factor}× slower to update than the recommended model — a verse takes noticeably longer to reach the screen, and every reference has to survive a second pass before it may fire, which is that much longer too.`;
  }
  return `${head} That is ${Math.round((1 / factor) * 100) / 100}× faster to update than the recommended model.`;
}

/**
 * What is known about wrong verses — always an answer, never a blank.
 *
 * `{ measured: false }` is the honest and common case, and the caller must render it
 * rather than hiding the row: a missing line reads as "nothing to worry about",
 * which is exactly the reading this finding is about. `small` is the one that matters
 * most — it is the model Relay's own lag warning now points at, and its single
 * accuracy attempt returned 1 of 8 and was WITHDRAWN as invalid, having measured
 * whisper's language election wandering rather than the model.
 */
export function describeAccuracy(model) {
  const text = typeof model?.accuracy === 'string' ? model.accuracy.trim() : '';
  if (text) return { measured: true, text };
  return {
    measured: false,
    text: 'Never measured. Relay has no evidence about how well this model hears a sermon — in any language.',
  };
}
