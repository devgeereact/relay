/**
 * THE LEVEL METER'S ARITHMETIC — RG-257.
 *
 * The operator asked for the meter OBS draws: a horizontal bar on a decibel
 * scale with a held peak. That is a different instrument from the scrolling
 * trace this card has had, and both are honest — the trace answers *what has
 * the room been doing for twenty seconds*, a meter answers *how loud is it now,
 * and did it clip*.
 *
 * **Rule 12 governs what the pipeline DECIDES from a level, not what units a
 * meter draws.** That was checked before any of this was written, because it
 * decides whether the instrument is allowed at all. DECISIONS §19's own wording
 * is about the VOICE GATE and the AUTO-GAIN — the two things that must track
 * the room rather than a number — and §51 adds that *"a level meter moving is
 * not 'Relay heard a voice'"*. The trace this sits beside already plots linear
 * amplitude against an absolute box height, and `FirstRun.svelte` and
 * `views/Settings.svelte` already ship horizontal meters on absolute scales. A
 * decibel axis is a change of units, not a change of kind.
 *
 * **The one absolute mark permitted is full scale**, which §19 grants by name:
 * the sample had nowhere left to go, so saying so is a measurement. There is no
 * "too quiet", no "good level" and no "hot but not clipping" — `readingKind`'s
 * own comment refuses the last of those, because inventing a warning level is
 * inventing the absolute threshold rule 12 removed.
 *
 * **What the bar is fed.** `audio://chunk` already carries `peaks`, sixteen
 * TRUE peaks (`max(|sample|)`) at 25 ms resolution — `audio::envelope`, whose
 * own doc says *"the peak is the measurement a meter is expected to show, and
 * it is the one that makes clipping visible at all"*. Nothing needed to change
 * in Rust, and the held peak is a maximum over readings the card already keeps
 * for the trace.
 *
 * Pure: every figure is an argument, so the rules can be asserted without an
 * audio device, a canvas or a clock.
 */

/**
 * The bottom of the scale.
 *
 * −60 dBFS is where OBS, and most desks, put it: far enough down that a quiet
 * room still moves the bar, near enough that the top 20 dB an operator actually
 * works in gets a third of the width. It is the bottom of a PICTURE, not a
 * judgement about any signal — nothing compares a level to it to decide
 * anything.
 */
export const METER_FLOOR_DB = -60;

/**
 * Full scale, and the only absolute mark this meter draws.
 *
 * DECISIONS §19: clipping is the one absolutely defined fault in audio, because
 * the sample had nowhere left to go. Every other line on a meter would be a
 * threshold somebody invented.
 */
export const CLIP_DB = 0;

/**
 * Linear amplitude (0..1) as decibels relative to full scale.
 *
 * Silence is the FLOOR rather than `-Infinity`: an infinity cannot be drawn and
 * cannot be compared, and the floor is what a meter shows for silence in any
 * case. Anything quieter than the floor is reported AS the floor, so a caller
 * never has to clamp before drawing.
 */
export function dbOf(linear) {
  if (!Number.isFinite(linear) || linear <= 0) return METER_FLOOR_DB;
  const db = 20 * Math.log10(linear);
  return db <= METER_FLOOR_DB ? METER_FLOOR_DB : db;
}

/**
 * How much of the bar a reading lights, 0..1.
 *
 * LINEAR IN DECIBELS, which is the whole reason the meter is drawn in dB at
 * all: a bar linear in amplitude spends four fifths of its length on the top
 * 12 dB and pins a quiet preacher to the left edge — the same shape of mistake
 * as the one rule 12 was written about, in a picture rather than in a decision.
 */
export function meterFill(db) {
  if (!Number.isFinite(db)) return 0;
  const f = (db - METER_FLOOR_DB) / (CLIP_DB - METER_FLOOR_DB);
  return Math.min(1, Math.max(0, f));
}

/**
 * The loudest reading inside a window, or `null` when the window is empty.
 *
 * This is the held peak, and it decays by forgetting: a reading that falls out
 * of the window stops counting, so the mark slides down as the room quietens
 * rather than standing for the rest of a service.
 *
 * `null` and NOT a zero when nothing is in the window. A zero would draw a mark
 * at the floor, which reads as a measurement of silence — and the difference
 * between "nothing arrived" and "the room is silent" is the one this card's
 * head already spends three states on (rule 35).
 *
 * @param {{t:number,v:number}[]} buf readings, oldest first
 */
export function peakOf(buf, now, windowMs) {
  if (!Array.isArray(buf) || !buf.length) return null;
  let top = null;
  for (const r of buf) {
    if (!r || !Number.isFinite(r.t) || !Number.isFinite(r.v)) continue;
    const age = now - r.t;
    if (age < 0 || age > windowMs) continue;
    if (top === null || r.v > top) top = r.v;
  }
  return top;
}
