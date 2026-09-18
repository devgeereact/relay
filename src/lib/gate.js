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

/** Whole percentage points, or null. `null` is the absence — never `NaN%`, which
 *  is a figure on screen that is nobody's setting. */
function pct(v) {
  const n = Number(v);
  return Number.isFinite(n) ? `${Math.round(n * 100)}%` : null;
}

/**
 * Turn the capture store's gate facts into what a surface may show.
 *
 * @param {object} [state] the four facts, as `capture.js` holds them:
 *   `available` (is the bridge attached), `sensitivityKnown` (has the engine ever
 *   answered), `sensitivity` (the dial position Rust reported), `thresholds`
 *   (`{auto_fire, suggest}`) and `gateOnDial` (does that dial position actually
 *   produce those thresholds).
 * @returns {{readable: boolean, dial: number, autoPct: string|null,
 *            suggestPct: string|null, drifted: boolean, note: string}}
 */
export function describeGate(state) {
  const s = state || {};
  const autoPct = pct(s.thresholds?.auto_fire);
  const suggestPct = pct(s.thresholds?.suggest);
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
