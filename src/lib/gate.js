// THE GATE READ-OUT — what a surface is allowed to say about the detection gate.
//
// ── One control, two doors, and one place that decides the words ──────────────
//
// Detection sensitivity is a single 0-100 dial (DECISIONS §26, §96). It is drawn
// in two places — the dock's Live audio card and Settings → AI & Detection — and
// those are two doors onto one control, not two controls: one store, one command,
// and `detection://thresholds` moves both the moment either moves.
//
// What the two doors must NOT do is form their own opinions about what the
// figures mean. That is the shape of the defect this module was written to close:
// Settings used to carry a second pair of sliders over the same fact, pointing the
// opposite way (right = stricter, against the dial's right = more eager), over a
// range the dial cannot express, and one of them was labelled backwards —
// HYPER-AWARE at the end where Relay makes the FEWEST suggestions. Two controls
// over one fact cannot be reconciled by syncing, because every sync makes one of
// them lie. So there is one control, and this is the one place that turns it into
// words. The same reasoning as `outputHealth.js::describeScreen` and
// `updater.js::describeChannel`, for the same reason.
//
// ── THE THREE THINGS THAT MUST STAY APART (rule 35) ──────────────────────────
//
// A number on its own cannot separate these, and an operator reading it has no
// way to know which one they are looking at:
//
//   1. **There is no engine.** Nothing to read. Every figure here would be a
//      placeholder wearing a reading's clothes.
//   2. **The engine has not answered yet.** 50 is both the shipped default and a
//      perfectly ordinary real setting, so the figure alone cannot tell "the gate
//      is at 50" from "nobody has asked since this window opened".
//   3. **The engine answered, and the gate is not where the dial would put it.**
//      Three things move the gate without anybody touching the dial: a voice
//      profile restoring what it LEARNED, a room being applied, and the
//      self-calibration on every confirm and dismiss. The dial is still drawn,
//      at the position NEAREST the gate — and drawn without a word, that position
//      reads as somebody's deliberate setting. `Thresholds::follows_dial` in
//      router.rs answers this, beside the mapping it is a question about; the
//      answer rides on `detection://thresholds` and on `get_thresholds` as
//      `on_dial`, and this module only puts it into English.
//
// Pure: no store, no bridge, no clock. Everything it knows is in its argument.

/** A readiness figure, 0-100, as a string — or null. `null` is the absence, never
 *  `0`, which is itself a setting ("never fire") and would read as one.
 *
 *  ── WHAT THIS FIGURE IS, AND THE TWO COMPLAINTS THAT SHAPED IT ─────────────
 *
 *  It is the CONFIDENCE each bar needs, 0-100. Auto-fire is always the larger of
 *  the two, by exactly one band, because an auto-fire is the harder bar.
 *
 *  Two operator complaints, three days apart, about the same pair. First
 *  (§117): *"when the sensor is on Auto fire above 100, then it auto fires not
 *  when on 0"* — a figure under a slider reads as that slider's setting, and
 *  this one runs the opposite way to it. That was answered by inverting the
 *  number into a readiness. Then (§121): *"suggestions should be lower by 20 if
 *  auto fire is on 100 so auto fire has the higher priority"* — on a readiness
 *  scale a suggestion is the LARGER number, because it is the easier bar, and
 *  that reads as a suggestion outranking an auto-fire.
 *
 *  Only one framing satisfies both, and it is a word rather than arithmetic: say
 *  what each bar NEEDS. Under "needs", a smaller number is obviously the easier
 *  bar rather than the keener setting, and auto-fire is the bigger figure
 *  because it is the stricter rule. The figures still FALL as the dial rises and
 *  that cannot be helped — a bar you must clear is lower when more gets through.
 *  The dial is the control and keeps its own direction.
 *
 *  Worked out in `Thresholds::readiness` in router.rs, beside the curve, in the
 *  one language that owns the mapping. Nothing here re-derives it, for the same
 *  reason nothing here re-derives the dial position (DECISIONS §96, §117, §121). */
function figure(v) {
  const n = Number(v);
  return Number.isFinite(n) ? `${Math.round(n)}` : null;
}

/**
 * Turn the capture store's gate facts into what a surface may show.
 *
 * @param {object} [state] the facts, as `capture.js` holds them:
 *   `available` (is the bridge attached), `sensitivityKnown` (has the engine ever
 *   answered), `sensitivity` (the dial position Rust reported), `readiness`
 *   (`{auto_fire, suggest}` on the 0-100 scale that rises with the dial) and
 *   `gateOnDial` (does that dial position actually produce that gate).
 * @returns {{readable: boolean, dial: number, autoPct: string|null,
 *            suggestPct: string|null, drifted: boolean, note: string}}
 *   `autoPct`/`suggestPct` are what each bar NEEDS, 0-100; auto-fire is always
 *   the larger, by one band.
 *   The names are kept so no caller has to be found and changed; the meaning is
 *   documented on `figure` above and stated on both surfaces that print them.
 */
export function describeGate(state) {
  const s = state || {};
  const autoPct = figure(s.readiness?.auto_fire);
  const suggestPct = figure(s.readiness?.suggest);
  const dial = Number.isFinite(Number(s.sensitivity)) ? Number(s.sensitivity) : 50;

  // A reading needs all three: something to ask, an answer, and numbers in it.
  const readable = !!s.available && !!s.sensitivityKnown && autoPct !== null && suggestPct !== null;

  if (!s.available) {
    return {
      readable: false,
      dial,
      autoPct: null,
      suggestPct: null,
      // Not "the gate is at 50". There is no gate to be at anything.
      drifted: false,
      note: 'Relay’s engine is not attached, so there is nothing to read here.',
    };
  }
  if (!readable) {
    return {
      readable: false,
      dial,
      autoPct: null,
      suggestPct: null,
      drifted: false,
      note: 'The engine has not answered with the gate yet, so the position below is not a reading.',
    };
  }

  // DRIFT IS A CLAIM ABOUT A GATE THAT WAS READ. It is only ever asserted from an
  // answer the engine actually gave; inferring it from silence would be inventing
  // the worse of the two facts, which is the same defect in the other direction.
  const drifted = s.gateOnDial === false;

  return {
    readable: true,
    dial,
    autoPct,
    suggestPct,
    drifted,
    note: drifted
      ? `These are not the dial’s own figures. Relay has moved the gate since it was last set — from what has been confirmed and dismissed during services, or from a voice profile or room being applied. The dial shows ${dial} because that is the position nearest the gate as it stands, not because anybody set it there. Moving the dial puts both figures back on its curve.`
      : '',
  };
}
